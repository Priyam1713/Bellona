//! Tools: declared once, exposed deliberately (registration ≠ exposure).

use crate::error::{ForgeError, ForgeResult};
use crate::id::AgentId;
use crate::primitives::EffectKind;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The static contract of a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub effect: EffectKind,
    /// Positively classified reads only. Everything else is a write.
    pub read_only: bool,
    /// JSON-schema of accepted arguments.
    #[serde(default)]
    pub schema: Value,
}

/// Runtime context handed to a tool on execution. Tools never receive the
/// deployment environment — only this scrubbed view (Law IV).
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub agent_id: AgentId,
    /// Workspace root the tool may touch; nothing outside is addressable.
    pub workspace: std::path::PathBuf,
}

/// A capability. Implementations must be deterministic about their own
/// failures and must not spawn effects outside `ToolContext::workspace`.
#[async_trait]
pub trait Tool: Send + Sync {
    fn spec(&self) -> &ToolSpec;

    async fn execute(&self, ctx: &ToolContext, args: Value) -> ForgeResult<Value>;
}

/// Registry with the Hermes-borrowed separation: **registration** makes a
/// tool exist; **exposure** makes it visible to agents. A registered but
/// unexposed tool cannot be invoked through normal paths.
#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
    exposed: BTreeMap<String, bool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool. It exists but is NOT yet exposed.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.spec().name.clone();
        self.tools.insert(name.clone(), tool);
        self.exposed.insert(name, false);
    }

    /// Expose (or hide) a registered tool to agent-visible surfaces.
    pub fn set_exposed(&mut self, name: &str, exposed: bool) -> ForgeResult<()> {
        if !self.tools.contains_key(name) {
            return Err(ForgeError::ToolNotFound(name.to_string()));
        }
        self.exposed.insert(name.to_string(), exposed);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let exposed = *self.exposed.get(name)?;
        if !exposed {
            return None;
        }
        self.tools.get(name).cloned()
    }

    /// Specs of all *exposed* tools — what agents are told about.
    pub fn exposed_specs(&self) -> Vec<ToolSpec> {
        self.tools
            .iter()
            .filter(|(n, _)| self.exposed.get(*n).copied().unwrap_or(false))
            .map(|(_, t)| t.spec().clone())
            .collect()
    }

    pub fn len_registered(&self) -> usize {
        self.tools.len()
    }
}
