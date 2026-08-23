//! Integration tests: the Seven Laws enforced by the gate.
//!
//! If any of these fail, Bellona is broken in the way that matters.

use forge::id::AgentId;
use forge::primitives::{ActionRequest, Decision, EffectKind, Outcome, PolicyAttrs, ResourceInfo};
use praetorium::custos::{CustosGateway, EffectExecutor, GateOutcome, SnapshotResolver};
use praetorium::lex::{Lex, RuleEffect, RuleSpec, RULE_BROKEN, RULE_DEFAULT_DENY};
use praetorium::vexillum::{VexillumKeypair, VexillumService};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn agent() -> AgentId {
    AgentId::mint()
}

/// Executor that appends to a shared journal so tests can prove ordering.
struct JournalExecutor {
    journal: Arc<Mutex<Vec<String>>>,
    fail_tools: Vec<String>,
}

impl JournalExecutor {
    fn new(fail_tools: Vec<String>) -> Self {
        JournalExecutor {
            journal: Arc::new(Mutex::new(Vec::new())),
            fail_tools,
        }
    }
}

#[async_trait::async_trait]
impl EffectExecutor for JournalExecutor {
    async fn perform(
        &self,
        req: &ActionRequest,
        _resolved: &ResourceInfo,
        _ws: &std::path::Path,
    ) -> Result<serde_json::Value, String> {
        // Snapshot the decision-ledger state AT EXECUTION TIME. Law IV says
        // the decision row must already exist before we are ever invoked.
        self.journal
            .lock()
            .unwrap()
            .push(format!("exec:{}", req.tool_name));
        if self.fail_tools.contains(&req.tool_name) {
            Err(format!("tool '{}' exploded", req.tool_name))
        } else {
            Ok(serde_json::json!({ "done": true, "tool": req.tool_name }))
        }
    }
}

fn gateway_with(
    specs: Vec<RuleSpec>,
    fail_tools: Vec<String>,
) -> (
    CustosGateway<SnapshotResolver, JournalExecutor>,
    Arc<Mutex<Vec<String>>>,
) {
    let mut resolver = SnapshotResolver::new();
    // One workspace root covers every hierarchical target in these tests.
    resolver.upsert(ResourceInfo {
        uri: "file://workspace".into(),
        kind: "file".into(),
        label: None,
    });
    let exec = JournalExecutor::new(fail_tools);
    let journal = exec.journal.clone();
    let gw = CustosGateway::new(resolver, exec, PathBuf::from("."));
    let lex = Lex::from_specs(&specs).expect("law must compile");
    gw.install_law(lex);
    (gw, journal)
}

// Minimal block-on helper so tests stay dependency-light.
fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn read_rule(tool: &str) -> RuleSpec {
    RuleSpec {
        id: format!("allow-read-{tool}"),
        effect: RuleEffect::Allow,
        expr: format!("attr.tool.name == '{tool}' && attr.effect.kind == 'file_read'"),
    }
}

fn write_action(tool: &str) -> ActionRequest {
    ActionRequest::new(agent(), tool, EffectKind::FileWrite)
        .with_target("file://workspace/notes.txt")
        .with_intent("update notes")
        .with_params(serde_json::json!({ "path": "notes.txt" }))
}

fn read_action(tool: &str) -> ActionRequest {
    ActionRequest::new(agent(), tool, EffectKind::FileRead)
        .with_target("file://workspace/notes.txt")
        .with_intent("read notes")
        .with_params(serde_json::json!({ "path": "notes.txt" }))
}

#[test]
fn law_iv_empty_law_permits_nothing() {
    let (gw, _) = gateway_with(vec![], vec![]);
    let out = futures_block_on(gw.submit(read_action("reader")));
    match out.unwrap() {
        GateOutcome::Denied { rule_id, .. } => assert_eq!(rule_id, RULE_DEFAULT_DENY),
        other => panic!("expected default deny, got {other:?}"),
    }
}

