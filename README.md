# ⚔️ Bellona

> **Dux belli.** The open-source agent harness forged from the lessons of the
> 2026 harness wars. `Agent = Model + Harness` — the model thinks, Bellona
> wages the war: tools, memory, governance, identity, fleets, receipts.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## Why

Frontier models cluster within points of each other. Published benchmark gaps
are mostly **harness engineering**. Yet every open harness today forces a
trade: lean-but-lawless (minimal loops), governed-but-hostage (platform lock-in),
or bold-but-unstable (pre-1.0 plugin everything).

Bellona refuses the trade. It is one structure where:

- **The Forge** — a micro-kernel; everything else is a capability-scoped plugin.
- **The Praetorian Gate** — every effect flows resolve → CEL policy (**fail-closed**) → hash-chained audit → then execute. No bypass path exists.
- **Vexillum** — every agent holds an Ed25519 keypair; events carry owner attestation. Models never hold keys.
- **Memoria** — tiered memory (pinned / scratchpad / SQLite archival) with sleep-time consolidation and lineage-based compaction.
- **Bellum** — pluggable strategies (ReAct, Plan-and-Execute) over a boring, reliable loop with budget caps and no-progress circuit breakers.
- **Castra** — sandbox ladder: process → container → gVisor/microVM → ephemeral cloud.
- **Officina & Ludus** — agents may forge new tools, but a tool is a civilian until it survives the Proving Grounds.
- **Foedus** — native MCP client+server, A2A, AG-UI, ACP. Bellona agents are citizens of any realm.
- **Vigiles & Colosseum** — OTel-convention tracing, full replay, and eval gates reporting **pass^k**, wired into CI.
- **Auxilia** — allied model clients: OpenAI-compatible (OpenAI/OpenRouter/vLLM/LM Studio/**Ollama**) + Anthropic.
- **Nuntii** — Telegram transport today; Discord/Slack tomorrow.
- **Tribunician Veto** — one command freezes outbound effects everywhere, including queued work and credentials.

Zero hostages: no feature requires anyone's cloud. Local-first always ships first.

## Quick start

```sh
# build the war machine
cargo build --release

# run the doctrine tests (the laws, enforced)
cargo test --workspace
```

### Drive a real campaign (Ollama, zero cloud)

```sh
# with Ollama running locally:
target/release/bellona \
  --goal "list the files here" \
  --workspace . \
  --model qwen2.5:7b --yolo
```

Reads are always allowed; writes are audited (`--yolo` auto-approves them);
the shell stays locked unless `--allow-shell`.

### Prove the machine (Colosseum)

```sh
# deterministic offline gate — no model provider needed:
target/release/bellona colosseum --suite suites/seed.json --offline
# exit 0 = pass^k 1.0 | 1 = reliability breach | 2 = budget breach
```

## Repository map

See [`BELLONA.md`](BELLONA.md) — the standing orders every contributor and
agent reads first. Design doctrine: [`docs/doctrine.md`](docs/doctrine.md).
Architecture deep-dive: [`docs/architecture.md`](docs/architecture.md).
Campaign status: [`docs/campaign.md`](docs/campaign.md).

## License

MIT © Bellona Works
