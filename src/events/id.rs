use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(String);

impl EventId {
    pub fn new() -> Self {
        Self(format!("evt_{}", Uuid::new_v4().simple()))
    }

    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<EventId> for aien_protocol_types::EventId {
    fn from(e: EventId) -> Self {
        let raw = e.as_str().strip_prefix("evt_").unwrap_or(e.as_str());
        if let Ok(u) = uuid::Uuid::parse_str(raw) {
            aien_protocol_types::EventId(u)
        } else {
            aien_protocol_types::EventId(uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                e.as_str().as_bytes(),
            ))
        }
    }
}

impl From<aien_protocol_types::EventId> for EventId {
    fn from(id: aien_protocol_types::EventId) -> Self {
        Self::from_string(format!("evt_{}", id.0.simple()))
    }
}
