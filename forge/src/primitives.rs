//! Kernel primitives: the vocabulary of decisions and effects.

use crate::id::{ActionId, AgentId, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Class of effect an action would produce. The gate keys policies on this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    FileRead,
    FileWrite,
    ShellExec,
    BrowserNavigate,
    BrowserAct,
    McpCall,
    MemoryWrite,
    ComponentPublish,
    Custom(String),
}

impl EffectKind {
    /// Conservative classification rule (OpenBot doctrine): anything not
    /// positively classified as a read is treated as a write.
    pub fn is_read(&self) -> bool {
        matches!(self, EffectKind::FileRead)
    }

    /// Canonical attribute string (no JSON quoting).
    pub fn as_attr(&self) -> String {
        match self {
            EffectKind::FileRead => "file_read".into(),
            EffectKind::FileWrite => "file_write".into(),
            EffectKind::ShellExec => "shell_exec".into(),
            EffectKind::BrowserNavigate => "browser_navigate".into(),
            EffectKind::BrowserAct => "browser_act".into(),
            EffectKind::McpCall => "mcp_call".into(),
            EffectKind::MemoryWrite => "memory_write".into(),
            EffectKind::ComponentPublish => "component_publish".into(),
            EffectKind::Custom(c) => c.clone(),
        }
    }
}

/// A resource the deployment knows about (server-held snapshot).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub uri: String,
    pub kind: String,
    /// Human-facing label used in audit rows.
    pub label: Option<String>,
}

/// One proposed effect awaiting judgment at the gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub id: ActionId,
    pub agent_id: AgentId,
    #[serde(default)]
    pub session_id: Option<SessionId>,
    pub tool_name: String,
    pub effect: EffectKind,
    /// Declared target URI; resolved against the registry snapshot.
    pub target_uri: String,
    /// Tool arguments (opaque to the kernel).
    pub params: serde_json::Value,
    /// The agent's stated intent — a policy attribute, never trusted alone.
    pub intent: String,
}

impl ActionRequest {
    pub fn new(agent_id: AgentId, tool_name: impl Into<String>, effect: EffectKind) -> Self {
        ActionRequest {
            id: ActionId::mint(),
            agent_id,
            session_id: None,
            tool_name: tool_name.into(),
            effect,
            target_uri: String::new(),
            params: serde_json::Value::Null,
            intent: String::new(),
        }
    }

    pub fn with_target(mut self, uri: impl Into<String>) -> Self {
        self.target_uri = uri.into();
        self
    }

    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = params;
        self
    }

    pub fn with_intent(mut self, intent: impl Into<String>) -> Self {
        self.intent = intent.into();
        self
    }
}

/// Typed attribute set handed to the policy engine. Keys follow the CEL
/// convention documented in `praetorium::lex`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyAttrs(pub BTreeMap<String, serde_json::Value>);

impl PolicyAttrs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(mut self, key: &str, value: serde_json::Value) -> Self {
        self.0.insert(key.to_string(), value);
        self
    }

    pub fn from_request(req: &ActionRequest, resolved: Option<&ResourceInfo>) -> Self {
        let mut attrs = PolicyAttrs::new()
            .set("tool.name", serde_json::json!(req.tool_name))
            .set("effect.kind", serde_json::json!(req.effect.as_attr()))
            .set("agent.id", serde_json::json!(req.agent_id.to_string()))
            .set("intent", serde_json::json!(req.intent))
            .set("target.uri", serde_json::json!(req.target_uri));
        if let Some(r) = resolved {
            attrs = attrs.set("resource.kind", serde_json::json!(r.kind));
        }
        attrs
    }
}

/// The verdict of the gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Decision {
    Allow { rule_id: String },
    Deny { rule_id: String, reason: String },
    RequireApproval { rule_id: String },
}

/// Terminal result of an executed effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Completed { result: serde_json::Value },
    Failed { error: String },
}
