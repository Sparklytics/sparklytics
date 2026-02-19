#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"

# ── Defaults (override via env) ──────────────────────────────
export SPARKLYTICS_PORT="${SPARKLYTICS_PORT:-3000}"
export SPARKLYTICS_DATA_DIR="${SPARKLYTICS_DATA_DIR:-$ROOT/data}"
export SPARKLYTICS_AUTH="${SPARKLYTICS_AUTH:-none}"
export SPARKLYTICS_HTTPS="${SPARKLYTICS_HTTPS:-false}"
export RUST_LOG="${RUST_LOG:-sparklytics=info}"

# ── Build ────────────────────────────────────────────────────
echo "Building sparklytics..."
cargo build --release --manifest-path "$ROOT/Cargo.toml" 2>&1 | tail -3

# ── Ensure data dir exists ───────────────────────────────────
mkdir -p "$SPARKLYTICS_DATA_DIR"

# ── Run ──────────────────────────────────────────────────────
echo ""
echo "Starting sparklytics on http://localhost:$SPARKLYTICS_PORT"
echo "  data dir : $SPARKLYTICS_DATA_DIR"
echo "  auth     : $SPARKLYTICS_AUTH"
echo ""
exec "$ROOT/target/release/sparklytics"
