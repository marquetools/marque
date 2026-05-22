// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T088 / SC-008 — `marque-2.0` audit-record byte-identity parity.
//!
//! PR 3c.2.D / D5 binding constraint: the CLI's
//! `marque::render::render_audit_line` and the WASM crate's
//! `audit_line_to_json_v1_0` MUST produce byte-identical NDJSON for
//! every `AuditLine<CapcoScheme>` value the engine emits. This test
//! exercises the WASM-side projection through every variant of the
//! v2.0 shape — strict / decoder discriminants, AppliedFix /
//! TextCorrection arms, optional-field null-emit, MessageArgs
//! partial-emit — and validates the contract-shape invariants per
//! `specs/006-engine-rule-refactor/contracts/audit-record.md`
//! §107-178 (AppliedFix) + §388-402 (TextCorrection).
//!
//! T044: schema-version cutover `marque-1.0` → `marque-2.0` carries
//! the 2-tuple `RuleId` shape. The `rule` field on the wire is now
//! a structured `{scheme, predicate_id}` object per PM OD-2. The
//! file is renamed from `audit_v1_0_parity.rs` to track the schema
//! label.
//!
//! Test-fixture construction uses [`marque_rules::AppliedFix::__engine_promote`]
//! and [`marque_rules::audit::AppliedTextCorrection::__engine_promote_text_correction`]
//! under the Constitution V Principle V test-fixture carve-out
//! (`#[cfg(test)]` / integration-test only, never commingled with
//! engine output, only for renderer-input construction).
//!
//! The CLI-vs-WASM byte-identity at the production emit boundary is
//! verified end-to-end by `tests/parity.rs` and `tests/native_parity.rs`
//! which drive the same engine through both surfaces. This test
//! focuses on the v2.0 shape's structural correctness across the
//! audit-record variants.
//!
//! Native-only target — `target_arch = "wasm32"` cannot host this
//! test's `__engine_promote` calls (the engine's test-fixture helpers
//! are not gated for wasm32).

#![cfg(not(target_arch = "wasm32"))]

use marque_capco::CapcoScheme;
use marque_ism::Span;
use marque_rules::audit::{AppliedFix as AuditAppliedFix, AppliedTextCorrection, AuditLine};
use marque_rules::{
    Confidence, EnginePromotionToken, FeatureId, FixIntent, FixSource, Message, MessageArgs,
    MessageTemplate, RuleId, Severity,
};
use marque_scheme::canonical::{Canonical, CanonicalConstructor, EngineConstructor};
use marque_scheme::fix_intent::RecanonScope;
use marque_scheme::{CategoryId, ReplacementIntent, Scope};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

// T044: `RuleId` constants used across the parity tests. Pre-T044
// these were flat `&'static str` rule-ID labels (`"E002"`, `"R001"`,
// `"E006"`, `"C001"`); post-T044 they are 2-tuple values per
// `docs/refactor-006/legacy-rule-id-map.md`.
//
// Defined at module scope so each test reads as a one-liner like
// `synth_applied_fix(RULE_E002, ...)` rather than re-spelling the
// tuple at every call site. `RuleId` is `Copy` so passing by value
// has no cost.
//
// **Reserved-scheme note**: `R001` historically named an engine-minted
// decoder-recognition diagnostic; under T044 its canonical 2-tuple is
// `("engine", "recognition.decoder-recognized")` per the
// `legacy-rule-id-map.md` §7 reserved-scheme convention. The parity
// fixture below preserves the engine-scheme identity since the audit
// record's provenance routes through the `Discriminant::Decoder` arm
// regardless of which `(scheme, predicate_id)` carries it.
const RULE_E002: RuleId = RuleId::new("capco", "portion.dissem.rel-to-missing-usa");
const RULE_E006: RuleId = RuleId::new("capco", "marking.deprecation.deprecated-dissem-control");
const RULE_R001_DECODER: RuleId = RuleId::new("engine", "recognition.decoder-recognized");

