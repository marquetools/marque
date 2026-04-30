// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CLI integration tests for `--corpus-override` (Phase 4 PR-5, T065).
//!
//! `assert_cmd` builds the `marque` binary at test runtime against the
//! features active in the workspace. This file is gated on
//! `#[cfg(feature = "corpus-override")]` so the tests only run when
//! the binary was built with the feature available; without the
//! feature, `--corpus-override` doesn't exist on the CLI surface
//! (clap rejects it as an unknown flag — see
//! `cli_corpus_override_absent_without_feature` below for the
//! mirror-image gating that fires in default builds).

use assert_cmd::Command;

fn marque() -> Command {
    Command::cargo_bin("marque").expect("marque binary")
}

#[cfg(feature = "corpus-override")]
const VALID_OVERRIDE: &str = r#"{
    "schema_version": "marque-corpus-override-1",
    "token_overrides": { "SECRET": { "log_prior": -2.5 } }
}"#;

#[cfg(feature = "corpus-override")]
const INVALID_OVERRIDE_BAD_SCHEMA: &str = r#"{
    "schema_version": "marque-corpus-override-99"
}"#;

#[cfg(feature = "corpus-override")]
#[test]
fn corpus_override_with_bad_schema_errors_ex_dataerr() {
    let tmp = tempfile::tempdir().unwrap();
    let override_path = tmp.path().join("override.json");
    std::fs::write(&override_path, INVALID_OVERRIDE_BAD_SCHEMA).unwrap();

    let assert = marque()
        .args(["check", "--corpus-override"])
        .arg(&override_path)
        .write_stdin("UNCLASSIFIED")
        .assert()
        .code(65); // EX_DATAERR

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("schema_version") || stderr.contains("schema"),
        "expected schema-version mismatch in error, got: {stderr}"
    );
}

#[cfg(feature = "corpus-override")]
#[test]
fn corpus_override_with_valid_json_runs_successfully() {
    let tmp = tempfile::tempdir().unwrap();
    let override_path = tmp.path().join("override.json");
    std::fs::write(&override_path, VALID_OVERRIDE).unwrap();

    // Plain unclassified text — no markings → no diagnostics → exit 0.
    // The point is to verify the override loads and the engine
    // accepts it; behavioral coverage of the audit annotation lives
    // in `crates/engine/tests/corpus_override.rs`.
    //
    // The decoder fallback is the engine default, so the override
    // takes effect without any opt-in flag. (Pre-PR #259 this test
    // also passed `--deep-scan`; that flag is now retired.)
    marque()
        .args(["check", "--corpus-override"])
        .arg(&override_path)
        .write_stdin("UNCLASSIFIED text only.")
        .assert()
        .code(0);
}

#[cfg(feature = "corpus-override")]
#[test]
fn corpus_override_with_missing_file_errors_ex_ioerr() {
    // Build a guaranteed-missing path inside a freshly-created tempdir
    // rather than hardcoding `/nonexistent/override.json` — the latter
    // is non-portable (Windows path semantics differ from Unix) and on
    // Unix can collide with a real path under unusual sandbox setups.
    // The tempdir itself exists (we just made it); the file inside it
    // does not — which is exactly the EX_IOERR-triggering condition we
    // want to exercise.
    let tmp = tempfile::tempdir().unwrap();
    let missing_path = tmp.path().join("missing-override.json");
    let missing_path_display = missing_path.display().to_string();

    let assert = marque()
        .args(["check", "--corpus-override"])
        .arg(&missing_path)
        .write_stdin("UNCLASSIFIED")
        .assert()
        .code(74); // EX_IOERR

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains(&missing_path_display) || stderr.contains("failed to read"),
        "expected IO error mentioning the missing path, got: {stderr}"
    );
}

/// Mirror of the gated tests above: in the default build (no
/// `corpus-override` feature), the CLI must reject the flag as
/// unknown — clap's exit code for unknown flags is 2 (EX_USAGE
/// would be 64; clap's own usage-error exit is 2). The behavior
/// being pinned is "the flag does not exist on the surface," not
/// "the flag exists but errors" — feature-gating at the clap level
/// is the load-bearing security property (a misconfigured build
/// cannot accept override input it shouldn't).
#[cfg(not(feature = "corpus-override"))]
#[test]
fn corpus_override_absent_without_feature() {
    let assert = marque()
        .args(["check", "--corpus-override", "anything.json"])
        .write_stdin("UNCLASSIFIED")
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("unexpected argument")
            || stderr.contains("corpus-override")
            || stderr.contains("--corpus-override"),
        "expected clap unknown-flag error mentioning --corpus-override, \
         got: {stderr}"
    );
}
