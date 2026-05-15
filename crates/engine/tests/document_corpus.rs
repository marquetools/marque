// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Structural count test for the documents corpus.
//!
//! Pins what Marque parses as a real marking against the per-document
//! ground truth in `tests/corpus/documents/<stem>.expected.json`:
//!
//! - `expected_portions = sum(pages[].paragraphs where mark != null)`
//! - `expected_banners  = pages.len() * 2`
//!   (renderer writes the banner top + bottom of every page; see
//!   `tools/cia-crest-corpus/render_corpus.py::render_marked`.)
//!
//! We count *parsed non-trivial markings*, not raw scanner candidates.
//! The scanner is a conservative first stage that emits a candidate
//! for every `(...)` sequence — including English parentheticals like
//! `(GAO)` or `(NPR)` — and the parser is the layer that decides
//! whether a candidate carries actual marking content. This mirrors
//! [`marque_engine::is_nontrivial_marking`] (engine `decoder.rs`),
//! the same predicate the engine uses when deciding whether to surface
//! a parse to the rule layer.
//!
//! Strict `==` everywhere. `EXPECTED_MISMATCHES` carries an allowlist
//! for files whose detected count legitimately deviates from ground
//! truth — see `CIA-RDP90B01370R000801120005-5` (embedded cable header
//! inside paragraph body produces a banner-shaped line the scanner
//! correctly detects).

use marque_core::{MarkingType, Parser, Scanner};
use marque_ism::{CapcoTokenSet, ParsedAttrs};
use marque_test_utils::{
    DocumentGroundTruth, load_document_ground_truth, marked_document_fixtures,
};
use std::collections::HashMap;

/// Predicate: does this parse carry actual marking content?
///
/// Mirrors `marque_engine::decoder::is_nontrivial_marking`. A
/// `(...)` candidate that parses with empty fields (e.g., an
/// English parenthetical like `(GAO)`) does NOT count as a
/// detected marking — the parser will succeed but produce a
/// zero-attribute `ParsedAttrs`, and the engine drops these from
/// the rule layer. This test applies the same filter.
fn is_nontrivial(attrs: &ParsedAttrs<'_>) -> bool {
    attrs.classification.is_some()
        || !attrs.sci_markings.is_empty()
        || !attrs.sci_controls.is_empty()
        || attrs.sar_markings.is_some()
        || !attrs.aea_markings.is_empty()
        || attrs.fgi_marker.is_some()
        || attrs.dissem_iter().next().is_some()
        || !attrs.non_ic_dissem.is_empty()
        || !attrs.rel_to.is_empty()
        || attrs.classified_by.is_some()
        || attrs.derived_from.is_some()
        || attrs.declassify_on.is_some()
        || attrs.declass_exemption.is_some()
}

/// Allowlist entry: a document whose detected portion/banner counts
/// legitimately deviate from ground truth, with the delta and a
/// human-readable reason. Detected count = ground-truth count + delta.
#[derive(Debug)]
struct ExpectedMismatch {
    /// Detected portion count minus ground-truth portion count.
    portion_delta: i32,
    /// Detected banner count minus ground-truth banner count.
    banner_delta: i32,
    /// Why this mismatch is acceptable. Surfaced in the failure
    /// message if the actual delta diverges from the pinned delta.
    reason: &'static str,
}

