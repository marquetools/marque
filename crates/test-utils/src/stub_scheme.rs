// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Shared minimal second [`MarkingScheme`] fixture.
//!
//! `StubScheme` is the smallest lawful implementation of the
//! scheme-adapter contract — [`MarkingScheme`] plus the companion
//! [`Vocabulary<S>`], [`Recognizer<S>`], and [`Codec<S>`] surfaces. It
//! exists so generic engine/rule code can be exercised against a *second*
//! scheme (not just `CapcoScheme`) without dragging in any CAPCO/ISM
//! vocabulary. Any generic-surface code (`Rule<S>`, a generic `Engine<S>`)
//! leans on having a real second `S` to instantiate; a generic body that
//! only ever sees `CapcoScheme` can silently bake in monomorphization
//! assumptions this fixture surfaces immediately.
//!
//! ## Self-containment (Constitution VII)
//!
//! This module imports only from `marque_scheme`, `marque_rules`, and
//! `std`. Both marque dependencies are domain-neutral WASM-safe crates
//! that sit *below* `marque-test-utils` (`marque-rules` depends on
//! `marque-scheme`, not the reverse), so neither edge inverts the
//! dependency graph. `marque_rules` is needed only for the empty
//! [`ConstraintBridge`](marque_rules::ConstraintBridge) impl below — a
//! generic `Engine<S>` instantiated with this fixture as its second
//! scheme must satisfy that bound. Pulling in `marque-engine` /
//! `marque-core` / `marque-ism`
//! / `marque-capco` here remains forbidden: the engine/scanner crates
//! would invert the graph, and `marque-ism` / `marque-capco` would drag in
//! the CAPCO/ISM vocabulary this fixture exists to avoid.
//! `marque-test-utils` is `publish=false` and consumed only under
//! `[dev-dependencies]`, so these edges never reach a shipping crate's
//! normal dep graph or the WASM artifact.
//!
//! ## Not a real grammar
//!
//! `parse` / `decode` / `recognize` return the engine-safe "nothing
//! recognized" answer (zero-candidate `Ambiguous`); `validate` is the
//! trait default (empty); `project` is identity-via-`bottom` under the
//! lattice contract. The point is a *closed, instantiable* trait surface,
//! not a working parser. Construct via [`StubScheme::new`].

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
// Sentinel ids so the constraint / rewrite catalogs point at something
// concrete.
// ---------------------------------------------------------------------------

/// The single sentinel token whose presence `StubMarking` tracks.
pub const STUB_TOKEN: TokenId = TokenId(1);
/// The single sentinel category the rewrite catalog references.
pub const STUB_CATEGORY: CategoryId = CategoryId(1);
/// A second token id so `Constraint::Conflicts` has two distinct
/// operands. The constraint can never fire (the marking only tracks one
/// sentinel), but the catalog entry must be well-formed.
pub const OTHER_TOKEN: TokenId = TokenId(2);

// ---------------------------------------------------------------------------
// Minimal marking type. One presence bit, lattice-trivial.
// ---------------------------------------------------------------------------

/// Minimal [`MarkingScheme::Marking`] — a single presence bit. `false`
/// is the lattice bottom.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StubMarking {
    /// Presence of the single sentinel token. `false` is bottom.
    pub has_token: bool,
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
// Parse error.
// ---------------------------------------------------------------------------

/// Stub [`MarkingScheme::ParseError`]. Never actually returned — the
/// stub `parse` yields a zero-candidate `Ambiguous` — but the trait
/// requires a named error type.
#[derive(Debug)]
pub struct StubParseError;

impl std::fmt::Display for StubParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StubParseError")
    }
}
impl std::error::Error for StubParseError {}

// ---------------------------------------------------------------------------
// Scheme.
// ---------------------------------------------------------------------------

/// Minimal second scheme. See the module docs.
///
/// Not `#[derive(Debug)]`: `PageRewrite<S>` does not implement `Debug`,
/// so the derive cannot apply to the `page_rewrites` field. The source
/// fixture omits it for the same reason.
pub struct StubScheme {
    constraints: [Constraint; 1],
    page_rewrites: [PageRewrite<StubScheme>; 1],
}

/// Sentinel citation for the fixture. Routes through
/// [`AuthoritativeSource::EngineInternal`] so `Display` renders
/// `[engine-internal]` and the value carries no false CAPCO §-claim.
const STUB_CITATION: Citation = Citation::new(
    AuthoritativeSource::EngineInternal,
    SectionRef::new(SectionLetter::A),
    match core::num::NonZeroU16::new(1) {
        Some(n) => n,
        None => unreachable!(),
    },
);

