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

#[cfg(feature = "batch")]
pub mod batch;
pub mod clock;
pub mod decoder;
pub mod engine;
pub mod errors;
pub mod options;
pub mod output;
pub mod pipeline;
pub mod recognizer;
pub mod scheduler;

#[cfg(feature = "batch")]
pub use batch::{BatchEngine, BatchError, BatchOptions};
pub use clock::{Clock, FixedClock, SystemClock};
pub use decoder::{DecoderRecognizer, StrictOrDecoderRecognizer};
pub use engine::{Engine, FixMode, InvalidThreshold};

/// Type alias marking the intentional CAPCO-pinning of [`Engine`].
///
/// [`Engine`] is currently NOT generic over the marking scheme — its
/// internal recognizer dispatch, decoder paths, and provenance
/// handling are CAPCO-typed. The output types (`Diagnostic<S>`,
/// `AppliedFix<S>`, `FixIntent<S>`, `LintResult`, `FixResult`) ARE
/// generic, so the rule-emission surface is scheme-neutral. Engine
/// generification (`Engine<S>` / `BatchEngine<S>`) is scheduled
/// alongside the audit-schema flip in PR 3c.B Commit 10 — see
/// `docs/plans/2026-05-10-pr3c-consolidated-plan.md`.
///
/// `CapcoEngine` exists so that `Engine` call sites that conceptually
/// "want a CapcoScheme engine" can express that intent in the type
/// system today, and so that Commit 10's generification PR can land
/// `Engine<S>` alongside `pub type CapcoEngine = Engine<CapcoScheme>;`
/// without breaking any call site.
pub type CapcoEngine = Engine;
pub use errors::{EngineConstructionError, EngineError};
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
/// deadline check (spec 005) calls `Instant::now()` whenever a
/// caller-supplied deadline is set, so any embedder constructing a
/// `LintOptions { deadline: Some(_) }` for the WASM target MUST use
/// this `Instant` (or `web_time::Instant` directly) rather than
/// `std::time::Instant`. CLI and server callers can keep using
/// `std::time::Instant` because the two types are identical on
/// native, and those binaries do not target wasm32-unknown-unknown.
pub use web_time::Instant;

/// Audit-record schema version emitted by this build.
///
/// Set at build time by `crates/engine/build.rs` (see `MARQUE_AUDIT_SCHEMA`),
/// validated against the closed accept-list `["marque-mvp-1", "marque-mvp-2"]`.
/// Defaults to `"marque-mvp-2"` (Phase D); a build can downgrade by exporting
/// `MARQUE_AUDIT_SCHEMA=marque-mvp-1`. Re-exported through this crate so CLI
/// and WASM emitters can populate the `schema` field without each owning a
/// separate copy of the constant (whitepaper §980 / FR-014).
///
/// Per FR-014 the value is fixed for the lifetime of a build — a single
/// binary emits exactly one schema, never a mix.
pub const AUDIT_SCHEMA_VERSION: &str = env!("MARQUE_AUDIT_SCHEMA");

/// `true` when this build emits Phase-D audit records (`marque-mvp-2`),
/// `false` when emitting the legacy `marque-mvp-1` shape.
///
/// Evaluated at compile time from [`AUDIT_SCHEMA_VERSION`]; the comparison
/// against a `&'static str` literal folds to a constant, so callers using
/// `if AUDIT_SCHEMA_IS_V2 { ... } else { ... }` get dead-branch elimination
/// at the matching schema's expense.
pub const AUDIT_SCHEMA_IS_V2: bool = const_str_eq(AUDIT_SCHEMA_VERSION, "marque-mvp-2");

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
