//! # praetorium — The Praetorian Gate
//!
//! Nothing acts without passing through here (Law IV):
//! **resolve ▸ policy ▸ audit ▸ act**, fail-closed at every stage.

pub mod annales;
pub mod custos;
pub mod error;
pub mod lex;
pub mod verify;
pub mod vexillum;

pub use annales::{Annales, LedgerRecord};
pub use custos::{
    ApprovalTicket, CustosGateway, EffectExecutor, GateOutcome, SnapshotResolver, TargetResolver,
    VetoGuard,
};
pub use error::PraetoriumError;
pub use lex::{Lex, RuleEffect, RuleSpec};
pub use verify::{verify_export, VerifyReport};
pub use vexillum::{IdentityRecord, VexillumKeypair, VexillumService};
