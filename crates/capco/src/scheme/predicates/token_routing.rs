// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CVE sentinel → [`CategoryId`] routing + the always-false
//! [`never_fires`] predicate used by Phase-3 stub `PageRewrite` rows.
//! Lifted from the monolithic `predicates.rs` per the issue #466
//! Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_scheme::{CategoryId, TokenId};

use super::super::*;

/// Map a sentinel CVE `TokenId` to its [`CategoryId`].
///
/// Used by [`<CapcoScheme as MarkingScheme>::category_of`] to route
/// `FactRef::Cve(id)` to the right marking-axis. Returns `None` for
/// sentinels not associated with a concrete category (the marker
/// sentinels like `TOK_IC_DISSEM`, `TOK_NON_US_CLASSIFICATION`,
/// `TOK_US_CLASSIFIED`, `TOK_FGI_MARKER` are excluded — they label
/// categorical predicates in the constraint catalog rather than
/// addressable atomic tokens). The engine surfaces `None` as
/// [`ApplyIntentError::UnknownToken`].
///
/// The mapping mirrors the existing per-token presence semantics in
/// `satisfies_attrs` so a rule emitting `FactRemove(TOK_X)` lands on
/// the same axis where `satisfies_attrs` would look for `X`.
pub(crate) fn capco_token_category(id: TokenId) -> Option<CategoryId> {
    // Sentinel IDs are declared in the const block above (lines 60+).
    // Keep the matches in declaration order so a reviewer can trace
    // the catalog by line position.
    match id {
        // CAT_DISSEM — IC dissemination controls
        TOK_NOFORN
        | TOK_RELIDO
        | TOK_DISPLAY_ONLY
        | TOK_ORCON
        | TOK_ORCON_USGOV
        // Stage D (T108c) additions — IC dissem controls needed for closure-rule
        // triggers (IMCON, DSEN, RSEN, FOUO per §4.7.1 implicit-NOFORN / implicit-RELIDO):
        | TOK_IMCON
        | TOK_DSEN
        | TOK_RSEN
        | TOK_FOUO
        // PR 4b-C Commit 1: PROPIN / FISA / RAWFISA live in
        // `attrs.dissem_us` (DissemControl::Pr / Fisa / Rawfisa).
        // §H.8 p148 + §H.8 p161. verified 2026-05-16.
        | TOK_PROPIN
        | TOK_FISA
        | TOK_RAWFISA
        // EYES (USA/[LIST] EYES ONLY) routes through the IC dissem axis.
        // The sentinel landed in PR 3.7 rev 3; the category routing
        // here is PR 3.7 rev 4 per Copilot review pass 4 (token_category
        // returning None would break any closure/intent/tooling path
        // that needs the host category for cone-addition or audit-note
        // projection).
        | TOK_EYES => Some(CAT_DISSEM),
        // CAT_NON_IC_DISSEM — non-IC dissemination controls.
        // PR 3c.B Sub-PR 8.F.2 added `TOK_SBU_NF` and `TOK_LES_NF` so
        // the Pattern A `capco/sbu-nf-implies-noforn` / `capco/les-nf-implies-noforn`
        // PageRewrites can route through this category.
        // Stage D (T108c) adds LIMDIS, LES, SBU, SSI as closure-rule trigger
        // sentinels (§4.7.1 implicit-NOFORN list).
        // PR 4b-C Commit 1: TOK_NNPI lives in `attrs.non_ic_dissem`
        // (NonIcDissem::Nnpi). Closes issue #407. verified 2026-05-16.
        TOK_NODIS | TOK_EXDIS | TOK_SBU_NF | TOK_LES_NF | TOK_LIMDIS | TOK_LES | TOK_SBU
        | TOK_SSI | TOK_NNPI => Some(CAT_NON_IC_DISSEM),
        // CAT_REL_TO — country codes in the dissemination context.
        // `TOK_USA` removes USA from the axis; the `TOK_REL_TO`
        // sentinel (PR 3c.B Sub-PR 8.D.2) clears the whole axis. Both
        // route through the same category so `apply_fact_remove`'s
        // CAT_REL_TO branch can discriminate.
        TOK_USA | TOK_REL_TO => Some(CAT_REL_TO),
        // CAT_AEA — atomic-energy markings. ATOMAL lives in the AEA
        // axis per CAPCO-2016 §H.7 p122 worked example
        // (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`). Issue #407:
        // `TOK_DCNI` (DOD UCNI, §H.6 p116) and `TOK_UCNI` (DOE UCNI,
        // §H.6 p118) are now distinct sentinels routed to the same
        // AEA axis where their `AeaMarking::DodUcni` /
        // `AeaMarking::DoeUcni` variants live.
        TOK_RD | TOK_FRD | TOK_TFNI | TOK_CNWDI | TOK_UCNI | TOK_DCNI | TOK_ATOMAL => {
            Some(CAT_AEA)
        }
        // CAT_SCI — sensitive compartmented information control systems.
        // BALK / BOHEMIA are NATO SAPs in the SCI category position per
        // §G.2 p40 + §H.7 p127 (rendered standalone, no SAR- prefix).
        // Issue #524 Phase 1: per-compartment SCI sentinels (SI-G,
        // HCS-O, HCS-P, TK-BLFH, TK-IDIT, TK-KAND) route to the same
        // CAT_SCI category — they address specific compartments under
        // their parent SCI control systems and are addressable in the
        // SCI axis alongside the bare control sentinel `TOK_HCS`.
        // Issue #524 Phase 2: `TOK_HCS_P_SUB` is the grammar-shape
        // sentinel for HCS-P with at least one sub-compartment (§H.4
        // p68 implication semantics differ from §H.4 p66 bare HCS-P).
        TOK_HCS | TOK_BALK | TOK_BOHEMIA | TOK_SI_G | TOK_HCS_O | TOK_HCS_P | TOK_HCS_P_SUB
        | TOK_TK_BLFH | TOK_TK_IDIT | TOK_TK_KAND => Some(CAT_SCI),
        // CAT_JOINT_CLASSIFICATION — JOINT classification marker
        TOK_JOINT => Some(CAT_JOINT_CLASSIFICATION),
        // CAT_CLASSIFICATION — overall classification level surface.
        // Issue #524 Phase 3: `TOK_US_COLLATERAL_CLASSIFIED` is a
        // grammar-shape sentinel firing on US collateral
        // classification (Restricted / Confidential / Secret /
        // TopSecret) — used as the trigger for `CLOSURE_RELIDO_US_CLASS`
        // to gate the implicit-RELIDO closure to collateral
        // classified content (§H.8 p154 carves out unclassified).
        TOK_RESTRICTED | TOK_US_COLLATERAL_CLASSIFIED => Some(CAT_CLASSIFICATION),
        // Sentinel marker tokens (used in catalog predicates, not as
        // addressable atomic tokens): no category mapping.
        _ => None,
    }
}

