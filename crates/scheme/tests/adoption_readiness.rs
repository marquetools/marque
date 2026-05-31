// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Scheme-adoption readiness compile test.
//!
//! Defines a minimal `StubScheme` that exercises every trait the
//! scheme-adapter contract publishes:
//!
//! - [`MarkingScheme`] — the central scheme trait.
//! - [`Vocabulary<S>`] — per-token metadata accessors.
//! - [`Codec<S>`] — round-trip serialization surface (no impl required;
//!   we declare the trait can be named).
//! - [`Recognizer<S>`] — pluggable recognizer surface.
//! - [`Constraint`] — declarative invariants.
//! - [`PageRewrite<S>`] — page-scope cross-category rewrites.
//!
//! ## What this test asserts
//!
//! 1. The trait surface is **closed** — every trait can be implemented
//!    against a fresh scheme without any engine-side dependency, no
//!    compile-error escapes, no missing default impls.
//! 2. The scheme crate is **self-contained** — this file imports only
//!    from `marque_scheme` (and `std`). Constitution VII forbids
//!    `marque-engine` / `marque-core` / `marque-rules` /
//!    `marque-ism` / `marque-capco` from this side of the dependency
//!    graph; an accidental import of any of those would compile-fail.
//! 3. Adoption-readiness pre-verification — a future second scheme (CUI,
//!    NATO, or a partner-national framework) can land without engine
//!    edits. If a trait change inadvertently requires adapter crates to
//!    reach into engine internals, this file stops compiling and the
//!    regression is caught early.
//!
//! ## What this test does NOT assert
//!
//! `StubScheme` does **not** implement a real grammar. The
//! `parse` / `decode` / `recognize` / `validate` methods all return
//! the safe-empty answer (zero-candidate `Ambiguous`, empty violation
//! list, identity `project`). The point is to show the trait surface
//! is closed against construction, not to demonstrate a working
//! parser.

use marque_scheme::ambiguity::{Candidate, Parsed};
use marque_scheme::category::{Category, CategoryId, TokenId};
use marque_scheme::citation::{AuthoritativeSource, Citation, SectionLetter, SectionRef};
use marque_scheme::codec::{Codec, CodecError};
use marque_scheme::constraint::{Constraint, TokenRef};
use marque_scheme::lattice::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};
use marque_scheme::page_rewrite::{CategoryAction, CategoryPredicate, PageRewrite};
use marque_scheme::recognizer::{ParseContext, Recognizer};
use marque_scheme::scheme::MarkingScheme;
use marque_scheme::scope::Scope;
use marque_scheme::template::Template;
use marque_scheme::vocabulary::{
    Authority, Deprecation, FormSet, OwnerProducer, OwnerProducerKind, PointOfContact,
    TokenMetadataFull, Vocabulary,
};

// ---------------------------------------------------------------------------
// Minimal marking type. One token id, lattice-trivial.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct StubMarking {
    /// A presence bit for the single sentinel token. `false` is bottom.
    has_token: bool,
}

impl JoinSemilattice for StubMarking {
    fn join(&self, other: &Self) -> Self {
        Self {
            has_token: self.has_token || other.has_token,
        }
    }
}

impl MeetSemilattice for StubMarking {
    fn meet(&self, other: &Self) -> Self {
        Self {
            has_token: self.has_token && other.has_token,
        }
    }
}

impl BoundedJoinSemilattice for StubMarking {
    fn bottom() -> Self {
        Self { has_token: false }
    }
}

impl BoundedMeetSemilattice for StubMarking {
    fn top() -> Self {
        Self { has_token: true }
    }
}

// ---------------------------------------------------------------------------
// Sentinel token + category ids so the constraint/rewrite catalogs
// have something concrete to point at.
// ---------------------------------------------------------------------------

const STUB_TOKEN: TokenId = TokenId(1);
const STUB_CATEGORY: CategoryId = CategoryId(1);

// `Constraint::Conflicts` needs two distinct tokens; the constraint
// can never fire because StubMarking only carries presence of one
// sentinel — but the catalog entry must be well-formed.
const OTHER_TOKEN: TokenId = TokenId(2);

// ---------------------------------------------------------------------------
// Scheme.
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct StubParseError;

impl std::fmt::Display for StubParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StubParseError")
    }
}
impl std::error::Error for StubParseError {}

struct StubScheme {
    constraints: [Constraint; 1],
    page_rewrites: [PageRewrite<StubScheme>; 1],
}

