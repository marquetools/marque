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

// `fact_bitmask` is internal API — `#[doc(hidden)]` keeps it out of
// rustdoc; `pub` is required because integration tests in `tests/`
// link against the crate as an external dependency and need access
// to the projection helpers. The production consumer (`CLOSURE_TABLE`)
// and `CapcoScheme::closure` are wired separately. At that
// point this visibility tightens to `pub(crate)` and the doc-hidden
// attribute is unnecessary.
#[doc(hidden)]
pub mod fact_bitmask;

// `build_inputs` is imported by `build.rs` via
// `#[path = "src/build_inputs.rs"] mod build_inputs;` (a module
// import that pulls the .rs file in directly, bypassing
// `src/lib.rs`'s module graph) AND exposed here as a regular `pub
// mod` so the `build_input_pin_test` integration test can re-verify
// the same digest at test time. The two surfaces share the constant
// declaration; bumping the pin is a single-site edit.
//
// `pub(crate)` would suffice for runtime use, but the integration
// test under `crates/capco/tests/` needs `pub` reach.
#[doc(hidden)]
pub mod build_inputs;

// `closure_table` lives under `scheme/` but is re-exported here at
// `#[doc(hidden)] pub` so integration tests in `tests/` can reach
// `CLOSURE_TABLE` / `close` / `ALL_TRIGGER_MASK` /
// `MAX_CLOSURE_ITERATIONS` for the equivalence cross-check and the
// proptests. `CapcoScheme::closure` consumes it directly through the
// `scheme::closure_table::*` path; the latency gate
// tighten this visibility back to `pub(crate)` once the integration
// tests migrate to consuming through `MarkingScheme::closure`'s
// observable behavior.
#[doc(hidden)]
pub use scheme::closure_table;
pub mod lattice;
pub mod priors;
pub mod provenance;
pub(crate) mod render;
pub mod rules;
pub mod scheme;
pub mod vocab;
// `vocabulary` is implementation detail — it carries the
// `impl Vocabulary<CapcoScheme> for CapcoScheme` adapter and the
// internal `LazyLock`-backed metadata tables. Callers reach the
// trait surface through `marque_scheme::Vocabulary` + standard
// trait-method resolution, never via the module path.
mod vocabulary;

pub use lattice::{
    AeaPrimary, AeaSet, ClassificationLattice, DeclassifyOnLattice, DissemSet, FgiSet, JointSet,
    NatoClassLattice, NatoDissemSet, RelToBlock, SarSet, SciSet, UcniKind,
};
pub use marque_ism::CapcoTokenSet;
pub use provenance::{DecoderProvenance, HEURISTIC_RECOGNITION_CAP, build_decoder_diagnostic};
pub use rules::CapcoRuleSet;
pub use scheme::{CapcoMarking, CapcoOpenVocabRef, CapcoScheme};
// Surface the active-sentinel-set count so integration
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

/// Version of CAPCO's lattice algebra — the meet/join semantics and the
/// category set (`SciSet` / `SarSet` / `FgiSet`, the §3.3a equal-depth
/// meet, NOFORN supersession, the `Lattice` split from #456 / PR #502).
///
/// Tracked independently of [`SCHEMA_VERSION`]: the ODNI CVE package
/// label can move without changing the lattice semantics, and the
/// lattice can evolve (a new axis, a changed meet) against a fixed CVE
/// package. Surfaced into audit-record session metadata via
/// `MarkingScheme::lattice_version`. Bump on any change to the lattice
/// laws or the category set.
pub const LATTICE_VERSION: &str = "capco-lattice-1";
