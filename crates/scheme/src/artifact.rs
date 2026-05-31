// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Document-scoped artifact node model.
//!
//! A *document artifact* is a marking-system object that lives at document
//! (not portion) scope: a classification-authority block (CAB / CUI
//! designation block), a declassify instruction, a notice, a caveat layer,
//! a front marking. This module defines the domain-neutral node model:
//!
//! - [`ArtifactKind`] — what kind of artifact a node represents.
//! - [`ArtifactState`] — the five-state presence × requirement product.
//! - [`DocumentArtifact`] — a full node: kind + scope + state + value
//!   derivation + inbound derivation edges + an optional source span.
//!
//! The payload an artifact carries when present is scheme-specific (CAPCO's
//! parsed CAB, for example). It is supplied through the opt-in
//! [`SchemeArtifacts`](crate::scheme::SchemeArtifacts) extension trait's
//! `ArtifactPayload` associated type, keeping the frozen `MarkingScheme`
//! surface free of a new required associated type.
//!
//! # Constitution V Principle V (audit content-ignorance)
//!
//! Every field here carries only structural data — enum tags, a value-
//! derivation tag, derivation-edge topology, and an optional byte
//! [`Span`]. The present-state payload type is scheme-chosen; schemes MUST
//! keep it content-ignorant for audit-adjacent uses (CAPCO binds it to a
//! parsed-CAB structural type, not raw document bytes).

use crate::derivation::DerivationEdge;
use crate::provenance::ValueDerivation;
use crate::scheme::SchemeArtifacts;
use crate::scope::Scope;
use crate::span::Span;

/// What kind of document-scoped artifact a node represents.
///
/// Domain-neutral: each scheme decides the concrete shape behind each kind
/// (a CAPCO CAB vs. a CUI designation block are both
/// [`ArtifactKind::AuthorityBlock`]). `#[non_exhaustive]` reserves
/// grow-path for future artifact kinds.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactKind {
    /// A classification-authority block — a CAPCO CAB, a CUI designation
    /// block, or the equivalent. The scheme decides the block's shape.
    AuthorityBlock,
    /// A declassification instruction — e.g. a `Declassify On:` line.
    DeclassifyInstruction,
    /// A notice — a US-Person notice, a distribution statement, or the
    /// equivalent policy notice.
    Notice,
    /// A caveat layer — the second banner-line caveats (issue #128).
    CaveatLayer,
    /// A front marking — the document's overall "classified up to"
    /// classification (issue #799).
    FrontMarking,
}

/// The five-state node model — the product of *presence* and *requirement*.
///
/// This is a **status enum, not a lattice**. One recognizer yields exactly
/// one state per node; states of the same node are never joined. Do NOT
/// implement
/// [`JoinSemilattice`](crate::lattice::JoinSemilattice) /
/// [`MeetSemilattice`](crate::lattice::MeetSemilattice) for this type —
/// per the lattice-consultant verdict (LV1), joining node states is not a
/// meaningful operation and would invite the engine to "combine" two
/// readings of the same node, which is exactly the bug the five-state model
/// prevents.
///
/// The five states are genuinely distinct:
///
/// - [`Present`](Self::Present) vs.
///   [`PresentNonCanonical`](Self::PresentNonCanonical): present in
///   canonical form vs. present but diverging from canonical form (a fix
///   target).
/// - [`PresentNotRequired`](Self::PresentNotRequired): present yet
///   superfluous (e.g. a §E.5 string in a pure-NATO document) — distinct
///   from `Present` (which is required).
/// - [`AbsentButRequired`](Self::AbsentButRequired) vs.
///   [`AbsentNotRequired`](Self::AbsentNotRequired): an inbound requirement
///   edge demands the node vs. the node is legitimately absent.
///
/// `P` is the present-state payload (the scheme's parsed artifact). The
/// `Absent*` states carry no payload.
///
/// `Debug` / `Clone` / `PartialEq` / `Eq` derive normally because `P` is a
/// plain type parameter; the derives add the natural `where P: Trait`
/// bounds, which is the correct constraint here (unlike the
/// `S::ArtifactPayload` projection on [`DocumentArtifact`], which needs
/// manual impls).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactState<P> {
    /// Present, canonical, and required — the fully-correct state.
    Present(P),
    /// Present and parsed, but the form diverges from canonical (a
    /// recanonicalization fix target).
    PresentNonCanonical(P),
    /// Present but superfluous — no inbound requirement edge demands it
    /// (e.g. a §E.5 standard string in a pure-NATO document).
    PresentNotRequired(P),
    /// Absent, and an inbound requirement edge demands it (a fix target:
    /// the node must be added).
    AbsentButRequired,
    /// Absent, and legitimately so — no requirement edge demands it.
    AbsentNotRequired,
}

