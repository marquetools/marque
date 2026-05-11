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
//! full surface. The pre-Commit-2 directive-enum design (closed-CVE
//! token, open-vocab `RenderDirective<S>`, byte-`Delete`) is retired
//! per architecture.md "What was lost during PR 3c.1": rules emit
//! fact-set deltas at a `Scope`, not `(span, replacement_bytes)`
//! pairs.
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
use core::hash::Hash;

use marque_scheme::{MarkingScheme, Scope, TokenId};
use smallvec::SmallVec;

use crate::confidence::{Confidence, FeatureId};
use crate::message::Message;

/// A reference to a token in the projected fact set.
///
/// `FactRef` identifies the *fact-set position* the rule is naming,
/// not bytes in the input. This is what makes `FactRemove`
/// source-buffer-agnostic: the engine names what to remove by its
/// place in the projected lattice, never by an input span.
///
/// Closed-CVE tokens use [`FactRef::Cve`]; open-vocabulary tokens
/// (SAR program identifiers, SCI compartment / sub-compartment
/// paths, FGI tetragraphs in CAPCO) use [`FactRef::OpenVocab`] with
/// the scheme's [`MarkingScheme::OpenVocabRef`] carrier. Schemes
/// without open-vocab axes bind `OpenVocabRef = Infallible`, making
/// the `OpenVocab` variant statically unreachable.
///
/// # Constitution V Principle V (G13 closure)
///
/// `FactRef` carries no document content. `OpenVocab` payloads come
/// from the scheme's *canonicalize* output — typed structural
/// references whose payload is the canonicalized value, not raw
/// input bytes (a SAR program ID *value*, not a slice of source).
/// This is what preserves audit-content-ignorance: an `AppliedFix`
/// referring to a token via `FactRef` never stores subject-side
/// bytes.
///
/// `Debug` / `Clone` / `PartialEq` / `Eq` / `Hash` are written
/// manually rather than derived so the trait bounds resolve through
/// `S::OpenVocabRef` (which carries the full bound set per
/// `MarkingScheme`) instead of over-constraining to `S: Debug +
/// Clone + ...`.
pub enum FactRef<S: MarkingScheme> {
    /// Closed-vocabulary token; resolves to a unique entry in the
    /// scheme's CVE-registered vocabulary.
    Cve(TokenId),
    /// Open-vocabulary structural reference. The scheme's
    /// canonicalize step produces these from input; the engine
    /// re-renders from them. See [`MarkingScheme::OpenVocabRef`].
    OpenVocab(S::OpenVocabRef),
}

impl<S: MarkingScheme> Debug for FactRef<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FactRef::Cve(id) => f.debug_tuple("Cve").field(id).finish(),
            FactRef::OpenVocab(r) => f.debug_tuple("OpenVocab").field(r).finish(),
        }
    }
}

impl<S: MarkingScheme> Clone for FactRef<S> {
    fn clone(&self) -> Self {
        match self {
            FactRef::Cve(id) => FactRef::Cve(*id),
            FactRef::OpenVocab(r) => FactRef::OpenVocab(r.clone()),
        }
    }
}

impl<S: MarkingScheme> PartialEq for FactRef<S> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FactRef::Cve(a), FactRef::Cve(b)) => a == b,
            (FactRef::OpenVocab(a), FactRef::OpenVocab(b)) => a == b,
            _ => false,
        }
    }
}

impl<S: MarkingScheme> Eq for FactRef<S> {}

impl<S: MarkingScheme> Hash for FactRef<S> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        match self {
            FactRef::Cve(id) => {
                0u8.hash(state);
                id.hash(state);
            }
            FactRef::OpenVocab(r) => {
                1u8.hash(state);
                r.hash(state);
            }
        }
    }
}

/// Three structural fix variants — see
/// `specs/006-engine-rule-refactor/architecture.md` "What fixes
/// are."
///
/// - [`ReplacementIntent::FactAdd`] — add a token to the projected
///   fact set at a given scope. Repairs `Constraint::Requires`
///   violations.
/// - [`ReplacementIntent::FactRemove`] — remove a token from the
///   projected fact set at a given scope. Repairs
///   `Constraint::Conflicts` violations.
/// - [`ReplacementIntent::Recanonicalize`] — input form diverges
///   from canonical form on a scope. The fact set is correct; only
///   the rendering isn't. Subsumes delimiter normalization, sort
///   canonicalization, abbreviation canonicalization, block
///   reordering, and banner roll-up form.
///
/// No `Box<str>` payloads. No multi-span carriers. No `Delete` /
/// `Render` byte-surgery variants. The directive-enum design was the
/// wrong abstraction layer (per architecture.md "What was lost
/// during PR 3c.1").
///
/// `Debug` and `Clone` are written manually rather than derived for
/// the same reason as on [`FactRef`] — to avoid over-constraining
/// the trait bound to `S: Debug + Clone`.
pub enum ReplacementIntent<S: MarkingScheme> {
    /// Add a token to the projected fact set at `scope`.
    FactAdd {
        /// The token to add. Closed-vocab via [`FactRef::Cve`] or
        /// open-vocab via [`FactRef::OpenVocab`].
        token: FactRef<S>,
        /// Which projection level the addition applies to
        /// (`Portion` / `Page` / `Document`).
        scope: Scope,
    },

