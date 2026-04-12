//! T069 — Corpus accuracy harness (SC-002 / SC-003 / SC-003a).
//!
//! Validates lint and fix behavior across the full test corpus with per-rule
//! accuracy thresholds:
//!
//! - **SC-002**: >=95% lint accuracy per-rule and overall against `.expected.json`
//! - **SC-003**: >=95% fix accuracy per-rule and overall (zero remaining violations)
//! - **SC-003a**: Zero diagnostics on clean prose (precision gate)

use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_test_utils::{
    invalid_fixtures, load_expected, load_fixture, prose_fixtures, valid_fixtures,
};
use std::collections::HashMap;

fn make_engine() -> Engine {
    Engine::new(Config::default(), marque_engine::default_ruleset())
}

// ---------------------------------------------------------------------------
// SC-002: Lint accuracy on invalid fixtures (>=95% per-rule and overall)
// ---------------------------------------------------------------------------

#[test]
fn lint_accuracy_invalid_fixtures() {
    let engine = make_engine();
    let fixtures = invalid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no invalid fixtures found in corpus — cannot validate SC-002"
    );

    // Per-rule tracking: rule_id -> (matched, expected_total)
    let mut per_rule: HashMap<String, (usize, usize)> = HashMap::new();
    let mut total_expected = 0usize;
    let mut total_matched = 0usize;

    for path in &fixtures {
        let source = load_fixture(path);
        let expected = load_expected(path);
        let result = engine.lint(&source);

        for exp in &expected.diagnostics {
            total_expected += 1;
            let entry = per_rule.entry(exp.rule.clone()).or_insert((0, 0));
            entry.1 += 1;

            // Match: same rule ID AND same span
            let matched = result.diagnostics.iter().any(|d| {
                d.rule.as_str() == exp.rule
                    && d.span.start == exp.span.start
                    && d.span.end == exp.span.end
            });

            if matched {
                total_matched += 1;
                entry.0 += 1;
            }
        }
    }

    // Report per-rule accuracy
    let mut failures = Vec::new();
    for (rule, (matched, total)) in &per_rule {
        let accuracy = if *total == 0 {
            1.0
        } else {
            *matched as f64 / *total as f64
        };
        if accuracy < 0.95 {
            failures.push(format!(
                "  {rule}: {matched}/{total} = {:.1}% (need >=95%)",
                accuracy * 100.0
            ));
        }
    }

    let overall = if total_expected == 0 {
        1.0
    } else {
        total_matched as f64 / total_expected as f64
    };

    assert!(
        failures.is_empty() && overall >= 0.95,
        "SC-002 lint accuracy FAILED\n\
         Overall: {total_matched}/{total_expected} = {:.1}%\n\
         Per-rule failures:\n{}",
        overall * 100.0,
        if failures.is_empty() {
            "  (none — overall below threshold)".to_string()
        } else {
            failures.join("\n")
        }
    );
}

// ---------------------------------------------------------------------------
// SC-003: Fix accuracy on invalid fixtures (>=95% per-rule zero-remaining)
// ---------------------------------------------------------------------------

