//! marque-capco — CAPCO rule implementations for marque.
//!
//! # Code Generation
//! `build.rs` parses ODNI ISM specification files from `schemas/ISM-v<version>/`:
//! - `CVEValues.xml`  → `generated/values.rs`   (token enumerations, lookup tables)
//! - `ISM-XML.xsd`   → `generated/schema.rs`    (attribute constraints)
//! - `ISM-XML.sch`   → `generated/validators.rs` (Schematron assertion predicates)
//!
//! Generated files are in `.gitignore`; they are always rebuilt from schema sources.
//!
//! # Schema Versioning
//! The active schema version is pinned in `Cargo.toml` under
//! `[package.metadata.marque] ism-schema-version`. Bump intentionally when ODNI
//! publishes spec updates. Previous schema versions are retained under `schemas/`
//! to support marking migration rules.

// Generated code — produced by build.rs, never hand-edited.
// mod generated {
//     pub mod values;       // CVE enumerations
//     pub mod validators;   // Schematron-derived predicates
//     pub mod migrations;   // deprecated value → replacement mappings
// }

pub mod rules;
pub mod token_set;

pub use token_set::CapcoTokenSet;

use marque_rules::RuleSet;

/// Entry point: returns the CAPCO rule set for use by the engine.
pub fn capco_rules() -> impl RuleSet {
    rules::CapcoRuleSet::new()
}

/// ISM schema version this crate was compiled against.
pub const SCHEMA_VERSION: &str = "2022-DEC";
