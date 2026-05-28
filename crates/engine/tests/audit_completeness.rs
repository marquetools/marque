// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Audit completeness tests.
//!
//! Every AppliedFix has a complete payload (no missing fields, no
//! orphaned changes). Sub-threshold FixProposals never appear in the
//! audit stream.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_rules::MessageTemplate;
use marque_rules::audit::AuditLine;
use std::collections::HashMap;
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
    // This source triggers the REL-TO-missing-USA fix at confidence
    // 0.97 — the "high confidence single fix" anchor.
    let source = b"SECRET//REL TO GBR\n";
    let result = engine.fix(source, FixMode::Apply);

    assert!(
        result.applied_fixes().next().is_some(),
        "should have at least one applied fix"
    );

    // The engine's audit stream is `Vec<AuditLine>`. Each line is
    // either a marking fix (`AppliedFix`) or a text-correction
    // (`AppliedTextCorrection`); both carry the audit-record contract
    // fields. Walk both arms.
    for line in &result.audit_lines {
        let (rule, span, source, timestamp) = match line {
            AuditLine::AppliedFix(f) => (&f.rule, f.span, f.source, f.timestamp),
            AuditLine::TextCorrection(tc) => (&tc.rule, tc.span, tc.source, tc.timestamp),
            _ => continue,
        };

        // Rule is the 2-tuple `(scheme, predicate_id)`. The engine
        // emits only known schemes (`"capco"` for CAPCO rules,
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
fn applied_fixes_always_meet_configured_threshold() {
    // Structural gate invariant: every fix the engine promotes to
    // `applied_fixes()` must have `combined() >= configured_threshold`.
    //
    // PR A pinned every strict-path emission at `recognition = 1.0`,
    // so strict-path fixes always clear any threshold ≤ 1.0. The
    // meaningful sub-threshold-blocks-apply assertion comes from the
    // decoder path: `(SERCET)` is a bare-classification mangled
    // portion the decoder recognizes as `(SECRET)` with recognition
    // ≈ 0.926 (the prose null-hypothesis runner-up shrinks the
    // posterior for short fuzzy fixes — bare-classification portions
    // have a non-trivial prose prior so the posterior never saturates
    // above the default 0.95 threshold). At default threshold the
    // decoder candidate is sub-threshold and MUST NOT be auto-applied;
    // the lint post-pass demotes its diagnostic to `Severity::Suggest`.
    //
    // The engine is constructed with the default
    // `StrictOrDecoderRecognizer` (no explicit `with_recognizer` call)
    // so the decoder fallback fires on the mangled input.
    let config = Config::default();
    let threshold = config.confidence_threshold();
    let engine = Engine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let source: &[u8] = b"(SERCET)";
    let result = engine.fix(source, FixMode::Apply);

    // No decoder-path fix lands in applied at the default threshold:
    // the recognition score is sub-threshold so the lint post-pass
    // has already demoted the diagnostic to Suggest, which is then
    // a hard exclusion from auto-apply.
    let decoder_applied: Vec<_> = result
        .applied_fixes()
        .filter(|f| f.source == marque_rules::FixSource::DecoderPosterior)
        .collect();
    assert!(
        decoder_applied.is_empty(),
        "decoder-path fix for `(SERCET)` must NOT auto-apply at \
         the default threshold {threshold} (recognition ~0.926 is \
         sub-threshold); applied count: {}",
        decoder_applied.len(),
    );

    // Every applied fix that DID land (strict-path collateral, if
    // any) still satisfies the gate.
    for fix in result.applied_fixes() {
        let combined = fix.fix.replacement.confidence.combined();
        assert!(
            combined >= threshold,
            "sub-threshold fix (confidence {combined}) appeared in \
             applied at threshold {threshold}"
        );
    }

    // Pin the load-bearing property explicitly: the sub-threshold
    // decoder candidate survives in remaining_diagnostics as a
    // Suggest with recognition strictly below 0.95. The
    // `decoder_applied.is_empty()` assertion above would fire first
    // on calibration drift above threshold; this pin documents the
    // sub-threshold property at the call site so a future maintainer
    // doesn't have to re-derive it.
    let suggest_recognition = result
        .remaining_diagnostics
        .iter()
        .find(|d| d.severity == marque_rules::Severity::Suggest)
        .and_then(|d| d.fix.as_ref())
        .map(|f| f.confidence.recognition);
    assert!(
        suggest_recognition.is_some_and(|r| r < threshold),
        "surviving Suggest candidate must have recognition < threshold \
         {threshold}; got {suggest_recognition:?}. A drift above the \
         threshold indicates decoder calibration shifted such that \
         `(SERCET)` is no longer a sub-threshold gate fixture.",
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
// R002 audit-record integrity
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

// ---------------------------------------------------------------------------
// Issue #709 — MessageTemplate parity between lint Diagnostic and audit AppliedFix
// ---------------------------------------------------------------------------
//
// Audit-record contract: for every fix-emitting rule reachable via a fixture,
// the lint-side `Diagnostic.message.template` (what `marque check` shows) and
// the fix-side template projected into `AppliedFix.message.template` /
// `AppliedTextCorrection.message.template` (what `marque fix` writes to the
// NDJSON audit log) MUST each match that rule's production-side template
// assignment. The two sides COINCIDE for single-Message rules (R001, C001,
// banner-rollup), but they deliberately DIFFER for rules whose violation
// class and fix action differ — e.g. E002 lints `NonCanonicalOrder` but its
// USA-injection fix emits `RequiredByPresence`. The invariant is "each side
// matches production," not "the two sides are equal."
//
// Pre-issue-#709 the engine's test stubs and the wasm parity helper hardcoded
// `MessageTemplate::BannerRollupMismatch` on the FixIntent message field of
// every synthetic fixture, baking a mislabel into the test surface that
// masked the per-rule assignment.
//
// `Config::default()` starts with an empty corrections map and no
// migration-table inputs; this helper then supplies a small in-memory
// corrections map so the C001 path is exercisable alongside the
// rule-emitting paths.

fn engine_with_corrections(corrections: HashMap<String, String>) -> Engine {
    let mut config = Config::default();
    config.corrections = corrections;
    Engine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// Fixture-rule pair: a source buffer that triggers exactly one rule
/// whose lint + audit templates we want to verify against the
/// production-side per-rule emissions.
///
/// Two templates are pinned because the contract is **per-side**, not
/// strict equality — some production rules deliberately emit different
/// templates on the Diagnostic (violation-class label) and the FixIntent
/// (action-class label). E002, for example, emits `NonCanonicalOrder` on
/// the Diagnostic and `RequiredByPresence` on the `FactAdd` FixIntent
/// (USA injection) because the violation is "ordering" while the fix
/// action is "add required token".
struct TemplateParityFixture {
    /// Free-form name used in assertion messages.
    name: &'static str,
    /// Source buffer to lint and fix.
    source: &'static [u8],
    /// Predicate id of the rule under test.
    predicate_id: &'static str,
    /// Expected template on the lint-side Diagnostic.
    expected_lint_template: MessageTemplate,
    /// Expected template on the FixIntent (= projected AppliedFix
    /// template at audit time). `None` if the diagnostic emits without
    /// an autofix.
    expected_fix_template: Option<MessageTemplate>,
}

/// Templates emitted by lint and audit MUST stay in agreement with the
/// production-side per-rule template assignments.
///
/// Issue #709 root cause: synthetic test FixIntents hardcoded
/// `MessageTemplate::BannerRollupMismatch` on the audit side regardless
/// of which rule they stood in for, breaking the production-side per-rule
/// template assignment and the implicit wire-format JSON projection
/// contract. The fix corrected each per-rule template; this test pins
/// the invariant going forward.
///
/// Coverage targets the parity_corpus.json rows + the engine-synthetic
/// sentinels:
///
/// - E002 (`portion.dissem.rel-to-missing-usa`):
///   lint `NonCanonicalOrder`, fix `RequiredByPresence` (USA-missing
///   case) — intentional asymmetry, see `rel_to.rs`'s
///   `MissingUsaTrigraphRule`.
/// - Banner-rollup rules (SAR, non-IC dissem):
///   both sides emit `BannerRollupMismatch`.
/// - C001 (`marking.correction.token-typo`): see
///   `lint_diag_template_equals_text_correction_template_for_c001`.
/// - R001 (`recognition.decoder-recognized`): see
///   `r001_lint_and_applied_templates_agree`.
#[test]
fn diagnostic_and_fix_templates_match_production_per_rule() {
    let fixtures: &[TemplateParityFixture] = &[
        // E002 USA-missing path — Diagnostic carries the violation
        // template, FixIntent (FactAdd USA) carries the action template.
        // Both are pinned to catch any drift from the production
        // emission shape.
        TemplateParityFixture {
            name: "E002 rel-to-missing-usa",
            source: b"SECRET//REL TO GBR, AUS\n",
            predicate_id: "portion.dissem.rel-to-missing-usa",
            expected_lint_template: MessageTemplate::NonCanonicalOrder,
            expected_fix_template: Some(MessageTemplate::RequiredByPresence),
        },
        // Banner-rollup rules emit a lint `Diagnostic` carrying
        // `BannerRollupMismatch` with no attached `FixIntent` (see
        // `banner/eval_sar.rs`). This fixture pins the lint-side template
        // and asserts `.fix` is None (`expected_fix_template: None`).
        //
        // There is no audit-side template to pin for these predicates:
        // on `Engine::fix` the rollup diagnostic produces zero applied
        // audit lines (it is advisory — not auto-promoted to an
        // `AppliedFix`/`AppliedTextCorrection`), verified by the
        // `banner_rollup_fix_produces_no_audit_line` companion test below.
        // So lint-vs-audit template parity is vacuously satisfied here,
        // not asserted.
        TemplateParityFixture {
            name: "banner-rollup SAR portions",
            source: b"(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN\n",
            predicate_id: "banner.banner-rollup.sar-portions-roll-up",
            expected_lint_template: MessageTemplate::BannerRollupMismatch,
            expected_fix_template: None,
        },
        TemplateParityFixture {
            name: "banner-rollup non-IC dissem",
            source: b"(S//NF//ND)\nSECRET//NOFORN\n",
            predicate_id: "banner.banner-rollup.non-ic-dissem-roll-up",
            expected_lint_template: MessageTemplate::BannerRollupMismatch,
            expected_fix_template: None,
        },
    ];

    let engine = test_engine();
    for fx in fixtures {
        // Lint side — find a diagnostic for this rule and pin its template.
        let lint = engine.lint(fx.source);
        let lint_diag = lint
            .diagnostics
            .iter()
            .find(|d| d.rule.predicate_id() == fx.predicate_id)
            .unwrap_or_else(|| {
                panic!(
                    "{}: lint must produce a Diagnostic for predicate `{}`; \
                     got {} diagnostics: {:?}",
                    fx.name,
                    fx.predicate_id,
                    lint.diagnostics.len(),
                    lint.diagnostics
                        .iter()
                        .map(|d| d.rule.to_string())
                        .collect::<Vec<_>>(),
                )
            });
        assert_eq!(
            lint_diag.message.template(),
            fx.expected_lint_template,
            "{}: lint Diagnostic.message.template mismatch",
            fx.name,
        );

        // Audit-side FixIntent template. `Diagnostic.fix.message` is the
        // value projected into `AppliedFix.message` at `__engine_promote`
        // time. Pre-issue-#709 several test stubs hardcoded
        // `BannerRollupMismatch` here regardless of the production
        // emission, breaking the per-rule template assignment.
        match (lint_diag.fix.as_ref(), fx.expected_fix_template) {
            (Some(fi), Some(expected)) => {
                assert_eq!(
                    fi.message.template(),
                    expected,
                    "{}: Diagnostic.fix.message.template \
                     (= AppliedFix.message.template projection) mismatch",
                    fx.name,
                );
            }
            (None, None) => { /* both sides agree: no autofix attached */ }
            (Some(fi), None) => panic!(
                "{}: lint Diagnostic carries a FixIntent (template {:?}) \
                 but the fixture declares no expected_fix_template",
                fx.name,
                fi.message.template(),
            ),
            (None, Some(expected)) => panic!(
                "{}: lint Diagnostic carries NO FixIntent but the fixture \
                 declares expected_fix_template = {expected:?}",
                fx.name,
            ),
        }
    }
}

/// Companion to `diagnostic_and_fix_templates_match_production_per_rule`:
/// the banner-rollup predicates emit advisory lint diagnostics that are
/// NOT auto-applied, so `Engine::fix` produces no audit line for them —
/// which is why the parity test pins only their lint-side template and
/// has no audit-side template to assert.
///
/// This test pins that advisory status. If a future change promotes a
/// banner-rollup diagnostic to an applied fix (an `AppliedFix` or
/// `AppliedTextCorrection`), this test fails — forcing the author to add
/// the audit-side template assertion the parity test currently omits, so
/// a promotion/projection regression can't slip through silently.
/// (Addresses the PR #752 review note on un-pinned rollup audit templates.)
#[test]
fn banner_rollup_fix_produces_no_audit_line() {
    let engine = test_engine();
    let cases: [(&str, &[u8]); 2] = [
        (
            "banner.banner-rollup.sar-portions-roll-up",
            b"(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN\n",
        ),
        (
            "banner.banner-rollup.non-ic-dissem-roll-up",
            b"(S//NF//ND)\nSECRET//NOFORN\n",
        ),
    ];
    for (predicate, source) in cases {
        let result = engine.fix(source, FixMode::Apply);
        let applied: Vec<&str> = result
            .audit_lines
            .iter()
            .filter_map(|line| match line {
                AuditLine::AppliedFix(f) if f.rule.predicate_id() == predicate => {
                    Some(f.rule.predicate_id())
                }
                AuditLine::TextCorrection(tc) if tc.rule.predicate_id() == predicate => {
                    Some(tc.rule.predicate_id())
                }
                _ => None,
            })
            .collect();
        assert!(
            applied.is_empty(),
            "{predicate}: banner-rollup is advisory — Engine::fix must not \
             emit an applied audit line for it; got {applied:?}. If this rule \
             now auto-applies, pin its audit-side template in \
             `diagnostic_and_fix_templates_match_production_per_rule`.",
        );
    }
}

#[test]
fn lint_diag_template_equals_text_correction_template_for_c001() {
    // C001 (`marking.correction.token-typo`) is a TextCorrection-shaped
    // emission with the corrections map. Lint produces a
    // `Diagnostic::text_correction`; fix produces an
    // `AppliedTextCorrection` audit line — both sides emit
    // `MessageTemplate::CorrectionsApplied` per `pipeline.rs` C001
    // emission.
    let mut corrections = HashMap::new();
    corrections.insert("SERCET".to_owned(), "SECRET".to_owned());
    let engine = engine_with_corrections(corrections);
    let source = b"(TS//SERCET//NF)";

    // Lint side.
    let lint = engine.lint(source);
    let lint_diag = lint
        .diagnostics
        .iter()
        .find(|d| d.rule.predicate_id() == "marking.correction.token-typo")
        .expect("C001 must lint on `(TS//SERCET//NF)` with SERCET→SECRET correction");
    assert_eq!(
        lint_diag.message.template(),
        MessageTemplate::CorrectionsApplied,
        "C001 lint Diagnostic template mismatch",
    );

    // Audit side.
    let result = engine.fix(source, FixMode::Apply);
    let tc = result
        .audit_lines
        .iter()
        .find_map(|line| match line {
            AuditLine::TextCorrection(tc)
                if tc.rule.predicate_id() == "marking.correction.token-typo" =>
            {
                Some(tc)
            }
            _ => None,
        })
        .expect("C001 must produce an AppliedTextCorrection audit line");
    assert_eq!(
        tc.message.template(),
        MessageTemplate::CorrectionsApplied,
        "C001 AppliedTextCorrection template mismatch — \
         audit-record contract violated (issue #709)",
    );
}

#[test]
fn r001_lint_and_applied_templates_agree() {
    // R001 (`recognition.decoder-recognized`) is the engine-synthetic
    // decoder-recognition diagnostic. Both sides MUST emit
    // `MessageTemplate::DecoderRecognized` — the original symptom in
    // issue #709 was a hypothetical lint=DecoderRecognized /
    // audit=BannerRollupMismatch mismatch, addressed by the
    // post-#736 engine.rs split. This test pins the contract for the
    // sentinel so a future synthesis refactor cannot silently drift.
    //
    // `(TS//SAR-fk)` triggers R001 via the decoder's lowercase-program-id
    // recovery path; the threshold-zero config admits R001's
    // sub-default-threshold fix into the audit stream.
    let mut config = Config::default();
    config
        .set_confidence_threshold(0.0)
        .expect("0.0 is a valid threshold");
    let engine = Engine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles");
    let source = b"(TS//SAR-fk)";

    // Lint side.
    let lint = engine.lint(source);
    let r001 = lint
        .diagnostics
        .iter()
        .find(|d| d.rule.predicate_id() == "recognition.decoder-recognized")
        .expect("R001 must lint on `(TS//SAR-fk)`");
    assert_eq!(
        r001.message.template(),
        MessageTemplate::DecoderRecognized,
        "R001 lint Diagnostic must emit `DecoderRecognized` template",
    );

    // Audit side.
    let result = engine.fix(source, FixMode::Apply);
    let r001_audit = result
        .audit_lines
        .iter()
        .find_map(|line| match line {
            AuditLine::AppliedFix(f)
                if f.rule.predicate_id() == "recognition.decoder-recognized" =>
            {
                Some(f)
            }
            _ => None,
        })
        .expect("R001 must produce an AppliedFix audit line under threshold=0");
    assert_eq!(
        r001_audit.message.template(),
        MessageTemplate::DecoderRecognized,
        "R001 AppliedFix template MUST agree with lint Diagnostic — \
         audit-record contract violated (issue #709)",
    );
}
