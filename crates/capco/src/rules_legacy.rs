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
use marque_ism::generated::migrations::find_migration;
use marque_ism::{
    CanonicalAttrs, CountryCode, MarkingClassification, MarkingType, SciControlSystem, SciMarking,
    Span, TETRAGRAPH_MEMBERS, TokenKind, TokenSpan, sar_sort_key,
};
use marque_rules::{
    Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Phase,
    Rule, RuleContext, RuleId, RuleSet, Severity,
};
use marque_scheme::{
    Citation, FactRef, MarkingScheme, RecanonScope, ReplacementIntent, Scope, SectionLetter, capco,
    capco_section,
};
use std::collections::HashSet;

use crate::rules::helpers::{
    FixDiagnosticParams, canonicalize_trigraph_list, is_fgi_invalid_ownership_token,
    make_fix_diagnostic, sar_block_span,
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

// ---------------------------------------------------------------------------
// Rule: E002 — Missing USA in REL TO trigraph list
// ---------------------------------------------------------------------------

/// E002 detects missing or misplaced `USA` in the REL TO marking template
/// from CAPCO-2016 §H.8 (p150–151, "Additional Marking Instructions"):
///
/// - Line 3713: "'USA' must always appear first whenever the REL TO string
///   is used to communicate release decisions either by the US or a Non-US
///   entity."
///
/// When E002 fires, its fix also produces a canonical REL TO list in a
/// single pass by placing `USA` first and alphabetizing the remaining
/// trigraphs. That canonicalization aligns the output with p151:
///
/// - Line 3714: "After 'USA', list the required one or more trigraph country
///   codes in alphabetical order followed by tetragraph codes listed in
///   alphabetical order. Each code is separated by a comma and a space."
///
/// E002 does not, by itself, detect alphabetical-ordering errors when `USA`
/// is already present and first; those cases are handled by the renderer's
/// REL TO axis (`render_rel_to.rs`) per CAPCO-2016 §H.8 p150–151 (pre-PR-3c.B
/// the ordering check belonged to E020 / E060, both retired). The 0.97
/// confidence is predicated on single-pass canonicalization so an E002 fix
/// does not leave behind a latent alphabetical-ordering violation for a
/// second pass.
///
/// Scope boundaries:
/// - Tetragraph alphabetization is deferred. `CountryCode` (issue
///   #183 PR-A) now carries tetragraphs, but E002 still sorts the
///   list as a flat alphabetical sequence rather than the §H.8 p151
///   "trigraphs alpha, then tetragraphs alpha, USA first" form.
///   Separate follow-up — the canonicalizer should partition true
///   country trigraphs (`code.len() == 3`) from the remaining codes
///   (the 2-byte `EU`, the 4-byte tetragraphs, and 15-byte
///   `AUSTRALIA_GROUP` belong in the non-trigraph bucket) before
///   sorting, or ideally derive the buckets from the CVE schema
///   groups in `CVEnumISMCATRelTo.xsd`.
/// - "REL TO USA" alone (p151, a non-authorized marking with no
///   following country codes) is out of scope. E002 does not fire when
///   USA is present and first; a separate rule is needed for that case.
struct MissingUsaTrigraphRule;

/// Citations E002 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
///
/// §H.8 p151 is the precise authority for "USA always appears first"
/// in the REL TO list. §H.8 p150 is the section anchor (REL TO marking
/// template) but the verbatim USA-first rule sits in the
/// "Additional Marking Instructions" block on p151.
const E002_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 8, 151)];

impl Rule<CapcoScheme> for MissingUsaTrigraphRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.dissem.rel-to-missing-usa")
    }
    fn name(&self) -> &'static str {
        "missing-usa-trigraph"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    /// Phase::WholeMarking: rewrites the entire REL TO block (multi-token
    /// span covering first→last `RelToTrigraph` plus any trailing
    /// separators); requires whole-marking attrs to canonicalize.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E002_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        if attrs.rel_to.is_empty() {
            return vec![];
        }

        let has_usa = attrs.rel_to.contains(&marque_ism::CountryCode::USA);
        let usa_first = attrs
            .rel_to
            .first()
            .is_some_and(|t| *t == marque_ism::CountryCode::USA);

        if has_usa && usa_first {
            return vec![];
        }

        // PR 3c.2.C C5 / G13: drop the runtime string distinction;
        // `MessageTemplate::NonCanonicalOrder` with `category =
        // Some(CAT_REL_TO)` identifies the violation class. Both arms
        // (missing USA / USA not first) map to the same template
        // because both are "REL TO ordering violation" per
        // §H.8 p150-151. The narrower distinction lives in the
        // `MessageArgs` populated below.
        let message = Message::new(
            MessageTemplate::NonCanonicalOrder,
            MessageArgs {
                category: Some(crate::scheme::CAT_REL_TO),
                ..MessageArgs::default()
            },
        );
        // §H.8 p151 carries the verbatim USA-first rule (the
        // Additional Marking Instructions block on the REL TO page);
        // p150 is the section anchor for the REL TO marking template
        // generally. T044 reviewer pass corrected this from p150 to
        // p151 to match the precision of `cited_authorities()` —
        // declared and emitted citations must agree (F.1 corpus-
        // fidelity gate).
        let citation = capco(SectionLetter::H, 8, 151);

        // Locate the `RelToBlock` this diagnostic refers to. If the
        // marking has more than one REL TO block (e.g.,
        // `SECRET//REL TO GBR//NF//REL TO AUS`), a single first→last
        // splice would delete intervening `//...//` content. In that
        // case we emit a diagnostic with no FixProposal and let the
        // author resolve manually. The discriminator only needs to
        // distinguish 0 / 1 / many, so we pull two iterator items and
        // match on the shape rather than allocating a `Vec`.
        let mut rel_to_blocks_iter = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToBlock);
        let block = match (rel_to_blocks_iter.next(), rel_to_blocks_iter.next()) {
            (Some(first), None) => first,
            (None, _) => {
                // No block tagging (defensive: `attrs.rel_to` non-empty
                // should imply at least one `RelToBlock` token). Emit
                // diagnostic without a fix rather than risk mis-splice.
                return vec![Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    Span::new(0, 0),
                    message.clone(),
                    citation,
                    None,
                )];
            }
            (Some(first), Some(_)) => {
                // Multiple REL TO blocks present; the message template
                // is the same NonCanonicalOrder class, but the
                // recoverability differs (single-pass fix is unsafe).
                return vec![Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    first.span,
                    message.clone(),
                    citation,
                    None,
                )];
            }
        };

        // Collect RelToTrigraph spans that fall inside the single
        // RelToBlock. Filtering on block containment is defensive
        // against future parser changes that might surface trigraph
        // tokens outside their block.
        let rel_to_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| {
                t.kind == TokenKind::RelToTrigraph
                    && t.span.start >= block.span.start
                    && t.span.end <= block.span.end
            })
            .collect();
        let (first, last) = match (rel_to_spans.first(), rel_to_spans.last()) {
            (Some(f), Some(l)) => (f, l),
            _ => {
                return vec![Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    block.span,
                    message.clone(),
                    citation,
                    None,
                )];
            }
        };

        // Span: first→last `RelToTrigraph` within this block, extended
        // through any trailing `,`/whitespace tail *only when* the
        // remainder of the RelToBlock after the last trigraph is
        // delimiter-only. This consumes stale delimiters like the
        // trailing `,` in `REL TO GBR, AUS,` so the splice leaves a
        // clean list. We gate on delimiter-only to preserve any
        // content we can't recognize (tokens outside the CVE
        // TRIGRAPHS list — `is_trigraph` returns false, so the parser
        // never emits a `RelToTrigraph` span for them; deleting them
        // would be wrong).
        let start = first.span.start;
        let mut end = last.span.end;
        let tail_offset = end - block.span.start;
        let block_bytes = block.text.as_bytes();
        if tail_offset <= block_bytes.len() {
            let tail = &block_bytes[tail_offset..];
            if tail.iter().all(|b| matches!(b, b',' | b' ' | b'\t')) {
                end = block.span.end;
            }
        }
        let span = Span::new(start, end);

        // Build the fully canonical list (USA first, non-USA entries
        // alphabetical per CAPCO-2016 §H.8 p151, no duplicates) via
        // [`canonicalize_trigraph_list`]. When USA is missing from
        // input we add it before canonicalizing so the output always
        // has USA first; the helper itself treats USA as "first if
        // present" without injecting it (the helper's contract is
        // "rearrange, don't synthesize"). Producing the canonical form
        // in a single pass is required because the renderer's REL TO
        // axis only canonicalizes when USA is already first — the
        // §H.8 p151 ordering invariant moved into `render_rel_to.rs`
        // when E020 / E060 retired. Dedup before canonicalize so
        // E002's fix output stays canonical when input also has
        // duplicates — under the C-1 overlap guard E002's narrow span
        // would not deduplicate other rules' edits, so we deduplicate
        // PR 3c.B Commit 10: structural FixIntent only. The engine's
        // synthesis path (`synthesize_fixes`) re-renders the canonical
        // bytes from the per-page projection at promotion time via
        // `apply_intent` + `render_canonical`. The rule emits the
        // structural intent only; no byte-precise replacement
        // computation lives on this path post-cutover (G13).
        //
        //   - USA missing → `FactAdd { USA, Scope::Portion }`
        //     (USA injection is a fact-set addition mandated by §H.8 p151).
        //   - USA not first → `Recanonicalize { Portion }` (the sort
        //     is renderer territory; `render_canonical` absorbs
        //     USA-first alpha by construction).
        let intent_scope_recanon = match ctx.marking_type {
            marque_ism::MarkingType::Portion => RecanonScope::Portion,
            _ => RecanonScope::Page,
        };
        let intent_scope_factadd = match ctx.marking_type {
            marque_ism::MarkingType::Portion => Scope::Portion,
            _ => Scope::Page,
        };
        let fix_intent = if !has_usa {
            FixIntent {
                replacement: ReplacementIntent::FactAdd {
                    token: FactRef::Cve(crate::scheme::TOK_USA),
                    scope: intent_scope_factadd,
                },
                confidence: Confidence::strict(0.97),
                feature_ids: Default::default(),
                message: Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            }
        } else {
            FixIntent {
                replacement: ReplacementIntent::Recanonicalize {
                    scope: intent_scope_recanon,
                },
                confidence: Confidence::strict(0.97),
                feature_ids: Default::default(),
                message: Message::new(MessageTemplate::NonCanonicalOrder, MessageArgs::default()),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            }
        };
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            span,
            ctx.candidate_span,
            message,
            citation,
            fix_intent,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E005 — Declassification instruction misplaced (belongs in CAB)
// ---------------------------------------------------------------------------

/// E005 fires when a declassification exemption or `Declassify On` date
/// appears inside a banner or portion marking rather than the Classification
/// Authority Block (CAB).
///
/// # Authority
///
/// Two CAPCO-2016 passages together establish the invariant:
///
/// - **§E.1 p31** enumerates `Declassify On` as a CAB line and lists its
///   valid values: YYYYMMDD dates, events, `25X#`, `50X#`, `75X#`,
///   `50X1-HUM`, `50X2-WMD`, `25X1, EO 12951`, and the `N/A …` forms.
///   This is the authoritative "declass values live here" list.
///   §E.2 p32 reaffirms it for derivative classification: "Only a single
///   value must be used on the `Declassify On` line of the classification
///   authority block."
/// - **§D.1 p27** enumerates the banner syntax's permitted categories —
///   classification, SCI, SAP, AEA, Dissem, Non-IC Dissem. Declassification
///   is **not** on this closed list, and §C.1 p26 lines 525ff gives
///   portions the same category set. A declass token appearing between
///   `//` separators of a banner or portion is unambiguously misplaced.
///
/// The invariant is safely broader than CAPCO's OCA (§E.1) vs derivative
/// (§E.2) vs FGI (§E.4) distinctions — all variants place declass in the
/// CAB, so the predicate does not branch on classification source.
///
/// # Scope
///
/// Fires on `MarkingType::Banner` and `MarkingType::Portion`. Explicitly
/// does NOT fire on `MarkingType::Cab` — that is the correct location for
/// declass info and a CAB candidate carrying `declassify_on` /
/// `declass_exemption` is well-formed, not violating.
///
/// # Fix
///
/// None. Repairing a misplaced declass marking requires moving the token
/// from the banner/portion into a CAB, which is multi-span document-level
/// rewriting rather than a local replacement. E005 surfaces the
/// diagnostic; the author resolves manually.
// ---------------------------------------------------------------------------
// Migration status (PR 3c.B Sub-PR 9, 2026-05-11): provisional Path A
// per `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md` D4.
// E005 stays as a hand-written `Rule` impl in this file; it does NOT
// migrate to a `Constraint::Custom` catalog row on `CapcoScheme` in this
// PR.
//
// Retirement target: `Recanonicalize { scope: Scope::Document }` on the
// `MarkingScheme` trait surface, once `render_canonical` (deferred per
// `architecture.md` §"What this commits us to") can position declass in
// the Classification Authority Block (CAB) by construction. Authority:
// CAPCO-2016 §E.1 p31 + §E.2 p32 (`Declassify On` is a CAB line — the
// single-value mandate makes the position unambiguous) + §D.1 p27 (the
// banner category list enumerates classification + control markings;
// declassification is conspicuously absent — negative-inference). §E
// commingling exemptions at pp 33-34 are CAB-line *content* rules (e.g.,
// "N/A to RD/FRD/TFNI portions"), not placement rules, and do not weaken
// the "declass belongs in CAB" invariant.
//
// Structural blocker (why Path A in PR 3c.B Commit 9):
// `MarkingScheme::evaluate_custom` (crates/scheme/src/scheme.rs:124-130)
// receives only `&Self::Marking`. It has no access to
// `RuleContext.marking_type`, so a constraint-catalog predicate cannot
// reproduce the existing `Banner | Portion` gate (lines below). Without
// that gate, the predicate would fire on every CAB candidate — declass
// in a CAB is the correct location, not a violation. The trait-surface
// extension that would unblock this migration is tracked in
// `specs/006-engine-rule-refactor/followups/constraint-context-extension.md`.
//
// `Diagnostic::with_fix(..., None)` constructor: this rule emits
// neither a legacy `FixProposal` nor a structural `FixIntent<S>` because
// the repair is multi-span document-level rewriting (move the declass
// token from banner/portion into a CAB). The constructor swap (vs the
// `Diagnostic::new(..., None)` form) signals consciously-decided deferred
// migration evaluation, matching the PR #349 pattern for E016/E036.
// Downstream audit consumers observe no behavioral difference: both
// constructors leave `fix: None` and `fix_intent: None`.
struct DeclassifyMisplacedRule;

/// E005 secondary CAPCO §-citations.
///
/// PR 10.A.1 Commit 4: the migration to typed `Citation` collapsed the
/// pre-migration string form `"CAPCO-2016 §E.1 p31 + §D.1 p27"` into a
/// single `capco(SectionLetter::E, 1, 31)` value on the emitted
/// diagnostic (typed `Citation` carries one passage). The cross-reference
/// to §D.1 p27 (banner categories exclude declassification) survived in
/// the rule's doc-comment but was un-checked — a rename or removal of
/// the comment wouldn't trip a test. This constant pins the dropped
/// cross-reference structurally so a regression that loses the §D.1 p27
/// connection still fails a test.
///
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1
/// Commit 4 authorship per Constitution VIII propagation rule:
/// §D.1 p27 enumerates the banner-line categories and conspicuously
/// excludes declassification, the negative-inference complement to
/// §E.1 p31's positive "Declassify On is a CAB line" rule.
///
/// The constant is rule-authoritative metadata intended for runtime
/// introspection by a future PR 10.A.2 `Rule::cited_authorities()`
/// trait method (deferred per the PR brief). Today the only consumer
/// is the `citation_cross_refs_tests` module at the bottom of this
/// file (`#[cfg(test)]`-gated, parallel to but not conflated with the
/// `#[cfg(any())]`-gated inline `mod tests` that's dead code pending a
/// separate rewrite). The const is `pub(crate)` so the test mod can
/// reach it directly; under non-test builds, the `#[allow(dead_code)]`
/// keeps the compiler quiet and the linker DCEs the const at use-site
/// (consts in Rust are inlined; an unused `pub(crate) const` does not
/// add to the production binary footprint, including the WASM-shipped
/// crate surface).
#[allow(dead_code)] // used by `citation_cross_refs_tests` at end of file
pub(crate) const E005_CROSS_REFS: &[Citation] = &[capco(SectionLetter::D, 1, 27)];

/// Citations E005 may emit on diagnostics. Combines the primary
/// `Diagnostic.citation` value (§E.1 p31) with the
/// [`E005_CROSS_REFS`] cross-references. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E005_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::E, 1, 31),
    capco(SectionLetter::D, 1, 27),
];

impl Rule<CapcoScheme> for DeclassifyMisplacedRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.declassification.declassify-on-misplaced")
    }
    fn name(&self) -> &'static str {
        "declassify-misplaced"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::WholeMarking: no auto-fix; flags declass-token placement
    /// at document scope (move into the CAB). Decision reads across the
    /// banner/portion/CAB axes.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E005_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::MarkingType;
        // Fire on banner AND portion. CAB candidates are the correct
        // location for declass info and must be skipped. PageBreak is
        // not a marking and carries no attributes.
        if !matches!(ctx.marking_type, MarkingType::Banner | MarkingType::Portion) {
            return vec![];
        }
        if attrs.declassify_on.is_none() && attrs.declass_exemption.is_none() {
            return vec![];
        }

        // Span: whichever declass-related token is present.
        let span = attrs
            .token_spans
            .iter()
            .find(|t| matches!(t.kind, TokenKind::DeclassExemption | TokenKind::DeclassDate))
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        // PR 3c.B Sub-PR 9: provisional Path A — `with_fix_intent(..., None)`
        // signals consciously-decided deferred migration evaluation. See the
        // migration-status block above `struct DeclassifyMisplacedRule;` for
        // the full rationale and retirement target.
        //
        // Citation: §E.1 p31 governs the "Declassify On is a CAB line"
        // rule; §D.1 p27 affirms banner categories do not include
        // declassification. The typed `Citation` field anchors at §E.1
        // p31; the cross-reference to §D.1 p27 lives in the doc-comment
        // above this rule (the typed-Citation struct carries one
        // §-citation per Diagnostic).
        vec![Diagnostic::with_fix(
            self.id(),
            self.default_severity(),
            span,
            Message::new(MessageTemplate::WrongTokenForm, MessageArgs::default()),
            capco(SectionLetter::E, 1, 31),
            None, // Fix requires document-level context (moving a token
                  // from banner/portion into a CAB is multi-span).
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E006 — Deprecated dissem control
// ---------------------------------------------------------------------------

/// Fires when a marking contains a deprecated dissemination control.
///
/// Most deprecated dissem controls (e.g., `LIMDIS`, `FOUO`) are absent from
/// the modern CVE entirely, so the parser surfaces them as `Unknown` tokens.
/// E006 walks Unknown tokens and looks each up in the migration table; a
/// hit whose replacement is a known dissem control fires the diagnostic.
///
/// Entries owned by E001 (banner abbreviation, e.g., `NF`→`NOFORN`) are
/// handled by E001 instead, so the duplicate dispatch is suppressed via the
/// `is_dissem_replacement` filter below.
struct DeprecatedDissemRule;

/// Citations E006 may emit on diagnostics. §F has no numbered
/// subsections; the bare-section page anchor at p35 marks the
/// start of the §F "Legacy Control Markings" passage. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E006_AUTHORITIES: &[Citation] = &[capco_section(SectionLetter::F, 35)];

impl Rule<CapcoScheme> for DeprecatedDissemRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "marking.deprecation.deprecated-dissem-control")
    }
    fn name(&self) -> &'static str {
        "deprecated-dissem"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::Localized: each fix rewrites a single `DissemControl` /
    /// `Unknown` token in place via the migration table (e.g.
    /// `LIMDIS → LIMITED DISTRIBUTION`). Span is strictly the one
    /// `TokenSpan` the rule walked.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E006_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let mut diagnostics = Vec::new();
        // Walk every TokenSpan whose kind is either DissemControl (the
        // deprecated marking is in the modern CVE — e.g., FOUO) or Unknown
        // (the deprecated marking has been removed from the CVE — e.g.,
        // LIMDIS). For each, look up the migration table by text. A hit
        // whose replacement is a known dissem name is an E006 violation.
        for token in attrs.token_spans.iter() {
            if !matches!(token.kind, TokenKind::DissemControl | TokenKind::Unknown) {
                continue;
            }
            let Some(entry) = find_migration(token.text.as_ref()) else {
                continue;
            };
            // Skip declass-shorthand entries (E007 owns those).
            if !is_dissem_replacement(entry.replacement) {
                continue;
            }
            // Form-pair ownership (NF/OC/IMC/DSEN/PR ↔ NOFORN/ORCON/IMCON/
            // DEA SENSITIVE/PROPIN): owned by
            // `capco:banner.metadata.uses-portion-form` +
            // `capco:portion.metadata.uses-banner-form` per #677. The
            // historical `is_abbreviation_expansion` guard here was
            // dead-code-by-construction: the `MIGRATIONS` table in
            // `marque_ism::generated::migrations` carries only declass
            // shorthand entries today (`25X1-` / `50X1-` X-shorthand
            // patterns per CAPCO-2016 §E.6 p34), and the
            // `is_dissem_replacement` filter above rejects every one of
            // them BEFORE reaching this point. No form-pair entry has
            // existed in `MIGRATIONS` since T035c-4 (legacy IDs E001 /
            // E009, now migrated to the wire strings cited above).
            // Tracked as a follow-up: the `MIGRATIONS` doc-comment in
            // `crates/ism/build.rs` still references the legacy E001 /
            // E009 rule IDs and the removed `is_abbreviation_expansion`
            // guard; updating that doc is engine-crate territory under
            // Constitution VII §IV and cannot land in this CAPCO PR.
            // Constitution V Principle V (G13): the original document
            // bytes (`token.text`) and the canonical replacement
            // (`entry.replacement`) do NOT flow into the typed
            // `Message`. The replacement is on the permitted-identifier
            // list (token canonical from a closed vocabulary), but
            // `MessageArgs.expected_token` carries a `TokenId`, not a
            // raw `&str` — and we do not have a guaranteed `TokenId`
            // projection for every deprecation-table entry. The
            // bytes ARE still carried by `Diagnostic.text_correction.replacement`
            // (the canonical replacement is on the permitted list).
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::MigrationTable,
                span: token.span,
                message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                // §F covers all legacy Control Marking deprecations
                // (E006 dissem migration table). §F has no numbered
                // subsections in CAPCO-2016 (the citation-index
                // confirms `section: F` carries no `subsections:`
                // list); use the bare-section helper with page 35
                // (start of §F per citation-index).
                citation: capco_section(SectionLetter::F, 35),
                original: token.text.to_string(),
                replacement: entry.replacement.to_owned(),
                confidence: entry.confidence,
                migration_ref: Some(entry.reference),
            }));
        }
        diagnostics
    }
}

/// Returns `true` if `replacement` is one of the dissemination-control
/// replacements that E006 is allowed to claim from MIGRATIONS.
///
/// This is intentionally a narrow allowlist, not a general "is this a
/// current CAPCO dissem control?" predicate. E006 uses it as a guard
/// because the migration table can also contain non-dissem replacements
/// (for example, declass-shorthand entries like `25X1-` → `25X1`, which
/// E007 owns), and those MUST NOT dispatch as E006. Active dissem
/// controls absent from this allowlist (e.g., FOUO) simply never appear
/// as a replacement today — adding one is a deliberate E006 scope change,
/// not a passive widening.
///
/// `CUI` is intentionally excluded. Per CAPCO-2016 §F (and
/// `CVEnumISMDissem.xml`), `CUI` is not a CAPCO dissem control — it is a
/// NARA marking system. No MIGRATIONS entry currently has `CUI` as a
/// replacement (a prior `FOUO → CUI` entry was removed as factually
/// incorrect; see `crates/ism/build.rs` MIGRATIONS doc block). Keeping
/// `CUI` out of this set defends against re-introduction.
fn is_dissem_replacement(replacement: &str) -> bool {
    matches!(
        replacement,
        "RELIDO" | "NOFORN" | "ORCON" | "IMCON" | "DEA SENSITIVE" | "PROPIN"
    )
}

// ---------------------------------------------------------------------------
// Rule: E007 — X-shorthand declassification date
// ---------------------------------------------------------------------------

