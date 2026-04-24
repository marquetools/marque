// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Engine construction errors.
//!
//! Defines the error surface the Phase 3 `Engine::new` constructor
//! (tasks T031–T032) will return when the scheme's declarative
//! artifacts (constraints, page rewrites) fail pre-flight checks.
//! The constructor will run a topological sort over
//! `PageRewrite::reads` / `writes` and fail closed with
//! [`EngineConstructionError::RewriteCycle`] when a cycle exists.
//!
//! The current `Engine::new` signature returns `Self` directly; the
//! transition to `Result<Self, EngineConstructionError>` lands
//! alongside T031–T032 when the scheduler that actually emits these
//! variants ships. Declaring the error surface in Phase 2 lets
//! downstream tooling (IDE plugins, the scheme-exploration CLI that
//! will land in Phase G) target a stable shape while the runtime
//! path catches up.
//!
//! Kept in its own module so callers can match on the error without
//! pulling in the runtime pipeline.

use marque_scheme::{CategoryId, RewriteId};

/// Errors that will be raised while constructing an `Engine`.
///
/// Every variant is intended to be a **hard** failure — the Phase 3
/// `Engine::new` implementation will return `Err` rather than
/// silently degrading. Runtime lint / fix never emits these; they are
/// build-time configuration errors the integrator is expected to
/// resolve before shipping.
///
/// Until that constructor path lands, this enum documents the planned
/// engine-construction error surface for downstream tooling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineConstructionError {
    /// A read/write cycle exists among the declared page rewrites.
    ///
    /// `axis` is one category in the cycle (there may be several — the
    /// engine reports the first one it hits during the topological
    /// sort). `members` names **every** rewrite participating in the
    /// cycle.
    ///
    /// The variable-length slice form (not `[RewriteId; 2]`) is
    /// deliberate: cycles of length ≥ 3 are a real failure mode —
    /// foundational-plan line 1066 notes the JOINT/FGI/REL-TO
    /// interaction as one that could plausibly trip this path if
    /// authored incorrectly.
    ///
    /// The list is owned (`Box<[RewriteId]>`, not `&'static [...]`)
    /// because cycle membership is computed at engine-construction
    /// time from the declared rewrite graph, not borrowed from a
    /// static table. Owning it here avoids the memory-leak /
    /// lifetime-gymnastics tradeoff a `'static` slice would force on
    /// the Phase 3 scheduler. `RewriteId` is itself `&'static str`,
    /// so the per-entry payload is still `'static`; only the
    /// container is heap-allocated.
    ///
    /// Fired by the Phase 3 scheduler when `Engine::new` runs Kahn's
    /// algorithm over the rewrite graph (tasks T031–T032).
    RewriteCycle {
        axis: CategoryId,
        members: Box<[RewriteId]>,
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
    /// A `[rules]` entry in the merged config references a key that is
    /// neither a known rule ID (e.g., `E001`) nor a known rule name
    /// (e.g., `portion-mark-in-banner`) across the registered rule sets.
    ///
    /// `key` is the unknown string as the user wrote it. `did_you_mean`
    /// is a best-effort suggestion based on edit distance against the
    /// union of known IDs and names — `None` when no candidate is close
    /// enough to be useful.
    ///
    /// Fired by `Engine::new` / `Engine::with_clock` when canonicalizing
    /// the config's severity overrides against the registered rules.
    /// This is a user-config error, not an internal invariant violation;
    /// `exit_code()` maps it to `EX_DATAERR` (65).
    UnknownRuleOverride {
        key: String,
        did_you_mean: Option<String>,
    },
    /// The user specified the same rule two different ways in the merged
    /// config (e.g., `E001 = "warn"` and `portion-mark-in-banner = "error"`)
    /// and the two entries resolved to different severity strings.
    ///
    /// Duplicate forms with the *same* severity are silently accepted —
    /// only a genuine value conflict hard-fails.
    ///
    /// `rule_id` is the canonical ID both keys resolved to. `keys`
    /// contains the two source keys as the user wrote them; `severities`
    /// contains the two conflicting severity strings, index-aligned with
    /// `keys`.
    ConflictingRuleOverride {
        rule_id: String,
        keys: Box<[String]>,
        severities: Box<[String]>,
    },
}

impl EngineConstructionError {
    /// Exit code for this error per `contracts/cli.md`.
    ///
    /// - `UnknownRuleOverride` / `ConflictingRuleOverride` → `EX_DATAERR`
    ///   (65). These are user-config defects — the `.marque.toml` refers
    ///   to a rule that doesn't exist, or contradicts itself — and the
    ///   user fixes them by editing their config.
    /// - `RewriteCycle` / `UnannotatedCustomAxes` → `EX_UNAVAILABLE`
    ///   (69). These are defects in the declarative scheme the engine
    ///   was built against (developer / rule-author errors, not
    ///   user-config errors), so the tool can't honor the request until
    ///   the developer ships a corrected build.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::RewriteCycle { .. } | Self::UnannotatedCustomAxes { .. } => 69,
            Self::UnknownRuleOverride { .. } | Self::ConflictingRuleOverride { .. } => 65,
        }
    }
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
            Self::UnknownRuleOverride { key, did_you_mean } => {
                write!(
                    f,
                    "unknown rule {key:?} in [rules] — no registered rule has this ID or name"
                )?;
                if let Some(hint) = did_you_mean {
                    write!(f, " (did you mean {hint:?}?)")?;
                }
                Ok(())
            }
            Self::ConflictingRuleOverride {
                rule_id,
                keys,
                severities,
            } => {
                write!(
                    f,
                    "conflicting severity overrides for rule {rule_id}: "
                )?;
                let mut first = true;
                for (k, s) in keys.iter().zip(severities.iter()) {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k:?} = {s:?}")?;
                    first = false;
                }
                write!(
                    f,
                    " — specify only one form (either the rule ID or the rule name), not both with different severities"
                )
            }
        }
    }
}

impl std::error::Error for EngineConstructionError {}
