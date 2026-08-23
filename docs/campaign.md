# Campaign Log â€” Total War

> Standing orders for the conquest phase. Bellona is safe; now she must be
> *inevitable*. Doctrine unchanged (BELLONA.md Seven Laws) â€” new operating
> principle added:
>
> **Small battles, total victory.** Every engagement below is sized to ship
> alone â€” code + tests + a runnable demo + a receipt (benchmark/report).
> No battle ends on "should work". Big picture is won by refusing to lose
> small ones.

## Status ledger

| Campaign | Theme | Status |
|----------|-------|--------|
| Iâ€“IV     | Founding Â· Real Blood Â· Surfaces Â· Optimization | âœ… shipped (see CHANGELOG) |
| V        | Arsenal Expansion | âš”ï¸ active |
| VI       | Voices | ✅ shipped 9ca7d45 |
| VII      | The War Room | ✅ shipped 9ca7d45 |
| VIII     | Legions | ✅ shipped e1bba95 |
| IX       | Foedus Rising (A2A) | ✅ shipped e1bba95 |
| X        | Living Memory | ✅ shipped b80a303 |
| XI       | Self-Forging Legionaries | ✅ shipped b80a303 |
| XII      | The Armory Market | ✅ host shipped 321a8da; marketplace registry next |
| XIII     | Proof Against the Gods | 🔧 infra done (runner+receipts+nightly held); live runs await API keys + workflow-scope token |

Sequencing rule: V â†’ VI â†’ VII run in parallel-friendly order; VIII depends on
V (worker tools) ; IX after VIII (teams delegate outward) ; X feeds VIII & XI
(memory makes both smarter) ; XI after V+VII (forging needs promotion UX) ;
XII last-but-one ; XIII strictly last.

---

## CAMPAIGN V â€” Arsenal Expansion
*Goal: an agent is only as dangerous as its toolbox. Fill it.*

### V1 â€” Tool ergonomics (force multiplier, do FIRST)
- **V1.1** `bellum::tools::tool!` declarative builder â€” define a tool from a
  plain async fn + doc comment (name/effect/schema inferred).
  *Victory:* existing 4 CLI tools rewritten with zero boilerplate loss.
- **V1.2** Conformance test-kit (`forge-testkit`): any tool must pass â€” spec
  validity, workspace-escape probes, scrubbed-env execution, JSON-schema round
  trip. One line to test a tool forever.
- *Watch for:* effect-kind drift between spec and actual behavior (writes
  disguised as reads). Testkit asserts `spec.read_only == !mutates`.

### V2 â€” Git operations
- **V2.1** Read-only set: `git_status`, `git_diff`, `git_log`, `git_blame`
  (subprocess via Castra Prima, cwd pinned to workspace).
  Policy: classified `file_read` â†’ auto-allowed.
- **V2.2** Mutating set: `git_add`, `git_commit`, `git_branch`, `git_worktree`.
  Classified `file_write` â†’ gated/yolo per existing law.
- **V2.3** E2E victory lap: scripted campaign edits a file â†’ commits on a
  branch â†’ assertions inspect a REAL temp repo (author, message, tree diff).
