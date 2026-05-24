//! [`DecoderRecognizer`] — the Phase-D probabilistic recognizer.
//!
//! The hot loop: take a byte slice, generate canonical-byte attempts
//! via [`super::candidates::generate_candidate_bytes`], strict-parse
//! each, score with [`super::scoring::score_candidate`], gate against
//! the prose null hypothesis from [`super::null_hypothesis`], and
//! emit either `Parsed::Unambiguous(top)` or `Parsed::Ambiguous { … }`.

use marque_capco::provenance::DecoderProvenance;
use marque_capco::{CapcoMarking, CapcoScheme};
use marque_core::Parser;
use marque_ism::{
    CapcoTokenSet,
    span::{MarkingCandidate, MarkingType, Span},
};
use marque_rules::confidence::FeatureContribution;
use marque_scheme::MarkingScheme;
use marque_scheme::ambiguity::{Candidate, Parsed};
use marque_scheme::recognizer::{ParseContext, Recognizer};
use smallvec::SmallVec;

use crate::recognizer::is_us_restricted;

use super::candidates::generate_candidate_bytes;
use super::null_hypothesis::{compute_context_features, observed_prose_log_prior};
use super::recovery::meets_classification_floor;
use super::scoring::score_candidate;
use super::shape::{
    has_double_slash, infer_marking_type, is_bare_classification_shape,
    is_fast_path_candidate_shape, is_nontrivial_marking, is_single_letter_portion,
    try_fast_parse_us_class_and_dissem,
};
use super::types::{FeatureEntry, ScoredCandidate, feature_entry_to_evidence};
use super::{K_MAX_CANDIDATES, NULL_HYPOTHESIS_LOG_MARGIN, UNAMBIGUOUS_LOG_MARGIN};

/// Phase-D probabilistic marking recognizer.
///
/// Stateless — all priors are baked `&'static` tables consumed at
/// scoring time. Cheaply constructible; the engine holds a single
/// instance behind `Arc` for the lifetime of one [`crate::Engine`].
///
/// When `ParseContext::strict_evidence == true` the decoder defers to
/// the strict path by returning a zero-candidate result. The engine
/// is responsible for calling the strict recognizer first and only
/// invoking the decoder on deep-scan regions (see
/// `crate::Engine::lint` dispatch).
#[derive(Debug, Default, Clone, Copy)]
pub struct DecoderRecognizer;

impl DecoderRecognizer {
    /// Construct a decoder recognizer.
    pub const fn new() -> Self {
        Self
    }
}

