// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO rule implementations — Layer 2 diagnostic intelligence.
//!
//! Each rule uses Layer 1 schema predicates (from generated/validators.rs) to
//! detect violations, then produces enriched diagnostics with fixes and
//! confidence. Phase 3 lands the full set of MVP rules with byte-precise
//! spans threaded through `CanonicalAttrs::token_spans`.
//!
//! Rule IDs follow the post-T044 (2026-05-22) 2-tuple wire-string form
//! `<scheme>:<surface>.<category>.<predicate>`. The current registered
//! set is enumerated in `crates/capco/README.md` and pinned at
//! `crates/capco/tests/post_3b_registration_pin.rs`.
//!
//! Retirement provenance for the historical `E### / W### / S###`
//! flat-string IDs lives at `crates/capco/docs/archaeology/`
//! (`retirement-history.md` for the per-rule retirement record,
//! `rule-id-cross-refs.md` for inline cross-refs grouped by live rule).
//! The T044 legacy-ID ↔ wire-string translation table lives at
//! `docs/refactor-006/legacy-rule-id-map.md`.

use crate::scheme::CapcoScheme;
use marque_rules::{Rule, RuleSet};


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
        use crate::rules_declarative::{BareCanonicalCompoundRule, DeprecatedSciLongFormRule};
        Self {
            // Retirement narratives for the rules absent from this list
            // (PR 3c.B Commit 6 form-bucket migration, T035c-14 W001,
            // PR #470 W002, T035b E017-E019, PR #578 13 wrappers,
            // PR 3c.B Commit 7.3 E058 class-floor migration,
            // PR 3c.B Commit 7.4 / PR 3b.E E042-E051 SCI-per-system
            // migration, PR #488 S005/S006 collapse) live at
            // `crates/capco/docs/archaeology/retirement-history.md`.
            rules: vec![
                Box::new(crate::rules::rel_to::MissingUsaTrigraphRule),
                Box::new(crate::rules::text_handling::DeclassifyMisplacedRule),
                Box::new(crate::rules::dissem::DeprecatedDissemRule),
                Box::new(crate::rules::text_handling::XShorthandDateRule),
                Box::new(crate::rules::text_handling::UnknownTokenRule),
                Box::new(crate::rules::text_handling::CorrectionsMapRule),
                Box::new(crate::rules::dissem::NonIcInClassifiedBannerRule),
                Box::new(crate::rules::sci::SciCustomControlInfoRule),
                // T035c-21 PR-B: NODIS/EXDIS page-level + portion-level
                // hand-written rules. E039 (REL TO clear), E041 (NODIS
                // supersedes EXDIS in portion). The page-level E040
                // (banner roll-up) is owned by `BannerMatchesProjectedRule`
                // below per the §H.9 p172 + p174 NODIS/EXDIS rollup
                // citations. The retired mutual-exclusion E037 and
                // require-NOFORN E038 fire through the engine bridge
                // (see archaeology/retirement-history.md PR #578).
                Box::new(crate::rules::nodis_exdis::NodisExdisClearsBannerRelToRule),
                Box::new(crate::rules::nodis_exdis::NodisSupersedesExdisInPortionRule),
                // PR 3b Sub-move A — banner-roll-up walker (T026a).
                // Subsumes the three retired literal rules E031 / E035
                // / E040 — emitted diagnostics carry per-row IDs for
                // audit-stream continuity. Authorities: §H.5 p101 (SAR),
                // §H.4 per-system (SCI), §H.9 p172 + p174 (NODIS / EXDIS).
                Box::new(crate::rules::banner::BannerMatchesProjectedRule),
                // S003: joint-usa-first style rule. Info severity.
                // Follow-up from PR #97 (T035c-18) — §H.3 prescribes
                // pure alpha for JOINT, but IC convention puts USA
                // first. See JointUsaFirstRule doc.
                Box::new(crate::rules::joint::JointUsaFirstRule),
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
                Box::new(crate::rules::rel_to_suggest::RelToTrigraphSuggestRule),
                // S005 (issue #206): REL TO membership-uncertain
                // reduction. `Phase::PageFinalization` Suggest-severity
                // diagnostic. Authorities: CAPCO-2016 §H.8 + §D.2 Table 3
                // rule 21.
                Box::new(crate::rules::rel_to_uncertainty::RelToOpaqueUncertainReductionSuggestRule),
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
                Box::new(crate::rules::sci::HcsBareAtConfidentialLegacyRemarkRule),
                Box::new(crate::rules::sci::HcsBareSuggestSubcompartmentRule),
                Box::new(crate::rules::sci::RsvBareRequiresCompartmentRule),
                // PR 9a T135a Commit 5 (issue #307): EYES / EYES ONLY →
                // REL TO conversion per §H.8 p157 + p158. NSA-only and
                // deprecated since the markings waiver expired 1 Oct 2017.
                // The fix emits a byte-precise text_correction on the
                // compound EYES block span; trigraphs carry forward to the
                // new REL TO list.
                Box::new(crate::rules::eyes::EyesOnlyConvertToRelToRule),
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
                Box::new(crate::rules::nato::LegacyNatoCompoundRemarkRule),
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
                Box::new(crate::rules::form_mismatch::PortionFormInBannerRule),
                Box::new(crate::rules::form_mismatch::BannerFormInPortionRule),
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
                Box::new(crate::rules::nato::BareNatoRequiresRelToRule),
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
                Box::new(crate::rules::dissem_closure::RelidoImpliedByClosureRule),
                // Issue #261: FGI classification with an explicit trigraph
                // when the source must be concealed, or with a trigraph that
                // contradicts the acknowledged REL TO countries.
                // Authority: CAPCO-2016 §H.7 p124.
                Box::new(crate::rules::fgi_concealment::FgiExplicitWithTrigraphRule),
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
                Box::new(crate::rules::fgi::FgiInvalidOwnershipTokenRule),
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
                Box::new(crate::rules::fgi::FgiOwnershipTrigraphSuggestRule),
                // Issue #250: S009 prefer-tetragraph-collapse. Default Off
                // — tetragraph vs. explicit-member form is a classification-
                // authority style choice. Enable via `[rules] S009 = "suggest"`.
                // Authority: CAPCO-2016 §H.8 p150.
                Box::new(crate::rules::rel_to::PreferTetragraphCollapseRule),
                // Issue #251: S010 collapse-uniform-rel-portions. Default Off.
                // When all portions with explicit REL TO carry the same list
                // as the banner, suggest the compact authorized form `REL`.
                // Phase::PageFinalization. Authority: CAPCO-2016 §H.8 p150.
                Box::new(crate::rules::rel_to::CollapseUniformRelPortionsRule),
                // Issue #251: E072 bare-rel-portion-divergence. Default Warn.
                // When bare-REL portions and explicit-REL-TO portions with a
                // divergent list coexist on the same page.
                // Phase::PageFinalization. Authority: CAPCO-2016 §H.8 p150-151.
                Box::new(crate::rules::rel_to::BareRelPortionDivergenceRule),
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
                Box::new(crate::rules::joint::JointDisunityCollapseRule),
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




// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Rule: E027 — SAR requires TS, S, or C classification (RETIRED)
// ---------------------------------------------------------------------------
//
// PR 3b.D (T026d) → PR 3c.B Commit 7.3: retired. The SAR floor
// invariant lives in `CapcoScheme`'s constraint catalog as the row
// `E058/SAR-classification-floor` (CAPCO §H.5). The engine's
// constraint-catalog bridge is the sole emitter; emitted diagnostics
// carry `Diagnostic.rule = "E058"` (audit-stream + config-override
// continuity with the retired walker convention). The legacy `E027`
// rule ID is NOT preserved (per project memory
// `feedback_pre_users_no_deprecation_phasing.md`: marque is
// pre-users — no severity-config back-compat).












// TOMBSTONED post-PR-#488: the `S006` references in this module's bodies
// (e.g., `s006_info_when_banner_equals_atom_intersection`,
// `s006_info_when_banner_is_proper_superset_of_atom_intersection`,
// `s006_emits_no_fix_and_no_fix_intent_pending_stage4_admonition_channel`,
// the `count_s005_or_s006` helper, and the
// `assert!(ids.contains(&"S006"))` call in `capco_rule_set_registers_all_rules`)
// describe the pre-#488 world where S005 and S006 were both registered.
// PR #488 retired S006 entirely (collapsed the Suggest/Info split into a
// single Suggest-severity S005 at `Phase::PageFinalization`). When this
// `#[cfg(any())]` module is rewritten (per the "PR 3c.B Commit 10: inline
// tests reading legacy FixProposal fields disabled pending rewrite" header
// below), update every S006 reference to reflect the post-#488 reality:
// the `s006_*` test names should be removed (the Info-branch contract
// no longer exists), `count_s005_or_s006` collapses to a plain S005
// counter, and the registration assertion drops the `S006` membership
// check. Per Constitution Principle V (audit-first compliance) the
// fabricated `FixProposal`-bearing test bodies were already dead at
// the cfg gate; this is a documentation tombstone, not a semantic claim.
// NOTE (PR 3c.2.C C7, reviewer R2 LOW): this `cfg(any())`-gated module
// has been dead since PR 3c.B Commit 10. Its test bodies call the
// pre-3c.2.C string `Message` API (`.message.contains(...)`, etc.) and
// would NOT compile under the post-3c.2.C closed-template `Message`
// shape if the gate were lifted — re-enabling requires a full rewrite

// ---------------------------------------------------------------------------


