<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# regression-grep — pattern-anchored regression guards

**FR**: FR-015 (and future FR additions per the in-script guard table)
**Checklist**: CHK030 (`specs/006-engine-rule-refactor/checklists/correctness.md`)
**Lands**: PR 2 (`feat/refactor-006-pr-2-shape-admits`)
**Owner**: PR-2 author until ownership rotates with the next pattern added

---

## What this is

A shell-only CI guard that fails the build if a forbidden source
pattern reappears in a file scope it was migrated out of. One file,
one script, no Rust toolchain — by design. Local-runnable for a
fast feedback loop (`./tools/regression-grep/regression-grep.sh`)
and CI-runnable as a single shell job.

Each guard records: forbidden regex, file scope, mandating FR / CHK,
and the migration target (where the now-forbidden code now lives).
A reviewer reading a guard violation should be able to fix it
without leaving the script.

## Why a script and not Rust

The first guard exists to keep `is_ascii_alphanumeric()` from
re-appearing in `crates/core/src/parser.rs` after PR 2 migrated four
admission sites to vocabulary-surface predicates. A regex-grep is
the appropriate weight class. A Rust AST lint (the citation-lint
shape) would be heavier than the problem and would trip on its own
test fixtures. If a future guard genuinely needs AST awareness,
that guard belongs in a sibling Rust crate (`tools/<guard>-lint/`),
not extended into this script.

## Doc-comment exclusion

Lines that start with a `//` comment marker (after the `grep -n`
line-number prefix) are excluded from match — guards must not fight
their own documentation. The exclusion regex (`^\s*[0-9]+:\s*//`)
covers the doc-comment case but is not a full Rust-comment parser:
strings containing `//` and other false-positive paths exist but
don't apply to the patterns this script currently guards against.
If a future guard needs strict comment-awareness, it belongs in an
AST lint, not here.

## Running

```bash
# From workspace root
./tools/regression-grep/regression-grep.sh
```

Exit code 0 on clean; exit code 1 (with `::error::` annotations) on
any guard violation.

## Adding a new guard

Add a `guard` invocation in `regression-grep.sh` with the five
fields documented at the top of the script. Add an item to the
list below, with a one-line description and a link to the spec
authority. Add a CI test that confirms both states (clean and
violation) — the existing pattern is to run the script in CI on
the post-migration tree (clean) and trust review to confirm it
would catch a regression (since adding back the forbidden pattern
to test the negative case is risky).

## Active guards

| # | Pattern | File scope | Authority | Migration target |
|---|---------|------------|-----------|------------------|
| 1 | `is_ascii_alphanumeric` | `crates/core/src/parser.rs` | FR-015 / CHK030 | `Vocabulary<CapcoScheme>::shape_admits` or the lifted predicates in `marque-ism` (`CountryCode::admits_fgi_trigraph`, `SarProgram::admits_program_id_*`, `SarCompartment::admits_identifier`) |

## Removing a guard

When a guard's authority is retired (e.g., the FR superseded), the
guard is removed in the same PR that retires the FR. Removal is
the cheap path; the discipline is keeping the guard list
self-justifying as it grows. A guard whose justification has gone
stale is worse than no guard.
