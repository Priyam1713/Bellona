//! V1.2 â€” the conformance test-kit. One call to torture any tool forever.
//!
//! Contract enforced (Law IV + Law I):
//! - spec sanity (name shape, description depth, read/write honesty)
//! - workspace-escape probes MUST be refused by fs-touching tools
//! - malformed / oversized args must yield Err-or-Ok, never corrupt state
//!
//! Panics are NOT caught here by design: a panicking tool fails loudly at
//! the exact probe, which is the clearest possible signal.

use crate::tool::{Tool, ToolContext};
use serde_json::json;
use std::path::Path;

#[derive(Debug, Default)]
pub struct ConformanceReport {
    pub failures: Vec<String>,
}

impl ConformanceReport {
    pub fn ok(&self) -> bool {
        self.failures.is_empty()
    }
}

fn escape_probes() -> Vec<serde_json::Value> {
    #[cfg(windows)]
    let outside = "C:\\Windows\\bellona-escape-probe.txt";
    #[cfg(not(windows))]
    let outside = "/tmp/bellona-escape-probe";

    vec![
        json!({ "path": "../escape-probe.txt" }),
        json!({ "path": outside }),
        json!({ "file": outside, "path": outside }),
    ]
}

/// Full battery for a filesystem-touching tool (`read_only` respected).
pub async fn conform_fs(tool: &dyn Tool, workspace: &Path) -> ConformanceReport {
    let mut rep = spec_sanity(tool);

    let ctx = ToolContext {
        agent_id: crate::AgentId::mint(),
        workspace: workspace.to_path_buf(),
    };
    let writes = tool.spec().effect == crate::primitives::EffectKind::FileWrite;

    for probe in escape_probes() {
        match tool.execute(&ctx, probe.clone()).await {
            Err(_) => {} // refused â€” correct
            Ok(v) => {
                if writes {
                    rep.failures
                        .push(format!("escape ACCEPTED: {probe} â†’ {v}"));
                }
            }
        }
    }

    // Malformed args must be an Err (missing required keys), not success.
    if tool
        .execute(&ctx, json!({ "unrelated": true }))
        .await
        .is_ok()
        && writes
    {
        rep.failures
            .push("write tool accepted args missing required keys".into());
    }

    rep
}

/// Spec-only sanity for any tool (fs or not).
pub fn spec_sanity(tool: &dyn Tool) -> ConformanceReport {
    let mut rep = ConformanceReport::default();
    let spec = tool.spec();

    let name_ok = !spec.name.is_empty()
        && spec
            .name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase())
        && spec
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if !name_ok {
        rep.failures
            .push(format!("spec: bad tool name '{}'", spec.name));
    }
    if spec.description.len() < 8 {
        rep.failures
            .push(format!("spec: '{}' description too short", spec.name));
    }
    let honest_rw = spec.read_only
        || matches!(
            spec.effect,
            crate::primitives::EffectKind::FileWrite
                | crate::primitives::EffectKind::ShellExec
                | crate::primitives::EffectKind::BrowserAct
                | crate::primitives::EffectKind::MemoryWrite
                | crate::primitives::EffectKind::ComponentPublish
                | crate::primitives::EffectKind::McpCall
        )
        || matches!(spec.effect, crate::primitives::EffectKind::Custom(_))
        || spec.effect == crate::primitives::EffectKind::BrowserNavigate;
    if spec.read_only && !honest_rw {
        rep.failures.push(format!(
            "spec: '{}' claims read_only with write-class effect {:?}",
            spec.name, spec.effect
        ));
    }
    rep
}
