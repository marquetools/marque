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
use marque_ism::{
    CanonicalAttrs, CountryCode, MarkingClassification, MarkingType, Span, TokenKind, TokenSpan,
};
use marque_rules::{
    Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Phase,
    Rule, RuleContext, RuleId, RuleSet, Severity,
};
use marque_scheme::{
    Citation, FactRef, RecanonScope, ReplacementIntent, Scope, SectionLetter, capco,
};


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
// Rule: S004 — rel-to-trigraph-suggest (suggest-don't-fix channel)
// ---------------------------------------------------------------------------

/// S004: Surface a suggest-channel hint when a REL TO trigraph entry
/// is corpus-rare AND has a corpus-common neighbor within edit
/// distance 2.
///
/// # Authority and scope
///
/// Per CAPCO-2016 §H.8 p150 (REL TO grammar: Authorized Banner Line
/// Marking Title, Authorized Portion Mark) and §H.8 p151 (REL TO
/// "[USA, LIST]" syntax — "Register, Annex B trigraph country
/// codes"), REL TO entries are drawn from the CAPCO Register Annex
/// B trigraph code list. Every entry in `attrs.rel_to` has already
/// passed the strict-grammar trigraph check; the rule does not
/// invalidate any of them. The signal here is **statistical**:
/// `AUT` (Austria, ISO 3166-1 alpha-3) is a legitimate trigraph but
/// appears two orders of magnitude less often in real REL TO blocks
/// than `AUS` (Australia), and the two are 1 substitution apart.
/// When a low-prior entry has a high-prior 1- or 2-edit neighbor,
/// the entry might be correct (Austria really IS the recipient) or
/// might be a typo (`AUT` → `AUS`). The rule cannot tell which —
/// hence the suggest channel: emit a candidate replacement, do not
/// auto-apply.
///
/// # Severity
///
/// `Suggest` by default. The engine never auto-applies a fix
/// attached to a `Severity::Suggest` diagnostic regardless of
/// `confidence`, so the candidate replacement stays informational.
///
/// # Predicate
///
/// For each `CountryCode` in `attrs.rel_to`:
///
/// 1. Look up the entry's `country_code_log_prior`. Skip if absent
///    (decoder fallback is not in scope here — S004 only fires on
///    parsed-and-priored trigraphs).
/// 2. Iterate the corpus's country-code priors table. Find the
///    highest-prior code at edit distance 1 (or 2 for 3-letter
///    trigraphs only) from the entry, where the prior delta vs the
///    entry exceeds [`SUGGEST_LOG_MARGIN`].
/// 3. If such a neighbor exists, emit a `Severity::Suggest`
///    diagnostic with a `FixProposal` whose `replacement` is the
///    neighbor and `confidence` is a strict-built scalar
///    [`SUGGEST_CONFIDENCE`] (purely informational — `Suggest`
///    diagnostics never auto-apply).
///
/// # Coverage of #186 ambiguous fixtures
///
/// - `USB` → decoder PR-A (#238) handles. USB is not a trigraph; it
///   never reaches `attrs.rel_to`. S004 is silent.
/// - `AUT` → S004 fires, suggesting `AUS`.
///   `log_prior(AUS) - log_prior(AUT)` ≈ 4.36 nats, above
///   [`SUGGEST_LOG_MARGIN`].
/// - `ASU` → decoder PR-A handles. ASU is not a trigraph; never
///   reaches `attrs.rel_to`.
/// - `SA` → 2-character non-trigraph; same as USB / ASU, not in
///   `attrs.rel_to`. Decoder/parser path.
///
/// # Coverage exclusion (issue #439)
///
/// If the candidate replacement trigraph is **already covered** by
/// another entry in the same `attrs.rel_to` block — either directly
/// (the other entry equals the candidate) or transitively (the other
/// entry is a decomposable tetragraph whose
/// [`expand_tetragraph`](crate::vocab::expand_tetragraph) members
/// contain the candidate) — S004 stays silent for that entry. The
/// author's `AUT` cannot be a typo for `AUS` if `FVEY` (or `ACGU`, or
/// a direct `AUS`) already covers Australia in the same block:
/// `AUS` is *already* a permitted recipient, so duplicating it as
/// `AUT` would have produced redundant content rather than a typo.
/// The remaining hypothesis is "the author meant Austria"; S004
/// respects that and emits nothing.
///
/// The check is general over the ODNI ISMCAT Tetragraph Taxonomy —
/// `FVEY`, `ACGU`, `NATO`, `AUSTRALIA_GROUP`, and any other
/// `decomposable="Yes"` row are all consulted via the same table.
/// Atomic tetragraphs (`decomposable="No"` — `EU`, `GCCH`, `KFOR`,
/// …), deprecated entries (`decomposable="NA"`), and codes unknown
/// to both the taxonomy and `country_extensions.toml` return
/// `None` from `expand_tetragraph` and therefore cannot suppress
/// the diagnostic.
///
/// Authority: CAPCO-2016 §D.2 Table 3 Row 23 pp28–30 explicitly
/// licenses tetragraph-to-trigraph expansion for banner-line REL TO
/// roll-up — "Expansion of the TEYE, ACGU, and FVEY tetragraphs is
/// allowed for common country roll-up of banner line REL TO [USA,
/// LIST] marking". §H.8 p151 (REL TO Precedence Rules for Banner
/// Line Guidance) delegates roll-up semantics to §D.2 Table 3 by
/// reference. The suppression operationalizes that already-licensed
/// equivalence: if `FVEY` is in the block, its expanded members are
/// already permitted recipients of the same banner-rolled-up release
/// decision, so a "did you mean a member of that expansion?"
/// suggestion against a different rare trigraph is corpus noise.
/// The data source for the expansion (the ODNI ISMCAT
/// `decomposable="Yes"` rows) is described under
/// [`expand_tetragraph`](crate::vocab::expand_tetragraph).
///
/// # Constitution V audit-content-ignorance
///
/// The diagnostic message uses **only canonical token strings**
/// (the trigraph itself, the candidate trigraph, and English country
/// names from the [`COUNTRY_NAMES`](crate::vocab::COUNTRY_NAMES)
/// table) — no document content, no surrounding span text, no
/// user-provided fields. Verified by `s004_audit_content_ignorance`
/// in `crates/capco/tests/`.
///
/// # Reuse for #206
///
/// Issue #206 (REL TO opaque-uncertain reduction) wants the same
/// rendering channel without a candidate replacement: emit
/// `Severity::Suggest` with `fix: None`. The engine and renderer
/// both handle the missing-fix case cleanly (verified by
/// `s004_suggest_with_no_fix_round_trips_renderer`). #206 will land
/// as a separate rule that constructs `Diagnostic { severity:
/// Suggest, fix: None, .. }` directly.
struct RelToTrigraphSuggestRule;

/// Minimum log-prior delta for S004 to suggest a neighbor over the
/// observed entry. `4.0` nats ≈ `e^4.0` ≈ 55× odds ratio — the
/// neighbor is at least 55× more likely than the observed entry in
/// real REL TO contexts. Empirically calibrated against the AUT/AUS
/// pair (delta ≈ 4.36) so the canonical #186 fixture fires while
/// closer pairs (e.g., `USA`/`UKR` at delta ≈ 1.2 if it were ever
/// triggered) do not.
const SUGGEST_LOG_MARGIN: f32 = 4.0;

