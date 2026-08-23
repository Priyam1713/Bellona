# Bellona & Regulatory Compliance (EU AI Act focus)

> **Not legal advice.** This is an engineering mapping: which Bellona
> mechanism produces evidence for which obligation, so your compliance team
> can decide coverage. As of August 2026, high-risk obligations under the EU
> AI Act are in force.

## Why this matters commercially

Most harnesses treat "audit log" as a feature checkbox. Bellona treats
**verifiability as architecture**: cryptographic identities, fail-closed
gates, and hash-chained receipts that a *third party* can verify without
access to your systems (`bellona verify`). When an auditor, customer, or
regulator asks *"prove what your agent did"*, competitors send screenshots.
Bellona sends math.

## Mapping

| AI Act theme | Articles | Bellona mechanism | Evidence artifact |
|---|---|---|---|
| Record-keeping / logs of operation | Art. 12 | Annales hash-chained ledger; audit-before-execution ordering; tamper-evident chain + Merkle root | `/v1/ledger/export` → `bellona verify` |
| Transparency to deployers/operators | Art. 13 | Human-readable refusal reasons naming the rule; AG-UI live event stream; War Room console | War Room `/v1/events`, audit viewer |
| Human oversight | Art. 14 | Approval gates on irreversible actions; human takeover protocol; **Tribunician Veto** halting queued effects and credentials | `approval_granted/rejected/cancelled_by_veto` rows |
| Accuracy & robustness | Art. 15 | Colosseum eval gates with pass^k reliability thresholds wired into CI; Aerarium budget breakers against runaway behavior | `docs/receipts.md`, suite reports |
| Responsibility allocation in the value chain | Art. 25 | Per-agent Ed25519 standards + **owner countersignature** on every effect; per-worker fleet roles scoped by policy | `IdentityRecord` inside each decision row |
| Data governance | Art. 10 | Workspace-scoped path resolution refusing escapes; secret redaction from transcripts/traces; binary/PII refusal hooks | testkit escape probes; redaction hooks |
| Sub-deployer oversight (foreign agents) | Art. 24/25 | A2A delegation with exactly-once idempotency ledger; foreign tasks enter under fingerprinted identity through the same gate | task ids interlocking both ledgers |

## The three-line pitch for your CISO

1. Every agent action is decided by a deterministic policy engine *before*
   execution and recorded in a tamper-evident chain *before* the effect runs.
2. Every action is signed by the agent's keypair and countersigned by a
   human owner's key — provable to any third party with one command.
3. One command freezes all agent activity everywhere, including work already
   queued.

## Gaps we declare openly

- Redaction is hook-based; deployments must configure PII rules per jurisdiction.
- Owner keys self-provision on first use; enterprises should supply hardware-backed keys (integration point exists).
- This document is engineering evidence mapping, not certification.
