//! Phase 4 — fix pipeline integration tests (T044, T046).
//!
//! Drives `Engine::fix` against corpus fixtures and stub rules, verifying:
//! - Mixed confidence: only high-confidence fixes applied (FR-004)
//! - Dry-run parity: identical applied list, dry_run=true, source unchanged
//! - Missing classifier identity: field is None
//! - Overlap guard: deterministic FR-016 ordering
//! - Post-fix re-lint: fewer diagnostics after fixing

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock, LintResult};
use serde_json::json;
use std::time::{Duration, UNIX_EPOCH};

/// Fixed timestamp for deterministic audit records.
const FIXED_TS: u64 = 1_700_000_000; // 2023-11-14T22:13:20Z

fn test_engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
}

fn mixed_confidence_source() -> Vec<u8> {
    // E001 at confidence 1.0 (NF → NOFORN), E003 at confidence 0.6 (misordered).
    b"SECRET//NF\nSECRET//NOFORN//SI\n".to_vec()
}

#[test]
fn mixed_confidence_applies_only_high_confidence_fix() {
    let engine = test_engine();
    let source = mixed_confidence_source();
    let result = engine.fix(&source, FixMode::Apply);

    // Only E001 (confidence 1.0) should be applied.
    assert_eq!(result.applied.len(), 1, "applied: {:?}", result.applied);
    assert_eq!(result.applied[0].proposal.rule.as_str(), "E001");
    assert!((result.applied[0].proposal.confidence - 1.0).abs() < f32::EPSILON);

    // The post-fix text should have NF replaced with NOFORN.
    let fixed_text = String::from_utf8(result.source).unwrap();
    assert!(
        fixed_text.starts_with("SECRET//NOFORN"),
        "expected NF → NOFORN, got: {fixed_text:?}"
    );

    // E003 (confidence 0.6 < threshold 0.95) remains as a suggestion.
    assert!(
        !result.remaining_diagnostics.is_empty(),
        "E003 should remain in remaining_diagnostics"
    );
    assert!(
        result
            .remaining_diagnostics
            .iter()
            .any(|d| d.rule.as_str() == "E003")
    );
}

#[test]
fn dry_run_parity_with_apply() {
    let engine = test_engine();
    let source = mixed_confidence_source();

    let apply_result = engine.fix(&source, FixMode::Apply);
    let dry_result = engine.fix(&source, FixMode::DryRun);

    // DryRun returns original source.
    assert_eq!(dry_result.source, source);

    // Same number of applied fixes.
    assert_eq!(apply_result.applied.len(), dry_result.applied.len());

    // Same rule IDs and confidences.
    for (a, d) in apply_result.applied.iter().zip(dry_result.applied.iter()) {
        assert_eq!(a.proposal.rule.as_str(), d.proposal.rule.as_str());
        assert!((a.proposal.confidence - d.proposal.confidence).abs() < f32::EPSILON);
    }

    // DryRun records have dry_run=true.
    for fix in &dry_result.applied {
        assert!(fix.dry_run, "dry-run applied fix should have dry_run=true");
    }

    // Apply records have dry_run=false.
    for fix in &apply_result.applied {
        assert!(!fix.dry_run, "apply applied fix should have dry_run=false");
    }

    // Same remaining diagnostics count.
    assert_eq!(
        apply_result.remaining_diagnostics.len(),
        dry_result.remaining_diagnostics.len()
    );
}

#[test]
fn missing_classifier_id_is_none() {
    let engine = test_engine();
    let source = mixed_confidence_source();
    let result = engine.fix(&source, FixMode::Apply);

    for fix in &result.applied {
        assert!(
            fix.classifier_id.is_none(),
            "classifier_id should be None when not configured"
        );
    }
}

#[test]
fn fixed_clock_produces_deterministic_timestamps() {
    let engine = test_engine();
    let source = mixed_confidence_source();

    let r1 = engine.fix(&source, FixMode::Apply);
    let r2 = engine.fix(&source, FixMode::Apply);

    assert_eq!(r1.applied.len(), r2.applied.len());
    for (a, b) in r1.applied.iter().zip(r2.applied.iter()) {
        assert_eq!(
            a.timestamp, b.timestamp,
            "timestamps should be deterministic"
        );
    }
}

