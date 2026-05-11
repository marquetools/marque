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
//! Rule IDs follow the convention: E### = error, W### = warning, C### = correction.
//! Assignments per spec tasks.md:
//!   E001 = retired in PR 3c.B Commit 6 (form-bucket migration) —
//!           portion-mark-in-banner absorbed by
//!           `MarkingScheme::render_canonical`
//!   E002 = REL TO missing USA trigraph (T031); dual-pop migration tracked
//!           separately, retained for now
//!   E003 = retired in PR 3c.B Commit 6 — block ordering absorbed by
//!           the renderer
//!   E004 = retired in PR 3c.B Commit 6 — separator normalization
//!           absorbed by the renderer
//!   E005 = declassification misplaced (banner or portion; belongs in CAB) (T034)
//!   E006 = deprecated dissem control (T035)
//!   E007 = X-shorthand declass date (T036)
//!   E008 = unrecognized token (T037)
//!   E009 = retired in PR 3c.B Commit 6 — banner→portion form
//!           normalization absorbed by the renderer
//!   E010 = bare HCS without compartment suffix
//!   E011 = retired in PR 3c.B Commit 6 — `//`-prefix normalization on
//!           non-US classification absorbed by the renderer
//!   E012 = dual classification (US + foreign conflict)
//!   E013 = retired in PR 3c.B Commit 6 — list-delimiter normalization
//!           absorbed by the renderer
//!   E014 = JOINT participants missing from REL TO
//!   E015 = non-US classification without dissem control
//!   W001 = retired in T035c-14 (CAPCO-2016 §F treats legacy
//!           markings as unauthorized, not "deprecated but legal";
//!           no authoritative bucket for a warning-severity rule)
//!   W002 = US + FGI comingling in portion
//!   E016 = RESTRICTED not allowed with JOINT
//!   E017 = retired in T035b (over-restrictive per CAPCO §H.3 p57)
//!   E018 = retired in T035b (over-restrictive per CAPCO §H.3 p57)
//!   E019 = retired in T035b (over-restrictive per CAPCO §H.3 p57)
//!   E020 = retired in PR 3b.F (T026f) — country code list ordering
//!           rolled into E060; E060 retired in PR 3c.B Commit 6 into the
//!           renderer
//!   E021 = RD/FRD requires NOFORN (configurable to warn)
//!   E022 = CNWDI only with TS or S RD
//!   E023 = retired in PR 3b.F (T026f) — SIGMA ordering rolled into
//!           E060; E060 retired in PR 3c.B Commit 6 into the renderer
//!   E024 = RD precedence over FRD/TFNI
//!   E025 = UCNI only with UNCLASSIFIED
//!   E026 = retired in PR 3c.B Commit 6 — SAR portion form
//!           absorbed by the renderer
//!   E028 = retired in PR 3b.F (T026f) — SAR program ordering rolled
//!           into E060; E060 retired in PR 3c.B Commit 6 into the renderer
//!   E029 = retired in PR 3c.B Commit 6 — SAR compartment ordering
//!           absorbed by the renderer
//!   E030 = retired in PR 3c.B Commit 6 — SAR indicator repetition
//!           absorbed by the renderer
//!   W003 = non-IC dissem in classified banner
//!   E032 = retired in PR 3c.B Commit 6 — SCI sort order absorbed
//!           by the renderer
//!   E033 = retired in PR 3b.F (T026f) — SCI compartment ordering
//!           rolled into E060; E060 retired in PR 3c.B Commit 6
//!   W034 = SCI custom (unpublished) control-system audit visibility
//!   E035 = SCI banner rollup (missing compartments from portions)
//!   E036 = JOINT may not be used with HCS markings (T035b, replaces E017-E019)
//!   E037 = NODIS and EXDIS must not coexist (T035c-21 PR-A)
//!   E038 = NODIS / EXDIS require NOFORN (T035c-21 PR-A)
//!   E039 = REL TO not allowed in banner with NODIS/EXDIS portion (T035c-21 PR-B)
//!   E040 = banner must roll up NODIS (or EXDIS if no NODIS) (T035c-21 PR-B)
//!   E041 = NODIS supersedes EXDIS in portion (T035c-21 PR-B)
//!   S001 = retired in PR 3c.B Commit 6 — banner-abbrev preference
//!           absorbed by the renderer
//!   S002 = retired in PR 3c.B Commit 6 — banner-form consistency
//!           absorbed by the renderer
//!   S003 = JOINT country list should lead with USA (style, follow-up from #97);
//!           dual-pop migration tracked separately
//!   S004 = REL TO trigraph suggest-don't-fix (issue #235 / #186 PR-3)
//!   E052 = retired in PR 3c.B Commit 6 — REL TO duplicates absorbed
//!           by the renderer
//!   E053 = NOFORN conflicts with REL TO (§H.8 p145, declarative wrapper)
//!   E054 = RELIDO conflicts with NOFORN — subtractive fix removes RELIDO (§H.8 p154, declarative wrapper — PR 3b.C)
//!   E055 = RELIDO conflicts with DISPLAY ONLY — subtractive fix removes RELIDO (§H.8 p154, declarative wrapper — PR 3b.C)
//!   E056 = ORCON conflicts with RELIDO — subtractive fix removes RELIDO (§H.8 p136, declarative wrapper — PR 3b.C)
//!   E057 = ORCON-USGOV conflicts with RELIDO — subtractive fix removes RELIDO (§H.8 p140, declarative wrapper — PR 3b.C)
//!   E060 = retired in PR 3c.B Commit 6 — non-canonical-input walker
//!           (5 ordering rows: REL TO USA-first §H.8 p150-151, JOINT
//!           alpha §H.3 p56, AEA SIGMA numeric sort §H.6 p108, SAR
//!           program ascending alpha §H.5 p99, SCI compartment +
//!           sub-compartment numeric-then-alpha §H.4 p61) absorbed by
//!           `MarkingScheme::render_canonical`
//!   S005 = REL TO membership-uncertain reduction — Suggest branch (issue #206)
//!   S006 = REL TO membership-uncertain reduction — Info branch (issue #206)
//!   C001 = corrections-map typo (T058, Phase 5)

use crate::scheme::CapcoScheme;
use marque_ism::generated::migrations::find_migration;
use marque_ism::{
    CanonicalAttrs, MarkingClassification, SciControlSystem, SciMarking, Span, TokenKind,
    TokenSpan, sar_sort_key,
};
use marque_rules::{
    Confidence, Diagnostic, FactRef, FixIntent, FixProposal, FixSource, Message, MessageArgs,
    MessageTemplate, RecanonScope, ReplacementIntent, Rule, RuleContext, RuleId, RuleSet, Severity,
};
use marque_scheme::Scope;
use std::collections::HashSet;

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
        use crate::rules_declarative::{
            DeclarativeAeaNofornRule, DeclarativeBareHcsRule, DeclarativeCominglingWarningRule,
            DeclarativeDualClassificationRule, DeclarativeJointHcsRule, DeclarativeJointRelToRule,
            DeclarativeJointRestrictedRule, DeclarativeNofornRelToConflictRule,
            DeclarativeNonUsMissingDissemRule, DeclarativeOrconRelidoConflictRule,
            DeclarativeOrconUsgovRelidoConflictRule, DeclarativeRdPrecedenceRule,
            DeclarativeRelidoDisplayOnlyConflictRule, DeclarativeRelidoNofornConflictRule,
        };
        Self {
            rules: vec![
                // PR 3c.B Commit 6 (form-bucket migration): the following
                // 13 hand-written rules + the E060 walker were retired
                // because their concerns are absorbed by the renderer
                // (`MarkingScheme::render_canonical`) by construction.
                // After this commit `lint` no longer surfaces these
                // divergences; `fix` (renderer) still produces canonical
                // output. Retired:
                //   E001  PortionMarkInBannerRule       (§H.8 portion-in-banner)
                //   E003  MisorderedBlocksRule          (§A.6 block order)
                //   E004  SeparatorCountRule            (§A.6 / §D.1 separators)
                //   E009  PortionAbbreviationRule       (§H.1 / §H.8 / §H.9 portion forms)
                //   S001  PreferBannerAbbreviationRule  (§A.6 banner-abbrev preference)
                //   S002  BannerConsistentFormRule      (§A.6 banner-form consistency)
                //   E011  MissingNonUsPrefix            (§A.6 / §H.3 non-US `//` prefix)
                //   E013  DelimiterMismatchRule         (§H.3 / §H.8 list delimiters)
                //   E026  SarPortionFormRule            (§H.5 SAR portion form)
                //   E029  SarCompartmentOrderRule       (§H.5 SAR compartment order)
                //   E030  SarIndicatorRepeatRule        (§H.5 SAR indicator repeat)
                //   E032  SciSystemOrderRule            (§H.4 SCI sort order)
                //   E052  RelToNoDuplicatesRule         (§H.8 list dedup)
                //   E060  DeclarativeNonCanonicalInputRule (walker — REL TO/JOINT/SIGMA/SAR/SCI ordering)
                // See `docs/plans/2026-05-10-pr3c-consolidated-plan.md`
                // lines 788–862 for the architectural commitment. E002
                // and S003 stay (separate dual-pop migration); all other
                // unrelated rules are unaffected.
                Box::new(MissingUsaTrigraphRule),
                Box::new(DeclassifyMisplacedRule),
                Box::new(DeprecatedDissemRule),
                Box::new(XShorthandDateRule),
                Box::new(UnknownTokenRule),
                // T035c-14: W001 (DeprecatedMarkingWarningRule) retired.
                // CAPCO-2016 §F "Legacy Control Markings" (p35) treats
                // legacy markings as unauthorized — an error category
                // owned by E006 / E008 — not "deprecated but still legal."
                // §I "Banner Line Syntax History" (p192–193 Table 8) is
                // syntax-history, not token-deprecation guidance, and is
                // non-normative for citations. No CAPCO-2016 passage
                // sanctions a warning-severity "legal but preferred-newer"
                // vocabulary tier, so the rule stub had no authoritative
                // ground to populate. If org-policy deprecations (FOUO-
                // style transitional warnings) later need a home, that is
                // a separate rule with org-config authority, not CAPCO §F.
                Box::new(CorrectionsMapRule),
                // T035a: declarative wrappers for E010/E012/E014-E016/
                // E021/E022/E024/E025/W002. Catalog in `crate::scheme`
                // owns the predicate; wrappers own span/message/fix
                // construction.
                //
                // T035b: E017/E018/E019 retired entirely (over-
                // restrictive per CAPCO §H.3 lines 4140-4146).
                // Replacement: E036 `joint-hcs` (the only specific
                // JOINT exclusion §H.3 p57 actually names).
                Box::new(DeclarativeBareHcsRule),
                Box::new(DeclarativeDualClassificationRule),
                Box::new(DeclarativeCominglingWarningRule),
                Box::new(DeclarativeJointRelToRule),
                Box::new(DeclarativeNonUsMissingDissemRule),
                Box::new(NonIcInClassifiedBannerRule),
                Box::new(DeclarativeJointRestrictedRule),
                Box::new(DeclarativeJointHcsRule),
                Box::new(DeclarativeAeaNofornRule),
                Box::new(DeclarativeRdPrecedenceRule),
                // PR 3c.B Commit 7.3: `DeclarativeClassFloorRule` (rule
                // ID E058) retired. The 27 class-floor catalog rows now
                // fire through the engine's constraint-catalog bridge
                // directly — `class_floor_emit` populates
                // `ConstraintViolation::{span, severity}`, and the
                // bridge folds `E058/<purpose>` and
                // `class-floor/<marking>` row names to
                // `Diagnostic.rule = "E058"` so audit-stream consumers
                // and `[rules] E058 = "off"` config overrides keep
                // working. The 23 family rows
                // (`class-floor/<marking>`) plus the 4 walker-prefixed
                // rows (`E058/CNWDI`, `E058/SAR`, `E058/DOD-UCNI`,
                // `E058/DOE-UCNI`) remain declared as
                // `Constraint::Custom` entries in
                // `CapcoScheme::build_constraints()`. See
                // `specs/006-engine-rule-refactor/decisions/06-commit-7-subdivision.md`
                // for the architectural rationale.
                Box::new(SciCustomControlInfoRule),
                // T035c-21 PR-A: NODIS/EXDIS constraint rules per
                // CAPCO-2016 §H.9. E037 (mutual exclusion) and E038
                // (require NOFORN). Declarative — see
                // `CapcoScheme::constraints()` for the source citation
                // chain.
                Box::new(crate::rules_declarative::DeclarativeNodisConflictsExdisRule),
                Box::new(crate::rules_declarative::DeclarativeDosDissemNofornRule),
                // T035c-21 PR-B: NODIS/EXDIS page-level + portion-level
                // hand-written rules. E039 (REL TO clear), E040
                // (banner roll-up), E041 (NODIS supersedes EXDIS in
                // portion). See §H.9 p172 + p174 for each citation.
                Box::new(NodisExdisClearsBannerRelToRule),
                Box::new(NodisSupersedesExdisInPortionRule),
                // PR 3b Sub-move A — banner-roll-up walker (T026a).
                // Subsumes the three retired literal rules:
                //   E031 SarBannerRollupRule       (§H.5 p101)
                //   E035 SciBannerRollupRule       (§H.4 per-system)
                //   E040 NodisExdisBannerRollupRule (§H.9 p172 + p174)
                // Emitted diagnostics carry per-row IDs (E031 / E035 /
                // E040) for audit-stream continuity.
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
                // diagnostic regardless of confidence.
                Box::new(RelToTrigraphSuggestRule),
                // PR 3b.E (T026e) → PR 3c.B Commit 7.4: retired the
                // 10 hand-written per-SCI-system rules `E042`–`E051`
                // (PR 3b.E walker `DeclarativeSciPerSystemRule`, ID
                // `E059`) into the engine's constraint-catalog bridge.
                // The catalog rows still fire — they emit via
                // `CapcoScheme::bridge_sci_per_system_diagnostics`
                // with `Diagnostic.rule = "E059"` and full
                // `FixProposal` payloads attached (companion-insertion
                // at the dissem-block anchor, ORCON-USGOV → ORCON
                // replacement). The 5 catalog rows
                // (`sci-per-system/{HCS-O,HCS-P-NOFORN,HCS-P-sub,SI-G,
                // TK-compartment-NOFORN}-*`) remain declared as
                // `Constraint::Custom` entries in
                // `CapcoScheme::build_constraints()` for documentation /
                // dispatch parity with class-floor; the bridge takes
                // the inherent-method shortcut. See
                // `specs/006-engine-rule-refactor/decisions/06-commit-7-subdivision.md`.
                // Issue #206 — REL TO membership-uncertain reduction.
                // Two registered rules sharing one analysis helper:
                //   S005 — Suggest, fires when banner is missing a
                //          code atom-semantics says should survive,
                //          OR banner has no REL TO at all.
                //   S006 — Info, fires when banner is consistent
                //          with atom-semantics. Audit-only signal.
                Box::new(RelToOpaqueUncertainReductionSuggestRule),
                Box::new(RelToOpaqueUncertainReductionInfoRule),
                // Issue #256: NOFORN + REL TO mutual exclusion at
                // marking level. §H.8 p145 says NOFORN "Cannot be
                // used with REL TO." Declarative wrapper over the
                // `capco/noforn-conflicts-rel-to` constraint already
                // declared in `CapcoScheme::constraints()`.
                Box::new(DeclarativeNofornRelToConflictRule),
                // PR 3b.C (T026c): RELIDO incompatibility declarative wrappers.
                // Four directly-cited §H.8 conflict pairs from CAPCO-2016:
                //   E054 — RELIDO ⊥ NOFORN        (§H.8 p154)
                //   E055 — RELIDO ⊥ DISPLAY ONLY  (§H.8 p154)
                //   E056 — ORCON  ⊥ RELIDO        (§H.8 p136; assertion on ORCON template)
                //   E057 — ORCON-USGOV ⊥ RELIDO   (§H.8 p140; assertion on ORCON-USGOV template)
                Box::new(DeclarativeRelidoNofornConflictRule),
                Box::new(DeclarativeRelidoDisplayOnlyConflictRule),
                Box::new(DeclarativeOrconRelidoConflictRule),
                Box::new(DeclarativeOrconUsgovRelidoConflictRule),
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

// PR 3b.C (T026c): re-export the four RELIDO incompatibility wrappers
// and the `compute_relido_removal_span` helper for integration tests in
// `crates/capco/tests/`. Both the underlying `pub struct` items and
// these re-exports carry `#[doc(hidden)]`, signaling "technically pub
// for compilation but not stable public API" — the same convention
// `marque_rules::AppliedFix::__engine_promote` uses (per Constitution V
// Principle V test-fixture carve-out). Future refactors are free to
// consolidate or rename these without a breaking-change concern.
#[doc(hidden)]
pub use crate::rules_declarative::{
    DeclarativeOrconRelidoConflictRule, DeclarativeOrconUsgovRelidoConflictRule,
    DeclarativeRelidoDisplayOnlyConflictRule, DeclarativeRelidoNofornConflictRule,
};

#[doc(hidden)]
pub use crate::rules_declarative::compute_relido_removal_span;

#[doc(hidden)]
pub use crate::rules_declarative::find_dissem_token_span;

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

impl Rule<CapcoScheme> for MissingUsaTrigraphRule {
    fn id(&self) -> RuleId {
        RuleId::new("E002")
    }
    fn name(&self) -> &'static str {
        "missing-usa-trigraph"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
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

        let current = attrs
            .rel_to
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        let message = if !has_usa {
            "REL TO list missing required USA trigraph"
        } else {
            "USA must be the first trigraph in REL TO list"
        };
        let citation = "CAPCO-2016 §H.8 (REL TO, p150–151)";

        // Locate the `RelToBlock` this diagnostic refers to. If the
        // marking has more than one REL TO block (e.g.,
        // `SECRET//REL TO GBR//NF//REL TO AUS`), a single first→last
        // splice would delete intervening `//...//` content. In that
        // case we emit a diagnostic with no FixProposal and let the
        // author resolve manually.
        let rel_to_blocks: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToBlock)
            .collect();
        let Some(&block) = rel_to_blocks.first() else {
            // No block tagging (defensive: `attrs.rel_to` non-empty
            // should imply at least one `RelToBlock` token). Emit
            // diagnostic without a fix rather than risk mis-splice.
            return vec![Diagnostic::new(
                self.id(),
                self.default_severity(),
                Span::new(0, 0),
                message.to_owned(),
                citation,
                None,
            )];
        };
        if rel_to_blocks.len() > 1 {
            return vec![Diagnostic::new(
                self.id(),
                self.default_severity(),
                block.span,
                format!(
                    "{message} (multiple REL TO blocks present; fix suppressed to avoid cross-block corruption — resolve manually)"
                ),
                citation,
                None,
            )];
        }

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
                    message.to_owned(),
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
        // inline to preserve single-pass idempotency.
        let mut codes: Vec<marque_ism::CountryCode> = attrs.rel_to.to_vec();
        if !has_usa {
            codes.push(marque_ism::CountryCode::USA);
        }
        // E002 is REL TO only; pass `usa_first: true` per §H.8 p151.
        let canonical_codes = dedup_country_codes(&codes);
        let fixed = canonicalize_trigraph_list(&canonical_codes, true).join(", ");

        // PR 3c.B Commit 6 — dual-population per Path C of the
        // consolidated plan. The byte-precise `FixProposal` carries
        // the narrow REL-TO splice (existing pre-Commit-6 fix; mvp-2
        // audit shape stays byte-stable). The structural `FixIntent`
        // declares the architectural intent:
        //   - USA missing → `FactAdd { USA, Scope::Portion }`
        //     (page-rewrite per audit row E002 — USA injection is a
        //     fact-set addition mandated by §H.8 p151).
        //   - USA not first → `Recanonicalize { Portion }` (per audit
        //     row E002 form sub-shape — the sort is renderer
        //     territory; the renderer absorbs USA-first alpha by
        //     construction at Commit 10's cutover).
        // Commit 10 retires `fix` and renders the intent's
        // replacement bytes from the per-page projection. See
        // `specs/006-engine-rule-refactor/decisions/05-commit-6-prerequisites-audit.md`.
        let fix_proposal = FixProposal::new(
            self.id(),
            FixSource::BuiltinRule,
            span,
            current,
            fixed,
            Confidence::strict(0.97), // per spec T031
            None,
        );
        // Scope follows the marking-type context: a portion-level
        // REL-TO divergence recanonicalizes at portion scope; banner
        // and CAB diverged REL-TO lists recanonicalize at page scope
        // (banner roll-up). The renderer at Commit 10 will materialize
        // the correct byte span from the projection.
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
            }
        } else {
            FixIntent {
                replacement: ReplacementIntent::Recanonicalize {
                    scope: intent_scope_recanon,
                },
                confidence: Confidence::strict(0.97),
                feature_ids: Default::default(),
                message: Message::new(MessageTemplate::NonCanonicalOrder, MessageArgs::default()),
            }
        };
        vec![Diagnostic::with_fix_and_intent(
            self.id(),
            self.default_severity(),
            span,
            message.to_owned(),
            citation,
            fix_proposal,
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
// `Diagnostic::with_fix_intent(..., None)` constructor: this rule emits
// neither a legacy `FixProposal` nor a structural `FixIntent<S>` because
// the repair is multi-span document-level rewriting (move the declass
// token from banner/portion into a CAB). The constructor swap (vs the
// `Diagnostic::new(..., None)` form) signals consciously-decided deferred
// migration evaluation, matching the PR #349 pattern for E016/E036.
// Downstream audit consumers observe no behavioral difference: both
// constructors leave `fix: None` and `fix_intent: None`.
struct DeclassifyMisplacedRule;

impl Rule<CapcoScheme> for DeclassifyMisplacedRule {
    fn id(&self) -> RuleId {
        RuleId::new("E005")
    }
    fn name(&self) -> &'static str {
        "declassify-misplaced"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
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
        vec![Diagnostic::with_fix_intent(
            self.id(),
            self.default_severity(),
            span,
            "declassification marking belongs on the Declassify On line of \
             the Classification Authority Block, not in a banner or portion \
             — remove the declass token here and add it to the CAB",
            "CAPCO-2016 §E.1 p31 (Declassify On is a CAB line) + \
             §D.1 p27 (banner categories do not include declassification)",
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

impl Rule<CapcoScheme> for DeprecatedDissemRule {
    fn id(&self) -> RuleId {
        RuleId::new("E006")
    }
    fn name(&self) -> &'static str {
        "deprecated-dissem"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
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
            // Portion-form abbreviations (NF, OC, IMC, DSEN, PR) are NOT
            // deprecations — they are the canonical portion form and the
            // banner expansion is owned by E001. Skip them at every layer.
            if is_abbreviation_expansion(token.text.as_ref(), entry.replacement) {
                continue;
            }
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::MigrationTable,
                span: token.span,
                message: format!(
                    "{:?} is a deprecated dissemination control; replace with {:?}",
                    token.text, entry.replacement
                ),
                citation: "CAPCO-2016 §F",
                original: token.text.to_string(),
                replacement: entry.replacement.to_owned(),
                confidence: entry.confidence,
                migration_ref: Some(entry.reference),
            }));
        }
        diagnostics
    }
}

/// Returns `true` if `from`→`to` is a portion-form abbreviation expansion
/// owned by E001 (so E006 should not double-fire on the same span).
fn is_abbreviation_expansion(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("NF", "NOFORN")
            | ("OC", "ORCON")
            | ("IMC", "IMCON")
            | ("DSEN", "DEA SENSITIVE")
            | ("PR", "PROPIN")
    )
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

