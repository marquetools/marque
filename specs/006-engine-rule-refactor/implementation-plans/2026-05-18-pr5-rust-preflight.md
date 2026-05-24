<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 5 — Rust-specialist preflight (foreign banner correctness)

Status: Pre-implementation, branch `refactor-006-pr-5-foreign-banner-correctness`
off `origin/staging` HEAD `7d4ad231` (post-PR-6c).

## TL;DR

The literal targets named in T059/T060 (`expected_classification()`,
`scheme.rs:365` hardcode, `page_context_to_attrs`) **no longer exist** — they
were retired in PR 4b-E (#539) and PR 6c (#547). PR 5 is therefore **not the
deletion PR the task plan describes**. The substantive bugs (#276 foreign
banner roll-up; #261 redundant `FGI` token drop) survive in different shapes
and need new, smaller surgery against the lattice path
(`CapcoMarking::join_via_lattice_body` at `crates/capco/src/scheme/marking.rs:225-349`)
and the existing `crates/capco/src/render/render_fgi.rs::render_fgi`.

Workspace `cargo +stable check --workspace` is clean.
`cargo +stable clippy --workspace --all-targets -- -D warnings` is clean.

## 1. Surviving `MarkingClassification::Us(_)` site classification

Total references workspace-wide: **190** (`grep -rn`). After excluding
`/tests/` directories the count is 62 production-code lines. After excluding
doc-comment-only mentions the count is 62 lines across 18 production files.

The classification table below is exhaustive over the 18 production files. The
only sites that are **production-code re-projections / hardcodes** (the
analogues of the retired `scheme.rs:365`) live in
`crates/capco/src/scheme/marking.rs:311-348`. Every other production site is a
**correct discriminator** (variant match asking "is this US?"), an
**axis-flatten by §H.7 reciprocal-classification rule** (correct), or a
documented per-axis renderer dispatch.

### 1.1 Hardcode-during-projection sites (PR 5's actual targets)

**`crates/capco/src/scheme/marking.rs:311-348`** — `CapcoMarking::join_via_lattice_body`,
the post-PR-4b-D.2 hot-path projection function. Five distinct re-projections
to `Us(_)`:

| Line | Variant | Justification on the books | PR 5 question |
|------|---------|------|---------------|
| 316 | `Joint(j)` → `Us(j.level)` (non-JOINT branch) | §H.3 p57 "JOINT does not roll up in US documents" — comment at 313-314 | **Correct**. Non-JOINT branch means `JointSet::to_marking_classification()` returned `None` (`Bottom` or `Mixed`), so the §H.3 p57 flatten applies. Keep. |
| 322-324 | `Nato(n)` → `Us(n.us_equivalent())` (gated `!solely_non_us`) | §H.7 pp123-125 reciprocal-raise — comment at 318-321 | **Correct** when `!solely_non_us`, **but `solely_non_us` is defined by counting `Us(_)` + `Conflict` + `Joint(_)` as US-bearing**. This is exactly what #276 reports — a page with only `Nato(_)` + `Joint(USA, GBR)` portions sets `solely_non_us = false` (the JOINT side counts as US-bearing per G-9b's §H.3 p56 reading), so the NATO portion is reciprocal-raised to `Us(_)`. Whether that's correct depends on whether the architect's PM-resolved reading of §H.7 vs §H.3 keeps JOINT as US-bearing for the reciprocal gate. **This is the load-bearing question.** |
| 327 | `Fgi(f)` → `Us(f.level)` (gated `!solely_non_us`) | Same as above | Same answer as line 322-324. Identical gating. |
| 340 | `Conflict { us, .. }` → `Us(*us)` (always) | §H.7 pp123-125 — comment at 329-338 | **Correct** by design. `Conflict` already records "US wins" semantically; flatten is structural. Keep. |

The "hardcode" framing from the original T059/T060 task plan does not capture
what's happening here. These are **deliberate §H.7 reciprocal-normalization
projections**, each with a verified citation. The question is whether the
**`solely_non_us` gate predicate** is correct — specifically whether `Joint(_)`
should count toward `has_us_class` on line 276. That's the substantive #276
decision point and belongs in the strategic plan, not in T059's "delete the
hardcode" framing.

There is **no fallback-defaulting site** in production code where the engine
emits `Us(...)` when classification is unknown. The `Option<MarkingClassification>`
contract is preserved end-to-end (the parser only emits `Some(Us(...))` when
it actually parsed a US classification token).

### 1.2 Correct discriminator sites (NOT PR 5 targets — keep)

All sites below pattern-match `Some(MarkingClassification::Us(_))` as a
**check** (does this marking carry a US axis?), not as a **construction**.

- `crates/core/src/parser.rs:364` — parser records the parsed US token. Correct.
- `crates/core/src/parser.rs:687, 758, 4350` — FGI marker / foreign-classification
  detection gated on "marking already carries US axis". Correct.
- `crates/engine/src/recognizer.rs:168` — `is_us_restricted` helper used to
  reject the nonsensical `Us(Restricted)` shape. Correct.
- `crates/capco/src/rules.rs:2058` — E007/E008-class rule classifying "is this
  banner classified" across all four classification variants. Correct.
- `crates/capco/src/scheme/predicates/satisfies.rs:499` — `collect_present_tokens`,
  per-variant token emission. The `Us | Conflict` arm intentionally emits
  nothing (US is implicit; foreign axes emit explicit FGI/NATO/JOINT tokens).
- `crates/capco/src/scheme/predicates/spans.rs:43` — `us_level()` accessor;
  returns `None` for pure FGI/NATO/JOINT. Correct.
- `crates/capco/src/scheme/actions/fgi.rs:70` — `extract_foreign_sources`;
  US-only → empty list. Correct.
- `crates/capco/src/render/render_classification.rs:83` — per-axis dispatch on
  variant. Correct.
- `crates/ism/src/dissem_attribution.rs:125` — dissem reciprocity router; US
  axis → `DefaultOrigin::Us`. Correct (PR 9b T132).
- `crates/ism/src/canonical.rs:155` — `us_classification()` convenience accessor.
  Correct.
- `crates/capco/src/lattice.rs:1244, 1295, 1468` — variant-rank / same-variant
  join+meet logic (Lattice axiom support). Correct.
- `crates/capco/src/lattice.rs:1767` — `ClassificationLattice::top()` =
  `Us(TopSecret)`. This is the meet bound for the lattice. **Audit point**:
  meet of `Nato(NatoSecret)` with `Us(TopSecret)` falls through
  `classification_meet_same_variant` (line 1457) which returns `None` for
  cross-variant pairs, producing lattice bottom. So the `top()` definition is
  not load-bearing in practice for foreign cases. Keep.
- `crates/capco/src/lattice.rs:2727` — `JointSet::DisunityCollapse` →
  `Us(highest_level)`. Documented as "non-US producers ride to FgiSet
  separately" per §H.3 p57. Correct.

### 1.3 Default-fallback sites

**None found.** Every production site that emits `Some(Us(...))` does so either
because the parser observed a US classification token OR because of a documented
§H.7/§H.3 reciprocal-normalization rule. The "silently default to US" failure
mode #276 describes does not exist as a literal hardcode in the post-PR-6c
codebase; the failure is structural in the `solely_non_us` gate's reading of
`has_us_class`.

## 2. `FgiSet` trait surface audit

Located at `crates/capco/src/lattice.rs:494-655`.

### 2.1 Shape

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum FgiSet {
    #[default] None,
    Present { concealed: bool, countries: BTreeSet<CountryCode> },
}
```

`#[non_exhaustive]` is already set (B-4 from PR 4b-B 8th-pass).

### 2.2 Trait impls

- `JoinSemilattice` (line 557-588) — concealed dominates, otherwise union.
  Correct.
- `MeetSemilattice` (line 590-655) — concealed acts as top, otherwise
  intersect. Correct.
- `Lattice` — **inherited via blanket impl** from PR #456:
  `impl<T: JoinSemilattice + MeetSemilattice> Lattice for T {}`. Confirmed by
  inspection.
- `BoundedJoinSemilattice` / `BoundedMeetSemilattice` / `BoundedLattice` — **NOT
  implemented**. Per lattice-design memory: "agency-extensible open sets, so no
  lawful finite `top` exists" applies to SCI/SAR; for FGI the country code
  space is also open-vocab (Appendix A trigraphs + future tetragraphs). This
  is **correct** — do not add a `BoundedLattice` impl.
- `Send + Sync` — `BTreeSet<CountryCode>` is `Send + Sync` and the rest is
  primitive; `FgiSet` is auto-`Send + Sync`. Confirmed.

### 2.3 T061 extension point

T061 (per tasks.md:207) calls for `FgiSet::render_canonical` to drop the
redundant `FGI` token when a trigraph is present per §H.7. This is **wrong as
literally stated** — `render_canonical` is a trait method on `MarkingScheme`
(`crates/scheme/src/scheme.rs:577`), not a method on a per-axis lattice type.
The actual extension point is one of:

1. **A new `FgiSet`-local helper** invoked from
   `crates/capco/src/render/render_fgi.rs::render_fgi` (line 50). The current
   renderer prints `FGI` followed by the country list (lines 55-86) and does
   not drop the `FGI` token when trigraphs are present. T061's #261 concern is
   that the redundant `FGI` literal appears when the classification axis
   already conveys "this is FGI" (e.g., `//GBR S` already starts with the
   trigraph). The dispatch needs to be aware of what `render_classification`
   wrote out — which is the `MarkingClassification::Fgi(_)` axis case at
   `render_classification.rs:84` calling its own `render_fgi` (line 103, a
   different function).
2. **The cross-axis coordination point** is the per-axis dispatch loop in
   `marking_scheme_impl.rs::render_canonical` (`RENDER_TABLE`). If the
   classification axis already emitted `//GBR S` (the FGI-as-classification
   form), the FGI-marker axis should suppress its own redundant `FGI` literal.

**Recommended Rust shape** (subject to the architect's strategic call):

- Either add `fn FgiSet::render_canonical_into(&self, classification_emitted_fgi: bool, out: &mut dyn fmt::Write) -> fmt::Result`
- Or migrate the suppression into `render_fgi.rs::render_fgi` by passing a flag
  from the dispatch loop.

Neither option requires a `MarkingScheme` trait-surface change — both are
crate-local in `marque-capco`.

## 3. Type-shape change risk

PR 5's task plan does not introduce a new top-level type (no
`ForeignBannerKind` enum). The plan is purely about:

- T059/T060 — already vacuous post-PR-6c (the literal targets are gone).
- T061 — internal `FgiSet` rendering; no public API change.
- T062/T063/T063a — corpus fixtures.
- T064 — CI regression-grep guard.

**Risk** the architect MAY introduce: a new `ForeignBannerKind` or
`solely_non_us` shape change inside `join_via_lattice_body`. If introduced:

- Affects: `marque-capco` only. The lattice path lives entirely inside that
  crate.
- Crosses `#[doc(hidden)] pub` semver-unstable boundary: **no**. The scheme
  trait surface (`MarkingScheme`, `Vocabulary<S>`, `Codec<S>`) carries no
  `#[doc(hidden)]` annotations — these are publicly stable. Adding a new
  trait method is a breaking change to anyone outside the tree, but per the
  consolidated plan §3.10 the trait surface is intentionally unstable
  pre-1.0; an additive method is in-bounds.
- Touches `RuleContext`: **no**. PR 6c already wired `page_marking:
  Option<Arc<ProjectedMarking>>` (`crates/rules/src/lib.rs:499`). PR 5
  consumes that field; it does not need to add new fields.
- `FgiSet` field changes: any extension (e.g., adding a `Partial` variant) is
  in-bounds because `FgiSet` is already `#[non_exhaustive]`.

## 4. Constitution VII §IV engine-crate touch

Does PR 5 need to edit `marque-engine`, `marque-scheme`, `marque-core`,
`marque-rules`, or `marque-ism`?

**Best case (the architect lands T059/T060 as no-ops)**: No engine-crate
touches. Only `marque-capco` (scheme/marking.rs, render/render_fgi.rs,
lattice.rs) + corpus fixtures + regression-grep. **Recommended**: this is the
clean path. No Constitution VII §IV authorization needed.

**Worst case (T061 requires render-trait-surface change)**: Touches
`marque-scheme` only if `MarkingScheme::render_canonical` needs a new
parameter. Within-006 precedent for trait-surface edits exists (PR 4b-B/4b-C/4b-D.2/4b-D.3/4b-E/4b-F/6c
all touched engine crates). All those precedents are **bugfix-class deletions
in `marque-ism`** or **new-mechanism with PM addendum** — adding a parameter
to `render_canonical` is new mechanism, not bugfix. **Would require explicit
PM authorization.**

**Recommended posture**: Land T061 as a `marque-capco`-internal change
(option 2 in §2.3 — pass the flag from the dispatch loop). This stays inside
the scheme-adoption boundary that Constitution VII §IV protects.

If the architect's plan calls for an engine-crate touch (T064's grep guard
file lives at `.github/workflows/ci.yml` + `tools/regression-grep/regression-grep.sh`,
which are not engine crates so they're outside §IV's scope), surface the
specific touch in the architect's strategic plan with the precedent justification.

## 5. Clippy stable drift risk

Per memory `feedback_clippy_nightly_vs_stable_drift`: local clippy is nightly
0.1.97; CI is stable. Patterns to avoid:

- **Doc comments with `+ ` at line start** trip `doc list item without
  indentation` on stable. The PR 5 work touches `render_fgi.rs` doc comments
  in §A.6 references — these already use indented `-` bullets, no `+ `
  pattern. Safe.
- **`clippy::const_is_empty`** fires on stable for `const X: bool = "".is_empty()`-shape
  patterns. PR 5 has no const-bool need; safe.
- **`clippy::or_fun_call`** can fire on `.or_else(|| f())` vs `.or_else(f)`.
  The existing `render_fgi.rs` and `marking.rs` already pass `cargo +stable
  clippy --workspace --all-targets -- -D warnings` (verified by this preflight).

**Recommendation**: Run `cargo +stable clippy --workspace --all-targets --
-D warnings` locally before pushing any code change. The stable proxy works.

## 6. Performance considerations

PR 5's hot path is `lint_10kb` (SC-001 ceiling: p95 ≤ 16 ms). Baseline at last
PR was ~828 µs with 10% threshold ~911 µs (memory
`project_bench_baseline_staleness`).

### 6.1 `FgiSet::render_canonical` overhead

Renderer is **not** on `lint`'s hot path. `lint` runs the rule loop, which calls
`scheme.project(Scope::Page, ...)` once per page; the renderer runs only when
`fix` (or banner-validation rules) generate output. **No `lint_10kb` impact.**

### 6.2 `solely_non_us` gate predicate change

If PR 5 modifies the `has_us_class` accumulation loop at `marking.rs:270-282`,
the loop is O(portions) and runs once per page projection. Each iteration is
a 4-arm match on the classification enum — cache-friendly, branch-predictable.
A new predicate arm or an extra check adds a constant factor. **Risk: low**;
even doubling the iteration cost adds nanoseconds, not microseconds.

### 6.3 `FgiSet::from_attrs_iter` cross-axis fold (line 3197)

Already on the hot path. If PR 5 extends this with additional classification-derived
producer logic (e.g., to handle the new #276 case), the iteration is still
O(portions). Safe.

**Recommendation**: re-run `lint_10kb` Criterion locally on the final
implementation. If the bench fires within 10% of baseline, the change is safe.
If close to threshold, request a baseline refresh per memory
`project_perf_baseline_pr5_trigger` (PR 5 is the PR after which the user
commissions dedicated perf-analysis work if baselines haven't fallen back
naturally).

## 7. Test architecture risk

### 7.1 Existing foreign-related corpus

- `tests/corpus/lattice/fgi-banner-rollup.txt` (4 lines, single fixture).
- `tests/corpus/valid/classified_banner_nato.txt` (1 line, banner-only).
- `tests/corpus/valid/classified_banner_atomal_as_aea.txt` (ATOMAL).
- `tests/corpus/valid/clean_banner_non_us_restricted.txt`.

No `tests/corpus/foreign/` directory exists yet; T062/T063/T063a create it.

### 7.2 Parity-gate coverage

- `crates/capco/tests/lattice_vs_scheme_parity.rs` (2580 lines, 74+ fixtures)
  — the highest-signal cross-crate test. T062-T063a fixtures should drive
  through this gate's `project_via_lattice` ↔ `project_via_scheme` comparison
  to detect future divergence.
- `crates/capco/tests/corpus_parity.rs` (433 lines) — corpus-vs-engine
  regression harness. The post-PR-3b registration pin lives at
  `crates/capco/tests/post_3b_registration_pin.rs` (47-rule exact set).
- `crates/capco/tests/proptest_page_rollup.rs` (288 lines) — **coverage gap**:
  the `arb_ism_attrs` strategy at line 89-119 generates **only US
  classification** (line 100: `arb_classification().prop_map(|c| Some(MarkingClassification::Us(c)))`).
  No FGI / NATO / JOINT variants. PR 5 SHOULD extend this to include foreign
  variants, otherwise #276 is unreachable from proptest. Recommend a new
  `arb_classification_any_variant` strategy.

### 7.3 Recommended new tests

- T062 (`tests/corpus/foreign/pure_foreign_banner.json`) — page of `(C//FGI DEU)`
  portions. **MUST** assert banner retains `FGI DEU`, NOT a silent
  reciprocal-raise to US.
- T063 (`tests/corpus/foreign/joint_us_uk.json`,
  `tests/corpus/foreign/nato_only_page.json`).
- T063a (`tests/corpus/foreign/mixed_us_foreign_rollup.json`) — `(S//NF)` +
  `(//DEU TS//REL TO USA, DEU)` → `TOP SECRET//FGI DEU//NOFORN`. **The
  load-bearing #276 fixture.**
- A new property-test addition to `proptest_page_rollup.rs` covering FGI/NATO/JOINT
  variants in `arb_classification`.

## 8. Risk register

| # | Severity | Risk | Mitigation | Rollback |
|---|----------|------|------------|----------|
| R1 | **High** | T059/T060 vacuous — the literal targets `expected_classification()` and `scheme.rs:365` were deleted in PR 4b-E/6c. PR 5 risks landing as test-only additions without addressing #276. | Architect re-scopes T059/T060 to the **substantive `solely_non_us` gate predicate** at `marking.rs:270-282` (counting `Joint(_)` as `has_us_class`). Surfaced in architect's strategic plan. | If PR 5 ends up test-only, file follow-up issue against `marking.rs:join_via_lattice_body` for the gate predicate change. |
| R2 | **Medium** | T061 `FgiSet::render_canonical` literal naming is wrong; method belongs to `MarkingScheme` trait not `FgiSet`. Misreading triggers a Constitution VII §IV scheme-adoption boundary violation if implemented as `MarkingScheme` trait change. | Implement T061 as a `marque-capco`-internal change in `render_fgi.rs` (pass suppression flag from dispatch loop). No trait-surface edit. | Revert to single-crate render fix; trait surface stays untouched. |
| R3 | **Medium** | The `solely_non_us` gate counts `Joint(_)` as US-bearing per G-9b's §H.3 p56 reading; a misreading of #276 may want the opposite. Whichever way PR 5 changes the gate, **fixtures in `lattice_vs_scheme_parity.rs` will reveal divergence** between the lattice path and the declarative PageRewrite catalog. | Run `cargo test --test lattice_vs_scheme_parity` after any `marking.rs` edit. Document any new divergence with a §-citation per parity-gate discipline (memory: PR 4b-E §3 OQ-7 BLOCKING). | Revert `marking.rs` change; parity gate is the canary. |
| R4 | **Medium** | New rules emitted by PR 5 for #276 cases overlap with existing banner-validation walker `BannerMatchesProjectedRule` (E031/E035/E040, per PR 3b.A). Severity-config aliases retired post-3b.F, but per-row diagnostic message text is still load-bearing for tests. | If PR 5 introduces new diagnostic IDs, append to the walker's `Diagnostic.citation` field per the PR 3b.A precedent — don't add a top-level new rule unless necessary. Verify the `post_3b_registration_pin.rs` exact-set pin updates accordingly. | Drop the new rule; rely on the existing walker to surface the banner mismatch. |
| R5 | **Low** | T064 CI grep guard for `MarkingClassification::Us` hardcode re-introduction is **non-trivial** because of the 30+ correct-discriminator sites. A naive regex would block every legitimate `match`. | Anchor the regex to **construction sites only** — `classification = Some(MarkingClassification::Us(...))` or `Some(MarkingClassification::Us(...))` in a `_ => Some(...)` arm. The existing `tools/regression-grep/regression-grep.sh::guard` shape supports doc-comment exclusion already. Pin to specific files (e.g., `crates/capco/src/scheme/marking.rs`) rather than the whole tree. | Drop the guard; rely on parity-gate divergence detection. |
| R6 | **Low** | The `FgiMarker::SourceConcealed | Acknowledged` discriminant from PR 2 T094 needs careful interaction with #276 — a source-concealed page hitting the new `solely_non_us` predicate must not reciprocal-raise the concealed source to a US level (which would leak the concealed source's existence). | `FgiSet::from_attrs_iter` at `lattice.rs:3197` already preserves `has_source_concealed` correctly. PR 5 fixtures MUST include a source-concealed-on-foreign-page case. | The `concealed` flag is set-bound; misuse manifests at the renderer (`render_fgi.rs:57` — `SourceConcealed` writes only `FGI`), which is caller-visible immediately. |

## 9. Open questions for PM (Rust-shape only)

1. **`solely_non_us` gate semantics**: should `Joint(_)` count toward
   `has_us_class` (current PR 4b-B G-9b behavior, citing §H.3 p56's "USA is in
   the producer list") or toward `has_non_us_class` (the #276-driven reading
   that JOINT shouldn't suppress NATO reciprocal preservation)? This is the
   load-bearing predicate change for PR 5.

2. **T061 implementation surface**: confirm that "`FgiSet::render_canonical`"
   in tasks.md is a typo for "logic that suppresses the redundant `FGI` token
   inside `render_fgi.rs`" (option 2 in §2.3) — and that the engine-crate
   boundary (Constitution VII §IV) is respected by keeping the fix
   `marque-capco`-internal.

3. **Default-fallback site policy**: there are **zero** default-`Us` fallback
   sites in production code. Should T064's grep guard nonetheless block
   future re-introduction of the pattern `_ => Some(MarkingClassification::Us(...))`,
   or pin to specific files (`marking.rs`)?

4. **Proptest extension**: should `arb_classification` in
   `proptest_page_rollup.rs` be extended to include FGI/NATO/JOINT variants
   in PR 5, or deferred to a follow-up?

## Appendix A — Files surveyed

Production code (no `/tests/`, no doc-comment-only): 18 files, 62 lines.

```
crates/capco/src/lattice.rs                              13 lines
crates/capco/src/render/render_classification.rs          2 lines
crates/capco/src/rules.rs                                 1 line
crates/capco/src/scheme/actions/fgi.rs                    1 line
crates/capco/src/scheme/closure.rs                       18 lines (ALL in #[cfg(test)] modules at L1067/L1204/L1656/L2279)
crates/capco/src/scheme/marking.rs                        5 lines  ← §1.1 PR 5 question lives here
crates/capco/src/scheme/predicates/satisfies.rs           3 lines (1 production + 2 in #[test])
crates/capco/src/scheme/predicates/spans.rs               1 line
crates/capco/src/scheme/tests.rs                          1 line (file is test-mod)
crates/core/src/parser.rs                                 5 lines (3 production + 2 in #[cfg(test)])
crates/engine/benches/profile_project.rs                  2 lines (bench, not lint)
crates/engine/src/decoder.rs                              3 lines (all in #[test] modules)
crates/engine/src/engine.rs                               1 line (in #[cfg(test)] helper)
crates/engine/src/recognizer.rs                           1 line
crates/ism/src/attrs.rs                                   1 line (in #[test])
crates/ism/src/canonical.rs                               1 line
crates/ism/src/dissem_attribution.rs                      3 lines (1 prod + 2 in #[test])
crates/ism/src/projected.rs                               3 lines (all in #[test])
```
