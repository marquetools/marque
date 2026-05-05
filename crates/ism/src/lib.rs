// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![deny(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-ism — ISM vocabulary types, generated CVE enums, and core spans.
//!
//! This crate is a leaf dependency in the marque workspace. It owns:
//! - `Span` and scanner candidate types (zero-copy position tracking)
//! - The pivot type pair ([`ParsedAttrs<'src>`], [`CanonicalAttrs`]) and the
//!   `from_parsed_unchecked` transitional adapter that bridges parser output
//!   to rule input. (`ProjectedMarking` — the third leg of the
//!   data-model.md split — is deferred to PR 5/6 where it has consumers;
//!   defining it in `marque-ism` would require a `marque-scheme` dep that
//!   violates Constitution VII's peer-leaf placement.)
//! - `TokenSet` trait and `CapcoTokenSet` (Aho-Corasick CVE token matching)
//! - Generated code from ODNI ISM schemas (CVE enums, validators, migrations)
//!
//! **WASM-safe**: no I/O, no format dependencies, no platform-specific code.

pub mod attrs;
pub mod canonical;
pub mod date;
pub mod generated;
pub mod marking_forms;
pub mod page_context;
pub mod parsed;
pub mod span;
pub mod token_set;

// Re-export primary types at crate root for convenience.
pub use attrs::{
    AeaMarking, Classification, CountryCode, DeclassExemption, DissemControl, FgiClassification,
    FgiMarker, ForeignClassification, FrdBlock, JointClassification, MarkingClassification,
    NatoClassification, NatoLevel, NonIcDissem, RdBlock, SarCompartment, SarIndicator, SarMarking,
    SarProgram, SciCompartment, SciControl, SciControlBare, SciControlSystem, SciMarking,
    TokenKind, TokenSpan,
};
pub use canonical::{CanonicalAttrs, from_parsed_unchecked};
pub use date::{ApproxIsmDate, ApproxQualifier, IsmDate, ParseIsmDateError, UtcOffset};
pub use generated::values::{
    ISMCAT_TETRA_VERSION, SCHEMA_VERSION, TETRAGRAPH_MEMBERS, TRIGRAPHS, TetragraphProvenance,
    is_bare_cve_value, is_decomposable, lookup_tetragraph_members, lookup_tetragraph_provenance,
};
pub use page_context::{PageContext, sar_sort_key};
pub use parsed::{
    ParsedAea, ParsedAttrs, ParsedClassification, ParsedDeclassifyOn, ParsedDissem,
    ParsedFgiMarker, ParsedNonIcDissem, ParsedRelToEntry, ParsedSarMarking, ParsedSciMarking,
    SourceOrigin,
};
pub use span::{DocumentPosition, MarkingCandidate, MarkingType, Span, Zone};
pub use token_set::{CapcoTokenSet, TokenSet};