impl Rule<CapcoScheme> for XShorthandDateRule {
    fn id(&self) -> RuleId {
        RuleId::new("E007")
    }
    fn name(&self) -> &'static str {
        "x-shorthand-date"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
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
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::MigrationTable,
                    span: token.span,
                    message: format!(
                        "X-shorthand declassification code {text:?} is deprecated; \
                         use {:?}",
                        entry.replacement
                    ),
                    citation: "CAPCO-2016 §E.6",
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
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::MigrationTable,
                    span: token.span,
                    message: format!(
                        "X-shorthand declassification code {text:?} is deprecated; \
                         use {replacement:?}"
                    ),
                    citation: "CAPCO-2016 §E.6",
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

impl Rule<CapcoScheme> for UnknownTokenRule {
    fn id(&self) -> RuleId {
        RuleId::new("E008")
    }
    fn name(&self) -> &'static str {
        "unrecognized-token"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
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
                find_migration(text).is_none()
                    && !looks_like_deprecated_x_shorthand(text)
                    && !is_repeated_sar_owned_by_e030(text, has_first_sar)
            })
            .map(|t| {
                Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    t.span,
                    "unrecognized token inside marking — does not match any \
                     known CAPCO classification, control, or trigraph",
                    "CAPCO-2016 §G.1 (Register of Authorized Markings, p36)",
                    None, // FR-012: no fix offered
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// W001 retired in T035c-14. See registration-site comment in
// `CapcoRuleSet::new()` for the §F / §I rationale.
// ---------------------------------------------------------------------------

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

impl Rule<CapcoScheme> for CorrectionsMapRule {
    fn id(&self) -> RuleId {
        RuleId::new("C001")
    }
    fn name(&self) -> &'static str {
        "corrections-map"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
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
            let text = token_span.text.as_ref();
            let Some(replacement) = corrections.get(text) else {
                continue;
            };
            // M2: skip no-op corrections (replacement == original)
            if replacement == text {
                continue;
            }
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::CorrectionsMap,
                span: token_span.span,
                message: format!("corrections map: {text:?} → {replacement:?}"),
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
/// document text. The pre-existing legacy channel
/// `FixProposal.original = classification_text.to_owned()` does
/// carry source bytes, but per Path C of the consolidated plan that
/// is the known mvp-2 audit pre-existing channel scheduled for
/// retirement at Commit 10 — outside this rule's scope. The new
/// `Diagnostic.fix_intent` payload is content-ignorant: it carries
/// only `Recanonicalize { RecanonScope::Page }`, a `Confidence`
/// scalar, and a `Message::new(MessageTemplate::NonCanonicalOrder,
/// MessageArgs::default())` template-only carrier with no input
/// bytes. The G13 envelope walker in
/// `crates/capco/tests/g13_closure_fix_intent.rs` pins this
/// invariant structurally.
struct JointUsaFirstRule;

impl Rule<CapcoScheme> for JointUsaFirstRule {
    fn id(&self) -> RuleId {
        RuleId::new("S003")
    }
    fn name(&self) -> &'static str {
        "joint-usa-first"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
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

        // JOINT span covers the full `Classification` token. Preserve
        // the `JOINT <level>` prefix by anchoring on the first
        // source-order country's position in the token text. (The
        // pre-PR-3c.B JOINT canonicalization helper used the same
        // trick — retired alongside E020 / E060.)
        let Some(classification_tok) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
        else {
            return vec![];
        };
        let classification_text = classification_tok.text.as_ref();
        let actual_first = j.countries[0].as_str();
        let prefix_end = classification_text
            .find(actual_first)
            .unwrap_or(classification_text.len());
        let prefix = &classification_text[..prefix_end];

        let joined_actual: Vec<&str> = j.countries.iter().map(|t| t.as_str()).collect();
        let joined_actual_str = joined_actual.join(" ");
        let joined_canonical_str = canonical.join(" ");
        let replacement = format!("{prefix}{joined_canonical_str}");

        let message = format!(
            "JOINT country list does not lead with USA: [{joined_actual_str}] \
             → [{joined_canonical_str}] (IC convention — §H.3 prescribes \
             pure alphabetical but every other US-authored country list \
             leads with USA; style rule, disable via S003 = \"off\")"
        );

        // PR 3c.B Commit 6 — dual-population per Path C. The
        // structural `FixIntent` declares `Recanonicalize { Page }`:
        // JOINT classification rendering is a page-scope concern (the
        // banner-line classification axis), and the convention is
        // layered above the renderer's §H.3 pure-alpha default
        // (Commit 10 will gate the convention via config rather than
        // re-render at fix-emit time, so the intent stays meaningful
        // across the cutover).
        let citation = concat!(
            "IC convention (not CAPCO mandate) — §H.3 p56 ",
            "prescribes pure alphabetical for JOINT with no USA-first ",
            "carve-out; S003 encodes the convention observed in REL TO ",
            "§H.8 pp 150–151 across all US-authored country ",
            "lists. Style rule; configure S003 = \"off\" for strict ",
            "§H.3 conformance.",
        );
        let fix_proposal = FixProposal::new(
            self.id(),
            FixSource::BuiltinRule,
            classification_tok.span,
            classification_text.to_owned(),
            replacement,
            Confidence::strict(1.0),
            None,
        );
        let fix_intent = FixIntent {
            replacement: ReplacementIntent::Recanonicalize {
                scope: RecanonScope::Page,
            },
            confidence: Confidence::strict(1.0),
            feature_ids: Default::default(),
            message: Message::new(MessageTemplate::NonCanonicalOrder, MessageArgs::default()),
        };
        vec![Diagnostic::with_fix_and_intent(
            self.id(),
            self.default_severity(),
            classification_tok.span,
            message,
            citation,
            fix_proposal,
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

impl Rule<CapcoScheme> for RelToTrigraphSuggestRule {
    fn id(&self) -> RuleId {
        RuleId::new("S004")
    }
    fn name(&self) -> &'static str {
        "rel-to-trigraph-suggest"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
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

            let proposal = FixProposal::new(
                self.id(),
                FixSource::BuiltinRule,
                span,
                trigraph.to_owned(),
                candidate.to_owned(),
                marque_rules::Confidence::strict(SUGGEST_CONFIDENCE),
                None,
            );
            diagnostics.push(Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                message,
                "CAPCO-2016 §H.8 p150–151",
                Some(proposal),
            ));
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

impl Rule<CapcoScheme> for NonIcInClassifiedBannerRule {
    fn id(&self) -> RuleId {
        RuleId::new("W003")
    }
    fn name(&self) -> &'static str {
        "non-ic-dissem-in-classified-banner"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
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

            diagnostics.push(Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                format!(
                    "non-IC dissem control {} should not appear in a classified banner; \
                     use only in portion markings",
                    nic.banner_str(),
                ),
                "CAPCO-2016 §H.9",
                None,
            ));
        }

        diagnostics
    }
}

/// Canonicalize a country code list. The `usa_first` flag selects the
/// convention:
///
/// - `usa_first = true` — REL TO convention per CAPCO-2016 §H.8 p151:
///   "After 'USA', list the required one or more trigraph country
///   codes in alphabetical order followed by tetragraph codes listed
///   in alphabetical order." USA is elevated to the front when
///   present; remaining codes are alphabetical.
/// - `usa_first = false` — JOINT convention per CAPCO-2016 §H.3 p56:
///   "Country trigraph codes are listed alphabetically followed
///   by tetragraph codes in alphabetical order." Pure alphabetical;
///   USA is NOT elevated.
///
/// Duplicates in the input are preserved as-is — this helper does
/// not deduplicate. Callers that need a fully canonical list (USA-
/// first + alphabetical + unique) compose [`dedup_country_codes`]
/// before this canonicalizer:
///
/// ```text
/// canonicalize_trigraph_list(&dedup_country_codes(codes), usa_first)
/// ```
///
/// E002 (REL TO, fix path) uses that composition so its fix
/// replacement is byte-canonical and stays single-pass idempotent.
/// Pre-PR-3c.B the same composition served E020 (and its
/// successor walker E060) — both retired into the renderer's REL TO
/// axis (`render_rel_to.rs`) at PR 3c.B Commit 6; E052
/// (no-duplicates) likewise retired at the same commit.
///
/// The IC practice of rendering USA first in JOINT lists is widespread
/// but is convention, not CAPCO rule. A style rule (S003
/// `joint-usa-first`) to flag deviations is a planned follow-up; this
/// helper does NOT encode the convention into correctness.
///
/// This is the shared ordering rule for E002 (REL TO, fix path) —
/// pre-PR-3c.B it was also shared with E020 / E060 (now retired) and
/// composed alongside E052 (also retired) for the no-duplicates
/// path. Today it gives E002 a single source of truth for the
/// USA-first + alphabetical invariant cited in §H.8 p151, mirroring
/// what the renderer's REL TO axis (`render_rel_to.rs`) produces at
/// fix time. The duplication is intentional during the dual-pop
/// transition window (Commits 6–9); Commit 10 retires the rule-side
/// helper and routes E002 / S003 through `render_canonical` at
/// audit-promote time.
///
/// Visibility is `pub(crate)`: the decoder text-level path in
/// `marque-engine` does not call this helper directly — it operates
/// pre-strict-parse on raw text — and no other crate currently needs
/// it. Should a future consumer (e.g., a downstream formatter or a
/// programmatic API) need to canonicalize a `&[CountryCode]` list, it
/// should call through `marque-capco`'s public surface or this helper
/// can be promoted to `pub` at that point with an honest rationale.
///
/// Tetragraph partition handling is deferred — issue #183 PR-A
/// widened `CountryCode` so 4-byte tetragraphs are now first-class
/// entries in `attrs.rel_to`, but this helper still sorts the whole
/// list flat-alphabetically rather than the §H.3 p56 / §H.8 p151
/// "trigraphs alpha, then tetragraphs alpha" partition. Follow-up:
/// bucket true trigraphs (`code.len() == 3`) before everything else
/// (the 2-byte `EU`, the 4-byte tetragraphs, and 15-byte
/// `AUSTRALIA_GROUP` go in the non-trigraph bucket), or ideally
/// derive the buckets from the CVE schema groups in
/// `CVEnumISMCATRelTo.xsd`.
pub(crate) fn canonicalize_trigraph_list(
    codes: &[marque_ism::CountryCode],
    usa_first: bool,
) -> Vec<&str> {
    if usa_first {
        let has_usa = codes.contains(&marque_ism::CountryCode::USA);
        let mut sorted: Vec<&str> = codes
            .iter()
            .filter(|t| **t != marque_ism::CountryCode::USA)
            .map(|t| t.as_str())
            .collect();
        sorted.sort_unstable();
        if has_usa {
            sorted.insert(0, "USA");
        }
        sorted
    } else {
        let mut sorted: Vec<&str> = codes.iter().map(|t| t.as_str()).collect();
        sorted.sort_unstable();
        sorted
    }
}

/// Collapse duplicate country codes while preserving first-occurrence
/// order. Composed with [`canonicalize_trigraph_list`] inside E002's
/// fix path so its replacement is byte-canonical (USA-first +
/// alphabetical + unique).
///
/// Pre-PR-3c.B this helper was also the source-of-truth for E052
/// (REL TO no-duplicates fix path) and was composed with E020 / E060
/// for the JOINT + REL TO ordering paths — all three rules retired
/// in PR 3c.B Commit 6 into the renderer's REL TO axis
/// (`render_rel_to.rs`). E002 keeps the in-rule composition to stay
/// dual-pop with mvp-2 audit byte-stability through Commit 9;
/// Commit 10 routes E002 / S003 through `render_canonical` at
/// audit-promote time and the helpers retire.
///
/// **CAPCO authority**: §H.8 p151 specifies the REL TO list grammar
/// as "After 'USA', list the required one or more trigraph country
/// codes in alphabetical order followed by tetragraph codes listed
/// in alphabetical order." There is no textual prohibition of
/// duplicates — the rationale is structural: a list of country codes
/// describing a release decision is a set, and a duplicate is
/// redundant by construction. Mirrors the rationale block in
/// `try_rel_to_fuzzy_trigraph_candidates` (decoder side, issue #233)
/// for why duplicate-creating fuzzy candidates are filtered.
pub(crate) fn dedup_country_codes(
    codes: &[marque_ism::CountryCode],
) -> Vec<marque_ism::CountryCode> {
    let mut seen: HashSet<marque_ism::CountryCode> = HashSet::with_capacity(codes.len());
    let mut out: Vec<marque_ism::CountryCode> = Vec::with_capacity(codes.len());
    for &code in codes.iter() {
        if seen.insert(code) {
            out.push(code);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Rules: S005 + S006 — REL TO membership-uncertain reduction (issue #206)
// ---------------------------------------------------------------------------
//
// Conceptually one diagnostic with a context-dependent severity (Info
// when the banner is consistent with atom-semantics; Suggest when not),
// per plan §3.1. Implementation-wise two registered rules because
// `marque_engine::Engine::lint` overwrites every emitted diagnostic's
// severity with the rule's configured/default severity (engine.rs
// `// Apply configured severity override`); a single rule cannot
// stably emit at two different severities. Both rules share
// `analyze_uncertain_reduction` and split only on which branch they
// keep.
//
// Naming:
//   S005 `rel-to-opaque-uncertain-reduction` — Suggest. Active
//        validation (banner has no REL TO) or banner REL TO drops
//        a code atom-semantics says should survive.
//   S006 `rel-to-opaque-uncertain-reduction-info` — Info. Banner
//        REL TO is consistent with atom-semantics
//        (`expected ⊆ banner`). The producer plausibly drew on
//        membership data we don't have; surface the uncertainty
//        without flagging.
//
// Both rules share:
//   - the trigger condition (uncertain code dropped + non-empty
//     other-codes set)
//   - the diagnostic message body (state text from
//     `marque_ism::lookup_tetragraph_provenance`, atom intersection,
//     membership-hypothesis candidates)
//   - the `fix: None` shape (ambiguity not resolvable from in-tree
//     data)
//   - audit-content-ignorance per Constitution V (canonical tokens
//     plus verbatim ODNI taxonomy `<Description>` text only)

/// S005 (Suggest): REL TO membership-uncertain reduction — primary
/// branch. Fires when an `is_decomposable == None` tetragraph drops
/// out of the page-level atom-semantics intersection AND either
///
/// - the banner has no REL TO list (active-validation context — we'd
///   be computing the marking from scratch), or
/// - the banner's REL TO list (post tetragraph expansion) is missing
///   a code atom-semantics says should survive (`expected ⊄ banner`).
///
/// See the module-level S005/S006 header for the rationale on the
/// two-rule split. Severity: `Suggest` (engine-overridable).
///
/// # Authority
///
/// CAPCO-2016 §H.8 (REL TO list grammar) + ODNI ISMCAT
/// V[`marque_ism::ISMCAT_TETRA_VERSION`] Tetragraph Taxonomy. Atom-
/// semantics is the lowest-risk default (drop the code), but when
/// the code is uncertain the default is not authoritatively grounded
/// — the dropped code might genuinely include the atoms the
/// intersection just lost. The "consistent" comparison runs in
/// post-expansion atom space, matching the rollup XSL's
/// `util:expandDecomposableTetras` semantics
/// (`Schematron/ISM_XML-ROLLUP-phase.xsl`, plan §8 Q3).
///
/// # Fix
///
/// **None.** The rule cannot resolve the ambiguity from in-tree
/// data — the dropped uncertain tetragraph may genuinely include
/// the atoms the intersection just lost; only the producer's
/// external membership data can settle the question. Note that
/// `Engine::fix_inner` excludes `Severity::Suggest` only (see the
/// `d.severity != Severity::Suggest` filter at engine.rs ~L1378),
/// so emitting a `FixProposal` here WOULD risk auto-apply at
/// engine-overridable Info/Warn/Error severities. A no-fix
/// diagnostic is the safer shape.
// ---------------------------------------------------------------------------
// Migration status (PR 3c.B Sub-PR 9, 2026-05-11): provisional Path A
// per `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md` D4.
// S005 (and its sister rule S006 below) stay as hand-written `Rule` impls
// in this file; neither migrates to a `Constraint::Custom` catalog row on
// `CapcoScheme` in this PR.
//
// Retirement target: admonition emitter channel (deferred per
// `specs/006-engine-rule-refactor/followups/admonition-channel.md`). The
// Suggest/Info severity split between S005 and S006 is operational, NOT
// §-grounded — CAPCO-2016 §H.8 treats REL TO via pure set-membership
// language, and §D.2 Table 3 rule 21 (the roll-up intersection law)
// applies uniformly without distinguishing "active validation" from
// "consistent case." The split exists because
// `marque_engine::Engine::lint` overwrites every emitted diagnostic's
// severity with the rule's configured/default severity (see engine.rs
// `// Apply configured severity override`); a single rule cannot stably
// emit at two severities. The admonition channel — when built — collapses
// the two registered rules into one signal with per-emission severity.
//
// Authority for the underlying invariant: CAPCO-2016 §H.8 (REL TO list
// grammar — syntax and tetragraph definition) + ODNI ISMCAT Tetragraph
// Taxonomy (member-country expansion). The ISMCAT taxonomy is the
// authoritative member-country source per ODNI; §H.8 itself does not
// delegate to ISMCAT (the string "ISMCAT" does not appear in
// `crates/capco/docs/CAPCO-2016.md`). The two authorities compose; they
// are not in a delegating relationship. The `S005_CITATION` const below
// uses an additive `+` form for historical continuity; readers should
// interpret it as "§H.8 (grammar) plus ISMCAT (expansion data)", not as
// "§H.8 delegating to ISMCAT."
//
// Citations explicitly NOT load-bearing for S005/S006:
//   - §D.2 Table 3 rule 23 (TEYE/ACGU/FVEY-only intersection special
//     case) — strictly outside S005/S006's general-tetragraph case.
//   - §H.8 p151 ("Commingling Rule(s) Within a Portion" — per-portion,
//     not page-level roll-up).
// Reviewers verifying citation chains for S005/S006 should not follow
// either of these as authority for the rules' behavior.
//
// Structural blocker (why Path A in PR 3c.B Commit 9):
// `MarkingScheme::evaluate_custom` (crates/scheme/src/scheme.rs:124-130)
// receives only `&Self::Marking`. It has no access to
// `RuleContext.page_context`. The entire body of
// `analyze_uncertain_reduction` below is page-context-dependent — it
// reads `page.portions()`, computes a page-level atom-semantics
// intersection across all portions carrying REL TO, and decides the
// Suggest vs Info branch from page-wide banner state. A constraint-
// catalog predicate cannot reproduce any of this without context access.
// The trait-surface extension that would unblock this migration is
// tracked in
// `specs/006-engine-rule-refactor/followups/constraint-context-extension.md`.
//
// `Diagnostic::with_fix_intent(..., None)` constructor (in the `check`
// bodies below): this rule emits neither a legacy `FixProposal` nor a
// structural `FixIntent<S>` because the ambiguity is not resolvable from
// in-tree data — the dropped uncertain tetragraph may genuinely include
// the atoms the intersection lost; only the producer's external
// membership data can settle the question. The constructor swap (vs
// `Diagnostic::new(..., None)`) signals consciously-decided deferred
// migration evaluation, matching the PR #349 pattern for E016/E036.
// Downstream audit consumers observe no behavioral difference.
struct RelToOpaqueUncertainReductionSuggestRule;

/// S006 (Info): REL TO membership-uncertain reduction — companion
/// branch covering the banner-consistent case. Fires under the same
/// trigger as S005 (uncertain code dropped + non-empty other-codes
/// set), but only when the banner's REL TO list (post tetragraph
/// expansion) is consistent with atom-semantics (`expected ⊆
/// banner_atomic`). The producer plausibly drew on membership data
/// we don't have; surface the uncertainty for audit visibility
/// without raising it to Suggest.
///
/// Severity: `Info` (engine-overridable). The banner-consistent
/// case has high false-positive cost if surfaced as Suggest — the
/// operator's marking is correct under the safe default — so plan
/// §3.1 distinguishes it as audit-only signal.
///
/// See the S005 doc above for authority and the module-level
/// S005/S006 header for why the split is two registered rules.
// Migration status (PR 3c.B Sub-PR 9, 2026-05-11): provisional Path A,
// same retirement target and structural blocker as S005. See the
// migration-status comment block above S005's struct decl
// (`RelToOpaqueUncertainReductionSuggestRule`) for the full rationale,
// the admonition-channel retirement target, citation guidance, and the
// `evaluate_custom` context-access blocker.
struct RelToOpaqueUncertainReductionInfoRule;

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

/// Which severity branch a candidate diagnostic belongs to. The
/// branch is determined per-page (banner state is page-wide, not
/// per-uncertain-code) and is identical for every diagnostic emitted
/// from a single `analyze_uncertain_reduction` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S005Branch {
    /// Active validation (banner has no REL TO) OR banner is
    /// inconsistent (`expected ⊄ banner_atomic`). S005 fires.
    Suggest,
    /// Banner is consistent with atom-semantics
    /// (`expected ⊆ banner_atomic`). S006 fires.
    Info,
}

/// One candidate diagnostic produced by the shared analysis. Both
/// S005 and S006 build their final `Diagnostic` from these, filtering
/// by branch.
struct S005Candidate {
    branch: S005Branch,
    span: Span,
    message: String,
}

/// Run the full S005/S006 trigger analysis and return the candidate
/// diagnostics tagged with the severity branch they belong to. Both
/// rule wrappers call this; each wrapper filters by branch.
///
/// The analysis runs once per banner/CAB candidate per registered
/// rule (so twice total under the current registration). The cost is
/// bounded by the number of portions with non-empty REL TO and the
/// number of uncertain codes across them — a handful of operations
/// over BTreeSets in practice. Sharing the helper keeps S005 and
/// S006 from drifting on the trigger condition or the message body.
fn analyze_uncertain_reduction(attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<S005Candidate> {
    use marque_ism::{MarkingType, is_decomposable};

    if !matches!(ctx.marking_type, MarkingType::Banner | MarkingType::Cab) {
        return Vec::new();
    }

    let Some(page) = ctx.page_context.as_ref() else {
        return Vec::new();
    };

    // Plan §3.2 requires "at least two portions carrying a
    // non-empty REL TO list." Anything less and there's no
    // intersection to compute.
    let portions_with_rel_to: Vec<&CanonicalAttrs> = page
        .portions()
        .iter()
        .filter(|p| !p.rel_to.is_empty())
        .collect();
    if portions_with_rel_to.len() < 2 {
        return Vec::new();
    }

    // NOFORN supersedes REL TO at the page level (CAPCO-2016
    // §H.8 + §H.9 — NOFORN/REL TO mutual exclusion). When any
    // portion carries NOFORN, or when the non-IC SBU-NF/LES-NF
    // split forces NF injection at banner roll-up,
    // `PageContext::expected_rel_to` returns empty *because the
    // marking is superseded*, not because the atom intersection
    // is empty. Firing S005/S006 in that case produces a
    // misleading "intersection produced REL TO (empty)"
    // diagnostic — the operator's actual problem is "you have
    // NOFORN AND REL TO portions on the same page", which is a
    // different rule's territory. Bail so S005/S006 only run
    // when REL TO is semantically in play. Mirrors the
    // supersession checks `PageContext::expected_rel_to` runs
    // internally; we duplicate them here because the rule needs
    // to distinguish "empty due to supersession" from "empty
    // due to genuinely-disjoint portion REL TO lists" (the
    // latter is a legitimate S005/S006 trigger). (Caught by
    // Copilot review on PR #249.)
    let any_portion_noforn = page.portions().iter().any(|p| {
        p.dissem_controls
            .iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf))
    });
    if any_portion_noforn {
        return Vec::new();
    }
    let (_expected_non_ic, needs_nf_from_split) = page.expected_non_ic_dissem();
    if needs_nf_from_split {
        return Vec::new();
    }

    // The atom-semantics intersection. `PageContext::expected_rel_to`
    // already does tetragraph expansion before intersection and
    // returns the result USA-first then alphabetical (per CAPCO
    // §H.8). We project to a string set for set-algebra.
    let expected = page.expected_rel_to();
    let expected_set: std::collections::BTreeSet<&str> =
        expected.iter().map(|c| c.as_str()).collect();

    // Banner's REL TO, if present. `attrs.rel_to.is_empty()`
    // distinguishes "banner doesn't carry an REL TO at all" (active
    // validation context — Suggest) from "banner has an REL TO list"
    // (consistency check decides Info vs Suggest). Expansion runs in
    // atom space, matching the rollup XSL's
    // `util:expandDecomposableTetras` semantics
    // (Schematron/ISM_XML-ROLLUP-phase.xsl, plan §8 Q3).
    let banner_atomic: Option<std::collections::BTreeSet<&str>> = if attrs.rel_to.is_empty() {
        None
    } else {
        Some(s005_expand_atomic(&attrs.rel_to))
    };

    // Branch is page-wide (banner state, not per-uncertain-code).
    // The "consistent" comparison is `expected ⊆ banner_atomic` —
    // the banner may legitimately carry MORE codes than
    // atom-semantics produced (operator drew on external membership
    // data), but it must not drop a code atom-semantics says should
    // survive.
    let branch = match &banner_atomic {
        None => S005Branch::Suggest,
        Some(b) if expected_set.is_subset(b) => S005Branch::Info,
        Some(_) => S005Branch::Suggest,
    };

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

    // Pick the diagnostic span: prefer the banner's RelToBlock if
    // present, fall back to the first banner token. Pointing at the
    // RelToBlock makes the diagnostic land where the operator can
    // act on it; the first-token fallback covers the
    // banner-without-REL TO active-validation case. Banner has at
    // least one token (the candidate parsed successfully) so the
    // `Span::new(0, 0)` fallback is purely defensive.
    let span = attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::RelToBlock)
        .or_else(|| attrs.token_spans.first())
        .map(|t| t.span)
        .unwrap_or(Span::new(0, 0));

    let mut candidates = Vec::new();
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

        let message = format!(
            "REL TO code `{x}` has uncertain membership ({state}). \
             Atom-semantics intersection produced REL TO {expected_str}, \
             but `{x}`'s hypothetical membership may include {other_str} \
             from other portions. Resolution: (a) add `{x}` membership \
             to country_extensions.toml with an authoritative source \
             citation, or (b) revise the marking to use codes with \
             known membership."
        );

        candidates.push(S005Candidate {
            branch,
            span,
            message,
        });
    }
    candidates
}

/// Citation shared by S005 and S006. Stays static (not formatted with
/// `ISMCAT_TETRA_VERSION`) because `Diagnostic::citation` is
/// `&'static str`. The version reference is in the state text inside
/// the message body, which is dynamically formatted via
/// `s005_state_text`.
const S005_CITATION: &str =
    "CAPCO-2016 §H.8 + ODNI ISMCAT Tetragraph Taxonomy (see ISMCAT_TETRA_VERSION)";

impl Rule<CapcoScheme> for RelToOpaqueUncertainReductionSuggestRule {
    fn id(&self) -> RuleId {
        RuleId::new("S005")
    }
    fn name(&self) -> &'static str {
        "rel-to-opaque-uncertain-reduction"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
    }

    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        analyze_uncertain_reduction(attrs, ctx)
            .into_iter()
            .filter(|c| c.branch == S005Branch::Suggest)
            .map(|c| {
                // PR 3c.B Sub-PR 9: provisional Path A — `with_fix_intent(..., None)`
                // signals consciously-decided deferred migration evaluation. See the
                // migration-status block above `struct RelToOpaqueUncertainReductionSuggestRule;`
                // for the full rationale and admonition-channel retirement target.
                Diagnostic::with_fix_intent(
                    self.id(),
                    self.default_severity(),
                    c.span,
                    c.message,
                    S005_CITATION,
                    None,
                )
            })
            .collect()
    }
}

impl Rule<CapcoScheme> for RelToOpaqueUncertainReductionInfoRule {
    fn id(&self) -> RuleId {
        RuleId::new("S006")
    }
    fn name(&self) -> &'static str {
        "rel-to-opaque-uncertain-reduction-info"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        analyze_uncertain_reduction(attrs, ctx)
            .into_iter()
            .filter(|c| c.branch == S005Branch::Info)
            .map(|c| {
                // PR 3c.B Sub-PR 9: provisional Path A — `with_fix_intent(..., None)`
                // signals consciously-decided deferred migration evaluation. See the
                // migration-status block above `struct RelToOpaqueUncertainReductionSuggestRule;`
                // for the full rationale and admonition-channel retirement target.
                Diagnostic::with_fix_intent(
                    self.id(),
                    self.default_severity(),
                    c.span,
                    c.message,
                    S005_CITATION,
                    None,
                )
            })
            .collect()
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
        Some(obs) => obs.programs.iter().map(|p| p.identifier.as_ref()).collect(),
        None => HashSet::new(),
    };

    expected
        .programs
        .iter()
        .filter(|p| !observed_ids.contains(p.identifier.as_ref()))
        .map(|p| p.identifier.as_ref())
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

impl Rule<CapcoScheme> for SciCustomControlInfoRule {
    fn id(&self) -> RuleId {
        RuleId::new("W034")
    }
    fn name(&self) -> &'static str {
        "sci-custom-control-info"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
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
                let span = sys_spans
                    .get(idx)
                    .map(|t| t.span)
                    .unwrap_or(Span::new(0, 0));
                out.push(Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    span,
                    format!(
                        "unpublished SCI control system {:?} present; verify agency \
                         allocation via ODNI/P&S registry",
                        text.as_ref()
                    ),
                    "CAPCO-2016 §A.6 p16; §H.4 p61",
                    None,
                ));
            }
        }
        out
    }
}

