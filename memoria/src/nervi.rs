//! Nervi — the vitals. Pinned context that survives every compaction
//! byte-identical (Law VI).

use serde::{Deserialize, Serialize};

/// An ordered set of pinned statements: the active goal, constraints, and
/// the minimum proof needed for the next action.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Nervi {
    entries: Vec<String>,
}

impl Nervi {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pin a statement. Duplicates are refused — pinned context is scarce.
    pub fn pin(&mut self, stmt: impl Into<String>) -> bool {
        let s = stmt.into();
        if self.entries.iter().any(|e| e == &s) {
            return false;
        }
        self.entries.push(s);
        true
    }

    /// Unpin by exact match; returns true if removed.
    pub fn unpin(&mut self, stmt: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e != stmt);
        self.entries.len() != before
    }

    /// Render for injection at the top of the window.
    pub fn render(&self) -> String {
        let mut out = String::from("[PINNED]\n");
        for e in &self.entries {
            out.push_str("• ");
            out.push_str(e);
            out.push('\n');
        }
        out
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.entries.iter()
    }
}
