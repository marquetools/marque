// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #439 — `RelToTrigraphSuggestRule` (S004) coverage exclusion.
//!
//! S004 stays silent when its candidate replacement trigraph is already
//! covered by another entry in the same REL TO block — either directly
//! (another trigraph equals the candidate) or transitively (another
//! entry is a decomposable tetragraph whose expansion contains the
//! candidate). The author's rare entry cannot plausibly be a typo for
//! an already-permitted recipient.
//!
//! # Driving pair
//!
//! `ASM` (American Samoa, trigraph) is rare in the marking-stratum
//! corpus and within edit distance 2 of `USA`. The
//! `log_prior(USA) - log_prior(ASM)` delta exceeds
//! `SUGGEST_LOG_MARGIN = 4.0`, so the rule fires on a bare
//! `(C//REL TO ASM, …)` portion. This integration suite uses that pair
//! as the trigger; the issue-spec's `AUT → AUS` example no longer fires
//! at the current corpus calibration (the priors have shifted across
//! corpus growth and stratification work — issue #258), but the
//! coverage-exclusion semantics the issue describes are independent of
//! which pair drives the rule.
//!
//! Authority: CAPCO-2016 §D.2 Table 3 Row 23 pp28–30 licenses
//! tetragraph-to-trigraph expansion for banner-line REL TO roll-up
//! ("Expansion of the TEYE, ACGU, and FVEY tetragraphs is allowed for
//! common country roll-up of banner line REL TO [USA, LIST]
//! marking"). §H.8 p151 (REL TO Precedence Rules for Banner Line
//! Guidance) delegates roll-up semantics to §D.2 Table 3 by reference.
//! The tetragraph-membership data source is the ODNI ISMCAT
//! Tetragraph Taxonomy (`decomposable="Yes"` rows; surfaced through
//! `marque_capco::vocab::expand_tetragraph`).

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{Diagnostic, RuleSet};

fn engine() -> Engine {
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    Engine::new(Config::default(), rule_sets, CapcoScheme::new())
        .expect("default CAPCO scheme constructs without rewrite cycles")
}

fn s004_diags(source: &[u8]) -> Vec<Diagnostic<CapcoScheme>> {
    engine()
        .lint(source)
        .diagnostics
        .into_iter()
        .filter(|d| d.rule.as_str() == "S004")
        .collect()
}

fn replacement(d: &Diagnostic<CapcoScheme>) -> &str {
    d.text_correction
        .as_ref()
        .expect("S004 emits text_correction")
        .replacement
        .as_str()
}

// =========================================================================
// Negative controls: rule still fires when no coverage is present.
// =========================================================================

#[test]
fn s004_asm_alone_still_fires() {
    // Baseline: ASM rare, no USA-covering entry in the block →
    // S004 suggests `ASM → USA`. The block carries DEU (a registered
    // trigraph at edit distance 3 from ASM and not a decomposable
    // tetragraph) so it contributes no coverage of USA.
    let diags = s004_diags(b"(C//REL TO ASM, DEU)\n");
    assert_eq!(
        diags.len(),
        1,
        "ASM alone with no USA-covering entry must fire S004; got {} diags ({diags:?})",
        diags.len(),
    );
    assert_eq!(replacement(&diags[0]), "USA");
}

#[test]
fn s004_asm_with_atomic_eu_still_fires() {
    // `EU` is `decomposable="No"` in ISMCAT V2022-NOV — atomic and
    // does NOT expand to constituent trigraphs.
    // `expand_tetragraph("EU")` returns `None`, so EU cannot suppress
    // the ASM → USA suggestion.
    let diags = s004_diags(b"(C//REL TO ASM, EU)\n");
    assert_eq!(
        diags.len(),
        1,
        "EU is atomic (decomposable=No); cannot cover USA; S004 must still fire on ASM",
    );
    assert_eq!(replacement(&diags[0]), "USA");
}

// =========================================================================
// Positive cases: candidate is covered → no diagnostic.
// =========================================================================

#[test]
fn s004_asm_with_direct_usa_suppressed() {
    // Case 1 of the issue spec: another entry IS the candidate
    // replacement (direct trigraph match).
    let diags = s004_diags(b"(C//REL TO USA, ASM)\n");
    assert!(
        diags.is_empty(),
        "direct USA in block covers the ASM → USA suggestion; S004 must stay silent; got {diags:?}",
    );
}

#[test]
fn s004_asm_with_fvey_suppressed() {
    // Case 2 of the issue spec: another entry is a decomposable
    // tetragraph (FVEY = AUS, CAN, GBR, NZL, USA) whose expansion
    // contains the candidate (USA).
    let diags = s004_diags(b"(C//REL TO ASM, FVEY)\n");
    assert!(
        diags.is_empty(),
        "FVEY expands to include USA; S004 must stay silent on ASM; got {diags:?}",
    );
}

#[test]
fn s004_asm_with_acgu_suppressed() {
    // Generality pin: the suppression is NOT FVEY/NATO-specific.
    // ACGU = AUS, CAN, GBR, USA also contains USA and must suppress.
    let diags = s004_diags(b"(C//REL TO ASM, ACGU)\n");
    assert!(
        diags.is_empty(),
        "ACGU expands to include USA; S004 must stay silent on ASM; got {diags:?}",
    );
}

#[test]
fn s004_asm_with_nato_suppressed() {
    // Generality pin: NATO (a third decomposable tetragraph that
    // also contains USA — USA is a NATO member). Distinct expansion
    // from FVEY/ACGU; pins that the table is consulted generically.
    let diags = s004_diags(b"(C//REL TO ASM, NATO)\n");
    assert!(
        diags.is_empty(),
        "NATO expands to include USA; S004 must stay silent on ASM; got {diags:?}",
    );
}

#[test]
fn s004_asm_with_australia_group_suppressed() {
    // Generality pin: AUSTRALIA_GROUP (a fourth decomposable
    // tetragraph — 40-member multilateral export-control regime
    // that includes USA). Confirms the suppression is over the
    // whole `decomposable="Yes"` set, not a hardcoded 3-tetragraph
    // shortlist.
    let diags = s004_diags(b"(C//REL TO ASM, AUSTRALIA_GROUP)\n");
    assert!(
        diags.is_empty(),
        "AUSTRALIA_GROUP expands to include USA; S004 must stay silent on ASM; got {diags:?}",
    );
}
