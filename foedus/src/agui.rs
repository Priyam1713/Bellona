//! AG-UI — the agent↔user-interface event protocol. Bellona emits these
//! from its bus so any surface (TUI, web, desktop, OpenBot-style rooms)
//! renders the same stream.

use async_trait::async_trait;
use forge::event::BusEvent;
use serde::{Deserialize, Serialize};

/// AG-UI-flavoured events (subset sufficient for v0 surfaces).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgUiEvent {
    RunStarted { run_id: String },
    TextMessageContent { delta: String },
    ToolCallStarted { name: String },
    ToolCallEnded { name: String, ok: bool },
    StateSnapshot { state: serde_json::Value },
    RunFinished { run_id: String, ok: bool },
    Error { message: String },
}

/// Translate internal bus events into surface events.
pub fn from_bus(ev: &BusEvent) -> Option<AgUiEvent> {
    match ev {
        BusEvent::RunStarted { run_id, .. } => Some(AgUiEvent::RunStarted {
            run_id: run_id.clone(),
        }),
        BusEvent::ActionRequested { .. } => None, // pre-decision: not surfaced
        BusEvent::DecisionMade {
            verdict, rule_id, ..
        } => match verdict.as_str() {
            "allow" => Some(AgUiEvent::ToolCallStarted {
                name: format!("rule:{rule_id}"),
            }),
            "deny" => Some(AgUiEvent::Error {
                message: format!("refused by rule '{rule_id}'"),
            }),
            _ => None,
        },
        BusEvent::EffectSettled { ok, .. } => Some(AgUiEvent::ToolCallEnded {
            name: String::new(),
            ok: *ok,
        }),
        BusEvent::RunFinished { run_id, ok } => Some(AgUiEvent::RunFinished {
            run_id: run_id.clone(),
            ok: *ok,
        }),
        BusEvent::VetoRaised { reason } => Some(AgUiEvent::Error {
            message: format!("TRIBUNICIAN VETO: {reason}"),
        }),
        BusEvent::AuditCommitted { .. } => None,
    }
}

/// A sink for surface events.
#[async_trait]
pub trait AgUiEmitter: Send + Sync {
    async fn emit(&self, ev: AgUiEvent) -> Result<(), crate::FoedusError>;
}

/// Fan-out to many surfaces.
#[derive(Default)]
pub struct AgUiFanout {
    emitters: Vec<std::sync::Arc<dyn AgUiEmitter>>,
}

impl AgUiFanout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach(&mut self, e: std::sync::Arc<dyn AgUiEmitter>) {
        self.emitters.push(e);
    }

    pub async fn broadcast(&self, ev: AgUiEvent) {
        for e in &self.emitters {
            let _ = e.emit(ev.clone()).await;
        }
    }
}
