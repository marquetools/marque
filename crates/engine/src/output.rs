// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Output types returned by the engine's synchronous API surface.

use marque_capco::CapcoScheme;
use marque_rules::Diagnostic;
use marque_rules::audit::{AppliedFix, AppliedTextCorrection, AuditLine};
use secrecy::SecretSlice;

/// Result of a lint pass — diagnostics without source modification.
///
/// `#[non_exhaustive]` ensures future lint-time observations (per-rule
/// timing histograms, decoder posterior quartiles, etc.) can be added
/// without further breaking downstream callers. Adding the attribute
/// itself in spec 005 IS a one-time breaking change for external
/// callers that previously brace-constructed or exhaustively
/// pattern-matched `LintResult`; from this version on, external
/// callers MUST construct via `Default::default()` plus public field
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
/// cancellation.
///
/// **Phase 1 status (current build):** deadline enforcement is not
/// wired yet. Lint passes run to completion regardless of
/// `LintOptions::deadline`, so `truncated` is always `false` and
/// both candidate-count fields are always `0`. The semantics below
/// describe the Phase 2 behavior that lands in tasks T007–T009.
///
/// Once Phase 2 wiring lands: a fully completed pass reports
/// `truncated: false` with `candidates_processed ==
/// candidates_total`. An already-expired deadline returns
/// immediately with `truncated: true` and both counts at `0`.
/// Mid-document expiry produces `truncated: true` with
/// `0 < candidates_processed < candidates_total`.
#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub struct LintResult {
    pub diagnostics: Vec<Diagnostic<CapcoScheme>>,
    /// `true` when the lint pass aborted before processing every
    /// scanner-emitted candidate due to deadline expiry. The
    /// `diagnostics` vector contains every diagnostic produced from
    /// candidates that *were* processed before the abort. Spec §R3.
    pub truncated: bool,
    /// Number of scanner-emitted candidates the engine started
    /// processing past the per-candidate deadline check before
    /// returning. Counted at the top of each candidate iteration
    /// (after the deadline check, before any per-candidate work),
    /// so it includes every iteration that survived the cancellation
    /// boundary — fully-rule-evaluated candidates AND structural
    /// "early-continue" candidates such as page-break resets,
    /// empty-span skips, and ambiguous-recognition skips. This
    /// definition is what makes `candidates_processed ==
    /// candidates_total` hold on a non-truncated pass; if the
    /// counter only fired on the rule-loop completion path,
    /// page-break candidates would silently break that invariant
    /// on multi-page documents. On a truncated pass,
    /// `candidates_processed < candidates_total` and the delta is
    /// the count of candidates the deadline preempted.
    pub candidates_processed: usize,
    /// Total number of scanner-emitted candidates (the
    /// post-scanner, pre-rule-loop count). Populated from the
    /// scanner output regardless of whether the pass completed.
    pub candidates_total: usize,
    /// Number of candidates that produced a `Parsed::Unambiguous`
    /// recognition (i.e., the recognizer returned a marking, not
    /// `Ambiguous`). Distinct from `candidates_processed`, which
    /// counts every iteration the engine started — including
    /// ambiguous-recognition skips and page-break resets. Used by
    /// `TwoPassFixer`'s R002 trigger to detect "pass-1 splice
    /// destroyed marking shape" without depending on the issue
    /// #433 deferred `parsed_markings` cache, whose population is
    /// gated on `d.fix.is_some()` and so does not reflect every
    /// recognized marking.
    pub recognized_marking_count: usize,
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

    /// Number of diagnostics at `Severity::Suggest` — the
    /// suggest-don't-fix channel. Visible in lint output but the
    /// engine never auto-applies the attached fix (issue #235 / #186
    /// PR-3). Like `Info`, contributes to neither exit-code gate.
    pub fn suggest_count(&self) -> usize {
        use marque_rules::Severity;
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Suggest)
            .count()
    }

    /// Number of diagnostics that are configured at `Severity::Fix` AND
    /// carry a fix payload (either legacy [`FixProposal`] or new
    /// [`marque_rules::FixIntent`]). A diagnostic at `Fix` severity but
    /// with neither `fix` nor `fix_intent` populated is not counted,
    /// since it cannot produce an `AppliedFix` downstream.
    ///
    /// Both arms are counted to keep `fix_count` honest across the
    /// PR 3c.B Commit 2–9 transition: in Commit 2 only `d.fix` ever
    /// fires, but Commit 3+ migrates rules to emit `fix_intent`. The
    /// server's response struct ([`marque_server`]) and CLI exit-code
    /// summary both depend on `fix_count` matching the eventual
    /// `applied.len()` from `Engine::fix`.
    pub fn fix_count(&self) -> usize {
        use marque_rules::Severity;
        self.diagnostics
            .iter()
            .filter(|d| {
                d.severity == Severity::Fix && (d.fix.is_some() || d.text_correction.is_some())
            })
            .count()
    }
}