/// Build the synthetic `FixIntent<CapcoScheme>` for a Recanonicalize fix.
fn make_recanonicalize_intent() -> FixIntent<CapcoScheme> {
    FixIntent {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
        },
        confidence: Confidence::strict(1.0),
        feature_ids: Default::default(),
        message: Message::new(
            MessageTemplate::BannerRollupMismatch,
            MessageArgs::default(),
        ),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    }
}

/// Construct a synthetic [`AuditAppliedFix`] (v2 marking-side audit
/// record). Constitution V Principle V test-fixture carve-out per
/// `__engine_promote`'s engine-only seal — test code MAY construct
/// synthetic audit-record fixtures inside integration-test contexts
/// to exercise renderers, never commingled with engine output.
///
/// T044: the `rule` parameter is a constructed [`RuleId`] (2-tuple
/// `(scheme, predicate_id)`) rather than a flat `&'static str` so each
/// call site shows the structured shape explicitly.
fn synth_applied_fix(
    rule: RuleId,
    source: FixSource,
    classifier_id: Option<Arc<str>>,
    dry_run: bool,
    input: Option<Arc<str>>,
) -> AuditAppliedFix<CapcoScheme> {
    let mut intent = make_recanonicalize_intent();
    intent.source = source;
    // Build canonical via EngineConstructor (the open-vocab path the
    // engine uses at promotion). CategoryId::MARKING since the intent
    // is Recanonicalize (whole-marking scope per PR 3c.2.D's
    // CategoryId resolution).
    //
    // Test-fixture carve-out per Constitution V Principle V — synthetic
    // fixture builder for the SC-008 parity test; never reaches an
    // engine audit stream.
    let constructor = EngineConstructor::<CapcoScheme>::__engine_construct();
    let canonical: Canonical<CapcoScheme> =
        constructor.build_open_vocab(CategoryId::MARKING, Box::from("(S)"), Scope::Portion);
    // Test-fixture carve-out per Constitution V Principle V.
    let token = EnginePromotionToken::__engine_construct();
    AuditAppliedFix::<CapcoScheme>::__engine_promote(
        rule,
        Severity::Fix,
        Span::new(8, 10),
        intent,
        b"(S)",
        canonical,
        UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        classifier_id,
        dry_run,
        input,
        token,
    )
}

/// Construct a synthetic [`AppliedTextCorrection`]. Constitution V
/// Principle V test-fixture carve-out as above.
fn synth_text_correction(
    classifier_id: Option<Arc<str>>,
    dry_run: bool,
    input: Option<Arc<str>>,
) -> AppliedTextCorrection {
    let original_digest = blake3::hash(b"SERCET");
    // Test-fixture carve-out per Constitution V Principle V.
    let token = EnginePromotionToken::__engine_construct();
    AppliedTextCorrection::__engine_promote_text_correction(
        // T044: `C001` → `("capco", "marking.correction.token-typo")`
        // per `docs/refactor-006/legacy-rule-id-map.md` §1.
        RuleId::new("capco", "marking.correction.token-typo"),
        Severity::Fix,
        Span::new(0, 6),
        original_digest,
        "SECRET".into(),
        FixSource::CorrectionsMap,
        Confidence::strict(1.0),
        None,
        Message::new(MessageTemplate::CorrectionsApplied, MessageArgs::default()),
        UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        classifier_id,
        dry_run,
        input,
        token,
    )
}

/// Run the WASM-side projection.
fn project(line: &AuditLine<CapcoScheme>) -> serde_json::Value {
    let scheme = marque_engine::default_scheme();
    marque_wasm::audit_line_to_json_v1_0(&scheme, line)
}

/// Top-level contract-shape gate per `contracts/audit-record.md`.
fn validate_contract_shape(value: &serde_json::Value, expected_type: &str) {
    assert_eq!(
        value["type"], expected_type,
        "audit-line type field mismatch"
    );
    assert_eq!(value["schema"], marque_engine::AUDIT_SCHEMA_VERSION);
    assert!(
        value["timestamp"].as_str().unwrap().contains('T'),
        "timestamp must be RFC3339"
    );
}