impl Recognizer<CapcoScheme> for DecoderRecognizer {
    fn recognize(
        &self,
        bytes: &[u8],
        offset: usize,
        scheme: &CapcoScheme,
        cx: &ParseContext,
    ) -> Parsed<CapcoMarking> {
        // Strict-path callers get zero candidates so the engine's
        // strict recognizer remains the authoritative answer under
        // interactive-authoring latency. The engine only
        // invokes the decoder when `strict_evidence = false` is
        // explicitly requested (deep-scan mode or rule-escalated
        // region).
        if cx.strict_evidence {
            return Parsed::Ambiguous {
                candidates: Vec::new(),
            };
        }

        let Some(kind) = infer_marking_type(bytes) else {
            return Parsed::Ambiguous {
                candidates: Vec::new(),
            };
        };

        // Prose-glue suppression: a single-letter portion candidate
        // (`(s)`, `(c)`, `(u)`, `(r)`, …) immediately glued to a
        // preceding word — `letter(s)`, `function(c)`, `loss(s)` —
        // is overwhelmingly a plural-suffix or function-call-shaped
        // prose glyph, not a real CAPCO marking. The strict recognizer
        // doesn't have the surrounding-byte context to tell these
        // apart; the engine populates `cx.preceded_by_whitespace`
        // from the source byte preceding the candidate's span and
        // hands it to the decoder so this fallback path doesn't
        // resurrect the false positive that the strict path would
        // never have produced (the strict parser is case-sensitive
        // and rejects lowercase tokens, so `(s)` only reaches the
        // decoder via the case-fold canonicalization).
        //
        // Bullets and numbered-list markers are not a problem — they
        // always have whitespace between the bullet and the marking
        // (`1. (S)`, `* (S//NF)`, `(a) (S)` all set
        // `preceded_by_whitespace = true`).
        //
        // **Interaction with the post-#472 null gate.** Both filters
        // remain independently load-bearing — they are not
        // duplicates:
        //
        // - This prose-glue early-return suppresses BEFORE scoring,
        //   keyed on `!cx.preceded_by_whitespace` (a positional
        //   signal the null gate cannot see). The null gate keys on
        //   token-vs-prose log-prior deltas and shape predicates.
        // - For `(s)` glued to a word, both filters would suppress.
        //   For `(u)` glued to a word, ONLY this filter suppresses —
        //   the `U`-token marking-y delta (`+2.86` per the
        //   [`NULL_HYPOTHESIS_LOG_MARGIN`] doc) exceeds the
        //   `+2.5` margin, so an isolated `(u)` recovers (the
        //   `decoder_residual_gap_isolated_u_recovers_to_unclassified`
        //   test pins that). The prose-glue early-return is what
        //   prevents `function(u)` mid-prose from reaching that
        //   recovery path.
        // - The prose-glue check also short-circuits scoring,
        //   canonicalization, and strict-parse work for the common
        //   `function(s)` / `loss(c)` cases. Removing it would
        //   force every such candidate through the full pipeline
        //   before the null gate caught most of them.
        //
        // Test pin: `decoder_prose_glue_suppresses_u_that_null_gate_would_admit`
        // demonstrates the independence by constructing an `(u)`
        // input that the null gate alone admits and showing the
        // prose-glue early-return suppresses it.
        if !cx.preceded_by_whitespace
            && matches!(kind, MarkingType::Portion)
            && is_single_letter_portion(bytes)
        {
            return Parsed::Ambiguous {
                candidates: Vec::new(),
            };
        }

        // 1. Canonicalize the observed bytes into zero-or-more
        //    candidate byte-strings + per-candidate feature trace.
        let canonical_attempts = generate_candidate_bytes(bytes, kind);
        if canonical_attempts.is_empty() {
            return Parsed::Ambiguous {
                candidates: Vec::new(),
            };
        }

        // 2. Strict-parse each canonicalized attempt. Anything that
        //    fails strict parsing is discarded — the strict parser is
        //    the arbiter of "is this a CAPCO-shape marking." This is
        //    what guarantees the decoder never fabricates a marking
        //    shape the grammar forbids.
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let synthetic_candidate = MarkingCandidate {
            span: Span::new(0, 0), // re-set per attempt below
            kind,
        };
        // Inline-4: the pre-truncate accumulator is bounded above by
        // `canonical_attempts.len() <= K_MAX_CANDIDATES * 2 = 16`, then
        // sorted and truncated to `K_MAX_CANDIDATES = 8` at line ~423.
        // Typical decoder runs see 1-4 viable scored candidates after
        // the strict-parse and finite-posterior filters; the inline
        // budget covers the common case while spillover handles the
        // pre-truncate tail without inflating the stack frame
        // (ScoredCandidate is ~200 bytes — 4 inline ≈ 800 B).
        let mut scored: SmallVec<[ScoredCandidate; 4]> = SmallVec::new();
        for attempt in canonical_attempts {
            let parse_with_strict = || -> Option<CapcoMarking> {
                let candidate = MarkingCandidate {
                    span: Span::new(0, attempt.bytes.len()),
                    ..synthetic_candidate
                };
                let Ok(parsed) = parser.parse(&candidate, &attempt.bytes) else {
                    return None;
                };

                // 3a. Reject partial canonicalizations. Any
                //     `TokenKind::Unknown` span surviving strict parse of
                //     the canonicalized bytes means the decoder passed an
                //     uncorrectable token through unchanged (see Case 4
                //     in `fuzzy_correct_tokens`). Accepting such a
                //     candidate would silently drop the unknown token
                //     from `token_spans` in step 3b and fabricate a
                //     partial marking — e.g., `(SECRET//WIBBLE)` would
                //     land as `classification: Some(Secret)` with
                //     WIBBLE simply discarded. The correct behavior is
                //     to discard the candidate so the decoder's output
                //     set stays honest: either a token fully resolves or
                //     the whole candidate goes away.
                let has_unknown_token = parsed
                    .attrs
                    .token_spans
                    .iter()
                    .any(|s| matches!(s.kind, marque_ism::TokenKind::Unknown));
                if has_unknown_token {
                    return None;
                }

                // Canonicalization seam: `MarkingScheme::canonicalize`
                // is the sole `ParsedAttrs → CanonicalAttrs` route. The
                // recognizer receives the scheme via the `&S` parameter
                // threaded through `recognize()` (issue #634).
                let mut attrs = scheme.canonicalize(parsed.attrs);

                // 3b. Span-offset contract: `CanonicalAttrs::token_spans`
                //     returned by the strict parser carry offsets into
                //     `attempt.bytes` (the canonicalized buffer), NOT the
                //     original `bytes` slice the caller passed to
                //     `recognize()`. Propagating those spans would
                //     violate the [`Recognizer`] contract — "spans are by
                //     offset into [the input] buffer" — and misplace
                //     downstream diagnostics/fixes whenever
                //     canonicalization changed spacing, delimiter form,
                //     token order, or token length (e.g., `COMINT` → `SI`
                //     changes a 6-byte token to 2 bytes). Until we have a
                //     proper source↔canonical span map, decoder-produced
                //     markings must not carry token spans; downstream
                //     CAPCO rules that consume `attrs.token_spans` fall
                //     back to marking-level spans for decoder fixes.
                //
                //     Clearing happens AFTER the Unknown-token check
                //     above — we need the spans to filter partial
                //     canonicalizations, but must drop them before the
                //     marking leaves the decoder.
                attrs.token_spans = Box::new([]);
                Some(CapcoMarking::new(attrs))
            };
            let marking = if is_fast_path_candidate_shape(kind, &attempt.bytes) {
                try_fast_parse_us_class_and_dissem(kind, &attempt.bytes).or_else(parse_with_strict)
            } else {
                parse_with_strict()
            };
            let Some(marking) = marking else {
                continue;
            };

            // 3c. The strict parser is lenient — it accepts any
            //     `BYTES//BYTES` shape and emits a `CanonicalAttrs`
            //     with empty fields when nothing is recognized. Drop
            //     such trivial parses so the decoder doesn't
            //     fabricate a marking for prose like `FROBNITZ//WIBBLE`.
            if !is_nontrivial_marking(&marking) {
                continue;
            }

            // 3c-bis. Reject `Us(Restricted)` markings. Same rationale
            //         as the strict recognizer (see [`is_us_restricted`]):
            //         RESTRICTED is by definition a non-US classification,
            //         so any candidate the parser landed on the US axis
            //         is invalid regardless of what other tokens
            //         (`fgi_marker`, dissem controls, REL TO) accompany
            //         it. Real foreign-origin RESTRICTED markings parse
            //         to `Fgi(...)` / `Nato(...)` / `Joint(...)` and
            //         pass through.
            if is_us_restricted(&marking) {
                continue;
            }

            // 3d. Drop candidates below the page's strict
            //     classification floor.
            if let Some(floor) = cx.classification_floor
                && !meets_classification_floor(&marking, floor)
            {
                continue;
            }

            // 3e. Portion/Banner shapes REQUIRE a classification to
            //     be a meaningful marking. The strict parser is
            //     lenient — `(YS//NF)` parses to a marking with
            //     `classification: None, dissem_controls: [Nf]`
            //     because `YS` doesn't resolve to any
            //     [`Classification`] variant. The decoder's
            //     bag-of-tokens scorer rewards FEWER negative-log-
            //     prior tokens, so without this filter the
            //     no-classification candidate would outrank a
            //     heuristic-corrected `(TS//NF)` candidate that
            //     contributed both `TOP SECRET` and `NF` priors.
            //
            //     For CAB shapes the analogous completeness check
            //     is "any of classified_by / derived_from /
            //     declassify_on / declass_exemption is set" —
            //     [`is_nontrivial_marking`] above already covers
            //     that for the CAB code path. For
            //     [`MarkingType::PageBreak`] this filter is
            //     intentionally a no-op: page breaks are control
            //     shapes the decoder shouldn't be asked to recover.
            if matches!(kind, MarkingType::Portion | MarkingType::Banner)
                && marking.0.classification.is_none()
            {
                continue;
            }

            // 4. Score: compute prior and posterior separately. The
            //    prior is the sum of baked corpus log-priors over the
            //    marking's canonical tokens; the posterior is the
            //    prior plus the per-feature log-odds deltas recorded
            //    during canonicalization. `Candidate::prior_log_odds`
            //    is documented as the prior alone (see
            //    `crates/scheme/src/ambiguity.rs`) and is combined
            //    additively with `EvidenceFeature.log_odds` by any
            //    downstream resolver — storing the full posterior
            //    there would double-count the features once the
            //    resolver re-adds them. Internal decoder sort /
            //    threshold decisions use the posterior.
            //
            //    Issue #472: `null_posterior` is *not* per-candidate
            //    in shape — it's a single observed-prose-prior sum
            //    over the original input bytes, computed once below
            //    the scoring loop and replicated into every
            //    candidate's `null_posterior` field. Pre-#472 the
            //    null was summed over each candidate's canonical
            //    tokens; that evaluated the prose hypothesis against
            //    a token set the user never typed whenever fuzzy
            //    correction shifted a common prose acronym (e.g.,
            //    `(CMS)`) to a rare CAPCO token (e.g., `CTS`).
            let (prior, posterior) = score_candidate(&attempt, &marking, kind);
            scored.push(ScoredCandidate {
                marking,
                prior,
                posterior,
                null_posterior: 0.0, // set below from observed bytes
                canonical_bytes: attempt.bytes.into_boxed_slice(),
                features: attempt.features,
                fix_source: attempt.fix_source,
            });
        }

        // Issue #472: compute the prose null hypothesis once from the
        // original observed bytes (NOT each candidate's canonical
        // tokens) and replicate across every scored candidate. The
        // observed bytes are the same for every candidate by
        // construction, so this is a constant per `recognize` call —
        // computing it inside the loop would just redo the same work.
        //
        // Constitution V Principle V: the bytes are read here to
        // compute a scalar log-prior; the scalar flows into
        // `null_posterior` and from there into scoring math
        // (`posterior >= null_posterior + margin` and
        // `recognition_runner_up`). No byte content escapes.
        let observed_null = observed_prose_log_prior(bytes);
        for candidate in &mut scored {
            candidate.null_posterior = observed_null;
        }

        if scored.is_empty() {
            return Parsed::Ambiguous {
                candidates: Vec::new(),
            };
        }

        // 4-bis. Context features. The features computed here depend
        // on the candidate's POSITION in the source document, not on
        // its canonical token content, so they apply uniformly to
        // every surviving scored candidate. The audit-trace footprint
        // is bounded: the pre-truncate pool is capped at
        // `K_MAX_CANDIDATES * 2 = 16` candidates and the post-sort
        // truncate keeps at most `K_MAX_CANDIDATES = 8` for
        // emission, so worst-case context-feature audit-trace
        // entries per call are `8 × CONTEXT_FEATURE_MAX = 16`. The
        // features on losing candidates carry diagnostic value
        // (why did the runner-up lose?) so the audit trace
        // intentionally captures them for the full top-K rather
        // than gating to the winner only. Two signals:
        //
        // - **Line position** (Task 9): a portion candidate deep
        //   into a non-anchor line is overwhelmingly prose. The
        //   penalty is mutually exclusive with the bullet-anchor
        //   bonus: when `line_prefix` looks like an enumeration
        //   anchor (`1B.a.3.`, `(a) `, `* `, …), the bonus fires
        //   and the penalty is skipped.
        //
        // - **Lowercase surrounding context** (Task 10): a candidate
        //   with lowercase letters embedded in lowercase prose is
        //   overwhelmingly a parenthetical glyph, not a marking.
        //   Archival all-caps documents short-circuit naturally —
        //   the candidate stays uppercase, so the
        //   `candidate_has_lowercase` predicate never trips.
        //
        // Both features apply ONLY to portion candidates today.
        // Banner and CAB shapes have richer structural evidence
        // (line breaks, fielded labels) and don't share the prose-
        // glyph confusability that motivates these features.
        let context_features = compute_context_features(kind, bytes, cx);
        if !context_features.is_empty() {
            for candidate in &mut scored {
                for &(id, delta) in &context_features {
                    candidate.posterior += delta;
                    candidate.features.push(FeatureEntry { id, delta });
                }
            }
        }

        // 5. Drop any candidate with a non-finite posterior, sort
        //    descending, keep top K=8.
        //
        // NaN posteriors should be impossible —
        // `MISSING_TOKEN_LOG_PRIOR = -12.0` and every feature delta
        // is a finite constant — but a future scoring change could
        // introduce a NaN-producing codepath. Under `f32::total_cmp`
        // with the descending comparator (`b.total_cmp(&a)`), `+NaN`
        // would sort *ahead* of every finite posterior and become the
        // "top" candidate — its NaN posterior would then propagate
        // into `log_margin` and `DecoderProvenance::posterior`, where
        // `Confidence::validate` would later panic at audit-record
        // promotion. Filter non-finite candidates out before the sort
        // so the dispatch can never see one.
        //
        // `debug_assert` keeps the original assumption (decoder code
        // does not produce NaN today) loud in dev builds; the filter
        // is the production safeguard for if that assumption ever
        // breaks silently.
        debug_assert!(
            scored.iter().all(|c| c.posterior.is_finite()),
            "decoder produced non-finite posterior — invariant violated"
        );
        scored.retain(|c| c.posterior.is_finite());
        if scored.is_empty() {
            return Parsed::Ambiguous {
                candidates: Vec::new(),
            };
        }
        // Per-candidate prose null-hypothesis filter (issue #258
        // PR #313 review-2). Each candidate's `null_posterior` is
        // computed against *its own* canonical token set, so the
        // marking-vs-null comparison is per-candidate, not a
        // single-top property. Drop candidates whose prose-side
        // posterior beats their marking-side posterior before sort
        // / dispatch. Without this filter the previous early-return
        // could silently suppress a perfectly good marking-y
        // candidate just because some other candidate (with a
        // different token set) ranked higher by raw posterior but
        // lost to its own null. Filtering first means the dispatch
        // sees only candidates whose marking interpretation actually
        // beats their prose alternative.
        debug_assert!(
            scored.iter().all(|c| c.null_posterior.is_finite()),
            "decoder produced non-finite null_posterior — invariant violated"
        );
        // Portion-shape null-hypothesis filter (issue #472, expanded
        // from issue #258).
        //
        // Before issue #472 the gate fired only for single-letter
        // portions (`(s)`, `(c)`, `(u)`, `(r)`). That covered the
        // SC-003a-Federalist `(s)` case but missed the broader class
        // of prose acronym parentheticals (`(CMS)`, `(CTs)`, `(MD)`,
        // …) where the user typed a 2-5-letter English acronym that
        // happens to fuzzy-correct to a CAPCO portion shape. Before
        // issue #472 landed, the gate also keyed on canonical-token
        // shape, so an observed `(CMS)` whose decoder canonicalized
        // to `CTS` was measured against the prose prior for the
        // (rare) CAPCO token it became, not the (common) prose
        // acronym the user typed.
        //
        // #472 generalizes the gate to **every** portion shape except:
        //
        // - `has_double_slash(bytes)`: a portion containing `//`
        //   carries a category separator that prose convention does
        //   not produce, so the marking interpretation is the only
        //   plausible reading; bypass the null comparison.
        // - `is_bare_classification_shape(bytes)`: a portion whose
        //   inner content is exactly a canonical classification token
        //   (`(U)`, `(C)`, `(S)`, `(TS)`, `(R)` or the NATO
        //   abbreviations `(NU)`, `(NR)`, `(NC)`, `(NS)`, `(CTS)`)
        //   is the grammar's *only* shape for that classification
        //   level. Suppressing it would reject legitimate IC
        //   abbreviation recovery on the same grounds that motivate
        //   the gate.
        //
        // Banner and CAB shapes bypass the filter entirely — their
        // forms are long enough that English prose doesn't fabricate
        // them by glyph coincidence.
        if kind == MarkingType::Portion
            && !has_double_slash(bytes)
            && !is_bare_classification_shape(bytes)
        {
            scored.retain(|c| c.posterior >= c.null_posterior + NULL_HYPOTHESIS_LOG_MARGIN);
        }
        if scored.is_empty() {
            // Every candidate's prose hypothesis beat its marking
            // hypothesis — prose-shaped input that round-tripped
            // through the strict parser into a CAPCO shape but is
            // more likely prose than any of the recovery candidates
            // (e.g., `(s)` mid-sentence in Federalist 10). Return
            // zero-candidate Ambiguous so the engine emits no
            // diagnostic and no auto-fix — the honest "we see signal,
            // can't resolve" outcome.
            return Parsed::Ambiguous {
                candidates: Vec::new(),
            };
        }
        scored.sort_by(|a, b| b.posterior.total_cmp(&a.posterior));
        scored.truncate(K_MAX_CANDIDATES);

        // 6. Decision: top-over-runner-up log margin on the posterior.
        //
        // Issue #258 split this into two concerns:
        //
        // 1. **Dispatch decision** (Unambiguous vs Ambiguous): driven
        //    by the marking-side runner-up only — i.e., the second-
        //    best CAPCO candidate. Preserves the invariant established
        //    before issue #258 that a single CAPCO candidate (after
        //    strict-parse filtering) collapses to Unambiguous
        //    regardless of how confident the prose alternative is, as
        //    long as the prose alternative does not outright beat the
        //    marking interpretation (the null-wins early return below).
        //    Applying `UNAMBIGUOUS_LOG_MARGIN` against the null
        //    hypothesis would tighten the threshold for short fuzzy
        //    fixes (e.g., `(SERCET//NF)`) that already cleared the
        //    marking-vs-null comparison.
        //
        // 2. **Recognition score** (the user-visible confidence
        //    flowing into `recognition_score`): driven by the
        //    *strongest* runner-up — `max(marking_runner_up,
        //    top.null_posterior)` — so a fix that's only marginally
        //    more likely than the prose alternative carries
        //    appropriate uncertainty even when the dispatch returns
        //    Unambiguous. The null hypothesis competes for the
        //    runner-up slot in the audit-record provenance even
        //    when it doesn't change the dispatch decision.
        //
        // The null-wins early return below handles the case where
        // prose outright beats the marking interpretation — the
        // Federalist-corpus `(s)` regression that motivated #258. No
        // candidates returned, no diagnostic, no auto-fix.
        let top_score = scored[0].posterior;
        // After the per-candidate `posterior >= null_posterior`
        // filter above, the top candidate's prose alternative is by
        // construction at most equal to its marking interpretation —
        // no separate null-wins early return is needed here.
        let top_null_score = scored[0].null_posterior;
        let marking_runner_up = scored
            .get(1)
            .map(|c| c.posterior)
            .unwrap_or(f32::NEG_INFINITY);

        // Recognition runner-up: whichever of (marking-side runner-up,
        // null hypothesis) is the strongest alternative to the top.
        // `f32::max` would propagate NaN; we already filtered non-
        // finite posteriors above so the inputs are well-defined.
        let recognition_runner_up = if marking_runner_up >= top_null_score {
            marking_runner_up
        } else {
            top_null_score
        };
        let log_margin_recognition = top_score - recognition_runner_up;

        // Dispatch margin: marking-side runner-up only. When there's
        // a single CAPCO candidate, marking_runner_up is
        // `f32::NEG_INFINITY`, so log_margin_marking is `+∞` and
        // the `scored.len() == 1` short-circuit fires.
        let log_margin_marking = top_score - marking_runner_up;

        if scored.len() == 1 || log_margin_marking >= UNAMBIGUOUS_LOG_MARGIN {
            // Move the top candidate out so we can hand `canonical_bytes`
            // and `features` directly to provenance without an extra
            // clone — the marking carries the heaviest payload and we
            // only need it once.
            let top = scored.swap_remove(0);
            // `runner_up_ratio = exp(log_margin_recognition)`. The
            // recognition margin uses `max(marking_runner_up,
            // top.null_posterior)` (issue #258) so the audit record
            // reflects the strongest competing interpretation — be it
            // a runner-up CAPCO candidate or the prose null
            // hypothesis. A sufficiently separated top overflows
            // `f32::exp()` to `+∞` past `log_margin ≈ 88.7`, and
            // `Confidence::validate` would then reject the resulting
            // record as non-finite — making `FixProposal::new` panic
            // at the audit boundary on extreme score separations.
            // Saturate at `f32::MAX` so the audit record carries
            // "the ratio is enormous" instead of crashing the engine.
            let runner_up_ratio = if recognition_runner_up.is_finite() {
                let ratio = log_margin_recognition.exp();
                Some(if ratio.is_finite() { ratio } else { f32::MAX })
            } else {
                None
            };
            let mut marking = top.marking;
            // Issue #431 span-offset contract: the Recognizer trait
            // now requires absolute-source-coordinate spans on return.
            // Decoder candidates currently clear `token_spans` (see
            // §3b above — canonicalization may rewrite span lengths,
            // so spans are dropped until a source↔canonical span map
            // lands), so this shift is a no-op today. The call stays
            // here so the contract holds the moment decoder-side
            // span preservation lands without a second engine edit.
            //
            // The debug_assert pins the no-op assumption: if a future
            // change to decoder-side span preservation wires up
            // `token_spans` without re-checking the offset arithmetic,
            // this fires before the silent miscoordination ships.
            debug_assert!(
                marking.0.token_spans.is_empty(),
                "decoder must clear token_spans until source↔canonical span-map lands; \
                 wiring up token_spans without revisiting the offset shift would silently \
                 produce spans at wrong absolute coordinates",
            );
            crate::recognizer::shift_token_spans(&mut marking.0, offset);
            marking.1 = Some(DecoderProvenance::new(
                top.canonical_bytes,
                top.posterior,
                runner_up_ratio,
                top.features
                    .into_iter()
                    .map(|f| FeatureContribution {
                        id: f.id,
                        delta: f.delta,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                top.fix_source,
            ));
            return Parsed::Unambiguous(marking);
        }

        // Ambiguous: return the whole K-truncated set with per-feature
        // evidence so the engine can surface a user-visible diagnostic.
        // `prior_log_odds` carries the prior alone; `evidence` carries
        // the feature deltas. A resolver that re-computes the
        // posterior as `prior + Σ evidence.log_odds` reproduces the
        // decoder's internal score without double-counting.
        Parsed::Ambiguous {
            candidates: scored
                .into_iter()
                .map(|s| Candidate {
                    marking: s.marking,
                    evidence: s.features.iter().map(feature_entry_to_evidence).collect(),
                    prior_log_odds: s.prior,
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Tests live in `tests/recognizer_tests.rs`. They were carved out of
// this file to keep the combined production + test surface within
// the 800-line gate.

#[path = "tests/recognizer_tests.rs"]
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(unused_imports)]
mod tests;
