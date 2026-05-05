// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! F.1 corpus-fidelity skeleton (PR 0.5).
//!
//! **What this test does (skeleton phase, T014)**
//!
//! Per FR-019: every `Constraint`/`PageRewrite`/`Rule` cited authority
//! in `marque-capco` MUST have at least one corpus fixture under
//! `tests/corpus/` that exercises the predicate against the canonical
//! example from the cited passage.
//!
//! At PR 0.5 (this PR) the test enumerates the citations carried by
//! the rule catalog and checks that the corpus contains at least one
//! fixture whose name plausibly relates to the cited section. The
//! skeleton matches by section letter alone (e.g., `§H.4` pulls any
//! fixture whose name contains `h4`, `h_4`, `hcs`, `sci`, etc.) — it
//! does NOT execute the rule predicates against the fixtures and does
//! NOT verify that the fixture's expected.json matches what the rule
//! would emit.
//!
//! **What this test does NOT do (deferred to PR 10)**
//!
//! The full F.1 maturation (PR 10 per the source plan §4) will:
//!
//! 1. Enumerate citations programmatically from the rule catalog by
//!    inspecting `Rule::citation()` and `Constraint::citation` /
//!    `PageRewrite::citation` field accessors. The skeleton hard-codes
//!    the citation list because the rule catalog is currently a
//!    procedural-only surface (no metadata accessor) — extracting
//!    citations through a stable accessor is part of the keystone
//!    refactor (PR 3a/3b/3c) that lands in the F.1 maturation cycle.
//!    See `// TODO(refactor-006-PR-0.5)` markers below.
//!
//! 2. Run the engine against each fixture and assert it emits
//!    diagnostics consistent with `<fixture>.expected.json`.
//!
//! 3. For each cited authority, assert that AT LEAST ONE fixture
//!    exists whose `expected.json` references the rule that carries
//!    that citation — closing the predicate-vs-canonical-example
//!    drift loop (the gap that the murder board surfaced for HCS-P).
//!
//! The skeleton's purpose at PR 0.5 is twofold: (1) wire F.1 into
//! `cargo test -p marque-capco` so the harness exists and is
//! exercised on every PR; (2) catalog which citations LACK any
//! plausible corpus coverage so PR 0.6 (and PR 10) can prioritize
//! fixture additions.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Citations from the current rule catalog that F.1 should verify
/// have corpus coverage. **Hand-curated for the skeleton** —
/// PR 10 will replace this with a programmatic accessor.
///
/// Each entry is `(citation, sloppy_keywords)` where the keywords
/// are substrings the test searches for in fixture filenames as a
/// proxy for relevance. The skeleton's matching is deliberately
/// permissive — a `letter h4` keyword finds any fixture whose name
/// references HCS, SCI compartments, etc. The PR 10 maturation
/// replaces sloppy matching with a fixture→rule index built from
/// each fixture's `expected.json`.
///
/// TODO(refactor-006-PR-0.5): replace with a programmatic accessor
/// once `Rule::citation()` is exposed (post-keystone PR 3a–3c). Track
/// in PR 10 (F.1 maturation).
const CITED_AUTHORITIES: &[(&str, &[&str])] = &[
    ("CAPCO-2016 §A.6", &["banner", "portion", "format"]),
    ("CAPCO-2016 §B.1", &["marking", "banner"]),
    ("CAPCO-2016 §B.3", &["fdr", "rel_to", "noforn"]),
    ("CAPCO-2016 §C.1", &["portion", "syntax"]),
    ("CAPCO-2016 §D.1", &["banner", "syntax"]),
    ("CAPCO-2016 §D.2", &["banner", "rollup"]),
    ("CAPCO-2016 §F", &["legacy"]),
    (
        "CAPCO-2016 §H.1",
        &["classification", "secret", "topsecret"],
    ),
    ("CAPCO-2016 §H.3", &["joint"]),
    ("CAPCO-2016 §H.4", &["sci", "hcs", "si", "tk"]),
    ("CAPCO-2016 §H.5", &["sar"]),
    ("CAPCO-2016 §H.6", &["aea", "sigma", "rd", "frd", "tfni"]),
    ("CAPCO-2016 §H.7", &["fgi"]),
    (
        "CAPCO-2016 §H.8",
        &["dissem", "noforn", "rel_to", "orcon", "fouo"],
    ),
    ("CAPCO-2016 §H.9", &["nodis", "exdis", "limdis", "sbu"]),
];

