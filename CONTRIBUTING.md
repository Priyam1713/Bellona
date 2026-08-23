# Contributing

Read [`BELLONA.md`](BELLONA.md) first — it is the standing order for humans
and agents alike.

## Ground rules

1. Every PR must state which of the Seven Laws it touches and how.
2. Any new tool ships with a forge-testkit conformance test.
3. Any gateway change updates `praetorium/tests/laws.rs`.
4. Benchmarks are only accepted as harness-controlled runs with pass^k.

## Mechanics

- `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` must stay green.
- Commit style: `area: imperative summary`.
