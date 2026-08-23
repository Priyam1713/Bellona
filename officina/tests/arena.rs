use castra::{CampCommand, EnvScrubPolicy, SandboxDriver};
use officina::{promote, run_battery, BatteryCase, ForgedTool, ScriptLang};
use std::path::PathBuf;

/// A camp that "executes" by echoing the rendered args â€” enough to prove
/// the battery mechanics without spawning processes.
struct EchoCamp;

#[async_trait::async_trait]
impl SandboxDriver for EchoCamp {
    fn level(&self) -> castra::CastraLevel {
        castra::CastraLevel::Prima
    }
    async fn run(
        &self,
        cmd: &CampCommand,
        _env: &EnvScrubPolicy,
    ) -> castra::CastraResult<castra::CampOutcome> {
        let stdout = cmd.args.join(" ");
        Ok(castra::CampOutcome {
            exit_ok: true,
            stdout,
            stderr: String::new(),
        })
    }
}

#[tokio::test]
async fn forged_tool_survives_battery_and_earns_promotion() {
    let tool = ForgedTool {
        name: "word_counter".into(),
        description: "counts occurrences of a word in a file".into(),
        lang: ScriptLang::Shell,
        // Rendered args echo the input; our case asserts on it.
        script_template: "count {input}".into(),
        forged_by: "agt_forging".into(),
    };
    let battery = vec![BatteryCase {
        input_json: r#"{"path":"a.txt","word":"rome"}"#.into(),
        output_contains: vec!["rome".into()],
    }];

    let verdict = run_battery(&tool, &battery, &EchoCamp, PathBuf::from(".")).await;
    assert!(verdict.passed, "failures: {:?}", verdict.failures);

    let legionary = promote(tool, &verdict, Some("sig_owner_attestation_42")).unwrap();
    assert_eq!(legionary.name, "word_counter");
}

#[tokio::test]
async fn failing_battery_blocks_promotion() {
    let tool = ForgedTool {
        name: "broken_tool".into(),
        description: "promises what it cannot do".into(),
        lang: ScriptLang::Shell,
        script_template: "do {input}".into(),
        forged_by: "agt_forging".into(),
    };
    let battery = vec![BatteryCase {
        input_json: "{}".into(),
        output_contains: vec!["expected-but-missing".into()],
    }];

    let verdict = run_battery(&tool, &battery, &EchoCamp, PathBuf::from(".")).await;
    assert!(!verdict.passed);
    assert!(promote(tool, &verdict, Some("sig_owner_attestation_42")).is_err());
}

#[tokio::test]
async fn promotion_without_countersignature_is_refused() {
    let tool = ForgedTool {
        name: "good_but_unsigned".into(),
        description: "passes the arena, lacks the seal".into(),
        lang: ScriptLang::Shell,
        script_template: "ok {input}".into(),
        forged_by: "agt_forging".into(),
    };
    let battery = vec![BatteryCase {
        input_json: "{}".into(),
        output_contains: vec!["{}".into()],
    }];
    let verdict = run_battery(&tool, &battery, &EchoCamp, PathBuf::from(".")).await;
    assert!(verdict.passed);
    assert!(promote(tool, &verdict, None).is_err());
}

#[test]
fn malformed_manifests_never_reach_the_arena() {
    let bad_name = ForgedTool {
        name: "Bad Name!".into(),
        description: "described well enough here".into(),
        lang: ScriptLang::Shell,
        script_template: "x {input}".into(),
        forged_by: "a".into(),
    };
    assert!(bad_name.validate().is_err());

    let no_input_ref = ForgedTool {
        name: "fine_name".into(),
        description: "described well enough here".into(),
        lang: ScriptLang::Python,
        script_template: "print('hi')".into(), // no {input}
        forged_by: "a".into(),
    };
    assert!(no_input_ref.validate().is_err());
}
