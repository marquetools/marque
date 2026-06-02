// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Engine` — the configured, ready-to-run pipeline.

use crate::clock::{Clock, SystemClock};
use crate::decoder::StrictOrDecoderRecognizer;
use crate::errors::{EngineConstructionError, EngineError};
use crate::options::{FixOptions, LintOptions};
use crate::output::{FixResult, LintResult};
use crate::recognizer::StrictRecognizer;
use crate::scheduler::{ScheduledStep, project_rewrites, schedule_steps, validate_intent_rewrites};
use crate::text_correction::{SynthesizedFix, TextCorrectionProposal};
use aho_corasick::AhoCorasick;
use marque_capco::CapcoScheme;
use marque_config::Config;
use marque_rules::audit::{AppliedTextCorrection, AuditLine};
use marque_rules::{
    CORRECTIONS_MAP_CITATION, ConstraintBridge, Diagnostic, EnginePromotionToken, FixSource, Phase,
    RuleId, RuleSet, Severity, SmallVec,
};
use marque_scheme::Span;
use marque_scheme::ambiguity::Parsed;
use marque_scheme::canonical::{Canonical, CanonicalConstructor, EngineConstructor};
use marque_scheme::category::CategoryId;
use marque_scheme::recognizer::{ParseContext, Recognizer};
use marque_scheme::scope::Scope;
use marque_scheme::{MarkingScheme, RewriteId};
use secrecy::{SecretBox, SecretSlice};
use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use zeroize::Zeroizing;
// See note in `options.rs` — `web_time::Instant` is `std::time::Instant`
// on native and a Performance.now() polyfill on wasm32-unknown-unknown.
use web_time::Instant;

/// Cooperative-cancellation predicate (spec 005 §R3). Centralizing this
/// in one helper keeps the wall-clock comparison consistent across every
/// deadline check site (`lint_with_options` pre-pass, per-candidate,
/// `fix_inner` post-lint, per-fix-application). The predicate is `now >=
/// deadline`, so a deadline equal to the current `Instant` triggers
/// cancellation — the spec's "expired" semantics.
#[inline]
fn deadline_expired(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|d| Instant::now() >= d)
}

/// Window radius (bytes on each side of the candidate) inspected by the
/// surrounding-lowercase majority check. 64 bytes covers the local
/// sentence/clause context — enough to distinguish lowercase prose from
/// an uppercase banner zone, without the cost of scanning the whole
/// document for every candidate.
const LOWERCASE_WINDOW_RADIUS: usize = 64;

/// Minimum lowercase-letter count required before the lowercase-majority
/// flag can trip. Prevents a noise majority on very short windows (e.g.,
/// a candidate at the very start or end of a tiny document where the
/// window has only a handful of letters total).
const LOWERCASE_MIN_COUNT: usize = 3;

/// Whether the ASCII-letter content of the source bytes within
/// [`LOWERCASE_WINDOW_RADIUS`] before and after `[start, end)` is
/// lowercase-dominant.
///
/// `true` only when lowercase letters outnumber uppercase ASCII letters
/// AND at least [`LOWERCASE_MIN_COUNT`] lowercase letters were seen.
/// The double gate keeps tiny windows (sub-3 lowercase letters total)
/// from producing a noise positive; uppercase-dominant banner/header
/// zones return `false` so legitimate uppercase markings recovered from
/// uppercase context aren't penalized.
fn surrounding_lowercase_majority(source: &[u8], start: usize, end: usize) -> bool {
    // Bounds-safe even when caller hands us a malformed span with
    // `start > source.len()` (e.g., a scanner regression). Every
    // index expression below is clamped to `source.len()` BEFORE
    // the slice operation so an inverted range (e.g., `lo_start =
    // 936` from `start - LOWERCASE_WINDOW_RADIUS` paired with
    // `source.len() = 100`) is impossible. The result on a
    // degenerate span is an empty window → `false` rather than a
    // panic. This is what allows the caller in `lint_inner` to
    // invoke us BEFORE its own `candidate.span.start
    // .min(source.len())` clamp without re-introducing a possible
    // out-of-bounds index.
    let start_clamped = start.min(source.len());
    let end_clamped = end.min(source.len());
    let lo_start = start_clamped.saturating_sub(LOWERCASE_WINDOW_RADIUS);
    let hi_end = end_clamped
        .saturating_add(LOWERCASE_WINDOW_RADIUS)
        .min(source.len());
    let lo_slice = &source[lo_start..start_clamped];
    let hi_slice = &source[end_clamped..hi_end];
    let mut lowercase = 0usize;
    let mut uppercase = 0usize;
    for &b in lo_slice.iter().chain(hi_slice.iter()) {
        if b.is_ascii_lowercase() {
            lowercase += 1;
        } else if b.is_ascii_uppercase() {
            uppercase += 1;
        }
    }
    lowercase >= LOWERCASE_MIN_COUNT && lowercase > uppercase
}

