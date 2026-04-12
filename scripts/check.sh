#!/usr/bin/env bash
# scripts/check.sh — Run all workspace quality checks.
#
# Usage: ./scripts/check.sh [--bench]
#   --bench  Also run the performance regression gate (scripts/bench-check.sh)
# Exit code: non-zero if any check fails.

set -euo pipefail

echo "==> cargo fmt --check"
cargo fmt --check

echo "==> cargo clippy --workspace --benches -- -D warnings"
cargo clippy --workspace --benches -- -D warnings

echo "==> cargo nextest run --workspace (or cargo test)"
if command -v cargo-nextest &>/dev/null; then
    cargo nextest run --workspace
else
    echo "    (cargo-nextest not found, falling back to cargo test)"
    cargo test --workspace
fi

if [[ "${1:-}" == "--bench" ]]; then
    echo "==> scripts/bench-check.sh (performance regression gate)"
    bash "$(dirname "$0")/bench-check.sh"
else
    echo "==> Skipping bench-check (pass --bench to enable)"
fi

echo "==> All checks passed."
