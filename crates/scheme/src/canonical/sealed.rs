// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Private sealing module for [`super::CanonicalConstructor`].
//!
//! External crates cannot name [`Sealed`] because the module path
//! `marque_scheme::canonical::sealed` is private — `mod sealed;` in
//! `super` does NOT export the module. Therefore external crates
//! cannot satisfy the `CanonicalConstructor<S>: Sealed<S>` supertrait
//! bound and cannot implement [`super::CanonicalConstructor`].
//!
//! This is the standard Rust API-guidelines sealed-trait pattern; see
//! <https://rust-lang.github.io/api-guidelines/future-proofing.html>.
//!
//! # Compile-fail proof
//!
//! The `compile_fail` doctest demonstrating that `use
//! marque_scheme::canonical::sealed::Sealed;` does not resolve from
//! an external crate lives on [`super::CanonicalConstructor`] in
//! `crates/scheme/src/canonical.rs` (alongside the related "external
//! crate cannot impl `CanonicalConstructor`" and "assoc-fn shorthand
//! cannot bypass `__engine_construct`" proofs). Run via
//! `cargo test --doc -p marque-scheme`. The integration tests at
//! `crates/scheme/tests/canonical_unconstructable.rs` carry the
//! complementary positive controls (`from_cve` reachable, engine
//! open-vocab path reachable under the test-fixture carve-out).

use crate::scheme::MarkingScheme;

/// Sealing marker. Crate-private; cannot be implemented outside
/// `marque-scheme`. The generic parameter mirrors the marker on
/// [`super::CanonicalConstructor`] so the seal is per-scheme rather
/// than universal.
pub trait Sealed<S: MarkingScheme + ?Sized> {}
