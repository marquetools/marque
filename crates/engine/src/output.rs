// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Output types returned by the engine's synchronous API surface.

use marque_rules::{AppliedFix, Diagnostic};

/// Result of a lint pass — diagnostics without source modification.
///
/// `#[non_exhaustive]` so future lint-time observations (per-rule
/// timing histograms, decoder posterior quartiles, etc.) can join
/// without breaking callers that brace-construct. External callers
/// MUST construct via `Default::default()` plus public field
/// assignment (struct-update syntax is only allowed in-crate):
///
/// ```
/// use marque_engine::LintResult;
/// let mut result = LintResult::default();
/// result.diagnostics.clear();
/// ```
///
/// Spec 005 added `truncated`, `candidates_processed`, and
/// `candidates_total` to surface deadline-driven cooperative
/// cancellation. On a fully-completed pass `truncated` is `false`
/// and `candidates_processed == candidates_total`. On an
/// already-expired deadline the pass returns immediately with
/// `truncated: true` and both counts at `0`. Mid-document expiry
/// produces `truncated: true` with `0 < candidates_processed <
/// candidates_total`.
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct LintResult {
    pub diagnostics: Vec<Diagnostic>,
    /// `true` when the lint pass aborted before processing every
    /// scanner-emitted candidate due to deadline expiry. The
    /// `diagnostics` vector contains every diagnostic produced from
    /// candidates that *were* processed before the abort. Spec §R3.
    pub truncated: bool,
    /// Number of scanner-emitted candidates the engine processed
    /// before returning. On a non-truncated pass equals
    /// `candidates_total`. Set during Phase 2 wiring; Phase 1 leaves
    /// this `0` for back-compat with callers that already exist.
    pub candidates_processed: usize,
    /// Total number of scanner-emitted candidates (the
    /// post-scanner, pre-rule-loop count). Populated from the
    /// scanner output regardless of whether the pass completed.
    /// Phase 1 leaves this `0`; Phase 2 wires it.
    pub candidates_total: usize,
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

    /// Number of diagnostics at `Severity::Info` — visible, but not
    /// counted toward either the error/fix exit gate
    /// (`EX_DIAG_ERROR`) or the warn exit gate (`EX_DIAG_WARN`). See
    /// `Severity` docs for the tonal distinction (`Info` = "probably
    /// intentional, worth surfacing"; `Warn` = "this might be wrong").
    pub fn info_count(&self) -> usize {
        use marque_rules::Severity;
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Info)
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
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_core::Span;
    use marque_rules::{Diagnostic, RuleId, Severity};

    #[test]
    fn is_clean_returns_true_when_no_diagnostics() {
        let clean_result = LintResult {
            diagnostics: vec![],
            ..Default::default()
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
            ..Default::default()
        };
        assert!(!dirty_result.is_clean());
    }

    #[test]
    fn info_count_isolates_info_from_error_and_warn() {
        // T035c-2: `Severity::Info` diagnostics count in `info_count()`
        // only — they do NOT contribute to `error_count()` or
        // `warn_count()`. Critical because the CLI has two non-zero
        // exit gates: `error_count() > 0 || fix_count() > 0` maps to
        // EX_DIAG_ERROR (exit 1), and `warn_count() > 0` maps to
        // EX_DIAG_WARN (exit 2). Info must land in neither bucket so
        // that a rule configured at Info keeps the CLI exit code at
        // 0 — that's the whole point of the severity between Off and
        // Warn.
        let result = LintResult {
            diagnostics: vec![
                Diagnostic::new(
                    RuleId::new("E034"),
                    Severity::Info,
                    Span::new(0, 0),
                    "info one",
                    "test",
                    None,
                ),
                Diagnostic::new(
                    RuleId::new("E034"),
                    Severity::Info,
                    Span::new(0, 0),
                    "info two",
                    "test",
                    None,
                ),
                Diagnostic::new(
                    RuleId::new("W003"),
                    Severity::Warn,
                    Span::new(0, 0),
                    "warn",
                    "test",
                    None,
                ),
                Diagnostic::new(
                    RuleId::new("E001"),
                    Severity::Error,
                    Span::new(0, 0),
                    "err",
                    "test",
                    None,
                ),
            ],
            ..Default::default()
        };
        assert_eq!(result.info_count(), 2);
        assert_eq!(result.warn_count(), 1);
        assert_eq!(result.error_count(), 1);
        assert_eq!(result.fix_count(), 0);
    }
}
