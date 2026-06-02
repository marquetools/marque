// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Document-scope derivation cascade tests (issue #799).
//!
//! Drives `Engine::resolve_document`'s decision-tracing emission through a
//! synthetic [`marque_scheme::MarkingScheme`] that declares document
//! artifacts and chained derivation edges. Each firing edge emits exactly
//! one content-ignorant [`DecisionEvent`] with `kind == Derived`, threaded
//! through `triggered_by` so a [`RecordingSink`] can reconstruct the
//! derivation chain. The CAPCO no-op case pins that a scheme declaring no
//! document artifacts emits nothing.

#![cfg(feature = "decision-tracing")]

use std::sync::{Arc, Mutex};

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{CapcoEngine, Engine, EngineConstructionError, SystemClock};
use marque_rules::{ConstraintBridge, RuleSet};
use marque_scheme::recognizer::{ParseContext, Recognizer};
use marque_scheme::{
    ArtifactKind, Category, CategoryId, Citation, Constraint, ConstraintViolation, DecisionEvent,
    DecisionKind, DecisionSink, DecisionSite, DerivationEdge, DerivationRelation, FiringPredicate,
    JoinSemilattice, MarkingScheme, MeetSemilattice, Parsed, RecordingSink, Scope, SectionLetter,
    Template, TokenId, TokenRef,
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

// Category axes for the chained edges. Each artifact kind below maps to one
// of these so `document_artifacts()` is non-empty (else `resolve_document`
// early-returns before the cascade helper).
const CAT_X: CategoryId = CategoryId(1);
const CAT_Y: CategoryId = CategoryId(2);
const CAT_Z: CategoryId = CategoryId(3);
const CAT_W: CategoryId = CategoryId(4);

// ---------------------------------------------------------------------------
// StubMarking / StubScheme — declares document artifacts + chained edges.
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

// Four artifact kinds, mapped one-to-one onto CAT_X/Y/Z/W below.
const ARTIFACTS: &[ArtifactKind] = &[
    ArtifactKind::AuthorityBlock,
    ArtifactKind::Notice,
    ArtifactKind::DeclassifyInstruction,
    ArtifactKind::CaveatLayer,
];

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
        "stub-cascade"
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
            ArtifactKind::AuthorityBlock => Some(CAT_X),
            ArtifactKind::Notice => Some(CAT_Y),
            ArtifactKind::DeclassifyInstruction => Some(CAT_Z),
            ArtifactKind::CaveatLayer => Some(CAT_W),
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
/// answer. Keeps `lint()` off the scheme's canonical-conversion methods, so
/// the pipeline runs to end-of-document and computes `resolve_document`
/// against the default rollup.
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

/// A `Rollup` edge with explicit `reads`/`writes`, so the cascade's
/// `triggered_by` threading can be exercised across a chain.
fn chained_edge(
    id: &'static str,
    reads: &'static [CategoryId],
    writes: &'static [CategoryId],
) -> DerivationEdge {
    DerivationEdge::new(
        id,
        DerivationRelation::Rollup,
        TEST_CITATION,
        reads,
        writes,
        FiringPredicate::Always,
    )
}

/// A `Rollup` edge gated on a deployment mode, so the firing filter can be
/// exercised when the mode is not active.
fn when_mode_edge(
    id: &'static str,
    reads: &'static [CategoryId],
    writes: &'static [CategoryId],
    mode: &'static str,
) -> DerivationEdge {
    DerivationEdge::new(
        id,
        DerivationRelation::Rollup,
        TEST_CITATION,
        reads,
        writes,
        FiringPredicate::WhenMode(mode),
    )
}

// Static read/write axis tables (`reads`/`writes` must be `&'static`).
const READS_NONE: &[CategoryId] = &[];
const READS_X: &[CategoryId] = &[CAT_X];
const READS_Y: &[CategoryId] = &[CAT_Y];
const READS_YZ: &[CategoryId] = &[CAT_Y, CAT_Z];
const WRITES_X: &[CategoryId] = &[CAT_X];
const WRITES_XY: &[CategoryId] = &[CAT_X, CAT_Y];
const WRITES_Y: &[CategoryId] = &[CAT_Y];
const WRITES_Z: &[CategoryId] = &[CAT_Z];
const WRITES_W: &[CategoryId] = &[CAT_W];

/// Shared-mutable capture sink — pushes every event into an
/// `Arc<Mutex<Vec<_>>>` the test retains a clone of, so the event stream is
/// reachable after the sink moves into [`Engine::with_decision_sink`].
#[derive(Clone)]
struct Inspectable {
    events: Arc<Mutex<Vec<DecisionEvent>>>,
}

impl DecisionSink for Inspectable {
    fn record(&mut self, event: DecisionEvent) {
        // Fail fast on a poisoned mutex: poisoning means an earlier panic, and
        // silently dropping events would mask the real failure downstream.
        self.events
            .lock()
            .expect("capture sink mutex not poisoned")
            .push(event);
    }
}

/// The three-edge chain A → B → C, each writing a distinct category that the
/// next reads.
fn chain_edges() -> Vec<DerivationEdge> {
    vec![
        chained_edge("stub/edge-a", READS_NONE, WRITES_X),
        chained_edge("stub/edge-b", READS_X, WRITES_Y),
        chained_edge("stub/edge-c", READS_Y, WRITES_Z),
    ]
}

/// Collect the captured events, filtered to `kind == Derived`, after a lint.
fn derived_events(events: &Arc<Mutex<Vec<DecisionEvent>>>) -> Vec<DecisionEvent> {
    events
        .lock()
        .expect("mutex not poisoned")
        .iter()
        .copied()
        .filter(|e| matches!(e.kind, DecisionKind::Derived))
        .collect()
}

/// Find the single derived event whose source edge id matches `id`.
fn event_for(derived: &[DecisionEvent], id: &str) -> DecisionEvent {
    let matching: Vec<DecisionEvent> = derived
        .iter()
        .copied()
        .filter(|e| matches!(e.source, marque_scheme::DecisionSource::Derivation(eid) if eid == id))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one derived event for edge {id:?}, got {}",
        matching.len(),
    );
    matching[0]
}

