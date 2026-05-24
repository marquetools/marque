// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`CapcoRuleSet`] — the registered rule set returned by
//! `marque_capco::capco_rules()`.

use marque_rules::{Rule, RuleSet};

use super::banner::BannerMatchesProjectedRule;
use super::dissem::{DeprecatedDissemRule, NonIcInClassifiedBannerRule};
use super::dissem_closure::RelidoImpliedByClosureRule;
use super::eyes::EyesOnlyConvertToRelToRule;
use super::fgi::{FgiInvalidOwnershipTokenRule, FgiOwnershipTrigraphSuggestRule};
use super::fgi_concealment::FgiExplicitWithTrigraphRule;
use super::form_mismatch::{BannerFormInPortionRule, PortionFormInBannerRule};
use super::joint::{JointDisunityCollapseRule, JointUsaFirstRule};
use super::nato::{BareNatoRequiresRelToRule, LegacyNatoCompoundRemarkRule};
use super::nodis_exdis::{NodisExdisClearsBannerRelToRule, NodisSupersedesExdisInPortionRule};
use super::recanonicalize::BareCanonicalCompoundRule;
use super::rel_to::{
    BareRelPortionDivergenceRule, CollapseUniformRelPortionsRule, MissingUsaTrigraphRule,
    PreferTetragraphCollapseRule,
};
use super::rel_to_suggest::RelToTrigraphSuggestRule;
use super::rel_to_uncertainty::RelToOpaqueUncertainReductionSuggestRule;
use super::sci::{
    HcsBareAtConfidentialLegacyRemarkRule, HcsBareSuggestSubcompartmentRule,
    RsvBareRequiresCompartmentRule, SciCustomControlInfoRule,
};
use super::sci_deprecated::DeprecatedSciLongFormRule;
use super::text_handling::{
    CorrectionsMapRule, DeclassifyMisplacedRule, UnknownTokenRule, XShorthandDateRule,
};
use crate::scheme::CapcoScheme;

/// The full CAPCO rule set returned by `marque_capco::capco_rules()`.
pub struct CapcoRuleSet {
    rules: Vec<Box<dyn Rule<CapcoScheme>>>,
}

impl Default for CapcoRuleSet {
    fn default() -> Self {
        Self::new()
    }
}

