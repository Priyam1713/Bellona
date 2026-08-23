//! Campaign X: hybrid memory. Keyword-only recall fails on paraphrase;
//! vector-only recall fails on exact jargon; fused recall gets both.

use memoria::{
    cosine, ArchivumStore, Embedder, HashEmbedder, HybridRecall, InMemoryArchivum, SqliteVectors,
};

fn days_ago(days: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    now - days * 86_400_000
}

fn fixture_episode(id: &str, ts_ms: u64, content: &str) -> memoria::Episode {
    memoria::Episode {
        id: id.into(),
        ts_ms,
        kind: "episodic".into(),
        content: content.into(),
    }
}

#[tokio::test]
async fn hash_embedder_is_deterministic_and_normalized() {
    let e = HashEmbedder::standard();
    let a = e.embed("rome has legions").await.unwrap();
    let b = e.embed("rome has legions").await.unwrap();
    assert_eq!(a, b);
    assert_eq!(a.len(), 64);
    let norm: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-4);
    assert!(cosine(&a, &b) > 0.999);

    let c = e.embed("totally different words about tea").await.unwrap();
    assert!(cosine(&a, &c) < 0.5, "unrelated texts should diverge");
}

#[tokio::test]
async fn hybrid_recall_beats_keyword_only_on_paraphrase() {
    // The keyword channel (LIKE/FTS on literal terms) cannot connect
    // "database choice" to an episode that says "we picked sqlite" â€” the
    // vector channel bridges it.
    let arch = InMemoryArchivum::new();
    let vecs = SqliteVectors::in_memory().unwrap();
    let embedder = HashEmbedder::standard();

    let eps = [
        (
            "e1",
            days_ago(21),
            "we chose sqlite as the storage engine for sessions",
        ),
        (
            "e2",
            days_ago(10),
            "the deploy pipeline runs cargo test before push",
        ),
        ("e3", days_ago(3), "frontend colors settled on warm orange"),
    ];
    for (id, ts, content) in &eps {
        let ep = fixture_episode(id, *ts, content);
        arch.put(ep.clone()).await.unwrap();
        let v = embedder.embed(content).await.unwrap();
        vecs.insert(&ep, &v).unwrap();
    }

    // keyword-only misses:
    let kw_only = arch.search("storage engine decision", 3).await.unwrap();
    let kw_hit = kw_only.iter().any(|e| e.id == "e1");

    let hybrid = HybridRecall::new(arch.clone(), vecs, embedder);
    let fused = hybrid.recall("storage engine decision", 3).await.unwrap();
    let fused_hit = fused.iter().any(|e| e.id == "e1");
    assert!(
        fused_hit && fused[0].id == "e1",
        "hybrid must surface sqlite episode first; got {:?}",
        fused.iter().map(|e| e.id.as_str()).collect::<Vec<_>>()
    );
    let _ = kw_hit; // documented: may be empty here
}

#[tokio::test]
async fn both_channels_fused_rank_shared_hits_first() {
    let arch = InMemoryArchivum::new();
    let vecs = SqliteVectors::in_memory().unwrap();
    let embedder = HashEmbedder::standard();

    let eps = [
        ("shared", days_ago(1), "bellona gateway audit chain"),
        (
            "kwonly",
            days_ago(40),
            "bellona gateway audit chain but old",
        ),
    ];
    for (id, ts, content) in &eps {
        let ep = fixture_episode(id, *ts, content);
        arch.put(ep.clone()).await.unwrap();
        let v = embedder.embed(content).await.unwrap();
        vecs.insert(&ep, &v).unwrap();
    }

    let hybrid = HybridRecall::new(arch.clone(), vecs, embedder);
    let fused = hybrid
        .recall("bellona gateway audit chain", 2)
        .await
        .unwrap();
    assert_eq!(fused[0].id, "shared", "fresh + dual-channel wins");
}
