// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Build-time integrity pins for vendored authoritative sources.
//!
//! This module is imported by both `build.rs` (via
//! `#[path = "src/build_inputs.rs"] mod build_inputs;` — a module
//! import that pulls the file in directly without going through
//! `src/lib.rs`'s module graph) and the runtime crate (via the
//! `pub mod build_inputs;` declaration in `src/lib.rs`) so the same
//! pin constants are visible to:
//!
//! 1. `build.rs::verify_capco_2016_md` — runs at compile time, panics
//!    the build if the vendored markdown drifts from the pinned
//!    digest.
//! 2. `crates/capco/tests/build_input_pin_test.rs` — runs at test
//!    time, re-computes the digest and asserts equality. Defense in
//!    depth: protects against `--offline` runs or cached `OUT_DIR`
//!    skipping the `build.rs` re-execution path. Either gate firing
//!    is a Constitution VIII violation.
//!
//! See `crates/capco/build.rs::verify_capco_2016_md` for the failure
//! message that ships with the build-time gate.

/// BLAKE3 digest (lowercase hex) of
/// `crates/capco/docs/CAPCO-2016.md`.
///
/// Computed via `b3sum crates/capco/docs/CAPCO-2016.md` from the workspace root
/// (or `b3sum docs/CAPCO-2016.md` from `crates/capco/`).
/// Bumping this constant is a deliberate, reviewed action — see the
/// build.rs failure message for the propagation checklist.
pub const CAPCO_2016_MD_BLAKE3: &str =
    "9395240efdcad6704cf5c4d63c7fb01d16ec4d10635673214a20dcf0bca8620c";