/// Synthetic rule identifier for the post-pass-1 re-parse-failure
/// sentinel. Emitted when the post-pass-1 buffer fails to re-parse —
/// pass-1 produced ≥1 applied fix that turned the source into an
/// unparseable shape, so pass-2 is skipped and the engine returns the
/// pass-1 buffer + this diagnostic carrying the contributing pass-1
/// rule IDs. The `"engine"` scheme is the reserved namespace for
/// engine-minted diagnostics; the predicate id describes the failure
/// mode in plain English.
pub const R002_RULE_ID: RuleId = RuleId::new("engine", "fix.reparse-failed");

/// Typed [`Citation`](marque_scheme::Citation) attached to `R002`
/// diagnostics — the synthetic re-parse-failure sentinel has no CAPCO
/// §-citation by construction (Constitution VIII requires a real
/// passage; R002 is engine-internal guidance, not a CAPCO rule). Uses
/// [`marque_scheme::AuthoritativeSource::EngineInternal`]. Display
/// renders as `[engine-internal]`.
const R002_CITATION_TYPED: marque_scheme::Citation = marque_scheme::Citation::new(
    marque_scheme::AuthoritativeSource::EngineInternal,
    marque_scheme::SectionRef::new(marque_scheme::SectionLetter::A),
    // Niche-sentinel page value — never rendered (Display elides
    // section/page when source is non-CAPCO).
    match core::num::NonZeroU16::new(1) {
        Some(n) => n,
        None => unreachable!(),
    },
);

/// Default capacity for the per-page portion accumulator
/// (`Engine::lint_inner`'s `page_portions: Vec<CanonicalAttrs>`).
/// Sized to the typical CAPCO per-page portion count: the scanner
/// emits `MarkingType::PageBreak` candidates at form-feed and at
/// `\n\n\n+` runs, slicing larger docs into multiple per-page
/// contexts, so 8 covers the typical 1-10-portion case in zero
/// reallocations. Larger pages pay only the reallocations needed
/// past 8 instead of the early growth sequence a `Vec::new()` path
/// would incur on the first several pushes.
///
/// Owned at the engine accumulator (issue #430).
pub(crate) const DEFAULT_PORTIONS_CAPACITY: usize = 8;

/// Construct a fresh per-page portion accumulator pre-sized to
/// [`DEFAULT_PORTIONS_CAPACITY`].
///
/// Centralized so the lint-loop startup site and the `MarkingType::PageBreak`
/// reset site cannot drift apart. If a future edit replaces this body with
/// `Vec::new()`, multi-page documents would pay the `Vec` growth sequence
/// (capacity doubling: 0→4→8→16…) on the first several `add_portion` pushes
/// of every page — silent perf regression that no functional test catches.
/// The `fresh_accumulator_uses_default_capacity` unit test below pins the
/// capacity contract.
#[inline]
pub(crate) fn fresh_page_portions_accumulator<S: MarkingScheme>() -> Vec<S::Canonical> {
    Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY)
}

/// Whether to apply fixes or just simulate (dry-run).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixMode {
    /// Apply fixes to the source text.
    Apply,
    /// Simulate fixes — audit stream is identical but source is unchanged.
    DryRun,
}

/// Error returned when a caller supplies a runtime confidence threshold
/// override that is outside the valid `[0.0, 1.0]` range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidThreshold(pub f32);

impl std::fmt::Display for InvalidThreshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "confidence threshold {} is outside [0.0, 1.0] or is NaN",
            self.0
        )
    }
}

impl std::error::Error for InvalidThreshold {}

/// Marker trait combining [`marque_scheme::DecisionSink`] with the
/// `Send + Sync` bounds the engine needs to store a boxed sink behind
/// a `Mutex` without requiring those bounds at the [`marque_scheme`]
/// trait level (Phase C of the decision-tracing pipeline).
///
/// The blanket impl makes every `Send + Sync` `DecisionSink` (the
/// built-in [`NoopSink`](marque_scheme::NoopSink),
/// [`CountingSink`](marque_scheme::CountingSink),
/// [`RecordingSink`](marque_scheme::RecordingSink), plus any
/// downstream sink that holds only `Send + Sync` data) usable as
/// [`Engine::with_decision_sink`] input without any additional
/// declaration on the sink side.
///
/// Only compiled when the `decision-tracing` Cargo feature is on. With
/// the feature off, the engine carries no sink field and this trait
/// is unused.
#[cfg(feature = "decision-tracing")]
pub trait SyncDecisionSink: marque_scheme::DecisionSink + Send + Sync {}

#[cfg(feature = "decision-tracing")]
impl<T: marque_scheme::DecisionSink + Send + Sync> SyncDecisionSink for T {}

/// The CAPCO instantiation of [`Engine`] — `Engine<CapcoScheme, EngineRecognizer>`
/// spelled out.
///
/// [`Engine`] has no type-parameter defaults: the core engine type is
/// domain-neutral, so the CAPCO-ness must be named. This alias is the
/// canonical name for the CAPCO instantiation — write `CapcoEngine`
/// wherever the engine type appears in a CAPCO context (`-> CapcoEngine`,
/// `Arc<CapcoEngine>`, `let e: CapcoEngine`). `Engine::new` and the other
/// CAPCO conveniences resolve through it without naming the params.
pub type CapcoEngine = Engine<CapcoScheme, EngineRecognizer>;

