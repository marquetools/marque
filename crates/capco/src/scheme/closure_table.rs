// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Static `(trigger_mask, cone_mask)` closure-rule catalog over
//! [`FactBitmask`] + bitwise Kleene fixpoint [`close`].
//!
//! Part of the FactBitmask refactor (issue #371). This module owns the
//! data + dispatch shape; [`CapcoScheme::closure`] calls into it on the
//! hot path.
//!
//! # Post-#704 architecture
//!
//! Issue #704 removed the `suppressor_mask` field from [`ClosureRow`]
//! and the corresponding suppression branch from [`close`]. The
//! suppressor gating was mathematically incompatible with the closure
//! operator's algebraic **monotonicity** property
//! (`a ⊑ b ⟹ Cl(a) ⊑ Cl(b)`): adding suppressor bits via `b_extra`
//! blocked cones that fired for `a` alone, strictly losing bits from
//! `close(b)`. The proptest at
//! `crates/capco/tests/proptest_closure_table.rs::p3_monotonicity_realistic`
//! shrinks to a one-line counterexample on the pre-#704 architecture.
//!
//! Issue #704's refinement: the four "default if absent" rules
//! (pre-#704 Rows 0/7/8/9) — caveated→NOFORN, NATO→REL TO USA NATO,
//! SCI→RELIDO, US-class→RELIDO — are inherently non-monotone by
//! §-spec design (§B.3 paragraph b p19's "NOT MARKED PREVIOUSLY"
//! gate). They cannot live inside a closure operator that honors the
//! monotone contract. They moved to
//! [`crate::scheme::default_fill`], a separate post-close stage that
//! runs after `close()` converges. `CLOSURE_TABLE` now carries only
//! the six "unconditional implication" rules (HCS-O/HCS-P[sub] →
//! NOFORN+ORCON, SI-G → ORCON, TK-BLFH/IDIT/KAND → NOFORN per §H.4
//! marking templates), all of which fire unconditionally with no
//! suppressor, so `close()` is purely additive at the bitmask layer
//! and P3 monotonicity holds by construction.
//!
//! The narrower §H.8 p145 NOFORN-dominates semantics that handle
//! **input-explicit** FD&R contradictions (e.g., user marks
//! `{S, NOFORN, REL TO USA}` explicitly — §H.8 p145 says strip REL TO)
//! live in [`crate::lattice::dissem::DissemSet::with_all_overlays_reapplied`]
//! and [`crate::lattice::rel_to::RelToBlock::with_nato_implicit_stripped`].
//! Pipeline: `parse → join → close() (Rows 1-6) → apply_default_fill
//! (Rows 0/7/8/9) → apply_supersession_overlays (§H.8 p145
//! input-explicit) → PageRewrites → render`.
//!
//! # Row inventory (post-#704)
//!
//! The catalog carries **6 rows** post-#704 (Rows 1-6 of the pre-#704
//! 10-row catalog). The four retired rows (pre-#704 Rows 0/7/8/9 —
//! caveated/NATO/SCI/US-class "default if absent") relocated to
//! [`crate::scheme::default_fill`] because they are inherently
//! non-monotone (§B.3 paragraph b p19 "NOT MARKED PREVIOUSLY" gate)
//! and cannot live in a closure operator honoring the monotone
//! contract. See the module doc-comment for the full rationale.
//!
//! The surviving rows are per-marking unconditional implications
//! (HCS-O, HCS-P[sub], SI-G, TK-{BLFH,IDIT,KAND}) per §H.4 marking
//! templates. All fire unconditionally (no suppressor, no gate) so
//! `close()` is purely additive at the bitmask layer.
//!
//! # Dispatch semantics
//!
//! [`close`] walks the catalog in order within each Kleene iteration,
//! OR-ing the cone of any row whose trigger fires. Mutation happens to
//! the accumulated `next` value between rows, so an earlier row's cone
//! is visible to a later row's trigger check in the same iteration —
//! matching the in-pass ordering invariant from
//! `CAPCO_CLOSURE_RULES`'s catalog-order doc-comment.
//!
//! The Kleene loop runs at most [`MAX_CLOSURE_ITERATIONS`] = 16 passes;
//! the CAPCO catalog's longest causal chain is depth 2 (per-marking
//! cones add NOFORN/ORCON; CAVEATED promotes ORCON → NOFORN), so 16 is
//! a generous ceiling. Failing to converge under that ceiling is a
//! programming bug, and [`close`] **panics unconditionally** (release
//! builds included) per the `MarkingScheme::closure` trait contract
//! (§576 of `marque_scheme::scheme`: "The override MUST panic if it
//! exceeds [...] iterations without reaching a fixed point").
//!
//! # Citation discipline (Constitution VIII)
//!
//! Each row's [`ClosureRow::label`] preserves the §-citation chain from
//! the corresponding fn-pointer rule in `closure.rs`. The labels are
//! the source-of-truth for downstream consumers (audit emission, future
//! row-name severity overrides); they are not re-derived from any other
//! authority here.

