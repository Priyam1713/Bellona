//! VI.1 — Discord Gateway v10 codec and heartbeat state machine.
//!
//! The wire layer (websocket I/O) is thin; the *decisions* live here as pure
//! functions so they are unit-testable without a socket:
//! - HELLO → heartbeat interval extraction
//! - IDENTIFY payload construction (token never logged)
//! - HEARTBEAT payload with last-seq
//! - dispatch dedupe rules (resumable vs non-resumable closes)

use serde::{Deserialize, Serialize};

pub const GATEWAY_VERSION: u8 = 10;

/// Opcodes from Discord gateway protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Dispatch = 0,
    Heartbeat = 1,
    Identify = 2,
    Hello = 10,
    HeartbeatAck = 11,
}

impl TryFrom<u8> for Op {
    type Error = ();
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Ok(match v {
            0 => Op::Dispatch,
            1 => Op::Heartbeat,
            2 => Op::Identify,
            10 => Op::Hello,
            11 => Op::HeartbeatAck,
            _ => return Err(()),
        })
    }
}

/// Parsed inbound frame.
#[derive(Debug, Clone)]
pub struct Frame {
    pub op: Op,
    /// Only present on Dispatch frames.
    pub t: Option<String>,
    /// Sequence number (Dispatch only).
    pub s: Option<u64>,
    pub data: serde_json::Value,
}

pub fn parse_frame(raw: &str) -> Result<Option<Frame>, String> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let op_u8 = v.get("op").and_then(|x| x.as_u64()).unwrap_or(u64::MAX);
    let op_u8 = u8::try_from(op_u8).map_err(|_| format!("op out of range: {op_u8}"))?;
    let op = Op::try_from(op_u8).map_err(|_| format!("unknown op {op_u8}"))?;
    if !matches!(
        op,
        Op::Dispatch | Op::Hello | Op::HeartbeatAck | Op::Heartbeat
    ) {
        // Other ops are ignored by our minimal client.
        return Ok(None);
    }
    Ok(Some(Frame {
        op,
        t: v.get("t").and_then(|x| x.as_str()).map(String::from),
        s: v.get("s").and_then(|x| x.as_u64()),
        data: v.get("d").cloned().unwrap_or(serde_json::Value::Null),
    }))
}

/// Extract heartbeat interval (ms) from a HELLO frame.
pub fn hello_interval(frame: &Frame) -> Option<u64> {
    frame
        .data
        .get("heartbeat_interval")
        .and_then(|v| v.as_u64())
}

/// IDENTIFY payload. The token rides once in `d.token`; callers must not log
/// the returned JSON verbatim.
pub fn identify_payload(token: &str) -> String {
    serde_json::json!({
        "op": Op::Identify as u8,
        "d": {
            "token": token,
            "intents": 512 | 32768, // GUILD_MESSAGES | MESSAGE_CONTENT
            "properties": { "os": "bellona", "browser": "nuntii", "device": "nuntii" },
        }
    })
    .to_string()
}

/// HEARTBEAT payload carrying the last seen sequence.
pub fn heartbeat_payload(last_seq: Option<u64>) -> String {
    serde_json::json!({
        "op": Op::Heartbeat as u8,
        "d": last_seq,
    })
    .to_string()
}

/// An extracted mention from a MESSAGE_CREATE dispatch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscordMessage {
    pub message_id: String,
    pub channel_id: String,
    pub author_id: String,
    pub content: String,
}

/// Parse MESSAGE_CREATE `d` into a mention; empty/other-bot messages skipped.
pub fn parse_message_create(d: &serde_json::Value) -> Option<DiscordMessage> {
    let id = d.get("id").and_then(|v| v.as_str())?.to_string();
    let channel_id = d.get("channel_id").and_then(|v| v.as_str())?.to_string();
    let author_bot = d
        .get("author")
        .map(|a| a.get("bot").and_then(|b| b.as_bool()).unwrap_or(false))
        .unwrap_or(true); // no author → treat as bot-ish, skip
    if author_bot {
        return None;
    }
    let content = d
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if content.is_empty() {
        return None;
    }
    Some(DiscordMessage {
        message_id: id,
        channel_id,
        author_id: d
            .pointer("/author/id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .into(),
        content: content.to_string(),
    })
}

/// Close-code classification: resumable codes we should reconnect+resume for.
pub fn is_resumable_close(code: u16) -> bool {
    matches!(
        code,
        1000..=1006 | 4000..=4010 // generic + server-side/gateway errors
    ) && code != 4004 // auth failure is NOT resumable
}
