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
//!   C001 = corrections-map typo (T058, Phase 5)

use marque_ism::generated::migrations::find_migration;
use marque_ism::{
    ForeignClassification, IsmAttributes, MarkingClassification, Span, TokenKind, TokenSpan,
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
                Box::new(SarPortionFormRule),
                Box::new(SarClassificationRule),
                Box::new(SarProgramOrderRule),
                Box::new(SarCompartmentOrderRule),
                Box::new(SarIndicatorRepeatRule),
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
                citation: "CAPCO-2016 §A.6",
                original: abbrev.to_owned(),
                replacement: full.to_owned(),
                confidence: 1.0,
                migration_ref: None,
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
                citation: "CAPCO-2016 §A.6",
                original: abbrev.to_owned(),
                replacement: full.to_owned(),
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
                None,
            )
        });

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "marking blocks are out of CAPCO order \
             (expected: Classification // SCI // SAR // Dissem // REL TO // Non-IC)",
            "CAPCO-2016 §A.6",
            fix,
        )]
    }
}

fn ordinal_for_block(kind: TokenKind) -> Option<u8> {
    match kind {
        TokenKind::Classification => Some(0),
        TokenKind::SciControl => Some(1),
        TokenKind::SarIndicator => Some(2),
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
            TokenKind::SarIndicator => sar.push(token.text.as_ref()),
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
///
/// Categories that can legitimately contain multiple values joined by `/`
/// within a single block are represented; TokenKinds that never appear as
/// multi-value blocks (e.g., `Classification`, `FgiMarker`, `DeclassDate`)
/// return `None` from [`category_of`] so the rule declines to fire.
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
        TokenKind::SciControl => Some(SeparatorCategory::Sci),
        TokenKind::DissemControl => Some(SeparatorCategory::Dissem),
        TokenKind::NonIcDissem => Some(SeparatorCategory::NonIcDissem),
        TokenKind::AeaMarking => Some(SeparatorCategory::Aea),
        TokenKind::SarIndicator => Some(SeparatorCategory::Sar),
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
                find_migration(text).is_none() && !looks_like_deprecated_x_shorthand(text)
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
        use marque_ism::SciControl;

        let has_bare_hcs = attrs.sci_controls.contains(&SciControl::Hcs);
        if !has_bare_hcs {
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
            citation: "CAPCO-2016 §H.4",
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
            citation: "CAPCO-2016 §H.4",
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
            citation: "CAPCO-2016 §B.1",
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
            "CAPCO-2016 §H.7",
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
            "CAPCO-2016 §H.3",
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
            "CAPCO-2016 §B.3",
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
                "CAPCO-2016 §H.9",
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
            "CAPCO-2016 §H.3",
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
            "CAPCO-2016 §H.3",
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
                "CAPCO-2016 §H.3",
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
                "CAPCO-2016 §H.3",
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
            "CAPCO-2016 §H.6",
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
            "CAPCO-2016 §H.6",
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
                    "CAPCO-2016 §H.6",
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
                        citation: "CAPCO-2016 §H.6",
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
                "CAPCO-2016 §H.6",
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
            "CAPCO-2016 §H.6",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E026 — SAR portion must use `SAR-` abbreviation
// ---------------------------------------------------------------------------

/// Portion marks must use the `SAR-` abbreviation, not the full
/// `SPECIAL ACCESS REQUIRED-` form (CAPCO-2016 §H.5 p101 "Authorized
/// Portion Mark"). No fix is proposed because abbreviating an arbitrary
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

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "portion marks must use the SAR- abbreviation, not the \
             SPECIAL ACCESS REQUIRED- full form",
            "CAPCO-2016 §H.5",
            None,
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
        let mut sorted = sar.programs.to_vec();
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

        // Detect any out-of-order level. We emit at most one diagnostic
        // per SAR block (the fix reorders every level at once); the
        // message mentions whichever level tripped first so the author
        // sees the specific violation.
        let mut offending_level: Option<&'static str> = None;
        for prog in sar.programs.iter() {
            if prog.compartments.len() >= 2
                && !prog.compartments.windows(2).all(|w| {
                    sar_sort_key(&w[0].identifier) <= sar_sort_key(&w[1].identifier)
                })
            {
                offending_level = Some("compartments");
                break;
            }
            for comp in prog.compartments.iter() {
                if comp.sub_compartments.len() >= 2
                    && !comp
                        .sub_compartments
                        .windows(2)
                        .all(|w| sar_sort_key(&w[0]) <= sar_sort_key(&w[1]))
                {
                    offending_level = Some("sub-compartments");
                    break;
                }
            }
            if offending_level.is_some() {
                break;
            }
        }
        let Some(level) = offending_level else {
            return vec![];
        };
        let Some(span) = sar_block_span(attrs) else {
            return vec![];
        };

        let original = render_sar_block(sar.indicator, &sar.programs);

        // Sort every level in place.
        let mut sorted_programs = sar.programs.to_vec();
        for prog in sorted_programs.iter_mut() {
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
        let replacement = render_sar_block(sar.indicator, &sorted_programs);

        vec![make_fix_diagnostic(FixDiagnosticParams {
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
        })]
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
        for tok in attrs.token_spans.iter() {
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
            // Extend the span backward by the preceding `//` separator
            // so the fix can collapse `//SAR-CD` → `/CD`.
            let orig_start = tok.span.start;
            let fix_start = orig_start.saturating_sub(2);
            let fix_span = Span::new(fix_start, tok.span.end);
            let replacement = format!("/{stripped}");
            let original = format!("//{text}");

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

// Helpers
// ---------------------------------------------------------------------------

/// Sort key for CAPCO ascending order within a SAR block:
/// numeric-prefixed values sort before pure-alpha values, numeric
/// prefixes compare as `u64`, and ties break on byte-lex of the
/// remainder (CAPCO-2016 §H.5 p99, §A.6 p16 SAP bullet 5).
///
/// Returns `(is_alpha_only, numeric_prefix, remainder)`. The leading
/// bool sorts `false` (numeric-prefixed) before `true` (pure alpha).
fn sar_sort_key(s: &str) -> (bool, u64, &str) {
    let bytes = s.as_bytes();
    let mut digits_end = 0;
    while digits_end < bytes.len() && bytes[digits_end].is_ascii_digit() {
        digits_end += 1;
    }
    if digits_end == 0 {
        // Pure alpha (or starts with alpha) — sort after all numeric-prefixed.
        return (true, 0, s);
    }
    // Parse the leading digit run; overflow falls back to u64::MAX so
    // pathologically long numbers still compare deterministically.
    let num: u64 = s[..digits_end].parse().unwrap_or(u64::MAX);
    (false, num, &s[digits_end..])
}

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

/// Render a SAR block back to source form for fix replacements.
///
/// Abbreviated form: `SAR-<PROG>[-<COMP>[ <SUB>...]]{/<PROG>...}`.
/// Full form: `SPECIAL ACCESS REQUIRED-<PROG>{/<PROG>...}` — per §H.5
/// the parser keeps compartments empty for the full form, so this is a
/// simple join.
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
        assert!(ids.contains(&"E026"));
        assert!(ids.contains(&"E027"));
        assert!(ids.contains(&"E028"));
        assert!(ids.contains(&"E029"));
        assert!(ids.contains(&"E030"));
        assert_eq!(set.rules().len(), 34);
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
    fn e004_fires_on_same_category_sci_double_slash() {
        // Per CAPCO-2016 §A.6 Figure 2: SI and TK are both SCI controls and
        // must be joined with `/` within one block, not `//` across blocks.
        let diags = lint_banner("SECRET//SI//TK//NOFORN");
        let e004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E004").collect();
        assert_eq!(e004.len(), 1, "exactly one E004 on the SI//TK boundary: {diags:?}");
        let src = b"SECRET//SI//TK//NOFORN";
        // The span must point at the `//` between SI and TK (bytes 10..12).
        assert_eq!(e004[0].span.as_str(src).unwrap(), "//");
        assert_eq!(e004[0].span.start, 10);
        assert_eq!(e004[0].span.end, 12);
        let fix = e004[0].fix.as_ref().expect("E004 must carry a FixProposal");
        assert_eq!(fix.original.as_ref(), "//");
        assert_eq!(fix.replacement.as_ref(), "/");
        assert!((fix.confidence - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn e004_fires_on_same_category_dissem_double_slash() {
        // ORCON and NOFORN are both dissem controls — must be joined with `/`.
        let diags = lint_banner("SECRET//ORCON//NOFORN");
        let e004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E004").collect();
        assert_eq!(e004.len(), 1, "exactly one E004 on ORCON//NOFORN: {diags:?}");
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
        assert_eq!(e026.len(), 1, "E026 must fire on full form in portion: {diags:?}");
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
        assert!((fix.confidence - 0.85).abs() < f32::EPSILON);
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
        let diags = lint_banner("SECRET//SAR-BP-J12 Z9 A3//NOFORN");
        let e029: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E029").collect();
        assert_eq!(
            e029.len(),
            1,
            "E029 must fire on subs [Z9, A3] (out of order): {diags:?}"
        );
        let fix = e029[0].fix.as_ref().expect("E029 must carry a FixProposal");
        assert_eq!(fix.replacement.as_ref(), "SAR-BP-J12 A3 Z9");
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
        let diags = lint_banner("SECRET//SAR-BP-K15-J12//NOFORN");
        let e029: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E029").collect();
        assert_eq!(
            e029.len(),
            1,
            "E029 must fire on compartments K15 then J12: {diags:?}"
        );
        let fix = e029[0].fix.as_ref().unwrap();
        assert_eq!(fix.replacement.as_ref(), "SAR-BP-J12-K15");
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
        assert!((fix.confidence - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn e030_does_not_fire_on_single_sar_block() {
        let diags = lint_banner("SECRET//SAR-BP/CD//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E030"),
            "E030 must not fire when programs coalesce in one block: {diags:?}"
        );
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
