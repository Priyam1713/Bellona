//! The War Loop — boring, reliable, and always through the gate.

pub mod budget;
pub mod model;
pub mod planexecute;
pub mod react;
pub mod strategy;

pub use budget::Aerarium;
pub use model::{BellumError, CascadeRouter, ModelClient, ModelReply, Phase, ToolCall};
pub use planexecute::PlanExecuteStrategy;
pub use react::ReActStrategy;
pub use strategy::{Step, Strategy};

use forge::event::{BusEvent, EventBus};
use forge::id::AgentId;
use forge::primitives::{ActionRequest, EffectKind, Outcome};
use forge::tool::ToolRegistry;
use praetorium::custos::{CustosGateway, EffectExecutor, GateOutcome, TargetResolver};
use std::sync::Arc;

/// Terminal report of one campaign.
#[derive(Debug, Clone)]
pub struct RunReport {
    pub ok: bool,
    pub answer: String,
    pub steps_used: usize,
    pub cost_cents: u64,
    /// Set when a breaker (not the goal) ended the run.
    pub breaker: Option<String>,
}

/// The loop. Generic over the gate's resolver/executor so tests can plug
/// doubles and production plugs Castra + Foedus.
pub struct WarLoop<R: TargetResolver, E: EffectExecutor> {
    gateway: Arc<CustosGateway<R, E>>,
    registry: Arc<ToolRegistry>,
    router: CascadeRouter,
    aerarium: Aerarium,
    /// When set, pending approvals are auto-approved by this principal
    /// (non-interactive runs). When None, tickets park.
    auto_approver: Option<String>,
    bus: EventBus,
}

impl<R: TargetResolver, E: EffectExecutor> WarLoop<R, E> {
    pub fn new(
        gateway: Arc<CustosGateway<R, E>>,
        registry: Arc<ToolRegistry>,
        router: CascadeRouter,
        aerarium: Aerarium,
    ) -> Self {
        let bus = gateway.bus();
        WarLoop {
            gateway,
            registry,
            router,
            aerarium,
            auto_approver: None,
            bus,
        }
    }

    /// Enable non-interactive approval handling.
    pub fn with_auto_approver(mut self, principal: impl Into<String>) -> Self {
        self.auto_approver = Some(principal.into());
        self
    }

    async fn execute_through_gate(
        &self,
        agent_id: &AgentId,
        call: &ToolCall,
        intent: &str,
    ) -> Result<Outcome, BellumError> {
        // Effect classification comes from the tool's own declared spec —
        // never from the model's claim.
        let effect = self
            .registry
            .get(&call.name)
            .map(|t| t.spec().effect.clone())
            .unwrap_or(EffectKind::Custom("unknown_tool".into()));
        let mut req = ActionRequest::new(agent_id.clone(), call.name.clone(), effect)
            .with_intent(intent.to_string())
            .with_params(call.args.clone());
        // Tools act within the campaign workspace; an unaddressable effect
        // targets its root.
        req.target_uri = {
            let u = target_uri_from_args(&call.args);
            if u.is_empty() {
                "file://workspace".to_string()
            } else {
                u
            }
        };

        match self.gateway.submit(req).await? {
            GateOutcome::Executed { outcome, .. } => Ok(outcome),
            GateOutcome::Denied { rule_id, reason } => {
                // Denial is an observation, not an exception — the agent
                // learns and may adapt within policy.
                Ok(Outcome::Failed {
                    error: format!("DENIED by rule '{rule_id}': {reason}"),
                })
            }
            GateOutcome::PendingApproval { ticket_id } => {
                if let Some(approver) = &self.auto_approver {
                    match self.gateway.approve(&ticket_id, approver).await? {
                        GateOutcome::Executed { outcome, .. } => Ok(outcome),
                        other => Ok(Outcome::Failed {
                            error: format!("approval flow returned {other:?}"),
                        }),
                    }
                } else {
                    Ok(Outcome::Failed {
                        error: format!("PENDING human approval (ticket {ticket_id})"),
                    })
                }
            }
        }
    }

    /// Run one campaign to completion.
    pub async fn run(
        &self,
        goal: &str,
        mut strategy: Box<dyn Strategy>,
        phase_model_override: Option<Arc<dyn ModelClient>>,
    ) -> Result<RunReport, BellumError> {
        let agent_id = AgentId::mint();
        self.bus.publish(BusEvent::RunStarted {
            run_id: agent_id.to_string(),
            goal: goal.to_string(),
        });
        strategy.begin(goal).await?;

        let mut observation: Option<String> = None;
        let cost = 0u64;
        let mut steps = 0usize;
        let mut answer = String::new();
        let mut breaker: Option<String> = None;

        loop {
            let client: Arc<dyn ModelClient> = phase_model_override
                .clone()
                .or_else(|| self.router.route(Phase::Execute))
                .ok_or_else(|| BellumError::Model("no model enrolled".into()))?;

            match strategy
                .next_step(observation.as_deref(), client.as_ref())
                .await?
            {
                Step::Think(t) => {
                    steps += 1;
                    observation = Some(t);
                }
                Step::CallTool(call) => {
                    steps += 1;
                    let outcome = self
                        .execute_through_gate(&agent_id, &call, "war-loop step")
                        .await?;
                    observation = Some(match outcome {
                        Outcome::Completed { result } => {
                            format!("OK {}", serde_json::to_string(&result).unwrap_or_default())
                        }
                        Outcome::Failed { error } => format!("ERROR {error}"),
                    });
                }
                Step::Finish(a) => {
                    answer = a;
                    break;
                }
                Step::Breaker(reason) => {
                    breaker = Some(reason);
                    break;
                }
            }

            if steps >= self.aerarium.max_steps {
                breaker = Some(format!("aerarium max_steps={}", self.aerarium.max_steps));
                break;
            }
        }

        self.bus.publish(BusEvent::RunFinished {
            run_id: agent_id.to_string(),
            ok: breaker.is_none(),
        });

        Ok(RunReport {
            ok: breaker.is_none(),
            answer,
            steps_used: steps,
            cost_cents: cost,
            breaker,
        })
    }
}

/// Derive a target URI hint from common argument shapes. Tools declare real
/// targets in their specs; this only feeds policy attributes.
fn target_uri_from_args(args: &serde_json::Value) -> String {
    if let Some(p) = args.get("path").and_then(|v| v.as_str()) {
        return format!("file://workspace/{p}");
    }
    if let Some(u) = args.get("url").and_then(|v| v.as_str()) {
        return u.to_string();
    }
    if let Some(c) = args.get("command").and_then(|v| v.as_str()) {
        return format!("shell://{c}");
    }
    String::new()
}
