// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Engine` — the configured, ready-to-run pipeline.

use crate::clock::{Clock, SystemClock};
use crate::errors::{EngineConstructionError, EngineError};
use crate::options::{FixOptions, LintOptions};
use crate::output::{FixResult, LintResult};
use crate::recognizer::shift_token_spans;
use crate::scheduler::{schedule_rewrites, validate_intent_rewrites};
use crate::text_correction::{SynthesizedFix, TextCorrectionProposal};
use aho_corasick::AhoCorasick;
use marque_capco::CapcoScheme;
use marque_capco::provenance::DecoderProvenance;
use marque_config::Config;
use marque_ism::Span;
use marque_rules::{
    AppliedFix, CORRECTIONS_MAP_CITATION, Confidence, Diagnostic, EnginePromotionToken, FixIntent,
    FixSource, Phase, RuleId, RuleSet, Severity, SmallVec,
};
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{ParseContext, Recognizer};
use marque_scheme::{MarkingScheme, RewriteId};
use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
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

/// Synthetic rule identifier the engine attaches to decoder-path
/// `FixSource::DecoderPosterior` diagnostics emitted from
/// `Engine::lint`. Phase 4 PR-4b mints this identifier so the
/// recognition-layer rewrite carries a real `RuleId` (rules and
/// fixes share that requirement) without colliding with any CAPCO
/// `E### / W### / C### / S###` namespace. A diagnostic stamped
/// `R001` originates from the decoder, not from a CAPCO rule.
const DECODER_RULE_ID: &str = "R001";

/// Citation attached to `R001 decoder-recognition` diagnostics. Points
/// at CAPCO-2016 §A.6 — the canonical-marking-form section the decoder
/// is enforcing. Per Constitution VIII the citation is verifiable: §A.6
/// is "(U) Formatting" beginning on page 15 (table of contents,
/// `crates/capco/docs/CAPCO-2016.md` line 49) and contains the
/// canonical syntax for portion / banner / CAB markings the decoder
/// canonicalizes input toward.
const DECODER_CITATION: &str = "CAPCO-2016 §A.6 p15";

/// Synthetic rule identifier for `R002 reparse-failed` diagnostics
/// (PR 7b, FR-024). Emitted when the post-pass-1 buffer fails to
/// re-parse — pass-1 produced ≥1 applied fix that turned the source
/// into an unparseable shape, so pass-2 is skipped and the engine
/// returns the pass-1 buffer + an R002 diagnostic carrying the
/// contributing pass-1 rule IDs.
///
/// **Type note**: this lands as [`RuleId`], not `&'static str`.
/// [`DECODER_RULE_ID`] above is `&'static str` for historical reasons
/// (predates the `RuleId` newtype migration); R002 corrects that. When
/// the (scheme, predicate-id) 2-tuple `RuleId` form lands
/// (post-PR-10 FR-049 unfreeze), this becomes
/// `RuleId::new("engine", "r002.reparse-failed")` per FR-044.
/// `docs/refactor-006/legacy-rule-id-map.md` will record the rename.
/// `DECODER_RULE_ID`'s migration to a real `RuleId` is intentionally
/// deferred (D-7.4).
pub const R002_RULE_ID: RuleId = RuleId::new("R002");

/// Citation attached to `R002` diagnostics — the synthetic
/// re-parse-failure sentinel has no CAPCO §-citation by construction
/// (Constitution VIII requires a real passage; R002 is engine-internal
/// guidance, not a CAPCO rule). Mirrors [`DECODER_CITATION`]'s
/// engine-synthetic origin while staying distinct so a renderer
/// branching on citation strings can tell them apart.
const R002_CITATION: &str = "engine-synthetic";

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

/// A configured engine instance.
pub struct Engine {
    config: Config,
    rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>,
    /// Scheme catalog held for the PR 3c.B Commit 7.2 constraint-bridge
    /// dispatch in `lint_inner`. A fresh `CapcoScheme::new()` is built
    /// at construction time because the engine is concrete over
    /// `CapcoScheme` (the generic-`S` parameter on the constructors is
    /// only used to extract `page_rewrites()` for scheduling — the
    /// scheduler test in `crates/engine/tests/scheduler.rs:106` passes
    /// a stub scheme through that surface, but every production call
    /// site passes `CapcoScheme::new()` and the bridge fires only
    /// against the default catalog). A future PR that makes
    /// `Engine<S>` truly generic over the scheme will replace this
    /// field with the user-supplied `S`.
    scheme: CapcoScheme,
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
    /// edges fully determine the order, FR-007's declaration-order-
    /// independence guarantee holds; when two rewrites have no edge
    /// between them, the scheduler breaks the tie by declaration
    /// order (Kahn's algorithm seeded in declaration order). Empty
    /// when the scheme declares no rewrites.
    scheduled_rewrites: Box<[RewriteId]>,
    /// Recognizer used by `lint()` to resolve each scanner candidate to
    /// a `CanonicalAttrs`. Held behind `Arc<dyn Recognizer>` so callers
    /// can override the default via [`Engine::with_recognizer`] without
    /// touching the lint loop. Shared across threads unchanged — the
    /// recognizer trait is `Send + Sync` and `BatchEngine` workers hold
    /// the same `Arc` reference (Constitution VI, FR-023).
    ///
    /// Default: [`StrictOrDecoderRecognizer`] — strict-first dispatch
    /// with a decoder fallback on strict-parse zero-candidate. The
    /// decoder recovers mangled markings that are edit-distance-1/2,
    /// token-reordered, superseded, or case-mangled from a real
    /// CAPCO-2016 marking. Live-typing surfaces concerned with
    /// per-keystroke latency are expected to debounce their calls into
    /// the engine; surfaces that need to pin strict-only behavior (the
    /// SC-001 interactive-latency benchmark, tests asserting strict
    /// dispatch) install [`StrictRecognizer`] explicitly via
    /// [`Engine::with_recognizer`].
    recognizer: Arc<dyn Recognizer<CapcoScheme>>,

    /// CLI-supplied corpus override (Phase 4 PR-5 / FR-013 / T069).
    /// Held only behind the `corpus-override` Cargo feature so the
    /// WASM artifact and the `marque-server` build cannot
    /// accidentally accept one through any code path.
    ///
    /// The decoder does not yet substitute these priors into scoring
    /// — PR-5 minimal scope wires the surface end-to-end and stamps
    /// every decoder fix with
    /// [`marque_rules::FeatureId::CorpusOverrideInEffect`] in the
    /// audit record so an auditor can identify fixes produced under
    /// organizational overrides vs. stock priors. The prior-
    /// substitution wiring is the next-PR step; this field is the
    /// seam.
    #[cfg(feature = "corpus-override")]
    corpus_override: Option<std::sync::Arc<marque_config::corpus_override::CorpusOverride>>,

    /// Phase partition of the registered rule set, computed once at
    /// construction time (PR 7a, FR-021). Each entry is a
    /// `(rule_set_index, rule_index_within_set)` pair indexing back into
    /// `self.rule_sets[i].rules()[j]`. `pass1_rule_indices` lists every
    /// rule whose `phase()` returned [`Phase::Localized`];
    /// `pass2_rule_indices` lists every rule whose `phase()` returned
    /// [`Phase::WholeMarking`]. Together they enumerate every registered
    /// rule exactly once.
    ///
    /// **Inline-size choice.** `[(usize, usize); 4]` for pass-1
    /// (Localized rules are rare — 4 of 31 in the CAPCO ruleset at
    /// PR 7a: C001, E006, E007, S004) and `[(usize, usize); 32]` for
    /// pass-2. With 27 WholeMarking rules today and an inline capacity
    /// of 32, the partition has 5 entries of headroom before the
    /// SmallVec spills to the heap at the 33rd entry. The current
    /// rule-collapse trajectory (PR 3b retired 13 rules into walkers;
    /// further reductions targeted in stages 3–4) makes 32 comfortable
    /// for the foreseeable future. The canonical per-rule list lives
    /// in `crates/capco/tests/phase_assignment.rs`. Inline storage
    /// means no extra heap allocation in the common case — the
    /// partitions live wherever the `Engine` itself does.
    ///
    /// **Current consumer.** Read by
    /// [`TwoPassFixer::localized_rule_id_set`] to build the
    /// `(Localized rule id) → pass-1` lookup set that drives the
    /// per-document phase dispatch in `TwoPassFixer::run`. Stable for
    /// the lifetime of the engine.
    pass1_rule_indices: Pass1Indices,
    /// Pass-2 (WholeMarking) partition counterpart of
    /// [`Engine::pass1_rule_indices`].
    ///
    /// **PR 7b behavior.** Stored but not yet read. Pass-2 dispatch
    /// in `TwoPassFixer::run` currently routes diagnostics to pass-2
    /// as the **complement** of the pass-1 (Localized) set, which is
    /// sufficient for today's rule shape — every diagnostic emitted
    /// by `lint()` comes from a registered rule, so the complement
    /// equals the WholeMarking partition. PR 7c will switch pass-2 to
    /// a positive whitelist read off this field for symmetry with
    /// pass-1 and to make a future "unregistered emitted ID" land in
    /// neither pass instead of silently in pass-2. See
    /// [`Engine::pass1_rule_indices`] for the shape rationale.
    #[allow(dead_code)]
    pass2_rule_indices: Pass2Indices,
}

/// Cached AhoCorasick automaton + the active (key, value) pairs that
/// correspond to its pattern indices.
struct CachedAhoCorasick {
    ac: AhoCorasick,
    /// Active correction pairs, indexed by `PatternID::as_usize()`.
    active: Vec<(Box<str>, Box<str>)>,
}

impl Engine {
    /// Create a new engine with the given configuration, rule sets, and
    /// marking scheme.
    ///
    /// Runs the page-rewrite scheduler (Kahn's algorithm over the
    /// scheme's declared `reads` / `writes` axes) once at construction
    /// time. Cycles and unannotated `Custom` rewrites fail closed with
    /// [`EngineConstructionError`] rather than degrading at lint time.
    ///
    /// Use [`Engine::with_clock`] for deterministic-timestamp testing.
    pub fn new<S: MarkingScheme>(
        config: Config,
        rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>,
        scheme: S,
    ) -> Result<Self, EngineConstructionError> {
        Self::with_clock(config, rule_sets, scheme, Box::new(SystemClock))
    }

    /// Create an engine with a custom clock (for deterministic tests).
    pub fn with_clock<S: MarkingScheme>(
        mut config: Config,
        rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>,
        scheme: S,
        clock: Box<dyn Clock>,
    ) -> Result<Self, EngineConstructionError> {
        // Instantiate the constraint-catalog bridge's `CapcoScheme`
        // up front so the override canonicalizer can consult its
        // `bridge_emitted_rule_ids()` for IDs the engine emits without
        // a registered `Rule`. The user-supplied generic `scheme: S`
        // is `drop()`-ped below the corrections-map setup and
        // `bridge_scheme` becomes the engine's stored scheme (the
        // `let scheme = bridge_scheme;` step) — see PR 3c.B Commit
        // 7.2's silent-drop note inside that block for the broader
        // design rationale.
        let bridge_scheme = CapcoScheme::new();

        // Canonicalize [rules] overrides against the registered rule
        // set: accept either the rule ID (e.g. "E001") or the rule
        // name (e.g. "portion-mark-in-banner"), resolve both to the
        // canonical ID before the engine stores the map, and hard-fail
        // on any unknown key. See `canonicalize_rule_overrides`.
        canonicalize_rule_overrides(&mut config, &rule_sets, &bridge_scheme)?;

        // PR 3c.B Sub-PR 8.F engine-prereq: validate every
        // `CategoryAction::Intent` payload BEFORE scheduling. Reordering
        // matters when a rewrite table contains both an unroutable
        // Intent token AND a topological cycle: validate-first surfaces
        // the per-rewrite-id error (more actionable) instead of the
        // graph-shaped cycle error, and avoids wasting the scheduler
        // pass on a scheme that can't construct anyway. Walks each
        // intent's `FactRef`s and confirms the scheme can route each
        // one via `category_of`; a scheme-authoring bug surfaces here
        // at engine-construction time instead of silently no-opping on
        // the first page that triggers the rewrite.
        validate_intent_rewrites(&scheme, scheme.page_rewrites())?;
        let scheduled_rewrites = schedule_rewrites(scheme.page_rewrites())?;
        // Take ownership of the corrections map instead of cloning —
        // nothing reads config.corrections after construction.
        let corrections_arc = if config.corrections.is_empty() {
            None
        } else {
            Some(Arc::new(std::mem::take(&mut config.corrections)))
        };

        // Pre-build the AhoCorasick automaton for pre-scanner text corrections.
        // This is O(total pattern bytes) and done once, not per-lint call.
        let corrections_ac = corrections_arc.as_ref().and_then(|corrections| {
            // Sort by key for deterministic pattern ordering — HashMap
            // iteration order is random (hash seed varies per process),
            // and AhoCorasick pattern IDs depend on insertion order.
            let mut active: Vec<(Box<str>, Box<str>)> = corrections
                .iter()
                .filter(|(k, v)| k != v && k.as_str() != "//")
                .map(|(k, v)| (k.as_str().into(), v.as_str().into()))
                .collect();
            active.sort_by(|(a, _), (b, _)| a.cmp(b));
            if active.is_empty() {
                return None;
            }
            let patterns: Vec<&str> = active.iter().map(|(k, _)| k.as_ref()).collect();
            match AhoCorasick::new(&patterns) {
                Ok(ac) => Some(CachedAhoCorasick { ac, active }),
                Err(e) => {
                    tracing::warn!(
                        "failed to build AhoCorasick automaton for corrections map \
                         ({} patterns): {e}; pre-scanner text corrections disabled",
                        patterns.len()
                    );
                    None
                }
            }
        });

        // Drop the user-supplied scheme after page-rewrite extraction;
        // the constraint-catalog bridge in `lint_inner` uses a fresh
        // `CapcoScheme::new()` (see the `scheme` field doc above for
        // the design rationale).
        //
        // CAUTION (review-pass HIGH): this discard is SILENT. A caller
        // that passes a configured `CapcoScheme` (custom catalog,
        // runtime-amended constraint rows, alternative rewrite axis
        // beyond what we already extracted) loses every customization
        // here. No compile-time guard — the `S: MarkingScheme` bound
        // permits any scheme because the scheduler test
        // (`crates/engine/tests/scheduler.rs:106`) deliberately
        // exercises that flexibility with a `StubScheme`. Every
        // production call site today passes `CapcoScheme::new()` (the
        // default), so the discard is currently lossless; a future
        // refactor that makes `Engine<S>` truly generic over the
        // scheme will close this. The `tracing::debug!` below makes
        // the silent drop observable to a developer running with
        // `MARQUE_LOG=marque_engine=debug` (off by default in
        // production).
        tracing::debug!(
            target: "marque_engine::scheme_discard",
            "user-supplied scheme dropped; constraint-catalog bridge uses default \
             CapcoScheme::new() (a future Engine<S> generic-cleanup PR closes this)"
        );
        drop(scheme);
        let scheme = bridge_scheme;

        // PR 7a phase-partition walk (FR-021). Read every registered
        // rule's declared `Phase` and partition the rule set into a
        // pass-1 (Localized) list and a pass-2 (WholeMarking) list
        // indexed by `(rule_set_index, rule_index_within_set)`. The
        // walk runs once at construction time; per-document dispatch
        // reads the cached partition. Phase partition stored but
        // unused in 7a; 7b restructures `fix_inner` to dispatch on
        // it.
        let (pass1_rule_indices, pass2_rule_indices) = partition_rules_by_phase(&rule_sets);

        Ok(Self {
            config,
            rule_sets,
            scheme,
            clock,
            corrections_arc,
            corrections_ac,
            scheduled_rewrites,
            recognizer: Arc::new(crate::decoder::StrictOrDecoderRecognizer::new()),
            #[cfg(feature = "corpus-override")]
            corpus_override: None,
            pass1_rule_indices,
            pass2_rule_indices,
        })
    }

    /// The topologically-sorted rewrite order computed by the scheduler
    /// at construction time.
    ///
    /// Exposed for diagnostic / test inspection. Per-document lint does
    /// not re-sort; this slice is the canonical order every page roll-up
    /// walks.
    pub fn scheduled_rewrites(&self) -> &[RewriteId] {
        &self.scheduled_rewrites
    }

    /// Override the engine's recognizer. The default installed by
    /// [`Engine::new`] is [`StrictOrDecoderRecognizer`] (strict-first,
    /// decoder fallback). Callers that need to pin a different dispatch
    /// — most commonly [`StrictRecognizer`] for the SC-001 interactive-
    /// latency benchmark or tests asserting strict-only behavior —
    /// install one explicitly here.
    ///
    /// Returns the engine by value so callers can chain:
    ///
    /// ```ignore
    /// let engine = Engine::new(config, rules, scheme)?
    ///     .with_recognizer(Arc::new(StrictRecognizer::new()));
    /// ```
    #[must_use = "with_recognizer returns a new Engine; the returned value must be bound for the override to take effect"]
    pub fn with_recognizer(mut self, recognizer: Arc<dyn Recognizer<CapcoScheme>>) -> Self {
        self.recognizer = recognizer;
        self
    }

    /// Install a CLI-supplied corpus override. Only available when
    /// the engine is built with the `corpus-override` Cargo feature
    /// (CLI-only — `marque-server` rejects override input on every
    /// channel per T066, and the WASM crate cannot enable the feature
    /// at all per T067).
    ///
    /// Phase 4 PR-5 minimal scope: the engine retains the override
    /// for audit-annotation purposes only. Every subsequent decoder-
    /// path fix produced by [`Engine::lint`] gets a
    /// [`FeatureId::CorpusOverrideInEffect`] feature contribution
    /// appended to its `Confidence.features` so an auditor can
    /// identify fixes produced under organizational overrides vs.
    /// stock priors. Substituting the override priors into the
    /// decoder's prior-table lookup is the next-PR step.
    #[cfg(feature = "corpus-override")]
    #[must_use = "with_corpus_override returns a new Engine; the result must be bound to take effect — `engine.with_corpus_override(o)` alone leaves the engine without an override installed"]
    pub fn with_corpus_override(
        mut self,
        override_data: std::sync::Arc<marque_config::corpus_override::CorpusOverride>,
    ) -> Self {
        self.corpus_override = Some(override_data);
        self
    }

    /// Whether a corpus override is in effect for this engine.
    ///
    /// Returns `false` unconditionally when the `corpus-override`
    /// Cargo feature is not compiled in — the WASM and server
    /// builds therefore cannot observe a `true` here regardless of
    /// what any caller passes through other surfaces. Callers that
    /// need to thread the flag into audit-record construction (the
    /// private `build_decoder_diagnostic` helper inside this module)
    /// should go through this method rather than poking at the
    /// field directly.
    #[inline]
    pub fn corpus_override_active(&self) -> bool {
        #[cfg(feature = "corpus-override")]
        {
            self.corpus_override.is_some()
        }
        #[cfg(not(feature = "corpus-override"))]
        {
            false
        }
    }

    /// Lint a UTF-8 text buffer. Returns diagnostics without modifying input.
    ///
    /// Back-compat shim over [`Engine::lint_with_options`] — calling
    /// `lint(src)` is equivalent to
    /// `lint_with_options(src, &LintOptions::default())`. New code that
    /// needs a deadline (spec 005 §R3) should call the `_with_options`
    /// variant directly.
    pub fn lint(&self, source: &[u8]) -> LintResult {
        self.lint_with_options(source, &LintOptions::default())
    }

    /// Lint with per-call options (spec 005 §R2).
    ///
    /// Phase 2 honors `opts.deadline` via cooperative cancellation
    /// (spec §R3): a pre-pass check returns immediately on an
    /// already-expired deadline, and a per-candidate check inside
    /// the rule loop breaks out as soon as the deadline passes. The
    /// returned `LintResult` carries `truncated: bool` together with
    /// `candidates_processed` / `candidates_total` so the caller can
    /// distinguish a complete pass from a deadline-bounded partial
    /// pass.
    ///
    /// Granularity: the engine checks the deadline at candidate
    /// boundaries (between scanner-emitted candidates), not inside
    /// any individual rule's `check`. A pathologically slow rule
    /// running on one large candidate can therefore overrun the
    /// deadline by the time that one rule takes; this is the spec
    /// §R3 trade-off — a finer-grained check inside `Rule::check`
    /// would require a deadline-aware rule trait.
    pub fn lint_with_options(&self, source: &[u8], opts: &LintOptions) -> LintResult {
        // Public surface: discard the parsed-markings cache. Internal
        // callers that need it (`fix_inner`, for intent-only fix
        // synthesis without re-parsing) go through
        // `lint_with_options_internal` directly.
        self.lint_with_options_internal(source, opts).0
    }

