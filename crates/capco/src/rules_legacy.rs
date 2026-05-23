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

use crate::rules::rel_to_suggest::{
    SUGGEST_CONFIDENCE, SUGGEST_LOG_MARGIN, s004_candidate_covered_by_block, s004_edit_distance,
};
use crate::scheme::CapcoScheme;
use marque_ism::{
    CanonicalAttrs, CountryCode, MarkingClassification, MarkingType, Span, TokenKind, TokenSpan,
};
use marque_rules::{
    Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Phase,
    Rule, RuleContext, RuleId, RuleSet, Severity,
};
use marque_scheme::{Citation, FactRef, ReplacementIntent, Scope, SectionLetter, capco};


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
// Rule: capco:portion.fgi.ownership-trigraph-suggest (issue #545)
// ---------------------------------------------------------------------------

/// FGI ownership-trigraph-suggest rule.
///
/// Fires on shape-admitted-but-unregistered FGI ownership tokens like
/// `(S//FGI XX)` or `(S//FGI ZZZ)`. Today the parser admits any 2- or
/// 3-byte ASCII-upper token in the FGI ownership slot (plus the literal
/// `NATO` tetragraph and `EU`), then leaves registry validation to the
/// rule layer — the established parser/rule split documented at
/// [`marque_ism::CountryCode::admits_fgi_ownership_token`].
///
/// This is the FGI-ownership twin of [`RelToTrigraphSuggestRule`]
/// (S004, `capco:portion.dissem.rel-to-trigraph-suggest`). The
/// architectural shape is intentionally identical:
///
/// 1. Walk the country list for tokens that fail [`CapcoTokenSet::is_trigraph`]
///    (unregistered tokens; "admits=true ∧ is_trigraph=false" — but
///    admits=true is already proven by the parser having accepted
///    the token, so the rule only needs the trigraph predicate).
/// 2. For each unregistered token, find the highest-prior neighbor
///    within edit distance ≤2 whose log-prior delta clears
///    [`SUGGEST_LOG_MARGIN`].
/// 3. Skip when the candidate is already covered by the same FGI
///    ownership list (direct match or transitive coverage via
///    [`expand_tetragraph`](crate::vocab::expand_tetragraph) — issue
///    #439's coverage-exclusion semantic).
/// 4. Emit a `Severity::Suggest` `text_correction` at the precise
///    `TokenKind::FgiOwnershipTrigraph` byte span. No fix for the
///    no-candidate case (suggest a category-specific diagnostic
///    only — same as E073's no-fix template).
///
/// # Why a separate rule from S004
///
/// FGI ownership and REL TO release lists are different axes per
/// CAPCO-2016 §H.7 (ownership) vs. §H.8 (release). The reuse
/// surface here is the corpus-prior + edit-distance machinery, NOT
/// the axis semantics. Sharing a rule would conflate two distinct
/// per-marking concerns: a fix replacing an FGI ownership token
/// must NOT also alter the REL TO list, and vice versa.
///
/// # Behavioral divergence from S004
///
/// Non-3-letter unregistered ownership tokens (e.g. `XX`) emit a
/// `text_correction: None` advisory diagnostic rather than silently
/// passing. On the FGI ownership axis, a 2-byte shape-admitted token
/// is unambiguously a registry-miss (only `EU` is a registered 2-byte
/// FGI ownership identifier); on REL TO (S004's axis), 2-byte codes
/// are uncommon enough that S004's silent-skip is appropriate. The
/// calibrated edit-distance + corpus-prior candidate machinery is
/// length-3-only by construction — both rules share this gate.
///
/// # Authority
///
/// CAPCO-2016 §H.7 p122 (FGI ownership-list grammar; "`[LIST]`
/// pertains to one or more Register, Annex B trigraph country codes
/// or Register, Annex A tetragraph code(s), or Manual, Appendix B
/// NATO/NAC code(s)") + §A.6 p16 ("Multiple FGI trigraph country
/// codes or tetragraph codes must be separated by a single space").
/// Both citations re-verified against `crates/capco/docs/CAPCO-2016.md`
/// at authorship per Constitution VIII.
struct FgiOwnershipTrigraphSuggestRule;

/// Citations the FGI ownership-trigraph-suggest rule may emit. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate contract.
const FGI_OWNERSHIP_SUGGEST_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::H, 7, 122),
    capco(SectionLetter::A, 6, 16),
];

