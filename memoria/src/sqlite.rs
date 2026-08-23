//! SQLite-backed Archivum Ã¢â‚¬â€ the durable camp archive (Law III: local-first).
//!
//! FTS5 when the bundled build provides it; deterministic LIKE-scoring as
//! the always-correct fallback. Same [`ArchivumStore`] contract either way.

use crate::{ArchivumStore, Episode, MemoriaError};
use async_trait::async_trait;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub struct SqliteArchivum {
    conn: Mutex<Connection>,
    fts_available: bool,
}

impl SqliteArchivum {
    /// Open (creating if needed) an archive at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MemoriaError> {
        let conn = Connection::open(path).map_err(|e| MemoriaError(format!("sqlite open: {e}")))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS episodes (
                 id TEXT PRIMARY KEY,
                 ts_ms INTEGER NOT NULL,
                 kind TEXT NOT NULL,
                 content TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_episodes_kind ON episodes(kind);",
        )
        .map_err(|e| MemoriaError(format!("sqlite schema: {e}")))?;

        // Probe FTS5 once.
        let fts_available = conn
            .execute_batch(
                "CREATE VIRTUAL TABLE IF NOT EXISTS episodes_fts \
                 USING fts5(content, content='episodes', content_rowid='rowid');",
            )
            .is_ok();

        Ok(SqliteArchivum {
            conn: Mutex::new(conn),
            fts_available,
        })
    }

    pub fn in_memory() -> Result<Self, MemoriaError> {
        Self::open(":memory:")
    }

    pub fn uses_fts(&self) -> bool {
        self.fts_available
    }

    fn like_search(&self, query: &str, limit: usize) -> Result<Vec<Episode>, MemoriaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| MemoriaError("archive lock poisoned".into()))?;
        let terms: Vec<String> = query
            .split_whitespace()
            .filter(|t| !t.is_empty())
            .map(|t| format!("%{}%", t.replace(['%', '_'], "")))
            .collect();
        let mut scored: Vec<(i64, Episode)> = Vec::new();
        let mut stmt = conn
            .prepare("SELECT id, ts_ms, kind, content FROM episodes")
            .map_err(|e| MemoriaError(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Episode {
                    id: r.get(0)?,
                    ts_ms: r.get(1)?,
                    kind: r.get(2)?,
                    content: r.get(3)?,
                })
            })
            .map_err(|e| MemoriaError(e.to_string()))?;
        for row in rows {
            let e = row.map_err(|e| MemoriaError(e.to_string()))?;
            let hay = e.content.to_lowercase();
            let kind_l = e.kind.to_lowercase();
            let score: i64 = terms
                .iter()
                .map(|t| {
                    let tl = t.trim_matches('%').to_lowercase();
                    (hay.matches(&tl).count() + if kind_l.contains(&tl) { 1 } else { 0 }) as i64
                })
                .sum();
            if score > 0 {
                scored.push((score, e));
            }
        }
        scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
        Ok(scored.into_iter().take(limit).map(|(_, e)| e).collect())
    }
}

