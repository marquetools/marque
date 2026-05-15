// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Default-impl pinning for [`Vocabulary::is_fdr_dissem`].
//!
//! A scheme that does NOT override `is_fdr_dissem` must see the
//! trait's default impl return `false` for every token. This is the
//! correct behavior for schemes with no FD&R concept (e.g., a
//! hypothetical CUI-only scheme where every disclosure decision is
//! unconditional) — see the doc comment on
//! `Vocabulary::is_fdr_dissem` for the full contract.
//!
//! This test is intentionally separated from
//! `crates/scheme/tests/adoption_readiness.rs` (the Phase-F readiness
//! compile test) so a failure here points unambiguously at the
//! default-impl contract rather than at the broader trait surface.
//! It is also separated from
//! `crates/scheme/tests/proptest_closure.rs` because that file's
//! `ClosureStubScheme` does not implement `Vocabulary` — wiring one
//! on would expand its surface beyond closure-operator concerns.
//!
//! The override-side bidirectional pin against `FDR_DOMINATORS`
//! lives in `crates/capco/src/vocabulary.rs::fdr_dissem_pin` (an
//! in-crate unit-test module). The override's public-API behavior
//! is exercised in `crates/capco/tests/fdr_dissem_predicate.rs`.
//! Together the three sites pin every direction of the contract.

use marque_scheme::ambiguity::Parsed;
use marque_scheme::category::{Category, CategoryId, TokenId};
use marque_scheme::constraint::{Constraint, TokenRef};
use marque_scheme::lattice::Lattice;
use marque_scheme::page_rewrite::PageRewrite;
use marque_scheme::scheme::MarkingScheme;
use marque_scheme::scope::Scope;
use marque_scheme::template::Template;
use marque_scheme::vocabulary::{
    Authority, Deprecation, FormSet, OwnerProducer, OwnerProducerKind, PointOfContact,
    TokenMetadataFull, Vocabulary,
};

// ---------------------------------------------------------------------------
// Minimal scheme with no FD&R concept.
// ---------------------------------------------------------------------------
//
// `NoFdrScheme` mirrors the shape of `crates/scheme/tests/adoption_readiness.rs`'s
// `StubScheme` but trimmed to what `Vocabulary::is_fdr_dissem` needs:
// a `MarkingScheme` impl whose `Token` type is `TokenId`, and a
// `Vocabulary` impl that does NOT override `is_fdr_dissem`. The
// default impl from the trait should then return `false` for every
// token.

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct NoFdrMarking;

impl Lattice for NoFdrMarking {
    fn join(&self, _other: &Self) -> Self {
        Self
    }
    fn meet(&self, _other: &Self) -> Self {
        Self
    }
}

const SOME_TOKEN: TokenId = TokenId(1);
const ANOTHER_TOKEN: TokenId = TokenId(2);

#[derive(Debug)]
struct NoFdrParseError;

impl core::fmt::Display for NoFdrParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "no-fdr stub parse error")
    }
}

impl std::error::Error for NoFdrParseError {}

struct NoFdrScheme;

impl MarkingScheme for NoFdrScheme {
    type Token = TokenId;
    type Marking = NoFdrMarking;
    type ParseError = NoFdrParseError;
    type OpenVocabRef = core::convert::Infallible;

