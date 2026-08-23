//! Colosseum mode Ã¢â‚¬â€ `bellona colosseum --suite FILE [--offline]`.
//!
//! Runs each suite case k times through the full war loop (real tools, real
//! gate) and reports pass^k with honest exit codes. Offline mode uses a
//! deterministic echo model: it validates harness plumbing, never model
//! quality Ã¢â‚¬â€ receipts about the machine itself (Law VII).

use bellum::{
    Aerarium, BellumError, CascadeRouter, ModelClient, ModelReply, ReActStrategy, ToolCall, WarLoop,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use vigiles::{CaseResult, CaseVerdict, Gate, GateVerdict, SuiteReport};

/// Deterministic task interpreter. Task grammar:
///   `say <text>`                  Ã¢â€ â€™ answers `<text>`
///   `write <rel/path> :: <text>`  Ã¢â€ â€™ writes the file, then answers `wrote`
#[derive(Default)]
pub struct EchoModel {
    last_task: Mutex<Option<String>>,
}

impl EchoModel {
    pub fn new() -> Self {
        Self::default()
    }

    fn extract_goal(prompt: &str) -> String {
        prompt
            .lines()
            .find(|l| l.starts_with("GOAL:"))
            .map(|l| l.trim_start_matches("GOAL:").trim().to_string())
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl ModelClient for EchoModel {
    fn tier(&self) -> &'static str {
        "terra"
    }
    async fn complete(&self, prompt: &str) -> Result<ModelReply, BellumError> {
        let mut slot = self.last_task.lock().unwrap();
        // A fresh goal resets the interpreter; an observation means we just acted.
        let observed = prompt.contains("LAST OBSERVATION:");
        let task = if observed {
            slot.clone().unwrap_or_default()
        } else {
            let g = Self::extract_goal(prompt);
            *slot = Some(g.clone());
            g
        };

        if let Some(rest) = task.strip_prefix("write ") {
            if observed {
                return Ok(ModelReply {
                    thought: String::new(),
                    tool_calls: vec![],
                    final_answer: Some("wrote".into()),
                    cost_cents: 0,
                });
            }
            let (path, content) = match rest.split_once("::") {
                Some((p, c)) => (p.trim(), c.trim().to_string()),
                None => ("note.txt", String::new()),
            };
            return Ok(ModelReply {
                thought: "writing".into(),
                tool_calls: vec![ToolCall {
                    name: "write_file".into(),
                    args: serde_json::json!({ "path": path, "content": content }),
                }],
                final_answer: None,
                cost_cents: 0,
            });
        }

        // Default verb: say
        let said = task.strip_prefix("say ").unwrap_or(&task).to_string();
        Ok(ModelReply {
            thought: String::new(),
            tool_calls: vec![],
            final_answer: Some(said),
            cost_cents: 0,
        })
    }
}

fn load_suite(path: &Path) -> Result<vigiles::SuiteFile, String> {
    let raw = std::fs::read_to_string(path).map(|s| s.trim_start_matches('﻿').to_string())
        .map_err(|e| format!("cannot read suite '{:?}': {e}", path))?;
    serde_json::from_str(&raw).map_err(|e| format!("suite not valid JSON: {e}"))
}

pub struct ColosseumOutcome {
    pub report: SuiteReport,
    pub verdict: GateVerdict,
}

/// Execute the suite. `model` is injected so tests can be hermetic.
pub async fn run_suite(
    cfg: &crate::BellonaConfig,
    suite: vigiles::SuiteFile,
    model: Arc<dyn ModelClient>,
    gate: Gate,
) -> Result<ColosseumOutcome, String> {
    let assembled = crate::assemble(cfg).map_err(|e| e.to_string())?;
    let router = CascadeRouter::new(vec![model]);
    let loop_ = WarLoop::new(
        assembled.gateway.clone(),
        assembled.registry.clone(),
        router,
        Aerarium::new(cfg.max_steps.max(8), gate.max_cost_cents),
    )
    .with_auto_approver("colosseum");

    let mut results: Vec<CaseResult> = Vec::new();
    let mut total_cost: u64 = 0;

    for case in &suite.cases {
        let mut outputs = Vec::with_capacity(case.trials);
        for _ in 0..case.trials {
            // Fresh strategy per trial Ã¢â‚¬â€ no cross-trial memory leakage.
            let report = loop_
                .run(
                    &case.task,
                    Box::new(ReActStrategy::new(case.task.clone(), cfg.max_steps)),
                    None,
                )
                .await
                .map_err(|e| format!("case '{}' failed: {e}", case.id))?;
            total_cost += report.cost_cents;
            outputs.push(report.answer);
        }
        results.push(CaseResult {
            case_id: case.id.clone(),
            outputs,
        });
    }

    let verdicts = verdicts_of(&results, &suite);

    let report = SuiteReport {
        suite_name: suite.name.clone(),
        pass_at_k: vigiles::compute_pass_at_k(&verdicts),
        cases: verdicts,
        total_cost_cents: total_cost,
    };

    let verdict = gate.evaluate(&report);
    Ok(ColosseumOutcome { report, verdict })
}

fn verdicts_of(results: &[CaseResult], suite: &vigiles::SuiteFile) -> Vec<CaseVerdict> {
    results
        .iter()
        .map(|r| {
            let case = suite.cases.iter().find(|c| c.id == r.case_id);
            let flags = r
                .outputs
                .iter()
                .map(|o| case.map(|c| c.verifier.judge(o)).unwrap_or(false))
                .collect::<Vec<_>>();
            let complete = case.map(|c| r.outputs.len() >= c.trials).unwrap_or(false);
            CaseVerdict {
                case_id: r.case_id.clone(),
                passed_at_k: complete && flags.iter().all(|f| *f),
                trial_flags: flags,
            }
        })
        .collect()
}

/// Entry point from main: parse args, run, print, return exit code.
pub async fn cli(args: &[String], cfg: &crate::BellonaConfig) -> i32 {
    let suite_path = arg_of(args, "--suite").unwrap_or_else(|| "suites/seed.json".into());
    let min_passk: f64 = arg_of(args, "--min-passk")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let max_cost: u64 = arg_of(args, "--max-cost-cents")
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);

    let suite_json = match load_suite(Path::new(&suite_path)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("colosseum: {e}");
            return 2;
        }
    };
    let suite: vigiles::SuiteFile = suite_json;

    // XIII: live providers. Offline remains the deterministic default.
    let model: Arc<dyn ModelClient> = if args.iter().any(|a| a == "--offline") {
        Arc::new(EchoModel::new())
    } else {
        let provider = arg_of(args, "--provider").unwrap_or_else(|| "openai".into());
        match provider.as_str() {
            "anthropic" => Arc::new(auxilia::AnthropicClient::new(
                std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
                arg_of(args, "--model").unwrap_or_else(|| "claude-sonnet-4-20250514".into()),
                "sol",
            )),
            "openai" | "ollama" => {
                let base = if provider == "ollama" {
                    "http://localhost:11434/v1".to_string()
                } else {
                    arg_of(args, "--base-url").unwrap_or_else(|| "https://api.openai.com/v1".into())
                };
                Arc::new(auxilia::OpenAiCompatClient::new(
                    base,
                    std::env::var("OPENAI_API_KEY").ok(),
                    arg_of(args, "--model").unwrap_or_else(|| "gpt-4o-mini".into()),
                    "sol",
                ))
            }
            other => {
                eprintln!("colosseum: unknown provider '{other}' (openai|anthropic|ollama)");
                return 2;
            }
        }
    };

    match run_suite(
        cfg,
        suite,
        model,
        Gate {
            min_pass_at_k: min_passk,
            max_cost_cents: max_cost,
        },
    )
    .await
    {
        Ok(outcome) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&outcome.report).unwrap_or_default()
            );
            outcome.verdict.exit_code()
        }
        Err(e) => {
            eprintln!("colosseum: {e}");
            2
        }
    }
}

fn arg_of(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

// Keep PathBuf import used for suite loading signature clarity.
#[allow(unused)]
fn _p(p: PathBuf) -> PathBuf {
    p
}
