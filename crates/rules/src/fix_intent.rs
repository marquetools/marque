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
//! directly from `marque_scheme` — the types live there because the
//! [`MarkingScheme::apply_intent`](marque_scheme::MarkingScheme::apply_intent)
//! trait method needs to reference them at the trait surface, and
//! `marque-rules` already depends on
//! `marque-scheme` — importing in the other direction would create a
//! cycle. `marque-rules` does NOT re-export them: rule crates import
//! directly from `marque_scheme::{FactRef, ReplacementIntent, RecanonScope}`.
//!
//! # Lifecycle
//!
//! 1. Rule's `check(...)` returns `Vec<Diagnostic<S>>`. Each
//!    `Diagnostic` carries `fix: Option<FixIntent<S>>` — the sole
//!    fix-emission channel.
//! 2. Engine filters by `Recognition::combined() >= threshold`.
//! 3. Engine sorts non-overlapping fixes and resolves overlaps. Span
//!    ordering comes from the *diagnostic's* span — `FixIntent<S>`
//!    carries no `target_span` (spans are diagnostic-only per
//!    architecture.md "Type sketch" invariant 3).
//! 4. Engine snapshots runtime state (timestamp, classifier id,
//!    dry-run flag, input identifier) onto the rule's pure-data
//!    `FixIntent` to produce an `AppliedFix<S>` via
//!    `AppliedFix::__engine_promote(...)`.

use core::fmt::Debug;

use marque_scheme::{MarkingScheme, ReplacementIntent};
use smallvec::SmallVec;

use crate::FixSource;
use crate::message::Message;
use crate::recognition::{FeatureId, Recognition};

/// Rule-emission API.
///
/// **Rules construct this type; the engine promotes.** External
/// rule crates depend on `marque-rules` for [`FixIntent`],
/// [`Message`], [`crate::MessageTemplate`], [`crate::MessageArgs`],
/// [`Recognition`], and [`FeatureId`]; they import
/// [`ReplacementIntent`], [`FactRef`], and [`RecanonScope`] directly
/// from `marque_scheme` (which `marque-rules` already depends on,
/// so the trait surface is transitively available). They do NOT
/// depend on `marque-engine`.
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
/// containing diagnostic's `span` field when it needs to order fixes
/// or resolve overlaps. A `FixIntent` is a structural fact-set delta
/// plus the message attached to its diagnostic; it never references
/// the source buffer.
///
/// # Sole rule-emission API
///
/// Every rule emits `FixIntent<S>` via `Diagnostic::with_fix(...)` /
/// `Diagnostic::with_fix_at_span(...)`, and the engine promotes through
/// the single `__engine_promote` constructor. The internal
/// text-correction helper lives inside `marque-engine` (engine-only
/// `TextCorrectionProposal`), but no rule crate constructs it.
///
/// `FixIntent<S>` deliberately does NOT derive `PartialEq` /
/// `Eq` / `Hash` — `Recognition` and `Message` are not equatable
/// (`Recognition` carries `f32`s, `Message` carries `Box<str>`
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

    /// Recognition-axis confidence. `Recognition::combined()` is the
    /// scalar gated against the engine's threshold.
    pub confidence: Recognition,

    /// Closed-set list of contributing features. Inline-4 capacity
    /// covers the 99th-percentile case (most fixes carry 0–2
    /// features); SmallVec keeps the heap-free path on the hot
    /// path.
    pub feature_ids: SmallVec<[FeatureId; 4]>,

    /// Diagnostic message attached to this fix. Closed template +
    /// closed args; see [`crate::Message`].
    pub message: Message,

    /// Provenance: where this fix recommendation originated
    /// (BuiltinRule, CorrectionsMap, MigrationTable, decoder).
    ///
    /// Rules own their fix's provenance; the engine snapshots it onto
    /// `AppliedFix<S>` at promotion time.
    pub source: FixSource,

    /// Reference to the CAPCO rule or migration document
    /// justifying this fix. `None` for most rules; populated when
    /// a specific section / doc anchor adds value beyond the
    /// diagnostic's `citation`.
    pub migration_ref: Option<&'static str>,
}

impl<S: MarkingScheme> Debug for FixIntent<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FixIntent")
            .field("replacement", &self.replacement)
            .field("confidence", &self.confidence)
            .field("feature_ids", &self.feature_ids)
            .field("message", &self.message)
            .field("source", &self.source)
            .field("migration_ref", &self.migration_ref)
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
            source: self.source,
            migration_ref: self.migration_ref,
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::{MessageArgs, MessageTemplate, Recognition};
    use marque_scheme::TokenId;
    use marque_scheme::ambiguity::Parsed;
    use marque_scheme::category::Category;
    use marque_scheme::constraint::Constraint;
    use marque_scheme::fix_intent::{FactRef, RecanonScope};
    use marque_scheme::lattice::{
        BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
    };
    use marque_scheme::scope::Scope;
    use marque_scheme::template::Template;

    // Minimal MarkingScheme fixture so we can instantiate
    // FixIntent<TestScheme> in unit tests. Mirrors the StubScheme
    // pattern from `crates/scheme/tests/adoption_readiness.rs`.

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct TestMarking;

    impl JoinSemilattice for TestMarking {
        fn join(&self, _other: &Self) -> Self {
            TestMarking
        }
    }

    impl MeetSemilattice for TestMarking {
        fn meet(&self, _other: &Self) -> Self {
            TestMarking
        }
    }

    impl BoundedJoinSemilattice for TestMarking {
        fn bottom() -> Self {
            TestMarking
        }
    }

    impl BoundedMeetSemilattice for TestMarking {
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
        type Parsed<'src> = ();
        type Canonical = ();

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
            _ctx: &marque_scheme::RenderContext,
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
            confidence: Recognition::strict(),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
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
            replacement: ReplacementIntent::fact_remove(FactRef::Cve(TokenId(11)), Scope::Page),
            confidence: Recognition::strict(),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };
        match &intent.replacement {
            ReplacementIntent::FactRemove { facts, scope } => {
                assert_eq!(facts.len(), 1);
                assert!(matches!(facts[0], FactRef::Cve(TokenId(11))));
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
            confidence: Recognition::strict(),
            feature_ids: SmallVec::new(),
            message: Message::new(
                // Generic Recanonicalize FixIntent unit test —
                // `NonCanonicalForm` is the closed-set template the
                // Recanonicalize replacement intent semantically pairs
                // with (the marking is in non-canonical form and the
                // rule emits a whole-marking recanonicalization).
                // Aligns with the audit-record contract (issue #709).
                MessageTemplate::NonCanonicalForm,
                MessageArgs::default(),
            ),
            source: FixSource::BuiltinRule,
            migration_ref: None,
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
