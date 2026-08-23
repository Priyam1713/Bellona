//! VI.0 — the unified transport contract. Telegram becomes adapter #1, not
//! a special case; Discord and Slack implement the same two verbs.

use crate::{Inbound, NuntiiError, Result};
use async_trait::async_trait;

/// A channel transport: pull inbound mentions, push replies.
///
/// Contract (Law IV applied to channels):
/// - `poll` MUST NOT re-deliver below the caller's acknowledged high-water
///   mark (transports own their dedupe).
/// - `send` MUST chunk to platform limits itself.
#[async_trait]
pub trait ChannelTransport: Send + Sync {
    /// Human-readable platform name ("telegram", "discord", "slack").
    fn platform(&self) -> &'static str;

    async fn poll(&mut self, timeout_secs: u64) -> Result<Vec<Inbound>>;

    /// `chat_id` is platform-shaped (i64 chat / snowball channel id string).
    async fn send(&self, chat_id: &str, text: &str) -> Result<()>;
}

// Telegram adapter conforms:
#[async_trait]
impl ChannelTransport for crate::TelegramTransport {
    fn platform(&self) -> &'static str {
        "telegram"
    }
    async fn poll(&mut self, timeout_secs: u64) -> Result<Vec<Inbound>> {
        crate::TelegramTransport::poll(self, timeout_secs).await
    }
    async fn send(&self, chat_id: &str, text: &str) -> Result<()> {
        let id: i64 = chat_id
            .parse()
            .map_err(|_| NuntiiError::Payload(format!("bad telegram chat id '{chat_id}'")))?;
        crate::TelegramTransport::send(self, id, text).await
    }
}
