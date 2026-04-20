// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! Shared test utilities for the marque workspace.
//!
//! Provides uniform access to `tests/corpus/` fixtures from any crate's test suite.
//! Add this crate as a `[dev-dependencies]` path dependency.

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

/// Expected diagnostic from a `.expected.json` sidecar file.
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedDiagnostic {
    pub rule: String,
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
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedFixture {
    pub diagnostics: Vec<ExpectedDiagnostic>,
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
