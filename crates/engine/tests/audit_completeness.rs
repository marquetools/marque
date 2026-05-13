// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

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
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn applied_fix_has_all_required_fields() {
    let engine = test_engine();
    // This source triggers E002 at confidence 0.97. (PR 3c.B Commit
    // 6 retired E001 — the original "high confidence single fix"
    // anchor — into the renderer.)
    let source = b"SECRET//REL TO GBR\n";
    let result = engine.fix(source, FixMode::Apply);

    assert!(
        !result.applied.is_empty(),
        "should have at least one applied fix"
    );

    for fix in &result.applied {
        // rule: non-empty string matching E/W/C + 3 digits
        let rule = fix.rule.as_str();
        assert!(!rule.is_empty(), "rule ID must not be empty");
        assert!(
            rule.len() == 4
                && (rule.starts_with('E') || rule.starts_with('W') || rule.starts_with('C')),
            "rule ID must match [EWC]NNN pattern, got: {rule}"
        );

        // source: valid FixSource variant (always passes by type system, but
        // verify the string form is one of the contract values). Read from
        // the top-level snapshot per the v2 audit contract (the audit record
        // is what auditors see; promotion-time adjustments must be visible).
        let source_str = match fix.source {
            marque_rules::FixSource::BuiltinRule => "BuiltinRule",
            marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
            marque_rules::FixSource::MigrationTable => "MigrationTable",
            marque_rules::FixSource::DecoderPosterior => "DecoderPosterior",
            marque_rules::FixSource::DecoderClassificationHeuristic => {
                "DecoderClassificationHeuristic"
            }
        };
        assert!(
            [
                "BuiltinRule",
                "CorrectionsMap",
                "MigrationTable",
                "DecoderPosterior",
                "DecoderClassificationHeuristic",
            ]
            .contains(&source_str),
            "source must be a valid enum variant"
        );

        // span: non-empty
        assert!(
            fix.span.start < fix.span.end,
            "span must be non-empty: {:?}",
            fix.span
        );

        // proposal shape: every applied fix must carry either a
        // structural FixIntent or a TextCorrection payload (post
        // Commit 10 the audit envelope is one of these two shapes).
        match &fix.proposal {
            marque_rules::AppliedFixProposal::FixIntent(_) => {}
            marque_rules::AppliedFixProposal::TextCorrection { replacement } => {
                assert!(
                    !replacement.is_empty(),
                    "text-correction replacement must not be empty"
                );
            }
        }

        // confidence: in [0.0, 1.0]. Read from the top-level snapshot per
        // the v2 audit contract — the audit record's confidence is what
        // auditors see; future promotion-time adjustments (e.g.,
        // region-context calibration) must remain bounded.
        let combined = fix.confidence.combined();
        assert!(
            (0.0..=1.0).contains(&combined),
            "confidence must be in [0.0, 1.0], got: {combined}"
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
    // Override the default 0.95 threshold to 0.99 so E002's 0.97 fix
    // is **below** threshold — exercising the sub-threshold gate
    // without depending on any rule whose default fix is <0.95.
    // (Pre-PR-3c.B-Commit-6 this test relied on E003 at 0.6 being
    // sub-threshold against the default 0.95; E003 retired into the
    // renderer.)
    let mut config = Config::default();
    config
        .set_confidence_threshold(0.99)
        .expect("0.99 is in [0.0, 1.0]");
    let engine = Engine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let source = b"SECRET//REL TO GBR\n";
    let result = engine.fix(source, FixMode::Apply);

    // No fix should be applied at threshold 0.99 — E002's 0.97 is
    // sub-threshold.
    assert!(
        result.applied.is_empty(),
        "no sub-threshold fix may appear in applied; got: {:?}",
        result.applied
    );

    // Every entry in `applied` (vacuously none here) would have
    // ≥0.99 confidence. The audit-contract assertion: the gate is
    // honored.
    for fix in &result.applied {
        let combined = fix.confidence.combined();
        assert!(
            combined >= 0.99,
            "sub-threshold fix (confidence {combined}) must not appear in applied"
        );
    }

    // E002 should remain in remaining_diagnostics.
    assert!(
        result
            .remaining_diagnostics
            .iter()
            .any(|d| d.rule.as_str() == "E002"),
        "E002 should remain as a suggestion in remaining_diagnostics"
    );
}

#[test]
fn dry_run_applied_fixes_have_dry_run_flag() {
    let engine = test_engine();
    let source = b"SECRET//REL TO GBR\n";
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
    let source = b"SECRET//REL TO GBR\n";
    let result = engine.fix(source, FixMode::Apply);

    for fix in &result.applied {
        assert_eq!(
            fix.timestamp, expected_ts,
            "timestamp should match the injected FixedClock"
        );
    }
}
