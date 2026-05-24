// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-core — format-agnostic text scanner and attribute parser for the marque rule engine.
//!
//! This crate is WASM-safe: no format dependencies, no I/O, operates on `&[u8]`.
//! The pipeline entry point is [`Scanner`], which produces [`Span`]s that the
//! [`Parser`] converts into `marque_ism::ParsedAttrs<'src>`. The engine
//! then runs `MarkingScheme::canonicalize` (the trait route — for CAPCO
//! that is `CapcoScheme::canonicalize`) to land owned [`CanonicalAttrs`]
//! for rule consumption. That trait method is the sole production
//! `ParsedAttrs → CanonicalAttrs` path.
//!
//! Core ISM types (`Span`, `CanonicalAttrs`, `TokenSet`, etc.) are defined in
//! `marque-ism` and re-exported here for ergonomic access.

pub mod error;
pub mod fuzzy;
pub mod parser;
pub mod scanner;

pub use error::CoreError;
pub use parser::Parser;
pub use scanner::Scanner;
