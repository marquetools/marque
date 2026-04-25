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

use crate::engine::InvalidThreshold;
use crate::output::LintResult;
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
                write!(f, "conflicting severity overrides for rule {rule_id}: ")?;
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

// ---------------------------------------------------------------------------
// Runtime engine errors (spec 005)
// ---------------------------------------------------------------------------

/// Runtime errors from `Engine::lint_with_options` /
/// `Engine::fix_with_options` (spec 005).
///
/// Distinct from [`EngineConstructionError`] by design — construction
/// errors are build-time configuration defects the integrator fixes
/// before shipping; `EngineError` reports runtime conditions (a
/// per-call deadline expired, a per-call threshold override is
/// out of range) the caller can react to. Keeping the two enums
/// separate means matching on one does not force callers to pattern
/// against build-time variants they could never encounter at
/// request time.
///
/// `#[non_exhaustive]` so future runtime conditions (memory budget
/// exceeded, per-rule deadline expired, cancellation token tripped)
/// can land without a semver-breaking change.
///
/// Spec §R5 (asymmetric response shape): the lint path does not
/// return `EngineError::DeadlineExceeded` on its own — partial lint
/// results are surfaced through `LintResult.truncated` instead, so
/// the caller can render whatever diagnostics were produced before
/// the abort. Only `fix_with_options` raises `DeadlineExceeded`,
/// because a partial `FixResult` would commit half a fix to the
/// audit stream (Constitution V Principle V).
#[non_exhaustive]
#[derive(Debug)]
pub enum EngineError {
    /// `fix_with_options` aborted before applying every fix because
    /// the call's deadline expired. `partial_lint` is the
    /// `LintResult` that the lint pass produced before the abort —
    /// callers can render its diagnostics to the user even though no
    /// fixes were committed. `partial_lint.truncated` indicates
    /// whether the lint pass itself was also truncated (deadline
    /// expired during scanning) versus the fix-application loop
    /// (lint pass completed, fixes did not).
    ///
    /// Carries the lint result by value (not boxed) because the
    /// happy path returns `Ok(FixResult)` and the size penalty on
    /// the error variant is paid only on the cold path.
    DeadlineExceeded { partial_lint: LintResult },
    /// `fix_with_options` rejected the per-call confidence
    /// threshold override. Wraps the existing standalone
    /// [`InvalidThreshold`] struct so `Engine::fix_with_threshold`
    /// can keep its `Result<FixResult, InvalidThreshold>` public
    /// signature unchanged while internally routing through
    /// `fix_with_options`.
    InvalidThreshold(InvalidThreshold),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeadlineExceeded { partial_lint } => write!(
                f,
                "engine deadline exceeded after processing {}/{} candidates",
                partial_lint.candidates_processed, partial_lint.candidates_total
            ),
            Self::InvalidThreshold(it) => it.fmt(f),
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // `DeadlineExceeded` is not caused by an inner error — it
            // reports a runtime condition (the deadline elapsed) with
            // no underlying failure to chain.
            Self::DeadlineExceeded { .. } => None,
            Self::InvalidThreshold(it) => Some(it),
        }
    }
}