use marque_scheme::{Citation, FactBitmask, SectionLetter, Severity, capco};

use crate::fact_bitmask::fact_bit;

// ---------------------------------------------------------------------------
// ClosureRow + CLOSURE_TABLE
// ---------------------------------------------------------------------------
//
// Post-#704: the `ROW0_NOFORN_IF_CAVEATED_TRIGGERS` 20-atom caveated
// trigger mask relocated to `crate::scheme::default_fill::ROW0_CAVEATED_TRIGGERS`
// when Row 0 (`capco/noforn-if-caveated`) moved out of the closure
// catalog. Per-trigger §-authorities (§B.3 Table 2 p21 + per-trigger
// §H.5 / §H.6 / §H.7 / §H.8 / §H.9 marking templates) are preserved
// on the relocated mask's doc-comment.

/// A single bitmask-form closure rule.
///
/// Mirrors [`marque_scheme::ClosureRule`] but stores trigger / cone as
/// raw `u128` masks instead of `&[TokenRef]` slices. The trade-off:
/// the bitmask form has no notion of `AnyInCategory` (so any
/// category-presence trigger must surface as a derived bit on
/// [`fact_bit`]) but evaluates with a single `&` op per axis instead
/// of an `iter().any()` walk per atom.
///
/// Row 7 (`capco/rel-to-usa-nato-if-nato-classification`) is the only
/// hybrid row in the catalog: its closed-vocab cone lives here
/// (`REL_TO_USA` bit), but its open-vocab NATO tetragraph cone is
/// applied by the `closure()` body via the
/// [`marque_scheme::ClosureRule::cone_derived`] fn-pointer on the
/// corresponding [`CAPCO_CLOSURE_RULES`](super::closure) entry.
///
/// # Post-#704 — no suppressor field
///
/// Issue #704 removed the pre-existing `suppressor_mask` field. The
/// suppressor gating violated the closure operator's algebraic
/// monotonicity property (`a ⊑ b ⟹ Cl(a) ⊑ Cl(b)`) — adding bits via
/// `b_extra` could activate suppressors and strictly lose cone bits
/// from `close(b)`. The §H.8 p145 / §B.3.a p19 FD&R supersession
/// semantics the suppressors encoded moved to
/// [`CapcoScheme::apply_supersession_overlays`], which runs after
/// [`close`] converges and observes the post-closure state. See the
/// module-level doc-comment for the architectural rationale.
///
/// # Fields
///
/// - `name` — stable severity-override key; matches the corresponding
///   [`marque_scheme::ClosureRule::name`] in [`CAPCO_CLOSURE_RULES`](super::closure).
/// - `label` — §-citation for the rule's primary authority; preserved
///   verbatim from the source fn-pointer rule's `label` per
///   Constitution VIII.
/// - `default_severity` — catalog default severity intent. Mirrors
///   `ClosureRule::default_severity` for unified inventory/discovery paths.
/// - `trigger_mask` — fires the row iff
///   `(working.bits() & trigger_mask) != 0`, where `working` is the
///   *evolving accumulator within the current Kleene iteration* —
///   NOT the original input. Earlier rows' cone bits become triggers
///   for later rows in the same iteration (e.g., SI-G → ORCON in
///   Row 3 then triggers Row 0's ORCON entry, adding NOFORN in the
///   same iteration). The intra-iteration mutation pattern matches
///   the fn-pointer catalog's documented in-pass ordering invariant.
/// - `cone_mask` — bits OR-ed into `working` when the row fires; the
///   updated `working` is what subsequent rows in the same iteration
///   read.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ClosureRow {
    pub name: &'static str,
    /// Human-readable display label for inventory surfaces. Migrated
    /// split the pre-existing `label: &'static str` field into two
    /// parts so the typed Citation can stand alone on `citation` while
    /// inventory consumers still get a human-facing string here.
    pub display_label: &'static str,
    /// Typed authoritative-source citation (migrated from `&'static str`
    /// to [`Citation`] — mirrors `ClosureRule.label` on the
    /// trait side; both feed the same `AuditNote.citation` field at
    /// firing time).
    pub label: Citation,
    pub default_severity: Severity,
    pub trigger_mask: u128,
    pub cone_mask: u128,
}