- *Watch for:* commit identity leakage (use camp-scoped author env, never the
  operator's global gitconfig); detached-HEAD footguns; Windows CRLF noise in
  diffs.

### V3 â€” Document search
- **V3.1** `search_files` (regex/literal, glob-scoped, respects .gitignore,
  result cap + pagination) â€” walk-and-scan pure Rust first.
- **V3.2** `read_document` chunked reader for long files (offset/limit) so a
  50k-line file cannot eat the context window (token discipline is Law I).
- **V3.3** Index hook: every search hit optionally appended to Archivum
  (`kind="episodic"`) â€” searches become memories.
- **V3.4** PDF extraction behind feature flag (`pdf-extract`), off by default.
- *Watch for:* binary-file poisoning (detect NUL bytes early), regex DoS
  (timeout every scan), case-fold across unicode.

### V4 â€” Web reading (HTTP-level browsing)
- **V4.1** `web_fetch`: GET-only, 2 MB cap, 15 s timeout, HTMLâ†’text
  (`html2text`), returns title + readable body.
- **V4.2** SSRF shield (critical small thing): resolve host â†’ refuse
  loopback/private/link-local/metadata IPs (169.254.169.254!), allowlist
  schemes http/https, redirect re-validation on EVERY hop.
- **V4.3** Policy wiring: populate real `page.url`/`page.host` attrs from the
  FINAL post-redirect URL; per-host Lex rules become possible.
- **V4.4** `web_search` adapter (Brave/DuckDuckGo HTML) behind explicit
  enable-flag â€” search engines are opt-in, never ambient.
- **V4.5 (stretch)** CDP-driven `browser_navigate/act` via `chromiumoxide`
  for JS-heavy pages â€” enters War-Room territory (VII.6 shares this driver).

**Campaign V exit criteria:** seed Colosseum suite extended with one case
per tool (offline EchoModel exercises plumbing; live variants wait for XIII).

---

## CAMPAIGN VI â€” Voices (Discord/Slack)
*Goal: same messenger muscle, three arenas.*

- **VI.0** Refactor: extract `ChannelTransport` trait from TelegramTransport
  (`poll() -> Vec<Inbound>` / `send(chat, text)`). Telegram becomes adapter #1,
  not a special case.
- **VI.1** Discord adapter: minimal Gateway v10 client over tokio-tungstenite â€”
  IDENTIFY, HEARTBEAT (respect server heartbeat_interval!), resume on close,
  MESSAGE_CREATE â†’ Inbound. No mega-framework dependency.
- **VI.2** Slack adapter: Socket Mode (wss) with `apps.connections.open`,
  event ACKs within 3 s (miss = duplicate delivery â€” dedupe on event_id),
  `chat.postMessage` outbound.
- **VI.3** Router config: `channels.yaml` mapping channelâ†’agent profile +
  workspace + per-channel Lex overlay + per-channel rate limit (messages/min)
  + allowed-principals allowlist.
- **VI.4** Secrets discipline: tokens from env only; redacted from traces
  (trace scrubber hook in vigiles).
- **VI.5** Tests: frame codec unit tests + local WS echo-server integration;
  reconnect storm simulation (kill socket 5Ã—, transport must self-heal).
- *Watch for:* Discord zombie-heartbeat after sleep; Slack retry replays
  (idempotent handling by event_id); multi-channel offset confusion.

**Exit criteria:** one bot account per network answers `!goal <text>` with a
full audited campaign, reply chunked to platform limits.

---

## CAMPAIGN VII â€” The War Room (watch your agent work)
*Goal: eyes on the battle â€” and take the wheel when it stalls.*

- **VII.1** `bellona serve` (axum): REST surface matching the TS SDK exactly
  (`/v1/gate/submit|approve|reject`, `/v1/ledger`, `/v1/sessions`) + `/v1/runs`
  POST that spawns campaigns.
- **VII.2** Live stream: SSE endpoint piping EventBus â†’ foedus AG-UI events
  (translation exists; wire + test ordering guarantees).
- **VII.3** Approval console endpoints: pending tickets list, approve/reject
  with principal attribution (feeds Annales automatically).
- **VII.4** Static war-room UI (single index.html + vanilla JS/htmx, served by
  the server â€” zero node build step): channel view, activity feed, approve
  buttons, ledger inspector with hash-chain verify badge.
- **VII.5** Take-the-wheel v1 (shell/files): when a tool call needs human
  hands (login walls come later with browsers), server parks the loop,
  exposes an input form, resumes with the observation. Events:
  `control_taken/control_released` already specced.
- **VII.6** Screen streaming (browser bots): `agent-computer` container
  (Chromium + CDP via chromiumoxide), MJPEG/PNG screenshot poll endpoint,
  input proxy; human drive-mode refuses bot actions (existing semantics).
  Docker-compose brings up server + computer.
- *Watch for:* SSE buffering behind proxies (send keepalives), CORS lockdown
  (same-origin default), screenshot size vs bandwidth (quality scaling),
  NEVER render raw model HTML without sanitization (generative UI = XSS
  surface).

**Exit criteria:** operator watches a campaign live, approves one write,
takes the wheel once, verifies the chain â€” all in a browser.

---

## CAMPAIGN VIII â€” Legions (agent teams)
*Goal: one Centurion, many specialists, zero chaos.*

- **VIII.1** `CenturioStrategy` in bellum: decomposes goal â†’ worker briefs
  (JSON plan-of-agents), dispatches, synthesizes.
- **VIII.2** Worker identity & scoping: each worker = fresh AgentId + own
  session + own Castra level; Lex gains `attr.worker.role` so rules can say
  *"researchers read, writers write"*.
- **VIII.3** Federated budgets: parent allocates child Aerariums; child
  overrun trips child breaker AND surfaces to parent â€” never silently eats
  the fleet budget.
- **VIII.4** Fleet governor: max concurrent workers, per-worker no-progress
  detection, fleet-wide veto (Tribunician Veto already cancels children via
  ticket cancellation â€” extend to spawned loops).
- **VIII.5** Result contracts: worker replies validated against declared
  schema before synthesis (garbage-in refused at the door).
- **VIII.6** E2E: researcher (reads) + writer (writes) complete a mini report
  in a temp workspace; ledger shows both identities; veto mid-run freezes both.
- *Watch for:* recursive spawning (workers spawning workers â€” forbid unless
  depth-tagged), context pollution from worker transcripts (parents get
  summaries only), thundering-herd on shared files (worktree isolation where
  git present).

---

## CAMPAIGN IX â€” Foedus Rising (A2A fluency)
*Goal: hire agents beyond your walls without lowering the gate.*

- **IX.1** Delegatee HTTP server (axum) implementing foedus A2A shapes;
  **idempotency ledger**: duplicate `idempotency_key` returns cached verdict
  (Annales-backed lookup) â€” retries must never double-fire effects.
- **IX.2** Delegator client + static discovery registry (`allies.yaml`: cards
  of known foreign agents); ARD-style dynamic discovery later.
- **IX.3** Foreign-task trust path: inbound tasks execute under a
  `foreign:<card-fingerprint>` identity through the SAME gate with its own
  restrictive Lex preset; unknown skills refused, not guessed.
- **IX.4** Capability negotiation: task routed only when card.skills âŠ‡ task
  requirements; stale-card TTL + refresh.
- **IX.5** E2E: two `bellona serve` processes â€” one delegates "research X"
  to the other; audit chains on BOTH sides interlock via task ids.
- *Watch for:* clock skew in TTLs, partial-failure semantics (delegatee died
  post-audit pre-reply â†’ idempotent replay), secret bleed in task contexts
  (context sanitizer).

---

## CAMPAIGN X â€” Living Memory
*Goal: recall like a colleague, forget like a professional.*

- **X.1** Embedding provider trait (OpenAI `/embeddings` shape + Ollama
  embeddings; Ollama = local-first default, zero hostages).
- **X.2** Vector store v1: embeddings as BLOBs in SQLite + brute-force cosine
  (honest up to ~100k vectors); documented upgrade path (sqlite-vec/HNSW).
  Batch inserts; dimension pinned per collection.
- **X.3** Hybrid recall: FTS âˆª vector â†’ reciprocal-rank fusion + recency
  decay; ONE `ArchivumStore::recall()` the loop actually uses.
- **X.4** Model-backed Somnium (LLM consolidator) behind existing trait;
  heuristic stays offline default; consolidation writes provenance
  (source episode ids) into distilled facts.
- **X.5** Time & truth: episodes carry `valid_at`; contradictions resolved
  latest-wins WITH provenance retained (never silent overwrite).
- **X.6** Memory evals in Colosseum: fixture conversations, questions about
  "three weeks ago", pass^k-gated.
- *Watch for:* embedding-dimension drift on provider switch (pin + version
  collections), cosine ties, chunking boundaries splitting facts, PII in
  long-term memory (redaction hook before put()).

---

## CAMPAIGN XI â€” Self-Forging Legionaries
*Goal: the agent that builds its own weapons â€” under inspection.*

- **XI.1** Trigger wiring: repeated `tool_not_found` observations (Nâ‰¥2) prompt
  the strategy to draft an Officina proposal (name/script/battery plan).
- **XI.2** Battery generation: model proposes cases; Ludus REQUIRES â‰¥3 cases
  including adversarial set (empty args, escape-attempt path, oversized input)
  â€” a tool untested against malice is untested.
- **XI.3** Promotion UX: verdict rendered to CLI/War-Room approval; owner
  countersignature (Vexillum) mandatory; refusal reasons recorded verbatim.
- **XI.4** Persistence & revocation: promoted tools land in
  `armamentarium/local/`; `bellona tools revoke <name>` tombstones them;
  revoked names cannot be re-promoted without new signature.
- **XI.5** Rate & blast-radius limits: forging attempts/session capped;
  forged execution floor = Castra Secunda; deny-pattern scan (egress URLs,
  credential-shaped strings) on every script template BEFORE trials.
- *Watch for:* battery overfitting (cases mirroring implementation instead of
  intent â€” require intent descriptions per case), tool-name squatting,
  forged-tool calling forged-tools (depth limit 1 until XII).

---

## CAMPAIGN XII â€” The Armory Market (WASM) *(paired with XIII at the end)*
*Goal: third-party power without third-party trust.*

- **XII.1** WIT contract v1: exports `describe()/call(input)->output`;
  imports granted individually â€” `cap_log`, `cap_kv`, `cap_http(host
  allowlist)`, `cap_fs_workspace`. Deny-by-default linker.
- **XII.2** `forge-plugins` host crate (wasmtime) mapping capabilities to
  real effects THROUGH the Custos gate (plugins are citizens, not exceptions).
- **XII.3** Packaging: `.bwasm` bundle = wasm + manifest + witness signature;
  `bellona plugin install/verify/list`; unsigned = advisory mode.
- **XII.4** Marketplace-lite: git-index registry (zero infrastructure);
  install from any git URL; supply-chain scan hook (deny-pattern + size +
  import-cap diff review).
- **XII.5** Boundary hardening: cargo-fuzz targets on the host ABI; cap-diff
  review prompt on upgrades (new capability = re-approval).
- *Watch for:* WASI sprawl (grant minimal preview1), memory limits per
  instance, epoch-interruption for runaway plugins, version-skew attacks on
  manifests (sign the manifest AND the bytes).

---

## CAMPAIGN XIII â€” Proof Against the Gods *(strictly last)*
*Goal: receipts against real intelligence. Build everything first; measure it second.*

- **XIII.1** Prereq: GitHub token with **workflow scope** (current blocker â€”
  flip Repository permissions â†’ Workflows: Read & write).
- **XIII.2** Live runner: `bellona colosseum --provider openai|anthropic|
  ollama [--model ...]` â€” Aerarium cost-caps enforced per suite; API keys env-
  only; redacted artifacts.
- **XIII.3** Nightly matrix in CI (openai/anthropic/ollama Ã— seed + swe-mini
  suites); failures open issues with full replay bundles.
- **XIII.4** Suites: `seed-live` (plumbing vs real models), `swe-mini`
  (temp-repo bugfixes verified by real test runs â€” SWE-bench-shaped, private
  fixtures to dodge contamination), `memory-recall` (from X.6).
- **XIII.5** Regression policy: fail PR if pass^k drops >5 pts vs stored
  baseline; baselines committed as versioned JSON artifacts.
- **XIII.6** Public receipts: auto-generated `docs/receipts.md` â€” harness-
  controlled numbers, model+tier+date cited, pass^k always shown (Law VII).
- *Watch for:* flaky-provider noise (quarantine lane, not silent retries),
  cost blowups at 3 AM (hard nightly spend ceiling), data contamination
  (fixtures stay private), key rotation drills.

---

## Risk register (the small things that kill big plans)

| Risk | Lives in | Mitigation |
|------|----------|------------|
| Workflow-scope token missing | XIII.1 | flip permission before campaign XIII |
| SSRF via redirects | V4.2 | re-validate every hop |
| Duplicate deliveries | VI.2/VI.3, IX.1 | event_id / idempotency ledger |
| Context window gluttony | IIIâ€“X | chunked readers, summary-only parents |
| Secret leakage into traces/memory | VI.4, X | redaction hooks at trace + put() |
| Recursive agent spawn | VIII | depth tags + forbidding default |
| Overfit proving batteries | XI.2 | adversarial-case minimums |
| Provider flakiness masking regressions | XIII | quarantine lane + pass^k windows |

## Definition of overall victory
A stranger clones Bellona, runs one command, watches their local model fix a
real repo issue in a browser window, approves one risky step themselves, and
reads tomorrow's public receipts showing pass^k â€” without ever trusting a
vendor promise.

