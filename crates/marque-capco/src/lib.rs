//! marque-capco — CAPCO rule implementations for marque.
//!
//! # Code Generation
//! Generated ISM vocabulary types and validation predicates live in `marque-ism`.
//! This crate provides the hand-written Layer 2 rule implementations that consume
//! those generated predicates to produce enriched diagnostics.
//!
//! # Schema Versioning
//! The active schema version is pinned in `marque-ism/Cargo.toml` under
//! `[package.metadata.marque] ism-schema-version`. Bump intentionally when ODNI
//! publishes spec updates.

pub mod rules;

pub use marque_ism::CapcoTokenSet;

use marque_rules::RuleSet;

/// Entry point: returns the CAPCO rule set for use by the engine.
pub fn capco_rules() -> impl RuleSet {
    rules::CapcoRuleSet::new()
}

/// ISM schema version this crate was compiled against (from marque-ism).
pub const SCHEMA_VERSION: &str = marque_ism::generated::values::SCHEMA_VERSION;