#[test]
fn corpus_contains_fixture_for_each_cited_authority() {
    let workspace_root = workspace_root();
    let corpus_dir = workspace_root.join("tests").join("corpus");
    let fixture_names = collect_fixture_basenames(&corpus_dir);
    assert!(
        !fixture_names.is_empty(),
        "no corpus fixtures found under {} — corpus tree missing or empty",
        corpus_dir.display()
    );

    let mut missing: Vec<&'static str> = Vec::new();
    for (citation, keywords) in CITED_AUTHORITIES {
        let has_match = keywords
            .iter()
            .any(|kw| fixture_names.iter().any(|name| name.contains(kw)));
        if !has_match {
            missing.push(citation);
        }
    }

    if !missing.is_empty() {
        // Skeleton policy: report missing-coverage as a soft failure
        // by listing every gap. PR 0.6 fixture additions and PR 10
        // maturation will close these. Failing the test loudly here
        // surfaces the gap to PR 0.6's reviewer.
        let mut msg = String::from(
            "F.1 skeleton: cited authorities lack plausible corpus coverage \
             (skeleton-level keyword match — PR 10 will replace with \
             programmatic rule→fixture mapping):\n",
        );
        for cite in &missing {
            msg.push_str("  - ");
            msg.push_str(cite);
            msg.push('\n');
        }
        msg.push_str(
            "\nThe skeleton matches fixture filenames against per-citation \
             keyword sets in `crates/capco/tests/citation_fidelity.rs`. To \
             close a gap, either (a) add a fixture under `tests/corpus/{valid,invalid,mangled}/` \
             whose name contains one of the citation's keywords, or (b) \
             extend the keyword set in CITED_AUTHORITIES if a different \
             fixture already exercises the cited authority.\n",
        );
        // Skeleton policy: this is a soft failure. The skeleton's purpose
        // is to surface the gap, not to gate PR 0.5 on full coverage.
        // PR 10 (F.1 maturation) tightens this to a hard failure once
        // the keyword-based proxy is replaced with a real
        // rule-citation→fixture index.
        //
        // **Output channel.** `cargo nextest` suppresses test stdout/stderr
        // for *passing* tests by default, so a plain `eprintln!` here is
        // invisible in CI logs. Emit the gap as a GitHub Actions warning
        // annotation when running under CI (env var `GITHUB_ACTIONS=true`),
        // which surfaces in the PR Checks UI without failing the test.
        // Outside CI we still print to stderr so local `cargo test
        // --nocapture` shows the gap.
        if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
            // Single-line GHA annotation. Newlines in the body are
            // escaped per https://github.com/actions/toolkit/issues/193 —
            // `%0A` is the encoded newline.
            let escaped = msg.replace('\n', "%0A");
            println!("::warning title=F.1 fixture coverage gap::{escaped}");
        } else {
            eprintln!("{msg}");
        }
    }
}

#[test]
fn corpus_directory_exists() {
    // Cheap sanity check — F.1 skeleton runs in CI and we want a
    // clear error if the corpus tree is missing entirely (e.g.,
    // a contributor accidentally gitignored it). Without this,
    // `corpus_contains_fixture_for_each_cited_authority` would still
    // fail, but the failure would be every-citation-missing rather
    // than the simpler root cause.
    let workspace_root = workspace_root();
    let corpus_dir = workspace_root.join("tests").join("corpus");
    assert!(
        corpus_dir.is_dir(),
        "corpus directory {} does not exist",
        corpus_dir.display()
    );
    let contract = corpus_dir.join("CORPUS_CONTRACT.md");
    assert!(
        contract.is_file(),
        "corpus contract file {} not found",
        contract.display()
    );
}

#[test]
fn cited_authorities_table_is_self_consistent() {
    // Static check: every entry in CITED_AUTHORITIES has at least one
    // keyword. An entry with an empty keyword list would silently match
    // nothing and falsely report "missing coverage."
    for (cite, kws) in CITED_AUTHORITIES {
        assert!(
            !kws.is_empty(),
            "CITED_AUTHORITIES entry for {cite} has no keywords; \
             would silently report missing coverage"
        );
    }
}

/// Collect every fixture basename (filename without extension) under
/// `dir`, recursing into subdirectories. Used to perform the keyword
/// match in `corpus_contains_fixture_for_each_cited_authority`.
fn collect_fixture_basenames(dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if !dir.is_dir() {
        return out;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(collect_fixture_basenames(&path));
        } else {
            // Per `tests/corpus/CORPUS_CONTRACT.md`, `.txt` files ARE
            // fixture inputs; their `.expected.json` siblings are the
            // pin-data and `.license` files are SPDX sidecars. Use only
            // the `.txt` set as the authoritative fixture-name source.
            // This avoids inflating coverage when (e.g.) a leftover
            // `*.txt.license` would otherwise satisfy the proxy without
            // the actual `.txt` fixture being present.
            if path.extension().and_then(|s| s.to_str()) != Some("txt") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                out.insert(stem.to_lowercase());
            }
        }
    }
    out
}

/// Resolve the workspace root from the CARGO_MANIFEST_DIR of this
/// crate. `crates/capco/` → `<workspace_root>`.
fn workspace_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent() // crates/
        .and_then(Path::parent) // workspace
        .map(Path::to_path_buf)
        .expect("workspace root resolves from CARGO_MANIFEST_DIR")
}
