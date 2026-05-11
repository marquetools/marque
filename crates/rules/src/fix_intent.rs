// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `FixIntent<S>` — the rule-emission API.
//!
//! Rules emit [`FixIntent`] values; the engine promotes them via
//! `Engine::fix_inner` (sealed `AppliedFix::__engine_promote`). Rules
//! MUST NOT construct `AppliedFix` or any other audit-promotion type
//! directly.
//!
//! See `specs/006-engine-rule-refactor/architecture.md` ("What fixes
//! are") for the binding structural commitment. The three-variant
//! vocabulary — `FactAdd` / `FactRemove` / `Recanonicalize` — is the
//! full surface, defined in
//! [`marque_scheme::fix_intent`](marque_scheme::fix_intent). Rules
//! import [`FactRef`], [`ReplacementIntent`], and [`RecanonScope`]
//! through `marque-rules`'s re-exports; the types themselves live in
//! `marque-scheme` because the
//! [`MarkingScheme::apply_intent`](marque_scheme::MarkingScheme::apply_intent)
//! trait method (the engine-prereq PR) needs to reference them at the
//! trait surface, and `marque-rules` already depends on
//! `marque-scheme` — importing in the other direction would create a
//! cycle.
//!
//! # Lifecycle (post-PR-3c.B Commit 2)
//!
//! 1. Rule's `check(...)` returns `Vec<Diagnostic<S>>`. Each
//!    `Diagnostic` carries `fix_intent: Option<FixIntent<S>>` for
//!    migrated rules; legacy rules continue to populate
//!    `fix: Option<FixProposal>` until they are migrated in
//!    Commit 3+.
//! 2. Engine filters by `Confidence::combined() >= threshold`
//!    (FR-016).
//! 3. Engine sorts non-overlapping fixes (I-3) and resolves overlaps
//!    (C-1). Span ordering comes from the *diagnostic's* span —
//!    `FixIntent<S>` carries no `target_span` (spans are
//!    diagnostic-only per architecture.md "Type sketch" invariant 3).
//! 4. Engine snapshots runtime state (timestamp, classifier id,
//!    dry-run flag, input identifier) onto the rule's pure-data
//!    `FixIntent` to produce an `AppliedFix<S>` via
//!    `AppliedFix::__engine_promote(...)` (new) or
//!    `__engine_promote_legacy(...)` (legacy `FixProposal` path).
//!    Both variants land in `AppliedFix.proposal:
//!    AppliedFixProposal<S>` for the duration of the Commit 2–9
//!    transition; Commit 10 retires the legacy variant atomically
//!    with the audit-schema flip.

use core::fmt::Debug;

use marque_scheme::{MarkingScheme, ReplacementIntent};
use smallvec::SmallVec;

use crate::confidence::{Confidence, FeatureId};
use crate::message::Message;

/// Rule-emission API.
///
/// **Rules construct this type; the engine promotes.** External
/// rule crates depend on `marque-rules` (which re-exports
/// [`FixIntent`], [`ReplacementIntent`], [`FactRef`],
/// [`RecanonScope`], [`Message`], [`crate::MessageTemplate`],
/// [`crate::MessageArgs`], [`Confidence`], [`FeatureId`]); they do
/// NOT depend on `marque-engine`.
///
/// # Type parameter
///
/// `S: MarkingScheme` — `FixIntent<S>` is constructed concretely;
/// the rule writes `FixIntent<CapcoScheme>` literally. A
/// `dyn MarkingScheme` is not a use case here, so the bound is
/// `Sized` (no `?Sized`).
///
/// # No `target_span`
///
/// `FixIntent` carries no span. Spans are diagnostic-only per
/// architecture.md "Type sketch" invariant 3. The engine reads the
/// containing diagnostic's `span` field when it needs to order
/// fixes (FR-016) or resolve overlaps (C-1). A `FixIntent` is a
/// structural fact-set delta plus the message attached to its
/// diagnostic; it never references the source buffer.
///
/// # Transitional coexistence with `FixProposal` (Commit 2–9)
///
/// `marque-rules` ships `FixIntent<S>` and the legacy
/// [`crate::FixProposal`] in parallel. Rules migrate from one to
/// the other one at a time over Commits 3–9. Commit 10 retires
/// `FixProposal` atomically with the audit-schema flip.
///
/// `FixIntent<S>` deliberately does NOT derive `PartialEq` /
/// `Eq` / `Hash` — `Confidence` and `Message` are not equatable
/// (`Confidence` carries `f32`s, `Message` carries `Box<str>`
/// payloads whose equality is content-dependent). The engine keys
/// its audit-fix lookup tables on `(RuleId, Span)` from the parent
/// diagnostic, not on the intent payload itself, so the trait
/// bounds are not load-bearing.
///
/// `Debug` and `Clone` are written manually rather than derived so
/// the trait bounds resolve through `S::OpenVocabRef` (which is
/// `Debug + Clone` per `MarkingScheme`'s trait surface) instead of
/// over-constraining to `S: Debug + Clone`.
pub struct FixIntent<S: MarkingScheme> {
    /// What to do — fact-set add / remove / recanonicalize.
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

impl<S: MarkingScheme> Debug for FixIntent<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FixIntent")
            .field("replacement", &self.replacement)
            .field("confidence", &self.confidence)
            .field("feature_ids", &self.feature_ids)
            .field("message", &self.message)
            .finish()
    }
}

