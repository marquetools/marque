// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO rule implementations — Layer 2 diagnostic intelligence.
//!
//! Each rule uses Layer 1 schema predicates (from generated/validators.rs) to
//! detect violations, then produces enriched diagnostics with fixes and
//! confidence. Phase 3 lands the full set of MVP rules with byte-precise
//! spans threaded through `IsmAttributes::token_spans`.
//!
//! Rule IDs follow the convention: E### = error, W### = warning, C### = correction.
//! Assignments per spec tasks.md:
//!   E001 = portion mark used in banner (correctness)
//!   E002 = REL TO missing USA trigraph (T031)
//!   E003 = misordered banner blocks (T032)
//!   E004 = separator-count normalization (T033)
//!   E005 = declassification misplaced (banner or portion; belongs in CAB) (T034)
//!   E006 = deprecated dissem control (T035)
//!   E007 = X-shorthand declass date (T036)
//!   E008 = unrecognized token (T037)
//!   E009 = portion abbreviation
//!   E010 = bare HCS without compartment suffix
//!   E011 = missing leading // on non-US classification
//!   E012 = dual classification (US + foreign conflict)
//!   E013 = JOINT comma / REL TO delimiter mismatch (§H.3 / §H.8)
//!   E014 = JOINT participants missing from REL TO
//!   E015 = non-US classification without dissem control
//!   W001 = retired in T035c-14 (CAPCO-2016 §F treats legacy
//!           markings as unauthorized, not "deprecated but legal";
//!           no authoritative bucket for a warning-severity rule)
//!   W002 = US + FGI comingling in portion
//!   E016 = RESTRICTED not allowed with JOINT
//!   E017 = retired in T035b (over-restrictive per CAPCO §H.3 line 4140)
//!   E018 = retired in T035b (over-restrictive per CAPCO §H.3 line 4140)
//!   E019 = retired in T035b (over-restrictive per CAPCO §H.3 line 4140)
//!   E020 = country code list ordering (alphabetical after USA)
//!   E021 = RD/FRD requires NOFORN (configurable to warn)
//!   E022 = CNWDI only with TS or S RD
//!   E023 = SIGMA valid values + numerical order
//!   E024 = RD precedence over FRD/TFNI
//!   E025 = UCNI only with UNCLASSIFIED
//!   W003 = non-IC dissem in classified banner
//!   E032 = SCI control-system sort order (spec 003-sci-compartments)
//!   E033 = SCI compartment / sub-compartment sort order
//!   E034 = SCI custom (unpublished) control-system audit visibility
//!   E035 = SCI banner rollup (missing compartments from portions)
//!   E036 = JOINT may not be used with HCS markings (T035b, replaces E017-E019)
//!   C001 = corrections-map typo (T058, Phase 5)

use marque_ism::generated::migrations::find_migration;
use marque_ism::{
    IsmAttributes, MarkingClassification, SciControlSystem, SciMarking, Span, TokenKind, TokenSpan,
    sar_sort_key,
};
use marque_rules::{
    Diagnostic, FixProposal, FixSource, Rule, RuleContext, RuleId, RuleSet, Severity,
};

/// The full CAPCO rule set returned by `marque_capco::capco_rules()`.
pub struct CapcoRuleSet {
    rules: Vec<Box<dyn Rule>>,
}

impl Default for CapcoRuleSet {
    fn default() -> Self {
        Self::new()
    }
}

impl CapcoRuleSet {
    pub fn new() -> Self {
        use crate::rules_declarative::{
            DeclarativeAeaNofornRule, DeclarativeBareHcsRule, DeclarativeCnwdiConstraintRule,
            DeclarativeCominglingWarningRule, DeclarativeDualClassificationRule,
            DeclarativeJointHcsRule, DeclarativeJointRelToRule, DeclarativeJointRestrictedRule,
            DeclarativeNonUsMissingDissemRule, DeclarativeRdPrecedenceRule,
            DeclarativeUcniClassificationRule,
        };
        Self {
            rules: vec![
                Box::new(PortionMarkInBannerRule),
                Box::new(PortionAbbreviationRule),
                Box::new(PreferBannerAbbreviationRule),
                Box::new(BannerConsistentFormRule),
                Box::new(MissingUsaTrigraphRule),
                Box::new(MisorderedBlocksRule),
                Box::new(SeparatorCountRule),
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
                // JOINT exclusion §H.3 line 4146 actually names).
                Box::new(DeclarativeBareHcsRule),
                Box::new(MissingNonUsPrefix),
                Box::new(DeclarativeDualClassificationRule),
                Box::new(DelimiterMismatchRule),
                Box::new(DeclarativeCominglingWarningRule),
                Box::new(DeclarativeJointRelToRule),
                Box::new(DeclarativeNonUsMissingDissemRule),
                Box::new(NonIcInClassifiedBannerRule),
                Box::new(DeclarativeJointRestrictedRule),
                Box::new(DeclarativeJointHcsRule),
                Box::new(CountryCodeOrderingRule),
                Box::new(DeclarativeAeaNofornRule),
                Box::new(DeclarativeCnwdiConstraintRule),
                Box::new(SigmaValidationRule),
                Box::new(DeclarativeRdPrecedenceRule),
                Box::new(DeclarativeUcniClassificationRule),
                Box::new(SarPortionFormRule),
                Box::new(SarClassificationRule),
                Box::new(SarProgramOrderRule),
                Box::new(SarCompartmentOrderRule),
                Box::new(SarIndicatorRepeatRule),
                Box::new(SarBannerRollupRule),
                Box::new(SciSystemOrderRule),
                Box::new(SciCompartmentOrderRule),
                Box::new(SciCustomControlInfoRule),
                Box::new(SciBannerRollupRule),
            ],
        }
    }
}

impl RuleSet for CapcoRuleSet {
    fn rules(&self) -> &[Box<dyn Rule>] {
        &self.rules
    }

    fn schema_version(&self) -> &'static str {
        crate::SCHEMA_VERSION
    }
}

// ---------------------------------------------------------------------------
// Rule: E001 — Portion mark used in banner (correctness)
// ---------------------------------------------------------------------------

/// Portion marks must not appear in banner lines. CAPCO defines three forms
/// per marking (Marking Title / Banner Line Abbreviation / Portion Mark — see
/// §H.8 / §H.9 per-marking entries); banners permit the first two but not the
/// third. Portion marks that happen to equal the banner abbreviation (e.g.,
/// SBU, LES, SSI, FISA where all forms are identical) do not fire this rule
/// because no substitution is needed or possible.
///
/// This is a **correctness** rule — the fix is non-negotiable, the portion
/// form is categorically wrong in a banner. A parallel style rule (`S001`
/// `prefer-banner-abbreviation`, deferred to T035c-1b) will cover the
/// complementary case of long "Marking Title" forms in banners where the
/// user has authored-but-unidiomatic text.
struct PortionMarkInBannerRule;

impl Rule for PortionMarkInBannerRule {
    fn id(&self) -> RuleId {
        RuleId::new("E001")
    }
    fn name(&self) -> &'static str {
        "portion-mark-in-banner"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;
        if ctx.marking_type != MarkingType::Banner {
            return vec![];
        }
        let mut diagnostics = Vec::new();
        // Walk dissem-control token spans in document order. For each one
        // whose CVE portion form has a distinct banner abbreviation, check
        // whether the SOURCE BYTES are the portion form. The parser also
        // accepts the banner abbreviation via `parse_dissem_full_form`, so
        // a banner already carrying the abbreviation is skipped.
        //
        // `portion_to_banner` (see `marque_ism::marking_forms`) returns the
        // banner abbreviation (NOT the long Marking Title), so the fix
        // target is already correct for this rule. The module's `banner`
        // column name is historical; it stores the abbreviation.
        let dissem_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::DissemControl)
            .collect();
        for (idx, control) in attrs.dissem_controls.iter().enumerate() {
            let Some(banner_abbrev) =
                marque_ism::marking_forms::portion_to_banner(control.as_str())
            else {
                // portion form == banner abbreviation (e.g., FISA, RELIDO)
                // — no substitution possible. Rule does not fire.
                continue;
            };
            // The Nth dissem token span corresponds to the Nth dissem
            // control entry — both vectors are in document order.
            let Some(token_span) = dissem_spans.get(idx) else {
                continue;
            };
            let portion = control.as_str();
            // Only fire when the literal source text is the portion form.
            // A banner containing "NOFORN" parses to DissemControl::Nf but
            // token_span.text is "NOFORN" — skip it (already correct).
            if token_span.text.as_ref() != portion {
                continue;
            }
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span: token_span.span,
                message: format!(
                    "banner contains portion mark {portion:?} for an IC dissem control; \
                     use banner abbreviation {banner_abbrev:?}"
                ),
                citation: "CAPCO-2016 §H.8",
                original: portion.to_owned(),
                replacement: banner_abbrev.to_owned(),
                confidence: 1.0,
                migration_ref: None,
            }));
        }
        // Walk non-IC dissem token spans. Same logic as the IC branch: the
        // portion form (e.g., "DS" for LIMDIS, "XD" for EXDIS) must be
        // replaced with the banner abbreviation.
        let nic_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::NonIcDissem)
            .collect();
        for (idx, nic) in attrs.non_ic_dissem.iter().enumerate() {
            let Some(banner_abbrev) =
                marque_ism::marking_forms::portion_to_banner(nic.portion_str())
            else {
                // banner abbreviation == portion form (e.g., SBU, LES, SSI)
                // — no substitution possible. Rule does not fire.
                continue;
            };
            let Some(token_span) = nic_spans.get(idx) else {
                continue;
            };
            let portion = nic.portion_str();
            if token_span.text.as_ref() != portion {
                continue;
            }
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span: token_span.span,
                message: format!(
                    "banner contains portion mark {portion:?} for a non-IC dissem control; \
                     use banner abbreviation {banner_abbrev:?}"
                ),
                citation: "CAPCO-2016 §H.9",
                original: portion.to_owned(),
                replacement: banner_abbrev.to_owned(),
                confidence: 1.0,
                migration_ref: None,
            }));
        }
        diagnostics
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
/// trigraphs. That canonicalization aligns the output with line 3714:
///
/// - Line 3714: "After 'USA', list the required one or more trigraph country
///   codes in alphabetical order followed by tetragraph codes listed in
///   alphabetical order. Each code is separated by a comma and a space."
///
/// E002 does not, by itself, detect line-3714 ordering errors when `USA` is
/// already present and first; those cases are handled by E020. The 0.97
/// confidence is predicated on single-pass canonicalization so an E002 fix
/// does not leave behind a latent alphabetical-ordering violation for a
/// second pass.
///
/// Scope boundaries:
/// - Tetragraph alphabetization is deferred: `Trigraph` is 3-byte only
///   (see `marque_ism::Trigraph` doc). When the broader `CountryCode` type
///   lands, E002 should be extended to sort trigraphs before tetragraphs
///   per line 3714.
/// - "REL TO USA" alone (line 3715, a non-authorized marking with no
///   following country codes) is out of scope. E002 does not fire when
///   USA is present and first; a separate rule is needed for that case.
struct MissingUsaTrigraphRule;

impl Rule for MissingUsaTrigraphRule {
    fn id(&self) -> RuleId {
        RuleId::new("E002")
    }
    fn name(&self) -> &'static str {
        "missing-usa-trigraph"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if attrs.rel_to.is_empty() {
            return vec![];
        }

        let has_usa = attrs.rel_to.contains(&marque_ism::Trigraph::USA);
        let usa_first = attrs
            .rel_to
            .first()
            .is_some_and(|t| *t == marque_ism::Trigraph::USA);

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
        // content we can't tokenize as a trigraph today (tetragraphs
        // are 4-byte and don't fit `Trigraph`; deleting them would be
        // wrong).
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
        // alphabetical per CAPCO-2016 §H.8 line 3714) via the shared
        // helper used by E020. When USA is missing from input we add
        // it before canonicalizing so the output always has USA first;
        // the helper itself treats USA as "first if present" without
        // injecting it (E020 must not synthesize countries that aren't
        // there). Producing the canonical form in a single pass is
        // required because E020 gates on `rel_to[0] == USA` and is
        // therefore silent whenever E002 fires.
        let mut codes: Vec<marque_ism::Trigraph> = attrs.rel_to.to_vec();
        if !has_usa {
            codes.push(marque_ism::Trigraph::USA);
        }
        // E002 is REL TO only; pass `usa_first: true` per §H.8 line 3714.
        let fixed = canonicalize_trigraph_list(&codes, true).join(", ");

        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: self.id(),
            severity: self.default_severity(),
            source: FixSource::BuiltinRule,
            span,
            message: message.to_owned(),
            citation,
            original: current,
            replacement: fixed,
            confidence: 0.97, // per spec T031
            migration_ref: None,
        })]
    }
}

// ---------------------------------------------------------------------------
// Rule: E003 — Misordered banner blocks
// ---------------------------------------------------------------------------

/// CAPCO requires the order:
/// `Classification // SCI controls // SAR identifiers // Dissem (incl REL TO)`
///
/// E003 fires when the order is violated for a banner or portion marking.
/// Confidence 0.6 — kept as a suggestion under the default 0.95 threshold
/// because reordering changes byte spans across the whole marking.
struct MisorderedBlocksRule;

impl Rule for MisorderedBlocksRule {
    fn id(&self) -> RuleId {
        RuleId::new("E003")
    }
    fn name(&self) -> &'static str {
        "misordered-blocks"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;
        if !matches!(ctx.marking_type, MarkingType::Banner | MarkingType::Portion) {
            return vec![];
        }

        // Walk token kinds in document order, ignoring separators. Map each
        // kind to a CAPCO ordinal per §A.6 lines 781-841:
        //   0=Class, 1=SCI, 2=SAR, 3=AEA, 4=FGI, 5=Dissem/RelTo, 6=NonIC.
        // Any descending step is a violation.
        let kinds: Vec<u8> = attrs
            .token_spans
            .iter()
            .filter_map(|t| ordinal_for_block(t.kind))
            .collect();

        if kinds.len() < 2 {
            return vec![];
        }
        let mut max_seen = kinds[0];
        let mut violation = false;
        for &k in &kinds[1..] {
            if k < max_seen {
                violation = true;
                break;
            }
            max_seen = max_seen.max(k);
        }
        if !violation {
            return vec![];
        }

        // Span: the whole marking (first → last block-bearing token).
        let first = attrs
            .token_spans
            .iter()
            .find(|t| ordinal_for_block(t.kind).is_some())
            .map(|t| t.span);
        let last = attrs
            .token_spans
            .iter()
            .rev()
            .find(|t| ordinal_for_block(t.kind).is_some())
            .map(|t| t.span);
        let span = match (first, last) {
            (Some(f), Some(l)) => Span::new(f.start, l.end),
            _ => return vec![],
        };

        // T032: emit a FixProposal with confidence 0.6. Below the default
        // 0.95 threshold so it stays a suggestion, but present in the
        // diagnostic stream so consumers (Phase 5 corrections, lower-
        // threshold runs, IDE quick-fix surfaces) can act on it.
        //
        // `original` is left empty: the engine splices by `span` alone and
        // never reads `FixProposal.original`, so the field is a cosmetic
        // audit display only. A prior reconstruction that joined token
        // texts dropped the "REL TO " prefix for REL TO blocks (because
        // the parser stores individual trigraph spans without the block
        // prefix), producing a string that did NOT match the actual source
        // bytes at `span`. An empty original is unambiguously "unknown at
        // this layer"; consumers that need the original bytes should read
        // `source[span.start..span.end]` from the authoritative buffer.
        let reordered = reorder_marking(attrs);
        let fix = reordered.map(|replacement| {
            FixProposal::new(
                self.id(),
                FixSource::BuiltinRule,
                span,
                String::new(),
                replacement,
                marque_rules::Confidence::strict(0.6),
                None,
            )
        });

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "marking blocks are out of CAPCO order \
             (expected: Classification // SCI // SAR // AEA // FGI // \
             Dissem // REL TO // Non-IC)",
            "CAPCO-2016 §A.6 p15-16",
            fix,
        )]
    }
}

/// Map a `TokenKind` to the CAPCO §A.6 block ordinal, or `None` for tokens
/// that don't participate in block ordering (separators, declass dates,
/// unknown tokens).
///
/// CAPCO §A.6 lines 770-841 define seven ordered marking blocks, starting
/// with the classification (lines 770-779):
///
/// | Ordinal | Block                          | §A.6 line |
/// |---------|--------------------------------|-----------|
/// | 0       | US / Non-US / JOINT Classification | 770-779 |
/// | 1       | SCI Control Systems            | 781       |
/// | 2       | Special Access Programs (SAR)  | 802       |
/// | 3       | Atomic Energy Act (AEA)        | 818       |
/// | 4       | Foreign Government Information (FGI) | 823  |
/// | 5       | Dissemination Controls / REL TO | 830      |
/// | 6       | Non-IC Dissemination Controls  | 837       |
///
/// T035c-3 added ordinals for AEA and FGI, which the earlier mapping
/// skipped. Without them, common misorderings like `SECRET//REL TO USA//RD`
/// (Dissem before AEA) and `SECRET//REL TO USA//FGI GBR` (Dissem before
/// FGI) went unflagged — the AEA and FGI tokens returned `None` and their
/// positions weren't compared against the running max.
fn ordinal_for_block(kind: TokenKind) -> Option<u8> {
    match kind {
        TokenKind::Classification => Some(0),
        TokenKind::SciControl => Some(1),
        TokenKind::SarIndicator => Some(2),
        TokenKind::AeaMarking => Some(3),
        TokenKind::FgiMarker => Some(4),
        TokenKind::DissemControl | TokenKind::RelToTrigraph => Some(5),
        // Non-IC dissem always comes after IC dissem (last block).
        TokenKind::NonIcDissem => Some(6),
        // Separators, declass, and unknown tokens do not participate in
        // ordering — they belong to other blocks or other rules.
        _ => None,
    }
}

/// Rebuild a marking string from `attrs.token_spans`, ordered by CAPCO
/// §A.6 block ordinals: Classification // SCI // SAR // AEA // FGI //
/// Dissem // REL TO // Non-IC.
///
/// Within each block, tokens preserve their document order. REL TO trigraphs
/// are reassembled into a single `REL TO ...` block. AEA markings (RD, FRD,
/// TFNI, UCNI) appear per §A.6 line 818; FGI tokens per §A.6 line 823-828.
/// Non-IC dissem controls appear last per §A.6 line 837. Returns `None` if
/// there is nothing meaningful to reorder (no classification recorded).
///
/// This is the suggestion path for E003 (T032). It is not byte-equivalent to
/// the original markup whitespace, but it is a valid CAPCO marking that the
/// engine could splice if a caller lowers the threshold below 0.6.
fn reorder_marking(attrs: &IsmAttributes) -> Option<String> {
    // Group token texts by ordinal, preserving document order.
    let mut classification: Vec<&str> = Vec::new();
    let mut sci: Vec<&str> = Vec::new();
    let mut aea: Vec<&str> = Vec::new();
    let mut fgi: Vec<&str> = Vec::new();
    let mut dissem: Vec<&str> = Vec::new();
    let mut rel_to: Vec<&str> = Vec::new();
    let mut non_ic: Vec<&str> = Vec::new();

    for token in attrs.token_spans.iter() {
        match token.kind {
            TokenKind::Classification => classification.push(token.text.as_ref()),
            TokenKind::SciControl => sci.push(token.text.as_ref()),
            TokenKind::AeaMarking => aea.push(token.text.as_ref()),
            TokenKind::FgiMarker => fgi.push(token.text.as_ref()),
            TokenKind::DissemControl => dissem.push(token.text.as_ref()),
            TokenKind::RelToTrigraph => rel_to.push(token.text.as_ref()),
            TokenKind::NonIcDissem => non_ic.push(token.text.as_ref()),
            // SAR tokens are collected via attrs.sar_markings below; skip
            // individual SAR token kinds to avoid duplicating or truncating
            // compartment/sub-compartment data.
            _ => {}
        }
    }

    if classification.is_empty() {
        return None;
    }

    let mut blocks: Vec<String> = Vec::with_capacity(8);
    blocks.push(classification.join(" "));
    if !sci.is_empty() {
        blocks.push(sci.join("/"));
    }
    // Build the SAR block from the parsed structure so that program
    // identifiers, compartments, and sub-compartments are all preserved.
    if let Some(sar) = attrs.sar_markings.as_ref() {
        blocks.push(render_sar_block(sar.indicator, &sar.programs));
    }
    if !aea.is_empty() {
        // §A.6 line 820: multiple AEA markings separated by `/`.
        blocks.push(aea.join("/"));
    }
    if !fgi.is_empty() {
        // §A.6 line 824: multiple FGI trigraph/tetragraph codes
        // separated by a single space. In practice `attrs.fgi_marker`
        // is Option<_>, so a single banner has at most one FGI token
        // span with the full `FGI GBR JPN NATO` text; the space join
        // handles any future multi-token representation.
        blocks.push(fgi.join(" "));
    }
    if !dissem.is_empty() {
        blocks.push(dissem.join("/"));
    }
    if !rel_to.is_empty() {
        blocks.push(format!("REL TO {}", rel_to.join(", ")));
    }
    if !non_ic.is_empty() {
        blocks.push(non_ic.join("/"));
    }

    let joined = blocks.join("//");
    // Portion spans exclude the outer parentheses, so the replacement must
    // be the inner marking text only (no wrapping parens) to avoid producing
    // `((…))` when the fix proposal is spliced back into the original source.
    Some(joined)
}

// ---------------------------------------------------------------------------
// Rule: E004 — Wrong separator: `//` between categories, `/` within a category
// ---------------------------------------------------------------------------

/// E004 detects two distinct separator errors, each with its own
/// authoritative source in CAPCO-2016:
///
/// 1. **Redundant `////+` runs** — CAPCO-2016 §D.1 line 558: "No slashes,
///    hyphens or spaces are used to hold the place of control marking
///    categories when the control marking is not represented in a
///    document." Back-to-back `//` separators imply a missing category
///    between them, which is explicitly disallowed.
///
/// 2. **`//` between same-category values** — CAPCO-2016 §A.6
///    (Formatting, Figure 2). Within-category sibling values are joined
///    by `/`, not `//`. The per-category statements are at lines 319
///    (SCI), 328 (SAP), 330 (AEA), 334 (Dissem), and 336 (Non-IC
///    Dissem). FGI is deliberately excluded from this check because
///    §A.6 line 332 mandates a SPACE (not `/`) between multiple FGI
///    codes — an E004 fix proposing `/` would be wrong for FGI, so
///    `SeparatorCategory` omits it and `category_of` returns `None`
///    for FGI tokens.
///
/// Both branches are gated against double-firing on the same span: the
/// same-category branch skips separators that are part of a `////+` run
/// (owned by branch 1), and branch 1 only emits one diagnostic per
/// run-pair (consecutive windows).
struct SeparatorCountRule;

