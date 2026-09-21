use super::event::AegisEvent;
use super::id::EventId;
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: EventId,
    pub session_id: Option<String>,
    pub run_id: Option<String>,
    pub sequence: u64,
    pub timestamp: i64,
    pub event: AegisEvent,
}

impl EventEnvelope {
    pub fn new(
        event: AegisEvent,
        session_id: Option<String>,
        run_id: Option<String>,
        sequence: u64,
    ) -> Self {
        Self {
            event_id: EventId::new(),
            session_id,
            run_id,
            sequence,
            timestamp: Utc::now().timestamp_millis(),
            event,
        }
    }
}
