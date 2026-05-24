// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Corpus-fidelity gate.
//!
//! # Contract
//!
//! Every authoritative-source citation declared by
//! `marque-capco`'s rule + catalog surface MUST be exercised by at
//! least one corpus fixture under `tests/corpus/`. The gate is
//! bi-directional — declared ⊆ harvested ∪ whitelist, and the
//! whitelist itself MUST stay disjoint from harvested (no
//! "became-covered-but-still-whitelisted" zombies) and a subset of
//! declared (no orphan whitelist entries).
//!
//! ## Declared set
//!
//! Four sources contribute:
//!
//! 1. `scheme.constraints()` — every `Constraint::label()` value.
//! 2. `scheme.page_rewrites()` — every `PageRewrite::citation` value.
//! 3. `scheme.closure_rules()` — every `ClosureRule::metadata().citation`
//!    value (the `label` field rendered through the metadata view).
//! 4. `ruleset.rules()` — every `Rule::cited_authorities()` slice
//!    union'd across registered rules.
//!
//! ## Adding a fixture vs. a whitelist entry
//!
//! When the gate fails with assertion (a) — declared citation with
//! no corpus coverage — the default response is to add a fixture
//! under `tests/corpus/{valid,invalid,foreign,lattice,mangled}/`
//! that exercises the cited authority. The corpus tree's fixture-
//! add procedure (frontmatter requirements, sidecar `.expected.json`
//! shape, §-citation re-verification) lives in
//! `tests/corpus/CORPUS_CONTRACT.md`. Only when the citation is
//! provably unreachable from any fixture (one of the structural
//! reasons in the whitelist contract below) should the response be
//! an `EXPECTED_UNCOVERED` carve-out instead.
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
    capco_table,
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
/// **Current entries**: the structurally-unreachable carve-outs
/// documented in `docs/refactor-006/citation-coverage-report.md`.
/// Two synthetic sentinels (`[config]` / `[engine-internal]`), one
/// data-dependent rule trigger gap (`§F p35` — no dissem entries in
/// MIGRATIONS), and six PageRewrite citations whose enforcement is
/// projection-time only with no `Diagnostic` emission.
///
/// Issue #661 closed (PR fix/661-e070-frd-tfni-bridge): E070 (§H.6
/// p120 — FRD precedence over TFNI) was previously suppressed by
/// the engine bridge because its `ConstraintViolation` carried
/// `span: None, severity: None`. The fix wired both fields through
/// (anchoring on the dominated TFNI token, severity `Fix` mirroring
/// `e024_rd_precedence`) and added the corpus fixture
/// `tests/corpus/invalid/e070_frd_tfni_precedence.txt`, so the
/// citation is now harvested.
///
/// Issue #677 closed: §D.1 p27 (banner-line syntax — Marking Title
/// OR Authorized Abbreviation) became the primary citation for the
/// new `PortionFormInBannerRule`, harvested by corpus fixtures
/// `tests/corpus/invalid/677_*.txt`. Removed from the whitelist —
/// the gate now treats the coverage as authoritative.
const EXPECTED_UNCOVERED: &[(Citation, &str)] = &[
    // Synthetic non-CAPCO sentinels.
    (marque_rules::CORRECTIONS_MAP_CITATION, "config-sentinel"),
    (engine_internal_sentinel(), "engine-internal-sentinel"),
    // Rule trigger conditions not exercisable from current data.
    (
        capco_section(SectionLetter::F, 35),
        "f-p35-deprecated-dissem",
    ),
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
    // §H.8 p150 is declared by the Suggest-severity REL TO rules
    // (S003 / S004 / S005 / S010) but no current corpus fixture
    // exercises any of them. The latent gap was previously masked by
    // E002 (Error-severity, exercised by the `missing_usa_trigraph`
    // fixtures) emitting §H.8 p150 too; E002 now cites the more precise
    // §H.8 p151 (the verbatim USA-first rule lives in the Additional
    // Marking Instructions block on p151, not the section anchor on
    // p150). The Suggest-
    // rule gap is structural — Suggest rules don't auto-apply and
    // the corpus does not yet carry trigraph-typo / uniform-REL-TO
    // / uncertain-reduction fixtures. Tracked for follow-up
    // fixture-coverage; allowlisted by this gate's contract.
    (capco(SectionLetter::H, 8, 150), "h8-p150-suggest-rules-gap"),
    // #704 (post-review-cycle Fix 2): S008's authority slice gained
    // §B.3 Table 2 p21 as the primary trigger authority (the
    // default-if-absent obligation that drives the implicit-RELIDO
    // injection S008 surfaces). Per-Diagnostic emission is
    // single-Citation by API shape and stays at §H.8 p154 (RELIDO
    // marking template — what RELIDO means once present), where
    // every existing S008 fixture's expected.json already harvests
    // it. The §B.3 Table 2 p21 declaration is honest authority-set
    // metadata that the harvest gate cannot see in fixture output;
    // the citation IS exercised by S008-firing fixtures end-to-end,
    // just not in the per-Diagnostic Citation field.
    (
        capco_table(SectionLetter::B, 3, 2, 21),
        "b3-table2-p21-s008-trigger-authority-not-emitted",
    ),
    // #738: §A.6 p16 is declared by `FgiOwnershipTrigraphSuggestRule`
    // (the verified FGI trigraph-separator grammar — "Multiple FGI
    // trigraph country codes or tetragraph codes must be separated by a
    // single space") as honest authority-set metadata, but that rule
    // emits the more-specific §H.7 p122 (the FGI ownership-list grammar)
    // on every Diagnostic, so §A.6 p16 is never harvestable from FGI
    // fixtures. The latent declared-but-not-emitted gap was previously
    // masked by W034 (`SciCustomControlInfoRule`) emitting §A.6 p16 on
    // its diagnostics. #738 re-anchored W034 to its substantive backing
    // (§H.4 p61 — the registered-but-unpublished SCI registry text),
    // which removed that incidental coverage and exposed the FGI gap.
    // Same class as the §H.8 p150 and §B.3 Table 2 p21 entries above:
    // legitimate authority-set metadata the per-Diagnostic harvest
    // cannot see. Tracked for follow-up fixture-coverage;
    // allowlisted by this gate's contract.
    (
        capco(SectionLetter::A, 6, 16),
        "a6-p16-fgi-separator-grammar-not-emitted",
    ),
];

