// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::{Diagnostic, Phase, RuleContext, Severity};
use marque_ism::CanonicalAttrs;
use marque_scheme::MarkingScheme;

/// Unique rule identifier — a `(scheme, predicate_id)` 2-tuple.
///
/// `scheme` names the marking-scheme namespace the rule belongs to
/// (e.g., `"capco"` for CAPCO/ISM rules); `predicate_id` is a
/// dot-separated `<surface>.<category>.<predicate>` path identifying
/// the specific rule (e.g.,
/// `"portion.dissem.noforn-conflicts-rel-to"`). The canonical
/// wire-string form is `"<scheme>:<predicate_id>"` — produced by the
/// `Display` impl, consumed by `.marque.toml` `[rules]` keys, CLI text
/// output, and any caller that wants a single grep-friendly string.
/// JSON audit records serialize the structured 2-tuple shape, never
/// the wire string.
///
/// # Reserved schemes
///
/// Two `scheme` values are reserved by the engine itself and MUST NOT
/// be used by a `MarkingScheme` registration:
///
/// - `"engine"` — synthetic engine-minted diagnostics. The engine
///   uses `("engine", "recognition.decoder-recognized")` for the
///   decoder-recognition rewrite and `("engine", "fix.reparse-failed")`
///   for the post-pass-1 re-parse-failure sentinel. A future
///   engine-level sentinel adds a new `predicate_id` under the same
///   `"engine"` scheme — never a new scheme name. The
///   `("engine", _)` tuple is the cross-version anchor for these
///   diagnostics; consumers filter by `rule.scheme() == "engine"` to
///   surface only engine-internal records.
/// - `"test"` — test fixtures and synthetic identifiers for unit /
///   integration tests of the audit-record machinery (renderers,
///   sentinel checks, NDJSON serialization). Never reaches production
///   audit output by Constitution V's permitted-identifier rule.
///   Tests fabricate values like
///   `("test", "synthetic.r999-fixture")`.
///
/// Neither reserved scheme is a valid [`MarkingScheme`] registration
/// target. A scheme adapter (e.g., `CapcoScheme`) MUST pick a
/// distinct scheme name (`"capco"` today; `"cui"`, `"nato"` for
/// future schemes).
///
/// # Construction
///
/// Both fields are `&'static str` so construction is free and
/// `Copy`-able. There is exactly one constructor — [`RuleId::new`]
/// taking the two segments separately — by design (per the T044 PM
/// decisions, OD-6): a single 2-arg form makes the misuse of
/// confusing scheme with predicate unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId {
    scheme: &'static str,
    predicate_id: &'static str,
}

impl RuleId {
    /// Construct a rule identifier from a `(scheme, predicate_id)` pair.
    ///
    /// Both arguments are `&'static str`; the result is `Copy`. The
    /// constructor performs no validation — callers are responsible for
    /// picking a `scheme` distinct from the reserved `"engine"` and
    /// `"test"` namespaces (see the type-level doc).
    #[inline]
    pub const fn new(scheme: &'static str, predicate_id: &'static str) -> Self {
        Self {
            scheme,
            predicate_id,
        }
    }

    /// Return the scheme component of this rule identifier.
    ///
    /// For built-in CAPCO rules this is `"capco"`. For engine-minted
    /// diagnostics it is `"engine"`. For test fixtures it is `"test"`.
    #[inline]
    pub const fn scheme(&self) -> &'static str {
        self.scheme
    }

    /// Return the predicate-id component of this rule identifier.
    ///
    /// The predicate id is the dot-separated
    /// `<surface>.<category>.<predicate>` path identifying the
    /// specific rule within its scheme — e.g.,
    /// `"portion.dissem.noforn-conflicts-rel-to"` or
    /// `"fix.reparse-failed"`.
    #[inline]
    pub const fn predicate_id(&self) -> &'static str {
        self.predicate_id
    }
}

