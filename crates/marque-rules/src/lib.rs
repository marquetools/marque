//! marque-rules — trait definitions for the marque rule system.
//!
//! This crate defines the contract every rule crate must satisfy.
//! It has no rule implementations — those live in `marque-capco` and future crates.
//! The engine depends only on this crate, enabling rule crates to be swapped.
//!
//! # Type split: FixProposal vs AppliedFix
//!
//! `FixProposal` is pure data emitted by rules — deterministic, timestamp-free,
//! classifier-free. `AppliedFix` wraps a proposal with runtime context (timestamp,
//! classifier id, dry-run flag) and is constructed **only** by `Engine::fix`.
//! This makes "suggested vs applied" a type-system invariant.

use marque_ism::{IsmAttributes, Span};
use std::sync::Arc;
use std::time::SystemTime;

pub use marque_ism::{DocumentPosition, MarkingType, Zone};

// ---------------------------------------------------------------------------
// RuleId
// ---------------------------------------------------------------------------

/// Unique rule identifier string (e.g., "E001", "capco/banner-abbreviation").
///
/// The inner `&'static str` is private; construct via [`RuleId::new`] so that
/// construction is explicit at every call site.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(&'static str);

impl RuleId {
    /// Construct a rule identifier from a static string slice.
    #[inline]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// Return the rule identifier as a string slice.
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Rule severity level. Configurable per rule in `.marque.toml`.
///
/// # Ordering
///
/// The derived `Ord` is `Off < Warn < Error < Fix`. The ordering is
/// exposed for consumers that want to compare severities (e.g.,
/// "is this at least `Error`?") but the config loader does **not** use it
/// as a merge operator today.
///
/// # Merge semantics (current: last-write-wins)
///
/// `marque-config` merges layers in strict precedence order — env vars
/// override `.marque.local.toml` which overrides `.marque.toml`. Whatever
/// the highest-precedence layer says for a given rule wins, including
/// downgrades: a local override of `"off"` will suppress a project-config
/// `"error"`. This is intentional — individual classifiers sometimes need
/// to silence a rule while iterating, and the audit log still records the
/// configured severity for every applied fix.
///
/// If a future policy requires strictness-only merging (where a lower
/// layer cannot downgrade a higher layer's severity), change the loader
/// to `.max()` over `Severity::parse_config` values rather than `extend`.
/// The derived `Ord` above is already the correct operator for that case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Rule is disabled entirely. FR-008: severity=off is unrepresentable on emitted diagnostics
    /// — a rule at `Off` never fires, so no `Diagnostic` is produced.
    Off,
    /// Emit warning; do not block.
    Warn,
    /// Emit error; blocks `--check` exit code.
    Error,
    /// Apply fix automatically when `--fix` flag is present.
    Fix,
}

impl Severity {
    /// Parse a severity level from a config string. Returns `None` for
    /// unrecognized values; the config loader treats `None` as a hard error.
    pub fn parse_config(s: &str) -> Option<Self> {
        match s {
            "off" => Some(Self::Off),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            "fix" => Some(Self::Fix),
            _ => None,
        }
    }

    /// Canonical lowercase string form, suitable for JSON output.
    ///
    /// This is the inverse of [`Severity::parse_config`] and is the stable
    /// surface that JSON consumers should depend on — never `format!("{:?}")`
    /// (which exposes Debug formatting as an unintended API).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
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

// ---------------------------------------------------------------------------
// RuleContext
// ---------------------------------------------------------------------------

/// Document position context passed to rules alongside parsed markings.
///
/// Phase 3 made `zone` and `position` `Option`-typed: the scanner cannot
/// reliably determine header/footer/body or document position from raw
/// text alone, so a rule that reads either field must handle `None`.
/// They will become populated in a future scanner pass that consumes
/// document structural metadata (page count, line numbers, header/footer
/// detection on extracted documents).
///
/// `page_context` is populated by the engine for every non-portion
/// candidate (Banner, CAB) so banner-validation rules can compare the
/// observed banner against the composite expected from all preceding
/// portions. The engine resets it at scanner-emitted `MarkingType::PageBreak`
/// candidates (form-feed `\f` and `\n\n\n+` heuristics) so the context
/// reflects only the current page.
#[derive(Debug, Clone)]
pub struct RuleContext {
    pub marking_type: MarkingType,
    /// Document zone (header/footer/body/CAB) when known. `None` in Phase 3
    /// — the scanner cannot prove header vs footer from raw text.
    pub zone: Option<Zone>,
    /// Coarse document position when known. `None` in Phase 3.
    pub position: Option<DocumentPosition>,
    /// Accumulated portion data for the current page, reset at every
    /// scanner-emitted `MarkingType::PageBreak`.
    pub page_context: Option<std::sync::Arc<marque_ism::PageContext>>,
}

// ---------------------------------------------------------------------------
// FixSource
// ---------------------------------------------------------------------------

/// Provenance of a fix proposal — where the fix recommendation originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixSource {
    /// Hand-written Layer 2 CAPCO rule.
    BuiltinRule,
    /// User `[corrections]` entry (FR-009).
    CorrectionsMap,
    /// Deterministic deprecated-marking conversion (FR-004a).
    MigrationTable,
}

