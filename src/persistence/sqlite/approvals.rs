use crate::defense::approval::{Approval, ApprovalStatus};
use crate::persistence::async_db::AsyncDatabase;
use crate::persistence::error::PersistenceError;
use crate::sessions::id::{ActionId, ApprovalId, RunId, SessionId};
use crate::sessions::store::ApprovalStore;
use async_trait::async_trait;
use rusqlite::params;

#[derive(Clone)]
pub struct SqliteApprovalStore {
    db: AsyncDatabase,
}

impl SqliteApprovalStore {
    pub fn new(db: AsyncDatabase) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ApprovalStore for SqliteApprovalStore {
    async fn create(&self, approval: &Approval) -> Result<(), PersistenceError> {
        let approval = approval.clone();
        self.db
            .interact(move |conn| {
                conn.execute(
                    "INSERT INTO approvals (id, action_id, run_id, session_id, prompt, status, requested_at, decided_at, decided_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        approval.id.as_str(),
                        approval.action_id.as_str(),
                        approval.run_id.as_str(),
                        approval.session_id.as_str(),
                        approval.prompt,
                        approval.status.as_str(),
                        approval.requested_at,
                        approval.decided_at,
                        approval.decided_by,
                    ],
                )?;
                Ok(())
            })
            .await
    }

    async fn get(&self, id: &ApprovalId) -> Result<Option<Approval>, PersistenceError> {
        let id_str = id.to_string();
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, action_id, run_id, session_id, prompt, status, requested_at, decided_at, decided_by
                     FROM approvals WHERE id = ?1",
                )?;
                let mut rows = stmt.query(params![id_str])?;
                if let Some(row) = rows.next()? {
                    let id_val: String = row.get(0)?;
                    let action_val: String = row.get(1)?;
                    let run_val: String = row.get(2)?;
                    let session_val: String = row.get(3)?;
                    let prompt: String = row.get(4)?;
                    let status_str: String = row.get(5)?;
                    let requested_at: i64 = row.get(6)?;
                    let decided_at: Option<i64> = row.get(7)?;
                    let decided_by: Option<String> = row.get(8)?;

                    let status = ApprovalStatus::from_str(&status_str)
                        .ok_or_else(|| PersistenceError::Serialization(format!("Invalid status: {status_str}")))?;

                    Ok(Some(Approval {
                        id: ApprovalId::from_string(id_val),
                        action_id: ActionId::from_string(action_val),
                        run_id: RunId::from_string(run_val),
                        session_id: SessionId::from_string(session_val),
                        prompt,
                        status,
                        requested_at,
                        decided_at,
                        decided_by,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
    }

    async fn update(&self, approval: &Approval) -> Result<(), PersistenceError> {
        let approval = approval.clone();
        self.db
            .interact(move |conn| {
                let rows = conn.execute(
                    "UPDATE approvals SET status = ?1, decided_at = ?2, decided_by = ?3 WHERE id = ?4",
                    params![
                        approval.status.as_str(),
                        approval.decided_at,
                        approval.decided_by,
                        approval.id.as_str(),
                    ],
                )?;
                if rows == 0 {
                    return Err(PersistenceError::NotFound(approval.id.to_string()));
                }
                Ok(())
            })
            .await
    }

    async fn list_pending(&self) -> Result<Vec<Approval>, PersistenceError> {
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, action_id, run_id, session_id, prompt, status, requested_at, decided_at, decided_by
                     FROM approvals WHERE status = 'pending' ORDER BY requested_at ASC",
                )?;
                let rows = stmt.query_map([], |row| {
                    let id_val: String = row.get(0)?;
                    let action_val: String = row.get(1)?;
                    let run_val: String = row.get(2)?;
                    let session_val: String = row.get(3)?;
                    let prompt: String = row.get(4)?;
                    let status_str: String = row.get(5)?;
                    let requested_at: i64 = row.get(6)?;
                    let decided_at: Option<i64> = row.get(7)?;
                    let decided_by: Option<String> = row.get(8)?;
                    Ok((id_val, action_val, run_val, session_val, prompt, status_str, requested_at, decided_at, decided_by))
                })?;

                let mut approvals = Vec::new();
                for r in rows {
                    let (id_val, action_val, run_val, session_val, prompt, status_str, requested_at, decided_at, decided_by) = r?;
                    let status = ApprovalStatus::from_str(&status_str)
                        .ok_or_else(|| PersistenceError::Serialization(format!("Invalid status: {status_str}")))?;
                    approvals.push(Approval {
                        id: ApprovalId::from_string(id_val),
                        action_id: ActionId::from_string(action_val),
                        run_id: RunId::from_string(run_val),
                        session_id: SessionId::from_string(session_val),
                        prompt,
                        status,
                        requested_at,
                        decided_at,
                        decided_by,
                    });
                }
                Ok(approvals)
            })
            .await
    }
}
