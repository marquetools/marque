// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! No-I/O dependency audit.
//!
//! Asserts that the `marque-wasm` crate's non-dev dependency tree contains
//! no I/O crates (filesystem, network, async runtime), so the WASM build
//! attempts no file system or network access.

#![cfg(not(target_arch = "wasm32"))]

use std::process::Command;

/// Banned crate names that indicate filesystem, network, or TLS dependencies.
/// If any of these appear in the non-dev dependency tree, the WASM build would
/// carry I/O capabilities it must not have.
///
/// Note: `tokio`, `mio`, and `socket2` are NOT banned — they are transitive
/// deps of `marque-engine` (via `recoco-utils`) and compile for wasm32 without
/// providing real I/O. On `wasm32-unknown-unknown` there are no filesystem or
/// network syscalls regardless. What we ban are crates that provide *real*
/// HTTP/TLS networking capabilities and marque crates with filesystem deps.
const BANNED_CRATES: &[&str] = &[
    "reqwest",
    "hyper",
    "axum",
    "native-tls",
    "openssl",
    "rustls",
    "marque-extract",
    "marque-server",
];

/// Run `cargo tree` for the WASM crate targeting `wasm32-unknown-unknown`
/// (excluding dev-dependencies) and return the stdout output.
/// Uses the actual WASM target graph, not the host-target graph, so
/// cfg-gated dependencies are resolved correctly.
fn wasm_dep_tree() -> String {
    let output = Command::new("cargo")
        .args([
            "tree",
            "-p",
            "marque-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "-e=no-dev",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .output()
        .expect("failed to run `cargo tree`; is cargo installed?");

    assert!(
        output.status.success(),
        "cargo tree failed (is the wasm32-unknown-unknown target installed? \
         run `rustup target add wasm32-unknown-unknown`): {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Check whether a crate name appears in the dep tree output using word-boundary
/// matching: "crate_name vX.Y.Z" but not "crate_name-extra vX.Y.Z".
fn tree_contains_crate(tree_output: &str, crate_name: &str) -> bool {
    tree_output.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(crate_name)
            && trimmed
                .get(crate_name.len()..)
                .is_some_and(|rest| rest.starts_with(' ') || rest.is_empty())
    })
}

#[test]
fn wasm_dep_tree_has_no_io_crates() {
    let tree_output = wasm_dep_tree();

    for banned in BANNED_CRATES {
        assert!(
            !tree_contains_crate(&tree_output, banned),
            "FR-013 violation: banned crate `{banned}` found in marque-wasm dependency tree.\n\
             The WASM build must not contain filesystem or network dependencies.\n\
             Run `cargo tree -p marque-wasm -e=no-dev` to investigate."
        );
    }
}

#[test]
fn wasm_dep_tree_no_extract_crate() {
    // Redundant with the above but makes the intent explicit: marque-extract
    // (Kreuzberg wrapper) must NEVER be a dependency of the WASM crate.
    let tree_output = wasm_dep_tree();
    assert!(
        !tree_contains_crate(&tree_output, "marque-extract"),
        "marque-extract must not be a dependency of marque-wasm (format extraction is caller's responsibility)"
    );
}
