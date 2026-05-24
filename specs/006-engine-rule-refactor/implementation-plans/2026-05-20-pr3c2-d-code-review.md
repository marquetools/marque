<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 3c.2.D Code Review

**Branch**: `refactor-006-pr3c2-d-atomic-cutover`  
**Reviewer**: general code-reviewer  
**Review date**: 2026-05-20  
**Verdict**: WARNING — 2 HIGH issues should be resolved before merge; 1 MEDIUM is documentation-only and can be addressed in D10's follow-up. No CRITICAL issues.

---

## Scope

Reviewed 11 commits (D1–D10 + style commit). Scope: the atomic `marque-mvp-3 → marque-1.0`
audit-schema cutover per FR-035a. Four structural commitments: `Canonical<S>` provenance
in audit emit, BLAKE3 digesting, closed `MessageTemplate` JSON, `AppliedFix` v2 reshape +
`AppliedTextCorrection` split.

## Pre-review verification

```
cargo check --workspace --tests   → clean (exit 0)
```

All mandated grep checks run. Full finding inventory below.

---

## [HIGH] `crates/engine/tests/corpus_override.rs` uses stale v1 API — fails when `corpus-override` feature is enabled

**File**: `crates/engine/tests/corpus_override.rs` lines 68, 74–78, 88, 127, 132–137, 143

```rust
// Line 68 — stale: result.applied does not exist on FixResult post-D7
for fix in &result.applied {

// Lines 74–78 — stale: fix.confidence.features is now at
// fix.fix.replacement.confidence.features (v2 shape)
let override_features: Vec<_> = fix
    .confidence
    .features
    .iter()
    .filter(|f| f.id == FeatureId::CorpusOverrideInEffect)
```

`FixResult.applied` was retired in D7; the sole audit channel is now `FixResult.audit_lines`.
`fix.confidence.features` is now `fix.fix.replacement.confidence.features` under the v2 shape.

This file is guarded by `#![cfg(feature = "corpus-override")]` and is never compiled in
the default `cargo test --workspace` build, so `cargo check` passes. However:
- CI runs `cargo test --features corpus-override` in the `pr-4b-corpus-regression` job
  (T145). That job will fail if the branch is merged and that job is triggered.
- Any developer enabling the `corpus-override` feature locally will see a compile error.

**Root cause**: This file was not included in the D6 "test fixtures migrated to v2 shape"
commit scope. The architect preflight (`2026-05-20-pr3c2-d-architect-preflight.md`) does
not mention `corpus_override.rs` in the T009a fixture migration inventory.

**Required fix before merge**: Migrate `corpus_override.rs` to the v2 API:
```rust
// Replace:
for fix in &result.applied {
    if fix.source != FixSource::DecoderPosterior { continue; }
    fix.confidence.features.iter()...

// With:
for fix in result.applied_fixes() {
    if fix.source != FixSource::DecoderPosterior { continue; }
    fix.fix.replacement.confidence.features.iter()...
```

---

## [HIGH] `FixResult::applied_fixes()` / `applied_text_corrections()` allocate `Vec` on every call — benchmark site calls each 3–4× per assertion

**File**: `crates/engine/benches/fix_latency.rs` lines 91–105  
**File**: `crates/engine/src/output.rs` lines 227–251

