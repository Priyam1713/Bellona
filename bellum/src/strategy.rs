//! Strategies — HOW the loop thinks. Pluggable per Law I; the loop itself
//! stays boring.

use crate::model::ModelClient;

/// The next move the war loop should take.
#[derive(Debug, Clone)]
pub enum Step {
    /// Deliberate out loud (recorded to context, costs no effect).
    Think(String),
    /// Request an effect through the Praetorian Gate.
    CallTool(crate::model::ToolCall),
    /// Goal satisfied.
    Finish(String),
    /// Circuit breaker tripped — halt with the reason on the record.
    Breaker(String),
}

/// A strategy owns its state and produces steps until Finish/Breaker.
#[async_trait::async_trait]
pub trait Strategy: Send + Sync {
    /// Called once at campaign start with the goal.
    async fn begin(&mut self, goal: &str) -> Result<(), crate::BellumError>;

    /// Produce the next step. `observation` is None before the first tool
    /// call of the campaign.
    async fn next_step(
        &mut self,
        observation: Option<&str>,
        model: &dyn ModelClient,
    ) -> Result<Step, crate::BellumError>;

    /// Human-readable strategy name for audit rows.
    fn name(&self) -> &'static str;

    /// Total model spend accrued inside this strategy so far. The loop
    /// enforces the Aerarium against this (denial-of-wallet is a failure
    /// class).
    fn accrued_cost_cents(&self) -> u64 {
        0
    }
}
