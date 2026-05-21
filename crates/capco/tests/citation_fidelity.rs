// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! F.1 corpus-fidelity gate (PR 10.A.2, T100/T101).
//!
//! # Contract
//!
//! Per FR-019 every authoritative-source citation declared by
//! `marque-capco`'s rule + catalog surface MUST be exercised by at
//! least one corpus fixture under `tests/corpus/`. The gate is
//! bi-directional — declared ⊆ harvested ∪ whitelist, and the
//! whitelist itself MUST stay disjoint from harvested (no
//! "became-covered-but-still-whitelisted" zombies) and a subset of
//! declared (no orphan whitelist entries).
//!
//! ## Declared set
//!
//! Three sources contribute:
//!
//! 1. `scheme.constraints()` — every `Constraint::label()` value.
//! 2. `scheme.page_rewrites()` — every `PageRewrite::citation` value.
//! 3. `scheme.closure_rules()` — every `ClosureRule::metadata().citation`
//!    value (the `label` field rendered through the metadata view).
//! 4. `ruleset.rules()` — every `Rule::cited_authorities()` slice
//!    union'd across registered rules.
//!
//! ## Harvested set
//!
//! `Engine::lint(fixture_bytes)` over every `.txt` file under
//! `tests/corpus/` (recursive). The harvested set is
//! `union(Diagnostic.citation)` across every fixture's lint output.
//!
//! ## Assertion shape
//!
//! Three hard-fail assertions:
//!
//! 1. `declared ⊆ harvested ∪ EXPECTED_UNCOVERED` — every declared
//!    citation is either exercised by a fixture or has a documented
//!    justification in the coverage report. Catches dead declarations.
//! 2. `EXPECTED_UNCOVERED ⊆ declared` — every whitelist entry must
//!    actually appear in the declared set. Catches orphans introduced
//!    when a rule is retired without removing its whitelist row.
//! 3. `EXPECTED_UNCOVERED ∩ harvested == ∅` — whitelist entries that
//!    later get covered MUST be removed. Catches stale whitelist
//!    entries (zombie coverage hides).
//!
//! Constitution VIII Authoritative Source Fidelity: every entry MUST
//! trace to a real CAPCO-2016 passage; whitelist justifications MUST
//! anchor into `docs/refactor-006/citation-coverage-report.md` so a
//! reviewer can audit the carve-out.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::RuleSet;
use marque_scheme::{
    AuthoritativeSource, Citation, MarkingScheme as _, SectionLetter, capco, capco_section,
};

// ===========================================================================
// EXPECTED_UNCOVERED whitelist
// ===========================================================================

