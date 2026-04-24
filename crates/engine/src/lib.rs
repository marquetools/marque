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
pub mod engine;
pub mod errors;
pub mod output;
pub mod pipeline;
pub mod recognizer;
pub mod scheduler;

#[cfg(feature = "batch")]
pub use batch::{BatchEngine, BatchError, BatchOptions};
pub use clock::{Clock, FixedClock, SystemClock};
pub use engine::{Engine, FixMode, InvalidThreshold};
pub use errors::EngineConstructionError;
pub use output::{FixResult, LintResult};
pub use recognizer::{StrictRecognizer, shift_token_spans};

/// Returns the default rule set for marque (CAPCO rules).
///
/// Both the CLI and WASM front ends use this to share one registration entry point.
pub fn default_ruleset() -> Vec<Box<dyn marque_rules::RuleSet>> {
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
