//! Campaign VII â€” the War Room: eyes on every battle, hands on every gate.
//!
//! `bellona serve` exposes:
//! - the Praetorian Gate over HTTP (matching @bellona-works/sdk exactly)
//! - the ledger + chain verification
//! - campaign launching with live AG-UI event streams (SSE)
//! - an embedded single-file console (no node build step, Law I)

use axum::extract::State;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use bellum::{Aerarium, CascadeRouter, ReActStrategy, WarLoop};
use foedus::agui::from_bus;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;

pub struct WarRoom {
    pub assembled: crate::Assembled,
    pub cfg: crate::BellonaConfig,
    pub model: Arc<dyn bellum::ModelClient>,
}

type St = Arc<WarRoom>;

// ---------- request bodies ----------

#[derive(Deserialize)]
struct SubmitBody {
    agent_id: String,
    tool_name: String,
    effect: String,
    target_uri: String,
    #[serde(default)]
    params: serde_json::Value,
    #[serde(default)]
    intent: String,
}

#[derive(Deserialize)]
struct ApproveBody {
    ticket_id: String,
    approver: String,
}

#[derive(Deserialize)]
struct RejectBody {
    ticket_id: String,
    approver: String,
    reason: String,
}

#[derive(Deserialize)]
struct RunBody {
    goal: String,
    #[serde(default)]
    max_steps: Option<usize>,
}

// ---------- handlers ----------

