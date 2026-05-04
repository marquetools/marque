// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for the AST scanner.
//!
//! Each test sets up a synthetic workspace under a `TempDir` containing a
//! `crates/<name>/tests/` directory, copies one or more fixture files in,
//! and runs `scan_workspace`. The fixture files live at
//! `tests/fixtures/*.rs` next to this file; we copy rather than load
//! directly so the scanner sees a directory structure equivalent to a real
//! marque crate.

use std::fs;
use std::path::Path;

use masking_pin_lint::pin::PinKind;
use masking_pin_lint::scanner::scan_workspace;
use tempfile::TempDir;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("reading fixture {}", path.display()))
}

fn make_workspace_with(fixtures: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let crate_tests = dir.path().join("crates").join("synthetic").join("tests");
    fs::create_dir_all(&crate_tests).unwrap();
    for (name, body) in fixtures {
        fs::write(crate_tests.join(name), body).unwrap();
    }
    dir
}

#[test]
fn valid_masking_pin_classifies_as_masking() {
    let body = fixture("valid_masking.rs");
    let dir = make_workspace_with(&[("valid_masking.rs", &body)]);
    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 2, "should find both pins; got {pins:#?}");
    let masking_count = pins
        .iter()
        .filter(|p| matches!(p.kind, PinKind::Masking { .. }))
        .count();
    let intentional_count = pins
        .iter()
        .filter(|p| matches!(p.kind, PinKind::IntentionalStrict { .. }))
        .count();
    assert_eq!(masking_count, 1);
    assert_eq!(intentional_count, 1);
    // Verify issue extraction.
    let masking = pins
        .iter()
        .find_map(|p| match &p.kind {
            PinKind::Masking { issue, reason } => Some((*issue, reason.clone())),
            _ => None,
        })
        .unwrap();
    assert_eq!(masking.0, 258);
    assert!(masking.1.contains("integration test"));
}

#[test]
fn unmarked_pin_classifies_as_unmarked() {
    let body = fixture("unmarked_pin.rs");
    let dir = make_workspace_with(&[("unmarked.rs", &body)]);
    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 1);
    assert!(matches!(pins[0].kind, PinKind::Unmarked));
}

#[test]
fn both_markers_classifies_as_both() {
    let body = fixture("both_markers.rs");
    let dir = make_workspace_with(&[("both.rs", &body)]);
    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 1);
    assert!(matches!(pins[0].kind, PinKind::BothMarkers));
}

#[test]
fn malformed_marker_classifies_as_bad_format() {
    let body = fixture("bad_format.rs");
    let dir = make_workspace_with(&[("bad.rs", &body)]);
    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 1);
    assert!(
        matches!(pins[0].kind, PinKind::BadFormat(_)),
        "expected BadFormat, got {:?}",
        pins[0].kind
    );
}

#[test]
fn boundary_5_line_window() {
    let body = fixture("boundary_window.rs");
    let dir = make_workspace_with(&[("boundary.rs", &body)]);
    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 2, "should find 2 pins; got {pins:#?}");
    // First pin: marker 4 lines above the call → within window.
    // Second pin: marker 7 lines above → outside window → unmarked.
    let mut sorted = pins.clone();
    sorted.sort_by_key(|p| p.line);
    assert!(
        matches!(&sorted[0].kind, PinKind::Masking { issue, .. } if *issue == 100),
        "first pin (line {}) should be Masking #100, got {:?}",
        sorted[0].line,
        sorted[0].kind
    );
    assert!(
        matches!(sorted[1].kind, PinKind::Unmarked),
        "second pin (line {}) should be Unmarked (marker outside window), got {:?}",
        sorted[1].line,
        sorted[1].kind
    );
}

#[test]
fn scanner_skips_non_test_directories() {
    let dir = TempDir::new().unwrap();
    // Put a pin under crates/foo/src/ — should NOT be picked up.
    let src_dir = dir.path().join("crates").join("foo").join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        src_dir.join("lib.rs"),
        fixture("unmarked_pin.rs"),
    )
    .unwrap();
    let pins = scan_workspace(dir.path()).unwrap();
    assert!(pins.is_empty(), "non-test dirs must be ignored: {pins:#?}");
}

