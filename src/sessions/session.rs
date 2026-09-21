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
