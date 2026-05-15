// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-capco — CAPCO rule implementations for marque.
//!
//! # Code Generation
//! Generated ISM vocabulary types and validation predicates live in `marque-ism`.
//! This crate provides the hand-written Layer 2 rule implementations that consume
//! those generated predicates to produce enriched diagnostics.
//!
//! # Schema Versioning
//! The active schema version is pinned in `marque-ism/Cargo.toml` under
//! `[package.metadata.marque]` — `ism-schema-version` for the upstream
//! ODNI label, `ism-data-version` for the [`marquetools/ism-data`](https://github.com/marquetools/ism-data)
//! workspace snapshot whose `ism` / `ism-ismcat` build-deps marque-ism
//! resolves, and `ismcat-tetra-version` for the Tetragraph Taxonomy
//! revision. Bump in lock-step when ODNI publishes spec updates and
//! `ism-data` is re-vendored.

pub mod lattice;
pub mod priors;
pub mod provenance;
pub(crate) mod render;
pub mod rules;
pub(crate) mod rules_declarative;
pub mod scheme;
pub mod vocab;
// `vocabulary` is implementation detail — it carries the
// `impl Vocabulary<CapcoScheme> for CapcoScheme` adapter and the
// internal `LazyLock`-backed metadata tables. Callers reach the
// trait surface through `marque_scheme::Vocabulary` + standard
// trait-method resolution, never via the module path.
mod vocabulary;

pub use lattice::{
    AeaPrimary, AeaSet, ClassificationLattice, DeclassifyOnLattice, DissemSet, FgiSet,
    NatoClassLattice, NatoDissemSet, SarSet, SciSet, UcniKind,
};
pub use marque_ism::CapcoTokenSet;
pub use provenance::DecoderProvenance;
pub use rules::CapcoRuleSet;
pub use scheme::{CapcoMarking, CapcoOpenVocabRef, CapcoScheme};
// PR 3d.3: surface the active-sentinel-set count so integration
// tests (notably `vocabulary_forms.rs`) can pin EXPECTED_FORMS
// against the authoritative size without coupling to private
// `vocabulary` module internals.
pub use vocabulary::active_sentinel_count;

use marque_rules::RuleSet;

/// Entry point: returns the CAPCO rule set for use by the engine.
pub fn capco_rules() -> impl RuleSet<CapcoScheme> {
    rules::CapcoRuleSet::new()
}

/// ISM schema version this crate was compiled against (from marque-ism).
pub const SCHEMA_VERSION: &str = marque_ism::generated::values::SCHEMA_VERSION;
