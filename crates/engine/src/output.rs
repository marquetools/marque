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
    use marque_rules::{Diagnostic, RuleId, Severity};

    #[test]
    fn is_clean_returns_true_when_no_diagnostics() {
        let clean_result = LintResult {
            diagnostics: vec![],
        };
        assert!(clean_result.is_clean());
    }

    #[test]
    fn is_clean_returns_false_when_has_diagnostics() {
        let dirty_result = LintResult {
            diagnostics: vec![Diagnostic::new(
                RuleId::new("E001"),
                Severity::Error,
                Span::new(0, 0),
                "test",
                "test",
                None,
            )],
        };
        assert!(!dirty_result.is_clean());
    }
}
