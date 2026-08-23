//! Campaign X.2/X.3 Ã¢â‚¬â€ SQLite vector store + hybrid recall.
//!
//! Vectors live as BLOBs; search is brute-force cosine (honest to ~100k
//! vectors, upgrade path documented). Recall fuses keyword and vector
//! channels with Reciprocal Rank Fusion Ã¢â‚¬â€ the small thing everyone skips
//! and every production system needs.

use crate::{ArchivumStore, Embedder, Episode, MemoriaError};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct SqliteVectors {
    conn: Mutex<Connection>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl SqliteVectors {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MemoriaError> {
        let conn = Connection::open(path).map_err(|e| MemoriaError(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS vectors (
                 episode_id TEXT PRIMARY KEY,
                 kind TEXT NOT NULL,
                 content TEXT NOT NULL,
                 ts_ms INTEGER NOT NULL,
                 dim INTEGER NOT NULL,
                 blob BLOB NOT NULL
             );",
        )
        .map_err(|e| MemoriaError(e.to_string()))?;
        Ok(SqliteVectors {
            conn: Mutex::new(conn),
        })
    }

    pub fn in_memory() -> Result<Self, MemoriaError> {
        Self::open(":memory:")
    }

    pub fn insert(&self, ep: &Episode, vec: &[f32]) -> Result<(), MemoriaError> {
        let conn = self.conn.lock().map_err(|_| MemoriaError("lock".into()))?;
        conn.execute(
            "INSERT OR REPLACE INTO vectors (episode_id, kind, content, ts_ms, dim, blob) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                ep.id,
                ep.kind,
                ep.content,
                ep.ts_ms as i64,
                vec.len() as i64,
                crate::embed::pack(vec),
            ],
        )
        .map_err(|e| MemoriaError(e.to_string()))?;
        Ok(())
    }

    /// Brute-force cosine scan; returns (episode, similarity) top-k with
    /// dimension-mismatched rows skipped loudly-quietly (they are dead data).
    pub fn similar(&self, q: &[f32], k: usize) -> Result<Vec<(Episode, f32)>, MemoriaError> {
        let conn = self.conn.lock().map_err(|_| MemoriaError("lock".into()))?;
        let mut stmt = conn
            .prepare("SELECT episode_id, kind, content, ts_ms, dim, blob FROM vectors")
            .map_err(|e| MemoriaError(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Vec<u8>>(5)?,
                ))
            })
            .map_err(|e| MemoriaError(e.to_string()))?;

        let mut scored: Vec<(Episode, f32)> = Vec::new();
        for row in rows {
            let (id, kind, content, ts, dim, blob) =
                row.map_err(|e| MemoriaError(e.to_string()))?;
            if dim as usize != q.len() {
                continue;
            }
            let v = crate::embed::unpack(&blob);
            let sim = crate::embed::cosine(q, &v);
            if sim > 0.05 {
                scored.push((
                    Episode {
                        id,
                        kind,
                        content,
                        ts_ms: ts as u64,
                    },
                    sim,
                ));
            }
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        Ok(scored)
    }

    pub fn is_empty(&self) -> Result<bool, MemoriaError> {
        Ok(self.len()? == 0)
    }

    pub fn len(&self) -> Result<usize, MemoriaError> {
        let conn = self.conn.lock().map_err(|_| MemoriaError("lock".into()))?;
        conn
            .query_row("SELECT COUNT(*) FROM vectors", [], |r| r.get(0))
            .map_err(|e| MemoriaError(e.to_string()))
    }
}

// ---------- X.3: hybrid recall ----------

/// Fuses keyword-channel results (from any ArchivumStore) with vector
/// results via Reciprocal Rank Fusion (k=60), then applies a gentle recency
/// boost so fresh facts outrank ancient twins at equal relevance.
pub struct HybridRecall<A, E> {
    pub archivum: A,
    pub vectors: SqliteVectors,
    pub embedder: E,
}

const RRF_K: f32 = 60.0;

impl<A: ArchivumStore, E: Embedder> HybridRecall<A, E> {
    pub fn new(archivum: A, vectors: SqliteVectors, embedder: E) -> Self {
        HybridRecall {
            archivum,
            vectors,
            embedder,
        }
    }

    pub async fn recall(&self, query: &str, limit: usize) -> Result<Vec<Episode>, MemoriaError> {
        // Channel 1: keyword (FTS/LIKE semantics behind the trait).
        let kw = self.archivum.search(query, limit).await.unwrap_or_default();

        // Channel 2: vectors.
        let qv = self.embedder.embed(query).await?;
        let vec_hits = self.vectors.similar(&qv, limit)?;

        // RRF fusion keyed by content (ids may differ across stores).
        let mut scores: std::collections::HashMap<String, (f32, Episode)> =
            std::collections::HashMap::new();
        for (rank, ep) in kw.iter().enumerate() {
            let s = 1.0 / (RRF_K + rank as f32);
            let e = scores
                .entry(ep.content.clone())
                .or_insert((0.0, ep.clone()));
            e.0 += s;
        }
        for (rank, (ep, _sim)) in vec_hits.iter().enumerate() {
            let s = 1.0 / (RRF_K + rank as f32);
            let e = scores
                .entry(ep.content.clone())
                .or_insert((0.0, ep.clone()));
            e.0 += s;
            // appearing in BOTH channels is a strong signal Ã¢â‚¬â€ it already
            // accumulated both ranks above
        }

        let now = now_ms();
        let mut fused: Vec<(f32, Episode)> = scores.into_values().collect();
        for (score, ep) in &mut fused {
            let age_days = now.saturating_sub(ep.ts_ms) / 86_400_000;
            *score *= (-(age_days as f32) / 30.0).exp(); // ~30-day half-life-ish
        }
        fused.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(fused.into_iter().take(limit).map(|(_, e)| e).collect())
    }
}

