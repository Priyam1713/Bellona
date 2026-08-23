//! Campaign VI codec tests — the protocol decisions, proven without sockets.

use nuntii::discord::*;
use nuntii::slack::*;

#[test]
fn discord_hello_yields_interval_and_identify_carries_token() {
    let hello = r#"{"op":10,"d":{"heartbeat_interval":41250,"_trace":["x"]}}"#;
    let frame = parse_frame(hello).unwrap().unwrap();
    assert_eq!(frame.op, Op::Hello);
    assert_eq!(hello_interval(&frame), Some(41250));

    let id = identify_payload("SECRET-TOKEN");
    assert!(id.contains("SECRET-TOKEN"));
    assert!(id.contains("\"op\":2"));
}

#[test]
fn dispatch_frames_carry_seq_and_type() {
    let raw = serde_json::json!({
        "op": 0, "t": "MESSAGE_CREATE", "s": 42,
        "d": {"id": "m1", "channel_id": "c1", "content": "hail",
              "author": {"id": "u1"}}
    })
    .to_string();
    let frame = parse_frame(&raw).unwrap().unwrap();
    assert_eq!(frame.op, Op::Dispatch);
    assert_eq!(frame.t.as_deref(), Some("MESSAGE_CREATE"));
    assert_eq!(frame.s, Some(42));

    let msg = parse_message_create(&frame.data).unwrap();
    assert_eq!(msg.channel_id, "c1");
    assert_eq!(msg.content, "hail");
}

#[test]
fn bot_messages_and_empties_are_skipped() {
    let bot_msg = serde_json::json!({
        "id": "m2", "channel_id": "c1", "content": "beep",
        "author": {"id": "b1", "bot": true}
    });
    assert!(parse_message_create(&bot_msg).is_none());

    let empty = serde_json::json!({
        "id": "m3", "channel_id": "c1", "content": "",
        "author": {"id": "u9"}
    });
    assert!(parse_message_create(&empty).is_none());
}

#[test]
fn heartbeat_and_close_semantics_are_correct() {
    assert_eq!(heartbeat_payload(Some(42)), r#"{"d":42,"op":1}"#);
    assert_eq!(heartbeat_payload(None), r#"{"d":null,"op":1}"#);

    // Auth failure is fatal; transport-level closes are resumable.
    assert!(!is_resumable_close(4004));
    assert!(is_resumable_close(4000));
    assert!(is_resumable_close(1006));
}

#[test]
fn slack_envelopes_ack_and_dedupe_bot_events() {
    let env = serde_json::json!({
        "type": "events_api",
        "envelope_id": "env-7",
        "payload": {
            "event_id": "Ev1",
            "event": {"channel": "C1", "text": "storm the hill", "user": {"id": "U1"}}
        }
    })
    .to_string();

    assert!(is_events_api(&env));
    assert_eq!(ack_for(&env).unwrap(), r#"{"envelope_id":"env-7"}"#);

    // Non-events frames are not ACKed.
    assert!(!is_events_api(r#"{"type":"hello"}"#));

    let msg = parse_event(&env).unwrap();
    assert_eq!(msg.event_id, "Ev1");
    assert_eq!(msg.channel, "C1");
    assert!(!msg.from_bot);

    // Bot-authored events are skipped (loop prevention).
    let bot_env = serde_json::json!({
        "type": "events_api", "envelope_id": "e2",
        "payload": {"event_id": "Ev2",
            "event": {"channel": "C1", "bot_id": "B1", "text": "auto"}}
    })
    .to_string();
    let m = parse_event(&bot_env).unwrap();
    assert!(m.from_bot);
}
