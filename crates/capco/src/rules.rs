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
//!   E005 = declassification in banner (T034)
//!   E006 = deprecated dissem control (T035)
//!   E007 = X-shorthand declass date (T036)
//!   E008 = unrecognized token (T037)
//!   E009 = portion abbreviation
//!   E010 = bare HCS without compartment suffix
//!   E011 = missing leading // on non-US classification
//!   E012 = dual classification (US + foreign conflict)
//!   E013 = JOINT/REL TO delimiter mismatch
//!   E014 = JOINT participants missing from REL TO
//!   E015 = non-US classification without dissem control
//!   W001 = deprecated marking warning (T038)
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
                Box::new(MissingUsaTrigraphRule),
                Box::new(MisorderedBlocksRule),
                Box::new(SeparatorCountRule),
                Box::new(DeclassifyInBannerRule),
                Box::new(DeprecatedDissemRule),
                Box::new(XShorthandDateRule),
                Box::new(UnknownTokenRule),
                Box::new(DeprecatedMarkingWarningRule),
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

        // Build corrected list: USA first, then the rest in original order.
        let mut fixed_parts: Vec<&str> = vec!["USA"];
        for t in attrs.rel_to.iter() {
            if *t != marque_ism::Trigraph::USA {
                fixed_parts.push(t.as_str());
            }
        }
        let fixed = fixed_parts.join(", ");

        let message = if !has_usa {
            "REL TO list missing required USA trigraph"
        } else {
            "USA must be the first trigraph in REL TO list"
        };

        // Span: the first REL TO trigraph in the marking. This points the
        // user at the leading edge of the offending list.
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::RelToTrigraph)
            .map(|t| t.span)
            // Defensive: if there's no token span (shouldn't happen given
            // attrs.rel_to is non-empty), use a zero-length span which the
            // engine's fix path will filter rather than mis-splice.
            .unwrap_or(Span::new(0, 0));

        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: self.id(),
            severity: self.default_severity(),
            source: FixSource::BuiltinRule,
            span,
            message: message.to_owned(),
            citation: "CAPCO-2016 §H.8",
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
// Rule: E004 — Wrong separator count (should always be exactly `//`)
// ---------------------------------------------------------------------------

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
                    citation: "CAPCO-2016 §A.6",
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
                citation: "CAPCO-2016 §A.6",
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
// Rule: E005 — Declassification marking in banner (should be in CAB)
// ---------------------------------------------------------------------------

struct DeclassifyInBannerRule;

