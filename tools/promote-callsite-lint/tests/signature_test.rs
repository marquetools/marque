// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for the D12 / R-11 signature-shape lint.
//!
//! Each test materializes a synthetic workspace under a
//! `tempfile::TempDir`, drops one source file at a path that
//! exercises a whitelist (or non-whitelist) classification, and
//! asserts the lint pass produces the expected diagnostics.

use std::fs;
use std::path::Path;

use promote_callsite_lint::{Severity, signature};
use tempfile::TempDir;

fn write(tmp: &Path, rel: &str, contents: &str) {
    let path = tmp.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, contents).unwrap();
}

#[test]
fn whitelist_marking_scheme_canonicalize_qualified_path_is_allowed() {
    // The canonical `MarkingScheme` trait declaration lives at
    // `crates/scheme/src/scheme.rs`; the lint recognizes that path
    // specifically. The carve-out at `impl <Trait> for X` sites
    // requires the fully-qualified `marque_scheme::MarkingScheme`
    // path — bare `MarkingScheme` is rejected to close the
    // shadow-trait bypass.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/scheme/src/scheme.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
pub trait MarkingScheme {
    fn canonicalize(&self, parsed: ParsedAttrs) -> CanonicalAttrs;
}
",
    );
    write(
        tmp.path(),
        "crates/capco/src/scheme.rs",
        r"
struct FooScheme;
struct ParsedAttrs;
struct CanonicalAttrs;
impl marque_scheme::MarkingScheme for FooScheme {
    fn canonicalize(&self, parsed: ParsedAttrs) -> CanonicalAttrs {
        let _ = parsed;
        CanonicalAttrs
    }
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn shadow_marking_scheme_trait_in_unrelated_path_is_denied() {
    // A trait merely *named* `MarkingScheme` declared outside
    // `crates/scheme/src/` is suspicious — it could be a shadowing
    // bypass attempt — and must be flagged. This is the bypass that
    // accepting the bare single-segment trait path would have left
    // open at the impl site (closed by `is_marking_scheme_trait_path`)
    // and at the trait declaration site (closed by
    // `rel_path_is_marque_scheme_src`).
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
pub trait MarkingScheme {
    fn canonicalize(&self, parsed: ParsedAttrs) -> CanonicalAttrs;
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1, "expected exactly one PRC100, got {diags:#?}");
    assert_eq!(diags[0].code, "PRC100");
}

#[test]
fn impl_marking_scheme_bare_path_is_denied() {
    // `impl MarkingScheme for X` (bare, single-segment) MUST NOT
    // match the carve-out — see `is_marking_scheme_trait_path`.
    // The contributor must write `impl marque_scheme::MarkingScheme`.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
struct FooScheme;
trait MarkingScheme {
    fn canonicalize(&self, parsed: ParsedAttrs) -> CanonicalAttrs;
}
impl MarkingScheme for FooScheme {
    fn canonicalize(&self, parsed: ParsedAttrs) -> CanonicalAttrs {
        let _ = parsed;
        CanonicalAttrs
    }
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    // Expect at least the trait declaration to flag (PRC100). The
    // impl method's signature also flags because the bare-path
    // carve-out is rejected.
    assert!(
        diags.iter().any(|d| d.code == "PRC100"),
        "expected PRC100 to flag the bare-path impl, got {diags:#?}"
    );
}

#[test]
fn whitelist_unsafe_fn_is_allowed() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
pub unsafe fn rough_cast(p: ParsedAttrs) -> CanonicalAttrs {
    let _ = p;
    CanonicalAttrs
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn whitelist_transitional_from_parsed_unchecked_is_allowed() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/ism/src/canonical.rs",
        r"
pub struct ParsedAttrs;
pub struct CanonicalAttrs;
pub fn from_parsed_unchecked(p: ParsedAttrs) -> CanonicalAttrs {
    let _ = p;
    CanonicalAttrs
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn non_whitelisted_shape_is_denied() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
pub fn shady(p: ParsedAttrs) -> CanonicalAttrs {
    let _ = p;
    CanonicalAttrs
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1, "expected one diagnostic, got {diags:#?}");
    assert_eq!(diags[0].code, "PRC100");
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(
        diags[0].message.contains("shady"),
        "diagnostic message should name the offending fn: {}",
        diags[0].message
    );
}

#[test]
fn result_wrapped_canonical_is_denied() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
pub struct ParseError;
pub fn maybe_canonicalize(p: ParsedAttrs) -> Result<CanonicalAttrs, ParseError> {
    let _ = p;
    Ok(CanonicalAttrs)
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "PRC100");
}

#[test]
fn reference_arg_still_matched() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
pub fn from_ref(p: &ParsedAttrs) -> CanonicalAttrs {
    let _ = p;
    CanonicalAttrs
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "PRC100");
}

#[test]
fn missing_parsed_is_not_matched() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
struct CanonicalAttrs;
pub fn returns_only(s: &str) -> CanonicalAttrs {
    let _ = s;
    CanonicalAttrs
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn missing_canonical_is_not_matched() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
struct ParsedAttrs;
pub fn just_parses(p: ParsedAttrs) -> u32 {
    let _ = p;
    0
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
}

#[test]
fn from_parsed_unchecked_outside_whitelist_path_is_denied() {
    // Function name alone does not unlock the whitelist; it's
    // path-keyed to `crates/ism/src/canonical.rs`.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
pub fn from_parsed_unchecked(p: ParsedAttrs) -> CanonicalAttrs {
    let _ = p;
    CanonicalAttrs
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "PRC100");
}

#[test]
fn canonicalize_outside_marking_scheme_impl_is_denied() {
    // Method name alone does not unlock the whitelist; it must be
    // inside an `impl MarkingScheme for ...` block.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
pub fn canonicalize(p: ParsedAttrs) -> CanonicalAttrs {
    let _ = p;
    CanonicalAttrs
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "PRC100");
}

#[test]
fn top_level_workspace_member_src_is_walked_and_flagged() {
    // Regression test for Copilot R5 #2 / R8 #3: the signature pass
    // previously only walked `crates/**`, missing top-level workspace
    // members like `marque/`. A future
    // `ParsedAttrs -> CanonicalAttrs` adapter added in `marque/src/`
    // would have bypassed PRC100 entirely. Construct a top-level
    // `<member>/Cargo.toml` + `<member>/src/lib.rs` and assert the
    // prohibited shape is detected.
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "marque/Cargo.toml", "[package]\nname = \"marque\"\n");
    write(
        tmp.path(),
        "marque/src/lib.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
pub fn naughty(p: ParsedAttrs) -> CanonicalAttrs {
    let _ = p;
    CanonicalAttrs
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1, "expected the top-level member's signature to be flagged, got {diags:#?}");
    assert_eq!(diags[0].code, "PRC100");
    assert!(
        diags[0].file.to_string_lossy().contains("marque/src/lib.rs"),
        "expected the diagnostic to point at the top-level member; got {:?}",
        diags[0].file
    );
}

#[test]
fn top_level_workspace_member_tests_walked_too() {
    // Companion: signature pass also walks `<member>/tests/`.
    // A `ParsedAttrs -> CanonicalAttrs` shape declared inside a
    // top-level integration test must still be flagged because the
    // test-fixture carve-out is callsite-scoped (PRC001), not
    // signature-scoped (PRC100). The shape itself is the prohibited
    // architectural pattern regardless of test-vs-production scope.
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "marque/Cargo.toml", "[package]\nname = \"marque\"\n");
    write(
        tmp.path(),
        "marque/tests/it.rs",
        r"
struct ParsedAttrs;
struct CanonicalAttrs;
pub fn naughty(p: ParsedAttrs) -> CanonicalAttrs {
    let _ = p;
    CanonicalAttrs
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert_eq!(diags.len(), 1, "expected the top-level member's tests/ signature to be flagged, got {diags:#?}");
    assert_eq!(diags[0].code, "PRC100");
}

#[test]
fn whitelist_marking_scheme_canonicalize_bare_path_with_use_import_is_allowed() {
    // Regression test for Copilot R9 #2: the codebase's established
    // adapter convention is `use marque_scheme::{...};
    // impl MarkingScheme for X { ... }` (see
    // `crates/capco/src/scheme.rs:26 + :1267` for the canonical
    // example). The PRC100 carve-out must accept this bare form
    // when the file imports `MarkingScheme` from `marque_scheme`,
    // OR PR 3a–3c's adapter rewrite would falsely flag the
    // legitimate `canonicalize` impl. Imports-aware matching closes
    // both the false-positive (legitimate adapter) and the
    // shadow-trait bypass (a crate-local `trait MarkingScheme`
    // declared in some other file would lack the use-import).
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/capco/src/scheme.rs",
        r"
use marque_scheme::MarkingScheme;
struct FooScheme;
struct ParsedAttrs;
struct CanonicalAttrs;
impl MarkingScheme for FooScheme {
    fn canonicalize(&self, parsed: ParsedAttrs) -> CanonicalAttrs {
        let _ = parsed;
        CanonicalAttrs
    }
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert!(
        diags.is_empty(),
        "bare-form impl with `use marque_scheme::MarkingScheme` import should be accepted; got {diags:#?}"
    );
}

#[test]
fn whitelist_marking_scheme_canonicalize_bare_path_with_glob_import_is_allowed() {
    // `use marque_scheme::*;` glob also satisfies the
    // imports-aware bare-form acceptance.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/capco/src/scheme.rs",
        r"
use marque_scheme::*;
struct FooScheme;
struct ParsedAttrs;
struct CanonicalAttrs;
impl MarkingScheme for FooScheme {
    fn canonicalize(&self, parsed: ParsedAttrs) -> CanonicalAttrs {
        let _ = parsed;
        CanonicalAttrs
    }
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert!(
        diags.is_empty(),
        "bare-form impl with `use marque_scheme::*` glob should be accepted; got {diags:#?}"
    );
}

#[test]
fn whitelist_marking_scheme_canonicalize_bare_path_with_group_import_is_allowed() {
    // `use marque_scheme::{TokenId, MarkingScheme};` group also
    // satisfies the imports-aware bare-form acceptance — this is
    // the actual shape used in `crates/capco/src/scheme.rs:26`.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/capco/src/scheme.rs",
        r"
use marque_scheme::{Category, MarkingScheme, TokenId};
struct FooScheme;
struct ParsedAttrs;
struct CanonicalAttrs;
impl MarkingScheme for FooScheme {
    fn canonicalize(&self, parsed: ParsedAttrs) -> CanonicalAttrs {
        let _ = parsed;
        CanonicalAttrs
    }
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert!(
        diags.is_empty(),
        "bare-form impl with grouped `use` import should be accepted; got {diags:#?}"
    );
}

#[test]
fn bare_marking_scheme_with_renamed_import_is_denied() {
    // `use marque_scheme::MarkingScheme as Foo;` does NOT satisfy
    // the bare-form acceptance: the local name is `Foo`, not
    // `MarkingScheme`. A `impl MarkingScheme for X` in such a file
    // refers to some OTHER `MarkingScheme` (likely a shadow trait),
    // not the canonical one. The lint correctly rejects.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/foo/src/lib.rs",
        r"
use marque_scheme::MarkingScheme as MS;
struct ParsedAttrs;
struct CanonicalAttrs;
struct FooScheme;
trait MarkingScheme {
    fn canonicalize(&self, parsed: ParsedAttrs) -> CanonicalAttrs;
}
impl MarkingScheme for FooScheme {
    fn canonicalize(&self, parsed: ParsedAttrs) -> CanonicalAttrs {
        let _ = parsed;
        CanonicalAttrs
    }
}
",
    );
    let diags = signature::scan_workspace(tmp.path()).unwrap();
    assert!(
        diags.iter().any(|d| d.code == "PRC100"),
        "rename-aliased import must NOT trigger bare-form acceptance; expected PRC100, got {diags:#?}"
    );
}
