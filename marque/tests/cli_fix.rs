// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 4 — CLI integration tests for `marque fix` (T051, T051a).

use assert_cmd::Command;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
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
fn fix_applies_high_confidence_and_emits_audit() {
    // Copy fixture to temp dir so in-place write doesn't clobber corpus.
    // Uses tempdir (not NamedTempFile) to avoid Windows file-locking issues.
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_path = tmp_dir.path().join("mixed_confidence.txt");
    std::fs::copy(fixture("invalid/mixed_confidence.txt"), &tmp_path).unwrap();

    let assert = marque().args(["fix"]).arg(&tmp_path).assert().code(1); // E003 remains

    // File should be modified: NF → NOFORN
    let fixed = std::fs::read_to_string(&tmp_path).unwrap();
    assert!(
        fixed.starts_with("SECRET//NOFORN"),
        "E001 fix (NF→NOFORN) should be applied, got: {fixed:?}"
    );

    // stderr should contain audit NDJSON with schema version.
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("\"schema\":\"marque-mvp-1\""),
        "audit record should contain schema version, got: {stderr}"
    );
    assert!(
        stderr.contains("\"rule\":\"E001\""),
        "audit record should contain rule E001, got: {stderr}"
    );
    assert!(
        stderr.contains("\"dry_run\":false"),
        "audit record should have dry_run=false, got: {stderr}"
    );
}

#[test]
fn fix_dry_run_does_not_modify_file() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_path = tmp_dir.path().join("mixed_confidence.txt");
    std::fs::copy(fixture("invalid/mixed_confidence.txt"), &tmp_path).unwrap();
    let original = std::fs::read_to_string(&tmp_path).unwrap();

    let assert = marque()
        .args(["fix", "--dry-run"])
        .arg(&tmp_path)
        .assert()
        .code(1); // E003 remains

    // File must be unchanged.
    let after = std::fs::read_to_string(&tmp_path).unwrap();
    assert_eq!(original, after, "dry-run must not modify the file");

    // stderr should contain audit with dry_run=true.
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("\"dry_run\":true"),
        "dry-run audit should have dry_run=true, got: {stderr}"
    );
}

#[test]
fn fix_stdin_writes_stdout_by_default() {
    let assert = marque()
        .args(["fix"])
        .write_stdin("SECRET//NF\n")
        .assert()
        .success(); // E001 is the only issue and it gets fixed

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(
        stdout.as_ref(),
        "SECRET//NOFORN\n",
        "stdin fix should write to stdout"
    );
}

#[test]
fn fix_dry_run_and_in_place_mutual_exclusion() {
    marque()
        .args(["fix", "--dry-run", "--in-place", "dummy.txt"])
        .assert()
        .code(64);
}

#[test]
fn fix_in_place_and_write_stdout_mutual_exclusion() {
    marque()
        .args(["fix", "--in-place", "--write-stdout", "dummy.txt"])
        .assert()
        .code(64);
}

#[test]
fn fix_quiet_does_not_suppress_audit() {
    let assert = marque()
        .args(["fix", "-q"])
        .write_stdin("SECRET//NF\n")
        .assert()
        .success();

    // -q suppresses narration but NOT audit records.
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("\"schema\":\"marque-mvp-1\""),
        "-q must not suppress audit NDJSON, got: {stderr}"
    );
    // Narration line should be absent.
    assert!(
        !stderr.contains("applied"),
        "-q should suppress narration lines, got: {stderr}"
    );
}

#[test]
fn fix_exit_code_zero_when_all_fixed() {
    // SECRET//NF only triggers E001 (confidence 1.0) — fully fixable.
    marque()
        .args(["fix"])
        .write_stdin("SECRET//NF\n")
        .assert()
        .success();
}

#[test]
fn fix_exit_code_one_when_issues_remain() {
    // mixed_confidence has E003 (0.6) which stays as a suggestion.
    marque()
        .args(["fix"])
        .write_stdin("SECRET//NF\nSECRET//NOFORN//SI\n")
        .assert()
        .code(1);
}

#[test]
fn fixed_timestamp_rejected_without_env_var() {
    marque()
        .args(["fix", "--fixed-timestamp", "2024-01-01T00:00:00Z"])
        .write_stdin("SECRET//NF\n")
        .assert()
        .code(64);
}

