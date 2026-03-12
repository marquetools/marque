//! marque-core — format-agnostic scanner and parser for IC classification markings.
//!
//! This crate is WASM-safe: no format dependencies, no I/O, operates on `&[u8]`.
//! The pipeline entry point is [`Scanner`], which produces [`Span`]s that the
//! [`Parser`] converts into [`IsmAttributes`].

pub mod scanner;
pub mod parser;
pub mod span;
pub mod attrs;
pub mod error;

pub use attrs::IsmAttributes;
pub use span::{Span, MarkingType};
pub use scanner::Scanner;
pub use parser::Parser;
pub use error::CoreError;