/// Citation string for E035 — shared between the with-fix and no-fix
/// emission paths so they cannot silently diverge. References the
/// per-system "Precedence Rules for Banner Line Guidance" template
/// that appears in every §H.4 entry (HCS p62 is one of 18 identical
/// instances).
///
/// Per T026a D13 single-citation discipline, this string carries the
/// **operative** banner-roll-up rule for SCI only — §H.4 per-system
/// precedence. §D.2 p28 (CAPCO-2016 lines 577–579) restates the same
/// banner/portion consistency invariant in general-algorithm prose;
/// the spec wording in
/// `specs/006-engine-rule-refactor/tasks.md` T026a explicitly directs
/// background §-references to row documentation rather than the
/// citation string ("§D.2 is general-algorithm prose (per-category
/// citations are tighter and verifiable per Constitution VIII)").
/// The §D.2 background pointer therefore lives on the SCI evaluator's
/// doc comment, not here.
const E035_CITATION: &str = concat!(
    "CAPCO-2016 §H.4 per-system \"Precedence Rules for Banner Line ",
    "Guidance\" (e.g. HCS p62, SI p74, TK p85). All unique SCI ",
    "markings in portions must appear in the banner line; unlike ",
    "SAR, SCI has no hierarchy-optional carve-out.",
);

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// SCI rule helpers
// ---------------------------------------------------------------------------

