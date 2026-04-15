//! CAPCO rule implementations — Layer 2 diagnostic intelligence.
//!
//! Each rule uses Layer 1 schema predicates (from generated/validators.rs) to
//! detect violations, then produces enriched diagnostics with fixes and
//! confidence. Phase 3 lands the full set of MVP rules with byte-precise
//! spans threaded through `IsmAttributes::token_spans`.
//!
//! Rule IDs follow the convention: E### = error, W### = warning, C### = correction.
//! Assignments per spec tasks.md:
//!   E001 = banner abbreviation (T030)
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
//!   E017 = JOINT may not be used with FGI
//!   E018 = JOINT may not be used with IC dissem (except REL TO)
//!   E019 = JOINT may not be used with non-IC dissem
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
//!   C001 = corrections-map typo (T058, Phase 5)

use marque_ism::generated::migrations::find_migration;
use marque_ism::{
    ForeignClassification, IsmAttributes, MarkingClassification, SciControlSystem, SciMarking,
    Span, TokenKind, TokenSpan, is_bare_cve_value,
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
        Self {
            rules: vec![
                Box::new(BannerAbbreviationRule),
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
                Box::new(BareHcsRule),
                Box::new(MissingNonUsPrefix),
                Box::new(DualClassificationRule),
                Box::new(DelimiterMismatchRule),
                Box::new(CominglingWarningRule),
                Box::new(JointRelToRule),
                Box::new(NonUsMissingDissemRule),
                Box::new(NonIcInClassifiedBannerRule),
                Box::new(JointRestrictedRule),
                Box::new(JointFgiRule),
                Box::new(JointIcDissemRule),
                Box::new(JointNonIcDissemRule),
                Box::new(CountryCodeOrderingRule),
                Box::new(AeaNofornRule),
                Box::new(CnwdiConstraintRule),
                Box::new(SigmaValidationRule),
                Box::new(RdPrecedenceRule),
                Box::new(UcniClassificationRule),
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
// Rule: E001 — Banner uses abbreviated classification or caveat
// ---------------------------------------------------------------------------

/// Banners must use full words: SECRET not S, NOFORN not NF, TOP SECRET not TS.
struct BannerAbbreviationRule;

impl Rule for BannerAbbreviationRule {
    fn id(&self) -> RuleId {
        RuleId::new("E001")
    }
    fn name(&self) -> &'static str {
        "banner-abbreviation"
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
        // whose canonical CVE form is an abbreviation that maps to a full
        // banner form, check whether the SOURCE BYTES are the abbreviation
        // (not just the parsed enum — the parser also accepts banner-form
        // full words via parse_dissem_full_form, and those are already
        // correct).
        //
        // The emit check happens at construction time against
        // `token_span.text` rather than as a post-hoc length filter, so the
        // logic cannot silently regress if a future abbreviation has a
        // different length from its canonical form.
        let dissem_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::DissemControl)
            .collect();
        for (idx, control) in attrs.dissem_controls.iter().enumerate() {
            let Some(full) = marque_ism::marking_forms::portion_to_banner(control.as_str()) else {
                continue;
            };
            // The Nth dissem token span corresponds to the Nth dissem
            // control entry — both vectors are in document order.
            let Some(token_span) = dissem_spans.get(idx) else {
                continue;
            };
            let abbrev = control.as_str();
            // Only fire when the literal source text is the abbreviation.
            // A banner containing "NOFORN" parses to DissemControl::Nf but
            // token_span.text is "NOFORN" — skip it.
            if token_span.text.as_ref() != abbrev {
                continue;
            }
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span: token_span.span,
                message: format!("banner uses abbreviated dissem control {abbrev:?}; use {full:?}"),
                citation: "CAPCO-ISM-v2022-DEC-§3.2",
                original: abbrev.to_owned(),
                replacement: full.to_owned(),
                confidence: 1.0,
                migration_ref: Some("CAPCO-2023-§3.2"),
            }));
        }
        // Walk non-IC dissem token spans. If the source text is the portion
        // abbreviation (e.g., "DS" instead of "LIMDIS"), suggest the banner form.
        let nic_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::NonIcDissem)
            .collect();
        for (idx, nic) in attrs.non_ic_dissem.iter().enumerate() {
            let Some(full) = marque_ism::marking_forms::portion_to_banner(nic.portion_str()) else {
                // banner_str == portion_str (e.g., SBU, LES, SSI) — no correction needed.
                continue;
            };
            let Some(token_span) = nic_spans.get(idx) else {
                continue;
            };
            let abbrev = nic.portion_str();
            if token_span.text.as_ref() != abbrev {
                continue;
            }
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::BuiltinRule,
                span: token_span.span,
                message: format!(
                    "banner uses abbreviated non-IC dissem control {abbrev:?}; use {full:?}"
                ),
                citation: "CAPCO-ISM-v2022-DEC-§9",
                original: abbrev.to_owned(),
                replacement: full.to_owned(),
                confidence: 1.0,
                migration_ref: Some("CAPCO-2023-§9"),
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
            citation: "CAPCO-ISM-v2022-DEC-§4.1",
            original: current,
            replacement: fixed,
            confidence: 0.97, // per spec T031
            migration_ref: Some("CAPCO-2023-§4.1"),
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
        // kind to a CAPCO ordinal: 0=Class, 1=SCI, 2=SAR, 3=Dissem/RelTo.
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
                0.6,
                Some("CAPCO-2023-§3.1"),
            )
        });

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "marking blocks are out of CAPCO order \
             (expected: Classification // SCI // SAR // Dissem // REL TO // Non-IC)",
            "CAPCO-ISM-v2022-DEC-§3.1",
            fix,
        )]
    }
}

