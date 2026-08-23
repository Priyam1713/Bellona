# The Seven Laws — Design Doctrine

Every architectural decision in Bellona traces to a documented strength or a
documented failure observed across the 2026 harness landscape. This document
records the law, the evidence, and the enforcement mechanism.

---

## Law I — Lean core, iron edges

**Evidence.** The most effective coding harness of the era ships ~1.6% of its
code as AI-specific logic; the rest is deterministic infrastructure. A minimal
harness (~100 LOC) with one tool matches proprietary scaffolds within points
when the model is strong. Sub-1000-token system prompts with lazy-loaded
skills outperform walls of instructions.

**Decision.** The Forge kernel exposes exactly seven primitives and nothing
else: `loop`, `context`, `tool`, `session`, `policy`, `channel`, `memory`.
Strategy intelligence lives in plugins. If a pull request grows the kernel
beyond its mandate, it is rejected regardless of quality.

**Enforcement.** Kernel surface is enumerated in `forge::primitives`; CI fails
on new public kernel items without a doctrine-linked justification.

## Law II — Everything is a plugin, nothing is a hostage

**Evidence.** Micro-kernel "everything is a plugin" architectures won mindshare
at unprecedented speed in 2026 — and immediately warned users about breaking
changes, days-old plugins running with ambient trust, and docs lagging code.

**Decision.** All capabilities are replaceable plugins behind **frozen API
contracts**: semver, two-minor deprecation windows, annual LTS. Third-party
plugins target a capability-scoped WASM host (Milestone II); first-party native
plugins are privileged but reviewed.

**Enforcement.** `semver-checks` in CI; plugin trait objects versioned via
`PluginApiVersion` constant; contract tests per minor release.

## Law III — Zero hostages

**Evidence.** A prominent open agent platform requires an external vendor
service for durable threads and memory even on the free plan. Users noticed.

**Decision.** Every managed capability (memory consolidation, eval storage,
channel relay) must ship with a local-first implementation enabled by default.
Optional hosted variants are adapters, never dependencies. No workspace crate
may add a network dependency on a vendor SaaS.

**Enforcement.** Dependency review gate; `cargo deny`-style license/source
audit in CI.

## Law IV — Governance rides the gateway

**Evidence.** Production incident analyses converge on one pattern: wrap
probabilistic AI capabilities in deterministic infrastructure. The strongest
open platform design funnels every browser/file/MCP action through a single
gateway that resolves the target, evaluates policy, writes the audit row, and
only then acts — with fail-closed semantics: missing policy permits nothing,
broken rules refuse rather than open, deny evaluated before allow.

**Decision.** The Custos Gateway is the only path from decision to effect.
Policies are CEL expressions over typed attribute sets. Refusals name the rule
that caused them.

**Enforcement.** `praetorium::custos` integration tests prove: no-execute-
without-audit-row, deny-before-allow ordering, broken-rule refusal,
missing-policy refusal. Any bypass path is a critical defect by BELLONA.md.

## Law V — Identity before action

**Evidence.** A Nostr-based human+agent workspace demonstrated that giving
agents their own cryptographic keypairs — with owner attestation countersigned
onto every event, keys never visible to the model — produces verifiable provenance.

**Decision.** Vexillum mints an Ed25519 keypair per agent. Every event carries
`agent_signature` + `owner_attestation`. Signing happens in a tool boundary;
the model receives references, never key material.

**Enforcement.** `praetorium::vexillum` round-trip tests; gateway rejects
unsigned effects when identity enforcement is armed.

## Law VI — Sessions are infrastructure

**Evidence.** Independent teardowns of the strongest open harness credit its
durability model: sessions as first-class infrastructure — persistence,
recovery, full-text search, lineage-based compression — plus tiered memory
with sleep-time consolidation. Conversely, compaction-everywhere designs
accumulate loss across passes.

**Decision.** Memoria provides four tiers (nervi/tabella/archivum/somnium),
lineage-preserving compaction triggered well below window limits, and
decision-records over raw logs.

**Enforcement.** `memoria` tests assert lineage integrity across compaction;
sessions restore byte-identical pinned context.

## Law VII — Receipts or it didn't happen

**Evidence.** Identical models score up to 35 points apart depending on the
harness used to measure them; contamination inflates headline numbers; agents
pass once and fail on repeats. Marketing numbers are ceilings, not floors.

**Decision.** Colosseum publishes harness-controlled suites with pass^k
reliability metrics, cost-per-task, and tool-call precision/recall, wired as
CI gates. Docs are executed as tests where possible.

**Enforcement.** `vigiles::colosseum` computes pass^k and enforces thresholds
via exit codes; release CI runs the seed suite.

---

## Failure-mode catalog (encoded as defaults)

| Pattern          | Characteristic failure | Bellona default                          |
|------------------|------------------------|------------------------------------------|
| ReAct            | infinite loop          | max-step cap + escalation                |
| ReAct            | non-progress           | no-progress detector → circuit break     |
| Plan-and-Execute | stale plan             | observation-mismatch replan trigger      |
| Plan-and-Execute | planner hallucination  | plan validation before execution         |
| Reflexion        | critique loop          | critic budget + accept-on-timeout        |
| Reflexion        | critic capture         | rotatable critic, adversarial sampling   |
| State-graph      | graph sprawl           | max-node warnings, completeness tests    |
| Any              | denial-of-wallet       | Aerarium budgets + breakers at all tiers |
