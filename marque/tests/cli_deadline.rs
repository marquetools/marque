// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Spec 005 Phase 3a — CLI integration tests for `--deadline`.
//!
//! Covers T026–T028:
//! - `--deadline 0` (and `--deadline 0s`) → `EX_USAGE` (64).
//! - `marque check --deadline 1ms` against a multi-candidate input
//!   succeeds with a stderr truncation warning.
//! - `marque fix --deadline 1ms` against a fixture with applicable
//!   fixes returns `EX_TEMPFAIL` (75).

use assert_cmd::Command;
use std::path::PathBuf;
use std::time::Instant;

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

/// Produce a stdin input large enough that a small fraction of an
/// observed baseline still maps to a deadline that reliably trips the
/// engine's candidate loop. 4 000 banners is the same shape the engine
/// deadline-overhead bench uses; small enough to keep the test fast,
/// large enough that a fraction-of-baseline deadline cannot complete
/// before the engine starts checking.
fn many_banners(count: usize) -> String {
    "SECRET//NF\n\n\n".repeat(count)
}

/// Run `marque <args>` once with a known-clean (rule-free) stdin and
/// return one-hundredth of the elapsed wall time (floor 1 ms). Used to
/// derive a per-test deadline that scales with the host's actual
/// `marque` runtime — mirrors the `crates/engine/tests/deadline.rs`
/// baseline pattern, lifted to the CLI level.
///
/// The baseline input is *deliberately* rule-free ("Plain text\n"
/// repeated, no portion markings, no banner candidates) so the
/// observed wall time reflects process spawn + scanner pass + minimal
/// engine work — *not* output rendering of thousands of diagnostics
/// against the diagnostic-heavy test fixture, which would inflate the
/// baseline and yield a too-generous deadline. The deadline check is
/// enforced inside the engine loop, not during output rendering, so
/// using a clean baseline keeps the derived deadline tied to engine
/// work.
///
/// The 1/100 fraction means the deadline is well below process spawn
/// cost alone — by the time the engine reads `Instant::now()` for its
/// pre-pass / per-candidate check, the deadline is almost always
/// already in the past. Truncation is robust against machine speed
/// because the budget effectively says "abort the moment you start."
fn measure_baseline_ms(args: &[&str]) -> u64 {
    let start = Instant::now();
    let _ = marque()
        .args(args)
        .write_stdin("Plain text with no markings.\n".to_owned())
        .assert();
    let elapsed = start.elapsed();
    ((elapsed.as_millis() / 100) as u64).max(1)
}

#[test]
fn cli_deadline_zero_exits_with_ex_usage() {
    // `--deadline 0` — humantime requires a unit suffix, so this fails
    // at parse time. We map both parse failure and zero-duration to
    // EX_USAGE (64).
    marque()
        .args(["check", "--deadline", "0"])
        .write_stdin("TOP SECRET\n")
        .assert()
        .code(64);
}

#[test]
fn cli_deadline_zero_seconds_exits_with_ex_usage() {
    // `--deadline 0s` parses to Duration::ZERO; we explicitly reject
    // it because a zero budget would always trip the pre-pass deadline
    // check on entry, which is not what the operator intended.
    marque()
        .args(["check", "--deadline", "0s"])
        .write_stdin("TOP SECRET\n")
        .assert()
        .code(64);
}

#[test]
fn cli_deadline_unparsable_exits_with_ex_usage() {
    // Garbage input → humantime returns Err → EX_USAGE.
    marque()
        .args(["check", "--deadline", "not-a-duration"])
        .write_stdin("TOP SECRET\n")
        .assert()
        .code(64);
}

#[test]
fn cli_deadline_truncates_check_output_with_warning() {
    // Deadline derived from a per-host baseline (≈1/100 of the
    // observed `marque check` wall time on this fixture, floor 1 ms)
    // — a hard-coded `1ms` would be timing-flaky across machine
    // classes (debug vs. release, fast laptop vs. slow CI runner,
    // future hardware). Mirrors the engine-level deadline tests in
    // `crates/engine/tests/deadline.rs`, lifted to the CLI level.
    //
    // We assert: (1) the truncation warning appears on stderr;
    // (2) exit is 0/1/2 (partial diagnostic set, not a hard
    // failure 64/65/74/75).
    let payload = many_banners(4_000);
    let deadline_ms = measure_baseline_ms(&["check", "--format", "json"]);
    let deadline_arg = format!("{deadline_ms}ms");
    let assert = marque()
        .args(["check", "--format", "json", "--deadline", &deadline_arg])
        .write_stdin(payload)
        .assert();
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("⚠ deadline exceeded: covered"),
        "expected truncation warning on stderr (deadline={deadline_arg}), got: {stderr:?}"
    );
    let code = output.status.code().unwrap_or(-1);
    assert!(
        matches!(code, 0..=2),
        "expected exit 0/1/2 for partial check, got: {code}"
    );
}

