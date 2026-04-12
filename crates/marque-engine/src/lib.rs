//! marque-engine — pipeline orchestration.
//!
//! Wires together: scanner → parser → validator → fixer → output.
//! The pipeline is a chain of async streams; each stage is a `Stream` impl.
//! CLI, WASM, and server are different Source/Sink configurations wired to the same middle.

#[cfg(feature = "batch")]
pub mod batch;
pub mod clock;
pub mod engine;
pub mod output;
pub mod pipeline;

#[cfg(feature = "batch")]
pub use batch::{BatchEngine, BatchError, BatchOptions};
pub use clock::{Clock, FixedClock, SystemClock};
pub use engine::{Engine, FixMode, InvalidThreshold};
pub use output::{FixResult, LintResult};

/// Returns the default rule set for marque (CAPCO rules).
///
/// Both the CLI and WASM front ends use this to share one registration entry point.
pub fn default_ruleset() -> Vec<Box<dyn marque_rules::RuleSet>> {
    vec![Box::new(marque_capco::rules::CapcoRuleSet::new())]
}