/// A configured engine instance, generic over the marking scheme `S` and
/// recognizer `R`.
///
/// Neither parameter is defaulted — the engine core is domain-neutral, so
/// a CAPCO engine names its scheme via the [`CapcoEngine`] alias (or
/// `Engine<CapcoScheme, EngineRecognizer>` spelled out). `R` is the
/// recognizer type; for CAPCO it is [`EngineRecognizer`], the opaque
/// CAPCO-bound recognizer that keeps default dispatch monomorphized
/// (strict-first with a decoder fallback) while preserving a trait-object
/// escape hatch. `R` exists so a second scheme can drive its own
/// recognizer through the same `Engine` core.
pub struct Engine<S: MarkingScheme, R: Recognizer<S>> {
    config: Config,
    rule_sets: Vec<Box<dyn RuleSet<S>>>,
    /// Scheme catalog held for constraint-bridge dispatch in
    /// `lint_inner`. Stores the scheme passed to the constructor:
    /// [`Engine::with_clock_and_recognizer`] takes an arbitrary
    /// `scheme: S` and stores it directly, and the CAPCO conveniences
    /// ([`Engine::new`] / [`Engine::with_clock`]) pass the caller's
    /// `CapcoScheme` through unchanged. The constraint-catalog bridge in
    /// `lint_inner` fires against this stored scheme's catalog.
    ///
    /// # Bridge diagnostic population
    ///
    /// The engine bridge uses row names from the `Constraint` catalog
    /// to populate `Diagnostic.rule`. The bridge is a **no-op
    /// pass-through**: the catalog row's `constraint_label` IS the
    /// predicate id; the bridge constructs the `RuleId` from
    /// [`MarkingScheme::constraint_rule_id`], whose default namespaces
    /// the predicate under the scheme's own `scheme_id()` (`"capco"` for
    /// `CapcoScheme`) with no string manipulation. Every catalog row
    /// gets its own predicate id at the row level.
    ///
    /// The bridge code lives in
    /// [`Engine::bridge_constraint_diagnostic`].
    scheme: S,
    clock: Box<dyn Clock>,
    /// Corrections map wrapped in Arc once at construction time so that each
    /// `RuleContext` clone in `lint()` is an O(1) refcount bump, not a
    /// deep-clone of the entire HashMap.
    corrections_arc: Option<Arc<HashMap<String, String>>>,
    /// Pre-built Aho-Corasick automaton for pre-scanner text corrections.
    /// Built once at construction time from the corrections map (excluding
    /// no-op and "//" entries). `None` when the corrections map is empty or
    /// all entries are filtered out.
    corrections_ac: Option<CachedAhoCorasick>,
    /// Topologically-sorted rewrite ids, computed once at construction
    /// time from the scheme's `page_rewrites()` declaration. The order
    /// satisfies: for every edge `a → b` (rewrite `a` writes a
    /// category `b` reads), `a` appears before `b`. When dataflow
    /// edges fully determine the order, the rewrite order is
    /// independent of declaration order; when two rewrites have no edge
    /// between them, the scheduler breaks the tie by declaration
    /// order (Kahn's algorithm seeded in declaration order). Empty
    /// when the scheme declares no rewrites.
    ///
    /// Derived from [`Self::scheduled_steps`] by projecting out the
    /// page-rewrite steps, so the two orders cannot disagree.
    scheduled_rewrites: Box<[RewriteId]>,
    /// Topologically-sorted union of page-rewrite and derivation-edge
    /// steps, computed once at construction time from the scheme's
    /// `page_rewrites()` and `derivation_edges()` declarations. The
    /// combined order is the schedule downstream document-scope
    /// resolution consumes; the cycle and ambiguous-co-writer guards
    /// covering it run once at construction. For a scheme that declares
    /// no edges this is the same sequence as `scheduled_rewrites`,
    /// tagged as `PageRewrite` steps.
    scheduled_steps: Box<[ScheduledStep]>,
    /// The engine modes currently active. A derivation edge whose firing
    /// predicate is [`FiringPredicate::WhenMode`](marque_scheme::FiringPredicate::WhenMode)
    /// fires during document-scope resolution only when its mode label is
    /// in this set; [`FiringPredicate::Always`](marque_scheme::FiringPredicate::Always)
    /// edges always fire (issue #799). Empty by default — no public
    /// mode-setter ships yet; the mode taxonomy arrives in a later phase
    /// (#645). The set is consulted by `firing_active` and never mutated
    /// after construction outside the crate-internal test setter.
    active_modes: std::collections::BTreeSet<&'static str>,
    /// Recognizer used by `lint()` to resolve each scanner candidate to
    /// the scheme's canonical form. Typed as the generic `R`; for CAPCO it
    /// is [`EngineRecognizer`] — the opaque CAPCO-bound recognizer that
    /// keeps default dispatch monomorphized (strict-first with a decoder
    /// fallback) while preserving the `with_recognizer(Arc<dyn
    /// Recognizer<_>>)` trait-object escape hatch. A [`CapcoEngine`] stores
    /// an `EngineRecognizer`; the generic parameter opens the seam for a
    /// second scheme's recognizer.
    ///
    /// Installed by [`Engine::new`]: [`StrictOrDecoderRecognizer`] —
    /// strict-first dispatch with a decoder fallback on strict-parse
    /// zero-candidate (the field carries no type-parameter default; this
    /// is the recognizer the CAPCO constructor supplies). The
    /// decoder recovers mangled markings that are edit-distance-1/2,
    /// token-reordered, superseded, or case-mangled from a real
    /// CAPCO-2016 marking. Live-typing surfaces concerned with
    /// per-keystroke latency are expected to debounce their calls into
    /// the engine; surfaces that need to pin strict-only behavior (the
    /// interactive-latency benchmark, tests asserting strict
    /// dispatch) should call [`Engine::with_strict_recognizer`].
    recognizer: R,