/// CAPCO X-shorthand declass codes (e.g., `25X1-`, `25X2-`, `50X1-`,
/// `50X1-HUM-`) are deprecated in favor of the canonical forms (`25X1`,
/// `50X1-HUM`, etc.). The deprecated dashed form is not in the CVE, so
/// the parser surfaces it as `TokenKind::Unknown`. E007 walks Unknown
/// tokens via two paths:
///
/// 1. **Migration table lookup**: exact match in the seed `MIGRATIONS`
///    table (e.g., `25X1-` → `25X1`, `50X1-` → `50X1-HUM`). This path
///    uses the table's authoritative confidence and reference.
/// 2. **Pattern match** (fallback): any `TokenKind::Unknown` whose text
///    matches the `\d+X\d+(-[A-Z]+)?-` shape — i.e., a CAPCO
///    X-shorthand form with a trailing `-`. This catches forms the
///    seed table does not enumerate (e.g., `25X2-`, `25X5-`, `25X9-`).
///    The suggested replacement is the text with the trailing `-`
///    stripped; confidence is 0.95 (slightly lower than the 0.97 used
///    for table-backed matches to reflect the lack of an authoritative
///    replacement mapping).
struct XShorthandDateRule;

/// Citations E007 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E007_AUTHORITIES: &[Citation] = &[capco(SectionLetter::E, 6, 33)];

impl Rule<CapcoScheme> for XShorthandDateRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.metadata.x-shorthand-date-pattern")
    }
    fn name(&self) -> &'static str {
        "x-shorthand-date"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::Localized: each fix rewrites a single `Unknown` token in
    /// place — either a migration-table hit or a pattern-stripped
    /// `25X1-` → `25X1` style derivation. Span is the token the rule
    /// walked.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E007_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let mut diagnostics = Vec::new();
        for token in attrs.token_spans.iter() {
            if token.kind != TokenKind::Unknown {
                continue;
            }
            let text = token.text.as_ref();

            // Path 1: exact migration-table match. Uses the table's
            // authoritative replacement and reference. Skips entries
            // owned by E006 (dissem deprecations).
            if let Some(entry) = find_migration(text) {
                if is_dissem_replacement(entry.replacement) {
                    continue;
                }
                // G13: original `text` and `entry.replacement` do not
                // flow into the typed `Message`; the canonical
                // replacement still rides on `Diagnostic.text_correction.replacement`.
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::MigrationTable,
                    span: token.span,
                    message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                    citation: capco(SectionLetter::E, 6, 33),
                    original: text.to_owned(),
                    replacement: entry.replacement.to_owned(),
                    confidence: entry.confidence,
                    migration_ref: Some(entry.reference),
                }));
                continue;
            }

            // Path 2: pattern match for X-shorthand forms not in the
            // seed migration table (e.g., `25X2-`, `25X5-`, `25X9-`).
            // Strip the trailing `-` to produce the canonical form.
            if looks_like_deprecated_x_shorthand(text) {
                let replacement = text.trim_end_matches('-').to_owned();
                if replacement.is_empty() {
                    continue;
                }
                // G13: pattern-derived `replacement` is on the audit
                // permitted list (canonical form, deterministic
                // stripping). The typed `Message` carries no args
                // for this path — the template label identifies the
                // migration class.
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::MigrationTable,
                    span: token.span,
                    message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                    citation: capco(SectionLetter::E, 6, 33),
                    original: text.to_owned(),
                    replacement,
                    // 0.95: slightly below table-backed 0.97 because
                    // the canonical form is derived by pattern stripping
                    // rather than an authoritative CVE mapping.
                    confidence: 0.95,
                    migration_ref: None,
                }));
            }
        }
        diagnostics
    }
}

/// Returns `true` if `s` looks like a DEPRECATED CAPCO X-shorthand
/// declassification form — specifically a canonical form with a
/// trailing `-`.
///
/// Matched patterns:
/// - `NNXNN-`             (e.g., `25X1-`, `25X2-`, `50X1-`)
/// - `NNXNN-AAA-`         (e.g., `50X1-HUM-`, `25X9-WMD-`)
///
/// The canonical (modern) forms (`25X1`, `50X1-HUM`) are in the CVE and
/// parse as `DeclassExemption`, so they never reach this function via
/// the `TokenKind::Unknown` walk.
///
/// Used by both E007 (to emit) and E008 (to skip) so the two rules
/// cannot drift on which tokens each owns.
fn looks_like_deprecated_x_shorthand(s: &str) -> bool {
    let bytes = s.as_bytes();
    // Must end with `-`.
    if bytes.last() != Some(&b'-') {
        return false;
    }
    let inner = &bytes[..bytes.len() - 1];
    if inner.is_empty() {
        return false;
    }
    let mut i = 0;
    // Leading digits.
    while i < inner.len() && inner[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i >= inner.len() {
        return false;
    }
    // `X` separator.
    if inner[i] != b'X' {
        return false;
    }
    i += 1;
    // One or more digits after `X`.
    let start_digits = i;
    while i < inner.len() && inner[i].is_ascii_digit() {
        i += 1;
    }
    if i == start_digits {
        return false;
    }
    // Optional `-LETTERS` suffix (e.g., `-HUM`, `-WMD`).
    if i == inner.len() {
        return true;
    }
    if inner[i] != b'-' {
        return false;
    }
    i += 1;
    while i < inner.len() {
        if !inner[i].is_ascii_uppercase() {
            return false;
        }
        i += 1;
    }
    true
}

/// Whether an `Unknown` token matches the repeated-SAR shape that E008
/// suppresses in favor of E030.
///
/// This helper intentionally implements only the subset of checks needed
/// here — a cheap, string-only predicate on the `Unknown` token itself:
///   - A first SAR parsed successfully (`attrs.sar_markings.is_some()`).
///   - The Unknown text starts with `SAR-` or `SPECIAL ACCESS REQUIRED-`.
///   - The suffix after the prefix is non-empty.
///
/// `SarIndicatorRepeatRule::check` applies additional gates before it
/// emits (preceding-Separator lookup, byte-contiguity between the
/// separator and the Unknown token). Those gates are kept inside E030
/// — when they fail E030 emits a no-fix diagnostic so the shape is
/// still surfaced to the user rather than being silently dropped. This
/// helper therefore does NOT need to model them.
///
/// When any of this helper's checks fails, E008 must fire — the token
/// is not something E030 treats as a repeated-SAR shape. Without this
/// gate, a malformed first SAR like `SAR-` (empty program) would be
/// silently dropped: E030 early-exits on `sar_markings.is_none()`, and
/// E008's old prefix-only suppression would swallow the token.
fn is_repeated_sar_owned_by_e030(text: &str, has_first_sar: bool) -> bool {
    if !has_first_sar {
        return false;
    }
    let suffix = if let Some(rest) = text.strip_prefix("SAR-") {
        rest
    } else if let Some(rest) = text.strip_prefix("SPECIAL ACCESS REQUIRED-") {
        rest
    } else {
        return false;
    };
    !suffix.is_empty()
}

// ---------------------------------------------------------------------------
// Rule: E008 — Unrecognized token inside marking
// ---------------------------------------------------------------------------

/// FR-012: any token inside a marking candidate boundary that the parser
/// could not classify is reported as an error with no fix offered.
///
/// Authority: CAPCO-2016 §G.1 (Register of Authorized Markings, p36):
/// "All markings used in a banner line and portion mark must be in
/// accordance with the values listed in the Register, unless a waiver
/// has been obtained from P&S/IMD in accordance with ICD 710 and
/// applicable ICS." Any token not matching a Register entry (or an
/// Annex A/B code, or a structurally-valid SCI/SAR/REL TO composition)
/// is by definition unauthorized and must be surfaced.
///
/// Suppression paths (an `Unknown` that hits any is NOT unrecognized —
/// another rule owns it):
///
/// 1. **Migration-table hit** — deprecated forms like `25X1-` that
///    `crates/ism/build.rs` MIGRATIONS captures. E007 (X-shorthand)
///    or E006 (migrated-dissem) fires instead.
/// 2. **X-shorthand pattern** — any `\d+X\d+(-[A-Z]+)?-` shape the
///    seed table does not enumerate (e.g., `25X2-`, `25X9-`). E007
///    catches these via its pattern fallback.
/// 3. **Repeated SAR block** — when a first SAR parsed successfully
///    into `attrs.sar_markings`, the parser tags every subsequent
///    same-marking SAR block as `Unknown` whose text starts with
///    `SAR-` or `SPECIAL ACCESS REQUIRED-` AND has a non-empty
///    suffix. E030 (sar-indicator-repeat) owns those; E008 steps
///    aside. The suppression predicate matches the token-shape
///    preconditions `SarIndicatorRepeatRule::check` keys on: it
///    only applies when `attrs.sar_markings.is_some()` and the
///    stripped SAR suffix is non-empty, so a malformed FIRST SAR
///    block — which leaves `sar_markings = None` or has an empty
///    suffix — still fires E008. Without this tightening a marking
///    like `SECRET//SAR-` would be silently dropped: the first SAR
///    fails grammar (no `SarMarking` produced), E008's old
///    prefix-only suppression matched anyway, and E030 early-exited
///    on its `attrs.sar_markings.is_none()` gate. Note E030 also
///    applies a byte-contiguity gate between the Unknown token and
///    its preceding separator; this helper does not model that gate
///    because E030 emits a no-fix diagnostic when contiguity fails,
///    so the shape is still surfaced to the user.
///
/// Malformed SCI-shaped tokens the structural subparser rejected
/// (e.g., `SI-`, `SI--G`) DO fire E008 — users see a real error,
/// not a silent fallback.
struct UnknownTokenRule;

/// Citations E008 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E008_AUTHORITIES: &[Citation] = &[capco(SectionLetter::G, 1, 36)];

impl Rule<CapcoScheme> for UnknownTokenRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "marking.metadata.unrecognized-token")
    }
    fn name(&self) -> &'static str {
        "unrecognized-token"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::WholeMarking: no fix is emitted (FR-012); diagnostics
    /// point at a single `Unknown` span but the firing decision reads
    /// cross-token state (`attrs.sar_markings.is_some()` to suppress
    /// repeated-SAR shapes E030 owns). Default to whole-marking per
    /// D-7.2 — the dispatch consequence is conservative.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E008_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Precompute whether a first SAR block parsed successfully. The
        // repeated-SAR suppression path below must only fire when E030's
        // own token-shape preconditions are met; otherwise a malformed
        // FIRST SAR block would be silently dropped (E030 early-exits,
        // E008 suppresses). The relevant gates inside
        // `SarIndicatorRepeatRule::check` are the `attrs.sar_markings
        // .is_none()` early-exit and the `stripped.is_empty()` skip.
        let has_first_sar = attrs.sar_markings.is_some();
        attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Unknown)
            // Skip entries that E006/E007/E030 will pick up. Three paths:
            //   1. Migration-table hit (covers LIMDIS/FOUO for E006 and
            //      25X1-/50X1- for E007).
            //   2. Pattern-matched X-shorthand with a trailing `-` for
            //      forms not in the seed table (25X2-, 25X9-, etc.).
            //   3. A repeated SAR category block — but ONLY when a
            //      first SAR succeeded AND the stripped suffix is
            //      non-empty (E030's actual preconditions). A
            //      malformed first SAR like `SAR-` (empty suffix)
            //      must still fire E008, not be silently swallowed.
            // An Unknown that hits any path is not "unrecognized" — it
            // is a deprecated or structurally-owned form another rule
            // will surface.
            .filter(|t| {
                let text = t.text.as_ref();
                // Note: malformed SCI-shaped tokens (e.g., `SI-`, `SI--G`)
                // that the structural subparser rejected DO fire E008 —
                // the user sees a real diagnostic instead of a silent
                // fallback. Only suppress well-known specialized paths.
                //
                // Issue #407: bare canonical compound forms (CNWDI / NK /
                // EU in SCI position) are owned by E067
                // (`BareCanonicalCompoundRule`); suppress E008 co-fire
                // so the user sees only the actionable E067
                // `text_correction` diagnostic, not a redundant
                // "unrecognized token" Error.
                //
                // Issue #501: invalid FGI ownership tokens (e.g.,
                // `"FGI FVEY"`, `"FGI DEUX"`, `"FOREIGN GOVERNMENT
                // INFORMATION ACGU"`) are owned by E073
                // (`FgiInvalidOwnershipTokenRule`); suppress E008 co-
                // fire so the user sees only the actionable, category-
                // specific E073 diagnostic.
                find_migration(text).is_none()
                    && !looks_like_deprecated_x_shorthand(text)
                    && !is_repeated_sar_owned_by_e030(text, has_first_sar)
                    && !crate::rules_declarative::is_bare_canonical_compound_form(text)
                    && !is_fgi_invalid_ownership_token(text)
            })
            .map(|t| {
                Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    t.span,
                    Message::new(MessageTemplate::UnrecognizedToken, MessageArgs::default()),
                    capco(SectionLetter::G, 1, 36),
                    None, // FR-012: no fix offered
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Rule: C001 — Corrections-map typo replacement
// ---------------------------------------------------------------------------

/// Scans token spans against the organization-specific corrections map from
/// `[corrections]` in `.marque.toml`. Each match produces a fix proposal with
/// `FixSource::CorrectionsMap` and `confidence = 1.0`.
///
/// # Not a CAPCO rule
///
/// C001 is intentionally NOT anchored to a CAPCO passage. No CAPCO section
/// governs user-defined typo replacements — they are organization-specific
/// mappings supplied through `.marque.toml`. The citation string
/// [`marque_rules::CORRECTIONS_MAP_CITATION`] (`"CONFIG:[corrections]"`) is
/// a config pointer rather than a §/page/line reference. This is deliberate
/// and Constitution VIII-compliant: fabricating a CAPCO citation for a
/// user-defined mapping would be worse than no citation. Auditors
/// distinguish C001 fixes from CAPCO-authoritative fixes via
/// `FixSource::CorrectionsMap` in the audit record.
///
/// # FR-009 precedence (spec: `specs/001-marque-mvp/spec.md` §Functional
/// Requirements, FR-009)
///
/// User corrections take precedence over built-in rules on the same span.
/// This is automatic under FR-016 sort order — `"C001" < "E001"`
/// lexicographically, so C001 wins under the C-1 overlap guard. No
/// special-case code in the engine; the invariant falls out of the sort
/// key alone. Exercised by
/// `fr009_c001_wins_over_builtin_rule_on_same_span` in
/// `crates/capco/tests/corrections_map.rs`.
///
/// # `migration_ref = None`
///
/// C001 emits `migration_ref: None`. `migration_ref` identifies a
/// deterministic migration-table entry (FR-004a, `FixSource::MigrationTable`)
/// — C001 is a user map, not an ODNI migration, so there is no ref to
/// carry. PR #6 review explicitly rejected the earlier
/// `Some("corrections-map")` placeholder; the `FixSource` enum already
/// distinguishes provenance without a string label.
///
/// # Emission paths
///
/// Two call sites emit C001 diagnostics:
/// 1. This rule's `check` method — triggered when the scanner detected a
///    marking and the parser produced a `TokenSpan` whose text matches a
///    corrections key.
/// 2. `Engine::lint` pre-scanner text scan — triggered when the scanner
///    missed a marking (e.g., `SERCET//NF` whose classification prefix is
///    not recognized). Both paths use
///    [`marque_rules::CORRECTIONS_MAP_CITATION`] so the audit record shape
///    is identical.
struct CorrectionsMapRule;

/// Citations C001 may emit on diagnostics. C001 is **not** a CAPCO
/// rule — it surfaces user-defined `[corrections]` map entries, so
/// its citation is the [`AuthoritativeSource::Config`] sentinel
/// (`[config]`) rather than a §/page reference. See
/// [`marque_rules::CORRECTIONS_MAP_CITATION`] and
/// [`Rule::cited_authorities`] for the F.1 gate contract.
const C001_AUTHORITIES: &[Citation] = &[marque_rules::CORRECTIONS_MAP_CITATION];

impl Rule<CapcoScheme> for CorrectionsMapRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "marking.correction.token-typo")
    }
    fn name(&self) -> &'static str {
        "corrections-map"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    /// Phase::Localized: each fix replaces a single `TokenSpan` with the
    /// user-configured `[corrections]` mapping (e.g. `SERCET → SECRET`).
    /// Span is strictly one token.
    ///
    /// Architecturally C001 also runs as a separate pre-pass-0 in
    /// `Engine::fix_inner` (text-correction Aho-Corasick scan against
    /// raw bytes before parsing — `docs/refactor-006/pr-7-architect-plan.md`
    /// §3.5). The phase tag governs the rule-dispatch path; the
    /// pre-pass-0 path is a separate channel that bypasses rule
    /// dispatch entirely.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        C001_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Engine guarantees corrections is Some only when the map is non-empty
        // (engine.rs: corrections_arc is None when config.corrections.is_empty()).
        let Some(corrections) = ctx.corrections.as_ref() else {
            return vec![];
        };

        let mut diagnostics = Vec::new();
        for token_span in attrs.token_spans.iter() {
            // M1: skip structural separators — corrections never apply to "//"
            if token_span.kind == TokenKind::Separator {
                continue;
            }
            let text = token_span.text.as_str();
            let Some(replacement) = corrections.get(text) else {
                continue;
            };
            // M2: skip no-op corrections (replacement == original)
            if replacement == text {
                continue;
            }
            // G13: drop the runtime byte text from the message per
            // PM-C-5. Original document bytes (`text`) and the
            // user-config replacement (`replacement`) do not flow into
            // the typed `Message` — `MessageArgs.token` would need a
            // `TokenId` projection that does not exist for arbitrary
            // user-config `String → String` mappings. The closed-template
            // label identifies the corrections-map class; the canonical
            // replacement still rides on `Diagnostic.text_correction.replacement`
            // for the engine's apply path.
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::CorrectionsMap,
                span: token_span.span,
                message: Message::new(MessageTemplate::CorrectionsApplied, MessageArgs::default()),
                citation: marque_rules::CORRECTIONS_MAP_CITATION,
                original: text.to_owned(),
                replacement: replacement.clone(),
                confidence: 1.0,
                migration_ref: None,
            }));
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Rule: S003 — joint-usa-first (style)
// ---------------------------------------------------------------------------

/// S003: Prefer USA first in JOINT country lists.
///
/// # Authority: convention, not §H.3
///
/// CAPCO-2016 §H.3 p56 prescribes **pure alphabetical** order
/// for JOINT country lists ("Country trigraph codes are listed
/// alphabetically followed by tetragraph codes in alphabetical order").
/// The section has NO USA-first carve-out. Prior to PR #97 / T035c-18,
/// the pre-decomposition JOINT fix path (then E020, later folded into
/// E060 — both retired) incorrectly elevated USA to the front — that
/// was an authority-drift violation of Constitution VIII. #97 narrowed
/// the JOINT canonicalization path to pure alpha; PR 3c.B Commit 6
/// retired the rule-side path entirely into `render_classification.rs`.
///
/// However, every other US-authored country list **does** lead with
/// USA — REL TO §H.8 pp 150–151 is explicit ("After 'USA',
/// list the required one or more trigraph country codes..."). The IC
/// practice of rendering USA first in JOINT lists is a widespread
/// convention that extends this REL-TO pattern across all
/// country-list contexts, even where CAPCO is silent.
///
/// S003 encodes that convention as a **style rule** (`Severity::Info`
/// by default). It does not claim §H.3 authority; the rule doc and
/// diagnostic citation make the "convention, not mandate" framing
/// explicit. Orgs that want strict §H.3 conformance can disable S003
/// via `S003 = "off"` in `.marque.toml`. Orgs that want USA-first
/// auto-applied can configure `S003 = "fix"`.
///
/// # Predicate
///
/// Fires on a banner-context `MarkingClassification::Joint` when the
/// country list contains USA AND USA is NOT the first country. The
/// rule only fires on banners (matching S001/S002's banner-only
/// scope) — portion-form JOINT is rarely used, and applying
/// convention-based style to portions is a judgment call best
/// deferred.
///
/// # Interaction with E060 (JOINT row)
///
/// E060's JOINT row and S003 can both fire on the same JOINT list
/// when it is neither pure-alpha nor USA-first (e.g., `GBR USA AUS`
/// is not alpha AND not USA-first). Both fixes target the same
/// Classification token span:
///
/// - E060 (non-canonical input walker, JOINT row) fix: `AUS GBR USA`
///   (pure alpha per §H.3 p56).
/// - S003 fix: `USA AUS GBR` (USA first, rest alpha per convention).
///
/// Under FR-016's rule-id tiebreaker ("E060" < "S003" lexically),
/// E060 wins the overlap guard and applies. On re-lint, E060 is
/// silent (list now pure-alpha) and S003 still wants USA first;
/// running fix again converges to `USA AUS GBR`. Two passes. Orgs
/// that want single-pass USA-first convergence can disable E060
/// for JOINT (currently not configurable; would need a per-list-type
/// severity override — follow-up).
///
/// (Pre-PR-3b.F this was E020; PR 3b.F retired E020 into the E060
/// walker, which preserved the same fix shape and citation but
/// changed the rule-ID. PR 3c.B Commit 6 retired E060 into the
/// renderer.)
///
/// # Constitution V audit-content-ignorance
///
/// The message at lines below interpolates `joined_actual_str` and
/// `joined_canonical_str`, both derived from
/// `CountryCode::as_str()` over the *parsed* country list — those
/// strings are CVE-canonical trigraphs (`USA`, `GBR`, `CAN`, …)
/// drawn from the closed ODNI `CVEnumISMCATRelTo` set, **not**
/// document text. Post-Commit-10 the audit record carries no
/// document bytes for this path: the `Diagnostic.fix` field is a
/// `FixIntent` whose `replacement` is
/// `Recanonicalize { RecanonScope::Page }`, the `Confidence` is a
/// scalar, and the `Message` is a closed template + closed args.
/// The pre-Commit-10 `FixProposal.original` byte channel retired
/// with the `mvp-2 → mvp-3` schema flip; the structural payload
/// closes G13 by construction at this rule's emission site.
struct JointUsaFirstRule;

/// S003 secondary CAPCO §-citations.
///
/// PR 10.A.1 Commit 4: the migration to typed `Citation` collapsed the
/// pre-migration string form `"CAPCO-2016 §H.3 p56 + §H.8 pp 150-151 (IC convention)"`
/// into a single `capco(SectionLetter::H, 3, 56)` value on the emitted
/// diagnostic. The cross-reference to §H.8 p150 (REL TO USA-first
/// convention — the analogous IC convention S003 layers above §H.3's
/// pure-alpha JOINT default) survived in the rule's doc-comment but
/// was un-checked. This constant pins the dropped cross-reference
/// structurally.
///
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1
/// Commit 4 authorship per Constitution VIII propagation rule: §H.8
/// pp 150-151 establish the REL TO USA-first convention ("USA first,
/// remaining trigraphs alphabetical") that S003 ports to JOINT
/// classifications. Anchor citation uses p150 since typed `Citation`
/// holds a single page; the range "pp 150-151" lives in the rule
/// doc-comment.
///
/// `#[allow(dead_code)]`: see [`E005_CROSS_REFS`] for the rationale —
/// this is rule-authoritative metadata read by
/// `citation_cross_refs_tests` (bottom of this file). The runtime
/// `Rule::cited_authorities()` surface reads [`S003_AUTHORITIES`]
/// instead, which combines the primary `§H.3 p56` anchor with the
/// `S003_CROSS_REFS` cross-references in one slice.
#[allow(dead_code)]
pub(crate) const S003_CROSS_REFS: &[Citation] = &[capco(SectionLetter::H, 8, 150)];

/// Citations S003 may emit on diagnostics. Primary anchor §H.3 p56
/// (the JOINT pure-alpha rule the IC convention layers above) plus
/// the §H.8 p150 REL TO precedent S003 ports forward. See
/// [`Rule::cited_authorities`] for the F.1 gate contract.
const S003_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::H, 3, 56),
    capco(SectionLetter::H, 8, 150),
];

