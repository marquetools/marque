// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for the citation-lint.
//!
//! Each test sets up a synthetic workspace under a `TempDir` with a
//! `crates/synthetic/src/` containing a fixture Rust file and a
//! `crates/capco/docs/CAPCO-2016.md` containing a synthetic CAPCO
//! source. Then runs `lint_workspace` and asserts the right defects
//! were detected (or not).
//!
//! Each defect class is covered by at least one fixture so the lint
//! cannot drift on its own correctness silently.

use std::fs;
use std::path::Path;

use citation_lint::diagnostic::DefectClass;
use citation_lint::lint_workspace;
use tempfile::TempDir;

const SYNTHETIC_CAPCO: &str = include_str!("fixtures/synthetic_capco.md");

fn make_workspace_with(rust_source: &str) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let src_dir = dir.path().join("crates").join("synthetic").join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("lib.rs"), rust_source).unwrap();
    let capco_dir = dir.path().join("crates").join("capco").join("docs");
    fs::create_dir_all(&capco_dir).unwrap();
    fs::write(capco_dir.join("CAPCO-2016.md"), SYNTHETIC_CAPCO).unwrap();
    dir
}

fn lint(dir: &Path) -> Vec<citation_lint::Defect> {
    lint_workspace(dir).expect("lint runs")
}

#[test]
fn valid_citation_produces_no_defects() {
    let body = r#"
        pub struct Foo;
        pub fn make() -> Foo {
            // CAPCO-2016 §H.1 p20 is the classification section.
            Foo
        }
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(defects.is_empty(), "got {defects:#?}");
}

#[test]
fn bare_numeric_section_detected_in_capco_context() {
    let body = r#"
        pub fn x() {
            let _ = "CAPCO-2016 §4 p99";
        }
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects
            .iter()
            .any(|d| matches!(d.class, DefectClass::BareSection)),
        "expected BareSection, got {defects:#?}"
    );
}

#[test]
fn bare_numeric_section_skipped_outside_capco_context() {
    // `§4 of the design doc` should not trip the lint when the
    // surrounding string has no CAPCO anchor.
    let body = r#"
        pub fn x() {
            // §4 of the Phase B design doc explains the lattice.
            let _ = 1;
        }
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(defects.is_empty(), "got {defects:#?}");
}

#[test]
fn non_normative_section_i_detected() {
    let body = r#"
        pub fn x() {
            let _ = "CAPCO-2016 §I.1 p30 banner history";
        }
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects
            .iter()
            .any(|d| matches!(d.class, DefectClass::NonNormativeSection { letter: 'I' })),
        "expected NonNormativeSection(I), got {defects:#?}"
    );
}

#[test]
fn unknown_subsection_detected() {
    let body = r#"
        pub fn x() {
            let _ = "CAPCO-2016 §A.99 p5";
        }
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects.iter().any(|d| matches!(
            d.class,
            DefectClass::UnknownSubsection {
                letter: 'A',
                number: 99
            }
        )),
        "got {defects:#?}"
    );
}

#[test]
fn page_out_of_range_detected() {
    // §A.1 spans pp 5-7 in synthetic. p15 is in section F.
    let body = r#"
        pub fn x() {
            let _ = "CAPCO-2016 §A.1 p15";
        }
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects
            .iter()
            .any(|d| matches!(d.class, DefectClass::PageOutOfRange { .. })),
        "got {defects:#?}"
    );
}

#[test]
fn page_out_of_document_detected() {
    let body = r#"
        pub fn x() {
            let _ = "CAPCO-2016 §A.1 p999";
        }
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects
            .iter()
            .any(|d| matches!(d.class, DefectClass::PageOutOfDocument { .. })),
        "got {defects:#?}"
    );
}

#[test]
fn doubled_page_anchor_detected() {
    let body = r#"
        pub fn x() {
            let _ = "CAPCO-2016 §B.1 p10–11 p11";
        }
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects
            .iter()
            .any(|d| matches!(d.class, DefectClass::DoubledPageAnchor { .. })),
        "got {defects:#?}"
    );
}

#[test]
fn legacy_line_form_detected() {
    let body = r#"
        // CAPCO-2016 §H.1 line 4140 is wrong — line forms are retired.
        pub fn x() {}
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects
            .iter()
            .any(|d| matches!(d.class, DefectClass::LegacyLineForm { .. })),
        "got {defects:#?}"
    );
}

#[test]
fn citation_field_implicit_capco() {
    // No explicit `CAPCO-2016` prefix in the citation field — the
    // field name itself is the CAPCO anchor.
    let body = r#"
        pub struct Constraint {
            pub citation: &'static str,
        }
        pub const C: Constraint = Constraint { citation: "§4 p10" };
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects
            .iter()
            .any(|d| matches!(d.class, DefectClass::BareSection)),
        "expected BareSection on citation:, got {defects:#?}"
    );
}

