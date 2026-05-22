// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 4 (006 T118) — lattice corpus regression runner.
//!
//! Drives the 5 worked-example fixtures under `tests/corpus/lattice/`
//! through `CapcoScheme::project(Scope::Page, ...)` and asserts byte-
//! identity against `.expected.json` sidecars. Dispatches on fixture
//! structural shape (CAB-commingling vs portions+banner) per the
//! `tests/corpus/lattice/README.md` contract.
//!
//! Also hosts the T119 manual probe (`probe_documents_lint_clean`)
//! as an `#[ignore]`-gated diagnostic for the 40 CIA CREST documents
//! under `tests/corpus/documents/marked/`. Probe stays in-tree
//! post-merge as a regression replay surface.
//!
//! # Sidecar shape (parallel to `marque_test_utils::ExpectedFixture`)
//!
//! Per PM doc D-3 (2026-05-19): rather than extending the canonical
//! `ExpectedFixture` (which would force every existing sidecar under
//! `valid/`/`invalid/`/`foreign/` to carry or default an
//! `expected_banner` field), the runner defines a parallel
//! `LatticeExpectedFixture` type local to this module. The
//! `marque_test_utils::ExpectedDiagnostic` type is re-used directly
//! for the `diagnostics` array so the per-rule + span shape stays
//! consistent with the rest of the corpus contract.
//!
//! # CAB-shape dispatch (D-5)
//!
//! `classify_shape` walks the fixture line-by-line, skipping blank
//! and `#`-prefixed comment lines, and case-insensitively prefix-
//! matches `Classified By:` / `Derived From:` / `Declassify On:` on
//! the first content line. Confirmed against the 5 in-tree fixtures
//! 2026-05-19: `aea-commingling.txt` fires (line 1 = `Classified By:
//! First Reviewer`); the other 4 start with `(` (portion form) and
//! fall to `PortionsBanner`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_config::Config;
use marque_core::Parser;
use marque_engine::Engine;
use marque_ism::CanonicalAttrs;
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_rules::RuleSet;
use marque_scheme::{MarkingScheme as _, Scope, Span};
use marque_test_utils::{ExpectedDiagnostic, fixtures_in, load_fixture, marked_document_fixtures};
use serde::Deserialize;

// ===========================================================================
// Sidecar type
// ===========================================================================

/// Lattice-corpus sidecar — parallel to `marque_test_utils::ExpectedFixture`
/// per PM doc D-3. `_note` carries the `§X.Y pNN` citation re-verified at
/// authorship against `crates/capco/docs/CAPCO-2016.md` (Constitution VIII).
#[derive(Debug, Clone, Deserialize)]
struct LatticeExpectedFixture {
    #[serde(rename = "_note", default)]
    #[allow(dead_code)] // surfaced for reviewer auditing, not test-time use.
    note: Option<String>,
    /// Expected banner rendered by
    /// `CapcoScheme::render_banner(&scheme.project(Scope::Page, &markings))`.
    /// For the `CabCommingling` fixture shape this field is intentionally
    /// `None` — the fixture has no portions to project.
    #[serde(default)]
    expected_banner: Option<String>,
    /// Expected diagnostics emitted by `Engine::lint(whole_fixture_bytes)`.
    /// Spans are not enforced today (the diagnostics list is positional
    /// rule-id pin only) to match the loose ground-truth contract used
    /// by `mixed_us_foreign_rollup.expected.json`.
    #[serde(default)]
    diagnostics: Vec<ExpectedDiagnostic>,
}

// ===========================================================================
// Fixture shape dispatch (D-5)
// ===========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureShape {
    /// Fixture starts with a Classification Authority Block — no
    /// portion-line / banner-line shape to project. Tested via
    /// `Engine::lint(bytes)` only.
    CabCommingling,
    /// Fixture is one-or-more `(...)` portion lines followed by a
    /// banner line. Tested via `scheme.project(Scope::Page, &portions)`
    /// + `scheme.render_banner(&projected)` + `Engine::lint(bytes)`.
    PortionsBanner,
}

