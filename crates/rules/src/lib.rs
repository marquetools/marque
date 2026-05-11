// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-rules — trait definitions for the marque rule system.
//!
//! This crate defines the contract every rule crate must satisfy.
//! It has no rule implementations — those live in `marque-capco` and future crates.
//! The engine depends only on this crate, enabling rule crates to be swapped.
//!
//! # Module layout
//!
//! - [`confidence`] — `Confidence` (recognition × rule axes), `FeatureId`,
//!   `FeatureContribution`. Phase D audit-provenance payload attached to every
//!   `FixProposal` / `FixIntent`.
//! - [`message`] — `Message`, `MessageTemplate` (closed enum), `MessageArgs`
//!   (closed-set struct). The G13 type-system closure of the diagnostic-message
//!   leak channel: only `Message::new(template, args)` constructs a `Message`,
//!   and `MessageArgs` cannot carry input bytes (no `String` / `&str` / `Vec<u8>`
//!   fields). PR 3c.1 lands the surface; PR 3c.2 reshapes `Diagnostic.message`
//!   to consume it. (Source plan §8.3.)
//! - [`fix_intent`] — `FixIntent<S>`, `ReplacementIntent<S>`, `FactRef<S>`,
//!   `RecanonScope`. The rule-emission API for the bag-of-tokens vocabulary
//!   from `architecture.md` §"What fixes are": fact-set deltas (`FactAdd` /
//!   `FactRemove`) and renderer recanonicalization (`Recanonicalize`). The
//!   engine promotes a `FixIntent<S>` to an `AppliedFix<S>` via
//!   `__engine_promote`; the synthesized legacy projection is materialized
//!   inside `AppliedFixProposal::New` (Path C) so audit-emit code that reads
//!   `applied_fix.proposal.span` etc. via `Deref<Target = FixProposal>` keeps
//!   working unchanged.
//!
//! # Type split: FixProposal vs FixIntent vs AppliedFix
//!
//! `FixProposal` (legacy) and `FixIntent<S>` (new) are both pure data
//! emitted by rules — deterministic, timestamp-free, classifier-free.
//! `AppliedFix<S>` wraps either of them (via the `AppliedFixProposal<S>`
//! enum) with runtime context (timestamp, classifier id, dry-run flag)
//! and is constructed **only** by `Engine::fix_inner`. This makes
//! "suggested vs applied" a type-system invariant.
//!
//! `FixProposal` and `FixIntent<S>` coexist on `marque-rules` through
//! the PR 3c.B Commit 2–9 transition window. Commit 10 retires
//! `FixProposal` (and the `AppliedFixProposal::Legacy` arm) atomically
//! with the `MARQUE_AUDIT_SCHEMA` flip from `"marque-mvp-2"` to
//! `"marque-mvp-3"`.
//!
//! # G13 (audit content ignorance) status
//!
//! `FixIntent<S>` and `AppliedFixProposal::New` carry only structural
//! references (`FactRef`, category IDs, `Scope` / `RecanonScope` tags)
//! — no document bytes. The synthesized `FixProposal` cached inside
//! `AppliedFixProposal::New` for `Deref` projection sets `original = ""`
//! to preserve G13 closure on the new path.
//!
//! `Diagnostic.message: Box<str>` (legacy) is NOT type-system-closed
//! against the diagnostic-message leak channel today; only [`Message`]
//! (closed template + closed args) provides that guarantee. The narrow
//! to `Diagnostic.message: Message` is deferred to a follow-up commit
//! tracked alongside the wider [`Diagnostic<S>`] reshape — when that
//! lands the G13 closure becomes a type-level invariant for both paths.

pub mod confidence;
pub mod fix_intent;
pub mod message;

use marque_ism::{CanonicalAttrs, Span};
use marque_scheme::MarkingScheme;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

pub use confidence::{Confidence, FeatureContribution, FeatureId};
pub use fix_intent::{FactRef, FixIntent, RecanonScope, ReplacementIntent};
pub use marque_ism::{DocumentPosition, MarkingType, Zone};
pub use message::{Blake3Hash, Message, MessageArgs, MessageTemplate};

// ---------------------------------------------------------------------------
// RuleId
// ---------------------------------------------------------------------------

