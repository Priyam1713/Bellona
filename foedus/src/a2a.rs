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

// ---------- Campaign IX: exactly-once delegation service ----------

/// Storage contract for the idempotency ledger. Back it with SQLite in
/// deployments; HashMap in tests.
#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    /// Returns `Some(cached_response_json)` if already executed.
    async fn claim(&self, key: &str) -> Result<Option<String>, String>;
    async fn complete(&self, key: &str, response_json: &str) -> Result<(), String>;
}

/// The thing that actually performs delegated work (a WarLoop campaign).
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute(&self, req: &TaskRequest) -> Result<TaskResponse, String>;
}

/// The delegatee service: ACK semantics, dedupe, and execution ordering.
pub struct A2aService<S: IdempotencyStore, X: TaskExecutor> {
    pub store: S,
    pub executor: X,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("store error: {0}")]
    Store(String),
    #[error("executor error: {0}")]
    Executor(String),
}

impl<S: IdempotencyStore, X: TaskExecutor> A2aService<S, X> {
    /// Handle a task with exactly-once semantics:
    /// 1. claim the idempotency slot,
    /// 2. if cached ? replay response verbatim,
    /// 3. else execute, persist response, answer.
    pub async fn handle(&self, req: TaskRequest) -> Result<TaskResponse, ServiceError> {
        let cached = self
            .store
            .claim(&req.idempotency_key)
            .await
            .map_err(ServiceError::Store)?;
        if let Some(json) = cached {
            return serde_json::from_str(&json)
                .map_err(|e| ServiceError::Store(format!("cached decode: {e}")));
        }
        let resp = self
            .executor
            .execute(&req)
            .await
            .map_err(ServiceError::Executor)?;
        let json = serde_json::to_string(&resp).map_err(|e| ServiceError::Store(e.to_string()))?;
        self.store
            .complete(&req.idempotency_key, &json)
            .await
            .map_err(ServiceError::Store)?;
        Ok(resp)
    }
}
