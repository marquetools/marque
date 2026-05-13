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
use serde_json::json;
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
    assert_eq!(result.applied.len(), 1, "applied: {:?}", result.applied);
    assert_eq!(result.applied[0].rule.as_str(), "E002");
    assert!((result.applied[0].confidence.combined() - 0.97).abs() < 0.001);

    // The post-fix first line should have USA elevated and codes
    // sorted alphabetically.
    let fixed_text = String::from_utf8(result.source).unwrap();
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
            .any(|d| d.rule.as_str() == "E010"),
        "E010 (bare HCS, conscious-defer no-fix) should remain; \
         remaining: {:?}",
        result
            .remaining_diagnostics
            .iter()
            .map(|d| d.rule.as_str())
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
    assert_eq!(dry_result.source, source);

    // Same number of applied fixes.
    assert_eq!(apply_result.applied.len(), dry_result.applied.len());

    // Same rule IDs and confidences.
    for (a, d) in apply_result.applied.iter().zip(dry_result.applied.iter()) {
        assert_eq!(a.rule.as_str(), d.rule.as_str());
        assert!((a.confidence.combined() - d.confidence.combined()).abs() < f32::EPSILON);
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
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

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

/// Serialize an `AppliedFix` to a v1-or-v2 audit-record JSON value for
/// snapshot testing. Schema version is sourced from the engine's
/// build-time constant so snapshots track the active schema; v2-only
/// fields (`recognition`, `runner_up_ratio`, `features`) are emitted
/// only when this build is `marque-mvp-2` (default), matching the
/// CLI emitter's dispatch (`marque/src/render.rs::render_audit_record`).
///
/// Per the v2 schema contract documented on `AppliedFix` and the CLI
/// emitter at `marque/src/render.rs:applied_fix_to_audit_json_v2`,
/// `source` and `confidence` (plus its derived `recognition` /
/// `runner_up_ratio` / `features`) are read from the **top-level
/// snapshot fields** on `AppliedFix`, NOT from `proposal.*`. Today the
/// two are identical copies (`__engine_promote` snapshots them
/// unchanged), but a future engine-side adjustment at promotion time
/// (e.g., region-context calibration) must reflect in v2 output. This
/// helper matches the CLI emitter verbatim so snapshot regressions
/// here track the v2 contract, not just the snapshot accident. `rule`
/// and `span` / `original` / `replacement` / `migration_ref` stay on
/// `proposal.*` because they have no separate top-level snapshot.
fn applied_fix_to_json(
    fix: &marque_rules::AppliedFix<marque_capco::CapcoScheme>,
) -> serde_json::Value {
    let source_str = match fix.source {
        marque_rules::FixSource::BuiltinRule => "BuiltinRule",
        marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
        marque_rules::FixSource::MigrationTable => "MigrationTable",
        marque_rules::FixSource::DecoderPosterior => "DecoderPosterior",
        marque_rules::FixSource::DecoderClassificationHeuristic => "DecoderClassificationHeuristic",
    };
    // Schema-pinned scope projection — mirrors `scope_str` /
    // `recanon_scope_str` in `marque/src/render.rs` so the snapshot
    // helper doesn't drift from the production audit JSON shape.
    fn scope_str(s: marque_scheme::Scope) -> &'static str {
        match s {
            marque_scheme::Scope::Portion => "Portion",
            marque_scheme::Scope::Page => "Page",
            marque_scheme::Scope::Document => "Document",
            marque_scheme::Scope::Diff => "Diff",
        }
    }
    fn recanon_scope_str(s: marque_scheme::fix_intent::RecanonScope) -> &'static str {
        match s {
            marque_scheme::fix_intent::RecanonScope::Portion => "Portion",
            marque_scheme::fix_intent::RecanonScope::Page => "Page",
            marque_scheme::fix_intent::RecanonScope::Document => "Document",
        }
    }
    let proposal_json = match &fix.proposal {
        marque_rules::AppliedFixProposal::FixIntent(intent) => {
            // Inline FactRef projection — mirrored from production
            // marque/src/render.rs `fact_ref_to_json`.
            fn fact_ref_to_json(
                fact: &marque_scheme::FactRef<marque_capco::CapcoScheme>,
            ) -> serde_json::Value {
                match fact {
                    marque_scheme::FactRef::Cve(token_id) => json!({
                        "kind": "Cve",
                        "token_id": token_id.0,
                    }),
                    marque_scheme::FactRef::OpenVocab(r) => json!({
                        "kind": "OpenVocab",
                        "ref": format!("{r:?}"),
                    }),
                }
            }
            let kind_obj = match &intent.replacement {
                marque_scheme::ReplacementIntent::FactAdd { token, scope } => json!({
                    "kind": "FactAdd",
                    "scope": scope_str(*scope),
                    "token": fact_ref_to_json(token),
                }),
                marque_scheme::ReplacementIntent::FactRemove { scope, facts } => {
                    let facts_json: Vec<serde_json::Value> =
                        facts.iter().map(fact_ref_to_json).collect();
                    json!({
                        "kind": "FactRemove",
                        "scope": scope_str(*scope),
                        "facts": facts_json,
                    })
                }
                marque_scheme::ReplacementIntent::Recanonicalize { scope } => json!({
                    "kind": "Recanonicalize",
                    "scope": recanon_scope_str(*scope),
                }),
                _ => json!({"kind": "Unknown"}),
            };
            json!({"kind": "FixIntent", "intent": kind_obj})
        }
        marque_rules::AppliedFixProposal::TextCorrection { replacement } => json!({
            "kind": "TextCorrection",
            "replacement": replacement.as_ref(),
        }),
    };
    let mut record = json!({
        "schema": marque_engine::AUDIT_SCHEMA_VERSION,
        "rule": fix.rule.as_str(),
        "source": source_str,
        "span": {
            "start": fix.span.start,
            "end": fix.span.end,
        },
        "proposal": proposal_json,
        "confidence": fix.confidence.combined(),
        "migration_ref": fix.migration_ref,
        "timestamp": humantime::format_rfc3339(fix.timestamp).to_string(),
        "classifier_id": fix.classifier_id.as_ref().map(|s| s.as_ref()),
        "dry_run": fix.dry_run,
        "input": fix.input.as_ref().map(|s| s.as_ref()),
    });

    if marque_engine::AUDIT_SCHEMA_IS_V3 {
        let c = &fix.confidence;
        let object = record.as_object_mut().expect("record is a JSON object");
        object.insert("recognition".to_owned(), json!(c.recognition));
        if let Some(r) = c.runner_up_ratio {
            object.insert("runner_up_ratio".to_owned(), json!(r));
        }
        if !c.features.is_empty() {
            let features: Vec<serde_json::Value> = c
                .features
                .iter()
                .map(|f| json!({"id": f.id.as_str(), "delta": f.delta}))
                .collect();
            object.insert("features".to_owned(), json!(features));
        }
    }
    record
}

