//! A2A — agent-to-agent delegation across org boundaries.
//!
//! An AgentCard is the capability advertisement; tasks are delegated with an
//! idempotency key and answered with a structured response.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Advertisement of what this agent can do (A2A card shape).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    /// e.g. ["research", "code-review"]
    pub skills: Vec<String>,
    pub endpoint: String,
    /// Protocol versions this agent speaks.
    pub protocol_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    pub task_id: String,
    /// Idempotency key — retries must not double-execute effects.
    pub idempotency_key: String,
    pub instruction: String,
    #[serde(default)]
    pub context: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TaskResponse {
    Completed { artifacts: serde_json::Value },
    Failed { error: String },
    RequiresApproval { reason: String },
}

/// Delegatee side: accept and execute delegated tasks — still through our
/// own Praetorian Gate. Federation never bypasses governance.
#[async_trait]
pub trait A2aDelegatee: Send + Sync {
    fn card(&self) -> AgentCard;
    async fn handle_task(&self, req: TaskRequest) -> Result<TaskResponse, crate::FoedusError>;
}

/// Delegator side: reach foreign agents by their cards.
#[async_trait]
pub trait A2aDelegator: Send + Sync {
    async fn discover(&self, skill: &str) -> Result<Vec<AgentCard>, crate::FoedusError>;
    async fn delegate(
        &self,
        endpoint: &str,
        req: TaskRequest,
    ) -> Result<TaskResponse, crate::FoedusError>;
}
