//! V1.1 â€” declarative tool construction without trait boilerplate.
//!
//! A `SimpleTool` wraps an async handler closure; the spec is data. Used by
//! the arsenal (git/search/web) so every new capability is ~10 lines.

use crate::error::ForgeResult;
use crate::primitives::EffectKind;
use crate::tool::{Tool, ToolContext, ToolSpec};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type HandlerReturn = Pin<Box<dyn Future<Output = ForgeResult<Value>> + Send>>;

pub type Handler = Arc<dyn Fn(ToolContext, Value) -> HandlerReturn + Send + Sync>;

pub struct SimpleTool {
    spec: ToolSpec,
    handler: Handler,
}

impl SimpleTool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &str,
        description: &str,
        effect: EffectKind,
        read_only: bool,
        schema: Value,
        handler: impl Fn(ToolContext, Value) -> HandlerReturn + Send + Sync + 'static,
    ) -> Self {
        SimpleTool {
            spec: ToolSpec {
                name: name.to_string(),
                description: description.to_string(),
                effect,
                read_only,
                schema,
            },
            handler: Arc::new(handler),
        }
    }

    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

#[async_trait::async_trait]
impl Tool for SimpleTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> ForgeResult<Value> {
        (self.handler)(ctx.clone(), args).await
    }
}

/// Convenience: require a string argument or fail with a readable error.
pub fn need_str(args: &Value, key: &str) -> ForgeResult<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| crate::ForgeError::Other(format!("missing required string arg '{key}'")))
}

/// Convenience: optional string argument with default.
pub fn opt_str(args: &Value, key: &str, default: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}
