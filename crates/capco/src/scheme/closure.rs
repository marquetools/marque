// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO closure-rule catalog (residual) — `FDR_DOMINATORS` slice +
//! empty `CAPCO_CLOSURE_RULES` fn-pointer surface.
//!
//! # Post-#704 shape
//!
//! Issue #704 retired the residual `CLOSURE_REL_TO_USA_NATO` fn-pointer
//! rule that the post-PR-D state carried. The rule was a "default if
//! absent" semantic per §H.7 p127 + §G.2 Table 5 p40 — non-monotone
//! by §-spec design (§B.3 paragraph b p19's "NOT MARKED PREVIOUSLY"
//! gate). Non-monotone rules cannot live in a closure operator that
//! honors the `MarkingScheme::closure` trait's monotonicity contract;
//! the rule relocated to `crate::scheme::default_fill::row7_should_fill`
//! along with the other three "default if absent" rules (Rows 0/8/9
//! from the pre-#704 catalog).
//!
//! Post-#704 the fn-pointer trait surface (`CAPCO_CLOSURE_RULES`) is
//! an empty `&[]`. Six per-marking unconditional implications
//! (HCS-O / HCS-P[sub] / SI-G / TK-{BLFH,IDIT,KAND} per §H.4
//! marking templates) live in the bitmask `CLOSURE_TABLE` at
//! `closure_table.rs`; they fire unconditionally with no suppressor
//! so `close()` is purely additive at the bitmask layer and P3
//! monotonicity holds by construction.
//!
//! The `FDR_DOMINATORS` `TokenRef` slice below stays — it is the
//! source-of-truth FD&R-membership enumeration consumed by
//! `Vocabulary::is_fdr_dissem` in `vocabulary.rs` (independent of
//! the suppressor / default-fill architecture). The bitmask
//! `MASK_FDR_DOMINATORS` projection in `fact_bitmask.rs` mirrors
//! this slice for the `default_fill` predicates' FD&R-absent gates.
//!
//! # Historical note
//!
//! Pre-PR-D the catalog was a 10-row fn-pointer slice; PR-D moved 9
//! rows into the bitmask `CLOSURE_TABLE`. Pre-#704 the bitmask
//! catalog kept the same 10-row shape with `suppressor_mask` field
//! gating the four "default if absent" rows. Issue #704's refinement
//! moved the four default-if-absent rules out of `close()` entirely
//; the bitmask `CLOSURE_TABLE` now ships 6 rows.

use marque_scheme::{ClosureRule, TokenRef};

use super::*;

// ---------------------------------------------------------------------------
// Closure-rule catalog + family predicates
// ---------------------------------------------------------------------------

