// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-time `Send + Sync` assertions for the concrete `CapcoScheme`
//! recognizer dispatch (FR-038, T003 of engine-refactor-006). The stub-scheme
//! companion lives at `crates/scheme/tests/send_sync.rs`; this file pins the
//! property for the only scheme that ships in production today. Adding new
//! schemes (e.g. a future `marque-cui` adapter) MUST add the equivalent
//! file under that crate's `tests/`.
//!
//! `marque-engine` is a dev-dependency here (path-only, see
//! `crates/capco/Cargo.toml`); the runtime dep flows the other way
//! (`marque-engine` consumes `marque-capco`). Keeping this assertion in
//! a `tests/` file rather than inside the lib crate keeps the WASM-safe
//! set honest under Constitution III + VII — `marque-capco` MUST NOT
//! gain a non-dev dep on the engine.

use marque_capco::CapcoScheme;
use marque_engine::{StrictOrDecoderRecognizer, StrictRecognizer};
use marque_scheme::Recognizer;
use std::sync::Arc;

const _: fn() = || {
    fn _assert_send_sync<T: ?Sized + Send + Sync>() {}

    _assert_send_sync::<dyn Recognizer<CapcoScheme>>();
    _assert_send_sync::<Box<dyn Recognizer<CapcoScheme>>>();
    _assert_send_sync::<Arc<dyn Recognizer<CapcoScheme>>>();
    _assert_send_sync::<StrictRecognizer>();
    _assert_send_sync::<StrictOrDecoderRecognizer>();
};

#[test]
fn capco_recognizer_dispatch_is_send_sync() {
    // The const-fn block above is the assertion; this test exists so
    // `cargo test -p marque-capco --test send_sync` lights up the crate's
    // test surface and the assertions get compiled in CI.
}
