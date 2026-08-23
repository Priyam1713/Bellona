//! End-to-end war-loop tests: scripted models, real gate, real registry.

use async_trait::async_trait;
use bellum::{
    Aerarium, BellumError, CascadeRouter, ModelClient, ModelReply, ReActStrategy, ToolCall,
};
use forge::primitives::{ActionRequest, ResourceInfo};
use forge::tool::{Tool, ToolContext, ToolRegistry, ToolSpec};
use praetorium::custos::{CustosGateway, EffectExecutor, SnapshotResolver};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// ---------- doubles ----------

struct ScriptedModel {
    tier: &'static str,
    script: Mutex<VecDeque<ModelReply>>,
}

impl ScriptedModel {
    fn new(tier: &'static str, replies: Vec<ModelReply>) -> Self {
        ScriptedModel {
            tier,
            script: Mutex::new(VecDeque::from(replies)),
        }
    }
}

#[async_trait]
impl ModelClient for ScriptedModel {
    fn tier(&self) -> &'static str {
        self.tier
    }
    async fn complete(&self, _prompt: &str) -> Result<ModelReply, BellumError> {
        self.script
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| BellumError::Model("script exhausted".into()))
    }
}

struct ReadNotesTool;

#[async_trait]
impl Tool for ReadNotesTool {
    fn spec(&self) -> &ToolSpec {
        static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
        SPEC.get_or_init(|| ToolSpec {
            name: "read_notes".into(),
            description: "reads notes.txt".into(),
            effect: forge::primitives::EffectKind::FileRead,
            read_only: true,
            schema: serde_json::json!({}),
        })
    }
    async fn execute(
        &self,
        _ctx: &ToolContext,
        _args: serde_json::Value,
    ) -> forge::ForgeResult<serde_json::Value> {
        Ok(serde_json::json!({ "content": "the launch code is 0000" }))
    }
}

/// Bridge executor: performs effects by invoking registered tools inside a
/// scrubbed context Ã¢â‚¬â€ the production Castra driver does this in a sandbox.
struct RegistryExecutor {
    registry: Arc<ToolRegistry>,
}

#[async_trait]
impl EffectExecutor for RegistryExecutor {
    async fn perform(
        &self,
        req: &ActionRequest,
        _resolved: &ResourceInfo,
        ws: &std::path::Path,
    ) -> Result<serde_json::Value, String> {
        let tool = self
            .registry
            .get(&req.tool_name)
            .ok_or_else(|| format!("tool '{}' missing", req.tool_name))?;
        let ctx = ToolContext {
            agent_id: req.agent_id.clone(),
            workspace: ws.to_path_buf(),
        };
        tool.execute(&ctx, req.params.clone())
            .await
            .map_err(|e| e.to_string())
    }
}

fn build_gateway(
    registry: Arc<ToolRegistry>,
    law: praetorium::Lex,
) -> Arc<CustosGateway<SnapshotResolver, RegistryExecutor>> {
    let mut resolver = SnapshotResolver::new();
    resolver.upsert(ResourceInfo {
        uri: "file://workspace".into(),
        kind: "file".into(),
        label: None,
    });
    let gw = CustosGateway::new(
        resolver,
        RegistryExecutor {
            registry: registry.clone(),
        },
        PathBuf::from("."),
    );
    gw.install_law(law);
    Arc::new(gw)
}

fn reply_thought_tool(thought: &str, tool: &str, args: serde_json::Value) -> ModelReply {
    ModelReply {
        thought: thought.into(),
        tool_calls: vec![ToolCall {
            name: tool.into(),
            args,
        }],
        final_answer: None,
        cost_cents: 1,
    }
}

fn reply_answer(ans: &str) -> ModelReply {
    ModelReply {
        thought: String::new(),
        tool_calls: vec![],
        final_answer: Some(ans.into()),
        cost_cents: 1,
    }
}

// ---------- the wars ----------

#[tokio::test]
async fn react_campaign_reads_then_finishes() {
    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(ReadNotesTool));
    reg.set_exposed("read_notes", true).unwrap();
    let registry = Arc::new(reg);

    let law = praetorium::Lex::from_specs(&[praetorium::RuleSpec {
        id: "allow-reads".into(),
        effect: praetorium::RuleEffect::Allow,
        expr: "attr.effect.kind == 'file_read'".into(),
    }])
    .unwrap();

    let gw = build_gateway(registry.clone(), law);
    let router = CascadeRouter::new(vec![Arc::new(ScriptedModel::new(
        "terra",
        vec![
            reply_thought_tool(
                "need notes",
                "read_notes",
                serde_json::json!({"path":"notes.txt"}),
            ),
            reply_answer("launch code is 0000"),
        ],
    ))]);

    let loop_ = bellum::WarLoop::new(gw, registry, router, Aerarium::default());
    let report = loop_
        .run(
            "find the launch code",
            Box::new(ReActStrategy::new("find code", 10)),
            None,
        )
        .await
        .unwrap();

    assert!(report.ok);
    assert_eq!(report.answer, "launch code is 0000");
    assert!(report.breaker.is_none());
}

#[tokio::test]
async fn denial_is_observation_not_crash() {
    // Only writes are governed here; reads default-deny too (empty allow set).
    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(ReadNotesTool));
    reg.set_exposed("read_notes", true).unwrap();
    let registry = Arc::new(reg);

    let law = praetorium::Lex::empty(); // nothing permitted
    let gw = build_gateway(registry.clone(), law);

    let router = CascadeRouter::new(vec![Arc::new(ScriptedModel::new(
        "terra",
        vec![
            reply_thought_tool(
                "try read",
                "read_notes",
                serde_json::json!({"path":"notes.txt"}),
            ),
            // After seeing DENIED observation, model concedes.
            reply_answer("access denied, standing down"),
        ],
    ))]);

    let loop_ = bellum::WarLoop::new(gw, registry, router, Aerarium::default());
    let report = loop_
        .run("exfiltrate", Box::new(ReActStrategy::new("x", 5)), None)
        .await
        .unwrap();

    assert!(report.ok);
    assert_eq!(report.answer, "access denied, standing down");

    // And the refusal is in the ledger.
    // (gateway dropped with loop; verified via the earlier laws suite.)
}

#[tokio::test]
async fn no_progress_breaker_halts_repetition() {
    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(ReadNotesTool));
    reg.set_exposed("read_notes", true).unwrap();
    let registry = Arc::new(reg);

    let law = praetorium::Lex::from_specs(&[praetorium::RuleSpec {
        id: "allow-reads".into(),
        effect: praetorium::RuleEffect::Allow,
        expr: "attr.effect.kind == 'file_read'".into(),
    }])
    .unwrap();

    let gw = build_gateway(registry.clone(), law);

    // The model loops forever on the same callÃ¢â‚¬Â¦
    let scripted = (0..8)
        .map(|_| {
            reply_thought_tool(
                "again",
                "read_notes",
                serde_json::json!({"path":"same.txt"}),
            )
        })
        .collect::<Vec<_>>();
    let router = CascadeRouter::new(vec![Arc::new(ScriptedModel::new("terra", scripted))]);

    let loop_ = bellum::WarLoop::new(gw, registry, router, Aerarium::default());
    let report = loop_
        .run("stuck", Box::new(ReActStrategy::new("s", 50)), None)
        .await
        .unwrap();

    assert!(!report.ok);
    assert!(report.breaker.unwrap().contains("no-progress"));
}
