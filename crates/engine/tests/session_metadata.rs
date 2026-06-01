// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Session-level audit metadata + `require_signature` gate (issue #399).
//!
//! Exercises the engine-side behavior added in `marque-3.2`:
//!
//! - Every `FixResult` carries a `SessionMetadata` with the engine /
//!   lattice / decoder versions, the active audit schema, a `blake3:`
//!   integrity seal, the applying interface, the resolved classifier
//!   identity, and an optional carry-only signature.
//! - A per-call `FixOptions` identity override beats the engine
//!   `Config` — both in the metadata record AND in the per-record
//!   `AppliedFix.classifier_id`.
//! - The `require_signature` config gate refuses `fix_with_options`
//!   when no signature is supplied, and admits it when one is.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{CapcoEngine, EngineError, FixMode, FixOptions, InterfaceCode};
use marque_rules::audit::AuditLine;

/// Input that reliably produces at least one applied fix (REL TO
/// missing the USA trigraph → the engine injects `USA` and
/// canonicalizes).
const FIXING_INPUT: &[u8] = b"SECRET//REL TO GBR\n";

fn engine_with(config: Config) -> CapcoEngine {
    CapcoEngine::new(
        config,
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn session_metadata_carries_versions_and_seal() {
    let engine = engine_with(Config::default());
    let result = engine
        .fix_with_options(FIXING_INPUT, FixMode::Apply, &FixOptions::default())
        .expect("fix succeeds");

    let meta = &result.session_metadata;
    assert_eq!(meta.marque_version, marque_engine::MARQUE_VERSION);
    assert_eq!(meta.audit_schema, marque_engine::AUDIT_SCHEMA_VERSION);
    assert_eq!(meta.lattice_version.as_str(), marque_capco::LATTICE_VERSION);
    assert_eq!(meta.decoder_version, marque_engine::DECODER_VERSION);
    // Default FixOptions → `Other` interface, no identity / signature.
    assert_eq!(meta.interface, InterfaceCode::Other);
    assert!(meta.classifier_id.is_none());
    assert!(meta.classification_authority.is_none());
    assert!(meta.signature.is_none());

    let seal = meta.seal();
    assert!(seal.starts_with("blake3:"));
    assert_eq!(seal.len(), "blake3:".len() + 64);

    // NDJSON projection is well-formed and content-free.
    let v: serde_json::Value = serde_json::from_str(&meta.to_ndjson()).unwrap();
    assert_eq!(v["type"], "session_metadata");
    assert_eq!(v["schema"], marque_engine::AUDIT_SCHEMA_VERSION);
    assert_eq!(v["interface"], "O");
}

#[test]
fn interface_code_flows_into_metadata() {
    let engine = engine_with(Config::default());
    for (code, wire) in [
        (InterfaceCode::Server, "S"),
        (InterfaceCode::Cli, "C"),
        (InterfaceCode::Wasm, "W"),
        (InterfaceCode::Other, "O"),
    ] {
        let mut opts = FixOptions::default();
        opts.interface = code;
        let result = engine
            .fix_with_options(FIXING_INPUT, FixMode::Apply, &opts)
            .expect("fix succeeds");
        assert_eq!(result.session_metadata.interface, code);
        let v: serde_json::Value =
            serde_json::from_str(&result.session_metadata.to_ndjson()).unwrap();
        assert_eq!(v["interface"], wire);
    }
}

#[test]
fn per_call_identity_override_beats_config() {
    // Config sets one identity; the per-call FixOptions override must
    // win — both in the metadata record AND in every per-record
    // AppliedFix.classifier_id.
    let mut config = Config::default();
    config.user.classifier_id = Some("config-id".to_owned());
    config.user.classification_authority = Some("config-authority".to_owned());
    let engine = engine_with(config);

    let mut opts = FixOptions::default();
    opts.classifier_id = Some("override-id".to_owned());
    opts.classification_authority = Some("override-authority".to_owned());
    let result = engine
        .fix_with_options(FIXING_INPUT, FixMode::Apply, &opts)
        .expect("fix succeeds");

    assert_eq!(
        result.session_metadata.classifier_id.as_deref(),
        Some("override-id"),
    );
    assert_eq!(
        result.session_metadata.classification_authority.as_deref(),
        Some("override-authority"),
    );

    let mut saw_fix = false;
    for line in &result.audit_lines {
        if let AuditLine::AppliedFix(fix) = line {
            saw_fix = true;
            assert_eq!(
                fix.classifier_id.as_deref(),
                Some("override-id"),
                "per-record classifier_id must reflect the FixOptions override, not config"
            );
        }
    }
    assert!(
        saw_fix,
        "the fixing input must produce ≥1 AppliedFix record"
    );
}

#[test]
fn identity_falls_back_to_config_when_no_override() {
    let mut config = Config::default();
    config.user.classifier_id = Some("config-id".to_owned());
    config.user.classification_authority = Some("config-authority".to_owned());
    let engine = engine_with(config);

    let result = engine
        .fix_with_options(FIXING_INPUT, FixMode::Apply, &FixOptions::default())
        .expect("fix succeeds");
    assert_eq!(
        result.session_metadata.classifier_id.as_deref(),
        Some("config-id"),
    );
    assert_eq!(
        result.session_metadata.classification_authority.as_deref(),
        Some("config-authority"),
    );
}

#[test]
fn require_signature_refuses_fix_without_signature() {
    let mut config = Config::default();
    config.require_signature = true;
    let engine = engine_with(config);

    let err = engine
        .fix_with_options(FIXING_INPUT, FixMode::Apply, &FixOptions::default())
        .expect_err("require_signature with no signature must be refused");
    assert!(matches!(err, EngineError::SignatureRequired));
}

#[test]
fn require_signature_admits_fix_with_signature() {
    let mut config = Config::default();
    config.require_signature = true;
    let engine = engine_with(config);

    let mut opts = FixOptions::default();
    opts.signature = Some("detached-sig".to_owned());
    let result = engine
        .fix_with_options(FIXING_INPUT, FixMode::Apply, &opts)
        .expect("a supplied signature satisfies the gate");
    assert_eq!(
        result.session_metadata.signature.as_deref(),
        Some("detached-sig"),
        "the carry-only signature is stamped into the metadata record"
    );
}

#[test]
fn signature_is_carried_into_metadata_ndjson() {
    let engine = engine_with(Config::default());
    let mut opts = FixOptions::default();
    opts.signature = Some("sig-token".to_owned());
    let result = engine
        .fix_with_options(FIXING_INPUT, FixMode::Apply, &opts)
        .expect("fix succeeds");
    let v: serde_json::Value = serde_json::from_str(&result.session_metadata.to_ndjson()).unwrap();
    assert_eq!(v["signature"], "sig-token");
}

#[test]
fn legacy_fix_bypasses_require_signature_gate() {
    // `Engine::fix` (the back-compat shim) calls `fix_inner` directly,
    // so it is not subject to the `require_signature` gate — it carries
    // no signature by construction. This documents the intentional
    // boundary: production surfaces use `fix_with_options`.
    let mut config = Config::default();
    config.require_signature = true;
    let engine = engine_with(config);
    // Must not panic despite require_signature being set.
    let _ = engine.fix(FIXING_INPUT, FixMode::Apply);
}