// ---------------------------------------------------------------------------
// FixProposal
// ---------------------------------------------------------------------------

/// A proposed fix for a diagnostic violation.
///
/// Pure data — deterministic, timestamp-free, classifier-free, safe to snapshot
/// in tests. A `FixProposal` is a *suggestion* until `Engine::fix` promotes it
/// to an `AppliedFix` when `confidence >= configuration.confidence_threshold`.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct FixProposal {
    /// The rule that generated this proposal.
    pub rule: RuleId,
    /// Provenance: built-in rule, corrections map, or migration table.
    pub source: FixSource,
    /// Byte range in original source to replace.
    pub span: Span,
    /// The bytes currently occupying `span`.
    pub original: Box<str>,
    /// Replacement text.
    pub replacement: Box<str>,
    /// Confidence in this fix (0.0–1.0). Fixes below the configured threshold
    /// are surfaced as suggestions rather than applied automatically.
    pub confidence: f32,
    /// Reference to the CAPCO rule or migration document justifying this fix.
    pub migration_ref: Option<&'static str>,
}

impl FixProposal {
    /// Create a new fix proposal with invariant checks.
    ///
    /// # Panics
    ///
    /// Panics if `confidence` is outside `[0.0, 1.0]` or is `NaN`. The check
    /// runs in release builds (not just debug) because `NaN` silently fails
    /// every threshold comparison and `INFINITY` silently bypasses every
    /// threshold — both are correctness-impacting bugs in release.
    pub fn new(
        rule: RuleId,
        source: FixSource,
        span: Span,
        original: impl Into<Box<str>>,
        replacement: impl Into<Box<str>>,
        confidence: f32,
        migration_ref: Option<&'static str>,
    ) -> Self {
        assert!(
            (0.0..=1.0).contains(&confidence) && !confidence.is_nan(),
            "FixProposal confidence must be in [0.0, 1.0] and not NaN, got {confidence}"
        );
        Self {
            rule,
            source,
            span,
            original: original.into(),
            replacement: replacement.into(),
            confidence,
            migration_ref,
        }
    }
}

// ---------------------------------------------------------------------------
// AppliedFix (= Audit Record)
// ---------------------------------------------------------------------------

/// A promoted `FixProposal` with runtime context.
///
/// Constructed **only** by `Engine::fix` at the moment a `FixProposal` meets
/// the confidence threshold. Never constructed by a rule or suggestion path.
///
/// Serves as the audit record: the NDJSON schema at `contracts/audit-record.json`
/// serializes this type.
///
/// `classifier_id` is an `Arc<str>` so promoting many fixes from a single
/// document only clones an atomic refcount, not the underlying string.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct AppliedFix {
    /// The original proposal that was applied.
    pub proposal: FixProposal,
    /// Timestamp of application (clock-injected).
    pub timestamp: SystemTime,
    /// Classifier identity from runtime config. `None` if not configured.
    pub classifier_id: Option<Arc<str>>,
    /// `true` if produced under `--dry-run` (FR-006).
    pub dry_run: bool,
    /// Caller-supplied input identifier (file path, "-" for stdin, `None` if N/A).
    pub input: Option<Arc<str>>,
}