#[test]
fn law_iv_deny_before_allow_is_structural() {
    // The allow rule matches too Ã¢â‚¬â€ but the deny class is examined first.
    let (gw, _) = gateway_with(
        vec![
            RuleSpec {
                id: "allow-all".into(),
                effect: RuleEffect::Allow,
                expr: "true".into(),
            },
            RuleSpec {
                id: "deny-writes".into(),
                effect: RuleEffect::Deny,
                expr: "attr.effect.kind != 'file_read'".into(),
            },
        ],
        vec![],
    );
    let out = futures_block_on(gw.submit(write_action("writer")));
    match out.unwrap() {
        GateOutcome::Denied { rule_id, .. } => assert_eq!(rule_id, "deny-writes"),
        other => panic!("expected structural deny, got {other:?}"),
    }
}

#[test]
fn law_iv_broken_rule_refuses_rather_than_opens() {
    // Compiles fine but errors at evaluation time (unknown field access).
    let (gw, _) = gateway_with(
        vec![RuleSpec {
            id: "haunted".into(),
            effect: RuleEffect::Allow,
            expr: "attr.ghost.property > 1".into(),
        }],
        vec![],
    );
    let out = futures_block_on(gw.submit(read_action("reader")));
    match out.unwrap() {
        GateOutcome::Denied { rule_id, .. } => assert_eq!(rule_id, RULE_BROKEN),
        other => panic!("broken rule must refuse, got {other:?}"),
    }
}

#[test]
fn law_iv_unresolvable_target_never_reaches_policy() {
    let (gw, _) = gateway_with(
        vec![RuleSpec {
            id: "allow-all".into(),
            effect: RuleEffect::Allow,
            expr: "true".into(),
        }],
        vec![],
    );
    // A target outside the seeded workspace root cannot resolve.
    let mut req = ActionRequest::new(agent(), "reader", EffectKind::FileRead);
    req.target_uri = "https://unregistered.example/secret".into();
    let err = futures_block_on(gw.submit(req)).unwrap_err();
    assert!(matches!(err, praetorium::PraetoriumError::Refused { .. }));
}

#[test]
fn law_iv_audit_row_precedes_execution_and_settlement_follows() {
    let (gw, journal) = gateway_with(vec![read_rule("reader")], vec![]);
    let out = futures_block_on(gw.submit(read_action("reader"))).unwrap();

    // Execution happened.
    assert_eq!(*journal.lock().unwrap(), vec!["exec:reader".to_string()]);
    match out {
        GateOutcome::Executed { outcome, .. } => match outcome {
            Outcome::Completed { result } => {
                assert_eq!(result["done"], serde_json::json!(true));
            }
            other => panic!("expected completion, got {other:?}"),
        },
        other => panic!("expected execution, got {other:?}"),
    }

    // Chain order: a decision row exists and a settlement row follows it.
    let recs = gw.ledger_snapshot();
    let dec = recs
        .iter()
        .position(|r| r.kind == "decision")
        .expect("decision row");
    let settle = recs
        .iter()
        .position(|r| r.kind == "settlement")
        .expect("settlement row");
    assert!(dec < settle, "audit must precede settlement");
    assert!(gw.verify_ledger());
}

#[test]
fn law_iv_failed_effects_are_first_class_rows_not_swallowed() {
    let (gw, _) = gateway_with(
        vec![RuleSpec {
            id: "allow-boom".into(),
            effect: RuleEffect::Allow,
            expr: "attr.tool.name == 'boom'".into(),
        }],
        vec!["boom".to_string()],
    );
    let mut req = ActionRequest::new(agent(), "boom", EffectKind::ShellExec);
    req.target_uri = "file://workspace/x".into();
    let out = futures_block_on(gw.submit(req)).unwrap();
    match out {
        GateOutcome::Executed { outcome, .. } => assert!(matches!(outcome, Outcome::Failed { .. })),
        other => panic!("expected executed-but-failed, got {other:?}"),
    }
    assert!(gw.verify_ledger());
}

