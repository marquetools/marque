<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7 Rust Review — Phase-Tagged Pass Split

**Branch**: `refactor-006-pr-7-phase-tagged-pass-split`
**Tasks in scope**: T073–T085
**Reviewer**: rust-reviewer agent
**Date**: 2026-05-13
**Pre-flight**: `cargo check` green, `clippy -D warnings` clean, `fmt --check` clean, all tests pass.

---

## Diagnostic Status

No CRITICAL or HIGH issues are pre-existing in the reviewed files. The observations below are all forward-looking — they describe where PR 7's implementation will land if the naive path is taken, and where each one becomes a trap.

---

## 1. Ownership and Lifetime Analysis

### 1.1 `RuleContext.pre_pass_1_attrs: Option<&CanonicalAttrs<'src>>`

This is the central lifetime question for PR 7.

`CanonicalAttrs<'src>` borrows into the **original** source `&[u8]`. After pass 1 splices corrections into `effective_source` (a newly allocated `Vec<u8>` in `apply_text_corrections`), the post-pass-1 buffer is a separate allocation. If `pre_pass_1_attrs` holds a `&CanonicalAttrs<'src>` where `'src` names the lifetime of the original `source: &[u8]` passed into `fix_inner`, then:

- The cache itself (a `SmallVec<[CanonicalAttrs<'src>; 4]>`) must be allocated inside `fix_inner` and live through the entire pass-2 dispatch.
- Pass-2 dispatches against the **post-pass-1 buffer** (`&effective_source`), which is a local `Vec<u8>`. Its `CanonicalAttrs<'post>` borrow has a different, shorter lifetime.
- `RuleContext.pre_pass_1_attrs: Option<&CanonicalAttrs<'src>>` therefore borrows from the **stack-local cache**, not from `source` directly.

Concretely the signature must be something like:

```rust
// The cache lives on fix_inner's stack for the duration of pass-2 dispatch.
let pre_pass_1_cache: SmallVec<[CanonicalAttrs<'_>; 4]> = ...;
// RuleContext borrows into the cache, not into the original source bytes.
ctx.pre_pass_1_attrs = Some(&pre_pass_1_cache[i]);
```

This is fine — the cache and the `RuleContext` both die at the end of `fix_inner` — but the lifetime annotation in T080 (`Option<&CanonicalAttrs<'src>>`) names the wrong lifetime. `'src` should be the lifetime of the **cache allocation on the stack**, not the source buffer. If the implementer writes the field as `Option<&'src CanonicalAttrs<'src>>` and tries to satisfy it by pointing into `parsed_markings1` (the original-pass parsed markings), there will be a borrow conflict: `parsed_markings1` is inside a conditional branch (`if !pass1_applied.is_empty()`) that may have already been consumed into `lint` by the time pass-2 needs it (`crates/engine/src/engine.rs:1356`).

**Recommendation**: Do not name the lifetime `'src` in the field type. Use an anonymous lifetime or a named `'ctx` lifetime scoped to the `RuleContext` borrow, and document that it names the cache — not the source buffer — in the field doc comment. The spec task (T080) should be read as "a reference to pre-pass-1 canonical attrs" not "a reference bearing the source-buffer lifetime."

### 1.2 `SmallVec<[CanonicalAttrs<'src>; 4]>` cache inline sizing

T080 specifies inline-4. The cache is populated only when a marking's span overlaps a pass-1 fix span. Pass-1 dispatches `Phase::Localized` rules only; C001 (corrections-map) is the dominant source of pass-1 fixes in practice. Real documents with marking density ≤ 5/KB will produce at most 1–3 pass-1 fixes in the 10 KB SC-001 benchmark document. Inline-4 is appropriate and will not heap-allocate in the p95 case.

However, the cache is indexed by canonical-attrs position, not by fix index. A document with N markings where K of them overlap pass-1 fixes has a cache of size K, not N. Confirm the implementation populates by iterating markings and checking overlap — not by pre-allocating N entries.

