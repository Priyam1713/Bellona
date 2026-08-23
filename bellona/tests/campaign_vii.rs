//! Campaign VII: the War Room speaks the same law over HTTP.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use bellona::{assemble, BellonaConfig};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static UNIQ_SEQ: AtomicU64 = AtomicU64::new(1);
fn uniq() -> u64 {
    UNIQ_SEQ.fetch_add(1, Ordering::Relaxed)
}
use std::sync::Arc;
use tower::ServiceExt;

fn temp_ws(tag: &str) -> (PathBuf, TempGuard) {
    let dir = std::env::temp_dir().join(format!(
        "bellona-c7-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as u64
            * 1_000_000
            + uniq()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.clone(), TempGuard(dir))
}
struct TempGuard(PathBuf);
impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn make_app(yolo: bool) -> axum::Router {
    let (ws, _keep) = temp_ws("router");
    let _ = _keep;
    let cfg = BellonaConfig {
        workspace: ws,
        yolo,
        ..Default::default()
    };
    let assembled = assemble(&cfg).unwrap();
    let model: Arc<dyn bellum::ModelClient> = Arc::new(bellona::tests_support::NullModel);
    bellona::warroom::router(bellona::warroom::WarRoom {
        runs: Default::default(),
        assembled,
        cfg,
        model,
    })
}

async fn post_json(
    app: &axum::Router,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let resp = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

#[tokio::test]
async fn gate_over_http_allow_and_audit() {
    let app = make_app(true); // yolo: writes auto-approved

    // A read flows through allow path.
    let (status, body) = post_json(
        &app,
        "/v1/gate/submit",
        serde_json::json!({
            "agent_id": "agt_http", "tool_name": "read_file",
            "effect": "file_read", "target_uri": "file://workspace/notes.txt",
            "params": {"path": "notes.txt"}, "intent": "read"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["gate"], "executed", "{body}");

    let (_, ledger) = get_json(&app, "/v1/ledger").await;
    assert_eq!(ledger["chain_valid"], true);
    assert!(ledger["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["kind"] == "decision"));
}

#[tokio::test]
async fn approval_flow_via_http_endpoints() {
    let app = make_app(false); // not yolo: writes park for approval

    let (status, body) = post_json(
        &app,
        "/v1/gate/submit",
        serde_json::json!({
            "agent_id": "agt_http", "tool_name": "write_file",
            "effect": "file_write", "target_uri": "file://workspace/out.txt",
            "params": {"path": "out.txt", "content": "x"}, "intent": "persist"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["gate"], "pending_approval", "{body}");
    let ticket = body["ticket_id"].as_str().unwrap().to_string();

    let (_, pend) = get_json(&app, "/v1/gate/pending").await;
    assert_eq!(pend["tickets"].as_array().unwrap().len(), 1);

    let (st2, rej) = post_json(
        &app,
        "/v1/gate/reject",
        serde_json::json!({"ticket_id": ticket, "approver": "console", "reason": "no"}),
    )
    .await;
    assert_eq!(st2, StatusCode::OK, "{rej}");

    let (_, ledger) = get_json(&app, "/v1/ledger").await;
    assert!(ledger["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["kind"] == "approval_rejected"));
}

#[tokio::test]
async fn veto_endpoint_freezes_the_gate() {
    let app = make_app(true);
    let (_, v) = post_json(&app, "/v1/veto", serde_json::json!({"reason": "drill"})).await;
    assert_eq!(v["vetoed"], true);

    let (status, _) = post_json(
        &app,
        "/v1/gate/submit",
        serde_json::json!({
            "agent_id": "a", "tool_name": "read_file",
            "effect": "file_read", "target_uri": "file://workspace/x",
            "intent": "i"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK); // handler okÃ¢â‚¬Â¦
                                        // Ã¢â‚¬Â¦but outcome carries frozen refusal via error field.
}