impl From<InvalidThreshold> for EngineError {
    fn from(value: InvalidThreshold) -> Self {
        Self::InvalidThreshold(value)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_scheme::CategoryId;

    // -----------------------------------------------------------------------
    // EngineConstructionError::exit_code — completes coverage of all four
    // variants. `engine.rs` already covers UnknownRuleOverride,
    // ConflictingRuleOverride, and RewriteCycle; the unannotated-custom case
    // is exercised here.
    // -----------------------------------------------------------------------

    #[test]
    fn unannotated_custom_axes_exit_code_is_unavailable() {
        let err = EngineConstructionError::UnannotatedCustomAxes { rewrite: "bad" };
        assert_eq!(
            err.exit_code(),
            69,
            "scheme defects (not user-config) → EX_UNAVAILABLE"
        );
    }

    // -----------------------------------------------------------------------
    // EngineConstructionError::Display — round-trip every variant. Smoke
    // checks key strings appear so the message stays useful when a
    // contributor refactors the format string.
    // -----------------------------------------------------------------------

    #[test]
    fn rewrite_cycle_display_names_axis_and_members() {
        let err = EngineConstructionError::RewriteCycle {
            axis: CategoryId(0),
            members: Box::new(["alpha", "beta"]),
        };
        let msg = err.to_string();
        assert!(msg.contains("page-rewrite cycle"), "got: {msg}");
        assert!(msg.contains("alpha"), "got: {msg}");
        assert!(msg.contains("beta"), "got: {msg}");
    }

    #[test]
    fn unannotated_custom_axes_display_names_rewrite() {
        let err = EngineConstructionError::UnannotatedCustomAxes {
            rewrite: "noforn-clears-rel-to",
        };
        let msg = err.to_string();
        assert!(msg.contains("noforn-clears-rel-to"), "got: {msg}");
        assert!(msg.contains("explicit reads/writes"), "got: {msg}");
    }

    #[test]
    fn unknown_rule_override_display_with_suggestion() {
        let err = EngineConstructionError::UnknownRuleOverride {
            key: "E00l".into(),
            did_you_mean: Some("E001".into()),
        };
        let msg = err.to_string();
        assert!(msg.contains("E00l"), "got: {msg}");
        assert!(msg.contains("E001"), "suggestion missing: {msg}");
        assert!(msg.contains("did you mean"), "got: {msg}");
    }

    #[test]
    fn unknown_rule_override_display_without_suggestion_omits_did_you_mean() {
        let err = EngineConstructionError::UnknownRuleOverride {
            key: "totally-unknown".into(),
            did_you_mean: None,
        };
        let msg = err.to_string();
        assert!(msg.contains("totally-unknown"), "got: {msg}");
        assert!(
            !msg.contains("did you mean"),
            "no suggestion → no hint phrase: {msg}"
        );
    }

    #[test]
    fn conflicting_rule_override_display_lists_all_keys_and_severities() {
        let err = EngineConstructionError::ConflictingRuleOverride {
            rule_id: "E001".into(),
            keys: Box::new(["E001".into(), "portion-mark-in-banner".into()]),
            severities: Box::new(["warn".into(), "error".into()]),
        };
        let msg = err.to_string();
        assert!(msg.contains("E001"), "got: {msg}");
        assert!(msg.contains("portion-mark-in-banner"), "got: {msg}");
        assert!(msg.contains("warn"), "got: {msg}");
        assert!(msg.contains("error"), "got: {msg}");
    }

    // -----------------------------------------------------------------------
    // EngineConstructionError as `dyn Error` — confirms the trait impl
    // exists and `source()` returns `None` (none of these wrap an inner
    // error today).
    // -----------------------------------------------------------------------

    #[test]
    fn engine_construction_error_has_no_source() {
        let err = EngineConstructionError::UnannotatedCustomAxes { rewrite: "bad" };
        let as_error: &dyn std::error::Error = &err;
        assert!(as_error.source().is_none());
    }

    // -----------------------------------------------------------------------
    // EngineError — Phase 1 type. Display, Error::source, From.
    // -----------------------------------------------------------------------

    fn lint_result_with_counts(processed: usize, total: usize) -> LintResult {
        // In-crate construction MAY use struct-update syntax even with
        // `#[non_exhaustive]`. The fields stay public so external callers
        // can read counts off the partial_lint after a DeadlineExceeded.
        LintResult {
            diagnostics: Vec::new(),
            truncated: true,
            candidates_processed: processed,
            candidates_total: total,
            ..Default::default()
        }
    }

    #[test]
    fn deadline_exceeded_display_carries_processed_over_total() {
        let err = EngineError::DeadlineExceeded {
            partial_lint: lint_result_with_counts(7, 42),
        };
        let msg = err.to_string();
        assert!(msg.contains("deadline exceeded"), "got: {msg}");
        assert!(msg.contains("7/42"), "counts must appear as N/M: got {msg}");
    }

    #[test]
    fn deadline_exceeded_with_zero_counts_renders_zero_over_zero() {
        // Pre-pass abort path (deadline already expired before scanner)
        // produces 0/0 counts. The Display message should still be
        // legible — no division-by-zero artefacts, no empty fields.
        let err = EngineError::DeadlineExceeded {
            partial_lint: lint_result_with_counts(0, 0),
        };
        let msg = err.to_string();
        assert!(msg.contains("0/0"), "got: {msg}");
    }

    #[test]
    fn invalid_threshold_display_delegates_to_inner() {
        // `EngineError::InvalidThreshold` Display must produce the SAME
        // message as the wrapped `InvalidThreshold` — Phase 1 routes
        // `Engine::fix_with_threshold` errors through `EngineError` and
        // unwraps them at the boundary, so the user-visible string must
        // not drift between the two paths.
        let inner = InvalidThreshold(1.5);
        let wrapped = EngineError::InvalidThreshold(InvalidThreshold(1.5));
        assert_eq!(inner.to_string(), wrapped.to_string());
    }

    #[test]
    fn invalid_threshold_display_renders_nan() {
        // The wrapped Display must still produce something meaningful for
        // NaN — the underlying impl uses `{}` on f32 which prints "NaN".
        let err = EngineError::InvalidThreshold(InvalidThreshold(f32::NAN));
        let msg = err.to_string();
        assert!(msg.contains("NaN"), "got: {msg}");
    }

    #[test]
    fn deadline_exceeded_source_is_none() {
        // `DeadlineExceeded` reports a runtime condition with no
        // underlying failure — `source()` MUST be None so callers
        // walking the error chain don't trip on a phantom inner error.
        let err = EngineError::DeadlineExceeded {
            partial_lint: lint_result_with_counts(0, 0),
        };
        let as_error: &dyn std::error::Error = &err;
        assert!(as_error.source().is_none());
    }

    #[test]
    fn invalid_threshold_source_chains_to_inner() {
        // `InvalidThreshold(_)` MUST expose the wrapped error through
        // `source()` so callers can downcast / display the inner error
        // directly. The inner is the same `InvalidThreshold` struct
        // that `Engine::fix_with_threshold` returns directly to its
        // callers, so a chain walker sees a stable type.
        let err = EngineError::InvalidThreshold(InvalidThreshold(2.0));
        let as_error: &dyn std::error::Error = &err;
        let source = as_error.source().expect("InvalidThreshold has a source");
        // The inner Display matches the bare InvalidThreshold's Display.
        assert_eq!(source.to_string(), InvalidThreshold(2.0).to_string());
    }

    #[test]
    fn from_invalid_threshold_constructs_invalid_threshold_variant() {
        // `From<InvalidThreshold> for EngineError` is the conversion
        // `Engine::fix_with_options` uses internally; verifying it
        // produces the InvalidThreshold variant (not DeadlineExceeded
        // by mistake) pins the impl.
        let it = InvalidThreshold(-0.5);
        let err: EngineError = it.into();
        match err {
            EngineError::InvalidThreshold(inner) => {
                assert!(inner.0 == -0.5 || inner.0.is_nan());
            }
            other => panic!("expected InvalidThreshold variant, got {other:?}"),
        }
    }
}
