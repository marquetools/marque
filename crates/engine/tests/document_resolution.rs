// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Document-scope resolution tests (issue #799).
//!
//! Drives `Engine::resolve_document` (and the lint pipeline that surfaces
//! its result on `LintResult`) through a synthetic
//! [`marque_scheme::MarkingScheme`] that declares document artifacts and
//! derivation edges. Modeled on `scheduler.rs`'s `StubScheme`, extended
//! with `document_artifacts()`, `artifact_category()`, and a non-trivial
//! `Canonical = u32` so derived values are assertable. Because
//! `marque-scheme` has no dependency on `marque-capco` (Constitution VII),
//! the stub lives here rather than exercising `CapcoScheme`.

use marque_config::Config;
use marque_engine::{Engine, EngineConstructionError, SystemClock};
use marque_rules::{ConstraintBridge, RuleSet};
use marque_scheme::recognizer::{ParseContext, Recognizer};
use marque_scheme::{
    ArtifactKind, Category, CategoryId, Citation, Constraint, ConstraintViolation, DerivationEdge,
    DerivationRelation, FiringPredicate, Fixability, JoinSemilattice, MarkingScheme,
    MeetSemilattice, Parsed, Scope, SectionLetter, Template, TokenId, TokenRef,
};

// Test-fixture sentinel Citation (Constitution V Principle V test
// carve-out). Routes through `AuthoritativeSource::EngineInternal` so
// Display renders `[engine-internal]` and the value carries no false CAPCO
// §-claim.
const TEST_CITATION: Citation = Citation::new(
    marque_scheme::AuthoritativeSource::EngineInternal,
    marque_scheme::SectionRef::new(SectionLetter::A),
    match core::num::NonZeroU16::new(1) {
        Some(n) => n,
        None => unreachable!(),
    },
);

// Category axes. CAT_A is written by a Rollup edge (so an artifact mapped
// to it is fixable); CAT_B has no edge (so an artifact mapped to it is
// flag-only).
const CAT_A: CategoryId = CategoryId(1);
const CAT_B: CategoryId = CategoryId(2);

// ---------------------------------------------------------------------------
// StubMarking / StubScheme — declares document artifacts + derivation edges.
// `Canonical = u32` so the rollup-derived value is assertable.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Default)]
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

/// The artifact kinds the scheme declares. `AuthorityBlock` maps to CAT_A
/// (written by a Rollup edge → fixable); `Notice` maps to CAT_B (no edge →
/// flag-only).
const ARTIFACTS: &[ArtifactKind] = &[ArtifactKind::AuthorityBlock, ArtifactKind::Notice];

struct StubScheme {
    edges: Vec<DerivationEdge>,
}

impl StubScheme {
    fn with_edges(edges: Vec<DerivationEdge>) -> Self {
        Self { edges }
    }
}

impl MarkingScheme for StubScheme {
    type Token = TokenId;
    type Marking = StubMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = u32;
    type Projected = ();

