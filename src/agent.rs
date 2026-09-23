use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

use crate::inference::InferenceEngine;
use crate::persistence::Database;
use crate::skills::{SkillExecutionRequest, SkillRegistry};

pub const DEFAULT_MAX_CONTEXT_TOKENS: usize = 8192;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub output: String,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub step_index: usize,
    pub thought: Option<String>,
    pub tool_calls: Vec<ToolExecutionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionResult {
    pub final_response: String,
    pub steps: Vec<AgentStep>,
    pub turns_taken: usize,
    pub duration_ms: u128,
    pub completed: bool,
}

#[derive(Clone)]
pub struct AgentEngine {
    inference: Arc<dyn InferenceEngine>,
    skills: Arc<SkillRegistry>,
    db: Option<Arc<Database>>,
}

impl AgentEngine {
    pub fn new(
        inference: Arc<dyn InferenceEngine>,
        skills: Arc<SkillRegistry>,
        db: Option<Arc<Database>>,
    ) -> Self {
        Self {
            inference,
            skills,
            db,
        }
    }

    pub fn skills(&self) -> Arc<SkillRegistry> {
        self.skills.clone()
    }

    pub fn estimate_tokens(messages: &[serde_json::Value]) -> usize {
        let mut count = 0;
        for m in messages {
            if let Some(content) = m.get("content").and_then(|c| c.as_str()) {
                count += (content.split_whitespace().count() * 4) / 3 + 4;
            }
            if let Some(calls) = m.get("tool_calls").and_then(|c| c.as_array()) {
                for call in calls {
                    if let Ok(serialized) = serde_json::to_string(call) {
                        count += (serialized.split_whitespace().count() * 4) / 3 + 4;
                    }
                }
            }
        }
        count
    }

    pub fn prune_context_window(
        messages: &mut Vec<serde_json::Value>,
        token_budget: usize,
    ) -> bool {
        if messages.len() <= 2 {
            return false;
        }
        let mut pruned = false;
        while messages.len() > 2 && Self::estimate_tokens(messages) > token_budget {
            // Retain system prompt (index 0) and initial user goal (index 1), prune oldest historical turn
            messages.remove(2);
            pruned = true;
        }
        pruned
    }

    pub async fn execute_task(
        &self,
        goal: &str,
        system_override: Option<&str>,
        max_turns: usize,
    ) -> Result<AgentExecutionResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = Instant::now();
        let tools = self.skills.to_openai_tools();

        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let user = std::env::var("USER").unwrap_or_else(|_| "sovereign".to_string());
        let default_system = format!(
            "You are AIEN, a sovereign native AI systems agent running on NVIDIA DGX Spark (Grace Blackwell GB10) as user {user}. Active workspaces reside strictly in {home}/workspace/ (aegis-runtime, aien-harness-publish, etc.) and {home}/atlas-prime-workspace/. Default working directory is {home}/workspace. You have native tools available to run a catalogued local command (bash_eval: git status, git diff, git log -1 --oneline, or ls), inspect files (read_file, list_dir, git_status), and modify files (write_file). Memory search is not connected. Do not claim that it is. Never run broad root filesystem scans or find /. Invoke tools directly on specific workspace targets. Adhere strictly to the unslop standard: zero em dashes and zero en dashes, no transitional fluff, and direct technical proof. When finished, provide a concise final summary.",
            user = user,
            home = home_dir
        );

        let sys_prompt = system_override.unwrap_or(&default_system);

        let mut messages: Vec<serde_json::Value> = vec![
            json!({
                "role": "system",
                "content": sys_prompt
            }),
            json!({
                "role": "user",
                "content": goal
            }),
        ];

        let mut steps: Vec<AgentStep> = Vec::new();
        let mut final_response = String::new();
        let mut turns_taken = 0;
        let mut completed = false;

