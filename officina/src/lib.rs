//! # officina — the workshop where agents forge tools,
//! # and the Ludus where only survivors earn legionary rank.

pub mod forger;
pub mod ludus;

pub use forger::{ForgedTool, ScriptLang};
pub use ludus::{promote, run_battery, BatteryCase, LudusVerdict, LUDUS_MARKER};

/// Workshop errors.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct OfficinaError(pub String);
