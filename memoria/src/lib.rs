//! # memoria Ã¢â‚¬â€ Tiered memory of the camp.
//!
//! Four tiers per doctrine:
//! - **nervi** Ã¢â‚¬â€ pinned, always-in-context vitals (goal + minimum proof)
//! - **tabella** Ã¢â‚¬â€ the wax tablet: decision records for the running task
//! - **archivum** Ã¢â‚¬â€ durable episodic/semantic/procedural store
//! - **somnium** Ã¢â‚¬â€ sleep-time consolidation distilling episodes into knowledge

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
pub use sqlite::SqliteArchivum;
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