    fn name(&self) -> &str {
        "no-fdr"
    }
    fn schema_version(&self) -> &str {
        "no-fdr-1"
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
    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &[]
    }
    fn parse(&self, _input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        Ok(Parsed::Ambiguous {
            candidates: Vec::new(),
        })
    }
    fn satisfies(&self, _marking: &Self::Marking, _token_ref: &TokenRef) -> bool {
        false
    }
    fn project(&self, _scope: Scope, _markings: &[Self::Marking]) -> Self::Marking {
        NoFdrMarking
    }
    fn render_canonical(
        &self,
        _m: &Self::Marking,
        _scope: Scope,
        _out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Vocabulary impl — every required accessor returns `&'static` data.
// `is_fdr_dissem` is INTENTIONALLY NOT overridden — the whole point
// of this test file is to pin the trait's default return value.
// ---------------------------------------------------------------------------

static NO_FDR_AUTHORITY: Authority = Authority {
    source_name: "No-FDR Stub Authority",
    urn: "urn:example:no-fdr:scheme",
    schema_version: "no-fdr-1",
    point_of_contact: PointOfContact {
        name: "No-FDR POC",
        email: "no-fdr@example.invalid",
        organization: "No-FDR Org",
    },
};

static NO_FDR_OWNER: OwnerProducer = OwnerProducer {
    code: "NOFDR",
    name: "No-FDR Org",
    kind: OwnerProducerKind::Organization,
};

static NO_FDR_POC: PointOfContact = PointOfContact {
    name: "No-FDR POC",
    email: "no-fdr@example.invalid",
    organization: "No-FDR Org",
};

static NO_FDR_METADATA: TokenMetadataFull<TokenId> = TokenMetadataFull {
    canonical: "NOFDR",
    urn: "urn:example:no-fdr:token",
    schema_version: "no-fdr-1",
    authority: NO_FDR_AUTHORITY,
    owner_producer: NO_FDR_OWNER,
    point_of_contact: NO_FDR_POC,
    deprecation: None,
    portion_form: "NOFDR",
    banner_form: "NOFDR",
    banner_abbreviation: None,
};

static NO_FDR_FORM_SET: FormSet = FormSet {
    portion: "NOFDR",
    banner_title: "NOFDR",
    banner_abbreviation: None,
    recognized_aliases: &[],
};

impl Vocabulary<NoFdrScheme> for NoFdrScheme {
    fn authority(&self, _token: &TokenId) -> &'static Authority {
        &NO_FDR_AUTHORITY
    }
    fn owner_producer(&self, _token: &TokenId) -> &'static OwnerProducer {
        &NO_FDR_OWNER
    }
    fn point_of_contact(&self, _token: &TokenId) -> &'static PointOfContact {
        &NO_FDR_POC
    }
    fn deprecation(&self, _token: &TokenId) -> Option<&'static Deprecation<TokenId>> {
        None
    }
    fn forms(&self, _token: &TokenId) -> &'static FormSet {
        &NO_FDR_FORM_SET
    }
    fn metadata(&self, _token: &TokenId) -> &'static TokenMetadataFull<TokenId> {
        &NO_FDR_METADATA
    }
    fn shape_admits(&self, _category: CategoryId, _bytes: &[u8]) -> bool {
        false
    }
    // `is_fdr_dissem` deliberately NOT overridden. The trait's
    // default `fn is_fdr_dissem(&self, _: &S::Token) -> bool { false }`
    // is the contract under test.
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

/// The default `Vocabulary::is_fdr_dissem` returns `false` for every
/// token id. A scheme that opts into the default impl by not
/// overriding accepts this no-FD&R semantic.
#[test]
fn default_is_fdr_dissem_returns_false_for_every_token() {
    let scheme = NoFdrScheme;
    assert!(
        !scheme.is_fdr_dissem(&SOME_TOKEN),
        "default `is_fdr_dissem` must return false; a scheme that \
         declares no FD&R override is treated as having no FD&R \
         concept",
    );
    assert!(
        !scheme.is_fdr_dissem(&ANOTHER_TOKEN),
        "default `is_fdr_dissem` must return false uniformly across \
         token ids — the default impl is constant-false by design",
    );
    // Exercise additional token-id values to pin the constant-false
    // property across a non-trivial range. The default impl's
    // contract is "false for every token", so 0, 1, u32::MAX and a
    // few hand-picked points are sufficient evidence.
    for raw in [0_u32, 100, 1_000, 65_535, u32::MAX] {
        assert!(
            !scheme.is_fdr_dissem(&TokenId(raw)),
            "default `is_fdr_dissem` returned true for TokenId({raw})",
        );
    }
}
