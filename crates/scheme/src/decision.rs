// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Decision-tracing instrumentation surface.
//!
//! An opt-in observability hook that lets the engine count and trace the
//! marking decisions it makes (parser dispatch, constraint firings, page
//! rewrites, closure expansions, supersession reductions, banner roll-up,
//! recanonicalization) without changing what those decisions are.
//!
//! ## Design
//!
//! - The [`DecisionSink`] trait is the single dispatch point. The engine
//!   calls [`DecisionSink::record`] at every choice point worth tracing.
//! - [`NoopSink`] is a ZST whose `record` is `#[inline(always)]` with an
//!   empty body. When the engine threads the sink as a **monomorphized
//!   generic parameter** (`S: DecisionSink`) the compiler can elide every
//!   `NoopSink::record` call site. When the engine threads it as
//!   `&mut dyn DecisionSink` (the configurable / boxed path) the vtable
//!   lookup remains, so dyn dispatch is paid only on the opt-in path.
//! - [`CountingSink`] keeps running tallies (total, by kind, by category,
//!   by portion) — useful for "how busy was the engine on this document?"
//!   reporting.
//! - [`RecordingSink`] keeps the full event stream and can reconstruct
//!   cascade chains by walking [`DecisionEvent::triggered_by`] edges.
//!
//! ## Constitution V (content ignorance)
//!
//! [`DecisionEvent`] is `Copy` and carries only IDs, indices, enum tags,
//! and `&'static str` rule labels (the same content-neutral surface as
//! [`crate::citation::Citation`]). No document text, no token strings,
//! no rule-message bodies. The trace can be emitted alongside the audit
//! log without expanding the audit surface beyond what
//! [`crate::canonical::Canonical`] already guarantees.
//!
//! ## Feature gating
//!
//! The types in this module always compile so downstream callers can
//! write conditional code that references them. The `decision-tracing`
//! feature is a marker for engine-side threading: when the feature is
//! **off**, the engine MUST NOT thread a sink through evaluation at all
//! (no [`NoopSink`] argument, no vtable, nothing) — that is what makes
//! Constitution Principle I (SC-001 16 ms p95) trivially preserved. When
//! the feature is **on**, the engine wires the caller-supplied sink
//! through the pipeline. Implementations should prefer monomorphized
//! generic dispatch over `&mut dyn DecisionSink` wherever the call site
//! lives on the lint hot path, reserving dyn dispatch for the
//! configurable entry-point boundary.

use crate::category::CategoryId;

pub mod report;
pub mod sinks;

#[cfg(test)]
mod tests;

/// Sink for [`DecisionEvent`]s emitted by the engine during evaluation.
///
/// Implementations are stateful (they own whatever counters or buffers
/// they need) and receive a `&mut self` per record call. The engine
/// threads a single sink through one document's evaluation; sinks are
/// not required to be `Send + Sync` because the per-document evaluator
/// is single-threaded by construction (the batch layer holds one sink
/// per worker).
///
/// The trait has one method by design: every decision goes through the
/// same dispatch surface, so a sink that wants to filter (e.g., only
/// record cascade roots) does so by matching on the event payload.
pub trait DecisionSink {
    /// Receive one decision event. Called on the hot path; implementors
    /// SHOULD keep this cheap.
    fn record(&mut self, event: DecisionEvent);
}

/// One decision the engine made during evaluation.
///
/// Pure data; `Copy`. The struct is 56 bytes on a 64-bit target (the
/// layout floor is set by [`DecisionSource`], which carries `&'static str`
/// fat pointers) — small enough that ten thousand events fit in roughly
/// half a megabyte. The exact size is pinned by a `const_assert_eq!` in
/// the test module so a layout change surfaces as a test failure. All
/// fields are IDs, indices, or enum tags; Constitution Principle V
/// (content ignorance) is preserved by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecisionEvent {
    /// Monotone per-document step counter, assigned by the engine.
    /// Used to correlate events into cascade chains.
    pub step: u32,
    /// Where in the document the decision applies.
    pub site: DecisionSite,
    /// Which scheme category the decision touches.
    /// [`CategoryId::MARKING`] is the multi-category sentinel for
    /// whole-marking decisions (e.g., recanonicalization).
    pub category: CategoryId,
    /// What kind of decision this is.
    pub kind: DecisionKind,
    /// Which subsystem produced the decision.
    pub source: DecisionSource,
    /// `Some(step)` when this event was caused by an earlier event;
    /// `None` for cascade roots.
    pub triggered_by: Option<u32>,
}

