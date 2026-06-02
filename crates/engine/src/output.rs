// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Output types returned by the engine's synchronous API surface.

use marque_capco::CapcoScheme;
use marque_rules::Diagnostic;
use marque_rules::audit::{AppliedFix, AppliedTextCorrection, AuditLine};
use marque_scheme::{MarkingScheme, ResolvedDocument};
use secrecy::SecretSlice;

use crate::session::SessionMetadata;

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
/// let mut result: LintResult = LintResult::default();
/// result.diagnostics.clear();
/// ```
///
/// `truncated`, `candidates_processed`, and `candidates_total` surface
/// deadline-driven cooperative cancellation. A fully completed pass
/// reports `truncated: false` with `candidates_processed ==
/// candidates_total`. An already-expired deadline returns immediately
/// with `truncated: true` and both counts at `0`. Mid-document expiry
/// produces `truncated: true` with
/// `0 < candidates_processed < candidates_total`.
#[non_exhaustive]
pub struct LintResult<S: MarkingScheme = CapcoScheme> {
    pub diagnostics: Vec<Diagnostic<S>>,
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
    /// Document-scope resolution of the scheme's declared document
    /// artifacts (issue #799). Decoupled from fixing: it is computed on
    /// every completed lint pass — including a fixing-off `lint()` call —
    /// so the resolution-classification is observable through the normal
    /// lint flow. Empty (default) for a scheme that declares no document
    /// artifacts (the CAPCO case) and for a truncated pass (a truncated
    /// lint has no complete document rollup to resolve against).
    pub resolved_document: ResolvedDocument<S>,
}

// Hand-written `Default` / `Clone` bounded only on `S: MarkingScheme` — a
// `#[derive]` would over-constrain with a spurious `S: Default` / `S: Clone`
// bound (the scheme marker is neither), and the generic lint pipeline
// constructs `LintResult { .. ..Default::default() }` without those bounds.
// `Diagnostic<S>` itself hand-writes `Clone` for the same reason; `Vec` /
// `bool` / `usize` carry the rest. (Same rationale as `Diagnostic<S>` and
// `RuleContext<'_, S>`.)
// Manual `Debug` bounded `where S::Canonical: Debug` — a `#[derive(Debug)]`
// would only add `S: Debug` for the type parameter and would not reach the
// `S::Canonical: Debug` bound the `resolved_document` field needs (the
// `Diagnostic<S>` field's own `Debug` is bounded only on `S`). Every scheme
// driven through the engine satisfies `S::Canonical: Debug`.
impl<S: MarkingScheme + core::fmt::Debug> core::fmt::Debug for LintResult<S>
where
    S::Canonical: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LintResult")
            .field("diagnostics", &self.diagnostics)
            .field("truncated", &self.truncated)
            .field("candidates_processed", &self.candidates_processed)
            .field("candidates_total", &self.candidates_total)
            .field("recognized_marking_count", &self.recognized_marking_count)
            .field("resolved_document", &self.resolved_document)
            .finish()
    }
}

impl<S: MarkingScheme> Default for LintResult<S> {
    fn default() -> Self {
        Self {
            diagnostics: Vec::new(),
            truncated: false,
            candidates_processed: 0,
            candidates_total: 0,
            recognized_marking_count: 0,
            resolved_document: ResolvedDocument::default(),
        }
    }
}

// `Clone` adds a `where S::Canonical: Clone` clause that `Default` does not:
// cloning the `resolved_document` field clones each `S::Canonical` derived
// value it carries, so the bound is genuinely needed here (and only here).
// Every scheme driven through the engine pipeline already satisfies it (the
// pipeline block bounds `S::Canonical: Clone`), so no real consumer loses
// the impl.
impl<S: MarkingScheme> Clone for LintResult<S>
where
    S::Canonical: Clone,
{
    fn clone(&self) -> Self {
        Self {
            diagnostics: self.diagnostics.clone(),
            truncated: self.truncated,
            candidates_processed: self.candidates_processed,
            candidates_total: self.candidates_total,
            recognized_marking_count: self.recognized_marking_count,
            resolved_document: self.resolved_document.clone(),
        }
    }
}

