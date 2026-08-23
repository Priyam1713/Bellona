//! Custos Ã¢â‚¬â€ the gateway. The ONLY path from decision to effect.
//!
//! Pipeline (Law IV): resolve Ã¢â€“Â¸ Lex Ã¢â€“Â¸ Annales Ã¢â€“Â¸ execute Ã¢â€“Â¸ settle.
//! Fail-closed at every stage. Audit rows precede execution. Refusals name
//! their rule.

use crate::annales::Annales;
use crate::error::PraetoriumError;
use crate::lex::{Lex, RULE_UNRESOLVED};
use crate::vexillum::VexillumService;
use forge::event::{BusEvent, EventBus};
use forge::primitives::{ActionRequest, Decision, Outcome, PolicyAttrs, ResourceInfo};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Resolves declared target URIs against a server-held snapshot.
pub trait TargetResolver: Send + Sync {
    fn resolve(&self, req: &ActionRequest) -> Result<ResourceInfo, PraetoriumError>;
}

/// Static snapshot resolver Ã¢â‚¬â€ the deployment's registry of known resources.
#[derive(Default)]
pub struct SnapshotResolver {
    resources: BTreeMap<String, ResourceInfo>,
}

impl SnapshotResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a resource; returns the previous entry if any.
    pub fn upsert(&mut self, info: ResourceInfo) -> Option<ResourceInfo> {
        self.resources.insert(info.uri.clone(), info)
    }

    pub fn len(&self) -> usize {
        self.resources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }
}

impl TargetResolver for SnapshotResolver {
    fn resolve(&self, req: &ActionRequest) -> Result<ResourceInfo, PraetoriumError> {
        // Exact URI match first...
        if let Some(info) = self.resources.get(&req.target_uri) {
            return Ok(info.clone());
        }
        // ...then prefix matches for hierarchical schemes (file://workspace/Ã¢â‚¬Â¦).
        for (uri, info) in &self.resources {
            if !uri.is_empty()
                && req.target_uri.starts_with(uri.as_str())
                && (req.target_uri.len() == uri.len()
                    || req.target_uri[uri.len()..].starts_with('/'))
            {
                return Ok(info.clone());
            }
        }
        Err(PraetoriumError::Refused {
            rule: RULE_UNRESOLVED.to_string(),
            reason: format!(
                "target '{}' is not in the registry snapshot",
                req.target_uri
            ),
        })
    }
}

/// The executor on the far side of the gate. Implementations are sandbox
/// drivers (Castra), MCP bridges (Foedus), or test doubles.
#[async_trait::async_trait]
pub trait EffectExecutor: Send + Sync {
    async fn perform(
        &self,
        req: &ActionRequest,
        resolved: &ResourceInfo,
        workspace: &std::path::Path,
    ) -> Result<serde_json::Value, String>;
}

/// An effect waiting on human approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalTicket {
    pub id: String,
    pub action: ActionRequest,
    pub rule_id: String,
}

/// What `submit` returns to the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "gate", rename_all = "snake_case")]
pub enum GateOutcome {
    /// Executed to completion.
    Executed { action_id: String, outcome: Outcome },
    /// Refused; carries the naming rule.
    Denied { rule_id: String, reason: String },
    /// Parked for human approval.
    PendingApproval { ticket_id: String },
}

/// Shared veto state Ã¢â‚¬â€ one bit above every layer.
#[derive(Default)]
pub struct VetoGuard(AtomicBool);

