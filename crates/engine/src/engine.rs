// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Engine` — the configured, ready-to-run pipeline.

use crate::clock::{Clock, SystemClock};
use crate::errors::{EngineConstructionError, EngineError};
use crate::options::{FixOptions, LintOptions};
use crate::output::{FixResult, LintResult};
use crate::recognizer::{StrictRecognizer, shift_token_spans};
use crate::scheduler::schedule_rewrites;
use aho_corasick::AhoCorasick;
use marque_capco::CapcoScheme;
use marque_capco::provenance::DecoderProvenance;
use marque_config::Config;
use marque_ism::Span;
use marque_rules::{
    AppliedFix, CORRECTIONS_MAP_CITATION, Confidence, Diagnostic, EnginePromotionToken,
    FixProposal, FixSource, RuleId, RuleSet, Severity,
};
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{ParseContext, Recognizer};
use marque_scheme::{MarkingScheme, RewriteId};
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

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
    rule_sets: Vec<Box<dyn RuleSet>>,
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
    /// Strict-path recognizer used by `lint()` to resolve each scanner
    /// candidate to an `IsmAttributes`. Held behind `Arc<dyn Recognizer>`
    /// so Phase 4 PR-3 can swap in a `DecoderRecognizer` or combined
    /// strict→decoder dispatcher at construction time without touching
    /// the lint loop. Shared across threads unchanged — the recognizer
    /// trait is `Send + Sync` and `BatchEngine` workers hold the same
    /// `Arc` reference (Constitution VI, FR-023).
    recognizer: Arc<dyn Recognizer<CapcoScheme>>,
    /// When `true`, `lint()` passes `strict_evidence = false` in the
    /// `ParseContext` so the recognizer is allowed to fall back to the
    /// decoder on strict-parse zero-candidate. Flipped by
    /// [`Engine::with_deep_scan`]; false by default so interactive-
    /// authoring latency (SC-001) is identical to PR-2.
    deep_scan: bool,

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
        rule_sets: Vec<Box<dyn RuleSet>>,
        scheme: S,
    ) -> Result<Self, EngineConstructionError> {
        Self::with_clock(config, rule_sets, scheme, Box::new(SystemClock))
    }

    /// Create an engine with a custom clock (for deterministic tests).
    pub fn with_clock<S: MarkingScheme>(
        mut config: Config,
        rule_sets: Vec<Box<dyn RuleSet>>,
        scheme: S,
        clock: Box<dyn Clock>,
    ) -> Result<Self, EngineConstructionError> {
        // Canonicalize [rules] overrides against the registered rule
        // set: accept either the rule ID (e.g. "E001") or the rule
        // name (e.g. "portion-mark-in-banner"), resolve both to the
        // canonical ID before the engine stores the map, and hard-fail
        // on any unknown key. See `canonicalize_rule_overrides`.
        canonicalize_rule_overrides(&mut config, &rule_sets)?;

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

        Ok(Self {
            config,
            rule_sets,
            clock,
            corrections_arc,
            corrections_ac,
            scheduled_rewrites,
            recognizer: Arc::new(StrictRecognizer::new()),
            deep_scan: false,
            #[cfg(feature = "corpus-override")]
            corpus_override: None,
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

    /// Swap the engine's strict-only recognizer for the strict-then-
    /// decoder dispatcher (Phase 4 PR-3). Returns the engine by value
    /// so callers can chain:
    ///
    /// ```ignore
    /// let engine = Engine::new(config, rules, scheme)?.with_deep_scan();
    /// ```
    ///
    /// With deep-scan on, [`Engine::lint`] first tries the strict path
    /// for every scanner candidate; if strict returns a zero-candidate
    /// `Ambiguous`, the engine falls back to [`DecoderRecognizer`].
    /// The decoder recovers mangled markings that are edit-distance-1/2,
    /// token-reordered, superseded, or case-mangled from a real
    /// CAPCO-2016 marking.
    ///
    /// Interactive-authoring latency is not affected: without
    /// deep-scan, the engine's dispatch is identical to Phase 4 PR-2.
    /// The decoder only fires on explicit opt-in (this method today;
    /// a `--deep-scan` CLI flag lands in PR-4 alongside audit v2).
    #[must_use = "with_deep_scan returns a new Engine; the result must be bound to take effect — `engine.with_deep_scan()` alone leaves the engine in strict-only mode"]
    pub fn with_deep_scan(mut self) -> Self {
        self.recognizer = Arc::new(crate::decoder::StrictOrDecoderRecognizer::new());
        self.deep_scan = true;
        self
    }

    /// Whether this engine is running in deep-scan mode.
    ///
    /// Exposed for test inspection and for `BatchEngine` /
    /// `marque-server` code paths that need to mirror the engine's
    /// mode onto their own configuration (e.g., audit records'
    /// `FixSource::DecoderPosterior` only makes sense in deep-scan).
    pub fn deep_scan_enabled(&self) -> bool {
        self.deep_scan
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
    /// Phase 1 implementation: the body is identical to the legacy
    /// [`Engine::lint`] codepath — `opts.deadline` is **ignored** so
    /// no observable behavior change ships in Phase 1. Phase 2
    /// (tasks T007–T009) wires the cooperative-cancellation checks
    /// against `opts.deadline`. The signature lands now so the
    /// surface wiring (CLI / server / WASM / batch in Phase 3) can
    /// land in parallel against a stable type surface.
    pub fn lint_with_options(&self, source: &[u8], opts: &LintOptions) -> LintResult {
        // Phase 1: deadline is plumbed but not honored. The bind here
        // documents the field exists and silences unused-variable
        // warnings without `#[allow(unused)]`.
        let _ = opts.deadline;

        use marque_core::Scanner;
        use marque_ism::{MarkingType, PageContext};
        use marque_rules::RuleContext;

        let candidates = Scanner::scan(source);

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

        for candidate in &candidates {
            // Page-break candidates are scanner-emitted boundaries with no
            // parsable content. Reset the context BEFORE attempting to parse
            // — otherwise the parser's MalformedMarking error would skip the
            // continue and leave us accumulating across pages.
            if candidate.kind == MarkingType::PageBreak {
                page_context = PageContext::new();
                page_context_arc = None;
                classification_floor = None;
                continue;
            }

            // Parse context built per-candidate so the floor accumulated
            // earlier on the page reaches the recognizer. `strict_evidence`
            // is the engine-level deep-scan opt-in mirror; the dispatcher
            // (StrictOrDecoderRecognizer) reads it to decide whether to
            // fall back to the decoder on strict-parse zero-candidate.
            let parse_cx = ParseContext {
                strict_evidence: !self.deep_scan,
                zone: None,
                position: None,
                classification_floor,
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
            // Capture the decoder-provenance side channel before
            // collapsing the marking onto its `IsmAttributes` payload.
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
                page_context: ctx_page,
                corrections: corrections_arc.clone(),
            };
            for rule_set in &self.rule_sets {
                for rule in rule_set.rules() {
                    // Skip rules that are configured as Off.
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
                    // Apply configured severity override.
                    for d in &mut diags {
                        d.severity = configured_severity;
                    }
                    diagnostics.extend(diags);
                }
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
                        let proposal = FixProposal::new(
                            RuleId::new("C001"),
                            FixSource::CorrectionsMap,
                            span,
                            key.as_ref(),
                            value.as_ref(),
                            marque_rules::Confidence::strict(1.0),
                            None,
                        );
                        diagnostics.push(Diagnostic::new(
                            RuleId::new("C001"),
                            c001_severity,
                            span,
                            format!("corrections map: {key:?} → {value:?}"),
                            CORRECTIONS_MAP_CITATION,
                            Some(proposal),
                        ));
                    }
                }
            }
        }

        LintResult {
            diagnostics,
            ..Default::default()
        }
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
    /// Phase 1 implementation: `opts.deadline` is **ignored**; the
    /// body delegates to the existing fix path with
    /// `opts.threshold_override`. Phase 2 (tasks T010–T012) wires
    /// cooperative cancellation against `opts.deadline`, returning
    /// `Err(EngineError::DeadlineExceeded { partial_lint })` per spec
    /// §R4 (asymmetric response).
    ///
    /// The threshold override is honored from day one because it is
    /// not a deadline concern — it fits naturally into the new
    /// options struct and lifting it now lets callers stop reaching
    /// for `fix_with_threshold` immediately.
    pub fn fix_with_options(
        &self,
        source: &[u8],
        mode: FixMode,
        opts: &FixOptions,
    ) -> Result<FixResult, EngineError> {
        // Phase 1: deadline is plumbed but not honored.
        let _ = opts.deadline;

        let threshold = match opts.threshold_override {
            Some(value) => {
                if !(0.0..=1.0).contains(&value) || value.is_nan() {
                    return Err(EngineError::InvalidThreshold(InvalidThreshold(value)));
                }
                value
            }
            None => self.config.confidence_threshold(),
        };

        Ok(self.fix_inner(source, mode, threshold))
    }

    fn fix_inner(&self, source: &[u8], mode: FixMode, threshold: f32) -> FixResult {
        use std::collections::HashSet;

        // Two-pass fix strategy for pre-scanner text corrections.
        //
        // Pass 1: lint the original source. The pre-scanner text scan may
        // produce C001 diagnostics for corrections-map matches the scanner
        // missed (e.g., "SERCET" is not a known classification prefix).
        // Apply those C001 fixes to produce an intermediate source.
        //
        // Pass 2: re-lint the intermediate source. The scanner now detects
        // the corrected marking (e.g., "SECRET//NF") and additional rules
        // fire (e.g., E001 on NF→NOFORN). Apply those fixes on top.
        //
        // Without this, the spec scenario "SERCET//NF → SECRET//NOFORN"
        // would stop at "SECRET//NF".
        let lint1 = self.lint(source);
        let (effective_source, pass1_applied) =
            self.apply_text_corrections(source, &lint1, threshold, mode);

        let lint = if !pass1_applied.is_empty() {
            // Re-lint the corrected source so the scanner picks up newly-valid markings.
            self.lint(&effective_source)
        } else {
            lint1
        };

        let mut fixes: Vec<_> = lint
            .diagnostics
            .iter()
            .filter_map(|d| d.fix.as_ref())
            .filter(|f| f.confidence.combined() >= threshold)
            .filter(|f| !f.span.is_empty())
            .collect();

        // FR-016: deterministic total-order fix application.
        // Sort by (span.end DESC, span.start DESC, rule_id ASC, replacement ASC).
        fixes.sort_by(|a, b| {
            b.span
                .end
                .cmp(&a.span.end)
                .then(b.span.start.cmp(&a.span.start))
                .then(a.rule.cmp(&b.rule))
                .then(a.replacement.cmp(&b.replacement))
        });

        // C-1: overlap guard. After the FR-016 sort, two fixes can still
        // touch the same byte range if multiple rules emit a fix for the
        // same span (or overlapping spans). Applying both via `splice`
        // would silently corrupt the byte stream. We keep the first fix
        // per span (which under FR-016 ordering is deterministic) and
        // surface the dropped fixes through `remaining_diagnostics`.
        //
        // The walk is over fixes in reverse-end order, so a fix is kept
        // only if its `span.end` is at or below the previous kept fix's
        // `span.start` — i.e., strictly to the left, no overlap.
        let mut kept_fixes: Vec<&marque_rules::FixProposal> = Vec::with_capacity(fixes.len());
        let mut next_window_end: Option<usize> = None;
        for fix in &fixes {
            let fits = match next_window_end {
                Some(boundary) => fix.span.end <= boundary,
                None => true,
            };
            if fits {
                next_window_end = Some(fix.span.start);
                kept_fixes.push(*fix);
            }
        }

        // M-4: hold the classifier id in an `Arc<str>` so cloning into each
        // applied-fix audit record is an O(1) refcount bump rather than a
        // full string copy per fix.
        let classifier_id: Option<std::sync::Arc<str>> = self
            .config
            .user
            .classifier_id
            .as_deref()
            .map(std::sync::Arc::from);
        let dry_run = mode == FixMode::DryRun;
        let now = self.clock.now();

        // H-7: applied-fix lookup is keyed by (RuleId, Span). Use a HashSet
        // so the per-diagnostic filter at the bottom of this function is
        // O(1) per query instead of O(n) over a Vec.
        let mut applied_keys: HashSet<(RuleId, Span)> = HashSet::with_capacity(kept_fixes.len());
        let mut applied: Vec<AppliedFix> = Vec::with_capacity(kept_fixes.len());

        // Only allocate the output buffer when we actually need to mutate it.
        // Dry-run returns the original source verbatim.
        let output = match mode {
            FixMode::Apply => {
                let mut buf = effective_source.clone();
                for fix in kept_fixes {
                    buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());
                    applied_keys.insert((fix.rule.clone(), fix.span));
                    applied.push(AppliedFix::__engine_promote(
                        fix.clone(),
                        now,
                        classifier_id.clone(),
                        dry_run,
                        None, // input identifier set by CLI at the boundary
                        engine_promotion_token(),
                    ));
                }
                buf
            }
            FixMode::DryRun => {
                for fix in kept_fixes {
                    applied_keys.insert((fix.rule.clone(), fix.span));
                    applied.push(AppliedFix::__engine_promote(
                        fix.clone(),
                        now,
                        classifier_id.clone(),
                        dry_run,
                        None,
                        engine_promotion_token(),
                    ));
                }
                source.to_vec()
            }
        };

        // Prepend pass-1 text corrections to the applied list so they
        // appear in the audit trail.
        let mut all_applied = pass1_applied;
        all_applied.extend(applied);

        // Remaining diagnostics: those whose fix was not applied.
        // Filter by (rule_id, span) pair — not just rule ID — so that if
        // rule E001 fires on three spans and only one is fixed, the other
        // two remain.
        let remaining_diagnostics = lint
            .diagnostics
            .into_iter()
            .filter(|d| {
                !d.fix
                    .as_ref()
                    .is_some_and(|f| applied_keys.contains(&(f.rule.clone(), f.span)))
            })
            .collect();

        FixResult {
            source: output,
            applied: all_applied,
            remaining_diagnostics,
        }
    }

    /// Apply pre-scanner text corrections (C001) from lint diagnostics and
    /// return the corrected source + applied fixes. Used by `fix_inner` to
    /// produce an intermediate source that the scanner can detect.
    fn apply_text_corrections(
        &self,
        source: &[u8],
        lint: &LintResult,
        threshold: f32,
        mode: FixMode,
    ) -> (Vec<u8>, Vec<AppliedFix>) {
        let mut text_fixes: Vec<&FixProposal> = lint
            .diagnostics
            .iter()
            .filter(|d| d.rule.as_str() == "C001")
            .filter_map(|d| d.fix.as_ref())
            .filter(|f| f.source == FixSource::CorrectionsMap)
            .filter(|f| f.confidence.combined() >= threshold)
            .filter(|f| !f.span.is_empty())
            .collect();

        if text_fixes.is_empty() {
            return (source.to_vec(), Vec::new());
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
        let mut kept: Vec<&FixProposal> = Vec::new();
        let mut next_end: Option<usize> = None;
        for fix in &text_fixes {
            let fits = next_end.is_none_or(|b| fix.span.end <= b);
            if fits {
                next_end = Some(fix.span.start);
                kept.push(*fix);
            }
        }

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
        for fix in &kept {
            buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());
            applied.push(AppliedFix::__engine_promote(
                (*fix).clone(),
                now,
                classifier_id.clone(),
                dry_run,
                None,
                engine_promotion_token(),
            ));
        }

        (buf, applied)
    }
}

// ---------------------------------------------------------------------------
// Engine-only AppliedFix promotion gate (Constitution V Principle V)
// ---------------------------------------------------------------------------

/// Mint an [`EnginePromotionToken`] for [`AppliedFix::__engine_promote`].
///
/// This is the **single** place inside `marque-engine` where the engine
/// grants itself the privilege to promote a `FixProposal` to an
/// `AppliedFix`. Constitution V Principle V scopes audit-record
/// promotion to `Engine::fix_inner` and `Engine::apply_text_corrections`
/// (the three production call sites in this file). Centralizing the
/// token construction here makes "where does the engine decide to
/// promote?" a one-grep question, and means a future refactor that
/// adds a fourth promotion site has to thread through this function
/// — a deliberate decision, not an accident.
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
) -> Option<Diagnostic> {
    use marque_rules::confidence::{FeatureContribution, FeatureId};

    let original = std::str::from_utf8(original_bytes).ok()?;
    let replacement = std::str::from_utf8(&provenance.canonical_bytes).ok()?;

    // No-op rewrite (canonicalization preserved bytes byte-for-byte) is
    // not informative and would produce a degenerate audit record; skip.
    if original == replacement {
        return None;
    }

    let mut features: Vec<FeatureContribution> = provenance.features.to_vec();
    if corpus_override_active {
        features.push(FeatureContribution {
            id: FeatureId::CorpusOverrideInEffect,
            delta: 0.0,
        });
    }

    let confidence = Confidence {
        recognition: provenance.recognition_score(),
        rule: 1.0,
        region: None,
        runner_up_ratio: provenance.runner_up_ratio,
        features,
    };
    let rule = RuleId::new(DECODER_RULE_ID);
    let proposal = FixProposal::new(
        rule.clone(),
        FixSource::DecoderPosterior,
        span,
        original,
        replacement,
        confidence,
        None,
    );
    Some(Diagnostic::new(
        rule,
        // `Severity::Fix` so the engine's normal fix gate applies the
        // proposal in `--fix` mode and surfaces it in `--check` mode.
        Severity::Fix,
        span,
        format!("decoder-recognized canonical form: {original:?} → {replacement:?}"),
        DECODER_CITATION,
        Some(proposal),
    ))
}

