// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Post-PR-4b umbrella exact-state pin for the three CAPCO catalogs.
//!
//! Asserts the **exact identity** of the post-4b-F terminal-state
//! catalogs exposed by [`CapcoScheme`]:
//!
//!  * 27 [`PageRewrite`] rows returned by
//!    [`MarkingScheme::page_rewrites`], pinned as a **positional list**
//!    (row order is load-bearing for the topological scheduler — see
//!    `crates/capco/src/scheme/rewrites/mod.rs::build_page_rewrites`
//!    doc-comment; reordering would silently shift Kahn's-algorithm
//!    cohort ordering);
//!  * 10 [`ClosureRule`] rows returned by
//!    [`MarkingScheme::closure_rules`], pinned as a **positional list**
//!    (Kleene-fixpoint walk order is load-bearing — see
//!    `crates/capco/src/scheme/closure.rs::CAPCO_CLOSURE_RULES`
//!    doc-comment; the per-marking implication rows precede the
//!    RELIDO rows so the NOFORN/ORCON cones populate `working`
//!    before suppressor checks fire);
//!  * 39 [`Constraint::Custom`] row labels returned by
//!    [`MarkingScheme::constraints`], pinned as a **sorted set**
//!    (constraint evaluation order is not engine-observable; only
//!    membership matters for the bridge dispatcher).
//!
//! Each sub-assertion uses the triple-pin shape from
//! `crates/capco/tests/post_3b_registration_pin.rs`: raw-slice length
//! (catches duplicate registration), set/list cardinality (catches
//! count drift), missing/unexpected diff (catches rename-at-same-count
//! and swap-at-same-count drift — the load-bearing exact-set check).
//!
//! ## Derivation through the nine 4b sub-PRs
//!
//! The post-4b-F terminal state is the cumulative result of nine
//! sub-PRs (see `docs/plans/2026-05-19-pr4b-closeout-architect-plan.md`
//! §1.0):
//!
//! - **4b-A #426** adds [`AeaSet`] (Join + Meet); zero rewrites, zero
//!   closures, zero new rules.
//! - **4b-B #437** adds 7 lattice impls (ClassificationLattice +
//!   NatoClassLattice with Bounded halves; DissemSet / JointSet
//!   Join-only per PR #456; NatoDissemSet / RelToBlock / DeclassifyOn
//!   with both halves) plus `W004` rule (registered count 38 → 39).
//! - **4b-C #468** adds 9 declarative `PageRewrite` rows in two
//!   patterns: Pattern B FOUO eviction (2 rows per §H.8 p134); Pattern
//!   C classified-strip semantics (7 rows per §H.6 / §H.8 / §H.9 —
//!   LIMDIS / SBU / DOD UCNI promote-and-strip / DOE UCNI
//!   promote-and-strip / FOUO). PageRewrite count 14 → 23 (per the
//!   landing CLAUDE.md "14 → 23" entry). The 8th Pattern-C row
//!   `capco/sbu-nf-evicted-by-classified` was added later by #541 in
//!   the 4b-F window (see below); not counted here.
//! - **4b-D.0 #514** lands the `ClosureRule` generic + `cone_derived`
//!   surface in `marque-scheme`. Catalog count unchanged (rows declared
//!   pre-4b, runtime-activated by 4b-D.1).
//! - **4b-D.1 #517** runtime-activates the 10-row `CAPCO_CLOSURE_RULES`
//!   catalog via [`CapcoScheme::closure_rules`] override. Pattern-D
//!   `*-implies-noforn` PageRewrite rows continue to coexist as
//!   defensive belts (the closure operator is the engine path).
//! - **4b-D.2 #527** flips the hot path: `Engine::project` reads
//!   `MarkingScheme::project(Scope::Page, …)` instead of
//!   `PageContext::expected_*`. Drops `impl JoinSemilattice for
//!   CapcoMarking` and relaxes `MarkingScheme::Marking: JoinSemilattice`
//!   bound per Copilot R1 D24. **Adds 1 PageRewrite**:
//!   `capco/noforn-clears-display-only-to` in `noforn_clears.rs` per
//!   §H.8 p145 NOFORN-dominates DISPLAY ONLY axis. PageRewrite count
//!   23 → 24.
//! - **4b-D.3 #535** migrates S007 to read `ProjectedMarking` instead
//!   of `PageContext::is_solely_nato_classified`. Zero catalog-row
//!   delta.
//! - **4b-E #539** retires the [`PageContext`] expected_*/render
//!   surface (~3457-line deletion). Adds [`DisplayOnlyBlock`] Join
//!   impl, plus 4 aggregator helpers (`FgiSet::from_attrs_iter`,
//!   `NonIcDissemSet::from_attrs_iter`,
//!   `DeclassExemptionAccumulator`, `sci_controls_from_markings`).
//!   Zero rewrite / closure / constraint-row delta.
//! - **4b-F #542** retires the last `&PageContext` parameter from the
//!   lattice-fold chain. Zero direct catalog-row delta in #542
//!   itself. In the **4b-F window** the following concurrent / fix
//!   PRs landed PageRewrites: **#541** added the 8th Pattern-C row
//!   `capco/sbu-nf-evicted-by-classified` (PageRewrite count
//!   24 → 25 per §H.9 p178); **#552** added
//!   `capco/sbu-nf-supersedes-sbu` (25 → 26 per §H.9 p178); **#555**
//!   added `capco/les-nf-supersedes-les` (26 → 27 per §H.9 p185).
//!   PageRewrite count reaches the final 27.
//!
//! Plus the 8 transmutation_stubs.rs Phase-3 stubs declared pre-4b
//! that remain in `build_page_rewrites()` for declaration ordering
//! but carry `never_fires` / `noop_action` placeholder bodies. They
//! count in the structural exact-state pin per OQ-8.
//!
//! ## Why a separate test from the count assertion
//!
//! `crates/capco/tests/transmutation_rewrites.rs::scheduled_rewrites`
//! asserts the PageRewrite count == 27 (count drift). This pin
//! complements that by catching:
//!
//!  * a row renamed at the same count (e.g., `capco/nodis-implies-noforn`
//!    → `capco/no-distribution-noforn`)
//!  * a row deleted and an unrelated row added at the same count
//!  * a row reordered to a different position in `build_page_rewrites()`
//!    (positional drift — Kahn's algorithm's tie-breaker shifts)
//!
//! All three drift patterns are exactly what a refactor regression
//! should catch. The exact-set pin closes the gap that the count-only
//! pin leaves open.
//!
//! ## Drift policy
//!
//! Bumping this test requires intentional review. Do **not** silently
//! edit any of `EXPECTED_PAGE_REWRITES`, `EXPECTED_CLOSURE_RULES`, or
//! `EXPECTED_CUSTOM_CONSTRAINTS` to make CI green. If a new row is
//! added or retired, the running-count derivation comment above must
//! be updated in lock-step.
//!
//! ## Authority
//!
//! - `docs/plans/2026-05-19-pr4b-closeout-pm-decisions.md` (PM
//!   contract for this PR; OQ-RUST-2 = positional list for rewrites,
//!   OQ-8 = transmutation_stubs included);
//! - `docs/plans/2026-05-19-pr4b-closeout-architect-plan.md` §3.2
//!   (exact-state pin design);
//! - `docs/plans/2026-05-19-pr4b-closeout-rust-preflight.md` §4
//!   (drift-class taxonomy: D1 rename, D2 count, D3 type-bound,
//!   D4 dead-code-masking);
//! - Per-row §-citations live in the originating sub-PR's plan and
//!   in each catalog declaration's `citation` field.

