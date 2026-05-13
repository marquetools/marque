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
use marque_ism::Span;
use marque_rules::{Confidence, FixIntent, FixSource, RuleId};
use smol_str::SmolStr;

/// Engine-internal text-correction proposal — see module-level doc.
#[derive(Debug, Clone)]
pub(crate) struct TextCorrectionProposal {
    /// Rule that emitted the diagnostic (always C001 today).
    pub rule: RuleId,
    /// Byte range in the source to replace.
    pub span: Span,
    /// Canonical replacement bytes.
    pub replacement: SmolStr,
    /// Multi-axis confidence; gated against the engine threshold.
    pub confidence: Confidence,
    /// Provenance (typically `FixSource::CorrectionsMap`).
    pub source: FixSource,
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
#[derive(Debug, Clone)]
pub(crate) struct SynthesizedFix {
    /// Rule that emitted the diagnostic.
    pub rule: RuleId,
    /// Byte range in the source the fix targets.
    pub span: Span,
    /// Engine-synthesized canonical replacement bytes.
    pub replacement: Box<str>,
    /// The rule's structural emission, snapshotted at synthesis
    /// time. Carried forward into `AppliedFixProposal::FixIntent`.
    pub intent: FixIntent<CapcoScheme>,
}
