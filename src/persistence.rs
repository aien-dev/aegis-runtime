use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::heartbeat::PulseReceipt;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskRecord {
    pub id: String,
    pub task_type: String,
    pub payload: String,
    pub status: String,
    pub result: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrumbRecord {
    pub key: String,
    pub value: String,
    pub namespace: String,
    pub created_at: String,
}

pub struct Database {
    conn: Arc<Mutex<Connection>>,
    transcript_path: Option<PathBuf>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "cache_size", "-64000")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            transcript_path: Some(path.with_file_name("events.jsonl")),
        };
        db.init_tables()?;

        info!("OpenClaw SQLite WAL database initialized at {:?}", path);
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            transcript_path: None,
        };
        db.init_tables()?;
        Ok(db)
    }

    pub fn set_transcript_path(&mut self, path: PathBuf) {
        self.transcript_path = Some(path);
    }

    fn init_tables(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS turns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                prompt TEXT NOT NULL,
                response TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS heartbeats (
                tick_id INTEGER PRIMARY KEY,
                timestamp TEXT NOT NULL,
                status TEXT NOT NULL,
                tasks_scanned INTEGER NOT NULL,
                actions_dispatched INTEGER NOT NULL,
                notes TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                task_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                status TEXT NOT NULL,
                result TEXT,
                created_at TEXT NOT NULL,
                completed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS memory_crumbs (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                namespace TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS skills (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                command TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    pub fn record_turn(
        &self,
        role: &str,
        prompt: &str,
        response: &str,
        duration_ms: u64,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO turns (session_id, role, prompt, response, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "default",
                role,
                prompt,
                response,
                duration_ms as i64,
                Utc::now().to_rfc3339()
            ],
        )?;

        self.record_transcript(
            "chat_turn",
            &serde_json::json!({
                "role": role,
                "prompt": prompt,
                "response": response,
                "duration_ms": duration_ms,
            }),
        );

        Ok(())
    }

    pub fn record_heartbeat_tick(&self, receipt: &PulseReceipt) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO heartbeats (tick_id, timestamp, status, tasks_scanned, actions_dispatched, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                receipt.tick_id as i64,
                receipt.timestamp,
                receipt.status,
                receipt.tasks_scanned as i64,
                receipt.actions_dispatched as i64,
                receipt.notes
            ],
        )?;

        self.record_transcript("heartbeat_tick", &receipt);
        Ok(())
    }

    pub fn create_task(
        &self,
        id: &str,
        task_type: &str,
        payload: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO tasks (id, task_type, payload, status, created_at)
             VALUES (?1, ?2, ?3, 'pending', ?4)",
            params![id, task_type, payload, now],
        )?;

        self.record_transcript(
            "task_created",
            &serde_json::json!({
                "id": id,
                "task_type": task_type,
                "payload": payload,
            }),
        );
        Ok(())
    }

    pub fn list_pending_tasks(&self) -> Result<Vec<TaskRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, task_type, payload, status, result, created_at, completed_at
             FROM tasks WHERE status = 'pending' ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TaskRecord {
                id: row.get(0)?,
                task_type: row.get(1)?,
                payload: row.get(2)?,
                status: row.get(3)?,
                result: row.get(4)?,
                created_at: row.get(5)?,
                completed_at: row.get(6)?,
            })
        })?;

        let mut tasks = Vec::new();
        for r in rows {
            tasks.push(r?);
        }
        Ok(tasks)
    }

    pub fn complete_task(&self, id: &str, result: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE tasks SET status = 'completed', result = ?1, completed_at = ?2 WHERE id = ?3",
            params![result, now, id],
        )?;

        self.record_transcript(
            "task_completed",
            &serde_json::json!({
                "id": id,
                "result": result,
                "completed_at": now,
            }),
        );
        Ok(())
    }

    pub fn store_crumb(
        &self,
        key: &str,
        value: &str,
        namespace: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO memory_crumbs (key, value, namespace, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![key, value, namespace, now],
        )?;

        self.record_transcript(
            "memory_crumb",
            &serde_json::json!({
                "key": key,
                "value": value,
                "namespace": namespace,
            }),
        );
        Ok(())
    }

    pub fn get_recent_crumbs(
        &self,
        namespace: &str,
        limit: usize,
    ) -> Result<Vec<CrumbRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT key, value, namespace, created_at
             FROM memory_crumbs WHERE namespace = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![namespace, limit as i64], |row| {
            Ok(CrumbRecord {
                key: row.get(0)?,
                value: row.get(1)?,
                namespace: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;

        let mut crumbs = Vec::new();
        for r in rows {
            crumbs.push(r?);
        }
        Ok(crumbs)
    }

    pub fn record_transcript<T: Serialize>(&self, event_type: &str, data: &T) {
        if let Some(ref path) = self.transcript_path {
            let event = serde_json::json!({
                "timestamp": Utc::now().to_rfc3339(),
                "event_type": event_type,
                "data": data,
            });
            if let Ok(line) = serde_json::to_string(&event) {
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                    let _ = writeln!(file, "{}", line);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_lifecycle_and_crumbs() {
        let db = Database::open_in_memory().unwrap();
        db.create_task("t1", "build_check", "cargo check").unwrap();

        let pending = db.list_pending_tasks().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "t1");
        assert_eq!(pending[0].task_type, "build_check");

        db.complete_task("t1", "passed").unwrap();
        let remaining = db.list_pending_tasks().unwrap();
        assert_eq!(remaining.len(), 0);

        db.store_crumb("summary", "all tests passed", "cortex").unwrap();
        let crumbs = db.get_recent_crumbs("cortex", 10).unwrap();
        assert_eq!(crumbs.len(), 1);
        assert_eq!(crumbs[0].key, "summary");
        assert_eq!(crumbs[0].value, "all tests passed");
    }
}
