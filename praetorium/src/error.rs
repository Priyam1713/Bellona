//! Gate errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PraetoriumError {
    #[error("unresolvable target: {0}")]
    UnresolvableTarget(String),

    #[error("refused by rule '{rule}': {reason}")]
    Refused { rule: String, reason: String },

    #[error("approval required by rule '{0}'")]
    ApprovalRequired(String),

    #[error("unknown approval ticket: {0}")]
    UnknownTicket(String),

    #[error("the Tribunician Veto is raised: {0}")]
    Frozen(String),

    #[error("identity enforcement is armed but the agent is unknown: {0}")]
    UnknownAgent(String),

    #[error("policy error: {0}")]
    Lex(String),

    #[error("ledger error: {0}")]
    Ledger(String),

    #[error("executor error: {0}")]
    Executor(String),

    #[error("kernel error: {0}")]
    Forge(#[from] forge::ForgeError),
}
