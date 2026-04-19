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
    let tmp_dir = tempfile::tempdir().unwrap();
    let config_path = tmp_dir.path().join(".marque.toml");
    std::fs::write(
        &config_path,
        format!("[rules]\nE001 = \"warn\"\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"),
    )
    .unwrap();

    // SECRET//NF triggers E001 (banner abbreviation). With E001=warn,
    // the exit code should be 2 (warnings only) instead of 1 (errors).
    let assert = marque()
        .args(["check", "--format", "json", "--config"])
        .arg(&config_path)
        .write_stdin("SECRET//NF\n")
        .assert()
        .code(2); // Warnings exit code

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\"rule\":\"E001\""),
        "E001 should be present in diagnostics, got: {stdout}"
    );
    assert!(
        stdout.contains("\"severity\":\"warn\""),
        "diagnostic should have severity=warn per config override, got: {stdout}"
    );
}

#[test]
fn severity_override_off_suppresses_rule() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let config_path = tmp_dir.path().join(".marque.toml");
    std::fs::write(
        &config_path,
        format!("[rules]\nE001 = \"off\"\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"),
    )
    .unwrap();

    // SECRET//NF normally triggers E001. With E001=off, it should not appear.
    let assert = marque()
        .args(["check", "--format", "json", "--config"])
        .arg(&config_path)
        .write_stdin("SECRET//NF\n")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains("\"rule\":\"E001\""),
        "E001 should not fire when configured to off, got: {stdout}"
    );
}

// F-08: Layer 4 (CLI flag) overrides all other layers.
#[test]
fn cli_confidence_threshold_overrides_config() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let config_path = tmp_dir.path().join(".marque.toml");
    // Config sets threshold=0.5, which would auto-apply E003 (confidence 0.6).
    // CLI flag --confidence-threshold=0.99 should override, excluding E003.
    std::fs::write(
        &config_path,
        format!("confidence_threshold = 0.5\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"),
    )
    .unwrap();

    // With threshold=0.99, only fixes >= 0.99 confidence are applied.
    // E001 (confidence 1.0) is applied but E003 (confidence 0.6) is not.
    let assert = marque()
        .args(["fix", "--confidence-threshold", "0.99", "--config"])
        .arg(&config_path)
        .write_stdin("SECRET//NF\n")
        .assert()
        .success(); // E001 applied, no remaining errors

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(
        stdout.as_ref(),
        "SECRET//NOFORN\n",
        "CLI threshold override should still allow E001 (conf=1.0)"
    );
}

// -----------------------------------------------------------------------
// Classifier ID in audit records
// -----------------------------------------------------------------------

#[test]
fn classifier_id_env_var_appears_in_audit_ndjson() {
    let assert = marque()
        .env("MARQUE_CLASSIFIER_ID", "CLI-TEST-ID-77")
        .args(["fix"])
        .write_stdin("SECRET//NF\n")
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
        .write_stdin("SECRET//NF\n")
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
    // The audit record should contain C001 as the winning rule (FR-009)
    assert!(
        stderr.contains("\"rule\":\"C001\""),
        "corrections-map fix should produce C001 audit record, got: {stderr}"
    );
    assert!(
        stderr.contains("\"source\":\"CorrectionsMap\""),
        "corrections-map fix should cite CorrectionsMap source, got: {stderr}"
    );

    // Fixed output should have NOFORN
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(stdout.as_ref(), "SECRET//NOFORN\n");
}