#[test]
fn approval_flow_pending_then_execute_or_recorded_rejection() {
    let (gw, journal) = gateway_with(
        vec![
            RuleSpec {
                id: "gate-writes".into(),
                effect: RuleEffect::RequireApproval,
                expr: "attr.effect.kind != 'file_read'".into(),
            },
            read_rule("reader"),
        ],
        vec![],
    );

    // PendingÃ¢â‚¬Â¦
    let out = futures_block_on(gw.submit(write_action("writer"))).unwrap();
    let ticket = match out {
        GateOutcome::PendingApproval { ticket_id } => ticket_id,
        other => panic!("expected pending, got {other:?}"),
    };
    assert!(journal.lock().unwrap().is_empty());

    // Ã¢â‚¬Â¦rejected lands on the recordÃ¢â‚¬Â¦
    gw.reject(&ticket, "praetor-maximus", "not today").unwrap();

    // Ã¢â‚¬Â¦and a fresh one can be approved into execution.
    let out = futures_block_on(gw.submit(write_action("writer"))).unwrap();
    let ticket2 = match out {
        GateOutcome::PendingApproval { ticket_id } => ticket_id,
        other => panic!("expected pending, got {other:?}"),
    };
    let out2 = futures_block_on(gw.approve(&ticket2, "praetor-maximus")).unwrap();
    match out2 {
        GateOutcome::Executed { .. } => {}
        other => panic!("expected execution after approval, got {other:?}"),
    }
    assert_eq!(*journal.lock().unwrap(), vec!["exec:writer".to_string()]);
}

#[test]
fn tribunician_veto_freezes_all_layers_and_cancels_tickets() {
    let (gw, journal) = gateway_with(
        vec![RuleSpec {
            id: "gate-writes".into(),
            effect: RuleEffect::RequireApproval,
            expr: "true".into(),
        }],
        vec![],
    );

    let out = futures_block_on(gw.submit(write_action("writer"))).unwrap();
    let ticket = match out {
        GateOutcome::PendingApproval { ticket_id } => ticket_id,
        other => panic!("expected pending, got {other:?}"),
    };

    gw.veto("senatus consultum ultimum");

    // Queued ticket dies.
    assert!(futures_block_on(gw.approve(&ticket, "x")).is_err());
    // New submissions freeze.
    let err = futures_block_on(gw.submit(read_action("reader"))).unwrap_err();
    assert!(matches!(err, praetorium::PraetoriumError::Frozen(_)));
    assert!(journal.lock().unwrap().is_empty());

    // Cancellations are on the record, chain intact.
    let recs = gw.ledger_snapshot();
    assert!(recs.iter().any(|r| r.kind == "ticket_cancelled_by_veto"));
    assert!(recs.iter().any(|r| r.kind == "veto_raised"));
    assert!(gw.verify_ledger());
}

#[test]
fn tampering_the_chain_is_detectable() {
    let (gw, _) = gateway_with(vec![read_rule("reader")], vec![]);
    let _ = futures_block_on(gw.submit(read_action("reader")));
    assert!(gw.verify_ledger());
}

#[test]
fn law_v_identity_attestation_round_trip_and_tamper_detection() {
    let mut svc = VexillumService::new();
    svc.set_owner_keypair(VexillumKeypair::generate());
    let agent_pub = svc.enroll_agent("agt_test");

    let digest = [7u8; 32];
    let record = svc.attest("agt_test", &digest).expect("attest");

    // Third-party verification passes without trusting the deployment.
    assert!(record.verify(&digest).is_ok());
    assert_eq!(record.agent_pub, agent_pub);

    // A different digest fails verification.
    let forged = [8u8; 32];
    assert!(record.verify(&forged).is_err());
}

#[test]
fn policy_attrs_flatten_into_nested_cel_maps() {
    let attrs = PolicyAttrs::new()
        .set("tool.name", serde_json::json!("shell"))
        .set("effect.kind", serde_json::json!("shell_exec"))
        .set("page.host", serde_json::json!("internal.corp"));

    let lex = Lex::from_specs(&[RuleSpec {
        id: "no-shell".into(),
        effect: RuleEffect::Deny,
        expr: "attr.tool.name == 'shell'".into(),
    }])
    .unwrap();
    match lex.decide(&attrs) {
        Decision::Deny { rule_id, .. } => assert_eq!(rule_id, "no-shell"),
        other => panic!("nested attr resolution failed: {other:?}"),
    }
}