/// Strict-built confidence axis value for S004 fixes. The actual
/// number is informational only — the engine never auto-applies a
/// `Severity::Suggest` diagnostic's fix regardless of confidence.
/// Picked at `0.5` to make the audit-record posterior land in a
/// neutral middle bucket (a value at `1.0` would suggest "we're
/// sure" and confuse downstream tooling that filters by confidence).
///
/// **Config-override interaction**: setting `S004 = "fix"` in
/// `.marque.toml` is a no-op. The severity-override pass would
/// rewrite `Suggest → Fix`, but the engine's lint post-pass then
/// demotes any `Fix`-severity diagnostic with a sub-threshold
/// fix back to `Suggest` — and `0.5 < 0.95` (the default
/// confidence threshold) means S004's fix never clears the gate.
/// To get S004 fixes auto-applied a user would need both
/// `S004 = "fix"` AND a per-call `--confidence 0.5` (or lower)
/// override; for now the suggest-don't-fix channel is intentionally
/// hard advisory.
const SUGGEST_CONFIDENCE: f32 = 0.5;

/// Compute Levenshtein edit distance between two byte slices.
///
/// Trigraphs are short (≤ 3 bytes for the S004 use case) so the
/// O(m*n) two-row DP allocates two `Vec<usize>` of size `≤ 4` per
/// call — negligible. Inlined here rather than depending on
/// `marque-engine` (which `marque-capco` does not depend on).
fn s004_edit_distance(a: &str, b: &str) -> usize {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Issue #439: returns `true` when `candidate` (the trigraph the
/// caller would suggest) is already covered by some other entry in
/// `block` — either directly (another entry equals `candidate`) or
/// transitively (another entry is a tetragraph whose
/// [`expand_tetragraph`](crate::vocab::expand_tetragraph) members
/// contain `candidate`). Generic over the ODNI ISMCAT
/// `decomposable="Yes"` rows; atomic and unknown entries return
/// `None` from `expand_tetragraph` and therefore never cover.
///
/// The `self_idx` parameter excludes the rare entry itself from
/// the scan — `expand_tetragraph` returns `None` for a trigraph
/// like `AUT`, but skipping the self-index avoids both the lookup
/// and any future-edit pitfall if the table grows to include
/// trigraph rows.
///
/// # Naming
///
/// Originally `s004_candidate_covered_by_block(rel_to: &[CountryCode], ...)`.
/// Issue #545's `FgiOwnershipTrigraphSuggestRule` reuses the same
/// coverage-exclusion semantics on an FGI ownership list (a
/// `&[CountryCode]` from `attrs.fgi_marker.countries()`), so the
/// parameter was renamed `block` and the function generalized.
/// Both S004 (`attrs.rel_to`) and the FGI rule (`fgi_marker.countries()`)
/// pass their respective country-list slice; the helper is
/// shape-agnostic.
fn s004_candidate_covered_by_block(
    block: &[marque_ism::CountryCode],
    candidate: &str,
    self_idx: usize,
) -> bool {
    block.iter().enumerate().any(|(i, code)| {
        if i == self_idx {
            return false;
        }
        let s = code.as_str();
        if s == candidate {
            return true;
        }
        crate::vocab::expand_tetragraph(s).is_some_and(|members| members.contains(&candidate))
    })
}

/// Build an S004 diagnostic message for a given (rare, candidate)
/// trigraph pair.
///
/// Extracted from the rule body so each of the four `(Option,
/// Option)` country-name arms can be exercised directly in tests
/// — building real `CanonicalAttrs` to drive every arm requires
/// finding trigraph pairs that satisfy both the corpus-prior gap
/// AND the partial COUNTRY_NAMES coverage, which is brittle. The
/// helper lets us pin the formatting contract independently.
///
/// The output is content-ignorant per Constitution V: it only
/// references the input trigraph tokens (vocabulary) and the
/// canonical English country names (vocabulary), never any
/// document-source bytes.
fn s004_message(
    trigraph: &str,
    candidate: &str,
    entry_name: Option<&str>,
    candidate_name: Option<&str>,
) -> String {
    match (entry_name, candidate_name) {
        (Some(en), Some(cn)) => format!(
            "{trigraph:?} ({en}) is far less common in REL TO than \
             {candidate:?} ({cn}); did you mean {candidate:?}?"
        ),
        (None, Some(cn)) => format!(
            "{trigraph:?} is rare in REL TO blocks; did you mean \
             {candidate:?} ({cn})?"
        ),
        (Some(en), None) => format!(
            "{trigraph:?} ({en}) is rare in REL TO blocks; did you mean \
             {candidate:?}?"
        ),
        (None, None) => format!(
            "{trigraph:?} is rare in REL TO blocks; did you mean \
             {candidate:?}?"
        ),
    }
}

/// Citations S004 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const S004_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 8, 150)];

impl Rule<CapcoScheme> for RelToTrigraphSuggestRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.dissem.rel-to-trigraph-suggest")
    }
    fn name(&self) -> &'static str {
        "rel-to-trigraph-suggest"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
    }
    /// Phase::Localized: each emitted `Diagnostic::text_correction`
    /// replaces a single `RelToTrigraph` token with a corpus-derived
    /// canonical trigraph (e.g. `GRB → GBR`). Span is one token.
    /// `Severity::Suggest` means the engine never auto-promotes, but
    /// the phase declaration governs dispatch even for suggest-only
    /// rules.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        S004_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use crate::priors::{COUNTRY_CODE_BASE_RATES, country_code_log_prior};
        use crate::vocab::country_name;

        if attrs.rel_to.is_empty() {
            return Vec::new();
        }

        // Build a lookup from CountryCode → its `RelToTrigraph` token
        // span so we can attach the diagnostic to the exact source
        // bytes the user typed. Per-CountryCode mapping is positional:
        // the parser emits one `RelToTrigraph` token per `rel_to` entry
        // in source order.
        let trigraph_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToTrigraph)
            .collect();

        let mut diagnostics = Vec::new();
        for (idx, code) in attrs.rel_to.iter().enumerate() {
            let trigraph = code.as_str();
            // Only operate on 3-letter trigraphs. 2-letter codes (EU)
            // and longer codes (FVEY, AUSTRALIA_GROUP) have a different
            // ambiguity profile and would need their own calibration.
            if trigraph.len() != 3 {
                continue;
            }
            let Some(entry_log_prior) = country_code_log_prior(trigraph) else {
                continue;
            };

            // Find the highest-prior neighbor within edit distance 2.
            // Iterating the full `COUNTRY_CODE_BASE_RATES` table is
            // O(n) but the table is bounded (~340 codes) and the rule
            // fires once per `rel_to` entry. Acceptable.
            //
            // The triple `(token, log_prior, dist)` is what the
            // tie-breaking ladder reads — distance is tracked so a
            // log-prior tie deterministically picks the shorter-edit
            // candidate, and a same-distance tie picks the
            // lexicographically smaller token. Corpus-derived priors
            // tie exactly only when two entries share a build-time
            // smoothing floor, but pinning the order makes the rule's
            // output reproducible across `COUNTRY_CODE_BASE_RATES`
            // table reorderings.
            let mut best: Option<(&'static str, f32, usize)> = None;
            for cand in COUNTRY_CODE_BASE_RATES {
                if cand.token == trigraph {
                    continue;
                }
                if cand.token.len() != 3 {
                    continue;
                }
                if cand.log_prior - entry_log_prior < SUGGEST_LOG_MARGIN {
                    // Neighbor isn't substantially more likely — skip.
                    continue;
                }
                let dist = s004_edit_distance(trigraph, cand.token);
                if dist == 0 || dist > 2 {
                    continue;
                }
                // Pick the higher-prior candidate. On a log-prior
                // tie, prefer the shorter edit distance; on a
                // distance tie too, fall back to lexicographic
                // order on the token. Each rung of the ladder is a
                // strict comparison so the resolution is total.
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

            let Some((candidate, _candidate_log_prior, _candidate_dist)) = best else {
                continue;
            };

            // Issue #439: skip when the candidate replacement is
            // already covered by another entry in the same REL TO
            // block (direct trigraph match OR transitive coverage via
            // a decomposable tetragraph like FVEY / ACGU / NATO /
            // AUSTRALIA_GROUP). The author cannot have meant the
            // candidate trigraph as a typo target when it's already
            // a permitted recipient — the rare entry is either
            // intentional or a typo for something else entirely, and
            // S004's signal in that regime is at its weakest.
            if s004_candidate_covered_by_block(&attrs.rel_to, candidate, idx) {
                continue;
            }

            // Pull the matching span. If the parser's RelToTrigraph
            // tokens don't match `rel_to.len()` (defensive against a
            // future parser change), skip rather than emit a
            // misaligned diagnostic.
            let Some(span_token) = trigraph_spans.get(idx) else {
                continue;
            };
            let span = span_token.span;

            // Compose a content-ignorant message. The trigraph,
            // candidate, and country names are vocabulary-derived;
            // none of the surrounding document text appears.
            let message = s004_message(
                trigraph,
                candidate,
                country_name(trigraph),
                country_name(candidate),
            );

            // S004 suggests a trigraph swap (corpus-derived
            // canonical replacement, no fact-set delta). Encode as
            // a `text_correction` diagnostic. Even though
            // `apply_text_corrections` filters by C001, S004 emits
            // at `Severity::Suggest` so the engine's auto-apply
            // path correctly excludes it (the engine's Suggest
            // exclusion is a hard channel-cutoff). The text
            // correction carries the canonical trigraph for
            // renderer / UI display.
            let _ = (trigraph, message);
            diagnostics.push(Diagnostic::text_correction(
                self.id(),
                self.default_severity(),
                span,
                Message::new(MessageTemplate::CorrectionsApplied, MessageArgs::default()),
                capco(SectionLetter::H, 8, 150),
                candidate.to_owned(),
                FixSource::BuiltinRule,
                Confidence::strict(SUGGEST_CONFIDENCE),
                None,
            ));
        }

        diagnostics
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
// Rule: S005 — REL TO membership-uncertain reduction (issue #206; PR #488)
// ---------------------------------------------------------------------------
//
// What S005 detects. An `is_decomposable == None` tetragraph (NA-
// deprecated, taxonomy-absent, or org-fork extension code) drops out
// of the page-level REL TO atom-semantics intersection because at
// least one portion does not carry the code, AND there exist atoms in
// every-portion-without-the-code that the operator might have
// intended to release to via the dropped code's hypothetical
// membership. The rule cannot decide whether the producer drew on
// external membership data we don't have or simply mis-marked, so it
// surfaces the uncertainty for the operator to resolve.
//
// Phase. Phase::PageFinalization. The trigger is page-scoped — it
// computes the REL TO atom-semantics intersection across every
// portion on the page and emits one diagnostic per uncertain
// tetragraph that drops out of that intersection. The rule reads
// `ctx.page_portions` only; under PageFinalization dispatch the
// engine passes `CanonicalAttrs::default()` as `attrs`, so the rule
// neither reads nor depends on banner-witness state (pre-PR-#488
// the rule read `attrs.rel_to` to decide a Suggest-vs-Info branch;
// see "History — retired S006" below for why that branch was
// removed). The rule therefore must run once per page on the
// closed page-level fixpoint snapshot, not once per banner/CAB
// candidate. The pre-PR-#488 Phase::WholeMarking dispatch produced
// a documented false-negative on banner-first layouts (no closing
// banner ⇒ no Banner candidate ⇒ no firing surface) and a 6th-pass
// false-positive on intermediate snapshots when the rule briefly
// ran on Portion candidates. Phase::PageFinalization closes both —
// the engine dispatches S005 exactly once per page at every
// scanner-emitted `MarkingType::PageBreak` BEFORE the per-page
// accumulator reset, plus once at end-of-document.
//
// Severity / fix. Severity::Suggest with no fix. The ambiguity is not
// resolvable from in-tree data — only the producer's external
// membership data can settle it. The engine never auto-applies a
// Suggest-severity diagnostic regardless of confidence
// (`Engine::fix_inner` excludes `Severity::Suggest` from the apply
// gate), so the no-fix shape is the safest and most honest signal.
//
// History — retired S006. Pre-PR-#488 the rule was a Suggest/Info
// pair: S005 emitted when the banner was inconsistent or missing
// (active validation), S006 emitted at Info severity when the banner
// was consistent with atom-semantics (`expected ⊆ banner_atomic`).
// The two-rule split was an engine-workaround, NOT §-grounded —
// CAPCO-2016 §H.8 treats REL TO via pure set-membership language and
// §D.2 Table 3 rule 21 (the roll-up intersection law) applies
// uniformly without distinguishing "active validation" from
// "consistent case." The split existed because
// `marque_engine::Engine::lint` overwrites every emitted diagnostic's
// severity with the rule's configured/default severity, so a single
// rule could not stably emit at two severities. PR #488 collapsed the
// pair to a single Suggest-severity rule. The eventual admonition
// channel (deferred per
// `specs/006-engine-rule-refactor/followups/admonition-channel.md`)
// will restore per-emission severity if a future need arises; the
// collapse-now matches that eventual end state.
//
// Authority. CAPCO-2016 §H.8 (REL TO list grammar — syntax and
// tetragraph definition) + ODNI ISMCAT
// V[`marque_ism::ISMCAT_TETRA_VERSION`] Tetragraph Taxonomy (member-
// country expansion). The ISMCAT taxonomy is the authoritative
// member-country source per ODNI; §H.8 itself does not delegate to
// ISMCAT (the string "ISMCAT" does not appear in
// `crates/capco/docs/CAPCO-2016.md`). The two authorities compose;
// they are not in a delegating relationship. `S005_CITATION` below
// uses an additive `+` form; read it as "§H.8 (grammar) plus ISMCAT
// (expansion data)", not as "§H.8 delegating to ISMCAT."
//
// Citations explicitly NOT load-bearing for S005:
//   - §D.2 Table 3 rule 23 (TEYE/ACGU/FVEY-only intersection special
//     case) — strictly outside S005's general-tetragraph case.
//   - §H.8 p151 ("Commingling Rule(s) Within a Portion" — per-portion,
//     not page-level roll-up).
// Reviewers verifying citation chains for S005 should not follow
// either of these as authority for the rule's behavior.
//
// Audit-content-ignorance per Constitution V Principle V G13. The
// diagnostic message embeds canonical token strings (CAPCO REL TO
// codes that survived parsing — closed vocabulary, never document
// text) plus verbatim ODNI taxonomy `<Description>` text from
// `lookup_tetragraph_provenance`. No input bytes from the document
// being linted are interpolated. §-citation re-verified 2026-05-17
// against `crates/capco/docs/CAPCO-2016.md`.
struct RelToOpaqueUncertainReductionSuggestRule;

/// Format the `{state}` text for an S005 diagnostic. Pulls from the
/// build-time-generated [`marque_ism::TetragraphProvenance`] table so
/// the description text stays stable across taxonomy revisions and
/// the `is_decomposable` runtime API stays single-purpose.
///
/// The match arms cover the four `is_decomposable == None` shapes
/// the V2022-NOV taxonomy actually produces, plus the
/// taxonomy-absent case. A hypothetical future revision that maps
/// some code to `Some(_)` won't reach this function (the rule's
/// outer guard filters on `is_decomposable == None`); the defensive
/// fallback exists so a future taxonomy revision that introduces a
/// new `(decomposable, membership_shape)` pair still produces a
/// readable diagnostic instead of panicking.
fn s005_state_text(code: &str) -> String {
    use marque_ism::{ISMCAT_TETRA_VERSION, lookup_tetragraph_provenance};
    match lookup_tetragraph_provenance(code) {
        None => "absent (org-fork extension or unknown code)".to_owned(),
        Some(p) => match (p.decomposable, p.membership_shape) {
            ("NA", "Suppressed") => format!(
                "deprecated, membership suppressed \
                 (NA-Suppressed in V{ISMCAT_TETRA_VERSION})"
            ),
            ("NA", "Description") => {
                let desc = p.description.unwrap_or("(no description text)").trim();
                format!(
                    "deprecated, refer to original classification authority \
                     per ODNI: \"{desc}\""
                )
            }
            ("NA", shape) if shape.starts_with("Members") => {
                // Members(recursive) — BHTF in V2022-NOV.
                "deprecated, recursive membership (out of scope for v1)".to_owned()
            }
            (decomp, shape) => format!(
                "ISMCAT V{ISMCAT_TETRA_VERSION} taxonomy: \
                 decomposable={decomp:?}, membership_shape={shape:?}"
            ),
        },
    }
}

/// Expand a slice of `CountryCode` entries into a flat set of
/// atomic country-code strings. Decomposable tetragraphs (FVEY,
/// ACGU, NATO, …) expand to their constituent trigraphs;
/// opaque atoms (EU, KFOR, MNFI, …) pass through unchanged.
///
/// Lifetime: the returned set borrows from the input slice for
/// passthrough atoms and from `'static` storage for tetragraph
/// expansions. Both narrow into `&'a str` cleanly.
fn s005_expand_atomic(rel_to: &[marque_ism::CountryCode]) -> std::collections::BTreeSet<&str> {
    use crate::vocab::expand_tetragraph;
    let mut set = std::collections::BTreeSet::new();
    for code in rel_to.iter() {
        let s = code.as_str();
        if let Some(members) = expand_tetragraph(s) {
            for &m in members {
                set.insert(m);
            }
        } else {
            set.insert(s);
        }
    }
    set
}

/// Render an atomic country-code set as a `, `-joined string with
/// `USA` first (per CAPCO §H.8) and the rest alphabetical.
fn s005_render_set(set: &std::collections::BTreeSet<&str>) -> String {
    let mut codes: Vec<&str> = set.iter().copied().collect();
    if let Some(pos) = codes.iter().position(|s| *s == "USA") {
        if pos != 0 {
            let usa = codes.remove(pos);
            codes.insert(0, usa);
        }
    }
    codes.join(", ")
}

/// Run the S005 trigger analysis on the page-level fixpoint snapshot
/// and emit one Suggest-severity diagnostic per uncertain code that
/// dropped out of the intersection and had a non-empty "other codes"
/// candidate set.
///
/// Called by `RelToOpaqueUncertainReductionSuggestRule::check` under
/// `Phase::PageFinalization`. The `_attrs` parameter is unused — the
/// engine passes `CanonicalAttrs::default()` for PageFinalization
/// dispatch — and the entire decision is made from `ctx.page_portions`
/// (the closed page state) per the rule's doc comment.
///
/// The cost is bounded by the number of portions with non-empty REL
/// TO and the number of uncertain codes across them — a handful of
/// operations over `BTreeSet`s in practice.
///
/// **PR 4b-D.3 note (2026-05-18):** This helper intentionally reads
/// `ctx.page_portions` rather than `ctx.page_marking`. S005's
/// per-portion REL TO + uncertain-trigraph membership analysis
/// requires the portion-level `CanonicalAttrs` slice that
/// `ProjectedMarking` does not expose by design (a projected
/// marking is an aggregate, not a portion view). The
/// architecturally-clean successor is lifting per-portion REL TO
/// membership analysis into the lattice / scheme layer as derived
/// state on `ProjectedMarking`, deferred post-PR-6c.
fn analyze_uncertain_reduction(
    _attrs: &CanonicalAttrs,
    ctx: &RuleContext,
) -> Vec<Diagnostic<CapcoScheme>> {
    use marque_ism::is_decomposable;

    // Defensive — `dispatch_page_finalization` force-initializes
    // `ctx.page_portions` to `Some(_)` before invoking PageFinalization
    // rules (see `crates/engine/src/engine.rs::dispatch_page_finalization`
    // doc). This belt-and-suspenders early-return keeps the rule
    // safe under future engine refactors that might relax the
    // invariant; it should never fire in production. Same shape as
    // W004's defensive early-return in `JointDisunityCollapseRule`.
    //
    // PR 6c migration (T069): read `ctx.page_portions` (the
    // `Box<[CanonicalAttrs]>` slice snapshot) instead of the retired
    // `ctx.page_context` / `PageContext::portions()` accessor pair.
    let Some(page_portions) = ctx.page_portions.as_ref() else {
        return Vec::new();
    };
    let portions: &[CanonicalAttrs] = page_portions.as_ref();

    // Plan §3.2 requires "at least two portions carrying a
    // non-empty REL TO list." Anything less and there's no
    // intersection to compute.
    let portions_with_rel_to: Vec<&CanonicalAttrs> =
        portions.iter().filter(|p| !p.rel_to.is_empty()).collect();
    if portions_with_rel_to.len() < 2 {
        return Vec::new();
    }

    // NOFORN supersedes REL TO at the page level (CAPCO-2016
    // §H.8 + §H.9 — NOFORN/REL TO mutual exclusion). Four trigger
    // families cause `PageContext::expected_rel_to` to return empty
    // *because the marking is superseded*, not because the atom
    // intersection is empty:
    //
    //   1. Any portion carries DissemControl::Nf (NOFORN directly).
    //   2. SBU-NF / LES-NF classified-context split injects NF
    //      (§H.9 p178 / p185).
    //   3. Any portion carries NODIS (§H.9 p174 — "REL TO is not
    //      authorized in the banner line if any portion contains
    //      NODIS information. In this case, NOFORN would convey in
    //      the banner line.").
    //   4. Any portion carries EXDIS (§H.9 p172 — "REL TO is not
    //      authorized in the banner line if any portion contains
    //      EXDIS information. In this case, NOFORN would convey in
    //      the banner line.").
    //
    // Firing S005 under any of these conditions produces a
    // misleading "intersection produced REL TO (empty)" diagnostic
    // — the operator's actual problem is supersession, which is a
    // different rule's territory. Bail so S005 only runs when REL
    // TO is semantically in play. Mirrors the supersession checks
    // `PageContext::expected_rel_to` runs internally; we duplicate
    // them here because the rule needs to distinguish "empty due to
    // supersession" from "empty due to genuinely-disjoint portion
    // REL TO lists" (the latter is a legitimate S005 trigger).
    // Trigger 1 (NOFORN-direct) needs its own check because
    // `expected_non_ic_dissem`'s `needs_nf` only covers triggers
    // 2–4; triggers 2–4 are all reflected in the `needs_nf` flag.
    // Caught originally by Copilot review on PR #249; expanded to
    // cover triggers 3–4 in PR 3c.B-8F-engine-gap. Page-extension
    // stable post-PR-#488 — the bails fire on the same closed page
    // state PageFinalization observes.
    let any_portion_noforn = portions.iter().any(|p| {
        p.dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf))
    });
    if any_portion_noforn {
        return Vec::new();
    }
    // PR 4b-E: migrated from `page.expected_non_ic_dissem()` (the
    // retired PageContext method) to the lattice-native
    // `NonIcDissemSet::from_attrs_iter` constructor. Same
    // SBU-NF/LES-NF/NODIS/EXDIS NF-injection semantics
    // (§H.9 p172/p174/p178/p185); the second tuple element
    // `needs_nf` is the same flag.
    let needs_nf = crate::lattice::NonIcDissemSet::from_attrs_iter(portions).needs_nf();
    if needs_nf {
        return Vec::new();
    }

    // The atom-semantics intersection. The lattice-native
    // `RelToBlock::from_attrs_iter` does tetragraph expansion before
    // intersection and `into_boxed_slice` returns the result USA-first
    // then alphabetical per §H.8 p150-151. We project to a string set
    // for set-algebra.
    //
    // PR 4b-E: migrated from `page.expected_rel_to()` (the retired
    // PageContext method). The NOFORN-dominates / NODIS/EXDIS
    // supersession is encoded as the `NofornSuperseded` arm of
    // `RelToBlock`; the per-axis bails above already short-circuit
    // S005 in those cases (so the lattice-side supersession arms
    // produce the same empty result without redundant work).
    let expected = crate::lattice::RelToBlock::from_attrs_iter(portions).into_boxed_slice();
    let expected_set: std::collections::BTreeSet<&str> =
        expected.iter().map(|c| c.as_str()).collect();

    // Collect uncertain codes (deduped, sorted) across all portions.
    //
    // Trigraph filter: ISMCAT is — as the name says — a *tetragraph*
    // taxonomy. ISO 3166-1 alpha-3 trigraphs (USA, GBR, AUS, …)
    // aren't listed, so `is_decomposable(trigraph)` returns `None`
    // for the same reason `is_decomposable("XYZW")` does. Trigraphs
    // are atomic by ISO convention, not uncertain — skip them. The
    // shipped CVEnumISMCATRelTo recognition surface holds 280
    // length-3 trigraphs, 1 length-2 code (EU;
    // `is_decomposable=Some(false)` so already filtered by the
    // `is_none()` check), 58 length-4 tetragraphs, and 1 length-15
    // special code (AUSTRALIA_GROUP; `is_decomposable=Some(true)`).
    // The `len != 3` plus `is_none()` gates together select exactly
    // the codes the rule cares about: NA-deprecated tetragraphs and
    // taxonomy-absent (org-fork extension) tetragraphs.
    let mut uncertain_codes: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for portion in &portions_with_rel_to {
        for code in portion.rel_to.iter() {
            let s = code.as_str();
            if s.len() == 3 {
                continue;
            }
            if is_decomposable(s).is_none() {
                uncertain_codes.insert(s.to_owned());
            }
        }
    }
    if uncertain_codes.is_empty() {
        return Vec::new();
    }

    // Diagnostic span anchor: the engine passes a zero-length
    // `Span(boundary_offset, boundary_offset)` at the page-break
    // boundary (or `source.len()` for the EOD dispatch). PageContext
    // does not store per-portion spans, so per-uncertain-code span
    // precision would require extending the hot-path PageContext
    // data type. The boundary anchor is the best available pointer
    // today; users locating "which page surfaced uncertainty?" map
    // the byte offset to a page number via their own document-position
    // metadata. Same convention as W004 (`JointDisunityCollapseRule`).
    let span = ctx.candidate_span;

    let mut diagnostics = Vec::new();
    for x in &uncertain_codes {
        // Per X: portions that don't contain X. Since X is opaque
        // (atom-semantics treats it as an atom), X survives
        // intersection iff X is in every portion's rel_to.
        // Conversely, X drops iff there is at least one portion
        // without X. That set of portions-without-X is what bounds
        // the "other codes" candidate set below.
        let portions_without_x: Vec<&CanonicalAttrs> = portions_with_rel_to
            .iter()
            .copied()
            .filter(|p| !p.rel_to.iter().any(|c| c.as_str() == x.as_str()))
            .collect();
        if portions_without_x.is_empty() {
            // X in every portion ⇒ X survives atom-semantics; nothing
            // to surface for this X.
            continue;
        }

        // "Other codes" = atoms that appear in EVERY portion-without-X
        // but didn't survive intersection AND aren't X itself.
        //
        // Why "every portion-without-X" (intersection across them)
        // rather than "any portion": for an atom Y to survive
        // atom-semantics intersection IF X's hypothetical membership
        // included Y, Y must be in every portion's expansion. The
        // X-containing portions get Y "for free" via the hypothesis
        // (Y ∈ M(X)); the portions without X must have Y in their
        // own rel_to atoms. So the candidate set is exactly atoms
        // present in every portion-without-X.
        //
        // Why "not in expected": those already survived; nothing for
        // X's hypothetical membership to add.
        //
        // Why "≠ X": X is the uncertain code we're hypothesizing
        // about, not a candidate to be added by its own membership.
        //
        // Note: an atom Y that appears alongside X in the same
        // portion is irrelevant here — Y is already explicitly
        // listed in that portion, so X's hypothetical membership
        // doesn't change Y's intersection survival in any direction.
        // (Caught by Copilot review on PR #249: a previous version
        // used `union(all portions) − expected − {X}`, which
        // included same-portion atoms and produced false-positive
        // diagnostics when those atoms were missing from another
        // portion.)
        let mut atoms_in_every_without_x = s005_expand_atomic(&portions_without_x[0].rel_to);
        for p in &portions_without_x[1..] {
            let exp = s005_expand_atomic(&p.rel_to);
            atoms_in_every_without_x = atoms_in_every_without_x
                .intersection(&exp)
                .copied()
                .collect();
        }
        let other_codes: std::collections::BTreeSet<&str> = atoms_in_every_without_x
            .iter()
            .copied()
            .filter(|s| !expected_set.contains(s) && *s != x.as_str())
            .collect();
        if other_codes.is_empty() {
            continue;
        }

        let state = s005_state_text(x);
        let expected_str = if expected_set.is_empty() {
            "(empty — atom intersection produced no shared codes)".to_owned()
        } else {
            s005_render_set(&expected_set)
        };
        let other_str = s005_render_set(&other_codes);

        // G13: drop the runtime variable interpolation. Template
        // identifies the rel-to ambiguity class; the affected category
        // is CAT_REL_TO.
        let _ = (x, state, expected_str, other_str);
        let message = Message::new(
            MessageTemplate::NonCanonicalOrder,
            MessageArgs {
                category: Some(crate::scheme::CAT_REL_TO),
                ..MessageArgs::default()
            },
        );

        // No fix — the ambiguity is not resolvable from in-tree
        // data. `Diagnostic::with_fix(..., None)` signals the
        // conscious deferred-migration decision per the same
        // pattern E016/E036 used pre-PR-3c.B (matching PR #349).
        diagnostics.push(Diagnostic::with_fix(
            RuleId::new("capco", "page.dissem.rel-to-uncertain-reduction"),
            Severity::Suggest,
            span,
            message,
            S005_CITATION,
            None,
        ));
    }
    diagnostics
}

