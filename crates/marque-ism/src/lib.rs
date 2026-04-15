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
pub mod marking_forms;
pub mod page_context;
pub mod span;
pub mod token_set;

// Re-export primary types at crate root for convenience.
pub use attrs::{
    AeaMarking, Classification, DeclassExemption, DissemControl, FgiClassification, FgiMarker,
    ForeignClassification, FrdBlock, IsmAttributes, JointClassification, MarkingClassification,
    NatoClassification, NatoLevel, NonIcDissem, RdBlock, SarIdentifier, SciCompartment,
    SciControl, SciControlBare, SciControlSystem, SciMarking, TokenKind, TokenSpan, Trigraph,
};
pub use generated::values::{is_bare_cve_value, SCHEMA_VERSION};
pub use page_context::PageContext;
pub use span::{DocumentPosition, MarkingCandidate, MarkingType, Span, Zone};
pub use token_set::{CapcoTokenSet, TokenSet};