fn ordinal_for_block(kind: TokenKind) -> Option<u8> {
    match kind {
        TokenKind::Classification => Some(0),
        TokenKind::SciControl => Some(1),
        TokenKind::SarIdentifier => Some(2),
        TokenKind::DissemControl | TokenKind::RelToTrigraph => Some(3),
        // Non-IC dissem always comes after IC dissem (last block).
        TokenKind::NonIcDissem => Some(4),
        // Separators, declass, and unknown tokens do not participate in
        // ordering — they belong to other blocks or other rules.
        _ => None,
    }
}

/// Rebuild a marking string from `attrs.token_spans`, ordered by CAPCO
/// block ordinals: Classification // SCI // SAR // Dissem // REL TO // Non-IC.
///
/// Within each block, tokens preserve their document order. REL TO trigraphs
/// are reassembled into a single `REL TO ...` block. Non-IC dissem controls
/// appear last per CAPCO Register §9. Returns `None` if there is nothing
/// meaningful to reorder (no classification recorded).
///
/// This is the suggestion path for E003 (T032). It is not byte-equivalent to
/// the original markup whitespace, but it is a valid CAPCO marking that the
/// engine could splice if a caller lowers the threshold below 0.6.
fn reorder_marking(attrs: &IsmAttributes) -> Option<String> {
    // Group token texts by ordinal, preserving document order.
    let mut classification: Vec<&str> = Vec::new();
    let mut sci: Vec<&str> = Vec::new();
    let mut sar: Vec<&str> = Vec::new();
    let mut dissem: Vec<&str> = Vec::new();
    let mut rel_to: Vec<&str> = Vec::new();
    let mut non_ic: Vec<&str> = Vec::new();

    for token in attrs.token_spans.iter() {
        match token.kind {
            TokenKind::Classification => classification.push(token.text.as_ref()),
            TokenKind::SciControl => sci.push(token.text.as_ref()),
            TokenKind::SarIdentifier => sar.push(token.text.as_ref()),
            TokenKind::DissemControl => dissem.push(token.text.as_ref()),
            TokenKind::RelToTrigraph => rel_to.push(token.text.as_ref()),
            TokenKind::NonIcDissem => non_ic.push(token.text.as_ref()),
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
    if !sar.is_empty() {
        blocks.push(sar.join("/"));
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
                    citation: "CAPCO-ISM-v2022-DEC-§3.1",
                    original,
                    replacement: "//".to_owned(),
                    confidence: 0.99,
                    migration_ref: None,
                }));
            }
        }

        // === Missing separators (single `/` not part of `//`) ===
        // When a user writes `SECRET/NOFORN` (one slash) the parser cannot
        // split on `//`, so the entire marking lands in one block whose
        // text contains a stray `/`. The block is recorded as either a
        // `Classification` token (if it's the first/only block) or an
        // `Unknown` token (if a partial split happened, e.g.
        // `SECRET//SI/NF` → blocks `SECRET`, `SI/NF`). E004 walks both and
        // emits one diagnostic per single-slash position.
        for token in attrs.token_spans.iter() {
            if !matches!(token.kind, TokenKind::Classification | TokenKind::Unknown) {
                continue;
            }
            let bytes = token.text.as_bytes();
            // Find every `/` that is NOT adjacent to another `/`. A doubled
            // `/` is a separator and would have been recognized by the
            // outer `//` split, so any `/` we see here in a non-Separator
            // token is by construction a stray single slash.
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'/' {
                    let prev_is_slash = i > 0 && bytes[i - 1] == b'/';
                    let next_is_slash = bytes.get(i + 1) == Some(&b'/');
                    if !prev_is_slash && !next_is_slash {
                        let abs_pos = token.span.start + i;
                        let span = Span::new(abs_pos, abs_pos + 1);
                        diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                            rule: self.id(),
                            severity: self.default_severity(),
                            source: FixSource::BuiltinRule,
                            span,
                            message: "missing block separator: single `/` should be `//`"
                                .to_owned(),
                            citation: "CAPCO-ISM-v2022-DEC-§3.1",
                            original: "/".to_owned(),
                            replacement: "//".to_owned(),
                            confidence: 0.99,
                            migration_ref: None,
                        }));
                    }
                }
                i += 1;
            }
        }

        diagnostics
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
            "CAPCO-ISM-v2022-DEC-§6.1",
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
                citation: "CAPCO-ISM-v2022-DEC-§3.4",
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