    /// Remove a token from the projected fact set at `scope`.
    FactRemove {
        /// The token to remove, identified by its position in the
        /// projected fact set (never by raw input bytes).
        token_ref: FactRef<S>,
        /// Which projection level the removal applies to.
        scope: Scope,
    },

    /// Recanonicalize the rendering of `scope`. Input form
    /// diverges from canonical form; the fact set is correct.
    /// The renderer re-renders the scope per `render_canonical`.
    ///
    /// # Engine dispatch contract
    ///
    /// At fix-application time, `Engine::fix_inner` consults its
    /// in-scope projection (already computed during `lint` per
    /// Constitution VI's dataflow pipeline) for the named
    /// [`RecanonScope`], then calls
    /// `render_canonical(&projection.marking, scope.into(),
    /// &mut writer)` on the active scheme. Rules NEVER carry the
    /// `ProjectedMarking` — the engine is the authority on
    /// per-scope projections; the rule only names which scope to
    /// re-render. See `MarkingScheme::render_canonical` doc
    /// comment for the full writer-passing + lattice-equal-byte-
    /// identical contract.
    Recanonicalize {
        /// The positional scope to re-render. `RecanonScope`
        /// excludes `Scope::Diff` because a diff context is not a
        /// recanonicalization target — `Diff` is a rule-context
        /// query mode (architecture.md type-sketch).
        scope: RecanonScope,
    },
}

impl<S: MarkingScheme> Debug for ReplacementIntent<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReplacementIntent::FactAdd { token, scope } => f
                .debug_struct("FactAdd")
                .field("token", token)
                .field("scope", scope)
                .finish(),
            ReplacementIntent::FactRemove { token_ref, scope } => f
                .debug_struct("FactRemove")
                .field("token_ref", token_ref)
                .field("scope", scope)
                .finish(),
            ReplacementIntent::Recanonicalize { scope } => f
                .debug_struct("Recanonicalize")
                .field("scope", scope)
                .finish(),
        }
    }
}

impl<S: MarkingScheme> Clone for ReplacementIntent<S> {
    fn clone(&self) -> Self {
        match self {
            ReplacementIntent::FactAdd { token, scope } => ReplacementIntent::FactAdd {
                token: token.clone(),
                scope: *scope,
            },
            ReplacementIntent::FactRemove { token_ref, scope } => ReplacementIntent::FactRemove {
                token_ref: token_ref.clone(),
                scope: *scope,
            },
            ReplacementIntent::Recanonicalize { scope } => {
                ReplacementIntent::Recanonicalize { scope: *scope }
            }
        }
    }
}

/// The recanonicalization scope.
///
/// A narrowing of [`Scope`] that excludes `Scope::Diff`.
/// `Scope::Diff` is a rule-context query mode, not a
/// projection-output scope, so it is not a meaningful
/// recanonicalization target — see
/// `specs/006-engine-rule-refactor/architecture.md` type-sketch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecanonScope {
    /// Recanonicalize a single portion's rendering.
    Portion,
    /// Recanonicalize a page's rendering (banner / CAB roll-up).
    Page,
    /// Recanonicalize a document's rendering. Usually agrees with
    /// `Page` on single-page documents.
    Document,
}

impl From<RecanonScope> for Scope {
    /// Widen a [`RecanonScope`] back to a full [`Scope`]. Inverse
    /// is the [`TryFrom<Scope>`] impl, which rejects `Scope::Diff`.
    fn from(s: RecanonScope) -> Self {
        match s {
            RecanonScope::Portion => Scope::Portion,
            RecanonScope::Page => Scope::Page,
            RecanonScope::Document => Scope::Document,
        }
    }
}

impl TryFrom<Scope> for RecanonScope {
    /// `Scope::Diff` is the rejection case — it is a rule-context
    /// query mode, not a projection-output scope, and is not a
    /// meaningful recanonicalization target (architecture.md
    /// type-sketch invariant). The other three variants round-trip
    /// with [`From<RecanonScope> for Scope`].
    type Error = ();

    fn try_from(s: Scope) -> Result<Self, Self::Error> {
        match s {
            Scope::Portion => Ok(RecanonScope::Portion),
            Scope::Page => Ok(RecanonScope::Page),
            Scope::Document => Ok(RecanonScope::Document),
            Scope::Diff => Err(()),
        }
    }
}

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

// Compile-time guarantee that `Debug + Clone + PartialEq + Eq +
// Hash` flow through the small positional types. `FactRef`
// requires `S::OpenVocabRef: Eq + Hash + Clone + Debug` from the
// `MarkingScheme` trait bound, so the derives compile for every
// scheme. `FixIntent<S>` itself does NOT participate in equality
// because `Confidence` (f32) and `Message` (Box<str>) are not
// equatable — see the doc comment on `FixIntent`.
const _: fn() = || {
    fn assert_traits<T: Debug + Clone + PartialEq + Eq + Hash>() {}
    let _ = assert_traits::<RecanonScope>;
};

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::{Confidence, MessageArgs, MessageTemplate};
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
    fn recanon_scope_widens_to_scope() {
        assert_eq!(Scope::from(RecanonScope::Portion), Scope::Portion);
        assert_eq!(Scope::from(RecanonScope::Page), Scope::Page);
        assert_eq!(Scope::from(RecanonScope::Document), Scope::Document);
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