/// Unique rule identifier string (e.g., "E001", "capco/portion-mark-in-banner").
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
/// The derived `Ord` is `Off < Suggest < Info < Warn < Error < Fix`.
/// The ordering is exposed for consumers that want to compare
/// severities (e.g., "is this at least `Error`?") but the config
/// loader does **not** use it as a merge operator today. `Suggest`
/// sits between `Off` and `Info` because it is the lightest
/// firing-but-non-actionable channel — quieter than `Info` (which
/// has no candidate replacement attached) and louder than `Off`
/// (which is non-firing entirely).
///
/// # Exit-code semantics
///
/// `marque check` maps severities to exit codes as follows:
///
/// | Severity counts present       | Exit code              |
/// |-------------------------------|------------------------|
/// | `Error` or `Fix`              | `1` (`EX_DIAG_ERROR`)  |
/// | `Warn` only                   | `2` (`EX_DIAG_WARN`)   |
/// | `Info` / `Suggest` only, none | `0` (`EX_OK`)          |
///
/// `Info` and `Suggest` are the only severities whose diagnostics are
/// emitted *and* keep the exit code at zero. `Warn` still fails CI
/// via `EX_DIAG_WARN`. The tonal distinction is advisory: `Warn`
/// means "this might be wrong"; `Info` means "FYI, probably
/// intentional but worth surfacing"; `Suggest` means "I have a
/// candidate replacement but I'm not confident enough to auto-apply
/// it — eyes on it." Rules like `W034 sci-custom-control-info`
/// (which reports unpublished SCI control systems — legitimate per
/// CAPCO but rare) are natural `Info` candidates; rules like `S004
/// rel-to-trigraph-suggest` (which proposes a higher-prior trigraph
/// alternative for an ambiguous REL TO entry) emit at `Suggest`.
///
/// # `Suggest` channel semantics
///
/// `Suggest` is the firing-but-non-applying channel: a diagnostic
/// emitted at `Suggest` carries a candidate `FixProposal` that the
/// engine will **never** auto-apply, regardless of `confidence`. The
/// fix is informational — it tells the user what the rule would
/// suggest if confidence were higher. Two paths produce
/// `Suggest`-severity diagnostics:
///
/// 1. **Explicit emission**: a rule constructs the diagnostic with
///    `Severity::Suggest` directly. `S004 rel-to-trigraph-suggest`
///    is the first such rule.
/// 2. **Engine rewrite**: any diagnostic whose attached `FixProposal`
///    has `confidence.combined() < confidence_threshold` is rewritten
///    to `Severity::Suggest` by the engine in `lint`. This subsumes
///    the prior silent-drop behavior at threshold-gate time so
///    below-threshold proposals stay observable.
///
/// In both cases, `Engine::fix` filters out `Suggest` diagnostics
/// from auto-apply by construction. `Suggest` diagnostics with
/// `fix: None` are also valid (informational suggestion with no
/// candidate replacement — used by future rules like #206's
/// REL TO opaque-uncertain reduction, where the rule has signal
/// to surface but no specific replacement to propose); the
/// renderer handles the missing-fix case cleanly.
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
    /// Advisory channel — diagnostic carries a candidate fix that
    /// will **not** auto-apply.
    ///
    /// Distinct from `Info` (FYI, no actionable replacement) and
    /// from `Off` (non-firing). The fix-bearing diagnostic remains
    /// visible in lint output but the engine excludes it from
    /// auto-apply regardless of `confidence`. This is the
    /// suggest-don't-fix channel: rules with low-confidence
    /// candidate corrections (e.g., `S004 rel-to-trigraph-suggest`)
    /// can surface "did you mean?" hints without committing to the
    /// rewrite.
    ///
    /// `Suggest` keeps the CLI exit code at `0` (same as `Info`),
    /// so it is CI-silent.
    Suggest,
    /// Emit informational diagnostic; does not block `check`-mode exit
    /// code. Intended for "audit-visible but probably intentional"
    /// signals — cases where the marking may be correct but the user
    /// may want to verify (e.g., unpublished SCI control systems).
    Info,
    /// Emit warning; non-error, but still non-zero in `check` mode
    /// (produces `EX_DIAG_WARN` = 2). Different from `Info` in tone
    /// *and* exit-code impact: Warn is "this might be wrong" and
    /// CI-visible; Info is "FYI, probably intentional but worth
    /// surfacing" and CI-silent (exit 0).
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
    /// This is the inverse of [`Severity::parse_config`] and is the stable
    /// surface that JSON consumers should depend on — never `format!("{:?}")`
    /// (which exposes Debug formatting as an unintended API).
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
    /// Organization-specific corrections map from config `[corrections]`.
    /// `None` when no corrections are configured.
    pub corrections: Option<Arc<HashMap<String, String>>>,
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
    /// Probabilistic decoder produced this fix from a recognition
    /// candidate's posterior (Phase D, see
    /// `docs/plans/2026-04-16-probabilistic-recognition.md`). Paired
    /// with a non-trivial `features` list in
    /// [`FixProposal::confidence`] so auditors can reconstruct the
    /// scoring path.
    DecoderPosterior,
    /// Decoder produced this fix via a position-aware short-token
    /// classification heuristic — a keyboard-proximity table applied
    /// to the leading classification slot of a portion or banner
    /// marking when the token is too short for vocab-based fuzzy
    /// matching (e.g., `(YS//NF) → (TS//NF)`, `(W//NF) → (S//NF)`).
    /// See issue #133 PR 2.
    ///
    /// The heuristic is inherently less certain than a fuzzy-vocab
    /// match because the inference is "this token is keyboard-
    /// adjacent to a known classification" rather than "this token
    /// is edit-distance ≤ 2 from a known canonical token in a
    /// closed vocabulary." The engine therefore (a) emits the
    /// diagnostic at [`Severity::Warn`] (the fix-and-warn pattern —
    /// always visible, non-zero exit code in `--check`), and
    /// (b) caps [`Confidence::rule`] at `0.80` so `combined ≤ 0.80`
    /// stays below the default `confidence_threshold` of `0.95`.
    /// The fix only auto-applies when the user has explicitly
    /// lowered the threshold to opt into the heuristic's bar.
    DecoderClassificationHeuristic,
}

/// Canonical citation string for diagnostics whose authority is the user's
/// `[corrections]` config entry (C001 and the engine's pre-scanner text-scan
/// path). C001 is not a CAPCO rule — no CAPCO passage governs user-defined
/// typo replacements — so the citation is a config pointer rather than a
/// §/page/line reference. Holding the string in one place prevents silent
/// drift between the rule-pipeline emission site in `marque-capco` and the
/// pre-scanner emission site in `marque-engine`; both paths produce the
/// same audit-record shape.
pub const CORRECTIONS_MAP_CITATION: &str = "CONFIG:[corrections]";

// ---------------------------------------------------------------------------
// FixProposal
// ---------------------------------------------------------------------------

