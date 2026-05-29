#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2026 Knitli Inc.
#
# SPDX-License-Identifier: MIT OR Apache-2.0

# CI path filter. Classifies the files changed in this push / pull_request
# into coarse categories and emits one boolean per category to
# $GITHUB_OUTPUT. The CI workflow (.github/workflows/ci.yml) gates each job
# on the matching output, so a docs-only or config-only change skips the
# Rust matrix, the WASM build, the supply-chain audit, etc.
#
# Implemented as a plain git diff rather than a third-party path-filter
# action to keep CI dependency-free — the same supply-chain rationale the
# `reuse` job documents for installing its linter via pipx instead of a
# Marketplace action.
#
# Categories:
#   rust  — anything that can affect a Rust build / test / lint result.
#           Broad by design (the whole `crates/**`, `tools/**`,
#           `scripts/**`, `benches/**` trees, every Cargo manifest, the
#           toolchain pin, and this workflow). Over-triggering a fast job
#           is cheaper than silently skipping a real regression.
#   wasm  — only the WASM-safe crate set (Constitution III) plus the wasm
#           crate and the wasm-size tooling. A change to e.g.
#           marque-server can never alter the wasm artifact, so it must
#           not trigger the (slower) wasm build.
#   deps  — Cargo manifests / lockfile / supply-chain config: cargo-deny
#           (deny*.toml), cargo-audit (.cargo/audit.toml), cargo-vet
#           (supply-chain/).
#   refs  — vendored CAPCO reference PDFs (integrity checksum job).
#   flake — the flake-watch queue file (cap-check job).
#
# Env (set by the workflow):
#   BASE_SHA — PR base sha, or `github.event.before` on push.
#   HEAD_SHA — `github.sha`.

set -euo pipefail

BASE="${BASE_SHA:-}"
HEAD="${HEAD_SHA:-HEAD}"

emit() {
    # emit NAME true|false
    echo "$1=$2" >>"${GITHUB_OUTPUT:?GITHUB_OUTPUT not set}"
    echo "ci-detect-changes: $1=$2"
}

emit_all() {
    # emit_all true|false — set every category to the same value.
    local value="$1"
    local cat
    for cat in rust wasm deps refs flake; do
        emit "$cat" "$value"
    done
}

ZERO="0000000000000000000000000000000000000000"

if [[ -z "$BASE" || "$BASE" == "$ZERO" ]]; then
    # New branch (no `before` commit) or unknown base. Diff against the
    # parent commit when it exists; otherwise (the very first commit) we
    # cannot compute a delta, so run everything.
    if git rev-parse --verify --quiet "HEAD~1" >/dev/null; then
        BASE="$(git rev-parse HEAD~1)"
    else
        echo "ci-detect-changes: no base ref available; marking all categories changed" >&2
        emit_all "true"
        exit 0
    fi
fi

# `git diff --name-only BASE HEAD`. If BASE is not an ancestor (force-push,
# rebase) git still produces a file list; the over-inclusive direction is
# the safe one for a CI gate.
if ! CHANGED="$(git diff --name-only "$BASE" "$HEAD" 2>/dev/null)"; then
    echo "ci-detect-changes: git diff failed for ${BASE}..${HEAD}; marking all categories changed" >&2
    emit_all "true"
    exit 0
fi

if [[ -z "$CHANGED" ]]; then
    echo "ci-detect-changes: no files changed between ${BASE} and ${HEAD}" >&2
    emit_all "false"
    exit 0
fi

echo "ci-detect-changes: changed files:" >&2
while IFS= read -r path; do
    printf '  %s\n' "$path" >&2
done <<<"$CHANGED"

emit_match() {
    # emit_match NAME EXTENDED_REGEX
    local name="$1" re="$2"
    if grep -qE "$re" <<<"$CHANGED"; then
        emit "$name" "true"
    else
        emit "$name" "false"
    fi
}

RUST_RE='(\.rs$)|(/Cargo\.toml$)|(^Cargo\.(toml|lock)$)|(^rust-toolchain\.toml$)|(^crates/)|(^benches/)|(^tools/)|(^scripts/)|(^\.github/workflows/ci\.yml$)'
WASM_RE='(^crates/(ism|core|rules|scheme|capco|wasm)/)|(^Cargo\.lock$)|(^rust-toolchain\.toml$)|(^tools/wasm-size-check\.sh$)|(^tools/wasm-size-baseline\.txt$)|(^\.github/workflows/ci\.yml$)'
DEPS_RE='(/Cargo\.toml$)|(^Cargo\.(toml|lock)$)|(^deny(\.wasm-safe)?\.toml$)|(^supply-chain/)|(^\.cargo/audit\.toml$)'
REFS_RE='^crates/capco/docs/original-refs/'
FLAKE_RE='^tools/flake-watch/issues\.md$'

emit_match rust "$RUST_RE"
emit_match wasm "$WASM_RE"
emit_match deps "$DEPS_RE"
emit_match refs "$REFS_RE"
emit_match flake "$FLAKE_RE"