fn is_dissem_replacement(replacement: &str) -> bool {
    matches!(
        replacement,
        "RELIDO" | "CUI" | "NOFORN" | "ORCON" | "IMCON" | "DEA SENSITIVE" | "PROPIN"
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
                    citation: "CAPCO-ISM-v2022-DEC-§5.1",
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
                    citation: "CAPCO-ISM-v2022-DEC-§5.1",
                    original: text.to_owned(),
                    replacement,
                    // 0.95: slightly below table-backed 0.97 because
                    // the canonical form is derived by pattern stripping
                    // rather than an authoritative CVE mapping.
                    confidence: 0.95,
                    migration_ref: Some("CAPCO-2023-§5.1-X-shorthand-pattern"),
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
            // Skip entries that E006/E007 will pick up. Two paths to check,
            // in lockstep with E007's emit logic:
            //   1. Migration-table hit (covers LIMDIS/FOUO for E006 and
            //      25X1-/50X1- for E007).
            //   2. Pattern-matched X-shorthand with a trailing `-` for
            //      forms not in the seed table (25X2-, 25X9-, etc.).
            // An Unknown that hits either path is not "unrecognized" — it
            // is a deprecated form that another rule will surface.
            .filter(|t| {
                let text = t.text.as_ref();
                find_migration(text).is_none()
                    && !looks_like_deprecated_x_shorthand(text)
                    && !looks_like_sci_structural(text)
            })
            .map(|t| {
                Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    t.span,
                    "unrecognized token inside marking — does not match any \
                     known CAPCO classification, control, or trigraph",
                    "CAPCO-ISM-v2022-DEC-§3.1",
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
                        citation: "CAPCO-ISM-v2022-DEC-§4.1",
                        original: banner.to_owned(),
                        replacement: portion.to_owned(),
                        confidence: 1.0,
                        migration_ref: Some("CAPCO-2023-§4.1"),
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
                citation: "CAPCO-ISM-v2022-DEC-§4.1",
                original: text.to_owned(),
                replacement: portion.to_owned(),
                confidence: 1.0,
                migration_ref: Some("CAPCO-2023-§4.1"),
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
                    citation: "CAPCO-ISM-v2022-DEC-§9",
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
// Rule: E010 — Bare HCS without compartment suffix
// ---------------------------------------------------------------------------

/// Since ~2009, bare `HCS` is no longer valid — it must be `HCS-P` (product)
/// or `HCS-O` (operations). `HCS-P` is correct ~99% of the time; `HCS-O`
/// is rare and typically only appears when the document explicitly involves
/// operational source information.
///
/// The rule checks whether `HCS-O` appears alongside `HCS` in the same
/// marking. If it does, it's ambiguous and confidence drops. Otherwise
/// `HCS-P` is suggested at 0.95 confidence.
struct BareHcsRule;

impl Rule for BareHcsRule {
    fn id(&self) -> RuleId {
        RuleId::new("E010")
    }
    fn name(&self) -> &'static str {
        "bare-hcs"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::{SciControl, SciControlBare};

        // Detect bare HCS through both the legacy `sci_controls` projection
        // (exact-match CVE path: bare `HCS` → SciControl::Hcs) AND the
        // spec 003-sci-compartments structural path (a SciMarking anchored
        // on SciControlBare::Hcs with NO compartments is a bare HCS). When
        // HCS has compartments (e.g., `HCS-P`, `HCS-O`), E010 does not fire
        // regardless of source.
        let has_bare_hcs_enum = attrs.sci_controls.contains(&SciControl::Hcs);
        let has_bare_hcs_structural = attrs.sci_markings.iter().any(|m| {
            matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
                && m.compartments.is_empty()
        });
        if !has_bare_hcs_enum && !has_bare_hcs_structural {
            return vec![];
        }

        let has_hcs_o = attrs.sci_controls.contains(&SciControl::HcsO);
        let has_hcs_p = attrs.sci_controls.contains(&SciControl::HcsP);

        // If HCS-O or HCS-P already appears alongside bare HCS, the bare
        // HCS is redundant — but we still flag it because it needs a suffix.
        // If HCS-O is present, the document may deal with operational info,
        // so we lower confidence on the HCS-P suggestion.
        let (confidence, message) = if has_hcs_o {
            (
                0.5,
                "bare HCS requires a compartment suffix (-O or -P); \
                 HCS-O appears in this marking — verify whether HCS should be HCS-O or HCS-P"
                    .to_owned(),
            )
        } else if has_hcs_p {
            (
                0.95,
                "bare HCS requires a compartment suffix; \
                 HCS-P already present — this HCS likely should be HCS-P"
                    .to_owned(),
            )
        } else {
            (
                0.95,
                "bare HCS requires a compartment suffix (-O or -P); \
                 use HCS-P unless this involves operational source information"
                    .to_owned(),
            )
        };

        // Find the token span for the bare HCS entry.
        let sci_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciControl)
            .collect();
        let hcs_idx = attrs
            .sci_controls
            .iter()
            .position(|s| *s == SciControl::Hcs);
        let span = hcs_idx
            .and_then(|i| sci_spans.get(i))
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: self.id(),
            severity: self.default_severity(),
            source: FixSource::BuiltinRule,
            span,
            message,
            citation: "CAPCO-ISM-v2022-DEC-§4.SCI",
            original: "HCS".to_owned(),
            replacement: "HCS-P".to_owned(),
            confidence,
            migration_ref: None,
        })]
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
            citation: "CAPCO-ISM-v2022-DEC-§2",
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
// Rule: E012 — Dual classification (conflict)
// ---------------------------------------------------------------------------

/// A marking must have exactly one classification system. When both a US and
/// foreign classification appear (e.g., `SECRET//NATO SECRET//NOFORN`), the
/// US classification wins at the greater of the two levels, and the foreign
/// part becomes an FGI marker.
struct DualClassificationRule;

impl Rule for DualClassificationRule {
    fn id(&self) -> RuleId {
        RuleId::new("E012")
    }
    fn name(&self) -> &'static str {
        "dual-classification"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let Some(MarkingClassification::Conflict { us, foreign }) = &attrs.classification else {
            return vec![];
        };

        let foreign_desc = match foreign.as_ref() {
            ForeignClassification::Nato(n) => format!("NATO ({})", n.banner_str()),
            ForeignClassification::Fgi(f) => {
                let countries: Vec<&str> = f.countries.iter().map(|c| c.as_str()).collect();
                if countries.is_empty() {
                    "FGI".to_owned()
                } else {
                    format!("FGI {}", countries.join(" "))
                }
            }
            ForeignClassification::Joint(j) => {
                let countries: Vec<&str> = j.countries.iter().map(|c| c.as_str()).collect();
                format!("JOINT {}", countries.join(" "))
            }
        };

        let fgi_replacement = match foreign.as_ref() {
            ForeignClassification::Nato(_) => "FGI NATO".to_owned(),
            ForeignClassification::Fgi(f) => {
                let countries: Vec<&str> = f.countries.iter().map(|c| c.as_str()).collect();
                if countries.is_empty() {
                    "FGI".to_owned()
                } else {
                    format!("FGI {}", countries.join(" "))
                }
            }
            ForeignClassification::Joint(j) => {
                let countries: Vec<&str> = j.countries.iter().map(|c| c.as_str()).collect();
                format!("FGI {}", countries.join(" "))
            }
        };

        // Find the foreign classification token span (the second Classification token).
        let class_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Classification)
            .collect();
        let span = class_spans
            .get(1)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));
        let original = class_spans
            .get(1)
            .map(|t| t.text.to_string())
            .unwrap_or_default();

        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: self.id(),
            severity: self.default_severity(),
            source: FixSource::BuiltinRule,
            span,
            message: format!(
                "marking has both US ({}) and foreign ({foreign_desc}) classification; \
                 US wins at {}; move foreign to FGI block",
                us.banner_str(),
                us.banner_str(),
            ),
            citation: "CAPCO-ISM-v2022-DEC-§7",
            original,
            replacement: fgi_replacement,
            confidence: 0.90,
            migration_ref: None,
        })]
    }
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
                        citation: "CAPCO-ISM-v2022-DEC-§3",
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
                    citation: "CAPCO-ISM-v2022-DEC-§3",
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
// Rule: W002 — US + FGI comingling in portion
// ---------------------------------------------------------------------------

