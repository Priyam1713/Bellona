//! # foedus — the treaty.
//!
//! Bellona agents are citizens of every realm: MCP (down to tools),
//! A2A (across organizations), AG-UI (up to surfaces), ACP (into rooms).

pub mod a2a;
pub mod acp;
pub mod agui;
pub mod mcp;

pub use a2a::{A2aDelegatee, A2aDelegator, AgentCard, TaskRequest, TaskResponse};
pub use acp::{AcpAdapter, ContributionKind, RoomContribution, RoomMention};
pub use agui::{from_bus, AgUiEmitter, AgUiEvent, AgUiFanout};
pub use mcp::{FoedusError, McpClient, McpServer, McpToolDescriptor};

/// Protocol versions Bellona v0.1 speaks.
pub const PROTOCOL_VERSIONS: &[&str] = &["mcp/2025-11-25", "a2a/1", "ag-ui/1", "acp/1"];
