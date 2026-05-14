<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7c — Rust Specialist Pre-flight

> **STATUS UPDATE (2026-05-14)**: The `marque-mvp-3 → marque-1.0` audit-schema bump
> originally part of PR 7c scope is **REMOVED** and deferred to PR 3c.2 per D-7.18.
> The E003 retirement finding below stands — but the resolution per D-7.19 is that
> `PrecedingFixPenalty` is engine-applied at the pass-2 confidence-threshold gate, not
> rule-applied. See `pr-7-pm-decisions.md` D-7.18 through D-7.21 for full rationale.
> The branch name reflects the original scope and will be renamed for the PR 7c work.

**Scope**: T080–T085 (pre-pass-1 attrs cache, `PrecedingFixPenalty`, property/invariant
tests, `fix_10kb` bench). Audit-schema bump deferred to PR 3c.2.
**Date**: 2026-05-13

---

## Critical Finding: E003 Is Retired

**Before implementing T082**, verify which rule receives `PrecedingFixPenalty`.
`E003` (`MisorderedBlocksRule`) was retired in PR 3c.B Commit 6 and is actively
rejected at `crates/capco/src/rules.rs:3779-3780`. The PM decisions doc (D-7.10)
and `specs/006-engine-rule-refactor/tasks.md:247` both say "apply to E003" — this is
stale. Identify the surviving pass-2 `Phase::WholeMarking` rule (block ordering was
absorbed into `E060` via PR 3b.F). Align with the PM before wiring T082. **Wiring
the penalty to E003 is a no-op on a non-existent rule.**

---

## 1. `RuleContext<'a>` Lifetime Mechanics

`RuleContext` at `crates/rules/src/lib.rs:296` gains one field and one lifetime:

```rust
pub struct RuleContext<'a> {
    // ... existing fields unchanged ...
    /// Some when this marking's span overlaps a pass-1 fix (FR-023 / R-4).
    /// Rules MUST handle None — do not unconditionally unwrap.
    pub pre_pass_1_attrs: Option<&'a CanonicalAttrs>,
}
```

The `Rule` trait method adopts `'_` elision — **rule bodies do not change**:

```rust
// 7b shape:                 fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext)
// 7c shape (elision only):  fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext<'_>)
```

`'_` is inferred at each call site. The trait signature remains `trait Rule<S:
MarkingScheme>` — no generic lifetime on the trait itself. `dyn Rule<CapcoScheme>`
in the rule-set vector is unchanged; object safety is preserved.

**Variance**: `Option<&'a CanonicalAttrs>` is covariant over `'a` — no `PhantomData`
needed. `&'a T` already carries covariance.

**The 31-impl rename**: 7b's doc comment at `engine.rs:1551-1556` explicitly deferred
this. Mechanical find-replace: `&RuleContext` → `&RuleContext<'_>` at every
`impl Rule` call-site in `crates/capco/src/rules.rs`. Run `cargo check` to confirm.

---

## 2. Attrs Cache Borrow-Checker Walkthrough

**Cache shape** (prefer local variable in `run()` over a field on `TwoPassFixer` — the
field is only populated mid-run):

```rust
let cache: SmallVec<[(Span, CanonicalAttrs); 4]> =
    self.populate_pre_pass_1_cache(&pass1);  // write phase — owned output
```

**`CanonicalAttrs` size** (`crates/ism/src/canonical.rs:64`): ~14 pointer-sized fields
≈ 112 bytes on 64-bit. Inline-4 storage = ~448 bytes on the stack. Acceptable.
If a profiling pass shows pressure, switch to `SmallVec<[Box<CanonicalAttrs>; 4]>`.

**Borrow chain**:

```text
cache: SmallVec<[(Span, CanonicalAttrs); 4]>  -- owned
  ↓ &cache[i].1
RuleContext<'a>.pre_pass_1_attrs: Option<&'a CanonicalAttrs>
  -- 'a tied to the borrow of `cache`; dies when run_pass2_whole_marking returns
```

**Critical scope ordering** — write phase must complete before read phase begins, and
the `pass2_diags` SmallVec must drop before `lint.diagnostics` is moved (reproducing
the 7b `engine.rs:1838-1846` pattern):

```rust
// Correct shape inside TwoPassFixer::run():
let cache: SmallVec<[(Span, CanonicalAttrs); 4]> =
    self.populate_pre_pass_1_cache(&pass1);       // write phase done

let pass2 = {
    let (_p1, pass2_diags) = partition_diags_by_phase(...);
    self.run_pass2_whole_marking(&pass2_source, &pass2_markings,
                                 &pass2_diags, &cache, &lint)?
    // pass2_diags (SmallVec<[&Diagnostic; 32]>) drops HERE — before lint moves
};
```

