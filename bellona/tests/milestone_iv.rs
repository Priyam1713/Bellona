//! M-IV additions: cost accounting enforced by the Aerarium, durable
//! SQLite sessions, and veto propagation.

use async_trait::async_trait;
use bellum::{
    Aerarium, BellumError, CascadeRouter, ModelClient, ModelReply, ReActStrategy, WarLoop,
};
use forge::session::{Session, SessionStore};
use memoria::SqliteSessionStore;
use praetorium::custos::{CustosGateway, SnapshotResolver};
use std::sync::{Arc, Mutex};

struct PricedModel {
    cents_per_call: u64,
    answer: &'static str,
    calls: Mutex<u32>,
}

impl PricedModel {
    fn new(cents: u64) -> Self {
        PricedModel {
            cents_per_call: cents,
            answer: "done",
            calls: Mutex::new(0),
        }
    }
}

#[async_trait]
impl ModelClient for PricedModel {
    fn tier(&self) -> &'static str {
        "terra"
    }
    async fn complete(&self, _p: &str) -> Result<ModelReply, BellumError> {
        *self.calls.lock().unwrap() += 1;
        // Answer immediately but at an absurd price.
        Ok(ModelReply {
            thought: String::new(),
            tool_calls: vec![],
            final_answer: Some(self.answer.to_string()),
            cost_cents: self.cents_per_call,
        })
    }
}

fn tiny_loop(
    model: Arc<dyn ModelClient>,
) -> (
    WarLoop<SnapshotResolver, bellona::RegistryExecutor>,
    Arc<CustosGateway<SnapshotResolver, bellona::RegistryExecutor>>,
) {
    let cfg = bellona::BellonaConfig::default();
    let a = bellona::assemble(&cfg).unwrap();
    let gw = a.gateway.clone();
    let router = CascadeRouter::new(vec![model]);
    let l = WarLoop::new(
        a.gateway.clone(),
        a.registry.clone(),
        router,
        Aerarium::new(50, 100),
    );
    (l, gw)
}

#[tokio::test]
async fn aerarium_halts_when_model_spend_exceeds_budget() {
    // 90 cents per call; budget 100 â†’ second call must trip the breaker
    // before the run can continue past it.
    let model = Arc::new(PricedModel::new(150));
    let (loop_, _gw) = tiny_loop(model.clone());
    let report = loop_
        .run("priced", Box::new(ReActStrategy::new("p", 50)), None)
        .await
        .unwrap();

    let brk = match &report.breaker {
        Some(b) => b.clone(),
        None => panic!("no breaker; report={report:?}"),
    };
    assert!(brk.contains("max_cost"));
    assert_eq!(report.cost_cents, 150);
    assert!(!report.ok);
}

#[tokio::test]
async fn priced_run_that_finishes_reports_real_cost() {
    let model = Arc::new(PricedModel::new(7));
    let (loop_, _) = tiny_loop(model);
    let report = loop_
        .run("cheap", Box::new(ReActStrategy::new("c", 10)), None)
        .await
        .unwrap();
    assert!(report.ok);
    assert_eq!(report.cost_cents, 7);
}

#[tokio::test]
async fn sqlite_sessions_survive_reopen_and_round_trip_ledgers() {
    let dir = std::env::temp_dir().join(format!("bellona-sess-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("sessions.sqlite3");

    {
        let store = SqliteSessionStore::open(&db).unwrap();
        let mut s = Session::new("take the hill");
        s.pinned.push("[PINNED] goal: take the hill".into());
        s.append(
            "decision",
            "flanked at dawn",
            serde_json::json!({"ok": true}),
        );
        store.put(s).await.unwrap();
    }
    {
        let store = SqliteSessionStore::open(&db).unwrap();
        let ids = store.list().await.unwrap();
        assert_eq!(ids.len(), 1);
        let s = store.get(&ids[0]).await.unwrap();
        assert_eq!(s.goal, "take the hill");
        assert_eq!(s.ledger[0].summary, "flanked at dawn");
        assert_eq!(s.pinned[0], "[PINNED] goal: take the hill");
    }
    let _ = std::fs::remove_file(&db);
}

#[tokio::test]
async fn veto_event_reaches_the_bus_for_surface_layers() {
    use forge::event::BusEvent;
    let cfg = bellona::BellonaConfig::default();
    let a = bellona::assemble(&cfg).unwrap();
    let mut rx = a.gateway.bus().subscribe();
    a.gateway.veto("drill");
    match rx.recv().await.unwrap() {
        BusEvent::VetoRaised { reason } => assert_eq!(reason, "drill"),
        other => panic!("expected veto event, got {other:?}"),
    }
}