#[test]
fn applied_fix_strict_discriminant_full_context() {
    // FixSource::BuiltinRule routes to Discriminant::Strict per PM-D-7.
    let fix = synth_applied_fix(
        RULE_E002,
        FixSource::BuiltinRule,
        Some(Arc::from("classifier-42")),
        false,
        Some(Arc::from("test.txt")),
    );
    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    validate_contract_shape(&v, "applied_fix");
    // T044 PM OD-2: structured-object `rule` shape on the wire.
    assert_eq!(v["rule"]["scheme"], "capco");
    assert_eq!(v["rule"]["predicate_id"], "portion.dissem.rel-to-missing-usa");
    assert_eq!(v["severity"], "fix");
    assert_eq!(v["fix"]["replacement"]["discriminant"], "strict");
    assert_eq!(v["classifier_id"], "classifier-42");
    assert_eq!(v["dry_run"], false);
    assert_eq!(v["input"], "test.txt");
}

#[test]
fn applied_fix_decoder_discriminant_full_context() {
    // FixSource::DecoderPosterior routes to Discriminant::Decoder.
    let fix = synth_applied_fix(
        RULE_R001_DECODER,
        FixSource::DecoderPosterior,
        Some(Arc::from("classifier-42")),
        false,
        Some(Arc::from("decoder.txt")),
    );
    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    validate_contract_shape(&v, "applied_fix");
    assert_eq!(v["fix"]["replacement"]["discriminant"], "decoder");
}

#[test]
fn applied_fix_decoder_classification_heuristic_routes_to_decoder() {
    // FixSource::DecoderClassificationHeuristic also maps to "decoder".
    let fix = synth_applied_fix(
        RULE_R001_DECODER,
        FixSource::DecoderClassificationHeuristic,
        None,
        false,
        None,
    );
    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    assert_eq!(v["fix"]["replacement"]["discriminant"], "decoder");
}

#[test]
fn applied_fix_migration_table_routes_to_strict() {
    // FixSource::MigrationTable also maps to "strict" per PM-D-7.
    let fix = synth_applied_fix(RULE_E006, FixSource::MigrationTable, None, false, None);
    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    assert_eq!(v["fix"]["replacement"]["discriminant"], "strict");
}

#[test]
fn applied_fix_classifier_id_absent_emits_null() {
    let fix = synth_applied_fix(
        RULE_E002,
        FixSource::BuiltinRule,
        None,
        false,
        Some(Arc::from("test.txt")),
    );
    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    assert!(
        v["classifier_id"].is_null(),
        "absent classifier_id must emit as null for audit-consumer stability"
    );
}

#[test]
fn applied_fix_input_absent_emits_null() {
    let fix = synth_applied_fix(
        RULE_E002,
        FixSource::BuiltinRule,
        Some(Arc::from("classifier-42")),
        false,
        None,
    );
    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    assert!(
        v["input"].is_null(),
        "absent input must emit as null for audit-consumer stability"
    );
}

#[test]
fn applied_fix_dry_run_toggles() {
    for &dry in &[true, false] {
        let fix = synth_applied_fix(RULE_E002, FixSource::BuiltinRule, None, dry, None);
        let line = AuditLine::AppliedFix(fix);
        let v = project(&line);
        assert_eq!(
            v["dry_run"], dry,
            "dry_run boolean must round-trip identically; dry={dry}"
        );
    }
}

#[test]
fn applied_fix_strict_path_canonical_open_vocab_shape() {
    // The Recanonicalize fixture routes through CategoryId::MARKING →
    // open_vocab path. Validate the open_vocab arm carries category +
    // render_call_site + bytes_digest (no token_id).
    let fix = synth_applied_fix(RULE_E002, FixSource::BuiltinRule, None, false, None);

    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    let canonical = &v["fix"]["replacement"]["canonical"];
    assert_eq!(canonical["source"], "open_vocab");
    assert_eq!(canonical["category"], "Marking");
    assert!(
        canonical["bytes_digest"]
            .as_str()
            .unwrap()
            .starts_with("blake3:"),
        "bytes_digest must be 'blake3:<hex>'"
    );
    assert!(
        canonical["render_call_site"].as_str().is_some(),
        "open_vocab arm must carry render_call_site"
    );
    assert!(
        canonical.get("token_id").is_none(),
        "open_vocab arm must elide CVE-only token_id field"
    );
}

