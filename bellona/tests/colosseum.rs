use bellona::colosseum::{run_suite, EchoModel};
use bellona::BellonaConfig;
use std::sync::Arc;
use vigiles::{Gate, SuiteFile};

fn temp_ws() -> (std::path::PathBuf, TempGuard) {
    let dir = std::env::temp_dir().join(format!(
        "bellona-colosseum-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let g = TempGuard(dir.clone());
    (dir, g)
}
struct TempGuard(std::path::PathBuf);
impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn offline_seed_suite_passes_fully() {
    let (_ws, _g) = temp_ws();
    let cfg = BellonaConfig {
        workspace: _ws.clone(),
        yolo: true,
        ..Default::default()
    };
    let suite: SuiteFile = serde_json::from_str(
        r#"{
        "name": "seed-test",
        "cases": [
            {"id": "a", "task": "say hail bellona", "trials": 2,
             "verifier": {"verifier": "exact_output", "expected": "hail bellona"}},
            {"id": "b", "task": "write notes/x.md :: content-here", "trials": 1,
             "verifier": {"verifier": "starts_with", "prefix": "wrote"}}
        ]}"#,
    )
    .unwrap();

    let out = run_suite(
        &cfg,
        suite,
        Arc::new(EchoModel::new()),
        Gate {
            min_pass_at_k: 1.0,
            max_cost_cents: 100,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        out.report.pass_at_k,
        1.0,
        "{:?}",
        out.report
            .cases
            .iter()
            .map(|c| (c.case_id.as_str(), c.trial_flags.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(out.verdict.exit_code(), 0);

    // The tool-write case REALLY wrote through the gate.
    let written = std::fs::read_to_string(_ws.join("notes").join("x.md")).unwrap();
    assert_eq!(written, "content-here");
}

#[tokio::test]
async fn failing_verifier_drives_gate_to_exit_one() {
    let (_ws, _g) = temp_ws();
    let cfg = BellonaConfig {
        workspace: _ws.clone(),
        yolo: true,
        ..Default::default()
    };
    let suite: SuiteFile = serde_json::from_str(
        r#"{
        "name": "doomed",
        "cases": [
            {"id": "wrong", "task": "say the wrong thing", "trials": 2,
             "verifier": {"verifier": "exact_output", "expected": "the right thing"}}
        ]}"#,
    )
    .unwrap();

    let out = run_suite(
        &cfg,
        suite,
        Arc::new(EchoModel::new()),
        Gate {
            min_pass_at_k: 0.5,
            max_cost_cents: 100,
        },
    )
    .await
    .unwrap();
    assert!(out.report.pass_at_k < 0.5);
    assert_eq!(out.verdict.exit_code(), 1);
}
