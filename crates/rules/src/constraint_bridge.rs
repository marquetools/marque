// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Scheme-side hooks for translating constraint evaluation into engine
//! diagnostics.
//!
//! # Why this trait lives in `marque-rules`, not `marque-scheme`
//!
//! The natural home for these hooks would be the [`MarkingScheme`] trait
//! itself — they are scheme behavior. But their signatures reference
//! [`FixIntent`], [`Message`], and [`Diagnostic`], which all live in
//! `marque-rules`, and `marque-scheme` is the dependency-graph leaf: it
//! cannot depend on `marque-rules` (Constitution VII). So the hooks sit
//! here, on a trait with [`MarkingScheme`] as its supertrait — the same
//! placement rationale as [`Rule`](crate::Rule), which is also generic
//! over a scheme but returns `marque-rules` types.
//!
//! The engine's constraint bridge calls these methods generically through
//! an `S: ConstraintBridge` bound, so the bridge stays scheme-agnostic. A
//! scheme that declares no diagnostic constraints inherits the no-op
//! defaults and contributes nothing to the diagnostic stream.

use std::collections::HashMap;

use marque_ism::MarkingType;
use marque_scheme::{MarkingScheme, Scope, Span};

use crate::{Diagnostic, FixIntent, Message, Severity};

/// What the engine learns from a recognized marking beyond the marking
/// itself: whether the recognition came from a probabilistic
/// (non-strict) recognizer, its posterior score when it did, and the
/// optional synthetic diagnostic that records the recognition.
///
/// The engine's recognition boundary consumes the three fields
/// independently — `is_decoder_path` gates the classification-rank floor
/// (a probabilistic reading does not raise the floor), `recognition_score`
/// gates the confidence threshold, and `diagnostic` is pushed when the
/// recognizer rewrote the candidate into a different canonical form. A
/// scheme whose recognizer is purely strict returns the default
/// (strict-path, no score, no diagnostic).
pub struct RecognitionOutcome<S: MarkingScheme> {
    /// `true` when the marking carries recognizer side-channel state from
    /// a probabilistic (non-strict) recognition; `false` for the strict
    /// path. Strict recognitions raise the page rank floor; probabilistic
    /// ones do not (they are themselves bounded by the existing floor).
    pub is_decoder_path: bool,
    /// The recognizer's posterior, present only on the probabilistic path.
    /// `None` on the strict path (which is unconditionally accepted). The
    /// engine rejects the candidate when the score is below its configured
    /// confidence threshold.
    pub recognition_score: Option<f32>,
    /// The synthetic recognition diagnostic, when one should be surfaced
    /// (a probabilistic recognition that rewrote the candidate into a
    /// different canonical form). `None` on the strict path and on a
    /// probabilistic recognition that preserved the bytes verbatim.
    pub diagnostic: Option<Diagnostic<S>>,
}

/// Scheme hooks the engine's constraint bridge invokes to turn
/// constraint-catalog evaluation into [`Diagnostic`]s.
///
/// Every method has a default matching the "scheme declares no
/// diagnostic constraints" behavior, so a minimal scheme implements this
/// trait with an empty `impl` block. CAPCO overrides all five.
///
/// `Sized` is required because two methods return `FixIntent<Self>` /
/// `Diagnostic<Self>` (both are `Sized` structs over the scheme). The
/// engine consumes this trait only as a generic bound on a concrete
/// scheme; any scheme-erasure path uses a separate object-safe trait, so
/// foregoing `dyn ConstraintBridge` costs nothing.
pub trait ConstraintBridge: MarkingScheme + Sized {
    /// Whether this scheme's constraint catalog can produce
    /// span/severity-bearing diagnostics. The engine uses this as a
    /// hot-path gate: a `false` return lets it skip the per-candidate
    /// constraint walk entirely.
    ///
    /// Default: `false` (no diagnostic constraints).
    fn has_diagnostic_constraints(&self) -> bool {
        false
    }

    /// Rule IDs the engine's constraint-catalog bridge emits that
    /// correspond to no registered `Rule::id()`. Each entry is a
    /// `(wire_string, name)` pair the engine's
    /// `canonicalize_rule_overrides` validator folds into the known-key
    /// map, so `.marque.toml [rules] <id-or-name> = "off"` references to
    /// bridge-emitted IDs resolve instead of failing as unknown keys.
    ///
    /// Default: empty — a scheme with no bridge-emitted diagnostics
    /// contributes no extra config keys.
    fn bridge_emitted_rule_ids(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// Synthesize the optional [`FixIntent`] for a fired constraint,
    /// keyed by the catalog row's name (= predicate id).
    ///
    /// Default: `None` (no fix intent).
    fn fix_intent_by_name(
        &self,
        name: &str,
        canonical: &Self::Canonical,
        marking_type: MarkingType,
    ) -> Option<FixIntent<Self>> {
        let _ = (name, canonical, marking_type);
        None
    }

    /// Resolve the typed user-facing [`Message`] for a fired constraint,
    /// keyed by the catalog row's name. Returning `None` lets the engine
    /// fall back to its generic closed-template message (audit
    /// content-ignorance preserved either way).
    ///
    /// Default: `None` (engine uses its generic fallback).
    fn message_by_name(
        &self,
        name: &str,
        canonical: &Self::Canonical,
        marking_type: MarkingType,
    ) -> Option<Message> {
        let _ = (name, canonical, marking_type);
        None
    }

    /// Emit per-system diagnostics that don't map cleanly onto the
    /// single-row constraint-catalog shape (e.g. CAPCO's SCI per-system
    /// companion rules, where one row emits several diagnostics with
    /// distinct fixes). `emitted_id_overrides` is the engine's
    /// predicate-id → severity override map.
    ///
    /// Default: empty (no per-system diagnostics).
    fn bridge_sci_per_system_diagnostics(
        &self,
        canonical: &Self::Canonical,
        candidate_span: Span,
        fix_scope: Scope,
        emitted_id_overrides: &HashMap<&'static str, Severity>,
    ) -> Vec<Diagnostic<Self>> {
        let _ = (canonical, candidate_span, fix_scope, emitted_id_overrides);
        Vec::new()
    }

    /// Interpret a freshly recognized marking: report whether it came
    /// from the probabilistic path, its posterior score, and the optional
    /// synthetic recognition diagnostic.
    ///
    /// Intended to be invoked once per recognized candidate. The synthesis
    /// of the diagnostic lives behind this hook (rather than in the
    /// engine) because it reads scheme-private recognizer side-channel
    /// state and produces a scheme-typed [`Diagnostic`] — neither of which
    /// the engine can name generically. `original_bytes` is the candidate
    /// slice, `kind` its scanner-emitted candidate kind, and
    /// `corpus_override_active` whether an organizational corpus override
    /// is in effect (recorded as an audit-trail feature on the
    /// diagnostic).
    ///
    /// Default: the strict-path outcome (`is_decoder_path: false`, no
    /// score, no diagnostic) — correct for any scheme whose recognizer is
    /// purely strict.
    fn recognition_outcome(
        &self,
        marking: &Self::Marking,
        span: Span,
        original_bytes: &[u8],
        kind: MarkingType,
        corpus_override_active: bool,
    ) -> RecognitionOutcome<Self> {
        let _ = (marking, span, original_bytes, kind, corpus_override_active);
        RecognitionOutcome {
            is_decoder_path: false,
            recognition_score: None,
            diagnostic: None,
        }
    }
}
