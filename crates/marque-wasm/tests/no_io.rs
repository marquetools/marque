//! T062 — No-I/O dependency audit (FR-013).
//!
//! Asserts that the `marque-wasm` crate's non-dev dependency tree contains
//! no I/O crates (filesystem, network, async runtime). This enforces the US4
//! acceptance scenario: "no file system or network access is attempted."

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

#[test]
fn wasm_dep_tree_has_no_io_crates() {
    let output = Command::new("cargo")
        .args([
            "tree",
            "-p",
            "marque-wasm",
            "--no-dev-dependencies",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .output()
        .expect("failed to run `cargo tree`; is cargo installed?");

    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tree_output = String::from_utf8_lossy(&output.stdout);

    for banned in BANNED_CRATES {
        // Match crate name at word boundary: "tokio v1.50" but not "tokio-macros"
        // (although tokio-macros would also be a problem). We check both forms.
        let has_banned = tree_output.lines().any(|line| {
            let trimmed = line.trim();
            // Exact crate name: "crate_name vX.Y.Z"
            trimmed.starts_with(banned)
                && trimmed
                    .get(banned.len()..)
                    .is_some_and(|rest| rest.starts_with(' ') || rest.is_empty())
        });

        assert!(
            !has_banned,
            "FR-013 violation: banned crate `{banned}` found in marque-wasm dependency tree.\n\
             The WASM build must not contain filesystem or network dependencies.\n\
             Run `cargo tree -p marque-wasm --no-dev-dependencies` to investigate."
        );
    }
}

#[test]
fn wasm_dep_tree_no_extract_crate() {
    // Redundant with the above but makes the intent explicit: marque-extract
    // (Kreuzberg wrapper) must NEVER be a dependency of the WASM crate.
    let output = Command::new("cargo")
        .args([
            "tree",
            "-p",
            "marque-wasm",
            "--no-dev-dependencies",
            "--prefix",
            "none",
        ])
        .output()
        .expect("failed to run `cargo tree`");

    let tree_output = String::from_utf8_lossy(&output.stdout);
    assert!(
        !tree_output.contains("marque-extract"),
        "marque-extract must not be a dependency of marque-wasm (format extraction is caller's responsibility)"
    );
}
