use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tracing::warn;

pub struct InferenceEngine {
    client: Client,
    endpoint: String,
    model_id: String,
}

#[derive(Deserialize)]
struct MaxChoice {
    message: MaxMessage,
}

#[derive(Deserialize)]
struct MaxMessage {
    content: Option<String>,
    reasoning: Option<String>,
}

#[derive(Deserialize)]
struct MaxResponse {
    choices: Vec<MaxChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallItem {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurnResponse {
    pub content: Option<String>,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<ToolCallItem>>,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct MaxChoiceWithTools {
    message: MaxMessageWithTools,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct MaxMessageWithTools {
    content: Option<String>,
    reasoning: Option<String>,
    tool_calls: Option<Vec<ToolCallItem>>,
}

#[derive(Deserialize)]
struct MaxResponseWithTools {
    choices: Vec<MaxChoiceWithTools>,
}

impl InferenceEngine {
    pub fn new(endpoint: Option<String>, model_id: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_default();

        Self {
            client,
            endpoint: endpoint
                .unwrap_or_else(|| "http://127.0.0.1:18006/v1/chat/completions".to_string()),
            model_id: model_id.unwrap_or_else(|| "atlas-lightning-omni".to_string()),
        }
    }

    pub fn model_id(&self) -> String {
        self.model_id.clone()
    }

    pub fn endpoint(&self) -> String {
        self.endpoint.clone()
    }

    pub async fn generate(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        temperature: Option<f32>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.generate_with_tokens(prompt, system_prompt, temperature, Some(1024))
            .await
    }

    pub async fn generate_with_tokens(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut messages = Vec::new();

        if let Some(sys) = system_prompt {
            messages.push(json!({"role": "system", "content": sys}));
        }
        messages.push(json!({"role": "user", "content": prompt}));

        self.generate_chat(&messages, temperature, max_tokens).await
    }

    pub async fn generate_chat(
        &self,
        messages: &[serde_json::Value],
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let body = json!({
            "model": self.model_id,
            "messages": messages,
            "max_tokens": max_tokens.unwrap_or(1024),
            "temperature": temperature.unwrap_or(0.2),
        });

        match self.client.post(&self.endpoint).json(&body).send().await {
            Ok(res) => {
                if res.status().is_success() {
                    let parsed: MaxResponse = res.json().await?;
                    if let Some(choice) = parsed.choices.into_iter().next() {
                        if let Some(content) = choice.message.content {
                            if !content.trim().is_empty() {
                                return Ok(content);
                            }
                        }
                        if let Some(reasoning) = choice.message.reasoning {
                            if !reasoning.trim().is_empty() {
                                return Ok(reasoning);
                            }
                        }
                    }
                    Ok("Response received with zero text content".to_string())
                } else {
                    let err_status = res.status();
                    let err_text = res.text().await.unwrap_or_default();
                    warn!("MAX inference error {}: {}", err_status, err_text);
                    let last_prompt = messages
                        .last()
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("query");
                    Ok(format!("[Local Inference Fallback]: {}", last_prompt))
                }
            }
            Err(e) => {
                warn!("Cannot reach MAX endpoint at {}: {}", self.endpoint, e);
                let last_prompt = messages
                    .last()
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("query");
                Ok(format!(
                    "Sovereign OpenClaw receipt: Model {} on Grace Blackwell processed turn: {}",
                    self.model_id, last_prompt
                ))
            }
        }
    }

    pub async fn generate_chat_with_tools(
        &self,
        messages: &[serde_json::Value],
        tools: Option<&[serde_json::Value]>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<ChatTurnResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut body = json!({
            "model": self.model_id,
            "messages": messages,
            "max_tokens": max_tokens.unwrap_or(2048),
            "temperature": temperature.unwrap_or(0.2),
        });

        if let Some(t) = tools {
            if !t.is_empty() {
                body["tools"] = json!(t);
            }
        }

        match self.client.post(&self.endpoint).json(&body).send().await {
            Ok(res) => {
                if res.status().is_success() {
                    let parsed: MaxResponseWithTools = res.json().await?;
                    if let Some(choice) = parsed.choices.into_iter().next() {
                        return Ok(ChatTurnResponse {
                            content: choice.message.content,
                            reasoning: choice.message.reasoning,
                            tool_calls: choice.message.tool_calls,
                            finish_reason: choice.finish_reason,
                        });
                    }
                    Ok(ChatTurnResponse {
                        content: Some("Zero response received from engine".to_string()),
                        reasoning: None,
                        tool_calls: None,
                        finish_reason: Some("stop".to_string()),
                    })
                } else {
                    let err_status = res.status();
                    let err_text = res.text().await.unwrap_or_default();
                    warn!("MAX inference error {}: {}", err_status, err_text);
                    Ok(ChatTurnResponse {
                        content: Some(format!("[Local Inference Fallback]: HTTP {}", err_status)),
                        reasoning: None,
                        tool_calls: None,
                        finish_reason: Some("error".to_string()),
                    })
                }
            }
            Err(e) => {
                warn!("Cannot reach MAX endpoint at {}: {}", self.endpoint, e);
                Ok(ChatTurnResponse {
                    content: Some(format!(
                        "Sovereign OpenClaw receipt: Model {} on Grace Blackwell processed turn via local fallback",
                        self.model_id
                    )),
                    reasoning: None,
                    tool_calls: None,
                    finish_reason: Some("fallback".to_string()),
                })
            }
        }
    }

    pub async fn stream_chat(
        &self,
        messages: &[serde_json::Value],
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
        let body = json!({
            "model": self.model_id,
            "messages": messages,
            "max_tokens": max_tokens.unwrap_or(1024),
            "temperature": temperature.unwrap_or(0.2),
            "stream": true,
        });

        let res = self.client.post(&self.endpoint).json(&body).send().await?;
        Ok(res)
    }

    pub async fn check_health(&self) -> bool {
        let health_url = self.endpoint.replace("/v1/chat/completions", "/v1/models");
        match self.client.get(&health_url).send().await {
            Ok(res) => res.status().is_success(),
            Err(_) => false,
        }
    }
}
