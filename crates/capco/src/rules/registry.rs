// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`CapcoRuleSet`] — the registered rule set returned by
//! `marque_capco::capco_rules()`.
//!
//! Retirement narratives for rules absent from this registry live at
//! `crates/capco/docs/archaeology/retirement-history.md` — they
//! enumerate the historical `E### / W### / S###` flat-string IDs that
//! retired in PR 3c.B Commit 6 (form-bucket migration), T035c-14
//! (W001), PR #470 (W002), T035b (E017-E019), PR #578 (13 declarative
//! wrappers), PR 3c.B Commit 7.3 (E058 class-floor migration),
//! PR 3c.B Commit 7.4 / PR 3b.E (E042-E051 SCI-per-system migration),
//! and PR #488 (S005/S006 collapse).

use marque_rules::{Rule, RuleSet};

use crate::rules::banner::BannerMatchesProjectedRule;
use crate::rules::dissem::{DeprecatedDissemRule, NonIcInClassifiedBannerRule};
use crate::rules::dissem_closure::RelidoImpliedByClosureRule;
use crate::rules::eyes::EyesOnlyConvertToRelToRule;
use crate::rules::fgi::{FgiInvalidOwnershipTokenRule, FgiOwnershipTrigraphSuggestRule};
use crate::rules::fgi_concealment::FgiExplicitWithTrigraphRule;
use crate::rules::form_mismatch::{BannerFormInPortionRule, PortionFormInBannerRule};
use crate::rules::joint::{JointDisunityCollapseRule, JointUsaFirstRule};
use crate::rules::nato::{BareNatoRequiresRelToRule, LegacyNatoCompoundRemarkRule};
use crate::rules::nodis_exdis::{
    NodisExdisClearsBannerRelToRule, NodisSupersedesExdisInPortionRule,
};
use crate::rules::rel_to::{
    BareRelPortionDivergenceRule, CollapseUniformRelPortionsRule, MissingUsaTrigraphRule,
    PreferTetragraphCollapseRule,
};
use crate::rules::rel_to_suggest::RelToTrigraphSuggestRule;
use crate::rules::rel_to_uncertainty::RelToOpaqueUncertainReductionSuggestRule;
use crate::rules::sci::{
    HcsBareAtConfidentialLegacyRemarkRule, HcsBareSuggestSubcompartmentRule,
    RsvBareRequiresCompartmentRule, SciCustomControlInfoRule,
};
use crate::rules::text_handling::{
    CorrectionsMapRule, DeclassifyMisplacedRule, UnknownTokenRule, XShorthandDateRule,
};
use crate::rules_declarative::{BareCanonicalCompoundRule, DeprecatedSciLongFormRule};
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
                // T035c-21 PR-B: NODIS/EXDIS page-level + portion-level
                // hand-written rules. E039 (REL TO clear), E041 (NODIS
                // supersedes EXDIS in portion). The page-level E040
                // (banner roll-up) is owned by `BannerMatchesProjectedRule`
                // below per the §H.9 p172 + p174 NODIS/EXDIS rollup
                // citations. The retired mutual-exclusion E037 and
                // require-NOFORN E038 fire through the engine bridge
                // (see archaeology/retirement-history.md PR #578).
                Box::new(NodisExdisClearsBannerRelToRule),
                Box::new(NodisSupersedesExdisInPortionRule),
                // PR 3b Sub-move A — banner-roll-up walker (T026a).
                // Subsumes the three retired literal rules E031 / E035
                // / E040 — emitted diagnostics carry per-row IDs for
                // audit-stream continuity. Authorities: §H.5 p101 (SAR),
                // §H.4 per-system (SCI), §H.9 p172 + p174 (NODIS / EXDIS).
                Box::new(BannerMatchesProjectedRule),
                // S003: joint-usa-first style rule. Info severity.
                // Follow-up from PR #97 (T035c-18) — §H.3 prescribes
                // pure alpha for JOINT, but IC convention puts USA
                // first. See JointUsaFirstRule doc.
                Box::new(JointUsaFirstRule),
                // S004: rel-to-trigraph-suggest — issue #235 / #186
                // PR-3. First consumer of the suggest-don't-fix
                // channel. Surfaces a `Severity::Suggest` diagnostic
                // when a REL TO trigraph has a corpus-rare prior and
                // a corpus-common 1- or 2-edit neighbor (e.g.,
                // `AUT` → `AUS?`). The fix is informational; the
                // engine never auto-applies a Suggest-severity
                // diagnostic regardless of confidence. Stays a
                // registered walker rule (corpus-derived replacement
                // computed during evaluation cannot fold into the
                // bridge's static `fix_intent_by_name` shape).
                Box::new(RelToTrigraphSuggestRule),
                // S005 (issue #206): REL TO membership-uncertain
                // reduction. `Phase::PageFinalization` Suggest-severity
                // diagnostic. Authorities: CAPCO-2016 §H.8 + §D.2 Table 3
                // rule 21.
                Box::new(RelToOpaqueUncertainReductionSuggestRule),
                // PR 9a T135a (issue #307 Group D): canonicalization
                // walker for deprecated SCI long-form tokens (HUMINT →
                // HCS, COMINT / SPECIAL INTELLIGENCE → SI, ECI <COMP> →
                // SI-<COMP>, EL / ENDSEAL <COMP> → SI-<COMP>,
                // KDK / KLONDIKE-<COMP> → TK-<COMP>). Catalog ordered
                // longer-prefix-first inside `rules_declarative.rs`.
                // Authority: CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85.
                Box::new(DeprecatedSciLongFormRule),
                // PR 9a (issue #307): class-specific bare-HCS / bare-RSV
                // rules per §H.4.
                //   E061  hcs-bare-at-confidential-legacy-remark  (§H.4 p62)
                //   E062  hcs-bare-suggest-subcompartment         (§H.4 p62)
                //   E063  rsv-bare-requires-compartment           (§H.4 p70)
                // E061 / E062 complement E010 with class-specific
                // guidance; E063 is net new (no prior coverage).
                Box::new(HcsBareAtConfidentialLegacyRemarkRule),
                Box::new(HcsBareSuggestSubcompartmentRule),
                Box::new(RsvBareRequiresCompartmentRule),
                // PR 9a T135a Commit 5 (issue #307): EYES / EYES ONLY →
                // REL TO conversion per §H.8 p157 + p158. NSA-only and
                // deprecated since the markings waiver expired 1 Oct 2017.
                // The fix emits a byte-precise text_correction on the
                // compound EYES block span; trigraphs carry forward to the
                // new REL TO list.
                Box::new(EyesOnlyConvertToRelToRule),
                // Issue #407 (rule E067): bare-canonical-compound
                // rewriter. Three legacy short-forms (bare CNWDI / NK /
                // EU in SCI position) have authoritative CAPCO-2016
                // canonical compound portion marks (RD-CNWDI per §H.6
                // p106; SI-NK per §H.4 p83; SI-EU per §H.4 p78). The
                // walker filters `TokenKind::Unknown`, matches against
                // a 3-row catalog, and emits `Severity::Fix`
                // `text_correction` diagnostics whose replacements are
                // hardcoded static literals (Constitution V audit
                // content-ignorance). Co-firing with E008
                // (`UnknownTokenRule`) is suppressed via
                // `is_bare_canonical_compound_form` so the user sees
                // only the actionable E067 diagnostic.
                Box::new(BareCanonicalCompoundRule),
                // PR 9c.1 T134 (rule E066): legacy NATO compound text
                // re-marking per CAPCO-2016 §G.2 p40 (Table 5: ARH
                // registers ATOMAL/BOHEMIA/BALK as standalone control
                // markings) + §H.7 p122 (ATOMAL worked example in AEA
                // axis) + §H.7 p127 (BOHEMIA worked example in SCI
                // axis). Catches the eight legacy portion-form patterns
                // (CTSA / CTS-A / CTS-B / CTS-BALK / NSAT / NS-A / NCA
                // / NC-A) plus the five banner-form equivalents, and
                // emits a Recanonicalize fix at confidence 1.0. The
                // parser canonicalizes the input attrs structure at
                // parse time; this rule surfaces the text-level
                // re-marking.
                Box::new(LegacyNatoCompoundRemarkRule),
                // Issue #677 — restore detection retired in PR 3c.B Commit 6.
                // The renderer's fix path was in place but no rule emitted
                // the `Recanonicalize` `FixIntent` that triggers it; the
                // two rules close that gap broadly (every form-pair in
                // `MARKING_FORMS` + US classification shorthand). One
                // diagnostic per offending marking, `Recanonicalize` at
                // marking scope. EYES suppression in the banner direction
                // (E064 owns the §H.8 p157 cross-axis conversion). See
                // the section header above each rule struct for the full
                // rationale + Constitution VIII §-verification.
                Box::new(PortionFormInBannerRule),
                Box::new(BannerFormInPortionRule),
                // PR 9c.2 (FR-048): bare NATO classification portion
                // appearing in a US-classified document should carry
                // `REL TO USA, NATO` per CAPCO-2016 §H.7 p127 Notional
                // Example 2 worked example
                // `(//CTS//BOHEMIA//REL TO USA, NATO)`. Suggest-channel
                // severity (S005/S006/S004 precedent — example-derived
                // citation, no "MUST" prose); users opt up via
                // `[rules] S007 = "warn"` if their org demands stronger
                // surfacing. The solely-NATO-document case is carved
                // out via `ProjectedMarking::is_solely_nato_classified`.
                Box::new(BareNatoRequiresRelToRule),
                // #559 close-out C1 (PM decision 2026-05-19): byte-
                // surfacing twin of `CLOSURE_RELIDO_SCI` /
                // `CLOSURE_RELIDO_US_CLASS`. Mirrors S007's
                // text-layer pattern (byte rule alongside an existing
                // lattice closure). Authority: CAPCO-2016 §H.8 p154 +
                // §D.2 Table 3 rule 17. The rule runs the closure to
                // detect whether RELIDO would be injected and emits a
                // `Severity::Suggest` `FactAdd(RELIDO, Scope::Portion)`
                // intent at confidence `S008_SUGGEST_CONFIDENCE = 0.85`
                // — matching S007's calibration precedent.
                Box::new(RelidoImpliedByClosureRule),
                // Issue #261: FGI classification with an explicit trigraph
                // when the source must be concealed, or with a trigraph that
                // contradicts the acknowledged REL TO countries.
                // Authority: CAPCO-2016 §H.7 p124.
                Box::new(FgiExplicitWithTrigraphRule),
                // Issue #501: invalid FGI ownership tokens. Replaces the
                // generic E008 "unrecognized token" Error on FGI-marker
                // spans whose ownership-list tail contains a token that
                // fails `CountryCode::admits_fgi_ownership_token` (e.g.,
                // `(S//FGI FVEY)`, `(S//FGI DEUX)`, `(S//FGI ACGU)`). The
                // E008 emission path suppresses co-firing on these spans
                // via `is_fgi_invalid_ownership_token`, so the user sees
                // only the category-specific E073 diagnostic. No fix is
                // offered (no single right replacement). Authority:
                // CAPCO-2016 §H.7 p123 (FGI Authorized Portion / Banner
                // forms; `[LIST]` grammar = Register Annex B trigraphs +
                // Annex A tetragraphs + Manual Appendix B NATO/NAC).
                Box::new(FgiInvalidOwnershipTokenRule),
                // Issue #545: FGI ownership-trigraph-suggest. Shape-
                // admitted-but-unregistered ownership tokens like
                // `(S//FGI XX)` / `(S//FGI ZZZ)`. Architectural twin
                // of S004 `RelToTrigraphSuggestRule`. Reuses the
                // corpus-prior + edit-distance machinery but operates
                // on the FGI ownership axis (`attrs.fgi_marker.countries()`,
                // `TokenKind::FgiOwnershipTrigraph` spans) rather than
                // the REL TO release axis. Stays a registered walker
                // (cannot fold into the constraint-catalog bridge) for
                // the same reason as S004 — the candidate replacement
                // is corpus-derived during evaluation. Authority:
                // CAPCO-2016 §H.7 p122 + §A.6 p16. Re-verified at
                // authorship per Constitution VIII.
                Box::new(FgiOwnershipTrigraphSuggestRule),
                // Issue #250: S009 prefer-tetragraph-collapse. Default Off
                // — tetragraph vs. explicit-member form is a classification-
                // authority style choice. Enable via `[rules] S009 = "suggest"`.
                // Authority: CAPCO-2016 §H.8 p150.
                Box::new(PreferTetragraphCollapseRule),
                // Issue #251: S010 collapse-uniform-rel-portions. Default Off.
                // When all portions with explicit REL TO carry the same list
                // as the banner, suggest the compact authorized form `REL`.
                // Phase::PageFinalization. Authority: CAPCO-2016 §H.8 p150.
                Box::new(CollapseUniformRelPortionsRule),
                // Issue #251: E072 bare-rel-portion-divergence. Default Warn.
                // When bare-REL portions and explicit-REL-TO portions with a
                // divergent list coexist on the same page.
                // Phase::PageFinalization. Authority: CAPCO-2016 §H.8 p150-151.
                Box::new(BareRelPortionDivergenceRule),
                // PR 4b-B Commit 9 (006 T112): W004 joint-disunity-
                // collapse-to-FGI per CAPCO-2016 §H.3 p57 + §H.7 p123
                // (CV-4 PR 4b-B 8th-pass updated from §H.3 p56 — the
                // migration trigger lives on p57's "Derivative Use"
                // bullets, not the p56 grammar block). P-3 (8th-pass):
                // reverted to Banner-only firing to avoid Mixed-page
                // false positives, then issue #461 moved to
                // `Phase::PageFinalization` so the rule observes the
                // page-level fixpoint snapshot exactly once per page —
                // see the `JointDisunityCollapseRule` doc-comment for
                // the layout-gap trade-off. Fires at PageFinalization
                // dispatch (per page at `MarkingType::PageBreak` BEFORE
                // the engine resets the per-page portion accumulator,
                // plus once at end-of-document); reads `ctx.page_portions`
                // for the `JointSet::DisunityCollapse` state. The
                // diagnostic message uses canonical CountryCode
                // trigraphs only (Constitution V Principle V G13).
                // Severity: Warn (configurable per .marque.toml).
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
