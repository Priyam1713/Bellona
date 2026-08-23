//! Anthropic Messages API client.

use crate::post_json;
use async_trait::async_trait;
use bellum::{BellumError, ModelClient, ModelReply};
use serde_json::json;

pub struct AnthropicClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    tier: &'static str,
    max_tokens: u32,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, tier: &'static str) -> Self {
        AnthropicClient {
            http: reqwest::Client::new(),
            base_url: ANTHROPIC_URL.to_string(),
            api_key: api_key.into(),
            model: model.into(),
            tier,
            max_tokens: 2048,
        }
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Point at an alternate endpoint (tests, proxies, gateways).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub(crate) fn transport(&self) -> &reqwest::Client {
        &self.http
    }

    pub(crate) fn headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("x-api-key", self.api_key.clone()),
            ("anthropic-version", "2023-06-01".to_string()),
        ]
    }

    pub(crate) fn request_body(&self, prompt: &str) -> serde_json::Value {
        json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "messages": [{"role": "user", "content": prompt}],
        })
    }
}

pub(crate) const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";

#[async_trait]
impl ModelClient for AnthropicClient {
    fn tier(&self) -> &'static str {
        self.tier
    }

    async fn complete(&self, prompt: &str) -> Result<ModelReply, BellumError> {
        let resp = post_json(
            self.transport(),
            &self.base_url,
            &self.headers(),
            &self.request_body(prompt),
        )
        .await?;
        let content = resp["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        Ok(crate::openai::OpenAiCompatClient::parse_reply(&content))
    }
}