/// Per-row cone-mask constants for [`fact_bit::NOFORN`] / [`fact_bit::ORCON`]
/// Per-row cone-mask constants for the surviving Rows 1-6.
/// Pulled out so each row's `cone_mask` is one named atom rather
/// than a magic `1u128 << X` literal. The pre-#704 CONE_RELIDO and
/// CONE_REL_TO_USA constants retired with Rows 7/8/9.
const CONE_NOFORN: u128 = 1u128 << fact_bit::NOFORN;
const CONE_ORCON: u128 = 1u128 << fact_bit::ORCON;

/// The CAPCO closure-rule catalog in bitmask form (Rows 1-6 of the
/// pre-#704 10-row catalog).
///
/// **Post-#704 inventory**: six per-marking unconditional implication
/// rules from §H.4 marking templates (HCS-O / HCS-P[sub] / SI-G /
/// TK-BLFH / TK-IDIT / TK-KAND). Each row fires unconditionally on
/// its trigger atom with no suppressor, so `close()` is purely
/// additive at the bitmask layer and P3 monotonicity holds by
/// construction.
///
/// The pre-#704 Rows 0/7/8/9 (caveated→NOFORN, NATO→REL TO USA,
/// SCI→RELIDO, US-class→RELIDO) are inherently non-monotone
/// "default if absent" rules per §B.3 paragraph b p19's "NOT MARKED
/// PREVIOUSLY" gate. They relocated to
/// [`crate::scheme::default_fill`] where they live outside the
/// closure operator's monotone contract. See the module doc-comment
/// for the full rationale and the `default_fill` module for per-row
/// authority preservation.
pub static CLOSURE_TABLE: &[ClosureRow] = &[
    // Row 1 — Per-marking: HCS-O → NOFORN + ORCON.
    ClosureRow {
        name: "capco:closure.dissem.hcs-o-implies-noforn-orcon",
        display_label: "HCS-O implies NOFORN + ORCON",
        label: capco(SectionLetter::H, 4, 64),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_HCS_O,
        cone_mask: CONE_NOFORN | CONE_ORCON,
    },
    // Row 2 — Per-marking: HCS-P[sub] → NOFORN + ORCON.
    ClosureRow {
        name: "capco:closure.dissem.hcs-p-sub-implies-noforn-orcon",
        display_label: "HCS-P[sub] implies NOFORN + ORCON",
        label: capco(SectionLetter::H, 4, 68),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_HCS_P_SUB,
        cone_mask: CONE_NOFORN | CONE_ORCON,
    },
    // Row 3 — Per-marking: SI-G → ORCON.
    //
    // §H.4 p80 Example Banner Line is `TOP SECRET//SI-G//ORCON`
    // (ORCON only). NOFORN is intentionally NOT in SI-G's cone;
    // pre-#704 it was added transitively when Row 0 (CAVEATED) saw
    // the Row-3-added ORCON in the next Kleene iteration. Post-#704
    // the SI-G→ORCON→NOFORN chain still works: Row 3 fires in
    // `close()`, then `apply_default_fill` reads the post-close
    // bitmask (which has ORCON) and fires Row 0's default-fill,
    // adding NOFORN. The chain is preserved end-to-end.
    ClosureRow {
        name: "capco:closure.dissem.si-g-implies-orcon",
        display_label: "SI-G implies ORCON",
        label: capco(SectionLetter::H, 4, 80),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_SI_G,
        cone_mask: CONE_ORCON,
    },
    // Row 4 — Per-marking: TK-BLFH → NOFORN.
    ClosureRow {
        name: "capco:closure.dissem.tk-blfh-implies-noforn",
        display_label: "TK-BLFH implies NOFORN",
        label: capco(SectionLetter::H, 4, 87),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_TK_BLFH,
        cone_mask: CONE_NOFORN,
    },
    // Row 5 — Per-marking: TK-IDIT → NOFORN.
    ClosureRow {
        name: "capco:closure.dissem.tk-idit-implies-noforn",
        display_label: "TK-IDIT implies NOFORN",
        label: capco(SectionLetter::H, 4, 91),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_TK_IDIT,
        cone_mask: CONE_NOFORN,
    },
    // Row 6 — Per-marking: TK-KAND → NOFORN.
    ClosureRow {
        name: "capco:closure.dissem.tk-kand-implies-noforn",
        display_label: "TK-KAND implies NOFORN",
        label: capco(SectionLetter::H, 4, 95),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_TK_KAND,
        cone_mask: CONE_NOFORN,
    },
];

