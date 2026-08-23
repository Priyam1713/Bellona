//! Archivum — the durable store. Episodic, semantic, procedural knowledge
//! with a pluggable backend (Law III: local-first default here).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A stored memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub ts_ms: u64,
    /// "episodic" | "semantic" | "procedural" | "distilled"
    pub kind: String,
    pub content: String,
}

use crate::MemoriaError;

/// Backend contract. SQLite/FTS5, pgvector, temporal-graph and managed
/// adapters implement this; nothing above depends on which.
#[async_trait]
pub trait ArchivumStore: Send + Sync {
    async fn put(&self, episode: Episode) -> Result<(), crate::MemoriaError>;
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Episode>, MemoriaError>;
    async fn by_kind(&self, kind: &str) -> Result<Vec<Episode>, MemoriaError>;
}

/// Local-first default: keyword-scored recall over all episodes.
///
/// Deterministic, dependency-free, and honest about being simple. Vector or
/// graph backends slot in behind [`ArchivumStore`] without touching callers.
#[derive(Default)]
pub struct InMemoryArchivum {
    episodes: std::sync::RwLock<Vec<Episode>>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl InMemoryArchivum {
    pub fn new() -> Self {
        Self::default()
    }

    fn score(episode: &Episode, terms: &[String]) -> usize {
        let hay = episode.content.to_lowercase();
        let kind = episode.kind.to_lowercase();
        terms
            .iter()
            .map(|t| {
                let t = t.to_lowercase();
                let mut s = hay.matches(&t).count();
                if kind.contains(&t) {
                    s += 1;
                }
                s
            })
            .sum()
    }
}

#[async_trait]
impl ArchivumStore for InMemoryArchivum {
    async fn put(&self, episode: Episode) -> Result<(), crate::MemoriaError> {
        self.episodes
            .write()
            .map_err(|_| MemoriaError("archivum lock poisoned".into()))?
            .push(episode);
        Ok(())
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Episode>, crate::MemoriaError> {
        let store = self
            .episodes
            .read()
            .map_err(|_| MemoriaError("archivum lock poisoned".into()))?;
        let terms: Vec<String> = query.split_whitespace().map(|s| s.to_string()).collect();
        let mut scored: Vec<(usize, Episode)> = store
            .iter()
            .map(|e| (Self::score(e, &terms), e.clone()))
            .filter(|(s, _)| *s > 0)
            .collect();
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        Ok(scored.into_iter().take(limit).map(|(_, e)| e).collect())
    }

    async fn by_kind(&self, kind: &str) -> Result<Vec<Episode>, crate::MemoriaError> {
        let store = self
            .episodes
            .read()
            .map_err(|_| MemoriaError("archivum lock poisoned".into()))?;
        Ok(store.iter().filter(|e| e.kind == kind).cloned().collect())
    }
}

/// Convenience constructor used by Somnium.
pub fn new_episode(kind: &str, content: String) -> Episode {
    Episode {
        id: format!("ep_{:016x}", now_ms() ^ ((content.len() as u64) << 3)),
        ts_ms: now_ms(),
        kind: kind.to_string(),
        content,
    }
}
