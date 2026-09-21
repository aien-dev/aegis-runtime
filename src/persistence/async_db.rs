use super::error::PersistenceError;
use super::sqlite::schema::initialize_schema;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AsyncDatabase {
    conn: Arc<Mutex<Connection>>,
    transcript_path: Option<PathBuf>,
}

impl AsyncDatabase {
    pub fn open(path: &Path) -> Result<Self, PersistenceError> {
        let conn = Connection::open(path)?;
        initialize_schema(&conn)?;
        let transcript_path = path.parent().map(|p| p.join("transcript.jsonl"));
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            transcript_path,
        })
    }

    pub fn open_in_memory() -> Result<Self, PersistenceError> {
        let conn = Connection::open_in_memory()?;
        initialize_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            transcript_path: None,
        })
    }

    pub fn from_connection_arc(
        conn: Arc<Mutex<Connection>>,
        transcript_path: Option<PathBuf>,
    ) -> Self {
        Self {
            conn,
            transcript_path,
        }
    }

    pub fn shared_conn(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }

    pub fn transcript_path(&self) -> Option<&PathBuf> {
        self.transcript_path.as_ref()
    }

    pub async fn interact<F, R>(&self, f: F) -> Result<R, PersistenceError>
    where
        F: FnOnce(&mut Connection) -> Result<R, PersistenceError> + Send + 'static,
        R: Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let mut guard = conn.lock().map_err(|_| PersistenceError::LockPoisoned)?;
            f(&mut guard)
        })
        .await
        .map_err(|e| PersistenceError::TaskJoin(e.to_string()))?
    }
}
