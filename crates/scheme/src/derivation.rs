// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Derivation edges — the inbound relations into a document-scoped artifact
//! node.
//!
//! A [`DerivationEdge`] is the document-scope analogue of a
//! [`PageRewrite`](crate::page_rewrite::PageRewrite): it declares a
//! `reads` / `writes` dataflow footprint so the engine's Kahn scheduler
//! (Phase C) can order edges and page rewrites in one topological pass. The
//! topology is **static** — every edge a scheme declares stays in the DAG
//! and is validated at `Engine::new`. Whether an edge actually *fires* at
//! evaluation time is a separate, conditional question gated by the edge's
//! [`FiringPredicate`] (research D3): a declared edge is never swapped out
//! of the graph; only its firing is conditional.
//!
//! Like `PageRewrite`, an edge carries a typed [`Citation`] so the
//! authoritative-source provenance for the derivation is recorded with the
//! relation, not bolted on elsewhere (Constitution VIII).

use crate::category::CategoryId;
use crate::citation::Citation;

/// Stable identifier for a [`DerivationEdge`]. Alias for `&'static str`,
/// mirroring [`RewriteId`](crate::page_rewrite::RewriteId).
///
/// Convention: `"scheme/snake-case-description"` (e.g.,
/// `"capco/cab-rolls-up-from-portions"`).
pub type EdgeId = &'static str;

/// An inbound derivation relation into a document-scoped artifact node.
///
/// The topology is static: the edge is always declared and validated at
/// engine construction. The [`FiringPredicate`] only gates whether the
/// edge fires during a given evaluation — it never removes the edge from
/// the DAG (research D3, "always-declared" edges).
///
/// `reads` / `writes` mirror [`PageRewrite`](crate::page_rewrite::PageRewrite)'s
/// shape so a single Kahn scheduler can order derivation edges and page
/// rewrites together (Phase C). `reads` are the categories the derivation
/// consumes; `writes` are the categories (or whole-marking sentinel) the
/// derived node populates.
///
/// Every field is `Copy` (`&'static str`, `Copy` enums, `Copy` slices, a
/// `Copy` [`Citation`]), so `Debug` / `Clone` / `PartialEq` / `Eq` derive
/// cleanly. `Clone` matters because [`DocumentArtifact`](crate::artifact::DocumentArtifact)
/// holds `Box<[DerivationEdge]>` and clones it when the node clones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivationEdge {
    /// Stable identifier. Surfaced in diagnostics and audit records.
    /// Convention: `"scheme/snake-case-description"`.
    pub id: EdgeId,
    /// The kind of derivation relation this edge expresses.
    pub relation: DerivationRelation,
    /// The edge's typed authoritative-source citation — the §-reference
    /// that governs the derivation. Carried on the edge (matching
    /// `PageRewrite::citation`) so propagation re-verification has a single
    /// home (Constitution VIII).
    pub citation: Citation,
    /// Categories this edge consumes. Feeds the scheduler
    /// (writers-before-readers ordering).
    pub reads: &'static [CategoryId],
    /// Categories this edge populates. Feeds the scheduler.
    pub writes: &'static [CategoryId],
    /// Always declared; the predicate gates whether the edge fires at
    /// evaluation time (including mode-gated firing).
    pub firing: FiringPredicate,
}

impl DerivationEdge {
    /// Construct a derivation edge.
    ///
    /// `const fn` so scheme authors can declare their edge tables as
    /// `&'static` constants — matching the `PageRewrite` constructor
    /// posture (the scheme's rewrite / edge tables are `const` at
    /// scheme-construction time, walked by the scheduler without owning).
    pub const fn new(
        id: EdgeId,
        relation: DerivationRelation,
        citation: Citation,
        reads: &'static [CategoryId],
        writes: &'static [CategoryId],
        firing: FiringPredicate,
    ) -> Self {
        Self {
            id,
            relation,
            citation,
            reads,
            writes,
            firing,
        }
    }
}

/// The kind of derivation relation a [`DerivationEdge`] expresses.
///
/// `#[non_exhaustive]` reserves grow-path for future relations.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DerivationRelation {
    /// The target node is a roll-up of the document's portions — e.g. a
    /// banner / CAB derived from the page's portion markings.
    Rollup,
    /// Presence of one node demands another: `X present ⇒ Y required`
    /// (e.g. a token whose presence makes a notice mandatory). Repairs
    /// "notice-iff-token" requirements.
    Requirement,
    /// The target document node is derived from a containing bundle's
    /// source markings (issue #823, source-derivation). Reserved; not
    /// wired at this phase.
    SourceDerived,
    /// The target node is a policy-mandated literal string — e.g. a
    /// §E.4 / §E.5 canned notice whose text is fixed by policy.
    CannedString,
    /// The target node passes a value through unchanged from its source.
    Passthrough,
}

