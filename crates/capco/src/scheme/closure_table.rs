// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Static `(trigger_mask, suppressor_mask, cone_mask)` closure-rule catalog
//! over [`FactBitmask`] + bitwise Kleene fixpoint [`close`].
//!
//! PR-C of the FactBitmask refactor (issue #371). This module owns the
//! data + dispatch shape; PR-D rewires [`CapcoScheme::closure`] to call
//! into it on the hot path. PR-C itself does NOT consume the table from
//! the production pipeline — `CapcoScheme::closure` still walks the
//! fn-pointer [`CAPCO_CLOSURE_RULES`] catalog. The PR-C deliverable is
//! the table + the Kleene loop + the equivalence cross-check against
//! the existing catalog (`tests/closure_table_equivalence.rs`) +
//! P1-P4 proptests (`tests/proptest_closure_table.rs`).
//!
//! # Row inventory
//!
//! The catalog mirrors `crates/capco/src/scheme/closure.rs::CAPCO_CLOSURE_RULES`
//! row-for-row in **catalog order** (section 4 of the PR-B plan; load-bearing
//! per the `post_4b_lattice_inventory_pin.rs` positional pin). The 10 rows
//! split into:
//!
//! 1. **Row 0** — Trio 1 `capco/noforn-if-caveated`.
//! 2. **Rows 1-6** — per-marking unconditional implications (HCS-O,
//!    HCS-P[sub], SI-G, TK-{BLFH,IDIT,KAND}).
//! 3. **Row 7** — Trio 3 `capco/rel-to-usa-nato-if-nato-classification`.
//!    *Hybrid*: bitmask trigger + bitmask suppressor + closed-vocab
//!    static cone (`REL_TO_USA`). The open-vocab NATO tetragraph (the
//!    `cone_derived` fn-pointer pass) lives outside the bitmask table
//!    and is applied by PR-D's `closure()` body after the Kleene loop
//!    converges.
//! 4. **Rows 8-9** — Trio 2 `capco/relido-if-sci-and-not-incompatible`
//!    + `capco/relido-if-us-collateral-class`.
//!
//! # Dispatch semantics
//!
//! [`close`] walks the catalog in order within each Kleene iteration,
//! OR-ing the cone of any row whose trigger fires and whose suppressor
//! does not. Mutation happens to the accumulated `next` value between
//! rows, so an earlier row's cone is visible to a later row's
//! suppressor check in the same iteration — matching the in-pass
//! ordering invariant from `CAPCO_CLOSURE_RULES`'s catalog-order
//! doc-comment.
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

use marque_scheme::{Citation, FactBitmask, SectionLetter, Severity, capco, capco_table};

use crate::fact_bitmask::{
    MASK_FDR_DOMINATORS, MASK_FDR_OR_RELIDO_INCOMPAT, MASK_RELIDO_US_CLASS_SUPPRESSORS, fact_bit,
};

// ---------------------------------------------------------------------------
// Trigger masks
// ---------------------------------------------------------------------------

