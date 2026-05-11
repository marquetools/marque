// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T088 — WASM-target parity harness (SC-008).
//!
//! Runs under `wasm-pack test crates/wasm --node`, exercising the
//! actual `wasm32-unknown-unknown` build of `marque-wasm`. For each
//! corpus fixture in the baked artifact, calls `lint_native` and
//! asserts byte-equal NDJSON against the expected output that the
//! native engine produced when the artifact was last regenerated.
//!
//! ## Why bake the corpus
//!
//! `wasm-pack test --node` runs the test binary inside Node's WASM
//! runtime where `std::fs` is unavailable on the `wasm32-unknown-unknown`
//! target. The corpus and the expected output are baked into the
//! binary via `include_str!` — pure-computational, no I/O.
//!
//! ## Sync gate
//!
//! `tests/parity_sync_check.rs` runs natively and asserts the baked
//! artifact stays in sync with what `Engine::lint` produces today.
//! That test fails first when the engine output legitimately changes,
//! pointing the developer at the regeneration command. This file
//! therefore has one job: prove the WASM compilation path produces
//! the same bytes as the native compilation path on the same input.
//!
//! ## Why prose is not in the artifact
//!
//! Prose corpus (`tests/corpus/prose/article.txt`, ~125KB) would
//! dominate the artifact size. The native parity harness
//! (`native_parity.rs::lint_parity_prose_fixtures`) covers prose
//! parity natively, which exercises the same lint codepath through
//! the WASM crate's `lint_native` shim. The remaining gap that
//! `parity.rs` closes is native-vs-WASM compilation divergence on
//! the same algorithm; that gap is fully exercised by the smaller
//! `invalid` + `valid` corpus.

#![cfg(target_arch = "wasm32")]

use serde::Deserialize;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_node_experimental);

const PARITY_CORPUS_JSON: &str = include_str!("parity_corpus.json");

#[derive(Debug, Deserialize)]
struct ParityEntry {
    name: String,
    /// Corpus subdir (`invalid` / `valid`). Surfaced in failure messages
    /// so a parity break points at the offending fixture's category.
    category: String,
    input: String,
    expected_lint: String,
}

#[derive(Debug, Deserialize)]
struct ParityCorpus {
    schema: String,
    entries: Vec<ParityEntry>,
}

const EXPECTED_SCHEMA: &str = "marque-wasm-parity-1";

fn parse_corpus() -> ParityCorpus {
    let corpus: ParityCorpus =
        serde_json::from_str(PARITY_CORPUS_JSON).expect("baked parity corpus JSON parses");
    assert_eq!(
        corpus.schema, EXPECTED_SCHEMA,
        "parity corpus schema mismatch: artifact says {:?}, this test expects {:?}",
        corpus.schema, EXPECTED_SCHEMA
    );
    assert!(
        !corpus.entries.is_empty(),
        "parity corpus is empty; T088 requires at least one fixture"
    );
    corpus
}

#[wasm_bindgen_test]
fn wasm_lint_parity_matches_baked_native_output() {
    let corpus = parse_corpus();

    let mut failures: Vec<String> = Vec::new();

    for entry in &corpus.entries {
        let actual = match marque_wasm::lint_native(&entry.input, None) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!(
                    "{}/{}: lint_native errored: {e}",
                    entry.category, entry.name
                ));
                continue;
            }
        };

        if actual != entry.expected_lint {
            failures.push(format!(
                "{}/{}: SC-008 byte-equal NDJSON parity violated\n  \
                 expected ({} bytes): {:?}\n  \
                 actual   ({} bytes): {:?}",
                entry.category,
                entry.name,
                entry.expected_lint.len(),
                entry.expected_lint,
                actual.len(),
                actual,
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} parity failure(s) out of {} fixtures:\n\n{}",
            failures.len(),
            corpus.entries.len(),
            failures.join("\n\n")
        );
    }
}

#[wasm_bindgen_test]
fn wasm_lint_clean_input_produces_empty_ndjson() {
    let result = marque_wasm::lint_native("SECRET//NOFORN\n", None)
        .expect("lint_native must succeed on clean input");
    assert_eq!(
        result, "",
        "clean banner should produce no diagnostics under wasm32"
    );
}

