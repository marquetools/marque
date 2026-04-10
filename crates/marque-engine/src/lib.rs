//! marque-engine — pipeline orchestration.
//!
//! Wires together: scanner → parser → validator → fixer → output.
//! The pipeline is a chain of async streams; each stage is a `Stream` impl.
//! CLI, WASM, and server are different Source/Sink configurations wired to the same middle.

pub mod batch;
pub mod engine;
pub mod output;
pub mod pipeline;

pub use batch::{BatchEngine, BatchOptions};
pub use engine::Engine;
pub use output::{FixResult, LintResult};