    /// CLI-supplied corpus override. Held only behind the
    /// `corpus-override` Cargo feature so the WASM artifact and the
    /// `marque-server` build cannot accidentally accept one through any
    /// code path.
    ///
    /// The decoder does not yet substitute these priors into scoring —
    /// the surface is wired end-to-end and every decoder fix is stamped
    /// with [`marque_rules::FeatureId::CorpusOverrideInEffect`] in the
    /// audit record so an auditor can identify fixes produced under
    /// organizational overrides vs. stock priors. The prior-substitution
    /// wiring is not yet done; this field is the seam.
    #[cfg(feature = "corpus-override")]
    corpus_override: Option<std::sync::Arc<marque_config::corpus_override::CorpusOverride>>,

    /// Phase partition of the registered rule set, computed once at
    /// construction time. Each entry is a
    /// `(rule_set_index, rule_index_within_set)` pair indexing back into
    /// `self.rule_sets[i].rules()[j]`. `pass1_rule_indices` lists every
    /// rule whose `phase()` returned [`Phase::Localized`];
    /// `pass2_rule_indices` lists every rule whose `phase()` returned
    /// [`Phase::WholeMarking`]. Together they enumerate every registered
    /// rule exactly once.
    ///
    /// **Inline-size choice.** `[(usize, usize); 4]` for pass-1
    /// (Localized rules are rare — a handful in the CAPCO ruleset) and
    /// `[(usize, usize); 32]` for pass-2. With ~27 WholeMarking rules
    /// today and an inline capacity of 32, the partition has headroom
    /// before the SmallVec spills to the heap at the 33rd entry. The
    /// canonical per-rule list lives in
    /// `crates/capco/tests/phase_assignment.rs`. Inline storage means no
    /// extra heap allocation in the common case — the partitions live
    /// wherever the `Engine` itself does.
    ///
    /// **Current consumer.** Read by
    /// [`TwoPassFixer::localized_rule_id_set`] to build the
    /// `(Localized rule id) → pass-1` lookup set that drives the
    /// per-document phase dispatch in `TwoPassFixer::run`. Stable for
    /// the lifetime of the engine.
    pass1_rule_indices: dispatch::Pass1Indices,
    /// Pass-2 (WholeMarking) partition counterpart of
    /// [`Engine::pass1_rule_indices`].
    ///
    /// Stored but not yet read at dispatch time. Pass-2 in
    /// `TwoPassFixer::run` routes diagnostics as the **complement** of
    /// the pass-1 (Localized) set via `partition_diags_by_phase` —
    /// sufficient for today's rule shape because every diagnostic
    /// emitted by `lint()` comes from a registered rule, so the
    /// complement equals the WholeMarking partition. The field stays
    /// available for a future change that wants the symmetry with
    /// pass-1 and the "unregistered emitted ID falls into neither pass"
    /// property. See [`Engine::pass1_rule_indices`] for the shape
    /// rationale.
    #[allow(dead_code)]
    pass2_rule_indices: dispatch::Pass2Indices,
    /// PageFinalization rule partition (issue #461) — read by
    /// `dispatch_page_finalization` at every scanner-emitted
    /// `MarkingType::PageBreak` (BEFORE the PageContext reset) and
    /// once at end-of-document. Each entry is a
    /// `(rule_set_index, rule_index_within_set)` pair indexing back
    /// into `self.rule_sets[set_idx].rules()[rule_idx]`.
    ///
    /// Today's consumers are W004 `joint-disunity-collapse` (issue
    /// #461; fires on the page-level fixpoint snapshot of the
    /// classification axis) and S005 `rel-to-opaque-uncertain-reduction`
    /// (issue #488; fires on the page-level fixpoint snapshot of the
    /// REL TO axis — pre-#488 the rule was Banner-gated under
    /// `Phase::WholeMarking` and missed banner-less layouts). Future
    /// PageFinalization rules (S007 and `BannerMatchesProjectedRule`
    /// migrations are scheduled follow-ups) will appear here without
    /// altering the dispatch structure. The partition is read at lint
    /// time (via `dispatch_page_finalization`); none of today's
    /// PageFinalization rules emit a `FixProposal`, so fix-time
    /// pass-2 does not yet need to consult this field. When the
    /// first fixable PageFinalization rule lands, the `TwoPassFixer`
    /// will need a matching pass-2 read site here.
    pass_finalization_rule_indices: dispatch::PassFinalizationIndices,