    /// Internal lint entrypoint that returns the parsed-markings cache
    /// alongside the public `LintResult`.
    ///
    /// The cache maps each scanner-emitted candidate's `Span` (the
    /// source-relative byte range of the candidate, not the
    /// post-`shift_token_spans` attribute spans) to the
    /// `Parsed::Unambiguous` `CapcoMarking` produced by the
    /// recognizer. `synthesize_intent_only_fixes` reads this so the
    /// intent-only synthesis path NEVER re-parses with a different
    /// `ParseContext` — and therefore cannot diverge from the lint
    /// phase's recognition decision (Copilot PR #369 finding #2:
    /// `classification_floor` divergence between lint and synthesis
    /// could turn a previously-unambiguous candidate into
    /// `Parsed::Ambiguous` and silently drop a fix).
    ///
    /// Candidates that fail to recognize (ambiguous or zero
    /// candidates), page-break candidates, and corrections-map text
    /// candidates do not populate the cache; only successful
    /// unambiguous recognitions are stored.
    fn lint_with_options_internal(
        &self,
        source: &[u8],
        opts: &LintOptions,
    ) -> (LintResult, HashMap<Span, marque_capco::CapcoMarking>) {
        use marque_core::Scanner;
        use marque_ism::{MarkingType, PageContext};
        use marque_rules::RuleContext;

        // T007: pre-pass deadline check. An already-expired deadline
        // returns a fully-truncated empty result before the scanner
        // runs at all, preserving the spec invariant that the
        // expired path is observable in zero work.
        if deadline_expired(opts.deadline) {
            return (
                LintResult {
                    truncated: true,
                    ..Default::default()
                },
                HashMap::new(),
            );
        }

        let candidates = Scanner::scan(source);
        // T009: candidates_total is fixed once the scanner has
        // produced the candidate stream. It is independent of how
        // many candidates the rule loop ultimately processes — the
        // delta against `candidates_processed` is what makes
        // truncation observable to the caller (R3). On a complete
        // pass these are equal; on a deadline-bounded pass the
        // function returns early from inside the loop with the
        // partial `candidates_processed`, so the post-loop
        // `LintResult` construction below is reached ONLY on
        // non-truncated completion.
        let candidates_total = candidates.len();
        let mut candidates_processed: usize = 0;

        // Cache of successfully-recognized markings, keyed by the
        // scanner candidate's source-relative `Span`. Populated below
        // immediately after each `Parsed::Unambiguous` recognition and
        // consumed by `synthesize_intent_only_fixes` so the synthesis
        // path looks up the same marking the lint phase saw — avoiding
        // the `ParseContext` divergence Copilot finding #2 flagged.
        let mut parsed_markings: HashMap<Span, marque_capco::CapcoMarking> = HashMap::new();

        // corrections_arc was built once at Engine construction; each clone here
        // is an O(1) refcount bump.
        let corrections_arc = self.corrections_arc.clone();

        let mut diagnostics = Vec::new();
        // Build page context by accumulating portion markings in document order.
        // Banner and CAB rules receive this context so they can validate the
        // observed banner against the expected composite. Phase 3 wires the
        // page-break reset below — the scanner emits a `MarkingType::PageBreak`
        // candidate at every form-feed and at every `\n\n\n+` run; on each
        // such candidate we drop the accumulator and start a fresh page.
        let mut page_context = PageContext::new();
        // Cache the current Arc<PageContext> so that consecutive banner/CAB
        // candidates on the same page share a single allocation. The cache is
        // invalidated (set to None) whenever a new portion is accumulated or
        // a page break resets the context.
        let mut page_context_arc: Option<Arc<PageContext>> = None;

        // FR-011: per-page strict classification floor. Tracks the
        // highest classification rank produced by the strict path on
        // the current page (`marque_ism::Classification as u8`,
        // Unclassified=0 … TopSecret=4). Threaded into
        // `ParseContext::classification_floor` so the decoder rejects
        // any candidate at a strictly-lower level on the same page.
        // Reset on `MarkingType::PageBreak` per Constitution VI's
        // "PageContext resets at scanner-emitted page-break candidates"
        // invariant. Updated *only* by classifications drawn from
        // strict-path recognitions — decoder-recovered markings do not
        // raise the floor for themselves (otherwise a misrecognition
        // would self-justify by raising the floor it then clears).
        let mut classification_floor: Option<u8> = None;

        // PR 3c.B Commit 4 — per-page scratch buffer for
        // `MarkingScheme::render_canonical`. The writer-passing
        // contract on `render_canonical` (caller pre-allocates and
        // reuses) only pays off when this buffer survives across
        // every portion on a page; allocating per-call would defeat
        // the SC-001 latency budget Decision 5 cites in the
        // architecture spec. The buffer is `clear()`ed at every
        // `MarkingType::PageBreak` boundary (alongside the
        // `PageContext` reset, per Constitution VI's pipeline
        // invariant) so a banner roll-up rendered for page N+1
        // starts from an empty buffer rather than appending to
        // page N's residue.
        //
        // Commit 4 ships the allocation + reset site only: no rule
        // emits `Recanonicalize` yet, and `Engine::fix_inner` does
        // not call `render_canonical`. The page-break `.clear()`
        // call is the only mutation the buffer sees today; that
        // mutation is what justifies the `mut` binding (and so
        // satisfies `unused_mut` under `-D warnings` — `.clear()`
        // takes `&mut self`, which counts as a use of the binding's
        // mutability). Commit 6 is the first consumer — when the
        // first `Recanonicalize`-emitting rule lands, the
        // per-portion `render_canonical` call site reuses this
        // buffer instead of allocating a fresh `String` per call.
        let mut render_scratch = String::new();

        // Hoist the `[rules] E059 = ...` config override resolution once
        // per `lint()` call. The override map is immutable for the
        // lifetime of a single lint invocation, and the bridge SCI
        // per-system walk consults this value on every SCI-bearing
        // candidate. Matches the hoisting pattern used for
        // `c001_severity` below (rust-reviewer MEDIUM finding on
        // commit a2fbf12b).
        let e059_override = self
            .config
            .rules
            .overrides
            .get("E059")
            .and_then(|s| Severity::parse_config(s));

        for candidate in &candidates {
            // T008: per-candidate deadline check. Checking at the top
            // of the loop (before any per-candidate work — including
            // a page-break reset) guarantees the abort happens
            // between candidates, never partway through the rule
            // pipeline. On expiry we return immediately so the
            // post-loop corrections-map AhoCorasick pass — which is
            // O(source bytes) — does NOT overrun the deadline.
            // Returning here also gives the spec-correct
            // `truncated/processed/total` triple to the caller
            // without falling through the rest of the function.
            if deadline_expired(opts.deadline) {
                return (
                    LintResult {
                        diagnostics,
                        truncated: true,
                        candidates_processed,
                        candidates_total,
                        ..Default::default()
                    },
                    parsed_markings,
                );
            }

            // T009: count every candidate the engine started
            // processing past the deadline boundary. The increment
            // sits ABOVE the early-`continue` paths below
            // (page-break reset, empty span, ambiguous recognition)
            // so a complete pass always reports
            // `candidates_processed == candidates_total` — the
            // documented contract for a non-truncated `LintResult`.
            // A pass that aborts mid-loop reports `processed <
            // total` with the count of candidates we got past the
            // per-candidate check.
            candidates_processed += 1;

            // Page-break candidates are scanner-emitted boundaries with no
            // parsable content. Reset the context BEFORE attempting to parse
            // — otherwise the parser's MalformedMarking error would skip the
            // continue and leave us accumulating across pages.
            if candidate.kind == MarkingType::PageBreak {
                page_context = PageContext::new();
                page_context_arc = None;
                classification_floor = None;
                // PR 3c.B Commit 4: clear the per-page render
                // scratch buffer at the same boundary as the
                // PageContext reset (Constitution VI invariant).
                // Commit 6's first `Recanonicalize`-emitting rule
                // depends on this happening BEFORE the next page's
                // first portion is rendered.
                render_scratch.clear();
                continue;
            }

            // Parse context built per-candidate so the floor accumulated
            // earlier on the page reaches the recognizer. `strict_evidence
            // = false` permits the dispatcher
            // (`StrictOrDecoderRecognizer`, the default) to fall back to
            // the decoder on strict-parse zero-candidate. The
            // `StrictRecognizer` ignores this flag entirely; consumers
            // that pin strict-only behavior install it via
            // [`Engine::with_recognizer`].
            //
            // `preceded_by_whitespace` is computed against the source
            // buffer here — the decoder receives only the candidate
            // slice and cannot recover the surrounding context on its
            // own. Used downstream to suppress prose-glue false
            // positives like `letter(s)` / `loss(s)` /
            // `function(c)`. Start-of-buffer counts as whitespace by
            // the `ParseContext` convention.
            let preceded_by_whitespace = match candidate.span.start.checked_sub(1) {
                None => true,
                Some(prev_idx) => source
                    .get(prev_idx)
                    .map(|b| b.is_ascii_whitespace())
                    .unwrap_or(true),
            };
            let parse_cx = ParseContext {
                strict_evidence: false,
                zone: None,
                position: None,
                classification_floor,
                as_of: None,
                preceded_by_whitespace,
            };

            // Route each candidate's bytes through the recognizer. Zero-
            // candidate `Ambiguous` means "no plausible interpretation" —
            // skip, same as a strict-path parser error would in the old
            // flow (foundational-plan line 609-612). `Unambiguous` returns
            // a `CapcoMarking` whose `token_spans` are zero-origin relative
            // to the candidate bytes; shift them back to source-relative
            // offsets before rules see them.
            let start = candidate.span.start.min(source.len());
            let end = candidate.span.end.min(source.len());
            if start >= end {
                continue;
            }
            let bytes = &source[start..end];
            let Parsed::Unambiguous(mut marking) = self.recognizer.recognize(bytes, &parse_cx)
            else {
                continue;
            };
            shift_token_spans(&mut marking.0, start);
            // Cache the recognized marking before destructuring so
            // `synthesize_intent_only_fixes` can recover it by
            // candidate span without re-parsing under a divergent
            // `ParseContext` (Copilot PR #369 finding #2). The clone
            // path here is bounded by the candidate count — same cost
            // shape as the existing `CapcoMarking::from(attrs.clone())`
            // construction the constraint bridge does below at the
            // `has_diagnostic_constraints()` arm.
            parsed_markings.insert(candidate.span, marking.clone());
            // Capture the decoder-provenance side channel before
            // collapsing the marking onto its `CanonicalAttrs` payload.
            // Strict-path recognizers leave this `None`; the decoder
            // populates it with the canonical bytes / posterior /
            // features the engine needs to mint a
            // `FixSource::DecoderPosterior` diagnostic below.
            let provenance = marking.1.take();
            let attrs = marking.0;

            // FR-011 strict-floor accumulator: only strict-path
            // recognitions raise the floor. A decoder-path
            // recognition (provenance.is_some()) does not — we cannot
            // let a probabilistic recovery self-justify by raising
            // the threshold it then clears.
            if provenance.is_none() {
                if let Some(level) = attrs
                    .classification
                    .as_ref()
                    .map(|c| c.effective_level() as u8)
                {
                    classification_floor = Some(match classification_floor {
                        Some(prev) => prev.max(level),
                        None => level,
                    });
                }
            }

            // Decoder-path emission (T068): when the recognizer carries
            // provenance, the recognition went through the decoder
            // fallback. Synthesize an R001 `decoder-recognition`
            // diagnostic whose fix rewrites the original mangled bytes
            // to the decoder's canonical form, with `FixSource::DecoderPosterior`
            // and a populated `Confidence` (`recognition < 1.0`,
            // `runner_up_ratio = Some(r)`, non-empty `features`). The
            // fix participates in the regular confidence-threshold
            // gate inside `Engine::fix_inner`.
            if let Some(prov) = provenance {
                let span = Span::new(start, end);
                if let Some(diagnostic) = build_decoder_diagnostic(
                    span,
                    bytes,
                    &prov,
                    candidate.kind,
                    self.corpus_override_active(),
                ) {
                    diagnostics.push(diagnostic);
                }
            }

            // Accumulate portions before running banner/CAB rules so that
            // when we reach a banner candidate the context already reflects
            // all preceding portion data.
            if candidate.kind == MarkingType::Portion {
                page_context.add_portion(attrs.clone());
                // Invalidate the cached Arc so the next banner/CAB gets a
                // fresh snapshot. We rebuild it lazily below.
                page_context_arc = None;
            }

            // Phase 3: zone and position are Option-typed and stay None
            // until a structural scanner pass can prove them. The previous
            // hardcoded `Zone::Body`/`DocumentPosition::Body` was a silent
            // lie to any future rule that read them.
            let ctx_page = if candidate.kind != MarkingType::Portion && !page_context.is_empty() {
                // Lazily wrap the accumulated context in an Arc once per
                // page-context snapshot; subsequent banner/CAB candidates on
                // the same page clone only the cheap Arc pointer.
                Some(
                    page_context_arc
                        .get_or_insert_with(|| Arc::new(page_context.clone()))
                        .clone(),
                )
            } else {
                None
            };
            let ctx = RuleContext {
                marking_type: candidate.kind,
                zone: None,
                position: None,
                // PR 3c.B engine-prereq: the scanner's candidate span
                // is the marking-scope anchor for intent-only fix
                // synthesis. Rules emitting `FixIntent` copy this into
                // `Diagnostic.candidate_span` so the engine can clone
                // the marking, apply intents via
                // `MarkingScheme::apply_intent`, and render the
                // result via `MarkingScheme::render_canonical`.
                candidate_span: candidate.span,
                page_context: ctx_page,
                corrections: corrections_arc.clone(),
            };
            for rule_set in &self.rule_sets {
                for rule in rule_set.rules() {
                    // Hybrid Off handling:
                    //
                    //   - **Fast path** (every non-walker rule): when
                    //     `additional_emitted_ids().is_empty()`, the
                    //     rule emits diagnostics only under its
                    //     registered ID, so configuring that ID to
                    //     `Off` deterministically silences every
                    //     diagnostic the rule could produce. We honor
                    //     the registered-ID Off override BEFORE
                    //     invoking `check()` and skip the rule's body
                    //     entirely. This restores the pre-T026a CPU
                    //     profile for users who disable many rules
                    //     and prevents a buggy rule from logging
                    //     panic warnings while configured `Off`.
                    //
                    //   - **Walker path** (`additional_emitted_ids()`
                    //     non-empty — currently only
                    //     `BannerMatchesProjectedRule`, T026a): the
                    //     rule emits under per-row catalog IDs that
                    //     can each be configured independently, so
                    //     the registered-ID Off override does not
                    //     generalize. The walker's `check()` runs
                    //     unconditionally and per-emitted-id Off
                    //     filtering applies post-check (`diags.retain`
                    //     below).
                    //
                    // The condition reads a `&'static [...]` length —
                    // branch prediction handles the dispatch and the
                    // fast path stays free.
                    if rule.additional_emitted_ids().is_empty() {
                        let configured_severity = self
                            .config
                            .rules
                            .overrides
                            .get(rule.id().as_str())
                            .and_then(|s| Severity::parse_config(s))
                            .unwrap_or(rule.default_severity());
                        if configured_severity == Severity::Off {
                            continue;
                        }
                    }

                    // Whitepaper §6.3 / gap register #10: a buggy rule
                    // that constructs an out-of-range `Confidence`
                    // panics inside `FixProposal::new`. Without this
                    // wrapper, that panic propagates out of `lint()`
                    // and aborts the entire document — turning one
                    // rule's defect into a service outage. Catch the
                    // unwind, log a warning naming the rule, and
                    // skip it. Other rules and other candidates keep
                    // running.
                    //
                    // `AssertUnwindSafe` is a deliberate best-effort
                    // containment — `Send + Sync` (which `Rule`
                    // requires) is NOT the same property as
                    // `UnwindSafe`. The justification rests on the
                    // engine's stateless-rule contract
                    // (`crates/rules/src/lib.rs` `Rule` doc comments):
                    // `check()` must not mutate state visible across
                    // invocations. A rule that violates that contract
                    // via interior mutability could in principle
                    // observe a torn invariant after a panic — but the
                    // alternative is to abort the whole `lint()` on
                    // any rule defect, which is the bug this wrapper
                    // exists to fix. Containing the failure to the
                    // offending rule is strictly better than letting
                    // it cascade. Diagnostics we'd otherwise have
                    // appended on success are built fresh inside the
                    // closure, so they don't pollute the outer
                    // accumulator on the panic path.
                    //
                    // Requires `panic = "unwind"` in the release
                    // profile (`Cargo.toml`). With `panic = "abort"`
                    // the panic terminates the process before this
                    // catch can fire.
                    let rule_id = rule.id();
                    let catch_result =
                        std::panic::catch_unwind(AssertUnwindSafe(|| rule.check(&attrs, &ctx)));
                    let mut diags = match catch_result {
                        Ok(d) => d,
                        Err(payload) => {
                            let msg = panic_payload_to_string(&payload);
                            tracing::warn!(
                                target: "marque_engine::rule_panic",
                                rule = rule_id.as_str(),
                                error = %msg,
                                "rule check panicked; skipping this rule for the current candidate"
                            );
                            Vec::new()
                        }
                    };
                    // Apply configured severity override per emitted
                    // diagnostic, keyed on the diagnostic's `rule` ID
                    // (which may differ from the registered rule's ID
                    // when a dispatcher walker like
                    // `BannerMatchesProjectedRule` emits diagnostics
                    // under per-row catalog IDs — T026a). When no
                    // config override exists for the emitted ID, the
                    // emitted severity is preserved — so per-row
                    // catalog severities (e.g. Fix for E031, Error
                    // for E035 / E040) survive into the audit stream
                    // unchanged.
                    //
                    // Per-emitted-id Off filtering also lives here:
                    // a config that turns off E035 or E040 (which
                    // share the walker's E031 registration) drops
                    // those diagnostics without disabling the others.
                    //
                    // For non-walker rules this filter is a no-op:
                    // the fast-path pre-check above already returned
                    // early on Off, so any diagnostics that reach
                    // this point are non-Off by construction. The
                    // override-application loop below still does
                    // useful work for non-walker rules (a non-Off
                    // override translates the rule's emitted severity
                    // to the configured one).
                    // Single-pass per-emitted-id resolution: one HashMap
                    // lookup + one parse_config per diagnostic. Off drops
                    // the diagnostic; a non-Off override replaces the
                    // rule-emitted severity; absence keeps it (which for
                    // non-walker rules matches `rule.default_severity()`
                    // by convention; for walker rules carries the per-row
                    // catalog severity).
                    diags.retain_mut(|d| {
                        match self
                            .config
                            .rules
                            .overrides
                            .get(d.rule.as_str())
                            .and_then(|s| Severity::parse_config(s))
                        {
                            Some(Severity::Off) => false,
                            Some(override_severity) => {
                                d.severity = override_severity;
                                true
                            }
                            None => true,
                        }
                    });
                    diagnostics.extend(diags);
                }
            }

            // PR 3c.B Commit 7.2 — scheme-side constraint catalog bridge.
            //
            // Walks the scheme's declarative constraint catalog against
            // the current candidate's attributes and emits a
            // `Diagnostic` for each `ConstraintViolation` whose `span`
            // AND `severity` are both populated. Violations with `None`
            // span or `None` severity are advisory — the dyadic
            // `Conflicts` / `Requires` / `Implies` / `Supersedes` arms
            // emit those today, and they continue to flow through as a
            // tooling-only signal until / unless a future PR commits
            // them to user-facing diagnostics.
            //
            // # Cold-land contract (PR 3c.B Commit 7.2)
            //
            // No catalog row populates the `Option<Span>` /
            // `Option<Severity>` fields yet — every `ConstraintViolation`
            // produced by `scheme.validate(...)` in 7.2 carries `None`
            // for both. The `let (Some(span), Some(severity)) = ...
            // else { continue }` guard below short-circuits every
            // iteration. This bridge fires its first user-visible
            // diagnostic when PR 3c.B Commit 7.3 wires the E058
            // class-floor catalog rows to populate the fields from
            // `ClassFloorRow.severity` / `class_floor_anchor_span`.
            //
            // # Cold-land short-circuit
            //
            // The bridge's work is wasted when no catalog row
            // produces a diagnostic-shape `ConstraintViolation` (i.e.,
            // populated span + severity). The `has_diagnostic_constraints()`
            // predicate is the scheme-side declaration of that state:
            // it returns `false` in 7.2 (no row populates yet) and
            // flips to `true` in 7.3 when the first class-floor row
            // gains span / severity from `ClassFloorRow.severity` and
            // a lifted `class_floor_anchor_span`. Skipping the block
            // entirely here avoids the `CapcoMarking::from(attrs.clone())`
            // per-candidate allocation (`CanonicalAttrs` uses `Box<[T]>`
            // for categorical fields, so the clone allocates per
            // list) plus the full `scheme.validate(...)` catalog walk.
            // Keeps SC-001 p95-≤16ms benchmark off the bridge's cost
            // path until there is real catalog work to do.
            if self.scheme.has_diagnostic_constraints() {
                let marking = marque_capco::CapcoMarking::from(attrs.clone());
                let violations = self.scheme.validate(&marking);
                for v in violations {
                    let (Some(span), Some(severity)) = (v.span, v.severity) else {
                        // Advisory-only violation: the catalog row
                        // did not commit to a user-facing diagnostic.
                        // Trace the discard so a developer running
                        // with `MARQUE_LOG=marque_engine=trace` can
                        // see every advisory signal the engine
                        // swallows. No allocation cost in production
                        // (trace is off by default).
                        tracing::trace!(
                            target: "marque_engine::constraint_bridge",
                            constraint = v.constraint_label,
                            "advisory constraint violation (no span / severity); \
                             not surfaced as Diagnostic"
                        );
                        continue;
                    };
                    // PR 3c.B Commit 7.3: catalog-row → rule-ID folding.
                    //
                    // The retired walker rules (E058 class-floor,
                    // E059 SCI per-system) emitted every diagnostic
                    // under their walker-level rule ID with the
                    // per-row identity carried in the message text.
                    // Preserving that convention keeps three external
                    // surfaces stable across the deletion:
                    //   1. `Diagnostic.rule` strings in audit streams
                    //      and NDJSON output;
                    //   2. `[rules] E058 = "off"` / `E059 = "off"`
                    //      config overrides;
                    //   3. `class_floor_catalog.rs` /
                    //      `sci_per_system_catalog.rs` test
                    //      assertions on `diag.rule.as_str()`.
                    //
                    // Inline prefix check (not a scheme-trait method)
                    // because the fold is a CAPCO-specific transient
                    // — PR 4 will retire the prefix convention in
                    // favor of per-row IDs once the audit-stream
                    // contract is renegotiated. Documented per
                    // Constitution VII as an engine-edit channel
                    // sanctioned by the PR 3c.B Commit 7 decision
                    // record (specs/006-engine-rule-refactor/decisions/
                    // 06-commit-7-subdivision.md Amendments 2 and 6).
                    //
                    // # E058 and E059 active arms
                    //
                    // The `E058/` and `class-floor/` arms fold the
                    // 27 class-floor catalog rows under E058 (PR 7.3
                    // wired this on through the `ConstraintViolation`
                    // envelope path). The `E059/` and `sci-per-system/`
                    // arms fold the 5 SCI per-system catalog rows
                    // under E059 — though in production these flow
                    // through the separate
                    // `bridge_sci_per_system_diagnostics` direct path
                    // below (decision record Amendment 6), so the
                    // E059 arms here are reachable only if a future
                    // PR rewires SCI per-system back through the
                    // ConstraintViolation envelope. Both prefix arms
                    // belong here regardless because the canonicalizer
                    // accepts both `E058` and `E059` via
                    // `bridge_emitted_rule_ids()`.
                    let rule_id_str = if v.constraint_label.starts_with("E058/")
                        || v.constraint_label.starts_with("class-floor/")
                    {
                        "E058"
                    } else if v.constraint_label.starts_with("E059/")
                        || v.constraint_label.starts_with("sci-per-system/")
                    {
                        "E059"
                    } else {
                        v.constraint_label
                    };
                    let rule_id = RuleId::new(rule_id_str);
                    let final_severity = self
                        .config
                        .rules
                        .overrides
                        .get(rule_id.as_str())
                        .and_then(|s| Severity::parse_config(s))
                        .unwrap_or(severity);
                    if final_severity == Severity::Off {
                        continue;
                    }
                    // `fix_intent_by_name` resolves a per-row fix intent
                    // for the class-floor catalog rows; pass the raw
                    // `constraint_label` (the catalog row's `name`), NOT
                    // the folded `rule_id`. The fold above collapses 27
                    // class-floor rows to `"E058"`; the scheme-side
                    // helper needs row-level precision to pick the
                    // correct `FixIntent`. Today (PR 3c.B Commit 7.4
                    // landed) this returns `None` for every input —
                    // class-floor violations require human review per
                    // §H.5 / §H.6, so no fix intent populates. The
                    // SCI per-system catalog (E059) takes the direct
                    // `bridge_sci_per_system_diagnostics` path below
                    // because its fix-flow needs (legacy `FixProposal`
                    // payload, multiple violations per row with
                    // distinct fixes) cannot be expressed through this
                    // single-FixIntent-per-violation interface.
                    let fix_intent = self.scheme.fix_intent_by_name(v.constraint_label, &attrs);
                    let diag = Diagnostic::with_fix(
                        rule_id,
                        final_severity,
                        span,
                        v.message,
                        v.citation,
                        fix_intent,
                    );
                    diagnostics.push(diag);
                }

                // PR 3c.B Commit 7.4 — SCI per-system catalog direct path.
                //
                // The SCI per-system catalog rows produce fixes (companion-
                // insertion at the dissem-block anchor; ORCON-USGOV → ORCON
                // replacement). `ConstraintViolation` cannot carry
                // `FixProposal` (marque-scheme is the graph leaf;
                // marque-rules sits above), and a single row can emit
                // multiple violations with distinct fixes which a
                // (name, attrs) helper cannot disambiguate. The fix path
                // takes the direct route: `CapcoScheme` returns full
                // `Diagnostic` values straight from the catalog's emit
                // bodies, with `FixProposal` intact. The retired walker
                // `DeclarativeSciPerSystemRule` did the same dispatch
                // internally; relocating it to the scheme keeps the
                // catalog as the single source of truth.
                //
                // Severity override resolved against the bridge-emitted
                // rule id `"E059"` (registered in the canonicalizer via
                // `CapcoScheme::bridge_emitted_rule_ids`). `Severity::Off`
                // suppresses the entire catalog (FR-008); a non-`Off`
                // override replaces each emitted diagnostic's severity.
                // The override value is hoisted once per `lint()` call
                // above the candidate loop — config is immutable for the
                // lifetime of the call.
                // SCI per-system FactAdd scope tracks the candidate's
                // marking type: a portion candidate emits at portion
                // scope; a banner candidate emits at page scope (the
                // FactAdd applies to the banner roll-up's per-page
                // projection). CAB / page-break candidates don't
                // reach here — the outer loop filters them earlier.
                let fix_scope = match candidate.kind {
                    MarkingType::Portion => marque_scheme::Scope::Portion,
                    _ => marque_scheme::Scope::Page,
                };
                diagnostics.extend(self.scheme.bridge_sci_per_system_diagnostics(
                    &attrs,
                    candidate.span,
                    fix_scope,
                    e059_override,
                ));
            }
        }

        // Pre-scanner text corrections: scan the raw source for
        // corrections-map keys that the scanner missed (e.g., "SERCET" is
        // not a known classification prefix, so the scanner never detects
        // "SERCET//NF" as a candidate, and C001 never sees the token).
        //
        // This pass emits C001 diagnostics for raw-text matches that don't
        // overlap with any C001 diagnostic already produced by the rule
        // pipeline above. Spans reference the original source buffer.
        if let Some(cached) = &self.corrections_ac {
            let c001_severity = self
                .config
                .rules
                .overrides
                .get("C001")
                .and_then(|s| Severity::parse_config(s))
                .unwrap_or(Severity::Fix);

            if c001_severity != Severity::Off {
                // Collect spans already covered by rule-pipeline C001.
                let existing_c001_spans: std::collections::HashSet<Span> = diagnostics
                    .iter()
                    .filter(|d| d.rule.as_str() == "C001")
                    .map(|d| d.span)
                    .collect();

                // Use the pre-built AhoCorasick automaton to scan the full
                // source in a single O(n + m) pass. The automaton and its
                // active pairs were built once at Engine construction time.
                for mat in cached.ac.find_iter(source) {
                    let span = Span::new(mat.start(), mat.end());
                    let (ref key, ref value) = cached.active[mat.pattern().as_usize()];

                    // Skip if the rule pipeline already produced a C001
                    // diagnostic for this exact span.
                    if !existing_c001_spans.contains(&span) {
                        diagnostics.push(Diagnostic::text_correction(
                            RuleId::new("C001"),
                            c001_severity,
                            span,
                            format!("corrections map: {key:?} → {value:?}"),
                            CORRECTIONS_MAP_CITATION,
                            value.as_ref(),
                            FixSource::CorrectionsMap,
                            marque_rules::Confidence::strict(1.0),
                            None,
                        ));
                    }
                }
            }
        }

        // Suggest-don't-fix channel post-pass (issue #235 / #186 PR-3).
        //
        // Only `Severity::Fix` diagnostics are rewritten — those are
        // the ones whose authoring rule expects auto-application. A
        // sub-threshold `FixProposal` attached to a `Fix`-severity
        // diagnostic stays observable in lint output by being
        // demoted to `Severity::Suggest` instead of being silently
        // dropped at the fix-collection threshold gate.
        //
        // Error/Warn/Info rules with sub-threshold fixes keep their
        // severity (the violation IS what the rule says it is; only
        // the suggested replacement is uncertain) and the fix is
        // silently dropped at the apply gate as before. Suggest-channel
        // reuse for Error/Warn fixes is out of scope for PR-C — making
        // a normative ordering rule like E003 CI-silent because its
        // fix confidence sits below threshold would be a behavioral
        // regression.
        //
        // This unifies two emission paths into a single visible
        // channel for `Fix`-severity rules:
        //
        //   - Rules that explicitly emit at `Severity::Suggest`
        //     (e.g., `S004 rel-to-trigraph-suggest`).
        //   - `Fix`-severity rules whose proposal confidence falls
        //     below the configured threshold (decoder-sourced fixes
        //     that didn't quite clear the bar are the canonical case).
        //
        // The fix stays attached because the renderer surfaces the
        // candidate replacement; only the severity is changed. The
        // constitutional V audit-content-ignorance invariant is
        // preserved — no fields are modified except `severity`,
        // which is metadata not document content.
        //
        // `Engine::fix_inner` re-applies the threshold gate on its own
        // (and now also filters by `severity != Suggest`), so a
        // diagnostic rewritten here will not be promoted to an
        // `AppliedFix` even if a later threshold-override raises the
        // floor.
        let threshold = self.config.confidence_threshold();
        for d in &mut diagnostics {
            if d.severity != Severity::Fix {
                continue;
            }
            // Post Commit 10 `Diagnostic.fix` is the sole structural
            // fix channel. C001 text-correction diagnostics carry
            // their replacement bytes on `Diagnostic.text_correction`
            // and run at `Confidence::strict(1.0)` (no posterior
            // uncertainty) — the threshold gate would always pass,
            // so we keep them at their declared severity.
            let combined = match d.fix.as_ref() {
                Some(fix) => fix.confidence.combined(),
                None => continue,
            };
            if combined < threshold {
                d.severity = Severity::Suggest;
            }
        }

        (
            LintResult {
                diagnostics,
                truncated: false,
                candidates_processed,
                candidates_total,
                ..Default::default()
            },
            parsed_markings,
        )
    }