### 1.3 `Phase` enum in `Box<dyn Rule<S>>`

`Phase` as specified is `Copy` (two fieldless variants). The `phase(&self) -> Phase` method returns by value, no borrow involved. No lifetime or ownership issue here — a per-impl `const` method is the zero-cost idiom.

---

## 2. Trait-Surface Ergonomics

### 2.1 `fn phase(&self) -> Phase` — required vs defaulted

**Recommendation: default to `WholeMarking`, not required.**

The current `Rule<S>` trait (`crates/rules/src/lib.rs:962`) defaults `additional_emitted_ids` rather than requiring it, for exactly this reason: 47 existing rules shouldn't need to be touched for a feature that only matters for a few. Requiring `phase()` with no default means every rule in `crates/capco/src/rules.rs` needs a `fn phase(&self) -> Phase { Phase::WholeMarking }` stub before the build compiles — 47 boilerplate adds that serve no semantic purpose.

`Phase::WholeMarking` is the correct default: it means "I don't make pass-1 promises, evaluate me at full marking scope." A `Phase::Localized` declaration is an affirmative, span-constraining claim that requires a deliberate addition.

The drift risk (a rule author forgets to declare `Phase::Localized` and their rule ends up in pass 2 instead) is **lower-consequence** than the usability cost of requiring 47 adds: a `WholeMarking` default means a missing declaration runs the rule in pass 2, which is the safe conservative choice. A wrongly-declared `Phase::Localized` that passes span validation would be caught by T075's `EngineConstructionError::PhaseSpanShapeMismatch` check.

### 2.2 `Phase` enum location

Define `Phase` in a new `crates/rules/src/phase.rs` and re-export it from `crates/rules/src/lib.rs`. Rationale:

- `lib.rs` is already 1095 lines. `Phase` is a stable, leaf type with no dependencies on other `lib.rs` items. Isolating it avoids the "add a doc comment and touch half the file" problem.
- `FeatureId` and `MessageTemplate` both live in separate modules (`confidence.rs`, `message.rs`) for the same reason — the pattern is established.
- A separate file makes the compile-fail doctests for `Phase` (if any are warranted) natural to add later.

### 2.3 Interaction with `additional_emitted_ids`

The consolidated plan §9.1 explicitly forbids a single rule entry emitting different IDs at different phases. The `additional_emitted_ids` mechanism was added for the walker pattern (one registered `id()` emitting multiple row-specific diagnostic rule IDs). These two mechanisms compose cleanly — each registered rule has one `Phase`, and if a walker emits under multiple IDs in its single phase, `additional_emitted_ids` covers that. There is no conflict.

The case that does NOT compose: if someone tries to register a walker at both `Phase::Localized` and `Phase::WholeMarking` behind one struct. §9.1 forbids this at the design level; the two-entry-sharing-a-module pattern is the right shape. The trait surface does not need to prevent this programmatically — the `PhaseSpanShapeMismatch` check at registration (T075) catches span violations, and §9.1's two-entry discipline is a code-review gate, not a type-system gate.

### 2.4 Twin-struct ergonomics for rules needing both phases

The twin-struct pattern (two zero-size structs sharing a backend module, each implementing `Rule<S>` with its own `phase()` and potentially different `id()`) has one ergonomic trap: if both structs' `check` methods call the same backend and the backend is parameterized on phase, a `match phase` inside the backend will re-run work the registration-check contract already guaranteed. Keep the backends genuinely separate: pass-1 backend emits only localized fixes; pass-2 backend reads `pre_pass_1_attrs` when available and emits whole-marking diagnostics. Do not share the body via a `pub fn check_impl(attrs, ctx, phase: Phase) -> Vec<Diagnostic<S>>` that branches internally — that reconstructs the escape hatch §9.1 banned.

---

## 3. `EnginePromotionToken` + R002 Emission

### 3.1 R002 emission shape