/// `&'static [CategoryId]` axes for the stub rewrite. Both `reads` and
/// `writes` cover the same single category — fine for a fixture (the
/// engine scheduler rejects cycles between distinct rewrites, not
/// self-loops).
static STUB_REWRITE_AXES: &[CategoryId] = &[STUB_CATEGORY];

impl StubScheme {
    /// Construct the fixture with its one-entry constraint and rewrite
    /// catalogs populated.
    pub fn new() -> Self {
        Self {
            constraints: [Constraint::Conflicts {
                left: TokenRef::Token(STUB_TOKEN),
                right: TokenRef::Token(OTHER_TOKEN),
                name: "stub/conflicts",
                label: STUB_CITATION,
                severity: None,
                span_anchor: None,
            }],
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

impl Default for StubScheme {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkingScheme for StubScheme {
    type Token = TokenId;
    type Marking = StubMarking;
    type ParseError = StubParseError;
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();
    type Projected = ();

    fn name(&self) -> &str {
        "stub"
    }
    fn schema_version(&self) -> &str {
        "stub-1"
    }
    // Override the default `"scheme"` so the fixture presents a genuine
    // *second* namespace — generic code that gates on `scheme_id()` (e.g.
    // constraint rule-id namespacing) is then exercised against a real
    // non-CapcoScheme value, not the accidental unoverridden default.
    fn scheme_id(&self) -> &'static str {
        "stub"
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
        // recognized" answer per the Recognizer / parser contract.
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
        // Degenerate delegation — byte-identical with the
        // `render_item` / `render_summary` overrides above. `Scope::Diff`
        // rejects per the trait contract.
        match ctx.scope {
            Scope::Portion => out.write_str(&self.render_item(m)),
            // Bundle ≡ Document for single-doc inputs; multi-doc mosaic is #823.
            Scope::Page | Scope::Document | Scope::Bundle => out.write_str(&self.render_summary(m)),
            Scope::Diff => Err(core::fmt::Error),
        }
    }

    // `evaluate_custom` intentionally not overridden: the trait default
    // (empty) suffices for a scheme whose only `Constraint` is the
    // standard `Conflicts` variant.
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

/// The sentinel token's `FormSet` — all three canonical fields are
/// `"STUB"`; the `Vocabulary` default projections handle
/// `portion_form` / `banner_form` / `banner_abbreviation`.
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
        // Stub scheme admits no bytes — `false` satisfies the totality
        // contract without CAPCO-specific machinery.
        false
    }
}

// ---------------------------------------------------------------------------
// ConstraintBridge impl — empty body, inheriting every no-op default.
// ---------------------------------------------------------------------------

/// `StubScheme` declares no diagnostic constraints, so it takes the full
/// set of [`ConstraintBridge`](marque_rules::ConstraintBridge) defaults
/// (no constraints, no fix intents, no messages, no per-system
/// diagnostics). The empty `impl` is what lets a generic
/// `Engine<StubScheme>` satisfy an `S: ConstraintBridge` bound, so the
/// engine can observe the no-constraint behavior against a second scheme.
impl marque_rules::ConstraintBridge for StubScheme {}

// ---------------------------------------------------------------------------
// Recognizer impl — zero-candidate Ambiguous (the engine-safe answer).
// ---------------------------------------------------------------------------

/// Stub [`Recognizer<StubScheme>`] — always zero-candidate `Ambiguous`.
pub struct StubRecognizer;

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

/// Stub [`Codec<StubScheme>`] — round-trips the single sentinel.
pub struct StubCodec;

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
            // Audit content-ignorance (per `CodecError` type-level
            // docs): `observed` MUST NOT contain any substring of the
            // input. A real codec reads the schema-version identifier
            // from a known-safe location in the decoded structure (a
            // `version=` attribute, a `<schema>` element) and populates
            // `observed` from THAT — never from raw input bytes via
            // `String::from_utf8_lossy(bytes)`. The stub has no such
            // structure, so the placeholder `"<unknown>"` stands in for
            // "couldn't determine which schema the input claimed". A
            // scheme adapter copying this stub MUST replace this branch
            // with a real schema-version extractor before shipping.
            Err(CodecError::SchemaMismatch {
                expected: "stub-1",
                observed: String::from("<unknown>"),
            })
        }
    }
}
