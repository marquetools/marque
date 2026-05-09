// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `FixIntent<S>` — the rule-emission API.
//!
//! Rules emit [`FixIntent`] values; the engine renders them through
//! `MarkingScheme::render_canonical` (PR 3c.2) to produce
//! `Canonical<S>` and promotes via `Engine::fix_inner` (sealed
//! `AppliedFix::__engine_promote`). Rules MUST NOT construct
//! `Canonical<S>`, `AppliedFix`, or any other audit-promotion type
//! directly.
//!
//! See `specs/006-engine-rule-refactor/contracts/fix-intent.md` for
//! the full contract.
//!
//! # Lifecycle (post-PR-3c.2)
//!
//! 1. Rule's `evaluate(...)` returns `Vec<Diagnostic>`, each
//!    `Diagnostic` carrying `fix: Option<FixIntent<S>>`.
//! 2. Engine filters by `Confidence::combined() >= threshold`
//!    (FR-016).
//! 3. Engine sorts non-overlapping fixes (I-3) and resolves overlaps
//!    (C-1).
//! 4. Engine calls
//!    `S::render_canonical::<EngineConstructor<S>>(&intent, &ctx)`
//!    to produce `Canonical<S>` (closed-CVE via
//!    `Canonical::from_cve`; open-vocab via
//!    `EngineConstructor::build_open_vocab`
//!    → `Canonical::from_render`).
//! 5. Engine constructs `AppliedFix` via `__engine_promote(...)`.
//!
//! PR 3c.1 (this PR) ships the types; PR 3c.2 wires the lifecycle
//! and migrates rule emission from `FixProposal` to `FixIntent`.

use core::marker::PhantomData;

use marque_ism::Span;
use marque_scheme::{CategoryId, MarkingScheme, Scope, TokenId};
use smallvec::SmallVec;

use crate::confidence::{Confidence, FeatureId};
use crate::message::Message;

/// Placeholder for the scheme-specific render directive type.
///
/// **PR 3c.1 ships this as a phantom-type alias.** PR 3c.2 lifts the
/// binding to `<S as MarkingScheme>::RenderDirective` (a new
/// associated type on `MarkingScheme`) once `CapcoScheme` is ready
/// to bind it to its concrete `CapcoRenderDirective` enum (covering
/// SCI compositional grammar, SAR program identifiers, FGI trigraph
/// blocks). Keeping the lift atomic with the rule migration in
/// PR 3c.2 avoids a cross-PR breakage of
/// `impl MarkingScheme for CapcoScheme` (associated-type defaults
/// are not stable, so adding a required assoc type here would
/// require touching every existing scheme impl in PR 3c.1 — outside
/// the additive-only constraint).
///
/// During PR 3c.1 no production code constructs a value of this
/// type — `ReplacementIntent::Render` is reachable but no rule
/// emits it yet. The type exists to be migrated.
pub type RenderDirective<S> = PhantomData<S>;

/// Three replacement variants for [`FixIntent`].
///
/// Each variant carries the discriminant the engine needs to dispatch
/// the right canonical construction path:
///
/// - [`ReplacementIntent::Cve`] — closed-vocabulary token; engine
///   renders via `Canonical::from_cve`.
/// - [`ReplacementIntent::Render`] — open-vocabulary structured data
///   (SCI compartment grammar, SAR programs, FGI trigraphs); engine
///   renders via `MarkingScheme::render_canonical`, which calls
///   `EngineConstructor::build_open_vocab`.
/// - [`ReplacementIntent::Delete`] — remove the token entirely;
///   audit records this as `Canonical: <empty>` with
///   `TokenSource::OpenVocab` provenance pointing at the engine
///   call site.
#[derive(Debug, Clone)]
pub enum ReplacementIntent<S: MarkingScheme> {
    /// Closed-vocabulary replacement. The token MUST come from the
    /// scheme's vocabulary surface (`Vocabulary<S>::lookup`). Engine
    /// renders via `Canonical::from_cve`.
    Cve {
        /// The canonical token to render.
        token: TokenId,
        /// Where in the marking the token appears (portion vs banner
        /// vs CAB).
        scope: Scope,
    },

    /// Open-vocabulary replacement. The directive carries
    /// scheme-specific structured data describing what to render;
    /// the engine renders via `MarkingScheme::render_canonical`,
    /// which calls `EngineConstructor::build_open_vocab`.
    ///
    /// Used in CAPCO for SCI compartments / sub-compartments
    /// (CAPCO-2016 §A.6 compositional grammar), SAR program
    /// identifiers (CAPCO-2016 §H.5), and country trigraphs in
    /// some FGI contexts (CAPCO-2016 §H.3).
    Render {
        /// Which category the open-vocab token populates.
        category: CategoryId,
        /// Scheme-specific structured render directive. PR 3c.1
        /// ships this as a phantom-type alias; PR 3c.2 lifts to
        /// `<S as MarkingScheme>::RenderDirective`.
        directive: RenderDirective<S>,
        /// Where in the marking the token appears.
        scope: Scope,
    },

    /// Delete the token entirely. Audit records this as
    /// `Canonical: <empty>` with `TokenSource::OpenVocab` provenance
    /// pointing at the engine call site.
    Delete,
}