impl AppliedFix {
    /// Promote a `FixProposal` to an `AppliedFix` with runtime context.
    ///
    /// # Engine-only contract
    ///
    /// This constructor exists in `marque-rules` for type co-location, but
    /// **must only be called from `marque-engine::Engine::fix`**. Rule crates
    /// and CLI code must never construct `AppliedFix` directly — they produce
    /// `FixProposal` values and let the engine promote them.
    ///
    /// This is enforced by convention and code review, not by the type system,
    /// because `AppliedFix` must be defined in `marque-rules` (which the engine
    /// depends on, not the reverse).
    #[doc(hidden)]
    pub fn __engine_promote(
        proposal: FixProposal,
        timestamp: SystemTime,
        classifier_id: Option<Arc<str>>,
        dry_run: bool,
        input: Option<Arc<str>>,
    ) -> Self {
        Self {
            proposal,
            timestamp,
            classifier_id,
            dry_run,
            input,
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// A single diagnostic emitted by a rule check.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule: RuleId,
    pub severity: Severity,
    /// Byte span in the original source buffer.
    pub span: Span,
    /// Human-readable description of the violation.
    pub message: Box<str>,
    /// CAPCO section citation, e.g., "CAPCO-2023-§3.1".
    pub citation: &'static str,
    /// Proposed fix, if the rule can generate one.
    pub fix: Option<FixProposal>,
}

impl Diagnostic {
    /// Construct a new diagnostic.
    pub fn new(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: impl Into<Box<str>>,
        citation: &'static str,
        fix: Option<FixProposal>,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            message: message.into(),
            citation,
            fix,
        }
    }
}

// ---------------------------------------------------------------------------
// Rule trait
// ---------------------------------------------------------------------------

/// The core trait every rule implementation must satisfy.
///
/// Rules are stateless. All configuration (severity overrides, corrections map)
/// is resolved by the engine before rule invocation and passed via context.
pub trait Rule: Send + Sync {
    fn id(&self) -> RuleId;
    fn name(&self) -> &'static str;
    /// Default severity — overridable per rule in `.marque.toml`.
    fn default_severity(&self) -> Severity;
    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic>;
}

/// A collection of rules provided by a rule crate.
/// Returned by the rule crate's entry point function.
pub trait RuleSet: Send + Sync {
    fn rules(&self) -> &[Box<dyn Rule>];
    fn schema_version(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_id_round_trip() {
        let r = RuleId::new("E001");
        assert_eq!(r.as_str(), "E001");
        assert_eq!(r.to_string(), "E001");
    }

    #[test]
    fn severity_parse_config_accepts_known_values() {
        assert_eq!(Severity::parse_config("off"), Some(Severity::Off));
        assert_eq!(Severity::parse_config("warn"), Some(Severity::Warn));
        assert_eq!(Severity::parse_config("error"), Some(Severity::Error));
        assert_eq!(Severity::parse_config("fix"), Some(Severity::Fix));
    }

    #[test]
    fn severity_parse_config_is_case_sensitive() {
        assert_eq!(Severity::parse_config("OFF"), None);
        assert_eq!(Severity::parse_config("Warn"), None);
    }

    #[test]
    fn severity_parse_config_rejects_unknown_strings() {
        assert_eq!(Severity::parse_config("err"), None);
        assert_eq!(Severity::parse_config("disable"), None);
        assert_eq!(Severity::parse_config(""), None);
    }

    #[test]
    fn severity_display_round_trips() {
        for s in [
            Severity::Off,
            Severity::Warn,
            Severity::Error,
            Severity::Fix,
        ] {
            assert_eq!(Severity::parse_config(s.as_str()), Some(s));
            assert_eq!(s.to_string(), s.as_str());
        }
    }

    #[test]
    fn severity_ord_off_is_lowest() {
        // Off < Warn < Error < Fix — see the doc comment on Severity for the
        // intentional design rationale.
        assert!(Severity::Off < Severity::Warn);
        assert!(Severity::Warn < Severity::Error);
        assert!(Severity::Error < Severity::Fix);
    }

    #[test]
    fn fix_proposal_new_accepts_boundary_confidence() {
        let _zero = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            0.0,
            None,
        );
        let _one = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            1.0,
            None,
        );
    }

    #[test]
    #[should_panic(expected = "FixProposal confidence")]
    fn fix_proposal_new_panics_on_negative_confidence() {
        let _ = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            -0.1,
            None,
        );
    }

    #[test]
    #[should_panic(expected = "FixProposal confidence")]
    fn fix_proposal_new_panics_on_above_one_confidence() {
        let _ = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            1.5,
            None,
        );
    }

    #[test]
    #[should_panic(expected = "FixProposal confidence")]
    fn fix_proposal_new_panics_on_nan_confidence() {
        let _ = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            f32::NAN,
            None,
        );
    }
}
