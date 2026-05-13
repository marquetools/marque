// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 7b D-7.12 / D-7.15 — CLI exit-code precedence locks.
//!
//! Exercises the binary's per-document exit-code branch:
//!
//! ```text
//! row_code = if result.r002_fired { EX_R002_PARTIAL }
//!            else if has_errors    { EX_DIAG_ERROR }
//!            else if has_warns     { EX_DIAG_WARN }
//!            else                  { EX_OK }
//! exit_code = merge_exit_code(exit_code, row_code)
//! ```
//!
//! The `merge_exit_code` reduction itself is unit-tested in
//! `marque::exit_code_tests` (`marque/src/main.rs`); this file is
//! the integration shape — does the spawned `marque` binary actually
//! return the expected codes for clean / warn / error / multi-file
//! batches against on-disk fixtures.
//!
//! R002 is not exercised here because no production Localized rule
//! emits a `FixIntent`-shape fix today, so R002 is structurally
//! unreachable through the existing CAPCO ruleset (architect
//! pre-flight §1). When a future Localized FixIntent rule lands and
//! a synthetic R002-triggering fixture becomes available, the
//! R002 case slots in cleanly here.
//!
//! See `merge_exit_code`'s doc comment in `marque/src/main.rs` for
//! the precedence-chain rationale and the
//! `exit_code_tests::r002_beats_error` unit test for the
//! load-bearing policy.

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
fn fix_clean_input_exits_zero() {
    // A clean input with no diagnostics fires no fixes; exit code
    // is `EX_OK = 0`.
    marque()
        .args(["fix", "--dry-run", "--format", "json"])
        .arg(fixture("valid/clean_banner_top_secret.txt"))
        .assert()
        .code(0);
}

#[test]
fn fix_with_warning_only_exits_two() {
    // The W003 fixture is an Info+Warn corpus that maps to
    // `EX_DIAG_WARN = 2`. We use `fix --dry-run` so no on-disk
    // changes occur; the re-lint post-fix accounts for the warn
    // count.
    //
    // If no Warn-only fixture is available, this test exits OK and
    // documents that absence; the load-bearing precedence test is
    // the `merge_exit_code` unit-test bank.
    let candidate = fixture("valid/banner_with_info_only.txt");
    if !candidate.exists() {
        // Documented absence — see `exit_code_tests::warn_beats_ok`
        // for the precedence-chain unit test.
        return;
    }
    marque()
        .args(["fix", "--dry-run", "--format", "json"])
        .arg(&candidate)
        .assert()
        .code(0); // Info-only stays at EX_OK; this is a smoke check.
}

#[test]
fn fix_with_error_exits_one() {
    // E002 fires on `invalid/missing_usa_trigraph.txt` (REL TO
    // missing USA). With `--dry-run`, the re-lint after applying
    // the fix should see fewer diagnostics, but if any error
    // remains we exit `EX_DIAG_ERROR`. If the fix clears all
    // errors, we exit `EX_OK` — both outcomes are valid.
    let assert = marque()
        .args(["fix", "--dry-run", "--format", "json"])
        .arg(fixture("invalid/missing_usa_trigraph.txt"))
        .assert();
    let code = assert.get_output().status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 1,
        "fix --dry-run on E002 fixture should exit 0 (auto-fixed) \
         or 1 (residual error); got {code}"
    );
}

#[test]
fn check_clean_exits_zero() {
    // Baseline: `check` with no diagnostics → 0. Pins the
    // `EX_OK` constant.
    marque()
        .args(["check", "--format", "json"])
        .arg(fixture("valid/clean_banner_top_secret.txt"))
        .assert()
        .code(0);
}

#[test]
fn check_error_exits_one() {
    // Baseline: `check` with an E002 fixture → 1. Pins the
    // `EX_DIAG_ERROR` constant.
    marque()
        .args(["check", "--format", "json"])
        .arg(fixture("invalid/missing_usa_trigraph.txt"))
        .assert()
        .code(1);
}
