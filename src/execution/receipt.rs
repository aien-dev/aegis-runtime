use crate::defense::DoctrineDecision;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionReceipt {
    pub action_id: String,
    pub session_id: String,
    pub run_id: String,
    pub capability: String,
    pub target: String,
    pub decision: DoctrineDecision,
    pub output: String,
    pub success: bool,
    pub duration_ms: u64,
}
