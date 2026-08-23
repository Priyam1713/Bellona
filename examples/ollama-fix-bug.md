# Offline bugfix campaign (Ollama)

```sh
# 1. make a scratch repo with a failing test
mkdir bellona-demo && cd bellona-demo && git init -q
cat > lib.rs <<'EOF'
fn add(a: i32, b: i32) -> i32 { a - b }   // the bug
#[cfg(test)]
mod tests { #[test] fn two_plus_two() { assert_eq!(super::add(2,2), 4); } }
EOF
git add -A && git commit -qm "broken"

# 2. launch (free local model):
../target/release/bellona \
  --workspace . \
  --goal "make 'cargo test' pass in this workspace" \
  --yolo --allow-shell --model qwen2.5:7b

# 3. verify:
git log --oneline        # camp-scoped commit by "Bellona Agent"
```

What just happened: reads were auto-allowed, the write was audited, the shell
ran scrubbed (no env secrets), and every decision landed in the hash chain.
Inspect it: `sqlite3` not required — the ledger lives in-process; run with
`serve` to browse it in the War Room.