/// Result of a fix pass — modified source and audit trail.
///
/// `#[non_exhaustive]` so future audit-stream additions land
/// additively without breaking external brace constructions.
#[non_exhaustive]
#[derive(Debug)]
pub struct FixResult {
    /// Fixed source bytes. Preserves UTF-8 validity: the input is UTF-8, and every
    /// replacement is a valid UTF-8 `String`, so the result is always valid UTF-8.
    ///
    /// Wrapped in [`secrecy::SecretSlice<u8>`] per Constitution Principle II
    /// (Marque-owned content-bearing buffers wipe on drop). Readouts go
    /// through [`secrecy::ExposeSecret::expose_secret`] which returns
    /// `&[u8]` — every readout site is grep-able for security review. The
    /// `Debug` impl auto-redacts to `SecretBox<[u8]>([REDACTED])`, closing
    /// the accidental-log channel. Marque's responsibility ends at the
    /// `SecretSlice` boundary: a caller that clones the inner bytes
    /// (e.g. via `expose_secret().to_vec()` or `String::from_utf8`)
    /// owns the clone's lifecycle.
    pub source: SecretSlice<u8>,
    /// **`marque-1.0` audit stream.** Per PR 3c.2.D / PM-D-8 a
    /// single [`AuditLine<S>`] stream preserves the FR-016
    /// promotion-order invariant across the marking-fix channel
    /// (`AuditLine::AppliedFix`) and the text-correction channel
    /// (`AuditLine::TextCorrection`). The renderer projects each
    /// line to its NDJSON record type.
    ///
    /// Post PR 3c.2.D (atomic cutover) this is the sole audit-output
    /// channel. The pre-cutover `applied: Vec<AppliedFix<S>>` v1
    /// stream retired with the `marque-mvp-3 → marque-1.0` schema
    /// flip.
    pub audit_lines: Vec<AuditLine<CapcoScheme>>,
    /// Diagnostics that could not be auto-fixed (below confidence threshold,
    /// or require human judgment).
    pub remaining_diagnostics: Vec<Diagnostic<CapcoScheme>>,
    /// `true` when pass-1 re-parse failed and the engine emitted an
    /// `R002` synthetic diagnostic (PR 7b, FR-024). When set:
    ///
    /// - [`Self::source`] holds the post-pass-1 buffer ONLY. Pass-2
    ///   never ran, so any pass-2 fixes that would have applied are
    ///   absent from [`Self::audit_lines`].
    /// - [`Self::remaining_diagnostics`] contains the R002 diagnostic
    ///   (and any other unfixed pass-1 diagnostics).
    /// - WASM / IDE consumers MUST test this field BEFORE applying
    ///   [`Self::source`] to the user's editor; splicing the
    ///   partial buffer in silently is destructive without consent.
    /// - The CLI exit-code precedence chain
    ///   (`EX_R002_PARTIAL > EX_DIAG_ERROR > EX_DIAG_WARN > EX_OK`)
    ///   maps this to exit code `3`. See `marque/src/main.rs::merge_exit_code`.
    ///
    /// The flag exists so consumers can detect R002 without scanning
    /// the diagnostic stream (D1's "detectable without NDJSON
    /// parsing" binding constraint). A second synthetic-error
    /// boolean lands cleanly on the same surface; a third synthetic
    /// signal would suggest collapsing to a `partial_state:
    /// PartialState` enum.
    pub r002_fired: bool,
}