impl Rule for SeparatorCountRule {
    fn id(&self) -> RuleId {
        RuleId::new("E004")
    }
    fn name(&self) -> &'static str {
        "separator-count"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        // === Extra separators (`////` or longer runs) ===
        // Adjacent separator spans (back-to-back `//` with nothing between)
        // indicate `///+` runs. Phase 3 records every separator span via the
        // parser; we walk consecutive pairs and emit E004 when their gap
        // is empty.
        let seps: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Separator)
            .collect();
        for window in seps.windows(2) {
            let a = window[0].span;
            let b = window[1].span;
            if b.start == a.end {
                // `////` — back-to-back separators with no block between.
                let span = Span::new(a.start, b.end);
                let original = "//".repeat((span.end - span.start) / 2);
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::BuiltinRule,
                    span,
                    message: "redundant block separator: collapse to a single `//`".to_owned(),
                    citation: "CAPCO-2016 §D.1 (Banner Line Syntax, line 558)",
                    original,
                    replacement: "//".to_owned(),
                    confidence: 0.99,
                    migration_ref: None,
                }));
            }
        }

        // === Same-category `//` between sibling values ===
        // Per CAPCO-2016 §A.6 Figure 2, `/` is the within-category separator
        // and `//` is the between-category separator. When a user writes
        // `SECRET//SI//TK//NOFORN`, SI and TK are both SCI controls and must
        // be joined with `/` (→ `SECRET//SI/TK//NOFORN`). We detect this by
        // walking each `Separator` and checking whether the token
        // immediately before and immediately after resolve to the same
        // CAPCO category. If either side is Unknown/unclassifiable or they
        // belong to different categories, we do not fire — that avoids
        // double-flagging legitimately different blocks.
        let spans = &attrs.token_spans;
        for (idx, tok) in spans.iter().enumerate() {
            if tok.kind != TokenKind::Separator {
                continue;
            }
            // Skip separators that are part of a `////+` run — those are
            // owned by the redundant-separator branch above, and emitting
            // a same-category diagnostic here would double-fire.
            let prev_sep_adjacent = idx > 0
                && spans[idx - 1].kind == TokenKind::Separator
                && spans[idx - 1].span.end == tok.span.start;
            let next_sep_adjacent = spans
                .get(idx + 1)
                .is_some_and(|n| n.kind == TokenKind::Separator && n.span.start == tok.span.end);
            if prev_sep_adjacent || next_sep_adjacent {
                continue;
            }
            // Previous non-separator token.
            let prev = spans[..idx]
                .iter()
                .rev()
                .find(|t| t.kind != TokenKind::Separator);
            // Next non-separator token.
            let next = spans[idx + 1..]
                .iter()
                .find(|t| t.kind != TokenKind::Separator);
            let (Some(prev), Some(next)) = (prev, next) else {
                continue;
            };
            let Some(a) = category_of(prev.kind) else {
                continue;
            };
            let Some(b) = category_of(next.kind) else {
                continue;
            };
            if a != b {
                continue;
            }
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span: tok.span,
                message: "redundant block separator: consecutive same-category \
                         values must be joined with `/`, not `//`"
                    .to_owned(),
                citation: "CAPCO-2016 §A.6 (Formatting, Figure 2)",
                original: "//".to_owned(),
                replacement: "/".to_owned(),
                confidence: 0.95,
                migration_ref: None,
            }));
        }

        diagnostics
    }
}

/// CAPCO marking category — used by E004 to detect `//` between values that
/// belong to the same category and should have been joined with `/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeparatorCategory {
    Sci,
    Dissem,
    NonIcDissem,
    Aea,
    Sar,
    RelTo,
}

fn category_of(kind: TokenKind) -> Option<SeparatorCategory> {
    match kind {
        TokenKind::SciControl
        | TokenKind::SciSystem
        | TokenKind::SciCompartment
        | TokenKind::SciSubCompartment => Some(SeparatorCategory::Sci),
        TokenKind::DissemControl => Some(SeparatorCategory::Dissem),
        TokenKind::NonIcDissem => Some(SeparatorCategory::NonIcDissem),
        TokenKind::AeaMarking => Some(SeparatorCategory::Aea),
        TokenKind::SarProgram
        | TokenKind::SarCompartment
        | TokenKind::SarSubCompartment
        | TokenKind::SarIndicator => Some(SeparatorCategory::Sar),
        TokenKind::RelToTrigraph | TokenKind::RelToBlock => Some(SeparatorCategory::RelTo),
        _ => None,
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
struct DeclassifyMisplacedRule;

impl Rule for DeclassifyMisplacedRule {
    fn id(&self) -> RuleId {
        RuleId::new("E005")
    }
    fn name(&self) -> &'static str {
        "declassify-misplaced"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
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

        vec![Diagnostic::new(
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

impl Rule for DeprecatedDissemRule {
    fn id(&self) -> RuleId {
        RuleId::new("E006")
    }
    fn name(&self) -> &'static str {
        "deprecated-dissem"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
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

impl Rule for XShorthandDateRule {
    fn id(&self) -> RuleId {
        RuleId::new("E007")
    }
    fn name(&self) -> &'static str {
        "x-shorthand-date"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
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
/// Authority: CAPCO-2016 §G.1 (Register of Authorized Markings, line 748):
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

impl Rule for UnknownTokenRule {
    fn id(&self) -> RuleId {
        RuleId::new("E008")
    }
    fn name(&self) -> &'static str {
        "unrecognized-token"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
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
                    "CAPCO-2016 §G.1 (Register of Authorized Markings, line 748)",
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

impl Rule for CorrectionsMapRule {
    fn id(&self) -> RuleId {
        RuleId::new("C001")
    }
    fn name(&self) -> &'static str {
        "corrections-map"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
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

/// E009: Portion markings must use abbreviated forms, not banner-style
/// expansions.
///
/// Mirror of E001: whereas E001 catches portion abbreviations in banners
/// (e.g., `NF` → `NOFORN`), E009 catches banner expansions in portions
/// (e.g., `NOFORN` → `NF`, `SECRET` → `S`).
///
/// Authority chain: CAPCO-2016 §G.1 line 748 ("All markings used in a
/// banner line and portion mark must be in accordance with the values
/// listed in the Register") + Table 4 / §H per-template entries, which
/// list three forms per marking (Banner Line Marking Title, Banner Line
/// Abbreviation, Authorized Portion Mark). This rule specifically
/// detects portion text matching banner-form classification strings
/// (for US classifications the title and banner abbreviation coincide,
/// e.g., `SECRET`) or banner-form dissem abbreviations (e.g., `NOFORN`,
/// `ORCON`, `LIMDIS`) — both authorized only in a banner line, not a
/// portion mark. Long dissem marking titles (e.g., `ORIGINATOR
/// CONTROLLED`) are out of scope today: the dissem branch keys on
/// `marking_forms::banner_to_portion()` which only indexes banner
/// abbreviations, and the parser does not accept long titles in either
/// banners or portions on this branch. Adding title-form coverage is a
/// follow-up once the parser and `marking_forms` lookup grow a
/// title column. Branch citations match E001's per-branch convention:
///
/// - **Classification**: CAPCO-2016 §H.1 (US Classification Markings,
///   Authorized Portion Mark per template). E.g., TOP SECRET→TS
///   (p47 line 988), SECRET→S (p48), CONFIDENTIAL→C (p50 line 1074),
///   UNCLASSIFIED→U (p51 line 1114).
/// - **Dissem controls**: CAPCO-2016 §H.8 (Authorized Portion Mark per
///   template). E.g., NOFORN→NF, ORCON→OC.
/// - **Non-IC dissem controls**: CAPCO-2016 §H.9 (Authorized Portion
///   Mark per template). E.g., LIMDIS→DS. SBU/LES/SSI are skipped
///   because their banner and portion forms are identical, so no
///   substitution is possible.
///
/// Data sources:
/// - Classification: `Classification::banner_str()` / `portion_str()` (hand-written in marque-ism)
/// - Dissem controls: `marking_forms::banner_to_portion()` (inverse of E001's path)
/// - Non-IC dissem: `NonIcDissem::banner_str()` / `portion_str()` with
///   equal-form guard
struct PortionAbbreviationRule;

impl Rule for PortionAbbreviationRule {
    fn id(&self) -> RuleId {
        RuleId::new("E009")
    }
    fn name(&self) -> &'static str {
        "portion-abbreviation"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        let mut diagnostics = Vec::new();

        // --- Classification: banner form in portion → abbreviate ---
        // E009 only handles US classification; non-US/NATO/JOINT have their
        // own banner↔portion rules that will be added with those systems.
        if let Some(classification) = attrs.us_classification() {
            let banner = classification.banner_str();
            if let Some(token_span) = attrs
                .token_spans
                .iter()
                .find(|t| t.kind == TokenKind::Classification)
            {
                // Only fire when the source text is the banner form.
                // A portion containing "S" parses to Classification::Secret
                // but token_span.text is "S" — skip it.
                if token_span.text.as_ref() == banner {
                    let portion = classification.portion_str();
                    diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                        rule: self.id(),
                        severity: self.default_severity(),
                        source: FixSource::BuiltinRule,
                        span: token_span.span,
                        message: format!(
                            "portion uses banner-form classification {banner:?}; use {portion:?}"
                        ),
                        citation: "CAPCO-2016 §H.1 (US Classification Markings)",
                        original: banner.to_owned(),
                        replacement: portion.to_owned(),
                        confidence: 1.0,
                        migration_ref: None,
                    }));
                }
            }
        }

        // --- Dissem controls: banner form in portion → abbreviate ---
        // Walk dissem-control token spans. For each one whose source text
        // is a known banner form, suggest the portion abbreviation.
        // Mapping sourced from `marque_ism::marking_forms`.
        let dissem_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::DissemControl)
            .collect();
        for (idx, _control) in attrs.dissem_controls.iter().enumerate() {
            let Some(token_span) = dissem_spans.get(idx) else {
                continue;
            };
            let text = token_span.text.as_ref();
            let Some(portion) = marque_ism::marking_forms::banner_to_portion(text) else {
                continue;
            };
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span: token_span.span,
                message: format!(
                    "portion uses banner-form dissem control {text:?}; use {portion:?}"
                ),
                citation: "CAPCO-2016 §H.8",
                original: text.to_owned(),
                replacement: portion.to_owned(),
                confidence: 1.0,
                migration_ref: None,
            }));
        }

        // --- Non-IC dissem controls: banner form in portion → abbreviate ---
        let non_ic_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::NonIcDissem)
            .collect();
        for (idx, control) in attrs.non_ic_dissem.iter().enumerate() {
            let Some(token_span) = non_ic_spans.get(idx) else {
                continue;
            };
            let text = token_span.text.as_ref();
            let banner = control.banner_str();
            let portion = control.portion_str();
            // Only fire when banner and portion forms differ and source text
            // is the banner form.
            if banner != portion && text == banner {
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::BuiltinRule,
                    span: token_span.span,
                    message: format!(
                        "portion uses banner-form non-IC dissem {text:?}; use {portion:?}"
                    ),
                    citation: "CAPCO-2016 §H.9",
                    original: text.to_owned(),
                    replacement: portion.to_owned(),
                    confidence: 1.0,
                    migration_ref: None,
                }));
            }
        }

        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Rule: S001 — prefer-banner-abbreviation (style)
// ---------------------------------------------------------------------------

/// S001: Prefer the Banner Line Abbreviation over the long "Marking Title"
/// form inside a banner line.
///
/// CAPCO-2016 §A.6 line 317 authorizes both forms:
///
/// > Any control markings in the banner line may be spelled out per the
/// > "Marking Title" (e.g., TALENT KEYHOLE) or abbreviated as per the
/// > "Authorized Abbreviation" (e.g., TK) in accordance with the Register,
/// > unless otherwise directed by IC element policy or procedures to use
/// > one form over the other.
///
/// Both forms are legal; neither is canonically required at the CAPCO
/// level. S001 encodes the common IC-element preference for the shorter
/// Banner Line Abbreviation — shorter markings are easier to scan and
/// keep banners on a single line. This is a **style** rule (severity
/// `Info` by default), not a correctness rule: the diagnostic is informative
/// and the fix is non-destructive (abbreviation and title refer to the
/// same marking per §G.1 Table 4).
///
/// Rows where the Register lists no distinct abbreviation
/// (`DEA SENSITIVE` — §G.1 Table 4 line 831 shows `None` under the
/// abbreviation column) are skipped: no substitution is possible.
///
/// Complementary rules:
/// - **E001** (`portion-mark-in-banner`, correctness) — catches the
///   portion abbreviation in a banner (`NF`), which is categorically wrong.
/// - **E009** (`portion-abbreviation`, correctness) — catches banner or
///   title forms in a portion, which are categorically wrong.
/// - **S002** (`banner-consistent-form`, style, T035c-8) — catches
///   banners that mix long-title and abbreviation forms.
struct PreferBannerAbbreviationRule;

impl Rule for PreferBannerAbbreviationRule {
    fn id(&self) -> RuleId {
        RuleId::new("S001")
    }
    fn name(&self) -> &'static str {
        "prefer-banner-abbreviation"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;
        if ctx.marking_type != MarkingType::Banner {
            return vec![];
        }

        let mut diagnostics = Vec::new();
        let citation = "CAPCO-2016 §A.6 line 317 + §G.1 Table 4";

        // IC dissem block — scan each DissemControl span for a long-title
        // match in MARKING_FORMS. `title_to_banner` gates on `title !=
        // banner`, so the DEA-SENSITIVE row is correctly skipped.
        let dissem_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::DissemControl)
            .collect();
        for token_span in &dissem_spans {
            let text = token_span.text.as_ref();
            let Some(abbrev) = marque_ism::marking_forms::title_to_banner(text) else {
                continue;
            };
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span: token_span.span,
                message: format!(
                    "banner uses long-title dissem form {text:?}; prefer \
                     banner abbreviation {abbrev:?}"
                ),
                citation,
                original: text.to_owned(),
                replacement: abbrev.to_owned(),
                confidence: 1.0,
                migration_ref: None,
            }));
        }

        // Non-IC dissem block — same pattern.
        let non_ic_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::NonIcDissem)
            .collect();
        for token_span in &non_ic_spans {
            let text = token_span.text.as_ref();
            let Some(abbrev) = marque_ism::marking_forms::title_to_banner(text) else {
                continue;
            };
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span: token_span.span,
                message: format!(
                    "banner uses long-title non-IC dissem form {text:?}; \
                     prefer banner abbreviation {abbrev:?}"
                ),
                citation,
                original: text.to_owned(),
                replacement: abbrev.to_owned(),
                confidence: 1.0,
                migration_ref: None,
            }));
        }

        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Rule: S002 — banner-consistent-form (style)
// ---------------------------------------------------------------------------

/// S002: A banner line should use a single form consistently for its dissem
/// and non-IC dissem entries — either all long "Marking Titles" or all
/// Banner Line Abbreviations, but not a mix of both within the same banner.
///
/// Both forms are legal per CAPCO-2016 §A.6 line 317 ("may be spelled out
/// per the 'Marking Title' ... or abbreviated as per the 'Authorized
/// Abbreviation' ... unless otherwise directed by IC element policy"), and
/// §G.1 Table 4 lists both columns per marking. Neither CAPCO nor the
/// Register prescribes consistency within a single banner — this rule
/// encodes the readability convention followed by most IC-element
/// style guides: a reader scanning a banner shouldn't have to context-
/// switch between "NOFORN" and "CAUTION-PROPRIETARY INFORMATION INVOLVED"
/// in the same line.
///
/// Severity: `Info`. Style guidance, not correctness.
///
/// # Scoring
///
/// For each dissem / non-IC token in the banner, classify by its
/// `MARKING_FORMS` match:
///
/// - **title-form**: source text equals a `title` of a row where
///   `title != banner` (long form used where an abbreviation exists).
/// - **abbrev-form**: source text equals a `banner` of a row where
///   `title != banner` (abbreviation used where a distinct long form
///   exists).
/// - **same-form**: `title == banner` (e.g., `DEA SENSITIVE`) — the
///   marking has only one form; excluded from the count.
/// - **other**: token not in `MARKING_FORMS` with a distinct title
///   (e.g., `RELIDO`, `HCS`, `FISA`) — excluded from the count.
///
/// The banner is "mixed" when `title-form count ≥ 1` AND `abbrev-form
/// count ≥ 1`. S002 fires **once per banner** with a single diagnostic
/// spanning the first title-form token. The diagnostic carries no
/// `FixProposal` — per-token normalization is S001's job, and running
/// `marque fix` with S001 enabled will drive the banner to a consistent
/// all-abbrev form, resolving S002 on the next pass.
///
/// # Relationship to S001
///
/// - S001 fires on every long-title token (mixed or not).
/// - S002 fires exactly once per banner when mixing is detected.
///
/// When a banner is mixed, both rules fire; their messages carry
/// different information (S001 says "prefer abbrev for this token",
/// S002 says "this banner has mixed forms"). Users running `marque
/// fix` see S001's fixes applied; S002's diagnostic remains visible
/// so reviewers can audit the intent.
struct BannerConsistentFormRule;

impl Rule for BannerConsistentFormRule {
    fn id(&self) -> RuleId {
        RuleId::new("S002")
    }
    fn name(&self) -> &'static str {
        "banner-consistent-form"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;
        use marque_ism::marking_forms::MARKING_FORMS;

        if ctx.marking_type != MarkingType::Banner {
            return vec![];
        }

        // Walk dissem + non-IC spans in document order. For each, check
        // MARKING_FORMS for a match where title != banner; classify the
        // token as title-form or abbrev-form. Ignore tokens that map to
        // same-form rows or aren't in the table.
        let mut first_title_span: Option<Span> = None;
        let mut first_title_text: Option<&str> = None;
        let mut first_abbrev_text: Option<&str> = None;

        for token in attrs.token_spans.iter() {
            if !matches!(
                token.kind,
                TokenKind::DissemControl | TokenKind::NonIcDissem
            ) {
                continue;
            }
            let text = token.text.as_ref();
            let Some(form) = MARKING_FORMS
                .iter()
                .find(|f| f.title != f.banner && (f.title == text || f.banner == text))
            else {
                continue;
            };
            if form.title == text {
                if first_title_span.is_none() {
                    first_title_span = Some(token.span);
                    first_title_text = Some(text);
                }
            } else if first_abbrev_text.is_none() {
                first_abbrev_text = Some(text);
            }
        }

        // Mixed only when both a title-form and an abbrev-form were
        // seen. Same-form rows (DEA SENSITIVE) and opaque tokens
        // (RELIDO) neither count nor block firing.
        let (Some(span), Some(long_text), Some(abbrev_text)) =
            (first_title_span, first_title_text, first_abbrev_text)
        else {
            return vec![];
        };

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            format!(
                "banner mixes long-title and abbreviation forms \
                 (saw {long_text:?} and {abbrev_text:?}); normalize to a \
                 single form — prefer the banner abbreviation (S001) for \
                 readability"
            ),
            "CAPCO-2016 §A.6 line 317 + §G.1 Table 4",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E011 — Missing leading // on non-US classification
// ---------------------------------------------------------------------------

/// Non-US classifications (FGI, NATO, JOINT) must start with `//` to indicate
/// the US classification slot is empty. When a marking's first block fails to
/// parse as a US classification but looks like a non-US pattern, the `//` prefix
/// is likely missing.
///
/// Example: `(GBR S//NF)` → should be `(//GBR S//NF)`
struct MissingNonUsPrefix;

impl Rule for MissingNonUsPrefix {
    fn id(&self) -> RuleId {
        RuleId::new("E011")
    }
    fn name(&self) -> &'static str {
        "missing-non-us-prefix"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        // Only fire when classification failed to parse (None) and the
        // classification token text looks like a non-US pattern.
        if attrs.classification.is_some() {
            return vec![];
        }

        let class_span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification);
        let Some(token) = class_span else {
            return vec![];
        };
        let text = token.text.as_ref();

        // Check if the text looks like a non-US classification:
        // - NATO patterns: "NATO SECRET", "NS", "COSMIC TOP SECRET", "CTS", etc.
        // - JOINT patterns: starts with "JOINT "
        // - FGI patterns: 3-letter uppercase + space + classification level
        let looks_non_us = text.starts_with("NATO ")
            || text.starts_with("COSMIC ")
            || text.starts_with("JOINT ")
            || matches!(
                text,
                "NS" | "NR" | "NC" | "NCA" | "NC-B" | "NS-BALK" | "CTS" | "CTSA" | "NU"
            )
            || looks_like_fgi_classification(text);

        if !looks_non_us {
            return vec![];
        }

        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: self.id(),
            severity: self.default_severity(),
            source: FixSource::BuiltinRule,
            span: token.span,
            message: format!(
                "non-US classification {text:?} is missing the leading //; \
                 use //{text} to indicate the US classification slot is empty"
            ),
            // §A.6 lines 771-772: "For non-US or Joint information,
            // the banner line and portion mark must always start
            // with a double forward slash ('//') with no interjected
            // space." §H.3 line 4020 reinforces for JOINT: "The
            // JOINT classification marking always starts with a
            // double forward slash ('//')."
            //
            // Earlier revisions cited §H.4, which is the SCI control
            // system section — unrelated to the non-US prefix rule.
            // T035c-7 corrected the citation to the two sections
            // that actually establish the predicate.
            citation: "CAPCO-2016 §A.6 + §H.3",
            original: text.to_owned(),
            replacement: format!("//{text}"),
            confidence: 0.95,
            migration_ref: None,
        })]
    }
}

/// Heuristic: does this string look like an FGI classification?
/// Pattern: 3 uppercase ASCII letters + space + valid classification level.
fn looks_like_fgi_classification(s: &str) -> bool {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 2 {
        return false;
    }
    // Last token (or last two for TOP SECRET) must be a classification level.
    let last = parts[parts.len() - 1];
    let is_top_secret = parts.len() >= 3 && parts[parts.len() - 2] == "TOP" && last == "SECRET";
    let is_single_token_level = matches!(
        last,
        "TS" | "S" | "C" | "R" | "U" | "SECRET" | "CONFIDENTIAL" | "RESTRICTED" | "UNCLASSIFIED"
    );
    let is_level = is_single_token_level || is_top_secret;
    if !is_level {
        return false;
    }
    // Preceding tokens should look like country trigraphs or "FGI".
    let country_end = if is_top_secret {
        parts.len() - 2
    } else {
        parts.len() - 1
    };
    parts[..country_end]
        .iter()
        .all(|t| *t == "FGI" || (t.len() == 3 && t.bytes().all(|b| b.is_ascii_uppercase())))
}

// ---------------------------------------------------------------------------
// Rule: E013 — JOINT/REL TO delimiter mismatch
// ---------------------------------------------------------------------------

/// E013 fires when a JOINT country list uses commas (should be single
/// spaces) or a REL TO country list fails to use canonical comma-space
/// delimiters between codes.
///
/// # Authority (per-template, most-specific)
///
/// - **JOINT, §H.3 p56 line 1258**: "Multiple codes are separated by a
///   single space."
/// - **REL TO, §H.8 p150–151 line 3714**: "Each code is separated by a
///   comma and a space."
///
/// The global formatting passage §A.6 p15–16 (rendered inline in the
/// vendored markdown — `## 6. (U) Formatting` starts at line 317)
/// reinforces both: FGI/JOINT-style lists use single space (line 332)
/// and REL TO uses comma-with-interjected-space (line 334). The
/// per-template sections are cited because they are the narrowest
/// authoritative passages per Constitution VIII.
///
/// # Scope
///
/// E013 targets the two most-commonly-confused delimiters. Other CAPCO
/// delimiter conventions are owned by sibling rules (SCI `/` between
/// control systems by the SCI cluster; SAR `-` by the SAR cluster; FGI
/// space handled as part of the FGI rules when they land). Keeping
/// E013 scoped to JOINT + REL TO keeps its message specific and
/// actionable.
///
/// # Predicate
///
/// - **JOINT**: fires when the classification token text for a
///   `MarkingClassification::Joint` contains any `,`. Fix replaces the
///   comma delimiters with a single space and normalizes any run of
///   whitespace.
/// - **REL TO**: fires when the RelToBlock's country-list region does
///   not match the canonical `alpha(, alpha)*` form. Split on any
///   mixture of comma-and-whitespace characters, then compare the
///   input's list region (the slice starting at the first non-keyword
///   code) against the joined-with-`", "` canonical form. This catches:
///   - space-only delimiters: `REL TO USA GBR`
///   - missing-space-after-comma: `REL TO USA,GBR`
///   - mixed delimiters: `REL TO USA GBR,AUS`
///   - trailing-delimiter artifacts inside a multi-country list
///     (e.g., `REL TO USA, GBR,` — trailing `,` between `GBR` and
///     end-of-block)
///
///   Prefix-only issues (extra whitespace between `REL` and `TO` or
///   between `TO` and the first code, when the country list itself is
///   already canonical) are deliberately out of scope. E013's message
///   describes a list-delimiter mismatch; firing on a prefix problem
///   would be misleading. When the list IS non-canonical, the fix
///   produces the full canonical block text, which incidentally
///   cleans any prefix whitespace as a side effect.
///
/// Token order is preserved in both fixes — E020
/// (country-code-ordering) owns ordering.
struct DelimiterMismatchRule;