#[test]
fn message_field_implicit_capco() {
    let body = r#"
        pub struct Diagnostic { pub message: &'static str }
        pub const D: Diagnostic = Diagnostic { message: "§4 p10" };
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects
            .iter()
            .any(|d| matches!(d.class, DefectClass::BareSection)),
        "expected BareSection on message:, got {defects:#?}"
    );
}

#[test]
fn doc_comment_section_letter_form_detected() {
    let body = r#"
        /// Authority: CAPCO-2016 §A.99 — wrong subsection.
        pub fn x() {}
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects
            .iter()
            .any(|d| matches!(d.class, DefectClass::UnknownSubsection { .. })),
        "got {defects:#?}"
    );
}

#[test]
fn letter_only_for_section_with_subsections_detected() {
    let body = r#"
        pub struct C { pub citation: &'static str }
        pub const X: C = C { citation: "CAPCO-2016 §A" };
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects.iter().any(|d| matches!(
            d.class,
            DefectClass::LetterOnlyButSectionHasSubsections { letter: 'A' }
        )),
        "got {defects:#?}"
    );
}

#[test]
fn letter_only_for_section_without_subsections_accepted() {
    // §F has no numbered subsections in the synthetic — `§F`
    // standalone is lawful.
    let body = r#"
        pub struct C { pub citation: &'static str }
        pub const X: C = C { citation: "CAPCO-2016 §F" };
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(defects.is_empty(), "got {defects:#?}");
}

#[test]
fn page_range_form_resolves() {
    let body = r#"
        pub fn x() {
            // §H.4 spans pp 25-30. `pp 25–29` is in range.
            let _ = "CAPCO-2016 §H.4 pp 25–29";
        }
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(defects.is_empty(), "got {defects:#?}");
}

#[test]
fn page_range_partial_overlap_detected() {
    let body = r#"
        pub fn x() {
            // §H.4 spans pp 25-30 in synthetic. p35 is past max — but
            // §H.4 doesn't reach p35 either way.
            let _ = "CAPCO-2016 §H.4 pp 25–35";
        }
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    assert!(
        defects.iter().any(|d| {
            matches!(
                d.class,
                DefectClass::PageOutOfRange { .. } | DefectClass::PageOutOfDocument { .. }
            )
        }),
        "got {defects:#?}"
    );
}

#[test]
fn determinism_across_runs() {
    let body = r#"
        pub fn x() {
            let _ = "CAPCO-2016 §I.1 p30";
            let _ = "CAPCO-2016 §A.99 p5";
            let _ = "CAPCO-2016 §4 p10";
        }
    "#;
    let dir = make_workspace_with(body);
    let a = lint(dir.path());
    let b = lint(dir.path());
    assert_eq!(a, b, "lint output must be deterministic across runs");
}

#[test]
fn fr020_known_defect_classes_all_detected() {
    // Smoke test that all four known defect classes are
    // surfaced by a single source containing all four. (Class (d)
    // — the HCS-P two-sided predicate — is a *predicate* defect
    // that this lint doesn't claim to detect; corpus-fidelity
    // checks cover it.)
    let body = r#"
        // (a) `§4` fabrication for HCS — should be `§H.4`.
        // CAPCO-2016 §4 p64 — wrong section letter.
        pub struct C { pub citation: &'static str }
        pub const A: C = C { citation: "CAPCO-2016 §4 p64" };
        // (b) doubled page anchor.
        pub const B: C = C { citation: "CAPCO-2016 §B.1 p10–11 p11" };
        // (c) legacy line form.
        // CAPCO-2016 §H.1 line 4140 — retired form.
        pub fn x() {}
    "#;
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    let has = |class_id: &str| defects.iter().any(|d| d.class.class_id() == class_id);
    assert!(has("bare-section"), "missing bare-section: {defects:#?}");
    assert!(
        has("doubled-page-anchor"),
        "missing doubled-page-anchor: {defects:#?}"
    );
    assert!(
        has("legacy-line-form"),
        "missing legacy-line-form: {defects:#?}"
    );
}

#[test]
fn column_accuracy_doc_comment() {
    // Guard: verify that doc-comment (`//!`) citations report the
    // correct 1-indexed column for the `§` character.
    //
    // Source line: `//! CAPCO-2016 §A.99 p5`
    // Breakdown:
    //   `//!`          = 3 chars (columns 1–3)
    //   ` CAPCO-2016 ` = 12 chars (columns 4–15)
    //   `§`            at column 16 (1-indexed)
    //
    // Before the fix `compute_line_col` used `+2+1` for all surfaces,
    // which under-counted by 1 for doc-comments (reporting 15 instead
    // of 16). After the fix it uses `prefix_len=3` for DocComment.
    let body = "//! CAPCO-2016 §A.99 p5\npub fn x() {}\n";
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    let doc_defects: Vec<_> = defects
        .iter()
        .filter(|d| matches!(d.source_kind, citation_lint::SourceKind::DocComment))
        .collect();
    assert!(
        !doc_defects.is_empty(),
        "expected a defect from the doc-comment citation, got none; all defects: {defects:#?}"
    );
    let col = doc_defects[0].column;
    assert_eq!(
        col, 16,
        "doc-comment §A.99: expected column 16 (1-indexed), got {col}; \
         pre-fix value would have been 15"
    );
}

#[test]
fn column_accuracy_string_literal() {
    // Guard: verify that string-literal citations report the correct
    // 1-indexed column for the `§` character.
    //
    // Source line: `pub fn x() { let _ = "CAPCO-2016 §A.99 p5"; }`
    // Breakdown:
    //   `pub fn x() { let _ = ` = 21 chars (columns 1–21)
    //   `"`                     at column 22 → span.start().column = 21 (0-indexed)
    //   `CAPCO-2016 `           = 11 chars (columns 23–33)
    //   `§`                     at column 34 (1-indexed)
    //
    // Before the fix `compute_line_col` used `+2+1` for all surfaces,
    // which over-counted by 1 for string literals (reporting 35 instead
    // of 34). After the fix it uses `prefix_len=1` for StringLiteral.
    let body = "pub fn x() { let _ = \"CAPCO-2016 §A.99 p5\"; }\n";
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    let str_defects: Vec<_> = defects
        .iter()
        .filter(|d| matches!(d.source_kind, citation_lint::SourceKind::StringLiteral))
        .collect();
    assert!(
        !str_defects.is_empty(),
        "expected a defect from the string-literal citation, got none; all defects: {defects:#?}"
    );
    let col = str_defects[0].column;
    assert_eq!(
        col, 34,
        "string-literal §A.99: expected column 34 (1-indexed), got {col}; \
         pre-fix value would have been 35"
    );
}

#[test]
fn column_accuracy_explicit_doc_attr() {
    // Guard: verify that explicit `#[doc = "..."]` attributes report the
    // correct 1-indexed column for the `§` character.
    //
    // Source line: `#[doc = "CAPCO-2016 §A.99 p5"]`
    // Breakdown:
    //   `#[doc = "`  = 9 chars (columns 1–9)
    //   `CAPCO-2016 ` = 11 chars (columns 10–20)
    //   `§`           at column 21 (1-indexed)
    //
    // An explicit `#[doc = "..."]` attribute has its LitStr span at the
    // opening `"` (column 8, 0-indexed), so prefix_len must be 1 — the
    // same as a regular string literal. Without the desugared-vs-explicit
    // detection, `prefix_len = 3` would report column 23 (over by 2).
    let body = "#[doc = \"CAPCO-2016 §A.99 p5\"]\npub fn x() {}\n";
    let dir = make_workspace_with(body);
    let defects = lint(dir.path());
    let doc_defects: Vec<_> = defects
        .iter()
        .filter(|d| matches!(d.source_kind, citation_lint::SourceKind::DocComment))
        .collect();
    assert!(
        !doc_defects.is_empty(),
        "expected a defect from the explicit doc attr citation; all defects: {defects:#?}"
    );
    let col = doc_defects[0].column;
    assert_eq!(
        col, 21,
        "explicit #[doc = ...] §A.99: expected column 21 (1-indexed), got {col}; \
         without desugared detection, prefix_len=3 would give 23"
    );
}

#[test]
fn scan_workspace_includes_top_level_marque_crate() {
    // Regression guard: previously
    // `scan_workspace` only walked `crates/*/src/**`, which silently
    // missed citations in the top-level `marque/` CLI binary crate
    // (and any future top-level workspace member). Verify the
    // widened logic now picks up marque/ by checking that at least
    // one occurrence has a path containing `marque/src/`.
    use std::path::PathBuf;
    let workspace_root: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // tools/
        .and_then(|p| p.parent()) // workspace root
        .unwrap()
        .to_path_buf();
    // Skip if the marque/ dir is not present (e.g., partial checkout).
    if !workspace_root.join("marque").join("src").is_dir() {
        eprintln!(
            "skipping: workspace_root {:?} has no marque/src/",
            workspace_root
        );
        return;
    }
    let (occurrences, _legacy) =
        citation_lint::scan_workspace(&workspace_root).expect("scan_workspace");
    let marque_paths: Vec<_> = occurrences
        .iter()
        .filter(|o| o.file.components().any(|c| c.as_os_str() == "marque"))
        .collect();
    assert!(
        !marque_paths.is_empty(),
        "expected at least one scanned occurrence under marque/src/; \
         scan_workspace must include top-level workspace members, not just \
         crates/*. Total occurrences: {}",
        occurrences.len()
    );
}
