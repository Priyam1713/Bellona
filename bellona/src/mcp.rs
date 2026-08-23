//! Campaign XIV-2 â€” Model Context Protocol server, both transports.
//!
//! `bellona mcp` (stdio, line-delimited JSON-RPC) and POST /mcp (streamable
//! HTTP single-response mode). Tools come from the live registry; every
//! tools/call passes through the Praetorian Gate like everything else.
//!
//! Protocol: JSON-RPC 2.0, MCP 2025-03-26 shape:
//!   initialize â†’ capabilities + serverInfo
//!   tools/list â†’ { tools: [{name, description, inputSchema}] }
//!   tools/call â†’ { content: [{type:"text", text}], isError }

use crate::Assembled;
use forge::primitives::{ActionRequest, EffectKind, Outcome};
use praetorium::custos::GateOutcome;
use serde_json::{json, Value};
use std::sync::Arc;

pub const PROTOCOL_VERSION: &str = "2025-03-26";

type RpcResult = Result<Value, Value>;

fn rpc_error(id: Value, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Pure handler â€” transport layers feed requests in, get optional responses
/// out (notifications yield None).
pub async fn handle_request(assembled: &Assembled, req: &Value) -> Option<Value> {
    let method = req.get("method").and_then(|m| m.as_str())?;
    let id = req.get("id").cloned()?;
    let params = req.get("params").cloned().unwrap_or(Value::Null);

    let result: RpcResult = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "bellona",
                "version": env!("CARGO_PKG_VERSION"),
            },
        })),
        "tools/list" => {
            let tools: Vec<Value> = assembled
                .registry
                .exposed_specs()
                .iter()
                .map(|s| {
                    json!({
                        "name": s.name,
                        "description": s.description,
                        "inputSchema": if s.schema.is_null() {
                            json!({"type": "object", "properties": {}})
                        } else {
                            s.schema.clone()
                        },
                    })
                })
                .collect();
            Ok(json!({ "tools": tools }))
        }
        "tools/call" => call_tool(assembled, &params).await,
        "ping" => Ok(json!({})),
        other => Err(rpc_error(
            id.clone(),
            -32601,
            format!("method not found: {other}"),
        )),
    };

    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(err_obj) => err_obj,
    })
}

async fn call_tool(assembled: &Assembled, params: &Value) -> RpcResult {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| rpc_error(Value::Null, -32602, "missing tool name".into()))?
        .to_string();
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);

    // Effect from the tool's own spec â€” never guessed (Law IV).
    let effect = assembled
        .registry
        .get(&name)
        .map(|t| t.spec().effect.clone())
        .unwrap_or(EffectKind::Custom("unknown_tool".into()));

    let mut req = ActionRequest::new(
        forge::AgentId(forge::Id("agt_mcp".into())),
        name.clone(),
        effect,
    )
    .with_intent(format!("mcp client call to {name}"))
    .with_params(args.clone());
    let derived = bellum::target_uri_hint(&args);
    req.target_uri = if derived.is_empty() {
        "file://workspace".into()
    } else {
        derived
    };

    let outcome = assembled
        .gateway
        .submit(req)
        .await
        .map_err(|e| rpc_error(Value::Null, -32000, format!("gate refusal: {e}")))?;

    match outcome {
        GateOutcome::Executed {
            outcome: Outcome::Completed { result },
            ..
        } => Ok(json!({
            "content": [{ "type": "text",
                "text": serde_json::to_string_pretty(&result).unwrap_or_default() }],
            "isError": false,
        })),
        GateOutcome::Executed {
            outcome: Outcome::Failed { error },
            ..
        } => Ok(tool_text_error(error)),
        GateOutcome::Denied { rule_id, reason } => Ok(tool_text_error(format!(
            "refused by rule '{rule_id}': {reason}"
        ))),
        GateOutcome::PendingApproval { ticket_id } => Ok(tool_text_error(format!(
            "parked for human approval (ticket {ticket_id})"
        ))),
    }
}

fn tool_text_error(text: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": text.into() }],
        "isError": true,
    })
}

/// stdio loop: one JSON-RPC request per line; responses on stdout.
pub async fn serve_stdio(assembled: Arc<Assembled>) -> i32 {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    loop {
        let line = match lines.next_line().await {
            Ok(Some(l)) => l,
            Ok(None) | Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(req) => {
                if let Some(resp) = handle_request(&assembled, &req).await {
                    let _ = stdout.write_all(format!("{resp}\n").as_bytes()).await;
                    let _ = stdout.flush().await;
                }
            }
            Err(_) => {
                let resp = rpc_error(Value::Null, -32700, "parse error".into());
                let _ = stdout.write_all(format!("{resp}\n").as_bytes()).await;
                let _ = stdout.flush().await;
            }
        }
    }
    0
}
