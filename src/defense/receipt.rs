use super::decision::DoctrineDecision;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefenseReceipt {
    pub action_id: String,
    pub actor_id: String,
    pub capability: String,
    pub target: String,
    pub decision: DoctrineDecision,
    pub doctrine_version: String,
    pub evaluated_at: i64,
}
