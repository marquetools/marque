// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

// VocabularyProvider trait sketch — design exploration, not compilable code.
// This file captures the interface shape for critique before implementation.
//
// Design goal: express the structure of any classification marking system
// (CAPCO, NATO, UK, French, Australian, JOINT, FGI) as data that the engine
// consumes, with optional behavioral overrides for edge cases.

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Unique identifier for a token within a vocabulary.
/// Cheaply copyable, used as lookup keys throughout the engine.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenId(u32);

/// Unique identifier for a token category.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CategoryId(u32);

/// A token in the vocabulary — one recognized symbol.
pub struct TokenEntry {
    pub id: TokenId,
    pub canonical: Box<str>,       // "SECRET", "NF", "SI-G"
    pub category: CategoryId,
    pub kind: TokenKind,

    /// Alias forms that should resolve to this token.
    /// Includes abbreviations, expansions, deprecated forms.
    /// Each alias carries a relationship type so the engine knows
    /// whether resolving it is a correction (deprecated → current),
    /// a normalization (expansion → abbreviation in portions), or
    /// just recognition (alternate spelling).
    pub aliases: Box<[TokenAlias]>,

    /// Edit distance threshold for fuzzy matching. Shorter tokens
    /// need tighter thresholds to avoid false positives.
    /// None = use the engine's default (typically len/3, min 1).
    pub max_edit_distance: Option<u8>,

    /// Base rate: occurrences per million words in general English.
    /// From corpus analysis. 0.0 = marking-exclusive (NOFORN, FVEY).
    /// Higher values = more ambiguous (SECRET=1.4, USA=108).
    pub base_rate_per_million: f32,
}

/// How a token behaves in the vocabulary.
pub enum TokenKind {
    /// Fixed, closed-set token. The vocabulary defines all valid values.
    /// Examples: classification levels, dissem controls, SCI control systems.
    Fixed,

    /// Open-ended: the vocabulary defines the *prefix* or *category shape*,
    /// but actual values are unconstrained within format rules.
    /// Examples: SAR program names, SCI compartments/sub-compartments,
    /// country trigraphs (from an external registry).
    Open {
        /// Pattern constraint for recognition.
        /// None = any string in this position is accepted.
        pattern: Option<OpenPattern>,
    },
}

/// Format constraint for open-ended tokens.
pub enum OpenPattern {
    /// Exact character count (e.g., trigraphs = 3, sub-compartments = 4).
    ExactLength(u8),
    /// Character count range.
    LengthRange { min: u8, max: u8 },
    /// Must follow a specific prefix token (e.g., compartments must follow
    /// their SCI control system: "SI-G" where G follows SI).
    RequiresPrefix(TokenId),
    /// Regex-like pattern (kept simple — no full regex engine).
    AlphaNumeric { min_len: u8, max_len: u8 },
}

/// A relationship between an alias form and the canonical token.
pub struct TokenAlias {
    pub form: Box<str>,            // "SECRET", "NOFORN", "FOUO"
    pub relationship: AliasRelation,
}

pub enum AliasRelation {
    /// Abbreviation ↔ expansion. Direction depends on context:
    /// portions use abbreviated form, banners use expanded.
    Abbreviation,
    /// Deprecated form. Resolving this is a correction with a migration ref.
    Deprecated {
        replacement_note: Option<Box<str>>,  // "CAPCO-2022-§2.1"
    },
    /// Alternate recognized spelling (not a correction, just recognition).
    AlternateForm,
}

// ---------------------------------------------------------------------------
// Category schema
// ---------------------------------------------------------------------------

/// A category of tokens within the marking grammar.
pub struct CategoryEntry {
    pub id: CategoryId,
    pub name: Box<str>,            // "classification", "sci", "dissem", "trigraph"

    /// Where this category appears in the marking's left-to-right order.
    /// Categories are sorted by rank when composing a canonical marking.
    /// Multiple categories can share a rank (composed in sub-order).
    pub ordering_rank: u16,

