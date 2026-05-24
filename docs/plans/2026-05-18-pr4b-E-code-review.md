# Code Review: PR 4b-E — PageContext `expected_*` / Renderer Deletion + 5 New Lattice Helpers

**Reviewer**: rust-reviewer agent  
**Branch**: `refactor-006-pr-4b-d-0-closure-rule-generic`  
**Date**: 2026-05-18  
**Scope**: Read-only. PM triages and acts on findings.

---

## Executive Summary

PR 4b-E is largely well-executed: the 17-method `PageContext::expected_*` surface is cleanly retired, citation fidelity is high across the board, G13 compliance is maintained, Constitution VII §IV authorization is sound, and the 3 pre-PR-4b-E parity divergences all converged as required. The parity gate, proptest coverage, send/sync assertions, and documentation updates are all present and correct.

**Two HIGH findings block PR open:**

1. `DeclassExemptionLattice` implements `JoinSemilattice` while explicitly documenting a non-commutative `join` — a direct violation of the trait contract.
2. `DisplayOnlyBlock` and `DeclassExemptionLattice`, the two new `JoinSemilattice` implementors introduced in this PR, have no proptest algebraic-law coverage, leaving the commutativity violation undetected by the test suite.

All other findings are MEDIUM or below and do not block.

---

## CRITICAL

*None.*

---

## HIGH

### H-1 — `DeclassExemptionLattice` violates `JoinSemilattice` commutativity law

**File**: `crates/capco/src/lattice.rs`, lines 3455–3473  
**Trait contract**: `crates/scheme/src/lattice.rs:57` — "Implementors must satisfy the three join laws: commutativity, associativity, and idempotency."

The `join` implementation explicitly documents the violation:

```rust
// Commutative? NO — last-observed is order-sensitive by design.
// We expose this only as a `JoinSemilattice` because the
// operator we need is associative + idempotent over our
// construction path (only `from_attrs_iter` builds non-bottom
// values, and that path is intrinsically order-preserving).
// Production composition routes through `from_attrs_iter`,
// never through repeated `join` calls; this impl is here for
// type-system symmetry with sibling lattices.
```

The "type-system symmetry" justification does not resolve the contract violation. Any caller who obtains two `DeclassExemptionLattice` values from different origins and joins them — for example, a future `PageRewrite` walking `Scope::Page` intermediate values, a test that constructs fixtures via `join`, or a second-scheme adapter — will observe `a.join(&b) != b.join(&a)` without any static warning from the type system. The existing `proptest_lattice.rs` suite would catch this immediately if `DeclassExemptionLattice` were included.

The rationalization that "production composition routes through `from_attrs_iter`" is a behavioral invariant enforced only by naming convention, not by types. `from_attrs_iter` is a safe public constructor; `join` is also public and satisfies all syntactic preconditions; callers have no machine-checkable reason to prefer one over the other.

**Fix options** (in order of preference):

- **(a) Remove `impl JoinSemilattice for DeclassExemptionLattice`.** The `from_attrs_iter` + `into_inner` / `as_inner` surface is sufficient for all current consumers. Remove the `JoinSemilattice` impl. The `CapcoScheme::project` composition path calls `from_attrs_iter` directly; it does not need `join`. This is the cleanest option.
- **(b) Change semantics to max-priority-wins (commutative).** The doc comment contains a `TODO` about "longest duration" semantics. A commutative operator (e.g., highest-priority exemption by CAPCO §E.2 precedence, or per the existing `DeclassifyOnLattice` `MaxDate` model) satisfies the trait. Requires a §-citation justification.
- **(c) Weaken the `JoinSemilattice` trait doc.** Drop the commutativity requirement. This would break the algebraic foundation relied upon by `SciSet`, `SarSet`, `RelToBlock`, and `DisplayOnlyBlock`. Not recommended.

Option (a) is preferred. The impl was added "for type-system symmetry" — the correct answer to type-system symmetry with a non-commutative structure is not to implement a commutative trait.

---

### H-2 — `DisplayOnlyBlock` and `DeclassExemptionLattice` lack proptest algebraic-law coverage

**File**: `crates/capco/tests/proptest_lattice.rs`

The existing `proptest_lattice.rs` covers `SciSet`, `SarSet`, `FgiSet`, and `RelToBlock` with commutativity, idempotency, and associativity suites. PR 4b-E introduces two new `JoinSemilattice` implementors — `DisplayOnlyBlock` and `DeclassExemptionLattice` — with no proptest coverage of the algebraic laws. Only hardcoded happy-path tests in the `lattice.rs` `#[cfg(test)]` module are present.

This gap is directly load-bearing: a proptest commutativity suite for `DeclassExemptionLattice` would immediately catch H-1 above (`a.join(&b) != b.join(&a)` for `Some(X)` vs `Some(Y)`). The absence of this test allowed the violation to survive the test suite undetected.

Additionally, `FgiSet::from_attrs_iter` (new constructor in this PR) is not covered by the existing `fgi_join_commutative` proptest, which uses `FgiSet::from_markings`. The two constructors should produce equivalent outputs for equivalent inputs; a proptest roundtrip would verify this.

**Fix**: Add proptest law suites to `proptest_lattice.rs` for `DisplayOnlyBlock` and `DeclassExemptionLattice`. Extend the `FgiSet` proptest suite with a `from_attrs_iter` path. These additions are also the direct remediation for H-1: if option (a) is taken (remove the `JoinSemilattice` impl), the `DeclassExemptionLattice` proptest would need to target its `from_attrs_iter` idempotency property instead of `join` laws.

---

## MEDIUM