#[test]
fn audit_record_snapshot_e002_apply() {
    // Post-PR-3c.B-Commit-6: E001 retired; E002 (REL TO missing USA)
    // is now the canonical "high-confidence single fix" fixture.
    let engine = test_engine();
    let source = b"SECRET//REL TO GBR\n";
    let result = engine.fix(source, FixMode::Apply);
    assert_eq!(result.applied.len(), 1);

    let json: Vec<serde_json::Value> = result.applied.iter().map(applied_fix_to_json).collect();
    // Snapshot is suffixed with the active audit schema so v1-downgrade
    // and v2-default builds maintain independent fixtures (FR-014: each
    // build emits exactly one schema, and the snapshot tracks that
    // schema's expected shape).
    insta::with_settings!({snapshot_suffix => marque_engine::AUDIT_SCHEMA_VERSION}, {
        insta::assert_json_snapshot!(json);
    });
}

#[test]
fn audit_record_snapshot_e002_dry_run() {
    let engine = test_engine();
    let source = b"SECRET//REL TO GBR\n";
    let result = engine.fix(source, FixMode::DryRun);
    assert_eq!(result.applied.len(), 1);

    let json: Vec<serde_json::Value> = result.applied.iter().map(applied_fix_to_json).collect();
    insta::with_settings!({snapshot_suffix => marque_engine::AUDIT_SCHEMA_VERSION}, {
        insta::assert_json_snapshot!(json);
    });
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

    let e002_applied: Vec<_> = result
        .applied
        .iter()
        .filter(|f| f.rule.as_str() == "E002")
        .collect();
    assert_eq!(
        e002_applied.len(),
        1,
        "E002 must apply once: {:?}",
        result.applied
    );

    let fixed_text = String::from_utf8(result.source).unwrap();
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

    let fixed_text = String::from_utf8(result.source).unwrap();
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

    let fixed_text = String::from_utf8(result.source).unwrap();
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
    let e002_applied: Vec<_> = result
        .applied
        .iter()
        .filter(|f| f.rule.as_str() == "E002")
        .collect();
    assert!(
        e002_applied.is_empty(),
        "E002 must not apply a fix across multiple REL TO blocks: \
         {e002_applied:?}"
    );

    // Intermediate `//NF//` content must survive in the output. Some
    // other rules may still rewrite other parts of the source (e.g.,
    // normalizing), so we only assert that NF is preserved.
    let fixed_text = String::from_utf8(result.source).unwrap();
    assert!(
        fixed_text.contains("NF") || fixed_text.contains("NOFORN"),
        "intermediate NF content must survive multi-block scenario: \
         {fixed_text:?}"
    );
}