/// Citation for S005. Stays static (not formatted with
/// `ISMCAT_TETRA_VERSION`) because `Diagnostic::citation` is
/// `&'static str`. The version reference is in the state text inside
/// the message body, which is dynamically formatted via
/// `s005_state_text`. Pre-PR-#488 this constant was shared with S006;
/// post-#488 S005 is the sole consumer.
/// S005 (REL TO opaque-uncertain reduction suggestion) citation. The
/// typed `Citation` anchors at §H.8 p150 (REL TO grammar); the
/// secondary authority is the ODNI ISMCAT Tetragraph Taxonomy
/// (`ISMCAT_TETRA_VERSION`), which is not a CAPCO §-citation and
/// thus does not encode into the typed `Citation` field. The
/// per-rule doc comment carries the full provenance.
const S005_CITATION: Citation = capco(SectionLetter::H, 8, 150);

/// Citations S005 may emit on diagnostics. Wraps [`S005_CITATION`]
/// for the [`Rule::cited_authorities`] surface.
const S005_AUTHORITIES: &[Citation] = &[S005_CITATION];

impl Rule<CapcoScheme> for RelToOpaqueUncertainReductionSuggestRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.rel-to-uncertain-reduction")
    }
    fn name(&self) -> &'static str {
        "rel-to-opaque-uncertain-reduction"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
    }
    /// Phase::PageFinalization (PR #488): observes the page-level
    /// fixpoint snapshot of the REL TO axis and emits one diagnostic
    /// per uncertain code that dropped out of the page intersection.
    /// The engine dispatches this rule once per page at every
    /// scanner-emitted `MarkingType::PageBreak` BEFORE the
    /// `PageContext` reset, plus once at end-of-document. The
    /// pre-#488 `Phase::WholeMarking` + Banner-only gating produced
    /// a documented false-negative on banner-first layouts (closed
    /// by the EOD path) and a 6th-pass false-positive on
    /// intermediate Portion-time snapshots (does not recur under
    /// PageFinalization because the rule fires exactly once per
    /// page on the closed state).
    fn phase(&self) -> Phase {
        Phase::PageFinalization
    }
    /// Trusted: implementation is a pure read-only set-algebra walk
    /// over `PageContext::expected_rel_to` + per-portion REL TO
    /// projections plus a `format!` message synthesis using only
    /// canonical CountryCode strings (closed CAPCO vocabulary) and a
    /// fixed §-citation. No mutable global state, no I/O, no
    /// allocation that could fail unexpectedly; the rule is safe to
    /// skip `catch_unwind` per PR #448.
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        S005_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        analyze_uncertain_reduction(attrs, ctx)
    }
}

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










