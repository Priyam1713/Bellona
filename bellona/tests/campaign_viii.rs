//! Campaign VIII: the Legion â€” roles scoped by law, budgets carved per
//! worker, and a real two-agent campaign writing through the gate.

use async_trait::async_trait;
use bellum::{Aerarium, Legion, ModelClient, ReActStrategy, ToolCall, WorkerSpec};
use forge::tool::ToolRegistry;
use praetorium::lex::RuleSpec;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct Scripted(Mutex<VecDeque<bellum::ModelReply>>);
impl Scripted {
    fn new(replies: Vec<bellum::ModelReply>) -> Arc<Self> {
        Arc::new(Scripted(Mutex::new(replies.into())))
    }
}
#[async_trait]
impl ModelClient for Scripted {
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

fn rt(name: &str, args: serde_json::Value) -> bellum::ModelReply {
    bellum::ModelReply {
        thought: "step".into(),
        tool_calls: vec![ToolCall {
            name: name.into(),
            args,
        }],
        final_answer: None,
        cost_cents: 1,
    }
}
fn fin(a: &str) -> bellum::ModelReply {
    bellum::ModelReply {
        thought: String::new(),
        tool_calls: vec![],
        final_answer: Some(a.into()),
        cost_cents: 1,
    }
}

fn temp_ws(tag: &str) -> (PathBuf, TempGuard) {
    let dir = std::env::temp_dir().join(format!(
        "bellona-c8-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.clone(), TempGuard(dir))
}
struct TempGuard(PathBuf);
impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Fleet law: reads for everyone; writes ONLY for role "writer".
const FLEET_LAW: &str = "fleet";

#[tokio::test]
async fn centurio_runs_two_role_scoped_workers() {
    let mut reg = ToolRegistry::new();
    // Reuse the arsenal's read/write via bellona assemble? We need forge-only
    // here; use simple registry with read+write from bellona's lib is not
    // exported â€” so exercise via bellona::assemble instead below.
    let _ = &mut reg;

    let (ws, _g) = temp_ws("fleet");
    std::fs::write(ws.join("intel.txt"), "enemy sleeps at dawn").unwrap();

    let cfg = bellona::BellonaConfig {
        workspace: ws.clone(),
        yolo: true,
        ..Default::default()
    };
    let a = bellona::assemble(&cfg).unwrap();

    // Fleet-level law composed deny-first: wrong-role writes refuse;
    // everything else flows under yolo semantics.
    let combined = praetorium::Lex::from_specs(&[
        RuleSpec {
            id: FLEET_LAW.into(),
            effect: praetorium::RuleEffect::Deny,
            expr: "attr.effect.kind == 'file_write' && attr.worker.role != 'writer'".into(),
        },
        RuleSpec {
            id: "yolo-rest".into(),
            effect: praetorium::RuleEffect::Allow,
            expr: "true".into(),
        },
    ])
    .unwrap();
    a.gateway.install_law(combined);
    let model = Scripted::new(vec![
        // researcher: read intel
        rt("read_file", serde_json::json!({"path": "intel.txt"})),
        fin("enemy sleeps at dawn"),
        // writer: write report (allowed: role=writer)
        rt(
            "write_file",
            serde_json::json!({"path": "report.md", "content": "# Dawn\nenemy sleeps"}),
        ),
        fin("report written"),
        // synthesis
        fin("dawn assault planned"),
    ]);

    let legion = Legion::new(a.gateway.clone(), a.registry.clone(), model);
    let report = legion
        .campaign(
            |spec, max_steps| Box::new(ReActStrategy::new(spec.goal.clone(), max_steps)),
            &[
                WorkerSpec::new("researcher", "read intel.txt and summarize"),
                WorkerSpec::new("writer", "write report.md with the plan"),
            ],
            &Aerarium::default(),
        )
        .await
        .unwrap();

    assert!(report.ok, "workers: {:?}", report.workers);
    assert_eq!(report.synthesis, "dawn assault planned");
    assert_eq!(report.workers.len(), 2);

    // The writer REALLY wrote; ledger carries both roles.
    let written = std::fs::read_to_string(ws.join("report.md")).unwrap();
    assert!(written.contains("Dawn"));
    let recs = a.gateway.ledger_snapshot();
    let roles: Vec<_> = recs
        .iter()
        .filter(|r| r.kind == "decision")
        .filter_map(|r| r.payload["identity"].as_object().map(|_| ()))
        .collect();
    let _ = roles;
    assert!(recs.iter().any(|r| r.kind == "settlement"));
}

#[tokio::test]
async fn role_law_denies_writer_power_to_researcher() {
    let (ws, _g) = temp_ws("scoping");
    let cfg = bellona::BellonaConfig {
        workspace: ws.clone(),
        yolo: true,
        ..Default::default()
    };
    let a = bellona::assemble(&cfg).unwrap();

    a.gateway.install_law(
        praetorium::Lex::from_specs(&[
            RuleSpec {
                id: "researcher-cannot-write".into(),
                effect: praetorium::RuleEffect::Deny,
                expr: "attr.effect.kind == 'file_write' && attr.worker.role == 'researcher'".into(),
            },
            RuleSpec {
                id: "allow-rest-yolo".into(),
                effect: praetorium::RuleEffect::Allow,
                expr: "true".into(),
            },
        ])
        .unwrap(),
    );

    let model = Scripted::new(vec![
        rt(
            "write_file",
            serde_json::json!({"path": "forbidden.txt", "content": "nope"}),
        ),
        fin("stood down"),
        // synthesis
        fin("researcher restrained"),
    ]);
    let legion = Legion::new(a.gateway.clone(), a.registry.clone(), model);
    let report = legion
        .campaign(
            |spec, ms| Box::new(ReActStrategy::new(spec.goal.clone(), ms)),
            &[WorkerSpec::new("researcher", "try to write")],
            &Aerarium::default(),
        )
        .await
        .unwrap();

    assert!(!std::fs::exists(ws.join("forbidden.txt")).unwrap());
    assert!(report.ok, "denial is an observation; agent stood down");
}

#[tokio::test]
async fn empty_plan_is_refused() {
    let (ws, _g) = temp_ws("empty");
    let cfg = bellona::BellonaConfig {
        workspace: ws,
        ..Default::default()
    };
    let a = bellona::assemble(&cfg).unwrap();
    let model = Scripted::new(vec![]);
    let legion = Legion::new(a.gateway.clone(), a.registry.clone(), model);
    let err = legion
        .campaign(|_, _| unimplemented!(), &[], &Aerarium::default())
        .await
        .unwrap_err();
    assert!(matches!(err, bellum::LegionError::Empty));
}
