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
             (expected: Classification // SCI // SAR // Dissem // REL TO)",
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
        // Separators, declass, and unknown tokens do not participate in
        // ordering — they belong to other blocks or other rules.
        _ => None,
    }
}

/// Rebuild a marking string from `attrs.token_spans`, ordered by CAPCO
/// block ordinals: Classification // SCI // SAR // Dissem // REL TO.
///
/// Within each block, tokens preserve their document order. REL TO trigraphs
/// are reassembled into a single `REL TO ...` block. Returns `None` if there
/// is nothing meaningful to reorder (no classification recorded).
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

    for token in attrs.token_spans.iter() {
        match token.kind {
            TokenKind::Classification => classification.push(token.text.as_ref()),
            TokenKind::SciControl => sci.push(token.text.as_ref()),
            TokenKind::SarIdentifier => sar.push(token.text.as_ref()),
            TokenKind::DissemControl => dissem.push(token.text.as_ref()),
            TokenKind::RelToTrigraph => rel_to.push(token.text.as_ref()),
            _ => {}
        }
    }

    if classification.is_empty() {
        return None;
    }

    let mut blocks: Vec<String> = Vec::with_capacity(8);
    blocks.push(classification.join(" "));
    for s in sci {
        blocks.push(s.to_owned());
    }
    for s in sar {
        blocks.push(s.to_owned());
    }
    for d in dissem {
        blocks.push(d.to_owned());
    }
    if !rel_to.is_empty() {
        blocks.push(format!("REL TO {}", rel_to.join(", ")));
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
                find_migration(text).is_none() && !looks_like_deprecated_x_shorthand(text)
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

        let has_bare_hcs = attrs.sci_controls.iter().any(|s| *s == SciControl::Hcs);
        if !has_bare_hcs {
            return vec![];
        }

        let has_hcs_o = attrs.sci_controls.iter().any(|s| *s == SciControl::HcsO);
        let has_hcs_p = attrs.sci_controls.iter().any(|s| *s == SciControl::HcsP);

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
    let is_level = matches!(last, "TS" | "S" | "C" | "R" | "U"
        | "TOP SECRET" | "SECRET" | "CONFIDENTIAL" | "RESTRICTED" | "UNCLASSIFIED");
    if !is_level && !(parts.len() >= 3 && parts[parts.len() - 2] == "TOP" && last == "SECRET") {
        return false;
    }
    // Preceding tokens should look like country trigraphs or "FGI".
    let country_end = if parts.len() >= 3 && parts[parts.len() - 2] == "TOP" {
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
/// This rule detects commas in JOINT classification token text and spaces
/// (without commas) in REL TO token text.
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
        assert!(ids.contains(&"C001"));
        assert_eq!(set.rules().len(), 18);
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
            diags.iter().filter(|d| d.rule.as_str() == "E012").collect::<Vec<_>>()
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
            diags.iter().filter(|d| d.rule.as_str() == "E014").collect::<Vec<_>>()
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
            diags.iter().filter(|d| d.rule.as_str() == "E015").collect::<Vec<_>>()
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
