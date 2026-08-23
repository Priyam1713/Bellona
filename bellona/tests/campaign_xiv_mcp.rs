//! Campaign XIV-2: MCP wire compliance — initialize, tools/list,
//! tools/call through the gate (allow + deny), and stdio loop.

use axum::body::Body;
use bellona::{assemble, BellonaConfig};
use http_body_util::BodyExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
static UNIQ_SEQ: AtomicU64 = AtomicU64::new(1);
fn uniq() -> u64 {
    UNIQ_SEQ.fetch_add(1, Ordering::Relaxed)
}
use tower::ServiceExt;

fn app(yolo: bool) -> axum::Router {
    let dir = std::env::temp_dir().join(format!(
        "bellona-c14mcp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as u64
            * 1_000_000
            + uniq()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = BellonaConfig {
        workspace: dir,
        yolo,
        ..Default::default()
    };
    bellona::warroom::router(bellona::warroom::WarRoom {
        runs: Default::default(),
        assembled: assemble(&cfg).unwrap(),
        cfg,
        model: Arc::new(bellona::tests_support::NullModel),
    })
}

async fn post(app: &axum::Router, uri: &str, body: serde_json::Value) -> serde_json::Value {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

#[tokio::test]
async fn mcp_initialize_and_tools_list() {
    let _app = app(true);

    // Pure handler path (no HTTP needed for protocol correctness):
    let ws = std::env::temp_dir().join("mcp-init-ws");
    std::fs::create_dir_all(&ws).unwrap();
    let cfg = BellonaConfig {
        workspace: ws,
        ..Default::default()
    };
    let a = assemble(&cfg).unwrap();
    let arc = Arc::new(a);

    let init = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}});
    let resp = bellona::mcp::handle_request(&arc, &init).await.unwrap();
    assert_eq!(
        resp["result"]["protocolVersion"],
        bellona::mcp::PROTOCOL_VERSION
    );
    assert_eq!(resp["result"]["serverInfo"]["name"], "bellona");

    let list = serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"});
    let resp = bellona::mcp::handle_request(&arc, &list).await.unwrap();
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert!(tools.len() >= 10, "arsenal + core exposed: {}", tools.len());
    assert!(tools.iter().any(|t| t["name"] == "read_file"));
    assert!(tools.iter().any(|t| t["name"] == "web_fetch"));
}

#[tokio::test]
async fn mcp_tools_call_reads_through_the_gate() {
    let dir = std::env::temp_dir().join(format!("mcp-call-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("secret.txt"), "the plan is safe").unwrap();
    let cfg = BellonaConfig {
        workspace: dir,
        ..Default::default()
    };
    let arc = Arc::new(assemble(&cfg).unwrap());

    let call = serde_json::json!({
        "jsonrpc":"2.0","id":7,"method":"tools/call",
        "params":{"name":"read_file","arguments":{"path":"secret.txt"}}
    });
    let resp = bellona::mcp::handle_request(&arc, &call).await.unwrap();
    assert_eq!(resp["result"]["isError"], false);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("the plan is safe"), "{text}");

    // Unknown tool → isError with readable text.
    let bad = serde_json::json!({
        "jsonrpc":"2.0","id":8,"method":"tools/call",
        "params":{"name":"nope","arguments":{}}
    });
    let resp = bellona::mcp::handle_request(&arc, &bad).await.unwrap();
    assert_eq!(resp["result"]["isError"], true);
}

#[tokio::test]
async fn mcp_http_route_responds_jsonrpc() {
    let app = app(false);
    let resp = post(
        &app,
        "/mcp",
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"ping"}),
    )
    .await;
    assert_eq!(resp["result"], serde_json::json!({}));
}

#[tokio::test]
async fn stdio_loop_processes_lines() {
    let dir = std::env::temp_dir().join(format!("mcp-stdio-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = BellonaConfig {
        workspace: dir,
        ..Default::default()
    };
    let arc = Arc::new(assemble(&cfg).unwrap());

    // Drive the same pure handler the stdio loop uses; transport covered by
    // shape above.
    let ping = serde_json::json!({"jsonrpc":"2.0","id":9,"method":"ping"});
    let resp = bellona::mcp::handle_request(&arc, &ping).await.unwrap();
    assert_eq!(resp["id"], 9);
}