    /// Lint and apply fixes. Returns fixed source and audit log.
    ///
    /// Fix application order follows FR-016: `(span.end DESC, span.start DESC,
    /// rule_id ASC, replacement ASC)` so reverse-byte application preserves
    /// earlier-span offsets and equal-span ties break deterministically.
    ///
    /// Uses the confidence threshold configured in the engine's `Config`.
    /// To supply a per-call override (e.g., from a `--confidence` CLI flag
    /// or an HTTP request field), use [`Engine::fix_with_threshold`] or
    /// [`Engine::fix_with_options`].
    ///
    /// Back-compat shim over [`Engine::fix_with_options`] — `fix(src, mode)`
    /// is equivalent to `fix_with_options(src, mode, &FixOptions::default())`
    /// (no deadline, no threshold override). Both invariants make the
    /// `expect` here unreachable: the default options carry no deadline so
    /// `EngineError::DeadlineExceeded` cannot fire, and the config
    /// threshold is pre-validated at load time so
    /// `EngineError::InvalidThreshold` cannot fire.
    pub fn fix(&self, source: &[u8], mode: FixMode) -> FixResult {
        self.fix_with_options(source, mode, &FixOptions::default())
            .expect(
                "fix() default options cannot fail: no deadline + pre-validated config threshold",
            )
    }

    /// Lint and apply fixes using an optional per-call confidence threshold.
    ///
    /// When `threshold_override` is `Some`, it replaces the config-level
    /// threshold for this call only and is validated against `[0.0, 1.0]`.
    /// When `None`, the engine falls back to `Config::confidence_threshold`.
    ///
    /// This signature is preserved for back-compat. New callers should
    /// prefer [`Engine::fix_with_options`], which carries the deadline
    /// surface alongside the threshold override.
    pub fn fix_with_threshold(
        &self,
        source: &[u8],
        mode: FixMode,
        threshold_override: Option<f32>,
    ) -> Result<FixResult, InvalidThreshold> {
        let opts = FixOptions {
            threshold_override,
            ..Default::default()
        };
        match self.fix_with_options(source, mode, &opts) {
            Ok(result) => Ok(result),
            Err(EngineError::InvalidThreshold(it)) => Err(it),
            // No caller can reach this arm: `fix_with_threshold`'s
            // public signature does not accept a deadline, so the
            // `FixOptions` we built above has `deadline: None`. A
            // future signature change that introduces one would have
            // to remove this `unreachable!` deliberately.
            Err(EngineError::DeadlineExceeded { .. }) => {
                unreachable!("fix_with_threshold cannot set a deadline through its signature")
            }
        }
    }

    /// Lint and apply fixes with per-call options (spec 005 §R2).
    ///
    /// Phase 2 honors `opts.deadline` via cooperative cancellation
    /// (spec §R3). Asymmetric response per §R4 / Constitution V
    /// Principle V (audit-record integrity): a deadline expiring at
    /// any point during the fix path returns
    /// `Err(EngineError::DeadlineExceeded { partial_lint })` rather
    /// than a partial `FixResult`. The `partial_lint` carries
    /// whatever the lint phase had produced before the deadline
    /// fired (or a fully-truncated lint when the deadline was
    /// already expired on entry); no half-applied fix is ever
    /// emitted into the audit stream.
    ///
    /// `opts.threshold_override` is honored from Phase 1 onward; an
    /// out-of-range / NaN value is rejected as
    /// `EngineError::InvalidThreshold` before any work runs.
    pub fn fix_with_options(
        &self,
        source: &[u8],
        mode: FixMode,
        opts: &FixOptions,
    ) -> Result<FixResult, EngineError> {
        let threshold = match opts.threshold_override {
            Some(value) => {
                if !(0.0..=1.0).contains(&value) || value.is_nan() {
                    return Err(EngineError::InvalidThreshold(InvalidThreshold(value)));
                }
                value
            }
            None => self.config.confidence_threshold(),
        };

        self.fix_inner(source, mode, threshold, opts.deadline)
    }

    fn fix_inner(
        &self,
        source: &[u8],
        mode: FixMode,
        threshold: f32,
        deadline: Option<Instant>,
    ) -> Result<FixResult, EngineError> {
        // Five-line trampoline (PR 7b D-7.9). Every stage of the
        // pipeline now lives on `TwoPassFixer`; this method exists
        // to bind the public surface (`fix_with_options` ->
        // `fix_inner`) to the new struct shape.
        TwoPassFixer {
            engine: self,
            source,
            mode,
            threshold,
            deadline,
        }
        .run()
    }

    /// Apply pre-scanner text corrections (C001) from lint diagnostics and
    /// return the corrected source + applied fixes + dropped diagnostics.
    /// Used by `fix_inner` to produce an intermediate source the scanner
    /// can detect; the dropped diagnostics surface via
    /// `remaining_diagnostics`.
    fn apply_text_corrections(
        &self,
        source: &[u8],
        lint: &LintResult,
        threshold: f32,
        mode: FixMode,
    ) -> (
        Vec<u8>,
        Vec<AppliedFix<CapcoScheme>>,
        Vec<Diagnostic<CapcoScheme>>,
    ) {
        // Mirror `fix_inner`'s suggest-channel exclusion: a C001
        // diagnostic that the lint post-pass rewrote to
        // `Severity::Suggest` (because its confidence fell below
        // threshold) must not be auto-applied here either.
        //
        // Post Commit 10: text-correction diagnostics carry their
        // canonical replacement bytes + provenance in
        // `Diagnostic.text_correction` (a `TextCorrection` payload).
        // The engine synthesizes `TextCorrectionProposal` records
        // from those diagnostics and promotes them via
        // `AppliedFix::__engine_promote_text_correction`. Provenance
        // (`source`, `confidence`, `migration_ref`) is preserved per
        // the rule's emission — the engine does NOT overwrite it,
        // because C001 (corrections-map) and E006-shaped (deprecation
        // migration) and other byte-substitution rules all share this
        // channel but carry distinct provenance.
        let mut text_fixes: Vec<TextCorrectionProposal> = lint
            .diagnostics
            .iter()
            .filter(|d| d.severity != Severity::Suggest)
            .filter_map(|d| {
                d.text_correction.as_ref().map(|tc| TextCorrectionProposal {
                    rule: d.rule.clone(),
                    span: d.span,
                    replacement: tc.replacement.clone(),
                    confidence: tc.confidence.clone(),
                    source: tc.source,
                })
            })
            .filter(|p| p.confidence.combined() >= threshold)
            .filter(|p| !p.span.is_empty())
            .collect();

        if text_fixes.is_empty() {
            return (source.to_vec(), Vec::new(), Vec::new());
        }

        // Sort and deduplicate using FR-016 order + C-1 overlap guard.
        text_fixes.sort_by(|a, b| {
            b.span
                .end
                .cmp(&a.span.end)
                .then(b.span.start.cmp(&a.span.start))
                .then(a.rule.cmp(&b.rule))
                .then(a.replacement.cmp(&b.replacement))
        });
        let mut kept: Vec<TextCorrectionProposal> = Vec::new();
        let mut dropped_keys: HashSet<(RuleId, Span)> = HashSet::new();
        let mut next_end: Option<usize> = None;
        for fix in &text_fixes {
            let fits = next_end.is_none_or(|b| fix.span.end <= b);
            if fits {
                next_end = Some(fix.span.start);
                kept.push(fix.clone());
            } else {
                dropped_keys.insert((fix.rule.clone(), fix.span));
            }
        }
        let kept_keys: HashSet<(RuleId, Span)> =
            kept.iter().map(|f| (f.rule.clone(), f.span)).collect();
        // Resurrect the diagnostics for the dropped fixes so they can
        // surface via `remaining_diagnostics`.
        let dropped_diags: Vec<Diagnostic<CapcoScheme>> = lint
            .diagnostics
            .iter()
            .filter(|d| {
                d.text_correction.is_some()
                    && dropped_keys.contains(&(d.rule.clone(), d.span))
                    && !kept_keys.contains(&(d.rule.clone(), d.span))
            })
            .cloned()
            .collect();

        let classifier_id: Option<Arc<str>> =
            self.config.user.classifier_id.as_deref().map(Arc::from);
        let dry_run = mode == FixMode::DryRun;
        let now = self.clock.now();

        // Always apply text corrections to the intermediate buffer, even in
        // DryRun mode. This buffer is internal — pass 2 needs it to re-lint
        // corrected text so downstream rules fire (e.g., E001 on NF after
        // SERCET→SECRET). The final output for DryRun returns the original
        // source in fix_inner, not this intermediate buffer.
        let mut buf = source.to_vec();
        let mut applied = Vec::with_capacity(kept.len());
        for fix in kept {
            buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());
            applied.push(AppliedFix::__engine_promote_text_correction(
                fix.rule,
                fix.span,
                fix.replacement,
                fix.source,
                fix.confidence,
                now,
                classifier_id.clone(),
                dry_run,
                None,
                engine_promotion_token(),
            ));
        }

        (buf, applied, dropped_diags)
    }
}

// ---------------------------------------------------------------------------
// TwoPassFixer (PR 7b D-7.9) — phase-split fix pipeline orchestrator
// ---------------------------------------------------------------------------

/// Phase-split fix pipeline orchestrator (PR 7b).
///
/// `Engine::fix_inner` extracted into a stack-bound struct so the
/// pipeline shape — pass-0 text-corrections, pass-1 [`Phase::Localized`]
/// rule fixes, optional re-parse, pass-2 [`Phase::WholeMarking`] rule
/// fixes — is reviewer-traceable and each stage is independently
/// testable. PM decision D-7.9 enumerates the shape; D-7.6 enumerates
/// the three-stage composition.
///
/// `TwoPassFixer` is constructed and consumed inside a single
/// `fix_inner` call. It borrows the [`Engine`] for read-only access
/// to the rule-set partition + clock + recognizer; it owns no mutable
/// state across the run.
///
/// `'engine` is the only lifetime parameter; 7c will introduce `'a`
/// for the pre-pass-1 attrs cache when the cache lifetime needs to be
/// threaded into [`marque_rules::RuleContext`]. Pre-threading `'a` in
/// 7b would force the same 31-block `impl Rule` rename with no
/// functional benefit, so it is deliberately deferred (rust pre-flight
/// Q11).
struct TwoPassFixer<'engine> {
    engine: &'engine Engine,
    source: &'engine [u8],
    mode: FixMode,
    threshold: f32,
    deadline: Option<Instant>,
}

/// Promoted-fix tuple returned by [`TwoPassFixer::apply_kept_fixes`].
type AppliedTuple = (
    Vec<u8>,
    Vec<AppliedFix<CapcoScheme>>,
    HashSet<(RuleId, Span)>,
);

/// Outcome of pass-0 (text-corrections, UNCHANGED behavior).
struct Pass0Result {
    /// Source bytes after pass-0 text corrections have been applied.
    /// Equals `source.to_vec()` when no text corrections fired.
    effective_source: Vec<u8>,
    /// Promoted [`AppliedFix`] records from pass-0.
    applied: Vec<AppliedFix<CapcoScheme>>,
    /// Diagnostics whose text-correction fixes were dropped by the
    /// C-1 overlap guard during pass-0. Surfaced via
    /// `FixResult.remaining_diagnostics` because pass-2's re-lint
    /// runs on the corrected buffer and would not re-emit them.
    dropped_diags: Vec<Diagnostic<CapcoScheme>>,
}

/// Outcome of pass-1 ([`Phase::Localized`] rule fixes).
struct Pass1Result {
    /// Buffer after pass-1 fixes have been spliced into `effective_source`.
    /// Equals `effective_source` when pass-1 produced no fixes.
    post_buffer: Vec<u8>,
    /// Promoted [`AppliedFix`] records from pass-1.
    applied: Vec<AppliedFix<CapcoScheme>>,
    /// `(rule_id, span)` keys of pass-1 fixes — feeds the
    /// `remaining_diagnostics` filter so a fixed diagnostic is not
    /// reported again.
    applied_keys: HashSet<(RuleId, Span)>,
}

/// Outcome of pass-2 ([`Phase::WholeMarking`] rule fixes).
struct Pass2Result {
    /// Final buffer (Apply mode) or original source (DryRun).
    output: Vec<u8>,
    applied: Vec<AppliedFix<CapcoScheme>>,
    applied_keys: HashSet<(RuleId, Span)>,
}

impl<'engine> TwoPassFixer<'engine> {
    /// Run the full three-stage pipeline and produce a [`FixResult`].
    ///
    /// Error mapping mirrors the pre-7b `fix_inner` shape:
    /// - `Err(EngineError::DeadlineExceeded { partial_lint })` —
    ///   deadline expired at any boundary; per Constitution V Principle V
    ///   no partial `FixResult` ever escapes.
    /// - `Err(EngineError::InvalidThreshold(_))` — cannot fire from
    ///   inside `run`; the threshold is validated upstream in
    ///   `fix_with_options`.
    ///
    /// On R002 (post-pass-1 re-parse failure) `run` returns
    /// `Ok(FixResult { r002_fired: true, ..})` carrying the pass-1
    /// buffer + the union of pass-0 / pass-1 applied fixes + the
    /// synthetic R002 diagnostic. Pass-2 does NOT run.
    fn run(self) -> Result<FixResult, EngineError> {
        let lint_opts = LintOptions {
            deadline: self.deadline,
            ..Default::default()
        };

        // Pass-0: lint original + apply text corrections.
        let (lint1, parsed_markings1) = self
            .engine
            .lint_with_options_internal(self.source, &lint_opts);
        if deadline_expired(self.deadline) {
            return Err(EngineError::DeadlineExceeded {
                partial_lint: lint1,
            });
        }
        let pass0 = self.run_pass0_c001(&lint1);

        // Pass-2 lint baseline: re-lint when pass-0 changed bytes
        // (the scanner sees a different source and can detect newly-
        // valid markings — e.g. `SERCET//NF` → `SECRET//NF` → E001
        // fires on `NF`). Move ownership of the chosen `(lint,
        // markings)` pair so synthesis can consume `parsed_markings`
        // without an explicit clone.
        let (lint, parsed_markings) = if !pass0.applied.is_empty() {
            self.engine
                .lint_with_options_internal(&pass0.effective_source, &lint_opts)
        } else {
            (lint1, parsed_markings1)
        };

        if deadline_expired(self.deadline) {
            return Err(EngineError::DeadlineExceeded { partial_lint: lint });
        }

        // Build the (Localized rule id) lookup set once per run.
        // `additional_emitted_ids` IDs ride with their owning rule's
        // declared phase — a walker that registers as `Localized`
        // emits all its catalog IDs at pass-1.
        let localized_ids = self.localized_rule_id_set();

        // Pass-1 + pass-2 share the synthesized diagnostic stream
        // from `lint`. Partition the diagnostic list by the firing
        // rule's declared phase so each pass's synthesis sees only
        // its own slice — the two C-1 dedup walks are independent
        // by construction (architect pre-flight §2).
        let (pass1_diags, pass2_diags) =
            partition_diags_by_phase(&lint.diagnostics, &localized_ids);

        // Pass-1: Localized FixIntent fixes against the post-pass-0 buffer.
        let pass1 = self.run_pass1_localized(
            &pass0.effective_source,
            &parsed_markings,
            &pass1_diags,
            &lint,
        )?;

        // Destructure pass0 into owned locals so the re-parse / R002
        // branches can move `effective_source` directly (eliminating
        // the prior `.clone()` of the full document buffer on the
        // hot path) while `applied` and `dropped_diags` flow through
        // to the merge step or the R002 assembler without an
        // intermediate `&Pass0Result` borrow that forces cloning.
        // After this point `pass0` is consumed — only the three
        // locals are live.
        let Pass0Result {
            effective_source: pass0_effective_source,
            applied: pass0_applied,
            dropped_diags: pass0_dropped_diags,
        } = pass0;

        // Re-parse decision. Short-circuit when pass-1 produced no
        // applied fixes — the byte stream is unchanged, so the
        // pass-2 lint baseline is identical to the post-pass-0 lint
        // and we can reuse `parsed_markings` AND the pre-pass-1
        // `pass2_diags` partition directly.
        //
        // CanonicalAttrs is owned (no `<'src>` parameter), and
        // parsed_markings is `HashMap<Span, CapcoMarking>`. Moving
        // it in both branches keeps both arms producing the same
        // owned type — no `Cow`, no clone (rust pre-flight Q3).
        //
        // FR-023 (partial — full reshape-aware disambiguation lands
        // in PR 7c with the pre-pass-1 attrs cache + the
        // `(scheme, predicate-id) → no re-fire` gate): when pass-1
        // changed bytes, the re-parse arm dispatches pass-2 against
        // **post-pass-1 attrs AND post-pass-1 diagnostics**, NOT
        // against the stale pre-pass-1 `pass2_diags` partition. Pass-1
        // may have shifted spans (so a pre-pass-1 diagnostic's span no
        // longer points at what the rule meant) or eliminated the
        // very condition a WholeMarking diagnostic flagged (so the
        // diagnostic is obsolete). The fresh re-lint reflects current
        // truth. On the no-fix short-circuit (`pass1.applied.is_empty()`),
        // the buffer didn't change, so the pre-pass-1 `pass2_diags`
        // partition is still current and is reused — saving one
        // re-lint pass on the hot path.
        let (pass2_source, pass2_markings, pass1_applied, pass1_applied_keys, pass2_diags) =
            if pass1.applied.is_empty() {
                // Short-circuit: pass-1 produced no applied fixes, so the
                // byte stream is unchanged. Move `pass0_effective_source`
                // directly into `pass2_source` — no document-buffer clone.
                // `pass1.applied` is empty so we move it through as-is.
                let Pass1Result {
                    post_buffer: _,
                    applied,
                    applied_keys,
                } = pass1;
                (
                    pass0_effective_source,
                    parsed_markings,
                    applied,
                    applied_keys,
                    pass2_diags,
                )
            } else {
                let (relint, new_markings) = self
                    .engine
                    .lint_with_options_internal(&pass1.post_buffer, &lint_opts);
                // R002 trigger (PR 7b, FR-024): pass-1 changed bytes,
                // but the post-pass-1 buffer no longer yields any
                // parsed markings. Marque's recognizer is total — it
                // never returns a hard `Err` — so the "re-parse failed"
                // signal is detected as "pass-1 had markings to begin
                // with, applied fixes against them, and the post-splice
                // buffer has zero markings." That's a buffer the
                // scanner walked clean of every candidate the original
                // had; pass-2 has nothing to fire against and the
                // operator needs to know pass-1 may have corrupted the
                // marking shape.
                //
                // A clean conservative trigger (no false positives on
                // partial cleanups): if the pre-pass-1 buffer had ≥1
                // marking AND the post-pass-1 buffer has zero, the
                // pass-1 splice destroyed marking shape.
                let post_pass1_had_no_markings = new_markings.is_empty();
                let pre_pass1_had_markings = !parsed_markings.is_empty();
                if post_pass1_had_no_markings && pre_pass1_had_markings {
                    let contributing = self.contributing_pass1_rule_ids(&pass1.applied);
                    let failure_span = Span::new(0, pass1.post_buffer.len());
                    let r002 = build_r002_diagnostic(contributing, failure_span);
                    return Ok(self.assemble_r002_result(
                        pass0_applied,
                        pass0_dropped_diags,
                        pass1,
                        lint,
                        r002,
                    ));
                }
                // Non-R002 re-parse path: re-partition the FRESH
                // post-pass-1 diagnostic stream by phase. Pass-2
                // dispatches against `fresh_pass2_diags`, not against
                // the now-stale `pass2_diags` derived from the pre-
                // pass-1 lint. `localized_ids` is unchanged across
                // the pass (the rule registry is `Engine::new`-time
                // immutable), so the partition predicate is identical.
                // The pre-pass-1 `pass1_diags` partition is discarded
                // here because pass-1 has already run; what we need
                // for pass-2 is its post-pass-1 phase partition.
                let (_fresh_pass1_diags, fresh_pass2_diags) =
                    partition_diags_by_phase(&relint.diagnostics, &localized_ids);
                // Non-R002 re-parse path: destructure pass1 and move
                // `post_buffer` directly into `pass2_source` — no
                // document-buffer clone.
                let Pass1Result {
                    post_buffer,
                    applied,
                    applied_keys,
                } = pass1;
                (
                    post_buffer,
                    new_markings,
                    applied,
                    applied_keys,
                    fresh_pass2_diags,
                )
            };

        if deadline_expired(self.deadline) {
            return Err(EngineError::DeadlineExceeded { partial_lint: lint });
        }

        // Pass-2: WholeMarking FixIntent fixes against post-pass-1 buffer.
        let pass2 =
            self.run_pass2_whole_marking(&pass2_source, &pass2_markings, &pass2_diags, &lint)?;

        // Merge applied lists: pass-0 corrections, then pass-1, then
        // pass-2. Order matches the existing audit-stream contract
        // (D-7.6: "applied-fix records emit in the order c001_applied;
        // pass1_applied; pass2_applied").
        let mut all_applied: Vec<AppliedFix<CapcoScheme>> =
            Vec::with_capacity(pass0_applied.len() + pass1_applied.len() + pass2.applied.len());
        all_applied.extend(pass0_applied);
        all_applied.extend(pass1_applied);
        all_applied.extend(pass2.applied);

        // Build the applied-keys set from EVERY applied entry so the
        // remaining-diagnostics filter knows what survived each pass.
        let mut applied_keys: HashSet<(RuleId, Span)> = HashSet::with_capacity(all_applied.len());
        for a in &all_applied {
            applied_keys.insert((a.rule.clone(), a.span));
        }
        for k in &pass1_applied_keys {
            applied_keys.insert(k.clone());
        }
        for k in &pass2.applied_keys {
            applied_keys.insert(k.clone());
        }

        let mut remaining_diagnostics: Vec<Diagnostic<CapcoScheme>> = lint
            .diagnostics
            .into_iter()
            .filter(|d| {
                let fix_applied = if d.fix.is_some() {
                    let span = d.candidate_span.unwrap_or(d.span);
                    applied_keys.contains(&(d.rule.clone(), span))
                } else if d.text_correction.is_some() {
                    applied_keys.contains(&(d.rule.clone(), d.span))
                } else {
                    false
                };
                !fix_applied
            })
            .collect();
        // Pass-0 dropped text-correction diagnostics surface here:
        // pass-2 lint runs on the corrected buffer, so it never
        // re-emits them. The C-1 overlap guard in `apply_text_corrections`
        // recorded the dropped set so we can route it through.
        for d in pass0_dropped_diags {
            remaining_diagnostics.push(d);
        }

        Ok(FixResult {
            source: pass2.output,
            applied: all_applied,
            remaining_diagnostics,
            r002_fired: false,
        })
    }

    /// Pass-0 — text-correction promotion via the existing engine
    /// helper. **Behavior unchanged** from pre-7b (D-7.6: "C001 stays
    /// as pass-0"); this method exists to keep the pipeline shape
    /// visible at the `run()` call site.
    fn run_pass0_c001(&self, lint: &LintResult) -> Pass0Result {
        let (effective_source, applied, dropped_diags) =
            self.engine
                .apply_text_corrections(self.source, lint, self.threshold, self.mode);
        Pass0Result {
            effective_source,
            applied,
            dropped_diags,
        }
    }

    /// Pass-1 — synthesize + filter + sort + C-1 dedup + forward-pass
    /// splice for [`Phase::Localized`] rule fixes.
    ///
    /// First-fire span-shape check (D-7.16) drops out-of-shape fixes
    /// BEFORE the FR-016 sort, so a rule that misdeclared `Localized`
    /// and emitted a marking-wide span never enters the sort or the
    /// C-1 walk. `debug_assert!` panics in debug builds (CI catches);
    /// `tracing::error!` is always-on. The audit stream records
    /// nothing for a dropped fix (no `AppliedFix`).
    ///
    /// # `post_buffer` invariant
    ///
    /// When `applied.is_empty()`, `post_buffer` is returned **empty**
    /// (`Vec::new()`) — the caller MUST consume `pass0.effective_source`
    /// (or equivalent pre-pass-1 buffer) instead of `post_buffer` on
    /// the short-circuit branch. The no-op clone of the full document
    /// was load-bearing nowhere and burned an O(N) allocation per
    /// clean run; eliding it keeps the no-fix path zero-copy. Callers
    /// already destructure `post_buffer: _` on the `applied.is_empty()`
    /// branch (see `TwoPassFixer::run`).
    fn run_pass1_localized(
        &self,
        effective_source: &[u8],
        parsed_markings: &HashMap<Span, marque_capco::CapcoMarking>,
        pass1_diags: &[Diagnostic<CapcoScheme>],
        lint: &LintResult,
    ) -> Result<Pass1Result, EngineError> {
        if pass1_diags.is_empty() {
            // No diagnostics → no fixes → caller short-circuits and
            // consumes its own pre-pass-1 buffer. Skip the document
            // clone; see the function's `post_buffer` invariant above.
            return Ok(Pass1Result {
                post_buffer: Vec::new(),
                applied: Vec::new(),
                applied_keys: HashSet::new(),
            });
        }

        let synthesized: Vec<SynthesizedFix> = synthesize_fixes(
            &self.engine.scheme,
            parsed_markings,
            effective_source,
            pass1_diags,
            self.threshold,
        );

        // First-fire span-shape filter (PR 7b D-7.16). For a
        // `Phase::Localized` rule the fix span MUST be contained
        // within the candidate marking's parsed bytes. The
        // synthesized record carries `span` set from the
        // diagnostic's `candidate_span` (or `span`); the parsed
        // marking's bytes are `parsed_markings`' key span. A fix
        // whose span sits outside the candidate is a misuse of the
        // phase tag and is dropped before the FR-016 sort.
        let in_shape: Vec<SynthesizedFix> = synthesized
            .into_iter()
            .filter(
                |sf| match find_containing_marking(parsed_markings, sf.span) {
                    Some(marking_span) => {
                        if span_is_within_marking(sf.span, marking_span) {
                            true
                        } else {
                            tracing::error!(
                                rule_id = %sf.rule,
                                fix_span = ?sf.span,
                                marking_span = ?marking_span,
                                "Phase::Localized rule emitted out-of-shape span; dropping fix"
                            );
                            debug_assert!(
                                false,
                                "Localized rule '{}' emitted span {:?} outside marking {:?}",
                                sf.rule, sf.span, marking_span
                            );
                            false
                        }
                    }
                    None => {
                        tracing::error!(
                            rule_id = %sf.rule,
                            fix_span = ?sf.span,
                            "Phase::Localized rule fix span has no enclosing parsed marking; \
                             dropping fix"
                        );
                        debug_assert!(
                            false,
                            "Localized rule '{}' emitted span {:?} with no enclosing marking",
                            sf.rule, sf.span
                        );
                        false
                    }
                },
            )
            .collect();

        let kept_fixes = sort_and_c1_dedup(in_shape);
        if kept_fixes.is_empty() {
            // All synthesized fixes were filtered out (out-of-shape
            // drops or C-1 dedup losers). Caller short-circuits and
            // consumes its own pre-pass-1 buffer; skip the document
            // clone per the `post_buffer` invariant above.
            return Ok(Pass1Result {
                post_buffer: Vec::new(),
                applied: Vec::new(),
                applied_keys: HashSet::new(),
            });
        }
        let (post_buffer, applied, applied_keys) =
            self.apply_kept_fixes(effective_source, kept_fixes, lint)?;
        Ok(Pass1Result {
            post_buffer,
            applied,
            applied_keys,
        })
    }

