//! The Ludus Proving Grounds.
//!
//! A forged tool is a *civilian*: it may run in the arena, but it cannot
//! enter the legion (the registry) until it survives its full battery AND
//! carries the owner's countersignature. Self-evolution without supply-chain
//! chaos.

use crate::forger::ForgedTool;
use castra::{CampCommand, EnvScrubPolicy, SandboxDriver};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One proving-ground case: given this input, the tool must exit cleanly and
/// produce output containing these fragments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryCase {
    pub input_json: String,
    #[serde(default)]
    pub output_contains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LudusVerdict {
    pub tool_name: String,
    pub passed: bool,
    pub failures: Vec<String>,
}

pub const LUDUS_MARKER: &str = "ludus://verdict";

impl LudusVerdict {
    /// The verdict is itself auditable evidence — render for the Annales.
    pub fn as_evidence(&self) -> serde_json::Value {
        serde_json::json!({
            "marker": LUDUS_MARKER,
            "tool": self.tool_name,
            "passed": self.passed,
            "failures": self.failures,
        })
    }
}

/// Run the battery. Every case runs inside the provided camp — forged code
/// never touches the host un-sandboxed, not even during trials.
pub async fn run_battery(
    tool: &ForgedTool,
    battery: &[BatteryCase],
    driver: &dyn SandboxDriver,
    workspace: PathBuf,
) -> LudusVerdict {
    let mut failures = Vec::new();
    if let Err(e) = tool.validate() {
        failures.push(format!("manifest: {e}"));
        return LudusVerdict {
            tool_name: tool.name.clone(),
            passed: false,
            failures,
        };
    }

    for (i, case) in battery.iter().enumerate() {
        let args = tool.render(&case.input_json);
        let cmd = CampCommand {
            program: match tool.lang {
                crate::forger::ScriptLang::Shell => "cmd".to_string(),
                crate::forger::ScriptLang::Python => "python".to_string(),
            },
            args,
            working_dir: workspace.clone(),
            timeout_secs: 30,
        };
        match driver.run(&cmd, &EnvScrubPolicy::default()).await {
            Ok(outcome) => {
                if !outcome.exit_ok {
                    failures.push(format!("case {i}: non-zero exit: {}", outcome.stderr));
                    continue;
                }
                for frag in &case.output_contains {
                    if !outcome.stdout.to_lowercase().contains(&frag.to_lowercase()) {
                        failures.push(format!(
                            "case {i}: output missing '{frag}' (got: {})",
                            outcome.stdout.trim()
                        ));
                    }
                }
            }
            Err(e) => failures.push(format!("case {i}: camp error: {e}")),
        }
    }

    LudusVerdict {
        tool_name: tool.name.clone(),
        passed: failures.is_empty(),
        failures,
    }
}

/// Promotion to *legionary*. Refuses unless:
/// 1. the verdict passed, and
/// 2. the owner's countersignature is present (Law V).
pub fn promote(
    tool: ForgedTool,
    verdict: &LudusVerdict,
    owner_countersig: Option<&str>,
) -> Result<ForgedTool, crate::OfficinaError> {
    if !verdict.passed {
        return Err(crate::OfficinaError(format!(
            "refusing promotion: ludus failures: {:?}",
            verdict.failures
        )));
    }
    let sig = owner_countersig.ok_or_else(|| {
        crate::OfficinaError("refusing promotion: no owner countersignature".into())
    })?;
    if sig.len() < 16 {
        return Err(crate::OfficinaError("countersignature malformed".into()));
    }
    Ok(tool)
}
