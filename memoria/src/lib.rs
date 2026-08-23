//! # memoria ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â Tiered memory of the camp.
//!
//! Four tiers per doctrine:
//! - **nervi** ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â pinned, always-in-context vitals (goal + minimum proof)
//! - **tabella** ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â the wax tablet: decision records for the running task
//! - **archivum** ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â durable episodic/semantic/procedural store
//! - **somnium** ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â sleep-time consolidation distilling episodes into knowledge

pub mod archivum;
pub mod embed;
pub mod hybrid;
pub mod nervi;
pub mod sessions;
pub mod somnium;
pub mod sqlite;
pub mod tabella;

pub use archivum::{new_episode, ArchivumStore, Episode, InMemoryArchivum};
pub use embed::{cosine, Embedder, HashEmbedder};
pub use hybrid::{HybridRecall, SqliteVectors};
pub use nervi::Nervi;
pub use sessions::SqliteSessionStore;
pub use somnium::{Consolidator, DistilledKnowledge, HeuristicConsolidator, SleepDaemon};
pub use sqlite::{IdempotencyLedger, SqliteArchivum};
pub use tabella::Tabella;

use serde::{Deserialize, Serialize};

/// Memory-layer error.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct MemoriaError(pub String);

/// A distilled fact or skill produced by Somnium.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Distillate {
    Fact(String),
    Skill(String),
}