// Sentinel test citation for the readiness fixture. Routes through
// `AuthoritativeSource::EngineInternal` so Display renders
// `[engine-internal]` and the value carries no false CAPCO §-claim.
const STUB_CITATION: Citation = Citation::new(
    AuthoritativeSource::EngineInternal,
    SectionRef::new(SectionLetter::A),
    match core::num::NonZeroU16::new(1) {
        Some(n) => n,
        None => unreachable!(),
    },
);

impl StubScheme {
    fn new() -> Self {
        Self {
            constraints: [Constraint::Conflicts {
                left: TokenRef::Token(STUB_TOKEN),
                right: TokenRef::Token(OTHER_TOKEN),
                // `name` is the stable identifier (rule-style id);
                // `label` is the typed authoritative-source citation.
                // Keeping them semantically distinct in the readiness
                // fixture mirrors what every real scheme does — a future
                // scheme adapter that copies this stub gets the right
                // shape by default rather than collapsing the two fields.
                name: "stub/conflicts",
                label: STUB_CITATION,
                severity: None,
                span_anchor: None,
            }],
            // `PageRewrite::declarative` is the const-friendly
            // constructor. The trigger / action arms here are the
            // simplest forms that compile against the trait surface
            // without exercising the engine's scheduler — that's a
            // marque-engine concern, out of scope for the readiness
            // check.
            page_rewrites: [PageRewrite::declarative(
                "stub/noop-rewrite",
                STUB_CITATION,
                CategoryPredicate::Contains {
                    category: STUB_CATEGORY,
                    token: STUB_TOKEN,
                },
                CategoryAction::Clear {
                    category: STUB_CATEGORY,
                },
                STUB_REWRITE_AXES,
                STUB_REWRITE_AXES,
            )],
        }
    }
}

/// `&'static [CategoryId]` axes for the stub rewrite. Both
/// `reads` and `writes` cover the same single category — that's
/// fine for the readiness fixture (the engine's scheduler doesn't
/// reject self-loops, only cycles between distinct rewrites).
static STUB_REWRITE_AXES: &[CategoryId] = &[STUB_CATEGORY];

impl MarkingScheme for StubScheme {
    type Token = TokenId;
    type Marking = StubMarking;
    type ParseError = StubParseError;
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();

    fn name(&self) -> &str {
        "stub"
    }
    fn schema_version(&self) -> &str {
        "stub-1"
    }
    fn categories(&self) -> &[Category] {
        &[]
    }
    fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }
    fn templates(&self) -> &[Template] {
        &[]
    }
    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &self.page_rewrites
    }
    fn parse(&self, _input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        // Zero-candidate Ambiguous is the engine-safe "nothing
        // recognized" answer per the Recognizer / parser contract
        // (foundational-plan §"Never silent fallthrough").
        Ok(Parsed::Ambiguous {
            candidates: Vec::new(),
        })
    }
    fn satisfies(&self, marking: &Self::Marking, token_ref: &TokenRef) -> bool {
        match token_ref {
            TokenRef::Token(STUB_TOKEN) => marking.has_token,
            _ => false,
        }
    }
    fn project(&self, _scope: Scope, markings: &[Self::Marking]) -> Self::Marking {
        markings
            .iter()
            .fold(StubMarking::bottom(), |acc, m| acc.join(m))
    }
    fn render_item(&self, m: &Self::Marking) -> String {
        if m.has_token { "(STUB)" } else { "()" }.to_string()
    }
    fn render_summary(&self, m: &Self::Marking) -> String {
        if m.has_token { "STUB" } else { "" }.to_string()
    }
    fn render_canonical(
        &self,
        m: &Self::Marking,
        ctx: &marque_scheme::RenderContext,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        // Degenerate delegation — preserves byte-identity with the
        // existing `render_item` / `render_summary` overrides
        // above. `Scope::Diff` rejects per the trait contract.
        match ctx.scope {
            Scope::Portion => out.write_str(&self.render_item(m)),
            // Bundle ≡ Document for single-doc inputs; multi-doc mosaic is #823
            Scope::Page | Scope::Document | Scope::Bundle => out.write_str(&self.render_summary(m)),
            Scope::Diff => Err(core::fmt::Error),
        }
    }

    // `evaluate_custom` is intentionally NOT overridden. The trait
    // default (returning `Vec::new()`) is sufficient for any scheme
    // whose `Constraint` variants are exhausted by the standard
    // declarative set (`Forbids`, `Requires`, `Conflicts`, `Implies`,
    // `Supersedes`, `OneOf`). `StubScheme` declares one
    // `Constraint::Conflicts` (above) and nothing custom, so the
    // default holds. A scheme that needs bespoke constraint shapes
    // (e.g., a NATO scheme that needs to express
    // "this marking forbids a non-token property of the document
    // body") override `evaluate_custom` — see
    // `crates/scheme/tests/codec_surface.rs::MockScheme` for the
    // override pattern.
}