// --- Shared suppressor slices ---
//
// FD&R-dominator family: any of these present on a marking/page means an
// explicit FD&R decision exists; the implicit-default trio (Trio 1, 2, 3)
// should NOT fire. Per CAPCO-2016 §B.3.a p19 (canonical enumeration —
// "NOFORN, REL TO, RELIDO, or DISPLAY ONLY"), §B.3 Table 2 pp 21-22
// (scenario-summary table, derivative), and `marque-applied.md` §4.7.1.
//
// Includes:
//   - NOFORN (most restrictive FD&R, top of chain per §H.8 p145)
//   - RELIDO (deferred-release per SFDRA arrangement, §H.8 p154)
//   - DISPLAY ONLY (viewing-only FD&R, §H.8 p163)
//   - REL TO (any country list; `AnyInCategory` covers all partial lists,
//     §H.8 p150)
//   - EYES (US/[LIST] EYES ONLY is an FD&R marking at §H.8 p157)
//
// Note: LES-NF and SBU-NF are NOT included. They are non-IC dissem controls
// that carry NOFORN treatment via PageRewrite, not FD&R markers themselves.
// §B.3.a p19 is the authoritative enumeration of the FD&R set; §B.3 Table 2
// pp 21-22 is the per-scenario marking-summary table (derivative, not the
// definition).
//
// Algebraic note (re: `marque-applied.md` §4.7.3 has_fdr definition):
// §4.7.3 defines `has_fdr(x)` to include LES-NF / SBU-NF for the
// table-design-property monotonicity proof. The in-tree FDR_DOMINATORS
// omits them because (a) LES-NF and SBU-NF entail NOFORN through their
// own PageRewrite (so the operational behavior is preserved — when LES-NF
// is present, NOFORN is added via PageRewrite, and the Trio-1 row would
// then be suppressed by the post-PageRewrite NOFORN regardless), and
// (b) the §4.7.3 case-2 table-design property is preserved per-row because
// the suppressed cone {NOFORN} is exactly the fact that LES-NF / SBU-NF's
// PageRewrite would have added. The monotonicity proof holds via the
// downstream PageRewrite step rather than via FDR_DOMINATORS membership;
// the Trio-1 row is permitted to over-fire on bare-LES-NF / bare-SBU-NF
// because the PageRewrite supplies the suppressor fact downstream.
// `pub(crate)` so the `Vocabulary::is_fdr_dissem` override in
// `crates/capco/src/vocabulary.rs` and the bidirectional value-pin test
// (`mod fdr_dissem_pin` in the same file) can read this slice as the
// single source-of-truth.
//
// **Maintenance contract.** This slice and the neighboring
// `is_fdr_dominator` function answer *different* questions about
// the FD&R family, and the two enumerations are independent on
// purpose:
//   - `FDR_DOMINATORS` (this slice) enumerates **FD&R-set
//     membership** per §B.3.a p19 — the four canonical FD&R
//     markings (NOFORN / REL TO / RELIDO / DISPLAY ONLY) plus the
//     §H.8 p157 EYES legacy. `Vocabulary::is_fdr_dissem` walks
//     this slice and is the authoritative FD&R-membership API.
//   - `is_fdr_dominator` (below) enumerates **FD&R dominators
//     *over* RELIDO** for the `Constraint::ConflictsWithFamily`
//     dispatch on the RELIDO conflict catalog (E054/E055). It
//     deliberately **excludes RELIDO itself** because RELIDO-vs-
//     RELIDO is a tautology in the conflict family — there is no
//     such conflict to detect.
// The intersection of the two sets is "FD&R members that conflict
// with RELIDO" (NOFORN, DISPLAY ONLY, REL TO, EYES). The slice is
// the strict superset. Do not collapse them: a future refactor
// that delegates `is_fdr_dissem` through `is_fdr_dominator` will
// silently under-fire on RELIDO and is pinned against in
// `vocabulary.rs::fdr_dissem_pin::relido_admits_despite_is_fdr_dominator_excluding_it`.
//
// Adding a `Token` entry to this slice requires:
//   1. Considering whether the new token should also dominate
//      RELIDO. If yes, add a parallel arm to `is_fdr_dominator`'s
//      `matches!`. If no, leave `is_fdr_dominator` alone.
//   2. The `Vocabulary::is_fdr_dissem` override picks up the new
//      entry automatically — it iterates this slice directly.
// Adding an `AnyInCategory(CAT_X)` entry requires updating the
// override's per-category routing in `vocabulary.rs` because the
// override receives a single `TokenId` and dispatches through
// `capco_token_category` rather than passing a `TokenRef`.
pub(crate) static FDR_DOMINATORS: &[TokenRef] = &[
    TokenRef::Token(TOK_NOFORN),
    TokenRef::Token(TOK_RELIDO),
    TokenRef::Token(TOK_DISPLAY_ONLY),
    TokenRef::AnyInCategory(CAT_REL_TO),
    // EYES (USA/[LIST] EYES ONLY) is an FD&R marking per §H.8 p157.
    // It is parsed as `DissemControl::Eyes` (deprecated 2017-10-01 per
    // §H.8 p157 but still recognized for legacy-input compatibility), and
    // requires its own `TOK_EYES` sentinel + `satisfies_attrs` /
    // `iter_present_tokens` wiring — `CAT_REL_TO` fallthrough does NOT
    // cover it because `CAT_REL_TO` only checks `attrs.rel_to`. Including
    // EYES here ensures EYES-only portions correctly suppress the
    // implicit-NOFORN trio rows.
    TokenRef::Token(TOK_EYES),
];

// `FDR_OR_RELIDO_INCOMPAT` — retired in issue #704 along with the
// `phase2_closure_pin` test module that was its sole consumer. The
// suppressor architecture it backstopped (`CLOSURE_TABLE` Row 8 +
// `MASK_FDR_OR_RELIDO_INCOMPAT` projection) violated the closure
// operator's algebraic monotonicity property; the §H.8 p145 / §B.3.a
// p19 FD&R supersession semantics moved to
// `CapcoScheme::apply_supersession_overlays`. See the post-#704
// architecture note in `closure_table.rs`'s module doc-comment for
// the full rationale.

