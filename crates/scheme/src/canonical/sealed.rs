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
//! `crates/scheme/tests/canonical_unconstructable.rs` carries a
//! `compile_fail` doctest demonstrating that `use
//! marque_scheme::canonical::sealed::Sealed;` does not resolve from
//! an external crate.

use crate::scheme::MarkingScheme;

/// Sealing marker. Crate-private; cannot be implemented outside
/// `marque-scheme`. The generic parameter mirrors the marker on
/// [`super::CanonicalConstructor`] so the seal is per-scheme rather
/// than universal.
pub trait Sealed<S: MarkingScheme + ?Sized> {}