/// Convergence ceiling for [`close`].
///
/// Re-exports [`marque_scheme::MAX_CLOSURE_ITERATIONS`] (the scheme-
/// level cap referenced by the `MarkingScheme::closure` trait
/// contract). Aliasing here — rather than redefining `16` — closes
/// the drift class where the bitmask path's local cap could fall
/// out of sync with the trait surface and produce a non-convergence
/// failure mode the trait contract doesn't anticipate.
///
/// The CAPCO catalog's longest causal chain is depth 2 (SI-G → ORCON
/// → CAVEATED → NOFORN); the `N=16` ceiling is calibrated upstream
/// against that chain depth with 5× safety padding (see
/// `marque_scheme::closure` doc-comment). Reaching the ceiling
/// without converging is a programming bug — [`close`] panics
/// unconditionally in that case to honor the trait contract (§576
/// in `marque_scheme::scheme::MarkingScheme::closure`: "The override
/// MUST panic if it exceeds [...] iterations without reaching a
/// fixed point").
pub const MAX_CLOSURE_ITERATIONS: usize = marque_scheme::MAX_CLOSURE_ITERATIONS;

// ---------------------------------------------------------------------------
// Kleene fixpoint
// ---------------------------------------------------------------------------

/// Bitwise Kleene fixpoint over [`CLOSURE_TABLE`].
///
/// Walks the catalog in order within each iteration, OR-ing each
/// firing row's `cone_mask` into the accumulating `next` value.
/// Mutations from earlier rows are visible to later rows' trigger
/// checks in the same iteration. Iterates until the bitmask stops
/// changing or [`MAX_CLOSURE_ITERATIONS`] is reached.
///
/// # Correctness properties (proptest-verified)
///
/// - **Idempotence (P1)** — `close(close(b)) == close(b)`.
/// - **Extensivity (P2)** — `(close(b).bits() & b.bits()) == b.bits()`
///   (every input bit survives — `close` is purely additive on the
///   accumulator; rows OR cone bits in but never strip input bits).
/// - **Monotonicity (P3)** — `a ⊑ b ⟹ close(a) ⊑ close(b)`. Holds
///   because the operator is **purely additive** post-#704: each
///   row's firing predicate is the upward-closed presence check
///   `(working & trigger_mask) != 0`, which is monotone in
///   `working` (more bits → at least as many triggers fire), and
///   the row body only OR-s `cone_mask` into `working` — never
///   removes bits. Pre-#704 the table carried a
///   `suppressor_mask` whose presence flipped the firing predicate
///   to `trigger ∧ ¬suppressor`, which is anti-monotone in
///   `working` and broke P3 (adding bits via `b_extra` could
///   activate suppressors and strictly lose cone bits from
///   `close(b)` vs `close(a)`). The §H.8 p145 / §B.3.a p19 FD&R
///   supersession semantics that the suppressors encoded moved to
///   [`CapcoScheme::apply_supersession_overlays`] — a post-closure
///   overlay that observes the post-Kleene state and is composed
///   with the purely-additive closure operator without breaking
///   the closure layer's algebraic monotonicity.
/// - **Convergence (P4)** — converges in ≤ [`MAX_CLOSURE_ITERATIONS`]
///   iterations. Each iteration is non-decreasing (cone_mask is OR'd
///   in); the bitmask state is bounded by `2^WIDTH`; the loop
///   terminates in at most one iteration per bit added, capped at
///   [`MAX_CLOSURE_ITERATIONS`] for the convergence-bound assertion.
///
/// # Hot-path note
///
/// This data module does NOT call [`close`] on the production path; the
/// `closure()` body
/// [`CapcoScheme::closure`] to invoke it after a HOT-1 early-exit
/// against [`ALL_TRIGGER_MASK`].
#[must_use]
pub fn close(input: FactBitmask) -> FactBitmask {
    let mut bits = input.bits();
    for _ in 0..MAX_CLOSURE_ITERATIONS {
        let mut next = bits;
        for row in CLOSURE_TABLE {
            if (next & row.trigger_mask) != 0 {
                next |= row.cone_mask;
            }
        }
        if next == bits {
            return FactBitmask::from_bits(bits);
        }
        bits = next;
    }
    // Trait contract enforcement (§576 in
    // `marque_scheme::scheme::MarkingScheme::closure` — "The override
    // MUST panic if it exceeds [...] iterations without reaching a
    // fixed point"). The post-#704 6-row CAPCO catalog has max causal
    // depth 1 (each Row 1-6 fires at most once on its trigger atom;
    // the SI-G → ORCON → NOFORN chain that produced depth 2 in the
    // pre-#704 catalog now crosses the close()/default_fill boundary).
    // The upstream `N=16` cap provides 16× safety padding, so reaching
    // this branch means a future catalog edit introduced a cycle —
    // a programming bug. Panicking unconditionally (not just in debug
    // builds) honors the trait contract and prevents release-build
    // silent fallthrough.
    panic!(
        "close() did not converge in {MAX_CLOSURE_ITERATIONS} iterations; \
         bits = {bits:#034x}. The post-#704 CAPCO catalog's max causal \
         depth (1) makes this unreachable for the current 6-row catalog; \
         if this fires, a closure-row edit introduced a cycle.",
    );
}