impl<S: MarkingScheme> Clone for FixIntent<S> {
    fn clone(&self) -> Self {
        Self {
            replacement: self.replacement.clone(),
            confidence: self.confidence.clone(),
            feature_ids: self.feature_ids.clone(),
            message: self.message.clone(),
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::{Confidence, MessageArgs, MessageTemplate};
    use marque_scheme::ambiguity::Parsed;
    use marque_scheme::category::Category;
    use marque_scheme::constraint::Constraint;
    use marque_scheme::fix_intent::{FactRef, RecanonScope};
    use marque_scheme::lattice::{BoundedLattice, Lattice};
    use marque_scheme::scope::Scope;
    use marque_scheme::template::Template;
    use marque_scheme::TokenId;

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
        type OpenVocabRef = core::convert::Infallible;

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
        fn render_canonical(
            &self,
            _m: &Self::Marking,
            _scope: Scope,
            _out: &mut dyn core::fmt::Write,
        ) -> core::fmt::Result {
            Ok(())
        }
    }

    #[test]
    fn fix_intent_fact_add() {
        let intent: FixIntent<TestScheme> = FixIntent {
            replacement: ReplacementIntent::FactAdd {
                token: FactRef::Cve(TokenId(7)),
                scope: Scope::Portion,
            },
            confidence: Confidence::strict(0.95),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
        };
        match &intent.replacement {
            ReplacementIntent::FactAdd { token, scope } => {
                assert!(matches!(token, FactRef::Cve(TokenId(7))));
                assert_eq!(*scope, Scope::Portion);
            }
            _ => panic!("expected FactAdd replacement"),
        }
        assert_eq!(intent.message.template(), MessageTemplate::SupersededToken);
    }

    #[test]
    fn fix_intent_fact_remove() {
        let intent: FixIntent<TestScheme> = FixIntent {
            replacement: ReplacementIntent::FactRemove {
                token_ref: FactRef::Cve(TokenId(11)),
                scope: Scope::Page,
            },
            confidence: Confidence::strict(0.9),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
        };
        match &intent.replacement {
            ReplacementIntent::FactRemove { token_ref, scope } => {
                assert!(matches!(token_ref, FactRef::Cve(TokenId(11))));
                assert_eq!(*scope, Scope::Page);
            }
            _ => panic!("expected FactRemove replacement"),
        }
    }

    #[test]
    fn fix_intent_recanonicalize() {
        let intent: FixIntent<TestScheme> = FixIntent {
            replacement: ReplacementIntent::Recanonicalize {
                scope: RecanonScope::Page,
            },
            confidence: Confidence::strict(1.0),
            feature_ids: SmallVec::new(),
            message: Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs::default(),
            ),
        };
        assert!(matches!(
            intent.replacement,
            ReplacementIntent::Recanonicalize {
                scope: RecanonScope::Page
            }
        ));
    }

    #[test]
    fn fix_intent_is_send_and_sync() {
        // Constitution VI: rule-emission types must be Send + Sync
        // for BatchEngine to schedule them across worker threads.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FixIntent<TestScheme>>();
        assert_send_sync::<ReplacementIntent<TestScheme>>();
        assert_send_sync::<FactRef<TestScheme>>();
        assert_send_sync::<RecanonScope>();
    }
}
