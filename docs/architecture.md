# Architecture

```
                            ┌─────────────────────────────┐
                            │      THE FORUM  (surfaces)  │
                            └──────────────┬──────────────┘
                                           │ AG-UI / ACP
                ╔══════════════════════════▼═══════════════════════════╗
                ║              THE PRAETORIAN GATE                      ║
                ║   Custos Gateway: resolve ▸ Lex ▸ Annales ▸ act       ║
                ╚══════════════════╤═══════════════╤═══════════════════╝
                                   │               │
        ┌──────────────────────────▼───┐   ┌───────▼────────────────┐
        │      THE FORGE (kernel)      │   │   THE FOEDUS           │
        └──────────────┬───────────────┘   └────────────────────────┘
                       │
     ┌─────────────────┼─────────────────┐
     ▼                 ▼                 ▼
  BELLUM         ARMAMENTARIUM        MEMORIA
     │                 │                 │
     ▼                 ▼                 ▼
 CENTURIO         OFFICINA            SOMNIUM
                  + LUDUS
     ┌──────────────────────────────────────────────────────┐
     │  CASTRA ladder │ AERARIUM budgets │ VIGILES traces    │
     └──────────────────────────────────────────────────────┘
                           ▼
                   TRIBUNICIAN VETO
```

## Crate dependency graph

```
forge  ◄── praetorium ◄── bellum ◄── (binaries, surfaces)
  ▲            ▲             │
  │            │             ▼
memoria ◀──────┼────── officina
  ▲            ▼             │
vigiles ◄──── castra ◄───────┘
foedus ◄──────┴── (all effectors)
```

Rules:
- `forge` depends on nothing internal.
- `praetorium` wraps all execution; `bellum` drives models and calls tools
  **only through** the gateway.
- Effectors (`castra`, `officina`, `nuntii`) implement forge traits.
- `vigiles` observes everything, is trusted by nothing.

## The decision-to-effect pipeline

Every agent-originated effect traverses five stages:

1. **Resolve** — the gateway resolves the declared target against a
   server-held snapshot of registered resources. Unresolvable targets refuse.
2. **Evaluate (Lex)** — CEL rules over typed attributes. Ordering: explicit
   deny first, then allow, then default-deny. Evaluation errors refuse.
3. **Record (Annales)** — a hash-chained row containing the decision, the
   rule trail, and the identity attestations, committed before execution.
4. **Execute** — dispatched to the sandbox driver selected by the Castra level.
5. **Settle** — outcome appended to the same chain; failures are recorded as
   first-class rows, never swallowed.

## Identity model

- `VexillumKeypair` per agent (Ed25519).
- `OwnerAttestation` countersigned by the human principal's key.
- Events: `{payload, agent_signature, owner_attestation}` — verifiable by
  third parties without trusting the deployment.

## Memory model

| Tier    | Latin    | Content                              | Lifetime |
|---------|----------|--------------------------------------|----------|
| L1      | nervi    | pinned goal + minimum proof          | session  |
| L2      | tabella  | task ledger, decisions-as-records    | session  |
| L3      | archivum | episodic/semantic/procedural stores  | durable  |
| daemon  | somnium  | consolidation distillation           | idle-time |

Compaction preserves lineage links: every summary block references the span
of source blocks it replaced, enabling replay-grade reconstruction.

## Protocol federation (Foedus)

| Protocol | Direction | Bellona role        |
|----------|-----------|---------------------|
| MCP      | down      | client + server     |
| A2A      | lateral   | delegatee + delegator |
| AG-UI    | up        | event emitter       |
| ACP      | lateral   | workspace adapter   |
