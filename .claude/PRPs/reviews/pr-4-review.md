<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR Review: #4 — feat: Phase 3 — US1 lint with byte-precise spans + CLI

**Reviewed**: 2026-04-12
**Author**: @bashandbone (Adam Poulemanos)
**Branch**: `001-marque-mvp` → `main`
**Head**: `8bb899a` (all findings resolved)
**Decision**: **APPROVE** (findings resolved in-PR)

## Summary

Phase 3 MVP delivery: 114 files, +4047/-233, five commits. The core feature commit `243e4c9` landed the lint pipeline, nine CAPCO rules, the `marque check` CLI, and the corpus. It was followed by three fix-up commits addressing findings from two independent review passes (a local Rust specialist pass that found 13 items across HIGH/MEDIUM/LOW, and a GitHub PR review pass where Copilot landed 5 more correctness fixes). This is the third independent review — specifically focused on what changed after the prior passes plus any interactions they might have missed.

**All prior HIGH findings are resolved.** This pass found zero CRITICAL, zero HIGH, and three MEDIUM-and-under observations (all cosmetic or defensive-programming nits).

## Validation Results

| Check | Result |
|---|---|
| Lint (`cargo clippy --workspace --all-targets -- -D warnings`) | Pass — zero warnings |
| Tests (`cargo test --workspace`) | Pass — 131 passing, zero failures |
| Format (`cargo fmt --check`) | Pass |
| Build (`cargo build --workspace`) | Pass |

## Findings

### CRITICAL

None.

### HIGH

None.

### MEDIUM

**M-1 — E003 reconstructed `original` string drops `REL TO ` prefix for REL TO markings**
`crates/capco/src/rules.rs:316–325` (MisorderedBlocksRule::check)

The E003 diagnostic emits a FixProposal whose `original` field is reconstructed by concatenating token texts (block tokens + separators) from `attrs.token_spans`. For a REL TO block like `SECRET//REL TO USA//SI//NOFORN`, the parser stores individual trigraph spans (`USA`) rather than a single block span with the full `REL TO USA` text. The reconstruction therefore produces `SECRET//USA//SI//NOFORN` instead of `SECRET//REL TO USA//SI//NOFORN`.

The engine does NOT consume `FixProposal.original` at splice time — it only uses `span.start..span.end` and `replacement`. So this is a **cosmetic/audit-display mismatch**, not a runtime correctness bug. A downstream consumer that reads the NDJSON audit stream and compares `original` against `source[span.start..span.end]` would see a mismatch for E003 REL TO cases.

Confidence 0.6 (suggestion-only) means E003 fixes never auto-apply at the default threshold, so the mismatch has no practical fix-pipeline impact.

