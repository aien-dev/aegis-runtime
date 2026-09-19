use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

use crate::inference::InferenceEngine;
use crate::persistence::Database;
use crate::skills::{SkillExecutionRequest, SkillRegistry};

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
    inference: Arc<InferenceEngine>,
    skills: Arc<SkillRegistry>,
    db: Option<Arc<Database>>,
}

impl AgentEngine {
    pub fn new(
        inference: Arc<InferenceEngine>,
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

    pub async fn execute_task(
        &self,
        goal: &str,
        system_override: Option<&str>,
        max_turns: usize,
    ) -> Result<AgentExecutionResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = Instant::now();
        let tools = self.skills.to_openai_tools();

        let default_system = "You are AIEN, a sovereign native AI systems agent running on NVIDIA DGX Spark (Grace Blackwell GB10) as user drakestapleton. Active workspaces reside strictly in /home/drakestapleton/workspace/ (openclaw-rs, aien-harness-publish, etc.) and /home/drakestapleton/atlas-prime-workspace/. Default working directory is /home/drakestapleton/workspace. You have native tools available to execute commands (bash_eval), inspect files (read_file, list_dir, git_status), modify files (write_file), and query memory (cortex_recall). Never run broad root filesystem scans or find /. Invoke tools directly on specific workspace targets. Adhere strictly to the unslop standard: zero em dashes and zero en dashes, no transitional fluff, and direct technical proof. When finished, provide a concise final summary.";

        let sys_prompt = system_override.unwrap_or(default_system);

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
                        res.error.clone().unwrap_or_else(|| "Unknown error".to_string())
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
            let _ = db.record_turn(
                "agent_autonomous",
                goal,
                &final_response,
                duration as u64,
            );
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

    #[tokio::test]
    async fn test_agent_engine_creation_and_skills() {
        let inference = Arc::new(InferenceEngine::new(None, None));
        let skills = Arc::new(SkillRegistry::new());
        let agent = AgentEngine::new(inference, skills.clone(), None);

        let registered = agent.skills().list_skills();
        assert!(!registered.is_empty());
    }
}