impl CapcoRuleSet {
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(MissingUsaTrigraphRule),
                Box::new(DeclassifyMisplacedRule),
                Box::new(DeprecatedDissemRule),
                Box::new(XShorthandDateRule),
                Box::new(UnknownTokenRule),
                Box::new(CorrectionsMapRule),
                Box::new(NonIcInClassifiedBannerRule),
                Box::new(SciCustomControlInfoRule),
                // NODIS/EXDIS page-level + portion-level hand-written
                // rules: REL TO clear, and NODIS supersedes EXDIS in a
                // portion. The page-level banner roll-up is owned by
                // `BannerMatchesProjectedRule` below per §H.9 p172 +
                // p174.
                Box::new(NodisExdisClearsBannerRelToRule),
                Box::new(NodisSupersedesExdisInPortionRule),
                // Banner-roll-up walker. Emitted diagnostics carry
                // per-row IDs. Authorities: §H.5 p101 (SAR), §H.4
                // per-system (SCI), §H.9 p172 + p174 (NODIS / EXDIS).
                Box::new(BannerMatchesProjectedRule),
                // joint-usa-first style rule (Info severity). §H.3
                // prescribes pure alpha for JOINT, but IC convention
                // puts USA first. See `JointUsaFirstRule` doc.
                Box::new(JointUsaFirstRule),
                // rel-to-trigraph-suggest. Surfaces a `Severity::Suggest`
                // diagnostic when a REL TO trigraph has a corpus-rare
                // prior and a corpus-common 1- or 2-edit neighbor (e.g.,
                // `AUT` → `AUS?`). The fix is informational; the engine
                // never auto-applies a Suggest-severity diagnostic
                // regardless of confidence. Stays a registered walker
                // because the corpus-derived replacement is computed
                // during evaluation and cannot fold into the declarative
                // bridge's static shape.
                Box::new(RelToTrigraphSuggestRule),
                // REL TO membership-uncertain reduction.
                // `Phase::PageFinalization` Suggest-severity diagnostic.
                // Authorities: CAPCO-2016 §H.8 + §D.2 Table 3 rule 21.
                Box::new(RelToOpaqueUncertainReductionSuggestRule),
                // Canonicalization walker for deprecated SCI long-form
                // tokens (HUMINT → HCS, COMINT / SPECIAL INTELLIGENCE →
                // SI, ECI <COMP> → SI-<COMP>, EL / ENDSEAL <COMP> →
                // SI-<COMP>, KDK / KLONDIKE-<COMP> → TK-<COMP>). Catalog
                // ordered longer-prefix-first inside `sci_deprecated.rs`.
                // Authority: CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85.
                Box::new(DeprecatedSciLongFormRule),
                // Class-specific bare-HCS / bare-RSV rules per §H.4:
                // bare HCS at Confidential legacy re-mark (§H.4 p62),
                // bare HCS suggest sub-compartment (§H.4 p62), bare RSV
                // requires compartment (§H.4 p70).
                Box::new(HcsBareAtConfidentialLegacyRemarkRule),
                Box::new(HcsBareSuggestSubcompartmentRule),
                Box::new(RsvBareRequiresCompartmentRule),
                // EYES / EYES ONLY → REL TO conversion per §H.8 p157 +
                // p158. NSA-only and deprecated since the markings waiver
                // expired 1 Oct 2017. The fix emits a byte-precise
                // text_correction on the compound EYES block span;
                // trigraphs carry forward to the new REL TO list.
                Box::new(EyesOnlyConvertToRelToRule),
                // bare-canonical-compound rewriter. Three legacy
                // short-forms (bare CNWDI / NK / EU in SCI position) have
                // authoritative CAPCO-2016 canonical compound portion
                // marks (RD-CNWDI per §H.6 p106; SI-NK per §H.4 p83;
                // SI-EU per §H.4 p78). The walker filters
                // `TokenKind::Unknown`, matches a 3-row catalog, and
                // emits `Severity::Fix` `text_correction` diagnostics
                // whose replacements are hardcoded static literals (audit
                // content-ignorance). Co-firing with `UnknownTokenRule`
                // is suppressed via `is_bare_canonical_compound_form`.
                Box::new(BareCanonicalCompoundRule),
                // Legacy NATO compound text re-marking per CAPCO-2016
                // §G.2 p40 (Table 5: ARH registers ATOMAL/BOHEMIA/BALK
                // as standalone control markings) + §H.7 p122 (ATOMAL
                // worked example in AEA axis) + §H.7 p127 (BOHEMIA worked
                // example in SCI axis). Catches the eight legacy
                // portion-form patterns (CTSA / CTS-A / CTS-B / CTS-BALK
                // / NSAT / NS-A / NCA / NC-A) plus the five banner-form
                // equivalents, and emits a Recanonicalize fix at
                // confidence 1.0. The parser canonicalizes the input
                // attrs at parse time; this rule surfaces the text-level
                // re-marking.
                Box::new(LegacyNatoCompoundRemarkRule),
                // Portion-form-in-banner / banner-form-in-portion
                // detection. The renderer's fix path emits the
                // `Recanonicalize` `FixIntent`; these two rules trigger
                // it broadly (every form-pair in `MARKING_FORMS` + US
                // classification shorthand). One diagnostic per offending
                // marking, `Recanonicalize` at marking scope. EYES is
                // suppressed in the banner direction
                // (`EyesOnlyConvertToRelToRule` owns the §H.8 p157
                // cross-axis conversion). See each rule's struct doc for
                // the full rationale + Constitution VIII §-verification.
                Box::new(PortionFormInBannerRule),
                Box::new(BannerFormInPortionRule),
                // Bare NATO classification portion appearing in a
                // US-classified document should carry `REL TO USA, NATO`
                // per CAPCO-2016 §H.7 p127 Notional Example 2
                // (`(//CTS//BOHEMIA//REL TO USA, NATO)`). Suggest-channel
                // severity (example-derived citation, no "MUST" prose);
                // users can opt up via `.marque.toml`. The
                // solely-NATO-document case is carved out via
                // `ProjectedMarking::is_solely_nato_classified`.
                Box::new(BareNatoRequiresRelToRule),
                // Byte-surfacing twin of the `default_fill` RELIDO
                // closure predicates: a text-layer rule alongside the
                // lattice closure. Authority: §B.3 Table 2 p21 (trigger
                // — the default-if-absent obligation); §D.2 Table 3 rule
                // 17 (FD&R precedence); §H.8 p154 (RELIDO marking
                // template). Runs the project pipeline to detect whether
                // RELIDO would be injected and emits a `Severity::Suggest`
                // `FactAdd(RELIDO, Scope::Portion)` intent at the
                // suggest-confidence calibration in
                // `rules/dissem_closure.rs`.
                Box::new(RelidoImpliedByClosureRule),
                // FGI classification with an explicit trigraph when the
                // source must be concealed, or with a trigraph that
                // contradicts the acknowledged REL TO countries.
                // Authority: CAPCO-2016 §H.7 p124.
                Box::new(FgiExplicitWithTrigraphRule),
                // Invalid FGI ownership tokens. Replaces the generic
                // "unrecognized token" Error on FGI-marker spans whose
                // ownership-list tail contains a token that fails
                // `CountryCode::admits_fgi_ownership_token` (e.g.,
                // `(S//FGI FVEY)`, `(S//FGI DEUX)`, `(S//FGI ACGU)`). The
                // unrecognized-token rule suppresses co-firing on these
                // spans via `is_fgi_invalid_ownership_token`, so the user
                // sees only the category-specific diagnostic. No fix is
                // offered (no single right replacement). Authority:
                // CAPCO-2016 §H.7 p123 (FGI Authorized Portion / Banner
                // forms; `[LIST]` grammar = Register Annex B trigraphs +
                // Annex A tetragraphs + Manual Appendix B NATO/NAC).
                Box::new(FgiInvalidOwnershipTokenRule),
                // FGI ownership-trigraph-suggest. Shape-admitted-but-
                // unregistered ownership tokens like `(S//FGI XX)` /
                // `(S//FGI ZZZ)`. Reuses the corpus-prior + edit-distance
                // machinery but operates on the FGI ownership axis
                // (`attrs.fgi_marker.countries()`,
                // `TokenKind::FgiOwnershipTrigraph` spans) rather than the
                // REL TO release axis. Stays a registered walker because
                // the candidate replacement is corpus-derived during
                // evaluation. Authority: CAPCO-2016 §H.7 p122 + §A.6 p16.
                Box::new(FgiOwnershipTrigraphSuggestRule),
                // prefer-tetragraph-collapse. Default Off — tetragraph vs.
                // explicit-member form is a classification-authority style
                // choice. Enable via `.marque.toml`. Authority:
                // CAPCO-2016 §H.8 p150.
                Box::new(PreferTetragraphCollapseRule),
                // collapse-uniform-rel-portions. Default Off. When all
                // portions with explicit REL TO carry the same list as
                // the banner, suggest the compact authorized form `REL`.
                // Phase::PageFinalization. Authority: CAPCO-2016 §H.8 p150.
                Box::new(CollapseUniformRelPortionsRule),
                // bare-rel-portion-divergence. Default Warn. When
                // bare-REL portions and explicit-REL-TO portions with a
                // divergent list coexist on the same page.
                // Phase::PageFinalization. Authority: CAPCO-2016 §H.8 p150-151.
                Box::new(BareRelPortionDivergenceRule),
                // joint-disunity-collapse-to-FGI per CAPCO-2016 §H.3 p57
                // + §H.7 p123 (the migration trigger lives on p57's
                // "Derivative Use" bullets, not the p56 grammar block).
                // `Phase::PageFinalization` so the rule observes the
                // page-level fixpoint snapshot exactly once per page (per
                // page at `MarkingType::PageBreak` before the engine
                // resets the per-page portion accumulator, plus once at
                // end-of-document); reads `ctx.page_portions` for the
                // `JointSet::DisunityCollapse` state. The diagnostic
                // message uses canonical CountryCode trigraphs only (audit
                // content-ignorance). Severity: Warn (configurable). See
                // the `JointDisunityCollapseRule` doc-comment for the
                // layout-gap trade-off.
                Box::new(JointDisunityCollapseRule),
            ],
        }
    }
}

impl RuleSet<CapcoScheme> for CapcoRuleSet {
    fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
        &self.rules
    }

    fn schema_version(&self) -> &'static str {
        crate::SCHEMA_VERSION
    }
}
