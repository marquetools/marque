// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-time proof that `Recognizer<S>` carries `Send + Sync` correctly.
//!
//! `BatchEngine` holds recognizers behind `Arc<dyn Recognizer<S>>` and
//! dispatches them across Tokio tasks; if the trait's supertrait
//! bounds ever stopped enforcing `Send + Sync` we would only find out
//! at runtime when cross-task dispatch stopped compiling at the
//! call site. The `const _: fn() = ...` pattern turns that into a
//! compile-time failure here instead.
//!
//! The assertion uses a minimal in-test scheme rather than
//! `marque_capco::CapcoScheme`: the `Box<dyn Recognizer<S>>: Send + Sync`
//! property depends on the trait's supertraits, not on any particular
//! `S`, and `marque-scheme` cannot depend on `marque-capco` without
//! introducing a cycle (Constitution VII). See the companion assertion
//! in `crates/capco/tests/send_sync.rs` (landing alongside Phase 4
//! task T058) for the concrete `CapcoScheme` form.

use marque_scheme::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, Candidate, Category, Constraint,
    ConstraintViolation, EvidenceFeature, JoinSemilattice, MarkingScheme, MeetSemilattice,
    ParseContext, Parsed, Recognizer, Scope, Template,
};

#[derive(Clone, Debug, PartialEq, Eq)]
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

impl BoundedJoinSemilattice for StubMarking {
    fn bottom() -> Self {
        Self
    }
}

impl BoundedMeetSemilattice for StubMarking {
    fn top() -> Self {
        Self
    }
}

struct StubScheme;

impl MarkingScheme for StubScheme {
    type Token = u32;
    type Marking = StubMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;

    fn name(&self) -> &str {
        "stub"
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
    fn validate(&self, _: &Self::Marking) -> Vec<ConstraintViolation> {
        Vec::new()
    }
    fn project(&self, _: Scope, _: &[Self::Marking]) -> Self::Marking {
        StubMarking
    }
    fn render_portion(&self, _: &Self::Marking) -> String {
        String::new()
    }
    fn render_banner(&self, _: &Self::Marking) -> String {
        String::new()
    }
    fn render_canonical(
        &self,
        _: &Self::Marking,
        _: Scope,
        _: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        Ok(())
    }
}

struct NoopRecognizer;

impl Recognizer<StubScheme> for NoopRecognizer {
    fn recognize(&self, _bytes: &[u8], _offset: usize, _cx: &ParseContext) -> Parsed<StubMarking> {
        // Zero-candidate Ambiguous is the engine-safe "nothing
        // recognized" signal (foundational-plan line 609-612).
        Parsed::Ambiguous {
            candidates: Vec::<Candidate<StubMarking>>::new(),
        }
    }
}

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}
fn assert_send_sync<T: Send + Sync>() {}

/// Compile-time assertion that `Box<dyn Recognizer<S>>` is `Send + Sync`.
///
/// The function body never executes at runtime (the `const _` binding
/// runs type-checking only), but Rust's trait-resolution pass verifies
/// every call here. If the `Recognizer` trait's supertraits ever drop
/// `Send` or `Sync`, this fails to type-check.
const _: fn() = || {
    assert_send::<Box<dyn Recognizer<StubScheme>>>();
    assert_sync::<Box<dyn Recognizer<StubScheme>>>();
    assert_send_sync::<Box<dyn Recognizer<StubScheme>>>();
    assert_send_sync::<std::sync::Arc<dyn Recognizer<StubScheme>>>();
};

/// Runtime smoke test — also exercises the `Recognizer::recognize`
/// call path so coverage tools pick the trait up.
#[test]
fn recognizer_trait_object_is_usable_as_dyn() {
    let r: Box<dyn Recognizer<StubScheme>> = Box::new(NoopRecognizer);
    let cx = ParseContext::default();
    match r.recognize(b"anything", 0, &cx) {
        Parsed::Ambiguous { candidates } => {
            assert!(candidates.is_empty(), "stub returns zero candidates");
            // Touch the feature type so the assertion keeps importing
            // the evidence surface it's meant to exercise.
            let _: Option<EvidenceFeature> = None;
        }
        Parsed::Unambiguous(_) => panic!("stub should not return Unambiguous"),
    }
}
