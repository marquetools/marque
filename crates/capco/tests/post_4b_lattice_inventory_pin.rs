// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Post-PR-4b umbrella exact-state pin for the three CAPCO catalogs.
//!
//! Asserts the **exact identity** of the post-4b-F terminal-state
//! catalogs exposed by [`CapcoScheme`]:
//!
//!  * 30 [`PageRewrite`] rows returned by
//!    [`MarkingScheme::page_rewrites`], pinned as a **positional list**
//!    (row order is load-bearing for the topological scheduler — see
//!    `crates/capco/src/scheme/rewrites/mod.rs::build_page_rewrites`
//!    doc-comment; reordering would silently shift Kahn's-algorithm
//!    cohort ordering). #559 close-out (2026-05-19) added two
//!    RELIDO-eviction rewrites converting the retired E056 / E057
//!    Conflicts rows (27 → 29); #618 added the deferred E055 DISPLAY
//!    ONLY > RELIDO row once `satisfies(TOK_DISPLAY_ONLY)` was widened
//!    to recognize the canonical `display_only_to` parser axis (29 → 30);
//!  * 1 [`ClosureRule`] row returned by
//!    [`MarkingScheme::closure_rules`] (post-PR-D of the FactBitmask
//!    refactor, issue #371). The 10-row catalog walk that PR 4b-D.1
//!    runtime-activated was retired in PR-D: nine rows had cones in
//!    the closed-vocab atom inventory and migrated to the bitmask
//!    Kleene fast path (`CLOSURE_TABLE` in `closure_table.rs`); only
//!    `CLOSURE_REL_TO_USA_NATO` survives in fn-pointer form because
//!    its `cone_derived` open-vocab NATO tetragraph has no
//!    closed-vocab bit. The 10-row bitmask catalog gets a parallel
//!    positional pin against `CLOSURE_TABLE` here — the broader
//!    drift-catch property that the original 10-row `ClosureRule`
//!    list provided now lives on the bitmask side;
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
//!   Join-only per the `Lattice` trait split (issue #456 / PR #502);
//!   NatoDissemSet / RelToBlock / DeclassifyOn with both halves) plus
//!   `W004` rule (registered count 38 → 39).
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
//!   bound. **Adds 1 PageRewrite**:
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
use marque_scheme::{Constraint, MarkingScheme, Severity};
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
    // relido_clears — #559 close-out (2026-05-19) + #618: §H.8
    // RELIDO eviction by DISPLAY ONLY / ORCON / ORCON-USGOV at page
    // scope (3 rows). Retired the E055 / E056 / E057
    // `Constraint::Conflicts` rows whose portion-scope intent could
    // not fire on cross-portion supersession. Authority: §H.8 p154
    // (DISPLAY ONLY), §H.8 p136 (ORCON), §H.8 p140 (ORCON-USGOV).
    // The DISPLAY ONLY row was deferred behind #618 until
    // `satisfies(TOK_DISPLAY_ONLY)` was widened to recognize the
    // canonical `display_only_to` parser axis.
    "capco/display-only-clears-relido",
    "capco/orcon-clears-relido",
    "capco/orcon-usgov-clears-relido",
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

/// Closed list of the residual ClosureRule IDs from
/// `CAPCO_CLOSURE_RULES` after issue #704's architectural refinement.
/// All fn-pointer rules retired — the four "default if absent" rules
/// (caveated → NOFORN; NATO → REL TO USA, NATO; SCI → RELIDO;
/// US-class → RELIDO) relocated to `crate::scheme::default_fill`
/// because they are non-monotone by §-design and cannot live in a
/// closure operator that honors the `MarkingScheme::closure` monotone
/// contract. The six per-marking unconditional rows (HCS-O, HCS-P[sub],
/// SI-G, TK-BLFH, TK-IDIT, TK-KAND) live in `CLOSURE_TABLE`'s bitmask
/// form. Net: the fn-pointer trait surface is empty post-#704.
const EXPECTED_CLOSURE_RULES: &[&str] = &[];

