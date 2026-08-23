//! The event bus — every layer announces itself here; Vigiles listens.

use crate::id::ActionId;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Events emitted across the machine. Observers subscribe; nothing in the
/// kernel may *require* an observer (Vigiles is trusted by nothing).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum BusEvent {
    /// An action was proposed to the gate.
    ActionRequested {
        action_id: ActionId,
    },
    /// The gate reached a verdict (allow / deny / require_approval).
    DecisionMade {
        action_id: ActionId,
        verdict: String,
        rule_id: String,
    },
    /// An audit row entered the Annales chain.
    AuditCommitted {
        seq: u64,
        hash: String,
    },
    /// An effect finished executing.
    EffectSettled {
        action_id: ActionId,
        ok: bool,
    },
    /// The Tribunician Veto was raised — everything freezes.
    VetoRaised {
        reason: String,
    },
    /// Loop-level lifecycle.
    RunStarted {
        run_id: String,
        goal: String,
    },
    RunFinished {
        run_id: String,
        ok: bool,
    },
}

/// Broadcast bus with a bounded queue. Slow subscribers drop; they must not
/// slow the war machine down.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<BusEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        EventBus { tx }
    }

    pub fn publish(&self, ev: BusEvent) {
        // A send with no subscribers is fine; a full queue drops oldest —
        // observability must never become the bottleneck.
        let _ = self.tx.send(ev);
    }

    /// Subscribe to the stream of events.
    pub fn subscribe(&self) -> broadcast::Receiver<BusEvent> {
        self.tx.subscribe()
    }
}