// Trio 1 (`CLOSURE_NOFORN_CAVEATED`) was retired in PR-D of the
// FactBitmask refactor (issue #371). The 20-trigger caveated-NOFORN
// row was bit-packed into `CLOSURE_TABLE` Row 0 with the
// `ROW0_NOFORN_IF_CAVEATED_TRIGGERS` mask (21 source `TokenRef`
// entries collapse to 20 atom bits — the `TOK_FGI_MARKER` +
// `AnyInCategory(CAT_FGI_MARKER)` redundant pair both project to
// `fact_bit::FGI_PRESENT`). The §-citation chain — universal §B.3
// p20 Note + §B.3 Table 2 p21 algebraic anchor + per-trigger Section
// H authorities — is preserved verbatim on the `CLOSURE_TABLE` Row 0
// `label` field and on the `ROW0_NOFORN_IF_CAVEATED_TRIGGERS`
// doc-comment in `closure_table.rs`.

// `CLOSURE_REL_TO_USA_NATO` and its `rel_to_usa_nato_derived_cone`
// helper were retired in issue #704's architectural refinement. The
// rule is a "default if absent" semantic per §H.7 p127 +
// §G.2 Table 5 p40 — non-monotone by §-spec design (the §B.3
// paragraph b p19 "NOT MARKED PREVIOUSLY" gate applies to NATO
// classifications too: when input already carries REL TO USA or
// NOFORN, the implicit REL TO USA, NATO default does NOT fire).
// Non-monotone rules cannot live in a closure operator that honors
// the `MarkingScheme::closure` trait's monotonicity contract; the
// rule relocated to `crate::scheme::default_fill::row7_should_fill`
// + the open-vocab NATO tetragraph tail. The §H.7 p127 worked
// example authority is preserved on `default_fill::ROW7_NATO_CLASS_TRIGGER`
// and on the module's per-row authority table.

// The six per-marking unconditional SCI implication rules
// (CLOSURE_HCS_O_IMPLIES_NF_OC, CLOSURE_HCS_P_SUB_IMPLIES_NF_OC,
// CLOSURE_SI_G_IMPLIES_OC, CLOSURE_TK_BLFH_IMPLIES_NF,
// CLOSURE_TK_IDIT_IMPLIES_NF, CLOSURE_TK_KAND_IMPLIES_NF) were
// retired in PR-D of the FactBitmask refactor (issue #371). The
// trigger sentinels (TOK_HCS_O / TOK_HCS_P_SUB / TOK_SI_G / TOK_TK_*)
// and their NOFORN / ORCON cones all live in the closed-vocab atom
// inventory, so each retired rule became a single positional row in
// `CLOSURE_TABLE` (Rows 1–6 in `closure_table.rs`). The §H.4
// per-marking authority chain — §H.4 p64 (HCS-O), §H.4 p68
// (HCS-P[sub]), §H.4 p80 (SI-G), §H.4 p87 / p91 / p95 (TK-BLFH /
// TK-IDIT / TK-KAND) — is preserved on the per-row `label` fields
// in `closure_table.rs`.

// ---------------------------------------------------------------------------
// Trio 2 RELIDO suppressor slices — retired in issue #704
// ---------------------------------------------------------------------------
//
// The two Trio 2 closure rules (`CLOSURE_RELIDO_SCI` +
// `CLOSURE_RELIDO_US_CLASS`) were retired in PR-D of the FactBitmask
// refactor (issue #371) into `CLOSURE_TABLE` Rows 8-9.
//
// The `RELIDO_US_CLASS_SUPPRESSORS` `TokenRef` slice that backstopped
// the bitmask `MASK_RELIDO_US_CLASS_SUPPRESSORS` projection retired
// with the `phase3_closure_pin` test module in issue #704 — both the
// slice and its sole consumer encoded the pre-#704 suppressor
// architecture that violated the closure operator's algebraic
// monotonicity property. The §H.8 p145 / §B.3.a p19 FD&R supersession
// semantics moved to `CapcoScheme::apply_supersession_overlays`. See
// the post-#704 architecture note in `closure_table.rs`'s module
// doc-comment for the full rationale.

