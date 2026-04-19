// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 5 — Corrections-map integration tests (T054).
//!
//! Exercises FR-009: user corrections take precedence over built-in rules
//! when both match the same span. The C001 rule emits `FixSource::CorrectionsMap`
//! and `FixSource::CorrectionsMap` for audit trail fidelity.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_rules::FixSource;
use std::collections::HashMap;
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

fn engine_with_corrections(corrections: HashMap<String, String>) -> Engine {
    let mut config = Config::default();
    config.corrections = corrections;
    Engine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
}

fn engine_default() -> Engine {
    engine_with_corrections(HashMap::new())
}

// -----------------------------------------------------------------------
// C001 basics
// -----------------------------------------------------------------------

#[test]
fn c001_fires_on_corrections_map_match() {
    // C001 can only correct tokens inside markings the scanner detects.
    // The scanner recognizes banners starting with known classification
    // prefixes (SECRET, TOP SECRET, etc.), so we use a valid banner with
    // a corrections-map entry matching a dissem control token.
    let mut corrections = HashMap::new();
    corrections.insert("NF".to_owned(), "NOFORN".to_owned());
    let engine = engine_with_corrections(corrections);

    let source = b"SECRET//NF\n";
    let result = engine.lint(source);

    let c001_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "C001")
        .collect();

    assert!(
        !c001_diags.is_empty(),
        "C001 must fire when corrections map matches a token"
    );
    let fix = c001_diags[0].fix.as_ref().expect("C001 should have a fix");
    assert_eq!(fix.source, FixSource::CorrectionsMap);
    assert_eq!(fix.replacement.as_ref(), "NOFORN");
    assert!((fix.confidence - 1.0).abs() < f32::EPSILON);
    assert_eq!(fix.migration_ref, None);
}

#[test]
fn c001_no_match_when_corrections_empty() {
    let engine = engine_default();
    let source = b"SECRET//NOFORN\n";
    let result = engine.lint(source);

    let c001_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "C001")
        .collect();
    assert!(
        c001_diags.is_empty(),
        "C001 should not fire with empty corrections map"
    );
}

#[test]
fn c001_no_match_when_token_not_in_map() {
    // Corrections map has "SERCET" but the input contains "SECRET" — no match.
    let mut corrections = HashMap::new();
    corrections.insert("SERCET".to_owned(), "SECRET".to_owned());
    let engine = engine_with_corrections(corrections);

    let source = b"SECRET//NOFORN\n";
    let result = engine.lint(source);

    let c001_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "C001")
        .collect();
    assert!(
        c001_diags.is_empty(),
        "C001 should not fire when no token matches corrections map"
    );
}

// -----------------------------------------------------------------------
// FR-009: corrections-map precedence over built-in rules
// -----------------------------------------------------------------------

#[test]
fn fr009_c001_wins_over_builtin_rule_on_same_span() {
    // Set up a corrections map that matches a dissem control that E001
    // would also flag: "NF" → "NOFORN". Both C001 and E001 will fire
    // on the same span. FR-016 sort + C-1 overlap guard should keep C001
    // (because "C001" < "E001" lexicographically).
    let mut corrections = HashMap::new();
    corrections.insert("NF".to_owned(), "NOFORN".to_owned());
    let engine = engine_with_corrections(corrections);

    let source = b"SECRET//NF\n";
    let result = engine.fix(source, FixMode::Apply);

    // At least one fix should be applied.
    assert!(
        !result.applied.is_empty(),
        "at least one fix should be applied for NF→NOFORN"
    );

    // Find the fix for the NF span.
    let nf_fixes: Vec<_> = result
        .applied
        .iter()
        .filter(|f| f.proposal.replacement.as_ref() == "NOFORN")
        .collect();

    // Both C001 and E001 compete for the NF span. C-1 overlap guard keeps
    // only one. FR-016 sort picks C001 ("C001" < "E001" lexicographically).
    assert_eq!(
        nf_fixes.len(),
        1,
        "exactly one NOFORN fix should be applied (C-1 overlap guard)"
    );
    assert_eq!(
        nf_fixes[0].proposal.rule.as_str(),
        "C001",
        "C001 should win over E001 on the same span (FR-009)"
    );
    assert_eq!(nf_fixes[0].proposal.source, FixSource::CorrectionsMap);
}

