// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! Shared test utilities for the marque workspace.
//!
//! Provides uniform access to `tests/corpus/` fixtures from any crate's test suite.
//! Add this crate as a `[dev-dependencies]` path dependency.

pub mod fixtures;

/// Shared minimal second [`MarkingScheme`](marque_scheme::scheme::MarkingScheme)
/// fixture for Phase B generic-surface tests. See the [`stub_scheme`]
/// module docs.
pub mod stub_scheme;

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Root of the test corpus relative to the workspace root.
const CORPUS_REL: &str = "tests/corpus";

/// Returns the absolute path to the corpus root directory.
///
/// Resolves relative to `CARGO_MANIFEST_DIR`'s ancestor that contains `tests/corpus/`.
/// Works from any crate in the workspace.
pub fn corpus_root() -> PathBuf {
    // Walk up from CARGO_MANIFEST_DIR until we find the workspace root
    // (identified by the presence of tests/corpus/).
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set — run via cargo");
    let mut dir = PathBuf::from(&manifest_dir);
    loop {
        let candidate = dir.join(CORPUS_REL);
        if candidate.is_dir() {
            return candidate;
        }
        if !dir.pop() {
            panic!(
                "could not find {CORPUS_REL}/ in any ancestor of {manifest_dir}; \
                 is the workspace root missing tests/corpus/?"
            );
        }
    }
}

/// Returns paths to all `.txt` fixture files under the given corpus subdirectory.
pub fn fixtures_in(subdir: &str) -> Vec<PathBuf> {
    let dir = corpus_root().join(subdir);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("failed to read corpus directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    paths.sort();
    paths
}

/// Returns all invalid (known-bad) fixture paths.
pub fn invalid_fixtures() -> Vec<PathBuf> {
    fixtures_in("invalid")
}

/// Returns all valid (known-good) fixture paths.
pub fn valid_fixtures() -> Vec<PathBuf> {
    fixtures_in("valid")
}

/// Returns all prose corpus fixture paths.
pub fn prose_fixtures() -> Vec<PathBuf> {
    fixtures_in("prose")
}

/// Expected rule identifier from a `.expected.json` sidecar file.
///
/// `RuleId` has the canonical `(scheme, predicate_id)` 2-tuple form.
/// The structured JSON shape (object form, never a flattened string)
/// makes the rule identity human-readable in audit-record fixtures: a
/// 2030 auditor reading a 2026 sidecar can see `{"scheme": "capco",
/// "predicate_id": "portion.dissem.noforn-conflicts-rel-to"}` directly
/// and trace it to the CAPCO §-citation without consulting a glossary.
///
/// Owned `String` storage (rather than `&'static str`) because the
/// values come from runtime JSON deserialization — corpus fixtures
/// are read at test-execution time. The audit-record contract carries
/// the same `(scheme, predicate_id)` JSON shape; this type is the
/// fixture-side mirror used by the corpus regression harness.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ExpectedRuleId {
    pub scheme: String,
    pub predicate_id: String,
}

/// Expected diagnostic from a `.expected.json` sidecar file.
///
/// The `rule` field carries the structured 2-tuple shape.
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedDiagnostic {
    pub rule: ExpectedRuleId,
    pub span: ExpectedSpan,
    #[serde(default)]
    pub severity: Option<String>,
}

/// Expected byte span.
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedSpan {
    pub start: usize,
    pub end: usize,
}

/// Expected diagnostics loaded from a `.expected.json` file.
///
/// `ground_truth` is populated for document fixtures (under
/// `tests/corpus/documents/`) and absent for the per-rule micro-fixtures
/// under `valid/` and `invalid/`.
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedFixture {
    pub diagnostics: Vec<ExpectedDiagnostic>,
    #[serde(default)]
    pub ground_truth: Option<DocumentGroundTruth>,
}

/// Structural ground truth for a document fixture under
/// `tests/corpus/documents/`. Mirrors the schema produced by
/// `tools/cia-crest-corpus/render_corpus.py::truth_record`.
#[derive(Debug, Clone, Deserialize)]
pub struct DocumentGroundTruth {
    pub identifier: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub source_pdf: Option<String>,
    #[serde(default)]
    pub cab: Option<CabGroundTruth>,
    pub pages: Vec<PageGroundTruth>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// CAB (Classification Authority Block) ground-truth fields. All
/// three sub-fields are optional because hand-curated specs may
/// leave any individual line blank.
#[derive(Debug, Clone, Deserialize)]
pub struct CabGroundTruth {
    #[serde(default)]
    pub classified_by: Option<String>,
    #[serde(default)]
    pub derived_from: Option<String>,
    #[serde(default)]
    pub declassify_on: Option<String>,
}

/// One page of a document fixture: page number, page banner, and the
/// ordered list of paragraph-level portions.
#[derive(Debug, Clone, Deserialize)]
pub struct PageGroundTruth {
    pub page_num: u32,
    pub banner: String,
    pub paragraphs: Vec<ParagraphGroundTruth>,
}

/// One paragraph in a document fixture. `mark` is `None` for
/// paragraphs the renderer left unmarked — e.g. the embedded-cable
/// header block in `CIA-RDP90B01370R000801120005-5` whose body
/// happens to contain banner-shaped text but is not itself a portion.
#[derive(Debug, Clone, Deserialize)]
pub struct ParagraphGroundTruth {
    #[serde(default)]
    pub mark: Option<String>,
    pub text: String,
    #[serde(default)]
    pub is_table: bool,
}

/// Load the `.expected.json` sidecar for a given fixture path.
///
/// Given `tests/corpus/invalid/banner_abbrev.txt`, loads
/// `tests/corpus/invalid/banner_abbrev.expected.json`.
pub fn load_expected(fixture_path: &Path) -> ExpectedFixture {
    let json_path = fixture_path.with_extension("expected.json");
    if !json_path.exists() {
        panic!(
            "missing expected file for fixture: {} (expected {})",
            fixture_path.display(),
            json_path.display()
        );
    }
    let content = std::fs::read_to_string(&json_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", json_path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", json_path.display()))
}

