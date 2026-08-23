//! Full-campaign integration: a scripted model drives the REAL file tools
//! through the REAL gate against a REAL temp workspace.

use async_trait::async_trait;
use bellona::{assemble, BellonaConfig};
use bellum::{Aerarium, BellumError, ModelClient, ModelReply, ReActStrategy, ToolCall, WarLoop};
use std::sync::{Arc, Mutex};

struct Scripted {
    steps: std::sync::Mutex<VecDeque<ModelReply>>,
}

use std::collections::VecDeque;
impl Scripted {
    fn new(replies: Vec<ModelReply>) -> Self {
        Scripted {
            steps: Mutex::new(VecDeque::from(replies)),
        }
    }
}
#[async_trait]
impl ModelClient for Scripted {
    fn tier(&self) -> &'static str {
        "terra"
    }
    async fn complete(&self, _p: &str) -> Result<ModelReply, BellumError> {
        self.steps
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| BellumError::Model("script exhausted".into()))
    }
}

fn reply_tool(name: &str, args: serde_json::Value) -> ModelReply {
    ModelReply {
        thought: "step".into(),
        tool_calls: vec![ToolCall {
            name: name.into(),
            args,
        }],
        final_answer: None,
        cost_cents: 1,
    }
}
fn reply_answer(a: &str) -> ModelReply {
    ModelReply {
        thought: String::new(),
        tool_calls: vec![],
        final_answer: Some(a.into()),
        cost_cents: 1,
    }
}

fn temp_ws() -> (std::path::PathBuf, PathBufGuard) {
    let dir = std::env::temp_dir().join(format!(
        "bellona-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.clone(), PathBufGuard(dir))
}
struct PathBufGuard(std::path::PathBuf);
impl Drop for PathBufGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn campaign_writes_a_file_and_finishes() {
    let (_ws, _guard) = temp_ws();
    let cfg = BellonaConfig {
        workspace: _ws.clone(),
        yolo: true,
        ..Default::default()
    };
    let assembled = assemble(&cfg).unwrap();

    let model = Scripted::new(vec![
        reply_tool("list_files", serde_json::json!({})),
        reply_tool(
            "write_file",
            serde_json::json!({"path":"reports/victory.md","content":"we marched"}),
        ),
        reply_tool(
            "read_file",
            serde_json::json!({"path":"reports/victory.md"}),
        ),
        reply_answer("the report reads: we marched"),
    ]);

    let loop_ = WarLoop::new(
        assembled.gateway.clone(),
        assembled.registry.clone(),
        bellum::CascadeRouter::new(vec![Arc::new(model) as Arc<dyn ModelClient>]),
        Aerarium::default(),
    )
    .with_auto_approver("test-operator");

    let report = loop_
        .run(
            "write and verify reports/victory.md",
            Box::new(ReActStrategy::new("g", 12)),
            None,
        )
        .await
        .unwrap();

    assert!(report.ok, "breaker: {:?}", report.breaker);
    assert_eq!(report.answer, "the report reads: we marched");
    assert!(report.steps_used >= 3);

    // The file REALLY exists on disk.
    let written = std::fs::read_to_string(_ws.join("reports").join("victory.md")).unwrap();
    assert_eq!(written, "we marched");

    // And the ledger holds decision + settlement rows. In yolo mode writes
    // are allowed outright (audited, not ticketed).
    let recs = assembled.gateway.ledger_snapshot();
    assert!(recs.iter().any(|r| r.kind == "decision"));
    assert!(recs.iter().any(|r| r.kind == "settlement"));
    assert!(!recs.iter().any(|r| r.kind == "approval_granted"));
    assert!(assembled.gateway.verify_ledger());
}

#[tokio::test]
async fn workspace_escape_is_refused_by_the_tool_itself() {
    let (_ws, _guard) = temp_ws();
    let cfg = BellonaConfig {
        workspace: _ws.clone(),
        yolo: true,
        ..Default::default()
    };
    let assembled = assemble(&cfg).unwrap();
    let model = Scripted::new(vec![
        // Attempt to escape via absolute path outside workspace.
        reply_tool(
            "write_file",
            serde_json::json!({"path": "C:/Windows/bellona-escape.txt", "content": "nope"}),
        ),
        reply_answer("refused"),
    ]);
    let loop_ = WarLoop::new(
        assembled.gateway.clone(),
        assembled.registry.clone(),
        bellum::CascadeRouter::new(vec![Arc::new(model) as Arc<dyn ModelClient>]),
        Aerarium::default(),
    )
    .with_auto_approver("t");
    let report = loop_
        .run("escape", Box::new(ReActStrategy::new("e", 6)), None)
        .await
        .unwrap();
    assert!(report.ok);
    assert!(!std::path::Path::new("C:/Windows/bellona-escape.txt").exists());
}
