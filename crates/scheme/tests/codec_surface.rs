// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-test for the `Codec<S>` trait surface (Phase 5 PR-3,
//! task T078).
//!
//! Phase E publishes the codec trait without any concrete impls.
//! Phase G lands XML and JSON impls without further trait evolution
//! (FR-019, SC-010). This file's only job is to assert that the
//! surface exists, compiles cleanly with no concrete impls, and that
//! the auxiliary types (`CodecError`, `Parsed<S::Marking>`) round-trip
//! through it without any engine-side dependency.
//!
//! ## Why this is a test, not a doc-test
//!
//! The same compile property COULD live as a `///` doc-test on
//! `crates/scheme/src/codec.rs`, but a separate integration file
//! makes the SC-010 readiness invariant explicit and gives Phase G
//! a single file to extend when concrete impls land.
//!
//! ## Scope
//!
//! - Define a minimal local `MockScheme` so the compile test does not
//!   depend on `marque-capco` or any other downstream crate.
//! - Construct a function `accepts_codec(_: &impl Codec<MockScheme>)`
//!   to prove the trait can be named generically.
//! - Run a smoke check that exercises every `CodecError` variant's
//!   `Display` impl — guards against an accidental pattern that
//!   silently drops a variant.

use marque_scheme::ambiguity::Parsed;
use marque_scheme::category::TokenId;
use marque_scheme::codec::{Codec, CodecError};
use marque_scheme::constraint::{Constraint, ConstraintViolation, TokenRef};
use marque_scheme::page_rewrite::PageRewrite;
use marque_scheme::scheme::MarkingScheme;
use marque_scheme::scope::Scope;
use marque_scheme::template::Template;

// ---------------------------------------------------------------------------
// Minimal `MarkingScheme` to bind `Codec<S>` against.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct MockScheme;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MockMarking;

#[derive(Debug)]
struct MockParseError;

impl std::fmt::Display for MockParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MockParseError")
    }
}
impl std::error::Error for MockParseError {}

impl marque_scheme::lattice::JoinSemilattice for MockMarking {
    fn join(&self, _other: &Self) -> Self {
        Self
    }
}

impl marque_scheme::lattice::MeetSemilattice for MockMarking {
    fn meet(&self, _other: &Self) -> Self {
        Self
    }
}

impl MarkingScheme for MockScheme {
    type Token = TokenId;
    type Marking = MockMarking;
    type ParseError = MockParseError;
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();

    fn name(&self) -> &str {
        "mock"
    }
    fn schema_version(&self) -> &str {
        "mock-1"
    }
    fn categories(&self) -> &[marque_scheme::category::Category] {
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
        Err(MockParseError)
    }
    fn satisfies(&self, _marking: &Self::Marking, _token_ref: &TokenRef) -> bool {
        false
    }
    fn evaluate_custom(
        &self,
        _name: &'static str,
        _marking: &Self::Marking,
        _bits: marque_scheme::FactBitmask,
    ) -> Vec<ConstraintViolation> {
        Vec::new()
    }
    fn project(&self, _scope: Scope, _markings: &[Self::Marking]) -> Self::Marking {
        MockMarking
    }
    fn render_portion(&self, _m: &Self::Marking) -> String {
        String::new()
    }
    fn render_banner(&self, _m: &Self::Marking) -> String {
        String::new()
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
// Compile-only assertions.
// ---------------------------------------------------------------------------

/// Accepts any `Codec<MockScheme>`. The function body is never called
/// — its existence is the assertion that `Codec<S>` can be named in a
/// generic bound. If a refactor accidentally makes `Codec<S>` a
/// non-object-safe trait or drops the trait entirely, this declaration
/// fails to compile.
#[allow(dead_code)]
fn accepts_codec_by_trait_object(_codec: &dyn Codec<MockScheme>) {}

#[allow(dead_code)]
fn accepts_codec_generically<C: Codec<MockScheme>>(_codec: &C) {}

#[test]
fn codec_compiles_without_impls() {
    // Scope of the "no impls" claim: no concrete `impl Codec<S>` exists
    // in the `marque_scheme` LIBRARY (`src/`) — only in test crates.
    // T089b's `tests/adoption_readiness.rs` declares a `StubCodec`
    // impl as part of the trait-surface readiness exercise, but that
    // is intentional fixture-side scaffolding outside the library
    // boundary. Phase G lands the production XML/JSON impls without
    // further trait evolution (FR-019, SC-010).
    //
    // The two `accepts_codec_*` helpers above are the load-bearing
    // compile-time assertion: if they build, `Codec<S>` is a usable
    // trait surface even though no library-side implementor exists.
    //
    // The runtime assertion below is a sanity check on `CodecError`'s
    // `Display` impl — every variant must round-trip through
    // `to_string()` without panicking.
    let errors: &[CodecError] = &[
        CodecError::Malformed("bad input".to_string()),
        CodecError::UnsupportedFormat("yaml"),
        CodecError::SchemaMismatch {
            expected: "ISM-v2022-DEC",
            observed: "ISM-v2018-NOV".to_string(),
        },
    ];
    for err in errors {
        let s = err.to_string();
        assert!(
            !s.is_empty(),
            "CodecError::Display produced an empty string for {err:?}",
        );
    }
}

#[test]
fn codec_decode_returns_parsed_shape() {
    // `Codec::decode` returns `Result<Parsed<S::Marking>, CodecError>`.
    // The `Parsed::Ambiguous { candidates: vec![] }` zero-candidate
    // form is the "decoded cleanly but no plausible marking" signal
    // (per the `Codec::decode` doc-comment); pin the type at compile
    // time to keep the contract from drifting.
    let p: Parsed<MockMarking> = Parsed::Ambiguous {
        candidates: Vec::new(),
    };
    match p {
        Parsed::Unambiguous(_) => panic!("constructed Ambiguous"),
        Parsed::Ambiguous { candidates } => {
            assert!(candidates.is_empty());
        }
    }
}
