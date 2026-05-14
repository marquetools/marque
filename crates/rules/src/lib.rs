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
//!   `FeatureContribution`. Phase D audit-provenance payload attached to
//!   every `FixIntent<S>`.
//! - [`message`] — `Message`, `MessageTemplate` (closed enum), `MessageArgs`
//!   (closed-set struct). The G13 type-system closure of the diagnostic-message
//!   leak channel: only `Message::new(template, args)` constructs a `Message`,
//!   and `MessageArgs` cannot carry input bytes (no `String` / `&str` / `Vec<u8>`
//!   fields).
//! - [`fix_intent`] — `FixIntent<S>`. The rule-emission API for the
//!   bag-of-tokens vocabulary from `architecture.md` §"What fixes are":
//!   fact-set deltas (`FactAdd` / `FactRemove`) and renderer
//!   recanonicalization (`Recanonicalize`). `ReplacementIntent<S>`,
//!   `FactRef<S>`, and `RecanonScope` live in `marque-scheme`; rules
//!   import them directly from there. The engine promotes a
//!   `FixIntent<S>` to an `AppliedFix<S>` via `__engine_promote`.
//!
//! # Type split: FixIntent vs AppliedFix
//!
//! `FixIntent<S>` is pure data emitted by rules — deterministic,
//! timestamp-free, classifier-free, safe to snapshot in tests.
//! `AppliedFix<S>` wraps it (via the `AppliedFixProposal<S>` enum)
//! with runtime context (timestamp, classifier id, dry-run flag) and
//! is constructed **only** by `Engine::fix_inner`. This makes
//! "suggested vs applied" a type-system invariant.
//!
//! The Commit 2–9 transition through a legacy `FixProposal` shape
//! retired in PR 3c.B Commit 10 — atomically with the
//! `MARQUE_AUDIT_SCHEMA` flip from `"marque-mvp-2"` to `"marque-mvp-3"`.
//! `AppliedFixProposal<S>` is now a two-variant enum: `FixIntent(_)`
//! for engine-promoted rule emissions and `TextCorrection { ... }`
//! for engine-internal C001 text replacements.
//!
//! # G13 (audit content ignorance)
//!
//! `FixIntent<S>` carries only structural references (`FactRef`,
//! category IDs, `Scope` / `RecanonScope` tags) — no document bytes.
//! `AppliedFixProposal::TextCorrection` carries the canonical
//! replacement string (a corpus-derived token canonical on
//! Constitution V's permitted-identifier list, e.g. `"SECRET"`
//! replacing a typo); it never carries the document's original bytes.
//! Audit records emit no `original` field as of `marque-mvp-3`.

pub mod audit_note;
pub mod confidence;
pub mod fix_intent;
pub mod message;

use marque_ism::{CanonicalAttrs, Span};
use marque_scheme::MarkingScheme;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

pub use audit_note::{AuditNote, AuditNoteKind, AuditNoteStructural};
pub use confidence::{Confidence, FeatureContribution, FeatureId};
pub use fix_intent::FixIntent;
// Re-export `SmallVec` + the `smallvec!` macro so external consumers
// can construct `Confidence.features` (a `SmallVec<[FeatureContribution; 4]>`)
// and any other rules-crate SmallVec field without depending on the
// `smallvec` crate directly. The inline storage is an implementation
// detail of the audit-record payload; the re-export keeps it that
// way at the boundary.
pub use smallvec::{SmallVec, smallvec};
// `FactRef`, `ReplacementIntent`, and `RecanonScope` moved to
// `marque-scheme` as of the PR 3c.B engine-prereq (the new
// `MarkingScheme::apply_intent` trait method needs them at the trait
// surface; `marque-rules` already depends on `marque-scheme`, so the
// types must live below us in the dependency graph). Import them
// directly from `marque_scheme::{FactRef, RecanonScope, ReplacementIntent}`.
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
/// `Severity` lives in `marque-scheme` as of PR 3c.B Commit 7 prep
/// (so [`marque_scheme::constraint::ConstraintViolation`] and other
/// scheme-layer types can carry per-row severity without violating
/// Constitution VII's leaf-only rule for the scheme crate).
/// `marque_rules::Severity` is a re-export so existing import sites
/// continue to work unchanged.
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
pub use marque_scheme::Severity;

// ---------------------------------------------------------------------------
// Phase
// ---------------------------------------------------------------------------