#[test]
fn sc007_cascade_chain_reconstructs_three_levels() {
    let events: Arc<Mutex<Vec<DecisionEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let engine = build(StubScheme::with_edges(chain_edges()))
        .expect("acyclic edge set builds")
        .with_decision_sink(Inspectable {
            events: events.clone(),
        });

    let _ = engine.lint(b"text with no markings\n");

    let derived = derived_events(&events);
    assert_eq!(
        derived.len(),
        3,
        "exactly one derived event per firing edge (A, B, C)",
    );

    let a = event_for(&derived, "stub/edge-a");
    let b = event_for(&derived, "stub/edge-b");
    let c = event_for(&derived, "stub/edge-c");

    // Every derived event sits at the document site and writes its edge's
    // category.
    for e in [a, b, c] {
        assert_eq!(e.site, DecisionSite::Document);
    }
    assert_eq!(a.category, CAT_X);
    assert_eq!(b.category, CAT_Y);
    assert_eq!(c.category, CAT_Z);

    // Cascade threading: A is a root; B points at A; C points at B.
    assert_eq!(a.triggered_by, None);
    assert_eq!(b.triggered_by, Some(a.step));
    assert_eq!(c.triggered_by, Some(b.step));

    // Reconstruct the chain from the captured stream.
    let report = RecordingSink::into_report_from_events(derived.clone());
    assert_eq!(
        report.cascade_chains.len(),
        1,
        "the three edges form a single cascade chain",
    );
    let chain = &report.cascade_chains[0];
    assert_eq!(chain.root_event, a.step);
    assert_eq!(chain.depth, 2);
    assert_eq!(chain.events.len(), 3);
}

#[test]
fn sc007_diamond_attributes_to_latest_dependency() {
    // A → B → C as before, plus D reading both Y (from B) and Z (from C).
    // D's single `triggered_by` parent is the latest-arriving dependency:
    // C (scheduled after B, so its step is larger), giving a spanning-tree
    // projection of the diamond DAG.
    let mut edges = chain_edges();
    edges.push(chained_edge("stub/edge-d", READS_YZ, WRITES_W));

    let events: Arc<Mutex<Vec<DecisionEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let engine = build(StubScheme::with_edges(edges))
        .expect("acyclic edge set builds")
        .with_decision_sink(Inspectable {
            events: events.clone(),
        });

    let _ = engine.lint(b"text with no markings\n");

    let derived = derived_events(&events);
    assert_eq!(derived.len(), 4, "one derived event per firing edge");

    let c = event_for(&derived, "stub/edge-c");
    let d = event_for(&derived, "stub/edge-d");
    assert_eq!(d.category, CAT_W);
    assert_eq!(
        d.triggered_by,
        Some(c.step),
        "a diamond reader attributes to its latest-arriving dependency (C)",
    );
}

