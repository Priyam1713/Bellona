//! SQLite-backed session store — durable campaigns (Law VI), local-first
//! (Law III). Sessions are stored as canonical JSON blobs: schema evolution
//! stays trivial and full fidelity is preserved across versions.

use crate::MemoriaError;
use async_trait::async_trait;
use forge::session::{Session, SessionStore};
use forge::SessionId;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct SqliteSessionStore {
    conn: Mutex<Connection>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl SqliteSessionStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MemoriaError> {
        let conn = Connection::open(path).map_err(|e| MemoriaError(format!("sqlite open: {e}")))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS sessions (
                 id TEXT PRIMARY KEY,
                 updated_ms INTEGER NOT NULL,
                 data TEXT NOT NULL
             );",
        )
        .map_err(|e| MemoriaError(format!("sqlite schema: {e}")))?;
        Ok(SqliteSessionStore {
            conn: Mutex::new(conn),
        })
    }

    pub fn in_memory() -> Result<Self, MemoriaError> {
        Self::open(":memory:")
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn put(&self, session: Session) -> forge::ForgeResult<()> {
        let data = serde_json::to_string(&session)?;
        self.conn
            .lock()
            .map_err(|_| forge::ForgeError::Other("session lock poisoned".into()))?
            .execute(
                "INSERT OR REPLACE INTO sessions (id, updated_ms, data) VALUES (?1, ?2, ?3)",
                rusqlite::params![session.id.0 .0, now_ms() as i64, data],
            )
            .map_err(|e| forge::ForgeError::Other(e.to_string()))?;
        Ok(())
    }

    async fn get(&self, id: &SessionId) -> forge::ForgeResult<Session> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| forge::ForgeError::Other("session lock poisoned".into()))?;
        let mut stmt = conn
            .prepare("SELECT data FROM sessions WHERE id = ?1")
            .map_err(|e| forge::ForgeError::Other(e.to_string()))?;
        let json: String = stmt
            .query_row([&id.0 .0], |r| r.get(0))
            .map_err(|_| forge::ForgeError::SessionNotFound(id.to_string()))?;
        serde_json::from_str(&json)
            .map_err(|e| forge::ForgeError::Other(format!("session decode: {e}")))
    }

    async fn list(&self) -> forge::ForgeResult<Vec<SessionId>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| forge::ForgeError::Other("session lock poisoned".into()))?;
        let mut stmt = conn
            .prepare("SELECT id FROM sessions ORDER BY updated_ms DESC")
            .map_err(|e| forge::ForgeError::Other(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| forge::ForgeError::Other(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(SessionId(forge::Id(
                r.map_err(|e| forge::ForgeError::Other(e.to_string()))?,
            )));
        }
        Ok(out)
    }
}