/// Dispatch phase declared by each [`Rule`] at registration. Drives the
/// engine's two-pass fix pipeline (PR 7 of the engine refactor).
///
/// FR-021 (`specs/006-engine-rule-refactor/spec.md`) makes the phase a
/// rule-level promise about the span shape of every `Diagnostic` the
/// rule emits — the `Diagnostic::span` field, regardless of whether
/// the rule's fix payload is a structural `FixIntent` or a
/// `Diagnostic::text_correction` (e.g., C001 corrections-map, E006
/// deprecation migrations). Note: `FixIntent` itself carries no span
/// — spans live on `Diagnostic::span` and `RuleContext::candidate_span`
/// and are promoted onto `AppliedFix::span` by the engine. The phase
/// is not an engine-side classification.
///
/// The engine partitions the registered rule set by phase once at
/// `Engine::new`; pass-1 dispatches `Phase::Localized` rules against
/// the post-C001 buffer and applies their fixes, then re-parses;
/// pass-2 dispatches `Phase::WholeMarking` rules against the post-pass-1
/// attrs (with the pre-pass-1 attrs cached for FR-023 disambiguation;
/// the cache plumbing lands in PR 7c).
///
/// No `Phase::Both` escape hatch. A defect class that genuinely needs
/// detection in both phases registers two rule entries (one per phase)
/// sharing a backend module — see `docs/plans/2026-05-02-engine-refactor-consolidated.md`
/// §9.1 for the design rationale.
///
/// PR 7a (this commit) plumbs the type into `Rule` and stashes a
/// partition on `Engine`; pass-split dispatch lands in 7b.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Every `Diagnostic` the rule emits has a `Diagnostic::span`
    /// strictly inside a single token boundary — applies regardless
    /// of whether the fix payload is a structural `FixIntent` or a
    /// `Diagnostic::text_correction`. Examples: a deprecation rewrite
    /// (`OC → ORCON`) carrying a `FixIntent`, or a corpus-typo
    /// correction (`SERCET → SECRET`) carrying a
    /// `Diagnostic::text_correction`. Pass-1 applies these fixes via
    /// a forward-pass buffer splice before re-parsing for pass-2.
    /// The constraint is *boundary-respect*, not span stability:
    /// any byte-length-changing splice shifts every later span, but the
    /// re-parse between passes recomputes spans from scratch. The
    /// reason pass-1 fixes must stay inside one token is that crossing
    /// a token boundary (separators, structural delimiters) risks
    /// producing an unparseable buffer — handled by the FR-024 R002
    /// path, but better avoided by construction.
    ///
    /// First-fire span-shape enforcement lives in `Engine::fix_inner`
    /// (PR 7b); a rule that misdeclares `Localized` and emits a wider
    /// span is dropped from pass-1 with a `tracing::error!`, not
    /// promoted to `AppliedFix`.
    Localized,
    /// `Diagnostic::span` (and `Diagnostic::candidate_span`, when
    /// populated) covers a portion, banner, or page scope. Examples:
    /// a banner roll-up walker, a class-floor walker, or any rule
    /// whose `FixIntent` carries `ReplacementIntent::FactAdd` /
    /// `FactRemove` / `Recanonicalize`. `Diagnostic::text_correction`
    /// is rare in this phase but follows the same span-shape contract
    /// when used. Pass-2 sees post-pass-1 attrs and, in PR 7c, the
    /// pre-pass-1 attrs cache for FR-023 disambiguation.
    ///
    /// This is the default returned by [`Rule::phase`] for rules that
    /// do not override the method (see [`Rule::phase`]'s documentation
    /// for the design rationale per PM decision D-7.2 in
    /// `docs/refactor-006/pr-7-pm-decisions.md`).
    WholeMarking,
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
    /// Byte span of the scanner-emitted candidate this rule check is
    /// running against. This is the marking-scope span (the full
    /// portion or banner candidate), distinct from any sub-span a
    /// diagnostic might point at (e.g., a single token within the
    /// portion).
    ///
    /// Intent-only rules (those emitting `Diagnostic.fix_intent` with
    /// no `fix` field) copy this into `Diagnostic.candidate_span` so
    /// the engine's intent-synthesis path knows which scope-bytes to
    /// re-render via `MarkingScheme::apply_intent` +
    /// `MarkingScheme::render_canonical`.
    ///
    /// Added in the PR 3c.B engine-prereq commit. Populated by the
    /// engine from `candidate.span` before invoking each rule.
    pub candidate_span: Span,
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
    /// [`FixIntent::confidence`] so auditors can reconstruct the
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
// AppliedFix (= Audit Record)
// ---------------------------------------------------------------------------