#[test]
fn scanner_walks_top_level_tests_dir() {
    let dir = TempDir::new().unwrap();
    let top_tests = dir.path().join("tests");
    fs::create_dir_all(&top_tests).unwrap();
    fs::write(top_tests.join("top.rs"), fixture("unmarked_pin.rs")).unwrap();
    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 1);
}

#[test]
fn issue_number_regex_extraction() {
    // Verifies the regex captures the number out of `tracks #NNN`.
    let body = "use std::sync::Arc;\n\
                fn make() {\n\
                    // MASKING-PIN: tracks #4242 — extraction test\n\
                    foo().with_recognizer(Arc::new(StrictRecognizer::new()));\n\
                }\n";
    let dir = make_workspace_with(&[("ext.rs", body)]);
    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 1);
    if let PinKind::Masking { issue, .. } = &pins[0].kind {
        assert_eq!(*issue, 4242);
    } else {
        panic!("expected Masking, got {:?}", pins[0].kind);
    }
}

#[test]
fn intentional_strict_reason_extraction() {
    let body = "use std::sync::Arc;\n\
                fn make() {\n\
                    // INTENTIONAL-STRICT: pinning to assert decoder-vs-strict drift\n\
                    foo().with_recognizer(Arc::new(StrictRecognizer::new()));\n\
                }\n";
    let dir = make_workspace_with(&[("int.rs", body)]);
    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 1);
    if let PinKind::IntentionalStrict { reason } = &pins[0].kind {
        assert!(reason.contains("drift"), "got reason {reason:?}");
    } else {
        panic!("expected IntentionalStrict, got {:?}", pins[0].kind);
    }
}

#[test]
fn boxed_strict_recognizer_detected() {
    // Recognize Box::new(StrictRecognizer::new()) as well.
    let body = "fn make() {\n\
                    // INTENTIONAL-STRICT: box variant\n\
                    foo().with_recognizer(Box::new(StrictRecognizer::new()));\n\
                }\n";
    let dir = make_workspace_with(&[("box.rs", body)]);
    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 1);
}

#[test]
fn fully_qualified_strict_recognizer_detected() {
    let body = "fn make() {\n\
                    // INTENTIONAL-STRICT: fully-qualified variant\n\
                    foo().with_recognizer(std::sync::Arc::new(marque_engine::StrictRecognizer::new()));\n\
                }\n";
    let dir = make_workspace_with(&[("fq.rs", body)]);
    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 1);
}

#[test]
fn with_recognizer_without_strict_ignored() {
    // A `with_recognizer(SomethingElse)` call should NOT be flagged.
    let body = "fn make() {\n\
                    foo().with_recognizer(Arc::new(DecoderRecognizer::new()));\n\
                }\n";
    let dir = make_workspace_with(&[("decoder.rs", body)]);
    let pins = scan_workspace(dir.path()).unwrap();
    assert!(pins.is_empty(), "non-strict recognizer must not be flagged");
}

#[test]
fn top_level_workspace_member_tests_walked() {
    // Regression test for Copilot R3 #1 / R8 #2: the previous walker
    // recognized only `<workspace>/tests/` and `crates/*/tests/`,
    // missing `<member>/tests/` for top-level workspace members like
    // the `marque/` binary crate. A masking pin under `marque/tests/`
    // would have bypassed FR-039 entirely while CI stayed green.
    //
    // Construct a synthetic top-level member: a directory with both
    // `Cargo.toml` (the marker `collect_test_roots` looks for) and a
    // `tests/` subdirectory containing a fixture that should produce
    // exactly one masking-pin entry.
    let dir = TempDir::new().expect("tempdir");
    let member = dir.path().join("marque");
    fs::create_dir_all(member.join("tests")).unwrap();
    fs::write(
        member.join("Cargo.toml"),
        "[package]\nname = \"marque\"\n",
    )
    .unwrap();
    let body = "fn make() {\n\
                    // MASKING-PIN: tracks #999 — top-level member regression test\n\
                    foo().with_recognizer(Arc::new(StrictRecognizer::new()));\n\
                }\n";
    fs::write(member.join("tests").join("integration.rs"), body).unwrap();

    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(
        pins.len(),
        1,
        "expected the top-level member's pin to be detected, got {pins:#?}"
    );
    assert!(matches!(pins[0].kind, PinKind::Masking { issue: 999, .. }));
    assert!(
        pins[0].file.to_string_lossy().contains("marque/tests/integration.rs"),
        "expected file to point at the top-level member's tests; got {:?}",
        pins[0].file
    );
}

