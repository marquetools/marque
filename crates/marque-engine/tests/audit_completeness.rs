//! Phase 4 — audit completeness tests (T045).
//!
//! Enforces SC-004: every AppliedFix has a complete payload (no missing fields,
//! no orphaned changes). Sub-threshold FixProposals never appear in the audit
//! stream.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

fn test_engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
}

#[test]
fn applied_fix_has_all_required_fields() {
    let engine = test_engine();
    // This source triggers E001 at confidence 1.0.
    let source = b"SECRET//NF\n";
    let result = engine.fix(source, FixMode::Apply);

    assert!(
        !result.applied.is_empty(),
        "should have at least one applied fix"
    );

    for fix in &result.applied {
        // rule: non-empty string matching E/W/C + 3 digits
        let rule = fix.proposal.rule.as_str();
        assert!(!rule.is_empty(), "rule ID must not be empty");
        assert!(
            rule.len() == 4
                && (rule.starts_with('E') || rule.starts_with('W') || rule.starts_with('C')),
            "rule ID must match [EWC]NNN pattern, got: {rule}"
        );

        // source: valid FixSource variant (always passes by type system, but
        // verify the string form is one of the contract values)
        let source_str = match fix.proposal.source {
            marque_rules::FixSource::BuiltinRule => "BuiltinRule",
            marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
            marque_rules::FixSource::MigrationTable => "MigrationTable",
        };
        assert!(
            ["BuiltinRule", "CorrectionsMap", "MigrationTable"].contains(&source_str),
            "source must be a valid enum variant"
        );

        // span: non-empty
        assert!(
            fix.proposal.span.start < fix.proposal.span.end,
            "span must be non-empty: {:?}",
            fix.proposal.span
        );

        // replacement: non-empty for actual fixes
        assert!(
            !fix.proposal.replacement.is_empty(),
            "replacement must not be empty"
        );

        // confidence: in [0.0, 1.0]
        assert!(
            (0.0..=1.0).contains(&fix.proposal.confidence),
            "confidence must be in [0.0, 1.0], got: {}",
            fix.proposal.confidence
        );

        // timestamp: must be after UNIX epoch
        assert!(
            fix.timestamp >= UNIX_EPOCH,
            "timestamp must be after UNIX epoch"
        );

        // dry_run: boolean (always valid by type)
        // classifier_id: Option<Arc<str>> (valid by type)
    }
}

#[test]
fn sub_threshold_proposals_never_in_applied() {
    let engine = test_engine();
    // This source triggers E003 at confidence 0.6 (below default 0.95 threshold).
    // E001 fires at 1.0 but on the second line only E003 fires.
    let source = b"SECRET//NOFORN//SI\n";
    let result = engine.fix(source, FixMode::Apply);

    // E003 is sub-threshold — it must NOT appear in applied.
    for fix in &result.applied {
        assert!(
            fix.proposal.confidence >= 0.95,
            "sub-threshold fix (confidence {}) must not appear in applied",
            fix.proposal.confidence
        );
    }

    // E003 should remain in remaining_diagnostics.
    assert!(
        result
            .remaining_diagnostics
            .iter()
            .any(|d| d.rule.as_str() == "E003"),
        "E003 should remain as a suggestion in remaining_diagnostics"
    );
}

#[test]
fn dry_run_applied_fixes_have_dry_run_flag() {
    let engine = test_engine();
    let source = b"SECRET//NF\n";
    let result = engine.fix(source, FixMode::DryRun);

    for fix in &result.applied {
        assert!(
            fix.dry_run,
            "all DryRun applied fixes must have dry_run=true"
        );
    }
}

#[test]
fn applied_fix_timestamp_matches_clock() {
    let expected_ts = UNIX_EPOCH + Duration::from_secs(FIXED_TS);
    let engine = test_engine();
    let source = b"SECRET//NF\n";
    let result = engine.fix(source, FixMode::Apply);

    for fix in &result.applied {
        assert_eq!(
            fix.timestamp, expected_ts,
            "timestamp should match the injected FixedClock"
        );
    }
}
