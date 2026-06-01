// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `marque-3.0` audit-record byte-identity parity.
//!
//! The CLI's `marque::render::render_audit_line` and the WASM crate's
//! `audit_line_to_json_v1_0` MUST produce byte-identical NDJSON for
//! every `AuditLine<CapcoScheme>` value the engine emits. This test
//! exercises the WASM-side projection through every variant of the
//! v3.0 shape — strict / decoder discriminants, AppliedFix /
//! TextCorrection arms, optional-field null-emit, MessageArgs
//! partial-emit — and validates the contract-shape invariants.
//!
//! The `marque-3.0` schema carries the 2-tuple `RuleId` shape: the
//! `rule` field on the wire is a structured `{scheme, predicate_id}`
//! object.
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
//! focuses on the v3.0 shape's structural correctness across the
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
    EnginePromotionToken, FeatureId, FixIntent, FixSource, Message, MessageArgs, MessageTemplate,
    Recognition, RuleId, Severity,
};
use marque_scheme::canonical::{Canonical, CanonicalConstructor, EngineConstructor};
use marque_scheme::fix_intent::RecanonScope;
use marque_scheme::{CategoryId, ReplacementIntent, Scope};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

// `RuleId` constants used across the parity tests, defined at module
// scope so each test reads as a one-liner like
// `synth_applied_fix(RULE_E002, ...)` rather than re-spelling the
// tuple at every call site. `RuleId` is `Copy` so passing by value
// has no cost.
//
// Reserved-scheme note: the decoder-recognition diagnostic's canonical
// 2-tuple is `("engine", "recognition.decoder-recognized")`. The parity
// fixture below preserves the engine-scheme identity since the audit
// record's provenance routes through the `Discriminant::Decoder` arm
// regardless of which `(scheme, predicate_id)` carries it.
const RULE_E002: RuleId = RuleId::new("capco", "portion.dissem.rel-to-missing-usa");
const RULE_E006: RuleId = RuleId::new("capco", "marking.deprecation.deprecated-dissem-control");
const RULE_R001_DECODER: RuleId = RuleId::new("engine", "recognition.decoder-recognized");

/// [`MessageTemplate`] for `rule`'s **Recanonicalize-branch** fix in the
/// synthetic parity-corpus fixtures.
///
/// Every fixture here is built via [`make_recanonicalize_intent`] — a
/// `ReplacementIntent::Recanonicalize` fix — so this helper returns the
/// template that branch carries per rule, populating the synthetic
/// `AppliedFix.message.template` to mirror the `parity_corpus.json` rows.
///
/// A `RuleId` alone does NOT determine a rule's fix template in general —
/// the template is branch-dependent. E002 is the clear case: its
/// Recanonicalize (USA-not-first reorder) branch carries `NonCanonicalOrder`
/// (returned here), while its USA-missing `FactAdd` branch carries
/// `RequiredByPresence` (see `crates/capco/src/rules/rel_to.rs`'s
/// `MissingUsaTrigraphRule`; that branch is exercised by the engine-side
/// `audit_completeness` test, not modeled here). Issue #709 fixed the prior
/// bug where every fixture hardcoded `BannerRollupMismatch` regardless of rule.
///
/// Production sources of truth:
///
/// - `RULE_E002` (`portion.dissem.rel-to-missing-usa`) →
///   `NonCanonicalOrder` — see `crates/capco/src/rules/rel_to.rs`
///   (and `parity_corpus.json` rows for the same predicate id).
/// - `RULE_E006` (`marking.deprecation.deprecated-dissem-control`) →
///   `SupersededToken` — see `crates/capco/src/rules/dissem.rs`.
/// - `RULE_R001_DECODER` (`recognition.decoder-recognized`) →
///   `DecoderRecognized` — see
///   `marque_capco::build_decoder_diagnostic`.
fn template_for_rule(rule: RuleId) -> MessageTemplate {
    if rule == RULE_E002 {
        MessageTemplate::NonCanonicalOrder
    } else if rule == RULE_E006 {
        MessageTemplate::SupersededToken
    } else if rule == RULE_R001_DECODER {
        MessageTemplate::DecoderRecognized
    } else {
        panic!(
            "template_for_rule: unmapped rule {rule}; \
             add the production-side MessageTemplate mapping here"
        );
    }
}

