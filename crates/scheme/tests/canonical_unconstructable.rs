// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T036 — cross-crate positive controls for the [`Canonical`] seal.
//!
//! The compile-fail proofs that no inadmissible construction path
//! exists live as `compile_fail` doctests on the
//! [`marque_scheme::Canonical`] and
//! [`marque_scheme::CanonicalConstructor`] types in
//! `crates/scheme/src/canonical.rs`. They run via
//! `cargo test --doc -p marque-scheme`.
//!
//! This integration file pins the complementary positive controls:
//!
//! 1. The documented closed-CVE constructor [`Canonical::from_cve`]
//!    is callable from outside `marque-scheme` and produces a
//!    [`TokenSource::Cve`]-tagged value.
//! 2. The engine-only open-vocab construction path
//!    ([`EngineConstructor::__engine_construct`] +
//!    [`CanonicalConstructor::build_open_vocab`]) works when
//!    invoked from outside `marque-scheme` (the
//!    `tools/promote-callsite-lint/` CI lint flags external call
//!    sites; this test simulates the engine's legitimate use under
//!    the test-fixture carve-out per Constitution V Principle V).
//!
//! Together with the doctests, the seal is closed at the type
//! level — external rule crates can construct
//! `Canonical<S>::from_cve(...)` (with a vocabulary-validated
//! `TokenId`) but have no public path to construct an open-vocab
//! [`Canonical`] without going through the engine.

use marque_scheme::ambiguity::Parsed;
use marque_scheme::category::Category;
use marque_scheme::constraint::Constraint;
use marque_scheme::lattice::{BoundedLattice, Lattice};
use marque_scheme::scope::Scope;
use marque_scheme::template::Template;
use marque_scheme::{
    Canonical, CanonicalConstructor, CategoryId, EngineConstructor, MarkingScheme, TokenId,
    TokenSource,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StubMarking;

impl Lattice for StubMarking {
    fn join(&self, _other: &Self) -> Self {
        StubMarking
    }
    fn meet(&self, _other: &Self) -> Self {
        StubMarking
    }
}

impl BoundedLattice for StubMarking {
    fn bottom() -> Self {
        StubMarking
    }
    fn top() -> Self {
        StubMarking
    }
}

struct StubScheme;

impl MarkingScheme for StubScheme {
    type Token = ();
    type Marking = StubMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;

    fn name(&self) -> &str {
        "StubScheme"
    }
    fn schema_version(&self) -> &str {
        "0.0.1"
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
    fn parse(&self, _input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        Ok(Parsed::Unambiguous(StubMarking))
    }
    fn project(&self, _scope: Scope, _markings: &[Self::Marking]) -> Self::Marking {
        StubMarking
    }
    fn render_portion(&self, _m: &Self::Marking) -> String {
        String::new()
    }
    fn render_banner(&self, _m: &Self::Marking) -> String {
        String::new()
    }
}

#[test]
fn external_crate_can_construct_canonical_via_from_cve() {
    // The documented closed-CVE path works from outside marque-scheme.
    // A future marque-cui rule crate would use this exact form
    // (with its own scheme type) when emitting a closed-vocab fix
    // intent.
    let c: Canonical<StubScheme> =
        Canonical::from_cve(TokenId(42), Scope::Portion, Box::from("EXAMPLE"));
    assert_eq!(c.bytes(), "EXAMPLE");
    assert_eq!(c.scope(), Scope::Portion);
    assert!(matches!(c.source(), TokenSource::Cve(t) if *t == TokenId(42)));
}

#[test]
fn engine_only_open_vocab_path_works_under_test_fixture_carve_out() {
    // The engine's legitimate render path is exercised here in a test
    // file (cfg(test)-equivalent) without an actual Engine. The
    // `tools/promote-callsite-lint/` CI lint flags
    // `__engine_construct` call sites outside the engine's
    // allow-listed surface; this file's call site is the documented
    // exception case used to verify the engine-only door is wired
    // correctly across the crate boundary.
    //
    // PR 3c.2 wires this same call shape into Engine::fix_inner.
    // Test-fixture carve-out per Constitution V Principle V.
    let ctor: EngineConstructor<StubScheme> = EngineConstructor::__engine_construct();
    // Test-fixture carve-out per Constitution V — see comment above.
    // Method-call form (`.build_open_vocab(...)`) is the only path:
    // the trait method takes `&self`, so the assoc-fn shorthand
    // `<EngineConstructor<S> as CanonicalConstructor<S>>::build_open_vocab(...)`
    // does not compile. The compile-fail proof at
    // `crates/scheme/src/canonical.rs::CanonicalConstructor` covers
    // the seal property.
    let canonical: Canonical<StubScheme> =
        ctor.build_open_vocab(CategoryId(7), Box::from("OPEN-VOCAB"), Scope::Page);
    assert_eq!(canonical.bytes(), "OPEN-VOCAB");
    assert_eq!(canonical.scope(), Scope::Page);
    match canonical.source() {
        TokenSource::OpenVocab {
            category,
            render_call_site,
        } => {
            assert_eq!(*category, CategoryId(7));
            assert!(
                render_call_site
                    .file()
                    .ends_with("canonical_unconstructable.rs")
            );
        }
        other => panic!("expected OpenVocab source, got {other:?}"),
    }
}

#[test]
fn canonical_is_send_and_sync_across_scheme_parameter() {
    // The PhantomData<fn() -> S> marker keeps Canonical<S>: Send + Sync
    // regardless of S's auto-trait status. This is load-bearing for
    // BatchEngine concurrency (Constitution VI).
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Canonical<StubScheme>>();
    assert_send_sync::<EngineConstructor<StubScheme>>();
}
