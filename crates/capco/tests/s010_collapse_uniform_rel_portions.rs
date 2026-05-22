// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for rule **S010** — `collapse-uniform-rel-portions`.
//!
//! S010 is `Severity::Off` by default; all active tests configure the engine
//! with `[rules] S010 = "suggest"` to activate it. Tests cover:
//!
//! - Off-by-default gate (rule does not fire when not configured)
//! - All explicit REL TO portions match banner → Suggest per portion
//! - Some portions diverge → no fire (gate: all-or-nothing)
//! - No explicit REL TO portions → no fire
//! - NOFORN present → no fire (REL TO superseded)
//! - Multiple matching portions → one Suggest per portion
//! - Replacement is exactly `"REL"` (compact form, not `"REL TO ..."`)
//! - Already compact (bare `REL` portions present, no explicit) → no fire
//!
//! Authority: CAPCO-2016 §H.8 p150 re-verified at authorship against
//! `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Severity;

/// Build an engine with S010 enabled at `suggest` severity.
fn engine_with_s010() -> Engine {
    let mut config = Config::default();
    config.rules.overrides.insert(
        // T044: rule-override key uses the wire-string form per OD-7.
        "capco:page.dissem.collapse-uniform-rel-portions".to_string(),
        "suggest".to_string(),
    );
    Engine::new(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine construction should succeed")
}

/// Build an engine with S010 at default severity (Off).
fn engine_default() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine construction should succeed")
}

/// Find S010 diagnostics in the lint result.
fn s010_diags(
    result: &marque_engine::LintResult,
) -> Vec<&marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    result
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.collapse-uniform-rel-portions")
        .collect()
}

// ---------------------------------------------------------------------------
// Off-by-default gate
// ---------------------------------------------------------------------------

#[test]
fn s010_does_not_fire_when_off_by_default() {
    let engine = engine_default();
    // All portions match the same REL TO list — would fire if enabled.
    let result = engine
        .lint(b"SECRET//REL TO USA, GBR\n(S//REL TO USA, GBR) paragraph one\n(S//REL TO USA, GBR) paragraph two");
    let diags = s010_diags(&result);
    assert!(
        diags.is_empty(),
        "S010 must not fire when severity is Off (default); got: {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Single portion — all explicit match banner → Suggest
// ---------------------------------------------------------------------------

#[test]
fn s010_fires_when_single_portion_matches_banner() {
    let engine = engine_with_s010();
    let result = engine.lint(b"SECRET//REL TO USA, GBR\n(S//REL TO USA, GBR) paragraph text");
    let diags = s010_diags(&result);
    assert_eq!(
        diags.len(),
        1,
        "expected exactly one S010 diagnostic; got {diags:?}",
    );
    let d = &diags[0];
    assert_eq!(d.severity, Severity::Suggest);
    let tc = d
        .text_correction
        .as_ref()
        .expect("S010 must carry a text_correction replacement");
    assert_eq!(
        tc.replacement.as_str(),
        "REL",
        "S010 replacement must be compact 'REL'; got {:?}",
        tc.replacement,
    );
}

// ---------------------------------------------------------------------------
// Multiple matching portions → one Suggest per portion
// ---------------------------------------------------------------------------

#[test]
fn s010_fires_once_per_matching_portion() {
    let engine = engine_with_s010();
    let result = engine.lint(
        b"SECRET//REL TO USA, GBR\n\
          (S//REL TO USA, GBR) first\n\
          (S//REL TO USA, GBR) second\n\
          (S//REL TO USA, GBR) third",
    );
    let diags = s010_diags(&result);
    assert_eq!(
        diags.len(),
        3,
        "expected one S010 per explicit-REL-TO portion; got {diags:?}",
    );
    for d in &diags {
        assert_eq!(d.severity, Severity::Suggest);
        let tc = d
            .text_correction
            .as_ref()
            .expect("every S010 must carry text_correction");
        assert_eq!(tc.replacement.as_str(), "REL");
    }
}

// ---------------------------------------------------------------------------
// Mixed: some portions diverge → no fire (all-or-nothing gate)
// ---------------------------------------------------------------------------

#[test]
fn s010_no_fire_when_any_portion_diverges() {
    let engine = engine_with_s010();
    // Second portion has DEU in addition — does not match the banner list.
    let result = engine.lint(
        b"SECRET//REL TO USA, GBR\n\
          (S//REL TO USA, GBR) matching\n\
          (S//REL TO USA, DEU, GBR) diverging",
    );
    let diags = s010_diags(&result);
    assert!(
        diags.is_empty(),
        "S010 must not fire when any explicit-REL-TO portion diverges; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// No explicit REL TO portions → no fire
// ---------------------------------------------------------------------------

#[test]
fn s010_no_fire_when_no_explicit_rel_to_portions() {
    let engine = engine_with_s010();
    // Portions have bare REL (no explicit list) — nothing to collapse.
    let result = engine.lint(b"SECRET//REL TO USA, GBR\n(S//REL) paragraph");
    let diags = s010_diags(&result);
    assert!(
        diags.is_empty(),
        "S010 must not fire when there are no explicit-REL-TO portions; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// NOFORN present → no fire
// ---------------------------------------------------------------------------

#[test]
fn s010_no_fire_when_noforn_present() {
    let engine = engine_with_s010();
    // One portion carries NOFORN — REL TO is superseded; rule must bail.
    let result = engine.lint(
        b"SECRET//REL TO USA, GBR//NOFORN\n\
          (S//REL TO USA, GBR) no-noforn portion\n\
          (S//REL TO USA, GBR//NF) noforn portion",
    );
    let diags = s010_diags(&result);
    assert!(
        diags.is_empty(),
        "S010 must not fire when any portion carries NOFORN; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Replacement is compact "REL", not "REL TO ..."
// ---------------------------------------------------------------------------

#[test]
fn s010_replacement_is_exactly_rel() {
    let engine = engine_with_s010();
    let result = engine.lint(b"TOP SECRET//REL TO USA, AUS, CAN\n(TS//REL TO USA, AUS, CAN) para");
    let diags = s010_diags(&result);
    assert!(!diags.is_empty(), "expected S010 to fire; got {diags:?}");
    let tc = diags[0]
        .text_correction
        .as_ref()
        .expect("must have text_correction");
    assert_eq!(
        tc.replacement.as_str(),
        "REL",
        "replacement must be exactly 'REL' (not 'REL TO ...'): got {:?}",
        tc.replacement,
    );
}

// ---------------------------------------------------------------------------
// Tetragraph in portion expands to match trigraph-form banner
// ---------------------------------------------------------------------------

#[test]
fn s010_fires_when_portion_uses_tetragraph_matching_banner_trigraphs() {
    let engine = engine_with_s010();
    // Banner spells out the FVEY members as individual trigraphs.
    // Portion uses the FVEY tetragraph — after expansion both sides are
    // {AUS, CAN, GBR, NZL, USA}, so the rule must fire.
    let result = engine.lint(
        b"SECRET//REL TO USA, AUS, CAN, GBR, NZL\n\
          (S//REL TO USA, FVEY) fvey portion",
    );
    let diags = s010_diags(&result);
    assert_eq!(
        diags.len(),
        1,
        "S010 must fire when tetragraph-expanded portion matches banner; got {diags:?}",
    );
    assert_eq!(
        diags[0]
            .text_correction
            .as_ref()
            .expect("must carry text_correction")
            .replacement
            .as_str(),
        "REL",
    );
}
