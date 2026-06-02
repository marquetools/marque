// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Reverse validation of a document's "classified up to" front marking
//! (issue #799).
//!
//! Forward resolution ([`crate::resolution`]) derives a document-scoped
//! artifact from the page rollup. Reverse validation runs the other
//! direction: given a document's declared overall *front marking* (its
//! "classified up to" ceiling) and the rollup of every page's markings, does
//! the front faithfully cover the body? Three outcomes partition the
//! relationship, plus an `Unresolved` answer for an operand that cannot be
//! compared.
//!
//! # Why the comparison is in canonical space
//!
//! [`MarkingScheme::Marking`] intentionally carries no
//! [`JoinSemilattice`](crate::JoinSemilattice) / `Eq` bound — the cross-axis
//! fold is a projection, not a lattice operation (see the `Marking` doc
//! comment). So divergence cannot be computed on markings. It is computed on
//! [`MarkingScheme::Canonical`], which is `Clone + Default + Eq` at the
//! comparison site: equality answers "do they match," and the
//! least-upper-bound from
//! [`canonical_document_join`](MarkingScheme::canonical_document_join)
//! answers "does the front dominate the body." This keeps the algorithm
//! domain-neutral — it never names a scheme's vocabulary, only the lattice
//! surface every scheme already provides.
//!
//! # Constitution V Principle V (audit content-ignorance)
//!
//! [`Divergence`] is an enum tag; [`ReverseValidation`] pairs it with a
//! [`ResolvedArtifact`] (itself structural — an `ArtifactKind`, a
//! `Fixability`, the firing edge ids, and the scheme's structural
//! `Canonical`). No document bytes appear.

use crate::resolution::ResolvedArtifact;
use crate::scheme::MarkingScheme;

/// How a document's front marking relates to the rollup of the markings it
/// claims to cover.
///
/// Derived from the canonical-space join + `Eq` surface, so it is
/// domain-neutral. `#[non_exhaustive]` reserves grow-path.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Divergence {
    /// Front marking and rollup are equal — the front covers the body
    /// exactly, nothing more, nothing less.
    Match,
    /// The rollup carries markings the front does not dominate: the body is
    /// classified or controlled above what the front declares. The
    /// security-significant case (an under-marked document front). The
    /// incomparable case — front and body each carry something the other
    /// lacks — lands here too, since the body still carries content the
    /// front fails to cover.
    FrontUnderClaims,
    /// The front strictly dominates the rollup: it declares more than any
    /// page carries (over-classification at the document front).
    FrontOverClaims,
    /// An operand could not be resolved to a single marking (an ambiguous
    /// or empty reading), so the relationship cannot be computed. Reported
    /// rather than guessed — a false `Match` would hide an under-marking and
    /// a false `FrontUnderClaims` would cry wolf.
    Unresolved,
}

/// The reverse-validation result for a document's front marking.
///
/// Pairs the [`Divergence`] verdict with the resolved
/// [`FrontMarking`](crate::ArtifactKind::FrontMarking) node, so a caller sees
/// both *that* the front diverged and the node it diverged on. The algorithm
/// is the domain-neutral [`divergence`] free function; the engine wraps it as
/// an end-of-document entry point (`Engine::reverse_validate`).
///
/// `Debug` / `Clone` / `PartialEq` are written manually rather than derived
/// because a blanket derive over `S` would over-constrain — it would demand
/// `S: Debug + Clone + ...` even though only `S::Canonical` (inside `front`)
/// needs the bound, the same pattern [`ResolvedArtifact`] uses.
pub struct ReverseValidation<S: MarkingScheme + ?Sized> {
    /// The verdict.
    pub divergence: Divergence,
    /// The resolved front-marking node — `kind:
    /// ArtifactKind::FrontMarking`, fixability following whether a firing
    /// derivation edge can populate it.
    pub front: ResolvedArtifact<S>,
}