#[test]
fn sc007_multi_write_edge_uses_marking_sentinel() {
    // An edge writing more than one category cannot honestly name a single
    // category, so its event carries the `MARKING` multi-category sentinel
    // rather than an arbitrary first write.
    let edges = vec![chained_edge("stub/edge-multi", READS_NONE, WRITES_XY)];

    let events: Arc<Mutex<Vec<DecisionEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let engine = build(StubScheme::with_edges(edges))
        .expect("acyclic edge set builds")
        .with_decision_sink(Inspectable {
            events: events.clone(),
        });

    let _ = engine.lint(b"text with no markings\n");

    let derived = derived_events(&events);
    assert_eq!(derived.len(), 1, "the single multi-write edge emits once");
    let e = event_for(&derived, "stub/edge-multi");
    assert_eq!(e.category, CategoryId::MARKING);
}

#[test]
fn sc007_inactive_when_mode_edge_emits_no_event() {
    // A `WhenMode` edge whose mode is not active is filtered out of the
    // firing set, so the cascade records nothing for it — only the
    // always-firing edge emits. The engine starts with an empty active-mode
    // set, so "stub/mode/never-active" never fires.
    let edges = vec![
        chained_edge("stub/edge-a", READS_NONE, WRITES_X),
        when_mode_edge(
            "stub/edge-gated",
            READS_X,
            WRITES_Y,
            "stub/mode/never-active",
        ),
    ];

    let events: Arc<Mutex<Vec<DecisionEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let engine = build(StubScheme::with_edges(edges))
        .expect("acyclic edge set builds")
        .with_decision_sink(Inspectable {
            events: events.clone(),
        });

    let _ = engine.lint(b"text with no markings\n");

    let derived = derived_events(&events);
    assert_eq!(derived.len(), 1, "only the always-firing edge emits");
    let a = event_for(&derived, "stub/edge-a");
    assert_eq!(a.category, CAT_X);
    assert_eq!(a.triggered_by, None);
    assert!(
        !derived.iter().any(|e| matches!(
            e.source,
            marque_scheme::DecisionSource::Derivation(id) if id == "stub/edge-gated"
        )),
        "the inactive WhenMode edge records no derivation event",
    );
}

#[test]
fn sc007_derivation_emits_nothing_without_observer() {
    // No `with_decision_sink` — the engine keeps its default NoopSink, so
    // the `tracing_active` gate short-circuits the cascade helper. The
    // resolution result on the lint output must still be correct: emission
    // is side-effect-free on the returned ResolvedDocument.
    let engine = build(StubScheme::with_edges(chain_edges())).expect("acyclic edge set builds");

    let result = engine.lint(b"text with no markings\n");
    assert!(
        !result.resolved_document.is_empty(),
        "a scheme that declares document artifacts must resolve to a non-empty document",
    );
    assert!(
        result.resolved_document.artifacts().len() >= 3,
        "all four declared artifacts resolve regardless of the observer",
    );
}

#[test]
fn capco_emits_no_derivation_events() {
    // CAPCO declares no document artifacts, so `resolve_document`
    // early-returns before the cascade helper. No `Derived`-kind event may
    // appear in the captured stream (G13 no-op guard).
    let events: Arc<Mutex<Vec<DecisionEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let engine: CapcoEngine = CapcoEngine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme constructs cleanly")
    .with_decision_sink(Inspectable {
        events: events.clone(),
    });

    let _ = engine.lint(b"SECRET//NOFORN\n\n(S//NF) representative portion text\n");

    let has_derived = events
        .lock()
        .expect("mutex not poisoned")
        .iter()
        .any(|e| matches!(e.kind, DecisionKind::Derived));
    assert!(
        !has_derived,
        "CAPCO declares no document artifacts; no DecisionKind::Derived event may be emitted",
    );
}
