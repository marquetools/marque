// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Document-scope resolution result types (issue #799).
//!
//! Resolution answers, for each document-scoped artifact a scheme declares,
//! whether the artifact is *derivable* from the document rollup and the
//! firing derivation edges — and, when it is, what the derived value would
//! be. Resolution is **decoupled from fixing**: it runs on every lint pass
//! (fixing off included) and produces only a classification of each node,
//! never an applied change. The engine computes a [`ResolvedDocument`] at
//! end-of-document and surfaces it on its lint result.
//!
//! These are pure-data result types: the algorithm that produces them lives
//! in the engine (it needs the scheduler order and the document rollup).
//!
//! # Constitution V Principle V (audit content-ignorance)
//!
//! Every field here carries only structural data — an [`ArtifactKind`] tag,
//! a [`Fixability`] tag, the firing [`EdgeId`]s, and the scheme's own
//! `Canonical` value (which is structural, not document bytes). Nothing
//! here carries free-form document content.

use crate::artifact::ArtifactKind;
use crate::derivation::EdgeId;
use crate::scheme::MarkingScheme;

/// Whether a resolved artifact node can be fixed automatically.
///
/// Fixability follows derivability: a node that an inbound firing
/// value-producing edge can populate is [`Fixable`](Self::Fixable); a node
/// that no edge produces is [`FlagOnly`](Self::FlagOnly) — the engine can
/// surface it but cannot derive a value to fill it.
///
/// `#[non_exhaustive]` reserves grow-path (e.g. a future partial-fix tier).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fixability {
    /// At least one firing value-producing derivation edge can populate
    /// this node — a derived value exists, so the node is fixable.
    Fixable,
    /// No firing value-producing edge populates this node. The engine can
    /// surface the node but cannot derive a value for it.
    FlagOnly,
}

/// A single resolved document-scoped artifact node.
///
/// `Debug` / `Clone` / `PartialEq` are written manually rather than derived
/// because a blanket derive over `S` would over-constrain — it would demand
/// `S: Debug + Clone + ...` even though only `S::Canonical` actually needs
/// the bound (the same pattern used for
/// [`DocumentArtifact`](crate::artifact::DocumentArtifact) in
/// `artifact.rs`).
pub struct ResolvedArtifact<S: MarkingScheme + ?Sized> {
    /// What kind of artifact this resolved node is.
    pub kind: ArtifactKind,
    /// Whether the node can be fixed automatically (derivable) or only
    /// flagged.
    pub fixability: Fixability,
    /// The derived value when the node is [`Fixability::Fixable`] via a
    /// value-producing edge; `None` for [`Fixability::FlagOnly`] nodes.
    pub derived_value: Option<S::Canonical>,
    /// The ids of the firing edges that produce this node (the edges whose
    /// `writes` axis contains the node's category and whose firing
    /// predicate is active this run).
    pub fired_edges: Box<[EdgeId]>,
}

impl<S: MarkingScheme + ?Sized> core::fmt::Debug for ResolvedArtifact<S>
where
    S::Canonical: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResolvedArtifact")
            .field("kind", &self.kind)
            .field("fixability", &self.fixability)
            .field("derived_value", &self.derived_value)
            .field("fired_edges", &self.fired_edges)
            .finish()
    }
}

impl<S: MarkingScheme + ?Sized> Clone for ResolvedArtifact<S>
where
    S::Canonical: Clone,
{
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            fixability: self.fixability,
            derived_value: self.derived_value.clone(),
            fired_edges: self.fired_edges.clone(),
        }
    }
}

impl<S: MarkingScheme + ?Sized> PartialEq for ResolvedArtifact<S>
where
    S::Canonical: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.fixability == other.fixability
            && self.derived_value == other.derived_value
            && self.fired_edges == other.fired_edges
    }
}

/// The document-scope resolution result — one [`ResolvedArtifact`] per
/// derivable-or-flagged document-scoped artifact node the scheme declares.
///
/// A scheme that declares no document artifacts resolves to an empty
/// document ([`Self::is_empty`] is `true`). `Debug` / `Clone` / `PartialEq`
/// are manual for the same reason as [`ResolvedArtifact`]; `Default`
/// produces the empty document with no `S: Default` bound.
pub struct ResolvedDocument<S: MarkingScheme + ?Sized> {
    artifacts: Box<[ResolvedArtifact<S>]>,
}

impl<S: MarkingScheme + ?Sized> ResolvedDocument<S> {
    /// Construct a resolved document from its artifact nodes.
    pub fn new(artifacts: Box<[ResolvedArtifact<S>]>) -> Self {
        Self { artifacts }
    }

    /// The resolved artifact nodes.
    pub fn artifacts(&self) -> &[ResolvedArtifact<S>] {
        &self.artifacts
    }

    /// Whether this resolution produced no artifact nodes — `true` for a
    /// scheme that declares no document artifacts (the CAPCO no-op case).
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }
}

impl<S: MarkingScheme + ?Sized> core::fmt::Debug for ResolvedDocument<S>
where
    S::Canonical: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResolvedDocument")
            .field("artifacts", &self.artifacts)
            .finish()
    }
}

impl<S: MarkingScheme + ?Sized> Clone for ResolvedDocument<S>
where
    S::Canonical: Clone,
{
    fn clone(&self) -> Self {
        Self {
            artifacts: self.artifacts.clone(),
        }
    }
}

impl<S: MarkingScheme + ?Sized> PartialEq for ResolvedDocument<S>
where
    S::Canonical: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.artifacts == other.artifacts
    }
}

