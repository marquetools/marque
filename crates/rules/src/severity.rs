// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

/// Rule severity level. Configurable per rule in `.marque.toml`.
///
/// `Severity` lives in `marque-scheme` so
/// [`marque_scheme::constraint::ConstraintViolation`] and other
/// scheme-layer types can carry per-row severity without violating
/// Constitution VII's leaf-only rule for the scheme crate.
/// `marque_rules::Severity` is a re-export so import sites use either
/// path interchangeably.
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
/// emitted at `Suggest` may carry a candidate `FixIntent` in
/// `Diagnostic::fix`, but the engine will **never** auto-apply it,
/// regardless of `confidence`. The fix is informational — it tells
/// the user what the rule would suggest if confidence were higher.
/// Two paths produce `Suggest`-severity diagnostics:
///
/// 1. **Explicit emission**: a rule constructs the diagnostic with
///    `Severity::Suggest` directly. `S004 rel-to-trigraph-suggest`
///    is the first such rule.
/// 2. **Engine rewrite**: any diagnostic whose attached `FixIntent`
///    carries `confidence.combined() < confidence_threshold` is
///    rewritten to `Severity::Suggest` by the engine in `lint`. This
///    subsumes the prior silent-drop behavior at threshold-gate time
///    so below-threshold fixes stay observable.
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

/// Dispatch phase declared by each [`crate::Rule`] at registration. Drives the
/// engine's two-pass fix pipeline.
///
/// The phase is a rule-level promise about the span shape of every
/// `Diagnostic` the
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
/// the post-corrections buffer and applies their fixes, then
/// re-parses; pass-2 dispatches `Phase::WholeMarking` rules against the
/// post-pass-1 attrs (with the pre-pass-1 attrs cached for
/// two-pass-reshape disambiguation).
///
/// No `Phase::Both` escape hatch. A defect class that genuinely needs
/// detection in both phases registers two rule entries (one per phase)
/// sharing a backend module — see `docs/plans/2026-05-02-engine-refactor-consolidated.md`
/// §9.1 for the design rationale. The same rationale extends to
/// [`Phase::PageFinalization`] (issue #461): a rule that needs both a
/// per-marking pass and a page-level fixpoint pass registers two
/// entries, not a Phase::Both wildcard.
///
/// **`#[non_exhaustive]`** (issue #461): adding a future dispatch
/// phase (e.g., document-finalization once cross-page rules land)
/// should be a non-breaking change for downstream consumers. The
/// project is pre-1.0 with no published external rule crates today, so
/// the cost of adding it now is zero and the long-term option value
/// is high.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
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
    /// producing an unparseable buffer — handled by the reparse-failed
    /// path, but better avoided by construction.
    ///
    /// First-fire span-shape enforcement lives in `Engine::fix_inner`;
    /// a rule that misdeclares `Localized` and emits a wider span is
    /// dropped from pass-1 with a `tracing::error!`, not promoted to
    /// `AppliedFix`.
    Localized,
    /// `Diagnostic::span` (and `Diagnostic::candidate_span`, when
    /// populated) covers a portion, banner, or page scope. Examples:
    /// a banner roll-up walker, a class-floor walker, or any rule
    /// whose `FixIntent` carries `ReplacementIntent::FactAdd` /
    /// `FactRemove` / `Recanonicalize`. `Diagnostic::text_correction`
    /// is rare in this phase but follows the same span-shape contract
    /// when used. Pass-2 sees post-pass-1 attrs plus the pre-pass-1
    /// attrs cache for two-pass-reshape disambiguation.
    ///
    /// This is the default returned by [`crate::Rule::phase`] for rules that
    /// do not override the method (see [`crate::Rule::phase`]'s documentation
    /// for the design rationale).
    WholeMarking,
    /// Dispatched exactly once per page on the **closed** page-level
    /// fixpoint — at every scanner-emitted page-break boundary (BEFORE
    /// the per-page accumulator reset, see
    /// [`marque_ism::MarkingType::PageFinalization`]) and once at
    /// end-of-document. At dispatch time the engine has finished
    /// accumulating every portion's contribution to the page-level
    /// state, so a rule reading `ctx.page_portions` /
    /// `ctx.page_marking` sees the Knaster-Tarski fixpoint of the
    /// page-axis lattices (classification, SCI, SAR, AEA, dissem,
    /// REL TO, FGI marker), not an intermediate snapshot. This is the
    /// closure of issue #461.
    ///
    /// Both `ctx.page_portions` and `ctx.page_marking` are always
    /// populated on a PageFinalization dispatch (the engine
    /// force-initializes both Arcs from the live accumulator before
    /// invoking the rule); a defensive `.as_ref()?` early-return is
    /// nonetheless idiomatic so the rule stays safe under future
    /// engine refactors that might relax the invariant.
    ///
    /// **Triggering surface.** The engine synthesizes a single
    /// dispatch per `MarkingType::PageBreak` candidate (BEFORE the
    /// per-page accumulator reset, so the dispatched rules see the
    /// closing page) and one final dispatch at end-of-document
    /// covering any trailing portions that never reached a
    /// page-break. Empty pages (no portions) are skipped — there is
    /// no page-level fixpoint to observe.
    ///
    /// **`Diagnostic::span`.** The engine provides
    /// `ctx.candidate_span` as a zero-length anchor at the page-break
    /// byte offset (or `source.len()` at end-of-document). Today this
    /// is the only span a PageFinalization rule can produce: the
    /// per-page accumulator stores `[CanonicalAttrs]` only — no
    /// per-portion spans — so there is no way to recover a portion's
    /// own span from `ctx.page_portions`. Issue #461 chose not to
    /// extend the hot-path data type for a single Warn-severity
    /// diagnostic. Rules using the boundary anchor MUST document the
    /// limitation in their doc comment (W004 is the worked example).
    /// A future enhancement that adds spans to the accumulator or
    /// threads a portion-span lookup into `RuleContext` would let
    /// rules refine the anchor to the specific offending portion.
    ///
    /// **No-fix emission convention.** Rules in this phase today
    /// surface diagnostics with `fix: None` (W004 is the first
    /// consumer — the JOINT→FGI migration is renderer-canonical
    /// territory; see W004's doc comment for the trade-off rationale).
    /// A future PR that introduces a fixable PageFinalization rule
    /// will need to thread the synthetic boundary candidate through
    /// the existing two-pass fix pipeline. The naming
    /// (`TwoPassFixer`) reflects fix-application passes — pass-1
    /// Localized splice → re-parse → pass-2 WholeMarking apply_intent
    /// — and stays accurate: PageFinalization rules ride pass-2 at
    /// fix-time if they ever produce fixes.
    ///
    /// Issue #461 introduces this phase. The §9.1 "no Phase::Both
    /// escape hatch" rationale (above) extends here: a rule needing
    /// both a per-marking pass and a page-level pass registers two
    /// entries.
    PageFinalization,
}
