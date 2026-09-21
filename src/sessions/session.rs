use super::id::SessionId;
use super::message::Message;
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Suspended,
    Closed,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Closed => "closed",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "suspended" => Some(Self::Suspended),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionBudget {
    pub max_total_tokens: u64,
    pub max_wall_time_secs: u64,
}

impl Default for SessionBudget {
    fn default() -> Self {
        Self {
            max_total_tokens: 128_000,
            max_wall_time_secs: 3600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub parent_session_id: Option<SessionId>,
    pub status: SessionStatus,
    pub objective: Option<String>,
    pub agent_profile: String,
    pub model_profile: String,
    pub policy_profile: String,
    pub messages: Vec<Message>,
    pub budget: SessionBudget,
    pub created_at: i64,
    pub updated_at: i64,
    pub version: u64,
}

impl Session {
    pub fn new(objective: Option<String>) -> Self {
        let now = Utc::now().timestamp();
        Self {
            id: SessionId::new(),
            parent_session_id: None,
            status: SessionStatus::Active,
            objective,
            agent_profile: "default".to_string(),
            model_profile: "default".to_string(),
            policy_profile: "default".to_string(),
            messages: Vec::new(),
            budget: SessionBudget::default(),
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }

    pub fn branch(&self) -> Self {
        let now = Utc::now().timestamp();
        Self {
            id: SessionId::new(),
            parent_session_id: Some(self.id.clone()),
            status: SessionStatus::Active,
            objective: self.objective.clone(),
            agent_profile: self.agent_profile.clone(),
            model_profile: self.model_profile.clone(),
            policy_profile: self.policy_profile.clone(),
            messages: self.messages.clone(),
            budget: self.budget.clone(),
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }
}

impl Session {
    pub fn to_agent_state(
        &self,
        agent_id: &aien_protocol_types::AgentId,
        model_profile: &str,
    ) -> aien_agent_state_abi::AgentState {
        use aien_agent_state_abi::*;
        use aien_protocol_types::*;

        let closed_at = if self.status == SessionStatus::Closed {
            Some(Timestamp::from_micros(self.updated_at as u64 * 1_000_000))
        } else {
            None
        };

        AgentState {
            abi_version: ProtocolVersion::new(1, 0),
            agent: AgentIdentity {
                agent_id: *agent_id,
                name: self.agent_profile.clone(),
                model_profile: model_profile.to_string(),
                created_at: Timestamp::from_micros(self.created_at as u64 * 1_000_000),
            },
            session: SessionState {
                session_id: self.id.clone().into(),
                title: self
                    .objective
                    .clone()
                    .unwrap_or_else(|| "AEGIS Session".to_string()),
                created_at: Timestamp::from_micros(self.created_at as u64 * 1_000_000),
                closed_at,
            },
            objective: self.objective.as_ref().map(|obj| Objective {
                objective_id: uuid::Uuid::new_v4(),
                description: obj.clone(),
                success_criteria: Vec::new(),
            }),
            context: ContextState {
                conversation: Vec::new(),
                pinned: Vec::new(),
                summaries: Vec::new(),
                cortex: Vec::new(),
                inference: None,
            },
            authority: AuthorityState {
                principal: uuid::Uuid::new_v4(),
                policy_profile: self.policy_profile.clone(),
                capabilities: Vec::new(),
                restrictions: Vec::new(),
                containment: aien_action_protocol::ContainmentState::default(),
            },
            budget: ResourceBudget {
                max_tokens: Some(self.budget.max_total_tokens),
                max_cost_micro_usd: None,
                max_duration_seconds: Some(self.budget.max_wall_time_secs),
                max_subagents: None,
            },
            execution: ExecutionState {
                active_run: None,
                status: match self.status {
                    SessionStatus::Active => AgentExecutionStatus::Idle,
                    SessionStatus::Suspended => AgentExecutionStatus::Suspended,
                    SessionStatus::Closed => AgentExecutionStatus::Terminated,
                },
                pending_actions: Vec::new(),
                pending_approvals: Vec::new(),
                last_event: None,
            },
            parent: self.parent_session_id.as_ref().map(|p| StateRef {
                state_id: aien_protocol_types::SessionId::from(p.clone()).0,
                digest: Digest32::ZERO,
            }),
            sequence: SequenceNumber(self.version),
        }
    }
}
