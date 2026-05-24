#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Knitli Inc.
#
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
#
# audit-reader absence-check.
#
# This check defends a *negative* property: there is no
# `marque-audit-reader` crate, no reader-only feature flag, and no
# `marque_engine::reader::*` public surface. The clean-break
# audit-schema cutover depends on this property — pre-cutover audit
# records are unreadable by post-cutover binaries by *type-level*
# construction, not by convention.
#
# Negative properties need positive enforcement: comment-propagated
# absence is the failure mode this gate guards against.
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
        "A reader crate is forbidden. Found: crates/audit-reader/" \
        "the audit-reader absence property (clean-break audit-schema cutover)"
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
# The invariant applies to the live workspace surface, not to
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
        "the audit-reader absence property (clean-break audit-schema cutover)"
fi

# ---------------------------------------------------------------------------
# Assert 3: no `pub mod reader` / `pub use ...reader` re-export in the
# marque-engine crate source tree.
#
# `pub mod reader` declares a public submodule named `reader`;
# `pub use ...::reader` re-exports a `reader` path. Either would
# expose `marque_engine::reader::*` from the engine crate.
#
# The detector covers four `pub use` shapes that all expose the same
# `marque_engine::reader::*` surface:
#
#   1. `pub use crate::reader`           — direct path
#   2. `pub use crate::a::b::reader`     — nested path
#   3. `pub use crate::{reader, ...}`    — brace-list with `reader`
#   4. `pub use crate::something as reader` — alias-renamed export
#
# Plus `pub mod reader` for the submodule-declaration form.
# Anchored on the structural forms above (not free-text `reader`) so
# unrelated `reader: Reader` fields or local-scope items are out of
# scope by construction.
# ---------------------------------------------------------------------------
if [ -d "crates/engine/src" ]; then
    READER_HITS="$(grep -RnE \
        '^\s*pub\s+mod\s+reader\b|^\s*pub\s+use\s+[A-Za-z0-9_:]*(::|[[:space:]])?reader\b|^\s*pub\s+use\s+[A-Za-z0-9_:]+\s*\{[^}]*\breader\b[^}]*\}|^\s*pub\s+use\s+[A-Za-z0-9_:]+\s+as\s+reader\b' \
        crates/engine/src/ 2>/dev/null || true)"
    if [ -n "${READER_HITS}" ]; then
        print_fail \
            "pub mod reader / pub use ...::reader present in marque-engine" \
            "$(echo "${READER_HITS}" | sed 's/^/    /')" \
            "the audit-reader absence property (clean-break audit-schema cutover)"
    fi
fi

if [ ${EXIT_CODE} -ne 0 ]; then
    echo "" >&2
    echo "audit-cleanup-check FAILED — see asserts above." >&2
    echo "This check keeps the clean-break audit-schema property a" >&2
    echo "type-level guarantee rather than a convention. Removing the" >&2
    echo "reader surface entirely is the enforcement." >&2
    exit ${EXIT_CODE}
fi

echo "audit-cleanup-check OK: audit-reader absence properties hold."
