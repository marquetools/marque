//! `Engine` — the configured, ready-to-run pipeline.

use marque_config::Config;
use marque_rules::RuleSet;
use crate::output::{FixResult, LintResult};

/// A configured engine instance. Cheap to clone; rule sets are behind `Arc`.
pub struct Engine {
    config: Config,
    rule_sets: Vec<Box<dyn RuleSet>>,
}

impl Engine {
    /// Create a new engine with the given configuration and rule sets.
    pub fn new(config: Config, rule_sets: Vec<Box<dyn RuleSet>>) -> Self {
        Self { config, rule_sets }
    }

    /// Lint a UTF-8 text buffer. Returns diagnostics without modifying input.
    pub fn lint(&self, source: &[u8]) -> LintResult {
        use marque_core::{Scanner, Parser};
        use marque_capco::CapcoTokenSet;
        use marque_rules::RuleContext;

        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidates = Scanner::scan(source);

        let mut diagnostics = Vec::new();

        for candidate in &candidates {
            let Ok(parsed) = parser.parse(candidate, source) else { continue };
            let ctx = RuleContext {
                marking_type: candidate.kind,
                zone: marque_core::span::Zone::Body,
                document_position: marque_core::span::DocumentPosition::Body,
                paragraph_index: 0,
            };
            for rule_set in &self.rule_sets {
                for rule in rule_set.rules() {
                    let mut diags = rule.check(&parsed.attrs, &ctx);
                    // Inject classifier_id into audit records from config.
                    for diag in &mut diags {
                        if let Some(fix) = &mut diag.fix {
                            fix.audit.classifier_id =
                                self.config.user.classifier_id.clone();
                        }
                    }
                    diagnostics.extend(diags);
                }
            }
        }

        LintResult { diagnostics }
    }

    /// Lint and apply fixes. Returns fixed source and audit log.
    pub fn fix(&self, source: &[u8]) -> FixResult {
        let lint = self.lint(source);

        // Collect fixes above the configured confidence threshold.
        // Apply in reverse span order to preserve offsets.
        let threshold = 0.9_f32; // TODO: read from config
        let mut fixes: Vec<_> = lint.diagnostics.iter()
            .filter_map(|d| d.fix.as_ref())
            .filter(|f| f.confidence >= threshold)
            .collect();

        fixes.sort_by(|a, b| b.span.start.cmp(&a.span.start));

        let mut output = source.to_vec();
        let mut applied = Vec::new();

        for fix in fixes {
            output.splice(fix.span.start..fix.span.end, fix.replacement.bytes());
            applied.push(fix.audit.clone());
        }

        FixResult {
            source: output,
            applied,
            remaining_diagnostics: lint.diagnostics,
        }
    }
}
