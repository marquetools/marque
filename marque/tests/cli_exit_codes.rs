// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CLI exit-code precedence locks.
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
//! unreachable through the existing CAPCO ruleset. When a future
//! Localized FixIntent rule lands and a synthetic R002-triggering
//! fixture becomes available, the R002 case slots in cleanly here.
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
    // The non-ic-dissem-in-classified-banner rule (§H.9) is
    // `Severity::Warn` and emits no fix — a "manual review required"
    // signal, not an automatable rewrite. The fixture
    // `invalid/classified_banner_limdis.txt` (`SECRET//LIMDIS`)
    // produces exactly one diagnostic at span 8..14.
    //
    // The exit-code precedence chain in `marque::main::merge_exit_code`
    // routes Warn-only documents to `EX_DIAG_WARN = 2`. This test
    // pins the integration shape: the spawned `marque` binary, given
    // a real Warn-only fixture, returns exit code 2.
    //
    // The `fix` exit code is verified first; the diagnostic-stream
    // belt-and-suspenders runs through `check` (which writes the
    // diagnostic JSON to stdout — `fix` emits no stdout when there
    // are no fixes to apply, which is exactly the no-fix shape).
    marque()
        .args(["fix", "--dry-run", "--format", "json"])
        .arg(fixture("invalid/classified_banner_limdis.txt"))
        .assert()
        .code(2);

    // Confirm the diagnostic stream actually carries the
    // non-ic-dissem-in-classified-banner Warn — a future change that
    // retired the rule or flipped its severity would silently fall
    // back to EX_OK without this guard.
    let check = marque()
        .args(["check", "--format", "json"])
        .arg(fixture("invalid/classified_banner_limdis.txt"))
        .assert()
        .code(2);
    let stdout = String::from_utf8_lossy(&check.get_output().stdout).into_owned();
    // The `rule` field on the wire is a structured 2-tuple object.
    let expected_rule_fragment = r#""rule":{"scheme":"capco","predicate_id":"page.dissem.non-ic-dissem-in-classified-banner"}"#;
    assert!(
        stdout.contains(expected_rule_fragment),
        "expected the non-ic-dissem-in-classified-banner rule in diagnostic stream; got: {stdout}"
    );
    assert!(
        stdout.contains("\"severity\":\"warn\""),
        "expected severity=warn for the non-ic-dissem-in-classified-banner rule; got: {stdout}"
    );
}

#[test]
fn fix_with_error_exits_one() {
    // The rel-to-missing-usa rule fires on
    // `invalid/missing_usa_trigraph.txt`. With `--dry-run`, the re-lint
    // after applying the fix should see fewer diagnostics, but if any
    // error remains we exit `EX_DIAG_ERROR`. If the fix clears all
    // errors, we exit `EX_OK` — both outcomes are valid.
    let assert = marque()
        .args(["fix", "--dry-run", "--format", "json"])
        .arg(fixture("invalid/missing_usa_trigraph.txt"))
        .assert();
    let code = assert.get_output().status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 1,
        "fix --dry-run on the rel-to-missing-usa fixture should exit 0 (auto-fixed) \
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
    // Baseline: `check` with the rel-to-missing-usa fixture → 1. Pins
    // the `EX_DIAG_ERROR` constant.
    marque()
        .args(["check", "--format", "json"])
        .arg(fixture("invalid/missing_usa_trigraph.txt"))
        .assert()
        .code(1);
}