#[test]
fn c001_fix_carries_corrections_map_source_in_audit() {
    let mut corrections = HashMap::new();
    corrections.insert("NF".to_owned(), "NOFORN".to_owned());
    let engine = engine_with_corrections(corrections);

    let source = b"SECRET//NF\n";
    let result = engine.fix(source, FixMode::Apply);

    let c001_fixes: Vec<_> = result
        .applied
        .iter()
        .filter(|f| f.proposal.rule.as_str() == "C001")
        .collect();

    assert!(
        !c001_fixes.is_empty(),
        "C001 must appear in applied fixes for NF→NOFORN"
    );
    assert_eq!(c001_fixes[0].proposal.source, FixSource::CorrectionsMap);
    assert_eq!(c001_fixes[0].proposal.migration_ref, None);
}

// -----------------------------------------------------------------------
// Classifier ID propagation (T060)
// -----------------------------------------------------------------------

#[test]
fn classifier_id_propagated_into_audit_records() {
    let mut config = Config::default();
    config.user.classifier_id = Some("TEST-AUDIT-99".to_owned());
    let engine = Engine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    );

    let source = b"SECRET//NF\n";
    let result = engine.fix(source, FixMode::Apply);

    for fix in &result.applied {
        assert_eq!(
            fix.classifier_id.as_deref(),
            Some("TEST-AUDIT-99"),
            "classifier_id must propagate from config into audit records"
        );
    }
}

#[test]
fn absent_classifier_id_is_none_in_audit() {
    let engine = engine_default();
    let source = b"SECRET//NF\n";
    let result = engine.fix(source, FixMode::Apply);

    for fix in &result.applied {
        assert!(
            fix.classifier_id.is_none(),
            "absent classifier_id should be None, not empty"
        );
    }
}

// -----------------------------------------------------------------------
// Edge cases (M1, M2, T3, T4)
// -----------------------------------------------------------------------

#[test]
fn c001_does_not_fire_on_separator_tokens() {
    // M1: corrections map entry for "//" must not match separator tokens.
    let mut corrections = HashMap::new();
    corrections.insert("//".to_owned(), "///".to_owned());
    let engine = engine_with_corrections(corrections);

    let source = b"SECRET//NOFORN\n";
    let result = engine.lint(source);

    let c001_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "C001")
        .collect();
    assert!(
        c001_diags.is_empty(),
        "C001 must not fire on separator tokens, got: {c001_diags:?}"
    );
}

#[test]
fn c001_skips_noop_correction() {
    // M2: a corrections entry where key == value must not produce a fix.
    let mut corrections = HashMap::new();
    corrections.insert("NOFORN".to_owned(), "NOFORN".to_owned());
    let engine = engine_with_corrections(corrections);

    let source = b"SECRET//NOFORN\n";
    let result = engine.fix(source, FixMode::Apply);

    let c001_fixes: Vec<_> = result
        .applied
        .iter()
        .filter(|f| f.proposal.rule.as_str() == "C001")
        .collect();
    assert!(
        c001_fixes.is_empty(),
        "no-op correction (replacement == original) must not produce an applied fix"
    );
}

// -----------------------------------------------------------------------
// LOW-2: multi-token marking where only one token matches corrections
// -----------------------------------------------------------------------

#[test]
fn c001_fires_only_on_matching_token_in_multi_token_marking() {
    // SECRET//NF//NOFORN — corrections map has NF→NOFORN. C001 should
    // fire on the "NF" token but NOT on "NOFORN" (not in the map) or
    // "SECRET" (not in the map).
    let mut corrections = HashMap::new();
    corrections.insert("NF".to_owned(), "NOFORN".to_owned());
    let engine = engine_with_corrections(corrections);

    let source = b"SECRET//NF//NOFORN\n";
    let result = engine.lint(source);

    let c001_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "C001")
        .collect();

    assert_eq!(
        c001_diags.len(),
        1,
        "C001 should fire exactly once (on NF), not on SECRET or NOFORN: {c001_diags:?}"
    );
    let fix = c001_diags[0].fix.as_ref().unwrap();
    assert_eq!(fix.original.as_ref(), "NF");
    assert_eq!(fix.replacement.as_ref(), "NOFORN");
}

// -----------------------------------------------------------------------
// F-13: exact spec input scenario
// -----------------------------------------------------------------------