use marque_capco::CapcoScheme;
use marque_scheme::{Constraint, MarkingScheme};
use std::collections::BTreeSet;

/// Closed list of 27 PageRewrite IDs in positional order, matching
/// the assembly sequence in `build_page_rewrites()`:
///
///   pattern_a (4) → pattern_c (8) → pattern_b (2) →
///   supersession (2) → noforn_clears (3) → transmutation_stubs (8)
///
/// Row order is load-bearing for the topological scheduler — see
/// `crates/capco/src/scheme/rewrites/mod.rs::build_page_rewrites`
/// doc-comment. Sorted-set comparison would miss a reorder that
/// silently shifts Kahn's algorithm's tie-breaking cohort order.
const EXPECTED_PAGE_REWRITES: &[&str] = &[
    // pattern_a — §H.9 / §B.3 Table 2 p21 implies-noforn (4 rows)
    "capco/nodis-implies-noforn",
    "capco/exdis-implies-noforn",
    "capco/sbu-nf-implies-noforn",
    "capco/les-nf-implies-noforn",
    // pattern_c — §H.6 / §H.8 / §H.9 classified-strip semantics (8 rows)
    "capco/limdis-evicted-by-classified",
    "capco/sbu-evicted-by-classified",
    "capco/sbu-nf-evicted-by-classified",
    "capco/dod-ucni-promotes-noforn-when-classified",
    "capco/dod-ucni-evicted-by-classified",
    "capco/doe-ucni-promotes-noforn-when-classified",
    "capco/doe-ucni-evicted-by-classified",
    "capco/fouo-evicted-by-classified",
    // pattern_b — §H.8 p134 FOUO eviction (2 rows)
    "capco/classification-evicts-fouo",
    "capco/non-fdr-control-evicts-fouo",
    // supersession — §H.9 p178 / p185 same-axis compound-supersedes-bare (2 rows)
    "capco/sbu-nf-supersedes-sbu",
    "capco/les-nf-supersedes-les",
    // noforn_clears — §H.8 NOFORN supersession (3 rows)
    "capco/noforn-clears-rel-to",
    "capco/noforn-clears-fdr-family",
    "capco/noforn-clears-display-only-to",
    // transmutation_stubs — Stage 4+ deferred Phase-3 placeholders (8 rows)
    "capco/frd-sigma-consolidates-into-rd-sigma",
    "capco/fgi-rollup-on-us-contact",
    "capco/fgi-restricted-rollup-on-us-contact",
    "capco/joint-cross-class-rollup",
    "capco/us-presence-promotes-bare-fgi-attribution",
    "capco/orcon-nato-to-us-orcon-on-us-contact",
    "capco/sbu-nf-transmutes-on-classified-contact",
    "capco/les-nf-transmutes-on-classified-contact",
];