/// A proposed fix for a diagnostic violation.
///
/// Pure data — deterministic, timestamp-free, classifier-free, safe to snapshot
/// in tests. A `FixProposal` is a *suggestion* until `Engine::fix` promotes it
/// to an `AppliedFix` when `confidence.combined() >= configuration.confidence_threshold`.
///
/// # Phase D: Multi-axis confidence
///
/// `confidence` is a [`Confidence`] record rather than a scalar. Strict-path
/// rules construct it via [`Confidence::strict`]; the Phase D decoder
/// constructs a full record with `recognition`, `runner_up_ratio`, and
/// feature contributions. The engine threshold gate uses
/// [`Confidence::combined`] so a 0.95-recognition × 0.9-rule fix that
/// previously would have been scalar-0.855 still gates the same way.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct FixProposal {
    /// The rule that generated this proposal.
    pub rule: RuleId,
    /// Provenance: built-in rule, corrections map, migration table, or
    /// decoder posterior.
    pub source: FixSource,
    /// Byte range in original source to replace.
    pub span: Span,
    /// The bytes currently occupying `span`.
    pub original: Box<str>,
    /// Replacement text.
    pub replacement: Box<str>,
    /// Multi-axis confidence for this fix.
    pub confidence: Confidence,
    /// Reference to the CAPCO rule or migration document justifying this fix.
    pub migration_ref: Option<&'static str>,
}