impl Rule<CapcoScheme> for JointUsaFirstRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.classification.joint-usa-first-style")
    }
    fn name(&self) -> &'static str {
        "joint-usa-first"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    /// Phase::WholeMarking: emits `ReplacementIntent::Recanonicalize`
    /// at `RecanonScope::Page`; the engine re-renders the JOINT
    /// classification across the candidate scope.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        S003_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{CountryCode, MarkingType};
        if ctx.marking_type != MarkingType::Banner {
            return vec![];
        }
        let Some(MarkingClassification::Joint(j)) = &attrs.classification else {
            return vec![];
        };
        if j.countries.len() < 2 {
            // Single-country JOINT (or zero) can't have USA out of
            // first position meaningfully.
            return vec![];
        }
        if !j.countries.contains(&CountryCode::USA) {
            // JOINT without USA is anomalous per §H.3 p163 but
            // not S003's concern. Let other rules flag it.
            return vec![];
        }
        if j.countries.first() == Some(&CountryCode::USA) {
            return vec![];
        }

        // Canonicalize: USA first, remaining trigraphs alphabetical.
        let canonical = canonicalize_trigraph_list(&j.countries, true);

        // Locate the `Classification` token to anchor the diagnostic
        // span; the replacement-bytes computation retired with the
        // mvp-3 cutover (the engine's `render_canonical` produces
        // canonical JOINT bytes at fix-application time).
        let Some(classification_tok) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
        else {
            return vec![];
        };

        // G13: drop the runtime country lists from the message;
        // they appeared in the format!-built string but are document
        // bytes by way of `j.countries.iter().map(|t| t.as_str())`.
        // The typed `Message` identifies the ordering-violation class
        // for the JOINT axis.
        let _ = canonical; // canonical is consumed by the fix_intent path below
        let message = Message::new(
            MessageTemplate::NonCanonicalOrder,
            MessageArgs {
                category: Some(crate::scheme::CAT_JOINT_CLASSIFICATION),
                ..MessageArgs::default()
            },
        );

        // PR 3c.B Commit 10 — structural FixIntent only. JOINT
        // classification rendering is a page-scope concern (the
        // banner-line classification axis); the convention is layered
        // above the renderer's §H.3 pure-alpha default.
        //
        // Citation: §H.3 p56 prescribes pure alphabetical for JOINT
        // with no USA-first carve-out; S003 encodes the IC convention
        // observed across REL TO (§H.8 pp 150-151). Typed Citation
        // anchors at §H.3 p56; the cross-reference to §H.8 lives in
        // the rule doc comment.
        let citation = capco(SectionLetter::H, 3, 56);
        let fix_intent = FixIntent {
            replacement: ReplacementIntent::Recanonicalize {
                scope: RecanonScope::Page,
            },
            confidence: Confidence::strict(1.0),
            feature_ids: Default::default(),
            message: Message::new(MessageTemplate::NonCanonicalOrder, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            classification_tok.span,
            ctx.candidate_span,
            message,
            citation,
            fix_intent,
        )]
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
// Rule: W003 — Non-IC dissem in classified banner
// ---------------------------------------------------------------------------

/// Some non-IC dissemination controls must not appear in classified banners.
///
/// Per CAPCO-2016 §H.9 "Precedence Rules for Banner Line Guidance" (see
/// the per-marking rows on [`marque_ism::NonIcDissem::propagates_to_classified_banner`]):
///
/// - **Propagate to classified banners** (no W003): EXDIS, NODIS, LES,
///   LES-NF, SSI.
/// - **Do NOT propagate** (W003 fires): LIMDIS, SBU, SBU-NF. These
///   markings are "applicable only to unclassified information" per
///   §H.9 and their precedence rules explicitly say the marking is
///   stripped from the banner when the document is classified.
///
/// W003 is banner-only — a non-IC dissem control in a *portion* marking
/// is fine at any classification.
///
/// ## Important Exceptions
///
/// `LES-NF` has a further §H.9 canonicalization — the banner form
/// `SECRET//NOFORN//LES` rather than `SECRET//LES NOFORN`. That split
/// is a page-rewrite concern, not a W003 concern, so LES-NF is
/// considered propagating here.
///
/// Importantly, SBU-NF behaves similarly to LES-NF. 'SBU'
/// never propagates to a classified marking, but its
/// `NF` attribute *does*.
struct NonIcInClassifiedBannerRule;

/// Citations W003 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const W003_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 9, 169)];

impl Rule<CapcoScheme> for NonIcInClassifiedBannerRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.non-ic-dissem-in-classified-banner")
    }
    fn name(&self) -> &'static str {
        "non-ic-dissem-in-classified-banner"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: banner-only decision reading the
    /// classification axis × non-IC dissem axis together; emits no fix
    /// (the SBU/LIMDIS removal is intentionally manual).
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        W003_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::MarkingType;
        if ctx.marking_type != MarkingType::Banner {
            return vec![];
        }

        if attrs.non_ic_dissem.is_empty() {
            return vec![];
        }

        // Non-IC dissem controls are fine only in UNCLASSIFIED banners.
        // Determine classification from the full banner classification, not
        // just the US-specific view, so non-US classified banners (NATO,
        // JOINT, FGI forms) are also checked.
        let is_classified = match &attrs.classification {
            Some(marque_ism::MarkingClassification::Us(c)) => {
                *c > marque_ism::Classification::Unclassified
            }
            Some(
                marque_ism::MarkingClassification::Fgi(_)
                | marque_ism::MarkingClassification::Nato(_)
                | marque_ism::MarkingClassification::Joint(_)
                | marque_ism::MarkingClassification::Conflict { .. },
            ) => true,
            None => false,
        };
        if !is_classified {
            return vec![];
        }

        let mut diagnostics = Vec::new();
        let nic_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::NonIcDissem)
            .collect();

        for (idx, nic) in attrs.non_ic_dissem.iter().enumerate() {
            // LIMDIS, LES, LES-NF, SSI propagate to classified banners.
            if nic.propagates_to_classified_banner() {
                continue;
            }

            let span = nic_spans
                .get(idx)
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));

            // G13: drop the runtime token-text interpolation. Template
            // identifies the violation class; the affected category is
            // CAT_NON_IC_DISSEM.
            let _ = nic; // emit-class is known without the runtime value
            diagnostics.push(Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                Message::new(
                    MessageTemplate::NonIcDissemInClassifiedBanner,
                    MessageArgs {
                        category: Some(crate::scheme::CAT_NON_IC_DISSEM),
                        ..MessageArgs::default()
                    },
                ),
                capco(SectionLetter::H, 9, 169),
                None,
            ));
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

/// Collect program identifiers that appear in `expected` but not in
/// `observed`.
///
/// Compares by program identifier only. Compartments and sub-compartments
/// are deliberately NOT compared — per CAPCO-2016 §H.5 p101
/// and §H.5 p99, banner hierarchy depiction below the program
/// level is optional even when portions carry hierarchy. A banner showing
/// `SAR-BP` when a portion shows `SAR-BP-J12` is therefore compliant and
/// must not be flagged.
///
/// Returns borrowed `&str` views into `expected.programs[i].identifier`.
/// The caller uses these only for (a) the diagnostic message and (b)
/// the insertion-fix replacement string; neither path needs the
/// expected-side compartment/sub-compartment hierarchy, so returning
/// owned `SarProgram` clones would be unnecessary allocation.
fn sar_missing_programs<'a>(
    observed: Option<&marque_ism::SarMarking>,
    expected: &'a marque_ism::SarMarking,
) -> Vec<&'a str> {
    let observed_ids: HashSet<&str> = match observed {
        Some(obs) => obs.programs.iter().map(|p| p.identifier.as_str()).collect(),
        None => HashSet::new(),
    };

    expected
        .programs
        .iter()
        .filter(|p| !observed_ids.contains(p.identifier.as_str()))
        .map(|p| p.identifier.as_str())
        .collect()
}

// ---------------------------------------------------------------------------
// Rule: W034 — SCI custom-control audit visibility
// ---------------------------------------------------------------------------

/// Per CAPCO-2016 §A.6 p16 + §H.4 p61: unpublished (agency-allocated) SCI
/// control systems are legitimate — the manual describes ODNI/P&S's
/// unpublished registry and explicitly permits these markings. This rule
/// surfaces each Custom control identifier so a classifier can verify the
/// allocation is registered.
///
/// # Severity: Warn (default)
///
/// Field experience: the four spelled-out SCI controls in CAPCO (SI, TK,
/// RSV, HCS) account for the vast majority (>99%) of real-world SCI
/// control usage. Seeing an unpublished control is more likely a typo,
/// stale legacy marking, or unregistered use than a valid agency
/// allocation. `Warn` reflects that rarity without making it
/// error-level by default. (Note: `Warn` still produces a non-zero
/// CLI exit via `EX_DIAG_WARN`, so orgs that treat any warning as
/// CI-blocking should configure `W034 = "info"` if they want
/// audit-visibility only.)
///
/// T035c-2 landed the `Severity::Info` variant and dropped the earlier
/// `Severity::Off` workaround. Previously, the rule emitted `Diagnostic`
/// values at `Severity::Off` — a state `Principle IV` declares
/// unrepresentable — and relied on the test harness bypassing
/// engine-level severity filtering to observe the diagnostics. That was
/// a constitutional-invariant violation. Users who want informational
/// (non-warn) treatment can configure `W034 = "info"` in `.marque.toml`;
/// users who want it silent can configure `W034 = "off"`.
struct SciCustomControlInfoRule;

/// Citations W034 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const W034_AUTHORITIES: &[Citation] = &[capco(SectionLetter::A, 6, 16)];

impl Rule<CapcoScheme> for SciCustomControlInfoRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.sci.unpublished-custom-control")
    }
    fn name(&self) -> &'static str {
        "sci-custom-control-info"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: audit-visibility surface for unpublished SCI
    /// control identifiers. No fix emitted; the diagnostic flags every
    /// Custom-control span in the marking. Decision is per-marking, not
    /// per-token.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        W034_AUTHORITIES
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let sys_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciSystem)
            .collect();

        let mut out = Vec::new();
        for (idx, marking) in attrs.sci_markings.iter().enumerate() {
            if let SciControlSystem::Custom(text) = &marking.system {
                // Plausible-allocation suppression: 1-3 ASCII-uppercase
                // identifiers are within the typical CAPCO-2016 §A.6 p15
                // agency-allocated shape and don't warrant per-marking
                // audit-visibility noise. W034 still fires on anything
                // outside this shape (digits, longer identifiers,
                // unusual casing) where the chance of typo or
                // unregistered use is materially higher. Citation:
                // CAPCO-2016 §A.6 p15 (agency-allocated control
                // identifier shape) + §H.4 p61 (publication channel).
                let s = text.as_str();
                let is_plausible_allocation =
                    (1..=3).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_uppercase());
                if is_plausible_allocation {
                    continue;
                }
                let span = sys_spans
                    .get(idx)
                    .map(|t| t.span)
                    .unwrap_or(Span::new(0, 0));
                // G13: drop runtime byte text. Template names the
                // unpublished-control class.
                let _ = s;
                out.push(Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    span,
                    Message::new(
                        MessageTemplate::UnpublishedSciControl,
                        MessageArgs::default(),
                    ),
                    capco(SectionLetter::A, 6, 16),
                    None,
                ));
            }
        }
        out
    }
}

// ===========================================================================
// E061 — Bare HCS at CONFIDENTIAL (class-specific legacy guidance)
// ===========================================================================
//
// §H.4 p62 carries a class-specific note for legacy CONFIDENTIAL//HCS
// information: "When legacy information at the CONFIDENTIAL//HCS level
// is discovered, contact the originator for guidance prior to reusing
// the information." Distinct from the general bare-HCS guidance that
// recommends the HCS-O / HCS-P / HCS-O-P templates (covered by E010).
//
// E061 fires only when classification is CONFIDENTIAL AND a bare HCS
// is present. The diagnostic carries no fix (the manual prescribes
// contacting the originator, not a mechanical re-mark).
//
// Bare HCS is a structurally-incomplete marking, not an invalid one —
// the HCS control system is canonical per §H.4 p62; the user just
// hasn't specified the required compartment. Marque can't pick the
// compartment without content-domain context. Severity::Warn (not
// Error): the marking will be valid once the user adds the compartment;
// the rule's job is to surface the gap, not to claim the marking is
// structurally invalid. Contrast with E065's deprecated-control-system
// rows (bare KDK/KLONDIKE/EL/ENDSEAL/ECI) where the source control
// system itself is retired and the marking has no canonical migration.

/// Rule E061 — bare HCS at CONFIDENTIAL: legacy guidance per §H.4 p62.
struct HcsBareAtConfidentialLegacyRemarkRule;

/// Citations E061 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E061_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 4, 62)];

impl Rule<CapcoScheme> for HcsBareAtConfidentialLegacyRemarkRule {
    fn id(&self) -> RuleId {
        RuleId::new(
            "capco",
            "portion.sci.hcs-bare-at-confidential-legacy-remark",
        )
    }
    fn name(&self) -> &'static str {
        "hcs-bare-at-confidential-legacy-remark"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: needs cross-token classification + SCI
    /// state to determine "bare HCS at CONFIDENTIAL" class-specific
    /// trigger. No fix emitted; the manual prescribes contacting the
    /// originator.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E061_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{Classification, SciControlBare, SciControlSystem};

        // Class-specific gate: only fires at CONFIDENTIAL.
        if attrs.us_classification() != Some(Classification::Confidential) {
            return vec![];
        }

        // Find bare HCS (Published Hcs system with no compartments).
        let bare_hcs_idx = attrs.sci_markings.iter().position(|m| {
            matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
                && m.compartments.is_empty()
        });
        let Some(idx) = bare_hcs_idx else {
            return vec![];
        };

        // Anchor span at the bare HCS SciSystem token. The structural
        // parser emits one `TokenKind::SciSystem` per SCI marking; we
        // index by position to align with the matched `sci_markings`
        // entry. Defensive fallback to `Span::new(0, 0)` if the spans
        // got out of sync (would indicate a parser regression caught
        // elsewhere).
        let sys_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciSystem)
            .collect();
        let span = sys_spans
            .get(idx)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
            capco(SectionLetter::H, 4, 62),
            None,
        )]
    }
}

// ===========================================================================
// E062 — Bare HCS at SECRET / TOP SECRET (legacy form; suggest templates)
// ===========================================================================
//
// §H.4 p62 (general bare-HCS guidance): "When incorporating legacy
// material marked 'HCS' into a new product, re-mark the new document
// and associated portion according to the instructions in the HCS-O
// and HCS-P marking templates."
//
// E062 fires at SECRET / TOP SECRET (the class levels where HCS-O /
// HCS-P / HCS-O-P are authorized). It emits per-candidate Suggest-
// severity diagnostics for HCS-O, HCS-P, and HCS-O-P. The choice
// between them is a content-domain decision Marque cannot make:
// HCS-O is operational source information; HCS-P is analytical
// product; HCS-O-P is both. Surfacing 3 candidates lets the
// classifier pick.
//
// Distinct from E010: E010 fires at any class level with a single
// text-only "consult HCS-O/HCS-P templates" message. E062 emits
// per-candidate text_corrections so editors can offer one-click
// substitution. Orgs that want either rule silenced configure
// `.marque.toml [rules] E062 = "off"` (or E010 = "off").

/// Rule E062 — bare HCS at S/TS: suggest HCS-O / HCS-P / HCS-O-P
/// templates per §H.4 p62.
struct HcsBareSuggestSubcompartmentRule;

/// Citations E062 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E062_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 4, 62)];

impl Rule<CapcoScheme> for HcsBareSuggestSubcompartmentRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.sci.hcs-bare-suggest-subcompartment")
    }
    fn name(&self) -> &'static str {
        "hcs-bare-suggest-subcompartment"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: needs cross-token classification + SCI
    /// state to gate "S/TS class level". Emits per-candidate
    /// text_corrections at Suggest severity so the engine never
    /// auto-applies; the classifier picks via UI.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E062_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{Classification, SciControlBare, SciControlSystem};

        // Class-specific gate: only fires at SECRET / TOP SECRET.
        let class = attrs.us_classification();
        if !matches!(
            class,
            Some(Classification::Secret) | Some(Classification::TopSecret)
        ) {
            return vec![];
        }

        // Find bare HCS (Published Hcs system with no compartments).
        let Some(idx) = attrs.sci_markings.iter().position(|m| {
            matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
                && m.compartments.is_empty()
        }) else {
            return vec![];
        };

        let sys_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciSystem)
            .collect();
        let span = sys_spans
            .get(idx)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        // Emit per-candidate Suggest-severity diagnostics. Each carries
        // a text_correction whose `replacement` is the canonical short
        // form for the matching sub-compartment. The engine never
        // auto-applies Suggest-severity diagnostics by construction
        // (Severity::Suggest is a hard exclusion in Engine::fix); the
        // candidates surface in the editor / CLI for human selection.
        //
        // Diagnostics emit at Severity::Suggest by default — the engine
        // preserves the per-diagnostic severity when no
        // `.marque.toml [rules] E062 = "..."` override is configured
        // (engine.rs:1001-1007 applies the override only when present).
        // Suggest prevents auto-apply, so the classifier picks among
        // the three candidates. To escalate to Warn or Error at the
        // user surface, the operator configures
        // `[rules] E062 = "warn"` in `.marque.toml`.
        let candidates: &[&str] = &["HCS-O", "HCS-P", "HCS-O-P"];
        let mut out = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            // G13: candidate replacement is on the audit permitted list
            // (canonical token from a closed set); the typed `Message`
            // identifies the superseded-token class.
            out.push(Diagnostic::text_correction(
                self.id(),
                Severity::Suggest,
                span,
                Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                capco(SectionLetter::H, 4, 62),
                *candidate,
                FixSource::BuiltinRule,
                // Confidence 0.75: the canonical replacement is one of
                // three, and Marque cannot pick the right one. The
                // value is below typical auto-apply thresholds (0.95)
                // so even an engine that ignored the Suggest gate
                // would not auto-apply.
                Confidence::strict(0.75),
                None,
            ));
        }
        out
    }
}

// ===========================================================================
// E063 — Bare RSV requires compartment (§H.4 p70)
// ===========================================================================
//
// §H.4 p70: "the RSV marking may not be used alone and requires the
// associated compartment". §H.4 p72: `RSV-[COMPARTMENT]` (3-alnum),
// TS/S only, requires RESERVE.
//
// Bare RSV is a structurally-incomplete marking, not an invalid one —
// the RESERVE control system is canonical per §H.4 p70; the user just
// hasn't specified the required compartment. Marque can't pick the
// compartment without content-domain context (the compartment identifier
// is org-private and not in the public vocabulary). Severity::Warn (not
// Error): the marking will be valid once the user adds the compartment;
// the rule's job is to surface the gap, not to claim the marking is
// structurally invalid. Contrast with E065's deprecated-control-system
// rows (bare KDK/KLONDIKE/EL/ENDSEAL/ECI) where the source control
// system itself is retired and the marking has no canonical migration.
// Suggest-only (no fix proposed) because the compartment identifier is
// org-private content beyond Marque's vocabulary.

/// Rule E063 — bare RSV requires compartment per §H.4 p70.
struct RsvBareRequiresCompartmentRule;

/// Citations E063 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E063_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 4, 70)];

impl Rule<CapcoScheme> for RsvBareRequiresCompartmentRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.sci.rsv-bare-requires-compartment")
    }
    fn name(&self) -> &'static str {
        "rsv-bare-requires-compartment"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: needs cross-token SCI state to find bare
    /// RSV (no compartment). No fix emitted; the compartment
    /// identifier is org-private content beyond Marque's vocabulary.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E063_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{SciControlBare, SciControlSystem};

        // Find bare RSV (Published Rsv system with no compartments).
        let Some(idx) = attrs.sci_markings.iter().position(|m| {
            matches!(m.system, SciControlSystem::Published(SciControlBare::Rsv))
                && m.compartments.is_empty()
        }) else {
            return vec![];
        };

        let sys_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciSystem)
            .collect();
        let span = sys_spans
            .get(idx)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
            capco(SectionLetter::H, 4, 70),
            None,
        )]
    }
}

// ===========================================================================
// E064 — EYES / EYES ONLY → REL TO conversion (T135a Commit 5)
// ===========================================================================
//
// Authority: CAPCO-2016 §H.8 p157 + §H.8 p158.
//
// §H.8 p157: EYES ONLY is NSA-only and deprecated; the markings waiver
// expired 1 Oct 2017 (post-manual). §H.8 p158: "When extracting EYES
// ONLY portions from SIGINT reporting, convert the EYES ONLY portion
// marks to REL TO" and "carry forward the trigraph/tetragraph codes
// listed in the source document banner line to the new portion mark."
//
// E064 emits a `text_correction` covering the source-bytes of the EYES
// block (the parser preserves `<TRIGRAPHS> EYES [ONLY]` source text
// verbatim in `TokenSpan.text` per the Commit 2 recognizer). The
// replacement is the canonical `REL TO USA, <list>` form: USA
// prepended per §A.6 p16 + §H.8 p150-151 REL TO template, remaining
// codes sorted alphabetically, comma-space delimited per §A.6 p16.
//
// Note: the EYES source format is trigraph-only per §H.8 p157 line
// 3874-3875 ("Country trigraph codes are separated by single forward
// slashes"), so the recognizer rejects tetragraph inputs in the EYES
// prefix. The diagnostic message still mirrors §H.8 p158's
// "trigraph/tetragraph" wording verbatim because that wording refers
// to the carry-forward from the source-document banner line, where
// tetragraphs may legitimately appear. A future page-context-aware
// pass may surface banner-line tetragraphs into REL TO output, but
// is out of PR 9a scope.
//
// Implementation note: cross-axis migration (remove EYES from dissem +
// add trigraphs to rel_to) is not expressible as a single
// `ReplacementIntent` — the intent vocabulary's `FactAdd` /
// `FactRemove` / `Recanonicalize` variants are strictly single-axis-
// scoped. A `FixIntent` mirror of the E041 pattern would either need a
// new `Migrate { from, to, scope }` intent variant (engine/scheme
// edit out of scope here) or an engine-side composition of two atomic
// intents (architectural change beyond Commit 5's scope). The
// `text_correction` channel is the existing route that delivers the
// same user-facing outcome — a byte-precise canonicalization splice
// at the EYES block span. The brief's "FixIntent / mirror E041"
// guidance assumed intra-axis migration shape; the EYES → REL TO
// case is documented as cross-axis in `project_incompatibility_class.md`
// (memory). Selecting the existing text_correction path is the
// citation-honest implementation under today's intent vocabulary.

/// Rule E064 — convert EYES / EYES ONLY portions to REL TO per §H.8 p157.
struct EyesOnlyConvertToRelToRule;

/// Citations E064 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E064_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 8, 157)];

impl Rule<CapcoScheme> for EyesOnlyConvertToRelToRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.dissem.eyes-only-convert-to-rel-to")
    }
    fn name(&self) -> &'static str {
        "eyes-only-convert-to-rel-to"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::Localized: the diagnostic span covers a single
    /// `TokenKind::DissemControl` block (the EYES compound block).
    /// `text_correction` is a byte-precise single-span splice that
    /// fits inside one token boundary — exactly the Localized
    /// contract. Pass-1 applies the fix; the re-parse for pass-2
    /// sees the canonical REL TO output.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E064_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let mut out = Vec::new();
        for token in attrs.token_spans.iter() {
            if token.kind != TokenKind::DissemControl {
                continue;
            }
            // The compound EYES block carries `<trigraph>(/<trigraph>)*
            // EYES [ONLY]`. We detect the compound form by suffix-
            // matching ` EYES ONLY` / ` EYES` (with a space before EYES)
            // so the prefix is the trigraph list. The bare forms (`"EYES"`
            // and `"EYES ONLY"` without any preceding list) are handled
            // by the explicit equality arms below — they do not carry the
            // leading space that `strip_suffix` requires.
            let text = token.text.as_str();
            let (prefix, _full_form) = if let Some(p) = text.strip_suffix(" EYES ONLY") {
                (p, true)
            } else if let Some(p) = text.strip_suffix(" EYES") {
                (p, false)
            } else if text == "EYES ONLY" {
                // Bare ODNI-title form: token text is the full ODNI long
                // description "EYES ONLY" (from MARKING_FORMS banner
                // form). No trigraph prefix — empty prefix triggers the
                // banner-FVEY branch below.
                ("", true)
            } else if text == "EYES" {
                // Bare CVE-value form: token text is the raw CVE value
                // "EYES". Same semantics as bare "EYES ONLY" — no
                // trigraph prefix, banner-FVEY branch below.
                ("", false)
            } else {
                continue;
            };
            if prefix.is_empty() {
                // Bare `EYES` / `EYES ONLY` token — no preceding country
                // list. Semantics differ by marking context:
                //
                // • Banner context: per §H.8 p157, a bare EYES ONLY banner
                //   without a country list implies the full Five Eyes (FVEY)
                //   membership (USA, AUS, CAN, GBR, NZL). Fire E064 with the
                //   FVEY REL TO replacement so the author gets a canonical
                //   conversion rather than a silent, unresolvable token.
                //
                // • Portion context: out of scope. §H.8 p158 says "carry
                //   forward the trigraph codes listed in the source document
                //   banner line" — a bare portion `EYES` is intentionally
                //   abbreviated when the page banner has the full `[LIST]
                //   EYES ONLY` form. Marque cannot synthesize the country
                //   list from the portion alone without banner context.
                //
                // Authority: CAPCO-2016 §H.8 p157 + p158.
                if ctx.marking_type == MarkingType::Banner {
                    out.push(Diagnostic::text_correction(
                        self.id(),
                        self.default_severity(),
                        token.span,
                        Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                        capco(SectionLetter::H, 8, 157),
                        build_rel_to_replacement(&[
                            CountryCode::USA.to_string(),
                            CountryCode::AUS.to_string(),
                            CountryCode::CAN.to_string(),
                            CountryCode::GBR.to_string(),
                            CountryCode::NZL.to_string(),
                        ]),
                        FixSource::BuiltinRule,
                        Confidence::strict(1.0),
                        None,
                    ));
                }
                continue;
            }

            // Parse the trigraph list, USA-first sort the rest.
            let trigraphs = parse_eyes_trigraphs(prefix);
            let canonical = build_rel_to_replacement(&trigraphs);

            // No-op guard: if the trigraph list is somehow empty after
            // sorting (should not happen given the parser's
            // shape gate), skip emission.
            if canonical.is_empty() {
                continue;
            }

            out.push(Diagnostic::text_correction(
                self.id(),
                self.default_severity(),
                token.span,
                Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                capco(SectionLetter::H, 8, 157),
                canonical,
                FixSource::BuiltinRule,
                Confidence::strict(1.0),
                None,
            ));
        }
        out
    }
}

