// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

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
use secrecy::ExposeSecret as _;
use std::time::{Duration, UNIX_EPOCH};

/// Fixed timestamp for deterministic audit records.
const FIXED_TS: u64 = 1_700_000_000; // 2023-11-14T22:13:20Z

fn test_engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn mixed_confidence_source() -> Vec<u8> {
    // E002 at confidence 0.97 (REL TO missing USA → fix applied), and
    // E010 (bare HCS legacy form, §H.4 p62) emitting a no-fix
    // Severity::Error diagnostic that stays in `remaining_diagnostics`.
    //
    // Fixture history:
    // - PR 3c.B Commit 6 retired the original E001/E003 fixture; the
    //   replacement used `//JOINT SECRET USA GBR\n` for the no-fix
    //   line (E014 + E015 both Error-no-fix at the time).
    // - PR 3c.B Sub-PR 8.D.4 migrated E014 to FactAdd; the JOINT
    //   fixture's E014 now auto-applies the missing co-owners to REL
    //   TO, transitively satisfying E015 (CAT_DISSEM is satisfied by
    //   non-empty rel_to per `satisfies_attrs`). The JOINT line no
    //   longer exercises the "remaining no-fix Error diagnostic"
    //   channel. `(TS//HCS)\n` is the stable replacement — E010 is
    //   consciously-deferred (HCS-O vs HCS-P is a classifier decision
    //   per §H.4) and intentionally has no auto-fix path, so its
    //   diagnostic persists through the fix pass and lands in
    //   `remaining_diagnostics`.
    b"SECRET//REL TO GBR, AUS\n(TS//HCS)\n".to_vec()
}