/// Files where strict `==` will not hold. Keyed by `file_stem`.
/// Empty by default — every entry MUST come with a documented
/// reason and a citation/issue link if it's tied to a known gap.
const EXPECTED_MISMATCHES: &[(&str, ExpectedMismatch)] = &[(
    // Ground truth records the embedded cable header body as a
    // single `mark: null` paragraph. The renderer writes that
    // paragraph's text verbatim, which contains a banner-shaped
    // line `TOP SECRET//RD//NOFORN/PROPIN 00 RUEAIIB` followed
    // by cable routing metadata. The scanner emits that line as
    // a Banner candidate and the parser accepts it — leading
    // `TOP SECRET//RD//NOFORN/PROPIN` resolves to a non-trivial
    // marking (`classification: TopSecret`, `aea: [Rd]`,
    // `dissem_us: [Nf, Pr]`); the trailing `00 RUEAIIB` is
    // discarded as non-marking trailing text. Ground truth says
    // 2 banners (top + bottom of 1 page); the engine sees 3.
    //
    // This is by-design real-world IC content: embedded cables
    // carry their own banner classifications. Marque is doing
    // the right thing here — it sees a banner-shaped line in
    // document text and surfaces it. A future engine layer
    // could distinguish "page banner" from "embedded-cable
    // banner," but that's a downstream concern, not a defect
    // in detection itself.
    "CIA-RDP90B01370R000801120005-5",
    ExpectedMismatch {
        portion_delta: 0,
        banner_delta: 1,
        reason: "embedded cable header inside paragraph body produces a \
                     banner-shaped line (TOP SECRET//RD//NOFORN/PROPIN ...) \
                     that the engine correctly detects as a banner",
    },
)];

fn lookup_mismatch(stem: &str) -> Option<&ExpectedMismatch> {
    EXPECTED_MISMATCHES
        .iter()
        .find_map(|(s, m)| (*s == stem).then_some(m))
}

fn expected_portions(gt: &DocumentGroundTruth) -> usize {
    gt.pages
        .iter()
        .flat_map(|p| p.paragraphs.iter())
        .filter(|para| para.mark.is_some())
        .count()
}

fn expected_banners(gt: &DocumentGroundTruth) -> usize {
    gt.pages.len() * 2
}

