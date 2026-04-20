// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Output types returned by the engine's synchronous API surface.

use marque_rules::{AppliedFix, Diagnostic};

/// Result of a lint pass — diagnostics without source modification.
#[derive(Debug, Default)]
pub struct LintResult {
    pub diagnostics: Vec<Diagnostic>,
}

impl LintResult {
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn error_count(&self) -> usize {
        use marque_rules::Severity;
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    pub fn warn_count(&self) -> usize {
        use marque_rules::Severity;
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warn)
            .count()
    }

    /// Number of diagnostics that are configured at `Severity::Fix` AND
    /// carry an actual `FixProposal`. A diagnostic at `Fix` severity but
    /// with `fix: None` is not counted, since it cannot produce an
    /// `AppliedFix` downstream.
    pub fn fix_count(&self) -> usize {
        use marque_rules::Severity;
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Fix && d.fix.is_some())
            .count()
    }
}

/// Result of a fix pass — modified source and audit trail.
#[derive(Debug)]
pub struct FixResult {
    /// Fixed source bytes. Preserves UTF-8 validity: the input is UTF-8, and every
    /// replacement is a valid UTF-8 `String`, so the result is always valid UTF-8.
    pub source: Vec<u8>,
    /// Audit records for every fix that was applied.
    pub applied: Vec<AppliedFix>,
    /// Diagnostics that could not be auto-fixed (below confidence threshold,
    /// or require human judgment).
    pub remaining_diagnostics: Vec<Diagnostic>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use marque_core::Span;
    use marque_rules::{FixProposal, FixSource, RuleId, Severity};

    fn dummy_diagnostic(severity: Severity, with_fix: bool) -> Diagnostic {
        let fix = if with_fix {
            Some(FixProposal::new(
                RuleId::new("E001"),
                FixSource::BuiltinRule,
                Span::new(0, 0),
                "",
                "",
                1.0,
                None,
            ))
        } else {
            None
        };
        Diagnostic::new(
            RuleId::new("E001"),
            severity,
            Span::new(0, 0),
            "test diagnostic",
            "TEST-1",
            fix,
        )
    }

    #[test]
    fn lint_result_is_clean_when_empty() {
        let result = LintResult::default();
        assert!(result.is_clean());
        assert_eq!(result.error_count(), 0);
        assert_eq!(result.warn_count(), 0);
        assert_eq!(result.fix_count(), 0);
    }

    #[test]
    fn lint_result_counts_severities_correctly() {
        let result = LintResult {
            diagnostics: vec![
                dummy_diagnostic(Severity::Error, false),
                dummy_diagnostic(Severity::Error, true),
                dummy_diagnostic(Severity::Warn, false),
                dummy_diagnostic(Severity::Fix, true),
                dummy_diagnostic(Severity::Fix, false), // Should not be counted in fix_count
            ],
        };

        assert!(!result.is_clean());
        assert_eq!(result.error_count(), 2);
        assert_eq!(result.warn_count(), 1);
        assert_eq!(result.fix_count(), 1); // Only Severity::Fix WITH a fix
    }
}
