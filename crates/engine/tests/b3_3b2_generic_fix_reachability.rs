// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Generic-fix reachability guard for the B3.3b.2 engine generification.
//!
//! The fix pipeline is now `impl<S: MarkingScheme + ConstraintBridge>
//! Engine<S, R>` (returning `FixResult<S>` / `EngineError<S>`) rather than
//! `Engine<CapcoScheme, EngineRecognizer>`. The CapcoScheme fix tests prove the
//! default instantiation is behavior-identical; this file proves the *other*
//! half — that the fix surface (`fix` / `fix_with_options` /
//! `fix_with_threshold`) is genuinely generic over the scheme and has not
//! silently retained a `CapcoScheme` / `CapcoMarking` / `CanonicalAttrs`
//! assumption.
//!
//! The proof here is a compile-time monomorphization, not a run: it forces the
//! compiler to type-check every generic fix entry point against a second,
//! non-CAPCO scheme (`StubScheme`, whose `Canonical = ()`). If a future edit
//! pins any fix method back to `CapcoScheme`, the monomorphization below stops
//! compiling — a guard a *runtime* test cannot give, since a value leak that
//! type-checks would still pass.
//!
//! B3.4 closed the constructor scheme-discard, so an
//! `Engine<StubScheme, StubRecognizer>` is now constructible and the *live*
//! second-scheme lint/fix run lives in
//! `b3_4_second_scheme_construction.rs`. This file is retained as the
//! type-level half of the pair.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixOptions, FixResult};
use marque_rules::ConstraintBridge;
use marque_scheme::MarkingScheme;
use marque_scheme::recognizer::Recognizer;
use marque_test_utils::stub_scheme::{StubRecognizer, StubScheme};

/// Type-checks the public fix surface for an arbitrary scheme meeting the
/// B3.3b.2 bounds. This `#[allow(dead_code)]` function is never *called*; its
/// purpose is the type-check of `fix` / `fix_with_options` /
/// `fix_with_threshold` resolving generically. The live second-scheme fix run
/// (now that `Engine<StubScheme, StubRecognizer>` is constructible, B3.4) lives
/// in `b3_4_second_scheme_construction.rs`.
#[allow(dead_code)]
fn fix_surface_is_generic_over_scheme<S, R>(engine: &Engine<S, R>, source: &[u8]) -> FixResult<S>
where
    S: MarkingScheme + ConstraintBridge,
    S::Canonical: Clone + Default + PartialEq,
    R: Recognizer<S>,
{
    let _ = engine.fix_with_options(source, FixMode::Apply, &FixOptions::default());
    let _ = engine.fix_with_threshold(source, FixMode::Apply, None);
    engine.fix(source, FixMode::Apply)
}

// Force monomorphization for the stub scheme: coercing the generic fn to a
// concrete fn pointer over `Engine<StubScheme, StubRecognizer>` makes the
// compiler instantiate the fix pipeline for a scheme that is emphatically not
// CAPCO (`StubScheme::Canonical = ()`). A `CapcoScheme` leak in any fix call
// site would fail this coercion at compile time.
const _STUB_FIX_REACHABLE: fn(&Engine<StubScheme, StubRecognizer>, &[u8]) -> FixResult<StubScheme> =
    fix_surface_is_generic_over_scheme::<StubScheme, StubRecognizer>;

/// A live run of the same generic surface at the default scheme. The in-crate
/// fix tests already exercise CAPCO behavior exhaustively; this is a single
/// smoke check that the generic entry points still produce a clean result on
/// empty input through the public API.
#[test]
fn capco_fix_through_generic_surface_is_clean_on_empty_input() {
    let engine: Engine = Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");
    let result: FixResult = engine.fix(b"", FixMode::Apply);
    assert!(
        result.audit_lines.is_empty(),
        "empty input applies no fixes"
    );
    assert!(
        result.remaining_diagnostics.is_empty(),
        "empty input produces no diagnostics"
    );
    assert!(
        !result.r002_fired,
        "empty input cannot trigger a reparse failure"
    );
}
