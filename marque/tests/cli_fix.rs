// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CLI integration tests for `marque fix`.

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
    //
    // `missing_usa_trigraph.txt` makes the rel-to-missing-usa rule fire
    // with confidence 0.97 (passes the 0.95 threshold) and the fix
    // produces `SECRET//REL TO USA, GBR`. Validates that a
    // high-confidence fix is applied AND emits an audit record on
    // stderr.
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_path = tmp_dir.path().join("missing_usa_trigraph.txt");
    std::fs::copy(fixture("invalid/missing_usa_trigraph.txt"), &tmp_path).unwrap();

    let assert = marque().args(["fix"]).arg(&tmp_path).assert().success();

    // File should be modified: REL TO GBR, AUS → REL TO USA, AUS, GBR.
    let fixed = std::fs::read_to_string(&tmp_path).unwrap();
    assert!(
        fixed.starts_with("SECRET//REL TO USA"),
        "rel-to-missing-usa fix should be applied, got: {fixed:?}"
    );

    // stderr should contain audit NDJSON with schema version.
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains(&format!(
            "\"schema\":\"{}\"",
            marque_engine::AUDIT_SCHEMA_VERSION
        )),
        "audit record should contain schema version, got: {stderr}"
    );
    // The `rule` field on the wire is a structured 2-tuple object with
    // alphabetically-ordered keys per serde_json's BTreeMap-backed
    // Value serialization.
    let expected_rule_fragment =
        r#""rule":{"predicate_id":"portion.dissem.rel-to-missing-usa","scheme":"capco"}"#;
    assert!(
        stderr.contains(expected_rule_fragment),
        "audit record should contain the rel-to-missing-usa rule, got: {stderr}"
    );
    assert!(
        stderr.contains("\"dry_run\":false"),
        "audit record should have dry_run=false, got: {stderr}"
    );
}

#[test]
fn fix_dry_run_does_not_modify_file() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_path = tmp_dir.path().join("missing_usa_trigraph.txt");
    std::fs::copy(fixture("invalid/missing_usa_trigraph.txt"), &tmp_path).unwrap();
    let original = std::fs::read_to_string(&tmp_path).unwrap();

    let assert = marque()
        .args(["fix", "--dry-run"])
        .arg(&tmp_path)
        .assert()
        .success();

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
        .write_stdin("SECRET//REL TO GBR\n")
        .assert()
        .success(); // rel-to-missing-usa is the only issue and it gets fixed

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(
        stdout.as_ref(),
        "SECRET//REL TO USA, GBR\n",
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
        .write_stdin("SECRET//REL TO GBR\n")
        .assert()
        .success();

    // -q suppresses narration but NOT audit records.
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains(&format!(
            "\"schema\":\"{}\"",
            marque_engine::AUDIT_SCHEMA_VERSION
        )),
        "-q must not suppress audit NDJSON, got: {stderr}"
    );
    // Narration line should be absent. The audit record carries
    // `"type": "applied_fix"` so a bare `contains("applied")` would
    // false-positive on the audit record itself. Match the narration
    // prefix (`<label>: applied N fix(es)`) instead.
    assert!(
        !stderr.contains(": applied "),
        "-q should suppress narration lines, got: {stderr}"
    );
}

#[test]
fn fix_exit_code_zero_when_all_fixed() {
    // SECRET//REL TO GBR only triggers rel-to-missing-usa (confidence
    // 0.97) — fully fixable at the default 0.95 threshold.
    marque()
        .args(["fix"])
        .write_stdin("SECRET//REL TO GBR\n")
        .assert()
        .success();
}

#[test]
fn fix_exit_code_one_when_issues_remain() {
    // `(TS//HCS)` triggers the bare-HCS legacy-form rule (classifier
    // must choose HCS-O vs HCS-P) — a conscious-defer rule with
    // `fix_intent: None`. The diagnostic emits but no auto-fix
    // applies; the error remains after the fix pass → exit 1.
    //
    // HCS-O vs HCS-P is a classifier decision per §H.4, so the rule is
    // consciously deferred and intentionally has no auto-fix path —
    // which is exactly what makes `(TS//HCS)` exercise the
    // "issues remain" path.
    marque()
        .args(["fix"])
        .write_stdin("(TS//HCS)\n")
        .assert()
        .code(1);
}

#[test]
fn fixed_timestamp_rejected_without_env_var() {
    marque()
        .args(["fix", "--fixed-timestamp", "2024-01-01T00:00:00Z"])
        .write_stdin("SECRET//REL TO GBR\n")
        .assert()
        .code(64);
}

#[test]
fn fixed_timestamp_produces_deterministic_audit() {
    let run = |_n: usize| -> String {
        let assert = marque()
            .env("MARQUE_ALLOW_FIXED_CLOCK", "1")
            .args(["fix", "--fixed-timestamp", "2024-06-15T12:00:00Z"])
            .write_stdin("SECRET//REL TO GBR\n")
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
        .write_stdin("SECRET//REL TO GBR\n")
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
        .write_stdin("SECRET//REL TO GBR\n")
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
        stderr.contains(&format!(
            "\"schema\":\"{}\"",
            marque_engine::AUDIT_SCHEMA_VERSION
        )),
        "dry-run should still emit audit records on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("\"dry_run\":true"),
        "dry-run audit should have dry_run=true, got: {stderr}"
    );
}

// --- H6: no-fix diagnostics → exit 1, no audit ---

