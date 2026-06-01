// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Object-safe erasure + heterogeneous co-residence (Phase B4.2).
//!
//! `MarkingScheme` has associated types and is not object-safe, so engines
//! over different schemes cannot share a `Vec<Engine<S>>`. `ErasedEngine`
//! erases the *output* — bytes in, grammar-tagged scheme-agnostic results out
//! — so heterogeneous engines co-reside as `Box<dyn ErasedEngine>` behind one
//! `MultiGrammarEngine`. These tests prove:
//!
//! 1. `ErasedEngine` is object-safe (`assert_obj_safe!`).
//! 2. A CAPCO engine and a non-CAPCO (`StubScheme`) engine genuinely co-reside
//!    behind one registry and each grammar's rules run independently.
//! 3. The grammar tag round-trips: every `ErasedLintResult.grammar_id` equals
//!    the boxed engine's `scheme().scheme_id()`.
//! 4. Erasure boxes at most once per scheme per document (the registry holds
//!    each box for the whole run; lint is one vtable dispatch per grammar).
//! 5. `fix_erased` pre-renders the audit stream to NDJSON for any scheme.

use marque_capco::CapcoRuleSet;
use marque_config::Config;
use marque_engine::{CapcoEngine, Engine, ErasedEngine, FixMode, MultiGrammarEngine, SystemClock};
use marque_rules::RuleSet;
use marque_scheme::{InputContext, InputSource, MarkingScheme};
use marque_test_utils::stub_scheme::{StubRecognizer, StubScheme};

// `ErasedEngine` must be object-safe — the whole co-residence design rests on
// `Box<dyn ErasedEngine>` being a valid type.
static_assertions::assert_obj_safe!(ErasedEngine);

/// A live CAPCO engine with the full rule set (produces real diagnostics).
fn capco_engine() -> CapcoEngine {
    CapcoEngine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// A live engine over the non-CAPCO `StubScheme`: no rules, recognizes
/// nothing, so it produces no diagnostics — the minimal second grammar.
fn stub_engine() -> Engine<StubScheme, StubRecognizer> {
    Engine::with_clock_and_recognizer(
        Config::default(),
        Vec::<Box<dyn RuleSet<StubScheme>>>::new(),
        StubScheme::new(),
        StubRecognizer,
        Box::new(SystemClock),
    )
    .expect("StubScheme declares no rewrites, so scheduling cannot fail")
}

/// A document-content `InputContext` (the conservative text path).
fn doc_ctx() -> InputContext<'static> {
    InputContext::new(InputSource::DocumentContent)
}

/// Two distinct concrete schemes coexist behind `Box<dyn ErasedEngine>` in one
/// registry; each grammar's rules run independently. This is the load-bearing
/// co-residence proof (tasks T028 / T029).
#[test]
fn two_schemes_coexist_and_run_independently() {
    let mut registry = MultiGrammarEngine::new();
    registry.register(Box::new(capco_engine()));
    registry.register(Box::new(stub_engine()));

    assert_eq!(registry.len(), 2);
    assert_eq!(
        registry.grammar_ids().collect::<Vec<_>>(),
        vec!["capco", "stub"],
        "grammar tags follow registration order"
    );

    // A banner that fires a known CAPCO rule (REL TO missing USA).
    let results = registry.lint(b"SECRET//REL TO GBR\n", &doc_ctx());
    assert_eq!(results.len(), 2, "one result per registered grammar");

    let capco = &results[0];
    let stub = &results[1];

    assert_eq!(capco.grammar_id, "capco");
    assert_eq!(stub.grammar_id, "stub");

    // The CAPCO grammar produces its expected diagnostic; the stub produces
    // none — proof the two rule sets run independently, not commingled.
    assert!(
        capco
            .diagnostics
            .iter()
            .any(|d| d.rule.predicate_id() == "portion.dissem.rel-to-missing-usa"),
        "CAPCO grammar must fire its rule, got: {:?}",
        capco.diagnostics
    );
    assert!(
        stub.is_clean(),
        "stub grammar registers no rules: no diagnostics, got: {:?}",
        stub.diagnostics
    );
}

/// Every erased result's `grammar_id` round-trips to the boxed engine's
/// `scheme().scheme_id()`.
#[test]
fn grammar_tag_round_trips() {
    let capco = capco_engine();
    let stub = stub_engine();
    let capco_id = capco.scheme().scheme_id();
    let stub_id = stub.scheme().scheme_id();

    let mut registry = MultiGrammarEngine::new();
    registry.register(Box::new(capco));
    registry.register(Box::new(stub));

    let results = registry.lint(b"(TS//SI//NF)\n", &doc_ctx());
    assert_eq!(results[0].grammar_id, capco_id);
    assert_eq!(results[1].grammar_id, stub_id);
}

/// A single boxed engine handles multiple documents through the shared
/// registry reference — structurally one box per scheme for the registry's
/// lifetime, never a box per candidate or per diagnostic.
#[test]
fn boxing_is_once_per_scheme_not_per_document() {
    let mut registry = MultiGrammarEngine::new();
    registry.register(Box::new(capco_engine()));

    // Lint several documents through the same registry. The box was created
    // once at `register`; each `lint` is a vtable dispatch over the held box.
    for doc in [
        b"SECRET//REL TO GBR\n".as_slice(),
        b"(TS//SI//NF)\n".as_slice(),
        b"".as_slice(),
    ] {
        let results = registry.lint(doc, &doc_ctx());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].grammar_id, "capco");
    }
}

/// `fix_erased` runs the typed fix and pre-renders the audit stream to NDJSON.
/// On a clean input nothing is fixed, so the audit stream is empty — the point
/// is that the erased fix surface dispatches end-to-end and tags the result.
#[test]
fn fix_erased_dispatches_and_tags() {
    let engine = capco_engine();
    let boxed: Box<dyn ErasedEngine> = Box::new(engine);

    let result = boxed.fix_erased(b"(TS//SI//NF)\n", FixMode::Apply);
    assert_eq!(result.grammar_id, "capco");
    // Clean portion: no fixes, so no audit lines and no residual diagnostics.
    assert!(result.audit_ndjson.is_empty());
    assert!(result.remaining_diagnostics.is_empty());
    assert!(!result.r002_fired);
}

/// `fix_erased` over the stub scheme: the generic audit-render path renders
/// `AuditLine<StubScheme>` to NDJSON without any CAPCO assumption. No rules =
/// no audit lines, but the surface must dispatch for a non-CAPCO scheme.
#[test]
fn fix_erased_works_for_non_capco_scheme() {
    let boxed: Box<dyn ErasedEngine> = Box::new(stub_engine());
    let result = boxed.fix_erased(b"(S//STUB) text", FixMode::Apply);
    assert_eq!(result.grammar_id, "stub");
    assert!(result.audit_ndjson.is_empty());
    assert!(result.remaining_diagnostics.is_empty());
}
