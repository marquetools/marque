// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

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

/// Detects structurally-formed SCI tokens (spec 003-sci-compartments) like
/// `SI-G ABCD`, `HCS-P INTEL OPS`, `SI-G ABCD DEFG-MMM AACD`. These are
/// structurally parsed rather than CVE-matched, so the vocabulary-bounded
/// token check does not apply. A token qualifies when its first
/// hyphen-separated segment is a known bare SCI control system and the
/// remaining text contains only uppercase alphanumeric identifiers,
/// spaces, and hyphens.
fn is_structural_sci_token(token: &str) -> bool {
    let Some(first_segment_end) = token.find(['-', ' ']) else {
        return false;
    };
    let head = &token[..first_segment_end];
    if !marque_ism::is_bare_cve_value(head) {
        return false;
    }
    // Remaining characters must be uppercase alphanumerics, spaces, or
    // hyphens — the structural SCI grammar alphabet.
    token
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-' || c == ' ')
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
                // Skip SAR blocks — program identifiers are agency-assigned
                // codewords (not in any CVE enumeration) per CAPCO-2016 §H.5.
                if block.starts_with("SAR-") || block.starts_with("SPECIAL ACCESS REQUIRED-") {
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
                    // Skip structural SCI blocks (spec 003-sci-compartments):
                    // `SYS-COMP [SUB ...]` / `SYS-COMP-COMP2 ...` forms are
                    // structurally parsed rather than CVE-matched. Detect by
                    // a leading bare SCI system followed by `-`.
                    if is_structural_sci_token(token) {
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

// -----------------------------------------------------------------------
// Mangled-fixture token-only invariant (whitepaper §5.5 / gap register #19)
// -----------------------------------------------------------------------
//
// The mangled fixtures under `tests/fixtures/mangled/` are a generated
// artifact of `tools/corpus-analysis/analyze.py --mode mangled` and a
// load-bearing input to the SC-004 decoder accuracy gate. Each JSON file
// has the shape:
//
//     { "observed": "...", "expected": "...",
//       "mangling_class": "Typo", "source_confidence": 0.82 }
//
// `observed` is the mangled marking the decoder must resolve; `expected`
// is the canonical CAPCO marking it should resolve to. By construction,
// both fields should contain only marking-shaped content — uppercase
// letters, digits, spaces, marking delimiters, and the small set of
// mangling glyphs the generator can produce. They MUST NOT contain
// surrounding prose or classifier-id-shaped digit runs; the
// source-narrowing invariant in `tests/fixtures/mangled/README.md`
// keeps the generator pinned to `tests/corpus/valid/`, but a
// regression in the generator (or a manual edit that didn't get
// regenerated) would silently leak prose into the harness fixtures
// and from there into Phase-D telemetry.
//
// This test is the post-condition. It walks every mangled JSON file
// and asserts:
//
//   1. `observed` / `expected` decode as JSON strings.
//   2. Neither contains a prose sentinel from the audit-stream
//      content-ignorance corpus (mirrors `crates/engine/tests/audit.rs`).
//   3. Neither contains a classifier-id-shaped digit run (5+ ASCII
//      digits inside quotes / on a line by itself).
//   4. Neither exceeds the per-marking length cap (256 bytes — well
//      above the longest realistic CAPCO banner). Prose creeping in
//      would blow this cap long before any other check.
//
// This is intentionally narrow — it does NOT enforce vocabulary
// membership the way `sc002a_fixture_tokens_within_known_vocabulary`
// does for `tests/corpus/valid/`, because mangled fixtures
// deliberately contain typos / superseded tokens / wrong-case forms
// that are out-of-vocabulary by design. The check is "no prose got
// in", not "every token is canonical".

const MANGLED_PROSE_SENTINELS: &[&str] = &[
    // Drawn from `tests/corpus/prose/article.txt` — multi-word English
    // fragments that cannot appear in any valid CAPCO/ISM marking and
    // therefore cannot legitimately appear in `observed` / `expected`.
    // Kept in sync with `crates/engine/tests/audit.rs::PROSE_SENTINELS`
    // so a sentinel that rules out a leak in the audit stream also
    // rules it out here.
    "republic has over a democracy",
    "numerous advantages promised",
    "Liberty is to faction what air",
    "insuperable obstacle to a uniformity",
    "early prevalence of these sentiments",
    "distinct interests in society",
    "various and interfering interests",
    "adjust these clashing interests",
    "protection of these faculties",
    "principal task of modern legislation",
    "judge in his own cause",
    "enlightened statesmen",
];

const MANGLED_FIELD_BYTE_CAP: usize = 256;

#[test]
fn mangled_fixtures_observed_expected_token_only() {
    let mangled_dir = workspace_root()
        .join("tests")
        .join("fixtures")
        .join("mangled");
    if !mangled_dir.exists() {
        // Mangled corpus is regenerable from `tools/corpus-analysis/`.
        // A bare checkout that hasn't run the generator is a permitted
        // state — skip rather than fail, matching the existing
        // sc002a tests that bail when their corpus is absent.
        return;
    }

    let files = walkdir(&mangled_dir);
    let mut violations: Vec<String> = Vec::new();

    for file in &files {
        if file.extension().is_none_or(|e| e != "json") {
            continue;
        }
        // Record an unreadable fixture as a violation rather than a
        // silent skip — a partial checkout, a corrupted file, or a
        // permission glitch otherwise lets this invariant test pass
        // without actually validating the fixture set.
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(err) => {
                violations.push(format!("{}: unreadable fixture: {err}", file.display()));
                continue;
            }
        };

        // Parse the fixture as JSON and pull the `observed` / `expected`
        // strings out via `serde_json`. The generator
        // (`tools/corpus-analysis/analyze.py`) writes the fixture with
        // `json.dumps`, so any input that doesn't round-trip through
        // `serde_json::from_str` is itself a corruption violation.
        for field in ["observed", "expected"] {
            let Some(value) = extract_string_field(&content, field) else {
                violations.push(format!(
                    "{}: missing or non-string field {field:?}",
                    file.display()
                ));
                continue;
            };

            if value.len() > MANGLED_FIELD_BYTE_CAP {
                violations.push(format!(
                    "{}: field {field:?} exceeds {MANGLED_FIELD_BYTE_CAP}-byte cap \
                     ({} bytes) — likely prose leakage",
                    file.display(),
                    value.len()
                ));
                continue;
            }

            for sentinel in MANGLED_PROSE_SENTINELS {
                if value.contains(sentinel) {
                    violations.push(format!(
                        "{}: field {field:?} contains prose sentinel {sentinel:?}",
                        file.display()
                    ));
                }
            }

            // 5+ consecutive ASCII digits = classifier-id-shaped run.
            // CAPCO markings don't carry runs that long: the longest
            // legitimate digit run is the 8-digit `declassify_on` date
            // (YYYYMMDD), and that always sits inside a `Declassify On:`
            // CAB line — never bare in the marking text. Mangled
            // fixtures don't carry CAB content at all.
            let mut run = 0usize;
            for b in value.bytes() {
                if b.is_ascii_digit() {
                    run += 1;
                    if run >= 5 {
                        violations.push(format!(
                            "{}: field {field:?} contains 5+-digit run \
                             — possible classifier-id leakage",
                            file.display()
                        ));
                        break;
                    }
                } else {
                    run = 0;
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Mangled-fixture token-only invariant: {} violation(s):\n{}",
        violations.len(),
        violations.join("\n")
    );
}

/// Pull the string value of a top-level JSON field.
///
/// Parses the fixture via `serde_json` and returns the named top-level
/// field only when it is present and is a JSON string. Returns `None`
/// for non-JSON input, missing fields, and non-string values; the
/// caller treats `None` as a fixture-shape violation. `marque` already
/// depends on `serde_json` (workspace dep), so this carries no
/// additional dependency cost over the previous hand-rolled scan.
fn extract_string_field(json: &str, field: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let object = value.as_object()?;
    object.get(field)?.as_str().map(ToOwned::to_owned)
}
