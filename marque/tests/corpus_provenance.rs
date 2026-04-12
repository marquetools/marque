//! Phase 5 — SC-002a corpus provenance scan (T055a).
//!
//! Validates corpus integrity:
//! (a) Every file under `tests/corpus/` matches a registered path pattern
//! (b) `CORPUS_PROVENANCE.md` exists and contains a reviewer line
//! (c) No fixture contains classifier-id-shaped strings

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().expect("workspace root").to_path_buf()
}

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

/// Registered path patterns for files under tests/corpus/.
fn is_registered_pattern(relative: &str) -> bool {
    let patterns = [
        // invalid fixtures
        ("invalid/", ".txt"),
        ("invalid/", ".expected.json"),
        ("invalid/", ".expected_fix.json"),
        // valid fixtures
        ("valid/", ".txt"),
        ("valid/", ".expected.json"),
        // prose fixtures (future)
        ("prose/", ".txt"),
    ];

    // Top-level files
    let top_level = ["CORPUS_CONTRACT.md", "CORPUS_PROVENANCE.md", "README.md"];

    // .gitkeep files are allowed in any subdirectory
    if relative.ends_with(".gitkeep") {
        return true;
    }

    if top_level.contains(&relative) {
        return true;
    }

    for (prefix, suffix) in &patterns {
        if relative.starts_with(prefix) && relative.ends_with(suffix) {
            return true;
        }
    }

    false
}

#[test]
fn sc002a_every_corpus_file_matches_registered_pattern() {
    let corpus_dir = workspace_root().join("tests").join("corpus");
    assert!(corpus_dir.exists(), "tests/corpus/ directory must exist");

    let files = walkdir(&corpus_dir);
    let mut violations = Vec::new();

    for file in &files {
        let relative = file
            .strip_prefix(&corpus_dir)
            .expect("strip prefix")
            .to_string_lossy()
            .replace('\\', "/"); // normalize Windows paths

        if !is_registered_pattern(&relative) {
            violations.push(relative);
        }
    }

    assert!(
        violations.is_empty(),
        "SC-002a: {} file(s) under tests/corpus/ don't match registered patterns:\n{}",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn sc002a_corpus_provenance_exists_and_has_reviewer() {
    let provenance = workspace_root()
        .join("tests")
        .join("corpus")
        .join("CORPUS_PROVENANCE.md");
    assert!(provenance.exists(), "CORPUS_PROVENANCE.md must exist");

    let content = std::fs::read_to_string(&provenance).expect("read CORPUS_PROVENANCE.md");
    let has_reviewer = content.lines().any(|line| {
        let lower = line.to_lowercase();
        lower.contains("reviewer") || lower.contains("reviewed by") || lower.contains("attested")
    });
    assert!(
        has_reviewer,
        "CORPUS_PROVENANCE.md must contain a reviewer attestation line"
    );
}

#[test]
fn sc002a_no_classifier_id_in_corpus_fixtures() {
    let corpus_dir = workspace_root().join("tests").join("corpus");
    let files = walkdir(&corpus_dir);
    let mut violations = Vec::new();

    for file in &files {
        if !file.extension().is_some_and(|e| e == "txt" || e == "json") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(file) else {
            continue;
        };
        // Look for realistic-looking classifier IDs in fixture content.
        // A realistic classifier ID is a numeric string of 5+ digits.
        for (line_num, line) in content.lines().enumerate() {
            if line.contains("classifier_id") && line.contains('"') {
                // Check if value looks realistic (not "null" or test sentinel)
                if !line.contains("null") && !line.contains("TEST") && !line.contains("None") {
                    violations.push(format!(
                        "{}:{}: {}",
                        file.display(),
                        line_num + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "SC-002a(c): {} classifier-id-shaped string(s) in corpus fixtures:\n{}",
        violations.len(),
        violations.join("\n")
    );
}
