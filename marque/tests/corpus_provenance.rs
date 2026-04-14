//! Phase 5 — SC-002a corpus provenance scan (T055a).
//!
//! Validates corpus integrity:
//! (a) Every file under `tests/corpus/` matches a registered path pattern
//! (b) `CORPUS_PROVENANCE.md` exists and contains a reviewer line
//! (c) No fixture contains classifier-id-shaped strings
//! (d) Fixture token strings are drawn from known CVE enumerations

use marque_ism::SciControl;
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
        // A realistic classifier ID is a quoted numeric string of 5+ digits.
        for (line_num, line) in content.lines().enumerate() {
            if let Some((_, remainder)) = line.split_once("classifier_id") {
                if let Some(start) = remainder.find('"') {
                    let quoted = &remainder[start + 1..];
                    if let Some(end) = quoted.find('"') {
                        let value = &quoted[..end];
                        if value.len() >= 5 && value.chars().all(|c| c.is_ascii_digit()) {
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
        }
    }

    assert!(
        violations.is_empty(),
        "SC-002a(c): {} classifier-id-shaped string(s) in corpus fixtures:\n{}",
        violations.len(),
        violations.join("\n")
    );
}

// -----------------------------------------------------------------------
// T055a(d): fixture tokens must come from known CVE enumerations
// -----------------------------------------------------------------------

/// Build a set of all known token strings from the ISM CVE enumerations.
fn known_cve_tokens() -> std::collections::HashSet<&'static str> {
    let mut tokens = std::collections::HashSet::new();

    // Classification levels (full and abbreviated forms)
    for s in &[
        "TOP SECRET",
        "TS",
        "SECRET",
        "S",
        "CONFIDENTIAL",
        "C",
        "UNCLASSIFIED",
        "U",
    ] {
        tokens.insert(*s);
    }

    // SCI controls — verify each parses through the generated code
    for s in &[
        "SI", "TK", "HCS", "BUR", "KLM", "RSV", "MVL", "SI-G", "SI-NK", "SI-EU", "HCS-O", "HCS-P",
        "HCS-X", "BUR-BLG", "BUR-DTP", "BUR-WRG", "KLM-R", "TK-BLFH", "TK-IDIT", "TK-KAND",
    ] {
        assert!(
            SciControl::parse(s).is_some(),
            "SCI control {s:?} must parse"
        );
        tokens.insert(*s);
    }

    // Dissem controls (canonical and abbreviated forms)
    for s in &[
        "NOFORN",
        "NF",
        "ORCON",
        "OC",
        "IMCON",
        "IMC",
        "PROPIN",
        "RELIDO",
        "DEA SENSITIVE",
        "DSEN",
        "FISA",
        "DISPLAY ONLY",
        "WAIVED",
    ] {
        tokens.insert(*s);
    }

    // Non-IC dissem controls (CAPCO Register §9)
    for s in &[
        "LIMDIS",
        "DS",
        "EXDIS",
        "XD",
        "NODIS",
        "ND",
        "SBU",
        "SBU NOFORN",
        "SBU-NF",
        "LES",
        "LES NOFORN",
        "LES-NF",
        "SSI",
    ] {
        tokens.insert(*s);
    }

    // REL TO, trigraphs, and structural elements
    for s in &[
        "REL TO", "USA", "GBR", "CAN", "AUS", "NZL", "FVEY", "//", "/",
    ] {
        tokens.insert(*s);
    }

    // Declassification exemption markers
    for s in &["X1", "X2", "X3", "X4", "X5", "X6", "X7", "X8"] {
        tokens.insert(*s);
    }

    tokens
}

#[test]
fn sc002a_fixture_tokens_within_known_vocabulary() {
    // Only validate valid/ fixtures — invalid/ fixtures intentionally
    // contain non-CVE tokens to exercise error-detection rules.
    let corpus_dir = workspace_root().join("tests").join("corpus").join("valid");
    if !corpus_dir.exists() {
        return; // no valid fixtures yet
    }
    let known = known_cve_tokens();
    let mut violations = Vec::new();

    for file in walkdir(&corpus_dir) {
        if file.extension().is_none_or(|e| e != "txt") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Strip portion parens if present
            let marking = trimmed
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(trimmed);

            // Split on "//" separators and check each token
            for block in marking.split("//") {
                let block = block.trim();
                if block.is_empty() {
                    continue;
                }
                // Skip "REL TO ..." blocks — trigraph list varies
                if block.starts_with("REL TO") || block.starts_with("REL ") {
                    continue;
                }
                // Skip date-like tokens (YYYYMMDD or Xn patterns)
                if block.len() == 8 && block.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                // Skip "Classified By:", "Derived From:", etc. (CAB lines)
                if block.contains(':') {
                    continue;
                }
                // Skip prose content (lines with spaces and lowercase chars)
                if block.contains(' ') && block.chars().any(|c| c.is_lowercase()) {
                    continue;
                }

                // Handle comma-separated lists (e.g., "SI, TK")
                // Also handle CAPCO §D.1 slash-separated entries within a block
                // (e.g., "SI/TK" → ["SI", "TK"]).
                let sub_tokens: Vec<&str> = if block.contains(',') {
                    block.split(',').map(|t| t.trim()).collect()
                } else if block.contains('/') {
                    block.split('/').map(|t| t.trim()).collect()
                } else {
                    vec![block]
                };

                for token in sub_tokens {
                    if token.is_empty() {
                        continue;
                    }
                    if !known.contains(token) {
                        violations.push(format!(
                            "{}:{}: unknown token {:?}",
                            file.display(),
                            line_num + 1,
                            token
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "SC-002a(d): {} token(s) in corpus fixtures not in CVE vocabulary:\n{}",
        violations.len(),
        violations.join("\n")
    );
}