/// Parse the `/`-delimited trigraph prefix of an EYES block into a
/// `Vec<String>`. The prefix is the part before ` EYES` / ` EYES ONLY`.
/// Trigraphs are uppercase 3-letter codes per §H.8 p150-151.
fn parse_eyes_trigraphs(prefix: &str) -> Vec<String> {
    prefix
        .split('/')
        .map(|s| s.to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Build the canonical `REL TO USA, <list>` replacement string.
///
/// Per CAPCO-2016 §A.6 p16 + §H.8 p150-151 the country list begins
/// with USA when USA is present; remaining codes are sorted
/// alphabetically. The list separator is `, ` (comma-space) per
/// §A.6 p16. (§H.3's USA-first rule applies to JOINT's own
/// `[LIST]`, not to REL TO.)
fn build_rel_to_replacement(trigraphs: &[String]) -> String {
    if trigraphs.is_empty() {
        return String::new();
    }
    let mut deduped: Vec<String> = Vec::with_capacity(trigraphs.len());
    for t in trigraphs {
        if !deduped.contains(t) {
            deduped.push(t.clone());
        }
    }
    // After dedup the list is non-empty by virtue of the caller's
    // parser shape gate plus the early-return above; `rest` may be
    // empty (input was just `USA`), but `out` always starts with
    // `REL TO USA`, so no truncated partial output is possible.
    let mut rest: Vec<String> = deduped.into_iter().filter(|t| t != "USA").collect();
    rest.sort();
    let mut out = String::with_capacity(8 + 5 * (rest.len() + 1));
    out.push_str("REL TO USA");
    for code in rest {
        out.push_str(", ");
        out.push_str(&code);
    }
    out
}

// ---------------------------------------------------------------------------
// Rule: S007 — bare NATO classification in a US-classified document
//              should carry `REL TO USA, NATO`.
//
// PR 9c.2 / FR-048. Authority: CAPCO-2016 §H.7 p127 Notional Example 2
// worked example `(//CTS//BOHEMIA//REL TO USA, NATO)` — "a NATO COSMIC
// TOP SECRET (CTS) BOHEMIA portion within a US classified document and
// is releasable back to NATO". The citation is example-derived (no
// "MUST" prose), so the rule is `Severity::Suggest` to match the
// S004 + S005 precedent (post-PR-#488 collapse of the historical
// S005/S006 pair). Users can opt up via `[rules] S007 = "warn"` in
// `.marque.toml` if their org demands stronger surfacing.
//
// The original FR-048 wording mandated a declarative `Constraint`. The
// followup at
// `specs/006-engine-rule-refactor/followups/constraint-context-extension.md`
// tracks why that shape cannot land yet: `MarkingScheme::evaluate_custom`
// (`crates/capco/src/scheme.rs`) receives `&CanonicalAttrs` only — no
// `&PageContext` access — so a per-portion gate that needs to enumerate
// sibling portions ("not a solely-NATO document") cannot be a
// `Constraint::Custom` today. Hand-written rule with page-context access
// is the right shape until the trait surface extends.
// ---------------------------------------------------------------------------

/// Confidence scalar emitted by S007 (`bare-nato-requires-rel-to-usa-nato`)
/// alongside its `text_correction` fix.
///
/// **Calibration.** §H.7 p127 Notional Example 2 is the load-bearing
/// citation. The worked example is illustrative prose ("a NATO COSMIC
/// TOP SECRET (CTS) BOHEMIA portion within a US classified document and
/// is releasable back to NATO"), not "MUST"-mandate prose; S004 + S005
/// set the precedent (post-PR-#488 collapse of the historical S005/S006
/// pair) that example-derived FD&R guidance ships as
/// `Severity::Suggest`. Within the suggest channel, the confidence
/// scalar reflects how strongly the source dictates the rewrite. The
/// chosen value `0.85` is below the (broader) `Confidence::strict(0.95)`
/// used by mandate-prose rewrites in `rules_declarative.rs` (e.g. the
/// REL TO canonical-form rewrite at line 711, the JOINT class-floor
/// rewrite at line 1077) and above the `0.75` used by lower-evidence
/// suggest-channel rewrites in this file.
///
/// **Threshold ladder relationship (load-bearing for auto-apply).**
/// The default `confidence_threshold` in `Engine::fix_inner` is `0.95`.
/// A user who sets `[rules] S007 = "fix"` in `.marque.toml` will see the
/// engine demote the override back to `Suggest` because the diagnostic's
/// confidence is `0.85 < 0.95`. To get S007 auto-applied a user must
/// also drop the threshold to `≤ 0.80` (or any value `< 0.85`) via
/// `confidence_threshold = 0.80`. The dual-override pattern is
/// exercised end-to-end by `engine_with_s007_as_fix()` in
/// `crates/capco/tests/fr048_bare_nato_rel_to.rs`.
///
/// **S007 vs. S004.** S004 emits at the file-level constant
/// [`SUGGEST_CONFIDENCE`] (currently `0.5`), which cannot clear any
/// reasonable threshold even with a `fix` override. S007 is the first
/// text_correction-bearing Suggest-severity rule whose confidence is
/// high enough to clear a relaxed threshold; the next author who adds a
/// suggest-with-apply rule should pick a confidence consciously rather
/// than copy-paste from S004's hard-advisory channel.
const S007_SUGGEST_CONFIDENCE: f32 = 0.85;

/// Rule **S007** — `bare-nato-requires-rel-to-usa-nato`.
///
/// Fires on a portion whose classification axis is a bare
/// [`MarkingClassification::Nato`] variant when the page also carries at
/// least one non-NATO portion AND the portion's existing REL TO list
/// does not already cover `{USA, NATO}`. Emits a `Severity::Suggest`
/// `text_correction` that either (a) replaces the classification token
/// with `<class>//REL TO USA, NATO` (e.g. `NS` →
/// `NS//REL TO USA, NATO` in `(//NS)`), or (b) augments an existing
/// REL TO block to include `USA` and `NATO` in canonical USA-first /
/// alpha-sorted order.
///
/// # Authority
///
/// CAPCO-2016 §H.7 p127 Notional Example 2 — the worked example
/// `(//CTS//BOHEMIA//REL TO USA, NATO)` shows the canonical form for a
/// bare-NATO portion in a US-classified document. By extension a NATO
/// classification axis (NU/NR/NC/NS/CTS) in a US-classified document
/// should carry `REL TO USA, NATO` even without an SCI block.
///
/// # Early-return clauses (in order)
///
/// 1. **Portion-only**: `ctx.marking_type != MarkingType::Portion`.
///    Banner roll-up flows automatically from the per-portion
///    `REL TO USA, NATO` via `BannerMatchesProjectedRule`; firing on a
///    banner here would double-report.
/// 2. **Bare NATO classification only**: `attrs.classification` must be
///    `Some(MarkingClassification::Nato(_))`. US/JOINT/FGI/Conflict
///    portions are not in scope. (ATOMAL companions on the AEA axis
///    coexist with `Nato(_)` and do **not** immunize the portion — see
///    project memory `project_atomal_is_aea`.)
/// 3. **Solely-NATO doc carve-out**: when `ctx.page_marking` is `Some`
///    and `ProjectedMarking::is_solely_nato_classified()` returns `true`,
///    every portion on the page is bare NATO and alliance ownership is
///    implicit — `REL TO USA, NATO` is not needed. **Today this branch
///    is forward-looking**: `Engine::lint` gates
///    `with_page_marking(ctx_page_marking)` on
///    `candidate.kind != MarkingType::Portion && !page_portions.is_empty()`,
///    so portion rules always see `page_marking = None` and the
///    carve-out is unreachable. S007 fires on every bare-NATO portion
///    regardless of solely-NATO document status until a future engine
///    pass plumbs page-level state to portion-rule dispatch
///    (load-bearing for that migration; see fr048 trip-wire test).
///    Users in solely-NATO contexts can silence with
///    `[rules] S007 = "off"` in `.marque.toml`. PM decision #2.
/// 4. **NOFORN guard**: when this portion carries `DissemControl::Nf`,
///    the conflict is owned by the page rewrite
///    `capco/noforn-conflicts-rel-to` (declarative constraint in
///    `CapcoScheme`). S007 must not propose a `REL TO` that the
///    NOFORN/REL-TO conflict rule would immediately remove.
///
/// # Coverage check (intentional byte-compare for NATO)
///
/// The predicate is `has_usa && has_nato` where:
///
/// - `has_usa` is `attrs.rel_to.contains(&CountryCode::USA)` (USA has a
///   `pub const`).
/// - `has_nato` is `attrs.rel_to.iter().any(|c| c.as_bytes() == b"NATO")`.
///
/// We deliberately do **NOT** use `rel_to_covers` here — tetragraph
/// expansion would accept `REL TO USA, DEU, GBR, FRA, ...` as covering
/// NATO, which §H.7 p127's literal example does not endorse. We also
/// deliberately do **NOT** add a `CountryCode::NATO` constant: that
/// would bump the `marque-ism` public surface for a single use site
/// without a second consumer — Constitution VII Principle IV "shallow
/// adapter" discipline. Byte-compare at the one site is the right idiom
/// until a second consumer materializes.
///
/// # Splice strategy (two helpers, two responsibilities)
///
/// The fix path uses two single-responsibility helpers:
///
/// - [`build_bare_nato_rel_to_insertion`] — used when the portion has
///   NO existing REL TO block. Replaces the **Classification token's
///   span** with `<class>//REL TO USA, NATO`, where `<class>` is the
///   canonical portion abbreviation from `NatoClassification::portion_str`
///   (one of NU/NR/NC/NS/CTS). A non-empty span is required because
///   the engine's `text_correction` synthesizer rejects empty spans
///   (`engine.rs::apply_text_corrections_to` line 1529); a zero-length
///   insert-cursor would be filtered out before promotion. Re-emitting
///   the corpus-derived classification abbreviation is G13-safe — it
///   is a closed-vocab canonical token, not raw document content.
/// - [`build_bare_nato_rel_to_augmentation`] — used when the portion
///   has an existing REL TO block. Emits the canonical
///   `REL TO USA, <sorted-existing-plus-NATO>` body via
///   [`build_rel_to_replacement`] so the canonical-form contract stays
///   in one place.
///
/// Two helpers, not one: the splice site (replace-classification-token
/// vs replace-existing-rel-to-block) is different, and the emitted
/// body shape (classification-prefixed `//REL TO ...` vs bare
/// `REL TO ...` list) is different. Folding them into one helper would
/// require a Boolean parameter that re-introduces the bug class
/// single-responsibility was supposed to remove.
///
/// # `Phase::WholeMarking`, not `Phase::Localized`
///
/// The augmentation branch modifies a `RelToBlock` token that is not
/// the same token as the classification block — the fix crosses a
/// token boundary. `Phase::Localized`'s "single-token-span contract"
/// (`marque_rules::lib.rs` line 241) would fail validation for the
/// augmentation branch. `Phase::WholeMarking` is correct for both
/// branches.
///
/// # G13 audit-content-ignorance
///
/// The diagnostic message is a `&'static`-derived string. No
/// `format!` interpolation of input bytes. The replacement string is
/// constructed from `CountryCode` values only (which are
/// corpus-derived canonical token bytes) plus the literal `NATO`
/// and `REL TO USA, ` template — no document text contributes to
/// audit output. Constitution V Principle V (G13).
///
/// # First text_correction-bearing Suggest-severity rule with apply path
///
/// S007 is the first text_correction-bearing `Severity::Suggest` rule
/// in marque-capco whose emitted confidence is high enough to clear a
/// relaxed `confidence_threshold` when paired with a `[rules] S007 =
/// "fix"` override. The threshold ladder:
///
/// - S004 emits at [`SUGGEST_CONFIDENCE`] = `0.5`. Even with `[rules]
///   S004 = "fix"` the diagnostic cannot clear the default
///   `confidence_threshold = 0.95`, so S004 stays hard-advisory under
///   any reasonable threshold.
/// - S007 emits at [`S007_SUGGEST_CONFIDENCE`] = `0.85`. With both
///   `[rules] S007 = "fix"` AND `confidence_threshold ≤ 0.80` set, the
///   engine's `lint` suggest-channel demotion pass keeps the override
///   intact and `Engine::fix_inner` applies the splice.
///
/// The dual-override pattern is exercised end-to-end by the
/// `engine_with_s007_as_fix()` helper and the augmentation tests in
/// `crates/capco/tests/fr048_bare_nato_rel_to.rs`. The next author who
/// adds a suggest-with-apply rule should pick a confidence scalar with
/// the same care: too low and the auto-apply path is unreachable; too
/// high and a Suggest-default rule auto-applies under the default
/// threshold the moment a user adds the severity override.
struct BareNatoRequiresRelToRule;

/// Build the insertion body for the no-existing-REL-TO branch.
///
/// The splice strategy replaces the **classification token's span**
/// (a non-empty span; the engine's `text_correction` synthesizer
/// rejects empty spans per `engine.rs::apply_text_corrections_to`
/// line 1529) with `<class>//REL TO USA, NATO`, where `<class>` is the
/// canonical portion abbreviation from
/// [`marque_ism::NatoClassification::portion_str`] (one of NU/NR/NC/
/// NS/CTS). For input `(//NS)` the splice replaces `NS` with
/// `NS//REL TO USA, NATO`, yielding `(//NS//REL TO USA, NATO)`.
///
/// Pure function over `&NatoClassification`: input is a corpus-derived
/// canonical token; no document content flows through. Constitution V
/// Principle V (G13).
///
/// **Branch invariant.** This helper is the no-REL-TO branch only. The
/// caller in `BareNatoRequiresRelToRule::check` gates the call site
/// behind `rel_to_blocks.is_empty()` (the branch that selects this
/// helper over [`build_bare_nato_rel_to_augmentation`]); the
/// `debug_assert!` at the call site re-checks `attrs.rel_to.is_empty()`
/// as a redundant guard. If the existing-REL-TO case ever needs the
/// classification-token splice as well, factor a shared helper rather
/// than re-introducing an `existing` parameter here.
fn build_bare_nato_rel_to_insertion(class: marque_ism::NatoClassification) -> String {
    // 18 bytes for `//REL TO USA, NATO`: 2 for `//` + 16 for the
    // suffix literal. `class.portion_str()` is `NU`/`NR`/`NC`/`NS`/
    // `CTS` (2 or 3 bytes); capacity covers the longest variant.
    let mut out = String::with_capacity(class.portion_str().len() + 18);
    out.push_str(class.portion_str());
    out.push_str("//REL TO USA, NATO");
    out
}

/// Build the canonical replacement body for the existing-REL-TO branch.
/// Takes the existing `rel_to` slice, drops `USA` and `NATO` (so they
/// aren't double-listed), appends both back via
/// [`build_rel_to_replacement`] which enforces USA-first +
/// alpha-sorted canonical form per §A.6 p16 + §H.8 p150-151.
///
/// Pure function over `&[CountryCode]`: byte input is the rel_to slice
/// (canonical token bytes only), byte output is the canonical
/// replacement body. Constitution V G13: no document text flows
/// through this function.
fn build_bare_nato_rel_to_augmentation(existing: &[CountryCode]) -> String {
    // Collect the existing codes minus USA and NATO, then add both back
    // via the canonical builder — the builder prepends USA and sorts
    // the remainder alphabetically. `NATO` (a tetragraph) sorts among
    // the remainder by `Vec::sort` byte ordering.
    let mut codes: Vec<String> = existing
        .iter()
        .map(|c| c.as_str().to_owned())
        .filter(|s| s != "USA" && s != "NATO")
        .collect();
    codes.push("USA".to_owned());
    codes.push("NATO".to_owned());
    build_rel_to_replacement(&codes)
}

/// Citations S007 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const S007_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 7, 127)];

impl Rule<CapcoScheme> for BareNatoRequiresRelToRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.nato.bare-nato-requires-rel-to-usa-nato")
    }
    fn name(&self) -> &'static str {
        "bare-nato-requires-rel-to-usa-nato"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
    }
    /// `Phase::WholeMarking`: the augmentation branch's splice target
    /// is a `RelToBlock` token distinct from the classification token
    /// — the fix crosses a token boundary. `Phase::Localized`'s
    /// single-token-span contract fails the augmentation branch. The
    /// insertion branch is technically Localized-compatible (single
    /// zero-length insert site at the classification-block edge) but
    /// declaring two different phases for the two branches would
    /// double the rule's registration surface; one phase covers both.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        S007_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::MarkingType;

        // Clause 1: portion-only.
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        // Clause 2: bare NATO classification only. Capture the NATO
        // variant for the insertion-branch canonical text (used by
        // `build_bare_nato_rel_to_insertion`).
        let Some(MarkingClassification::Nato(nato_class)) = &attrs.classification else {
            return vec![];
        };
        let nato_class = *nato_class;

        // Clause 3: solely-NATO doc carve-out. When page-marking is
        // populated AND every portion is bare-NATO, alliance ownership
        // is implicit — silence. **Today this branch is forward-looking**:
        // `Engine::lint` does not populate `RuleContext::page_marking`
        // for `MarkingType::Portion` candidates, so `ctx.page_marking`
        // is always `None` here and the carve-out is unreachable. S007
        // fires on every bare-NATO portion regardless of solely-NATO
        // document status until a future engine pass plumbs page state
        // to portion-rule dispatch (see fr048 trip-wire test).
        //
        // PR 4b-D.3 (2026-05-18): migrated from `ctx.page_context` to
        // `ctx.page_marking`. Both predicates return identical answers
        // (the legacy `PageContext::is_solely_nato_classified` walks
        // `self.portions` with the same `matches!` pattern); the
        // migration is architectural alignment ahead of PR 4b-E
        // retiring the `PageContext.expected_*` machinery and
        // consolidating page-aggregate reads on `ctx.page_marking`.
        if let Some(page) = ctx.page_marking.as_ref()
            && page.is_solely_nato_classified()
        {
            return vec![];
        }

        // Clause 4: NOFORN guard. NOFORN on this portion is structurally
        // incompatible with REL TO; the conflict is owned by the page
        // rewrite `capco/noforn-conflicts-rel-to`. S007 must not propose
        // a REL TO that the NOFORN/REL-TO conflict rule would
        // immediately remove.
        if attrs
            .dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf))
        {
            return vec![];
        }

        // Coverage check: byte-compare against the literal NATO
        // tetragraph plus the `CountryCode::USA` const. Do NOT use
        // `rel_to_covers` here — tetragraph expansion is not what
        // §H.7 p127's example endorses (see the doc-block above).
        let has_usa = attrs.rel_to.contains(&CountryCode::USA);
        let has_nato = attrs.rel_to.iter().any(|c| c.as_bytes() == b"NATO");
        if has_usa && has_nato {
            return vec![];
        }

        // Locate the splice point. Two branches:
        //
        //  (a) no REL TO block: replace the Classification token's
        //      span with `<class>//REL TO USA, NATO` where `<class>`
        //      is the canonical portion abbreviation from
        //      `NatoClassification::portion_str()`. For `(//NS)` the
        //      Classification token spans the `NS` bytes; the splice
        //      replaces them with `NS//REL TO USA, NATO`, yielding
        //      `(//NS//REL TO USA, NATO)`. A non-empty span is
        //      required because the engine's `text_correction`
        //      synthesizer rejects empty spans
        //      (`engine.rs::apply_text_corrections_to` line 1529).
        //  (b) existing REL TO block: replace that token's span with
        //      the augmented canonical body. The first RelToBlock
        //      token wins; if multiple RelToBlock tokens exist (a
        //      malformed shape), we skip the rule rather than risk
        //      cross-block corruption — the cleaner authoring
        //      decision is deferred to the operator.
        // The discriminator only needs to distinguish 0 / 1 / many
        // RelToBlock tokens, so we pull two iterator items and match
        // on the shape rather than allocating a `Vec`.
        let mut rel_to_blocks_iter = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToBlock);
        let (span, replacement) = match (rel_to_blocks_iter.next(), rel_to_blocks_iter.next()) {
            (None, _) => {
                // No existing REL TO block — insertion branch. We
                // replace the **classification token's span** (a
                // non-empty span; the engine's `text_correction`
                // synthesizer rejects empty spans) with
                // `<class>//REL TO USA, NATO`, where `<class>` is the
                // canonical portion abbreviation. The result for
                // `(//NS)` is `(//NS//REL TO USA, NATO)`.
                //
                // INVARIANT: `attrs.rel_to` is empty in this branch.
                // The parser populates `attrs.rel_to` from
                // `RelToBlock` token spans
                // (`marque-core::parser::parse_rel_to_block`), so no
                // `RelToBlock` tokens implies an empty
                // `attrs.rel_to`. We re-check defensively so a future
                // parser change that populates `attrs.rel_to` from a
                // non-`RelToBlock` source can't silently corrupt the
                // splice.
                debug_assert!(
                    attrs.rel_to.is_empty(),
                    "S007 insertion branch requires empty attrs.rel_to (no \
                     RelToBlock tokens implies no rel_to entries); the \
                     augmentation branch (build_bare_nato_rel_to_augmentation) \
                     owns the non-empty case",
                );
                let Some(class_tok) = attrs
                    .token_spans
                    .iter()
                    .find(|t| t.kind == TokenKind::Classification)
                else {
                    // Defensive: a NATO classification axis without a
                    // Classification token span would mean the parser
                    // failed to emit the token. Skip emission rather
                    // than risk a wrong-span splice.
                    return vec![];
                };
                let body = build_bare_nato_rel_to_insertion(nato_class);
                (class_tok.span, body)
            }
            (Some(block), None) => {
                // Single existing REL TO block — augmentation branch.
                let body = build_bare_nato_rel_to_augmentation(&attrs.rel_to);
                (block.span, body)
            }
            (Some(_), Some(_)) => {
                // Multiple REL TO blocks is structurally malformed; an
                // E002 / parser-shape diagnostic owns that case. Skip
                // S007 emission to avoid splice-collision damage.
                return vec![];
            }
        };

        vec![Diagnostic::text_correction(
            self.id(),
            self.default_severity(),
            span,
            Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
            capco(SectionLetter::H, 7, 127),
            replacement,
            FixSource::BuiltinRule,
            Confidence::strict(S007_SUGGEST_CONFIDENCE),
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: S008 — RELIDO implied by closure (byte-surfacing twin of
//              CLOSURE_RELIDO_SCI / CLOSURE_RELIDO_US_CLASS).
//
// C1 per #559 close-out (PM decision 2026-05-19). The lattice-layer
// closures `CLOSURE_RELIDO_SCI` (§H.8 p154; CAT_SCI implies RELIDO
// absent FD&R suppressors) and `CLOSURE_RELIDO_US_CLASS` (issue #524
// Phase 3; US collateral classified implies RELIDO absent FD&R
// suppressors) already propagate the RELIDO fact at the lattice
// layer. This rule is the byte-level twin: when the closure would
// inject RELIDO into a portion that doesn't currently carry it, S008
// fires a `Severity::Suggest` diagnostic with a `FactAdd(RELIDO,
// Scope::Portion)` intent so the user-visible text can match the
// lattice-level state.
//
// Per S007's precedent (the byte-surfacing twin of
// `CLOSURE_REL_TO_USA_NATO` at §H.7 p127), the text-layer rule
// proposes the byte-level insertion; the lattice-layer closure stays
// out of the diagnostic surface.
//
// Authority: CAPCO-2016 §H.8 p154 (RELIDO template) + §D.2 Table 3
// rule 17 (FD&R defaults for caveated content; verified against
// `crates/capco/docs/CAPCO-2016.md`).
// ---------------------------------------------------------------------------

/// Confidence scalar emitted by S008 (`relido-implied-by-closure`)
/// alongside its `FactAdd` fix intent.
///
/// **Calibration.** Mirrors `S007_SUGGEST_CONFIDENCE = 0.85` —
/// example/closure-derived guidance that ships at `Severity::Suggest`
/// with confidence high enough to clear a relaxed
/// `confidence_threshold` when paired with `[rules] S008 = "fix"`.
/// The §H.8 p154 RELIDO template + §D.2 Table 3 rule 17 backing the
/// CLOSURE_RELIDO_SCI / CLOSURE_RELIDO_US_CLASS rows is template-
/// prose plus FD&R-defaults derivation, not "MUST"-mandate prose; the
/// suggest channel is the right home.
const S008_SUGGEST_CONFIDENCE: f32 = 0.85;

/// Shared `CapcoScheme` used by S008's `check()` to apply the closure
/// fixpoint to a portion's marking. Constructed lazily — `CapcoScheme::new()`
/// runs the constraint/page-rewrite/closure-rule catalog build once,
/// then every `check()` call borrows the cached instance instead of
/// reconstructing it per-portion. Mirrors the `SCHEME` pattern in
/// `rules_declarative.rs` (the wrapper-layer file slated for retirement
/// in the post-#578 refactor); having an independent instance here
/// survives that retirement without coupling S008 to a file being
/// deleted.
static S008_SCHEME: std::sync::LazyLock<CapcoScheme> = std::sync::LazyLock::new(CapcoScheme::new);

/// Rule **S008** — `relido-implied-by-closure`.
///
/// Fires on a portion whose closure-applied projection carries RELIDO
/// in `dissem_us` AND whose source text does NOT already carry RELIDO.
/// Emits a `Severity::Suggest` diagnostic with a
/// `FactAdd(TOK_RELIDO, Scope::Portion)` intent at confidence
/// [`S008_SUGGEST_CONFIDENCE`].
///
/// # Closure-based trigger detection
///
/// The rule runs `S008_SCHEME.closure(marking)` and compares the
/// post-closure dissem axis against the pre-closure state. This is
/// more robust than hand-rolling the closure trigger / suppressor
/// logic because:
///
/// - `CLOSURE_RELIDO_SCI` triggers on `CAT_SCI` presence and
///   suppresses on `FDR_OR_RELIDO_INCOMPAT` (NOFORN / RELIDO / REL TO
///   / DISPLAY ONLY / EYES plus six per-compartment SCI sentinels
///   plus FGI / JOINT / NATO classification — at least 14 distinct
///   tokens with subtle interactions).
/// - `CLOSURE_RELIDO_US_CLASS` triggers on US collateral classification
///   and suppresses on `RELIDO_US_CLASS_SUPPRESSORS` (the same FD&R
///   dominators plus six per-compartment SCI sentinels).
/// - Both closures interact with `with_noforn_injected` (the §H.8
///   p145 supersession overlay) which strips RELIDO whenever NOFORN
///   appears at any iteration.
///
/// Replicating that decision tree inline would double the source-of-
/// truth surface for the same policy decision. Calling
/// `scheme.closure(...)` once and reading the result keeps S008
/// aligned with the closure catalog by construction. The short-
/// circuit in `CapcoScheme::closure` (line 561,
/// `any_closure_trigger_fires`) returns the input identically when
/// no trigger fires, so the cost on non-triggering portions is bounded
/// to a single trigger check.
///
/// # Early-return clauses (in order)
///
/// 1. **Portion-only**: `ctx.marking_type != MarkingType::Portion`.
///    Banner roll-up flows automatically once each portion carries
///    RELIDO; firing on a banner would double-report.
/// 2. **RELIDO already present**: `attrs.dissem_us` contains
///    `DissemControl::Relido`. Nothing to suggest.
/// 3. **Closure does not inject RELIDO**: the closure short-circuited
///    on `any_closure_trigger_fires`, OR a suppressor blocked the
///    cone, OR the with_noforn_injected overlay stripped it. No
///    diagnostic — the lattice-layer decided RELIDO is not implied.
///
/// # Fix shape
///
/// `ReplacementIntent::FactAdd { token: FactRef::Cve(TOK_RELIDO),
/// scope: Scope::Portion }` — the engine's renderer composes the
/// post-add marking back into bytes. This is the same intent shape
/// E021 uses for `FactAdd(NOFORN)` (see
/// `rules_declarative.rs::aea_noforn_add_intent`); the engine handles
/// canonical placement (per-axis sort) and separator insertion. No
/// manual splice logic in this rule, in contrast to S007 which crosses
/// a token boundary (RelToBlock vs Classification).
///
/// # `Phase::WholeMarking`
///
/// The `FactAdd` intent is whole-marking-scope: the engine re-renders
/// the portion from canonical attrs after applying the fact. The
/// candidate_span tells the engine which region to replace.
///
/// # G13 audit-content-ignorance
///
/// The diagnostic message is a `&'static`-derived string. No
/// `format!` interpolation of input bytes. The fact added
/// (`TOK_RELIDO`) is a `TokenId`, not a byte sequence — Constitution
/// V Principle V (G13).
struct RelidoImpliedByClosureRule;

/// Citations S008 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const S008_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 8, 154)];

impl Rule<CapcoScheme> for RelidoImpliedByClosureRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.dissem.relido-implied-by-closure")
    }
    fn name(&self) -> &'static str {
        "relido-implied-by-closure"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
    }
    /// `Phase::WholeMarking`: `FactAdd` intent is whole-marking-scope.
    /// The engine re-renders the full portion from canonical attrs.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        S008_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use crate::scheme::{CapcoMarking, TOK_RELIDO};
        use marque_ism::DissemControl;

        // Clause 1: portion-only.
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        // Clause 2: RELIDO already present — nothing to suggest. Cheap
        // check that short-circuits before the closure call.
        if attrs
            .dissem_iter()
            .any(|d| matches!(d, DissemControl::Relido))
        {
            return vec![];
        }

        // Clause 3: run the closure and check the post-closure state.
        // `S008_SCHEME.closure(marking)` short-circuits via
        // `any_closure_trigger_fires` when no closure rule's trigger
        // fires (the bench-corpus typical case), returning the input
        // marking identically. When a trigger fires, the fixpoint
        // loop converges in 1–2 iterations on real-world inputs
        // (proptest harness pins MAX_CLOSURE_ITERATIONS as the
        // worst-case cap).
        let marking = CapcoMarking::new(attrs.clone());
        let closed = S008_SCHEME.closure(marking);
        let closure_adds_relido = closed
            .0
            .dissem_us
            .iter()
            .any(|d| matches!(d, DissemControl::Relido));
        if !closure_adds_relido {
            return vec![];
        }

        // Build the FactAdd intent. Scope::Portion matches E021's
        // FactAdd(NOFORN) precedent and the closure-rule cone (cone
        // operates on per-portion dissem axis).
        let fix_intent = FixIntent {
            replacement: ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_RELIDO),
                scope: Scope::Portion,
            },
            confidence: Confidence::strict(S008_SUGGEST_CONFIDENCE),
            feature_ids: Default::default(),
            message: Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };

        // Diagnostic span: anchor at the candidate (whole-portion)
        // since the suggestion is "add RELIDO to this portion." No
        // sub-token span is more informative than the marking-scope
        // span for an add-fact suggestion.
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            ctx.candidate_span,
            ctx.candidate_span,
            Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
            // Typed Citation anchors at §H.8 p154 (RELIDO grammar);
            // the §D.2 Table 3 row-17 cross-reference lives in the
            // rule doc comment.
            capco(SectionLetter::H, 8, 154),
            fix_intent,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: S009 — prefer-tetragraph-collapse.