/// Engine-promoted proposal payload — the body of an [`AppliedFix`].
///
/// Carries either a rule-emitted [`FixIntent<S>`] or an engine-internal
/// text-correction (C001, the `[corrections]` map). The legacy
/// `FixProposal` shape retired in PR 3c.B Commit 10 atomically with
/// the `marque-mvp-3` audit schema flip; no rule emits the legacy
/// shape post-cutover.
///
/// The `TextCorrection` variant carries only the canonical
/// replacement string (a corpus-derived token canonical, e.g.
/// `"SECRET"` replacing a typo). The original document bytes are
/// never copied into the audit record — Constitution V Principle V
/// (G13). The audit envelope's `proposal.kind` discriminant tells
/// downstream consumers which arm produced the fix.
#[derive(Debug)]
pub enum AppliedFixProposal<S: MarkingScheme> {
    /// Rule-emitted structural fix intent — the sole rule-emission
    /// channel post Commit 10. Boxed because `FixIntent<S>` carries
    /// `Confidence` + `Message` + `SmallVec` inline storage and the
    /// enum's other variant is a thin two-field carrier; without the
    /// box clippy's `large_enum_variant` lint fires.
    FixIntent(Box<FixIntent<S>>),

    /// Engine-internal text correction — the C001 path
    /// (`[corrections]` map). Constructed only by
    /// `Engine::apply_text_corrections`; never by a rule crate. The
    /// `replacement` field carries the canonical token bytes
    /// (corpus-derived, on Constitution V's permitted-identifier
    /// list) — no document content.
    TextCorrection {
        /// Canonical replacement bytes (corpus-derived token canonical).
        replacement: SmolStr,
    },
}

// Manual Clone for AppliedFixProposal<S> — see the parallel Clone
// impl on `AppliedFix<S>` for the rationale. `S` itself is never
// cloned; only `S::OpenVocabRef` (which is `Clone`-bounded by the
// `MarkingScheme` trait) flows through.
impl<S: MarkingScheme> Clone for AppliedFixProposal<S> {
    fn clone(&self) -> Self {
        match self {
            AppliedFixProposal::FixIntent(intent) => AppliedFixProposal::FixIntent(intent.clone()),
            AppliedFixProposal::TextCorrection { replacement } => {
                AppliedFixProposal::TextCorrection {
                    replacement: replacement.clone(),
                }
            }
        }
    }
}