/// Citations the declared catalog carries that no corpus fixture
/// currently exercises, with a justification anchor into
/// `docs/refactor-006/citation-coverage-report.md`.
///
/// Each entry is `(citation, anchor)`:
///
/// - `citation` — the unreachable `Citation` value (must appear in
///   the declared set; assertion 2 below catches orphans).
/// - `anchor` — the `<a id="..."></a>` fragment in the coverage
///   report where the justification lives.
///
/// Adding an entry is a deliberate carve-out — it MUST come with a
/// matching paragraph in the coverage report (and a test reviewer
/// MUST verify the carve-out is actually unreachable, not just
/// "fixture missing"). Removing an entry is automatic when a fixture
/// covers it (assertion 3 fires).
///
/// **Current entries** (11): the structurally-unreachable carve-outs
/// documented in `docs/refactor-006/citation-coverage-report.md`.
/// Two synthetic sentinels (`[config]` / `[engine-internal]`), one
/// cross-reference pin (`§D.1 p27`), one data-dependent rule trigger
/// gap (`§F p35` — no dissem entries in MIGRATIONS), one parser-
/// interaction follow-up (`§H.6 p120` — FRD/TFNI commingling), and
/// six PageRewrite citations whose enforcement is projection-time
/// only with no `Diagnostic` emission.
const EXPECTED_UNCOVERED: &[(Citation, &str)] = &[
    // Synthetic non-CAPCO sentinels.
    (marque_rules::CORRECTIONS_MAP_CITATION, "config-sentinel"),
    (engine_internal_sentinel(), "engine-internal-sentinel"),
    // Cross-reference pin (E005 secondary cite §D.1 p27).
    (capco(SectionLetter::D, 1, 27), "d1-p27-cross-ref"),
    // Rule trigger conditions not exercisable from current data.
    (
        capco_section(SectionLetter::F, 35),
        "f-p35-deprecated-dissem",
    ),
    (capco(SectionLetter::H, 6, 120), "h6-p120-frd-tfni"),
    // PageRewrite citations — projection-time mutation, no Diagnostic.
    (capco(SectionLetter::H, 8, 134), "h8-p134-fouo-eviction"),
    (
        capco(SectionLetter::H, 8, 140),
        "h8-p140-oc-usgov-supersession",
    ),
    (capco(SectionLetter::H, 9, 170), "h9-p170-limdis-eviction"),
    (capco(SectionLetter::H, 9, 176), "h9-p176-sbu-eviction"),
    (
        capco(SectionLetter::H, 9, 178),
        "h9-p178-sbu-nf-supersession",
    ),
    (
        capco(SectionLetter::H, 9, 185),
        "h9-p185-les-nf-supersession",
    ),
];

/// Construct the `[engine-internal]` sentinel Citation. The
/// `AuthoritativeSource::EngineInternal` variant has no const-fn
/// helper in `marque-scheme` today (CAPCO citations use `capco(...)`
/// / `capco_section(...)`; non-CAPCO sentinels go through
/// `Citation::new`); this helper mirrors the shape used by
/// `output.rs::stub_citation` and `scheduler.rs` so the F.1 whitelist
/// keeps the same canonical sentinel byte layout.
const fn engine_internal_sentinel() -> Citation {
    use marque_scheme::SectionRef;
    Citation::new(
        AuthoritativeSource::EngineInternal,
        SectionRef::new(SectionLetter::A),
        match core::num::NonZeroU16::new(1) {
            Some(n) => n,
            None => unreachable!(),
        },
    )
}

// ===========================================================================
// Test
// ===========================================================================

