//! marque-ism — ISM vocabulary types, generated CVE enums, and core spans.
//!
//! This crate is the leaf dependency in the marque workspace. It owns:
//! - `Span` and scanner candidate types (zero-copy position tracking)
//! - `IsmAttributes` (the canonical parsed marking representation)
//! - `TokenSet` trait and `CapcoTokenSet` (Aho-Corasick CVE token matching)
//! - Generated code from ODNI ISM schemas (CVE enums, validators, migrations)
//!
//! **WASM-safe**: no I/O, no format dependencies, no platform-specific code.

pub mod attrs;
pub mod generated;
pub mod span;
pub mod token_set;

// Re-export primary types at crate root for convenience.
pub use attrs::{Classification, DeclassOn, IsmAttributes, Trigraph};
pub use span::{Candidate, DocumentPosition, MarkingType, Span, Zone};
pub use token_set::{CapcoTokenSet, TokenSet};