#[test]
fn fix_no_fix_diagnostics_only_exits_one_no_audit() {
    // `(TS//HCS)` emits one Error-severity diagnostic (bare-HCS
    // legacy form, §H.4 p62) with `fix_intent: None` — a
    // conscious-defer rule because HCS-O vs HCS-P vs HCS-O-P is a
    // classifier decision the engine cannot make. After fix, the
    // error remains → exit 1 with no audit records.
    //
    // This test exercises the "all-no-fix-diagnostics → no audit +
    // exit 1" surface. The parallel CLI-level "fix proposal below
    // threshold" gate lives in
    // `cli_confidence_threshold_overrides_config` in
    // `marque/tests/cli_config.rs`; the engine-level "sub-threshold
    // proposals never auto-apply" gate is pinned in
    // `crates/engine/tests/audit_completeness.rs`.
    let assert = marque()
        .args(["fix"])
        .write_stdin("(TS//HCS)\n")
        .assert()
        .code(1);

    // No fixes applied → no audit records.
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let audit_lines: Vec<&str> = stderr.lines().filter(|l| l.starts_with('{')).collect();
    assert!(
        audit_lines.is_empty(),
        "no fixes applied → no audit records, got: {audit_lines:?}"
    );

    // stdout should contain the original text (unchanged, written via --write-stdout default).
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(stdout.as_ref(), "(TS//HCS)\n");
}

// --- Suggest-only narration (issue #235 / #186 PR-3, M-3) ---
//
// Suggest-channel diagnostics are advisory; they don't "require manual
// review", they offer optional alternatives. A document whose only
// outstanding diagnostics are Suggest-severity must NOT trigger the
// "N issue(s) require manual review" narration on stderr.

#[test]
fn fix_suggest_only_input_emits_no_manual_review_narration() {
    // The rel-to-trigraph-suggest rule fires on `AUT` (Austria) with a
    // suggestion of `AUS` (Australia). No other rule fires on this
    // banner — the only outstanding diagnostic is Suggest-severity.
    let assert = marque()
        .args(["fix"])
        .write_stdin("SECRET//REL TO USA, AUT, GBR\n")
        .assert()
        .success(); // Suggest is CI-silent → exit 0

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !stderr.contains("require manual review"),
        "Suggest-only diagnostics must not trigger manual-review narration, \
         got stderr: {stderr}"
    );
}

// --- L3: --write-stdout with file path ---

#[test]
fn fix_write_stdout_on_file_input() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_path = tmp_dir.path().join("input.txt");
    std::fs::write(&tmp_path, "SECRET//REL TO GBR\n").unwrap();
    let original = std::fs::read_to_string(&tmp_path).unwrap();

    let assert = marque()
        .args(["fix", "--write-stdout"])
        .arg(&tmp_path)
        .assert()
        .success();

    // stdout should have fixed content.
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(stdout.as_ref(), "SECRET//REL TO USA, GBR\n");

    // File should be UNCHANGED (--write-stdout overrides --in-place default).
    let after = std::fs::read_to_string(&tmp_path).unwrap();
    assert_eq!(original, after, "--write-stdout must not modify the file");
}

// --- L3 continued: dry-run exit code matches apply exit code ---

#[test]
fn fix_dry_run_exit_code_matches_apply_exit_code() {
    // Mixed input: one line is fully fixable (rel-to-missing-usa), the
    // other has a no-fix error on the JOINT line. Apply and dry-run
    // must produce the same exit code (both should exit 1 because the
    // JOINT errors remain regardless of mode).
    let input = "SECRET//REL TO GBR\n//JOINT SECRET USA GBR\n";
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

// ---------------------------------------------------------------------------
// Single-schema-per-build invariant on the audit stream.
// ---------------------------------------------------------------------------
//
// An engine binary must emit exactly one audit-record schema for the
// lifetime of the build — never a mix of pre-cutover and post-cutover
// records on the same stream. The build-layer half is enforced in
// `crates/engine/build.rs`, which validates `MARQUE_AUDIT_SCHEMA` to
// the closed accept-list `["marque-3.0"]` and panics on anything else.
// This test pins the runtime-emitter half: every audit record on
// stderr must declare the matching `schema` string, and any
// pre-cutover label must not appear anywhere in the stream.

#[test]
fn audit_stream_uses_only_one_schema_version() {
    // A multi-fix input that exercises several rule emitters in a
    // single run, so the test isn't trivially passing on a single
    // record.
    let input = "SECRET//REL TO GBR\nSECRET//REL TO AUS\nSECRET//REL TO JPN\n";
    let assert = marque().args(["fix"]).write_stdin(input).assert().success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let audit_lines: Vec<&str> = stderr.lines().filter(|l| l.starts_with('{')).collect();
    assert!(
        !audit_lines.is_empty(),
        "vacuity guard: input must produce ≥1 audit record, got 0 \
         (stderr was: {stderr:?})"
    );

    let active_schema = marque_engine::AUDIT_SCHEMA_VERSION;
    // Known-rejected pre-cutover value. If the active schema ever names
    // this directly the build would have panicked at
    // `crates/engine/build.rs`, so reaching this test means no
    // contamination is possible from the build side. This pin catches a
    // hypothetical emitter that still strings the old label into a
    // record.
    let other_schema = "marque-mvp-3";

    for line in &audit_lines {
        let parsed: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("audit record must be valid JSON ({e}): {line:?}"));
        assert_eq!(
            parsed["schema"].as_str(),
            Some(active_schema),
            "every audit record must declare schema {active_schema:?}; \
             record was: {line:?}"
        );
        assert!(
            !line.contains(&format!("\"schema\":\"{other_schema}\"")),
            "stream contains the other schema {other_schema:?}; \
             record was: {line:?}"
        );
    }
}
