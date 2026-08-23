//! Campaign XI â€” Self-Forging Legionaries.
//!
//! The loop may propose tools; the Ludus decides; the owner seals; the
//! registry persists. Deny-patterns and adversarial batteries are NOT
//! optional â€” they are the price of admission.

use crate::arsenal::run_camp;
use forge::error::{ForgeError, ForgeResult};
use forge::primitives::EffectKind;
use forge::simple_tool::SimpleTool;
use forge::tool::ToolSpec;
use officina::ForgedTool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ---------- XI.1: the trigger ----------

/// Tracks repeated missing-tool observations per session.
#[derive(Default)]
pub struct ForgingTrigger {
    counts: HashMap<String, u32>,
}

const TRIGGER_THRESHOLD: u32 = 2;

impl ForgingTrigger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed an observation; returns Some(missing_name) when the SAME unknown
    /// tool has been requested `TRIGGER_THRESHOLD` times.
    pub fn observe(&mut self, observation: &str) -> Option<String> {
        // Signal = the pattern `tool '<name>' missing` anywhere in the
        // observation. Anything else is unrelated and never counted.
        let rest = observation.split_once("tool '")?.1;
        let name = rest.split('\'').next()?.to_string();
        if !rest.contains("missing") {
            return None;
        }
        let c = self.counts.entry(name.clone()).or_insert(0);
        *c += 1;
        (*c >= TRIGGER_THRESHOLD).then_some(name)
    }
}

// ---------- deny patterns (blast-radius pre-filter) ----------

const DENY_PATTERNS: &[&str] = &[
    "http://",
    "https://",
    "ftp://",
    "curl ",
    "wget ",
    "invoke-webrequest",
    "invoke-restmethod",
    "ssh ",
    "scp ",
    "api_key",
    "apikey",
    "password",
    "secret",
    "token=",
    "../",
    "..\\",
    "remove-item",
    "rm -rf",
    "del /f",
    "format ",
];

pub fn deny_reasons(script_template: &str) -> Vec<String> {
    let lower = script_template.to_lowercase();
    DENY_PATTERNS
        .iter()
        .filter(|p| lower.contains(*p))
        .map(|p| format!("deny-pattern '{p}' present"))
        .collect()
}

// ---------- XI.2: adversarial battery is mandatory ----------

/// Augment any proposed battery with the non-negotiable adversarial cases.
pub fn prepare_battery(proposed: &[officina::BatteryCase]) -> Vec<officina::BatteryCase> {
    let mut cases = proposed.to_vec();
    cases.push(officina::BatteryCase {
        input_json: "{}".into(),
        output_contains: vec![],
    });
    cases.push(officina::BatteryCase {
        input_json: r#"{"path": "../../etc/passwd"}"#.into(),
        output_contains: vec![],
    });
    cases.push(officina::BatteryCase {
        input_json: format!("{{\"content\": \"{}\"}}", "x".repeat(100_000)),
        output_contains: vec![],
    });
    cases
}

// ---------- XI.4: persistence & revocation ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTool {
    pub tool: ForgedTool,
    /// ludus evidence hash chain reference (audit tie-in).
    pub verdict_evidence: serde_json::Value,
}

pub const TOMBSTONE: &str = ".revoked";

pub fn persist_promoted(dir: &Path, stored: &StoredTool) -> ForgeResult<PathBuf> {
    let out = dir.join(format!("{}.json", stored.tool.name));
    std::fs::create_dir_all(dir).map_err(ForgeError::Io)?;
    let data = serde_json::to_string_pretty(stored).map_err(ForgeError::Serde)?;
    std::fs::write(&out, data).map_err(ForgeError::Io)?;
    Ok(out)
}

pub fn revoke(dir: &Path, name: &str) -> ForgeResult<()> {
    std::fs::write(dir.join(format!("{name}{TOMBSTONE}")), "revoked").map_err(ForgeError::Io)
}

pub fn is_revoked(dir: &Path, name: &str) -> bool {
    dir.join(format!("{name}{TOMBSTONE}")).exists()
}

pub fn load_promoted(dir: &Path) -> ForgeResult<Vec<StoredTool>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(dir).map_err(ForgeError::Io)? {
        let entry = entry.map_err(ForgeError::Io)?;
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&p).map_err(ForgeError::Io)?;
        let stored: StoredTool = serde_json::from_str(&raw).map_err(ForgeError::Serde)?;
        if is_revoked(dir, &stored.tool.name) {
            continue;
        }
        out.push(stored);
    }
    Ok(out)
}

// ---------- executable promoted tools ----------

/// A promoted forged tool as a live `SimpleTool` executing its template
/// inside the workspace camp. Depth limit: forged tools cannot trigger
/// further forging (the loop checks `forged` markers by name prefix).
pub fn script_tool(stored: &StoredTool) -> Arc<SimpleTool> {
    let template = stored.tool.script_template.clone();
    SimpleTool::into_arc(SimpleTool::new(
        &format!("forged_{}", stored.tool.name),
        &format!("[forged] {}", stored.tool.description),
        EffectKind::Custom("forged_script".into()),
        false, // unclassified â‡’ treated as write by policy doctrine
        json!({}),
        move |ctx, args| {
            let ws = ctx.workspace.clone();
            let template = template.clone();
            let input = serde_json::to_string(&args).unwrap_or_default();
            Box::pin(async move {
                #[cfg(windows)]
                let (prog, argv) = {
                    let filled = template.replace("{input}", &input);
                    ("cmd".to_string(), vec!["/C".to_string(), filled])
                };
                #[cfg(not(windows))]
                let (prog, argv) = {
                    let filled = template.replace("{input}", &input);
                    ("sh".to_string(), vec!["-c".to_string(), filled])
                };
                run_camp(&ws, &prog, argv).await.map_err(ForgeError::Other)
            })
        },
    ))
}

/// Spec helper for tests/registration symmetry.
pub fn spec_of(stored: &StoredTool) -> ToolSpec {
    ToolSpec {
        name: format!("forged_{}", stored.tool.name),
        description: format!("[forged] {}", stored.tool.description),
        effect: EffectKind::Custom("forged_script".into()),
        read_only: false,
        schema: json!({}),
    }
}
