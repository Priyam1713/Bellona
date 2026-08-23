//! OpenAI-compatible chat-completions client.
//!
//! Speaks to OpenAI, OpenRouter, vLLM, LM Studio, and **Ollama** (same shape
//! at `http://localhost:11434/v1`). One client, many realms — Law III.

use crate::post_json;
use async_trait::async_trait;
use bellum::{BellumError, ModelClient, ModelReply, ToolCall};
use serde_json::json;

pub struct OpenAiCompatClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
    tier: &'static str,
}

impl OpenAiCompatClient {
    /// `base_url` like `https://api.openai.com/v1` or
    /// `http://localhost:11434/v1` (Ollama). Empty key is fine locally.
    pub fn new(
        base_url: impl Into<String>,
        api_key: Option<String>,
        model: impl Into<String>,
        tier: &'static str,
    ) -> Self {
        OpenAiCompatClient {
            http: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            model: model.into(),
            tier,
        }
    }

    pub(crate) fn transport(&self) -> &reqwest::Client {
        &self.http
    }

    pub(crate) fn url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    pub(crate) fn headers(&self) -> Vec<(&'static str, String)> {
        match &self.api_key {
            Some(k) if !k.is_empty() => vec![("Authorization", format!("Bearer {k}"))],
            _ => vec![],
        }
    }

    pub(crate) fn request_body(&self, prompt: &str) -> serde_json::Value {
        json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.2,
        })
    }

    /// Map our reply schema onto whatever the model returned, tolerantly:
    /// models emit either strict JSON per instruction or chatty text with a
    /// JSON object inside — we extract the last balanced object.
    pub fn parse_reply(content: &str) -> ModelReply {
        if let Some(obj_text) = last_balanced_json(content.trim()) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&obj_text) {
                let thought = v
                    .get("thought")
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .to_string();
                if let Some(ans) = v.get("final_answer").and_then(|a| a.as_str()) {
                    return ModelReply {
                        thought,
                        tool_calls: vec![],
                        final_answer: Some(ans.to_string()),
                        cost_cents: 0,
                    };
                }
                if let Some(tool) = v.get("tool") {
                    let name = tool
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if !name.is_empty() {
                        return ModelReply {
                            thought,
                            tool_calls: vec![ToolCall {
                                name,
                                args: tool.get("args").cloned().unwrap_or(json!({})),
                            }],
                            final_answer: None,
                            cost_cents: 0,
                        };
                    }
                }
            }
        }
        // No usable JSON found: the whole content is a thought.
        ModelReply {
            thought: content.to_string(),
            tool_calls: vec![],
            final_answer: None,
            cost_cents: 0,
        }
    }
}

/// Find the last balanced `{...}` block in text.
fn last_balanced_json(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    for end in (0..bytes.len()).rev() {
        if bytes[end] != b'}' {
            continue;
        }
        let mut depth = 0usize;
        let mut in_str = false;
        let mut escape = false;
        for i in (0..=end).rev() {
            let b = bytes[i];
            if escape {
                escape = false;
                continue;
            }
            match b {
                b'\\' if in_str => escape = true,
                b'"' => in_str = !in_str,
                b'}' if !in_str => depth += 1,
                b'{' if !in_str => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(s[i..=end].to_string());
                    }
                }
                _ => {}
            }
        }
        return None;
    }
    None
}

#[async_trait]
impl ModelClient for OpenAiCompatClient {
    fn tier(&self) -> &'static str {
        self.tier
    }

    async fn complete(&self, prompt: &str) -> Result<ModelReply, BellumError> {
        let resp = post_json(
            self.transport(),
            &self.url(),
            &self.headers(),
            &self.request_body(prompt),
        )
        .await?;
        let content = resp["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        Ok(Self::parse_reply(&content))
    }
}