//
// Issue #250. Authority: CAPCO-2016 §H.8 p150 (canonical REL TO form;
// worked examples consistently use the compact tetragraph form when all
// members are present). ISMCAT Tetragraph Taxonomy
// V[`marque_ism::ISMCAT_TETRA_VERSION`] (membership tables).
//
// Default severity is `Off` — tetragraph vs. explicit-member form is a
// classification-authority / org style choice; neither form violates
// CAPCO-2016 §H.8. Users opt in via `[rules] S009 = "suggest"`.
// ---------------------------------------------------------------------------

/// Confidence scalar for S009 (`prefer-tetragraph-collapse`).
///
/// Mirrors `S007_SUGGEST_CONFIDENCE = 0.85` — sufficient for the
/// suggestion channel. The collapse is purely additive (no
/// information loss: tetragraphs are decomposable), so 0.85 is
/// conservative; users who set `[rules] S009 = "fix"` will need
/// `confidence_threshold ≤ 0.84` to auto-apply.
const S009_SUGGEST_CONFIDENCE: f32 = 0.85;

/// Rule **S009** — `prefer-tetragraph-collapse`.
///
/// When a REL TO list enumerates all individual members of a known
/// decomposable tetragraph (e.g., `AUS, CAN, GBR, NZL` for `FVEY`),
/// suggests replacing the explicit list with the compact tetragraph form.
///
/// Example: `REL TO USA, AUS, CAN, GBR, NZL` → `REL TO USA, FVEY`.
///
/// **Default severity: `Off`** — tetragraph vs. explicit-member form is
/// an org/classification-authority style choice, not a CAPCO mandate.
/// Enable via `[rules] S009 = "suggest"` (or `"warn"`) in `.marque.toml`.
///
/// **Algorithm**: Greedy set cover over decomposable tetragraphs —
/// tetragraphs with a non-empty member slice in the ISMCAT taxonomy
/// (opaque tetragraphs with no published membership, such as `EU`, are
/// skipped). Candidates are sorted by member-count descending, then alpha,
/// so larger groups (FVEY 5 members) are preferred over overlapping
/// sub-groups (ACGU 4 members). USA is **never** absorbed — §H.8 p150
/// worked examples always emit `USA` explicitly even when `FVEY` is the
/// tetragraph.
///
/// A no-op gate suppresses emission when every selected tetragraph is
/// already present in the REL TO list (input is already compact).
///
/// Authority: CAPCO-2016 §H.8 p150 (canonical REL TO form; USA-first,
/// trigraphs alpha, tetragraphs alpha — worked examples use compact
/// tetragraph form throughout).
struct PreferTetragraphCollapseRule;

impl Rule<CapcoScheme> for PreferTetragraphCollapseRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.prefer-tetragraph-collapse")
    }
    fn name(&self) -> &'static str {
        "prefer-tetragraph-collapse"
    }
    /// `Severity::Off` — disabled by default.
    fn default_severity(&self) -> Severity {
        Severity::Off
    }
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Gate 1: nothing to collapse with an empty REL TO.
        if attrs.rel_to.is_empty() {
            return vec![];
        }

        // O(1)-lookup set for membership tests.
        let rel_to_codes: HashSet<&str> = attrs.rel_to.iter().map(|c| c.as_str()).collect();

        // Find decomposable tetragraphs (non-empty member slice per ISMCAT)
        // whose full member set is covered by the current REL TO list.
        // Opaque tetragraphs (empty member slice — e.g. EU) are skipped;
        // S005 handles opaque-tetragraph uncertainty separately.
        let mut candidates: Vec<(&str, &'static [&'static str])> = TETRAGRAPH_MEMBERS
            .iter()
            .filter_map(|(code, members)| {
                if members.is_empty() {
                    return None;
                }
                if members.iter().all(|m| rel_to_codes.contains(*m)) {
                    Some((*code, *members))
                } else {
                    None
                }
            })
            .collect();

        if candidates.is_empty() {
            return vec![];
        }

        // Greedy set cover: largest-member-set first, alpha tie-break.
        // Larger groups preferred (FVEY 5-member over ACGU 4-member).
        candidates.sort_unstable_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));

        // `collapsed` tracks non-USA trigraphs absorbed into a selected
        // tetragraph. USA is intentionally excluded — §H.8 p150 worked
        // examples always emit USA explicitly even when FVEY is selected.
        let mut collapsed: HashSet<&str> = HashSet::new();
        let mut selected: Vec<&str> = Vec::new();
        for (code, members) in &candidates {
            let overlaps = members
                .iter()
                .any(|m| *m != "USA" && collapsed.contains(*m));
            if overlaps {
                continue;
            }
            selected.push(code);
            for m in *members {
                if *m != "USA" {
                    collapsed.insert(m);
                }
            }
        }

        if selected.is_empty() {
            return vec![];
        }

        // No-op gate: every selected tetragraph already in the REL TO list
        // means the input is already in compact form — nothing to suggest.
        if selected.iter().all(|code| rel_to_codes.contains(*code)) {
            return vec![];
        }

        // Build the replacement using the canonical §H.8 p150 / §A.6 p16
        // sort: USA first, remaining trigraphs ascending alpha, tetragraphs
        // ascending alpha.
        let has_usa = rel_to_codes.contains("USA");
        let mut remaining_trigraphs: Vec<&str> = rel_to_codes
            .iter()
            .copied()
            .filter(|&c| c != "USA" && c.len() == 3 && !collapsed.contains(c))
            .collect();
        remaining_trigraphs.sort_unstable();
        let mut tetragraph_bucket: Vec<&str> = rel_to_codes
            .iter()
            .copied()
            .filter(|&c| c.len() != 3 && !collapsed.contains(c))
            .collect();
        tetragraph_bucket.extend_from_slice(&selected);
        tetragraph_bucket.sort_unstable();
        tetragraph_bucket.dedup();

        let mut replacement =
            String::with_capacity(7 + 5 * (remaining_trigraphs.len() + tetragraph_bucket.len()));
        replacement.push_str("REL TO");
        let mut first_code = true;
        let emit_code = |code: &str, out: &mut String, first: &mut bool| {
            if *first {
                out.push(' ');
                *first = false;
            } else {
                out.push_str(", ");
            }
            out.push_str(code);
        };
        if has_usa {
            emit_code("USA", &mut replacement, &mut first_code);
        }
        for code in &remaining_trigraphs {
            emit_code(code, &mut replacement, &mut first_code);
        }
        for code in &tetragraph_bucket {
            emit_code(code, &mut replacement, &mut first_code);
        }

        // Single RelToBlock span — same splice pattern as S007.
        let mut blocks = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToBlock);
        let block = match (blocks.next(), blocks.next()) {
            (Some(b), None) => b,
            // Multiple RelToBlock tokens is a malformed shape; E002 /
            // parser-shape diagnostics own that case. Skip to avoid
            // cross-block splice damage.
            (Some(_), Some(_)) | (None, _) => return vec![],
        };

        if block.text.as_str() == replacement {
            return vec![];
        }

        vec![Diagnostic::text_correction(
            self.id(),
            Severity::Suggest,
            block.span,
            Message::new(
                MessageTemplate::NonCanonicalForm,
                MessageArgs {
                    category: Some(crate::scheme::CAT_REL_TO),
                    ..Default::default()
                },
            ),
            capco(SectionLetter::H, 8, 150),
            replacement,
            FixSource::BuiltinRule,
            Confidence::strict(S009_SUGGEST_CONFIDENCE),
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: S010 — collapse-uniform-rel-portions
// ---------------------------------------------------------------------------
//
// Phase: PageFinalization. Off by default.
//
// CAPCO-2016 §H.8 p150: "Authorized Portion Mark (when the portion's
// country trigraphs and/or tetragraph list is the SAME as the banner line
// REL TO marking): REL". When ALL portions with an explicit REL TO list
// carry the same list as the projected banner REL TO, the compact `REL`
// form is equally valid. S010 suggests this compaction. Gate: only fires
// when EVERY explicit-REL-TO portion matches, so the suggested
// transformation replaces all of them uniformly.

/// Confidence scalar for S010.
const S010_SUGGEST_CONFIDENCE: f32 = 0.85;
const S010_CITATION: Citation = capco(SectionLetter::H, 8, 150);

/// Rule **S010** — `collapse-uniform-rel-portions`.
///
/// Off by default. Enable via `[rules] S010 = "suggest"`.
/// Authority: CAPCO-2016 §H.8 p150.
struct CollapseUniformRelPortionsRule;

impl Rule<CapcoScheme> for CollapseUniformRelPortionsRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.collapse-uniform-rel-portions")
    }
    fn name(&self) -> &'static str {
        "collapse-uniform-rel-portions"
    }
    fn default_severity(&self) -> Severity {
        Severity::Off
    }
    fn phase(&self) -> Phase {
        Phase::PageFinalization
    }
    fn trusted(&self) -> bool {
        true
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        check_collapse_uniform_rel_portions(attrs, ctx)
    }
}

/// Expand a REL TO list into an atomic set of trigraphs.
///
/// Tetragraph codes in `rel_to` (e.g. `FVEY`, `NATO`) are replaced by their
/// member trigraphs. Opaque tetragraphs with no published membership (e.g.
/// `EU`) pass through unchanged. This normalizes both the banner-projected
/// set and per-portion sets to a common representation before comparison, so
/// `(S//REL TO USA, FVEY)` and `(S//REL TO USA, AUS, CAN, GBR, NZL)` are
/// treated as equivalent.
fn expand_rel_to_atomic(
    codes: &[marque_ism::CountryCode],
) -> std::collections::BTreeSet<marque_ism::CountryCode> {
    let mut out = std::collections::BTreeSet::new();
    for code in codes {
        if let Some(members) = crate::vocab::expand_tetragraph(code.as_str()) {
            for m in members {
                if let Some(cc) = marque_ism::CountryCode::try_new(m.as_bytes()) {
                    out.insert(cc);
                }
            }
        } else {
            out.insert(*code);
        }
    }
    out
}

fn check_collapse_uniform_rel_portions(
    _attrs: &CanonicalAttrs,
    ctx: &RuleContext,
) -> Vec<Diagnostic<CapcoScheme>> {
    let Some(page_portions) = ctx.page_portions.as_ref() else {
        return Vec::new();
    };
    let portions: &[CanonicalAttrs] = page_portions.as_ref();
    let Some(page_mark) = ctx.page_marking.as_ref() else {
        return Vec::new();
    };
    // No banner REL TO list projected — nothing to match against.
    if page_mark.rel_to.is_empty() {
        return Vec::new();
    }
    // NOFORN guard: REL TO superseded by NOFORN (mirrors S005).
    let any_noforn = portions.iter().any(|p| {
        p.dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf))
    });
    if any_noforn {
        return Vec::new();
    }
    let banner_set = expand_rel_to_atomic(&page_mark.rel_to);
    let explicit_portions: Vec<&CanonicalAttrs> =
        portions.iter().filter(|p| !p.rel_to.is_empty()).collect();
    if explicit_portions.is_empty() {
        return Vec::new();
    }
    // Gate: EVERY explicit-REL-TO portion must match the banner list.
    let all_match = explicit_portions.iter().all(|p| {
        let portion_set = expand_rel_to_atomic(&p.rel_to);
        portion_set == banner_set
    });
    if !all_match {
        return Vec::new();
    }
    let mut diagnostics = Vec::new();
    for portion in explicit_portions {
        let Some(block) = portion
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::RelToBlock)
        else {
            continue;
        };
        diagnostics.push(Diagnostic::text_correction(
            RuleId::new("capco", "page.dissem.collapse-uniform-rel-portions"),
            Severity::Suggest,
            block.span,
            Message::new(
                MessageTemplate::NonCanonicalForm,
                MessageArgs {
                    category: Some(crate::scheme::CAT_REL_TO),
                    ..Default::default()
                },
            ),
            S010_CITATION,
            "REL",
            FixSource::BuiltinRule,
            Confidence::strict(S010_SUGGEST_CONFIDENCE),
            None,
        ));
    }
    diagnostics
}

// ---------------------------------------------------------------------------
// Rule: E072 — bare-rel-portion-divergence
// ---------------------------------------------------------------------------
//
// Phase: PageFinalization. Warn by default.
//
// CAPCO-2016 §H.8 p150-151: bare `REL` in a portion means "my releasability
// = the banner's REL TO list." When bare-REL portions and explicit-REL-TO
// portions with a different list coexist on the same page, extraction is
// ambiguous: bare-REL portions implicitly carry the banner list while the
// divergent explicit portions carry a different list. E072 warns on each
// divergent explicit portion.

const E072_CITATION: Citation = capco(SectionLetter::H, 8, 151);

/// Rule **E072** — `bare-rel-portion-divergence`.
///
/// Default severity: [`Severity::Warn`]. Authority: CAPCO-2016 §H.8 p150-151.
struct BareRelPortionDivergenceRule;

impl Rule<CapcoScheme> for BareRelPortionDivergenceRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.bare-rel-portion-divergence")
    }
    fn name(&self) -> &'static str {
        "bare-rel-portion-divergence"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    fn phase(&self) -> Phase {
        Phase::PageFinalization
    }
    fn trusted(&self) -> bool {
        true
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        check_bare_rel_portion_divergence(attrs, ctx)
    }
}

fn check_bare_rel_portion_divergence(
    _attrs: &CanonicalAttrs,
    ctx: &RuleContext,
) -> Vec<Diagnostic<CapcoScheme>> {
    let Some(page_portions) = ctx.page_portions.as_ref() else {
        return Vec::new();
    };
    let portions: &[CanonicalAttrs] = page_portions.as_ref();
    let Some(page_mark) = ctx.page_marking.as_ref() else {
        return Vec::new();
    };
    // NOFORN guard: REL TO superseded by NOFORN (mirrors S005).
    let any_noforn = portions.iter().any(|p| {
        p.dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf))
    });
    if any_noforn {
        return Vec::new();
    }
    // No projected banner REL TO — nothing to compare against.
    if page_mark.rel_to.is_empty() {
        return Vec::new();
    }
    // E072 only applies when at least one bare-REL portion exists.
    let has_bare_rel = portions.iter().any(|p| {
        p.rel_to.is_empty()
            && p.dissem_iter()
                .any(|d| matches!(d, marque_ism::DissemControl::Rel))
    });
    if !has_bare_rel {
        return Vec::new();
    }
    let banner_set = expand_rel_to_atomic(&page_mark.rel_to);
    let mut diagnostics = Vec::new();
    for portion in portions {
        if portion.rel_to.is_empty() {
            continue;
        }
        let portion_set = expand_rel_to_atomic(&portion.rel_to);
        if portion_set == banner_set {
            continue;
        }
        // Portion's explicit list diverges from what bare-REL portions imply.
        //
        // Parser invariant (verified against `parse_rel_to_with_spans`, the
        // sole producer of `rel_to` entries): every push into `rel_to` is
        // immediately preceded by a `TokenKind::RelToBlock` `TokenSpan` push
        // at the two call sites in `marque-core::parser`. The
        // `portion.rel_to.is_empty()` guard above means we reach this site
        // only when `rel_to` is non-empty, therefore the `find()` MUST
        // succeed. The `else` arm is defense-in-depth against future parser
        // changes that would violate the invariant; uses the same let-else
        // shape as S010, with a `debug_assert!` on the invariant itself
        // (not on a constant) so dev/test builds panic loud if the parser
        // ever drops the RelToBlock span while keeping `rel_to` populated.
        debug_assert!(
            portion
                .token_spans
                .iter()
                .any(|t| t.kind == TokenKind::RelToBlock),
            "E072: portion with non-empty rel_to has no RelToBlock token span \
             (parser invariant violation; see parse_rel_to_with_spans call sites \
             in marque-core::parser)"
        );
        let Some(block) = portion
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::RelToBlock)
        else {
            continue;
        };
        let span = block.span;
        diagnostics.push(Diagnostic::new(
            RuleId::new("capco", "page.dissem.bare-rel-portion-divergence"),
            Severity::Warn,
            span,
            Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs {
                    category: Some(crate::scheme::CAT_REL_TO),
                    ..Default::default()
                },
            ),
            E072_CITATION,
            None,
        ));
    }
    diagnostics
}

