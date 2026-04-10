//! marque-core — format-agnostic scanner and parser for IC classification markings.
//!
//! This crate is WASM-safe: no format dependencies, no I/O, operates on `&[u8]`.
//! The pipeline entry point is [`Scanner`], which produces [`Span`]s that the
//! [`Parser`] converts into [`IsmAttributes`].
//!
//! Core ISM types (`Span`, `IsmAttributes`, `TokenSet`, etc.) are defined in
//! `marque-ism` and re-exported here for backward compatibility.

pub mod attrs;
pub mod error;
pub mod parser;
pub mod scanner;
pub mod span;

pub use error::CoreError;
pub use marque_ism::{IsmAttributes, MarkingType, Span};
pub use parser::Parser;
pub use scanner::Scanner;