/// Closed list of 10 ClosureRule IDs in the positional order of
/// `CAPCO_CLOSURE_RULES`. The walk order is load-bearing for the
/// Kleene-fixpoint pass: per-marking implication rows precede the
/// Trio-2 RELIDO rows so the NOFORN/ORCON cones populate `working`
/// before RELIDO's suppressor checks fire in the same Kleene
/// iteration. The doc-comment in `CAPCO_CLOSURE_RULES` carries the
/// full rationale.
///
/// Positional sequence:
///
/// - Row 1 — `noforn-if-caveated` per §B.3 Table 2 p21 (caveated → NOFORN).
/// - Rows 2 through 7 — per-marking implications per §H.4 SCI per-system
///   rows (HCS-O / HCS-P-sub / SI-G / TK-BLFH / TK-IDIT / TK-KAND).
/// - Row 8 — `rel-to-usa-nato-if-nato-classification` per §H.7 p127
///   (NATO REL TO portion-level closure).
/// - Rows 9 and 10 — RELIDO closure rows per §H.8 pp 155-156
///   (RELIDO observed-unanimity for SCI-portion / US-classified-portion).
const EXPECTED_CLOSURE_RULES: &[&str] = &[
    "capco/noforn-if-caveated",
    "capco/hcs-o-implies-noforn-orcon",
    "capco/hcs-p-sub-implies-noforn-orcon",
    "capco/si-g-implies-orcon",
    "capco/tk-blfh-implies-noforn",
    "capco/tk-idit-implies-noforn",
    "capco/tk-kand-implies-noforn",
    "capco/rel-to-usa-nato-if-nato-classification",
    "capco/relido-if-sci-and-not-incompatible",
    "capco/relido-if-us-collateral-class",
];