// ---------------------------------------------------------------------------
// Vocabulary impl — every accessor returns `&'static` data.
// ---------------------------------------------------------------------------

static STUB_AUTHORITY: Authority = Authority {
    source_name: "Stub Authority",
    urn: "urn:example:stub:scheme",
    schema_version: "stub-1",
    point_of_contact: PointOfContact {
        name: "Stub POC",
        email: "stub@example.invalid",
        organization: "Stub Org",
    },
};

static STUB_OWNER: OwnerProducer = OwnerProducer {
    code: "STUB",
    name: "Stub Org",
    kind: OwnerProducerKind::Organization,
};

static STUB_POC: PointOfContact = PointOfContact {
    name: "Stub POC",
    email: "stub@example.invalid",
    organization: "Stub Org",
};

static STUB_METADATA: TokenMetadataFull<TokenId> = TokenMetadataFull {
    canonical: "STUB",
    urn: "urn:example:stub:token:stub",
    schema_version: "stub-1",
    authority: STUB_AUTHORITY,
    owner_producer: STUB_OWNER,
    point_of_contact: STUB_POC,
    deprecation: None,
    portion_form: "STUB",
    banner_form: "STUB",
    banner_abbreviation: None,
};

// The StubScheme's single sentinel token returns a `FormSet` whose
// three canonical fields are all `"STUB"`. The default projections in
// the `Vocabulary` trait handle `portion_form` / `banner_form` /
// `banner_abbreviation` automatically.
static STUB_FORM_SET: FormSet = FormSet {
    portion: "STUB",
    banner_title: "STUB",
    banner_abbreviation: None,
    recognized_aliases: &[],
};

impl Vocabulary<StubScheme> for StubScheme {
    fn authority(&self, _token: &TokenId) -> &'static Authority {
        &STUB_AUTHORITY
    }
    fn owner_producer(&self, _token: &TokenId) -> &'static OwnerProducer {
        &STUB_OWNER
    }
    fn point_of_contact(&self, _token: &TokenId) -> &'static PointOfContact {
        &STUB_POC
    }
    fn deprecation(&self, _token: &TokenId) -> Option<&'static Deprecation<TokenId>> {
        None
    }
    fn forms(&self, _token: &TokenId) -> &'static FormSet {
        &STUB_FORM_SET
    }
    fn metadata(&self, _token: &TokenId) -> &'static TokenMetadataFull<TokenId> {
        &STUB_METADATA
    }
    fn shape_admits(&self, _category: CategoryId, _bytes: &[u8]) -> bool {
        // Stub scheme has no admissible bytes — returning `false`
        // unconditionally satisfies the totality contract and
        // demonstrates the trait surface compiles for a second
        // scheme without dragging in CAPCO-specific machinery.
        false
    }
}

// ---------------------------------------------------------------------------
// Recognizer impl — zero-candidate Ambiguous (the engine-safe answer).
// ---------------------------------------------------------------------------

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
            candidates: Vec::<Candidate<StubMarking>>::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Codec impl — encode/decode with the safe-empty defaults.
// ---------------------------------------------------------------------------

struct StubCodec;