impl std::fmt::Display for RuleId {
    /// Render the canonical wire string: `"<scheme>:<predicate_id>"`.
    ///
    /// The colon separator is deliberate: slash collides with the
    /// existing `Constraint::Custom` catalog-row label convention; dot
    /// collides with the dotted segments inside `predicate_id` itself
    /// and would lose the scheme boundary at parse time.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.scheme, self.predicate_id)
    }
}
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
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext<'_>) -> Vec<Diagnostic<S>>;

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

    /// Whether the engine trusts this rule's `check()` to be panic-free.
    ///
    /// Default: `false`. Untrusted rules run inside `std::panic::catch_unwind`
    /// so a panicking rule degrades one document gracefully instead of
    /// aborting the run. Overriding to `true` is the deliberate opt-out from
    /// that containment — think of it like an `unsafe` block: a load-bearing
    /// statement that this rule has been audited for panic-safety and that
    /// any future change to `check()` carries the same obligation.
    ///
    /// The trust shortcut leans on the engine's stateless-rule contract
    /// (Constitution VI): `check()` must not mutate state visible across
    /// invocations. A trusted rule that violates that contract via interior
    /// mutability could observe torn invariants on a future panic — there
    /// is no `catch_unwind` to contain it. Auditing a rule for `trusted()`
    /// means auditing it for both panic-safety AND statelessness.
    ///
    /// In-tree rules override to `true` (audited as part of the catalog).
    /// Out-of-tree rules inherit safe-by-default.
    fn trusted(&self) -> bool {
        false
    }

    /// Authoritative-source citations this rule relies on and/or may emit.
    ///
    /// Covers two disjoint categories:
    ///
    /// - **Emitted citations** — every `Citation` the rule constructs
    ///   on `Diagnostic.citation` from its `check()` body. These
    ///   surface in `Engine::lint()` output and are harvestable from
    ///   the corpus.
    /// - **Non-emitted cross-reference pins** — §-citations the
    ///   rule's logic depends on but never surfaces as a
    ///   `Diagnostic.citation` (e.g. E005's `§D.1 p27` entry, which
    ///   authorizes the rule's class-floor predicate without being
    ///   emitted). These are load-bearing for Constitution VIII
    ///   audit traceability: a future maintainer reading the rule's
    ///   `cited_authorities()` MUST see every authoritative passage
    ///   the rule's correctness rests on, not only the ones the
    ///   diagnostic emits.
    ///
    /// Used by the PR 10.A.2 F.1 corpus-fidelity gate
    /// (`crates/capco/tests/citation_fidelity.rs`) to cross-check the
    /// declared catalog against what the engine actually emits over
    /// the corpus. The gate runs in both directions:
    ///
    /// 1. **Harvested ⊆ declared ∪ engine_emitted.** The harvested
    ///    set (`union(Diagnostic.citation)` across every fixture's
    ///    `Engine::lint()` output) MUST be a subset of the declared
    ///    set (catalog rows ∪ rule declarations), modulo a small
    ///    closed allow-list of engine-internal citations (R001 / R002).
    ///    Catches rules that emit citations they didn't declare.
    /// 2. **Declared ⊆ harvested ∪ whitelist.** The declared set
    ///    MUST be a subset of the harvested set, modulo a documented
    ///    `EXPECTED_UNCOVERED` whitelist
    ///    (`docs/refactor-006/citation-coverage-report.md`). The
    ///    whitelist exists *because* non-emitted cross-reference pins
    ///    are legitimate — each whitelist entry is anchor-tagged with
    ///    the reason the declared citation does not surface
    ///    (intentional cross-reference, advisory suppression, etc.).
    ///    A whitelist entry is not a bug; an *un-whitelisted* missing
    ///    citation IS.
    ///
    /// The default `&[]` is forward-compatible: rules that emit at
    /// most one `Diagnostic.citation` value matching their primary
    /// catalog row don't need to override, because their citation
    /// flows through the `Constraint`/`PageRewrite`/`ClosureRule`
    /// catalog declaration the bridge synthesizes.
    ///
    /// Override on every rule that:
    /// - constructs a `Citation` value directly inside `check()`
    ///   (the typical hand-written rule shape under
    ///   `crates/capco/src/rules.rs`), **or**
    /// - depends on a §-citation that authorizes its logic but is
    ///   never surfaced in any diagnostic (declare it here so the
    ///   audit chain is complete).
    ///
    /// Walker-style rules (e.g. `BannerMatchesProjectedRule`) MUST
    /// list the union of per-row citations emitted under any per-row
    /// `rule_id`, because the F.1 gate harvests by
    /// `Diagnostic.citation` not by `Rule::id()`.
    ///
    /// Constitution VIII (Authoritative Source Fidelity): every
    /// entry — emitted or non-emitted cross-reference — MUST trace
    /// to a real CAPCO-2016 passage (or the appropriate
    /// authoritative source for the rule's scheme) re-verified
    /// against the primary source at the point of declaration.
    fn cited_authorities(&self) -> &'static [marque_scheme::Citation] {
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