/// Closed list of 6 `ClosureRow` names in the positional order of
/// `marque_capco::closure_table::CLOSURE_TABLE`. Issue #704 trimmed
/// the pre-#704 10-row catalog to the 6 per-marking unconditional
/// implications from §H.4 marking templates; Rows 0/7/8/9 (the four
/// "default if absent" rules) relocated to
/// `crate::scheme::default_fill`. Walk order is no longer load-bearing
/// at the closure layer (the retired chain dependency between Row 3
/// SI-G→ORCON and Row 0 ORCON→NOFORN now crosses the
/// close()/default_fill boundary; default_fill snapshots the
/// post-close bitmask once and evaluates all four default-fill
/// predicates against it).
///
/// Authority per row:
/// - HCS-O / HCS-P[sub] → NOFORN+ORCON per §H.4 p64 / p68.
/// - SI-G → ORCON per §H.4 p80 (NOFORN reaches SI-G via the
///   close()/default_fill boundary).
/// - TK-BLFH / TK-IDIT / TK-KAND → NOFORN per §H.4 p87 / p91 / p95.
const EXPECTED_BITMASK_CLOSURE_ROWS: &[&str] = &[
    "capco:closure.dissem.hcs-o-implies-noforn-orcon",
    "capco:closure.dissem.hcs-p-sub-implies-noforn-orcon",
    "capco:closure.dissem.si-g-implies-orcon",
    "capco:closure.dissem.tk-blfh-implies-noforn",
    "capco:closure.dissem.tk-idit-implies-noforn",
    "capco:closure.dissem.tk-kand-implies-noforn",
];

/// Closed **sorted set** of 39 `Constraint::Custom` row names from the
/// three catalog modules. Evaluation order is not engine-observable
/// (the bridge dispatcher routes by name string); only membership
/// matters, so the sorted-set form is the correct pin shape.
///
///   - core_catalog (9): the original `Custom` rows for the rules
///     whose predicate body did not fit `Conflicts` / `Requires` /
///     `Supersedes`. #559 close-out (2026-05-19) added E070 for the
///     FRD>TFNI leg per §H.6 p120; the prior 7-row count is bumped
///     to 8 by that addition. #388 (2026-05-21) added W005 for the
///     reverse-direction E014 check (REL TO entries not in JOINT)
///     per §H.3 p57 "[LIST]" superset semantics; the count rises
///     from 8 to 9.
///   - class_floor_catalog (27): the class-floor family per §H.4 /
///     §H.6 / §H.7 / §H.8 / §H.9. Includes the four `passthrough-*`
///     stubs for tokens not yet wired into a class-level predicate.
///   - sci_per_system_catalog (5): the SCI per-system family per §H.4
///     (HCS-O / HCS-P-NOFORN / HCS-P-sub / SI-G / TK-comp).
///
/// Total: 9 + 27 + 5 = 41. Note: the four RELIDO E054-E057 rows are
/// `Constraint::Conflicts`, NOT `Custom` — they do not appear here.
const EXPECTED_CUSTOM_CONSTRAINTS: &[&str] = &[
    // core_catalog (9) — predicate-ID form
    "portion.sci.hcs-system-constraints",
    "portion.classification.dual-classification",
    "portion.classification.joint-requires-rel-to-coverage",
    "portion.aea.rd-frd-requires-noforn",
    "portion.aea.rd-precedence",
    "portion.dissem.nodis-or-exdis-requires-noforn",
    // #559 close-out (2026-05-19): FRD>TFNI precedence per §H.6 p120.
    // Sibling of E024 (RD>FRD/TFNI); independent policy decision with
    // its own audit lineage per Constitution V Principle V.
    "portion.aea.frd-tfni-precedence",
    // #388 (2026-05-21): W005 reverse-direction E014. Flags REL TO
    // entries not in the JOINT participant list per §H.3 p57
    // "[LIST]" superset semantics. Warn-only (no auto-fix): cannot
    // distinguish intentional expansion from authoring error without
    // classifier input.
    "portion.classification.rel-to-not-in-joint-coverage",
    "portion.classification.joint-requires-usa",
    // class_floor_catalog (27)
    "banner.aea.floor-cnwdi",
    "banner.aea.ceiling-dod-ucni",
    "banner.aea.ceiling-doe-ucni",
    "banner.classification.floor-sar",
    "banner.aea.floor-atomal",
    "banner.classification.floor-balk",
    "banner.classification.floor-bohemia",
    "banner.dissem.floor-eyes-only",
    "banner.aea.floor-frd",
    "banner.aea.floor-frd-sg",
    "banner.classification.floor-hcs-comp",
    "banner.classification.floor-hcs-comp-sub",
    "banner.dissem.floor-imcon",
    "banner.dissem.floor-orcon",
    "banner.aea.floor-rd",
    "banner.aea.floor-rd-sg",
    "banner.dissem.floor-rsen",
    "banner.classification.floor-rsv-comp",
    "banner.classification.floor-si",
    "banner.classification.floor-si-comp",
    "banner.aea.floor-tfni",
    "banner.classification.floor-tk",
    "banner.classification.floor-tk-blfh",
    "banner.classification.floor-passthrough-bur",
    "banner.classification.floor-passthrough-hcs-x",
    "banner.classification.floor-passthrough-klm",
    "banner.classification.floor-passthrough-mvl",
    // sci_per_system_catalog (5)
    "marking.sci.hcs-o-companions",
    "marking.sci.hcs-p-noforn-required",
    "marking.sci.hcs-p-sub-companions",
    "marking.sci.si-g-companions",
    "marking.sci.tk-compartment-noforn-required",
];