    /// Pre-resolved severity for every registered rule's *registered* ID,
    /// indexed outer-by-rule-set, inner-by-rule-index-within-set. Built
    /// once at construction time by [`build_severity_tables`] and read
    /// from the lint hot loop (Site A — fast-path Off-skip) instead of
    /// the per-candidate `config.rules.overrides` HashMap probe + per-
    /// candidate `Severity::parse_config` parse.
    ///
    /// **Population.** For each registered rule, the entry is
    /// `overrides.get(rule.id().predicate_id()).and_then(parse_config)
    /// .unwrap_or(rule.default_severity())` — preserving the pre-hoist
    /// semantics exactly. Walker rules (those with non-empty
    /// `additional_emitted_ids()`) get an entry too; Site A's
    /// `additional_emitted_ids().is_empty()` guard means the entry is
    /// populated-but-unread for walkers. The per-emitted-id path
    /// (Site B — `diags.retain_mut`) handles walker rule overrides
    /// against [`Engine::emitted_id_overrides`] instead.
    ///
    /// **Invariant.** Built once in [`Engine::with_clock`] post-
    /// `canonicalize_rule_overrides`; the backing slice is never
    /// mutated after construction. Indices match
    /// [`Engine::pass1_rule_indices`] / [`Engine::pass2_rule_indices`]
    /// — the same `(set_idx, rule_idx)` pair that addresses
    /// `self.rule_sets[set_idx].rules()[rule_idx]` addresses
    /// `self.fast_path_severities[set_idx][rule_idx]`.
    fast_path_severities: dispatch::FastPathSeverities,

    /// Pre-resolved per-emitted-ID severity overrides. Keys are the
    /// `&'static str` rule-ID slices carried by [`RuleId`]; values are
    /// the user-configured [`Severity`] resolved through
    /// [`Severity::parse_config`]. **Absence means "no override"** —
    /// callers preserve the diagnostic's emitted severity unchanged
    /// (which for non-walker rules matches `rule.default_severity()`
    /// by convention; for walker rules carries the per-row catalog
    /// severity, e.g. `Fix` for E031 vs `Error` for E035 / E040).
    ///
    /// **Population.** Built once in [`Engine::with_clock`] from
    /// `config.rules.overrides` post-`canonicalize_rule_overrides`.
    /// Every canonical override key that parses to a non-malformed
    /// severity AND has a known `&'static str` intern (from the
    /// registered rules' `id()` / `additional_emitted_ids()` plus the
    /// bridge's `bridge_emitted_rule_ids()`) is inserted. Unknown keys
    /// would have been caught by `canonicalize_rule_overrides`'s
    /// hard-fail; malformed severity strings are silently skipped to
    /// preserve the pre-hoist `.and_then(parse_config)` semantics
    /// (`build_severity_tables_skips_unparsable_severity` pins this).
    ///
    /// **Hot-loop consumers.** Read by the per-diagnostic `retain_mut`
    /// rewrite, the bridge `ConstraintViolation` envelope, and the
    /// corrections-map post-pass. This field handles the
    /// construction-time part of the optimization by precomputing
    /// emitted-ID override severities once, so hot paths avoid repeated
    /// parse/canonicalization work. Per-row severity overrides for the
    /// SCI per-system catalog flow directly through this map — each
    /// catalog row's `name` is its own predicate ID and is
    /// independently overridable via
    /// `[rules] "capco:marking.sci.<row>" = "<severity>"`; the bridge
    /// dispatches per-row in `bridge_sci_per_system_diagnostics`.
    emitted_id_overrides: dispatch::EmittedIdOverrides,

    /// Boxed [`DecisionSink`] threaded through every instrumented
    /// engine decision point (Phase C of the decision-tracing
    /// pipeline).
    ///
    /// Defaults to [`marque_scheme::NoopSink`] when no caller supplies
    /// one via [`Engine::with_decision_sink`]. The `Mutex` is the
    /// thread-safety boundary: `Engine::lint` takes `&self`, so the
    /// sink needs interior mutability AND `Engine: Send + Sync` must
    /// hold for the `BatchEngine` path. The lock contention is
    /// trivial — at most one sink event per microsecond in the worst
    /// case, no callers wait — and pays for the broader API surface
    /// (any `Send + Sync` `DecisionSink` is usable without changing
    /// the trait's bound shape in `marque-scheme`).
    ///
    /// Only present when the `decision-tracing` Cargo feature is on.
    /// With the feature off the field doesn't exist and every
    /// emission site is `#[cfg(feature = "decision-tracing")]`-gated,
    /// so the engine's per-document hot path carries no extra
    /// branches.
    #[cfg(feature = "decision-tracing")]
    sink: std::sync::Mutex<Box<dyn SyncDecisionSink>>,

    /// `true` once a non-Noop sink is installed via
    /// [`Engine::with_decision_sink`]; `false` while the engine carries
    /// its default [`marque_scheme::NoopSink`].
    ///
    /// This is the hot-path gate that keeps the feature-ON / no-observer
    /// build within the SC-001 latency floor. Every emission site is
    /// already `#[cfg(feature = "decision-tracing")]`-compiled-out on the
    /// OFF-feature build, but on the ON-feature build the default sink is
    /// still a boxed trait object behind a `Mutex`, so a naive `emit()`
    /// would pay an `AtomicU32::fetch_add` + `Mutex::lock` + vtable call
    /// per decision point even when no observer is listening. Gating
    /// [`Engine::emit`] and the scheme-side projection routing on this
    /// flag makes the no-observer path a single predictable-branch `bool`
    /// read, so wiring the feature in but leaving it off stays free.
    ///
    /// Set once at construction (`false` in [`Engine::new`], flipped to
    /// `true` by [`Engine::with_decision_sink`]) and never mutated after
    /// the engine is shared, so a plain `bool` is `Sync`-clean — no atomic
    /// needed.
    ///
    /// Only present when the `decision-tracing` Cargo feature is on.
    #[cfg(feature = "decision-tracing")]
    tracing_active: bool,