    /// How many tokens from this category can appear in one marking.
    pub cardinality: Cardinality,

    /// What delimiter separates this category from the next.
    /// None = inherits the vocabulary's default delimiter.
    pub delimiter_after: Option<Box<str>>,

    /// What delimiter separates multiple tokens *within* this category.
    /// Examples: space between SCI compartments, comma+space between trigraphs.
    pub internal_delimiter: Option<Box<str>>,

    /// Nesting: can tokens in this category have sub-tokens?
    /// If Some, the sub-tokens belong to the specified category.
    /// Example: SCI control → compartments (3-char) → sub-compartments (4-char)
    pub nesting: Option<NestingRule>,
}

pub enum Cardinality {
    /// Exactly one (classification level).
    One,
    /// Zero or one (optional single value).
    Optional,
    /// Zero or more (dissem controls, trigraphs).
    Many,
}

pub struct NestingRule {
    /// The category that sub-tokens belong to.
    pub child_category: CategoryId,
    /// Delimiter between parent and child tokens.
    pub delimiter: Box<str>,       // "-" for SCI: SI-G, "/" for compartment separation
}

// ---------------------------------------------------------------------------
// Structural templates
// ---------------------------------------------------------------------------

/// A structural template defines what a valid marking looks like
/// in a specific position (portion, banner, CAB, etc.).
pub struct MarkingTemplate {
    pub name: Box<str>,            // "us_portion", "us_banner", "fgi_portion", "nato_portion"

    /// Default delimiter between categories (e.g., "//" for CAPCO).
    pub category_delimiter: Box<str>,

    /// Which categories are required, optional, or forbidden.
    pub category_rules: Box<[CategoryRule]>,

    /// Wrapping: does this template use parens, brackets, or nothing?
    pub wrapping: Wrapping,

    /// Does this template use abbreviated or expanded token forms?
    pub token_form: TokenForm,
}

pub struct CategoryRule {
    pub category: CategoryId,
    pub presence: Presence,
}

pub enum Presence {
    Required,
    Optional,
    /// This category is forbidden in this template. Example: SAR might
    /// not be allowed in certain foreign marking systems.
    Forbidden,
}

pub enum Wrapping {
    Parenthesized,     // (S//NF)
    None,              // SECRET//NOFORN  (banners)
    Bracketed,         // [S//NF]  (some NATO formats?)
    Custom(Box<str>, Box<str>),  // arbitrary open/close
}

pub enum TokenForm {
    Abbreviated,       // Portions: S, NF, TS
    Expanded,          // Banners: SECRET, NOFORN, TOP SECRET
    AsWritten,         // Don't normalize — accept either
}

// ---------------------------------------------------------------------------
// Conflict and implication rules
// ---------------------------------------------------------------------------

/// Semantic relationships between tokens that affect validation.
pub enum TokenConstraint {
    /// These tokens cannot appear together in one marking.
    /// Example: NOFORN and REL TO are contradictory.
    Conflicts(TokenId, TokenId),

    /// If token A appears, token B is implied and can be omitted.
    Implies(TokenId, TokenId),

    /// Token A requires token B to also be present.
    /// Example: a compartment requires its parent SCI control.
    Requires(TokenId, TokenId),

    /// Token A supersedes token B (B is redundant if A is present).
    /// Used for banner roll-up: NOFORN supersedes REL TO USA.
    Supersedes(TokenId, TokenId),
}

// ---------------------------------------------------------------------------
// The trait
// ---------------------------------------------------------------------------

/// Provides a complete vocabulary definition for one classification marking
/// system. The engine consumes this to perform token resolution, structural
/// matching, and confidence scoring without any domain-specific knowledge.
///
/// Implementors (e.g., `marque-capco`) provide data describing their marking
/// system. The engine handles the mechanics of recognition, fuzzy matching,
/// and fix generation.
///
/// # Design: data-heavy with behavioral escape hatches
///
/// The default trait methods cover ~90% of cases through the data returned
/// by the required methods. The optional `override_*` methods let an
/// implementor customize behavior for edge cases without the engine needing
/// to anticipate every system's quirks.
pub trait VocabularyProvider: Send + Sync {
    // --- Required: the data ---

