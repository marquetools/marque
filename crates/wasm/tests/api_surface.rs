// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T067c — WASM deep-scan API-surface compile test.
//!
//! Phase 4 PR-4b / FR-013a / `cli-server-wasm-gates.md` Gate 2 require
//! that `lint_deep_scan` and `fix_deep_scan` accept ONLY the byte
//! buffer — no config struct, no priors override, no threshold
//! parameter. This file pins that signature contract at compile time
//! using function-pointer coercion. A future change that adds a
//! parameter to either export fails to compile here, with a type-error
//! pointing at the offending signature.
//!
//! ## Scope: native-callable forms only
//!
//! The pin covers the native-callable surface
//! (`lint_deep_scan_native` / `fix_deep_scan_native`), which is what
//! every internal caller and parity test goes through. The `#[wasm_bindgen]`
//! exports `lint_deep_scan` / `fix_deep_scan` are thin wrappers around
//! those natives that adapt the error type from `String` to `JsValue`
//! and forward the byte buffer unchanged — adding a parameter to a
//! wasm-bindgen export without first adding it to its native peer is
//! not an addition pattern any reasonable refactor would take.
//!
//! Pinning the wasm-bindgen signatures directly would require the
//! wasm32 target to instantiate `JsValue` (and `wasm-bindgen`'s proc
//! macro), so the file is `#![cfg(not(target_arch = "wasm32"))]`. The
//! tighter end-to-end signature gate for the wasm-bindgen layer lives
//! in the wasm-pack build itself (CI runs `wasm-pack build --target
//! web --release`, which fails compilation if the bindgen-export
//! signatures drift from valid `JsValue`-bearing forms).

#![cfg(not(target_arch = "wasm32"))]

use marque_wasm::{fix_deep_scan_native, lint_deep_scan_native};

/// Compile-time pin: `lint_deep_scan_native` takes exactly one
/// parameter, `&[u8]`, and returns `Result<String, String>`.
///
/// Adding a config-struct parameter to the function would change its
/// type; the assignment below would fail with `expected fn(&[u8]) ->
/// _, found fn(&[u8], _) -> _`.
#[allow(dead_code)]
const _LINT_DEEP_SCAN_SIGNATURE: fn(&[u8]) -> Result<String, String> = lint_deep_scan_native;

/// Compile-time pin: `fix_deep_scan_native` takes exactly one
/// parameter, `&[u8]`, and returns `Result<String, String>`.
///
/// In particular, the absence of a `threshold: f32` second parameter
/// (which `fix_native` carries) is the Gate-2 enforcement: a WASM
/// deep-scan caller cannot tune the fix gate at runtime.
#[allow(dead_code)]
const _FIX_DEEP_SCAN_SIGNATURE: fn(&[u8]) -> Result<String, String> = fix_deep_scan_native;

#[test]
fn deep_scan_signatures_are_pinned() {
    // The const items above are the load-bearing checks. This test
    // exists so a hostile reorder that demoted them to private
    // function-bodies (where they could be optimized away or
    // accidentally deleted) shows up as a missing test — and so a
    // human reading the test file knows what the const pin is FOR.
    let _: fn(&[u8]) -> Result<String, String> = _LINT_DEEP_SCAN_SIGNATURE;
    let _: fn(&[u8]) -> Result<String, String> = _FIX_DEEP_SCAN_SIGNATURE;
}
