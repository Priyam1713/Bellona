#!/usr/bin/env bash
# Bellona installer (Linux/macOS). Requires Rust 1.85+ (rustup).
set -euo pipefail

command -v cargo >/dev/null 2>&1 || {
  echo "cargo not found. Install via https://rustup.rs first." >&2
  exit 1
}

echo "[bellona] building the war machine (release)..."
cargo build --release

echo "[bellona] running doctrine tests..."
cargo test --workspace --quiet

echo
echo "Bellona is forged. See BELLONA.md for standing orders."