`cache` is owned (no `#[may_dangle]` concern); only the `&CanonicalAttrs` references
derived from it inside `run_pass2_whole_marking` need scope-containment, and they
naturally drop when that function returns.

---

## 3. `FeatureId::PrecedingFixPenalty` Mechanics

**Add after `CorpusOverrideInEffect` at `confidence.rs:223`**:

```rust
/// A preceding pass-1 localized fix rewrote this marking before pass-2 evaluated it.
/// Reserved in `marque-mvp-3` per FR-035; filled by PR 7c.
PrecedingFixPenalty,
```

**`as_str` arm** at `confidence.rs:242` (exhaustiveness-checked by compiler):

```rust
FeatureId::PrecedingFixPenalty => "PrecedingFixPenalty",
```

**Test table** at `confidence.rs:304` (not covered by exhaustiveness — this is the
"string-without-arm" guard that must be updated manually):

```rust
(FeatureId::PrecedingFixPenalty, "PrecedingFixPenalty"),
```

**Doc comment at `confidence.rs:198`** — update to reflect the reserved-slot exception
(D-7.10):

```rust
/// New variants ordinarily require a coordinated bump of `MARQUE_AUDIT_SCHEMA`.
/// PR 3c reserved a slot for `PrecedingFixPenalty` in `marque-mvp-3`, so adding
/// that one variant in PR 7 does not require a schema bump. ANY OTHER variant does.
```

**Penalty application** (engine-side at the pass-2 confidence-threshold gate, per D-7.19):

Rule implementations should identify the correct rule and emit `base_confidence`
unchanged. The `PrecedingFixPenalty` mutation is then applied by the engine during
pass 2 threshold evaluation when preceding-fix context is present; do **not** apply
this in rule-side matching/scoring or via the stale E003 path.

```rust
// Rule-side scoring returns `base_confidence` unchanged.
//
// Engine-side pass-2 threshold gate applies the penalty, e.g.:
let final_confidence = if ctx.pre_pass_1_attrs.is_some() {
    Confidence {
        rule: base_confidence.rule * 0.90,   // -10% per D-7.10 / D-7.19
        features: smallvec![FeatureContribution {
            id: FeatureId::PrecedingFixPenalty, delta: -0.10 }],
        ..base_confidence
    }
} else {
    base_confidence
};
```

---

## 4. Audit-Schema: NOT Bumped in PR 7c

Per D-7.10 and FR-035, `PrecedingFixPenalty` fills a reserved slot — no schema bump.
The following files must be LEFT UNCHANGED in this PR:

| File | Pinned value |
|------|-------------|
| `crates/engine/build.rs:24-25` | `ACCEPTED = &["marque-mvp-3"]`; `DEFAULT = "marque-mvp-3"` |
| `crates/engine/tests/audit_schema_accept_list.rs:42` | verbatim `["marque-mvp-3"]` string |
| `crates/engine/src/lib.rs:87` | `AUDIT_SCHEMA_VERSION` via `env!` |
| `crates/engine/src/lib.rs:98` | `AUDIT_SCHEMA_IS_V3` const |

The `build_rs_accept_list_pinned` test (`audit_schema_accept_list.rs:35`) will catch
an accidental schema flip. The `AUDIT_SCHEMA_IS_V3` const rename (to `AUDIT_SCHEMA_IS_V1_0`
or deletion) is deferred to the PR 3c.2 `marque-mvp-3 → marque-1.0` bump.

---

## 5. Proptest Strategies

**FR-022 (`two_pass_invariants.rs`, new file)** — span non-overlap under random fix ordering:

```rust
proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(256))]
    #[test]
    fn pass1_pass2_spans_never_overlap(
        spans in prop::collection::vec(
            (0usize..100usize).prop_flat_map(|start| {
                (start+1..start+20).prop_map(move |end| (start, end))
            }),
            1..5usize
        )
    ) {
        // assign first half to pass-1, second half to pass-2
        // assert: for all (s1, s2), s1.end <= s2.start || s2.end <= s1.start
    }
}
```

**FR-023 (`two_pass_invariants.rs`)** — discrete cases are cleaner than proptest here:

```rust
#[test] fn i19_same_rule_no_refire() { /* same RuleId → pass-2 suppressed */ }
#[test] fn i19_different_rule_fires() { /* different RuleId → pass-2 fires */ }
#[test] fn i19_no_overlap_fires_normally() { /* no pass-1 fix on span → pass-2 normal */ }
```

---

## 6. Criterion Bench (`fix_10kb.rs`)

**Placement**: `crates/engine/benches/fix_10kb.rs` (per D-7.11; add `[[bench]]` in
`crates/engine/Cargo.toml`).

