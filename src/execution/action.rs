use crate::execution::receipt::ActionReceipt;
use crate::sessions::id::{ActionId, RunId, SessionId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Pending,
    Approved,
    Denied,
    Executing,
    Completed,
    Failed,
}

impl ActionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
            Self::Executing => "executing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "denied" => Some(Self::Denied),
            "executing" => Some(Self::Executing),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: ActionId,
    pub run_id: RunId,
    pub session_id: SessionId,
    pub capability: String,
    pub intent: String,
    pub params: serde_json::Value,
    pub status: ActionStatus,
    pub receipt: Option<ActionReceipt>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

impl Action {
    pub fn new(
        run_id: RunId,
        session_id: SessionId,
        capability: impl Into<String>,
        intent: impl Into<String>,
        params: serde_json::Value,
    ) -> Self {
        Self {
            id: ActionId::new(),
            run_id,
            session_id,
            capability: capability.into(),
            intent: intent.into(),
            params,
            status: ActionStatus::Pending,
            receipt: None,
            created_at: chrono::Utc::now().timestamp(),
            completed_at: None,
        }
    }
}