/// A document-scoped artifact node.
///
/// Generic over the scheme `S` (via the opt-in
/// [`SchemeArtifacts`](crate::scheme::SchemeArtifacts) extension trait) so
/// the present-state payload type is the scheme's own
/// [`ArtifactPayload`](crate::scheme::SchemeArtifacts::ArtifactPayload).
///
/// `Debug` / `Clone` / `PartialEq` are written manually rather than derived
/// because a blanket derive over `S` would over-constrain — it would demand
/// `S: Debug + Clone + ...` even though only `S::ArtifactPayload` actually
/// needs the bound (the same pattern used for
/// [`FactRef`](crate::FactRef) / [`ReplacementIntent`](crate::ReplacementIntent)
/// in `fix_intent.rs`).
pub struct DocumentArtifact<S: SchemeArtifacts + ?Sized> {
    /// What kind of artifact this node is.
    pub kind: ArtifactKind,
    /// The scope this node lives at. `Document` today; `Bundle` arrives in
    /// a later phase (Phase 0b) when `Scope` gains the variant.
    pub scope: Scope,
    /// The node's presence × requirement state, carrying the scheme's
    /// payload in the three `Present*` states.
    pub state: ArtifactState<S::ArtifactPayload>,
    /// How this node's value was derived.
    pub derivation: ValueDerivation,
    /// The inbound derivation edges into this node (static topology).
    pub inbound: Box<[DerivationEdge]>,
    /// The source location of a present node; `None` when the node is
    /// absent (the `Absent*` states have no span).
    pub span: Option<Span>,
}

impl<S: SchemeArtifacts + ?Sized> core::fmt::Debug for DocumentArtifact<S>
where
    S::ArtifactPayload: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DocumentArtifact")
            .field("kind", &self.kind)
            .field("scope", &self.scope)
            .field("state", &self.state)
            .field("derivation", &self.derivation)
            .field("inbound", &format_args!("[{} edge(s)]", self.inbound.len()))
            .field("span", &self.span)
            .finish()
    }
}

impl<S: SchemeArtifacts + ?Sized> Clone for DocumentArtifact<S>
where
    S::ArtifactPayload: Clone,
{
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            scope: self.scope,
            state: self.state.clone(),
            derivation: self.derivation,
            inbound: self.inbound.clone(),
            span: self.span,
        }
    }
}

impl<S: SchemeArtifacts + ?Sized> PartialEq for DocumentArtifact<S>
where
    S::ArtifactPayload: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.scope == other.scope
            && self.state == other.state
            && self.derivation == other.derivation
            && self.inbound == other.inbound
            && self.span == other.span
    }
}

