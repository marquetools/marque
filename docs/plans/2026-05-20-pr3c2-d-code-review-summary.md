<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 3c.2.D — Code Review Summary

**Note**: full review written by ecc:code-reviewer to `/home/knitli/marque/docs/plans/2026-05-20-pr3c2-d-code-review.md`. This file is the PM-side summary for synthesis with the other 2 reviews.

**Verdict: WARNING** — 2 HIGH issues should be resolved before merge.

`cargo check --workspace --tests` passes clean. No CRITICAL issues found.

## HIGH-1: `crates/engine/tests/corpus_override.rs` — stale v1 API

Same finding as rust-reviewer's HIGH. See `2026-05-20-pr3c2-d-rust-review.md` for details.

## HIGH-2: `FixResult::applied_fixes()` / `applied_text_corrections()` allocate per call

**File**: `crates/engine/src/output.rs:227-251`
**Caller impact**: `crates/engine/benches/fix_latency.rs` calls `applied_fixes()` four times in one function; `proptest_engine.rs` calls it twice in one boolean expression.

Accessors look like cheap field reads but each allocates a fresh `Vec`. Stabilized at `marque-1.0`; switching return type post-stabilization would be breaking.

**Fix options**:
- (a) Change return type to `impl Iterator<Item = &AppliedFix<S>>` (zero-alloc, callers use `.count()` instead of `.len()`)
- (b) Document allocation explicitly + advise caching at call sites

PM decision: option (a). Zero-alloc iterators are the durable shape; pre-stabilization is the right time to change.

## MEDIUM-1: `specs/006-engine-rule-refactor/data-model.md` §"AppliedFix v2" stale

Lines 290-325. Shows `FixReplacement` enum (Strict/Decoder arms), `DateTime<Utc>`, `ClassifierId` newtype, missing `source: FixSource`. What landed per PM-D-7 is flat `AppliedReplacement<S>` struct, `SystemTime`, `Option<Arc<str>>`, with `source: FixSource`.

D10 updated `contracts/audit-record.md` but not `data-model.md`.

## MEDIUM-2: Stale `[Self::applied]` doc link

Same finding as rust-reviewer's MEDIUM.

## LOW-1: `AuditLine` wildcard arms silently drop future variants

A future `AuditLine` variant produces `Value::Null` from renderer + `None` from canary. Vacuity guard doesn't catch. Doc-note prevents.

## LOW-2: 13 `#![cfg(any())]` dead test files

`byte_identity_pr3c.rs`, `g13_closure_fix_intent.rs`, etc. Reference types that no longer exist (`AppliedFixProposal`, `result.applied`). Mislead grep sweeps + new contributors.

## LOW-3: CHANGELOG §Removed inaccuracy

`/home/knitli/marque/CHANGELOG.md:50` lists `mvp-1`/`mvp-2`/`mvp-3` but only `mvp-3` was the prior active schema.

## Constitution attestation

Principles I-VII: PASS. Principle VIII: PARTIAL (data-model.md stale per MEDIUM-1).
