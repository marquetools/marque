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

/// Scheme hooks the engine's constraint bridge invokes to turn
/// constraint-catalog evaluation into [`Diagnostic`]s.
///
/// Every method has a default matching the "scheme declares no
/// diagnostic constraints" behavior, so a minimal scheme implements this
/// trait with an empty `impl` block. CAPCO overrides all four.
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
}