impl<S: SchemeArtifacts + ?Sized> Eq for DocumentArtifact<S> where S::ArtifactPayload: Eq {}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::ambiguity::Parsed;
    use crate::category::{Category, TokenId};
    use crate::constraint::{Constraint, ConstraintViolation};
    use crate::lattice::{JoinSemilattice, MeetSemilattice};
    use crate::scope::Scope;
    use crate::template::Template;

    // ---- Five-state model tests (payload = a plain test type) ----

    #[test]
    fn five_states_are_constructible() {
        // All five states must be constructible — this documents that the
        // model genuinely distinguishes them.
        let _present: ArtifactState<u32> = ArtifactState::Present(7);
        let _non_canon: ArtifactState<u32> = ArtifactState::PresentNonCanonical(7);
        let _superfluous: ArtifactState<u32> = ArtifactState::PresentNotRequired(7);
        let _absent_req: ArtifactState<u32> = ArtifactState::AbsentButRequired;
        let _absent_ok: ArtifactState<u32> = ArtifactState::AbsentNotRequired;
    }

    #[test]
    fn absent_but_required_differs_from_absent_not_required() {
        // The core distinction the five-state model exists to make: an
        // absent node that a requirement edge demands is a fix target;
        // an absent node nothing demands is fine.
        let a: ArtifactState<u32> = ArtifactState::AbsentButRequired;
        let b: ArtifactState<u32> = ArtifactState::AbsentNotRequired;
        assert_ne!(a, b);
    }

    #[test]
    fn present_not_required_differs_from_present() {
        // A superfluous-but-present node (e.g. §E.5 string in a pure-NATO
        // doc) is distinct from a required, canonical present node.
        let present: ArtifactState<u32> = ArtifactState::Present(1);
        let superfluous: ArtifactState<u32> = ArtifactState::PresentNotRequired(1);
        assert_ne!(present, superfluous);
    }

    #[test]
    fn present_non_canonical_differs_from_present() {
        let present: ArtifactState<u32> = ArtifactState::Present(1);
        let non_canon: ArtifactState<u32> = ArtifactState::PresentNonCanonical(1);
        assert_ne!(present, non_canon);
    }

    #[test]
    fn present_states_carry_payload() {
        // The three Present* states carry the payload; the Absent* states
        // do not.
        match ArtifactState::Present(42_u32) {
            ArtifactState::Present(p) => assert_eq!(p, 42),
            _ => panic!("wrong variant"),
        }
        match ArtifactState::PresentNonCanonical(43_u32) {
            ArtifactState::PresentNonCanonical(p) => assert_eq!(p, 43),
            _ => panic!("wrong variant"),
        }
        match ArtifactState::PresentNotRequired(44_u32) {
            ArtifactState::PresentNotRequired(p) => assert_eq!(p, 44),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn artifact_state_clone_and_eq() {
        let s: ArtifactState<u32> = ArtifactState::Present(9);
        assert_eq!(s.clone(), s);
    }

    #[test]
    fn artifact_kind_variants_are_distinct() {
        let all = [
            ArtifactKind::AuthorityBlock,
            ArtifactKind::DeclassifyInstruction,
            ArtifactKind::Notice,
            ArtifactKind::CaveatLayer,
            ArtifactKind::FrontMarking,
        ];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    // ---- DocumentArtifact tests (need a concrete SchemeArtifacts) ----

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct FakeMarking(u32);

    impl JoinSemilattice for FakeMarking {
        fn join(&self, other: &Self) -> Self {
            Self(self.0.max(other.0))
        }
    }

    impl MeetSemilattice for FakeMarking {
        fn meet(&self, other: &Self) -> Self {
            Self(self.0.min(other.0))
        }
    }

    // A scheme that DOES model document artifacts: it implements both
    // MarkingScheme and the opt-in SchemeArtifacts extension.
    struct ArtifactScheme;

    impl crate::scheme::MarkingScheme for ArtifactScheme {
        type Token = TokenId;
        type Marking = FakeMarking;
        type ParseError = ();
        type OpenVocabRef = core::convert::Infallible;
        type Parsed<'src> = ();
        type Canonical = ();

        fn name(&self) -> &str {
            "artifact-fake"
        }
        fn schema_version(&self) -> &str {
            "v0"
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
        fn parse(&self, _: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
            Err(())
        }
        fn validate(&self, _: &Self::Marking) -> Vec<ConstraintViolation> {
            vec![]
        }
        fn project(&self, _: Scope, _: &[Self::Marking]) -> Self::Marking {
            FakeMarking(0)
        }
        fn render_canonical(
            &self,
            _: &Self::Marking,
            _: &crate::RenderContext,
            _: &mut dyn core::fmt::Write,
        ) -> core::fmt::Result {
            Ok(())
        }
    }

    impl SchemeArtifacts for ArtifactScheme {
        // A small structural payload — NOT document bytes (Constitution V).
        type ArtifactPayload = u32;
    }

    #[test]
    fn scheme_declares_one_artifact() {
        // A scheme that implements SchemeArtifacts can carry a
        // DocumentArtifact parameterized by its payload type.
        let node: DocumentArtifact<ArtifactScheme> = DocumentArtifact {
            kind: ArtifactKind::AuthorityBlock,
            scope: Scope::Document,
            state: ArtifactState::Present(7),
            derivation: ValueDerivation::Authored,
            inbound: Box::new([]),
            span: Some(Span::new(0, 10)),
        };
        assert_eq!(node.kind, ArtifactKind::AuthorityBlock);
        assert_eq!(node.scope, Scope::Document);
        assert_eq!(node.derivation, ValueDerivation::Authored);
        assert!(node.inbound.is_empty());
        assert_eq!(node.span, Some(Span::new(0, 10)));
        match node.state {
            ArtifactState::Present(p) => assert_eq!(p, 7),
            _ => panic!("wrong state"),
        }
    }

    #[test]
    fn absent_node_has_no_span() {
        let node: DocumentArtifact<ArtifactScheme> = DocumentArtifact {
            kind: ArtifactKind::Notice,
            scope: Scope::Document,
            state: ArtifactState::AbsentButRequired,
            derivation: ValueDerivation::CannedPolicyString,
            inbound: Box::new([]),
            span: None,
        };
        assert_eq!(node.span, None);
        assert!(matches!(node.state, ArtifactState::AbsentButRequired));
    }

    #[test]
    fn document_artifact_debug_and_clone() {
        // Manual Debug/Clone impls flow through S::ArtifactPayload bounds.
        let node: DocumentArtifact<ArtifactScheme> = DocumentArtifact {
            kind: ArtifactKind::FrontMarking,
            scope: Scope::Document,
            state: ArtifactState::PresentNonCanonical(3),
            derivation: ValueDerivation::RolledUp,
            inbound: Box::new([]),
            span: Some(Span::new(2, 5)),
        };
        let cloned = node.clone();
        assert_eq!(cloned.kind, node.kind);
        assert_eq!(cloned.span, node.span);
        let dbg = format!("{node:?}");
        assert!(dbg.contains("DocumentArtifact"));
        assert!(dbg.contains("FrontMarking"));
        assert!(dbg.contains("edge(s)"));
    }

    #[test]
    fn sc005_bundle_seam_composes() {
        // SC-005 reserved bundle seam: Scope::Bundle +
        // DerivationRelation::SourceDerived + a Bundle-scope
        // DocumentArtifact compose with no #823 source-metadata adapter;
        // full wiring is #823. This is purely a COMPILE-GATE proving the
        // bundle vocabulary composes — the real declassify-on node is
        // Phase D.
        use crate::citation::{AuthoritativeSource, Citation, SectionLetter, SectionRef};
        use crate::derivation::{DerivationEdge, DerivationRelation, FiringPredicate};
        use core::num::NonZeroU16;

        let citation = Citation::new(
            AuthoritativeSource::EngineInternal,
            SectionRef::new(SectionLetter::A),
            NonZeroU16::new(1).unwrap(),
        );
        let edge = DerivationEdge::new(
            "test/bundle-source-derived",
            DerivationRelation::SourceDerived,
            citation,
            &[],
            &[],
            FiringPredicate::Always,
        );

        let node: DocumentArtifact<ArtifactScheme> = DocumentArtifact {
            kind: ArtifactKind::DeclassifyInstruction,
            scope: Scope::Bundle,
            state: ArtifactState::AbsentNotRequired,
            derivation: ValueDerivation::RolledUp,
            inbound: Box::new([edge]),
            span: None,
        };

        assert_eq!(node.scope, Scope::Bundle);
        assert_eq!(node.inbound.len(), 1);
        assert_eq!(node.inbound[0].relation, DerivationRelation::SourceDerived);
    }

    /// Pins that the manual [`PartialEq`] impl compares every one of the
    /// six fields. Constructs a baseline, asserts an identical copy is
    /// equal, then for each field in turn builds a sibling that differs
    /// only in that field and asserts inequality. A future seventh field
    /// added to `DocumentArtifact` but dropped from `eq` cannot pass this
    /// test once it is folded into the baseline.
    #[test]
    fn document_artifact_eq_distinguishes_each_field() {
        use crate::citation::{AuthoritativeSource, Citation, SectionLetter, SectionRef};
        use crate::derivation::{DerivationEdge, DerivationRelation, FiringPredicate};
        use core::num::NonZeroU16;

        // A derivation edge used only to make `inbound` non-empty, so the
        // empty-vs-one-edge contrast exercises the `inbound` comparison.
        let edge = || {
            DerivationEdge::new(
                "test/eq-edge",
                DerivationRelation::SourceDerived,
                Citation::new(
                    AuthoritativeSource::EngineInternal,
                    SectionRef::new(SectionLetter::A),
                    NonZeroU16::new(1).unwrap(),
                ),
                &[],
                &[],
                FiringPredicate::Always,
            )
        };

        // Baseline. Every sibling below copies this and mutates exactly
        // one field.
        let base = || DocumentArtifact::<ArtifactScheme> {
            kind: ArtifactKind::FrontMarking,
            scope: Scope::Document,
            state: ArtifactState::Present(1),
            derivation: ValueDerivation::Authored,
            inbound: Box::new([]),
            span: Some(Span::new(0, 10)),
        };

        // An identical pair compares equal.
        assert_eq!(base(), base());

        // `kind` participates in equality.
        let mut differs = base();
        differs.kind = ArtifactKind::Notice;
        assert_ne!(base(), differs);

        // `scope` participates in equality.
        let mut differs = base();
        differs.scope = Scope::Page;
        assert_ne!(base(), differs);

        // `state` participates in equality.
        let mut differs = base();
        differs.state = ArtifactState::Present(2);
        assert_ne!(base(), differs);

        // `derivation` participates in equality.
        let mut differs = base();
        differs.derivation = ValueDerivation::RolledUp;
        assert_ne!(base(), differs);

        // `inbound` participates in equality.
        let mut differs = base();
        differs.inbound = Box::new([edge()]);
        assert_ne!(base(), differs);

        // `span` participates in equality.
        let mut differs = base();
        differs.span = None;
        assert_ne!(base(), differs);
    }
}
