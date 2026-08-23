//! # memoria ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Tiered memory of the camp.
//!
//! Four tiers per doctrine:
//! - **nervi** ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â pinned, always-in-context vitals (goal + minimum proof)
//! - **tabella** ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â the wax tablet: decision records for the running task
//! - **archivum** ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â durable episodic/semantic/procedural store
//! - **somnium** ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â sleep-time consolidation distilling episodes into knowledge

pub mod archivum;
pub mod nervi;
pub mod sessions;
pub mod somnium;
pub mod sqlite;
pub mod tabella;

pub use archivum::{new_episode, ArchivumStore, Episode, InMemoryArchivum};
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
