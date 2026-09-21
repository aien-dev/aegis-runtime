use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AegisEvent {
    SessionCreated {
        session_id: String,
        objective: Option<String>,
    },
    SessionBranched {
        session_id: String,
        parent_session_id: String,
    },
    SessionClosed {
        session_id: String,
    },

    RunStarted {
        run_id: String,
        session_id: String,
        trigger: String,
    },
    RunCompleted {
        run_id: String,
        steps: u32,
        duration_ms: u64,
    },
    RunFailed {
        run_id: String,
        error: String,
    },

    AssistantDelta {
        text: String,
    },

    ActionRequested {
        action_id: String,
        capability: String,
    },
    ActionAuthorized {
        action_id: String,
        decision: String,
    },
    ActionDenied {
        action_id: String,
        reason: String,
    },
    ActionCompleted {
        action_id: String,
        success: bool,
        duration_ms: u64,
    },

    ApprovalRequired {
        approval_id: String,
        action_id: String,
        capability: String,
    },
    ApprovalResolved {
        approval_id: String,
        approved: bool,
    },

    ContainmentActivated {
        level: String,
        reason: String,
    },
}