/// Classify a fixture's structural shape per PM doc D-5.
///
/// Walks the fixture line-by-line: skip blank + `#`-comment lines,
/// case-insensitively prefix-match on the first content line.
fn classify_shape(source: &[u8]) -> FixtureShape {
    let text = std::str::from_utf8(source).expect("fixture text is valid UTF-8");
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let upper = line.to_ascii_uppercase();
        if upper.starts_with("CLASSIFIED BY:")
            || upper.starts_with("DERIVED FROM:")
            || upper.starts_with("DECLASSIFY ON:")
        {
            return FixtureShape::CabCommingling;
        }
        // First content line is not a CAB header — assume portion shape.
        return FixtureShape::PortionsBanner;
    }
    // All-blank fixture (degenerate). Treat as CAB so the runner skips
    // banner projection rather than parsing an empty portion list.
    FixtureShape::CabCommingling
}

// ===========================================================================
// Helpers
// ===========================================================================

fn engine() -> Engine {
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    Engine::new(Config::default(), rule_sets, CapcoScheme::new())
        .expect("default CAPCO scheme constructs without rewrite cycles")
}

/// Parse a single `(...)` portion line into a `CanonicalAttrs`.
///
/// PR 3c.2.B (PM-B-3 second clause): the helper takes `&CapcoScheme`
/// so callers (`discover`, `lattice_corpus_fixtures_match_expected`)
/// that already construct a scheme for `scheme.project(Scope::Page,
/// ...)` can reuse it.
fn parse_portion_line(scheme: &CapcoScheme, line: &str) -> CanonicalAttrs {
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let bytes = line.as_bytes();
    let candidate = MarkingCandidate {
        span: Span::new(0, bytes.len()),
        kind: MarkingType::Portion,
    };
    let parsed = parser
        .parse(&candidate, bytes)
        .unwrap_or_else(|e| panic!("portion `{line}` must parse: {e:?}"));
    scheme.canonicalize(parsed.attrs)
}

/// Split a fixture's bytes into per-line portion candidates, filtering
/// out blank lines and any line that doesn't begin with `(`. The
/// trailing banner line (last non-blank line of `PortionsBanner`
/// fixtures) is NOT a portion and is excluded.
fn extract_portion_lines(source: &[u8]) -> Vec<String> {
    let text = std::str::from_utf8(source).expect("fixture text is valid UTF-8");
    text.lines()
        .map(|s| s.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && l.starts_with('('))
        .map(|s| s.to_string())
        .collect()
}

// ===========================================================================
// Discovery helper (used by maintainers; not a #[test])
// ===========================================================================
//
// Run with `cargo test -p marque-capco --test lattice_corpus_runner -- \
//     --ignored discover --nocapture` to capture the current
// `scheme.project()` + `Engine::lint()` output per fixture without
// asserting against the sidecars. Used at PR authorship to verify
// expected_banner / diagnostics arrays before pinning them.

#[test]
#[ignore = "manual discovery; run via cargo test -- --ignored discover --nocapture"]
fn discover() {
    let scheme = CapcoScheme::new();
    let engine = engine();
    let fixtures = fixtures_in("lattice");
    assert!(!fixtures.is_empty(), "no lattice fixtures found");

    for path in &fixtures {
        let source = load_fixture(path);
        let shape = classify_shape(&source);
        let fname = path.file_name().unwrap().to_string_lossy();

        println!("=== {fname} (shape: {shape:?}) ===");

        if shape == FixtureShape::PortionsBanner {
            let portion_lines = extract_portion_lines(&source);
            let portions: Vec<CanonicalAttrs> = portion_lines
                .iter()
                .map(|s| parse_portion_line(&scheme, s))
                .collect();
            let markings: Vec<CapcoMarking> =
                portions.iter().cloned().map(CapcoMarking::new).collect();
            let projected = scheme.project(Scope::Page, &markings);
            let rendered = scheme.render_banner(&projected);
            println!("expected_banner: {rendered:?}");
        }

        let result = engine.lint(&source);
        println!("diagnostics ({} total):", result.diagnostics.len());
        for d in &result.diagnostics {
            println!(
                "  {} at {}..{}: {}",
                d.rule.predicate_id(),
                d.span.start,
                d.span.end,
                d.message.template().as_str()
            );
        }
        println!();
    }
}

// ===========================================================================
// T118 — lattice corpus fixtures byte-identity gate
// ===========================================================================

/// Load the lattice sidecar `<stem>.expected.json` for a fixture path.
fn load_lattice_expected(fixture_path: &Path) -> LatticeExpectedFixture {
    let json_path = fixture_path.with_extension("expected.json");
    assert!(
        json_path.exists(),
        "missing lattice sidecar for fixture {} (expected {})",
        fixture_path.display(),
        json_path.display()
    );
    let content = std::fs::read_to_string(&json_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", json_path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", json_path.display()))
}

