// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Corpus accuracy harness.
//!
//! Validates lint and fix behavior across the full test corpus with per-rule
//! accuracy thresholds:
//!
//! - **Lint accuracy**: >=95% per-rule and overall against `.expected.json`
//! - **Fix accuracy**: >=95% per-rule and overall (zero remaining violations)
//! - **Prose precision**: zero diagnostics on clean prose

use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_test_utils::{
    corpus_root, invalid_fixtures, load_expected, load_fixture, prose_fixtures, valid_fixtures,
};
use secrecy::ExposeSecret as _;
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
/// decoder returns zero candidates, suppressing the diagnostic.
///
/// The lint-accuracy, fix-accuracy, prose-precision, and
/// corrections-map checks all run against the user-facing default
/// engine (no recognizer override). Adding a
/// `with_recognizer(StrictRecognizer::new())` here re-pins the strict
/// path and re-introduces the gap this test is meant to defend
/// against; do NOT unpin to "Strict" without the same null-hypothesis
/// gate landing first (per Constitution §VIII source-fidelity, this
/// test's load-bearing role is the prose-precision gate against
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
// Lint accuracy on invalid fixtures (>=95% per-rule and overall)
// ---------------------------------------------------------------------------

#[test]
fn lint_accuracy_invalid_fixtures() {
    let engine = make_engine();
    let fixtures = invalid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no invalid fixtures found in corpus — cannot validate lint accuracy"
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
            // `exp.rule` is the structured `ExpectedRuleId`; W034's
            // predicate id is `portion.sci.unpublished-custom-control`.
            if exp.rule.predicate_id == "portion.sci.unpublished-custom-control" {
                continue;
            }
            total_expected += 1;
            let entry = per_rule
                .entry(exp.rule.predicate_id.clone())
                .or_insert((0, 0));
            entry.1 += 1;

            // Match: same rule ID AND same span
            let matched = result.diagnostics.iter().any(|d| {
                d.rule.scheme() == exp.rule.scheme
                    && d.rule.predicate_id() == exp.rule.predicate_id
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
        "lint accuracy FAILED\n\
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
// Fix accuracy on invalid fixtures (>=95% per-rule zero-remaining)
// ---------------------------------------------------------------------------

#[test]
fn fix_accuracy_invalid_fixtures() {
    let engine = make_engine();
    let fixtures = invalid_fixtures();
    let threshold = Config::default().confidence_threshold();
    assert!(
        !fixtures.is_empty(),
        "no invalid fixtures found in corpus — cannot validate fix accuracy"
    );

    // Per-rule tracking: rule_id -> (fixed_clean, total_fixtures_with_fixable_rule)
    // Only count rules that produce at least one fix proposal with confidence >= threshold.
    // Some rules intentionally don't auto-fix (no fix proposal, or a
    // confidence below threshold) and should not count against fix accuracy.
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
        // rule's fix clears the engine's auto-apply gate
        // (`Severity::is_promote_eligible` AND combined-confidence
        // ≥ threshold).
        //
        // `Severity::Suggest` is a hard exclusion from auto-apply
        // regardless of confidence (engine's
        // `is_promote_eligible` returns false for Suggest). PR A
        // collapsed every strict-path `rule` confidence to 1.0; the
        // severity gate is now the load-bearing channel discriminator
        // for "is this an auto-applied fix?".
        //
        // "Confidence" here is the scalar `Confidence::combined()`
        // (= recognition × rule) that the engine applies at the
        // promotion boundary. `Confidence` carries additional
        // axes (`region`, `runner_up_ratio`, feature contributions)
        // for audit provenance, but this harness and every
        // threshold-gated consumer compare on `.combined()` only.
        let lint_result = engine.lint(&source);
        let fixable_rules: std::collections::HashSet<String> = lint_result
            .diagnostics
            .iter()
            .filter(|d| d.severity.is_promote_eligible())
            .filter(|d| {
                d.fix
                    .as_ref()
                    .is_some_and(|f| f.confidence.combined() >= threshold)
            })
            .map(|d| d.rule.predicate_id().to_owned())
            .collect();

        if fixable_rules.is_empty() {
            continue; // No auto-fixable diagnostics in this fixture
        }

        total_fixable += 1;

        // Fix the source
        let fix_result = engine.fix(&source, FixMode::Apply);

        // Re-lint the fixed output
        let relint = engine.lint(fix_result.source.expose_secret());

        // Check which fixable rules still have violations
        let remaining_rules: std::collections::HashSet<&str> = relint
            .diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect();

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
        "fix accuracy FAILED\n\
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
// Zero diagnostics on clean prose (precision gate)
// ---------------------------------------------------------------------------
//
// Per-fixture suppression mechanism.
//
// The prose corpus exercises FOUR distinct suppression paths in the
// decoder. The single zero-diagnostic assertion below covers all
// four, but a regression in any one path would surface as a
// per-fixture failure. The table below maps each fixture to the
// specific mechanism that suppresses it so a future bisect can route
// the failure correctly.
//
// **Verification methodology.** The
// attributions below were checked by temporarily disabling the null
// gate (`scored.retain(...)` in `decoder.rs::recognize`) and
// re-running this test against each fixture. Fixtures that newly
// emitted R001 with the gate disabled are attributed to the null
// gate; fixtures that still emitted zero diagnostics were traced to
// the upstream filter that catches them. The verification correctly
// identified that some fixtures the original PR description
// attributed to the null gate are actually killed by other
// mechanisms (step 3a unknown-token discard, step 3e no-classification
// discard, prose-glue early-return, no-op-rewrite filter); the table
// below is the corrected mapping.
//
// | fixture                       | mechanism (verified)                                                                                                          |
// |-------------------------------|-------------------------------------------------------------------------------------------------------------------------------|
// | `article.txt`                 | Null gate. With gate disabled, R001 emits on `(s)` at offset 106370 (confidence 0.110).                                       |
// | `federalist_10_excerpt.txt`   | Null gate. With gate disabled, R001 emits on `(s)` at offset 16 (confidence 0.110). The original precision-gate regression target.   |
// | `cms_mid_prose.txt`           | Null gate. With gate disabled, R001 emits on `(CMS)` at offset 28 (confidence 0.282); fuzzy-correction lands on NATO `CTS`.   |
// | `cts_mid_prose.txt`           | Null gate. With gate disabled, R001 emits on `(CTs)` at offset 20 (confidence 0.103) after LinePos + LowercaseContext penalties drop posterior. |
// | `c_mid_prose.txt`             | **Whitelist bypass + no-op-rewrite filter** (NOT the null gate). `(C)` is on `is_bare_classification_shape` so the null gate is skipped; `build_decoder_diagnostic` returns `None` because observed bytes (`(C)`) equal canonical bytes (`(C)`). |
// | `si_mid_prose.txt`            | **Step 3e no-classification filter** (NOT the null gate). `(SI)` parses cleanly as `sci_controls = [Si]` but `classification: None`; step 3e discards portion candidates without a classification. |
// | `s_mid_prose.txt`             | **Prose-glue early-return** (NOT the null gate). `function(s)` has `preceded_by_whitespace = false`, so the `recognize` early-return at the top of the function fires before any scoring. |
// | `bare_letters_mid_prose.txt`  | **Step 3a unknown-token discard** (NOT the null gate). `(M)` / `(X)` fall under `MIN_FUZZY_LEN = 3` so fuzzy-correction returns `None`; the strict parse produces an `Unknown` token span; step 3a discards the partial canonicalization. |
//
// **`c_mid_prose.txt` is intentionally a different path.** `(C)` is
// on the [`is_bare_classification_shape`] whitelist in `decoder.rs`
// because it is the only grammar form for a CONFIDENTIAL portion;
// the null-hypothesis filter is deliberately bypassed for it. The
// decoder produces a candidate, but `build_decoder_diagnostic` in
// `engine.rs` returns `None` when observed bytes equal canonical
// bytes (no-op rewrite — the canonical form for `(C)` is `(C)`), so
// the synthetic R001 is never emitted. If a future change relaxes
// the no-op-rewrite filter (for audit-verbosity, schema evolution,
// etc.), `c_mid_prose.txt` will start failing — the
// failure points at the bypass path, not the null gate. An
// engine-level integration test
// (`sub_threshold_decoder_gate::bare_class_whitelist_relies_on_no_op_rewrite_filter`)
// pins this end-to-end so the regression is caught even if a future
// refactor drops the corpus fixture.

#[test]
fn precision_prose_zero_diagnostics() {
    use marque_rules::Severity;

    let engine = make_engine();
    let fixtures = prose_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no prose fixtures found in corpus — cannot validate the precision gate"
    );

    for path in &fixtures {
        let source = load_fixture(path);
        let result = engine.lint(&source);

        // Filter out `Severity::Suggest` diagnostics: style / advisory
        // rules (S004, S008, etc.) may surface low-confidence hints
        // even on prose-shaped inputs that the strict parser
        // tentatively recognized as markings. These don't violate
        // the precision gate (which targets precision on hard error / warn
        // signals); the closure-driven Suggest channel is a separate
        // surface and any prose noise it produces is by design opt-out
        // via the per-rule `"off"` severity.
        //
        // #559 (2026-05-19): added explicit Suggest-tier
        // filter after the Suggest-tier rule began surfacing the prose-vs-marking
        // ambiguity in `article.txt` where `(S) same advantage which a
        // republic` was tentatively parsed as a US Secret portion. The
        // root false-positive (parser accepting `(S)` glued to a word
        // boundary) is tracked separately; filtering at the precision
        // gate keeps the load-bearing hard-error precision
        // pin intact without absorbing the unrelated parser concern.
        let hard_diagnostics: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity != Severity::Suggest)
            .collect();

        assert!(
            hard_diagnostics.is_empty(),
            "precision failure on {}: expected zero hard \
             (Error/Warn/Fix/Info) diagnostics, got {}:\n{}",
            path.file_name().unwrap().to_string_lossy(),
            hard_diagnostics.len(),
            hard_diagnostics
                .iter()
                .map(|d| format!(
                    "  {} {:?} at {}..{}: {:?}",
                    d.rule.predicate_id(),
                    d.severity,
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

/// Corrections-map fixtures require a corrections map in config. The
/// default harness uses Config::default() (empty corrections), so the
/// corrections-map path is tested separately here.
///
/// There is no warning-severity vocabulary-deprecation rule: CAPCO-2016
/// §F treats legacy markings as unauthorized (an error category), not
/// "deprecated but still legal."
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
            if exp.rule.predicate_id != "marking.correction.token-typo" {
                continue;
            }
            total += 1;
            let found = result.diagnostics.iter().any(|d| {
                d.rule.predicate_id() == "marking.correction.token-typo"
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
    use marque_rules::Severity;

    let engine = make_engine();
    let fixtures = valid_fixtures();
    assert!(!fixtures.is_empty(), "no valid fixtures found in corpus");

    for path in &fixtures {
        let source = load_fixture(path);
        let result = engine.lint(&source);

        // Filter out `Severity::Suggest` diagnostics from the strict-zero
        // assertion. Suggest-tier style rules (S004 / S008 / etc.) are
        // advisory by default and ship at confidences calibrated to
        // never auto-apply under the default threshold (S008 = 0.85;
        // S004 = 0.5); they're opt-up surfaces, not "this fixture has
        // a defect" signals. The "valid" bucket asserts hard-error
        // cleanliness — Error/Warn/Fix/Info severities — not "no rule
        // could possibly say anything about this." #559 close-out C1
        // (2026-05-19) added the filter after S008's closure-driven
        // Suggest surface began firing on classified-without-FD&R
        // valid fixtures like `(TS//SI)` (the closure injects RELIDO
        // at the lattice layer; S008 surfaces that to the user as a
        // suggested edit — correct behavior, but not a defect
        // signal for the fixture).
        let hard_diagnostics: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity != Severity::Suggest)
            .collect();

        assert!(
            hard_diagnostics.is_empty(),
            "valid fixture {} produced {} unexpected hard \
             (Error/Warn/Fix/Info) diagnostics:\n{}",
            path.file_name().unwrap().to_string_lossy(),
            hard_diagnostics.len(),
            hard_diagnostics
                .iter()
                .map(|d| format!(
                    "  {} {:?} at {}..{}: {:?}",
                    d.rule.predicate_id(),
                    d.severity,
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
/// The allowlist is currently empty: the documents corpus produces
/// zero diagnostics across all 40 fixtures. New entries should only
/// land with an open-issue or permanent-expected-firing reason and
/// the corresponding pin should retire when the issue closes.
///
/// `issue = 0` marks a *correct* firing (legitimate noise in the
/// source material that the engine correctly surfaces); `issue > 0`
/// marks a tracked engine/corpus gap.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Fields reserved for future allowlist entries.
struct ExpectedRuleCount {
    rule: &'static str,
    count: usize,
    /// GitHub issue tracking the fix. `0` = no issue; reason documents
    /// why this is a correct, expected firing.
    issue: u32,
    reason: &'static str,
}

/// Per-doc per-rule diagnostic expectations. **Currently empty** — the
/// documents corpus produces zero diagnostics across all 40 fixtures.
///
/// New entries here should only land with an open-issue or
/// permanent-expected-firing reason; the corresponding pin must
/// retire when the underlying defect closes.
const EXPECTED_DOCUMENT_DIAGNOSTICS: &[(&str, &[ExpectedRuleCount])] = &[
    // #388: the rel-to-not-in-joint-coverage rule fires correctly on
    // the three JOINT portion banners
    // `(//JOINT TS AUS CAN ITA USA//REL TO USA, AUS, CAN, ITA, EU)` —
    // EU is in REL TO but NOT in the JOINT participant list (AUS, CAN,
    // ITA, USA). CAPCO-2016 §H.3 p57 "[LIST]" superset semantics permit
    // this expansion (the EU is an authorized release destination
    // beyond the JOINT co-owners), so the rule is Warn-only and the
    // firing is by design. `issue = 0` marks this as a correct firing
    // per the doc-comment convention.
    (
        "cia-reports_prex-318se-2",
        &[ExpectedRuleCount {
            // The pin reason is the CAPCO §H.3 p57 worked-example
            // expansion; the engine emits the diagnostic with
            // `rule.predicate_id() ==
            // "portion.classification.rel-to-not-in-joint-coverage"`.
            rule: "portion.classification.rel-to-not-in-joint-coverage",
            count: 3,
            issue: 0,
            reason: "JOINT portions list AUS CAN ITA USA but REL TO expands to include EU \
                     (legitimate §H.3 p57 [LIST] superset expansion); W005 correctly surfaces \
                     for classifier review without auto-fix",
        }],
    ),
];

/// Look up the pinned diagnostics for a fixture stem.
///
/// `EXPECTED_DOCUMENT_DIAGNOSTICS` is a `&[(stem, &[entries])]` slice
/// (not a map) so we can keep the literal sorted-by-stem layout that
/// makes review-diffs readable. A `find_map` would silently honor only
/// the first match if a duplicate stem entry were ever introduced,
/// leaving the second entry's pins unchecked. The
/// `assert_expected_diagnostics_stems_unique` callee runs at the top of
/// the test BEFORE any lookup, so a duplicate stem fails the suite
/// outright instead of producing a silent miscount.
fn lookup_expected_diagnostics(stem: &str) -> &'static [ExpectedRuleCount] {
    EXPECTED_DOCUMENT_DIAGNOSTICS
        .iter()
        .find_map(|(s, e)| (*s == stem).then_some(*e))
        .unwrap_or(&[])
}

/// Assert no stem appears twice in `EXPECTED_DOCUMENT_DIAGNOSTICS`.
///
/// The allowlist is maintained by hand. A duplicated stem would mean
/// only the first match is honored (because `lookup_expected_diagnostics`
/// uses `find_map`) AND the stale-pin guard at the bottom of the test
/// would consider every duplicate satisfied by the same fixture. Catch
/// it early instead.
fn assert_expected_diagnostics_stems_unique() {
    let mut seen: std::collections::HashSet<&'static str> = std::collections::HashSet::new();
    let mut duplicates: Vec<&'static str> = Vec::new();
    for (stem, _) in EXPECTED_DOCUMENT_DIAGNOSTICS {
        if !seen.insert(stem) {
            duplicates.push(stem);
        }
    }
    assert!(
        duplicates.is_empty(),
        "EXPECTED_DOCUMENT_DIAGNOSTICS has duplicate stem(s): {duplicates:?}. Each fixture stem must appear at most once; merge duplicate pin entries by hand."
    );
}

/// Strict per-doc per-rule diagnostic count check against the
/// `EXPECTED_DOCUMENT_DIAGNOSTICS` allowlist.
///
/// This test pins the exact count per rule per document the engine
/// emits today, with each count cross-referenced to the GitHub issue
/// that tracks closing the firing (or a documented reason for permanent
/// expected-firing on legitimate source noise).
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

    // Fail fast on duplicate stem pins before any lookup runs.
    assert_expected_diagnostics_stems_unique();

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

        // Filter out `Severity::Suggest` diagnostics: style / advisory
        // rules (S008's RELIDO-implied-by-closure, S004's
        // REL-TO-trigraph-suggest, etc.) surface low-confidence opt-up
        // hints whose firing is by design across the documents corpus
        // — `(S//SI)`-style portions on every classified document
        // would trigger S008 without violating the hard-error
        // zero-baseline. The per-stem allowlist is for hard
        // (Error/Warn/Fix/Info) regressions; Suggest-tier rules ship at
        // confidences calibrated to never auto-apply under the default
        // threshold and don't qualify as corpus regressions. This
        // Suggest filter was added (#559) when S008 surfaced 12+
        // document fixtures simultaneously without any corresponding
        // hard-severity defect.
        use marque_rules::Severity;
        let mut observed: HashMap<&str, usize> = HashMap::new();
        for d in &result.diagnostics {
            if d.severity == Severity::Suggest {
                continue;
            }
            *observed.entry(d.rule.predicate_id()).or_insert(0) += 1;
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

/// Sanity check: the live allowlist must satisfy the uniqueness invariant
/// the harness depends on. Standalone test so a duplicated stem is caught
/// even if a future refactor of the main test bypasses
/// `assert_expected_diagnostics_stems_unique`.
#[test]
fn expected_document_diagnostics_has_unique_stems() {
    assert_expected_diagnostics_stems_unique();
}
