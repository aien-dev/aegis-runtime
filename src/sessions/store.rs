use super::id::{MessageId, SessionId};
use super::message::Message;
use super::session::Session;
use async_trait::async_trait;

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, session: &Session) -> Result<(), String>;
    async fn get(&self, id: &SessionId) -> Result<Option<Session>, String>;
    async fn update(&self, session: &Session) -> Result<(), String>;
}

#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn append(&self, session_id: &SessionId, message: &Message) -> Result<(), String>;
    async fn list(&self, session_id: &SessionId) -> Result<Vec<Message>, String>;
    async fn get(&self, message_id: &MessageId) -> Result<Option<Message>, String>;
}
