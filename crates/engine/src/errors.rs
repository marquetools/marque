// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Engine construction errors.
//!
//! Raised at [`crate::Engine::new`] when the scheme's declarative
//! artifacts (constraints, page rewrites) fail pre-flight checks. The
//! Phase 3 `Engine::new` implementation (tasks T031–T032) runs a
//! topological sort over `PageRewrite::reads` / `writes` and fails
//! closed with [`EngineConstructionError::RewriteCycle`] when a cycle
//! exists.
//!
//! Kept in its own module so downstream tooling (IDE plugins, the
//! scheme-exploration CLI that will land in Phase G) can match on the
//! error surface without pulling in the runtime pipeline.

use marque_scheme::{CategoryId, RewriteId};

/// Errors raised while constructing an `Engine`.
///
/// Every variant is a **hard** failure — `Engine::new` returns `Err`
/// rather than silently degrading. Runtime lint / fix never emits
/// these; they are build-time configuration errors the integrator is
/// expected to resolve before shipping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineConstructionError {
    /// A read/write cycle exists among the declared page rewrites.
    ///
    /// `axis` is one category in the cycle (there may be several — the
    /// engine reports the first one it hits during the topological
    /// sort). `members` names **every** rewrite participating in the
    /// cycle. The slice form (not `[RewriteId; 2]`) is deliberate:
    /// cycles of length ≥ 3 are a real failure mode — foundational-
    /// plan line 1066 notes the JOINT/FGI/REL-TO interaction as one
    /// that could plausibly trip this path if authored incorrectly.
    ///
    /// Fired by the Phase 3 scheduler when `Engine::new` runs Kahn's
    /// algorithm over the rewrite graph (tasks T031–T032).
    RewriteCycle {
        axis: CategoryId,
        members: &'static [RewriteId],
    },
    /// A `PageRewrite::custom` was declared without explicit
    /// `reads` / `writes` (or with empty slices).
    ///
    /// The `declarative` constructor derives these from the variant
    /// shapes; `custom` uses function pointers so the engine cannot
    /// derive them. Failing closed forces the rewrite author to
    /// annotate the dataflow explicitly — an un-annotated `custom`
    /// rewrite could not be scheduled relative to other rewrites.
    UnannotatedCustomAxes { rewrite: RewriteId },
}

impl std::fmt::Display for EngineConstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RewriteCycle { axis, members } => {
                write!(f, "page-rewrite cycle on category {axis:?}: {members:?}")
            }
            Self::UnannotatedCustomAxes { rewrite } => write!(
                f,
                "custom page-rewrite {rewrite:?} was declared without explicit reads/writes"
            ),
        }
    }
}

impl std::error::Error for EngineConstructionError {}
