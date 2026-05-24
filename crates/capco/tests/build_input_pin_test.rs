// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Defense-in-depth runtime verification of vendored authoritative-
//! source integrity pins.
//!
//! `build.rs::verify_capco_2016_md` runs at compile time and panics
//! the build if `docs/CAPCO-2016.md` has drifted from the pinned
//! BLAKE3 digest in `crates/capco/src/build_inputs.rs`. The build-
//! time gate covers `cargo build` / `cargo check` / `cargo test`
//! runs that recompile the crate.
//!
//! But the build-time gate has a known evasion path: `cargo
//! test --offline` against a cached `target/` directory can skip
//! the `build.rs` re-execution if cargo's freshness check decides
//! none of the `rerun-if-changed` paths have moved. This is rare in
//! CI (where `Swatinem/rust-cache` invalidates aggressively on
//! source change) but possible in a developer's local environment
//! after a deliberate `cargo build` followed by an edit to the
//! vendored markdown.
//!
//! This integration test reads `docs/CAPCO-2016.md` at test time
//! and asserts the same pin. It runs on every `cargo test` even
//! when `build.rs` is skipped, closing the evasion channel.
//!
//! # Failure
//!
//! On mismatch, the panic message mirrors `build.rs`'s
//! `verify_capco_2016_md` so the operator sees the same propagation
//! checklist whether the build-time or test-time gate fires.

use std::fs;
use std::path::PathBuf;

use marque_capco::build_inputs;

/// Re-hash `docs/CAPCO-2016.md` at test time and assert the pin.
#[test]
fn capco_2016_md_matches_pinned_blake3() {
    let path = workspace_capco_docs().join("CAPCO-2016.md");
    let bytes = fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "failed to read {}: {err}. The vendored CAPCO-2016 markdown is the \
             authoritative source for every CAPCO §-citation in the rule \
             catalog (Constitution VIII). Restore from git or regenerate from \
             the original PDF.",
            path.display(),
        );
    });
    let actual = blake3::hash(&bytes);
    let actual_hex = actual.to_hex();
    assert_eq!(
        actual_hex.as_str(),
        build_inputs::CAPCO_2016_MD_BLAKE3,
        "\
\n\
BLAKE3 digest mismatch on {} (defense-in-depth runtime check;\n\
build.rs::verify_capco_2016_md is the primary gate). The vendored\n\
CAPCO-2016 markdown has been modified without an intentional pin\n\
bump — this is a Constitution VIII / Authoritative Source Fidelity\n\
violation. See build.rs for the propagation checklist.\n\
\n\
expected: {}\n\
actual:   {}\n",
        path.display(),
        build_inputs::CAPCO_2016_MD_BLAKE3,
        actual_hex,
    );
}

/// Resolve the workspace-root `crates/capco/docs/` from the test's
/// `CARGO_MANIFEST_DIR` (which IS `crates/capco/` for an integration
/// test in this crate).
fn workspace_capco_docs() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("docs")
}