/// When a declared [`DerivationEdge`] actually fires.
///
/// The edge is **always declared** — it stays in the DAG and is validated
/// at `Engine::new` regardless of this predicate (research D3). This
/// predicate only gates firing at evaluation time. It never causes a
/// topology swap.
///
/// The mode-gated variant carries a stable `&'static str` mode label rather
/// than an engine / mode type, because `marque-scheme` is the domain-neutral
/// leaf and MUST NOT pull in engine types. The engine resolves the label at
/// evaluation time.
///
/// `#[non_exhaustive]` reserves grow-path for richer firing predicates.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FiringPredicate {
    /// The edge always fires when scheduled.
    Always,
    /// The edge fires only when the engine is operating in the named mode.
    /// The label is a stable `&'static str` the engine resolves — kept
    /// minimal to avoid pulling engine / mode types into the leaf crate.
    WhenMode(&'static str),
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::AuthoritativeSource;
    use crate::SectionLetter;
    use crate::SectionRef;
    use core::num::NonZeroU16;

    // Test fixture: sentinel `Citation` routed through
    // `AuthoritativeSource::EngineInternal` so Display omits §/page and the
    // value carries no source-relative claim (matches page_rewrite.rs
    // test-citation construction).
    fn test_citation() -> Citation {
        Citation::new(
            AuthoritativeSource::EngineInternal,
            SectionRef::new(SectionLetter::A),
            NonZeroU16::new(1).unwrap(),
        )
    }

    // Static category-axis tables — `reads` / `writes` must be `&'static`.
    const READS: &[CategoryId] = &[CategoryId(1)];
    const WRITES: &[CategoryId] = &[CategoryId(2)];

    #[test]
    fn edge_declares_reads_and_writes() {
        let edge = DerivationEdge::new(
            "test/rollup",
            DerivationRelation::Rollup,
            test_citation(),
            READS,
            WRITES,
            FiringPredicate::Always,
        );
        assert_eq!(edge.id, "test/rollup");
        assert_eq!(edge.relation, DerivationRelation::Rollup);
        assert_eq!(edge.reads, READS);
        assert_eq!(edge.writes, WRITES);
        assert_eq!(edge.firing, FiringPredicate::Always);
    }

    #[test]
    fn firing_always_differs_from_when_mode() {
        // The two firing predicates must be distinguishable so the engine
        // can decide whether a declared edge fires this run.
        assert_ne!(FiringPredicate::Always, FiringPredicate::WhenMode("strict"));
    }

    #[test]
    fn when_mode_carries_label() {
        let p = FiringPredicate::WhenMode("derivative");
        match p {
            FiringPredicate::WhenMode(label) => assert_eq!(label, "derivative"),
            FiringPredicate::Always => panic!("wrong variant"),
        }
    }

    #[test]
    fn when_mode_labels_are_compared_by_value() {
        assert_eq!(
            FiringPredicate::WhenMode("a"),
            FiringPredicate::WhenMode("a")
        );
        assert_ne!(
            FiringPredicate::WhenMode("a"),
            FiringPredicate::WhenMode("b")
        );
    }

    #[test]
    fn derivation_relation_variants_are_distinct() {
        let all = [
            DerivationRelation::Rollup,
            DerivationRelation::Requirement,
            DerivationRelation::SourceDerived,
            DerivationRelation::CannedString,
            DerivationRelation::Passthrough,
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

    #[test]
    fn edge_carries_citation() {
        let edge = DerivationEdge::new(
            "test/requirement",
            DerivationRelation::Requirement,
            test_citation(),
            READS,
            WRITES,
            FiringPredicate::WhenMode("strict"),
        );
        // The citation is carried on the edge (matching PageRewrite).
        assert_eq!(edge.citation, test_citation());
    }

    // Compile-time pin: `DerivationEdge::new` MUST be `const fn` so scheme
    // authors can declare `&'static` edge tables. If it stops being const,
    // this fails at compile time.
    const _EDGE: DerivationEdge = DerivationEdge::new(
        "test/const",
        DerivationRelation::Passthrough,
        Citation::new(
            AuthoritativeSource::EngineInternal,
            SectionRef::new(SectionLetter::A),
            match NonZeroU16::new(1) {
                Some(n) => n,
                None => unreachable!(),
            },
        ),
        READS,
        WRITES,
        FiringPredicate::Always,
    );
}