R001 is synthesized in `build_decoder_diagnostic` (`crates/engine/src/engine.rs:2083`). That function is a standalone `fn` that takes explicit `(span, canonical_bytes, confidence, ...)` and returns `Option<Diagnostic<CapcoScheme>>`. R002 should follow the same shape: a standalone `fn build_r002_diagnostic(pass1_fix_ids: &SmallVec<[RuleId; 4]>, failure_span: Span) -> Diagnostic<CapcoScheme>`.

Do not create a generic `build_synthetic_diagnostic` abstraction yet. R001 and R002 have structurally different args (R001 carries decoder confidence; R002 carries contributing rule IDs), and the abstraction surface would require a generic args type that adds complexity without a third client to justify it. YAGNI applies here; the two parallel functions are maintainable.

### 3.2 R002 does not need `__engine_promote` at diagnostic-creation time

R002 is a `Diagnostic`, not an `AppliedFix`. It is emitted into `LintResult.diagnostics` like any other diagnostic — the promotion path is not involved. `__engine_promote` is only called if R002's diagnostic has a `fix: Some(FixIntent<S>)` that the engine chooses to apply. Per §9.4 of the consolidated plan, R002 carries no fix — it is informational (the engine returns the pass-1 buffer; no further fix is auto-applied). So R002 construction bypasses the promotion seal entirely; no token needed.

The `const R002_RULE_ID = RuleId::new("engine/r002.reparse-failed")` should live in `crates/engine/src/engine.rs` alongside `DECODER_RULE_ID` (line 51). The consolidated plan §9.4 says "centralizing the synthetic-engine-diagnostic IDs into `marque-rules` is a separate refactor not in scope for this plan." Respect that scope boundary — do not move `DECODER_RULE_ID` in this PR.

Note on T078's `RuleId("engine", "r002.reparse-failed")` notation: the current `RuleId` is a single-string newtpe (`pub struct RuleId(&'static str)`). The 2-tuple form has not landed. Use `RuleId::new("R002")` or `RuleId::new("engine/r002.reparse-failed")` (flat string) consistent with how `DECODER_RULE_ID = "R001"` is declared today. Do not brace-construct the 2-tuple form that does not exist yet.

### 3.3 `contributing_pass1_fix_ids: SmallVec<[RuleId; 4]>` inline sizing

Inline-4 is appropriate. The re-parse failure path is the error path; a document triggering it has at most K pass-1 fixes, and K is typically ≤ 3 for the C001-dominated correction workload. Even in adversarial inputs with many corrections, the failures are bounded by the number of distinct rule IDs that contributed (not the total count of fixes applied per rule), making inline-4 a safe bound for the p95 case.

---

## 4. `Engine::fix_inner` Restructure Shape

### 4.1 Current size and PR 7 additions

`fix_inner` is currently lines 1309–1644 = 335 lines. PR 7 adds:
- Pre-pass-1 attrs cache population (T080): ~20 lines
- Pass-2 dispatch filtered by phase (T081): ~40 lines
- I-18 overlap guard for pass-2 (T081): ~20 lines
- I-19 pre-pass-1 re-validation check (T081): ~25 lines
- R002 emission path (T077): ~15 lines
- Phase-1/2 split of the existing fix loop: structural refactor of ~100 lines

Conservative estimate: PR 7 would push `fix_inner` to ~550 lines. That is beyond the 50-line function limit by 11x and well past sustainable.

**Recommendation: extract a `TwoPassFixer` struct.**

```rust
struct TwoPassFixer<'engine> {
    engine: &'engine Engine,
    source: &'engine [u8],
    mode: FixMode,
    threshold: f32,
    deadline: Option<Instant>,
}

