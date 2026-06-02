// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Reverse-validation tests (issue #799).
//!
//! Drives `Engine::reverse_validate` through a synthetic
//! [`marque_scheme::MarkingScheme`] whose canonical is a `u32` bitset, so the
//! canonical-space join is a genuine least-upper-bound (bitwise OR) and every
//! [`marque_scheme::Divergence`] branch is reachable. The motivating #799
//! case — a front marking declaring less than the document body rolls up to
//! (`(TS//SI-G//OC)` front vs `(TS//SI-G//OC/RELIDO)` body) — must report
//! `FrontUnderClaims`.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{CapcoEngine, Engine, EngineConstructionError, SystemClock, default_scheme};
use marque_ism::{CanonicalAttrs, Classification, MarkingClassification};
use marque_rules::{ConstraintBridge, RuleSet};
use marque_scheme::recognizer::{ParseContext, Recognizer};
use marque_scheme::{
    ArtifactKind, Category, Constraint, ConstraintViolation, DiffInput, DiffRelation, Divergence,
    Fixability, JoinSemilattice, MarkingScheme, MeetSemilattice, Parsed, Scope, Template, TokenId,
    TokenRef,
};

// Bit positions modeling a stacked marking. The motivating case adds RELIDO on
// top of an otherwise-identical front, so the body strictly exceeds the front.
const TS: u32 = 0b001;
const SI_G: u32 = 0b010;
const OC: u32 = 0b100;
const RELIDO: u32 = 0b1000;

