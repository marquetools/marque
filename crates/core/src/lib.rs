// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-core — format-agnostic text scanner and attribute parser for the marque rule engine.
//!
//! This crate is WASM-safe: no format dependencies, no I/O, operates on `&[u8]`.
//! The pipeline entry point is [`Scanner`], which produces [`Span`]s that the
//! [`Parser`] converts into [`IsmAttributes`].
//!
//! Core ISM types (`Span`, `IsmAttributes`, `TokenSet`, etc.) are defined in
//! `marque-ism` and re-exported here for backward compatibility.

pub mod attrs;
pub mod error;
pub mod fuzzy;
pub mod parser;
pub mod scanner;
pub mod span;

pub use error::CoreError;
pub use marque_ism::{IsmAttributes, MarkingType, Span};
pub use parser::Parser;
pub use scanner::Scanner;