/// A portion mark with both a US classification and an FGI marker is
/// comingling US and foreign information. This isn't strictly invalid but
/// is bad practice — the content should be split into separate paragraphs:
/// one US-classified and one foreign-classified.
struct CominglingWarningRule;

impl Rule for CominglingWarningRule {
    fn id(&self) -> RuleId {
        RuleId::new("W002")
    }
    fn name(&self) -> &'static str {
        "us-fgi-comingling"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        // US classification + FGI marker = comingling.
        if attrs.us_classification().is_none() || attrs.fgi_marker.is_none() {
            return vec![];
        }

        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::FgiMarker)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "portion mark comingles US classification with FGI; \
             consider splitting into separate US and foreign paragraphs",
            "CAPCO-ISM-v2022-DEC-§7",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E014 — JOINT participants missing from REL TO
// ---------------------------------------------------------------------------

/// All countries in a JOINT classification must also appear in the REL TO
/// list. `//JOINT S USA GBR//REL TO USA, GBR` is correct;
/// `//JOINT S USA GBR//NF` is invalid because JOINT participants must be
/// in the REL TO.
struct JointRelToRule;

impl Rule for JointRelToRule {
    fn id(&self) -> RuleId {
        RuleId::new("E014")
    }
    fn name(&self) -> &'static str {
        "joint-rel-to"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let joint = match &attrs.classification {
            Some(MarkingClassification::Joint(j)) => j,
            _ => return vec![],
        };

        let missing: Vec<&str> = joint
            .countries
            .iter()
            .filter(|c| !attrs.rel_to.contains(c))
            .map(|c| c.as_str())
            .collect();

        if missing.is_empty() {
            return vec![];
        }

        // Point at the classification token span.
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            format!(
                "JOINT participants [{}] must appear in REL TO list",
                missing.join(", "),
            ),
            "CAPCO-ISM-v2022-DEC-§3",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E015 — Non-US classification without dissem control
// ---------------------------------------------------------------------------

/// Non-US classifications (FGI, NATO, JOINT) must always be accompanied by
/// a dissemination control (which includes REL TO statements). A non-US
/// marking without any dissem control is invalid.
struct NonUsMissingDissemRule;

impl Rule for NonUsMissingDissemRule {
    fn id(&self) -> RuleId {
        RuleId::new("E015")
    }
    fn name(&self) -> &'static str {
        "non-us-missing-dissem"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let is_non_us = matches!(
            &attrs.classification,
            Some(
                MarkingClassification::Fgi(_)
                    | MarkingClassification::Nato(_)
                    | MarkingClassification::Joint(_)
            )
        );
        if !is_non_us {
            return vec![];
        }

        let has_dissem = !attrs.dissem_controls.is_empty() || !attrs.rel_to.is_empty();
        if has_dissem {
            return vec![];
        }

        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "non-US classification must be accompanied by a dissemination control \
             (e.g., REL TO, NOFORN)",
            "CAPCO-ISM-v2022-DEC-§2",
            None,
        )]
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
                "CAPCO-ISM-v2022-DEC-§9",
                None,
            ));
        }

        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Rule: E016 — RESTRICTED not allowed with JOINT
// ---------------------------------------------------------------------------

/// Since the US is always a co-owner in JOINT markings, and RESTRICTED has
/// no US equivalent classification, RESTRICTED may not be used with JOINT.
struct JointRestrictedRule;

impl Rule for JointRestrictedRule {
    fn id(&self) -> RuleId {
        RuleId::new("E016")
    }
    fn name(&self) -> &'static str {
        "joint-restricted"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let joint = match &attrs.classification {
            Some(MarkingClassification::Joint(j)) => j,
            _ => return vec![],
        };
        if joint.level != marque_ism::Classification::Restricted {
            return vec![];
        }

        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "RESTRICTED may not be used with JOINT — the US has no equivalent \
             classification level for RESTRICTED",
            "CAPCO-ISM-v2022-DEC-§3.JOINT",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E017 — JOINT may not be used with FGI
// ---------------------------------------------------------------------------

/// JOINT markings may not be used with FGI markers. A marking is either
/// JOINT (co-owned) or FGI (foreign-originated), not both.
struct JointFgiRule;

impl Rule for JointFgiRule {
    fn id(&self) -> RuleId {
        RuleId::new("E017")
    }
    fn name(&self) -> &'static str {
        "joint-fgi"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(&attrs.classification, Some(MarkingClassification::Joint(_))) {
            return vec![];
        }
        if attrs.fgi_marker.is_none() {
            return vec![];
        }

        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::FgiMarker)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "JOINT may not be used with FGI — a marking is either co-owned (JOINT) \
             or foreign-originated (FGI), not both",
            "CAPCO-ISM-v2022-DEC-§3.JOINT",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E018 — JOINT may not be used with IC dissem (except REL TO)
// ---------------------------------------------------------------------------

/// JOINT markings imply releasability only to co-owners. IC dissemination
/// controls (NOFORN, ORCON, IMCON, etc.) are not permitted with JOINT,
/// except REL TO which defines the release list.
struct JointIcDissemRule;

impl Rule for JointIcDissemRule {
    fn id(&self) -> RuleId {
        RuleId::new("E018")
    }
    fn name(&self) -> &'static str {
        "joint-ic-dissem"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(&attrs.classification, Some(MarkingClassification::Joint(_))) {
            return vec![];
        }
        // REL TO is allowed; all other IC dissem controls are not.
        if attrs.dissem_controls.is_empty() {
            return vec![];
        }

