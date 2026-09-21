use crate::persistence::async_db::AsyncDatabase;
use crate::persistence::error::PersistenceError;
use crate::sessions::id::{MessageId, SessionId};
use crate::sessions::message::Message;
use crate::sessions::store::MessageStore;
use async_trait::async_trait;
use rusqlite::params;

#[derive(Clone)]
pub struct SqliteMessageStore {
    db: AsyncDatabase,
}

impl SqliteMessageStore {
    pub fn new(db: AsyncDatabase) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MessageStore for SqliteMessageStore {
    async fn append(
        &self,
        session_id: &SessionId,
        message: &Message,
    ) -> Result<(), PersistenceError> {
        let session_id = session_id.clone();
        let message = message.clone();
        self.db
            .interact(move |conn| {
                let role_str = match message.role() {
                    crate::sessions::message::Role::System => "system",
                    crate::sessions::message::Role::User => "user",
                    crate::sessions::message::Role::Assistant => "assistant",
                    crate::sessions::message::Role::Tool => "tool",
                };
                let data_json = serde_json::to_string(&message)?;
                let created_at = match &message {
                    Message::System(m) => m.created_at,
                    Message::User(m) => m.created_at,
                    Message::Assistant(m) => m.created_at,
                    Message::Tool(m) => m.created_at,
                };
                conn.execute(
                    "INSERT INTO messages (id, session_id, role, data_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        message.id().as_str(),
                        session_id.as_str(),
                        role_str,
                        data_json,
                        created_at,
                    ],
                )?;
                Ok(())
            })
            .await
    }

    async fn list(&self, session_id: &SessionId) -> Result<Vec<Message>, PersistenceError> {
        let session_id = session_id.clone();
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT data_json FROM messages WHERE session_id = ?1 ORDER BY created_at ASC, rowid ASC",
                )?;
                let rows = stmt.query_map(params![session_id.as_str()], |row| {
                    let data: String = row.get(0)?;
                    Ok(data)
                })?;
                let mut messages = Vec::new();
                for r in rows {
                    let data = r?;
                    let msg: Message = serde_json::from_str(&data)?;
                    messages.push(msg);
                }
                Ok(messages)
            })
            .await
    }

    async fn get(&self, message_id: &MessageId) -> Result<Option<Message>, PersistenceError> {
        let id_str = message_id.to_string();
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare("SELECT data_json FROM messages WHERE id = ?1")?;
                let mut rows = stmt.query(params![id_str])?;
                if let Some(row) = rows.next()? {
                    let data: String = row.get(0)?;
                    let msg: Message = serde_json::from_str(&data)?;
                    Ok(Some(msg))
                } else {
                    Ok(None)
                }
            })
            .await
    }
}