### M-1 — `crates/capco/src/lattice.rs` at 4933 lines exceeds project guidance by 6×

**File**: `crates/capco/src/lattice.rs`  
**Project guidance**: 200–400 lines typical; 800 lines maximum (CLAUDE.md, coding-style rules).

The file was already large before this PR. PR 4b-E adds approximately 770 lines (helpers at lines 3160–3727 plus tests at lines 4445–4695). This is a pre-existing condition that the PR worsens materially.

The five new helpers and their tests would split cleanly into a sibling module — for example, `crates/capco/src/lattice/page_helpers.rs` — without changing any public API surface. The existing test suite in `lattice.rs::tests` (lines 4445–4695) could move to `crates/capco/tests/lattice_helpers.rs` in the same pass.

This finding does not block PR open on its own but is worth a follow-up issue before the file grows further in PR 4b-F or Stage 4.

---

## LOW

### L-1 — Stale `PageContext::expected_*` references in historical doc comments

**Files**:
- `crates/engine/tests/closure_hotpath.rs`, lines 447, 456, 495
- `crates/capco/src/render/render_dissem.rs`, lines 33, 139
- `crates/capco/src/scheme/rewrites/pattern_c.rs`, line 86
- `crates/capco/src/scheme/rewrites/noforn_clears.rs`, line 156
- `crates/capco/src/scheme.rs`, line 306

These are doc comments and inline explanatory comments that name the now-retired `expected_*` methods for historical context (e.g., "previously this path called `expected_rel_to`"). They are not live API calls and will not cause compile failures. Verified by grep: no live call sites remain.

They may confuse future maintainers who search for `expected_` and find these remnants. Low-priority cleanup pass suggested after PR 4b-E merges.

---

## INFO

### I-1 — `generate_cab_native` dual-accumulator semantic asymmetry is by design but inconsistent with `DeclassExemptionLattice`

**File**: `crates/wasm/src/lib.rs`, `generate_cab_native`

The CAB generator uses `found_declass_exemption` (first-wins) semantics internally while `DeclassExemptionLattice::from_attrs_iter` uses last-observed semantics. The comment at the call site documents this as intentional ("preserves pre-PR-4b-E semantics exactly"). G13 compliance is intact — no exemption content reaches the audit stream, diagnostic messages, or logs.

The inconsistency is not a correctness problem today, but creates a semantic fork: a future second WASM consumer of `DeclassExemptionLattice` would get last-observed behavior silently differing from the existing CAB path. The planned `CabProjection` PR should rationalize these two paths and pick a canonical semantic with a §-citation (§E.1 p31 or equivalent). No action needed in PR 4b-E.

---

## PASS — Items Confirmed Clean

All of the following were audited and pass without findings:

| Area | Verdict |
|------|---------|
| Constitution VII §IV engine-touch authorization | PASS — all deletions are within-006 bugfix precedent; no `marque-engine` rule logic edited |
| Citation fidelity (all `§X.Y pNN` spot checks) | PASS — 10+ citations verified against `crates/capco/docs/CAPCO-2016.md` `begin page`/`end page` anchors |
| G13 content-ignorance in `generate_cab_native` | PASS — exemption content stays in local `String` variables, never reaches audit stream |
| Walk-adjacent-paths: `page_rewrite.rs`, `builtins.rs`, `projected.rs` | PASS — stale `expected_rel_to` / `expected_declassify_on` references removed |
| README.md, CLAUDE.md, CAPCO-CONTEXT.md documentation updates | PASS — all updated consistently with PR 4b-E content |
| 3 pre-PR-4b-E parity divergences converged (G-3, joint_unanimous_two_portions, joint_single_portion_no_us) | PASS — all three fixtures assert convergence in both lattice and scheme paths |
| 12 `dissem_us` divergence fixtures | PASS — correctly annotated with `expected_divergences = &["dissem_us"]` and `§B.3 Table 2 p21` reasoning |
| `send_sync.rs` static assertions | PASS — `assert_impl_all!(PageContext: Send, Sync)` and `assert_impl_all!(CanonicalAttrs: Send, Sync)` correctly added |
| `sar_sort_key` relocation to `crates/ism/src/sar_sort.rs` | PASS — clean move; `§H.5 p99–100` citation correct; 4 unit tests including overflow saturation |
| `page_context.rs` shim surface | PASS — 275 lines; 6 coverage tests present; doc comment correctly notes PR 4b-E retirement |
| `page_context_to_attrs` dead code | PASS — fully deleted with explanatory comment |
| `DisplayOnlyBlock` citation (`§H.8 p163`, `§D.2 Table 3 rows 18-20/25-27`) | PASS — verified |
| `FgiSet::from_attrs_iter` citations (`§H.7 p122/p123/p128`) | PASS — verified |
| `NonIcDissemSet` intentionally NOT implementing `JoinSemilattice` | PASS — correct; doc comment explains classification-gate dependency |
| `compute_banner_native` migration to `scheme.render_banner(scheme.project(Scope::Page, ...))` | PASS — clean |

---

## Review Summary

| Severity | Count | Status |
|----------|-------|--------|
| CRITICAL | 0     | pass   |
| HIGH     | 2     | block  |
| MEDIUM   | 1     | warn   |
| LOW      | 1     | note   |
| INFO     | 1     | note   |

**Verdict: BLOCK** — H-1 (`DeclassExemptionLattice` violates `JoinSemilattice` commutativity) and H-2 (missing proptest coverage for new lattice impls) must be resolved before PR open. Both are fixable with localized changes to `lattice.rs` and `proptest_lattice.rs` that do not require architectural revision.