#[test]
fn corpus_covers_every_declared_authority() {
    // --- 1. Declared set ----------------------------------------------------
    let scheme = CapcoScheme::new();
    let ruleset = CapcoRuleSet::new();

    let mut declared: HashSet<Citation> = HashSet::new();
    for c in scheme.constraints() {
        declared.insert(c.label());
    }
    for r in scheme.page_rewrites() {
        declared.insert(r.citation);
    }
    for c in scheme.closure_rules() {
        // `ClosureRule.label` is the typed Citation; `closure_inventory`
        // would expose this via `ClosureRuleMetadata.citation: Option<Citation>`
        // but reading the rule directly avoids the `Option` wrap.
        declared.insert(c.label);
    }
    for rule in ruleset.rules() {
        for cite in rule.cited_authorities() {
            declared.insert(*cite);
        }
    }

    // --- 2. Harvested set ---------------------------------------------------
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    let engine = Engine::new(Config::default(), rule_sets, CapcoScheme::new())
        .expect("default CAPCO engine constructs without rewrite cycles");

    let corpus_dir = workspace_root().join("tests").join("corpus");
    let fixtures = collect_txt_fixtures(&corpus_dir);
    assert!(
        !fixtures.is_empty(),
        "no .txt fixtures found under {} — corpus tree is missing or empty",
        corpus_dir.display()
    );

    let mut harvested: HashSet<Citation> = HashSet::new();
    for fixture in &fixtures {
        let source = match fs::read(fixture) {
            Ok(bytes) => bytes,
            Err(e) => panic!("failed to read fixture {}: {e}", fixture.display()),
        };
        let result = engine.lint(&source);
        for d in &result.diagnostics {
            harvested.insert(d.citation);
        }
    }

    // --- 3. Whitelist consistency ------------------------------------------
    let whitelist_set: HashSet<Citation> = EXPECTED_UNCOVERED.iter().map(|(c, _)| *c).collect();

    // Assertion (a): declared ⊆ harvested ∪ whitelist.
    let undocumented_gaps: Vec<Citation> = declared
        .iter()
        .filter(|c| !harvested.contains(c) && !whitelist_set.contains(c))
        .copied()
        .collect();

    // Assertion (b): whitelist ⊆ declared.
    let orphaned_whitelist: Vec<Citation> = EXPECTED_UNCOVERED
        .iter()
        .filter(|(c, _)| !declared.contains(c))
        .map(|(c, _)| *c)
        .collect();

    // Assertion (c): whitelist ∩ harvested == ∅.
    let now_covered_whitelist: Vec<Citation> = EXPECTED_UNCOVERED
        .iter()
        .filter(|(c, _)| harvested.contains(c))
        .map(|(c, _)| *c)
        .collect();

    // Hard-fail with a single combined diagnostic so the operator can
    // see every failure mode at once instead of fixing them one-PR at
    // a time. Format each citation through its `Display` impl so the
    // failure message is reviewer-readable.
    if !undocumented_gaps.is_empty()
        || !orphaned_whitelist.is_empty()
        || !now_covered_whitelist.is_empty()
    {
        let mut msg = String::from("F.1 corpus-fidelity gate failed:\n\n");

        if !undocumented_gaps.is_empty() {
            msg.push_str(
                "  (a) Declared citations with no corpus coverage AND no \
                 EXPECTED_UNCOVERED whitelist entry:\n",
            );
            for c in &undocumented_gaps {
                msg.push_str(&format!("      - {c}\n"));
            }
            msg.push_str(
                "\n      → Either add a fixture under tests/corpus/{valid,invalid,foreign,\n\
                  \x20       lattice,mangled}/ that exercises the cited authority, OR add an\n\
                  \x20       EXPECTED_UNCOVERED entry with a justification anchor in\n\
                  \x20       docs/refactor-006/citation-coverage-report.md.\n\n",
            );
        }

        if !orphaned_whitelist.is_empty() {
            msg.push_str(
                "  (b) EXPECTED_UNCOVERED entries that do not appear in the declared set:\n",
            );
            for c in &orphaned_whitelist {
                msg.push_str(&format!("      - {c}\n"));
            }
            msg.push_str(
                "\n      → A rule or catalog row carrying this citation was retired without\n\
                  \x20       removing the whitelist entry. Remove the row from\n\
                  \x20       EXPECTED_UNCOVERED.\n\n",
            );
        }

        if !now_covered_whitelist.is_empty() {
            msg.push_str(
                "  (c) EXPECTED_UNCOVERED entries that ARE now covered by a corpus fixture:\n",
            );
            for c in &now_covered_whitelist {
                msg.push_str(&format!("      - {c}\n"));
            }
            msg.push_str(
                "\n      → A fixture started exercising the cited authority. Remove the\n\
                  \x20       whitelist entry; the gate will treat the coverage as authoritative.\n\n",
            );
        }

        msg.push_str(&format!(
            "Counts: declared={}, harvested={}, whitelisted={}",
            declared.len(),
            harvested.len(),
            whitelist_set.len(),
        ));

        panic!("{msg}");
    }
}