// ===========================================================================
// Issue #677 — banner.metadata.uses-portion-form / portion.metadata.uses-banner-form
// ===========================================================================
//
// Restores detection that was retired in PR 3c.B Commit 6.
//
// Commit 6 retired the historical E001 (`PortionMarkInBannerRule`) and E009
// (banner→portion form normalization) on the premise that
// `MarkingScheme::render_canonical` would absorb both fix paths. The
// renderer's fix path IS in place — but no rule emits the `Recanonicalize`
// `FixIntent` that would trigger it. Result: `SECRET//NF`, `(S//NOFORN)`,
// `SECRET//OC`, and parallel cases produced zero diagnostics on the
// pre-fix tree. Two new rules close the gap.
//
// ## Scope decision (BROAD — synthesis-brief §"Scope")
//
// Both rules walk every token in `attrs.token_spans` and dispatch through
// the `MARKING_FORMS` helpers `portion_to_banner` / `banner_to_portion`.
// Those helpers gate on the `f.banner != f.portion` condition built into
// `MARKING_FORMS`, so same-form entries (`FOUO`, `RELIDO`, `RD`, `TK`,
// etc.) return `None` and do not fire — exactly the desired filter.
//
// Coverage is therefore broad by construction:
// - Dissem pairs: NF↔NOFORN, OC↔ORCON, IMC↔IMCON, DSEN↔DEA SENSITIVE,
//   PR↔PROPIN, RS↔RSEN. (Authority: CAPCO-2016 §H.8 prose pp 132-167.)
// - Non-IC dissem: DS↔LIMDIS, XD↔EXDIS, ND↔NODIS, SBU-NF↔SBU NOFORN,
//   LES-NF↔LES NOFORN, DCNI↔DOD UCNI, UCNI↔DOE UCNI. (Authority:
//   CAPCO-2016 §H.9 prose pp 169-191.)
// - NATO classifications: CTS↔COSMIC TOP SECRET, NS↔NATO SECRET,
//   NC↔NATO CONFIDENTIAL, NR↔NATO RESTRICTED, NU↔NATO UNCLASSIFIED.
//   (Authority: CAPCO-2016 §H.2 p55, §G.1 Table 4 p36.)
// - SCI compounds: SI-EU↔SI-ECRU, SI-NK↔SI-NONBOOK. (Authority:
//   CAPCO-2016 §H.4 p78, p83.)
//
// US classification shorthand (S↔SECRET, TS↔TOP SECRET, C↔CONFIDENTIAL,
// U↔UNCLASSIFIED, R↔RESTRICTED) is NOT in `MARKING_FORMS` — the table's
// header doc-comment explicitly carves classification levels out
// because `Classification::banner_str` / `portion_str` own that mapping.
// The banner rule adds a small classification branch reading
// `attrs.classification` to cover this; it catches the PM-4 sister bug
// (`S//NOFORN` — classification abbreviation in banner) per CAPCO-2016
// §D.1 p27 ("The classification level must be in English without
// abbreviation"). One rule covers two gaps; no separate
// sister-bug issue is needed.
//
// ## Emission shape — ONE diagnostic per marking
//
// Both rules emit exactly ONE diagnostic per offending marking even when
// multiple tokens in the same marking carry the wrong form (e.g.,
// `S//NF` has both `S` and `NF` defective in a banner). The fix payload
// is `ReplacementIntent::Recanonicalize { scope }` — the engine's
// `render_canonical` re-emits the entire marking from canonical attrs,
// so a single intent at the marking scope covers every wrong-form token
// within it. Per-token emission would force the C-1 overlap guard to
// deduplicate effectively-identical `Recanonicalize` intents.
//
// The diagnostic's primary `span` is the first offending token's span
// (so the user sees where the violation is). The `candidate_span` is
// the marking-scope span (where the fix applies).
//
// ## EYES suppression — banner direction only
//
// Bare `EYES` / `EYES ONLY` in a banner is owned by E064
// (`EyesOnlyConvertToRelToRule`), whose §H.8 p157 + p158 authority
// covers the cross-axis conversion to `REL TO USA, AUS, CAN, GBR, NZL`
// (FVEY) on derivative use. `PortionFormInBannerRule` suppresses these
// tokens so the engine's C-1 overlap guard does not have to arbitrate
// between E064's richer cross-axis fix and our `Recanonicalize` — E064
// wins on §-grounded richness and the suppression keeps that intent
// reachable.
//
// E064 does NOT fire in portion context for bare `EYES` (§H.8 p158
// says "carry forward the trigraph codes listed in the source document
// banner line" — synthesis from a portion alone is not safe), so
// `BannerFormInPortionRule` does NOT suppress; the modest improvement
// of canonicalizing `(S//EYES ONLY)` → `(S//EYES)` via the portion-form
// re-render is better than silent acceptance.
//
// ## Authority
//
// - `PortionFormInBannerRule` emits at §D.1 p27 — "Any control markings
//   in the banner line may be spelled out per the 'Marking Title'
//   (e.g., TALENT KEYHOLE) or abbreviated as per the 'Authorized
//   Abbreviation' (e.g., TK)". A portion form (`NF`) is neither.
// - `BannerFormInPortionRule` emits at §C.1 p25 — "An authorized
//   portion mark is listed for each classification and control marking
//   entry in the Register." A banner form (`NOFORN`) is not the listed
//   portion mark.
//
// Each citation re-verified at authorship per Constitution VIII
// against `crates/capco/docs/CAPCO-2016.md` lines 503 and 560.