#[wasm_bindgen_test]
fn wasm_lint_empty_input_produces_empty_ndjson() {
    let result =
        marque_wasm::lint_native("", None).expect("lint_native must succeed on empty input");
    assert_eq!(
        result, "",
        "empty input should produce no diagnostics under wasm32"
    );
}

// ---------------------------------------------------------------------------
// fix_native on wasm32
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn wasm_fix_clean_input_is_unchanged() {
    let result = marque_wasm::fix_native("SECRET//NOFORN\n", 0.5, None)
        .expect("fix_native must succeed on clean input");
    let parsed: serde_json::Value =
        serde_json::from_str(&result).expect("fix_native must return valid JSON");
    assert_eq!(
        parsed["fixed_text"].as_str().unwrap(),
        "SECRET//NOFORN\n",
        "clean input must be unchanged after fix under wasm32"
    );
    assert_eq!(
        parsed["applied"].as_array().unwrap().len(),
        0,
        "clean input must produce no applied fixes under wasm32"
    );
}

#[wasm_bindgen_test]
fn wasm_fix_applies_correction() {
    // E002: REL TO missing USA trigraph. fix should inject `USA` and
    // canonicalize the list (USA-first alpha per §H.8 p150-151).
    // Pre-PR-3c.B Commit 6 fixture anchored on E001 (NF → NOFORN),
    // retired into the renderer at that commit.
    let result = marque_wasm::fix_native("SECRET//REL TO GBR\n", 0.0, None)
        .expect("fix_native must succeed");
    let parsed: serde_json::Value =
        serde_json::from_str(&result).expect("fix_native must return valid JSON");
    let fixed = parsed["fixed_text"].as_str().unwrap();
    assert!(
        fixed.contains("USA"),
        "fix under wasm32 should inject USA into REL TO list, got: {fixed}"
    );
}

#[wasm_bindgen_test]
fn wasm_fix_invalid_threshold_returns_error() {
    let result = marque_wasm::fix_native("SECRET//REL TO GBR\n", -1.0, None);
    assert!(
        result.is_err(),
        "fix_native must reject negative threshold under wasm32"
    );
}

// ---------------------------------------------------------------------------
// lint_batch_native on wasm32
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn wasm_lint_batch_empty_array() {
    let result = marque_wasm::lint_batch_native("[]", None)
        .expect("lint_batch_native must succeed on empty array");
    let parsed: serde_json::Value =
        serde_json::from_str(&result).expect("lint_batch_native must return valid JSON");
    assert_eq!(
        parsed.as_array().unwrap().len(),
        0,
        "empty batch must produce empty results under wasm32"
    );
}

#[wasm_bindgen_test]
fn wasm_lint_batch_two_entries() {
    // Pre-PR-3c.B Commit 6 the fixtures here anchored on E001
    // (SECRET//NF → diagnostic) and E001 absence (SECRET//NOFORN →
    // clean). E001 retired into `MarkingScheme::render_canonical` at
    // that commit. The replacement diagnostic-firing fixture is
    // SECRET//REL TO GBR (E002, REL TO missing USA trigraph,
    // §H.8 p150-151); the clean fixture switches to the canonical
    // SECRET//REL TO USA, GBR (USA-first alpha).
    let entries = r#"[
        {"id": "inv", "text": "SECRET//REL TO GBR\n"},
        {"id": "ok",  "text": "SECRET//REL TO USA, GBR\n"}
    ]"#;
    let result =
        marque_wasm::lint_batch_native(entries, None).expect("lint_batch_native must succeed");
    let parsed: serde_json::Value =
        serde_json::from_str(&result).expect("lint_batch_native must return valid JSON");
    let arr = parsed.as_array().unwrap();
    assert_eq!(
        arr.len(),
        2,
        "must return one result per entry under wasm32"
    );
    assert_eq!(arr[0]["id"], "inv");
    assert_eq!(arr[1]["id"], "ok");
    assert!(
        !arr[0]["diagnostics"].as_array().unwrap().is_empty(),
        "SECRET//REL TO GBR must produce diagnostics under wasm32"
    );
    assert!(
        arr[1]["diagnostics"].as_array().unwrap().is_empty(),
        "SECRET//REL TO USA, GBR must be clean under wasm32"
    );
}

