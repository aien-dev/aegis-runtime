use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistenceError {
    Sqlite(String),
    NotFound(String),
    ConcurrencyConflict {
        id: String,
        expected_version: u64,
    },
    ActiveRunConflict {
        session_id: String,
        active_run_id: String,
    },
    Serialization(String),
    LockPoisoned,
    TaskJoin(String),
    Other(String),
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "SQLite error: {e}"),
            Self::NotFound(msg) => write!(f, "Record not found: {msg}"),
            Self::ConcurrencyConflict {
                id,
                expected_version,
            } => {
                write!(
                    f,
                    "Concurrency conflict on {id}: expected version {expected_version}"
                )
            }
            Self::ActiveRunConflict {
                session_id,
                active_run_id,
            } => {
                write!(
                    f,
                    "Session {session_id} already has active run: {active_run_id}"
                )
            }
            Self::Serialization(e) => write!(f, "Serialization error: {e}"),
            Self::LockPoisoned => write!(f, "Database lock poisoned"),
            Self::TaskJoin(e) => write!(f, "Task join error: {e}"),
            Self::Other(msg) => write!(f, "Persistence error: {msg}"),
        }
    }
}

impl std::error::Error for PersistenceError {}

impl From<rusqlite::Error> for PersistenceError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sqlite(err.to_string())
    }
}

impl From<serde_json::Error> for PersistenceError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}