// Manual `Default` — the empty document. A `#[derive(Default)]` would
// demand `S: Default`, but the empty box needs no scheme value.
impl<S: MarkingScheme + ?Sized> Default for ResolvedDocument<S> {
    fn default() -> Self {
        Self {
            artifacts: Box::new([]),
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::ambiguity::Parsed;
    use crate::category::{Category, TokenId};
    use crate::constraint::{Constraint, ConstraintViolation};
    use crate::lattice::{JoinSemilattice, MeetSemilattice};
    use crate::render_context::RenderContext;
    use crate::scope::Scope;
    use crate::template::Template;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct StubMarking;

    impl JoinSemilattice for StubMarking {
        fn join(&self, _: &Self) -> Self {
            Self
        }
    }

    impl MeetSemilattice for StubMarking {
        fn meet(&self, _: &Self) -> Self {
            Self
        }
    }

    // A closed-vocab stub scheme whose `Canonical = u32`, so derived
    // values are assertable.
    struct StubScheme;

    impl crate::scheme::MarkingScheme for StubScheme {
        type Token = TokenId;
        type Marking = StubMarking;
        type ParseError = ();
        type OpenVocabRef = core::convert::Infallible;
        type Parsed<'src> = ();
        type Canonical = u32;
        type Projected = ();

        fn name(&self) -> &str {
            "stub"
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
            StubMarking
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
    fn fixability_variants_are_distinct() {
        assert_ne!(Fixability::Fixable, Fixability::FlagOnly);
    }

    #[test]
    fn resolved_artifact_carries_derived_value() {
        let node: ResolvedArtifact<StubScheme> = ResolvedArtifact {
            kind: ArtifactKind::AuthorityBlock,
            fixability: Fixability::Fixable,
            derived_value: Some(7),
            fired_edges: Box::new(["stub/rollup"]),
        };
        assert_eq!(node.derived_value, Some(7));
        assert_eq!(node.fixability, Fixability::Fixable);
        assert_eq!(node.fired_edges.as_ref(), &["stub/rollup"]);
    }

    #[test]
    fn flag_only_artifact_omits_derived_value() {
        let node: ResolvedArtifact<StubScheme> = ResolvedArtifact {
            kind: ArtifactKind::Notice,
            fixability: Fixability::FlagOnly,
            derived_value: None,
            fired_edges: Box::new([]),
        };
        assert_eq!(node.derived_value, None);
        assert_eq!(node.fixability, Fixability::FlagOnly);
        assert!(node.fired_edges.is_empty());
    }

    #[test]
    fn resolved_document_holds_nodes_and_reports_emptiness() {
        let empty: ResolvedDocument<StubScheme> = ResolvedDocument::default();
        assert!(empty.is_empty());
        assert!(empty.artifacts().is_empty());

        let node: ResolvedArtifact<StubScheme> = ResolvedArtifact {
            kind: ArtifactKind::FrontMarking,
            fixability: Fixability::Fixable,
            derived_value: Some(3),
            fired_edges: Box::new(["stub/edge"]),
        };
        let doc: ResolvedDocument<StubScheme> = ResolvedDocument::new(Box::new([node]));
        assert!(!doc.is_empty());
        assert_eq!(doc.artifacts().len(), 1);
        assert_eq!(doc.artifacts()[0].kind, ArtifactKind::FrontMarking);
    }

    #[test]
    fn default_resolved_document_is_empty() {
        let doc: ResolvedDocument<StubScheme> = ResolvedDocument::default();
        assert!(doc.is_empty());
        assert_eq!(doc, ResolvedDocument::default());
    }

    /// Pins that the manual `PartialEq` impl on `ResolvedArtifact` compares
    /// every one of its four fields. Builds a baseline, asserts an
    /// identical copy is equal, then for each field builds a sibling that
    /// differs only in that field and asserts inequality. A future fifth
    /// field added but dropped from `eq` cannot pass once folded into the
    /// baseline.
    #[test]
    fn resolved_artifact_eq_distinguishes_each_field() {
        let base = || ResolvedArtifact::<StubScheme> {
            kind: ArtifactKind::AuthorityBlock,
            fixability: Fixability::Fixable,
            derived_value: Some(1),
            fired_edges: Box::new(["stub/a"]),
        };

        assert_eq!(base(), base());

        let mut differs = base();
        differs.kind = ArtifactKind::Notice;
        assert_ne!(base(), differs);

        let mut differs = base();
        differs.fixability = Fixability::FlagOnly;
        assert_ne!(base(), differs);

        let mut differs = base();
        differs.derived_value = Some(2);
        assert_ne!(base(), differs);

        let mut differs = base();
        differs.fired_edges = Box::new(["stub/b"]);
        assert_ne!(base(), differs);
    }

    #[test]
    fn resolved_artifact_clone_is_independent() {
        let node: ResolvedArtifact<StubScheme> = ResolvedArtifact {
            kind: ArtifactKind::CaveatLayer,
            fixability: Fixability::Fixable,
            derived_value: Some(42),
            fired_edges: Box::new(["stub/c"]),
        };
        let cloned = node.clone();
        assert_eq!(cloned, node);

        let dbg = format!("{node:?}");
        assert!(dbg.contains("ResolvedArtifact"));
        assert!(dbg.contains("CaveatLayer"));
    }

    #[test]
    fn resolved_document_clone_and_debug() {
        let doc: ResolvedDocument<StubScheme> =
            ResolvedDocument::new(Box::new([ResolvedArtifact {
                kind: ArtifactKind::DeclassifyInstruction,
                fixability: Fixability::FlagOnly,
                derived_value: None,
                fired_edges: Box::new([]),
            }]));
        let cloned = doc.clone();
        assert_eq!(cloned, doc);
        let dbg = format!("{doc:?}");
        assert!(dbg.contains("ResolvedDocument"));
    }
}
