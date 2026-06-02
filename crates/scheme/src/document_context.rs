// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Document-scoped rollup context.
//!
//! A [`DocumentContext`] is the document-scope analogue of the page roll-up:
//! the join over the per-page canonical rollups, plus the document-scoped
//! artifact nodes (CAB, declassify instruction, notices, front marking).
//!
//! ## "Analogue of PageContext" = the same fold, one scope up
//!
//! There is no `PageContext` struct in the tree; the page roll-up is the
//! generic lattice fold
//! [`MarkingScheme::canonical_page_join`](crate::scheme::MarkingScheme::canonical_page_join).
//! [`DocumentContext::from_pages`] re-applies that fold one scope up via
//! [`MarkingScheme::canonical_document_join`](crate::scheme::MarkingScheme::canonical_document_join):
//! pages → document is the identical semilattice join (research D12 / LV3),
//! so RELIDO-unanimity, NOFORN supersession, and JointSet disunity collapse
//! survive the page→document fold *for free* — the fold routes through the
//! same per-axis lattices the page join uses. The document fold MUST NOT
//! set-union the flat canonical fields; that "naive re-union" is exactly
//! what LV3 forbids (it would re-admit dominated tokens and lose RELIDO
//! unanimity).
//!
//! ## Constitution VII (placement)
//!
//! This type lives in `marque-scheme` (the domain-neutral graph leaf),
//! naming only `S::Canonical` and [`DocumentArtifact<S>`]. The
//! `DissemSet`/`JointSet` behavior stays entirely inside CAPCO's
//! `canonical_page_join → join_via_lattice`, reached through the trait, so
//! `marque-scheme` never names a domain type and the acyclic graph holds.

use crate::artifact::DocumentArtifact;
use crate::scheme::SchemeArtifacts;

/// A document-scoped rollup context.
///
/// Generic over the scheme `S` (via the opt-in [`SchemeArtifacts`]
/// extension trait) so the `artifacts` field can carry the scheme's own
/// [`DocumentArtifact<S>`] nodes. `SchemeArtifacts: MarkingScheme`, so
/// `S::Canonical` is in scope for the `rollup` field.
///
/// `Debug` / `Clone` / `PartialEq` / `Eq` are written manually (not derived)
/// because a blanket `#[derive]` over `S` would emit spurious
/// `where S: Debug + Clone + ...` bounds even though only the field
/// projections need them — the same pattern [`DocumentArtifact`] uses, and
/// the B3.3b `LintResult<S>` lesson. The bounds are kept minimal per impl:
/// `Clone` / `PartialEq` / `Eq` need both `S::Canonical` and
/// `S::ArtifactPayload`, but `Debug` renders `artifacts` as a node *count*
/// (never a payload), so it needs only `S::Canonical: Debug`.
///
/// There is intentionally **no** `Default`: a document context is built via
/// [`Self::from_pages`] (which needs the scheme to perform the fold; the
/// empty-`pages` case still routes through the fold and yields the canonical
/// bottom), so advertising `Default` would force `S::Canonical: Default` on
/// the container for no caller.
pub struct DocumentContext<S: SchemeArtifacts + ?Sized> {
    /// Document-level canonical rollup — the join over the page rollups.
    pub rollup: S::Canonical,
    /// Document-scoped artifact nodes (CAB, declassify instruction,
    /// notices, front marking). `Box<[T]>` (not `Vec`) per Constitution II:
    /// built once at document finalization, never grown — mirrors
    /// [`DocumentArtifact::inbound`].
    pub artifacts: Box<[DocumentArtifact<S>]>,
}

impl<S: SchemeArtifacts + ?Sized> DocumentContext<S> {
    /// Fold per-page canonical rollups into the document rollup.
    ///
    /// The fold IS
    /// [`MarkingScheme::canonical_document_join`](crate::scheme::MarkingScheme::canonical_document_join)
    /// — the page→document semilattice join. Artifact nodes are populated
    /// later (the C2 engine accumulator); `from_pages` leaves `artifacts`
    /// empty.
    pub fn from_pages(scheme: &S, pages: &[S::Canonical]) -> Self
    where
        S::Canonical: Clone + Default,
    {
        Self {
            rollup: scheme.canonical_document_join(pages),
            artifacts: Box::new([]),
        }
    }
}

impl<S: SchemeArtifacts + ?Sized> core::fmt::Debug for DocumentContext<S>
where
    S::Canonical: core::fmt::Debug,
{
    // `artifacts` renders as a node count, not its contents, so this impl
    // does not need `S::ArtifactPayload: Debug` (keeping the bound off lets
    // `DocumentContext<S>` be debug-formatted for payloads that aren't Debug).
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DocumentContext")
            .field("rollup", &self.rollup)
            .field(
                "artifacts",
                &format_args!("[{} node(s)]", self.artifacts.len()),
            )
            .finish()
    }
}

impl<S: SchemeArtifacts + ?Sized> Clone for DocumentContext<S>
where
    S::Canonical: Clone,
    S::ArtifactPayload: Clone,
{
    fn clone(&self) -> Self {
        Self {
            rollup: self.rollup.clone(),
            artifacts: self.artifacts.clone(),
        }
    }
}

impl<S: SchemeArtifacts + ?Sized> PartialEq for DocumentContext<S>
where
    S::Canonical: PartialEq,
    S::ArtifactPayload: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.rollup == other.rollup && self.artifacts == other.artifacts
    }
}