impl FixProposal {
    /// Create a new fix proposal with invariant checks.
    ///
    /// # Panics
    ///
    /// Panics if `confidence` fails [`Confidence::validate`] — i.e.,
    /// any individual axis is out of range or `NaN` / non-finite. The
    /// per-axis check is the load-bearing one: `combined() =
    /// recognition × rule` can land in `[0.0, 1.0]` for individually-
    /// invalid axes (e.g., `recognition = 2.0`, `rule = 0.4` ⇒
    /// `combined = 0.8`), so validating only the product would let an
    /// invalid record through. The check runs in release builds (not
    /// just debug) because `NaN` silently fails every threshold
    /// comparison and `INFINITY` silently bypasses every threshold —
    /// both are correctness-impacting bugs in release.
    pub fn new(
        rule: RuleId,
        source: FixSource,
        span: Span,
        original: impl Into<Box<str>>,
        replacement: impl Into<Box<str>>,
        confidence: Confidence,
        migration_ref: Option<&'static str>,
    ) -> Self {
        if let Err(msg) = confidence.validate() {
            panic!("FixProposal invalid confidence: {msg}");
        }
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

/// Engine-promoted proposal payload — the body of an [`AppliedFix`].
///
/// Carries either the legacy [`FixProposal`] (for non-migrated rules
/// during the PR 3c.B Commit 2–9 transition) or the new
/// [`FixIntent<S>`] (for migrated rules). The engine's promotion
/// path selects the variant; the audit-emit path converts a `New`
/// variant to legacy-shape JSON via the engine's
/// `fix_intent_to_legacy_proposal` helper. Commit 10 retires the
/// `Legacy` variant atomically with the schema bump.
///
/// `AppliedFixProposal::Legacy(_)` implements `Deref<Target =
/// FixProposal>` so existing audit-emit code that reads
/// `applied_fix.proposal.span`, `.original`, `.replacement`,
/// `.source`, etc., continues to compile and behave identically.
/// For `New { intent, synthesized }` variants, `Deref` returns
/// `&synthesized` — a legacy-shape `FixProposal` constructed by the
/// engine via `fix_intent_to_legacy_proposal` at promotion time and
/// cached inside the variant. Caching the synthesis at construction
/// time (rather than projecting on every `Deref` call) eliminates
/// the foot-gun where a `Deref` path could panic at audit-emit time
/// when the first `FixIntent`-emitting rule lands; the panic now
/// surfaces at `__engine_promote` if `fix_intent_to_legacy_proposal`
/// is still `unimplemented!()` for the `ReplacementIntent` variant
/// the rule emitted.
#[derive(Debug)]
pub enum AppliedFixProposal<S: MarkingScheme> {
    /// Legacy `FixProposal` — emitted by non-migrated rules during
    /// the Commit 2–9 transition window. Direct field access via
    /// `Deref<Target = FixProposal>`.
    Legacy(FixProposal),
    /// New `FixIntent<S>` — emitted by migrated rules from Commit 3
    /// onward. The engine constructs `synthesized` at promotion time
    /// via `fix_intent_to_legacy_proposal` and caches it here so
    /// `Deref<Target = FixProposal>` returns `&synthesized` for
    /// audit-emit consumers. Commit 10 retires this variant (and the
    /// synthesized cache) atomically with the audit-schema flip when
    /// audit emitters move to read `FixIntent` variants directly.
    ///
    /// `intent` is `Box`ed so the `New` variant's stack size stays
    /// comparable to `Legacy`'s. Without the box, `FixIntent<S>` (with
    /// its `SmallVec`, `Confidence`, and `Message` inline storage) plus
    /// `FixProposal` together push the enum past clippy's
    /// `large_enum_variant` threshold and inflate every `Legacy`
    /// allocation by the same delta.
    New {
        /// The structural fact-set delta the rule emitted.
        intent: Box<FixIntent<S>>,
        /// Engine-synthesized legacy projection. Carries `original = ""`
        /// per Constitution V Principle V (G13 closure on the new
        /// emission path); other fields populated by the engine.
        synthesized: FixProposal,
    },
}

// Manual Clone for AppliedFixProposal<S> — see the parallel Clone
// impl on `AppliedFix<S>` for the rationale. `S` itself is never
// cloned; only `S::OpenVocabRef` (which is `Clone`-bounded by the
// `MarkingScheme` trait) flows through.
impl<S: MarkingScheme> Clone for AppliedFixProposal<S> {
    fn clone(&self) -> Self {
        match self {
            AppliedFixProposal::Legacy(p) => AppliedFixProposal::Legacy(p.clone()),
            AppliedFixProposal::New {
                intent,
                synthesized,
            } => AppliedFixProposal::New {
                intent: intent.clone(),
                synthesized: synthesized.clone(),
            },
        }
    }
}

impl<S: MarkingScheme> std::ops::Deref for AppliedFixProposal<S> {
    type Target = FixProposal;

    /// Project to the legacy `FixProposal`-shape view.
    ///
    /// For [`AppliedFixProposal::Legacy`], this is a direct field
    /// borrow. For [`AppliedFixProposal::New`], this returns
    /// `&synthesized` — the engine-cached legacy projection
    /// constructed at `__engine_promote` time via
    /// `marque_engine::fix_intent_to_legacy_proposal`. Both arms
    /// return a valid `&FixProposal`; no panic surface.
    fn deref(&self) -> &FixProposal {
        match self {
            AppliedFixProposal::Legacy(p) => p,
            AppliedFixProposal::New { synthesized, .. } => synthesized,
        }
    }
}

/// A promoted `FixProposal` or `FixIntent<S>` with runtime context.
///
/// Constructed **only** by `Engine::fix_inner` (or its
/// `apply_text_corrections` partner) at the moment a fix meets the
/// confidence threshold. Never constructed by a rule or suggestion
/// path. Serves as the audit record: the NDJSON schemas at
/// `contracts/audit-record*.json` serialize this type.
///
/// `classifier_id` is an `Arc<str>` so promoting many fixes from a
/// single document only clones an atomic refcount, not the
/// underlying string.
///
/// # Generic over the marking scheme
///
/// `AppliedFix<S>` is generic so the `New(FixIntent<S>)` variant
/// of [`AppliedFixProposal`] preserves the scheme-typed payload.
/// `marque-engine` and downstream surfaces (server, WASM, CLI)
/// instantiate `AppliedFix<CapcoScheme>` at the boundary.
///
/// # v2 audit fields (`confidence`, `source`)
///
/// Phase D promotes the fix's [`Confidence`] and [`FixSource`] to
/// **top-level** fields on `AppliedFix` so the v2 audit emitter
/// doesn't need to descend into `.proposal` to find them. They are
/// a snapshot at promotion time — the engine may (in future
/// phases) adjust them for region context before promotion, so they
/// can diverge from the original `proposal.confidence` /
/// `proposal.source`.
#[non_exhaustive]
#[derive(Debug)]
pub struct AppliedFix<S: MarkingScheme> {
    /// The original proposal that was applied. Carries either a
    /// legacy [`FixProposal`] (commit 2–9 transition) or a new
    /// [`FixIntent<S>`] (commit 3+ migrated rules). `Deref<Target =
    /// FixProposal>` projects to the legacy view; audit-emit code
    /// that reads `applied_fix.proposal.span` etc. continues to
    /// compile unchanged.
    pub proposal: AppliedFixProposal<S>,
    /// Snapshot of the fix's confidence at promotion time (v2 audit).
    pub confidence: Confidence,
    /// Snapshot of the fix's provenance at promotion time (v2 audit).
    pub source: FixSource,
    /// Timestamp of application (clock-injected).
    pub timestamp: SystemTime,
    /// Classifier identity from runtime config. `None` if not configured.
    pub classifier_id: Option<Arc<str>>,
    /// `true` if produced under `--dry-run` (FR-006).
    pub dry_run: bool,
    /// Caller-supplied input identifier (file path, "-" for stdin, `None` if N/A).
    pub input: Option<Arc<str>>,
}

// Manual Clone for AppliedFix<S> that does NOT require S: Clone.
// The scheme `S` itself is never cloned; what matters is that its
// associated type `S::OpenVocabRef` is `Clone` (which is a trait
// bound on `MarkingScheme::OpenVocabRef`). The derive macro would
// over-constrain to `S: Clone`, breaking call sites where
// `S = CapcoScheme` (stateful, not derived `Clone`).
impl<S: MarkingScheme> Clone for AppliedFix<S> {
    fn clone(&self) -> Self {
        Self {
            proposal: self.proposal.clone(),
            confidence: self.confidence.clone(),
            source: self.source,
            timestamp: self.timestamp,
            classifier_id: self.classifier_id.clone(),
            dry_run: self.dry_run,
            input: self.input.clone(),
        }
    }
}

impl<S: MarkingScheme> AppliedFix<S> {
    /// Promote a `FixProposal` to an `AppliedFix` with runtime context.
    ///
    /// # Reserved name (FR-040 lint contract)
    ///
    /// The function name `__engine_promote` is **reserved by the
    /// marque project**. The `tools/promote-callsite-lint/` CI lint
    /// (FR-040) flags every call expression whose path's last
    /// segment is `__engine_promote`, regardless of the leading
    /// qualifier — qualified, fully-qualified, `Self::`, aliased
    /// (`use AppliedFix as AF; AF::__engine_promote(...)`), or
    /// `<AppliedFix as Trait>::` UFCS forms. Defining or calling a
    /// free function or method with this exact name elsewhere will
    /// fail the lint. The lint is an external AST-walking tool —
    /// it does NOT honor `#[allow(...)]` attributes, because Rust
    /// attribute lints and external CI lints are separate
    /// mechanisms. The remediation paths are: (a) rename the
    /// offending function (the simplest answer; the `__` prefix is
    /// project-reserved precisely so this rename is a normal cost),
    /// (b) co-locate the fn inside the engine's allow-listed surface
    /// (`Engine::fix_inner` / `Engine::apply_text_corrections` /
    /// the `engine_promotion_token` helper at
    /// `crates/engine/src/engine.rs`), or (c) extend the lint's
    /// allow-list in `tools/promote-callsite-lint/src/callsite.rs`
    /// after explicit team-review approval (and add a regression
    /// test pinning the new shape). The `__` prefix and
    /// `#[doc(hidden)]` attribute below reinforce that the name is
    /// project-internal — anyone reading this name in source should
    /// know they're looking at the engine-only audit-promotion seal.
    ///
    /// # Engine-only contract (production code)
    ///
    /// This constructor exists in `marque-rules` for type co-location, but
    /// in **production code** **must only be called from
    /// `marque-engine::Engine::fix`**. Rule crates and CLI code must never
    /// construct `AppliedFix` directly — they produce `FixProposal`
    /// values and let the engine promote them.
    ///
    /// The engine snapshots `proposal.confidence` and `proposal.source`
    /// into the top-level `confidence` / `source` fields at promotion
    /// time. A future phase may adjust these per region-context before
    /// snapshotting; Phase 2 copies them unchanged.
    ///
    /// # Type-level seal
    ///
    /// The `_token: EnginePromotionToken` parameter is the seal: an
    /// instance can only be obtained via
    /// [`EnginePromotionToken::__engine_construct`], whose
    /// engine-only contract mirrors this one. Because
    /// `EnginePromotionToken`'s sole field is private to
    /// `marque-rules`, no external crate can brace-construct one — the
    /// bypass surface collapses to a single named type. A grep for
    /// `EnginePromotionToken` outside `marque-engine` (or test code
    /// covered by the carve-out below) flags every Constitution V
    /// violation in one pass.
    ///
    /// The seal is still convention-based at the cross-crate level
    /// (Rust does not provide a way to scope `pub` to a specific
    /// downstream crate without `cfg` features that any caller can
    /// flip), but the convention is now load-bearing at the type
    /// level: the named token threads the bypass through one
    /// auditable choke point instead of leaving it as a single
    /// generically-named function.
    ///
    /// # Test-fixture carve-out
    ///
    /// Test code MAY call `__engine_promote` directly (and mint a
    /// token via [`EnginePromotionToken::__engine_construct`]) to
    /// construct synthetic `AppliedFix` fixtures for unit-testing
    /// audit-emission machinery (renderers, sentinel checks, NDJSON
    /// serialization) without spinning up a full `Engine`. The
    /// carve-out is scoped per Constitution V Principle V:
    ///
    /// - Call sites MUST live inside `#[cfg(test)]` modules, `tests/`
    ///   integration files, or test-utility crates gated as
    ///   `dev-dependencies`. Production code calling this constructor
    ///   from `cfg(not(test))` violates the contract.
    /// - Fabricated `AppliedFix` values MUST NOT be commingled with
    ///   engine-promoted fixes (spliced into a real audit stream,
    ///   etc.).
    /// - The carve-out covers test-fixture *construction* only. CLI
    ///   helpers, batch tooling, and benchmark drivers that want an
    ///   `AppliedFix` for non-test purposes are not in scope.
    ///
    /// Each test call site SHOULD carry an inline comment naming the
    /// carve-out so future reviewers don't have to re-derive the
    /// policy.
    ///
    /// # PR 3c.B Commit 2 — legacy-path constructor
    ///
    /// This is the legacy-path constructor that wraps a
    /// [`FixProposal`] in [`AppliedFixProposal::Legacy`]. Used by
    /// non-migrated rules during the Commit 2–9 transition. The
    /// reserved-name lint flags every path whose last segment is
    /// `__engine_promote`; `__engine_promote_legacy` (this method)
    /// is **NOT** caught by the suffix-match lint
    /// (anchored on exact last-segment equality), so this method's
    /// existence does not require an allow-list extension.
    ///
    /// Commit 10 retires this constructor atomically with the
    /// audit-schema flip; from that point onward only
    /// [`AppliedFix::__engine_promote`] (taking a `FixIntent<S>`)
    /// remains.
    #[doc(hidden)]
    pub fn __engine_promote_legacy(
        proposal: FixProposal,
        timestamp: SystemTime,
        classifier_id: Option<Arc<str>>,
        dry_run: bool,
        input: Option<Arc<str>>,
        _token: EnginePromotionToken,
    ) -> Self {
        let confidence = proposal.confidence.clone();
        let source = proposal.source;
        Self {
            proposal: AppliedFixProposal::Legacy(proposal),
            confidence,
            source,
            timestamp,
            classifier_id,
            dry_run,
            input,
        }
    }

    /// Promote a [`FixIntent<S>`] to an [`AppliedFix<S>`] with
    /// runtime context.
    ///
    /// Mirrors [`AppliedFix::__engine_promote_legacy`] for the new
    /// emission path. Wraps the intent in
    /// [`AppliedFixProposal::New`]; snapshots `confidence` from the
    /// intent. `source` defaults to [`FixSource::BuiltinRule`] for
    /// Commit 2 — rules that need to carry an explicit `FixSource`
    /// through the new path do so in a follow-up; the default is
    /// the only `FixSource` that fires in Commit 2 because no rule
    /// emits `FixIntent` yet.
    ///
    /// The same engine-only contract and test-fixture carve-out
    /// from [`AppliedFix::__engine_promote_legacy`] apply — see
    /// that method's doc comment for the binding policy.
    ///
    /// # Reserved name (FR-040 lint contract)
    ///
    /// The function name `__engine_promote` is reserved by the
    /// marque project. The `tools/promote-callsite-lint/` CI lint
    /// flags every call expression whose path's last segment is
    /// `__engine_promote` regardless of the leading qualifier. The
    /// companion method `__engine_promote_legacy` is **not** caught
    /// by the lint because the last-segment check is anchored on
    /// exact equality (`"__engine_promote"`) and
    /// `"__engine_promote_legacy"` is a distinct identifier.
    ///
    /// `_rule_id` belongs on the diagnostic; the `FixIntent` carries
    /// no rule field by design. The parameter is accepted here so the
    /// call shape mirrors `__engine_promote_legacy` (which derives the
    /// rule from `proposal.rule`). It is prefixed with `_` per the
    /// idiomatic "accepted-but-intentionally-unused parameter" convention.
    /// Commit 3+ may surface a rule field on `AppliedFix` directly
    /// when the audit-schema flip lands.
    ///
    /// `synthesized` is the engine's projection of `intent` into a
    /// legacy `FixProposal` shape — constructed by the caller (the
    /// engine) via `marque_engine::fix_intent_to_legacy_proposal`
    /// before this constructor runs. Caching it inside the
    /// `AppliedFixProposal::New` variant means `Deref<Target =
    /// FixProposal>` always returns a valid `&FixProposal`, so the
    /// audit-emit path stays Path-C-stable through commits 2–9
    /// without a runtime-panic foot-gun. The synthesized projection
    /// MUST set `original = ""` to preserve Constitution V Principle V
    /// G13 closure on the new path; the engine's helper enforces this.
    /// `source` is snapshotted from `synthesized.source` for symmetry
    /// with `__engine_promote_legacy`.
    //
    // `clippy::too_many_arguments` allowed because every parameter
    // carries engine-only runtime context that the seal must capture
    // atomically: the rule_id (audit-record provenance), the intent
    // (the rule's emission), the synthesized projection (the engine's
    // canonicalization), the clock-injected timestamp, the classifier
    // identity, the dry-run flag, the caller-supplied input
    // identifier, and the EnginePromotionToken seal proof. Refactoring
    // into a struct argument would shift the API surface without
    // reducing the parameter count visible at the engine call site.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __engine_promote(
        _rule_id: RuleId,
        intent: FixIntent<S>,
        synthesized: FixProposal,
        timestamp: SystemTime,
        classifier_id: Option<Arc<str>>,
        dry_run: bool,
        input: Option<Arc<str>>,
        _token: EnginePromotionToken,
    ) -> Self {
        let confidence = intent.confidence.clone();
        let source = synthesized.source;
        Self {
            proposal: AppliedFixProposal::New {
                intent: Box::new(intent),
                synthesized,
            },
            confidence,
            source,
            timestamp,
            classifier_id,
            dry_run,
            input,
        }
    }
}

/// Engine-only proof-of-construction token for [`AppliedFix::__engine_promote`].
///
/// `AppliedFix::__engine_promote` accepts an `EnginePromotionToken`; the
/// only way to obtain one is [`EnginePromotionToken::__engine_construct`].
/// Because the token's sole field is private to `marque-rules`, no
/// external crate can brace-construct one, and the constructor is
/// `#[doc(hidden)]` and named to make the bypass intent obvious at the
/// call site.
///
/// This is the type-level seal for Constitution V Principle V's
/// engine-only contract on audit-record promotion. See
/// [`AppliedFix::__engine_promote`] for the binding contract and the
/// test-fixture carve-out.
///
/// # Compile-fail proof of the seal
///
/// External crates cannot brace-construct an `EnginePromotionToken`
/// because the `_seal` field is private to `marque-rules`. Doctests
/// compile as separate crates against the library's public API, so
/// the following snippet is rejected by the compiler — which is what
/// `compile_fail` asserts:
///
/// ```compile_fail
/// // External crates see `EnginePromotionToken` but not `_seal`,
/// // so brace-construction is rejected. Bypass requires calling
/// // `EnginePromotionToken::__engine_construct()`, which is the
/// // single auditable bypass surface.
/// let _token = marque_rules::EnginePromotionToken { _seal: () };
/// ```
#[derive(Debug)]
pub struct EnginePromotionToken {
    _seal: (),
}

impl EnginePromotionToken {
    /// Mint an [`EnginePromotionToken`].
    ///
    /// # Reserved name (FR-040 lint contract)
    ///
    /// As with [`AppliedFix::__engine_promote`], the function name
    /// `__engine_construct` is reserved by the marque project. The
    /// `tools/promote-callsite-lint/` CI lint flags every call
    /// expression whose path's last segment is `__engine_construct`
    /// regardless of leading qualifier (qualified, fully-qualified,
    /// `Self::`, aliased, UFCS). Defining or calling another
    /// function with this exact name elsewhere will fail the lint.
    /// The `__` prefix + `#[doc(hidden)]` attribute reinforce the
    /// reserved status; see [`AppliedFix::__engine_promote`] for the
    /// full contract and the rationale for last-segment matching.
    ///
    /// # Engine-only contract (production code)
    ///
    /// Only `marque-engine` may call this in production code. The
    /// same three-constraint test-fixture carve-out from
    /// [`AppliedFix::__engine_promote`] applies here verbatim — see
    /// that constructor's doc comment for the binding definition.
    /// Outside the engine, calling this from `cfg(not(test))` code
    /// violates Constitution V Principle V.
    #[doc(hidden)]
    #[inline]
    pub const fn __engine_construct() -> Self {
        Self { _seal: () }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// A single diagnostic emitted by a rule check.
///
/// # Generic over the marking scheme
///
/// `Diagnostic<S>` is generic post-PR 3c.B so the new `fix_intent`
/// field can carry a scheme-typed [`FixIntent<S>`]. The legacy
/// `fix` field continues to carry a `FixProposal` for non-migrated
/// rules during the Commit 2–9 transition. At most one of the two
/// fields is populated per diagnostic; the engine fires an
/// invariant check (debug-assert) on construction paths where both
/// are present.
#[non_exhaustive]
#[derive(Debug)]
pub struct Diagnostic<S: MarkingScheme> {
    pub rule: RuleId,
    pub severity: Severity,
    /// Byte span in the original source buffer.
    pub span: Span,
    /// Human-readable description of the violation.
    pub message: Box<str>,
    /// CAPCO section citation, e.g., "CAPCO-2016 §A.6"
    /// (refers to the CAPCO Register and Manual, 2016).
    pub citation: &'static str,
    /// Proposed fix, if the rule can generate one (legacy path —
    /// non-migrated rules emit here through Commit 9).
    pub fix: Option<FixProposal>,
    /// Structural fix intent, if the rule can generate one (new
    /// path — migrated rules emit here from Commit 3+). At most one
    /// of `fix` and `fix_intent` is populated per diagnostic; the
    /// engine's promotion path debug-asserts the invariant.
    pub fix_intent: Option<FixIntent<S>>,
}

// Manual Clone for Diagnostic<S> — see the parallel Clone impl on
// `AppliedFix<S>` for the rationale. The derive would over-constrain
// to `S: Clone`; the manual impl works for any well-formed scheme
// because the only S-typed payload is `FixIntent<S>` (which is
// `Clone` by its own derive).
impl<S: MarkingScheme> Clone for Diagnostic<S> {
    fn clone(&self) -> Self {
        Self {
            rule: self.rule.clone(),
            severity: self.severity,
            span: self.span,
            message: self.message.clone(),
            citation: self.citation,
            fix: self.fix.clone(),
            fix_intent: self.fix_intent.clone(),
        }
    }
}

impl<S: MarkingScheme> Diagnostic<S> {
    /// Construct a new diagnostic carrying a legacy `FixProposal`
    /// (or `None`).
    ///
    /// This is the back-compat constructor used by non-migrated
    /// rules during the Commit 2–9 transition. Migrated rules use
    /// [`Diagnostic::with_fix_intent`] (Commit 3+).
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
            fix_intent: None,
        }
    }