/// Location within a document where a decision was made.
///
/// The variants form a flat coordinate system over the per-document
/// pipeline: portions are indexed in document order; banner and document
/// are single-instance; page carries the page index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionSite {
    /// A specific portion marking. The index is the portion's
    /// document-order position.
    Portion(u32),
    /// The document banner / CAB.
    Banner,
    /// A specific page (banner roll-up scope). The index is the
    /// page number.
    Page(u32),
    /// Document-level decision (cross-page roll-up,
    /// document-wide recanonicalization).
    Document,
}

/// What category of decision this event represents.
///
/// The variants are ordered for stable `BTreeMap` rendering in the
/// [`report::DecisionReport`] aggregations and for the small
/// fixed-size `by_kind` array in [`sinks::CountingSink`]. Don't reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DecisionKind {
    /// A rule/constraint/closure was evaluated against the marking.
    /// Includes vacuous evaluations (predicate had no effect).
    Evaluated,
    /// A rule/constraint/closure was evaluated and made a substantive
    /// observation (fired, or would have fired but was suppressed).
    /// Subset of [`DecisionKind::Evaluated`].
    EvaluatedSubstantive,
    /// The marking value mutated as a result of the decision.
    Mutated,
    /// A [`crate::constraint::Constraint`] fired (yielded a
    /// `ConstraintViolation`).
    ConstraintFired,
    /// A [`crate::page_rewrite::PageRewrite`] was scheduled for
    /// application (matched its predicate; runs after the current
    /// pass).
    RewriteScheduled,
    /// A scheduled [`crate::page_rewrite::PageRewrite`] was applied.
    RewriteApplied,
    /// A [`crate::closure::ClosureRule`] fired (triggers present,
    /// suppressors absent, cone added).
    ClosureFired,
    /// The marking was re-rendered to its canonical form
    /// (recanonicalization fix path).
    Recanonicalized,
}

/// Which subsystem produced a decision.
///
/// Variants with a `&'static str` argument carry the rule/rewrite/
/// closure stable name (e.g., a CAPCO rule identifier like
/// `"capco:banner.classification.usa-trigraph"`). The string is the
/// same `&'static` label that already appears in
/// [`crate::citation::Citation`] / [`crate::closure::ClosureRuleMetadata`]
/// / [`crate::page_rewrite::RewriteId`] — content-neutral by
/// construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionSource {
    /// Parser dispatch (recognizer chose this marking shape, or
    /// strict-vs-decoder path).
    Parser,
    /// A declared [`crate::constraint::Constraint`] fired.
    /// Argument is the constraint's stable label.
    Constraint(&'static str),
    /// A declared [`crate::page_rewrite::PageRewrite`] fired.
    /// Argument is the rewrite's stable label.
    PageRewrite(&'static str),
    /// A declared [`crate::closure::ClosureRule`] fired.
    /// Argument is the closure rule's stable name.
    Closure(&'static str),
    /// A default-fill rule populated an absent category.
    /// Argument is the rule's stable name.
    DefaultFill(&'static str),
    /// A supersession reduction collapsed a dominated token.
    /// Argument is the supersession relation's stable name.
    Supersession(&'static str),
    /// Banner roll-up combined per-portion values into a page
    /// banner.
    BannerRollup,
    /// A `Rule::check` body produced a diagnostic. Argument is the
    /// rule's stable predicate ID.
    RuleCheck(&'static str),
}

pub use report::{CascadeChain, DecisionReport};
pub use sinks::{CountingSink, NoopSink, RecordingSink};