/// Returns the text form of a SciControlSystem for sort/display purposes.
fn sci_system_text(system: &SciControlSystem) -> &str {
    match system {
        SciControlSystem::Published(bare) => bare.as_str(),
        SciControlSystem::Custom(text) => text.as_ref(),
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

/// Thin wrapper around `PageContext::expected_sci_markings()` that returns
/// a `Vec<SciMarking>` for E035's internal use. P4 landed the inherent
/// method returning `Box<[SciMarking]>`; this helper normalizes to `Vec`.
fn page_expected_sci_markings(page: &marque_ism::PageContext) -> Vec<SciMarking> {
    page.expected_sci_markings().into_vec()
}

// Helpers
// ---------------------------------------------------------------------------

/// Compute the byte span covering the full SAR block: from the start of
/// its `SarIndicator` token through the end of the last SAR-constituent
/// token (`SarProgram` / `SarCompartment` / `SarSubCompartment`).
pub(crate) fn sar_block_span(attrs: &CanonicalAttrs) -> Option<Span> {
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;
    for tok in attrs.token_spans.iter() {
        let is_sar = matches!(
            tok.kind,
            TokenKind::SarIndicator
                | TokenKind::SarProgram
                | TokenKind::SarCompartment
                | TokenKind::SarSubCompartment
        );
        if !is_sar {
            continue;
        }
        if tok.kind == TokenKind::SarIndicator && start.is_none() {
            start = Some(tok.span.start);
        }
        let new_end = tok.span.end;
        end = Some(end.map_or(new_end, |e| e.max(new_end)));
    }
    match (start, end) {
        (Some(s), Some(e)) if e >= s => Some(Span::new(s, e)),
        _ => None,
    }
}

/// Bundle of all the inputs `make_fix_diagnostic` needs. Replaces a 9-arg
/// positional helper signature so call sites read top-down by name.
pub(crate) struct FixDiagnosticParams {
    pub rule: RuleId,
    pub severity: Severity,
    pub source: FixSource,
    pub span: Span,
    pub message: String,
    pub citation: &'static str,
    pub original: String,
    pub replacement: String,
    pub confidence: f32,
    pub migration_ref: Option<&'static str>,
}

pub(crate) fn make_fix_diagnostic(p: FixDiagnosticParams) -> Diagnostic<CapcoScheme> {
    let proposal = FixProposal::new(
        p.rule.clone(),
        p.source,
        p.span,
        p.original,
        p.replacement,
        marque_rules::Confidence::strict(p.confidence),
        p.migration_ref,
    );
    Diagnostic::new(
        p.rule,
        p.severity,
        p.span,
        p.message,
        p.citation,
        Some(proposal),
    )
}

// ===========================================================================
// T035c-21 PR-B: NODIS / EXDIS page-level + portion-level rules (§H.9)
// ===========================================================================
//
// Three hand-written rules that can't ride the declarative-constraint
// path (all three need either page_context access or token-level fix
// proposals):
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

impl Rule<CapcoScheme> for NodisExdisClearsBannerRelToRule {
    fn id(&self) -> RuleId {
        RuleId::new("E039")
    }
    fn name(&self) -> &'static str {
        "nodis-exdis-clears-banner-rel-to"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
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

        let Some(page) = ctx.page_context.as_ref() else {
            return vec![];
        };

        let (expected_non_ic, _needs_nf) = page.expected_non_ic_dissem();
        let has_nodis_or_exdis = expected_non_ic
            .iter()
            .any(|d| matches!(d, NonIcDissem::Nodis | NonIcDissem::Exdis));
        if !has_nodis_or_exdis {
            return vec![];
        }

        // Point at the first RelToBlock (or RelToTrigraph) span so the
        // user sees exactly where the offending REL TO is.
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::RelToBlock)
            .or_else(|| {
                attrs
                    .token_spans
                    .iter()
                    .find(|t| t.kind == TokenKind::RelToTrigraph)
            })
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "REL TO is not authorized in the banner line when any portion \
             contains NODIS or EXDIS; NOFORN conveys the foreign-release \
             decision in this case per CAPCO-2016 §H.9",
            concat!("CAPCO-2016 §H.9 p172 (EXDIS) + ", "p174 (NODIS)",),
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
// explicit `&PageContext` parameter (the marking-type and page_context
// guards moved up to the walker's `check`).

/// Walker that asserts the banner / CAB candidate matches the page's
/// projected marking for each per-category roll-up. See the section header
/// above for the design rationale.
pub(crate) struct BannerMatchesProjectedRule;

impl Rule<CapcoScheme> for BannerMatchesProjectedRule {
    fn id(&self) -> RuleId {
        // Bookkeeping ID. Per-row IDs travel on emitted diagnostics for
        // audit traceability.
        RuleId::new("E031")
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

    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::MarkingType;

        // Marking-type guard (≤3 branches per D13).
        if !matches!(ctx.marking_type, MarkingType::Banner | MarkingType::Cab) {
            return vec![];
        }
        // Page-context guard.
        let Some(page) = ctx.page_context.as_ref() else {
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
        &[
            ("E031", "sar-banner-rollup"),
            ("E035", "sci-banner-rollup"),
            ("E040", "nodis-exdis-banner-rollup"),
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
    evaluate: fn(
        &CanonicalAttrs,
        &marque_ism::PageContext,
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
        rule_id: RuleId::new("E031"),
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
        rule_id: RuleId::new("E035"),
        severity: Severity::Error,
        evaluate: evaluate_sci_banner_rollup,
    },
    // Non-IC dissem — §H.9 p174 (NODIS) and §H.9 p172 (EXDIS): NODIS
    // takes priority over EXDIS, and either token, if present in any
    // portion, must roll up to the banner. Both passages are the
    // operative supersession-and-roll-up rule for this category.
    BannerCategoryRow {
        rule_id: RuleId::new("E040"),
        severity: Severity::Error,
        evaluate: evaluate_non_ic_dissem_banner_rollup,
    },
];

// ---------------------------------------------------------------------------
// Per-row evaluators
// ---------------------------------------------------------------------------

/// SAR banner roll-up evaluator. Verbatim move of the body of
/// `SarBannerRollupRule::check`, parameterized over the page projection
/// and the catalog row (so the row supplies the rule ID + severity).
///
/// Authority: CAPCO-2016 §H.5 p101 (Unique SAPs contained in portion
/// marks must always appear in the banner line; hierarchy depiction
/// optional per §H.5 p101 + p99).
fn evaluate_sar_banner_rollup(
    attrs: &CanonicalAttrs,
    page_context: &marque_ism::PageContext,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    let Some(expected) = page_context.expected_sar_marking() else {
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
    let missing_ids: Vec<&str> = sar_missing_programs(attrs.sar_markings.as_ref(), &expected);
    if missing_ids.is_empty() {
        return vec![];
    }

    const CITATION: &str = concat!(
        "CAPCO-2016 §H.5 p101 ",
        "(Unique SAPs contained in portion marks must always appear ",
        "in the banner line; hierarchy depiction optional per §H.5 ",
        "p101 + p99)",
    );

    // Sort missing identifiers per §H.5 p99 (ascending,
    // numeric first, then alpha) so the fix output is
    // deterministic and self-canonical for the new tail.
    let mut sorted_missing = missing_ids.clone();
    sorted_missing.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));

    match attrs.sar_markings.as_ref() {
        Some(_observed) => {
            let message = format!(
                "banner SAR block is missing programs present in portions: {}",
                sorted_missing.join(", "),
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
                rule: row.rule_id.clone(),
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
            // human place the block. The message wording describes the
            // actual shape of the violation (a whole missing block,
            // not a partial one) so the user isn't misled into
            // looking for a block to edit.
            let message = format!(
                "banner is missing an SAR block required by portions: \
                 {}",
                sorted_missing.join(", "),
            );
            let span = attrs
                .token_spans
                .first()
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));
            vec![Diagnostic::new(
                row.rule_id.clone(),
                Severity::Error,
                span,
                message,
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
    page: &marque_ism::PageContext,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    let expected = page_expected_sci_markings(page);
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
                                exp_comp.identifier.as_ref()
                            ));
                        }
                        Some(oc) => {
                            for exp_sub in exp_comp.sub_compartments.iter() {
                                if !oc.sub_compartments.iter().any(|s| s == exp_sub) {
                                    missing.push(format!(
                                        "{}-{} {} (sub-compartment missing from banner)",
                                        exp_key,
                                        exp_comp.identifier.as_ref(),
                                        exp_sub.as_ref()
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
        return vec![Diagnostic::new(
            row.rule_id.clone(),
            Severity::Error,
            Span::new(0, 0),
            format!(
                "banner is missing an SCI block that portions require: {}",
                missing.join("; ")
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

    vec![make_fix_diagnostic(FixDiagnosticParams {
        rule: row.rule_id.clone(),
        severity: row.severity,
        source: FixSource::BuiltinRule,
        span: fix_span,
        message: format!(
            "banner SCI block is missing markings present in the page's \
             portions (systems, compartments, and/or sub-compartments): {}",
            missing.join("; ")
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
    page: &marque_ism::PageContext,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    use marque_ism::NonIcDissem;

    let (expected_non_ic, _) = page.expected_non_ic_dissem();
    let portions_have_nodis = expected_non_ic
        .iter()
        .any(|d| matches!(d, NonIcDissem::Nodis));
    let portions_have_exdis = expected_non_ic
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
    let message = format!(
        "banner is missing {required_str} required by portions \
         (§H.9 roll-up rule: {required_str} in any portion must \
         appear in the banner)"
    );
    const CITATION: &str = concat!("CAPCO-2016 §H.9 p172 (EXDIS) + ", "p174 (NODIS)",);

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
                rule: row.rule_id.clone(),
                severity: row.severity,
                source: FixSource::BuiltinRule,
                span: insertion,
                message,
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
            vec![Diagnostic::new(
                row.rule_id.clone(),
                Severity::Error,
                span,
                message,
                CITATION,
                None,
            )]
        }
    }
}

// ---------------------------------------------------------------------------
// Rule: E041 — Portion-level NODIS supersedes EXDIS
// ---------------------------------------------------------------------------

/// Fires when a portion carries BOTH NODIS and EXDIS. Emits a
/// `Warn` diagnostic pointing at the EXDIS token; no auto-fix
/// (see the "# No auto-fix" section below). Per the supersession
/// rule in §H.9, NODIS survives and the user removes EXDIS
/// manually.
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
/// EXDIS cannot coexist" rule from line 4235/4295). When a portion
/// has both tokens, both rules fire:
/// - E037 (`Error`, no fix) states the violation.
/// - E041 (`Warn`, no fix) states the supersession rule: NODIS wins,
///   so EXDIS must be removed from the portion marking.
///
/// After the user manually removes EXDIS, re-linting clears both
/// diagnostics.
///
/// # Severity
///
/// `Warn` — the diagnostic surfaces the supersession rule; the user
/// resolves manually by removing EXDIS. Orgs that want to escalate
/// can configure `E041 = "error"` in `.marque.toml`.
///
/// # No auto-fix
///
/// The source is unambiguous about which marking survives (NODIS),
/// but auto-removing EXDIS would require constructing a clean
/// `FixProposal.original` spanning `XD` + an adjacent `/` separator.
/// The parser emits `TokenKind::Separator` only for between-category
/// `//` — within-category `/` is gap bytes the rule cannot safely
/// reconstruct. A fix implementation that overruns the single
/// within-category byte risks corrupting the audit record per
/// Constitution V. E041 therefore ships as a no-fix diagnostic;
/// a follow-up PR can add the auto-fix once within-category
/// separator handling lands in the parser.
struct NodisSupersedesExdisInPortionRule;

impl Rule<CapcoScheme> for NodisSupersedesExdisInPortionRule {
    fn id(&self) -> RuleId {
        RuleId::new("E041")
    }
    fn name(&self) -> &'static str {
        "nodis-supersedes-exdis-in-portion"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
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

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            exdis_span_tok.span,
            "portion contains both NODIS and EXDIS; NODIS (ND) supersedes \
             EXDIS (XD) per §H.9 — remove EXDIS from the portion mark",
            concat!(
                "CAPCO-2016 §H.9 p172 (EXDIS) + ",
                "p174 (NODIS): NODIS supersedes EXDIS in the ",
                "portion mark when both are present",
            ),
            None,
        )]
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_capco_test_support::{lint_banner, lint_portion};

    #[test]
    fn capco_rule_set_registers_all_rules() {
        let set = CapcoRuleSet::new();
        let ids: Vec<&str> = set.rules().iter().map(|r| r.id().as_str()).collect();
        // Kept rules.
        assert!(ids.contains(&"E002"));
        assert!(ids.contains(&"E005"));
        assert!(ids.contains(&"E006"));
        assert!(ids.contains(&"E007"));
        assert!(ids.contains(&"E008"));
        assert!(ids.contains(&"E010"));
        assert!(ids.contains(&"E012"));
        assert!(ids.contains(&"E014"));
        assert!(ids.contains(&"E015"));
        assert!(ids.contains(&"W002"));
        assert!(ids.contains(&"E016"));
        assert!(ids.contains(&"E021"));
        assert!(ids.contains(&"E024"));
        assert!(ids.contains(&"W003"));
        assert!(ids.contains(&"C001"));
        assert!(ids.contains(&"E031")); // BannerMatchesProjectedRule walker
        assert!(ids.contains(&"W034"));
        assert!(ids.contains(&"E036"));
        assert!(ids.contains(&"E037"));
        assert!(ids.contains(&"E038"));
        assert!(ids.contains(&"E039"));
        assert!(ids.contains(&"E041"));
        assert!(ids.contains(&"S003"));
        assert!(ids.contains(&"S004"));
        assert!(ids.contains(&"S005"));
        assert!(ids.contains(&"S006"));
        assert!(ids.contains(&"E053"));
        assert!(ids.contains(&"E054"));
        assert!(ids.contains(&"E055"));
        assert!(ids.contains(&"E056"));
        assert!(ids.contains(&"E057"));
        // PR 3c.B Commit 7.3: `DeclarativeClassFloorRule` (E058) retired.
        // The 27 catalog rows fire through the engine's constraint-
        // catalog bridge with `Diagnostic.rule = "E058"` (audit-stream
        // continuity); no registered `Rule::id() == "E058"` post-7.3.
        assert!(
            !ids.contains(&"E058"),
            "E058 walker retired in PR 3c.B Commit 7.3; the catalog rows \
             emit via the engine bridge."
        );
        // PR 3c.B Commit 7.4: `DeclarativeSciPerSystemRule` (E059) retired.
        // The 5 catalog rows fire through the bridge's direct path
        // (`CapcoScheme::bridge_sci_per_system_diagnostics`) with
        // `Diagnostic.rule = "E059"` and full `FixProposal` payloads;
        // no registered `Rule::id() == "E059"` post-7.4.
        assert!(
            !ids.contains(&"E059"),
            "E059 walker retired in PR 3c.B Commit 7.4; the catalog rows \
             emit via the engine bridge with fixes intact."
        );

        // Retired-rule guards. PR 3c.B Commit 6 retires 13 rules + the
        // E060 walker into `MarkingScheme::render_canonical` (form-bucket
        // migration). See `docs/plans/2026-05-10-pr3c-consolidated-plan.md`
        // lines 788–862 for the architectural commitment.
        assert!(
            !ids.contains(&"E001"),
            "E001 retired in PR 3c.B Commit 6 — portion-mark-in-banner \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E003"),
            "E003 retired in PR 3c.B Commit 6 — block ordering absorbed \
             by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E004"),
            "E004 retired in PR 3c.B Commit 6 — separator normalization \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E009"),
            "E009 retired in PR 3c.B Commit 6 — banner→portion abbrev \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"S001"),
            "S001 retired in PR 3c.B Commit 6 — banner-abbrev preference \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"S002"),
            "S002 retired in PR 3c.B Commit 6 — banner-form consistency \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E011"),
            "E011 retired in PR 3c.B Commit 6 — //-prefix normalization \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E013"),
            "E013 retired in PR 3c.B Commit 6 — list-delimiter \
             normalization absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E026"),
            "E026 retired in PR 3c.B Commit 6 — SAR portion form \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E029"),
            "E029 retired in PR 3c.B Commit 6 — SAR compartment order \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E030"),
            "E030 retired in PR 3c.B Commit 6 — SAR indicator repetition \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E032"),
            "E032 retired in PR 3c.B Commit 6 — SCI sort order absorbed \
             by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E052"),
            "E052 retired in PR 3c.B Commit 6 — REL TO duplicates \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E060"),
            "E060 (non-canonical input walker) retired in PR 3c.B Commit \
             6 — its 5 ordering rows are absorbed by \
             MarkingScheme::render_canonical"
        );

        // Pre-existing retirement guards (still valid).
        assert!(!ids.contains(&"W001"), "W001 retired in T035c-14");
        assert!(!ids.contains(&"E017"), "E017 retired in T035b");
        assert!(!ids.contains(&"E018"), "E018 retired in T035b");
        assert!(!ids.contains(&"E019"), "E019 retired in T035b");
        assert!(
            !ids.contains(&"E020"),
            "E020 retired in PR 3b.F (and now via E060 in PR 3c.B Commit 6)"
        );
        assert!(
            !ids.contains(&"E022"),
            "E022 retired in PR 3b.D into E058 catalog"
        );
        assert!(
            !ids.contains(&"E023"),
            "E023 retired in PR 3b.F (and now via E060 in PR 3c.B Commit 6)"
        );
        assert!(
            !ids.contains(&"E025"),
            "E025 retired in PR 3b.D into E058 catalog"
        );
        assert!(
            !ids.contains(&"E027"),
            "E027 retired in PR 3b.D into E058 catalog"
        );
        assert!(
            !ids.contains(&"E028"),
            "E028 retired in PR 3b.F (and now via E060 in PR 3c.B Commit 6)"
        );
        assert!(
            !ids.contains(&"E033"),
            "E033 retired in PR 3b.F (and now via E060 in PR 3c.B Commit 6)"
        );
        assert!(
            !ids.contains(&"E035"),
            "E035 retired as a registered rule ID by T026a; emitted as a \
             per-row catalog ID by BannerMatchesProjectedRule"
        );
        assert!(
            !ids.contains(&"E040"),
            "E040 retired as a registered rule ID by T026a; emitted as a \
             per-row catalog ID by BannerMatchesProjectedRule"
        );
        for retired_e042_to_e051 in [
            "E042", "E043", "E044", "E045", "E046", "E047", "E048", "E049", "E050", "E051",
        ] {
            assert!(
                !ids.contains(&retired_e042_to_e051),
                "{retired_e042_to_e051} retired in PR 3b.E into the E059 SCI per-system walker"
            );
        }

        // Post-PR-3c.B-Commit-7.4 registered count: 31 rules.
        // History: PR 3b umbrella closed at 47. PR 3c.B Commit 6 retires
        // 13 form rules + 1 walker (E060) into the renderer (form-bucket
        // migration); 47 - 14 = 33. PR 3c.B Commit 7.3 retires
        // `DeclarativeClassFloorRule` (E058) into the constraint-catalog
        // bridge; 33 - 1 = 32. PR 3c.B Commit 7.4 retires
        // `DeclarativeSciPerSystemRule` (E059) into the bridge's direct
        // path (with fixes preserved); 32 - 1 = 31.
        assert_eq!(set.rules().len(), 31);
    }

    #[test]
    fn e002_fires_when_usa_missing_with_real_span() {
        let src_str = "SECRET//REL TO GBR, AUS";
        let diags = lint_banner(src_str);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        // Span covers the full REL TO trigraph list (first → last), not
        // just the first trigraph — required so `Engine::fix` can splice
        // the full list with the canonical replacement in one step.
        assert_eq!(e002[0].span.as_str(src_str.as_bytes()).unwrap(), "GBR, AUS");
    }

    // T035c-10: fix canonicalization — E002's replacement must produce
    // the fully canonical REL TO list (USA first + non-USA entries
    // alphabetical per CAPCO-2016 §H.8 p151) in a single pass. This
    // is required because E020 gates on `rel_to[0] == USA` and so is
    // silent whenever E002 fires; if E002's fix preserved input order,
    // the output would still carry a latent alphabetical-ordering
    // violation that only a second pass would catch.

    #[test]
    fn e002_fix_sorts_non_usa_trigraphs_when_usa_missing() {
        // USA absent and non-USA entries in non-alphabetical order.
        // Canonical form: USA, AUS, GBR.
        let diags = lint_banner("SECRET//REL TO GBR, AUS");
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a FixProposal");
        assert_eq!(
            fix.replacement.as_ref(),
            "USA, AUS, GBR",
            "E002 must produce canonical REL TO (USA first + alphabetical rest)"
        );
    }

    #[test]
    fn e002_fix_sorts_non_usa_trigraphs_when_usa_misplaced() {
        // USA present but not first, and non-USA entries unsorted.
        // Canonical form: USA, AUS, GBR.
        let diags = lint_banner("SECRET//REL TO GBR, USA, AUS");
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a FixProposal");
        assert_eq!(
            fix.replacement.as_ref(),
            "USA, AUS, GBR",
            "E002 must produce canonical REL TO in one pass: {}",
            fix.replacement.as_ref()
        );
    }

    // T035c-10 second-round review fixes: trailing-delimiter tail
    // consumption and multi-block suppression.

    #[test]
    fn e002_fix_consumes_trailing_comma_in_rel_to_block() {
        // `REL TO GBR, AUS,` has a trailing `,` inside the RelToBlock.
        // Splicing only `GBR, AUS` (first→last trigraph) would leave
        // the trailing `,` behind: `REL TO USA, AUS, GBR,` — still
        // malformed. The fix span must extend through the delimiter
        // tail so the rewritten banner is clean.
        let src = "SECRET//REL TO GBR, AUS,";
        let diags = lint_banner(src);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a fix");
        assert_eq!(
            fix.span.as_str(src.as_bytes()).unwrap(),
            "GBR, AUS,",
            "fix span must cover the delimiter-only tail so splicing \
             leaves no stale `,`/whitespace behind"
        );
    }

    #[test]
    fn e002_fix_span_includes_recognized_tetragraph_tail() {
        // Issue #183 PR-A: tetragraphs (FVEY, ACGU, NATO, …) are now
        // first-class `CountryCode` values, recognized by
        // `is_trigraph` and stored in `rel_to`. The E002 fix span
        // (first→last `RelToTrigraph` token within the block) must
        // therefore extend through FVEY in the tail — splicing
        // `GBR, AUS` only would leave a stale `, FVEY` behind. Pre-
        // PR-A this test asserted the inverse (FVEY was silently
        // dropped at the parser, so the splice intentionally stopped
        // at AUS); the inverse is now wrong.
        let src = "SECRET//REL TO GBR, AUS, FVEY";
        let diags = lint_banner(src);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a fix");
        assert_eq!(
            fix.span.as_str(src.as_bytes()).unwrap(),
            "GBR, AUS, FVEY",
            "tetragraph FVEY is now a recognized country code (issue #183) \
             — the fix span must include it",
        );
    }

    #[test]
    fn e002_fix_span_stops_at_unrecognized_tail_token() {
        // Companion to `e002_fix_span_includes_recognized_tetragraph_tail`
        // — the defensive invariant that a non-recognized tail token
        // is NOT swallowed by the splice still holds. Issue #183 PR-A
        // widened recognition from trigraphs to all CVE country codes
        // (incl. tetragraphs and the longer registered codes), but
        // anything outside that vocabulary still fails the
        // `is_trigraph` gate at the parser, never gets a
        // `RelToTrigraph` token span, and so the fix span stops at
        // the last recognized code. `XYZQ` here is a 4-char string
        // outside the CVE TRIGRAPHS list.
        let src = "SECRET//REL TO GBR, AUS, XYZQ";
        let diags = lint_banner(src);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a fix");
        assert_eq!(
            fix.span.as_str(src.as_bytes()).unwrap(),
            "GBR, AUS",
            "unrecognized tail token must not be swallowed by the splice"
        );
    }

    #[test]
    fn e002_suppresses_fix_on_multiple_rel_to_blocks() {
        // If the parser sees more than one REL TO block in a marking,
        // a single first→last splice would delete intervening `//...//`
        // content (here `//NF//`). The rule must emit a diagnostic
        // without a FixProposal so the engine cannot corrupt the
        // source.
        let src = "SECRET//REL TO GBR//NF//REL TO AUS";
        let diags = lint_banner(src);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(
            e002.len(),
            1,
            "E002 must still fire (diagnostic present): {diags:?}"
        );
        assert!(
            e002[0].fix.is_none(),
            "E002 must NOT carry a fix when multiple REL TO blocks \
             are present (cross-block splice would delete intervening \
             `//NF//`): {e002:?}"
        );
    }

    #[test]
    fn e002_fix_output_does_not_trigger_e020() {
        // Apply E002's fix as the new input and confirm E020 stays silent —
        // this is the invariant that lets E020 gate on `rel_to[0] == USA`.
        let diags_round1 = lint_banner("CONFIDENTIAL//REL TO FRA, DEU");
        let e002: Vec<_> = diags_round1
            .iter()
            .filter(|d| d.rule.as_str() == "E002")
            .collect();
        assert_eq!(e002.len(), 1);
        let fixed = e002[0].fix.as_ref().unwrap().replacement.as_ref();
        assert_eq!(fixed, "USA, DEU, FRA");

        // Round 2: feed the canonicalized REL TO back through the linter;
        // neither E002 nor the E060 walker (REL TO row) should fire on
        // the rewritten banner.
        let round2_banner = format!("CONFIDENTIAL//REL TO {fixed}");
        let diags_round2 = lint_banner(&round2_banner);
        assert!(
            diags_round2
                .iter()
                .all(|d| d.rule.as_str() != "E002" && d.rule.as_str() != "E060"),
            "E002's canonical output must not fire E002 or E060: {diags_round2:?}"
        );
    }

    #[test]
    fn e005_fires_on_declass_exemption_in_banner() {
        let diags = lint_banner("SECRET//25X1//NOFORN");
        let e005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E005").collect();
        assert_eq!(e005.len(), 1);
        let src = b"SECRET//25X1//NOFORN";
        assert_eq!(e005[0].span.as_str(src).unwrap(), "25X1");
    }

    // T035c-16: E005 audit — scope expansion and citation lockdown.

    #[test]
    fn e005_fires_on_declass_exemption_in_portion() {
        // Portion-scope coverage: CAPCO §D.1 p27's closed category list
        // for banners is mirrored for portions (§C.1 p26 lines 525ff),
        // so `25X1` between `//` separators in a portion is just as
        // misplaced as in a banner. Before T035c-16 this fired nothing
        // (the rule was banner-only); the audit extended scope to cover
        // portions.
        let diags = lint_portion("(S//25X1//NF)");
        let e005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E005").collect();
        assert_eq!(
            e005.len(),
            1,
            "E005 must fire on declass exemption inside a portion: {diags:?}"
        );
        let src = b"(S//25X1//NF)";
        assert_eq!(e005[0].span.as_str(src).unwrap(), "25X1");
    }

    #[test]
    fn e005_citation_points_at_specific_sections() {
        // Lock down the T035c-16 citation retargeting — `§E.1 p31` and
        // `§D.1 p27` are the specific passages that jointly establish
        // the invariant. A future regression that drifts to a bare
        // `§E` would pass Constitution VIII's surface check but fail
        // re-verifiability, which is the whole point.
        let diags = lint_banner("SECRET//25X1//NOFORN");
        let e005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E005").collect();
        assert_eq!(e005.len(), 1);
        assert!(
            e005[0].citation.contains("§E.1 p31"),
            "E005 citation must reference §E.1 p31 (Declassify On is a CAB line); \
             got: {:?}",
            e005[0].citation
        );
        assert!(
            e005[0].citation.contains("§D.1 p27"),
            "E005 citation must reference §D.1 p27 (banner categories exclude \
             declassification); got: {:?}",
            e005[0].citation
        );
    }

    #[test]
    fn e008_fires_on_unknown_token() {
        let diags = lint_banner("SECRET//XYZZY//NOFORN");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        assert_eq!(e008.len(), 1);
        let src = b"SECRET//XYZZY//NOFORN";
        assert_eq!(e008[0].span.as_str(src).unwrap(), "XYZZY");
    }

    #[test]
    fn looks_like_deprecated_x_shorthand_matches_expected_patterns() {
        use super::looks_like_deprecated_x_shorthand as m;
        // Deprecated forms (must match)
        assert!(m("25X1-"));
        assert!(m("25X2-"));
        assert!(m("25X9-"));
        assert!(m("50X1-"));
        assert!(m("50X1-HUM-"));
        assert!(m("25X3-WMD-"));
        // Canonical forms (must NOT match — no trailing dash)
        assert!(!m("25X1"));
        assert!(!m("50X1-HUM"));
        // Malformed / unrelated
        assert!(!m(""));
        assert!(!m("-"));
        assert!(!m("X1-"));
        assert!(!m("25-X1-"));
        assert!(!m("25X-"));
        assert!(!m("ABCX1-"));
        assert!(!m("25X1-hum-"), "lowercase suffix should not match");
        assert!(!m("NOFORN"));
    }

    #[test]
    fn e007_fires_on_pattern_matched_x_shorthand_not_in_migration_table() {
        // `25X2-` is NOT in the seed MIGRATIONS table. Before the pattern
        // fallback, this would have fallen through to E008. Now E007
        // should fire with a confidence of 0.95 and a replacement of
        // `25X2` (trailing `-` stripped).
        let diags = lint_banner("SECRET//25X2-//NOFORN");
        let e007: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E007").collect();
        assert_eq!(e007.len(), 1);
        let fix = e007[0].fix.as_ref().expect("E007 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "25X2");
        assert!((fix.confidence.combined() - 0.95).abs() < f32::EPSILON);
        // E008 must NOT also fire on the same span.
        assert!(diags.iter().all(|d| d.rule.as_str() != "E008"));
    }

    #[test]
    fn e007_still_fires_on_migration_table_entries() {
        // The existing 25X1- path (table-backed) must still work.
        let diags = lint_banner("SECRET//25X1-//NOFORN");
        let e007: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E007").collect();
        assert_eq!(e007.len(), 1);
        let fix = e007[0].fix.as_ref().unwrap();
        assert_eq!(fix.replacement.as_ref(), "25X1");
        // Table confidence from the seed MIGRATIONS entry (0.97).
        assert!((fix.confidence.combined() - 0.97).abs() < f32::EPSILON);
    }

    #[test]
    fn migrations_table_contains_no_fouo_entry() {
        // FOUO remains a valid CAPCO dissem control per CVEnumISMDissem.xml
        // and CAPCO-2016 §F. CUI is a separate (NARA) marking system, not a
        // CAPCO dissem control. A prior `FOUO → CUI` migration entry was
        // removed as factually incorrect; this regression guard prevents
        // re-introduction. Any future "suggest CUI on non-IC documents"
        // behavior must live in a CUI adapter gated by opt-in config.
        use marque_ism::generated::migrations::find_migration;
        assert!(
            find_migration("FOUO").is_none(),
            "FOUO must not appear in MIGRATIONS (see crates/ism/build.rs doc block)"
        );
    }

    #[test]
    fn migrations_table_contains_no_limdis_entry() {
        // LIMDIS is a current non-IC dissem control (CAPCO-2016 §H.9).
        // A prior `LIMDIS → RELIDO` migration entry was removed as
        // factually incorrect; this regression guard prevents
        // re-introduction.
        use marque_ism::generated::migrations::find_migration;
        assert!(
            find_migration("LIMDIS").is_none(),
            "LIMDIS must not appear in MIGRATIONS (see crates/ism/build.rs doc block)"
        );
    }

    #[test]
    fn e006_does_not_fire_on_fouo_in_banner() {
        // Full-pipeline regression: the absence of a FOUO migration entry
        // must produce no E006 diagnostic in a banner containing FOUO.
        // The policy question "FOUO in a classified banner" is handled at
        // the PageContext roll-up (FOUO drops from classified banners) and
        // in Phase C as a declarative `Constraint::Conflicts(FOUO, Classified)`.
        let diags = lint_banner("UNCLASSIFIED//FOUO");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E006"),
            "E006 must not fire on FOUO: {diags:?}"
        );
    }

    #[test]
    fn e008_no_fix_offered() {
        let diags = lint_banner("SECRET//XYZZY//NOFORN");
        let e008 = diags.iter().find(|d| d.rule.as_str() == "E008").unwrap();
        assert!(e008.fix.is_none(), "FR-012: E008 must not propose a fix");
    }

    // T035c-12: pin-down tests for E008's four suppression paths,
    // plus regression guards that confirm E008 still fires when expected.

    #[test]
    fn e008_suppressed_on_migration_backed_unknown() {
        // `25X1-` is an Unknown token that the seed MIGRATIONS table
        // captures. E007 owns X-shorthand; E008 must step aside AND
        // E007 must actually fire — otherwise a future change that
        // breaks E007's migration lookup could produce a silent
        // suppression with no diagnostic at all.
        let diags = lint_banner("SECRET//25X1-//NOFORN");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        let e007: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E007").collect();
        assert!(
            e008.is_empty(),
            "E008 must be suppressed for migration-backed X-shorthand \
             (E007 owns this path): {diags:?}"
        );
        assert!(
            !e007.is_empty(),
            "E007 must fire for migration-backed X-shorthand — \
             otherwise suppression is a silent drop: {diags:?}"
        );
    }

    #[test]
    fn e008_suppressed_on_pattern_matched_x_shorthand() {
        // `25X9-` is not in the seed MIGRATIONS table but matches the
        // X-shorthand pattern E007 catches via fallback. E008 must
        // still step aside — see the suppression path 2 in the rule
        // doc comment. Also assert that E007 actually fires so this
        // cannot regress into a silent drop where E008 is suppressed
        // but no owning diagnostic is emitted.
        let diags = lint_banner("SECRET//25X9-//NOFORN");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        let e007: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E007").collect();
        assert!(
            e008.is_empty(),
            "E008 must be suppressed for pattern-matched X-shorthand \
             even when not in seed MIGRATIONS (E007 owns): {diags:?}"
        );
        assert!(
            !e007.is_empty(),
            "E007 must fire for pattern-matched X-shorthand — \
             otherwise suppression is a silent drop: {diags:?}"
        );
    }

    #[test]
    fn e008_fires_on_malformed_first_sar_with_empty_program() {
        // `SAR-` alone (no program identifier) fails SAR grammar. The
        // parser does not produce a `SarMarking`, so `attrs.sar_markings`
        // stays `None` and `SarIndicatorRepeatRule::check` returns early
        // at its `attrs.sar_markings.is_none()` guard. An earlier
        // version of E008's suppression matched on prefix only, so this
        // malformed token was silently dropped. Tightening the
        // suppression to require `attrs.sar_markings.is_some()` AND a
        // non-empty suffix restores the E008 error.
        let diags = lint_banner("SECRET//SAR-//NOFORN");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        assert!(
            !e008.is_empty(),
            "E008 must fire on malformed first SAR (empty program) — \
             E030 cannot run without a successful first SAR, so E008 \
             is the only rule that can surface this: {diags:?}"
        );
    }

    #[test]
    fn e008_fires_on_malformed_first_spelled_sar_with_empty_program() {
        // Same regression as above for the `SPECIAL ACCESS REQUIRED-`
        // prefix. `SPECIAL ACCESS REQUIRED-` with no program must not
        // be silently dropped.
        let diags = lint_banner("SECRET//SPECIAL ACCESS REQUIRED-//NOFORN");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        assert!(
            !e008.is_empty(),
            "E008 must fire on malformed first `SPECIAL ACCESS \
             REQUIRED-` (empty program): {diags:?}"
        );
    }

    #[test]
    fn no_diagnostics_on_clean_banner() {
        let diags = lint_banner("TOP SECRET//SI//NOFORN");
        assert!(
            diags.is_empty(),
            "clean banner should produce no diagnostics, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostics_on_clean_portion() {
        let diags = lint_portion("(S//NF)");
        // Both "S" and "NF" are correct portion-form abbreviations.
        // E001 must not fire (not a banner), and E009 must not fire
        // (already using abbreviated forms).
        assert!(
            diags.is_empty(),
            "clean portion should produce no diagnostics, got: {diags:?}"
        );
    }

    // --- S003: joint-usa-first (style) ---

    #[test]
    fn s003_fires_when_joint_usa_not_first() {
        // `AUS GBR USA` is pure-alpha canonical per §H.3 p56
        // (E020 is silent), but USA is last. S003 fires at Info
        // severity and offers a fix that reorders to USA-first.
        let src = "//JOINT S AUS GBR USA";
        let diags = lint_banner(src);
        let s003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S003").collect();
        assert_eq!(
            s003.len(),
            1,
            "S003 must fire on JOINT with USA not first: {diags:?}"
        );
        assert_eq!(s003[0].severity, Severity::Info);

        let fix = s003[0].fix.as_ref().expect("S003 must carry a fix");
        // Span covers the full Classification token.
        assert_eq!(
            fix.span.as_str(src.as_bytes()).unwrap(),
            "JOINT S AUS GBR USA",
            "S003 span must cover the full Classification token"
        );
        assert_eq!(fix.original.as_ref(), "JOINT S AUS GBR USA");
        assert_eq!(
            fix.replacement.as_ref(),
            "JOINT S USA AUS GBR",
            "S003 fix must move USA to first, rest alphabetical"
        );

        // Applied splice: preserves `//JOINT S ` banner prefix and
        // produces the canonical USA-first list.
        let mut buf = src.as_bytes().to_vec();
        buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());
        let applied = std::str::from_utf8(&buf).unwrap();
        assert_eq!(
            applied, "//JOINT S USA AUS GBR",
            "applied fix must produce canonical USA-first JOINT block"
        );
    }

    #[test]
    fn s003_does_not_fire_when_usa_already_first() {
        let diags = lint_banner("//JOINT S USA GBR AUS//REL TO USA, AUS, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S003"),
            "S003 must not fire when USA is already first: {diags:?}"
        );
    }

    #[test]
    fn s003_does_not_fire_without_usa_in_joint_list() {
        // Anomalous per §H.3 p163 (USA always in JOINT), but
        // S003 only fires when USA IS present but not first. Other
        // rules flag the missing-USA case.
        let diags = lint_banner("//JOINT S GBR AUS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S003"),
            "S003 must not fire when USA is absent: {diags:?}"
        );
    }

    #[test]
    fn s003_does_not_fire_on_single_country_joint() {
        // Single-country JOINT (just USA) — nothing to reorder.
        let diags = lint_banner("//JOINT S USA");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S003"),
            "S003 must not fire on single-country JOINT: {diags:?}"
        );
    }

    #[test]
    fn s003_does_not_fire_in_portion() {
        // S003 is banner-only, matching S001/S002's scope. Portion-
        // form JOINT is rarely used; convention-based style rules
        // are banner-focused.
        let diags = lint_portion("(//JOINT S AUS GBR USA)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S003"),
            "S003 must not fire in portion context: {diags:?}"
        );
    }

    #[test]
    fn s003_citation_frames_as_convention_not_mandate() {
        // Constitution VIII: the citation MUST make clear that S003
        // encodes convention, not a CAPCO mandate. §H.3 is explicitly
        // silent on USA-first. Lock the "IC convention" framing so a
        // regression that fabricates a §H.3 carve-out fails here.
        let diags = lint_banner("//JOINT S AUS GBR USA");
        let s003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S003").collect();
        assert_eq!(s003.len(), 1);
        let citation = s003[0].citation;
        assert!(
            citation.contains("IC convention"),
            "S003 citation must frame as IC convention (not CAPCO \
             mandate); got: {citation:?}"
        );
        assert!(
            citation.contains("§H.3 p56"),
            "S003 citation must reference the §H.3 passage it defers to \
             (pure alpha); got: {citation:?}"
        );
        assert!(
            citation.contains("§H.8 pp 150"),
            "S003 citation must reference the REL TO USA-first source \
             at §H.8 pp 150–151 that establishes the convention; got: \
             {citation:?}"
        );
    }

    // --- S004: rel-to-trigraph-suggest (issue #235 / #186 PR-3) ---
    //
    // S004 surfaces a `Severity::Suggest` diagnostic when a REL TO
    // entry has a corpus-rare prior and a corpus-common 1- or 2-edit
    // neighbor. The fix is informational; the engine never auto-
    // applies a Suggest-severity diagnostic regardless of confidence.

    #[test]
    fn s004_fires_on_aut_suggesting_aus() {
        // The canonical #186 ambiguous fixture: `AUT` (Austria) is a
        // valid trigraph but rare in REL TO; `AUS` (Australia) is
        // far more common. The corpus prior delta exceeds
        // SUGGEST_LOG_MARGIN.
        let diags = lint_banner("SECRET//REL TO USA, AUT, GBR");
        let s004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S004").collect();
        assert_eq!(s004.len(), 1, "S004 must fire on AUT: {diags:?}");
        assert_eq!(s004[0].severity, marque_rules::Severity::Suggest);
        let fix = s004[0].fix.as_ref().expect("S004 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "AUS");
        // Original is the rare entry, replacement is the common one.
        assert_eq!(fix.original.as_ref(), "AUT");
    }

    #[test]
    fn s004_does_not_fire_on_pure_common_partner_list() {
        // USA, AUS, GBR are all common partners. No suggest channel.
        let diags = lint_banner("SECRET//REL TO USA, AUS, GBR");
        let s004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S004").collect();
        assert!(
            s004.is_empty(),
            "S004 must stay silent on common-partner REL TO: {diags:?}"
        );
    }

    #[test]
    fn s004_does_not_fire_when_rel_to_is_empty() {
        // Banner without REL TO is out of scope.
        let diags = lint_banner("SECRET//NOFORN");
        let s004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S004").collect();
        assert!(
            s004.is_empty(),
            "S004 must stay silent without REL TO: {diags:?}"
        );
    }

    #[test]
    fn s004_message_uses_canonical_token_strings_only() {
        // Constitution V audit-content-ignorance: the diagnostic
        // message must reference only the trigraph (vocabulary) and
        // English country names (vocabulary), never document text.
        let diags = lint_banner("SECRET//REL TO USA, AUT, GBR");
        let s004 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S004")
            .expect("S004 must fire");
        let msg = s004.message.as_ref();
        // Vocabulary-only references: trigraph, candidate, country name.
        assert!(
            msg.contains("\"AUT\""),
            "message must reference the rare trigraph: {msg}"
        );
        assert!(
            msg.contains("\"AUS\""),
            "message must reference the candidate: {msg}"
        );
        assert!(
            msg.contains("Austria") && msg.contains("Australia"),
            "message must use canonical country names: {msg}"
        );
        // No surrounding banner content (e.g., "SECRET", "GBR") leaks
        // into the message — those would be document text under the
        // content-ignorance invariant.
        assert!(
            !msg.contains("SECRET") && !msg.contains("GBR"),
            "message must not splice document content: {msg}"
        );
    }

    #[test]
    fn s004_does_not_fire_on_tetragraph_entry() {
        // `FVEY` is a 4-letter tetragraph, not a 3-letter trigraph;
        // the rule's `trigraph.len() != 3` guard must skip it. This
        // pins the no-tetragraph contract — S004 only operates on
        // trigraphs because tetragraph priors and edit-distance
        // semantics need their own calibration.
        let diags = lint_banner("SECRET//REL TO USA, FVEY");
        let s004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S004").collect();
        assert!(
            s004.is_empty(),
            "S004 must skip tetragraph entries: {diags:?}"
        );
    }

    #[test]
    fn s004_edit_distance_handles_empty_inputs() {
        // The two early-return paths in the DP: when either input
        // is empty, the distance is the length of the other. Pin
        // both so the helper stays correct as it picks up callers
        // beyond S004.
        assert_eq!(super::s004_edit_distance("", ""), 0);
        assert_eq!(super::s004_edit_distance("", "AUS"), 3);
        assert_eq!(super::s004_edit_distance("AUS", ""), 3);
    }

    #[test]
    fn s004_edit_distance_pins_canonical_pairs() {
        // The substitution / transposition path the rule actually
        // walks for the canonical #186 ambiguous fixtures. Edit-
        // distance ≤ 2 is the gate; ≥ 3 must be excluded.
        assert_eq!(super::s004_edit_distance("AUS", "AUS"), 0);
        assert_eq!(super::s004_edit_distance("AUT", "AUS"), 1); // substitution
        assert_eq!(super::s004_edit_distance("USB", "USA"), 1); // substitution
        assert_eq!(super::s004_edit_distance("ASU", "AUS"), 2); // transposition (2 substitutions)
        assert_eq!(super::s004_edit_distance("AUS", "GBR"), 3); // beyond threshold
    }

    #[test]
    fn s004_message_renders_all_country_name_arms() {
        // The four `(entry_name, candidate_name)` arms each have a
        // distinct phrasing because the surrounding parenthetical
        // English name only renders when the trigraph is in the
        // hand-curated COUNTRY_NAMES table. Driving every arm
        // through real `CanonicalAttrs` requires manufactured
        // priors — pinning the helper directly keeps the contract
        // visible and stable.
        //
        // (Some, Some): canonical AUT → AUS form with both names.
        let both = super::s004_message("AUT", "AUS", Some("Austria"), Some("Australia"));
        assert!(both.contains("Austria"));
        assert!(both.contains("Australia"));
        assert!(both.contains("far less common"));
        assert!(both.contains("did you mean \"AUS\""));

        // (None, Some): rare trigraph not in COUNTRY_NAMES.
        let rare_unnamed = super::s004_message("XYZ", "AUS", None, Some("Australia"));
        assert!(rare_unnamed.contains("\"XYZ\" is rare"));
        assert!(rare_unnamed.contains("\"AUS\" (Australia)"));
        // The "(EnglishName)" parenthetical only appears for the
        // candidate, not for the unnamed trigraph itself.
        assert!(!rare_unnamed.contains("\"XYZ\" ("));

        // (Some, None): candidate not in COUNTRY_NAMES.
        let candidate_unnamed = super::s004_message("AUT", "XYZ", Some("Austria"), None);
        assert!(candidate_unnamed.contains("\"AUT\" (Austria)"));
        assert!(candidate_unnamed.contains("did you mean \"XYZ\""));
        // No trailing "(name)" for the unnamed candidate.
        assert!(!candidate_unnamed.contains("\"XYZ\" ("));

        // (None, None): neither in COUNTRY_NAMES.
        let neither = super::s004_message("XYZ", "ABC", None, None);
        assert!(neither.contains("\"XYZ\" is rare"));
        assert!(neither.contains("did you mean \"ABC\""));
        assert!(!neither.contains("("));
    }

    #[test]
    fn s004_message_never_contains_document_content() {
        // Constitution V audit-content-ignorance: the helper takes
        // only vocabulary inputs — trigraph tokens and English
        // country names — so even passing it adversarial inputs
        // cannot leak document body text. The rule body is
        // responsible for never SOURCING those inputs from the
        // document; this test pins the helper's promise.
        let msg = super::s004_message("AUT", "AUS", Some("Austria"), Some("Australia"));
        // Sanity: the helper output references only its inputs.
        let allowed_tokens = ["AUT", "AUS", "Austria", "Australia"];
        // Strip the structural words and check what's left is
        // either whitespace, punctuation, or one of the inputs.
        for word in msg.split_whitespace() {
            let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
            if trimmed.is_empty() {
                continue;
            }
            let in_allowed = allowed_tokens.contains(&trimmed);
            let in_phrasing = matches!(
                trimmed,
                "is" | "far"
                    | "less"
                    | "common"
                    | "in"
                    | "REL"
                    | "TO"
                    | "than"
                    | "did"
                    | "you"
                    | "mean"
            );
            assert!(
                in_allowed || in_phrasing,
                "unexpected token {trimmed:?} in S004 message: {msg}"
            );
        }
    }

    #[test]
    fn s004_skips_when_trigraph_spans_shorter_than_rel_to_list() {
        // Defensive guard against a future parser change that no
        // longer emits one `RelToTrigraph` token span per `rel_to`
        // entry. Today the parser does emit them 1:1; if that
        // contract drifts (e.g., a parser refactor that filters
        // tetragraph-expanded entries differently), the rule must
        // skip the misaligned entries instead of producing a
        // diagnostic with the wrong span.
        use marque_ism::{CanonicalAttrs, CountryCode};
        use marque_rules::{MarkingType, RuleContext};

        let mut attrs = CanonicalAttrs::default();
        // Two REL TO entries (AUT triggers the suggest, USA does
        // not) but ZERO RelToTrigraph token spans — the defensive
        // path must hit the `trigraph_spans.get(idx)` None arm
        // for AUT's would-be suggestion and bail out.
        attrs.rel_to = Box::new([
            CountryCode::try_new(b"USA").expect("USA is a valid country code"),
            CountryCode::try_new(b"AUT").expect("AUT is a valid country code"),
        ]);
        // Leave attrs.token_spans empty.

        let ctx = RuleContext {
            marking_type: MarkingType::Banner,
            zone: None,
            position: None,
            page_context: None,
            corrections: None,
        };
        let rule = super::RelToTrigraphSuggestRule;
        let diags =
            <super::RelToTrigraphSuggestRule as Rule<CapcoScheme>>::check(&rule, &attrs, &ctx);
        assert!(
            diags.is_empty(),
            "S004 must skip when trigraph spans don't align with rel_to: {diags:?}"
        );
    }

    #[test]
    fn s004_tie_breaking_is_deterministic() {
        // M-1 (Copilot review): the doc comment promises tie-breaking
        // by (1) shorter edit distance, then (2) lexicographic order
        // on the token. Pin the contract end-to-end against the
        // canonical AUT → AUS fixture: AUS is the unique winner —
        // any rerun of the rule must pick AUS, never `AUT`-adjacent
        // codes that share a similar log-prior delta.
        let diags = lint_banner("SECRET//REL TO USA, AUT, GBR");
        let s004 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S004")
            .expect("S004 must fire on AUT");
        let fix = s004.fix.as_ref().expect("S004 must carry a fix");
        // Run again — same input, same output (no nondeterministic
        // tie-break paths).
        let diags2 = lint_banner("SECRET//REL TO USA, AUT, GBR");
        let s004_2 = diags2
            .iter()
            .find(|d| d.rule.as_str() == "S004")
            .expect("S004 must fire on second run");
        let fix2 = s004_2.fix.as_ref().expect("second-run fix");
        assert_eq!(
            fix.replacement.as_ref(),
            fix2.replacement.as_ref(),
            "S004 picks must be deterministic across runs"
        );
        assert_eq!(fix.replacement.as_ref(), "AUS");
    }

    // Note: the end-to-end engine-fix test for S004's suggest-don't-fix
    // invariant was relocated to `crates/capco/tests/s004_engine_fix.rs`.
    // Rationale: post-PR-3c.B Commit 2, `Engine` consumes
    // `CapcoScheme` through a generic-typed `MarkingScheme` bound. The
    // `marque-engine` ↔ `marque-capco` dev-dep cycle compiles the two
    // crates with separate `CapcoScheme` instances when an in-lib test
    // tries to construct an `Engine` directly, so the generic-bind
    // refuses to unify the two. Integration tests in
    // `crates/capco/tests/` see a single coherent `marque-capco` and
    // pass through cleanly.

    // --- S005: REL TO opaque-uncertain reduction (issue #206) ---
    //
    // Test fixtures use NA-deprecated tetragraphs from the V2022-NOV
    // taxonomy (RSMA, EUDA, BHTF) rather than the org-fork extension
    // example (`MNFI`) the plan §3.5 cites. Reason: org-fork
    // extensions live in `country_extensions.toml`, which ships
    // empty by default — a fixture using `MNFI` would require
    // populating extensions just for the test, polluting the
    // build-time data. NA-deprecated codes are in the CVE recognition
    // surface so the parser keeps them in `attrs.rel_to`, AND
    // `is_decomposable` returns `None` for them, which is exactly
    // the trigger condition S005 cares about. Both categories
    // produce identical runtime semantics; only the `{state}` text
    // in the diagnostic differs (covered by `s005_state_text_for_*`).

    #[test]
    fn s005_suggests_when_uncertain_drops_and_banner_has_no_rel_to() {
        // Two portions; RSMA appears in only one. Atom-semantics
        // intersection is {USA, GBR}; RSMA dropped. Banner has no
        // REL TO at all (NOFORN supersedes) — active validation
        // context per plan §3.1 → Suggest.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        let s005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S005").collect();
        assert_eq!(s005.len(), 1, "S005 must fire once on RSMA: {diags:?}");
        assert_eq!(
            s005[0].severity,
            marque_rules::Severity::Suggest,
            "banner has no REL TO ⇒ active validation ⇒ Suggest, got {:?}",
            s005[0].severity,
        );
        assert!(s005[0].fix.is_none(), "S005 emits no fix");
        assert!(
            s005[0].message.contains("RSMA"),
            "S005 message must name the uncertain code: {:?}",
            s005[0].message
        );
        assert!(
            s005[0].message.contains("AUS"),
            "S005 message must list 'other codes' that AUS could have entered \
             through RSMA's hypothetical membership: {:?}",
            s005[0].message
        );
    }

    #[test]
    fn s006_info_when_banner_equals_atom_intersection() {
        // Banner carries exactly the atom-semantics intersection.
        // expected = {USA, GBR}; banner_atomic = {USA, GBR}.
        // expected ⊆ banner ⇒ Info branch ⇒ S006 (not S005). S005
        // stays silent on this fixture; the engine-level severity
        // override flattening means S005 cannot also emit at Info.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//REL TO USA, GBR";
        let diags = lint_banner(source);
        let s005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S005").collect();
        let s006: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S006").collect();
        assert!(
            s005.is_empty(),
            "S005 must NOT fire when banner is consistent (S006 covers Info): {s005:?}"
        );
        assert_eq!(s006.len(), 1, "S006 must fire once: {diags:?}");
        assert_eq!(
            s006[0].severity,
            marque_rules::Severity::Info,
            "expected ⊆ banner ⇒ Info, got {:?}",
            s006[0].severity,
        );
        assert!(s006[0].fix.is_none(), "S006 emits no fix");
    }

    #[test]
    fn s006_info_when_banner_is_proper_superset_of_atom_intersection() {
        // Banner extends atom-semantics with FRA. The plan's
        // consistency check is `expected ⊆ banner`, not equality —
        // the operator may legitimately have membership data we
        // don't (Constitution VIII forbids invention of facts). FRA
        // pulled from outside is honored as Info (S006), not S005.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//REL TO USA, FRA, GBR";
        let diags = lint_banner(source);
        let s005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S005").collect();
        let s006: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S006").collect();
        assert!(s005.is_empty(), "S005 must NOT fire: {s005:?}");
        assert_eq!(s006.len(), 1, "S006 must fire: {diags:?}");
        assert_eq!(
            s006[0].severity,
            marque_rules::Severity::Info,
            "banner ⊇ expected (extras allowed) ⇒ Info"
        );
    }

    #[test]
    fn s005_suggests_when_banner_drops_a_code_atom_semantics_preserves() {
        // Banner is missing GBR which atom-semantics says must
        // survive. expected = {USA, GBR}; banner_atomic = {USA}.
        // expected ⊄ banner ⇒ Suggest — the safe default isn't met.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//REL TO USA";
        let diags = lint_banner(source);
        let s005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S005").collect();
        assert_eq!(s005.len(), 1, "S005 must fire: {diags:?}");
        assert_eq!(
            s005[0].severity,
            marque_rules::Severity::Suggest,
            "banner drops GBR ⇒ inconsistent ⇒ Suggest"
        );
    }

    /// Helper: count diagnostics for either rule of the
    /// S005/S006 pair (they share the trigger condition; only one
    /// of the two emits per banner candidate).
    fn count_s005_or_s006(diags: &[Diagnostic<CapcoScheme>]) -> usize {
        diags
            .iter()
            .filter(|d| matches!(d.rule.as_str(), "S005" | "S006"))
            .count()
    }

    #[test]
    fn s005_does_not_fire_when_uncertain_code_in_every_portion() {
        // RSMA in BOTH portions ⇒ survives atom-semantics
        // intersection. The atom result reflects RSMA's presence;
        // neither S005 nor S006 has anything to surface.
        let source = "(S//REL TO USA, RSMA)\n\
                      (S//REL TO USA, RSMA)\n\
                      SECRET//REL TO USA";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire when uncertain code survives intersection: {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_for_atom_by_authority_kfor() {
        // KFOR is `decomposable=No` — atom by authority.
        // `is_decomposable("KFOR") == Some(false)`, so the rule's
        // `is_none()` filter excludes it. Atom-semantics is the
        // correct answer: the code IS the recipient.
        let source = "(S//REL TO USA, GBR, KFOR)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire on KFOR (decomposable=No): {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_for_atom_by_authority_eu() {
        // EU is the 2-letter atom-by-authority special case. Same
        // logic as KFOR — `is_decomposable("EU") == Some(false)`,
        // filtered by the `is_none()` gate.
        let source = "(S//REL TO USA, GBR, EU)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire on EU (decomposable=No): {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_for_decomposable_known_fvey() {
        // FVEY is `decomposable=Yes` — atom-semantics expands to
        // {AUS, CAN, GBR, NZL, USA} before intersection. Both
        // portions get the same expanded set; intersection is
        // precise; no uncertainty to surface.
        let source = "(S//REL TO USA, FVEY)\n\
                      (S//REL TO USA, FVEY)\n\
                      SECRET//REL TO USA, FVEY";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire on FVEY (decomposable=Yes): {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_for_single_rel_to_portion() {
        // Only one portion has a non-empty REL TO list. No
        // intersection to compute; rule bails out at the
        // `portions_with_rel_to.len() < 2` guard.
        let source = "(S//REL TO USA, RSMA)\n\
                      (S//FOUO)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire with fewer than 2 REL TO portions: {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_only_trigraphs_appear() {
        // Pure trigraph portions. The trigraph filter (`s.len() ==
        // 3`) excludes every code; uncertain_codes is empty;
        // diagnostic suppressed. ISO 3166-1 alpha-3 codes are atomic
        // by convention, not uncertain.
        let source = "(S//REL TO USA, GBR)\n\
                      (S//REL TO USA, AUS)\n\
                      SECRET//REL TO USA";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire on pure-trigraph fixtures \
             (trigraph filter): {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_other_codes_set_is_empty() {
        // RSMA dropped, but every surviving atom IS in expected.
        // `other_codes` is empty — there's nothing the operator
        // might have intended to release to through RSMA's
        // hypothetical membership. Suppress.
        let source = "(S//REL TO USA, RSMA)\n\
                      (S//REL TO USA)\n\
                      SECRET//REL TO USA";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must suppress when no 'other codes' to surface: {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_non_ic_split_injects_nf() {
        // The non-IC SBU-NF/LES-NF split forces NF injection at
        // banner roll-up in classified documents (CAPCO-2016
        // §H.9). When that split fires,
        // `PageContext::expected_rel_to` returns empty even though
        // no portion carries `DissemControl::Nf` directly — REL TO
        // is superseded at the page level. Pin the second NOFORN
        // bail in `analyze_uncertain_reduction` (the
        // `needs_nf_from_split` branch).
        //
        // Fixture: portion 1 has SBU-NF (the split trigger);
        // portions 2 and 3 have classified REL TO with an uncertain
        // code (RSMA). Without the bail, the rule would compute
        // `portions_with_rel_to.len() == 2`, `expected_set = {}`
        // (NF-injection supersession), and fire a misleading
        // "intersection produced REL TO (empty…)" diagnostic.
        let source = "(S//SBU-NF)\n\
                      (S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN//SBU";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must bail when non-IC SBU-NF split forces NF \
             injection at banner roll-up: {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_a_portion_carries_noforn() {
        // Regression for Copilot review on PR #249: NOFORN supersedes
        // REL TO at the page level. `PageContext::expected_rel_to`
        // returns empty because the marking is superseded, not
        // because the atom intersection is empty — firing S005 in
        // that case produces a misleading "intersection produced
        // REL TO (empty…)" diagnostic. Pin the bail.
        //
        // Fixture: portion 1 has NOFORN, portions 2+3 have REL TO
        // with an uncertain code (RSMA). Pre-fix, the rule would
        // have computed `portions_with_rel_to.len() == 2`,
        // `expected_set = {}` (NOFORN supersession), and fired
        // S005 with empty-intersection wording. Post-fix, the
        // NOFORN check bails before any of that runs.
        let source = "(S//NF)\n\
                      (S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire when any portion carries NOFORN \
             (REL TO is superseded at the page level): {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_other_codes_only_appear_alongside_x() {
        // Regression for Copilot review on PR #249: the previous
        // `union − expected − {X}` definition included atoms that
        // appeared only in the same portion as X. Such atoms can't
        // be hypothetically pulled in via X's membership — they're
        // already explicitly listed in the X-containing portion, so
        // their intersection survival depends on whether they also
        // appear in the OTHER portions, not on X's membership.
        //
        // Here GBR appears only alongside RSMA (in portion 1).
        // Portion 2 has only USA. atom-semantics intersection =
        // {USA}. RSMA dropped, but no atom in portions-without-X
        // (= {USA} only) is missing from expected. The rule must
        // stay silent. The pre-fix implementation would have
        // computed `other_codes = {USA, GBR, RSMA} − {USA} − {RSMA}
        // = {GBR}` and fired a false-positive Info diagnostic.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA)\n\
                      SECRET//REL TO USA";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire when 'other codes' only appear \
             alongside X (post-Copilot-review fix): {diags:?}"
        );
    }

    #[test]
    fn s005_quotes_verbatim_taxonomy_description_for_na_description_codes() {
        // EUDA is `decomposable=NA` with `<Membership><Description>`
        // in V2022-NOV. The taxonomy carries verbatim ODNI text
        // ("As of 15 March 2016, disclosure request should be
        // referred to the original classification authority...").
        // Plan §3.3 requires that text to surface verbatim in the
        // diagnostic — Constitution V audit-content-ignorance is
        // satisfied because the text is ODNI taxonomy data, not
        // user-document content.
        let source = "(S//REL TO USA, GBR, EUDA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        let s005 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S005")
            .unwrap_or_else(|| panic!("S005 must fire on EUDA: {diags:?}"));
        assert!(
            s005.message.contains("disclosure request"),
            "S005 must quote verbatim Description text for NA-Description codes; got: {:?}",
            s005.message
        );
        assert!(
            s005.message.contains("original classification authority"),
            "S005 must include the OCA-deferral phrase ODNI published: {:?}",
            s005.message
        );
    }

    #[test]
    fn s005_state_text_for_na_suppressed_code() {
        let text = super::s005_state_text("RSMA");
        assert!(
            text.contains("deprecated") && text.contains("suppressed"),
            "RSMA is NA-Suppressed; state text must say so: {text:?}"
        );
    }

    #[test]
    fn s005_state_text_for_na_description_code() {
        let text = super::s005_state_text("EUDA");
        assert!(
            text.contains("deprecated"),
            "EUDA is NA; state text must mark it deprecated: {text:?}"
        );
        assert!(
            text.contains("original classification authority"),
            "EUDA Description text must reach state output: {text:?}"
        );
    }

    #[test]
    fn s005_state_text_for_recursive_code() {
        let text = super::s005_state_text("BHTF");
        assert!(
            text.contains("recursive") || text.contains("out of scope"),
            "BHTF is NA-Members(recursive): {text:?}"
        );
    }

    #[test]
    fn s005_state_text_for_unknown_code() {
        // Code absent from V2022-NOV taxonomy entirely — represents
        // org-fork extensions or genuinely unknown codes.
        let text = super::s005_state_text("XYZW");
        assert!(
            text.contains("absent"),
            "unknown-code state text must mention absence: {text:?}"
        );
    }

    #[test]
    fn s005_state_text_decomposable_yes_hits_defensive_fallback() {
        // FVEY is `decomposable="Yes"` / `membership_shape="Members"`
        // in V2022-NOV. The rule's outer `is_decomposable == None`
        // guard means the state-text helper is never called with
        // FVEY in production (S005's loop filters such codes out
        // before formatting), but the function is callable
        // directly and its catch-all arm `(decomp, shape) =>
        // format!(…)` is the defensive fallback if a future
        // taxonomy revision introduces a `(non-NA, *)` reachable
        // shape. Pin the fallback's format so the contract is
        // documented behavior.
        let text = super::s005_state_text("FVEY");
        assert!(
            text.contains("decomposable=\"Yes\""),
            "fallback must surface decomposable verbatim: {text:?}"
        );
        assert!(
            text.contains("membership_shape=\"Members\""),
            "fallback must surface membership_shape verbatim: {text:?}"
        );
        assert!(
            text.contains("ISMCAT V"),
            "fallback includes the ISMCAT_TETRA_VERSION preamble: {text:?}"
        );
    }

    #[test]
    fn s005_state_text_decomposable_no_hits_defensive_fallback() {
        // EU is `decomposable="No"` (atom by authority) in V2022-NOV.
        // Same defensive-fallback contract as the Yes case.
        let text = super::s005_state_text("EU");
        assert!(
            text.contains("decomposable=\"No\""),
            "fallback for No: {text:?}"
        );
        assert!(
            text.contains("membership_shape=\"Suppressed\""),
            "fallback for No (Suppressed shape): {text:?}"
        );
    }

    #[test]
    fn s005_handles_empty_atom_intersection() {
        // Disjoint REL TO portions ⇒ atom-semantics intersection
        // is empty (no shared codes), but the rule should still
        // surface the silent-loss case if uncertain codes drop and
        // there are other-portion atoms that would have been
        // pulled in by hypothetical membership. Pins the
        // empty-set arm of `expected_str` rendering
        // (`"(empty — atom intersection produced no shared codes)"`).
        //
        // Fixture is intentionally malformed (REL TO without USA
        // per §H.8) — that's the only way to land an empty atom
        // intersection in well-formed input. E002
        // (missing-USA-trigraph) will also fire on both portions;
        // its diagnostic is independent of S005's.
        let source = "(S//REL TO GBR, RSMA)\n\
                      (S//REL TO AUS)\n\
                      SECRET//NF";
        let diags = lint_banner(source);
        let s005 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S005")
            .unwrap_or_else(|| {
                panic!("S005 must fire on empty-intersection RSMA fixture: {diags:?}")
            });
        assert!(
            s005.message.contains("(empty"),
            "expected empty-intersection wording in S005 message: {:?}",
            s005.message
        );
    }

    #[test]
    fn s005_multi_portion_uses_intersection_across_portions_without_x() {
        // Three portions: portion 1 carries X=RSMA; portions 2 and
        // 3 don't. `atoms_in_every_without_x` is the intersection of
        // p2's expansion = {USA, GBR} and p3's expansion = {USA, GBR}
        // = {USA, GBR}. After subtracting expected={USA} and {RSMA},
        // `other_codes = {GBR}` — non-empty, S005 fires. This
        // exercises the `for p in &portions_without_x[1..]` loop
        // body that the two-portion fixtures don't reach.
        let source = "(S//REL TO USA, RSMA)\n\
                      (S//REL TO USA, GBR)\n\
                      (S//REL TO USA, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        let s005 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S005")
            .unwrap_or_else(|| panic!("S005 must fire on 3-portion RSMA fixture: {diags:?}"));
        assert!(
            s005.message.contains("GBR"),
            "S005 must surface GBR (intersect({{USA, GBR}}, {{USA, GBR}}) \
             − {{USA}} − {{RSMA}} = {{GBR}}): {:?}",
            s005.message
        );
        assert!(
            !s005.message.contains("RSMA, GBR") && !s005.message.contains("AUS"),
            "the two non-X portions are identical; only GBR should \
             reach other_codes: {:?}",
            s005.message
        );
    }

    #[test]
    fn s005_does_not_fire_when_portions_without_x_have_disjoint_atoms() {
        // Three portions: p1 has X=RSMA, p2 has GBR but not AUS,
        // p3 has AUS but not GBR. atoms_in_every_without_x =
        // intersect({USA, GBR}, {USA, AUS}) = {USA}. After
        // subtracting expected={USA} and {RSMA}, other_codes = {}.
        // The rule must stay silent — even hypothetically including
        // GBR or AUS in RSMA's membership wouldn't make either
        // survive intersection (the OTHER non-X portion lacks them).
        // This pins the intersection-vs-union semantics: a union
        // implementation would have produced other_codes={GBR, AUS}
        // and fired a false positive.
        let source = "(S//REL TO USA, RSMA)\n\
                      (S//REL TO USA, GBR)\n\
                      (S//REL TO USA, AUS)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire when portions-without-X have \
             disjoint atoms outside expected (intersection wipes \
             them): {diags:?}"
        );
    }

    #[test]
    fn s005_rule_trait_getters() {
        // Cover the `id` / `name` / `default_severity` accessors that
        // the inline-test harness's direct `rule.check()` calls
        // bypass. Engine-level tests exercise these too, but pinning
        // the contract here keeps the regression closer to the
        // implementation.
        let rule = super::RelToOpaqueUncertainReductionSuggestRule;
        assert_eq!(<_ as Rule<CapcoScheme>>::id(&rule).as_str(), "S005");
        assert_eq!(
            <_ as Rule<CapcoScheme>>::name(&rule),
            "rel-to-opaque-uncertain-reduction"
        );
        assert_eq!(
            <_ as Rule<CapcoScheme>>::default_severity(&rule),
            marque_rules::Severity::Suggest
        );
    }

    #[test]
    fn s006_rule_trait_getters() {
        let rule = super::RelToOpaqueUncertainReductionInfoRule;
        assert_eq!(<_ as Rule<CapcoScheme>>::id(&rule).as_str(), "S006");
        assert_eq!(
            <_ as Rule<CapcoScheme>>::name(&rule),
            "rel-to-opaque-uncertain-reduction-info"
        );
        assert_eq!(
            <_ as Rule<CapcoScheme>>::default_severity(&rule),
            marque_rules::Severity::Info
        );
    }

    #[test]
    fn s005_helpers_render_set_promotes_usa_and_alphabetizes_rest() {
        // `s005_render_set` produces the comma-separated string
        // S005/S006 messages embed for `expected_str` and
        // `other_str`. USA goes first; the rest alpha. Pin the
        // contract directly because the integration tests only
        // observe it through the diagnostic message wording.
        use std::collections::BTreeSet;
        let set: BTreeSet<&str> = ["GBR", "AUS", "USA", "FRA"].into_iter().collect();
        let rendered = super::s005_render_set(&set);
        assert_eq!(rendered, "USA, AUS, FRA, GBR");

        // No USA — pure alphabetical (BTreeSet already sorts the
        // input, so the join order matches insertion order).
        let no_usa: BTreeSet<&str> = ["GBR", "AUS", "FRA"].into_iter().collect();
        assert_eq!(super::s005_render_set(&no_usa), "AUS, FRA, GBR");

        // Empty set → empty string. The rule guards against this
        // path via the `expected_set.is_empty()` branch but pinning
        // the helper's behavior keeps the contract honest.
        let empty: BTreeSet<&str> = BTreeSet::new();
        assert_eq!(super::s005_render_set(&empty), "");
    }

    #[test]
    fn s005_helpers_expand_atomic_round_trips_through_tetragraph() {
        // `s005_expand_atomic` is the rule's view of "what trigraphs
        // does this REL TO list cover after tetragraph expansion?"
        // FVEY decomposes; opaque codes (RSMA, KFOR) and trigraphs
        // pass through unchanged. Direct unit test because the
        // integration tests don't observe the function's output
        // shape, only the downstream diagnostic.
        use marque_ism::CountryCode;
        use std::collections::BTreeSet;

        let rel_to: Vec<CountryCode> = ["USA", "FVEY"]
            .into_iter()
            .map(|s| CountryCode::try_new(s.as_bytes()).unwrap())
            .collect();
        let expanded = super::s005_expand_atomic(&rel_to);
        let expected: BTreeSet<&str> = ["USA", "AUS", "CAN", "GBR", "NZL"].into_iter().collect();
        assert_eq!(
            expanded, expected,
            "FVEY must expand to its 5 trigraph members + USA passthrough"
        );

        // Opaque tetragraph (RSMA NA-Suppressed) and trigraphs pass
        // through.
        let opaque: Vec<CountryCode> = ["USA", "RSMA"]
            .into_iter()
            .map(|s| CountryCode::try_new(s.as_bytes()).unwrap())
            .collect();
        let expanded_opaque = super::s005_expand_atomic(&opaque);
        let expected_opaque: BTreeSet<&str> = ["USA", "RSMA"].into_iter().collect();
        assert_eq!(expanded_opaque, expected_opaque);
    }

    #[test]
    fn s005_audit_content_ignorance_no_user_content_in_message() {
        // Constitution V: the diagnostic message must reference only
        // canonical token strings (the tetragraph, the trigraphs in
        // expected/other_codes, and verbatim taxonomy data) — never
        // surrounding source bytes. Pin the contract by feeding a
        // fixture whose surrounding text would be obviously visible
        // if leaked. Banner has no REL TO so this is the active-
        // validation Suggest case ⇒ S005 fires (not S006).
        let source = "Document subject: \"Operation Confidential\"\n\
                      (S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        let s005 = diags
            .iter()
            .find(|d| matches!(d.rule.as_str(), "S005" | "S006"))
            .expect("S005 or S006 must fire on RSMA fixture");
        assert!(
            !s005.message.contains("Operation Confidential"),
            "S005/S006 message must not leak surrounding document text: {:?}",
            s005.message
        );
        assert!(
            !s005.message.contains("Document subject"),
            "S005/S006 message must not leak surrounding document text: {:?}",
            s005.message
        );
    }

    // --- E010: Bare HCS rule ---

    #[test]
    fn e010_fires_on_bare_hcs_in_banner() {
        let diags = lint_banner("TOP SECRET//HCS//NOFORN");
        let e010: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E010").collect();
        assert_eq!(e010.len(), 1);
        let src = b"TOP SECRET//HCS//NOFORN";
        assert_eq!(e010[0].span.as_str(src).unwrap(), "HCS");
        let fix = e010[0].fix.as_ref().expect("E010 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "HCS-P");
        assert!((fix.confidence.combined() - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn e010_fires_on_bare_hcs_in_portion() {
        let diags = lint_portion("(TS//HCS//NF)");
        let e010: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E010").collect();
        assert_eq!(e010.len(), 1);
        let fix = e010[0].fix.as_ref().expect("E010 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "HCS-P");
    }

    #[test]
    fn e010_does_not_fire_on_hcs_p() {
        let diags = lint_banner("TOP SECRET//HCS-P//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E010"),
            "E010 must not fire on HCS-P, got: {diags:?}"
        );
    }

    #[test]
    fn e010_does_not_fire_on_hcs_o() {
        let diags = lint_banner("TOP SECRET//HCS-O//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E010"),
            "E010 must not fire on HCS-O, got: {diags:?}"
        );
    }

    #[test]
    fn e010_lowers_confidence_when_hcs_o_present() {
        // If HCS-O appears alongside bare HCS, the suggestion is ambiguous.
        let diags = lint_banner("TOP SECRET//HCS//HCS-O//NOFORN");
        let e010: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E010").collect();
        assert_eq!(e010.len(), 1);
        let fix = e010[0].fix.as_ref().unwrap();
        assert!(
            (fix.confidence.combined() - 0.5).abs() < f32::EPSILON,
            "confidence should be 0.5 when HCS-O is present, got {}",
            fix.confidence.combined()
        );
    }

    // --- E012: Dual classification ---

    #[test]
    fn e012_fires_on_us_plus_nato() {
        let diags = lint_banner("SECRET//NATO SECRET//NOFORN");
        let e012: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E012").collect();
        assert_eq!(e012.len(), 1);
        assert!(e012[0].message.contains("US") && e012[0].message.contains("NATO"));
        // Pin the citation field to the catalog-matched authoritative
        // passage. Drift back to the legacy `§B.1` umbrella reference
        // would be caught here; structural citation-lint (which
        // accepts both `§B.1` and `§H.3 p55` as well-formed)
        // would not flag the regression.
        assert_eq!(e012[0].citation, "CAPCO-2016 §H.3 p55");
    }

    #[test]
    fn e012_does_not_fire_on_us_only() {
        let diags = lint_banner("SECRET//NOFORN");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E012"));
    }

    #[test]
    fn e012_does_not_fire_on_nato_only() {
        let diags = lint_banner("//NATO SECRET//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E012"),
            "E012 should not fire on pure NATO, got: {:?}",
            diags
                .iter()
                .filter(|d| d.rule.as_str() == "E012")
                .collect::<Vec<_>>()
        );
    }

    // --- W002: Comingling warning ---

    #[test]
    fn w002_fires_on_us_plus_fgi_in_portion() {
        let diags = lint_portion("(S//FGI DEU//REL TO USA, DEU)");
        let w002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W002").collect();
        assert_eq!(w002.len(), 1);
    }

    #[test]
    fn w002_does_not_fire_on_banner() {
        // Comingling warning is portion-only.
        let diags = lint_banner("SECRET//FGI DEU//NOFORN");
        assert!(diags.iter().all(|d| d.rule.as_str() != "W002"));
    }

    #[test]
    fn w002_does_not_fire_without_fgi_marker() {
        let diags = lint_portion("(S//NF)");
        assert!(diags.iter().all(|d| d.rule.as_str() != "W002"));
    }

    // --- E014: JOINT participants missing from REL TO ---

    #[test]
    fn e014_fires_when_joint_country_missing_from_rel_to() {
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA");
        let e014: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E014").collect();
        assert_eq!(e014.len(), 1);
        assert!(e014[0].message.contains("GBR"));
    }

    #[test]
    fn e014_does_not_fire_when_all_present() {
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E014"),
            "E014 should not fire when all JOINT countries in REL TO, got: {:?}",
            diags
                .iter()
                .filter(|d| d.rule.as_str() == "E014")
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn e014_does_not_fire_when_joint_country_covered_by_fvey_tetragraph() {
        // GBR is a FVEY member; REL TO USA, FVEY implicitly covers GBR.
        // §H.8 p145 defines tetragraphs as collective references to their
        // constituent trigraphs.
        let diags = lint_banner("//JOINT S GBR USA//REL TO USA, FVEY");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E014"),
            "E014 must not fire when JOINT country is covered by FVEY: {diags:?}"
        );
    }

    #[test]
    fn e014_does_not_fire_when_all_five_eyes_in_joint_covered_by_fvey() {
        // All five FVEY members in JOINT; FVEY alone covers them all.
        let diags = lint_banner("//JOINT S AUS CAN GBR NZL USA//REL TO USA, FVEY");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E014"),
            "E014 must not fire when all JOINT countries covered by FVEY: {diags:?}"
        );
    }

    #[test]
    fn e014_still_fires_when_joint_country_not_covered_by_tetragraph() {
        // DEU is not a FVEY member; REL TO USA, FVEY does not cover DEU.
        let diags = lint_banner("//JOINT S DEU USA//REL TO USA, FVEY");
        let e014: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E014").collect();
        assert_eq!(
            e014.len(),
            1,
            "E014 must still fire when a JOINT country is not in any REL TO tetragraph: {diags:?}"
        );
        assert!(e014[0].message.contains("DEU"));
    }

    // --- E015: Non-US without dissem ---

    #[test]
    fn e015_fires_on_nato_without_dissem() {
        let diags = lint_banner("//NATO SECRET");
        let e015: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E015").collect();
        assert_eq!(e015.len(), 1);
        // Pin the citation to the catalog-matched authoritative pair
        // (§H.7 p122 + §B.3 p20). Regression guard against drift back
        // to the legacy `§B.3`-only umbrella reference; structural
        // citation-lint accepts both forms and would not catch it.
        assert_eq!(e015[0].citation, "CAPCO-2016 §H.7 p122 + §B.3 p20");
    }

    #[test]
    fn e015_does_not_fire_with_rel_to() {
        let diags = lint_banner("//NATO SECRET//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E015"),
            "E015 should not fire when dissem present, got: {:?}",
            diags
                .iter()
                .filter(|d| d.rule.as_str() == "E015")
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn e015_does_not_fire_on_us_classification() {
        let diags = lint_banner("SECRET");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E015"));
    }

    // --- Non-US clean markings produce no unexpected diagnostics ---

    #[test]
    fn clean_nato_portion_no_diagnostics() {
        let diags = lint_portion("(//NS//REL TO USA, GBR)");
        let unexpected: Vec<_> = diags
            .iter()
            .filter(|d| !matches!(d.rule.as_str(), "E002")) // E002 may fire on USA ordering
            .collect();
        assert!(
            unexpected.is_empty(),
            "clean NATO portion should have no unexpected diagnostics, got: {unexpected:?}"
        );
    }

    // --- Non-IC dissem controls ---

    #[test]
    fn non_ic_dissem_parses_in_portion() {
        let diags = lint_portion("(U//DS)");
        // DS = LIMDIS portion form. Should parse without E008 (unknown token).
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E008"),
            "DS should be recognized as non-IC dissem, not unknown: {diags:?}"
        );
    }

    #[test]
    fn non_ic_dissem_les_nf_parses() {
        let diags = lint_portion("(U//LES-NF)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E008"),
            "LES-NF should be recognized: {diags:?}"
        );
    }

    // --- W003: Non-IC dissem in classified banner ---

    #[test]
    fn w003_fires_on_sbu_in_classified_banner() {
        let diags = lint_banner("CONFIDENTIAL//SBU");
        let w003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W003").collect();
        assert_eq!(w003.len(), 1);
        assert!(w003[0].message.contains("SBU"));
    }

    #[test]
    fn w003_does_not_fire_on_unclassified_banner() {
        let diags = lint_banner("UNCLASSIFIED//SBU");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "W003 should not fire on UNCLASSIFIED banner: {diags:?}"
        );
    }

    #[test]
    fn w003_fires_on_limdis_in_classified_banner() {
        // CAPCO-2016 §H.9 p170: "When a document contains LIMDIS
        // and classified portions, LIMDIS is not used in the banner
        // line." Prior impl incorrectly placed LIMDIS in the
        // propagating set on a paraphrased "NGA Title 10" justification;
        // §H.9 is explicit that LIMDIS is stripped from classified
        // banners.
        let diags = lint_banner("SECRET//LIMDIS");
        let w003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W003").collect();
        assert_eq!(
            w003.len(),
            1,
            "W003 must fire on LIMDIS in classified banner (§H.9 p170): {diags:?}"
        );
        assert!(w003[0].message.contains("LIMDIS"));
    }

    #[test]
    fn w003_does_not_fire_on_exdis_in_classified_banner() {
        // CAPCO-2016 §H.9 p172: "If EXDIS is contained in any
        // portion of a document that does not contain one or more NODIS
        // portions, EXDIS must appear in the banner line." Example
        // banner on p173: SECRET//NOFORN//EXDIS. Prior impl excluded
        // EXDIS from the propagating set; the §H.9 rule is the
        // opposite.
        let diags = lint_banner("SECRET//NOFORN//EXDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "EXDIS propagates to classified banners per §H.9 p172: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_nodis_in_classified_banner() {
        // CAPCO-2016 §H.9 p174: "If NODIS is contained in any
        // portion of a document, it must appear in the banner line."
        // Example banner on p174: SECRET//NOFORN//NODIS. Prior impl
        // excluded NODIS from the propagating set; the §H.9 rule is
        // the opposite.
        let diags = lint_banner("SECRET//NOFORN//NODIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "NODIS propagates to classified banners per §H.9 p174: {diags:?}"
        );
    }

    #[test]
    fn w003_fires_on_sbu_nf_in_classified_banner() {
        // CAPCO-2016 §H.9 p178: SBU NOFORN "Applicable only to
        // unclassified information." p179 example 2 shows a
        // `SECRET//NOFORN` banner with a `(U//SBU-NF)` portion — SBU-NF
        // absent from banner. The NOFORN half of SBU-NF *does*
        // propagate via `PageContext::expected_non_ic_dissem` (it
        // splits portion-level SBU-NF into SBU + NF-flag, emitting
        // NOFORN into the classified banner's dissem block). What
        // W003 catches is the literal `SBU NOFORN` *banner* form in a
        // classified document — that surface form is non-canonical
        // per §H.9, independent of whether NOFORN itself propagates.
        let diags = lint_banner("SECRET//SBU NOFORN");
        let w003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W003").collect();
        assert_eq!(
            w003.len(),
            1,
            "W003 must fire on literal SBU-NF in classified banner (§H.9 p178): {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_les_in_classified_banner() {
        // CAPCO-2016 §H.9 p181: "The LES marking always appears in
        // the banner line if contained in any portion, regardless of
        // classification level." Example banners on p183: SECRET//REL
        // TO USA, FVEY//LES, SECRET//NOFORN//LES.
        let diags = lint_banner("SECRET//LES");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "LES propagates to classified banners per §H.9 p181: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_les_nf_in_classified_banner() {
        // CAPCO-2016 §H.9 p185: "The LES marking always appears
        // in the banner line if LES information (either LES or LES
        // NOFORN) is contained in the document, regardless of the
        // document's classification level." The §H.9 canonical form
        // in classified docs is "LES" at banner with NOFORN split into
        // the dissem block (§H.9 p185), but `LES NOFORN` in a
        // classified banner is not a W003 concern — the canonicalization
        // is a separate page-rewrite concern.
        let diags = lint_banner("SECRET//LES NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "LES-NF propagates to classified banners per §H.9 p185: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_ssi_in_classified_banner() {
        // CAPCO-2016 §H.9 p189: "If the SSI marking is contained
        // in any portion of a document it must appear in the banner
        // line, regardless of the document's overall classification
        // level." Example banner on p191: SECRET//REL TO USA,
        // ACGU//SSI.
        let diags = lint_banner("SECRET//SSI");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "SSI propagates to classified banners per §H.9 p189: {diags:?}"
        );
    }

    #[test]
    fn w003_fires_on_sbu_in_nato_classified_banner() {
        // Non-US (NATO) classified banners are still classified — W003 should fire.
        let diags = lint_banner("//NS//SBU");
        let w003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W003").collect();
        assert_eq!(
            w003.len(),
            1,
            "W003 must fire on SBU in NATO classified banner: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_portion() {
        let diags = lint_portion("(C//DS)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "W003 is banner-only: {diags:?}"
        );
    }

    #[test]
    fn non_ic_dissem_correct_classified_doc() {
        let diags = lint_banner("CONFIDENTIAL//NOFORN");
        assert!(
            diags.is_empty(),
            "clean classified banner should have no diagnostics: {diags:?}"
        );
        let diags = lint_portion("(U//DS)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "non-IC dissem in portion should not fire W003: {diags:?}"
        );
    }

    // --- E016: RESTRICTED not allowed with JOINT ---

    #[test]
    fn e016_fires_on_joint_restricted() {
        let diags = lint_banner("//JOINT R USA GBR//REL TO USA, GBR");
        let e016: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E016").collect();
        assert_eq!(e016.len(), 1);
        assert!(e016[0].message.contains("RESTRICTED"));
        // PR 3c.B Sub-PR 8.B — message must surface the operational Five
        // Eyes equivalence hint so users know how to re-mark the violating
        // text manually. Wording stays context-neutral because the rule's
        // `check` does not consult `RuleContext` and can fire on either a
        // portion or a banner (the test input here is a banner). The hint
        // is framed as "per Five Eyes practice" — NOT as a §H.3 claim —
        // because the equivalence lives in CAPCO-2016 Appendix A §4 (Five
        // Eyes Marking Comparisons), not in §H.3. See the module-level
        // comment on `DeclarativeJointRestrictedRule` in
        // `rules_declarative.rs` and the followup at
        // `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`.
        assert!(
            e016[0].message.contains("CONFIDENTIAL"),
            "E016 message must surface the operational equivalence hint \
             (RESTRICTED → CONFIDENTIAL per Five Eyes practice) so the \
             user knows how to re-mark; got: {:?}",
            e016[0].message
        );
        assert!(
            e016[0].message.contains("Five Eyes"),
            "E016 message must frame the equivalence as Five Eyes practice \
             (NOT as a §H.3 claim — Constitution VIII citation fidelity); \
             got: {:?}",
            e016[0].message
        );
        // PR 3c.B Sub-PR 8.B — citation pin (D13 single-citation discipline).
        assert_eq!(e016[0].citation, "CAPCO-2016 §H.3 p56");
    }

    /// PR 3c.B Sub-PR 8.B — pin the consciously-decided-no-fix-intent
    /// migration state for E016.
    ///
    /// Per the 2026-05-11 lattice-consultant session captured in
    /// `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`,
    /// E016 is **Category A.3 — Transmute via foreign-equivalence map**:
    /// the eventual Stage-4 target is `Remove(RESTRICTED) ⊕ Add(CONFIDENTIAL)`
    /// emitted as one atomic audit repair, driven by a foreign-equivalence
    /// vocabulary table. That vocabulary table does not exist in
    /// `marque-capco::vocab` today and its source is open (see the
    /// followup file's Open Question 1 — candidates include CAPCO-2016
    /// Appendix A §4 / Five Eyes Marking Comparisons, currently not
    /// vendored). Until the source is resolved, the rule emits a
    /// diagnostic with both `fix.is_none()` AND `fix_intent.is_none()`.
    ///
    /// **Do not** dual-populate this rule with a single-fact
    /// `FactRemove(RESTRICTED, Portion)` intent in the interim — that
    /// would land a half-fix (leaving the marking without a
    /// classification level) and corrupt the audit log under
    /// Constitution V.
    ///
    /// **Coverage note:** the G13 closure walker at
    /// `tests/g13_closure_fix_intent.rs::all_migrated_rule_intents_pass_g13_envelope_walker`
    /// only inspects rules that auto-apply through the engine (those
    /// emitting `AppliedFixProposal::New` records, which require
    /// `fix_intent.is_some()`). E016 with `fix_intent: None` is never
    /// reached by that walker — so this symmetry pin is the **only**
    /// guard against a future commit accidentally producing an
    /// asymmetric `(fix, fix_intent)` pair on E016. Without it, a drift
    /// toward `fix.is_some() && fix_intent.is_none()` (or the inverse)
    /// would slip through CI silently.
    #[test]
    fn e016_emits_no_fix_and_no_fix_intent_pending_stage4_a3_transmute() {
        let diags = lint_banner("//JOINT R USA GBR//REL TO USA, GBR");
        let e016 = diags
            .iter()
            .find(|d| d.rule.as_str() == "E016")
            .expect("E016 must fire on `//JOINT R USA GBR//REL TO USA, GBR`");
        assert!(
            e016.fix.is_none(),
            "E016 fix must be None until Stage-4 A.3 consolidation lands; \
             see incompatibility-primitive-consolidation.md followup"
        );
        assert!(
            e016.fix_intent.is_none(),
            "E016 fix_intent must be None (symmetric with fix.is_none(). \
             The G13 walker does NOT see (None, None) rules; this test is \
             the only guard against asymmetric drift)"
        );
    }

    #[test]
    fn e016_does_not_fire_on_joint_secret() {
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E016"));
    }

    // --- E017/E018/E019 retirement regressions (T035b) ---
    //
    // These tests pin the retirement: markings that the legacy
    // rules wrongly flagged must NOT emit those rule IDs after
    // T035b. CAPCO §H.3 p57 permits JOINT with IC and non-IC
    // dissem (excluding only NOFORN and HCS per §H.3 p57) and with
    // FGI (cross-ref §H.7). Any reintroduction of E017/E018/E019
    // diagnostics would regress CAPCO-2016 fidelity.

    #[test]
    fn e017_does_not_fire_on_joint_rel_to_banner() {
        // Generic retirement check: E017 (JOINT + FGI marker) is
        // retired — the rule ID must never appear on the diagnostic
        // stream regardless of input. This test uses a plain
        // JOINT+REL TO banner, which does NOT exercise an FGI-marker
        // path (the parser's banner grammar does not surface
        // `fgi_marker` on a JOINT classification). True FGI-marker
        // coverage requires constructing `CanonicalAttrs` directly;
        // that's covered at the scheme level in
        // `scheme_equivalence.rs::no_legacy_e017_e018_e019_constraints_in_catalog`.
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E017"),
            "E017 retired; must never fire: {diags:?}"
        );
    }

    #[test]
    fn e018_does_not_fire_on_joint_with_noforn() {
        // Pre-T035b: E018 flagged JOINT + NOFORN as "IC dissem other
        // than REL TO". CAPCO §H.3 p57 does exclude NOFORN
        // from JOINT, but that's caught indirectly via
        // `capco/noforn-conflicts-rel-to` + E014 (REL TO required).
        // E018 itself must not fire.
        let diags = lint_banner("//JOINT S USA GBR//NF");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E018"),
            "E018 retired; must never fire: {diags:?}"
        );
    }

    #[test]
    fn e018_does_not_fire_on_joint_with_rel_to_only() {
        // Still holds post-retirement — plain `//JOINT S USA GBR//
        // REL TO USA, GBR` is the canonical valid JOINT form.
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E018"),
            "E018 retired; must never fire: {diags:?}"
        );
    }

    #[test]
    fn e019_does_not_fire_on_joint_with_limdis() {
        // Pre-T035b: E019 flagged JOINT + LIMDIS as "JOINT + non-IC
        // dissem". CAPCO §H.3 p57 explicitly permits non-IC
        // dissem with JOINT "as appropriate". Retired entirely.
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR//LIMDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E019"),
            "E019 retired; must never fire: {diags:?}"
        );
    }

    // --- E036: JOINT + HCS markings (T035b replacement) ---

    #[test]
    fn legacy_joint_hcs_rules_do_not_fire_on_parser_path() {
        // §H.3 p57: "May not be used with the HCS markings".
        // This parser-driven test does not reliably provide positive
        // E036 coverage because the grammar may not surface HCS in
        // a JOINT banner at this point. What it *does* verify is
        // that the retired legacy JOINT rules (E017/E018/E019)
        // never appear on this input path. Positive E036 coverage
        // lives in scheme-level tests
        // (`scheme_equivalence::e036_fires_on_joint_with_bare_hcs` /
        // `_with_hcs_p`) where attrs can be constructed directly.
        let diags = lint_banner("//JOINT S USA GBR//HCS-P//REL TO USA, GBR");
        assert!(
            diags
                .iter()
                .all(|d| !matches!(d.rule.as_str(), "E017" | "E018" | "E019")),
            "legacy E017/E018/E019 must not fire post-T035b: {diags:?}"
        );
    }

    /// PR 3c.B Sub-PR 8.B — pin the consciously-decided-no-fix-intent
    /// migration state for E036.
    ///
    /// Per the 2026-05-11 lattice-consultant session captured in
    /// `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`,
    /// E036 is **Category B — genuine mutual exclusion without policy
    /// decision**: the eventual Stage-4 target is `Reject { suggest:
    /// Some(...) }` — error diagnostic with an optional
    /// `Severity::Suggest` companion ("did you mean
    /// `SECRET//HCS-P//REL TO [LIST]`?"). No auto-applied fix exists for
    /// this combination — JOINT changes attribution semantics; HCS is
    /// CIA-owned and US-only; the marking shape is contradictory in a
    /// way no removal can resolve.
    ///
    /// JOINT+HCS is academic in practice (JOINT classifications are
    /// largely DOD-only; HCS is CIA-only; the agencies' marking
    /// vocabularies don't overlap on this axis), so the diagnostic-only
    /// landing is functionally sufficient.
    ///
    /// **Parser-gap note:** the existing test
    /// `legacy_joint_hcs_rules_do_not_fire_on_parser_path` above
    /// documents that the engine pipeline (`lint_banner`) does not
    /// reliably surface E036 because the parser may not emit `TOK_HCS`
    /// inside a JOINT banner. This symmetry pin therefore constructs
    /// `CanonicalAttrs` programmatically and calls
    /// `DeclarativeJointHcsRule.check()` directly — at the Rule-emission
    /// layer (Diagnostic), one layer above the scheme-validation
    /// (ConstraintViolation) layer covered by
    /// `tests/scheme_equivalence.rs::e036_fires_on_joint_with_bare_hcs`.
    /// The `(fix, fix_intent)` symmetry is a Diagnostic-shape invariant
    /// that scheme-level tests cannot pin.
    ///
    /// **Coverage note:** the G13 closure walker at
    /// `tests/g13_closure_fix_intent.rs::all_migrated_rule_intents_pass_g13_envelope_walker`
    /// only inspects rules that auto-apply through the engine (those
    /// emitting `AppliedFixProposal::New` records, which require
    /// `fix_intent.is_some()`). E036 with `fix_intent: None` is never
    /// reached by that walker — so this symmetry pin is the **only**
    /// guard against a future commit accidentally producing an
    /// asymmetric `(fix, fix_intent)` pair on E036. Without it, a drift
    /// toward `fix.is_some() && fix_intent.is_none()` (or the inverse)
    /// would slip through CI silently.
    #[test]
    fn e036_emits_no_fix_and_no_fix_intent_pending_stage4_b_reject() {
        use crate::rules_declarative::DeclarativeJointHcsRule;
        use marque_ism::{
            CanonicalAttrs, Classification, CountryCode, JointClassification,
            MarkingClassification, MarkingType, SciCompartment, SciControlBare, SciControlSystem,
            SciMarking,
        };
        use marque_rules::{Rule, RuleContext};

        let mut attrs = CanonicalAttrs::default();
        attrs.classification = Some(MarkingClassification::Joint(JointClassification {
            level: Classification::Secret,
            countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
        }));
        attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
        attrs.sci_markings = vec![SciMarking::new(
            SciControlSystem::Published(SciControlBare::Hcs),
            Vec::<SciCompartment>::new().into_boxed_slice(),
            None,
        )]
        .into();

        let ctx = RuleContext {
            marking_type: MarkingType::Banner,
            zone: None,
            position: None,
            page_context: None,
            corrections: None,
        };

        let rule = DeclarativeJointHcsRule;
        let diags = rule.check(&attrs, &ctx);

        assert_eq!(
            diags.len(),
            1,
            "E036 must emit exactly one Diagnostic on JOINT+HCS attrs; got: {diags:?}"
        );
        let d = &diags[0];
        assert_eq!(d.rule.as_str(), "E036");
        assert_eq!(d.citation, "CAPCO-2016 §H.3 p57");
        assert!(
            d.fix.is_none(),
            "E036 fix must be None until Stage-4 B reject lands; \
             see incompatibility-primitive-consolidation.md followup"
        );
        assert!(
            d.fix_intent.is_none(),
            "E036 fix_intent must be None (symmetric with fix.is_none(). \
             The G13 walker does NOT see (None, None) rules; this test is \
             the only guard against asymmetric drift)"
        );
    }

    /// PR 3c.B Sub-PR 8.B — programmatic negative case complementing
    /// `e036_emits_no_fix_and_no_fix_intent_pending_stage4_b_reject`.
    ///
    /// Closes the layer-symmetry gap raised by the code-reviewer:
    /// the positive case above tests `DeclarativeJointHcsRule.check()`
    /// directly with programmatic `CanonicalAttrs`. The engine-path
    /// negative case at `e036_does_not_fire_on_joint_without_hcs`
    /// covers a different layer (engine pipeline) and inherits the
    /// parser-gap caveat documented on
    /// `legacy_joint_hcs_rules_do_not_fire_on_parser_path`. This test
    /// closes that gap: it confirms `DeclarativeJointHcsRule.check()`
    /// returns an empty `Vec` when given JOINT+non-HCS-SCI attrs, at
    /// the same Rule-emission layer as the positive case.
    #[test]
    fn e036_does_not_fire_on_joint_with_non_hcs_sci_at_rule_layer() {
        use crate::rules_declarative::DeclarativeJointHcsRule;
        use marque_ism::{
            CanonicalAttrs, Classification, CountryCode, JointClassification,
            MarkingClassification, MarkingType, SciControl,
        };
        use marque_rules::{Rule, RuleContext};

        let mut attrs = CanonicalAttrs::default();
        attrs.classification = Some(MarkingClassification::Joint(JointClassification {
            level: Classification::Secret,
            countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
        }));
        attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
        // SI is permitted with JOINT (§H.3 p57: SCI excluding HCS is
        // permitted with JOINT). The rule must NOT fire.
        attrs.sci_controls = vec![SciControl::Si].into();

        let ctx = RuleContext {
            marking_type: MarkingType::Banner,
            zone: None,
            position: None,
            page_context: None,
            corrections: None,
        };

        let rule = DeclarativeJointHcsRule;
        let diags = rule.check(&attrs, &ctx);

        assert!(
            diags.is_empty(),
            "E036 must NOT fire on JOINT+SI (SCI sans HCS is permitted per \
             §H.3 p57); got: {diags:?}"
        );
    }

    #[test]
    fn e036_does_not_fire_on_joint_without_hcs() {
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E036"),
            "E036 must not fire without HCS present: {diags:?}"
        );
    }

    // --- E037: NODIS ⊥ EXDIS (T035c-21 PR-A, §H.9 p172 + p174) ---

    #[test]
    fn e037_fires_when_nodis_and_exdis_coexist() {
        // Banner carries both NODIS and EXDIS — mutually exclusive per
        // §H.9 p172 + p174. NOFORN is also
        // required (E038), so include it so we only see E037.
        let diags = lint_banner("SECRET//NOFORN//NODIS/EXDIS");
        let e037: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E037").collect();
        assert_eq!(
            e037.len(),
            1,
            "E037 must fire when both NODIS and EXDIS are present: {diags:?}"
        );
        assert!(
            e037[0].citation.contains("§H.9 p172"),
            "E037 citation must pin §H.9 p172; got: {:?}",
            e037[0].citation
        );
        assert!(
            e037[0].citation.contains("p174"),
            "E037 citation must pin p174 (NODIS authority); got: {:?}",
            e037[0].citation
        );
    }

    #[test]
    fn e037_does_not_fire_with_only_nodis() {
        let diags = lint_banner("SECRET//NOFORN//NODIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E037"),
            "E037 must not fire when only NODIS present: {diags:?}"
        );
    }

    #[test]
    fn e037_does_not_fire_with_only_exdis() {
        let diags = lint_banner("SECRET//NOFORN//EXDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E037"),
            "E037 must not fire when only EXDIS present: {diags:?}"
        );
    }

    // --- E038: NODIS / EXDIS require NOFORN (T035c-21 PR-A, §H.9) ---

    #[test]
    fn e038_fires_on_nodis_without_noforn() {
        // §H.9 p174: NODIS "May be used only with NOFORN
        // information." Banner with NODIS and no NOFORN is a
        // violation.
        let diags = lint_banner("SECRET//NODIS");
        let e038: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E038").collect();
        assert_eq!(
            e038.len(),
            1,
            "E038 must fire on NODIS without NOFORN: {diags:?}"
        );
        assert!(
            e038[0].citation.contains("§H.9 p172"),
            "E038 citation must pin §H.9 p172 (EXDIS authority); got: {:?}",
            e038[0].citation
        );
        assert!(
            e038[0].citation.contains("p174"),
            "E038 citation must pin p174 (NODIS authority); got: {:?}",
            e038[0].citation
        );
    }

    #[test]
    fn e038_fires_on_exdis_without_noforn() {
        let diags = lint_banner("SECRET//EXDIS");
        let e038: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E038").collect();
        assert_eq!(
            e038.len(),
            1,
            "E038 must fire on EXDIS without NOFORN: {diags:?}"
        );
    }

    #[test]
    fn e038_does_not_fire_when_nodis_has_noforn() {
        let diags = lint_banner("SECRET//NOFORN//NODIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E038"),
            "E038 must not fire when NOFORN is present: {diags:?}"
        );
    }

    #[test]
    fn e038_does_not_fire_when_exdis_has_noforn() {
        let diags = lint_banner("SECRET//NOFORN//EXDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E038"),
            "E038 must not fire when NOFORN is present: {diags:?}"
        );
    }

    #[test]
    fn e038_fires_only_once_when_both_nodis_and_exdis_lack_noforn() {
        // A single marking with both NODIS and EXDIS (invalid per
        // E037) AND no NOFORN should fire E037 once + E038 once —
        // not E038 twice. The declarative Custom constraint fuses
        // the NODIS/EXDIS disjunction into a single violation.
        let diags = lint_banner("SECRET//NODIS/EXDIS");
        let e038: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E038").collect();
        assert_eq!(
            e038.len(),
            1,
            "E038 must fire exactly once even when both NODIS and EXDIS \
             are present: {diags:?}"
        );
    }

    // --- E039: REL TO cleared from banner when portion has NODIS/EXDIS ---

    #[test]
    fn e039_fires_on_banner_rel_to_with_nodis_portion() {
        // Portion carries NODIS; banner carries REL TO. §H.9 p174
        // line 4301: REL TO not authorized in banner when any portion
        // has NODIS.
        let source = "(S//NF//ND)\nSECRET//NOFORN//NODIS//REL TO USA, GBR";
        let diags = lint_banner(source);
        let e039: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E039").collect();
        assert_eq!(
            e039.len(),
            1,
            "E039 must fire when banner has REL TO and portion has NODIS: {diags:?}"
        );
        assert!(
            e039[0].fix.is_none(),
            "E039 emits no fix (removing REL TO is multi-span and \
             requires human judgment): {:?}",
            e039[0].fix
        );
        assert!(
            e039[0].citation.contains("§H.9 p172"),
            "E039 citation must pin §H.9 p172 (EXDIS); got: {:?}",
            e039[0].citation
        );
        assert!(
            e039[0].citation.contains("p174"),
            "E039 citation must pin p174 (NODIS); got: {:?}",
            e039[0].citation
        );
    }

    #[test]
    fn e039_fires_on_banner_rel_to_with_exdis_portion() {
        let source = "(S//NF//XD)\nSECRET//NOFORN//EXDIS//REL TO USA, GBR";
        let diags = lint_banner(source);
        let e039: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E039").collect();
        assert_eq!(
            e039.len(),
            1,
            "E039 must fire when banner has REL TO and portion has EXDIS: {diags:?}"
        );
    }

    #[test]
    fn e039_does_not_fire_without_nodis_or_exdis_in_portions() {
        // Banner has REL TO, portion has no NODIS/EXDIS — E039 must
        // stay silent (this is a normal REL TO banner).
        let source = "(S//NF)\nSECRET//NOFORN//REL TO USA, GBR";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E039"),
            "E039 must not fire without NODIS/EXDIS in any portion: {diags:?}"
        );
    }

    #[test]
    fn e039_does_not_fire_when_banner_has_no_rel_to() {
        let source = "(S//NF//ND)\nSECRET//NOFORN//NODIS";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E039"),
            "E039 must not fire when banner has no REL TO: {diags:?}"
        );
    }

    // --- E040: Banner must roll up NODIS (or EXDIS if no NODIS) ---

    #[test]
    fn e040_fires_when_banner_missing_nodis_from_portion() {
        // Portion has NODIS; banner has no NODIS. §H.9 p174 line
        // 4300: NODIS in any portion must appear in the banner.
        // Banner already has a non-IC dissem block (LIMDIS), so fix
        // is an insertion at the end of that block.
        let source = "(S//NF//ND)\nSECRET//NOFORN//LIMDIS";
        let diags = lint_banner(source);
        let e040: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E040").collect();
        assert_eq!(
            e040.len(),
            1,
            "E040 must fire when banner omits NODIS: {diags:?}"
        );
        assert!(
            e040[0].message.contains("NODIS"),
            "E040 message must name the missing token; got: {:?}",
            e040[0].message
        );
        let fix = e040[0].fix.as_ref().expect("E040 must carry a fix");
        assert_eq!(
            fix.span.start, fix.span.end,
            "E040 fix must be a zero-width insertion"
        );
        assert_eq!(fix.replacement.as_ref(), "/NODIS");
    }

    #[test]
    fn e040_fires_when_banner_missing_exdis_and_no_nodis_anywhere() {
        // Portion has EXDIS; no NODIS anywhere; banner has no EXDIS.
        // §H.9 p172.
        let source = "(S//NF//XD)\nSECRET//NOFORN//LIMDIS";
        let diags = lint_banner(source);
        let e040: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E040").collect();
        assert_eq!(
            e040.len(),
            1,
            "E040 must fire when banner omits EXDIS with no NODIS: {diags:?}"
        );
        let fix = e040[0].fix.as_ref().expect("fix expected");
        assert_eq!(fix.replacement.as_ref(), "/EXDIS");
    }

    #[test]
    fn e040_nodis_has_priority_over_exdis_when_both_in_portions() {
        // Portions have both NODIS and EXDIS; banner has neither.
        // §H.9 p172 / p174: NODIS has priority
        // over EXDIS in the banner. Banner must carry NODIS (not
        // EXDIS).
        let source = "(S//NF//ND)\n(S//NF//XD)\nSECRET//NOFORN//LIMDIS";
        let diags = lint_banner(source);
        let e040: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E040").collect();
        assert_eq!(e040.len(), 1);
        assert!(
            e040[0].message.contains("NODIS"),
            "E040 must require NODIS (not EXDIS) when both are in portions; \
             got message: {:?}",
            e040[0].message
        );
        let fix = e040[0].fix.as_ref().expect("fix expected");
        assert_eq!(
            fix.replacement.as_ref(),
            "/NODIS",
            "fix must add NODIS, not EXDIS"
        );
    }

    #[test]
    fn e040_does_not_fire_when_banner_already_has_required_token() {
        let source = "(S//NF//ND)\nSECRET//NOFORN//NODIS";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E040"),
            "E040 must not fire when banner already has NODIS: {diags:?}"
        );
    }

    #[test]
    fn e040_emits_no_fix_when_banner_has_no_non_ic_dissem_block() {
        // Banner has classification + IC dissem only, but NO
        // Non-IC dissem block at all. Inserting a new category block
        // is unsafe (needs separator-positioning), so E040 emits a
        // no-fix Error.
        let source = "(S//NF//ND)\nSECRET//NOFORN";
        let diags = lint_banner(source);
        let e040: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E040").collect();
        assert_eq!(e040.len(), 1);
        assert!(
            e040[0].fix.is_none(),
            "E040 must not carry a fix when banner has no Non-IC dissem \
             block (byte-positioning a new block is unsafe): {:?}",
            e040[0].fix
        );
    }

    // --- E041: NODIS supersedes EXDIS in a portion ---

    #[test]
    fn e041_fires_on_portion_with_both_nodis_and_exdis() {
        // §H.9 p172 / p174: when a portion has
        // both, NODIS supersedes EXDIS. E041 surfaces the diagnostic
        // at Warn severity with no auto-fix (user removes EXDIS
        // manually). See the rule doc for why the auto-fix is
        // deferred.
        let diags = lint_portion("(S//NF//ND/XD)");
        let e041: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E041").collect();
        assert_eq!(
            e041.len(),
            1,
            "E041 must fire on portion with both NODIS and EXDIS: {diags:?}"
        );
        assert_eq!(e041[0].severity, Severity::Warn);
        assert!(
            e041[0].fix.is_none(),
            "E041 emits no auto-fix (the parser does not emit within-\
             category `/` as a Separator token; see rule doc); got: \
             {:?}",
            e041[0].fix
        );
        assert!(
            e041[0].message.contains("NODIS") && e041[0].message.contains("EXDIS"),
            "E041 message must name both tokens; got: {:?}",
            e041[0].message
        );
    }

    #[test]
    fn e041_points_at_exdis_token_in_both_orderings() {
        // E041's diagnostic span should point at the EXDIS token
        // regardless of whether it appears before or after NODIS in
        // the portion. Exercise both orderings.
        for src in ["(S//NF//ND/XD)", "(S//NF//XD/ND)"] {
            let diags = lint_portion(src);
            let e041: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E041").collect();
            assert_eq!(e041.len(), 1, "E041 must fire on {src:?}: {diags:?}");
            let span_text = e041[0].span.as_str(src.as_bytes()).unwrap();
            assert_eq!(
                span_text, "XD",
                "E041 span must point at the EXDIS token in {src:?}; \
                 got: {span_text:?}"
            );
        }
    }

    #[test]
    fn e041_does_not_fire_on_portion_with_only_nodis() {
        let diags = lint_portion("(S//NF//ND)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E041"),
            "E041 must not fire on portion with only NODIS: {diags:?}"
        );
    }

    #[test]
    fn e041_does_not_fire_on_banner_even_when_both_present() {
        // E041 is portion-only per §H.9 p172 / p174 line
        // 4306 ("in the portion mark"). The banner case is owned by
        // E037 (mutual exclusion, Error).
        let diags = lint_banner("SECRET//NOFORN//NODIS/EXDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E041"),
            "E041 must not fire on banner context: {diags:?}"
        );
        // But E037 must still fire.
        assert!(
            diags.iter().any(|d| d.rule.as_str() == "E037"),
            "E037 must still fire on banner NODIS+EXDIS: {diags:?}"
        );
    }

    // -----------------------------------------------------------------------
    // E053 — NOFORN conflicts with REL TO (§H.8 p145)
    // -----------------------------------------------------------------------

    #[test]
    fn e053_fires_when_noforn_and_rel_to_coexist_in_banner() {
        // §H.8 p145: NOFORN "Cannot be used with REL TO."
        let diags = lint_banner("SECRET//REL TO USA, GBR//NOFORN");
        let e053: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E053").collect();
        assert_eq!(
            e053.len(),
            1,
            "E053 must fire once when NOFORN and REL TO coexist: {diags:?}"
        );
    }

    #[test]
    fn e053_fires_on_portion_with_nf_and_rel_to() {
        // Portion-mark form: `NF` is the portion abbreviation for NOFORN.
        let diags = lint_portion("(S//REL TO USA, GBR/NF)");
        let e053: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E053").collect();
        assert_eq!(
            e053.len(),
            1,
            "E053 must fire on portion with NF and REL TO: {diags:?}"
        );
    }

    #[test]
    fn e053_silent_when_only_noforn_no_rel_to() {
        let diags = lint_banner("SECRET//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E053"),
            "E053 must not fire when REL TO is absent: {diags:?}"
        );
    }

    #[test]
    fn e053_silent_when_only_rel_to_no_noforn() {
        let diags = lint_banner("SECRET//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E053"),
            "E053 must not fire when NOFORN is absent: {diags:?}"
        );
    }

    #[test]
    fn e002_fix_output_dedups_when_input_has_duplicates() {
        // Issue #234 PR-B fixup (Copilot review): E002's fix path
        // also composes dedup + canonicalize so its replacement stays
        // single-pass idempotent against E052 on overlapping spans.
        // Input: USA missing AND non-USA codes duplicated → E002
        // fires (missing USA), E052 fires (GBR repeated). FR-016
        // tiebreaker keeps E002 (lex), so E002's replacement must be
        // canonical or re-lint would still fire E052.
        let src = "SECRET//REL TO GBR, AUS, GBR";
        let diags = lint_banner(src);
        let e002_fix = diags
            .iter()
            .find(|d| d.rule.as_str() == "E002")
            .and_then(|d| d.fix.as_ref())
            .expect("E002 must fire and carry a fix when USA is missing from REL TO");
        assert_eq!(
            e002_fix.replacement.as_ref(),
            "USA, AUS, GBR",
            "E002 fix must dedup before sorting (canonical form, no duplicates)"
        );
    }

    #[test]
    fn dedup_country_codes_preserves_first_occurrence_order() {
        use marque_ism::CountryCode;
        let codes = vec![
            CountryCode::USA,
            CountryCode::try_new(b"GBR").unwrap(),
            CountryCode::USA,
            CountryCode::try_new(b"AUS").unwrap(),
            CountryCode::try_new(b"GBR").unwrap(),
        ];
        let deduped = dedup_country_codes(&codes);
        let expected = vec![
            CountryCode::USA,
            CountryCode::try_new(b"GBR").unwrap(),
            CountryCode::try_new(b"AUS").unwrap(),
        ];
        assert_eq!(deduped, expected);
    }

    // --- E021: RD/FRD requires NOFORN ---

    #[test]
    fn e021_fires_on_rd_without_noforn() {
        let diags = lint_banner("SECRET//RD//REL TO USA, GBR");
        let e021: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E021").collect();
        assert_eq!(e021.len(), 1);
    }

    #[test]
    fn e021_does_not_fire_on_rd_with_noforn() {
        let diags = lint_banner("SECRET//RD//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E021"),
            "E021 should not fire with NOFORN present: {diags:?}"
        );
    }

    #[test]
    fn e021_fires_on_frd_without_noforn() {
        let diags = lint_banner("SECRET//FRD//REL TO USA, GBR");
        let e021: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E021").collect();
        assert_eq!(e021.len(), 1);
    }

    // --- CNWDI floor (formerly E022, now bridge-emitted E058 via
    // catalog row `E058/CNWDI-classification-floor`) ---
    //
    // PR 3b.D (T026d): the CNWDI floor invariant moved into the
    // class-floor catalog. PR 3c.B Commit 7.3: the walker
    // (`DeclarativeClassFloorRule`) retired; the engine's constraint-
    // catalog bridge is the sole emitter. The lib-level tests
    // (`e022_fires_on_cnwdi_with_confidential` and friends) that
    // exercised `lint_banner` retired alongside the walker — the
    // bridge fires through `engine.lint`, which the lib-level harness
    // bypasses. The 27 class-floor catalog rows are covered
    // comprehensively (fires-below / silent-at-floor / silent-when-
    // absent triplet per row, plus span-anchor + severity-override
    // tests) by the engine-level test suite in
    // `crates/capco/tests/class_floor_catalog.rs`. The CNWDI-specific
    // entry points are `cnwdi_fires_below_secret` and
    // `cnwdi_does_not_fire_when_marking_absent` in that file.

    // --- E024: RD precedence ---

    #[test]
    fn e024_fires_on_rd_plus_frd() {
        // Both RD and FRD in same marking — FRD should be removed.
        let diags = lint_banner("SECRET//RD//FRD//NOFORN");
        let e024: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E024").collect();
        assert_eq!(e024.len(), 1);
        assert!(e024[0].message.contains("FRD"));
    }

    #[test]
    fn e024_does_not_fire_on_rd_alone() {
        let diags = lint_banner("SECRET//RD//NOFORN");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E024"));
    }

    // --- UCNI ceiling (formerly E025, now bridge-emitted E058 via
    // catalog rows `E058/DOD-UCNI-classification-ceiling` +
    // `E058/DOE-UCNI-classification-ceiling`) ---
    //
    // PR 3b.D (T026d): the UCNI ceiling invariant moved into the
    // class-floor catalog as TWO rows (DOD UCNI + DOE UCNI; split per
    // PM decision so each carries its own §H.6 sub-page citation).
    // PR 3c.B Commit 7.3: lib-level `lint_banner` tests retired
    // alongside the walker — the engine-level UCNI coverage lives in
    // `crates/capco/tests/class_floor_catalog.rs::dod_ucni_*` and
    // `doe_ucni_*`.

    // --- Spec 003 SCI compartments: E010 structural regression ---

    #[test]
    fn e010_still_fires_when_hcs_reaches_rule_through_structural_path() {
        // Bare `HCS` is dispatched to the structural subparser (is_bare_cve_value
        // matches) and surfaces as SciMarking { Published(Hcs), compartments: [] }.
        // The canonical_enum projection also populates sci_controls, so both
        // detection predicates in E010 see the bare HCS. This test pins that
        // the combined predicate still fires once (not twice) for regression.
        let diags = lint_banner("TOP SECRET//HCS//NOFORN");
        let e010: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E010").collect();
        assert_eq!(e010.len(), 1, "E010 must fire exactly once for bare HCS");
    }

    // --- Shared sort key ---

    #[test]
    fn sar_sort_key_numeric_before_alpha() {
        // Numeric-prefixed sorts before pure alpha.
        assert!(sar_sort_key("12") < sar_sort_key("BP"));
        assert!(sar_sort_key("7ALPHA") < sar_sort_key("BP"));
    }

    #[test]
    fn sar_sort_key_numeric_by_value() {
        // Numeric prefixes compare as integers, not bytewise.
        assert!(sar_sort_key("9") < sar_sort_key("12"));
        assert!(sar_sort_key("J12") < sar_sort_key("J54"));
    }

    #[test]
    fn sar_sort_key_alpha_by_bytelex() {
        assert!(sar_sort_key("BP") < sar_sort_key("CD"));
        assert!(sar_sort_key("CD") < sar_sort_key("XR"));
    }

    // --- SAR floor (formerly E027, now bridge-emitted E058 via
    // catalog row `E058/SAR-classification-floor`) ---
    //
    // PR 3b.D (T026d): the SAR floor invariant moved into the class-floor
    // catalog. PR 3c.B Commit 7.3: lib-level `lint_banner` tests retired
    // alongside the walker. Engine-level coverage:
    // `crates/capco/tests/class_floor_catalog.rs::sar_fires_on_unclassified`,
    // `sar_does_not_fire_at_confidential`, and
    // `sar_does_not_fire_when_marking_absent`.

    // --- W034: SCI custom control info ---

    #[test]
    fn e034_fires_on_custom_control_via_structural_path() {
        // `123/SI-G` routes through the structural subparser; the `123` head
        // creates a Custom-system SciMarking. W034 surfaces that for audit
        // visibility (severity Off by default, so the engine gates it).
        let diags = lint_banner("TOP SECRET//123/SI-G//NOFORN");
        let w034: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W034").collect();
        assert_eq!(
            w034.len(),
            1,
            "W034 must fire on custom control 123: {diags:?}"
        );
        assert!(w034[0].fix.is_none(), "W034 must not propose a fix");
        // T035c-2: W034 now defaults to Warn (was Off with a harness
        // workaround). Info is available as a config-opt-in.
        assert_eq!(w034[0].severity, marque_rules::Severity::Warn);
        assert!(w034[0].message.contains("unpublished SCI control system"));
    }

    #[test]
    fn e034_does_not_fire_on_published_only() {
        let diags = lint_banner("TOP SECRET//SI-G//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W034"),
            "W034 must not fire on SI-G alone: {diags:?}"
        );
    }

    // --- E035: SCI banner rollup ---

    #[test]
    fn e035_no_ops_without_page_context() {
        // The test harness passes `page_context: None`. Until P4 lands and
        // populates a real PageContext with expected_sci_markings(), E035
        // must stay silent rather than emit false positives.
        let diags = lint_banner("TOP SECRET//SI-G//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E035"),
            "E035 must no-op without a PageContext: {diags:?}"
        );
    }

    #[test]
    fn e035_fires_on_missing_compartment_sci_asymmetry_with_sar() {
        // SCI/SAR asymmetry lockdown: portion has `SI-G` (system SI,
        // compartment G); banner has bare `SI` (no compartment shown).
        // E035 MUST fire. This is the exact shape that E031 (SAR)
        // deliberately does NOT fire on after T035c-19 PR-C — §H.5
        // p101 makes SAR hierarchy optional. §H.4 contains
        // no equivalent carve-out for SCI, so E035 enforces full
        // hierarchy roll-up. Flipping this test would break the
        // source-level semantic distinction.
        let source = "(TS//SI-G//NF)\nTOP SECRET//SI//NOFORN";
        let diags = lint_banner(source);
        let e035: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E035").collect();
        assert_eq!(
            e035.len(),
            1,
            "E035 MUST fire when banner omits compartment G that appears in \
             a portion — SCI has no hierarchy-optional carve-out: {diags:?}"
        );
        assert!(
            e035[0].message.contains("G"),
            "message must name the missing compartment; got: {:?}",
            e035[0].message
        );
    }

    #[test]
    fn e035_fires_on_missing_sub_compartment_sci_asymmetry_with_sar() {
        // Sibling asymmetry test: portion has `SI-G ABCD` (sub-comp
        // ABCD under compartment G); banner has `SI-G` (no
        // sub-compartment). E035 MUST fire; E031 would not for the
        // SAR-equivalent shape.
        let source = "(TS//SI-G ABCD//NF)\nTOP SECRET//SI-G//NOFORN";
        let diags = lint_banner(source);
        let e035: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E035").collect();
        assert_eq!(
            e035.len(),
            1,
            "E035 MUST fire when banner omits sub-compartment ABCD present \
             in a portion: {diags:?}"
        );
        assert!(
            e035[0].message.contains("ABCD"),
            "message must name the missing sub-compartment; got: {:?}",
            e035[0].message
        );
    }

    #[test]
    fn e035_does_not_fire_when_banner_covers_full_hierarchy() {
        // Happy path: banner's hierarchy matches the portion's. E035
        // must stay silent.
        let source = "(TS//SI-G ABCD//NF)\nTOP SECRET//SI-G ABCD//NOFORN";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E035"),
            "E035 must not fire when banner already covers portion hierarchy: \
             {diags:?}"
        );
    }

    #[test]
    fn e035_message_wording_covers_all_hierarchy_levels() {
        // PR #102 review: the rule's `missing` list can contain
        // three shapes — system-missing, compartment-missing, and
        // sub-compartment-missing. The earlier diagnostic message
        // said only "missing compartments", which was inaccurate
        // for the system-missing case (entire SCI control system
        // absent from banner). This test locks the corrected
        // wording.
        //
        // Scenario: portion carries `TK` (entire system); banner
        // carries only `SI`. So TK is missing as an ENTIRE SYSTEM,
        // not just a compartment. The message must reflect that.
        let source = "(TS//SI/TK//NF)\nTOP SECRET//SI//NOFORN";
        let diags = lint_banner(source);
        let e035: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E035").collect();
        assert_eq!(e035.len(), 1);
        let msg = &e035[0].message;
        assert!(
            msg.contains("systems, compartments, and/or sub-compartments")
                || msg.contains("markings"),
            "E035 message must describe the hierarchy-level breadth \
             accurately (not only 'compartments'); got: {msg:?}"
        );
        assert!(
            msg.contains("TK"),
            "E035 message must name the missing TK system; got: {msg:?}"
        );
        // The per-entry format still specifies the level for each
        // missing item, so `TK` carries "(system missing from banner)".
        assert!(
            msg.contains("system missing from banner"),
            "E035 per-entry annotation must mark TK as an entirely \
             missing system; got: {msg:?}"
        );
    }

    #[test]
    fn e035_cites_per_system_precedence_rules() {
        let source = "(TS//SI-G//NF)\nTOP SECRET//SI//NOFORN";
        let diags = lint_banner(source);
        let e035: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E035").collect();
        assert_eq!(e035.len(), 1);
        // T026a D13 single-citation discipline: the citation string
        // carries §H.4 per-system "Precedence Rules for Banner Line
        // Guidance" only — that's the operative banner-roll-up rule
        // for SCI per `specs/006-engine-rule-refactor/tasks.md`
        // T026a. §D.2 p28 (CAPCO-2016 lines 577–579) restates the
        // same invariant in general-algorithm prose; it lives as a
        // background reference in `evaluate_sci_banner_rollup`'s doc
        // comment, NOT in the citation string.
        assert!(
            e035[0].citation.contains("§H.4"),
            "E035 citation must reference §H.4; got: {:?}",
            e035[0].citation
        );
        assert!(
            e035[0]
                .citation
                .contains("Precedence Rules for Banner Line"),
            "E035 citation must reference the per-system Precedence Rules \
             template; got: {:?}",
            e035[0].citation
        );
        // §D.2 was demoted to a background-only doc-comment reference
        // per the M-1 review condition (citation-discipline cleanup).
        // Pin its absence so a future change that re-adds it to the
        // citation string trips this test instead of silently
        // re-introducing a co-primary cross-citation.
        assert!(
            !e035[0].citation.contains("§D.2"),
            "E035 citation must NOT mix §D.2 (general-algorithm prose) \
             with §H.4 (per-category operative rule) — D13 single-\
             citation discipline. §D.2 lives in evaluator doc comment \
             as a background reference. got: {:?}",
            e035[0].citation
        );
    }

    // --- E008 skip filter: structural SCI tokens ---

    #[test]
    fn e008_does_not_fire_on_structurally_formed_sci_tokens() {
        // `SI-G ABCD DEFG` is a structurally-formed SCI token. When the
        // parser accepts it, no Unknown span is produced and E008 stays
        // silent for that reason. This test pins the structural happy path.
        let diags = lint_banner("SECRET//SI-G ABCD DEFG//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E008"),
            "E008 must not fire on structurally-parsed SI-G block: {diags:?}"
        );
    }

    // PR 3b.D (T026d): retired E027 → E058 catalog row
    // `E058/SAR-classification-floor`. PR 3c.B Commit 7.3: the walker
    // retired; SAR-floor citation coverage moved to the engine-level
    // test `crates/capco/tests/class_floor_catalog.rs::sar_fires_on_unclassified`
    // (asserts `sar[0].citation == "CAPCO-2016 §H.5"`).

    // --- E031: sar-banner-rollup ---

    #[test]
    fn e031_fires_when_banner_missing_program_from_portion() {
        // Portions introduce SAR-BP and SAR-CD; banner only mentions BP.
        // E031's fix is a zero-width INSERTION at the end of the SAR
        // block — so the fix span is (block_end, block_end) and the
        // replacement is `/CD`. This shape lets E031 coexist with E028
        // / E029 fixes on the same marking under the engine's overlap
        // guard (see rule doc for the full FR-016 argument).
        let source = "(S//SAR-BP//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(
            e031.len(),
            1,
            "E031 must fire when banner omits CD: {diags:?}"
        );
        let d = e031[0];
        assert!(
            d.message.contains("CD"),
            "message must name the missing program: {}",
            d.message
        );
        let fix = d
            .fix
            .as_ref()
            .expect("E031 must carry a fix when banner has SAR block");

        // Zero-width insertion span: start == end == end-of-block byte.
        assert_eq!(
            fix.span.start, fix.span.end,
            "E031 fix must be a zero-width insertion"
        );
        assert_eq!(
            fix.original.as_ref(),
            "",
            "zero-width insertion must have empty `original`"
        );
        assert_eq!(
            fix.replacement.as_ref(),
            "/CD",
            "insertion replacement must be `/<missing>`"
        );
        assert!((fix.confidence.combined() - 0.9).abs() < f32::EPSILON);

        // Applied output: simulate the splice and confirm the banner now
        // contains `SAR-BP/CD`.
        let mut buf = source.as_bytes().to_vec();
        buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());
        let applied = std::str::from_utf8(&buf).unwrap();
        assert!(
            applied.contains("SECRET//SAR-BP/CD//NOFORN"),
            "applied fix must produce `SECRET//SAR-BP/CD//NOFORN`; \
             got: {applied:?}"
        );
    }

    #[test]
    fn e031_does_not_fire_when_banner_omits_portion_compartment() {
        // T035c-19 PR-C: narrowed predicate. §H.5 p101 and
        // §H.5 p99 make banner hierarchy depth (below the
        // program identifier) optional. A portion with `SAR-BP-J12`
        // rolling up to a banner with `SAR-BP` (no compartment shown)
        // is compliant — the author deliberately omitted hierarchy,
        // which §H.5 permits. The prior behavior treated this as an
        // E031 violation; that was over-restriction relative to
        // source.
        let source = "(S//SAR-BP-J12//NF)\nSECRET//SAR-BP//NOFORN";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E031"),
            "E031 must NOT fire on optional-hierarchy banner \
             (portion has BP-J12, banner has bare BP — §H.5 p101 \
             permits): {diags:?}"
        );
    }

    #[test]
    fn e031_does_not_fire_when_banner_omits_portion_sub_compartment() {
        // Sibling case: portion has `SAR-BP-J12 K15` (J12 is a
        // compartment, K15 is a sub-compartment of J12); banner has
        // `SAR-BP-J12` (omits the sub-compartment). §H.5 p101 line
        // 2460 covers sub-compartments too ("hierarchy ... below the
        // program identifier is optional"). Must not fire.
        let source = "(S//SAR-BP-J12 K15//NF)\nSECRET//SAR-BP-J12//NOFORN";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E031"),
            "E031 must NOT fire when banner omits sub-compartment \
             present in portion (hierarchy is optional): {diags:?}"
        );
    }

    #[test]
    fn e031_fires_when_banner_has_no_sar_block_but_portion_does() {
        // Portion has SAR-BP; banner has no SAR block at all.
        let source = "(S//SAR-BP//NF)\nSECRET//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(
            e031.len(),
            1,
            "E031 must fire when banner lacks any SAR block: {diags:?}"
        );
        // No fix when banner has no SAR block (byte-positioning is unsafe).
        assert!(
            e031[0].fix.is_none(),
            "E031 must not propose a fix when no SAR block exists"
        );
        // And severity escalates to Error for this variant.
        assert_eq!(e031[0].severity, Severity::Error);

        // PR #101 review: the no-block message must describe a whole
        // missing block, NOT read like the block exists but is
        // missing internal programs. Pin the distinct wording so a
        // regression that re-merges the two branches' messages
        // fails here.
        let msg = &e031[0].message;
        assert!(
            msg.contains("missing an SAR block"),
            "no-block message must state that the SAR block itself is \
             missing; got: {msg:?}"
        );
        assert!(
            !msg.contains("SAR block is missing programs"),
            "no-block message must NOT reuse the with-block \
             'block is missing programs' wording; got: {msg:?}"
        );
    }

    #[test]
    fn e031_does_not_fire_when_banner_matches_portions() {
        let source = "(S//SAR-BP//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP/CD//NOFORN";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E031"),
            "E031 must not fire when banner SAR block covers all portions: {diags:?}"
        );
    }

    #[test]
    fn e031_does_not_fire_when_no_portions_have_sar() {
        // Banner has a SAR block but no portions carry SAR — the rollup
        // produces None and nothing is missing.
        let source = "(S//NF)\nSECRET//SAR-BP//NOFORN";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E031"),
            "E031 must not fire without any SAR portions: {diags:?}"
        );
    }

    #[test]
    fn e031_fix_preserves_observed_hierarchy_when_adding_missing_program() {
        // T035c-19 PR-C: the zero-width insertion at end-of-block
        // preserves the observed banner's hierarchy verbatim (because
        // it doesn't touch the observed bytes at all) and adds only
        // the missing programs as bare identifiers. §H.5 p101 line
        // 2460 makes hierarchy depiction the author's choice — the
        // fix honors that for existing programs by construction.
        //
        // Portion: SAR-BP-J12 (BP with compartment J12) and SAR-CD.
        // Banner observed: SAR-BP-J12 (BP with compartment shown, CD
        // missing). Applied output: SAR-BP-J12/CD (J12 preserved,
        // bare CD appended — NO invented hierarchy on CD).
        let source = "(S//SAR-BP-J12//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP-J12//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(
            e031.len(),
            1,
            "E031 must fire on missing program CD: {diags:?}"
        );
        let fix = e031[0].fix.as_ref().expect("E031 must have fix");

        assert_eq!(fix.replacement.as_ref(), "/CD");
        assert_eq!(fix.span.start, fix.span.end);

        let mut buf = source.as_bytes().to_vec();
        buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());
        let applied = std::str::from_utf8(&buf).unwrap();
        assert!(
            applied.contains("SECRET//SAR-BP-J12/CD//NOFORN"),
            "applied fix must preserve BP-J12 and append bare CD; \
             got: {applied:?}"
        );
    }

    #[test]
    fn e031_cites_line_2458_and_hierarchy_optional_note() {
        // T035c-19 PR-C citation lockdown. E031's authority is:
        //   §H.5 p101  — programs MUST roll up
        //   §H.5 p101  — hierarchy MAY be omitted
        // The citation string must reference both so reviewers land
        // on the two passages that together define the narrowed
        // predicate.
        let source = "(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(e031.len(), 1);
        assert!(
            e031[0].citation.contains("§H.5 p101"),
            "E031 citation must pin §H.5 p101 (roll-up rule); got: {:?}",
            e031[0].citation
        );
        assert!(
            e031[0].citation.contains("§H.5 p101"),
            "E031 citation must reference the hierarchy-optional carve-out \
             at §H.5 p101; got: {:?}",
            e031[0].citation
        );
    }

    #[test]
    fn e008_fires_on_malformed_sci_shape() {
        // `SI-` is SCI-shaped but invalid (dangling hyphen). The structural
        // subparser rejects it, so it falls through as Unknown and E008
        // correctly fires — no silent suppression.
        let diags = lint_banner("SECRET//SI-//NOFORN");
        assert!(
            diags.iter().any(|d| d.rule.as_str() == "E008"),
            "E008 must fire on malformed SCI-shaped token: {diags:?}"
        );
    }

    /// PR 3c.B Sub-PR 9 — pin the consciously-decided-no-fix-intent
    /// migration state for E005.
    ///
    /// Per `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md`
    /// D4 (Path A fallback), E005 stays as a registered hand-written
    /// `Rule` impl in `rules.rs` until `render_canonical` lands on the
    /// `MarkingScheme` trait surface (the
    /// `Recanonicalize { scope: Document }` retirement target). The
    /// structural blocker — `MarkingScheme::evaluate_custom` having no
    /// access to `RuleContext.marking_type` — is tracked in
    /// `specs/006-engine-rule-refactor/followups/constraint-context-extension.md`.
    /// Until the retirement vehicle lands, the rule emits a diagnostic
    /// with both `fix.is_none()` AND `fix_intent.is_none()`.
    ///
    /// **Coverage note:** the G13 closure walker at
    /// `tests/g13_closure_fix_intent.rs::all_migrated_rule_intents_pass_g13_envelope_walker`
    /// only inspects rules that auto-apply through the engine (those
    /// emitting `AppliedFixProposal::New` records, which require
    /// `fix_intent.is_some()`). E005 with `fix_intent: None` is never
    /// reached by that walker — so this symmetry pin is the **only**
    /// guard against a future commit accidentally producing an
    /// asymmetric `(fix, fix_intent)` pair on E005.
    #[test]
    fn e005_emits_no_fix_and_no_fix_intent_pending_stage4_recanonicalize_document() {
        let diags = lint_banner("SECRET//25X1//NOFORN");
        let e005 = diags
            .iter()
            .find(|d| d.rule.as_str() == "E005")
            .expect("E005 must fire on `SECRET//25X1//NOFORN` (declass exemption in banner)");
        assert!(
            e005.fix.is_none(),
            "E005 fix must be None until Stage-4 `Recanonicalize {{ scope: Document }}` \
             lands; see constraint-context-extension.md followup"
        );
        assert!(
            e005.fix_intent.is_none(),
            "E005 fix_intent must be None (symmetric with fix.is_none()). \
             The G13 walker does NOT see (None, None) rules; this test is \
             the only guard against asymmetric drift"
        );
    }

    /// PR 3c.B Sub-PR 9 — pin the consciously-decided-no-fix-intent
    /// migration state for S005.
    ///
    /// Per `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md`
    /// D4 (Path A fallback), S005 stays as a registered hand-written
    /// `Rule` impl in `rules.rs` until the admonition emitter channel
    /// is specced and built per
    /// `specs/006-engine-rule-refactor/followups/admonition-channel.md`.
    /// The structural blocker — `MarkingScheme::evaluate_custom` having
    /// no access to `RuleContext.page_context` (the entire body of
    /// `analyze_uncertain_reduction` is page-context-dependent) — is
    /// tracked in
    /// `specs/006-engine-rule-refactor/followups/constraint-context-extension.md`.
    /// Until either retirement vehicle lands, the rule emits a diagnostic
    /// with both `fix.is_none()` AND `fix_intent.is_none()`.
    ///
    /// **Coverage note:** same as the E005 pin — the G13 walker doesn't
    /// see `(None, None)` rules; this symmetry pin is the only guard.
    #[test]
    fn s005_emits_no_fix_and_no_fix_intent_pending_stage4_admonition_channel() {
        // RSMA is an NA-deprecated tetragraph from the V2022-NOV ISMCAT
        // taxonomy (per the existing test-module note at L~4585);
        // `is_decomposable("RSMA")` returns `None`, so it qualifies as
        // an uncertain code. Two portions list it differently; the
        // page-level atom intersection drops RSMA. Banner has no REL TO
        // (NOFORN supersedes) — active-validation / Suggest branch.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        let s005 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S005")
            .expect("S005 must fire on RSMA uncertain-reduction (Suggest branch)");
        assert!(
            s005.fix.is_none(),
            "S005 fix must be None until Stage-4 admonition channel lands; \
             see admonition-channel.md and constraint-context-extension.md followups"
        );
        assert!(
            s005.fix_intent.is_none(),
            "S005 fix_intent must be None (symmetric with fix.is_none())"
        );
    }

    /// PR 3c.B Sub-PR 9 — pin the consciously-decided-no-fix-intent
    /// migration state for S006. Same shape as S005's pin; the Info
    /// branch fires when the banner's REL TO is consistent with the
    /// atom-semantics intersection.
    #[test]
    fn s006_emits_no_fix_and_no_fix_intent_pending_stage4_admonition_channel() {
        // Banner REL TO equals the atom-semantics intersection
        // ({USA, GBR}); `expected ⊆ banner` ⇒ Info branch ⇒ S006 fires.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//REL TO USA, GBR";
        let diags = lint_banner(source);
        let s006 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S006")
            .expect("S006 must fire on RSMA uncertain-reduction (Info branch)");
        assert!(
            s006.fix.is_none(),
            "S006 fix must be None until Stage-4 admonition channel lands"
        );
        assert!(
            s006.fix_intent.is_none(),
            "S006 fix_intent must be None (symmetric with fix.is_none())"
        );
    }
}