/// `capco:banner.metadata.uses-portion-form` — portion-form token
/// appearing in a banner-line marking. CAPCO-2016 §D.1 p27.
struct PortionFormInBannerRule;

/// Citations `PortionFormInBannerRule` may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const PORTION_FORM_IN_BANNER_AUTHORITIES: &[Citation] = &[capco(SectionLetter::D, 1, 27)];

impl Rule<CapcoScheme> for PortionFormInBannerRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "banner.metadata.uses-portion-form")
    }
    fn name(&self) -> &'static str {
        "portion-form-in-banner"
    }
    fn default_severity(&self) -> Severity {
        // Severity::Fix — deterministic auto-applicable canonicalization.
        // The closed Register (§G.1 Table 4 p38) guarantees a single
        // canonical banner form per marking; the renderer's
        // `render_canonical` produces those bytes from the parsed
        // attrs without classifier judgment.
        Severity::Fix
    }
    /// `Phase::WholeMarking`: the `Recanonicalize { Page }` intent
    /// covers the full banner span at promotion time. The diagnostic
    /// span points at the first offending token (a sub-region of the
    /// banner), but the fix scope is the whole marking by construction
    /// — matches the precedent set by E066
    /// (`LegacyNatoCompoundRemarkRule`).
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        PORTION_FORM_IN_BANNER_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Fire on Banner only. Portion-form-in-portion is canonical;
        // CAB/PageBreak/PageFinalization are not banner-line surfaces.
        if ctx.marking_type != MarkingType::Banner {
            return Vec::new();
        }
        if let Some(offending_span) = find_portion_form_in_banner(attrs) {
            vec![emit_form_mismatch(
                self.id(),
                self.default_severity(),
                offending_span,
                ctx.candidate_span,
                RecanonScope::Page,
                capco(SectionLetter::D, 1, 27),
            )]
        } else {
            Vec::new()
        }
    }
}