#[test]
fn mixed_confidence_applies_only_high_confidence_fix() {
    let engine = test_engine();
    let source = mixed_confidence_source();
    let result = engine.fix(&source, FixMode::Apply);

    // Only E002 (confidence 0.97 ≥ 0.95) should be applied. The
    // remaining diagnostic on this fixture is E010 (bare HCS legacy
    // form, no-fix on the `(TS//HCS)\n` second line — conscious-defer
    // per §H.4 p62, classifier picks HCS-O vs HCS-P) — verified below
    // in the `remaining_diagnostics` assertion.
    // PR 3c.2.D fixup F-3: `applied_fixes()` returns `impl Iterator`;
    // collect once for index access + Debug formatting.
    let applied: Vec<_> = result.applied_fixes().collect();
    assert_eq!(applied.len(), 1, "applied: {applied:?}");
    assert_eq!(applied[0].rule.predicate_id(), "E002");
    assert!((applied[0].fix.replacement.confidence.combined() - 0.97).abs() < 0.001);

    // The post-fix first line should have USA elevated and codes
    // sorted alphabetically.
    let fixed_text = String::from_utf8(result.source.expose_secret().to_vec()).unwrap();
    assert!(
        fixed_text.starts_with("SECRET//REL TO USA"),
        "expected canonical REL TO list, got: {fixed_text:?}"
    );

    // E010 (bare HCS, no-fix Error) remains.
    assert!(
        !result.remaining_diagnostics.is_empty(),
        "E010 should remain in remaining_diagnostics"
    );
    assert!(
        result
            .remaining_diagnostics
            .iter()
            .any(|d| d.rule.predicate_id() == "E010"),
        "E010 (bare HCS, conscious-defer no-fix) should remain; \
         remaining: {:?}",
        result
            .remaining_diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn dry_run_parity_with_apply() {
    let engine = test_engine();
    let source = mixed_confidence_source();

    let apply_result = engine.fix(&source, FixMode::Apply);
    let dry_result = engine.fix(&source, FixMode::DryRun);

    // DryRun returns original source.
    assert_eq!(dry_result.source.expose_secret(), source);

    // Same number of applied fixes.
    assert_eq!(
        apply_result.applied_fixes().count(),
        dry_result.applied_fixes().count()
    );

    // Same rule IDs and confidences.
    for (a, d) in apply_result.applied_fixes().zip(dry_result.applied_fixes()) {
        assert_eq!(a.rule.predicate_id(), d.rule.predicate_id());
        assert!(
            (a.fix.replacement.confidence.combined() - d.fix.replacement.confidence.combined())
                .abs()
                < f32::EPSILON
        );
    }

    // DryRun records have dry_run=true.
    for fix in dry_result.applied_fixes() {
        assert!(fix.dry_run, "dry-run applied fix should have dry_run=true");
    }

    // Apply records have dry_run=false.
    for fix in apply_result.applied_fixes() {
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

    for fix in result.applied_fixes() {
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

    assert_eq!(r1.applied_fixes().count(), r2.applied_fixes().count());
    for (a, b) in r1.applied_fixes().zip(r2.applied_fixes()) {
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
    let after: LintResult = engine.lint(result.source.expose_secret());

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
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let source = mixed_confidence_source();
    let result = engine.fix(&source, FixMode::Apply);

    for fix in result.applied_fixes() {
        assert_eq!(
            fix.classifier_id.as_deref(),
            Some("TEST-CLASSIFIER-42"),
            "classifier_id should match config"
        );
    }
}

// --- H3: insta snapshot tests for audit NDJSON shape (T046) ---
//
// Post-PR-3c.2.D (atomic schema cutover): the v1 snapshot helper +
// the two snapshot tests retired here. The marque-1.0 NDJSON wire
// shape is pinned at two layers:
//
//   1. `marque/src/render.rs::render_audit_line_produces_valid_v1_0_ndjson`
//      exercises the CLI emit path on a synthetic AuditLine.
//   2. `crates/wasm/tests/audit_v1_0_parity.rs` pins CLI/WASM
//      byte-identity on the same shape (SC-008 invariant).
//
// The retired snapshot tests added zero coverage beyond those two —
// the v1 envelope they pinned is no longer representable, and the
// audit-record contract spec at `contracts/audit-record.md` body §
// is the single wire-format source of truth.

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
        .map(|d| d.rule.predicate_id())
        .collect();
    let dry_rules: Vec<&str> = dry_result
        .remaining_diagnostics
        .iter()
        .map(|d| d.rule.predicate_id())
        .collect();
    assert_eq!(
        apply_rules, dry_rules,
        "remaining diagnostic rule IDs must match between Apply and DryRun"
    );
}

// --- T035c-10: E002 REL TO canonicalization round-trip ---
//
// Verifies that E002's fix splices the canonical REL TO list into the
// banner as a single replacement. The rule's span covers first → last
// `RelToTrigraph` so `Engine::fix` replaces the entire offending list,
// not just the first trigraph — a narrow span plus a full-list
// replacement would corrupt the banner (e.g., leave a stale `, AUS`
// tail after the canonical list).

#[test]
fn e002_fix_rewrites_banner_with_canonical_rel_to_list() {
    let engine = test_engine();

    // USA missing from an unsorted REL TO list. Canonical form per
    // CAPCO-2016 §H.8 lines 3713–3714 is `USA, AUS, GBR`.
    let source = b"SECRET//REL TO GBR, AUS\n".to_vec();
    let result = engine.fix(&source, FixMode::Apply);

    // PR 3c.2.D fixup F-3: `applied_fixes()` is `impl Iterator`; collect
    // once for the filter pass + Debug-render in the assertion message.
    let applied: Vec<_> = result.applied_fixes().collect();
    let e002_applied: Vec<_> = applied
        .iter()
        .filter(|f| f.rule.predicate_id() == "E002")
        .collect();
    assert_eq!(e002_applied.len(), 1, "E002 must apply once: {applied:?}");

    let fixed_text = String::from_utf8(result.source.expose_secret().to_vec()).unwrap();
    assert_eq!(
        fixed_text, "SECRET//REL TO USA, AUS, GBR\n",
        "E002's splice must rewrite the full REL TO list, not just the \
         first trigraph (narrow-span + full-replacement would corrupt the \
         banner)"
    );
}

#[test]
fn e002_fix_rewrites_banner_when_usa_misplaced() {
    let engine = test_engine();

    // USA present but not first, and non-USA entries unsorted. Canonical
    // form: `USA, AUS, GBR`. This exercises the USA-already-present
    // branch of the canonicalization path.
    let source = b"SECRET//REL TO GBR, USA, AUS\n".to_vec();
    let result = engine.fix(&source, FixMode::Apply);

    let fixed_text = String::from_utf8(result.source.expose_secret().to_vec()).unwrap();
    assert_eq!(
        fixed_text, "SECRET//REL TO USA, AUS, GBR\n",
        "E002 must canonicalize a misplaced USA + unsorted rest in one \
         pass"
    );
}

#[test]
fn e002_fix_leaves_no_trailing_comma_after_splice() {
    let engine = test_engine();

    // The RelToBlock ends with a stale `,` after the last trigraph.
    // If the fix span stopped at the last trigraph, the splice would
    // leave `REL TO USA, AUS, GBR,` behind (still malformed). The
    // span must extend through the delimiter tail.
    let source = b"SECRET//REL TO GBR, AUS,\n".to_vec();
    let result = engine.fix(&source, FixMode::Apply);

    let fixed_text = String::from_utf8(result.source.expose_secret().to_vec()).unwrap();
    assert_eq!(
        fixed_text, "SECRET//REL TO USA, AUS, GBR\n",
        "E002 splice must consume the trailing `,` inside the \
         RelToBlock — leaving it behind would be a still-malformed \
         REL TO list"
    );
}

#[test]
fn e002_does_not_corrupt_source_on_multiple_rel_to_blocks() {
    let engine = test_engine();

    // Two REL TO blocks with `//NF//` between them. A naïve
    // first→last splice across both blocks would delete the `//NF//`
    // content. The fix must be suppressed entirely so Engine::fix
    // leaves the source untouched (the diagnostic still fires).
    let source = b"SECRET//REL TO GBR//NF//REL TO AUS\n".to_vec();
    let result = engine.fix(&source, FixMode::Apply);

    // No E002 fix should have been applied — the proposal is None.
    // PR 3c.2.D fixup F-3: collect once for filter + Debug.
    let applied: Vec<_> = result.applied_fixes().collect();
    let e002_applied: Vec<_> = applied
        .iter()
        .filter(|f| f.rule.predicate_id() == "E002")
        .collect();
    assert!(
        e002_applied.is_empty(),
        "E002 must not apply a fix across multiple REL TO blocks: \
         {e002_applied:?}"
    );

    // Intermediate `//NF//` content must survive in the output. Some
    // other rules may still rewrite other parts of the source (e.g.,
    // normalizing), so we only assert that NF is preserved.
    let fixed_text = String::from_utf8(result.source.expose_secret().to_vec()).unwrap();
    assert!(
        fixed_text.contains("NF") || fixed_text.contains("NOFORN"),
        "intermediate NF content must survive multi-block scenario: \
         {fixed_text:?}"
    );
}

// ---------------------------------------------------------------------------
// PR 7b — TwoPassFixer behavioral locks
// ---------------------------------------------------------------------------
//
// These tests lock the consumer-visible properties of the two-pass
// pipeline: the no-pass-1-fixes short-circuit (pass-2 result byte-equals
// pass-0 output, `r002_fired == false`), forward-buffer correctness
// when pass-1 produces multiple fixes in one marking, and R002
// emission when the post-pass-1 buffer cannot re-parse.
//
// "Behavior" means user-visible properties: byte equivalence, the
// `r002_fired` flag, the rule ID of synthetic diagnostics. Internal
// pipeline mechanics (the partition data structure, the synthesis
// helpers) are NOT pinned here — they are implementation details.

#[test]
fn pass1_zero_fixes_skips_reparse() {
    // No `Phase::Localized` rule in the production CAPCO ruleset emits
    // a `FixIntent`-shape fix today (all 4 Localized rules — C001 /
    // E006 / E007 / S004 — flow through pass-0 text-correction). So
    // pass-1 produces zero fixes for every input, and the engine
    // short-circuits the re-parse. The user-visible properties:
    // `r002_fired == false`, and the returned source byte-equals the
    // pass-0 (text-correction) output — pass-2 sees the same buffer
    // pass-0 produced, no intermediate re-parse.
    //
    // 5-year-maintenance posture: a future PR that adds a Localized
    // FixIntent rule would break this test because pass-1 would
    // produce fixes; the test name itself ("zero fixes skips
    // reparse") is the spec, and a regression toward "re-parse
    // always" would be visible here.
    let engine = test_engine();
    let source = mixed_confidence_source();
    let result = engine.fix(&source, FixMode::Apply);
    assert!(
        !result.r002_fired,
        "no pass-1 fixes -> no re-parse -> no R002"
    );
}

#[test]
fn r002_fired_false_on_clean_fixture() {
    // A document that produces NO fixes at all (no diagnostics
    // fire) MUST set `r002_fired = false` and return the source
    // byte-identical to the input. This is the consumer-surface
    // contract that lets a WASM/IDE caller read `r002_fired`
    // without checking `applied.is_empty()` first.
    let engine = test_engine();
    let source = b"This is plain text with no markings.\n".to_vec();
    let result = engine.fix(&source, FixMode::Apply);
    assert!(!result.r002_fired);
    assert_eq!(result.source.expose_secret(), source);
    assert!(result.applied_fixes().next().is_none());
}

#[test]
fn r002_not_minted_as_applied_fix() {
    // Constitution V Principle V lock: NO `AppliedFix` in any fix
    // pass result carries `rule == R002_RULE_ID`. R002 is a
    // diagnostic, never a fix; promotion via `__engine_promote`
    // would inject a false-positive audit record claiming a fix
    // was applied when none was.
    //
    // Pairs with `audit_completeness.rs::r002_does_not_mint_applied_fix`
    // which exercises the same property via a different fixture
    // path.
    let engine = test_engine();
    let source = mixed_confidence_source();
    let result = engine.fix(&source, FixMode::Apply);
    for fix in result.applied_fixes() {
        // Compare against the typed constant rather than the string
        // literal so a future rename of `R002_RULE_ID` is caught here
        // instead of silently passing on stale identifier drift —
        // matches the pattern in
        // `audit_completeness.rs::r002_does_not_mint_applied_fix`
        // (security-panel LOW-4 fix).
        assert_ne!(
            fix.rule,
            marque_engine::R002_RULE_ID,
            "R002 must never appear as an AppliedFix; \
             Constitution V Principle V (audit-record integrity)"
        );
    }
}

#[test]
fn r002_fired_field_independent_of_applied_count() {
    // The `r002_fired` flag is consumer-visible and independent of
    // `applied.is_empty()`. A run that produces N>0 applied fixes
    // and does NOT trigger R002 must still have `r002_fired == false`
    // — a consumer must not infer R002 from "no fixes applied"
    // (the empty case is normal for clean documents).
    let engine = test_engine();
    let source = mixed_confidence_source();
    let result = engine.fix(&source, FixMode::Apply);
    // Fixture is mixed_confidence_source -> E002 fires.
    assert!(result.applied_fixes().next().is_some());
    assert!(!result.r002_fired);
}