/// The residual CAPCO closure-rule fn-pointer catalog.
///
/// Post-#704 this slice is empty. The four pre-#704 "default if absent"
/// rules (caveated → NOFORN, NATO → REL TO USA NATO, SCI → RELIDO,
/// US-class → RELIDO) relocated to `crate::scheme::default_fill`
/// (they are non-monotone by §-design and cannot honor the
/// `MarkingScheme::closure` trait's monotonicity contract). The six
/// per-marking unconditional rows (HCS-O / HCS-P[sub] / SI-G /
/// TK-{BLFH,IDIT,KAND}) live in the bitmask `CLOSURE_TABLE` —
/// `closure_inventory()` projects them onto `ClosureRuleMetadata`
/// for unified discovery without going through this slice.
///
/// The slice stays as an empty `&[]` to preserve the
/// `MarkingScheme::closure_rules` trait surface (per `decisions.md`
/// D18 — every scheme owns a `closure_rules()` method even when it
/// has none) and to keep a stable expansion seam: if a future CAPCO
/// rule ships an open-vocab cone that does not project onto a
/// closed-vocab bit (the original purpose of the fn-pointer surface),
/// it lands here.
pub(super) static CAPCO_CLOSURE_RULES: &[ClosureRule<CapcoScheme>] = &[];

// ---------------------------------------------------------------------------
// Issue #525 — FISA / RAWFISA / PROPIN as CAVEATED triggers
// ---------------------------------------------------------------------------

