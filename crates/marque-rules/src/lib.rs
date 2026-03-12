//! marque-rules — trait definitions for the marque rule system.
//!
//! This crate defines the contract every rule crate must satisfy.
//! It has no rule implementations — those live in `marque-capco` and future crates.
//! The engine depends only on this crate, enabling rule crates to be swapped.

use std::time::SystemTime;
use marque_core::{IsmAttributes, Span};

pub use marque_core::span::{DocumentPosition, MarkingType, Zone};

/// Document position context passed to rules alongside parsed markings.
#[derive(Debug, Clone)]
pub struct RuleContext {
    pub marking_type: MarkingType,
    pub zone: Zone,
    pub document_position: DocumentPosition,
    pub paragraph_index: usize,
}

/// Unique rule identifier string (e.g., "E001", "capco/banner-abbreviation").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleId(pub &'static str);

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Rule severity level. Configurable per rule in `.marque.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Apply fix automatically when `--fix` flag is present.
    Fix,
    /// Emit warning; do not block.
    Warn,
    /// Emit error; blocks `--check` exit code.
    Error,
}

/// A proposed fix for a diagnostic violation.
#[derive(Debug, Clone)]
pub struct Fix {
    /// Byte range in original source to replace.
    pub span: Span,
    /// Replacement text.
    pub replacement: String,
    /// Confidence in this fix (0.0–1.0). Fixes below a configured threshold
    /// are surfaced as suggestions rather than applied automatically.
    pub confidence: f32,
    /// Audit record — always generated, even for trivial fixes.
    pub audit: AuditRecord,
    /// Reference to the CAPCO rule or migration document justifying this fix.
    pub migration_ref: Option<&'static str>,
}

/// Immutable audit record generated for every applied or proposed fix.
/// Written to the audit log regardless of whether the fix was applied.
#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub rule: RuleId,
    pub original: String,
    pub replacement: String,
    pub confidence: f32,
    pub timestamp: SystemTime,
    /// Classifier identifier from user config, if present.
    pub classifier_id: Option<String>,
}

/// A single diagnostic emitted by a rule check.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule: RuleId,
    pub severity: Severity,
    /// Byte span in the original source buffer.
    pub span: Span,
    /// Human-readable description of the violation.
    pub message: String,
    /// Proposed fix, if the rule can generate one.
    pub fix: Option<Fix>,
}

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