/// Construct the `[engine-internal]` sentinel Citation. The
/// `AuthoritativeSource::EngineInternal` variant has no const-fn
/// helper in `marque-scheme` today (CAPCO citations use `capco(...)`
/// / `capco_section(...)`; non-CAPCO sentinels go through
/// `Citation::new`); this helper mirrors the shape used by
/// `output.rs::stub_citation` and `scheduler.rs` so the allowlist
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

    // Engine-side synthetic citations the harvest may legitimately
    // observe even though no `Rule::cited_authorities()` or catalog
    // row declares them. R001 (decoder canonicalization) cites
    // §A.6 p15 and R002 (re-parse failure) cites the
    // `[engine-internal]` sentinel; both are engine-side machinery,
    // not catalog data. Excluding them from the
    // `harvested ⊆ declared` check is correct — a stricter check
    // would force engine-side citations into the catalog surface,
    // which is the inversion of dependency the typed-citation
    // migration was designed to avoid.
    //
    // If new engine-side synthetic citations get added (e.g., new
    // diagnostic rules emitted by `marque-engine` directly), add
    // them here.
    let engine_emitted: HashSet<Citation> = [
        capco(SectionLetter::A, 6, 15), // R001 — `engine.rs::DECODER_CITATION_TYPED`
        engine_internal_sentinel(),     // R002 — `engine.rs::R002_CITATION_TYPED`
    ]
    .into_iter()
    .collect();

    // Assertion (a): declared ⊆ harvested ∪ whitelist.
    // Catches: declared citation has no fixture exercising it (the rule
    // exists but no corpus input triggers it). Sorted for deterministic
    // failure output ordering — Copilot review on PR #662 caught the
    // nondeterministic `HashSet` iteration order in the prior
    // implementation.
    let mut undocumented_gaps: Vec<Citation> = declared
        .iter()
        .filter(|c| !harvested.contains(c) && !whitelist_set.contains(c))
        .copied()
        .collect();
    undocumented_gaps.sort();

    // Assertion (b): whitelist ⊆ declared.
    // Catches: whitelist entry refers to a citation no rule / catalog row
    // declares (rule was retired but whitelist not cleaned up).
    let mut orphaned_whitelist: Vec<Citation> = EXPECTED_UNCOVERED
        .iter()
        .filter(|(c, _)| !declared.contains(c))
        .map(|(c, _)| *c)
        .collect();
    orphaned_whitelist.sort();

    // Assertion (c): whitelist ∩ harvested == ∅.
    // Catches: a fixture started exercising a whitelisted citation
    // (good news — coverage improved) but the whitelist wasn't
    // cleaned up. Whitelist must drain as coverage grows.
    let mut now_covered_whitelist: Vec<Citation> = EXPECTED_UNCOVERED
        .iter()
        .filter(|(c, _)| harvested.contains(c))
        .map(|(c, _)| *c)
        .collect();
    now_covered_whitelist.sort();

    // Assertion (d): harvested ⊆ declared ∪ engine_emitted.
    // Catches: a rule emits a `Diagnostic.citation` that doesn't
    // appear in its `cited_authorities()` declaration (drift in the
    // OTHER direction — adding a new `capco(...)` literal to a
    // rule's `check()` body without updating `cited_authorities()`).
    // Without this, the fidelity gate is one-directional and lets
    // undeclared emissions slip through silently.
    let mut undeclared_emissions: Vec<Citation> = harvested
        .iter()
        .filter(|c| !declared.contains(c) && !engine_emitted.contains(c))
        .copied()
        .collect();
    undeclared_emissions.sort();

    // Hard-fail with a single combined diagnostic so the operator can
    // see every failure mode at once instead of fixing them one-PR at
    // a time. Format each citation through its `Display` impl so the
    // failure message is reviewer-readable.
    if !undocumented_gaps.is_empty()
        || !orphaned_whitelist.is_empty()
        || !now_covered_whitelist.is_empty()
        || !undeclared_emissions.is_empty()
    {
        let mut msg = String::from("corpus-fidelity gate failed:\n\n");

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

        if !undeclared_emissions.is_empty() {
            msg.push_str(
                "  (d) Harvested citations that no rule declares via cited_authorities() AND\n\
                  \x20     no catalog row carries:\n",
            );
            for c in &undeclared_emissions {
                msg.push_str(&format!("      - {c}\n"));
            }
            msg.push_str(
                "\n      → A rule emitted a Diagnostic with a Citation it never declared.\n\
                  \x20       Either add the citation to the rule's `cited_authorities()` const\n\
                  \x20       (preferred — the rule does emit it), OR if the rule should not have\n\
                  \x20       emitted that citation, fix the `check()` body. If the citation comes\n\
                  \x20       from engine-side machinery (R001/R002), add it to the local\n\
                  \x20       `engine_emitted` set above and document why.\n\n",
            );
        }

        msg.push_str(&format!(
            "Counts: declared={}, harvested={}, whitelisted={}, engine_emitted={}",
            declared.len(),
            harvested.len(),
            whitelist_set.len(),
            engine_emitted.len(),
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

    // Sort declared citations by native `Ord` (`Citation` and its three
    // component types implement `Ord` + `PartialOrd`). Lexicographic on
    // `(AuthoritativeSource, SectionLetter, subsection, table, page)`
    // — close enough to the Display form for reviewer-readability,
    // and avoids the per-element `format!()` allocation of the prior
    // `sort_by_key(|c| format!("{c}"))` workaround.
    let mut declared_sorted: Vec<Citation> = declared.into_iter().collect();
    declared_sorted.sort();

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
    // Cheap sanity: the gate depends on the corpus tree being present.
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
fn whitelist_anchors_exist_in_report() {
    // Load-bearing 5-year property: every `(citation, anchor)` row in
    // `EXPECTED_UNCOVERED` MUST resolve to a matching
    // `<a id="..."></a>` fragment in
    // `docs/refactor-006/citation-coverage-report.md`. Without this
    // assertion, the whitelist silts up over time as report anchors
    // get renamed, deleted, or never written — the test would still
    // pass because the citation-side mechanics (in-set / not-harvested)
    // hold, but a reviewer following the anchor would land on a
    // missing target.
    //
    // Separated from `corpus_covers_every_declared_authority` so the
    // failure mode is precisely diagnosed: a missing anchor is a
    // documentation-side defect distinct from a coverage gap.
    let report_path = workspace_root()
        .join("docs")
        .join("refactor-006")
        .join("citation-coverage-report.md");
    let report = fs::read_to_string(&report_path).unwrap_or_else(|e| {
        panic!(
            "citation coverage report must be readable at {}: {e}",
            report_path.display()
        )
    });
    let mut missing_anchors: Vec<&'static str> = Vec::new();
    for (_cite, anchor) in EXPECTED_UNCOVERED {
        let needle = format!("<a id=\"{anchor}\"></a>");
        if !report.contains(&needle) {
            missing_anchors.push(*anchor);
        }
    }
    assert!(
        missing_anchors.is_empty(),
        "EXPECTED_UNCOVERED anchors with no matching <a id=\"...\"></a> \
         in {}: {:?}\n\n\
         Either add the missing anchor paragraph to the coverage report, \
         or remove the EXPECTED_UNCOVERED row and add a fixture that \
         covers the citation (see tests/corpus/CORPUS_CONTRACT.md).",
        report_path.display(),
        missing_anchors,
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