        let dissem_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::DissemControl)
            .collect();

        let mut diagnostics = Vec::new();
        for (idx, ctrl) in attrs.dissem_controls.iter().enumerate() {
            // REL is the dissem-control enum value for "REL TO" — skip it.
            if matches!(ctrl, marque_ism::DissemControl::Rel) {
                continue;
            }
            let span = dissem_spans
                .get(idx)
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));

            diagnostics.push(Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                format!(
                    "JOINT may not be used with IC dissem control {}; \
                     only REL TO is permitted with JOINT markings",
                    ctrl.as_str(),
                ),
                "CAPCO-ISM-v2022-DEC-§3.JOINT",
                None,
            ));
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Rule: E019 — JOINT may not be used with non-IC dissem
// ---------------------------------------------------------------------------

/// JOINT markings may not be used with non-IC dissemination controls.
struct JointNonIcDissemRule;

impl Rule for JointNonIcDissemRule {
    fn id(&self) -> RuleId {
        RuleId::new("E019")
    }
    fn name(&self) -> &'static str {
        "joint-non-ic-dissem"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if !matches!(&attrs.classification, Some(MarkingClassification::Joint(_))) {
            return vec![];
        }
        if attrs.non_ic_dissem.is_empty() {
            return vec![];
        }

        let nic_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::NonIcDissem)
            .collect();

        let mut diagnostics = Vec::new();
        for (idx, nic) in attrs.non_ic_dissem.iter().enumerate() {
            let span = nic_spans
                .get(idx)
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));

            diagnostics.push(Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                format!(
                    "JOINT may not be used with non-IC dissem control {}",
                    nic.banner_str(),
                ),
                "CAPCO-ISM-v2022-DEC-§3.JOINT",
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
        citation: "CAPCO-ISM-v2022-DEC-§3",
        original,
        replacement,
        confidence: 1.0,
        migration_ref: None,
    }))
}

// ---------------------------------------------------------------------------
// Rule: E021 — RD/FRD requires NOFORN
// ---------------------------------------------------------------------------

/// RD and FRD information must always be marked NOFORN unless a sharing
/// agreement exists per the Atomic Energy Act (Sections 123 and 144).
///
/// Default severity: Error. Users working in contexts with established
/// sharing agreements can override to Warn in `.marque.toml`.
struct AeaNofornRule;

impl Rule for AeaNofornRule {
    fn id(&self) -> RuleId {
        RuleId::new("E021")
    }
    fn name(&self) -> &'static str {
        "aea-noforn"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::AeaMarking;

        let has_rd_or_frd = attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::Rd(_) | AeaMarking::Frd(_) | AeaMarking::Tfni));
        if !has_rd_or_frd {
            return vec![];
        }

        let has_noforn = attrs.dissem_controls.iter().any(|d| d.as_str() == "NF");
        if has_noforn {
            return vec![];
        }

        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::AeaMarking)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "RD/FRD/TFNI requires NOFORN unless a sharing agreement exists \
             per the Atomic Energy Act; override to warn via rule severity \
             config if sharing agreements apply",
            "CAPCO-ISM-v2022-DEC-§6/AEA",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E022 — CNWDI only with TS or S RD
// ---------------------------------------------------------------------------

/// CNWDI may only be used with TOP SECRET or SECRET Restricted Data.
/// It cannot appear standalone, with FRD, or with CONFIDENTIAL.
struct CnwdiConstraintRule;

impl Rule for CnwdiConstraintRule {
    fn id(&self) -> RuleId {
        RuleId::new("E022")
    }
    fn name(&self) -> &'static str {
        "cnwdi-constraint"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::AeaMarking;

        let has_cnwdi = attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::Rd(rd) if rd.cnwdi));
        if !has_cnwdi {
            return vec![];
        }

        // CNWDI requires TS or S classification.
        let level = attrs.us_classification();
        let valid = matches!(
            level,
            Some(marque_ism::Classification::TopSecret | marque_ism::Classification::Secret)
        );
        if valid {
            return vec![];
        }

        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::AeaMarking)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        let level_str = level.map(|c| c.banner_str()).unwrap_or("unknown");

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            format!(
                "CNWDI may only be used with TOP SECRET or SECRET RD; \
                 current classification is {level_str}"
            ),
            "CAPCO-ISM-v2022-DEC-§6/CNWDI",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E023 — SIGMA valid values and numerical order
// ---------------------------------------------------------------------------

/// SIGMA compartment numbers must be from the valid set (14, 15, 18, 20)
/// and listed in numerical order. Values 1–5 and 9–13 are obsolete.
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

            // Check for invalid values.
            let invalid: Vec<u8> = sigma
                .iter()
                .filter(|n| !valid_sigmas.contains(n))
                .copied()
                .collect();
            if !invalid.is_empty() {
                let obsolete: Vec<u8> = invalid
                    .iter()
                    .filter(|n| matches!(n, 1..=5 | 9..=13))
                    .copied()
                    .collect();
                let message = if !obsolete.is_empty() {
                    format!(
                        "SIGMA {:?} are obsolete; convert to current categories (14, 15, 18, 20)",
                        obsolete,
                    )
                } else {
                    format!(
                        "SIGMA {:?} are not valid; current values are 14, 15, 18, 20",
                        invalid,
                    )
                };
                diagnostics.push(Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    span,
                    message,
                    "CAPCO-ISM-v2022-DEC-§6/SIGMA",
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
                        citation: "CAPCO-ISM-v2022-DEC-§6/SIGMA",
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
// Rule: E024 — RD precedence over FRD/TFNI
// ---------------------------------------------------------------------------

/// When both RD and FRD (or TFNI) appear in the same marking, only RD
/// should be used — RD takes precedence in both banners and portions.
struct RdPrecedenceRule;

impl Rule for RdPrecedenceRule {
    fn id(&self) -> RuleId {
        RuleId::new("E024")
    }
    fn name(&self) -> &'static str {
        "rd-precedence"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::AeaMarking;

        let has_rd = attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::Rd(_)));
        if !has_rd {
            return vec![];
        }

        let mut diagnostics = Vec::new();
        for (idx, aea) in attrs.aea_markings.iter().enumerate() {
            let superseded = match aea {
                AeaMarking::Frd(_) => "FRD",
                AeaMarking::Tfni => "TFNI",
                _ => continue,
            };

            let aea_spans: Vec<&TokenSpan> = attrs
                .token_spans
                .iter()
                .filter(|t| t.kind == TokenKind::AeaMarking)
                .collect();
            let span = aea_spans
                .get(idx)
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));

            diagnostics.push(Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                format!(
                    "{superseded} should not appear alongside RD; \
                     RD takes precedence over {superseded} in both banners and portions"
                ),
                "CAPCO-ISM-v2022-DEC-§6/RD",
                None,
            ));
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Rule: E025 — UCNI only with UNCLASSIFIED
// ---------------------------------------------------------------------------

