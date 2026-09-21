use crate::sessions::id::{ActionId, ApprovalId, RunId, SessionId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

impl ApprovalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub id: ApprovalId,
    pub action_id: ActionId,
    pub run_id: RunId,
    pub session_id: SessionId,
    pub prompt: String,
    pub status: ApprovalStatus,
    pub requested_at: i64,
    pub decided_at: Option<i64>,
    pub decided_by: Option<String>,
}

impl Approval {
    pub fn new(
        action_id: ActionId,
        run_id: RunId,
        session_id: SessionId,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            id: ApprovalId::new(),
            action_id,
            run_id,
            session_id,
            prompt: prompt.into(),
            status: ApprovalStatus::Pending,
            requested_at: chrono::Utc::now().timestamp(),
            decided_at: None,
            decided_by: None,
        }
    }
}
