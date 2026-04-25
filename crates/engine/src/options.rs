// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Per-call options for `Engine::lint_with_options` and
//! `Engine::fix_with_options` (spec 005).
//!
//! These types are the durable surface for runtime budgets and
//! per-call overrides. Phase 1 lands the type surface with no
//! observable behavior change — the deadline field is plumbed but
//! not yet honored. Phase 2 wires cooperative cancellation against
//! `LintOptions::deadline` / `FixOptions::deadline` per spec §R3.
//!
//! Both structs are `#[non_exhaustive]` so future fields (cancellation
//! tokens, memory budgets, per-rule deadlines) can land without a
//! semver-breaking change. From outside the engine crate, construct
//! via `Default::default()` + public field assignment:
//!
//! ```
//! use marque_engine::{LintOptions, FixOptions};
//! use std::time::{Duration, Instant};
//!
//! let mut lint = LintOptions::default();
//! lint.deadline = Some(Instant::now() + Duration::from_secs(1));
//!
//! let mut fix = FixOptions::default();
//! fix.deadline = Some(Instant::now() + Duration::from_secs(1));
//! fix.threshold_override = Some(0.85);
//! ```
//!
//! In-crate code (engine internals, this crate's tests) may still use
//! struct-update syntax — `#[non_exhaustive]` only restricts
//! construction across crate boundaries.

// `web_time::Instant` is `std::time::Instant` on native targets and a
// Performance.now() polyfill on wasm32-unknown-unknown. Identical type
// on native (literal `pub use` re-export), so this is source-compatible
// with any caller that previously constructed an `Instant` from
// `std::time`.
use web_time::Instant;

/// Per-call options for [`Engine::lint_with_options`].
///
/// **Phase 1 status (current build):** the type surface ships, but
/// `Engine::lint_with_options` IGNORES `deadline`. The pass always
/// runs to completion, returns `truncated: false`, and leaves
/// `candidates_processed` / `candidates_total` at `0`. The semantics
/// below describe the *Phase 2* behavior that lands in tasks
/// T007–T009; consult the changelog (Appendix C in the security
/// whitepaper) before relying on deadline behavior in production.
///
/// `deadline` is an absolute wall-clock instant after which the
/// engine MUST abort cooperatively. Spec §R1, §R3:
///
/// - `None` (default) — no budget; lint runs to completion.
/// - `Some(d)` where `d <= Instant::now()` — pre-pass abort returns
///   immediately with `LintResult { truncated: true,
///   candidates_processed: 0, candidates_total: 0, diagnostics:
///   vec![] }`.
/// - `Some(d)` where `d > Instant::now()` — engine checks the deadline
///   at each candidate boundary; on expiry the loop breaks and
///   `LintResult.truncated` is set to `true` with partial counts.
///
/// The choice of `Instant` over `Duration` is deliberate: callers
/// stamp the deadline once at the boundary they care about
/// (request arrival, document permit acquisition for batch) and
/// the engine carries no implicit clock. This makes the budget
/// composable across `BatchEngine` permit waits and HTTP middleware.
///
/// [`Engine::lint_with_options`]: crate::Engine::lint_with_options
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct LintOptions {
    /// Absolute wall-clock deadline after which the lint pass MUST
    /// abort cooperatively. See struct-level docs for semantics —
    /// **and the Phase 1 status note**: the current build ignores
    /// this field, deadline-driven cancellation lands in Phase 2.
    pub deadline: Option<Instant>,
}

/// Per-call options for [`Engine::fix_with_options`].
///
/// **Phase 1 status (current build):** `Engine::fix_with_options`
/// IGNORES `deadline` (the field is plumbed but not honored), so
/// `EngineError::DeadlineExceeded` cannot be observed yet. The
/// `threshold_override` field IS active from Phase 1: invalid values
/// produce `EngineError::InvalidThreshold` immediately. Deadline
/// enforcement and the asymmetric `Err(DeadlineExceeded)` response
/// described below land in Phase 2 (tasks T010–T012).
///
/// Carries both the deadline (spec §R3) and the per-call confidence
/// threshold override that previously lived on
/// [`Engine::fix_with_threshold`]. The two are combined here so
/// future per-call concerns (per-rule overrides, dry-run-without-mode
/// flag) can join without further signature churn.
///
/// `deadline` semantics: same as [`LintOptions::deadline`], but the
/// engine returns `Err(EngineError::DeadlineExceeded { partial_lint })`
/// rather than a partial `FixResult`. Spec §R4 (asymmetric response):
/// a partial `FixResult` would commit half a fix to the audit stream,
/// which violates Constitution V Principle V (audit-record integrity).
///
/// `threshold_override`:
/// - `None` (default) — falls back to `Config::confidence_threshold`.
/// - `Some(value)` — replaces the config threshold for this call only;
///   validated against `[0.0, 1.0]`. Out-of-range / NaN values produce
///   `EngineError::InvalidThreshold` at the start of the call.
///
/// [`Engine::fix_with_options`]: crate::Engine::fix_with_options
/// [`Engine::fix_with_threshold`]: crate::Engine::fix_with_threshold
/// [`LintOptions::deadline`]: crate::LintOptions::deadline
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct FixOptions {
    /// Absolute wall-clock deadline. See [`LintOptions::deadline`] for
    /// the semantic shape; the difference for `fix` is that expiry
    /// returns `Err(EngineError::DeadlineExceeded)`, not a partial
    /// success.
    ///
    /// **Phase 1 status:** ignored by the current build; deadline
    /// enforcement lands in Phase 2.
    ///
    /// [`LintOptions::deadline`]: crate::LintOptions::deadline
    pub deadline: Option<Instant>,
    /// Per-call confidence threshold override; `None` = use config.
    /// Values outside `[0.0, 1.0]` (including NaN) produce
    /// `EngineError::InvalidThreshold`. Active from Phase 1.
    pub threshold_override: Option<f32>,
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn lint_options_default_yields_no_deadline() {
        let opts = LintOptions::default();
        assert!(opts.deadline.is_none());
    }

    #[test]
    fn fix_options_default_yields_no_deadline_and_no_threshold_override() {
        let opts = FixOptions::default();
        assert!(opts.deadline.is_none());
        assert!(opts.threshold_override.is_none());
    }

    #[test]
    fn lint_options_supports_struct_update_syntax() {
        // Forward-compat smoke test — `#[non_exhaustive]` requires
        // struct-update syntax for in-crate construction with new
        // fields. Verifying the pattern compiles documents the
        // expected idiom for callers.
        let now = Instant::now();
        let opts = LintOptions {
            deadline: Some(now),
            ..Default::default()
        };
        assert_eq!(opts.deadline, Some(now));
    }

    #[test]
    fn fix_options_supports_struct_update_syntax() {
        let now = Instant::now();
        let opts = FixOptions {
            deadline: Some(now),
            threshold_override: Some(0.5),
            ..Default::default()
        };
        assert_eq!(opts.deadline, Some(now));
        assert_eq!(opts.threshold_override, Some(0.5));
    }
}