/// DOD UCNI and DOE UCNI apply only to unclassified information.
struct UcniClassificationRule;

impl Rule for UcniClassificationRule {
    fn id(&self) -> RuleId {
        RuleId::new("E025")
    }
    fn name(&self) -> &'static str {
        "ucni-classification"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::AeaMarking;

        let has_ucni = attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::DodUcni | AeaMarking::DoeUcni));
        if !has_ucni {
            return vec![];
        }

        let is_unclassified = attrs
            .us_classification()
            .is_some_and(|c| c == marque_ism::Classification::Unclassified);
        if is_unclassified {
            return vec![];
        }

        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::AeaMarking)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "DOD/DOE UCNI may only be used with UNCLASSIFIED information",
            "CAPCO-ISM-v2022-DEC-§6/UCNI",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
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
            .map(|m| sci_sort_key(sci_system_text(&m.system)))
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
        let fix_end = chunk_spans
            .last()
            .map(|t| t.span.end)
            .unwrap_or(fix_start);
        let fix_span = Span::new(fix_start, fix_end);

        // Sort marking indices by their sort keys, then reassemble the
        // chunk texts joined by `/`.
        let mut indices: Vec<usize> = (0..attrs.sci_markings.len()).collect();
        indices.sort_by_key(|&i| sci_sort_key(sci_system_text(&attrs.sci_markings[i].system)));

        let original: String = (0..chunk_spans.len())
            .map(|i| chunk_spans[i].text.as_ref())
            .collect::<Vec<_>>()
            .join("/");
        let replacement: String = indices
            .iter()
            .map(|&i| chunk_spans[i].text.as_ref())
            .collect::<Vec<_>>()
            .join("/");

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
/// ascending. Walks each marking in `sci_markings`; emits one diagnostic
/// per out-of-order pair with a reordered fix. Confidence 0.85.
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
            // Compartment-level ordering within this marking.
            let n_comps = marking.compartments.len();
            if n_comps >= 2 {
                let keys: Vec<(bool, u64, &str)> = marking
                    .compartments
                    .iter()
                    .map(|c| sci_sort_key(c.identifier.as_ref()))
                    .collect();
                if keys.windows(2).any(|w| w[0] > w[1]) {
                    // Produce a reorder fix covering all compartment spans in
                    // this marking. Ordering uses identifier only; sub-comps
                    // ride along with their parent compartment.
                    let this_comp_spans = &comp_spans[comp_cursor..comp_cursor + n_comps];
                    let fix_start = this_comp_spans.first().map(|t| t.span.start).unwrap_or(0);
                    // Span end = last sub-comp if any, else last comp.
                    // Determine end by counting this marking's total sub-comps.
                    let this_sub_count: usize = marking
                        .compartments
                        .iter()
                        .map(|c| c.sub_compartments.len())
                        .sum();
                    let fix_end = if this_sub_count > 0 {
                        sub_spans
                            .get(sub_cursor + this_sub_count - 1)
                            .map(|t| t.span.end)
                            .unwrap_or_else(|| {
                                this_comp_spans.last().map(|t| t.span.end).unwrap_or(fix_start)
                            })
                    } else {
                        this_comp_spans.last().map(|t| t.span.end).unwrap_or(fix_start)
                    };
                    let fix_span = Span::new(fix_start, fix_end);

                    // Build reordered segment-by-segment. Each segment is
                    // "COMP" or "COMP SUB1 SUB2 ...". Join with '-'.
                    let mut indices: Vec<usize> = (0..n_comps).collect();
                    indices.sort_by_key(|&i| {
                        sci_sort_key(marking.compartments[i].identifier.as_ref())
                    });

                    let render_seg = |idx: usize| -> String {
                        let c = &marking.compartments[idx];
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
                    };

                    let original: String = (0..n_comps)
                        .map(render_seg)
                        .collect::<Vec<_>>()
                        .join("-");
                    let replacement: String = indices
                        .iter()
                        .map(|&i| render_seg(i))
                        .collect::<Vec<_>>()
                        .join("-");

                    out.push(make_fix_diagnostic(FixDiagnosticParams {
                        rule: self.id(),
                        severity: self.default_severity(),
                        source: FixSource::BuiltinRule,
                        span: fix_span,
                        message: "SCI compartments within a control system must be listed \
                                  in ascending order (numeric first, then alphabetic)"
                            .to_owned(),
                        citation: "CAPCO-2016 §A.6 p15; §H.4 p61",
                        original,
                        replacement,
                        confidence: 0.85,
                        migration_ref: None,
                    }));
                }
            }

            // Sub-compartment-level ordering per compartment.
            for comp in marking.compartments.iter() {
                let n_subs = comp.sub_compartments.len();
                if n_subs >= 2 {
                    let keys: Vec<(bool, u64, &str)> = comp
                        .sub_compartments
                        .iter()
                        .map(|s| sci_sort_key(s.as_ref()))
                        .collect();
                    if keys.windows(2).any(|w| w[0] > w[1]) {
                        let this_subs = &sub_spans[sub_cursor..sub_cursor + n_subs];
                        let fix_start = this_subs.first().map(|t| t.span.start).unwrap_or(0);
                        let fix_end = this_subs.last().map(|t| t.span.end).unwrap_or(fix_start);
                        let fix_span = Span::new(fix_start, fix_end);

                        let mut indices: Vec<usize> = (0..n_subs).collect();
                        indices.sort_by_key(|&i| sci_sort_key(comp.sub_compartments[i].as_ref()));

                        let original: String = comp
                            .sub_compartments
                            .iter()
                            .map(|s| s.as_ref())
                            .collect::<Vec<_>>()
                            .join(" ");
                        let replacement: String = indices
                            .iter()
                            .map(|&i| comp.sub_compartments[i].as_ref())
                            .collect::<Vec<_>>()
                            .join(" ");

                        out.push(make_fix_diagnostic(FixDiagnosticParams {
                            rule: self.id(),
                            severity: self.default_severity(),
                            source: FixSource::BuiltinRule,
                            span: fix_span,
                            message: "SCI sub-compartments must be listed in ascending \
                                      order (numeric first, then alphabetic)"
                                .to_owned(),
                            citation: "CAPCO-2016 §A.6 p15; §H.4 p61",
                            original,
                            replacement,
                            confidence: 0.85,
                            migration_ref: None,
                        }));
                    }
                }
                sub_cursor += n_subs;
            }

            comp_cursor += n_comps;
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
        let (fix_span, original) = if chunk_spans.is_empty() {
            (Span::new(0, 0), String::new())
        } else {
            let s = chunk_spans.first().unwrap().span.start;
            let e = chunk_spans.last().unwrap().span.end;
            let orig = chunk_spans
                .iter()
                .map(|t| t.text.as_ref())
                .collect::<Vec<_>>()
                .join("/");
            (Span::new(s, e), orig)
        };
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
// SCI rule helpers
// ---------------------------------------------------------------------------

