// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for rule **E072** — `bare-rel-portion-divergence`.
//!
//! E072 is `Severity::Warn` by default. Tests verify:
//!
//! - Bare-REL + two explicit REL TO with divergent lists → Warn on divergent portions
//! - No bare-REL portions → no fire (E031 handles banner-only mismatch)
//! - All explicit portions agree on the same list → no fire
//! - NOFORN present → no fire (REL TO superseded)
//! - Multiple divergent portions → one Warn per divergent portion
//! - Only bare-REL portions (no explicit) → no fire (no REL TO list to intersect)
//! - Default severity is Warn (fires without configuration)
//! - E072 carries no text_correction (no automated fix)
//!
//! ## How E072 detects divergence
//!
//! `page_mark.rel_to` is the lattice intersection of all explicit-REL-TO
//! portions on the page — bare-REL portions contribute `Bottom` (universe)
//! and do not restrict the intersection. When bare-REL portions exist AND
//! at least one explicit-REL-TO portion's list is a strict superset of the
//! intersection (i.e., it disagrees with the intersection), E072 fires on
//! that portion.
//!
//! Consequence: with only **one** explicit-REL-TO portion the intersection
//! equals its own list and no divergence is detectable. Real divergence
//! requires at least two explicit portions with differing lists, which
//! reduces the intersection below each individual list.
//!
//! Authority: CAPCO-2016 §H.8 p150-151 re-verified at authorship against
//! `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Severity;

/// Build an engine with default config (E072 is Warn by default).
fn engine_default() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine construction should succeed")
}

/// Find E072 diagnostics in the lint result.
fn e072_diags(
    result: &marque_engine::LintResult,
) -> Vec<&marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    result
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.bare-rel-portion-divergence")
        .collect()
}

// ---------------------------------------------------------------------------
// Trigger: bare-REL + two explicit REL TO with divergent lists
// ---------------------------------------------------------------------------

#[test]
fn e072_fires_when_bare_rel_and_explicit_lists_diverge() {
    let engine = engine_default();
    // page_mark.rel_to = intersection({USA, GBR}, {USA, CAN, GBR}) = {USA, GBR}
    // Portion 3 has extra CAN → diverges from intersection → E072 fires.
    let result = engine.lint(
        b"SECRET//REL TO USA, GBR\n\
          (S//REL) bare rel\n\
          (S//REL TO USA, GBR) matching intersection\n\
          (S//REL TO USA, CAN, GBR) diverging from intersection",
    );
    let diags = e072_diags(&result);
    assert_eq!(
        diags.len(),
        1,
        "expected one E072 for the divergent portion; got {diags:?}",
    );
    assert_eq!(
        diags[0].severity,
        Severity::Warn,
        "E072 must fire at Warn severity",
    );
}

// ---------------------------------------------------------------------------
// Default severity is Warn (no config required)
// ---------------------------------------------------------------------------

#[test]
fn e072_default_severity_is_warn() {
    let engine = engine_default();
    // page_mark.rel_to = {USA, GBR} ∩ {USA, AUS, GBR} = {USA, GBR}
    // Divergent portion fires at Warn without explicit config.
    let result = engine.lint(
        b"SECRET//REL TO USA, GBR\n\
          (S//REL) bare\n\
          (S//REL TO USA, GBR) matching\n\
          (S//REL TO USA, AUS, GBR) divergent",
    );
    let diags = e072_diags(&result);
    assert!(
        !diags.is_empty(),
        "E072 should fire at default (Warn) severity; got {diags:?}",
    );
    for d in &diags {
        assert_eq!(d.severity, Severity::Warn);
    }
}

// ---------------------------------------------------------------------------
// No bare-REL portions → no fire
// ---------------------------------------------------------------------------

