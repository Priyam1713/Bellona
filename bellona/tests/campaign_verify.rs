//! The Spark proof: a campaign's receipts are independently verifiable by
//! anyone — and any lie is detectable.

use bellona::{assemble, BellonaConfig};
use praetorium::verify_export;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct Scripted(Mutex<VecDeque<bellum::ModelReply>>);
use std::collections::VecDeque;
impl Scripted {
    fn new(replies: Vec<bellum::ModelReply>) -> Arc<Self> {
        Arc::new(Scripted(Mutex::new(replies.into())))
    }
}
#[async_trait::async_trait]
impl bellum::ModelClient for Scripted {
    fn tier(&self) -> &'static str {
        "terra"
    }
    async fn complete(&self, _p: &str) -> Result<bellum::ModelReply, bellum::BellumError> {
        self.0
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| bellum::BellumError::Model("exhausted".into()))
    }
}

fn temp_ws(tag: &str) -> (PathBuf, Guard) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("bellona-verify-{tag}-{}-{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.clone(), Guard(dir))
}
struct Guard(PathBuf);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn signed_receipts_survive_third_party_audit() {
    let (ws, _g) = temp_ws("honest");
    std::fs::write(ws.join("plan.txt"), "march at dawn").unwrap();

    let cfg = BellonaConfig {
        workspace: ws.clone(),
        yolo: true,
        ..Default::default()
    };
    let a = assemble(&cfg).unwrap();

    // Arm identity enforcement BEFORE any effect flows.
    a.gateway.set_identity_enforced(true);

    let model = Scripted::new(vec![
        bellum::ModelReply {
            thought: "step".into(),
            tool_calls: vec![bellum::ToolCall {
                name: "read_file".into(),
                args: serde_json::json!({"path": "plan.txt"}),
            }],
            final_answer: None,
            cost_cents: 1,
        },
        bellum::ModelReply {
            thought: String::new(),
            tool_calls: vec![],
            final_answer: Some("dawn it is".into()),
            cost_cents: 1,
        },
    ]);

    let loop_ = bellum::WarLoop::new(
        a.gateway.clone(),
        a.registry.clone(),
        bellum::CascadeRouter::new(vec![model]),
        bellum::Aerarium::default(),
    )
    .with_auto_approver("t");

    let report = loop_
        .run(
            "read the plan",
            Box::new(bellum::ReActStrategy::new("r", 8)),
            None,
        )
        .await
        .unwrap();
    assert!(report.ok);

    // Export → hand to a skeptical third party.
    let export = a.gateway.export();
    let rep = verify_export(&export).unwrap();
    assert!(rep.chain_valid, "chain must hold");
    assert!(
        rep.signed_decisions >= 1,
        "identity was armed; decisions must be signed"
    );
    assert!(
        rep.signature_failures.is_empty(),
        "{:?}",
        rep.signature_failures
    );
    assert!(rep.fully_valid());
}

#[tokio::test]
async fn a_forged_row_is_caught_by_the_verifier() {
    let (ws, _g) = temp_ws("forged");
    let cfg = BellonaConfig {
        workspace: ws,
        yolo: true,
        ..Default::default()
    };
    let a = assemble(&cfg).unwrap();
    a.gateway.set_identity_enforced(true);

    let model = Scripted::new(vec![
        bellum::ModelReply {
            thought: String::new(),
            tool_calls: vec![bellum::ToolCall {
                name: "read_file".into(),
                args: serde_json::json!({"path": "x.txt"}),
            }],
            final_answer: None,
            cost_cents: 0,
        },
        bellum::ModelReply {
            thought: String::new(),
            tool_calls: vec![],
            final_answer: Some("ok".into()),
            cost_cents: 0,
        },
    ]);
    let loop_ = bellum::WarLoop::new(
        a.gateway.clone(),
        a.registry.clone(),
        bellum::CascadeRouter::new(vec![model]),
        bellum::Aerarium::default(),
    )
    .with_auto_approver("t");
    let _ = loop_
        .run("x", Box::new(bellum::ReActStrategy::new("x", 8)), None)
        .await;

    // Take the honest export, then FORGE a decision row with a fabricated
    // signature over different content.
    let mut export = a.gateway.export();
    let records = export["records"].as_array_mut().unwrap();
    if let Some(decision) = records.iter_mut().find(|r| r["kind"] == "decision") {
        // attacker edits params AFTER signing
        decision["payload"]["request"]["params"]["path"] = serde_json::json!("../../etc/shadow");
    }
    let rep = verify_export(&export).unwrap();
    assert!(
        !rep.signature_failures.is_empty() || !rep.chain_valid,
        "a forged receipt must fail verification"
    );
}