/// Trigger mask for `capco/noforn-if-caveated` (Row 0).
///
/// Bitmask form of `CLOSURE_NOFORN_CAVEATED.triggers` from
/// `closure.rs`:
///
/// - 1 SAR — `AnyInCategory(CAT_SAR)`
/// - 5 AEA — `TOK_RD`, `TOK_FRD`, `TOK_TFNI`, `TOK_UCNI`, `TOK_DCNI`
/// - 2 FGI — `TOK_FGI_MARKER` + `AnyInCategory(CAT_FGI_MARKER)` (per
///   `closure.rs::CLOSURE_NOFORN_CAVEATED.triggers`; `TOK_FGI_CLASS`
///   is NOT in the rule's trigger list — `TOK_FGI_MARKER`'s
///   `satisfies_attrs` resolution already covers both the dissem-
///   axis `fgi_marker` and the classification-axis
///   `MarkingClassification::Fgi(_)` paths)
/// - 8 IC dissem — `TOK_ORCON`, `TOK_ORCON_USGOV`, `TOK_RSEN`,
///   `TOK_IMCON`, `TOK_PROPIN`, `TOK_DSEN`, `TOK_FISA`, `TOK_RAWFISA`
/// - 5 non-IC dissem — `TOK_LIMDIS`, `TOK_LES`, `TOK_NNPI`,
///   `TOK_SBU`, `TOK_SSI`
///
/// Total: 21 `TokenRef` entries on the fn-pointer rule. The two FGI
/// predicate forms collapse to the single [`fact_bit::FGI_PRESENT`]
/// sentinel in the bitmask projection (the bit lights for *any* FGI
/// presence — marker axis OR classification axis — via
/// `derive_bits`). Net bitmask form: **20 distinct atom bits**.
///
/// Authority: see [`CLOSURE_NOFORN_CAVEATED`](super::closure) doc-comment
/// per-trigger authority table.
const ROW0_NOFORN_IF_CAVEATED_TRIGGERS: u128 = (1u128 << fact_bit::SAR_PRESENT)
    | (1u128 << fact_bit::AEA_RD)
    | (1u128 << fact_bit::AEA_FRD)
    | (1u128 << fact_bit::AEA_TFNI)
    | (1u128 << fact_bit::AEA_DOE_UCNI)
    | (1u128 << fact_bit::AEA_DOD_UCNI)
    | (1u128 << fact_bit::FGI_PRESENT)
    | (1u128 << fact_bit::ORCON)
    | (1u128 << fact_bit::ORCON_USGOV)
    | (1u128 << fact_bit::RSEN)
    | (1u128 << fact_bit::IMCON)
    | (1u128 << fact_bit::PROPIN)
    | (1u128 << fact_bit::DSEN)
    | (1u128 << fact_bit::FISA)
    | (1u128 << fact_bit::RAWFISA)
    | (1u128 << fact_bit::LIMDIS)
    | (1u128 << fact_bit::LES)
    | (1u128 << fact_bit::NNPI)
    | (1u128 << fact_bit::SBU)
    | (1u128 << fact_bit::SSI);

// ---------------------------------------------------------------------------
// ClosureRow + CLOSURE_TABLE
// ---------------------------------------------------------------------------

/// A single bitmask-form closure rule.
///
/// Mirrors [`marque_scheme::ClosureRule`] but stores trigger / suppressor /
/// cone as raw `u128` masks instead of `&[TokenRef]` slices. The trade-off:
/// the bitmask form has no notion of `AnyInCategory` (so any
/// category-presence trigger must surface as a derived bit on
/// [`fact_bit`]) but evaluates with a single `&` op per axis instead of
/// an `iter().any()` walk per atom.
///
/// Row 7 (`capco/rel-to-usa-nato-if-nato-classification`) is the only
/// hybrid row in the catalog: its closed-vocab cone lives here
/// (`REL_TO_USA` bit), but its open-vocab NATO tetragraph cone is
/// applied by PR-D's `closure()` body via the
/// [`marque_scheme::ClosureRule::cone_derived`] fn-pointer on the
/// corresponding [`CAPCO_CLOSURE_RULES`](super::closure) entry.
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
/// - `suppressor_mask` — suppresses the row iff
///   `(working.bits() & suppressor_mask) != 0` (same evolving state
///   semantic — earlier rows' cones can activate suppressors for
///   later rows in the same iteration). A mask of `0` means no
///   suppressors (unconditional firing).
/// - `cone_mask` — bits OR-ed into `working` when the row fires; the
///   updated `working` is what subsequent rows in the same iteration
///   read.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ClosureRow {
    pub name: &'static str,
    /// Human-readable display label for inventory surfaces. PR 10.A.1
    /// split the pre-existing `label: &'static str` field into two
    /// parts so the typed Citation can stand alone on `citation` while
    /// inventory consumers still get a human-facing string here.
    pub display_label: &'static str,
    /// Typed authoritative-source citation (migrated from `&'static str`
    /// to [`Citation`] in PR 10.A.1 — mirrors `ClosureRule.label` on the
    /// trait side; both feed the same `AuditNote.citation` field at
    /// firing time).
    pub label: Citation,
    pub default_severity: Severity,
    pub trigger_mask: u128,
    pub suppressor_mask: u128,
    pub cone_mask: u128,
}

