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
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
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
    assert_eq!(
        diags.len(),
        1,
        "expected exactly 1 diagnostic, got {diags:#?}"
    );
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
    assert_eq!(
        diags.len(),
        1,
        "expected exactly 1 diagnostic, got {diags:#?}"
    );
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

#[test]
fn top_level_workspace_member_src_is_walked_and_flagged() {
    // Regression test for Copilot R1 #1 / R8 #1: the previous walker
    // only visited `crates/*/src` + `crates/*/tests`, missing the
    // top-level `marque/` binary crate. This fixture creates a
    // `<member>/Cargo.toml` + `<member>/src/lib.rs` pair (mimicking
    // the workspace's `marque/` shape) and asserts the call site
    // inside it is detected. Without the top-level-member discovery
    // path in `collect_rust_files`, this call wouldn't be scanned
    // at all and the assert would be `0` instead of `1`.
    let tmp = TempDir::new().unwrap();
    // The directory must contain a `Cargo.toml` for the discovery
    // logic to recognize it as a workspace member.
    write(
        tmp.path(),
        "marque/Cargo.toml",
        "[package]\nname = \"marque\"\n",
    );
    write(
        tmp.path(),
        "marque/src/lib.rs",
        r"
fn naughty_in_top_level_member() {
    let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(
        diags.len(),
        1,
        "expected the top-level member's call to be flagged, got {diags:#?}"
    );
    assert_eq!(diags[0].code, "PRC002");
    assert!(
        diags[0]
            .file
            .to_string_lossy()
            .contains("marque/src/lib.rs"),
        "expected the diagnostic to point at the top-level member; got {:?}",
        diags[0].file
    );
}

#[test]
fn top_level_workspace_member_tests_carve_out_is_recognized() {
    // Companion to the above: a call in `<member>/tests/<...>.rs`
    // with the carve-out comment must be allowed (PRC001 not fired).
    // Verifies BOTH the discovery path AND the corresponding
    // `is_test_path` branch that handles the `<member>/tests/<...>`
    // shape (R6 #1 fix).
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "marque/Cargo.toml",
        "[package]\nname = \"marque\"\n",
    );
    write(
        tmp.path(),
        "marque/tests/integration.rs",
        r"
fn marque_top_level_test_fixture() {
    // Test-fixture carve-out per Constitution V
    let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(
        diags.is_empty(),
        "carve-out comment should silence PRC001 in <member>/tests/; got {diags:#?}"
    );
}

#[test]
fn aliased_import_does_not_bypass_lint() {
    // Regression test for the round-8 audit blocker: a `use ... as ...`
    // import does NOT bypass the lint. Last-segment-only matching on
    // `__engine_promote` / `__engine_construct` ensures any call to
    // those reserved names is caught regardless of the path qualifier
    // — qualified, fully-qualified, aliased, or `Self::`.
    //
    // The function names are deliberately reserved by the project
    // (both are `#[doc(hidden)]` engine-only seal mechanisms with `__`
    // prefixes); a free fn with one of those names is itself a
    // Constitution V Principle V violation and the lint flags
    // accordingly.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
fn naughty_aliased() {
    let _ = AF::__engine_promote((), (), (), false, None, ());
}
fn naughty_self_path() {
    let _ = Self::__engine_promote((), (), (), false, None, ());
}
fn naughty_construct_aliased() {
    let _ = EPT::__engine_construct();
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(
        diags.len(),
        3,
        "expected all three aliased/Self call shapes flagged, got {diags:#?}"
    );
    for d in &diags {
        assert_eq!(d.code, "PRC002");
    }
}

#[test]
fn fully_qualified_marque_rules_path_is_caught() {
    // The fully-qualified `marque_rules::AppliedFix::__engine_promote`
    // form must be caught — extra leading segment doesn't bypass.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
fn naughty_fqn() {
    let _ = marque_rules::AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(
        diags.len(),
        1,
        "expected the FQN call to be flagged, got {diags:#?}"
    );
    assert_eq!(diags[0].code, "PRC002");
}

#[test]
fn engine_promote_legacy_is_not_caught_by_suffix_match() {
    // PR 3c.B Commit 2 regression: `AppliedFix::__engine_promote_legacy`
    // was added as the back-compat path during the FixIntent migration.
    // The reserved-name lint flags calls whose last path-segment is
    // EXACTLY `__engine_promote` (or `__engine_construct`) — anchored
    // on string equality, NOT on prefix containment. The companion
    // method `__engine_promote_legacy` is a distinct identifier and
    // must NOT trip the lint, regardless of its `__engine_promote` prefix.
    //
    // If a future refactor switches `path_ends_with`'s comparison to
    // prefix-match (e.g., `starts_with("__engine_promote")`), this
    // test will fail loudly. The brief for PR 3c.B Commit 2 pins
    // last-segment exact-equality as load-bearing.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
fn not_a_violation() {
    // The legacy-path constructor — distinct identifier from the
    // reserved-name suffix. Must NOT be flagged.
    let _ = AppliedFix::__engine_promote_legacy((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(
        diags.is_empty(),
        "expected no diagnostics on `__engine_promote_legacy` (PRC002 \
         must anchor on exact last-segment equality, not prefix), \
         got {diags:#?}"
    );
}

#[test]
fn engine_promote_proper_name_is_still_caught() {
    // Companion to the test above: this is the exact-name case the
    // lint MUST still catch. Adding a regression test for both ends
    // of the suffix-match contract pins the behavior end-to-end.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
fn naughty_proper_name() {
    let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(
        diags.len(),
        1,
        "expected the exact-name call to be flagged, got {diags:#?}"
    );
    assert_eq!(diags[0].code, "PRC002");
}

#[test]
fn production_allowed_inside_two_pass_fixer_apply_kept_fixes() {
    // PR 7b extracts the two `__engine_promote` calls from
    // `Engine::fix_inner` into `TwoPassFixer::apply_kept_fixes`.
    // The phase-split orchestrator is a private struct in
    // `crates/engine/src/engine.rs`; its `apply_kept_fixes` method
    // is the new authorized production call site. The allow-list
    // must accept it explicitly so the engine-only contract is
    // still mechanically enforced by the lint.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
struct TwoPassFixer;
impl TwoPassFixer {
    fn apply_kept_fixes(&self) {
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
        "expected no diagnostics for TwoPassFixer::apply_kept_fixes, got {diags:#?}",
    );
}

#[test]
fn two_pass_fixer_method_not_on_allow_list_is_denied() {
    // The `TwoPassFixer` allow-list is closed: only `apply_kept_fixes`
    // may call `__engine_promote`. A new method on the same struct
    // calling the promotion API must be rejected, forcing any
    // future fourth-promotion-site addition to be a deliberate
    // amendment to the allow-list — exactly the property the
    // FR-040 contract pins.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
struct TwoPassFixer;
impl TwoPassFixer {
    fn unauthorized_promotion_site(&self) {
        let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(
        diags.len(),
        1,
        "expected exactly 1 diagnostic for unauthorized TwoPassFixer method, got {diags:#?}",
    );
    assert_eq!(diags[0].code, "PRC002");
}

#[test]
fn two_pass_fixer_shadow_type_outside_canonical_file_is_denied() {
    // Copilot round-3 R3-1 regression test: a same-name `TwoPassFixer`
    // type defined in a different file under `crates/engine/src/**`
    // MUST NOT inherit the canonical allow-list. The allow-list is
    // pinned to the single canonical home `crates/engine/src/engine.rs`
    // via the `is_engine_canonical_helper_file` path guard. Without
    // that guard, type-name-only matching on `impl_self_type` would
    // let any shadow type with `apply_kept_fixes` defined anywhere
    // under `crates/engine/src/**` bypass the FR-040 engine-only
    // contract.
    //
    // Mirrors the existing free-fn pin (`ENGINE_FREE_FN_ALLOW_LIST` +
    // `is_engine_canonical_helper_file`): "one allow-list entry, one
    // canonical home." A contributor who genuinely needs to expand
    // the allow-list must do so deliberately by amending both the
    // method list and (if needed) the canonical-path guard — not
    // accidentally by re-using a type name.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/shadow.rs",
        r"
struct TwoPassFixer;
impl TwoPassFixer {
    fn apply_kept_fixes(&self) {
        let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(
        diags.len(),
        1,
        "expected exactly 1 diagnostic for shadow TwoPassFixer in non-canonical file, got {diags:#?}",
    );
    assert_eq!(diags[0].code, "PRC002");
    assert!(
        diags[0]
            .file
            .to_string_lossy()
            .contains("crates/engine/src/shadow.rs"),
        "expected the diagnostic to point at the shadow file; got {:?}",
        diags[0].file,
    );
}