/// Build the synthetic `FixIntent<CapcoScheme>` for a Recanonicalize fix
/// carrying the production-side `MessageTemplate` for `rule`.
///
/// Each rule carries its own production template via
/// [`template_for_rule`] (deprecation → `SupersededToken`,
/// decoder-recognition → `DecoderRecognized`, etc.) rather than a single
/// hardcoded template, so the synthetic audit records are not mislabeled
/// (issue #709).
fn make_recanonicalize_intent(rule: RuleId) -> FixIntent<CapcoScheme> {
    FixIntent {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
            prior: None,
        },
        confidence: Recognition::strict(),
        feature_ids: Default::default(),
        message: Message::new(template_for_rule(rule), MessageArgs::default()),
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
/// The `rule` parameter is a constructed [`RuleId`] (2-tuple
/// `(scheme, predicate_id)`) so each call site shows the structured
/// shape explicitly.
fn synth_applied_fix(
    rule: RuleId,
    source: FixSource,
    classifier_id: Option<Arc<str>>,
    dry_run: bool,
    input: Option<Arc<str>>,
) -> AuditAppliedFix<CapcoScheme> {
    let mut intent = make_recanonicalize_intent(rule);
    intent.source = source;
    // Build canonical via EngineConstructor (the open-vocab path the
    // engine uses at promotion). CategoryId::MARKING since the intent
    // is Recanonicalize (whole-marking scope).
    //
    // Test-fixture carve-out per Constitution V Principle V — synthetic
    // fixture builder for the parity test; never reaches an engine
    // audit stream.
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
        // Corrections-map typo fix predicate.
        RuleId::new("capco", "marking.correction.token-typo"),
        Severity::Fix,
        Span::new(0, 6),
        original_digest,
        "SECRET".into(),
        FixSource::CorrectionsMap,
        Recognition::strict(),
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
    // FixSource::BuiltinRule routes to Discriminant::Strict.
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
    // Structured-object `rule` shape on the wire.
    assert_eq!(v["rule"]["scheme"], "capco");
    assert_eq!(
        v["rule"]["predicate_id"],
        "portion.dissem.rel-to-missing-usa"
    );
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
    // FixSource::MigrationTable also maps to "strict".
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
    // PR B retired `rule`, `region`, and `combined` from the wire
    // shape — strict-path emissions are pinned at `recognition = 1.0`,
    // the decoder uses span info elsewhere, and `combined` was a
    // tautology after the axis collapse (`combined == recognition`).
    // `Recognition::combined()` stays as an engine-internal method
    // for threshold gates but is no longer projected onto the wire.
    assert_eq!(confidence["recognition"], 1.0);
    assert!(
        confidence.get("rule").is_none(),
        "PR B retired the rule axis; field must not appear on the wire"
    );
    assert!(
        confidence.get("region").is_none(),
        "PR B retired the region field; must not appear on the wire"
    );
    assert!(
        confidence.get("combined").is_none(),
        "PR B retired the combined wire field (tautology with \
         recognition post-axis-collapse); field must not appear"
    );
    // Recognition::strict produces runner_up_ratio = None;
    // serde emits as explicit null.
    assert!(confidence["runner_up_ratio"].is_null());
    // Default Recognition::strict's features SmallVec is empty.
    assert_eq!(
        confidence["features"].as_array().unwrap().len(),
        0,
        "Recognition::strict has empty features"
    );
}

#[test]
fn applied_fix_message_default_args_emit_empty_map() {
    let fix = synth_applied_fix(RULE_E002, FixSource::BuiltinRule, None, false, None);

    let line = AuditLine::AppliedFix(fix);
    let v = project(&line);
    let message = &v["message"];
    // This fixture models E002's Recanonicalize (USA-not-first) branch,
    // whose fix template is `NonCanonicalOrder` (see `template_for_rule`
    // above + the `parity_corpus.json` E002 row). E002's other branch
    // (USA-missing `FactAdd`) carries `RequiredByPresence` and is not
    // modeled here. Pre-issue-#709 this asserted "BannerRollupMismatch"
    // because `make_recanonicalize_intent` hardcoded that template,
    // baking the bug into the parity-test golden.
    assert_eq!(message["template"], "NonCanonicalOrder");
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
    // The intent.message is overwritten below to exercise the
    // populated-args round-trip path on a `SupersededToken` template;
    // the seed-template choice (E006's) is internally consistent with
    // the fixture's `RULE_E006` rule id, but unused after the rebind.
    let mut intent = make_recanonicalize_intent(RULE_E006);
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
    // this parity test and never reach an engine audit stream.
    let constructor = EngineConstructor::<CapcoScheme>::__engine_construct();
    let canonical: Canonical<CapcoScheme> =
        constructor.build_open_vocab(CategoryId::MARKING, Box::from("(S)"), Scope::Portion);
    // Test-fixture carve-out per Constitution V Principle V (continued).
    let token = EnginePromotionToken::__engine_construct();
    // Test-fixture carve-out per Constitution V Principle V (continued).
    let fix = AuditAppliedFix::<CapcoScheme>::__engine_promote(
        // See `RULE_E006` const above.
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
    // Structured-object `rule` shape on the wire.
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
    // top-level `type` field.
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
    // Constitution V Principle V — audit content-ignorance. The audit
    // record's original_digest field MUST be "blake3:<hex>"; bytes
    // themselves never appear in the audit output.
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
