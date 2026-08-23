//! Campaign "Spark" â€” independent verification of a Bellona deployment's
//! receipts. A third party needs only this module (or any reimplementation
//! of two hashes) and the exported JSON: no database, no daemon, no trust.
//!
//! Checks:
//! 1. Hash chain: every record commits to its predecessor, genesis â†’ head.
//! 2. Signatures: every decision row carrying an IdentityRecord must verify
//!    against the effect it describes (agent sig + owner countersign).

use crate::annales::LedgerRecord;
use crate::custos::effect_digest;
use crate::vexillum::IdentityRecord;
use forge::primitives::{ActionRequest, EffectKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VerifyReport {
    pub chain_valid: bool,
    pub records: usize,
    /// Decision rows that carried signatures.
    pub signed_decisions: usize,
    /// Rows whose identity failed verification (empty = trustworthy).
    pub signature_failures: Vec<String>,
}

impl VerifyReport {
    pub fn fully_valid(&self) -> bool {
        self.chain_valid && self.signature_failures.is_empty()
    }
}

fn parse_effect(v: &Value) -> Option<EffectKind> {
    if let Some(s) = v.as_str() {
        return Some(match s {
            "file_read" => EffectKind::FileRead,
            "file_write" => EffectKind::FileWrite,
            "shell_exec" => EffectKind::ShellExec,
            "browser_navigate" => EffectKind::BrowserNavigate,
            "browser_act" => EffectKind::BrowserAct,
            "mcp_call" => EffectKind::McpCall,
            "memory_write" => EffectKind::MemoryWrite,
            "component_publish" => EffectKind::ComponentPublish,
            other => EffectKind::Custom(other.to_string()),
        });
    }
    // Custom variant serializes as {"custom":"..."}
    v.get("custom")
        .and_then(|c| c.as_str())
        .map(|c| EffectKind::Custom(c.to_string()))
}

/// Verify an export produced by `CustosGateway::export()`.
pub fn verify_export(export: &Value) -> Result<VerifyReport, String> {
    let records: Vec<LedgerRecord> = serde_json::from_value(
        export
            .get("records")
            .cloned()
            .ok_or("export missing 'records'")?,
    )
    .map_err(|e| format!("records decode: {e}"))?;

    let mut report = VerifyReport {
        chain_valid: AnnalesRef::verify(&records),
        records: records.len(),
        signed_decisions: 0,
        signature_failures: Vec::new(),
    };

    for r in &records {
        if r.kind != "decision" {
            continue;
        }
        let Some(identity_val) = r.payload.get("identity") else {
            continue; // enforcement wasn't armed on this row
        };
        report.signed_decisions += 1;

        // Rebuild the exact canonical request the signature committed to â€”
        // from data inside the row itself (self-contained receipts).
        let req = ActionRequest {
            id: forge::ActionId(forge::Id(
                r.payload
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            )),
            agent_id: forge::AgentId(forge::Id(
                r.payload
                    .get("agent")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            )),
            session_id: None,
            tool_name: r
                .payload
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            effect: r
                .payload
                .get("effect")
                .and_then(parse_effect)
                .unwrap_or(EffectKind::Custom("__unparseable__".into())),
            target_uri: r
                .payload
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            params: r
                .payload
                .pointer("/request/params")
                .cloned()
                .unwrap_or(Value::Null),
            intent: r
                .payload
                .pointer("/request/intent")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            worker_role: None,
        };
        let digest = effect_digest(&req);

        match serde_json::from_value::<IdentityRecord>(identity_val.clone()) {
            Ok(rec) => {
                if let Err(e) = rec.verify(&digest) {
                    report
                        .signature_failures
                        .push(format!("seq {}: {}", r.seq, e));
                }
            }
            Err(e) => report
                .signature_failures
                .push(format!("seq {}: identity decode: {e}", r.seq)),
        }
    }

    Ok(report)
}

/// Narrow alias so we don't leak the Annales struct into third-party code.
mod AnnalesRef {
    use super::*;
    pub fn verify(records: &[LedgerRecord]) -> bool {
        crate::annales::Annales::verify_records(records)
    }
}