/// `capco:portion.metadata.uses-banner-form` — Authorized Banner Line
/// Marking Title OR Authorized Banner Line Abbreviation (i.e., either
/// column 1 or column 2 of §G.1 Table 4 p38) appearing in portion-mark
/// position where the canonical portion form per column 3 differs.
///
/// Authority: CAPCO-2016 §C.1 p25 (portion marks are Register-closed)
/// + §G.1 Table 4 p38 (the three columns are the authoritative
///   Title/Abbreviation/Portion-Mark surface).
struct BannerFormInPortionRule;

/// Citations `BannerFormInPortionRule` may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const BANNER_FORM_IN_PORTION_AUTHORITIES: &[Citation] = &[capco(SectionLetter::C, 1, 25)];

impl Rule<CapcoScheme> for BannerFormInPortionRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.metadata.uses-banner-form")
    }
    fn name(&self) -> &'static str {
        "banner-form-in-portion"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    /// `Phase::WholeMarking`: mirror of `PortionFormInBannerRule`. The
    /// `Recanonicalize { Portion }` intent covers the full portion span;
    /// per-token splice would not let the engine canonicalize multiple
    /// wrong-form tokens in one pass.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        BANNER_FORM_IN_PORTION_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        if ctx.marking_type != MarkingType::Portion {
            return Vec::new();
        }
        if let Some(offending_span) = find_banner_form_in_portion(attrs) {
            vec![emit_form_mismatch(
                self.id(),
                self.default_severity(),
                offending_span,
                ctx.candidate_span,
                RecanonScope::Portion,
                capco(SectionLetter::C, 1, 25),
            )]
        } else {
            Vec::new()
        }
    }
}

