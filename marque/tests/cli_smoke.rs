// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 3 — CLI smoke tests for `marque check`.
//!
//! Spawns the compiled binary against the canonical corpus fixtures and
//! asserts stdout, stderr, and exit code match `contracts/cli.md`.

use assert_cmd::Command;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is the marque/ binary crate. Walk up one level.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().expect("workspace root").to_path_buf()
}

fn fixture(rel: &str) -> PathBuf {
    workspace_root().join("tests/corpus").join(rel)
}

fn marque() -> Command {
    Command::cargo_bin("marque").expect("marque binary")
}

#[test]
fn check_clean_banner_exits_zero() {
    marque()
        .args(["check", "--format", "json"])
        .arg(fixture("valid/clean_banner_top_secret.txt"))
        .assert()
        .success();
}

#[test]
fn check_invalid_banner_exits_one_with_e001() {
    let assert = marque()
        .args(["check", "--format", "json"])
        .arg(fixture("invalid/banner_abbrev.txt"))
        .assert()
        .code(1);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"rule\":\"E001\""), "got: {stdout}");
    assert!(stdout.contains("\"span\""), "got: {stdout}");
}

#[test]
fn check_unknown_token_fixture_fires_e008() {
    let assert = marque()
        .args(["check", "--format", "json"])
        .arg(fixture("invalid/unknown_token.txt"))
        .assert()
        .code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("\"rule\":\"E008\""));
}

#[test]
fn check_stdin_dash_sentinel_works() {
    let assert = marque()
        .args(["check", "--format", "json", "-"])
        .write_stdin("TOP SECRET//SI//NF\n")
        .assert()
        .code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("\"rule\":\"E001\""));
}

#[test]
fn check_stdin_default_when_no_paths() {
    let assert = marque()
        .args(["check", "--format", "json"])
        .write_stdin("TOP SECRET//SI//NF\n")
        .assert()
        .code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("\"rule\":\"E001\""));
}

#[test]
fn explain_config_emits_json_and_exits_zero() {
    let assert = marque()
        .args(["check", "--explain-config"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("\"confidence_threshold\""));
    assert!(stdout.contains("\"schema_version\""));
    assert!(stdout.contains("\"classifier_id_present\""));
}

#[test]
fn explain_config_never_leaks_classifier_id_value() {
    // Set MARQUE_CLASSIFIER_ID to a recognizable value and verify the
    // literal value never appears in --explain-config output, only the
    // boolean presence flag.
    let assert = marque()
        .args(["check", "--explain-config"])
        .env("MARQUE_CLASSIFIER_ID", "test-classifier-id-99999")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains("test-classifier-id-99999"),
        "--explain-config must NEVER emit the classifier_id value, got: {stdout}"
    );
    assert!(
        stdout.contains("\"classifier_id_present\": true")
            || stdout.contains("\"classifier_id_present\":true")
    );
}

#[test]
fn explain_config_with_paths_exits_64() {
    marque()
        .args(["check", "--explain-config"])
        .arg(fixture("valid/clean_banner_top_secret.txt"))
        .assert()
        .code(64);
}

#[test]
fn explain_config_emits_corrections_keys_not_count() {
    // D.2: `--explain-config` must emit the sorted list of corrections
    // keys per contracts/cli.md "corrections-map keys", not a count.
    let assert = marque()
        .args(["check", "--explain-config"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("\"corrections\""),
        "--explain-config must emit a `corrections` array, got: {stdout}"
    );
    assert!(
        !stdout.contains("corrections_count"),
        "--explain-config must NOT emit corrections_count (deprecated), got: {stdout}"
    );
}

#[test]
fn verbose_flag_does_not_break_invocation() {
    // D.3: `-v` must be parsed and wired to the tracing subscriber. We
    // can't easily assert the log-level effect in an integration test,
    // but we can assert the flag doesn't error and the subcommand runs
    // to completion.
    marque()
        .args(["check", "-v", "--format", "json"])
        .arg(fixture("valid/clean_banner_top_secret.txt"))
        .assert()
        .success();
}

#[test]
fn no_color_env_var_suppresses_ansi() {
    // With NO_COLOR set, the human format must not contain ANSI escapes.
    let assert = marque()
        .args(["check", "--format", "human"])
        .arg(fixture("invalid/banner_abbrev.txt"))
        .env("NO_COLOR", "1")
        .assert()
        .code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.contains("\x1b["),
        "NO_COLOR must suppress ANSI, got: {stdout:?}"
    );
}
