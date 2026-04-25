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

/// Produce a stdin input large enough that a 1 ms deadline will trip
/// inside the candidate loop (or, on a slow runner, before the first
/// candidate). 4 000 banners is the same shape the engine deadline-
/// overhead bench uses; it is small enough to keep the test fast and
/// large enough to reliably exceed a 1 ms budget.
fn many_banners(count: usize) -> String {
    "SECRET//NF\n\n\n".repeat(count)
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
    // A 1 ms budget against 4 000 banner candidates will trip the
    // per-candidate deadline check and emit the truncation warning to
    // stderr. The exit code depends on whether any diagnostics fired
    // before the abort: 0 if the loop exited before the first
    // candidate was scored, otherwise 1/2 based on the partial set.
    // We assert only the stderr warning and the absence of a hard
    // failure (exit 64/65/74/75) — the lint output itself is whatever
    // the engine got through.
    let assert = marque()
        .args(["check", "--format", "json", "--deadline", "1ms"])
        .write_stdin(many_banners(4_000))
        .assert();
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("⚠ deadline exceeded: covered"),
        "expected truncation warning on stderr, got: {stderr:?}"
    );
    let code = output.status.code().unwrap_or(-1);
    assert!(
        matches!(code, 0..=2),
        "expected exit 0/1/2 for partial check, got: {code}"
    );
}

#[test]
fn cli_deadline_fix_exits_ex_tempfail() {
    // A 1 ms budget against many fixable banners cannot complete the
    // full lint+fix pass on any reasonable runner. The deadline trip
    // can land at the pre-pass / per-candidate (lint) check or at the
    // post-lint / per-fix check; both routes converge on
    // `Err(DeadlineExceeded)` → EX_TEMPFAIL (75).
    let assert = marque()
        .args([
            "fix",
            "--write-stdout",
            "--format",
            "json",
            "--deadline",
            "1ms",
        ])
        .write_stdin(many_banners(4_000))
        .assert()
        .code(75);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("deadline exceeded"),
        "expected deadline-exceeded explanation on stderr, got: {stderr:?}"
    );
    assert!(
        stderr.contains("no fixes applied"),
        "expected explicit 'no fixes applied' note on stderr, got: {stderr:?}"
    );
}

#[test]
fn cli_no_deadline_runs_to_completion() {
    // Sanity check that the `--deadline` plumbing does not regress the
    // happy path. A small fixture with no deadline produces no
    // truncation warning.
    let assert = marque()
        .args(["check", "--format", "json"])
        .arg(fixture("invalid/banner_abbrev.txt"))
        .assert()
        .code(1);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !stderr.contains("⚠ deadline exceeded"),
        "no-deadline run must not emit truncation warning, got: {stderr:?}"
    );
}