impl Rule for DelimiterMismatchRule {
    fn id(&self) -> RuleId {
        RuleId::new("E013")
    }
    fn name(&self) -> &'static str {
        "delimiter-mismatch"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // --- JOINT: comma delimiter is wrong; canonical is single space.
        if let Some(MarkingClassification::Joint(_)) = &attrs.classification {
            if let Some(token) = attrs
                .token_spans
                .iter()
                .find(|t| t.kind == TokenKind::Classification)
            {
                let text = token.text.as_ref();
                if text.contains(',') {
                    // Replace `,` with a space and normalize whitespace.
                    // `split_whitespace().join(" ")` handles any run of
                    // whitespace and preserves the "JOINT <level> "
                    // prefix because it normalizes the whole string.
                    //
                    // Parser boundary: this branch only runs once the
                    // JOINT block has already parsed successfully, so
                    // it applies to inputs where commas coexist with
                    // whitespace token boundaries:
                    //   `USA, GBR`   → `USA GBR` (comma + trailing space)
                    //   `USA,  GBR`  → `USA GBR` (comma + extra spaces)
                    //
                    // A bare `USA,GBR` (comma, no whitespace) does NOT
                    // reach this branch: `parse_joint_classification`
                    // tokenizes on whitespace, so the list fails grammar
                    // entirely and `attrs.classification` is not
                    // `Joint(_)`. Fixing that shape would require parser-
                    // level degradation tolerance and is out of scope
                    // for this rule.
                    let fixed: String = text
                        .replace(',', " ")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                        rule: self.id(),
                        severity: self.default_severity(),
                        source: FixSource::BuiltinRule,
                        span: token.span,
                        message: "JOINT country list must be space-delimited, \
                                  not comma-delimited"
                            .to_owned(),
                        citation: "CAPCO-2016 §H.3 p56 line 1258 \
                                   (JOINT codes separated by a single space)",
                        original: text.to_owned(),
                        replacement: fixed,
                        confidence: 0.95,
                        migration_ref: None,
                    }));
                }
            }
        }

        // --- REL TO: canonical comma-space delimiter.
        if let Some(token) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::RelToBlock)
        {
            let text = token.text.as_ref();
            // Tokenize the whole block on commas + whitespace, then
            // drop the leading `REL` / `TO` keywords. This is robust
            // to non-canonical whitespace between the keywords (e.g.,
            // `REL  TO USA GBR` with a double space between REL and
            // TO). Stripping literal `"REL TO"` / `"REL"` prefixes —
            // what the earlier implementation did — would have
            // fallen through to the `"REL"` branch on double-space
            // input and left `TO` as an apparent country code,
            // producing a fix like `REL TO TO, USA, GBR`.
            //
            // `skip_while` is safe here because neither `REL` nor
            // `TO` is a valid ISO 3166 alpha-3 country code, so
            // dropping them from the list cannot remove a real
            // country. Only LEADING `REL`/`TO` tokens are dropped;
            // a pathological `USA REL TO GBR` would not be altered
            // by the skip.
            let codes: Vec<&str> = text
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .skip_while(|&s| s == "REL" || s == "TO")
                .collect();
            if codes.len() < 2 {
                // Single code (or zero) cannot have a delimiter mismatch —
                // there is nothing to separate. Trailing-delimiter
                // artifacts on a one-country list (e.g., `REL TO USA,`)
                // fall outside E013's delimiter-between-codes scope
                // and are handled by E002 in its own fix path.
                return diagnostics;
            }
            let canonical_list = codes.join(", ");
            // Compare canonical_list to the input's list region (the
            // slice starting at the first non-`REL`/`TO` code), NOT
            // the full block text. This scopes E013 to its actual name
            // — a list-delimiter mismatch — so it does not fire on
            // prefix-only whitespace issues like `REL  TO USA, GBR`
            // (double space between `REL` and `TO`, list itself
            // canonical). When E013 does fire, its fix produces the
            // full canonical block text, which incidentally cleans up
            // prefix whitespace as a side effect.
            //
            // `text.find(codes[0])` is safe here: `codes[0]` is
            // whatever survived after skipping leading `REL`/`TO`
            // keywords, and neither keyword nor any valid country
            // trigraph/tetragraph contains the other as a substring,
            // so the first occurrence is the actual list start.
            let list_start = text.find(codes[0]).unwrap_or(0);
            let list_region = &text[list_start..];
            if list_region != canonical_list {
                let canonical_text = format!("REL TO {canonical_list}");
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::BuiltinRule,
                    span: token.span,
                    message: "REL TO country list must use comma-space \
                              delimiters (\"USA, GBR\"), not plain spaces \
                              or bare commas"
                        .to_owned(),
                    citation: "CAPCO-2016 §H.8 p150-151 line 3714 \
                               (REL TO codes separated by a comma and a space)",
                    original: text.to_owned(),
                    replacement: canonical_text,
                    confidence: 0.95,
                    migration_ref: None,
                }));
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

impl Rule for NonIcInClassifiedBannerRule {
    fn id(&self) -> RuleId {
        RuleId::new("W003")
    }
    fn name(&self) -> &'static str {
        "non-ic-dissem-in-classified-banner"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
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

// ---------------------------------------------------------------------------
// Rule: E020 — Country code list ordering
// ---------------------------------------------------------------------------

/// REL TO and JOINT country lists must be alphabetically ordered.
///
/// # Authority (per-template)
///
/// - **REL TO, §H.8 p150–151 line 3714**: "After 'USA', list the
///   required one or more trigraph country codes in alphabetical
///   order followed by tetragraph codes listed in alphabetical
///   order." REL TO elevates USA to the front.
/// - **JOINT, §H.3 p56 line 1258**: "Country trigraph codes are
///   listed alphabetically followed by tetragraph codes in
///   alphabetical order." JOINT prescribes **pure alphabetical** —
///   no USA-first carve-out.
///
/// `canonicalize_trigraph_list` takes a `usa_first: bool` flag so
/// each caller selects its authoritative convention. The REL TO
/// path passes `true`; the JOINT path passes `false`.
///
/// # JOINT USA-first convention is style, not rule
///
/// The widespread IC practice of rendering USA first in JOINT
/// lists — because every other US-authored country list leads
/// with USA — is convention, not CAPCO text. E020 does NOT encode
/// it as a correctness error. A follow-up style rule
/// (S003 `joint-usa-first`, `Severity::Info`) will surface
/// deviations without conflating them with ordering violations.
///
/// # Scope
///
/// Fires on REL TO (`attrs.rel_to`) and JOINT (`attrs.classification`
/// when it is `MarkingClassification::Joint`). Does NOT currently
/// cover:
///
/// - **FGI ordering** (`attrs.fgi_marker.countries`) — §A.6 p15-16
///   line 332 establishes the same trigraph-then-tetragraph alpha
///   rule for FGI, but extending E020 to cover it is a future
///   follow-up; no FGI-ordering test fixtures exist today.
/// - **Tetragraph sorting** — `Trigraph` is a 3-byte type and cannot
///   represent 4-byte codes, so `canonicalize_trigraph_list` treats
///   every entry as a trigraph. When a broader `CountryCode` type
///   lands, the helper should sort trigraphs before tetragraphs per
///   the per-template passages above.
///
/// # Interaction with E002
///
/// REL TO ordering is skipped entirely when USA is missing or not
/// first (see `attrs.rel_to` guard). E002's fix produces the fully
/// canonical list in a single pass (USA first, non-USA entries
/// alphabetical), so E020's concern is already absorbed whenever E002
/// is active. This prevents double-firing on the same span.
///
/// This is a fixable error. Fix confidence is `1.0` for both paths —
/// the sort is deterministic with exact trigraph matches and no
/// fuzzy matching today. When fuzzy matching lands in a future
/// decoder phase, per-candidate confidence may need to plumb
/// through `check_trigraph_ordering`; this helper signature is
/// designed to accommodate that change.
struct CountryCodeOrderingRule;

impl Rule for CountryCodeOrderingRule {
    fn id(&self) -> RuleId {
        RuleId::new("E020")
    }
    fn name(&self) -> &'static str {
        "country-code-ordering"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check REL TO ordering. Skip if USA is missing or not first —
        // E002 fires for those cases and its fix produces the fully
        // canonical list (USA first, non-USA entries alphabetical per
        // CAPCO-2016 §H.8 line 3714), so E020's concern is already
        // absorbed when E002 is active.
        if attrs.rel_to.len() >= 2
            && attrs
                .rel_to
                .first()
                .is_some_and(|t| *t == marque_ism::Trigraph::USA)
        {
            // Locate the `RelToBlock` for this list. A single first→last
            // `RelToTrigraph` splice across the whole marking would
            // delete intervening `//...//` content when more than one
            // REL TO block is present (e.g.,
            // `SECRET//REL TO USA, GBR//NF//REL TO AUS`). Mirrors E002
            // (line 345) in scoping the fix to a single block and
            // suppressing it when multiple blocks are present.
            let rel_to_blocks: Vec<&TokenSpan> = attrs
                .token_spans
                .iter()
                .filter(|t| t.kind == TokenKind::RelToBlock)
                .collect();
            // `concat!` avoids any ambiguity around whether `\<newline>`
            // preserves embedded whitespace in the resulting string.
            const REL_TO_CITATION: &str = concat!(
                "CAPCO-2016 §H.8 p150-151 line 3714 ",
                "(REL TO: trigraphs alpha, then tetragraphs alpha, USA first)",
            );
            if rel_to_blocks.len() > 1 {
                // Suppress the fix rather than risk cross-block corruption.
                // Span the first block so downstream consumers have a
                // location to display.
                let actual: Vec<&str> = attrs.rel_to.iter().map(|t| t.as_str()).collect();
                // REL TO is USA-first per §H.8 line 3714.
                let sorted = canonicalize_trigraph_list(&attrs.rel_to, true);
                if actual != sorted {
                    diagnostics.push(Diagnostic::new(
                        self.id(),
                        self.default_severity(),
                        rel_to_blocks[0].span,
                        format!(
                            "REL TO country codes must be alphabetically ordered \
                             (USA first when present): [{}] → [{}] \
                             (multiple REL TO blocks present; fix suppressed to avoid \
                             cross-block corruption — resolve manually)",
                            actual.join(", "),
                            sorted.join(", "),
                        ),
                        REL_TO_CITATION,
                        None,
                    ));
                }
            } else if let Some(&block) = rel_to_blocks.first() {
                if let Some(diag) = check_trigraph_ordering(
                    &attrs.rel_to,
                    "REL TO",
                    self.id(),
                    self.default_severity(),
                    attrs,
                    Some(block.span),
                    REL_TO_CITATION,
                    true, // REL TO: USA-first per §H.8 line 3714
                ) {
                    diagnostics.push(diag);
                }
            }
            // If `rel_to_blocks` is empty while `attrs.rel_to` is
            // populated, the parser is in an inconsistent state; skip
            // silently rather than synthesize a span.
        }

        // Check JOINT country ordering. JOINT countries live inside a
        // single `Classification` token, so the multi-block concern
        // that motivates REL TO's block scoping does not apply here.
        // JOINT's ordering rule lives in §H.3 (its own template), not
        // §H.8 (REL TO's template), and §H.3 line 1258 prescribes
        // pure alphabetical order — no USA-first carve-out. The
        // widespread IC practice of rendering USA first in JOINT
        // lists is style convention, not CAPCO rule; a planned
        // follow-up S003 `joint-usa-first` style rule will surface
        // deviations without conflating them with a correctness
        // error.
        if let Some(MarkingClassification::Joint(j)) = &attrs.classification {
            if j.countries.len() >= 2 {
                const JOINT_CITATION: &str = concat!(
                    "CAPCO-2016 §H.3 p56 line 1258 ",
                    "(JOINT: trigraphs alpha, then tetragraphs alpha)",
                );
                if let Some(diag) = check_trigraph_ordering(
                    &j.countries,
                    "JOINT",
                    self.id(),
                    self.default_severity(),
                    attrs,
                    None,
                    JOINT_CITATION,
                    false, // JOINT: pure alpha per §H.3 line 1258 (no USA-first)
                ) {
                    diagnostics.push(diag);
                }
            }
        }

        diagnostics
    }
}

/// Canonicalize a country code list. The `usa_first` flag selects the
/// convention:
///
/// - `usa_first = true` — REL TO convention per CAPCO-2016 §H.8 line
///   3714: "After 'USA', list the required one or more trigraph
///   country codes in alphabetical order." USA is elevated to the
///   front when present; remaining codes are alphabetical.
/// - `usa_first = false` — JOINT convention per CAPCO-2016 §H.3 line
///   1258: "Country trigraph codes are listed alphabetically followed
///   by tetragraph codes in alphabetical order." Pure alphabetical;
///   USA is NOT elevated.
///
/// The IC practice of rendering USA first in JOINT lists is widespread
/// but is convention, not CAPCO rule. A style rule (S003
/// `joint-usa-first`) to flag deviations is a planned follow-up; this
/// helper does NOT encode the convention into correctness.
///
/// This is the shared ordering rule for E002 (REL TO, fix path) and
/// E020 (REL TO + JOINT, both check and fix paths). Extracting it
/// prevents the two rules from drifting if the ordering rule changes
/// (tetragraph sorting, delimiter normalization, etc.).
///
/// Tetragraph handling is deferred — `Trigraph` is 3-byte only today
/// and cannot represent tetragraph codes. When a broader `CountryCode`
/// type lands, this helper should be extended to sort trigraphs before
/// tetragraphs per §H.3 line 1258 and §H.8 line 3714.
fn canonicalize_trigraph_list(
    codes: &[marque_ism::Trigraph],
    usa_first: bool,
) -> Vec<&str> {
    if usa_first {
        let has_usa = codes.contains(&marque_ism::Trigraph::USA);
        let mut sorted: Vec<&str> = codes
            .iter()
            .filter(|t| **t != marque_ism::Trigraph::USA)
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

/// Check that a country code list is in the expected order.
///
/// `usa_first` selects the canonicalization convention — see
/// `canonicalize_trigraph_list` for the per-list authorities. For
/// REL TO (§H.8 line 3714), USA is elevated; for JOINT (§H.3 line
/// 1258), the order is pure alphabetical with no USA carve-out.
///
/// `block_span`, when `Some`, restricts the trigraph-token search to
/// spans that fall inside it. This is required for REL TO because a
/// marking may contain multiple `RelToBlock`s (e.g.,
/// `...REL TO USA, GBR//NF//REL TO AUS...`) and a first→last splice
/// across blocks would delete intervening `//...//` content. Callers
/// that cover a whole-marking list (JOINT sits inside a single
/// `Classification` token) pass `None`.
///
/// `citation` is caller-supplied so each list type cites its own
/// authoritative passage verbatim (Constitution VIII).
#[allow(clippy::too_many_arguments)]
fn check_trigraph_ordering(
    codes: &[marque_ism::Trigraph],
    list_name: &str,
    rule: RuleId,
    severity: Severity,
    attrs: &IsmAttributes,
    block_span: Option<Span>,
    citation: &'static str,
    usa_first: bool,
) -> Option<Diagnostic> {
    let sorted = canonicalize_trigraph_list(codes, usa_first);
    let actual: Vec<&str> = codes.iter().map(|t| t.as_str()).collect();
    if actual == sorted {
        return None;
    }

    // Compute the fix span. The kind differs by list type:
    // - REL TO: `RelToTrigraph` is one token per country, so first→last
    //   covers exactly the country-list region of the `RelToBlock`.
    //   Fix `original`/`replacement` are the joined country strings —
    //   clean splice.
    // - JOINT: the parser emits a single `Classification` token
    //   covering the whole block (e.g., `"JOINT S USA GBR AUS"`).
    //   There is no per-country sub-token. A replacement of just the
    //   joined country list would splice out the `JOINT <level>`
    //   prefix and corrupt the marking. We therefore widen the JOINT
    //   `replacement` to include the original `JOINT <level>` prefix
    //   byte-for-byte, and set `original` to the full classification
    //   token text to match `span`.
    let kind = if list_name == "REL TO" {
        TokenKind::RelToTrigraph
    } else {
        TokenKind::Classification
    };
    let matching_spans: Vec<&TokenSpan> = attrs
        .token_spans
        .iter()
        .filter(|t| {
            t.kind == kind
                && block_span.is_none_or(|b| t.span.start >= b.start && t.span.end <= b.end)
        })
        .collect();
    let span = match (matching_spans.first(), matching_spans.last()) {
        (Some(first), Some(last)) => Span::new(first.span.start, last.span.end),
        _ => Span::new(0, 0),
    };

    // Separator for the list: REL TO uses ", "; JOINT uses " ".
    let sep = if list_name == "REL TO" { ", " } else { " " };
    let joined_actual = actual.join(sep);
    let joined_sorted = sorted.join(sep);

    // Build span-matching `original` + `replacement`.
    let (original, replacement) = if list_name == "REL TO" {
        // REL TO span covers exactly the country list.
        (joined_actual.clone(), joined_sorted.clone())
    } else {
        // JOINT span covers the full Classification token. Preserve
        // the `JOINT <level>` prefix by anchoring on the first
        // source-order country's position in the token text.
        // `actual[0]` is a 3-letter trigraph; neither the keyword
        // `JOINT` nor any valid Classification-level spelling
        // (`TS`, `S`, `C`, `U`, `TOP SECRET`, `SECRET`,
        // `CONFIDENTIAL`, `UNCLASSIFIED`, `RESTRICTED`) contains a
        // trigraph as a substring, so the first occurrence of
        // `actual[0]` in the token text is the start of the country
        // list.
        let classification_text = matching_spans
            .first()
            .map(|t| t.text.as_ref())
            .unwrap_or("");
        let first_country = actual[0];
        let prefix_end = classification_text
            .find(first_country)
            .unwrap_or(classification_text.len());
        let prefix = &classification_text[..prefix_end];
        (
            classification_text.to_owned(),
            format!("{prefix}{joined_sorted}"),
        )
    };

    // Message reports the country-list delta (not the full block
    // text) so it stays readable regardless of list type. REL TO's
    // "USA first when present" clause is only correct for REL TO;
    // JOINT's pure-alpha rule has no USA carve-out in the source.
    let message = if usa_first {
        format!(
            "{list_name} country codes must be alphabetically ordered \
             (USA first when present): [{joined_actual}] → [{joined_sorted}]"
        )
    } else {
        format!(
            "{list_name} country codes must be alphabetically ordered: \
             [{joined_actual}] → [{joined_sorted}]"
        )
    };

    Some(make_fix_diagnostic(FixDiagnosticParams {
        rule,
        severity,
        source: FixSource::BuiltinRule,
        span,
        message,
        citation,
        // Fix confidence is 1.0 — the sort is deterministic with
        // exact trigraph matches (no fuzzy matching). When fuzzy
        // matching lands in a future decoder phase, callers may want
        // to plumb a lower per-candidate confidence through this
        // helper; today the value is uniformly 1.0 for all list types.
        original,
        replacement,
        confidence: 1.0,
        migration_ref: None,
    }))
}

// ---------------------------------------------------------------------------
// Rule: E023 — SIGMA valid values and numerical order
// ---------------------------------------------------------------------------

/// SIGMA compartment numbers must be from the currently authorized set
/// (14, 15, 18, 20) and listed in numerical order.
///
/// # Historical SIGMA range
///
/// CAPCO v1.2 (2008) §7 documented SIGMA as ranging from 1 to 99
/// (`crates/capco/docs/original-refs/CAPCO_v1.2_(2008).pdf`, p14 entry for
/// `-SIGMA [#]`). CAPCO v5.1 (2012) §H.6 line 4090 and CAPCO 2016 §H.6 line
/// 7129 both narrow this to "SIGMA # currently represents one or more of
/// the following numbers: 14, 15, 18, and 20." Neither manual enumerates
/// which specific values outside the current set were formally obsoleted —
/// only that the current set is the narrow four. An earlier revision of
/// this rule asserted that values `1..=5 | 9..=13` were "obsolete" while
/// `6..=8 | 16..=17 | 19 | 21..=99` were "invalid"; that bifurcation was
/// project inference, not backed by CAPCO source text. The unified
/// "not in current authorized set" message below matches what the source
/// actually says.
struct SigmaValidationRule;

impl Rule for SigmaValidationRule {
    fn id(&self) -> RuleId {
        RuleId::new("E023")
    }
    fn name(&self) -> &'static str {
        "sigma-validation"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::AeaMarking;

        let mut diagnostics = Vec::new();
        let valid_sigmas: &[u8] = &[14, 15, 18, 20];

        for aea in attrs.aea_markings.iter() {
            let sigma = match aea {
                AeaMarking::Rd(rd) => &rd.sigma,
                AeaMarking::Frd(frd) => &frd.sigma,
                _ => continue,
            };
            if sigma.is_empty() {
                continue;
            }

            let span = attrs
                .token_spans
                .iter()
                .find(|t| t.kind == TokenKind::AeaMarking)
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));

            // Check for values outside the currently authorized set.
            // Unified message (no obsolete/invalid bifurcation) — CAPCO
            // 2016 §H.6 line 7129 only names the current four, not any
            // specific obsolete subset. Contact the originating
            // program for guidance on historical SIGMA numbers (CAPCO
            // v1.2 2008 permitted 1-99).
            let invalid: Vec<u8> = sigma
                .iter()
                .filter(|n| !valid_sigmas.contains(n))
                .copied()
                .collect();
            if !invalid.is_empty() {
                diagnostics.push(Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    span,
                    format!(
                        "SIGMA {:?} not in the currently authorized set \
                         (14, 15, 18, 20); contact the originating \
                         program for guidance on historical values",
                        invalid,
                    ),
                    "CAPCO-2016 §H.6 line 7129",
                    None,
                ));
            }

            // Check numerical order.
            if sigma.len() >= 2 {
                let mut sorted = sigma.to_vec();
                sorted.sort_unstable();
                sorted.dedup();
                if sigma.as_ref() != sorted.as_slice() {
                    let original: Vec<String> = sigma.iter().map(|n| n.to_string()).collect();
                    let replacement: Vec<String> = sorted.iter().map(|n| n.to_string()).collect();
                    diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                        rule: self.id(),
                        severity: self.default_severity(),
                        source: FixSource::BuiltinRule,
                        span,
                        message: format!(
                            "SIGMA numbers must be in numerical order: {} → {}",
                            original.join(" "),
                            replacement.join(" "),
                        ),
                        // §H.6 line 7130 (RD block): "Multiple SIGMA
                        // numbers shall be listed in numerical order
                        // with a space preceding each value."
                        citation: "CAPCO-2016 §H.6 line 7130",
                        original: original.join(" "),
                        replacement: replacement.join(" "),
                        confidence: 1.0,
                        migration_ref: None,
                    }));
                }
            }
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Rule: E026 — SAR portion must use `SAR-` abbreviation
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

/// Portion marks must use the `SAR-` abbreviation, not the full
/// `SPECIAL ACCESS REQUIRED-` form.
///
/// Authority: CAPCO-2016 §H.5 p101 line 2432 — "Authorized Portion
/// Mark: SAR-[program identifier abbreviation]". The banner may use
/// either the full `SPECIAL ACCESS REQUIRED-` (line 2428) or the
/// abbreviation (line 2430); the portion entry lists the abbreviation
/// only.
///
/// When all program identifiers are already abbrev-shaped (2–3
/// alphanumeric characters per §H.5 p101 line 2454 — "A program
/// identifier abbreviation is the two or three-character designator
/// for the program"), a low-confidence (0.35) suggestion is proposed
/// to replace the full indicator with the `SAR-` prefix. Otherwise no
/// fix is proposed — abbreviating an arbitrary program nickname (e.g.,
/// `BUTTER POPCORN` → `BP`) requires a registry lookup the engine
/// does not have.
struct SarPortionFormRule;

