# Campaign Log

> Marching orders for the current build-out. Update after each engagement.

## Milestone I — The Founding ✅
Eight crates, doctrine docs, 27 tests. See CHANGELOG 0.1.0.

## Milestone II — Real Blood ✅
- `auxilia`: OpenAI-compatible + Anthropic clients (httpmock-tested),
  tolerant JSON reply extraction, Ollama-first defaults.
- `memoria`: SqliteArchivum (FTS5-or-LIKE recall, WAL, reopen persistence);
  fixed `new_episode` id-collision bug that silently dropped episodes.
- `bellona` bin: full terminal agent — workspace-scoped tools with
  canonicalized path escapes refused, built-in Lex law, event stream to
  stderr, answer to stdout. E2E tests write real files through the real gate.
- 36+ tests green; clippy `-D warnings`; fmt clean.

## Milestone III — Surfaces & Receipts ✅
- `nuntii`: Telegram long-poll transport (chunked sends, offset dedupe,
  mock-server tested). Fixed missing `/` in Bot-API path caught by mocks.
- Colosseum runner wired into CLI: `bellona colosseum --suite suites/seed.json
  --offline` → pass^k report + honest exit codes (0/1/2).
- `EchoModel` deterministic interpreter (`say` / `write` verbs) for
  harness-plumbing gates that never touch a provider.
- Release-binary smoke test: seed suite pass^k = 1.0, exit 0.

## Milestone IV — Optimization & Completion ✅
- **Aerarium is real**: strategy-accrued model cost now reaches the loop;
  ceilings are hard — an over-budget Finish trips the breaker and the
  answer is preserved only as evidence.
- **Durable sessions**: SqliteSessionStore (WAL, reopen persistence,
  full-fidelity JSON blobs) behind the existing SessionStore trait.
- **Hot path**: Lex nests the attribute map once per decision instead of
  once per rule.
- Veto propagation verified at the bus (surface layers can react).
- Release profile: LTO + codegen-units=1; release smoke: colosseum seed
  pass^k 1.0, exit 0.

## Deferred (with rationale)
- **Wasmtime plugin host**: capability-scoped WASM remains the Law-II
  target (`forge::PLUGIN_API_VERSION=1` frozen); deferred because the
  component-model toolchain weight belongs in a dedicated campaign with its
  own supply-chain review. In-process native plugins remain first-party-only.
- Discord/Slack transports, AG-UI web surface, live-provider smoke suite
  (`BELLONA_LIVE_TESTS=1`).


## Engagements

| # | Engagement | Status |
|---|------------|--------|
| 1 | Root doctrine: README, BELLONA.md, doctrine.md, architecture.md | ✅ |
| 2 | forge: kernel primitives, events, tool/session/context traits | ✅ |
| 3 | praetorium: Lex (CEL, fail-closed), Annales (hash chain), Vexillum (Ed25519), Custos gateway | ✅ |
| 4 | bellum: WarLoop + ReAct/PlanExecute strategies + cascade router | ✅ |
| 5 | memoria: tiers + somnium consolidation trait | ✅ |
| 6 | castra: sandbox ladder abstraction | ✅ |
| 7 | officina/ludus: tool forging + proving ground gate | ✅ |
| 8 | foedus: protocol adapter traits | ✅ |
| 9 | vigiles: OTel-style spans + Colosseum pass^k runner + CI gates | ✅ |
| 10 | armamentarium spec + seed skills | ✅ |
| 11 | sdks: TypeScript + Python event-model skeletons | ✅ |
| 12 | CI workflows + install scripts | ✅ |

## Next campaign (Milestone II candidates)

- `forge-wasm`: Wasmtime plugin host with capability imports
- SQLite-backed Archivum + FTS5 recall search
- Real ModelClient adapters (OpenAI-compatible, Anthropic, Ollama)
- `forum-tui`: ratatui terminal surface
- Nuntii gateway: Telegram first
- Colosseum seed suite wired to GitHub Actions gates
