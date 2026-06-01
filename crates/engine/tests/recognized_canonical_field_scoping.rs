// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #699 — scoping pin for `Diagnostic.recognized_canonical`.
//!
//! The field is populated only by `build_decoder_diagnostic` in
//! `marque-capco` today (for R001 / `engine:recognition.decoder-
//! recognized` diagnostics). This test file pins the absence of the
//! field on every other diagnostic path the engine emits so a future
//! rule that gains a `recognized_canonical` payload by accident
//! (e.g., a copy-paste of the decoder builder, a too-eager
//! `with_recognized_canonical(...)` chain) fails this test rather
//! than silently leaking content into the lint-side renderer for
//! rule families that haven't been audited for it.
//!
//! Audit-content-ignorance is pinned separately in
//! `recognized_canonical_lint_vs_fix.rs` and `audit_g13_canary.rs`;
//! this file pins lint-side rule-emission scoping only.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::CapcoEngine;
use marque_rules::{Diagnostic, RuleSet};

fn build_engine() -> CapcoEngine {
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    CapcoEngine::new(Config::default(), rule_sets, CapcoScheme::new())
        .expect("default CAPCO scheme constructs without rewrite cycles")
}

fn build_engine_with_corrections() -> CapcoEngine {
    let mut config = Config::default();
    config.corrections.insert("SERCET".into(), "SECRET".into());
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    CapcoEngine::new(config, rule_sets, CapcoScheme::new())
        .expect("default CAPCO scheme with corrections constructs without rewrite cycles")
}

/// Locate the first diagnostic for a given predicate ID.
fn find_predicate<'a>(
    diags: &'a [Diagnostic<CapcoScheme>],
    predicate_id: &str,
) -> Option<&'a Diagnostic<CapcoScheme>> {
    diags.iter().find(|d| d.rule.predicate_id() == predicate_id)
}

#[test]
fn r001_recognized_canonical_absent_for_non_decoder_diagnostic() {
    // `(S//XYZZY)` — `XYZZY` is a wrong-shape token in the dissem
    // category position. The strict parser does not classify it as a
    // recognized marking; E008 (`marking.metadata.unrecognized-token`)
    // fires. E008 is not a decoder-emitted rule; the
    // `recognized_canonical` field is unpopulated.
    let engine = build_engine();
    let lint = engine.lint(b"(S//XYZZY)");
    let e008 = find_predicate(&lint.diagnostics, "marking.metadata.unrecognized-token")
        .expect("E008 must fire on unrecognized token");
    assert!(
        e008.recognized_canonical.is_none(),
        "non-decoder diagnostics must not carry recognized_canonical",
    );
}

#[test]
fn r001_recognized_canonical_absent_for_text_correction() {
    // `(SERCET//NF)` — the engine's pre-scanner aho-corasick
    // corrections pass canonicalizes `SERCET → SECRET`. The
    // resulting C001 diagnostic carries its replacement bytes via
    // `text_correction`, NOT via `recognized_canonical` — the two
    // fields are independent surface channels and the issue #699
    // wiring is decoder-side only.
    let engine = build_engine_with_corrections();
    let lint = engine.lint(b"(SERCET//NF)");
    let c001 = lint
        .diagnostics
        .iter()
        .find(|d| d.text_correction.is_some())
        .expect("a text_correction diagnostic must fire for the SERCET → SECRET fixup");
    assert!(
        c001.text_correction.is_some(),
        "the text_correction channel carries the replacement bytes",
    );
    assert!(
        c001.recognized_canonical.is_none(),
        "text_correction diagnostics must not also carry recognized_canonical",
    );
}

#[test]
fn r001_prose_glue_suppressed_no_diagnostic() {
    // `letter(s)` — prose-glued single-letter portion. The decoder's
    // `preceded_by_whitespace = false` early-return suppresses any
    // candidate emission, so no R001 diagnostic exists and no
    // `recognized_canonical` payload reaches the lint surface. This
    // pins the "prose glue produces no diagnostic at all" contract
    // that the field's `Some(_)`-on-R001 invariant rests on.
    let engine = build_engine();
    let lint = engine.lint(b"the letter(s)");
    assert!(
        lint.diagnostics.is_empty(),
        "prose-glued single-letter portion must produce zero diagnostics; got: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| (d.rule.predicate_id(), d.message.template().as_str()))
            .collect::<Vec<_>>(),
    );
}
