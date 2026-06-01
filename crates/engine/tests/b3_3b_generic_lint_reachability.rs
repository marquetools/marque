// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Generic-lint reachability guard for the B3.3b engine generification.
//!
//! The lint pipeline is now `impl<S: MarkingScheme + ConstraintBridge>
//! Engine<S, R>` rather than `Engine<CapcoScheme, R>`. The 300-plus
//! CapcoScheme lint tests prove the default instantiation is
//! behavior-identical; this file proves the *other* half — that the pipeline
//! is genuinely generic over the scheme and has not silently retained a
//! `CapcoScheme` assumption.
//!
//! The proof is a compile-time monomorphization, not a run: the engine
//! constructors are still pinned to `Engine<CapcoScheme, EngineRecognizer>`, so
//! an `Engine<StubScheme, _>` is not yet constructible. What we *can* do — and
//! what catches a regression — is force the compiler to type-check every
//! generic lint entry point against a second, non-CAPCO scheme (`StubScheme`,
//! whose `Canonical = ()`). If a future edit pins any lint method back to
//! `CapcoScheme` or `CanonicalAttrs`, the monomorphization below stops
//! compiling.
//!
//! A *live* run of the lint pipeline through `StubScheme` is deferred to the
//! phase that generifies the engine constructors (once an `Engine<StubScheme>`
//! can be built); this compile-time guard is the strongest check available
//! while construction stays scheme-pinned.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, LintOptions, LintResult};
use marque_rules::ConstraintBridge;
use marque_scheme::MarkingScheme;
use marque_scheme::recognizer::Recognizer;
use marque_test_utils::stub_scheme::{StubRecognizer, StubScheme};

/// Type-checks the public lint surface for an arbitrary scheme meeting the
/// B3.3b bounds. The body never runs against `StubScheme` (no constructor
/// exists yet); its purpose is the type-check of `lint` / `lint_with_options`
/// resolving generically.
#[allow(dead_code)]
fn lint_surface_is_generic_over_scheme<S, R>(engine: &Engine<S, R>, source: &[u8]) -> LintResult<S>
where
    S: MarkingScheme + ConstraintBridge,
    S::Canonical: Clone + Default + PartialEq,
    R: Recognizer<S>,
{
    let _ = engine.lint_with_options(source, &LintOptions::default());
    engine.lint(source)
}

// Force monomorphization for the stub scheme: coercing the generic fn to a
// concrete fn pointer over `Engine<StubScheme, StubRecognizer>` makes the
// compiler instantiate the lint pipeline for a scheme that is emphatically not
// CAPCO (`StubScheme::Canonical = ()`). A `CapcoScheme`/`CanonicalAttrs` leak
// in any lint call site would fail this coercion at compile time.
const _STUB_LINT_REACHABLE: fn(
    &Engine<StubScheme, StubRecognizer>,
    &[u8],
) -> LintResult<StubScheme> = lint_surface_is_generic_over_scheme::<StubScheme, StubRecognizer>;

/// A live run of the same generic surface at the default scheme. The 300-plus
/// in-crate lint tests already exercise CAPCO behavior exhaustively; this is a
/// single smoke check that the generic entry points still produce a clean
/// result on empty input through the public API.
#[test]
fn capco_lint_through_generic_surface_is_clean_on_empty_input() {
    let engine: Engine = Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");
    let result: LintResult = engine.lint(b"");
    assert!(result.is_clean());
    assert_eq!(result.candidates_total, 0);
}