/// Issue #525 closure-row pins.
///
/// CAPCO §B.3 p20 Note: a portion carrying any IC dissem control is
/// caveated. PROPIN (§H.8 p148) and FISA (§H.8 p161) are IC dissem
/// controls; RAWFISA is the post-CAPCO-2016 unminimized variant of
/// FISA registered in ODNI `CVEnumISMDissem.xml` (no CAPCO-2016
/// prose section — see the RAWFISA paragraph below the CAVEATED
/// authority table). All three are structurally identical to
/// ORCON / RSEN / IMCON / DSEN already in
/// `CLOSURE_NOFORN_CAVEATED.triggers`. These pins assert each fires
/// the CAVEATED row's NOFORN cone in isolation AND that the
/// concurrent `CLOSURE_RELIDO_US_CLASS` row is suppressed (NOFORN
/// dominates RELIDO via the §H.8 p145 supersession overlay).
///
/// The pre-existing `every_fdr_dominator_suppresses_caveated_noforn_injection`
/// pin covers the suppressor side of the CAVEATED row; these three
/// pins are the trigger-side companions for the new entries.
///
/// Banner-roll-up of `(U//FISA)` as "considered RELIDO for purposes
/// of developing the overall banner line FD&R marking" (CAPCO-2016
/// `crates/capco/docs/CAPCO-2016.md` notional example, verified
/// 2026-05-18) is a PageContext-layer artifact — it is how a single
/// unclassified-FISA portion contributes to a page's FD&R
/// determination when classified portions are also present. It is
/// NOT a per-portion closure semantic and is out of scope for #525.
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod issue_525_caveated_dissem_pin {
    use super::*;
    use marque_ism::{CanonicalAttrs, Classification, DissemControl, MarkingClassification};
    use marque_scheme::MarkingScheme;

    /// Construct a `(S, dissem)` marking — Secret base with one IC
    /// dissem control on the dissem_us axis. Secret is chosen
    /// arbitrarily as a classified collateral level; the CAVEATED
    /// trigger is class-agnostic (the row has no class floor), so any
    /// classification level not in `FDR_DOMINATORS` would work.
    fn secret_with_dissem(d: DissemControl) -> CapcoMarking {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(Classification::Secret));
        a.dissem_us = Box::new([d]);
        CapcoMarking::new(a)
    }

    /// PROPIN is an IC dissem control per §H.8 p148. Caveated per
    /// §B.3 p20 Note → §B.3 Table 2 p21 default → NOFORN absent
    /// FD&R. `TOK_PROPIN` is in the `default_fill::ROW0_CAVEATED_TRIGGERS`
    /// mask post-#704 (the rule relocated from `CLOSURE_NOFORN_CAVEATED`
    /// to `default_fill::row0_should_fill`).
    ///
    /// Test uses `scheme.project(Scope::Page, ...)` to exercise the
    /// full post-#704 pipeline (close + default_fill + supersession
    /// overlay). Pre-#704 the rule fired inside `close()` directly;
    /// post-#704 it fires in `apply_default_fill` after `close()`
    /// converges. End-to-end behavior is preserved.
    #[test]
    fn caveated_fires_on_propin() {
        use marque_scheme::Scope;
        let scheme = CapcoScheme::new();
        let m = secret_with_dissem(DissemControl::Pr);
        let out = scheme.project(Scope::Page, std::slice::from_ref(&m));
        assert!(
            out.0.dissem_us.contains(&DissemControl::Nf),
            "PROPIN must trigger default-fill Row 0 → NOFORN injection. \
             Authority: §H.8 p148 (PROPIN as IC dissem control) + §B.3 p20 \
             Note + §B.3 Table 2 p21. dissem_us = {:?}",
            out.0.dissem_us
        );
        assert!(
            out.0.dissem_us.contains(&DissemControl::Pr),
            "PROPIN itself must be retained — Row 0's cone is `{{NOFORN}}` \
             only. dissem_us = {:?}",
            out.0.dissem_us
        );
        assert!(
            !out.0.dissem_us.contains(&DissemControl::Relido),
            "RELIDO must NOT appear: default-fill Row 0 fires NOFORN; \
             default-fill Row 9 (US-class → RELIDO) is suppressed by the \
             post-Row-0 NOFORN being in MASK_RELIDO_US_CLASS_SUPPRESSORS. \
             dissem_us = {:?}",
            out.0.dissem_us
        );
    }

    /// FISA is an IC dissem control per §H.8 p161. Caveated → NOFORN
    /// absent FD&R via post-#704 `default_fill::row0_should_fill`.
    #[test]
    fn caveated_fires_on_fisa() {
        use marque_scheme::Scope;
        let scheme = CapcoScheme::new();
        let m = secret_with_dissem(DissemControl::Fisa);
        let out = scheme.project(Scope::Page, std::slice::from_ref(&m));
        assert!(
            out.0.dissem_us.contains(&DissemControl::Nf),
            "FISA must trigger default-fill Row 0 → NOFORN injection. \
             Authority: §H.8 p161 (FISA as IC dissem control) + §B.3 p20 \
             Note + §B.3 Table 2 p21. dissem_us = {:?}",
            out.0.dissem_us
        );
        assert!(
            out.0.dissem_us.contains(&DissemControl::Fisa),
            "FISA itself must be retained. dissem_us = {:?}",
            out.0.dissem_us
        );
        assert!(
            !out.0.dissem_us.contains(&DissemControl::Relido),
            "RELIDO must NOT appear. dissem_us = {:?}",
            out.0.dissem_us
        );
    }

    /// RAWFISA is the post-CAPCO-2016 unminimized variant of FISA,
    /// registered in ODNI `CVEnumISMDissem.xml`. Same caveated →
    /// NOFORN semantic via post-#704 `default_fill::row0_should_fill`.
    #[test]
    fn caveated_fires_on_rawfisa() {
        use marque_scheme::Scope;
        let scheme = CapcoScheme::new();
        let m = secret_with_dissem(DissemControl::Rawfisa);
        let out = scheme.project(Scope::Page, std::slice::from_ref(&m));
        assert!(
            out.0.dissem_us.contains(&DissemControl::Nf),
            "RAWFISA must trigger default-fill Row 0 → NOFORN injection. \
             Authority: ODNI `CVEnumISMDissem.xml` (post-CAPCO-2016) + §B.3 \
             p20 Note + §B.3 Table 2 p21. dissem_us = {:?}",
            out.0.dissem_us
        );
        assert!(
            out.0.dissem_us.contains(&DissemControl::Rawfisa),
            "RAWFISA itself must be retained. dissem_us = {:?}",
            out.0.dissem_us
        );
        assert!(
            !out.0.dissem_us.contains(&DissemControl::Relido),
            "RELIDO must NOT appear. dissem_us = {:?}",
            out.0.dissem_us
        );
    }

    /// Source-of-truth pin: PROPIN / FISA / RAWFISA all trigger
    /// default-fill Row 0 end-to-end via `project()`. Post-#704 the
    /// caveated trigger list lives on `default_fill::ROW0_CAVEATED_TRIGGERS`
    /// (the pre-#704 `CLOSURE_TABLE[0].trigger_mask` retired with
    /// Row 0). The end-to-end behavioral evidence above already proves
    /// each token reaches default-fill's predicate; this test is the
    /// catalog-level drift pin — if a future edit drops PROPIN / FISA
    /// / RAWFISA from the default-fill trigger mask, this test fires.
    #[test]
    fn each_new_trigger_fires_default_fill_caveated_row_end_to_end() {
        use marque_scheme::Scope;
        let scheme = CapcoScheme::new();
        for (name, dissem) in [
            ("PROPIN", DissemControl::Pr),
            ("FISA", DissemControl::Fisa),
            ("RAWFISA", DissemControl::Rawfisa),
        ] {
            let m = secret_with_dissem(dissem);
            let out = scheme.project(Scope::Page, std::slice::from_ref(&m));
            assert!(
                out.0.dissem_us.contains(&DissemControl::Nf),
                "trigger {name} did NOT reach default-fill Row 0 — \
                 NOFORN missing from end-to-end project output. \
                 Authority: §B.3 p20 Note (IC dissem controls are \
                 caveated) + §B.3 Table 2 p21 (caveated-default \
                 obligation). dissem_us = {:?}",
                out.0.dissem_us,
            );
        }
    }

    /// Post-#704 FD&R supersession on the issue #525 trigger arms:
    /// `project(Page)` with PROPIN/FISA/RAWFISA + NOFORN converges to
    /// `{Trigger, NOFORN}` with no RELIDO. The closure() layer
    /// produces `{Trigger, NOFORN, RELIDO}` (Row 0 dedups NOFORN; Row
    /// 9 adds RELIDO unconditionally — closure is purely additive
    /// post-#704); the supersession overlay strips RELIDO per §H.8
    /// p145 at the project() boundary.
    ///
    /// Pre-#704 the test asserted `dissem_us.len()` stable at the
    /// closure() layer (the `MASK_FDR_DOMINATORS` suppressor on
    /// Row 0 + Row 9 prevented both implicit defaults from firing).
    /// The post-#704 reading is observationally equivalent at the
    /// project() level: input `{Trigger, NF}` → output `{Trigger,
    /// NF}`, length 2 → 2. The §-citations are preserved verbatim.
    ///
    /// Authority: §B.3 Table 2 p21 (caveated-default obligation
    /// drives Row 0 firing); §H.8 p145 (NOFORN-dominates supersession
    /// overlay strips RELIDO); §H.8 p148 (PROPIN), §H.8 p161 (FISA +
    /// RAWFISA) per the issue #525 trigger registration.
    #[test]
    fn project_resolves_new_trigger_plus_noforn_dominates_relido() {
        use marque_scheme::Scope;
        let scheme = CapcoScheme::new();
        for trigger in [
            DissemControl::Pr,
            DissemControl::Fisa,
            DissemControl::Rawfisa,
        ] {
            let mut a = CanonicalAttrs::default();
            a.classification = Some(MarkingClassification::Us(Classification::Secret));
            a.dissem_us = Box::new([trigger, DissemControl::Nf]);
            let before_len = a.dissem_us.len();
            let m = CapcoMarking::new(a);
            let out = scheme.project(Scope::Page, &[m]);
            assert_eq!(
                out.0.dissem_us.len(),
                before_len,
                "project must converge to length-stable `{{Trigger, NF}}` on \
                 {trigger:?} + NOFORN (closure adds RELIDO; overlay strips \
                 it per §H.8 p145); dissem_us = {:?}",
                out.0.dissem_us
            );
            assert!(
                out.0.dissem_us.contains(&DissemControl::Nf),
                "NOFORN must be retained on trigger {trigger:?}: project must \
                 not strip the seed NOFORN fact; dissem_us = {:?}",
                out.0.dissem_us
            );
            assert!(
                !out.0.dissem_us.contains(&DissemControl::Relido),
                "RELIDO must be stripped by §H.8 p145 overlay on \
                 {trigger:?} + NOFORN; dissem_us = {:?}",
                out.0.dissem_us
            );
        }
    }
}
