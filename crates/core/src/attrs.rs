// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Re-exports from `marque-ism` for ergonomic access from `marque-core`.
//!
//! Canonical definitions live in `marque_ism::attrs` and the new
//! `marque_ism::canonical` module that PR 3a introduced.

pub use marque_ism::attrs::{Classification, CountryCode};
pub use marque_ism::canonical::CanonicalAttrs;
