use crate::sessions::id::{ActionId, MessageId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Trigger {
    UserMessage(MessageId),
    Heartbeat,
    ScheduledTask(String),
    ToolResult(ActionId),
    ExternalEvent(String),
    Resume,
}
