use crate::orchestration::budget::RunBudget;
use crate::orchestration::run::{Run, RunState};
use crate::orchestration::termination::TerminationReason;
use crate::orchestration::trigger::Trigger;
use crate::persistence::async_db::AsyncDatabase;
use crate::persistence::error::PersistenceError;
use crate::sessions::id::{RunId, SessionId};
use crate::sessions::store::RunStore;
use async_trait::async_trait;
use rusqlite::params;

#[derive(Clone)]
pub struct SqliteRunStore {
    db: AsyncDatabase,
}

impl SqliteRunStore {
    pub fn new(db: AsyncDatabase) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RunStore for SqliteRunStore {
    async fn create(&self, run: &Run) -> Result<(), PersistenceError> {
        let run = run.clone();
        self.db
            .interact(move |conn| {
                let active_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM runs WHERE session_id = ?1 AND state IN ('running', 'waiting_for_approval', 'waiting_for_tool')",
                    params![run.session_id.as_str()],
                    |r| r.get(0),
                )?;
                if active_count > 0 {
                    let active_id: String = conn.query_row(
                        "SELECT id FROM runs WHERE session_id = ?1 AND state IN ('running', 'waiting_for_approval', 'waiting_for_tool') LIMIT 1",
                        params![run.session_id.as_str()],
                        |r| r.get(0),
                    )?;
                    return Err(PersistenceError::ActiveRunConflict {
                        session_id: run.session_id.to_string(),
                        active_run_id: active_id,
                    });
                }

                let trigger_str = serde_json::to_string(&run.trigger)?;
                let budget_str = serde_json::to_string(&run.budget)?;
                let term_str = run.termination.as_ref().map(|t| serde_json::to_string(t)).transpose()?;

                conn.execute(
                    "INSERT INTO runs (id, session_id, state, trigger_json, steps_used, inference_calls_used, actions_used, budget_json, started_at, completed_at, termination_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        run.id.as_str(),
                        run.session_id.as_str(),
                        run.state.as_str(),
                        trigger_str,
                        run.steps_used as i64,
                        run.inference_calls_used as i64,
                        run.actions_used as i64,
                        budget_str,
                        run.started_at,
                        run.completed_at,
                        term_str,
                    ],
                )?;
                Ok(())
            })
            .await
    }

    async fn get(&self, id: &RunId) -> Result<Option<Run>, PersistenceError> {
        let id_str = id.to_string();
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, session_id, state, trigger_json, steps_used, inference_calls_used, actions_used, budget_json, started_at, completed_at, termination_json
                     FROM runs WHERE id = ?1",
                )?;
                let mut rows = stmt.query(params![id_str])?;
                if let Some(row) = rows.next()? {
                    let id_val: String = row.get(0)?;
                    let session_val: String = row.get(1)?;
                    let state_str: String = row.get(2)?;
                    let trigger_json: String = row.get(3)?;
                    let steps_used: i64 = row.get(4)?;
                    let inference_calls_used: i64 = row.get(5)?;
                    let actions_used: i64 = row.get(6)?;
                    let budget_json: String = row.get(7)?;
                    let started_at: i64 = row.get(8)?;
                    let completed_at: Option<i64> = row.get(9)?;
                    let term_json: Option<String> = row.get(10)?;

                    let state = RunState::from_str(&state_str)
                        .ok_or_else(|| PersistenceError::Serialization(format!("Invalid state: {state_str}")))?;
                    let trigger: Trigger = serde_json::from_str(&trigger_json)?;
                    let budget: RunBudget = serde_json::from_str(&budget_json)?;
                    let termination: Option<TerminationReason> = term_json
                        .map(|s| serde_json::from_str(&s))
                        .transpose()?;

                    Ok(Some(Run {
                        id: RunId::from_string(id_val),
                        session_id: SessionId::from_string(session_val),
                        state,
                        trigger,
                        steps_used: steps_used as u32,
                        inference_calls_used: inference_calls_used as u32,
                        actions_used: actions_used as u32,
                        budget,
                        started_at,
                        completed_at,
                        termination,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
    }

    async fn update(&self, run: &Run) -> Result<(), PersistenceError> {
        let run = run.clone();
        self.db
            .interact(move |conn| {
                let budget_str = serde_json::to_string(&run.budget)?;
                let term_str = run
                    .termination
                    .as_ref()
                    .map(|t| serde_json::to_string(t))
                    .transpose()?;

                let rows = conn.execute(
                    "UPDATE runs SET state = ?1, steps_used = ?2, inference_calls_used = ?3,
                     actions_used = ?4, budget_json = ?5, completed_at = ?6, termination_json = ?7
                     WHERE id = ?8",
                    params![
                        run.state.as_str(),
                        run.steps_used as i64,
                        run.inference_calls_used as i64,
                        run.actions_used as i64,
                        budget_str,
                        run.completed_at,
                        term_str,
                        run.id.as_str(),
                    ],
                )?;
                if rows == 0 {
                    return Err(PersistenceError::NotFound(run.id.to_string()));
                }
                Ok(())
            })
            .await
    }

    async fn list_by_session(&self, session_id: &SessionId) -> Result<Vec<Run>, PersistenceError> {
        let session_id = session_id.clone();
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, session_id, state, trigger_json, steps_used, inference_calls_used, actions_used, budget_json, started_at, completed_at, termination_json
                     FROM runs WHERE session_id = ?1 ORDER BY started_at DESC",
                )?;
                let rows = stmt.query_map(params![session_id.as_str()], |row| {
                    let id_val: String = row.get(0)?;
                    let session_val: String = row.get(1)?;
                    let state_str: String = row.get(2)?;
                    let trigger_json: String = row.get(3)?;
                    let steps_used: i64 = row.get(4)?;
                    let inference_calls_used: i64 = row.get(5)?;
                    let actions_used: i64 = row.get(6)?;
                    let budget_json: String = row.get(7)?;
                    let started_at: i64 = row.get(8)?;
                    let completed_at: Option<i64> = row.get(9)?;
                    let term_json: Option<String> = row.get(10)?;
                    Ok((id_val, session_val, state_str, trigger_json, steps_used, inference_calls_used, actions_used, budget_json, started_at, completed_at, term_json))
                })?;

                let mut runs = Vec::new();
                for r in rows {
                    let (id_val, session_val, state_str, trigger_json, steps_used, inference_calls_used, actions_used, budget_json, started_at, completed_at, term_json) = r?;
                    let state = RunState::from_str(&state_str)
                        .ok_or_else(|| PersistenceError::Serialization(format!("Invalid state: {state_str}")))?;
                    let trigger: Trigger = serde_json::from_str(&trigger_json)?;
                    let budget: RunBudget = serde_json::from_str(&budget_json)?;
                    let termination: Option<TerminationReason> = term_json
                        .map(|s| serde_json::from_str(&s))
                        .transpose()?;

                    runs.push(Run {
                        id: RunId::from_string(id_val),
                        session_id: SessionId::from_string(session_val),
                        state,
                        trigger,
                        steps_used: steps_used as u32,
                        inference_calls_used: inference_calls_used as u32,
                        actions_used: actions_used as u32,
                        budget,
                        started_at,
                        completed_at,
                        termination,
                    });
                }
                Ok(runs)
            })
            .await
    }

    async fn get_active_run(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<Run>, PersistenceError> {
        let session_id = session_id.clone();
        self.db
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, session_id, state, trigger_json, steps_used, inference_calls_used, actions_used, budget_json, started_at, completed_at, termination_json
                     FROM runs WHERE session_id = ?1 AND state IN ('running', 'waiting_for_approval', 'waiting_for_tool') LIMIT 1",
                )?;
                let mut rows = stmt.query(params![session_id.as_str()])?;
                if let Some(row) = rows.next()? {
                    let id_val: String = row.get(0)?;
                    let session_val: String = row.get(1)?;
                    let state_str: String = row.get(2)?;
                    let trigger_json: String = row.get(3)?;
                    let steps_used: i64 = row.get(4)?;
                    let inference_calls_used: i64 = row.get(5)?;
                    let actions_used: i64 = row.get(6)?;
                    let budget_json: String = row.get(7)?;
                    let started_at: i64 = row.get(8)?;
                    let completed_at: Option<i64> = row.get(9)?;
                    let term_json: Option<String> = row.get(10)?;

                    let state = RunState::from_str(&state_str)
                        .ok_or_else(|| PersistenceError::Serialization(format!("Invalid state: {state_str}")))?;
                    let trigger: Trigger = serde_json::from_str(&trigger_json)?;
                    let budget: RunBudget = serde_json::from_str(&budget_json)?;
                    let termination: Option<TerminationReason> = term_json
                        .map(|s| serde_json::from_str(&s))
                        .transpose()?;

                    Ok(Some(Run {
                        id: RunId::from_string(id_val),
                        session_id: SessionId::from_string(session_val),
                        state,
                        trigger,
                        steps_used: steps_used as u32,
                        inference_calls_used: inference_calls_used as u32,
                        actions_used: actions_used as u32,
                        budget,
                        started_at,
                        completed_at,
                        termination,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
    }
}
