//! CAPCO rule implementations — Layer 2 diagnostic intelligence.
//!
//! Each rule uses Layer 1 schema predicates (from generated/validators.rs, once wired)
//! to detect violations, then produces enriched diagnostics with fixes and confidence.
//!
//! Rule IDs follow the convention: E### = error, W### = warning, C### = correction.
//! Assignments per spec tasks.md:
//!   E001 = banner abbreviation (T030)
//!   E002 = REL TO missing USA trigraph (T031)
//!   E003 = misordered banner blocks (T032, Phase 3)
//!   E004 = separator-count normalization (T033)
//!   E005 = declassification in banner (T034)
//!   E006 = deprecated dissem control (T035, Phase 3)
//!   E007 = X-shorthand declass date (T036, Phase 3)
//!   E008 = unrecognized token (T037, Phase 3)
//!   W001 = deprecated marking warning (T038, Phase 3)
//!   C001 = corrections-map typo (T058, Phase 5)

use marque_ism::{IsmAttributes, Span};
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
                Box::new(SeparatorCountRule),
                Box::new(DeclassifyInBannerRule),
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
        for control in attrs.dissem_controls.iter() {
            if let Some(full) = expand_dissem_abbreviation(control) {
                let abbrev = control.as_str().to_owned();
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    span: Span::new(0, 0), // TODO: wire actual span (Phase 3)
                    message: format!(
                        "banner uses abbreviated dissem control {abbrev:?}; use {full:?}"
                    ),
                    citation: "CAPCO-ISM-v2022-DEC-§3.2",
                    original: abbrev,
                    replacement: full.to_owned(),
                    confidence: 1.0,
                    migration_ref: Some("CAPCO-2023-§3.2"),
                }));
            }
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

        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: self.id(),
            severity: self.default_severity(),
            span: Span::new(0, 0), // TODO: wire actual span (Phase 3)
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

    fn check(&self, _attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        // Requires raw source text in rule context — not available until Phase 3
        // wires per-token spans. IsmAttributes is post-parse and discards separators.
        vec![]
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
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;
        if ctx.marking_type != MarkingType::Banner {
            return vec![];
        }
        if attrs.declassify_on.is_none() {
            return vec![];
        }

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            Span::new(0, 0), // TODO: wire actual span (Phase 3)
            "declassification marking belongs in Classification Authority Block \
             (Declassify On:), not in the banner — remove from banner and add to CAB",
            "CAPCO-ISM-v2022-DEC-§6.1",
            None, // Fix requires document-level context (multi-span);
                  // confidence 0.55 per T034 — suggestion only.
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
        FixSource::BuiltinRule,
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
