# BELLONA.md

> Project memory for agents and humans. Read this file before every session.
> It is the standing order of the Bellona war camp.

## What this repository is

Bellona is an open-source **agent harness**: the deterministic machinery that
turns a language model into an agent you can trust with real access.

Central formula:

    Agent = Model + Harness

The model thinks. The harness reads files, runs commands, enforces policy,
records history, manages memory, and coordinates other agents. Frontier models
are a near-tie; the harness decides who wins the war.

## The Seven Laws (non-negotiable)

1. **Lean core, iron edges** — the kernel stays small; deterministic
   infrastructure does the heavy lifting. No strategy logic in the kernel.
2. **Everything is a plugin, nothing is a hostage** — all capabilities are
   replaceable plugins behind frozen API contracts. Semver is sacred.
3. **Zero hostages** — no feature may *require* a vendor's cloud. Local-first
   fallbacks are mandatory for every managed capability.
4. **Governance rides the gateway** — every effect flows through the Custos
   Gateway: resolve → policy → audit → act. There is no bypass path.
5. **Identity before action** — every agent holds a keypair; every event is
   signed by the agent and countersigned by its owner. Models never hold keys.
6. **Sessions are infrastructure** — durable, resumable, searchable,
   lineage-compressed. Losing a session is a bug class we refuse.
7. **Receipts or it didn't happen** — benchmarks are published under
   harness-controlled conditions with pass^k. Docs are tested.

## Architecture map

| Crate        | Latin name      | Role                                              |
|--------------|-----------------|---------------------------------------------------|
| `forge`      | The Forge       | Kernel primitives, events, traits                 |
| `praetorium` | Praetorian Gate | Custos gateway, Lex policy, Annales ledger, Vexillum identity |
| `bellum`     | The War Loop    | Agent loop + strategy plugins (react, plan-execute) |
| `memoria`    | Memoria         | Tiered memory: nervi/tabella/archivum + somnium consolidation |
| `castra`     | Castra          | Sandbox ladder: process → container → gVisor → cloud |
| `officina`   | Officina/Ludus  | Self-forged tools + proving grounds               |
| `foedus`     | Foedus          | Protocol federation: MCP / A2A / AG-UI / ACP      |
| `vigiles`    | Vigiles         | Tracing, replay, Colosseum eval runner (pass^k)   |

Surfaces (TUI/web/desktop/channels) live outside the workspace core by design:
the war machine must run headless first.

## Invariants reviewers must enforce

- `praetorium::custos` is the ONLY path from an agent decision to an effect.
  Any new code path that executes a tool without passing through the gateway
  is a critical defect.
- Policy evaluation is **fail-closed**: missing policy permits nothing; a
  broken rule refuses rather than opens; deny is evaluated before allow.
- Audit rows are written BEFORE execution, not after.
- The kill switch (`tribunician_veto`) must halt queued effects and credential
  use across all layers. Never route around it.
- No crate in this workspace may add a network dependency on a vendor SaaS.

## Conventions

- Rust edition 2021, workspace versioning via `[workspace.package]`.
- Every public type carries a doc comment stating its Law alignment.
- Tests live beside code; integration tests in `crates/<name>/tests/`.
- Commit style: `area: imperative summary` (e.g., `praetorium: fail-closed lex`).

## Current campaign status

See `docs/campaign.md` for the marching orders of the current milestone.