#[test]
fn post_pr_4b_declares_exact_30_page_rewrites_in_order() {
    let scheme = CapcoScheme::new();
    let rewrites = scheme.page_rewrites();

    // Raw-slice cardinality — catches duplicate registration that a
    // set-equality check would silently collapse.
    let raw_len = rewrites.len();
    assert_eq!(
        raw_len, 30,
        "post-4b PageRewrite slice length drifted from 30: raw_len={raw_len}"
    );

    // Positional comparison — load-bearing because the topological
    // scheduler in `marque_engine::scheduler` breaks ties on
    // declaration order. A row reorder that the sorted-set check
    // would silently absorb would shift Kahn's-algorithm output.
    let actual: Vec<&str> = rewrites.iter().map(|r| r.id).collect();
    let expected: Vec<&str> = EXPECTED_PAGE_REWRITES.to_vec();

    assert_eq!(
        expected.len(),
        30,
        "EXPECTED_PAGE_REWRITES does not contain 30 entries: \
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
fn post_pr_d_declares_exact_residual_closure_rules() {
    let scheme = CapcoScheme::new();
    let rules = scheme.closure_rules();

    let raw_len = rules.len();
    assert_eq!(
        raw_len, 0,
        "post-#704 ClosureRule slice length drifted from 0: raw_len={raw_len}. \
         Issue #704 retired the residual `CLOSURE_REL_TO_USA_NATO` fn-pointer \
         rule along with the rest of the pre-#704 'default if absent' \
         architecture — Rows 0/7/8/9 relocated to \
         `crate::scheme::default_fill`. The fn-pointer trait surface is \
         empty post-#704; the 6 per-marking unconditional rows live in \
         the bitmask `CLOSURE_TABLE`."
    );

    let actual: Vec<&str> = rules.iter().map(|r| r.name).collect();
    let expected: Vec<&str> = EXPECTED_CLOSURE_RULES.to_vec();

    assert_eq!(
        expected.len(),
        0,
        "EXPECTED_CLOSURE_RULES does not contain 0 entries: \
         test data drifted, not the catalog"
    );

    if actual != expected {
        let actual_set: BTreeSet<&str> = actual.iter().copied().collect();
        let expected_set: BTreeSet<&str> = expected.iter().copied().collect();
        let missing: Vec<&str> = expected_set.difference(&actual_set).copied().collect();
        let unexpected: Vec<&str> = actual_set.difference(&expected_set).copied().collect();
        panic!(
            "post-#704 ClosureRule list drifted.\n\
             Missing: {missing:?}.\n\
             Unexpected: {unexpected:?}.\n\
             If both diffs are empty, the rows were reordered. Bumping \
             this test requires intentional review.\n\n\
             Actual order:   {actual:?}\n\
             Expected order: {expected:?}"
        );
    }
}

#[test]
fn post_pr_d_declares_unified_closure_inventory_in_registry_order() {
    let scheme = CapcoScheme::new();
    let inventory: Vec<_> = scheme.closure_inventory().collect();

    let raw_len = inventory.len();
    assert_eq!(
        raw_len, 6,
        "post-#704 closure inventory length drifted from 6: raw_len={raw_len}. \
         Unified inventory now mirrors the 6-row `CLOSURE_TABLE` directly \
         (Rows 0/7/8/9 retired to `default_fill`)."
    );

    let actual: Vec<&str> = inventory.iter().map(|row| row.name).collect();
    let expected: Vec<&str> = EXPECTED_BITMASK_CLOSURE_ROWS.to_vec();
    assert_eq!(
        actual, expected,
        "closure inventory row order drifted from registry order"
    );

    assert!(
        inventory.iter().all(|row| row.citation.is_some()),
        "every closure inventory row must expose citation metadata"
    );
    assert!(
        inventory
            .iter()
            .all(|row| row.default_severity == Severity::Info),
        "every current CAPCO closure inventory row must default to Severity::Info"
    );
}

/// Post-#704 parallel pin against the 6-row bitmask `CLOSURE_TABLE`.
#[test]
fn post_pr_d_declares_exact_6_bitmask_closure_rows_in_order() {
    use marque_capco::closure_table::CLOSURE_TABLE;

    let raw_len = CLOSURE_TABLE.len();
    assert_eq!(
        raw_len, 6,
        "post-#704 CLOSURE_TABLE row count drifted from 6: raw_len={raw_len}. \
         The bitmask catalog is the source-of-truth for the per-marking \
         unconditional closure rows; a count change here means a row was \
         added or removed."
    );

    let actual: Vec<&str> = CLOSURE_TABLE.iter().map(|r| r.name).collect();
    let expected: Vec<&str> = EXPECTED_BITMASK_CLOSURE_ROWS.to_vec();

    assert_eq!(
        expected.len(),
        6,
        "EXPECTED_BITMASK_CLOSURE_ROWS does not contain 6 entries: \
         test data drifted, not the catalog"
    );

    if actual != expected {
        let actual_set: BTreeSet<&str> = actual.iter().copied().collect();
        let expected_set: BTreeSet<&str> = expected.iter().copied().collect();
        let missing: Vec<&str> = expected_set.difference(&actual_set).copied().collect();
        let unexpected: Vec<&str> = actual_set.difference(&expected_set).copied().collect();
        panic!(
            "post-PR-D CLOSURE_TABLE positional list drifted.\n\
             Missing: {missing:?}.\n\
             Unexpected: {unexpected:?}.\n\
             If both diffs are empty, the rows were reordered — the \
             Kleene-fixpoint walk order is load-bearing per the \
             `CLOSURE_TABLE` doc-comment in `closure_table.rs`. \
             Bumping this test requires intentional review.\n\n\
             Actual order:   {actual:?}\n\
             Expected order: {expected:?}"
        );
    }
}

#[test]
fn post_pr_4b_declares_exact_41_custom_constraints() {
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
    // assertion catches duplicate-name drift:
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
        41,
        "EXPECTED_CUSTOM_CONSTRAINTS does not contain 41 unique entries: \
         test data drifted, not the catalog"
    );

    assert_eq!(
        raw_count, 41,
        "post-4b Constraint::Custom raw catalog count drifted from 41 \
         (5 SCI-per-system + 27 class-floor + 9 core-catalog): \
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
        41,
        "post-4b Constraint::Custom unique set size drifted from 41: \
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
