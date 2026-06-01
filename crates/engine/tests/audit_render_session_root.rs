// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Engine-side audit NDJSON projection + session-root integration
//! (issue #184, `marque-3.1`).
//!
//! Exercises `marque_engine::audit_render` (the projection the server's
//! `/v1/fix` session-root surface uses) end-to-end: drive `Engine::fix`,
//! serialize every `AuditLine` via [`audit_line_to_ndjson`], compute a
//! [`SessionRoot`] over those exact bytes, and prove the root round-trips
//! and detects tampering. This is the engine-side analogue of the CLI's
//! `marque/tests/session_root.rs`, and it covers the `audit_render`
//! module that the CLI test (running a separate bin) cannot reach.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{CapcoEngine, FixMode, FixedClock, SessionRoot, audit_line_to_ndjson};
use std::collections::HashMap;
use std::time::{Duration, UNIX_EPOCH};

/// Fixed clock so the emitted audit records (and therefore the Merkle
/// root) are byte-reproducible across runs — the determinism property
/// the module documents.
const FIXED_TS: u64 = 1_700_000_000;

fn test_engine() -> CapcoEngine {
    engine_with_config(Config::default())
}

fn engine_with_config(config: Config) -> CapcoEngine {
    CapcoEngine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// Serialize a fix result's audit lines exactly as the server does, then
/// build the per-document session root over them.
fn audit_log_and_root(engine: &CapcoEngine, source: &[u8]) -> (Vec<String>, SessionRoot) {
    let result = engine.fix(source, FixMode::Apply);
    let scheme = engine.scheme();
    let lines: Vec<String> = result
        .audit_lines
        .iter()
        .map(|line| audit_line_to_ndjson(scheme, line))
        .collect();
    let root = SessionRoot::compute(&lines);
    (lines, root)
}

#[test]
fn audit_line_to_ndjson_emits_valid_json_records() {
    let engine = test_engine();
    // Triggers the REL-TO-missing-USA fix → at least one audit line.
    let (lines, _root) = audit_log_and_root(&engine, b"SECRET//REL TO GBR\n");
    assert!(!lines.is_empty(), "fixture should produce audit records");

    for line in &lines {
        let v: serde_json::Value =
            serde_json::from_str(line).expect("each audit line must be valid JSON");
        // Engine-side projection carries the closed record-type
        // discriminant and the active schema constant.
        let kind = v["type"].as_str().expect("record carries a `type`");
        assert!(
            kind == "applied_fix" || kind == "text_correction",
            "unexpected audit record type: {kind}"
        );
        assert_eq!(
            v["schema"],
            marque_engine::AUDIT_SCHEMA_VERSION,
            "every record's schema must match the build constant"
        );
        // `rule` is the 2-tuple `{scheme, predicate_id}`.
        assert!(v["rule"]["scheme"].is_string(), "rule.scheme present");
        assert!(
            v["rule"]["predicate_id"].is_string(),
            "rule.predicate_id present"
        );
    }
}

#[test]
fn session_root_round_trips_over_engine_audit_log() {
    let engine = test_engine();
    let (lines, root) = audit_log_and_root(&engine, b"SECRET//REL TO GBR\n");
    assert!(!lines.is_empty());
    assert_eq!(root.record_count, lines.len());
    assert!(
        SessionRoot::verify(&lines, &root.root),
        "the published root must verify against its own audit log"
    );
}

#[test]
fn session_root_is_reproducible_under_a_fixed_clock() {
    // Two independent engines on the same fixed clock must produce the
    // same audit bytes and therefore the same root.
    let (lines_a, root_a) = audit_log_and_root(&test_engine(), b"SECRET//REL TO GBR\n");
    let (lines_b, root_b) = audit_log_and_root(&test_engine(), b"SECRET//REL TO GBR\n");
    assert_eq!(lines_a, lines_b, "audit bytes must be reproducible");
    assert_eq!(
        root_a.root, root_b.root,
        "the Merkle root must be reproducible under a fixed clock"
    );
}

#[test]
fn tampering_with_the_engine_audit_log_breaks_the_root() {
    let engine = test_engine();
    let (lines, root) = audit_log_and_root(&engine, b"SECRET//REL TO GBR\n");
    assert!(!lines.is_empty());

    // Mutate one record byte → verification must fail.
    let mut tampered = lines.clone();
    tampered[0].push(' ');
    assert!(
        !SessionRoot::verify(&tampered, &root.root),
        "a mutated record must fail verification"
    );

    // Drop the last record → verification must fail.
    let mut truncated = lines.clone();
    truncated.pop();
    assert!(
        !SessionRoot::verify(&truncated, &root.root),
        "a deleted record must fail verification"
    );
}

#[test]
fn text_correction_arm_projects_and_round_trips() {
    // A corrections-map entry drives the C001 text-correction path, so
    // the emitted audit log exercises the `text_correction` arm of the
    // engine-side projection (the `applied_fix`-only fixtures above never
    // reach it). `(TS//SERCET//NF)` corrects SERCET→SECRET.
    let mut corrections = HashMap::new();
    corrections.insert("SERCET".to_owned(), "SECRET".to_owned());
    let mut config = Config::default();
    config.corrections = corrections;
    let engine = engine_with_config(config);

    let (lines, root) = audit_log_and_root(&engine, b"(TS//SERCET//NF)");
    assert!(
        !lines.is_empty(),
        "corrections-map fixture should produce audit records"
    );
    // At least one record must be a text_correction (the C001 path).
    let has_text_correction = lines.iter().any(|l| {
        serde_json::from_str::<serde_json::Value>(l)
            .ok()
            .and_then(|v| v["type"].as_str().map(|s| s == "text_correction"))
            .unwrap_or(false)
    });
    assert!(
        has_text_correction,
        "SERCET→SECRET correction must emit a text_correction record; got:\n{lines:#?}"
    );
    assert!(
        SessionRoot::verify(&lines, &root.root),
        "the root must verify over a text-correction audit log"
    );
}

#[test]
fn clean_input_yields_empty_audit_log_and_empty_marker_root() {
    let engine = test_engine();
    let (lines, root) = audit_log_and_root(&engine, b"nothing to mark here.\n");
    assert!(lines.is_empty(), "a clean input produces no audit records");
    assert_eq!(root.record_count, 0);
    // The empty-marker root is well-defined and verifiable.
    assert!(SessionRoot::verify(&lines, &root.root));
}
