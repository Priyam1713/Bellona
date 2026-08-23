//! MCP — Model Context Protocol. Bellona speaks both directions:
//! as a **server** exposing its registry, as a **client** consuming foreign
//! servers. Tool descriptors map 1:1 with `forge::ToolSpec`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wire-shape of an MCP tool descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDescriptor {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Serving side: expose Bellona's arsenal to any MCP-capable host.
/// The gateway remains the only execution path — `call` routes through it.
#[async_trait]
pub trait McpServer: Send + Sync {
    async fn list_tools(&self) -> Result<Vec<McpToolDescriptor>, FoedusError>;
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value, FoedusError>;
}

/// Consuming side: a foreign MCP server becomes a source of arms.
#[async_trait]
pub trait McpClient: Send + Sync {
    /// Enumerate the foreign server's tools; import filters apply upstream.
    async fn list_tools(&self) -> Result<Vec<McpToolDescriptor>, FoedusError>;

    async fn call_tool(&self, name: &str, args: Value) -> Result<Value, FoedusError>;
}

#[derive(Debug, thiserror::Error)]
#[error("foedus/mcp: {0}")]
pub struct FoedusError(pub String);