The two new accessors each allocate a fresh `Vec<&T>` on every call. The benchmark's
`assert_bench_invariants` function (called once before Criterion's measurement loop) calls
`applied_fixes()` four times:
```rust
fix_result.applied_fixes().len()    // line 92
fix_result.applied_fixes().len()    // line 97 (error message formatting)
fix_result.applied_fixes().iter()…  // line 99 (error message formatting)
&fix_result.applied_fixes()[0]      // line 105
```

While `assert_bench_invariants` runs outside the hot timing loop, the pattern establishes
a precedent: callers naturally call the accessor multiple times in the same scope because
it looks like a cheap field access (`result.applied_fixes()`) but actually allocates each
time. Downstream test code in `proptest_engine.rs` already calls `applied_fixes()` twice
in the same expression at line 143:
```rust
dry.applied_fixes().len() == apply.applied_fixes().len()
```

**Recommended fix**: Return `impl Iterator<Item = &AppliedFix<S>>` (lazy iterator, zero
allocation) or add an `applied_fixes_count()` shortcut alongside the collecting form.
Alternatively, document prominently (in the method signature's `#[inline]` or doc comment)
that callers should bind the result to a local variable and not call multiple times per
scope. The current doc comment does not mention the allocation:

```rust
// CURRENT (misleading by omission):
/// Filter the marking-side audit lines into a borrowed view.
#[inline]
pub fn applied_fixes(&self) -> Vec<&AppliedFix<CapcoScheme>> {

// BETTER (lazy, zero-alloc):
pub fn applied_fixes(&self) -> impl Iterator<Item = &AppliedFix<CapcoScheme>> {
    self.audit_lines.iter().filter_map(|line| match line {
        AuditLine::AppliedFix(f) => Some(f),
        _ => None,
    })
}
```

Migrating callers to the iterator form is mechanical: `.applied_fixes().len()` →
`.applied_fixes().count()`, `for fix in result.applied_fixes()` is unchanged (iterators
are `IntoIterator`), `result.applied_fixes()[0]` → `result.applied_fixes().next().unwrap()`.

This is flagged HIGH because the accessor is part of the public API surface this PR is
defining. The allocation behaviour should be intentional (documented) or fixed before
the shape becomes stable. Changing the return type from `Vec<&T>` to `impl Iterator` after
`marque-1.0` stabilizes is a breaking API change.

---

## [MEDIUM] `data-model.md` §"AppliedFix v2" shows stale pre-PM-D-7 type signatures

**File**: `specs/006-engine-rule-refactor/data-model.md` lines 290–325

The spec's data model still shows:
```rust
// Stale: FixReplacement enum with Strict/Decoder variants (pre-PM-D-7 design)
pub enum FixReplacement {
    Strict { canonical: Canonical<CapcoScheme>, confidence: Confidence },
    Decoder { canonical: Canonical<CapcoScheme>, confidence: Confidence },
}

// Stale: DateTime<Utc> (actual: SystemTime)
pub timestamp: DateTime<Utc>,

// Stale: ClassifierId newtype (actual: Arc<str>)
pub classifier_id: Option<ClassifierId>,

// Missing: source: FixSource field (present in actual AppliedFix v2)
```

What actually landed per PM-D-7:
- `FixReplacement` enum was NOT implemented. Instead `AppliedReplacement<S>` is a flat
  struct with `canonical: Canonical<S>`, `confidence: Confidence`, and
  `bytes_digest: Blake3Hash`. The `discriminant` is derived at audit-emit time via
  `discriminant_from_source(fix.source)`, NOT stored on the struct.
- Timestamp is `std::time::SystemTime`, not `DateTime<Utc>`.
- `classifier_id` is `Option<Arc<str>>`, not a `ClassifierId` newtype.
- `source: FixSource` is present on the struct but absent from the spec.

**Impact**: A reviewer or future implementer consulting `data-model.md` to understand the
`AppliedFix` v2 shape will see a materially different structure than what exists. Constitution
VIII (Authoritative Source Fidelity) applies to spec documents as well as code citations.

**Required fix**: Update `data-model.md` §"AppliedFix v2" to reflect the as-landed
`AppliedReplacement<S>` flat-struct design and correct field types. The note at line 322
("The `Canonical<S>` already encodes provenance") is accurate and should be preserved.

---

## [MEDIUM] `FixResult::r002_fired` doc comment at line 196 has stale link to retired `Self::applied`

**File**: `crates/engine/src/output.rs` line 196

```rust
/// - [`Self::source`] holds the post-pass-1 buffer ONLY. Pass-2
///   never ran, so any pass-2 fixes that would have applied are
///   absent from [`Self::applied`].    ← STALE: 'applied' field was retired in D7
```

`FixResult::applied` was retired when the sole audit-output channel became
`FixResult::audit_lines`. The link `[Self::applied]` is a dead doc-link that will produce
a `rustdoc` warning (`use of undeclared identifier 'applied'`) if `cargo doc` is run with
`--deny=warnings`.

**Fix**: Replace `[Self::applied]` with `[Self::audit_lines]`.

---

## [LOW] `AuditLine` wildcard-arm gap: future variants silently drop through both the renderer and the G13 canary

**File**: `marque/src/render.rs` line 887  
**File**: `crates/engine/tests/audit_g13_canary.rs` line 221

The `audit_line_to_json_v1_0` renderer returns `Value::Null` for unknown `AuditLine`
variants. The G13 canary's `render_audit_line_to_json` function returns `None` for the
same wildcard arm, and the canary's corpus-sweep loop calls `continue` on `None`. The
canary's vacuity guard (`total_lines_scanned > 0`) does not detect this because the known
variants still produce records.

**Effect**: A future `AuditLine::SomeNewVariant(...)` would silently produce `Null` in
the renderer, be silently skipped by the canary, and never receive a content-ignorance
check. Both wildcard arms are independently correct per `#[non_exhaustive]` discipline,
but the combination creates a silent omission rather than a loud signal.

**Note**: This is not a bug in D's scope — the `#[non_exhaustive]` design and wildcard
handling are correct. The gap is inherent to the `#[non_exhaustive]` + wildcard pattern.

**Suggestion**: Add a tracing-level log or a compile-time doc-note that a new `AuditLine`
variant requires updates to both the renderer arm AND the canary's `render_audit_line_to_json`
helper. Alternatively, extract the permitted-identifier scan from the canary into a standalone
function that can be reused when the renderer adds a new arm — making the canary extensible
is cheaper than documenting the omission.

---

## [LOW] `#![cfg(any())]` dead-file accumulation — 13 permanently disabled test files

**Files**: `crates/capco/tests/byte_identity_pr3c.rs`, `g13_closure_fix_intent.rs`,
`fix_intent_round_trip.rs`, and ~10 others.

These files are permanently disabled with the comment "PR 3c.B Commit 10: legacy
FixProposal-shape test disabled pending rewrite." After D6's fixture migration, the
test bodies are fully superseded. Dead files carrying `AppliedFixProposal`,
`result.applied`, and `FixIntent | TextCorrection` patterns are a maintenance trap:
- New contributors see code referencing types that don't exist and spend time debugging.
- `grep` sweeps (like this review's adjacent-code-path checks) require filtering.
- Accumulated dead files suppress the "something is wrong" signal when a real
  compile error exists in one of them.

These files were outside D's explicit scope (PM-D-4 / D-D-5: fixture migration is atomic
in D6 for ACTIVE fixtures). However, the deferred-rewrite label is now stale: the rewrites
happened in D6 (parity tests covering the same rules are active in `fix_pipeline.rs`,
`audit.rs`, `audit_g13_canary.rs`). The `#![cfg(any())]` files can be deleted.

**Suggested follow-up**: File a cleanup issue. Deleting the files removes ~1500 lines
of dead code at zero risk (they are never compiled).

---

## [LOW] `CHANGELOG.md` §Removed says "`mvp-1` / `mvp-2` / `mvp-3`" but only `mvp-3` was active at cutover

**File**: `CHANGELOG.md` line 50

```markdown
- Pre-cutover `marque-mvp-1` / `marque-mvp-2` / `marque-mvp-3`
  envelope shapes — accept-list contracts to a single value
  `["marque-1.0"]`.
```

The build.rs accept-list previously contained `["marque-mvp-3"]`; it never contained
`mvp-1` or `mvp-2` simultaneously. Listing all three in the "Removed" section implies
they were all valid at the cutover point, which is misleading. The audit-record.md
contract correctly states the prior active schema was `marque-mvp-3`.

This is cosmetic and does not affect correctness. The contract document and CHANGELOG
both note that pre-cutover records are not interoperable, which is the load-bearing
consumer-facing fact.

---

## Constitution checklist

| Principle | Assessment |
|-----------|------------|
| I — Uncompromising Performance | PASS. BLAKE3 digesting at promotion time (already on the allocation boundary). No hot-path allocations added. Bench invariant check (`assert_bench_invariants`) runs outside Criterion timing loop. |
| II — Zero-Copy, Streaming Core | PASS. `original_bytes: &[u8]` accepted at promotion, digested, not stored. `FixResult.source` retains `SecretSlice<u8>` wipe-on-drop. |
| III — Format-Agnostic Core / WASM Safety | PASS. All new types in WASM-safe crates (`marque-rules`, `marque-scheme`). `blake3` added to both; `Blake3Hash = blake3::Hash` (not a newtype). `#![cfg(not(target_arch = "wasm32"))]` guard on the parity test is correct. |
| IV — Two-Layer Rule Architecture | PASS. No rule implementations changed. Engine crate touch is authorized: D1 wires `blake3` dep (infra), D2/D3 add audit types (structural commitment per FR-035a), D7 deletes pre-cutover v1 path. |
| V — Audit-First Compliance | PASS. `__engine_promote` is the sole promotion path in production code. Four `#[cfg(test)]` carve-out sites in `render.rs`, `audit_v1_0_parity.rs`, `audit_g13_canary.rs` each carry the required inline comment naming the carve-out. G13 invariant is now a type + canary invariant. |
| VI — Dataflow Pipeline Model | PASS. `AuditLine<S>` sum-type preserves FR-016 promotion order across both channels. |
| VII — Crate Discipline | PASS. `blake3` added to `marque-rules` and `marque-engine` only. Dependency graph acyclic. `corpus_override.rs` compile break (HIGH finding) is in a `[dev-dependencies]`-scoped context so it does not affect the crate graph. |
| VIII — Authoritative Source Fidelity | PARTIAL. No CAPCO citations added by D (correct — this is an infrastructure PR). `data-model.md` §"AppliedFix v2" shows stale type signatures (MEDIUM finding). The CHANGELOG and contract doc are accurate regarding the schema flip and what was removed. |

---

## Accessor method design evaluation

`FixResult::applied_fixes()` and `FixResult::applied_text_corrections()` return
`Vec<&T>` (allocating). The design choice is documented in the method comment
("preserves the pre-cutover read shape for consumers that only need marking fixes")
which is a valid rationale. The HIGH finding above recommends either changing the return
type to `impl Iterator` or explicitly documenting the allocation in the signature.

The `AuditLine<S>` sum type itself is well-designed: `#[non_exhaustive]` for future
variants, manual `Clone` to avoid over-constraining `S: Clone`, cross-record promotion
order preserved by interleaving both channels into a single `Vec<AuditLine>`.

---

## Test quality

- **T055 G13 canary** (`audit_g13_canary.rs`): Well-constructed. Self-test (`canary_fires_on_synthetic_regression`) demonstrates the detection path. Vacuity guard ensures the corpus produces at least one audit line. Three false-positive guard tests (`permits_blake3_digest_strings`, `permits_span_integer_overlap`) pin the known permitted-identifier exemptions.
- **SC-008 parity test** (`audit_v1_0_parity.rs`): Thorough. Covers all discriminant routes (BuiltinRule → strict, DecoderPosterior → decoder, MigrationTable → strict, DecoderClassificationHeuristic → decoder), both record types, optional-field null-emit, and BLAKE3 digest format.
- **D9 CLI version test** (`cli_version.rs`): Minimal and correct. Pins the `^audit_schema:` grep target and the ordering invariant.
- **Constitution V Principle V carve-out comments**: Present at all four active `__engine_promote` call sites. ✓

---

## Summary

| Severity | Count | Status |
|----------|-------|--------|
| CRITICAL | 0     | pass   |
| HIGH     | 2     | warn   |
| MEDIUM   | 2     | info   |
| LOW      | 3     | note   |

**Verdict: WARNING — 2 HIGH issues should be resolved before merge.**

HIGH-1 (`corpus_override.rs` stale API) is a compile break behind the `corpus-override`
feature flag. It will fail when the `pr-4b-corpus-regression` CI job is triggered. Fix
is mechanical: migrate 2 loop sites to `result.applied_fixes()` + `fix.fix.replacement.confidence.features`.

HIGH-2 (`applied_fixes()` allocating Vec + no API contract documentation) should be
resolved before `marque-1.0` stabilizes the public accessor surface. Changing the return
type post-stabilization is a breaking change. Either switch to `impl Iterator` now or
add an explicit allocation warning to the doc comment.

MEDIUM-1 (`data-model.md` stale type signatures) can be addressed in a follow-up commit
to D10 without blocking merge if the PM decides to proceed on the HIGH resolution alone.

MEDIUM-2 (stale `[Self::applied]` doc link) is a one-line fix; recommend including it
with the `corpus_override.rs` migration commit.