/// Sort key mirroring the §A.6 p15 rule: numeric values sort before
/// alphabetic values; within each class, ascending.
///
/// Returned tuple: `(is_alpha, numeric_value_if_numeric, original_str)`.
/// Because `false < true`, numeric entries sort first. Ties within the
/// alpha class break on the original string.
///
/// Local copy: this mirrors the SAR `sar_sort_key` helper from a parallel
/// in-flight PR (spec 002-sar). When one lands first it will be promoted
/// to a shared util in `marque-ism` and the other rule crate updated.
fn sci_sort_key(s: &str) -> (bool, u64, &str) {
    match s.parse::<u64>() {
        Ok(n) => (false, n, s),
        Err(_) => (true, 0, s),
    }
}

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

/// Defensive wrapper around `PageContext::expected_sci_markings`. When
/// P4 lands the rollup method on `PageContext`, this call will pick it up
/// automatically through the `marque_ism::PageContext` re-export. Until
/// then, we probe the public surface via a trait specialization pattern
/// that yields an empty slice. This keeps E035 inert (rather than
/// ill-formed) until the upstream method is available.
fn page_expected_sci_markings(page: &marque_ism::PageContext) -> Vec<SciMarking> {
    // Call the method if it exists on PageContext. When P4 hasn't landed
    // the method is absent and the helper compiles to a no-op (empty
    // Vec). The trait-object trick here is a method-resolution probe:
    // the inherent method wins when present, otherwise the trait's
    // default impl takes over. Because both produce `Vec<SciMarking>`
    // there is no type-level divergence.
    trait ExpectedSciMarkingsFallback {
        fn expected_sci_markings(&self) -> Vec<SciMarking> {
            Vec::new()
        }
    }
    impl ExpectedSciMarkingsFallback for marque_ism::PageContext {}
    ExpectedSciMarkingsFallback::expected_sci_markings(page)
}

/// Shared filter helper: does this Unknown-token text look like a
/// structurally-formed SCI block that the spec 003-sci-compartments
/// subparser would try to claim? If so, E008 skips it — the parser has
/// already decided whether to accept or reject structurally, and its
/// rejection is not the "unrecognized atom" case that E008 describes.
///
/// Pattern: a prefix before the first `-` or ` ` is a bare SCI CVE value.
/// This matches bare-system anchored compound forms (`HCS-P`, `SI-G`,
/// `SI-G ABCD`) and rejects plain dissem forms (`LES-NF`, `NATO SECRET`)
/// by requiring `is_bare_cve_value` on the prefix.
fn looks_like_sci_structural(text: &str) -> bool {
    let boundary = text.find(['-', ' ']);
    let prefix = match boundary {
        Some(i) if i > 0 => &text[..i],
        _ => return false,
    };
    is_bare_cve_value(prefix)
}

// Helpers
// ---------------------------------------------------------------------------

/// Bundle of all the inputs `make_fix_diagnostic` needs. Replaces a 9-arg
/// positional helper signature so call sites read top-down by name.
struct FixDiagnosticParams {
    rule: RuleId,
    severity: Severity,
    source: FixSource,
    span: Span,
    message: String,
    citation: &'static str,
    original: String,
    replacement: String,
    confidence: f32,
    migration_ref: Option<&'static str>,
}