impl<S: MarkingScheme + ?Sized> core::fmt::Debug for ReverseValidation<S>
where
    S::Canonical: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReverseValidation")
            .field("divergence", &self.divergence)
            .field("front", &self.front)
            .finish()
    }
}

impl<S: MarkingScheme + ?Sized> Clone for ReverseValidation<S>
where
    S::Canonical: Clone,
{
    fn clone(&self) -> Self {
        Self {
            divergence: self.divergence,
            front: self.front.clone(),
        }
    }
}

impl<S: MarkingScheme + ?Sized> PartialEq for ReverseValidation<S>
where
    S::Canonical: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.divergence == other.divergence && self.front == other.front
    }
}

/// Compute the [`Divergence`] verdict between a front marking and the rollup
/// it claims to cover, using only the canonical-space join + `Eq` surface.
///
/// `front` is the document's declared "classified up to" overall canonical;
/// `rollup` is the document-scope canonical rollup of every page. The
/// least-upper-bound `front ⊔ rollup` is taken via
/// [`canonical_document_join`](MarkingScheme::canonical_document_join):
///
/// - `front == rollup` → [`Divergence::Match`].
/// - else `front ⊔ rollup == front` (front dominates the body strictly) →
///   [`Divergence::FrontOverClaims`].
/// - else (the body carries content the front does not dominate, including
///   the incomparable case) → [`Divergence::FrontUnderClaims`].
///
/// [`Divergence::Unresolved`] is never produced here — it is the engine
/// wrapper's answer for an ambiguous operand it could not project into a
/// single canonical.
///
/// Correctness depends on the scheme's `canonical_document_join` being a
/// genuine semilattice join (the case for any scheme that overrides
/// `canonical_page_join` with a real lattice fold; the default last-element
/// join is order-dependent and not suitable here).
pub fn divergence<S: MarkingScheme + ?Sized>(
    scheme: &S,
    front: &S::Canonical,
    rollup: &S::Canonical,
) -> Divergence
where
    S::Canonical: Clone + Default + Eq,
{
    if front == rollup {
        return Divergence::Match;
    }
    let lub = scheme.canonical_document_join(&[front.clone(), rollup.clone()]);
    if &lub == front {
        Divergence::FrontOverClaims
    } else {
        Divergence::FrontUnderClaims
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::ambiguity::Parsed;
    use crate::artifact::ArtifactKind;
    use crate::category::{Category, TokenId};
    use crate::constraint::{Constraint, ConstraintViolation};
    use crate::lattice::{JoinSemilattice, MeetSemilattice};
    use crate::render_context::RenderContext;
    use crate::resolution::Fixability;
    use crate::scope::Scope;
    use crate::template::Template;

    // A bitset marking so the canonical join is a genuine least-upper-bound
    // (bitwise OR) and all three comparison branches — match, dominate, and
    // incomparable — are reachable.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct StubMarking(u32);

    impl JoinSemilattice for StubMarking {
        fn join(&self, other: &Self) -> Self {
            Self(self.0 | other.0)
        }
    }

    impl MeetSemilattice for StubMarking {
        fn meet(&self, other: &Self) -> Self {
            Self(self.0 & other.0)
        }
    }

    // `Canonical = u32`; `canonical_page_join` overridden to bitwise OR so the
    // document join is a real semilattice fold (the default last-element join
    // would not answer dominance correctly).
    struct StubScheme;

    impl MarkingScheme for StubScheme {
        type Token = TokenId;
        type Marking = StubMarking;
        type ParseError = ();
        type OpenVocabRef = core::convert::Infallible;
        type Parsed<'src> = ();
        type Canonical = u32;
        type Projected = ();

        fn name(&self) -> &str {
            "stub-reverse"
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
            StubMarking(0)
        }
        fn canonical_page_join(&self, portions: &[Self::Canonical]) -> Self::Canonical {
            portions.iter().fold(0, |acc, &c| acc | c)
        }
        fn render_canonical(
            &self,
            _: &Self::Marking,
            _: &RenderContext,
            _: &mut dyn core::fmt::Write,
        ) -> core::fmt::Result {
            Ok(())
        }
    }

    #[test]
    fn divergence_equal_markings_is_match() {
        assert_eq!(divergence(&StubScheme, &0b111, &0b111), Divergence::Match);
    }

    #[test]
    fn divergence_front_superset_over_claims() {
        // Front carries every bit the body does, plus more → front dominates.
        assert_eq!(
            divergence(&StubScheme, &0b111, &0b011),
            Divergence::FrontOverClaims,
        );
    }

    #[test]
    fn divergence_front_subset_under_claims() {
        // The body carries a bit (0b100) the front omits → under-claim.
        assert_eq!(
            divergence(&StubScheme, &0b011, &0b111),
            Divergence::FrontUnderClaims,
        );
    }

    #[test]
    fn divergence_incomparable_is_under_claims() {
        // Each carries a bit the other lacks; the body still carries content
        // the front fails to cover, so the safe verdict is under-claim.
        assert_eq!(
            divergence(&StubScheme, &0b010, &0b001),
            Divergence::FrontUnderClaims,
        );
    }

    #[test]
    fn divergence_variants_are_distinct() {
        assert_ne!(Divergence::Match, Divergence::FrontUnderClaims);
        assert_ne!(Divergence::Match, Divergence::FrontOverClaims);
        assert_ne!(Divergence::Match, Divergence::Unresolved);
        assert_ne!(Divergence::FrontUnderClaims, Divergence::FrontOverClaims);
        assert_ne!(Divergence::FrontUnderClaims, Divergence::Unresolved);
        assert_ne!(Divergence::FrontOverClaims, Divergence::Unresolved);
    }

    fn front_node(value: Option<u32>, fixability: Fixability) -> ResolvedArtifact<StubScheme> {
        ResolvedArtifact {
            kind: ArtifactKind::FrontMarking,
            fixability,
            derived_value: value,
            fired_edges: Box::new([]),
        }
    }

    #[test]
    fn reverse_validation_carries_verdict_and_node() {
        let rv: ReverseValidation<StubScheme> = ReverseValidation {
            divergence: Divergence::FrontUnderClaims,
            front: front_node(None, Fixability::FlagOnly),
        };
        assert_eq!(rv.divergence, Divergence::FrontUnderClaims);
        assert_eq!(rv.front.kind, ArtifactKind::FrontMarking);
        assert_eq!(rv.front.fixability, Fixability::FlagOnly);
    }

    /// Pins that the manual `PartialEq` impl compares both fields: an
    /// identical copy is equal, and a sibling differing only in `divergence`
    /// or only in `front` is unequal.
    #[test]
    fn reverse_validation_eq_distinguishes_each_field() {
        let base = || ReverseValidation::<StubScheme> {
            divergence: Divergence::Match,
            front: front_node(Some(1), Fixability::Fixable),
        };

        assert_eq!(base(), base());

        let mut differs = base();
        differs.divergence = Divergence::FrontOverClaims;
        assert_ne!(base(), differs);

        let mut differs = base();
        differs.front = front_node(Some(2), Fixability::Fixable);
        assert_ne!(base(), differs);
    }

    #[test]
    fn reverse_validation_clone_and_debug_are_content_ignorant() {
        let rv: ReverseValidation<StubScheme> = ReverseValidation {
            divergence: Divergence::FrontOverClaims,
            front: front_node(Some(9), Fixability::Fixable),
        };
        let cloned = rv.clone();
        assert_eq!(cloned, rv);

        // Debug carries only structural tokens — the verdict tag, the node
        // kind, and structural canonical/fixability — no free-form content.
        let dbg = format!("{rv:?}");
        assert!(dbg.contains("ReverseValidation"));
        assert!(dbg.contains("FrontOverClaims"));
        assert!(dbg.contains("FrontMarking"));
    }
}
