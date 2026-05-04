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
fn whitelist_marking_scheme_canonicalize_is_allowed() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "crates/capco/src/scheme.rs",
        r"
struct FooScheme;
struct ParsedAttrs;
struct CanonicalAttrs;
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
    assert!(diags.is_empty(), "expected no diagnostics, got {diags:#?}");
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
        "crates/ism/src/attrs.rs",
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
    // path-keyed to `crates/ism/src/attrs.rs`.
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