/// Manual discovery probe (`#[ignore]`-gated) — print every declared
/// citation alongside the fixture path(s) that exercise it (or
/// `<UNCOVERED>` if no fixture covers it). Used when adding new rules
/// or constructing the `EXPECTED_UNCOVERED` whitelist; the gate above
/// gives the count, this probe gives the per-citation breakdown.
///
/// Run with:
///
/// ```text
/// cargo test -p marque-capco --test citation_fidelity \
///     -- --ignored citation_coverage_probe --nocapture
/// ```
#[test]
#[ignore = "manual discovery; run via --ignored citation_coverage_probe --nocapture"]
fn citation_coverage_probe() {
    let scheme = CapcoScheme::new();
    let ruleset = CapcoRuleSet::new();

    let mut declared: HashSet<Citation> = HashSet::new();
    for c in scheme.constraints() {
        declared.insert(c.label());
    }
    for r in scheme.page_rewrites() {
        declared.insert(r.citation);
    }
    for c in scheme.closure_rules() {
        declared.insert(c.label);
    }
    for rule in ruleset.rules() {
        for cite in rule.cited_authorities() {
            declared.insert(*cite);
        }
    }

    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    let engine = Engine::new(Config::default(), rule_sets, CapcoScheme::new())
        .expect("default CAPCO engine");

    let corpus_dir = workspace_root().join("tests").join("corpus");
    let fixtures = collect_txt_fixtures(&corpus_dir);

    let mut by_citation: HashMap<Citation, Vec<String>> = HashMap::new();
    for fixture in &fixtures {
        let Ok(source) = fs::read(fixture) else {
            continue;
        };
        let result = engine.lint(&source);
        let display_path = fixture
            .strip_prefix(workspace_root())
            .unwrap_or(fixture)
            .to_string_lossy()
            .into_owned();
        for d in &result.diagnostics {
            by_citation
                .entry(d.citation)
                .or_default()
                .push(display_path.clone());
        }
    }

    // Sort declared citations by Display form so output is reviewer-readable.
    let mut declared_sorted: Vec<Citation> = declared.into_iter().collect();
    declared_sorted.sort_by_key(|c| format!("{c}"));

    println!("=== Citation coverage probe ===");
    for cite in &declared_sorted {
        match by_citation.get(cite) {
            Some(paths) => {
                let mut unique: Vec<&String> = paths.iter().collect();
                unique.sort();
                unique.dedup();
                println!("  {cite}: {} fixtures", unique.len());
                for p in unique.iter().take(3) {
                    println!("      - {p}");
                }
                if unique.len() > 3 {
                    println!("      ... ({} more)", unique.len() - 3);
                }
            }
            None => println!("  {cite}: <UNCOVERED>"),
        }
    }
}

#[test]
fn corpus_directory_exists() {
    // Cheap sanity: F.1 depends on the corpus tree being present.
    // Without this, a missing tree would surface as "every citation
    // missing" — clearer to fail explicitly.
    let corpus_dir = workspace_root().join("tests").join("corpus");
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
fn whitelist_entries_are_unique() {
    // Static check: no duplicate citation values in EXPECTED_UNCOVERED.
    // A duplicate would mask one anchor reference behind another and
    // make the "remove the entry when covered" workflow ambiguous.
    let mut seen: HashMap<Citation, &'static str> = HashMap::new();
    for (cite, anchor) in EXPECTED_UNCOVERED {
        if let Some(prior) = seen.insert(*cite, *anchor) {
            panic!(
                "EXPECTED_UNCOVERED carries duplicate citation {cite} \
                 (anchors `{prior}` and `{anchor}`)"
            );
        }
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Walk `tests/corpus/` recursively and return every `.txt` fixture path.
/// `.expected.json` sidecars and license files are intentionally excluded —
/// only the `.txt` set is run through the engine.
fn collect_txt_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_txt(dir, &mut out);
    out.sort();
    out
}

fn walk_txt(dir: &Path, out: &mut Vec<PathBuf>) {
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_txt(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("txt") {
            out.push(path);
        }
    }
}

/// Resolve the workspace root from `CARGO_MANIFEST_DIR`
/// (`crates/capco/` → workspace root).
fn workspace_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent() // crates/
        .and_then(Path::parent) // workspace
        .map(Path::to_path_buf)
        .expect("workspace root resolves from CARGO_MANIFEST_DIR")
}