    fn name(&self) -> &str {
        "stub-resolution"
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
    fn satisfies(&self, _: &Self::Marking, _: &TokenRef) -> bool {
        false
    }
    fn validate(&self, _: &Self::Marking) -> Vec<ConstraintViolation> {
        vec![]
    }
    fn project(&self, _: Scope, _: &[Self::Marking]) -> Self::Marking {
        StubMarking
    }
    fn document_artifacts(&self) -> &[ArtifactKind] {
        ARTIFACTS
    }
    fn derivation_edges(&self) -> &[DerivationEdge] {
        &self.edges
    }
    fn artifact_category(&self, kind: ArtifactKind) -> Option<CategoryId> {
        match kind {
            ArtifactKind::AuthorityBlock => Some(CAT_A),
            ArtifactKind::Notice => Some(CAT_B),
            _ => None,
        }
    }
    fn render_item(&self, _: &Self::Marking) -> String {
        String::new()
    }
    fn render_summary(&self, _: &Self::Marking) -> String {
        String::new()
    }
    fn render_canonical(
        &self,
        _: &Self::Marking,
        _: &marque_scheme::RenderContext,
        _: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        Ok(())
    }
}

impl ConstraintBridge for StubScheme {}

/// Zero-candidate recognizer — the engine-safe "nothing recognized"
/// answer. Keeps `lint()` off the scheme's `unimplemented!()` canonical
/// conversion methods (no candidate is ever recognized), so the pipeline
/// runs to EOD and computes `resolve_document` against the default rollup.
struct StubRecognizer;

impl Recognizer<StubScheme> for StubRecognizer {
    fn recognize(
        &self,
        _bytes: &[u8],
        _offset: usize,
        _scheme: &StubScheme,
        _cx: &ParseContext,
    ) -> Parsed<StubMarking> {
        Parsed::Ambiguous {
            candidates: Vec::new(),
        }
    }
}

fn build(
    scheme: StubScheme,
) -> Result<Engine<StubScheme, StubRecognizer>, EngineConstructionError> {
    Engine::with_clock_and_recognizer(
        Config::default(),
        Vec::<Box<dyn RuleSet<StubScheme>>>::new(),
        scheme,
        StubRecognizer,
        Box::new(SystemClock),
    )
}

fn rollup_edge(
    id: &'static str,
    writes: &'static [CategoryId],
    firing: FiringPredicate,
) -> DerivationEdge {
    DerivationEdge::new(
        id,
        DerivationRelation::Rollup,
        TEST_CITATION,
        &[],
        writes,
        firing,
    )
}

const WRITES_A: &[CategoryId] = &[CAT_A];

/// Resolution is computed and surfaced on the lint result even though no
/// fix pass runs — `lint()` carries it. The scheme declares two artifacts,
/// so the resolved document is non-empty.
#[test]
fn resolution_present_with_fixing_off() {
    let scheme = StubScheme::with_edges(vec![rollup_edge(
        "stub/cab-rollup",
        WRITES_A,
        FiringPredicate::Always,
    )]);
    let engine = build(scheme).expect("acyclic edge set builds");

    let result = engine.lint(b"some text with no recognized markings\n");
    assert!(
        !result.resolved_document.is_empty(),
        "a scheme that declares document artifacts must resolve to a non-empty document",
    );
    assert_eq!(result.resolved_document.artifacts().len(), 2);
}

/// A `WhenMode` edge stays in the construction-time DAG (it appears in
/// `scheduled_steps()`) but is skipped at firing time when its mode is not
/// active — so the node it would produce resolves `FlagOnly`, not
/// `Fixable`. No topology swap: the edge is present, only its firing is
/// gated.
#[test]
fn when_mode_edge_in_dag_but_skipped() {
    let scheme = StubScheme::with_edges(vec![rollup_edge(
        "stub/cab-rollup-mode-gated",
        WRITES_A,
        FiringPredicate::WhenMode("derivative"),
    )]);
    let engine = build(scheme).expect("acyclic edge set builds");

    // The edge is in the construction-time DAG.
    use marque_engine::ScheduledStep;
    assert!(
        engine
            .scheduled_steps()
            .contains(&ScheduledStep::DerivationEdge("stub/cab-rollup-mode-gated")),
        "the WhenMode edge must remain in the scheduled DAG (no topology swap)",
    );

    // But because "derivative" mode is not active (no public setter; empty
    // default), the edge does not fire — the AuthorityBlock node it would
    // produce resolves FlagOnly.
    let resolved = engine.resolve_document(&7u32);
    let cab = resolved
        .artifacts()
        .iter()
        .find(|a| a.kind == ArtifactKind::AuthorityBlock)
        .expect("AuthorityBlock node must be present");
    assert_eq!(
        cab.fixability,
        Fixability::FlagOnly,
        "a WhenMode edge that does not fire leaves its node flag-only",
    );
    assert_eq!(cab.derived_value, None);
    assert!(cab.fired_edges.is_empty());
}

/// Paired harness: ONE scheme, two artifact kinds. Kind A
/// (AuthorityBlock → CAT_A, written by a firing Rollup edge) is `Fixable`
/// with a derived value; kind B (Notice → CAT_B, no edge writes it) is
/// `FlagOnly` with no derived value. The per-kind `artifact_category`
/// association is what keeps this honest: a blanket "any producing edge
/// fixes every kind" association would wrongly make B fixable.
#[test]
fn sc007_paired_fixable_and_flag_only() {
    let scheme = StubScheme::with_edges(vec![rollup_edge(
        "stub/cab-rollup",
        WRITES_A,
        FiringPredicate::Always,
    )]);
    let engine = build(scheme).expect("acyclic edge set builds");

    let resolved = engine.resolve_document(&99u32);

    let cab = resolved
        .artifacts()
        .iter()
        .find(|a| a.kind == ArtifactKind::AuthorityBlock)
        .expect("AuthorityBlock node");
    assert_eq!(cab.fixability, Fixability::Fixable);
    assert!(
        cab.derived_value.is_some(),
        "a derivable node carries the derived value",
    );

    let notice = resolved
        .artifacts()
        .iter()
        .find(|a| a.kind == ArtifactKind::Notice)
        .expect("Notice node");
    assert_eq!(notice.fixability, Fixability::FlagOnly);
    assert!(
        notice.derived_value.is_none(),
        "a node no edge produces carries no derived value",
    );
}

/// An absent node with an inbound firing Rollup edge resolves to the actual
/// derived value — the document rollup handed to `resolve_document`.
#[test]
fn rollup_node_returns_derived_value() {
    let scheme = StubScheme::with_edges(vec![rollup_edge(
        "stub/cab-rollup",
        WRITES_A,
        FiringPredicate::Always,
    )]);
    let engine = build(scheme).expect("acyclic edge set builds");

    let expected_rollup: u32 = 4242;
    let resolved = engine.resolve_document(&expected_rollup);
    let cab = resolved
        .artifacts()
        .iter()
        .find(|a| a.kind == ArtifactKind::AuthorityBlock)
        .expect("AuthorityBlock node");
    assert_eq!(
        cab.derived_value,
        Some(expected_rollup),
        "a Rollup-derived node returns the document rollup as its value",
    );
    assert_eq!(cab.fired_edges.as_ref(), &["stub/cab-rollup"]);
}

/// Deferral pin (#823): a `SourceDerived` edge is not a value-producing
/// relation in this phase — only `Rollup` is. A node whose sole inbound edge
/// is `SourceDerived` therefore resolves `FlagOnly` with no derived value.
/// This guards against a future bundle source-derivation wiring silently
/// starting to produce a value before the #823 source-metadata adapter lands.
#[test]
fn source_derived_yields_no_value() {
    let scheme = StubScheme::with_edges(vec![DerivationEdge::new(
        "stub/cab-source-derived",
        DerivationRelation::SourceDerived,
        TEST_CITATION,
        &[],
        WRITES_A,
        FiringPredicate::Always,
    )]);
    let engine = build(scheme).expect("acyclic edge set builds");

    let resolved = engine.resolve_document(&555u32);
    let cab = resolved
        .artifacts()
        .iter()
        .find(|a| a.kind == ArtifactKind::AuthorityBlock)
        .expect("AuthorityBlock node");
    assert_eq!(
        cab.fixability,
        Fixability::FlagOnly,
        "SourceDerived is not yet a value-producing relation (#823)",
    );
    assert_eq!(cab.derived_value, None);
}
