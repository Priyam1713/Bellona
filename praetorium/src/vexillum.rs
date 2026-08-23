//! Vexillum â€” identity of the war camp.
//!
//! Every agent carries an Ed25519 keypair (its *vexillum*, its standard).
//! Every effect is signed by the agent and countersigned by its human
//! owner's attestation key. The model never sees key material â€” signing
//! happens at this boundary only (Law V).

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use forge::ForgeError;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A signing identity: agent standard or owner seal.
pub struct VexillumKeypair {
    signing: SigningKey,
}

impl VexillumKeypair {
    pub fn generate() -> Self {
        VexillumKeypair {
            signing: SigningKey::generate(&mut OsRng),
        }
    }

    /// Hex-encoded public half â€” the only part events ever carry.
    pub fn public_hex(&self) -> String {
        hex::encode(self.signing.verifying_key().to_bytes())
    }

    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        self.signing.sign(msg).to_bytes()
    }

    pub fn verify(public_hex: &str, msg: &[u8], sig: &[u8; 64]) -> Result<(), ForgeError> {
        let raw =
            hex::decode(public_hex).map_err(|_| ForgeError::Other("bad public key hex".into()))?;
        let arr: [u8; 32] = raw
            .try_into()
            .map_err(|_| ForgeError::Other("bad public key length".into()))?;
        let vk = VerifyingKey::from_bytes(&arr)
            .map_err(|e| ForgeError::Other(format!("bad public key: {e}")))?;
        vk.verify(msg, &Signature::from_bytes(sig))
            .map_err(|e| ForgeError::Other(format!("signature verify failed: {e}")))
    }
}

/// The verifiable provenance block attached to audit rows when identity
/// enforcement is armed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRecord {
    pub agent_pub: String,
    pub owner_pub: String,
    /// Signature over the effect digest.
    pub agent_sig: String,
    /// Owner countersignature over (effect_digest || agent_sig).
    pub owner_sig: String,
}

impl IdentityRecord {
    /// Third-party verification â€” no trust in the deployment required.
    pub fn verify(&self, effect_digest: &[u8]) -> Result<(), ForgeError> {
        let agent_sig: [u8; 64] = hex::decode(&self.agent_sig)
            .ok()
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| ForgeError::Other("bad agent signature".into()))?;
        VexillumKeypair::verify(&self.agent_pub, effect_digest, &agent_sig)?;

        // The owner countersigned exactly (digest || raw_agent_sig).
        let mut counter_input = Vec::with_capacity(effect_digest.len() + 64);
        counter_input.extend_from_slice(effect_digest);
        counter_input.extend_from_slice(&agent_sig);

        let owner_sig: [u8; 64] = hex::decode(&self.owner_sig)
            .ok()
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| ForgeError::Other("bad owner signature".into()))?;
        VexillumKeypair::verify(&self.owner_pub, &counter_input, &owner_sig)?;
        Ok(())
    }
}

/// Registry of agent standards under one owner seal.
#[derive(Default)]
pub struct VexillumService {
    agents: HashMap<String, VexillumKeypair>,
    owner: Option<VexillumKeypair>,
}

impl VexillumService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set (or rotate) the owner attestation key.
    pub fn set_owner_keypair(&mut self, kp: VexillumKeypair) {
        self.owner = Some(kp);
    }

    /// Provision an owner key on first use (self-sovereign default).
    pub fn ensure_owner(&mut self) {
        if self.owner.is_none() {
            self.owner = Some(VexillumKeypair::generate());
        }
    }

    /// Mint a fresh standard for an agent.
    pub fn enroll_agent(&mut self, agent_id: &str) -> String {
        let kp = VexillumKeypair::generate();
        let public = kp.public_hex();
        self.agents.insert(agent_id.to_string(), kp);
        public
    }

    pub fn agent_public(&self, agent_id: &str) -> Option<String> {
        self.agents.get(agent_id).map(|k| k.public_hex())
    }

    /// Sign an effect digest as the agent, countersign as the owner.
    pub fn attest(
        &self,
        agent_id: &str,
        effect_digest: &[u8],
    ) -> Result<IdentityRecord, ForgeError> {
        let agent = self
            .agents
            .get(agent_id)
            .ok_or_else(|| ForgeError::Other(format!("unknown agent '{agent_id}'")))?;
        let owner = self
            .owner
            .as_ref()
            .ok_or_else(|| ForgeError::Other("no owner key enrolled".into()))?;

        let agent_sig = agent.sign(effect_digest);
        let mut counter_input = Vec::with_capacity(effect_digest.len() + 64);
        counter_input.extend_from_slice(effect_digest);
        counter_input.extend_from_slice(&agent_sig);

        Ok(IdentityRecord {
            agent_pub: agent.public_hex(),
            owner_pub: owner.public_hex(),
            agent_sig: hex::encode(agent_sig),
            owner_sig: hex::encode(owner.sign(&counter_input)),
        })
    }
}
