//! # nuntii â€” the messengers.
//!
//! Channel transports that carry Bellona's voice to humans where they
//! already are. Telegram long-polling first; Discord/Slack follow the same
//! shape. Transports carry *messages*, never camp credentials.

pub mod discord;
pub mod slack;
pub mod transport;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum NuntiiError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("bad payload: {0}")]
    Payload(String),
}

pub type Result<T> = std::result::Result<T, NuntiiError>;

pub use transport::ChannelTransport;

/// One inbound message worth an agent's attention.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Inbound {
    pub update_id: u64,
    pub chat_id: i64,
    pub text: String,
}

/// Telegram Bot API long-poll transport.
pub struct TelegramTransport {
    http: reqwest::Client,
    token: String,
    offset: u64,
    base_url: String,
}

impl TelegramTransport {
    pub fn new(token: impl Into<String>) -> Self {
        TelegramTransport {
            http: reqwest::Client::new(),
            token: token.into(),
            offset: 0,
            base_url: "https://api.telegram.org".into(),
        }
    }

    /// Alternate endpoint (tests, local gateways).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    #[doc(hidden)]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn api(&self, method: &str) -> String {
        format!("{}/bot{}/{}", self.base_url, self.token, method)
    }

    /// Poll once; returns inbound messages and advances the internal offset.
    pub async fn poll(&mut self, timeout_secs: u64) -> Result<Vec<Inbound>> {
        let resp = self
            .http
            .post(self.api("getUpdates"))
            .json(&serde_json::json!({
                "timeout": timeout_secs,
                "offset": self.offset,
                "allowed_updates": ["message"],
            }))
            .send()
            .await
            .map_err(|e| NuntiiError::Transport(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .map_err(|e| NuntiiError::Transport(e.to_string()))?;
        if status != 200 {
            return Err(NuntiiError::Transport(format!("telegram {status}: {body}")));
        }
        let mut updates = parse_updates(&body)?;
        // Defensive dedupe: never re-deliver below our high-water mark
        // (servers should honor offset; we don't bet on it).
        updates.retain(|u| u.update_id >= self.offset);
        if let Some(max) = updates.iter().map(|u| u.update_id).max() {
            self.offset = max + 1;
        }
        Ok(updates)
    }

    /// Reply into a chat (chunked under Telegram's 4096-char cap).
    pub async fn send(&self, chat_id: i64, text: &str) -> Result<()> {
        for chunk in split_chunks(text, 4000) {
            let resp = self
                .http
                .post(self.api("sendMessage"))
                .json(&serde_json::json!({ "chat_id": chat_id, "text": chunk }))
                .send()
                .await
                .map_err(|e| NuntiiError::Transport(e.to_string()))?;
            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(NuntiiError::Transport(format!("send failed: {body}")));
            }
        }
        Ok(())
    }
}

fn split_chunks(text: &str, cap: usize) -> Vec<String> {
    if text.len() <= cap {
        return vec![text.to_string()];
    }
    text.as_bytes()
        .chunks(cap)
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect()
}

/// Parse a getUpdates payload; tolerant of non-message updates.
pub fn parse_updates(body: &str) -> Result<Vec<Inbound>> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| NuntiiError::Payload(e.to_string()))?;
    let arr = v
        .get("result")
        .and_then(|r| r.as_array())
        .ok_or_else(|| NuntiiError::Payload("missing result[]".into()))?;
    let mut out = Vec::new();
    for u in arr {
        let update_id = u.get("update_id").and_then(|x| x.as_u64());
        let msg = u.get("message");
        let chat_id = msg
            .and_then(|m| m.get("chat"))
            .and_then(|c| c.get("id"))
            .and_then(|i| i.as_i64());
        let text = msg
            .and_then(|m| m.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string();
        if let (Some(uid), Some(cid)) = (update_id, chat_id) {
            if !text.is_empty() {
                out.push(Inbound {
                    update_id: uid,
                    chat_id: cid,
                    text,
                });
            }
        }
    }
    Ok(out)
}
