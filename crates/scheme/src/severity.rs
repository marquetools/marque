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
/// - **`Off`** — Rule is disabled entirely. An `Off`-severity
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
    /// Rule is disabled entirely. severity=off is unrepresentable on
    /// emitted diagnostics — a rule at `Off` never fires, so no
    /// `Diagnostic` is produced.
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

    /// Single source of truth for "does this severity level promote a
    /// [`Diagnostic`]'s attached fix into an `AppliedFix` when the
    /// confidence threshold is met?"
    ///
    /// Promote-eligible: `Info`, `Warn`, `Error`, `Fix`.
    /// Non-promoting: `Off`, `Suggest`.
    ///
    /// `Off` is non-promoting trivially — no diagnostic is emitted at
    /// all. `Suggest` is the explicit advisory channel (see this enum's
    /// variant doc: "carries a candidate fix that will **not**
    /// auto-apply"). Every other severity carries a fix to the
    /// auto-apply pipeline when one is attached.
    ///
    /// Two engine sites consume this predicate and MUST stay aligned:
    /// the pass-2 promotion gate in `synthesize_fixes` (which skips
    /// non-eligible diagnostics) and the overlap-demotion guard in
    /// `apply_fr023_and_i18` (which demotes eligible diagnostics
    /// overlapping a pass-1 fix span to `Suggest`). If they drift, an
    /// overlapping pass-2 fix at a previously-untracked severity can
    /// be promoted on the same byte range as a pass-1 fix, violating
    /// the "pass-2 MUST NOT auto-apply on the same byte range"
    /// invariant. Using one method at both sites makes the drift
    /// structurally impossible.
    ///
    /// [`Diagnostic`]: ../marque_rules/struct.Diagnostic.html
    pub const fn is_promote_eligible(self) -> bool {
        match self {
            Self::Off | Self::Suggest => false,
            Self::Info | Self::Warn | Self::Error | Self::Fix => true,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Exhaustive map locks the [`Severity::is_promote_eligible`]
    /// contract: any change to a variant's classification (or any
    /// added variant) forces this table to be updated, which surfaces
    /// the consequence for the two engine sites cited in the helper's
    /// doc comment (`synthesize_fixes` promotion gate +
    /// `apply_fr023_and_i18` overlap-demotion guard). Without this
    /// lock, an additive enum variant (or a typo in the match arms)
    /// could silently re-open the auto-apply leak channel (#414).
    #[test]
    fn is_promote_eligible_exhaustive_classification() {
        let cases: &[(Severity, bool)] = &[
            (Severity::Off, false),
            (Severity::Suggest, false),
            (Severity::Info, true),
            (Severity::Warn, true),
            (Severity::Error, true),
            (Severity::Fix, true),
        ];
        for (sev, expected) in cases {
            assert_eq!(
                sev.is_promote_eligible(),
                *expected,
                "Severity::{sev:?} classification drifted; \
                 update the helper + this test together (engine \
                 sites depend on this predicate — see helper doc)"
            );
        }
    }
}
