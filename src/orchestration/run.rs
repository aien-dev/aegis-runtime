use super::budget::RunBudget;
use super::termination::TerminationReason;
use super::trigger::Trigger;
use crate::sessions::id::{RunId, SessionId};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Running,
    WaitingForApproval,
    WaitingForTool,
    Completed,
    Failed,
    Cancelled,
}

impl RunState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::WaitingForApproval => "waiting_for_approval",
            Self::WaitingForTool => "waiting_for_tool",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "running" => Some(Self::Running),
            "waiting_for_approval" => Some(Self::WaitingForApproval),
            "waiting_for_tool" => Some(Self::WaitingForTool),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub session_id: SessionId,
    pub state: RunState,
    pub trigger: Trigger,
    pub steps_used: u32,
    pub inference_calls_used: u32,
    pub actions_used: u32,
    pub budget: RunBudget,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub termination: Option<TerminationReason>,
}

impl Run {
    pub fn new(session_id: SessionId, trigger: Trigger, budget: RunBudget) -> Self {
        Self {
            id: RunId::new(),
            session_id,
            state: RunState::Running,
            trigger,
            steps_used: 0,
            inference_calls_used: 0,
            actions_used: 0,
            budget,
            started_at: Utc::now().timestamp(),
            completed_at: None,
            termination: None,
        }
    }
}
