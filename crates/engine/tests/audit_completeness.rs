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
use marque_rules::audit::AuditLine;
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
        result.applied_fixes().next().is_some(),
        "should have at least one applied fix"
    );

    // Post-PR-3c.2.D the engine's audit stream is `Vec<AuditLine>`.
    // Each line is either a marking fix (`AppliedFix`) or a text-
    // correction (`AppliedTextCorrection`); both carry the audit-
    // record contract fields. Walk both arms.
    for line in &result.audit_lines {
        let (rule, span, source, timestamp) = match line {
            AuditLine::AppliedFix(f) => (&f.rule, f.span, f.source, f.timestamp),
            AuditLine::TextCorrection(tc) => (&tc.rule, tc.span, tc.source, tc.timestamp),
            _ => continue,
        };

        // T044: rule is the 2-tuple `(scheme, predicate_id)`. The
        // engine emits only known schemes (`"capco"` for CAPCO rules,
        // `"engine"` for R001/R002 sentinels) — there is no `"test"`
        // emission in a real engine audit stream. Predicate ids are
        // non-empty by the `RuleId::new` contract.
        let scheme = rule.scheme();
        let predicate = rule.predicate_id();
        assert!(!scheme.is_empty(), "rule scheme must not be empty");
        assert!(!predicate.is_empty(), "rule predicate_id must not be empty");
        assert!(
            scheme == "capco" || scheme == "engine",
            "engine emits only `capco` (registered rules + bridge) and `engine` (R001/R002 sentinels); got scheme {scheme} (predicate {predicate})"
        );

        // source: valid FixSource variant (always passes by type system, but
        // verify the string form is one of the contract values).
        let source_str = match source {
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
        assert!(span.start < span.end, "span must be non-empty: {span:?}");

        // text-correction-arm-specific: replacement must not be empty.
        // The marking-side arm carries a `Canonical<S>` payload sealed
        // by `EngineConstructor`; the constructor guarantees non-empty
        // bytes for every promotion.
        if let AuditLine::TextCorrection(tc) = line {
            assert!(
                !tc.replacement.is_empty(),
                "text-correction replacement must not be empty"
            );
            let combined = tc.confidence.combined();
            assert!(
                (0.0..=1.0).contains(&combined),
                "confidence must be in [0.0, 1.0], got: {combined}"
            );
        }
        if let AuditLine::AppliedFix(f) = line {
            // confidence lives at `fix.replacement.confidence` per
            // the marque-1.0 audit-record shape.
            let combined = f.fix.replacement.confidence.combined();
            assert!(
                (0.0..=1.0).contains(&combined),
                "confidence must be in [0.0, 1.0], got: {combined}"
            );
        }

        // timestamp: must be after UNIX epoch
        assert!(
            timestamp >= UNIX_EPOCH,
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
    // PR 3c.2.D fixup F-3: `applied_fixes()` is `impl Iterator` (not
    // `Debug`); collect once for the `is_empty` + Debug-render path.
    let applied: Vec<_> = result.applied_fixes().collect();
    assert!(
        applied.is_empty(),
        "no sub-threshold fix may appear in applied; got: {applied:?}",
    );

    // Every entry (vacuously none here) would have ≥0.99 confidence.
    // The audit-contract assertion: the gate is honored.
    for fix in result.applied_fixes() {
        let combined = fix.fix.replacement.confidence.combined();
        assert!(
            combined >= 0.99,
            "sub-threshold fix (confidence {combined}) must not appear in applied"
        );
    }

    // E002 (post-T044: `portion.dissem.rel-to-missing-usa`) should
    // remain in remaining_diagnostics.
    assert!(
        result
            .remaining_diagnostics
            .iter()
            .any(|d| d.rule.predicate_id() == "portion.dissem.rel-to-missing-usa"),
        "E002 should remain as a suggestion in remaining_diagnostics; got: {:?}",
        result
            .remaining_diagnostics
            .iter()
            .map(|d| d.rule.to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn dry_run_applied_fixes_have_dry_run_flag() {
    let engine = test_engine();
    let source = b"SECRET//REL TO GBR\n";
    let result = engine.fix(source, FixMode::DryRun);

    for line in &result.audit_lines {
        let dry_run = match line {
            AuditLine::AppliedFix(f) => f.dry_run,
            AuditLine::TextCorrection(tc) => tc.dry_run,
            _ => continue,
        };
        assert!(dry_run, "all DryRun applied fixes must have dry_run=true");
    }
}

#[test]
fn applied_fix_timestamp_matches_clock() {
    let expected_ts = UNIX_EPOCH + Duration::from_secs(FIXED_TS);
    let engine = test_engine();
    let source = b"SECRET//REL TO GBR\n";
    let result = engine.fix(source, FixMode::Apply);

    for line in &result.audit_lines {
        let timestamp = match line {
            AuditLine::AppliedFix(f) => f.timestamp,
            AuditLine::TextCorrection(tc) => tc.timestamp,
            _ => continue,
        };
        assert_eq!(
            timestamp, expected_ts,
            "timestamp should match the injected FixedClock"
        );
    }
}

// ---------------------------------------------------------------------------
// PR 7b — R002 audit-record integrity
// ---------------------------------------------------------------------------

#[test]
fn r002_does_not_mint_applied_fix() {
    // Constitution V Principle V (audit-record integrity) lock:
    // R002 is a synthetic diagnostic emitted when the post-pass-1
    // buffer cannot be re-parsed. It has no replacement, no intent,
    // no fix proposal — it is informational guidance about why
    // pass-2 did not run, not an action taken. Promoting it via
    // `__engine_promote` would inject a false-positive audit record
    // claiming a fix was applied when none was.
    //
    // The pin: `result.applied` MUST NOT contain ANY entry whose
    // `rule == R002_RULE_ID`. Holds regardless of whether R002
    // fired or not on the fixture below (today no production
    // Localized rule emits a FixIntent that could trigger R002, so
    // `r002_fired == false` here, but the absence-of-R002-fix
    // invariant must hold in either branch).
    //
    // This integration test is a canary; it becomes load-bearing
    // when a future `Phase::Localized` rule lands that can trigger
    // R002. Today the loop iterates over fixes that R002 cannot
    // appear in, so the per-fix assertion is vacuously satisfied —
    // the direct shape pin lives at the unit-test layer in
    // `engine.rs::tests::build_r002_diagnostic_returns_diagnostic_not_appliedfix`,
    // which exercises `build_r002_diagnostic` itself and verifies
    // the returned `Diagnostic` carries neither a `FixIntent` nor a
    // `TextCorrection` (the two channels a `Diagnostic` can become
    // an `AppliedFix` through).
    let engine = test_engine();
    let source = b"SECRET//REL TO GBR\n(TS//HCS)\n";
    let result = engine.fix(source, FixMode::Apply);
    for line in &result.audit_lines {
        let rule = match line {
            AuditLine::AppliedFix(f) => &f.rule,
            AuditLine::TextCorrection(tc) => &tc.rule,
            _ => continue,
        };
        // Compare against the typed constant rather than the string
        // literal so a future rename of `R002_RULE_ID` (e.g., to
        // adopt the engine-synthetic namespace
        // `("engine", "r002.reparse-failed")` referenced in
        // `MessageTemplate::ReparseFailed`'s doc) is caught here
        // instead of silently passing on stale identifier drift.
        assert_ne!(
            *rule,
            marque_engine::R002_RULE_ID,
            "R002 must never appear as an AppliedFix"
        );
    }
}