impl<'engine> TwoPassFixer<'engine> {
    fn run(self) -> Result<FixResult, EngineError> { ... }
    fn run_pass1(&self, ...) -> Pass1Result { ... }
    fn run_pass2(&self, pass1: &Pass1Result, ...) -> Pass2Result { ... }
    fn build_r002(&self, pass1: &Pass1Result, failure_span: Span) -> Diagnostic<CapcoScheme> { ... }
}
```

`fix_inner` becomes a 5-line trampoline:

```rust
fn fix_inner(&self, source: &[u8], mode: FixMode, threshold: f32, deadline: Option<Instant>)
    -> Result<FixResult, EngineError>
{
    TwoPassFixer { engine: self, source, mode, threshold, deadline }.run()
}
```

This is not purely cosmetic. It makes each phase independently testable without going through the full fix pipeline, which the property tests at T083/T084 will want.

### 4.2 Reconciling the existing C001 two-pass with the new phase-split

Three options were flagged in the review prompt. Analysis:

**Option (c) — C001 corrections stay where they are; new pass-1/pass-2 operate on the post-corrections buffer** is correct. Rationale:

- The existing C001 two-pass exists because `SERCET//NF` requires a spelling correction before the scanner can detect it at all. This is a scanner-visibility problem, not a rule-phase problem. It is logically prior to rule dispatch.
- `Phase::Localized` rules (typo fixes, token-form corrections that don't cross token boundaries) are semantically analogous to C001 but rule-registered, not config-driven. They operate on a buffer the scanner has already parsed.
- Making C001 "pass-0" before the phase-1/phase-2 split is architecturally correct and already describes what the current code does. The naming just makes it explicit.

Concretely: the three-level pipeline becomes:
1. **Pass-0** (existing): apply C001 text corrections → produce `effective_source`.
2. **Pass-1** (PR 7 new): dispatch `Phase::Localized` rules against `effective_source`; apply their fixes via single-pass forward splice → produce `post_pass1_source`.
3. **Re-parse**: `parse(post_pass1_source)` → if fails, emit R002, return `post_pass1_source`.
4. **Pass-2** (PR 7 new): dispatch `Phase::WholeMarking` rules against `post_pass1_source`; apply with I-18 non-overlap and I-19 re-validation.

Options (a) and (b) are problematic: (a) adds a third re-parse cycle for the common case where C001 corrections are empty (wasted work); (b) merges two semantically distinct repair channels (user-config byte-level replacement vs. rule-emitted structural intent) into a single dispatch bucket, complicating provenance tracking.

---

## 5. Performance

### 5.1 SC-001 budget exposure from the new re-parse

The current `fix_inner` does one re-parse: the lint of `effective_source` at line 1358. PR 7 adds a second re-parse of `post_pass1_source` between pass-1 and pass-2.

From the `lint_latency` Criterion benchmark baseline: a lint pass on 10 KB inputs currently runs at p50 ≈ 2–4 ms (the scanner + parser combined; rule dispatch adds ~1 ms for 47 rules). Two additional lint passes therefore add ~4–8 ms to the `fix` path.

The SC-001 budget is p95 ≤ 16 ms for `lint`, not `fix`. But `fix` inherits the same user-visible latency contract for interactive use (IDE), and the `fix_10kb` bench (T085) needs to stay within SC-008's budget (FR-032 cites the same 16 ms threshold).

The risk is not that a single re-parse is expensive; it is that the hot path (no pass-1 fixes → no re-parse needed) must remain fast. Confirm the implementation short-circuits: if `pass1_applied.is_empty()`, skip the re-parse entirely and run pass-2 directly on `effective_source`. This is the same optimization already present for the C001 case at `crates/engine/src/engine.rs:1356`.

### 5.2 Pre-pass-1 attrs cache scope

The spec (T080) says "populate per-marking only when the marking's span overlaps a pass-1 fix span." If the implementation instead populates for all markings (using `parsed_markings1` as-is), the cache becomes O(markings) rather than O(fixes). On a dense 10 KB document with 30 markings and 2 pass-1 fixes, the waste is a factor of 15x in cache entries. The `SmallVec` inline-4 guard protects the heap allocation case but not the O(30) vs O(2) work case.

This matters for SC-001: the pre-pass-1 cache population should be a single linear scan over `pass1_applied` spans, intersected against the markings list via binary search (markings are span-sorted after the scanner pass). O(K log N) where K = pass-1 fixes, N = markings. Not O(N).

### 5.3 T085 `fix_10kb` benchmark shape

Suggested Criterion structure:

```rust
fn fix_10kb(c: &mut Criterion) {
    let mut group = c.benchmark_group("fix");
    // Baseline: document with no pass-1 fixes triggered (measures pass-0 + pass-2 only).
    group.bench_function("fix_10kb_pass2_only", |b| {
        b.iter(|| engine.fix(black_box(&source_no_corrections), FixMode::Apply))
    });
    // With pass-1 fixes: document with known-mangled tokens triggering Phase::Localized rules.
    group.bench_function("fix_10kb_two_pass", |b| {
        b.iter(|| engine.fix(black_box(&source_with_corrections), FixMode::Apply))
    });
    // Assert in a separate #[test] (not in bench): both p95 ≤ 16 ms.
    group.finish();
}
```

The two-bench split is important: it isolates the pass-1/pass-2 overhead from the baseline, making regressions in either path visible separately. Do not collapse into one bench that mixes both — "two-pass overhead within SC-008 budget" (T085's criterion) requires seeing the delta.

---

## 6. Idiomatic Rust Concerns

### 6.1 `FeatureId::PrecedingFixPenalty` — exhaustiveness and test table

Adding this variant requires two coordinated updates:

1. `FeatureId::as_str` match arm in `crates/rules/src/confidence.rs:241` — the `const fn as_str` match is exhaustive, so the build fails until the arm is added. The compiler catches this.

2. The `feature_id_as_str_matches_audit_contract` test table at `crates/rules/src/confidence.rs:304`. This is a **manual add** — the exhaustiveness check does not reach into the test table. A new `FeatureId` variant whose `as_str` label is pinned only by the match arm (and not by the test table) can have its label silently renamed in a future refactor without the test breaking. The table must be updated with `(FeatureId::PrecedingFixPenalty, "PrecedingFixPenalty")` in the same commit as the variant addition.

3. `MARQUE_AUDIT_SCHEMA` in `crates/engine/build.rs` must bump from `"marque-mvp-3"` to a new value per the `FeatureId` doc comment ("New variants MUST bump the audit schema version"). This is the blocking dependency that T082 shares with the schema bump. Verify against the tasks checklist whether PR 7 is the intended schema bump PR — if not, `PrecedingFixPenalty` cannot be added without the schema bump happening first.

### 6.2 `MessageArgs` and the R002 contributing-ids encoding

`MessageArgs.feature_ids: SmallVec<[FeatureId; 4]>` is the field available for R002's contributing-rule-id list. But `FeatureId` is the wrong type — it names decoder-evidence features (edit distance, token reorder, etc.), not rule IDs. Encoding pass-1 contributing rule IDs as `FeatureId` values would either require adding rule-ID-shaped variants to `FeatureId` (conceptually wrong — `FeatureId` is decoder-evidence, not rule provenance) or abusing the existing `CorpusOverrideInEffect` slot (wrong).

The cleaner path: R002's contributing rule IDs are carried as a separate field on `R002Diagnostic` — a struct internal to `marque-engine`, not a `Diagnostic<CapcoScheme>` at all. R002 can be emitted as a `Diagnostic<CapcoScheme>` with `MessageTemplate::ReparseFailed` and no contributing-id detail in `MessageArgs.feature_ids`, while the actual contributing IDs are logged via `tracing::warn!` (audit-content-safe — rule IDs are token canonicals, not document bytes). The `SmallVec<[RuleId; 4]>` mentioned in T077 then lives on `R002Diagnostic` (the engine-internal struct), not on a `MessageArgs` field.

This avoids the need to add a new `MessageArgs` field (which would require a `MessageArgs` struct update, a closed-set pin test update at `crates/rules/tests/message_args_closed_set.rs`, and potentially a `marque-mvp-3` audit schema audit if the field lands on the audit record). If the contributing IDs need to be in the audit record, the right place is a top-level `R002AppliedDiagnostic` type (parallel to `AppliedFix`) rather than forcing them into the `MessageArgs` closed-set field.

### 6.3 `apply_text_corrections` uses `Vec::splice` — the phase-1 path should not

`apply_text_corrections` at line 1748 still uses `buf.splice(...)` in a loop (the pre-Commit-6 forward-pass approach). This is O(N × M) on the text-corrections path. For pass-1 (PR 7's Phase::Localized dispatch), the implementer must use the forward-pass buffer construction pattern that `fix_inner`'s `FixMode::Apply` block uses (lines 1495–1526): pre-allocate with `extra` bytes, copy gaps and replacements in a single left-to-right pass. Do not copy `apply_text_corrections`'s splice loop into pass-1.

Note: `apply_text_corrections`'s splice is acceptable for C001 because C001 handles ≤5 corrections per document in practice and the input is already the original `source` bytes. Pass-1 in PR 7 could in principle have more Phase::Localized fixes across a dense document, making the O(N × M) behavior a latency risk on the critical path.

### 6.4 No new `unsafe` expected

The pre-existing codebase has `#![forbid(unsafe_code)]` at `crates/rules/src/lib.rs:5`. PR 7 does not touch unsafe code. No `unsafe` blocks are expected or warranted. If any appear, they are a defect.

---

## 7. `cargo +stable clippy` Proxy Concerns

Per the `feedback_clippy_nightly_vs_stable_drift` memory note: local nightly clippy diverges from CI stable clippy. Patterns the implementer should pre-emptively guard against:

- **`clippy::const_is_empty` (stable fires, nightly may not)**: Any `if slice.is_empty() {}` on a `&'static []` in a `const fn` context. Applicable to `additional_emitted_ids` default (`&[]`). The existing default returns `&[]`, which stable clippy may flag. Check before assuming local nightly is the CI signal.

- **`clippy::match_single_binding`**: If `phase(&self) -> Phase` is implemented as `match self { Self => Phase::WholeMarking }` (single-arm match on a ZST) stable clippy will flag this. Use a direct return.

- **`clippy::missing_const_for_fn`**: The `phase()` method on zero-size rule structs is eligible for `const fn` (no runtime state). Stable clippy may suggest this. Either add `const` (cleaner) or add `#[allow(clippy::missing_const_for_fn)]` with a rationale comment.

- **`clippy::too_many_arguments`**: Already present on `__engine_promote` with an `#[allow]` annotation. The `TwoPassFixer::run_pass1` / `run_pass2` methods may approach the argument count threshold if they accept all the deadline / mode / threshold parameters inline. The struct shape proposed in §4.1 sidesteps this.

- **`clippy::large_enum_variant`**: The `R002Diagnostic` internal struct (if placed in an enum alongside other engine-internal diagnostics) will trigger this if it carries a `SmallVec` inline. Keep `R002Diagnostic` as a plain struct, not a variant.

---

## 8. What Would Make Us Regret This in 5 Years

### 8.1 `Phase` as a rule-property instead of a fix-property

The spec tags the **rule** with a phase. But a rule's span shape depends on the fixing intent, and different `FixIntent` variants produce different span shapes (a `Recanonicalize` spans the full marking; a `FactRemove` spans a single token). If a rule can emit both a `FactRemove` (localized) and a `Recanonicalize` (whole-marking) — which is legal under the current `FixIntent` design — tagging the rule as `Phase::Localized` would mismatch the `Recanonicalize` span.

The `PhaseSpanShapeMismatch` check at T075 catches this at registration time, which prevents silent bugs. But it means some rules that could emit localized fixes are forced to declare `WholeMarking` because they also emit a whole-marking fix somewhere. In 5 years, when there are 100 rules and 8 of them want to participate in pass-1, you will want per-intent phase tagging (`FixIntent::phase: Phase`), not per-rule. The current design is not wrong, but it is not the final shape. Document this explicitly so the PR 7 implementer does not design the twin-struct workaround in a way that makes the per-intent migration harder.

### 8.2 `RuleId` as `&'static str` when R002 needs scheme-qualified IDs

The consolidated plan §9.4 specifies `("engine", "r002.reparse-failed")` as the R002 identifier. The current `RuleId` is a single `&'static str` (`crates/rules/src/lib.rs:96`). Using `RuleId::new("R002")` is fine for now. Using `RuleId::new("engine/r002.reparse-failed")` encodes the 2-tuple as a slash-separated string — a convention that is not enforced anywhere. In 5 years, when the 2-tuple form lands, every site that encoded `"engine/r002.reparse-failed"` as a flat string will need a migration. Use `RuleId::new("R002")` (consistent with `DECODER_RULE_ID = "R001"`) and leave the scheme-qualified naming to the deferred refactor. Do not prematurely encode the 2-tuple convention in a string.

### 8.3 Pre-pass-1 cache holding `CanonicalAttrs` by value

T080 specifies `SmallVec<[CanonicalAttrs<'src>; 4]>`. `CanonicalAttrs` is the pivot type for rule dispatch — it is non-trivially sized (multiple `Box<[T]>` fields for SCI markings, SAR markings, etc., per `crates/ism/src/lib.rs`). Storing these by value in a `SmallVec` on the stack means copying the full `CanonicalAttrs` payload for each overlapping marking. The `Box<[T]>` fields clone shallowly (they copy the pointer + length, not the heap data), but the struct copy itself is still non-trivial.

In 5 years, when the parse cache holds `Arc<CanonicalAttrs>` (likely if the incremental LMDB cache at v0.2 materializes), the cache would naturally hold `Arc` references and the copy cost drops to an atomic refcount. The current design force-copies because `parsed_markings1` is a `Vec<CanonicalAttrs>` (not `Arc`-wrapped), and the cache needs to outlive the original `parsed_markings1` binding. If the implementer uses `Arc<CanonicalAttrs>` in `parsed_markings` today (or `&CanonicalAttrs` borrows from a stable cache), the pre-pass-1 cache can store references instead of copies.

Concrete recommendation: define the pre-pass-1 cache as `SmallVec<[Arc<CanonicalAttrs>; 4]>` if `parsed_markings` is changed to `Vec<Arc<CanonicalAttrs>>` (which it should be, given the `BatchEngine` and the `synthesize_fixes` function both read from it after construction). Do not force a value copy for a temporary cache.

---

## Approval Assessment

**Status: WARN (no CRITICAL or HIGH; 1 HIGH-adjacent and several MEDIUM)**

The implementation surface does not exist yet — this is pre-implementation review. The items above are design-time traps, not code defects. The single item closest to HIGH:

- **Section 1.1**: The `'src` lifetime annotation in T080 names the wrong lifetime and will produce a borrow-conflict or a conceptual error if taken literally. Must be resolved before the cache implementation lands.

All other items are MEDIUM: implementation shape choices where the naive approach produces correct but unsustainable code (§4.1 `fix_inner` size), a schema-bump dependency that blocks T082 (§6.1), and a type mismatch between `FeatureId` and rule-ID encoding for R002 (§6.2).

**Gate items before merging PR 7**:
1. Confirm the `'src` lifetime annotation on `pre_pass_1_attrs` names the cache lifetime, not the source-buffer lifetime (§1.1).
2. Confirm `FeatureId::PrecedingFixPenalty` is accompanied by a `MARQUE_AUDIT_SCHEMA` bump, or confirm it is deferred to a later PR (§6.1).
3. Confirm `fix_inner` is refactored (§4.1) before the PR is opened — not as a follow-up.
4. CHK015 and CHK018 gates from `checklists/correctness.md` §2.