    /// Construct a new diagnostic carrying a structural
    /// [`FixIntent<S>`] (or `None`).
    ///
    /// This is the new-path constructor that migrated rules use
    /// starting in Commit 3+. The legacy `fix` field is set to
    /// `None`; the engine's promotion path picks up `fix_intent`
    /// instead.
    pub fn with_fix_intent(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: impl Into<Box<str>>,
        citation: &'static str,
        fix_intent: Option<FixIntent<S>>,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            message: message.into(),
            citation,
            fix: None,
            fix_intent,
        }
    }

    /// Construct a new diagnostic carrying BOTH a legacy
    /// [`FixProposal`] AND a structural [`FixIntent<S>`].
    ///
    /// This is the dual-population constructor used by rules
    /// migrated under PR 3c.B Commit 3+ (E054, E057, E021, ...).
    /// The legacy `fix` field is the pre-migration FixProposal —
    /// byte-identical to what the rule emitted before migration —
    /// and the engine wraps it inside `AppliedFixProposal::New {
    /// intent, synthesized: fix }` at promotion time so the
    /// NDJSON audit shape stays byte-stable through the Commit
    /// 2–9 transition (Path C of the consolidated plan).
    ///
    /// The `Engine::fix_inner` promotion path builds an
    /// `(rule_id, span)`-keyed index of `fix_intent` values from
    /// the diagnostic stream and routes paired diagnostics to
    /// [`AppliedFix::__engine_promote`] (taking the intent +
    /// synthesized projection) instead of
    /// [`AppliedFix::__engine_promote_legacy`] (taking only the
    /// proposal).
    ///
    /// `fix` and `fix_intent` MUST describe the same repair; the
    /// rule's responsibility is to ensure the structural intent
    /// agrees with what the byte-precise FixProposal will produce
    /// when applied. Commit 10 retires the dual-population
    /// pattern atomically with the audit-schema flip — at that
    /// point migrated rules drop the legacy `fix` field entirely
    /// and the synthesized projection is reconstructed from the
    /// intent at audit-emit time.
    pub fn with_fix_and_intent(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: impl Into<Box<str>>,
        citation: &'static str,
        fix: FixProposal,
        fix_intent: FixIntent<S>,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            message: message.into(),
            citation,
            fix: Some(fix),
            fix_intent: Some(fix_intent),
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
///
/// # Generic over the marking scheme
///
/// `Rule<S>` is generic post-PR 3c.B so `check`'s return type can
/// carry scheme-typed [`FixIntent<S>`] payloads through
/// [`Diagnostic<S>`]. Every consumer crate instantiates
/// `Rule<CapcoScheme>`. The `Box<dyn Rule<S>>` shape stays sound;
/// `Box<dyn Rule<CapcoScheme>>` is the production form used by
/// `RuleSet<CapcoScheme>`.
pub trait Rule<S: MarkingScheme>: Send + Sync {
    fn id(&self) -> RuleId;
    fn name(&self) -> &'static str;
    /// Default severity — overridable per rule in `.marque.toml`.
    fn default_severity(&self) -> Severity;
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<S>>;

    /// Additional rule IDs / names this rule may emit on diagnostics
    /// beyond its registered `id()` / `name()`. Each entry is
    /// `(rule_id, rule_name)` and contributes to:
    ///
    /// 1. The engine's `canonicalize_rule_overrides` known-keys set —
    ///    so a `.marque.toml` configuring an emitted-only ID
    ///    (`E035 = "warn"`) is accepted instead of failing as
    ///    `UnknownRuleOverride`.
    /// 2. The engine's per-emitted-id severity-override path at lint
    ///    time — the override the user wrote against the catalog ID
    ///    is resolved against the diagnostic's emitted `rule` field.
    ///
    /// Default: empty. Only dispatcher walkers like
    /// `BannerMatchesProjectedRule` (T026a) — which register under one
    /// bookkeeping ID but emit diagnostics under per-row catalog IDs
    /// — need to override this. A rule whose registered `id()` matches
    /// every diagnostic it emits should leave this at the default.
    fn additional_emitted_ids(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }
}

/// A collection of rules provided by a rule crate.
/// Returned by the rule crate's entry point function.
pub trait RuleSet<S: MarkingScheme>: Send + Sync {
    fn rules(&self) -> &[Box<dyn Rule<S>>];
    fn schema_version(&self) -> &'static str;
}

// FR-038 / T002 — `Send + Sync` for the `Rule` and `RuleSet` traits is
// declared by the `pub trait Rule: Send + Sync` and
// `pub trait RuleSet: Send + Sync` supertrait bounds above. The
// trait-object dimension (`Box<dyn Rule>: Send + Sync`,
// `Arc<dyn Rule>: Send + Sync`, plus the analogous `RuleSet` shapes)
// is exercised by `tests/send_sync.rs`, which is the integration test
// that fails to compile if a future bound relaxation breaks the
// trait-object form. This file no longer carries an inline assertion;
// the supertrait bounds plus that companion test are the load-bearing
// guards.

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
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
        assert_eq!(Severity::parse_config("suggest"), Some(Severity::Suggest));
        assert_eq!(Severity::parse_config("info"), Some(Severity::Info));
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
            Severity::Suggest,
            Severity::Info,
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
        // Off < Suggest < Info < Warn < Error < Fix — see the doc comment
        // on Severity for the intentional design rationale.
        assert!(Severity::Off < Severity::Suggest);
        assert!(Severity::Suggest < Severity::Info);
        assert!(Severity::Info < Severity::Warn);
        assert!(Severity::Warn < Severity::Error);
        assert!(Severity::Error < Severity::Fix);
    }

