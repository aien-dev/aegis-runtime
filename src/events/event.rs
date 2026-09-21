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

impl AegisEvent {
    pub fn to_canonical_agent_event(&self) -> Option<aien_agent_state_abi::AgentEvent> {
        use crate::sessions::id::{ActionId, ApprovalId, RunId, SessionId};
        use aien_agent_state_abi::AgentEvent;

        match self {
            AegisEvent::SessionCreated { .. } => Some(AgentEvent::SessionCreated),
            AegisEvent::SessionClosed { .. } => Some(AgentEvent::SessionClosed),
            AegisEvent::SessionBranched { session_id, .. } => {
                let sid: aien_protocol_types::SessionId = SessionId::from_string(session_id).into();
                Some(AgentEvent::ContextBranched(sid.0))
            }
            AegisEvent::RunStarted { run_id, .. } => {
                let rid: aien_protocol_types::RunId = RunId::from_string(run_id).into();
                Some(AgentEvent::RunStarted(rid))
            }
            AegisEvent::RunCompleted { run_id, .. } => {
                Some(AgentEvent::RunCompleted(run_id.clone()))
            }
            AegisEvent::ActionRequested { action_id, .. } => {
                let aid: aien_protocol_types::ActionId = ActionId::from_string(action_id).into();
                Some(AgentEvent::ActionRequested(aid))
            }
            AegisEvent::ActionAuthorized { action_id, .. } => {
                let aid: aien_protocol_types::ActionId = ActionId::from_string(action_id).into();
                Some(AgentEvent::ActionAuthorized(aid))
            }
            AegisEvent::ActionDenied { action_id, .. } => {
                let aid: aien_protocol_types::ActionId = ActionId::from_string(action_id).into();
                Some(AgentEvent::ActionDenied(aid))
            }
            AegisEvent::ActionCompleted { action_id, .. } => {
                let aid: aien_protocol_types::ActionId = ActionId::from_string(action_id).into();
                Some(AgentEvent::ActionCompleted(aid))
            }
            AegisEvent::ApprovalRequired { approval_id, .. } => {
                let apid: aien_protocol_types::ApprovalId =
                    ApprovalId::from_string(approval_id).into();
                Some(AgentEvent::ApprovalRequested(apid))
            }
            AegisEvent::ApprovalResolved { approval_id, .. } => {
                let apid: aien_protocol_types::ApprovalId =
                    ApprovalId::from_string(approval_id).into();
                Some(AgentEvent::ApprovalResolved(apid))
            }
            _ => None,
        }
    }
}