impl<S: SchemeArtifacts + ?Sized> Eq for DocumentContext<S>
where
    S::Canonical: Eq,
    S::ArtifactPayload: Eq,
{
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::artifact::{ArtifactKind, ArtifactState, DocumentArtifact};
    use crate::provenance::ValueDerivation;
    use crate::scope::Scope;
    use crate::span::Span;

    // Reuse the existing `ArtifactScheme` fake from the artifact module's
    // tests would require it to be exported; instead define an equivalent
    // local stub. `ArtifactPayload = u32` matches `artifact.rs`'s fake so
    // the manual-impl bounds are exercised against a concrete payload that
    // is NOT document content (Constitution V).
    use crate::ambiguity::Parsed;
    use crate::category::{Category, TokenId};
    use crate::constraint::{Constraint, ConstraintViolation};
    use crate::lattice::{JoinSemilattice, MeetSemilattice};
    use crate::template::Template;

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
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

    /// A scheme whose `Canonical` is a `Clone + Default` `u32`, so the
    /// defaulted `canonical_page_join` / `canonical_document_join` bodies
    /// are callable. The default `canonical_page_join` folds to the last
    /// element (or `Default` for an empty slice), which is the wiring
    /// `from_pages` exercises here.
    struct DocStubScheme;

    impl crate::scheme::MarkingScheme for DocStubScheme {
        type Token = TokenId;
        type Marking = FakeMarking;
        type ParseError = ();
        type OpenVocabRef = core::convert::Infallible;
        type Parsed<'src> = ();
        type Canonical = u32;
        type Projected = ();

        fn name(&self) -> &str {
            "doc-stub"
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

    impl SchemeArtifacts for DocStubScheme {
        // A small structural payload — NOT document bytes (Constitution V).
        type ArtifactPayload = u32;
    }

    fn sample_artifact() -> DocumentArtifact<DocStubScheme> {
        DocumentArtifact {
            kind: ArtifactKind::AuthorityBlock,
            scope: Scope::Document,
            state: ArtifactState::Present(7),
            derivation: ValueDerivation::Authored,
            inbound: Box::new([]),
            span: Some(Span::new(0, 10)),
        }
    }

    #[test]
    fn document_context_debug_clone_eq() {
        // Proves the manual Debug/Clone/PartialEq impls compile with the
        // field-projection bounds (no spurious `S: Trait`) and behave.
        let ctx: DocumentContext<DocStubScheme> = DocumentContext {
            rollup: 42,
            artifacts: Box::new([sample_artifact()]),
        };
        let cloned = ctx.clone();
        assert_eq!(cloned, ctx);
        let dbg = format!("{ctx:?}");
        assert!(dbg.contains("DocumentContext"), "Debug names the type");
        assert!(dbg.contains("node(s)"), "Debug summarizes artifacts");
    }

    #[test]
    fn document_context_eq_distinguishes_fields() {
        let base = || DocumentContext::<DocStubScheme> {
            rollup: 1,
            artifacts: Box::new([]),
        };
        assert_eq!(base(), base());

        // `rollup` participates in equality.
        let mut differs = base();
        differs.rollup = 2;
        assert_ne!(base(), differs);

        // `artifacts` participates in equality.
        let mut differs = base();
        differs.artifacts = Box::new([sample_artifact()]);
        assert_ne!(base(), differs);
    }

    #[test]
    fn document_context_artifacts_is_boxed_slice() {
        // Type-level assertion the field is `Box<[DocumentArtifact<S>]>`
        // (Constitution II — no over-allocating `Vec`).
        let ctx: DocumentContext<DocStubScheme> = DocumentContext {
            rollup: 0,
            artifacts: Box::new([]),
        };
        // Type-level proof the field is exactly `Box<[DocumentArtifact<S>]>`:
        // a helper that only accepts that owned type compiles only if the
        // field has it. Uses a clone so `ctx` is still usable afterward.
        fn expect_boxed_slice(_: Box<[DocumentArtifact<DocStubScheme>]>) {}
        expect_boxed_slice(ctx.artifacts.clone());
        assert!(ctx.artifacts.is_empty());
    }

    #[test]
    fn from_pages_empty_is_default_rollup() {
        // An empty document folds to the canonical bottom (Default), and
        // `from_pages` leaves the artifact slice empty.
        let scheme = DocStubScheme;
        let ctx = DocumentContext::from_pages(&scheme, &[]);
        assert_eq!(ctx.rollup, u32::default());
        assert!(ctx.artifacts.is_empty());
    }

    #[test]
    fn from_pages_single_is_identity() {
        // The default `canonical_page_join` (and thus
        // `canonical_document_join`) folds to the last element; a single
        // page therefore yields exactly that page's rollup.
        let scheme = DocStubScheme;
        let ctx = DocumentContext::from_pages(&scheme, &[99]);
        assert_eq!(ctx.rollup, 99);
        assert!(ctx.artifacts.is_empty());
    }

    #[test]
    fn from_pages_routes_through_canonical_document_join() {
        // With this stub's default (last-element) fold, a multi-page slice
        // resolves to the last page's rollup — confirms `from_pages` routes
        // through `canonical_document_join` and not, e.g., `pages[0]`. The
        // semantic lattice-join coverage lives in the CAPCO algebra tests
        // (`crates/capco/tests/document_rollup.rs`); this only pins wiring.
        let scheme = DocStubScheme;
        let ctx = DocumentContext::from_pages(&scheme, &[1, 2, 3]);
        assert_eq!(ctx.rollup, 3);
    }
}