/// Closed **sorted set** of 39 `Constraint::Custom` row names from the
/// three catalog modules. Evaluation order is not engine-observable
/// (the bridge dispatcher routes by name string); only membership
/// matters, so the sorted-set form is the correct pin shape.
///
///   - core_catalog (7): the original `Custom` rows for the 7 rules
///     whose predicate body did not fit `Conflicts` / `Requires` /
///     `Supersedes`.
///   - class_floor_catalog (27): the PR 3b.D + 3b.E class-floor
///     family per §H.4 / §H.6 / §H.7 / §H.8 / §H.9. Includes the
///     four `passthrough-*` stubs for tokens not yet wired into a
///     class-level predicate.
///   - sci_per_system_catalog (5): the PR 3b.E SCI per-system family
///     per §H.4 (HCS-O / HCS-P-NOFORN / HCS-P-sub / SI-G / TK-comp).
///
/// Total: 7 + 27 + 5 = 39. Note: the four RELIDO E054-E057 rows are
/// `Constraint::Conflicts`, NOT `Custom` — they do not appear here.
const EXPECTED_CUSTOM_CONSTRAINTS: &[&str] = &[
    // core_catalog (7)
    "E010/HCS-system-constraints",
    "E012/dual-classification",
    "E014/joint-requires-rel-to-coverage",
    "E021/aea-requires-noforn",
    "E024/rd-precedence",
    "E038/nodis-or-exdis-requires-noforn",
    "capco/joint-requires-usa",
    // class_floor_catalog (27)
    "E058/CNWDI-classification-floor",
    "E058/DOD-UCNI-classification-ceiling",
    "E058/DOE-UCNI-classification-ceiling",
    "E058/SAR-classification-floor",
    "class-floor/ATOMAL",
    "class-floor/BALK",
    "class-floor/BOHEMIA",
    "class-floor/EYES-ONLY",
    "class-floor/FRD",
    "class-floor/FRD-SG",
    "class-floor/HCS-comp",
    "class-floor/HCS-comp-sub",
    "class-floor/IMCON",
    "class-floor/ORCON",
    "class-floor/RD",
    "class-floor/RD-SG",
    "class-floor/RSEN",
    "class-floor/RSV-comp",
    "class-floor/SI",
    "class-floor/SI-comp",
    "class-floor/TFNI",
    "class-floor/TK",
    "class-floor/TK-BLFH",
    "class-floor/passthrough-BUR",
    "class-floor/passthrough-HCS-X",
    "class-floor/passthrough-KLM",
    "class-floor/passthrough-MVL",
    // sci_per_system_catalog (5)
    "sci-per-system/HCS-O-companions",
    "sci-per-system/HCS-P-NOFORN",
    "sci-per-system/HCS-P-sub-companions",
    "sci-per-system/SI-G-companions",
    "sci-per-system/TK-compartment-NOFORN",
];

#[test]
fn post_pr_4b_declares_exact_27_page_rewrites_in_order() {
    let scheme = CapcoScheme::new();
    let rewrites = scheme.page_rewrites();

    // Raw-slice cardinality — catches duplicate registration that a
    // set-equality check would silently collapse.
    let raw_len = rewrites.len();
    assert_eq!(
        raw_len, 27,
        "post-4b PageRewrite slice length drifted from 27: raw_len={raw_len}"
    );

    // Positional comparison — load-bearing because the topological
    // scheduler in `marque_engine::scheduler` breaks ties on
    // declaration order. A row reorder that the sorted-set check
    // would silently absorb would shift Kahn's-algorithm output.
    let actual: Vec<&str> = rewrites.iter().map(|r| r.id).collect();
    let expected: Vec<&str> = EXPECTED_PAGE_REWRITES.to_vec();

    assert_eq!(
        expected.len(),
        27,
        "EXPECTED_PAGE_REWRITES does not contain 27 entries: \
         test data drifted, not the catalog"
    );

    // Exact positional check — the load-bearing assertion.
    if actual != expected {
        // Compute set-difference for a more helpful error message —
        // distinguishing "renamed" from "reordered" drift.
        let actual_set: BTreeSet<&str> = actual.iter().copied().collect();
        let expected_set: BTreeSet<&str> = expected.iter().copied().collect();
        let missing: Vec<&str> = expected_set.difference(&actual_set).copied().collect();
        let unexpected: Vec<&str> = actual_set.difference(&expected_set).copied().collect();
        panic!(
            "post-4b PageRewrite positional list drifted.\n\
             Missing (expected but not registered): {missing:?}.\n\
             Unexpected (registered but not expected): {unexpected:?}.\n\
             If both diffs are empty, the rows were *reordered* — \
             row order is load-bearing for the topological scheduler. \
             Bumping this test requires intentional review.\n\n\
             Actual order:   {actual:?}\n\
             Expected order: {expected:?}"
        );
    }
}