#[test]
fn us3_acceptance_scenario_combined_corrections_and_builtin_fix() {
    // US3 acceptance scenario 2 (adapted): corrections map for NF→NOFORN,
    // input "SECRET//NF\n" → output "SECRET//NOFORN\n". C001 handles NF
    // and E001 would also fire but C001 wins via FR-009/FR-016.
    let mut corrections = HashMap::new();
    corrections.insert("NF".to_owned(), "NOFORN".to_owned());
    let engine = engine_with_corrections(corrections);

    let source = b"SECRET//NF\n";
    let result = engine.fix(source, FixMode::Apply);

    let fixed_text = String::from_utf8(result.source).unwrap();
    assert_eq!(
        fixed_text, "SECRET//NOFORN\n",
        "combined fix should produce SECRET//NOFORN"
    );

    // Verify C001 is the rule that won for the NF→NOFORN span
    let nf_fix = result
        .applied
        .iter()
        .find(|f| f.proposal.replacement.as_ref() == "NOFORN")
        .expect("should have a NOFORN fix");
    assert_eq!(nf_fix.proposal.rule.as_str(), "C001");
    assert_eq!(nf_fix.proposal.source, FixSource::CorrectionsMap);
    assert_eq!(nf_fix.proposal.migration_ref, None);
}

// -----------------------------------------------------------------------
// Pre-scanner text corrections (markings the scanner misses)
// -----------------------------------------------------------------------

#[test]
fn pre_scanner_corrections_fires_on_unrecognized_classification_prefix() {
    // "SERCET" is not a known classification prefix, so the scanner does
    // not detect "SERCET//NF" as a banner candidate. The pre-scanner text
    // corrections pass should still find "SERCET" and emit a C001 diagnostic.
    let mut corrections = HashMap::new();
    corrections.insert("SERCET".to_owned(), "SECRET".to_owned());
    let engine = engine_with_corrections(corrections);

    let source = b"SERCET//NF\n";
    let result = engine.lint(source);

    let c001_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "C001")
        .collect();

    assert!(
        !c001_diags.is_empty(),
        "pre-scanner corrections must fire on SERCET even though the scanner \
         doesn't detect it as a banner"
    );
    let fix = c001_diags[0].fix.as_ref().expect("C001 should have a fix");
    assert_eq!(fix.source, FixSource::CorrectionsMap);
    assert_eq!(fix.original.as_ref(), "SERCET");
    assert_eq!(fix.replacement.as_ref(), "SECRET");
}

#[test]
fn pre_scanner_corrections_fix_produces_correct_output() {
    // Full spec acceptance scenario: SERCET//NF with corrections SERCET→SECRET.
    // After fix: the pre-scanner pass replaces SERCET→SECRET, then the
    // scanner detects SECRET//NF, E001 fires on NF→NOFORN, and the final
    // output is SECRET//NOFORN.
    let mut corrections = HashMap::new();
    corrections.insert("SERCET".to_owned(), "SECRET".to_owned());
    let engine = engine_with_corrections(corrections);

    let source = b"SERCET//NF\n";
    let result = engine.fix(source, FixMode::Apply);

    let fixed_text = String::from_utf8(result.source).unwrap();
    assert_eq!(
        fixed_text, "SECRET//NOFORN\n",
        "SERCET//NF should become SECRET//NOFORN after corrections + E001 fix"
    );

    // The C001 fix for SERCET→SECRET should be in the audit trail.
    let c001_fix = result
        .applied
        .iter()
        .find(|f| f.proposal.rule.as_str() == "C001");
    assert!(
        c001_fix.is_some(),
        "audit trail must contain a C001 fix for SERCET→SECRET"
    );
    assert_eq!(c001_fix.unwrap().proposal.source, FixSource::CorrectionsMap);
}

#[test]
fn pre_scanner_corrections_does_not_duplicate_rule_pipeline_c001() {
    // When the rule pipeline already produces a C001 diagnostic for a span
    // (because the scanner DID detect the marking), the pre-scanner pass
    // must not emit a duplicate.
    let mut corrections = HashMap::new();
    corrections.insert("NF".to_owned(), "NOFORN".to_owned());
    let engine = engine_with_corrections(corrections);

    // SECRET//NF — the scanner detects this as a banner. The rule pipeline's
    // C001 matches "NF" via token_spans. The pre-scanner text scan also
    // finds "NF" in the raw text. Only ONE C001 diagnostic should exist.
    let source = b"SECRET//NF\n";
    let result = engine.lint(source);

    let c001_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "C001")
        .collect();

    // There should be exactly one C001, not two.
    assert_eq!(
        c001_diags.len(),
        1,
        "pre-scanner must not duplicate rule-pipeline C001 diagnostics: {c001_diags:?}"
    );
}
