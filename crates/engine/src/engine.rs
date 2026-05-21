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
use crate::scheduler::{schedule_rewrites, validate_intent_rewrites};
use crate::text_correction::{SynthesizedFix, TextCorrectionProposal};
use aho_corasick::AhoCorasick;
use marque_capco::CapcoScheme;
use marque_capco::provenance::DecoderProvenance;
use marque_config::Config;
use marque_rules::audit::{AppliedTextCorrection, AuditLine};
use marque_rules::{
    CORRECTIONS_MAP_CITATION, Confidence, Diagnostic, EnginePromotionToken, FixIntent, FixSource,
    Phase, RuleId, RuleSet, Severity, SmallVec,
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
///
/// PR 3c.2.C C5 migrated this from `&'static str` →
/// [`marque_scheme::Citation`] atomically with the `Diagnostic.citation`
/// field-type flip. (PR 10.A.1 Commit 4 retargeted the import path from
/// `marque_rules::Citation` to the canonical `marque_scheme::Citation`
/// when the back-compat re-export was deleted.)
const DECODER_CITATION_TYPED: marque_scheme::Citation =
    marque_scheme::capco(marque_scheme::SectionLetter::A, 6, 15);

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

/// Typed [`Citation`](marque_scheme::Citation) attached to `R002`
/// diagnostics — the synthetic re-parse-failure sentinel has no CAPCO
/// §-citation by construction (Constitution VIII requires a real
/// passage; R002 is engine-internal guidance, not a CAPCO rule). Uses
/// [`marque_scheme::AuthoritativeSource::EngineInternal`]. Display
/// renders as `[engine-internal]`.
///
/// PR 3c.2.C C5 migrated this from `&'static str` → typed `Citation`
/// atomically with the `Diagnostic.citation` field-type flip. PR 10.A.1
/// Commit 4 retargeted the import path from `marque_rules::Citation`
/// to the canonical `marque_scheme::Citation` when the back-compat
/// re-export was deleted.
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
/// PR 6c (T069) moved this const from the retired
/// `marque_ism::PageContext` to its single owner site at the engine
/// accumulator. Issue #430.
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
pub(crate) fn fresh_page_portions_accumulator() -> Vec<marque_ism::CanonicalAttrs> {
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
    ///
    /// # Bridge diagnostic population
    ///
    /// The engine bridge (Commit 7.3+) uses row names from the
    /// `Constraint` catalog to populate `Diagnostic.rule`. For CAPCO,
    /// the bridge applies the following mappings to ensure audit-stream
    /// continuity with retired hand-written rules:
    ///
    /// - `class-floor/<marking>` and `E058/<purpose>` → `RuleId("E058")`
    /// - `sci-per-system/<row>` → `RuleId("E059")`
    /// - `E010/HCS-system-constraints` → `RuleId("E010")`
    /// - `E012/dual-classification` → `RuleId("E012")`
    /// - `E014/joint-requires-rel-to-coverage` → `RuleId("E014")`
    /// - `E015/non-us-requires-dissem` → `RuleId("E015")`
    /// - `E016/joint-conflicts-restricted` → `RuleId("E016")`
    /// - `E036/joint-conflicts-hcs` → `RuleId("E036")`
    /// - `E021/rd-frd-requires-noforn` → `RuleId("E021")`
    /// - `E024/rd-precedence` → `RuleId("E024")`
    /// - `capco/noforn-conflicts-rel-to` → `RuleId("E053")`
    /// - `E037/nodis-conflicts-exdis` → `RuleId("E037")`
    /// - `E038/nodis-or-exdis-requires-noforn` → `RuleId("E038")`
    /// - `E054/relido-conflicts-noforn` → `RuleId("E054")`
    /// - `E055/relido-conflicts-display-only` → `RuleId("E055")`
    /// - `E056/orcon-conflicts-relido` → `RuleId("E056")`
    /// - `E057/orcon-usgov-conflicts-relido` → `RuleId("E057")`
    ///
    /// The bridge logic for these mappings lives in `crates/engine/src/engine.rs`.
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
    /// a `CanonicalAttrs`. Stored as an enum with concrete variants for
    /// the in-tree recognizers (`Strict`, `StrictOrDecoder`)
    /// so default dispatch stays monomorphized; a `Dyn` escape hatch
    /// preserves the existing `with_recognizer(Arc<dyn Recognizer<_>>)`
    /// customization surface for downstream callers.
    ///
    /// Default: [`StrictOrDecoderRecognizer`] — strict-first dispatch
    /// with a decoder fallback on strict-parse zero-candidate. The
    /// decoder recovers mangled markings that are edit-distance-1/2,
    /// token-reordered, superseded, or case-mangled from a real
    /// CAPCO-2016 marking. Live-typing surfaces concerned with
    /// per-keystroke latency are expected to debounce their calls into
    /// the engine; surfaces that need to pin strict-only behavior (the
    /// SC-001 interactive-latency benchmark, tests asserting strict
    /// dispatch) should call [`Engine::with_strict_recognizer`].
    recognizer: EngineRecognizer,

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
    /// **Post-PR-7c behavior.** Stored but not yet read at dispatch
    /// time. Pass-2 in `TwoPassFixer::run` routes diagnostics as the
    /// **complement** of the pass-1 (Localized) set via
    /// `partition_diags_by_phase` — sufficient for today's rule
    /// shape because every diagnostic emitted by `lint()` comes
    /// from a registered rule, so the complement equals the
    /// WholeMarking partition. PR 7c retained this dispatch shape
    /// (implementer Decision #4) rather than flipping to a positive
    /// whitelist; the field stays available for a deferred future
    /// PR that wants the symmetry with pass-1 and the "unregistered
    /// emitted ID falls into neither pass" property. No schedule for
    /// that work is set. See [`Engine::pass1_rule_indices`] for the
    /// shape rationale.
    #[allow(dead_code)]
    pass2_rule_indices: Pass2Indices,
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
    pass_finalization_rule_indices: PassFinalizationIndices,

    /// Pre-resolved severity for every registered rule's *registered* ID,
    /// indexed outer-by-rule-set, inner-by-rule-index-within-set. Built
    /// once at construction time by [`build_severity_tables`] and read
    /// from the lint hot loop (Site A — fast-path Off-skip) instead of
    /// the per-candidate `config.rules.overrides` HashMap probe + per-
    /// candidate `Severity::parse_config` parse.
    ///
    /// **Population.** For each registered rule, the entry is
    /// `overrides.get(rule.id().as_str()).and_then(parse_config)
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
    fast_path_severities: FastPathSeverities,

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
    /// **Hot-loop consumers.** Read by Sites B (per-diagnostic
    /// `retain_mut` rewrite), C (bridge `ConstraintViolation`
    /// envelope), and D (C001 corrections-map post-pass). This field
    /// handles the construction-time part of the optimization by
    /// precomputing emitted-ID override severities once, so hot paths
    /// avoid repeated parse/canonicalization work. The pre-`lint()`
    /// `e059_override` hoist still exists intentionally: each `lint()`
    /// call does `self.emitted_id_overrides.get("E059").copied()`
    /// once and passes that cached value through to
    /// `bridge_sci_per_system_diagnostics`, avoiding per-candidate
    /// `HashMap` probes.
    emitted_id_overrides: EmittedIdOverrides,
}

#[derive(Clone)]
enum EngineRecognizer {
    /// Fully monomorphized strict-only recognizer path.
    Strict(StrictRecognizer),
    /// Fully monomorphized strict-first/decoder-fallback recognizer path
    /// used by `Engine::new`.
    StrictOrDecoder(StrictOrDecoderRecognizer),
    /// Trait-object escape hatch for caller-supplied recognizers that are
    /// not one of the in-tree concrete variants above.
    Dyn(Arc<dyn Recognizer<CapcoScheme>>),
}

impl Default for EngineRecognizer {
    fn default() -> Self {
        Self::StrictOrDecoder(StrictOrDecoderRecognizer::new())
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
        match self {
            Self::Strict(r) => r.recognize(bytes, offset, scheme, cx),
            Self::StrictOrDecoder(r) => r.recognize(bytes, offset, scheme, cx),
            Self::Dyn(r) => r.recognize(bytes, offset, scheme, cx),
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
        // `MARQUE_LOG=marque=debug` (off by default in
        // production).
        tracing::debug!(
            target: "marque_engine::scheme_discard",
            "user-supplied scheme dropped; constraint-catalog bridge uses default \
             CapcoScheme::new() (a future Engine<S> generic-cleanup PR closes this)"
        );
        drop(scheme);
        Self::with_clock_prepared(config, rule_sets, clock, bridge_scheme, scheduled_rewrites)
    }

    /// Non-generic tail of [`Engine::with_clock`].
    ///
    /// Keeping the heavy construction path behind a concrete signature
    /// avoids monomorphizing the full constructor body for every `S`
    /// used at call sites; only the rewrite-validation/scheduling front
    /// edge remains generic.
    fn with_clock_prepared(
        mut config: Config,
        rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>,
        clock: Box<dyn Clock>,
        bridge_scheme: CapcoScheme,
        scheduled_rewrites: Box<[RewriteId]>,
    ) -> Result<Self, EngineConstructionError> {
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

        let scheme = bridge_scheme;

        // PR 7a phase-partition walk (FR-021). Read every registered
        // rule's declared `Phase` and partition the rule set into a
        // pass-1 (Localized) list, a pass-2 (WholeMarking) list, and
        // (issue #461) a pass-finalization (PageFinalization) list,
        // each indexed by `(rule_set_index, rule_index_within_set)`.
        // The walk runs once at construction time; per-document
        // dispatch reads the cached partition. Phase partition stored
        // but unused in 7a; 7b restructures `fix_inner` to dispatch on
        // it; the issue #461 third bucket is read by
        // `dispatch_page_finalization` at every PageBreak boundary
        // and at EOD.
        let (pass1_rule_indices, pass2_rule_indices, pass_finalization_rule_indices) =
            partition_rules_by_phase(&rule_sets);

        // Pre-resolve all rule severity overrides into indexed lookup
        // tables consumed by the lint hot loop. Drops the per-candidate
        // `config.rules.overrides` HashMap probes + per-candidate
        // `Severity::parse_config` parses from Sites A/B/C/D in
        // `lint_inner` (a perf-only refactor — semantics preserved
        // byte-for-byte at the audit boundary).
        let (fast_path_severities, emitted_id_overrides) = build_severity_tables(
            &rule_sets,
            &config.rules.overrides,
            scheme.bridge_emitted_rule_ids(),
        );

        Ok(Self {
            config,
            rule_sets,
            scheme,
            clock,
            corrections_arc,
            corrections_ac,
            scheduled_rewrites,
            recognizer: EngineRecognizer::default(),
            #[cfg(feature = "corpus-override")]
            corpus_override: None,
            pass1_rule_indices,
            pass2_rule_indices,
            pass_finalization_rule_indices,
            fast_path_severities,
            emitted_id_overrides,
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
    /// decoder fallback). Callers that need to install a custom
    /// recognizer implementation can do so here. For strict-only dispatch
    /// without trait-object dispatch, prefer
    /// [`Engine::with_strict_recognizer`].
    ///
    /// Returns the engine by value so callers can chain:
    ///
    /// ```ignore
    /// let engine = Engine::new(config, rules, scheme)?
    ///     .with_recognizer(Arc::new(MyCustomRecognizer::new()));
    /// ```
    #[must_use = "with_recognizer returns a new Engine; the returned value must be bound for the override to take effect"]
    pub fn with_recognizer(mut self, recognizer: Arc<dyn Recognizer<CapcoScheme>>) -> Self {
        self.recognizer = EngineRecognizer::Dyn(recognizer);
        self
    }

    /// Override the engine recognizer with the strict parser path
    /// without introducing trait-object dispatch.
    ///
    /// Prefer this helper in latency-sensitive strict-only paths (for
    /// example SC-001 benchmark setups). Use [`Engine::with_recognizer`]
    /// when installing a custom recognizer implementation.
    ///
    /// ```ignore
    /// let engine = Engine::new(config, rules, scheme)?
    ///     .with_strict_recognizer();
    /// ```
    #[must_use = "with_strict_recognizer returns a new Engine; the returned value must be bound for the override to take effect"]
    pub fn with_strict_recognizer(mut self) -> Self {
        self.recognizer = EngineRecognizer::Strict(StrictRecognizer::new());
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

    /// Borrow the engine's active marking scheme.
    ///
    /// Used by the CLI / WASM audit-record renderers (PR 3c.2.D / D4)
    /// to project [`AuditLine<CapcoScheme>`] values through the
    /// scheme's [`Vocabulary`](marque_scheme::Vocabulary) and
    /// [`MarkingScheme::categories`](marque_scheme::MarkingScheme::categories)
    /// surfaces for the `marque-1.0` JSON shape. Off the lint/scan
    /// hot path — purely a wire-format projection helper.
    pub fn scheme(&self) -> &CapcoScheme {
        &self.scheme
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
    /// recognizer's per-token attribute spans) to the
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
    ) -> (LintResult, Vec<(Span, marque_capco::CapcoMarking)>) {
        // Public entry point — no pre-pass-1 cache is available
        // outside the two-pass fix orchestrator. Delegates to the
        // cache-aware path with `None`.
        self.lint_with_options_internal_with_cache(source, opts, None)
    }

    /// Cache-aware variant of [`Self::lint_with_options_internal`]
    /// used by [`TwoPassFixer`] for the post-pass-1 re-lint. The
    /// `pre_pass_1_cache` is a borrow into a `SmallVec` that lives
    /// on `TwoPassFixer::run`'s stack frame; each candidate's
    /// [`marque_rules::RuleContext::pre_pass_1_attrs`] field is
    /// populated by a containment scan over the cache entries.
    ///
    /// `None` (every caller outside the engine's two-pass fix path)
    /// retains the pre-7c behavior: rules see
    /// `pre_pass_1_attrs: None` because no pass-1 fix has been
    /// applied yet for this lint invocation.
    fn lint_with_options_internal_with_cache(
        &self,
        source: &[u8],
        opts: &LintOptions,
        pre_pass_1_cache: Option<&[(Span, marque_ism::CanonicalAttrs)]>,
    ) -> (LintResult, Vec<(Span, marque_capco::CapcoMarking)>) {
        use marque_core::Scanner;
        use marque_ism::MarkingType;
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
                Vec::new(),
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
        // Counts every `Parsed::Unambiguous` recognition this lint
        // pass produces (distinct from `parsed_markings.len()`, which
        // tracks only FixIntent-bearing candidates under issue
        // #433's deferred cache). Returned in `LintResult` so the
        // R002 sentinel in `TwoPassFixer` can detect "pass-1 splice
        // destroyed marking shape" against the broader "had any
        // recognized marking" signal instead of the narrower
        // "had a FixIntent-bearing marking."
        let mut recognized_marking_count: usize = 0;

        // Cache of recognized markings, keyed by the scanner
        // candidate's source-relative `Span`. Consumed by
        // `synthesize_intent_only_fixes` so the synthesis path looks
        // up the same marking the lint phase saw — avoiding the
        // `ParseContext` divergence Copilot PR #369 finding #2
        // flagged.
        //
        // Population policy (issue #433): the cache populates lazily
        // at the END of each candidate's iteration, gated on
        // `d.fix.is_some()` for any diagnostic that iteration
        // produced. Candidates that emit only `text_correction`,
        // no-fix, or no diagnostics leave the cache untouched —
        // `synthesize_intent_only_fixes` reads the cache only for
        // FixIntent-bearing diagnostics, so the gate matches the
        // consumer exactly.
        //
        // Storage shape (issue #432): a sorted `Vec<(Span, _)>` rather
        // than a `HashMap`. Cache-insertion order tracks scanner-emitted
        // candidate order, which `Scanner::scan` sorts by
        // `(span.start, kind_sort_priority)`. At the scanner boundary
        // co-located candidates exist (PageBreak gets priority 0 so it
        // sorts before a same-start content candidate, ensuring the
        // engine's PageContext reset runs first). PageBreak candidates
        // hit the engine's early-`continue` BEFORE reaching the cache
        // push site below, so the strictly-increasing-start invariant
        // holds at the cache slice (cf. the `debug_assert!` on push
        // below). Point lookups go through binary search on `Span.start`;
        // containment scans stay linear (defect path, <100 markings
        // typical per `find_containing_marking` doc). Cache-locality + no
        // SipHash + no bucket traversal — see the paired stress benches
        // `lint_parsed_markings_cache_population_stress` (lint path,
        // isolates push/drop cost) and `fix_parsed_markings_cache_stress`
        // (fix path, amortizes the cache delta into the full
        // TwoPassFixer pipeline) for the measurement decomposition.
        let mut parsed_markings: Vec<(Span, marque_capco::CapcoMarking)> = Vec::new();

        // corrections_arc was built once at Engine construction; each clone here
        // is an O(1) refcount bump.
        let corrections_arc = self.corrections_arc.clone();

        let mut diagnostics = Vec::new();
        // Build per-page state by accumulating portion markings in
        // document order. Banner and CAB rules receive this context
        // so they can validate the observed banner against the
        // expected composite. Phase 3 wires the page-break reset
        // below — the scanner emits a `MarkingType::PageBreak`
        // candidate at every form-feed and at every `\n\n\n+` run;
        // on each such candidate we drop the accumulator and start
        // a fresh page.
        //
        // PR 6c (T069) retired the `marque_ism::PageContext` wrapper
        // type in favor of inlining the `Vec<CanonicalAttrs>`
        // accumulator at its single owner site. The `DEFAULT_PORTIONS_CAPACITY`
        // pre-size (issue #430) moves alongside the accumulator. The
        // banner/CAB rule hand-off freezes the live `Vec` into an
        // `Arc<Box<[CanonicalAttrs]>>` lazily at first banner/CAB
        // use; consecutive banner/CAB candidates on the same page
        // share that Arc through the cache below.
        let mut page_portions: Vec<marque_ism::CanonicalAttrs> = fresh_page_portions_accumulator();
        // Cache the current `Arc<Box<[CanonicalAttrs]>>` snapshot so
        // consecutive banner/CAB candidates on the same page share a
        // single allocation. Invalidated (set to None) whenever a new
        // portion is accumulated or a page break resets the
        // accumulator.
        let mut page_portions_arc: Option<Arc<Box<[marque_ism::CanonicalAttrs]>>> = None;
        // PR 9b (T133 / FR-006). Cache of the page-marking projection
        // for `RuleContext::page_marking`. Same invalidation
        // semantics as `page_portions_arc` — lazy on first banner/CAB
        // consumer, dropped on portion accumulation and on page
        // break. PR 4b-D.2 flipped the projection driver from
        // `PageContext::project` to `scheme.project(Scope::Page, ...)`
        // via the `project_page_marking` helper; the lattice + closure
        // + PageRewrite pipeline is now the single source of truth
        // for the page roll-up that banner-validation rules consume.
        let mut page_marking_arc: Option<Arc<marque_ism::ProjectedMarking>> = None;
        // Incremental page-projection accumulator. Maintained as a
        // running lattice join of all portions seen since the last
        // page reset. Updated O(1) per portion so that
        // `project_page_marking` can project from a single element
        // instead of re-folding the entire `page_portions` slice on
        // every banner/CAB candidate (O(N²) → O(N) for the
        // `fix_throughput` bench pattern where `\n\n` never trips a
        // page reset — issue #306).
        let mut page_join_acc: marque_ism::CanonicalAttrs = marque_ism::CanonicalAttrs::default();

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

        // Per-`lint()` hoist for the `E059` bridge-emitted override.
        // The construction-time `emitted_id_overrides` table eliminates
        // the per-call `Severity::parse_config` cost, but the
        // per-candidate HashMap probe remains if the lookup is inlined
        // at the bridge call site. The SCI per-system bridge runs on
        // every SCI-bearing candidate, so hoisting the `Option<Severity>`
        // out of the loop matches the precedent established by the
        // pre-PR-427 `e059_override` hoist (rust-reviewer MEDIUM on
        // commit a2fbf12b) and keeps the per-candidate path probe-free.
        let e059_override: Option<Severity> = self.emitted_id_overrides.get("E059").copied();

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
                        recognized_marking_count,
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
            // parsable content. Reset the accumulator BEFORE attempting to
            // parse — otherwise the parser's MalformedMarking error would
            // skip the continue and leave us accumulating across pages.
            if candidate.kind == MarkingType::PageBreak {
                // Issue #461: dispatch every `Phase::PageFinalization`
                // rule against the CLOSING page's fixpoint snapshot
                // BEFORE the accumulator reset, so the rule observes
                // every portion that contributed to this page. The
                // skip on empty pages is in `dispatch_page_finalization`'s
                // caller (this `if`) so an empty page costs zero rule
                // dispatches and no projection call. On deadline
                // expiry the dispatch returns `Err(())`; we propagate
                // the truncated `LintResult` the same way the
                // per-candidate deadline check at the top of the loop
                // does.
                if !page_portions.is_empty()
                    && dispatch_page_finalization(
                        &self.scheme,
                        &self.rule_sets,
                        &self.pass_finalization_rule_indices,
                        &self.fast_path_severities,
                        &self.emitted_id_overrides,
                        &page_portions,
                        &mut page_portions_arc,
                        &mut page_marking_arc,
                        &page_join_acc,
                        &corrections_arc,
                        candidate.span.start,
                        opts.deadline,
                        &mut diagnostics,
                    )
                    .is_err()
                {
                    return (
                        LintResult {
                            diagnostics,
                            truncated: true,
                            candidates_processed,
                            candidates_total,
                            recognized_marking_count,
                            ..Default::default()
                        },
                        parsed_markings,
                    );
                }
                page_portions = fresh_page_portions_accumulator();
                page_join_acc = marque_ism::CanonicalAttrs::default();
                page_portions_arc = None;
                // PR 9b (T133): the page-marking cache resets on the
                // same boundary as the per-page accumulator
                // (Constitution VI invariant — page-rollup state is
                // per-page).
                page_marking_arc = None;
                classification_floor = None;
                // PR 3c.B Commit 4: clear the per-page render
                // scratch buffer at the same boundary as the
                // per-page accumulator reset (Constitution VI
                // invariant). Commit 6's first
                // `Recanonicalize`-emitting rule depends on this
                // happening BEFORE the next page's first portion is
                // rendered.
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
            // Clamp the candidate span to `source.len()` BEFORE any
            // source indexing in this block. The existing block below
            // re-clamps for the recognizer call, but the new
            // preceded_by_whitespace / line_offset / line_prefix /
            // surrounding_lowercase computations also index into
            // `source` and must use the same clamped bounds. A
            // scanner regression that produced `span.start >
            // source.len()` would otherwise panic in
            // `source[..candidate.span.start]` before reaching the
            // existing clamp at the recognizer call site below.
            let span_start = candidate.span.start.min(source.len());
            let span_end = candidate.span.end.min(source.len());
            let preceded_by_whitespace = match span_start.checked_sub(1) {
                None => true,
                Some(prev_idx) => source
                    .get(prev_idx)
                    .map(|b| b.is_ascii_whitespace())
                    .unwrap_or(true),
            };
            // Compute the line/context signals the decoder uses to
            // discriminate real markings from prose glyphs:
            //
            // - `line_offset`: byte distance from the previous '\n'
            //   to the candidate's start. Used by the position
            //   penalty — a portion deep into a line of running
            //   prose is overwhelmingly a parenthetical, not a
            //   marking.
            // - `line_prefix`: trailing up-to-32 bytes of the line
            //   preceding the candidate. Used by the bullet anchor
            //   bonus — `1B.a.3.(c)` and `(a) (S)` patterns should
            //   NOT receive the position penalty.
            // - `surrounding_is_lowercase`: lowercase-vs-uppercase
            //   majority in a ±64 byte window. Used by the
            //   lowercase-context penalty — lowercase candidates in
            //   lowercase prose are overwhelmingly not markings.
            //   Archival all-caps documents short-circuit naturally
            //   (the candidate itself stays uppercase).
            let line_start = source[..span_start]
                .iter()
                .rposition(|&b| b == b'\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let line_offset = span_start - line_start;
            let line_prefix =
                marque_scheme::recognizer::LinePrefix::from_slice(&source[line_start..span_start]);
            let surrounding_is_lowercase =
                surrounding_lowercase_majority(source, span_start, span_end);
            let parse_cx = ParseContext {
                strict_evidence: false,
                zone: None,
                position: None,
                classification_floor,
                as_of: None,
                preceded_by_whitespace,
                line_offset: Some(line_offset),
                line_prefix: Some(line_prefix),
                surrounding_is_lowercase,
            };

            // Route each candidate's bytes through the recognizer. Zero-
            // candidate `Ambiguous` means "no plausible interpretation" —
            // skip, same as a strict-path parser error would in the old
            // flow (foundational-plan line 609-612). `Unambiguous` returns
            // a `CapcoMarking` whose `token_spans` are already absolute
            // source coordinates: per the issue #431 span-offset contract
            // the engine passes `start` as `offset` and the recognizer
            // composes the source-shift into its own internal shift
            // (e.g. strict-path leading-whitespace stripping). No
            // post-pass needed.
            let start = span_start;
            let end = span_end;
            if start >= end {
                continue;
            }
            let bytes = &source[start..end];
            let Parsed::Unambiguous(marking) =
                self.recognizer
                    .recognize(bytes, start, &self.scheme, &parse_cx)
            else {
                continue;
            };
            recognized_marking_count += 1;
            // Issue #433: defer the `parsed_markings` cache insert
            // until the end of this iteration, so we know whether any
            // diagnostic for this candidate carries a `FixIntent`.
            // `synthesize_intent_only_fixes` reads the cache ONLY for
            // diagnostics with `fix.is_some()`; candidates that
            // produce no FixIntent (the common case — most
            // diagnostics are text corrections or non-fix violations)
            // leave the cache untouched.
            //
            // The ParseContext-divergence rationale from the original
            // eager-cache site (Copilot PR #369 finding #2) is
            // preserved: the same `attrs` + `marking.1` values the
            // recognizer produced from this candidate's `parse_cx`
            // flow into the cache. The synthesis path never re-parses;
            // the deferral changes *when* the cache populates, not
            // *what* it holds.
            //
            // Snapshot `diagnostics.len()` here so the end-of-iteration
            // check can scan only the entries this candidate added.
            let diagnostics_pre_candidate = diagnostics.len();
            // Partial-move on `marking`: `attrs` consumes `marking.0`
            // by value (#434's shape — `page_context.add_portion`
            // consumes attrs at end-of-iteration, no clone). The
            // remaining `marking.1: Option<DecoderProvenance>` stays
            // accessible for both the decoder-path emit below (via
            // `as_ref()` to keep it intact) and the end-of-iteration
            // cache reconstruction.
            let attrs = marking.0;
            // Strict-path recognizers leave `marking.1` as `None`; the
            // decoder populates it with the canonical bytes /
            // posterior / features the engine needs to mint a
            // `FixSource::DecoderPosterior` diagnostic below.
            // `as_ref()` (rather than `.take()`) preserves the
            // provenance value so it can be moved into the cache at
            // end-of-iteration alongside `attrs`.

            // FR-011 strict-floor accumulator: only strict-path
            // recognitions raise the floor. A decoder-path
            // recognition (`marking.1.is_some()`) does not — we cannot
            // let a probabilistic recovery self-justify by raising
            // the threshold it then clears.
            if marking.1.is_none() {
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
            if let Some(prov) = marking.1.as_ref() {
                let span = Span::new(start, end);
                if let Some(diagnostic) = build_decoder_diagnostic(
                    span,
                    bytes,
                    prov,
                    candidate.kind,
                    self.corpus_override_active(),
                ) {
                    diagnostics.push(diagnostic);
                }
            }

            // Issue #471: gate downstream rule dispatch and page-context
            // accumulation on the decoder's recognition confidence. The
            // recognizer can return a `Parsed::Unambiguous` for a
            // candidate whose decoder posterior is below the configured
            // confidence threshold — the R001 diagnostic above informs
            // the user (and demotes to `Severity::Suggest` in the
            // post-emission pass), but the parse is NOT authoritative
            // until the user accepts it.
            //
            // Running rules against a sub-threshold decoder parse, or
            // folding its synthetic attrs into `PageContext`, mints
            // false positives keyed on a canonicalization the user
            // never approved. Concrete repro:
            // `(CTs)` / `(CMS)` in prose contexts → decoder weakly
            // recognizes both as NATO CTS (recognition ≈ 0.86, below
            // default 0.95 threshold); the synthetic
            // `MarkingClassification::Nato(_)` then triggers E015
            // `non-us-missing-dissem` at `span = 0..0` (the rule's
            // span fallback for missing Classification tokens).
            //
            // The decoder's job in this state is to *suggest*; the
            // rule pipeline's job is to enforce policy on accepted
            // canonical forms. Skipping both rule dispatch and
            // `page_context.add_portion` here aligns the two so the
            // suggestion is the only visible signal until the user
            // accepts (at which point a fix-apply re-lint exercises
            // rules on the canonical form). Constitution V Principle V
            // (audit content-ignorance) is preserved — the gate uses
            // only the `recognition` scalar and the configured
            // threshold, never document bytes.
            //
            // Strict-path parses (`marking.1.is_none()`) and decoder
            // parses meeting the threshold both fall through to the
            // rule dispatch loop unchanged.
            if let Some(prov) = marking.1.as_ref()
                && prov.recognition_score() < self.config.confidence_threshold()
            {
                continue;
            }

            // Phase 3: zone and position are Option-typed and stay None
            // until a structural scanner pass can prove them. The previous
            // hardcoded `Zone::Body`/`DocumentPosition::Body` was a silent
            // lie to any future rule that read them.
            //
            // Issue #306 (O(N²) fix): `ctx.page_portions` is consumed
            // exclusively by Phase::PageFinalization rules, which are
            // skipped in the main candidate loop (see skip gate below).
            // Materializing `page_portions_arc` here on every banner/CAB
            // candidate — cloning all k accumulated portions each time —
            // was O(N²) with zero consumers. `dispatch_page_finalization`
            // force-inits the Arc once per page break/EOD boundary; the
            // main loop passes `None` for the field unconditionally.
            //
            // (Historical: PR 6c T069 introduced the lazy-init Arc to
            // amortize the snapshot clone; issue #306 eliminates the
            // snapshot from the hot path entirely by proving no consumer
            // exists outside PageFinalization dispatch.)
            let ctx_page_portions: Option<Arc<Box<[marque_ism::CanonicalAttrs]>>> = None;
            // N-9-2 (PR 437 10th-pass): `cross_portion_context` removed.
            // The field cloned the full per-page accumulator once per
            // Portion candidate (O(N²) over N portions per page —
            // clone at portion K copies K `CanonicalAttrs` values, so
            // total cost is 0+1+...+(N-1)). W004 `joint-disunity-
            // collapse` was the only planned consumer but was reverted
            // to Banner-only in the 8th-pass (P-3 trade-off: portion-
            // time snapshots can't distinguish DisunityCollapse from a
            // future Mixed state per §H.3 p57). Per Constitution
            // Principle I, O(N²) hot-path cost MUST be benchmarked;
            // zero-consumer O(N²) work fails that gate. Future cross-
            // portion aggregation rules must use a lazy/gated approach
            // with explicit capability declaration — see `RuleContext`
            // doc note added in this PR.
            //
            // PR 9b (T133): lazy/cached construction for the
            // page-marking projection. Built from
            // `project_page_marking(&self.scheme, &page_join_acc)`
            // (post-PR-4b-D.2 hot-path flip — the helper invokes
            // `CapcoScheme::project_from_attrs_slice` which drives
            // the lattice + closure + page-rewrite pipeline) so
            // banner-validation rules see the rolled-up shape
            // (classification / SCI / SAR / AEA / dissem_us /
            // dissem_nato / REL TO). `page_join_acc` is the
            // incremental lattice join over all portions seen so far
            // on this page (issue #306, PR #674); the Arc is shared
            // across consecutive banner/CAB candidates on the same
            // page and invalidated on page break.
            let ctx_page_marking =
                if candidate.kind != MarkingType::Portion && !page_portions.is_empty() {
                    Some(
                        page_marking_arc
                            .get_or_insert_with(|| {
                                Arc::new(project_page_marking(&self.scheme, &page_join_acc))
                            })
                            .clone(),
                    )
                } else {
                    None
                };
            // PR 7c: look up the pre-pass-1 attrs for this marking
            // span when the engine is dispatching the post-pass-1
            // re-lint (`TwoPassFixer` threads a cache through here
            // via `lint_with_options_internal`'s `pre_pass_1_cache`
            // parameter). First-lint dispatch (pass-0) and any
            // external `lint_with_options` call see `None` because no
            // pass-1 fix has yet been promoted.
            let pre_pass_1_attrs =
                pre_pass_1_cache.and_then(|cache| pre_pass_1_attrs_for_span(cache, candidate.span));
            // PR 3c.B engine-prereq: the scanner's candidate span is
            // the marking-scope anchor for intent-only fix synthesis.
            // Rules emitting `FixIntent` copy this into
            // `Diagnostic.candidate_span` so the engine can clone the
            // marking, apply intents via `MarkingScheme::apply_intent`,
            // and render the result via `MarkingScheme::render_canonical`.
            //
            // PR 4b-B 9th-pass follow-up: `RuleContext` is now
            // `#[non_exhaustive]`; cross-crate construction must go
            // through `RuleContext::new` + `with_*` setters because
            // both bare-literal and `..base` functional-update
            // construction are blocked across crate boundaries on
            // a non-exhaustive struct. New optional fields land in
            // `RuleContext::new` as `None` defaults and gain a
            // `with_*` setter; the engine's hot-path call site
            // chains the setters here once per candidate dispatch.
            let ctx = RuleContext::new(candidate.kind, candidate.span)
                .with_page_portions(ctx_page_portions)
                .with_page_marking(ctx_page_marking)
                .with_corrections(corrections_arc.clone())
                .with_pre_pass_1_attrs(pre_pass_1_attrs);
            for (set_idx, rule_set) in self.rule_sets.iter().enumerate() {
                for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
                    // `Phase::PageFinalization` rules are dispatched
                    // exclusively by `dispatch_page_finalization` at
                    // every scanner-emitted `MarkingType::PageBreak`
                    // (BEFORE the per-page accumulator reset) and
                    // once at end-of-document. Skipping them in the
                    // main candidate loop is what makes the "fires
                    // once on the page fixpoint" contract documented
                    // on `Phase::PageFinalization`
                    // (`crates/rules/src/lib.rs`) mechanically true.
                    // Without this skip the rule would ALSO fire on
                    // every Banner/CAB candidate whose
                    // `ctx.page_portions` is populated, producing a
                    // duplicate diagnostic per page on layouts with
                    // a closing banner — caught by Copilot review on
                    // PR #487 (issue #461).
                    //
                    // The linear scan over `pass_finalization_rule_indices`
                    // is intentional: the SmallVec is inline-4 and holds
                    // 2 entries today (W004 from issue #461; S005 from
                    // issue #488), growing to a small handful as S007 +
                    // BannerMatchesProjectedRule migrate. A HashSet probe
                    // pays a hash + bucket traversal per rule per
                    // candidate; a 1-3 element linear scan is faster and
                    // generates simpler code. Revisit if the bucket grows
                    // past ~16 entries.
                    if self
                        .pass_finalization_rule_indices
                        .iter()
                        .any(|&(s, r)| s == set_idx && r == rule_idx)
                    {
                        continue;
                    }

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
                    //
                    // The configured-severity lookup is pre-resolved
                    // at engine construction time
                    // (`fast_path_severities[set_idx][rule_idx]`,
                    // built by `build_severity_tables`) — the
                    // pre-hoist code did a `HashMap<String, String>`
                    // probe + a `Severity::parse_config` parse on
                    // every (candidate × rule) pair; both moved to
                    // construction time. Walker rules also have an
                    // entry here but it stays unread because the
                    // `additional_emitted_ids().is_empty()` guard
                    // gates this whole block.
                    if rule.additional_emitted_ids().is_empty() {
                        let configured_severity = self.fast_path_severities[set_idx][rule_idx];
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
                    //
                    // `Rule::trusted()` (defaulted to `false`) lets the
                    // engine bypass `catch_unwind` for rules audited as
                    // panic-safe. In-tree CAPCO rules override to `true`
                    // (the catalog is audited as a set); out-of-tree
                    // rules inherit safe-by-default and keep the
                    // containment. The bypass is the deliberate
                    // `unsafe`-block-shaped opt-out documented on
                    // `Rule::trusted()`.
                    let rule_id = rule.id();
                    let mut diags = if rule.trusted() {
                        rule.check(&attrs, &ctx)
                    } else {
                        match std::panic::catch_unwind(AssertUnwindSafe(|| {
                            rule.check(&attrs, &ctx)
                        })) {
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
                    // lookup per diagnostic against the pre-resolved
                    // `emitted_id_overrides` table (built once at
                    // engine construction; the per-diagnostic
                    // `Severity::parse_config` parse the pre-hoist
                    // code did in the hot loop is gone). Off drops
                    // the diagnostic; a non-Off override replaces the
                    // rule-emitted severity; absence keeps it (which
                    // for non-walker rules matches
                    // `rule.default_severity()` by convention; for
                    // walker rules carries the per-row catalog
                    // severity).
                    diags.retain_mut(|d| {
                        match self.emitted_id_overrides.get(d.rule.as_str()).copied() {
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
                for v in self.scheme.validate(&marking) {
                    if let Some(diag) = self.bridge_constraint_diagnostic(&v, &attrs, candidate) {
                        diagnostics.push(diag);
                    }
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
                // The override value is pre-resolved at engine
                // construction time in `emitted_id_overrides` (no parse
                // cost per call) and additionally hoisted to
                // `e059_override` once per `lint()` call (no per-candidate
                // HashMap probe). Both layers are loop-invariant by
                // construction.
                // SCI per-system FactAdd scope tracks the candidate's
                // marking type: a portion candidate emits at portion
                // scope; a banner candidate emits at page scope (the
                // FactAdd applies to the banner roll-up's per-page
                // projection). Two engine-internal `MarkingType`
                // variants don't reach here, by separate-but-equivalent
                // exclusion paths:
                //   - `MarkingType::PageBreak` (scanner-emitted): the
                //     outer candidate loop's early-`continue` at the
                //     page-break branch fires above the scheme bridge
                //     call.
                //   - `MarkingType::PageFinalization` (issue #461,
                //     engine-synthesized): never appears in the
                //     scanner-emitted candidate stream; the
                //     `dispatch_page_finalization` helper handles its
                //     own RuleContext and bypasses this bridge
                //     entirely (the synthetic candidate carries no
                //     parsed attrs to bridge over).
                // `MarkingType` is `#[non_exhaustive]` so the `_` arm
                // also fields future variants safely.
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

            // Issue #433 + #434 end-of-iteration synthesis: decide
            // cache-insert (#433) AND page-context accumulation (#434)
            // by candidate.kind × intent_emitted. The four arms
            // combine both wins:
            //
            //   - `marking.1` (the provenance Option) is moved into the
            //     cache by value alongside the attrs, preserving the
            //     decoder's `Some(...)` payload (Copilot PR #369
            //     finding #2 — the same recognition the lint phase
            //     saw flows into `synthesize_intent_only_fixes`).
            //   - For non-Portion candidates we never call
            //     `add_portion`, so attrs simply moves into the cache
            //     (when an intent fires) or drops (otherwise) — no
            //     clones.
            //   - For Portion candidates WITHOUT an intent, attrs
            //     moves into `add_portion` — #434's by-value win
            //     stays intact for the common path.
            //   - For Portion candidates WITH an intent (rare:
            //     E014 JOINT-RELTO, E021 AEA-NOFORN, and similar
            //     FactAdd rules), we must satisfy both consumers,
            //     so we clone attrs once. The clone cost is bounded
            //     by the (already-rare) Portion+intent intersection.
            //
            // `diagnostics_pre_candidate` is captured at the top of
            // this iteration before any diagnostic-emitting site
            // fires. The slice bound scopes the predicate to
            // diagnostics this candidate added — any new
            // diagnostic-emitting site added between the snapshot and
            // this block MUST stay above this block for the predicate
            // to remain accurate.
            let intent_emitted = diagnostics[diagnostics_pre_candidate..]
                .iter()
                .any(|d| d.fix.is_some());
            // Issue #432 invariant: `parsed_markings` stays sorted by
            // `Span.start` because PageBreak candidates are filtered
            // above (early `continue`) and the remaining content
            // candidates have distinct starts at the cache-insertion
            // boundary. `lookup_marking`'s `binary_search_by_key` on
            // `Span.start` is sound iff this holds. The assertion runs
            // only in debug builds; the release path stays unconditional
            // push.
            debug_assert!(
                parsed_markings
                    .last()
                    .is_none_or(|(prev, _)| prev.start < candidate.span.start),
                "parsed_markings push violated strictly-increasing-start invariant: \
                 prev.start={:?} candidate.span.start={}",
                parsed_markings.last().map(|(s, _)| s.start),
                candidate.span.start
            );
            if candidate.kind == MarkingType::Portion {
                // Update the incremental join accumulator (issue #306).
                // First portion on the page: copy directly so that
                // cross-axis state in `join_via_lattice_body` (JointSet,
                // DissemSet unanimity, etc.) is seeded from a real
                // portion, NOT from `CanonicalAttrs::default()`.
                // `CanonicalAttrs::default()` has `classification = None`,
                // which `JointSet::from_attrs_iter` counts as a non-JOINT
                // element — joining it with a JOINT portion would falsely
                // produce `Mixed` instead of `UnanimousProducers`.
                // For k ≥ 1 the join is correct because `page_join_acc`
                // already carries a real (post-overlay) CanonicalAttrs,
                // so `JointSet::from_attrs_iter([acc, new_p])` sees
                // `acc.classification = Some(Joint(...))` and correctly
                // detects the unanimous case.
                if page_portions.is_empty() {
                    page_join_acc = attrs.clone();
                } else {
                    page_join_acc = marque_capco::CapcoMarking::join_via_lattice(
                        &[std::mem::take(&mut page_join_acc), attrs.clone()],
                    );
                }
                if intent_emitted {
                    parsed_markings.push((
                        candidate.span,
                        marque_capco::CapcoMarking(attrs.clone(), marking.1),
                    ));
                    page_portions.push(attrs);
                } else {
                    page_portions.push(attrs);
                }
                // Invalidate the cached Arc so the next banner/CAB gets a
                // fresh snapshot. We rebuild it lazily above on the next
                // iteration when a non-Portion candidate arrives.
                page_portions_arc = None;
                // PR 9b (T133): the projected page marking also goes
                // stale when a new portion arrives.
                page_marking_arc = None;
            } else if intent_emitted {
                parsed_markings
                    .push((candidate.span, marque_capco::CapcoMarking(attrs, marking.1)));
            }
        }

        // Issue #461: end-of-document PageFinalization dispatch.
        // After the candidate loop closes, the final page's
        // accumulator still holds every trailing portion that did
        // not precede a `MarkingType::PageBreak` boundary. Without
        // this dispatch, banner-first / single-page layouts (no
        // closing page-break, no footer banner) would never see
        // their PageFinalization rules fire — the documented
        // false-negative W004 had under the pre-#461 Banner-only
        // firing.
        //
        // Skips when the final page is empty (no portions in the
        // accumulator), mirroring the PageBreak branch above. The
        // boundary anchor is `source.len()` (the EOD offset). On
        // deadline expiry the dispatch returns `Err(())` and we
        // propagate the truncated `LintResult` the same way the
        // PageBreak branch does.
        if !page_portions.is_empty()
            && dispatch_page_finalization(
                &self.scheme,
                &self.rule_sets,
                &self.pass_finalization_rule_indices,
                &self.fast_path_severities,
                &self.emitted_id_overrides,
                &page_portions,
                &mut page_portions_arc,
                &mut page_marking_arc,
                &page_join_acc,
                &corrections_arc,
                source.len(),
                opts.deadline,
                &mut diagnostics,
            )
            .is_err()
        {
            return (
                LintResult {
                    diagnostics,
                    truncated: true,
                    candidates_processed,
                    candidates_total,
                    recognized_marking_count,
                    ..Default::default()
                },
                parsed_markings,
            );
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
                .emitted_id_overrides
                .get("C001")
                .copied()
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
                        // G13: `key` is intentionally not interpolated — the
                        // typed `Message` identifies the corrections-map class.
                        // `CORRECTIONS_MAP_CITATION` is now a typed `Citation`
                        // with `AuthoritativeSource::Config`. `value` IS used
                        // below as the replacement payload (`.as_ref()` at the
                        // Diagnostic::text_correction call) — only `key` is
                        // discarded here.
                        let _ = key;
                        diagnostics.push(Diagnostic::text_correction(
                            RuleId::new("C001"),
                            c001_severity,
                            span,
                            marque_rules::Message::new(
                                marque_rules::MessageTemplate::CorrectionsApplied,
                                marque_rules::MessageArgs::default(),
                            ),
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
                recognized_marking_count,
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
    /// return the corrected source + applied audit lines + dropped diagnostics.
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
        Vec<Diagnostic<CapcoScheme>>,
        Vec<AuditLine<CapcoScheme>>,
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
                    severity: d.severity,
                    span: d.span,
                    replacement: tc.replacement.clone(),
                    confidence: tc.confidence.clone(),
                    source: tc.source,
                    message: d.message.clone(),
                    migration_ref: tc.migration_ref,
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
        let mut audit_lines: Vec<AuditLine<CapcoScheme>> = Vec::with_capacity(kept.len());
        for fix in kept {
            // PM-D-6 / G13: hash pre-correction bytes BEFORE the
            // splice. `original_bytes` borrows from `source` for the
            // hashing call only — never stored in an audit-record
            // field. Order: hash → splice → audit, so the splice
            // doesn't invalidate `original_bytes` and the audit
            // captures the original-bytes digest.
            let original_bytes = &source[fix.span.start..fix.span.end];
            let original_digest = blake3::hash(original_bytes);

            // Splice the canonical replacement into the buffer.
            buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());

            // marque-1.0 promote — AppliedTextCorrection (separate
            // line type per PM-D-4; CorrectionsMap and text-correction-
            // shaped rule emissions route here, never to the marking-
            // side Discriminant).
            let text_correction = AppliedTextCorrection::__engine_promote_text_correction(
                fix.rule,
                fix.severity,
                fix.span,
                original_digest,
                fix.replacement,
                fix.source,
                fix.confidence,
                fix.migration_ref,
                fix.message,
                now,
                classifier_id.clone(),
                dry_run,
                None,
                engine_promotion_token(),
            );
            audit_lines.push(AuditLine::TextCorrection(text_correction));
        }

        (buf, dropped_diags, audit_lines)
    }

    /// Translate a scheme-emitted [`ConstraintViolation`] into an
    /// engine-side [`Diagnostic`].
    ///
    /// Returns `None` for advisory violations — entries whose `span`
    /// or `severity` is `None` are tooling-only signals that never
    /// surface to users. Returns `None` for severity-`Off` overrides
    /// (FR-008: `Off`-severity diagnostics are unrepresentable).
    ///
    /// For qualifying violations the bridge:
    ///
    /// 1. Folds the catalog row's `constraint_label` into a stable
    ///    `RuleId` (e.g. `class-floor/...` / `E058/...` → `E058`,
    ///    `sci-per-system/...` → `E059`, `E054/...` → `E054`,
    ///    `capco/noforn-conflicts-rel-to` → `E053`).
    /// 2. Applies the user-configured severity override
    ///    (`emitted_id_overrides`) keyed on the resolved `RuleId`.
    /// 3. Synthesizes the optional [`FixIntent`] via
    ///    [`CapcoScheme::fix_intent_by_name`] from the row name +
    ///    `attrs` + candidate `MarkingType`.
    /// 4. Resolves the user-facing message via
    ///    [`CapcoScheme::message_by_name`]; falls back to the generic
    ///    evaluator text from `ConstraintViolation.message` when the
    ///    scheme returns `None` for the row name.
    /// 5. Builds the [`Diagnostic`] with the resolved `message`
    ///    and `citation` carried through verbatim, and stamps the
    ///    candidate's outer span as `candidate_span`.
    ///
    /// [`ConstraintViolation`]: marque_scheme::ConstraintViolation
    /// [`Diagnostic`]: marque_rules::Diagnostic
    /// [`FixIntent`]: marque_rules::FixIntent
    /// [`CapcoScheme::fix_intent_by_name`]: CapcoScheme::fix_intent_by_name
    /// [`CapcoScheme::message_by_name`]: CapcoScheme::message_by_name
    fn bridge_constraint_diagnostic(
        &self,
        v: &marque_scheme::ConstraintViolation,
        attrs: &marque_ism::CanonicalAttrs,
        candidate: &marque_ism::MarkingCandidate,
    ) -> Option<marque_rules::Diagnostic<CapcoScheme>> {
        use marque_rules::{Diagnostic, RuleId, Severity};

        let span = match v.span {
            Some(s) => s,
            None => {
                tracing::trace!(
                    target: "marque_engine::constraint_bridge",
                    constraint = v.constraint_label,
                    "advisory constraint violation (no span); not surfaced as Diagnostic"
                );
                return None;
            }
        };

        let severity = match v.severity {
            Some(s) => s,
            None => {
                tracing::trace!(
                    target: "marque_engine::constraint_bridge",
                    constraint = v.constraint_label,
                    "advisory constraint violation (no severity); not surfaced as Diagnostic"
                );
                return None;
            }
        };

        let rule_id = if v.constraint_label.starts_with("class-floor/")
            || v.constraint_label.starts_with("E058/")
        {
            RuleId::new("E058")
        } else if v.constraint_label.starts_with("sci-per-system/") {
            RuleId::new("E059")
        } else if let Some(id_part) = v.constraint_label.split('/').next() {
            // PR for #388 (W005 rel-to-not-in-joint-coverage): extend the
            // bridge's structural ID prefix recognition from `E` only to
            // `E | W`. The bridge previously routed every Warn-class
            // constraint-catalog row to the `E008` fallback, which both
            // misattributed the diagnostic and prevented severity-config
            // resolution via the rule's own ID.
            //
            // Constitution VII precedent (engine-crate edit in a scheme-
            // adoption PR): structural bridge gap revealed by the W005
            // adoption — analogous to the PR 4b-B Commit 2 PageContext
            // bugfixes (OC-USGOV / RELIDO supersession). Bugfix-class
            // change confined to the bridge's ID-recognition predicate;
            // the engine's rule-execution semantics are unchanged.
            if matches!(
                id_part.as_bytes(),
                [b'E' | b'W', b'0'..=b'9', b'0'..=b'9', b'0'..=b'9']
            ) {
                RuleId::new(id_part)
            } else if v.constraint_label == "capco/noforn-conflicts-rel-to" {
                RuleId::new("E053")
            } else {
                RuleId::new("E008") // Fallback to Unrecognized (should be rare)
            }
        } else {
            RuleId::new("E008")
        };

        let final_severity = self
            .emitted_id_overrides
            .get(rule_id.as_str())
            .copied()
            .unwrap_or(severity);

        if final_severity == Severity::Off {
            return None;
        }

        let fix_intent = self
            .scheme
            .fix_intent_by_name(v.constraint_label, attrs, candidate.kind);

        // PR 3c.2.C C5 / PM-C-1 / PR 10.A.1 bridge layer: convert the
        // carrier-string `ConstraintViolation.message: String` to a typed
        // `Diagnostic.message: Message`. The citation channel is no
        // longer bridged — PR 10.A.1 made `Constraint.label: Citation`
        // typed at declaration, and `ConstraintViolation.citation:
        // Citation` flows verbatim through the evaluator. The
        // `citation_by_name` lookup and `EngineInternal` sentinel
        // fallback were retired in PR 10.A.1.
        //
        // The message lookup still falls back to a generic sentinel
        // when the constraint_label is not in the explicit mapping;
        // the fallback shape preserves audit-content-ignorance (no
        // `v.message` raw bytes flow through).
        let message = self
            .scheme
            .message_by_name(v.constraint_label, attrs, candidate.kind)
            .unwrap_or_else(|| {
                // Unknown constraint label — emit a generic
                // `ConflictsWith` template with no args so the audit
                // record is still closed-template. The original String
                // message is dropped (G13). Future labels SHOULD be
                // added to `message_by_name` explicitly.
                tracing::trace!(
                    target: "marque_engine::constraint_bridge",
                    constraint = v.constraint_label,
                    "no typed Message mapping for constraint_label; using generic fallback",
                );
                marque_rules::Message::new(
                    marque_rules::MessageTemplate::ConflictsWith,
                    marque_rules::MessageArgs::default(),
                )
            });

        // PR 10.A.1: catalog-row citations are now typed end-to-end. The
        // `ConstraintViolation.citation: Citation` value flowed verbatim
        // from the constraint's `label: Citation` declaration via
        // `marque_scheme::constraint::evaluate`, so the bridge is a direct
        // copy — the prior `citation_by_name` fallback and
        // `EngineInternal` sentinel are gone.
        let citation = v.citation;

        let mut diag =
            Diagnostic::with_fix(rule_id, final_severity, span, message, citation, fix_intent);
        diag.candidate_span = Some(candidate.span);
        Some(diag)
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
///
/// The post-splice buffer is wrapped in [`zeroize::Zeroizing`] so the
/// scratch bytes wipe on drop — Constitution Principle II. Pass-1's
/// post-buffer flows into pass-2's dispatch and then drops; pass-2's
/// post-buffer flows into the public [`FixResult.source`]
/// [`secrecy::SecretSlice`] via [`into_secret_slice`]. Either way, the
/// transient `Vec<u8>` never sits in freed memory unwiped.
/// Tuple returned by [`TwoPassFixer::apply_kept_fixes`].
///
/// Carries the post-pass output buffer + the `marque-1.0` audit-line
/// stream + the `(rule_id, span)` keys of applied fixes. Post-cutover
/// (PR 3c.2.D) `audit_lines` is the sole audit channel.
type AppliedTuple = (
    Zeroizing<Vec<u8>>,
    HashSet<(RuleId, Span)>,
    Vec<AuditLine<CapcoScheme>>,
);

/// Move a [`Zeroizing<Vec<u8>>`] into a [`SecretSlice<u8>`] without
/// leaking content through a shrink-reallocation.
///
/// The naive `vec.into()` path goes through [`Vec::into_boxed_slice`],
/// which **may reallocate to shrink** when `capacity > len` — and
/// the freed source allocation contains the content bytes the
/// realloc just copied, never wiped. [`splice_fixes_forward`]'s
/// pre-sized `Vec::with_capacity(source.len() + extra)` lands in
/// that state whenever a fix shrinks bytes (replacement shorter
/// than its span), so the channel is reachable on every
/// shrinking-fix path.
///
/// This helper sidesteps the channel by allocating the destination
/// `Box<[u8]>` separately via [`Box::from`] over a borrowed slice.
/// The `T: Copy` specialization for `u8` allocates exactly
/// `slice.len()` bytes through `RawVec::with_capacity` and
/// constructs the `Box<[u8]>` with `len == slice.len()` — no
/// shrinking realloc, no freed-and-unwiped buffer. The source
/// [`Zeroizing<Vec<u8>>`] retains ownership of its (possibly
/// over-allocated) original buffer through the copy and drops at
/// end-of-scope, wiping its full capacity via the `Vec<u8>`
/// [`zeroize::Zeroize`] impl before the Vec's backing memory is
/// freed.
///
/// Trade-off: one additional `len`-byte allocation and memcpy per
/// fix call (~1µs on 10KB inputs). Constitutional ceilings
/// (SC-001 16ms p95 on 10KB) remain comfortable.
///
/// Constitution Principle II — the single Zeroizing → SecretSlice
/// transition for the public `FixResult.source` field. Engine-only
/// helper; not exported.
#[inline]
fn into_secret_slice(z: Zeroizing<Vec<u8>>) -> SecretSlice<u8> {
    let bytes: Box<[u8]> = Box::from(&z[..]);
    SecretBox::new(bytes)
    // `z` drops here. `Zeroizing::drop` wipes its full capacity
    // (including the over-allocation tail that motivated this
    // helper) BEFORE the backing Vec frees its buffer.
}

/// Pre-pass-1 attribute cache entries (PR 7c / FR-023 / R-4).
///
/// One entry per marking whose span overlaps a pass-1 fix. The
/// engine builds the cache before the pass-1 splice so the
/// `CanonicalAttrs` snapshot reflects the bytes the rule originally
/// matched against. Inline-4 matches the existing
/// `Phase::Localized` rule cap (C001 / E006 / E007 / S004 — at most
/// one fix per Localized rule per marking; the typical document
/// has ≤4 reshape sites). Spills to heap on dense documents.
///
/// The `Span` keys are the **marking spans** (i.e., scanner
/// candidate spans), not the fix sub-spans. Lookup is a linear scan
/// using `span_is_within_marking` so a query span (a candidate
/// span at re-lint time) finds the cache entry whose marking
/// contains it. The post-pass-1 re-lint may produce candidates with
/// shifted offsets (the splice changed byte positions), so an
/// exact-equality keying scheme would miss every entry. The
/// containment scan is robust because spans grow monotonically
/// left-to-right; for any post-splice candidate the originating
/// pre-pass-1 marking is the unique cache entry whose pre-splice
/// span contained the same source bytes.
///
/// Inline-4 storage is 4 × `sizeof(Span) + sizeof(CanonicalAttrs)`
/// ≈ 4 × (16 + 112) = ~512 B on the stack. SmallVec spill to heap
/// is acceptable when documents exceed the cap (rust pre-flight §2).
type PrePass1Cache = SmallVec<[(Span, marque_ism::CanonicalAttrs); 4]>;

/// Look up the pre-pass-1 attrs for a marking whose span contains
/// `query_span`. Linear scan over the ≤4-entry cache; the call is
/// per-candidate inside `lint_with_options_internal_with_cache` and
/// the inline-4 bound makes the scan stack-only on typical inputs.
///
/// Returns `Some(&attrs)` when a cache entry's marking span contains
/// the query span; `None` otherwise. The CONTENT of the entry
/// (`CanonicalAttrs`) is borrowed into `RuleContext.pre_pass_1_attrs`
/// as the architectural two-pass-reshape signal — no current rule
/// reads it, but the field stays plumbed for future consumers
/// (D-7.22).
#[inline]
fn pre_pass_1_attrs_for_span(
    cache: &[(Span, marque_ism::CanonicalAttrs)],
    query_span: Span,
) -> Option<&marque_ism::CanonicalAttrs> {
    cache.iter().find_map(|(marking_span, attrs)| {
        if span_is_within_marking(query_span, *marking_span) {
            Some(attrs)
        } else {
            None
        }
    })
}

/// Outcome of pass-0 (text-corrections, UNCHANGED behavior).
///
/// `effective_source` is wrapped in [`Zeroizing`] per Constitution
/// Principle II — Marque-owned scratch buffers wipe on drop.
struct Pass0Result {
    /// Source bytes after pass-0 text corrections have been applied.
    /// Equals `source.to_vec()` when no text corrections fired.
    effective_source: Zeroizing<Vec<u8>>,
    /// Promoted `marque-1.0` audit-line records from pass-0. Each
    /// entry is an [`AuditLine::TextCorrection`] for the pass-0
    /// path.
    audit_lines: Vec<AuditLine<CapcoScheme>>,
    /// Diagnostics whose text-correction fixes were dropped by the
    /// C-1 overlap guard during pass-0. Surfaced via
    /// `FixResult.remaining_diagnostics` because pass-2's re-lint
    /// runs on the corrected buffer and would not re-emit them.
    dropped_diags: Vec<Diagnostic<CapcoScheme>>,
}

/// Outcome of pass-1 ([`Phase::Localized`] rule fixes).
///
/// `post_buffer` is wrapped in [`Zeroizing`] per Constitution
/// Principle II.
struct Pass1Result {
    /// Buffer after pass-1 fixes have been spliced into `effective_source`.
    /// Equals `effective_source` when pass-1 produced no fixes.
    post_buffer: Zeroizing<Vec<u8>>,
    /// Promoted `marque-1.0` audit-line records from pass-1. Each
    /// entry is an [`AuditLine::AppliedFix`] for the pass-1 marking
    /// path.
    audit_lines: Vec<AuditLine<CapcoScheme>>,
    /// `(rule_id, span)` keys of pass-1 fixes — feeds the
    /// `remaining_diagnostics` filter so a fixed diagnostic is not
    /// reported again.
    applied_keys: HashSet<(RuleId, Span)>,
}

/// Outcome of pass-2 ([`Phase::WholeMarking`] rule fixes).
///
/// `output` is wrapped in [`Zeroizing`] per Constitution Principle II.
/// On the happy path it transfers to [`FixResult.source`]
/// ([`SecretSlice<u8>`]) via [`into_secret_slice`] — the wipe
/// guarantee flows from the scratch wrapper to the public wrapper.
struct Pass2Result {
    /// Final buffer (Apply mode) or original source (DryRun).
    output: Zeroizing<Vec<u8>>,
    /// Promoted `marque-1.0` audit-line records from pass-2. Each
    /// entry is an [`AuditLine::AppliedFix`].
    audit_lines: Vec<AuditLine<CapcoScheme>>,
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
        let (lint, parsed_markings) = if !pass0.audit_lines.is_empty() {
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

        // Pass-1 sees the pre-pass-1 partition (`pass1_diags`) of the
        // `lint` diagnostic stream. The reference vector borrows from
        // `lint.diagnostics`; we keep the borrow scoped to the pass-1
        // dispatch call so the borrow checker can let `lint` move
        // (or be replaced by `relint`) in the rebind below.
        //
        // We deliberately drop the pre-pass-1 `pass2_diags` partition
        // immediately after pass-1: the post-pass-1 rebind below may
        // replace `lint` with `relint`, and pass-2 in either branch
        // operates on the post-rebind `lint.diagnostics`. The fresh
        // re-partition is the correct pass-2 input (FR-023 partial —
        // Copilot round-1 #2). Reference partitioning is O(N) pointer
        // pushes; the prior owned-vec clone allocated O(N) Diagnostic
        // bodies on every call (Constitution I).
        let pass1 = {
            let (pass1_diags, _pass2_diags_pre) =
                partition_diags_by_phase(&lint.diagnostics, &localized_ids);
            self.run_pass1_localized(
                &pass0.effective_source,
                &parsed_markings,
                &pass1_diags,
                &lint,
            )?
        };

        // PR 7c — capture pre-pass-1 attrs for every marking whose
        // span overlaps a pass-1 applied fix. The cache is owned on
        // this stack frame so the references it spawns
        // (`RuleContext.pre_pass_1_attrs`) cannot outlive `run()`.
        // `parsed_markings` is still the pre-pass-1 cache at this
        // point — the re-parse arm below will move ownership of it
        // (replacing it with a fresh post-pass-1 cache), so the
        // snapshot has to land BEFORE that branch. Empty when pass-1
        // promoted no fixes; the field-only consumer
        // (`RuleContext.pre_pass_1_attrs`) sees `None` in that case.
        // The originally-planned `PrecedingFixPenalty` consumer of
        // this cache was retired in PR 7c per D-7.22; the cache + the
        // field stay as the architectural two-pass-reshape signal
        // for future rule consumers.
        let pre_pass_1_cache = self.populate_pre_pass_1_cache(&pass1.audit_lines, &parsed_markings);

        // Destructure pass0 into owned locals so the re-parse / R002
        // branches can move `effective_source` directly (eliminating
        // the prior `.clone()` of the full document buffer on the
        // hot path) while `audit_lines` and `dropped_diags` flow
        // through to the merge step or the R002 assembler without an
        // intermediate `&Pass0Result` borrow that forces cloning.
        let Pass0Result {
            effective_source: pass0_effective_source,
            audit_lines: pass0_audit_lines,
            dropped_diags: pass0_dropped_diags,
        } = pass0;

        // Re-parse decision. Short-circuit when pass-1 produced no
        // applied fixes — the byte stream is unchanged, so the
        // pass-2 lint baseline is identical to the post-pass-0 lint
        // and we can reuse `parsed_markings` AND the pre-pass-1
        // `pass2_diags` partition directly.
        //
        // CanonicalAttrs is owned (no `<'src>` parameter), and
        // parsed_markings is `Vec<(Span, CapcoMarking)>` (issue
        // #432 swapped the type from `HashMap` to a sorted `Vec`).
        // Moving it in both branches keeps both arms producing the
        // same owned type — no `Cow`, no clone (rust pre-flight Q3).
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
        //
        // `lint` is rebound to the post-pass-1 re-lint on the re-parse
        // arm so EVERY downstream consumer — the pass-2 dispatch, the
        // `DeadlineExceeded { partial_lint }` payload, and the
        // remaining-diagnostics filter — sees the same post-pass-1
        // state. Round 2 of Copilot review caught three call sites
        // that previously kept reading the pre-pass-1 `lint` here;
        // tupling `lint` through the decision is what makes
        // post-pass-1 propagation total rather than partial. The
        // R002 short-circuit explicitly passes the **pre-pass-1**
        // `lint` into `assemble_r002_result`: pass-1 destroyed the
        // marking shape, so post-pass-1 diagnostics are degenerate
        // and the surfaced remaining-diagnostics stream should
        // reflect what the operator saw before pass-1 ran.
        // Rebind `(pass2_source, pass2_markings, pass1_applied,
        // pass1_applied_keys, lint)` first — splitting `pass2_diags`
        // off into its own subsequent let-binding (below) is what
        // unblocks the reference-propagation refactor (Copilot
        // round-3 R3-2). With the old single-tuple shape, the `else`
        // arm tried to move both `relint` and a `Vec<&relint.diagnostics>`
        // out together, which Rust correctly rejects as a
        // self-referential bundle. Splitting lets `lint` settle to
        // its final owner first, then `pass2_diags` borrows from it.
        let (pass2_source, pass2_markings, pass1_audit_lines, pass1_applied_keys, lint) =
            if pass1.audit_lines.is_empty() {
                // Short-circuit: pass-1 produced no audit lines, so the
                // byte stream is unchanged. Move `pass0_effective_source`
                // directly into `pass2_source` — no document-buffer clone.
                // The pre-pass-1 `lint` is still current here (no byte
                // change → no re-lint needed) and is moved through.
                let Pass1Result {
                    post_buffer: _,
                    audit_lines,
                    applied_keys,
                } = pass1;
                (
                    pass0_effective_source,
                    parsed_markings,
                    audit_lines,
                    applied_keys,
                    lint,
                )
            } else {
                // PR 7c — pass the pre-pass-1 attrs cache into the
                // post-pass-1 re-lint so every candidate's RuleContext
                // gets a populated `pre_pass_1_attrs` field when its
                // span overlaps a pass-1-reshaped marking. The field
                // is the architectural two-pass-reshape signal kept
                // for future rule consumers (D-7.22 retired the
                // originally-planned `PrecedingFixPenalty` engine
                // consumer; the field's data source — the cache —
                // stays plumbed.)
                let (relint, new_markings) = self.engine.lint_with_options_internal_with_cache(
                    &pass1.post_buffer,
                    &lint_opts,
                    Some(&pre_pass_1_cache),
                );
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
                // Issue #433 note on the sentinel signals: the
                // R002 trigger is "pre had recognized markings AND
                // post has zero." Under #433's deferred cache,
                // `parsed_markings.is_empty()` no longer answers
                // "had any recognized marking" — only "had any
                // FixIntent-bearing marking." So both sides of the
                // conjunction now read `LintResult
                // .recognized_marking_count`, which is incremented
                // on every `Parsed::Unambiguous` recognition and
                // is independent of the cache-population gate.
                // Using the cache-emptiness signal here would
                // produce both false negatives (pre had only
                // text-correction-emitting markings and pass-1
                // destroyed them — sentinel never fires) and
                // false positives (pass-1 fixed the only
                // FixIntent issue and the re-lint cleanly parses
                // — sentinel fires spuriously). The recognized-
                // count signal avoids both.
                let post_pass1_had_no_markings = relint.recognized_marking_count == 0;
                let pre_pass1_had_markings = lint.recognized_marking_count > 0;
                if post_pass1_had_no_markings && pre_pass1_had_markings {
                    let contributing = self.contributing_pass1_rule_ids(&pass1.audit_lines);
                    let failure_span = Span::new(0, pass1.post_buffer.len());
                    let r002 = build_r002_diagnostic(contributing, failure_span);
                    return Ok(self.assemble_r002_result(
                        pass0_audit_lines,
                        pass0_dropped_diags,
                        pass1,
                        lint,
                        r002,
                    ));
                }
                // Non-R002 re-parse path: destructure pass1 and move
                // `post_buffer` directly into `pass2_source` — no
                // document-buffer clone. `relint` is the fresh
                // post-pass-1 LintResult — move it into the `lint`
                // slot so the deadline-error payload, the pass-2
                // dispatch borrow, the partition below, and the
                // remaining-diagnostics filter all see post-pass-1
                // state.
                let Pass1Result {
                    post_buffer,
                    audit_lines,
                    applied_keys,
                } = pass1;
                (post_buffer, new_markings, audit_lines, applied_keys, relint)
            };

        // Re-partition the (now post-pass-1, when applicable) lint's
        // diagnostic stream. Pass-2 dispatches against this fresh
        // partition (FR-023 partial — Copilot round-1 #2). The pre-
        // pass-1 partition went out of scope when its enclosing block
        // ended above, so its borrow of the pre-rebind `lint` doesn't
        // outlive the rebind. `localized_ids` is `Engine::new`-time
        // immutable, so the partition predicate is unchanged. The
        // unused pass-1 slot here is discarded because pass-1 has
        // already run; pass-2 only needs its own phase partition.
        //
        // The partition itself is O(N) pointer pushes into two
        // reference vectors — no clones, no extra owned-diagnostic
        // bodies (Copilot round-3 R3-2 / Constitution I).
        if deadline_expired(self.deadline) {
            return Err(EngineError::DeadlineExceeded { partial_lint: lint });
        }

        // Pass-2: WholeMarking FixIntent fixes against post-pass-1
        // buffer. The partition + dispatch live in an inner scope so
        // `pass2_diags` (a `SmallVec<[&Diagnostic; 32]>` whose Drop
        // impl is not `#[may_dangle]`) drops before the later
        // `lint.diagnostics.into_iter()` move — `Vec<&T>`'s borrowck
        // relaxation doesn't carry over to `SmallVec`.
        let pass2 = {
            let (_pass1_diags_post, pass2_diags) =
                partition_diags_by_phase(&lint.diagnostics, &localized_ids);
            self.run_pass2_whole_marking(
                &pass2_source,
                &pass2_markings,
                &pass2_diags,
                &pass1_applied_keys,
                &lint,
            )?
        };

        // Merge audit-line streams (PR 3c.2.D / D7). Order matches the
        // existing audit-stream contract (D-7.6: "applied-fix records
        // emit in the order c001; pass1; pass2"). PM-D-8 generalizes
        // FR-016 to the marking-fix + text-correction sum-type stream.
        let mut all_audit_lines: Vec<AuditLine<CapcoScheme>> = Vec::with_capacity(
            pass0_audit_lines.len() + pass1_audit_lines.len() + pass2.audit_lines.len(),
        );
        all_audit_lines.extend(pass0_audit_lines);
        all_audit_lines.extend(pass1_audit_lines);
        all_audit_lines.extend(pass2.audit_lines);

        // Build the applied-keys set from EVERY audit line so the
        // remaining-diagnostics filter knows what survived each pass.
        let mut applied_keys: HashSet<(RuleId, Span)> =
            HashSet::with_capacity(all_audit_lines.len());
        for line in &all_audit_lines {
            match line {
                AuditLine::AppliedFix(fix) => {
                    applied_keys.insert((fix.rule.clone(), fix.span));
                }
                AuditLine::TextCorrection(tc) => {
                    applied_keys.insert((tc.rule.clone(), tc.span));
                }
                _ => {}
            }
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
            source: into_secret_slice(pass2.output),
            audit_lines: all_audit_lines,
            remaining_diagnostics,
            r002_fired: false,
        })
    }

    /// Pass-0 — text-correction promotion via the existing engine
    /// helper. **Behavior unchanged** from pre-7b (D-7.6: "C001 stays
    /// as pass-0"); this method exists to keep the pipeline shape
    /// visible at the `run()` call site.
    fn run_pass0_c001(&self, lint: &LintResult) -> Pass0Result {
        let (effective_source, dropped_diags, audit_lines) =
            self.engine
                .apply_text_corrections(self.source, lint, self.threshold, self.mode);
        Pass0Result {
            effective_source: Zeroizing::new(effective_source),
            audit_lines,
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
        parsed_markings: &[(Span, marque_capco::CapcoMarking)],
        pass1_diags: &[&Diagnostic<CapcoScheme>],
        lint: &LintResult,
    ) -> Result<Pass1Result, EngineError> {
        if pass1_diags.is_empty() {
            // No diagnostics → no fixes → caller short-circuits and
            // consumes its own pre-pass-1 buffer. Skip the document
            // clone; see the function's `post_buffer` invariant above.
            return Ok(Pass1Result {
                post_buffer: Zeroizing::new(Vec::new()),
                audit_lines: Vec::new(),
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
                post_buffer: Zeroizing::new(Vec::new()),
                audit_lines: Vec::new(),
                applied_keys: HashSet::new(),
            });
        }
        let (post_buffer, applied_keys, audit_lines) =
            self.apply_kept_fixes(effective_source, kept_fixes, lint)?;
        Ok(Pass1Result {
            post_buffer,
            audit_lines,
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
    ///
    /// PR 7c adds two reshape-aware adjustments before the fixes
    /// pass into [`synthesize_fixes`]:
    ///
    /// - **FR-023 disambiguation**: a pass-2 diagnostic whose
    ///   `(rule, span)` equals a pass-1 promoted fix is dropped. The
    ///   same rule has already fired on the same marking-scope span;
    ///   re-emitting it after the reshape would double-fire and
    ///   pollute `remaining_diagnostics`.
    /// - **I-18 overlap demotion**: a pass-2 diagnostic whose span
    ///   overlaps ANY pass-1 promoted fix span (any rule) at
    ///   `Severity::{Error, Warn, Fix}` is demoted to
    ///   `Severity::Suggest`. The pass-1 fix already shipped, so
    ///   pass-2 MUST NOT auto-apply on the same byte range
    ///   (Constitution V audit-record integrity); `Suggest` surfaces
    ///   the finding as advisory and is excluded from the audit
    ///   stream by `synthesize_fixes`' existing filter (FR-042 —
    ///   `Suggest` does not trigger `EX_DIAG_WARN`).
    ///
    /// Both adjustments operate on owned clones of the affected
    /// diagnostics so the input reference vector stays unmodified
    /// (pass-1 dispatch may still hold references into the same
    /// `LintResult.diagnostics` storage; cloning is the only sound
    /// way to alter severity without aliasing).
    fn run_pass2_whole_marking(
        &self,
        pass2_source: &[u8],
        parsed_markings: &[(Span, marque_capco::CapcoMarking)],
        pass2_diags: &[&Diagnostic<CapcoScheme>],
        pass1_applied_keys: &HashSet<(RuleId, Span)>,
        lint: &LintResult,
    ) -> Result<Pass2Result, EngineError> {
        if pass2_diags.is_empty() {
            return Ok(Pass2Result {
                output: Zeroizing::new(match self.mode {
                    FixMode::Apply => pass2_source.to_vec(),
                    FixMode::DryRun => self.source.to_vec(),
                }),
                audit_lines: Vec::new(),
                applied_keys: HashSet::new(),
            });
        }

        // PR 7c FR-023 disambiguation + I-18 overlap demotion. The
        // owned vector holds the post-adjustment diagnostics; a ref
        // vector keyed to its addresses feeds `synthesize_fixes`
        // (which signature still takes `&[&Diagnostic]`). The owned
        // vector lives for the duration of this function so the
        // refs are valid.
        let adjusted_owned = apply_fr023_and_i18(pass2_diags, pass1_applied_keys);
        let adjusted_refs: Vec<&Diagnostic<CapcoScheme>> = adjusted_owned.iter().collect();

        let synthesized = synthesize_fixes(
            &self.engine.scheme,
            parsed_markings,
            pass2_source,
            &adjusted_refs,
            self.threshold,
        );
        let kept_fixes = sort_and_c1_dedup(synthesized);
        let (post_buffer, applied_keys, audit_lines) =
            self.apply_kept_fixes(pass2_source, kept_fixes, lint)?;

        let output = match self.mode {
            FixMode::Apply => post_buffer,
            // DryRun returns the original source verbatim — pass-1's
            // post-buffer is discarded so callers cannot accidentally
            // consume partial bytes when they asked for dry-run.
            FixMode::DryRun => Zeroizing::new(self.source.to_vec()),
        };
        Ok(Pass2Result {
            output,
            audit_lines,
            applied_keys,
        })
    }

    /// Apply `kept_fixes` (already FR-016-sorted and C-1-deduped)
    /// against `source_buf`, producing the post-splice buffer and the
    /// promoted [`AppliedFix`] records. Shared between pass-1 and
    /// pass-2 because the splice semantics are identical at this
    /// layer.
    ///
    /// The post-splice buffer is built in **both** [`FixMode::Apply`]
    /// and [`FixMode::DryRun`] because pass-1's `post_buffer` is the
    /// input to pass-2's re-lint + dispatch (FR-022 / FR-023): pass-2
    /// MUST see the post-pass-1 coordinate space regardless of mode,
    /// or DryRun would silently dispatch pass-2 against the pre-pass-1
    /// buffer and produce a different applied set than Apply. The
    /// DryRun-vs-Apply distinction only affects [`FixResult.source`]
    /// at the outer layer — `run_pass2_whole_marking` substitutes
    /// `self.source.to_vec()` for DryRun there, so the user-visible
    /// `FixResult.source` remains the unmodified original.
    ///
    /// `dry_run` is plumbed into every [`AppliedFix`] record so the
    /// audit stream reflects mode regardless of which buffer the
    /// engine chose to keep as `FixResult.source`.
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
        // `marque-1.0` audit-line stream — the sole audit-output
        // channel post-cutover. The CLI / WASM renderers project
        // each line to its NDJSON record type.
        let mut audit_lines: Vec<AuditLine<CapcoScheme>> = Vec::with_capacity(kept_fixes.len());

        if deadline_expired(self.deadline) {
            return Err(EngineError::DeadlineExceeded {
                partial_lint: lint.clone(),
            });
        }

        // Build the post-splice buffer in both modes — pass-2 needs
        // the post-pass-1 coordinate space to dispatch correctly even
        // in DryRun. Wrap in `Zeroizing` so the scratch bytes wipe on
        // drop per Constitution Principle II.
        let post_buffer = Zeroizing::new(splice_fixes_forward(source_buf, &kept_fixes));

        // EngineConstructor mints the open-vocab Canonical<S> values
        // for v2 audit records. The sealed constructor name + the
        // sealed `CanonicalConstructor` supertrait keep this path
        // engine-only per `marque-scheme::canonical` doc comment.
        let constructor: EngineConstructor<CapcoScheme> =
            EngineConstructor::<CapcoScheme>::__engine_construct();

        for fix in kept_fixes {
            if deadline_expired(self.deadline) {
                return Err(EngineError::DeadlineExceeded {
                    partial_lint: lint.clone(),
                });
            }
            let key = (fix.rule.clone(), fix.span);
            applied_keys.insert(key);

            // PM-D-6 / G13: hash pre-fix bytes for the
            // `original_digest`. `original_bytes` borrows from
            // `source_buf` for the lifetime of the hashing call only.
            let original_bytes = &source_buf[fix.span.start..fix.span.end];

            // Build the v2 Canonical<S> via EngineConstructor (the
            // sealed open-vocab path). PM-D / D-A3: derive the
            // CategoryId from the originating ReplacementIntent so the
            // audit-record renderer can project the
            // `replacement.canonical.category` JSON field accurately
            // per `contracts/audit-record.md` §272:
            //
            // - `FactAdd { token, .. }`: route the token through
            //   `MarkingScheme::category_of`. The scheme resolves both
            //   closed-CVE [`FactRef::Cve`] and open-vocab
            //   [`FactRef::OpenVocab`] arms; if the routing table is
            //   missing the token (e.g., a future variant not yet
            //   wired) the engine falls back to the multi-category
            //   sentinel rather than panicking — the audit consumer
            //   sees `"Marking"` and can investigate, but the audit
            //   record stays well-formed.
            // - `FactRemove { facts, .. }`: route the first fact's
            //   token (all facts in a single intent share an axis per
            //   FR-???; the multi-fact form is a chained removal on
            //   one axis like E024's RD/FRD/TFNI cluster).
            // - `Recanonicalize { .. }`: spans multiple categories by
            //   construction — re-renders an entire `Scope::Page` or
            //   `Scope::Document`. Routes to
            //   [`CategoryId::MARKING`] (the reserved multi-category
            //   sentinel; projects to `"Marking"` in the JSON).
            let scheme_ref: &CapcoScheme = &self.engine.scheme;
            let category_id: CategoryId = match &fix.intent.replacement {
                marque_scheme::ReplacementIntent::FactAdd { token, .. } => {
                    scheme_ref.category_of(token).unwrap_or(CategoryId::MARKING)
                }
                marque_scheme::ReplacementIntent::FactRemove { facts, .. } => facts
                    .first()
                    .and_then(|fact| scheme_ref.category_of(fact))
                    .unwrap_or(CategoryId::MARKING),
                marque_scheme::ReplacementIntent::Recanonicalize { .. } => CategoryId::MARKING,
                // `#[non_exhaustive]` guard — a future variant
                // routes to the multi-category sentinel until the
                // scheme's `category_of` mapping is extended.
                _ => CategoryId::MARKING,
            };
            let canonical: Canonical<CapcoScheme> = constructor.build_open_vocab(
                category_id,
                Box::from(fix.replacement.as_ref()),
                fix.scope,
            );

            // v2 promote — AppliedFix (marque-1.0 shape). The
            // constructor hashes both `original_bytes` and
            // `canonical.bytes()` inline per PM-D-6.
            let v2_applied = marque_rules::audit::AppliedFix::<CapcoScheme>::__engine_promote(
                fix.rule,
                fix.severity,
                fix.span,
                fix.intent,
                original_bytes,
                canonical,
                now,
                classifier_id.clone(),
                dry_run,
                None,
                engine_promotion_token(),
            );
            audit_lines.push(AuditLine::AppliedFix(v2_applied));
        }
        Ok((post_buffer, applied_keys, audit_lines))
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

    /// Capture pre-pass-1 attribute snapshots for every marking
    /// whose span overlaps a pass-1 applied fix (PR 7c / FR-023 / R-4).
    /// Returns an empty cache when pass-1 promoted no fixes — pass-2
    /// has no reshape to disambiguate against and the
    /// `RuleContext.pre_pass_1_attrs` field is `None` everywhere on
    /// the post-pass-1 re-lint.
    ///
    /// Cache shape: at most one entry per pass-1-reshaped marking.
    /// `Phase::Localized` rules emit sub-token fix spans (PR 7b
    /// D-7.16 first-fire check), so a single fix always anchors to a
    /// single parent marking. Two fixes against the same marking
    /// dedupe to one cache entry. Inline-4 storage matches the
    /// existing Localized rule cap (C001 / E006 / E007 / S004).
    ///
    /// The `parsed_markings` slice is the pre-pass-1 cache from
    /// `lint_with_options_internal` (issue #432 swapped the storage
    /// from `HashMap<Span, _>` to a sorted `Vec<(Span, _)>` for
    /// cache-locality wins on high-candidate inputs): its
    /// `CapcoMarking.0` is the `CanonicalAttrs` snapshot the rule
    /// originally fired against. Cloning the attrs is unavoidable
    /// here because the cache outlives the `parsed_markings` slice
    /// (the engine moves the underlying `Vec` into the re-parse arm).
    fn populate_pre_pass_1_cache(
        &self,
        pass1_audit_lines: &[AuditLine<CapcoScheme>],
        parsed_markings: &[(Span, marque_capco::CapcoMarking)],
    ) -> PrePass1Cache {
        let mut cache: PrePass1Cache = SmallVec::new();
        if pass1_audit_lines.is_empty() {
            return cache;
        }
        // For each applied pass-1 fix, find the marking whose span
        // contains the fix span and dedupe into the cache. Sub-token
        // span containment is what `find_containing_marking` already
        // checks for the PR 7b first-fire path, so reuse it.
        //
        // Walks both `AppliedFix` and `TextCorrection` arms of
        // `AuditLine` — pass-1 today only produces marking fixes
        // (text-corrections run in pass-0), but the cache shape stays
        // sum-type-safe so a future Phase::Localized text-correction
        // would still get its marking captured.
        for line in pass1_audit_lines {
            let (rule, span) = match line {
                AuditLine::AppliedFix(fix) => (&fix.rule, fix.span),
                AuditLine::TextCorrection(tc) => (&tc.rule, tc.span),
                _ => continue,
            };
            let Some(marking_span) = find_containing_marking(parsed_markings, span) else {
                // A Localized rule whose fix span has no enclosing
                // marking should already have been dropped by the
                // PR 7b in_shape filter; if one slips through it
                // means a violated phase contract elsewhere. Log and
                // skip — never panic on the audit hot path.
                tracing::warn!(
                    rule_id = %rule,
                    fix_span_start = %span.start,
                    fix_span_end = %span.end,
                    "pass-1 applied fix has no enclosing parsed marking; \
                     skipping pre-pass-1 cache entry"
                );
                continue;
            };
            if cache.iter().any(|(s, _)| *s == marking_span) {
                continue;
            }
            let Some(marking) = lookup_marking(parsed_markings, marking_span) else {
                continue;
            };
            cache.push((marking_span, marking.0.clone()));
        }
        cache
    }

    /// Collect the unique contributing pass-1 rule IDs in
    /// FR-016-stable order (sort + dedup) for the R002 payload.
    /// Capped at 4 entries to fit the `SmallVec<[RuleId; 4]>` inline
    /// capacity exactly — pass-1 has at most 4 rule families today
    /// (C001/E006/E007/S004), and a future Localized rule expansion
    /// can lift the cap in lockstep with the inline-N bump.
    fn contributing_pass1_rule_ids(
        &self,
        pass1_audit_lines: &[AuditLine<CapcoScheme>],
    ) -> SmallVec<[RuleId; 4]> {
        let mut seen: HashSet<RuleId> = HashSet::new();
        let mut ids: Vec<RuleId> = Vec::new();
        for line in pass1_audit_lines {
            let rule = match line {
                AuditLine::AppliedFix(fix) => &fix.rule,
                AuditLine::TextCorrection(tc) => &tc.rule,
                _ => continue,
            };
            if seen.insert(rule.clone()) {
                ids.push(rule.clone());
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
        pass0_audit_lines: Vec<AuditLine<CapcoScheme>>,
        pass0_dropped_diags: Vec<Diagnostic<CapcoScheme>>,
        pass1: Pass1Result,
        lint: LintResult,
        r002: Diagnostic<CapcoScheme>,
    ) -> FixResult {
        // Audit-line merge (PR 3c.2.D / D7). R002 is a remaining-
        // diagnostic synthetic — it does NOT contribute an
        // `AuditLine::AppliedFix` entry; only promoted fixes do. The
        // R002 itself surfaces via `remaining_diagnostics` below.
        let mut all_audit_lines: Vec<AuditLine<CapcoScheme>> =
            Vec::with_capacity(pass0_audit_lines.len() + pass1.audit_lines.len());
        all_audit_lines.extend(pass0_audit_lines);
        all_audit_lines.extend(pass1.audit_lines);

        let mut applied_keys: HashSet<(RuleId, Span)> =
            HashSet::with_capacity(all_audit_lines.len());
        for line in &all_audit_lines {
            match line {
                AuditLine::AppliedFix(fix) => {
                    applied_keys.insert((fix.rule.clone(), fix.span));
                }
                AuditLine::TextCorrection(tc) => {
                    applied_keys.insert((tc.rule.clone(), tc.span));
                }
                _ => {}
            }
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
            FixMode::DryRun => Zeroizing::new(self.source.to_vec()),
        };

        FixResult {
            source: into_secret_slice(output),
            audit_lines: all_audit_lines,
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
///
/// # Hot-path allocation note (Copilot round-3 R3-2)
///
/// Returns reference vectors (`Vec<&Diagnostic<_>>`) rather than
/// cloning the diagnostics into owned vectors. Constitution I
/// (Uncompromising Performance) — this function runs up to twice
/// per fix and on the pre-Phase-D 10 KB fix path was previously
/// cloning the entire diagnostic stream on each call. Reference
/// propagation is the natural shape because every downstream
/// consumer (`run_pass1_localized`, `run_pass2_whole_marking`,
/// `synthesize_fixes`, `relint`-arm logging) only reads the
/// diagnostics; ownership is never required.
///
/// The returned references borrow from `diagnostics`, which the
/// caller (`TwoPassFixer::run`) keeps alive via the owning
/// `LintResult` for the duration of pass-1 + pass-2 dispatch.
/// Pass-1 diagnostic-reference partition. Inline-4 mirrors the
/// 4-rule `Phase::Localized` cap (C001, E006, E007, S004) — covers
/// "each Localized rule fires once for one marking" stack-only;
/// spills to heap only on pathological multi-fire (e.g., a
/// corrections map with ≥5 typo hits in one document).
type Pass1DiagRefs<'a> = SmallVec<[&'a Diagnostic<CapcoScheme>; 4]>;

/// Pass-2 diagnostic-reference partition. Inline-32 mirrors the
/// existing `Pass2Indices` precedent (5 entries of headroom from the
/// 27 `Phase::WholeMarking` rules; ample for typical 10 KB
/// documents). Refs are 8 bytes each → 256 bytes of inline storage +
/// Vec header. Spills to heap for documents with very many concurrent
/// WholeMarking diagnostics.
type Pass2DiagRefs<'a> = SmallVec<[&'a Diagnostic<CapcoScheme>; 32]>;

fn partition_diags_by_phase<'a>(
    diagnostics: &'a [Diagnostic<CapcoScheme>],
    localized_ids: &HashSet<&'static str>,
) -> (Pass1DiagRefs<'a>, Pass2DiagRefs<'a>) {
    let mut pass1_diags: Pass1DiagRefs<'a> = SmallVec::new();
    let mut pass2_diags: Pass2DiagRefs<'a> = SmallVec::new();
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
            pass1_diags.push(d);
        } else {
            pass2_diags.push(d);
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

/// True when two byte spans overlap (share at least one byte). Used
/// by PR 7c's I-18 overlap demotion to detect pass-2 diagnostics that
/// land on byte ranges already promoted by pass-1.
///
/// The half-open `[start, end)` convention matches the rest of
/// `marque-ism::Span`: spans `(0, 5)` and `(5, 10)` are adjacent but
/// do NOT overlap. Empty spans (`start == end`) never overlap
/// anything by construction.
#[inline]
fn spans_overlap(a: Span, b: Span) -> bool {
    a.start < b.end && b.start < a.end
}

/// PR 7c FR-023 + I-18 — apply reshape-aware disambiguation and
/// overlap demotion to a pass-2 diagnostic partition. Returns an
/// owned vector of post-adjustment diagnostics.
///
/// Adjustments:
///
/// - **FR-023 disambiguation**: a pass-2 diagnostic whose
///   `(rule, candidate_span ?? span)` matches a pass-1 promoted fix is
///   dropped. The same rule already fired on the same marking-scope
///   span; re-emitting it after the reshape would double-fire.
/// - **I-18 overlap demotion**: a pass-2 diagnostic whose
///   marking-scope span overlaps ANY pass-1 promoted span (any rule)
///   at `Severity::{Error, Warn, Fix}` is demoted to
///   `Severity::Suggest`. The pass-1 fix already shipped, so pass-2
///   MUST NOT auto-apply on the same byte range
///   (Constitution V audit-record integrity).
///
/// Short-circuit: when `pass1_applied_keys` is empty (the common
/// case: pass-1 produced no applied fixes), returns clones of every
/// input diagnostic without filtering. The clone cost is bounded by
/// `pass2_diags.len()`, which is typically ≤32 (the
/// `Pass2DiagRefs` inline cap). When pass-1 DID apply fixes, the
/// same clones happen — the reshape-aware path is the slow path by
/// construction.
///
/// Cloning the entire partition (rather than threading an index +
/// owned-only-on-demotion vector through Rust's borrow checker) is
/// the safe-code shape that satisfies Constitution `forbid(unsafe_code)`.
/// On the FR-023 / I-18 hot path the allocation is one `Vec` with
/// ≤32 `Diagnostic` clones — well below the SC-001 budget at 10 KB.
fn apply_fr023_and_i18(
    pass2_diags: &[&Diagnostic<CapcoScheme>],
    pass1_applied_keys: &HashSet<(RuleId, Span)>,
) -> Vec<Diagnostic<CapcoScheme>> {
    let mut out: Vec<Diagnostic<CapcoScheme>> = Vec::with_capacity(pass2_diags.len());
    for &d in pass2_diags {
        // FR-023: drop diagnostics with the same (rule, span) as a
        // pass-1 promoted fix. The candidate_span is the marking-
        // scope anchor — match against it (falling back to `span` for
        // diagnostics that don't carry a candidate span; matches the
        // `apply_kept_fixes` keying convention at engine.rs:2228+).
        let key_span = d.candidate_span.unwrap_or(d.span);
        if pass1_applied_keys.contains(&(d.rule.clone(), key_span)) {
            continue;
        }

        // I-18: demote diagnostics whose marking-scope span overlaps
        // any pass-1 promoted span at promote-eligible severity. The
        // overlap check uses `key_span` so a sub-token pass-2 finding
        // within a reshaped marking is also caught. The predicate
        // `Severity::is_promote_eligible` is the single source of
        // truth shared with `synthesize_fixes` (engine.rs:~2850) —
        // see its doc comment for why drift between the two sites
        // would re-open the I-18 leak channel.
        let needs_demote = d.severity.is_promote_eligible()
            && pass1_applied_keys
                .iter()
                .any(|(_, p1_span)| spans_overlap(key_span, *p1_span));

        let mut cloned = d.clone();
        if needs_demote {
            cloned.severity = Severity::Suggest;
        }
        out.push(cloned);
    }
    out
}

/// Find the marking span (a key in the sorted `parsed_markings`
/// slice) whose byte range contains `fix_span`. Linear scan over the
/// markings table — typical documents have <100 markings and this is
/// the defect path (a well-behaved Localized rule emits sub-token
/// spans by construction), so no binary-search optimization is
/// justified.
///
/// The slice is sorted by `Span.start` because the scanner emits
/// disjoint non-overlapping candidates in source order; this function
/// does not rely on that order for correctness, but a future
/// containment-scan optimization could (e.g., `partition_point`
/// against `start <= fix_span.start`).
fn find_containing_marking(
    parsed_markings: &[(Span, marque_capco::CapcoMarking)],
    fix_span: Span,
) -> Option<Span> {
    // Binary search: `parsed_markings` is sorted by `Span.start` ascending
    // (Scanner::scan order, enforced by the push-site debug_assert).
    // `partition_point` gives the first index where start > fix_span.start;
    // the candidate containing marking (if any) is therefore at index − 1.
    // O(log N) rather than the prior O(N) linear scan.
    let idx = parsed_markings.partition_point(|(s, _)| s.start <= fix_span.start);
    if idx == 0 {
        return None;
    }
    let (marking_span, _) = &parsed_markings[idx - 1];
    if span_is_within_marking(fix_span, *marking_span) {
        Some(*marking_span)
    } else {
        None
    }
}

/// Point lookup for the recognized marking at exactly `span`.
///
/// `parsed_markings` is sorted by `Span.start` because cache insertion
/// happens AFTER the engine's early-`continue` filters PageBreak
/// candidates out of the candidate stream. `Scanner::scan` sorts the
/// raw stream by `(span.start, kind_sort_priority)` and can emit
/// co-located candidates at the scanner boundary (PageBreak +
/// content), but the engine's PageBreak `continue` happens above the
/// cache push site, so the post-filter slice held by `parsed_markings`
/// has strictly increasing starts. The push site enforces this via
/// `debug_assert!`.
///
/// `binary_search_by_key` on `Span.start` therefore finds the unique
/// entry (if any). The post-search equality check additionally
/// validates `Span.end` — preserving the prior `HashMap`'s
/// full-`Span`-equality lookup semantics exactly, and degrading
/// gracefully to `None` in the (currently impossible by construction)
/// degenerate case where two cache entries share a start.
fn lookup_marking(
    parsed_markings: &[(Span, marque_capco::CapcoMarking)],
    span: Span,
) -> Option<&marque_capco::CapcoMarking> {
    let idx = parsed_markings
        .binary_search_by_key(&span.start, |(s, _)| s.start)
        .ok()?;
    // `binary_search_by_key` may land on ANY entry in a
    // matching-start run, so the equality check on the landed entry
    // alone could miss the target if a future scanner regression
    // introduces duplicate starts. Walk the matching-start run
    // (backward to the first matching-start entry, then forward to
    // the last) and full-`Span`-equality-check each entry. By
    // construction the cache slice has strictly-increasing starts
    // (PageBreak filtered, debug_assert on push), so the walk
    // collapses to a single iteration on the fast path — zero
    // measurable cost relative to the prior `HashMap`'s single
    // bucket probe.
    let target_start = span.start;
    let mut i = idx;
    while i > 0 && parsed_markings[i - 1].0.start == target_start {
        i -= 1;
    }
    while i < parsed_markings.len() && parsed_markings[i].0.start == target_start {
        if parsed_markings[i].0 == span {
            return Some(&parsed_markings[i].1);
        }
        i += 1;
    }
    None
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
/// via [`TwoPassFixer::apply_kept_fixes`]. `fixes` MUST be FR-016
/// sorted (span.end DESC, span.start DESC) so `iter().rev()` yields
/// ascending order for the left-to-right walk. Pre-allocates
/// capacity using the per-fix growth contribution (`saturating_sub`
/// upper bound).
///
/// The earlier name `apply_pass1_fixes` predated the PR 7b
/// phase-split orchestrator and implied pass-1 exclusivity; the
/// renamed `splice_fixes_forward` names what the function actually
/// does — a forward splice — so a reader scanning either pass's
/// caller can see the operation without re-reading the body.
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
fn splice_fixes_forward(source: &[u8], fixes: &[SynthesizedFix]) -> Vec<u8> {
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
            "overlapping fix in splice_fixes_forward: cursor={cursor}, span={:?}",
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
    parsed_markings: &[(Span, marque_capco::CapcoMarking)],
    source: &[u8],
    diagnostics: &[&marque_rules::Diagnostic<CapcoScheme>],
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
    for &d in diagnostics {
        let Some(intent) = d.fix.as_ref() else {
            continue;
        };
        // Pass-2 promotion gate. Uses the single-source-of-truth
        // `Severity::is_promote_eligible` so this site and the I-18
        // overlap-demotion guard in `apply_fr023_and_i18` stay aligned
        // by construction — any future severity-classification change
        // updates both sites at once.
        if !d.severity.is_promote_eligible() {
            continue;
        }
        let cspan = d.candidate_span.unwrap_or(d.span);
        if cspan.is_empty() {
            continue;
        }

        if intent.confidence.combined() < threshold {
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
        let Some(marking) = lookup_marking(parsed_markings, cspan) else {
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
        let is_portion = trimmed.first() == Some(&b'(') && trimmed.last() == Some(&b')');
        let core: String = if is_portion {
            format!("({})", scheme.render_portion(&modified))
        } else {
            scheme.render_banner(&modified)
        };
        let scope = if is_portion {
            Scope::Portion
        } else {
            Scope::Page
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
            .filter_map(|d| d.fix.as_ref().map(|i| i.confidence.combined()))
            .fold(f32::INFINITY, f32::min);
        let mut combined_intent = owning_intent.clone();
        // Within-group audit-collapse scaling. The owning diagnostic's
        // `rule` axis is scaled down so `combined()` equals the
        // minimum across the group.
        let combined_intent_combined = combined_intent.confidence.combined();
        if min_combined < combined_intent_combined && combined_intent.confidence.rule > 0.0 {
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
            severity: owning_diag.severity,
            span: cspan,
            replacement: replacement.into_boxed_str(),
            scope,
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
        marque_rules::Message::new(
            marque_rules::MessageTemplate::DecoderRecognized,
            marque_rules::MessageArgs {
                span: Some(span),
                ..marque_rules::MessageArgs::default()
            },
        ),
        DECODER_CITATION_TYPED,
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
/// presence. The R002 `Diagnostic` carries the contributing rule
/// IDs as a typed `SmallVec<[RuleId; 4]>` field on `MessageArgs`
/// — PR 3c.2.C completed the `Diagnostic.message: Box<str>` →
/// `Message` migration, so this function constructs
/// `Message::new(MessageTemplate::ReparseFailed,
/// MessageArgs { contributing_rule_ids, .. })` directly. The
/// `contributing_rule_ids` parameter is moved into the args struct
/// (no clone) — `RuleId` is on Constitution V's permitted-identifier
/// list, not document bytes.
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
    // PR 3c.2.C C5: typed `Message` per `MessageTemplate::ReparseFailed`.
    // `MessageArgs.contributing_rule_ids` carries the closed-list of
    // pass-1 RuleIds that contributed to the failure — `RuleId` is on
    // Constitution V's permitted-identifier list (enumerated identifier,
    // not document bytes). The contributing list flows into the audit
    // record as a structured field instead of an interpolated string.
    // The SmallVec is moved into `MessageArgs` (no clone — the function
    // owns the argument and doesn't use it afterward).
    let message = marque_rules::Message::new(
        marque_rules::MessageTemplate::ReparseFailed,
        marque_rules::MessageArgs {
            contributing_rule_ids,
            ..marque_rules::MessageArgs::default()
        },
    );

    Diagnostic::new(
        R002_RULE_ID,
        Severity::Error,
        failure_span,
        message,
        R002_CITATION_TYPED,
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
/// PageFinalization rule-index partition (issue #461). Inline-4 —
/// PageFinalization is dispatched once per page (not per candidate),
/// so the registered-rule count drives this, not call frequency. W004
/// (issue #461) and S005 (issue #488) are today's consumers (two
/// rules). Two scheduled follow-up PRs (S007, BannerMatchesProjectedRule)
/// will add at most a handful more; if the count grows beyond inline
/// capacity the SmallVec spills to the heap (one allocation at engine
/// construction time — not on the hot path). 4 is a deliberate
/// small-inline budget: this
/// partition is consulted once per page-break + once at EOD, which
/// is O(pages) per document, not O(candidates).
type PassFinalizationIndices = SmallVec<[(usize, usize); 4]>;

/// Pre-resolved registered-ID severity table consumed by Site A's
/// fast-path Off-skip. Outer-indexed by rule-set, inner by rule-index-
/// within-set — same shape as [`Pass1Indices`] / [`Pass2Indices`].
/// See [`Engine::fast_path_severities`] for the full invariant.
type FastPathSeverities = Box<[Box<[Severity]>]>;

/// Pre-resolved per-emitted-ID severity overrides. Keyed by `&'static
/// str` because [`RuleId::as_str()`] returns `&'static str`, so the
/// lookup `map.get(d.rule.as_str())` works without an owned
/// allocation. See [`Engine::emitted_id_overrides`] for the full
/// invariant.
type EmittedIdOverrides = HashMap<&'static str, Severity>;

/// Partition the registered rules by their declared [`Phase`] (FR-021).
///
/// Returns `(pass1, pass2, pass_finalization)` where each entry is a
/// `(rule_set_index, rule_index_within_set)` pair indexing back into
/// the caller's `rule_sets[i].rules()[j]`:
///
/// - `pass1` enumerates every [`Phase::Localized`] rule (pass-1
///   forward-splice in `TwoPassFixer`).
/// - `pass2` enumerates every [`Phase::WholeMarking`] rule (pass-2
///   apply_intent in `TwoPassFixer`).
/// - `pass_finalization` enumerates every [`Phase::PageFinalization`]
///   rule (issue #461). Dispatched by the engine's synthetic-candidate
///   path at each scanner-emitted `MarkingType::PageBreak` (BEFORE the
///   PageContext reset) and once at end-of-document — see
///   `dispatch_page_finalization`.
///
/// Together they cover every registered rule exactly once. `Phase` is
/// `#[non_exhaustive]` per issue #461; the wildcard arm below is a
/// loud failure for a future variant rather than a silent bucket.
///
/// Walked once at [`Engine::with_clock`] time and cached on the engine.
/// Per-document `fix` dispatch reads `pass1_rule_indices` via
/// [`TwoPassFixer::localized_rule_id_set`] (PR 7b); the walk does not
/// re-run. `pass2_rule_indices` is stored against a deferred future
/// migration that would switch pass-2 from the current
/// complement-of-pass-1 dispatch to a positive whitelist (read off
/// that field) for symmetry with pass-1. PR 7c retained the
/// complement dispatch (implementer Decision #4); see
/// [`Engine::pass2_rule_indices`] for the rationale and the
/// deferred-migration framing. `pass_finalization_rule_indices` is
/// the issue #461 third bucket — read directly by
/// `dispatch_page_finalization` at lint time and (when a future
/// PageFinalization rule emits a fix) by the corresponding pass-2
/// path at fix time. Today's consumers (W004 from issue #461; S005
/// from issue #488) emit no fix.
fn partition_rules_by_phase(
    rule_sets: &[Box<dyn RuleSet<CapcoScheme>>],
) -> (Pass1Indices, Pass2Indices, PassFinalizationIndices) {
    let mut pass1: Pass1Indices = SmallVec::new();
    let mut pass2: Pass2Indices = SmallVec::new();
    let mut pass_finalization: PassFinalizationIndices = SmallVec::new();
    for (set_idx, rule_set) in rule_sets.iter().enumerate() {
        for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
            match rule.phase() {
                Phase::Localized => pass1.push((set_idx, rule_idx)),
                Phase::WholeMarking => pass2.push((set_idx, rule_idx)),
                Phase::PageFinalization => pass_finalization.push((set_idx, rule_idx)),
                // `Phase` is `#[non_exhaustive]` (issue #461). A
                // future variant should fail loudly at engine
                // construction time so the dispatch path stays
                // explicit — never silently bucket a new phase
                // into an existing pass.
                _ => panic!(
                    "partition_rules_by_phase: unknown Phase variant for rule {:?}; \
                     `Phase` is #[non_exhaustive] and a new variant requires explicit \
                     engine plumbing before it can be registered",
                    rule.id().as_str()
                ),
            }
        }
    }
    (pass1, pass2, pass_finalization)
}

/// Dispatch every registered [`Phase::PageFinalization`] rule against
/// the page-level fixpoint snapshot at a page-boundary anchor offset.
///
/// Issue #461. Called by the engine's lint loop at every
/// scanner-emitted [`marque_ism::MarkingType::PageBreak`] (BEFORE the
/// per-page accumulator reset, so the dispatched rules see the
/// closing page's final state) and once at end-of-document (so
/// trailing portions that never reached a page-break boundary still
/// observe the fixpoint).
///
/// This is a free function rather than an `&self` method on `Engine`
/// because the inputs are decomposed and the helper has no need for
/// other engine state. Threading the decomposition explicitly keeps
/// the contract visible at the call site — every input the dispatch
/// depends on is named in the parameter list, and a future refactor
/// can lift it into a different orchestration shape (an iterator
/// transformation, a streaming dispatch) without spelunking through
/// `Engine`'s field list.
///
/// # Invariants
///
/// - `page_portions` must be non-empty at call time (the caller
///   guards on `!page_portions.is_empty()`). An empty-page dispatch
///   produces no useful work and `CapcoScheme::project_from_attrs_slice`
///   would emit a noisy default. The skip is in the caller so the
///   cost of the `is_empty()` probe is paid at the boundary, not
///   per rule.
/// - `page_portions_arc` / `page_marking_arc` are mutable `Option`
///   references because the dispatch path force-initializes both
///   Arcs (PageFinalization rules expect `Some(_)` for both). The
///   caller threads the same Arcs through to a possible subsequent
///   banner/CAB candidate on the same page — except for the
///   end-of-document call, where the document ends without further
///   candidates.
/// - The synthetic boundary candidate carries a zero-length `Span`
///   at the boundary offset. Today this is the only span a
///   PageFinalization rule can emit on its `Diagnostic`: the
///   per-page accumulator stores `[CanonicalAttrs]` without
///   per-portion spans, so `ctx.page_portions` cannot recover an
///   offending portion's own offsets. Rules document this
///   limitation in their doc comments (W004 from issue #461 and
///   S005 from issue #488 are the worked examples). A future
///   enhancement that threads per-portion spans through the
///   accumulator — or a span-lookup helper into `RuleContext` —
///   would let rules refine the anchor to the specific offending
///   portion.
/// - `candidates_processed` is NOT incremented by this dispatch.
///   That counter tracks scanner-emitted candidates; the synthetic
///   PageFinalization candidate is engine-internal.
///
/// # Returns
///
/// `Ok(())` on a complete dispatch pass. `Err(())` on per-dispatch
/// deadline expiry — the caller propagates the truncated `LintResult`
/// shape. The deadline is checked once at the top of the dispatch
/// (the per-page work is small relative to the per-candidate rule
/// loop) so an already-expired deadline returns immediately without
/// invoking any rule.
#[allow(clippy::too_many_arguments)]
fn dispatch_page_finalization(
    scheme: &CapcoScheme,
    rule_sets: &[Box<dyn RuleSet<CapcoScheme>>],
    pass_finalization_rule_indices: &PassFinalizationIndices,
    fast_path_severities: &FastPathSeverities,
    emitted_id_overrides: &EmittedIdOverrides,
    page_portions: &[marque_ism::CanonicalAttrs],
    page_portions_arc: &mut Option<Arc<Box<[marque_ism::CanonicalAttrs]>>>,
    page_marking_arc: &mut Option<Arc<marque_ism::ProjectedMarking>>,
    page_join_acc: &marque_ism::CanonicalAttrs,
    corrections_arc: &Option<Arc<HashMap<String, String>>>,
    boundary_offset: usize,
    deadline: Option<Instant>,
    out_diagnostics: &mut Vec<Diagnostic<CapcoScheme>>,
) -> Result<(), ()> {
    use marque_ism::MarkingType;
    use marque_rules::RuleContext;

    // Deadline guard once at the dispatch boundary. Per-rule
    // deadline checks would amortize the wall-clock probe across a
    // very short rule list (currently 1 rule); the boundary-level
    // check is cheap and keeps the failure mode aligned with the
    // main candidate loop's pre-iteration check.
    if deadline_expired(deadline) {
        return Err(());
    }

    // Empty-bucket short-circuit (Copilot review on PR #487 / issue
    // #461). If no rule declared `Phase::PageFinalization` the Arc
    // force-init below would still clone the accumulator and project
    // `page_marking` — both non-trivial — without a consumer. This
    // matters for future schemes that may register no PageFinalization
    // rules, and for any future config layer that disables every
    // PageFinalization rule via severity override `Off` (the per-rule
    // Off-skip below would no-op, but the clone has already happened).
    // Returning early keeps the cost proportional to actual consumer
    // count.
    if pass_finalization_rule_indices.is_empty() {
        return Ok(());
    }

    // All-Off short-circuit (Copilot round-2 on PR #487). If every
    // PageFinalization rule's registered-id severity resolves to
    // `Off`, the per-rule loop below would skip them all — but only
    // after the Arc force-init paid the snapshot clone +
    // `CapcoScheme::project_from_attrs_slice`. Pre-scanning the
    // bucket lets us return BEFORE those costs.
    //
    // Walker rules (those with `additional_emitted_ids()` non-empty)
    // can still fire under per-emitted-id severity overrides even
    // when their registered-id severity is `Off`, so they MUST NOT
    // be treated as Off by this gate. No PageFinalization rule
    // today registers walker IDs; the gate is shaped to stay
    // correct if one is added.
    let any_rule_can_fire = pass_finalization_rule_indices.iter().any(|&(s, r)| {
        let rule = &rule_sets[s].rules()[r];
        !rule.additional_emitted_ids().is_empty() || fast_path_severities[s][r] != Severity::Off
    });
    if !any_rule_can_fire {
        return Ok(());
    }

    // PageFinalization rules contract: ctx.page_portions AND
    // ctx.page_marking are both populated. Force-init both Arcs
    // here BEFORE building the RuleContext so the rule body can
    // unconditionally read them. Subsequent same-page banner/CAB
    // candidates reuse these Arcs through the normal lazy path.
    let page_portions_arc = page_portions_arc
        .get_or_insert_with(|| Arc::new(page_portions.to_vec().into_boxed_slice()))
        .clone();
    let page_mark_arc = page_marking_arc
        .get_or_insert_with(|| Arc::new(project_page_marking(scheme, page_join_acc)))
        .clone();

    // Zero-length span at the boundary anchor. Rules use this as
    // the candidate-span anchor; if the rule wants a user-facing
    // span on a specific portion, it walks `ctx.page_portions` and
    // refers to that portion's span (when tracked). The per-page
    // accumulator does not store per-portion spans today; rules
    // that need sub-page precision fall back to this anchor and
    // document the limitation.
    let boundary_span = Span::new(boundary_offset, boundary_offset);

    // PageFinalization rules don't read `attrs`; they read
    // `ctx.page_portions` / `ctx.page_marking`. The dummy attrs are
    // a `Default::default()` to satisfy the `Rule::check`
    // signature. We pass `&dummy` so the borrow doesn't outlive the
    // dispatch loop; rules that try to introspect dummy attrs will
    // observe `Default` values (e.g., empty `Box<[T]>` collections)
    // — they would be misimplemented PageFinalization rules anyway.
    let dummy_attrs = marque_ism::CanonicalAttrs::default();

    // `pre_pass_1_attrs` is `None` because the synthetic boundary
    // span has no preceding-portion identity in the pre-pass-1
    // attrs cache (the cache is keyed by content-candidate spans;
    // a boundary span at offset N never equals one).
    // PR #490: clone `page_portions_arc` for the `RuleContext` so the
    // original handle stays in scope through the dispatch loop and
    // remains available to the portion-snapshot sentinel below
    // (which observes the slice the rule actually reads via
    // `ctx.page_portions`). `Arc::clone` is a refcount bump, no
    // slice data is copied.
    let ctx = RuleContext::new(MarkingType::PageFinalization, boundary_span)
        .with_page_portions(Some(page_portions_arc.clone()))
        .with_page_marking(Some(page_mark_arc))
        .with_corrections(corrections_arc.clone())
        .with_pre_pass_1_attrs(None);

    // PR #490: portion-snapshot sentinel for the PageRewrite
    // read-only-attrs invariant. `Phase::PageFinalization` rules
    // read `ctx.page_portions` and re-project per-portion lattices
    // from that slice (e.g., W004's `JointSet::from_attrs_iter` per
    // §H.3 p57 derivative-use migration trigger). A rule that
    // mutated portions through any future API change — or a future
    // closure-operator rewrite-application site that did so — would
    // silently break that predicate's input invariance. Today
    // `ctx.page_portions` exposes `&[CanonicalAttrs]` via
    // `Box<[_]>` with no `&mut` API, so a conformant rule cannot
    // violate the contract through the public API; the sentinel is
    // a static guard against future API changes that would open a
    // mutation path. See
    // `docs/plans/2026-05-01-lattice-design.md` section 3 (e.1).
    //
    // **Sibling sentinel (PR 4b-D.2).** The closure-operator's
    // rewrite-application site now lives in `CapcoScheme::project`
    // (`crates/capco/src/scheme/marking_scheme_impl.rs`), where it
    // sits between the `join_via_lattice` composition and the
    // declarative PageRewrite catalog. That site carries its own
    // `#[cfg(debug_assertions)]` snapshot-and-compare against the raw
    // per-portion CanonicalAttrs slice it observes, asserting the
    // closure's read-only-attrs invariant per
    // `docs/plans/2026-05-01-lattice-design.md` §3 (e.1). The two
    // sentinels cover different invocation contexts: this one fires
    // around `Phase::PageFinalization` rule dispatch (where rules
    // read `ctx.page_portions`); the scheme-side sentinel fires
    // inside the per-projection pipeline that produces
    // `ctx.page_marking`. Together they pin the read-only contract
    // across both engine-facing surfaces.
    //
    // Snapshot the slice the rule actually observes (via
    // `page_portions_arc`). The sentinel's purpose is to catch
    // FUTURE API loosenings — e.g., a `portions_mut()` addition
    // on a future newtype wrapper, or an `Arc::get_mut` bypass via
    // a future debug API. Cost: a clone of `[CanonicalAttrs]` in
    // debug builds only; `--release` strips the snapshot and the
    // assertion entirely. Placement is AFTER the empty-bucket /
    // all-Off short-circuits (they early-return before reaching
    // this point).
    //
    // Note on the type chain: `page_portions_arc` at this point is
    // the locally-rebound `Arc<Box<[CanonicalAttrs]>>` (NOT the outer
    // `Option<Arc<Box<[CanonicalAttrs]>>>` parameter — see the
    // `.get_or_insert_with(...).clone()` rebinding earlier in this
    // function). `Arc::as_ref()` yields `&Box<[CanonicalAttrs]>`
    // which auto-derefs to `&[CanonicalAttrs]`; `<[T]>::to_vec()`
    // then produces `Vec<CanonicalAttrs>` directly.
    #[cfg(debug_assertions)]
    let portions_before: Vec<marque_ism::CanonicalAttrs> = page_portions_arc.as_ref().to_vec();

    // Mirror the main candidate-loop dispatch shape: fast-path
    // Off-skip via `fast_path_severities[set_idx][rule_idx]`,
    // `catch_unwind` for untrusted rules, per-diagnostic
    // emitted-id override via `emitted_id_overrides`. The walker
    // path (`additional_emitted_ids().is_empty()` is false) gates
    // on the per-diagnostic override loop below; no
    // PageFinalization rule today registers walker IDs, but the
    // shape is preserved for forward compatibility.
    for &(set_idx, rule_idx) in pass_finalization_rule_indices.iter() {
        let rule = &rule_sets[set_idx].rules()[rule_idx];

        if rule.additional_emitted_ids().is_empty() {
            let configured_severity = fast_path_severities[set_idx][rule_idx];
            if configured_severity == Severity::Off {
                continue;
            }
        }

        let rule_id = rule.id();
        let mut diags = if rule.trusted() {
            rule.check(&dummy_attrs, &ctx)
        } else {
            match std::panic::catch_unwind(AssertUnwindSafe(|| rule.check(&dummy_attrs, &ctx))) {
                Ok(d) => d,
                Err(payload) => {
                    let msg = panic_payload_to_string(&payload);
                    tracing::warn!(
                        target: "marque_engine::rule_panic",
                        rule = rule_id.as_str(),
                        error = %msg,
                        "PageFinalization rule check panicked; skipping this rule for the current page boundary"
                    );
                    Vec::new()
                }
            }
        };

        // Per-emitted-id override (Site B equivalent for the
        // synthetic dispatch). Mirrors the main loop's
        // `diags.retain_mut` exactly so a `[rules] W004 = "off"`
        // config silences the rule the same way it would in the
        // main candidate loop.
        diags.retain_mut(
            |d| match emitted_id_overrides.get(d.rule.as_str()).copied() {
                Some(Severity::Off) => false,
                Some(override_severity) => {
                    d.severity = override_severity;
                    true
                }
                None => true,
            },
        );
        out_diagnostics.extend(diags);
    }

    // PR #490: portion-snapshot assertion — see snapshot comment
    // above. The PageRewrite read-only-attrs invariant requires
    // that no `Phase::PageFinalization` rule mutate the per-portion
    // `CanonicalAttrs` slice (observed via `page_ctx_arc`, matching
    // the slice the rule itself reads).
    //
    // The comparison + error-message construction lives in
    // [`check_portions_unchanged`] below so it can be unit-tested
    // directly (Codecov patch-coverage gate on PR #498). On
    // mismatch the helper returns `Err(msg)` and the call site
    // panics with that message — the panic body is the
    // structurally-uncoverable hot-path branch.
    //
    // Why this avoids `debug_assert_eq!`: `assert_eq!` /
    // `debug_assert_eq!` call `core::panicking::assert_failed`,
    // which formats both operands via `Debug`
    // (`left: {left:?} right: {right:?}`) regardless of any custom
    // message. That would dump both `&[CanonicalAttrs]` slices —
    // token IDs, span offsets, country lists, AEA blocks — into the
    // panic output, violating G13 (Constitution V Principle V:
    // audit-content-ignorance). The helper-returns-`String` shape
    // formats only counts + indices; debug builds may still run in
    // classified-content environments.
    //
    // The outer-loop placement cannot attribute the violation to a
    // specific rule; if a sentinel firing requires per-rule
    // attribution, switch to a per-iteration snapshot inside the
    // loop temporarily for debugging.
    #[cfg(debug_assertions)]
    if let Err(msg) = check_portions_unchanged(
        portions_before.as_slice(),
        page_portions_arc.as_ref(),
        pass_finalization_rule_indices.len(),
    ) {
        panic!("{msg}");
    }

    Ok(())
}

/// Project the current per-page accumulator slice into a
/// [`marque_ism::ProjectedMarking`] via the scheme's production
/// page-projection path.
///
/// PR 4b-D.2 flipped the hot path from `PageContext::project()` (the
/// transitional PageContext-driven projection) to
/// `scheme.project(Scope::Page, ...)` (the lattice + closure +
/// PageRewrite pipeline). The bridge from `CanonicalAttrs` to
/// `ProjectedMarking` lives in `marque_ism::ProjectedMarking::from_canonical`
/// so the scheme crate and the engine crate share one source of truth.
///
/// Authorization: this helper centralizes the projection-call shape
/// shared by the primary lazy-init in `Engine::lint` (around the
/// banner/CAB candidate dispatch) and the secondary
/// `dispatch_page_finalization` initialization. Both sites need the
/// scheme handle to drive the lattice path; passing `scheme` and
/// the accumulator slice here keeps the closure capture minimal at
/// each call site and avoids duplicating the per-portion conversion
/// logic.
///
/// PR 4b-D.2 Copilot R1 #5: this helper lives BELOW
/// `dispatch_page_finalization` so its doc-comment doesn't run into
/// the dispatch function's `# Returns` block. The placement is purely
/// for doc-attribution clarity.
///
/// PR 6c (T069) flattened the parameter from `&PageContext` to
/// `&[CanonicalAttrs]` so the caller no longer needs to construct
/// the intermediate accumulator type.
///
/// Authority: `docs/plans/2026-05-01-lattice-design.md` §4.7.4
/// pipeline ordering.
fn project_page_marking(
    scheme: &CapcoScheme,
    page_join_acc: &marque_ism::CanonicalAttrs,
) -> marque_ism::ProjectedMarking {
    // PR 4b-D.2 Commit 7 perf optimization: route through
    // `CapcoScheme::project_from_attrs_slice`, the engine fast-path
    // that consumes the per-page accumulator slice directly.
    //
    // Issue #306 (O(N²) fix): the caller now passes the pre-computed
    // incremental join accumulator (`page_join_acc`) rather than the
    // full `page_portions` slice. The accumulator is maintained by the
    // portion-push site via `join_via_lattice([acc, new_portion])` so
    // this call projects a single element (O(1)) instead of re-folding
    // all N portions on every banner/CAB candidate (O(N)).
    let projected = scheme.project_from_attrs_slice(std::slice::from_ref(page_join_acc));
    marque_ism::ProjectedMarking::from_canonical(projected)
}

/// Compare two `CanonicalAttrs` slices for the PageFinalization
/// read-only-attrs sentinel. Returns `Ok(())` on equality,
/// `Err(msg)` with a G13-compliant diagnostic message on mismatch
/// (counts + indices only — never portion content).
///
/// Debug-only — only callers inside a `#[cfg(debug_assertions)]`
/// block invoke this. `pub(crate)` for unit-testability: the
/// helper is the detection primitive for the invariant described
/// in `docs/plans/2026-05-01-lattice-design.md` section 3 (e.1).
/// Extracted from the inline `debug_assert!` body in
/// [`dispatch_page_finalization`] so the comparison +
/// error-message-construction paths land in Codecov patch coverage
/// (PR #498 / issue #490).
///
/// # G13 (Constitution V Principle V) compliance
///
/// The returned error message contains **only**:
///
/// - The literal string `"PageFinalization rule dispatch ..."`
/// - The before-slice length (a `usize` count, not content)
/// - The after-slice length (a `usize` count, not content)
/// - The `rule_count` parameter (a `usize` count, not content)
/// - The doc-cross-reference literal
///
/// It MUST NOT contain any `CanonicalAttrs` field values, type
/// names that imply field content (e.g., `"SciControl"`,
/// `"Span"`), or any string formed from slice element content.
/// `sentinel_tests::check_portions_unchanged_error_message_is_g13_compliant`
/// pins this invariant with a synthetic distinctive-content
/// fixture — modifying the format string MUST be done together
/// with re-running that test.
#[cfg(debug_assertions)]
pub(crate) fn check_portions_unchanged(
    before: &[marque_ism::CanonicalAttrs],
    after: &[marque_ism::CanonicalAttrs],
    rule_count: usize,
) -> Result<(), String> {
    if before == after {
        Ok(())
    } else {
        Err(format!(
            "PageFinalization rule dispatch mutated the per-page portion slice \
             ({} portion(s) before vs {} after, {} rule(s) dispatched). \
             This violates the PageRewrite read-only-attrs invariant in \
             docs/plans/2026-05-01-lattice-design.md section 3 (e.1). \
             The portion-snapshot sentinel cannot pin the violating rule \
             from this outer-loop placement; to attribute, switch to \
             a per-iteration snapshot inside the loop temporarily.",
            before.len(),
            after.len(),
            rule_count,
        ))
    }
}

/// Pre-resolve all rule severity overrides into two indexed lookup
/// tables consumed by the lint hot loop. Built once at
/// [`Engine::with_clock`] time, after `canonicalize_rule_overrides`
/// has reduced the override map to canonical-ID keys.
///
/// Returns `(fast_path_severities, emitted_id_overrides)`:
///
/// - `fast_path_severities`: outer-indexed by rule-set, inner-indexed
///   by rule-index-within-set. Each entry is the resolved [`Severity`]
///   for that rule's *registered* ID — `overrides.get(id).and_then
///   (parse_config).unwrap_or(rule.default_severity())`. Indices match
///   [`Engine::pass1_rule_indices`] / [`Engine::pass2_rule_indices`].
///   Site A (the fast-path Off-skip in `lint_inner`) reads from this
///   table by `[set_idx][rule_idx]`.
///
/// - `emitted_id_overrides`: keyed by `&'static str` rule ID (the slice
///   carried by [`RuleId`]); value is the user-configured [`Severity`]
///   only when one is set AND parses cleanly. Absence preserves the
///   diagnostic's emitted severity (`None` arm in the lookup). Sites B
///   (per-diagnostic `retain_mut`), C (bridge `ConstraintViolation`),
///   and D (C001 post-pass) read from this map.
///
/// The pre-hoist code path performed both lookups + a
/// `Severity::parse_config` parse on every candidate × rule (Site A)
/// and on every emitted diagnostic (Site B), both inside the hot loop;
/// this hoist replaces them with one indexed slice load and one
/// `HashMap::get(&'static str)` lookup respectively. Lookup keys are
/// `&'static str` — `RuleId::as_str()` returns `&'static str` — so
/// `HashMap<&'static str, Severity>::get(rule_id.as_str())` works
/// directly without an owned allocation.
///
/// **`bridge_rule_ids` parameter.** The caller MUST pass the same
/// bridge IDs slice it handed to `canonicalize_rule_overrides`
/// (e.g., `scheme.bridge_emitted_rule_ids()` where `scheme` is the
/// stored `CapcoScheme` instance). Threading the slice through
/// instead of constructing a second `CapcoScheme::new()` inside this
/// helper closes a divergence channel both reviewers' HIGH flagged:
/// if `bridge_emitted_rule_ids()` ever becomes non-deterministic or
/// differs across `CapcoScheme` instances (a future configurable
/// constraint catalog, a per-instance bridge override), the
/// canonicalizer and the severity-table builder MUST see the same
/// set of bridge IDs or the canonicalizer's "every surviving key has
/// a registered intern" invariant breaks and the `.expect()` at
/// Pass 2 panics. Explicit parameter = explicit coupling.
fn build_severity_tables(
    rule_sets: &[Box<dyn RuleSet<CapcoScheme>>],
    overrides: &HashMap<String, String>,
    bridge_rule_ids: &'static [(&'static str, &'static str)],
) -> (FastPathSeverities, EmittedIdOverrides) {
    // Pass 1: collect every canonical `&'static str` rule ID emitted
    // by the rule set — both registered IDs (`rule.id().as_str()`) and
    // per-row catalog IDs from dispatcher walkers
    // (`rule.additional_emitted_ids()`). The override map's keys
    // canonicalize against this superset; everything not in it would
    // have been rejected by `canonicalize_rule_overrides`.
    let mut known_ids: HashSet<&'static str> = HashSet::new();
    for rule_set in rule_sets {
        for rule in rule_set.rules() {
            known_ids.insert(rule.id().as_str());
            for (catalog_id, _catalog_name) in rule.additional_emitted_ids() {
                known_ids.insert(catalog_id);
            }
        }
    }
    // Bridge-emitted IDs (E058 / E059) are valid override keys too,
    // registered through `bridge_emitted_rule_ids` in the
    // canonicalizer. They have no corresponding registered `Rule`
    // impl, but `Engine::lint_inner` emits diagnostics under them
    // from the constraint-bridge path; Sites C/D need their overrides
    // in `emitted_id_overrides`. The caller passes the same bridge
    // IDs slice it handed to `canonicalize_rule_overrides`, making
    // the coupling explicit and ruling out future divergence if
    // `CapcoScheme::bridge_emitted_rule_ids()` ever becomes
    // non-deterministic or differs across `CapcoScheme` instances
    // (both reviewers' HIGH).
    for (bridge_id, _bridge_name) in bridge_rule_ids {
        known_ids.insert(bridge_id);
    }

    // Pass 2: walk the canonicalized override map. The map's keys are
    // owned `String` (canonical IDs the canonicalizer produced from
    // either `id` or `name` forms), but we look them up against
    // `known_ids: HashSet<&'static str>` and store the resolved
    // intern. Malformed severities are silently skipped to preserve
    // the pre-hoist `.and_then(parse_config)` semantics (which would
    // have returned `None` and fallen through to the unwrap_or
    // default).
    let mut emitted_id_overrides: HashMap<&'static str, Severity> = HashMap::new();
    for (canonical_id, severity_str) in overrides {
        let Some(severity) = Severity::parse_config(severity_str.as_str()) else {
            // Pre-hoist behavior: the `.and_then(parse_config)` arm
            // would return `None` for an unparseable severity string,
            // so the fast path fell through to `default_severity()`
            // and the per-emitted-id path preserved the emitted
            // severity. Match that by skipping the insert here.
            continue;
        };
        // The canonicalizer guarantees every surviving key is a
        // known `&'static str` intern (it walks the same superset
        // and `unwrap_or(rule.default_severity())`'s the lookup);
        // if we miss here it means the canonicalizer's invariant
        // has been broken upstream.
        let intern = *known_ids
            .get(canonical_id.as_str())
            .expect("canonicalized override key has a registered &'static intern");
        emitted_id_overrides.insert(intern, severity);
    }

    // Pass 3: build the registered-ID severity table. For each
    // rule-set in declared order, walk its rules in registered order
    // and resolve each rule's registered-ID severity. Lookup against
    // `emitted_id_overrides` (cheaper than the original
    // `overrides.get(...).and_then(parse_config)` chain because the
    // parse already happened in pass 2); fall back to
    // `rule.default_severity()` when absent.
    let fast_path_severities: Box<[Box<[Severity]>]> = rule_sets
        .iter()
        .map(|rule_set| {
            rule_set
                .rules()
                .iter()
                .map(|rule| {
                    emitted_id_overrides
                        .get(rule.id().as_str())
                        .copied()
                        .unwrap_or(rule.default_severity())
                })
                .collect::<Vec<Severity>>()
                .into_boxed_slice()
        })
        .collect::<Vec<Box<[Severity]>>>()
        .into_boxed_slice();

    (fast_path_severities, emitted_id_overrides)
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
    use marque_rules::audit::AppliedFix;
    use marque_rules::{
        Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Rule, RuleContext,
        RuleId, RuleSet, Severity,
    };
    use marque_scheme::fix_intent::RecanonScope;
    use marque_scheme::{
        AuthoritativeSource, Citation, ReplacementIntent, SectionLetter, SectionRef,
    };
    use secrecy::ExposeSecret as _;
    use std::time::{Duration, UNIX_EPOCH};

    /// Test-fixture `Message` stub for `Diagnostic` constructors that
    /// don't exercise message content.
    ///
    /// Uses `UnrecognizedToken` (a generic closed-set template variant)
    /// with default args — no `TokenId` lookup needed, no axis-specific
    /// payload required. The engine tests that consume this helper
    /// assert against `Diagnostic.rule`, `.span`, `.severity`, and
    /// fix-attachment shape, never against message content.
    #[inline]
    fn stub_message() -> Message {
        Message::new(MessageTemplate::UnrecognizedToken, MessageArgs::default())
    }

    /// Filter the marking-side audit lines from a [`FixResult`] into
    /// the legacy `Vec<&AppliedFix>` view the pre-cutover tests read.
    ///
    /// Post PR 3c.2.D the engine's sole audit-output channel is
    /// `FixResult.audit_lines: Vec<AuditLine<S>>`. The cutover
    /// retired the parallel `applied: Vec<AppliedFix<S>>` field;
    /// this helper preserves the pre-cutover read shape for unit
    /// tests that consume only the marking side without rewriting
    /// every assertion site to pattern-match the sum type.
    /// Text-correction audit lines (`AuditLine::TextCorrection`) are
    /// surfaced by [`applied_text_corrections`] below.
    #[inline]
    fn applied_fixes(result: &FixResult) -> Vec<&AppliedFix<CapcoScheme>> {
        result
            .audit_lines
            .iter()
            .filter_map(|line| match line {
                AuditLine::AppliedFix(f) => Some(f),
                _ => None,
            })
            .collect()
    }

    /// Filter the text-correction audit lines from a [`FixResult`]
    /// into a `Vec<&AppliedTextCorrection>` view.
    #[inline]
    fn applied_text_corrections(result: &FixResult) -> Vec<&AppliedTextCorrection> {
        result
            .audit_lines
            .iter()
            .filter_map(|line| match line {
                AuditLine::TextCorrection(tc) => Some(tc),
                _ => None,
            })
            .collect()
    }

    /// Test-fixture `Citation` stub for `Diagnostic` constructors that
    /// don't exercise citation content.
    ///
    /// Uses `AuthoritativeSource::EngineInternal` (a non-CAPCO sentinel
    /// source per PM-C-4) so the citation-lint scanner skips this entry
    /// — these stubs are test fixtures, not real CAPCO citations, and
    /// must not trip the §-citation resolver. The `SectionRef` /
    /// `PageNumber` carry niche-sentinel values the Display impl
    /// deliberately elides for non-CAPCO sources.
    #[inline]
    fn stub_citation() -> Citation {
        Citation::new(
            AuthoritativeSource::EngineInternal,
            SectionRef::new(SectionLetter::A),
            core::num::NonZeroU16::new(1).unwrap(),
        )
    }

    /// Pins the issue #430 pre-size contract on the per-page portion
    /// accumulator. If `fresh_page_portions_accumulator` ever drifts
    /// to `Vec::new()` (or a smaller capacity), every subsequent page
    /// on a multi-page document would pay the `Vec` growth sequence
    /// (4 → 8 → 16 …) on the first several portion pushes — a silent
    /// perf regression no functional test catches. This test fails
    /// at compile-after-edit time if the helper body diverges from
    /// `DEFAULT_PORTIONS_CAPACITY`.
    #[test]
    fn fresh_accumulator_uses_default_capacity() {
        let v = fresh_page_portions_accumulator();
        assert!(
            v.is_empty(),
            "fresh accumulator must be empty, got len={}",
            v.len()
        );
        assert_eq!(
            v.capacity(),
            DEFAULT_PORTIONS_CAPACITY,
            "fresh accumulator capacity drifted from DEFAULT_PORTIONS_CAPACITY ({}); \
             multi-page perf regression risk per issue #430",
            DEFAULT_PORTIONS_CAPACITY,
        );
    }

    /// Pins the rewrite scheduling contract for the generic front edge
    /// of `Engine::with_clock`: extracting a non-generic tail must not
    /// change the rewrite schedule chosen for the default scheme.
    #[test]
    fn with_clock_uses_default_rewrite_schedule() {
        let via_new = Engine::new(
            Config::default(),
            crate::default_ruleset(),
            crate::default_scheme(),
        )
        .expect("default CAPCO scheme has no rewrite cycles");
        let via_with_clock = Engine::with_clock(
            Config::default(),
            crate::default_ruleset(),
            crate::default_scheme(),
            Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(0))),
        )
        .expect("default CAPCO scheme has no rewrite cycles");

        assert_eq!(
            via_new.scheduled_rewrites(),
            via_with_clock.scheduled_rewrites(),
            "scheduling regression: with_clock no longer preserves the scheduler output for the default scheme"
        );
    }

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
                        stub_message(),
                        stub_citation(),
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
        let out = std::str::from_utf8(result.source.expose_secret()).unwrap();
        assert!(out.starts_with("AA//BB"), "got: {out:?}");
        // StubRule emits text-correction diagnostics; the marque-1.0
        // audit stream surfaces them on the `TextCorrection` arm.
        assert_eq!(applied_text_corrections(&result).len(), 2);
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
        let applied = applied_text_corrections(&result);
        assert_eq!(applied.len(), 1, "applied: {applied:?}");
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
        assert_eq!(
            result.source.expose_secret(),
            TEST_SRC,
            "dry-run must not mutate source"
        );
        let text_corrections = applied_text_corrections(&result);
        assert_eq!(text_corrections.len(), 1);
        assert!(text_corrections[0].dry_run, "dry_run flag must be set");
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
        // StubRule emits text-correction diagnostics; timestamps live
        // on `AppliedTextCorrection`.
        assert_eq!(
            applied_text_corrections(&r1)[0].timestamp,
            applied_text_corrections(&r2)[0].timestamp
        );
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
        assert_eq!(applied_fixes(&result).len(), 0);
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
        assert_eq!(applied_fixes(&fix_result).len(), 0);
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
                    stub_message(),
                    stub_citation(),
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
                    stub_message(),
                    stub_citation(),
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
            applied_fixes(&fix_result).len(),
            0,
            "explicit Suggest-severity fix must not auto-apply regardless of confidence"
        );
    }

    #[test]
    fn confidence_at_default_threshold_is_included() {
        // A fix at exactly 0.95 must be applied (inclusive threshold).
        let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.95)]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(applied_text_corrections(&result).len(), 1);
    }

    // M-5: the zero-length-span filter (`!f.span.is_empty()`) in fix_inner
    // is what masked the Phase 2 Span::new(0, 0) placeholders from the
    // C-1 overlap guard. This test pins that guard explicitly so a future
    // refactor that drops the filter is caught.
    #[test]
    fn zero_length_span_fix_is_filtered_before_sort() {
        let engine = engine_with(vec![proposal("E001", 5, 5, "X")]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(applied_text_corrections(&result).len(), 0);
        // Source unchanged: no splice was attempted.
        assert_eq!(result.source.expose_secret(), TEST_SRC);
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
        // Only the 0.6 fix is applied. StubRule emits text-corrections.
        let text_corrections = applied_text_corrections(&result);
        assert_eq!(text_corrections.len(), 1);
        assert_eq!(text_corrections[0].rule.as_str(), "E002");
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

    // F.1: per-page accumulator reset semantics are observable.
    //
    // ContextRecorderRule captures the live `ctx.page_portions` length
    // every time it's invoked. By running the engine over a multi-page
    // document and inspecting the captured counts at each banner candidate,
    // we prove that the engine resets the accumulator at the page break
    // instead of accumulating across pages.
    #[derive(Clone)]
    struct ContextRecorderRule {
        observations: std::sync::Arc<std::sync::Mutex<Vec<(marque_ism::MarkingType, usize)>>>,
    }

    impl Rule<CapcoScheme> for ContextRecorderRule {
        fn id(&self) -> RuleId {
            RuleId::new("RECORD")
        }
        fn name(&self) -> &'static str {
            "page-portions-recorder"
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
                .page_portions
                .as_ref()
                .map(|pp| pp.as_ref().len())
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
    fn page_portions_reset_observably_across_form_feed() {
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
        // (NOT 2) — the form feed must have reset the accumulator.
        let src: &[u8] = b"(SECRET//NF) p1 text\nSECRET//NOFORN\n\x0c(CONFIDENTIAL//NF) p2\nCONFIDENTIAL//NOFORN\n";
        let _ = engine.lint(src);

        let obs = observations.lock().unwrap();
        // The recorder ran once per non-PageBreak candidate. Filter to
        // banners and check the per-page portion count each banner saw.
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
    fn page_portions_lint_starts_fresh_on_each_call() {
        // Calling Engine::lint twice on the same engine must produce a
        // fresh per-page accumulator for the second call — no cross-call
        // accumulation.
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

    #[test]
    fn parsed_markings_cache_persists_across_page_breaks() {
        // CA-1 guard: page-break handling resets the per-page projection
        // accumulators, but must NOT reset the per-document
        // `parsed_markings` cache used by fix synthesis.
        struct ParsedCacheIntentRule;
        impl Rule<CapcoScheme> for ParsedCacheIntentRule {
            fn id(&self) -> RuleId {
                RuleId::new("PARSED_CACHE_TEST")
            }
            fn name(&self) -> &'static str {
                "parsed-cache-test"
            }
            fn default_severity(&self) -> Severity {
                Severity::Fix
            }
            fn check(
                &self,
                _attrs: &CanonicalAttrs,
                ctx: &RuleContext,
            ) -> Vec<Diagnostic<CapcoScheme>> {
                if ctx.marking_type != marque_ism::MarkingType::Portion {
                    return vec![];
                }
                vec![Diagnostic::with_fix_at_span(
                    self.id(),
                    self.default_severity(),
                    ctx.candidate_span,
                    ctx.candidate_span,
                    stub_message(),
                    stub_citation(),
                    FixIntent {
                        replacement: ReplacementIntent::Recanonicalize {
                            scope: RecanonScope::Portion,
                        },
                        confidence: Confidence::strict(0.99),
                        feature_ids: SmallVec::new(),
                        message: Message::new(
                            MessageTemplate::BannerRollupMismatch,
                            MessageArgs::default(),
                        ),
                        source: FixSource::BuiltinRule,
                        migration_ref: None,
                    },
                )]
            }
        }

        let set: Box<dyn RuleSet<CapcoScheme>> =
            Box::new(RecorderSet(vec![Box::new(ParsedCacheIntentRule)]));
        let engine = Engine::with_clock(
            Config::default(),
            vec![set],
            marque_capco::scheme::CapcoScheme::new(),
            Box::new(FixedClock::new(
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )),
        )
        .expect("default CAPCO scheme has no rewrite cycles")
        .with_strict_recognizer();

        // Two portions split by a form-feed page break. The test rule
        // emits a FixIntent on each portion, so both candidates should
        // populate `parsed_markings`.
        let src = b"(S)\n\x0c(S)\n";
        let (lint, parsed_markings) =
            engine.lint_with_options_internal(src, &LintOptions::default());

        assert!(!lint.truncated, "test fixture should not hit lint deadline");
        assert_eq!(
            parsed_markings.len(),
            2,
            "parsed_markings must retain entries from both pages; a page-break reset here would drop the first page entry"
        );
        assert!(
            parsed_markings[0].0.start < parsed_markings[1].0.start,
            "cache order must stay scanner-order sorted by Span.start"
        );
    }

    // M6: FR-016 tiebreaker — same span, different rule IDs.
    // The sort is (span.end DESC, span.start DESC, rule_id ASC, replacement ASC).
    // When two fixes target the exact same span, rule_id ASC breaks the tie,
    // and C-1 drops the second (overlapping) fix.
    #[test]
    fn fr016_same_span_different_rule_ids_picks_lower_rule_id() {
        // Two proposals for span 0..6 with different rule IDs.
        // "C001" < "E001" lexicographically, so C001 is kept and E001 dropped.
        let engine = engine_with(vec![
            proposal("E001", 0, 6, "BB"),
            proposal("C001", 0, 6, "AA"),
        ]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        // Text-correction fixes flow through `AuditLine::TextCorrection`
        // post-cutover.
        let text_corrections = applied_text_corrections(&result);
        assert_eq!(text_corrections.len(), 1);
        assert_eq!(text_corrections[0].rule.as_str(), "C001");
        assert_eq!(text_corrections[0].replacement.as_str(), "AA");
    }

    // FR-016 tiebreaker — same span, same rule ID, different replacements.
    #[test]
    fn fr016_same_span_same_rule_picks_lower_replacement() {
        let engine = engine_with(vec![
            proposal("E001", 0, 6, "ZZZ"),
            proposal("E001", 0, 6, "AAA"),
        ]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        let text_corrections = applied_text_corrections(&result);
        assert_eq!(text_corrections.len(), 1);
        assert_eq!(text_corrections[0].replacement.as_str(), "AAA");
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
    // `build_severity_tables` — construction-time severity hoist
    // -----------------------------------------------------------------------
    //
    // These tests pin the population semantics of the two pre-resolved
    // tables that drive the lint hot-loop's Sites A/B/C/D:
    //
    //   - `fast_path_severities` — indexed by (set_idx, rule_idx),
    //     resolves to `default_severity` when no override exists.
    //   - `emitted_id_overrides` — sparse, only populated when an
    //     override is present AND parses to a valid severity.
    //
    // Walker rules (those with non-empty `additional_emitted_ids()`)
    // get a `fast_path_severities` entry too (Site A's guard means
    // it's read-but-unused for walkers), but catalog-ID overrides
    // (e.g., `E035` on `BannerMatchesProjectedRule`) only ever land
    // in `emitted_id_overrides` — they do NOT affect the walker
    // rule's `fast_path_severities` entry.

    #[test]
    fn build_severity_tables_empty_overrides_returns_defaults() {
        // No overrides — every rule's fast-path entry must equal its
        // `default_severity()` and `emitted_id_overrides` must be
        // empty. This pins the "absence preserves default" semantics
        // that Site A's `unwrap_or(rule.default_severity())` arm
        // relied on pre-hoist.
        let engine = capco_engine_with_overrides(&[]);
        assert!(
            engine.emitted_id_overrides.is_empty(),
            "no overrides means emitted_id_overrides empty; got: {:?}",
            engine.emitted_id_overrides,
        );
        assert_eq!(
            engine.fast_path_severities.len(),
            engine.rule_sets.len(),
            "fast_path_severities outer len must match rule_sets len",
        );
        for (set_idx, rule_set) in engine.rule_sets.iter().enumerate() {
            let set_table = &engine.fast_path_severities[set_idx];
            assert_eq!(
                set_table.len(),
                rule_set.rules().len(),
                "fast_path_severities[{set_idx}] inner len must match rule count",
            );
            for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
                assert_eq!(
                    set_table[rule_idx],
                    rule.default_severity(),
                    "fast_path_severities[{set_idx}][{rule_idx}] for rule {:?} \
                     must equal default_severity with no override; got {:?} \
                     vs default {:?}",
                    rule.id().as_str(),
                    set_table[rule_idx],
                    rule.default_severity(),
                );
            }
        }
    }

    #[test]
    fn build_severity_tables_registered_id_override_applies() {
        // Single registered-ID override: `E002 = "off"`. E002
        // (`missing-usa-trigraph`) is a non-walker rule registered
        // in `CapcoRuleSet::new()`. The fast-path table entry for
        // E002 must become `Off`; every other rule's entry must
        // stay at its default; `emitted_id_overrides` must contain
        // exactly `{"E002": Off}`.
        let engine = capco_engine_with_overrides(&[("E002", "off")]);

        // Find the (set_idx, rule_idx) for E002.
        let mut e002_loc: Option<(usize, usize)> = None;
        for (set_idx, rule_set) in engine.rule_sets.iter().enumerate() {
            for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
                if rule.id().as_str() == "E002" {
                    e002_loc = Some((set_idx, rule_idx));
                    break;
                }
            }
        }
        let (set_idx, rule_idx) = e002_loc.expect("E002 must be registered in CapcoRuleSet");

        assert_eq!(
            engine.fast_path_severities[set_idx][rule_idx],
            Severity::Off,
            "fast_path_severities for E002 must reflect the `off` override",
        );

        // Every other registered rule's entry must equal its default.
        for (s, rule_set) in engine.rule_sets.iter().enumerate() {
            for (r, rule) in rule_set.rules().iter().enumerate() {
                if (s, r) == (set_idx, rule_idx) {
                    continue;
                }
                assert_eq!(
                    engine.fast_path_severities[s][r],
                    rule.default_severity(),
                    "fast_path_severities[{s}][{r}] for rule {:?} must \
                     stay at default when only E002 is overridden",
                    rule.id().as_str(),
                );
            }
        }

        // `emitted_id_overrides` populated with exactly one entry.
        assert_eq!(
            engine.emitted_id_overrides.len(),
            1,
            "exactly one emitted_id_overrides entry; got: {:?}",
            engine.emitted_id_overrides,
        );
        assert_eq!(
            engine.emitted_id_overrides.get("E002").copied(),
            Some(Severity::Off),
            "emitted_id_overrides[\"E002\"] must be Off",
        );
    }

    #[test]
    fn build_severity_tables_catalog_id_override_lands_in_emitted_only() {
        // E035 (`sci-banner-rollup`) is a per-row catalog ID on
        // `BannerMatchesProjectedRule` — emitted by the walker but
        // NOT a registered rule ID (the walker registers under
        // E031). A `[rules] E035 = "warn"` override must:
        //
        //   1. Land in `emitted_id_overrides` so Site B's
        //      per-diagnostic `retain_mut` can rewrite the
        //      diagnostic's severity from its emitted Error to Warn.
        //   2. NOT change the walker's own `fast_path_severities`
        //      entry — Site A only consults that entry when the
        //      rule's `additional_emitted_ids().is_empty()`, which
        //      is false for walker rules, so the entry is unread;
        //      but pinning it here also catches an inverted
        //      population (a future bug that conflated registered
        //      and catalog ID lookups).
        let engine = capco_engine_with_overrides(&[("E035", "warn")]);

        // Find the walker rule (registered ID E031).
        let mut walker_loc: Option<(usize, usize, Severity)> = None;
        for (set_idx, rule_set) in engine.rule_sets.iter().enumerate() {
            for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
                if rule.id().as_str() == "E031" {
                    walker_loc = Some((set_idx, rule_idx, rule.default_severity()));
                    break;
                }
            }
        }
        let (set_idx, rule_idx, walker_default) =
            walker_loc.expect("BannerMatchesProjectedRule (E031) must be registered");

        assert_eq!(
            engine.fast_path_severities[set_idx][rule_idx], walker_default,
            "fast_path_severities[E031] must stay at the walker's \
             default_severity — an `E035 = warn` override is a \
             catalog-ID override that affects the per-emitted-id \
             path, not the registered-ID fast-path table",
        );

        // E035 (NOT E031) must be in `emitted_id_overrides`.
        assert_eq!(
            engine.emitted_id_overrides.get("E035").copied(),
            Some(Severity::Warn),
            "emitted_id_overrides[\"E035\"] must be Warn",
        );
        assert!(
            !engine.emitted_id_overrides.contains_key("E031"),
            "the override targets E035; E031 must NOT appear in \
             emitted_id_overrides",
        );
        assert_eq!(
            engine.emitted_id_overrides.len(),
            1,
            "exactly one emitted_id_overrides entry; got: {:?}",
            engine.emitted_id_overrides,
        );
    }

    #[test]
    fn build_severity_tables_skips_unparsable_severity() {
        // The canonicalizer accepts arbitrary severity strings (it
        // only validates the rule-key side), so a malformed
        // severity like `"borked"` survives to
        // `build_severity_tables`. The pre-hoist code used
        // `.and_then(parse_config)` which returned `None` on a
        // malformed string and fell through to
        // `unwrap_or(default_severity)`. Preserve that exactly: the
        // E002 rule's fast-path entry stays at its default and
        // `emitted_id_overrides` does NOT contain `"E002"`.
        let engine = capco_engine_with_overrides(&[("E002", "borked")]);

        // Find E002's location.
        let mut e002_loc: Option<(usize, usize, Severity)> = None;
        for (set_idx, rule_set) in engine.rule_sets.iter().enumerate() {
            for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
                if rule.id().as_str() == "E002" {
                    e002_loc = Some((set_idx, rule_idx, rule.default_severity()));
                    break;
                }
            }
        }
        let (set_idx, rule_idx, e002_default) =
            e002_loc.expect("E002 must be registered in CapcoRuleSet");

        assert_eq!(
            engine.fast_path_severities[set_idx][rule_idx], e002_default,
            "unparseable severity must fall through to default — \
             fast_path_severities[E002] expected {:?}, got {:?}",
            e002_default, engine.fast_path_severities[set_idx][rule_idx],
        );
        assert!(
            !engine.emitted_id_overrides.contains_key("E002"),
            "unparseable severity must NOT populate \
             emitted_id_overrides; got: {:?}",
            engine.emitted_id_overrides,
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
        // PR 3c.2.C C5: typed `Message` carries the contributing rule
        // IDs structurally via `MessageArgs.contributing_rule_ids`
        // (closed-set permitted type). The args check is stricter than
        // the legacy substring check because it asserts on a closed
        // type rather than a string substring.
        assert_eq!(diag.message.template(), MessageTemplate::ReparseFailed);
        let contributors = &diag.message.args().contributing_rule_ids;
        assert!(contributors.iter().any(|id| id.as_str() == "C001"));
        assert!(contributors.iter().any(|id| id.as_str() == "E006"));
    }

    #[test]
    fn build_r002_diagnostic_empty_contributors_uses_generic_message() {
        let contributing: SmallVec<[RuleId; 4]> = SmallVec::new();
        let failure_span = Span::new(0, 0);
        let diag = super::build_r002_diagnostic(contributing, failure_span);

        assert_eq!(diag.rule, super::R002_RULE_ID);
        assert!(diag.fix.is_none());
        assert!(diag.text_correction.is_none());
        // PR 3c.2.C C5: empty-contributors branch identified by
        // empty `contributing_rule_ids` SmallVec, not by message
        // substring.
        assert_eq!(diag.message.template(), MessageTemplate::ReparseFailed);
        assert!(diag.message.args().contributing_rule_ids.is_empty());
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
            stub_message(),
            stub_citation(),
            None,
        );
        let pass2_id = Diagnostic::<CapcoScheme>::new(
            RuleId::new("E022"),
            Severity::Error,
            Span::new(4, 8),
            stub_message(),
            stub_citation(),
            None,
        );
        let unknown_id = Diagnostic::<CapcoScheme>::new(
            RuleId::new("E999"),
            Severity::Error,
            Span::new(8, 12),
            stub_message(),
            stub_citation(),
            None,
        );
        let text_corr_no_fix = Diagnostic::text_correction(
            RuleId::new("C001"),
            Severity::Fix,
            Span::new(12, 16),
            stub_message(),
            stub_citation(),
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
    fn partition_diags_by_phase_returns_references_not_clones() {
        // Copilot round-3 R3-2 regression test: the partition MUST
        // return reference vectors borrowing from `diagnostics`, not
        // cloned owned vectors. The cloning shape allocated O(N)
        // Diagnostic bodies on every call, and `partition_diags_by_phase`
        // is called up to twice per fix on the hot path — that
        // allocation cost is unjustified under Constitution I.
        //
        // Behavioral lock: build an input slice, partition it, and
        // verify each entry in the returned partitions is pointer-
        // equal (via `std::ptr::eq`) to its source entry. Pointer
        // identity is the load-bearing assertion — a clone-based
        // partition would produce structurally-equal but
        // pointer-distinct Diagnostics, failing the test.
        let localized: HashSet<&'static str> = ["E006"].into_iter().collect();

        let diags = vec![
            Diagnostic::<CapcoScheme>::new(
                RuleId::new("E006"),
                Severity::Error,
                Span::new(0, 4),
                stub_message(),
                stub_citation(),
                None,
            ),
            Diagnostic::<CapcoScheme>::new(
                RuleId::new("E022"),
                Severity::Error,
                Span::new(4, 8),
                stub_message(),
                stub_citation(),
                None,
            ),
        ];
        let (p1, p2) = super::partition_diags_by_phase(&diags, &localized);
        assert_eq!(p1.len(), 1);
        assert_eq!(p2.len(), 1);
        assert!(
            std::ptr::eq(p1[0], &diags[0]),
            "pass-1 partition entry must be a reference to the original Diagnostic"
        );
        assert!(
            std::ptr::eq(p2[0], &diags[1]),
            "pass-2 partition entry must be a reference to the original Diagnostic"
        );
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
            stub_message(),
            stub_citation(),
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

        // Bind the input array to a named local so the reference
        // partition (Copilot round-3 R3-2) outlives the assertion
        // — `partition_diags_by_phase` now returns reference
        // vectors, so the source `[Diagnostic]` must remain live.
        let diags = [tc];
        let (p1, p2) = super::partition_diags_by_phase(&diags, &localized);
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
                    stub_message(),
                    stub_citation(),
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
        let applied_view = applied_fixes(&result);
        let saw_stub_fix = applied_view.iter().any(|f| f.rule.as_str() == "E899");
        assert!(
            saw_stub_fix,
            "stub localized FixIntent rule's fix must be promoted into AppliedFix; \
             applied: {:?}",
            applied_view
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
                    stub_message(),
                    stub_citation(),
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
        assert_eq!(
            result.source.expose_secret(),
            TEST_SRC,
            "DryRun must not mutate source"
        );
        assert!(!result.r002_fired);
        let applied_view = applied_fixes(&result);
        let stub_fix = applied_view
            .iter()
            .find(|f| f.rule.as_str() == "E898")
            .expect("stub fix should appear in applied list");
        assert!(
            stub_fix.dry_run,
            "DryRun applied fix must have dry_run=true"
        );
    }

    #[test]
    fn apply_kept_fixes_splices_post_buffer_in_dryrun_mode() {
        // Copilot round-2 finding R2-3 lock.
        //
        // Pre-R2-3, `apply_kept_fixes` short-circuited in DryRun mode
        // and returned the unspliced source as `post_buffer`. The outer
        // `TwoPassFixer::run` then re-linted that unspliced buffer and
        // dispatched pass-2 against the WRONG coordinate space — same
        // byte input as Apply but a different pass-2 context, breaking
        // the DryRun-as-preview contract (FR-022 / FR-023).
        //
        // Post-R2-3, `apply_kept_fixes` always builds the post-splice
        // buffer in BOTH modes; only the OUTER `FixResult.source`
        // differs between Apply and DryRun (the outer layer in
        // `run_pass2_whole_marking` substitutes `self.source.to_vec()`
        // for DryRun). The intermediate pass-1 → pass-2 buffer must
        // be the spliced output regardless of mode so pass-2 dispatch
        // is mode-invariant.
        //
        // This test pins the structural property directly: it calls
        // `apply_kept_fixes` with the same synthesized fixes in both
        // modes and asserts the returned `post_buffer` is the spliced
        // result in both. A future regression to "skip splicing in
        // DryRun" would flip the DryRun assertion to the unspliced
        // source bytes and fail loudly.
        let engine = engine_with(vec![]);
        let source = b"SECRET//NOFORN";

        // FR-016-sorted (span.end DESC): the synth helper produces
        // one fix at 8..14 replacing "NOFORN" with "REL TO USA".
        let kept_fixes = vec![synth_fix("E001", 8, 14, "REL TO USA")];
        let expected_post_buffer = b"SECRET//REL TO USA".to_vec();

        // Build a dummy LintResult so `apply_kept_fixes`'s deadline-
        // error branch has something to clone (the test never trips
        // the deadline, but the signature requires it).
        let dummy_lint = LintResult::default();

        // Apply mode — establish the spliced baseline.
        let apply_fixer = super::TwoPassFixer {
            engine: &engine,
            source,
            mode: FixMode::Apply,
            threshold: 0.95,
            deadline: None,
        };
        let (apply_post, _, apply_audit_lines) = apply_fixer
            .apply_kept_fixes(source, kept_fixes.clone(), &dummy_lint)
            .expect("apply_kept_fixes succeeds in Apply mode");
        assert_eq!(
            &*apply_post, &expected_post_buffer,
            "Apply mode: post_buffer must be the spliced result",
        );
        for line in &apply_audit_lines {
            if let AuditLine::AppliedFix(f) = line {
                assert!(!f.dry_run, "Apply: dry_run must be false");
            }
        }

        // DryRun mode — the load-bearing R2-3 assertion. Pre-R2-3,
        // this branch returned `source.to_vec()` (unspliced). After
        // the fix, it returns the spliced buffer just like Apply.
        let dry_run_fixer = super::TwoPassFixer {
            engine: &engine,
            source,
            mode: FixMode::DryRun,
            threshold: 0.95,
            deadline: None,
        };
        let (dry_run_post, _, dry_run_audit_lines) = dry_run_fixer
            .apply_kept_fixes(source, kept_fixes, &dummy_lint)
            .expect("apply_kept_fixes succeeds in DryRun mode");
        assert_eq!(
            &*dry_run_post, &expected_post_buffer,
            "DryRun mode: post_buffer must be the spliced result so pass-2 \
             dispatches against the same coordinate space as Apply (R2-3 lock)",
        );
        // Sanity: post_buffer is NOT the unspliced source — that's
        // the exact pre-R2-3 behavior this test exists to detect.
        assert_ne!(
            dry_run_post.as_slice(),
            source,
            "DryRun post_buffer must differ from the unspliced source — \
             returning the unspliced source is the R2-3 regression"
        );
        for line in &dry_run_audit_lines {
            if let AuditLine::AppliedFix(f) = line {
                assert!(f.dry_run, "DryRun: dry_run must be true");
            }
        }

        // Cross-mode parity at the AppliedFix (rule, span) level:
        // the promotion loop is shared, so the applied set must be
        // identical modulo the dry_run flag.
        let apply_applied: Vec<&AppliedFix<CapcoScheme>> = apply_audit_lines
            .iter()
            .filter_map(|l| {
                if let AuditLine::AppliedFix(f) = l {
                    Some(f)
                } else {
                    None
                }
            })
            .collect();
        let dry_run_applied: Vec<&AppliedFix<CapcoScheme>> = dry_run_audit_lines
            .iter()
            .filter_map(|l| {
                if let AuditLine::AppliedFix(f) = l {
                    Some(f)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            apply_applied.len(),
            dry_run_applied.len(),
            "applied list length must match across modes",
        );
        for (a, d) in apply_applied.iter().zip(dry_run_applied.iter()) {
            assert_eq!(a.rule, d.rule);
            assert_eq!(a.span, d.span);
        }
    }

    // -------------------------------------------------------------------
    // Pure-helper unit tests — sort_and_c1_dedup / splice_fixes_forward /
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
            severity: Severity::Fix,
            span: Span::new(start, end),
            replacement: replacement.into(),
            scope: Scope::Portion,
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
    fn splice_fixes_forward_splices_in_reverse_order() {
        // Source: "SECRET//NOFORN" (14 bytes).
        // Two fixes: 0..6 → "AA", 8..14 → "BB".
        // FR-016 sort (span.end DESC) → 8..14 first, then 0..6.
        // forward walk via `iter().rev()` yields 0..6 then 8..14.
        let source = b"SECRET//NOFORN";
        let fixes = super::sort_and_c1_dedup(vec![
            synth_fix("E001", 0, 6, "AA"),
            synth_fix("E002", 8, 14, "BB"),
        ]);
        let out = super::splice_fixes_forward(source, &fixes);
        assert_eq!(out, b"AA//BB");
    }

    #[test]
    fn splice_fixes_forward_with_empty_fixes_returns_source_clone() {
        let source = b"SECRET//NOFORN";
        let out = super::splice_fixes_forward(source, &[]);
        assert_eq!(out, source);
    }

    #[test]
    fn splice_fixes_forward_handles_replacement_growth_and_shrink() {
        // 0..6 → "TOP SECRET" (grow), 8..14 → "X" (shrink).
        let source = b"SECRET//NOFORN";
        let fixes = super::sort_and_c1_dedup(vec![
            synth_fix("E001", 0, 6, "TOP SECRET"),
            synth_fix("E002", 8, 14, "X"),
        ]);
        let out = super::splice_fixes_forward(source, &fixes);
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
        // Construct a synthetic `parsed_markings` directly. Issue #433
        // made the engine's cache populate lazily (only when a
        // diagnostic with `fix.is_some()` is emitted for the
        // candidate), so a fixture that exercises the cache via
        // `lint_with_options_internal` would need a FixIntent-emitting
        // input. The function under test (`find_containing_marking`)
        // keys on `Span` only — building the slice directly tests the
        // lookup semantics without coupling to engine cache policy.
        // Issue #432: cache type swapped from `HashMap<Span, ...>` to
        // `Vec<(Span, ...)>` sorted by `Span.start`; this fixture has
        // one entry so order is trivial.
        let marking_span = Span::new(0, 13);
        let markings: Vec<(Span, marque_capco::CapcoMarking)> = vec![(
            marking_span,
            marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
        )];
        // A sub-span inside marking_span resolves to marking_span.
        let sub = Span::new(marking_span.start, marking_span.start + 1);
        let found = super::find_containing_marking(&markings, sub);
        assert_eq!(found, Some(marking_span));
    }

    #[test]
    fn find_containing_marking_returns_none_when_no_marking_contains() {
        let markings: Vec<(Span, marque_capco::CapcoMarking)> = vec![(
            Span::new(0, 13),
            marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
        )];
        // Way past the inserted marking span — no marking contains it.
        let far = Span::new(10_000, 10_001);
        let found = super::find_containing_marking(&markings, far);
        assert!(found.is_none());
    }

    #[test]
    fn lookup_marking_finds_exact_span() {
        // Pin the binary-search-by-start lookup semantics — exact
        // `Span` match returns the entry; mismatched end returns None.
        // Issue #432.
        let span_a = Span::new(0, 13);
        let span_b = Span::new(20, 35);
        let markings: Vec<(Span, marque_capco::CapcoMarking)> = vec![
            (
                span_a,
                marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
            ),
            (
                span_b,
                marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
            ),
        ];

        assert!(super::lookup_marking(&markings, span_a).is_some());
        assert!(super::lookup_marking(&markings, span_b).is_some());
        // Same start, different end — does NOT match.
        assert!(super::lookup_marking(&markings, Span::new(0, 12)).is_none());
        // Start not in the table — does NOT match.
        assert!(super::lookup_marking(&markings, Span::new(5, 10)).is_none());
        // Between two entries — binary search lands on an adjacent
        // entry, the equality post-check rejects. Pins the case that
        // would silently regress if the search key changed from
        // `s.start` to something else.
        assert!(super::lookup_marking(&markings, Span::new(14, 19)).is_none());
    }

    #[test]
    fn lookup_marking_walks_duplicate_start_run() {
        // The cache's strictly-increasing-start invariant is enforced
        // at the push site by a `debug_assert!`, but `lookup_marking`
        // is defensive against future regressions: if duplicate-start
        // entries ever sneak in, the binary search may land on the
        // wrong same-start entry. The forward+backward walk over the
        // matching-start run finds the target if it exists. This test
        // builds a deliberately-degenerate slice (bypassing the engine
        // push site) to pin the walk's correctness in isolation.
        // Issue #432 + suppressed-comment follow-up on PR #481.
        let target = Span::new(50, 65);
        let markings: Vec<(Span, marque_capco::CapcoMarking)> = vec![
            (
                Span::new(50, 55),
                marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
            ),
            (
                Span::new(50, 60),
                marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
            ),
            (
                target,
                marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
            ),
            (
                Span::new(50, 70),
                marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
            ),
        ];
        // The walk finds the exact target regardless of which entry
        // the binary search initially landed on (criterion would
        // otherwise be non-deterministic across implementations).
        assert!(super::lookup_marking(&markings, target).is_some());
        // A start-matching but end-mismatching probe across the same
        // run still returns None.
        assert!(super::lookup_marking(&markings, Span::new(50, 80)).is_none());
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
    // Synthetic `AuditLine::AppliedFix` records here are constructed
    // via `__engine_promote` under the Constitution V Principle V
    // test-fixture carve-out — the fabricated fixes never flow into a
    // real audit stream; they exist to feed the assembler under test.
    fn synth_audit_line(rule: &'static str, start: usize, end: usize) -> AuditLine<CapcoScheme> {
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
        let span = Span::new(start, end);
        // Original-bytes slice for the synthetic record; the bytes
        // hash inline at construction and are never stored. The
        // EngineConstructor-minted Canonical carries the same
        // synthetic payload.
        let original_bytes: &[u8] = b"synth";
        // Test-fixture carve-out per Constitution V Principle V — the
        // `EngineConstructor` mint here mirrors the `__engine_promote`
        // mint below; both feed `synth_audit_line` and never reach a
        // real audit stream.
        let constructor: EngineConstructor<CapcoScheme> =
            EngineConstructor::<CapcoScheme>::__engine_construct();
        let canonical = constructor.build_open_vocab(
            CategoryId::MARKING,
            Box::from("(S)"),
            marque_scheme::Scope::Portion,
        );
        // Test-fixture carve-out per Constitution V Principle V — this
        // call sits inside #[cfg(test)] and feeds the
        // `assemble_r002_result` / `contributing_pass1_rule_ids` unit
        // tests; the fabricated record is never commingled with engine
        // output.
        let applied = AppliedFix::__engine_promote(
            RuleId::new(rule),
            Severity::Fix,
            span,
            intent,
            original_bytes,
            canonical,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            None,
            false,
            None,
            engine_promotion_token(),
        );
        AuditLine::AppliedFix(applied)
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
            synth_audit_line("E006", 0, 4),
            synth_audit_line("C001", 4, 8),
            synth_audit_line("E006", 8, 12),
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
            synth_audit_line("E009", 0, 4),
            synth_audit_line("E008", 4, 8),
            synth_audit_line("E007", 8, 12),
            synth_audit_line("E006", 12, 16),
            synth_audit_line("C001", 16, 20),
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

        let pass0_audit_lines = vec![synth_audit_line("C001", 0, 6)];
        let pass1_audit_lines = vec![synth_audit_line("E006", 8, 12)];
        let pass1 = Pass1Result {
            post_buffer: Zeroizing::new(b"POST-PASS-1-BUFFER".to_vec()),
            audit_lines: pass1_audit_lines,
            applied_keys: HashSet::new(),
        };
        let lint = LintResult {
            diagnostics: Vec::new(),
            truncated: false,
            candidates_processed: 0,
            candidates_total: 0,
            recognized_marking_count: 0,
        };
        let r002 = super::build_r002_diagnostic(
            smallvec::smallvec![RuleId::new("E006")],
            Span::new(0, 18),
        );
        let result =
            fixer.assemble_r002_result(pass0_audit_lines, Vec::new(), pass1, lint, r002.clone());

        // Order: pass0 (C001) then pass1 (E006). Synth records use
        // the `AuditLine::AppliedFix` arm regardless of rule ID (the
        // test-fixture path doesn't route by rule).
        assert_eq!(result.audit_lines.len(), 2);
        let first_rule = match &result.audit_lines[0] {
            AuditLine::AppliedFix(f) => f.rule.as_str(),
            AuditLine::TextCorrection(tc) => tc.rule.as_str(),
            _ => "unknown",
        };
        let second_rule = match &result.audit_lines[1] {
            AuditLine::AppliedFix(f) => f.rule.as_str(),
            AuditLine::TextCorrection(tc) => tc.rule.as_str(),
            _ => "unknown",
        };
        assert_eq!(first_rule, "C001");
        assert_eq!(second_rule, "E006");
        // R002 fired flag set.
        assert!(result.r002_fired);
        // R002 diagnostic is the last entry in remaining_diagnostics.
        assert!(!result.remaining_diagnostics.is_empty());
        let last = result.remaining_diagnostics.last().unwrap();
        assert_eq!(last.rule, super::R002_RULE_ID);
        // Apply mode returns the pass-1 buffer.
        assert_eq!(result.source.expose_secret(), b"POST-PASS-1-BUFFER");
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
            post_buffer: Zeroizing::new(b"POST-PASS-1-BUFFER".to_vec()),
            audit_lines: vec![synth_audit_line("E006", 8, 12)],
            applied_keys: HashSet::new(),
        };
        let lint = LintResult {
            diagnostics: Vec::new(),
            truncated: false,
            candidates_processed: 0,
            candidates_total: 0,
            recognized_marking_count: 0,
        };
        let r002 = super::build_r002_diagnostic(SmallVec::new(), Span::new(0, 0));
        let result = fixer.assemble_r002_result(Vec::new(), Vec::new(), pass1, lint, r002);
        assert_eq!(result.source.expose_secret(), TEST_SRC);
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
            stub_message(),
            stub_citation(),
            None,
        )];
        let pass1 = Pass1Result {
            post_buffer: Zeroizing::new(Vec::new()),
            audit_lines: Vec::new(),
            applied_keys: HashSet::new(),
        };
        let lint = LintResult {
            diagnostics: Vec::new(),
            truncated: false,
            candidates_processed: 0,
            candidates_total: 0,
            recognized_marking_count: 0,
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
            stub_message(),
            stub_citation(),
            Some(intent),
        );
        let pass1_audit_lines = vec![synth_audit_line("E006", 8, 14)];
        let pass1 = Pass1Result {
            post_buffer: Zeroizing::new(Vec::new()),
            audit_lines: pass1_audit_lines,
            applied_keys: HashSet::new(),
        };
        let lint = LintResult {
            diagnostics: vec![diag_with_fix],
            truncated: false,
            candidates_processed: 0,
            candidates_total: 0,
            recognized_marking_count: 0,
        };
        let r002 = super::build_r002_diagnostic(SmallVec::new(), Span::new(0, 0));
        let result = fixer.assemble_r002_result(Vec::new(), Vec::new(), pass1, lint, r002);
        // Pre-r002 entries are 0 (the E006 diag was filtered),
        // then R002 is pushed last.
        assert_eq!(result.remaining_diagnostics.len(), 1);
        assert_eq!(result.remaining_diagnostics[0].rule, super::R002_RULE_ID);
    }
}

// ---------------------------------------------------------------------------
// PR #490 — PageFinalization read-only-attrs sentinel helper tests
// ---------------------------------------------------------------------------
//
// Separate `#[cfg(test)]` module from `mod tests` above because the
// existing module carries `#[cfg_attr(coverage_nightly, coverage(off))]`
// — these sentinel tests need to land in Codecov patch coverage (the
// motivation for extracting `check_portions_unchanged` to a testable
// helper in the first place). Keeping them in a coverage-included
// module makes the comparison + error-message-construction paths
// of `check_portions_unchanged` visible to the coverage tool.

#[cfg(test)]
mod sentinel_tests {
    use super::check_portions_unchanged;
    use marque_ism::{CanonicalAttrs, Classification, MarkingClassification};

    /// Construct a default `CanonicalAttrs`. `CanonicalAttrs` is
    /// `#[non_exhaustive]` so we use `Default::default()` and patch
    /// the field(s) the test needs.
    fn empty_attrs() -> CanonicalAttrs {
        CanonicalAttrs::default()
    }

    /// `CanonicalAttrs` with a SECRET US classification — used as the
    /// "before" snapshot in mismatched-content tests so the
    /// "after" diverges on the classification field.
    ///
    /// `CanonicalAttrs` is `#[non_exhaustive]` so cross-crate
    /// construction goes through `Default::default()` + field
    /// mutation; the struct-expression form is not callable.
    fn secret_attrs() -> CanonicalAttrs {
        let mut attrs = CanonicalAttrs::default();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
        attrs
    }

    /// Test 1 — equality path returns `Ok(())`.
    ///
    /// Exercises the `before == after` branch of
    /// [`check_portions_unchanged`]. Covers both the empty-slice
    /// case (the typical PageFinalization dispatch shape on a
    /// no-portion page) and a single-portion case where the two
    /// sides are independent clones.
    #[test]
    fn check_portions_unchanged_returns_ok_on_equal_slices() {
        // Empty + empty — the typical no-portion dispatch shape.
        assert!(check_portions_unchanged(&[], &[], 0).is_ok());

        // Single portion, cloned — Vec clone proves the comparison
        // is value-equality, not pointer-equality.
        let portions = vec![secret_attrs()];
        let cloned = portions.clone();
        assert!(check_portions_unchanged(&portions, &cloned, 1).is_ok());
    }

    /// Test 2 — mismatched lengths return `Err`, error string
    /// carries counts + rule_count, and no type/field names appear.
    ///
    /// Exercises the `Err` branch and verifies the format-arg
    /// interpolation lands the three `usize` operands in the
    /// rendered string. The G13 negative assertions guard against
    /// a future format-string edit that re-introduces operand
    /// `Debug` representation.
    #[test]
    fn check_portions_unchanged_returns_err_on_mismatched_lengths() {
        let before = vec![secret_attrs()];
        let after: Vec<CanonicalAttrs> = vec![];

        let err = check_portions_unchanged(&before, &after, 7)
            .expect_err("length mismatch must surface as Err");

        // Counts present.
        assert!(
            err.contains("1 portion(s) before vs 0 after"),
            "expected count phrase in error, got: {err}"
        );
        assert!(
            err.contains("7 rule(s) dispatched"),
            "expected rule_count phrase in error, got: {err}"
        );

        // G13 (Constitution V Principle V): no type names that
        // would imply portion content leakage.
        assert!(
            !err.contains("CanonicalAttrs"),
            "G13 violation: type name `CanonicalAttrs` in error: {err}"
        );
        assert!(
            !err.contains("SciControl"),
            "G13 violation: type name `SciControl` in error: {err}"
        );
        assert!(
            !err.contains("Span"),
            "G13 violation: type name `Span` in error: {err}"
        );
        assert!(
            !err.contains("MarkingClassification"),
            "G13 violation: type name `MarkingClassification` in error: {err}"
        );
        assert!(
            !err.contains("Secret"),
            "G13 violation: classification variant `Secret` in error: {err}"
        );
    }

    /// Test 3 — same-length-but-different-content mismatch returns
    /// `Err`. Distinct from test 2 (which exercises a length
    /// mismatch); this one forces the slice `PartialEq` to walk
    /// into element-by-element comparison before returning `false`.
    #[test]
    fn check_portions_unchanged_returns_err_on_mismatched_content() {
        let before = vec![empty_attrs()];
        let after = vec![secret_attrs()];

        let err = check_portions_unchanged(&before, &after, 1)
            .expect_err("content mismatch must surface as Err");

        // Counts: both sides are length-1, so the count phrasing
        // is symmetric. The error message reports the symmetry
        // truthfully ("1 portion(s) before vs 1 after") — that is
        // the documented limitation of the outer-loop sentinel
        // placement (it cannot attribute which portion mutated).
        assert!(
            err.contains("1 portion(s) before vs 1 after"),
            "expected count phrase in error, got: {err}"
        );
        assert!(
            err.contains("1 rule(s) dispatched"),
            "expected rule_count phrase in error, got: {err}"
        );

        // The doc-cross-reference is the audit trail back to the
        // invariant statement — verify it survives format-arg
        // expansion.
        assert!(
            err.contains("section 3 (e.1)"),
            "expected doc-cross-reference in error, got: {err}"
        );
    }

    /// Test 4 — load-bearing G13 invariant test.
    ///
    /// Constructs a `CanonicalAttrs` with distinctive free-text
    /// content (`classified_by`) — the kind of field that the
    /// retired `debug_assert_eq!` macro would have auto-dumped via
    /// `Debug` formatting on panic per the Copilot round-1 finding
    /// (`core::panicking::assert_failed_inner` formats both
    /// operands as `left: {:?} right: {:?}` regardless of any
    /// custom message). Calls the helper with a mismatch and
    /// asserts the rendered error string does NOT contain the
    /// distinctive content.
    ///
    /// This is the redundant-by-design G13 check that pins the
    /// helper's content-ignorance contract independent of the
    /// type-name negative assertions in test 2 — the failure mode
    /// being guarded against is "a future helper edit pipes
    /// element content through `{:?}`", and the sentinel value
    /// here is what makes that regression detectable.
    #[test]
    fn check_portions_unchanged_error_message_is_g13_compliant() {
        // Distinctive sentinel embedded in a free-text field. If
        // any future edit to `check_portions_unchanged` formats
        // a `CanonicalAttrs` field via `Debug` / `Display`, this
        // string will surface in the rendered error.
        const G13_SENTINEL: &str = "MARQUE-PR-490-G13-CANARY-XYZZY-7F3A1B2C";

        let mut attrs_with_canary = CanonicalAttrs::default();
        attrs_with_canary.classified_by = Some(G13_SENTINEL.into());
        let before = vec![attrs_with_canary];
        let after: Vec<CanonicalAttrs> = vec![];

        let err = check_portions_unchanged(&before, &after, 1)
            .expect_err("mismatch must surface as Err for the G13 check");

        // The load-bearing assertion: the distinctive sentinel
        // string MUST NOT appear anywhere in the rendered error.
        // If this fires, the helper has regressed against the
        // round-1 Copilot finding — operand content is leaking
        // through the panic surface.
        assert!(
            !err.contains(G13_SENTINEL),
            "G13 violation: classified_by content leaked into sentinel \
             error message. Sentinel string `{G13_SENTINEL}` found in \
             rendered error: {err}"
        );

        // Sanity: the helper still rendered a non-empty error
        // (i.e., we didn't accidentally pass the assertion by
        // making the helper a no-op).
        assert!(
            !err.is_empty(),
            "G13 test fixture invalid: helper returned empty error \
             string — the negative assertion above is vacuous."
        );
    }
}