impl Rule for DeclassifyInBannerRule {
    fn id(&self) -> RuleId {
        RuleId::new("E005")
    }
    fn name(&self) -> &'static str {
        "declassify-in-banner"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;
        if ctx.marking_type != MarkingType::Banner {
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
            "declassification marking belongs in Classification Authority Block \
             (Declassify On:), not in the banner — remove from banner and add to CAB",
            "CAPCO-2016 §E",
            None, // Fix requires document-level context (multi-span);
                  // confidence 0.55 per T034 — suggestion only.
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

/// Returns `true` if `replacement` names a current dissemination control.
///
/// E006 uses this as a guard: the migration table can contain non-dissem
/// replacements (e.g., declass-shorthand entries like `25X1-` → `25X1`
/// which E007 owns), and those MUST NOT dispatch as E006.
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

// ---------------------------------------------------------------------------
// Rule: E008 — Unrecognized token inside marking
// ---------------------------------------------------------------------------

/// FR-012: any token inside a marking candidate boundary that the parser
/// could not classify is reported as an error with no fix offered.
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
        attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Unknown)
            // Skip entries that E006/E007/E030 will pick up. Three paths:
            //   1. Migration-table hit (covers LIMDIS/FOUO for E006 and
            //      25X1-/50X1- for E007).
            //   2. Pattern-matched X-shorthand with a trailing `-` for
            //      forms not in the seed table (25X2-, 25X9-, etc.).
            //   3. A second/subsequent SAR category block that the parser
            //      tagged Unknown precisely so E030 can flag the repeated
            //      indicator (§H.5 p100: the SAR indicator must not be
            //      repeated). E008 steps aside; E030 owns this shape.
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
                    && !text.starts_with("SAR-")
                    && !text.starts_with("SPECIAL ACCESS REQUIRED-")
            })
            .map(|t| {
                Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    t.span,
                    "unrecognized token inside marking — does not match any \
                     known CAPCO classification, control, or trigraph",
                    "CAPCO-2016 §G.1",
                    None, // FR-012: no fix offered
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Rule: W001 — Deprecated marking warning
// ---------------------------------------------------------------------------

/// W001 surfaces markings that are still legal but have a newer canonical
/// form. The seed migration table has no W001-flagged entries, so this rule
/// fires zero diagnostics in Phase 3 against real corpus content. Synthetic
/// entries can be injected through a custom `RuleSet` for tests; see
/// `tests/rules_us1.rs`.
struct DeprecatedMarkingWarningRule;

impl Rule for DeprecatedMarkingWarningRule {
    fn id(&self) -> RuleId {
        RuleId::new("W001")
    }
    fn name(&self) -> &'static str {
        "deprecated-marking-warning"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, _attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        // Phase 3: no W001-flagged migration entries exist yet. The rule is
        // wired so that adding a `is_warning_only: true` field to
        // MigrationEntry in a future build.rs change starts firing
        // diagnostics with no other code changes.
        vec![]
    }
}

// ---------------------------------------------------------------------------
// Rule: C001 — Corrections-map typo replacement
// ---------------------------------------------------------------------------

/// Scans token spans against the organization-specific corrections map from
/// `[corrections]` in `.marque.toml`. Each match produces a fix proposal with
/// `FixSource::CorrectionsMap` and `confidence = 1.0`.
///
/// FR-009: user corrections take precedence over built-in rules on the same
/// span. This is automatic under FR-016 sort order — `"C001" < "E001"`
/// lexicographically, so C001 wins under the C-1 overlap guard.
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
                citation: "CONFIG:[corrections]",
                original: text.to_owned(),
                replacement: replacement.clone(),
                confidence: 1.0,
                migration_ref: None,
            }));
        }
        diagnostics
    }
}

/// E009: Portion markings must use abbreviated forms, not banner-style expansions.
///
/// Mirror of E001: whereas E001 catches portion abbreviations in banners
/// (e.g., `NF` → `NOFORN`), E009 catches banner expansions in portions
/// (e.g., `NOFORN` → `NF`, `SECRET` → `S`).
///
/// The rule checks two token categories:
/// - **Classification**: banner form like "SECRET" should be "S"
/// - **Dissem controls**: banner form like "NOFORN" should be "NF"
///
/// Data sources:
/// - Classification: `Classification::banner_str()` / `portion_str()` (hand-written in marque-ism)
/// - Dissem controls: `contract_dissem_to_portion()` (inverse of E001's `expand_dissem_abbreviation`)
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
                        citation: "CAPCO-2016 §C.1",
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
                citation: "CAPCO-2016 §C.1",
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
                    citation: "CAPCO-2016 §C.1",
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

