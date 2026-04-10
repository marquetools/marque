//! CAPCO rule implementations — Layer 2 diagnostic intelligence.
//!
//! Each rule uses Layer 1 schema predicates (from generated/validators.rs, once wired)
//! to detect violations, then produces enriched diagnostics with fixes and confidence.
//!
//! Rule IDs follow the convention: E### = error, W### = warning, C### = correction.

use marque_ism::{IsmAttributes, Span};
use marque_rules::{AuditRecord, Diagnostic, Fix, Rule, RuleContext, RuleId, RuleSet, Severity};
use std::time::SystemTime;

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
                Box::new(UsaTrigraphOrderRule),
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
        RuleId("E001")
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
        // Check dissem controls for portion-form abbreviations in banner context.
        let mut diagnostics = Vec::new();
        for control in attrs.dissem_controls.iter() {
            if let Some(full) = expand_dissem_abbreviation(control) {
                diagnostics.push(make_fix_diagnostic(
                    self.id(),
                    self.default_severity(),
                    Span::new(0, 0), // TODO: wire actual span from parsed marking
                    format!("banner uses abbreviated dissem control {control:?}; use {full:?}"),
                    control.clone(),
                    full.to_owned(),
                    1.0,
                    Some("CAPCO-2023-§3.2"),
                ));
            }
        }
        diagnostics
    }
}

/// Expand known portion-form abbreviations to their full banner forms.
fn expand_dissem_abbreviation(s: &str) -> Option<&'static str> {
    match s {
        "NF" => Some("NOFORN"),
        "OC" => Some("ORCON"),
        "IMC" => Some("IMCON"),
        "DSEN" => Some("DEA SENSITIVE"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Rule: E004 — Missing USA in REL TO trigraph list
// ---------------------------------------------------------------------------

struct MissingUsaTrigraphRule;

impl Rule for MissingUsaTrigraphRule {
    fn id(&self) -> RuleId {
        RuleId("E004")
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
        if attrs.rel_to.contains(&marque_ism::Trigraph::USA) {
            return vec![];
        }

        let current = attrs
            .rel_to
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let fixed = format!("USA, {current}");

        vec![make_fix_diagnostic(
            self.id(),
            self.default_severity(),
            Span::new(0, 0),
            "REL TO list missing required USA trigraph".to_owned(),
            current,
            fixed,
            1.0,
            Some("CAPCO-2023-§4.1"),
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E005 — USA not first in REL TO list
// ---------------------------------------------------------------------------

struct UsaTrigraphOrderRule;

impl Rule for UsaTrigraphOrderRule {
    fn id(&self) -> RuleId {
        RuleId("E005")
    }
    fn name(&self) -> &'static str {
        "usa-trigraph-order"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if attrs.rel_to.len() < 2 {
            return vec![];
        }
        let first = &attrs.rel_to[0];
        if *first == marque_ism::Trigraph::USA {
            return vec![];
        }

        let current: Vec<&str> = attrs.rel_to.iter().map(|t| t.as_str()).collect();
        let mut fixed = current.clone();
        fixed.retain(|&t| t != "USA");
        fixed.insert(0, "USA");

        vec![make_fix_diagnostic(
            self.id(),
            self.default_severity(),
            Span::new(0, 0),
            "USA must be the first trigraph in REL TO list".to_owned(),
            current.join(", "),
            fixed.join(", "),
            1.0,
            Some("CAPCO-2023-§4.1"),
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E003 — Wrong separator count (should always be exactly `//`)
// ---------------------------------------------------------------------------

struct SeparatorCountRule;

impl Rule for SeparatorCountRule {
    fn id(&self) -> RuleId {
        RuleId("E003")
    }
    fn name(&self) -> &'static str {
        "separator-count"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, _attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        // TODO: wire raw source text into rule context so we can inspect
        // literal separator characters. Currently IsmAttributes is post-parse.
        vec![]
    }
}

// ---------------------------------------------------------------------------
// Rule: E006 — Declassification marking in banner (should be in CAB)
// ---------------------------------------------------------------------------

struct DeclassifyInBannerRule;

impl Rule for DeclassifyInBannerRule {
    fn id(&self) -> RuleId {
        RuleId("E006")
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

        vec![Diagnostic {
            rule: self.id(),
            severity: self.default_severity(),
            span: Span::new(0, 0),
            message: "declassification marking belongs in Classification Authority Block \
                      (Declassify On:), not in the banner — remove from banner and add to CAB"
                .to_owned(),
            fix: None, // Fix requires document-level context (CAB presence/location);
                       // engine handles this as a multi-span fix.
        }]
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn make_fix_diagnostic(
    rule: RuleId,
    severity: Severity,
    span: Span,
    message: String,
    original: String,
    replacement: String,
    confidence: f32,
    migration_ref: Option<&'static str>,
) -> Diagnostic {
    Diagnostic {
        rule: rule.clone(),
        severity,
        span,
        message,
        fix: Some(Fix {
            span,
            replacement: replacement.clone(),
            confidence,
            audit: AuditRecord {
                rule,
                original,
                replacement,
                confidence,
                timestamp: SystemTime::now(),
                classifier_id: None, // injected by engine from config
            },
            migration_ref,
        }),
    }
}
