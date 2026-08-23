//! Context window bookkeeping with lineage-aware compaction support.
//!
//! Token accounting uses a documented heuristic (≈4 chars/token). Real
//! tokenizer adapters can replace `estimate_tokens` per model; the kernel
//! only needs the budget arithmetic to stay honest.

use crate::error::{ForgeError, ForgeResult};
use serde::{Deserialize, Serialize};

/// Reference to source material a summary replaced (Law VI: lineage).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageRef {
    /// Inclusive sequence span of blocks this block summarizes.
    pub from_seq: u64,
    pub to_seq: u64,
}

/// One block in the window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub seq: u64,
    pub role: Role,
    pub content: String,
    #[serde(default)]
    pub pinned: bool,
    /// Present when this block is a summary of earlier blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<LineageRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Agent,
    Tool,
    Summary,
}

/// Documented heuristic: ~4 characters per token. Good enough for budgeting;
/// never for billing.
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

/// Ordered window with a token budget and pinning semantics.
///
/// Pinned blocks (`nervi`) survive every compaction byte-identical
/// (Law VI). Compaction itself is delegated: callers supply a summarizer.
#[derive(Debug, Clone)]
pub struct ContextWindow {
    blocks: Vec<Block>,
    next_seq: u64,
    budget: usize,
}

impl ContextWindow {
    pub fn new(budget: usize) -> Self {
        ContextWindow {
            blocks: Vec::new(),
            next_seq: 0,
            budget,
        }
    }

    pub fn budget(&self) -> usize {
        self.budget
    }

    pub fn push(
        &mut self,
        role: Role,
        content: impl Into<String>,
        pinned: bool,
    ) -> ForgeResult<()> {
        let content = content.into();
        let t = estimate_tokens(&content);
        if self.used() + t > self.budget {
            return Err(ForgeError::BudgetExhausted {
                used: self.used() + t,
                budget: self.budget,
            });
        }
        self.blocks.push(Block {
            seq: self.next_seq,
            role,
            content,
            pinned,
            lineage: None,
        });
        self.next_seq += 1;
        Ok(())
    }

    pub fn used(&self) -> usize {
        self.blocks
            .iter()
            .map(|b| estimate_tokens(&b.content))
            .sum()
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn is_pinned(&self, seq: u64) -> bool {
        self.blocks.iter().any(|b| b.seq == seq && b.pinned)
    }

    /// Compact unpinned blocks older than the most recent `keep_recent`
    /// unpinned entries into a single summary block with lineage.
    ///
    /// Returns the number of compacted blocks (0 if nothing to do).
    pub fn compact<S>(&mut self, keep_recent: usize, summarizer: S) -> ForgeResult<usize>
    where
        S: FnOnce(&[Block]) -> String,
    {
        // Indices of unpinned blocks eligible for compaction.
        let unpinned_idx: Vec<usize> = self
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| !b.pinned)
            .map(|(i, _)| i)
            .collect();

        if unpinned_idx.len() <= keep_recent {
            return Ok(0);
        }

        let cut = unpinned_idx.len() - keep_recent;
        let victim_idx: Vec<usize> = unpinned_idx[..cut].to_vec();
        let victims: Vec<Block> = victim_idx.iter().map(|&i| self.blocks[i].clone()).collect();
        let from_seq = victims.first().map(|b| b.seq).unwrap_or(0);
        let to_seq = victims.last().map(|b| b.seq).unwrap_or(0);

        let summary_text = summarizer(&victims);
        let summary = Block {
            seq: self.next_seq,
            role: Role::Summary,
            content: format!("[compacted {from_seq}..={to_seq}] {summary_text}"),
            pinned: false,
            lineage: Some(LineageRef { from_seq, to_seq }),
        };
        self.next_seq += 1;

        // Rebuild: skip victims, insert the summary where the first one sat.
        let first_pos = victim_idx[0];
        let old = std::mem::take(&mut self.blocks);
        let mut new_blocks: Vec<Block> = Vec::with_capacity(old.len() - victims.len() + 1);
        for (i, b) in old.into_iter().enumerate() {
            if i == first_pos {
                new_blocks.push(summary.clone());
            }
            if victim_idx.binary_search(&i).is_ok() {
                continue;
            }
            new_blocks.push(b);
        }
        self.blocks = new_blocks;

        if self.used() > self.budget {
            return Err(ForgeError::BudgetExhausted {
                used: self.used(),
                budget: self.budget,
            });
        }
        Ok(victims.len())
    }
}
