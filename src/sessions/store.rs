use super::id::{ActionId, ApprovalId, MessageId, RunId, SessionId};
use super::message::Message;
use super::session::Session;
use crate::defense::Approval;
use crate::execution::Action;
use crate::orchestration::Run;
use crate::persistence::PersistenceError;
use async_trait::async_trait;

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, session: &Session) -> Result<(), PersistenceError>;
    async fn get(&self, id: &SessionId) -> Result<Option<Session>, PersistenceError>;
    async fn update(&self, session: &Session) -> Result<(), PersistenceError>;
    async fn list(&self, limit: usize, offset: usize) -> Result<Vec<Session>, PersistenceError>;
}

#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn append(
        &self,
        session_id: &SessionId,
        message: &Message,
    ) -> Result<(), PersistenceError>;
    async fn list(&self, session_id: &SessionId) -> Result<Vec<Message>, PersistenceError>;
    async fn get(&self, message_id: &MessageId) -> Result<Option<Message>, PersistenceError>;
}

#[async_trait]
pub trait RunStore: Send + Sync {
    async fn create(&self, run: &Run) -> Result<(), PersistenceError>;
    async fn get(&self, id: &RunId) -> Result<Option<Run>, PersistenceError>;
    async fn update(&self, run: &Run) -> Result<(), PersistenceError>;
    async fn list_by_session(&self, session_id: &SessionId) -> Result<Vec<Run>, PersistenceError>;
    async fn get_active_run(&self, session_id: &SessionId)
        -> Result<Option<Run>, PersistenceError>;
}

#[async_trait]
pub trait ActionStore: Send + Sync {
    async fn create(&self, action: &Action) -> Result<(), PersistenceError>;
    async fn get(&self, id: &ActionId) -> Result<Option<Action>, PersistenceError>;
    async fn update(&self, action: &Action) -> Result<(), PersistenceError>;
    async fn list_by_run(&self, run_id: &RunId) -> Result<Vec<Action>, PersistenceError>;
}

#[async_trait]
pub trait ApprovalStore: Send + Sync {
    async fn create(&self, approval: &Approval) -> Result<(), PersistenceError>;
    async fn get(&self, id: &ApprovalId) -> Result<Option<Approval>, PersistenceError>;
    async fn update(&self, approval: &Approval) -> Result<(), PersistenceError>;
    async fn list_pending(&self) -> Result<Vec<Approval>, PersistenceError>;
}