impl Rule for SarPortionFormRule {
    fn id(&self) -> RuleId {
        RuleId::new("E026")
    }
    fn name(&self) -> &'static str {
        "sar-portion-form"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::{MarkingType, SarIndicator};
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }
        let Some(sar) = attrs.sar_markings.as_ref() else {
            return vec![];
        };
        if !matches!(sar.indicator, SarIndicator::Full) {
            return vec![];
        }
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::SarIndicator)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        // When all program identifiers are already abbrev-shaped (2–3
        // alphanumeric chars), propose a low-confidence suggestion to replace
        // the full indicator with `SAR-`. Otherwise the fix requires human
        // judgment and no proposal is emitted.
        let all_programs_abbreviated = sar.programs.iter().all(|p| {
            let id = p.identifier.as_ref();
            (2..=3).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric())
        });

        let fix = if all_programs_abbreviated {
            let block_span = sar_block_span(attrs).unwrap_or(span);
            let original = sar_block_source(attrs, block_span).unwrap_or_default();
            let replacement = render_sar_block(SarIndicator::Abbrev, &sar.programs);
            Some(FixProposal::new(
                self.id(),
                FixSource::BuiltinRule,
                block_span,
                original,
                replacement,
                marque_rules::Confidence::strict(0.35),
                None,
            ))
        } else {
            None
        };

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "portion marks must use the SAR- abbreviation, not the \
             SPECIAL ACCESS REQUIRED- full form",
            "CAPCO-2016 §H.5 p101 line 2432 (Authorized Portion Mark)",
            fix,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E027 — SAR requires TS, S, or C classification
// ---------------------------------------------------------------------------

/// SAR markings may only be used with TOP SECRET, SECRET, or CONFIDENTIAL
/// classifications.
///
/// Authority: CAPCO-2016 §H.5 p101 line 2456 — "Relationship(s) to Other
/// Markings: May only be used with TOP SECRET, SECRET, or CONFIDENTIAL."
/// All three classification levels are explicitly permitted; no
/// TS-only or C-excluded carve-out exists in §H.5.
///
/// The rule also fires when `attrs.classification` is `None` — §H.5
/// p101 line 2452 ("Applicable only to classified information") makes
/// this position derivative: a SAR block without any classification
/// token is malformed, not merely Unclassified. Treating the two
/// invalid states together (no classification vs Unclassified) is
/// defensible because both fail the §H.5 "classified information"
/// gate; the diagnostic message names the three valid classifications
/// so the user sees the remedy either way.
///
/// `UNCLASSIFIED//SAR-*` requires human review — no automated fix is
/// offered because the correct classification is outside the marking.
struct SarClassificationRule;

impl Rule for SarClassificationRule {
    fn id(&self) -> RuleId {
        RuleId::new("E027")
    }
    fn name(&self) -> &'static str {
        "sar-classification"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::{Classification, MarkingClassification};
        if attrs.sar_markings.is_none() {
            return vec![];
        }
        let invalid = matches!(
            &attrs.classification,
            None | Some(MarkingClassification::Us(Classification::Unclassified))
        );
        if !invalid {
            return vec![];
        }
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::SarIndicator)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "SAR markings may only be used with TOP SECRET, SECRET, or \
             CONFIDENTIAL classifications",
            "CAPCO-2016 §H.5 p101 line 2456 (Relationship(s) to Other Markings)",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E028 — SAR programs must be in ascending order
// ---------------------------------------------------------------------------

/// Programs within a SAR block must be listed in ascending sort order
/// with numbered values first, followed by alphabetic values.
///
/// Authority: CAPCO-2016 §H.5 p99 line 2391 — "Multiple program
/// identifiers are listed in ascending sort order with numbered values
/// first, followed by alphabetic values." Reinforced by §H.5 p100 line
/// 2402 Syntax Rules bullet 4 (same sort rule, `/` separator without
/// interjected spaces).
///
/// Note: SAR's ordering authority is solely §H.5. §A.6 covers SCI
/// ordering and is NOT a valid citation target for SAR rules.
///
/// When programs are out of order, the fix also sorts compartments and
/// sub-compartments within each program in a single whole-block rewrite
/// — so when E028 and E029 both detect violations on the same marking,
/// applying E028's fix fully normalizes the block and the E029 fixes
/// (which cover per-program sub-spans) become redundant. The engine's
/// overlap guard will retain E028 and drop E029 for that run; a
/// subsequent lint will confirm zero residual violations.
struct SarProgramOrderRule;

impl Rule for SarProgramOrderRule {
    fn id(&self) -> RuleId {
        RuleId::new("E028")
    }
    fn name(&self) -> &'static str {
        "sar-program-order"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let Some(sar) = attrs.sar_markings.as_ref() else {
            return vec![];
        };
        if sar.programs.len() < 2 {
            return vec![];
        }
        let in_order = sar
            .programs
            .windows(2)
            .all(|w| sar_sort_key(&w[0].identifier) <= sar_sort_key(&w[1].identifier));
        if in_order {
            return vec![];
        }
        let Some(span) = sar_block_span(attrs) else {
            return vec![];
        };
        let original = render_sar_block(sar.indicator, &sar.programs);

        // Sort programs and also normalize compartments/subs within each program
        // in the same pass. This ensures applying the E028 fix alone fully
        // normalizes the block even when E029 violations are present.
        let mut sorted = sar.programs.to_vec();
        for prog in sorted.iter_mut() {
            let mut comps = prog.compartments.to_vec();
            for comp in comps.iter_mut() {
                let mut subs = comp.sub_compartments.to_vec();
                subs.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));
                *comp = marque_ism::SarCompartment::new(
                    comp.identifier.clone(),
                    subs.into_boxed_slice(),
                );
            }
            comps.sort_by(|a, b| sar_sort_key(&a.identifier).cmp(&sar_sort_key(&b.identifier)));
            *prog = marque_ism::SarProgram::new(prog.identifier.clone(), comps.into_boxed_slice());
        }
        sorted.sort_by(|a, b| sar_sort_key(&a.identifier).cmp(&sar_sort_key(&b.identifier)));
        let replacement = render_sar_block(sar.indicator, &sorted);

        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: self.id(),
            severity: self.default_severity(),
            source: FixSource::BuiltinRule,
            span,
            message: "SAR programs must be in ascending order (numeric first, \
                 then alphabetic)"
                .to_owned(),
            citation: "CAPCO-2016 §H.5 p99 line 2391 \
                       (programs: ascending, numeric first, then alpha)",
            original,
            replacement,
            confidence: 0.85,
            migration_ref: None,
        })]
    }
}

// ---------------------------------------------------------------------------
// Rule: E029 — SAR compartments and sub-compartments must be in order
// ---------------------------------------------------------------------------

/// Compartments within a program — and sub-compartments within a
/// compartment — must be in ascending sort order.
///
/// Authority (per level):
/// - **Compartments**: CAPCO-2016 §H.5 p100 line 2404 — "Compartment(s)
///   (if any), must be kept with the SAP program identifier, listed
///   in ascending sort order with numbered values first, followed by
///   alphabetic values, and separated by a hyphen".
/// - **Sub-compartments**: CAPCO-2016 §H.5 p100 line 2405 — "Sub-
///   compartment(s) (if any), must be kept with the compartment,
///   listed alphanumerically, and separated by a single space."
///
/// The line-2405 phrasing ("alphanumerically") is terser than the
/// compartment/program phrasing, but the Table 7 example on line 2411
/// (`BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB`) shows sub-compartments
/// like `YYY 456 689` following the same numeric-first-then-alpha
/// convention. The rule applies the uniform `sar_sort_key` across
/// both levels, and the diagnostic's citation is chosen by level
/// below.
///
/// One diagnostic is emitted **per out-of-order program** (not one for the
/// whole SAR block). This gives each program a non-overlapping fix span so
/// all compartment-ordering fixes can be applied in a single pass, and so
/// the fix spans don't overlap with E028's whole-block span when both rules
/// fire on the same marking.
struct SarCompartmentOrderRule;

impl Rule for SarCompartmentOrderRule {
    fn id(&self) -> RuleId {
        RuleId::new("E029")
    }
    fn name(&self) -> &'static str {
        "sar-compartment-order"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let Some(sar) = attrs.sar_markings.as_ref() else {
            return vec![];
        };

        // Pre-compute SarProgram token positions once for per-program span
        // lookups via `sar_program_span`.
        let prog_positions: Vec<usize> = attrs
            .token_spans
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                if t.kind == TokenKind::SarProgram {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        let mut diagnostics = Vec::new();

        for (prog_idx, prog) in sar.programs.iter().enumerate() {
            let comps_ok = prog.compartments.len() < 2
                || prog
                    .compartments
                    .windows(2)
                    .all(|w| sar_sort_key(&w[0].identifier) <= sar_sort_key(&w[1].identifier));
            let subs_ok = prog.compartments.iter().all(|comp| {
                comp.sub_compartments.len() < 2
                    || comp
                        .sub_compartments
                        .windows(2)
                        .all(|w| sar_sort_key(&w[0]) <= sar_sort_key(&w[1]))
            });
            if comps_ok && subs_ok {
                continue;
            }

            let Some(span) = sar_program_span(&attrs.token_spans, &prog_positions, prog_idx) else {
                continue;
            };

            let original = render_single_program(prog);

            // Sort compartments and sub-compartments within this program.
            let mut sorted_comps = prog.compartments.to_vec();
            for comp in sorted_comps.iter_mut() {
                let mut subs = comp.sub_compartments.to_vec();
                subs.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));
                *comp = marque_ism::SarCompartment::new(
                    comp.identifier.clone(),
                    subs.into_boxed_slice(),
                );
            }
            sorted_comps
                .sort_by(|a, b| sar_sort_key(&a.identifier).cmp(&sar_sort_key(&b.identifier)));
            let sorted_prog = marque_ism::SarProgram::new(
                prog.identifier.clone(),
                sorted_comps.into_boxed_slice(),
            );
            let replacement = render_single_program(&sorted_prog);

            let (level, citation) = if !comps_ok {
                (
                    "compartments",
                    "CAPCO-2016 §H.5 p100 line 2404 \
                     (compartments: ascending, numeric first, then alpha)",
                )
            } else {
                (
                    "sub-compartments",
                    "CAPCO-2016 §H.5 p100 line 2405 \
                     (sub-compartments: alphanumerically, single space)",
                )
            };

            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span,
                message: format!(
                    "SAR {level} must be in ascending order (numeric first, \
                     then alphabetic)"
                ),
                citation,
                original,
                replacement,
                confidence: 0.85,
                migration_ref: None,
            }));
        }

        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Rule: E030 — SAR category indicator must not be repeated
// ---------------------------------------------------------------------------

/// The SAR category indicator must not be repeated when multiple
/// programs apply; multiple programs use a single indicator with `/`
/// separator.
///
/// Authority: CAPCO-2016 §H.5 p100 line 2403 (Syntax Rules bullet 5)
/// — "The SAP category indicator must not be repeated if multiple
/// SAP programs are applicable." Program separator is prescribed in
/// adjacent bullet 4 at line 2402: "separated by a single forward
/// slash (`/`) without interjected spaces."
///
/// §A.6 governs SCI ordering and is NOT a valid citation target for
/// SAR rules — this was incorrectly referenced in an earlier
/// revision of this doc comment.
///
/// The parser captures the first SAR block into `attrs.sar_markings`
/// and emits every subsequent same-marking SAR block as an `Unknown`
/// token whose text still starts with `SAR-` or `SPECIAL ACCESS
/// REQUIRED-`. This rule finds those Unknown tokens, extends the fix
/// span backward over the preceding `//` category separator, and
/// coalesces the repeated block into the preceding block.
///
/// # Historical note
///
/// Repeated `SAR-` indicators were permitted until **December 2011**,
/// when CAPCO removed the requirement (CAPCO-2016 §I Banner Line
/// Syntax History, p192 line 4700: "Removed repeating `SAR-` for
/// multiple SAR markings in the SAP category"). An earlier revision
/// of this doc said "prior to roughly 2003" — that conflated the
/// December-2011 repeat-rule change with the October 2003 category
/// move (§I line 4713: "Moved Special Access Required (SAR) from
/// Non-Intelligence Community Dissemination Control Markings to a
/// new category"). The two changes are unrelated. §I is historical
/// background rather than a valid predicate-citation target, but is
/// cited here for documentation context only.
///
/// Repeated indicators in modern documents are therefore an error
/// this rule must surface, even though older corpus material
/// (pre-2011) may legitimately contain them.
///
/// # No-fix diagnostic paths
///
/// When the repeated-SAR token shape is detected but the rule cannot
/// produce a clean fix — e.g., the parser trimmed whitespace between
/// the `//` separator and the Unknown token, so the fix span cannot
/// honestly reconstruct `FixProposal.original` — this rule still
/// emits a no-fix diagnostic. Suppressing entirely would silently
/// drop the shape (E008 also steps aside for repeated-SAR prefixes;
/// see `UnknownTokenRule`), so the user would see nothing at all.
struct SarIndicatorRepeatRule;

impl Rule for SarIndicatorRepeatRule {
    fn id(&self) -> RuleId {
        RuleId::new("E030")
    }
    fn name(&self) -> &'static str {
        "sar-indicator-repeat"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        // Fast exit: no SAR block at all.
        if attrs.sar_markings.is_none() {
            return vec![];
        }
        let mut diagnostics = Vec::new();
        // Walk token_spans by index so we can look back for the
        // Separator token that introduced the repeated SAR block.
        for (idx, tok) in attrs.token_spans.iter().enumerate() {
            if tok.kind != TokenKind::Unknown {
                continue;
            }
            let text = tok.text.as_ref();
            let stripped = if let Some(rest) = text.strip_prefix("SAR-") {
                rest
            } else if let Some(rest) = text.strip_prefix("SPECIAL ACCESS REQUIRED-") {
                rest
            } else {
                continue;
            };
            if stripped.is_empty() {
                continue;
            }
            let message = "SAR category indicator must not be repeated; \
                 multiple programs use a single indicator with '/' separator";
            let citation = concat!(
                "CAPCO-2016 §H.5 p100 line 2403 ",
                "(SAP category indicator must not be repeated if \
                 multiple SAP programs are applicable)",
            );
            // Find the closest preceding Separator token. The parser
            // trims leading whitespace per block, so the token's own
            // span does not necessarily sit flush against the `//`.
            let Some(sep_tok) = attrs.token_spans[..idx]
                .iter()
                .rev()
                .find(|t| t.kind == TokenKind::Separator)
            else {
                // No preceding separator — shouldn't happen for a valid
                // SAR-prefixed Unknown token, but emit a no-fix
                // diagnostic rather than drop it silently. E008
                // suppresses this token in favor of E030, so skipping
                // here would leave the user with no diagnostic at all.
                diagnostics.push(Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    tok.span,
                    message.to_owned(),
                    citation,
                    None,
                ));
                continue;
            };
            // A fix requires the separator and the Unknown token to be
            // byte-contiguous: splicing is by span, and fabricating
            // `FixProposal.original` bytes for a gap we don't have a
            // copy of would corrupt the audit record. When a gap is
            // present (e.g., `//  SAR-FOO` with leading whitespace the
            // parser trimmed), emit a no-fix diagnostic instead of
            // skipping — skipping combined with E008's suppression
            // would silently drop the repeated-SAR shape entirely.
            if sep_tok.span.end != tok.span.start {
                diagnostics.push(Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    tok.span,
                    message.to_owned(),
                    citation,
                    None,
                ));
                continue;
            }
            let fix_span = Span::new(sep_tok.span.start, tok.span.end);
            let replacement = format!("/{stripped}");
            let sep_text = sep_tok.text.as_ref();
            let original = format!("{sep_text}{text}");

            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span: fix_span,
                message: message.to_owned(),
                citation,
                original,
                replacement,
                confidence: 0.9,
                migration_ref: None,
            }));
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Rule: E031 — SAR banner roll-up (programs-only)
// ---------------------------------------------------------------------------

/// Every SAR **program** present in a portion mark must also appear in the
/// banner's SAR block. Banner hierarchy depth (compartments and
/// sub-compartments) is **optional** and NOT checked by this rule.
///
/// # Authority
///
/// - **§H.5 p101 line 2458** (Precedence Rules for Banner Line Guidance):
///   *"Unique SAPs contained in portion marks must always appear in the
///   banner line."* The "Unique SAPs" language refers to unique program
///   identifiers — the rule is a program-rollup rule.
/// - **§H.5 p101 line 2460** (Notes): *"Depicting the hierarchical
///   structure of a SAP program below the program identifier is optional
///   and dependent upon operational requirements. It is not mandatory to
///   reflect a SAP program's hierarchy in either the portion marks or
///   banner line."*
/// - **§H.5 p99 line 2393** (general): *"Depiction of the hierarchical
///   structure of a SAP below the program identifier in the banner line
///   or portion mark is optional."*
///
/// These three passages together establish that programs MUST roll up
/// to the banner, but compartments and sub-compartments MAY be omitted
/// from the banner even when present in portions. A banner showing
/// `SAR-BP` when a portion shows `SAR-BP-J12` is therefore valid.
///
/// # Predicate history
///
/// An earlier revision of this rule flagged missing compartments and
/// sub-compartments as violations, producing false positives on
/// hierarchy-optional banners. T035c-19 PR-C (this change) narrowed
/// the predicate to programs-only per the §H.5 p101 line 2460
/// provision. The prior behavior over-restricted relative to source.
///
/// # Fix semantics
///
/// - If the banner has a SAR block, replace it in-place at confidence
///   0.9 (severity `Fix`). The replacement **preserves the observed
///   banner's existing programs with whatever hierarchy they already
///   show**, and appends each missing program as a bare program
///   identifier (no compartments). This minimum-change fix honors the
///   "hierarchy is optional" rule — the user chose how much hierarchy
///   to show for the programs that were there, and we do not override
///   that choice for the programs that were missing.
/// - If the banner has no SAR block at all, emit at severity `Error`
///   with no fix — inserting a new block requires byte-positioning
///   between the SCI and AEA blocks, which the engine's single-pass
///   architecture does not reliably support from rule-level
///   information alone.
struct SarBannerRollupRule;

impl Rule for SarBannerRollupRule {
    fn id(&self) -> RuleId {
        RuleId::new("E031")
    }
    fn name(&self) -> &'static str {
        "sar-banner-rollup"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;

        // Banner / CAB markings only; portions are the input to the rollup,
        // not the subject of it.
        if !matches!(ctx.marking_type, MarkingType::Banner | MarkingType::Cab) {
            return vec![];
        }

        let Some(page_context) = ctx.page_context.as_ref() else {
            return vec![];
        };
        let Some(expected) = page_context.expected_sar_marking() else {
            return vec![];
        };
        if expected.programs.is_empty() {
            return vec![];
        }

        // Compute the set of program identifiers missing from the
        // observed banner. Hierarchy (compartments / sub-compartments)
        // is deliberately NOT compared — §H.5 p101 line 2460 makes
        // banner hierarchy depth optional even when portions carry
        // hierarchy. See the `sar_missing_programs` helper doc for
        // the authority trail.
        let missing_programs = sar_missing_programs(attrs.sar_markings.as_ref(), &expected);
        if missing_programs.is_empty() {
            return vec![];
        }

        const CITATION: &str = concat!(
            "CAPCO-2016 §H.5 p101 line 2458 ",
            "(Unique SAPs contained in portion marks must always appear ",
            "in the banner line; hierarchy depiction optional per §H.5 ",
            "p101 line 2460 + p99 line 2393)",
        );

        let missing_list: Vec<&str> = missing_programs
            .iter()
            .map(|p| p.identifier.as_ref())
            .collect();
        let message = format!(
            "banner SAR block is missing programs present in portions: {}",
            missing_list.join(", "),
        );

        match attrs.sar_markings.as_ref() {
            Some(observed) => {
                // Banner has a SAR block — replace it in place with a
                // MINIMUM-CHANGE fix: keep the observed banner's programs
                // (including whatever hierarchy the author already chose
                // to show) and append the missing programs as bare
                // identifiers. §H.5 p101 line 2460 says the author may
                // depict hierarchy or not — overwriting the observed
                // banner with `expected.programs` (the full portion
                // rollup) would force hierarchy onto a banner the
                // author deliberately kept flat.
                let Some(span) = sar_block_span(attrs) else {
                    return vec![];
                };
                let original_bytes = attrs
                    .token_spans
                    .iter()
                    .find(|t| t.kind == TokenKind::SarIndicator)
                    .map(|_| ())
                    .and_then(|_| sar_block_source(attrs, span))
                    .unwrap_or_else(|| render_sar_block(observed.indicator, &observed.programs));

                let mut merged_programs: Vec<marque_ism::SarProgram> = observed.programs.to_vec();
                for missing in &missing_programs {
                    // Append bare program (no compartments). The fix is
                    // correctness-minimum; hierarchy is the author's
                    // choice.
                    merged_programs.push(marque_ism::SarProgram::new(
                        missing.identifier.clone(),
                        Box::new([]),
                    ));
                }
                // Sort merged programs per §H.5 p99 line 2391
                // (ascending, numeric first, then alpha) so the fix
                // output is always canonical regardless of the original
                // banner's order.
                merged_programs
                    .sort_by(|a, b| sar_sort_key(&a.identifier).cmp(&sar_sort_key(&b.identifier)));
                let replacement = render_sar_block(observed.indicator, &merged_programs);
                if replacement == original_bytes {
                    // Defensive: if the observed already matched the
                    // merged form by text, `missing_programs` should
                    // have been empty — but skip rather than emit a
                    // no-op fix.
                    return vec![];
                }
                vec![make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::BuiltinRule,
                    span,
                    message,
                    citation: CITATION,
                    original: original_bytes,
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
                let span = attrs
                    .token_spans
                    .first()
                    .map(|t| t.span)
                    .unwrap_or(Span::new(0, 0));
                vec![Diagnostic::new(
                    self.id(),
                    Severity::Error,
                    span,
                    message,
                    CITATION,
                    None,
                )]
            }
        }
    }
}

/// Collect SAR programs that appear in `expected` but not in `observed`.
///
/// Compares by program identifier only. Compartments and sub-compartments
/// are deliberately NOT compared — per CAPCO-2016 §H.5 p101 line 2460
/// and §H.5 p99 line 2393, banner hierarchy depiction below the program
/// level is optional even when portions carry hierarchy. A banner showing
/// `SAR-BP` when a portion shows `SAR-BP-J12` is therefore compliant and
/// must not be flagged.
///
/// Returned programs carry their original expected-side compartments /
/// sub-compartments so callers can render informative diagnostic messages
/// if desired, but the membership test itself is program-id-only.
fn sar_missing_programs(
    observed: Option<&marque_ism::SarMarking>,
    expected: &marque_ism::SarMarking,
) -> Vec<marque_ism::SarProgram> {
    use std::collections::HashSet;

    let observed_ids: HashSet<&str> = match observed {
        Some(obs) => obs
            .programs
            .iter()
            .map(|p| p.identifier.as_ref())
            .collect(),
        None => HashSet::new(),
    };

    expected
        .programs
        .iter()
        .filter(|p| !observed_ids.contains(p.identifier.as_ref()))
        .cloned()
        .collect()
}

/// Render a single SAR program to its banner-block fragment form (without
/// the leading indicator prefix). Used to describe missing programs in
/// diagnostic messages.
fn render_single_program(prog: &marque_ism::SarProgram) -> String {
    let mut s = String::from(prog.identifier.as_ref());
    for comp in prog.compartments.iter() {
        s.push('-');
        s.push_str(&comp.identifier);
        for sub in comp.sub_compartments.iter() {
            s.push(' ');
            s.push_str(sub);
        }
    }
    s
}

