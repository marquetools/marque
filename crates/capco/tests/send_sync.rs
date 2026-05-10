// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Runtime smoke test verifying the concrete `CapcoScheme` recognizer
//! dispatch is `Send + Sync`-usable as a trait object (FR-038, T003 of
//! engine-refactor-006). The compile-time property is already pinned by
//! the `Recognizer<S>: Send + Sync` supertrait bound at
//! `crates/scheme/src/recognizer.rs:211` plus each
//! `impl Recognizer<CapcoScheme> for ...` block at
//! `crates/engine/src/recognizer.rs` — those impls cannot exist
//! without their concrete types being `Send + Sync`. This file
//! exercises the boxed-trait-object dispatch path at runtime so a
//! future regression that broke the dispatch (rather than the trait
//! bound itself) would be caught here, and so the test contributes
//! coverage to the dispatch surface alongside the compile-time
//! enforcement.
//!
//! `marque-engine` is a dev-dependency here (path-only, see
//! `crates/capco/Cargo.toml`); the runtime dep flows the other way
//! (`marque-engine` consumes `marque-capco`). Keeping this assertion
//! in a `tests/` file rather than inside the lib crate keeps the
//! WASM-safe set honest under Constitution III + VII — `marque-capco`
//! MUST NOT gain a non-dev dep on the engine.

use marque_capco::CapcoScheme;
use marque_engine::StrictRecognizer;
use marque_rules::{AppliedFix, Diagnostic, FixIntent};
use marque_scheme::recognizer::{ParseContext, Recognizer};
use std::sync::Arc;

fn assert_send_sync<T: ?Sized + Send + Sync>(_: &T) {}

#[test]
fn capco_recognizer_dispatch_is_send_sync_as_trait_object() {
    let boxed: Box<dyn Recognizer<CapcoScheme>> = Box::new(StrictRecognizer::new());
    assert_send_sync(&*boxed);

    let arced: Arc<dyn Recognizer<CapcoScheme>> = Arc::new(StrictRecognizer::new());
    assert_send_sync(&*arced);

    // Exercise the dispatch path. Empty input is a deterministic
    // zero-candidate case across all CapcoScheme recognizers, so the
    // assertion is stable without depending on parser internals.
    let cx = ParseContext::default();
    let _ = boxed.recognize(b"", &cx);
    let _ = arced.recognize(b"", &cx);
}

/// Compile-time `Send + Sync` proof for the rule-emission types
/// monomorphized at `<CapcoScheme>`. PR 3c.B Commit 2 introduced
/// `CapcoOpenVocabRef` as a new field on `FactRef::OpenVocab`, which
/// transitively flows through `FixIntent<CapcoScheme>`,
/// `Diagnostic<CapcoScheme>`, and `AppliedFix<CapcoScheme>`. The
/// structural correctness is already enforced by the
/// `OpenVocabRef: Send + Sync + 'static` bound on `MarkingScheme`,
/// but a future field addition to `CapcoOpenVocabRef` (e.g., wrapping
/// a non-`Sync` type) would break Constitution VI's `BatchEngine`
/// scheduling guarantee silently — this test catches that at compile
/// time. The function bodies are empty; the assertion is the type
/// constraint, fired at monomorphization.
#[test]
fn capco_rule_emission_types_are_send_sync() {
    fn assert<T: Send + Sync>() {}
    assert::<FixIntent<CapcoScheme>>();
    assert::<Diagnostic<CapcoScheme>>();
    assert::<AppliedFix<CapcoScheme>>();
}