/// Internal test support module — drives the parser and rules directly,
/// without depending on the engine crate. This avoids a circular dependency
/// (`marque-capco` is below `marque-engine` in the workspace graph).
///
/// `pub(crate)` so sibling rule modules (any future per-cluster module)
/// can share the same test harness rather than duplicating the parser-
/// driving boilerplate. Gated on `cfg(test)` so it never ships in release
/// builds.
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) mod marque_capco_test_support {
    use super::{CapcoRuleSet, CapcoScheme};
    use marque_core::{Parser, Scanner};
    use marque_ism::{CapcoTokenSet, MarkingType, PageContext};
    use marque_rules::{Diagnostic, RuleContext, RuleSet};
    use std::sync::Arc;

    fn run(source: &[u8]) -> Vec<Diagnostic<CapcoScheme>> {
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidates = Scanner::scan(source);
        let rule_set = CapcoRuleSet::new();
        let mut out = Vec::new();
        // Accumulate a PageContext across portions so banner/CAB rules that
        // read `ctx.page_context` (E031) behave the same here as in the
        // real engine. Reset on scanner-emitted PageBreak candidates.
        let mut page_context = PageContext::new();
        let mut page_context_arc: Option<Arc<PageContext>> = None;
        for candidate in &candidates {
            if candidate.kind == MarkingType::PageBreak {
                page_context = PageContext::new();
                page_context_arc = None;
                continue;
            }
            let Ok(parsed) = parser.parse(candidate, source) else {
                continue;
            };
            // PR-3a transitional adapter: parser produces ParsedAttrs<'src>;
            // PageContext / Rule::check consume CanonicalAttrs.
            // Test-fixture carve-out per Constitution V Principle V — the
            // adapter is invoked here to construct the test input only.
            let attrs = marque_ism::from_parsed_unchecked(parsed.attrs);
            if parsed.kind == MarkingType::Portion {
                page_context.add_portion(attrs.clone());
                page_context_arc = None;
            }
            let ctx_page = if parsed.kind != MarkingType::Portion && !page_context.is_empty() {
                Some(
                    page_context_arc
                        .get_or_insert_with(|| Arc::new(page_context.clone()))
                        .clone(),
                )
            } else {
                None
            };
            let ctx = RuleContext {
                marking_type: candidate.kind,
                zone: None,
                position: None,
                page_context: ctx_page,
                corrections: None,
            };
            for rule in rule_set.rules() {
                out.extend(rule.check(&attrs, &ctx));
            }
        }
        out
    }

    pub(crate) fn lint_banner(s: &str) -> Vec<Diagnostic<CapcoScheme>> {
        run(s.as_bytes())
    }

    pub(crate) fn lint_portion(s: &str) -> Vec<Diagnostic<CapcoScheme>> {
        run(s.as_bytes())
    }
}