#[test]
fn post_pr_4b_declares_exact_10_closure_rules_in_order() {
    let scheme = CapcoScheme::new();
    let rules = scheme.closure_rules();

    let raw_len = rules.len();
    assert_eq!(
        raw_len, 10,
        "post-4b ClosureRule slice length drifted from 10: raw_len={raw_len}"
    );

    let actual: Vec<&str> = rules.iter().map(|r| r.name).collect();
    let expected: Vec<&str> = EXPECTED_CLOSURE_RULES.to_vec();

    assert_eq!(
        expected.len(),
        10,
        "EXPECTED_CLOSURE_RULES does not contain 10 entries: \
         test data drifted, not the catalog"
    );

    if actual != expected {
        let actual_set: BTreeSet<&str> = actual.iter().copied().collect();
        let expected_set: BTreeSet<&str> = expected.iter().copied().collect();
        let missing: Vec<&str> = expected_set.difference(&actual_set).copied().collect();
        let unexpected: Vec<&str> = actual_set.difference(&expected_set).copied().collect();
        panic!(
            "post-4b ClosureRule positional list drifted.\n\
             Missing: {missing:?}.\n\
             Unexpected: {unexpected:?}.\n\
             If both diffs are empty, the rows were reordered — \
             the Kleene-fixpoint walk order is load-bearing per \
             `CAPCO_CLOSURE_RULES` doc-comment (per-marking cones \
             must populate `working` before RELIDO suppressor \
             checks). Bumping this test requires intentional \
             review.\n\n\
             Actual order:   {actual:?}\n\
             Expected order: {expected:?}"
        );
    }
}

#[test]
fn post_pr_4b_declares_exact_39_custom_constraints() {
    let scheme = CapcoScheme::new();
    let constraints = scheme.constraints();

    // Filter to `Constraint::Custom` variants only. The other
    // constraint kinds (`Conflicts`, `ConflictsWithFamily`,
    // `Requires`, `Supersedes`) are pinned by their own catalog
    // tests in `crates/capco/tests/scheme_constraints_*.rs` and are
    // not in scope here.
    //
    // Triple-pin: (1) raw count of filtered `Constraint::Custom`
    // entries before any deduplication, (2) BTreeSet size after
    // deduplication, and (3) raw_count == set_size — the equality
    // assertion catches the duplicate-name drift Copilot R1 flagged:
    // a duplicate `Constraint::Custom("capco/foo", ...)` row would
    // dedupe in the BTreeSet and the size-only assertion would pass
    // silently. Raw-count-equals-set-size is the load-bearing dedup
    // check.
    let custom_names: Vec<&str> = constraints
        .iter()
        .filter(|c| matches!(c, Constraint::Custom { .. }))
        .map(|c| c.name())
        .collect();
    let raw_count = custom_names.len();
    let actual: BTreeSet<&str> = custom_names.iter().copied().collect();

    let expected: BTreeSet<&str> = EXPECTED_CUSTOM_CONSTRAINTS.iter().copied().collect();

    assert_eq!(
        expected.len(),
        39,
        "EXPECTED_CUSTOM_CONSTRAINTS does not contain 39 unique entries: \
         test data drifted, not the catalog"
    );

    assert_eq!(
        raw_count, 39,
        "post-4b Constraint::Custom raw catalog count drifted from 39 \
         (5 SCI-per-system + 27 class-floor + 7 core-catalog): \
         raw_count={raw_count}, names={custom_names:?}"
    );

    assert_eq!(
        raw_count,
        actual.len(),
        "post-4b Constraint::Custom catalog contains duplicate label \
         (raw_count={raw_count} but unique set size={set_size}). \
         A duplicate label would mask drift under set-only assertions; \
         the raw-count-equals-set-size invariant rejects it. \
         names={custom_names:?}",
        set_size = actual.len()
    );

    assert_eq!(
        actual.len(),
        39,
        "post-4b Constraint::Custom unique set size drifted from 39: \
         actual={actual:?}"
    );

    let missing: Vec<&str> = expected.difference(&actual).copied().collect();
    let unexpected: Vec<&str> = actual.difference(&expected).copied().collect();
    assert!(
        missing.is_empty() && unexpected.is_empty(),
        "post-4b Constraint::Custom label set drifted.\n\
         Missing (expected but not registered): {missing:?}.\n\
         Unexpected (registered but not expected): {unexpected:?}.\n\
         Bumping this test requires intentional review; do not \
         silently edit EXPECTED_CUSTOM_CONSTRAINTS to make CI green."
    );
}
