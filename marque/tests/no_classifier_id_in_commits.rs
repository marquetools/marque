//! Phase 5 — SC-006: No classifier identity in committed test files (T055).
//!
//! Scans every file under `tests/corpus/`, `crates/*/tests/`, and
//! `crates/*/examples/` for classifier-id-shaped strings. Fails if any
//! real-looking classifier ID is found.
//!
//! Known test sentinels (e.g., "TEST-CLASSIFIER-42", "TEST-AUDIT-99") are
//! allowlisted — they are obviously synthetic and cannot be mistaken for
//! real PII.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().expect("workspace root").to_path_buf()
}

/// Allowlisted test sentinel values that are clearly synthetic.
const ALLOWED_SENTINELS: &[&str] = &[
    "TEST-CLASSIFIER-42",
    "TEST-AUDIT-99",
    "LOCAL-42",
    "ENV-99",
    "LEAKED-42",
    "from-root",
    "from-sub",
    "12345",
];

/// Scan a file for classifier-id-shaped strings.
///
/// Looks for patterns like `classifier_id` followed by a quoted value
/// that isn't a known test sentinel.
fn scan_file_for_classifier_ids(path: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };

    let mut violations = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        // Skip comment lines and known test infrastructure
        if line.trim_start().starts_with("//")
            || line.trim_start().starts_with('#')
            || line.contains("ALLOWED_SENTINELS")
            || line.contains("allowlist")
        {
            continue;
        }

        // Look for classifier_id assignments with suspicious values.
        // Pattern: classifier_id followed by = or : and a quoted value.
        if line.contains("classifier_id") {
            // Check if the value is a known sentinel
            let is_allowed = ALLOWED_SENTINELS
                .iter()
                .any(|sentinel| line.contains(sentinel));
            // Also allow None/null/empty/boolean references
            let is_meta = line.contains("None")
                || line.contains("null")
                || line.contains("is_none")
                || line.contains("is_some")
                || line.contains("classifier_id_present")
                || line.contains("as_deref")
                || line.contains(".classifier_id")
                || line.contains("MARQUE_CLASSIFIER_ID");

            if !is_allowed && !is_meta {
                // Check if there's a quoted string that looks like a real ID
                // (5+ digits or realistic alphanumeric pattern)
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start + 1..].find('"') {
                        let value = &line[start + 1..start + 1 + end];
                        let is_realistic = value.len() >= 5
                            && value.chars().all(|c| c.is_alphanumeric() || c == '-')
                            && !ALLOWED_SENTINELS.contains(&value);
                        if is_realistic {
                            violations.push(format!(
                                "{}:{}: suspicious classifier_id value {:?}",
                                path.display(),
                                line_num + 1,
                                value
                            ));
                        }
                    }
                }
            }
        }
    }
    violations
}

fn collect_files(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.exists() {
        return files;
    }
    for entry in walkdir(dir) {
        if let Some(ext) = entry.extension().and_then(|e| e.to_str()) {
            if extensions.contains(&ext) {
                files.push(entry);
            }
        }
    }
    files
}

/// Simple recursive directory walker (no external dependency needed).
fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir(&path));
            } else {
                result.push(path);
            }
        }
    }
    result
}

#[test]
fn sc006_no_classifier_id_in_committed_test_files() {
    let root = workspace_root();
    let scan_dirs = [root.join("tests").join("corpus"), root.join("crates")];

    let mut all_violations = Vec::new();
    for dir in &scan_dirs {
        let files = collect_files(dir, &["rs", "txt", "json", "toml"]);
        for file in &files {
            // Skip this test file itself
            if file.ends_with("no_classifier_id_in_commits.rs") {
                continue;
            }
            all_violations.extend(scan_file_for_classifier_ids(file));
        }
    }

    assert!(
        all_violations.is_empty(),
        "SC-006: Found {} suspicious classifier ID(s) in committed files:\n{}",
        all_violations.len(),
        all_violations.join("\n")
    );
}
