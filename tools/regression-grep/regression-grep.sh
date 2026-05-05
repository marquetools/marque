#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
#
# regression-grep.sh — pattern-anchored regression guards
#
# Each guard names: (a) the regex it forbids, (b) the file scope it
# enforces against, (c) the FR / CHK reference that mandates the gate,
# (d) the migration target (where forbidden code now lives).
#
# Doc-comment references that *name* a forbidden pattern (in the form
# `// ... <pattern> ...`) are excluded so guards don't fight their own
# documentation. The exclusion regex `^\s*[0-9]+:\s*//` matches lines
# that start (after `grep -n` prefix) with a comment marker — pragmatic,
# not bulletproof, but sufficient for the patterns this script guards.
#
# Run from workspace root: ./tools/regression-grep/regression-grep.sh
# Exit code: 0 on clean, 1 on any guard violation.

set -euo pipefail

# Resolve workspace root from this script's location (tools/regression-grep/).
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/../.." && pwd)
cd "$REPO_ROOT"

VIOLATIONS=0

# guard <pattern> <file> <fr> <chk> <migration_target>
guard() {
    local pattern="$1"
    local file="$2"
    local fr="$3"
    local chk="$4"
    local target="$5"

    if [ ! -f "$file" ]; then
        echo "regression-grep: ERROR — guarded file does not exist: $file"
        VIOLATIONS=$((VIOLATIONS + 1))
        return
    fi

    # `grep -nE` to get line numbers; pipe to grep -v exclude doc comments.
    # `|| true` so a zero-match `grep` (the desired clean state) does not
    # trip `set -e` before the violation check runs.
    local matches
    matches=$(grep -nE "$pattern" "$file" 2>/dev/null \
        | grep -vE '^\s*[0-9]+:\s*//' || true)

    if [ -n "$matches" ]; then
        echo
        echo "::error file=$file::$fr / $chk: forbidden pattern '$pattern' present"
        echo "  Migration target: $target"
        echo "  Hits:"
        echo "$matches" | sed 's/^/    /'
        VIOLATIONS=$((VIOLATIONS + 1))
    fi
}

# ---------------------------------------------------------------------------
# Guard 1: parser.rs must not re-introduce inline `is_ascii_alphanumeric()`
# byte-class checks for open-vocab admission.
#
# Background. PR 2 (specs/006-engine-rule-refactor) migrated four parser
# admission sites (one FGI trigraph silent-skip, three SAR shape checks)
# from inline `is_ascii_alphanumeric()` to vocabulary-surface predicates
# (`CountryCode::admits_fgi_trigraph`, `SarProgram::admits_program_id_*`,
# `SarCompartment::admits_identifier`) lifted into `marque-ism`. The
# admission contract and the parser are now pinned together by symbol.
#
# A `is_ascii_alphanumeric()` reintroduction in `parser.rs` would silently
# bypass that pin, re-opening GH #280 (silent open-vocabulary corruption).
# The grep is the guard. Doc-comment references that *name* the rule
# (e.g., "no inline `is_ascii_alphanumeric` byte-class checks") are
# allowed.
# ---------------------------------------------------------------------------

guard \
    'is_ascii_alphanumeric' \
    'crates/core/src/parser.rs' \
    'FR-015' \
    'CHK030' \
    'route through Vocabulary<CapcoScheme>::shape_admits or the lifted predicates in marque-ism (CountryCode::admits_fgi_trigraph, SarProgram::admits_program_id_*, SarCompartment::admits_identifier)'

# ---------------------------------------------------------------------------
# Result
# ---------------------------------------------------------------------------

if [ "$VIOLATIONS" -gt 0 ]; then
    echo
    echo "regression-grep: $VIOLATIONS guard(s) violated"
    exit 1
fi

echo "regression-grep: all guards clean"
exit 0
