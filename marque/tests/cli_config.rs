// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 5 — CLI integration tests for configuration behavior.
//!
//! Tests `--explain-config`, severity overrides, classifier_id in audit,
//! and corrections-map wiring through the CLI.

use assert_cmd::Command;

fn marque() -> Command {
    Command::cargo_bin("marque").expect("marque binary")
}

/// The compiled schema version for config files.
const SCHEMA_VERSION: &str = marque_ism::generated::values::SCHEMA_VERSION;

// -----------------------------------------------------------------------
// --explain-config
// -----------------------------------------------------------------------

#[test]
fn explain_config_outputs_valid_json_and_exits_zero() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let config_path = tmp_dir.path().join(".marque.toml");
    std::fs::write(
        &config_path,
        format!(
            "[rules]\nE001 = \"warn\"\n\n[corrections]\nSERCET = \"SECRET\"\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"
        ),
    )
    .unwrap();

    let assert = marque()
        .args(["check", "--explain-config", "--config"])
        .arg(&config_path)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("--explain-config should produce valid JSON");

    // Verify expected fields (f32 round-trips through JSON as f64)
    let threshold = json["confidence_threshold"].as_f64().unwrap();
    assert!(
        (threshold - 0.95).abs() < 0.001,
        "confidence_threshold should be ~0.95, got {threshold}"
    );
    assert_eq!(json["schema_version"], SCHEMA_VERSION);
    assert_eq!(json["rules"]["E001"], "warn");
    let corrections = json["corrections"]
        .as_array()
        .expect("corrections must be an array");
    assert_eq!(
        corrections,
        &vec![serde_json::Value::String("SERCET".into())],
        "corrections should contain exactly [\"SERCET\"]"
    );
    // classifier_id_present should be false (no local config)
    assert_eq!(json["classifier_id_present"], false);
}

#[test]
fn explain_config_never_exposes_classifier_id_value() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let config_path = tmp_dir.path().join(".marque.toml");
    std::fs::write(
        &config_path,
        format!("[capco]\nversion = \"{SCHEMA_VERSION}\"\n"),
    )
    .unwrap();
    std::fs::write(
        tmp_dir.path().join(".marque.local.toml"),
        "[user]\nclassifier_id = \"SUPER-SECRET-99\"\n",
    )
    .unwrap();

    let assert = marque()
        .args(["check", "--explain-config", "--config"])
        .arg(&config_path)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    // The classifier_id value must NEVER appear in the output
    assert!(
        !stdout.contains("SUPER-SECRET-99"),
        "classifier_id value must never be exposed in --explain-config output"
    );
    // But classifier_id_present should be true
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["classifier_id_present"], true);
}

#[test]
fn explain_config_mutually_exclusive_with_paths() {
    marque()
        .args(["check", "--explain-config", "dummy.txt"])
        .assert()
        .code(64);
}

#[test]
fn explain_config_mutually_exclusive_with_fix() {
    marque()
        .args(["fix", "--explain-config"])
        .write_stdin("SECRET//NF\n")
        .assert()
        .code(64);
}

// -----------------------------------------------------------------------
// Severity override via config
// -----------------------------------------------------------------------

#[test]
fn severity_override_downgrades_rule_to_warn() {
    // T044: legacy `E002` TOML key → 2-tuple form. The config-key
    // canonicalizer in `Engine::new` looks up overrides by predicate-id
    // alone (`rule.id().predicate_id()`), so the active TOML key here
    // is the descriptive form. Map: `("capco", "portion.dissem.rel-to-missing-usa")`
    // per `docs/refactor-006/legacy-rule-id-map.md` §1.
    let tmp_dir = tempfile::tempdir().unwrap();
    let config_path = tmp_dir.path().join(".marque.toml");
    std::fs::write(
        &config_path,
        format!(
            "[rules]\n\"portion.dissem.rel-to-missing-usa\" = \"warn\"\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"
        ),
    )
    .unwrap();

    // SECRET//REL TO GBR triggers the rule formerly known as E002
    // (REL TO missing USA). With the warn override, exit code is 2.
    let assert = marque()
        .args(["check", "--format", "json", "--config"])
        .arg(&config_path)
        .write_stdin("SECRET//REL TO GBR\n")
        .assert()
        .code(2); // Warnings exit code

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    // T044 PM OD-2: the `rule` field on the wire is the structured
    // 2-tuple object.
    let expected_rule_fragment =
        r#""rule":{"scheme":"capco","predicate_id":"portion.dissem.rel-to-missing-usa"}"#;
    assert!(
        stdout.contains(expected_rule_fragment),
        "rel-to-missing-usa rule should be present in diagnostics, got: {stdout}"
    );
    assert!(
        stdout.contains("\"severity\":\"warn\""),
        "diagnostic should have severity=warn per config override, got: {stdout}"
    );
}

