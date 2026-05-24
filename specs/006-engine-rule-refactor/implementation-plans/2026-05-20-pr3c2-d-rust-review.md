<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 3c.2.D — Rust-Mechanical Review

**Branch**: `refactor-006-pr-3c2-d-atomic-cutover` (commits D1-D10)
**Reviewer**: rust-reviewer (ecc:rust-reviewer agent)
**Date**: 2026-05-20

## Gate 0: Automated Checks

All four validation gates passed on the default build:
- `cargo check --workspace`: PASSED
- `cargo fmt --all --check`: PASSED
- `cargo clippy --workspace --all-targets -- -D warnings`: PASSED
- `cargo test --workspace`: PASSED
- `cargo test --doc`: PASSED (18+ compile-fail doctests in `rules` + `scheme`)

**NOTE**: `cargo test --workspace --features=corpus-override` was NOT run during automated checks. The HIGH finding below proves this path fails to compile.

## Verdict: BLOCK

One HIGH finding requires resolution before merge. Three MEDIUM findings.

## HIGH: `corpus_override.rs` Refers to Retired v1 `FixResult` API

**File**: `/home/knitli/marque/crates/engine/tests/corpus_override.rs`
**Lines**: 68, 88, 127, 143

`corpus_override.rs` is gated `#![cfg(feature = "corpus-override")]`, NOT `#![cfg(any())]`. It is a live test file that compiles when the feature is active.

The file iterates `result.applied` (line 68, 127) and accesses `fix.confidence.features` (line 88, 143), `fix.span.start`/`fix.span.end` (line 85-86, 143) from what it treats as a flat `AppliedFix`. Neither of these API surfaces exists on the post-D7 types:

- `FixResult.applied` — does not exist. The field is now `FixResult.audit_lines: Vec<AuditLine<CapcoScheme>>`.
- `fix.confidence` — does not exist on `AppliedFix<S>` v2. Confidence moved to `fix.fix.replacement.confidence`.

The v2 `AppliedFix<S>` fields are: `rule`, `severity`, `span`, `fix`, `source`, `message`, `timestamp`, `classifier_id`, `dry_run`, `input`. There is no top-level `confidence` field.

**Required fix**: Replace `&result.applied` with iteration over `result.audit_lines` (matching `AuditLine::AppliedFix` arm), and update field access from `fix.confidence.features` → `fix.fix.replacement.confidence.features`.

The vacuity guard (`decoder_fixes_examined >= 1`) and both test names are still sound; only the iteration path and field access need updating.

## MEDIUM: Stale Doc Link in `FixResult`

**File**: `/home/knitli/marque/crates/engine/src/output.rs`
**Line**: 196

`Self::applied` no longer exists. The correct link is `Self::audit_lines`. This is in the `r002_fired` field doc comment on `FixResult`.

## MEDIUM: Missing Required Compile-Fail Doctest 7.7

**File**: `crates/rules/src/audit.rs` (or `crates/rules/src/lib.rs`)

The rust-preflight (`docs/plans/2026-05-20-pr3c2-d-rust-preflight.md` §7.7) explicitly required a compile-fail doctest pinning that `AppliedFix::<()>::__engine_promote_text_correction` does not exist on the v2 type. The preflight called this doctest "load-bearing" because the FR-040 lint catches it at CI time but the doctest catches it at `cargo test --doc` time — an earlier gate.

Six of the seven required compile-fail doctests are present and correct:

| # | Location | Status |
|---|----------|--------|
| 7.1 | Covered by deletion of `AppliedFixProposal` (strong form) | Present (implicit) |
| 7.2 | `audit.rs:366` — No `Default for AppliedFix<S>` | Present |
| 7.3 | `audit.rs:391` — External brace-construct blocked | Present |
| 7.4 | `canonical.rs:196` — No `Serialize for Canonical<S>` | Present |
| 7.5 | `audit.rs:622` — `AppliedTextCorrection` not coercible to `AppliedFix<S>` | Present |
| 7.6 | `audit.rs:93` — `Discriminant` closed 2-variant | Present |
| 7.7 | — `AppliedFix::<()>::__engine_promote_text_correction` absent | **MISSING** |

## MEDIUM: Stale Module-Level Doc in `lib.rs`

**File**: `/home/knitli/marque/crates/rules/src/lib.rs`
**Lines**: 44, 995

Line 44 module doc: `AppliedFix<S>` wraps it (via the `AppliedFixProposal<S>` enum)`. `AppliedFixProposal<S>` is deleted in D7. Phrase should be removed.

Line 995 field doc: `[AppliedFix::__engine_promote_text_correction]` — method relocated to `AppliedTextCorrection`. Link should be updated.

## Attestation

- [X] Constitution V Principle V (G13) preserved
- [X] Constitution VII crate-graph boundary preserved
- [X] Constitution VIII citation discipline preserved
- [X] PM-D-1 through PM-D-16 ratified or deviations PM-justified
- [X] FR-040 lint coverage of v2 promote signatures (see security review for the related H-001 finding)
- [-] All 7 compile-fail doctests present and passing (6/7, see MEDIUM)
- [X] All 5 validation gates green (default features)
- [-] Walk-adjacent-code-paths discipline upheld (corpus_override.rs missed — HIGH)

## Summary of Required Actions Before Merge

1. **HIGH — BLOCK**: Fix `crates/engine/tests/corpus_override.rs` lines 68, 88, 127, 143 to use v2 API.
2. **MEDIUM — Recommended**: Add 7.7 compile-fail doctest to `crates/rules/src/audit.rs`.
3. **MEDIUM — Can merge with**: Fix stale `Self::applied` doc link at `crates/engine/src/output.rs:196`.
4. **MEDIUM — Can merge with**: Update stale `AppliedFixProposal<S>` references in `crates/rules/src/lib.rs` (line 44 module doc, line 995 field doc).
