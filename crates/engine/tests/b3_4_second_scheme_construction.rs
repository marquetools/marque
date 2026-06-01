// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Live construction of `Engine` over a second, non-CAPCO scheme.
//!
//! Phase B3.4 closed the constructor scheme-discard: `Engine::new` /
//! `with_clock` used to take a generic `scheme: S`, use it only to schedule
//! page rewrites, then `drop(scheme)` and store a fresh `CapcoScheme::new()`.
//! The generic core `Engine::with_clock_and_recognizer` now stores the passed
//! scheme and recognizer directly, so `Engine<StubScheme, StubRecognizer>` is
//! genuinely constructible.
//!
//! The B3.3b / B3.3b.2 reachability guards proved the lint / fix surfaces are
//! *typeable* over a second scheme (compile-time `const`-fn-ptr coercions). A
//! type-check cannot catch a hardcoded-`"capco"` *runtime value* leak: a wrong
//! string is not a type error. This file is the behavioral complement —
//! construct the second-scheme engine for real, run lint + fix through it, and
//! assert that the engine reports the scheme it was *given* (`scheme_id() ==
//! "stub"`), not the `CapcoScheme` the old discard substituted (`"capco"`).

use marque_config::Config;
use marque_engine::{Engine, FixMode, SystemClock};
use marque_rules::RuleSet;
use marque_scheme::MarkingScheme;
use marque_test_utils::stub_scheme::{StubRecognizer, StubScheme};

/// Build a live `Engine<StubScheme, StubRecognizer>` through the generic
/// constructor. No rules are registered; the stub recognizer recognizes
/// nothing and the stub scheme declares no rewrites or constraints, so the
/// engine is the minimal viable second-scheme instance.
fn build_stub_engine() -> Engine<StubScheme, StubRecognizer> {
    Engine::with_clock_and_recognizer(
        Config::default(),
        Vec::<Box<dyn RuleSet<StubScheme>>>::new(),
        StubScheme::new(),
        StubRecognizer,
        Box::new(SystemClock),
    )
    .expect("StubScheme declares no rewrites, so scheduling cannot fail")
}

/// The constructor stores the passed scheme rather than discarding it for a
/// fresh `CapcoScheme::new()`. `scheme_id()` is `"stub"` for `StubScheme` and
/// `"capco"` for `CapcoScheme`; observing `"stub"` here is the direct evidence
/// the discard is closed — under the old behavior `engine.scheme()` returned
/// the substituted CAPCO scheme.
#[test]
fn engine_stores_the_passed_second_scheme() {
    let engine = build_stub_engine();
    assert_eq!(
        engine.scheme().scheme_id(),
        "stub",
        "the engine must store the scheme it was given, not a substituted CapcoScheme"
    );
    assert_eq!(engine.scheme().name(), "stub");
}

/// A live lint through the second scheme. The stub recognizer returns
/// zero-candidate `Ambiguous` for every candidate and the scheme registers no
/// rules, so a clean run produces no diagnostics — the point is that the
/// pipeline executes end-to-end over a non-CAPCO scheme.
#[test]
fn lint_runs_through_the_second_scheme() {
    let engine = build_stub_engine();
    let result = engine.lint(b"(S//STUB) some text TOP SECRET//STUB");
    assert!(
        result.diagnostics.is_empty(),
        "stub scheme registers no rules and recognizes nothing: no diagnostics"
    );
}

/// A live fix through the second scheme. With no rules and nothing recognized,
/// the fix pipeline applies nothing and surfaces no diagnostics — exercising
/// the generic fix path against a non-CAPCO scheme at runtime, not just at
/// type-check time.
#[test]
fn fix_runs_through_the_second_scheme() {
    let engine = build_stub_engine();
    let result = engine.fix(b"(S//STUB) some text", FixMode::Apply);
    assert!(
        result.audit_lines.is_empty(),
        "no rules registered: no fixes applied, so no audit lines"
    );
    assert!(
        result.remaining_diagnostics.is_empty(),
        "no rules registered: no residual diagnostics"
    );
}