    /// Pass-2 — synthesize + sort + C-1 dedup + forward-pass splice
    /// for [`Phase::WholeMarking`] rule fixes. Operates on the
    /// post-pass-1 buffer and the corresponding freshly-parsed
    /// markings (or, when pass-1 short-circuited, on the post-pass-0
    /// buffer and its markings).
    ///
    /// The C-1 dedup walk is **independent** of pass-1's walk
    /// (architect pre-flight §2): pass-1 keeps the lex-min winner
    /// among Localized fixes; pass-2 keeps the lex-min winner among
    /// WholeMarking fixes. Because the partitions are disjoint by
    /// rule phase, the union of winners has no rule-and-span
    /// collision.
    fn run_pass2_whole_marking(
        &self,
        pass2_source: &[u8],
        parsed_markings: &HashMap<Span, marque_capco::CapcoMarking>,
        pass2_diags: &[Diagnostic<CapcoScheme>],
        lint: &LintResult,
    ) -> Result<Pass2Result, EngineError> {
        if pass2_diags.is_empty() {
            return Ok(Pass2Result {
                output: match self.mode {
                    FixMode::Apply => pass2_source.to_vec(),
                    FixMode::DryRun => self.source.to_vec(),
                },
                applied: Vec::new(),
                applied_keys: HashSet::new(),
            });
        }

        let synthesized = synthesize_fixes(
            &self.engine.scheme,
            parsed_markings,
            pass2_source,
            pass2_diags,
            self.threshold,
        );
        let kept_fixes = sort_and_c1_dedup(synthesized);
        let (post_buffer, applied, applied_keys) =
            self.apply_kept_fixes(pass2_source, kept_fixes, lint)?;

        let output = match self.mode {
            FixMode::Apply => post_buffer,
            // DryRun returns the original source verbatim — pass-1's
            // post-buffer is discarded so callers cannot accidentally
            // consume partial bytes when they asked for dry-run.
            FixMode::DryRun => self.source.to_vec(),
        };
        Ok(Pass2Result {
            output,
            applied,
            applied_keys,
        })
    }

    /// Apply `kept_fixes` (already FR-016-sorted and C-1-deduped)
    /// against `source_buf`, producing the post-splice buffer and the
    /// promoted [`AppliedFix`] records. Shared between pass-1 and
    /// pass-2 because the splice semantics are identical at this
    /// layer.
    ///
    /// Per-fix-application deadline check sits at the top of each
    /// iteration — the abort happens between fixes so we never
    /// construct a half-applied `FixResult` (Constitution V Principle
    /// V). On deadline expiry the function returns
    /// `Err(EngineError::DeadlineExceeded)` and discards any partial
    /// state; the audit stream gets nothing.
    fn apply_kept_fixes(
        &self,
        source_buf: &[u8],
        kept_fixes: Vec<SynthesizedFix>,
        lint: &LintResult,
    ) -> Result<AppliedTuple, EngineError> {
        let classifier_id: Option<std::sync::Arc<str>> = self
            .engine
            .config
            .user
            .classifier_id
            .as_deref()
            .map(std::sync::Arc::from);
        let dry_run = self.mode == FixMode::DryRun;
        let now = self.engine.clock.now();

        let mut applied_keys: HashSet<(RuleId, Span)> = HashSet::with_capacity(kept_fixes.len());
        let mut applied: Vec<AppliedFix<CapcoScheme>> = Vec::with_capacity(kept_fixes.len());

        if deadline_expired(self.deadline) {
            return Err(EngineError::DeadlineExceeded {
                partial_lint: lint.clone(),
            });
        }

        match self.mode {
            FixMode::Apply => {
                let post_buffer = apply_pass1_fixes(source_buf, &kept_fixes);
                for fix in kept_fixes {
                    if deadline_expired(self.deadline) {
                        return Err(EngineError::DeadlineExceeded {
                            partial_lint: lint.clone(),
                        });
                    }
                    let key = (fix.rule.clone(), fix.span);
                    applied_keys.insert(key);
                    applied.push(AppliedFix::__engine_promote(
                        fix.rule,
                        fix.span,
                        fix.intent,
                        now,
                        classifier_id.clone(),
                        dry_run,
                        None,
                        engine_promotion_token(),
                    ));
                }
                Ok((post_buffer, applied, applied_keys))
            }
            FixMode::DryRun => {
                for fix in kept_fixes {
                    if deadline_expired(self.deadline) {
                        return Err(EngineError::DeadlineExceeded {
                            partial_lint: lint.clone(),
                        });
                    }
                    let key = (fix.rule.clone(), fix.span);
                    applied_keys.insert(key);
                    applied.push(AppliedFix::__engine_promote(
                        fix.rule,
                        fix.span,
                        fix.intent,
                        now,
                        classifier_id.clone(),
                        dry_run,
                        None,
                        engine_promotion_token(),
                    ));
                }
                // DryRun: no buffer mutation. The post_buffer slot
                // is unused — `run_pass2_whole_marking` substitutes
                // `self.source` at the outer layer.
                Ok((source_buf.to_vec(), applied, applied_keys))
            }
        }
    }

    /// Build the set of `RuleId.as_str()` values that belong to
    /// [`Phase::Localized`], including IDs reported via
    /// [`Rule::additional_emitted_ids`]. Walker rules that register
    /// under one bookkeeping ID but emit diagnostics under per-row
    /// catalog IDs (e.g. `BannerMatchesProjectedRule`) propagate
    /// their declared phase to every catalog ID.
    fn localized_rule_id_set(&self) -> HashSet<&'static str> {
        let mut out: HashSet<&'static str> = HashSet::new();
        for &(set_idx, rule_idx) in self.engine.pass1_rule_indices.iter() {
            let rule = &self.engine.rule_sets[set_idx].rules()[rule_idx];
            out.insert(rule.id().as_str());
            for &(emitted_id, _) in rule.additional_emitted_ids() {
                out.insert(emitted_id);
            }
        }
        out
    }

    /// Collect the unique contributing pass-1 rule IDs in
    /// FR-016-stable order (sort + dedup) for the R002 payload.
    /// Capped at 4 entries to fit the `SmallVec<[RuleId; 4]>` inline
    /// capacity exactly — pass-1 has at most 4 rule families today
    /// (C001/E006/E007/S004), and a future Localized rule expansion
    /// can lift the cap in lockstep with the inline-N bump.
    fn contributing_pass1_rule_ids(
        &self,
        pass1_applied: &[AppliedFix<CapcoScheme>],
    ) -> SmallVec<[RuleId; 4]> {
        let mut seen: HashSet<RuleId> = HashSet::new();
        let mut ids: Vec<RuleId> = Vec::new();
        for fix in pass1_applied {
            if seen.insert(fix.rule.clone()) {
                ids.push(fix.rule.clone());
            }
        }
        ids.sort();
        let mut out: SmallVec<[RuleId; 4]> = SmallVec::new();
        for id in ids.into_iter().take(out.inline_size()) {
            out.push(id);
        }
        out
    }

    /// Assemble the R002 `FixResult` — pass-1 buffer + union of
    /// pass-0/pass-1 applied + R002 diagnostic appended to remaining.
    ///
    /// Takes `pass0_applied` and `pass0_dropped_diags` by value (rather
    /// than borrowing a `&Pass0Result`) so the assembler can `extend`
    /// directly without per-element clones — `pass0` is dead at the
    /// caller after destructuring, so move-semantics is appropriate.
    fn assemble_r002_result(
        &self,
        pass0_applied: Vec<AppliedFix<CapcoScheme>>,
        pass0_dropped_diags: Vec<Diagnostic<CapcoScheme>>,
        pass1: Pass1Result,
        lint: LintResult,
        r002: Diagnostic<CapcoScheme>,
    ) -> FixResult {
        let mut all_applied: Vec<AppliedFix<CapcoScheme>> =
            Vec::with_capacity(pass0_applied.len() + pass1.applied.len());
        all_applied.extend(pass0_applied);
        all_applied.extend(pass1.applied);

        let mut applied_keys: HashSet<(RuleId, Span)> = HashSet::with_capacity(all_applied.len());
        for a in &all_applied {
            applied_keys.insert((a.rule.clone(), a.span));
        }

        let mut remaining_diagnostics: Vec<Diagnostic<CapcoScheme>> = lint
            .diagnostics
            .into_iter()
            .filter(|d| {
                let fix_applied = if d.fix.is_some() {
                    let span = d.candidate_span.unwrap_or(d.span);
                    applied_keys.contains(&(d.rule.clone(), span))
                } else if d.text_correction.is_some() {
                    applied_keys.contains(&(d.rule.clone(), d.span))
                } else {
                    false
                };
                !fix_applied
            })
            .collect();
        remaining_diagnostics.extend(pass0_dropped_diags);
        remaining_diagnostics.push(r002);

        // Output buffer: post-pass-1 in Apply mode, original in DryRun.
        // Per D-7.6 the pass-1 buffer is the returned source even on
        // R002 (the fixes happened; the audit log is honest about it).
        let output = match self.mode {
            FixMode::Apply => pass1.post_buffer,
            FixMode::DryRun => self.source.to_vec(),
        };

        FixResult {
            source: output,
            applied: all_applied,
            remaining_diagnostics,
            r002_fired: true,
        }
    }
}

/// Partition a diagnostic stream by the firing rule's declared
/// `Phase` (PR 7b two-pass split). Diagnostics whose rule ID
/// appears in `localized_ids` flow to pass-1; everything else
/// flows to pass-2.
///
/// `text_correction` diagnostics whose `fix` is `None` are
/// excluded from both partitions: their fixes were promoted by
/// pass-0 (the `[corrections]` map text-correction channel), and
/// the no-fix text-correction case is a sub-threshold suggestion
/// that the remaining-diagnostics filter resurfaces via the
/// pre-existing keying path. Returns `(pass1_diags, pass2_diags)`.
///
/// Called twice per `TwoPassFixer::run`: once on the pre-pass-1
/// lint (`lint.diagnostics`), and again on the post-pass-1
/// re-lint (`relint.diagnostics`) when pass-1 changed bytes.
/// Pass-2 dispatches against the post-pass-1 partition (FR-023
/// partial — the full reshape-aware `(scheme, predicate-id)`
/// no-re-fire gate lands in PR 7c on top of the pre-pass-1 attrs
/// cache).
fn partition_diags_by_phase(
    diagnostics: &[Diagnostic<CapcoScheme>],
    localized_ids: &HashSet<&'static str>,
) -> (Vec<Diagnostic<CapcoScheme>>, Vec<Diagnostic<CapcoScheme>>) {
    let mut pass1_diags: Vec<Diagnostic<CapcoScheme>> = Vec::new();
    let mut pass2_diags: Vec<Diagnostic<CapcoScheme>> = Vec::new();
    for d in diagnostics {
        // text_correction diagnostics flow through pass-0 only;
        // they are excluded from both pass-1 and pass-2 splicing
        // (their fixes have already been promoted into
        // `pass0.applied`). The remaining-diagnostics filter
        // resurfaces any text_correction diagnostic whose fix did
        // not apply, via the pre-existing keying path.
        if d.text_correction.is_some() && d.fix.is_none() {
            continue;
        }
        if localized_ids.contains(d.rule.as_str()) {
            pass1_diags.push(d.clone());
        } else {
            pass2_diags.push(d.clone());
        }
    }
    (pass1_diags, pass2_diags)
}

/// Inline span-containment predicate (PR 7b D-7.16). Endpoints
/// inclusive on both sides: a fix whose span exactly matches a
/// token's boundaries is still sub-token-shape. Inline because the
/// pass-1 dispatch loop calls this per-fix.
#[inline]
fn span_is_within_marking(inner: Span, outer: Span) -> bool {
    inner.start >= outer.start && inner.end <= outer.end
}

/// Find the marking span (key in `parsed_markings`) whose byte range
/// contains `fix_span`. Linear scan over the markings table — typical
/// documents have <100 markings and this is the defect path (a
/// well-behaved Localized rule emits sub-token spans by
/// construction), so no binary-search optimization is justified.
fn find_containing_marking(
    parsed_markings: &HashMap<Span, marque_capco::CapcoMarking>,
    fix_span: Span,
) -> Option<Span> {
    parsed_markings
        .keys()
        .copied()
        .find(|marking_span| span_is_within_marking(fix_span, *marking_span))
}

/// FR-016 sort + C-1 dedup walk extracted into a helper so pass-1
/// and pass-2 share an identical ordering/dedup pipeline. The
/// walks are run independently per pass (architect pre-flight §2);
/// the helper exists to factor the algorithm, not the state.
///
/// Sorts `synthesized` **in place** and consumes each kept fix
/// into the result vector. Avoids the prior intermediate
/// `Vec<&SynthesizedFix>` reference-vector + per-element clone
/// (pre-PR-7b allocated and cloned twice; this pass allocates zero
/// extra `SynthesizedFix` values).
fn sort_and_c1_dedup(mut synthesized: Vec<SynthesizedFix>) -> Vec<SynthesizedFix> {
    synthesized.sort_by(|a, b| {
        b.span
            .end
            .cmp(&a.span.end)
            .then(b.span.start.cmp(&a.span.start))
            .then(a.rule.cmp(&b.rule))
            .then(a.replacement.cmp(&b.replacement))
    });
    let mut kept_fixes: Vec<SynthesizedFix> = Vec::with_capacity(synthesized.len());
    let mut next_window_end: Option<usize> = None;
    for fix in synthesized {
        let fits = next_window_end.is_none_or(|boundary| fix.span.end <= boundary);
        if fits {
            next_window_end = Some(fix.span.start);
            kept_fixes.push(fix);
        }
    }
    kept_fixes
}

/// Forward-pass buffer construction shared by pass-1 and pass-2
/// (rust pre-flight Q2). `fixes` MUST be FR-016 sorted (span.end
/// DESC, span.start DESC) so `iter().rev()` yields ascending order
/// for the left-to-right walk. Pre-allocates capacity using the
/// per-fix growth contribution (`saturating_sub` upper bound).
///
/// # Overlap handling
///
/// The `debug_assert!` catches overlap violations that C-1 dedup
/// should have removed; under `cfg(debug_assertions)` it panics
/// with the offending cursor/span (CI catches the bug). On release
/// builds the assertion is compiled out, but the very next line
/// (`buf.extend_from_slice(&source[cursor..fix.span.start])`) will
/// itself panic at the slice operation when `fix.span.start <
/// cursor` — the range `cursor..fix.span.start` is invalid and Rust
/// slicing panics on invalid ranges. Both the `debug_assert!` and
/// the subsequent slice are load-bearing: the assert gives a
/// targeted message in dev/CI, the slice panic provides a hard
/// stop in release. Neither silently corrupts the buffer; a real
/// overlap is observable in either build mode.
fn apply_pass1_fixes(source: &[u8], fixes: &[SynthesizedFix]) -> Vec<u8> {
    let extra: usize = fixes
        .iter()
        .map(|f| {
            f.replacement
                .len()
                .saturating_sub(f.span.end - f.span.start)
        })
        .sum();
    let mut buf = Vec::with_capacity(source.len() + extra);
    let mut cursor = 0usize;
    for fix in fixes.iter().rev() {
        debug_assert!(
            fix.span.start >= cursor,
            "overlapping pass-1 fix: cursor={cursor}, span={:?}",
            fix.span
        );
        buf.extend_from_slice(&source[cursor..fix.span.start]);
        buf.extend_from_slice(fix.replacement.as_bytes());
        cursor = fix.span.end;
    }
    buf.extend_from_slice(&source[cursor..]);
    buf
}

// ---------------------------------------------------------------------------
// Engine-only AppliedFix promotion gate (Constitution V Principle V)
// ---------------------------------------------------------------------------

/// Mint an [`EnginePromotionToken`] for [`AppliedFix::__engine_promote`].
///
/// This is the **single** place inside `marque-engine` where the engine
/// grants itself the privilege to promote a `FixProposal` to an
/// `AppliedFix`. Constitution V Principle V scopes audit-record
/// promotion to four production call sites in this file:
/// `Engine::fix_inner`, `Engine::apply_text_corrections`, this token-
/// mint helper, and `TwoPassFixer::apply_kept_fixes` (the phase-split
/// orchestrator extracted from `fix_inner` in PR 7b). Centralizing the
/// token construction here makes "where does the engine decide to
/// promote?" a one-grep question, and means a future refactor that
/// adds a fifth promotion site has to thread through this function
/// — a deliberate decision, not an accident. The `promote-callsite-lint`
/// CI gate (`tools/promote-callsite-lint/`) mechanically enforces the
/// allow-list.
///
/// `EnginePromotionToken`'s sole field is private to `marque-rules`,
/// so external crates cannot brace-construct one. The
/// `__engine_construct` constructor on the token is `#[doc(hidden)]`
/// and named to make its intent unmistakable to anyone reading a call
/// site outside the engine.
#[inline]
fn engine_promotion_token() -> EnginePromotionToken {
    EnginePromotionToken::__engine_construct()
}

// ---------------------------------------------------------------------------
// FixIntent → byte-precise replacement synthesis
// ---------------------------------------------------------------------------

/// Synthesize byte-precise [`SynthesizedFix`] records for fix-emitting
/// diagnostics.
///
/// Walks `diagnostics`, finds entries with `fix.is_some()`, groups them
/// by `candidate_span` (or `span` when `candidate_span` is unset), looks
/// up each candidate's recognized marking in the `parsed_markings`
/// cache populated by the lint phase, applies the group's intent batch
/// via [`CapcoScheme::apply_intent`], and renders the resulting marking
/// via [`CapcoScheme::render_portion`] or [`CapcoScheme::render_banner`].
/// The candidate's portion-vs-banner scope is inferred from the
/// candidate bytes themselves: a portion is wrapped in `()`, a banner
/// is not.
///
/// Returns one [`SynthesizedFix`] **per candidate-span group**.
///
/// # Audit collapse: one SynthesizedFix per group (lex-min rule_id wins)
///
/// When multiple diagnostics share a `candidate_span`, the function
/// collapses them into ONE record whose `rule` is the
/// lexicographically-smallest rule_id in the group (FR-016 deterministic
/// ordering) and whose carried `intent.confidence` is scaled down to
/// the minimum combined-confidence across the group's intents
/// (conservative — the engine's threshold gate compares against the
/// weakest signal in the batch).
///
/// **Rationale.** The C-1 overlap guard keeps only one fix per
/// overlapping span; collapsing at synthesis time means every dropped
/// diagnostic in the group corresponds to bytes the kept fix already
/// rewrote — an honest audit per Constitution V Principle V.
///
/// # Filters
///
/// - `Severity::Suggest` → excluded (hard exclusion from auto-apply).
/// - `Confidence::combined() < threshold` → excluded.
/// - Empty `candidate_span` → excluded.
/// - Candidate not present in `parsed_markings` → diagnostic dropped
///   with a `tracing::warn`.
/// - `scheme.apply_intent` returns `Err(IntentInapplicable)` → the
///   diagnostic is dropped silently (no-op fix).
///
/// # Audit shape
///
/// The synthesized record carries the rule's `FixIntent` directly;
/// `__engine_promote` moves it into
/// `AppliedFixProposal::FixIntent(_)`. Original bytes are never
/// copied into the audit record — Constitution V Principle V (G13).
fn synthesize_fixes(
    scheme: &CapcoScheme,
    parsed_markings: &HashMap<Span, marque_capco::CapcoMarking>,
    source: &[u8],
    diagnostics: &[marque_rules::Diagnostic<CapcoScheme>],
    threshold: f32,
) -> Vec<SynthesizedFix> {
    use std::collections::BTreeMap;

    // Group diagnostics by candidate_span (falls back to span when
    // `candidate_span` is unset) so multi-intent batches on the same
    // marking apply atomically. BTreeMap keyed on (start, end) so
    // iteration order is deterministic — Span itself doesn't impl Ord.
    #[allow(clippy::type_complexity)]
    let mut groups: BTreeMap<
        (usize, usize),
        (Span, Vec<&marque_rules::Diagnostic<CapcoScheme>>),
    > = BTreeMap::new();
    for d in diagnostics {
        let Some(intent) = d.fix.as_ref() else {
            continue;
        };
        if d.severity == Severity::Suggest {
            continue;
        }
        if intent.confidence.combined() < threshold {
            continue;
        }
        let cspan = d.candidate_span.unwrap_or(d.span);
        if cspan.is_empty() {
            continue;
        }
        groups
            .entry((cspan.start, cspan.end))
            .or_insert_with(|| (cspan, Vec::new()))
            .1
            .push(d);
    }

    if groups.is_empty() {
        return Vec::new();
    }

    let mut out: Vec<SynthesizedFix> = Vec::with_capacity(groups.len());

    for (_key, (cspan, mut group_diags)) in groups {
        let start = cspan.start.min(source.len());
        let end = cspan.end.min(source.len());
        if start >= end {
            continue;
        }
        let bytes = &source[start..end];

        // Look up the marking the lint phase recognized for this
        // candidate. The cache is populated by
        // `lint_with_options_internal` so the marking here is
        // byte-identical to the one the rule fired against.
        let Some(marking) = parsed_markings.get(&cspan) else {
            tracing::warn!(
                target: "marque_engine::fix_synth",
                start = start,
                end = end,
                "fix diagnostic's candidate_span missing from \
                 parsed-markings cache; rule may have populated \
                 candidate_span incorrectly. Skipping fix synthesis."
            );
            continue;
        };

        // Collect the intent batch for this candidate. Each diagnostic
        // contributes one intent; the scheme applies them in slice
        // order. `apply_intent` is required to be commutative within a
        // batch (trait doc), so slice order is not load-bearing.
        let intents: Vec<marque_scheme::ReplacementIntent<CapcoScheme>> = group_diags
            .iter()
            .filter_map(|d| d.fix.as_ref().map(|i| i.replacement.clone()))
            .collect();

        let modified = match scheme.apply_intent(marking, &intents) {
            Ok(m) => m,
            Err(marque_scheme::ApplyIntentError::IntentInapplicable) => {
                // Marking is already consistent — drop silently.
                continue;
            }
            Err(e) => {
                tracing::warn!(
                    target: "marque_engine::fix_synth",
                    start = start,
                    end = end,
                    error = %e,
                    "scheme.apply_intent failed during fix synthesis; skipping"
                );
                continue;
            }
        };

        // Render the modified marking, preserving any leading /
        // trailing ASCII whitespace from the candidate slice.
        // `render_banner` emits no surrounding whitespace; without
        // this preservation step the splice would strip indentation /
        // trailing spaces from any banner line.
        let leading_ws_len = bytes.iter().take_while(|b| b.is_ascii_whitespace()).count();
        let trailing_ws_len = bytes
            .iter()
            .rev()
            .take_while(|b| b.is_ascii_whitespace())
            .count();
        let trimmed_start = leading_ws_len;
        let trimmed_end = bytes.len().saturating_sub(trailing_ws_len);

        if trimmed_end <= trimmed_start {
            tracing::warn!(
                target: "marque_engine::fix_synth",
                start = start,
                end = end,
                "fix candidate bytes are all whitespace; skipping"
            );
            continue;
        }

        let trimmed = &bytes[trimmed_start..trimmed_end];
        // Portion vs banner: inferred from the trimmed candidate
        // bytes — a portion is wrapped in `()` per CAPCO-2016 §A.6.
        let core: String = if trimmed.first() == Some(&b'(') && trimmed.last() == Some(&b')') {
            format!("({})", scheme.render_portion(&modified))
        } else {
            scheme.render_banner(&modified)
        };

        let leading_ws =
            std::str::from_utf8(&bytes[..leading_ws_len]).expect("ASCII whitespace is valid UTF-8");
        let trailing_ws =
            std::str::from_utf8(&bytes[trimmed_end..]).expect("ASCII whitespace is valid UTF-8");
        let replacement = format!("{leading_ws}{core}{trailing_ws}");

        // Audit-collapse: one SynthesizedFix per candidate-span group.
        // The owning rule is the lex-smallest rule_id; the carried
        // `intent.confidence.rule` is scaled down so combined() equals
        // the minimum across the group.
        group_diags.sort_by(|a, b| a.rule.cmp(&b.rule));
        let owning_diag = group_diags[0];
        let owning_intent = owning_diag
            .fix
            .as_ref()
            .expect("filtered above by fix.is_some()");

        let min_combined: f32 = group_diags
            .iter()
            .filter_map(|d| d.fix.as_ref())
            .map(|i| i.confidence.combined())
            .fold(f32::INFINITY, f32::min);
        let mut combined_intent = owning_intent.clone();
        if min_combined < combined_intent.confidence.combined()
            && combined_intent.confidence.rule > 0.0
        {
            let scaled_rule = (min_combined
                / combined_intent
                    .confidence
                    .recognition
                    .max(f32::MIN_POSITIVE))
            .clamp(0.0, 1.0);
            combined_intent.confidence.rule = scaled_rule;
        }

        out.push(SynthesizedFix {
            rule: owning_diag.rule.clone(),
            span: cspan,
            replacement: replacement.into_boxed_str(),
            intent: combined_intent,
        });
    }

    out
}

