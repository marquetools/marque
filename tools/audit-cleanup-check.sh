#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Knitli Inc.
#
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
#
# T138a — FR-037 absence-check (006 polish phase, PR 10.B).
#
# FR-037 from `specs/006-engine-rule-refactor/spec.md` declares a
# *negative* property: there is no `marque-audit-reader` crate, no
# reader-only feature flag, and no `marque_engine::reader::*` public
# surface. The clean-break audit-schema cutover (`marque-mvp-3` →
# `marque-1.0`, landed in PR 3c.2.D) depends on this property — pre-
# cutover audit records are unreadable by post-cutover binaries by
# *type-level* construction, not by convention.
#
# Negative properties need positive enforcement. Source plan §10.1
# (`docs/plans/2026-05-02-engine-refactor-consolidated.md`) calls for
# a CI absence-check; comment-propagated absence is the failure mode
# the murder board (W6) called out. This script is that gate.
#
# Three structural asserts:
#
#   1. No `crates/audit-reader/` directory exists.
#   2. No `audit-reader` / `marque-audit-reader` / `marque_audit_reader`
#      identifier appears as a name, dependency, or feature in any
#      `Cargo.toml` workspace member. Hit pattern is the whole-token
#      grep over `Cargo.toml` files, NOT a free-text search — doc
#      comments and prose that *mention* the absence are out of scope.
#   3. No `pub mod reader` or `pub use ...::reader` surface exists
#      under `crates/engine/src/`. Re-exports anywhere else would not
#      satisfy `marque_engine::reader::*`, so the scan scope is the
#      engine crate's source tree.
#
# Each failure prints a citation to the constitutional / spec passage
# the assert defends and exits non-zero. Exit 0 on all-clean.

set -euo pipefail

# Resolve repository root from the script's location so the check
# works from any CWD (CI checks out at $GITHUB_WORKSPACE; local runs
# may invoke from a sub-directory).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

EXIT_CODE=0

print_fail() {
    local title="$1"
    local detail="$2"
    local citation="$3"
    echo "audit-cleanup-check FAIL: ${title}" >&2
    echo "  ${detail}" >&2
    echo "  cite: ${citation}" >&2
    EXIT_CODE=1
}

# ---------------------------------------------------------------------------
# Assert 1: no `crates/audit-reader/` directory.
# ---------------------------------------------------------------------------
if [ -d "crates/audit-reader" ]; then
    print_fail \
        "crates/audit-reader/ directory exists" \
        "FR-037 forbids a reader crate. Found: crates/audit-reader/" \
        "spec.md FR-037; consolidated plan §10.1"
fi

# ---------------------------------------------------------------------------
# Assert 2: no audit-reader name / dep / feature in any Cargo.toml.
#
# Whole-token match on `audit[-_]reader` and `marque[-_]audit[-_]reader`.
# The `--include='Cargo.toml'` filter restricts the scan to manifest
# files — prose mentions of the absence in markdown / source comments
# are out of scope by construction.
#
# Scope: workspace-member roots only. `.worktrees/`, `.claude/`, and
# `target/` are deliberately excluded — a sibling worktree at
# `.worktrees/some-branch/` may legitimately reference experimental
# crate names without that work having merged to the active branch.
# The FR-037 invariant applies to the live workspace surface, not to
# every checked-out parallel branch.
# ---------------------------------------------------------------------------
CARGO_HITS="$(grep -RnE '\b(audit[-_]reader|marque[-_]audit[-_]reader)\b' \
    --include='Cargo.toml' \
    --exclude-dir='.worktrees' \
    --exclude-dir='.claude' \
    --exclude-dir='target' \
    . 2>/dev/null || true)"
if [ -n "${CARGO_HITS}" ]; then
    print_fail \
        "audit-reader identifier present in Cargo.toml" \
        "$(echo "${CARGO_HITS}" | sed 's/^/    /')" \
        "spec.md FR-037; consolidated plan §10.1"
fi

# ---------------------------------------------------------------------------
# Assert 3: no `pub mod reader` / `pub use ...reader` re-export in the
# marque-engine crate source tree.
#
# `pub mod reader` declares a public submodule named `reader`;
# `pub use ...::reader` re-exports a `reader` path. Either would expose
# `marque_engine::reader::*` from the engine crate. Anchor on those
# two forms specifically (not free-text `reader`) so the check stays
# structurally targeted and ignores e.g. an unrelated `reader: Reader`
# field on a local type.
# ---------------------------------------------------------------------------
if [ -d "crates/engine/src" ]; then
    READER_HITS="$(grep -RnE '^\s*pub\s+(mod|use)\s+([A-Za-z0-9_:]+::)?reader\b' \
        crates/engine/src/ 2>/dev/null || true)"
    if [ -n "${READER_HITS}" ]; then
        print_fail \
            "pub mod reader / pub use ...::reader present in marque-engine" \
            "$(echo "${READER_HITS}" | sed 's/^/    /')" \
            "spec.md FR-037; consolidated plan §10.1"
    fi
fi

if [ ${EXIT_CODE} -ne 0 ]; then
    echo "" >&2
    echo "audit-cleanup-check FAILED — see asserts above." >&2
    echo "FR-037 keeps the clean-break audit-schema property" >&2
    echo "(marque-mvp-3 → marque-1.0 cutover, PR 3c.2.D) a type-level" >&2
    echo "guarantee rather than a convention. Removing the reader" >&2
    echo "surface entirely is the enforcement." >&2
    exit ${EXIT_CODE}
fi

echo "audit-cleanup-check OK: FR-037 absence properties hold."
