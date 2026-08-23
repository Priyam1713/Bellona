//! Model access: the CascadeRouter sends each phase of thought to the right
//! tier — frontier for planning, cheap models for mechanical work.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// One requested tool invocation from a model reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    #[serde(default)]
    pub args: serde_json::Value,
}

/// A model's turn.
#[derive(Debug, Clone)]
pub struct ModelReply {
    /// Reasoning text (may be empty).
    pub thought: String,
    /// Zero or more tool calls.
    pub tool_calls: Vec<ToolCall>,
    /// Set when the model believes the goal is met.
    pub final_answer: Option<String>,
    /// Cost of this call in cents (governed by the Aerarium).
    pub cost_cents: u64,
}

/// Which kind of thinking is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Plan,
    Execute,
    Summarize,
}

/// Any model provider. Adapters for OpenAI-compatible APIs, Anthropic and
/// Ollama implement this; nothing in Bellum knows their names.
#[async_trait]
pub trait ModelClient: Send + Sync {
    fn tier(&self) -> &'static str;
    async fn complete(&self, prompt: &str) -> Result<ModelReply, BellumError>;
}

#[derive(Debug, thiserror::Error)]
pub enum BellumError {
    #[error("model error: {0}")]
    Model(String),

    #[error("strategy error: {0}")]
    Strategy(String),

    #[error("gate error: {0}")]
    Gate(#[from] praetorium::PraetoriumError),
}

/// Phase → preferred tier mapping, then first-match across enrolled clients.
#[derive(Clone)]
pub struct CascadeRouter {
    clients: Vec<Arc<dyn ModelClient>>,
    plan_tier: String,
    execute_tier: String,
    summarize_tier: String,
}

impl CascadeRouter {
    pub fn new(clients: Vec<Arc<dyn ModelClient>>) -> Self {
        CascadeRouter {
            clients,
            plan_tier: "sol".into(),
            execute_tier: "terra".into(),
            summarize_tier: "luna".into(),
        }
    }

    pub fn with_tiers(mut self, plan: &str, execute: &str, summarize: &str) -> Self {
        self.plan_tier = plan.into();
        self.execute_tier = execute.into();
        self.summarize_tier = summarize.into();
        self
    }

    /// Resolve a client for the phase; falls back to the first enrolled.
    pub fn route(&self, phase: Phase) -> Option<Arc<dyn ModelClient>> {
        let want = match phase {
            Phase::Plan => &self.plan_tier,
            Phase::Execute => &self.execute_tier,
            Phase::Summarize => &self.summarize_tier,
        };
        self.clients
            .iter()
            .find(|c| c.tier() == want)
            .cloned()
            .or_else(|| self.clients.first().cloned())
    }

    pub fn len(&self) -> usize {
        self.clients.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }
}