/// Rule-emission API.
///
/// **Rules construct this type; the engine renders and promotes.**
/// External rule crates depend on `marque-rules` (which re-exports
/// [`FixIntent`], [`ReplacementIntent`], [`Message`],
/// [`crate::MessageTemplate`], [`crate::MessageArgs`],
/// [`Confidence`], [`FeatureId`]); they do NOT depend on
/// `marque-engine` or on `marque_scheme::canonical::sealed`.
///
/// # Type parameter
///
/// `S: MarkingScheme` — `FixIntent<S>` is constructed concretely;
/// the rule writes `FixIntent<CapcoScheme>` literally. A
/// `dyn MarkingScheme` is not a use case here, so the bound is
/// `Sized` (no `?Sized`).
///
/// # PR 3c.1 status
///
/// PR 3c.1 ships this type **alongside** the existing
/// [`crate::FixProposal`] / [`crate::Diagnostic`] surfaces. Rules
/// continue to emit `FixProposal`. PR 3c.2 migrates rule emission
/// to construct `FixIntent` and wires the engine render +
/// promotion lifecycle.
#[derive(Debug, Clone)]
pub struct FixIntent<S: MarkingScheme> {
    /// Byte span in the original source to replace.
    pub target_span: Span,

    /// What to put there. Three discriminants — see
    /// [`ReplacementIntent`].
    pub replacement: ReplacementIntent<S>,

    /// Multi-axis confidence. `recognition × rule` is gated against
    /// the engine's threshold (FR-016).
    pub confidence: Confidence,

    /// Closed-set list of contributing features. Inline-4 capacity
    /// covers the 99th-percentile case (most fixes carry 0–2
    /// features); SmallVec keeps the heap-free path on the hot
    /// path.
    pub feature_ids: SmallVec<[FeatureId; 4]>,

    /// Diagnostic message attached to this fix. Closed template +
    /// closed args; see [`crate::Message`].
    pub message: Message,
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::{Confidence, MessageArgs, MessageTemplate};
    use marque_ism::Span;
    use marque_scheme::ambiguity::Parsed;
    use marque_scheme::category::Category;
    use marque_scheme::constraint::Constraint;
    use marque_scheme::lattice::{BoundedLattice, Lattice};
    use marque_scheme::scope::Scope;
    use marque_scheme::template::Template;

    // Minimal MarkingScheme fixture so we can instantiate
    // FixIntent<TestScheme> in unit tests. Mirrors the StubScheme
    // pattern from `crates/scheme/tests/adoption_readiness.rs`.

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct TestMarking;

    impl Lattice for TestMarking {
        fn join(&self, _other: &Self) -> Self {
            TestMarking
        }
        fn meet(&self, _other: &Self) -> Self {
            TestMarking
        }
    }

    impl BoundedLattice for TestMarking {
        fn bottom() -> Self {
            TestMarking
        }
        fn top() -> Self {
            TestMarking
        }
    }

    struct TestScheme;

    impl MarkingScheme for TestScheme {
        type Token = ();
        type Marking = TestMarking;
        type ParseError = ();

        fn name(&self) -> &str {
            "TestScheme"
        }
        fn schema_version(&self) -> &str {
            "0.0.1"
        }
        fn categories(&self) -> &[Category] {
            &[]
        }
        fn constraints(&self) -> &[Constraint] {
            &[]
        }
        fn templates(&self) -> &[Template] {
            &[]
        }
        fn parse(&self, _input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
            Ok(Parsed::Unambiguous(TestMarking))
        }
        fn project(&self, _scope: Scope, _markings: &[Self::Marking]) -> Self::Marking {
            TestMarking
        }
        fn render_portion(&self, _m: &Self::Marking) -> String {
            String::new()
        }
        fn render_banner(&self, _m: &Self::Marking) -> String {
            String::new()
        }
    }

    #[test]
    fn fix_intent_cve_round_trip() {
        let intent: FixIntent<TestScheme> = FixIntent {
            target_span: Span::new(0, 4),
            replacement: ReplacementIntent::Cve {
                token: TokenId(7),
                scope: Scope::Portion,
            },
            confidence: Confidence::strict(0.95),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
        };
        assert_eq!(intent.target_span, Span::new(0, 4));
        match &intent.replacement {
            ReplacementIntent::Cve { token, scope } => {
                assert_eq!(*token, TokenId(7));
                assert_eq!(*scope, Scope::Portion);
            }
            _ => panic!("expected Cve replacement"),
        }
        assert_eq!(intent.message.template(), MessageTemplate::SupersededToken);
    }

    #[test]
    fn fix_intent_render_uses_phantom_directive() {
        // PR 3c.1: RenderDirective<S> is PhantomData<S>; PR 3c.2
        // lifts to a concrete CapcoRenderDirective. This test pins
        // that the variant compiles with the placeholder; PR 3c.2's
        // migration replaces this fixture with a real directive.
        let intent: FixIntent<TestScheme> = FixIntent {
            target_span: Span::new(10, 20),
            replacement: ReplacementIntent::Render {
                category: CategoryId(3),
                directive: PhantomData::<TestScheme>,
                scope: Scope::Page,
            },
            confidence: Confidence::strict(0.8),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::BannerRollupMismatch, MessageArgs::default()),
        };
        match &intent.replacement {
            ReplacementIntent::Render {
                category, scope, ..
            } => {
                assert_eq!(*category, CategoryId(3));
                assert_eq!(*scope, Scope::Page);
            }
            _ => panic!("expected Render replacement"),
        }
    }

    #[test]
    fn fix_intent_delete_round_trip() {
        let intent: FixIntent<TestScheme> = FixIntent {
            target_span: Span::new(5, 9),
            replacement: ReplacementIntent::Delete,
            confidence: Confidence::strict(1.0),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
        };
        assert!(matches!(intent.replacement, ReplacementIntent::Delete));
    }

    #[test]
    fn fix_intent_is_send_and_sync() {
        // Constitution VI: rule-emission types must be Send + Sync
        // for BatchEngine to schedule them across worker threads.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FixIntent<TestScheme>>();
        assert_send_sync::<ReplacementIntent<TestScheme>>();
    }
}