// ---------------------------------------------------------------------------
/// Citation string for E035 — shared between the with-fix and no-fix
/// emission paths so they cannot silently diverge.
///
/// E035 fires for EVERY SCI control system (HCS, RSV, SI, TK), not
/// just HCS. PR 3c.2.C C7 (reviewer R2) corrected the anchor from
/// `§H.4 p62` (HCS-specific subsection) to `§H.4 p61` (the cross-
/// system SCI banner-roll-up grammar in the §H.4 General Information
/// passage):
///
/// > "Use the following syntax rules for both portion marks and
/// > banner lines for all published and unpublished SCI control
/// > systems: [...] Only unique SCI control system, compartment, or
/// > sub-compartment markings will be used."
///
/// — CAPCO-2016 §H.4 p61 (lines 1339–1347), verified at PR 3c.2.C
/// C7 authorship per Constitution VIII propagation rule.
///
/// Per T026a D13 single-citation discipline, this carries the
/// **operative** banner-roll-up rule for SCI only — §H.4 grammar.
/// §D.2 p28 restates the same banner/portion consistency invariant
/// in general-algorithm prose; the §D.2 background pointer lives on
/// the SCI evaluator's doc comment, not here.
const E035_CITATION: Citation = capco(SectionLetter::H, 4, 61);

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// SCI rule helpers
// ---------------------------------------------------------------------------

/// Returns the text form of a SciControlSystem for sort/display purposes.
fn sci_system_text(system: &SciControlSystem) -> &str {
    match system {
        SciControlSystem::Published(bare) => bare.as_str(),
        SciControlSystem::Custom(text) => text.as_ref(),
        // NATO SAPs (BOHEMIA, BALK) per CAPCO-2016 §G.2 p40 + §H.7 p127.
        SciControlSystem::NatoSap(sap) => sap.as_str(),
    }
}

/// Render a list of SciMarkings back to the canonical wire form used in a
/// banner's SCI block — systems joined by `/`, each system's compartments
/// joined by `-`, and sub-compartments space-separated after a compartment.
/// Systems and compartments are emitted in source order; callers are
/// responsible for pre-sorting if they want canonical ascending output.
fn render_sci_block(markings: &[SciMarking]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(markings.len());
    for m in markings {
        let mut piece = sci_system_text(&m.system).to_owned();
        for comp in m.compartments.iter() {
            piece.push('-');
            piece.push_str(comp.identifier.as_ref());
            for sub in comp.sub_compartments.iter() {
                piece.push(' ');
                piece.push_str(sub.as_ref());
            }
        }
        parts.push(piece);
    }
    parts.join("/")
}

// PR 9b (T133): the `page_expected_sci_markings` helper retired with
// the migration of `evaluate_sci_banner_rollup` to read
// `ProjectedMarking::sci_markings` directly. Banner-validation rules
// no longer need a `&PageContext`-to-`Vec<SciMarking>` adapter.

// ===========================================================================
// T035c-21 PR-B: NODIS / EXDIS page-level + portion-level rules (§H.9)
// ===========================================================================
//
// Three hand-written rules that can't ride the declarative-constraint
// path. E039 and E040 read `ctx.page_marking` (the composite
// `ProjectedMarking` projection — banner-validation surface, PR 9b
// T133 / FR-006); E041 is portion-only and reads its dispatch attrs
// directly. None of the three has a single-span text replacement the
// declarative path can synthesize:
//
//   E039  — REL TO not authorized in banner when any portion has NODIS
//           or EXDIS. No fix (removing REL TO from a banner is multi-
//           span and requires human judgment on what to convey instead).
//   E040  — Banner must roll up NODIS (or EXDIS if no NODIS anywhere).
//           Insertion fix when banner already has a Non-IC dissem
//           category block; no-fix Error otherwise.
//   E041  — In a portion with both NODIS and EXDIS, NODIS supersedes
//           EXDIS. Warn-severity with no fix. Portion-only; the
//           banner case is owned by E037 (mutual exclusion, Error).
//           See the in-rule "# No auto-fix" section for why the
//           supersession is not auto-applied.

// ---------------------------------------------------------------------------
// Rule: E039 — REL TO not allowed in banner when portion has NODIS/EXDIS
// ---------------------------------------------------------------------------

/// Fires when the banner's REL TO list is populated and any portion on
/// the page carries NODIS or EXDIS.
///
/// Authority:
/// - **CAPCO-2016 §H.9 p172** (EXDIS): *"REL TO is not
///   authorized in the banner line if any portion contains EXDIS
///   information. In this case, NOFORN would convey in the banner
///   line."*
/// - **CAPCO-2016 §H.9 p174** (NODIS): same for NODIS.
///
/// # Why no fix
///
/// Removing REL TO from a banner is multi-span (the whole RelToBlock
/// comes out, along with its `//` separators), and the replacement
/// depends on whether the user wants to convert to NOFORN-only (the
/// source suggests) or take some other action. Emit an `Error`
/// diagnostic with no fix; the user decides manually.
struct NodisExdisClearsBannerRelToRule;

/// E037 secondary CAPCO §-citations.
///
/// PR 10.A.1 Commit 4: the migration to typed `Citation` collapsed the
/// pre-migration string form `"CAPCO-2016 §H.9 p172 + p174"` into a
/// single `capco(SectionLetter::H, 9, 172)` value on the diagnostic
/// emitted by the declarative `Conflicts` row at
/// `crates/capco/src/scheme/constraints/core_catalog.rs::core_constraints()`
/// (search for `"portion.dissem.nodis-conflicts-exdis"`). The cross-reference
/// to p174 (NODIS authority — the mutual-exclusion rule is stated
/// verbatim on both sides) survived in the catalog row's doc-comment
/// but was un-checked.
///
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1
/// Commit 4 authorship per Constitution VIII propagation rule: §H.9
/// p174 (NODIS Relationship(s) to Other Markings) states "NODIS and
/// EXDIS markings cannot be used together", mirroring §H.9 p172's
/// EXDIS-side wording. Both passages are operative for E037.
///
/// `#[allow(dead_code)]`: see [`E005_CROSS_REFS`] for the rationale.
#[allow(dead_code)]
pub(crate) const E037_CROSS_REFS: &[Citation] = &[capco(SectionLetter::H, 9, 174)];

/// E038 secondary CAPCO §-citations.
///
/// PR 10.A.1 Commit 4: identical mechanism to [`E037_CROSS_REFS`] —
/// the declarative `Custom` row at
/// `core_constraints()::"portion.dissem.nodis-or-exdis-requires-noforn"` carries
/// only the primary §H.9 p172 anchor. The cross-reference to p174
/// (NODIS "Requires NOFORN") survived in the catalog row's
/// doc-comment but was un-checked.
///
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1
/// Commit 4 authorship per Constitution VIII propagation rule: §H.9
/// p174 (NODIS Relationship(s) to Other Markings) carries the same
/// "Requires NOFORN" clause that the rule's primary citation at p172
/// establishes for EXDIS. Both passages are operative for E038.
///
/// `#[allow(dead_code)]`: see [`E005_CROSS_REFS`] for the rationale.
#[allow(dead_code)]
pub(crate) const E038_CROSS_REFS: &[Citation] = &[capco(SectionLetter::H, 9, 174)];

/// E039 secondary CAPCO §-citations.
///
/// PR 10.A.1 Commit 4: the migration to typed `Citation` collapsed the
/// pre-migration string form `"CAPCO-2016 §H.9 p172 + p174 (NODIS)"`
/// into a single `capco(SectionLetter::H, 9, 172)` value on the emitted
/// diagnostic. The cross-reference to p174 (NODIS authority — the
/// EXDIS rule at p172 is mirrored verbatim for NODIS at p174) survived
/// in the rule's doc-comment but was un-checked. This constant pins
/// the dropped cross-reference structurally.
///
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1
/// Commit 4 authorship per Constitution VIII propagation rule: §H.9
/// p174 (NODIS Relationship(s) to Other Markings) carries the same
/// "REL TO not authorized in banner when portion contains NODIS"
/// rule that the rule's primary citation at p172 establishes for
/// EXDIS. Both passages are operative for E039.
///
/// `#[allow(dead_code)]`: see [`E005_CROSS_REFS`] for the rationale.
#[allow(dead_code)]
pub(crate) const E039_CROSS_REFS: &[Citation] = &[capco(SectionLetter::H, 9, 174)];

/// Citations E039 may emit on diagnostics. Combines the primary
/// `Diagnostic.citation` value (§H.9 p172 — EXDIS) with the
/// [`E039_CROSS_REFS`] cross-references (§H.9 p174 — NODIS). See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E039_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::H, 9, 172),
    capco(SectionLetter::H, 9, 174),
];

impl Rule<CapcoScheme> for NodisExdisClearsBannerRelToRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.nodis-exdis-clears-banner-rel-to")
    }
    fn name(&self) -> &'static str {
        "nodis-exdis-clears-banner-rel-to"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::WholeMarking: banner-scope decision combining the
    /// banner's REL TO list with the page-context expected non-IC
    /// dissem set. No fix (removing REL TO from a banner is multi-span
    /// and policy-dependent).
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E039_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{MarkingType, NonIcDissem};

        // Banner-only (and CAB, since CABs can carry REL TO). Portion
        // candidates are the input; they don't trigger on themselves.
        if !matches!(ctx.marking_type, MarkingType::Banner | MarkingType::Cab) {
            return vec![];
        }

        if attrs.rel_to.is_empty() {
            return vec![];
        }

        // PR 9b (T133): banner-validation reads `ctx.page_marking`
        // (the `ProjectedMarking`) instead of going through
        // `PageContext::expected_non_ic_dissem`. The projection's
        // `non_ic_dissem` field carries the same supersession-
        // resolved roll-up.
        let Some(page) = ctx.page_marking.as_ref() else {
            return vec![];
        };

        let has_nodis_or_exdis = page
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, NonIcDissem::Nodis | NonIcDissem::Exdis));
        if !has_nodis_or_exdis {
            return vec![];
        }

        // Point at the first RelToBlock span so the user sees exactly where
        // the offending REL TO is.
        //
        // Parser invariant (verified against `parse_rel_to_with_spans`, the
        // sole producer of `rel_to` entries): every push into `rel_to` is
        // immediately preceded by a `TokenKind::RelToBlock` `TokenSpan` push
        // at the two call sites in `marque-core::parser`. The
        // `attrs.rel_to.is_empty()` guard above means we reach this site
        // only when `rel_to` is non-empty, therefore the `find()` MUST
        // succeed. The `else` arm is defense-in-depth against future parser
        // changes that would violate the invariant; uses the same let-else
        // shape as S010, with a `debug_assert!` on the invariant itself
        // (not on a constant) so dev/test builds panic loud if the parser
        // ever drops the RelToBlock span while keeping `rel_to` populated.
        debug_assert!(
            attrs
                .token_spans
                .iter()
                .any(|t| t.kind == TokenKind::RelToBlock),
            "E039: candidate with non-empty rel_to has no RelToBlock token span \
             (parser invariant violation; see parse_rel_to_with_spans call sites \
             in marque-core::parser)"
        );
        let Some(block) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::RelToBlock)
        else {
            return vec![];
        };
        let span = block.span;

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            Message::new(
                MessageTemplate::ConflictsWith,
                MessageArgs {
                    category: Some(crate::scheme::CAT_REL_TO),
                    ..MessageArgs::default()
                },
            ),
            // Typed Citation anchors at §H.9 p172 (EXDIS grammar);
            // the §H.9 p174 (NODIS) cross-reference lives in the
            // rule doc comment.
            capco(SectionLetter::H, 9, 172),
            None,
        )]
    }
}

// ===========================================================================
// PR 3b Sub-move A — banner-roll-up walker (T026a)
// ===========================================================================
//
// `BannerMatchesProjectedRule` collapses three literal banner-roll-up rules
// (E031 SAR, E035 SCI, E040 Non-IC dissem) into a single generic walker
// dispatched over a per-category catalog. Each row carries its own rule ID,
// citation, severity, and `evaluate` fn — so emitted diagnostics keep the
// historical rule IDs (E031 / E035 / E040) for audit-stream continuity and
// the C-1 overlap-guard interaction with E028 / E029 is preserved byte-for-
// byte. The walker's own `id()` is a bookkeeping ID (`E031`, the lowest of
// the retiring trio); the rule loop tracks via the per-row IDs on each
// emitted `Diagnostic`.
//
// Per `specs/006-engine-rule-refactor/tasks.md` T026a (D13 single-citation
// discipline): each catalog row carries ONE operative banner-roll-up
// CAPCO-§ citation. Background §-references are permitted in row
// documentation but are not counted as the row's primary citation.
//
// The `evaluate_*` fns are verbatim moves of the bodies of the retiring
// rules' `check` methods; the only structural change is that they take an
// explicit `&ProjectedMarking` parameter (the marking-type guard and the
// `ctx.page_marking.as_ref()` guard moved up to the walker's `check`).

/// Walker that asserts the banner / CAB candidate matches the page's
/// projected marking for each per-category roll-up. See the section header
/// above for the design rationale.
pub(crate) struct BannerMatchesProjectedRule;

/// Citations the [`BannerMatchesProjectedRule`] walker may emit on
/// diagnostics, one per catalog row in [`BANNER_CATEGORY_CATALOG`].
/// The walker registers under `E031` (bookkeeping ID); emitted
/// diagnostics carry per-row IDs and per-row citations from this
/// list. See [`Rule::cited_authorities`] for the F.1
/// corpus-fidelity gate contract.
const BANNER_MATCHES_PROJECTED_AUTHORITIES: &[Citation] = &[
    // SAR roll-up (E031) — §H.5 p101 "All unique SAPs contained in
    // portion marks must always appear in the banner line."
    capco(SectionLetter::H, 5, 101),
    // SCI roll-up (E035) — §H.4 p61 "Use the following syntax rules
    // for both portion marks and banner lines for all published and
    // unpublished SCI control systems."
    capco(SectionLetter::H, 4, 61),
    // Non-IC dissem roll-up (E040) — §H.9 p172 (EXDIS) with §H.9
    // p174 (NODIS) cross-reference; the typed Citation anchors at
    // p172. Both are operative per the walker's evaluator doc.
    capco(SectionLetter::H, 9, 172),
    // E068 banner classification mismatch — §H.7 p123 (Precedence
    // Rules for Banner Line Guidance + reciprocal classification).
    capco(SectionLetter::H, 7, 123),
    // E069 banner FGI marker mismatch — §H.7 p124 (FGI banner-line
    // roll-up + source-concealed-dominates rule).
    capco(SectionLetter::H, 7, 124),
];

impl Rule<CapcoScheme> for BannerMatchesProjectedRule {
    fn id(&self) -> RuleId {
        // Bookkeeping ID. Per-row IDs travel on emitted diagnostics for
        // audit traceability. The walker's registered tuple IS the SAR
        // roll-up tuple per the T044 legacy-rule-id-map §5.
        RuleId::new("capco", "banner.banner-rollup.sar-portions-roll-up")
    }

    fn name(&self) -> &'static str {
        "banner-matches-projected"
    }

    fn default_severity(&self) -> Severity {
        // Per-row severities take precedence on emitted diagnostics; the
        // walker-level default severity is the strictest of the three
        // catalog rows so a config that uses `BannerMatchesProjectedRule`
        // as the override anchor cannot accidentally weaken any row below
        // its authoring intent.
        Severity::Error
    }
    /// Phase::WholeMarking: banner roll-up walker (E031 SAR / E035 SCI /
    /// E040 Non-IC dissem). Every row reads the page projection across
    /// all portions and compares against the banner; fixes (when emitted)
    /// span the banner candidate.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        BANNER_MATCHES_PROJECTED_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::MarkingType;

        // Marking-type guard (≤3 branches per D13). CABs carry only
        // authority fields (Classified By / Derived From / Declassify
        // On) — they have no classification, SCI, dissem, or FGI
        // blocks — so every row evaluator would spuriously fire
        // "banner missing X block" with a placeholder (0,0) span.
        if !matches!(ctx.marking_type, MarkingType::Banner) {
            return vec![];
        }
        // PR 9b (T133 / FR-006): banner-validation rules read the
        // rolled-up shape via `ctx.page_marking` (the
        // `ProjectedMarking` projection) instead of going through
        // the retired `PageContext::expected_*` accessors. The
        // per-portion view is available via `ctx.page_portions`
        // (e.g. S005 post-PR-#488 — formerly the S005/S006 pair).
        let Some(page) = ctx.page_marking.as_ref() else {
            return vec![];
        };
        // Dispatch loop.
        let mut diags = Vec::new();
        for row in BANNER_CATEGORY_CATALOG {
            diags.extend((row.evaluate)(attrs, page, row));
        }
        diags
    }

    /// Catalog (id, name) pairs the walker emits on diagnostics beyond
    /// its registered `id()` / `name()`. Required by the engine's
    /// `canonicalize_rule_overrides` path so a `.marque.toml`
    /// configuring `E035 = "warn"` (or `sci-banner-rollup = "warn"`,
    /// the historical name from the retired `SciBannerRollupRule`) is
    /// accepted at engine construction.
    ///
    /// Each pair is self-canonical: the catalog ID maps to itself, the
    /// catalog name maps to the catalog ID. This keeps per-row override
    /// scope independent of the walker's bookkeeping ID. The historical
    /// names (`sar-banner-rollup`, `sci-banner-rollup`,
    /// `nodis-exdis-banner-rollup`) match the retired rules' `name()`
    /// values so existing configs that used the name form keep working
    /// across the T026a refactor.
    fn additional_emitted_ids(&self) -> &'static [(&'static str, &'static str)] {
        // Post-T044: the first column is the canonical wire-string form
        // (`<scheme>:<predicate_id>`); the second column is the
        // descriptive `name()` alias users may also type in
        // `.marque.toml`.
        &[
            (
                "capco:banner.banner-rollup.sar-portions-roll-up",
                "sar-banner-rollup",
            ),
            (
                "capco:banner.banner-rollup.sci-portions-roll-up",
                "sci-banner-rollup",
            ),
            (
                "capco:banner.banner-rollup.non-ic-dissem-roll-up",
                "nodis-exdis-banner-rollup",
            ),
            // PR 5 (006 T059a, closes #276): foreign-banner mismatch
            // rows on the same walker. Per-row IDs travel on emitted
            // diagnostics for audit traceability; the additional-
            // emitted-ids list lets `.marque.toml` configure
            // `capco:banner.classification.mismatch-vs-projected = "warn"`
            // / `capco:banner.fgi.marker-mismatch-vs-projected = "warn"`
            // even though the walker's `id()` is the SAR roll-up tuple.
            (
                "capco:banner.classification.mismatch-vs-projected",
                "banner-classification-mismatch",
            ),
            (
                "capco:banner.fgi.marker-mismatch-vs-projected",
                "banner-fgi-marker-mismatch",
            ),
        ]
    }
}

/// One catalog row per banner-roll-up category. Ordering of rows controls
/// only the order of emitted diagnostics for a single banner candidate; it
/// does not affect correctness.
struct BannerCategoryRow {
    /// Rule ID emitted on diagnostics from this row. Distinct from the
    /// walker's own `RuleId`, which is bookkeeping only — the audit
    /// stream and the FR-016 overlap-guard tiebreaker both key on the
    /// per-row ID.
    rule_id: RuleId,
    /// Per-row default severity. The walker copies this onto each emitted
    /// `Diagnostic`; the engine's severity-override layer can downgrade
    /// or upgrade per the user's `.marque.toml`.
    severity: Severity,
    /// Pure function returning the diagnostics this row produces for
    /// the given banner attributes and page projection. Implemented as a
    /// fn pointer so the catalog can be a `const`.
    ///
    /// PR 9b (T133 / FR-006): receives `&ProjectedMarking` (the
    /// engine-facing rolled-up shape) instead of `&PageContext`.
    /// Banner-validation rules don't need per-portion membership —
    /// the union/intersection/max math is already performed by the
    /// projection at the engine boundary.
    evaluate: fn(
        &CanonicalAttrs,
        &marque_ism::ProjectedMarking,
        &BannerCategoryRow,
    ) -> Vec<Diagnostic<CapcoScheme>>,
}

const BANNER_CATEGORY_CATALOG: &[BannerCategoryRow] = &[
    // SAR — §H.5 p101: "Unique SAPs contained in portion marks must
    // always appear in the banner line." Banner hierarchy depiction
    // (compartments / sub-compartments) is optional per §H.5 p101 +
    // p99; the walker matches by program identifier only. Severity
    // `Fix` because the with-block case has a deterministic zero-width
    // insertion fix; the no-block case escalates to `Error` inside the
    // evaluator (banner-positioning a new SAR block from rule context
    // alone is unsafe).
    BannerCategoryRow {
        rule_id: RuleId::new("capco", "banner.banner-rollup.sar-portions-roll-up"),
        severity: Severity::Fix,
        evaluate: evaluate_sar_banner_rollup,
    },
    // SCI — per-system "Precedence Rules for Banner Line Guidance" in
    // §H.4 (e.g. HCS p62, SI p74, TK p85; one of 18 identical
    // instances): "All unique SCI markings contained in the portion
    // marks must always appear in the banner line." Unlike SAR, §H.4
    // contains no hierarchy-optional carve-out, so compartments and
    // sub-compartments are also rolled up.
    BannerCategoryRow {
        rule_id: RuleId::new("capco", "banner.banner-rollup.sci-portions-roll-up"),
        severity: Severity::Error,
        evaluate: evaluate_sci_banner_rollup,
    },
    // Non-IC dissem — §H.9 p174 (NODIS) and §H.9 p172 (EXDIS): NODIS
    // takes priority over EXDIS, and either token, if present in any
    // portion, must roll up to the banner. Both passages are the
    // operative supersession-and-roll-up rule for this category.
    BannerCategoryRow {
        rule_id: RuleId::new("capco", "banner.banner-rollup.non-ic-dissem-roll-up"),
        severity: Severity::Error,
        evaluate: evaluate_non_ic_dissem_banner_rollup,
    },
    // E068 — Banner classification mismatch (PR 5, closes #276).
    //
    // Fires when the observed banner's classification disagrees with
    // the projected page-level classification (Us/Fgi/Nato/Joint/
    // Conflict variant or effective level). Severity `Error`, no fix:
    // cross-axis byte-positioning a missing or wrong classification
    // block from rule context alone is unsafe; deterministic fix
    // requires renderer-level coordination not yet wired. The
    // renderer produces canonical output via `fix`; `lint` surfaces
    // the mismatch only.
    //
    // Authority: CAPCO-2016 §H.7 pp123-125 (reciprocal classification
    // grammar — `(U) Precedence Rules for Banner Line Guidance` on
    // p124 covers the FGI / classification ladder roll-up; the
    // worked examples on pp126-129 anchor the cross-axis
    // composition).
    BannerCategoryRow {
        rule_id: RuleId::new("capco", "banner.classification.mismatch-vs-projected"),
        severity: Severity::Error,
        evaluate: evaluate_classification_banner_rollup,
    },
    // E069 — Banner FGI marker mismatch (PR 5, closes #276).
    //
    // Fires when the observed banner's FGI marker disagrees with the
    // projected page-level FGI marker (presence/absence; concealed vs
    // acknowledged variant). Severity `Error`, no fix — same
    // safety rationale as E068.
    //
    // Authority: CAPCO-2016 §H.7 p124 — *"Use FGI + Register, Annex
    // B trigraph country code(s) and/or Register Annex A tetragraph
    // code(s) in the banner line, unless the very fact that the
    // information is foreign government information must be
    // concealed."* Plus the source-concealed-dominates rule on the
    // same page: *"If any document contains portions of both
    // source-concealed FGI ... and source-acknowledged FGI, then
    // only the 'FGI' marking without the source trigraph(s)/
    // tetragraph(s) must appear in the banner line."* The §H.7 p127
    // worked example (`TOP SECRET//BOHEMIA//FGI AUS CAN DEU NATO//
    // NOFORN`) and §H.7 p129 worked example (`TOP SECRET//FGI CAN
    // DEU//NOFORN`) anchor the projection.
    BannerCategoryRow {
        rule_id: RuleId::new("capco", "banner.fgi.marker-mismatch-vs-projected"),
        severity: Severity::Error,
        evaluate: evaluate_fgi_marker_banner_rollup,
    },
];

// ---------------------------------------------------------------------------
// Per-row evaluators
// ---------------------------------------------------------------------------