#[test]
fn applied_fix_confidence_round_trip() {
    let fix = synth_applied_fix(RULE_E002, FixSource::BuiltinRule, None, false, None);

    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    let confidence = &v["fix"]["replacement"]["confidence"];
    assert_eq!(confidence["recognition"], 1.0);
    assert_eq!(confidence["rule"], 1.0);
    assert_eq!(confidence["combined"], 1.0);
    // Confidence::strict produces region / runner_up_ratio = None;
    // serde emits as explicit null.
    assert!(confidence["region"].is_null());
    assert!(confidence["runner_up_ratio"].is_null());
    // Default Confidence::strict's features SmallVec is empty.
    assert_eq!(
        confidence["features"].as_array().unwrap().len(),
        0,
        "Confidence::strict has empty features"
    );
}

#[test]
fn applied_fix_message_default_args_emit_empty_map() {
    let fix = synth_applied_fix(RULE_E002, FixSource::BuiltinRule, None, false, None);

    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    let message = &v["message"];
    assert_eq!(message["template"], "BannerRollupMismatch");
    let args = message["args"].as_object().unwrap();
    assert!(
        args.is_empty(),
        "default MessageArgs emits an empty args map (partial-emit elision); got: {args:?}"
    );
}

#[test]
fn applied_fix_message_populated_args_round_trip() {
    // Populated MessageArgs — partial-emit covers token, expected_token,
    // and feature_ids in a single fixture. Per Constitution V Principle V,
    // every populated field belongs to the closed permitted-identifier set.
    let mut intent = make_recanonicalize_intent();
    // TokenId(100) → NOFORN, TokenId(111) → RD (both real CAPCO
    // tokens registered in `SENTINEL_TO_CANONICAL` —
    // `qualified_token_label` resolves them to namespaced labels).
    let args = MessageArgs {
        token: Some(marque_scheme::TokenId(100)),
        expected_token: Some(marque_scheme::TokenId(111)),
        feature_ids: marque_rules::smallvec![FeatureId::EditDistance1],
        ..MessageArgs::default()
    };
    intent.message = Message::new(MessageTemplate::SupersededToken, args);

    // Test-fixture carve-out per Constitution V Principle V — the
    // EngineConstructor / EnginePromotionToken / AuditAppliedFix mints
    // below fabricate the populated-MessageArgs round-trip fixture for
    // this SC-008 parity test and never reach an engine audit stream.
    let constructor = EngineConstructor::<CapcoScheme>::__engine_construct();
    let canonical: Canonical<CapcoScheme> =
        constructor.build_open_vocab(CategoryId::MARKING, Box::from("(S)"), Scope::Portion);
    // Test-fixture carve-out per Constitution V Principle V (continued).
    let token = EnginePromotionToken::__engine_construct();
    // Test-fixture carve-out per Constitution V Principle V (continued).
    let fix = AuditAppliedFix::<CapcoScheme>::__engine_promote(
        // T044: see `RULE_E006` const above.
        RULE_E006,
        Severity::Warn,
        Span::new(0, 5),
        intent,
        b"FOUO",
        canonical,
        UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        None,
        false,
        None,
        token,
    );

    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    assert_eq!(v["message"]["template"], "SupersededToken");
    let msg_args = v["message"]["args"].as_object().unwrap();
    assert!(msg_args.contains_key("token"));
    assert!(msg_args.contains_key("expected_token"));
    assert!(msg_args.contains_key("feature_ids"));
    let feature_array = msg_args["feature_ids"].as_array().unwrap();
    assert_eq!(feature_array.len(), 1);
    assert_eq!(feature_array[0], "EditDistance1");
}