    /// Monotone per-document step counter assigned by [`Engine::emit`].
    /// Used by [`marque_scheme::DecisionEvent::triggered_by`] consumers
    /// (e.g., [`marque_scheme::RecordingSink::into_report`]) to
    /// reconstruct cascade chains.
    ///
    /// Atomic so the increment is `Send + Sync`-clean without
    /// double-locking the sink mutex; `Ordering::Relaxed` is
    /// sufficient because step ordering is established by the engine's
    /// single-threaded per-document execution (sink events from one
    /// document never race against another document's events).
    ///
    /// Only present when the `decision-tracing` Cargo feature is on.
    #[cfg(feature = "decision-tracing")]
    next_step: std::sync::atomic::AtomicU32,
}

#[cfg(feature = "decision-tracing")]
impl<S: MarkingScheme, R: Recognizer<S>> Engine<S, R> {
    /// Mint the next monotone step counter and record one
    /// [`DecisionEvent`](marque_scheme::DecisionEvent) on the engine's
    /// sink.
    ///
    /// The closure receives the freshly-minted `step` and returns the
    /// constructed event. Wrapping construction in a closure keeps the
    /// per-emission cost off the OFF-feature build entirely — every
    /// call site is `#[cfg(feature = "decision-tracing")]`-gated so the
    /// closure body never compiles into a release artifact without the
    /// feature.
    ///
    /// Lock contention on the sink mutex is negligible: per-document
    /// evaluation is single-threaded, so the only path that could
    /// contend is a hypothetical `BatchEngine` worker sharing the
    /// engine across documents — and the worker pool holds one engine
    /// per worker today. Poison from a panic-while-holding is treated
    /// as a no-op record (the engine drops the event); the panic
    /// itself surfaces through the caller's normal unwind path.
    #[inline]
    pub(crate) fn emit(&self, ev_builder: impl FnOnce(u32) -> marque_scheme::DecisionEvent) {
        // No observer installed: skip the step-counter `fetch_add`, the
        // `Mutex::lock`, and the vtable call entirely. This is the
        // feature-ON / default-`NoopSink` hot path; the branch is a
        // single `bool` read that predicts taken, so the cost collapses
        // to the no-feature path (`scripts/bench-check.sh`
        // `check_decision_tracing_overhead`'s absolute SC-001 ceiling).
        if !self.tracing_active {
            return;
        }
        let step = self
            .next_step
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let event = ev_builder(step);
        if let Ok(mut sink) = self.sink.lock() {
            sink.record(event);
        }
    }

    /// Whether a non-Noop [`DecisionSink`](marque_scheme::DecisionSink)
    /// is installed. Call sites that would otherwise route a real value
    /// (e.g. the per-page projection) through the sink-aware path use
    /// this to fall back to the plain, lock-free path when no observer
    /// is listening — keeping the feature-ON / no-observer build on the
    /// same code path as the OFF-feature build.
    #[inline]
    pub(crate) fn tracing_active(&self) -> bool {
        self.tracing_active
    }

    /// Run `body` with the engine's sink locked so a Phase D scheme-
    /// side call (`CapcoScheme::project_with_sink` /
    /// `closure_with_sink` / `project_from_attrs_slice_with_sink`) can
    /// thread the sink through projection-stage event emission.
    ///
    /// The closure receives `&mut dyn DecisionSink` (the trait object
    /// behind the boxed sink). The lock is held for the entire `body`
    /// invocation, which is acceptable because per-document
    /// evaluation is single-threaded and no other code path tries to
    /// acquire the sink lock concurrently.
    ///
    /// Poisoned-mutex handling: if the lock is poisoned, `body` is
    /// invoked against a transient `NoopSink` — the projection still
    /// runs; events for this document are silently dropped. This
    /// matches OFF-feature semantics and avoids unwinding into Tower
    /// middleware per Constitution VI.
    #[inline]
    pub(crate) fn with_sink<T>(
        &self,
        body: impl FnOnce(&mut dyn marque_scheme::DecisionSink) -> T,
    ) -> T {
        match self.sink.lock() {
            Ok(mut guard) => body(&mut **guard),
            Err(_poisoned) => {
                let mut noop = marque_scheme::NoopSink;
                body(&mut noop)
            }
        }
    }

