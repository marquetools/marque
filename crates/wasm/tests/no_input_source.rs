// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T015 — the WASM build must not accept an `input_source` parameter or
//! config field (#176 FR-031 / Constitution III).
//!
//! `InputSource::StructuredField` raises the recognizer's lone-case
//! posterior and licenses assertive fixes (SC-010). Honoring a
//! caller-supplied `input_source` from behind the WASM postMessage
//! boundary would be caller-provided posterior modulation on an
//! uninspected trust boundary — exactly the capability channel
//! Constitution III forbids the WASM target from accepting at runtime.
//! The WASM build therefore PINS `InputSource::DocumentContent`:
//!
//! 1. **Compile-time**: the `lint` / `fix` wasm-bindgen exports take no
//!    `InputSource` parameter (`lint(text, config_json)` /
//!    `fix(text, threshold, config_json)`), and `marque-wasm` calls the
//!    engine's plain `lint`/`fix` (the `DocumentContent` path) — never
//!    `lint_with_input_context`. There is no surface through which a
//!    WASM caller could request `StructuredField`.
//! 2. **Runtime accept-list**: `WasmConfig` is a closed set of six
//!    fields (`classifier_id`, `classification_authority`,
//!    `confidence_threshold`, `corrections`, `deadline_ms`,
//!    `signature`); `input_source` is deliberately absent, so a config
//!    JSON carrying it is silently ignored (unknown fields are dropped).
//!
//! This test is the regression pin for (2): a config carrying
//! `input_source` produces output identical to the bare config,
//! proving the field is ignored rather than honored. It mirrors
//! `no_corpus_override.rs`, the analogous capability-channel guard.

use marque_wasm::lint_native;

#[test]
fn wasm_ignores_input_source_field() {
    // A lone marking-shaped token: `(YS)` typed as if into a field.
    // Under `InputSource::StructuredField` the decoder's lone-case
    // heuristic WOULD fix `YS → TS` (SC-010). The WASM build must NOT
    // honor a caller-supplied `input_source`, so the bare config and
    // the `input_source`-carrying config must produce IDENTICAL output
    // — both on the conservative `DocumentContent` path where the lone
    // `(YS)` is left alone.
    let bare = r#"{"confidence_threshold": 0.9}"#;
    let with_source = r#"{"confidence_threshold": 0.9, "input_source": "StructuredField"}"#;

    let text = "(YS)";
    let out_bare = lint_native(text, Some(bare.to_string())).expect("bare config lints");
    let out_source =
        lint_native(text, Some(with_source.to_string())).expect("input_source config lints");

    assert_eq!(
        out_bare, out_source,
        "WASM must ignore an input_source config field — StructuredField \
         raises recognizer posteriors and Constitution III forbids \
         accepting it at the WASM runtime boundary (FR-031)"
    );
}
