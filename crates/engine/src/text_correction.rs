// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Engine-internal text-correction proposal.
//!
//! The engine needs a four-field carrier `(span, replacement,
//! confidence, source)` for the `[corrections]`-map path — a
//! text-level fix-set delta that runs pre-scanner and has no
//! structural `FixIntent` shape. `TextCorrectionProposal` is that
//! carrier, scoped `pub(crate)` to `marque-engine` so no rule crate
//! constructs it.
//!
//! The bytes in `replacement` are corpus-derived canonical tokens
//! (e.g. `"SECRET"` replacing the typo `"SERCET"`) — on Constitution
//! V Principle V's permitted-identifier list. The original document
//! bytes are never copied into a `TextCorrectionProposal`.

use marque_capco::CapcoScheme;
use marque_rules::{FixIntent, FixSource, Message, Recognition, RuleId, Severity};
use marque_scheme::{MarkingScheme, Span};
use smol_str::SmolStr;

/// Engine-internal text-correction proposal — see module-level doc.
///
/// `severity`, `message`, and `migration_ref` let the engine build an
/// `AppliedTextCorrection` audit record at promotion time without
/// redescending into the originating diagnostic stream. `severity` +
/// `message` come from the parent [`marque_rules::Diagnostic`];
/// `migration_ref` comes from the [`marque_rules::TextCorrection`]
/// payload — the rule's provenance flows through unchanged; the engine
/// doesn't overwrite `migration_ref`.
#[derive(Debug, Clone)]
pub(crate) struct TextCorrectionProposal {
    /// Rule that emitted the diagnostic (the corrections-map rule today).
    pub rule: RuleId,
    /// Severity at promotion time (snapshot from the originating
    /// `Diagnostic.severity`).
    pub severity: Severity,
    /// Byte range in the source to replace.
    pub span: Span,
    /// Canonical replacement bytes.
    pub replacement: SmolStr,
    /// Multi-axis confidence; gated against the engine threshold.
    pub confidence: Recognition,
    /// Provenance (typically `FixSource::CorrectionsMap`).
    pub source: FixSource,
    /// Diagnostic message — closed template + closed args. Snapshot
    /// from the originating `Diagnostic.message`.
    pub message: Message,
    /// §-citation backing the correction. `None` for corrections-map
    /// matches; `Some` for deprecation migrations.
    pub migration_ref: Option<&'static str>,
}

/// Engine-internal carrier for a synthesized rule-emitted fix.
///
/// `Engine::fix_inner` collects `FixIntent<S>` emissions
/// from the diagnostic stream, materializes their replacement bytes
/// (via `MarkingScheme::apply_intent` + `render_canonical`), and
/// stores them in `SynthesizedFix` records. The records flow
/// through the confidence-then-span sort + C-1 overlap-guard +
/// audit-promotion. At promotion time the engine moves the `FixIntent`
/// payload into `AppliedFixProposal::FixIntent(_)`.
///
/// Holding the intent alongside the synthesized bytes lets the
/// audit promotion (the `__engine_promote` call) carry the
/// structural payload directly, without an intent-index lookup.
///
/// `severity` + `scope` let the audit-record path build
/// [`AuditLine::AppliedFix`] entries without redescending into the
/// originating diagnostic stream. `severity` snapshots from
/// [`marque_rules::Diagnostic::severity`];
/// `scope` records whether the synthesizer chose portion vs banner
/// rendering (the same `(...)`-wrapper heuristic
/// `synthesize_fixes` uses today).
// `Debug` is derived (consistent with `FixIntent<S>` / `Diagnostic<S>`): the
// derive's `S: Debug` bound only constrains the `Debug` impl, never the struct,
// and the generic fix path never whole-struct-Debug-formats a `SynthesizedFix`.
// `Clone` is hand-written bounded only on `S: MarkingScheme` — a derive would
// impose a spurious `S: Clone` the generic fix path does not carry (same
// rationale as `FixIntent<S>`'s own hand-written `Clone`).
#[derive(Debug)]
pub(crate) struct SynthesizedFix<S: MarkingScheme = CapcoScheme> {
    /// Rule that emitted the diagnostic.
    pub rule: RuleId,
    /// Severity at promotion time. Snapshot from the originating
    /// `Diagnostic.severity`.
    pub severity: marque_rules::Severity,
    /// Byte range in the source the fix targets.
    pub span: Span,
    /// Engine-synthesized canonical replacement bytes.
    pub replacement: Box<str>,
    /// Render scope the synthesizer chose
    /// ([`marque_scheme::scope::Scope::Portion`] vs
    /// [`marque_scheme::scope::Scope::Page`]). Determined by the
    /// engine's universal `()`-wrapper heuristic — a parenthesized
    /// candidate renders as a portion — which originates from the CAPCO
    /// portion grammar (CAPCO-2016 §A.6).
    pub scope: marque_scheme::scope::Scope,
    /// The rule's structural emission, snapshotted at synthesis
    /// time. Carried forward into `AppliedFixProposal::FixIntent`.
    pub intent: FixIntent<S>,
}

impl<S: MarkingScheme> Clone for SynthesizedFix<S> {
    fn clone(&self) -> Self {
        Self {
            rule: self.rule,
            severity: self.severity,
            span: self.span,
            replacement: self.replacement.clone(),
            scope: self.scope,
            intent: self.intent.clone(),
        }
    }
}