    #[test]
    fn severity_suggest_round_trips_through_config_string() {
        // Issue #235 / #186 PR-3: the suggest-don't-fix channel must be
        // a stable parse target. The config string "suggest" must round
        // trip through both parse_config and as_str.
        assert_eq!(Severity::parse_config("suggest"), Some(Severity::Suggest));
        assert_eq!(Severity::Suggest.as_str(), "suggest");
        assert_eq!(Severity::Suggest.to_string(), "suggest");
    }

    #[test]
    fn severity_suggest_is_strictly_below_info_in_ord() {
        // The renderer relies on Suggest sorting BELOW Info so that
        // CI exit-code logic ("Info or none → exit 0") generalizes
        // to ("Info-or-Suggest or none → exit 0") via the same
        // strict-less-than comparison.
        assert!(Severity::Suggest < Severity::Info);
        assert!(Severity::Off < Severity::Suggest);
    }

    #[test]
    fn fix_proposal_new_accepts_boundary_confidence() {
        let _zero = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            Confidence::strict(0.0),
            None,
        );
        let _one = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            Confidence::strict(1.0),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "Confidence::strict rule confidence")]
    fn fix_proposal_new_panics_on_negative_confidence() {
        let _ = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            Confidence::strict(-0.1),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "Confidence::strict rule confidence")]
    fn fix_proposal_new_panics_on_above_one_confidence() {
        let _ = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            Confidence::strict(1.5),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "Confidence::strict rule confidence")]
    fn fix_proposal_new_panics_on_nan_confidence() {
        let _ = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            Confidence::strict(f32::NAN),
            None,
        );
    }

    #[test]
    fn fix_proposal_new_panics_when_axis_is_nan() {
        // A directly-constructed Confidence can still have NaN axes
        // that slip past the strict-path assert. Verify the
        // FixProposal::new gate catches that case too.
        let bad = Confidence {
            recognition: f32::NAN,
            rule: 1.0,
            region: None,
            runner_up_ratio: None,
            features: Vec::new(),
        };
        let caught = std::panic::catch_unwind(|| {
            FixProposal::new(
                RuleId::new("E001"),
                FixSource::BuiltinRule,
                Span::new(0, 0),
                "x",
                "y",
                bad,
                None,
            );
        });
        assert!(
            caught.is_err(),
            "expected FixProposal::new to panic on NaN recognition axis"
        );
    }

    #[test]
    fn fix_proposal_new_panics_when_axis_out_of_range() {
        // combined() = recognition × rule can still land in [0, 1]
        // even when an individual axis is out of range
        // (e.g. recognition = 2.0, rule = 0.4 ⇒ combined = 0.8).
        // Validating only the product would let this through; the
        // per-axis check catches it.
        let bad = Confidence {
            recognition: 2.0,
            rule: 0.4,
            region: None,
            runner_up_ratio: None,
            features: Vec::new(),
        };
        // Sanity check: combined() IS in [0, 1] — that's the whole
        // point of adding per-axis validation.
        assert!((0.0..=1.0).contains(&bad.combined()));
        let caught = std::panic::catch_unwind(|| {
            FixProposal::new(
                RuleId::new("E001"),
                FixSource::BuiltinRule,
                Span::new(0, 0),
                "x",
                "y",
                bad,
                None,
            );
        });
        assert!(
            caught.is_err(),
            "expected FixProposal::new to panic on out-of-range recognition axis"
        );
    }

    #[test]
    fn fix_proposal_new_panics_when_feature_delta_is_nan() {
        let bad = Confidence {
            recognition: 0.9,
            rule: 0.9,
            region: None,
            runner_up_ratio: None,
            features: vec![FeatureContribution {
                id: FeatureId::EditDistance1,
                delta: f32::NAN,
            }],
        };
        let caught = std::panic::catch_unwind(|| {
            FixProposal::new(
                RuleId::new("E001"),
                FixSource::BuiltinRule,
                Span::new(0, 0),
                "x",
                "y",
                bad,
                None,
            );
        });
        assert!(
            caught.is_err(),
            "expected FixProposal::new to panic on NaN feature delta"
        );
    }

    #[test]
    fn fix_proposal_new_accepts_runner_up_ratio_above_one() {
        // runner_up_ratio can legitimately be > 1.0 — it's a ratio,
        // not a unit interval. Verify the per-axis validator doesn't
        // over-constrain it.
        let ok = Confidence {
            recognition: 0.9,
            rule: 0.9,
            region: None,
            runner_up_ratio: Some(3.5),
            features: Vec::new(),
        };
        let _ = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(0, 0),
            "x",
            "y",
            ok,
            None,
        );
    }
}
