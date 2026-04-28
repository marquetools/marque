// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR #237 — allocator feature-matrix tests.
//!
//! PR #237 moved `spinning_top` from the `talc_alloc` dependency list into
//! `multi-threading` exclusively. The key invariants:
//!
//! | Feature             | `talc` present | `spinning_top` present |
//! |---------------------|----------------|------------------------|
//! | `web` (default)     | yes            | **no**                 |
//! | `cloud`             | yes            | **no**                 |
//! | `talc_alloc` only   | yes            | **no**                 |
//! | `multi-threading`   | yes            | **yes**                |
//! | `talc_debug`        | yes            | **yes**                |
//! | `cloud_talc`        | yes            | **yes**                |
//!
//! The tests below run `cargo tree` with the target feature set and assert
//! presence / absence. This catches a regression where `spinning_top` leaks
//! into single-threaded builds (introducing spinlock overhead on wasm32 where
//! it is incorrect) or disappears from multi-threaded builds (breaking the
//! TalcLock allocator).
//!
//! Tests run on the native host (not wasm32) by design — `cargo tree` is a
//! build-tool command and cannot execute inside a WASM runtime.

#![cfg(not(target_arch = "wasm32"))]

use std::process::Command;

/// Run `cargo tree -p marque-wasm --no-dedupe` for a given feature set and
/// return the stdout as a string.
///
/// `--no-dedupe` ensures every occurrence of a crate is listed even if it
/// appears multiple times in the tree; this guards against the case where
/// `spinning_top` is pulled in transitively but only appears once.
fn cargo_tree_features(features: &str) -> String {
    let mut args = vec![
        "tree",
        "-p",
        "marque-wasm",
        "--no-default-features",
        "--no-dedupe",
        "-e=no-dev",
        "--prefix",
        "none",
        "--format",
        "{p}",
    ];

    if !features.is_empty() {
        args.push("--features");
        args.push(features);
    }

    let output = Command::new("cargo")
        .args(&args)
        .output()
        .expect("failed to run `cargo tree`; is cargo installed?");

    assert!(
        output.status.success(),
        "cargo tree failed for features={features:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Returns `true` when `crate_name` appears as a standalone package in the
/// tree output (word-boundary match: "crate_name vX.Y.Z" but not
/// "crate_name-extra vX.Y.Z").
///
/// `cargo tree --format "{p}"` always produces "name version" lines, so the
/// crate name is always followed by a space and a version string. We match
/// only lines where a space immediately follows the crate name to avoid
/// false positives from crates with a longer shared prefix.
fn tree_has_crate(tree: &str, crate_name: &str) -> bool {
    tree.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(crate_name)
            && trimmed
                .get(crate_name.len()..)
                .is_some_and(|rest| rest.starts_with(' '))
    })
}

// ---------------------------------------------------------------------------
// Single-threaded features: talc present, spinning_top absent
// ---------------------------------------------------------------------------

/// `web` (the default feature) pulls in `talc_alloc`, which depends only on
/// `talc` — no `spinning_top`. A single-threaded web-worker build must not
/// carry spinlock overhead.
#[test]
fn web_feature_includes_talc_but_not_spinning_top() {
    let tree = cargo_tree_features("web");
    assert!(
        tree_has_crate(&tree, "talc"),
        "web feature must include `talc` (WasmDynamicTalc allocator)"
    );
    assert!(
        !tree_has_crate(&tree, "spinning_top"),
        "web feature must NOT include `spinning_top` (spinlock not needed for single-threaded \
         WasmDynamicTalc); PR #237 moved spinning_top to multi-threading only"
    );
}

/// `cloud` is a single-threaded deployment profile that also uses
/// `WasmDynamicTalc` without a spinlock.
#[test]
fn cloud_feature_includes_talc_but_not_spinning_top() {
    let tree = cargo_tree_features("cloud");
    assert!(
        tree_has_crate(&tree, "talc"),
        "cloud feature must include `talc` (WasmDynamicTalc allocator)"
    );
    assert!(
        !tree_has_crate(&tree, "spinning_top"),
        "cloud feature must NOT include `spinning_top`; single-threaded cloud deployments \
         do not require a spinlock allocator"
    );
}

/// `talc_alloc` alone (no `multi-threading`) uses `WasmDynamicTalc`.
#[test]
fn talc_alloc_alone_does_not_pull_in_spinning_top() {
    let tree = cargo_tree_features("talc_alloc");
    assert!(
        tree_has_crate(&tree, "talc"),
        "talc_alloc feature must include `talc`"
    );
    assert!(
        !tree_has_crate(&tree, "spinning_top"),
        "`talc_alloc` without `multi-threading` must NOT include `spinning_top`; \
         PR #237 removed spinning_top from the talc_alloc dependency list"
    );
}

// ---------------------------------------------------------------------------
// Multi-threaded / debug features: both talc and spinning_top present
// ---------------------------------------------------------------------------

/// `multi-threading` targets SharedArrayBuffer builds and requires `TalcLock`,
/// which needs `spinning_top`.
#[test]
fn multi_threading_feature_includes_both_talc_and_spinning_top() {
    let tree = cargo_tree_features("multi-threading");
    assert!(
        tree_has_crate(&tree, "talc"),
        "multi-threading feature must include `talc` (TalcLock allocator)"
    );
    assert!(
        tree_has_crate(&tree, "spinning_top"),
        "multi-threading feature must include `spinning_top` (required by TalcLock); \
         PR #237 moved spinning_top here from talc_alloc"
    );
}

/// `talc_debug` uses `TalcLock` with heap counters — requires `spinning_top`.
#[test]
fn talc_debug_feature_includes_both_talc_and_spinning_top() {
    let tree = cargo_tree_features("talc_debug");
    assert!(
        tree_has_crate(&tree, "talc"),
        "talc_debug feature must include `talc`"
    );
    assert!(
        tree_has_crate(&tree, "spinning_top"),
        "talc_debug feature must include `spinning_top` (TalcLock-based debug allocator)"
    );
}

/// `cloud_talc` = `cloud` + `multi-threading`, so it inherits `spinning_top`.
#[test]
fn cloud_talc_feature_includes_both_talc_and_spinning_top() {
    let tree = cargo_tree_features("cloud_talc");
    assert!(
        tree_has_crate(&tree, "talc"),
        "cloud_talc feature must include `talc`"
    );
    assert!(
        tree_has_crate(&tree, "spinning_top"),
        "cloud_talc feature must include `spinning_top` (inherits multi-threading)"
    );
}
