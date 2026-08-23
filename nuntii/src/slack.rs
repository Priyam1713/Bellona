//! VI.2 Ã¢â‚¬â€ Slack Socket Mode: envelope ACK + event dedupe logic as pure,
//! unit-tested functions (the wss I/O layer stays thin).

use serde::{Deserialize, Serialize};

/// Parse raw socket text; returns None for frames we must not ACK (keepalives).
pub fn is_events_api(raw: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| {
            v.get("type")
                .and_then(|x| x.as_str())
                .map(|s| s == "events_api")
        })
        .unwrap_or(false)
}

/// Build the ACK frame for an events_api envelope.
pub fn ack_for(raw: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let envelope_id = v
        .get("envelope_id")
        .and_then(|x| x.as_str())
        .ok_or("missing envelope_id")?
        .to_string();
    Ok(serde_json::json!({ "envelope_id": envelope_id }).to_string())
}

/// Extract the app_mention/message event from an events_api envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlackMessage {
    pub event_id: String,
    pub channel: String,
    pub user: String,
    pub text: String,
    /// bot_id present on bot-authored events Ã¢â€ â€™ skip to avoid loops.
    #[serde(default)]
    pub from_bot: bool,
}

pub fn parse_event(raw: &str) -> Option<SlackMessage> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let event = v.get("payload")?.get("event")?;
    let event_id = v
        .pointer("/payload/event_id")
        .and_then(|x| x.as_str())?
        .to_string();
    let channel = event
        .get("channel")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    if channel.is_empty() {
        return None;
    }
    let text = event
        .get("text")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    if text.is_empty() {
        return None;
    }
    Some(SlackMessage {
        event_id,
        channel,
        user: event
            .pointer("/user/id")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .into(),
        text,
        from_bot: event.get("bot_id").is_some(),
    })
}