/// Return a normalized SAR block string for use as the
/// `FixProposal::original` field when `span` covers SAR tokens.
///
/// This helper does not reconstruct the exact original source bytes or
/// preserve original formatting; it renders the parsed SAR structure via
/// `render_sar_block(...)`. Returns `None` when the attributes have no SAR
/// markings or when the provided span does not contain SAR tokens.
fn sar_block_source(attrs: &IsmAttributes, span: Span) -> Option<String> {
    // We do not have enough information here to recover exact original source
    // bytes. Instead, gate on whether the requested span contains SAR tokens
    // and then return the canonical rendering of the parsed SAR block.
    let sar = attrs.sar_markings.as_ref()?;
    // Sanity: ensure there is at least one SAR token within span.
    let has_in_span = attrs.token_spans.iter().any(|t| {
        matches!(
            t.kind,
            TokenKind::SarIndicator
                | TokenKind::SarProgram
                | TokenKind::SarCompartment
                | TokenKind::SarSubCompartment
        ) && t.span.start >= span.start
            && t.span.end <= span.end
    });
    if !has_in_span {
        return None;
    }
    Some(render_sar_block(sar.indicator, &sar.programs))
}

// Rule: E032 — SCI control-system sort order
// ---------------------------------------------------------------------------

/// Per CAPCO-2016 §A.6 p15: control systems within a single SCI category
/// block must be listed in ascending sort order (numeric first, then
/// alphabetic). Walks adjacent pairs in source order; on any out-of-order
/// pair, emits a single diagnostic with a reordered fix covering the full
/// block. Confidence 0.85.
struct SciSystemOrderRule;

impl Rule for SciSystemOrderRule {
    fn id(&self) -> RuleId {
        RuleId::new("E032")
    }
    fn name(&self) -> &'static str {
        "sci-system-order"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if attrs.sci_markings.len() < 2 {
            return vec![];
        }

        let keys: Vec<(bool, u64, &str)> = attrs
            .sci_markings
            .iter()
            .map(|m| sar_sort_key(sci_system_text(&m.system)))
            .collect();

        let out_of_order = keys.windows(2).any(|w| w[0] > w[1]);
        if !out_of_order {
            return vec![];
        }

        // Build the reordered replacement by sorting the source SciControl
        // chunk spans. The block-level SciControl TokenSpan covers the full
        // system chunk (per marque-core parser). One span per SciMarking,
        // in source order, zips to sci_markings.
        let chunk_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciControl)
            .collect();
        if chunk_spans.len() != attrs.sci_markings.len() {
            // Inconsistent — don't emit an unsafe fix.
            return vec![];
        }

        // Fix span covers the first through last chunk (same SCI block).
        let fix_start = chunk_spans.first().map(|t| t.span.start).unwrap_or(0);
        let fix_end = chunk_spans.last().map(|t| t.span.end).unwrap_or(fix_start);
        let fix_span = Span::new(fix_start, fix_end);

        // Build the replacement by also sorting compartments and
        // sub-compartments within each marking (mirrors SAR E028's
        // all-levels fix). This way, when E032 and E033 both fire on the
        // same block, the engine's overlap guard can drop E033 and this
        // single E032 fix fully normalizes the block — no residual
        // ordering violations after one apply pass.
        let mut sorted = attrs.sci_markings.to_vec();
        for m in sorted.iter_mut() {
            let mut comps = m.compartments.to_vec();
            for c in comps.iter_mut() {
                let mut subs = c.sub_compartments.to_vec();
                subs.sort_by(|a, b| sar_sort_key(a.as_ref()).cmp(&sar_sort_key(b.as_ref())));
                *c = marque_ism::SciCompartment::new(c.identifier.clone(), subs.into_boxed_slice());
            }
            comps.sort_by(|a, b| {
                sar_sort_key(a.identifier.as_ref()).cmp(&sar_sort_key(b.identifier.as_ref()))
            });
            *m = marque_ism::SciMarking::new(
                m.system.clone(),
                comps.into_boxed_slice(),
                m.canonical_enum,
            );
        }
        sorted.sort_by(|a, b| {
            sar_sort_key(sci_system_text(&a.system)).cmp(&sar_sort_key(sci_system_text(&b.system)))
        });

        let original: String = chunk_spans
            .iter()
            .map(|t| t.text.as_ref())
            .collect::<Vec<_>>()
            .join("/");
        let replacement = render_sci_block(&sorted);

        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: self.id(),
            severity: self.default_severity(),
            source: FixSource::BuiltinRule,
            span: fix_span,
            message: "SCI control systems within a block must be listed in ascending \
                      order (numeric first, then alphabetic)"
                .to_owned(),
            citation: "CAPCO-2016 §A.6 p15",
            original,
            replacement,
            confidence: 0.85,
            migration_ref: None,
        })]
    }
}

// ---------------------------------------------------------------------------
// Rule: E033 — SCI compartment / sub-compartment sort order
// ---------------------------------------------------------------------------

/// Per CAPCO-2016 §A.6 p15 + §H.4 p61: within each SCI control system,
/// compartments must be listed in ascending sort order (numeric first, then
/// alphabetic); within each compartment, sub-compartments must also be
/// ascending.
///
/// Emits **one diagnostic per out-of-order marking** (not one per level).
/// The fix sorts compartments AND sub-compartments together in a single
/// rewrite, matching SAR E029's shape. This guarantees:
///
///   * Comp-order and sub-order violations on the same marking don't
///     produce overlapping fix spans that the engine's C-1 guard would
///     have to drop (one would apply, the other would not, and the next
///     lint would re-fire the dropped one).
///   * When E032 (system-order) also fires on the same block, its
///     whole-block span supersedes every per-marking E033 span under
///     FR-016 ordering, and E032's all-levels fix fully normalizes —
///     so dropping E033 is safe.
///
/// Confidence 0.85.
struct SciCompartmentOrderRule;

impl Rule for SciCompartmentOrderRule {
    fn id(&self) -> RuleId {
        RuleId::new("E033")
    }
    fn name(&self) -> &'static str {
        "sci-compartment-order"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut out = Vec::new();

        let comp_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciCompartment)
            .collect();
        let sub_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciSubCompartment)
            .collect();

        let mut comp_cursor = 0usize;
        let mut sub_cursor = 0usize;

        for marking in attrs.sci_markings.iter() {
            let n_comps = marking.compartments.len();
            let this_sub_count: usize = marking
                .compartments
                .iter()
                .map(|c| c.sub_compartments.len())
                .sum();

            let comps_ok = n_comps < 2
                || marking.compartments.windows(2).all(|w| {
                    sar_sort_key(w[0].identifier.as_ref()) <= sar_sort_key(w[1].identifier.as_ref())
                });
            let subs_ok = marking.compartments.iter().all(|c| {
                c.sub_compartments.len() < 2
                    || c.sub_compartments
                        .windows(2)
                        .all(|w| sar_sort_key(w[0].as_ref()) <= sar_sort_key(w[1].as_ref()))
            });

            if comps_ok && subs_ok {
                comp_cursor += n_comps;
                sub_cursor += this_sub_count;
                continue;
            }

            // Span covers the whole compartment+sub-compartment region
            // for this marking: from the first compartment token through
            // the last sub-compartment token (or the last compartment
            // token when the marking has no sub-compartments).
            //
            // Use `.get()` defensively: if the token stream doesn't carry
            // the expected number of SciCompartment / SciSubCompartment
            // tokens (attrs built outside the parser, or future parser
            // changes), skip the fix instead of panicking.
            let this_comp_spans = if n_comps == 0 {
                &[][..]
            } else {
                match comp_spans.get(comp_cursor..comp_cursor + n_comps) {
                    Some(s) => s,
                    None => {
                        comp_cursor += n_comps;
                        sub_cursor += this_sub_count;
                        continue;
                    }
                }
            };
            let fix_start = this_comp_spans.first().map(|t| t.span.start).unwrap_or(0);
            let fix_end = if this_sub_count > 0 {
                sub_spans
                    .get(sub_cursor + this_sub_count - 1)
                    .map(|t| t.span.end)
                    .unwrap_or_else(|| {
                        this_comp_spans
                            .last()
                            .map(|t| t.span.end)
                            .unwrap_or(fix_start)
                    })
            } else {
                this_comp_spans
                    .last()
                    .map(|t| t.span.end)
                    .unwrap_or(fix_start)
            };
            let fix_span = Span::new(fix_start, fix_end);

            // Build the sorted marking: sort sub-compartments within
            // each compartment, then sort compartments by identifier.
            // Sub-comps ride along with their parent compartment.
            let mut sorted_comps = marking.compartments.to_vec();
            for c in sorted_comps.iter_mut() {
                let mut subs = c.sub_compartments.to_vec();
                subs.sort_by(|a, b| sar_sort_key(a.as_ref()).cmp(&sar_sort_key(b.as_ref())));
                *c = marque_ism::SciCompartment::new(c.identifier.clone(), subs.into_boxed_slice());
            }
            sorted_comps.sort_by(|a, b| {
                sar_sort_key(a.identifier.as_ref()).cmp(&sar_sort_key(b.identifier.as_ref()))
            });

            // Render this marking's compartment region (no system prefix —
            // the span only covers compartments+subs, not the system head).
            let render_comps = |comps: &[marque_ism::SciCompartment]| -> String {
                let parts: Vec<String> = comps
                    .iter()
                    .map(|c| {
                        if c.sub_compartments.is_empty() {
                            c.identifier.as_ref().to_owned()
                        } else {
                            let mut s = c.identifier.as_ref().to_owned();
                            for sub in c.sub_compartments.iter() {
                                s.push(' ');
                                s.push_str(sub.as_ref());
                            }
                            s
                        }
                    })
                    .collect();
                parts.join("-")
            };
            let original = render_comps(&marking.compartments);
            let replacement = render_comps(&sorted_comps);

            let level = if !comps_ok {
                "compartments"
            } else {
                "sub-compartments"
            };

            out.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span: fix_span,
                message: format!(
                    "SCI {level} must be listed in ascending order (numeric first, \
                     then alphabetic)"
                ),
                citation: "CAPCO-2016 §A.6 p15; §H.4 p61",
                original,
                replacement,
                confidence: 0.85,
                migration_ref: None,
            }));

            comp_cursor += n_comps;
            sub_cursor += this_sub_count;
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Rule: E034 — SCI custom-control audit visibility
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
/// CI-blocking should configure `E034 = "info"` if they want
/// audit-visibility only.)
///
/// T035c-2 landed the `Severity::Info` variant and dropped the earlier
/// `Severity::Off` workaround. Previously, the rule emitted `Diagnostic`
/// values at `Severity::Off` — a state `Principle IV` declares
/// unrepresentable — and relied on the test harness bypassing
/// engine-level severity filtering to observe the diagnostics. That was
/// a constitutional-invariant violation. Users who want informational
/// (non-warn) treatment can configure `E034 = "info"` in `.marque.toml`;
/// users who want it silent can configure `E034 = "off"`.
struct SciCustomControlInfoRule;

impl Rule for SciCustomControlInfoRule {
    fn id(&self) -> RuleId {
        RuleId::new("E034")
    }
    fn name(&self) -> &'static str {
        "sci-custom-control-info"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
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

// ---------------------------------------------------------------------------
// Rule: E035 — SCI banner rollup
// ---------------------------------------------------------------------------

/// Per CAPCO-2016 §H.4 per-system "Precedence Rules for Banner Line
/// Guidance" (HCS p62 and friends) + §D.2 p28: the banner's SCI block must
/// contain every compartment and sub-compartment that appears in any
/// portion marking on the same page. Compares the observed banner's
/// `sci_markings` against `page_context.expected_sci_markings()`; fires on
/// any missing compartment or sub-compartment.
///
/// **Defensive P4 coupling**: the rollup method on `PageContext` is
/// delivered by P4 (parallel branch). Until that lands this rule no-ops
/// gracefully — `expected_sci_markings` is called through a helper that
/// returns an empty slice when the method is unavailable. Once P4 lands,
/// the rule activates automatically with no rule-side change.
struct SciBannerRollupRule;

impl Rule for SciBannerRollupRule {
    fn id(&self) -> RuleId {
        RuleId::new("E035")
    }
    fn name(&self) -> &'static str {
        "sci-banner-rollup"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;