async fn submit(State(st): State<St>, Json(b): Json<SubmitBody>) -> Json<serde_json::Value> {
    let mut req = forge::ActionRequest::new(
        forge::AgentId(forge::Id(b.agent_id)),
        b.tool_name,
        parse_effect(&b.effect),
    )
    .with_target(b.target_uri)
    .with_intent(b.intent)
    .with_params(b.params);
    req.target_uri = if req.target_uri.is_empty() {
        "file://workspace".into()
    } else {
        req.target_uri
    };
    match st.assembled.gateway.submit(req).await {
        Ok(outcome) => Json(serde_json::to_value(outcome).unwrap_or_default()),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

fn parse_effect(s: &str) -> forge::primitives::EffectKind {
    match s {
        "file_read" => forge::primitives::EffectKind::FileRead,
        "file_write" => forge::primitives::EffectKind::FileWrite,
        "shell_exec" => forge::primitives::EffectKind::ShellExec,
        "browser_navigate" => forge::primitives::EffectKind::BrowserNavigate,
        "browser_act" => forge::primitives::EffectKind::BrowserAct,
        "mcp_call" => forge::primitives::EffectKind::McpCall,
        other => forge::primitives::EffectKind::Custom(other.to_string()),
    }
}

async fn approve(State(st): State<St>, Json(b): Json<ApproveBody>) -> Json<serde_json::Value> {
    match st
        .assembled
        .gateway
        .approve(&b.ticket_id, &b.approver)
        .await
    {
        Ok(outcome) => Json(serde_json::to_value(outcome).unwrap_or_default()),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

async fn reject(State(st): State<St>, Json(b): Json<RejectBody>) -> Json<serde_json::Value> {
    match st
        .assembled
        .gateway
        .reject(&b.ticket_id, &b.approver, &b.reason)
    {
        Ok(()) => Json(serde_json::json!({ "rejected": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

async fn pending(State(st): State<St>) -> Json<serde_json::Value> {
    let tickets = st.assembled.gateway.pending_tickets();
    let arr: Vec<serde_json::Value> = tickets
        .into_iter()
        .map(|(id, tool, intent)| json!({ "ticket_id": id, "tool": tool, "intent": intent }))
        .collect();
    Json(json!({ "tickets": arr }))
}

async fn veto(State(st): State<St>, Json(b): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let reason = b
        .get("reason")
        .and_then(|r| r.as_str())
        .unwrap_or("unspecified");
    st.assembled.gateway.veto(reason);
    Json(json!({ "vetoed": true }))
}

async fn ledger(State(st): State<St>) -> Json<serde_json::Value> {
    Json(json!({
        "records": st.assembled.gateway.ledger_snapshot(),
        "chain_valid": st.assembled.gateway.verify_ledger(),
    }))
}

async fn sessions(State(_st): State<St>) -> Json<serde_json::Value> {
    // Sessions live in memory for v1; durable store lands with X.
    Json(json!({ "note": "sessions via /v1/runs history in M-X" }))
}

async fn events(
    State(st): State<St>,
) -> Sse<impl futures_util::Stream<Item = Result<SseEvent, Infallible>>> {
    let rx = st.assembled.gateway.bus().subscribe();
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let payload = from_bus(&ev)
                        .map(|a| serde_json::to_string(&a).unwrap_or_default())
                        .unwrap_or_else(|| format!("{ev:?}"));
                    return Some((Ok(SseEvent::default().data(payload)), rx));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn run_campaign(State(st): State<St>, Json(b): Json<RunBody>) -> Json<serde_json::Value> {
    let aerarium = Aerarium::new(b.max_steps.unwrap_or(st.cfg.max_steps), 500);
    let loop_ = WarLoop::new(
        st.assembled.gateway.clone(),
        st.assembled.registry.clone(),
        CascadeRouter::new(vec![st.model.clone()]),
        aerarium,
    )
    .with_auto_approver("war-room-operator");

    tokio::spawn(async move {
        let _ = loop_
            .run(
                &b.goal,
                Box::new(ReActStrategy::new(b.goal.clone(), 40)),
                None,
            )
            .await;
    });
    Json(json!({ "started": true, "watch": "/v1/events" }))
}

// ---------- Campaign IX: A2A surface ----------

async fn a2a_card(State(_st): State<St>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "bellona-centurion",
        "description": "Delegates campaigns through the Praetorian Gate.",
        "skills": ["research", "write", "code"],
        "endpoint": "/a2a/tasks",
        "protocol_versions": foedus::PROTOCOL_VERSIONS,
    }))
}

async fn a2a_tasks(State(st): State<St>, body: String) -> Json<serde_json::Value> {
    use foedus::a2a::{A2aService, IdempotencyStore, TaskExecutor};

    struct MemStore(std::sync::Mutex<std::collections::HashMap<String, String>>);
    #[async_trait::async_trait]
    impl IdempotencyStore for MemStore {
        async fn claim(&self, key: &str) -> Result<Option<String>, String> {
            Ok(self.0.lock().unwrap().get(key).cloned())
        }
        async fn complete(&self, key: &str, resp: &str) -> Result<(), String> {
            self.0.lock().unwrap().insert(key.into(), resp.into());
            Ok(())
        }
    }

    struct LoopExecutor {
        assembled: crate::Assembled,
        model: Arc<dyn bellum::ModelClient>,
    }
    #[async_trait::async_trait]
    impl TaskExecutor for LoopExecutor {
        async fn execute(&self, req: &foedus::TaskRequest) -> Result<foedus::TaskResponse, String> {
            let loop_ = bellum::WarLoop::new(
                self.assembled.gateway.clone(),
                self.assembled.registry.clone(),
                CascadeRouter::new(vec![self.model.clone()]),
                Aerarium::new(16, 200),
            )
            .with_auto_approver("a2a-delegatee");
            let report = loop_
                .run(
                    &req.instruction,
                    Box::new(ReActStrategy::new(req.instruction.clone(), 16)),
                    None,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(foedus::TaskResponse::Completed {
                artifacts: serde_json::json!({ "answer": report.answer }),
            })
        }
    }

    let req: foedus::TaskRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => return Json(json!({ "error": format!("bad task: {e}") })),
    };
    let svc = A2aService {
        store: MemStore(Default::default()),
        executor: LoopExecutor {
            assembled: st.assembled.clone(),
            model: st.model.clone(),
        },
    };
    match svc.handle(req).await {
        Ok(resp) => Json(serde_json::to_value(resp).unwrap_or_default()),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn index() -> axum::response::Html<&'static str> {
    axum::response::Html(crate::warroom_html())
}

// ---------- router ----------

pub fn router(war_room: WarRoom) -> Router {
    let st: St = Arc::new(war_room);
    Router::new()
        .route("/", get(index))
        .route("/v1/gate/submit", post(submit))
        .route("/v1/gate/approve", post(approve))
        .route("/v1/gate/reject", post(reject))
        .route("/v1/gate/pending", get(pending))
        .route("/v1/veto", post(veto))
        .route("/v1/ledger", get(ledger))
        .route("/v1/sessions", get(sessions))
        .route("/v1/events", get(events))
        .route("/v1/runs", post(run_campaign))
        .route("/a2a/card", get(a2a_card))
        .route("/a2a/tasks", post(a2a_tasks))
        .with_state(st)
}
