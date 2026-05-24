// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Belt-and-suspenders check that `marque-wasm` cannot receive
//! the `corpus-override` Cargo feature.
//!
//! Enforces Constitution III WASM-safety: the WASM artifact cannot
//! contain a corpus-override codepath that would let a caller inject
//! decoder priors at runtime. Enforcement is compile-time (the
//! `corpus-override` Cargo feature is absent), not disabled at runtime.
//!
//! ## Two independent defenses
//!
//! 1. **Feature-absence.** `crates/wasm/Cargo.toml [features]` does not
//!    declare `corpus-override`. Any `cargo build --features
//!    corpus-override -p marque-wasm` fails with "Package `marque-wasm`
//!    does not have feature `corpus-override`" before a line of code
//!    is compiled.
//!
//! 2. **Compile-error guard.** `crates/wasm/src/lib.rs` carries a
//!    `#[cfg(all(target_arch = "wasm32", feature = "corpus-override"))]
//!    compile_error!(...)` that fires if the feature is somehow
//!    enabled (e.g., transitively via a dependency tree edit). This
//!    file carries a sibling guard so the test build itself fails if
//!    the feature leaks in.
//!
//! The test below is a trivial runtime no-op; the load-bearing check
//! is the compile-time `cfg` guard. If the feature is absent (normal
//! state of the tree), the test compiles, runs, and passes. If a
//! future commit enables the feature, `cargo test -p marque-wasm`
//! fails to compile, which fails CI.

// Second line of defense: the test build itself refuses to compile if
// the feature is somehow enabled, regardless of target architecture.
// `cargo test` on native must never see this feature either, since the
// WASM-safe invariant applies to the crate as a whole.
#[cfg(feature = "corpus-override")]
compile_error!(
    "marque-wasm must not be built with the `corpus-override` feature. \
     T3 enforcement per docs/security/WHITEPAPER.md §10.3. \
     If a future change needs runtime decoder priors, add a dedicated \
     feature surface — do not reuse the CLI-only `corpus-override` one."
);

/// Compile-time invariant captured as a runtime-passing test.
///
/// The real check is the `compile_error!` above; this `#[test]` ensures
/// the file is exercised by `cargo test` so the check cannot be
/// silently removed from the test target.
#[test]
fn corpus_override_feature_is_absent_from_wasm_crate() {
    // If this test runs, compilation succeeded, which means the
    // compile_error! guard above did not fire, which means the
    // `corpus-override` feature is NOT enabled for this build. That
    // is the invariant.
    //
    // An explicit runtime assertion (always-true) keeps the body
    // non-empty so lints do not flag the test as accidentally empty.
    let feature_present = cfg!(feature = "corpus-override");
    assert!(
        !feature_present,
        "corpus-override feature must be absent; compile_error! should have caught this"
    );
}
