//! # auxilia — the allied troops.
//!
//! Model providers and channel transports serve Bellona but owe it no
//! allegiance: each is swappable behind bellum's traits (Laws II/III).

pub mod anthropic;
pub mod openai;

pub use anthropic::AnthropicClient;
pub use openai::OpenAiCompatClient;

use bellum::BellumError;

/// Errors from the allies.
#[derive(Debug, thiserror::Error)]
pub enum AuxiliaError {
    #[error("transport error: {0}")]
    Transport(String),

    #[error("provider returned {status}: {body}")]
    Provider { status: u16, body: String },
}

impl From<AuxiliaError> for BellumError {
    fn from(e: AuxiliaError) -> Self {
        BellumError::Model(e.to_string())
    }
}

/// Shared HTTP posting with consistent error mapping.
pub(crate) async fn post_json(
    client: &reqwest::Client,
    url: &str,
    headers: &[(&str, String)],
    body: &serde_json::Value,
) -> Result<serde_json::Value, AuxiliaError> {
    let mut req = client.post(url).json(body);
    for (k, v) in headers {
        req = req.header(*k, v);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| AuxiliaError::Transport(e.to_string()))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| AuxiliaError::Transport(e.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(AuxiliaError::Provider { status, body: text });
    }
    serde_json::from_str(&text).map_err(|e| AuxiliaError::Provider {
        status,
        body: format!("bad json from provider: {e}"),
    })
}