// ---------------------------------------------------------------------------
// Rule-override canonicalization (task #49)
// ---------------------------------------------------------------------------

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
    rule_sets: &[Box<dyn RuleSet>],
) -> Result<(), EngineConstructionError> {
    if config.rules.overrides.is_empty() {
        return Ok(());
    }

    // Build the ID-and-name → canonical-ID lookup. Both sides live in
    // `&'static str` (RuleId's inner slice, rule.name()), so the map's
    // keys and values are all `'static`.
    let mut known: HashMap<&'static str, &'static str> = HashMap::new();
    for rule_set in rule_sets {
        for rule in rule_set.rules() {
            let id_str = rule.id().as_str();
            let name = rule.name();
            known.insert(id_str, id_str);
            known.insert(name, id_str);
        }
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
    use marque_ism::IsmAttributes;
    use marque_rules::{
        Diagnostic, FixProposal, FixSource, Rule, RuleContext, RuleId, RuleSet, Severity,
    };
    use std::time::{Duration, UNIX_EPOCH};

    /// A test rule that emits a fixed list of FixProposals on every check call,
    /// ignoring the parsed attributes. Lets us drive the engine deterministically
    /// without depending on real CAPCO rule output.
    struct StubRule {
        id: &'static str,
        proposals: Vec<FixProposal>,
    }

    impl Rule for StubRule {
        fn id(&self) -> RuleId {
            RuleId::new(self.id)
        }
        fn name(&self) -> &'static str {
            "stub"
        }
        fn default_severity(&self) -> Severity {
            Severity::Fix
        }
        fn check(&self, _attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
            self.proposals
                .iter()
                .map(|p| {
                    Diagnostic::new(
                        p.rule.clone(),
                        Severity::Fix,
                        p.span,
                        "stub",
                        "TEST",
                        Some(p.clone()),
                    )
                })
                .collect()
        }
    }

    struct StubSet(Vec<Box<dyn Rule>>);
    impl RuleSet for StubSet {
        fn rules(&self) -> &[Box<dyn Rule>] {
            &self.0
        }
        fn schema_version(&self) -> &'static str {
            "TEST"
        }
    }

    fn proposal(rule: &'static str, start: usize, end: usize, replacement: &str) -> FixProposal {
        proposal_with_confidence(rule, start, end, replacement, 1.0)
    }

    fn proposal_with_confidence(
        rule: &'static str,
        start: usize,
        end: usize,
        replacement: &str,
        confidence: f32,
    ) -> FixProposal {
        FixProposal::new(
            RuleId::new(rule),
            FixSource::BuiltinRule,
            Span::new(start, end),
            "x",
            replacement,
            marque_rules::Confidence::strict(confidence),
            None,
        )
    }

    fn engine_with(proposals: Vec<FixProposal>) -> Engine {
        engine_with_config(Config::default(), proposals)
    }

    fn engine_with_config(config: Config, proposals: Vec<FixProposal>) -> Engine {
        let stub = StubRule {
            id: "TEST",
            proposals,
        };
        let set: Box<dyn RuleSet> = Box::new(StubSet(vec![Box::new(stub)]));
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
        assert_eq!(result.applied[0].proposal.rule.as_str(), "E002");
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

    impl Rule for ContextRecorderRule {
        fn id(&self) -> RuleId {
            RuleId::new("RECORD")
        }
        fn name(&self) -> &'static str {
            "page-context-recorder"
        }
        fn default_severity(&self) -> Severity {
            Severity::Warn
        }
        fn check(&self, _attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
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

    struct RecorderSet(Vec<Box<dyn Rule>>);
    impl RuleSet for RecorderSet {
        fn rules(&self) -> &[Box<dyn Rule>] {
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
        let set: Box<dyn RuleSet> = Box::new(RecorderSet(vec![Box::new(rule)]));
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
        let set: Box<dyn RuleSet> = Box::new(RecorderSet(vec![Box::new(rule)]));
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
        // Two proposals for span 0..6 with different rule IDs.
        // "C001" < "E001" lexicographically, so C001 is kept and E001 dropped.
        let engine = engine_with(vec![
            proposal("E001", 0, 6, "BB"),
            proposal("C001", 0, 6, "AA"),
        ]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.applied[0].proposal.rule.as_str(), "C001");
        assert_eq!(result.applied[0].proposal.replacement.as_ref(), "AA");
    }

    // FR-016 tiebreaker — same span, same rule ID, different replacements.
    #[test]
    fn fr016_same_span_same_rule_picks_lower_replacement() {
        let engine = engine_with(vec![
            proposal("E001", 0, 6, "ZZZ"),
            proposal("E001", 0, 6, "AAA"),
        ]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.applied[0].proposal.replacement.as_ref(), "AAA");
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

    impl Rule for NamedStub {
        fn id(&self) -> RuleId {
            RuleId::new(self.id)
        }
        fn name(&self) -> &'static str {
            self.name
        }
        fn default_severity(&self) -> Severity {
            Severity::Warn
        }
        fn check(&self, _attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
            vec![]
        }
    }

    fn named_rule_set(rules: &[(&'static str, &'static str)]) -> Box<dyn RuleSet> {
        let rules: Vec<Box<dyn Rule>> = rules
            .iter()
            .map(|(id, name)| Box::new(NamedStub { id, name }) as Box<dyn Rule>)
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
        canonicalize_rule_overrides(&mut config, &sets).expect("should succeed");
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
        canonicalize_rule_overrides(&mut config, &sets).expect("should succeed");
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
        let err = canonicalize_rule_overrides(&mut config, &sets).unwrap_err();
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
        let err = canonicalize_rule_overrides(&mut config, &sets).unwrap_err();
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
        let err = canonicalize_rule_overrides(&mut config, &sets).unwrap_err();
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
        canonicalize_rule_overrides(&mut config, &sets)
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
        canonicalize_rule_overrides(&mut config, &sets).expect("should succeed");
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
        canonicalize_rule_overrides(&mut config, &sets).expect("empty overrides must succeed");
        assert!(config.rules.overrides.is_empty());
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
}