impl Rule<CapcoScheme> for FgiOwnershipTrigraphSuggestRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.fgi.ownership-trigraph-suggest")
    }
    fn name(&self) -> &'static str {
        "fgi-ownership-trigraph-suggest"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
    }
    /// Phase::Localized — each emitted `Diagnostic::text_correction`
    /// replaces a single `FgiOwnershipTrigraph` token with a
    /// corpus-derived canonical trigraph; span is one token. Matches
    /// S004's phase (the suggest-channel precedent).
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        FGI_OWNERSHIP_SUGGEST_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use crate::priors::{COUNTRY_CODE_BASE_RATES, country_code_log_prior};
        use marque_ism::CapcoTokenSet;
        use marque_ism::token_set::TokenSet;

        // FGI ownership-trigraph-suggest fires only on the acknowledged
        // form; `SourceConcealed` (the bare `FGI` banner) has no
        // country list to check, and `None` means no FGI marker
        // observed.
        let Some(marker) = attrs.fgi_marker.as_ref() else {
            return Vec::new();
        };
        let countries = marker.countries();
        if countries.is_empty() {
            return Vec::new();
        }

        // Collect the per-country `FgiOwnershipTrigraph` spans the
        // parser emitted. Per-CountryCode mapping is positional:
        // `parse_fgi_marker_with_spans` emits one span per
        // shape-admitted country in source order, matching the order
        // `FgiMarker::Acknowledged.countries` populates.
        // Scope the per-country `FgiOwnershipTrigraph` set to the byte
        // range of the chosen `FgiMarker` block-span before positional
        // indexing against `marker.countries()`. Per CAPCO §H.7 p122 a
        // marking carries at most one FGI category, and the parser's
        // overwrite semantics make `attrs.fgi_marker` correspond to the
        // LAST `FgiMarker` span pushed into `attrs.token_spans` — so
        // searching from the end with `rev().find(...)` locates the
        // block-span matching `attrs.fgi_marker`. Without this scoping,
        // if a future parser change or malformed input emitted multiple
        // FGI blocks in a single marking, positional indexing could
        // mis-anchor diagnostics onto spans from an earlier block whose
        // `FgiMarker` value was overwritten and is no longer reachable
        // through `attrs.fgi_marker`.
        let fgi_block_span = attrs
            .token_spans
            .iter()
            .rev()
            .find(|t| t.kind == TokenKind::FgiMarker)
            .map(|t| t.span);
        let ownership_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::FgiOwnershipTrigraph)
            .filter(|t| {
                fgi_block_span
                    .map(|block| t.span.start >= block.start && t.span.end <= block.end)
                    .unwrap_or(false)
            })
            .collect();

        let token_set = CapcoTokenSet;

        let mut diagnostics = Vec::new();
        for (idx, code) in countries.iter().enumerate() {
            let trigraph = code.as_str();
            // Skip the registered codes — they are the lawful CAPCO
            // ownership tokens per §H.7 p122. For this rule's surface
            // (FGI ownership), `is_trigraph` covers sovereign 3-letter
            // trigraphs, the EU 2-byte exception, and the literal
            // `NATO` tetragraph. The underlying `TRIGRAPHS` table also
            // carries `AUSTRALIA_GROUP`, but its 14-byte length is
            // rejected by `admits_fgi_ownership_token` upstream at the
            // parser shape gate, so AUSTRALIA_GROUP cannot reach this
            // rule via the FGI ownership context.
            if token_set.is_trigraph(trigraph) {
                continue;
            }

            let Some(span_token) = ownership_spans.get(idx) else {
                // Defensive: if the parser's `FgiOwnershipTrigraph`
                // tokens don't match `countries.len()` (future
                // parser drift), skip rather than emit a misaligned
                // diagnostic.
                continue;
            };
            let span = span_token.span;

            // Candidate-finding via corpus prior is restricted to
            // 3-letter trigraphs because that's where
            // `COUNTRY_CODE_BASE_RATES` provides the empirical
            // smoothed log-priors S004 calibrated against. 2-byte
            // codes (an unregistered `XX`, `YY`) and longer codes
            // have a different ambiguity profile — no calibrated
            // neighbor candidates today. The non-3-letter case
            // routes straight to the no-fix branch so the
            // diagnostic still surfaces (user-actionable signal),
            // it just doesn't carry a `text_correction`.
            //
            // The 3-letter case also takes the no-fix branch when
            // the entry has no corpus prior or no qualifying
            // neighbor — see `best` below.
            let best: Option<(&'static str, f32, usize)> = if trigraph.len() == 3
                && let Some(entry_log_prior) = country_code_log_prior(trigraph)
            {
                // Find the highest-prior neighbor within edit
                // distance 2. The tie-breaking ladder matches S004
                // byte-for-byte (log-prior > distance >
                // lexicographic). See S004's
                // `RelToTrigraphSuggestRule::check` for the full
                // ladder commentary.
                let mut best: Option<(&'static str, f32, usize)> = None;
                for cand in COUNTRY_CODE_BASE_RATES {
                    if cand.token == trigraph {
                        continue;
                    }
                    if cand.token.len() != 3 {
                        continue;
                    }
                    if cand.log_prior - entry_log_prior < SUGGEST_LOG_MARGIN {
                        continue;
                    }
                    let dist = s004_edit_distance(trigraph, cand.token);
                    if dist == 0 || dist > 2 {
                        continue;
                    }
                    let take = match best {
                        None => true,
                        Some((prev_token, prev_prior, prev_dist)) => {
                            if cand.log_prior > prev_prior {
                                true
                            } else if cand.log_prior < prev_prior {
                                false
                            } else if dist < prev_dist {
                                true
                            } else if dist > prev_dist {
                                false
                            } else {
                                cand.token < prev_token
                            }
                        }
                    };
                    if take {
                        best = Some((cand.token, cand.log_prior, dist));
                    }
                }
                best
            } else {
                None
            };

            match best {
                Some((candidate, _candidate_log_prior, _candidate_dist)) => {
                    // Issue #439 (shared with S004): skip when the
                    // candidate replacement is already covered by
                    // another entry in the same FGI ownership list
                    // — direct match or transitive coverage via
                    // a decomposable tetragraph. The author cannot
                    // have meant the candidate as a typo target when
                    // it's already a permitted ownership identifier.
                    if s004_candidate_covered_by_block(countries, candidate, idx) {
                        continue;
                    }

                    // Audit-content-ignorance (G13 / Constitution V Principle V)
                    // is structurally guaranteed by the closed
                    // `MessageTemplate::CorrectionsApplied` template + the
                    // closed `MessageArgs` struct — neither carries free-form
                    // bytes that could leak document content. The corresponding
                    // audit-content-ignorance test pins this at the `Diagnostic`
                    // surface.
                    diagnostics.push(Diagnostic::text_correction(
                        self.id(),
                        self.default_severity(),
                        span,
                        Message::new(
                            MessageTemplate::CorrectionsApplied,
                            MessageArgs {
                                category: Some(crate::scheme::CAT_FGI_MARKER),
                                ..MessageArgs::default()
                            },
                        ),
                        capco(SectionLetter::H, 7, 122),
                        candidate.to_owned(),
                        FixSource::BuiltinRule,
                        Confidence::strict(SUGGEST_CONFIDENCE),
                        None,
                    ));
                }
                None => {
                    // No corpus neighbor within margin/edit-distance
                    // → emit a no-fix diagnostic so the user still
                    // sees the unregistered token. Same shape as
                    // E073's no-fix template — the actionable signal
                    // is the diagnostic itself. UnrecognizedToken
                    // template + CAT_FGI_MARKER args keep audit
                    // surfaces content-ignorant.
                    diagnostics.push(Diagnostic::new(
                        self.id(),
                        self.default_severity(),
                        span,
                        Message::new(
                            MessageTemplate::UnrecognizedToken,
                            MessageArgs {
                                category: Some(crate::scheme::CAT_FGI_MARKER),
                                ..MessageArgs::default()
                            },
                        ),
                        capco(SectionLetter::H, 7, 122),
                        None,
                    ));
                }
            }
        }

        diagnostics
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
// of every `.message.contains` site to use `template()` / `args()`
// accessors against the closed `MessageTemplate` set. Tracked as part
// of the future inline-test-module re-enablement work.

// ---------------------------------------------------------------------------
// Rule: E071 — FGI explicit trigraph conflicts with concealment or acknowledgment
// ---------------------------------------------------------------------------
// CAPCO-2016 §H.7 p124: "Do not include country codes within the portion
// marks where the specific government(s) must be concealed."
//
// Detection: the classification `TokenSpan.text` starts with "FGI " followed
// by at least one trigraph (e.g. `"FGI DEU R"`). The parser drops the "FGI"
// token silently when building `FgiClassification`, so the raw text is the
// only reliable signal without adding a `had_fgi_prefix` field to the ISM
// crate (which would violate the Constitution VII scheme-adoption boundary).
//
// Case A (all countries ⊆ REL TO — acknowledged source): Error + fix.
//   `(//FGI DEU R//REL TO USA, DEU)` → `(//DEU R//REL TO USA, DEU)`
//
// Case B (`fgi.countries.is_empty()` — canonical unacknowledged form): valid.
//   `(//FGI S)` is correct; no diagnostic.
//
// Case C (countries ∩ REL TO = ∅ — no acknowledgment context): Warn + fix.
//   Primary fix: drop trigraphs → `(//FGI R)`.
//   Alternate Suggest: drop FGI → `(//DEU R)` (if the author meant acknowledged).
//   Optional NF Suggest: unacknowledged FGI is caveated → NOFORN is the
//   policy-coherent default (§B.3 Table 2 p21 Row 0 closure, Suggest only).
//
// Case D (partial REL TO overlap — ambiguous intent): Error, no auto-fix.
//   Suggest: acknowledge all (drop FGI, keep trigraphs).
//   Suggest: conceal all (drop trigraphs, keep FGI) + optional NF.

/// Case A confidence: countries fully ⊆ REL TO; unambiguous acknowledged-source fix.
const E071_ACK_ALL_CONFIDENCE: f32 = 1.0;
/// Case C primary confidence: conceal all (drop trigraphs).
const E071_CONCEAL_ALL_CONFIDENCE: f32 = 0.8;
/// Case C alternate confidence: acknowledge (drop FGI prefix).
const E071_CASE_C_ALT_CONFIDENCE: f32 = 0.6;
/// Case D suggest confidence: partial overlap, both paths offered.
const E071_CASE_D_CONFIDENCE: f32 = 0.6;
/// NOFORN companion confidence.
const E071_NF_CONFIDENCE: f32 = 0.7;

/// Overlap relationship between the FGI trigraphs in the classification
/// block and the REL TO country list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum E071Containment {
    /// All FGI countries present in REL TO (countries ⊆ rel_to).
    Full,
    /// No FGI country in REL TO (or REL TO is empty).
    Empty,
    /// Some but not all FGI countries in REL TO.
    Partial,
}

fn e071_rel_to_containment(countries: &[CountryCode], rel_to: &[CountryCode]) -> E071Containment {
    if rel_to.is_empty() {
        return E071Containment::Empty;
    }
    let matched = countries.iter().filter(|c| rel_to.contains(c)).count();
    if matched == 0 {
        E071Containment::Empty
    } else if matched == countries.len() {
        E071Containment::Full
    } else {
        E071Containment::Partial
    }
}

/// Drop the `"FGI"` token and any following whitespace.
/// `"FGI DEU R"` → `"DEU R"`, `"FGI  DEU R"` → `"DEU R"`.
/// Caller guarantees `tok_text` starts with `"FGI"` followed by whitespace.
fn e071_strip_fgi_prefix(tok_text: &str) -> String {
    tok_text["FGI".len()..].trim_start().to_owned()
}

/// Canonical concealed form: `"FGI {level}"` e.g. `"FGI R"`.
fn e071_concealed_form(level: marque_ism::Classification) -> String {
    format!("FGI {}", level.portion_str())
}

/// Canonical acknowledged form: `"{trigraphs} {level}"` e.g. `"DEU GBR R"`.
/// Sorts trigraphs alphabetically (canonical per §H.7 p124 / renderer).
fn e071_acknowledged_form(countries: &[CountryCode], level: marque_ism::Classification) -> String {
    let mut parts: Vec<&str> = countries.iter().map(|c| c.as_str()).collect();
    parts.sort_unstable();
    format!("{} {}", parts.join(" "), level.portion_str())
}

/// Rule **E071** — `fgi-explicit-with-trigraph`.
///
/// Fires when a non-US classification token carries both `FGI` (the
/// concealment marker) and explicit trigraph(s) — a contradiction per
/// CAPCO-2016 §H.7 p124. The REL TO country list resolves intent:
///
/// - Countries ⊆ REL TO → acknowledged source; FGI prefix is wrong. Fix.
/// - No REL TO overlap → unacknowledged source; trigraph(s) are wrong. Warn+Fix.
/// - Partial overlap → ambiguous. Error with two Suggests.
struct FgiExplicitWithTrigraphRule;

impl Rule<CapcoScheme> for FgiExplicitWithTrigraphRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.fgi.fgi-explicit-with-trigraph")
    }
    fn name(&self) -> &'static str {
        "fgi-explicit-with-trigraph"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::WholeMarking: the optional NOFORN `FactAdd` companion
    /// targets `Scope::Portion`, which crosses token boundaries.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use crate::scheme::TOK_NOFORN;
        use marque_ism::DissemControl;

        // Gate 1: portions only.
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        // Gate 2: must be an FGI classification.
        let Some(MarkingClassification::Fgi(fgi)) = &attrs.classification else {
            return vec![];
        };

        // Gate 3: locate the classification TokenSpan — carries the raw text.
        let Some(cls_tok) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
        else {
            return vec![];
        };

        let tok_text = cls_tok.text.as_str();

        // Gate 4: raw text must lead with the "FGI" token. The parser uses
        // `split_whitespace`, so any ASCII whitespace between "FGI" and the
        // first trigraph is admitted — match the same surface here rather than
        // a literal `starts_with("FGI ")` that silently misses tab/multi-space.
        // Bare `"FGI S"` is Case B — canonical unacknowledged form — handled
        // by Gate 5 (countries empty).
        if tok_text.split_whitespace().next() != Some("FGI") {
            return vec![];
        }

        // Gate 5: countries must be non-empty (parser populates only on
        // real trigraphs following the FGI prefix).
        if fgi.countries.is_empty() {
            return vec![];
        }

        let level = fgi.level;
        let citation = capco(SectionLetter::H, 7, 124);
        let mut out = Vec::new();

        let noforn_present = attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf));

        match e071_rel_to_containment(&fgi.countries, &attrs.rel_to) {
            E071Containment::Full => {
                // Case A: acknowledged source confirmed by REL TO.
                // FGI + trigraph + REL TO(trigraph) is contradictory.
                // Fix: drop "FGI " prefix from the classification token.
                let replacement = e071_strip_fgi_prefix(tok_text);
                out.push(Diagnostic::text_correction(
                    self.id(),
                    Severity::Error,
                    cls_tok.span,
                    Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
                    citation,
                    replacement,
                    FixSource::BuiltinRule,
                    Confidence::strict(E071_ACK_ALL_CONFIDENCE),
                    None,
                ));
            }
            E071Containment::Empty => {
                // Case C: no REL TO overlap — source must be concealed.
                // §H.7 p124: no trigraph should appear with concealed FGI.
                //
                // Primary (Warn): drop trigraphs → "FGI {level}".
                let conceal_form = e071_concealed_form(level);
                out.push(Diagnostic::text_correction(
                    self.id(),
                    Severity::Warn,
                    cls_tok.span,
                    Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
                    citation,
                    conceal_form,
                    FixSource::BuiltinRule,
                    Confidence::strict(E071_CONCEAL_ALL_CONFIDENCE),
                    None,
                ));
                // Alternate Suggest: drop FGI → "DEU R" (acknowledged path).
                let ack_form = e071_acknowledged_form(&fgi.countries, level);
                out.push(Diagnostic::text_correction(
                    self.id(),
                    Severity::Suggest,
                    cls_tok.span,
                    Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
                    citation,
                    ack_form,
                    FixSource::BuiltinRule,
                    Confidence::strict(E071_CASE_C_ALT_CONFIDENCE),
                    None,
                ));
                // Optional NF companion: unacknowledged FGI is caveated per IC
                // convention, so NOFORN is the policy-coherent default.
                if !noforn_present {
                    let nf_intent = FixIntent {
                        replacement: ReplacementIntent::FactAdd {
                            token: FactRef::Cve(TOK_NOFORN),
                            scope: Scope::Portion,
                        },
                        confidence: Confidence::strict(E071_NF_CONFIDENCE),
                        feature_ids: Default::default(),
                        message: Message::new(
                            MessageTemplate::RequiredByPresence,
                            MessageArgs::default(),
                        ),
                        source: FixSource::BuiltinRule,
                        migration_ref: None,
                    };
                    out.push(Diagnostic::with_fix_at_span(
                        self.id(),
                        Severity::Suggest,
                        ctx.candidate_span,
                        ctx.candidate_span,
                        Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
                        capco(SectionLetter::B, 3, 21),
                        nf_intent,
                    ));
                }
            }
            E071Containment::Partial => {
                // Case D: partial overlap — some trigraphs ack'd, some not.
                // Intent is ambiguous; no auto-fix.
                out.push(Diagnostic::with_fix(
                    self.id(),
                    Severity::Error,
                    cls_tok.span,
                    Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
                    citation,
                    None,
                ));
                // Suggest 1: acknowledge all (drop FGI, keep trigraphs).
                let ack_all = e071_acknowledged_form(&fgi.countries, level);
                out.push(Diagnostic::text_correction(
                    self.id(),
                    Severity::Suggest,
                    cls_tok.span,
                    Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
                    citation,
                    ack_all,
                    FixSource::BuiltinRule,
                    Confidence::strict(E071_CASE_D_CONFIDENCE),
                    None,
                ));
                // Suggest 2: conceal all (drop trigraphs, keep FGI).
                let conceal_all = e071_concealed_form(level);
                out.push(Diagnostic::text_correction(
                    self.id(),
                    Severity::Suggest,
                    cls_tok.span,
                    Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
                    citation,
                    conceal_all,
                    FixSource::BuiltinRule,
                    Confidence::strict(E071_CASE_D_CONFIDENCE),
                    None,
                ));
                // NF companion for the conceal-all path.
                if !noforn_present {
                    let nf_intent = FixIntent {
                        replacement: ReplacementIntent::FactAdd {
                            token: FactRef::Cve(TOK_NOFORN),
                            scope: Scope::Portion,
                        },
                        confidence: Confidence::strict(E071_NF_CONFIDENCE),
                        feature_ids: Default::default(),
                        message: Message::new(
                            MessageTemplate::RequiredByPresence,
                            MessageArgs::default(),
                        ),
                        source: FixSource::BuiltinRule,
                        migration_ref: None,
                    };
                    out.push(Diagnostic::with_fix_at_span(
                        self.id(),
                        Severity::Suggest,
                        ctx.candidate_span,
                        ctx.candidate_span,
                        Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
                        capco(SectionLetter::B, 3, 21),
                        nf_intent,
                    ));
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Rule: E073 — FGI invalid ownership token (category-specific diagnostic)
// ---------------------------------------------------------------------------

struct FgiInvalidOwnershipTokenRule;

/// Citations E073 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E073_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 7, 123)];

impl Rule<CapcoScheme> for FgiInvalidOwnershipTokenRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "marking.fgi.invalid-ownership-token")
    }
    fn name(&self) -> &'static str {
        "fgi-invalid-ownership-token"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::WholeMarking: reads `attrs.token_spans` for `Unknown`
    /// spans whose text leads with an `"FGI "` or long-form prefix.
    /// The diagnostic spans a sub-range of the FGI-marker block (one
    /// per invalid ownership token); WholeMarking is the conservative
    /// dispatch shape per D-7.2.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E073_AUTHORITIES
    }
    /// Emits one `Severity::Error` diagnostic per token in the FGI
    /// ownership slot that fails the
    /// [`CountryCode::admits_fgi_ownership_token`] shape gate
    /// (sovereign trigraphs + `EU` + literal `NATO`).
    ///
    /// # No fix
    ///
    /// No `text_correction` or `FixIntent` is offered. Invalid FGI
    /// ownership tokens have no single right replacement: `FVEY` is a
    /// 5-country coalition tetragraph (REL TO surface, not FGI
    /// ownership), and `DEUX` is shape-wrong rather than a typo for
    /// `DEU`. The category-specific diagnostic is itself the user-
    /// actionable signal.
    ///
    /// # Span anchoring
    ///
    /// The diagnostic span anchors at the offending token's byte range
    /// within the FGI-marker block. The parser packs the whole marker
    /// (`"FGI FVEY"`, `"FOREIGN GOVERNMENT INFORMATION DEUX"`) into a
    /// single `TokenKind::Unknown` `TokenSpan`; this rule splits the
    /// tail on whitespace and computes per-token offsets so the
    /// diagnostic points at the rejected token, not the whole marker.
    /// Audit-content-ignorance (Constitution V G13) is preserved: the
    /// span is a byte-offset locator into the source buffer, not a
    /// content payload.
    ///
    /// # Authority
    ///
    /// CAPCO-2016 §H.7 p123. The FGI Authorized Portion / Banner forms
    /// specify the ownership-token grammar: `[LIST]` is "one or more
    /// Register, Annex B trigraph country codes or Register, Annex A
    /// tetragraph code(s), or Manual, Appendix B NATO/NAC markings"
    /// per §G.1 p38 (Table 4 footnote on the §G.1 Register of
    /// Authorized Classification and Control Markings). The FGI
    /// ownership slot specifically admits sovereign trigraphs, the
    /// 2-byte `EU` exception, and the literal `NATO` tetragraph;
    /// distribution-list tetragraphs (`FVEY`, `ACGU`, `ISAF`, `CFIUS`)
    /// describe who may receive a marking, not who owns it (issue
    /// #280). Re-verified against `crates/capco/docs/CAPCO-2016.md` at
    /// authorship per Constitution VIII.
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let mut out = Vec::new();
        for tok in attrs.token_spans.iter() {
            if tok.kind != TokenKind::Unknown {
                continue;
            }
            let text = tok.text.as_str();
            // Strip the same prefixes the parser dispatches on. If
            // neither prefix is present this isn't an FGI marker — let
            // E008 own the generic-unknown surface.
            let (prefix_len, tail) = if let Some(rest) = text.strip_prefix("FGI ") {
                (4_usize, rest)
            } else if let Some(rest) = text.strip_prefix("FOREIGN GOVERNMENT INFORMATION ") {
                (31_usize, rest)
            } else {
                continue;
            };

            // Walk the tail and emit one diagnostic per invalid token,
            // anchored at the token's byte offset inside `tok.span`.
            // `split_whitespace` collapses runs; we recover offsets via
            // a manual byte cursor over `tail` to keep the span
            // precise.
            let span_start = tok.span.start;
            let base = span_start + prefix_len;
            let bytes = tail.as_bytes();
            let mut idx = 0_usize;
            let mut saw_token = false;
            while idx < bytes.len() {
                // Skip whitespace.
                while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
                    idx += 1;
                }
                if idx >= bytes.len() {
                    break;
                }
                let tok_start = idx;
                while idx < bytes.len() && !bytes[idx].is_ascii_whitespace() {
                    idx += 1;
                }
                let tok_end = idx;
                let candidate = &bytes[tok_start..tok_end];
                saw_token = true;
                if !CountryCode::admits_fgi_ownership_token(candidate) {
                    let abs_start = base + tok_start;
                    let abs_end = base + tok_end;
                    out.push(Diagnostic::new(
                        self.id(),
                        self.default_severity(),
                        Span::new(abs_start, abs_end),
                        Message::new(
                            MessageTemplate::UnrecognizedToken,
                            MessageArgs {
                                category: Some(crate::scheme::CAT_FGI_MARKER),
                                ..MessageArgs::default()
                            },
                        ),
                        capco(SectionLetter::H, 7, 123),
                        None,
                    ));
                }
            }
            // Forward-compat companion to the matching branch in
            // `is_fgi_invalid_ownership_token`: an empty tail (`"FGI "`
            // with no trailing tokens) is unreachable via the production
            // parser path because the block-walker trims input before
            // dispatch — `"FGI "` collapses to `"FGI"`, which
            // `parse_fgi_marker` admits as `SourceConcealed`. This
            // branch handles synthetic `TokenKind::Unknown` spans
            // (test-harness injection, out-of-tree consumers) and any
            // future parser change that allows an empty-tail FGI to
            // reach the rule layer. Anchor the diagnostic at the
            // trailing separator region rather than a zero-byte span
            // at end-of-token for a meaningful pointer.
            if !saw_token {
                let abs_start = span_start + prefix_len;
                let abs_end = tok.span.end;
                let span = if abs_end > abs_start {
                    Span::new(abs_start, abs_end)
                } else {
                    tok.span
                };
                out.push(Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    span,
                    Message::new(
                        MessageTemplate::UnrecognizedToken,
                        MessageArgs {
                            category: Some(crate::scheme::CAT_FGI_MARKER),
                            ..MessageArgs::default()
                        },
                    ),
                    capco(SectionLetter::H, 7, 123),
                    None,
                ));
            }
        }
        out
    }
}


