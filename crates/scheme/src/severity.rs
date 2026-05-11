// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Diagnostic severity levels.
//!
//! Lives in `marque-scheme` (the foundation leaf) so that
//! [`crate::constraint::ConstraintViolation`] and other scheme-layer
//! types can carry per-row severity without taking a dependency on
//! `marque-rules` (which would violate Constitution VII —
//! `marque-scheme` is the only true graph leaf).
//!
//! `marque-rules` re-exports `Severity` from this module, so existing
//! `marque_rules::Severity` import sites continue to work unchanged.
//! The single definition lives here.

/// Diagnostic severity level.
///
/// Severity controls both the CLI exit-code impact and the engine's
/// auto-apply gating. The variant ordering is intentionally
/// total-ordered (Off < Suggest < Info < Warn < Error < Fix) so
/// `.max()`-based strictness-only merging is available if a future
/// config-merge policy requires it.
///
/// # Variants
///
/// - **`Off`** — Rule is disabled entirely. FR-008: an `Off`-severity
///   diagnostic is unrepresentable, because a rule configured `Off`
///   never fires.
/// - **`Suggest`** — Advisory channel — diagnostic carries a candidate
///   fix that will **not** auto-apply. Distinct from `Info` (FYI, no
///   actionable replacement) and from `Off` (non-firing). The
///   fix-bearing diagnostic remains visible in lint output but the
///   engine excludes it from auto-apply regardless of `confidence`.
///   This is the suggest-don't-fix channel: rules with low-confidence
///   candidate corrections (e.g., `S004 rel-to-trigraph-suggest`) can
///   surface "did you mean?" hints without committing to the rewrite.
///   `Suggest` keeps the CLI exit code at `0` (same as `Info`), so it
///   is CI-silent.
/// - **`Info`** — Emit informational diagnostic; does not block
///   `check`-mode exit code. Intended for "audit-visible but probably
///   intentional" signals — cases where the marking may be correct
///   but the user may want to verify (e.g., unpublished SCI control
///   systems).
/// - **`Warn`** — Emit warning; non-error, but still non-zero in
///   `check` mode (produces `EX_DIAG_WARN` = 2). Different from
///   `Info` in tone *and* exit-code impact: Warn is "this might be
///   wrong" and CI-visible; Info is "FYI, probably intentional but
///   worth surfacing" and CI-silent (exit 0).
/// - **`Error`** — Emit error; blocks `--check` exit code.
/// - **`Fix`** — Apply fix automatically when `--fix` flag is present.
///
/// # Merge semantics
///
/// `marque-config` merges layers in strict precedence order — env vars
/// override `.marque.local.toml` which overrides `.marque.toml`.
/// Whatever the highest-precedence layer says for a given rule wins,
/// including downgrades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Rule is disabled entirely. FR-008: severity=off is
    /// unrepresentable on emitted diagnostics — a rule at `Off` never
    /// fires, so no `Diagnostic` is produced.
    Off,
    /// Advisory channel — diagnostic carries a candidate fix that
    /// will **not** auto-apply.
    Suggest,
    /// Emit informational diagnostic; does not block `check`-mode
    /// exit code.
    Info,
    /// Emit warning; non-error, but still non-zero in `check` mode
    /// (produces `EX_DIAG_WARN` = 2).
    Warn,
    /// Emit error; blocks `--check` exit code.
    Error,
    /// Apply fix automatically when `--fix` flag is present.
    Fix,
}

impl Severity {
    /// Parse a severity level from a config string. Returns `None`
    /// for unrecognized values; the config loader treats `None` as a
    /// hard error.
    pub fn parse_config(s: &str) -> Option<Self> {
        match s {
            "off" => Some(Self::Off),
            "suggest" => Some(Self::Suggest),
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            "fix" => Some(Self::Fix),
            _ => None,
        }
    }

    /// Canonical lowercase string form, suitable for JSON output.
    ///
    /// This is the inverse of [`Severity::parse_config`] and is the
    /// stable surface that JSON consumers should depend on — never
    /// `format!("{:?}")` (which exposes Debug formatting as an
    /// unintended API).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Suggest => "suggest",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Fix => "fix",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