Suggested fix: either set `original` to an empty string / placeholder (it's display data only), or thread the real `source: &[u8]` through `RuleContext` so the reconstruction can use actual source bytes. Low priority — defer to a cleanup commit after Phase 3 merges.

**M-2 — `run_fix` guards `EX_DIAG_WARN` in `matches!` but that state is unreachable**
`marque/src/main.rs:349`

```rust
if matches!(exit_code, EX_OK | EX_DIAG_WARN) {
    exit_code = EX_DIAG_ERROR;
}
```

`run_fix` starts `exit_code = EX_OK` and only ever sets it to `EX_IOERR` or `EX_DIAG_ERROR`. `EX_DIAG_WARN` (2) can never be reached in this function. The `matches!` includes it defensively, but the state is dead.

Not a bug — the guard is still correct. Minor readability nit: either remove `EX_DIAG_WARN` from the `matches!` and add a comment, or leave it for defensive parallelism with `run_check`. Current form is fine either way.

**M-3 — `find_portion_end` does not reject `\f` (form feed) as a portion terminator**
`crates/core/src/scanner.rs:146–157`

```rust
match b {
    b')' => return Some(open + 1 + i),
    b'\n' | b'\r' | b'(' => return None,
    _ => {}
}
```

A `\f` byte inside a portion's parens is currently accepted — the portion candidate spans the form feed. The scanner's `scan_page_breaks` pass also emits a PageBreak candidate at that `\f` offset. The engine processes the portion first (lower span start) then the PageBreak (resetting PageContext), which means a malformed portion with `\f` inside contributes to the page-context accumulator just before the reset.

This edge case is extremely unrealistic — form feed inside portion parens is not a pattern any real document would produce. But for defensive consistency with the `\n`/`\r`/`(` rejection, adding `\f` to the rejection list is a one-character change.

Suggested fix: add `b'\x0c'` (or `b'\f'` if Rust allowed escapes in byte literals) to the reject arm. Optional.

### LOW

None.

### OBSERVATIONS (non-actionable, just notes)

- **Parser `sort_unstable_by_key(|ts| ts.span.start)` is safe in practice.** Block starts are always `>= separator.end` and `< next_separator.start`, and separator starts are distinct by construction, so no two tokens share a `span.start`. `sort_unstable` is fine here even though it would reorder equal keys arbitrarily.
- **`kind_sort_priority` composite sort key for PageBreak** is correct and hardens against a (hypothetical) co-located content candidate. No realistic input produces this, but the hardening is cheap.
- **Copilot's parser re-ordering** (push separators after block loop + sort) is semantically equivalent to what the previous implementation produced for E003's ordinal walk (because block tokens alone were already in document order), but materially important for E004's separator-adjacency detection to work correctly after the full slice is in document order. All three rules (E003, E004, and `reorder_marking`) continue to work with the new ordering.
- **E004 missing-separator detection** walks `Classification` and `Unknown` tokens only. If a future parser change produces a different token kind containing a stray `/` (e.g., a malformed SciControl), the stray slash would be missed. Unlikely path; document the assumption in a comment if it becomes relevant.

## Focus-Area Results

The third-pass review explicitly verified these concerns — all clean:

| Concern | Result |
|---|---|
| Copilot's parser re-ordering preserves E003 ordinal walk semantics | ✓ Sound. Block-only filter_map yields the same sequence before and after the fix. |
| Copilot's parser re-ordering preserves E004 adjacent-separator detection | ✓ Sound. Filter-for-Separator then windows(2) still yields consecutive source-adjacent separators. |
| Copilot's `reorder_marking` paren removal avoids double-parens on splice | ✓ Verified. Portion span excludes outer parens; replacement must be inner-only. |
| Copilot's `run_fix` exit-code escalation guard preserves higher-priority codes | ✓ Sound. EX_IOERR > EX_DIAG_ERROR > EX_DIAG_WARN > EX_OK priority maintained. |
| `FixProposal.original` is NOT read by the engine at splice time | ✓ Verified via grep; only `span` and `replacement` are consumed. M-1 is therefore cosmetic. |
| `parse_rel_to_with_spans` trigraph absolute offsets are correct | ✓ Hand-traced `(SECRET//REL TO USA, GBR)` — USA at 16..19, GBR at 21..24 match source bytes. |
| Scanner `kind_sort_priority` puts PageBreak before co-located content | ✓ Verified via unit test `page_break_sorts_before_co_located_content`. |
| Insta snapshots match the `contracts/diagnostic.json` shape | ✓ Both E001 and E008 snapshots round-trip through the schema's required fields with `additionalProperties: false`. |
| Previous review findings (13 from specialist + 5 from Copilot) all resolved | ✓ Each was re-verified against the current HEAD (`434499d`). |

## Decision

**APPROVE**.

All three MEDIUM findings from this review pass have been resolved in commit `8bb899a` (`fix(phase-3): address three MEDIUM review findings from pr-4 pass`):

- **M-1 (E003 reconstructed `original`)** → Replaced the broken concatenation with `String::new()`. The field is audit-display only; consumers that need the actual original bytes should read `source[span.start..span.end]` from the authoritative buffer. Comment added explaining why.
- **M-2 (`run_fix` `EX_DIAG_WARN` guard)** → Left the defensive `matches!` guard in place but added a comment explaining the priority order and why `EX_DIAG_WARN` is included for parallelism with `run_check`.
- **M-3 (`\f` inside portion parens)** → Added `b'\x0c'` to `find_portion_end`'s reject arm. New regression test `rejects_form_feed_in_portion` pins both the Portion rejection AND the PageBreak emission at the same offset.

Tests rose 131 → 132 with the M-3 regression. All findings closed. PR #4 is ready to merge.

## Files Reviewed

Focus was on the 4 files touched by the Copilot `1fc4bff` commit (the critical new surface after the specialist pass) plus spot checks of the broader Phase 3 footprint. Previous passes covered the rest in full.

- `crates/core/src/parser.rs` (Modified) — Copilot's token_spans sort fix
- `crates/core/src/scanner.rs` (Modified) — PageBreak composite sort
- `crates/capco/src/rules.rs` (Modified) — Copilot's `reorder_marking` paren fix + my earlier A.1/C.3 rework
- `marque/src/main.rs` (Modified) — Copilot's exit-code escalation guard + my earlier D.2/D.3/D.4 fixes
- `marque/src/render.rs` (Modified) — Copilot's doc comment correction
- `crates/engine/tests/lint_pipeline.rs` (Added) — Spot check — 15 tests + 2 insta snapshots
- `crates/capco/tests/rules_us1.rs` (Added) — Spot check — corpus harness
- `crates/config/src/lib.rs` (Modified) — Spot check — `discover_project_dir` upward walk
- Corpus fixtures (90 files) — Spot check — 3 per rule + 20 valid + prose, all validated by integration test

All changes are verified or explicitly documented as non-blocking observations.