/// JOINT country lists are space-delimited, REL TO lists are comma-delimited.
/// A common error is using commas in JOINT or spaces in REL TO.
///
/// This rule detects:
/// - Commas in JOINT classification token text (`//JOINT S USA,GBR` → fix to space-delimited)
/// - Space-only delimiters in REL TO lists (`REL TO USA GBR` → fix to comma-delimited)
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

        // Check JOINT classification for comma-delimited countries.
        if let Some(MarkingClassification::Joint(_)) = &attrs.classification {
            if let Some(token) = attrs
                .token_spans
                .iter()
                .find(|t| t.kind == TokenKind::Classification)
            {
                let text = token.text.as_ref();
                if text.contains(',') {
                    // Strip "JOINT <level> " prefix to get the country part,
                    // then replace commas with spaces.
                    let fixed = text.replace(',', "").replace("  ", " ");
                    diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                        rule: self.id(),
                        severity: self.default_severity(),
                        source: FixSource::BuiltinRule,
                        span: token.span,
                        message: "JOINT country list must be space-delimited, not comma-delimited"
                            .to_owned(),
                        citation: "CAPCO-2016 §A.6",
                        original: text.to_owned(),
                        replacement: fixed,
                        confidence: 0.95,
                        migration_ref: None,
                    }));
                }
            }
        }

        // Check REL TO for space-only delimiters (commas required between trigraphs).
        if let Some(token) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::RelToBlock)
        {
            let text = token.text.as_ref();
            // Strip the "REL TO " / "REL " prefix to isolate the country list.
            let country_list = text
                .strip_prefix("REL TO")
                .or_else(|| text.strip_prefix("REL"))
                .unwrap_or(text)
                .trim_start();
            // Space-delimited error: multiple words, none of which are commas/comma-adjacent.
            if country_list.split_whitespace().count() > 1 && !country_list.contains(',') {
                // Build the correctly comma-delimited replacement.
                let fixed = format!(
                    "REL TO {}",
                    country_list
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::BuiltinRule,
                    span: token.span,
                    message: "REL TO country list must be comma-delimited, not space-delimited"
                        .to_owned(),
                    citation: "CAPCO-2016 §A.6",
                    original: text.to_owned(),
                    replacement: fixed,
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

/// Some non-IC dissemination controls should not appear in classified banners.
///
/// LIMDIS, LES, LES-NF, and SSI propagate to classified banners and are fine.
/// EXDIS, NODIS, SBU, and SBU-NF do NOT propagate — they belong only in
/// portion markings (or in UNCLASSIFIED banners).
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

/// Country/entity code lists (REL TO, JOINT, FGI) must be alphabetically
/// ordered after USA (which is always first when present). Trigraphs come
/// before tetragraphs, both groups sorted alphabetically.
///
/// This is a fixable error — the correct order can be computed with
/// complete confidence.
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
        // E002 handles that case and its fix implicitly corrects ordering.
        if attrs.rel_to.len() >= 2
            && attrs
                .rel_to
                .first()
                .is_some_and(|t| *t == marque_ism::Trigraph::USA)
        {
            if let Some(diag) = check_trigraph_ordering(
                &attrs.rel_to,
                "REL TO",
                self.id(),
                self.default_severity(),
                attrs,
            ) {
                diagnostics.push(diag);
            }
        }

        // Check JOINT country ordering.
        if let Some(MarkingClassification::Joint(j)) = &attrs.classification {
            if j.countries.len() >= 2 {
                if let Some(diag) = check_trigraph_ordering(
                    &j.countries,
                    "JOINT",
                    self.id(),
                    self.default_severity(),
                    attrs,
                ) {
                    diagnostics.push(diag);
                }
            }
        }

        diagnostics
    }
}

/// Check that a trigraph list is ordered: USA first (if present), then
/// remaining codes alphabetically.
fn check_trigraph_ordering(
    codes: &[marque_ism::Trigraph],
    list_name: &str,
    rule: RuleId,
    severity: Severity,
    attrs: &IsmAttributes,
) -> Option<Diagnostic> {
    // Build the expected sorted order: USA first, then rest alphabetical.
    let mut sorted: Vec<&str> = codes.iter().map(|t| t.as_str()).collect();
    let has_usa = sorted.contains(&"USA");

    // Remove USA, sort the rest, put USA back at front.
    sorted.retain(|s| *s != "USA");
    sorted.sort_unstable();
    if has_usa {
        sorted.insert(0, "USA");
    }

    let actual: Vec<&str> = codes.iter().map(|t| t.as_str()).collect();
    if actual == sorted {
        return None;
    }

    // Compute a span covering the entire country list (first → last trigraph).
    let kind = if list_name == "REL TO" {
        TokenKind::RelToTrigraph
    } else {
        TokenKind::Classification
    };
    let matching_spans: Vec<&TokenSpan> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == kind)
        .collect();
    let span = match (matching_spans.first(), matching_spans.last()) {
        (Some(first), Some(last)) => Span::new(first.span.start, last.span.end),
        _ => Span::new(0, 0),
    };

    // Separator for the list: REL TO uses ", "; JOINT/FGI use " ".
    let sep = if list_name == "REL TO" { ", " } else { " " };
    let original = actual.join(sep);
    let replacement = sorted.join(sep);

    Some(make_fix_diagnostic(FixDiagnosticParams {
        rule,
        severity,
        source: FixSource::BuiltinRule,
        span,
        message: format!(
            "{list_name} country codes must be alphabetically ordered \
             (USA first when present): [{original}] → [{replacement}]"
        ),
        citation: "CAPCO-2016 §H.8",
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
/// `SPECIAL ACCESS REQUIRED-` form (CAPCO-2016 §H.5 p101 "Authorized
/// Portion Mark"). When all program identifiers are already abbrev-shaped
/// (2–3 alphanumeric characters), a low-confidence (0.35) suggestion is
/// proposed to replace the full indicator with the `SAR-` prefix.
/// Otherwise no fix is proposed because abbreviating an arbitrary
/// program nickname requires human judgment.
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
            "CAPCO-2016 §H.5",
            fix,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E027 — SAR requires TS, S, or C classification
// ---------------------------------------------------------------------------

/// SAR markings may only be used with TOP SECRET, SECRET, or CONFIDENTIAL
/// classifications (CAPCO-2016 §H.5 p101 "Relationship(s) to Other
/// Markings"). `UNCLASSIFIED//SAR-*` is invalid and requires human review.
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
            "CAPCO-2016 §H.5",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E028 — SAR programs must be in ascending order
// ---------------------------------------------------------------------------

/// Programs within a SAR block must be listed in ascending sort order
/// with numbered values first, followed by alphabetic values (CAPCO-2016
/// §H.5 p99).
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
            citation: "CAPCO-2016 §H.5",
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
/// compartment — must be in ascending sort order per CAPCO-2016 §H.5 p99.
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

            let level = if !comps_ok {
                "compartments"
            } else {
                "sub-compartments"
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
                citation: "CAPCO-2016 §H.5",
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
/// separator (CAPCO-2016 §H.5 p100 Syntax Rules bullet 5; see also §A.6).
///
/// The parser captures the first SAR block into `attrs.sar_markings` and
/// emits every subsequent same-marking SAR block as an `Unknown` token
/// whose text still starts with `SAR-` or `SPECIAL ACCESS REQUIRED-`.
/// This rule finds those Unknown tokens, extends the fix span backward
/// over the preceding `//` category separator, and coalesces the
/// repeated block into the preceding block.
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
            // Find the closest preceding Separator token. The parser
            // trims leading whitespace per block, so the token's own
            // span does not necessarily sit flush against the `//`.
            let Some(sep_tok) = attrs.token_spans[..idx]
                .iter()
                .rev()
                .find(|t| t.kind == TokenKind::Separator)
            else {
                // No preceding separator — shouldn't happen for a valid
                // SAR-prefixed Unknown token, but skip defensively.
                continue;
            };
            // Only emit a fix when the separator and the Unknown token are
            // byte-contiguous (no whitespace gap between them). If there is
            // a gap we cannot honestly reconstruct the original bytes in
            // `FixProposal.original` without preserving the raw source, so
            // we skip to avoid fabricating `original` content.
            if sep_tok.span.end != tok.span.start {
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
                message: "SAR category indicator must not be repeated; \
                     multiple programs use a single indicator with '/' separator"
                    .to_owned(),
                citation: "CAPCO-2016 §H.5",
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
// Rule: E031 — SAR banner roll-up
// ---------------------------------------------------------------------------

/// Per CAPCO-2016 §H.5 p101 Precedence Rules for Banner Line Guidance:
/// "Unique SAPs contained in portion marks must always appear in the banner
/// line." The banner's SAR block must therefore contain every SAR program,
/// compartment, and sub-compartment present in any portion marking on the
/// page.
///
/// This rule consumes [`PageContext::expected_sar_marking`] (P4a) to compute
/// the required composite and compares it with the banner's observed SAR
/// block. Any program / compartment / sub-compartment present in the
/// expected set but absent from the observed banner is flagged.
///
/// Fix semantics:
/// - If the banner has a SAR block, replace it in-place with the rolled-up
///   form at confidence 0.9 (severity `Fix`).
/// - If the banner has no SAR block at all, emit at severity `Error` with
///   no fix — inserting a new block requires byte-positioning between the
///   SCI and AEA blocks, which the engine's single-pass architecture does
///   not reliably support from rule-level information alone.
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

        // Compute the missing set against the observed banner.
        let missing = sar_missing_identifiers(attrs.sar_markings.as_ref(), &expected);
        if missing.is_empty() {
            return vec![];
        }

        let message = format!(
            "banner SAR block is missing programs/compartments present in portions: {}",
            missing.join(", "),
        );

        match attrs.sar_markings.as_ref() {
            Some(observed) => {
                // Banner has a SAR block — replace it in place with the
                // rolled-up form. Preserve the observed indicator form
                // (abbreviated vs full) so we don't gratuitously rewrite
                // `SPECIAL ACCESS REQUIRED-` into `SAR-`.
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
                let replacement = render_sar_block(observed.indicator, &expected.programs);
                if replacement == original_bytes {
                    // Indicator-form-only difference, missing set was
                    // computed from identifiers; shouldn't happen, but be
                    // defensive.
                    return vec![];
                }
                vec![make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::BuiltinRule,
                    span,
                    message,
                    citation: "CAPCO-2016 §H.5",
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
                    "CAPCO-2016 §H.5",
                    None,
                )]
            }
        }
    }
}

/// Collect identifiers that appear in `expected` but not in `observed`.
///
/// Returns a flat human-readable list in `program` / `program-comp` /
/// `program-comp sub` form so the diagnostic message is actionable.
fn sar_missing_identifiers(
    observed: Option<&marque_ism::SarMarking>,
    expected: &marque_ism::SarMarking,
) -> Vec<String> {
    use std::collections::HashMap;

    // Build a lookup into observed: program_id -> (comp_id -> set of sub ids).
    let mut obs_map: HashMap<&str, HashMap<&str, std::collections::HashSet<&str>>> = HashMap::new();
    if let Some(obs) = observed {
        for prog in obs.programs.iter() {
            let comps = obs_map.entry(prog.identifier.as_ref()).or_default();
            for comp in prog.compartments.iter() {
                let subs = comps.entry(comp.identifier.as_ref()).or_default();
                for sub in comp.sub_compartments.iter() {
                    subs.insert(sub.as_ref());
                }
            }
        }
    }

    let mut missing: Vec<String> = Vec::new();
    for prog in expected.programs.iter() {
        match obs_map.get(prog.identifier.as_ref()) {
            None => {
                // Entire program missing — report it (plus its compartments
                // inline so the reader sees the full shape).
                let rendered = render_single_program(prog);
                missing.push(rendered);
            }
            Some(obs_comps) => {
                for comp in prog.compartments.iter() {
                    match obs_comps.get(comp.identifier.as_ref()) {
                        None => {
                            let mut s = format!("{}-{}", prog.identifier, comp.identifier);
                            for sub in comp.sub_compartments.iter() {
                                s.push(' ');
                                s.push_str(sub);
                            }
                            missing.push(s);
                        }
                        Some(obs_subs) => {
                            for sub in comp.sub_compartments.iter() {
                                if !obs_subs.contains(sub.as_ref()) {
                                    missing.push(format!(
                                        "{}-{} {}",
                                        prog.identifier, comp.identifier, sub,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    missing
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
/// exists purely for audit visibility: it surfaces each Custom control
/// identifier so a classifier can verify the allocation is registered.
///
/// Severity is `Off` by default (the `Severity` enum has no Info variant
/// and the FR-008 invariant makes Off non-firing in the engine). Sites
/// that want the audit trail must opt in via `.marque.toml` by setting
/// `sci-custom-control-info = "warn"`. No fix is offered.
struct SciCustomControlInfoRule;

impl Rule for SciCustomControlInfoRule {
    fn id(&self) -> RuleId {
        RuleId::new("E034")
    }
    fn name(&self) -> &'static str {
        "sci-custom-control-info"
    }
    fn default_severity(&self) -> Severity {
        // Shipped as Off pending an Info severity; users opt in via config.
        // The test harness bypasses engine-level severity filtering so unit
        // tests can still assert this rule fires on matching input.
        Severity::Off
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
        assert!(ids.contains(&"E010"));
        assert!(ids.contains(&"E011"));
        assert!(ids.contains(&"E012"));
        assert!(ids.contains(&"E013"));
        assert!(ids.contains(&"E014"));
        assert!(ids.contains(&"E015"));
        assert!(ids.contains(&"W001"));
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
        // Net count: 39 - 3 + 1 = 37.
        assert_eq!(set.rules().len(), 37);
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
        let diags = lint_banner("SECRET//REL TO GBR, AUS");
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        // Span points at the first trigraph in the list.
        let src = b"SECRET//REL TO GBR, AUS";
        assert_eq!(e002[0].span.as_str(src).unwrap(), "GBR");
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

    #[test]
    fn e005_fires_on_declass_exemption_in_banner() {
        let diags = lint_banner("SECRET//25X1//NOFORN");
        let e005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E005").collect();
        assert_eq!(e005.len(), 1);
        let src = b"SECRET//25X1//NOFORN";
        assert_eq!(e005[0].span.as_str(src).unwrap(), "25X1");
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
        assert_eq!(e009.len(), 1);
        let src = b"(SECRET//NF)";
        assert_eq!(e009[0].span.as_str(src).unwrap(), "SECRET");
        let fix = e009[0].fix.as_ref().expect("E009 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "S");
    }

    #[test]
    fn e009_fires_on_banner_form_dissem_in_portion() {
        let diags = lint_portion("(S//NOFORN)");
        let e009: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E009").collect();
        assert_eq!(e009.len(), 1);
        let src = b"(S//NOFORN)";
        assert_eq!(e009[0].span.as_str(src).unwrap(), "NOFORN");
        let fix = e009[0].fix.as_ref().expect("E009 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "NF");
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
    fn w003_does_not_fire_on_limdis_in_classified_banner() {
        // LIMDIS (NGA Title 10) propagates to classified banners.
        let diags = lint_banner("SECRET//LIMDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "LIMDIS propagates to classified banners: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_les_in_classified_banner() {
        // LES propagates to classified banners.
        let diags = lint_banner("SECRET//LES");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "LES propagates to classified banners: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_ssi_in_classified_banner() {
        // SSI propagates to classified banners.
        let diags = lint_banner("SECRET//SSI");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "SSI propagates to classified banners: {diags:?}"
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
        assert_eq!(e034[0].severity, marque_rules::Severity::Off);
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
    fn e031_fires_when_banner_missing_compartment_from_portion() {
        // Portion has SAR-BP-J12; banner has only SAR-BP.
        let source = "(S//SAR-BP-J12//NF)\nSECRET//SAR-BP//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(
            e031.len(),
            1,
            "E031 must fire when banner omits compartment J12: {diags:?}"
        );
        assert!(
            e031[0].message.contains("J12"),
            "message must name missing compartment: {}",
            e031[0].message
        );
        let fix = e031[0].fix.as_ref().expect("fix expected");
        assert_eq!(fix.replacement.as_ref(), "SAR-BP-J12");
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
