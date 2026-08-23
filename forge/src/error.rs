//! Kernel errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ForgeError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("tool '{tool}' is registered but not exposed")]
    ToolNotExposed { tool: String },

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("context window budget exhausted ({used} > {budget} tokens)")]
    BudgetExhausted { used: usize, budget: usize },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

/// Convenience alias used across the workspace.
pub type ForgeResult<T> = Result<T, ForgeError>;