#[test]
fn fixed_timestamp_produces_deterministic_audit() {
    let run = |_n: usize| -> String {
        let assert = marque()
            .env("MARQUE_ALLOW_FIXED_CLOCK", "1")
            .args(["fix", "--fixed-timestamp", "2024-06-15T12:00:00Z"])
            .write_stdin("SECRET//NF\n")
            .assert()
            .success();
        let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
        // Extract only NDJSON lines (start with '{').
        stderr
            .lines()
            .filter(|l| l.starts_with('{'))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let run1 = run(1);
    let run2 = run(2);

    assert_eq!(
        run1, run2,
        "two runs with --fixed-timestamp should produce identical audit NDJSON"
    );
    assert!(
        run1.contains("\"timestamp\":\"2024-06-15T12:00:00Z\""),
        "timestamp should match the fixed value, got: {run1}"
    );
}

#[test]
fn fix_dry_run_and_write_stdout_mutual_exclusion() {
    marque()
        .args(["fix", "--dry-run", "--write-stdout"])
        .write_stdin("SECRET//NF\n")
        .assert()
        .code(64);
}

// --- H4: empty input through fix path ---

#[test]
fn fix_empty_input_exits_zero_no_audit() {
    let assert = marque().args(["fix"]).write_stdin("").assert().success();

    // No markings → no fixes → no audit records.
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !stderr.contains("\"schema\""),
        "empty input should produce no audit records, got: {stderr}"
    );
    // stdout should be empty (no content to write).
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(stdout.as_ref(), "", "empty input → empty stdout");
}

// --- H5: dry-run with stdin ---

#[test]
fn fix_dry_run_stdin_produces_no_stdout() {
    let assert = marque()
        .args(["fix", "--dry-run"])
        .write_stdin("SECRET//NF\n")
        .assert()
        .success();

    // --dry-run should not write any output to stdout.
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(
        stdout.as_ref(),
        "",
        "dry-run should produce no stdout output"
    );

    // But audit records should still appear on stderr.
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("\"schema\":\"marque-mvp-1\""),
        "dry-run should still emit audit records on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("\"dry_run\":true"),
        "dry-run audit should have dry_run=true, got: {stderr}"
    );
}

// --- H6: all fixes below threshold ---

#[test]
fn fix_all_below_threshold_exits_one_no_audit() {
    // SECRET//NOFORN//SI triggers only E003 at confidence 0.6, below
    // the default 0.95 threshold. No fixes applied.
    let assert = marque()
        .args(["fix"])
        .write_stdin("SECRET//NOFORN//SI\n")
        .assert()
        .code(1); // E003 remains as error

    // No fixes applied → no audit records.
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let audit_lines: Vec<&str> = stderr.lines().filter(|l| l.starts_with('{')).collect();
    assert!(
        audit_lines.is_empty(),
        "no fixes applied → no audit records, got: {audit_lines:?}"
    );

    // stdout should contain the original text (unchanged, written via --write-stdout default).
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(stdout.as_ref(), "SECRET//NOFORN//SI\n");
}

// --- L3: --write-stdout with file path ---

#[test]
fn fix_write_stdout_on_file_input() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_path = tmp_dir.path().join("input.txt");
    std::fs::write(&tmp_path, "SECRET//NF\n").unwrap();
    let original = std::fs::read_to_string(&tmp_path).unwrap();

    let assert = marque()
        .args(["fix", "--write-stdout"])
        .arg(&tmp_path)
        .assert()
        .success();

    // stdout should have fixed content.
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(stdout.as_ref(), "SECRET//NOFORN\n");

    // File should be UNCHANGED (--write-stdout overrides --in-place default).
    let after = std::fs::read_to_string(&tmp_path).unwrap();
    assert_eq!(original, after, "--write-stdout must not modify the file");
}

// --- L3 continued: dry-run exit code matches apply exit code ---

#[test]
fn fix_dry_run_exit_code_matches_apply_exit_code() {
    let input = "SECRET//NF\nSECRET//NOFORN//SI\n";
    let apply_code = marque()
        .args(["fix"])
        .write_stdin(input)
        .assert()
        .get_output()
        .status
        .code();
    let dry_code = marque()
        .args(["fix", "--dry-run"])
        .write_stdin(input)
        .assert()
        .get_output()
        .status
        .code();
    assert_eq!(
        apply_code, dry_code,
        "dry-run exit code must match apply exit code"
    );
}

// --- L2: out-of-range --confidence-threshold exits EX_DATAERR (65) ---

#[test]
fn fix_confidence_threshold_out_of_range_exits_65() {
    marque()
        .args(["fix", "--confidence-threshold", "99.0"])
        .write_stdin("SECRET//NF\n")
        .assert()
        .code(65);
}

#[test]
fn check_confidence_threshold_out_of_range_exits_65() {
    marque()
        .args(["check", "--confidence-threshold", "1.5"])
        .write_stdin("SECRET//NF\n")
        .assert()
        .code(65);
}

#[test]
fn fix_explain_config_mutual_exclusion() {
    marque()
        .args(["fix", "--explain-config"])
        .write_stdin("SECRET//NF\n")
        .assert()
        .code(64);
}
