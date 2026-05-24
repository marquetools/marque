// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for the base call-site lint.
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
    // `Engine::fix_inner` is the marking-fix gate. Under the per-
    // reserved-name allow-list, it MAY call `__engine_promote` but
    // NOT `__engine_construct` directly — token mints route through
    // the `engine_promotion_token()` free helper (its own canonical
    // home). See `production_denied_engine_fix_inner_calling_construct`
    // for the negative half of this contract.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
struct Engine;
impl Engine {
    fn fix_inner(&self) {
        let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn production_allowed_inside_apply_text_corrections() {
    // Per-reserved-name allow-list: `apply_text_corrections` is the
    // text-correction gate and MAY call
    // `__engine_promote_text_correction`. Calling `__engine_promote`
    // (the marking-fix promoter) from `apply_text_corrections` is
    // explicitly denied — see
    // `production_denied_apply_text_corrections_calling_promote_marking`.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
struct Engine;
impl Engine {
    fn apply_text_corrections(&self) {
        let _ = AppliedTextCorrection::__engine_promote_text_correction((), (), (), false, None, ());
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn production_denied_apply_text_corrections_calling_promote_marking() {
    // Per-reserved-name allow-list. Even though
    // `apply_text_corrections` is in the `Engine` allow-list for
    // `__engine_promote_text_correction`, it MUST NOT call
    // `__engine_promote` (the marking-fix promoter). The two audit-
    // record types are disjoint per Constitution V Principle V; the
    // lint reflects that disjointness.
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
    assert_eq!(diags.len(), 1, "expected exactly 1 diagnostic, got {diags:#?}");
    assert_eq!(diags[0].code, "PRC002");
}

#[test]
fn production_denied_engine_fix_inner_calling_promote_text_correction() {
    // Symmetric to the test above: `Engine::fix_inner` is the
    // marking-fix gate and MUST NOT mint a text-correction record.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
struct Engine;
impl Engine {
    fn fix_inner(&self) {
        let _ = AppliedTextCorrection::__engine_promote_text_correction((), (), (), false, None, ());
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1, "expected exactly 1 diagnostic, got {diags:#?}");
    assert_eq!(diags[0].code, "PRC002");
}

#[test]
fn production_denied_engine_fix_inner_calling_construct_directly() {
    // Per-reserved-name allow-list: `__engine_construct` calls in
    // `Engine` methods are denied. Token mints route through the
    // `engine_promotion_token()` free helper (canonical home for
    // the `EnginePromotionToken::__engine_construct` call); the
    // `EngineConstructor::__engine_construct` Canonical-builder
    // mint lives in `TwoPassFixer::apply_kept_fixes`. Neither path
    // sits inside an `Engine` method.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
struct Engine;
impl Engine {
    fn fix_inner(&self) {
        let _ = EnginePromotionToken::__engine_construct();
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1, "expected exactly 1 diagnostic, got {diags:#?}");
    assert_eq!(diags[0].code, "PRC002");
}

#[test]
fn production_denied_engine_shadow_struct_in_other_file() {
    // Copilot visible #3: canonical-path guard for the `Engine`
    // allow-list. A shadow `struct Engine { ... } impl Engine { fn
    // fix_inner(...) }` defined in a file OTHER than the canonical
    // `crates/engine/src/engine.rs` MUST NOT inherit the allow-list.
    // Mirrors the existing `two_pass_fixer_shadow_type_outside_canonical_file_is_denied`
    // test for the `TwoPassFixer` allow-list.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/elsewhere.rs",
        r"
struct Engine;
impl Engine {
    fn fix_inner(&self) {
        let _ = AppliedFix::__engine_promote((), (), (), false, None, ());
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1, "expected exactly 1 diagnostic, got {diags:#?}");
    assert_eq!(diags[0].code, "PRC002");
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
    // Regression test: the previous walker
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
    // shape.
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
    // Regression test: a `use ... as ...`
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
    // Regression: `AppliedFix::__engine_promote_legacy`
    // was once a back-compat path during the FixIntent migration.
    // The reserved-name lint flags calls whose last path-segment is
    // EXACTLY `__engine_promote` (or `__engine_construct`) — anchored
    // on string equality, NOT on prefix containment. The companion
    // method `__engine_promote_legacy` is a distinct identifier and
    // must NOT trip the lint, regardless of its `__engine_promote` prefix.
    //
    // If a future refactor switches `path_ends_with`'s comparison to
    // prefix-match (e.g., `starts_with("__engine_promote")`), this
    // test will fail loudly: last-segment exact-equality is
    // load-bearing.
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
    // The two `__engine_promote` calls live in
    // `TwoPassFixer::apply_kept_fixes` (extracted from `Engine::fix_inner`).
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
    // engine-only contract pins.
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
    // Regression test: a same-name `TwoPassFixer`
    // type defined in a different file under `crates/engine/src/**`
    // MUST NOT inherit the canonical allow-list. The allow-list is
    // pinned to the single canonical home `crates/engine/src/engine.rs`
    // via the `is_engine_canonical_helper_file` path guard. Without
    // that guard, type-name-only matching on `impl_self_type` would
    // let any shadow type with `apply_kept_fixes` defined anywhere
    // under `crates/engine/src/**` bypass the engine-only
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

#[test]
fn engine_promote_text_correction_proper_name_is_caught() {
    // The
    // `AppliedTextCorrection::__engine_promote_text_correction`
    // reserved name (the text-correction split) MUST be flagged
    // when called from non-engine, non-test code — same
    // engine-only contract as `__engine_promote`. The matcher uses
    // exact-equality on the last path segment, so this name is
    // explicitly enumerated alongside `__engine_promote` and
    // `__engine_construct`.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
fn naughty_text_correction_call() {
    let _ = AppliedTextCorrection::__engine_promote_text_correction(
        (), (), (), (), (), (), (), false, None, (),
    );
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(
        diags.len(),
        1,
        "expected the text-correction call to be flagged, got {diags:#?}"
    );
    assert_eq!(diags[0].code, "PRC002");
}

#[test]
fn engine_promote_text_correction_production_allowed_inside_apply_text_corrections() {
    // The production carve-out for
    // `__engine_promote_text_correction` is `Engine::apply_text_corrections`.
    // The existing `ENGINE_METHOD_ALLOW_LIST`
    // already lists `apply_text_corrections`, and the matcher now
    // recognizes the new reserved name. End-to-end the call site
    // must be allowed.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
struct Engine;
impl Engine {
    fn apply_text_corrections(&self) {
        let _ = AppliedTextCorrection::__engine_promote_text_correction(
            (), (), (), (), (), (), (), false, None, (),
        );
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(
        diags.is_empty(),
        "expected no diagnostics for apply_text_corrections, got {diags:#?}",
    );
}

#[test]
fn engine_promote_text_correction_test_fixture_carve_out_honored() {
    // The Constitution V Principle V test-
    // fixture carve-out applies symmetrically to
    // `__engine_promote_text_correction`. A call site inside a
    // `tests/` integration file with the marker comment within five
    // lines above must be silenced (same lookback window as the
    // marking-side `__engine_promote`).
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/tests/audit_test.rs",
        r"
fn fabricate_leaky_text_correction() {
    // Test-fixture carve-out per Constitution V
    let _ = AppliedTextCorrection::__engine_promote_text_correction(
        (), (), (), (), (), (), (), false, None, (),
    );
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn engine_promote_text_correction_test_fixture_denied_without_marker() {
    // Companion to the carve-out-honored test: without the marker
    // comment, the lint MUST fire PRC001 just like it does for
    // `__engine_promote`. Otherwise the new reserved name would
    // open a silent test-fixture-unmarked bypass of the carve-out.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/tests/audit_test.rs",
        r"
fn fabricate_leaky_text_correction() {
    let _ = AppliedTextCorrection::__engine_promote_text_correction(
        (), (), (), (), (), (), (), false, None, (),
    );
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert_eq!(
        diags.len(),
        1,
        "expected exactly 1 PRC001 diagnostic, got {diags:#?}"
    );
    assert_eq!(diags[0].code, "PRC001");
}

#[test]
fn engine_promote_text_correction_legacy_suffix_not_caught() {
    // Mirrors `engine_promote_legacy_is_not_caught_by_suffix_match`
    // for the new reserved name. The matcher uses exact-equality on
    // the last path segment — a back-compat name like
    // `__engine_promote_text_correction_legacy` is a distinct
    // identifier and MUST NOT trip the lint, even though it shares
    // the `__engine_promote_text_correction` prefix. The exact-match
    // discipline keeps the closed reserved-name list deliberate.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
fn not_a_violation_legacy_text_correction() {
    // Hypothetical future legacy back-compat name; distinct from the
    // reserved suffix and must NOT be flagged.
    let _ = AppliedTextCorrection::__engine_promote_text_correction_legacy(
        (), (), (), (), (), (), (), false, None, (),
    );
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(
        diags.is_empty(),
        "expected no diagnostics on `__engine_promote_text_correction_legacy` \
         (matcher must anchor on exact last-segment equality, not prefix), \
         got {diags:#?}"
    );
}

#[test]
#[ignore = "documents a known syntactic-lint gap; mitigated at type-system + dep-review layers"]
fn known_gap_pirate_path_inside_allow_listed_fn_bypasses() {
    // User scenario (2026-05-20 "thinking out loud"): a fully-qualified
    // call to a non-marque path whose **last segment** is `__engine_promote`,
    // called from inside an allow-listed enclosing fn, is currently
    // NOT flagged by this lint. The lint matches by path-last-segment +
    // enclosing-fn-classification, both syntactic; it has no name-
    // resolution stage and cannot tell that the call resolves to a
    // third-party function rather than the real
    // `marque_rules::AppliedFix::__engine_promote`.
    //
    // Why this is acceptable defense-in-depth:
    //   1. The real `__engine_promote` takes a sealed
    //      `EnginePromotionToken` argument — minted only via
    //      `EnginePromotionToken::__engine_construct`, itself caught by
    //      the lint. A pirate fn wanting to forge an `AppliedFix`
    //      would either need to call the real constructor (its own
    //      call site is then caught) or use `mem::transmute` (a much
    //      louder Constitution V Principle V violation).
    //   2. `AppliedFix` is `#[non_exhaustive]` and has no other
    //      constructor; a pirate path cannot fabricate one from
    //      scratch.
    //   3. The `audit_g13_canary` end-to-end NDJSON test scans every
    //      audit-stream record for content-leak signatures, catching
    //      a malicious pirate-record at runtime regardless of how the
    //      pirate fn was named.
    //   4. New workspace dependencies require deny-list / license
    //      review (cargo deny) at PR time.
    //
    // `#[ignore]` keeps the assertion off the CI critical path while
    // pinning the behavior — if a future hardening pass adds path-
    // source restriction, flip the assertion and remove `#[ignore]`.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/engine/src/engine.rs",
        r"
struct Engine;
impl Engine {
    fn fix_inner(&self) {
        let _ = arrrrrrr_pirate::Engine::__engine_promote((), (), (), false, None, ());
    }
}
",
    );
    let diags = callsite::scan_workspace(tmp.path()).unwrap();
    assert!(
        diags.is_empty(),
        "documented gap: pirate path inside allow-listed fn is not currently flagged, \
         got {diags:#?}"
    );
}
