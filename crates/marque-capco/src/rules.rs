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
//!   W001 = deprecated marking warning (T038)
//!   C001 = corrections-map typo (T058, Phase 5)

use marque_ism::generated::migrations::find_migration;
use marque_ism::{IsmAttributes, Span, TokenKind, TokenSpan};
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
                Box::new(MissingUsaTrigraphRule),
                Box::new(MisorderedBlocksRule),
                Box::new(SeparatorCountRule),
                Box::new(DeclassifyInBannerRule),
                Box::new(DeprecatedDissemRule),
                Box::new(XShorthandDateRule),
                Box::new(UnknownTokenRule),
                Box::new(DeprecatedMarkingWarningRule),
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
            let Some(full) = expand_dissem_abbreviation(control) else {
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

/// Expand known portion-form abbreviations to their full banner forms.
fn expand_dissem_abbreviation(control: &marque_ism::DissemControl) -> Option<&'static str> {
    match control.as_str() {
        "NF" => Some("NOFORN"),
        "OC" => Some("ORCON"),
        "IMC" => Some("IMCON"),
        "DSEN" => Some("DEA SENSITIVE"),
        _ => None,
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
        let reordered = reorder_marking(attrs);
        let original: String = reordered
            .as_ref()
            .map(|_| {
                // The original bytes the engine would splice out: the
                // marking text from `first.start` to `last.end`. We don't
                // have source bytes here, so we reconstruct an approximation
                // by joining the in-order token texts. This is byte-
                // equivalent for whitespace-free markings (which is all
                // canonical fixtures) and good-enough as a display string.
                attrs
                    .token_spans
                    .iter()
                    .filter(|t| {
                        ordinal_for_block(t.kind).is_some() || t.kind == TokenKind::Separator
                    })
                    .map(|t| t.text.as_ref())
                    .collect::<String>()
            })
            .unwrap_or_default();

        let fix = reordered.map(|replacement| {
            FixProposal::new(
                self.id(),
                FixSource::BuiltinRule,
                span,
                original,
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

/// CAPCO X-shorthand declass codes (e.g., `25X1-`, `50X1-`) are deprecated
/// in favor of the canonical forms (`25X1`, `50X1-HUM`). The deprecated
/// dashed form is not in the CVE, so the parser surfaces it as
/// `TokenKind::Unknown`. E007 walks Unknown tokens and looks each up in
/// the migration table; a hit produces an E007 fix diagnostic.
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
            let Some(entry) = find_migration(token.text.as_ref()) else {
                continue;
            };
            // Skip entries owned by E006 (dissem deprecations). Reuses the
            // single `is_dissem_replacement` predicate so the E006/E007
            // guard sets cannot drift — if a new dissem control is added
            // to the shared set, both rules pick it up at the same time.
            if is_dissem_replacement(entry.replacement) {
                continue;
            }
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::MigrationTable,
                span: token.span,
                message: format!(
                    "X-shorthand declassification code {:?} is deprecated; use {:?}",
                    token.text, entry.replacement
                ),
                citation: "CAPCO-ISM-v2022-DEC-§5.1",
                original: token.text.to_string(),
                replacement: entry.replacement.to_owned(),
                confidence: entry.confidence,
                migration_ref: Some(entry.reference),
            }));
        }
        diagnostics
    }
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
            // Skip entries that E007 (deprecated X-shorthand) will pick up.
            // The deprecated migration table is the union of dissem and
            // declass entries; an Unknown that maps to one is not really
            // unrecognized, just deprecated.
            .filter(|t| find_migration(t.text.as_ref()).is_none())
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
    fn capco_rule_set_registers_all_phase3_rules() {
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
        assert!(ids.contains(&"W001"));
        assert_eq!(set.rules().len(), 9);
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
        let diags = lint_portion("(SECRET//NF)");
        // (NF is the portion-form abbreviation; portion markings legitimately
        // use abbreviations, so E001 must not fire on a portion candidate.)
        assert!(
            diags.is_empty(),
            "clean portion should produce no diagnostics, got: {diags:?}"
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