// ---------------------------------------------------------------------------
// Decoder-path diagnostic synthesis (Phase 4 PR-4b — T068)
// ---------------------------------------------------------------------------

/// Build the synthetic `R001 decoder-recognition` diagnostic the engine
/// emits when a recognizer returned a marking carrying
/// [`DecoderProvenance`]. Returns `None` when the original or canonical
/// bytes are not valid UTF-8 — `FixProposal` carries `Box<str>` for both
/// `original` and `replacement`, so we cannot construct the proposal
/// without UTF-8 validity. CAPCO markings are ASCII by spec (CAPCO-2016
/// §A.6); a non-UTF-8 result here would mean the canonicalization pass
/// produced something the strict parser shouldn't have accepted, which
/// is a separate bug to surface — silently dropping the synthetic
/// diagnostic is the conservative move.
///
/// # Audit-shape contract (Constitution V Principle V / G13)
///
/// The diagnostic's `message` MUST NOT carry verbatim input bytes —
/// only token canonicals, span offsets, and digests/posterior scalars
/// are permitted in audit output. The "before" form is omitted from
/// the message; the span tells the audit consumer *where* the fix
/// landed and the structural `FixIntent` carries *what* shape the
/// recognition became (a `Recanonicalize { scope: RecanonScope::Page }`
/// emission for R001).
///
/// Post-Commit-10 the audit record's `AppliedFix.proposal` no longer
/// carries any document bytes for the decoder path: the
/// `AppliedFixProposal::FixIntent(_)` variant carries the structural
/// intent only. Original document bytes already exist in the source;
/// the audit record is not the right channel for them. The legacy
/// `FixProposal { original, replacement }` byte-precise carrier that
/// previously held canonical bytes on this path retired with the
/// `mvp-2 → mvp-3` schema flip.
///
/// Note: this contract addresses the audit-record *shape*. A separate
/// upstream concern was whether the canonical-bytes synthesis was
/// well-formed when the decoder accepted unrecognized bytes as a
/// compartment-shaped token and uppercased them — that's a decoder-
/// correctness issue tracked separately; the structural-intent path
/// closes the audit-shape channel by construction.
///
/// The fix's `Confidence` is populated entirely from the decoder's
/// provenance trace:
///
/// - `recognition` derives from `runner_up_ratio` via softmax (see
///   [`DecoderProvenance::recognition_score`]); strictly less than
///   `1.0` so audit consumers can distinguish strict from decoder
///   provenance via a single field comparison.
/// - `rule` is `1.0` — once the decoder has decided unambiguously the
///   recognition-layer rewrite is itself unambiguous (rewrite the
///   observed bytes to canonical bytes), so the rule axis carries no
///   additional uncertainty. The decoder's recognition uncertainty is
///   already captured in `recognition`.
/// - `runner_up_ratio` and `features` thread through verbatim from the
///   provenance.
/// - When `corpus_override_active` is `true`, an extra
///   [`FeatureId::CorpusOverrideInEffect`] contribution with
///   `delta = 0.0` is appended to `features`. The zero delta is
///   load-bearing: PR-5 minimal scope wires the surface end-to-end
///   without yet substituting override priors into decoder scoring,
///   so the contribution is purely an audit-trail marker
///   ("this fix was produced under organizational overrides")
///   rather than an actual posterior shift. A future PR that wires
///   override-prior substitution will replace `0.0` with the real
///   delta and re-version the audit schema.
fn build_decoder_diagnostic(
    span: Span,
    original_bytes: &[u8],
    provenance: &DecoderProvenance,
    _kind: marque_ism::MarkingType,
    corpus_override_active: bool,
) -> Option<Diagnostic<CapcoScheme>> {
    use marque_rules::confidence::{FeatureContribution, FeatureId};

    let original = std::str::from_utf8(original_bytes).ok()?;
    let replacement = std::str::from_utf8(&provenance.canonical_bytes).ok()?;

    // No-op rewrite (canonicalization preserved bytes byte-for-byte) is
    // not informative and would produce a degenerate audit record; skip.
    if original == replacement {
        return None;
    }

    // `provenance.features` is a `Box<[FeatureContribution]>`; copy into
    // a `SmallVec<[…; 4]>` matching `Confidence::features` so the inline-4
    // case stays heap-free even after the optional override-marker push.
    let mut features: marque_rules::SmallVec<[FeatureContribution; 4]> =
        marque_rules::SmallVec::from_slice(&provenance.features);
    if corpus_override_active {
        features.push(FeatureContribution {
            id: FeatureId::CorpusOverrideInEffect,
            delta: 0.0,
        });
    }

    // Dispatch on the decoder's `fix_source`. Standard vocab-based
    // recognition emits at `Severity::Fix` with `rule = 1.0` (engine
    // applies whenever `recognition >= confidence_threshold`). The
    // position-aware classification heuristic (issue #133 PR 2) emits
    // at `Severity::Warn` (always-visible in `--check`, non-zero exit
    // code) with `rule = HEURISTIC_RULE_AXIS_CAP = 0.95` matching the
    // default `confidence_threshold`. PR 4's empirical corpus
    // measurement justifies the `0.95` value — see the cap's doc
    // comment for the analysis script and measured numbers.
    let (severity, rule_axis, fix_source) = match provenance.fix_source {
        FixSource::DecoderClassificationHeuristic => (
            Severity::Warn,
            HEURISTIC_RULE_AXIS_CAP,
            FixSource::DecoderClassificationHeuristic,
        ),
        // All non-heuristic decoder paths use the existing posterior
        // shape. Strict-source variants (BuiltinRule, CorrectionsMap,
        // MigrationTable) do not flow through this builder — they
        // come from rule-pipeline emissions, not the decoder — so
        // routing them to `DecoderPosterior` here is a defensive
        // default that preserves the existing strict-decoder shape
        // for any future fix-source variant.
        _ => (Severity::Fix, 1.0, FixSource::DecoderPosterior),
    };

    let confidence = Confidence {
        recognition: provenance.recognition_score(),
        rule: rule_axis,
        region: None,
        runner_up_ratio: provenance.runner_up_ratio,
        features,
    };
    let rule = RuleId::new(DECODER_RULE_ID);
    // Audit-shape contract: the decoder-path R001 record carries no
    // document bytes (Constitution V Principle V / G13). The span
    // identifies *where* the fix landed; the engine's synthesis path
    // re-renders the canonical form from a `Recanonicalize` intent at
    // promotion time. The unused `original` / `replacement` bindings
    // document that we held UTF-8 validity for the input + canonical
    // bytes but intentionally do not route them into the audit record.
    let _ = (original, replacement);
    use marque_scheme::{ReplacementIntent, fix_intent::RecanonScope};
    let intent = FixIntent::<CapcoScheme> {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
        },
        confidence,
        feature_ids: SmallVec::new(),
        message: marque_rules::Message::new(
            marque_rules::MessageTemplate::BannerRollupMismatch,
            marque_rules::MessageArgs::default(),
        ),
        source: fix_source,
        migration_ref: None,
    };
    Some(Diagnostic::with_fix_at_span(
        rule,
        severity,
        span,
        span,
        format!(
            "decoder-recognized canonical form at bytes {}..{}",
            span.start, span.end
        ),
        DECODER_CITATION,
        intent,
    ))
}

/// Build the synthetic `R002 reparse-failed` diagnostic the engine
/// emits when the post-pass-1 buffer cannot be re-parsed (PR 7b, FR-024).
///
/// R002 is a **diagnostic, never an [`AppliedFix`]** (Constitution V
/// Principle V): it has no replacement, no intent, no fix proposal.
/// The contributing pass-1 fixes DO produce `AppliedFix` records (they
/// applied successfully — the audit log is honest about what landed);
/// R002 sits alongside them in `FixResult.remaining_diagnostics` to
/// explain why pass-2 was skipped.
///
/// # Failure-span semantics
///
/// `failure_span` identifies the locus of the re-parse failure. The
/// parser cannot always localize the failure to a single byte range,
/// so the engine passes `Span::new(0, post_pass_1_buffer.len())` — a
/// document-wide sentinel — when no narrower span is available. A
/// renderer that wants to highlight the failure region can detect the
/// sentinel by comparing against the buffer length.
///
/// # Audit-content-ignorance (Constitution V Principle V / G13)
///
/// The diagnostic carries:
/// - [`R002_RULE_ID`] (permitted identifier)
/// - [`Span`] (permitted identifier — byte offsets only)
/// - A short fixed message string with the contributing rule IDs
///   interpolated by `RuleId::as_str()` (permitted identifiers, each
///   a CAPCO rule ID like `"C001"` / `"E006"` — token canonicals from
///   a closed vocabulary)
///
/// No document bytes flow through R002.
///
/// # Deferred wire-up to `MessageArgs`
///
/// The structured `MessageArgs.contributing_rule_ids` field (PM
/// D-7.5 / D-7.17) is plumbed at the type level today — the
/// closed-set destructure-pin test at
/// `crates/rules/tests/message_args_closed_set.rs` enforces its
/// presence. The R002 `Diagnostic` currently embeds the contributing
/// rule IDs in its `Box<str>` message because the
/// `Diagnostic.message: Box<str>` → `Message<MessageTemplate>`
/// migration lands in PR 3c.2. When that migration completes, this
/// function will construct
/// `Message::new(MessageTemplate::ReparseFailed,
/// MessageArgs { contributing_rule_ids, .. })` instead of formatting
/// the IDs inline. The `contributing_rule_ids` parameter is shipped
/// in the typed form already (`SmallVec<[RuleId; 4]>`, never a string
/// list) so PR 3c.2's migration is purely additive.
///
/// # Why no `__engine_promote` call
///
/// `__engine_promote` mints an `AppliedFix` audit record. R002 is not
/// an applied fix — pass-1 fixes landed, pass-2 fixes did not, and
/// R002 carries no bytes to apply. Promoting R002 would inject a
/// false-positive audit record claiming a fix was applied when none
/// was. The audit log integrity invariant (Constitution V Principle V)
/// forbids it.
pub(crate) fn build_r002_diagnostic(
    contributing_rule_ids: SmallVec<[RuleId; 4]>,
    failure_span: Span,
) -> Diagnostic<CapcoScheme> {
    // Render the contributing rule IDs as a short comma-separated
    // list in the message string. Each entry is a CAPCO rule ID
    // (permitted identifier per Constitution V); no input bytes.
    // PR 3c.2 will migrate this inline rendering to a
    // `Message::new(MessageTemplate::ReparseFailed, MessageArgs { .. })`
    // construction — see the function's "Deferred wire-up" doc section.
    let mut rule_list = String::new();
    for (i, id) in contributing_rule_ids.iter().enumerate() {
        if i > 0 {
            rule_list.push_str(", ");
        }
        rule_list.push_str(id.as_str());
    }

    let message = if rule_list.is_empty() {
        "post-pass-1 buffer failed to re-parse; pass-2 skipped".to_string()
    } else {
        format!(
            "post-pass-1 buffer failed to re-parse after applying \
             pass-1 fixes from {rule_list}; pass-2 skipped"
        )
    };

    Diagnostic::new(
        R002_RULE_ID,
        Severity::Error,
        failure_span,
        message,
        R002_CITATION,
        None,
    )
}

/// `Confidence::rule` cap for the position-aware classification
/// heuristic (`FixSource::DecoderClassificationHeuristic`). Pinned
/// at `0.95` matching the default `confidence_threshold` — solo-
/// candidate heuristic fixes auto-apply at the default threshold;
/// multi-candidate cases (heuristic plus a competing recovery)
/// drop below `0.95` because `recognition` falls with the runner-
/// up margin and the user retains agency to verify. The diagnostic
/// is always emitted at [`Severity::Warn`](marque_rules::Severity::Warn)
/// regardless of confidence, so `--check` exits non-zero whenever
/// the heuristic fires.
///
/// # Empirical justification (issue #133 PR 4)
///
/// The relevant FP rate isn't "trigger appears in arbitrary prose"
/// but "trigger appears as a standalone token in a context that
/// also contains marking-shape signals (`//` outside URLs, or any
/// CAPCO marking long-form like `NOFORN`/`SECRET`/`REL TO`/etc.)
/// within proximity" — because the decoder heuristic only fires
/// when the strict parse fails on input that's already
/// marking-shaped. PR 2's initial guess of `0.80` was based on the
/// reading "we can't be 97% sure"; PR 4 measured the conditional
/// FP rate against the full Enron corpus and confirmed the
/// in-context heuristic is well-calibrated above `0.95`.
///
/// Headline numbers from the committed evidence file
/// (`tools/corpus-analysis/output/heuristic_frequencies.json`,
/// case-insensitive scan over 510,596 Enron documents — case-
/// insensitive because the decoder uppercases inputs before running
/// the heuristic, so a runtime-faithful measurement must capture
/// lowercase trigger appearances too):
///
/// - **11 of 37 triggers** have zero marking-context hits across
///   the corpus (the case-sensitive prior measurement reported
///   23/37, but those numbers undercounted the runtime distribution).
/// - The worst-case per-occurrence in-context rate is `V` at
///   814/23,331 ≈ 3.49% (`V`→`C` heuristic). Interpreted as "of
///   every 100 standalone `V` tokens in body text, ~3.5 sit
///   within ~30 chars of a marking-shape signal." Corresponds to
///   ~96.5% per-occurrence precision — still above the 0.95 cap,
///   though with thinner headroom than the prior measurement
///   showed.
/// - Most other non-zero triggers stay below ~1.5% per-occurrence
///   (A: 0.15%, E: 0.34%, RE: 0.19%, W: 0.94%, F: 0.50%, etc.).
///
/// **Cap calibration**: the 0.95 cap is justified by the measured
/// per-occurrence in-context rates above. Two prior framings of
/// this paragraph (a "5,000-file sample" with hand-derived numbers
/// and a "Bayesian credible upper bound ≥ 99.94%" calculation) were
/// dropped because (a) the sample numbers were superseded by the
/// full-corpus measurement, and (b) the Bayesian calculation used
/// a different denominator (`marking_context / total_docs`) than
/// the per-occurrence rate (`marking_context / unrestricted`),
/// making them not directly comparable. Use the measured per-
/// occurrence rates directly.
///
/// **Important caveat — loose upper bound**: the per-occurrence rate
/// is an UPPER BOUND on the heuristic's true FP rate, not the rate
/// itself. The metric counts "trigger token appears within ~30 chars
/// of a marking signal," which catches every potential heuristic-
/// fire input but ALSO includes many that the
/// [`try_classification_heuristic_fix`](crate::decoder)
/// guards (lone-input check, leading-position requirement,
/// multi-token-after-leading-position requirement) would filter out
/// before the heuristic ever fires. The true FP rate is likely well
/// below the worst-case 3.49% bound — but if real-world deployment
/// shows V-shaped triggers producing too many false positives, the
/// per-trigger plumbing originally proposed for PR 4 should land
/// (skip-list V, drop its rule confidence, etc.).
///
/// Spot-check the evidence file for per-trigger detail; this doc
/// summarizes qualitatively to avoid drift if the file is
/// regenerated against a different corpus.
///
/// To re-measure (e.g., when a different corpus is added):
///
/// ```text
/// python3 tools/corpus-analysis/analyze.py \
///     --mode heuristic-frequency \
///     --output tools/corpus-analysis/output/heuristic_frequencies.json
/// ```
///
/// If a future measurement shows a trigger's marking-context FP
/// rate above ~1% (e.g., a corpus that contains heavy use of one
/// of these tokens in a marking-adjacent way), this cap should
/// drop or the per-trigger plumbing originally proposed for PR 4
/// should land. Pinned at the engine boundary by
/// `engine::tests::heuristic_rule_axis_cap_matches_default_threshold`.
const HEURISTIC_RULE_AXIS_CAP: f32 = 0.95;

// ---------------------------------------------------------------------------
// Rule-override canonicalization (task #49)
// ---------------------------------------------------------------------------

/// Pass-1 (Localized) rule-index partition. Each entry indexes back
/// into `Engine::rule_sets[i].rules()[j]` as `(i, j)`. Inline-4
/// because the production CAPCO ruleset has 4 Localized rules; future
/// schemes are expected to stay in the same order of magnitude.
type Pass1Indices = SmallVec<[(usize, usize); 4]>;
/// Pass-2 (WholeMarking) rule-index partition. Inline-32 covers the
/// current 27-rule whole-marking subset; the SmallVec spills to the
/// heap at the 33rd entry, leaving 5 entries of headroom. The
/// rule-collapse trajectory (PR 3b retired 13 rules into walkers;
/// end-state target ~10 across all 4 stages) means the count is
/// contracting, so 32 stays comfortable. See [`Engine::pass2_rule_indices`]
/// for the same rationale at greater length.
type Pass2Indices = SmallVec<[(usize, usize); 32]>;

/// Partition the registered rules by their declared [`Phase`] (FR-021).
///
/// Returns `(pass1, pass2)` where each entry is a
/// `(rule_set_index, rule_index_within_set)` pair indexing back into
/// the caller's `rule_sets[i].rules()[j]`. `pass1` enumerates every
/// [`Phase::Localized`] rule; `pass2` enumerates every
/// [`Phase::WholeMarking`] rule. Together they cover every registered
/// rule exactly once — the trait method is total over `Phase`'s two
/// variants.
///
/// Walked once at [`Engine::with_clock`] time and cached on the engine.
/// Per-document `fix` dispatch reads `pass1_rule_indices` via
/// [`TwoPassFixer::localized_rule_id_set`] (PR 7b); the walk does not
/// re-run. `pass2_rule_indices` is stored for PR 7c, when pass-2 will
/// switch from complement-of-pass-1 to a positive whitelist for
/// symmetry with pass-1 — see [`Engine::pass2_rule_indices`] for the
/// rationale.
fn partition_rules_by_phase(
    rule_sets: &[Box<dyn RuleSet<CapcoScheme>>],
) -> (Pass1Indices, Pass2Indices) {
    let mut pass1: Pass1Indices = SmallVec::new();
    let mut pass2: Pass2Indices = SmallVec::new();
    for (set_idx, rule_set) in rule_sets.iter().enumerate() {
        for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
            match rule.phase() {
                Phase::Localized => pass1.push((set_idx, rule_idx)),
                Phase::WholeMarking => pass2.push((set_idx, rule_idx)),
            }
        }
    }
    (pass1, pass2)
}

/// Resolve every key in `config.rules.overrides` against the registered
/// rule sets. Both the rule ID (`"E001"`) and the rule name
/// (`"portion-mark-in-banner"`) are accepted — after canonicalization
/// the override map keys by canonical ID only, and the per-rule lookup
/// in `lint()` / `fix_inner()` keeps working unchanged.
///
/// Fails closed on:
/// - **Unknown keys** — `E999 = "warn"` or `not-a-rule = "error"` → the
///   user has almost certainly typo'd a rule reference. Silent acceptance
///   (the pre-#49 behavior) means the user thought they were configuring
///   the rule, but nothing happened at lint time. Emits
///   `EngineConstructionError::UnknownRuleOverride` with a best-effort
///   `did_you_mean` suggestion (Levenshtein ≤ 3 against the union of
///   known IDs and names).
/// - **Conflicting duplicate forms** — `E001 = "warn"` AND
///   `portion-mark-in-banner = "error"` in the same merged config →
///   the two entries resolved to the same rule but with different
///   severities. One form would have silently won the HashMap race.
///   Emits `EngineConstructionError::ConflictingRuleOverride`.
///
/// Duplicate forms with the *same* severity are silently accepted —
/// a user writing both `E001 = "warn"` and `portion-mark-in-banner =
/// "warn"` (intentionally or via copy-paste across config layers) gets
/// the expected behavior.
fn canonicalize_rule_overrides(
    config: &mut Config,
    rule_sets: &[Box<dyn RuleSet<CapcoScheme>>],
    scheme: &CapcoScheme,
) -> Result<(), EngineConstructionError> {
    if config.rules.overrides.is_empty() {
        return Ok(());
    }

    // Build the ID-and-name → canonical-ID lookup. Both sides live in
    // `&'static str` (RuleId's inner slice, rule.name()), so the map's
    // keys and values are all `'static`.
    //
    // Dispatcher walkers like `BannerMatchesProjectedRule` (T026a)
    // register under one bookkeeping ID but emit diagnostics under
    // per-row catalog IDs (E035 / E040 in addition to E031). The
    // walker advertises those catalog (id, name) pairs through
    // `Rule::additional_emitted_ids`; each pair becomes a self-
    // canonical entry so a `.marque.toml` configuring the catalog ID
    // (`E035 = "warn"`) is accepted instead of failing as
    // `UnknownRuleOverride`. The per-emitted-id severity-override
    // path at lint time then resolves the override against the
    // diagnostic's emitted `rule` field.
    let mut known: HashMap<&'static str, &'static str> = HashMap::new();
    for rule_set in rule_sets {
        for rule in rule_set.rules() {
            let id_str = rule.id().as_str();
            let name = rule.name();
            known.insert(id_str, id_str);
            known.insert(name, id_str);
            // Catalog IDs / names from dispatcher walkers — each
            // entry maps to itself so config that names the catalog
            // ID directly resolves to that ID (not the walker's
            // bookkeeping ID), preserving per-row override scope.
            for (catalog_id, catalog_name) in rule.additional_emitted_ids() {
                known.insert(catalog_id, catalog_id);
                known.insert(catalog_name, catalog_id);
            }
        }
    }
    // PR 3c.B Commit 7.3 + 7.4: rule IDs emitted by the engine's
    // constraint-catalog bridge that have no corresponding registered
    // `Rule` impl. The bridge folds `E058/...` / `class-floor/...`
    // constraint labels to `Diagnostic.rule = "E058"` (the
    // ConstraintViolation envelope path), and emits
    // `Diagnostic.rule = "E059"` from the direct
    // `bridge_sci_per_system_diagnostics` path. Both walker `Rule`s
    // that used to advertise these IDs retired in 7.3 and 7.4, so
    // the canonicalizer needs an explicit handle on the bridge-
    // emitted ID set or `[rules] E058 = "off"` / `[rules] E059 = "off"`
    // configs fail `UnknownRuleOverride`. Same shape as
    // `Rule::additional_emitted_ids` — the bridge is just a
    // non-`Rule` emitter that participates in the same registration
    // convention.
    for (bridge_id, bridge_name) in scheme.bridge_emitted_rule_ids() {
        known.insert(bridge_id, bridge_id);
        known.insert(bridge_name, bridge_id);
    }

    // Walk the raw overrides; resolve each key to its canonical ID, and
    // track which source key contributed each canonical entry so we can
    // report both sides of a conflict.
    let raw = std::mem::take(&mut config.rules.overrides);
    let mut by_rule: HashMap<&'static str, (String, String)> = HashMap::new();
    for (key, value) in raw {
        match known.get(key.as_str()) {
            Some(&canonical_id) => {
                if let Some((prev_key, prev_sev)) = by_rule.get(canonical_id) {
                    if prev_sev != &value {
                        return Err(EngineConstructionError::ConflictingRuleOverride {
                            rule_id: canonical_id.to_owned(),
                            keys: Box::new([prev_key.clone(), key]),
                            severities: Box::new([prev_sev.clone(), value]),
                        });
                    }
                    // Duplicate form, same severity — accept silently.
                } else {
                    by_rule.insert(canonical_id, (key, value));
                }
            }
            None => {
                let did_you_mean = suggest_closest(&key, known.keys().copied());
                return Err(EngineConstructionError::UnknownRuleOverride { key, did_you_mean });
            }
        }
    }

    config.rules.overrides = by_rule
        .into_iter()
        .map(|(id, (_, sev))| (id.to_owned(), sev))
        .collect();
    Ok(())
}

/// Best-effort string extraction from a `catch_unwind` payload.
///
/// Rust panic payloads are `Box<dyn Any + Send>`. The standard
/// shapes a `panic!()` produces are `&'static str` (literal message)
/// and `String` (formatted message); arbitrary types are also
/// permissible. We try the two common cases and fall back to a
/// generic placeholder so the warning we emit always carries
/// *something* identifying the rule even if a future crate panics
/// with a custom payload type.
fn panic_payload_to_string(
    payload: &Box<dyn std::any::Any + Send + 'static>,
) -> std::borrow::Cow<'static, str> {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        std::borrow::Cow::Borrowed(*s)
    } else if let Some(s) = payload.downcast_ref::<String>() {
        std::borrow::Cow::Owned(s.clone())
    } else {
        std::borrow::Cow::Borrowed("<unstringifiable panic payload>")
    }
}

