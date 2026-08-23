//! Annales — the tamper-evident audit ledger.
//!
//! A hash chain: every record commits to its predecessor. Audit rows are
//! written BEFORE execution (Law IV) and outcomes settle into the same chain.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// One immutable row in the chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerRecord {
    pub seq: u64,
    pub ts_ms: u64,
    pub kind: String,
    pub payload: Value,
    /// Hex-encoded hash of the previous record ("genesis" for seq 0).
    pub prev_hash: String,
    /// Hex-encoded SHA-256 commitment of this record.
    pub hash: String,
}

const GENESIS: &str = "genesis";

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn digest(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update(b"\x1f"); // unit separator keeps field boundaries unambiguous
    }
    hex::encode(h.finalize())
}

/// The append-only ledger.
#[derive(Debug, Default, Clone)]
pub struct Annales {
    records: Vec<LedgerRecord>,
}

impl Annales {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a record; returns a copy with the computed chain hash.
    pub fn append(&mut self, kind: &str, payload: &Value) -> LedgerRecord {
        let seq = self.records.len() as u64;
        let ts = now_ms();
        let prev = self
            .records
            .last()
            .map(|r| r.hash.clone())
            .unwrap_or_else(|| GENESIS.to_string());
        let hash = digest(&[
            &seq.to_string(),
            &ts.to_string(),
            kind,
            &payload.to_string(),
            &prev,
        ]);
        let rec = LedgerRecord {
            seq,
            ts_ms: ts,
            kind: kind.to_string(),
            payload: payload.clone(),
            prev_hash: prev,
            hash,
        };
        self.records.push(rec.clone());
        rec
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records(&self) -> &[LedgerRecord] {
        &self.records
    }

    /// Recompute the whole chain; returns true iff untampered.
    pub fn verify_chain(&self) -> bool {
        let mut prev = GENESIS.to_string();
        for (i, r) in self.records.iter().enumerate() {
            if r.seq != i as u64 || r.prev_hash != prev {
                return false;
            }
            let recomputed = {
                let mut h = Sha256::new();
                for p in [
                    r.seq.to_string(),
                    r.ts_ms.to_string(),
                    r.kind.clone(),
                    serde_json::to_string(&r.payload).unwrap_or_default(),
                    r.prev_hash.clone(),
                ] {
                    h.update(p.as_bytes());
                    h.update(b"\x1f");
                }
                hex::encode(h.finalize())
            };
            if recomputed != r.hash {
                return false;
            }
            prev = r.hash.clone();
        }
        true
    }

    /// Merkle root over record hashes — the exportable fingerprint.
    pub fn merkle_root(&self) -> String {
        if self.records.is_empty() {
            return GENESIS.to_string();
        }
        let mut level: Vec<String> = self.records.iter().map(|r| r.hash.clone()).collect();
        while level.len() > 1 {
            let mut next = Vec::with_capacity(level.len().div_ceil(2));
            for pair in level.chunks(2) {
                let l = &pair[0];
                let r = pair.get(1).unwrap_or(l);
                next.push(digest(&[l, r]));
            }
            level = next;
        }
        level.remove(0)
    }
}