        for turn in 1..=max_turns {
            turns_taken = turn;
            info!("Agent loop turn {}/{}", turn, max_turns);

            // Context window management: trim to budget before sending to model
            Self::prune_context_window(&mut messages, DEFAULT_MAX_CONTEXT_TOKENS);

            let turn_res = self
                .inference
                .generate_chat_with_tools(&messages, Some(&tools), Some(0.2), Some(2048))
                .await?;

            let thought = turn_res.reasoning.clone();
            let has_tool_calls = turn_res
                .tool_calls
                .as_ref()
                .map(|calls| !calls.is_empty())
                .unwrap_or(false);

            if has_tool_calls {
                let tool_calls = turn_res.tool_calls.unwrap();
                let mut tool_exec_records = Vec::new();

                let tc_json: Vec<serde_json::Value> = tool_calls
                    .iter()
                    .map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.function.name,
                                "arguments": tc.function.arguments
                            }
                        })
                    })
                    .collect();

                messages.push(json!({
                    "role": "assistant",
                    "content": turn_res.content.clone().unwrap_or_default(),
                    "tool_calls": tc_json
                }));

                for tc in tool_calls {
                    info!("Executing tool: {} (call_id: {})", tc.function.name, tc.id);
                    let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                        .unwrap_or_else(|_| json!({"raw": tc.function.arguments}));

                    let req = SkillExecutionRequest {
                        skill_name: tc.function.name.clone(),
                        arguments: args.clone(),
                    };

                    let res = self.skills.execute(&req);
                    let output_str = if res.success {
                        res.output.clone()
                    } else {
                        res.error
                            .clone()
                            .unwrap_or_else(|| "Unknown error".to_string())
                    };

                    tool_exec_records.push(ToolExecutionRecord {
                        call_id: tc.id.clone(),
                        tool_name: tc.function.name.clone(),
                        arguments: args,
                        output: output_str.clone(),
                        success: res.success,
                    });

                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": output_str
                    }));
                }

                steps.push(AgentStep {
                    step_index: turn,
                    thought,
                    tool_calls: tool_exec_records,
                });
            } else {
                let content = turn_res.content.unwrap_or_default();
                final_response = if !content.trim().is_empty() {
                    content
                } else if let Some(r) = turn_res.reasoning {
                    r
                } else {
                    "Task processed with zero textual output.".to_string()
                };

                steps.push(AgentStep {
                    step_index: turn,
                    thought,
                    tool_calls: Vec::new(),
                });

                completed = true;
                break;
            }
        }

        if !completed && final_response.is_empty() {
            final_response = format!(
                "Agent reached maximum turn limit ({}) before producing terminal output.",
                max_turns
            );
        }

        let duration = start.elapsed().as_millis();

        if let Some(ref db) = self.db {
            let _ = db.record_turn("agent_autonomous", goal, &final_response, duration as u64);
        }

        Ok(AgentExecutionResult {
            final_response,
            steps,
            turns_taken,
            duration_ms: duration,
            completed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HttpInferenceBackend;

    #[tokio::test]
    async fn test_agent_engine_creation_and_skills() {
        let inference = Arc::new(HttpInferenceBackend::new(None, None));
        let skills = Arc::new(SkillRegistry::new());
        let agent = AgentEngine::new(inference, skills.clone(), None);

        let registered = agent.skills().list_skills();
        assert!(!registered.is_empty());
    }

    #[test]
    fn test_agent_token_estimation_and_boundary_pruning() {
        let mut messages = vec![
            json!({"role": "system", "content": "You are AEGIS."}),
            json!({"role": "user", "content": "Initial user task."}),
            json!({"role": "assistant", "content": "Executing intermediate action step 1."}),
            json!({"role": "tool", "content": "Output of intermediate step 1 with lots of verbose debugging details here."}),
            json!({"role": "assistant", "content": "Executing intermediate action step 2."}),
            json!({"role": "tool", "content": "Output of intermediate step 2."}),
        ];

        let initial_tokens = AgentEngine::estimate_tokens(&messages);
        assert!(initial_tokens > 20);

        // Pruning with huge budget should not change messages
        let pruned_none = AgentEngine::prune_context_window(&mut messages, 10000);
        assert!(!pruned_none);
        assert_eq!(messages.len(), 6);

        // Pruning with tiny budget should remove intermediate turns down to index 0 and 1
        let pruned = AgentEngine::prune_context_window(&mut messages, 10);
        assert!(pruned);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");

        // Boundary: calling prune when only 2 messages remain should return false
        let pruned_again = AgentEngine::prune_context_window(&mut messages, 0);
        assert!(!pruned_again);
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn test_agent_tool_error_handling_and_recovery() {
        let inference = Arc::new(HttpInferenceBackend::new(
            Some("http://127.0.0.1:9999/v1/chat/completions".to_string()),
            None,
        ));
        let skills = Arc::new(SkillRegistry::new());
        let agent = AgentEngine::new(inference, skills.clone(), None);

        // Test executing unknown tool directly via skills registry
        let unknown_req = SkillExecutionRequest {
            skill_name: "nonexistent_custom_skill".to_string(),
            arguments: json!({}),
        };
        let res = skills.execute(&unknown_req);
        assert!(!res.success);
        assert!(res.error.unwrap().contains("not found in registry"));

        // Test tool parameter validation (e.g. read_file with empty path)
        let empty_path_req = SkillExecutionRequest {
            skill_name: "read_file".to_string(),
            arguments: json!({"path": ""}),
        };
        let empty_res = skills.execute(&empty_path_req);
        assert!(!empty_res.success);
        assert_eq!(empty_res.error.unwrap(), "Path cannot be empty");

        // Test autonomous agent task execution fail-closed offline error handling
        let exec_res = agent
            .execute_task("Run status check on local machine", None, 1)
            .await;
        assert!(exec_res.is_err());
        let err = exec_res.unwrap_err();
        assert!(err.to_string().contains("Inference endpoint unreachable"));
    }

    #[tokio::test]
    async fn test_agent_multiturn_conversational_history() {
        let app = axum::Router::new().route(
            "/v1/chat/completions",
            axum::routing::post(|axum::Json(_body): axum::Json<serde_json::Value>| async {
                axum::Json(serde_json::json!({
                    "id": "chatcmpl-mock",
                    "object": "chat.completion",
                    "created": 1234567890,
                    "model": "atlas-lightning-omni",
                    "choices": [{
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": "Sovereign task completed."
                        },
                        "finish_reason": "stop"
                    }]
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let inference = Arc::new(HttpInferenceBackend::new(
            Some(format!("http://127.0.0.1:{}/v1/chat/completions", port)),
            None,
        ));
        let skills = Arc::new(SkillRegistry::new());
        let agent = AgentEngine::new(inference, skills.clone(), None);

        let custom_system = "You are AEGIS sovereign unit test runner.";
        let result = agent
            .execute_task("Perform sequential tasks", Some(custom_system), 2)
            .await
            .unwrap();

        let _duration = result.duration_ms;
        assert!(result.turns_taken >= 1);
        assert_eq!(result.steps.len(), 1);
        assert!(result.completed);
        assert_eq!(result.final_response, "Sovereign task completed.");
    }
}
