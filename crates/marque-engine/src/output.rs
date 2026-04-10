//! Output types returned by the engine's synchronous API surface.

use marque_rules::{AuditRecord, Diagnostic};

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

    pub fn fix_count(&self) -> usize {
        use marque_rules::Severity;
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Fix)
            .count()
    }
}

/// Result of a fix pass — modified source and audit trail.
#[derive(Debug)]
pub struct FixResult {
    /// Fixed source bytes (UTF-8).
    pub source: Vec<u8>,
    /// Audit records for every fix that was applied.
    pub applied: Vec<AuditRecord>,
    /// Diagnostics that could not be auto-fixed (below confidence threshold,
    /// or require human judgment).
    pub remaining_diagnostics: Vec<Diagnostic>,
}