impl<S: MarkingScheme> LintResult<S> {
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
    /// carry an actionable payload (`Diagnostic.fix` or
    /// `Diagnostic.text_correction`). A diagnostic at `Fix` severity
    /// with neither populated is not counted, since it cannot produce
    /// an `AppliedFix` or `TextCorrection` downstream.
    ///
    /// Both arms are counted because rules emit either `d.fix` or
    /// `d.text_correction`. The server's response struct ([`marque_server`])
    /// and CLI exit-code summary both depend on `fix_count` matching the
    /// eventual `applied.len()` from `Engine::fix`.
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
pub struct FixResult<S: MarkingScheme = CapcoScheme> {
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
    /// Audit stream. A single [`AuditLine<S>`] stream preserves the
    /// confidence-then-span promotion-order invariant across the
    /// marking-fix channel (`AuditLine::AppliedFix`) and the
    /// text-correction channel (`AuditLine::TextCorrection`). The
    /// renderer projects each line to its NDJSON record type. This is
    /// the sole audit-output channel.
    pub audit_lines: Vec<AuditLine<S>>,
    /// Diagnostics that could not be auto-fixed (below confidence threshold,
    /// or require human judgment).
    pub remaining_diagnostics: Vec<Diagnostic<S>>,
    /// `true` when pass-1 re-parse failed and the engine emitted an
    /// `R002` synthetic diagnostic. When set:
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
    /// Session-level audit metadata (`marque-3.2`, issue #399):
    /// engine/lattice/decoder versions, an integrity seal, the applying
    /// interface, the resolved classifier identity, and an optional
    /// carry-only signature. Surfaces emit
    /// [`SessionMetadata::to_ndjson`] as the first line of a non-empty
    /// audit stream and fold it into the [`crate::SessionRoot`] Merkle
    /// computation, so it is tamper-evident under the session root.
    pub session_metadata: SessionMetadata,
}

impl<S: MarkingScheme> FixResult<S> {
    /// Iterate marking-side audit lines (zero-alloc filter view).
    ///
    /// The sole audit-output channel is [`Self::audit_lines`]: a
    /// sum-type stream (`AuditLine::AppliedFix` for marking fixes,
    /// `AuditLine::TextCorrection` for the text-correction path). This
    /// accessor exposes a marking-fix-only read shape for consumers that
    /// don't need to pattern-match the sum type.
    ///
    /// # Zero-alloc
    ///
    /// Returns `impl Iterator<Item = &AppliedFix<S>>` —
    /// each invocation walks [`Self::audit_lines`] lazily without
    /// allocating an intermediate `Vec`. Callers that need `.len()`
    /// or `.is_empty()` use `.count()` / `.next().is_none()`
    /// respectively (or `Iterator::collect` into a local `Vec` when
    /// the same fixes need to be visited twice).
    #[inline]
    pub fn applied_fixes(&self) -> impl Iterator<Item = &AppliedFix<S>> {
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
    use marque_rules::{Diagnostic, Message, MessageArgs, MessageTemplate, RuleId, Severity};
    use marque_scheme::{AuthoritativeSource, Citation, SectionLetter, SectionRef, Span};

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
        let clean_result: LintResult = LintResult {
            diagnostics: vec![],
            ..Default::default()
        };
        assert!(clean_result.is_clean());
    }

    #[test]
    fn is_clean_returns_false_when_has_diagnostics() {
        let dirty_result: LintResult = LintResult {
            diagnostics: vec![Diagnostic::new(
                // Synthetic test fixture in the reserved `"test"` scheme.
                RuleId::new("test", "synthetic.is-clean-fixture"),
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

    /// The hand-written `Default` (bounded on `S: MarkingScheme`, not the
    /// derive that would impose a spurious `S: Default`) zeroes every field:
    /// empty diagnostics, not truncated, all counts at 0.
    #[test]
    fn default_zeroes_every_field() {
        let result = LintResult::<CapcoScheme>::default();
        assert!(result.diagnostics.is_empty());
        assert!(!result.truncated);
        assert_eq!(result.candidates_processed, 0);
        assert_eq!(result.candidates_total, 0);
        assert_eq!(result.recognized_marking_count, 0);
        assert!(result.resolved_document.is_empty());
    }

    /// The hand-written `Clone` (bounded on `S: MarkingScheme`, mirroring
    /// `Diagnostic<S>`) copies every field; mutating the clone does not
    /// disturb the original (deep copy of the diagnostics vector).
    #[test]
    fn clone_copies_every_field_independently() {
        let original: LintResult = LintResult {
            diagnostics: vec![Diagnostic::new(
                RuleId::new("test", "synthetic.clone-fixture"),
                Severity::Warn,
                Span::new(3, 7),
                stub_message(),
                stub_citation(),
                None,
            )],
            truncated: true,
            candidates_processed: 4,
            candidates_total: 9,
            recognized_marking_count: 2,
            ..Default::default()
        };

        let mut cloned = original.clone();
        assert_eq!(cloned.diagnostics.len(), original.diagnostics.len());
        assert!(cloned.truncated);
        assert_eq!(cloned.candidates_processed, 4);
        assert_eq!(cloned.candidates_total, 9);
        assert_eq!(cloned.recognized_marking_count, 2);

        // The clone owns its own diagnostics vector — clearing it leaves the
        // original intact.
        cloned.diagnostics.clear();
        assert_eq!(original.diagnostics.len(), 1);
    }

    #[test]
    fn info_count_isolates_info_from_error_and_warn() {
        // `Severity::Info` diagnostics count in `info_count()`
        // only — they do NOT contribute to `error_count()` or
        // `warn_count()`. Critical because the CLI has two non-zero
        // exit gates: `error_count() > 0 || fix_count() > 0` maps to
        // EX_DIAG_ERROR (exit 1), and `warn_count() > 0` maps to
        // EX_DIAG_WARN (exit 2). Info must land in neither bucket so
        // that a rule configured at Info keeps the CLI exit code at
        // 0 — that's the whole point of the severity between Off and
        // Warn.
        let result: LintResult = LintResult {
            diagnostics: vec![
                Diagnostic::new(
                    RuleId::new("capco", "portion.sci.unpublished-custom-control"),
                    Severity::Info,
                    Span::new(0, 0),
                    stub_message(),
                    stub_citation(),
                    None,
                ),
                Diagnostic::new(
                    RuleId::new("capco", "portion.sci.unpublished-custom-control"),
                    Severity::Info,
                    Span::new(0, 0),
                    stub_message(),
                    stub_citation(),
                    None,
                ),
                Diagnostic::new(
                    RuleId::new("capco", "page.dissem.non-ic-dissem-in-classified-banner"),
                    Severity::Warn,
                    Span::new(0, 0),
                    stub_message(),
                    stub_citation(),
                    None,
                ),
                Diagnostic::new(
                    // Synthetic test fixture to exercise error_count().
                    RuleId::new("test", "synthetic.error-count-fixture"),
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
