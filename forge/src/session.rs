//! Durable sessions (Law VI): resumable, searchable, lineage-aware.

use crate::error::{ForgeError, ForgeResult};
use crate::id::SessionId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::RwLock;

/// One entry in the session ledger — a decision record, never a raw log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub seq: u64,
    pub ts_ms: u64,
    pub kind: String,
    pub summary: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// A durable campaign thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub created_at_ms: u64,
    pub goal: String,
    /// Pinned context survives every compaction byte-identical.
    pub pinned: Vec<String>,
    /// Decision ledger.
    pub ledger: Vec<LedgerEntry>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl Session {
    pub fn new(goal: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Session {
            id: SessionId::mint(),
            created_at_ms: now,
            goal: goal.into(),
            pinned: Vec::new(),
            ledger: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    /// Append a decision record; returns its sequence number.
    pub fn append(&mut self, kind: &str, summary: &str, payload: serde_json::Value) -> u64 {
        let seq = self.ledger.len() as u64;
        self.ledger.push(LedgerEntry {
            seq,
            ts_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(seq),
            kind: kind.to_string(),
            summary: summary.to_string(),
            payload,
        });
        seq
    }

    /// Full-text-ish search over ledger summaries (FTS5 backend in M-II).
    pub fn search(&self, needle: &str) -> Vec<&LedgerEntry> {
        let n = needle.to_lowercase();
        self.ledger
            .iter()
            .filter(|e| e.summary.to_lowercase().contains(&n) || e.kind.to_lowercase().contains(&n))
            .collect()
    }
}

/// Storage contract for sessions.
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn put(&self, session: Session) -> ForgeResult<()>;
    async fn get(&self, id: &SessionId) -> ForgeResult<Session>;
    async fn list(&self) -> ForgeResult<Vec<SessionId>>;
}

/// In-memory store; SQLite/Postgres backends are drop-in replacements
/// behind this trait (Law III: zero hostages).
#[derive(Default)]
pub struct InMemorySessionStore {
    inner: RwLock<BTreeMap<SessionId, Session>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn put(&self, session: Session) -> ForgeResult<()> {
        self.inner
            .write()
            .map_err(|_| ForgeError::Other("session lock poisoned".into()))?
            .insert(session.id.clone(), session);
        Ok(())
    }

    async fn get(&self, id: &SessionId) -> ForgeResult<Session> {
        self.inner
            .read()
            .map_err(|_| ForgeError::Other("session lock poisoned".into()))?
            .get(id)
            .cloned()
            .ok_or_else(|| ForgeError::SessionNotFound(id.to_string()))
    }

    async fn list(&self) -> ForgeResult<Vec<SessionId>> {
        Ok(self
            .inner
            .read()
            .map_err(|_| ForgeError::Other("session lock poisoned".into()))?
            .keys()
            .cloned()
            .collect())
    }
}
