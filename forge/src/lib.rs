//! # forge — The Bellona Kernel
//!
//! Seven primitives, nothing more (Law I: lean core, iron edges):
//! `loop`, `context`, `tool`, `session`, `policy`, `channel`, `memory`.
//!
//! Strategy intelligence lives in plugins (`bellum`, …), never here.

pub mod context;
pub mod error;
pub mod event;
pub mod id;
pub mod primitives;
pub mod session;
pub mod tool;

pub use context::{Block, ContextWindow, LineageRef};
pub use error::{ForgeError, ForgeResult};
pub use event::{BusEvent, EventBus};
pub use id::{ActionId, AgentId, Id, RunId, SessionId};
pub use primitives::{ActionRequest, Decision, EffectKind, Outcome, PolicyAttrs, ResourceInfo};
pub use session::{Session, SessionStore};
pub use tool::{Tool, ToolContext, ToolRegistry, ToolSpec};

/// Kernel plugin-API contract version (Law II).
///
/// Bumped only on breaking change of any public kernel trait. Plugin authors
/// compile against this constant; hosts refuse mismatched major versions.
pub const PLUGIN_API_VERSION: u32 = 1;
