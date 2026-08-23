use httpmock::prelude::*;
use nuntii::{parse_updates, Inbound, TelegramTransport};

#[test]
fn update_parsing_extracts_only_text_messages() {
    let body = serde_json::json!({
        "ok": true,
        "result": [
            {"update_id": 1, "message": {"chat": {"id": 77}, "text": "march on rome"}},
            {"update_id": 2, "message": {"chat": {"id": 77}}},
            {"update_id": 3},
            {"update_id": 4, "message": {"chat": {"id": 88}, "text": ""}},
            {"update_id": 5, "message": {"chat": {"id": 99}, "text": "second front"}}
        ]
    })
    .to_string();

    let got = parse_updates(&body).unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(
        got[0],
        Inbound {
            update_id: 1,
            chat_id: 77,
            text: "march on rome".into()
        }
    );
    assert_eq!(got[1].chat_id, 99);
}

#[tokio::test]
async fn telegram_poll_and_reply_round_trip() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path_contains("/getUpdates");
        then.status(200).json_body(serde_json::json!({
            "ok": true,
            "result": [
                {"update_id": 9, "message": {"chat": {"id": 42}, "text": "report"}}
            ]
        }));
    });
    let send_mock = server.mock(|when, then| {
        when.method(POST)
            .path_contains("/sendMessage")
            .body_contains("hail the victor");
        then.status(200).body("{}");
    });

    let mut t = TelegramTransport::new("TESTTOKEN").with_base_url(server.url(""));
    let inbound = t.poll(0).await.unwrap();
    assert_eq!(inbound.len(), 1);
    assert_eq!(inbound[0].chat_id, 42);

    t.send(42, "hail the victor").await.unwrap();
    assert_eq!(send_mock.hits(), 1);

    // Offset advanced past update 9 → next poll is empty.
    let none = t.poll(0).await.unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn long_messages_are_chunked_under_telegram_cap() {
    let server = MockServer::start();
    let big = "x".repeat(9000);

    let chunked_send = server.mock(|when, then| {
        when.method(POST)
            .path_contains("/sendMessage")
            .body_contains("xxxxx");
        then.status(200).body("{}");
    });
    server.mock(|when, then| {
        when.method(POST).path_contains("/getUpdates");
        then.status(200)
            .json_body(serde_json::json!({"ok": true, "result": []}));
    });

    let mut t = TelegramTransport::new("T").with_base_url(server.url(""));
    let _ = t.poll(0).await.unwrap();
    t.send(1, &big).await.unwrap();
    assert!(chunked_send.hits() >= 2, ">4096 payload must be split");
}