/// SAR banner roll-up evaluator. Verbatim move of the body of
/// `SarBannerRollupRule::check`, parameterized over the page projection
/// and the catalog row (so the row supplies the rule ID + severity).
///
/// PR 9b (T133): reads `&ProjectedMarking` — the field
/// `sar_markings` carries the union of portion-contributed SAR
/// programs in the same shape `PageContext::expected_sar_marking`
/// used to compute.
///
/// Authority: CAPCO-2016 §H.5 p101 (Unique SAPs contained in portion
/// marks must always appear in the banner line; hierarchy depiction
/// optional per §H.5 p101 + p99).
fn evaluate_sar_banner_rollup(
    attrs: &CanonicalAttrs,
    page: &marque_ism::ProjectedMarking,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    let Some(expected) = page.sar_markings.as_ref() else {
        return vec![];
    };
    if expected.programs.is_empty() {
        return vec![];
    }

    // Compute the identifiers of programs missing from the
    // observed banner. Hierarchy (compartments / sub-compartments)
    // is deliberately NOT compared — §H.5 p101 makes
    // banner hierarchy depth optional even when portions carry
    // hierarchy. See the `sar_missing_programs` helper doc for
    // the authority trail.
    let missing_ids: Vec<&str> = sar_missing_programs(attrs.sar_markings.as_ref(), expected);
    if missing_ids.is_empty() {
        return vec![];
    }

    // Typed Citation anchors at §H.5 p101 (the SAR per-system banner
    // rule); the hierarchy-optional note at §H.5 p99 is cross-
    // referenced in the rule doc comment, not in the Citation field.
    const CITATION: Citation = capco(SectionLetter::H, 5, 101);

    // Sort missing identifiers per §H.5 p99 (ascending,
    // numeric first, then alpha) so the fix output is
    // deterministic and self-canonical for the new tail.
    let mut sorted_missing = missing_ids.clone();
    sorted_missing.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));

    match attrs.sar_markings.as_ref() {
        Some(_observed) => {
            // PR 3c.2.C C4 / G13: drop the runtime program list from
            // the typed `Message`. `MessageArgs.category =
            // Some(CAT_SAR)` identifies the axis. The canonical
            // replacement still rides on `Diagnostic.text_correction.replacement`.
            let message = Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs {
                    category: Some(crate::scheme::CAT_SAR),
                    ..MessageArgs::default()
                },
            );
            // Banner has a SAR block. Emit a RIGHT-ALIGNED INSERTION
            // fix at the end of the block so it does not overlap
            // with E028 (program-order, whole-block span) or E029
            // (compartment-order, last program's span) when they
            // fire on the same marking.
            //
            // Why insertion and not a whole-block rewrite: the
            // engine's C-1 overlap guard (FR-016 + `span.end <=
            // boundary`) drops overlapping fixes. If E031's fix
            // were a whole-block rewrite covering the same
            // `sar_block_span` as E028, the lexicographic rule-id
            // tiebreaker would favor E028, silently dropping the
            // missing-program addition. A zero-width span at the
            // block's end byte has `span.start == block_end`, so
            // it sorts FIRST under FR-016 (`span.start DESC`) and
            // its `span.start` becomes the boundary; E028's
            // subsequent `span.end == block_end` still satisfies
            // `<= boundary` and is kept. Both fixes apply.
            //
            // Single-apply convergence: when E028 and E031 both
            // fire, the first apply pass produces
            // `<observed-sorted>/<missing-sorted>` which may not
            // be fully canonical (the inserted missing programs
            // are suffix-appended, not merge-sorted). A second
            // `marque fix` pass will detect and repair that via
            // E028 alone. Net: never loses missing programs,
            // never overflows into E028/E029 territory, and
            // converges in ≤2 passes. The prior whole-block
            // fix dropped silently in the overlap case and
            // required 2 passes anyway — this is strictly
            // better.
            let Some(block) = sar_block_span(attrs) else {
                return vec![];
            };
            let insertion_span = Span::new(block.end, block.end);
            // Replacement: `/PROG1/PROG2` — leading slash separates
            // the inserted run from the last existing program
            // per §H.5 p100 bullet 4 (`/` between
            // program identifiers, no interjected spaces).
            let replacement = format!("/{}", sorted_missing.join("/"));

            vec![make_fix_diagnostic(FixDiagnosticParams {
                rule: row.rule_id,
                severity: row.severity,
                source: FixSource::BuiltinRule,
                span: insertion_span,
                message,
                citation: CITATION,
                // Zero-width insertion: `original` is empty to match
                // `span.start..span.end` being a zero-length slice.
                original: String::new(),
                replacement,
                confidence: 0.9,
                migration_ref: None,
            })]
        }
        None => {
            // No SAR block in the banner at all. Byte-positioning a new
            // block between SCI and AEA from rule context alone is
            // unsafe — report at Error severity with no fix and let a
            // human place the block.
            //
            // G13: the typed `Message` identifies the banner-rollup
            // mismatch class with category=CAT_SAR. Per-program detail
            // would require coordinated `MARQUE_AUDIT_SCHEMA` bump
            // (out of C scope per PM-C-6).
            let _ = sorted_missing;
            let span = attrs
                .token_spans
                .first()
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));
            vec![Diagnostic::new(
                row.rule_id,
                Severity::Error,
                span,
                Message::new(
                    MessageTemplate::BannerRollupMismatch,
                    MessageArgs {
                        category: Some(crate::scheme::CAT_SAR),
                        ..MessageArgs::default()
                    },
                ),
                CITATION,
                None,
            )]
        }
    }
}

/// SCI banner roll-up evaluator. Verbatim move of the body of
/// `SciBannerRollupRule::check`, parameterized over the page projection
/// and the catalog row.
///
/// **Operative authority**: CAPCO-2016 §H.4 per-system "Precedence
/// Rules for Banner Line Guidance" template (HCS p62, SI p74, TK p85,
/// …) — *"All unique SCI markings contained in the portion marks must
/// always appear in the banner line."* Unlike SAR (§H.5 p101), SCI
/// has no hierarchy-optional carve-out: compartments and
/// sub-compartments roll up too.
///
/// **Background**: §D.2 p28 (CAPCO-2016 lines 577–579) restates the
/// same banner/portion consistency invariant in general-algorithm
/// prose. Per T026a D13 single-citation discipline (and
/// `specs/006-engine-rule-refactor/tasks.md` T026a wording — *"§D.2
/// is general-algorithm prose (per-category citations are tighter and
/// verifiable per Constitution VIII)"*), §D.2 is a background pointer
/// only and is deliberately NOT included in `E035_CITATION`. The
/// per-category §H.4 reference is the row's primary citation.
fn evaluate_sci_banner_rollup(
    attrs: &CanonicalAttrs,
    page: &marque_ism::ProjectedMarking,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    // PR 9b (T133): SCI rollup reads `page.sci_markings` directly.
    // `ProjectedMarking` carries the union with §A.6 ordering already
    // applied by `PageContext::expected_sci_markings`.
    let expected: Vec<marque_ism::SciMarking> = page.sci_markings.to_vec();
    if expected.is_empty() {
        // Either P4 has not landed yet (helper returns empty) or no
        // portions have been accumulated. Either way, nothing to check.
        return vec![];
    }

    let mut missing: Vec<String> = Vec::new();
    for exp in expected.iter() {
        let exp_key = sci_system_text(&exp.system);
        let observed = attrs
            .sci_markings
            .iter()
            .find(|m| sci_system_text(&m.system) == exp_key);
        match observed {
            None => {
                missing.push(format!("{} (system missing from banner)", exp_key));
            }
            Some(obs) => {
                // Compartment check: every expected compartment must
                // appear in the observed marking.
                for exp_comp in exp.compartments.iter() {
                    let obs_comp = obs
                        .compartments
                        .iter()
                        .find(|c| c.identifier == exp_comp.identifier);
                    match obs_comp {
                        None => {
                            missing.push(format!(
                                "{}-{} (compartment missing from banner)",
                                exp_key,
                                exp_comp.identifier.as_str()
                            ));
                        }
                        Some(oc) => {
                            for exp_sub in exp_comp.sub_compartments.iter() {
                                if !oc.sub_compartments.iter().any(|s| s == exp_sub) {
                                    missing.push(format!(
                                        "{}-{} {} (sub-compartment missing from banner)",
                                        exp_key,
                                        exp_comp.identifier.as_str(),
                                        exp_sub.as_str()
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if missing.is_empty() {
        return vec![];
    }

    // Fix: replace the observed SCI block with the fully-rolled-up
    // form. The fix span covers every SciControl block token in order.
    let chunk_spans: Vec<&TokenSpan> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::SciControl)
        .collect();

    if chunk_spans.is_empty() {
        // Banner has no SCI block at all. Byte-positioning a new
        // block between classification and the next category from
        // rule context alone is unsafe (requires knowing the
        // separator offsets and the downstream block boundaries).
        // Escalate severity and emit a diagnostic without a fix
        // so the author inserts the block by hand.
        // G13: per-system detail dropped from the typed `Message`;
        // category=CAT_SCI identifies the axis.
        return vec![Diagnostic::new(
            row.rule_id,
            Severity::Error,
            Span::new(0, 0),
            Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs {
                    category: Some(crate::scheme::CAT_SCI),
                    ..MessageArgs::default()
                },
            ),
            E035_CITATION,
            None,
        )];
    }

    let fix_start = chunk_spans.first().unwrap().span.start;
    let fix_end = chunk_spans.last().unwrap().span.end;
    let original: String = chunk_spans
        .iter()
        .map(|t| t.text.as_ref())
        .collect::<Vec<_>>()
        .join("/");
    let fix_span = Span::new(fix_start, fix_end);
    let replacement = render_sci_block(&expected);

    // G13 (PM-C-6): drop the per-system `missing` list from the typed
    // `Message`. `MessageArgs.category = Some(CAT_SCI)` identifies the
    // axis that disagreed; per-system detail does NOT belong on the
    // audit record (would require a `MessageArgs.feature_ids`
    // population that needs a coordinated `MARQUE_AUDIT_SCHEMA` bump per
    // PM-C-6). The canonical replacement still rides on
    // `Diagnostic.text_correction.replacement` for the engine's apply path.
    vec![make_fix_diagnostic(FixDiagnosticParams {
        rule: row.rule_id,
        severity: row.severity,
        source: FixSource::BuiltinRule,
        span: fix_span,
        message: Message::new(
            MessageTemplate::BannerRollupMismatch,
            MessageArgs {
                category: Some(crate::scheme::CAT_SCI),
                ..MessageArgs::default()
            },
        ),
        citation: E035_CITATION,
        original,
        replacement,
        confidence: 0.9,
        migration_ref: None,
    })]
}

/// Non-IC dissem banner roll-up evaluator. Verbatim move of the body of
/// `NodisExdisBannerRollupRule::check`, parameterized over the page
/// projection and the catalog row.
///
/// Authority: CAPCO-2016 §H.9 p174 (NODIS) + §H.9 p172 (EXDIS) — NODIS
/// has priority over EXDIS in the banner; either token, if present in
/// any portion, must roll up. The single operative supersession-and-
/// roll-up rule.
fn evaluate_non_ic_dissem_banner_rollup(
    attrs: &CanonicalAttrs,
    page: &marque_ism::ProjectedMarking,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    use marque_ism::NonIcDissem;

    // PR 9b (T133): the NODIS/EXDIS supersession logic in
    // `PageContext::expected_non_ic_dissem` is preserved inside
    // `PageContext::project` (the `non_ic_dissem` field on the
    // projection comes from `expected_non_ic_dissem().0`). The
    // second tuple element (`needs_nf` injection signal) is
    // intentionally not surfaced here — this evaluator does not
    // consume it. If a future change needs it, plumb it through
    // either a `ProjectionProvenance` extension or a dedicated
    // accessor that returns the pre-projection
    // `(non_ic, needs_nf)` pair.
    let portions_have_nodis = page
        .non_ic_dissem
        .iter()
        .any(|d| matches!(d, NonIcDissem::Nodis));
    let portions_have_exdis = page
        .non_ic_dissem
        .iter()
        .any(|d| matches!(d, NonIcDissem::Exdis));

    // Determine what the banner MUST carry per §H.9. NODIS has
    // priority over EXDIS; if any portion has NODIS, the banner
    // must have NODIS even if other portions have EXDIS.
    let required = if portions_have_nodis {
        NonIcDissem::Nodis
    } else if portions_have_exdis {
        NonIcDissem::Exdis
    } else {
        return vec![];
    };

    let banner_has_required = attrs.non_ic_dissem.contains(&required);
    if banner_has_required {
        return vec![];
    }

    let required_str = required.banner_str();
    // PR 3c.2.C C5: both arms now use the typed `Message` shape.
    // §H.9 p172 (EXDIS) and §H.9 p174 (NODIS) — typed Citation
    // anchors at p172; the p174 cross-reference lives in the rule
    // doc comment.
    const CITATION: Citation = capco(SectionLetter::H, 9, 172);

    // Fix: if banner has at least one Non-IC dissem token, emit a
    // zero-width insertion at the end of that category block
    // appending `/<required>`. Otherwise, no-fix Error.
    let last_non_ic_span = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::NonIcDissem)
        .map(|t| t.span)
        .next_back();

    match last_non_ic_span {
        Some(last_span) => {
            let insertion = Span::new(last_span.end, last_span.end);
            let replacement = format!("/{required_str}");
            vec![make_fix_diagnostic(FixDiagnosticParams {
                rule: row.rule_id,
                severity: row.severity,
                source: FixSource::BuiltinRule,
                span: insertion,
                message: Message::new(
                    MessageTemplate::BannerRollupMismatch,
                    MessageArgs {
                        category: Some(crate::scheme::CAT_NON_IC_DISSEM),
                        ..MessageArgs::default()
                    },
                ),
                citation: CITATION,
                original: String::new(),
                replacement,
                confidence: 0.9,
                migration_ref: None,
            })]
        }
        None => {
            // No Non-IC dissem block in banner at all. Byte-
            // positioning a new block requires separator offsets
            // the rule cannot safely supply. No fix.
            let span = attrs
                .token_spans
                .first()
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));
            // G13: drop the runtime `required_str` interpolation.
            let _ = required_str;
            vec![Diagnostic::new(
                row.rule_id,
                Severity::Error,
                span,
                Message::new(
                    MessageTemplate::BannerRollupMismatch,
                    MessageArgs {
                        category: Some(crate::scheme::CAT_NON_IC_DISSEM),
                        ..MessageArgs::default()
                    },
                ),
                CITATION,
                None,
            )]
        }
    }
}

// ---------------------------------------------------------------------------
// E068 — Banner classification mismatch (PR 5 / #276)
// ---------------------------------------------------------------------------

/// E068 evaluator: banner classification disagrees with the projected
/// page state. Pure no-fix Error per CAPCO-2016 §H.7 pp123-125
/// reciprocal classification ladder and the worked examples on
/// pp126-129.
///
/// Detection cases (Constitution V G13: no document values
/// interpolated into the message; the message describes the axis
/// only, not the observed or projected values):
///
/// 1. Banner has no classification but page projects one
///    (`(None, Some(_))`). The banner is missing the classification
///    block required by the portions.
/// 2. Banner has a classification but page projects none
///    (`(Some(_), None)`). The banner is over-classified relative to
///    the (empty) projected page state — an odd but possible shape
///    when banner-only candidates appear.
/// 3. Banner classification level disagrees with the projected
///    effective level (`a.effective_level() != b.effective_level()`).
///    Covers the §H.7 p129 worked example case (`TOP SECRET//FGI
///    CAN DEU//NOFORN`) where portions roll up to `TopSecret` but
///    banner observes `Secret`.
/// 4. Banner classification VARIANT disagrees with the projected
///    variant (e.g. `Us(_)` observed when projection is `Fgi(_)` on
///    a pure-foreign page). Compared by [`classification_variant_rank`]
///    via discriminant equality. Covers the §H.7 pp123-125
///    solely-foreign preservation case.
///
/// **Authority**: CAPCO-2016 §H.7 pp123-125 (Precedence Rules for
/// Banner Line Guidance + reciprocal classification grammar). The
/// worked examples on pp126-129 anchor the cross-axis composition.
///
/// **Constitution VII (scheme-adoption boundary)**: this evaluator
/// is scheme-internal (`marque-capco`). No engine-crate touch.
fn evaluate_classification_banner_rollup(
    attrs: &CanonicalAttrs,
    page: &marque_ism::ProjectedMarking,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    // Discriminator helper: distinguish MarkingClassification variants
    // without leaking the contained values. Mirrors
    // `classification_variant_rank` in `marque-capco::lattice` but
    // local to this evaluator (the lattice helper is `pub(crate)`
    // there and we re-derive locally to avoid coupling rule emission
    // to lattice internals).
    fn variant_kind(c: &MarkingClassification) -> u8 {
        match c {
            MarkingClassification::Us(_) => 0,
            MarkingClassification::Fgi(_) => 1,
            MarkingClassification::Nato(_) => 2,
            MarkingClassification::Joint(_) => 3,
            MarkingClassification::Conflict { .. } => 4,
        }
    }

    // PR 3c.2.C C5 / G13: collapse the 4 string-literal reasons into
    // the typed `MessageTemplate::BannerRollupMismatch` with
    // `category=CAT_CLASSIFICATION`. The narrative distinction
    // (missing / over-classified / level-disagrees / variant-disagrees)
    // moves into the rule doc comment; the audit record carries only
    // the closed-set identifier.
    let has_mismatch = match (attrs.classification.as_ref(), page.classification.as_ref()) {
        (None, None) => false,
        (None, Some(_)) | (Some(_), None) => true,
        (Some(observed), Some(projected)) => {
            observed.effective_level() != projected.effective_level()
                || variant_kind(observed) != variant_kind(projected)
        }
    };

    if !has_mismatch {
        return vec![];
    }

    // Span: point at the first token of the banner candidate so the
    // user can locate the offending line. Per Constitution V G13 the
    // span is structural metadata, not document content.
    let span = attrs
        .token_spans
        .first()
        .map(|t| t.span)
        .unwrap_or(Span::new(0, 0));

    // Typed Citation anchors at §H.7 p123 (Precedence Rules for
    // Banner Line Guidance + reciprocal classification); worked
    // examples §H.7 pp126-129 cross-referenced in the rule doc.
    const CITATION: Citation = capco(SectionLetter::H, 7, 123);

    vec![Diagnostic::new(
        row.rule_id,
        row.severity,
        span,
        Message::new(
            MessageTemplate::BannerRollupMismatch,
            MessageArgs {
                category: Some(crate::scheme::CAT_CLASSIFICATION),
                ..MessageArgs::default()
            },
        ),
        CITATION,
        None,
    )]
}

// ---------------------------------------------------------------------------
// E069 — Banner FGI marker mismatch (PR 5 / #276)
// ---------------------------------------------------------------------------

/// E069 evaluator: banner FGI marker disagrees with the projected
/// page state. Pure no-fix Error.
///
/// Detection cases (Constitution V G13: no country code values
/// interpolated into the message):
///
/// 1. Banner has no FGI marker but page projects one
///    (`(None, Some(_))`). Covers the §H.7 p129 worked example
///    case (`TOP SECRET//FGI CAN DEU//NOFORN`) where the portions
///    carry FGI provenance but the banner omits it.
/// 2. Banner has an FGI marker but page projects none
///    (`(Some(_), None)`). Banner over-claims foreign provenance.
/// 3. Banner FGI variant disagrees with projection — concealed vs
///    acknowledged. Covers the §H.7 p124
///    source-concealed-dominates rule: if any portion is
///    source-concealed, the banner MUST use bare `FGI` without a
///    trigraph list.
/// 4. Banner is acknowledged and projection is acknowledged but the
///    country sets differ. Covers the §H.7 p126 worked example
///    (`TOP SECRET//FGI CAN DEU//REL TO USA, CAN, DEU`) where the
///    union of portion-contributed FGI countries must appear in
///    the banner list.
///
/// **Authority**: CAPCO-2016 §H.7 p124 — *"Use FGI + Register, Annex
/// B trigraph country code(s) ... in the banner line, unless the
/// very fact that the information is foreign government information
/// must be concealed."* Plus the source-concealed-dominates rule on
/// the same page. The §H.7 p126 (`TOP SECRET//FGI CAN DEU//REL TO
/// USA, CAN, DEU`) and §H.7 p129 (`TOP SECRET//FGI CAN DEU//NOFORN`)
/// worked examples anchor the projection.
///
/// **Constitution VII (scheme-adoption boundary)**: scheme-internal;
/// no engine-crate touch.
fn evaluate_fgi_marker_banner_rollup(
    attrs: &CanonicalAttrs,
    page: &marque_ism::ProjectedMarking,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    use marque_ism::FgiMarker;

    // Discriminator helper for FgiMarker variant comparison without
    // touching the country lists (Constitution V G13).
    fn fgi_variant_kind(m: &FgiMarker) -> u8 {
        match m {
            FgiMarker::SourceConcealed => 0,
            FgiMarker::Acknowledged { .. } => 1,
        }
    }

    // PR 3c.2.C C5 / G13: 4 narrative reasons collapse to the typed
    // `MessageTemplate::BannerRollupMismatch` with category =
    // `CAT_FGI_MARKER`. The narrative distinction lives in the rule
    // doc comment.
    let has_mismatch = match (attrs.fgi_marker.as_ref(), page.fgi_marker.as_ref()) {
        (None, None) => false,
        (None, Some(_)) | (Some(_), None) => true,
        (Some(observed), Some(projected)) => {
            if fgi_variant_kind(observed) != fgi_variant_kind(projected) {
                true
            } else {
                // Compare country lists as SETS, not slices. The
                // observed side comes from the parser in textual
                // order (`parse_fgi_marker` pushes tokens left-to-
                // right); the projected side comes from
                // `FgiSet::to_marker()` which iterates a
                // `BTreeSet<CountryCode>` (sorted). Slice equality
                // would false-positive on non-canonically-ordered
                // (but otherwise-equivalent) banner input — e.g.,
                // `FGI NZL GBR` vs projected `[GBR, NZL]`. Ordering
                // is the renderer's concern (canonical form); E069
                // is supposed to fire on a missing or wrong country,
                // not on a valid-but-non-canonically-ordered country
                // list. The `BTreeSet` allocation only runs in this
                // branch, which is per-banner-candidate (O(pages),
                // not O(tokens)).
                use std::collections::BTreeSet;
                let observed_set: BTreeSet<_> = observed.countries().iter().copied().collect();
                let projected_set: BTreeSet<_> = projected.countries().iter().copied().collect();
                observed_set != projected_set
            }
        }
    };

    if !has_mismatch {
        return vec![];
    }

    let span = attrs
        .token_spans
        .first()
        .map(|t| t.span)
        .unwrap_or(Span::new(0, 0));

    // Typed Citation anchors at §H.7 p124 (banner-line FGI roll-up
    // rule + source-concealed-dominates); worked examples §H.7 p126
    // and §H.7 p129 cross-referenced in the rule doc.
    const CITATION: Citation = capco(SectionLetter::H, 7, 124);

    vec![Diagnostic::new(
        row.rule_id,
        row.severity,
        span,
        Message::new(
            MessageTemplate::BannerRollupMismatch,
            MessageArgs {
                category: Some(crate::scheme::CAT_FGI_MARKER),
                ..MessageArgs::default()
            },
        ),
        CITATION,
        None,
    )]
}

// ---------------------------------------------------------------------------
// Rule: E041 — Portion-level NODIS supersedes EXDIS
// ---------------------------------------------------------------------------

/// Fires when a portion carries BOTH NODIS and EXDIS. Emits a
/// `Warn`-severity diagnostic pointing at the EXDIS token and an
/// intent-only `FactRemove(EXDIS, Scope::Portion)` fix that the
/// engine auto-applies via the synthesis path. Per the supersession
/// rule in §H.9, NODIS survives and EXDIS is removed.
///
/// Authority:
/// - **CAPCO-2016 §H.9 p172** (EXDIS Commingling): *"When a
///   portion contains both EXDIS and NODIS information, NODIS (ND)
///   supersedes EXDIS (XD) in the portion mark."*
/// - **CAPCO-2016 §H.9 p174** (NODIS Commingling): *"If a
///   portion contains both NODIS and EXDIS information, NODIS (ND)
///   supersedes EXDIS (XD) in the portion mark."*
///
/// # Scope
///
/// Portion-only per both source passages ("in the portion mark").
/// The banner-level mutual exclusion is E037's territory — it fires
/// as `Error` there with no fix because banner-level resolution
/// depends on which portions carry which token (see E040's roll-up
/// rule for how the banner should be composed).
///
/// # Interaction with E037
///
/// E037 also fires in portion context (it's a general "NODIS and
/// EXDIS cannot coexist" rule per §H.9 p172 + p174). When a portion
/// has both tokens, both rules fire:
/// - E037 (`Error`, no fix) states the violation.
/// - E041 (`Warn`, intent-only `FactRemove`) states the supersession
///   rule: NODIS wins, so EXDIS is removed from the portion marking.
///
/// E037 emits no `FixProposal`, so the FR-016 deterministic ordering
/// (lex-min rule id wins on overlap) does not block E041's fix from
/// applying — E041 is the only diagnostic in the candidate-span group
/// that contributes an intent. After the engine applies E041, re-linting
/// the resulting portion clears both diagnostics.
///
/// # Severity and auto-fix surface
///
/// `Warn` default severity. The engine's intent-only synthesis path
/// auto-applies the fix for every severity *except* `Severity::Suggest`
/// (see `crates/engine/src/engine.rs::synthesize_intent_only_fixes`),
/// so the default emission auto-fixes. Orgs that want to surface
/// the supersession without applying it can configure
/// `E041 = "suggest"` in `.marque.toml`; orgs that want the violation
/// promoted to an error can configure `E041 = "error"`.
///
/// # Auto-fix mechanism (PR 3c.B Sub-PR 8.E.2 — unblocks E041, primary rule named in #106)
///
/// Pre-PR-3c.B-Sub-PR-8.E.2 this rule shipped as a no-fix diagnostic.
/// The blocker was that a byte-precise legacy [`FixProposal`] would
/// need to splice EXDIS *plus* an adjacent within-category `/`
/// separator, but the parser only emits `TokenKind::Separator` for
/// between-category `//` delimiters — within-category `/` bytes are
/// gap bytes that no `TokenSpan` covers. Constructing the legacy
/// proposal from rule-level position info risked over-running on
/// edge inputs and corrupting the audit record per Constitution V.
///
/// The intent-only emission path obviates that gap. The rule emits
/// `FixIntent { ReplacementIntent::FactRemove { TOK_EXDIS, Portion } }`
/// alongside the rule's `RuleContext::candidate_span` (the full
/// portion span, including the parentheses). The engine's
/// `synthesize_intent_only_fixes` calls `CapcoScheme::apply_intent`
/// to remove EXDIS from the marking's `non_ic_dissem` axis, then
/// re-renders the portion via `MarkingScheme::render_canonical`
/// (delegated to `render_portion`). The synthesized
/// `FixProposal.span` covers the full candidate, so the
/// within-category `/` byte is replaced as part of the re-rendered
/// portion — no parser change required. Issue #106 remains open as
/// a tracking ticket for any future rule that genuinely needs
/// byte-precise within-category separator info (i.e., a rule that
/// cannot route through re-rendering); E041 itself no longer
/// blocks on it.
struct NodisSupersedesExdisInPortionRule;

/// Citations E041 may emit on diagnostics. Primary anchor §H.9 p174
/// (NODIS — the dominating token); the §H.9 p172 (EXDIS) cross-
/// reference is also operative because both passages state the
/// supersession rule verbatim. See [`Rule::cited_authorities`] for
/// the F.1 corpus-fidelity gate contract.
const E041_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::H, 9, 174),
    capco(SectionLetter::H, 9, 172),
];

impl Rule<CapcoScheme> for NodisSupersedesExdisInPortionRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.dissem.nodis-supersedes-exdis-in-portion")
    }
    fn name(&self) -> &'static str {
        "nodis-supersedes-exdis-in-portion"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: emits `ReplacementIntent::FactRemove` at
    /// `Scope::Portion`; the engine re-renders the full portion via
    /// `candidate_span`. Span shape is whole-marking by construction.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E041_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{MarkingType, NonIcDissem};

        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        let has_nodis = attrs
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, NonIcDissem::Nodis));
        let has_exdis = attrs
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, NonIcDissem::Exdis));
        if !(has_nodis && has_exdis) {
            return vec![];
        }

        // Locate the EXDIS token span for the diagnostic pointer.
        // The parser emits one `TokenKind::NonIcDissem` token per
        // entry in `attrs.non_ic_dissem` in source order.
        let non_ic_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::NonIcDissem)
            .collect();
        let Some(exdis_idx) = attrs
            .non_ic_dissem
            .iter()
            .position(|d| matches!(d, NonIcDissem::Exdis))
        else {
            return vec![];
        };
        let Some(exdis_span_tok) = non_ic_spans.get(exdis_idx) else {
            return vec![];
        };

        // PR 3c.B Sub-PR 8.E.2 — intent-only emission (unblocks E041 in #106).
        // The diagnostic's `span` points at the EXDIS token (the
        // user-facing pointer); `candidate_span` is the full portion
        // candidate so the engine's `synthesize_intent_only_fixes`
        // knows which scope-bytes to re-render after
        // `CapcoScheme::apply_intent` removes EXDIS from the
        // marking's non-IC-dissem axis. The within-category `/`
        // separator that previously blocked byte-precise splicing is
        // sidestepped because the engine replaces the full
        // candidate_span with the re-rendered output — no parser
        // change required (see the `# Auto-fix mechanism` section in
        // the rustdoc above for the issue-#106 sidestep rationale).
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            exdis_span_tok.span,
            ctx.candidate_span,
            Message::new(
                MessageTemplate::ConflictsWith,
                MessageArgs {
                    category: Some(crate::scheme::CAT_NON_IC_DISSEM),
                    ..MessageArgs::default()
                },
            ),
            // Typed Citation anchors at §H.9 p174 (NODIS — the
            // dominating token); §H.9 p172 (EXDIS) cross-referenced
            // in the rule doc comment.
            capco(SectionLetter::H, 9, 174),
            nodis_supersedes_exdis_intent(),
        )]
    }
}