/// A promoted `FixIntent<S>` (or engine-internal `TextCorrection`)
/// with runtime context.
///
/// Constructed **only** by `Engine::fix_inner` (or its
/// `apply_text_corrections` partner) at the moment a fix meets the
/// confidence threshold. Never constructed by a rule or suggestion
/// path. Serves as the audit record: the NDJSON schema at
/// `contracts/audit-record.md` (active version: `marque-mvp-3`)
/// serializes this type.
///
/// `classifier_id` is an `Arc<str>` so promoting many fixes from a
/// single document only clones an atomic refcount, not the
/// underlying string.
///
/// # Generic over the marking scheme
///
/// `AppliedFix<S>` is generic so the `FixIntent(FixIntent<S>)`
/// variant of [`AppliedFixProposal`] preserves the scheme-typed
/// payload. `marque-engine` and downstream surfaces (server, WASM,
/// CLI) instantiate `AppliedFix<CapcoScheme>` at the boundary.
///
/// # Top-level audit fields
///
/// `rule`, `span`, `confidence`, `source`, and `migration_ref` are
/// snapshot at the **top level** of `AppliedFix` (rather than
/// nested under `proposal`) so audit emitters do not have to descend
/// into the proposal variant for the common audit-shape fields. The
/// engine snapshots them from the intent (or text-correction
/// parameters) at promotion time; a future phase may adjust them
/// for region context before promotion, so they can diverge from
/// the original `FixIntent.confidence` / `FixIntent.source`.
#[non_exhaustive]
#[derive(Debug)]
pub struct AppliedFix<S: MarkingScheme> {
    /// The fix's rule ID. Snapshot at the top level so audit
    /// emitters don't have to peer into the proposal variant.
    pub rule: RuleId,
    /// Byte span in the original source the fix targeted.
    pub span: Span,
    /// The rule's structural emission (or engine-internal
    /// `TextCorrection`).
    pub proposal: AppliedFixProposal<S>,
    /// Snapshot of the fix's confidence at promotion time.
    pub confidence: Confidence,
    /// Snapshot of the fix's provenance at promotion time.
    pub source: FixSource,
    /// Reference to the CAPCO rule or migration document
    /// justifying this fix. Snapshot from the intent (or `None` for
    /// the C001 text-correction path).
    pub migration_ref: Option<&'static str>,
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
            rule: self.rule.clone(),
            span: self.span,
            proposal: self.proposal.clone(),
            confidence: self.confidence.clone(),
            source: self.source,
            migration_ref: self.migration_ref,
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
    /// Promote a [`FixIntent<S>`] to an [`AppliedFix<S>`] with
    /// runtime context.
    ///
    /// Snapshots `confidence`, `source`, and `migration_ref` from
    /// `intent` at the top level of the resulting `AppliedFix`. The
    /// intent itself moves into [`AppliedFixProposal::FixIntent`].
    ///
    /// # Reserved name (FR-040 lint contract)
    ///
    /// The function name `__engine_promote` is reserved by the
    /// marque project. The `tools/promote-callsite-lint/` CI lint
    /// flags every call expression whose path's last segment is
    /// `__engine_promote` regardless of the leading qualifier
    /// (qualified, fully-qualified, `Self::`, aliased, UFCS). See
    /// the top of this file's `EnginePromotionToken` doc for the
    /// rationale and remediation paths.
    //
    // `clippy::too_many_arguments` allowed because every parameter
    // carries engine-only runtime context that the seal must capture
    // atomically: the rule_id (audit-record provenance), the span
    // (where the fix lands), the intent (the rule's emission), the
    // clock-injected timestamp, the classifier identity, the dry-run
    // flag, the caller-supplied input identifier, and the
    // EnginePromotionToken seal proof. Refactoring into a struct
    // argument would shift the API surface without reducing the
    // parameter count visible at the engine call site.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __engine_promote(
        rule: RuleId,
        span: Span,
        intent: FixIntent<S>,
        timestamp: SystemTime,
        classifier_id: Option<Arc<str>>,
        dry_run: bool,
        input: Option<Arc<str>>,
        _token: EnginePromotionToken,
    ) -> Self {
        let confidence = intent.confidence.clone();
        let source = intent.source;
        let migration_ref = intent.migration_ref;
        Self {
            rule,
            span,
            proposal: AppliedFixProposal::FixIntent(Box::new(intent)),
            confidence,
            source,
            migration_ref,
            timestamp,
            classifier_id,
            dry_run,
            input,
        }
    }

    /// Engine-only promotion path for text corrections (C001 /
    /// `[corrections]` map).
    ///
    /// `Engine::apply_text_corrections` is the sole production
    /// call site. The C001 path runs pre-scanner, so there is no
    /// rule-emitted `FixIntent` to promote — the engine carries
    /// the canonical replacement bytes directly through
    /// [`AppliedFixProposal::TextCorrection`]. The `replacement`
    /// payload is the corpus-derived canonical token (e.g.
    /// `"SECRET"` replacing `"SERCET"`); the original document
    /// bytes are never copied into the audit record (Constitution
    /// V Principle V).
    ///
    /// The same engine-only contract and test-fixture carve-out
    /// from [`AppliedFix::__engine_promote`] apply.
    //
    // `clippy::too_many_arguments` allowed for the same reason as
    // `__engine_promote`.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __engine_promote_text_correction(
        rule: RuleId,
        span: Span,
        replacement: SmolStr,
        source: FixSource,
        confidence: Confidence,
        timestamp: SystemTime,
        classifier_id: Option<Arc<str>>,
        dry_run: bool,
        input: Option<Arc<str>>,
        _token: EnginePromotionToken,
    ) -> Self {
        Self {
            rule,
            span,
            proposal: AppliedFixProposal::TextCorrection { replacement },
            confidence,
            source,
            migration_ref: None,
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
/// Reused by `AuditNote::__engine_promote` (T108e) under the same
/// engine-only contract.
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
/// `Diagnostic<S>` carries a scheme-typed [`FixIntent<S>`] in its
/// `fix` field. The pre-Commit-10 dual-channel shape (legacy
/// `FixProposal` + structural `FixIntent<S>`) collapsed into a
/// single `fix: Option<FixIntent<S>>` channel atomically with the
/// `marque-mvp-3` audit schema flip.
#[non_exhaustive]
#[derive(Debug)]
pub struct Diagnostic<S: MarkingScheme> {
    pub rule: RuleId,
    pub severity: Severity,
    /// Byte span in the original source buffer.
    pub span: Span,
    /// Optional marking-scope span (full portion or banner) when the
    /// `span` field points at a sub-region (e.g., a single token).
    /// Rules whose [`Self::fix`] payload is a `FixIntent` (i.e.,
    /// every fix-emitting rule post Commit 10) set this from
    /// [`RuleContext::candidate_span`] so the engine's
    /// intent-synthesis path knows which scope-bytes to re-render
    /// via [`marque_scheme::MarkingScheme::apply_intent`] +
    /// [`marque_scheme::MarkingScheme::render_canonical`].
    ///
    /// `None` when the diagnostic's `span` already covers the full
    /// scope.
    pub candidate_span: Option<Span>,
    /// Human-readable description of the violation.
    pub message: Box<str>,
    /// CAPCO section citation, e.g., "CAPCO-2016 §A.6"
    /// (refers to the CAPCO Register and Manual, 2016).
    pub citation: &'static str,
    /// Structural fix intent, if the rule can generate one. `None`
    /// for diagnostics that consciously decline to propose a fix
    /// (e.g. provisional Path-A rules, opaque-uncertain reductions),
    /// for informational diagnostics, or for C001 text-correction
    /// diagnostics (which carry their replacement bytes in
    /// [`Self::text_correction`] instead).
    pub fix: Option<FixIntent<S>>,
    /// Engine-applied byte-substitution payload (the C001 corrections-map
    /// path, plus the closely-shaped E006 deprecation-migration path).
    ///
    /// Carries the canonical replacement bytes plus the fix's
    /// provenance (`source`, `confidence`, `migration_ref`) so the
    /// engine's `apply_text_corrections` path can promote the fix
    /// with the rule's true provenance instead of hardcoding
    /// `FixSource::CorrectionsMap` for every text-correction. The
    /// replacement bytes are corpus-derived canonical tokens (e.g.
    /// `"SECRET"` replacing the typo `"SERCET"`, or `"NOFORN"`
    /// replacing the deprecated `"FOUO"`) — on Constitution V's
    /// permitted-identifier list. Never carries original document
    /// bytes.
    pub text_correction: Option<TextCorrection>,
}

/// Payload for an engine-applied byte-substitution fix.
///
/// Populated on [`Diagnostic::text_correction`] by rules whose repair
/// is a literal byte substitution that the engine applies atomically
/// in its pre-scanner pass. Carries the rule's provenance so the
/// engine's promotion path produces a faithful audit record (without
/// silently overwriting `FixSource` / `Confidence` / `migration_ref`).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TextCorrection {
    /// Canonical replacement bytes. On Constitution V's permitted-
    /// identifier list (token canonicals from a closed vocabulary).
    pub replacement: SmolStr,
    /// Provenance of the fix.
    pub source: FixSource,
    /// Multi-axis confidence. Threshold-gated like any other fix in
    /// the engine's promotion path.
    pub confidence: Confidence,
    /// Reference to the migration document or CAPCO row justifying
    /// this fix (e.g., a `§F p…` cite for E006 deprecations).
    pub migration_ref: Option<&'static str>,
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
            candidate_span: self.candidate_span,
            message: self.message.clone(),
            citation: self.citation,
            fix: self.fix.clone(),
            text_correction: self.text_correction.clone(),
        }
    }
}

