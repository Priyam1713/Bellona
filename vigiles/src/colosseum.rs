//! Colosseum — where harnesses prove themselves.
//!
//! A suite of cases runs each task k times; a case "passes at k" only when
//! ALL k trials pass (tau-bench doctrine: agents can pass once and fail on
//! repeats). Gates turn suites into CI policy with honest exit codes.

use serde::{Deserialize, Serialize};

/// How to judge one trial's output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verifier", rename_all = "snake_case")]
pub enum Verifier {
    /// Output equals this string exactly.
    ExactOutput { expected: String },
    /// Output contains all fragments.
    ContainsAll { fragments: Vec<String> },
    /// Output must start with the marker (e.g. "OK").
    StartsWith { prefix: String },
}

impl Verifier {
    pub fn judge(&self, output: &str) -> bool {
        match self {
            Verifier::ExactOutput { expected } => output.trim() == expected.trim(),
            Verifier::ContainsAll { fragments } => fragments
                .iter()
                .all(|f| output.to_lowercase().contains(&f.to_lowercase())),
            Verifier::StartsWith { prefix } => output
                .trim_start()
                .to_lowercase()
                .starts_with(&prefix.to_lowercase()),
        }
    }
}

/// One evaluated task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteCase {
    pub id: String,
    pub task: String,
    #[serde(default = "default_trials")]
    pub trials: usize,
    pub verifier: Verifier,
}

fn default_trials() -> usize {
    3
}

/// Result rows per case, in trial order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub case_id: String,
    pub outputs: Vec<String>,
}

/// The computed report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteReport {
    pub suite_name: String,
    pub cases: Vec<CaseVerdict>,
    /// pass^k across cases (mean fraction of cases passing all k trials).
    pub pass_at_k: f64,
    pub total_cost_cents: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseVerdict {
    pub case_id: String,
    /// All trials passed?
    pub passed_at_k: bool,
    pub trial_flags: Vec<bool>,
}

/// pass^k: fraction of cases where every one of the k trials succeeded.
///
/// Cases with fewer recorded outputs than their declared trials count as
/// failing (a missing run is a failed run).
pub fn compute_pass_at_k(cases: &[CaseVerdict]) -> f64 {
    if cases.is_empty() {
        return 0.0;
    }
    let passed = cases.iter().filter(|c| c.passed_at_k).count();
    passed as f64 / cases.len() as f64
}

/// CI gate thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gate {
    /// Minimum acceptable pass^k, e.g. 0.8.
    pub min_pass_at_k: f64,
    /// Maximum acceptable spend for the whole suite.
    pub max_cost_cents: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateVerdict {
    Passed,
    FailedReliability,
    FailedBudget,
}

impl Gate {
    pub fn evaluate(&self, report: &SuiteReport) -> GateVerdict {
        if report.total_cost_cents > self.max_cost_cents {
            return GateVerdict::FailedBudget;
        }
        // Budget is checked first: burning the treasury is worse than a flaky
        // scorecard, and it fails loudly either way.
        if report.pass_at_k < self.min_pass_at_k {
            return GateVerdict::FailedReliability;
        }
        GateVerdict::Passed
    }

    /// Process exit code convention:
    /// 0 = pass, 1 = reliability failure, 2 = budget breach.
    pub fn exit_code(&self, verdict: GateVerdict) -> i32 {
        match verdict {
            GateVerdict::Passed => 0,
            GateVerdict::FailedReliability => 1,
            GateVerdict::FailedBudget => 2,
        }
    }
}