/// PR 4 (006 T118) load-bearing test: every fixture under
/// `tests/corpus/lattice/` MUST round-trip through
/// `CapcoScheme::project(Scope::Page, ...)` + `scheme.render_banner(...)`,
/// with the rendered banner asserted **byte-identical** to the sidecar's
/// `expected_banner` field. `Engine::lint(...)` diagnostics are matched
/// against the sidecar's `diagnostics` array by **rule-id occurrence
/// count only** (spans + messages intentionally not enforced) per the
/// loose-ground-truth contract established by `tests/corpus/foreign/`.
/// The two assertion granularities are deliberate: banners are precise
/// CAPCO §A.6 / §G.1 byte forms (any drift is a renderer bug); per-rule
/// diagnostic counts are robust to incidental span shifts as the rule
/// catalog evolves.
///
/// Dispatches on fixture shape per PM doc D-5: portion-line/banner
/// fixtures get both projection + lint coverage; CAB-only fixtures
/// get lint coverage only (no portions to project).
#[test]
fn lattice_corpus_fixtures_match_expected() {
    let scheme = CapcoScheme::new();
    let engine = engine();
    let fixtures = fixtures_in("lattice");
    assert!(
        !fixtures.is_empty(),
        "no lattice fixtures found — expected 5 under tests/corpus/lattice/"
    );

    for path in &fixtures {
        let source = load_fixture(path);
        let expected = load_lattice_expected(path);
        let shape = classify_shape(&source);
        let fname = path.file_name().unwrap().to_string_lossy();

        // -- Banner projection (PortionsBanner shape only) ---------------
        if shape == FixtureShape::PortionsBanner {
            let portion_lines = extract_portion_lines(&source);
            assert!(
                !portion_lines.is_empty(),
                "fixture {fname}: PortionsBanner shape has no `(...)` lines"
            );
            let portions: Vec<CanonicalAttrs> = portion_lines
                .iter()
                .map(|s| parse_portion_line(&scheme, s))
                .collect();
            let markings: Vec<CapcoMarking> =
                portions.iter().cloned().map(CapcoMarking::new).collect();
            let projected = scheme.project(Scope::Page, &markings);
            let rendered = scheme.render_banner(&projected);

            let expected_banner = expected.expected_banner.as_deref().unwrap_or_else(|| {
                panic!(
                    "fixture {fname}: PortionsBanner shape requires expected_banner \
                     in sidecar"
                )
            });
            assert_eq!(
                rendered, expected_banner,
                "fixture {fname}: banner roll-up mismatch"
            );
        } else {
            // CabCommingling fixtures must NOT carry an expected_banner
            // (the runner has nothing to project against).
            assert!(
                expected.expected_banner.is_none(),
                "fixture {fname}: CabCommingling shape MUST NOT carry expected_banner"
            );
        }

        // -- Engine.lint diagnostics --------------------------------------
        let result = engine.lint(&source);

        // Match expected diagnostics by rule-id occurrence count.
        // Spans intentionally are NOT enforced (loose ground-truth
        // contract per `mixed_us_foreign_rollup.expected.json` precedent —
        // we pin which rules fire, not which span the engine assigns).
        let mut actual_counts: BTreeMap<String, usize> = BTreeMap::new();
        for d in &result.diagnostics {
            *actual_counts
                .entry(d.rule.predicate_id().to_string())
                .or_insert(0) += 1;
        }
        // T044: `ExpectedRuleId` is the 2-tuple struct; key the counts
        // map on `predicate_id` to match the `actual_counts` shape
        // built from `RuleId::predicate_id()` above.
        let mut expected_counts: BTreeMap<String, usize> = BTreeMap::new();
        for e in &expected.diagnostics {
            *expected_counts.entry(e.rule.predicate_id.clone()).or_insert(0) += 1;
        }

        if actual_counts != expected_counts {
            let actual_str = format!("{actual_counts:?}");
            let expected_str = format!("{expected_counts:?}");
            let detail = result
                .diagnostics
                .iter()
                .map(|d| {
                    format!(
                        "  {} at {}..{}: {}",
                        d.rule.predicate_id(),
                        d.span.start,
                        d.span.end,
                        d.message.template().as_str()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            panic!(
                "fixture {fname}: diagnostic mismatch\n  expected: {expected_str}\n  actual: {actual_str}\nfull diagnostics:\n{detail}"
            );
        }
    }
}

// ===========================================================================
// T119 — documents-corpus precision probe (manual diagnostic)
// ===========================================================================

/// PR 4 (006 T119) manual probe: lint every rendered CIA CREST document
/// under `tests/corpus/documents/marked/*.md` and print per-document
/// diagnostic counts to stdout, sorted by descending count.
///
/// **Probe-first ordering (PM doc D-6).** The 40 marked-document
/// fixtures have ground-truth sidecars claiming `"diagnostics": []`,
/// but they have never been run through `Engine::lint` — only
/// `Scanner::scan` via `crates/engine/tests/document_corpus.rs::
/// scanner_counts_match_ground_truth`. Three failure modes are
/// possible (per PM doc D-6):
///
/// 1. Unexpected diagnostics emit → real engine bugs OR stale
///    ground-truth claims (Constitution VIII — neither tolerated
///    silently).
/// 2. Per-document precision logic differs from `prose/`-style
///    zero-diagnostic gate.
/// 3. Performance: 40 multi-page documents may materially exceed
///    `prose/`'s ~8 short fixtures.
///
/// The probe stays `#[ignore]`-gated as a diagnostic surface (not an
/// assertion gate) so it survives merge as a regression replay
/// surface. If the probe is clean (40/40 zero-diagnostic), an
/// `Engine`-side assertion gate (`precision_documents_zero_diagnostics`)
/// lands in `crates/engine/tests/corpus_accuracy.rs` mirroring
/// `precision_prose_zero_diagnostics`. If drift is found, the gate
/// is deferred to a follow-up issue with per-document triage.
///
/// # Content-ignorance (Constitution V Principle V / G13)
///
/// Probe output contains ONLY: file stem, diagnostic count, rule ID,
/// and span byte offsets. The `Diagnostic.message` field is
/// intentionally **excluded** because some rule messages interpolate
/// the offending source token text (e.g.,
/// `crates/capco/src/rules.rs` ~L879 `format!("...{:?}", token_text)`),
/// which would leak document body bytes into stdout — violating the
/// G13 invariant that no document content appears in engine output
/// streams. For full triage of a specific document, run the engine
/// directly on the file (e.g.,
/// `cargo run -p marque -- check tests/corpus/documents/marked/<file>`)
/// rather than reaching for the message field here.
///
/// Run with:
///
/// ```sh
/// cargo test -p marque-capco --test lattice_corpus_runner -- \
///     --ignored probe_documents_lint_clean --nocapture
/// ```
#[test]
#[ignore = "manual diagnostic; run via cargo test -- --ignored probe_documents_lint_clean --nocapture"]
fn probe_documents_lint_clean() {
    let engine = engine();
    let fixtures: Vec<PathBuf> = marked_document_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no marked-document fixtures found under tests/corpus/documents/marked/"
    );

    let mut counts: Vec<(String, usize, Vec<String>)> = Vec::new();
    for path in &fixtures {
        let source = load_fixture(path);
        let result = engine.lint(&source);
        let fname = path.file_name().unwrap().to_string_lossy().into_owned();
        // Content-ignorance: rule ID + span only; the `Diagnostic.message`
        // field can interpolate input tokens, so it is intentionally
        // excluded from probe output per Constitution V Principle V / G13.
        let detail: Vec<String> = result
            .diagnostics
            .iter()
            .map(|d| {
                format!(
                    "    {} at {}..{}",
                    d.rule.predicate_id(),
                    d.span.start,
                    d.span.end,
                )
            })
            .collect();
        counts.push((fname, result.diagnostics.len(), detail));
    }

    // Sort descending by diagnostic count so the noisiest documents
    // surface at the top of the report.
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let total: usize = counts.iter().map(|(_, n, _)| n).sum();
    let clean: usize = counts.iter().filter(|(_, n, _)| *n == 0).count();
    let total_docs = counts.len();

    println!("=== T119 probe — documents corpus lint diagnostics ===");
    println!(
        "  {clean}/{total_docs} documents emit zero diagnostics ({total} total diagnostics emitted)"
    );
    println!();
    for (fname, count, detail) in &counts {
        if *count == 0 {
            continue;
        }
        println!("  {fname}: {count} diagnostic(s)");
        for line in detail {
            println!("{line}");
        }
    }
    if clean == total_docs {
        println!("  All documents clean. T119 assertion gate may land.");
    } else {
        println!(
            "  {} document(s) emit diagnostics. Triage per fixture before \
             landing T119 assertion gate.",
            total_docs - clean
        );
    }
}
