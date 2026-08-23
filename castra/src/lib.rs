//! # castra — the camp ladder.
//!
//! Sandboxing is tiered by risk; not every action needs maximum isolation,
//! but every action runs somewhere *chosen*, never ambient.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Sandbox-layer error.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct CastraError(pub String);

/// Result alias for camp operations.
pub type CastraResult<T> = Result<T, CastraError>;

/// Isolation levels, weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CastraLevel {
    /// Scrubbed-environment process. Fast lane for trusted, read-only work.
    Prima,
    /// Dedicated container with its own workspace volume.
    Secunda,
    /// gVisor/microVM — kernel-boundary isolation.
    Tertia,
    /// Ephemeral cloud computer per long-running campaign.
    Quarta,
}

/// Which environment variables a spawned process may inherit.
///
/// Doctrine (OpenBot/Codex lesson): the process inherits PATH, locale,
/// terminal and proxy variables — nothing else of the deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvScrubPolicy {
    pub allow_prefixes: Vec<String>,
}

impl Default for EnvScrubPolicy {
    fn default() -> Self {
        EnvScrubPolicy {
            allow_prefixes: [
                "PATH",
                "SystemRoot",
                "SYSTEMROOT",
                "TEMP",
                "TMP",
                "HOME",
                "LANG",
                "LC_ALL",
                "TERM",
                "HTTP_PROXY",
                "HTTPS_PROXY",
                "NO_PROXY",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

impl EnvScrubPolicy {
    pub fn scrub(&self, env: &HashMap<String, String>) -> HashMap<String, String> {
        env.iter()
            .filter(|(k, _)| {
                self.allow_prefixes.iter().any(|p| {
                    k.eq_ignore_ascii_case(p) || k.to_uppercase().starts_with(&p.to_uppercase())
                })
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

/// A command the camp has been asked to run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampCommand {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub timeout_secs: u64,
}

/// Result of running inside a camp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampOutcome {
    pub exit_ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// A sandbox driver. Implement per level; the gateway never cares which.
#[async_trait]
pub trait SandboxDriver: Send + Sync {
    fn level(&self) -> CastraLevel;
    async fn run(&self, cmd: &CampCommand, env: &EnvScrubPolicy) -> CastraResult<CampOutcome>;
}

/// The default weak-lane driver: local process with scrubbed environment.
/// Suitable only for CastraLevel::Prima.
pub struct ProcessDriver;

fn now_limited(s: String, cap: usize) -> String {
    if s.len() > cap {
        format!("{}…[truncated]", &s[..cap])
    } else {
        s
    }
}

#[async_trait]
impl SandboxDriver for ProcessDriver {
    fn level(&self) -> CastraLevel {
        CastraLevel::Prima
    }

    async fn run(&self, cmd: &CampCommand, policy: &EnvScrubPolicy) -> CastraResult<CampOutcome> {
        let mut command = tokio::process::Command::new(&cmd.program);
        command
            .args(&cmd.args)
            .current_dir(&cmd.working_dir)
            .env_clear()
            .envs(policy.scrub(&std::env::vars().collect()));

        #[cfg(windows)]
        {
            // Windows needs SystemRoot for even trivial processes.
            if let Ok(sr) = std::env::var("SystemRoot") {
                command.env("SystemRoot", sr);
            }
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(cmd.timeout_secs.max(1)),
            command.output(),
        )
        .await
        .map_err(|_| CastraError("camp timeout".into()))?
        .map_err(|e| CastraError(format!("spawn failed: {e}")))?;

        Ok(CampOutcome {
            exit_ok: output.status.success(),
            stdout: now_limited(String::from_utf8_lossy(&output.stdout).into_owned(), 16_384),
            stderr: now_limited(String::from_utf8_lossy(&output.stderr).into_owned(), 16_384),
        })
    }
}