    /// Record one [`DecisionEvent`](marque_scheme::DecisionEvent) per
    /// firing document-derivation edge, threading the cascade through
    /// [`triggered_by`](marque_scheme::DecisionEvent::triggered_by).
    ///
    /// `firing_edges` arrives in scheduled order (writers before readers),
    /// so a numeric `max()` over the last-writer steps of an edge's read
    /// categories is the most-recently-arriving dependency. That single
    /// parent makes the recorded cascade a spanning tree of the derivation
    /// DAG: every edge still emits its own firing record, but an edge that
    /// reads from several writers attributes to the latest one only.
    ///
    /// The step counter is the document's running monotone sequence (no
    /// reset here — resolution runs at end-of-document after every
    /// per-portion / per-page emit, so a reset would orphan earlier steps).
    /// `last_writer` is per-invocation scratch (Constitution VI).
    #[inline]
    fn record_derivation_cascade(&self, firing_edges: &[&marque_scheme::DerivationEdge]) {
        use marque_scheme::category::CategoryId;
        use marque_scheme::{DecisionEvent, DecisionKind, DecisionSite, DecisionSource};

        if !self.tracing_active() {
            return;
        }
        let mut last_writer: std::collections::HashMap<CategoryId, u32> =
            std::collections::HashMap::new();
        self.with_sink(|sink| {
            for edge in firing_edges {
                // Trigger = most-recent edge that wrote a category THIS edge
                // reads. Computed BEFORE inserting this edge's own writes, so
                // an edge that both reads and writes a category never points
                // at itself.
                let triggered_by = edge
                    .reads
                    .iter()
                    .filter_map(|c| last_writer.get(c).copied())
                    .max();
                let step = self
                    .next_step
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                // A single-write edge names its category; an edge writing zero
                // or several categories uses the `MARKING` multi-category
                // sentinel so per-category aggregations are not misattributed.
                let category = match edge.writes {
                    [c] => *c,
                    _ => CategoryId::MARKING,
                };
                sink.record(DecisionEvent {
                    step,
                    site: DecisionSite::Document,
                    category,
                    kind: DecisionKind::Derived,
                    source: DecisionSource::Derivation(edge.id),
                    triggered_by,
                });
                for &w in edge.writes {
                    last_writer.insert(w, step);
                }
            }
        });
    }

    /// Reset the per-document step counter to zero.
    ///
    /// Called at the top of every public lint entry point so that
    /// step IDs and `triggered_by` references resolve correctly
    /// within a single document. Without the reset a long-lived
    /// engine would emit monotonically growing step IDs across
    /// documents, breaking [`marque_scheme::RecordingSink::into_report`]'s
    /// cascade-chain reconstruction (which assumes step IDs index
    /// into the current document's event stream).
    ///
    /// `Ordering::Relaxed` is sufficient: per-document evaluation is
    /// single-threaded, so the reset only needs to be visible to the
    /// subsequent `emit` calls on the same thread, which is guaranteed
    /// by program order.
    #[inline]
    pub(crate) fn reset_decision_step_counter(&self) {
        self.next_step
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Run `body` with a step-remapping sink wrapping the engine's
    /// real sink.
    ///
    /// Scheme-side `project_with_sink` / `closure_with_sink`
    /// overrides emit events with a **local** step counter starting
    /// at zero (the trait signature receives `&mut dyn DecisionSink`
    /// and has no visibility into the engine's per-document
    /// `next_step` counter). Without remapping, those local IDs
    /// would collide with engine-assigned IDs in the merged stream,
    /// breaking `RecordingSink::into_report` cascade reconstruction.
    ///
    /// `StepRemappingSink` translates each scheme-emitted event by
    /// minting a fresh global step via `Engine::next_step` and
    /// looking up the event's `triggered_by` (if any) in a per-call
    /// `local → global` map so scheme-internal cascade edges
    /// continue to resolve correctly after remapping. The map is
    /// dropped when `body` returns.
    #[inline]
    pub(crate) fn with_remapping_sink<T>(
        &self,
        body: impl FnOnce(&mut StepRemappingSink<'_>) -> T,
    ) -> T {
        self.with_sink(|inner| {
            let mut remapper = StepRemappingSink {
                inner,
                next_step: &self.next_step,
                local_to_global: std::collections::HashMap::new(),
            };
            body(&mut remapper)
        })
    }
}

/// Adapter that translates scheme-emitted [`marque_scheme::DecisionEvent`]s
/// into the engine's global step-ID space before forwarding to the
/// real sink.
///
/// The scheme-side `project_with_sink` / `closure_with_sink` overrides
/// use a local counter starting at zero (the trait carries no global
/// step-minter handle). This adapter:
///
/// 1. Mints a fresh global step from
///    [`Engine::next_step`](Engine::emit) for each event the scheme
///    emits.
/// 2. Records `local → global` in a per-call `HashMap` so subsequent
///    events whose `triggered_by` references an earlier local step
///    resolve to the right global parent.
/// 3. Forwards the rewritten event to the wrapped real sink.
///
/// The adapter is created and dropped within
/// [`Engine::with_remapping_sink`]; the map's lifetime never escapes
/// the call.
#[cfg(feature = "decision-tracing")]
pub(crate) struct StepRemappingSink<'a> {
    inner: &'a mut dyn marque_scheme::DecisionSink,
    next_step: &'a std::sync::atomic::AtomicU32,
    local_to_global: std::collections::HashMap<u32, u32>,
}

#[cfg(feature = "decision-tracing")]
impl marque_scheme::DecisionSink for StepRemappingSink<'_> {
    fn record(&mut self, mut event: marque_scheme::DecisionEvent) {
        let local_step = event.step;
        let global_step = self
            .next_step
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.local_to_global.insert(local_step, global_step);
        // Remap triggered_by if the parent's local step was recorded
        // during this call. Parents from outside the scheme call
        // (engine-emitted siblings) won't be in the map; resolving
        // to None preserves cascade-chain rootedness — the missing-
        // parent case is already a documented root condition for
        // `RecordingSink::into_report`.
        event.triggered_by = event
            .triggered_by
            .and_then(|t| self.local_to_global.get(&t).copied());
        event.step = global_step;
        self.inner.record(event);
    }
}