#[test]
fn e072_no_fire_when_no_bare_rel_portions() {
    let engine = engine_default();
    // All portions are explicit even though their lists diverge — without
    // bare-REL portions the ambiguity E072 guards against does not arise.
    // E031 banner roll-up handles the explicit-vs-banner mismatch.
    let result = engine.lint(
        b"SECRET//REL TO USA, GBR\n\
          (S//REL TO USA, GBR) matching\n\
          (S//REL TO USA, AUS, GBR) divergent but no bare-REL present",
    );
    let diags = e072_diags(&result);
    assert!(
        diags.is_empty(),
        "E072 must not fire when no bare-REL portions are present; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// All explicit portions agree → no fire
// ---------------------------------------------------------------------------

#[test]
fn e072_no_fire_when_all_explicit_agree() {
    let engine = engine_default();
    // Bare REL + all explicit have the same list → intersection equals all
    // individual lists → no divergence detectable → no diagnostic.
    let result = engine.lint(
        b"SECRET//REL TO USA, GBR\n\
          (S//REL) bare\n\
          (S//REL TO USA, GBR) explicit first\n\
          (S//REL TO USA, GBR) explicit second",
    );
    let diags = e072_diags(&result);
    assert!(
        diags.is_empty(),
        "E072 must not fire when all explicit REL TO portions agree; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// NOFORN present → no fire
// ---------------------------------------------------------------------------

#[test]
fn e072_no_fire_when_noforn_present() {
    let engine = engine_default();
    // NOFORN supersedes REL TO — rule must bail early.
    let result = engine.lint(
        b"SECRET//REL TO USA, GBR//NOFORN\n\
          (S//REL//NF) bare with noforn\n\
          (S//REL TO USA, GBR//NF) first explicit\n\
          (S//REL TO USA, AUS, GBR//NF) divergent but noforn present",
    );
    let diags = e072_diags(&result);
    assert!(
        diags.is_empty(),
        "E072 must not fire when any portion carries NOFORN; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Multiple divergent explicit portions → one Warn per divergent portion
// ---------------------------------------------------------------------------

#[test]
fn e072_fires_once_per_divergent_explicit_portion() {
    let engine = engine_default();
    // page_mark.rel_to = intersection({USA, AUS, GBR}, {USA, CAN, GBR}) = {USA, GBR}
    // Both explicit portions diverge from the intersection → two diagnostics.
    let result = engine.lint(
        b"SECRET//REL TO USA, GBR\n\
          (S//REL) bare\n\
          (S//REL TO USA, AUS, GBR) divergent one\n\
          (S//REL TO USA, CAN, GBR) divergent two",
    );
    let diags = e072_diags(&result);
    assert_eq!(
        diags.len(),
        2,
        "expected one E072 per divergent explicit-REL-TO portion; got {diags:?}",
    );
    for d in &diags {
        assert_eq!(d.severity, Severity::Warn);
    }
}

// ---------------------------------------------------------------------------
// Only bare-REL portions (no explicit) → no fire
// ---------------------------------------------------------------------------

#[test]
fn e072_no_fire_when_only_bare_rel_portions() {
    let engine = engine_default();
    // All portions are bare REL — no explicit REL TO list to form an
    // intersection against, so page_mark.rel_to is empty and the rule bails.
    let result = engine.lint(b"SECRET//REL TO USA, GBR\n(S//REL) first\n(S//REL) second");
    let diags = e072_diags(&result);
    assert!(
        diags.is_empty(),
        "E072 must not fire when there are no explicit-REL-TO portions; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// E072 produces no text_correction (no automated fix)
// ---------------------------------------------------------------------------

#[test]
fn e072_produces_no_text_correction() {
    let engine = engine_default();
    let result = engine.lint(
        b"SECRET//REL TO USA, GBR\n\
          (S//REL) bare\n\
          (S//REL TO USA, GBR) matching\n\
          (S//REL TO USA, AUS, GBR) divergent",
    );
    let diags = e072_diags(&result);
    assert!(!diags.is_empty(), "expected E072 to fire; got {diags:?}");
    for d in &diags {
        assert!(
            d.text_correction.is_none(),
            "E072 must not carry a text_correction (no automated fix for divergence); got {:?}",
            d.text_correction,
        );
    }
}
