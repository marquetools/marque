// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Engine-internal text-correction proposal.
//!
//! The legacy `marque_rules::FixProposal` retired in PR 3c.B Commit
//! 10 atomically with the `marque-mvp-3` audit schema bump. The
//! engine still needs a four-field carrier `(span, replacement,
//! confidence, source)` for the C001 / `[corrections]` map path —
//! a text-level fix-set delta that runs pre-scanner and has no
//! structural `FixIntent` shape. `TextCorrectionProposal` is that
//! carrier, scoped `pub(crate)` to `marque-engine` so no rule crate
//! constructs it.
//!
//! The bytes in `replacement` are corpus-derived canonical tokens
//! (e.g. `"SECRET"` replacing the typo `"SERCET"`) — on Constitution
//! V Principle V's permitted-identifier list. The original document
//! bytes are never copied into a `TextCorrectionProposal`.

use marque_capco::CapcoScheme;
use marque_rules::{Confidence, FixIntent, FixSource, Message, RuleId, Severity};
use marque_scheme::Span;
use smol_str::SmolStr;

/// Engine-internal text-correction proposal — see module-level doc.
///
/// PR 3c.2.D / D3 added `severity`, `message`, and `migration_ref` so
/// the engine can build a `marque-1.0` `AppliedTextCorrection` audit
/// record at promotion time without redescending into the originating
/// diagnostic stream. `severity` + `message` come from the parent
/// [`marque_rules::Diagnostic`]; `migration_ref` comes from the
/// [`marque_rules::TextCorrection`] payload (per
/// `feedback_audit_predicates_against_source.md`, the rule's
/// provenance must flow through unchanged — engine doesn't overwrite
/// `migration_ref`).
#[derive(Debug, Clone)]
pub(crate) struct TextCorrectionProposal {
    /// Rule that emitted the diagnostic (always C001 today).
    pub rule: RuleId,
    /// Severity at promotion time (snapshot from the originating
    /// `Diagnostic.severity`).
    pub severity: Severity,
    /// Byte range in the source to replace.
    pub span: Span,
    /// Canonical replacement bytes.
    pub replacement: SmolStr,
    /// Multi-axis confidence; gated against the engine threshold.
    pub confidence: Confidence,
    /// Provenance (typically `FixSource::CorrectionsMap`).
    pub source: FixSource,
    /// Diagnostic message — closed template + closed args. Snapshot
    /// from the originating `Diagnostic.message`.
    pub message: Message,
    /// §-citation backing the correction. `None` for C001
    /// corrections-map matches; `Some` for E006-shaped deprecation
    /// migrations.
    pub migration_ref: Option<&'static str>,
}

/// Engine-internal carrier for a synthesized rule-emitted fix.
///
/// `Engine::fix_inner` collects `FixIntent<CapcoScheme>` emissions
/// from the diagnostic stream, materializes their replacement bytes
/// (via `MarkingScheme::apply_intent` + `render_canonical`), and
/// stores them in `SynthesizedFix` records. The records flow
/// through FR-016 sort + C-1 overlap-guard + audit-promotion. At
/// promotion time the engine moves the `FixIntent` payload into
/// `AppliedFixProposal::FixIntent(_)`.
///
/// Holding the intent alongside the synthesized bytes lets the
/// audit promotion (the `__engine_promote` call) carry the
/// structural payload directly, without an intent-index lookup.
///
/// PR 3c.2.D / D3 added `severity` + `scope` so the marque-1.0 v2
/// audit-record path can build [`AuditLine::AppliedFix`] entries
/// without redescending into the originating diagnostic stream.
/// `severity` snapshots from [`marque_rules::Diagnostic::severity`];
/// `scope` records whether the synthesizer chose portion vs banner
/// rendering (the same `(...)`-wrapper heuristic
/// `synthesize_fixes` uses today).
#[derive(Debug, Clone)]
pub(crate) struct SynthesizedFix {
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
    /// [`marque_scheme::scope::Scope::Page`]). Determined by whether
    /// the original candidate bytes were wrapped in `()` per
    /// CAPCO-2016 §A.6.
    pub scope: marque_scheme::scope::Scope,
    /// The rule's structural emission, snapshotted at synthesis
    /// time. Carried forward into `AppliedFixProposal::FixIntent`.
    pub intent: FixIntent<CapcoScheme>,
}