```rust
fn bench_fix_10kb(c: &mut Criterion) {
    let mut group = c.benchmark_group("fix_10kb");
    group.measurement_time(std::time::Duration::from_secs(10));
    let engine = make_engine(); // HOIST outside b.iter() — AhoCorasick build is expensive

    let no_p1 = make_10kb_input_no_localized_fixes();
    group.bench_function("pass2_only", |b| b.iter(||
        engine.fix(criterion::black_box(&no_p1), FixMode::Apply).unwrap()));

    let with_p1 = make_10kb_input_with_localized_fixes();
    group.bench_function("two_pass", |b| b.iter(||
        engine.fix(criterion::black_box(&with_p1), FixMode::Apply).unwrap()));

    group.finish();
}
criterion_group!(benches, bench_fix_10kb);
criterion_main!(benches);
```

**Gate**: `p95 ≤ 16 ms` (SC-001) AND `p99 ≤ pre_pr7c_baseline.p99 × 1.05` (D-7.11).
Baseline capture: `cargo bench --bench fix_10kb -- --save-baseline pre-pr7c` on
`origin/staging` before opening the PR.

---

## 7. Fix-Invariants Test Shapes (`fix_invariants.rs`)

One test per invariant — sketch the assertion shape:

**I-1** (replacement from canonical renderer): assert no `TextCorrection` fix has an
empty replacement when its span is non-zero.

**I-2** (no document bytes in audit stream): serialize `result.applied` to NDJSON;
assert the document typo text does not appear verbatim in the serialized output.

**I-4** (pass-2 sees post-pass-1 attrs): C001 corrects typo in pass-0; a classification
rule that would fire on the typo should NOT fire in `result.remaining_diagnostics`.

**I-18** (pass-1 ∩ pass-2 spans = ∅): partition `result.applied` by rule phase;
assert pairwise `s1.end ≤ s2.start || s2.end ≤ s1.start` for all cross-phase pairs.

**I-19** (no retroactive false positive): input with a Localized defect that pass-1
corrects; assert no same-`RuleId` diagnostic reappears in `remaining_diagnostics` for
the same span.

---

## 8. Anti-Pattern Reminders (Based on 7b Panel)

- **No clones to satisfy borrow checker.** Pass `&cache[i].1` directly into
  `RuleContext.pre_pass_1_attrs`. Use inner scope blocks to drop conflicting borrows
  before a subsequent move (7b HIGH-1 pattern, `engine.rs:1838-1846`).
- **Remove `#[allow(dead_code)]` on `pass2_rule_indices`** (`engine.rs:226`). PR 7c
  switches pass-2 to positive-whitelist dispatch — this field is finally read.
- **No `unwrap()` on `pre_pass_1_attrs`** in production code. The field is `Option`
  by architectural contract; unconditional unwrap is a misimplemented rule.
- **No new `_`-prefixed dead bindings** without an explicit migration-obligation comment
  (7b HIGH-2 pattern: `_message_args` binding at `engine.rs:2702`).
- **Scope blocks for SmallVec borrows** — NLL does not help for `SmallVec<[&T; N]>`
  the same way it does for `Vec<&T>`. Failing to scope results in an error whose
  naive fix is `.clone()` — do not clone; use the block.
- **CI uses stable clippy.** Local toolchain is nightly; run `cargo +stable clippy
  --workspace -- -D warnings` before opening the PR (project memory: stable/nightly
  lint divergence is a known trap).

---

## Summary Table

| Item | File:location | Key risk | §  |
|------|---------------|----------|----|
| `RuleContext<'a>` + `&'_` elision | `crates/rules/src/lib.rs:296` | 31-impl rename; object safety | 1 |
| Cache scope shape | `crates/engine/src/engine.rs` (TwoPassFixer::run) | SmallVec borrow scope | 2 |
| `FeatureId::PrecedingFixPenalty` variant | `crates/rules/src/confidence.rs:223` | Test table at :304 not exhaustive-checked | 3 |
| **E003 retired — resolve target rule** | `crates/capco/src/rules.rs:3779` | Penalty wired to dead rule = no-op | Critical |
| No schema bump | `crates/engine/build.rs:24-25` + `src/lib.rs:98` | Accidental flip caught by accept-list test | 4 |
| `#[allow(dead_code)]` removal | `crates/engine/src/engine.rs:226` | Technical debt from 7a | 8 |
| Proptest FR-022/FR-023 | `crates/engine/tests/two_pass_invariants.rs` | Bounded ranges; discrete for I-19 | 5 |
| Criterion bench + Engine hoist | `crates/engine/benches/fix_10kb.rs` | Engine construction outside `b.iter()` | 6 |
