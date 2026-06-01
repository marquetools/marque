// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Object-safe erasure shim for heterogeneous scheme co-residence.
//!
//! [`MarkingScheme`] has associated types (`Marking`, `Canonical`, `Token`),
//! so it is **not** object-safe and a `Vec<Engine<S>>` cannot hold engines
//! over different schemes. [`ErasedEngine`] is the co-residence seam: it
//! erases the *output* (bytes in, grammar-tagged scheme-agnostic results
//! out), not the engine itself, and the concrete `S` re-emerges only inside
//! the blanket impl. A heterogeneous registry holds `Box<dyn ErasedEngine>`
//! (see [`crate::MultiGrammarEngine`]).
//!
//! # Subset of `contracts/multi-scheme.md` C2
//!
//! The C2 end-state trait also declares `resolve_erased` and `claims`, and
//! pairs the registry with a `CoherenceRegistry`. Those depend on the
//! scope-`resolve` surface, the `Claim` ownership-routing type, and Phase-E
//! coherence rules — none of which exist yet. This trait ships the subset
//! whose surfaces exist today: `grammar_id` + `lint_erased` + `fix_erased`.
//! The remaining methods land with their surfaces.

use marque_rules::audit::AuditLine;
use marque_rules::{ConstraintBridge, Diagnostic, Message, RuleId, Severity, TextCorrection};
use marque_scheme::{Citation, InputContext, MarkingScheme, Recognizer, Span, TokenId, Vocabulary};

use crate::audit_render::audit_line_to_ndjson;
use crate::engine::{Engine, FixMode};
use crate::options::LintOptions;
use crate::output::{FixResult, LintResult};
use crate::session::SessionMetadata;

/// A grammar-erased projection of [`Diagnostic<S>`] — every field except the
/// scheme-typed `fix: Option<FixIntent<S>>`, which the erased lint surface
/// does not render. The presence of a typed fix survives as [`Self::has_fix`].
///
/// `#[non_exhaustive]` so future [`Diagnostic`] fields land additively.
#[non_exhaustive]
#[derive(Debug)]
pub struct ErasedDiagnostic {
    /// 2-tuple rule identity (`scheme`, `predicate_id`).
    pub rule: RuleId,
    pub severity: Severity,
    pub span: Span,
    pub candidate_span: Option<Span>,
    pub message: Message,
    pub citation: Citation,
    pub text_correction: Option<TextCorrection>,
    /// Decoder-recognized canonical bytes (moved verbatim from the typed
    /// diagnostic — a Principle II ownership transfer, not a readout-clone).
    pub recognized_canonical: Option<secrecy::SecretSlice<u8>>,
    /// Whether the typed [`Diagnostic`] carried a `FixIntent<S>`. The intent
    /// itself is scheme-typed and stays on the typed `Engine<S>` fix path;
    /// the erased lint surface only needs to know it existed.
    pub has_fix: bool,
}

impl ErasedDiagnostic {
    /// Project an owned [`Diagnostic<S>`] to its scheme-agnostic form. Consumes
    /// the diagnostic so the content-bearing `recognized_canonical` buffer is
    /// **moved**, not duplicated (Constitution Principle II).
    pub fn from_typed<S: MarkingScheme>(d: Diagnostic<S>) -> Self {
        let has_fix = d.fix.is_some();
        Self {
            rule: d.rule,
            severity: d.severity,
            span: d.span,
            candidate_span: d.candidate_span,
            message: d.message,
            citation: d.citation,
            text_correction: d.text_correction,
            recognized_canonical: d.recognized_canonical,
            has_fix,
        }
    }
}

/// Grammar-tagged, scheme-agnostic lint result. Mirrors [`LintResult<S>`]'s
/// scheme-agnostic fields and adds the grammar tag.
///
/// # Tag placement
///
/// The tag lives at the *result* level, not per-diagnostic: every diagnostic
/// from one [`ErasedEngine::lint_erased`] call comes from one scheme, so a
/// per-diagnostic tag would be N copies of the same `&'static str`. (A
/// rule-emitted diagnostic's `rule.scheme()` already names its grammar, but
/// engine-minted sentinels use `scheme = "engine"`, so `rule.scheme()` is not
/// a reliable grammar tag — the result-level tag is.)
///
/// `#[non_exhaustive]` so future [`LintResult`] fields land additively.
#[non_exhaustive]
#[derive(Debug)]
pub struct ErasedLintResult {
    /// The scheme that produced every diagnostic here (`scheme_id()`).
    pub grammar_id: &'static str,
    pub diagnostics: Vec<ErasedDiagnostic>,
    pub truncated: bool,
    pub candidates_processed: usize,
    pub candidates_total: usize,
    pub recognized_marking_count: usize,
}

impl ErasedLintResult {
    /// Project an owned [`LintResult<S>`] under its grammar tag.
    pub fn from_typed<S: MarkingScheme>(grammar_id: &'static str, typed: LintResult<S>) -> Self {
        Self {
            grammar_id,
            diagnostics: typed
                .diagnostics
                .into_iter()
                .map(ErasedDiagnostic::from_typed)
                .collect(),
            truncated: typed.truncated,
            candidates_processed: typed.candidates_processed,
            candidates_total: typed.candidates_total,
            recognized_marking_count: typed.recognized_marking_count,
        }
    }