/// Return the closest known rule key (ID or name) to `needle` by
/// Levenshtein distance, if the closest candidate is within a small
/// edit-distance threshold. Threshold scales with `needle.len()`: short
/// strings only match on ≤ 1 edit, longer strings tolerate more.
///
/// Returns `None` when no candidate is close enough to be useful —
/// "did you mean 'REL-TO-noforn-supersession'?" for a user who typed
/// "E999" would be worse than no suggestion at all.
fn suggest_closest<'a, I>(needle: &str, candidates: I) -> Option<String>
where
    I: Iterator<Item = &'a str>,
{
    // Keep the threshold tight so we don't suggest matches that share
    // only a couple of characters. The max-distance formula mirrors
    // what rustc uses for its "did you mean" hints:
    //   - length 0–3: 1 edit max (too short to suggest at all, really)
    //   - length 4–7: 2 edits max
    //   - length 8+:  3 edits max
    let max_distance = match needle.len() {
        0..=3 => 1,
        4..=7 => 2,
        _ => 3,
    };

    let mut best: Option<(&'a str, usize)> = None;
    for cand in candidates {
        let dist = levenshtein(needle, cand);
        if dist > max_distance {
            continue;
        }
        match best {
            Some((_, prev_dist)) if dist >= prev_dist => {}
            _ => best = Some((cand, dist)),
        }
    }
    best.map(|(cand, _)| cand.to_owned())
}

/// Levenshtein edit distance between two byte strings. Small, inlineable,
/// no external dependency — the engine crate is on the WASM-safe surface
/// and adding a new runtime dep for a once-per-construction helper would
/// be a disproportionate trade (Constitution III).
///
/// Operates on bytes, not `char`s: rule IDs and names are ASCII by
/// construction, so the byte-level diff equals the codepoint-level diff.
fn levenshtein(a: &str, b: &str) -> usize {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    // Two-row DP: only the previous row is needed at any step.
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use marque_ism::CanonicalAttrs;
    use marque_rules::{
        Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Rule, RuleContext,
        RuleId, RuleSet, Severity,
    };
    use marque_scheme::ReplacementIntent;
    use marque_scheme::fix_intent::RecanonScope;
    use std::time::{Duration, UNIX_EPOCH};

    /// A pure-test stand-in for the old `FixProposal` shape: the
    /// fields engine tests actually exercise (rule, span, replacement,
    /// confidence). The engine pipeline post Commit 10 takes
    /// `FixIntent<S>` exclusively, so `StubRule` synthesizes a
    /// `Recanonicalize` intent + a separate Diagnostic per
    /// `StubProposal` and the engine's `synthesize_fixes` path runs
    /// the recanonicalization through a stub-scheme override.
    ///
    /// Tests that need byte-precise replacement assertions install a
    /// `replacement` here and assert against the engine output after
    /// fix application — they don't reach into the audit-record
    /// `proposal` shape.
    #[derive(Debug, Clone)]
    pub(super) struct StubProposal {
        pub rule: RuleId,
        pub span: Span,
        pub replacement: Box<str>,
        pub confidence: Confidence,
        pub source: FixSource,
    }

    #[test]
    fn heuristic_rule_axis_cap_matches_default_threshold() {
        // Issue #133 PR 4 invariant: the position-aware classification
        // heuristic's `Confidence::rule` cap is pinned at the default
        // `confidence_threshold` (0.95). Solo-candidate heuristic
        // fixes auto-apply at the default threshold; the empirical
        // corpus measurement (see `HEURISTIC_RULE_AXIS_CAP` doc and
        // `tools/corpus-analysis/output/heuristic_frequencies.json`)
        // justifies confidence ≥ 99.4% per-trigger, comfortably above
        // the cap.
        //
        // If a future change drops `HEURISTIC_RULE_AXIS_CAP` below
        // `Config::default().confidence_threshold()`, that's a
        // behavioral regression: heuristic fixes that previously auto-
        // applied at the default threshold would silently stop
        // applying, and the user-visible "fix-and-warn" surface
        // collapses to "warn-only-without-fix" without an explicit
        // intent recorded in the change.
        //
        // If a future change drops the default `confidence_threshold`
        // below `HEURISTIC_RULE_AXIS_CAP`, that's the inverse problem:
        // the heuristic suddenly becomes more aggressive than the
        // governance signal we agreed on. Either way, the equality
        // pin here forces a coordinated decision.
        let default_threshold = Config::default().confidence_threshold();
        assert!(
            (HEURISTIC_RULE_AXIS_CAP - default_threshold).abs() < 1e-6,
            "HEURISTIC_RULE_AXIS_CAP={HEURISTIC_RULE_AXIS_CAP} must equal \
             Config::default().confidence_threshold()={default_threshold}; \
             a divergence requires an intentional governance change recorded \
             in the cap's doc comment"
        );
    }

    /// A test rule that emits text-correction diagnostics directly
    /// (via `Diagnostic::text_correction`). Engine tests use this to
    /// exercise the fix-application + audit-promotion path without
    /// needing a real CAPCO scheme + `apply_intent` + `render_*`
    /// roundtrip. The promotion lands on
    /// `AppliedFix::__engine_promote_text_correction` via the engine's
    /// `apply_text_corrections` path, which the test's
    /// `text_correction`-bearing diagnostic feeds. The resulting
    /// `AppliedFixProposal::TextCorrection { replacement }` carries
    /// the canonical bytes for assertions.
    struct StubRule {
        id: &'static str,
        proposals: Vec<StubProposal>,
    }

    impl Rule<CapcoScheme> for StubRule {
        fn id(&self) -> RuleId {
            RuleId::new(self.id)
        }
        fn name(&self) -> &'static str {
            "stub"
        }
        fn default_severity(&self) -> Severity {
            Severity::Fix
        }
        fn check(
            &self,
            _attrs: &CanonicalAttrs,
            _ctx: &RuleContext,
        ) -> Vec<Diagnostic<CapcoScheme>> {
            // Emit text-correction diagnostics: the C001 path is the
            // only fix channel that carries byte-precise replacement
            // bytes the engine actually applies. Engine tests
            // exercise the application + C-1 overlap-guard +
            // remaining-diagnostics path through this channel.
            //
            // For sub-threshold proposals also attach a structural
            // FixIntent so the lint post-pass demotes the severity
            // to Suggest (the demotion gate consults
            // `d.fix.confidence`, not `text_correction`).
            self.proposals
                .iter()
                .map(|p| {
                    let mut d = Diagnostic::text_correction(
                        p.rule.clone(),
                        Severity::Fix,
                        p.span,
                        "stub",
                        "TEST",
                        p.replacement.clone(),
                        p.source,
                        p.confidence.clone(),
                        None,
                    );
                    if p.confidence.combined() < 1.0 {
                        d.fix = Some(FixIntent::<CapcoScheme> {
                            replacement: ReplacementIntent::Recanonicalize {
                                scope: RecanonScope::Portion,
                            },
                            confidence: p.confidence.clone(),
                            feature_ids: SmallVec::new(),
                            message: Message::new(
                                MessageTemplate::BannerRollupMismatch,
                                MessageArgs::default(),
                            ),
                            source: FixSource::BuiltinRule,
                            migration_ref: None,
                        });
                    }
                    d
                })
                .collect()
        }
    }

    struct StubSet(Vec<Box<dyn Rule<CapcoScheme>>>);
    impl RuleSet<CapcoScheme> for StubSet {
        fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
            &self.0
        }
        fn schema_version(&self) -> &'static str {
            "TEST"
        }
    }

    fn proposal(rule: &'static str, start: usize, end: usize, replacement: &str) -> StubProposal {
        proposal_with_confidence(rule, start, end, replacement, 1.0)
    }

    fn proposal_with_confidence(
        rule: &'static str,
        start: usize,
        end: usize,
        replacement: &str,
        confidence: f32,
    ) -> StubProposal {
        StubProposal {
            rule: RuleId::new(rule),
            span: Span::new(start, end),
            replacement: replacement.into(),
            confidence: marque_rules::Confidence::strict(confidence),
            source: FixSource::CorrectionsMap,
        }
    }

    fn engine_with(proposals: Vec<StubProposal>) -> Engine {
        engine_with_config(Config::default(), proposals)
    }

    fn engine_with_config(config: Config, proposals: Vec<StubProposal>) -> Engine {
        let stub = StubRule {
            id: "TEST",
            proposals,
        };
        let set: Box<dyn RuleSet<CapcoScheme>> = Box::new(StubSet(vec![Box::new(stub)]));
        Engine::with_clock(
            config,
            vec![set],
            marque_capco::scheme::CapcoScheme::new(),
            Box::new(FixedClock::new(
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )),
        )
        .expect("default CAPCO scheme has no rewrite cycles")
    }

    /// A source long enough to span the test fix offsets, AND containing a
    /// banner marking so the parser produces a candidate that triggers
    /// the rule loop in `Engine::lint`.
    const TEST_SRC: &[u8] = b"SECRET//NOFORN                                                ";

    #[test]
    fn fix_applies_disjoint_fixes_in_reverse_order() {
        // Two non-overlapping fixes; FR-016 sorts by span.end DESC so the
        // later one is applied first, preserving the earlier span's offsets.
        let engine = engine_with(vec![
            proposal("E001", 0, 6, "AA"),  // "SECRET" → "AA"
            proposal("E002", 8, 14, "BB"), // "NOFORN" → "BB"
        ]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        let out = String::from_utf8(result.source).unwrap();
        assert!(out.starts_with("AA//BB"), "got: {out:?}");
        assert_eq!(result.applied.len(), 2);
    }

    #[test]
    fn overlap_guard_drops_overlapping_fix() {
        // Two fixes whose spans collide. C-1: keep one, drop the other.
        let engine = engine_with(vec![
            proposal("E001", 0, 6, "AA"),
            proposal("E002", 3, 10, "BB"), // overlaps E001
        ]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        // Exactly one fix should be applied, the other should remain in
        // `remaining_diagnostics` so callers can see it was not silently
        // dropped.
        assert_eq!(result.applied.len(), 1, "applied: {:?}", result.applied);
        assert_eq!(
            result.remaining_diagnostics.len(),
            1,
            "remaining: {:?}",
            result.remaining_diagnostics
        );
    }

    #[test]
    fn dry_run_returns_original_source_but_records_applied() {
        let engine = engine_with(vec![proposal("E001", 0, 6, "AA")]);
        let result = engine.fix(TEST_SRC, FixMode::DryRun);
        assert_eq!(result.source, TEST_SRC, "dry-run must not mutate source");
        assert_eq!(result.applied.len(), 1);
        assert!(result.applied[0].dry_run, "dry_run flag must be set");
    }

    #[test]
    fn fix_with_threshold_rejects_nan() {
        let engine = engine_with(vec![]);
        assert!(matches!(
            engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(f32::NAN)),
            Err(InvalidThreshold(_))
        ));
    }

    #[test]
    fn fix_with_threshold_rejects_out_of_range() {
        let engine = engine_with(vec![]);
        assert!(matches!(
            engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(-0.1)),
            Err(InvalidThreshold(_))
        ));
        assert!(matches!(
            engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(1.1)),
            Err(InvalidThreshold(_))
        ));
    }

    #[test]
    fn fix_with_threshold_accepts_boundaries() {
        let engine = engine_with(vec![]);
        assert!(
            engine
                .fix_with_threshold(TEST_SRC, FixMode::Apply, Some(0.0))
                .is_ok()
        );
        assert!(
            engine
                .fix_with_threshold(TEST_SRC, FixMode::Apply, Some(1.0))
                .is_ok()
        );
    }

    #[test]
    fn fixed_clock_yields_deterministic_timestamps() {
        let engine = engine_with(vec![proposal("E001", 0, 6, "AA")]);
        let r1 = engine.fix(TEST_SRC, FixMode::Apply);
        let r2 = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(r1.applied[0].timestamp, r2.applied[0].timestamp);
    }

    // H-3: fix_with_threshold must reject non-finite overrides in all
    // directions, not just NaN. INFINITY and NEG_INFINITY are both caught
    // by the range check; this test pins that behavior so a future refactor
    // that uses e.g. `is_finite` instead of `contains + is_nan` cannot
    // silently regress.
    #[test]
    fn fix_with_threshold_rejects_infinity() {
        let engine = engine_with(vec![]);
        assert!(matches!(
            engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(f32::INFINITY)),
            Err(InvalidThreshold(_))
        ));
        assert!(matches!(
            engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(f32::NEG_INFINITY)),
            Err(InvalidThreshold(_))
        ));
    }

    // M-4: the confidence filter at `f.confidence.combined() >= threshold`
    // is on the hot path of Engine::fix. These two tests pin the `>=`
    // semantics so a future refactor that flips it to `>` (or vice versa)
    // is caught. "Confidence" here is the scalar `Confidence::combined()`
    // (= recognition × rule); the other axes (`region`, `runner_up_ratio`,
    // feature contributions) are audit-provenance metadata and do not
    // participate in the threshold gate.
    #[test]
    fn confidence_below_default_threshold_is_excluded() {
        // Config::default().confidence_threshold == 0.95. A fix at 0.94
        // must not be applied.
        let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.94)]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(result.applied.len(), 0);
        // The below-threshold fix is a suggestion — it survives in
        // remaining_diagnostics so the caller can surface it.
        assert_eq!(result.remaining_diagnostics.len(), 1);
    }

    #[test]
    fn lint_rewrites_below_threshold_fix_severity_to_suggest() {
        // Issue #235 / #186 PR-3: the lint post-pass turns a Fix-severity
        // diagnostic carrying a sub-threshold proposal into a Suggest-
        // severity diagnostic, preserving the fix payload so the renderer
        // can show "did you mean?" instead of silently dropping the
        // candidate at the threshold gate.
        let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.5)]);
        let lint = engine.lint(TEST_SRC);
        assert_eq!(lint.diagnostics.len(), 1);
        assert_eq!(lint.diagnostics[0].severity, Severity::Suggest);
        assert!(
            lint.diagnostics[0].fix.is_some(),
            "the candidate fix must stay attached so the renderer can surface it"
        );
        assert_eq!(lint.suggest_count(), 1);
        // Confirm the engine still excludes Suggest from auto-apply.
        let fix_result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(fix_result.applied.len(), 0);
    }

    #[test]
    fn lint_does_not_rewrite_at_threshold_boundary() {
        // A fix at exactly the threshold (0.95) must NOT be rewritten
        // — it is auto-apply territory, not Suggest territory. This
        // pins the boundary semantics: the rewrite predicate is
        // strictly less-than, matching the engine's `>= threshold`
        // application gate.
        let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.95)]);
        let lint = engine.lint(TEST_SRC);
        assert_eq!(lint.diagnostics.len(), 1);
        assert_eq!(lint.diagnostics[0].severity, Severity::Fix);
    }

    #[test]
    fn lint_post_pass_leaves_fix_severity_with_no_fix_payload_alone() {
        // The post-pass guard order matters: even though `Fix`-severity
        // diagnostics are the only ones eligible for the rewrite, a
        // diagnostic that doesn't carry a `FixProposal` (rare in
        // practice — `Fix`-severity rules normally always attach one
        // — but representable in the type) must be skipped by the
        // `let Some(fix) = d.fix.as_ref() else { continue }` arm and
        // keep its `Fix` severity. This pins the behavior so a future
        // refactor that hoists the threshold check above the fix-
        // presence check (and might rewrite to Suggest unconditionally)
        // is caught.
        struct FixWithoutProposalRule;
        impl Rule<CapcoScheme> for FixWithoutProposalRule {
            fn id(&self) -> RuleId {
                RuleId::new("E997")
            }
            fn name(&self) -> &'static str {
                "stub-fix-no-proposal"
            }
            fn default_severity(&self) -> Severity {
                Severity::Fix
            }
            fn check(
                &self,
                _attrs: &CanonicalAttrs,
                _ctx: &RuleContext,
            ) -> Vec<Diagnostic<CapcoScheme>> {
                vec![Diagnostic::info(
                    RuleId::new("E997"),
                    Severity::Fix,
                    Span::new(0, 6),
                    "fix-severity diagnostic with no proposal",
                    "TEST",
                )]
            }
        }

        let set: Box<dyn RuleSet<CapcoScheme>> =
            Box::new(StubSet(vec![Box::new(FixWithoutProposalRule)]));
        let engine = Engine::with_clock(
            Config::default(),
            vec![set],
            marque_capco::scheme::CapcoScheme::new(),
            Box::new(FixedClock::new(
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )),
        )
        .expect("default CAPCO scheme has no rewrite cycles");

        let lint = engine.lint(TEST_SRC);
        assert_eq!(lint.diagnostics.len(), 1);
        assert_eq!(
            lint.diagnostics[0].severity,
            Severity::Fix,
            "Fix-severity diagnostic with no fix payload must NOT be rewritten to Suggest",
        );
        assert!(lint.diagnostics[0].fix.is_none());
    }

    #[test]
    fn fix_excludes_explicit_suggest_severity_from_auto_apply() {
        // Issue #235 / #186 PR-3: a rule that emits at Severity::Suggest
        // directly with confidence ≥ threshold must STILL be excluded
        // from auto-apply by construction. The Suggest channel is a
        // hard "do not apply" signal regardless of the confidence
        // axis. This is the explicit-Suggest invariant; the StubRule
        // emits Fix-severity by default so we route through a custom
        // rule that emits Suggest directly.
        struct SuggestRule;
        impl Rule<CapcoScheme> for SuggestRule {
            fn id(&self) -> RuleId {
                RuleId::new("S999")
            }
            fn name(&self) -> &'static str {
                "stub-suggest"
            }
            fn default_severity(&self) -> Severity {
                Severity::Suggest
            }
            fn check(
                &self,
                _attrs: &CanonicalAttrs,
                _ctx: &RuleContext,
            ) -> Vec<Diagnostic<CapcoScheme>> {
                let intent = FixIntent::<CapcoScheme> {
                    replacement: ReplacementIntent::Recanonicalize {
                        scope: RecanonScope::Portion,
                    },
                    confidence: marque_rules::Confidence::strict(1.0),
                    feature_ids: SmallVec::new(),
                    message: Message::new(
                        MessageTemplate::BannerRollupMismatch,
                        MessageArgs::default(),
                    ),
                    source: FixSource::BuiltinRule,
                    migration_ref: None,
                };
                vec![Diagnostic::with_fix(
                    RuleId::new("S999"),
                    Severity::Suggest,
                    Span::new(0, 6),
                    "explicit suggest with high confidence",
                    "TEST",
                    Some(intent),
                )]
            }
        }

        let set: Box<dyn RuleSet<CapcoScheme>> = Box::new(StubSet(vec![Box::new(SuggestRule)]));
        let engine = Engine::with_clock(
            Config::default(),
            vec![set],
            marque_capco::scheme::CapcoScheme::new(),
            Box::new(FixedClock::new(
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )),
        )
        .expect("default CAPCO scheme has no rewrite cycles");

        let lint = engine.lint(TEST_SRC);
        assert_eq!(lint.diagnostics.len(), 1);
        // Severity stays Suggest (post-pass leaves explicit Suggest alone).
        assert_eq!(lint.diagnostics[0].severity, Severity::Suggest);
        // Even at confidence 1.0, a Suggest-severity fix must not auto-apply.
        let fix_result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(
            fix_result.applied.len(),
            0,
            "explicit Suggest-severity fix must not auto-apply regardless of confidence"
        );
    }

    #[test]
    fn confidence_at_default_threshold_is_included() {
        // A fix at exactly 0.95 must be applied (inclusive threshold).
        let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.95)]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(result.applied.len(), 1);
    }

    // M-5: the zero-length-span filter (`!f.span.is_empty()`) in fix_inner
    // is what masked the Phase 2 Span::new(0, 0) placeholders from the
    // C-1 overlap guard. This test pins that guard explicitly so a future
    // refactor that drops the filter is caught.
    #[test]
    fn zero_length_span_fix_is_filtered_before_sort() {
        let engine = engine_with(vec![proposal("E001", 5, 5, "X")]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(result.applied.len(), 0);
        // Source unchanged: no splice was attempted.
        assert_eq!(result.source, TEST_SRC);
    }

    // L-4: all the other threshold tests go through fix_with_threshold
    // (override path). This exercises the Config-supplied path explicitly
    // so both branches of `fix_with_threshold_inner`'s threshold selection
    // are covered.
    #[test]
    fn config_supplied_threshold_filters_proposals() {
        let mut config = Config::default();
        config.set_confidence_threshold(0.5).unwrap();
        let engine = engine_with_config(
            config,
            vec![
                proposal_with_confidence("E001", 0, 6, "AA", 0.4), // below
                proposal_with_confidence("E002", 8, 14, "BB", 0.6), // above
            ],
        );
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        // Only the 0.6 fix is applied.
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.applied[0].rule.as_str(), "E002");
        // The 0.4 fix surfaces as a remaining diagnostic.
        assert_eq!(result.remaining_diagnostics.len(), 1);
    }

    // Phase 3 Task 2: PageBreak candidates must reset the engine's
    // PageContext accumulator. Without this, banner-validation rules on
    // the second page would see portions from the first page, producing
    // over-restrictive expected aggregates.
    #[test]
    fn lint_handles_multi_page_document_with_form_feed() {
        let src: &[u8] = b"(SECRET//NOFORN) page 1 body.\nSECRET//NOFORN\n\x0c(CONFIDENTIAL) page 2 body.\nCONFIDENTIAL\n";
        let engine = engine_with(vec![]);
        let result = engine.lint(src);
        // Stub rule with no proposals: clean lint, no panic, no parser
        // error from the page-break candidate (which is filtered before
        // parser.parse is called).
        assert!(result.is_clean());
    }

    // F.1: PageContext reset semantics are observable.
    //
    // ContextRecorderRule captures the live `page_context.portion_count()`
    // every time it's invoked. By running the engine over a multi-page
    // document and inspecting the captured counts at each banner candidate,
    // we prove that the engine resets PageContext at the page break instead
    // of accumulating across pages.
    #[derive(Clone)]
    struct ContextRecorderRule {
        observations: std::sync::Arc<std::sync::Mutex<Vec<(marque_ism::MarkingType, usize)>>>,
    }

    impl Rule<CapcoScheme> for ContextRecorderRule {
        fn id(&self) -> RuleId {
            RuleId::new("RECORD")
        }
        fn name(&self) -> &'static str {
            "page-context-recorder"
        }
        fn default_severity(&self) -> Severity {
            Severity::Warn
        }
        fn check(
            &self,
            _attrs: &CanonicalAttrs,
            ctx: &RuleContext,
        ) -> Vec<Diagnostic<CapcoScheme>> {
            let count = ctx
                .page_context
                .as_ref()
                .map(|pc| pc.portion_count())
                .unwrap_or(0);
            self.observations
                .lock()
                .unwrap()
                .push((ctx.marking_type, count));
            vec![]
        }
    }

    struct RecorderSet(Vec<Box<dyn Rule<CapcoScheme>>>);
    impl RuleSet<CapcoScheme> for RecorderSet {
        fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
            &self.0
        }
        fn schema_version(&self) -> &'static str {
            "TEST"
        }
    }

    #[test]
    fn page_context_resets_observably_across_form_feed() {
        use marque_ism::MarkingType;
        let observations = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let rule = ContextRecorderRule {
            observations: std::sync::Arc::clone(&observations),
        };
        let set: Box<dyn RuleSet<CapcoScheme>> = Box::new(RecorderSet(vec![Box::new(rule)]));
        let engine = Engine::with_clock(
            Config::default(),
            vec![set],
            marque_capco::scheme::CapcoScheme::new(),
            Box::new(FixedClock::new(
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )),
        )
        .expect("default CAPCO scheme has no rewrite cycles");

        // Two pages, separated by a form feed:
        //   Page 1: one portion + one banner
        //   Page break (\f)
        //   Page 2: one portion + one banner
        //
        // The recorder fires on every candidate that reaches the rule loop.
        // For the page-1 banner we expect to see 1 accumulated portion.
        // For the page-2 banner we expect to see 1 accumulated portion
        // (NOT 2) — the form feed must have reset the context.
        let src: &[u8] = b"(SECRET//NF) p1 text\nSECRET//NOFORN\n\x0c(CONFIDENTIAL//NF) p2\nCONFIDENTIAL//NOFORN\n";
        let _ = engine.lint(src);

        let obs = observations.lock().unwrap();
        // The recorder ran once per non-PageBreak candidate. Filter to
        // banners and check the page_context count each banner saw.
        let banner_counts: Vec<usize> = obs
            .iter()
            .filter(|(kind, _)| *kind == MarkingType::Banner)
            .map(|(_, count)| *count)
            .collect();
        assert_eq!(
            banner_counts.len(),
            2,
            "expected 2 banner observations, got: {obs:?}"
        );
        assert_eq!(
            banner_counts[0], 1,
            "page-1 banner should see 1 accumulated portion"
        );
        assert_eq!(
            banner_counts[1], 1,
            "page-2 banner should see 1 accumulated portion (the page-1 \
             portion must be cleared by the form feed)"
        );
    }

    #[test]
    fn page_context_lint_starts_fresh_on_each_call() {
        // Calling Engine::lint twice on the same engine must produce a
        // fresh PageContext for the second call — no cross-call accumulation.
        use marque_ism::MarkingType;
        let observations = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let rule = ContextRecorderRule {
            observations: std::sync::Arc::clone(&observations),
        };
        let set: Box<dyn RuleSet<CapcoScheme>> = Box::new(RecorderSet(vec![Box::new(rule)]));
        let engine = Engine::with_clock(
            Config::default(),
            vec![set],
            marque_capco::scheme::CapcoScheme::new(),
            Box::new(FixedClock::new(
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )),
        )
        .expect("default CAPCO scheme has no rewrite cycles");
        let src: &[u8] = b"(SECRET//NF) text\nSECRET//NOFORN\n";
        let _ = engine.lint(src);
        let _ = engine.lint(src);

        let obs = observations.lock().unwrap();
        // Both calls should see identical observations — if the second
        // call leaked state from the first, the page-2 banner_count would
        // double.
        let banner_counts: Vec<usize> = obs
            .iter()
            .filter(|(kind, _)| *kind == MarkingType::Banner)
            .map(|(_, count)| *count)
            .collect();
        assert_eq!(
            banner_counts.len(),
            2,
            "two lint calls should produce two banner observations"
        );
        assert_eq!(banner_counts, vec![1, 1]);
    }

    // M6: FR-016 tiebreaker — same span, different rule IDs.
    // The sort is (span.end DESC, span.start DESC, rule_id ASC, replacement ASC).
    // When two fixes target the exact same span, rule_id ASC breaks the tie,
    // and C-1 drops the second (overlapping) fix.
    #[test]
    fn fr016_same_span_different_rule_ids_picks_lower_rule_id() {
        use marque_rules::AppliedFixProposal;
        // Two proposals for span 0..6 with different rule IDs.
        // "C001" < "E001" lexicographically, so C001 is kept and E001 dropped.
        let engine = engine_with(vec![
            proposal("E001", 0, 6, "BB"),
            proposal("C001", 0, 6, "AA"),
        ]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.applied[0].rule.as_str(), "C001");
        match &result.applied[0].proposal {
            AppliedFixProposal::TextCorrection { replacement } => {
                assert_eq!(replacement.as_str(), "AA");
            }
            other => panic!("expected TextCorrection, got {other:?}"),
        }
    }

    // FR-016 tiebreaker — same span, same rule ID, different replacements.
    #[test]
    fn fr016_same_span_same_rule_picks_lower_replacement() {
        use marque_rules::AppliedFixProposal;
        let engine = engine_with(vec![
            proposal("E001", 0, 6, "ZZZ"),
            proposal("E001", 0, 6, "AAA"),
        ]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(result.applied.len(), 1);
        match &result.applied[0].proposal {
            AppliedFixProposal::TextCorrection { replacement } => {
                assert_eq!(replacement.as_str(), "AAA");
            }
            other => panic!("expected TextCorrection, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // T026a (PR 3b Sub-move A) — per-emitted-id severity-override propagation
    // -----------------------------------------------------------------------
    //
    // The walker collapse changed the engine's configured-severity override
    // to key on each emitted diagnostic's `rule` ID (`d.rule.as_str()`)
    // instead of the registered rule's `id()`. The byte-equivalence claim
    // for non-walker rules holds when each rule's `default_severity()`
    // matches what `check()` emits — true for every existing CAPCO rule
    // by convention. These tests pin the post-change correctness of the
    // resolution path against a real `CapcoRuleSet`-driven engine, so a
    // future regression that quietly stops honoring per-emitted-id
    // overrides is caught at the engine layer (not only at the
    // walker-specific test surface).

    /// Triggers the SAR row of `BannerMatchesProjectedRule` (E031): a
    /// portion introduces SAR-CD; the banner has only SAR-BP. The
    /// walker emits one diagnostic with `Diagnostic.rule == "E031"`.
    /// Same fixture shape as the `crates/capco/tests/banner_rollup_walker.rs`
    /// behavior tests so a baseline drift on this string is caught here
    /// too.
    const SAR_BANNER_MISSING_PROGRAM: &[u8] =
        b"(S//SAR-BP//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN";

    /// Triggers the SCI row of `BannerMatchesProjectedRule` (E035): a
    /// portion carries SI-G; the banner has bare SI. §H.4 enforces
    /// hierarchy roll-up (no §H.5-style optional carve-out), so the
    /// walker emits one diagnostic with `Diagnostic.rule == "E035"`.
    const SCI_BANNER_MISSING_COMPARTMENT: &[u8] = b"(TS//SI-G//NF)\nTOP SECRET//SI//NOFORN";

    fn capco_engine_with_overrides(pairs: &[(&str, &str)]) -> Engine {
        let mut config = Config::default();
        for (k, v) in pairs {
            config
                .rules
                .overrides
                .insert((*k).to_owned(), (*v).to_owned());
        }
        Engine::new(
            config,
            vec![Box::new(marque_capco::CapcoRuleSet::new())],
            marque_capco::scheme::CapcoScheme::new(),
        )
        .expect("default CAPCO scheme has no rewrite cycles")
    }

    #[test]
    fn lint_propagates_warn_override_to_walker_emitted_e031_diagnostic() {
        // E031 is emitted by `BannerMatchesProjectedRule`'s SAR catalog
        // row. The walker registers under the bookkeeping ID `E031` and
        // emits diagnostics with the per-row ID `E031`. With
        // `E031 = "warn"` configured, the engine's per-emitted-id
        // override path must rewrite the diagnostic's severity from
        // its emitted value (Fix → demoted to Suggest by the post-pass)
        // to Warn.
        //
        // This is the load-bearing invariant of the engine change in
        // commit `refactor(capco,engine): collapse banner roll-up rules
        // into walker (T026a)`. A future regression that quietly
        // re-keys the override on the registered rule's `id()` would
        // still pass for non-walker rules (where registered ID equals
        // emitted ID) but would either lose the per-row override or
        // silently apply the walker's `default_severity()` to E035 /
        // E040 — both of which are the failure modes this test exists
        // to prevent.
        let engine = capco_engine_with_overrides(&[("E031", "warn")]);
        let diagnostics = engine.lint(SAR_BANNER_MISSING_PROGRAM).diagnostics;

        let e031: Vec<&Diagnostic<CapcoScheme>> = diagnostics
            .iter()
            .filter(|d| d.rule.as_str() == "E031")
            .collect();
        assert_eq!(
            e031.len(),
            1,
            "exactly one E031 diagnostic; got {} from full diag list: \
             {diagnostics:?}",
            e031.len(),
        );
        assert_eq!(
            e031[0].severity,
            Severity::Warn,
            "config `E031 = \"warn\"` must propagate to the walker-\
             emitted E031 diagnostic; got severity {:?}",
            e031[0].severity,
        );
    }

    #[test]
    fn lint_propagates_warn_override_to_walker_emitted_e035_diagnostic() {
        // Parallel test for E035 — the SCI row of the walker. E035 is
        // NOT a registered rule ID after T026a (the walker registers
        // under E031 only); a configured `E035 = "warn"` therefore can
        // ONLY take effect through the per-emitted-id override path.
        // This is exactly the case where pre-change behavior diverges
        // from post-change behavior: the prior engine looked up
        // overrides by `rule.id()`, never saw E035 because no
        // registered rule has that ID, and applied the walker's
        // `default_severity()` (Error) to the diagnostic. The post-
        // change path looks up by `d.rule.as_str()`, finds the E035
        // override, and rewrites to Warn.
        //
        // This is the strongest end-to-end pin available for the
        // per-emitted-id override path: there is no way for the test
        // to pass under the pre-change semantics.
        let engine = capco_engine_with_overrides(&[("E035", "warn")]);
        let diagnostics = engine.lint(SCI_BANNER_MISSING_COMPARTMENT).diagnostics;

        let e035: Vec<&Diagnostic<CapcoScheme>> = diagnostics
            .iter()
            .filter(|d| d.rule.as_str() == "E035")
            .collect();
        assert_eq!(
            e035.len(),
            1,
            "exactly one E035 diagnostic; got {} from full diag list: \
             {diagnostics:?}",
            e035.len(),
        );
        assert_eq!(
            e035[0].severity,
            Severity::Warn,
            "config `E035 = \"warn\"` must propagate to the walker-\
             emitted E035 diagnostic via the per-emitted-id override \
             path; got severity {:?}",
            e035[0].severity,
        );
    }

    #[test]
    fn lint_off_override_skips_non_walker_rule_via_fast_path() {
        // Non-walker rule fast path: a rule with empty
        // `additional_emitted_ids()` (i.e., every CAPCO rule except
        // `BannerMatchesProjectedRule`) emits diagnostics only under
        // its registered ID. Configuring that ID to `Off` must skip
        // the rule's `check()` body before invocation — the engine's
        // pre-check fast-path skip restored after the T026a refactor
        // made `check()` always run.
        //
        // PR 3c.B Commit 6 retired E001; this test now exercises the
        // fast-path skip on E002 (`missing-usa-trigraph`), a non-
        // walker rule that fires deterministically on
        // `SECRET//REL TO GBR`. The contract is identical: with
        // `E002 = "off"` configured, the engine must produce zero
        // E002 diagnostics via the fast-path skip.
        let engine = capco_engine_with_overrides(&[("E002", "off")]);
        let diagnostics = engine.lint(b"SECRET//REL TO GBR").diagnostics;
        let e002: Vec<&Diagnostic<CapcoScheme>> = diagnostics
            .iter()
            .filter(|d| d.rule.as_str() == "E002")
            .collect();
        assert!(
            e002.is_empty(),
            "config `E002 = \"off\"` must produce zero E002 \
             diagnostics via the fast-path pre-check skip; got: \
             {e002:?} (full diag list: {diagnostics:?})",
        );

        // Sanity check: without the Off override, E002 fires on the
        // same input.
        let engine_default = capco_engine_with_overrides(&[]);
        let baseline = engine_default.lint(b"SECRET//REL TO GBR").diagnostics;
        let baseline_e002: Vec<&Diagnostic<CapcoScheme>> = baseline
            .iter()
            .filter(|d| d.rule.as_str() == "E002")
            .collect();
        assert!(
            !baseline_e002.is_empty(),
            "fixture sanity check: without Off override, E002 must \
             fire on `SECRET//REL TO GBR`; got: {baseline:?}",
        );
    }

    // -----------------------------------------------------------------------
    // Task #49 — rule-alias canonicalization + fail-loud on unknown keys
    // -----------------------------------------------------------------------

    /// Stub rule with distinct, test-controlled id and name so we can
    /// exercise the alias-resolution logic. The base `StubRule` hardcodes
    /// `name() -> "stub"`, which collides across multiple rules and
    /// doesn't model real CAPCO rules.
    struct NamedStub {
        id: &'static str,
        name: &'static str,
    }

    impl Rule<CapcoScheme> for NamedStub {
        fn id(&self) -> RuleId {
            RuleId::new(self.id)
        }
        fn name(&self) -> &'static str {
            self.name
        }
        fn default_severity(&self) -> Severity {
            Severity::Warn
        }
        fn check(
            &self,
            _attrs: &CanonicalAttrs,
            _ctx: &RuleContext,
        ) -> Vec<Diagnostic<CapcoScheme>> {
            vec![]
        }
    }

    fn named_rule_set(rules: &[(&'static str, &'static str)]) -> Box<dyn RuleSet<CapcoScheme>> {
        let rules: Vec<Box<dyn Rule<CapcoScheme>>> = rules
            .iter()
            .map(|(id, name)| Box::new(NamedStub { id, name }) as Box<dyn Rule<CapcoScheme>>)
            .collect();
        Box::new(StubSet(rules))
    }

    fn config_with_overrides(pairs: &[(&str, &str)]) -> Config {
        let mut config = Config::default();
        for (k, v) in pairs {
            config
                .rules
                .overrides
                .insert((*k).to_owned(), (*v).to_owned());
        }
        config
    }

    #[test]
    fn canonicalize_accepts_rule_id_form_unchanged() {
        let mut config = config_with_overrides(&[("E001", "warn")]);
        let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
        canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
            .expect("should succeed");
        assert_eq!(
            config.rules.overrides.get("E001"),
            Some(&"warn".to_owned()),
            "ID-form override keeps its key"
        );
    }

    #[test]
    fn canonicalize_accepts_rule_name_form_and_resolves_to_id() {
        let mut config = config_with_overrides(&[("portion-mark-in-banner", "error")]);
        let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
        canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
            .expect("should succeed");
        assert_eq!(
            config.rules.overrides.get("E001"),
            Some(&"error".to_owned()),
            "name-form override resolves to canonical ID"
        );
        assert!(
            !config
                .rules
                .overrides
                .contains_key("portion-mark-in-banner"),
            "pre-canonicalization name key must not survive"
        );
    }

    #[test]
    fn canonicalize_rejects_unknown_key_with_suggestion_for_near_miss() {
        let mut config = config_with_overrides(&[("E00l", "warn")]); // lowercase-L, not 1
        let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
        let err = canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new()).unwrap_err();
        match err {
            EngineConstructionError::UnknownRuleOverride { key, did_you_mean } => {
                assert_eq!(key, "E00l");
                assert_eq!(
                    did_you_mean.as_deref(),
                    Some("E001"),
                    "single-character typo should suggest the canonical ID"
                );
            }
            other => panic!("expected UnknownRuleOverride, got {other:?}"),
        }
    }

    #[test]
    fn canonicalize_rejects_unknown_key_without_suggestion_when_nothing_close() {
        // No candidate is within edit distance 3, so did_you_mean must be None
        // — a nonsense suggestion is worse than no suggestion.
        let mut config = config_with_overrides(&[("totally-made-up-rule-name", "error")]);
        let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
        let err = canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new()).unwrap_err();
        match err {
            EngineConstructionError::UnknownRuleOverride { key, did_you_mean } => {
                assert_eq!(key, "totally-made-up-rule-name");
                assert!(
                    did_you_mean.is_none(),
                    "distant misses must not emit a suggestion; got {did_you_mean:?}"
                );
            }
            other => panic!("expected UnknownRuleOverride, got {other:?}"),
        }
    }

    #[test]
    fn canonicalize_rejects_conflicting_id_and_name_forms_with_different_severity() {
        let mut config =
            config_with_overrides(&[("E001", "warn"), ("portion-mark-in-banner", "error")]);
        let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
        let err = canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new()).unwrap_err();
        match err {
            EngineConstructionError::ConflictingRuleOverride {
                rule_id,
                keys,
                severities,
            } => {
                assert_eq!(rule_id, "E001");
                // HashMap iteration order isn't deterministic — verify by set.
                let k: std::collections::HashSet<&str> = keys.iter().map(|s| s.as_str()).collect();
                assert!(k.contains("E001"));
                assert!(k.contains("portion-mark-in-banner"));
                let s: std::collections::HashSet<&str> =
                    severities.iter().map(|s| s.as_str()).collect();
                assert!(s.contains("warn"));
                assert!(s.contains("error"));
            }
            other => panic!("expected ConflictingRuleOverride, got {other:?}"),
        }
    }

    #[test]
    fn canonicalize_accepts_duplicate_forms_with_same_severity() {
        // A user who writes both `E001 = "warn"` and `portion-mark-in-banner
        // = "warn"` (e.g., via copy-paste across layers) is unambiguous and
        // should not be punished.
        let mut config =
            config_with_overrides(&[("E001", "warn"), ("portion-mark-in-banner", "warn")]);
        let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
        canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
            .expect("duplicate forms with same severity must succeed");
        assert_eq!(config.rules.overrides.len(), 1);
        assert_eq!(config.rules.overrides.get("E001"), Some(&"warn".to_owned()));
    }

    #[test]
    fn canonicalize_accepts_overrides_across_multiple_rule_sets() {
        // Two rule sets registered; aliases from each must resolve.
        let mut config = config_with_overrides(&[
            ("portion-mark-in-banner", "error"), // name from set A
            ("M500", "warn"),                    // ID from set B
        ]);
        let sets = vec![
            named_rule_set(&[("E001", "portion-mark-in-banner")]),
            named_rule_set(&[("M500", "some-other-domain-rule")]),
        ];
        canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
            .expect("should succeed");
        assert_eq!(
            config.rules.overrides.get("E001"),
            Some(&"error".to_owned())
        );
        assert_eq!(config.rules.overrides.get("M500"), Some(&"warn".to_owned()));
    }

    #[test]
    fn canonicalize_empty_overrides_is_noop() {
        let mut config = Config::default();
        let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
        canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
            .expect("empty overrides must succeed");
        assert!(config.rules.overrides.is_empty());
    }

    // PR 3c.B Commit 7.3 + 7.4 — bridge-emitted rule IDs (no registered
    // `Rule` impl). The canonicalizer consults
    // `CapcoScheme::bridge_emitted_rule_ids()` so `.marque.toml` keys
    // referencing the retired walker IDs (`E058`, `E059`) or their
    // descriptive aliases (`class-floor-catalog`,
    // `sci-per-system-catalog`) are accepted rather than failing
    // `UnknownRuleOverride`. These tests pin the four key forms +
    // canonical-ID resolution so the bridge path can't silently regress.

    #[test]
    fn canonicalize_accepts_bridge_emitted_e058_id() {
        let mut config = config_with_overrides(&[("E058", "warn")]);
        let sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![];
        canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
            .expect("bridge-emitted E058 ID must be accepted");
        assert_eq!(
            config.rules.overrides.get("E058"),
            Some(&"warn".to_owned()),
            "E058 bridge ID resolves to itself as canonical"
        );
    }

    #[test]
    fn canonicalize_accepts_bridge_emitted_e058_name_alias() {
        let mut config = config_with_overrides(&[("class-floor-catalog", "error")]);
        let sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![];
        canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
            .expect("bridge-emitted `class-floor-catalog` name alias must be accepted");
        assert_eq!(
            config.rules.overrides.get("E058"),
            Some(&"error".to_owned()),
            "name-alias `class-floor-catalog` canonicalizes to `E058`"
        );
        assert!(
            !config.rules.overrides.contains_key("class-floor-catalog"),
            "pre-canonicalization name key must not survive"
        );
    }

    #[test]
    fn canonicalize_accepts_bridge_emitted_e059_id() {
        let mut config = config_with_overrides(&[("E059", "off")]);
        let sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![];
        canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
            .expect("bridge-emitted E059 ID must be accepted");
        assert_eq!(
            config.rules.overrides.get("E059"),
            Some(&"off".to_owned()),
            "E059 bridge ID resolves to itself as canonical"
        );
    }

    #[test]
    fn canonicalize_accepts_bridge_emitted_e059_name_alias() {
        let mut config = config_with_overrides(&[("sci-per-system-catalog", "warn")]);
        let sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![];
        canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
            .expect("bridge-emitted `sci-per-system-catalog` name alias must be accepted");
        assert_eq!(
            config.rules.overrides.get("E059"),
            Some(&"warn".to_owned()),
            "name-alias `sci-per-system-catalog` canonicalizes to `E059`"
        );
        assert!(
            !config
                .rules
                .overrides
                .contains_key("sci-per-system-catalog"),
            "pre-canonicalization name key must not survive"
        );
    }

    #[test]
    fn canonicalize_rejects_legacy_walker_id_with_unknown_rule_override() {
        // Regression guard: the retired walker IDs (E022 / E025 / E027
        // for class-floor; E042-E051 for SCI per-system) MUST NOT be
        // silently accepted as aliases for E058 / E059. Per project
        // memory `feedback_pre_users_no_deprecation_phasing.md` marque
        // is pre-users; legacy ID acceptance would be a deprecation-
        // phasing mechanism we don't carry.
        let mut config = config_with_overrides(&[("E022", "warn")]);
        let sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![];
        let err = canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
            .expect_err("retired legacy ID E022 must NOT be silently aliased to E058");
        match err {
            EngineConstructionError::UnknownRuleOverride { key, .. } => {
                assert_eq!(key, "E022");
            }
            other => panic!("expected UnknownRuleOverride for E022, got {other:?}"),
        }
    }

    #[test]
    fn unknown_rule_override_exit_code_is_dataerr() {
        let err = EngineConstructionError::UnknownRuleOverride {
            key: "E999".into(),
            did_you_mean: None,
        };
        assert_eq!(err.exit_code(), 65, "EX_DATAERR for user-config errors");
    }

    #[test]
    fn conflicting_rule_override_exit_code_is_dataerr() {
        let err = EngineConstructionError::ConflictingRuleOverride {
            rule_id: "E001".into(),
            keys: Box::new(["E001".into(), "portion-mark-in-banner".into()]),
            severities: Box::new(["warn".into(), "error".into()]),
        };
        assert_eq!(err.exit_code(), 65);
    }

    #[test]
    fn rewrite_cycle_exit_code_is_unavailable() {
        // Scheme defects (not user-config errors) stay on EX_UNAVAILABLE.
        use marque_scheme::CategoryId;
        let err = EngineConstructionError::RewriteCycle {
            axis: CategoryId(0),
            members: Box::new(["a", "b"]),
        };
        assert_eq!(err.exit_code(), 69);
    }

    #[test]
    fn levenshtein_matches_reference_values() {
        // Spot-check against hand-computed distances to catch regressions
        // in the DP implementation.
        assert_eq!(super::levenshtein("", ""), 0);
        assert_eq!(super::levenshtein("E001", "E001"), 0);
        assert_eq!(super::levenshtein("E001", "E002"), 1);
        assert_eq!(super::levenshtein("E001", "E00l"), 1);
        assert_eq!(super::levenshtein("kitten", "sitting"), 3);
        assert_eq!(super::levenshtein("", "abc"), 3);
        assert_eq!(super::levenshtein("abc", ""), 3);
    }

    #[test]
    fn suggest_closest_prefers_smaller_distance() {
        let cands = ["E001", "E002", "E010"];
        // "E00l" has dist 1 to E001 and dist 1 to E002 (single substitution),
        // and dist 2 to E010. E001 should win the tie-break because it appears
        // first among the equally close candidates.
        assert_eq!(
            super::suggest_closest("E00l", cands.iter().copied()),
            Some("E001".to_owned())
        );
    }

    #[test]
    fn suggest_closest_returns_none_when_nothing_is_close_enough() {
        let cands = ["portion-mark-in-banner", "missing-usa-trigraph"];
        // Very short needle with no near neighbors — threshold is 1 for
        // length 3, and the closest candidate is many edits away.
        assert!(super::suggest_closest("xyz", cands.iter().copied()).is_none());
    }

    // -------------------------------------------------------------------
    // PR 7b — `build_r002_diagnostic` shape pin (security finding 2)
    // -------------------------------------------------------------------
    //
    // R002 is a synthetic diagnostic emitted when the post-pass-1 buffer
    // cannot be re-parsed. Constitution V Principle V forbids R002 from
    // becoming an `AppliedFix` audit record — R002 carries no replacement
    // bytes and represents no action taken. The function-level pin below
    // exercises `build_r002_diagnostic` directly so the no-promotion
    // contract is enforced at the unit-test layer, not only at the
    // integration layer where R002 can fire today (which is never — no
    // production `Phase::Localized` rule emits a `FixIntent` that could
    // trigger R002 yet). The integration-layer test
    // (`audit_completeness.rs::r002_does_not_mint_applied_fix`) becomes
    // load-bearing when a future Localized rule lands; this unit test
    // makes the shape invariant load-bearing today.

    #[test]
    fn build_r002_diagnostic_returns_diagnostic_not_appliedfix() {
        use smallvec::smallvec;

        let contributing: SmallVec<[RuleId; 4]> =
            smallvec![RuleId::new("C001"), RuleId::new("E006")];
        let failure_span = Span::new(0, 64);
        let diag = super::build_r002_diagnostic(contributing, failure_span);

        // The function returns a `Diagnostic<CapcoScheme>`; the type
        // system already forbids it from being an `AppliedFix`. The
        // assertions below pin the structural shape that complements
        // the type-level guarantee: R002 carries no FixIntent and no
        // TextCorrection — the two channels through which a `Diagnostic`
        // can subsequently be promoted to an `AppliedFix` by the engine.
        assert_eq!(diag.rule, super::R002_RULE_ID);
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.span, failure_span);
        assert!(diag.fix.is_none(), "R002 must carry no FixIntent");
        assert!(
            diag.text_correction.is_none(),
            "R002 must carry no TextCorrection"
        );
        // G13: the message must interpolate only permitted identifiers.
        // The contributing rule IDs are CAPCO rule canonicals, on
        // Constitution V's permitted-identifier list. No document bytes.
        let msg = diag.message.as_ref();
        assert!(msg.contains("C001"));
        assert!(msg.contains("E006"));
    }

    #[test]
    fn build_r002_diagnostic_empty_contributors_uses_generic_message() {
        let contributing: SmallVec<[RuleId; 4]> = SmallVec::new();
        let failure_span = Span::new(0, 0);
        let diag = super::build_r002_diagnostic(contributing, failure_span);

        assert_eq!(diag.rule, super::R002_RULE_ID);
        assert!(diag.fix.is_none());
        assert!(diag.text_correction.is_none());
        // Empty-contributors branch uses the short generic message.
        let msg = diag.message.as_ref();
        assert!(msg.contains("post-pass-1 buffer failed to re-parse"));
        assert!(!msg.contains("from"));
    }

    // -------------------------------------------------------------------
    // PR 7b round-1 Copilot fixes — partition + re-lint data-flow locks
    // -------------------------------------------------------------------
    //
    // Copilot round-1 finding #2: the re-parse arm of `TwoPassFixer::run`
    // discarded the post-pass-1 re-lint's diagnostic stream and dispatched
    // pass-2 against the pre-pass-1 partition. The fix re-partitions
    // `relint.diagnostics` and feeds pass-2 the fresh post-pass-1
    // WholeMarking slice. Tests below pin the partition logic in
    // isolation and lock the data-flow contract via a stub Phase::Localized
    // FixIntent rule that mutates the buffer.

    #[test]
    fn partition_diags_by_phase_routes_by_localized_id_set() {
        // The partition predicate: rule IDs in `localized_ids` go to
        // pass-1; everything else goes to pass-2. text_correction
        // diagnostics with no `fix` are excluded from BOTH partitions.
        let localized: HashSet<&'static str> = ["E006", "E007", "C001"].into_iter().collect();

        let pass1_id = Diagnostic::<CapcoScheme>::new(
            RuleId::new("E006"),
            Severity::Error,
            Span::new(0, 4),
            "pass-1 candidate",
            "TEST",
            None,
        );
        let pass2_id = Diagnostic::<CapcoScheme>::new(
            RuleId::new("E022"),
            Severity::Error,
            Span::new(4, 8),
            "pass-2 candidate",
            "TEST",
            None,
        );
        let unknown_id = Diagnostic::<CapcoScheme>::new(
            RuleId::new("E999"),
            Severity::Error,
            Span::new(8, 12),
            "unknown id falls to pass-2 by default",
            "TEST",
            None,
        );
        let text_corr_no_fix = Diagnostic::text_correction(
            RuleId::new("C001"),
            Severity::Fix,
            Span::new(12, 16),
            "sub-threshold text correction",
            "TEST",
            "REPL",
            FixSource::CorrectionsMap,
            marque_rules::Confidence::strict(0.4),
            None,
        );

        let diags = vec![
            pass1_id.clone(),
            pass2_id.clone(),
            unknown_id.clone(),
            text_corr_no_fix.clone(),
        ];
        let (p1, p2) = super::partition_diags_by_phase(&diags, &localized);

        // Pass-1: E006 only (C001's text-correction with no fix is
        // excluded; pass-0 already promoted it or marked it as a sub-
        // threshold suggestion the remaining-diagnostics filter handles).
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].rule.as_str(), "E006");

        // Pass-2: E022 (declared) + E999 (unknown id ⇒ default to pass-2).
        assert_eq!(p2.len(), 2);
        let p2_ids: Vec<&str> = p2.iter().map(|d| d.rule.as_str()).collect();
        assert!(p2_ids.contains(&"E022"));
        assert!(p2_ids.contains(&"E999"));
        // text_correction-no-fix excluded from both:
        for d in p1.iter().chain(p2.iter()) {
            assert_ne!(
                d.rule.as_str(),
                "C001",
                "text_correction-no-fix must be excluded from both partitions"
            );
        }
    }

    #[test]
    fn partition_diags_by_phase_includes_text_correction_with_fix_in_partition() {
        // A text_correction diagnostic that ALSO carries a `fix` is a
        // sub-threshold suggestion — it stays in the partition routed by
        // its rule id's phase (so the engine's remaining-diagnostics
        // filter can re-surface it as a Suggest).
        let localized: HashSet<&'static str> = ["C001"].into_iter().collect();

        let mut tc = Diagnostic::text_correction(
            RuleId::new("C001"),
            Severity::Fix,
            Span::new(0, 6),
            "sub-threshold with structural fix",
            "TEST",
            "SECRET",
            FixSource::CorrectionsMap,
            marque_rules::Confidence::strict(0.4),
            None,
        );
        tc.fix = Some(FixIntent::<CapcoScheme> {
            replacement: ReplacementIntent::Recanonicalize {
                scope: RecanonScope::Portion,
            },
            confidence: marque_rules::Confidence::strict(0.4),
            feature_ids: SmallVec::new(),
            message: Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs::default(),
            ),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        });

        let (p1, p2) = super::partition_diags_by_phase(&[tc], &localized);
        assert_eq!(p1.len(), 1, "text_correction WITH fix → pass-1 (C001)");
        assert_eq!(p2.len(), 0);
    }

    #[test]
    fn pass1_localized_fixintent_run_dispatches_pass2_with_fresh_relint() {
        // FR-023 partial — Copilot round-1 finding #2 fix.
        //
        // Scenario: a stub `Phase::Localized` rule that emits a
        // `FixIntent` whose application changes the buffer in a way the
        // pre-pass-1 lint could not have seen. After pass-1, the engine
        // MUST re-lint the post-pass-1 buffer and partition the FRESH
        // diagnostics for pass-2 — NOT reuse the stale pre-pass-1
        // partition.
        //
        // The behavioral lock: after the fix runs, `FixResult.applied`
        // contains the stub rule's fix at the post-pass-0 span, and
        // pass-2's diagnostic dispatch operates against the post-pass-1
        // buffer's marking shape (verifiable through engine-level
        // outputs — `result.source` after Apply, `remaining_diagnostics`
        // reflecting the post-pass-1 state).
        //
        // Today no production `Phase::Localized` rule emits a FixIntent,
        // so this stub-based test is the load-bearing pin for the
        // re-partition data flow. When a future Localized FixIntent rule
        // lands, integration-level fixtures cover the same path.

        struct LocalizedFixIntentStub;
        impl Rule<CapcoScheme> for LocalizedFixIntentStub {
            fn id(&self) -> RuleId {
                RuleId::new("E899")
            }
            fn name(&self) -> &'static str {
                "stub-localized-fixintent"
            }
            fn default_severity(&self) -> Severity {
                Severity::Fix
            }
            fn phase(&self) -> marque_rules::Phase {
                marque_rules::Phase::Localized
            }
            fn check(
                &self,
                _attrs: &CanonicalAttrs,
                ctx: &RuleContext,
            ) -> Vec<Diagnostic<CapcoScheme>> {
                // Emit a Recanonicalize FixIntent at the marking's
                // portion span. CapcoScheme will recanonicalize the
                // portion in `apply_intent`, which produces a real
                // byte-level rewrite. The diagnostic's `span` is a
                // sub-region (the NOFORN token within the marking)
                // so the Localized span-shape filter accepts it;
                // `candidate_span` is the full marking span so the
                // synthesize step can look up the parsed marking.
                let intent = FixIntent::<CapcoScheme> {
                    replacement: ReplacementIntent::Recanonicalize {
                        scope: RecanonScope::Portion,
                    },
                    confidence: marque_rules::Confidence::strict(1.0),
                    feature_ids: SmallVec::new(),
                    message: Message::new(
                        MessageTemplate::BannerRollupMismatch,
                        MessageArgs::default(),
                    ),
                    source: FixSource::BuiltinRule,
                    migration_ref: None,
                };
                vec![Diagnostic::with_fix_at_span(
                    RuleId::new("E899"),
                    Severity::Fix,
                    Span::new(8, 14),
                    ctx.candidate_span,
                    "stub localized fix",
                    "TEST",
                    intent,
                )]
            }
        }

        let set: Box<dyn RuleSet<CapcoScheme>> =
            Box::new(StubSet(vec![Box::new(LocalizedFixIntentStub)]));
        let engine = Engine::with_clock(
            Config::default(),
            vec![set],
            marque_capco::scheme::CapcoScheme::new(),
            Box::new(FixedClock::new(
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )),
        )
        .expect("engine constructs cleanly");

        let result = engine.fix(TEST_SRC, FixMode::Apply);

        // The engine ran without panicking, returned a coherent
        // FixResult, and did NOT trigger R002 (the post-pass-1
        // buffer still parses). This is the data-flow integration
        // sanity for the re-partition arm — if pass-2 had been fed
        // stale pre-pass-1 diagnostics whose spans no longer matched
        // the post-pass-1 buffer, the engine would panic on the
        // splice slice (or assert in debug). Reaching here cleanly
        // is the lock.
        assert!(
            !result.r002_fired,
            "stub rule's fix does not collapse the marking, so R002 must not fire"
        );
        // Pass-1's fix was applied — at least one `AppliedFix`
        // carries the stub rule's ID.
        let saw_stub_fix = result.applied.iter().any(|f| f.rule.as_str() == "E899");
        assert!(
            saw_stub_fix,
            "stub localized FixIntent rule's fix must be promoted into AppliedFix; \
             applied: {:?}",
            result
                .applied
                .iter()
                .map(|f| f.rule.as_str())
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn pass1_localized_fixintent_dryrun_records_applied_without_mutating_source() {
        // Companion to the Apply test above: in DryRun mode the
        // engine MUST return the original source unmodified AND
        // still surface the stub Localized fix as an applied record
        // with `dry_run = true`. Locks the DryRun branch of
        // `apply_kept_fixes` (the second arm of the inner
        // `match self.mode`) which the Apply test does not reach.

        struct LocalizedFixIntentStub;
        impl Rule<CapcoScheme> for LocalizedFixIntentStub {
            fn id(&self) -> RuleId {
                RuleId::new("E898")
            }
            fn name(&self) -> &'static str {
                "stub-localized-fixintent-dryrun"
            }
            fn default_severity(&self) -> Severity {
                Severity::Fix
            }
            fn phase(&self) -> marque_rules::Phase {
                marque_rules::Phase::Localized
            }
            fn check(
                &self,
                _attrs: &CanonicalAttrs,
                ctx: &RuleContext,
            ) -> Vec<Diagnostic<CapcoScheme>> {
                let intent = FixIntent::<CapcoScheme> {
                    replacement: ReplacementIntent::Recanonicalize {
                        scope: RecanonScope::Portion,
                    },
                    confidence: marque_rules::Confidence::strict(1.0),
                    feature_ids: SmallVec::new(),
                    message: Message::new(
                        MessageTemplate::BannerRollupMismatch,
                        MessageArgs::default(),
                    ),
                    source: FixSource::BuiltinRule,
                    migration_ref: None,
                };
                vec![Diagnostic::with_fix_at_span(
                    RuleId::new("E898"),
                    Severity::Fix,
                    Span::new(8, 14),
                    ctx.candidate_span,
                    "stub localized fix (dry-run)",
                    "TEST",
                    intent,
                )]
            }
        }

        let set: Box<dyn RuleSet<CapcoScheme>> =
            Box::new(StubSet(vec![Box::new(LocalizedFixIntentStub)]));
        let engine = Engine::with_clock(
            Config::default(),
            vec![set],
            marque_capco::scheme::CapcoScheme::new(),
            Box::new(FixedClock::new(
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )),
        )
        .expect("engine constructs cleanly");

        let result = engine.fix(TEST_SRC, FixMode::DryRun);
        assert_eq!(result.source, TEST_SRC, "DryRun must not mutate source");
        assert!(!result.r002_fired);
        let stub_fix = result
            .applied
            .iter()
            .find(|f| f.rule.as_str() == "E898")
            .expect("stub fix should appear in applied list");
        assert!(
            stub_fix.dry_run,
            "DryRun applied fix must have dry_run=true"
        );
    }

    // -------------------------------------------------------------------
    // Pure-helper unit tests — sort_and_c1_dedup / apply_pass1_fixes /
    // span_is_within_marking / find_containing_marking
    // -------------------------------------------------------------------
    //
    // The TwoPassFixer methods invoke these via the engine end-to-end
    // path. Direct unit tests pin the algebraic contract of each helper
    // independently of the dispatcher, so a future change to the
    // dispatcher cannot silently break an invariant of the helper.

    /// Build a `SynthesizedFix` for unit tests of the splice / sort
    /// helpers. `intent` is filled with a no-op Recanonicalize because
    /// the helpers only read `rule`/`span`/`replacement`.
    fn synth_fix(
        rule: &'static str,
        start: usize,
        end: usize,
        replacement: &str,
    ) -> SynthesizedFix {
        SynthesizedFix {
            rule: RuleId::new(rule),
            span: Span::new(start, end),
            replacement: replacement.into(),
            intent: FixIntent::<CapcoScheme> {
                replacement: ReplacementIntent::Recanonicalize {
                    scope: RecanonScope::Portion,
                },
                confidence: marque_rules::Confidence::strict(1.0),
                feature_ids: SmallVec::new(),
                message: Message::new(
                    MessageTemplate::BannerRollupMismatch,
                    MessageArgs::default(),
                ),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            },
        }
    }

    #[test]
    fn sort_and_c1_dedup_orders_descending_by_span_end() {
        // FR-016 sort key: span.end DESC, then span.start DESC, then
        // rule ASC, then replacement ASC. Use truly disjoint spans
        // so the C-1 dedup walk keeps all of them.
        let synthesized = vec![
            synth_fix("E001", 0, 2, "AA"),   // span 0..2
            synth_fix("E002", 10, 14, "BB"), // span 10..14
            synth_fix("E003", 4, 8, "CC"),   // span 4..8
        ];
        let sorted = super::sort_and_c1_dedup(synthesized);
        // Disjoint spans, so all three survive. FR-016 sort →
        // 10..14, 4..8, 0..2.
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].span.end, 14);
        assert_eq!(sorted[1].span.end, 8);
        assert_eq!(sorted[2].span.end, 2);
    }

    #[test]
    fn sort_and_c1_dedup_drops_overlapping_fixes() {
        // Two overlapping fixes: keep the lex-min winner per C-1.
        // After FR-016 sort, span 4..10 comes first (later end),
        // then 0..8 (earlier end) — but 0..8 overlaps with 4..10,
        // so it is dropped.
        let synthesized = vec![
            synth_fix("E001", 0, 8, "AA"), // overlaps 4..10
            synth_fix("E002", 4, 10, "BB"),
        ];
        let kept = super::sort_and_c1_dedup(synthesized);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].span, Span::new(4, 10));
    }

    #[test]
    fn sort_and_c1_dedup_tiebreaks_lex_min_rule_then_replacement() {
        // Same span (1..5): tie-break by rule ASC, then replacement.
        let synthesized = vec![
            synth_fix("E003", 1, 5, "ZZ"),
            synth_fix("E001", 1, 5, "AA"),
            synth_fix("E002", 1, 5, "BB"),
        ];
        let kept = super::sort_and_c1_dedup(synthesized);
        // C-1 dedup: only one fix survives the overlap walk
        // (lex-min winner). With same span across all three, the
        // first to enter the kept set is the FR-016 sort head.
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].rule.as_str(), "E001");
    }

    #[test]
    fn sort_and_c1_dedup_empty_input_returns_empty() {
        let kept = super::sort_and_c1_dedup(Vec::new());
        assert!(kept.is_empty());
    }

    #[test]
    fn apply_pass1_fixes_splices_in_reverse_order() {
        // Source: "SECRET//NOFORN" (14 bytes).
        // Two fixes: 0..6 → "AA", 8..14 → "BB".
        // FR-016 sort (span.end DESC) → 8..14 first, then 0..6.
        // forward walk via `iter().rev()` yields 0..6 then 8..14.
        let source = b"SECRET//NOFORN";
        let fixes = super::sort_and_c1_dedup(vec![
            synth_fix("E001", 0, 6, "AA"),
            synth_fix("E002", 8, 14, "BB"),
        ]);
        let out = super::apply_pass1_fixes(source, &fixes);
        assert_eq!(out, b"AA//BB");
    }

    #[test]
    fn apply_pass1_fixes_with_empty_fixes_returns_source_clone() {
        let source = b"SECRET//NOFORN";
        let out = super::apply_pass1_fixes(source, &[]);
        assert_eq!(out, source);
    }

    #[test]
    fn apply_pass1_fixes_handles_replacement_growth_and_shrink() {
        // 0..6 → "TOP SECRET" (grow), 8..14 → "X" (shrink).
        let source = b"SECRET//NOFORN";
        let fixes = super::sort_and_c1_dedup(vec![
            synth_fix("E001", 0, 6, "TOP SECRET"),
            synth_fix("E002", 8, 14, "X"),
        ]);
        let out = super::apply_pass1_fixes(source, &fixes);
        assert_eq!(out, b"TOP SECRET//X");
    }

    #[test]
    fn span_is_within_marking_inclusive_on_both_endpoints() {
        let marking = Span::new(0, 14);
        // Exact match
        assert!(super::span_is_within_marking(Span::new(0, 14), marking));
        // Sub-span
        assert!(super::span_is_within_marking(Span::new(2, 8), marking));
        // Touching start
        assert!(super::span_is_within_marking(Span::new(0, 5), marking));
        // Touching end
        assert!(super::span_is_within_marking(Span::new(9, 14), marking));
        // Out of bounds on either side
        assert!(!super::span_is_within_marking(Span::new(0, 15), marking));
        assert!(!super::span_is_within_marking(Span::new(15, 20), marking));
    }

    #[test]
    fn find_containing_marking_returns_some_when_span_inside() {
        // Build a parsed_markings map with one entry and look up a
        // sub-span. We use `lint_with_options_internal` indirectly via
        // a real engine to get a real `CapcoMarking` value — the
        // function under test just keys on `Span`.
        let engine = engine_with(vec![]);
        let (_lint, markings) =
            engine.lint_with_options_internal(TEST_SRC, &LintOptions::default());
        assert!(!markings.is_empty(), "test source should parse");
        let any_span = *markings.keys().next().unwrap();
        // A sub-span inside any_span resolves to any_span.
        let sub = Span::new(any_span.start, any_span.start + 1);
        let found = super::find_containing_marking(&markings, sub);
        assert_eq!(found, Some(any_span));
    }

    #[test]
    fn find_containing_marking_returns_none_when_no_marking_contains() {
        let engine = engine_with(vec![]);
        let (_lint, markings) =
            engine.lint_with_options_internal(TEST_SRC, &LintOptions::default());
        // Way past the end of the source — no marking spans this far.
        let far = Span::new(10_000, 10_001);
        let found = super::find_containing_marking(&markings, far);
        assert!(found.is_none());
    }

    // -------------------------------------------------------------------
    // TwoPassFixer method-level tests — contributing_pass1_rule_ids /
    // assemble_r002_result
    // -------------------------------------------------------------------
    //
    // R002 is unreachable from production CAPCO rules today (no Localized
    // rule emits a FixIntent that collapses marking shape), so the
    // assemble_r002_result + contributing_pass1_rule_ids paths cannot be
    // exercised end-to-end through the public `Engine::fix`. The unit
    // tests below construct a `TwoPassFixer` directly and invoke the two
    // methods with synthetic inputs to pin the audit-stream invariant
    // (R002 result carries pass-0 + pass-1 fixes in order, R002
    // diagnostic appended last).
    //
    // Synthetic `AppliedFix` records here are constructed via
    // `__engine_promote` under the Constitution V Principle V
    // test-fixture carve-out — the fabricated fixes never flow into a
    // real audit stream; they exist to feed the assembler under test.

    fn synth_applied_fix(rule: &'static str, start: usize, end: usize) -> AppliedFix<CapcoScheme> {
        let intent = FixIntent::<CapcoScheme> {
            replacement: ReplacementIntent::Recanonicalize {
                scope: RecanonScope::Portion,
            },
            confidence: marque_rules::Confidence::strict(1.0),
            feature_ids: SmallVec::new(),
            message: Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs::default(),
            ),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };
        // Test-fixture carve-out per Constitution V Principle V — this
        // call sits inside #[cfg(test)] and feeds the
        // `assemble_r002_result` / `contributing_pass1_rule_ids` unit
        // tests; the fabricated record is never commingled with engine
        // output.
        AppliedFix::__engine_promote(
            RuleId::new(rule),
            Span::new(start, end),
            intent,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            None,
            false,
            None,
            engine_promotion_token(),
        )
    }

    #[test]
    fn contributing_pass1_rule_ids_dedupes_and_sorts() {
        let engine = engine_with(vec![]);
        let fixer = super::TwoPassFixer {
            engine: &engine,
            source: TEST_SRC,
            mode: FixMode::Apply,
            threshold: 0.95,
            deadline: None,
        };
        // Three fixes: two duplicates of E006, one of C001. The helper
        // dedupes and sorts ASC. Result: [C001, E006].
        let applied = vec![
            synth_applied_fix("E006", 0, 4),
            synth_applied_fix("C001", 4, 8),
            synth_applied_fix("E006", 8, 12),
        ];
        let out = fixer.contributing_pass1_rule_ids(&applied);
        let ids: Vec<&str> = out.iter().map(|id| id.as_str()).collect();
        assert_eq!(ids, vec!["C001", "E006"]);
    }

    #[test]
    fn contributing_pass1_rule_ids_caps_at_inline_capacity_4() {
        let engine = engine_with(vec![]);
        let fixer = super::TwoPassFixer {
            engine: &engine,
            source: TEST_SRC,
            mode: FixMode::Apply,
            threshold: 0.95,
            deadline: None,
        };
        // Five distinct IDs — only the first 4 (after sort) survive
        // the SmallVec inline cap.
        let applied = vec![
            synth_applied_fix("E009", 0, 4),
            synth_applied_fix("E008", 4, 8),
            synth_applied_fix("E007", 8, 12),
            synth_applied_fix("E006", 12, 16),
            synth_applied_fix("C001", 16, 20),
        ];
        let out = fixer.contributing_pass1_rule_ids(&applied);
        let ids: Vec<&str> = out.iter().map(|id| id.as_str()).collect();
        // ASC-sorted, then take(4) → C001, E006, E007, E008.
        assert_eq!(ids, vec!["C001", "E006", "E007", "E008"]);
    }

    #[test]
    fn contributing_pass1_rule_ids_empty_input_returns_empty() {
        let engine = engine_with(vec![]);
        let fixer = super::TwoPassFixer {
            engine: &engine,
            source: TEST_SRC,
            mode: FixMode::Apply,
            threshold: 0.95,
            deadline: None,
        };
        let out = fixer.contributing_pass1_rule_ids(&[]);
        assert!(out.is_empty());
    }

    #[test]
    fn assemble_r002_result_carries_pass0_then_pass1_applied_plus_r002_diag() {
        // The assembler concatenates pass-0 then pass-1 applied (audit
        // stream order D-7.6), filters remaining diagnostics by
        // applied_keys, then appends the R002 diagnostic LAST.
        let engine = engine_with(vec![]);
        let fixer = super::TwoPassFixer {
            engine: &engine,
            source: TEST_SRC,
            mode: FixMode::Apply,
            threshold: 0.95,
            deadline: None,
        };

        let pass0_applied = vec![synth_applied_fix("C001", 0, 6)];
        let pass1_applied = vec![synth_applied_fix("E006", 8, 12)];
        let pass1 = Pass1Result {
            post_buffer: b"POST-PASS-1-BUFFER".to_vec(),
            applied: pass1_applied,
            applied_keys: HashSet::new(),
        };
        let lint = LintResult {
            diagnostics: Vec::new(),
            truncated: false,
            candidates_processed: 0,
            candidates_total: 0,
        };
        let r002 = super::build_r002_diagnostic(
            smallvec::smallvec![RuleId::new("E006")],
            Span::new(0, 18),
        );
        let result =
            fixer.assemble_r002_result(pass0_applied, Vec::new(), pass1, lint, r002.clone());

        // Order: pass0 (C001) then pass1 (E006).
        assert_eq!(result.applied.len(), 2);
        assert_eq!(result.applied[0].rule.as_str(), "C001");
        assert_eq!(result.applied[1].rule.as_str(), "E006");
        // R002 fired flag set.
        assert!(result.r002_fired);
        // R002 diagnostic is the last entry in remaining_diagnostics.
        assert!(!result.remaining_diagnostics.is_empty());
        let last = result.remaining_diagnostics.last().unwrap();
        assert_eq!(last.rule, super::R002_RULE_ID);
        // Apply mode returns the pass-1 buffer.
        assert_eq!(result.source, b"POST-PASS-1-BUFFER");
    }

    #[test]
    fn assemble_r002_result_dryrun_returns_original_source() {
        // DryRun mode returns the original `self.source`, NOT the
        // pass-1 buffer — even though pass-1's audit records are
        // preserved (D-7.6: "the fixes happened; the audit log is
        // honest about it" doesn't mean the buffer mutates in dry-run).
        let engine = engine_with(vec![]);
        let fixer = super::TwoPassFixer {
            engine: &engine,
            source: TEST_SRC,
            mode: FixMode::DryRun,
            threshold: 0.95,
            deadline: None,
        };

        let pass1 = Pass1Result {
            post_buffer: b"POST-PASS-1-BUFFER".to_vec(),
            applied: vec![synth_applied_fix("E006", 8, 12)],
            applied_keys: HashSet::new(),
        };
        let lint = LintResult {
            diagnostics: Vec::new(),
            truncated: false,
            candidates_processed: 0,
            candidates_total: 0,
        };
        let r002 = super::build_r002_diagnostic(SmallVec::new(), Span::new(0, 0));
        let result = fixer.assemble_r002_result(Vec::new(), Vec::new(), pass1, lint, r002);
        assert_eq!(result.source, TEST_SRC);
        assert!(result.r002_fired);
    }

    #[test]
    fn assemble_r002_result_carries_through_pass0_dropped_diagnostics() {
        // Pass-0 dropped diagnostics (C-1 overlap-loss in the text-
        // correction layer) MUST surface via remaining_diagnostics
        // even on the R002 path; the pass-2 lint never runs to re-emit
        // them.
        let engine = engine_with(vec![]);
        let fixer = super::TwoPassFixer {
            engine: &engine,
            source: TEST_SRC,
            mode: FixMode::Apply,
            threshold: 0.95,
            deadline: None,
        };

        let dropped = vec![Diagnostic::<CapcoScheme>::new(
            RuleId::new("C001"),
            Severity::Fix,
            Span::new(20, 24),
            "dropped pass-0 text correction",
            "TEST",
            None,
        )];
        let pass1 = Pass1Result {
            post_buffer: Vec::new(),
            applied: Vec::new(),
            applied_keys: HashSet::new(),
        };
        let lint = LintResult {
            diagnostics: Vec::new(),
            truncated: false,
            candidates_processed: 0,
            candidates_total: 0,
        };
        let r002 = super::build_r002_diagnostic(SmallVec::new(), Span::new(0, 0));
        let result = fixer.assemble_r002_result(Vec::new(), dropped, pass1, lint, r002);
        // Dropped diagnostic + R002 = 2 entries; the dropped one
        // appears before the R002 entry (R002 pushed last).
        assert_eq!(result.remaining_diagnostics.len(), 2);
        assert_eq!(result.remaining_diagnostics[0].rule.as_str(), "C001");
        assert_eq!(result.remaining_diagnostics[1].rule, super::R002_RULE_ID);
    }

    #[test]
    fn assemble_r002_result_filters_fixed_diagnostics_from_remaining() {
        // A diagnostic whose fix landed (applied key matches) is
        // filtered out of `remaining_diagnostics`. Locks the
        // applied_keys filter on the R002 path — without this the
        // same diagnostic would surface in both applied and remaining.
        let engine = engine_with(vec![]);
        let fixer = super::TwoPassFixer {
            engine: &engine,
            source: TEST_SRC,
            mode: FixMode::Apply,
            threshold: 0.95,
            deadline: None,
        };

        let intent = FixIntent::<CapcoScheme> {
            replacement: ReplacementIntent::Recanonicalize {
                scope: RecanonScope::Portion,
            },
            confidence: marque_rules::Confidence::strict(1.0),
            feature_ids: SmallVec::new(),
            message: Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs::default(),
            ),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };
        let diag_with_fix = Diagnostic::with_fix(
            RuleId::new("E006"),
            Severity::Error,
            Span::new(8, 14),
            "had a fix",
            "TEST",
            Some(intent),
        );
        let pass1_applied = vec![synth_applied_fix("E006", 8, 14)];
        let pass1 = Pass1Result {
            post_buffer: Vec::new(),
            applied: pass1_applied,
            applied_keys: HashSet::new(),
        };
        let lint = LintResult {
            diagnostics: vec![diag_with_fix],
            truncated: false,
            candidates_processed: 0,
            candidates_total: 0,
        };
        let r002 = super::build_r002_diagnostic(SmallVec::new(), Span::new(0, 0));
        let result = fixer.assemble_r002_result(Vec::new(), Vec::new(), pass1, lint, r002);
        // Pre-r002 entries are 0 (the E006 diag was filtered),
        // then R002 is pushed last.
        assert_eq!(result.remaining_diagnostics.len(), 1);
        assert_eq!(result.remaining_diagnostics[0].rule, super::R002_RULE_ID);
    }
}
