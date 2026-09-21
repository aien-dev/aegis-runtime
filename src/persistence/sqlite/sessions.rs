use crate::persistence::async_db::AsyncDatabase;
use crate::persistence::error::PersistenceError;
use crate::sessions::id::SessionId;
use crate::sessions::session::{Session, SessionBudget, SessionStatus};
use crate::sessions::store::SessionStore;
use async_trait::async_trait;
use chrono::Utc;
use rusqlite::params;

#[derive(Clone)]
pub struct SqliteSessionStore {
    db: AsyncDatabase,
}

impl SqliteSessionStore {
    pub fn new(db: AsyncDatabase) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn create(&self, session: &Session) -> Result<(), PersistenceError> {
        let session = session.clone();
        self.db
            .interact(move |conn| {
                let budget_str = serde_json::to_string(&session.budget)?;
                conn.execute(
                    "INSERT INTO sessions (id, parent_session_id, status, objective, agent_profile, model_profile, policy_profile, budget_json, created_at, updated_at, version)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        session.id.as_str(),
                        session.parent_session_id.as_ref().map(|p| p.as_str()),
                        session.status.as_str(),
                        session.objective,
                        session.agent_profile,
                        session.model_profile,
                        session.policy_profile,
                        budget_str,
                        session.created_at,
                        session.updated_at,
                        session.version as i64,
                    ],
                )?;
                Ok(())
            })
            .await
    }

    async fn get(&self, id: &SessionId) -> Result<Option<Session>, PersistenceError> {
        let id_str = id.to_string();
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, parent_session_id, status, objective, agent_profile, model_profile, policy_profile, budget_json, created_at, updated_at, version
                     FROM sessions WHERE id = ?1",
                )?;
                let mut rows = stmt.query(params![id_str])?;
                if let Some(row) = rows.next()? {
                    let id_val: String = row.get(0)?;
                    let parent_val: Option<String> = row.get(1)?;
                    let status_str: String = row.get(2)?;
                    let objective: Option<String> = row.get(3)?;
                    let agent_profile: String = row.get(4)?;
                    let model_profile: String = row.get(5)?;
                    let policy_profile: String = row.get(6)?;
                    let budget_json: String = row.get(7)?;
                    let created_at: i64 = row.get(8)?;
                    let updated_at: i64 = row.get(9)?;
                    let version: i64 = row.get(10)?;

                    let status = SessionStatus::from_str(&status_str)
                        .ok_or_else(|| PersistenceError::Serialization(format!("Invalid status: {status_str}")))?;
                    let budget: SessionBudget = serde_json::from_str(&budget_json)?;

                    let mut msg_stmt = conn.prepare(
                        "SELECT data_json FROM messages WHERE session_id = ?1 ORDER BY created_at ASC, rowid ASC",
                    )?;
                    let msg_rows = msg_stmt.query_map(params![id_val], |r| {
                        let data: String = r.get(0)?;
                        Ok(data)
                    })?;
                    let mut messages = Vec::new();
                    for m in msg_rows {
                        let data = m?;
                        let msg: crate::sessions::message::Message = serde_json::from_str(&data)?;
                        messages.push(msg);
                    }

                    Ok(Some(Session {
                        id: SessionId::from_string(id_val),
                        parent_session_id: parent_val.map(SessionId::from_string),
                        status,
                        objective,
                        agent_profile,
                        model_profile,
                        policy_profile,
                        messages,
                        budget,
                        created_at,
                        updated_at,
                        version: version as u64,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
    }

    async fn update(&self, session: &Session) -> Result<(), PersistenceError> {
        let session = session.clone();
        self.db
            .interact(move |conn| {
                let budget_str = serde_json::to_string(&session.budget)?;
                let now = Utc::now().timestamp();

                let updated_rows = conn.execute(
                    "UPDATE sessions SET parent_session_id = ?1, status = ?2, objective = ?3,
                     agent_profile = ?4, model_profile = ?5, policy_profile = ?6,
                     budget_json = ?7, updated_at = ?8, version = version + 1
                     WHERE id = ?9 AND version = ?10",
                    params![
                        session.parent_session_id.as_ref().map(|p| p.as_str()),
                        session.status.as_str(),
                        session.objective,
                        session.agent_profile,
                        session.model_profile,
                        session.policy_profile,
                        budget_str,
                        now,
                        session.id.as_str(),
                        session.version as i64,
                    ],
                )?;

                if updated_rows == 0 {
                    return Err(PersistenceError::ConcurrencyConflict {
                        id: session.id.to_string(),
                        expected_version: session.version,
                    });
                }
                Ok(())
            })
            .await
    }

    async fn list(&self, limit: usize, offset: usize) -> Result<Vec<Session>, PersistenceError> {
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, parent_session_id, status, objective, agent_profile, model_profile, policy_profile, budget_json, created_at, updated_at, version
                     FROM sessions ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
                )?;
                let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
                    let id_val: String = row.get(0)?;
                    let parent_val: Option<String> = row.get(1)?;
                    let status_str: String = row.get(2)?;
                    let objective: Option<String> = row.get(3)?;
                    let agent_profile: String = row.get(4)?;
                    let model_profile: String = row.get(5)?;
                    let policy_profile: String = row.get(6)?;
                    let budget_json: String = row.get(7)?;
                    let created_at: i64 = row.get(8)?;
                    let updated_at: i64 = row.get(9)?;
                    let version: i64 = row.get(10)?;
                    Ok((id_val, parent_val, status_str, objective, agent_profile, model_profile, policy_profile, budget_json, created_at, updated_at, version))
                })?;

                let mut sessions = Vec::new();
                for r in rows {
                    let (id_val, parent_val, status_str, objective, agent_profile, model_profile, policy_profile, budget_json, created_at, updated_at, version) = r?;
                    let status = SessionStatus::from_str(&status_str)
                        .ok_or_else(|| PersistenceError::Serialization(format!("Invalid status: {status_str}")))?;
                    let budget: SessionBudget = serde_json::from_str(&budget_json)?;

                    sessions.push(Session {
                        id: SessionId::from_string(id_val),
                        parent_session_id: parent_val.map(SessionId::from_string),
                        status,
                        objective,
                        agent_profile,
                        model_profile,
                        policy_profile,
                        messages: Vec::new(),
                        budget,
                        created_at,
                        updated_at,
                        version: version as u64,
                    });
                }
                Ok(sessions)
            })
            .await
    }
}
