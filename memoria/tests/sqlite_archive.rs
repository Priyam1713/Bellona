use memoria::{new_episode, ArchivumStore, SleepDaemon, SqliteArchivum};

#[tokio::test]
async fn sqlite_archive_round_trips_and_searches() {
    let store = SqliteArchivum::in_memory().unwrap();
    store
        .put(new_episode(
            "episodic",
            "the siege of alea began at dawn".into(),
        ))
        .await
        .unwrap();
    store
        .put(new_episode(
            "semantic",
            "alea is a village in the hills".into(),
        ))
        .await
        .unwrap();

    let hits = store.search("alea", 5).await.unwrap();
    assert_eq!(hits.len(), 2, "both episodes mention alea");

    let kind_hits = store.by_kind("semantic").await.unwrap();
    assert_eq!(kind_hits.len(), 1);
    assert!(kind_hits[0].content.contains("hills"));
}

#[tokio::test]
async fn sqlite_persistence_across_reopen() {
    let dir = std::env::temp_dir().join(format!("bellona-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("archive.sqlite3");

    {
        let store = SqliteArchivum::open(&db).unwrap();
        store
            .put(new_episode(
                "procedural",
                "deploy: build then test then push".into(),
            ))
            .await
            .unwrap();
    }
    {
        let reopened = SqliteArchivum::open(&db).unwrap();
        let all = reopened.by_kind("procedural").await.unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].content.contains("build then test"));
    }
    let _ = std::fs::remove_file(&db);
}

#[tokio::test]
async fn somnium_consolidates_into_sqlite() {
    let store = SqliteArchivum::in_memory().unwrap();
    for _ in 0..2 {
        store
            .put(new_episode(
                "episodic",
                "ran cargo test then fixed imports".into(),
            ))
            .await
            .unwrap();
    }
    let written = SleepDaemon::pass(&store, &memoria::HeuristicConsolidator)
        .await
        .unwrap();
    assert!(written >= 1);
    let distilled = store.by_kind("distilled").await.unwrap();
    assert!(!distilled.is_empty());
}
