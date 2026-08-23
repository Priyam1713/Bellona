//! ACP — Agent Client Protocol adapters.
//!
//! Lets a Bellona agent sit inside workspace rooms built on ACP (Buzz-style
//! humans+agents channels) and lets foreign harnesses (Goose, Codex,
//! Claude Code) plug into Bellona-run spaces through the same shape.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// An inbound room event relevant to agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMention {
    pub channel: String,
    pub author: String,
    pub text: String,
    /// Thread to reply within, if any.
    #[serde(default)]
    pub thread: Option<String>,
}

/// Outbound agent contribution to a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomContribution {
    pub channel: String,
    #[serde(default)]
    pub thread: Option<String>,
    pub kind: ContributionKind,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionKind {
    Message,
    Patch,
    Review,
    Status,
}

/// The adapter contract. Implementations hold the room credentials —
/// never the model (Law V).
#[async_trait]
pub trait AcpAdapter: Send + Sync {
    /// Subscribe handle: implementations poll or push mentions in.
    async fn next_mention(&self) -> Result<Option<RoomMention>, crate::FoedusError>;
    async fn contribute(&self, c: RoomContribution) -> Result<(), crate::FoedusError>;
}