#[test]
fn severity_override_off_suppresses_rule() {
    // T044: see canonical map row above for the predicate-id form.
    let tmp_dir = tempfile::tempdir().unwrap();
    let config_path = tmp_dir.path().join(".marque.toml");
    std::fs::write(
        &config_path,
        format!(
            "[rules]\n\"portion.dissem.rel-to-missing-usa\" = \"off\"\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"
        ),
    )
    .unwrap();

    // SECRET//REL TO GBR normally triggers the rule. With the off
    // override, it should not appear.
    let assert = marque()
        .args(["check", "--format", "json", "--config"])
        .arg(&config_path)
        .write_stdin("SECRET//REL TO GBR\n")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains("portion.dissem.rel-to-missing-usa"),
        "rel-to-missing-usa rule should not fire when configured to off, got: {stdout}"
    );
}

// F-08: Layer 4 (CLI flag) overrides all other layers.
#[test]
fn cli_confidence_threshold_overrides_config() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let config_path = tmp_dir.path().join(".marque.toml");
    // Config sets threshold=0.5. CLI flag --confidence-threshold=0.99
    // should override and select only the >=0.99 fixes. E002's fix
    // has confidence 0.97 (below 0.99), so the fix should NOT be
    // applied; the diagnostic still surfaces.
    std::fs::write(
        &config_path,
        format!("confidence_threshold = 0.5\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"),
    )
    .unwrap();

    // (Post-PR-3c.B-Commit-6: this test originally validated that
    // E001's confidence=1.0 fix passed the 0.99 threshold and that
    // E003's 0.6 fix did not. E001 and E003 are both retired; the
    // channel now exercises E002 at confidence 0.97 — below 0.99 —
    // so no fix is applied at the higher threshold. The audit
    // stream remains empty.)
    let assert = marque()
        .args(["fix", "--confidence-threshold", "0.99", "--config"])
        .arg(&config_path)
        .write_stdin("SECRET//REL TO GBR\n")
        .assert()
        .code(1); // E002 diagnostic remains; fix not applied at 0.99 threshold

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(
        stdout.as_ref(),
        "SECRET//REL TO GBR\n",
        "with --confidence-threshold=0.99, no E002 fix (0.97) applies"
    );
}

// -----------------------------------------------------------------------
// Classifier ID in audit records
// -----------------------------------------------------------------------

#[test]
fn classifier_id_env_var_appears_in_audit_ndjson() {
    // (Post-PR-3c.B-Commit-6 fixture: E001 used to drive this; with
    // E001 retired we use E002, whose 0.97-confidence fix passes the
    // default 0.85 threshold and produces an audit record.)
    let assert = marque()
        .env("MARQUE_CLASSIFIER_ID", "CLI-TEST-ID-77")
        .args(["fix"])
        .write_stdin("SECRET//REL TO GBR\n")
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("\"classifier_id\":\"CLI-TEST-ID-77\""),
        "audit NDJSON should contain classifier_id from env var, got: {stderr}"
    );
}

#[test]
fn absent_classifier_id_is_null_in_audit_ndjson() {
    let assert = marque()
        .args(["fix"])
        .write_stdin("SECRET//REL TO GBR\n")
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("\"classifier_id\":null"),
        "absent classifier_id should be null in audit NDJSON, got: {stderr}"
    );
}

// -----------------------------------------------------------------------
// Corrections map via config
// -----------------------------------------------------------------------

#[test]
fn corrections_map_fires_c001_in_fix() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let config_path = tmp_dir.path().join(".marque.toml");
    // Add a corrections entry: NF → NOFORN (same as E001, to test FR-009)
    std::fs::write(
        &config_path,
        format!("[corrections]\nNF = \"NOFORN\"\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"),
    )
    .unwrap();

    let assert = marque()
        .args(["fix", "--config"])
        .arg(&config_path)
        .write_stdin("SECRET//NF\n")
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    // The audit record should contain the C001 corrections-map
    // predicate as the winning rule (FR-009). T044 PM OD-2: the
    // `rule` field is a structured 2-tuple object on the wire, not
    // a flat string. Legacy `C001` →
    // `("capco", "marking.correction.token-typo")` per
    // `docs/refactor-006/legacy-rule-id-map.md` §1. The serializer
    // emits the object's keys in alphabetical order — `predicate_id`
    // before `scheme` — so the literal fragment below matches the
    // shape the wire actually produces (verified against the failing
    // test's pre-fix output).
    let expected_rule_fragment =
        r#""rule":{"predicate_id":"marking.correction.token-typo","scheme":"capco"}"#;
    assert!(
        stderr.contains(expected_rule_fragment),
        "corrections-map fix should produce the C001 predicate-id in its audit record, got: {stderr}"
    );
    assert!(
        stderr.contains("\"source\":\"CorrectionsMap\""),
        "corrections-map fix should cite CorrectionsMap source, got: {stderr}"
    );

    // Fixed output should have NOFORN
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(stdout.as_ref(), "SECRET//NOFORN\n");
}