#[wasm_bindgen_test]
fn wasm_lint_batch_parity_with_single_lint() {
    // Batch result for each entry must match what lint_native returns for the
    // same input individually — validates the batch path doesn't diverge on wasm32.
    let texts = [
        ("a", "SECRET//NF\n"),
        ("b", "TOP SECRET//SI//NF\n"),
        ("c", "SECRET//NOFORN\n"),
    ];

    let entries_json = {
        let items: Vec<serde_json::Value> = texts
            .iter()
            .map(|(id, text)| serde_json::json!({"id": id, "text": text}))
            .collect();
        serde_json::to_string(&items).unwrap()
    };

    let batch_json = marque_wasm::lint_batch_native(&entries_json, None)
        .expect("lint_batch_native must succeed");
    let batch: Vec<serde_json::Value> = serde_json::from_str(&batch_json).unwrap();

    for (i, (id, text)) in texts.iter().enumerate() {
        let single_ndjson = marque_wasm::lint_native(text, None).expect("lint_native must succeed");
        let single_diags: Vec<serde_json::Value> = single_ndjson
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let batch_diags: Vec<serde_json::Value> =
            batch[i]["diagnostics"].as_array().unwrap().to_vec();
        assert_eq!(
            batch[i]["id"], *id,
            "batch entry {i} id mismatch under wasm32"
        );
        assert_eq!(
            single_diags, batch_diags,
            "batch diagnostics for {id} must match single lint under wasm32"
        );
    }
}

// ---------------------------------------------------------------------------
// compute_banner_native on wasm32
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn wasm_compute_banner_rollup() {
    let text = "(S//NF) Portion 1\n(TS//SI//NF) Portion 2";
    let banner = marque_wasm::compute_banner_native(text)
        .expect("compute_banner_native must succeed under wasm32");
    assert_eq!(
        banner, "TOP SECRET//SI//NOFORN",
        "banner roll-up must match expected value under wasm32"
    );
}

#[wasm_bindgen_test]
fn wasm_compute_banner_unclassified() {
    // PageContext::max() of UNCLASSIFIED is UNCLASSIFIED — not empty.
    // Empty CAB and empty banner are different operations; generate_cab returns
    // empty for U-only, but compute_banner returns the rolled-up level.
    let text = "(U) Unclassified only";
    let banner = marque_wasm::compute_banner_native(text)
        .expect("compute_banner_native must succeed on unclassified input");
    assert_eq!(
        banner, "UNCLASSIFIED",
        "unclassified-only input must produce UNCLASSIFIED banner under wasm32"
    );
}

// ---------------------------------------------------------------------------
// generate_cab_native on wasm32
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn wasm_generate_cab_basic() {
    let text = "(S//NF) This is secret.\n(TS//SI//REL TO USA, GBR) Top secret.";
    let cab = marque_wasm::generate_cab_native(text, None, None)
        .expect("generate_cab_native must succeed under wasm32");
    assert!(
        cab.contains("Classified By:"),
        "CAB must contain 'Classified By:' under wasm32, got: {cab}"
    );
    assert!(
        cab.contains("Declassify On:"),
        "CAB must contain 'Declassify On:' under wasm32, got: {cab}"
    );
}

#[wasm_bindgen_test]
fn wasm_generate_cab_unclassified_empty() {
    let text = "(U) Unclassified portion";
    let cab = marque_wasm::generate_cab_native(text, None, None)
        .expect("generate_cab_native must succeed on unclassified input");
    assert_eq!(
        cab, "",
        "unclassified-only input must produce empty CAB under wasm32"
    );
}

// ---------------------------------------------------------------------------
// Config passthrough on wasm32
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn wasm_lint_corrections_config_passthrough() {
    let config = r#"{"corrections":{"NF":"NOFORN"}}"#;
    let result = marque_wasm::lint_native("SECRET//NF\n", Some(config.to_owned()))
        .expect("lint_native must accept corrections config under wasm32");
    assert!(
        result.contains("\"rule\":\"C001\""),
        "corrections config must trigger C001 under wasm32, got: {result}"
    );
}

#[wasm_bindgen_test]
fn wasm_lint_invalid_config_returns_error() {
    let result = marque_wasm::lint_native("SECRET//NF\n", Some("not json".to_owned()));
    assert!(
        result.is_err(),
        "invalid config JSON must return error under wasm32"
    );
}