#[test]
fn cli_deadline_fix_exits_ex_tempfail_human_format() {
    // Same baseline-derived strategy as the check truncation test —
    // a 1/100-fraction budget reliably trips the deadline before the
    // full lint+fix pass completes. The deadline trip can land at
    // the pre-pass / per-candidate (lint) check or at the post-lint
    // / per-fix check; both routes converge on
    // `Err(DeadlineExceeded)` → EX_TEMPFAIL (75).
    //
    // Uses `--format human` because the trailing "no fixes applied"
    // narration on stderr is intentionally suppressed in JSON mode
    // (stderr remains valid NDJSON for pipe consumers there). JSON
    // mode is exercised by `cli_deadline_fix_json_format_emits_clean_ndjson`.
    let payload = many_banners(4_000);
    let deadline_ms = measure_baseline_ms(&["fix", "--dry-run", "--format", "human"]);
    let deadline_arg = format!("{deadline_ms}ms");
    let assert = marque()
        .args([
            "fix",
            "--write-stdout",
            "--format",
            "human",
            "--deadline",
            &deadline_arg,
        ])
        .write_stdin(payload)
        .assert()
        .code(75);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("deadline exceeded"),
        "expected deadline-exceeded explanation on stderr (deadline={deadline_arg}), got: {stderr:?}"
    );
    assert!(
        stderr.contains("no fixes applied"),
        "expected explicit 'no fixes applied' note on stderr, got: {stderr:?}"
    );
}

#[test]
fn cli_deadline_fix_json_format_emits_clean_ndjson() {
    // In `--format json` mode, the trailing human narration must be
    // suppressed so stderr remains valid NDJSON (one diagnostic per
    // line) for pipe consumers like `marque fix --format json | jq`.
    // The exit code (75) is the wire-level signal that the deadline
    // tripped; JSON consumers don't need a stderr explanation.
    let payload = many_banners(4_000);
    let deadline_ms = measure_baseline_ms(&["fix", "--dry-run", "--format", "json"]);
    let deadline_arg = format!("{deadline_ms}ms");
    let assert = marque()
        .args([
            "fix",
            "--write-stdout",
            "--format",
            "json",
            "--deadline",
            &deadline_arg,
        ])
        .write_stdin(payload)
        .assert()
        .code(75);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    // Human narration MUST be absent in JSON mode.
    assert!(
        !stderr.contains("no fixes applied"),
        "JSON mode must not emit the human narration line; got: {stderr:?}"
    );
    assert!(
        !stderr.contains("⚠"),
        "JSON mode must not emit warning glyphs in the diagnostic stream; got: {stderr:?}"
    );
    // Every non-empty stderr line must parse as JSON (NDJSON shape).
    for line in stderr.lines().filter(|l| !l.is_empty()) {
        assert!(
            serde_json::from_str::<serde_json::Value>(line).is_ok(),
            "stderr line is not valid JSON: {line:?}"
        );
    }
}

#[test]
fn cli_deadline_quiet_suppresses_truncation_warning() {
    // The `-q` / `--quiet` contract suppresses non-diagnostic stderr
    // narration. The deadline-truncation warning is operator narration,
    // not a diagnostic, so it must be silenced when `-q` is set.
    // Same baseline-derived deadline shape as the truncation test.
    let payload = many_banners(4_000);
    let deadline_ms = measure_baseline_ms(&["check", "--format", "json"]);
    let deadline_arg = format!("{deadline_ms}ms");
    let assert = marque()
        .args([
            "check",
            "-q",
            "--format",
            "json",
            "--deadline",
            &deadline_arg,
        ])
        .write_stdin(payload)
        .assert();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !stderr.contains("⚠ deadline exceeded"),
        "with -q, truncation warning must be suppressed; got: {stderr:?}"
    );
}

#[test]
fn cli_deadline_overflow_exits_cleanly() {
    // `Instant::now() + Duration::from_secs(huge)` panics on overflow
    // for very large user-controlled durations. The CLI uses
    // `checked_add` and maps overflow to EX_USAGE so a pathological
    // `--deadline` value cannot crash the binary. The exact value
    // that overflows depends on the platform clock; we pass a value
    // that humantime accepts but is large enough to exceed any
    // realistic Instant range, and assert the binary does NOT abort
    // via signal (a panic-induced exit would land at e.g. 134 / 139,
    // outside the documented exit-code set). A clean exit at 0/1/2
    // (deadline trivially didn't trip; large budget = full pass) or
    // 64 (overflow trapped to EX_USAGE) are both acceptable shapes.
    let assert = marque()
        .args(["check", "--format", "json", "--deadline", "9999years"])
        .write_stdin("SECRET//NF\n")
        .assert();
    let code = assert.get_output().status.code().unwrap_or(-1);
    assert!(
        matches!(code, 0..=2 | 64),
        "expected a normal diagnostic exit (0 clean / 1 errors / 2 warnings) \
         or 64 (overflow trapped to EX_USAGE), got: {code}"
    );
}

#[test]
fn cli_no_deadline_runs_to_completion() {
    // Sanity check that the `--deadline` plumbing does not regress the
    // happy path. A small fixture with no deadline produces no
    // truncation warning.
    let assert = marque()
        .args(["check", "--format", "json"])
        .arg(fixture("invalid/banner_abbrev_3.txt"))
        .assert()
        .code(1);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !stderr.contains("⚠ deadline exceeded"),
        "no-deadline run must not emit truncation warning, got: {stderr:?}"
    );
}
