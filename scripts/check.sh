#!/usr/bin/env bash
# scripts/check.sh — Run all workspace quality checks.
#
# Usage: ./scripts/check.sh
# Exit code: non-zero if any check fails.

set -euo pipefail

echo "==> cargo fmt --check"
cargo fmt --check

echo "==> cargo clippy --workspace -- -D warnings"
cargo clippy --workspace -- -D warnings

echo "==> cargo nextest run --workspace (or cargo test)"
if command -v cargo-nextest &>/dev/null; then
    cargo nextest run --workspace
else
    echo "    (cargo-nextest not found, falling back to cargo test)"
    cargo test --workspace
fi

echo "==> All checks passed."
