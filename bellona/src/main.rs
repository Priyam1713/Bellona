//! bellona ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â the war machine's terminal face.
//!
//! Usage:
//!   bellona [--workspace DIR] [--base-url URL] [--api-key KEY] --goal "..."
//!           [--model NAME] [--yolo] [--allow-shell] [--max-steps N]
//!
//! Defaults speak Ollama at localhost:11434 (OpenAI-compatible). No cloud,
//! no key, no hostages (Law III).

use bellum::{Aerarium, CascadeRouter, ReActStrategy, WarLoop};
use std::path::PathBuf;

fn arg_of(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn flag(args: &[String], key: &str) -> bool {
    args.iter().any(|a| a == key)
}

async fn serve(args: &[String]) {
    let bind = args
        .iter()
        .position(|a| a == "--bind")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "127.0.0.1:3001".into());
    let cfg_dir = std::env::current_dir().unwrap_or_default();
    let wrcfg = bellona::BellonaConfig {
        workspace: cfg_dir.clone(),
        ..Default::default()
    };
    let assembled = match bellona::assemble(&wrcfg) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("bellona serve: {e}");
            std::process::exit(2);
        }
    };
    let model = bellona::model_client(&wrcfg);
    let app = bellona::warroom::router(bellona::warroom::WarRoom {
        assembled,
        cfg: wrcfg,
        model,
    });
    eprintln!("bellona: war room open at http://{bind}");
    let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.first().map(|a| a == "serve").unwrap_or(false) {
        serve(&args[1..]).await;
        return;
    }

    if args.first().map(|a| a == "receipts").unwrap_or(false) {
        let mut reports = Vec::new();
        for (i, a) in args.iter().enumerate() {
            if a == "--report" {
                if let Some(path) = args.get(i + 1) {
                    let raw = std::fs::read_to_string(path).expect("report file");
                    let rf: bellona::receipts::ReportFile =
                        serde_json::from_str(&format!("{{\"report\":{raw}}}"))
                            .expect("report shape");
                    reports.push(rf);
                }
            }
        }
        let md = bellona::receipts::render(&reports, env!("CARGO_PKG_VERSION"));
        println!("{md}");
        return;
    }
    if args.first().map(|a| a == "colosseum").unwrap_or(false) {
        let cfg = bellona::BellonaConfig::default();
        let code = bellona::colosseum::cli(&args[1..], &cfg).await;
        std::process::exit(code);
    }

    if flag(&args, "--help") || flag(&args, "-h") {
        println!(
            "bellona ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â the war machine\n\
             \n\
             USAGE:\n  \
               bellona [flags] --goal \"...\"\n\
             \n\
             FLAGS:\n  \
               --workspace DIR   campaign root (default: .)\n  \
               --base-url URL    OpenAI-compatible endpoint (default: Ollama http://localhost:11434/v1)\n  \
               --api-key KEY     bearer token; omit for local servers\n  \
               --model NAME      model id (default: local-model)\n  \
               --yolo            auto-approve writes (still audited)\n  \
               --allow-shell     permit run_shell (gated unless --yolo)\n  \
               --max-steps N     aerarium ceiling (default: 24)"
        );
        return;
    }

    let goal =
        arg_of(&args, "--goal").or_else(|| args.iter().find(|a| !a.starts_with("--")).cloned());

    let cfg = bellona::BellonaConfig {
        workspace: PathBuf::from(arg_of(&args, "--workspace").unwrap_or_else(|| ".".into())),
        base_url: arg_of(&args, "--base-url").unwrap_or_else(|| "http://localhost:11434/v1".into()),
        api_key: arg_of(&args, "--api-key"),
        model: arg_of(&args, "--model").unwrap_or_else(|| "local-model".into()),
        yolo: flag(&args, "--yolo"),
        allow_shell: flag(&args, "--allow-shell"),
        max_steps: arg_of(&args, "--max-steps")
            .and_then(|v| v.parse().ok())
            .unwrap_or(24),
    };

    let Some(goal) = goal else {
        eprintln!("bellona: no goal given. Try `bellona --help`.");
        std::process::exit(2);
    };

    let assembled = match bellona::assemble(&cfg) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("bellona: {e}");
            std::process::exit(2);
        }
    };

    let client = bellona::model_client(&cfg);
    let router = CascadeRouter::new(vec![client]);

    let auto = if cfg.yolo {
        Some("cli-operator".to_string())
    } else {
        None
    };
    let loop_ = WarLoop::new(
        assembled.gateway.clone(),
        assembled.registry.clone(),
        router,
        Aerarium::new(cfg.max_steps, 500),
    );
    let loop_ = match auto {
        Some(a) => loop_.with_auto_approver(a),
        None => loop_,
    };

    // Surface the event stream on stderr ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â stdout stays for the answer.
    let mut rx = assembled.gateway.bus().subscribe();
    let ticker = tokio::spawn(async move {
        while let Ok(ev) = rx.recv().await {
            eprintln!("[{ev:?}]");
        }
    });

    eprintln!("bellona: marching. goal = {goal:?}");
    let report = loop_
        .run(
            &goal,
            Box::new(ReActStrategy::new(goal.clone(), cfg.max_steps)),
            None,
        )
        .await;

    ticker.abort();

    match report {
        Ok(r) => {
            if r.ok {
                println!("{}", r.answer);
                std::process::exit(0);
            } else {
                eprintln!(
                    "bellona: halted by breaker ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â {}",
                    r.breaker.as_deref().unwrap_or("unknown")
                );
                println!("{}", r.answer);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("bellona: campaign failed: {e}");
            std::process::exit(2);
        }
    }
}