impl VetoGuard {
    pub fn raise(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_raised(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// THE gateway. Construct one per deployment; share it everywhere.
pub struct CustosGateway<R: TargetResolver, E: EffectExecutor> {
    resolver: R,
    executor: E,
    lex: std::sync::Mutex<Lex>,
    annales: Mutex<Annales>,
    identity: Mutex<VexillumService>,
    tickets: Mutex<BTreeMap<String, ActionRequest>>,
    workspace: PathBuf,
    bus: EventBus,
    veto: Arc<VetoGuard>,
    identity_enforced: bool,
}

impl<R: TargetResolver, E: EffectExecutor> CustosGateway<R, E> {
    pub fn new(resolver: R, executor: E, workspace: PathBuf) -> Self {
        CustosGateway {
            resolver,
            executor,
            lex: Mutex::new(Lex::empty()),
            annales: Mutex::new(Annales::new()),
            identity: Mutex::new(VexillumService::new()),
            tickets: Mutex::new(BTreeMap::new()),
            workspace,
            bus: EventBus::default(),
            veto: Arc::new(VetoGuard::default()),
            identity_enforced: false,
        }
    }

    pub fn with_identity_enforced(mut self, enforced: bool) -> Self {
        self.identity_enforced = enforced;
        self
    }

    pub fn bus(&self) -> EventBus {
        self.bus.clone()
    }

    pub fn veto_guard(&self) -> Arc<VetoGuard> {
        self.veto.clone()
    }

    /// Swap the law. Deployment refuses broken rules upstream (`from_specs`),
    /// so this is infallible by construction.
    pub fn install_law(&self, lex: Lex) {
        *self.lex.lock().expect("law poisoned") = lex;
    }

    pub fn identity_service(&self) -> &Mutex<VexillumService> {
        &self.identity
    }

    /// Exportable ledger snapshot for verification.
    pub fn ledger_snapshot(&self) -> Vec<crate::annales::LedgerRecord> {
        self.annales
            .lock()
            .expect("annales poisoned")
            .records()
            .to_vec()
    }

    /// Verify the whole chain.
    pub fn verify_ledger(&self) -> bool {
        self.annales
            .lock()
            .expect("annales poisoned")
            .verify_chain()
    }

    fn record(&self, kind: &str, payload: &serde_json::Value) -> u64 {
        let mut a = self.annales.lock().expect("annales poisoned");
        let rec = a.append(kind, payload);
        self.bus.publish(BusEvent::AuditCommitted {
            seq: rec.seq,
            hash: rec.hash.clone(),
        });
        rec.seq
    }

    /// THE path. Resolve Ã¢â€“Â¸ decide Ã¢â€“Â¸ audit Ã¢â€“Â¸ act Ã¢â€“Â¸ settle.
    pub async fn submit(&self, req: ActionRequest) -> Result<GateOutcome, PraetoriumError> {
        if self.veto.is_raised() {
            let err = PraetoriumError::Frozen("veto raised".into());
            self.record(
                "refusal_frozen",
                &json!({ "action": req.id.to_string(), "tool": req.tool_name }),
            );
            return Err(err);
        }
        self.bus.publish(BusEvent::ActionRequested {
            action_id: req.id.clone(),
        });

        // 1 Ã¢â€“Â¸ RESOLVE Ã¢â‚¬â€ against the snapshot; unresolvable refuses.
        let resolved = match self.resolver.resolve(&req) {
            Ok(r) => r,
            Err(e) => {
                self.record(
                    "refusal_unresolved",
                    &json!({ "action": req.id.to_string(), "target": req.target_uri }),
                );
                return Err(e);
            }
        };

        // 2 Ã¢â€“Â¸ DECIDE Ã¢â‚¬â€ deny-before-allow, fail-closed.
        let attrs = PolicyAttrs::from_request(&req, Some(&resolved));
        let decision = self.lex.lock().expect("law poisoned").decide(&attrs);

        // 3 Ã¢â€“Â¸ AUDIT BEFORE ACTION Ã¢â‚¬â€ decision row committed first.
        let identity = if self.identity_enforced {
            let svc = self.identity.lock().expect("identity poisoned");
            let digest = effect_digest(&req);
            let rec = svc.attest(&req.agent_id.to_string(), &digest)?;
            Some(rec)
        } else {
            None
        };

        self.record(
            "decision",
            &json!({
                "action": req.id.to_string(),
                "agent": req.agent_id.to_string(),
                "tool": req.tool_name,
                "effect": req.effect,
                "target": resolved.uri,
                "decision": decision,
                "identity": identity,
            }),
        );

        match decision {
            Decision::Deny { rule_id, reason } => {
                self.bus.publish(BusEvent::DecisionMade {
                    action_id: req.id.clone(),
                    verdict: "deny".into(),
                    rule_id: rule_id.clone(),
                });
                Ok(GateOutcome::Denied { rule_id, reason })
            }
            Decision::RequireApproval { rule_id } => {
                let action_id = req.id.clone();
                let ticket_id = format!("tkt_{}", req.id);
                self.tickets
                    .lock()
                    .expect("tickets poisoned")
                    .insert(ticket_id.clone(), req);
                self.bus.publish(BusEvent::DecisionMade {
                    action_id,
                    verdict: "require_approval".into(),
                    rule_id: rule_id.clone(),
                });
                Ok(GateOutcome::PendingApproval { ticket_id })
            }
            Decision::Allow { rule_id } => {
                self.bus.publish(BusEvent::DecisionMade {
                    action_id: req.id.clone(),
                    verdict: "allow".into(),
                    rule_id: rule_id.clone(),
                });
                let outcome = self.execute_settled(&req, &resolved).await;
                Ok(GateOutcome::Executed {
                    action_id: req.id.to_string(),
                    outcome,
                })
            }
        }
    }

    /// Approve a parked ticket; re-checks the veto and executes.
    pub async fn approve(
        &self,
        ticket_id: &str,
        approver: &str,
    ) -> Result<GateOutcome, PraetoriumError> {
        if self.veto.is_raised() {
            return Err(PraetoriumError::Frozen("veto raised".into()));
        }
        let req = self
            .tickets
            .lock()
            .expect("tickets poisoned")
            .remove(ticket_id)
            .ok_or_else(|| PraetoriumError::UnknownTicket(ticket_id.to_string()))?;

        self.record(
            "approval_granted",
            &json!({ "ticket": ticket_id, "approver": approver,
                     "action": req.id.to_string() }),
        );
        let resolved = self.resolver.resolve(&req)?;
        let outcome = self.execute_settled(&req, &resolved).await;
        Ok(GateOutcome::Executed {
            action_id: req.id.to_string(),
            outcome,
        })
    }

    /// Reject a parked ticket; the refusal is on the record.
    pub fn reject(
        &self,
        ticket_id: &str,
        approver: &str,
        reason: &str,
    ) -> Result<(), PraetoriumError> {
        let req = self
            .tickets
            .lock()
            .expect("tickets poisoned")
            .remove(ticket_id)
            .ok_or_else(|| PraetoriumError::UnknownTicket(ticket_id.to_string()))?;
        self.record(
            "approval_rejected",
            &json!({ "ticket": ticket_id, "approver": approver,
                     "action": req.id.to_string(), "reason": reason }),
        );
        Ok(())
    }

    /// The Tribunician VETO Ã¢â‚¬â€ freezes every layer. Queued approvals die on
    /// the record.
    pub fn veto(&self, reason: &str) {
        self.veto.raise();
        self.bus.publish(BusEvent::VetoRaised {
            reason: reason.to_string(),
        });
        self.record("veto_raised", &json!({ "reason": reason }));
        let mut t = self.tickets.lock().expect("tickets poisoned");
        for (id, req) in std::mem::take(&mut *t) {
            self.record(
                "ticket_cancelled_by_veto",
                &json!({ "ticket": id, "action": req.id.to_string() }),
            );
        }
    }

    async fn execute_settled(&self, req: &ActionRequest, resolved: &ResourceInfo) -> Outcome {
        let result = self.executor.perform(req, resolved, &self.workspace).await;
        let outcome = match result {
            Ok(v) => Outcome::Completed { result: v },
            Err(e) => Outcome::Failed { error: e },
        };
        self.record(
            "settlement",
            &json!({
                "action": req.id.to_string(),
                "outcome": outcome,
            }),
        );
        self.bus.publish(BusEvent::EffectSettled {
            action_id: req.id.clone(),
            ok: matches!(outcome, Outcome::Completed { .. }),
        });
        outcome
    }
}

/// Canonical digest over an effect request Ã¢â‚¬â€ what signatures commit to.
pub fn effect_digest(req: &ActionRequest) -> [u8; 32] {
    let canon = json!({
        "agent": req.agent_id.to_string(),
        "tool": req.tool_name,
        "effect": req.effect,
        "target": req.target_uri,
        "params": req.params,
        "intent": req.intent,
    });
    let bytes = serde_json::to_vec(&canon).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}
