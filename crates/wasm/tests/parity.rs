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
    #[allow(dead_code)] // category is informational; assertions use `name`
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
