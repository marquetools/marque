// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

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
    corpus_root, invalid_fixtures, load_expected, load_fixture, prose_fixtures, valid_fixtures,
};
use std::collections::HashMap;

/// Default-engine corpus-accuracy gate.
///
/// Issue #258 landed the per-token prose null-hypothesis priors that
/// the `StrictOrDecoderRecognizer` dispatcher needs to reject prose-
/// shaped portions like the Federalist-corpus `Notwithstanding (s)
/// the early prevalence` case. With those priors in place, the
/// dispatcher's decoder fallback no longer auto-fixes `(s)` mid-prose
/// to a SECRET portion — `token_prose_log_prior("S")` exceeds
/// `token_log_prior("S")` so the null hypothesis wins and the
/// decoder returns zero candidates (FR-015), suppressing the
/// diagnostic.
///
/// SC-002 / SC-003 / SC-003a / C001 now run against the user-facing
/// default engine (no recognizer override). Adding a
/// `with_recognizer(StrictRecognizer::new())` here re-pins the strict
/// path and re-introduces the gap this test is meant to defend
/// against; do NOT unpin to "Strict" without the same null-hypothesis
/// gate landing first (per Constitution §VIII source-fidelity, this
/// test's load-bearing role is the SC-003a precision gate against
/// `tests/corpus/prose/article.txt`, NOT a strict-vs-decoder
/// equivalence check).
fn make_engine() -> Engine {
    Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
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
        // C001 fixtures require a corrections config — tested separately
        // in c001_corrections_map_accuracy.
        let fname = path.file_name().unwrap().to_string_lossy();
        if fname.starts_with("corrections_map_typo") {
            continue;
        }

        let source = load_fixture(path);
        let expected = load_expected(path);
        let result = engine.lint(&source);

        for exp in &expected.diagnostics {
            // W034 (sci-custom-control-info) ships Severity::Off by default;
            // the engine correctly skips it in the rule loop, so it cannot
            // fire here. The rules_us1 harness exercises W034 directly by
            // bypassing severity gating. Skip it from the engine harness.
            if exp.rule == "W034" {
                continue;
            }
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
    //
    // NOTE: `total_fixable` / `total_fixed_clean` count at fixture level — a fixture
    // is "fixed clean" only if ALL its fixable rules were resolved. This is stricter
    // than per-rule aggregation: a multi-rule fixture where one rule fails pulls the
    // overall metric down even if 9/10 rules pass individually.
    let mut per_rule: HashMap<String, (usize, usize)> = HashMap::new();
    let mut total_fixable = 0usize;
    let mut total_fixed_clean = 0usize;

    for path in &fixtures {
        let source = load_fixture(path);
        let expected = load_expected(path);
        if expected.diagnostics.is_empty() {
            continue;
        }

        // Lint first to discover which rules are fixable — i.e., the
        // rule's fix clears the engine's combined-confidence
        // threshold gate.
        //
        // "Confidence" here is the scalar `Confidence::combined()`
        // (= recognition × rule) that the engine applies at the
        // promotion boundary (FR-016). `Confidence` carries additional
        // axes (`region`, `runner_up_ratio`, feature contributions)
        // for audit provenance, but this harness and every
        // threshold-gated consumer compare on `.combined()` only.
        let lint_result = engine.lint(&source);
        let fixable_rules: std::collections::HashSet<String> = lint_result
            .diagnostics
            .iter()
            .filter(|d| {
                d.fix
                    .as_ref()
                    .is_some_and(|f| f.confidence.combined() >= threshold)
            })
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
// C001: Corrections-map accuracy (requires non-default config)
// ---------------------------------------------------------------------------

/// C001 fixtures require a corrections map in config. The default harness uses
/// Config::default() (empty corrections), so C001 is tested separately.
///
/// W001 was retired in T035c-14 — CAPCO-2016 §F treats legacy markings as
/// unauthorized (error category, owned by E006/E008), not "deprecated but
/// still legal." No authoritative bucket existed for a warning-severity
/// vocabulary-deprecation rule, so the stub was removed.
#[test]
fn c001_corrections_map_accuracy() {
    let c001_fixtures: Vec<_> = marque_test_utils::fixtures_in("invalid")
        .into_iter()
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("corrections_map_typo")
        })
        .collect();

    assert!(
        c001_fixtures.len() >= 3,
        "need >=3 C001 corpus fixtures, found {}",
        c001_fixtures.len()
    );

    // Each C001 fixture's expected.json has a _note explaining the required
    // corrections. We build a superset corrections map covering all fixtures.
    let mut corrections = std::collections::HashMap::new();
    corrections.insert("SERCET".to_string(), "SECRET".to_string());
    corrections.insert("NOFORM".to_string(), "NOFORN".to_string());
    corrections.insert("GBER".to_string(), "GBR".to_string());

    let mut config = Config::default();
    config.corrections = corrections;
    let engine = Engine::new(
        config,
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let mut matched = 0;
    let mut total = 0;

    for path in &c001_fixtures {
        let source = load_fixture(path);
        let expected = load_expected(path);
        let result = engine.lint(&source);

        for exp in &expected.diagnostics {
            if exp.rule != "C001" {
                continue;
            }
            total += 1;
            let found = result.diagnostics.iter().any(|d| {
                d.rule.as_str() == "C001"
                    && d.span.start == exp.span.start
                    && d.span.end == exp.span.end
            });
            if found {
                matched += 1;
            }
        }
    }

    assert!(
        total > 0,
        "no C001 expected diagnostics found in C001 fixtures"
    );
    let accuracy = matched as f64 / total as f64;
    assert!(
        accuracy >= 0.95,
        "C001 accuracy: {matched}/{total} = {:.1}% (need >=95%)",
        accuracy * 100.0
    );
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

/// Per-rule diagnostic count expectation for a single document fixture
/// in `tests/corpus/documents/marked/`.
///
/// Each entry pins the exact number of diagnostics of a given rule that
/// the default engine emits on a corpus document, with a citation to the
/// open issue that tracks fixing the firing. When the issue closes, the
/// engine fix lands, and the count moves toward zero — the test fails
/// loudly until this allowlist is updated, surfacing both regressions
/// (new firings) and improvements (fewer firings than pinned).
///
/// `issue = 0` marks a *correct* firing (legitimate noise in the source
/// material, e.g., embedded cable headers, historical CIA codewords not
/// in the published ODNI CVE registry) — the `reason` field documents
/// why the firing is expected to persist.
///
/// Every entry MUST cite either an open GitHub issue or a documented
/// reason for permanent expected-firing. The test enforces that pinned
/// stems correspond to real fixtures (file-deletion guard) and that the
/// observed per-rule count matches the pin exactly.
#[derive(Debug, Clone, Copy)]
struct ExpectedRuleCount {
    rule: &'static str,
    count: usize,
    /// GitHub issue tracking the fix. `0` = no issue; reason documents
    /// why this is a correct, expected firing.
    issue: u32,
    reason: &'static str,
}

/// Per-doc per-rule diagnostic expectations. Sorted by stem.
///
/// **Unwind plan**: as each issue below closes, the corresponding
/// `ExpectedRuleCount` entries shrink toward zero. The shape of this
/// allowlist makes the unwind mechanical:
///
/// - **#461** (Phase::PageFinalization): retires E031, E035, E040 entries.
/// - **#439** (S004 suppress under REL TO): retires S004 entries.
/// - **#470** (W002 over-fires on canonical FGI): retires W002 entries.
/// - **#471** (E015 zero-width false-positive): retires E015 entries.
/// - **#472** (R001 prose parenthetical false-positives): retires R001 entries.
/// - **#407** (Vocabulary sentinel set: CNWDI/UCNI long-forms): retires
///   the E008 entries on `RD-CRITICAL NUCLEAR WEAPON DESIGN INFORMATION`
///   and `DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION`.
/// - **issue 0** entries (W034 NTN, E008 embedded cable) stay forever —
///   they document correct firings on legitimate noise.
const EXPECTED_DOCUMENT_DIAGNOSTICS: &[(&str, &[ExpectedRuleCount])] = &[
    (
        "CIA-RDP01M00147R000100350002-7",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 1,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP09T00207R001000100002-2",
        &[
            ExpectedRuleCount {
                rule: "E035",
                count: 4,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "W002",
                count: 5,
                issue: 470,
                reason: "W002 over-fires on canonical (S//FGI XXX//NF)",
            },
        ],
    ),
    (
        "CIA-RDP09T00207R001000100012-1",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 1,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP09T00207R001000100017-6",
        &[ExpectedRuleCount {
            rule: "E040",
            count: 1,
            issue: 461,
            reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP09T00207R001000100021-1",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 1,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP09T00207R001000100022-0",
        &[
            ExpectedRuleCount {
                rule: "E031",
                count: 1,
                issue: 461,
                reason: "sar-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "E035",
                count: 1,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
        ],
    ),
    (
        "CIA-RDP64B00346R000300190014-8",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 3,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP69B00369R000100130011-9",
        &[ExpectedRuleCount {
            rule: "E040",
            count: 1,
            issue: 461,
            reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP69B00369R000200200020-0",
        &[
            ExpectedRuleCount {
                rule: "E035",
                count: 1,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "E040",
                count: 3,
                issue: 461,
                reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
            },
        ],
    ),
    (
        "CIA-RDP69B00369R000200200028-2",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 3,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP73B00148A000200150009-6",
        &[
            ExpectedRuleCount {
                rule: "E035",
                count: 3,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "S004",
                count: 2,
                issue: 439,
                reason: "S004 trigraph-suggest under REL TO block",
            },
        ],
    ),
    (
        "CIA-RDP74B00415R000300070018-9",
        &[
            ExpectedRuleCount {
                rule: "E035",
                count: 3,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "W002",
                count: 3,
                issue: 470,
                reason: "W002 over-fires on canonical (S//FGI XXX//NF)",
            },
        ],
    ),
    (
        "CIA-RDP74B00415R000500120103-5",
        &[
            ExpectedRuleCount {
                rule: "E035",
                count: 3,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "S004",
                count: 4,
                issue: 439,
                reason: "S004 trigraph-suggest under REL TO block",
            },
            ExpectedRuleCount {
                rule: "W034",
                count: 3,
                issue: 0,
                reason: "correct firing: historical CIA codeword `NTN` not in ODNI CVE registry; W034 audit-visibility surface is intended",
            },
        ],
    ),
    (
        "CIA-RDP75-00149R000500050001-4",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 1,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP75-00149R000500420001-3",
        &[ExpectedRuleCount {
            rule: "S004",
            count: 1,
            issue: 439,
            reason: "S004 trigraph-suggest under REL TO block",
        }],
    ),
    (
        "CIA-RDP75-00149R000500450034-4",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 1,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP75-00149R000500450044-3",
        &[ExpectedRuleCount {
            rule: "E008",
            count: 2,
            issue: 407,
            reason: "RD-CNWDI long-form `RD-CRITICAL NUCLEAR WEAPON DESIGN INFORMATION` not in vocabulary",
        }],
    ),
    (
        "CIA-RDP75-00149R000500450066-9",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 1,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP79B00972A000100570011-7",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 4,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP80-00809A000500340084-9",
        &[
            ExpectedRuleCount {
                rule: "E040",
                count: 1,
                issue: 461,
                reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "S004",
                count: 9,
                issue: 439,
                reason: "S004 trigraph-suggest under REL TO block",
            },
        ],
    ),
    (
        "CIA-RDP80-00809A000500720009-0",
        &[ExpectedRuleCount {
            rule: "E040",
            count: 1,
            issue: 461,
            reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP80B01139A000400200013-4",
        &[ExpectedRuleCount {
            rule: "E040",
            count: 1,
            issue: 461,
            reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP80B01676R000200140013-3",
        &[
            ExpectedRuleCount {
                rule: "E035",
                count: 3,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "E040",
                count: 1,
                issue: 461,
                reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
            },
        ],
    ),
    (
        "CIA-RDP90B01370R000801120005-5",
        &[
            ExpectedRuleCount {
                rule: "E008",
                count: 1,
                issue: 0,
                reason: "correct firing: embedded cable header `00 RUEAIIB` trailing tokens are genuine non-marking noise (paired with EXPECTED_MISMATCHES banner-count drift in document_corpus.rs)",
            },
            ExpectedRuleCount {
                rule: "E035",
                count: 2,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "S004",
                count: 1,
                issue: 439,
                reason: "S004 trigraph-suggest under REL TO block",
            },
        ],
    ),
    (
        "CIA-RDP96-00289R000200030004-1",
        &[
            ExpectedRuleCount {
                rule: "E015",
                count: 2,
                issue: 471,
                reason: "E015 zero-width span / wrong-classification false-positive on US TOP SECRET",
            },
            ExpectedRuleCount {
                rule: "E031",
                count: 1,
                issue: 461,
                reason: "sar-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "E035",
                count: 1,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "R001",
                count: 2,
                issue: 472,
                reason: "decoder R001 over-fires on prose parentheticals (CMS)/(C)",
            },
        ],
    ),
    (
        "CIA-RDP96-00289R000200030006-9",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 1,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIA-RDP96-00289R000200030017-7",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 3,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "CIAPolicyOnGAOOversight",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 2,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "cia-reports_prex-318se-2",
        &[ExpectedRuleCount {
            rule: "E040",
            count: 1,
            issue: 461,
            reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "implications-of-gligorov-16559483",
        &[
            ExpectedRuleCount {
                rule: "E035",
                count: 1,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "E040",
                count: 1,
                issue: 461,
                reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
            },
        ],
    ),
    (
        "initial-evidence-indicate-16559481",
        &[
            ExpectedRuleCount {
                rule: "E035",
                count: 1,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "S004",
                count: 13,
                issue: 439,
                reason: "S004 trigraph-suggest under REL TO block",
            },
        ],
    ),
    (
        "keyplayersinruss00wash",
        &[
            ExpectedRuleCount {
                rule: "E008",
                count: 2,
                issue: 407,
                reason: "DOE UCNI long-form `DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION` not in vocabulary",
            },
            ExpectedRuleCount {
                rule: "E040",
                count: 3,
                issue: 461,
                reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
            },
        ],
    ),
    (
        "keyplayersofsout00wash",
        &[ExpectedRuleCount {
            rule: "E035",
            count: 1,
            issue: 461,
            reason: "sci-banner-rollup gap (Phase::PageFinalization)",
        }],
    ),
    (
        "kiro-gligorov-macedonia-16555480",
        &[
            ExpectedRuleCount {
                rule: "E035",
                count: 1,
                issue: 461,
                reason: "sci-banner-rollup gap (Phase::PageFinalization)",
            },
            ExpectedRuleCount {
                rule: "E040",
                count: 4,
                issue: 461,
                reason: "nodis-exdis-banner-rollup gap (Phase::PageFinalization)",
            },
        ],
    ),
    (
        "topofficialsinru00wash",
        &[
            ExpectedRuleCount {
                rule: "S004",
                count: 7,
                issue: 439,
                reason: "S004 trigraph-suggest under REL TO block",
            },
            ExpectedRuleCount {
                rule: "W002",
                count: 3,
                issue: 470,
                reason: "W002 over-fires on canonical (S//FGI XXX//NF)",
            },
        ],
    ),
];

fn lookup_expected_diagnostics(stem: &str) -> &'static [ExpectedRuleCount] {
    EXPECTED_DOCUMENT_DIAGNOSTICS
        .iter()
        .find_map(|(s, e)| (*s == stem).then_some(*e))
        .unwrap_or(&[])
}

/// Strict per-doc per-rule diagnostic count check against the
/// `EXPECTED_DOCUMENT_DIAGNOSTICS` allowlist.
///
/// The previous incarnation of this test (`document_fixtures_lint_against_expected`)
/// iterated over `.expected.json` `diagnostics` arrays — every one of
/// which was empty — and asserted nothing. This replacement (PR2 follow-up)
/// pins the exact count per rule per document the engine emits today, with
/// each count cross-referenced to the GitHub issue that tracks closing the
/// firing (or a documented reason for permanent expected-firing on
/// legitimate source noise).
///
/// Failure modes the test catches:
/// 1. **New diagnostic appearing** — engine starts firing a rule we didn't
///    pin → fail (regression OR new rule needing pin).
/// 2. **Count increase** — engine fires N+ where we pinned N → fail (rule
///    got more aggressive).
/// 3. **Count decrease** — engine fires < N where we pinned N → fail
///    (improvement! shrink the pin).
/// 4. **Diagnostic disappears entirely** — engine emits 0 where we pinned
///    N → fail (improvement! remove the entry).
/// 5. **Pinned stem missing fixture** — allowlist entry has no corresponding
///    file → fail (stale pin after rename/delete).
///
/// Every failure is batched and reported together so a single regression
/// doesn't mask other drift.
#[test]
fn document_fixtures_lint_against_expected() {
    let engine = make_engine();
    let docs_root = corpus_root().join("documents");
    let marked_dir = docs_root.join("marked");
    assert!(
        marked_dir.is_dir(),
        "documents/marked directory missing at {}",
        marked_dir.display()
    );

    let mut marked_files: Vec<_> = std::fs::read_dir(&marked_dir)
        .expect("read documents/marked")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
        .collect();
    marked_files.sort();
    assert!(
        !marked_files.is_empty(),
        "no marked document fixtures found in {}",
        marked_dir.display()
    );

    let mut violations: Vec<String> = Vec::new();
    let mut fixture_stems: std::collections::HashSet<String> = std::collections::HashSet::new();

    for marked in &marked_files {
        let stem = marked
            .file_stem()
            .expect("marked file stem")
            .to_string_lossy()
            .into_owned();
        fixture_stems.insert(stem.clone());

        let source = std::fs::read(marked)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", marked.display()));
        let result = engine.lint(&source);

        let mut observed: HashMap<&str, usize> = HashMap::new();
        for d in &result.diagnostics {
            *observed.entry(d.rule.as_str()).or_insert(0) += 1;
        }

        let expected = lookup_expected_diagnostics(&stem);
        let mut expected_by_rule: HashMap<&str, &ExpectedRuleCount> = HashMap::new();
        for e in expected {
            expected_by_rule.insert(e.rule, e);
        }

        // Check observed against pinned.
        for (rule, observed_count) in &observed {
            match expected_by_rule.get(rule) {
                Some(pin) if pin.count == *observed_count => { /* match — clean */ }
                Some(pin) => {
                    let direction = if *observed_count < pin.count {
                        "decreased (likely IMPROVEMENT — shrink pin)"
                    } else {
                        "increased (REGRESSION — investigate)"
                    };
                    let issue_ref = if pin.issue == 0 {
                        "no issue (correct firing)".to_string()
                    } else {
                        format!("#{}", pin.issue)
                    };
                    violations.push(format!(
                        "{stem}: rule {rule}: pinned {} ({}), observed {} — count {direction}\n    pin reason: {}",
                        pin.count, issue_ref, observed_count, pin.reason
                    ));
                }
                None => {
                    violations.push(format!(
                        "{stem}: rule {rule}: unexpected firing (observed {observed_count}, not in allowlist) — add a pin or fix the regression"
                    ));
                }
            }
        }

        // Catch entries that pin a count but observe zero (full retirement).
        for (rule, pin) in &expected_by_rule {
            if !observed.contains_key(rule) {
                let issue_ref = if pin.issue == 0 {
                    "no issue (correct firing)".to_string()
                } else {
                    format!("#{}", pin.issue)
                };
                violations.push(format!(
                    "{stem}: rule {rule}: pinned {} ({}) but engine emits 0 — IMPROVEMENT, remove pin\n    pin reason: {}",
                    pin.count, issue_ref, pin.reason
                ));
            }
        }
    }

    // Stale-pin guard: every pinned stem must correspond to a real fixture.
    for (stem, _) in EXPECTED_DOCUMENT_DIAGNOSTICS {
        if !fixture_stems.contains(*stem) {
            violations.push(format!(
                "EXPECTED_DOCUMENT_DIAGNOSTICS entry {stem:?} has no corresponding fixture in documents/marked/; remove the stale pin"
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "{} violation(s) against EXPECTED_DOCUMENT_DIAGNOSTICS:\n  {}",
        violations.len(),
        violations.join("\n  ")
    );
}
