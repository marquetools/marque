// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-engine — pipeline orchestration.
//!
//! Wires together: scanner → parser → validator → fixer → output.
//! The pipeline is a chain of async streams; each stage is a `Stream` impl.
//! CLI, WASM, and server are different Source/Sink configurations wired to the same middle.

pub mod audit_render;
#[cfg(feature = "batch")]
pub mod batch;
pub mod clock;
pub mod decoder;
pub mod engine;
pub mod errors;
pub mod merkle;
pub mod options;
pub mod output;
pub mod pipeline;
pub mod recognizer;
pub mod scheduler;
pub mod session;
mod text_correction;

#[cfg(feature = "batch")]
pub use batch::{BatchEngine, BatchError, BatchOptions};
pub use clock::{Clock, FixedClock, SystemClock};
pub use decoder::{DECODER_VERSION, DecoderRecognizer, StrictOrDecoderRecognizer};
pub use engine::{CapcoEngine, Engine, EngineRecognizer, FixMode, InvalidThreshold, R002_RULE_ID};
pub use session::{InterfaceCode, SessionMetadata};

pub use audit_render::{audit_line_to_json_v1_0, audit_line_to_ndjson};
pub use errors::{EngineConstructionError, EngineError};
pub use marque_scheme::{InputContext, InputSource};
pub use merkle::{SessionRoot, merkle_root};
pub use options::{FixOptions, LintOptions};
pub use output::{FixResult, LintResult};
pub use pipeline::{Sink, Source, SourceError, TextChunk};
pub use recognizer::StrictRecognizer;

/// Re-export of [`web_time::Instant`].
///
/// On native targets this is `std::time::Instant` verbatim
/// (`web_time` `pub use`s the std type). On `wasm32-unknown-unknown`
/// it's a `Performance.now()` / `Date.now()` polyfill — `std::time::
/// Instant::now()` panics on that target. The engine's per-candidate
/// deadline check calls `Instant::now()` whenever a
/// caller-supplied deadline is set, so any embedder constructing a
/// `LintOptions { deadline: Some(_) }` for the WASM target MUST use
/// this `Instant` (or `web_time::Instant` directly) rather than
/// `std::time::Instant`. CLI and server callers can keep using
/// `std::time::Instant` because the two types are identical on
/// native, and those binaries do not target wasm32-unknown-unknown.
pub use web_time::Instant;

/// Audit-record schema version emitted by this build.
///
/// Set at build time by `crates/engine/build.rs` (see
/// `MARQUE_AUDIT_SCHEMA`), validated against the closed accept-list
/// `["marque-3.2"]`. Defaults to `"marque-3.2"`. Re-exported
/// through this crate so CLI and WASM emitters can populate the
/// `schema` field without each owning a separate copy of the constant.
///
/// The value is fixed for the lifetime of a build — a single binary
/// emits exactly one schema, never a mix.
///
/// The current schema is `"marque-3.2"`: every audit-record `"rule"`
/// field serializes as a structured `{ scheme, predicate_id }` object,
/// never a flat string, and the record carries a BLAKE3 digest, closed
/// `MessageTemplate` JSON serialization, and `Canonical<S>` provenance.
/// `marque-3.2` (issue #399) adds the additive session-level
/// `session_metadata` record (see [`SessionMetadata`]). There is no
/// audit-reader crate for older record shapes — they are not
/// interoperable with current binaries (clean break).
pub const AUDIT_SCHEMA_VERSION: &str = env!("MARQUE_AUDIT_SCHEMA");

/// Marque core version surfaced into audit-record session metadata.
///
/// This is the engine crate's `CARGO_PKG_VERSION`. The engine is the
/// audit emitter and the convergence point of the crate graph, so its
/// version is the canonical "which Marque made this change" answer for
/// [`SessionMetadata::marque_version`].
pub const MARQUE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// `true` when this build emits `marque-3.2` audit records.
///
/// Evaluated at compile time from [`AUDIT_SCHEMA_VERSION`]; folds
/// to a constant. The accept-list is currently a single value, so
/// the const is always `true` in any successfully-built binary;
/// the const exists to give downstream code a stable shape-discriminant
/// across future schema bumps. `marque-3.2` (issue #399) is additive
/// over `marque-3.1`: it adds the session-level `session_metadata`
/// record (engine/lattice/decoder versions, integrity seal, applying
/// interface, classifier identity, optional carry-only signature),
/// emitted as the first line of a non-empty audit stream and covered by
/// the terminal `session_root` Merkle root, while leaving the
/// `AppliedFix` / `TextCorrection` per-record shapes byte-identical.
pub const AUDIT_SCHEMA_IS_V3_2: bool = const_str_eq(AUDIT_SCHEMA_VERSION, "marque-3.2");

const fn const_str_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Returns the default rule set for marque (CAPCO rules).
///
/// Both the CLI and WASM front ends use this to share one registration entry point.
pub fn default_ruleset() -> Vec<Box<dyn marque_rules::RuleSet<marque_capco::CapcoScheme>>> {
    vec![Box::new(marque_capco::rules::CapcoRuleSet::new())]
}

/// Returns the default marking scheme for marque (CAPCO).
///
/// Callers pass this to [`Engine::new`] to get the standard CAPCO
/// page-rewrite schedule. The scheme is stateless and cheap to
/// construct on demand.
pub fn default_scheme() -> marque_capco::scheme::CapcoScheme {
    marque_capco::scheme::CapcoScheme::new()
}
