use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::heartbeat::PulseReceipt;

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        // Enable Write-Ahead-Log (WAL) mode for sub-millisecond concurrent operations
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "cache_size", "-64000")?;

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

            CREATE TABLE IF NOT EXISTS skills (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                command TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )?;

        info!("OpenClaw SQLite WAL database initialized at {:?}", path);
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS turns (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, role TEXT NOT NULL, prompt TEXT NOT NULL, response TEXT NOT NULL, duration_ms INTEGER NOT NULL, created_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS heartbeats (tick_id INTEGER PRIMARY KEY, timestamp TEXT NOT NULL, status TEXT NOT NULL, tasks_scanned INTEGER NOT NULL, actions_dispatched INTEGER NOT NULL, notes TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS skills (name TEXT PRIMARY KEY, description TEXT NOT NULL, command TEXT NOT NULL, updated_at TEXT NOT NULL);
            ",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
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
        Ok(())
    }
}