impl Codec<StubScheme> for StubCodec {
    fn encode(&self, marking: &StubMarking) -> Result<Vec<u8>, CodecError> {
        Ok(if marking.has_token {
            b"STUB".to_vec()
        } else {
            Vec::new()
        })
    }
    fn decode(&self, bytes: &[u8]) -> Result<Parsed<StubMarking>, CodecError> {
        if bytes == b"STUB" {
            Ok(Parsed::Unambiguous(StubMarking { has_token: true }))
        } else if bytes.is_empty() {
            Ok(Parsed::Unambiguous(StubMarking::bottom()))
        } else {
            // Schema-mismatch is one of the three published variants;
            // exercising it here keeps the readiness compile test
            // honest about the full surface.
            //
            // **Audit content-ignorance (per `CodecError` type-level
            // docs):** `observed` MUST NOT contain any substring of the
            // input. A real
            // codec reads the schema-version identifier from a
            // known-safe location in the decoded structure (a
            // `version=` attribute, a `<schema>` element, etc.) and
            // populates `observed` from THAT — not from raw input
            // bytes via `String::from_utf8_lossy(bytes)`. The stub
            // has no such structure to read from, so the placeholder
            // `"<unknown>"` stands in for "couldn't determine which
            // schema the input claimed". A scheme adapter that copies
            // this stub MUST replace this branch with a real
            // schema-version extractor before shipping.
            Err(CodecError::SchemaMismatch {
                expected: "stub-1",
                observed: String::from("<unknown>"),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Compile-time + runtime assertions.
// ---------------------------------------------------------------------------

/// Confirms `StubScheme` can be named through every Phase-E trait
/// without an engine-side crate. The function body never runs at
/// runtime (the `const _: fn() = ...` binding runs type-checking
/// only); a future trait change that requires an engine-internal
/// type fails to compile here.
const _: fn() = || {
    fn _accepts_marking_scheme<S: MarkingScheme>(_: &S) {}
    fn _accepts_vocabulary<S: MarkingScheme + Vocabulary<S>>(_: &S) {}
    fn _accepts_recognizer<S: MarkingScheme>(_: &dyn Recognizer<S>) {}
    fn _accepts_codec<S: MarkingScheme>(_: &dyn Codec<S>) {}
};

#[test]
fn second_scheme_builds_without_engine_edits() {
    let scheme = StubScheme::new();
    let recognizer = StubRecognizer;
    let codec = StubCodec;

    // MarkingScheme surface — every method invocable.
    assert_eq!(scheme.name(), "stub");
    assert_eq!(scheme.schema_version(), "stub-1");
    assert!(scheme.categories().is_empty());
    assert_eq!(scheme.constraints().len(), 1);
    assert!(scheme.templates().is_empty());
    assert_eq!(scheme.page_rewrites().len(), 1);
    assert!(matches!(
        scheme.parse("anything"),
        Ok(Parsed::Ambiguous { .. })
    ));

    let marking = StubMarking::top();
    assert!(scheme.satisfies(&marking, &TokenRef::Token(STUB_TOKEN)));
    assert!(!scheme.satisfies(&marking, &TokenRef::Token(OTHER_TOKEN)));
    assert_eq!(scheme.render_item(&marking), "(STUB)");
    assert_eq!(scheme.render_summary(&marking), "STUB");
    assert_eq!(scheme.render_summary(&StubMarking::bottom()), "");

    // `validate` defaults to `evaluate(scheme, marking)`. Our single
    // declared constraint is `Conflicts(STUB_TOKEN, OTHER_TOKEN)`,
    // and the marking only carries `STUB_TOKEN` — the `Conflicts`
    // arm requires both, so it does not fire.
    assert!(
        scheme.validate(&marking).is_empty(),
        "stub conflicts can't fire — only one token present",
    );

    // `project` is identity-via-bottom under the lattice contract.
    let projected = scheme.project(Scope::Page, &[marking.clone(), StubMarking::bottom()]);
    assert!(projected.has_token);

    // Vocabulary surface.
    assert_eq!(scheme.authority(&STUB_TOKEN).source_name, "Stub Authority");
    assert_eq!(scheme.owner_producer(&STUB_TOKEN).code, "STUB");
    assert_eq!(
        scheme.point_of_contact(&STUB_TOKEN).email,
        "stub@example.invalid"
    );
    assert!(scheme.deprecation(&STUB_TOKEN).is_none());
    assert_eq!(scheme.portion_form(&STUB_TOKEN), "STUB");
    assert_eq!(scheme.banner_form(&STUB_TOKEN), "STUB");
    assert!(scheme.banner_abbreviation(&STUB_TOKEN).is_none());
    assert_eq!(scheme.metadata(&STUB_TOKEN).canonical, "STUB");

    // Recognizer surface — through a `dyn` boxed object so the
    // dynamic-dispatch path is exercised too.
    let r: Box<dyn Recognizer<StubScheme>> = Box::new(recognizer);
    let parsed = r.recognize(b"anything", 0, &scheme, &ParseContext::default());
    assert!(matches!(parsed, Parsed::Ambiguous { ref candidates } if candidates.is_empty()));

    // Codec surface — encode + decode + every CodecError variant.
    let encoded = codec.encode(&StubMarking::top()).expect("encode top");
    assert_eq!(encoded, b"STUB");
    let decoded = codec.decode(&encoded).expect("decode round-trips");
    assert!(matches!(decoded, Parsed::Unambiguous(m) if m.has_token));

    // Mismatch path.
    let mismatch = codec
        .decode(b"NOT-STUB")
        .expect_err("non-stub bytes should mismatch");
    assert!(matches!(
        mismatch,
        CodecError::SchemaMismatch {
            expected: "stub-1",
            ..
        }
    ));
}