#[test]
fn fix_accuracy_invalid_fixtures() {
    let engine = make_engine();
    let fixtures = invalid_fixtures();
    let threshold = Config::default().confidence_threshold();
    assert!(
        !fixtures.is_empty(),
        "no invalid fixtures found in corpus — cannot validate SC-003"
    );

    // Per-rule tracking: rule_id -> (fixed_clean, total_fixtures_with_fixable_rule)
    // Only count rules that produce at least one fix proposal with confidence >= threshold.
    // Rules like E005 (no fix), E008 (FR-012: no fix), and E003 (confidence 0.6 < 0.95)
    // intentionally don't auto-fix and should not count against fix accuracy.
    let mut per_rule: HashMap<String, (usize, usize)> = HashMap::new();
    let mut total_fixable = 0usize;
    let mut total_fixed_clean = 0usize;

    for path in &fixtures {
        let source = load_fixture(path);
        let expected = load_expected(path);
        if expected.diagnostics.is_empty() {
            continue;
        }

        // Lint first to discover which rules are fixable (have above-threshold fixes)
        let lint_result = engine.lint(&source);
        let fixable_rules: std::collections::HashSet<String> = lint_result
            .diagnostics
            .iter()
            .filter(|d| d.fix.as_ref().is_some_and(|f| f.confidence >= threshold))
            .map(|d| d.rule.as_str().to_owned())
            .collect();

        if fixable_rules.is_empty() {
            continue; // No auto-fixable diagnostics in this fixture
        }

        total_fixable += 1;

        // Fix the source
        let fix_result = engine.fix(&source, FixMode::Apply);

        // Re-lint the fixed output
        let relint = engine.lint(&fix_result.source);

        // Check which fixable rules still have violations
        let remaining_rules: std::collections::HashSet<&str> =
            relint.diagnostics.iter().map(|d| d.rule.as_str()).collect();

        // A fixture counts as "fixed clean" if no fixable rules remain
        let all_fixable_resolved = fixable_rules
            .iter()
            .all(|r| !remaining_rules.contains(r.as_str()));

        if all_fixable_resolved {
            total_fixed_clean += 1;
        }

        for rule in &fixable_rules {
            let entry = per_rule.entry(rule.clone()).or_insert((0, 0));
            entry.1 += 1;
            if !remaining_rules.contains(rule.as_str()) {
                entry.0 += 1;
            }
        }
    }

    // Report per-rule fix accuracy
    let mut failures = Vec::new();
    for (rule, (fixed, total)) in &per_rule {
        let accuracy = if *total == 0 {
            1.0
        } else {
            *fixed as f64 / *total as f64
        };
        if accuracy < 0.95 {
            failures.push(format!(
                "  {rule}: {fixed}/{total} = {:.1}% (need >=95%)",
                accuracy * 100.0
            ));
        }
    }

    let overall = if total_fixable == 0 {
        1.0
    } else {
        total_fixed_clean as f64 / total_fixable as f64
    };

    assert!(
        failures.is_empty() && overall >= 0.95,
        "SC-003 fix accuracy FAILED\n\
         Overall: {total_fixed_clean}/{total_fixable} = {:.1}%\n\
         Per-rule failures:\n{}",
        overall * 100.0,
        if failures.is_empty() {
            "  (none — overall below threshold)".to_string()
        } else {
            failures.join("\n")
        }
    );
}

// ---------------------------------------------------------------------------
// SC-003a: Zero diagnostics on clean prose (precision gate)
// ---------------------------------------------------------------------------

#[test]
fn precision_prose_zero_diagnostics() {
    let engine = make_engine();
    let fixtures = prose_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no prose fixtures found in corpus — cannot validate SC-003a"
    );

    for path in &fixtures {
        let source = load_fixture(path);
        let result = engine.lint(&source);

        assert!(
            result.diagnostics.is_empty(),
            "SC-003a precision failure on {}: expected zero diagnostics, got {}:\n{}",
            path.file_name().unwrap().to_string_lossy(),
            result.diagnostics.len(),
            result
                .diagnostics
                .iter()
                .map(|d| format!(
                    "  {} at {}..{}: {}",
                    d.rule.as_str(),
                    d.span.start,
                    d.span.end,
                    d.message
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

// ---------------------------------------------------------------------------
// Bonus: Valid fixtures should produce zero diagnostics
// ---------------------------------------------------------------------------

#[test]
fn valid_fixtures_zero_diagnostics() {
    let engine = make_engine();
    let fixtures = valid_fixtures();
    assert!(!fixtures.is_empty(), "no valid fixtures found in corpus");

    for path in &fixtures {
        let source = load_fixture(path);
        let result = engine.lint(&source);

        assert!(
            result.diagnostics.is_empty(),
            "valid fixture {} produced {} unexpected diagnostics:\n{}",
            path.file_name().unwrap().to_string_lossy(),
            result.diagnostics.len(),
            result
                .diagnostics
                .iter()
                .map(|d| format!(
                    "  {} at {}..{}: {}",
                    d.rule.as_str(),
                    d.span.start,
                    d.span.end,
                    d.message
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
