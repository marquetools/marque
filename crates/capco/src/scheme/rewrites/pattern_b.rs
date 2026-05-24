// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Pattern-B structural FOUO-eviction page rewrites.

use marque_scheme::{
    CategoryAction, CategoryPredicate, FactRef, PageRewrite, ReplacementIntent, Scope,
    SectionLetter, capco,
};

use super::super::predicates::{fouo_classified_trigger, fouo_with_non_fdr_other_control_trigger};
use super::super::*;

/// The two Pattern-B FOUO-eviction rows in declaration order:
/// `capco/classification-evicts-fouo` followed by
/// `capco/non-fdr-control-evicts-fouo`.
pub(super) fn pattern_b_rows() -> Vec<PageRewrite<CapcoScheme>> {
    // classification-evicts-fouo: same axes as PATTERN_C_FOUO —
    // reads `[CAT_CLASSIFICATION]` only; writes `[CAT_DISSEM]`.
    // CAT_DISSEM is intentionally NOT in `reads` even though the
    // predicate scans it for FOUO, because the existing
    // `capco/noforn-clears-fdr-family` row already reads + writes
    // CAT_DISSEM (the scheduler accepts that as a 1-row self-
    // edge); declaring another reads-DISSEM/writes-DISSEM row
    // creates a 2-row cycle (Kahn rejects). The FOUO-presence
    // scan lives in the `fouo_classified_trigger` Custom
    // predicate body. §H.8 p134 (FOUO-in-classified clause).
    //
    // Predicate-scan-vs-dataflow convention, identical to the
    // Pattern-C rows.
    const PATTERN_B_CLASS_FOUO_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const PATTERN_B_CLASS_FOUO_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // non-fdr-control-evicts-fouo: reads NON_IC, AEA, SAR
    // (the three "other control" surfaces whose presence is the
    // load-bearing trigger); writes DISSEM (FactRemove FOUO).
    // §H.8 p134 (FOUO-with-other-non-FD&R clause).
    //
    // CAT_DISSEM is intentionally NOT in `reads` even though the
    // predicate scans it for non-FD&R-other-than-FOUO tokens.
    // Declaring it would create a 2-row cycle with the existing
    // `capco/noforn-clears-fdr-family` row (which reads + writes
    // CAT_DISSEM; the scheduler accepts that as a 1-row self-edge
    // but rejects a 2-row reads-DISSEM/writes-DISSEM cycle).
    // The DISSEM-presence scan lives in the
    // `fouo_with_non_fdr_other_control_trigger` Custom predicate
    // body. Predicate-scan-vs-dataflow convention (same-axis
    // self-reference), identical to the Pattern-C rows.
    const PATTERN_B_NON_FDR_READS: &[marque_scheme::CategoryId] =
        &[CAT_NON_IC_DISSEM, CAT_AEA, CAT_SAR];
    const PATTERN_B_NON_FDR_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    vec![
        // ===============================================================
        // Pattern-B structural FOUO eviction (2 rows)
        // ===============================================================
        //
        // Pattern B is the second half of the §H.8 p134 FOUO
        // Precedence Rules for Banner Line Guidance. §H.8 p134
        // verbatim, "FOUO in an unclassified document" sub-clause:
        //   "FOUO is not conveyed in the banner line if the document
        //    is UNCLASSIFIED with FOUO and other dissemination
        //    control markings, excluding any FD&R markings."
        //
        // Two structural rows cover the matrix. The "other
        // dissemination control markings" set is the union of
        // CAT_DISSEM (non-FD&R IC
        // dissem tokens), CAT_NON_IC_DISSEM (LIMDIS / SBU / SSI /
        // LES / NODIS / EXDIS / NNPI / SbuNf / LesNf — every
        // non-IC dissem token), CAT_AEA (RD / FRD / TFNI / UCNI /
        // ATOMAL), and CAT_SAR (any program identifier). The
        // "non-FD&R" qualifier reduces to "not in the broad
        // `FDR_DOMINATORS` membership set" — see
        // `is_fdr_dissem_token` helper. Critically the helper
        // uses `Vocabulary::is_fdr_dissem` semantics (which
        // INCLUDES RELIDO) — NOT `is_fdr_dominator` (which
        // EXCLUDES RELIDO; that helper answers the conflict-
        // dominator question, not the FD&R-membership question).
        // See the `FDR_DOMINATORS` doc-comment in
        // `crates/capco/src/scheme/closure.rs` for the distinction.
        //
        // Row 1 `capco/classification-evicts-fouo` overlaps with
        // `capco/fouo-evicted-by-classified` (the Pattern-C
        // dedicated row) on their FactRemove target. The overlap is
        // intentional: both rows produce the same FactRemove[TOK_FOUO]
        // payload on a classified page carrying FOUO; the second
        // invocation hits `apply_fact_remove`'s `IntentInapplicable`
        // arm (token already absent) and is a per-intent no-op. The
        // two rows carry distinct citation threads: the Pattern-C row
        // cites only §H.8 p134's "FOUO in a classified document"
        // sub-clause; this row cites §H.8 p134's overall umbrella rule
        // that combines both the classified-strip AND the
        // unclassified-with-other-controls strip. Keeping them separate
        // preserves single-§-citation discipline at the per-row level
        // even though both quote the same §H.8 p134 passage.
        //
        // Verified against `crates/capco/docs/CAPCO-2016.md`
        // §H.8 p134 (full FOUO Precedence Rules passage).

        // Pattern-B row 1: `capco/classification-evicts-fouo`.
        //
        // §H.8 p134 (FOUO Precedence Rules for Banner Line
        // Guidance, classified-document sub-clause): "FOUO in a
        // classified document: When a classified document
        // contains portions of FOUO information, the FOUO marking
        // is not used in the banner line."
        //
        // Structurally identical to `capco/fouo-evicted-by-classified`
        // (the Pattern-C dedicated row); both produce the same
        // FactRemove[TOK_FOUO] payload. Carried as a separate Pattern-B
        // row so the §H.8 p134 umbrella rule — which contains BOTH the
        // classified-strip clause AND the unclassified-with-other-controls
        // strip clause — has a single Pattern-B citation thread distinct
        // from the Pattern-C dedicated row's narrower citation.
        // FactRemove is idempotent; the second invocation on a page where
        // the Pattern-C row already fired is a per-intent no-op via
        // `apply_fact_remove`'s `IntentInapplicable` arm.
        //
        // Verified against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/classification-evicts-fouo",
            capco(SectionLetter::H, 8, 134),
            CategoryPredicate::Custom(fouo_classified_trigger),
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_FOUO)],
                scope: Scope::Page,
            }),
            PATTERN_B_CLASS_FOUO_READS,
            PATTERN_B_CLASS_FOUO_WRITES,
        ),
        // Pattern-B row 2: `capco/non-fdr-control-evicts-fouo`.
        //
        // §H.8 p134 (FOUO Precedence Rules for Banner Line
        // Guidance, unclassified-document sub-clause): "FOUO is
        // not conveyed in the banner line if the document is
        // UNCLASSIFIED with FOUO and other dissemination control
        // markings, excluding any FD&R markings."
        //
        // The §H.8 p134 wording lists "other dissemination
        // control markings" without a classification gate — the
        // sub-clause heads its own bullet under "FOUO in an
        // unclassified document". On unclassified pages where
        // FOUO appears alongside any non-FD&R control on any
        // axis (CAT_DISSEM, CAT_NON_IC_DISSEM, CAT_AEA, CAT_SAR),
        // FOUO is stripped from the banner.
        //
        // The trigger predicate
        // `fouo_with_non_fdr_other_control_trigger` checks the
        // four axes: dissem non-FD&R-other-than-FOUO, non-IC
        // dissem non-empty, AEA non-empty, SAR set. AEA markings
        // (RD / FRD / TFNI / UCNI / ATOMAL) are atomic-energy
        // controls, not FD&R markings; SAR identifiers are
        // program markings, not FD&R markings; non-IC dissem
        // tokens (LIMDIS / LES / SBU / SSI / NODIS / EXDIS /
        // NNPI / SbuNf / LesNf) are non-FD&R by construction —
        // none appears in `FDR_DOMINATORS`.
        //
        // No classification gate: the §H.8 p134 sub-clause
        // applies at any classification level — at classified
        // levels, Pattern-B row 1 / the Pattern-C row also fires
        // and the two are idempotent siblings.
        //
        // Axis annotations: reads `[CAT_NON_IC_DISSEM, CAT_AEA,
        // CAT_SAR]` (three "other control" surfaces); writes
        // `[CAT_DISSEM]` (FactRemove FOUO). CAT_DISSEM is
        // intentionally NOT in `reads` even though the predicate
        // also scans it — the existing
        // `capco/noforn-clears-fdr-family` row reads + writes
        // CAT_DISSEM (the scheduler accepts that as a 1-row
        // self-edge); adding another reads-DISSEM/writes-DISSEM
        // row creates a 2-row cycle that Kahn's algorithm
        // rejects. The DISSEM-presence scan lives in
        // `fouo_with_non_fdr_other_control_trigger` (the Custom
        // predicate body). Predicate-scan-vs-dataflow convention,
        // identical to the Pattern-C rows.
        //
        // Verified against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/non-fdr-control-evicts-fouo",
            capco(SectionLetter::H, 8, 134),
            CategoryPredicate::Custom(fouo_with_non_fdr_other_control_trigger),
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_FOUO)],
                scope: Scope::Page,
            }),
            PATTERN_B_NON_FDR_READS,
            PATTERN_B_NON_FDR_WRITES,
        ),
    ]
}
