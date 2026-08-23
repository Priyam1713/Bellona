# Changelog

All notable changes to Bellona. Format: Keep a Changelog; semver honored.

## [0.1.0] — The Founding (Milestone I)

### The Forge (kernel)
- Seven primitives: loop, context, tool, session, policy, channel, memory.
- `PLUGIN_API_VERSION` contract constant (Law II).
- ContextWindow with token budgeting, pinning, and lineage-preserving
  compaction (Law VI).
- ToolRegistry with registration/exposure separation.
- Durable SessionStore with decision ledgers and search.

### Praetorian Gate (praetorium)
- **CustosGateway**: resolve → Lex → Annales → execute → settle, fail-closed
  at every stage, audit-before-execution (Law IV).
- **Lex**: CEL policy engine with structural deny-before-allow ordering,
  broken-rule refusal, default-deny on no match.
- **Annales**: SHA-256 hash-chained audit ledger with chain verification and
  Merkle-root export.
- **Vexillum**: Ed25519 agent keypairs + owner attestation countersigning;
  third-party verifiability without trusting the deployment (Law V).
- Approval tickets (approve/reject on the record) and the **Tribunician Veto**
  that cancels queued effects across layers.

### Bellum (the war loop)
- `WarLoop` driving strategies through the gate — denial is observation,
  never a bypass.
- ReAct strategy with max-steps and no-progress breakers (failure-mode
  catalog encoded as defaults).
- Plan-and-Execute with plan validation (hallucinated tools refuse before
  execution) and one-shot stale-plan replanning.
- CascadeRouter: phase→tier model routing (sol/terra/luna).

### Memoria
- Nervi (pinned vitals), Tabella (decision records), Archivum (pluggable
  durable store; local-first InMemoryArchivum), Somnium sleep-time daemon
  with dedup-consolidation (HeuristicConsolidator).

### Castra
- Sandbox ladder levels Prima→Quarta; ProcessDriver with env-scrub policy
  (PATH/locale/terminal/proxy only — secrets never inherit).

### Officina & Ludus
- Forged-tool proposals with manifest validation; proving-ground battery in
  sandbox; promotion requires passed verdict AND owner countersignature.

### Foedus
- MCP server+client traits, A2A AgentCard/delegation traits, AG-UI event
  translation from the bus + fanout, ACP room adapters.

### Vigiles & Colosseum
- OTel-GenAI-style span recorder; eval suites with pass^k reliability math;
  CI gates with honest exit codes (0/1/2) for reliability and budget.

### Ecosystem
- Armamentarium skill spec (foreign-host compatible), seed skills, registry
  JSON schema.
- TypeScript SDK (typechecked, zero-dep) and Python SDK (compileall clean).
- CI: fmt/clippy(-D warnings)/test on Windows runner + SDK jobs.
