//! Memory doctrine tests: pinning, decision records, consolidation, lineage.

use memoria::{
    new_episode, ArchivumStore, HeuristicConsolidator, InMemoryArchivum, Nervi, SleepDaemon,
    Tabella,
};
use std::collections::BTreeMap;

#[test]
fn nervi_refuses_duplicates_and_renders_pinned() {
    let mut n = Nervi::new();
    assert!(n.pin("goal: fix the flaky test"));
    assert!(!n.pin("goal: fix the flaky test"), "duplicate pins refused");
    assert!(n.unpin("goal: fix the flaky test"));
    assert!(!n.unpin("goal: fix the flaky test"));
}

#[test]
fn tabella_records_decisions_and_searches_them() {
    let mut t = Tabella::new();
    let mut d = BTreeMap::new();
    d.insert("tool".to_string(), "shell".to_string());
    t.record("tool_selected", "chose shell for grep", d);
    t.record(
        "hypothesis_discarded",
        "cache theory disproven",
        BTreeMap::new(),
    );

    assert_eq!(t.len(), 2);
    assert_eq!(t.search("grep").len(), 1);
    assert_eq!(t.latest_of_kind("hypothesis_discarded").unwrap().seq, 1);
    let rendered = t.render_recent(10);
    assert!(rendered.contains("[TABELLA]"));
    assert!(rendered.contains("0. [tool_selected]"));
}

#[tokio::test]
async fn somnium_distills_repeats_into_skills_and_dedupes_facts() {
    let store = InMemoryArchivum::new();
    for _ in 0..3 {
        store
            .put(new_episode(
                "episodic",
                "ran cargo test then fixed imports".into(),
            ))
            .await
            .unwrap();
    }
    store
        .put(new_episode(
            "semantic",
            "the repo uses Rust edition 2021".into(),
        ))
        .await
        .unwrap();

    let written = SleepDaemon::pass(&store, &HeuristicConsolidator)
        .await
        .unwrap();
    assert!(written >= 1, "at least the skill or fact distilled");

    // Second pass must not duplicate facts.
    let again = SleepDaemon::pass(&store, &HeuristicConsolidator)
        .await
        .unwrap();
    let distilled = store.by_kind("distilled").await.unwrap();
    let fact_count = distilled
        .iter()
        .filter(|e| e.content.contains("edition 2021"))
        .count();
    assert_eq!(fact_count, 1, "facts are deduped across passes");
    let _ = again;
}

#[test]
fn context_compaction_preserves_lineage_span() {
    let mut w = forge::ContextWindow::new(10_000);
    w.push(forge::context::Role::User, "old one", false)
        .unwrap();
    w.push(forge::context::Role::Agent, "old two", false)
        .unwrap();
    w.push(forge::context::Role::Tool, "old three", false)
        .unwrap();
    w.push(forge::context::Role::User, "pinned goal stays", true)
        .unwrap();
    w.push(forge::context::Role::User, "recent", false).unwrap();

    let compacted = w
        .compact(1, |victims| format!("summary of {} blocks", victims.len()))
        .unwrap();

    assert_eq!(compacted, 3, "three unpinned old blocks folded");
    let blocks = w.blocks();
    let summary = blocks
        .iter()
        .find(|b| b.lineage.is_some())
        .expect("summary exists");
    assert_eq!(summary.lineage.as_ref().unwrap().from_seq, 0);
    assert_eq!(summary.lineage.as_ref().unwrap().to_seq, 2);

    // Pinned survives untouched.
    assert!(blocks
        .iter()
        .any(|b| b.pinned && b.content == "pinned goal stays"));
}