/// Build the `FactRemove { EXDIS, Scope::Portion }` intent emitted by
/// `NodisSupersedesExdisInPortionRule`. EXDIS is the rejected token
/// per §H.9 p172 + p174 ("NODIS (ND) supersedes EXDIS (XD) in the
/// portion mark"). Scope is portion-only: the supersession rule
/// names the portion mark explicitly at both source passages.
///
/// Confidence is `Confidence::strict(1.0)` — the source is
/// unambiguous about which token survives, and the strict recognizer
/// path is what produced the parse that surfaced both tokens.
///
/// `feature_ids` uses `Default::default()` (empty `SmallVec`) to
/// stay consistent with the other strict-path intent builders in
/// this crate (see `relido_remove_intent` in `rules_declarative.rs`).
fn nodis_supersedes_exdis_intent() -> FixIntent<CapcoScheme> {
    use crate::scheme::{TOK_EXDIS, TOK_NODIS};
    FixIntent {
        replacement: ReplacementIntent::fact_remove(FactRef::Cve(TOK_EXDIS), Scope::Portion),
        confidence: Confidence::strict(1.0),
        feature_ids: Default::default(),
        // `ConflictsWith` (not `SupersededToken`): §H.9 mutual-exclusion
        // with a dominated + surviving token, NOT §F deprecation /
        // canonical-replacement. `token` = the dominated EXDIS;
        // `expected_token` = the surviving NODIS.
        message: Message::new(
            MessageTemplate::ConflictsWith,
            MessageArgs {
                token: Some(TOK_EXDIS),
                expected_token: Some(TOK_NODIS),
                ..MessageArgs::default()
            },
        ),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    }
}

// ===========================================================================
// E066 — Legacy NATO compound text re-marking (PR 9c.1 T134)
// ===========================================================================
//
// Authority chain:
//   §G.2 p40 (Table 5: ARH by Registered Marking) lists ATOMAL,
//     BOHEMIA, and BALK as registered NATO control markings — each
//     has its own row with `Requires {marking} read-in`, confirming
//     they are control markings registered alongside (not fused with)
//     the NATO classification ladder.
//   §H.7 p122 worked example places ATOMAL in the AEA category
//     position: `SECRET//RD/ATOMAL//FGI NATO//NOFORN` ("ATOMAL is a
//     NATO Atomic Energy Act marking that follows the registered US
//     Atomic Energy Act marking RD").
//   §H.7 p127 worked example places BOHEMIA in the SCI category
//     position: `(//CTS//BOHEMIA//REL TO USA, NATO)`.
//   §G.1 Table 4 p38 portion-form column lists `ATOMAL` / `BALK` /
//     `BOHEMIA` as same-form across title / banner-abbrev / portion
//     (standalone canonical names — no `SAR-` prefix, no fused
//     class form).
//
// Per project memory `remark-on-derivative-use-is-marque-autofix`,
// "Marque exists precisely to automate the re-marking the manual
// permits doing by hand" — the §H.7 worked-example canonical forms
// ARE the autofix targets.
//
// E066 fires when the strict parser canonicalizes legacy NATO compound
// text (`CTSA`, `CTS-A`, `NSAT`, `NS-A`, `NCA`, `NC-A`, `CTS-B`,
// `CTS-BALK`, or their banner-form equivalents) into the canonical
// structural shape (bare class + AEA/SCI companion). The rule reads
// `attrs.token_spans` looking for a `TokenKind::Classification` whose
// text matches one of the legacy patterns; if found AND the parsed
// `attrs.classification` is a bare `NatoClassification::*` variant with
// the corresponding `AeaMarking::Atomal` / `SciControlSystem::NatoSap`
// companion present, the rule emits a Recanonicalize fix.
//
// The emitted FixIntent uses `ReplacementIntent::Recanonicalize {
// scope: Portion | Page }` — the engine re-renders the candidate via
// `MarkingScheme::render_canonical`, which emits the canonical
// multi-block form (`(//CTS//ATOMAL)`, `(//CTS//BOHEMIA)`, etc.) per
// the §H.7 p122 + §G.2 p40 + §H.7 p127 worked examples.
//
// Severity: `Fix` (auto-applies when confidence ≥ engine threshold).
// Confidence: `strict(1.0)` — the canonical form is unambiguous; the
// renderer produces deterministic bytes from the canonical attrs.
//
// G13 audit-content-ignorance: the diagnostic message does NOT echo
// input bytes. The message references canonical token names
// (`TOK_ATOMAL`, `TOK_BALK`, `TOK_BOHEMIA`) via `MessageArgs.token`
// and the message template's text. The fix payload is structural
// (`Recanonicalize`); the engine snapshots the canonical replacement
// at promotion time without any rule-side byte stringification.

/// Rule E066 — legacy NATO compound text re-marking per §G.2 p40
/// (Table 5 registration) + §H.7 p122 (ATOMAL → AEA) + §G.2 p40 +
/// §H.7 p127 (BALK/BOHEMIA → SCI).
struct LegacyNatoCompoundRemarkRule;

/// Citations E066 may emit on diagnostics. Two branches:
/// §H.7 p122 (ATOMAL → AEA position) and §H.7 p127 (BALK/BOHEMIA →
/// SCI position); the rule's `check()` selects per companion type.
/// See [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const E066_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::H, 7, 122),
    capco(SectionLetter::H, 7, 127),
];

impl Rule<CapcoScheme> for LegacyNatoCompoundRemarkRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "marking.recanonicalize.legacy-nato-compound")
    }
    fn name(&self) -> &'static str {
        "legacy-nato-compound-remark"
    }
    fn default_severity(&self) -> Severity {
        // Severity::Fix — auto-apply when confidence ≥ threshold. The
        // canonical form is unambiguous (single grammar; no
        // classifier-judgment branch); the renderer produces
        // deterministic bytes from the parsed canonical attrs.
        Severity::Fix
    }
    /// Phase::WholeMarking: the canonical re-rendering spans the full
    /// candidate (the classification block AND the appended AEA/SCI
    /// companion block need to land together), so the fix scope is
    /// whole-marking by construction.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        E066_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{
            AeaMarking, MarkingClassification, MarkingType, NatoSap, SciControlSystem,
        };

        // Gate on bare NATO classification + presence of an AeaMarking::Atomal
        // OR a SciControlSystem::NatoSap companion. The parser only
        // writes those companions for the canonicalized legacy text
        // paths (`CTSA`, `CTS-B`, etc., per crate::parser::parse_nato_classification).
        let is_nato = matches!(&attrs.classification, Some(MarkingClassification::Nato(_)));
        if !is_nato {
            return vec![];
        }

        let has_atomal = attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::Atomal(_)));
        let nato_sap = attrs.sci_markings.iter().find_map(|m| match m.system {
            SciControlSystem::NatoSap(sap) => Some(sap),
            _ => None,
        });
        if !has_atomal && nato_sap.is_none() {
            return vec![];
        }

        // Locate the classification TokenSpan and verify its raw text
        // matches one of the thirteen legacy compound patterns (eight
        // portion forms + five banner forms). The parser writes a
        // Classification token-span for the NATO block; the span's
        // `.text` carries the original bytes. We match against the
        // closed set of legacy forms — well-formed canonical multi-block
        // inputs (`(//CTS//ATOMAL)`, `(//CTS//BOHEMIA)`) will NOT match
        // here because the classification block in those inputs is just
        // `CTS`.
        let Some(classification_tok) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
        else {
            return vec![];
        };

        if !is_legacy_nato_compound_text(classification_tok.text.as_str()) {
            return vec![];
        }

        // Determine the canonical companion token for `MessageArgs.token`.
        // Per G13 audit-content-ignorance: this is a closed-vocab
        // TokenId, not raw bytes.
        let companion_token = if has_atomal {
            crate::scheme::TOK_ATOMAL
        } else {
            match nato_sap {
                Some(NatoSap::Balk) => crate::scheme::TOK_BALK,
                Some(NatoSap::Bohemia) => crate::scheme::TOK_BOHEMIA,
                // `None` is unreachable: the early-return above
                // guarantees exactly one of (has_atomal,
                // nato_sap.is_some()) is true at this point. The
                // `Some(_)` wildcard covers a future `NatoSap`
                // variant introduced after PR 9c.1 ([`NatoSap`] is
                // `#[non_exhaustive]`); defensively skip-emit until
                // the rule explicitly learns the new variant.
                None | Some(_) => return vec![],
            }
        };

        let scope = match ctx.marking_type {
            MarkingType::Portion => RecanonScope::Portion,
            _ => RecanonScope::Page,
        };

        // §G.2 p40 (Table 5: ARH by Registered Marking) registers
        // ATOMAL/BOHEMIA/BALK as control markings. §H.7 p122 worked
        // example shows ATOMAL in the AEA position; §H.7 p127 worked
        // example shows BOHEMIA in the SCI position. Choose the
        // structurally-most-precise anchor based on which companion
        // was written. The cross-axis §G.2 p40 reference lives in
        // the rule doc comment, not in the typed Citation field.
        let citation = if has_atomal {
            capco(SectionLetter::H, 7, 122)
        } else {
            capco(SectionLetter::H, 7, 127)
        };

        // G13: message template identifies the wrong-form class;
        // `MessageArgs.token` carries the canonical companion token.
        // The has_atomal vs has_natosap distinction is preserved
        // through `companion_token`.
        let message = Message::new(
            MessageTemplate::WrongTokenForm,
            MessageArgs {
                token: Some(companion_token),
                ..MessageArgs::default()
            },
        );

        let fix_intent = FixIntent {
            replacement: ReplacementIntent::Recanonicalize { scope },
            confidence: Confidence::strict(1.0),
            feature_ids: Default::default(),
            message: Message::new(
                MessageTemplate::WrongTokenForm,
                MessageArgs {
                    token: Some(companion_token),
                    ..MessageArgs::default()
                },
            ),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };

        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            classification_tok.span,
            ctx.candidate_span,
            message,
            citation,
            fix_intent,
        )]
    }
}

/// Returns `true` when `text` matches one of the thirteen legacy NATO
/// compound patterns (eight portion forms + five banner forms) retired
/// by PR 9c.1 T134.
///
/// The closed set is exactly the patterns the parser's
/// `parse_nato_classification` accepts in the legacy branch — anything
/// else is either canonical (`CTS`, `NS`, `NC`) or unrelated. Keeping
/// the predicate co-located with the rule (and citing the parser's
/// match table) means a future expansion of the legacy set requires a
/// coordinated edit in both places — the natural propagation point.
///
/// Citations: CAPCO-2016 §G.1 Table 4 p38 (portion-form column);
/// §G.2 p40 (Table 5 — registers ATOMAL/BOHEMIA/BALK as standalone
/// control markings, not classification suffixes).
fn is_legacy_nato_compound_text(text: &str) -> bool {
    matches!(
        text,
        // Portion forms.
        "CTSA"
            | "CTS-A"
            | "CTS-B"
            | "CTS-BALK"
            | "NSAT"
            | "NS-A"
            | "NCA"
            | "NC-A"
            // Banner forms (full-word legacy compounds).
            | "COSMIC TOP SECRET ATOMAL"
            | "COSMIC TOP SECRET-BOHEMIA"
            | "COSMIC TOP SECRET-BALK"
            | "NATO SECRET ATOMAL"
            | "NATO CONFIDENTIAL ATOMAL"
    )
}

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

// ===========================================================================
// W004 — JOINT producer-disunity collapse (issue #461 — Phase::PageFinalization)
// ===========================================================================
//
// Authority (verified 2026-05-16 against CAPCO-2016.md):
// - §H.3 p57 (JOINT not carried to banner in US documents — Derivative
//   Use bullets specify the FGI [LIST] migration trigger).
// - §H.7 p123 (FGI source-acknowledged form — the grammar the
//   migrated producers render under).
//
// Constitution V Principle V G13: the W004 diagnostic message MUST
// NOT contain document text. Permitted identifiers: `CountryCode`
// canonical trigraphs (vocabulary atoms), `Span` byte offsets,
// category IDs. The message template below uses placeholders only
// — no input bytes are interpolated.

/// JOINT producer-disunity collapse rule.
///
/// Fires when every portion on a page is JOINT-classified but the
/// portions disagree on their producer (country) list. The banner
/// cannot roll up the JOINT marking because the per-portion producer
/// lists don't share a unanimous set; per §H.3 p57 + §H.7 p123 the
/// non-US producers migrate to FGI [LIST], and JOINT is dropped from
/// the banner.
///
/// **Mixed JOINT + non-JOINT portions** (§H.3 p57 — "the JOINT
/// marking is not carried forward to the banner line in US
/// documents") do **NOT** fire W004. That case is `JointSet::Mixed`
/// (C-3 PR 4b-B follow-up — was `Bottom` pre-split) and is handled
/// by the existing PageContext-resident `expected_fgi_marker` path;
/// no W004 diagnostic emits on `Mixed`.
///
/// Severity: `Warn` (per `feedback_dissem_conflicts_emit_subtractive_fix.md`,
/// JOINT disunity is a subtractive-fix case).
///
/// **Fix payload deferred.** The cross-axis JOINT → FGI [LIST]
/// migration is a renderer-canonical concern, not a single-span text
/// replacement: a JOINT-disunity page has no banner JOINT block to
/// rewrite (§H.3 p57 says JOINT does not roll up to the banner), the
/// fix would have to edit each portion AND emit a new banner-shaped
/// FGI [LIST] elsewhere, and `Diagnostic::text_correction` /
/// `ReplacementIntent::FactAdd` / `FactRemove` / `Recanonicalize` are
/// all single-axis-scoped. The `MarkingScheme::render_canonical`
/// trait surface (PR 5+ Stage 4) is the right home for this
/// transformation. The W004 diagnostic surfaces the transformation
/// today so users have an audit trail without an auto-applied fix.
///
/// Authority: §H.3 p57 (JOINT not carried to banner — Derivative
/// Use bullets specify the FGI [LIST] migration trigger) + §H.7 p123
/// (FGI grammar). Verified 2026-05-16 against
/// `crates/capco/docs/CAPCO-2016.md`.
///
/// **Phase: PageFinalization (issue #461).** Pre-#461 W004 declared
/// `Phase::WholeMarking` and gated on `MarkingType::Banner`, which
/// produced a documented false-negative on banner-first layouts (no
/// closing banner → no Banner candidate ever runs against a
/// non-empty PageContext) AND a 6th-pass false-positive on Mixed
/// pages when the rule was briefly extended to Portion candidates
/// (intermediate snapshot misread as DisunityCollapse before the
/// final non-JOINT portion arrived). Phase::PageFinalization closes
/// both: the engine dispatches W004 once per page on the page-level
/// fixpoint snapshot, so banner-first layouts fire via the
/// end-of-document path and Mixed-page false-positives don't recur.
struct JointDisunityCollapseRule;

/// Citations W004 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const W004_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 3, 57)];

impl Rule<CapcoScheme> for JointDisunityCollapseRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.fgi.joint-disunity-collapses-to-fgi")
    }
    fn name(&self) -> &'static str {
        "joint-disunity-collapse"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::PageFinalization (issue #461): observes the
    /// page-level fixpoint snapshot of the classification axis. The
    /// engine dispatches this rule once per page at every
    /// scanner-emitted `MarkingType::PageBreak` BEFORE the
    /// PageContext reset, plus once at end-of-document. The
    /// pre-#461 Banner-only firing produced a documented
    /// false-negative on banner-first layouts (closed by the EOD
    /// path); the 6th-pass Portion-firing experiment produced a
    /// Mixed-page false-positive (not recur under PageFinalization
    /// because the rule fires exactly once per page on the closed
    /// state).
    fn phase(&self) -> Phase {
        Phase::PageFinalization
    }
    /// Trusted: implementation is a pure read-only check over
    /// `JointSet::from_attrs_iter`'s deterministic state machine plus
    /// a `format!` message synthesis using only `CountryCode` canonical
    /// trigraphs and a fixed §-citation. No mutable global state, no
    /// I/O, no allocation that could fail unexpectedly; the rule is
    /// safe to skip `catch_unwind` per PR #448.
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        W004_AUTHORITIES
    }
    fn check(&self, _attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Phase::PageFinalization invariant: the engine's
        // `dispatch_page_finalization` force-initializes
        // `ctx.page_portions` and `ctx.page_marking` before invoking
        // the rule. The defensive `.as_ref()?` early-return below is
        // belt-and-suspenders so the rule stays safe under future
        // engine refactors that might relax the invariant; it should
        // never fire in production.
        //
        // PR 4b-D.3 note (2026-05-18): W004 intentionally reads the
        // per-portion attrs slice rather than `ctx.page_marking`.
        // `JointSet::from_attrs_iter` requires the per-portion
        // `CanonicalAttrs` slice that `ProjectedMarking` does not
        // expose (the JointSet `DisunityCollapse` state is structurally
        // per-portion). Lifting `JointSet`'s derived state onto
        // `ProjectedMarking` is post-PR-6c future work.
        //
        // PR 6c migration (T069): read `ctx.page_portions` (the
        // `Box<[CanonicalAttrs]>` slice snapshot) instead of the
        // retired `ctx.page_context` / `PageContext::portions()`
        // accessor pair.
        let Some(page_portions) = ctx.page_portions.as_ref() else {
            return vec![];
        };
        let portions: &[CanonicalAttrs] = page_portions.as_ref();

        let joint_set = crate::lattice::JointSet::from_attrs_iter(portions);
        if !joint_set.is_disunity_collapse() {
            return vec![];
        }

        let Some(non_us) = joint_set.disunity_collapse_non_us_producers() else {
            return vec![];
        };

        // Render the producer set as canonical trigraphs, sorted
        // alphabetically. These are CountryCode vocabulary atoms;
        // no document text leaks per Constitution V G13.
        let producers_str: String = non_us
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        // Diagnostic span anchor: the engine passes a zero-length
        // `Span(boundary_offset, boundary_offset)` at the page-break
        // boundary (or `source.len()` for the EOD dispatch).
        // PageContext stores `Box<[CanonicalAttrs]>` for its portions
        // — no per-portion span is tracked. Per-portion span
        // precision would require extending the hot-path PageContext
        // data type for a single diagnostic with no fix, which the
        // PR brief judged scope-creep. The boundary anchor is the
        // best available pointer today; users joining "which page
        // had disunity?" map the byte offset to a page number via
        // their own document-position metadata.
        //
        // Authority: §H.3 p57 (Derivative Use bullets specify the
        // FGI [LIST] migration trigger) + §H.7 p123 (FGI grammar).
        // Re-verified 2026-05-16 against
        // `crates/capco/docs/CAPCO-2016.md`.
        // G13: drop the runtime `producers_str` interpolation. The
        // typed `MessageTemplate::BannerRollupMismatch` with
        // `category=CAT_JOINT_CLASSIFICATION` identifies the
        // collapse-to-FGI class. Typed Citation anchors at §H.3 p57
        // (Derivative Use FGI [LIST] migration trigger); the §H.7
        // p123 FGI grammar reference lives in the rule doc comment.
        let _ = producers_str;
        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            ctx.candidate_span,
            Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs {
                    category: Some(crate::scheme::CAT_JOINT_CLASSIFICATION),
                    ..MessageArgs::default()
                },
            ),
            capco(SectionLetter::H, 3, 57),
            None,
        )]
    }
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
    use super::{
        E005_CROSS_REFS, E037_CROSS_REFS, E038_CROSS_REFS, E039_CROSS_REFS, S003_CROSS_REFS,
    };
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