        // Portion candidates carry only their own SCI, not the page
        // rollup — this rule applies only to banner / CAB candidates.
        if ctx.marking_type == MarkingType::Portion {
            return vec![];
        }
        let Some(page) = ctx.page_context.as_ref() else {
            return vec![];
        };

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
                self.id(),
                Severity::Error,
                Span::new(0, 0),
                format!(
                    "banner is missing an SCI block that portions require: {}",
                    missing.join("; ")
                ),
                "CAPCO-2016 §H.4 p62 (HCS precedence); §D.2 p28",
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
            rule: self.id(),
            severity: self.default_severity(),
            source: FixSource::BuiltinRule,
            span: fix_span,
            message: format!(
                "banner SCI block is missing compartments present in the page's \
                 portions: {}",
                missing.join("; ")
            ),
            citation: "CAPCO-2016 §H.4 p62 (HCS precedence); §D.2 p28",
            original,
            replacement,
            confidence: 0.9,
            migration_ref: None,
        })]
    }
}

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
fn sar_block_span(attrs: &IsmAttributes) -> Option<Span> {
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

/// Compute the span of a single program (at index `prog_idx` in
/// `sar.programs`) within the SAR block's token spans.
///
/// Returns the span from the `SarProgram` token's start to the end of the
/// last compartment/sub-compartment token belonging to that program (or just
/// the `SarProgram` token's end when the program has no compartments).
/// `prog_positions` must be a pre-computed, index-ordered list of positions
/// of `SarProgram` tokens within `token_spans`.
///
/// This is used by E029 (`sar-compartment-order`) to emit per-program fix
/// spans rather than whole-block spans, ensuring E028 and E029 fixes are
/// non-overlapping when a SAR block has both out-of-order programs and
/// out-of-order compartments.
fn sar_program_span(
    token_spans: &[marque_ism::TokenSpan],
    prog_positions: &[usize],
    prog_idx: usize,
) -> Option<Span> {
    let tok_idx = *prog_positions.get(prog_idx)?;
    let start = token_spans[tok_idx].span.start;

    // Slice from this program's SarProgram token up to (but not including)
    // the next program's SarProgram token (or to the end of all tokens if
    // this is the last program).
    let end_range = match prog_positions.get(prog_idx + 1).copied() {
        Some(next_idx) => &token_spans[tok_idx..next_idx],
        None => &token_spans[tok_idx..],
    };

    // The program span ends at the last SAR sub-token in this program's range.
    let end = end_range
        .iter()
        .rev()
        .find(|t| {
            matches!(
                t.kind,
                TokenKind::SarProgram | TokenKind::SarCompartment | TokenKind::SarSubCompartment
            )
        })
        .map(|t| t.span.end)
        .unwrap_or(token_spans[tok_idx].span.end);

    Some(Span::new(start, end))
}

/// Render a SAR block back to source form for fix replacements.
///
/// Abbreviated form: `SAR-<PROG>[-<COMP>[ <SUB>...]]{/<PROG>...}`.
/// Full form: `SPECIAL ACCESS REQUIRED-<PROG>[-<COMP>[ <SUB>...]]{/<PROG>...}`.
/// The renderer preserves any compartments and sub-compartments
/// attached to each program for either indicator form.
fn render_sar_block(
    indicator: marque_ism::SarIndicator,
    programs: &[marque_ism::SarProgram],
) -> String {
    use marque_ism::SarIndicator;
    let prefix = match indicator {
        SarIndicator::Abbrev => "SAR-",
        SarIndicator::Full => "SPECIAL ACCESS REQUIRED-",
    };
    let mut out = String::with_capacity(prefix.len() + programs.len() * 8);
    out.push_str(prefix);
    for (i, prog) in programs.iter().enumerate() {
        if i > 0 {
            out.push('/');
        }
        out.push_str(&prog.identifier);
        for comp in prog.compartments.iter() {
            out.push('-');
            out.push_str(&comp.identifier);
            for sub in comp.sub_compartments.iter() {
                out.push(' ');
                out.push_str(sub);
            }
        }
    }
    out
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

pub(crate) fn make_fix_diagnostic(p: FixDiagnosticParams) -> Diagnostic {
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_capco_test_support::{lint_banner, lint_portion};

    #[test]
    fn capco_rule_set_registers_all_rules() {
        let set = CapcoRuleSet::new();
        let ids: Vec<&str> = set.rules().iter().map(|r| r.id().as_str()).collect();
        assert!(ids.contains(&"E001"));
        assert!(ids.contains(&"E002"));
        assert!(ids.contains(&"E003"));
        assert!(ids.contains(&"E004"));
        assert!(ids.contains(&"E005"));
        assert!(ids.contains(&"E006"));
        assert!(ids.contains(&"E007"));
        assert!(ids.contains(&"E008"));
        assert!(ids.contains(&"E009"));
        assert!(ids.contains(&"S001"));
        assert!(ids.contains(&"S002"));
        assert!(ids.contains(&"E010"));
        assert!(ids.contains(&"E011"));
        assert!(ids.contains(&"E012"));
        assert!(ids.contains(&"E013"));
        assert!(ids.contains(&"E014"));
        assert!(ids.contains(&"E015"));
        // W001 retired in T035c-14 (CAPCO-2016 §F treats legacy markings
        // as unauthorized, not "deprecated but legal" — no authoritative
        // bucket for a warning-severity rule).
        assert!(!ids.contains(&"W001"), "W001 retired in T035c-14");
        assert!(ids.contains(&"W002"));
        assert!(ids.contains(&"E016"));
        // E017/E018/E019 retired in T035b (over-restrictive vs
        // CAPCO §H.3 line 4140). Replacement: E036.
        assert!(!ids.contains(&"E017"), "E017 retired in T035b");
        assert!(!ids.contains(&"E018"), "E018 retired in T035b");
        assert!(!ids.contains(&"E019"), "E019 retired in T035b");
        assert!(ids.contains(&"E020"));
        assert!(ids.contains(&"E021"));
        assert!(ids.contains(&"E022"));
        assert!(ids.contains(&"E023"));
        assert!(ids.contains(&"E024"));
        assert!(ids.contains(&"E025"));
        assert!(ids.contains(&"W003"));
        assert!(ids.contains(&"C001"));
        assert!(ids.contains(&"E026"));
        assert!(ids.contains(&"E027"));
        assert!(ids.contains(&"E028"));
        assert!(ids.contains(&"E029"));
        assert!(ids.contains(&"E030"));
        assert!(ids.contains(&"E031"));
        assert!(ids.contains(&"E032"));
        assert!(ids.contains(&"E033"));
        assert!(ids.contains(&"E034"));
        assert!(ids.contains(&"E035"));
        assert!(ids.contains(&"E036"));
        // T035b: retired 3 rules (E017/E018/E019), added 1 (E036).
        // Net count pre-T035c-1b: 39 - 3 + 1 = 37.
        // T035c-1b: added S001 (prefer-banner-abbreviation). Net: 38.
        // T035c-8: added S002 (banner-consistent-form). Net: 39.
        // T035c-14: retired W001 (deprecated-marking-warning; §F
        // treats legacy markings as unauthorized, not "deprecated
        // but legal"). Net: 38.
        assert_eq!(set.rules().len(), 38);
    }

    #[test]
    fn e001_fires_on_abbreviated_dissem_in_banner_with_real_span() {
        let diags = lint_banner("TOP SECRET//SI//NF");
        let e001: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E001").collect();
        assert_eq!(e001.len(), 1);
        // The span must point at the literal "NF" bytes — not at 0..0.
        let src = b"TOP SECRET//SI//NF";
        assert_eq!(e001[0].span.as_str(src).unwrap(), "NF");
    }

    #[test]
    fn e001_does_not_fire_on_full_form_noforn() {
        let diags = lint_banner("TOP SECRET//SI//NOFORN");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E001"));
    }

    #[test]
    fn e001_fires_on_non_ic_dissem_portion_form_in_banner() {
        // "DS" is the portion form of LIMDIS; a banner should use "LIMDIS".
        let diags = lint_banner("SECRET//DS");
        let e001: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E001").collect();
        assert_eq!(e001.len(), 1, "E001 must fire on DS in banner: {diags:?}");
        let src = b"SECRET//DS";
        assert_eq!(e001[0].span.as_str(src).unwrap(), "DS");
        let fix = e001[0].fix.as_ref().expect("E001 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "LIMDIS");
    }

    #[test]
    fn e001_does_not_fire_on_non_ic_dissem_banner_form_in_banner() {
        // "LIMDIS" is the correct banner form — E001 must not fire.
        let diags = lint_banner("SECRET//LIMDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E001"),
            "E001 must not fire when banner uses LIMDIS (correct banner form): {diags:?}"
        );
    }

    #[test]
    fn e001_does_not_fire_on_non_ic_dissem_with_equal_banner_portion() {
        // SBU/LES/SSI have identical banner and portion forms — no correction.
        let diags = lint_banner("UNCLASSIFIED//SBU");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E001"),
            "E001 must not fire when banner=portion for SBU: {diags:?}"
        );
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
    // alphabetical per CAPCO-2016 §H.8 line 3714) in a single pass. This
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
    fn e002_fix_span_stops_at_non_delimiter_tail() {
        // When the block tail contains non-delimiter content (here the
        // literal unknown token `FVEY`, which is a tetragraph marker
        // that we cannot represent as a 3-byte `Trigraph` today), the
        // fix span must NOT extend through it — otherwise the splice
        // would silently delete the user's tetragraph. Lock this.
        let src = "SECRET//REL TO GBR, AUS, FVEY";
        let diags = lint_banner(src);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a fix");
        // Span must stop at end-of-AUS, not swallow `, FVEY`.
        assert_eq!(
            fix.span.as_str(src.as_bytes()).unwrap(),
            "GBR, AUS",
            "fix span must not swallow tetragraph content in the tail"
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
        // neither E002 nor E020 should fire on the rewritten banner.
        let round2_banner = format!("CONFIDENTIAL//REL TO {fixed}");
        let diags_round2 = lint_banner(&round2_banner);
        assert!(
            diags_round2
                .iter()
                .all(|d| d.rule.as_str() != "E002" && d.rule.as_str() != "E020"),
            "E002's canonical output must not fire E002 or E020: {diags_round2:?}"
        );
    }

    #[test]
    fn e003_fires_on_misordered_blocks() {
        // SCI (SI) appears AFTER dissem (NF) — out of order.
        let diags = lint_banner("SECRET//NF//SI");
        let e003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E003").collect();
        assert_eq!(e003.len(), 1);
    }

    #[test]
    fn e003_does_not_fire_on_correct_order() {
        let diags = lint_banner("SECRET//SI//NOFORN");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E003"));
    }

    #[test]
    fn e003_does_not_fire_on_non_ic_dissem_last() {
        // Non-IC dissem as last block is the correct CAPCO order.
        let diags = lint_banner("SECRET//NOFORN//LIMDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E003"),
            "non-IC dissem after dissem is correct order: {diags:?}"
        );
    }

    #[test]
    fn e003_fires_on_non_ic_dissem_before_ic_dissem() {
        // Non-IC dissem (LIMDIS) before IC dissem (NOFORN) is out of order.
        let diags = lint_banner("SECRET//LIMDIS//NOFORN");
        let e003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E003").collect();
        assert_eq!(
            e003.len(),
            1,
            "E003 must fire when non-IC dissem precedes IC dissem"
        );
        // The reordered fix must preserve the non-IC dissem as the last block.
        let fix = e003[0].fix.as_ref().expect("E003 must carry FixProposal");
        assert!(
            fix.replacement.as_ref().ends_with("//LIMDIS"),
            "non-IC dissem must be last in reordered output: {}",
            fix.replacement.as_ref()
        );
    }

    // --- T035c-3 regressions: AEA + FGI in block order ---
    //
    // Before T035c-3, `ordinal_for_block` skipped `TokenKind::AeaMarking`
    // and `TokenKind::FgiMarker`, so the rule silently missed
    // Dissem-before-AEA and Dissem-before-FGI misorderings. These
    // tests pin the corrected §A.6 block ordinals (Class→SCI→SAR→
    // AEA→FGI→Dissem→NonIC).

    #[test]
    fn e003_fires_on_rel_to_before_aea() {
        // RD belongs in the AEA block (ordinal 3); REL TO is a dissem
        // control (ordinal 5). REL TO-before-RD is out of order.
        let diags = lint_banner("SECRET//REL TO USA//RD");
        let e003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E003").collect();
        assert_eq!(
            e003.len(),
            1,
            "E003 must fire when REL TO precedes AEA (RD): {diags:?}"
        );
        // Verify `reorder_marking` emits the AEA block before REL TO.
        let fix = e003[0].fix.as_ref().expect("E003 must carry a FixProposal");
        let replacement = fix.replacement.as_ref();
        let rd_idx = replacement
            .find("RD")
            .expect("reordered output must contain RD");
        let rel_idx = replacement
            .find("REL TO")
            .expect("reordered output must contain REL TO");
        assert!(
            rd_idx < rel_idx,
            "AEA (RD) must precede REL TO in reordered output: {replacement:?}"
        );
    }

    #[test]
    fn e003_fires_on_dissem_before_fgi() {
        // FGI (ordinal 4) must precede dissem controls (ordinal 5).
        // NOFORN-before-FGI is out of order.
        let diags = lint_banner("SECRET//NOFORN//FGI GBR");
        let e003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E003").collect();
        assert_eq!(
            e003.len(),
            1,
            "E003 must fire when dissem precedes FGI: {diags:?}"
        );
        // Verify `reorder_marking` emits the FGI block before Dissem.
        let fix = e003[0].fix.as_ref().expect("E003 must carry a FixProposal");
        let replacement = fix.replacement.as_ref();
        let fgi_idx = replacement
            .find("FGI")
            .expect("reordered output must contain FGI");
        let nf_idx = replacement
            .find("NOFORN")
            .expect("reordered output must contain NOFORN");
        assert!(
            fgi_idx < nf_idx,
            "FGI must precede Dissem (NOFORN) in reordered output: {replacement:?}"
        );
    }

    #[test]
    fn e003_does_not_fire_on_aea_then_rel_to() {
        // Correct order per §A.6: RD (AEA, ordinal 3) then REL TO
        // (dissem, ordinal 5).
        let diags = lint_banner("SECRET//RD//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E003"),
            "E003 must not fire on AEA-before-Dissem: {diags:?}"
        );
    }

    #[test]
    fn e003_does_not_fire_on_fgi_then_dissem() {
        // Correct order per §A.6: FGI (ordinal 4) then Dissem (5).
        let diags = lint_banner("SECRET//FGI GBR//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E003"),
            "E003 must not fire on FGI-before-Dissem: {diags:?}"
        );
    }

    #[test]
    fn e003_emits_fix_proposal_with_confidence_06() {
        // T032: E003 must emit a FixProposal at confidence 0.6 (suggestion-
        // only under default 0.95 threshold) so consumers that lower the
        // threshold or surface fixes in IDE quick-fixes can act on it.
        let diags = lint_banner("SECRET//NOFORN//SI");
        let e003 = diags
            .iter()
            .find(|d| d.rule.as_str() == "E003")
            .expect("E003 must fire");
        let fix = e003
            .fix
            .as_ref()
            .expect("E003 must carry a FixProposal (T032)");
        assert!((fix.confidence.combined() - 0.6).abs() < f32::EPSILON);
        assert_eq!(fix.replacement.as_ref(), "SECRET//SI//NOFORN");
    }

    #[test]
    fn e004_fires_on_redundant_separator() {
        // `////` between SECRET and NOFORN is two separators back-to-back.
        let diags = lint_banner("SECRET////NOFORN");
        let e004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E004").collect();
        assert_eq!(e004.len(), 1);
        let src = b"SECRET////NOFORN";
        assert_eq!(e004[0].span.as_str(src).unwrap(), "////");
    }

    #[test]
    fn e004_run_does_not_double_fire_same_category_check() {
        // `SECRET//SI////TK//NOFORN` — SI and TK are both SCI, but the `////`
        // run owns those separator spans. The same-category check must NOT fire
        // on the adjacent separators, so exactly one E004 (for `////`) fires.
        let diags = lint_banner("SECRET//SI////TK//NOFORN");
        let e004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E004").collect();
        assert_eq!(
            e004.len(),
            1,
            "only the `////` run diagnostic must fire, not same-cat duplicates: {e004:?}"
        );
        let src = b"SECRET//SI////TK//NOFORN";
        assert_eq!(e004[0].span.as_str(src).unwrap(), "////");
    }

    #[test]
    fn e004_does_not_fire_on_clean_separator() {
        let diags = lint_banner("SECRET//NOFORN");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E004"));
    }

    #[test]
    fn e004_fires_on_same_category_sci_double_slash() {
        // Per CAPCO-2016 §A.6 Figure 2: SI and TK are both SCI controls and
        // must be joined with `/` within one block, not `//` across blocks.
        let diags = lint_banner("SECRET//SI//TK//NOFORN");
        let e004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E004").collect();
        assert_eq!(
            e004.len(),
            1,
            "exactly one E004 on the SI//TK boundary: {diags:?}"
        );
        let src = b"SECRET//SI//TK//NOFORN";
        // The span must point at the `//` between SI and TK (bytes 10..12).
        assert_eq!(e004[0].span.as_str(src).unwrap(), "//");
        assert_eq!(e004[0].span.start, 10);
        assert_eq!(e004[0].span.end, 12);
        let fix = e004[0].fix.as_ref().expect("E004 must carry a FixProposal");
        assert_eq!(fix.original.as_ref(), "//");
        assert_eq!(fix.replacement.as_ref(), "/");
        assert!((fix.confidence.combined() - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn e004_fires_on_same_category_dissem_double_slash() {
        // ORCON and NOFORN are both dissem controls — must be joined with `/`.
        let diags = lint_banner("SECRET//ORCON//NOFORN");
        let e004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E004").collect();
        assert_eq!(
            e004.len(),
            1,
            "exactly one E004 on ORCON//NOFORN: {diags:?}"
        );
        let src = b"SECRET//ORCON//NOFORN";
        assert_eq!(e004[0].span.as_str(src).unwrap(), "//");
        let fix = e004[0].fix.as_ref().unwrap();
        assert_eq!(fix.replacement.as_ref(), "/");
    }

    #[test]
    fn e004_does_not_fire_on_different_categories() {
        // SCI (SI) and Dissem (NOFORN) are different categories — `//` is correct.
        let diags = lint_banner("SECRET//SI//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E004"),
            "E004 must not fire between different categories: {diags:?}"
        );
    }

    #[test]
    fn e004_does_not_fire_on_correct_within_category_slash() {
        // `SI/TK` — already correct within-category form.
        let diags = lint_banner("SECRET//SI/TK//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E004"),
            "E004 must not fire on correct `/` between same-category values: {diags:?}"
        );
    }

    #[test]
    fn e004_does_not_fire_when_one_side_is_unknown() {
        // XYZZY is unclassifiable; we can't prove same-category so do not fire.
        let diags = lint_banner("SECRET//XYZZY//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E004"),
            "E004 must not fire when either side is Unknown: {diags:?}"
        );
    }

    // T035c-11 pin-downs.

    #[test]
    fn e004_does_not_fire_on_fgi_space_separated_codes() {
        // Per CAPCO-2016 §A.6 line 332, multiple FGI codes are separated
        // by a SPACE, not `/`. `SeparatorCategory` intentionally omits
        // FGI so E004 does not misfire with a `/` fix (which would be
        // wrong for FGI). Lock this intentional exclusion down.
        let diags = lint_banner("SECRET//FGI GBR JPN//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E004"),
            "E004 must not fire on space-separated FGI codes (§A.6 line \
             332 mandates space, not /): {diags:?}"
        );
    }

    #[test]
    fn e004_does_not_fire_on_fgi_with_double_slash_between_codes() {
        // Even when a user writes FGI codes with `//` between them (a
        // malformed marking), E004 must not propose `/` — that would
        // replace one wrong separator with another wrong separator. The
        // correct form uses a single space (§A.6 line 332). A separate
        // rule would be needed to catch this specific error; E004's
        // contract is explicitly limited to categories whose sibling
        // separator is `/`.
        let diags = lint_banner("SECRET//FGI GBR//JPN//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E004"),
            "E004 must not propose `/` between FGI codes (would be wrong \
             fix — correct form uses space): {diags:?}"
        );
    }

    #[test]
    fn e004_collapses_longer_separator_runs() {
        // `//////` (three `//` separators back-to-back) must still
        // collapse. §D.1 line 558 prohibits any placeholder slashes,
        // regardless of run length. This locks behavior against a future
        // regression where only the minimum `////` case is recognized.
        let diags = lint_banner("SECRET//////NOFORN");
        let e004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E004").collect();
        assert!(
            !e004.is_empty(),
            "E004 must fire on `//////` (3-separator run): {diags:?}"
        );
        // At least one diag must carry a fix that canonicalizes to `//`.
        assert!(
            e004.iter().any(|d| d
                .fix
                .as_ref()
                .is_some_and(|f| f.replacement.as_ref() == "//")),
            "at least one E004 diag must propose `//`: {e004:?}"
        );
    }

    #[test]
    fn e004_does_not_fire_on_hyphen_connected_sci_compartment() {
        // `SI-G` is SI with compartment G, connected by hyphen per
        // §A.6 line 319. No `//` exists between SI and G, so E004 has
        // no separator to fire on. This pins down that E004 does not
        // misread the hyphen as a category boundary or otherwise
        // double-fire with the SCI structural parser.
        let diags = lint_banner("SECRET//SI-G//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E004"),
            "E004 must not fire on hyphen-connected SCI compartment: {diags:?}"
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
    fn e008_suppressed_on_second_sar_block_with_abbrev_prefix() {
        // Second SAR block (`SAR-DUPE`) is tagged Unknown by the
        // parser so E030 (sar-indicator-repeat) can surface the
        // duplicate per CAPCO-2016 §H.5. E008 must step aside AND
        // E030 must actually fire — otherwise a future change that
        // drops E030 (or breaks its preconditions) could produce a
        // silent suppression with no diagnostic at all.
        let diags = lint_banner("SECRET//SAR-ABC//NF//SAR-DUPE");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        let e030: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E030").collect();
        assert!(
            e008.is_empty(),
            "E008 must be suppressed for second SAR block (E030 owns): \
             {diags:?}"
        );
        assert!(
            !e030.is_empty(),
            "E030 must fire on the repeated SAR block — otherwise \
             suppression is a silent drop: {diags:?}"
        );
    }

    #[test]
    fn e008_suppressed_on_second_sar_block_with_spelled_prefix() {
        // Same as above but with the spelled-out `SPECIAL ACCESS
        // REQUIRED-` category indicator. Banner form is rarely used
        // but must be covered — the suppression check keys on the
        // prefix string. Also asserts E030 is present (see
        // `e008_suppressed_on_second_sar_block_with_abbrev_prefix`
        // for the rationale).
        let diags =
            lint_banner("SECRET//SPECIAL ACCESS REQUIRED-ABC//NF//SPECIAL ACCESS REQUIRED-DUPE");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        let e030: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E030").collect();
        assert!(
            e008.is_empty(),
            "E008 must be suppressed for second `SPECIAL ACCESS \
             REQUIRED-` block (E030 owns): {diags:?}"
        );
        assert!(
            !e030.is_empty(),
            "E030 must fire on the repeated `SPECIAL ACCESS \
             REQUIRED-` block — otherwise suppression is a silent \
             drop: {diags:?}"
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

    // --- E009: Portion abbreviation rule ---

    #[test]
    fn e009_fires_on_banner_form_classification_in_portion() {
        let diags = lint_portion("(SECRET//NF)");
        let e009: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E009").collect();
        assert_eq!(
            e009.len(),
            1,
            "single-token fix must produce exactly one E009: {diags:?}"
        );
        let src = b"(SECRET//NF)";
        assert_eq!(e009[0].span.as_str(src).unwrap(), "SECRET");
        let fix = e009[0].fix.as_ref().expect("E009 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "S");
        // Lock down T035c-13 per-branch citation retargeting:
        // classification uses §H.1 (US Classification Markings).
        assert_eq!(
            e009[0].citation, "CAPCO-2016 §H.1 (US Classification Markings)",
            "classification branch must cite §H.1 per T035c-13"
        );
    }

    #[test]
    fn e009_fires_on_banner_form_dissem_in_portion() {
        let diags = lint_portion("(S//NOFORN)");
        let e009: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E009").collect();
        assert_eq!(
            e009.len(),
            1,
            "single-token fix must produce exactly one E009: {diags:?}"
        );
        let src = b"(S//NOFORN)";
        assert_eq!(e009[0].span.as_str(src).unwrap(), "NOFORN");
        let fix = e009[0].fix.as_ref().expect("E009 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "NF");
        // Lock down T035c-13 per-branch citation retargeting:
        // IC dissem controls cite §H.8.
        assert_eq!(
            e009[0].citation, "CAPCO-2016 §H.8",
            "IC dissem branch must cite §H.8 per T035c-13"
        );
    }

    #[test]
    fn e009_fires_on_both_classification_and_dissem() {
        let diags = lint_portion("(TOP SECRET//ORCON)");
        let e009: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E009").collect();
        assert_eq!(e009.len(), 2);
    }

    #[test]
    fn e009_does_not_fire_on_abbreviated_portion() {
        let diags = lint_portion("(TS//SI//NF)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E009"),
            "E009 must not fire on correctly abbreviated portion, got: {diags:?}"
        );
    }

    #[test]
    fn e009_does_not_fire_on_banner() {
        // E009 is portion-only; banner-form in banners is correct.
        let diags = lint_banner("TOP SECRET//SI//NOFORN");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E009"));
    }

    #[test]
    fn e009_fires_on_non_ic_dissem_banner_form_in_portion() {
        // "LIMDIS" is the banner form; a portion should use "DS".
        let diags = lint_portion("(S//LIMDIS)");
        let e009: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E009").collect();
        assert_eq!(
            e009.len(),
            1,
            "E009 must fire on LIMDIS in portion: {diags:?}"
        );
        let src = b"(S//LIMDIS)";
        assert_eq!(e009[0].span.as_str(src).unwrap(), "LIMDIS");
        let fix = e009[0].fix.as_ref().expect("E009 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "DS");
        // Lock down T035c-13 per-branch citation retargeting:
        // Non-IC dissem controls cite §H.9.
        assert_eq!(
            e009[0].citation, "CAPCO-2016 §H.9",
            "Non-IC dissem branch must cite §H.9 per T035c-13"
        );
    }

    #[test]
    fn e009_does_not_fire_on_non_ic_dissem_portion_form_in_portion() {
        // "DS" is the correct portion form — E009 must not fire.
        let diags = lint_portion("(S//DS)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E009"),
            "E009 must not fire when portion uses DS (correct portion form): {diags:?}"
        );
    }

    #[test]
    fn e009_does_not_fire_on_non_ic_dissem_with_equal_banner_portion() {
        // SBU/LES/SSI have identical banner and portion forms — no correction.
        let diags = lint_portion("(U//LES)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E009"),
            "E009 must not fire when banner=portion for LES: {diags:?}"
        );
    }

    // T035c-13: pin-down tests for per-branch citation coverage and
    // classification-level + dissem-form breadth.

    #[test]
    fn e009_fires_on_top_secret_banner_form_in_portion() {
        // CAPCO-2016 §H.1 (p47 line 988): TOP SECRET → TS.
        let diags = lint_portion("(TOP SECRET//NF)");
        let e009: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E009").collect();
        assert!(
            !e009.is_empty(),
            "E009 must fire on TOP SECRET in portion: {diags:?}"
        );
        let fix = e009[0].fix.as_ref().expect("E009 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "TS");
    }

    #[test]
    fn e009_fires_on_confidential_banner_form_in_portion() {
        // CAPCO-2016 §H.1 (p50 line 1074): CONFIDENTIAL → C.
        let diags = lint_portion("(CONFIDENTIAL)");
        let e009: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E009").collect();
        assert!(
            !e009.is_empty(),
            "E009 must fire on CONFIDENTIAL in portion: {diags:?}"
        );
        let fix = e009[0].fix.as_ref().expect("E009 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "C");
    }

    #[test]
    fn e009_fires_on_unclassified_banner_form_in_portion() {
        // CAPCO-2016 §H.1 (p51 line 1114): UNCLASSIFIED → U.
        let diags = lint_portion("(UNCLASSIFIED)");
        let e009: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E009").collect();
        assert!(
            !e009.is_empty(),
            "E009 must fire on UNCLASSIFIED in portion: {diags:?}"
        );
        let fix = e009[0].fix.as_ref().expect("E009 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "U");
    }

    #[test]
    fn e009_fires_on_orcon_banner_form_in_portion() {
        // CAPCO-2016 §H.8: ORCON → OC. Different dissem control from
        // NOFORN, so this locks breadth beyond the single NOFORN case.
        let diags = lint_portion("(S//ORCON)");
        let e009: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E009").collect();
        assert!(
            !e009.is_empty(),
            "E009 must fire on ORCON in portion: {diags:?}"
        );
        let fix = e009[0].fix.as_ref().expect("E009 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "OC");
    }

    #[test]
    fn e009_does_not_fire_on_dissem_with_equal_banner_portion() {
        // RELIDO has identical banner and portion forms — no
        // substitution possible. E009 must stay silent rather than
        // firing with an empty replacement.
        let diags = lint_portion("(S//RELIDO)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E009"),
            "E009 must not fire when banner=portion for RELIDO: {diags:?}"
        );
    }

    // T035c-1b: S001 prefer-banner-abbreviation (style). Fires when a
    // banner uses the long "Marking Title" form where a distinct
    // abbreviation is authorized. Severity is Info — both forms are
    // legal per CAPCO-2016 §A.6 line 317; the rule encodes the common
    // IC-element preference for the shorter abbreviation.

    #[test]
    fn s001_fires_on_long_title_dissem_in_banner() {
        // "NOT RELEASABLE TO FOREIGN NATIONALS" is the §G.1 Table 4
        // long title for NOFORN. S001 proposes the NOFORN abbreviation.
        let diags = lint_banner("SECRET//NOT RELEASABLE TO FOREIGN NATIONALS");
        let s001: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S001").collect();
        assert_eq!(s001.len(), 1, "{diags:?}");
        let fix = s001[0].fix.as_ref().expect("S001 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "NOFORN");
        assert_eq!(s001[0].severity, marque_rules::Severity::Info);
    }

    #[test]
    fn s001_fires_on_long_title_orcon_in_banner() {
        let diags = lint_banner("SECRET//ORIGINATOR CONTROLLED");
        let s001: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S001").collect();
        assert_eq!(s001.len(), 1, "{diags:?}");
        let fix = s001[0].fix.as_ref().expect("S001 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "ORCON");
    }

    #[test]
    fn s001_fires_on_long_title_non_ic_dissem_in_banner() {
        // "LIMITED DISTRIBUTION" is the long title for LIMDIS — non-IC
        // branch. S001 must cover both dissem and non-IC categories.
        let diags = lint_banner("SECRET//NOFORN//LIMITED DISTRIBUTION");
        let s001: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S001").collect();
        assert_eq!(s001.len(), 1, "{diags:?}");
        let fix = s001[0].fix.as_ref().expect("S001 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "LIMDIS");
    }

    #[test]
    fn s001_does_not_fire_on_banner_abbrev_form() {
        // Abbreviation is already the preferred form — no diag.
        let diags = lint_banner("SECRET//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S001"),
            "S001 must not fire on abbreviation form: {diags:?}"
        );
    }

    #[test]
    fn s001_does_not_fire_in_portion() {
        // Portion form E009 owns; S001 is banner-only (would
        // otherwise double-fire on a portion that contains a long
        // title).
        let diags = lint_portion("(S//NOT RELEASABLE TO FOREIGN NATIONALS)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S001"),
            "S001 must not fire in portion context: {diags:?}"
        );
    }

    #[test]
    fn s001_does_not_fire_on_dea_sensitive() {
        // §G.1 Table 4 line 831: DEA SENSITIVE has no distinct
        // abbreviation (`| DEA SENSITIVE | None | DSEN |`). S001 must
        // stay silent — no substitution is possible, and proposing a
        // no-op replacement would be noise.
        let diags = lint_banner("SECRET//DEA SENSITIVE");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S001"),
            "S001 must not fire on DEA SENSITIVE (no distinct abbrev per §G.1 line 831): {diags:?}"
        );
    }

    // T035c-8: S002 banner-consistent-form (style). Fires exactly once
    // per banner when a mix of long-title and abbreviation forms is
    // detected. Carries no FixProposal — S001 handles per-token
    // normalization and running `marque fix` with S001 enabled will
    // drive the banner to all-abbrev form.

    #[test]
    fn s002_fires_on_mixed_title_and_abbrev_forms() {
        // Long title "ORIGINATOR CONTROLLED" + abbrev "NOFORN" in one
        // banner. S002 should fire exactly once.
        let diags = lint_banner("SECRET//ORIGINATOR CONTROLLED/NOFORN");
        let s002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S002").collect();
        assert_eq!(s002.len(), 1, "{diags:?}");
        assert!(s002[0].fix.is_none(), "S002 must not carry a fix");
        assert_eq!(s002[0].severity, marque_rules::Severity::Info);
    }

    #[test]
    fn s002_does_not_fire_on_all_abbrev_banner() {
        // Canonical all-abbrev form — not mixed.
        let diags = lint_banner("SECRET//ORCON/NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S002"),
            "S002 must not fire on all-abbrev banner: {diags:?}"
        );
    }

    #[test]
    fn s002_does_not_fire_on_all_title_banner() {
        // All long titles — not mixed. S001 still fires per token.
        let diags =
            lint_banner("SECRET//ORIGINATOR CONTROLLED/NOT RELEASABLE TO FOREIGN NATIONALS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S002"),
            "S002 must not fire on all-title banner: {diags:?}"
        );
    }

    #[test]
    fn s002_does_not_fire_on_single_token_banner() {
        // One token can't mix with itself. Lock this so an off-by-one
        // in the counter doesn't silently fire.
        let diags = lint_banner("SECRET//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S002"),
            "S002 must not fire on single-token banner: {diags:?}"
        );
    }

    #[test]
    fn s002_does_not_fire_on_dea_sensitive_plus_abbrev() {
        // DEA SENSITIVE has `title == banner` per §G.1 line 831 — it
        // does NOT count as either title-form or abbrev-form for the
        // mix scoring. A banner of DEA SENSITIVE + NOFORN is not mixed.
        let diags = lint_banner("SECRET//NOFORN/DEA SENSITIVE");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S002"),
            "S002 must not fire when same-form rows (DEA SENSITIVE) \
             appear alongside abbreviations: {diags:?}"
        );
    }

    #[test]
    fn s002_fires_on_non_ic_mixed_with_dissem() {
        // Mix across categories: dissem long-title + non-IC abbreviation.
        let diags = lint_banner("SECRET//NOT RELEASABLE TO FOREIGN NATIONALS//LIMDIS");
        let s002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S002").collect();
        assert_eq!(
            s002.len(),
            1,
            "S002 must count tokens across dissem and non-IC categories: {diags:?}"
        );
    }

    #[test]
    fn s002_does_not_fire_in_portion() {
        // S002 is banner-only — portion doesn't authorize either form
        // of long title anyway (E009 catches them).
        let diags = lint_portion("(S//ORCON/NOT RELEASABLE TO FOREIGN NATIONALS)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S002"),
            "S002 must not fire in portion context: {diags:?}"
        );
    }

    #[test]
    fn s002_fires_exactly_once_regardless_of_long_title_count() {
        // Three long titles + one abbrev. S002 should fire exactly
        // once per banner, not per token.
        let diags = lint_banner(
            "SECRET//ORIGINATOR CONTROLLED/NOT RELEASABLE TO FOREIGN NATIONALS/CAUTION-PROPRIETARY INFORMATION INVOLVED//LIMDIS",
        );
        let s002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S002").collect();
        assert_eq!(
            s002.len(),
            1,
            "S002 must fire exactly once per banner: {diags:?}"
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

    // --- E013: JOINT/REL TO delimiter mismatch (T035c-17 audit) ---

    #[test]
    fn e013_fires_on_joint_comma_with_space_after() {
        // Canonical JOINT uses single space; a trailing-space comma like
        // `USA, GBR` must fire and fix to `USA GBR`.
        let diags = lint_banner("//JOINT S USA, GBR");
        let e013: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E013").collect();
        assert_eq!(e013.len(), 1, "E013 must fire on JOINT with comma: {diags:?}");
        let fix = e013[0].fix.as_ref().expect("E013 JOINT must carry a fix");
        assert_eq!(
            fix.replacement.as_ref(),
            "JOINT S USA GBR",
            "fix must replace `,` with single space"
        );
        assert!(
            e013[0].citation.contains("§H.3 p56 line 1258"),
            "JOINT citation must pin §H.3 p56 line 1258; got: {:?}",
            e013[0].citation
        );
    }

    #[test]
    fn e013_joint_fix_handles_extra_whitespace_in_comma_list() {
        // Regression: the prior implementation did
        // `text.replace(',', "").replace("  ", " ")` which handled at
        // most one run of double spaces. Inputs with three or more
        // intervening spaces (e.g., `USA,   GBR`) survived as `USA
        //  GBR` after the naive collapse. The new implementation uses
        // `split_whitespace().join(" ")` which normalizes any run of
        // whitespace, so this class of input fixes cleanly.
        //
        // Note on the harder case: `USA,GBR` (comma, no space) is a
        // parser-boundary limitation. The parser's JOINT subparser
        // requires whitespace between trigraphs, so `USA,GBR` fails
        // grammar entirely and `attrs.classification` is `None` —
        // E013 has no JOINT context to inspect. Fixing that case
        // would require either parser-level degradation tolerance
        // or a pre-scanner normalization pass; both are out of scope
        // for this rule-level audit.
        let diags = lint_banner("//JOINT S USA,   GBR");
        let e013: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E013").collect();
        assert_eq!(e013.len(), 1, "E013 must fire on comma-plus-spaces JOINT: {diags:?}");
        let fix = e013[0].fix.as_ref().expect("E013 must carry a fix");
        assert_eq!(
            fix.replacement.as_ref(),
            "JOINT S USA GBR",
            "comma + extra whitespace must normalize to single space"
        );
    }

    #[test]
    fn e013_does_not_fire_on_correctly_space_delimited_joint() {
        let diags = lint_banner("//JOINT S USA GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E013"),
            "E013 must not fire on canonical JOINT: {diags:?}"
        );
    }

    #[test]
    fn e013_fires_on_rel_to_space_only_delimiter() {
        let diags = lint_banner("SECRET//REL TO USA GBR");
        let e013: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E013").collect();
        assert_eq!(e013.len(), 1, "E013 must fire on space-only REL TO: {diags:?}");
        let fix = e013[0].fix.as_ref().expect("E013 REL TO must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "REL TO USA, GBR");
        assert!(
            e013[0].citation.contains("§H.8 p150-151 line 3714"),
            "REL TO citation must pin §H.8 p150-151 line 3714; got: {:?}",
            e013[0].citation
        );
    }

    #[test]
    fn e013_fires_on_rel_to_missing_space_after_comma() {
        // Previous predicate only checked `!contains(',')`, so it
        // silently passed `USA,GBR` (comma but no space). Canonical
        // form must insert the space.
        let diags = lint_banner("SECRET//REL TO USA,GBR");
        let e013: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E013").collect();
        assert_eq!(
            e013.len(),
            1,
            "E013 must fire on REL TO without space after comma: {diags:?}"
        );
        let fix = e013[0].fix.as_ref().expect("E013 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "REL TO USA, GBR");
    }

    #[test]
    fn e013_fires_on_rel_to_mixed_delimiters() {
        // Regression: previously the predicate only fired when the
        // country list contained NO commas. Mixed-delimiter input
        // like `USA GBR,AUS` passed silently even though §H.8 line
        // 3714 requires comma-space between every pair.
        let diags = lint_banner("SECRET//REL TO USA GBR,AUS");
        let e013: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E013").collect();
        assert_eq!(
            e013.len(),
            1,
            "E013 must fire on mixed-delimiter REL TO: {diags:?}"
        );
        let fix = e013[0].fix.as_ref().expect("E013 must carry a fix");
        assert_eq!(
            fix.replacement.as_ref(),
            "REL TO USA, GBR, AUS",
            "mixed delimiters must canonicalize to comma-space"
        );
    }

    #[test]
    fn e013_does_not_fire_on_canonical_rel_to() {
        let diags = lint_banner("SECRET//REL TO USA, GBR, AUS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E013"),
            "E013 must not fire on canonical REL TO: {diags:?}"
        );
    }

    #[test]
    fn e013_does_not_fire_on_rel_to_with_single_country() {
        // A single country code has no pair to delimit. E013 must be
        // silent (not synthesize a no-op fix).
        let diags = lint_banner("SECRET//REL TO USA");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E013"),
            "E013 must not fire on single-country REL TO: {diags:?}"
        );
    }

    #[test]
    fn e013_rel_to_fix_does_not_treat_to_as_country_code_on_double_space() {
        // Regression for PR #95 review: the earlier implementation did
        // `strip_prefix("REL TO").or_else(|| strip_prefix("REL"))`, so
        // for input `REL  TO USA GBR` (extra space between `REL` and
        // `TO`) the first prefix failed to match (no literal single
        // space), the second succeeded, and `TO` was left in the
        // token stream to be treated as a country code. The fix then
        // produced `REL TO TO, USA, GBR`. The new implementation
        // tokenizes the whole block and `skip_while`s leading `REL`
        // / `TO` keywords, which is robust to non-canonical prefix
        // whitespace.
        //
        // Acceptable outcomes for this input (either passes — it
        // depends on whether the scanner normalizes the prefix
        // whitespace before the rule sees it):
        //   A) E013 does not fire — scanner normalized the prefix.
        //   B) E013 fires with `replacement == "REL TO USA, GBR"`.
        //
        // What is NOT acceptable:
        //   C) E013 fires with `replacement` containing the phantom
        //      `TO, USA` / `TO TO` that the old buggy prefix
        //      stripping would produce.
        //   D) E013 fires WITHOUT a fix (every E013 diagnostic must
        //      carry a FixProposal today).
        let diags = lint_banner("SECRET//REL  TO USA GBR");
        let e013: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E013").collect();
        assert!(
            e013.len() <= 1,
            "E013 fires at most once per REL TO block: {diags:?}"
        );
        if let Some(d) = e013.first() {
            let fix = d
                .fix
                .as_ref()
                .expect("E013 diagnostics must carry a FixProposal");
            assert_eq!(
                fix.replacement.as_ref(),
                "REL TO USA, GBR",
                "canonical fix must drop leading REL/TO keywords and not \
                 reinterpret `TO` as a country code"
            );
        }
    }

    #[test]
    fn e013_does_not_fire_on_prefix_only_whitespace_with_canonical_list() {
        // E013 is a delimiter-mismatch rule. Prefix-only issues like
        // extra whitespace between `REL` and `TO`, when the country
        // list itself is already comma-space canonical, are out of
        // scope. A future rule targeting prefix normalization can
        // own that case. Scoping E013 strictly to the list region
        // keeps the diagnostic message ("REL TO country list must use
        // comma-space delimiters…") accurate — it would be misleading
        // to fire for a problem the message doesn't describe.
        //
        // Whether this input actually produces a RelToBlock with
        // non-canonical prefix depends on scanner behavior; the rule
        // must be correct either way, so we accept both "no E013" and
        // "E013 does not fire" — the only failure mode would be if
        // E013 fired with a canonical-ish fix on a list that was
        // already canonical.
        let diags = lint_banner("SECRET//REL  TO USA, GBR");
        for d in diags.iter().filter(|d| d.rule.as_str() == "E013") {
            if let Some(fix) = d.fix.as_ref() {
                // If the rule does fire here, it would be because
                // the scanner preserved the double space AND the
                // rule chose to treat that as a list problem — which
                // is the scope violation we're guarding against.
                panic!(
                    "E013 must not fire on prefix-only whitespace when \
                     the list itself is canonical; got replacement: {:?}",
                    fix.replacement.as_ref()
                );
            }
        }
    }

    #[test]
    fn e013_preserves_token_order_in_rel_to_fix() {
        // E020 owns ordering; E013 must canonicalize delimiters
        // WITHOUT reordering. Input `USA GBR AUS` (space-delimited,
        // wrong canonical alpha order) gets comma-delimited but
        // keeps `USA, GBR, AUS`, not `USA, AUS, GBR` — E020 fires
        // separately for the ordering issue.
        let diags = lint_banner("SECRET//REL TO USA GBR AUS");
        let e013: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E013").collect();
        assert_eq!(e013.len(), 1);
        let fix = e013[0].fix.as_ref().expect("E013 must carry a fix");
        assert_eq!(
            fix.replacement.as_ref(),
            "REL TO USA, GBR, AUS",
            "E013 fix must preserve input order — E020 handles ordering"
        );
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

    // --- E015: Non-US without dissem ---

    #[test]
    fn e015_fires_on_nato_without_dissem() {
        let diags = lint_banner("//NATO SECRET");
        let e015: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E015").collect();
        assert_eq!(e015.len(), 1);
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
        // CAPCO-2016 §H.9 line 4180: "When a document contains LIMDIS
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
            "W003 must fire on LIMDIS in classified banner (§H.9 line 4180): {diags:?}"
        );
        assert!(w003[0].message.contains("LIMDIS"));
    }

    #[test]
    fn w003_does_not_fire_on_exdis_in_classified_banner() {
        // CAPCO-2016 §H.9 line 4240: "If EXDIS is contained in any
        // portion of a document that does not contain one or more NODIS
        // portions, EXDIS must appear in the banner line." Example
        // banner on p173: SECRET//NOFORN//EXDIS. Prior impl excluded
        // EXDIS from the propagating set; the §H.9 rule is the
        // opposite.
        let diags = lint_banner("SECRET//NOFORN//EXDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "EXDIS propagates to classified banners per §H.9 line 4240: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_nodis_in_classified_banner() {
        // CAPCO-2016 §H.9 line 4300: "If NODIS is contained in any
        // portion of a document, it must appear in the banner line."
        // Example banner on p174: SECRET//NOFORN//NODIS. Prior impl
        // excluded NODIS from the propagating set; the §H.9 rule is
        // the opposite.
        let diags = lint_banner("SECRET//NOFORN//NODIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "NODIS propagates to classified banners per §H.9 line 4300: {diags:?}"
        );
    }

    #[test]
    fn w003_fires_on_sbu_nf_in_classified_banner() {
        // CAPCO-2016 §H.9 line 4408: SBU NOFORN "Applicable only to
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
            "W003 must fire on literal SBU-NF in classified banner (§H.9 line 4408): {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_les_in_classified_banner() {
        // CAPCO-2016 §H.9 line 4479: "The LES marking always appears in
        // the banner line if contained in any portion, regardless of
        // classification level." Example banners on p183: SECRET//REL
        // TO USA, FVEY//LES, SECRET//NOFORN//LES.
        let diags = lint_banner("SECRET//LES");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "LES propagates to classified banners per §H.9 line 4479: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_les_nf_in_classified_banner() {
        // CAPCO-2016 §H.9 line 4557: "The LES marking always appears
        // in the banner line if LES information (either LES or LES
        // NOFORN) is contained in the document, regardless of the
        // document's classification level." The §H.9 canonical form
        // in classified docs is "LES" at banner with NOFORN split into
        // the dissem block (line 4558), but `LES NOFORN` in a
        // classified banner is not a W003 concern — the canonicalization
        // is a separate page-rewrite concern.
        let diags = lint_banner("SECRET//LES NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "LES-NF propagates to classified banners per §H.9 line 4557: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_ssi_in_classified_banner() {
        // CAPCO-2016 §H.9 line 4651: "If the SSI marking is contained
        // in any portion of a document it must appear in the banner
        // line, regardless of the document's overall classification
        // level." Example banner on p191: SECRET//REL TO USA,
        // ACGU//SSI.
        let diags = lint_banner("SECRET//SSI");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "SSI propagates to classified banners per §H.9 line 4651: {diags:?}"
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
    // T035b. CAPCO §H.3 line 4140 permits JOINT with IC and non-IC
    // dissem (excluding only NOFORN and HCS per line 4146) and with
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
        // coverage requires constructing `IsmAttributes` directly;
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
        // than REL TO". CAPCO §H.3 line 4146 does exclude NOFORN
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
        // dissem". CAPCO §H.3 line 4140 explicitly permits non-IC
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
        // §H.3 line 4146: "May not be used with the HCS markings".
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

    #[test]
    fn e036_does_not_fire_on_joint_without_hcs() {
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E036"),
            "E036 must not fire without HCS present: {diags:?}"
        );
    }

    // --- E020: Country code ordering ---

    #[test]
    fn e020_fires_on_unordered_rel_to() {
        // GBR before AUS — should be USA, AUS, GBR.
        let diags = lint_banner("SECRET//REL TO USA, GBR, AUS");
        let e020: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E020").collect();
        assert_eq!(e020.len(), 1);
        let fix = e020[0].fix.as_ref().expect("E020 must have fix");
        assert_eq!(fix.replacement.as_ref(), "USA, AUS, GBR");
        assert!((fix.confidence.combined() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn e020_does_not_fire_on_ordered_rel_to() {
        let diags = lint_banner("SECRET//REL TO USA, AUS, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E020"),
            "E020 should not fire on correctly ordered list: {diags:?}"
        );
    }

    #[test]
    fn e020_fires_on_unordered_joint_countries() {
        // GBR before AUS in JOINT list.
        let diags = lint_banner("//JOINT S USA GBR AUS//REL TO USA, AUS, GBR");
        let e020: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E020").collect();
        assert!(
            !e020.is_empty(),
            "E020 should fire on unordered JOINT countries: {diags:?}"
        );
    }

    // T035c-10 fourth-round review: multi-RelToBlock safety.
    // Mirrors the E002 cross-block guard. A first→last `RelToTrigraph`
    // splice across the whole marking would delete intervening `//...//`
    // content when more than one REL TO block is present.

    #[test]
    fn e020_suppresses_fix_on_multiple_rel_to_blocks() {
        // USA, GBR, AUS is unordered (alphabetical after USA should be
        // AUS, GBR). With two RelToBlocks, E020 must still report the
        // ordering problem but MUST NOT carry a FixProposal — a single
        // first→last splice across the two blocks would delete the
        // intervening `//NF//` content.
        let src = "SECRET//REL TO USA, GBR//NF//REL TO AUS";
        let diags = lint_banner(src);
        let e020: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E020").collect();
        assert_eq!(
            e020.len(),
            1,
            "E020 must still fire (diagnostic present): {diags:?}"
        );
        assert!(
            e020[0].fix.is_none(),
            "E020 must NOT carry a fix when multiple REL TO blocks \
             are present (cross-block splice would delete intervening \
             `//NF//`): {e020:?}"
        );
        assert!(
            e020[0].message.contains("multiple REL TO blocks"),
            "suppression message must explain why no fix is offered: {}",
            e020[0].message
        );
    }

    #[test]
    fn e020_silent_on_ordered_list_across_multiple_rel_to_blocks() {
        // USA, AUS, GBR is already canonical; E020 must not fire even
        // when the canonical list is split across two RelToBlocks.
        let src = "SECRET//REL TO USA, AUS//NF//REL TO GBR";
        let diags = lint_banner(src);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E020"),
            "E020 must not fire on canonically-ordered list, even \
             across multiple REL TO blocks: {diags:?}"
        );
    }

    #[test]
    fn e020_fix_span_stays_inside_single_rel_to_block() {
        // When exactly one RelToBlock is present, the fix span must
        // cover first→last trigraph WITHIN that block — not stretch
        // across unrelated trigraphs elsewhere in the token stream.
        // This is the positive counterpart to the multi-block guard:
        // the block_span scope must be applied on the single-block
        // happy path too.
        let src = "SECRET//REL TO USA, GBR, AUS";
        let diags = lint_banner(src);
        let e020: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E020").collect();
        assert_eq!(e020.len(), 1);
        let fix = e020[0].fix.as_ref().expect("E020 must carry a fix");
        // Span should cover exactly `USA, GBR, AUS` — the first→last
        // trigraph range — not leak outside.
        assert_eq!(
            fix.span.as_str(src.as_bytes()).unwrap(),
            "USA, GBR, AUS",
            "fix span must cover the full trigraph range inside the block"
        );
    }

    // T035c-18: E020 standalone audit — per-branch citation lockdown
    // and JOINT fix-shape assertion.

    #[test]
    fn e020_joint_fix_produces_pure_alpha_ordering() {
        // JOINT ordering per §H.3 line 1258 is pure alphabetical —
        // no USA-first carve-out. Input `USA GBR AUS` sorts to
        // `AUS GBR USA`. The widespread IC practice of rendering USA
        // first in JOINT lists is style convention and will be owned
        // by a follow-up S003 `joint-usa-first` rule, not encoded into
        // E020's correctness fix.
        //
        // E020's JOINT fix span covers the full Classification token
        // (`JOINT S USA GBR AUS`). The replacement must therefore
        // include the `JOINT S` prefix byte-for-byte — replacing with
        // just the country list would corrupt the marking. This test
        // asserts the span, original, and replacement shapes together
        // so a regression that reverts to country-list-only replacement
        // fails here.
        let src = "//JOINT S USA GBR AUS//REL TO USA, AUS, GBR";
        let diags = lint_banner(src);
        let e020_joint: Vec<_> = diags
            .iter()
            .filter(|d| d.rule.as_str() == "E020" && d.message.contains("JOINT"))
            .collect();
        assert_eq!(
            e020_joint.len(),
            1,
            "E020 must fire exactly once for JOINT: {diags:?}"
        );
        let fix = e020_joint[0].fix.as_ref().expect("E020 JOINT must have fix");

        // Span must cover exactly the Classification token's bytes:
        // `JOINT S USA GBR AUS` (no leading `//`, no trailing `//`).
        assert_eq!(
            fix.span.as_str(src.as_bytes()).unwrap(),
            "JOINT S USA GBR AUS",
            "JOINT fix span must cover the full Classification token"
        );

        // `original` must match the span's source slice byte-for-byte.
        assert_eq!(
            fix.original.as_ref(),
            "JOINT S USA GBR AUS",
            "FixProposal.original must equal the span's source bytes"
        );

        // `replacement` must preserve the `JOINT S` prefix and produce
        // the pure-alpha-ordered country list.
        assert_eq!(
            fix.replacement.as_ref(),
            "JOINT S AUS GBR USA",
            "JOINT fix replacement must preserve the `JOINT <level>` \
             prefix and produce pure-alpha country order"
        );

        // Simulate applying the fix: splice `replacement` in place of
        // `span`'s byte range. The resulting buffer must still start
        // with `//JOINT S` — proving the fix does not corrupt the
        // marking.
        let mut buf = src.as_bytes().to_vec();
        buf.splice(
            fix.span.start..fix.span.end,
            fix.replacement.as_ref().bytes(),
        );
        let applied = std::str::from_utf8(&buf).unwrap();
        assert!(
            applied.starts_with("//JOINT S "),
            "applied fix must preserve the `//JOINT S ` banner prefix; \
             got: {applied:?}"
        );
        assert!(
            applied.contains("//JOINT S AUS GBR USA//"),
            "applied fix must emit the pure-alpha country list between \
             the expected `//` separators; got: {applied:?}"
        );

        // Message wording differs from REL TO: no "USA first when
        // present" clause.
        assert!(
            !e020_joint[0]
                .message
                .contains("USA first when present"),
            "JOINT message must NOT claim 'USA first' since §H.3 has \
             no such carve-out; got: {:?}",
            e020_joint[0].message
        );
    }

    #[test]
    fn e020_joint_fix_preserves_portion_form_level() {
        // Regression guard for the JOINT prefix-preservation logic:
        // with portion-form level `S` (single character), the
        // `JOINT S ` prefix must still be preserved. The prior bug
        // would have spliced `JOINT S` out entirely, leaving just
        // `AUS GBR USA` between the `//` separators — a malformed
        // marking.
        let src = "//JOINT S GBR AUS USA";
        let diags = lint_banner(src);
        let e020_joint: Vec<_> = diags
            .iter()
            .filter(|d| d.rule.as_str() == "E020" && d.message.contains("JOINT"))
            .collect();
        assert_eq!(e020_joint.len(), 1);
        let fix = e020_joint[0].fix.as_ref().expect("fix expected");
        assert_eq!(fix.replacement.as_ref(), "JOINT S AUS GBR USA");
        assert_eq!(fix.original.as_ref(), "JOINT S GBR AUS USA");
    }

    #[test]
    fn e020_joint_does_not_fire_on_pure_alpha_list() {
        // `AUS GBR USA` is the pure-alpha canonical JOINT order.
        // E020 must stay silent even though USA is not first —
        // firing here would re-introduce the style-as-correctness
        // confusion the audit is correcting.
        let diags = lint_banner("//JOINT S AUS GBR USA");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E020"),
            "E020 must not fire on pure-alpha JOINT even when USA is \
             last (style guidance is a separate follow-up rule): {diags:?}"
        );
    }

    #[test]
    fn e020_citations_have_no_stray_whitespace() {
        // Guard against citation strings accidentally embedding
        // multiple consecutive spaces — the previous impl used
        // `\<newline>` line continuations with indented continuations.
        // Rust normally strips those, but `concat!` is explicit and
        // immune to any edge-case drift. This test fails loud if
        // future editors reintroduce the pattern.
        let rel_to_diags = lint_banner("SECRET//REL TO USA, GBR, AUS");
        let rel_to: Vec<_> = rel_to_diags
            .iter()
            .filter(|d| d.rule.as_str() == "E020")
            .collect();
        assert_eq!(rel_to.len(), 1);
        assert!(
            !rel_to[0].citation.contains("  "),
            "REL TO citation must not contain double spaces; got: {:?}",
            rel_to[0].citation
        );

        let joint_diags = lint_banner("//JOINT S USA GBR AUS");
        let joint: Vec<_> = joint_diags
            .iter()
            .filter(|d| d.rule.as_str() == "E020" && d.message.contains("JOINT"))
            .collect();
        assert_eq!(joint.len(), 1);
        assert!(
            !joint[0].citation.contains("  "),
            "JOINT citation must not contain double spaces; got: {:?}",
            joint[0].citation
        );
    }

    #[test]
    fn e020_rel_to_cites_section_h8() {
        // T035c-18: REL TO's ordering rule is authoritatively in
        // §H.8 p150-151 line 3714. Previously cited as bare `§H.8`.
        // Lock the tightened pointer so a regression to a whole-section
        // citation fails here rather than silently drifting.
        let diags = lint_banner("SECRET//REL TO USA, GBR, AUS");
        let e020: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E020").collect();
        assert_eq!(e020.len(), 1);
        assert!(
            e020[0].citation.contains("§H.8 p150-151 line 3714"),
            "REL TO citation must pin §H.8 p150-151 line 3714; got: {:?}",
            e020[0].citation
        );
    }

    #[test]
    fn e020_joint_cites_section_h3_not_h8() {
        // T035c-18: JOINT's ordering rule is in §H.3 (its own template),
        // NOT §H.8 (REL TO's template). Previously both paths cited
        // bare `§H.8`, which was source-incorrect for JOINT. Lock that
        // JOINT now cites its own section.
        let diags = lint_banner("//JOINT S USA GBR AUS//REL TO USA, AUS, GBR");
        let e020_joint: Vec<_> = diags
            .iter()
            .filter(|d| d.rule.as_str() == "E020" && d.message.contains("JOINT"))
            .collect();
        assert_eq!(e020_joint.len(), 1);
        assert!(
            e020_joint[0].citation.contains("§H.3 p56 line 1258"),
            "JOINT citation must pin §H.3 p56 line 1258; got: {:?}",
            e020_joint[0].citation
        );
        assert!(
            !e020_joint[0].citation.contains("§H.8"),
            "JOINT citation must NOT reference §H.8 (REL TO template); got: {:?}",
            e020_joint[0].citation
        );
    }

    #[test]
    fn e020_multi_block_suppression_cites_section_h8() {
        // The multi-block no-fix path builds its diagnostic directly
        // rather than going through `check_trigraph_ordering`, so it
        // has a separate citation-emission site that must also carry
        // the tightened §H.8 p150-151 line 3714 pointer.
        let diags = lint_banner("SECRET//REL TO USA, GBR//NF//REL TO AUS");
        let e020: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E020").collect();
        assert_eq!(e020.len(), 1);
        assert!(
            e020[0].citation.contains("§H.8 p150-151 line 3714"),
            "multi-block E020 citation must pin §H.8 p150-151 line 3714; got: {:?}",
            e020[0].citation
        );
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

    // --- E022: CNWDI only with TS or S RD ---

    #[test]
    fn e022_fires_on_cnwdi_with_confidential() {
        let diags = lint_banner("CONFIDENTIAL//RD-CNWDI//NOFORN");
        let e022: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E022").collect();
        assert_eq!(e022.len(), 1);
    }

    #[test]
    fn e022_does_not_fire_on_cnwdi_with_secret() {
        let diags = lint_banner("SECRET//RD-CNWDI//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E022"),
            "E022 should not fire with SECRET: {diags:?}"
        );
    }

    #[test]
    fn e022_does_not_fire_on_cnwdi_with_top_secret() {
        let diags = lint_banner("TOP SECRET//RD-CNWDI//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E022"),
            "E022 should not fire with TOP SECRET: {diags:?}"
        );
    }

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

    // --- E025: UCNI only with UNCLASSIFIED ---

    #[test]
    fn e025_fires_on_ucni_with_secret() {
        let diags = lint_banner("SECRET//DOD UCNI");
        let e025: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E025").collect();
        assert_eq!(e025.len(), 1);
    }

    #[test]
    fn e025_does_not_fire_on_ucni_with_unclassified() {
        let diags = lint_banner("UNCLASSIFIED//DOD UCNI");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E025"),
            "E025 should not fire with UNCLASSIFIED: {diags:?}"
        );
    }

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

    // --- Spec 003 SCI compartments: E011 structural regression ---

    #[test]
    fn e011_not_triggered_by_structural_sci_blocks() {
        // Regression: structural SCI parsing must not accidentally route
        // anything through MissingNonUsPrefix. A plain US banner with an
        // SCI compound must produce zero E011 diagnostics.
        let diags = lint_banner("SECRET//SI-G ABCD DEFG//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E011"),
            "E011 must not fire on a normal US banner with structural SCI: {diags:?}"
        );
    }

    // --- E032: SCI system order ---

    #[test]
    fn e032_fires_on_numeric_after_alpha() {
        // `SI` (alpha) listed before `123` (numeric) violates §A.6 p15.
        let diags = lint_banner("TOP SECRET//SI/123//NOFORN");
        let e032: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E032").collect();
        assert_eq!(
            e032.len(),
            1,
            "E032 must fire on SI/123 ordering: {diags:?}"
        );
        let fix = e032[0].fix.as_ref().expect("E032 must carry a FixProposal");
        assert!((fix.confidence.combined() - 0.85).abs() < f32::EPSILON);
        // Reorder puts numeric first.
        assert_eq!(fix.replacement.as_ref(), "123/SI");
    }

    #[test]
    fn e032_does_not_fire_on_correct_numeric_alpha_order() {
        let diags = lint_banner("TOP SECRET//123/SI//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E032"),
            "E032 must not fire on 123/SI: {diags:?}"
        );
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

    // --- E026: sar-portion-form ---

    #[test]
    fn e026_fires_on_full_form_in_portion() {
        let diags = lint_portion("(TS//SPECIAL ACCESS REQUIRED-BUTTER POPCORN//NF)");
        let e026: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E026").collect();
        assert_eq!(
            e026.len(),
            1,
            "E026 must fire on full form in portion: {diags:?}"
        );
        assert!(e026[0].fix.is_none(), "E026 does not propose a fix");
    }

    #[test]
    fn e026_does_not_fire_on_abbrev_in_portion() {
        let diags = lint_portion("(TS//SAR-BP//NF)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E026"),
            "E026 must not fire on SAR- abbrev portion: {diags:?}"
        );
    }

    #[test]
    fn e026_does_not_fire_on_full_form_in_banner() {
        // Banner lines may use the full form per §H.5 p101.
        let diags = lint_banner("TOP SECRET//SPECIAL ACCESS REQUIRED-BUTTER POPCORN//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E026"),
            "E026 is portion-only: {diags:?}"
        );
    }

    // --- E027: sar-classification ---

    #[test]
    fn e027_fires_on_unclassified_banner_with_sar() {
        let diags = lint_banner("UNCLASSIFIED//SAR-BP");
        let e027: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E027").collect();
        assert_eq!(e027.len(), 1, "E027 must fire on U//SAR-*: {diags:?}");
        assert!(e027[0].fix.is_none(), "E027 requires human review, no fix");
    }

    #[test]
    fn e027_does_not_fire_on_secret_with_sar() {
        let diags = lint_banner("SECRET//SAR-BP//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E027"),
            "E027 must not fire on SECRET//SAR-*: {diags:?}"
        );
    }

    #[test]
    fn e027_does_not_fire_on_top_secret_with_sar() {
        let diags = lint_banner("TOP SECRET//SAR-BP//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E027"),
            "E027 must not fire on TS//SAR-*: {diags:?}"
        );
    }

    // --- E028: sar-program-order ---

    #[test]
    fn e028_fires_on_out_of_order_programs() {
        let diags = lint_banner("SECRET//SAR-CD/BP//NOFORN");
        let e028: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E028").collect();
        assert_eq!(e028.len(), 1, "E028 must fire on CD/BP: {diags:?}");
        let fix = e028[0].fix.as_ref().expect("E028 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "SAR-BP/CD");
        assert!((fix.confidence.combined() - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn e028_fix_also_sorts_compartments_and_subs() {
        // Programs out of order AND compartments out of order.  E028's fix
        // must normalize both so that when the engine drops E029 (overlap
        // guard), the block is fully normalized in one pass.
        let diags = lint_banner("SECRET//SAR-CD-K15-J12/BP//NOFORN");
        let e028: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E028").collect();
        assert_eq!(e028.len(), 1, "E028 must fire: {diags:?}");
        let fix = e028[0].fix.as_ref().expect("E028 must carry a FixProposal");
        // Programs sorted (BP before CD), compartments sorted (J12 before K15).
        assert_eq!(fix.replacement.as_ref(), "SAR-BP/CD-J12-K15");
    }

    #[test]
    fn e028_does_not_fire_on_sorted_programs() {
        let diags = lint_banner("SECRET//SAR-BP/CD//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E028"),
            "E028 must not fire on BP/CD (sorted): {diags:?}"
        );
    }

    #[test]
    fn e032_does_not_fire_on_single_system() {
        let diags = lint_banner("TOP SECRET//SI//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E032"),
            "E032 must not fire with a single SCI system: {diags:?}"
        );
    }

    // --- E033: SCI compartment / sub-compartment order ---

    #[test]
    fn e033_fires_on_sub_compartment_disorder() {
        // Sub-compartments DEFG ABCD are out of alpha order within SI-G.
        // One diagnostic per out-of-order marking; fix covers the whole
        // compartment+sub-compartment region of that marking (matches
        // SAR E029 shape).
        let diags = lint_banner("SECRET//SI-G DEFG ABCD//NOFORN");
        let e033: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E033").collect();
        assert_eq!(
            e033.len(),
            1,
            "E033 must fire once on the out-of-order marking: {diags:?}"
        );
        let fix = e033[0].fix.as_ref().expect("E033 must carry a FixProposal");
        assert!((fix.confidence.combined() - 0.85).abs() < f32::EPSILON);
        assert_eq!(fix.replacement.as_ref(), "G ABCD DEFG");
    }

    #[test]
    fn e033_fix_sorts_comp_and_sub_levels_in_one_pass() {
        // Compartments AND sub-compartments both out of order in the
        // same marking. A single E033 diagnostic must carry a fix that
        // normalizes both levels — no second diagnostic, no overlap.
        let diags = lint_banner("SECRET//SI-NK-G DEFG ABCD//NOFORN");
        let e033: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E033").collect();
        assert_eq!(e033.len(), 1, "E033 must fire once: {diags:?}");
        let fix = e033[0].fix.as_ref().expect("E033 must carry a FixProposal");
        // Parse: SI has compartments NK (no subs) and G (subs DEFG ABCD).
        // Sort compartments: G < NK. Sort subs of G: ABCD < DEFG.
        // NK had no subs; it trails.
        assert_eq!(fix.replacement.as_ref(), "G ABCD DEFG-NK");
    }

    #[test]
    fn e032_fix_also_sorts_compartments_and_subs() {
        // Systems out of order (SI/123 — numeric should come first)
        // AND compartments out of order within SI. Applying E032's
        // whole-block fix alone must produce a fully-normalized block
        // so the engine's overlap guard can safely drop E033.
        let diags = lint_banner("SECRET//SI-NK-G DEFG ABCD/123//NOFORN");
        let e032: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E032").collect();
        assert_eq!(e032.len(), 1, "E032 must fire: {diags:?}");
        let fix = e032[0].fix.as_ref().expect("E032 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "123/SI-G ABCD DEFG-NK");
    }

    #[test]
    fn e033_does_not_fire_on_sorted_sub_compartments() {
        let diags = lint_banner("SECRET//SI-G ABCD DEFG//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E033"),
            "E033 must not fire on ABCD DEFG: {diags:?}"
        );
    }

    // --- E034: SCI custom control info ---

    #[test]
    fn e034_fires_on_custom_control_via_structural_path() {
        // `123/SI-G` routes through the structural subparser; the `123` head
        // creates a Custom-system SciMarking. E034 surfaces that for audit
        // visibility (severity Off by default, so the engine gates it).
        let diags = lint_banner("TOP SECRET//123/SI-G//NOFORN");
        let e034: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E034").collect();
        assert_eq!(
            e034.len(),
            1,
            "E034 must fire on custom control 123: {diags:?}"
        );
        assert!(e034[0].fix.is_none(), "E034 must not propose a fix");
        // T035c-2: E034 now defaults to Warn (was Off with a harness
        // workaround). Info is available as a config-opt-in.
        assert_eq!(e034[0].severity, marque_rules::Severity::Warn);
        assert!(e034[0].message.contains("unpublished SCI control system"));
    }

    #[test]
    fn e034_does_not_fire_on_published_only() {
        let diags = lint_banner("TOP SECRET//SI-G//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E034"),
            "E034 must not fire on SI-G alone: {diags:?}"
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

    #[test]
    fn e028_does_not_fire_on_single_program() {
        let diags = lint_banner("SECRET//SAR-BP//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E028"),
            "E028 must not fire on single program: {diags:?}"
        );
    }

    // --- E029: sar-compartment-order ---

    #[test]
    fn e029_fires_on_out_of_order_sub_compartments() {
        // Compartment J12 with sub-compartments [Z9, A3] — A3 should come
        // first (alpha-by-bytelex: A < Z).
        // E029 now emits per-program spans: the fix covers only the program
        // text (identifier + compartments), not the whole SAR block.
        let diags = lint_banner("SECRET//SAR-BP-J12 Z9 A3//NOFORN");
        let e029: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E029").collect();
        assert_eq!(
            e029.len(),
            1,
            "E029 must fire on subs [Z9, A3] (out of order): {diags:?}"
        );
        let fix = e029[0].fix.as_ref().expect("E029 must carry a FixProposal");
        // Per-program replacement: "PROG_ID-COMP SUB..." (no SAR- prefix).
        assert_eq!(fix.replacement.as_ref(), "BP-J12 A3 Z9");
        assert!(
            e029[0].message.contains("sub-compartments"),
            "E029 message should mention sub-compartments: {}",
            e029[0].message
        );
    }

    #[test]
    fn e029_fires_on_out_of_order_compartments() {
        // Compartments `K15-J12` — J before K (two compartments, so split by `-`).
        // BP has compartments [K15, J12] — out of order.
        // E029 now emits per-program spans: the fix covers only the program
        // text (identifier + compartments), not the whole SAR block.
        let diags = lint_banner("SECRET//SAR-BP-K15-J12//NOFORN");
        let e029: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E029").collect();
        assert_eq!(
            e029.len(),
            1,
            "E029 must fire on compartments K15 then J12: {diags:?}"
        );
        let fix = e029[0].fix.as_ref().unwrap();
        // Per-program replacement: "PROG_ID-COMP..." (no SAR- prefix).
        assert_eq!(fix.replacement.as_ref(), "BP-J12-K15");
    }

    #[test]
    fn e029_does_not_fire_on_sorted_sub_compartments() {
        let diags = lint_banner("SECRET//SAR-BP-J12 K15//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E029"),
            "E029 must not fire on J12 K15 (sorted): {diags:?}"
        );
    }

    // T035c-19 PR-A: per-rule citation lockdown for E026–E029. Each
    // rule's citation was previously the whole-section `"CAPCO-2016
    // §H.5"`; tightened to per-page/per-line pointers so a regression
    // to the whole-section form (or propagation to a wrong subsection)
    // fails re-verifiability per Constitution VIII. E029 has two
    // citation strings (compartments vs sub-compartments) keyed by
    // diagnostic level.

    #[test]
    fn e026_cites_portion_mark_line_2432() {
        let diags = lint_portion("(S//SPECIAL ACCESS REQUIRED-BP)");
        let e026: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E026").collect();
        assert_eq!(e026.len(), 1);
        assert!(
            e026[0].citation.contains("§H.5 p101 line 2432"),
            "E026 citation must pin §H.5 p101 line 2432 \
             (Authorized Portion Mark); got: {:?}",
            e026[0].citation
        );
    }

    #[test]
    fn e027_cites_relationship_line_2456() {
        let diags = lint_banner("UNCLASSIFIED//SAR-BP");
        let e027: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E027").collect();
        assert_eq!(e027.len(), 1);
        assert!(
            e027[0].citation.contains("§H.5 p101 line 2456"),
            "E027 citation must pin §H.5 p101 line 2456 \
             (Relationship(s) to Other Markings); got: {:?}",
            e027[0].citation
        );
    }

    #[test]
    fn e028_cites_program_ordering_line_2391() {
        let diags = lint_banner("SECRET//SAR-CD/BP//NOFORN");
        let e028: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E028").collect();
        assert_eq!(e028.len(), 1);
        assert!(
            e028[0].citation.contains("§H.5 p99 line 2391"),
            "E028 citation must pin §H.5 p99 line 2391; got: {:?}",
            e028[0].citation
        );
        assert!(
            !e028[0].citation.contains("§A.6"),
            "E028 citation must NOT reference §A.6 (that is SCI's \
             ordering authority, not SAR's); got: {:?}",
            e028[0].citation
        );
    }

    #[test]
    fn e029_compartment_arm_cites_line_2404() {
        // Compartments out of order (K15 before J12 within BP).
        let diags = lint_banner("SECRET//SAR-BP-K15-J12//NOFORN");
        let e029: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E029").collect();
        assert_eq!(e029.len(), 1);
        assert!(
            e029[0].message.contains("compartments"),
            "expected compartment-level message; got: {:?}",
            e029[0].message
        );
        assert!(
            e029[0].citation.contains("§H.5 p100 line 2404"),
            "E029 compartment arm must pin §H.5 p100 line 2404; got: {:?}",
            e029[0].citation
        );
    }

    #[test]
    fn e029_sub_compartment_arm_cites_line_2405() {
        // Sub-compartments out of order (K15 before J54 within J12).
        // Parser reads `BP-J12 K15 J54` as compartment J12 with
        // sub-compartments [K15, J54]; alphanumeric order requires
        // J54 before K15. The existing
        // `e029_fires_on_out_of_order_sub_compartments` test uses
        // this shape; replicate here so the citation lockdown is
        // self-contained.
        let diags = lint_banner("SECRET//SAR-BP-J12 K15 J54//NOFORN");
        let e029: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E029").collect();
        assert_eq!(e029.len(), 1);
        assert!(
            e029[0].message.contains("sub-compartments"),
            "expected sub-compartment-level message; got: {:?}",
            e029[0].message
        );
        assert!(
            e029[0].citation.contains("§H.5 p100 line 2405"),
            "E029 sub-compartment arm must pin §H.5 p100 line 2405; \
             got: {:?}",
            e029[0].citation
        );
    }

    // --- E030: sar-indicator-repeat ---

    #[test]
    fn e030_fires_on_repeated_abbrev_indicator() {
        let diags = lint_banner("SECRET//SAR-BP//SAR-CD//NOFORN");
        let e030: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E030").collect();
        assert_eq!(
            e030.len(),
            1,
            "E030 must fire on repeated SAR- indicator: {diags:?}"
        );
        let fix = e030[0].fix.as_ref().expect("E030 must carry a FixProposal");
        // The fix extends backward over `//` so the replacement is `/CD`.
        assert_eq!(fix.original.as_ref(), "//SAR-CD");
        assert_eq!(fix.replacement.as_ref(), "/CD");
        assert!((fix.confidence.combined() - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn e030_does_not_fire_on_single_sar_block() {
        let diags = lint_banner("SECRET//SAR-BP/CD//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E030"),
            "E030 must not fire when programs coalesce in one block: {diags:?}"
        );
    }

    #[test]
    fn e030_emits_no_fix_diagnostic_when_separator_has_whitespace_gap() {
        // Parser trims leading whitespace per block, so a source like
        // `// SAR-CD` leaves a byte gap between the Separator and the
        // Unknown SAR token. The earlier implementation silently
        // skipped emitting in that case, which combined with E008's
        // suppression dropped the repeated-SAR shape entirely. E030
        // must now emit a no-fix diagnostic so the user sees the
        // problem — a fix still cannot be honestly reconstructed
        // without the raw gap bytes.
        let src = "SECRET//SAR-BP// SAR-CD//NOFORN";
        let diags = lint_banner(src);
        let e030: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E030").collect();
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        assert_eq!(
            e030.len(),
            1,
            "E030 must still emit a diagnostic on repeated SAR even \
             when the separator has a whitespace gap — otherwise E008 \
             suppression silently drops it: {diags:?}"
        );
        assert!(
            e030[0].fix.is_none(),
            "E030 must NOT carry a fix when contiguity fails — \
             reconstructing `FixProposal.original` without the raw gap \
             bytes would corrupt the audit record: {e030:?}"
        );
        assert!(
            e008.is_empty(),
            "E008 must still step aside for the repeated-SAR shape \
             (the token matches the prefix + non-empty suffix + \
             has_first_sar predicate); E030 owns the diagnostic: \
             {diags:?}"
        );
    }

    #[test]
    fn e030_cites_line_2403_not_section_a6() {
        // T035c-19 PR-B: E030's authority is §H.5 p100 line 2403
        // (Syntax Rules bullet 5 — "must not be repeated"). An
        // earlier revision of the doc comment included "see also
        // §A.6" — §A.6 governs SCI ordering, not SAR, and
        // propagating that citation to the diagnostic would be a
        // §I/propagated-stale-citation hazard. This test locks that
        // the emitted citation pins the specific SAR authority and
        // does NOT reference §A.6.
        //
        // Exercises both emission paths: with-fix (contiguous span)
        // and no-fix (whitespace gap).
        for src in [
            "SECRET//SAR-BP//SAR-CD//NOFORN",       // contiguous, fix present
            "SECRET//SAR-BP// SAR-CD//NOFORN",      // whitespace gap, no fix
        ] {
            let diags = lint_banner(src);
            let e030: Vec<_> = diags
                .iter()
                .filter(|d| d.rule.as_str() == "E030")
                .collect();
            assert_eq!(e030.len(), 1, "E030 must fire once on {src:?}: {diags:?}");
            assert!(
                e030[0].citation.contains("§H.5 p100 line 2403"),
                "E030 citation must pin §H.5 p100 line 2403; got: {:?}",
                e030[0].citation
            );
            assert!(
                !e030[0].citation.contains("§A.6"),
                "E030 citation must NOT reference §A.6 (that is SCI's \
                 ordering authority, not SAR's); got: {:?}",
                e030[0].citation
            );
        }
    }

    // --- E031: sar-banner-rollup ---

    #[test]
    fn e031_fires_when_banner_missing_program_from_portion() {
        // Portions introduce SAR-BP and SAR-CD; banner only mentions BP.
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
        // Expected rolled-up form: programs sorted per CAPCO ascending order
        // (alpha: BP before CD).
        assert_eq!(fix.replacement.as_ref(), "SAR-BP/CD");
        assert!((fix.confidence.combined() - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn e031_does_not_fire_when_banner_omits_portion_compartment() {
        // T035c-19 PR-C: narrowed predicate. §H.5 p101 line 2460 and
        // §H.5 p99 line 2393 make banner hierarchy depth (below the
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
             line 2460 permits): {diags:?}"
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
        // T035c-19 PR-C: the minimum-change fix must preserve the
        // observed banner's hierarchy for programs that were already
        // present, and append missing programs as bare identifiers.
        // §H.5 p101 line 2460 makes hierarchy depiction the author's
        // choice — the fix honors that for existing programs.
        //
        // Portion: SAR-BP-J12 (BP with compartment J12) and SAR-CD.
        // Banner observed: SAR-BP-J12 (BP with compartment shown, CD missing).
        // Expected fix: SAR-BP-J12/CD (BP's J12 preserved, CD appended
        // bare — NOT SAR-BP-J12/CD-YYY or similar that would invent
        // hierarchy on CD).
        let source = "(S//SAR-BP-J12//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP-J12//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(e031.len(), 1, "E031 must fire on missing program CD: {diags:?}");
        let fix = e031[0].fix.as_ref().expect("E031 must have fix");
        assert_eq!(
            fix.replacement.as_ref(),
            "SAR-BP-J12/CD",
            "fix must preserve observed BP-J12 hierarchy and append bare CD"
        );
    }

    #[test]
    fn e031_cites_line_2458_and_hierarchy_optional_note() {
        // T035c-19 PR-C citation lockdown. E031's authority is:
        //   §H.5 p101 line 2458  — programs MUST roll up
        //   §H.5 p101 line 2460  — hierarchy MAY be omitted
        // The citation string must reference both so reviewers land
        // on the two passages that together define the narrowed
        // predicate.
        let source = "(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(e031.len(), 1);
        assert!(
            e031[0].citation.contains("§H.5 p101 line 2458"),
            "E031 citation must pin §H.5 p101 line 2458 (roll-up rule); \
             got: {:?}",
            e031[0].citation
        );
        assert!(
            e031[0].citation.contains("§H.5 p101 line 2460")
                || e031[0].citation.contains("line 2460"),
            "E031 citation must reference the hierarchy-optional carve-out \
             at §H.5 p101 line 2460; got: {:?}",
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
}

/// Internal test support module — drives the parser and rules directly,
/// without depending on the engine crate. This avoids a circular dependency
/// (`marque-capco` is below `marque-engine` in the workspace graph).
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod marque_capco_test_support {
    use super::CapcoRuleSet;
    use marque_core::{Parser, Scanner};
    use marque_ism::{CapcoTokenSet, MarkingType, PageContext};
    use marque_rules::{Diagnostic, RuleContext, RuleSet};
    use std::sync::Arc;

    fn run(source: &[u8]) -> Vec<Diagnostic> {
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
            if parsed.kind == MarkingType::Portion {
                page_context.add_portion(parsed.attrs.clone());
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
                out.extend(rule.check(&parsed.attrs, &ctx));
            }
        }
        out
    }

    pub fn lint_banner(s: &str) -> Vec<Diagnostic> {
        run(s.as_bytes())
    }

    pub fn lint_portion(s: &str) -> Vec<Diagnostic> {
        run(s.as_bytes())
    }
}