/// Union of every row's `trigger_mask` in [`CLOSURE_TABLE`].
///
/// The `closure()` body uses this as the HOT-1 early-exit gate:
/// if `(derive_bits(attrs).bits() & ALL_TRIGGER_MASK) == 0`, no row
/// can fire and the bitmask projection / Kleene loop / inverse
/// projection are skipped entirely. Computed at compile time.
pub const ALL_TRIGGER_MASK: u128 = {
    let mut acc: u128 = 0;
    let mut i = 0;
    while i < CLOSURE_TABLE.len() {
        acc |= CLOSURE_TABLE[i].trigger_mask;
        i += 1;
    }
    acc
};

// ---------------------------------------------------------------------------
// Inline tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    /// Post-#704 the catalog ships 6 rows (Rows 1-6 from the pre-#704
    /// 10-row catalog). Rows 0/7/8/9 relocated to
    /// `crate::scheme::default_fill`. A drift here is the strongest
    /// signal that the table was edited without intent.
    #[test]
    fn catalog_has_six_rows() {
        assert_eq!(CLOSURE_TABLE.len(), 6);
    }

    /// Row names match the per-marking unconditional implications
    /// from §H.4 marking templates. Drift here breaks severity-
    /// override config keys + future audit row-name emission.
    #[test]
    fn row_names_match_per_marking_inventory() {
        let expected_names = [
            "capco:closure.dissem.hcs-o-implies-noforn-orcon",
            "capco:closure.dissem.hcs-p-sub-implies-noforn-orcon",
            "capco:closure.dissem.si-g-implies-orcon",
            "capco:closure.dissem.tk-blfh-implies-noforn",
            "capco:closure.dissem.tk-idit-implies-noforn",
            "capco:closure.dissem.tk-kand-implies-noforn",
        ];
        for (row, expected) in CLOSURE_TABLE.iter().zip(expected_names.iter()) {
            assert_eq!(row.name, *expected);
        }
    }

    /// Every cone bit must be one of the two `APPLY_ELIGIBLE_MASK`
    /// atoms the post-#704 catalog uses (NOFORN / ORCON). RELIDO
    /// and REL_TO_USA cones retired with Rows 7/8/9 (relocated to
    /// `crate::scheme::default_fill`).
    #[test]
    fn cones_within_apply_eligible_set() {
        let eligible = CONE_NOFORN | CONE_ORCON;
        for row in CLOSURE_TABLE {
            assert_eq!(
                row.cone_mask & !eligible,
                0,
                "row {} has cone bits outside the per-marking cone set",
                row.name,
            );
        }
    }

    /// Empty input → empty output (no row's trigger fires).
    #[test]
    fn close_of_empty_is_empty() {
        assert_eq!(close(FactBitmask::EMPTY), FactBitmask::EMPTY);
    }

    /// HCS-O alone → adds NOFORN + ORCON via Row 1.
    #[test]
    fn close_hcs_o_adds_noforn_and_orcon() {
        let input = FactBitmask::EMPTY.with_bit(fact_bit::SCI_HCS_O);
        let closed = close(input);
        assert!(closed.is_set(fact_bit::NOFORN));
        assert!(closed.is_set(fact_bit::ORCON));
    }

    /// SI-G alone → adds ORCON via Row 3 only.
    ///
    /// Post-#704: the SI-G → ORCON → NOFORN chain crosses the
    /// close()/default_fill boundary. close() runs Row 3 which adds
    /// ORCON; the NOFORN injection happens in `apply_default_fill`
    /// when its Row-0 predicate observes ORCON in the post-close
    /// bitmask. This test pins the closure-layer half of the chain
    /// (RELIDO is NOT in the closure output because Row 8 retired
    /// to default-fill, and SI_G is in MASK_FDR_OR_RELIDO_INCOMPAT
    /// so default-fill Row 8 wouldn't fire even if it ran here).
    #[test]
    fn close_si_g_adds_orcon_only() {
        let input = FactBitmask::EMPTY
            .with_bit(fact_bit::SCI_SI_G)
            .with_bit(fact_bit::SCI_PRESENT);
        let closed = close(input);
        assert!(closed.is_set(fact_bit::ORCON));
        // Post-#704: NOFORN is NOT added by close() — it comes from
        // `apply_default_fill`'s Row 0 default-fill running on the
        // post-close bitmask (where Row 3 added ORCON, the Row 0
        // caveat trigger).
        assert!(!closed.is_set(fact_bit::NOFORN));
        // Post-#704: RELIDO is NOT in the close() output (Row 8 moved
        // to default-fill).
        assert!(!closed.is_set(fact_bit::RELIDO));
    }

    /// US Secret alone — Row 9 retired to default-fill. close()
    /// produces no addition on bare US classification.
    #[test]
    fn close_us_secret_alone_is_noop() {
        let input = FactBitmask::EMPTY.with_bit(fact_bit::US_COLLATERAL_CLASSIFIED);
        let closed = close(input);
        // Post-#704: Row 9 (US-class → RELIDO) retired to
        // default-fill; close() leaves bare US classification
        // unchanged. The default-fill stage in project()'s pipeline
        // adds RELIDO when run on the post-close marking.
        assert_eq!(closed.bits(), input.bits());
    }

    /// Idempotence — close(close(b)) == close(b). Spot-check with the
    /// SI-G chain (the longest causal chain in the post-#704 catalog).
    #[test]
    fn idempotence_spot_check_si_g_chain() {
        let input = FactBitmask::EMPTY
            .with_bit(fact_bit::SCI_SI_G)
            .with_bit(fact_bit::SCI_PRESENT)
            .with_bit(fact_bit::US_COLLATERAL_CLASSIFIED);
        let once = close(input);
        let twice = close(once);
        assert_eq!(once, twice);
    }

    /// Extensivity — every input bit survives.
    #[test]
    fn extensivity_spot_check_hcs_o() {
        let input = FactBitmask::EMPTY
            .with_bit(fact_bit::SCI_HCS_O)
            .with_bit(fact_bit::US_COLLATERAL_CLASSIFIED);
        let closed = close(input);
        assert_eq!(closed.bits() & input.bits(), input.bits());
    }

    /// ALL_TRIGGER_MASK must be the union of every row's trigger_mask.
    #[test]
    fn all_trigger_mask_is_union() {
        let expected: u128 = CLOSURE_TABLE.iter().fold(0, |acc, r| acc | r.trigger_mask);
        assert_eq!(ALL_TRIGGER_MASK, expected);
    }

    /// HOT-1 invariant: if no trigger bit is set in the input, the
    /// fixpoint MUST equal the input. Post-#704 the catalog's triggers
    /// are limited to the six SCI sentinel bits (SCI_HCS_O /
    /// SCI_HCS_P_SUB / SCI_SI_G / SCI_TK_BLFH / SCI_TK_IDIT /
    /// SCI_TK_KAND), so a bitmask with any non-SCI atoms (e.g., ORCON,
    /// NODIS, US_COLLATERAL_CLASSIFIED) hits the early-exit.
    #[test]
    fn hot1_no_trigger_means_no_change() {
        let input = FactBitmask::EMPTY
            .with_bit(fact_bit::NODIS)
            .with_bit(fact_bit::EXDIS)
            .with_bit(fact_bit::ORCON)
            .with_bit(fact_bit::US_COLLATERAL_CLASSIFIED);
        assert_eq!(input.bits() & ALL_TRIGGER_MASK, 0);
        assert_eq!(close(input), input);
    }

    /// Post-#704: bare NATO classification is NOT a close() trigger
    /// (Row 7 retired to default-fill). close() leaves NATO inputs
    /// unchanged; `apply_default_fill` adds REL TO USA + NATO downstream.
    #[test]
    fn close_nato_classification_is_noop() {
        let input = FactBitmask::EMPTY.with_bit(fact_bit::NATO_CLASS);
        let closed = close(input);
        assert_eq!(closed.bits(), input.bits());
    }
}