/// Always-false [`CategoryPredicate::Custom`] body used by every
/// Phase-3 stub `PageRewrite` row.
///
/// The rewrite's `reads` / `writes` axes are what the Kahn scheduler
/// consumes (T031–T032). Its trigger body does not participate in
/// Phase 3 runtime dispatch because `Engine::lint` does not route
/// aggregation through `scheme.project(Scope::Page, …)` — the
/// hand-coded [`PageContext`] aggregator handles roll-up. Pinning the
/// trigger to `false` makes that no-op explicit: any test or tool
/// that calls `scheme.project()` on today's `CapcoScheme` will see
/// these rewrites declare but never fire.
pub(crate) fn never_fires(_: &CapcoMarking) -> bool {
    false
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod phase3_token_routing_pin {
    //! Issue #524 Phase 3 — `TOK_US_COLLATERAL_CLASSIFIED` routing pin.
    //!
    //! Confirms the new sentinel routes to `CAT_CLASSIFICATION` via
    //! [`capco_token_category`], alongside the pre-existing
    //! `TOK_RESTRICTED`. A regression that re-routes one of these
    //! (e.g., accidentally to `CAT_DISSEM` or to `None`) breaks the
    //! `FactRef::Cve(TOK_US_COLLATERAL_CLASSIFIED)` apply-fact path
    //! that `CLOSURE_RELIDO_US_CLASS` walks during the closure's
    //! Kleene loop. Pinning here is the unit-test counterpart to the
    //! through-the-closure coverage in
    //! `closure.rs::phase3_closure_pin`.

    use super::*;

    /// `TOK_US_COLLATERAL_CLASSIFIED` routes to `CAT_CLASSIFICATION`.
    /// Authority: this is the trigger sentinel for
    /// `CLOSURE_RELIDO_US_CLASS` per CAPCO-2016 §B.3 Table 2 p21
    /// (default-RELIDO obligation) and §H.8 p154 (Unclassified
    /// carve-out — handled at the predicate level, not the routing
    /// level).
    #[test]
    fn tok_us_collateral_classified_routes_to_cat_classification() {
        assert_eq!(
            capco_token_category(TOK_US_COLLATERAL_CLASSIFIED),
            Some(CAT_CLASSIFICATION),
            "TOK_US_COLLATERAL_CLASSIFIED must route to CAT_CLASSIFICATION; \
             a regression in `capco_token_category`'s pattern arm would break the \
             `FactRef::Cve(TOK_US_COLLATERAL_CLASSIFIED)` apply-fact path"
        );
    }

    /// `TOK_RESTRICTED` continues to route to `CAT_CLASSIFICATION`
    /// alongside the new Phase 3 sentinel. Pins the shared-arm
    /// invariant — a future refactor that splits the arm must
    /// preserve both routings.
    #[test]
    fn tok_restricted_still_routes_to_cat_classification() {
        assert_eq!(
            capco_token_category(TOK_RESTRICTED),
            Some(CAT_CLASSIFICATION),
            "TOK_RESTRICTED must continue to route to CAT_CLASSIFICATION after the Phase 3 \
             addition of TOK_US_COLLATERAL_CLASSIFIED to the same match arm"
        );
    }

    /// `never_fires` returns false for any marking. Pins the
    /// invariant relied on by Phase-3 stub `PageRewrite` rows
    /// (per the function's doc-comment). Cheap, complete, and
    /// pins the contract one test below the user-facing
    /// `Engine::lint` path.
    #[test]
    fn never_fires_always_returns_false() {
        let m = CapcoMarking::new(marque_ism::CanonicalAttrs::default());
        assert!(
            !never_fires(&m),
            "never_fires must return false for any marking"
        );
    }
}
