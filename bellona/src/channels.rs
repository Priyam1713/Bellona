//! Campaign XIV-1 Ã¢â‚¬â€ the daemons: your agent where you already are.
//!
//! `bellona telegram` / `bellona discord` run long-lived loops binding a
//! ChannelTransport to the full war machine (gate, law, audit included).
//! Every inbound chat message becomes a campaign; every reply is chunked,
//! attributed, and audited like everything else.

use bellona::{assemble, model_client, BellonaConfig};
use bellum::{Aerarium, CascadeRouter, ReActStrategy, WarLoop};
use nuntii::Inbound;
use std::sync::Arc;

/// Serialize campaigns within one daemon process Ã¢â‚¬â€ a chat is not a
/// thundering herd.
const MAX_CONCURRENT_CAMPAIGNS: usize = 2;

fn build_loop(
    cfg: &BellonaConfig,
    assembled: &bellona::Assembled,
    model: Arc<dyn bellum::ModelClient>,
) -> WarLoop<praetorium::custos::SnapshotResolver, bellona::RegistryExecutor> {
    let loop_ = WarLoop::new(
        assembled.gateway.clone(),
        assembled.registry.clone(),
        CascadeRouter::new(vec![model]),
        Aerarium::new(cfg.max_steps, 500),
    );
    if cfg.yolo {
        loop_.with_auto_approver("channel-operator")
    } else {
        loop_
    }
}

// ---------- telegram ----------

pub async fn run_telegram(cfg: BellonaConfig, token: String) -> i32 {
    let assembled = match assemble(&cfg) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("bellona telegram: {e}");
            return 2;
        }
    };
    let _model = model_client(&cfg);
    let mut transport = nuntii::TelegramTransport::new(token);

    eprintln!(
        "bellona telegram: online. workspace={} yolo={} writes-park={}",
        cfg.workspace.display(),
        cfg.yolo,
        !cfg.yolo
    );
    let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CAMPAIGNS));
    loop {
        match transport.poll(50).await {
            Ok(inbounds) => {
                for Inbound { chat_id, text, .. } in inbounds {
                    let permit = match semaphore.clone().acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => continue,
                    };
                    let cfg = cfg.clone();
                    let assembled = assembled.clone();
                    let model = model_client(&cfg);
                    let t2 = nuntii::TelegramTransport::new(
                        std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default(),
                    );
                    tokio::spawn(async move {
                        let loop_ = build_loop(&cfg, &assembled, model);
                        let report = loop_
                            .run(
                                &text,
                                Box::new(ReActStrategy::new(text.clone(), cfg.max_steps)),
                                None,
                            )
                            .await;
                        let reply = match report {
                            Ok(r) if r.ok => r.answer,
                            Ok(r) => format!(
                                "Ã¢Å¡Â  halted: {}\n{}",
                                r.breaker.as_deref().unwrap_or("unknown"),
                                r.answer
                            ),
                            Err(e) => format!("Ã¢Å¡Â  campaign failed: {e}"),
                        };
                        let _ = t2.send(chat_id, &reply).await;
                        drop(permit);
                    });
                }
            }
            Err(e) => {
                eprintln!("bellona telegram: poll error: {e} Ã¢â‚¬â€ retrying in 5s");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

// ---------- discord ----------

struct DiscordRest {
    http: reqwest::Client,
    token: String,
}

impl DiscordRest {
    async fn send(&self, channel_id: &str, text: &str) -> Result<(), String> {
        // 2000-char platform cap, chunked.
        for chunk in split_chunks(text, 1900) {
            let resp = self
                .http
                .post(format!(
                    "https://discord.com/api/v10/channels/{channel_id}/messages"
                ))
                .header("authorization", format!("Bot {}", self.token))
                .json(&serde_json::json!({ "content": chunk }))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!(
                    "discord send {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                ));
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

pub async fn run_discord(cfg: BellonaConfig, token: String) -> i32 {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    let assembled = match assemble(&cfg) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("bellona discord: {e}");
            return 2;
        }
    };
    let rest = DiscordRest {
        http: reqwest::Client::new(),
        token: token.clone(),
    };
    let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CAMPAIGNS));

    eprintln!("bellona discord: connecting to gatewayÃ¢â‚¬Â¦");
    'reconnect: loop {
        let (ws, _) =
            match tokio_tungstenite::connect_async("wss://gateway.discord.gg/?v=10&encoding=json")
                .await
            {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("bellona discord: gateway connect failed: {e} Ã¢â‚¬â€ retry 5s");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };
        let (mut sink, mut stream) = ws.split();

        // Frame 1 must be HELLO.
        let hello_interval_ms: u64 = loop {
            match stream.next().await {
                Some(Ok(Message::Text(raw))) => {
                    if let Some(frame) = nuntii::discord::parse_frame(&raw).unwrap_or(None) {
                        if let Some(ms) = nuntii::discord::hello_interval(&frame) {
                            break ms.max(5_000);
                        }
                    }
                }
                Some(Ok(_)) => continue,
                _ => {
                    eprintln!("bellona discord: no HELLO Ã¢â‚¬â€ reconnecting");
                    continue 'reconnect;
                }
            }
        };

        if sink
            .send(Message::Text(nuntii::discord::identify_payload(&token)))
            .await
            .is_err()
        {
            continue 'reconnect;
        }

        let mut heartbeat =
            tokio::time::interval(std::time::Duration::from_millis(hello_interval_ms));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_seq: Option<u64> = None;

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    if sink.send(Message::Text(nuntii::discord::heartbeat_payload(last_seq))).await.is_err() {
                        continue 'reconnect;
                    }
                }
                frame = stream.next() => {
                    let Some(Ok(Message::Text(raw))) = frame else { continue 'reconnect };
                    let Some(frame) = nuntii::discord::parse_frame(&raw).unwrap_or(None) else { continue };

                    match frame.op {
                        nuntii::discord::Op::HeartbeatAck => {}
                        nuntii::discord::Op::Heartbeat => {
                            if sink.send(Message::Text(nuntii::discord::heartbeat_payload(last_seq))).await.is_err() {
                                continue 'reconnect;
                            }
                        }
                        nuntii::discord::Op::Dispatch => {
                            if let Some(s) = frame.s { last_seq = Some(s); }
                            if frame.t.as_deref() == Some("MESSAGE_CREATE") {
                                if let Some(m) = nuntii::discord::parse_message_create(&frame.data) {
                                    let permit = match semaphore.clone().acquire_owned().await {
                                        Ok(p) => p,
                                        Err(_) => continue,
                                    };
                                    let cfg = cfg.clone();
                                    let assembled = assembled.clone();
                                    let model = model_client(&cfg);
                                    let rest = DiscordRest {
                                        http: rest.http.clone(),
                                        token: token.clone(),
                                    };
                                    tokio::spawn(async move {
                                        let loop_ = build_loop(&cfg, &assembled, model);
                                        let report = loop_
                                            .run_as(&m.content,
                                                Box::new(ReActStrategy::new(m.content.clone(), cfg.max_steps)),
                                                None,
                                                Some("discord".into()))
                                            .await;
                                        let reply = match report {
                                            Ok(r) if r.ok => r.answer,
                                            Ok(r) => format!("Ã¢Å¡Â  halted: {}\n{}", r.breaker.as_deref().unwrap_or("?"), r.answer),
                                            Err(e) => format!("Ã¢Å¡Â  failed: {e}"),
                                        };
                                        let _ = rest.send(&m.channel_id, &reply).await;
                                        drop(permit);
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
