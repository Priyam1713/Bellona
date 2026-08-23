//! The Aerarium — treasury of the campaign. Cost is a governed resource,
//! not an accident (denial-of-wallet is a failure class).

/// Hard ceilings for one run. Exceeding any raises the circuit breaker.
#[derive(Debug, Clone)]
pub struct Aerarium {
    /// Maximum war-loop steps before forced halt.
    pub max_steps: usize,
    /// Maximum spend in cents across model calls.
    pub max_cost_cents: u64,
}

impl Default for Aerarium {
    fn default() -> Self {
        Aerarium {
            max_steps: 40,
            max_cost_cents: 500,
        }
    }
}

impl Aerarium {
    pub fn new(max_steps: usize, max_cost_cents: u64) -> Self {
        Aerarium {
            max_steps,
            max_cost_cents,
        }
    }
}
