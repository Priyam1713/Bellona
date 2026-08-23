use auxilia::{AnthropicClient, OpenAiCompatClient};
use bellum::ModelClient;
use httpmock::prelude::*;

fn serve_openai(content: &str) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/chat/completions");
        then.status(200).json_body_obj(&serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": content}}]
        }));
    });
    server
}

#[tokio::test]
async fn openai_compat_strict_json_becomes_tool_call() {
    let server = serve_openai(
        r#"{"thought":"need the notes","tool":{"name":"read_notes","args":{"path":"n.txt"}}}"#,
    );
    let client = OpenAiCompatClient::new(server.url("/"), Some("k".into()), "m", "terra");
    let reply = client.complete("go").await.unwrap();
    assert_eq!(reply.tool_calls.len(), 1);
    assert_eq!(reply.tool_calls[0].name, "read_notes");
    assert_eq!(reply.tool_calls[0].args["path"], "n.txt");
}

#[tokio::test]
async fn openai_compat_chatty_json_is_extracted() {
    let server = serve_openai(
        "Sure! Here is my plan:\n```json\n{\"thought\":\"hmm\",\"final_answer\":\"42\"}\n```\nDone.",
    );
    let client = OpenAiCompatClient::new(server.url("/"), None, "m", "luna");
    let reply = client.complete("go").await.unwrap();
    assert_eq!(reply.final_answer.as_deref(), Some("42"));
}

#[tokio::test]
async fn openai_compat_plain_text_becomes_thought() {
    let server = serve_openai("I should look at the workspace first.");
    let client = OpenAiCompatClient::new(server.url("/"), None, "m", "terra");
    let reply = client.complete("go").await.unwrap();
    assert!(reply.final_answer.is_none());
    assert!(reply.tool_calls.is_empty());
    assert!(reply.thought.contains("workspace"));
}

#[tokio::test]
async fn auth_header_sent_when_key_present() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/chat/completions")
            .header("Authorization", "Bearer sk-test");
        then.status(200).json_body_obj(&serde_json::json!({
            "choices": [{"message": {"content": "{\"final_answer\":\"ok\"}"}}]
        }));
    });
    let client = OpenAiCompatClient::new(server.url("/"), Some("sk-test".into()), "m", "sol");
    let reply = client.complete("go").await.unwrap();
    assert_eq!(reply.final_answer.as_deref(), Some("ok"));
}

#[tokio::test]
async fn anthropic_shape_parsed_and_headers_sent() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/messages")
            .header("x-api-key", "ak-test")
            .header("anthropic-version", "2023-06-01");
        then.status(200).json_body_obj(&serde_json::json!({
            "content": [{"type": "text", "text": "{\"final_answer\":\"hail\"}"}]
        }));
    });
    let client = AnthropicClient::new("ak-test", "claude-x", "sol")
        .with_base_url(server.url("/v1/messages"));
    let reply = client.complete("go").await.unwrap();
    assert_eq!(reply.final_answer.as_deref(), Some("hail"));
}