#[test]
fn text_correction_arm_round_trip_full_context() {
    let tc = synth_text_correction(
        Some(Arc::from("classifier-42")),
        false,
        Some(Arc::from("test.txt")),
    );
    let line = AuditLine::TextCorrection(tc);
    let v = project(&line);
    validate_contract_shape(&v, "text_correction");
    // T044 PM OD-2: structured-object `rule` shape on the wire.
    assert_eq!(v["rule"]["scheme"], "capco");
    assert_eq!(v["rule"]["predicate_id"], "marking.correction.token-typo");
    assert_eq!(v["severity"], "fix");
    assert_eq!(v["replacement"], "SECRET");
    assert_eq!(v["source"], "CorrectionsMap");
    assert_eq!(v["classifier_id"], "classifier-42");
    assert_eq!(v["dry_run"], false);
    assert_eq!(v["input"], "test.txt");
}

#[test]
fn text_correction_arm_classifier_id_absent_emits_null() {
    let tc = synth_text_correction(None, false, Some(Arc::from("test.txt")));
    let line = AuditLine::TextCorrection(tc);
    let v = project(&line);
    assert!(
        v["classifier_id"].is_null(),
        "absent classifier_id must emit as null"
    );
}

#[test]
fn text_correction_arm_migration_ref_absent_emits_null() {
    let tc = synth_text_correction(None, false, None);
    let line = AuditLine::TextCorrection(tc);
    let v = project(&line);
    assert!(
        v["migration_ref"].is_null(),
        "absent migration_ref must emit as null"
    );
}

#[test]
fn text_correction_arm_dry_run_toggles() {
    for &dry in &[true, false] {
        let tc = synth_text_correction(None, dry, None);
        let line = AuditLine::TextCorrection(tc);
        let v = project(&line);
        assert_eq!(v["dry_run"], dry);
    }
}

#[test]
fn project_preserves_record_kind_dispatch() {
    // Both arms emit one NDJSON line each, distinguished by the
    // top-level `type` field. This is the dispatcher discipline per
    // contract §107 vs §388.
    let fix_line = AuditLine::AppliedFix(synth_applied_fix(
        RULE_E002,
        FixSource::BuiltinRule,
        None,
        false,
        None,
    ));
    let tc_line = AuditLine::TextCorrection(synth_text_correction(None, false, None));

    let fix_json = project(&fix_line);
    let tc_json = project(&tc_line);

    assert_eq!(fix_json["type"], "applied_fix");
    assert_eq!(tc_json["type"], "text_correction");
    assert_ne!(
        fix_json["type"], tc_json["type"],
        "the two arms MUST project to distinct top-level type values"
    );
}

#[test]
fn fix_record_carries_original_digest_blake3_prefix() {
    // Constitution V Principle V — G13 invariant. The audit record's
    // original_digest field MUST be "blake3:<hex>"; bytes themselves
    // never appear in the audit output.
    let fix = synth_applied_fix(RULE_E002, FixSource::BuiltinRule, None, false, None);

    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    let digest = v["fix"]["original_digest"].as_str().unwrap();
    assert!(
        digest.starts_with("blake3:"),
        "original_digest must be 'blake3:<hex>'; got: {digest}"
    );
    // BLAKE3 produces 32-byte / 64-hex-char digests.
    let hex_part = &digest["blake3:".len()..];
    assert_eq!(hex_part.len(), 64, "BLAKE3 hex digest length must be 64");
}

#[test]
fn text_correction_carries_original_digest_blake3_prefix() {
    let tc = synth_text_correction(None, false, None);
    let line = AuditLine::TextCorrection(tc);
    let v = project(&line);
    let digest = v["original_digest"].as_str().unwrap();
    assert!(digest.starts_with("blake3:"));
    let hex_part = &digest["blake3:".len()..];
    assert_eq!(hex_part.len(), 64);
}
