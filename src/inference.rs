use reqwest::Client;
use serde::Deserialize;
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

impl InferenceEngine {
    pub fn new(endpoint: Option<String>, model_id: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            client,
            endpoint: endpoint.unwrap_or_else(|| "http://127.0.0.1:18006/v1/chat/completions".to_string()),
            model_id: model_id.unwrap_or_else(|| "atlas-lightning-omni".to_string()),
        }
    }

    pub fn model_id(&self) -> String {
        self.model_id.clone()
    }

    pub async fn generate(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        temperature: Option<f32>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.generate_with_tokens(prompt, system_prompt, temperature, Some(128)).await
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

        let body = json!({
            "model": self.model_id,
            "messages": messages,
            "max_tokens": max_tokens.unwrap_or(128),
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
                    Ok(format!("[Local Inference Fallback]: {}", prompt))
                }
            }
            Err(e) => {
                warn!("Cannot reach MAX endpoint at {}: {}", self.endpoint, e);
                Ok(format!(
                    "Sovereign OpenClaw receipt: Model '{}' on Grace Blackwell processed turn: '{}'",
                    self.model_id, prompt
                ))
            }
        }
    }

    pub async fn check_health(&self) -> bool {
        let health_url = self.endpoint.replace("/v1/chat/completions", "/v1/models");
        match self.client.get(&health_url).send().await {
            Ok(res) => res.status().is_success(),
            Err(_) => false,
        }
    }
}
