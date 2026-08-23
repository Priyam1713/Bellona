//! Campaign VIII â€” Centurio: one plan, many specialists, zero chaos.
//!
//! The Legion composes ordinary WarLoops: every worker is a full agent
//! (own identity, own session, own sub-budget) whose effects still pass
//! the Praetorian Gate stamped with `attr.worker.role` â€” so fleet-level law
//! can say "researchers read, writers write".

use crate::model::ModelClient;
use crate::{Aerarium, RunReport, Strategy, WarLoop};
use praetorium::custos::TargetResolver;
use praetorium::custos::{CustosGateway, EffectExecutor};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// One specialist assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerSpec {
    /// Role tag â€” becomes `attr.worker.role` for fleet-level Lex rules.
    pub role: String,
    /// The worker's own goal statement.
    pub goal: String,
}

impl WorkerSpec {
    pub fn new(role: &str, goal: impl Into<String>) -> Self {
        WorkerSpec {
            role: role.to_string(),
            goal: goal.into(),
        }
    }
}

/// Errors from fleet orchestration.
#[derive(Debug, thiserror::Error)]
pub enum LegionError {
    #[error("empty legion: no workers in plan")]
    Empty,
    #[error("worker '{role}' failed: {source}")]
    Worker {
        role: String,
        source: crate::BellumError,
    },
}

pub struct Legion<R, E>
where
    R: TargetResolver,
    E: EffectExecutor,
{
    gateway: Arc<CustosGateway<R, E>>,
    registry: Arc<forge::tool::ToolRegistry>,
    model: Arc<dyn ModelClient>,
    /// Hard ceiling on workers (recursive-spawn guard lives at the caller).
    max_workers: usize,
}

impl<R: TargetResolver, E: EffectExecutor> Legion<R, E> {
    pub fn new(
        gateway: Arc<CustosGateway<R, E>>,
        registry: Arc<forge::tool::ToolRegistry>,
        model: Arc<dyn ModelClient>,
    ) -> Self {
        Legion {
            gateway,
            registry,
            model,
            max_workers: 8,
        }
    }

    pub fn with_max_workers(mut self, n: usize) -> Self {
        self.max_workers = n.max(1);
        self
    }

    /// Execute the plan sequentially; each worker gets a sub-budget carved
    /// from the parent Aerarium. Overruns trip that worker only â€” but are
    /// surfaced in the fleet report.
    pub async fn campaign(
        &self,
        strategy_factory: impl Fn(&WorkerSpec, usize) -> Box<dyn Strategy>,
        plan: &[WorkerSpec],
        parent: &Aerarium,
    ) -> Result<FleetReport, LegionError> {
        if plan.is_empty() {
            return Err(LegionError::Empty);
        }
        let take = plan.len().min(self.max_workers);
        let share = Aerarium {
            max_steps: (parent.max_steps / take).max(2),
            max_cost_cents: (parent.max_cost_cents / take as u64).max(1),
        };

        let mut worker_reports = Vec::with_capacity(take);
        for spec in &plan[..take] {
            let loop_ = WarLoop::new(
                self.gateway.clone(),
                self.registry.clone(),
                crate::CascadeRouter::new(vec![self.model.clone()]),
                share.clone(),
            );
            let strategy = strategy_factory(spec, share.max_steps);
            let report = loop_
                .run_as(&spec.goal, strategy, None, Some(spec.role.clone()))
                .await
                .map_err(|e| LegionError::Worker {
                    role: spec.role.clone(),
                    source: e,
                })?;
            worker_reports.push((spec.role.clone(), report));
        }

        // Synthesis through the summarizer tier.
        let synth_model = self.model.clone();
        let mut prompt = String::from("WORKER REPORTS:\n");
        for (role, r) in &worker_reports {
            prompt.push_str(&format!("[{role}] ok={} :: {}\n", r.ok, r.answer));
        }
        let reply = synth_model
            .complete(&prompt)
            .await
            .map_err(|e| LegionError::Worker {
                role: "synthesis".into(),
                source: e,
            })?;

        let all_ok = worker_reports.iter().all(|(_, r)| r.ok);
        let total_cost_cents = worker_reports_cost(&worker_reports);
        Ok(FleetReport {
            ok: all_ok,
            synthesis: reply.final_answer.unwrap_or(reply.thought),
            workers: worker_reports
                .into_iter()
                .map(|(role, r)| WorkerOutcome {
                    role,
                    ok: r.ok,
                    answer: r.answer,
                    breaker: r.breaker,
                })
                .collect(),
            total_cost_cents,
        })
    }
}

fn worker_reports_cost(reports: &[(String, RunReport)]) -> u64 {
    reports.iter().map(|(_, r)| r.cost_cents).sum()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerOutcome {
    pub role: String,
    pub ok: bool,
    pub answer: String,
    pub breaker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetReport {
    pub ok: bool,
    pub synthesis: String,
    pub workers: Vec<WorkerOutcome>,
    pub total_cost_cents: u64,
}
