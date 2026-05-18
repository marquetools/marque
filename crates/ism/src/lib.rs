// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![deny(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-ism — ISM vocabulary types, generated CVE enums, and core spans.
//!
//! This crate is the foundational vocabulary crate of the marque
//! workspace. It depends on `marque-scheme` (one-way edge —
//! `ProjectedMarking::scope` carries `marque_scheme::Scope`) and is
//! depended on by `marque-core` / `marque-rules` / `marque-capco` /
//! `marque-engine`. See Constitution VII v1.4.0 for the canonical
//! dep-graph diagram. It owns:
//! - `Span` and scanner candidate types (zero-copy position tracking)
//! - The pivot type triple ([`ParsedAttrs<'src>`], [`CanonicalAttrs`],
//!   [`ProjectedMarking`]) and the `from_parsed_unchecked` transitional
//!   adapter that bridges parser output to rule input. PR 6 wires
//!   `ProjectedMarking` into the engine alongside the `Scope::Page`
//!   projection cutover.
//! - `TokenSet` trait and `CapcoTokenSet` (Aho-Corasick CVE token matching)
//! - Generated code from ODNI ISM schemas (CVE enums, validators, migrations)
//!
//! **WASM-safe**: no I/O, no format dependencies, no platform-specific code.

pub mod attrs;
pub mod canonical;
pub mod companion_dedup;
pub mod date;
pub mod dissem_attribution;
pub mod generated;
pub mod marking_forms;
pub mod parsed;
pub mod projected;
pub mod sar_sort;
pub mod span;
pub mod token_set;

// Re-export primary types at crate root for convenience.
pub use attrs::{
    AeaMarking, AtomalBlock, Classification, CountryCode, DeclassExemption, DissemControl,
    FgiClassification, FgiMarker, ForeignClassification, FrdBlock, JointClassification,
    MarkingClassification, NatoClassification, NatoLevel, NatoSap, NonIcDissem, RdBlock,
    SarCompartment, SarIndicator, SarMarking, SarProgram, SciCompartment, SciControl,
    SciControlBare, SciControlSystem, SciMarking, TokenKind, TokenSpan,
};
pub use canonical::{CanonicalAttrs, from_parsed_unchecked};
pub use companion_dedup::dedup_companions;
pub use date::{ApproxIsmDate, ApproxQualifier, IsmDate, ParseIsmDateError, UtcOffset};
pub use dissem_attribution::{DefaultOrigin, attribute_dissems};
pub use generated::values::{
    ISMCAT_TETRA_VERSION, SCHEMA_VERSION, TETRAGRAPH_MEMBERS, TRIGRAPHS, TetragraphProvenance,
    is_bare_cve_value, is_decomposable, lookup_tetragraph_members, lookup_tetragraph_provenance,
};
// PR 4b-E: `sar_sort_key` lives in its own module post-relocation; the
// re-export at the crate root preserves the `marque_ism::sar_sort_key`
// public path (architect plan §3 Decision 4).
pub use parsed::{
    ParsedAea, ParsedAttrs, ParsedClassification, ParsedDeclassifyOn, ParsedDisplayOnlyEntry,
    ParsedDissem, ParsedFgiMarker, ParsedNonIcDissem, ParsedRelToEntry, ParsedSarMarking,
    ParsedSciMarking, SourceOrigin,
};
pub use projected::{ProjectedMarking, ProjectionProvenance};
pub use sar_sort::sar_sort_key;
pub use span::{DocumentPosition, MarkingCandidate, MarkingType, Span, Zone};
pub use token_set::{CapcoTokenSet, TokenSet};
