// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Per-row `r002_fired` inspectability locks for `BatchEngine`.
//!
//! Per-row `FixResult.r002_fired` is individually inspectable inside a
//! batch run, regardless of whether other rows triggered R002 or not.
//! The batch exit-code aggregation lives in the CLI loop (NOT in
//! `BatchEngine`); this test verifies the per-row field is readable so
//! the CLI aggregation can rely on it.
//!
//! R002 itself is not exercised here because no production Localized
//! rule emits a `FixIntent`-shape fix today, so R002 is structurally
//! unreachable through the existing CAPCO ruleset. The test pins the
//! per-row INSPECTABILITY property — the load-bearing piece for the CLI
//! aggregation — and not the R002 trigger condition itself.

#![cfg(feature = "batch")]

use futures::StreamExt;
use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{BatchEngine, BatchOptions, CapcoEngine};

fn engine() -> CapcoEngine {
    CapcoEngine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[tokio::test]
async fn r002_fired_per_row_inspectable() {
    // Mixed batch: clean rows + a row that exercises E002. Every
    // returned `FixResult` MUST expose `r002_fired` as a readable
    // bool. With today's production ruleset every value is `false`,
    // but a consumer that reads the field MUST be able to do so
    // without crashing or special-casing.
    let opts = BatchOptions::default();
    let batch = BatchEngine::new(engine(), opts);
    let docs: Vec<(String, Vec<u8>)> = vec![
        (
            "clean-1".to_owned(),
            b"(U) Plain unclassified content.\n".to_vec(),
        ),
        ("e002".to_owned(), b"SECRET//REL TO GBR\n".to_vec()),
        ("clean-2".to_owned(), b"(U) Another plain block.\n".to_vec()),
    ];

    let stream = batch.fix_many(docs);
    futures::pin_mut!(stream);
    let mut count_inspected = 0;
    while let Some((id, result)) = stream.next().await {
        let fix_result = result.expect("row should not error");
        // Pin: `r002_fired` is a readable field on `FixResult`.
        let _ = fix_result.r002_fired;
        assert!(
            !fix_result.r002_fired,
            "no production Localized rule triggers R002 today; \
             row {id} fired R002 unexpectedly"
        );
        count_inspected += 1;
    }
    assert_eq!(count_inspected, 3, "all three rows should arrive");
}
