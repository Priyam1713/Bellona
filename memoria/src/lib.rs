//! # memoria — Tiered memory of the camp.
//!
//! Four tiers per doctrine:
//! - **nervi** — pinned, always-in-context vitals (goal + minimum proof)
//! - **tabella** — the wax tablet: decision records for the running task
//! - **archivum** — durable episodic/semantic/procedural store
//! - **somnium** — sleep-time consolidation distilling episodes into knowledge

pub mod archivum;
pub mod nervi;
pub mod somnium;
pub mod tabella;

pub use archivum::{new_episode, ArchivumStore, Episode, InMemoryArchivum};
pub use nervi::Nervi;
pub use somnium::{Consolidator, DistilledKnowledge, HeuristicConsolidator, SleepDaemon};
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
