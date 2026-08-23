# ⚔️ Bellona

> **Give your local model a computer, a conscience, and a paper trail.**

The open-source agent harness where capability and safety stopped being a
trade-off. One binary turns *any* model — including free local ones via
[Ollama](https://ollama.com) — into an agent that reads files, runs commands,
browses the web, commits to git… while a fail-closed policy gate watches every
action, a tamper-evident ledger records everything, and one command freezes it all.

```text
you:  "fix the failing test in reports/"
  ⚙ bellona → reads repo → writes patch → runs tests → PASSES
  ⚙ audit chain: 14 decisions · 2 approvals · hash-verified ✔
```

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![tests](https://img.shields.io/badge/tests-79%20passing-3fb950)
![clippy](https://img.shields.io/badge/clippy-D--warnings-3fb950)

---

## The spark nobody else has

Every agent harness can *do things*. Bellona can **prove what it did** — to
your auditor, your customer, a court.

```sh
$ bellona verify --in receipts.json
chain: VALID ✔ · records: 14 · signed decisions: 6 · signature failures: 0
```

- Every action is **decided by a deterministic CEL policy before it runs**
  (fail-closed: broken rules refuse).
- Every action is recorded in a **SHA-256 hash chain** — tamper with one byte
  and verification screams.
- Every action carries an **Ed25519 agent signature + human owner
  countersignature**. The model never holds keys.
- One command (**Tribunician Veto**) freezes all agent activity — even work
  already queued.
- Third parties verify your deployment's behavior with `bellona verify`,
  needing nothing but the exported JSON. No database access. No trust.

Mapped line-by-line to EU AI Act obligations (Arts. 10–15, 24–26) in
[`docs/compliance.md`](docs/compliance.md).

---

## Why people star this

| | Claude Code / Codex | OpenClaw | **Bellona** |
|---|---|---|---|
| Runs on **free local models** | ✗/paid | ✓ | ✓ Ollama-first |
| Every action **policy-checked before execution** | partial | partial | ✓ fail-closed CEL gateway |
| Tamper-evident **audit ledger** | ✗ | ✗ | ✓ SHA-256 hash chain |
| Agents hold **cryptographic identities** | ✗ | ✗ | ✓ Ed25519 + owner countersign |
| **Per-role fleet law** (researchers read, writers write) | ✗ | ✗ | ✓ |
| **Kill switch** that halts queued work too | ✗ | ✗ | ✓ Tribunician Veto |
| Speaks MCP · A2A · AG-UI | MCP only | skills | ✓ all four directions |
| Zero vendor hostages | ✗ | partial | ✓ |

## Quickstart — 60 seconds

```sh
# 1. install ollama + pull any model (free, offline):
ollama pull qwen2.5:7b

# 2. build:
cargo build --release

# 3. march:
target/release/bellona --goal "list the rust files here" --yolo
```

Reads are always allowed. Writes are audited (`--yolo` auto-approves).
Shell stays locked unless `--allow-shell`.

### Prove the machine (no model needed)

```sh
target/release/bellona colosseum --suite suites/seed.json --offline
# exit 0 = pass^k 1.0 · 1 = reliability breach · 2 = budget breach
```

### Watch it live (War Room)

```sh
target/release/bellona serve --model qwen2.5:7b --yolo
# open http://127.0.0.1:3001 — launch campaigns, approve writes, veto anything
```

### Your agent in your chat

```sh
export TELEGRAM_BOT_TOKEN=...
target/release/bellona telegram --workspace ~/project --yolo
```

### Speak MCP from any editor

```sh
target/release/bellona mcp        # stdio JSON-RPC (Claude Desktop ready)
curl -X POST localhost:3001/mcp   # or streamable-http
```

## The Seven Laws

Every line of Bellona obeys [`BELLONA.md`](BELLONA.md): lean kernel,
plugin contracts with frozen APIs, zero vendor hostages, governance through
one gateway, cryptographic identity, durable sessions, and **receipts over
marketing** (`docs/receipts.md`).

## Repository map

Crate map, doctrine, architecture: start at [docs/doctrine.md](docs/doctrine.md)
→ [docs/architecture.md](docs/architecture.md) → [docs/campaign.md](docs/campaign.md).
Runnable recipes live in [`examples/`](examples/README.md).

## License

MIT © Bellona Works
