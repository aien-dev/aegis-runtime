use super::envelope::EventEnvelope;
use async_trait::async_trait;
use tokio::sync::broadcast;

pub const DEFAULT_BUS_CAPACITY: usize = 1024;

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<EventEnvelope>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(DEFAULT_BUS_CAPACITY)
    }
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, envelope: EventEnvelope) -> Result<usize, String> {
        self.sender
            .send(envelope)
            .map_err(|e| format!("Failed to publish event: {}", e))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.sender.subscribe()
    }
}

#[async_trait]
pub trait EventSubscriber: Send + Sync {
    async fn on_event(&self, envelope: &EventEnvelope) -> Result<(), String>;
}
