use super::*;

/// Inline span-containment predicate. Endpoints
/// inclusive on both sides: a fix whose span exactly matches a
/// token's boundaries is still sub-token-shape. Inline because the
/// pass-1 dispatch loop calls this per-fix.
#[inline]
pub(super) fn span_is_within_marking(inner: Span, outer: Span) -> bool {
    inner.start >= outer.start && inner.end <= outer.end
}

/// True when two byte spans overlap (share at least one byte). Used
/// by the overlap demotion to detect pass-2 diagnostics that
/// land on byte ranges already promoted by pass-1.
///
/// The half-open `[start, end)` convention matches the rest of
/// `marque-ism::Span`: spans `(0, 5)` and `(5, 10)` are adjacent but
/// do NOT overlap. Empty spans (`start == end`) never overlap
/// anything by construction.
#[inline]
pub(super) fn spans_overlap(a: Span, b: Span) -> bool {
    a.start < b.end && b.start < a.end
}

/// Apply reshape-aware disambiguation and overlap demotion to a pass-2
/// diagnostic partition. Returns an owned vector of post-adjustment
/// diagnostics.
///
/// Adjustments:
///
/// - **Disambiguation**: a pass-2 diagnostic whose
///   `(rule, candidate_span ?? span)` matches a pass-1 promoted fix is
///   dropped. The same rule already fired on the same marking-scope
///   span; re-emitting it after the reshape would double-fire.
/// - **Overlap demotion**: a pass-2 diagnostic whose
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
/// On this hot path the allocation is one `Vec` with ≤32 `Diagnostic`
/// clones — well below the interactive-latency budget at 10 KB.
pub(super) fn apply_fr023_and_i18(
    pass2_diags: &[&Diagnostic<CapcoScheme>],
    pass1_applied_keys: &HashSet<(RuleId, Span)>,
) -> Vec<Diagnostic<CapcoScheme>> {
    let mut out: Vec<Diagnostic<CapcoScheme>> = Vec::with_capacity(pass2_diags.len());
    for &d in pass2_diags {
        // Drop diagnostics with the same (rule, span) as a
        // pass-1 promoted fix. The candidate_span is the marking-
        // scope anchor — match against it (falling back to `span` for
        // diagnostics that don't carry a candidate span; matches the
        // `apply_kept_fixes` keying convention).
        let key_span = d.candidate_span.unwrap_or(d.span);
        if pass1_applied_keys.contains(&(d.rule, key_span)) {
            continue;
        }

        // Demote diagnostics whose marking-scope span overlaps
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
pub(super) fn find_containing_marking(
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
pub(super) fn lookup_marking(
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

/// Recognition-then-span sort + C-1 dedup walk extracted into a helper
/// so pass-1 and pass-2 share an identical ordering/dedup pipeline. The
/// walks are run independently per pass; the helper exists to factor
/// the algorithm, not the state.
///
/// Sorts `synthesized` **in place** and consumes each kept fix
/// into the result vector. Allocates zero extra `SynthesizedFix`
/// values — no intermediate reference vector, no per-element clone.
pub(super) fn sort_and_c1_dedup(mut synthesized: Vec<SynthesizedFix>) -> Vec<SynthesizedFix> {
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
/// via [`TwoPassFixer::apply_kept_fixes`]. `fixes` MUST be sorted
/// span.end DESC, span.start DESC so `iter().rev()` yields ascending
/// order for the left-to-right walk. Pre-allocates capacity using the
/// per-fix growth contribution (`saturating_sub` upper bound).
///
/// `splice_fixes_forward` names what the function does — a forward
/// splice — so a reader scanning either pass's caller can see the
/// operation without re-reading the body.
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
pub(super) fn splice_fixes_forward(source: &[u8], fixes: &[SynthesizedFix]) -> Vec<u8> {
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
/// lexicographically-smallest rule_id in the group (deterministic
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
/// - `Recognition::combined() < threshold` → excluded.
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
/// copied into the audit record — Constitution V Principle V.
pub(super) fn synthesize_fixes(
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
        // intent's `recognition` axis is the minimum across the group
        // so the audit envelope reflects the weakest signal in the
        // batch.
        //
        // Pre-PR-B this branch scaled within-group `rule` axis. PR B
        // retired the `rule` axis, so the scaling now lands on
        // `recognition` directly — strict-path members carry
        // `recognition = 1.0`, decoder-path members carry sub-1.0
        // posteriors, and the lex-smallest owning intent's
        // `recognition` is overwritten to the minimum so a mixed
        // group's audit record honors the threshold gate against the
        // weaker member.
        group_diags.sort_by_key(|a| a.rule);
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
        if min_combined < combined_intent.confidence.recognition {
            combined_intent.confidence.recognition = min_combined.clamp(0.0, 1.0);
        }

        out.push(SynthesizedFix {
            rule: owning_diag.rule,
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
// Decoder-path diagnostic synthesis
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
/// # Audit-shape contract (Constitution V Principle V)
///
/// The diagnostic's `message` MUST NOT carry verbatim input bytes —
/// only token canonicals, span offsets, and digests/posterior scalars
/// are permitted in audit output. The "before" form is omitted from
/// the message; the span tells the audit consumer *where* the fix
/// landed and the structural `FixIntent` carries *what* shape the
/// recognition became (a `Recanonicalize { scope: RecanonScope::Page }`
/// emission for R001).
///
/// The audit record's `AppliedFix.proposal` carries no document bytes
/// for the decoder path: the `AppliedFixProposal::FixIntent(_)` variant
/// carries the structural intent only. Original document bytes already
/// exist in the source; the audit record is not the right channel for
/// them.
///
/// Note: this contract addresses the audit-record *shape*. A separate
/// upstream concern was whether the canonical-bytes synthesis was
/// well-formed when the decoder accepted unrecognized bytes as a
/// compartment-shaped token and uppercased them — that's a decoder-
/// correctness issue tracked separately; the structural-intent path
/// closes the audit-shape channel by construction.
///
/// The fix's `Recognition` is populated entirely from the decoder's
/// provenance trace:
///
/// - `recognition` derives from `runner_up_ratio` via softmax (see
///   [`DecoderProvenance::recognition_score`]); strictly less than
///   `1.0` so audit consumers can distinguish strict from decoder
///   provenance via a single field comparison. For the position-aware
///   classification heuristic (issue #133) the posterior is further
///   capped at [`HEURISTIC_RECOGNITION_CAP`] so a single-candidate
///   heuristic recognition cannot saturate above the default threshold.
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
pub(super) fn build_decoder_diagnostic(
    span: Span,
    original_bytes: &[u8],
    provenance: &DecoderProvenance,
    _kind: marque_ism::MarkingType,
    corpus_override_active: bool,
) -> Option<Diagnostic<CapcoScheme>> {
    use marque_rules::recognition::{FeatureContribution, FeatureId};

    let original = std::str::from_utf8(original_bytes).ok()?;
    let replacement = std::str::from_utf8(&provenance.canonical_bytes).ok()?;

    // No-op rewrite (canonicalization preserved bytes byte-for-byte) is
    // not informative and would produce a degenerate audit record; skip.
    if original == replacement {
        return None;
    }

    // `provenance.features` is a `Box<[FeatureContribution]>`; copy into
    // a `SmallVec<[…; 4]>` matching `Recognition::features` so the inline-4
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
    // recognition emits at `Severity::Fix` with the decoder's full
    // posterior on `recognition` (engine applies whenever
    // `recognition >= confidence_threshold`). The position-aware
    // classification heuristic (issue #133) emits at `Severity::Warn`
    // (always-visible in `--check`, non-zero exit code) with
    // `recognition` capped at [`HEURISTIC_RECOGNITION_CAP = 0.95`] —
    // matching the default `confidence_threshold` so a single-candidate
    // heuristic fix lands at-threshold rather than saturating above
    // it. The `0.95` value is justified by empirical corpus measurement
    // — see the cap's doc comment for the analysis script and measured
    // numbers.
    //
    // Pre-PR-B the cap lived on the `rule` axis; PR B retired that
    // axis and the cap moved onto `recognition` directly.
    let raw_recognition = provenance.recognition_score();
    let (severity, recognition, fix_source) = match provenance.fix_source {
        FixSource::DecoderClassificationHeuristic => (
            Severity::Warn,
            raw_recognition.min(HEURISTIC_RECOGNITION_CAP),
            FixSource::DecoderClassificationHeuristic,
        ),
        // All non-heuristic decoder paths use the existing posterior
        // shape. Strict-source variants (BuiltinRule, CorrectionsMap,
        // MigrationTable) do not flow through this builder — they
        // come from rule-pipeline emissions, not the decoder — so
        // routing them to `DecoderPosterior` here is a defensive
        // default that preserves the existing strict-decoder shape
        // for any future fix-source variant.
        _ => (Severity::Fix, raw_recognition, FixSource::DecoderPosterior),
    };

    let confidence = Recognition {
        recognition,
        runner_up_ratio: provenance.runner_up_ratio,
        features,
    };
    // DECODER_RULE_ID is a `RuleId`, so the dispatcher hands it through
    // directly — no `RuleId::new` wrapping needed. The `Copy` bound
    // on the 2-tuple `RuleId` makes the let-binding free.
    let rule = DECODER_RULE_ID;
    // Audit-shape contract: the decoder-path engine-minted record
    // carries no document bytes (Constitution V Principle V). The
    // span identifies *where* the fix landed; the engine's synthesis
    // path re-renders the canonical form from a `Recanonicalize` intent
    // at promotion time.
    //
    // Issue #699: the lint-side `Diagnostic.recognized_canonical` field
    // DOES carry the canonical bytes so user-facing renderers can show
    // the recognized form in `check` output without running `fix`. The
    // asymmetry is intentional and pinned by
    // `lint_carries_recognized_canonical_fix_audit_does_not` — lint
    // shows the bytes; the audit envelope continues to carry only the
    // BLAKE3 digest + structural intent.
    //
    // The `original` / `replacement` bindings above served the
    // UTF-8-validity and no-op-rewrite gates only — both have already
    // run by this point. The canonical bytes feeding
    // `recognized_canonical` come directly from
    // `provenance.canonical_bytes`. The wrapper is `SecretSlice<u8>`
    // (alias for `SecretBox<[u8]>`), the same content-bearing type
    // backing `FixResult.source`. Constitution II — the secret wipes
    // on drop; every readout goes through `expose_secret()`.
    let _ = (original, replacement);
    let recognized_canonical = Some(secrecy::SecretBox::new(Box::from(
        provenance.canonical_bytes.as_ref(),
    )));
    use marque_scheme::{ReplacementIntent, fix_intent::RecanonScope};
    let intent = FixIntent::<CapcoScheme> {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
        },
        confidence,
        feature_ids: SmallVec::new(),
        message: marque_rules::Message::new(
            marque_rules::MessageTemplate::DecoderRecognized,
            marque_rules::MessageArgs::default(),
        ),
        source: fix_source,
        migration_ref: None,
    };
    Some(
        Diagnostic::with_fix_at_span(
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
        )
        .with_recognized_canonical(recognized_canonical),
    )
}

/// Build the synthetic `R002 reparse-failed` diagnostic the engine
/// emits when the post-pass-1 buffer cannot be re-parsed.
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
/// # Audit-content-ignorance (Constitution V Principle V)
///
/// The diagnostic carries:
/// - [`R002_RULE_ID`] (permitted identifier)
/// - [`Span`] (permitted identifier — byte offsets only)
/// - A closed-template message with the contributing rule IDs as a
///   structured field (permitted identifiers — predicate IDs from a
///   closed vocabulary)
///
/// No document bytes flow through R002.
///
/// # Wire-up to `MessageArgs`
///
/// The structured `MessageArgs.contributing_rule_ids` field is plumbed
/// at the type level — the closed-set destructure-pin test at
/// `crates/rules/tests/message_args_closed_set.rs` enforces its
/// presence. The R002 `Diagnostic` carries the contributing rule
/// IDs as a typed `SmallVec<[RuleId; 4]>` field on `MessageArgs`, so
/// this function constructs
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
pub(super) fn build_r002_diagnostic(
    contributing_rule_ids: SmallVec<[RuleId; 4]>,
    failure_span: Span,
) -> Diagnostic<CapcoScheme> {
    // Typed `Message` per `MessageTemplate::ReparseFailed`.
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

/// Cap applied to `recognition` for the position-aware classification
/// heuristic (issue #133) — pinned at the default
/// `confidence_threshold` (0.95) so a solo heuristic candidate lands
/// at-threshold rather than saturating above it. Pre-PR-B this cap
/// lived on the (now-retired) `Recognition::rule` axis as
/// `HEURISTIC_RULE_AXIS_CAP`; PR B collapsed the two axes into one and
/// the cap moved onto `recognition` directly. The empirical corpus
/// measurement justifying the `0.95` value (≥99.4% confidence per
/// trigger) is unchanged.
pub(super) const HEURISTIC_RECOGNITION_CAP: f32 = 0.95;