impl FixResult {
    /// Iterate marking-side audit lines (zero-alloc filter view).
    ///
    /// Post PR 3c.2.D (atomic cutover) the sole audit-output channel
    /// is [`Self::audit_lines`]: a sum-type stream
    /// (`AuditLine::AppliedFix` for marking fixes,
    /// `AuditLine::TextCorrection` for the C001 / E006 text-
    /// correction path). This accessor preserves the pre-cutover
    /// read shape for consumers that only need marking fixes, so
    /// the migration doesn't force every assertion site to pattern-
    /// match the sum type.
    ///
    /// # Zero-alloc (PR 3c.2.D fixup F-3)
    ///
    /// Returns `impl Iterator<Item = &AppliedFix<CapcoScheme>>` —
    /// each invocation walks [`Self::audit_lines`] lazily without
    /// allocating an intermediate `Vec`. Callers that need `.len()`
    /// or `.is_empty()` use `.count()` / `.next().is_none()`
    /// respectively (or `Iterator::collect` into a local `Vec` when
    /// the same fixes need to be visited twice). The pre-fixup
    /// `Vec<&AppliedFix>` shape allocated on every call, which the
    /// `crates/engine/benches/fix_latency.rs` hot path called four
    /// times in one function body — exactly the kind of post-
    /// stabilization breaking signature change pre-users freedom
    /// permits.
    #[inline]
    pub fn applied_fixes(&self) -> impl Iterator<Item = &AppliedFix<CapcoScheme>> {
        self.audit_lines.iter().filter_map(|line| match line {
            AuditLine::AppliedFix(f) => Some(f),
            _ => None,
        })
    }

    /// Iterate text-correction audit lines (zero-alloc filter view).
    ///
    /// Mirrors [`Self::applied_fixes`] for the
    /// `AuditLine::TextCorrection` arm — C001 corrections-map fixes
    /// and the E006-shaped deprecation-migration path. Same zero-
    /// alloc property; same `.count()` / `.next().is_none()` idiom
    /// for length / emptiness checks.
    #[inline]
    pub fn applied_text_corrections(&self) -> impl Iterator<Item = &AppliedTextCorrection> {
        self.audit_lines.iter().filter_map(|line| match line {
            AuditLine::TextCorrection(tc) => Some(tc),
            _ => None,
        })
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_rules::{
        AuthoritativeSource, Citation, Diagnostic, Message, MessageArgs, MessageTemplate, RuleId,
        SectionLetter, SectionRef, Severity,
    };
    use marque_scheme::Span;

    /// Test-fixture `Message` stub mirroring the helper in
    /// `engine.rs::tests`. Used by `LintResult` shape tests that do
    /// not exercise message content.
    #[inline]
    fn stub_message() -> Message {
        Message::new(MessageTemplate::UnrecognizedToken, MessageArgs::default())
    }

    /// Test-fixture `Citation` stub mirroring the helper in
    /// `engine.rs::tests`. Uses `AuthoritativeSource::EngineInternal`
    /// (non-CAPCO sentinel per PM-C-4) so citation-lint skips this
    /// entry — these stubs are test fixtures, not real CAPCO citations.
    #[inline]
    fn stub_citation() -> Citation {
        Citation::new(
            AuthoritativeSource::EngineInternal,
            SectionRef::new(SectionLetter::A),
            core::num::NonZeroU16::new(1).unwrap(),
        )
    }

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
                stub_message(),
                stub_citation(),
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
                    RuleId::new("W034"),
                    Severity::Info,
                    Span::new(0, 0),
                    stub_message(),
                    stub_citation(),
                    None,
                ),
                Diagnostic::new(
                    RuleId::new("W034"),
                    Severity::Info,
                    Span::new(0, 0),
                    stub_message(),
                    stub_citation(),
                    None,
                ),
                Diagnostic::new(
                    RuleId::new("W003"),
                    Severity::Warn,
                    Span::new(0, 0),
                    stub_message(),
                    stub_citation(),
                    None,
                ),
                Diagnostic::new(
                    RuleId::new("E001"),
                    Severity::Error,
                    Span::new(0, 0),
                    stub_message(),
                    stub_citation(),
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