// ---------------------------------------------------------------------------
// StubMarking / StubScheme — `Canonical = u32` bitset; `canonical_page_join`
// is bitwise OR (a real semilattice join) and `canonical_from_marking`
// projects the marking's bits, so the entry point can compare in canonical
// space.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Default)]
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
    fn satisfies(&self, _: &Self::Marking, _: &TokenRef) -> bool {
        false
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
    fn canonical_from_marking(&self, marking: &Self::Marking) -> Self::Canonical {
        marking.0
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

/// Zero-candidate recognizer — nothing is recognized from the byte buffer, so
/// `lint()` never touches the scheme's canonical conversion. The reverse-
/// validation operands are supplied directly to the entry point instead.
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

fn build() -> Result<Engine<StubScheme, StubRecognizer>, EngineConstructionError> {
    Engine::with_clock_and_recognizer(
        Config::default(),
        Vec::<Box<dyn RuleSet<StubScheme>>>::new(),
        StubScheme,
        StubRecognizer,
        Box::new(SystemClock),
    )
}

/// Build the operands for a document-level comparison: a `DiffInput` from two
/// unambiguous markings, carrying the banner-over-portions relation reused one
/// scope up. The comparison scope is implied by the operands — `DiffInput`
/// holds no `Scope`.
fn diff(front: u32, body: u32) -> DiffInput<StubMarking> {
    DiffInput {
        from: Parsed::Unambiguous(StubMarking(front)),
        to: Parsed::Unambiguous(StubMarking(body)),
        relation: DiffRelation::BannerOverPortions,
    }
}

#[test]
fn sc012_front_under_claim_reports_divergence() {
    // The motivating #799 case: a front declaring `(TS//SI-G//OC)` against a
    // body that rolls up to `(TS//SI-G//OC/RELIDO)`. The body carries RELIDO
    // the front omits, so the front under-claims.
    let engine = build().expect("stub engine builds");
    let front = TS | SI_G | OC;
    let body = TS | SI_G | OC | RELIDO;

    let result = engine.reverse_validate(&diff(front, body));
    assert_eq!(result.divergence, Divergence::FrontUnderClaims);
    assert_eq!(result.front.kind, ArtifactKind::FrontMarking);
}

#[test]
fn reverse_exact_match_reports_no_divergence() {
    let engine = build().expect("stub engine builds");
    let both = TS | SI_G | OC | RELIDO;

    let result = engine.reverse_validate(&diff(both, both));
    assert_eq!(result.divergence, Divergence::Match);
}

#[test]
fn reverse_over_claim_reports_over_classification() {
    // The front declares RELIDO the body never carries — over-classification
    // at the document front.
    let engine = build().expect("stub engine builds");
    let front = TS | SI_G | OC | RELIDO;
    let body = TS | SI_G | OC;

    let result = engine.reverse_validate(&diff(front, body));
    assert_eq!(result.divergence, Divergence::FrontOverClaims);
}

#[test]
fn reverse_ambiguous_operand_is_unresolved() {
    // An ambiguous front cannot be projected to a single canonical, so the
    // verdict is reported as unresolved rather than a guessed match.
    let engine = build().expect("stub engine builds");
    let ambiguous_diff = DiffInput {
        from: Parsed::Ambiguous {
            candidates: Vec::new(),
        },
        to: Parsed::Unambiguous(StubMarking(TS | SI_G)),
        relation: DiffRelation::BannerOverPortions,
    };

    let result = engine.reverse_validate(&ambiguous_diff);
    assert_eq!(result.divergence, Divergence::Unresolved);
    // An unresolved comparison cannot claim a fixable front — the node is
    // synthesized flag-only rather than derived from the (resolved) body.
    assert_eq!(result.front.fixability, Fixability::FlagOnly);
    assert_eq!(result.front.derived_value, None);
}

#[test]
fn reverse_both_operands_ambiguous_is_unresolved() {
    let engine = build().expect("stub engine builds");
    let both_ambiguous = DiffInput {
        from: Parsed::Ambiguous {
            candidates: Vec::new(),
        },
        to: Parsed::Ambiguous {
            candidates: Vec::new(),
        },
        relation: DiffRelation::BannerOverPortions,
    };

    let result = engine.reverse_validate(&both_ambiguous);
    assert_eq!(result.divergence, Divergence::Unresolved);
    assert_eq!(result.front.fixability, Fixability::FlagOnly);
}

#[test]
fn reverse_front_node_is_flag_only_without_derivation_edge() {
    // The stub declares no FrontMarking artifact and no derivation edge, so the
    // resolved front node is synthesized flag-only — no edge can populate it —
    // while the verdict is still computed from the two markings.
    let engine = build().expect("stub engine builds");

    let result = engine.reverse_validate(&diff(TS, TS | SI_G));
    assert_eq!(result.divergence, Divergence::FrontUnderClaims);
    assert_eq!(result.front.fixability, Fixability::FlagOnly);
    assert_eq!(result.front.derived_value, None);
    assert!(result.front.fired_edges.is_empty());
}

#[test]
fn sc012_capco_front_under_claim_on_real_scheme() {
    // Proves the entry point runs through CapcoScheme's production
    // `canonical_from_marking` + `canonical_document_join` (the §D.2 p28
    // max-classification roll-up lattice) + `CanonicalAttrs: Eq`, not just the
    // synthetic bitset stub. A front carrying no classification against a body
    // that rolls up to SECRET under-claims.
    let scheme = default_scheme();
    let engine: CapcoEngine = CapcoEngine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        default_scheme(),
    )
    .expect("default CAPCO scheme constructs");

    let front = scheme.marking_from_canonical(CanonicalAttrs::default());
    let body = scheme.marking_from_canonical(CanonicalAttrs {
        classification: Some(MarkingClassification::Us(Classification::Secret)),
        ..Default::default()
    });

    let diff = DiffInput {
        from: Parsed::Unambiguous(front),
        to: Parsed::Unambiguous(body),
        relation: DiffRelation::BannerOverPortions,
    };
    let result = engine.reverse_validate(&diff);
    assert_eq!(result.divergence, Divergence::FrontUnderClaims);
    // CAPCO declares no FrontMarking artifact, so the node is synthesized
    // flag-only — the verdict is still computed from the two markings.
    assert_eq!(result.front.fixability, Fixability::FlagOnly);
}