/// Walk `attrs` for a banner-line token using the portion form (or for
/// a US-classification token using its abbreviation). Returns the span
/// of the first offending token, or `None` if every token uses an
/// authorized banner form. The traversal terminates at the first hit
/// so the caller can emit exactly one diagnostic per marking — the
/// `Recanonicalize { Page }` fix re-renders all token positions at
/// promotion time, so additional defective tokens in the same banner
/// are covered by the single fix.
///
/// EYES / EYES ONLY tokens are skipped — E064 owns the §H.8 p157 +
/// p158 cross-axis conversion to `REL TO USA, AUS, CAN, GBR, NZL`
/// (FVEY). Suppressing those tokens here keeps E064's richer fix
/// reachable when both rules would otherwise produce overlapping
/// intents on the same span.
fn find_portion_form_in_banner(attrs: &CanonicalAttrs) -> Option<Span> {
    // Classification branch — US classifications carry portion forms
    // (S/TS/C/U/R) NOT in `MARKING_FORMS` (per the table's
    // doc-comment carve-out; classification mapping lives on
    // `Classification::banner_str` / `portion_str`).
    //
    // Read the US level via `CanonicalAttrs::us_classification()` so the
    // branch covers both `MarkingClassification::Us(_)` AND
    // `MarkingClassification::Conflict { us, .. }`. §D.1 p27 ("The
    // classification level must be in English without abbreviation")
    // applies to the US classification token regardless of whether the
    // banner also carries a NATO or JOINT side — the Conflict variant
    // is what the parser emits for those compound banners, and the
    // US-side token is still in banner position.
    if let Some(level) = attrs.us_classification() {
        let portion = level.portion_str();
        let banner = level.banner_str();
        if portion != banner {
            for token in attrs.token_spans.iter() {
                if token.kind == TokenKind::Classification && token.text.as_str() == portion {
                    return Some(token.span);
                }
            }
        }
    }
    // Token-walk branch — every other axis dispatches through
    // `MARKING_FORMS::portion_to_banner` (built-in `banner != portion`
    // gate is the universal filter).
    for token in attrs.token_spans.iter() {
        let text = token.text.as_ref();
        // E064 owns bare-EYES / EYES ONLY in banner — see the module-
        // header comment for the cross-rule suppression rationale.
        if text == "EYES" || text == "EYES ONLY" {
            continue;
        }
        if marque_ism::marking_forms::portion_to_banner(text).is_some() {
            return Some(token.span);
        }
    }
    None
}

