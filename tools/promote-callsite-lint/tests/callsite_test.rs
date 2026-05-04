// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for the FR-040 base call-site lint.
//!
//! Each test materializes a synthetic workspace tree under a
//! `tempfile::TempDir`, writes one `.rs` source file at a path
//! that exercises one of the four classifications
//! (production-allowed, test-fixture-allowed, test-fixture-unmarked,
//! other), runs `callsite::scan_workspace`, and asserts the
//! produced diagnostic vector matches expectations.

use std::fs;
use std::path::Path;

use promote_callsite_lint::{Severity, callsite};
use tempfile::TempDir;

fn write(tmp: &Path, rel: &str, contents: &str) {
    let path = tmp.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, contents).unwrap();
}

#[test]
fn production_allowed_inside_engine_fix_inner() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
struct Engine;
impl Engine {
    fn fix_inner(&self) {
        let _ = AppliedFix::__engine_promote(
            (),
            (),
            (),
            false,
            None,
            EnginePromotionToken::__engine_construct(),
        );
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(
        diags.is_empty(),
        "expected no diagnostics, got {diags:#?}",
    );
}

#[test]
fn production_allowed_inside_apply_text_corrections() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
struct Engine;
impl Engine {
    fn apply_text_corrections(&self) {
        let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn production_allowed_inside_engine_promotion_token_helper() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
fn engine_promotion_token() -> EnginePromotionToken {
    EnginePromotionToken::__engine_construct()
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn production_denied_outside_allow_listed_fns() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
fn evil_helper() {
    let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1, "expected exactly 1 diagnostic, got {diags:#?}");
    assert_eq!(diags[0].code, "PRC002");
    assert_eq!(diags[0].severity, Severity::Error);
}

#[test]
fn production_denied_in_non_engine_crate() {
    let tmp = TempDir::new().unwrap();
    // Even inside a `fix_inner` function, a non-engine crate is forbidden.
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
fn fix_inner() {
    let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "PRC002");
}

#[test]
fn test_fixture_allowed_with_carve_out_comment() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/tests/audit_test.rs",
        r"
fn fabricate_leaky_fix() {
    // Test-fixture carve-out per Constitution V
    let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn test_fixture_allowed_with_marker_within_5_lines_above_call() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/tests/audit_test.rs",
        r"
fn fabricate_leaky_fix() {
    // Test-fixture carve-out per Constitution V Principle V: this
    // synthetic value flows through G13 sentinel sweeps.
    //
    //
    let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn test_fixture_denied_without_carve_out_comment() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/tests/audit_test.rs",
        r"
fn fabricate_leaky_fix() {
    let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1, "expected exactly 1 diagnostic, got {diags:#?}");
    assert_eq!(diags[0].code, "PRC001");
    assert_eq!(diags[0].severity, Severity::Error);
}

#[test]
fn test_fixture_marker_too_far_above_is_denied() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/tests/audit_test.rs",
        r"
fn fabricate_leaky_fix() {
    // Test-fixture carve-out per Constitution V






    let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "PRC001");
}

#[test]
fn test_fixture_in_workspace_root_tests_dir() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "tests/integration.rs",
        r"
fn fixture() {
    // Test-fixture carve-out per Constitution V
    let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn cfg_test_module_in_src_treated_as_test_scope() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
#[cfg(test)]
mod tests {
    fn fixture() {
        // Test-fixture carve-out per Constitution V
        let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn cfg_test_module_without_marker_is_denied() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
#[cfg(test)]
mod tests {
    fn fixture() {
        let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "PRC001");
}

#[test]
fn engine_construct_call_also_flagged_outside_allow_list() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/bar/src/lib.rs",
        r"
fn naughty() {
    let _ = EnginePromotionToken::__engine_construct();
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "PRC002");
}