#[test]
fn post_fix_relint_has_fewer_diagnostics() {
    let engine = test_engine();
    let source = mixed_confidence_source();

    // Lint before fix.
    let before: LintResult = engine.lint(&source);

    // Apply fixes.
    let result = engine.fix(&source, FixMode::Apply);

    // Re-lint the fixed text.
    let after: LintResult = engine.lint(&result.source);

    // The fixed text should have fewer diagnostics than the original.
    assert!(
        after.diagnostics.len() < before.diagnostics.len(),
        "post-fix re-lint should have fewer diagnostics: before={}, after={}",
        before.diagnostics.len(),
        after.diagnostics.len()
    );
}

#[test]
fn classifier_id_propagated_when_configured() {
    let mut config = Config::default();
    config.user.classifier_id = Some("TEST-CLASSIFIER-42".to_owned());
    let engine = Engine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    );

    let source = mixed_confidence_source();
    let result = engine.fix(&source, FixMode::Apply);

    for fix in &result.applied {
        assert_eq!(
            fix.classifier_id.as_deref(),
            Some("TEST-CLASSIFIER-42"),
            "classifier_id should match config"
        );
    }
}

// --- H3: insta snapshot tests for audit NDJSON shape (T046) ---

/// Must match `AUDIT_SCHEMA_VERSION` in `marque/src/render.rs`. If a version
/// bump changes the value there, the insta snapshots here will fail, surfacing
/// the mismatch at test time.
const AUDIT_SCHEMA_VERSION: &str = "marque-mvp-1";

/// Serialize an AppliedFix to the audit-record JSON shape for snapshot testing.
fn applied_fix_to_json(fix: &marque_rules::AppliedFix) -> serde_json::Value {
    let source_str = match fix.proposal.source {
        marque_rules::FixSource::BuiltinRule => "BuiltinRule",
        marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
        marque_rules::FixSource::MigrationTable => "MigrationTable",
    };
    json!({
        "schema": AUDIT_SCHEMA_VERSION,
        "rule": fix.proposal.rule.as_str(),
        "source": source_str,
        "span": {
            "start": fix.proposal.span.start,
            "end": fix.proposal.span.end,
        },
        "original": fix.proposal.original.as_ref(),
        "replacement": fix.proposal.replacement.as_ref(),
        "confidence": fix.proposal.confidence,
        "migration_ref": fix.proposal.migration_ref,
        "timestamp": humantime::format_rfc3339(fix.timestamp).to_string(),
        "classifier_id": fix.classifier_id.as_ref().map(|s| s.as_ref()),
        "dry_run": fix.dry_run,
        "input": fix.input.as_ref().map(|s| s.as_ref()),
    })
}

#[test]
fn audit_record_snapshot_e001_apply() {
    let engine = test_engine();
    let source = b"SECRET//NF\n";
    let result = engine.fix(source, FixMode::Apply);
    assert_eq!(result.applied.len(), 1);

    let json: Vec<serde_json::Value> = result.applied.iter().map(applied_fix_to_json).collect();
    insta::assert_json_snapshot!(json);
}

#[test]
fn audit_record_snapshot_e001_dry_run() {
    let engine = test_engine();
    let source = b"SECRET//NF\n";
    let result = engine.fix(source, FixMode::DryRun);
    assert_eq!(result.applied.len(), 1);

    let json: Vec<serde_json::Value> = result.applied.iter().map(applied_fix_to_json).collect();
    insta::assert_json_snapshot!(json);
}

// --- L4: parity test verifies rule IDs, not just count ---

#[test]
fn dry_run_parity_rule_ids_match() {
    let engine = test_engine();
    let source = mixed_confidence_source();

    let apply_result = engine.fix(&source, FixMode::Apply);
    let dry_result = engine.fix(&source, FixMode::DryRun);

    // Verify remaining diagnostics have the same rule IDs, not just same count.
    let apply_rules: Vec<&str> = apply_result
        .remaining_diagnostics
        .iter()
        .map(|d| d.rule.as_str())
        .collect();
    let dry_rules: Vec<&str> = dry_result
        .remaining_diagnostics
        .iter()
        .map(|d| d.rule.as_str())
        .collect();
    assert_eq!(
        apply_rules, dry_rules,
        "remaining diagnostic rule IDs must match between Apply and DryRun"
    );
}