#[test]
fn test_utils_src_is_walked_for_pin_markers() {
    // Regression test for the round-8 audit's test-utils carve-out
    // asymmetry: `crates/test-utils/src/` is logically a test-fixture
    // scope (per Constitution V Principle V's first constraint
    // enumerating "test-utility crates gated as dev-dependencies"
    // alongside tests/ files and #[cfg(test)] modules). A shared
    // helper there that constructs `with_recognizer(StrictRecognizer)`
    // must carry an FR-039 marker just like a pin under
    // `crates/<crate>/tests/`. The prior masking-pin scanner only
    // walked `tests/`-shaped directories, leaving this scope
    // unscanned — a divergence from `promote-callsite-lint` that
    // would let a future helper at `crates/test-utils/src/...` bypass
    // FR-039 entirely.
    let dir = TempDir::new().expect("tempdir");
    let test_utils_src = dir.path().join("crates").join("test-utils").join("src");
    fs::create_dir_all(&test_utils_src).unwrap();
    let body = "fn build_strict_engine() {\n\
                    // INTENTIONAL-STRICT: shared test helper\n\
                    foo().with_recognizer(Arc::new(StrictRecognizer::new()));\n\
                }\n";
    fs::write(test_utils_src.join("helpers.rs"), body).unwrap();

    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(
        pins.len(),
        1,
        "expected the test-utils helper's pin to be detected, got {pins:#?}"
    );
    assert!(matches!(pins[0].kind, PinKind::IntentionalStrict { .. }));
    assert!(
        pins[0].file.to_string_lossy().contains("crates/test-utils/src/helpers.rs"),
        "expected file to point at crates/test-utils/src; got {:?}",
        pins[0].file
    );
}

#[test]
fn test_utils_src_unmarked_pin_fails() {
    // Companion: a pin in test-utils WITHOUT a marker fails the lint
    // (returns `Unmarked`), exactly like a pin in `crates/<crate>/tests/`.
    let dir = TempDir::new().expect("tempdir");
    let test_utils_src = dir.path().join("crates").join("test-utils").join("src");
    fs::create_dir_all(&test_utils_src).unwrap();
    let body = "fn build_strict_engine() {\n\
                    foo().with_recognizer(Arc::new(StrictRecognizer::new()));\n\
                }\n";
    fs::write(test_utils_src.join("helpers.rs"), body).unwrap();

    let pins = scan_workspace(dir.path()).unwrap();
    assert_eq!(pins.len(), 1);
    assert!(matches!(pins[0].kind, PinKind::Unmarked));
}

#[test]
fn other_utils_named_crates_are_not_special_scoped() {
    // The carve-out is scoped to the literal `test-utils` directory
    // name, NOT to any `*-utils` crate. A future production utility
    // crate (e.g. `format-utils`, `parse-utils`) must not
    // accidentally inherit the test-fixture scope. Verify by placing
    // a strict-recognizer call inside `crates/format-utils/src/` —
    // because `src/` is NOT a registered scan root for that crate
    // (only `tests/` is), the scanner should NOT pick up the call.
    let dir = TempDir::new().expect("tempdir");
    let production_utils_src = dir.path().join("crates").join("format-utils").join("src");
    fs::create_dir_all(&production_utils_src).unwrap();
    let body = "fn build_strict_engine() {\n\
                    foo().with_recognizer(Arc::new(StrictRecognizer::new()));\n\
                }\n";
    fs::write(production_utils_src.join("lib.rs"), body).unwrap();

    let pins = scan_workspace(dir.path()).unwrap();
    assert!(
        pins.is_empty(),
        "expected NO pins from a non-test-utils crate's src/, got {pins:#?}"
    );
}
