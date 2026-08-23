use std::collections::BTreeMap;
use vigiles::{
    compute_pass_at_k, CaseResult, CaseVerdict, Gate, GateVerdict, SpanKind, TraceRecorder,
    Verifier,
};

fn verdicts_from(results: &[CaseResult], trials: usize) -> Vec<CaseVerdict> {
    results
        .iter()
        .map(|r| {
            let flags = r
                .outputs
                .iter()
                .map(|o| o.contains("good"))
                .collect::<Vec<_>>();
            let complete = r.outputs.len() >= trials;
            CaseVerdict {
                case_id: r.case_id.clone(),
                passed_at_k: complete && flags.iter().all(|f| *f),
                trial_flags: flags,
            }
        })
        .collect()
}

#[test]
fn pass_at_k_demands_every_repeat() {
    // Case A passes all 3 trials; case B fails one of 3.
    let cases = verdicts_from(
        &[
            CaseResult {
                case_id: "A".into(),
                outputs: vec!["good".into(), "good".into(), "good".into()],
            },
            CaseResult {
                case_id: "B".into(),
                outputs: vec!["good".into(), "bad".into(), "good".into()],
            },
        ],
        3,
    );
    let k = compute_pass_at_k(&cases);
    assert!((k - 0.5).abs() < f64::EPSILON);

    // A missing trial is a failed run.
    let cases_short = verdicts_from(
        &[CaseResult {
            case_id: "C".into(),
            outputs: vec!["good".into()],
        }],
        3,
    );
    assert!(compute_pass_at_k(&cases_short) < f64::EPSILON);
}

#[test]
fn gate_exit_codes_are_ci_ready() {
    let gate = Gate {
        min_pass_at_k: 0.8,
        max_cost_cents: 100,
    };
    let report_ok = vigiles::SuiteReport {
        suite_name: "s".into(),
        cases: vec![],
        pass_at_k: 0.9,
        total_cost_cents: 50,
    };
    assert_eq!(gate.evaluate(&report_ok), GateVerdict::Passed);
    assert_eq!(gate.exit_code(GateVerdict::Passed), 0);

    let report_flaky = vigiles::SuiteReport {
        pass_at_k: 0.5,
        ..report_ok.clone()
    };
    assert_eq!(gate.evaluate(&report_flaky), GateVerdict::FailedReliability);
    assert_eq!(gate.exit_code(GateVerdict::FailedReliability), 1);

    let report_expensive = vigiles::SuiteReport {
        total_cost_cents: 900,
        ..report_ok
    };
    assert_eq!(gate.evaluate(&report_expensive), GateVerdict::FailedBudget);
    assert_eq!(gate.exit_code(GateVerdict::FailedBudget), 2);
}

#[test]
fn verifiers_are_honest() {
    let v = Verifier::ContainsAll {
        fragments: vec!["rome".into(), "fora".into()],
    };
    assert!(v.judge("ROME has many FORA"));
    assert!(!v.judge("rome has many baths"));

    let exact = Verifier::ExactOutput {
        expected: "42".into(),
    };
    assert!(exact.judge(" 42\n"), "whitespace-insensitive by design");
    assert!(!exact.judge("43"));

    let starts = Verifier::StartsWith {
        prefix: "OK".into(),
    };
    assert!(starts.judge("ok: done"));
}

#[test]
fn trace_recorder_times_spans_with_convention_names() {
    let rec = TraceRecorder::new();
    let out = rec.time(SpanKind::ToolCall, BTreeMap::new(), || 7 + 35);
    assert_eq!(out, 42);
    let snap = rec.snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0].name, "gen_ai.tool.call");
}