/// Load fixture text content as bytes.
pub fn load_fixture(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

/// Returns paths to every rendered marked-document fixture under
/// `tests/corpus/documents/marked/*.md`, sorted by file name.
///
/// Each path's sibling `tests/corpus/documents/<stem>.expected.json`
/// carries the structural ground truth (banner per page, mark per
/// paragraph, CAB) via [`ExpectedFixture::ground_truth`].
pub fn marked_document_fixtures() -> Vec<PathBuf> {
    let dir = corpus_root().join("documents").join("marked");
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("failed to read documents/marked directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
        .collect();
    paths.sort();
    paths
}

/// Load the per-document ground-truth fixture (`<stem>.expected.json`)
/// for a marked document path under `tests/corpus/documents/marked/`.
///
/// Panics if the sidecar is missing, malformed, or lacks the
/// `ground_truth` field — the documents corpus contract requires
/// every marked fixture to carry structural ground truth.
pub fn load_document_ground_truth(marked_path: &Path) -> (ExpectedFixture, DocumentGroundTruth) {
    let stem = marked_path
        .file_stem()
        .unwrap_or_else(|| panic!("marked path has no file stem: {}", marked_path.display()));
    let expected_path = corpus_root()
        .join("documents")
        .join(format!("{}.expected.json", stem.to_string_lossy()));
    if !expected_path.exists() {
        panic!(
            "missing expected fixture for {}: {}",
            marked_path.display(),
            expected_path.display()
        );
    }
    let content = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", expected_path.display()));
    let fixture: ExpectedFixture = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", expected_path.display()));
    let ground_truth = fixture
        .ground_truth
        .clone()
        .unwrap_or_else(|| panic!("{} missing ground_truth field", expected_path.display()));
    (fixture, ground_truth)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn expected_rule_id_round_trips_through_json() {
        // The corpus-fixture JSON shape carries the structured
        // `(scheme, predicate_id)` form. Verify that `serde_json`
        // deserializes the canonical shape without re-introducing the
        // flat-string form by accident.
        let json = r#"{
            "scheme": "capco",
            "predicate_id": "portion.dissem.noforn-conflicts-rel-to"
        }"#;
        let parsed: ExpectedRuleId =
            serde_json::from_str(json).expect("structured rule id must deserialize");
        assert_eq!(parsed.scheme, "capco");
        assert_eq!(
            parsed.predicate_id,
            "portion.dissem.noforn-conflicts-rel-to"
        );
    }

    #[test]
    fn expected_diagnostic_carries_structured_rule_id() {
        // The flat-string `"rule": "E007"` shape is structurally
        // rejected — JSON consumers that aren't on the 2-tuple shape
        // get a parse error at the boundary, not a silent string
        // mismatch later. Every sidecar must use the structured shape.
        let json = r#"{
            "rule": {
                "scheme": "capco",
                "predicate_id": "banner.classification.usa-trigraph"
            },
            "span": {"start": 8, "end": 13}
        }"#;
        let parsed: ExpectedDiagnostic =
            serde_json::from_str(json).expect("structured shape must deserialize");
        assert_eq!(parsed.rule.scheme, "capco");
        assert_eq!(
            parsed.rule.predicate_id,
            "banner.classification.usa-trigraph"
        );
        assert_eq!(parsed.span.start, 8);
        assert_eq!(parsed.span.end, 13);
        assert!(parsed.severity.is_none());
    }

    #[test]
    fn expected_diagnostic_rejects_legacy_flat_string_rule() {
        // The flat `"rule": "E007"` shape MUST NOT deserialize
        // through the structured field. A stray flat-string sidecar
        // makes the corpus runner fail fast at fixture-load time with
        // a parse error rather than continuing with a malformed fixture.
        let json = r#"{
            "rule": "E007",
            "span": {"start": 8, "end": 13}
        }"#;
        assert!(
            serde_json::from_str::<ExpectedDiagnostic>(json).is_err(),
            "legacy flat-string rule id must not deserialize into the \
             structured ExpectedDiagnostic shape",
        );
    }
}