/// Per-row cone-mask constants for [`fact_bit::NOFORN`] / [`fact_bit::ORCON`]
/// / [`fact_bit::RELIDO`] / [`fact_bit::REL_TO_USA`] singletons. Pulled out
/// so each row's `cone_mask` is one named atom rather than a magic
/// `1u128 << X` literal.
const CONE_NOFORN: u128 = 1u128 << fact_bit::NOFORN;
const CONE_ORCON: u128 = 1u128 << fact_bit::ORCON;
const CONE_RELIDO: u128 = 1u128 << fact_bit::RELIDO;
pub(crate) const CONE_REL_TO_USA: u128 = 1u128 << fact_bit::REL_TO_USA;

/// The 10-row CAPCO closure-rule catalog in bitmask form.
///
/// **Row ordering is load-bearing.** The order matches
/// [`CAPCO_CLOSURE_RULES`](super::closure) verbatim: Trio 1 first
/// (Row 0) so subsequent rows see updated NOFORN/REL_TO_PRESENT;
/// per-marking unconditional rows next (Rows 1-6) so Trio 3 / Trio 2
/// see updated NOFORN/ORCON; Trio 3 (Row 7); Trio 2 last (Rows 8-9).
/// The catalog walks in order within each Kleene iteration; mutations
/// from earlier rows are visible to later rows' suppressor checks.
///
/// The `post_4b_lattice_inventory_pin.rs` positional pin gates a
/// parallel pin for this table once PR-D wires the production path
/// through it (PR-C ships the table unused on production).
pub static CLOSURE_TABLE: &[ClosureRow] = &[
    // Row 0 — Trio 1: caveated → NOFORN.
    ClosureRow {
        name: "capco/noforn-if-caveated",
        display_label: "NOFORN if classified-and-caveated",
        label: capco_table(SectionLetter::B, 3, 2, 21),
        default_severity: Severity::Info,
        trigger_mask: ROW0_NOFORN_IF_CAVEATED_TRIGGERS,
        suppressor_mask: MASK_FDR_DOMINATORS,
        cone_mask: CONE_NOFORN,
    },
    // Row 1 — Per-marking: HCS-O → NOFORN + ORCON.
    ClosureRow {
        name: "capco/hcs-o-implies-noforn-orcon",
        display_label: "HCS-O implies NOFORN + ORCON",
        label: capco(SectionLetter::H, 4, 64),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_HCS_O,
        suppressor_mask: 0,
        cone_mask: CONE_NOFORN | CONE_ORCON,
    },
    // Row 2 — Per-marking: HCS-P[sub] → NOFORN + ORCON.
    ClosureRow {
        name: "capco/hcs-p-sub-implies-noforn-orcon",
        display_label: "HCS-P[sub] implies NOFORN + ORCON",
        label: capco(SectionLetter::H, 4, 68),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_HCS_P_SUB,
        suppressor_mask: 0,
        cone_mask: CONE_NOFORN | CONE_ORCON,
    },
    // Row 3 — Per-marking: SI-G → ORCON (NOFORN intentionally NOT in
    // cone per §H.4 p80 Example Banner Line; Trio 1 adds NOFORN
    // transitively via ORCON in caveated triggers).
    ClosureRow {
        name: "capco/si-g-implies-orcon",
        display_label: "SI-G implies ORCON",
        label: capco(SectionLetter::H, 4, 80),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_SI_G,
        suppressor_mask: 0,
        cone_mask: CONE_ORCON,
    },
    // Row 4 — Per-marking: TK-BLFH → NOFORN.
    ClosureRow {
        name: "capco/tk-blfh-implies-noforn",
        display_label: "TK-BLFH implies NOFORN",
        label: capco(SectionLetter::H, 4, 87),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_TK_BLFH,
        suppressor_mask: 0,
        cone_mask: CONE_NOFORN,
    },
    // Row 5 — Per-marking: TK-IDIT → NOFORN.
    ClosureRow {
        name: "capco/tk-idit-implies-noforn",
        display_label: "TK-IDIT implies NOFORN",
        label: capco(SectionLetter::H, 4, 91),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_TK_IDIT,
        suppressor_mask: 0,
        cone_mask: CONE_NOFORN,
    },
    // Row 6 — Per-marking: TK-KAND → NOFORN.
    ClosureRow {
        name: "capco/tk-kand-implies-noforn",
        display_label: "TK-KAND implies NOFORN",
        label: capco(SectionLetter::H, 4, 95),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_TK_KAND,
        suppressor_mask: 0,
        cone_mask: CONE_NOFORN,
    },
    // Row 7 — Trio 3: bare NATO classification → REL TO USA (+ NATO
    // open-vocab cone applied by PR-D outside the bitmask loop).
    ClosureRow {
        name: "capco/rel-to-usa-nato-if-nato-classification",
        display_label: "Bare NATO classification implies REL TO USA, NATO",
        label: capco(SectionLetter::H, 7, 127),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::NATO_CLASS,
        suppressor_mask: MASK_FDR_DOMINATORS,
        cone_mask: CONE_REL_TO_USA,
    },
    // Row 8 — Trio 2: SCI presence → RELIDO unless FD&R-marked or
    // RELIDO-incompatible.
    ClosureRow {
        name: "capco/relido-if-sci-and-not-incompatible",
        display_label: "SCI presence implies RELIDO (unless FD&R or incompatible)",
        label: capco(SectionLetter::H, 8, 154),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::SCI_PRESENT,
        suppressor_mask: MASK_FDR_OR_RELIDO_INCOMPAT,
        cone_mask: CONE_RELIDO,
    },
    // Row 9 — Trio 2: US collateral classification → RELIDO unless
    // FD&R-marked or per-compartment SCI sentinel present.
    ClosureRow {
        name: "capco/relido-if-us-collateral-class",
        display_label: "US collateral classification implies RELIDO (unless FD&R)",
        label: capco_table(SectionLetter::B, 3, 2, 21),
        default_severity: Severity::Info,
        trigger_mask: 1u128 << fact_bit::US_COLLATERAL_CLASSIFIED,
        suppressor_mask: MASK_RELIDO_US_CLASS_SUPPRESSORS,
        cone_mask: CONE_RELIDO,
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
/// Mutations from earlier rows are visible to later rows' suppressor
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
///   because each row's firing predicate (`trigger ∧ ¬suppressor`)
///   transforms monotonely with bit additions: a larger input can
///   only (a) activate more triggers and (b) activate more
///   suppressors. (a) strictly enlarges the cone-output set; (b)
///   prevents *additional* cone additions on the larger input but
///   never reverses a cone addition already made on the smaller
///   input — `close` is purely additive (P2), so suppressor
///   activation has no destructive effect. The intra-iteration walk
///   order is monotonicity-preserving for the same reason: a row
///   that fires on input `a` at row-index `i` and on input `b` at
///   the same index sees a strict superset of the working state,
///   so its decision can flip from fire-to-suppressed but never
///   from suppressed-to-fire when starting from a state that
///   already contained the suppressor bits.
/// - **Convergence (P4)** — converges in ≤ [`MAX_CLOSURE_ITERATIONS`]
///   iterations. Each iteration is non-decreasing (cone_mask is OR'd
///   in); the bitmask state is bounded by `2^WIDTH`; the loop
///   terminates in at most one iteration per bit added, capped at
///   [`MAX_CLOSURE_ITERATIONS`] for the convergence-bound assertion.
///
/// # Hot-path note
///
/// PR-C does NOT call [`close`] on the production path. PR-D rewires
/// [`CapcoScheme::closure`] to invoke it after a HOT-1 early-exit
/// against [`ALL_TRIGGER_MASK`]. The bench impact is measured in PR-F.
#[must_use]
pub fn close(input: FactBitmask) -> FactBitmask {
    let mut bits = input.bits();
    for _ in 0..MAX_CLOSURE_ITERATIONS {
        let mut next = bits;
        for row in CLOSURE_TABLE {
            let trigger_hit = (next & row.trigger_mask) != 0;
            let suppressed = row.suppressor_mask != 0 && (next & row.suppressor_mask) != 0;
            if trigger_hit && !suppressed {
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
    // fixed point"). The 10-row CAPCO catalog has max causal depth 2
    // (per-marking cones add NOFORN/ORCON; CAVEATED promotes ORCON →
    // NOFORN) with the upstream `N=16` cap providing 5× safety
    // padding, so reaching this branch means a future catalog edit
    // introduced a cycle or extended the chain past the ceiling —
    // both are programming bugs. Panicking unconditionally (not just
    // in debug builds) honors the trait contract and prevents
    // release-build silent fallthrough that would mask a non-monotone
    // catalog regression with a wrong cone output.
    panic!(
        "close() did not converge in {MAX_CLOSURE_ITERATIONS} iterations; \
         bits = {bits:#034x}. The CAPCO catalog's max causal depth (2) \
         + 5× safety padding makes this unreachable for the current \
         10-row catalog; if this fires, a closure-row edit introduced \
         a cycle or extended the chain past the upstream cap.",
    );
}

/// Union of every row's `trigger_mask` in [`CLOSURE_TABLE`].
///
/// PR-D's `closure()` rewire uses this as the HOT-1 early-exit gate:
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

    /// The catalog ships 10 rows. A drift here is the strongest signal
    /// that the table was edited without intent — gates PR-D's
    /// equivalence-test row count.
    #[test]
    fn catalog_has_ten_rows() {
        assert_eq!(CLOSURE_TABLE.len(), 10);
    }

    /// Row 0 trigger mask must include all 20 caveated-trigger atoms
    /// per `closure.rs::CLOSURE_NOFORN_CAVEATED.triggers`.
    #[test]
    fn row0_trigger_count() {
        assert_eq!(ROW0_NOFORN_IF_CAVEATED_TRIGGERS.count_ones(), 20);
    }

    /// Row names match the corresponding fn-pointer
    /// [`CAPCO_CLOSURE_RULES`] entries verbatim. Drift here breaks
    /// severity-override config keys + future audit row-name emission.
    #[test]
    fn row_names_match_fn_pointer_catalog() {
        let expected_names = [
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
        for (row, expected) in CLOSURE_TABLE.iter().zip(expected_names.iter()) {
            assert_eq!(row.name, *expected);
        }
    }

    /// Per-marking rows (1-6) have no suppressor — they fire
    /// unconditionally per `marque-applied.md` §4.7.5 ("Per-marking
    /// unconditional implications").
    #[test]
    fn per_marking_rows_have_no_suppressor() {
        for (i, row) in CLOSURE_TABLE.iter().enumerate().take(7).skip(1) {
            assert_eq!(
                row.suppressor_mask, 0,
                "row {} ({}) must have no suppressor",
                i, row.name,
            );
        }
    }

    /// Trio 1 / Trio 3 / Trio 2 rows MUST be suppressed by their
    /// respective suppressor masks. Row 0 + Row 7 use
    /// `MASK_FDR_DOMINATORS`; Row 8 uses `MASK_FDR_OR_RELIDO_INCOMPAT`;
    /// Row 9 uses `MASK_RELIDO_US_CLASS_SUPPRESSORS`.
    #[test]
    fn row_suppressors_match_design() {
        assert_eq!(CLOSURE_TABLE[0].suppressor_mask, MASK_FDR_DOMINATORS);
        assert_eq!(CLOSURE_TABLE[7].suppressor_mask, MASK_FDR_DOMINATORS);
        assert_eq!(
            CLOSURE_TABLE[8].suppressor_mask,
            MASK_FDR_OR_RELIDO_INCOMPAT
        );
        assert_eq!(
            CLOSURE_TABLE[9].suppressor_mask,
            MASK_RELIDO_US_CLASS_SUPPRESSORS,
        );
    }

    /// Every cone bit must be one of the four `APPLY_ELIGIBLE_MASK`
    /// atoms (NOFORN / ORCON / RELIDO / REL_TO_USA). The inverse
    /// projection silently drops any other cone bit, so a row with
    /// an out-of-set cone would no-op silently.
    #[test]
    fn cones_within_apply_eligible_set() {
        let eligible = CONE_NOFORN | CONE_ORCON | CONE_RELIDO | CONE_REL_TO_USA;
        for row in CLOSURE_TABLE {
            assert_eq!(
                row.cone_mask & !eligible,
                0,
                "row {} has cone bits outside APPLY_ELIGIBLE_MASK",
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

    /// SI-G alone → adds ORCON via Row 3, then transitively NOFORN
    /// via Row 0 (ORCON is in Row 0's trigger list).
    #[test]
    fn close_si_g_chains_orcon_then_noforn() {
        let input = FactBitmask::EMPTY
            .with_bit(fact_bit::SCI_SI_G)
            .with_bit(fact_bit::SCI_PRESENT);
        let closed = close(input);
        assert!(closed.is_set(fact_bit::ORCON));
        assert!(closed.is_set(fact_bit::NOFORN));
        // Row 8 (RELIDO_SCI) must NOT fire — SI_G is in
        // MASK_FDR_OR_RELIDO_INCOMPAT.
        assert!(!closed.is_set(fact_bit::RELIDO));
    }

    /// US Secret alone (no caveats, no SCI) → Row 9 fires +RELIDO.
    #[test]
    fn close_us_secret_alone_adds_relido() {
        let input = FactBitmask::EMPTY.with_bit(fact_bit::US_COLLATERAL_CLASSIFIED);
        let closed = close(input);
        assert!(closed.is_set(fact_bit::RELIDO));
        // Row 0 must NOT fire — no caveated trigger present.
        assert!(!closed.is_set(fact_bit::NOFORN));
    }

    /// FD&R presence (NOFORN) suppresses Row 0 + Row 9 + Row 7.
    #[test]
    fn fdr_presence_suppresses_caveated_relido_and_nato_rows() {
        // ORCON + NOFORN + US Secret. Row 0 normally fires on ORCON,
        // but NOFORN is in MASK_FDR_DOMINATORS so it's suppressed.
        let input = FactBitmask::EMPTY
            .with_bit(fact_bit::ORCON)
            .with_bit(fact_bit::NOFORN)
            .with_bit(fact_bit::US_COLLATERAL_CLASSIFIED);
        let closed = close(input);
        // Stable fixpoint: nothing new added.
        assert_eq!(closed, input);
    }

    /// Idempotence — close(close(b)) == close(b). Spot-check with the
    /// SI-G chain (the longest causal chain in the catalog).
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
    /// fixpoint MUST equal the input.
    #[test]
    fn hot1_no_trigger_means_no_change() {
        // NODIS / EXDIS / SBU_NF / LES_NF are non-IC dissem tokens NOT
        // in any closure-row trigger mask (Trio 1 fires on LIMDIS / LES
        // / NNPI / SBU / SSI but not on these four). A bitmask whose
        // only set bits are non-trigger atoms must converge to itself
        // — gates the HOT-1 early-exit PR-D will install at the
        // production call site.
        let input = FactBitmask::EMPTY
            .with_bit(fact_bit::NODIS)
            .with_bit(fact_bit::EXDIS);
        assert_eq!(input.bits() & ALL_TRIGGER_MASK, 0);
        assert_eq!(close(input), input);
    }

    /// NATO classification → Row 7 adds REL_TO_USA (the closed-vocab
    /// cone). NATO open-vocab cone lives outside this table.
    #[test]
    fn close_nato_classification_adds_rel_to_usa() {
        let input = FactBitmask::EMPTY.with_bit(fact_bit::NATO_CLASS);
        let closed = close(input);
        assert!(closed.is_set(fact_bit::REL_TO_USA));
    }
}