/// Strict portion-count and banner-count check against ground truth.
///
/// Runs the scanner over every `documents/marked/*.md` fixture and
/// asserts `Scanner::scan` emits exactly the count predicted by the
/// per-document ground-truth structure. Single batched failure
/// reporting — every mismatch is collected and reported together so a
/// single regression doesn't mask other drift.
#[test]
fn scanner_counts_match_ground_truth() {
    let fixtures = marked_document_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no marked document fixtures found in tests/corpus/documents/marked/"
    );

    let mut violations: Vec<String> = Vec::new();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);

    for marked_path in &fixtures {
        let stem = marked_path
            .file_stem()
            .expect("marked path has a stem")
            .to_string_lossy()
            .into_owned();
        let source = std::fs::read(marked_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", marked_path.display()));
        let (_fixture, gt) = load_document_ground_truth(marked_path);

        let candidates = Scanner::scan(&source);
        let mut detected_portions = 0usize;
        let mut detected_banners = 0usize;
        for cand in &candidates {
            if !matches!(cand.kind, MarkingType::Portion | MarkingType::Banner) {
                continue;
            }
            let Ok(parsed) = parser.parse(cand, &source) else {
                continue;
            };
            if !is_nontrivial(&parsed.attrs) {
                continue;
            }
            match cand.kind {
                MarkingType::Portion => detected_portions += 1,
                MarkingType::Banner => detected_banners += 1,
                _ => {}
            }
        }

        let exp_portions = expected_portions(&gt);
        let exp_banners = expected_banners(&gt);

        let pin = lookup_mismatch(&stem);
        let pinned_portion_delta = pin.map(|m| m.portion_delta).unwrap_or(0);
        let pinned_banner_delta = pin.map(|m| m.banner_delta).unwrap_or(0);

        let actual_portion_delta = detected_portions as i32 - exp_portions as i32;
        let actual_banner_delta = detected_banners as i32 - exp_banners as i32;

        if actual_portion_delta != pinned_portion_delta {
            violations.push(format!(
                "{stem}: portion count: expected {exp_portions} (+{pinned_portion_delta} pinned), \
                 got {detected_portions} (delta {actual_portion_delta})"
            ));
        }
        if actual_banner_delta != pinned_banner_delta {
            violations.push(format!(
                "{stem}: banner count: expected {exp_banners} (+{pinned_banner_delta} pinned), \
                 got {detected_banners} (delta {actual_banner_delta})"
            ));
        }
    }

    // Every pinned mismatch MUST correspond to a real fixture, so a
    // file deletion or rename surfaces the stale pin instead of
    // silently masking a regression somewhere else.
    let fixture_stems: HashMap<String, ()> = fixtures
        .iter()
        .map(|p| {
            (
                p.file_stem().expect("stem").to_string_lossy().into_owned(),
                (),
            )
        })
        .collect();
    for (stem, m) in EXPECTED_MISMATCHES {
        if !fixture_stems.contains_key(*stem) {
            violations.push(format!(
                "EXPECTED_MISMATCHES entry {stem:?} has no corresponding fixture; \
                 remove the pin (reason was: {})",
                m.reason
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "{} count mismatch(es) against ground truth:\n  {}",
        violations.len(),
        violations.join("\n  ")
    );
}

/// Cross-check the aggregate `documents/ground_truth.json` against the
/// per-document `<stem>.expected.json` files. Catches drift between
/// the two artifacts — e.g., a hand-edited per-doc fixture that didn't
/// get propagated into the aggregate, or vice versa.
#[test]
fn aggregate_ground_truth_matches_per_doc() {
    let docs_root = marque_test_utils::corpus_root().join("documents");
    let aggregate_path = docs_root.join("ground_truth.json");
    let aggregate_raw = std::fs::read_to_string(&aggregate_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", aggregate_path.display()));
    let aggregate: Vec<DocumentGroundTruth> = serde_json::from_str(&aggregate_raw)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", aggregate_path.display()));

    let fixtures = marked_document_fixtures();
    assert_eq!(
        aggregate.len(),
        fixtures.len(),
        "aggregate ground_truth.json has {} entries but documents/marked has {} fixtures",
        aggregate.len(),
        fixtures.len()
    );

    let mut by_id: HashMap<String, &DocumentGroundTruth> = aggregate
        .iter()
        .map(|gt| (gt.identifier.clone(), gt))
        .collect();
    let mut violations = Vec::new();

    for marked in &fixtures {
        let stem = marked
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .into_owned();
        let (_, per_doc) = load_document_ground_truth(marked);
        let Some(agg) = by_id.remove(&per_doc.identifier) else {
            violations.push(format!(
                "{stem}: per-doc identifier {:?} not present in aggregate ground_truth.json",
                per_doc.identifier
            ));
            continue;
        };

        if agg.pages.len() != per_doc.pages.len() {
            violations.push(format!(
                "{stem}: page count: aggregate has {}, per-doc has {}",
                agg.pages.len(),
                per_doc.pages.len()
            ));
            continue;
        }
        for (i, (a, p)) in agg.pages.iter().zip(per_doc.pages.iter()).enumerate() {
            if a.banner != p.banner {
                violations.push(format!(
                    "{stem} page {}: banner mismatch:\n    aggregate: {:?}\n    per-doc:   {:?}",
                    i + 1,
                    a.banner,
                    p.banner
                ));
            }
            if a.paragraphs.len() != p.paragraphs.len() {
                violations.push(format!(
                    "{stem} page {}: paragraph count: aggregate {}, per-doc {}",
                    i + 1,
                    a.paragraphs.len(),
                    p.paragraphs.len()
                ));
            }
        }
    }

    for leftover in by_id.keys() {
        violations.push(format!(
            "aggregate ground_truth.json carries identifier {leftover:?} with no \
             corresponding per-doc fixture"
        ));
    }

    assert!(
        violations.is_empty(),
        "{} aggregate-vs-per-doc drift violation(s):\n  {}",
        violations.len(),
        violations.join("\n  ")
    );
}
