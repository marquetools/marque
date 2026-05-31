// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Opt-in pinning for [`IcMarkingVocabulary::is_fdr_dissem`].
//!
//! FD&R membership lives on the [`IcMarkingVocabulary`] sub-trait, not
//! on the base [`Vocabulary`] trait. A scheme with no FD&R concept
//! simply does not implement [`IcMarkingVocabulary`] — there is no
//! default to fall through to and no "forgot to override → silent
//! false" footgun. This test pins that opt-in shape: `NoFdrScheme`
//! implements [`Vocabulary`] WITHOUT implementing
//! [`IcMarkingVocabulary`], and compiles + is usable as a vocabulary.
//! The compile-time absence of `is_fdr_dissem` on `NoFdrScheme` is the
//! property under test.
//!
//! The override-side bidirectional pin against `FDR_DOMINATORS` lives in
//! `crates/capco/src/vocabulary/fdr_dissem_pin.rs`; the override's
//! public-API behavior is exercised in
//! `crates/capco/tests/fdr_dissem_predicate.rs`.

use marque_scheme::ambiguity::Parsed;
use marque_scheme::category::{Category, CategoryId, TokenId};
use marque_scheme::constraint::{Constraint, TokenRef};
use marque_scheme::lattice::{JoinSemilattice, MeetSemilattice};
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
// `NoFdrScheme` mirrors the shape of `adoption_readiness.rs`'s
// `StubScheme` but trimmed: a `MarkingScheme` impl whose `Token` is
// `TokenId`, and a `Vocabulary` impl that does NOT implement the
// `IcMarkingVocabulary` sub-trait. The scheme therefore carries no
// `is_fdr_dissem` surface at all.

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct NoFdrMarking;

impl JoinSemilattice for NoFdrMarking {
    fn join(&self, _other: &Self) -> Self {
        Self
    }
}

impl MeetSemilattice for NoFdrMarking {
    fn meet(&self, _other: &Self) -> Self {
        Self
    }
}

const SOME_TOKEN: TokenId = TokenId(1);

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
    type Parsed<'src> = ();
    type Canonical = ();

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
        _ctx: &marque_scheme::RenderContext,
        _out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Vocabulary impl — every required accessor returns `&'static` data.
// `NoFdrScheme` deliberately does NOT implement `IcMarkingVocabulary`,
// so it carries no `is_fdr_dissem` surface.
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
    short_form: "NOFDR",
    long_form: "NOFDR",
    abbreviated_form: None,
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
    // `IcMarkingVocabulary` is deliberately NOT implemented for
    // `NoFdrScheme` — a non-IC scheme has no FD&R surface at all.
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

/// A scheme can implement the base `Vocabulary` trait without
/// implementing the IC-specific `IcMarkingVocabulary` sub-trait. This
/// pins the opt-in shape: FD&R membership is not forced onto every
/// scheme. The body exercises the base-trait surface to prove
/// `NoFdrScheme` is a usable `Vocabulary`; `is_fdr_dissem` is simply not
/// callable on it (it is not in scope without the sub-trait), which is
/// the property under test — enforced at compile time by the absence of
/// an `IcMarkingVocabulary` impl.
#[test]
fn non_ic_scheme_is_vocabulary_without_fdr_surface() {
    let scheme = NoFdrScheme;
    // Base-trait accessors work; the scheme is a valid Vocabulary.
    let _ = scheme.metadata(&SOME_TOKEN);
    let _ = scheme.forms(&SOME_TOKEN);
    assert!(
        !scheme.shape_admits(CategoryId::MARKING, b"X"),
        "NoFdrScheme::shape_admits is the trimmed stub returning false",
    );
    // `scheme.is_fdr_dissem(&SOME_TOKEN)` would NOT compile here:
    // `NoFdrScheme` does not implement `IcMarkingVocabulary`, so the
    // method is not in scope. That compile-time absence IS the opt-in
    // guarantee this test pins.
}