    /// No diagnostics produced.
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Count of `Severity::Error` diagnostics. Mirrors
    /// [`LintResult::error_count`].
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    /// Count of `Severity::Warn` diagnostics. Mirrors
    /// [`LintResult::warn_count`].
    pub fn warn_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warn)
            .count()
    }

    /// Count of `Severity::Fix` diagnostics carrying an actionable payload —
    /// either a (now-erased) `FixIntent` ([`ErasedDiagnostic::has_fix`]) or a
    /// `text_correction`. Mirrors [`LintResult::fix_count`] faithfully: the
    /// erased `has_fix` flag stands in for `d.fix.is_some()`.
    pub fn fix_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Fix && (d.has_fix || d.text_correction.is_some()))
            .count()
    }
}

/// Grammar-erased projection of [`FixResult<S>`].
///
/// The scheme-typed pieces of [`FixResult<S>`] are `audit_lines:
/// Vec<AuditLine<S>>` and `remaining_diagnostics: Vec<Diagnostic<S>>`. The
/// erasure pre-renders the audit lines to NDJSON via [`audit_line_to_ndjson`]
/// and projects the diagnostics to [`ErasedDiagnostic`]. `source`,
/// `r002_fired`, and `session_metadata` are already scheme-agnostic and carry
/// over verbatim (`source` is **moved**, never cloned — Principle II).
///
/// `#[non_exhaustive]` so future [`FixResult`] fields land additively.
#[non_exhaustive]
#[derive(Debug)]
pub struct ErasedFixResult {
    pub grammar_id: &'static str,
    /// Fixed source bytes (moved from the typed result; wipes on drop).
    pub source: secrecy::SecretSlice<u8>,
    /// Audit stream pre-rendered to canonical NDJSON record strings (one per
    /// `AuditLine<S>`, no trailing newline) — scheme-agnostic, the same wire
    /// form the CLI / server / WASM emit.
    pub audit_ndjson: Vec<String>,
    pub remaining_diagnostics: Vec<ErasedDiagnostic>,
    pub r002_fired: bool,
    pub session_metadata: SessionMetadata,
}

impl ErasedFixResult {
    /// Project an owned [`FixResult<S>`] under its grammar tag, rendering the
    /// audit stream against `scheme`. Consumes the typed result so `source`
    /// transfers ownership without a content readout-clone.
    pub fn from_typed<S: MarkingScheme<Token = TokenId> + Vocabulary<S>>(
        scheme: &S,
        grammar_id: &'static str,
        typed: FixResult<S>,
    ) -> Self {
        let audit_ndjson = typed
            .audit_lines
            .iter()
            .map(|line: &AuditLine<S>| audit_line_to_ndjson(scheme, line))
            .collect();
        Self {
            grammar_id,
            source: typed.source,
            audit_ndjson,
            remaining_diagnostics: typed
                .remaining_diagnostics
                .into_iter()
                .map(ErasedDiagnostic::from_typed)
                .collect(),
            r002_fired: typed.r002_fired,
            session_metadata: typed.session_metadata,
        }
    }
}

/// Object-safe façade over a concrete [`Engine<S, R>`]. This is the
/// heterogeneous co-residence seam: bytes in, grammar-tagged scheme-agnostic
/// results out. See the module docs for the relationship to
/// `contracts/multi-scheme.md` C2 and which methods are deferred.
///
/// `Send + Sync` matches the engine's existing concurrency discipline
/// (Constitution VI) — [`crate::BatchEngine`] already requires it.
pub trait ErasedEngine: Send + Sync {
    /// The grammar this engine recognizes (`scheme_id()`), e.g. `"capco"`.
    fn grammar_id(&self) -> &'static str;

    /// Lint `input` and return a grammar-tagged scheme-agnostic result. One
    /// call = one scheme = one [`ErasedLintResult::grammar_id`].
    fn lint_erased(&self, input: &[u8], ctx: &InputContext<'_>) -> ErasedLintResult;

    /// Fix `input` (infallible default-options path) and return a
    /// grammar-tagged scheme-agnostic result with the audit stream
    /// pre-rendered to NDJSON.
    fn fix_erased(&self, input: &[u8], mode: FixMode) -> ErasedFixResult;
}

impl<S, R> ErasedEngine for Engine<S, R>
where
    S: MarkingScheme<Token = TokenId> + ConstraintBridge + Vocabulary<S>,
    S::Canonical: Clone + Default + PartialEq,
    R: Recognizer<S>,
{
    fn grammar_id(&self) -> &'static str {
        self.scheme().scheme_id()
    }

    fn lint_erased(&self, input: &[u8], ctx: &InputContext<'_>) -> ErasedLintResult {
        let typed = self.lint_with_input_context(input, &LintOptions::default(), ctx);
        ErasedLintResult::from_typed(self.scheme().scheme_id(), typed)
    }

    fn fix_erased(&self, input: &[u8], mode: FixMode) -> ErasedFixResult {
        let typed = self.fix(input, mode);
        ErasedFixResult::from_typed(self.scheme(), self.scheme().scheme_id(), typed)
    }
}
