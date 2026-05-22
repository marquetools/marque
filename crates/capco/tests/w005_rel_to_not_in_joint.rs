// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! W005 — REL TO list contains entries not in the JOINT participant
//! list. Reverse direction of E014.
//!
//! §H.3 p57 "[LIST]" superset semantics: a classifier may expand REL TO
//! beyond JOINT co-owners (e.g., to additional partners who are not
//! producers but ARE authorized for release). Marque cannot distinguish
//! intentional expansion from an authoring error without classifier
//! input, so W005 surfaces as `Severity::Warn` with NO auto-fix.
//!
//! Tetragraphs in REL TO expand before the check (FVEY → AUS CAN GBR
//! NZL USA, etc.) so the predicate compares the expanded membership
//! against the JOINT participant set.
//!
//! USA is implicitly excluded (US is always a JOINT co-owner per §H.3
//! p55), so `REL TO USA, ...` never contributes USA to the W005
//! "not in JOINT" set.
//!
//! Authority: CAPCO-2016 §H.3 p57. Verified against
//! `crates/capco/docs/CAPCO-2016.md` at the time of authorship per
//! Constitution VIII propagation rule.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Severity;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

fn lint_w005(source: &[u8]) -> Vec<marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    engine()
        .lint(source)
        .diagnostics
        .into_iter()
        .filter(|d| d.rule.predicate_id() == "portion.classification.rel-to-not-in-joint-coverage")
        .collect()
}

// ---------------------------------------------------------------------------
// Core: fires when REL TO exceeds JOINT
// ---------------------------------------------------------------------------

#[test]
fn w005_fires_when_rel_to_exceeds_joint() {
    // JOINT participants = {AUS, CAN, USA}; REL TO = {USA, AUS, CAN, GBR}.
    // GBR is in REL TO but NOT in JOINT → W005 fires (one diagnostic).
    let diags = lint_w005(b"(//JOINT S AUS CAN USA//REL TO USA, AUS, CAN, GBR)\n");
    assert_eq!(
        diags.len(),
        1,
        "W005 should fire when GBR is in REL TO but not JOINT: {diags:?}"
    );
    assert_eq!(
        diags[0].severity,
        Severity::Warn,
        "W005 must be Severity::Warn (no auto-fix; §H.3 p57 superset \
         semantics permit intentional expansion): {:?}",
        diags[0].severity,
    );
}

#[test]
fn w005_does_not_fire_when_rel_to_matches_joint() {
    // JOINT = {AUS, CAN, USA}; REL TO = {USA, AUS, CAN}. Exact match
    // modulo USA — no W005.
    let diags = lint_w005(b"(//JOINT S AUS CAN USA//REL TO USA, AUS, CAN)\n");
    assert!(
        diags.is_empty(),
        "W005 must not fire when all REL TO entries are JOINT participants: {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// USA exclusion: USA is implicit US co-ownership
// ---------------------------------------------------------------------------

#[test]
fn w005_excludes_usa_implicit_coowner() {
    // JOINT = {AUS, USA}; REL TO = {USA, AUS, GBR}. Only GBR is the
    // "extra" entry; USA is always a JOINT co-owner per §H.3 p55 and
    // should NOT contribute to W005's not-in-JOINT set even when it
    // appears in REL TO.
    let diags = lint_w005(b"(//JOINT S AUS USA//REL TO USA, AUS, GBR)\n");
    assert_eq!(
        diags.len(),
        1,
        "W005 should fire exactly once for GBR (not USA): {diags:?}"
    );
    assert_eq!(
        diags[0].severity,
        Severity::Warn,
        "W005 must be Severity::Warn"
    );
}

// ---------------------------------------------------------------------------
// Tetragraph expansion
// ---------------------------------------------------------------------------

#[test]
fn w005_expands_fvey_tetragraph() {
    // FVEY expands to AUS CAN GBR NZL USA. JOINT has only {AUS, CAN, USA}.
    // GBR and NZL are FVEY members but not JOINT participants → W005
    // fires (one diagnostic covering both extras).
    let diags = lint_w005(b"(//JOINT S AUS CAN USA//REL TO USA, FVEY)\n");
    assert_eq!(
        diags.len(),
        1,
        "W005 should fire when FVEY expansion has members beyond JOINT: {diags:?}"
    );
    assert_eq!(
        diags[0].severity,
        Severity::Warn,
        "W005 must be Severity::Warn"
    );
}

#[test]
fn w005_does_not_fire_when_fvey_covers_joint() {
    // FVEY = AUS CAN GBR NZL USA; JOINT also has all 5 FVEY members.
    // Tetragraph expansion produces an exact match — no W005.
    let diags = lint_w005(b"(//JOINT S AUS CAN GBR NZL USA//REL TO USA, FVEY)\n");
    assert!(
        diags.is_empty(),
        "W005 must not fire when tetragraph members all appear in JOINT: {diags:?}",
    );
}