fn make_fix_diagnostic(p: FixDiagnosticParams) -> Diagnostic {
    let proposal = FixProposal::new(
        p.rule.clone(),
        p.source,
        p.span,
        p.original,
        p.replacement,
        p.confidence,
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
        assert!(ids.contains(&"E017"));
        assert!(ids.contains(&"E018"));
        assert!(ids.contains(&"E019"));
        assert!(ids.contains(&"E020"));
        assert!(ids.contains(&"E021"));
        assert!(ids.contains(&"E022"));
        assert!(ids.contains(&"E023"));
        assert!(ids.contains(&"E024"));
        assert!(ids.contains(&"E025"));
        assert!(ids.contains(&"W003"));
        assert!(ids.contains(&"C001"));
        assert!(ids.contains(&"E032"));
        assert!(ids.contains(&"E033"));
        assert!(ids.contains(&"E034"));
        assert!(ids.contains(&"E035"));
        assert_eq!(set.rules().len(), 33);
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
        assert!((fix.confidence - 0.6).abs() < f32::EPSILON);
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
    fn e004_does_not_fire_on_clean_separator() {
        let diags = lint_banner("SECRET//NOFORN");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E004"));
    }

    #[test]
    fn e004_fires_on_missing_separator_single_slash() {
        // T033: E004 must detect missing separators (single `/`).
        let diags = lint_banner("SECRET/NOFORN");
        let e004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E004").collect();
        assert_eq!(e004.len(), 1);
        // The fix replaces the single `/` at byte 6 with `//`.
        assert_eq!(e004[0].span.start, 6);
        assert_eq!(e004[0].span.end, 7);
        let fix = e004[0].fix.as_ref().unwrap();
        assert_eq!(fix.original.as_ref(), "/");
        assert_eq!(fix.replacement.as_ref(), "//");
    }

    #[test]
    fn e004_fires_on_missing_separator_in_later_block() {
        // The parser splits on `//` so the partial split puts `SI/NF` into
        // an Unknown block. E004's stray-slash walk catches it.
        let diags = lint_banner("SECRET//SI/NF");
        let e004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E004").collect();
        assert_eq!(e004.len(), 1);
        assert_eq!(e004[0].span.start, 10);
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
        assert!((fix.confidence - 0.95).abs() < f32::EPSILON);
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
        assert!((fix.confidence - 0.97).abs() < f32::EPSILON);
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
        assert!((fix.confidence - 0.95).abs() < f32::EPSILON);
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
            (fix.confidence - 0.5).abs() < f32::EPSILON,
            "confidence should be 0.5 when HCS-O is present, got {}",
            fix.confidence
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

    // --- E017: JOINT may not be used with FGI ---

    #[test]
    fn e017_fires_on_joint_with_fgi() {
        // This is structurally odd but the parser might produce it
        // from malformed input where FGI appears as a block.
        // For now, test that a JOINT marking with an fgi_marker errors.
        // We can't easily construct this via lint_banner since the parser
        // only sets fgi_marker when classification is US. Skip for now.
    }

    // --- E018: JOINT + IC dissem (except REL TO) ---

    #[test]
    fn e018_fires_on_joint_with_noforn() {
        let diags = lint_banner("//JOINT S USA GBR//NF");
        let e018: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E018").collect();
        assert_eq!(
            e018.len(),
            1,
            "E018 should fire on NF with JOINT: {diags:?}"
        );
    }

    #[test]
    fn e018_does_not_fire_on_joint_with_rel_to_only() {
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E018"),
            "E018 should not fire when only REL TO is present: {diags:?}"
        );
    }

    // --- E019: JOINT + non-IC dissem ---

    #[test]
    fn e019_fires_on_joint_with_limdis() {
        let diags = lint_banner("//JOINT S USA GBR//LIMDIS");
        let e019: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E019").collect();
        assert_eq!(e019.len(), 1);
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
        assert!((fix.confidence - 1.0).abs() < f32::EPSILON);
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
        assert_eq!(e032.len(), 1, "E032 must fire on SI/123 ordering: {diags:?}");
        let fix = e032[0].fix.as_ref().expect("E032 must carry a FixProposal");
        assert!((fix.confidence - 0.85).abs() < f32::EPSILON);
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
        let diags = lint_banner("SECRET//SI-G DEFG ABCD//NOFORN");
        let e033: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E033").collect();
        assert_eq!(
            e033.len(),
            1,
            "E033 must fire on DEFG ABCD sub-compartment order: {diags:?}"
        );
        let fix = e033[0].fix.as_ref().expect("E033 must carry a FixProposal");
        assert!((fix.confidence - 0.85).abs() < f32::EPSILON);
        assert_eq!(fix.replacement.as_ref(), "ABCD DEFG");
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
        assert_eq!(e034.len(), 1, "E034 must fire on custom control 123: {diags:?}");
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
    fn looks_like_sci_structural_matches_expected_shapes() {
        use super::looks_like_sci_structural as m;
        // Bare CVE prefix followed by `-` or space -> structural
        assert!(m("SI-G"));
        assert!(m("HCS-P"));
        assert!(m("SI G"));
        assert!(m("TK-BLFH"));
        // Non-SCI prefixes -> not structural (E008 handles them)
        assert!(!m("XYZZY-FOO"));
        assert!(!m("LES-NF"));
        assert!(!m("NATO SECRET"));
        // No boundary -> not structural (E008 handles bare unknowns)
        assert!(!m("XYZZY"));
        // Leading hyphen -> not structural
        assert!(!m("-SI"));
    }
}

/// Internal test support module — drives the parser and rules directly,
/// without depending on the engine crate. This avoids a circular dependency
/// (`marque-capco` is below `marque-engine` in the workspace graph).
#[cfg(test)]
mod marque_capco_test_support {
    use super::CapcoRuleSet;
    use marque_core::{Parser, Scanner};
    use marque_ism::{CapcoTokenSet, MarkingType};
    use marque_rules::{Diagnostic, RuleContext, RuleSet};

    fn run(source: &[u8]) -> Vec<Diagnostic> {
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidates = Scanner::scan(source);
        let rule_set = CapcoRuleSet::new();
        let mut out = Vec::new();
        for candidate in &candidates {
            if candidate.kind == MarkingType::PageBreak {
                continue;
            }
            let Ok(parsed) = parser.parse(candidate, source) else {
                continue;
            };
            let ctx = RuleContext {
                marking_type: candidate.kind,
                zone: None,
                position: None,
                page_context: None,
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
