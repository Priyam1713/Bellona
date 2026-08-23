//! Tabella — the wax tablet. Decisions-as-records for the running task,
//! never raw logs.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One decision record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub seq: u64,
    pub ts_ms: u64,
    /// e.g. "tool_selected", "plan_revised", "hypothesis_discarded"
    pub kind: String,
    /// One-line human/agent-readable summary.
    pub summary: String,
    #[serde(default)]
    pub detail: BTreeMap<String, String>,
}

/// The running-task ledger.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tabella {
    records: Vec<DecisionRecord>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Tabella {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a decision; returns its sequence.
    pub fn record(
        &mut self,
        kind: &str,
        summary: impl Into<String>,
        detail: BTreeMap<String, String>,
    ) -> u64 {
        let seq = self.records.len() as u64;
        self.records.push(DecisionRecord {
            seq,
            ts_ms: now_ms(),
            kind: kind.to_string(),
            summary: summary.into(),
            detail,
        });
        seq
    }

    /// The latest record of a kind, if any.
    pub fn latest_of_kind(&self, kind: &str) -> Option<&DecisionRecord> {
        self.records.iter().rev().find(|r| r.kind == kind)
    }

    /// Case-insensitive substring search over summaries.
    pub fn search(&self, needle: &str) -> Vec<&DecisionRecord> {
        let n = needle.to_lowercase();
        self.records
            .iter()
            .filter(|r| r.summary.to_lowercase().contains(&n))
            .collect()
    }

    /// Compact rendering for re-injection after compaction.
    pub fn render_recent(&self, last_n: usize) -> String {
        let mut out = String::from("[TABELLA]\n");
        for r in self.records.iter().rev().take(last_n).rev() {
            out.push_str(&format!("{}. [{}] {}\n", r.seq, r.kind, r.summary));
        }
        out
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