#[async_trait]
impl ArchivumStore for SqliteArchivum {
    async fn put(&self, episode: Episode) -> Result<(), MemoriaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| MemoriaError("archive lock poisoned".into()))?;
        conn.execute(
            "INSERT OR REPLACE INTO episodes (id, ts_ms, kind, content) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                episode.id,
                episode.ts_ms as i64,
                episode.kind,
                episode.content
            ],
        )
        .map_err(|e| MemoriaError(e.to_string()))?;
        #[allow(unused_must_use)]
        {
            conn.execute(
                "INSERT INTO episodes_fts(rowid, content) \
                 SELECT rowid, content FROM episodes WHERE id = ?1",
                rusqlite::params![episode.id],
            );
        }
        Ok(())
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Episode>, MemoriaError> {
        if query.trim().is_empty() {
            return Ok(vec![]);
        }
        if self.fts_available {
            let conn = self
                .conn
                .lock()
                .map_err(|_| MemoriaError("archive lock poisoned".into()))?;
            let match_query: String = query.split_whitespace().collect::<Vec<_>>().join(" OR ");
            let mut stmt = conn
                .prepare(
                    "SELECT e.id, e.ts_ms, e.kind, e.content \
                     FROM episodes_fts f JOIN episodes e ON e.rowid = f.rowid \
                     WHERE episodes_fts MATCH ?1 ORDER BY bm25(episodes_fts) LIMIT ?2",
                )
                .map_err(|e| MemoriaError(e.to_string()))?;
            let rows = stmt
                .query_map(rusqlite::params![match_query, limit as i64], |r| {
                    Ok(Episode {
                        id: r.get(0)?,
                        ts_ms: r.get(1)?,
                        kind: r.get(2)?,
                        content: r.get(3)?,
                    })
                })
                .map_err(|e| MemoriaError(e.to_string()))?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row.map_err(|e| MemoriaError(e.to_string()))?);
            }
            if !out.is_empty() {
                return Ok(out);
            }
            // FTS found nothing Ã¢â‚¬â€ fall through to scoring for partial hits.
        }
        self.like_search(query, limit)
    }

    async fn by_kind(&self, kind: &str) -> Result<Vec<Episode>, MemoriaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| MemoriaError("archive lock poisoned".into()))?;
        let mut stmt = conn
            .prepare("SELECT id, ts_ms, kind, content FROM episodes WHERE kind = ?1 ORDER BY ts_ms")
            .map_err(|e| MemoriaError(e.to_string()))?;
        let rows = stmt
            .query_map([kind], |r| {
                Ok(Episode {
                    id: r.get(0)?,
                    ts_ms: r.get(1)?,
                    kind: r.get(2)?,
                    content: r.get(3)?,
                })
            })
            .map_err(|e| MemoriaError(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| MemoriaError(e.to_string()))?);
        }
        Ok(out)
    }
}

/// Durable idempotency ledger for exactly-once delegation (Campaign IX).
pub struct IdempotencyLedger {
    conn: Mutex<Connection>,
}

impl IdempotencyLedger {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MemoriaError> {
        let conn = Connection::open(path).map_err(|e| MemoriaError(format!("sqlite open: {e}")))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS idempotency (
                 key TEXT PRIMARY KEY,
                 response TEXT NOT NULL,
                 ts_ms INTEGER NOT NULL
             );",
        )
        .map_err(|e| MemoriaError(format!("sqlite schema: {e}")))?;
        Ok(IdempotencyLedger {
            conn: Mutex::new(conn),
        })
    }

    pub fn in_memory() -> Result<Self, MemoriaError> {
        Self::open(":memory:")
    }

    /// Returns cached response if present; otherwise stores and returns None
    /// meaning "caller must execute".
    pub fn claim(&self, key: &str) -> Result<Option<String>, MemoriaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| MemoriaError("ledger lock".into()))?;
        let existing: Option<String> = conn
            .query_row(
                "SELECT response FROM idempotency WHERE key = ?1",
                [key],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(MemoriaError(other.to_string())),
            })?;
        if existing.is_some() {
            return Ok(existing);
        }
        // Reserve the slot with an empty marker so a concurrent caller sees it.
        conn.execute(
            "INSERT OR IGNORE INTO idempotency (key, response, ts_ms) VALUES (?1, '', ?2)",
            rusqlite::params![key, now_ms() as i64],
        )
        .map_err(|e| MemoriaError(e.to_string()))?;
        Ok(None)
    }

    pub fn complete(&self, key: &str, response: &str) -> Result<(), MemoriaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| MemoriaError("ledger lock".into()))?;
        conn.execute(
            "INSERT OR REPLACE INTO idempotency (key, response, ts_ms) VALUES (?1, ?2, ?3)",
            rusqlite::params![key, response, now_ms() as i64],
        )
        .map_err(|e| MemoriaError(e.to_string()))?;
        Ok(())
    }
}
