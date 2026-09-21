use crate::execution::action::{Action, ActionStatus};
use crate::execution::receipt::ActionReceipt;
use crate::persistence::async_db::AsyncDatabase;
use crate::persistence::error::PersistenceError;
use crate::sessions::id::{ActionId, RunId, SessionId};
use crate::sessions::store::ActionStore;
use async_trait::async_trait;
use rusqlite::params;

#[derive(Clone)]
pub struct SqliteActionStore {
    db: AsyncDatabase,
}

impl SqliteActionStore {
    pub fn new(db: AsyncDatabase) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ActionStore for SqliteActionStore {
    async fn create(&self, action: &Action) -> Result<(), PersistenceError> {
        let action = action.clone();
        self.db
            .interact(move |conn| {
                let params_str = serde_json::to_string(&action.params)?;
                let receipt_str = action
                    .receipt
                    .as_ref()
                    .map(|r| serde_json::to_string(r))
                    .transpose()?;

                conn.execute(
                    "INSERT INTO actions (id, run_id, session_id, capability, intent, params_json, status, receipt_json, created_at, completed_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        action.id.as_str(),
                        action.run_id.as_str(),
                        action.session_id.as_str(),
                        action.capability,
                        action.intent,
                        params_str,
                        action.status.as_str(),
                        receipt_str,
                        action.created_at,
                        action.completed_at,
                    ],
                )?;
                Ok(())
            })
            .await
    }

    async fn get(&self, id: &ActionId) -> Result<Option<Action>, PersistenceError> {
        let id_str = id.to_string();
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, run_id, session_id, capability, intent, params_json, status, receipt_json, created_at, completed_at
                     FROM actions WHERE id = ?1",
                )?;
                let mut rows = stmt.query(params![id_str])?;
                if let Some(row) = rows.next()? {
                    let id_val: String = row.get(0)?;
                    let run_val: String = row.get(1)?;
                    let session_val: String = row.get(2)?;
                    let capability: String = row.get(3)?;
                    let intent: String = row.get(4)?;
                    let params_json: String = row.get(5)?;
                    let status_str: String = row.get(6)?;
                    let receipt_json: Option<String> = row.get(7)?;
                    let created_at: i64 = row.get(8)?;
                    let completed_at: Option<i64> = row.get(9)?;

                    let params: serde_json::Value = serde_json::from_str(&params_json)?;
                    let status = ActionStatus::from_str(&status_str)
                        .ok_or_else(|| PersistenceError::Serialization(format!("Invalid status: {status_str}")))?;
                    let receipt: Option<ActionReceipt> = receipt_json
                        .map(|s| serde_json::from_str(&s))
                        .transpose()?;

                    Ok(Some(Action {
                        id: ActionId::from_string(id_val),
                        run_id: RunId::from_string(run_val),
                        session_id: SessionId::from_string(session_val),
                        capability,
                        intent,
                        params,
                        status,
                        receipt,
                        created_at,
                        completed_at,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
    }

    async fn update(&self, action: &Action) -> Result<(), PersistenceError> {
        let action = action.clone();
        self.db
            .interact(move |conn| {
                let receipt_str = action
                    .receipt
                    .as_ref()
                    .map(|r| serde_json::to_string(r))
                    .transpose()?;

                let rows = conn.execute(
                    "UPDATE actions SET status = ?1, receipt_json = ?2, completed_at = ?3 WHERE id = ?4",
                    params![
                        action.status.as_str(),
                        receipt_str,
                        action.completed_at,
                        action.id.as_str(),
                    ],
                )?;
                if rows == 0 {
                    return Err(PersistenceError::NotFound(action.id.to_string()));
                }
                Ok(())
            })
            .await
    }

    async fn list_by_run(&self, run_id: &RunId) -> Result<Vec<Action>, PersistenceError> {
        let run_id = run_id.clone();
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, run_id, session_id, capability, intent, params_json, status, receipt_json, created_at, completed_at
                     FROM actions WHERE run_id = ?1 ORDER BY created_at ASC",
                )?;
                let rows = stmt.query_map(params![run_id.as_str()], |row| {
                    let id_val: String = row.get(0)?;
                    let run_val: String = row.get(1)?;
                    let session_val: String = row.get(2)?;
                    let capability: String = row.get(3)?;
                    let intent: String = row.get(4)?;
                    let params_json: String = row.get(5)?;
                    let status_str: String = row.get(6)?;
                    let receipt_json: Option<String> = row.get(7)?;
                    let created_at: i64 = row.get(8)?;
                    let completed_at: Option<i64> = row.get(9)?;
                    Ok((id_val, run_val, session_val, capability, intent, params_json, status_str, receipt_json, created_at, completed_at))
                })?;

                let mut actions = Vec::new();
                for r in rows {
                    let (id_val, run_val, session_val, capability, intent, params_json, status_str, receipt_json, created_at, completed_at) = r?;
                    let params: serde_json::Value = serde_json::from_str(&params_json)?;
                    let status = ActionStatus::from_str(&status_str)
                        .ok_or_else(|| PersistenceError::Serialization(format!("Invalid status: {status_str}")))?;
                    let receipt: Option<ActionReceipt> = receipt_json
                        .map(|s| serde_json::from_str(&s))
                        .transpose()?;

                    actions.push(Action {
                        id: ActionId::from_string(id_val),
                        run_id: RunId::from_string(run_val),
                        session_id: SessionId::from_string(session_val),
                        capability,
                        intent,
                        params,
                        status,
                        receipt,
                        created_at,
                        completed_at,
                    });
                }
                Ok(actions)
            })
            .await
    }
}