// ---------------------------------------------------------------------------
// PR 10.A.1 Commit 4 — Citation cross-reference pins
// ---------------------------------------------------------------------------
//
// Live `#[cfg(test)]` module carrying the cross-reference
// secondary-passage guards that PR 10.A.1 Commit 2 dropped when
// collapsing the dual-passage `.contains("§...") + .contains("§...")`
// assertions on diagnostic citation strings into single typed-
// `Citation` `assert_eq!`s. The pre-#561 inline `mod tests`
// block that originally carried the `.contains(...)` test bodies
// was quarantined to `_disabled_tests.rs` (`#[cfg(any())]`-gated
// dead code, disposition tracked in issue #722) — the active test
// surface in this file is the cross-ref pins below plus the
// integration tests under `crates/capco/tests/`.
//
// # Why the consts live adjacent to each rule, not centralized
//
// Per the PR brief, the cross-references are rule-authoritative
// metadata. Living adjacent to each rule's struct (or, for the
// declarative E037/E038 family, adjacent to the corresponding rule
// struct at E039) makes "where is this rule's cross-reference?"
// answerable by reading the rule's source file alone. A future PR
// 10.A.2 `Rule::cited_authorities()` trait method (deferred per the
// brief) would migrate these consts to the trait surface.
//
// # CAPCO §-citation verification
//
// Every literal §-reference asserted below was re-verified against
// `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1 Commit 4 authorship
// per Constitution VIII propagation rule. See the per-const doc
// comments on `E005_CROSS_REFS` / `S003_CROSS_REFS` / `E037_CROSS_REFS`
// / `E038_CROSS_REFS` / `E039_CROSS_REFS` for the source passages.
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod citation_cross_refs_tests {
    use crate::rules::joint::S003_CROSS_REFS;
    use crate::rules::nodis_exdis::{E037_CROSS_REFS, E038_CROSS_REFS, E039_CROSS_REFS};
    use crate::rules::text_handling::E005_CROSS_REFS;
    use marque_scheme::{Citation, SectionLetter, capco};

    /// E005: secondary §D.1 p27 (banner categories exclude
    /// declassification — negative-inference complement to the
    /// primary §E.1 p31). PR 10.A.1 Commit 2 dropped the
    /// `.contains("§D.1 p27")` assertion in
    /// `e005_citation_points_at_specific_sections` (now quarantined
    /// in `_disabled_tests.rs`, disposition #722).
    #[test]
    fn e005_cross_refs_pin_section_d_1_p27() {
        let expected: Citation = capco(SectionLetter::D, 1, 27);
        assert!(
            E005_CROSS_REFS.contains(&expected),
            "E005_CROSS_REFS must include §D.1 p27; got: {:?}",
            E005_CROSS_REFS,
        );
    }

    /// S003: secondary §H.8 p150 (REL TO USA-first convention — the
    /// IC-convention analogue S003 ports to JOINT classifications).
    /// PR 10.A.1 Commit 2 dropped the `.contains("§H.8 pp 150")`
    /// assertion in `s003_citation_frames_as_convention_not_mandate`.
    #[test]
    fn s003_cross_refs_pin_section_h_8_p150() {
        let expected: Citation = capco(SectionLetter::H, 8, 150);
        assert!(
            S003_CROSS_REFS.contains(&expected),
            "S003_CROSS_REFS must include §H.8 p150; got: {:?}",
            S003_CROSS_REFS,
        );
    }

    /// E037: secondary §H.9 p174 (NODIS mutual-exclusion clause —
    /// mirror of the EXDIS clause at p172). PR 10.A.1 Commit 2
    /// dropped the `.contains("p174")` assertion in
    /// `e037_fires_when_nodis_and_exdis_coexist`.
    #[test]
    fn e037_cross_refs_pin_section_h_9_p174() {
        let expected: Citation = capco(SectionLetter::H, 9, 174);
        assert!(
            E037_CROSS_REFS.contains(&expected),
            "E037_CROSS_REFS must include §H.9 p174; got: {:?}",
            E037_CROSS_REFS,
        );
    }

    /// E038: secondary §H.9 p174 (NODIS "Requires NOFORN" — mirror
    /// of the EXDIS clause at p172). PR 10.A.1 Commit 2 dropped the
    /// `.contains("p174")` assertion in
    /// `e038_fires_on_nodis_without_noforn`.
    #[test]
    fn e038_cross_refs_pin_section_h_9_p174() {
        let expected: Citation = capco(SectionLetter::H, 9, 174);
        assert!(
            E038_CROSS_REFS.contains(&expected),
            "E038_CROSS_REFS must include §H.9 p174; got: {:?}",
            E038_CROSS_REFS,
        );
    }

    /// E039: secondary §H.9 p174 (NODIS authority for the
    /// REL-TO-not-authorized rule — mirror of the EXDIS clause at
    /// p172). PR 10.A.1 Commit 2 dropped the `.contains("p174")`
    /// assertion in `e039_fires_on_banner_rel_to_with_nodis_portion`
    /// AND the corresponding `e039_still_fires_after_engine_gap_close`
    /// regression-pin site (one const covers both sites).
    #[test]
    fn e039_cross_refs_pin_section_h_9_p174() {
        let expected: Citation = capco(SectionLetter::H, 9, 174);
        assert!(
            E039_CROSS_REFS.contains(&expected),
            "E039_CROSS_REFS must include §H.9 p174; got: {:?}",
            E039_CROSS_REFS,
        );
    }
}