/// Mirror of [`find_portion_form_in_banner`] for the portion-mark
/// direction. Returns the span of the first banner-form token, where
/// "banner form" means either an Authorized Banner Abbreviation
/// (§G.1 Table 4 p38 column 2, e.g., `NOFORN`, `IMCON`) OR an
/// Authorized Marking Title (§G.1 Table 4 p38 column 1, e.g.,
/// `TALENT KEYHOLE`, `ORIGINATOR CONTROLLED`,
/// `NOT RELEASABLE TO FOREIGN NATIONALS`) that has a distinct
/// portion form per column 3. The portion-mark Register surface is
/// closed (§C.1 p25 — "An authorized portion mark is listed for
/// each classification and control marking entry in the Register"),
/// so anything that maps to a different portion canonical via
/// either lookup is a form mismatch.
///
/// Two lookups are consulted in order:
///
/// 1. [`marque_ism::marking_forms::banner_to_portion`] covers the
///    `MARKING_FORMS.banner` column (Authorized Abbreviation column
///    of §G.1 Table 4). Catches `NOFORN`/`ORCON`/`IMCON`/etc. in
///    portion position.
/// 2. [`marque_ism::marking_forms::title_to_portion`] covers the
///    `MARKING_FORMS.title` column (Marking Title column). Catches
///    long-title forms like `TALENT KEYHOLE` (title) in portion
///    position where the canonical portion form is `TK`. The
///    `banner_to_portion` lookup misses these because its row gate
///    is `f.banner != f.portion`, and TALENT-KEYHOLE-class rows
///    have `f.banner == f.portion` (the Authorized Abbreviation
///    column matches the Portion Mark column — they happen to be
///    the same canonical bytes); only the Marking Title column
///    differs.
///
/// Does NOT skip EYES — E064 does not fire on bare-EYES in portion
/// context (§H.8 p158 says "carry forward the trigraph codes listed
/// in the source document banner line", which Marque cannot
/// synthesize from a portion alone), so the modest improvement of
/// canonicalizing `(S//EYES ONLY)` → `(S//EYES)` via the
/// portion-form re-render is strictly better than silent
/// acceptance. The `title_to_portion` lookup returns `None` for
/// `EYES ONLY` because its row has `title == banner`, so the
/// fallback is not a new source of EYES double-emit.
///
/// US classification banner forms (`SECRET`, `TOP SECRET`, etc.)
/// are NOT in `MARKING_FORMS`, and `Classification::banner_str`
/// returns `&'static str` not a `TokenId` — but the parser's
/// classification recognizer accepts these long forms in either
/// position and canonicalizes `attrs.classification` regardless.
/// The portion rule does not surface "long-form classification in
/// portion" today because there is no MARKING_FORMS row to drive
/// the `banner_to_portion` lookup; that case is the symmetric gap
/// to the banner-direction classification branch above, deferred to
/// a follow-up if needed (the more common direction is the
/// abbreviation-in-banner case PM-4 names).
fn find_banner_form_in_portion(attrs: &CanonicalAttrs) -> Option<Span> {
    for token in attrs.token_spans.iter() {
        let text = token.text.as_ref();
        // Banner Abbreviation column — `NOFORN`/`ORCON`/`IMCON`/etc.
        if marque_ism::marking_forms::banner_to_portion(text).is_some() {
            return Some(token.span);
        }
        // Marking Title column — `TALENT KEYHOLE`/`ORIGINATOR CONTROLLED`/
        // `NOT RELEASABLE TO FOREIGN NATIONALS`/etc. Only fires when the
        // Title column differs from the Authorized Abbreviation column
        // (the `title != banner` gate inside `title_to_portion`), so
        // same-form-as-banner titles (e.g., `DEA SENSITIVE` where
        // `title == banner == "DEA SENSITIVE"`) cannot double-emit:
        // `banner_to_portion("DEA SENSITIVE")` returns `Some("DSEN")`
        // above and short-circuits; `title_to_portion("DEA SENSITIVE")`
        // returns `None` and would never reach here anyway. Authority:
        // §C.1 p25 (portion mark must be the listed Register entry) +
        // §G.1 Table 4 p38 (Register-closed-set governs both column 1
        // Marking Title and column 2 Banner Abbreviation as the
        // authorized banner forms — neither is the portion form).
        if marque_ism::marking_forms::title_to_portion(text).is_some() {
            return Some(token.span);
        }
    }
    None
}

/// Construct a form-mismatch diagnostic. Shared between the banner-
/// and portion-direction rules because the emission shape is
/// identical — only the rule id, recanon scope, and citation differ.
///
/// The fix payload is `ReplacementIntent::Recanonicalize { scope }`
/// at confidence 1.0 (deterministic per the §G.1 Table 4 closed set).
/// `MessageArgs::default()` mirrors the E005 / E006 precedent: the
/// closed-token set is too varied to bind every form-pair to a
/// `TokenId`, and the per-rule predicate ID plus the diagnostic span
/// already identify the violation kind and location for audit
/// consumers (Constitution V Principle V G13 — no document bytes flow
/// through the typed message).
fn emit_form_mismatch(
    rule: RuleId,
    severity: Severity,
    offending_span: Span,
    candidate_span: Span,
    scope: RecanonScope,
    citation: Citation,
) -> Diagnostic<CapcoScheme> {
    let fix_intent = FixIntent {
        replacement: ReplacementIntent::Recanonicalize { scope },
        confidence: Confidence::strict(1.0),
        feature_ids: Default::default(),
        message: Message::new(MessageTemplate::WrongTokenForm, MessageArgs::default()),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    };
    Diagnostic::with_fix_at_span(
        rule,
        severity,
        offending_span,
        candidate_span,
        Message::new(MessageTemplate::WrongTokenForm, MessageArgs::default()),
        citation,
        fix_intent,
    )
}


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