    /// Human-readable name for this vocabulary (e.g., "CAPCO-ISM-v2022-DEC").
    fn name(&self) -> &str;

    /// Schema/version identifier used for cache invalidation and audit logs.
    fn schema_version(&self) -> &str;

    /// All tokens in the vocabulary, including their aliases and metadata.
    fn tokens(&self) -> &[TokenEntry];

    /// All token categories and their composition rules.
    fn categories(&self) -> &[CategoryEntry];

    /// Structural templates for each marking position.
    fn templates(&self) -> &[MarkingTemplate];

    /// Semantic constraints between tokens.
    fn constraints(&self) -> &[TokenConstraint];

    // --- Optional: behavioral overrides ---

    /// Override token resolution for ambiguous cases.
    ///
    /// Called when the engine's default fuzzy matcher finds a candidate but
    /// the confidence is below the auto-resolve threshold. The provider can
    /// use domain knowledge to either boost confidence, reject the match,
    /// or return a verification request.
    ///
    /// Default: returns None (use engine's default resolution).
    fn override_token_resolution(
        &self,
        _candidate: &str,
        _context: &ResolutionContext,
    ) -> Option<ResolutionOverride> {
        None
    }

    /// Override structural template selection.
    ///
    /// Called when the engine can't unambiguously determine which template
    /// applies (e.g., a marking that could be either a US portion or an
    /// FGI portion). The provider can use domain knowledge to disambiguate.
    ///
    /// Default: returns None (use engine's best-guess template).
    fn override_template_selection(
        &self,
        _tokens: &[ResolvedToken],
        _context: &ResolutionContext,
    ) -> Option<&MarkingTemplate> {
        None
    }

    /// Additional validation after the engine has resolved all tokens and
    /// selected a template. Lets the provider enforce domain rules that
    /// don't fit the constraint model (e.g., CAPCO's SCI ordering within
    /// the SCI category follows a non-alphabetical precedence).
    ///
    /// Default: returns empty (no additional diagnostics).
    fn post_validation(
        &self,
        _resolved: &ResolvedMarking,
    ) -> Vec<ValidationDiagnostic> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Context and result types used by the override methods
// ---------------------------------------------------------------------------

/// Context available during token resolution.
pub struct ResolutionContext {
    /// How confident we are that this is a marking at all (Layer 1 output).
    pub region_confidence: f32,
    /// What other tokens have already been resolved in this marking.
    pub resolved_neighbors: Vec<ResolvedToken>,
    /// The structural position hint, if known.
    pub position_hint: Option<PositionHint>,
    /// Whether the caller declared the input as a known marking.
    pub caller_declared_marking: bool,
}

pub enum PositionHint {
    Portion,
    Banner,
    Cab,
    Unknown,
}

pub struct ResolvedToken {
    pub token_id: TokenId,
    pub confidence: f32,
    pub original_text: Box<str>,
    pub canonical_form: Box<str>,
    pub alias_used: Option<AliasRelation>,
}

pub struct ResolvedMarking {
    pub template: Box<str>,        // which template was selected
    pub tokens: Vec<ResolvedToken>,
    pub overall_confidence: f32,
}

pub enum ResolutionOverride {
    /// Accept the candidate as this token with the given confidence.
    Accept { token_id: TokenId, confidence: f32 },
    /// Reject the candidate — not a token in this vocabulary.
    Reject,
    /// The candidate is ambiguous and requires human verification.
    /// The engine will surface this as a question, not a fix.
    NeedsVerification {
        possible_tokens: Vec<TokenId>,
        question: Box<str>,
    },
}

pub struct ValidationDiagnostic {
    pub message: Box<str>,
    pub severity: DiagnosticSeverity,
}

pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}