// Constitution VI: `Engine` must remain `Send + Sync` so
// `BatchEngine` can share it across Tokio workers. The Phase C
// decision-tracing fields (`Mutex<Box<dyn SyncDecisionSink>>` and
// `AtomicU32`) preserve both bounds by construction; the invariant
// is pinned in
// `crates/engine/tests/decision_tracing_smoke.rs` via
// `static_assertions::assert_impl_all!(Engine: Send, Sync)` (kept
// test-only because `static_assertions` is a `dev-dependency`).

/// The engine's default recognizer — an opaque wrapper over the private
/// dispatch enum.
///
/// Public because it is the second type argument of the public
/// [`CapcoEngine`] alias (`Engine<CapcoScheme, EngineRecognizer>`) — a
/// private type named in a `pub type` alias would leak through the public
/// API (E0446). It is only a valid `R` when `S = CapcoScheme`, since it
/// only implements `Recognizer<CapcoScheme>`.
///
/// The dispatch variants (strict, strict-first/decoder-fallback,
/// trait-object escape hatch) are deliberately NOT part of the public
/// surface: the inner enum is private, so downstream code can name the
/// type (to spell `Engine<CapcoScheme, EngineRecognizer>` or use the
/// [`CapcoEngine`] alias) but cannot construct or match on it. Callers
/// select a recognizer through the engine builders
/// ([`Engine::with_recognizer`] for a trait-object recognizer,
/// [`Engine::with_strict_recognizer`] for strict-only); the default
/// ([`Engine::new`]) is strict-first with a decoder fallback. Wrapping a
/// private enum is what makes that contract real — `#[non_exhaustive]`
/// alone would still leave the variants constructable.
#[derive(Clone)]
pub struct EngineRecognizer(EngineRecognizerKind);

/// Private dispatch enum behind [`EngineRecognizer`]. Keeping the
/// strict-only and strict-first/decoder arms as concrete variants lets
/// default dispatch stay monomorphized; `Dyn` is the trait-object escape
/// hatch.
#[derive(Clone)]
enum EngineRecognizerKind {
    /// Fully monomorphized strict-only recognizer path.
    Strict(StrictRecognizer),
    /// Fully monomorphized strict-first/decoder-fallback recognizer path
    /// used by `Engine::new`.
    StrictOrDecoder(StrictOrDecoderRecognizer),
    /// Trait-object escape hatch for caller-supplied recognizers that are
    /// not one of the in-tree concrete variants above.
    Dyn(Arc<dyn Recognizer<CapcoScheme>>),
}

impl EngineRecognizer {
    /// Strict-only dispatch. Backs [`Engine::with_strict_recognizer`].
    pub(crate) fn strict() -> Self {
        Self(EngineRecognizerKind::Strict(StrictRecognizer::new()))
    }

    /// Trait-object dispatch over a caller-supplied recognizer. Backs
    /// [`Engine::with_recognizer`].
    pub(crate) fn dynamic(recognizer: Arc<dyn Recognizer<CapcoScheme>>) -> Self {
        Self(EngineRecognizerKind::Dyn(recognizer))
    }
}

impl Default for EngineRecognizer {
    /// Strict-first with a decoder fallback — the dispatch installed by
    /// [`Engine::new`].
    fn default() -> Self {
        Self(EngineRecognizerKind::StrictOrDecoder(
            StrictOrDecoderRecognizer::new(),
        ))
    }
}

impl Recognizer<CapcoScheme> for EngineRecognizer {
    fn recognize(
        &self,
        bytes: &[u8],
        offset: usize,
        scheme: &CapcoScheme,
        cx: &ParseContext,
    ) -> Parsed<marque_capco::CapcoMarking> {
        match &self.0 {
            EngineRecognizerKind::Strict(r) => r.recognize(bytes, offset, scheme, cx),
            EngineRecognizerKind::StrictOrDecoder(r) => r.recognize(bytes, offset, scheme, cx),
            EngineRecognizerKind::Dyn(r) => r.recognize(bytes, offset, scheme, cx),
        }
    }
}

/// Cached AhoCorasick automaton + the active (key, value) pairs that
/// correspond to its pattern indices.
struct CachedAhoCorasick {
    ac: AhoCorasick,
    /// Active correction pairs, indexed by `PatternID::as_usize()`.
    active: Vec<(Box<str>, Box<str>)>,
}

mod bridge;
mod constructors;
mod dispatch;
mod fix;
mod fix_impl;
mod lint_helpers;
mod page_context;
mod pipeline;
mod synthesis;

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests;
