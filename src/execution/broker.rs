use crate::defense::DoctrineDecision;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub session_id: String,
    pub run_id: String,
    pub capability: String,
    pub intent: String,
    pub target: String,
    pub params: serde_json::Value,
}

#[async_trait]
pub trait ExecutionAuthority: Send + Sync {
    async fn authorize(&self, request: &ActionRequest) -> Result<DoctrineDecision, String>;
}