impl<S: MarkingScheme> Diagnostic<S> {
    /// Construct a new diagnostic carrying a structural
    /// [`FixIntent<S>`] (or `None`).
    ///
    /// Alias for [`Self::with_fix`] kept for call-site ergonomics:
    /// `Diagnostic::new(...)` reads more naturally than
    /// `Diagnostic::with_fix(...)` when the `fix` arg is `None`.
    /// Behavior is identical to [`Self::with_fix`].
    pub fn new(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: impl Into<Box<str>>,
        citation: &'static str,
        fix: Option<FixIntent<S>>,
    ) -> Self {
        Self::with_fix(rule, severity, span, message, citation, fix)
    }

    /// Construct a new diagnostic carrying a structural
    /// [`FixIntent<S>`] (or `None`).
    ///
    /// This is the sole fix-attached constructor post Commit 10.
    /// Rules that emit a diagnostic with no fix pass `None`.
    pub fn with_fix(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: impl Into<Box<str>>,
        citation: &'static str,
        fix: Option<FixIntent<S>>,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            candidate_span: None,
            message: message.into(),
            citation,
            fix,
            text_correction: None,
        }
    }

    /// Construct a new diagnostic carrying a structural
    /// [`FixIntent<S>`] anchored at a marking-scope span.
    ///
    /// Identical to [`Self::with_fix`] but also populates
    /// [`Self::candidate_span`] from [`RuleContext::candidate_span`].
    /// Use when:
    ///
    /// - The diagnostic's `span` points at a *sub-region* of the
    ///   marking (e.g., a single token within a portion) — the
    ///   sub-span tells the user *where* the violation is, but the
    ///   engine needs the full marking-scope span to replace the
    ///   re-rendered output.
    /// - The rule emits a `Recanonicalize` or per-fact intent that
    ///   the engine synthesizes the replacement bytes for at
    ///   promotion time.
    pub fn with_fix_at_span(
        rule: RuleId,
        severity: Severity,
        span: Span,
        candidate_span: Span,
        message: impl Into<Box<str>>,
        citation: &'static str,
        fix: FixIntent<S>,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            candidate_span: Some(candidate_span),
            message: message.into(),
            citation,
            fix: Some(fix),
            text_correction: None,
        }
    }

    /// Construct a new C001 text-correction diagnostic.
    ///
    /// Used by the engine's pre-scanner aho-corasick scan and by
    /// the CAPCO `CorrectionsMapRule`. The replacement bytes are
    /// carried in [`Self::text_correction`]; the engine's
    /// `apply_text_corrections` reads this field and promotes it
    /// to an [`AppliedFix`] via
    /// [`AppliedFix::__engine_promote_text_correction`].
    // 9 args is the irreducible carrying capacity of a text-correction
    // diagnostic: id/severity/span/message/citation for the diagnostic
    // surface + replacement/source/confidence/migration_ref for the
    // engine's promotion path. Constructing this via a builder would
    // shift the same parameter count onto the builder's `.with_*`
    // methods without reducing it.
    #[allow(clippy::too_many_arguments)]
    pub fn text_correction(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: impl Into<Box<str>>,
        citation: &'static str,
        replacement: impl Into<SmolStr>,
        source: FixSource,
        confidence: Confidence,
        migration_ref: Option<&'static str>,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            candidate_span: None,
            message: message.into(),
            citation,
            fix: None,
            text_correction: Some(TextCorrection {
                replacement: replacement.into(),
                source,
                confidence,
                migration_ref,
            }),
        }
    }

    /// Construct an informational diagnostic that carries no fix
    /// payload.
    ///
    /// For rules that emit diagnostics at `Info`, `Warn`, `Error`,
    /// or `Suggest` without proposing a repair. Equivalent to
    /// [`Self::with_fix`] with `fix: None` but reads more cleanly
    /// at call sites.
    pub fn info(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: impl Into<Box<str>>,
        citation: &'static str,
    ) -> Self {
        Self::with_fix(rule, severity, span, message, citation, None)
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

    /// Dispatch phase for the engine's two-pass fix pipeline (FR-021).
    ///
    /// Returns [`Phase::WholeMarking`] by default. The default is
    /// **intentional, not accidental** — per PM decision D-7.2 in
    /// `docs/refactor-006/pr-7-pm-decisions.md`:
    ///
    /// - Most rules in the catalog are whole-marking by construction
    ///   (27 of 31 CAPCO rules at PR 7a; see `crates/capco/tests/phase_assignment.rs`
    ///   for the canonical per-rule list).
    /// - Failing to declare yields the safer dispatch: a localized rule
    ///   running in pass-2 is conservative (no I-19 false positive),
    ///   whereas a whole-marking rule running in pass-1 violates the
    ///   span-shape constraint and trips the PR 7b first-fire check.
    /// - Drift mitigation lives in `crates/capco/tests/phase_assignment.rs`,
    ///   which enumerates every registered rule's declared phase
    ///   against a hand-maintained allowlist. Adding a new rule
    ///   without considering phase forces an allowlist edit — a
    ///   "stop and think" gate without the per-rule boilerplate of a
    ///   required-method.
    ///
    /// PR 7a (this commit) stores the phase on the engine as a
    /// partition but does NOT yet dispatch on it; both phases still
    /// run together in pass-2 exactly as before. Pass-split dispatch
    /// lands in 7b. The default is forward-compatible with future
    /// schemes whose rules are `WholeMarking`-by-construction.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }

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

    // FixProposal-construction validation tests retired in
    // PR 3c.B Commit 10 (along with the FixProposal type itself).
    // Confidence's per-axis validate() is tested directly in
    // `confidence.rs`; FixIntent<S> construction is exercised in
    // `fix_intent.rs::tests`.
}
