// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase-D probabilistic [`Recognizer`] — the "decoder".
//!
//! This module implements the deep-scan half of the strict/deep-scan
//! recognizer split introduced in Phase 4 PR-2. When the engine is
//! configured for deep-scan (batch reconciliation mode,
//! rule-escalated region, `--deep-scan` CLI flag), and the strict
//! recognizer returns zero candidates for a marking region, the
//! engine falls back to the decoder to recover mangled markings that
//! are one of a small set of canonical-shape deviations away from a
//! real CAPCO-2016 marking:
//!
//! - Edit-distance-1/2 token typos (`SERCET` → `SECRET`).
//! - Token reordering within categories (`NOFORN//SECRET` →
//!   `SECRET//NOFORN`).
//! - CAPCO-2016-superseded tokens (`COMINT` → `SI`).
//! - Case mistakes (`secret//noforn` → `SECRET//NOFORN`).
//! - Garbled delimiters (`S ∕∕ NOFORN` → `S//NOFORN`).
//!
//! The decoder never fabricates a marking where none exists. When the
//! observed tokens fit no CAPCO grammar template, it returns
//! `Parsed::Ambiguous { candidates: vec![] }` — the zero-candidate
//! signal per foundational-plan line 609-612.
//!
//! ## Why this lives in `marque-engine`, not `marque-capco`
//!
//! Same Constitution VII rationale as `StrictRecognizer` (PR-2):
//! `marque-capco` may not depend on `marque-core`, but the decoder
//! needs core's fuzzy-vocab matcher and strict parser to materialize
//! candidates. `marque-engine` is the sole crate where both chains
//! converge. The original tasks.md T059/T061 placement is amended in
//! tasks.md itself.
//!
//! ## Scoring approach (foundational-plan §5.2)
//!
//! For each candidate the decoder computes:
//!
//! ```text
//! log_posterior(candidate | observed)
//!   = log_prior(candidate)                      // baked corpus priors (PR-1)
//!   + Σ log_likelihood(feature | candidate)     // enumerated scored features
//! ```
//!
//! The decoder currently scores the candidate-shape features it
//! records from the closed [`FeatureId`] enum:
//! `EditDistance1`, `EditDistance2`, `TokenReorder`,
//! `SupersededToken`, and `BaseRateCommonMarking`. Each contributes
//! a fixed log-odds delta documented at the feature's call site.
//!
//! [`FeatureId::StrictContextClassification`] is part of the audit-
//! schema enum but is **not** currently a scored-feature term:
//! classification-level context is enforced through the separate
//! [`ParseContext::classification_floor`] hard filter (FR-011),
//! which rejects below-floor candidates before scoring rather than
//! adding a likelihood term to the posterior. [`FeatureId::CorpusOverrideInEffect`]
//! is reserved for PR-5 when corpus-override is wired; the decoder
//! does not emit it today. Turning either into an actual scored
//! contributor requires a coordinated audit-schema bump
//! (`MARQUE_AUDIT_SCHEMA`) per `marque-rules/src/confidence.rs` doc.
//!
//! The top candidate wins when its posterior exceeds the runner-up by
//! a configured ratio; below that threshold the decoder returns
//! `Parsed::Ambiguous { candidates }` so the engine can surface a
//! diagnostic rather than auto-apply. `Candidate::prior_log_odds`
//! carries the prior alone (sum of token log-priors); the
//! per-feature log-odds deltas live only in
//! `Candidate::evidence[i].log_odds`, so a resolver that reconstructs
//! `prior_log_odds + Σ evidence.log_odds` recovers the decoder's
//! internal posterior exactly, without double-counting.
//!
//! ## What this module is NOT
//!
//! - Not a full template-matching grammar engine. The MVP materializes
//!   candidates by canonicalizing observed tokens and round-tripping
//!   through the strict parser — the strict parser is the arbiter of
//!   "is this a CAPCO-shape marking." If the canonicalized bytes
//!   strict-parse, we have a candidate; if not, we discard.
//! - Not a learning system. All priors are compile-time-baked `&'static`
//!   tables from `marque_capco::priors` (Constitution III: no runtime
//!   corpus override on WASM).
//! - Not a fix applier. The decoder proposes `CapcoMarking` candidates;
//!   the engine applies them through the normal `Diagnostic` /
//!   `FixProposal` path with `FixSource::DecoderPosterior`.

use std::borrow::Cow;

use marque_capco::provenance::DecoderProvenance;
use marque_capco::{CapcoMarking, CapcoScheme};
use marque_core::{Parser, fuzzy::FuzzyVocabMatcher};
use marque_ism::{
    CanonicalAttrs, CapcoTokenSet, Classification, DissemControl, MarkingClassification,
    NatoClassification, SciControl, SciControlBare, SciControlSystem,
    span::{MarkingCandidate, MarkingType, Span},
    token_set::TokenSet as _,
};
use marque_rules::confidence::{FeatureContribution, FeatureId};
use marque_scheme::MarkingScheme;
use marque_scheme::ambiguity::{Candidate, EvidenceFeature, Parsed};
use marque_scheme::recognizer::{ParseContext, Recognizer};
use smallvec::SmallVec;

use crate::recognizer::{StrictRecognizer, is_us_restricted};

/// K=8 candidate bound per foundational-plan §5.2 and research.md R3.
///
/// Higher K burns latency without accuracy gain (diminishing returns
/// above 6 per the primary-source corpus analysis); lower K drops
/// recall on multi-token reorderings. Tunable in-place — the bound is
/// advisory, not a correctness invariant.
const K_MAX_CANDIDATES: usize = 8;

/// Runner-up posterior-ratio threshold for emitting `Unambiguous`.
///
/// The decoder computes `log_margin = top_posterior - runner_up_posterior`
/// in natural-log space. When `log_margin >= UNAMBIGUOUS_LOG_MARGIN`,
/// the decoder collapses to `Unambiguous(top)`; below the threshold it
/// returns `Ambiguous { candidates }` so the engine can surface a
/// diagnostic rather than auto-apply a close call.
///
/// `1.6` corresponds to a posterior odds ratio of `e^1.6 ≈ 4.95` —
/// i.e., the top candidate is roughly five times as likely as the
/// runner-up given the observed bytes. This is the **odds** ratio
/// (`P(top)/P(runner_up)`), not a probability ratio.
const UNAMBIGUOUS_LOG_MARGIN: f32 = 1.6;

/// Minimum log-margin a candidate's marking-side posterior must hold
/// over its prose null posterior to clear the per-candidate prose
/// filter (issue #258, expanded after the documents-corpus marking
/// stratum landed in PR1).
///
/// Originally the filter required only `posterior >= null_posterior`
/// (a zero-margin gate). That was sufficient when the marking
/// stratum was just `tests/corpus/valid/` (~34 short fixtures); the
/// per-token marking-vs-prose delta `log P(token|marking) − log
/// P(token|prose)` for short tokens like `S` and `U` stayed near
/// zero and the Federalist `(s)` mid-prose case suppressed cleanly.
///
/// After PR1 added `tests/corpus/documents/marked/` (40 multi-page
/// synthetic-positive documents with hundreds of `(S//*)` portion
/// marks) the marking-side prior for `S` strengthened from
/// `log_prior ≈ -3.97` to `-3.28`, while the prose-side prior
/// (Enron + Congressional Record + GAO + CIA CREST) sat at `-5.11` —
/// a `+1.83` delta. The zero-margin filter let the marking
/// hypothesis win for isolated `(s)` candidates and re-introduced
/// the SC-003a regression on `Notwithstanding (s) the early`.
///
/// `2.5` (e^2.5 ≈ 12.2×) is the smallest margin that suppresses the
/// Federalist `(s)` regression at its actual marking-vs-null delta of
/// `+2.21` (`S`: marking `-3.28`, prose `-5.49`). `(c)` at `+1.08`
/// and most other single-letter portions are rejected at the same
/// threshold by construction. `(u)` at `+2.86` survives this margin
/// — a lowercase `(u)` mid-prose canonicalizing to UNCLASSIFIED is
/// the residual false-positive surface; it has not been observed in
/// the test corpus, and the prose-glue heuristic
/// (`preceded_by_whitespace = false`) suppresses the much more
/// common `letter(s)` / `function(c)` cases independently.
///
/// **This margin applies to single-letter portion candidates only.**
/// `(s)`, `(c)`, `(u)`, `(r)` are the prose-glyph overlap cases —
/// plural-suffix, copyright, pronoun, etc. — where short-token
/// marking-vs-prose priors collide directly with English usage.
/// Multi-letter portion candidates (`(NU)`, `(NC)`, `(NR)`, `(TS)`,
/// `(SI)`, ...) and banner-form candidates (`UNCLASSIFIED`,
/// `CONFIDENTIAL`, etc.) bypass the null filter entirely: their
/// shapes are long enough that English prose doesn't fabricate them
/// by glyph coincidence, and pinning any positive margin on them
/// would reject legitimate NATO/IC abbreviation recovery (NU at
/// marking `-8.43`, prose `-8.34`, delta `-0.09`; NC at marking
/// `-8.43`, prose `-5.89`, delta `-2.54`) where the marking
/// stratum has zero examples but the strict grammar still
/// recognizes the token. The strict parser + scanner provide the
/// structural discrimination they need.
const NULL_HYPOTHESIS_LOG_MARGIN: f32 = 2.5;

/// Maximum line-column at which a portion candidate is considered
/// "near the start of its line" and does NOT receive the
/// [`FeatureId::LinePositionPenalty`] (Task 9, user brainstorm a).
///
/// Real portion markings appear at column 0 or after a short
/// bullet/anchor prefix (`1. (S)`, `* (S//NF)`, `1B.a.3.(c)`). A
/// portion-shaped `(x)` more than this many bytes into a line that
/// does not look like an enumeration anchor is overwhelmingly a
/// prose glyph — parenthetical aside, plural suffix, copyright
/// notice. 4 bytes covers the common short-bullet case (`1. `,
/// `(a) `) for the rare path where `looks_like_bullet_anchor`
/// returns false despite a short prefix.
///
/// **This budget gates the LinePositionPenalty only — bullet
/// anchors are recognized independently by
/// [`looks_like_bullet_anchor`].** [`compute_context_features`]
/// runs the anchor check first and emits the bullet bonus instead
/// of the penalty whenever the prefix shape is anchor-like,
/// regardless of `offset` value. The budget therefore fires only
/// for prefixes that fail anchor recognition AND exceed 4 bytes —
/// the residual "long non-anchor prefix" case, which is almost
/// always running prose.
const LINE_POSITION_BUDGET: usize = 4;

/// Negative log-odds delta added to a portion candidate's posterior
/// when its line position exceeds [`LINE_POSITION_BUDGET`] AND its
/// preceding bytes do not match a bullet/section-anchor pattern
/// (Task 9, [`FeatureId::LinePositionPenalty`]).
///
/// `-2.0` empirically reproduces the suppression behavior the
/// per-candidate null filter already gives single-letter portions
/// (the `(s)`-mid-Federalist case) for the broader class of
/// mid-line portion-shaped prose glyphs (e.g., `Foo (SOMETHING)
/// bar` where `SOMETHING` happens to fuzzy-match a CAPCO token).
/// Combined with the null filter (margin `2.5`), most single-letter
/// portion glyphs that survived the null filter under PR1 priors
/// alone are now suppressed by the additional position evidence.
const LINE_POSITION_PENALTY: f32 = -2.0;

/// Positive log-odds delta added when a portion candidate's
/// same-line preceding bytes look like a bullet or section anchor
/// (Task 9, [`FeatureId::BulletAnchorBonus`]).
///
/// Mutually exclusive with [`LINE_POSITION_PENALTY`] — when the
/// anchor pattern matches, the position penalty is skipped and this
/// bonus is recorded instead. `+1.5` ensures a legitimate
/// `1B.a.3.(c)` IC-document enumeration sails past the per-candidate
/// null filter even when the candidate itself is a single-letter
/// portion (those glyphs would otherwise lose to the prose
/// hypothesis under [`NULL_HYPOTHESIS_LOG_MARGIN`]).
const BULLET_ANCHOR_BONUS: f32 = 1.5;

/// Negative log-odds delta added to a candidate's posterior when the
/// candidate contains lowercase ASCII letters AND the surrounding
/// source context (`±` [`crate::engine::LOWERCASE_WINDOW_RADIUS`]
/// bytes) is lowercase-dominant (Task 10,
/// [`FeatureId::LowercaseSurroundingContext`]).
///
/// Lowercase markings in lowercase prose are overwhelmingly NOT
/// markings — they are parenthetical asides, plural suffixes,
/// copyright notices, sentence-internal references. Archival
/// all-caps documents short-circuit this feature naturally: the
/// candidate itself is uppercase in those documents, so the
/// "candidate has lowercase letters" predicate never trips.
///
/// `-2.0` matches the position penalty magnitude — both are
/// secondary signals on top of token-prior evidence, both should be
/// strong enough to flip the null filter when token priors leave
/// the candidate borderline. The two penalties are additive when
/// both apply (e.g., lowercase `(secret)` mid-prose: `-4.0` total
/// before the null filter).
const LOWERCASE_CONTEXT_PENALTY: f32 = -2.0;

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
        // interactive-authoring latency (SC-001). The engine only
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
        let canonical_attempts = generate_candidate_bytes(bytes);
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
                // is the sole `ParsedAttrs → CanonicalAttrs` route per
                // FR-043 (PR 3c.2.E retired the transitional
                // `marque_ism::from_parsed_unchecked` adapter). The
                // recognizer receives the scheme via the `&S`
                // parameter threaded through `recognize()` after
                // `engine-S-generic-recognizer-cleanup` (#634) landed.
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

            // 3d. FR-011 — drop candidates below the page's strict
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
        // Pre-#472 the gate fired only for single-letter portions
        // (`(s)`, `(c)`, `(u)`, `(r)`). That covered the
        // SC-003a-Federalist `(s)` case but missed the broader class
        // of prose acronym parentheticals (`(CMS)`, `(CTs)`, `(MD)`,
        // …) where the user typed a 2-5-letter English acronym that
        // happens to fuzzy-correct to a CAPCO portion shape. The
        // pre-#472 gate also gated by canonical-token shape, so an
        // observed `(CMS)` whose decoder canonicalized to `CTS` was
        // measured against the prose prior for the (rare) CAPCO token
        // it became, not the (common) prose acronym the user typed.
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
            // diagnostic and no auto-fix. FR-015: "we see signal,
            // can't resolve."
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
        //    best CAPCO candidate. Preserves the pre-#258 invariant
        //    that a single CAPCO candidate (after strict-parse
        //    filtering) collapses to Unambiguous regardless of how
        //    confident the prose alternative is, as long as the prose
        //    alternative does not outright beat the marking
        //    interpretation (the null-wins early return below).
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

/// One scored candidate kept in the decoder's working set.
///
/// `prior` and `posterior` are tracked separately so
/// `Candidate::prior_log_odds` can carry the prior alone (per the
/// trait-level contract in `crates/scheme/src/ambiguity.rs`) while
/// internal sort / threshold decisions use the posterior.
struct ScoredCandidate {
    marking: CapcoMarking,
    /// Sum of baked corpus log-priors over the marking's canonical
    /// tokens. No feature deltas included.
    prior: f32,
    /// `prior + Σ feature.delta`. Used for sorting and threshold
    /// comparisons inside the decoder; not stored in the emitted
    /// `Candidate` record.
    posterior: f32,
    /// Prose-side null posterior (issue #258).
    ///
    /// Sum of [`marque_capco::priors::token_prose_log_prior`] over
    /// the same canonical tokens used for [`Self::prior`], plus
    /// [`marque_capco::priors::country_code_prose_log_prior`] over
    /// the same `rel_to` codes. Carries the prose hypothesis for the
    /// candidate's token set — `log P(tokens | prose)` evaluated
    /// against the prose-stratum corpus.
    ///
    /// The dispatch logic in [`DecoderRecognizer::recognize`] §6
    /// compares this against [`Self::posterior`] for the top
    /// candidate. When `null_posterior > posterior` the decoder
    /// returns zero candidates (FR-015 — prose wins the null
    /// hypothesis competition, no fix is emitted). When
    /// `null_posterior <= posterior` the null hypothesis becomes a
    /// virtual runner-up that flows into `recognition_score`.
    null_posterior: f32,
    /// Canonical byte string the strict parser accepted for this
    /// candidate. Threaded into [`DecoderProvenance::canonical_bytes`]
    /// when this candidate wins the Unambiguous collapse, so the
    /// engine can emit the decoder fix from the original mangled
    /// bytes to this canonical form (Phase 4 PR-4b, T068).
    canonical_bytes: Box<[u8]>,
    /// Per-candidate feature contributions. `SmallVec<[…; 4]>` matches
    /// `Confidence::features` so the inline-4 case stays heap-free
    /// from canonicalization through audit emission.
    features: SmallVec<[FeatureEntry; 4]>,
    /// Provenance discriminator carried from the originating
    /// [`CanonicalAttempt`]. The engine maps this to
    /// [`Severity::Fix`](marque_rules::Severity::Fix) for
    /// `DecoderPosterior` and
    /// [`Severity::Warn`](marque_rules::Severity::Warn) for
    /// `DecoderClassificationHeuristic` (issue #133 PR 2).
    fix_source: marque_rules::FixSource,
}

/// One feature recorded during candidate generation, paired with its
/// log-odds contribution. The decoder accumulates these to reconstruct
/// `Confidence::features` at audit-emit time.
#[derive(Debug, Clone, Copy)]
struct FeatureEntry {
    id: FeatureId,
    delta: f32,
}

/// Project a `FeatureEntry` onto the wire-shape [`EvidenceFeature`].
///
/// Routes the label through [`FeatureId::as_str`] — the single source
/// of truth for the FeatureId → audit-record-string registry declared
/// in `crates/rules/src/confidence.rs`. Lifted out of the inline
/// closure in [`DecoderRecognizer::recognize`] so the projection is
/// directly testable: a divergent local label registry (the PR #142 H2
/// pre-fix shape) would now fail
/// [`tests::feature_entry_to_evidence_uses_canonical_label_registry`]
/// rather than going unnoticed because the dispatcher discards
/// `Parsed::Ambiguous` results today.
fn feature_entry_to_evidence(f: &FeatureEntry) -> EvidenceFeature {
    EvidenceFeature {
        label: f.id.as_str(),
        log_odds: f.delta,
    }
}

/// A canonicalization attempt: the byte string the decoder will hand
/// to the strict parser, plus the features that transformation
/// represents. Zero or more attempts are generated per observed input.
///
/// `bytes` and `features` use `SmallVec` inline storage tuned to the
/// empirical distribution of CAPCO markings: most candidate byte
/// strings fit in 64 bytes (portion shapes are tiny; banners can
/// spill to the heap when REL TO grows long) and most attempts record
/// 1–4 features. The inline path saves a per-attempt heap allocation
/// on the deep-scan latency budget (SC-002).
struct CanonicalAttempt {
    bytes: SmallVec<[u8; 64]>,
    features: SmallVec<[FeatureEntry; 4]>,
    /// Which decoder path produced this attempt. Defaults to
    /// [`marque_rules::FixSource::DecoderPosterior`] for the standard
    /// vocab-based pipeline (delimiter normalization, fuzzy
    /// correction, token reorder, superseded-token replacement).
    /// The position-aware classification heuristic emits attempts
    /// with [`marque_rules::FixSource::DecoderClassificationHeuristic`]
    /// (issue #133 PR 2) so the engine can downgrade to
    /// [`marque_rules::Severity::Warn`] and cap
    /// [`marque_rules::Confidence::rule`].
    fix_source: marque_rules::FixSource,
}

// ---------------------------------------------------------------------------
// Marking-type inference (mirrors `recognizer::infer_marking_type`)
// ---------------------------------------------------------------------------

/// Infer a [`MarkingType`] from the shape of `bytes`.
///
/// Same heuristic as the strict recognizer — portion on leading `(`,
/// CAB on authority-head prefix, banner otherwise. Lives locally so
/// the decoder doesn't need to poke into `StrictRecognizer` internals.
fn infer_marking_type(bytes: &[u8]) -> Option<MarkingType> {
    let first = bytes.iter().copied().find(|&b| !b.is_ascii_whitespace())?;
    if first == b'(' {
        return Some(MarkingType::Portion);
    }
    if is_cab_head(bytes) {
        return Some(MarkingType::Cab);
    }
    Some(MarkingType::Banner)
}

fn is_cab_head(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let trimmed = text.trim_start();
    trimmed.starts_with("Classified By:")
        || trimmed.starts_with("Derived From:")
        || trimmed.starts_with("Declassify On:")
}

fn is_fast_path_candidate_shape(kind: MarkingType, bytes: &[u8]) -> bool {
    if !matches!(kind, MarkingType::Portion) {
        return false;
    }
    if bytes.is_empty() {
        return false;
    }
    let mut start = 0usize;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while start < end && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if start >= end {
        return false;
    }
    let trimmed = &bytes[start..end];
    if trimmed.len() > 32 {
        return false;
    }
    if !(trimmed.first() == Some(&b'(') && trimmed.last() == Some(&b')')) {
        return false;
    }
    if trimmed
        .iter()
        .any(|b| matches!(b, b',' | b' ' | b'\t' | b'\n' | b'\r'))
    {
        return false;
    }
    let mut sep_count = 0usize;
    for w in trimmed.windows(2) {
        if w == b"//" {
            sep_count += 1;
            if sep_count > 1 {
                return false;
            }
        }
    }
    sep_count == 1
}

/// Decoder-only fast parse for the common US classification + dissem shape.
///
/// This avoids invoking the full strict parser for canonical attempts like
/// `(SECRET//NF)` and typo-shaped attempts like `(SERCET//NF)` where the decoder already knows the shape
/// is a simple portion/banner with an optional slash-delimited dissem block.
/// Any non-trivial form (non-US prefix, extra `//` blocks, mixed-category
/// slash blocks, REL TO/DISPLAY ONLY, etc.) falls back to the full parser.
fn try_fast_parse_us_class_and_dissem(kind: MarkingType, bytes: &[u8]) -> Option<CapcoMarking> {
    if !matches!(kind, MarkingType::Portion | MarkingType::Banner) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?.trim();
    let body = match kind {
        MarkingType::Portion => text.strip_prefix('(')?.strip_suffix(')')?,
        MarkingType::Banner => text,
        _ => return None,
    };
    if body.is_empty() || body.starts_with("//") {
        return None;
    }

    let mut blocks = body.split("//");
    let class_block = blocks.next()?.trim();
    let dissem_block = blocks.next().map(str::trim);
    if blocks.next().is_some() || class_block.is_empty() {
        return None;
    }
    if class_block
        .chars()
        .any(|c| c.is_ascii_whitespace() || c == '/' || c == ',')
    {
        return None;
    }

    let dissem_us = if let Some(block) = dissem_block {
        if block.is_empty() {
            return None;
        }
        let mut out: SmallVec<[DissemControl; 4]> = SmallVec::new();
        for token in block.split('/') {
            let token = token.trim();
            if token.is_empty() || token.chars().any(|c| c.is_ascii_whitespace() || c == ',') {
                return None;
            }
            let token = marque_ism::marking_forms::banner_to_portion(token)
                .or_else(|| marque_ism::marking_forms::title_to_portion(token))
                .unwrap_or(token);
            let control = DissemControl::parse(token)?;
            out.push(control);
        }
        out.into_vec().into_boxed_slice()
    } else {
        Box::new([])
    };

    let mut attrs = CanonicalAttrs::default();
    attrs.classification =
        parse_simple_us_classification(class_block).map(MarkingClassification::Us);
    attrs.dissem_us = dissem_us;
    Some(CapcoMarking::new(attrs))
}

fn parse_simple_us_classification(token: &str) -> Option<Classification> {
    match token {
        "U" | "UNCLASSIFIED" => Some(Classification::Unclassified),
        "R" | "RESTRICTED" => Some(Classification::Restricted),
        "C" | "CONFIDENTIAL" => Some(Classification::Confidential),
        "S" | "SECRET" => Some(Classification::Secret),
        "TS" => Some(Classification::TopSecret),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Candidate byte generation
// ---------------------------------------------------------------------------

/// Generate bounded canonical-byte candidates from a mangled input.
///
/// Each returned [`CanonicalAttempt`] is a `Vec<u8>` the decoder will
/// hand to the strict parser. Attempts cover the transforms named in
/// the module docs:
///
/// - Case normalization (`secret//noforn` → `SECRET//NOFORN`).
/// - Garbled-delimiter rewrite (`S ∕∕ NOFORN` → `S//NOFORN`).
/// - Per-token fuzzy correction (edit-distance ≤ 2 via
///   [`marque_core::fuzzy::FuzzyVocabMatcher`]).
/// - Superseded-token replacement (`COMINT` → `SI`).
/// - Token reordering — tried when categorical ordering is the obvious
///   deviation (e.g., portion `A//B` where B is a classification and
///   A isn't).
///
/// Bounded by [`K_MAX_CANDIDATES`] × 2 to keep the strict-parse pass
/// bounded; duplicates (different feature traces producing the same
/// canonical bytes) are deduplicated at emit time.
fn generate_candidate_bytes(bytes: &[u8]) -> SmallVec<[CanonicalAttempt; 4]> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return SmallVec::new();
    };

    // Strip surrounding whitespace; preserve leading `(` for portion
    // detection so the strict parser's portion path stays keyed off
    // the same first-non-whitespace byte the recognizer saw.
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return SmallVec::new();
    }

    // Inline-4: bounded above by `K_MAX_CANDIDATES * 2 = 16` via the
    // hard cap inside the `emit` closure below. Typical input produces
    // 1-4 attempts (raw + delimiter normalize + occasional fuzzy/sar
    // repair); the inline budget covers the common case while the
    // K=16 ceiling spills cleanly. Each `CanonicalAttempt` is ~150 B
    // with its own inline buffers — 4 inline ≈ 600 B on the stack.
    let mut attempts: SmallVec<[CanonicalAttempt; 4]> = SmallVec::new();
    let mut emit = |bytes: Vec<u8>,
                    features: SmallVec<[FeatureEntry; 4]>,
                    fix_source: marque_rules::FixSource| {
        // Hard cap at K_MAX_CANDIDATES × 2 — guarantees the strict-parse
        // work downstream is bounded even if new transform stages are added.
        if attempts.len() >= K_MAX_CANDIDATES * 2 {
            return;
        }
        // Dedup by the canonical byte string — different transform
        // sequences can converge on the same output. Emit-first wins:
        // the standard vocab-based attempts are emitted before the
        // heuristic attempt, so a heuristic candidate with bytes that
        // converge on a vocab-based result is dropped here, preserving
        // the more authoritative `FixSource::DecoderPosterior`
        // provenance.
        if !attempts
            .iter()
            .any(|a| a.bytes.as_slice() == bytes.as_slice())
        {
            attempts.push(CanonicalAttempt {
                bytes: SmallVec::from_vec(bytes),
                features,
                fix_source,
            });
        }
    };

    // ---- Raw: just trim + normalize delimiters/case. --------------
    let (normalized, mut delim_features) = normalize_delimiters_and_case(trimmed);

    // ---- REL TO structural repair (issue #133 PR 9) — applied as
    //      PREPROCESSING (before fuzzy correction) rather than as a
    //      competing candidate emission. All four PR-9 patterns are
    //      safe to apply unconditionally:
    //
    //      - Patterns 1/2 (`REL OT ` / `RELT O ` → `REL TO `) are
    //        literal-shape transforms. Neither pattern appears in any
    //        valid CAPCO text — REL has exactly two valid extensions
    //        (`REL TO` and `RELIDO`) — so the byte replacement is
    //        collision-free.
    //      - Patterns 3/4 (`A US` → `AUS`, `AU,S ` → `AUS, `) are
    //        trigraph-guarded inside a `REL TO ` block: the fix only
    //        fires when the joined 3-letter string is a known trigraph
    //        AND the shorter prefix alone is not, so a false positive
    //        would require the trigraph dictionary itself to disagree
    //        with reality.
    //
    //      Applying as preprocessing avoids two scoring problems that
    //      a separate-candidate emission would hit: (a) fuzzy
    //      correction would silently rewrite `RELT` → `REL` before
    //      pattern 2's header normalize could fire, and (b) REL TO
    //      trigraphs do NOT contribute to the prior in
    //      `for_each_canonical_token` (only classification, SCI, dissem,
    //      NIC, AEA, FGI do — see issue #186 for the corpus-weighted
    //      trigraph priors followup), so a separate fix candidate
    //      would tie with the raw on prior and lose on emit-order.
    //      Preprocessing eliminates the competing-raw-candidate
    //      problem entirely.
    //
    //      When structural repair fires, push a `BaseRateCommonMarking`
    //      feature onto `delim_features` so every candidate derived
    //      from the repaired text inherits the marker. This mirrors
    //      `try_insert_delimiter` and `try_sar_indicator_repair`
    //      (which add their own per-candidate `BaseRateCommonMarking`)
    //      and ensures the audit/provenance trace reflects that the
    //      input required cleanup beyond delimiter/case normalization.
    //      No dedicated `FeatureId` for structural repair exists in
    //      the audit schema (`marque-1.0`); reusing
    //      `BaseRateCommonMarking` keeps the schema closed and
    //      composes additively with the other normalization paths
    //      that share the same id.
    let repaired_text: Cow<'_, str> = match try_rel_to_structural_repair(&normalized) {
        Some(repaired) => {
            delim_features.push(FeatureEntry {
                id: FeatureId::BaseRateCommonMarking,
                delta: -0.3,
            });
            Cow::Owned(repaired)
        }
        None => normalized,
    };

    // ---- SCI delimiter repair (issue #198, #133 PR 10). Same
    //      preprocessing-shape as the REL TO repair above: rewrites
    //      concatenated CVE compounds (`HCSP → HCS-P`), missing
    //      slashes between bare control systems (`SITK → SI/TK`), and
    //      wrong-delimiter cases (`SI-TK → SI/TK`). All targets live
    //      in `CVEnumISMSCIControls.xml` — no agency vocab. Sub-
    //      compartments and unregistered compartments are out of
    //      scope (issue #180). Push a `BaseRateCommonMarking`
    //      penalty for the same reason as REL TO repair: a candidate
    //      that arrived clean should outrank one that needed
    //      structural cleanup when both produce the same shape.
    let repaired_text: Cow<'_, str> = match try_sci_delimiter_repair(&repaired_text) {
        Some(repaired) => {
            delim_features.push(FeatureEntry {
                id: FeatureId::BaseRateCommonMarking,
                delta: -0.3,
            });
            Cow::Owned(repaired)
        }
        None => repaired_text,
    };

    // ---- NATO longhand fold (T129, CAPCO-2016 §G.1 Table 4 pp 36-38).
    //      Recovers portions like `(NATO S)` / `(NATO S//NF)` by
    //      substituting the canonical abbreviation (`NS`, `CTS`, etc.)
    //      before the fuzzy-correction pass operates on it.
    //
    //      Fires AFTER SCI repair so the input is already in
    //      clean-delimiter form, and BEFORE fuzzy_correct_tokens so
    //      the post-fold canonical tokens (`NS`, `CTS`, …) pass through
    //      fuzzy correction unchanged. The `SupersededToken` feature
    //      records the fold in the audit trail. Delta is **zero** (not
    //      `-0.2` like the COMINT→SI precedent at ~line 1267) because
    //      the NATO fold is an equivalence transform between two valid
    //      surface forms, not a deprecated-token penalty. See the
    //      wire-site comment below for the null-hypothesis interaction
    //      that motivated dropping the penalty to 0.0.
    //
    //      Kind is re-derived from the trimmed text because
    //      `generate_candidate_bytes` does not carry the `MarkingType`
    //      inferred by the caller. The same heuristic used by
    //      `infer_marking_type`: leading `(` ⇒ Portion.
    let local_kind = if trimmed.starts_with('(') {
        MarkingType::Portion
    } else {
        MarkingType::Banner
    };
    let repaired_text: Cow<'_, str> = match try_nato_fold(&repaired_text, local_kind) {
        Some(folded) => {
            // Delta 0.0: the fold restores a canonical abbreviated form from a
            // valid longhand variant (`(NATO S)` → `(//NS)`). This is an
            // equivalence transform, not a superseded-token penalization. A
            // negative delta here would cause `NR`/`NC` (which use US-equivalent
            // single-letter tokens `"R"`/`"C"` with high prose frequency) to fail
            // the null-hypothesis filter even after `for_each_canonical_token` maps to
            // the low-prose-frequency NATO abbreviation form. The `SupersededToken`
            // feature still appears in the audit trail for provenance (T129).
            delim_features.push(FeatureEntry {
                id: FeatureId::SupersededToken,
                delta: 0.0,
            });
            Cow::Owned(folded)
        }
        None => repaired_text,
    };

    // ---- Per-token fuzzy correction on the repaired text. --------
    let vocab = CapcoTokenSet.correction_vocab();
    let matcher = FuzzyVocabMatcher::new(vocab);
    let (fuzzy_corrected, fuzzy_features) = fuzzy_correct_tokens(&repaired_text, &matcher);

    // Emit the straightforward "normalize + fuzzy-correct" attempt
    // first — this covers typos (T046) and case/delimiter mangling
    // by default.
    let mut features = delim_features.clone();
    features.extend(fuzzy_features.iter().copied());
    emit(
        fuzzy_corrected.as_bytes().to_vec(),
        features,
        marque_rules::FixSource::DecoderPosterior,
    );

    // ---- Also attempt a token-reorder pass. The reorder is gentle:
    //      inside each `//`-separated segment, if the segment's tokens
    //      look like they belong to multiple categories, we try a
    //      canonical category ordering (classification first).
    if let Some(reordered) = try_canonical_reorder(&fuzzy_corrected) {
        let mut features = delim_features.clone();
        features.extend(fuzzy_features.iter().copied());
        features.push(FeatureEntry {
            id: FeatureId::TokenReorder,
            delta: -0.4,
        });
        emit(
            reordered.into_bytes(),
            features,
            marque_rules::FixSource::DecoderPosterior,
        );
    }

    // ---- Non-US prefix insertion. For bare non-US markings that
    //      arrive with no `//` at all (e.g., `NS`, `JOINT S GBR USA`,
    //      `CAN S`), emit a `//{body}` candidate so the strict parser
    //      enters the non-US classification code path. The reorder pass
    //      above handles inputs that already contain `//` but are
    //      missing the leading empty-US-slot prefix.
    if let Some(prefixed) = try_add_non_us_prefix(&fuzzy_corrected) {
        let mut features = delim_features.clone();
        features.extend(fuzzy_features.iter().copied());
        features.push(FeatureEntry {
            id: FeatureId::TokenReorder,
            delta: -0.4,
        });
        emit(
            prefixed.into_bytes(),
            features,
            marque_rules::FixSource::DecoderPosterior,
        );
    }

    // ---- Missing-delimiter insertion (issue #133 PR 3). Walks the
    //      fuzzy-corrected text, inserts `//` at category-transition
    //      whitespace gaps. Tagged with `FixSource::DecoderPosterior`
    //      because the recovery is structural (missing punctuation),
    //      not a probabilistic guess like the classification heuristic
    //      below — auto-applies at default threshold when its strict
    //      parse + scoring outranks competing candidates.
    if let Some(delim_inserted) = try_insert_delimiter(&fuzzy_corrected) {
        let mut features = delim_features.clone();
        features.extend(fuzzy_features.iter().copied());
        // No FeatureId for delimiter insertion in the audit schema.
        // Reuse `BaseRateCommonMarking` with a small negative delta
        // to record that this attempt required cleanup beyond the
        // raw input — keeps the canonical-arrived-clean attempt
        // ranked higher when both produce the same shape.
        features.push(FeatureEntry {
            id: FeatureId::BaseRateCommonMarking,
            delta: -0.3,
        });
        emit(
            delim_inserted.into_bytes(),
            features,
            marque_rules::FixSource::DecoderPosterior,
        );
    }

    // ---- SAR indicator-keyword structural repair (issue #133 PR 6).
    //      Recovers `USAR-BP-J12...` (stray prefix on the SAR
    //      indicator) and `SARBP` (missing hyphen between indicator
    //      and program identifier). Same provenance / penalty story
    //      as `try_insert_delimiter`: a `BaseRateCommonMarking` delta
    //      records that the candidate required cleanup beyond raw
    //      input, so a canonical-arrived-clean candidate beats a
    //      SAR-repaired one with the same final shape.
    if let Some(sar_repaired) = try_sar_indicator_repair(&fuzzy_corrected) {
        let mut features = delim_features.clone();
        features.extend(fuzzy_features.iter().copied());
        features.push(FeatureEntry {
            id: FeatureId::BaseRateCommonMarking,
            delta: -0.3,
        });
        emit(
            sar_repaired.into_bytes(),
            features,
            marque_rules::FixSource::DecoderPosterior,
        );
    }

    // ---- Stray-character `/X/` recovery (issue #133 PR 7). Walks
    //      the fuzzy-corrected text looking for the pattern
    //      `<alnum>/<single_alnum_char>/<alnum>` — three transforms
    //      emitted per match (drop X, attach X to right token,
    //      attach X to left token). Step 3a's Unknown-token filter
    //      acts as the natural disambiguator: only the transform
    //      that produces a recognizable token survives. See
    //      [`try_collapse_stray_char_slash`] for the recovery
    //      shapes (`SI/U/NOFORN` → drop, `SI/N/OFORN` →
    //      right-attach, `SECRE/T/REL TO` → left-attach).
    for candidate in try_collapse_stray_char_slash(&fuzzy_corrected) {
        let mut features = delim_features.clone();
        features.extend(fuzzy_features.iter().copied());
        features.push(FeatureEntry {
            id: FeatureId::BaseRateCommonMarking,
            delta: -0.3,
        });
        emit(
            candidate.into_bytes(),
            features,
            marque_rules::FixSource::DecoderPosterior,
        );
    }

    // ---- REL TO trigraph fuzzy-priors expansion (issue #233).
    //      The standard fuzzy path in `fuzzy_correct_tokens` operates
    //      against `correction_vocab()`, which deliberately excludes
    //      country trigraphs (see the comment on `ALL_CVE_TOKENS` in
    //      `crates/ism/build.rs` and the design rationale in
    //      `EXTENDED_CORRECTION_VOCAB`). Trigraphs live in a separate
    //      `TRIGRAPHS` slice reached via `is_trigraph`. So an unknown
    //      3-char REL TO entry like `USB` doesn't get any fuzzy
    //      correction — the standard fuzzy walk has nothing to match
    //      against. The strict REL TO parser previously dropped
    //      unknown entries silently; issue #233 makes
    //      `parse_rel_to_with_spans` emit `TokenKind::Unknown` instead
    //      so the dispatcher's step 3a rejects the "drop USB"
    //      candidate.
    //
    //      With unknown entries no longer silently absorbed, the
    //      candidate set must include real trigraph alternates for
    //      the dispatcher to choose between. This block walks each
    //      `REL TO ` block, finds 3-char entries that aren't valid
    //      trigraphs, and emits one canonical-byte alternate per
    //      candidate from a fuzzy match against the TRIGRAPHS
    //      slice. The structural strict parse +
    //      `score_candidate` (which sums `country_code_log_prior`
    //      over the parsed `rel_to` slice) then picks the right
    //      winner: USA dominates UZB by ~7 nats, far above
    //      `UNAMBIGUOUS_LOG_MARGIN`.
    //
    //      Each alternate carries an `EditDistance1` /
    //      `EditDistance2` feature so the audit trail records the
    //      fuzzy work, plus a zero-delta `BaseRateCommonMarking`
    //      feature whose role is purely audit-trail provenance —
    //      "country-code priors were consulted on this candidate".
    //      The actual scoring weight comes from `score_candidate`
    //      summing `country_code_log_prior` over `attempt.rel_to`;
    //      adding a non-zero delta here would double-count. The
    //      other structural-cleanup paths in this file use `-0.3`
    //      because they have no parallel score-time prior to back
    //      them up; the trigraph path does, so the audit feature
    //      is informational only. No new `FeatureId` variant —
    //      adding one would bump the audit schema. Reusing
    //      `BaseRateCommonMarking` matches the variant's existing
    //      doc ("the candidate's base rate in the target corpus
    //      dominates the posterior").
    let trigraph_matcher = FuzzyVocabMatcher::new(marque_ism::TRIGRAPHS);
    for (alt_text, edit_feature) in
        try_rel_to_fuzzy_trigraph_candidates(&fuzzy_corrected, &trigraph_matcher)
    {
        let mut features = delim_features.clone();
        features.extend(fuzzy_features.iter().copied());
        features.push(edit_feature);
        // Trigraph-prior acknowledgement (see comment above for the
        // FeatureId reuse rationale + zero-delta justification).
        features.push(FeatureEntry {
            id: FeatureId::BaseRateCommonMarking,
            delta: 0.0,
        });
        emit(
            alt_text.into_bytes(),
            features,
            marque_rules::FixSource::DecoderPosterior,
        );
    }

    // ---- REL TO USA-injection for short first entries (issue #234 PR-B).
    //      Complementary to PR-A above: PR-A fuzzy-matches 3-char REL TO
    //      entries; PR-B handles 1-2 char first entries that are below
    //      `MIN_FUZZY_LEN`. The §H.8 p151 USA-first invariant gives us a
    //      strong structural signal that fuzzy matching cannot exploit
    //      on inputs that short — `SA → USA`, `S → USA`, etc. The
    //      `BaseRateCommonMarking` audit delta keeps the audit schema
    //      closed (no new `FeatureId` variant); see the doc on
    //      `try_rel_to_usa_injection_candidates` for the rationale.
    for (alt_text, prior_feature) in try_rel_to_usa_injection_candidates(&fuzzy_corrected) {
        let mut features = delim_features.clone();
        features.extend(fuzzy_features.iter().copied());
        features.push(prior_feature);
        emit(
            alt_text.into_bytes(),
            features,
            marque_rules::FixSource::DecoderPosterior,
        );
    }

    // ---- Position-aware classification heuristic (issue #133 PR 2).
    //      Runs LAST so the dedup-keep-first guard above lets a
    //      vocab-based attempt with the same canonical bytes win the
    //      provenance contest — the heuristic only "wins" when no
    //      vocab path produces the same shape.
    //
    //      Scoring intentionally adds NO `EditDistance1` penalty.
    //      The heuristic's value comes from RECOGNIZING a
    //      classification token where the vocab-only path would
    //      leave the slot as `classification: None`. The added prior
    //      contribution from the recognized classification (e.g.,
    //      `log_prior("TOP SECRET")`) is what should put the
    //      heuristic candidate ahead of the no-classification fuzzy
    //      fallback. An EditDistance penalty would push the
    //      heuristic candidate BELOW the no-classification candidate
    //      and the fuzzy one would win — defeating the heuristic's
    //      purpose. The audit-record provenance still distinguishes
    //      this path through `FixSource::DecoderClassificationHeuristic`.
    if let Some(heuristic_bytes) = try_classification_heuristic_fix(&fuzzy_corrected) {
        let mut features = delim_features.clone();
        features.extend(fuzzy_features.iter().copied());
        emit(
            heuristic_bytes.into_bytes(),
            features,
            marque_rules::FixSource::DecoderClassificationHeuristic,
        );
    }

    attempts
}

/// Diagnostic-only accessor exposing the canonicalized byte attempts
/// the decoder generates from `bytes`. Returns one byte string per
/// attempt, in emit order; feature traces and the internal
/// [`CanonicalAttempt`] type are deliberately not surfaced — the
/// diagnostic only needs the bytes the strict parser will see.
///
/// Gated by the `decoder-harness` feature so it does not appear in
/// production builds. The single consumer is
/// `crates/engine/tests/decoder_diagnostic.rs` (issue #133 root-cause
/// tracing). Calling the real [`generate_candidate_bytes`] eliminates
/// the drift class of bug a hand-rolled re-implementation in the
/// diagnostic would carry.
#[cfg(feature = "decoder-harness")]
pub fn diagnostic_canonical_attempts(bytes: &[u8]) -> Vec<Vec<u8>> {
    generate_candidate_bytes(bytes)
        .into_iter()
        .map(|a| a.bytes.into_vec())
        .collect()
}

/// Normalize delimiters and case on a trimmed input.
///
/// - Fullwidth slash variants (`∕∕`, `/ /`, ` / / `, spaced `//`) all
///   collapse to `//`.
/// - ASCII alphabetic characters are upper-cased; the CAPCO grammar
///   is case-sensitive uppercase (§D.1 p27 — banner line uppercase
///   syntax rule, applied uniformly to portions per §C.1 p25).
/// - Leading `(` and trailing `)` are preserved so portion detection
///   still works.
///
/// Returns the normalized string and the features that were applied.
/// When normalization was actually needed, a `BaseRateCommonMarking`
/// feature is recorded with a negative delta — the candidate pays a
/// small penalty for having required case- or delimiter-cleanup
/// rather than arriving in canonical form. A candidate that
/// normalized cleanly and also resolved its tokens via fuzzy
/// correction will still outrank a candidate that arrived dirty,
/// but a canonical-from-the-start candidate beats both.
fn normalize_delimiters_and_case(text: &str) -> (Cow<'_, str>, SmallVec<[FeatureEntry; 4]>) {
    // Order matters: multi-char sequences first so the longer patterns
    // win their byte ranges before the 2-char fallbacks consume them.
    // Sorted by `from.len()` descending so each pattern only fires on
    // residue its longer cousins didn't already match. Without this,
    // `"S / / NF"` would have the 4-byte `"/ / "` consume the spaces
    // before the 5-byte `" / / "` could see them, leaving a stray
    // `" //NF"` that the single forward pass below would not revisit.
    const REPLACEMENTS: &[(&str, &str)] = &[
        // fullwidth: 6 bytes
        ("∕∕", "//"),
        // 5 bytes
        (" / / ", "//"),
        // 4 bytes (∗ tied length, mutually disjoint)
        (" // ", "//"),
        ("/ / ", "//"),
        // 3 bytes (∗ tied length, mutually disjoint)
        ("// ", "//"),
        (" //", "//"),
        ("/ /", "//"),
    ];

    // Issue #452: short-circuit on the canonical-input common case. If
    // none of the delimiter patterns are present AND no ASCII lowercase
    // needs upper-casing, return the input borrowed with an empty
    // feature list — zero allocation on the hot path through the
    // decoder fallback.
    let need_delim = REPLACEMENTS.iter().any(|(from, _)| text.contains(from));
    // Pre-replacement scan vs. the prior post-replacement scan: equivalent
    // because every entry in `REPLACEMENTS` maps to `"//"` (no lowercase
    // ASCII byte introduced or removed by the delimiter substitution).
    // `bytes().any(is_ascii_lowercase)` is byte-level — equivalent to
    // `chars().any(is_ascii_lowercase)` for any byte sequence (the trait
    // method returns false for any non-ASCII byte, so multi-byte UTF-8
    // codepoints can't false-positive). If a future REPLACEMENTS entry
    // introduces or strips lowercase, move this scan after the
    // delimiter pass.
    let had_lowercase = text.bytes().any(|b| b.is_ascii_lowercase());

    if !need_delim && !had_lowercase {
        return (Cow::Borrowed(text), SmallVec::new());
    }

    let mut normalized = text.to_owned();
    if need_delim {
        // Fixpoint loop: a single forward pass over `REPLACEMENTS`
        // isn't enough for inputs where rule A's substitution creates
        // a shape rule B (earlier in the table) would now match. For
        // example, `"S / /NF"` (3-byte `"/ /"` at positions 2-4) first
        // collapses to `"S //NF"`, which only the 3-byte `" //"` rule
        // can finish — but `" //"` already ran above it. Iterate
        // until no rule fires; each iteration strictly shortens
        // `normalized` so this terminates in O(text.len()) iterations
        // worst case, and in practice ≤2 iterations for any input
        // that would have failed the single-pass shape.
        loop {
            let mut changed = false;
            for (from, to) in REPLACEMENTS {
                if normalized.contains(from) {
                    normalized = normalized.replace(from, to);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
    if had_lowercase {
        // Case normalization. If the input was all-lowercase or
        // mixed-case (Title Case), uppercasing is a significant
        // canonicalization the decoder flags (via the
        // `BaseRateCommonMarking` feature below) so the posterior
        // reflects that the candidate required cleanup.
        normalized = normalized.to_ascii_uppercase();
    }

    // Record a `BaseRateCommonMarking` feature with a penalty delta.
    // The feature doesn't fit into one of the sharper features
    // (`EditDistance*`, `TokenReorder`, `SupersededToken`), but it
    // flags that we had to massage the input — delimiters were
    // non-canonical, or case was wrong. A small negative delta means
    // a canonical-input candidate outranks an otherwise-equivalent
    // normalized one, which is the intent: "arrives clean" should be
    // preferred over "needed cleanup."
    let mut features: SmallVec<[FeatureEntry; 4]> = SmallVec::new();
    features.push(FeatureEntry {
        id: FeatureId::BaseRateCommonMarking,
        delta: -0.3,
    });

    (Cow::Owned(normalized), features)
}

/// Fuzzy-correct each whitespace/delimiter-separated token in `text`.
///
/// Tokens that are already canonical are passed through. Unknown
/// tokens are run through [`FuzzyVocabMatcher`]; if a correction is
/// unambiguous the replacement lands in the output and the appropriate
/// `EditDistance1`/`EditDistance2` feature is recorded. If no
/// correction is available, the token is dropped into the output
/// unchanged.
///
/// Note on pass-through safety: `marque_core::Parser` is lenient — it
/// does NOT reject the whole parse when an unknown token appears, it
/// emits the token as a `TokenKind::Unknown` span instead. So
/// dropping an uncorrectable token through this step does not by
/// itself reject the candidate. The decoder's outer loop
/// (`DecoderRecognizer::recognize` step 3a) checks for any Unknown
/// span on the strict-parse result and discards such candidates
/// before they reach scoring — that is where partial-canonicalization
/// candidates get filtered out.
///
/// Also consults [`SUPERSEDED_TOKEN_MAP`] for CAPCO-2016 retirement
/// pairs (currently just `COMINT` → `SI`), recording the
/// `SupersededToken` feature when triggered.
fn fuzzy_correct_tokens<'a>(
    text: &'a str,
    matcher: &FuzzyVocabMatcher<'_>,
) -> (Cow<'a, str>, SmallVec<[FeatureEntry; 4]>) {
    let mut features: SmallVec<[FeatureEntry; 4]> = SmallVec::new();

    // Issue #452: lazy-alloc output. `out` stays `None` while every
    // walked segment matches its source verbatim — when nothing in
    // the input needs correction (the common case through the
    // decoder fallback), the function returns `Cow::Borrowed(text)`
    // with zero string allocation. On the first segment that
    // changes, we allocate, prefill `out` with the text up to the
    // change point, and switch to the writing path.
    let mut out: Option<String> = None;
    let mut rest = text;
    // Byte offset into the ORIGINAL `text` of the start of `rest`.
    // Used to prefill `out` when the first change is detected.
    let mut pos: usize = 0;

    // We walk the text segment-by-segment, preserving the `//`,
    // `-`, `(`, `)`, `,`, and whitespace delimiters verbatim. Tokens
    // are the maximal runs of ASCII alphanumerics (plus `-` when it
    // appears between alphanumerics, to keep compounds like `SI-G`
    // intact).
    while !rest.is_empty() {
        // Take the non-token prefix (delimiters/whitespace/punct).
        let non_token_len = rest
            .chars()
            .take_while(|c| !is_token_char(*c))
            .map(|c| c.len_utf8())
            .sum::<usize>();
        if non_token_len > 0 {
            if let Some(buf) = out.as_mut() {
                buf.push_str(&rest[..non_token_len]);
            }
            rest = &rest[non_token_len..];
            pos += non_token_len;
            continue;
        }
        // Take the token: alnum + internal `-`.
        let token_len = scan_token(rest);
        if token_len == 0 {
            // Should not happen given the non-token prefix branch,
            // but guard against infinite loops on pathological input.
            // If we already switched to owned (a prior segment changed),
            // flush the unscanned suffix so the lazy-alloc shape doesn't
            // silently drop the tail relative to the previous unconditional-
            // String walker.
            if let Some(buf) = out.as_mut() {
                buf.push_str(rest);
            }
            break;
        }
        let (token, tail) = rest.split_at(token_len);
        rest = tail;

        // Helper: when a change is detected on this token, lazily
        // allocate `out` and prefill it with `text[..pos]` so the
        // already-walked verbatim prefix is preserved.
        macro_rules! ensure_owned_buf {
            () => {{
                if out.is_none() {
                    let mut buf = String::with_capacity(text.len());
                    buf.push_str(&text[..pos]);
                    out = Some(buf);
                }
                out.as_mut().expect("just allocated above")
            }};
        }

        // Case 1: exact superseded token (e.g., standalone `COMINT` → `SI`).
        if let Some(replacement) = SUPERSEDED_TOKEN_MAP
            .iter()
            .find(|&&(from, _)| from == token)
            .map(|&(_, to)| to)
        {
            let buf = ensure_owned_buf!();
            buf.push_str(replacement);
            features.push(FeatureEntry {
                id: FeatureId::SupersededToken,
                delta: -0.2,
            });
            pos += token_len;
            continue;
        }

        // Case 1b: embedded superseded token — the deprecated keyword
        // appears as a substring within a longer token. Handles compound
        // prefixes (`COMINT-G` → `SI-G`), embedded substitutions
        // (`UNCLASCOMINTFIED` → `UNCLASSIFIED`, `FRD-COMINTGMA 14` →
        // `FRD-SIGMA 14`, `SENCOMINTTIVE` → `SENSITIVE`). The token !=
        // from guard ensures the exact-match case above is the only path
        // for bare superseded tokens. CAPCO-2016 §H.4 p74.
        let embedded_replacement = SUPERSEDED_TOKEN_MAP
            .iter()
            .find(|&&(from, _)| token != from && token.contains(from))
            .map(|&(from, to)| token.replace(from, to));
        if let Some(replaced) = embedded_replacement {
            let buf = ensure_owned_buf!();
            buf.push_str(&replaced);
            features.push(FeatureEntry {
                id: FeatureId::SupersededToken,
                delta: -0.2,
            });
            pos += token_len;
            continue;
        }

        // Case 2: already canonical (known CVE token or trigraph).
        // Check this first so we don't run a vocab scan + edit-
        // distance pass on tokens we already recognize.
        if CapcoTokenSet.canonicalize(token).is_some() || CapcoTokenSet.is_trigraph(token) {
            if let Some(buf) = out.as_mut() {
                buf.push_str(token);
            }
            pos += token_len;
            continue;
        }

        // Case 3: fuzzy-correctable. Compute once and reuse; the
        // previous structure called `matcher.correct(token)` twice
        // on tokens that weren't already canonical, doubling the
        // vocab-scan cost on exactly the unknown-token hot path.
        if let Some(correction) = matcher.correct(token) {
            let buf = ensure_owned_buf!();
            buf.push_str(correction.token);
            // `FeatureId` is part of the audit-schema contract (see
            // `crates/rules/src/confidence.rs` and the
            // `MARQUE_AUDIT_SCHEMA` pin); a wildcard `_` arm on it
            // would silently absorb future-variant additions. Pair
            // each (id, delta) directly off `correction.distance` so
            // both arms are total over the only two outcomes the
            // outer guard permits (`distance > 0`, `distance <=
            // MAX_EDIT_DISTANCE = 2`).
            let feature = match correction.distance {
                // `correct` returns `None` for exact matches, so
                // `distance == 0` cannot reach here; `MAX_EDIT_DISTANCE
                // == 2` upstream caps `distance <= 2`.
                0 => None,
                1 => Some(FeatureEntry {
                    id: FeatureId::EditDistance1,
                    delta: -0.5,
                }),
                _ => Some(FeatureEntry {
                    id: FeatureId::EditDistance2,
                    delta: -1.2,
                }),
            };
            if let Some(entry) = feature {
                features.push(entry);
            }
            pos += token_len;
            continue;
        }

        // Case 4: unknown and uncorrectable. Pass through verbatim.
        // The strict parser will register this as a
        // `TokenKind::Unknown` span rather than failing the parse
        // outright, so the decoder's outer loop (step 3a of
        // `DecoderRecognizer::recognize`) is what filters the
        // resulting partial-canonicalization candidate out.
        if let Some(buf) = out.as_mut() {
            buf.push_str(token);
        }
        pos += token_len;
    }

    let cow = match out {
        Some(buf) => Cow::Owned(buf),
        None => Cow::Borrowed(text),
    };
    (cow, features)
}

/// Token characters: ASCII alphanumerics. `-` is handled by
/// [`scan_token`] as an internal separator.
fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

/// Scan a token starting at `text[0]`. Returns the token length in
/// bytes. A token is a run of alphanumerics, with internal `-` allowed
/// between alphanumerics to support compounds like `SI-G` and
/// `SAR-BP`.
fn scan_token(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let is_alnum = b.is_ascii_alphanumeric();
        let is_internal_hyphen =
            b == b'-' && i > 0 && i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphanumeric();
        if is_alnum || is_internal_hyphen {
            i += 1;
        } else {
            break;
        }
    }
    i
}

/// Map of CAPCO-2016-superseded tokens → their authoritative live
/// replacements. Each entry MUST cite a specific passage in
/// `crates/capco/docs/CAPCO-2016.md` (Constitution VIII). Adding an
/// entry without a verified citation is a correctness defect.
///
/// - `COMINT` → `SI`: CAPCO-2016 §H.4 p74 ("The COMINT title for the
///   Special Intelligence (SI) control system is no longer valid.")
///   inside §H.4 SCI Control System Markings.
const SUPERSEDED_TOKEN_MAP: &[(&str, &str)] = &[("COMINT", "SI")];

// ---------------------------------------------------------------------------
// Position-aware short-token classification heuristic (issue #133 PR 2)
// ---------------------------------------------------------------------------

/// Try to fix a malformed leading classification token using a
/// keyboard-proximity heuristic.
///
/// `MIN_FUZZY_LEN = 3` blocks the vocab-based fuzzy matcher from
/// running on 1- and 2-character tokens — `R`, `W`, `YS`, `XS` etc.
/// are too short for edit-distance to be reliable against the closed
/// vocabulary alone. But when such a token sits at the **leading
/// classification position** of a portion or banner marking, the
/// position itself is strong evidence: the user intended a
/// classification level, and the malformed token is almost certainly
/// keyboard-adjacent to a real one.
///
/// This helper applies a small keyboard-proximity table to the first
/// whitespace-separated token of the first `//`-separated segment.
/// It returns the corrected text (with the leading token replaced)
/// when a rule fires. Returns `None` when the leading token is
/// already canonical, longer than 2 chars, or doesn't match any
/// rule.
///
/// # Confidence
///
/// The decoder tags this attempt's [`CanonicalAttempt::fix_source`]
/// with [`FixSource::DecoderClassificationHeuristic`]. The engine
/// then (a) downgrades the diagnostic severity to
/// [`Severity::Warn`](marque_rules::Severity::Warn) — always-visible
/// in `--check`, exits non-zero — and (b) caps
/// [`Confidence::rule`](marque_rules::Confidence) at `0.80` so
/// `combined ≤ 0.80` stays below the default `confidence_threshold`
/// of `0.95`. The heuristic only auto-applies in `--fix` mode when
/// the user has explicitly lowered the threshold, opting into the
/// heuristic's bar of evidence.
///
/// # Rules (CAPCO-2016 §A.2 classification levels: U, R, C, S, TS)
///
/// Length is checked first — a 2-char token never reaches the 1-char
/// table. The keyboard-proximity sets are derived from the standard
/// QWERTY layout: keys physically adjacent to S (`A`, `W`, `E`, `Z`)
/// likely correspond to S typos; keys adjacent to T (`R`, `Y`, `H`,
/// `G`, `F`) likely correspond to T typos when followed by an
/// S-cluster character (so the pair maps to `TS`). The table is
/// intentionally narrow — wider sets produce more false positives
/// in normal prose.
///
/// **Length 3** (issue #133 PR 8) — exactly one mapping:
/// - `OTP` → `TOP` (T↔O transposition; standard Levenshtein dist 2,
///   blocked by `MIN_USEFUL_CONFIDENCE` for 3-char inputs at dist 2,
///   so the vocab path can't catch it even with `TOP` in vocab).
///
/// The 3-char rule is intentionally a single hardcoded mapping —
/// the dense 3-char trigraph vocab (`TON`, `TUR`, `TWN`, …, 289
/// entries) means a wider "all transpositions of TOP" rule
/// would generate too many false positives. Other corpus-attested
/// 3-char `TOP` typos (`TPP`, `UOP`) are at standard Levenshtein
/// dist 1 from the bare `TOP` in `EXTENDED_CORRECTION_VOCAB` and
/// recover via the vocab path; only transposition (which standard
/// Levenshtein scores as dist 2) needs the heuristic. See
/// [`try_3char_classification_heuristic`] for the implementation
/// and the `try_3char_classification_heuristic_only_matches_otp`
/// regression-pin for the narrow-scope policy.
///
/// **Length 2** (checked second):
/// - `[T, R, Y, H, G][A, W, E, Z, S]` → `TS` (e.g., `RS`, `YS`, `HE`)
/// - `[F][A, W, E, Z, S]` → `TS` (e.g., `FS`, `FE`)
/// - `TP` → `TOP` (issue #133 PR 8; corpus-attested keyboard typo
///   where the middle `O` was elided; bare `TP` has no other
///   canonical CAPCO meaning).
/// - `TO` → `TOP` (issue #133 PR 8; same family — trailing `P`
///   elided).
///
/// **Length 1**:
/// - `[A, W, E, Z]` → `S` (S-key neighbors; bare `S` is canonical)
/// - `[V, F]` → `C` (C-key neighbors; bare `C` is canonical)
/// - `[X]` → `S` (X is between C and S on QWERTY; default to the
///   higher classification per the issue #133 PR 2 design note)
///
/// **Length 4+**: returns `None`. Long-token typos benefit from the
/// vocab-based fuzzy matcher (4-char `TDOP`/`QTOP`/`TOPW` recover
/// to `TOP` at edit distance 1 via the standard fuzzy path now
/// that `TOP` lives in `EXTENDED_CORRECTION_VOCAB`); the
/// keyboard-proximity heuristic adds nothing here.
///
/// **Bare canonical**: returns `None` when the leading token is
/// already a known classification short form (`U`, `R`, `C`, `S`,
/// `TS`) OR the bare leading word `TOP` of the two-word
/// `TOP SECRET` classification. PR 8 added `TOP` to the canonical
/// short-circuit set because the new length-3 `OTP→TOP` heuristic
/// would otherwise have to walk the heuristic path on every
/// already-canonical `TOP SECRET//...` input. The strict parser
/// already accepts all of these. See
/// [`is_canonical_short_classification`] for the implementation.
///
/// # CAB markings
///
/// Returns `None` when `text` looks like a CAB (Classification
/// Authority Block) — those are keyed authority lines, not
/// classification-leading shapes, and the heuristic would emit
/// nonsense if applied. The check mirrors [`is_cab_head`].
fn try_classification_heuristic_fix(text: &str) -> Option<String> {
    // Skip CAB shapes — they don't have a leading classification token.
    if is_cab_head(text.as_bytes()) {
        return None;
    }

    // Strip portion-form parens (preserve them at output).
    let (open_paren, body, close_paren) = if text.starts_with('(') && text.ends_with(')') {
        ("(", &text[1..text.len() - 1], ")")
    } else {
        ("", text, "")
    };

    // First `//`-separated segment carries the leading classification.
    let first_seg_end = body.find("//").unwrap_or(body.len());
    let first_seg = &body[..first_seg_end];
    let after_first_seg = &body[first_seg_end..];

    // First whitespace-delimited token of that segment.
    let first_seg_trimmed_start = first_seg
        .char_indices()
        .find(|(_, c)| !c.is_whitespace())
        .map(|(i, _)| i)
        .unwrap_or(0);
    let leading_ws = &first_seg[..first_seg_trimmed_start];
    let after_leading_ws = &first_seg[first_seg_trimmed_start..];
    let token_end = after_leading_ws
        .find(char::is_whitespace)
        .unwrap_or(after_leading_ws.len());
    let first_token = &after_leading_ws[..token_end];
    let after_first_token = &after_leading_ws[token_end..];

    // Bare canonical → no fix needed.
    if is_canonical_short_classification(first_token) {
        return None;
    }

    // **Lone-input safety guard (issue #133 PR 4 / #176).** Skip the
    // heuristic when the input has no marking-shape signal beyond the
    // leading token — i.e., nothing after the first token within the
    // first segment AND no `//`-separated tail. The corpus measurement
    // committed at `tools/corpus-analysis/output/heuristic_frequencies.json`
    // validated heuristic confidence well above the acceptance
    // threshold only for the *in-context* case (trigger appears within
    // ~30 chars of `//` or a recognized vocab token). For lone inputs
    // the empirical FP rate against Enron body text is many orders of
    // magnitude higher — high-frequency triggers like `A` and `E` have
    // tens of thousands of unrestricted occurrences vs at most a few
    // hundred in marking-context, and a fix-and-warn that auto-applies
    // at default threshold would produce false positives on
    // parenthetical refs like `(A)` / `(W)` / `(F)` common in business
    // prose. Spot-check the evidence file directly for per-trigger
    // detail.
    //
    // Form-field input (`(YS)` typed into a portion-mark field)
    // SHOULD heuristic-fix at high confidence — the caller knows the
    // input is a marking attempt — but we don't yet have an input-
    // source signal to distinguish form-field from document-content.
    // Tracked in #176 (input-source signal on ParseContext); when
    // that lands, this safety guard becomes conditional on
    // `ParseContext::input_source == DocumentContent`.
    // Trailing whitespace doesn't count as "other content" — `(YS )`
    // is functionally equivalent to `(YS)` for the lone-case test.
    let has_other_marking_content = after_first_token.chars().any(|c| !c.is_whitespace())
        || after_first_seg.chars().any(|c| !c.is_whitespace());
    if !has_other_marking_content {
        return None;
    }

    let replacement = match first_token.len() {
        3 => try_3char_classification_heuristic(first_token)?,
        2 => try_2char_classification_heuristic(first_token)?,
        1 => try_1char_classification_heuristic(first_token)?,
        _ => return None,
    };

    Some(format!(
        "{open_paren}{leading_ws}{replacement}{after_first_token}{after_first_seg}{close_paren}"
    ))
}

/// True when `token` is a known CAPCO-2016 classification short
/// form (U, R, C, S, TS) OR the bare leading word of the
/// `TOP SECRET` two-word classification.
///
/// The full-word forms (UNCLASSIFIED, RESTRICTED, etc.) are
/// intentionally NOT matched here: a malformed full-word would
/// already be handled by the vocab-based fuzzy matcher (`SECRET`
/// is in `correction_vocab`).
///
/// Issue #133 PR 8 added `TOP` to the match set. Pre-PR-8 the
/// helper's whitespace tokenizer treated `TOP` as a non-canonical
/// token and the heuristic fired on perfectly-canonical
/// `TOP SECRET//...` input — a no-op when the heuristic returned
/// `None` for length-3 inputs, but a latent footgun once the
/// length-3 arm started returning `Some` (PR 8). Recognizing bare
/// `TOP` as canonical short-circuits the heuristic on the
/// already-correct case.
fn is_canonical_short_classification(token: &str) -> bool {
    matches!(token, "U" | "R" | "C" | "S" | "TS" | "TOP")
}

/// 2-char keyboard-proximity rule. Two mappings:
///
/// 1. T-cluster + S-cluster pair → `TS` (the original PR 2 rule).
/// 2. Specific `TP` / `TO` pair → `TOP` (issue #133 PR 8). These
///    are corpus-attested classification typos where the middle
///    `O` (`TP`) or trailing `P` (`TO`) was elided. Bare `TP` and
///    `TO` have no other canonical CAPCO meaning at the leading
///    classification position — `TP` isn't an SCI control or
///    dissem, `TO` isn't either (the `REL TO` keyword path lives
///    inside the structural REL TO parser, not here).
///
/// The TS rule is checked first; rule 2 only fires when rule 1
/// doesn't (so `TS` itself, which has T-cluster + S-cluster, would
/// already be marked canonical by `is_canonical_short_classification`
/// upstream and the heuristic doesn't run on it).
fn try_2char_classification_heuristic(token: &str) -> Option<&'static str> {
    let bytes = token.as_bytes();
    debug_assert_eq!(bytes.len(), 2);
    let first = bytes[0].to_ascii_uppercase();
    let second = bytes[1].to_ascii_uppercase();

    // T-key cluster: T itself plus QWERTY-adjacent keys (R, Y above-
    // adjacent on the home row; H, G, F on the row below). Wide
    // enough to catch the common transposition typos; narrow
    // enough to avoid touching unrelated 2-char prose.
    let t_cluster = matches!(first, b'T' | b'R' | b'Y' | b'H' | b'G' | b'F');
    // S-key cluster: S plus QWERTY-adjacent keys (A, W, E above-
    // adjacent on the upper row; Z below).
    let s_cluster = matches!(second, b'A' | b'W' | b'E' | b'Z' | b'S');

    if t_cluster && s_cluster {
        return Some("TS");
    }

    // PR 8: `TP` / `TO` → `TOP`. Tight pattern (literal pair, not
    // cluster) because broadening to e.g. `T[A-Z]` → `TOP` would
    // collide with too many real 2-char tokens in non-marking
    // prose. Anchored to T as the first byte and P / O as the
    // second.
    if first == b'T' && matches!(second, b'P' | b'O') {
        return Some("TOP");
    }

    None
}

/// 3-char keyboard-proximity rule (issue #133 PR 8). Maps a small
/// set of corpus-attested 3-char classification typos to their
/// canonical form when they appear in the leading classification
/// slot.
///
/// The vocab-based fuzzy matcher catches `TPP→TOP`, `UOP→TOP`, and
/// other distance-1 inputs once `TOP` lives in
/// `EXTENDED_CORRECTION_VOCAB`. This heuristic covers the residual
/// cases the fuzzy path can't reach:
///
/// - **`OTP` → `TOP`** — T↔O transposition. Standard Levenshtein
///   counts a transposition as 2 substitutions (distance 2), and
///   the fuzzy matcher's `MIN_USEFUL_CONFIDENCE` floor (0.45)
///   blocks distance-2 corrections for 3-char inputs (confidence
///   0.40). Switching the matcher to Damerau-Levenshtein would
///   recover this case but expand the false-positive surface
///   across the whole vocab; a targeted heuristic at the
///   classification slot is the lower-blast-radius fix.
///
/// Returns `None` for any other 3-char input — the heuristic is
/// intentionally narrow to avoid false positives in the dense
/// 3-char trigraph vocab (`TON`, `TUR`, `TWN`, …).
fn try_3char_classification_heuristic(token: &str) -> Option<&'static str> {
    let bytes = token.as_bytes();
    debug_assert_eq!(bytes.len(), 3);
    // Uppercase comparison is unnecessary here because the
    // `normalize_delimiters_and_case` pass upstream uppercases
    // ASCII before this helper runs, but we mirror the
    // length-1 / length-2 helpers' style for consistency.
    let upper = [
        bytes[0].to_ascii_uppercase(),
        bytes[1].to_ascii_uppercase(),
        bytes[2].to_ascii_uppercase(),
    ];
    if upper == *b"OTP" {
        return Some("TOP");
    }
    None
}

/// 1-char keyboard-proximity rule. Maps to S, C per the §A.2 short-
/// form classification ladder. See module-level table for the
/// per-character mapping rationale.
fn try_1char_classification_heuristic(token: &str) -> Option<&'static str> {
    let bytes = token.as_bytes();
    debug_assert_eq!(bytes.len(), 1);
    match bytes[0].to_ascii_uppercase() {
        b'A' | b'W' | b'E' | b'Z' => Some("S"),
        b'V' | b'F' => Some("C"),
        // X is between C and S on QWERTY; default to the higher
        // classification (S) per the issue #133 PR 2 design note —
        // false-negative cost (under-classified) > false-positive
        // cost (over-classified) for IC compliance work.
        b'X' => Some("S"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Missing-delimiter insertion (issue #133 PR 3)
// ---------------------------------------------------------------------------

/// Try to insert missing `//` segment separators at category-transition
/// boundaries.
///
/// CAPCO grammar requires `//` between segments —
/// `CLASSIFICATION//SCI_BLOCK//SAR_BLOCK//DISSEM_BLOCK`. Real-world
/// transcription frequently substitutes whitespace for one or more
/// `//` separators, producing inputs the strict parser cannot
/// recover (`SECRET//NOFORN EXDIS` strict-parses as
/// `classification: Secret, dissem: [Nf]` with `EXDIS` left as
/// `TokenKind::Unknown`; the decoder's step-3a Unknown-span filter
/// then discards the candidate).
///
/// This helper walks the input left-to-right and inserts `//` at
/// whitespace gaps that separate two distinct CAPCO segments. Two
/// rules drive insertion:
///
/// 1. **Classification → next segment.** Tokens at the start of the
///    input are classification-context (`U`, `R`, `C`, `S`, `TS`,
///    `UNCLASSIFIED`, …, plus the `TOP SECRET` two-word
///    classification). The first non-classification token after the
///    classification phrase, when no `//` has been emitted yet,
///    triggers `//` insertion before it. Covers the
///    `TOP SECRET HCS-P INTEL OPS//ORCON/NOFORN` / `SECRET REL TO
///    USA, AUS, GBR` family.
///
/// 2. **Hard-splitter dissem long-form.** A small set of unambiguous
///    long-form dissem control tokens (`NOFORN`, `ORCON`,
///    `ORCON-USGOV`, `PROPIN`, `IMCON`, `RELIDO`, `RSEN`,
///    `EYESONLY`, `EXDIS`, `NODIS`, `LIMDIS`, `FOUO`, `FISA`,
///    `DSEN`) ALWAYS start a new segment when they appear after a
///    whitespace gap, regardless of preceding context — these
///    tokens have no in-segment role inside SCI/SAR/REL TO
///    blocks. Covers the `NOFORN EXDIS` / `... SI NOFORN` /
///    `... HCS-P INTEL OPS ORCON/NOFORN` family. The full set is
///    pinned by [`is_hard_splitter_covers_documented_long_forms`].
///
/// Exceptions (do NOT insert):
///
/// - `SBU NOFORN` / `LES NOFORN` — non-IC dissem **banner long
///   forms** for `NonIcDissem::SbuNf` / `NonIcDissem::LesNf`. When
///   the previous token is `SBU` or `LES`, treat `NOFORN` as part
///   of the multi-word atom.
///
/// Returns `None` when no insertion was made — the caller should
/// not emit a duplicate of the input.
///
/// # Bounded
///
/// Hard-capped at [`MAX_DELIMITER_INSERTIONS`] insertions per call.
/// More than four insertions in a single marking is suspicious and
/// likely indicates the input isn't a CAPCO marking at all (or the
/// helper is wrong); rather than emit a wildly-rewritten candidate,
/// we cap and let the result strict-parse on the partial rewrite.
///
/// # SCI / SAR / SPECIAL-ACCESS-REQUIRED coverage
///
/// The PR-3-era doc note here used to defer SCI-starter (`TOP SECRET
/// SI ...`), SAR-prefix (`TOP SECRET SAR-BP ...`), and
/// `SPECIAL ACCESS REQUIRED-...` insertion to a follow-up. That defer
/// was based on a misread: rule 1 (classification → next segment)
/// already fires on every one of those shapes because
/// [`is_classification_token`] includes `TOP` and
/// [`is_classification_continuation`] handles the `TOP → SECRET`
/// special case, so the helper produces the canonical bytes for all
/// 17 MissingDelimiter fixtures in the SC-004 corpus. The remaining
/// 2/17 failures pre-PR-5 were a SCORING contest, not a missing
/// rewrite — handled by [`HARD_SPLITTER_ABSORPTION_PENALTY`] in
/// [`score_candidate`], not here.
fn try_insert_delimiter(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut result = String::with_capacity(text.len() + 8);
    let mut insertions = 0;

    let mut prev_token: Option<&str> = None;
    let mut in_classification = true;
    let mut seen_double_slash = false;

    let mut i = 0;
    while i < bytes.len() {
        // Existing `//` delimiter — copy and reset state.
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            result.push_str("//");
            seen_double_slash = true;
            in_classification = false;
            prev_token = None;
            i += 2;
            continue;
        }

        // Whitespace run — collect, then look at next token.
        if bytes[i].is_ascii_whitespace() {
            let ws_start = i;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let ws = &text[ws_start..i];

            // Find the next token (alnum + internal `-`) starting at `i`.
            let token_start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
                i += 1;
            }
            if token_start == i {
                // Whitespace then non-token character (e.g., `,` or `/` or end).
                // Just copy the whitespace and continue.
                result.push_str(ws);
                continue;
            }
            let next_token = &text[token_start..i];

            let should_insert = decide_insert_delimiter(
                prev_token,
                next_token,
                in_classification,
                seen_double_slash,
            );

            if should_insert && insertions < MAX_DELIMITER_INSERTIONS {
                result.push_str("//");
                insertions += 1;
                seen_double_slash = true;
                in_classification = false;
            } else {
                result.push_str(ws);
            }
            result.push_str(next_token);

            // Update state.
            if !is_classification_continuation(next_token, prev_token) {
                in_classification = false;
            }
            prev_token = Some(next_token);
            continue;
        }

        // Non-whitespace, non-`//` character — likely a `/` (single
        // slash, used as intra-segment separator e.g.
        // `ORCON/NOFORN`), comma, paren, or part of a token. Copy
        // verbatim and continue. Tokens that contain only alnum + `-`
        // are handled in the whitespace branch via the lookahead;
        // the leading-token-at-position-0 case enters here.
        let other_start = i;
        // Take a token (alnum + internal `-`) if at one.
        if bytes[i].is_ascii_alphanumeric() {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
                i += 1;
            }
            let leading_token = &text[other_start..i];
            result.push_str(leading_token);
            // Update prev_token / classification state for the
            // leading token (no insertion possible at position 0).
            if !is_classification_continuation(leading_token, prev_token) {
                in_classification = false;
            }
            prev_token = Some(leading_token);
            continue;
        }

        // Single non-token character (`/`, `(`, `)`, `,`, or any
        // non-ASCII character — e.g., a stray `∕` that the upstream
        // delimiter normalizer didn't catch). Preserve the original
        // UTF-8 character verbatim instead of doing `bytes[i] as
        // char`, which would corrupt multi-byte sequences by emitting
        // each byte as a separate Latin-1 codepoint.
        let ch = text[i..]
            .chars()
            .next()
            .expect("byte index must remain on a char boundary");
        result.push(ch);
        i += ch.len_utf8();
    }

    if insertions == 0 { None } else { Some(result) }
}

/// Hard cap on the number of `//` insertions per call. More than 4
/// in a single marking is very suspicious — real markings rarely
/// have that many segments at all. The cap prevents the helper
/// from rewriting non-marking prose that happens to contain
/// splitter words.
const MAX_DELIMITER_INSERTIONS: usize = 4;

/// Decide whether to insert `//` at a whitespace gap before
/// `next_token`. See [`try_insert_delimiter`] doc for the rules.
fn decide_insert_delimiter(
    prev_token: Option<&str>,
    next_token: &str,
    in_classification: bool,
    seen_double_slash: bool,
) -> bool {
    // Multi-word atom exceptions: don't split between SBU/LES and
    // their NOFORN companion (banner long forms for NonIcDissem
    // SbuNf/LesNf).
    if next_token == "NOFORN" && matches!(prev_token, Some("SBU") | Some("LES")) {
        return false;
    }

    // Rule 1: classification → next segment. The first non-
    // classification token after the classification phrase, when no
    // `//` has been emitted yet.
    if in_classification && !seen_double_slash && !is_classification_token(next_token) {
        return true;
    }

    // Rule 2: hard-splitter dissem long-form. These tokens always
    // start a new segment when they appear after whitespace.
    is_hard_splitter(next_token)
}

/// True when `token` is a classification short or long form that
/// can appear in classification context.
fn is_classification_token(token: &str) -> bool {
    matches!(
        token,
        "U" | "R"
            | "C"
            | "S"
            | "TS"
            | "TOP"
            | "UNCLASSIFIED"
            | "RESTRICTED"
            | "CONFIDENTIAL"
            | "SECRET"
    )
}

/// True when `next_token` continues the classification phrase from
/// `prev_token`. Specifically: `TOP SECRET` is the only multi-word
/// classification CAPCO recognizes; `SECRET` after `TOP` continues
/// the classification.
fn is_classification_continuation(next_token: &str, prev_token: Option<&str>) -> bool {
    if next_token == "SECRET" && prev_token == Some("TOP") {
        return true;
    }
    is_classification_token(next_token)
}

/// True when `token` is an unambiguous segment-starting dissem
/// long-form. These tokens have no in-segment role inside SCI / SAR /
/// REL TO blocks, so seeing one after whitespace always indicates a
/// missing `//` separator. Pinned by
/// `try_insert_delimiter_inserts_before_long_form_dissem`.
///
/// Excluded from this set:
///
/// - 2-char short forms (`NF`, `OC`, `PR`, `IMC`, `RS`) — could
///   collide with SAR compartment / sub-compartment naming.
/// - SCI starters (`SI`, `HCS`, `TK`, `KDK`) — 2-3 char tokens that
///   appear in compartment context.
/// - SAR prefixes (`SAR-*`) — handled in v2 with classification-
///   context lookahead.
fn is_hard_splitter(token: &str) -> bool {
    matches!(
        token,
        "NOFORN"
            | "ORCON"
            | "ORCON-USGOV"
            | "PROPIN"
            | "IMCON"
            | "RELIDO"
            | "RSEN"
            | "EYESONLY"
            | "FOUO"
            | "FISA"
            | "DSEN"
            | "EXDIS"
            | "NODIS"
            | "LIMDIS"
    )
}

// ---------------------------------------------------------------------------
// SAR indicator-keyword structural repair (issue #133 PR 6)
// ---------------------------------------------------------------------------

/// Repair stray-prefix and missing-hyphen mangling around the SAR
/// `SAR-` indicator (CAPCO-2016 §H.5 p100). Two structural patterns:
///
/// 1. **Prefix strip** — `<boundary>[A-Z]{1,3}SAR-` → `<boundary>SAR-`.
///    Strips ANY attached 1–3 letter ASCII-uppercase prefix before
///    the SAR indicator, including prefixes whose bytes happen to
///    spell a known CAPCO token (`U`, `S`, `SI`, `USA`, …). Canonical
///    CAPCO never glues a classification token, SCI control, or
///    trigraph directly to `SAR-` without a `//` separator, so a
///    prefix at a `//`/`(`/start boundary is OCR/transcription drift
///    regardless of whether the prefix bytes form a CVE token in
///    isolation. Recovers `SECRET//USAR-BP-J12...` →
///    `SECRET//SAR-BP-J12...` and `(USASAR-BP)` → `(SAR-BP)`. The
///    "smallest prefix that aligns with `SAR-`" wins (see
///    [`match_sar_prefix`]) so an ambiguous input like `USASAR-`
///    strips the longest aligning prefix (`USA`, length 3) — there
///    is no shorter alignment because `USASAR-` only contains `SAR-`
///    starting at offset 3. An earlier defensive guard that refused
///    to strip CAPCO-token prefixes was removed because it broke
///    the central `USAR-` case (`U` IS the UNCLASSIFIED portion
///    form); the test
///    `sar_indicator_repair_strips_even_capco_token_prefix` pins
///    the policy.
///
/// 2. **Missing-hyphen insertion** — `<boundary>SAR[A-Z0-9]{2,3}<delim>`
///    → `<boundary>SAR-[A-Z0-9]{2,3}<delim>`, where `<delim>` is `-`,
///    `/`, ASCII whitespace, or end-of-string. Recovers
///    `TOP SECRET//SARBP//NOFORN` → `TOP SECRET//SAR-BP//NOFORN` and
///    `SARBP-J12` → `SAR-BP-J12`.
///
/// Returns `None` when no change was made; the caller's `emit` dedup
/// would otherwise drop the duplicate candidate but the explicit
/// `None` saves the alloc.
///
/// # Why these patterns are structurally safe
///
/// Both patterns operate on the SAR **indicator keyword** (the literal
/// `SAR-` per §H.5 p100), not on the open-vocabulary program
/// identifier that follows. A prefix strip removes characters that
/// have no role in the CAPCO grammar — there is no marking syntax
/// where 1–3 alphabetic characters precede `SAR-` at a `//`/`(`/
/// start-of-string boundary. A missing-hyphen insertion adds the
/// syntactic separator the §H.5 grammar requires between the indicator
/// and the program identifier; it does not invent or modify the
/// identifier itself. Neither fix claims anything about SAR program-
/// identifier validity (which is agency-assigned and outside the
/// marque vocab — see `SAR_STRUCTURAL_KEYWORDS` in
/// `crates/ism/src/token_set.rs`). The corpus enhancement to fuzzy-
/// match against per-org SAR identifier lists is intentionally
/// deferred (issue follow-up): config-loaded vocab is a separate
/// trust boundary that needs its own design pass.
///
/// `SPECIAL ACCESS REQUIRED-` (the `Full` indicator form) is NOT
/// handled by this helper. The dominant `Full`-form failure mode in
/// the mangled corpus is a typo inside the indicator keywords
/// themselves (`SPCIAL`, `CCESS`, `SPECAL`), which is recovered by
/// the existing fuzzy matcher now that `SPECIAL` and `ACCESS` live in
/// `SAR_STRUCTURAL_KEYWORDS`. A `Full`-form analogue can land if a
/// future fixture surfaces with a stray prefix on
/// `SPECIAL ACCESS REQUIRED-`.
fn try_sar_indicator_repair(text: &str) -> Option<String> {
    // Cheap pre-check: if `SAR` doesn't appear at all, no repair is
    // possible. Saves the byte-walk cost on the overwhelmingly common
    // case where the input has no SAR block.
    if !text.contains("SAR") {
        return None;
    }

    let bytes = text.as_bytes();
    // Lazy allocation: `result` stays `None` until the first repair
    // pattern matches, at which point we allocate and copy the
    // verbatim prefix `text[..first_match_start]` into it. Inputs that
    // contain `SAR` but no repair-eligible pattern (the common case
    // for canonical SAR markings like `SECRET//SAR-BP//NOFORN`) walk
    // the bytes without ever allocating the output string. The
    // bytes-walk-only-no-alloc path matters because every candidate
    // bytes attempt the decoder generates calls into this helper, so
    // a per-call allocation would multiply allocator pressure across
    // the K candidates / N inputs hot path of the recognizer.
    let mut result: Option<String> = None;
    // `last_copied` is the byte index up to which `result` has been
    // populated. When a repair fires, we batch-copy the verbatim span
    // `text[last_copied..i]` into `result` before pushing the
    // canonical replacement; on the final return we flush
    // `text[last_copied..]`. The batch-copy approach also avoids the
    // per-character `chars().next()` UTF-8 iteration cost on the
    // verbatim-byte stretches.
    let mut last_copied: usize = 0;
    let mut i = 0;

    while i < bytes.len() {
        let at_boundary =
            i == 0 || matches!(bytes[i - 1], b'/' | b'(' | b' ' | b'\t' | b'\n' | b'\r');

        if at_boundary {
            // Pattern A: <prefix>SAR- where prefix is 1-3 ASCII
            // uppercase letters. The prefix is always treated as
            // noise to be stripped; a "known CAPCO word" defense
            // (refuse to strip if `U`, `USA`, `SI`, …) was tried
            // and rejected because it broke the central
            // `USAR-` case — `U` IS a CVE token (the
            // classification portion form for UNCLASSIFIED) but
            // canonical CAPCO never glues `U` directly to `SAR-`
            // without a `//` separator. Same logic applies to every
            // other CVE token in this position: a classification or
            // SCI control or trigraph that immediately precedes
            // `SAR-` with no separator is not a valid CAPCO marking
            // shape (the classification segment ends, `//` begins
            // the next segment, then SAR- starts the SAR block).
            // So an apparent prefix at a boundary directly before
            // `SAR-` is OCR/transcription drift regardless of
            // whether the prefix bytes spell a CAPCO token.
            if let Some((_prefix_len, post)) = match_sar_prefix(bytes, i) {
                let r = result.get_or_insert_with(|| String::with_capacity(text.len() + 4));
                r.push_str(&text[last_copied..i]);
                r.push_str("SAR-");
                last_copied = post;
                i = post;
                continue;
            }

            // Pattern B: SAR<2-3 alnum><delim>. The CAPCO §H.5 p100
            // SAR program identifier (Abbrev form) is exactly 2-3
            // alphanumeric characters; the canonical form requires a
            // hyphen between SAR and the identifier. Inserting that
            // hyphen does not invent identifier vocabulary.
            if let Some(end) = match_sar_missing_hyphen(bytes, i) {
                let r = result.get_or_insert_with(|| String::with_capacity(text.len() + 4));
                r.push_str(&text[last_copied..i]);
                r.push_str("SAR-");
                r.push_str(&text[i + 3..end]);
                last_copied = end;
                i = end;
                continue;
            }
        }

        // Default: advance past the current UTF-8 char without copying.
        // The verbatim span [last_copied..i] gets batch-copied into
        // `result` the next time a repair pattern fires (or flushed
        // on return below). Using char iteration rather than
        // `bytes[i] as char` keeps `i` aligned to char boundaries so
        // the `text[last_copied..i]` slice indexing is always valid
        // — multi-byte sequences (rare but possible in OCR'd input)
        // therefore round-trip intact.
        let ch = text[i..]
            .chars()
            .next()
            .expect("byte index must remain on a char boundary");
        i += ch.len_utf8();
    }

    // Flush any verbatim trailing span into the result. If `result`
    // is still `None`, no repair fired, and we never allocated —
    // return `None` to signal the no-op path.
    result.map(|mut r| {
        r.push_str(&text[last_copied..]);
        r
    })
}

/// At byte position `i`, look for `[A-Z]{1,3}SAR-`. Returns
/// `(prefix_len, post_index)` where `post_index` is the byte index
/// just after the `-` of `SAR-`. Returns `None` when the pattern
/// doesn't match.
///
/// Tries prefix lengths 1, 2, 3 in order; the **smallest** prefix
/// that aligns with a literal `SAR-` wins. The smallest-wins policy
/// is a conservative choice: a 1-char prefix (`U` in `USAR-`) is the
/// most likely OCR/transcription drift, and stripping fewer characters
/// is the lower-risk repair when the input is ambiguous between
/// shorter and longer prefix interpretations.
fn match_sar_prefix(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    for prefix_len in 1..=3 {
        let sar_start = i + prefix_len;
        if sar_start + 4 > bytes.len() {
            break;
        }
        if !bytes[i..sar_start].iter().all(|b| b.is_ascii_uppercase()) {
            break;
        }
        if &bytes[sar_start..sar_start + 4] == b"SAR-" {
            return Some((prefix_len, sar_start + 4));
        }
    }
    None
}

/// At byte position `i`, look for `SAR[A-Z0-9]{2,3}<delim>`. Returns
/// the byte index of the delimiter (one past the alphanumeric run).
/// Returns `None` when the pattern doesn't match — including the
/// canonical `SAR-` shape (alnum run is 0 because `-` stops the scan
/// immediately after `SAR`).
fn match_sar_missing_hyphen(bytes: &[u8], i: usize) -> Option<usize> {
    if i + 3 > bytes.len() || &bytes[i..i + 3] != b"SAR" {
        return None;
    }
    let after_sar = i + 3;
    let mut j = after_sar;
    while j < bytes.len() && bytes[j].is_ascii_alphanumeric() {
        j += 1;
    }
    let run = j - after_sar;
    if !(2..=3).contains(&run) {
        return None;
    }
    let next_is_delim =
        j == bytes.len() || matches!(bytes[j], b'-' | b'/' | b' ' | b'\t' | b'\n' | b'\r');
    if !next_is_delim {
        return None;
    }
    Some(j)
}

// ---------------------------------------------------------------------------
// Stray-character `/X/` recovery (issue #133 PR 7)
// ---------------------------------------------------------------------------

/// Walk `text` looking for the `<alnum>/<single_alnum_char>/<alnum>`
/// pattern. For each match (currently only the first match is
/// processed — see "scope" below) emit three candidate transforms:
///
/// 1. **Drop X** — `A/X/B` → `A//B`. Recovers stray characters
///    inserted between two valid tokens. Example:
///    `SECRET//NOFORN/R/EXDIS` → `SECRET//NOFORN//EXDIS` (the stray
///    `/R/` between NOFORN and EXDIS is removed).
///
/// 2. **Right-attach X** — `A/X/B` → `A//XB`. Recovers a single
///    character that got separated from the start of the right
///    token by a `/`. Example: `TOP SECRET//SI/N/OFORN` →
///    `TOP SECRET//SI//NOFORN` (the `N` was the leading character
///    of `NOFORN`).
///
/// 3. **Left-attach X** — `A/X/B` → `AX//B`. Recovers a single
///    character that got separated from the end of the left token
///    by a `/`. Example: `SECRE/T/REL TO USA, AUS, GBR` →
///    `SECRET//REL TO USA, AUS, GBR` (the `T` was the trailing
///    character of `SECRET`).
///
/// All three transforms are emitted as candidates; the recognizer's
/// step-3a [`TokenKind::Unknown`](marque_ism::TokenKind::Unknown)
/// filter is the natural disambiguator. For each input only one of
/// the three transforms produces fully-recognized tokens — the
/// other two leave broken-token fragments (`OFORN`, `NOFORNR`,
/// `SECRER`, …) that survive strict parsing as `TokenKind::Unknown`
/// and get dropped before scoring. The decoder doesn't need a
/// per-pattern lookup table to choose the right transform; the
/// vocab does the choosing implicitly.
///
/// # Scope (PR 7)
///
/// Only the FIRST `/X/` match in the input is processed; an input
/// with multiple stray-character patterns (e.g., `S/I/T/K`) is not
/// fully recovered by a single pass. The current corpus has very
/// few multi-pattern inputs (1–2 in the unresolved Typo set), and
/// adding a multi-pass loop here would complicate the candidate cap
/// in [`generate_candidate_bytes`] without proportional benefit. A
/// future PR can iterate if multi-pattern recovery becomes
/// load-bearing for SC-004 movement.
///
/// # Pattern boundary requirements
///
/// The `/X/` match requires alphanumeric context on both sides
/// (`<alnum>/<X>/<alnum>`). Without those guards the pattern would
/// fire on edge cases like `(/X/)` (start of portion form) where
/// the surrounding context is structural punctuation, not a token —
/// the recovery would be semantically meaningless there because
/// there's no token to attach `X` to.
fn try_collapse_stray_char_slash(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        // `/X/` shape: bytes[i] = `/`, bytes[i+1] = single ASCII
        // alnum, bytes[i+2] = `/`. The single-alnum requirement
        // prevents matching on `/AB/` (which would be a 2-char
        // token between slashes, not a stray character).
        if bytes[i] != b'/' || !bytes[i + 1].is_ascii_alphanumeric() || bytes[i + 2] != b'/' {
            i += 1;
            continue;
        }
        // Boundary check: the slashes must be sandwiched between
        // alphanumeric tokens on both sides. Without this guard
        // `(/X/)` (start-of-portion-form) would trip the match.
        let prev_alnum = i > 0 && bytes[i - 1].is_ascii_alphanumeric();
        let next_alnum = i + 3 < bytes.len() && bytes[i + 3].is_ascii_alphanumeric();
        if !prev_alnum || !next_alnum {
            i += 1;
            continue;
        }

        let x = bytes[i + 1];
        let prefix = &bytes[..i];
        let suffix = &bytes[i + 3..];

        // The unwraps are safe: `text` is valid UTF-8, `prefix` /
        // `suffix` are slices on byte boundaries (the pattern only
        // matched on ASCII bytes), and we only insert ASCII bytes
        // (`/`, `x` which is ASCII alnum) between them.
        let mut out = Vec::with_capacity(3);

        // 1. Drop X.
        let mut buf = Vec::with_capacity(bytes.len());
        buf.extend_from_slice(prefix);
        buf.extend_from_slice(b"//");
        buf.extend_from_slice(suffix);
        out.push(String::from_utf8(buf).expect("ASCII insertions on UTF-8 prefix/suffix"));

        // 2. Right-attach X.
        let mut buf = Vec::with_capacity(bytes.len());
        buf.extend_from_slice(prefix);
        buf.extend_from_slice(b"//");
        buf.push(x);
        buf.extend_from_slice(suffix);
        out.push(String::from_utf8(buf).expect("ASCII insertions on UTF-8 prefix/suffix"));

        // 3. Left-attach X.
        let mut buf = Vec::with_capacity(bytes.len());
        buf.extend_from_slice(prefix);
        buf.push(x);
        buf.extend_from_slice(b"//");
        buf.extend_from_slice(suffix);
        out.push(String::from_utf8(buf).expect("ASCII insertions on UTF-8 prefix/suffix"));

        return out;
    }
    Vec::new()
}

// ---------------------------------------------------------------------------
// REL TO structural repair (issue #133 PR 9)
// ---------------------------------------------------------------------------

/// REL TO structural repair.
///
/// Recovers four classes of REL TO structural typos that produce no
/// valid REL TO block in the strict parse path. All four are
/// **structural** (literal-shape) repairs, not vocabulary-based fuzzy
/// guesses — they fire only when the observed pattern is invalid
/// CAPCO AND the corrected pattern is unambiguously the intended form.
/// The riskier per-trigraph fuzzy-correction cluster (e.g.,
/// `USB → USA`, `AUT → AUS`) is deferred to issue #186 because it
/// requires corpus-weighted priors + block-level CAPCO §H.8
/// invariants to disambiguate safely.
///
/// # Patterns
///
/// 1. **Header transposition** — `REL OT ` → `REL TO `. The CAPCO
///    `REL` token has exactly two valid extensions (`REL TO` and
///    `RELIDO`); `REL OT` cannot appear in any valid CAPCO marking,
///    so the literal-bytes replacement is collision-free.
///
/// 2. **Header token-boundary** — `RELT O ` → `REL TO `. `RELT` is
///    not a CVE token, and `T O` as adjacent single-letter tokens
///    has no valid CAPCO meaning. The replacement reconstructs the
///    intended `REL TO ` header by migrating the trailing `T` from
///    `RELT` to the start of `O`.
///
/// 3. **Entry token-boundary** — `,A US,` → `,AUS,` (within a
///    REL TO block). A 1-letter + space + 2-letter sequence between
///    commas only fires when the joined 3-letter string is a known
///    trigraph (`is_trigraph` check) AND the 1-letter alone is not a
///    trigraph. The trigraph guard is what makes this safe — without
///    it, `,A B,` → `,AB,` would fire for any combination, but with
///    it the only joins that survive are those that round-trip
///    through the strict REL TO parser as valid trigraphs.
///
/// 4. **Entry comma misplacement** — `AU,S ` → `AUS, ` (within a
///    REL TO block). A 2-letter run + comma + 1-letter + space only
///    fires when the joined 3-letter string is a known trigraph AND
///    the 2-letter run alone is not. Same trigraph guard as
///    pattern 3 — the structural transform requires the corrected
///    output to be a valid trigraph.
///
/// # Scope (PR 9)
///
/// Patterns 1 and 2 affect the literal `REL TO` header and run
/// regardless of what follows. Patterns 3 and 4 require a `REL TO `
/// header in the input — they scan from each `REL TO ` substring
/// forward to the next `//` (or end of text) and only operate on
/// comma-separated entries within that block.
///
/// All four transforms are conservative: their false-positive risk
/// is bounded by the literal patterns not appearing in any valid
/// CAPCO text (patterns 1, 2) or by the `is_trigraph` guard
/// rejecting joins that aren't real country codes (patterns 3, 4).
/// The trigraph dictionary itself is the source of authority — no
/// new vocabulary is invented.
///
/// Returns `None` when no pattern matched. Allocation behavior:
///
/// - Inputs with no `REL` substring short-circuit before any work.
/// - Inputs with `REL` but no header-typo pattern run the header
///   walk allocation-free; the entry-level pass then short-circuits
///   on inputs lacking a literal `REL TO ` anchor.
/// - Inputs containing `REL TO ` in canonical form walk the entries
///   without allocating until a fix actually fires.
///
/// Allocation only occurs once a pattern produces a fixed string.
fn try_rel_to_structural_repair(text: &str) -> Option<String> {
    // Cheap pre-check: if `REL` doesn't appear at all, no repair is
    // possible. Saves the byte-walk cost on the overwhelmingly common
    // case where the input has no REL block.
    if !text.contains("REL") {
        return None;
    }

    let mut working: Option<String> = None;
    let mut any_change = false;

    // Patterns 1 and 2: header normalization. Apply first so the
    // entry-level scan in patterns 3 and 4 sees a canonical `REL TO `
    // header to anchor on.
    if let Some(normalized) = try_rel_to_header_normalize(text) {
        working = Some(normalized);
        any_change = true;
    }

    // Patterns 3 and 4: entry-level fixes. Operate on the
    // header-normalized text when patterns 1 or 2 fired, otherwise on
    // the raw input.
    let entry_input: &str = working.as_deref().unwrap_or(text);
    if let Some(entry_fixed) = try_rel_to_entry_normalize(entry_input) {
        working = Some(entry_fixed);
        any_change = true;
    }

    if any_change { working } else { None }
}

/// Patterns 1 and 2 — header normalization.
///
/// Walks `text` once, replacing each occurrence of `REL OT ` and
/// `RELT O ` (each at a token boundary) with `REL TO `. Lazy-allocates
/// the output string only on the first match — inputs that contain
/// `REL` but no header-typo pattern (the common case for canonical
/// `REL TO USA, AUS, GBR` markings) walk the bytes without ever
/// allocating.
///
/// The "token boundary" check (`at_boundary`) prevents matches
/// embedded inside a longer alphanumeric run. Without it `XREL OT`
/// would match the substring `REL OT` even though the leading `X`
/// makes the whole thing a single 6-character token, not a `REL`
/// header at all.
fn try_rel_to_header_normalize(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut result: Option<String> = None;
    let mut last_copied: usize = 0;
    let mut i = 0;

    while i < bytes.len() {
        let at_boundary =
            i == 0 || matches!(bytes[i - 1], b'/' | b'(' | b' ' | b'\t' | b'\n' | b'\r');

        if at_boundary && i + 7 <= bytes.len() {
            let window = &bytes[i..i + 7];
            // Pattern A (transposition): `REL OT ` → `REL TO `.
            // Pattern B (token-boundary): `RELT O ` → `REL TO `.
            // Both patterns are exactly 7 bytes; the same 7-byte
            // window is compared against each full literal
            // explicitly, so a single window read covers both.
            if window == b"REL OT " || window == b"RELT O " {
                let r = result.get_or_insert_with(|| String::with_capacity(text.len()));
                r.push_str(&text[last_copied..i]);
                r.push_str("REL TO ");
                last_copied = i + 7;
                i = last_copied;
                continue;
            }
        }

        let ch = text[i..]
            .chars()
            .next()
            .expect("byte index must remain on a char boundary");
        i += ch.len_utf8();
    }

    result.map(|mut r| {
        r.push_str(&text[last_copied..]);
        r
    })
}

/// Patterns 3 and 4 — entry-level normalization within REL TO blocks.
///
/// Scans `text` for each `REL TO ` substring and processes the
/// comma-separated entries that follow until the next `//` (or end of
/// text). Two patterns apply per entry pair:
///
/// - **Token-boundary** — within a single entry, `<single-upper> <two-upper>`
///   is replaced with the joined 3-letter trigraph when the join is a
///   known trigraph and the 1-letter prefix alone is not.
///
/// - **Comma misplacement** — across an entry pair,
///   `<2-upper>,<1-upper><space>...` (entry N ends with two letters,
///   entry N+1 starts with one letter followed by a space and then
///   content) is replaced with `<3-upper joined>,` and the leading
///   character is stripped from entry N+1, when the join is a known
///   trigraph and the 2-letter prefix alone is not. The space guard
///   (the 1-upper must be followed by ASCII space) is what
///   distinguishes the misplacement shape from a legitimate
///   shorter-than-3 entry typo and is enforced by `fix_rel_to_block`.
///
/// Both patterns require the corrected output to be a known trigraph
/// (`CapcoTokenSet::is_trigraph`). The trigraph dictionary is the
/// arbiter of "valid country code" — no fuzzy guessing.
fn try_rel_to_entry_normalize(text: &str) -> Option<String> {
    // Cheap pre-check: entry-level patterns 3 and 4 only fire inside a
    // `REL TO ` block, so `apply_rel_to_entry_pass` cannot match
    // without that anchor. Skip the `to_owned()` allocation entirely
    // when the input has no `REL TO ` substring (the common path for
    // canonical inputs and for non-REL-TO segments of the broader
    // structural-repair caller).
    if !text.contains("REL TO ") {
        return None;
    }

    let token_set = CapcoTokenSet;
    let mut any_change = false;
    let mut current: Option<String> = None;

    // Loop until no further fix fires. Most inputs converge in one
    // pass; the loop guards against the rare case where fixing one
    // pattern exposes another (e.g., a comma misplacement that ends a
    // block adjacent to a token-boundary pattern in the next entry).
    // First iteration borrows `text`; subsequent iterations re-pass the
    // previously rewritten `String` so the only allocation is the one
    // produced by the first successful fix (and any further passes).
    loop {
        let input: &str = current.as_deref().unwrap_or(text);
        match apply_rel_to_entry_pass(input, &token_set) {
            Some(rewritten) => {
                current = Some(rewritten);
                any_change = true;
            }
            None => break,
        }
    }

    if any_change { current } else { None }
}

/// Single pass of REL TO entry normalization. Returns the rewritten
/// text on first fix, or `None` if no pattern matched.
fn apply_rel_to_entry_pass(text: &str, token_set: &CapcoTokenSet) -> Option<String> {
    let mut search_start = 0;
    while let Some(rel_pos) = text[search_start..].find("REL TO ") {
        let header_end = search_start + rel_pos + "REL TO ".len();
        // Block ends at the next `//` (start of next category) or end
        // of text. The `//` boundary is always 2 bytes; we exclude it
        // from the block contents.
        let block_end = text[header_end..]
            .find("//")
            .map(|p| header_end + p)
            .unwrap_or(text.len());
        let block = &text[header_end..block_end];

        if let Some((rel_local_offset, fixed_block)) = fix_rel_to_block(block, token_set) {
            let mut result = String::with_capacity(text.len());
            result.push_str(&text[..header_end]);
            result.push_str(&fixed_block);
            result.push_str(&text[block_end..]);
            // Suppress unused-variable warning when the helper returns
            // a fix — `rel_local_offset` is reserved for a future
            // localized-emit optimization but not needed today since
            // we rebuild the full text.
            let _ = rel_local_offset;
            return Some(result);
        }

        search_start = block_end;
    }
    None
}

/// Walk the comma-separated entries of one REL TO block; apply
/// pattern 3 (token-boundary inside an entry) and pattern 4 (comma
/// misplaced between adjacent entries) on first match. Returns
/// `(local_offset, rewritten_block)` for the first fix, or `None` if
/// the block is already canonical.
///
/// `local_offset` is the byte offset within `block` where the fix
/// landed; reserved for future localized emit optimizations.
fn fix_rel_to_block(block: &str, token_set: &CapcoTokenSet) -> Option<(usize, String)> {
    // Collect entries with their byte offsets within the block so a
    // fix can be emitted with precise positioning.
    let mut entries: Vec<(usize, &str)> = Vec::new();
    let mut cursor = 0;
    for entry in block.split(',') {
        entries.push((cursor, entry));
        cursor += entry.len() + 1; // +1 for the comma separator
    }

    // Pattern 3: token-boundary inside a single entry.
    // `<lead-ws><single-upper> <two-upper><trail-ws>` → joined trigraph.
    for (entry_offset, entry) in &entries {
        let trimmed = entry.trim();
        // Need exactly 4 chars: `A US` shape. Anything else (3, 5, etc.)
        // is either canonical or a different recovery shape.
        if trimmed.len() != 4 {
            continue;
        }
        let bytes = trimmed.as_bytes();
        if !bytes[0].is_ascii_uppercase()
            || bytes[1] != b' '
            || !bytes[2].is_ascii_uppercase()
            || !bytes[3].is_ascii_uppercase()
        {
            continue;
        }
        let joined = format!(
            "{}{}{}",
            bytes[0] as char, bytes[2] as char, bytes[3] as char
        );
        if !token_set.is_trigraph(&joined) {
            continue;
        }
        // Defensive: don't fire if the 1-letter prefix is itself a
        // trigraph (no real CAPCO trigraph is 1-letter, but guard
        // anyway against future schema changes).
        let one_letter = std::str::from_utf8(&bytes[..1]).expect("ASCII upper");
        if token_set.is_trigraph(one_letter) {
            continue;
        }

        // Rebuild the block: replace the 4-char entry contents with
        // the 3-char joined trigraph, preserving any leading/trailing
        // whitespace inside the entry.
        // entry = lead_ws + trimmed + trail_ws; replace `trimmed`
        // (4 chars) with `joined` (3 chars), preserving the
        // surrounding whitespace verbatim.
        let lead_ws_len = entry.len() - entry.trim_start().len();
        let mut rewritten_entry = String::with_capacity(entry.len() - 1);
        rewritten_entry.push_str(&entry[..lead_ws_len]);
        rewritten_entry.push_str(&joined);
        rewritten_entry.push_str(&entry[lead_ws_len + trimmed.len()..]);

        let mut result = String::with_capacity(block.len());
        result.push_str(&block[..*entry_offset]);
        result.push_str(&rewritten_entry);
        result.push_str(&block[*entry_offset + entry.len()..]);
        return Some((*entry_offset, result));
    }

    // Pattern 4: comma misplaced between entries.
    // entries[i] = `<2-upper>` (trimmed) AND
    // entries[i+1] = `<1-upper><space><rest>` (trimmed) AND
    // joined 3-letter is a trigraph AND 2-letter alone is not.
    for i in 0..entries.len().saturating_sub(1) {
        let (left_off, left_entry) = &entries[i];
        let (right_off, right_entry) = &entries[i + 1];
        let left_trim = left_entry.trim();
        let right_trim_start = right_entry.trim_start();
        if left_trim.len() != 2 || !left_trim.chars().all(|c| c.is_ascii_uppercase()) {
            continue;
        }
        let right_bytes = right_trim_start.as_bytes();
        if right_bytes.len() < 2 || !right_bytes[0].is_ascii_uppercase() || right_bytes[1] != b' ' {
            continue;
        }
        let joined = format!("{}{}", left_trim, right_bytes[0] as char);
        if !token_set.is_trigraph(&joined) {
            continue;
        }
        if token_set.is_trigraph(left_trim) {
            // 2-letter alone is already a trigraph (e.g., EU); the
            // comma might be intentional. Skip.
            continue;
        }

        // Rebuild: left entry becomes `<lead-ws><joined>`, right
        // entry becomes ` <rest-after-first-char-and-space>` (we
        // strip the first char and the space, prepend a single
        // canonical space).
        let left_lead = left_entry.len() - left_entry.trim_start().len();
        let mut new_left = String::with_capacity(left_entry.len() + 1);
        new_left.push_str(&left_entry[..left_lead]);
        new_left.push_str(&joined);

        let right_lead = right_entry.len() - right_trim_start.len();
        // Skip the first char and the following space.
        let after_first = &right_trim_start[2..];
        let mut new_right = String::with_capacity(right_entry.len());
        new_right.push_str(&right_entry[..right_lead]);
        new_right.push(' ');
        new_right.push_str(after_first);

        // Emit: block[..left_off] + new_left + ',' + new_right + block[right_off+right_entry.len()..]
        let mut result = String::with_capacity(block.len() + 1);
        result.push_str(&block[..*left_off]);
        result.push_str(&new_left);
        result.push(',');
        result.push_str(&new_right);
        result.push_str(&block[*right_off + right_entry.len()..]);
        return Some((*left_off, result));
    }

    None
}

// ---------------------------------------------------------------------------
// REL TO trigraph fuzzy expansion (issue #233)
// ---------------------------------------------------------------------------

/// Emit one canonical-byte alternate per fuzzy candidate for each
/// unknown 3- or 4-char REL TO entry.
///
/// The standard fuzzy path in [`fuzzy_correct_tokens`] operates against
/// the [`CapcoTokenSet::correction_vocab`] slice, which deliberately
/// excludes country trigraphs (the design comment on `ALL_CVE_TOKENS`
/// in `crates/ism/build.rs` calls this out — country codes live
/// exclusively in [`marque_ism::TRIGRAPHS`] and are reached through
/// [`CapcoTokenSet::is_trigraph`]). So a typo'd 3-char REL TO entry
/// like `USB` gets no correction from the standard pass — there's
/// nothing in the vocab to match it against. The strict parser then
/// emits a `TokenKind::Unknown` for the entry (issue #233 change in
/// `parse_rel_to_with_spans`), and the dispatcher's step 3a rejects
/// the "drop USB" candidate.
///
/// With the original candidate filtered out, this function provides
/// the alternates the dispatcher chooses between: it walks each
/// `REL TO ` block in `text`, finds 3- or 4-char comma-separated
/// entries that aren't already valid trigraphs/tetragraphs, asks the
/// trigraph-vocab matcher for all candidates within the edit-distance
/// bound, and emits one alternate text per candidate (with the
/// substitution applied in-place).
///
/// Each emitted alternate carries an `EditDistance1` /
/// `EditDistance2` feature (paired with the candidate's distance) so
/// the audit trail records the fuzzy work. The caller pushes a
/// `BaseRateCommonMarking` feature acknowledging the trigraph-prior
/// contribution. The decoder's `score_candidate` later sums the
/// trigraph-prior contribution over the parsed `rel_to` slice; the
/// popular-vs-rare log-prior delta (e.g., `log_prior(USA) -
/// log_prior(UZB)` ≈ +7 nats) decides which alternate wins the
/// `UNAMBIGUOUS_LOG_MARGIN` (~1.6 nat) contest.
///
/// **Scope**: 3-char (trigraph) and 4-char (tetragraph) ASCII
/// uppercase entries only. Two-letter entries (`EU`) are below
/// `MIN_FUZZY_LEN`; longer multi-char entries (`AUSTRALIA_GROUP`)
/// have low fuzzy-tie risk because their lengths rarely collide.
/// Only fires when the entry token is NOT already a valid
/// trigraph/tetragraph — so `AUT`, `UZB`, `FVEY`, `ACGU`, `ISAF`
/// in legitimate use pass through unchanged. 4-char scope added to
/// recover coalition-shorthand typos (`FVYE` → `FVEY`,
/// `SGAF` → `ISAF`); issue #246.
///
/// **CAPCO authority**: REL TO syntax is defined in CAPCO-2016 §H.8.
/// The trigraph/tetragraph dictionary itself comes from the ODNI CVE
/// schema in `CVEnumISMCATRelTo.xsd`, baked into
/// [`CapcoTokenSet::is_trigraph`] and into the
/// [`marque_ism::TRIGRAPHS`] slice this function fuzzy-matches against.
fn try_rel_to_fuzzy_trigraph_candidates(
    text: &str,
    trigraph_matcher: &FuzzyVocabMatcher<'_>,
) -> Vec<(String, FeatureEntry)> {
    let token_set = CapcoTokenSet;
    let mut out: Vec<(String, FeatureEntry)> = Vec::new();

    let mut search_start = 0;
    while let Some(rel_pos) = text[search_start..].find("REL TO ") {
        let header_end = search_start + rel_pos + "REL TO ".len();
        // Block ends at the EARLIEST of: `//` (next category), `\n`
        // (banner/CAB candidates from `Scanner::scan_banners` arrive
        // as full lines, so a REL TO line can have trailing prose
        // beyond the marking), or `)` (portion-form close). CAPCO
        // §H.8 / §A authority: `//` is the category separator; `,`
        // separates entries within the REL TO category itself.
        // Mirrors the corpus analyzer's terminator priority in
        // `tools/corpus-analysis/analyze.py` (`_extract_rel_to_trigraphs`).
        let tail = &text[header_end..];
        let block_len = ["//", "\n", ")"]
            .iter()
            .filter_map(|sep| tail.find(sep))
            .min()
            .unwrap_or(tail.len());
        let block_end = header_end + block_len;
        let block = &text[header_end..block_end];

        // Walk the comma-separated entries with their byte offsets.
        let mut cursor = 0usize;
        for entry in block.split(',') {
            let entry_start = cursor;
            let entry_end = cursor + entry.len();
            cursor = entry_end + 1; // skip the comma

            let trimmed = entry.trim();
            // 3-char (trigraph) or 4-char (tetragraph) ASCII-uppercase
            // entries only — see fn doc for scope rationale.
            let tlen = trimmed.len();
            if (tlen != 3 && tlen != 4) || !trimmed.bytes().all(|b| b.is_ascii_uppercase()) {
                continue;
            }
            // Skip already-valid trigraphs/tetragraphs (the matcher's
            // binary search would also short-circuit on a vocab hit, but
            // keeping the explicit check means a token like `FVEY`
            // appearing legitimately never gets multi-cast).
            if token_set.is_trigraph(trimmed) {
                continue;
            }

            // Bypass the standard `MIN_USEFUL_CONFIDENCE` floor:
            // for a 3-char input, distance-2 corrections sit at
            // confidence 0.40, below the default 0.45 cutoff that
            // protects the standalone fuzzy path. Issue #233's score-
            // time tiebreak (corpus-weighted trigraph priors +
            // `UNAMBIGUOUS_LOG_MARGIN`) supplies the safety the
            // confidence-floor was substituting for; without lowering
            // it here, a typo like `ASU → AUS` (plain Levenshtein
            // distance 2) never reaches the scorer.
            let mut candidates = trigraph_matcher.correct_all_with_floor(trimmed, 0.0);
            if candidates.is_empty() {
                continue;
            }

            // Drop candidates that would duplicate a trigraph already
            // present elsewhere in this REL TO block. CAPCO-2016 §H.8
            // does not state "no duplicates" as an explicit textual
            // prohibition — the REL TO grammar (§A.6 / §H.8 p131-150)
            // describes a list of country codes ordered USA-first then
            // ascending alphabetic, which structurally implies a set of
            // distinct codes but does not forbid repetition in so many
            // words. The reason we drop duplicates here is mechanical,
            // not citational: the bag-of-tokens scorer happens to
            // *reward* duplicates (each instance adds its log-prior
            // again), so without this filter an ambiguous typo
            // adjacent to a popular trigraph could collapse to
            // "REL TO USA, USA, GBR" because USA's log-prior
            // contribution is additive. Emitting a duplicate-creating
            // candidate would therefore be structurally redundant and
            // cause the scorer to erroneously favor it. The block's
            // other entries are computed by re-walking
            // `block.split(',')` and taking the trigraph form of any
            // 3-char ASCII-uppercase entry that's in the CVE
            // recognition set.
            let other_trigraphs: Vec<&str> = block
                .split(',')
                .map(str::trim)
                .filter(|e| {
                    let elen = e.len();
                    (elen == 3 || elen == 4)
                        && e.bytes().all(|b| b.is_ascii_uppercase())
                        && *e != trimmed
                        && token_set.is_trigraph(e)
                })
                .collect();
            candidates.retain(|c| !other_trigraphs.contains(&c.token));
            if candidates.is_empty() {
                continue;
            }

            // Rank candidates by (distance, then country-code
            // log-prior). The plain Levenshtein hits for a 3-char
            // input often produce 20+ distance-2 candidates (every
            // other 3-char trigraph that shares one letter). Without
            // a prior-rank pre-filter, the K=16 attempt cap upstream
            // gets exhausted by low-prior alternates and the
            // high-prior ones get dropped. Sorting by (distance asc,
            // log-prior desc) keeps the most plausible candidates
            // first; we cap at TRIGRAPH_FUZZY_TOP_K per ambiguous
            // entry to bound the candidate-set growth.
            //
            // The cap value (4) is sized so a single ambiguous entry
            // doesn't crowd out the other decoder paths
            // (`fuzzy_corrected`, reorder, delimiter-insert, etc.):
            // 4 alternates ≤ K_MAX_CANDIDATES (8) leaves room for
            // the standard candidates the dispatcher also needs.
            const TRIGRAPH_FUZZY_TOP_K: usize = 4;
            candidates.sort_by(|a, b| {
                a.distance.cmp(&b.distance).then_with(|| {
                    let pa = marque_capco::priors::country_code_log_prior(a.token)
                        .unwrap_or(f32::NEG_INFINITY);
                    let pb = marque_capco::priors::country_code_log_prior(b.token)
                        .unwrap_or(f32::NEG_INFINITY);
                    pb.total_cmp(&pa)
                })
            });
            candidates.truncate(TRIGRAPH_FUZZY_TOP_K);

            for cand in &candidates {
                // Reconstruct the full `text` with the entry replaced.
                // The 3-char trimmed sub-slice within the entry
                // preserves any surrounding whitespace.
                let lead_ws_len = entry.len() - entry.trim_start().len();
                let trail_ws_len = entry.len() - entry.trim_end().len();
                let mut rewritten_entry = String::with_capacity(entry.len());
                rewritten_entry.push_str(&entry[..lead_ws_len]);
                rewritten_entry.push_str(cand.token);
                rewritten_entry.push_str(&entry[entry.len() - trail_ws_len..]);

                let mut alt = String::with_capacity(text.len());
                alt.push_str(&text[..header_end + entry_start]);
                alt.push_str(&rewritten_entry);
                alt.push_str(&text[header_end + entry_end..]);

                // `FeatureId` is a closed audit-schema enum (see
                // `crates/rules/src/confidence.rs` and `MARQUE_AUDIT_SCHEMA`);
                // pair each (id, delta) directly off `cand.distance`
                // so the match is total over the only two outcomes
                // `cand.distance` can take here. The standalone fuzzy
                // matcher caps results at `MAX_EDIT_DISTANCE = 2`.
                let entry = if cand.distance <= 1 {
                    FeatureEntry {
                        id: FeatureId::EditDistance1,
                        delta: -0.5,
                    }
                } else {
                    FeatureEntry {
                        id: FeatureId::EditDistance2,
                        delta: -1.2,
                    }
                };
                out.push((alt, entry));
            }
        }

        search_start = block_end;
    }

    out
}

// ---------------------------------------------------------------------------
// REL TO USA-injection for short first entries (issue #234 PR-B)
// ---------------------------------------------------------------------------

/// Emit one canonical-byte alternate per REL TO block whose first
/// entry is a 1- or 2-character ASCII-uppercase token AND USA is not
/// otherwise present in the block. The alternate replaces that short
/// first entry with `USA`.
///
/// **Why complement to PR-A.** Issue #233's
/// [`try_rel_to_fuzzy_trigraph_candidates`] handles 3-char REL TO
/// entries: an unknown trigraph-shaped token gets fuzzy-matched
/// against the [`marque_ism::TRIGRAPHS`] vocabulary, and corpus-
/// weighted log-priors break ties at score time. That path
/// deliberately skips entries below `MIN_FUZZY_LEN = 3` (see the
/// `if trimmed.len() != 3` guard in `try_rel_to_fuzzy_trigraph_candidates`)
/// because `phf`-style fuzzy matching is unreliable on inputs that
/// short — a 2-char input is edit-distance-1 from many distinct
/// trigraphs and the mapper has no signal to break the tie.
///
/// For REL TO specifically, the §H.8 p150–151 grammar gives us a
/// stronger signal that fuzzy-matching cannot exploit: **USA must
/// always appear first**. So when we see a REL TO block whose first
/// entry is a 1- or 2-character ASCII-uppercase token, the most
/// likely intent — far above any other 3-char trigraph — is that
/// the user typed USA and dropped one or two characters. The fixture
/// at `tests/fixtures/mangled/typo/ad2bcfe3ac0b0765.json`
/// (`REL TO SA, AUS, GBR` → `REL TO USA, AUS, GBR`) is the canonical
/// case: `SA` is shape-incompatible with PR-A's 3-char floor, so
/// without this complementary path the decoder produces zero
/// candidates and the fixture fails recovery.
///
/// **CAPCO authority**: the USA-first invariant is CAPCO-2016 §H.8
/// p151: "After 'USA', list the required one or more trigraph country
/// codes in alphabetical order." E020 enforces that invariant at the
/// rule layer (via the `marque-capco`-private `canonicalize_trigraph_list`
/// helper). This decoder path operates one stage earlier — pre-strict-
/// parse, on raw text — so it does NOT call the rule-layer helper; it
/// emits a candidate text and lets the downstream pipeline (strict
/// parse + E020) verify and re-canonicalize as needed.
///
/// **Scope and guards** (mirrors PR-A's design):
///
/// - Fires only when the first entry's trimmed length is 1 or 2 ASCII
///   uppercase bytes (3-char entries belong to PR-A's domain).
/// - Skips when USA is already present elsewhere in the block — that
///   case isn't a USA-typo, it's an unrelated short prefix the user
///   may have meant differently. The block stays as-is.
/// - Skips when the block has fewer than two entries — a single
///   short entry plus nothing else doesn't fit the §H.8 p151
///   "USA + trigraph list" shape.
/// - Emits the substitution transform only — full canonicalization
///   (USA first, remaining trigraphs alphabetical, no duplicates) is
///   downstream. If the original list's tail (other than the
///   corrupted first entry) wasn't already alphabetical, E020 will
///   fire on the post-decode text and produce its own fix; if the
///   injection produced a duplicate (USA was already present in the
///   block under a different shape), the `already_has_usa` guard
///   above suppresses emit. Keeping the decoder text-level (no
///   `marque-capco` imports) avoids re-entering the rule layer
///   mid-recognition while preserving the single-source-of-truth
///   property — the canonical ordering rule lives in `marque-capco`,
///   and the decoder defers to whatever it produces post-parse.
/// - Audit signal: each candidate carries
///   [`FeatureId::BaseRateCommonMarking`] as provenance only, with
///   zero delta. This records that USA is the dominant trigraph in
///   the corpus prior without changing score or double-counting that
///   prior in the posterior. Reusing `BaseRateCommonMarking` (vs
///   introducing a new variant) keeps the audit schema closed —
///   `MARQUE_AUDIT_SCHEMA` stays at `marque-1.0`.
fn try_rel_to_usa_injection_candidates(text: &str) -> Vec<(String, FeatureEntry)> {
    let mut out: Vec<(String, FeatureEntry)> = Vec::new();

    let mut search_start = 0;
    while let Some(rel_pos) = text[search_start..].find("REL TO ") {
        let header_end = search_start + rel_pos + "REL TO ".len();
        // Block ends at the EARLIEST of: `//` (next category), `\n`
        // (banner/CAB candidates from `Scanner::scan_banners` arrive
        // as full lines), or `)` (portion-form close). CAPCO §H.8 /
        // §A authority: `//` is the category separator; `,` separates
        // entries within the REL TO category itself. Mirrors the
        // terminator priority in `try_rel_to_fuzzy_trigraph_candidates`
        // and the corpus analyzer's `_extract_rel_to_trigraphs`.
        let tail = &text[header_end..];
        let block_len = ["//", "\n", ")"]
            .iter()
            .filter_map(|sep| tail.find(sep))
            .min()
            .unwrap_or(tail.len());
        let block_end = header_end + block_len;
        let block = &text[header_end..block_end];

        // Walk entries with their byte offsets within the block.
        // Pre-size from comma count + 1 — typical REL TO blocks have
        // 2–6 entries, so this avoids reallocations on the common case.
        let entries: Vec<(usize, &str)> = {
            let mut v = Vec::with_capacity(block.bytes().filter(|&b| b == b',').count() + 1);
            let mut cursor = 0usize;
            for entry in block.split(',') {
                v.push((cursor, entry));
                cursor += entry.len() + 1; // +1 for the comma separator
            }
            v
        };
        if entries.len() < 2 {
            // Single-entry block: doesn't match the §H.8 p151
            // "USA + trigraph list" shape we're recovering.
            search_start = block_end;
            continue;
        }

        // First entry is the candidate USA-typo position. The
        // structural guard is shape-only — len ∈ {1, 2}, all ASCII
        // uppercase. 3-char entries fall through to PR-A. Length 0
        // (e.g., a leading comma) is already filtered.
        let (first_entry_offset, first_entry) = entries[0];
        let trimmed = first_entry.trim();
        let is_short =
            (1..=2).contains(&trimmed.len()) && trimmed.bytes().all(|b| b.is_ascii_uppercase());
        if !is_short {
            search_start = block_end;
            continue;
        }

        // Skip if USA is already present elsewhere in the block —
        // a USA-injection candidate would create a duplicate, which
        // E052 (issue #234 PR-B) would then need to dedup. Short-
        // circuit here rather than emit-and-redup.
        let already_has_usa = entries.iter().skip(1).any(|(_, e)| e.trim() == "USA");
        if already_has_usa {
            search_start = block_end;
            continue;
        }

        // Build the substituted text. Preserve the entry's
        // surrounding whitespace (lead/trail) so the splice
        // round-trips through the strict parser the same way the
        // original would have.
        let lead_ws_len = first_entry.len() - first_entry.trim_start().len();
        let trail_ws_len = first_entry.len() - first_entry.trim_end().len();
        let mut rewritten_entry = String::with_capacity(first_entry.len() + 3);
        rewritten_entry.push_str(&first_entry[..lead_ws_len]);
        rewritten_entry.push_str("USA");
        rewritten_entry.push_str(&first_entry[first_entry.len() - trail_ws_len..]);

        let mut alt = String::with_capacity(text.len() + 3);
        alt.push_str(&text[..header_end + first_entry_offset]);
        alt.push_str(&rewritten_entry);
        alt.push_str(&text[header_end + first_entry_offset + first_entry.len()..]);

        // Audit-only provenance. The load-bearing scoring lives in
        // `score_candidate`, which sums `country_code_log_prior(USA)`
        // — already an extreme positive in the baked corpus prior —
        // over the parsed `rel_to` slice and is what carries the
        // candidate to victory. The `BaseRateCommonMarking` entry
        // here records the prior's contribution in the audit log
        // without double-counting it in the decoder's score, mirror-
        // ing PR-A's trigraph-prior treatment (delta = 0.0).
        let entry = FeatureEntry {
            id: FeatureId::BaseRateCommonMarking,
            delta: 0.0,
        };
        out.push((alt, entry));

        search_start = block_end;
    }

    out
}

// ---------------------------------------------------------------------------
// SCI delimiter recovery (issue #198 — #133 PR 10)
// ---------------------------------------------------------------------------

/// SCI delimiter recovery preprocessing — issue #198, #133 PR 10.
///
/// Repairs three classes of SCI delimiter typos against the closed
/// CVE vocabulary in `CVEnumISMSCIControls.xml`. Vocabulary checks
/// dispatch through the build-time-generated [`SciControlBare::parse`]
/// (bare control systems) and [`SciControl::parse`] (the full CVE set
/// including all registered control-compartment compounds), so the
/// repair surface tracks ODNI schema updates automatically — no
/// hand-maintained vocabulary slice to drift out of sync per
/// Constitution IV (Layer 1 generated predicates):
///
/// - **Pattern A (concatenated compound)**: a token equal to a compound
///   with the hyphen removed → canonical hyphenated form. `HCSP →
///   HCS-P`, `SIG → SI-G`, `TKKAND → TK-KAND`, etc.
/// - **Pattern B (concatenated bare control systems)**: a token of
///   length 4–6 that splits cleanly into two bare control systems →
///   slash-joined form (`SITK → SI/TK`, `HCSSI → HCS/SI`) per §A.6
///   p16 and the `TOP SECRET//ANB/SI/TK/XNB//NOFORN` example on p194.
///   Ambiguous splits bail out — see [`repair_sci_token`] for the
///   guard.
/// - **Pattern C (wrong delimiter)**: a token of the form
///   `<bare_cs>-<bare_cs>` that is NOT itself a registered compound →
///   slash-joined form. `SI-TK → SI/TK` (because `SI-TK` is not
///   registered), but `SI-G` is left alone (it IS registered — `-` is
///   the correct control-compartment separator per §A.6 p16).
///
/// **Out of scope** — sub-compartment fuzzy recovery (`ABCE → ABCD`),
/// unregistered-compartment recovery, and any rewrite that would
/// require fuzz-correcting against agency-assigned codewords. Those
/// require operator-supplied vocab (issue #180) — the engine cannot
/// invent identifiers it doesn't know are valid (Constitution VIII).
///
/// **Architectural shape** mirrors `try_rel_to_structural_repair`
/// (PR 9, #190): runs as preprocessing on the input string before
/// per-token fuzzy correction, returns `Some(repaired)` only when at
/// least one repair fired. The caller pushes a `BaseRateCommonMarking`
/// feature onto `delim_features` so every candidate derived from the
/// repaired text inherits the audit trace.
///
/// **Allocation behavior**: short-circuits without allocation when the
/// pre-check finds no SCI control system root in the text. The
/// per-token walk borrows the input until a fix actually fires.
fn try_sci_delimiter_repair(text: &str) -> Option<String> {
    if !contains_any_sci_root(text) {
        return None;
    }

    // ASCII-only guard. The SCI control-system vocabulary
    // (`SciControlBare::ALL`) and the registered compound names
    // (`SciControl::ALL`) are pure ASCII, as are the delimiters this
    // function recognizes (`-`, `/`, `(`, `)`, space, tab, newline,
    // CR, comma). So any non-ASCII input cannot match any pattern;
    // bailing early avoids the byte-vs-char-boundary hazard that
    // would otherwise arise from indexing `text` with byte offsets.
    if !text.is_ascii() {
        return None;
    }

    let bytes = text.as_bytes();
    let mut result: Option<String> = None;
    let mut last_copied = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        let at_boundary = i == 0
            || matches!(
                bytes[i - 1],
                b'/' | b'(' | b')' | b' ' | b'\t' | b'\n' | b'\r' | b','
            );
        if !at_boundary {
            i += 1;
            continue;
        }

        let token_start = i;
        let token_end = bytes[token_start..]
            .iter()
            .position(|&b| matches!(b, b'/' | b'(' | b')' | b' ' | b'\t' | b'\n' | b'\r' | b','))
            .map(|n| token_start + n)
            .unwrap_or(bytes.len());

        if token_start < token_end {
            let token = &text[token_start..token_end];
            if let Some(repaired) = repair_sci_token(token) {
                let r = result.get_or_insert_with(|| String::with_capacity(text.len()));
                r.push_str(&text[last_copied..token_start]);
                r.push_str(&repaired);
                last_copied = token_end;
            }
        }

        // Advance past the token; the next iteration will re-check the
        // boundary before the byte after the delimiter (or terminate at
        // end-of-input).
        i = token_end + 1;
    }

    result.map(|mut r| {
        r.push_str(&text[last_copied..]);
        r
    })
}

/// Cheap pre-check for [`try_sci_delimiter_repair`]: returns true when
/// the input contains at least one bare SCI control system identifier
/// as a substring. False positives just mean we walk the bytes and
/// return `None` — no correctness impact, only a performance
/// optimization for the overwhelmingly common case where the input has
/// no SCI category at all.
fn contains_any_sci_root(text: &str) -> bool {
    text.contains("HCS")
        || text.contains("KLM")
        || text.contains("MVL")
        || text.contains("RSV")
        || text.contains("BUR")
        || text.contains("SI")
        || text.contains("TK")
}

/// Per-token classifier for SCI delimiter repair. Returns the repaired
/// token if one of patterns A/B/C matches; otherwise `None`.
///
/// All vocabulary checks dispatch through the build-time-generated
/// [`SciControlBare::parse`] and [`SciControl::parse`] (from
/// `marque-ism`'s generated `values.rs`), so the repair surface tracks
/// `CVEnumISMSCIControls.xml` automatically. New CVE compounds added
/// in a future ODNI schema bump (e.g., a hypothetical `SI-XYZ`) are
/// auto-discovered by Pattern A without any code change here.
///
/// Pattern dispatch order:
/// 1. Pattern A (split into bare-CS prefix + suffix; if
///    `{prefix}-{suffix}` is a registered CVE value, return it)
/// 2. Pattern C (token contains `-`, neither side is a registered
///    compound's compartment, both halves are bare CS)
/// 3. Pattern B (no `-`, splits into two bare CS, unambiguous)
fn repair_sci_token(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }

    // ASCII-only guard. The CVE vocabulary is pure ASCII, so a non-
    // ASCII token cannot match any pattern; bailing early ensures
    // the byte-offset slicing below (`token[..split]`,
    // `token[split..]`, `token[..dash_pos]`, `token[dash_pos + 1..]`)
    // never lands in the middle of a multi-byte UTF-8 sequence. This
    // is a defense-in-depth check — the only production caller
    // (`try_sci_delimiter_repair`) already gates on ASCII — but
    // keeping it here makes the function's invariant local and
    // self-evident for any future caller (e.g., a unit test).
    if !token.is_ascii() {
        return None;
    }

    let len = token.len();

    // Pattern A — concatenated registered compound. Walk every split
    // where the prefix is a bare control system; if `{prefix}-{suffix}`
    // is in the CVE vocabulary, return the canonical hyphenated form.
    // Bare CS lengths are 2 or 3; suffix length range comes from CVE
    // (max compartment-form suffix is 4 chars, e.g. TK-BLFH).
    if !token.contains('-') && (3..=8).contains(&len) {
        for &split in &[2usize, 3] {
            if split >= len {
                continue;
            }
            let prefix = &token[..split];
            let suffix = &token[split..];
            if SciControlBare::parse(prefix).is_some() {
                let canonical = format!("{prefix}-{suffix}");
                if SciControl::parse(&canonical).is_some() {
                    return Some(canonical);
                }
            }
        }
    }

    // Pattern C — wrong delimiter (`-` between two bare CS). Skip if
    // the whole token is itself a registered CVE compound.
    if let Some(dash_pos) = token.find('-') {
        if SciControl::parse(token).is_some() {
            return None;
        }
        let prefix = &token[..dash_pos];
        let suffix = &token[dash_pos + 1..];
        if SciControlBare::parse(prefix).is_some() && SciControlBare::parse(suffix).is_some() {
            return Some(format!("{prefix}/{suffix}"));
        }
        return None;
    }

    // Pattern B — concatenated bare control systems (no delimiter).
    // Bare CS lengths are 2 or 3; the concatenation is therefore in
    // [4..=6]. Try splits at positions 2 and 3 (the only split points
    // that can yield two valid bare-CS halves) and require an
    // unambiguous match.
    if !(4..=6).contains(&len) {
        return None;
    }
    let mut found: Option<(&str, &str)> = None;
    for &split in &[2usize, 3] {
        if split >= len {
            continue;
        }
        let suffix_len = len - split;
        if !(2..=3).contains(&suffix_len) {
            continue;
        }
        let prefix = &token[..split];
        let suffix = &token[split..];
        if SciControlBare::parse(prefix).is_some() && SciControlBare::parse(suffix).is_some() {
            if found.is_some() {
                return None;
            }
            found = Some((prefix, suffix));
        }
    }
    found.map(|(p, s)| format!("{p}/{s}"))
}

// ---------------------------------------------------------------------------
// NATO longhand → canonical portion fold
// ---------------------------------------------------------------------------

/// Mapping from NATO longhand classification level tokens to canonical
/// [`NatoClassification`] variants. Keyed on the token string (abbreviation
/// or full-word form); the canonical portion string (`NS`, `CTS`, etc.) is
/// derived via [`NatoClassification::portion_str`] so that a future
/// enum-variant addition (ATOMAL sub-levels, PR 9 T134 BOHEMIA/BALK) is
/// enough to extend coverage without touching this fold logic.
///
/// Rows ordered: abbreviations first (U/R/C/S/TS), then full words. The
/// lookup is a linear scan over 10 rows — the total set is small and
/// bounded by the five standard NATO classification levels.
///
/// **Out of scope.** Parametric NATO-NAC-Activity rows from §G.1 Table 4
/// lines 776-779 (`NATO [NAC Activity] SECRET → N[NAC Activity]S` and
/// equivalents for C/R/U) are not covered here; they require distinct parser
/// support for the open-ended activity identifier and are not tracked in this
/// PR or PR 9 T134.
///
/// Citation: CAPCO-2016 §G.1 Table 4 pp 36-38 (canonical Register).
const NATO_LONGHAND_FOLD: &[(&str, NatoClassification)] = &[
    // Abbreviation forms (single-letter / two-letter)
    ("U", NatoClassification::NatoUnclassified),
    ("R", NatoClassification::NatoRestricted),
    ("C", NatoClassification::NatoConfidential),
    ("S", NatoClassification::NatoSecret),
    ("TS", NatoClassification::CosmicTopSecret),
    // Full-word forms ("TOP SECRET" is a two-word compound handled separately)
    ("UNCLASSIFIED", NatoClassification::NatoUnclassified),
    ("RESTRICTED", NatoClassification::NatoRestricted),
    ("CONFIDENTIAL", NatoClassification::NatoConfidential),
    ("SECRET", NatoClassification::NatoSecret),
    // Note: "TOP SECRET" requires two-token detection; see `fold_nato_segment`.
    // "TOP" alone is not a valid abbreviation and is excluded from this table.
];

/// Fold NATO longhand classification levels into canonical forms.
///
/// Recovers inputs the strict parser doesn't recognize. Handles both
/// portion and banner kinds:
///
/// For `MarkingType::Portion`, NATO abbreviation → portion abbreviation
/// (equivalence transform, both are valid surface forms):
///   NATO U → NU, NATO R → NR, NATO C → NC, NATO S → NS,
///   NATO TS → CTS, NATO UNCLASSIFIED → NU, NATO SECRET → NS, … (long-word
///   forms too). NATO TOP SECRET → CTS (two-word compound, handled explicitly).
///   Canonical inputs (e.g. `(//NS//NF)`) return `None` (idempotent).
///
/// For `MarkingType::Banner`, NATO abbreviation → banner long form:
///   NATO U → NATO UNCLASSIFIED, NATO R → NATO RESTRICTED,
///   NATO C → NATO CONFIDENTIAL, NATO S → NATO SECRET,
///   NATO TS → COSMIC TOP SECRET.
///   Inputs already in banner canonical form (e.g. `NATO SECRET//NOFORN`)
///   return `None` (idempotent). This closes the unimplemented half of #260:
///   the strict parser recognizes full banner forms (`NATO SECRET`,
///   `COSMIC TOP SECRET`) but not abbreviations (`NATO S`, `NATO TS`), so
///   a banner input `NATO S//NOFORN` fails strict parse and the fold
///   recovers it.
///
/// For `MarkingType::Cab` and `MarkingType::PageBreak`, returns `None`
/// (CAB authority lines and page-break sentinels don't carry NATO classifications).
///
/// **Segment restriction (FIX-2, CAPCO §H.7 FGI transmutation).** The fold
/// fires ONLY on the first non-empty `//`-separated segment (the
/// classification slot). NATO content in a non-first-slot position
/// (e.g., `(S//NATO C)`) indicates commingled US+NATO info, which per
/// CAPCO-2016 §H.7 should transmute to FGI (`(S//FGI NATO)`) — not
/// produce a NATO-axis canonical. PR 8 does not implement the transmutation
/// (Stage 4 / PR 9+ territory); restricting the fold to the first segment
/// ensures we don't manufacture wrong intermediates while the proper fix waits.
/// Cross-segment NATO inputs return decode-miss.
///
/// **Idempotence**: returns `None` when no segment was changed (including
/// when the input is already canonical).
///
/// **Pure function**: no captures, no global state. `Send + Sync` follows
/// automatically. Pre-uppercased input assumed (caller passes the
/// post-`normalize_delimiters_and_case` string).
///
/// Citation: CAPCO-2016 §G.1 Table 4 pp 36-38 (canonical Register);
/// §A.6 p15 (`//` prefix for non-US classifications); §H.7 (FGI transmutation).
fn try_nato_fold(text: &str, kind: MarkingType) -> Option<String> {
    // CAB, PageBreak, and PageFinalization inputs don't carry NATO
    // classifications — they are non-content / engine-synthesized
    // boundary candidates. PageFinalization (issue #461) is dispatched
    // only to `Phase::PageFinalization` rules and never enters the
    // strict/decoder recognize path on actual bytes; the early-return
    // mirrors the existing Cab/PageBreak shape.
    if matches!(
        kind,
        MarkingType::Cab | MarkingType::PageBreak | MarkingType::PageFinalization
    ) {
        return None;
    }
    // All NATO classification tokens are pure ASCII; non-ASCII input
    // cannot contain them.
    if !text.is_ascii() {
        return None;
    }

    // Strip surrounding parens — only portion form has them. Banner inputs
    // like `NATO S//NOFORN` never carry parens so this branch is
    // naturally a no-op for Banner kind.
    let (has_parens, inner) =
        if kind == MarkingType::Portion && text.starts_with('(') && text.ends_with(')') {
            (true, &text[1..text.len() - 1])
        } else {
            (false, text)
        };

    // Split into `//`-separated segments. A leading `//` (canonical
    // non-US form) produces an empty first element; we track this to
    // avoid adding a spurious second `//` prefix.
    let segments: Vec<&str> = inner.split("//").collect();
    let had_leading_empty = segments.first().map(|s| s.is_empty()).unwrap_or(false);

    // Determine the index of the first non-empty segment (the
    // classification slot). The fold ONLY fires on that segment;
    // all other segments are passed through verbatim.
    //
    // Rationale: NATO classifications always occupy the first
    // `//`-separated slot per CAPCO-2016 §A.6. `NATO X` in a
    // non-first slot (e.g., `(S//NATO C)`) indicates commingled
    // US+NATO info. Correct canonical form per §H.7 is FGI transmutation
    // (`(S//FGI NATO)`), not a NATO-axis canonical. PR 9+ handles that
    // transmutation; PR 8 produces a decode-miss to avoid wrong intermediates.
    let first_nonempty_idx = segments.iter().position(|s| !s.is_empty());
    let Some(class_slot_idx) = first_nonempty_idx else {
        return None; // All empty — degenerate input, nothing to fold.
    };

    let mut any_changed = false;
    let mut first_segment_folded = false;
    let mut result_segments: Vec<String> = Vec::with_capacity(segments.len());

    for (i, seg) in segments.iter().enumerate() {
        if i == class_slot_idx {
            // Classification slot — attempt the fold.
            match fold_nato_segment(seg, kind) {
                Some(folded) => {
                    any_changed = true;
                    if i == 0 {
                        first_segment_folded = true;
                    }
                    result_segments.push(folded);
                }
                None => {
                    result_segments.push(seg.to_string());
                }
            }
        } else {
            // Non-classification slot — pass through unchanged.
            result_segments.push(seg.to_string());
        }
    }

    if !any_changed {
        return None;
    }

    let rejoined = result_segments.join("//");

    // For portion inputs that arrived without a leading `//` (e.g., `(NATO S)` or
    // `(NATO S//NF)`), the fold converts the first segment to a canonical
    // NATO abbreviation. Non-US classifications require the `//` prefix per
    // CAPCO-2016 §A.6 p15 so the strict parser enters the non-US classification
    // code path. We add it only when the first segment was the one folded
    // AND the original had no leading empty segment (= no prior `//`).
    //
    // The same `//` logic applies to banner inputs: banner `NATO S//NF`
    // (no leading `//`) folds to `NATO SECRET//NF` → needs `//NATO SECRET//NF`
    // per §A.6 p15. The `first_segment_folded` flag is set whenever the
    // classification-slot segment folds, regardless of kind.
    let inner_out = if first_segment_folded && !had_leading_empty {
        format!("//{rejoined}")
    } else {
        rejoined
    };

    if has_parens {
        Some(format!("({inner_out})"))
    } else {
        Some(inner_out)
    }
}

/// Attempt to fold a single `//`-separated segment that starts with the
/// NATO keyword.
///
/// Returns `Some(canonical)` when the segment begins `NATO <level>` (with
/// `<level>` either an abbreviation from [`NATO_LONGHAND_FOLD`] or the
/// two-word compound `TOP SECRET`) AND the result differs from the input
/// (idempotence guard). Returns `None` for all other inputs, including
/// segments whose first token is not `NATO` (guard against false-positives
/// inside `REL TO USA, NATO` or FGI country lists).
///
/// The `kind` parameter controls the emission form:
/// - `MarkingType::Portion` — emits the portion abbreviation
///   (`NS`, `NC`, `CTS`, …) via [`NatoClassification::portion_str`].
/// - `MarkingType::Banner` — emits the banner long form
///   (`NATO SECRET`, `COSMIC TOP SECRET`, …) via
///   [`NatoClassification::banner_str`]. Idempotent: if the input segment
///   is already in banner long form (e.g. `NATO SECRET`), the emitted
///   `banner_str()` equals the input and `None` is returned.
///
/// Returns `None` when the segment is `NATO <level> <rest>` with non-empty
/// `<rest>` — compound forms like `NATO SECRET ATOMAL` parse through the
/// strict parser's `parse_nato_classification`, which now (PR 9c.1 T134)
/// canonicalizes legacy compound text into bare class + AEA/SCI companion
/// per CAPCO-2016 §H.7 p122 + §G.2 p40 + §H.7 p127. The fold must not
/// truncate the suffix; its job is the 5-base-level path only.
///
/// **Caller invariant.** The caller ([`try_nato_fold`]) restricts invocation
/// to the first non-empty `//`-separated segment (the classification slot) so
/// that `NATO X` in a non-classification-slot position (e.g., `(S//NATO C)`)
/// never reaches this function. This is defense-in-depth: the segment-leading
/// guard (`strip_prefix("NATO ")`) would also prevent non-NATO segments from
/// firing, but the first-segment restriction in the caller is the primary
/// mechanism ensuring semantic correctness per CAPCO-2016 §H.7. A
/// `NATO X` token in the SCI/dissem slot indicates commingled US+NATO info
/// that should transmute to FGI — not produce a NATO-axis canonical.
fn fold_nato_segment(seg: &str, kind: MarkingType) -> Option<String> {
    let trimmed = seg.trim();
    // Segment-leading guard: the fold ONLY fires when the first
    // non-delimiter token is the literal keyword `NATO`.
    let after_nato = trimmed.strip_prefix("NATO ")?;
    let after_nato = after_nato.trim_start();

    // Determine the `NatoClassification` variant from the level token(s).
    let nato_level: NatoClassification;

    // Special case: "TOP SECRET" is a two-word compound that cannot be
    // matched as a single entry in `NATO_LONGHAND_FOLD`. Detect it
    // explicitly before the single-token path.
    if let Some(after_ts) = after_nato.strip_prefix("TOP SECRET") {
        let rest = after_ts.trim_start();
        if !rest.is_empty() {
            // Compound NATO SAP forms (ATOMAL, BOHEMIA, BALK) are out of scope
            // for PR 8. The strict parser already accepts
            // `NATO TOP SECRET ATOMAL` / `NATO TOP SECRET-BOHEMIA` /
            // `NATO TOP SECRET-BALK` (parser.rs:1043-1052); folding the first
            // half would mangle the suffix and regress recovery.
            // PR 9 T134 will land an explicit fold for these compounds.
            return None;
        }
        nato_level = NatoClassification::CosmicTopSecret;
    } else {
        // Single-token level: split at the next whitespace to isolate the
        // level token, then look it up in `NATO_LONGHAND_FOLD`.
        let (level_token, rest) = match after_nato.find(char::is_whitespace) {
            Some(pos) => (&after_nato[..pos], after_nato[pos..].trim_start()),
            None => (after_nato, ""),
        };

        let found = NATO_LONGHAND_FOLD
            .iter()
            .find(|&&(key, _)| key == level_token)
            .map(|&(_, level)| level)?;

        if !rest.is_empty() {
            // Same rationale as the TOP SECRET branch: compound SAP forms
            // (NATO SECRET ATOMAL, NATO CONFIDENTIAL ATOMAL, etc.) are out of
            // scope. The strict parser handles them; the fold must not truncate
            // the suffix. PR 9 T134 will land the explicit ATOMAL/BOHEMIA/BALK
            // fold.
            return None;
        }
        nato_level = found;
    }

    // Emit the canonical form for the requested kind, then check idempotence.
    // For portion: `NATO SECRET` → `NS` (changed → emit). `NATO NS` would
    // not match (strip_prefix "NATO " yields "NS" which is not in table when
    // looked up as level_token; actually "NS" IS not in the table — only
    // abbreviations U/R/C/S/TS and long words). Idempotence fires on banner:
    // `NATO SECRET` → `banner_str() = "NATO SECRET"` — segment is the same.
    // But `seg` here is just the classification content without "NATO ",
    // so we need the full composed string for comparison.
    let canonical = match kind {
        MarkingType::Portion => nato_level.portion_str().to_owned(),
        // Banner form is the full level string (e.g. "NATO SECRET").
        // `banner_str()` already returns "NATO SECRET" / "COSMIC TOP SECRET"
        // etc. — it INCLUDES the "NATO " prefix for all non-CTS levels.
        _ => nato_level.banner_str().to_owned(),
    };

    // Idempotence: if the emitted canonical equals the input segment
    // (trimmed), no actual change occurred — return None so `any_changed`
    // stays false and `try_nato_fold` returns None overall.
    if canonical == trimmed {
        return None;
    }

    Some(canonical)
}

// ---------------------------------------------------------------------------
// Token reordering
// ---------------------------------------------------------------------------

/// Try to produce a canonical-order rewrite of `text`.
///
/// The CAPCO category order is: classification → SCI → SAR → dissem.
/// If the observed segments are out of order — e.g., `NOFORN//SECRET`
/// with dissem first — this helper swaps them into the canonical
/// order. Returns `None` when the input is already in canonical order
/// or when reordering doesn't apply (CAB lines, single-segment input).
fn try_canonical_reorder(text: &str) -> Option<String> {
    // Only banner/portion-shaped input (contains `//`) is reorderable
    // with this heuristic. CABs use keyed authority lines, not
    // category ordering.
    if !text.contains("//") {
        return None;
    }

    // Portion form: `(C//NF)` — strip the surrounding parens for
    // reasoning, re-wrap at emit.
    let (prefix, body, suffix) = if text.starts_with('(') && text.ends_with(')') {
        ("(", &text[1..text.len() - 1], ")")
    } else {
        ("", text, "")
    };

    let segments: Vec<&str> = body.split("//").collect();
    if segments.len() < 2 {
        return None;
    }

    // Classify each segment by its dominant category. We only
    // reorder when exactly one segment is classification-dominant
    // and at least one other is dissem-dominant — otherwise the
    // input is too ambiguous for a clean swap.
    let mut class_segments: Vec<&str> = Vec::new();
    let mut dissem_segments: Vec<&str> = Vec::new();
    let mut other_segments: Vec<&str> = Vec::new();
    for seg in &segments {
        let seg = seg.trim();
        if seg.is_empty() {
            continue;
        }
        match classify_segment(seg) {
            SegmentClass::Classification => class_segments.push(seg),
            SegmentClass::Dissem => dissem_segments.push(seg),
            SegmentClass::Other => other_segments.push(seg),
        }
    }

    if class_segments.is_empty() {
        return None;
    }

    // Detect non-US markings: any classification segment is a NATO,
    // JOINT, or FGI classification (not a US classification level).
    let is_non_us = class_segments
        .iter()
        .any(|s| is_non_us_classification_segment(s));

    // Already-canonical check: if the classification segment is the
    // first non-empty segment, no reorder is needed.
    // For non-US markings: also require that the body already starts
    // with `//` (the empty US classification slot). If the class is
    // first but the `//` prefix is absent, fall through to add it.
    if let Some(first) = segments.iter().find(|s| !s.trim().is_empty()) {
        if class_segments.contains(&first.trim()) {
            // US: already canonical.
            // Non-US: already canonical only when // prefix is present.
            if !is_non_us || body.starts_with("//") {
                return None;
            }
        }
    }

    // Emit: classification → other (SCI/SAR/FGI blocks) → dissem.
    let mut ordered: Vec<&str> = Vec::new();
    ordered.extend(class_segments);
    ordered.extend(other_segments);
    ordered.extend(dissem_segments);

    let joined = ordered.join("//");

    // Non-US canonical form: `//{class}//{others}//{dissems}`. The
    // leading `//` represents the empty US classification slot (per
    // CAPCO-2016 §A.6) and signals the strict parser to use the
    // non-US classification code path.
    if is_non_us {
        Some(format!("{prefix}//{joined}{suffix}"))
    } else {
        Some(format!("{prefix}{joined}{suffix}"))
    }
}

/// Which CAPCO category a `//`-separated segment primarily belongs to.
///
/// A segment is classification-dominant if its first token is a known
/// classification level (`U`, `C`, `S`, `TS`, `CONFIDENTIAL`, …).
/// Dissem-dominant if its first token is a known dissem control
/// (`NOFORN`, `NF`, `ORCON`, …). Otherwise Other (SCI/SAR/FGI
/// sub-blocks, REL TO lists, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentClass {
    Classification,
    Dissem,
    Other,
}

fn classify_segment(seg: &str) -> SegmentClass {
    let first_token = seg.split_whitespace().next().unwrap_or("");
    // Strip trailing commas.
    let first_token = first_token.trim_end_matches(',');
    // Single-whitespace-token classifications only. `TOP SECRET` and
    // multi-word NATO/JOINT forms are handled by the separate
    // starts_with branches below.
    const CLASSIFICATIONS: &[&str] = &[
        "U",
        "R",
        "C",
        "S",
        "TS",
        "UNCLASSIFIED",
        "RESTRICTED",
        "CONFIDENTIAL",
        "SECRET",
        // NATO classification abbreviations (single-token forms).
        // The five legacy compound forms (CTSA / NSAT / NCA / CTS-B /
        // CTS-BALK) stay in the decoder recognition set because the
        // strict parser, post-PR-9c.1 T134, canonicalizes them into
        // bare class + AEA/SCI companion writes (CAPCO-2016 §H.7 p122
        // for ATOMAL → AEA; §G.2 p40 + §H.7 p127 for BALK/BOHEMIA →
        // SCI). The E066 autofix rule then surfaces the text-level
        // re-marking suggestion per the §G.2 p40 Table 5 registration
        // of the canonical control-marking forms.
        "NS",
        "NC",
        "NU",
        "CTS",
        "CTSA",
        "NSAT",
        "NCA",
        "CTS-B",
        "CTS-BALK",
        // JOINT classification indicator.
        "JOINT",
    ];
    // Dissemination-control tokens — IC (§H.8) and non-IC (§H.9).
    // SCI controls (HCS, SI, TK, and all their sub-compartment forms)
    // are NOT in this list — they belong to their own category under
    // CAPCO §A.6 and the canonical order places them between
    // classification and dissem. Classifying an HCS segment as Dissem
    // would drive `try_canonical_reorder` to move it past the dissem
    // block, corrupting the rewrite. SCI segments therefore fall
    // through to `SegmentClass::Other`, which the reorder helper
    // inserts between classification and dissem — the right spot per
    // CAPCO-2016 §A.6.
    //
    // AEA controls (RD, FRD, TFNI, CNWDI, SIGMA) are also omitted —
    // they appear between SCI and dissem per §A.6. A pre-check above
    // `CLASSIFICATIONS.contains` prevents "RESTRICTED DATA" from being
    // mistaken for the NATO RESTRICTED classification.
    //
    // "REL" is the first token of "REL TO {country-list}" segments.
    //
    // Non-IC dissem controls (§H.9): portion marks (DS, XD, ND,
    // SBU, SBU-NF, LES, LES-NF, SSI) and banner abbreviations
    // (LIMDIS, EXDIS, NODIS) are included so reordering places them
    // in the dissem block, not the SCI/AEA block (CAPCO-2016 §A.6).
    const DISSEMS: &[&str] = &[
        // §H.8 IC dissemination controls
        "NOFORN", "NF", "ORCON", "OC", "PROPIN", "PR", "IMCON", "IMC", "RELIDO", "RS", "RSEN",
        "DSEN", "FISA", "FOUO", "EYES", "REL",
        // §H.9 non-IC dissemination controls — portion marks
        "DS", "XD", "ND", "SBU", "SBU-NF", "LES", "LES-NF", "SSI",
        // §H.9 non-IC dissemination controls — banner abbreviations
        "LIMDIS", "EXDIS", "NODIS",
    ];
    // Pre-check: "RESTRICTED DATA" (AEA marking, §H.6) must not be
    // mistaken for the NATO RESTRICTED classification even though
    // "RESTRICTED" appears in CLASSIFICATIONS. The bare token
    // "RESTRICTED" IS valid as NATO classification; "RESTRICTED DATA"
    // and longer AEA forms are not. CAPCO-2016 §H.6 p113.
    if first_token == "RESTRICTED" && seg.split_whitespace().nth(1).is_some() {
        return SegmentClass::Other;
    }
    if CLASSIFICATIONS.contains(&first_token) {
        SegmentClass::Classification
    // Single-token dissem controls and multi-word non-IC long-title forms.
    // Multi-word forms cannot be single-token-matched because their first words
    // ("LIMITED", "NO", "EXCLUSIVE", "LAW", "SENSITIVE") are too ambiguous;
    // they are checked via starts_with here. CAPCO-2016 §H.8–9.
    } else if DISSEMS.contains(&first_token)
        || (first_token == "LIMITED" && seg.starts_with("LIMITED DISTRIBUTION"))
        || (first_token == "NO" && seg.starts_with("NO DISTRIBUTION"))
        || (first_token == "EXCLUSIVE" && seg.starts_with("EXCLUSIVE DISTRIBUTION"))
        || (first_token == "LAW" && seg.starts_with("LAW ENFORCEMENT SENSITIVE"))
        || (first_token == "SENSITIVE"
            && (seg.starts_with("SENSITIVE BUT UNCLASSIFIED")
                || seg.starts_with("SENSITIVE SECURITY INFORMATION")))
    {
        SegmentClass::Dissem
    } else if (first_token == "TOP" && seg.starts_with("TOP SECRET"))
        || (first_token == "COSMIC" && seg.starts_with("COSMIC TOP SECRET"))
        || (first_token == "NATO"
            && (seg.starts_with("NATO SECRET")
                || seg.starts_with("NATO CONFIDENTIAL")
                || seg.starts_with("NATO UNCLASSIFIED")
                || seg.starts_with("NATO RESTRICTED")))
    {
        SegmentClass::Classification
    } else if CapcoTokenSet.is_trigraph(first_token) {
        // FGI pattern: {registered country trigraph} {classification level}.
        // Validated against the authoritative CVEnumISMCATRelTo vocabulary so
        // typos like "OTP" (→ TOP) don't get mistaken for FGI country codes.
        let second = seg.split_whitespace().nth(1).unwrap_or("");
        let second = second.trim_end_matches(',');
        if matches!(
            second,
            "U" | "R"
                | "C"
                | "S"
                | "TS"
                | "UNCLASSIFIED"
                | "RESTRICTED"
                | "CONFIDENTIAL"
                | "SECRET"
        ) || (second == "TOP"
            && seg
                .split_whitespace()
                .nth(2)
                .is_some_and(|t| t.trim_end_matches(',') == "SECRET"))
        {
            SegmentClass::Classification
        } else {
            SegmentClass::Other
        }
    } else {
        SegmentClass::Other
    }
}

/// Returns true when `seg` is a non-US classification segment: a NATO
/// classification abbreviation, a JOINT classification phrase, or an FGI
/// `{trigraph} {level}` pattern.
///
/// Used by `try_canonical_reorder` to decide whether the reordered output
/// needs a leading `//` (the empty US classification slot that signals the
/// strict parser to take the non-US code path).
fn is_non_us_classification_segment(seg: &str) -> bool {
    const NATO_ABBREVS: &[&str] = &[
        "NS", "NC", "NU", "CTS", "CTSA", "NSAT", "NCA", "CTS-B", "CTS-BALK",
    ];
    let mut tokens = seg.split_whitespace();
    let first = tokens.next().unwrap_or("");
    let first = first.trim_end_matches(',');
    if NATO_ABBREVS.contains(&first) {
        return true;
    }
    if first == "JOINT" {
        return true;
    }
    if first == "COSMIC" && seg.starts_with("COSMIC TOP SECRET") {
        return true;
    }
    if first == "NATO"
        && (seg.starts_with("NATO SECRET")
            || seg.starts_with("NATO CONFIDENTIAL")
            || seg.starts_with("NATO UNCLASSIFIED")
            || seg.starts_with("NATO RESTRICTED"))
    {
        return true;
    }
    // FGI: {registered country trigraph} {classification level}.
    // Validated against the authoritative CVEnumISMCATRelTo vocabulary so
    // typos like "OTP" (→ TOP) are not mistaken for FGI country codes.
    if CapcoTokenSet.is_trigraph(first) {
        let second = tokens.next().unwrap_or("");
        let second = second.trim_end_matches(',');
        if matches!(
            second,
            "U" | "R"
                | "C"
                | "S"
                | "TS"
                | "UNCLASSIFIED"
                | "RESTRICTED"
                | "CONFIDENTIAL"
                | "SECRET"
        ) {
            return true;
        }
        if second == "TOP"
            && tokens
                .next()
                .is_some_and(|t| t.trim_end_matches(',') == "SECRET")
        {
            return true;
        }
    }
    false
}

/// Prepends the non-US leading `//` when the entire input (no existing `//`)
/// looks like a non-US classification segment.
///
/// This covers bare non-US markings like `NS`, `JOINT S GBR USA`, or
/// `CAN S` that arrive with no delimiter at all — `try_canonical_reorder`
/// cannot act on them because it requires at least two `//`-separated
/// segments. Emitting `//NS`, `//JOINT S GBR USA`, etc. lets the strict
/// parser recognize the non-US code path (CAPCO-2016 §A.6, parser block 1).
fn try_add_non_us_prefix(text: &str) -> Option<String> {
    // Only act when there is no `//` at all — try_canonical_reorder
    // handles the has-// but missing-prefix case.
    if text.contains("//") {
        return None;
    }
    let (prefix, body, suffix) = if text.starts_with('(') && text.ends_with(')') {
        ("(", &text[1..text.len() - 1], ")")
    } else {
        ("", text, "")
    };
    if is_non_us_classification_segment(body.trim()) {
        Some(format!("{prefix}//{body}{suffix}"))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// FR-011 strict-context floor
// ---------------------------------------------------------------------------

/// True when `marking`'s classification level is ≥ `floor`.
///
/// FR-011 invariant. `floor` is the `Classification as u8` encoding
/// (Unclassified=0 … TopSecret=4) — see [`ParseContext::classification_floor`].
///
/// A marking with no classification info cannot clear a non-trivial
/// floor — return `false` so the candidate is dropped when the floor
/// is CONFIDENTIAL or above.
fn meets_classification_floor(marking: &CapcoMarking, floor: u8) -> bool {
    let Some(level) = marking_classification(marking) else {
        return floor == Classification::Unclassified as u8;
    };
    (level as u8) >= floor
}

/// Extract the effective classification level from a parsed marking.
///
/// Delegates to [`marque_ism::MarkingClassification::effective_level`],
/// which handles all variants (`Us`, `Fgi`, `Nato`, `Joint`,
/// `Conflict`) by mapping each to the canonical [`Classification`]
/// ladder. NATO levels map through
/// [`NatoClassification::us_equivalent`](marque_ism::NatoClassification::us_equivalent).
fn marking_classification(marking: &CapcoMarking) -> Option<Classification> {
    marking
        .0
        .classification
        .as_ref()
        .map(|c| c.effective_level())
}

/// True when the parsed marking carries at least one recognized
/// attribute — any classification, SCI / SAR / AEA / FGI / dissem /
/// REL-TO entry, or CAB field (Classified By, Derived From,
/// Declassify On, declass exemption).
///
/// Distinct from [`strict_parse_is_complete`]: a marking can be
/// nontrivial (has a dissem control) while still being incomplete
/// (missing its classification). The dispatcher consults both — a
/// strict result is only accepted when it is BOTH nontrivial AND
/// complete; otherwise the decoder is invoked to try to recover the
/// missing pieces.
///
/// True when `bytes` is a portion-shaped slice whose inner content
/// is exactly one ASCII letter — `(s)`, `(c)`, `(u)`, `(r)`, `(S)`,
/// etc. Tolerant of leading whitespace; the strict recognizer
/// already accepts a small amount of leading whitespace on portion
/// candidates (`StrictRecognizer::recognize` strips it before
/// parsing) and the prose-glue heuristic must do the same so the
/// caller's `cx.preceded_by_whitespace` flag remains the authoritative
/// signal for "is this glued to a word."
///
/// Used by [`DecoderRecognizer::recognize`] for the prose-glue
/// suppression early-out. A 2-letter inner content like `(TS)` is
/// outside the heuristic's scope — multi-letter classification
/// abbrevs are rare in prose and don't share the plural-suffix
/// confusability that drives this filter.
fn is_single_letter_portion(bytes: &[u8]) -> bool {
    let trimmed = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map(|i| &bytes[i..])
        .unwrap_or(bytes);
    matches!(trimmed, [b'(', inner, b')'] if inner.is_ascii_alphabetic())
}

/// Bare-classification-shape whitelist for the null-hypothesis gate
/// (issue #472).
///
/// A portion-shaped input whose inner content is exactly a canonical
/// classification token — `(U)`, `(C)`, `(S)`, `(TS)`, `(R)`, or one of
/// the NATO portion abbreviations (`NU`, `NR`, `NC`, `NS`, `CTS`) —
/// is the strict-grammar shape of a valid classification portion. The
/// null-hypothesis filter MUST NOT suppress these: their byte form is
/// short enough that a prose-side prior derived from observed bytes
/// can outweigh the marking-side prior even when the strict grammar
/// unambiguously accepts the form (e.g., short single-letter tokens
/// have non-trivial prose mass as standalone parenthetical glyphs but
/// are also the *only* CAPCO portion shape that exists for those
/// classification levels).
///
/// The list is closed and byte-exact: leading/trailing whitespace
/// inside the parens (`( C )`) is not matched, mixed case (`(cts)`,
/// `(Ts)`) is not matched. This is intentional — case folding to a
/// canonical bare form happens in the decoder's canonicalization
/// stage; this gate operates on the raw observed bytes the caller
/// passed to `recognize`, before any case-fold, so a lowercase or
/// mixed-case input still goes through the null-hypothesis filter and
/// is suppressed when the prose hypothesis dominates.
///
/// Companion to [`has_double_slash`] in the score-time null gate
/// (`recognize` §5). Together they pass through (a) bare-classification
/// portion shapes the grammar uniquely accepts and (b) any portion
/// carrying a category separator (`//`) — the latter being a shape no
/// English prose convention produces.
fn is_bare_classification_shape(bytes: &[u8]) -> bool {
    matches!(
        bytes,
        b"(U)"
            | b"(C)"
            | b"(S)"
            | b"(TS)"
            | b"(R)"
            | b"(NU)"
            | b"(NR)"
            | b"(NC)"
            | b"(NS)"
            | b"(CTS)"
    )
}

/// Does the input contain a `//` category separator anywhere in its
/// bytes? Used by the null-hypothesis gate (issue #472).
///
/// A portion or banner shape containing `//` is by construction not a
/// prose accident: English prose convention has no use for adjacent
/// double-slashes inside parentheses or at line position. The
/// presence of `//` is sufficient evidence that the input intends to
/// be a marking; the score-time null filter passes such candidates
/// through without the prose-vs-marking comparison.
///
/// Byte-windowed search — no allocation. Linear in `bytes.len()`,
/// trivially short for portion/banner shapes.
fn has_double_slash(bytes: &[u8]) -> bool {
    bytes.windows(2).any(|w| w == b"//")
}

/// Per-token prose log-prior floor for **observed** tokens absent
/// from the prose priors table (issue #472).
///
/// Distinct from [`marque_capco::priors::MISSING_PROSE_LOG_PRIOR`]
/// (`-12.0`), which is the floor for the *canonical*-token side of
/// the comparison. The canonical floor was sized to match
/// [`MISSING_TOKEN_LOG_PRIOR`] so that an unknown CAPCO token
/// contributes a zero marking-y delta — the right calibration for a
/// post-canonicalization vocabulary check.
///
/// The observed-side is calibrated differently. An observed token
/// that is **not** in the prose priors table is, by construction:
///
/// 1. Not a canonical CAPCO token (the vocabulary is closed, the
///    priors table covers every canonical token via the Laplace-
///    smoothed zero-count entry from `derive_priors`).
/// 2. Either a non-vocabulary string the user actually typed
///    (`CMS`, `MD`, `PR`, …) or a fuzzy-correctable variant of a
///    vocabulary token. Either way it is shape-equivalent to a
///    prose acronym: short, all-caps or mixed-case, occurring
///    inside a parenthetical glyph at non-anchor line position.
///
/// `-7.0` (e^-7 ≈ 9e-4) sits in the middle of the in-table prose
/// prior range (`-3` to `-12`): less informative than a known
/// high-frequency prose token (USA at `-2.0`, S at `-5.49`), more
/// informative than the canonical missing-token floor (`-12.0`). It
/// roughly corresponds to "moderate prose mass" — the conservative
/// estimate for an unknown short all-caps acronym mid-prose. The
/// SC-003a Federalist `(s)` regression and the issue-472 `(CMS)` /
/// `(MD)` parenthetical-acronym suppression both clear the
/// `NULL_HYPOTHESIS_LOG_MARGIN = 2.5` gate at this floor while
/// legitimate single-token mangled-marking recoveries
/// (`(SERCET//NF)`) stay above it because the marking-side prior
/// for `SECRET` dominates.
const OBSERVED_UNKNOWN_PROSE_LOG_PRIOR: f32 = -7.0;

/// Compute the prose-side log-prior sum for the input bytes (issue
/// #472).
///
/// Walks the original `bytes` slice (as received by `recognize`) to
/// produce a bag of observed tokens, then sums
/// [`marque_capco::priors::token_prose_log_prior`] per distinct token.
/// This is the **observed** null hypothesis: how prose-like are the
/// bytes the user actually typed, irrespective of what canonical form
/// the decoder later canonicalizes them to?
///
/// Pre-#472 the null-side prior was summed over the *canonical* tokens
/// produced after fuzzy correction, so an observed `(CMS)` whose
/// fuzzy-correction landed on `CTS` would contribute the prose prior
/// for `CTS` (rare in prose) to the null hypothesis even though the
/// user typed `CMS` (a much-more-common prose acronym). The fuzzy
/// correction was silently shifting the null-side mass away from the
/// shape the user actually produced. Re-deriving observed tokens here
/// restores the symmetric comparison: marking prior vs prose prior
/// over the *observed* bytes.
///
/// Tokenization is intentionally simple — split on `()/,-` and
/// whitespace, uppercase each piece, look up against the prose priors
/// table. Order does not matter (the result is a sum); duplicates are
/// removed via linear-search dedup over a `SmallVec` (typical token
/// counts are ≤ 8 for portion shapes). Unknown observed tokens fall
/// back to [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`] (`-7.0`) — see that
/// constant's doc for why this differs from
/// [`marque_capco::priors::MISSING_PROSE_LOG_PRIOR`] (`-12.0`, used
/// for the canonical-token path elsewhere).
///
/// **Dedup is an approximation (Copilot #5).** Tokens are dedup'd by
/// a 16-byte uppercase prefix, not by their full bytes. Tokens longer
/// than 16 bytes that share a prefix collide on the dedup key and
/// suppress one another's contribution to the sum. The effect is
/// bounded (one missed [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`] term per
/// collision pair) and biased toward marking-side admission, which
/// is the safe-for-correctness direction. The priors-table lookup is
/// unaffected — no CAPCO vocabulary entry exceeds 15 bytes, so
/// tokens beyond the truncation boundary would always fall to the
/// unknown-prose floor anyway. See the inline comment in the
/// function body for the full discussion.
///
/// **Constitution V Principle V**: the observed bytes are read here
/// to compute an `f32` log-prior sum, and the function returns only
/// the scalar. No byte content escapes through the return path. The
/// caller writes the resulting `f32` into `ScoredCandidate::null_posterior`,
/// where it flows into the decoder's scoring math but never reaches
/// `AppliedFix.proposal.original`, `proposal.replacement`, or the
/// R001 diagnostic message (those channels were closed in PR #259).
fn observed_prose_log_prior(bytes: &[u8]) -> f32 {
    let mut sum: f32 = 0.0;
    let mut seen: SmallVec<[[u8; 16]; 16]> = SmallVec::new();
    let mut seen_lens: SmallVec<[u8; 16]> = SmallVec::new();

    let mut start: Option<usize> = None;
    let mut i = 0;
    while i <= bytes.len() {
        let is_sep = if i == bytes.len() {
            true
        } else {
            let b = bytes[i];
            matches!(b, b'(' | b')' | b'/' | b',' | b'-') || b.is_ascii_whitespace()
        };
        if is_sep {
            if let Some(s) = start.take()
                && i > s
            {
                let raw = &bytes[s..i];
                // Skip empty / non-alphanumeric-only tokens.
                if raw.iter().any(|b| b.is_ascii_alphanumeric()) {
                    // Uppercase into a stack buffer; priors keys are
                    // uppercase.
                    //
                    // **Truncation is a deliberate approximation
                    // (Copilot #5).** The 16-byte stack key truncates
                    // observed tokens longer than 16 bytes for dedup
                    // purposes. The lookup against the priors tables
                    // is unaffected — tokens >16 bytes will not match
                    // a priors entry anyway (no CAPCO vocabulary
                    // entry exceeds 15 bytes, `AUSTRALIA_GROUP`), so
                    // they fall to [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`]
                    // regardless of whether we hashed the full bytes
                    // or a 16-byte prefix. The dedup is the only
                    // operation that sees the truncated key.
                    //
                    // **Dedup effect when truncation collides.** Two
                    // distinct prose tokens >16 bytes that share a
                    // 16-byte prefix (`internationalization` vs
                    // `internationalizes`, hyphen-less compounds,
                    // long identifiers without separators) collide
                    // on the truncated key and dedup-suppress one
                    // contribution. Each suppressed contribution
                    // would have been a single
                    // [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`] (`-7.0`)
                    // term added to the null sum. The miss therefore
                    // makes the null hypothesis slightly LESS
                    // negative — i.e., biased toward marking-side
                    // admission, which is the safe-for-correctness
                    // direction here (we under-suppress potential
                    // false positives rather than over-suppress
                    // recoveries). The effect is bounded at one
                    // missed `-7.0` per collision pair and stays
                    // small relative to the
                    // [`NULL_HYPOTHESIS_LOG_MARGIN`] (`+2.5`) gate.
                    //
                    // If profiling later shows the cost matters, the
                    // dedup can move to a slice hash (`FxHash` or
                    // `rapidhash` over the full token bytes) without
                    // changing this function's external contract.
                    let mut buf = [0u8; 16];
                    let take = raw.len().min(16);
                    for (dst, src) in buf[..take].iter_mut().zip(&raw[..take]) {
                        *dst = src.to_ascii_uppercase();
                    }
                    let key = &buf[..take];
                    // Dedup over seen tokens (linear, N ≤ 8 typical).
                    // See the truncation-approximation note above for
                    // why occasional false-positive matches on long
                    // prose tokens are acceptable.
                    let already = seen
                        .iter()
                        .zip(seen_lens.iter())
                        .any(|(b, l)| &b[..*l as usize] == key);
                    if !already {
                        seen.push(buf);
                        seen_lens.push(take as u8);
                        // Lazy table fallback (Copilot #6): the
                        // country-code lookup runs only when the
                        // token-table lookup returned `None`. Most
                        // observed tokens hit `token_prose_log_prior`
                        // (the common case for prose acronyms in the
                        // priors corpus), so eagerly evaluating
                        // `country_code_prose_log_prior` was wasted
                        // work. `or_else` short-circuits.
                        //
                        // Prefer the token table; fall back to country
                        // table for trigraph/tetragraph shapes that
                        // appear only there. Both tables are sourced
                        // from the same prose stratum so summing once
                        // (token OR country, not both) keeps the null
                        // hypothesis a single log-prior over the
                        // observed token bag.
                        //
                        // Unknown observed tokens fall to
                        // [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`] (`-7.0`),
                        // not [`marque_capco::priors::MISSING_PROSE_LOG_PRIOR`]
                        // (`-12.0`). See the constant's doc — the
                        // observed-side and canonical-side floors are
                        // intentionally asymmetric: an observed unknown
                        // token is shape-equivalent to a prose acronym
                        // (the user typed something not in the CAPCO
                        // vocabulary), and `-7.0` reflects "moderate
                        // prose mass" rather than the canonical
                        // post-vocab-check zero-signal floor.
                        let prior = std::str::from_utf8(key)
                            .ok()
                            .and_then(|s| {
                                marque_capco::priors::token_prose_log_prior(s).or_else(|| {
                                    marque_capco::priors::country_code_prose_log_prior(s)
                                })
                            })
                            .unwrap_or(OBSERVED_UNKNOWN_PROSE_LOG_PRIOR);
                        sum += prior;
                    }
                }
            }
        } else if start.is_none() {
            start = Some(i);
        }
        i += 1;
    }
    sum
}

/// Does this prefix look like a bullet, list, or section anchor?
///
/// Recognizes the common forms of enumeration prefix that precede
/// portion markings in legitimate IC and legal-style documents:
///
/// - `1. `, `12. `, `1) `, `1.2.3.` — numeric/alphanumeric with dot/paren
/// - `a. `, `a) `, `(a) `, `[a] ` — letter or parenthesized letter
/// - `1B.a.3.` — mixed alphanumeric with dot separators
/// - `* `, `- `, `• ` — bullet glyphs
///
/// Trailing whitespace is stripped before evaluation; the prefix
/// must end on a structural punctuation character that anchors the
/// enumeration (`.`, `)`, `]`, `*`, `-`).
fn looks_like_bullet_anchor(prefix: &[u8]) -> bool {
    /// Maximum length of any single alphanumeric run inside an
    /// enumeration body. `1B` (2), `12` (2), `123` (3) pass cleanly
    /// because they contain a digit. `iii` (3, Roman numeral) only
    /// passes when wrapped in parens — `(iii)` is accepted, bare
    /// `iii.` is not. The cap plus the digit-or-bracketed constraint
    /// is what distinguishes enumeration anchors from running prose
    /// ending in a short word + period (`the.`, `for.`, `its.`).
    const ANCHOR_RUN_MAX: usize = 3;
    /// Maximum length of an alpha-only (no digit) run that is NOT
    /// inside parens/brackets. `a.` / `b.` / `(a)` (1) pass; bare
    /// `vs.` (2-letter prose abbrev), bare `the.` (3, English
    /// word), and bare `ii.` / `iv.` (Roman numerals unparenthesized)
    /// do not. Inside brackets the longer `(ii)` / `(iii)` / `(iv)`
    /// forms are accepted via the bracket-context check below. The
    /// trade-off: bare Roman numerals are rejected. IC and legal
    /// docs overwhelmingly use the parenthesized form, so the
    /// false-negative is rare and the false-positive class —
    /// 2-letter prose abbreviations `vs.` / `cf.` / `eg.` —
    /// is much more common in running text.
    const ANCHOR_ALPHA_BARE_RUN_MAX: usize = 1;

    // Trim ASCII whitespace from both ends. Engine-emitted line
    // prefixes can carry leading indentation (`    * `, `\t- `) and
    // trailing space after the bullet glyph (`* `); we want the
    // bullet/anchor token in isolation for the equality test below.
    let mut start = 0;
    let mut end = prefix.len();
    while start < end && prefix[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && prefix[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let trimmed = &prefix[start..end];
    if trimmed.is_empty() {
        return false;
    }

    // Single-glyph bullet anchors. Exact-match so a hyphenated word
    // fragment like `re-` / `un-` / `co-` (which trims to a 3-byte
    // tail ending in `-`) is correctly NOT treated as a bullet
    // anchor. ASCII `*` / `-` plus the Unicode bullet `•` (U+2022 =
    // `\xE2\x80\xA2`).
    if matches!(trimmed, b"*" | b"-" | b"\xE2\x80\xA2") {
        return true;
    }

    let last = trimmed[trimmed.len() - 1];
    // Structural anchor must end on `.`, `)`, or `]`.
    if !matches!(last, b'.' | b')' | b']') {
        return false;
    }

    // The body (bytes before the anchor punctuation) must be a
    // short alphanumeric/separator sequence. Constraints:
    //
    // - Each alphanumeric run is ≤ ANCHOR_RUN_MAX (3) characters.
    // - Alpha-only runs (no digit in the run) are limited to
    //   ANCHOR_ALPHA_BARE_RUN_MAX (2) characters UNLESS the run
    //   sits inside an open bracket — so `(iii)` works but `the.`
    //   does not.
    // - A body containing separator characters (dot or bracket)
    //   MUST contain either a digit OR an opening bracket. This
    //   rejects `e.g.` and `e.g)` — prose abbreviations with
    //   internal dots — while still accepting `1.2.`, `1B.a.3.`,
    //   and `(iii)` enumeration patterns.
    //
    // This is what distinguishes `1B.a.3.` (runs of 2, 1, 1, with
    // a digit in the first), `(iii)`, and `(a)` from `prose.`,
    // `the.`, `e.g.`, `Notwithstanding.`.
    if trimmed.len() < 2 {
        return false;
    }
    let body = &trimmed[..trimmed.len() - 1];
    let mut current_run = 0usize;
    let mut current_run_alpha_only = true;
    let mut bracket_depth: u32 = 0;
    let mut body_had_opener = false;
    let mut body_had_digit = false;
    let mut body_had_separator = false;
    for &b in body {
        if b.is_ascii_alphanumeric() {
            current_run += 1;
            if b.is_ascii_digit() {
                current_run_alpha_only = false;
                body_had_digit = true;
            }
            if current_run > ANCHOR_RUN_MAX {
                return false;
            }
            if current_run > ANCHOR_ALPHA_BARE_RUN_MAX
                && current_run_alpha_only
                && bracket_depth == 0
            {
                return false;
            }
        } else if matches!(b, b'(' | b'[') {
            body_had_opener = true;
            body_had_separator = true;
            bracket_depth = bracket_depth.saturating_add(1);
            current_run = 0;
            current_run_alpha_only = true;
        } else if matches!(b, b')' | b']') {
            body_had_separator = true;
            bracket_depth = bracket_depth.saturating_sub(1);
            current_run = 0;
            current_run_alpha_only = true;
        } else if b == b'.' {
            body_had_separator = true;
            current_run = 0;
            current_run_alpha_only = true;
        } else {
            return false;
        }
    }
    // A body with internal separators must have either a digit
    // (numeric enumeration: `1.2.`, `1B.a.3.`) or an opening
    // bracket (parenthesized enumeration: `(a)`, `(iii)`). A
    // body with separators but neither is prose with internal
    // punctuation (`e.g.`, `e.g)`, `i.e.`) and must NOT be
    // treated as a bullet anchor.
    if body_had_separator && !body_had_digit && !body_had_opener {
        return false;
    }
    // `bracket_depth` is intentionally unconstrained at end-of-
    // loop. The structural terminator (`)` / `]`) lives in `last`
    // and is consumed before the body iteration; for `(a)` /
    // `[a]` the body ends with `bracket_depth == 1` because the
    // matching closer was never seen by the body walk. The
    // variable's only meaningful use is the in-loop "are we
    // inside parens right now" gate for the alpha-only run cap,
    // so its terminal value carries no failure consequence.
    // Reject empty alphanumeric body (e.g., `()`, `..`). Covered
    // partially by `trimmed.len() < 2` above; this final check
    // catches `()` (3 bytes total, body is `(`).
    body.iter().any(|b| b.is_ascii_alphanumeric())
}

/// Does the candidate byte content contain any lowercase ASCII letter?
///
/// Used by [`FeatureId::LowercaseSurroundingContext`] gate: a candidate
/// composed only of uppercase letters cannot be a lowercase prose
/// glyph regardless of surrounding context, so the lowercase penalty
/// is short-circuited (archival all-caps documents never trigger it).
fn candidate_has_lowercase(bytes: &[u8]) -> bool {
    bytes.iter().any(|b| b.is_ascii_lowercase())
}

/// Maximum number of context features [`compute_context_features`]
/// can emit per call. Bounded by the function's contract:
///
/// - At most one of [`FeatureId::LinePositionPenalty`] /
///   [`FeatureId::BulletAnchorBonus`] (mutually exclusive).
/// - At most one of [`FeatureId::LowercaseSurroundingContext`].
///
/// Total ≤ 2. The `SmallVec` inline capacity in
/// [`compute_context_features`]'s return type matches this so the
/// common case (and every case, today) stays heap-free. A future
/// third positional feature MUST bump this bound and the `SmallVec`
/// capacity in lock-step — the const exists so the doc comment on
/// `compute_context_features` and the storage shape can't drift
/// independently.
const CONTEXT_FEATURE_MAX: usize = 2;

/// Compute the context features that apply to every scored candidate
/// at this byte position, regardless of canonical-token content.
///
/// Returns up to [`CONTEXT_FEATURE_MAX`] `(FeatureId, delta)` pairs:
///
/// - At most one of [`FeatureId::LinePositionPenalty`] (line offset
///   exceeds [`LINE_POSITION_BUDGET`] and the line prefix doesn't
///   look like an enumeration anchor) or
///   [`FeatureId::BulletAnchorBonus`] (the line prefix matches a
///   bullet/anchor pattern). Mutually exclusive — bullet wins when
///   both predicates would otherwise fire.
/// - [`FeatureId::LowercaseSurroundingContext`] when the candidate
///   carries lowercase letters AND the surrounding context is
///   lowercase-dominant.
///
/// Empty vec when neither feature applies, when the marking is not
/// a portion (banners/CABs are not subject to these features), or
/// when the engine did not populate `line_offset` / `line_prefix`
/// (e.g., direct test-code callers using `ParseContext::default()`).
///
/// **Dispatch semantics.** The features are added uniformly to
/// every surviving scored candidate before the sort/null-filter
/// step. For **single-letter portion candidates** (`(s)`, `(c)`,
/// `(u)`, `(r)`) the features actively gate output: the null
/// filter at [`NULL_HYPOTHESIS_LOG_MARGIN`] compares each
/// candidate's posterior against its own (unshifted)
/// `null_posterior`, so a `-2.0` position or lowercase penalty
/// directly determines whether the candidate clears the prose
/// hypothesis. For **multi-letter portion candidates** (`(NU)`,
/// `(NC)`, `(TS)`, `(SI//NF)`, …) the null filter does not apply,
/// so the uniform additive shift does not change the marking-vs-
/// marking sort order. The features still influence dispatch
/// through the recognition-runner-up calculation
/// (`max(marking_runner_up, top.null_posterior)`) because
/// `null_posterior` is unshifted: a sufficiently large penalty
/// can let the prose null hypothesis win the runner-up slot,
/// lowering the emitted recognition score even when the dispatch
/// stays Unambiguous. The features are also recorded in the audit
/// feature trace for every survivor regardless of dispatch
/// behavior, so post-hoc analysis can see what the decoder saw.
fn compute_context_features(
    kind: MarkingType,
    bytes: &[u8],
    cx: &ParseContext,
) -> SmallVec<[(FeatureId, f32); CONTEXT_FEATURE_MAX]> {
    let mut out: SmallVec<[(FeatureId, f32); CONTEXT_FEATURE_MAX]> = SmallVec::new();

    // Position features only apply to portion shapes. Banners and
    // CABs have structural evidence the parser already uses.
    if !matches!(kind, MarkingType::Portion) {
        return out;
    }

    // Engine-populated position context fires position features.
    // The default `ParseContext` shape (test code, direct WASM
    // callers) has `line_offset` / `line_prefix` as `None` and skips
    // this block — identical behavior to the pre-feature decoder.
    if let (Some(offset), Some(prefix)) = (cx.line_offset, cx.line_prefix.as_ref()) {
        if looks_like_bullet_anchor(prefix.as_slice()) {
            out.push((FeatureId::BulletAnchorBonus, BULLET_ANCHOR_BONUS));
        } else if offset > LINE_POSITION_BUDGET {
            out.push((FeatureId::LinePositionPenalty, LINE_POSITION_PENALTY));
        }
    }

    if cx.surrounding_is_lowercase && candidate_has_lowercase(bytes) {
        out.push((
            FeatureId::LowercaseSurroundingContext,
            LOWERCASE_CONTEXT_PENALTY,
        ));
    }

    out
}

/// Used inside the decoder itself to filter out lenient-parse-
/// accepts-anything results (`FROBNITZ//WIBBLE` trip-fires the
/// banner scanner and produces a zero-attribute parse); without
/// the filter, every `X//Y` prose fragment would materialize a
/// fabricated empty marking candidate.
/// True when a parsed marking carries enough recognized fields to
/// surface as a real `CapcoMarking` candidate to the rule layer.
///
/// The strict parser is lenient by design — `FROBNITZ//WIBBLE` will
/// trip the banner scanner and produce a zero-attribute parse — so
/// this predicate filters out shape-matches that didn't resolve any
/// tokens against the CVE vocabulary. Used by
/// [`DecoderRecognizer::recognize`] step 3c to discard fabricated
/// empty candidates and by integration tests that need the same
/// "real marking?" predicate to count engine-relevant candidates.
///
/// # Stability
///
/// **Not part of the public API.** Marked `#[doc(hidden)]` so it
/// stays off rustdoc surfaces and `cargo doc` output — the
/// `pub` modifier exists solely so `crates/engine/tests/` can
/// reach it across the integration-test crate boundary
/// (Rust integration tests live in a separate crate and so
/// `pub(crate)` is not visible to them). Downstream consumers
/// MUST NOT depend on this signature; it can change at any
/// time alongside `CapcoMarking` evolution. The supported way
/// to ask "is this marking non-trivial?" is to run
/// [`Engine::lint`] and inspect its emitted diagnostics — the
/// engine applies this filter internally and surfaces only
/// non-trivial markings to the rule layer.
#[doc(hidden)]
pub fn is_nontrivial_marking(marking: &CapcoMarking) -> bool {
    let a = &marking.0;
    a.classification.is_some()
        || !a.sci_controls.is_empty()
        || a.sar_markings.is_some()
        || !a.aea_markings.is_empty()
        || a.fgi_marker.is_some()
        // PR 9b (T132): walk the unified dissem_iter — a marking with
        // any dissem on either namespace is non-trivial.
        || a.dissem_iter().next().is_some()
        || !a.non_ic_dissem.is_empty()
        || !a.rel_to.is_empty()
        || a.classified_by.is_some()
        || a.derived_from.is_some()
        || a.declassify_on.is_some()
        || a.declass_exemption.is_some()
}

/// True when the strict-parse result is complete enough that the
/// dispatcher should accept it and skip the decoder fallback.
///
/// The strict parser (`marque_core::Parser`) is lenient about
/// content: it categorizes tokens by *position* (the first token
/// inside `(...)` is marked as `TokenKind::Classification`
/// regardless of whether its text is a valid classification value),
/// and falls back to `TokenKind::Unknown` only for truly unplaceable
/// tokens. So a shape like `(SERCET//NOFORN)` parses to a marking
/// with `classification: None` (SERCET doesn't resolve to any
/// `Classification` variant), `dissem_controls: [Nf]` (NOFORN was
/// recognized), and a Classification-kind `TokenSpan` carrying the
/// literal text "SERCET". That result is *nontrivial* but also
/// *incomplete* — exactly the mangled-input case the decoder exists
/// to recover.
///
/// Predicate, kind-aware:
///
/// - [`MarkingType::Portion`] / [`MarkingType::Banner`]: complete
///   iff `classification.is_some()` AND no `TokenKind::Unknown`
///   spans survived. Both branches matter — SERCET→None catches
///   the classification-slot typo; the `Unknown` check catches
///   typos in the tail (e.g., `(S//FRBN)` where the classification
///   is fine but FRBN is mangled and lands as Unknown).
/// - [`MarkingType::Cab`]: complete iff any CAB field is present
///   (`classified_by` / `derived_from` / `declassify_on`).
///   CAB-kind input doesn't require a classification axis — an
///   isolated authority block stands on its own.
/// - Anything else: fall back to the generic nontrivial check.
fn strict_parse_is_complete(marking: &CapcoMarking, kind: MarkingType) -> bool {
    use marque_ism::TokenKind;
    let attrs = &marking.0;
    match kind {
        MarkingType::Portion | MarkingType::Banner => {
            attrs.classification.is_some()
                && !attrs
                    .token_spans
                    .iter()
                    .any(|s| matches!(s.kind, TokenKind::Unknown))
        }
        MarkingType::Cab => {
            attrs.classified_by.is_some()
                || attrs.derived_from.is_some()
                || attrs.declassify_on.is_some()
                || attrs.declass_exemption.is_some()
        }
        _ => is_nontrivial_marking(marking),
    }
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Floor log-prior for canonical tokens that don't appear in the
/// baked `TOKEN_BASE_RATES` table.
///
/// Baked priors are `log((hits + 1) / (total + |V|))` with
/// Laplace smoothing over the non-IC Enron corpus (see
/// `tools/corpus-analysis/analyze.py::derive_priors`). A token the
/// corpus never observed still receives a non-zero smoothed prior in
/// that build; this constant exists for the different, rarer case
/// where the canonical-tokens iterator produces a string that was
/// not in the build's vocabulary at all (e.g., a CVE token added
/// after the last priors regeneration). Without this floor, such
/// tokens would silently contribute `0.0` to the sum — and since
/// every real log-prior is negative, a missing token would score
/// HIGHER than a known one, inverting the ranking.
///
/// Magnitude (`-12.0` nats ≈ log(6e-6)) is chosen to be strictly
/// lower than every log-prior the generator would emit for a
/// non-empty corpus: the Enron-derived values bottom out around
/// `-11.7` for the most infrequent observed tokens (see
/// `crates/capco/corpus/priors.json`).
const MISSING_TOKEN_LOG_PRIOR: f32 = -12.0;

/// Posterior penalty applied when a candidate's strict parse buries a
/// reserved dissem-control token (a hard splitter — see
/// [`is_hard_splitter`]) inside a SAR or SCI sub-component slot.
///
/// **Why this exists.** Hard-splitter tokens (NOFORN, ORCON, EXDIS,
/// FOUO, …) have hard reserved meanings as dissem controls per CAPCO-
/// 2016 §H.8/§H.9; they have no in-segment role inside SCI or SAR
/// sub-components. A strict parse that places such a token under
/// [`marque_ism::SarMarking`] or [`marque_ism::SciMarking`] is
/// essentially always a missing-
/// `//` artifact in the input — the alternative parse with the token
/// emitted as a dissem control is the correct interpretation. (REL
/// TO is intentionally excluded from the penalty surface here: its
/// payload is a list of country trigraphs whose grammar accepts only
/// 3-letter alpha codes drawn from the CVE-derived trigraph table,
/// so a 4+-char hard splitter cannot land in a REL TO slot in the
/// first place. The Copilot review on PR #178 flagged a wider doc
/// claim that suggested otherwise — the doc is now scoped to the
/// slots the penalty actually defends.)
///
/// **Why scoring needs help.** The bag-of-tokens scorer above sums
/// log-priors for the marking's canonical tokens, and `for_each_canonical_token`
/// deliberately excludes SAR program/compartment/sub-compartment text
/// (open-set agency-assigned codewords). So an absorbing parse contributes
/// only the classification's prior; the equivalent delim-inserted parse
/// contributes classification + the dissem token's prior, which is a
/// MORE NEGATIVE log-posterior. Without a corrective penalty the
/// absorbing parse always wins. SCI absorption usually self-resolves
/// because [`marque_core::Parser::parse`]'s SCI subgrammar produces
/// [`marque_ism::TokenKind::Unknown`] for non-alphanumeric/wrong-shape
/// compartment tokens (which step 3a then drops), but SAR's grammar accepts any
/// `[A-Z0-9]+` identifier and absorbs cleanly — leaving SAR as the
/// observed failure mode on the SC-004 corpus (the `SAR-BP-J12 …` and
/// `SPECIAL ACCESS REQUIRED-BUTTER POPCORN …` fixtures pre-PR-5).
///
/// **Magnitude.** Empirically the absorbing-vs-delim-inserted spread
/// on those two fixtures is ~9 nats; the [`MISSING_TOKEN_LOG_PRIOR`]
/// floor (`-12.0`) gives a comfortable margin and is robust to small
/// future shifts in the priors table. Defining the penalty as
/// `MISSING_TOKEN_LOG_PRIOR` (rather than re-stating the literal)
/// keeps the two below-floor signals mechanically at parity for any
/// candidate that triggers both — a future ratchet of one constant
/// pulls the other along.
///
/// **Safety.** Hard-splitter tokens are all 4+ chars and have shapes
/// distinct from real SAR identifiers (`BP`, `CD`, `XR` are 2-char;
/// `BUTTER POPCORN`, `J12`, `K15`, `XRA` are alphanumeric short
/// codes that don't collide with the hard-splitter list). So this
/// penalty cannot fire on a legitimate SAR/SCI parse.
const HARD_SPLITTER_ABSORPTION_PENALTY: f32 = MISSING_TOKEN_LOG_PRIOR;

/// Per-entry structural penalty for SCI markings whose control system
/// landed as [`SciControlSystem::Custom`]. Issue #133 PR 6.
///
/// **Why this penalty exists.** `marque_core::Parser`'s structural SCI
/// subparser (CAPCO-2016 §A.6 grammar) accepts any alphanumeric
/// identifier as a "custom" control system / compartment when the
/// segment text contains `-` or `/`. That branch was added so legal
/// compound SCI shapes (`SI-G ABCD DEFG-MMM AACD`) parse correctly,
/// but it has a side effect: a typo'd or stray segment like
/// `USAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB` parses cleanly into
/// three `Custom`-system SCI markings (USAR/CD/XR with attached
/// compartments). The bag-of-tokens scorer can't tell that this is
/// the wrong interpretation — `Custom` SCI control systems don't
/// appear in `for_each_canonical_token`, so they don't shift the prior
/// either way, and the candidate ties with structurally-richer
/// alternatives like the SAR-repaired candidate that
/// `try_sar_indicator_repair` emits.
///
/// **What the penalty does.** Adds [`MISSING_TOKEN_LOG_PRIOR`] (the
/// same below-observed-floor magnitude as
/// [`HARD_SPLITTER_ABSORPTION_PENALTY`]) per `Custom`-system SCI
/// marking. The penalty is per-entry so candidates that absorbed
/// multiple stray segments (like the 3-segment USAR/CD/XR case) get
/// progressively worse posteriors, restoring the SAR-repair
/// candidate's lead by a margin that clears
/// [`UNAMBIGUOUS_LOG_MARGIN`].
///
/// **Magnitude.** Same `-12.0` as the hard-splitter penalty: both are
/// "this parse pattern is highly unlikely in well-formed CAPCO
/// markings" structural signals, and keeping them at parity by
/// definition (rather than literal duplication) lets a future
/// ratchet of one move both together. A single legitimate custom
/// control (the §A.6 p16 `99` example) gets one `-12.0` hit but
/// remains the sole candidate when no alternative interpretation
/// exists, so the dispatcher still emits `Unambiguous`.
///
/// **Safety / discriminator choice.** The discriminator is
/// `sm.system == SciControlSystem::Custom(_)`, NOT
/// `sm.canonical_enum.is_none()`. The two are NOT equivalent:
/// `canonical_enum` is also `None` for legitimate `Published`-system
/// SCI markings whenever the `{system}-{first_compartment}` pair
/// doesn't map to a CVE atom (per the `canonical_enum` doc in
/// `crates/scheme/src/scheme.rs` — populated only when "the bare
/// control or `{ctrl}-{first_comp}` matches a CVE value AND no
/// sub-compartments are present"). Using `canonical_enum` as the
/// discriminator would penalize legitimate `SI-G ABCD DEFG-MMM AACD`-
/// style markings (system=`Published(Si)`, sub-compartments present
/// → canonical_enum=None), broadly skewing scoring against rich
/// SCI shapes. Discriminating on `system` directly catches the
/// USAR/CD/XR custom-only case while leaving every published SCI
/// marking — bare or compound — untouched. A candidate with mixed
/// SCI (e.g., `SI-G ABCD//99`) gets a single penalty for the `99`
/// `Custom` entry only, which is a reasonable cost for a
/// structurally suspicious mixed shape. The penalty does NOT fire
/// on candidates with empty `sci_markings` — so the SAR-repaired
/// candidate (which projects no SCI) is unaffected.
const CUSTOM_SCI_MARKING_PENALTY: f32 = MISSING_TOKEN_LOG_PRIOR;

// (`LENIENT_REL_PREFIX_PENALTY` removed — under the current PR-9
// architecture, `try_rel_to_structural_repair` runs as preprocessing
// on the normalized text before any candidate is emitted, so
// `RELT O ` / `REL OT ` patterns at a token boundary are rewritten
// to canonical `REL TO ` before scoring sees them. The defense-in-
// depth scorer penalty that PR 9 originally introduced was meant to
// break a tie between competing raw vs. repaired *candidates* —
// that tie no longer exists since the repair is no longer a
// separate candidate. The accuracy harness
// (`resolution_rate_at_0_85`, `resolution_rate_does_not_regress`,
// per-class floors) is the load-bearing regression gate for this
// recovery path. Issue #186 (REL TO trigraph corpus-weighted
// recovery) is the followup that handles the remaining lenient-
// header cases via priors rather than scorer penalties.)

/// Bag-of-tokens scorer (foundational-plan §5.2).
///
/// Returns `(prior, posterior)` where:
///
/// - `prior` = Σ [`marque_capco::priors::token_log_prior`] over the
///   marking's canonical tokens **plus** Σ
///   [`marque_capco::priors::country_code_log_prior`] over the
///   marking's `rel_to` country codes (issue #233). This is the prior
///   alone — nothing else — and is what
///   [`Candidate::prior_log_odds`] is documented to carry (see
///   `crates/scheme/src/ambiguity.rs`). Tokens or country codes
///   missing from the baked tables contribute
///   [`MISSING_TOKEN_LOG_PRIOR`] (a below-observed-floor penalty)
///   rather than `0.0`. The country-code contribution is what lets
///   the decoder break fuzzy-correction ties between common (USA,
///   GBR, AUS) and rare-lookalike (USB-not-a-country, UZB, ASM, AUT)
///   trigraphs in REL TO blocks.
/// - `posterior` = `prior + Σ attempt.features[i].delta + structural
///   penalties`. This is the quantity the decoder sorts and thresholds
///   on. The only structural penalty today is
///   [`HARD_SPLITTER_ABSORPTION_PENALTY`], applied when the strict
///   parse buries a reserved dissem-control token in a SAR/SCI slot.
///
/// The null (prose) posterior is **not** computed here. Pre-#472 it
/// was, summed over the marking's canonical tokens; the canonical
/// token set is post-fuzzy-correction so the prose hypothesis was
/// evaluated on tokens the user never typed, biasing the
/// marking-vs-prose comparison whenever fuzzy correction shifted a
/// common prose acronym (e.g., `(CMS)`) to a rare CAPCO token (e.g.,
/// `CTS`). Issue #472 moves the null computation to
/// [`observed_prose_log_prior`], which walks the original `bytes`
/// parameter to `recognize` and sums prose priors per distinct
/// observed token — restoring the symmetric marking-vs-prose
/// comparison. The caller computes the observed null once per
/// `recognize` call and writes it into every
/// [`ScoredCandidate::null_posterior`].
///
/// Splitting prior and posterior prevents the caller from writing the
/// full posterior into `Candidate::prior_log_odds` — that would double-
/// count the feature deltas once any resolver re-adds
/// `EvidenceFeature.log_odds`. Structural penalties are deliberately
/// folded into the posterior only (not the prior or the per-feature
/// log-odds): they are a likelihood statement about parse plausibility,
/// not a corpus-frequency claim about token co-occurrence.
///
/// Precision: computed in `f32` — the baked priors are already `f32`
/// and the feature deltas are small constants (single-digit magnitude
/// at most), so the accumulator doesn't need `f64` headroom for the
/// K=8 candidate set.
///
/// The `kind` parameter selects portion vs banner canonical token
/// forms for the prior computation (e.g., `S` vs `SECRET`) so the
/// marking-side lookup matches the input shape.
fn score_candidate(
    attempt: &CanonicalAttempt,
    marking: &CapcoMarking,
    kind: MarkingType,
) -> (f32, f32) {
    // Prior: sum of baked log-priors for the canonical tokens that
    // appear in the parsed marking. Tokens missing from the baked
    // table receive the floor penalty rather than a neutral 0.0
    // contribution — see the MISSING_TOKEN_LOG_PRIOR doc.
    let mut prior: f32 = 0.0;

    // Issue #451: linear-search dedup over a SmallVec rather than a
    // BTreeSet. N (distinct canonical tokens per marking) is typically
    // ≤10, so a small stack-allocated buffer with linear `iter().any`
    // dedup is cache-friendlier than B-tree node allocations, and
    // folding the prior summation into the same dedup loop kills the
    // intermediate token collection entirely.
    let mut seen_tokens: SmallVec<[&'static str; 16]> = SmallVec::new();
    for_each_canonical_token(marking, kind, |token| {
        if !seen_tokens.contains(&token) {
            seen_tokens.push(token);
            prior +=
                marque_capco::priors::token_log_prior(token).unwrap_or(MISSING_TOKEN_LOG_PRIOR);
        }
    });

    // Country-code prior contribution (issue #233). REL TO country
    // codes are not part of the `for_each_canonical_token` set because
    // `CountryCode::as_str()` returns a borrowed `&str` rather than
    // `&'static str`, and because the per-token corpus coverage for
    // country codes used to be sparse. Issue #233 adds a parallel
    // `COUNTRY_CODE_BASE_RATES` table (issue #186 sub-feature 1) so
    // the decoder can break fuzzy ties between popular codes (USA,
    // GBR, AUS, FVEY, …) and rare lookalikes (UZB, ASM,
    // AUT-as-Austria) by log-prior delta rather than edit distance
    // alone. Look up each observed REL TO code at score-time —
    // shape-agnostic, so the loop handles 2-char (`EU`), 3-char, and
    // 4-char tetragraphs uniformly. Duplicate REL TO entries do not
    // provide additional evidence, so score each distinct country
    // code at most once. Unknown entries fall to
    // MISSING_TOKEN_LOG_PRIOR — the same penalty the non-country-code
    // path uses for unrecognized tokens, which is the correct
    // behavior for a candidate that resolved to a non-CVE country
    // string.
    //
    // Issue #451 sub-finding F3: SmallVec linear-search dedup over the
    // typical N=1-5 REL TO codes, rather than a per-call BTreeSet
    // allocation.
    let mut seen_rel_to_codes: SmallVec<[&str; 8]> = SmallVec::new();
    for country in marking.0.rel_to.iter() {
        let code = country.as_str();
        if !seen_rel_to_codes.contains(&code) {
            seen_rel_to_codes.push(code);
            prior += marque_capco::priors::country_code_log_prior(code)
                .unwrap_or(MISSING_TOKEN_LOG_PRIOR);
        }
    }

    // Posterior: prior plus feature deltas plus structural penalties.
    let feature_sum: f32 = attempt.features.iter().map(|f| f.delta).sum();
    let mut posterior = prior + feature_sum;
    if absorbs_hard_splitter_in_sar_or_sci(marking) {
        posterior += HARD_SPLITTER_ABSORPTION_PENALTY;
    }
    posterior += custom_sci_marking_penalty(marking);

    (prior, posterior)
}

/// Total per-entry penalty for SCI markings whose strict parse landed
/// with [`SciControlSystem::Custom`] as the control system. See
/// [`CUSTOM_SCI_MARKING_PENALTY`] for rationale, including why this
/// discriminates on `sm.system` rather than on
/// `sm.canonical_enum.is_none()`.
fn custom_sci_marking_penalty(marking: &CapcoMarking) -> f32 {
    let attrs = &marking.0;
    let custom_count = attrs
        .sci_markings
        .iter()
        .filter(|sm| matches!(sm.system, SciControlSystem::Custom(_)))
        .count();
    custom_count as f32 * CUSTOM_SCI_MARKING_PENALTY
}

/// True when the strict parse of a candidate buries a hard-splitter
/// dissem-control token (NOFORN, ORCON, EXDIS, FOUO, …) inside a SAR
/// program/compartment/sub-compartment slot or an SCI compartment/
/// sub-compartment slot.
///
/// Used by [`score_candidate`] to apply
/// [`HARD_SPLITTER_ABSORPTION_PENALTY`] — the penalty exists because
/// SAR's grammar accepts any alphanumeric identifier and quietly
/// absorbs trailing dissem-control tokens that should have been
/// separated from the SAR block by `//`. See the
/// `HARD_SPLITTER_ABSORPTION_PENALTY` doc for the full rationale.
///
/// Identifiers are checked both as whole strings AND as whitespace-
/// separated word sequences. The whitespace split matters for the
/// `Full` SAR indicator form (`SPECIAL ACCESS REQUIRED-BUTTER
/// POPCORN`): a multi-word program nickname like `"BUTTER POPCORN"`
/// may have `NOFORN` absorbed as a trailing word, producing
/// `identifier: "BUTTER POPCORN NOFORN"`. Without the per-word
/// check, the absorption pattern slips past the whole-string
/// `is_hard_splitter` lookup.
fn absorbs_hard_splitter_in_sar_or_sci(marking: &CapcoMarking) -> bool {
    let attrs = &marking.0;

    if let Some(sar) = attrs.sar_markings.as_ref() {
        for prog in sar.programs.iter() {
            if contains_hard_splitter_word(&prog.identifier) {
                return true;
            }
            for comp in prog.compartments.iter() {
                if contains_hard_splitter_word(&comp.identifier) {
                    return true;
                }
                if comp
                    .sub_compartments
                    .iter()
                    .any(|sub| contains_hard_splitter_word(sub))
                {
                    return true;
                }
            }
        }
    }

    for sci in attrs.sci_markings.iter() {
        for comp in sci.compartments.iter() {
            if contains_hard_splitter_word(&comp.identifier) {
                return true;
            }
            if comp
                .sub_compartments
                .iter()
                .any(|sub| contains_hard_splitter_word(sub))
            {
                return true;
            }
        }
    }

    false
}

/// True when `s` is a hard-splitter token, or contains a hard-
/// splitter token as a whitespace-separated word. The per-word check
/// covers multi-word `Full` SAR program nicknames (`BUTTER POPCORN`)
/// that absorbed a trailing dissem-control word.
fn contains_hard_splitter_word(s: &str) -> bool {
    if is_hard_splitter(s) {
        return true;
    }
    s.split_whitespace().any(is_hard_splitter)
}

/// Visit each canonical token present in `marking` that has a
/// `&'static str` representation suitable for
/// [`marque_capco::priors::TOKEN_BASE_RATES`] lookup.
///
/// Issue #451: this replaces the previous `canonical_tokens_for ->
/// Vec<&'static str>` shape, which allocated a `BTreeSet` for dedup
/// and a `Vec` for the return on every scored candidate (up to 16 per
/// `recognize()` call). The visitor pattern hands raw, possibly-
/// duplicate tokens to the caller; dedup happens at the call site
/// where it can ride along with whatever per-token work the caller is
/// already doing (e.g., [`score_candidate`] folds the prior summation
/// into the same SmallVec linear-search dedup).
///
/// Scored token families, by `CanonicalAttrs` field:
///
/// - `classification` — effective level's banner string
///   (`SECRET`, `TOP SECRET`, ...).
/// - `sci_controls` — each variant's `as_str()` (`SI`, `TK`, `HCS-P`, ...).
/// - `dissem_controls` — IC dissem variants' `as_str()`
///   (`NF`, `OC`, `RELIDO`, ...).
/// - `non_ic_dissem` — non-IC dissem variants' `banner_str()`
///   (`LIMDIS`, `EXDIS`, `NODIS`, `SBU`, `LES`, ...).
/// - `aea_markings` — category token `"AEA"` when any AEA marking is
///   present. Individual AEA sub-variants (RD / FRD / CNWDI /
///   SIGMA / UCNI variants) are not broken out for scoring because
///   the baked priors don't carry per-sub-variant base rates and
///   adding floor-penalty contributions for each variant would hurt
///   AEA-bearing candidates across the board.
/// - `fgi_marker` — category token `"FGI"` when an FGI marker is set.
///
/// Deliberately NOT included here:
///
/// - `sar_markings` — SAR program identifiers are agency-assigned
///   codewords (open set, not in the baked priors).
/// - `rel_to` country codes — scored separately in
///   [`score_candidate`] via
///   [`marque_capco::priors::country_code_log_prior`] (issue #233).
///   `CountryCode::as_str()` returns a `&str` tied to `&self`, not
///   `&'static str`, so the country-code contribution is summed at
///   score-time rather than collected here.
/// - CAB fields (`classified_by`, `derived_from`, `declassify_on`) —
///   free-form text, not CVE-enumerable.
///
/// Duplicate tokens (the unified `dissem_iter` can yield repeats
/// across namespaces; an `sci_controls` slot could in principle repeat
/// across positions) ARE visited per-occurrence — the caller must
/// dedup if double-counting matters. The previous `BTreeSet`-based
/// implementation deduped internally; the caller-side dedup in
/// [`score_candidate`] preserves the same per-distinct-token behavior.
fn for_each_canonical_token(
    marking: &CapcoMarking,
    kind: MarkingType,
    mut f: impl FnMut(&'static str),
) {
    let attrs = &marking.0;

    if let Some(class) = attrs.classification.as_ref() {
        // Pick the classification token that matches the marking shape and
        // classification system.
        //
        // For US/FGI/Joint portions: single-letter abbrevs (`S`, `C`, `U`, `TS`)
        //   via `effective_level().portion_str()`. Pre-#258 this always used the
        //   banner form; the portion form correctly matches low-prose-frequency
        //   single-letter tokens vs single-letter prose tokens, enabling the
        //   null-hypothesis filter to reject prose inputs like Federalist `(s)`.
        //
        // For NATO portions: use the NATO-specific abbreviation (`NS`, `NR`, `NC`,
        //   `NU`, `CTS`) directly from `NatoClassification::portion_str()`. Using
        //   `effective_level().portion_str()` here would yield `"R"`, `"C"`, etc.
        //   (US equivalents), which have high prose frequency (e.g., `"R"` appears
        //   5 797× in prose vs 1× in marking corpus), causing the null-hypothesis
        //   filter to reject valid NATO-folded portions like `(//NR)`. The NATO
        //   abbreviations have near-zero prose frequency and fall to
        //   `MISSING_TOKEN_LOG_PRIOR` (−12.0) on both sides, giving a neutral
        //   (zero) marking-y delta rather than a prose-weighted penalty. T129 /
        //   #260 fix; companion to `NATO_PORTION_FORMS` in `marque-ism::token_set`.
        //
        // For banner/CAB/PageBreak: always use the full-word form regardless of
        //   classification system (fold is portion-only; banners reach here via
        //   the non-folding strict-recognizer path or decoder direct banner inputs).
        let class_token = match kind {
            MarkingType::Portion => match class {
                MarkingClassification::Nato(n) => n.portion_str(),
                _ => class.effective_level().portion_str(),
            },
            // All non-Portion variants — kept as wildcard for
            // `#[non_exhaustive]` forward-compat (issue #461). The
            // feature extractor is shape-only and `banner_str()` is
            // the safe fallback for any non-portion shape, including
            // future variants of the `MarkingType` enum.
            _ => class.effective_level().banner_str(),
        };
        f(class_token);
    }

    for ctrl in attrs.sci_controls.iter() {
        f(ctrl.as_str());
    }
    // PR 9b (T132): the decoder feature extractor inserts dissem
    // canonical tokens regardless of namespace — the feature vector
    // captures "which control names appear?", not their attribution.
    for dis in attrs.dissem_iter() {
        f(dis.as_str());
    }
    for nic in attrs.non_ic_dissem.iter() {
        // `NonIcDissem::banner_str` returns `&'static str` with the
        // banner form (LIMDIS, EXDIS, NODIS, SBU, LES, SSI,
        // SBU NOFORN, LES NOFORN). The compound forms ("SBU NOFORN",
        // "LES NOFORN") won't hit a single-token priors entry — they
        // fall to MISSING_TOKEN_LOG_PRIOR. That's fine: the
        // comparison against peer candidates remains consistent.
        f(nic.banner_str());
    }
    if !attrs.aea_markings.is_empty() {
        f("AEA");
    }
    if attrs.fgi_marker.is_some() {
        f("FGI");
    }
}

// ---------------------------------------------------------------------------
// Strict + decoder dispatcher
// ---------------------------------------------------------------------------

/// Recognizer that runs the strict path first and falls back to the
/// decoder when the strict parse yields no meaningful attributes.
///
/// Default recognizer installed by [`crate::Engine::new`]. Callers
/// that need strict-only dispatch (the SC-001 interactive-latency
/// benchmark, tests asserting strict behavior) install
/// [`StrictRecognizer`] explicitly via
/// [`crate::Engine::with_recognizer`].
///
/// Within this recognizer, dispatch is keyed off
/// [`ParseContext::strict_evidence`]:
///
/// - `strict_evidence = true`: collapse to strict-only behavior. The
///   decoder is not called. The engine never sets this; it's reserved
///   for callers (e.g., test code) that construct a `ParseContext`
///   directly and want to drive only the strict half of the dispatcher.
/// - `strict_evidence = false` (the engine default): try strict first.
///   Fall back to the decoder when the strict result is either (a)
///   zero-candidate `Ambiguous` or (b) `Unambiguous` with an empty /
///   trivial [`CapcoMarking`] (no classification, no SCI, no dissem,
///   no FGI, etc.). The trivial-Unambiguous case matters because
///   `marque_core::Parser` is lenient: it accepts arbitrary
///   `BYTES//BYTES` shapes and returns `Ok` with an empty
///   `CanonicalAttrs` when nothing in the input is a recognized CVE
///   token. Treating such a result as a successful parse would leave
///   the decoder dormant on exactly the mangled inputs it exists to
///   recover (`SERCET//NOFORN`, `NOFORN//SECRET`, …). The dispatcher
///   passes the caller's [`ParseContext`] through to both inner
///   recognizers unmodified — [`StrictRecognizer::recognize`] ignores
///   every field of `ParseContext` (its parameter is `_cx`), and by
///   the time the dispatcher reaches the decoder leg the
///   `cx.strict_evidence` early return above has already established
///   that the flag is `false`, so the previous
///   clone-with-`strict_evidence`-override was redundant.
///
/// Other [`ParseContext`] fields (`zone`, `position`,
/// `classification_floor`) pass through unchanged.
#[derive(Debug, Default, Clone, Copy)]
pub struct StrictOrDecoderRecognizer {
    strict: StrictRecognizer,
    decoder: DecoderRecognizer,
}

impl StrictOrDecoderRecognizer {
    pub const fn new() -> Self {
        Self {
            strict: StrictRecognizer::new(),
            decoder: DecoderRecognizer::new(),
        }
    }
}

impl Recognizer<CapcoScheme> for StrictOrDecoderRecognizer {
    fn recognize(
        &self,
        bytes: &[u8],
        offset: usize,
        scheme: &CapcoScheme,
        cx: &ParseContext,
    ) -> Parsed<CapcoMarking> {
        // Pass `cx` through to the strict recognizer unmodified.
        // `StrictRecognizer::recognize` ignores every field of
        // `ParseContext` (its parameter is `_cx`), so cloning to
        // override `strict_evidence = true` would be pure overhead on
        // the strict-complete fast path — which is every candidate in
        // a well-formed document. Forward `offset` verbatim — inner
        // recognizers do the shift, the dispatcher never double-shifts
        // (issue #431).
        let strict_result = self.strict.recognize(bytes, offset, scheme, cx);

        // When the outer caller asked for strict-only via
        // `strict_evidence = true`, collapse to the strict result —
        // never call the decoder. The engine never sets this flag (it
        // installs `StrictRecognizer` directly via `with_recognizer`
        // when a strict-only mode is needed); this branch exists for
        // direct callers that construct a `ParseContext` themselves
        // (e.g., test code).
        if cx.strict_evidence {
            return strict_result;
        }

        // Infer the candidate kind from the byte shape so
        // `strict_parse_is_complete` can apply the right rule
        // (classification-requiring for portion/banner, CAB-field-
        // requiring for CAB). If inference fails the bytes are too
        // degenerate for either path — skip and return whatever the
        // strict path produced (most likely zero-candidate Ambiguous).
        let Some(kind) = infer_marking_type(bytes) else {
            return strict_result;
        };

        // Complete strict parse — take it, decoder not needed.
        if matches!(&strict_result, Parsed::Unambiguous(m) if strict_parse_is_complete(m, kind)) {
            return strict_result;
        }

        // Strict already produced non-empty candidates — keep them.
        if matches!(&strict_result, Parsed::Ambiguous { candidates } if !candidates.is_empty()) {
            return strict_result;
        }

        // Remaining cases: either an incomplete-but-Unambiguous strict parse
        // (partial attrs, `TokenKind::Unknown` spans, missing classification,
        // etc.) or a zero-candidate strict Ambiguous. Both warrant a decoder
        // attempt. Cases:
        //   (a) Truly empty attrs (`FROBNITZ//WIBBLE`) — zero-candidate strict.
        //   (b) Partial attrs (`(SERCET//NOFORN)` — NOFORN parsed, SERCET
        //       left in a Classification-kind span with
        //       `attrs.classification = None`) — incomplete Unambiguous.
        //
        // Pass `cx` directly: the `cx.strict_evidence` early return
        // above guarantees the flag is already `false`, so the
        // previous clone-with-override was redundant.
        let decoder_result = self.decoder.recognize(bytes, offset, scheme, cx);

        // Only adopt the decoder result when it produced an Unambiguous
        // marking. If the decoder is also uncertain, preserve the strict
        // result so rules can still fire on any partial attrs — avoiding
        // deep-scan silently reducing observability/diagnostics on
        // mangled input.
        match decoder_result {
            Parsed::Unambiguous(_) => decoder_result,
            _ => strict_result,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::sync::LazyLock;

    use super::*;
    use marque_scheme::recognizer::LinePrefix;

    /// Shared scheme instance for the test module. `CapcoScheme::new()`
    /// builds non-trivial `Vec` tables; constructing it once and
    /// borrowing `&*TEST_SCHEME` avoids repeated allocation across the
    /// (large) unit-test suite in this file.
    static TEST_SCHEME: LazyLock<CapcoScheme> = LazyLock::new(CapcoScheme::new);

    #[test]
    fn decoder_is_send_sync_as_trait_object() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<DecoderRecognizer>();
        assert_send_sync::<StrictOrDecoderRecognizer>();
        assert_send_sync::<std::sync::Arc<dyn Recognizer<CapcoScheme>>>();
    }

    fn deep_cx() -> ParseContext {
        ParseContext {
            strict_evidence: false,
            preceded_by_whitespace: true,
            ..ParseContext::default()
        }
    }

    // ----- Missing-delimiter insertion (issue #133 PR 3) -----

    #[test]
    fn try_insert_delimiter_inserts_before_long_form_dissem() {
        // Hard-splitter rule: long-form dissem after whitespace.
        let cases: &[(&str, &str)] = &[
            ("SECRET//NOFORN EXDIS", "SECRET//NOFORN//EXDIS"),
            ("SECRET//NOFORN ORCON", "SECRET//NOFORN//ORCON"),
            ("SECRET//SI ORCON", "SECRET//SI//ORCON"),
        ];
        for (input, expected) in cases {
            let result = try_insert_delimiter(input);
            assert_eq!(
                result.as_deref(),
                Some(*expected),
                "input {input:?} should produce {expected:?}; got {result:?}"
            );
        }
    }

    #[test]
    fn try_insert_delimiter_classification_boundary() {
        // Rule 1: classification → next segment.
        let cases: &[(&str, &str)] = &[
            (
                "SECRET REL TO USA, AUS, GBR",
                "SECRET//REL TO USA, AUS, GBR",
            ),
            ("SECRET NOFORN", "SECRET//NOFORN"),
            ("TOP SECRET NOFORN", "TOP SECRET//NOFORN"),
        ];
        for (input, expected) in cases {
            let result = try_insert_delimiter(input);
            assert_eq!(
                result.as_deref(),
                Some(*expected),
                "input {input:?} should produce {expected:?}; got {result:?}"
            );
        }
    }

    #[test]
    fn try_insert_delimiter_does_not_split_top_secret() {
        // TOP SECRET is the only multi-word classification — the
        // helper must not insert `//` between TOP and SECRET.
        // The first rule fires only on the first NON-classification
        // token; SECRET after TOP is a classification continuation.
        let result = try_insert_delimiter("TOP SECRET//NF");
        // No insertion needed at all (input is already canonical).
        assert_eq!(result, None);
    }

    #[test]
    fn try_insert_delimiter_does_not_split_sbu_noforn() {
        // SBU NOFORN is the non-IC dissem banner long form for
        // SbuNf — must remain a single multi-word atom.
        let result = try_insert_delimiter("SECRET//SBU NOFORN");
        assert_eq!(result, None, "SBU NOFORN must not be split; got {result:?}");
    }

    #[test]
    fn try_insert_delimiter_does_not_split_les_noforn() {
        // LES NOFORN is the non-IC dissem banner long form for
        // LesNf — must remain a single multi-word atom.
        let result = try_insert_delimiter("SECRET//LES NOFORN");
        assert_eq!(result, None, "LES NOFORN must not be split; got {result:?}");
    }

    #[test]
    fn try_insert_delimiter_no_op_on_canonical() {
        // Already-canonical inputs produce None (no insertion).
        for input in &[
            "SECRET//NOFORN",
            "TOP SECRET//SI//NOFORN",
            "(S//NF)",
            "UNCLASSIFIED",
        ] {
            let result = try_insert_delimiter(input);
            assert_eq!(
                result, None,
                "input {input:?} is canonical; should produce None, got {result:?}"
            );
        }
    }

    #[test]
    fn try_insert_delimiter_capped_at_max_insertions() {
        // Pathological input with many splitters — the cap should
        // limit insertions. Hard cap is `MAX_DELIMITER_INSERTIONS`
        // (4 today); 6 splitters in the input should produce at
        // most 4 insertions in the output.
        let input = "SECRET NOFORN ORCON PROPIN IMCON RELIDO RSEN";
        let result = try_insert_delimiter(input);
        assert!(result.is_some());
        let inserted = result.unwrap();
        let inserted_count = inserted.matches("//").count();
        assert!(
            inserted_count <= MAX_DELIMITER_INSERTIONS,
            "must not exceed MAX_DELIMITER_INSERTIONS={MAX_DELIMITER_INSERTIONS}; \
             got {inserted_count} insertions in {inserted:?}"
        );
    }

    #[test]
    fn try_insert_delimiter_preserves_existing_double_slash() {
        // Existing `//` separators must be preserved verbatim.
        let result = try_insert_delimiter("SECRET//NOFORN EXDIS");
        let s = result.expect("should insert");
        // Two `//` total: one preserved in SECRET//NOFORN, plus one
        // inserted for NOFORN//EXDIS.
        let count = s.matches("//").count();
        assert_eq!(
            count, 2,
            "expected 2 `//` total (1 preserved + 1 inserted), got {count} in {s:?}"
        );
    }

    #[test]
    fn try_insert_delimiter_preserves_non_ascii_characters_verbatim() {
        // Regression guard for PR #175 review: the helper used to do
        // `result.push(bytes[i] as char)` for non-token, non-`/`,
        // non-whitespace characters, which corrupts multi-byte UTF-8
        // sequences by emitting each byte as a separate Latin-1
        // codepoint (e.g., `∕` → 3 garbage codepoints). The fix
        // walks `text[i..].chars()` to take one full character and
        // advances `i` by `ch.len_utf8()`, preserving the original
        // UTF-8 byte sequence in the output.
        //
        // The fixture below has a stray `∕` (U+2215, 3 bytes in
        // UTF-8) that the upstream delimiter normalizer didn't catch.
        // The helper must echo the original bytes verbatim into the
        // output (no insertion would happen here — there's no
        // splitter token after the `∕`), and the round-trip must
        // preserve the `∕` character intact.
        let input = "SECRET ∕∕ NOFORN";
        let result = try_insert_delimiter(input);
        // Whether or not the helper emits a result depends on the
        // tokenization — what matters is that NO character in the
        // output corrupts the `∕` UTF-8 sequence. Test the result
        // (or the input passthrough if None).
        let was_some = result.is_some();
        let s = result.unwrap_or_else(|| input.to_string());
        assert!(
            s.is_char_boundary(s.len()),
            "output {s:?} must end on a char boundary"
        );
        // The `∕` character (U+2215) must survive intact in the
        // output. If the old `bytes[i] as char` shape was still in
        // play, the 3-byte UTF-8 sequence [0xE2, 0x88, 0x95] would
        // be emitted as three separate codepoints (U+00E2 U+0088
        // U+0095), and the original `∕` would not appear.
        assert!(
            !was_some || s.contains('∕'),
            "output {s:?} must preserve the U+2215 character when the \
             helper emitted any output"
        );
    }

    #[test]
    fn is_hard_splitter_covers_documented_long_forms() {
        // Pin the hard-splitter set against accidental shrinkage —
        // every long-form dissem from the doc table must remain
        // a hard splitter.
        for token in &[
            "NOFORN",
            "ORCON",
            "ORCON-USGOV",
            "PROPIN",
            "IMCON",
            "RELIDO",
            "RSEN",
            "EYESONLY",
            "FOUO",
            "FISA",
            "DSEN",
            "EXDIS",
            "NODIS",
            "LIMDIS",
        ] {
            assert!(
                is_hard_splitter(token),
                "{token:?} must be a hard splitter (issue #133 PR 3)"
            );
        }
    }

    #[test]
    fn is_hard_splitter_excludes_short_forms() {
        // Short-form abbreviations (NF, OC, PR, IMC, RS) are
        // intentionally excluded — they could collide with SAR
        // compartment / sub-compartment naming.
        for token in &["NF", "OC", "PR", "IMC", "RS"] {
            assert!(
                !is_hard_splitter(token),
                "{token:?} is intentionally NOT a hard splitter (collision risk)"
            );
        }
    }

    // ----- Position-aware classification heuristic (issue #133 PR 2) -----

    #[test]
    fn heuristic_2char_ts_cluster() {
        // T-cluster + S-cluster → TS. Cover the full 6×5 = 30
        // combinations that should fire, plus a couple that shouldn't.
        for first in &['T', 'R', 'Y', 'H', 'G', 'F'] {
            for second in &['A', 'W', 'E', 'Z', 'S'] {
                let token: String = [*first, *second].iter().collect();
                assert_eq!(
                    try_2char_classification_heuristic(&token),
                    Some("TS"),
                    "{token:?} should heuristic-fix to TS"
                );
            }
        }
        // Lowercase variants normalize via the helper's
        // to_ascii_uppercase.
        assert_eq!(try_2char_classification_heuristic("ys"), Some("TS"));
        assert_eq!(try_2char_classification_heuristic("Ys"), Some("TS"));
    }

    #[test]
    fn heuristic_2char_no_match_outside_clusters() {
        // First char outside T-cluster → no match.
        for token in &["AS", "WS", "ZS", "BS", "DS", "QS"] {
            assert_eq!(
                try_2char_classification_heuristic(token),
                None,
                "{token:?} should not heuristic-fix"
            );
        }
        // Second char outside S-cluster → no match.
        for token in &["TR", "RY", "HG", "GH", "FB"] {
            assert_eq!(
                try_2char_classification_heuristic(token),
                None,
                "{token:?} should not heuristic-fix"
            );
        }
    }

    #[test]
    fn heuristic_1char_s_cluster() {
        // S-key neighbors → S. Bare S is canonical and excluded by
        // the upstream `is_canonical_short_classification` guard, so
        // the helper returns Some("S") for S-key neighbors and the
        // outer logic suppresses the no-op case.
        for token in &["A", "W", "E", "Z"] {
            assert_eq!(
                try_1char_classification_heuristic(token),
                Some("S"),
                "{token:?} should heuristic-fix to S"
            );
        }
        // X is between C and S; defaults to S per the design note.
        assert_eq!(try_1char_classification_heuristic("X"), Some("S"));
    }

    #[test]
    fn heuristic_1char_c_cluster() {
        // C-key neighbors → C.
        for token in &["V", "F"] {
            assert_eq!(
                try_1char_classification_heuristic(token),
                Some("C"),
                "{token:?} should heuristic-fix to C"
            );
        }
    }

    #[test]
    fn heuristic_1char_no_match_outside_clusters() {
        // Letters not in any heuristic cluster.
        for token in &["B", "D", "G", "K", "M", "N", "Q", "T", "Y"] {
            assert_eq!(
                try_1char_classification_heuristic(token),
                None,
                "{token:?} should not heuristic-fix"
            );
        }
    }

    #[test]
    fn heuristic_skips_canonical_classifications() {
        // Bare canonical short forms must not produce a heuristic
        // fix — the strict parser already accepts them.
        for canonical in &["U", "R", "C", "S", "TS"] {
            assert!(
                is_canonical_short_classification(canonical),
                "{canonical:?} should be recognized as canonical"
            );
        }
        // And the wrapper helper short-circuits these.
        assert_eq!(try_classification_heuristic_fix("(S//NF)"), None);
        assert_eq!(try_classification_heuristic_fix("(TS//NF)"), None);
        assert_eq!(try_classification_heuristic_fix("(C//NF)"), None);
        assert_eq!(try_classification_heuristic_fix("SECRET//NOFORN"), None);
    }

    #[test]
    fn heuristic_fixes_portion_form() {
        assert_eq!(
            try_classification_heuristic_fix("(YS//NF)").as_deref(),
            Some("(TS//NF)")
        );
        assert_eq!(
            try_classification_heuristic_fix("(W//NF)").as_deref(),
            Some("(S//NF)")
        );
        assert_eq!(
            try_classification_heuristic_fix("(F//NF)").as_deref(),
            Some("(C//NF)")
        );
        // Lowercase first token (inside parens).
        assert_eq!(
            try_classification_heuristic_fix("(ys//NF)").as_deref(),
            Some("(TS//NF)")
        );
    }

    #[test]
    fn heuristic_fixes_banner_form() {
        // Banner shapes don't have parens but otherwise behave the
        // same — leading classification token in the first segment.
        assert_eq!(
            try_classification_heuristic_fix("RS//NOFORN").as_deref(),
            Some("TS//NOFORN")
        );
        assert_eq!(
            try_classification_heuristic_fix("X//NOFORN").as_deref(),
            Some("S//NOFORN")
        );
    }

    #[test]
    fn heuristic_skips_cab_shape() {
        // CAB lines don't have a leading classification token. The
        // `is_cab_head` short-circuit at the top of the helper should
        // catch every CAB-keyword prefix.
        assert_eq!(try_classification_heuristic_fix("Classified By: foo"), None);
        assert_eq!(try_classification_heuristic_fix("Derived From: bar"), None);
        assert_eq!(try_classification_heuristic_fix("Declassify On: baz"), None);
    }

    #[test]
    fn heuristic_skips_long_token() {
        // 4+ char tokens fall through the length match arm — the
        // vocab fuzzy path handles them. 3-char tokens are mostly
        // handled by the vocab path too (now that PR 8 added bare
        // `TOP` to `EXTENDED_CORRECTION_VOCAB`, shapes like `TPP`
        // and `UOP` correct via dist-1 fuzzy); the 3-char heuristic
        // is intentionally narrow (only `OTP` → `TOP`) so unrelated
        // 3-char tokens like `YES` return None.
        assert_eq!(try_classification_heuristic_fix("(YES//NF)"), None);
        assert_eq!(try_classification_heuristic_fix("(SECT//NF)"), None);
        assert_eq!(try_classification_heuristic_fix("SECRET//NOFORN"), None);
    }

    // ----- 3-char classification heuristic (issue #133 PR 8) -----

    #[test]
    fn heuristic_recovers_otp_to_top_via_3char_rule() {
        // OTP → TOP: T↔O transposition. Standard Levenshtein dist 2
        // blocked by the vocab fuzzy path's `MIN_USEFUL_CONFIDENCE`
        // floor; the targeted 3-char heuristic is the recovery path.
        let cases: &[(&str, &str)] = &[
            ("OTP SECRET//NOFORN", "TOP SECRET//NOFORN"),
            ("(OTP//NF)", "(TOP//NF)"),
            ("OTP SECRET//SI//NOFORN", "TOP SECRET//SI//NOFORN"),
        ];
        for (input, expected) in cases {
            let result = try_classification_heuristic_fix(input);
            assert_eq!(
                result.as_deref(),
                Some(*expected),
                "input {input:?} should heuristic-fix to {expected:?}; got {result:?}"
            );
        }
    }

    #[test]
    fn try_3char_classification_heuristic_only_matches_otp() {
        // The 3-char heuristic is intentionally narrow (a single
        // hardcoded `OTP → TOP` mapping). Any other 3-char input
        // returns None and falls through to other recovery paths.
        // Pinned because the dense 3-char trigraph vocab (TON, TUR,
        // TWN, …) means a wider rule would generate too many false
        // positives.
        assert_eq!(try_3char_classification_heuristic("OTP"), Some("TOP"));
        for not_a_match in &["TON", "TPP", "UOP", "TIP", "TPO", "TOO", "ABC", "YES"] {
            assert_eq!(
                try_3char_classification_heuristic(not_a_match),
                None,
                "3-char heuristic must not fire on {not_a_match:?}",
            );
        }
    }

    // ----- Extended 2-char heuristic for TP/TO → TOP -----

    #[test]
    fn heuristic_recovers_tp_and_to_to_top_via_2char_rule() {
        // PR 8 extended the 2-char heuristic to map `TP`/`TO` → `TOP`.
        // These are corpus-attested classification typos where the
        // middle `O` (`TP`) or trailing `P` (`TO`) was elided. They
        // must not collide with the TS rule because neither `P` nor
        // `O` is in the S-cluster.
        let cases: &[(&str, &str)] = &[
            ("TP SECRET//NOFORN", "TOP SECRET//NOFORN"),
            ("TO SECRET//NOFORN", "TOP SECRET//NOFORN"),
            ("(TP//NF)", "(TOP//NF)"),
            ("(TO//NF)", "(TOP//NF)"),
        ];
        for (input, expected) in cases {
            let result = try_classification_heuristic_fix(input);
            assert_eq!(
                result.as_deref(),
                Some(*expected),
                "input {input:?} should heuristic-fix to {expected:?}; got {result:?}"
            );
        }
    }

    #[test]
    fn try_2char_classification_heuristic_ts_rule_takes_precedence() {
        // The TS rule (T-cluster + S-cluster pair) is checked first;
        // the TP/TO → TOP rule is a fallback. None of the TP/TO
        // characters are in the S-cluster (P, O), so there's no
        // ambiguity in practice — but pinning the precedence here
        // keeps a future widening of the TP/TO rule from silently
        // overriding the TS rule.
        // Pure T-cluster + S-cluster → TS.
        assert_eq!(try_2char_classification_heuristic("TS"), Some("TS"));
        assert_eq!(try_2char_classification_heuristic("RS"), Some("TS"));
        assert_eq!(try_2char_classification_heuristic("YS"), Some("TS"));
        // T + non-S-cluster → TOP (only for P/O).
        assert_eq!(try_2char_classification_heuristic("TP"), Some("TOP"));
        assert_eq!(try_2char_classification_heuristic("TO"), Some("TOP"));
        // T + other non-S-cluster → still None (don't broaden).
        assert_eq!(try_2char_classification_heuristic("TI"), None);
        assert_eq!(try_2char_classification_heuristic("TX"), None);
    }

    #[test]
    fn is_canonical_short_classification_recognizes_top() {
        // PR 8 added bare `TOP` to the canonical-short set so the
        // classification heuristic doesn't fire on already-canonical
        // `TOP SECRET//...` input (whose first whitespace-token is
        // `TOP`). Pre-PR-8 this was a no-op because the length-3
        // heuristic always returned None; PR 8's OTP rule made it
        // load-bearing.
        assert!(is_canonical_short_classification("TOP"));
        // Existing canonical short forms still recognized.
        for s in &["U", "R", "C", "S", "TS"] {
            assert!(
                is_canonical_short_classification(s),
                "{s:?} must be recognized as canonical short classification",
            );
        }
        // Non-canonical or wrong-case forms still return false.
        assert!(!is_canonical_short_classification("TPP"));
        assert!(!is_canonical_short_classification("top")); // case-sensitive
        assert!(!is_canonical_short_classification("TOPS"));
    }

    #[test]
    fn heuristic_skips_unknown_first_char() {
        // First char isn't in any heuristic cluster → no fix.
        assert_eq!(try_classification_heuristic_fix("(B//NF)"), None);
        assert_eq!(try_classification_heuristic_fix("(QS//NF)"), None);
    }

    #[test]
    fn heuristic_skips_lone_inputs() {
        // Issue #133 PR 4 / #176 lone-input safety guard. The
        // heuristic must NOT fire on inputs without marking-shape
        // signals beyond the leading token — auto-applying lone-case
        // fixes would surface as false positives on parenthetical
        // refs like `(A)`, `(W)`, `(F)` that are common in business
        // prose. The corpus measurement at PR 4 found `A` alone has
        // 214,539 unrestricted body-text occurrences in the Enron
        // corpus vs 168 in marking-context — the lone-case FP rate
        // is ~3 orders of magnitude higher than the in-context rate.
        //
        // Form-field input (caller asserts the input IS a marking
        // attempt) should still fire; tracking via #176 — when the
        // input-source signal lands, this guard becomes conditional.
        for lone in &[
            "(YS)",  // 2-char trigger, parens, nothing else
            "(W)",   // 1-char trigger
            "(F)",   // 1-char trigger
            "(X)",   // 1-char trigger
            "YS",    // banner-shape lone
            "W",     // bare lone token
            "(YS )", // trailing whitespace only
        ] {
            assert_eq!(
                try_classification_heuristic_fix(lone),
                None,
                "lone input {lone:?} must not fire heuristic (#133 PR 4 / #176 lone-input guard)"
            );
        }
    }

    #[test]
    fn heuristic_fires_when_marking_signal_present() {
        // Counterpart to `heuristic_skips_lone_inputs`. The guard is
        // about LONE inputs only; inputs with ANY marking content
        // beyond the leading token (a `//` separator OR another
        // whitespace-separated token in the first segment) still
        // fire normally.
        let cases: &[(&str, &str)] = &[
            ("(YS//NF)", "(TS//NF)"), // `//` separator after token
            ("(YS NF)", "(TS NF)"),   // whitespace + another token
            ("YS//NOFORN", "TS//NOFORN"),
            ("W//NF", "S//NF"),
        ];
        for (input, expected) in cases {
            let result = try_classification_heuristic_fix(input);
            assert_eq!(
                result.as_deref(),
                Some(*expected),
                "input {input:?} should heuristic-fix to {expected:?} \
                 (marking signal present); got {result:?}"
            );
        }
    }

    #[test]
    fn decoder_defers_to_strict_when_strict_evidence_is_set() {
        let rx = DecoderRecognizer::new();
        let cx = ParseContext::default(); // strict_evidence = true
        match rx.recognize(b"(S//NF)", 0, &*TEST_SCHEME, &cx) {
            Parsed::Ambiguous { candidates } => assert!(candidates.is_empty()),
            other => panic!("expected zero-candidate Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn fast_path_parses_simple_us_class_and_dissem_shape() {
        let canonical = try_fast_parse_us_class_and_dissem(MarkingType::Portion, b"(SECRET//NF)")
            .expect("canonical simple portion should hit decoder fast-path");
        assert_eq!(
            canonical.0.classification,
            Some(MarkingClassification::Us(Classification::Secret))
        );
        assert_eq!(canonical.0.dissem_us.as_ref(), &[DissemControl::Nf]);
        assert!(canonical.0.token_spans.is_empty());

        // Intentional typo: the fast-path preserves strict-parser behavior for
        // unknown classification tokens by keeping `classification = None`
        // while still retaining known dissem controls.
        let marking = try_fast_parse_us_class_and_dissem(MarkingType::Portion, b"(SERCET//NF)")
            .expect("simple portion should hit decoder fast-path");
        assert_eq!(marking.0.classification, None);
        assert_eq!(marking.0.dissem_us.as_ref(), &[DissemControl::Nf]);
        assert!(marking.0.token_spans.is_empty());
    }

    #[test]
    fn fast_path_rejects_complex_or_mixed_category_shapes() {
        assert!(
            try_fast_parse_us_class_and_dissem(MarkingType::Portion, b"(S//SI/NF)").is_none(),
            "mixed SCI/dissem slash block must fall back to full strict parser",
        );
        assert!(
            try_fast_parse_us_class_and_dissem(MarkingType::Portion, b"(S//REL TO USA, GBR)")
                .is_none(),
            "REL TO block must fall back to full strict parser",
        );
    }

    #[test]
    fn decoder_zero_candidate_on_no_template_fit() {
        let rx = DecoderRecognizer::new();
        // Neither token is in the vocabulary and no fuzzy match.
        match rx.recognize(b"FROBNITZ//WIBBLE", 0, &*TEST_SCHEME, &deep_cx()) {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "unrecognized input must be zero-candidate, got {} candidate(s)",
                candidates.len()
            ),
            Parsed::Unambiguous(m) => panic!("unexpected strict match: {m:?}"),
        }
    }

    #[test]
    fn score_candidate_splits_prior_and_posterior() {
        // Synthesize a fake attempt with known non-zero feature deltas
        // and verify the (prior, posterior) return tuple: posterior
        // must be prior + Σ feature.delta, and prior must NOT include
        // any of the feature deltas.
        let scheme = CapcoScheme::new();
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 14),
            kind: MarkingType::Banner,
        };
        let parsed = parser
            .parse(&candidate, b"SECRET//NOFORN")
            .expect("SECRET//NOFORN must parse");
        // PR 3c.2.B B3 (PM-B-1, PM-B-3): inline scheme construction
        // per test for hermeticity; routes via the trait override.
        let marking = CapcoMarking::new(scheme.canonicalize(parsed.attrs));

        let features = [
            FeatureEntry {
                id: FeatureId::EditDistance1,
                delta: -0.5,
            },
            FeatureId::TokenReorder.into(),
        ];
        let attempt = CanonicalAttempt {
            bytes: SmallVec::from_slice(b"SECRET//NOFORN"),
            features: features.iter().copied().collect(),
            fix_source: marque_rules::FixSource::DecoderPosterior,
        };
        let (prior, posterior) = score_candidate(&attempt, &marking, MarkingType::Banner);

        let feature_sum: f32 = features.iter().map(|f| f.delta).sum();
        let reconstructed = prior + feature_sum;
        assert!(
            (reconstructed - posterior).abs() < 1e-6,
            "posterior must equal prior + Σ feature deltas; \
             prior={prior}, feature_sum={feature_sum}, posterior={posterior}"
        );
        // And the prior alone must differ from the posterior when
        // the features carry non-trivial deltas.
        assert!(
            (prior - posterior).abs() > f32::EPSILON,
            "prior_log_odds must exclude feature deltas; \
             prior={prior}, posterior={posterior}"
        );
    }

    // Convenience conversion for the test above.
    impl From<FeatureId> for FeatureEntry {
        fn from(id: FeatureId) -> Self {
            Self { id, delta: -0.4 }
        }
    }

    #[test]
    fn score_candidate_includes_country_code_prior_for_rel_to() {
        // Issue #233: `score_candidate` sums `country_code_log_prior` over
        // the `rel_to` slice of the parsed marking. A marking with TWO REL TO
        // entries must produce a strictly lower (more negative) prior than the
        // same marking with ONE entry, because each country code contributes a
        // negative log-prior term and GBR is a known high-frequency trigraph.
        // PR 3c.2.B B3 (PM-B-1, PM-B-3): inline scheme construction
        // per test for hermeticity; both call sites route via the trait override.
        let scheme = CapcoScheme::new();
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);

        let one_candidate = MarkingCandidate {
            span: Span::new(0, 18),
            kind: MarkingType::Banner,
        };
        let one_parsed = parser
            .parse(&one_candidate, b"SECRET//REL TO USA")
            .expect("SECRET//REL TO USA must parse");
        let one_marking = CapcoMarking::new(scheme.canonicalize(one_parsed.attrs));

        let two_candidate = MarkingCandidate {
            span: Span::new(0, 23),
            kind: MarkingType::Banner,
        };
        let two_parsed = parser
            .parse(&two_candidate, b"SECRET//REL TO USA, GBR")
            .expect("SECRET//REL TO USA, GBR must parse");
        let two_marking = CapcoMarking::new(scheme.canonicalize(two_parsed.attrs));

        let attempt_one = CanonicalAttempt {
            bytes: SmallVec::from_slice(b"SECRET//REL TO USA"),
            features: SmallVec::new(),
            fix_source: marque_rules::FixSource::DecoderPosterior,
        };
        let attempt_two = CanonicalAttempt {
            bytes: SmallVec::from_slice(b"SECRET//REL TO USA, GBR"),
            features: SmallVec::new(),
            fix_source: marque_rules::FixSource::DecoderPosterior,
        };

        let (prior_one, _) = score_candidate(&attempt_one, &one_marking, MarkingType::Banner);
        let (prior_two, _) = score_candidate(&attempt_two, &two_marking, MarkingType::Banner);

        // GBR has a known negative log-prior, so adding it to the REL TO
        // list must make the total prior strictly more negative.
        assert!(
            prior_two < prior_one,
            "adding GBR to REL TO must lower (more negative) the prior via \
             country_code_log_prior; prior_one={prior_one}, prior_two={prior_two}"
        );
    }

    #[test]
    fn score_candidate_deduplicates_rel_to_entries() {
        // Issue #233 dedup guard: a duplicate REL TO entry (e.g. "USA, USA")
        // must score identically to the deduplicated form ("USA") because
        // `seen_rel_to_codes` prevents double-counting.
        // PR 3c.2.B B3 (PM-B-1, PM-B-3): inline scheme construction
        // per test for hermeticity; both call sites route via the trait override.
        let scheme = CapcoScheme::new();
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);

        let dup_candidate = MarkingCandidate {
            span: Span::new(0, 23),
            kind: MarkingType::Banner,
        };
        // Parser may or may not produce two rel_to entries for "USA, USA" —
        // the dedup guard must be robust either way: the prior must equal
        // that of a single "USA" entry.
        let dup_parsed = parser
            .parse(&dup_candidate, b"SECRET//REL TO USA, USA")
            .expect("SECRET//REL TO USA, USA must parse leniently");
        let dup_marking = CapcoMarking::new(scheme.canonicalize(dup_parsed.attrs));

        let once_candidate = MarkingCandidate {
            span: Span::new(0, 18),
            kind: MarkingType::Banner,
        };
        let once_parsed = parser
            .parse(&once_candidate, b"SECRET//REL TO USA")
            .expect("SECRET//REL TO USA must parse");
        let once_marking = CapcoMarking::new(scheme.canonicalize(once_parsed.attrs));

        let attempt_dup = CanonicalAttempt {
            bytes: SmallVec::from_slice(b"SECRET//REL TO USA, USA"),
            features: SmallVec::new(),
            fix_source: marque_rules::FixSource::DecoderPosterior,
        };
        let attempt_once = CanonicalAttempt {
            bytes: SmallVec::from_slice(b"SECRET//REL TO USA"),
            features: SmallVec::new(),
            fix_source: marque_rules::FixSource::DecoderPosterior,
        };

        let (prior_dup, _) = score_candidate(&attempt_dup, &dup_marking, MarkingType::Banner);
        let (prior_once, _) = score_candidate(&attempt_once, &once_marking, MarkingType::Banner);

        // Deduplication ensures the duplicate USA is only scored once, so
        // both priors must be equal (same base tokens + same single USA prior).
        assert!(
            (prior_dup - prior_once).abs() < 1e-5,
            "duplicate REL TO entry must not double-count the country-code prior; \
             prior_dup={prior_dup}, prior_once={prior_once}"
        );
    }

    #[test]
    fn feature_entry_to_evidence_uses_canonical_label_registry() {
        // Regression guard for PR #142 H2: the projection from
        // `FeatureEntry` onto `EvidenceFeature::label` MUST route
        // through `FeatureId::as_str()` — the single source of truth
        // declared in `crates/rules/src/confidence.rs:208`. A divergent
        // local registry (the pre-fix shape, snake_case labels in a
        // duplicate match arm) produces wire-format drift the audit
        // emitter cannot detect, because today's dispatcher discards
        // `Parsed::Ambiguous` results and the bug stays latent.
        //
        // This test exhaustively covers every `FeatureId` variant. A
        // new variant added without an `as_str()` arm fails compilation
        // there (the match is exhaustive); a new variant whose label
        // diverges from `as_str()` here would have to be deliberately
        // wrong, since this test reads `id.as_str()` directly. The
        // load-bearing assertion is that `feature_entry_to_evidence`
        // does the same thing.
        for id in [
            FeatureId::EditDistance1,
            FeatureId::EditDistance2,
            FeatureId::TokenReorder,
            FeatureId::SupersededToken,
            FeatureId::BaseRateCommonMarking,
            FeatureId::StrictContextClassification,
            FeatureId::CorpusOverrideInEffect,
        ] {
            let entry = FeatureEntry { id, delta: -0.5 };
            let evidence = feature_entry_to_evidence(&entry);
            assert_eq!(
                evidence.label,
                id.as_str(),
                "decoder evidence label diverged from FeatureId::as_str() \
                 for {id:?}: got {label:?}, expected {expected:?}",
                label = evidence.label,
                expected = id.as_str(),
            );
            assert_eq!(evidence.log_odds, -0.5);
        }
    }

    #[test]
    fn runner_up_ratio_saturates_on_extreme_log_margin() {
        // Regression guard for PR #127 review comment on decoder.rs:305:
        // when `log_margin` is large enough that `f32::exp()` overflows
        // (≈ ≥ 88.7 nats on f32), the previous code emitted `+∞` into
        // `Confidence::runner_up_ratio` and `Confidence::validate`
        // rejected the resulting record at the audit boundary,
        // panicking inside `FixProposal::new`. The fix saturates at
        // `f32::MAX`. We exercise both branches here with bare
        // `f32::exp` since the saturation logic is the same closed
        // expression used in `recognize`.
        for &log_margin in &[88.0_f32, 100.0_f32, 200.0_f32, 1000.0_f32] {
            let ratio = log_margin.exp();
            let clamped = if ratio.is_finite() { ratio } else { f32::MAX };
            assert!(
                clamped.is_finite(),
                "log_margin = {log_margin}: clamped ratio must be finite, got {clamped}"
            );
            assert!(
                clamped > 0.0,
                "log_margin = {log_margin}: clamped ratio must be > 0, got {clamped}"
            );
        }
        // And a sanity check on the in-band path: at the
        // UNAMBIGUOUS_LOG_MARGIN threshold, `exp()` returns a finite
        // value and we don't clamp.
        let at_threshold = UNAMBIGUOUS_LOG_MARGIN.exp();
        assert!(at_threshold.is_finite() && at_threshold > 1.0);
    }

    #[test]
    fn strict_parse_is_complete_rejects_unknown_classification() {
        // This is the regression-guard for PR #114 review comment
        // on decoder.rs:946 — strict parse of `(SERCET//NOFORN)`
        // recognizes NOFORN but leaves `classification: None` because
        // SERCET doesn't resolve to any `Classification` variant.
        // Without the `strict_parse_is_complete` check, the
        // dispatcher would accept this as a complete strict result
        // and never fall through to the decoder.
        // PR 3c.2.B B3 (PM-B-1, PM-B-3): inline scheme per test.
        let scheme = CapcoScheme::new();
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 16),
            kind: MarkingType::Portion,
        };
        let parsed = parser
            .parse(&candidate, b"(SERCET//NOFORN)")
            .expect("strict parser should accept (SERCET//NOFORN) leniently");
        let marking = CapcoMarking::new(scheme.canonicalize(parsed.attrs));
        assert!(
            is_nontrivial_marking(&marking),
            "NOFORN survives as a dissem control → marking is nontrivial"
        );
        assert!(
            !strict_parse_is_complete(&marking, MarkingType::Portion),
            "SERCET left `classification: None` → strict parse is incomplete; \
             dispatcher must fall back to decoder. attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn strict_parse_is_complete_accepts_clean_marking() {
        // PR 3c.2.B B3 (PM-B-1, PM-B-3): inline scheme per test.
        let scheme = CapcoScheme::new();
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 7),
            kind: MarkingType::Portion,
        };
        let parsed = parser
            .parse(&candidate, b"(S//NF)")
            .expect("canonical portion must strict-parse");
        let marking = CapcoMarking::new(scheme.canonicalize(parsed.attrs));
        assert!(
            strict_parse_is_complete(&marking, MarkingType::Portion),
            "canonical (S//NF) must be accepted as complete; attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn strict_parse_is_complete_rejects_trailing_unknown_token() {
        // `(S//FRBN)` — classification parses (`S` → Secret) but the
        // tail token `FRBN` lands in an `Unknown` span. The
        // dispatcher must fall back so the decoder can resolve
        // `FRBN` → `NF` (or reject).
        // PR 3c.2.B B3 (PM-B-1, PM-B-3): inline scheme per test.
        let scheme = CapcoScheme::new();
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 9),
            kind: MarkingType::Portion,
        };
        let parsed = parser
            .parse(&candidate, b"(S//FRBN)")
            .expect("strict parser accepts (S//FRBN) leniently");
        let marking = CapcoMarking::new(scheme.canonicalize(parsed.attrs));
        // `S` resolved, so classification is Some — but the
        // Unknown-tail check still fires.
        assert!(
            !strict_parse_is_complete(&marking, MarkingType::Portion),
            "`FRBN` is Unknown-kind → strict parse is incomplete; attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn contains_hard_splitter_word_detects_per_word() {
        // Whole-string match.
        assert!(contains_hard_splitter_word("NOFORN"));
        assert!(contains_hard_splitter_word("ORCON"));
        assert!(contains_hard_splitter_word("EXDIS"));
        // Per-word match (the `Full` SAR-program-nickname absorption
        // shape — `BUTTER POPCORN NOFORN`).
        assert!(contains_hard_splitter_word("BUTTER POPCORN NOFORN"));
        assert!(contains_hard_splitter_word("ORCON BUTTER POPCORN"));
        assert!(contains_hard_splitter_word("BUTTER NOFORN POPCORN"));
        // Negatives — clean SAR identifiers must not match.
        assert!(!contains_hard_splitter_word("BP"));
        assert!(!contains_hard_splitter_word("J12"));
        assert!(!contains_hard_splitter_word("XRA"));
        assert!(!contains_hard_splitter_word("BUTTER POPCORN"));
        assert!(!contains_hard_splitter_word(""));
    }

    #[test]
    fn absorbs_hard_splitter_detects_full_sar_program_with_trailing_noforn() {
        // The `SPECIAL ACCESS REQUIRED-BUTTER POPCORN NOFORN` shape:
        // strict parser builds a `Full`-indicator SAR with the program
        // identifier `"BUTTER POPCORN NOFORN"` (multi-word nickname,
        // NOFORN absorbed as the trailing word). Pinned to ensure the
        // per-word check in `contains_hard_splitter_word` keeps firing.
        use marque_ism::{CanonicalAttrs, SarIndicator, SarMarking, SarProgram};
        let sar = SarMarking::new(
            SarIndicator::Full,
            Box::new([SarProgram::new("BUTTER POPCORN NOFORN", Box::new([]))]),
        );
        let mut attrs = CanonicalAttrs::default();
        attrs.sar_markings = Some(sar);
        let marking = CapcoMarking::new(attrs);
        assert!(
            absorbs_hard_splitter_in_sar_or_sci(&marking),
            "NOFORN as trailing word of multi-word SAR program identifier must be detected"
        );
    }

    #[test]
    fn absorbs_hard_splitter_in_sar_detects_noforn_as_subcomp() {
        // Direct construction: a SAR program with NOFORN buried as a
        // sub-compartment of a normal compartment. Mirrors the parse
        // shape produced by `SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/
        // XR-XRA RB NOFORN` when the strict parser absorbs NOFORN at
        // the SAR-block tail.
        use marque_ism::{CanonicalAttrs, SarCompartment, SarIndicator, SarMarking, SarProgram};
        use smol_str::SmolStr;
        let sar = SarMarking::new(
            SarIndicator::Abbrev,
            Box::new([SarProgram::new(
                "BP",
                Box::new([SarCompartment::new(
                    "J12",
                    Box::new([SmolStr::from("RB"), SmolStr::from("NOFORN")]),
                )]),
            )]),
        );
        let mut attrs = CanonicalAttrs::default();
        attrs.sar_markings = Some(sar);
        let marking = CapcoMarking::new(attrs);
        assert!(
            absorbs_hard_splitter_in_sar_or_sci(&marking),
            "NOFORN as SAR sub-compartment must be detected as absorption"
        );
    }

    #[test]
    fn absorbs_hard_splitter_in_sar_detects_noforn_as_compartment_identifier() {
        // PR #178 review (Codecov, decoder.rs:1795): pin the
        // SAR-compartment-IDENTIFIER branch (vs the sub-compartment
        // branch covered above). Some absorbing parses end up with the
        // hard splitter as the compartment identifier itself rather
        // than a sub-compartment leaf — e.g., a `SAR-BP NOFORN` shape
        // where the strict parser emits `BP` as the program and
        // `NOFORN` as a bare compartment with no sub-compartments.
        use marque_ism::{CanonicalAttrs, SarCompartment, SarIndicator, SarMarking, SarProgram};
        let sar = SarMarking::new(
            SarIndicator::Abbrev,
            Box::new([SarProgram::new(
                "BP",
                Box::new([SarCompartment::new("NOFORN", Box::new([]))]),
            )]),
        );
        let mut attrs = CanonicalAttrs::default();
        attrs.sar_markings = Some(sar);
        let marking = CapcoMarking::new(attrs);
        assert!(
            absorbs_hard_splitter_in_sar_or_sci(&marking),
            "NOFORN as SAR compartment identifier must be detected as absorption"
        );
    }

    #[test]
    fn absorbs_hard_splitter_accepts_clean_sar() {
        // Negative case: a SAR with realistic identifiers (`BP`, `J12`,
        // `RB`) and no hard-splitter token anywhere. Must NOT trigger
        // the penalty.
        use marque_ism::{CanonicalAttrs, SarCompartment, SarIndicator, SarMarking, SarProgram};
        use smol_str::SmolStr;
        let sar = SarMarking::new(
            SarIndicator::Abbrev,
            Box::new([SarProgram::new(
                "BP",
                Box::new([SarCompartment::new(
                    "J12",
                    Box::new([SmolStr::from("RB"), SmolStr::from("XRA")]),
                )]),
            )]),
        );
        let mut attrs = CanonicalAttrs::default();
        attrs.sar_markings = Some(sar);
        let marking = CapcoMarking::new(attrs);
        assert!(
            !absorbs_hard_splitter_in_sar_or_sci(&marking),
            "clean SAR identifiers must not trigger the absorption penalty"
        );
    }

    #[test]
    fn absorbs_hard_splitter_in_sci_detects_orcon_as_subcomp() {
        // Defensive coverage for SCI absorption — the existing strict-
        // parser path drops most SCI absorption via the
        // `TokenKind::Unknown` filter in step 3a, but a future grammar
        // change that loosens SCI compartment shape could let a hard
        // splitter through. Pinned so the penalty stays defensive.
        use marque_ism::{
            CanonicalAttrs, SciCompartment, SciControlBare, SciControlSystem, SciMarking,
        };
        use smol_str::SmolStr;
        let sci = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([SciCompartment::new("G", Box::new([SmolStr::from("ORCON")]))]),
            None,
        );
        let mut attrs = CanonicalAttrs::default();
        attrs.sci_markings = Box::new([sci]);
        let marking = CapcoMarking::new(attrs);
        assert!(
            absorbs_hard_splitter_in_sar_or_sci(&marking),
            "ORCON as SCI sub-compartment must be detected as absorption"
        );
    }

    #[test]
    fn absorbs_hard_splitter_in_sci_detects_orcon_as_compartment_identifier() {
        // PR #178 review (Codecov, decoder.rs:1811): pin the SCI-
        // compartment-IDENTIFIER branch (vs the sub-compartment branch
        // above). Defensive coverage — today's strict-parser SCI path
        // drops most absorption via the `TokenKind::Unknown` filter at
        // step 3a, but a future grammar change that lets a hard
        // splitter through as the compartment id needs the penalty
        // active.
        use marque_ism::{
            CanonicalAttrs, SciCompartment, SciControlBare, SciControlSystem, SciMarking,
        };
        let sci = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([SciCompartment::new(Box::from("ORCON"), Box::new([]))]),
            None,
        );
        let mut attrs = CanonicalAttrs::default();
        attrs.sci_markings = Box::new([sci]);
        let marking = CapcoMarking::new(attrs);
        assert!(
            absorbs_hard_splitter_in_sar_or_sci(&marking),
            "ORCON as SCI compartment identifier must be detected as absorption"
        );
    }

    #[test]
    fn absorbs_hard_splitter_negative_on_empty_marking() {
        // Sanity floor: a marking with neither SAR nor SCI never
        // triggers the penalty.
        use marque_ism::CanonicalAttrs;
        let attrs = CanonicalAttrs::default();
        let marking = CapcoMarking::new(attrs);
        assert!(
            !absorbs_hard_splitter_in_sar_or_sci(&marking),
            "marking without SAR/SCI must not trigger the penalty"
        );
    }

    #[test]
    fn decoder_resolves_sar_with_trailing_noforn_via_absorption_penalty() {
        // The SC-004 fixtures `SAR-BP-J12 …` and
        // `SPECIAL ACCESS REQUIRED-BUTTER POPCORN …` with a trailing
        // NOFORN have always produced the right candidate bytes from
        // `try_insert_delimiter`, but lost the scoring contest before
        // PR-5 because the absorbing strict parse contributed only the
        // classification's prior while the delim-inserted parse paid
        // the additional log-prior of NF. The
        // `HARD_SPLITTER_ABSORPTION_PENALTY` flips the contest; this
        // test pins both fixture shapes.
        let rx = DecoderRecognizer::new();
        for input in &[
            "TOP SECRET//SPECIAL ACCESS REQUIRED-BUTTER POPCORN NOFORN",
            "SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB NOFORN",
        ] {
            let parsed = rx.recognize(input.as_bytes(), 0, &*TEST_SCHEME, &deep_cx());
            match parsed {
                Parsed::Unambiguous(m) => {
                    assert!(
                        m.0.sar_markings.is_some(),
                        "input {input:?}: expected SAR present in winning candidate"
                    );
                    // PR #178 review (Copilot, decoder.rs:2841): assert
                    // the SPECIFIC dissem control we expect — `Nf`.
                    // The previous `!is_empty()` check would silently
                    // accept a future regression that emitted a
                    // different dissem token (e.g., a misclassified
                    // `Oc`/`Pr`) and still call the test green.
                    assert!(
                        m.0.dissem_iter()
                            .any(|d| matches!(d, marque_ism::DissemControl::Nf)),
                        "input {input:?}: expected NOFORN (DissemControl::Nf) to land \
                         as a dissem control (winning candidate must be the delim-\
                         inserted form, not the absorbing one); got dissem_us = \
                         {:?}, dissem_nato = {:?}",
                        m.0.dissem_us,
                        m.0.dissem_nato,
                    );
                    assert!(
                        !absorbs_hard_splitter_in_sar_or_sci(&m),
                        "input {input:?}: winning marking must not bury a hard \
                         splitter inside SAR/SCI"
                    );
                }
                other => panic!("input {input:?}: expected Unambiguous, got {other:?}"),
            }
        }
    }

    #[test]
    fn decoder_rejects_trivial_strict_parse() {
        // The strict parser is lenient: it accepts `FROBNITZ//WIBBLE`
        // and emits an CanonicalAttrs with classification=None,
        // dissem_controls=[], sci_controls=[]. The decoder must treat
        // that as "no real parse" and drop the candidate — otherwise
        // it would fabricate an empty marking for arbitrary prose.
        // PR 3c.2.B B3 (PM-B-1, PM-B-3): inline scheme per test.
        let scheme = CapcoScheme::new();
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 16),
            kind: MarkingType::Banner,
        };
        let parsed = parser
            .parse(&candidate, b"FROBNITZ//WIBBLE")
            .expect("strict parser should accept arbitrary bytes");
        let marking = CapcoMarking::new(scheme.canonicalize(parsed.attrs));
        assert!(
            !is_nontrivial_marking(&marking),
            "empty marking must be filtered"
        );
    }

    #[test]
    fn decoder_recovers_typo_sercet_to_secret() {
        let rx = DecoderRecognizer::new();
        match rx.recognize(b"SERCET//NOFORN", 0, &*TEST_SCHEME, &deep_cx()) {
            Parsed::Unambiguous(m) => {
                assert_eq!(
                    marking_classification(&m),
                    Some(Classification::Secret),
                    "expected SECRET classification from SERCET fuzzy-correction"
                );
            }
            other => panic!("expected Unambiguous(SECRET//NOFORN), got {other:?}"),
        }
    }

    #[test]
    fn decoder_recovers_case_mangled_input() {
        let rx = DecoderRecognizer::new();
        match rx.recognize(b"secret//noforn", 0, &*TEST_SCHEME, &deep_cx()) {
            Parsed::Unambiguous(m) => {
                assert_eq!(marking_classification(&m), Some(Classification::Secret));
            }
            other => panic!("expected Unambiguous, got {other:?}"),
        }
    }

    #[test]
    fn decoder_suppresses_prose_glue_single_letter_portion() {
        // Prose-glue heuristic: when the byte preceding the candidate
        // is NOT whitespace, a single-letter `(s)` / `(c)` is
        // overwhelmingly a plural-suffix (`letter(s)`) or function-
        // call glyph (`function(c)`). The decoder must produce zero
        // candidates so the engine doesn't synthesize a spurious R001
        // diagnostic.
        let rx = DecoderRecognizer::new();
        let glued = ParseContext {
            preceded_by_whitespace: false,
            ..deep_cx()
        };
        for input in &[b"(s)", b"(c)", b"(u)", b"(S)", b"(C)"] {
            match rx.recognize(*input, 0, &*TEST_SCHEME, &glued) {
                Parsed::Ambiguous { candidates } => assert!(
                    candidates.is_empty(),
                    "{:?} glued to a word must produce zero candidates, got {}",
                    std::str::from_utf8(*input).unwrap_or("<bytes>"),
                    candidates.len(),
                ),
                Parsed::Unambiguous(_) => panic!(
                    "{:?} glued to a word must not resolve",
                    std::str::from_utf8(*input).unwrap_or("<bytes>"),
                ),
            }
        }
    }

    #[test]
    fn decoder_prose_glue_suppresses_u_that_null_gate_would_admit() {
        // HIGH 1 (review) — pins the independence of the prose-glue
        // early-return from the post-#472 null gate.
        //
        // The `U`-token marking-y delta is `+2.86`, which exceeds the
        // [`NULL_HYPOTHESIS_LOG_MARGIN`] (`+2.5`) — an isolated `(u)`
        // with `preceded_by_whitespace = true` clears the null gate
        // and recovers to UNCLASSIFIED (the
        // `decoder_residual_gap_isolated_u_recovers_to_unclassified`
        // test pins that recovery).
        //
        // This test pins the symmetric case: the SAME `(u)` with
        // `preceded_by_whitespace = false` (e.g., `function(u)`,
        // `sec(u)rity`) must be suppressed. Because the null gate
        // alone would admit it, the prose-glue early-return is
        // independently load-bearing here. Removing the early-return
        // (e.g., on the assumption that the null gate now subsumes
        // it) would silently regress this case.
        let rx = DecoderRecognizer::new();

        // Baseline: not glued, null gate admits, recovers to
        // UNCLASSIFIED.
        let standalone = rx.recognize(b"(u)", 0, &*TEST_SCHEME, &deep_cx());
        assert!(
            matches!(
                &standalone,
                Parsed::Unambiguous(m)
                    if m.0.classification
                        == Some(MarkingClassification::Us(Classification::Unclassified))
            ),
            "standalone `(u)` must recover to UNCLASSIFIED via the \
             null gate's +2.86 marking-y delta exceeding the +2.5 \
             margin; got {standalone:?}",
        );

        // Glued: same input, `preceded_by_whitespace = false`. The
        // prose-glue early-return suppresses BEFORE the null gate.
        let glued_cx = ParseContext {
            preceded_by_whitespace: false,
            ..deep_cx()
        };
        let glued = rx.recognize(b"(u)", 0, &*TEST_SCHEME, &glued_cx);
        match glued {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "glued `(u)` (preceded_by_whitespace=false) must be \
                 zero-candidate via the prose-glue early-return; got \
                 {} candidate(s)",
                candidates.len(),
            ),
            Parsed::Unambiguous(m) => panic!(
                "glued `(u)` must be suppressed by the prose-glue \
                 early-return — the post-#472 null gate alone admits \
                 this case ({:+.2} delta exceeds {:+.2} margin), so \
                 prose-glue removal would silently regress it. Got \
                 Unambiguous({:?})",
                2.86_f32, NULL_HYPOTHESIS_LOG_MARGIN, m.0.classification,
            ),
        }
    }

    #[test]
    fn decoder_suppresses_single_letter_portion_via_null_hypothesis() {
        // Issue #258 + PR1 (documents-corpus marking stratum): an
        // isolated `(s)` (preceded by whitespace, so the prose-glue
        // heuristic is bypassed) is the prose null-hypothesis case.
        // The decoder must produce zero candidates so the engine
        // doesn't synthesize a spurious R001 diagnostic.
        //
        // Before PR1: the marking-side prior for `S` was the Laplace
        // floor (zero hits in `tests/corpus/valid/`) so the per-token
        // marking-y delta `log P("S"|marking) − log P("S"|prose)` was
        // negative — the null hypothesis won under the original
        // `posterior >= null_posterior` filter.
        //
        // After PR1: `tests/corpus/documents/marked/` contributes 173
        // hits for `S`, pushing the marking-side delta to `+2.21`
        // (`S`: marking `-3.28`, prose `-5.49`). A zero-margin
        // filter would let the marking hypothesis win and
        // re-introduce the SC-003a Federalist `(s)` regression. The
        // `NULL_HYPOTHESIS_LOG_MARGIN = 2.5` floor (see constant
        // doc) was tuned to keep `(s)` suppressed at +2.21 while
        // still admitting multi-token candidates whose delta is
        // many times larger.
        //
        // This is the exact behavior that closes the SC-003a
        // regression on `Notwithstanding (s) the early prevalence` —
        // the decoder doesn't auto-fix prose-shaped single-letter
        // portions to a SECRET portion.
        let rx = DecoderRecognizer::new();
        match rx.recognize(b"(s)", 0, &*TEST_SCHEME, &deep_cx()) {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "isolated lowercase (s) must be zero-candidate (null wins), \
                 got {} candidate(s)",
                candidates.len()
            ),
            Parsed::Unambiguous(m) => panic!(
                "isolated lowercase (s) must be suppressed by the prose null \
                 hypothesis, got Unambiguous({:?})",
                m.0.classification,
            ),
        }
    }

    #[test]
    fn is_bare_classification_shape_recognizes_whitelist() {
        // Issue #472: the 10-entry closed whitelist covers every
        // canonical CAPCO portion classification token. All entries
        // must match byte-exact.
        for s in &[
            b"(U)" as &[u8],
            b"(C)",
            b"(S)",
            b"(TS)",
            b"(R)",
            b"(NU)",
            b"(NR)",
            b"(NC)",
            b"(NS)",
            b"(CTS)",
        ] {
            assert!(
                is_bare_classification_shape(s),
                "whitelist entry {:?} must match",
                std::str::from_utf8(s).unwrap_or("<bytes>"),
            );
        }

        // Non-whitelist 3-letter all-caps acronyms — the prose-acronym
        // false-positive surface the gate is designed to suppress.
        for s in &[
            b"(CMS)" as &[u8],
            b"(MD)",
            b"(SI)",
            b"(CTs)", // mixed case — case-fold happens later, the gate runs on raw bytes
            b"(c)",   // lowercase fails the byte-exact match
            b"(s)",
            b"(u)",
            b"(C//NF)", // has `//`
            b"( C )",   // interior whitespace fails byte-exact
            b"(CT)",    // not on the canonical token set
        ] {
            assert!(
                !is_bare_classification_shape(s),
                "non-whitelist input {:?} must not match",
                std::str::from_utf8(s).unwrap_or("<bytes>"),
            );
        }
    }

    #[test]
    fn is_bare_classification_shape_is_byte_exact() {
        // Interior whitespace inside the parens (`( C )`, `(C )`,
        // `( C)`) does not match — that's intentional. Whitespace
        // tolerance happens elsewhere (the strict recognizer strips
        // leading whitespace on portion candidates), but this gate
        // operates on the raw observed bytes. Any whitespace-bearing
        // shape goes through the null-hypothesis filter so a
        // prose-shaped `( C )` mid-prose is correctly tested against
        // the observed prose prior.
        assert!(!is_bare_classification_shape(b"( C)"));
        assert!(!is_bare_classification_shape(b"(C )"));
        assert!(!is_bare_classification_shape(b"( C )"));
        assert!(!is_bare_classification_shape(b" (C)"));
        assert!(!is_bare_classification_shape(b"(C) "));
    }

    #[test]
    fn has_double_slash_detects_slash_slash() {
        // True cases: any input containing `//` anywhere.
        assert!(has_double_slash(b"(S//NF)"));
        assert!(has_double_slash(b"S//REL"));
        assert!(has_double_slash(b"//"));
        assert!(has_double_slash(b"prefix//suffix"));
        assert!(has_double_slash(b"SECRET//NOFORN"));

        // False cases: no `//` sequence.
        assert!(!has_double_slash(b"/"));
        assert!(!has_double_slash(b"(S)"));
        assert!(!has_double_slash(b"(S/NF)"));
        assert!(!has_double_slash(b""));
        assert!(!has_double_slash(b"/foo/bar/"));
    }

    #[test]
    fn observed_prose_log_prior_reflects_observed_not_canonical() {
        // Issue #472: the observed-side prior is summed over the
        // *bytes the user typed*, not over the canonical tokens the
        // fuzzy-corrector chose. Verify that an input whose
        // observed-token bag differs from a hypothetical canonical
        // bag receives a different prose-prior sum.
        //
        // `(CMS)` is not in any priors table → falls to
        // [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`] (`-7.0`).
        // `(CTS)` IS in the prose priors table (canonical token,
        // Laplace-smoothed entry materialized by `derive_priors`).
        //
        // The two values must differ — if they were equal, the
        // observed-vs-canonical asymmetry the gate depends on would
        // have collapsed, and the issue #472 fix would be a no-op.
        let observed_cms = observed_prose_log_prior(b"(CMS)");
        let observed_cts = observed_prose_log_prior(b"(CTS)");
        assert!(
            (observed_cms - OBSERVED_UNKNOWN_PROSE_LOG_PRIOR).abs() < 1e-5,
            "observed `(CMS)` must fall to OBSERVED_UNKNOWN_PROSE_LOG_PRIOR; \
             got {observed_cms}",
        );
        // CTS is in the prose table; the lookup must succeed.
        let cts_table = marque_capco::priors::token_prose_log_prior("CTS")
            .expect("CTS must be in token_prose_base_rates");
        assert!(
            (observed_cts - cts_table).abs() < 1e-5,
            "observed `(CTS)` must equal token_prose_log_prior(\"CTS\"); \
             got {observed_cts}, expected {cts_table}",
        );
        // The exact magnitude depends on the in-table value for `CTS`
        // (Laplace-smoothed prose count) vs the constant
        // [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`]; the two are calibrated
        // to live in adjacent regions of the log-prior range, so a
        // sub-1.0-nat difference is acceptable. The test pins
        // "different by more than floating-point noise" — the actual
        // numerical distance documents the calibration gap rather
        // than gates it.
        assert!(
            (observed_cms - observed_cts).abs() > 0.1,
            "observed prose prior for (CMS) must differ from (CTS) — the \
             observed-vs-canonical asymmetry is the issue #472 fix's \
             mechanism; got {observed_cms} vs {observed_cts}",
        );
    }

    #[test]
    fn observed_prose_log_prior_dedupes_repeated_tokens() {
        // The sum runs over *distinct* observed tokens; a duplicate
        // contributes once. Without dedup an input like `(USA, USA)`
        // would double-count USA's prose prior, which would
        // (incorrectly) push the null hypothesis arbitrarily low for
        // repeated tokens.
        let dup = observed_prose_log_prior(b"(USA, USA)");
        let once = observed_prose_log_prior(b"(USA)");
        assert!(
            (dup - once).abs() < 1e-5,
            "duplicate observed tokens must not double-count; \
             dup={dup}, once={once}",
        );
    }

    #[test]
    fn observed_prose_log_prior_handles_empty_and_separator_only() {
        // Defensive: empty input, separator-only input, and bare
        // delimiters must produce zero (no observed tokens to sum).
        assert_eq!(observed_prose_log_prior(b""), 0.0);
        assert_eq!(observed_prose_log_prior(b"()"), 0.0);
        assert_eq!(observed_prose_log_prior(b"//"), 0.0);
        assert_eq!(observed_prose_log_prior(b" , -/"), 0.0);
    }

    #[test]
    fn decoder_admits_mangled_marking_under_observed_null_gate() {
        // Copilot #3 follow-up: must-not-over-suppress stress test
        // for the [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`] = -7.0 floor.
        //
        // The constant's doc-comment names `(CMS)` / SC-003a as the
        // must-suppress side: prose acronyms with no marking
        // vocabulary support should fall below
        // [`NULL_HYPOTHESIS_LOG_MARGIN`] and be suppressed.
        // This test pins the symmetric must-NOT-over-suppress side:
        // a genuinely mangled-but-recoverable marking
        // (`(SERCET//NF)`, edit-distance-1 typo of `SECRET`) must
        // clear the same gate and reach `Parsed::Unambiguous`. If a
        // future calibration change tightens the floor too far —
        // raising it to where multi-token mangled markings get
        // swept up — this test fails.
        //
        // `(SERCET//NF)` chosen because:
        // 1. Already named in the [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`]
        //    constant doc as the example case the floor was sized to
        //    admit ("legitimate single-token mangled-marking
        //    recoveries (`(SERCET//NF)`) stay above it").
        // 2. Has `//` so [`has_double_slash`] bypasses the null gate
        //    entirely — this test pins the bypass + scoring path,
        //    not just the gate threshold. A regression that broke
        //    `has_double_slash` would surface as this candidate
        //    failing recovery, with the gate threshold a secondary
        //    suspect.
        // 3. Strong marking-side prior (SECRET + NOFORN both in
        //    `token_base_rates`) producing a high posterior so the
        //    runner-up ratio and resulting confidence sit well
        //    above the default `confidence_threshold = 0.95`.
        let rx = DecoderRecognizer::new();
        match rx.recognize(b"(SERCET//NF)", 0, &*TEST_SCHEME, &deep_cx()) {
            Parsed::Unambiguous(m) => {
                // The strict parse on the canonicalized bytes must
                // yield `Us(Secret)`.
                assert_eq!(
                    m.0.classification,
                    Some(MarkingClassification::Us(Classification::Secret)),
                    "(SERCET//NF) must recover to Us(Secret); got {:?}",
                    m.0.classification,
                );
                // Provenance must carry an EditDistance feature —
                // confirms the fuzzy-correction path was exercised
                // (SERCET → SECRET, Levenshtein 2: R↔C transpose
                // requires two substitutions). EditDistance1 OR
                // EditDistance2 both indicate the fuzzy path
                // produced the canonical form.
                let prov =
                    m.1.as_ref()
                        .expect("decoder-path recovery must carry DecoderProvenance");
                let has_edit_distance = prov
                    .features
                    .iter()
                    .any(|f| matches!(f.id, FeatureId::EditDistance1 | FeatureId::EditDistance2));
                assert!(
                    has_edit_distance,
                    "(SERCET//NF) recovery must record an EditDistance \
                     feature in provenance (SERCET → SECRET); got {:?}",
                    prov.features,
                );
            }
            Parsed::Ambiguous { candidates } => panic!(
                "(SERCET//NF) is the canonical must-not-over-suppress \
                 case named in the OBSERVED_UNKNOWN_PROSE_LOG_PRIOR \
                 constant doc — recovery must succeed. If this fails, \
                 audit (a) whether `has_double_slash` still bypasses \
                 the null gate for `//`-bearing inputs, (b) whether \
                 the `-7.0` floor or `+2.5` margin was tightened, or \
                 (c) whether SECRET / NOFORN dropped out of \
                 `token_base_rates`. Got Ambiguous with {} candidate(s).",
                candidates.len(),
            ),
        }
    }

    #[test]
    fn decoder_residual_gap_isolated_u_recovers_to_unclassified() {
        // KNOWN RESIDUAL GAP — pinning current behavior.
        //
        // `(u)` has a `+2.86` marking-vs-prose delta on the `U`
        // token (see the NULL_HYPOTHESIS_LOG_MARGIN constant doc on
        // line ~152). That delta exceeds the `+2.5` null filter
        // margin, so an isolated `(u)` recovers to UNCLASSIFIED
        // when no context features fire (test-default
        // `ParseContext` carries `line_offset: None`,
        // `line_prefix: None`, `surrounding_is_lowercase: false`).
        //
        // The Task 10 `LowercaseSurroundingContext` feature
        // (`-2.0`) suppresses the common mid-prose `(u)` case in
        // lowercase-dominant context (decoder.rs::
        // decoder_applies_lowercase_context_penalty_in_lowercase_prose
        // pins that). The residual surface is `(u)` at column 0
        // in mixed-case or uppercase context — vanishingly rare in
        // real IC text, but not zero.
        //
        // This test pins the current behavior so a future
        // regression (drift in token priors, threshold tuning, a
        // new feature) is loud. Closing the gap further likely
        // requires a third signal (document-level archival mode,
        // page zone, etc.) and is deferred — see PR description
        // "Deferred (separate work)".
        let rx = DecoderRecognizer::new();
        match rx.recognize(b"(u)", 0, &*TEST_SCHEME, &deep_cx()) {
            Parsed::Unambiguous(m) => {
                assert_eq!(
                    m.0.classification,
                    Some(MarkingClassification::Us(Classification::Unclassified)),
                    "isolated `(u)` at default ParseContext currently \
                     resolves to UNCLASSIFIED (documented residual gap, \
                     +2.86 marking-vs-prose delta exceeds +2.5 margin)",
                );
            }
            Parsed::Ambiguous { candidates } => {
                panic!(
                    "isolated `(u)` was expected to recover to UNCLASSIFIED \
                     under the pinned residual gap; got Ambiguous with {} \
                     candidate(s). If the decoder behavior tightened, this \
                     test should be inverted to assert zero candidates and \
                     the residual-gap doc rationale on \
                     NULL_HYPOTHESIS_LOG_MARGIN updated to reflect the new \
                     behavior.",
                    candidates.len(),
                );
            }
        }
    }

    // ----- Context features (Task 9 + Task 10) -----

    #[test]
    fn looks_like_bullet_anchor_recognizes_common_forms() {
        // Numeric/alphanumeric bullets with dot/paren terminator.
        for prefix in &[
            b"1. " as &[u8],
            b"12. ",
            b"123. ",
            b"1) ",
            b"1.2.3.",
            b"1B.a.3.",
            b"a. ",
            b"a) ",
            b"b) ",
            b"(a) ",
            b"(b) ",
            b"[a] ",
            b"(i) ",   // single-letter Roman (also single alpha)
            b"(ii) ",  // 2-char alpha — passes bare-alpha cap
            b"(iii) ", // 3-char alpha — passes only inside brackets
            b"(iv) ",
            b"   * ", // indented bullet, leading whitespace trimmed
            b"\t- ",  // tab-indented bullet
        ] {
            assert!(
                looks_like_bullet_anchor(prefix),
                "expected bullet-anchor: {:?}",
                std::str::from_utf8(prefix).unwrap_or("<bytes>"),
            );
        }
        // Single-character bullet glyphs.
        for prefix in &[b"* " as &[u8], b"- ", b"*", b"-"] {
            assert!(
                looks_like_bullet_anchor(prefix),
                "expected bullet-anchor: {:?}",
                std::str::from_utf8(prefix).unwrap_or("<bytes>"),
            );
        }
        // Unicode `•` (U+2022) bullet — three-byte UTF-8 sequence.
        assert!(
            looks_like_bullet_anchor(b"\xE2\x80\xA2 "),
            "expected Unicode `•` to be recognized as a bullet anchor",
        );
    }

    #[test]
    fn looks_like_bullet_anchor_rejects_running_prose() {
        // Plain prose endings — punctuation that doesn't anchor an
        // enumeration, or word characters with no terminator.
        for prefix in &[
            b"Notwithstanding " as &[u8],
            b"the early prevalence of ",
            b"function",
            b"loss",
            b"He said: ",
            b"this, ",
            b". ",  // bare period — no anchor content before it
            b"() ", // empty bracket pair, no alphanumeric body
        ] {
            assert!(
                !looks_like_bullet_anchor(prefix),
                "expected NOT a bullet-anchor: {:?}",
                std::str::from_utf8(prefix).unwrap_or("<bytes>"),
            );
        }
    }

    #[test]
    fn looks_like_bullet_anchor_rejects_short_alpha_words_ending_period() {
        // The substantive false-positive class the bullet-anchor
        // recognizer must reject: bare 3-letter English words
        // ending in a period immediately before a portion-shaped
        // glyph. Pre-fix, `the.`, `for.`, `its.` would have been
        // treated as enumeration anchors (alphanumeric run of 3,
        // dot terminator) and triggered BulletAnchorBonus on prose
        // text. The `ANCHOR_ALPHA_BARE_RUN_MAX = 2` constraint
        // rejects them while still accepting `(iii)` inside parens.
        for prefix in &[
            b"the." as &[u8],
            b"the. ",
            b"for.",
            b"its.",
            b"and.",
            b"but.",
        ] {
            assert!(
                !looks_like_bullet_anchor(prefix),
                "3-char alpha word ending in period must NOT be \
                 treated as a bullet anchor: {:?}",
                std::str::from_utf8(prefix).unwrap_or("<bytes>"),
            );
        }
        // Roman-numeral unwrapped variant — accepted false-negative
        // per the design rationale: legal/IC documents overwhelmingly
        // use parenthesized form `(iii)`. The reject here is the
        // necessary cost of suppressing the `the.` / `for.` class.
        assert!(
            !looks_like_bullet_anchor(b"iii."),
            "bare unwrapped Roman numeral is rejected (design \
             trade-off: parens-wrapped `(iii)` is supported instead)",
        );
    }

    #[test]
    fn looks_like_bullet_anchor_rejects_prose_abbreviations() {
        // Latin abbreviations like `e.g.` and `i.e.` and prose
        // tails ending in stray closing punctuation like `e.g)`,
        // `i.e]` have dotted internal structure but no digit and
        // no opening bracket — pre-fix they passed the alpha-cap
        // gate (`e`, `.`, `g` — each run is 1 alpha) and were
        // treated as enumeration anchors, swinging a portion-shaped
        // glyph after them by +3.5 log-odds. The fix requires a
        // separator-bearing body to contain a digit OR an opening
        // bracket.
        for prefix in &[
            b"e.g." as &[u8],
            b"i.e.",
            b"e.g) ",
            b"i.e]",
            b"i.e. ",
            b"vs.", // 2-letter abbrev + dot, alpha-cap rejects
            b"etc.",
        ] {
            assert!(
                !looks_like_bullet_anchor(prefix),
                "prose abbreviation must NOT be treated as a bullet \
                 anchor: {:?}",
                std::str::from_utf8(prefix).unwrap_or("<bytes>"),
            );
        }
    }

    #[test]
    fn looks_like_bullet_anchor_rejects_hyphenated_word_fragments() {
        // Hyphenated word fragments like `re-`, `un-`, `co-` (3-byte
        // tails ending in ASCII dash) must NOT be treated as bullet
        // glyphs. Pre-fix, the single-char-bullet rule used
        // `trimmed.len() <= 3` which accepted these. The fix
        // tightens to exact-match (`b"-"`, `b"*"`, Unicode bullet).
        for prefix in &[b"re-" as &[u8], b"un-", b"co-", b"a-", b"non-"] {
            assert!(
                !looks_like_bullet_anchor(prefix),
                "hyphenated word fragment must NOT be treated as a \
                 bullet glyph: {:?}",
                std::str::from_utf8(prefix).unwrap_or("<bytes>"),
            );
        }
    }

    #[test]
    fn decoder_applies_line_position_penalty_for_mid_line_portion() {
        // Issue #472: `(C)` is on the
        // [`is_bare_classification_shape`] whitelist — its byte form
        // IS the canonical grammar for CONFIDENTIAL, so the
        // null-hypothesis filter intentionally does NOT suppress it
        // at the decoder layer. The decoder returns
        // `Unambiguous(Us(Confidential))` and records the
        // `LinePositionPenalty` feature on the candidate's posterior.
        // Engine-layer no-op-rewrite filtering (the original bytes
        // equal the canonical bytes, so `build_decoder_diagnostic`
        // returns `None`) eats the synthetic R001 in production, so
        // the false-positive surface stays closed end-to-end; this
        // test pins the decoder-internal observation that the
        // position penalty was computed.
        //
        // `(C)` mid-prose canonicalizing to CONFIDENTIAL when the
        // observed bytes already match the canonical form is the
        // tracked #511 layered-confidence territory, not a #472
        // regression — the bypass exists because there is no other
        // grammar shape for that classification level, and the
        // remaining false-positive surface (when canonicalization
        // would change bytes — e.g., `(c)` → `(C)`) is handled by
        // the lowercase-context penalty pathway.
        let rx = DecoderRecognizer::new();
        let mid_line_cx = ParseContext {
            line_offset: Some(20),
            line_prefix: Some(LinePrefix::from_slice(b"that's clearly prose ")),
            ..deep_cx()
        };
        match rx.recognize(b"(C)", 0, &*TEST_SCHEME, &mid_line_cx) {
            Parsed::Unambiguous(m) => {
                // Verify the line position penalty was recorded on
                // the surviving candidate.
                let has_penalty = m.1.as_ref().is_some_and(|p| {
                    p.features
                        .iter()
                        .any(|f| matches!(f.id, FeatureId::LinePositionPenalty))
                });
                assert!(
                    has_penalty,
                    "(C) mid-line must record LinePositionPenalty in \
                     provenance even though it survives the null-filter \
                     bypass; got {:?}",
                    m.1,
                );
            }
            Parsed::Ambiguous { candidates } => panic!(
                "(C) on the bare-classification whitelist must reach \
                 Unambiguous (engine eats the no-op rewrite); got \
                 Ambiguous with {} candidate(s)",
                candidates.len(),
            ),
        }
    }

    #[test]
    fn decoder_records_position_penalty_vs_bullet_bonus_for_bare_classification() {
        // The bullet anchor `1B.a.3.` cancels the position penalty
        // AND adds a positive bonus; running-prose context with a
        // non-anchor prefix that exceeds the position budget records
        // the position penalty. Issue #472: `(C)` is on the
        // [`is_bare_classification_shape`] whitelist so both contexts
        // resolve to Unambiguous at the decoder layer — engine-layer
        // no-op-rewrite filtering eats any synthetic R001 when the
        // observed bytes already match the canonical form. This test
        // pins the *feature-emission* asymmetry (penalty vs bonus)
        // directly on the surviving candidates, identical input bytes,
        // differing context.
        let rx = DecoderRecognizer::new();
        let bullet_cx = ParseContext {
            line_offset: Some(8),
            line_prefix: Some(LinePrefix::from_slice(b"1B.a.3.")),
            ..deep_cx()
        };
        let prose_cx = ParseContext {
            line_offset: Some(24),
            line_prefix: Some(LinePrefix::from_slice(b"the early prevalence of ")),
            ..deep_cx()
        };
        let bullet_result = rx.recognize(b"(C)", 0, &*TEST_SCHEME, &bullet_cx);
        let prose_result = rx.recognize(b"(C)", 0, &*TEST_SCHEME, &prose_cx);

        // Prose context: position penalty recorded on the candidate.
        match &prose_result {
            Parsed::Unambiguous(m) => {
                let has_penalty = m.1.as_ref().is_some_and(|p| {
                    p.features
                        .iter()
                        .any(|f| matches!(f.id, FeatureId::LinePositionPenalty))
                });
                assert!(
                    has_penalty,
                    "prose context `(C)` must record LinePositionPenalty \
                     in provenance, got {:?}",
                    m.1,
                );
            }
            Parsed::Ambiguous { candidates } => panic!(
                "`(C)` mid-prose must reach Unambiguous at decoder layer \
                 (engine eats no-op rewrite); got Ambiguous with {} \
                 candidate(s)",
                candidates.len(),
            ),
        }
        // Bullet context: bonus recorded on the candidate.
        match &bullet_result {
            Parsed::Unambiguous(m) => {
                let has_bonus = m.1.as_ref().is_some_and(|p| {
                    p.features
                        .iter()
                        .any(|f| matches!(f.id, FeatureId::BulletAnchorBonus))
                });
                assert!(
                    has_bonus,
                    "bullet-anchor recovery must record \
                     BulletAnchorBonus in provenance, got {:?}",
                    m.1,
                );
            }
            Parsed::Ambiguous { candidates } => panic!(
                "`(C)` after `1B.a.3.` bullet anchor must recover, \
                 got Ambiguous with {} candidate(s)",
                candidates.len(),
            ),
        }
    }

    #[test]
    fn decoder_applies_lowercase_context_penalty_in_lowercase_prose() {
        // A lowercase candidate (`(c)`) in lowercase-dominant
        // context: feature fires, posterior drops, null filter
        // suppresses. This is the prose-glyph case Task 10 targets —
        // mid-sentence parenthetical copyright `(c)`.
        let rx = DecoderRecognizer::new();
        let lowercase_prose = ParseContext {
            line_offset: Some(12),
            line_prefix: Some(LinePrefix::from_slice(b"the work ")),
            surrounding_is_lowercase: true,
            ..deep_cx()
        };
        match rx.recognize(b"(c)", 0, &*TEST_SCHEME, &lowercase_prose) {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "(c) in lowercase prose must be zero-candidate, got {}",
                candidates.len(),
            ),
            Parsed::Unambiguous(m) => panic!(
                "(c) in lowercase prose must be suppressed by the \
                 combined position + lowercase penalty, got \
                 Unambiguous({:?})",
                m.0.classification,
            ),
        }
    }

    #[test]
    fn decoder_skips_lowercase_penalty_when_candidate_is_uppercase() {
        // Uppercase candidate (`(S//NF)`) in lowercase-dominant
        // context. The lowercase-surrounding feature gate requires
        // the candidate ITSELF to carry lowercase letters; an
        // uppercase marking surrounded by lowercase prose is the
        // canonical IC body-text case and must NOT receive the
        // lowercase penalty.
        //
        // `(S//NF)` (not `(S)`) bypasses both the
        // portion-shape null filter (the trigger is
        // `!has_double_slash(bytes) && !is_bare_classification_shape(bytes)`;
        // `S//NF` contains `//`) and the position penalty
        // (`line_offset: 0`). That isolates the lowercase feature
        // gate: if the candidate-has-lowercase predicate is wrong,
        // this test fails; if the gate is correct, the candidate
        // recovers.
        let rx = DecoderRecognizer::new();
        let cx = ParseContext {
            line_offset: Some(0),
            line_prefix: Some(LinePrefix::empty()),
            surrounding_is_lowercase: true,
            ..deep_cx()
        };
        match rx.recognize(b"(S//NF)", 0, &*TEST_SCHEME, &cx) {
            Parsed::Unambiguous(m) => {
                // Verify no lowercase-context feature was emitted —
                // the candidate is fully uppercase, so the gate
                // must short-circuit.
                if let Some(p) = m.1.as_ref() {
                    assert!(
                        !p.features
                            .iter()
                            .any(|f| matches!(f.id, FeatureId::LowercaseSurroundingContext)),
                        "uppercase candidate must not receive \
                         LowercaseSurroundingContext, got features \
                         {:?}",
                        p.features,
                    );
                }
            }
            Parsed::Ambiguous { candidates } => panic!(
                "uppercase `(S//NF)` in lowercase prose must recover \
                 (lowercase feature gate short-circuits on uppercase \
                 candidates), got Ambiguous({})",
                candidates.len(),
            ),
        }
    }

    #[test]
    fn compute_context_features_skips_banners_and_cabs() {
        // Position + lowercase features only apply to portion shapes.
        // A banner candidate at line offset 50 in lowercase prose
        // must NOT receive any context features (banners are
        // line-bound by structure; CAB rows have fielded labels).
        let banner_cx = ParseContext {
            line_offset: Some(50),
            line_prefix: Some(LinePrefix::from_slice(b"prose prose prose ")),
            surrounding_is_lowercase: true,
            ..deep_cx()
        };
        let features = compute_context_features(MarkingType::Banner, b"secret", &banner_cx);
        assert!(
            features.is_empty(),
            "banner shape must not receive position/lowercase context features, got {features:?}",
        );
        let cab_cx = banner_cx;
        let features = compute_context_features(MarkingType::Cab, b"Classified By: foo", &cab_cx);
        assert!(
            features.is_empty(),
            "CAB shape must not receive position/lowercase context features, got {features:?}",
        );
    }

    #[test]
    fn compute_context_features_no_op_without_engine_populated_position() {
        // Direct callers (test code, WASM) that don't compute
        // `line_offset` / `line_prefix` get default-empty features —
        // identical behavior to pre-Task-9/10 decoder.
        let cx_no_position = ParseContext {
            line_offset: None,
            line_prefix: None,
            surrounding_is_lowercase: false,
            ..deep_cx()
        };
        let features = compute_context_features(MarkingType::Portion, b"(s)", &cx_no_position);
        assert!(
            features.is_empty(),
            "engine without populated position must produce no features, got {features:?}",
        );
    }

    #[test]
    fn compute_context_features_lowercase_fires_independent_of_position() {
        // The lowercase-surrounding feature is orthogonal to the
        // position features — it depends on `surrounding_is_lowercase`
        // + the candidate-has-lowercase predicate alone. Verify the
        // gates compose correctly: even with `line_offset` /
        // `line_prefix` left as `None`, a lowercase candidate in
        // lowercase context still receives the penalty. This locks
        // in the contract documented in
        // `compute_context_features_no_op_without_engine_populated_position`
        // — that the position fields not being populated does NOT
        // suppress the lowercase feature.
        let cx = ParseContext {
            line_offset: None,
            line_prefix: None,
            surrounding_is_lowercase: true,
            ..deep_cx()
        };
        let features = compute_context_features(MarkingType::Portion, b"(s)", &cx);
        assert!(
            features
                .iter()
                .any(|(id, _)| matches!(id, FeatureId::LowercaseSurroundingContext)),
            "lowercase candidate in lowercase context must receive \
             LowercaseSurroundingContext regardless of line position \
             availability, got {features:?}",
        );
        assert!(
            !features.iter().any(|(id, _)| matches!(
                id,
                FeatureId::LinePositionPenalty | FeatureId::BulletAnchorBonus
            )),
            "position features must not fire without engine-populated \
             line_offset/line_prefix, got {features:?}",
        );
    }

    #[test]
    fn compute_context_features_emits_bullet_bonus_for_anchor_prefix() {
        let bullet_cx = ParseContext {
            line_offset: Some(8),
            line_prefix: Some(LinePrefix::from_slice(b"1B.a.3.")),
            ..deep_cx()
        };
        let features = compute_context_features(MarkingType::Portion, b"(C)", &bullet_cx);
        assert!(
            features
                .iter()
                .any(|(id, _)| matches!(id, FeatureId::BulletAnchorBonus)),
            "expected BulletAnchorBonus, got {features:?}",
        );
        assert!(
            !features
                .iter()
                .any(|(id, _)| matches!(id, FeatureId::LinePositionPenalty)),
            "BulletAnchorBonus and LinePositionPenalty are mutually exclusive, got {features:?}",
        );
    }

    #[test]
    fn compute_context_features_emits_position_penalty_for_non_anchor() {
        let prose_cx = ParseContext {
            line_offset: Some(20),
            line_prefix: Some(LinePrefix::from_slice(b"the early prevalence of ")),
            ..deep_cx()
        };
        let features = compute_context_features(MarkingType::Portion, b"(s)", &prose_cx);
        let has_position = features
            .iter()
            .any(|(id, _)| matches!(id, FeatureId::LinePositionPenalty));
        assert!(
            has_position,
            "expected LinePositionPenalty, got {features:?}",
        );
    }

    // ---- end Task 9 / 10 context-feature tests ----

    #[test]
    fn decoder_rejects_bare_restricted_via_recognizer_predicate() {
        // `(R)` parses cleanly under the strict path's lenient
        // grammar but fails `is_us_restricted` at
        // both the strict recognizer and inside the decoder's
        // candidate loop (step 3c-bis). The decoder must produce
        // zero candidates regardless of preceded-by-whitespace.
        let rx = DecoderRecognizer::new();
        for cx in &[
            deep_cx(),
            ParseContext {
                preceded_by_whitespace: false,
                ..deep_cx()
            },
        ] {
            match rx.recognize(b"(r)", 0, &*TEST_SCHEME, cx) {
                Parsed::Ambiguous { candidates } => assert!(
                    candidates.is_empty(),
                    "bare (r) must be zero-candidate (preceded_by_whitespace={}), got {}",
                    cx.preceded_by_whitespace,
                    candidates.len()
                ),
                Parsed::Unambiguous(m) => panic!(
                    "bare (r) must be rejected, got Unambiguous({:?})",
                    m.0.classification
                ),
            }
        }
    }

    #[test]
    fn decoder_recovers_superseded_comint_to_si() {
        let rx = DecoderRecognizer::new();
        // SECRET//COMINT//NOFORN — COMINT is CAPCO-2016 §A.6 p16-superseded to SI.
        match rx.recognize(b"SECRET//COMINT//NOFORN", 0, &*TEST_SCHEME, &deep_cx()) {
            Parsed::Unambiguous(m) => {
                assert_eq!(marking_classification(&m), Some(Classification::Secret));
                // Verify SI is in the SCI controls list after correction.
                let has_si =
                    m.0.sci_controls
                        .iter()
                        .any(|c| matches!(c, marque_ism::SciControl::Si));
                assert!(
                    has_si,
                    "expected SI in sci_controls after COMINT supersession"
                );
            }
            other => panic!("expected Unambiguous, got {other:?}"),
        }
    }

    #[test]
    fn decoder_recovers_reordered_banner() {
        let rx = DecoderRecognizer::new();
        // Dissem-first mangled; canonical is classification-first.
        match rx.recognize(b"NOFORN//SECRET", 0, &*TEST_SCHEME, &deep_cx()) {
            Parsed::Unambiguous(m) => {
                assert_eq!(marking_classification(&m), Some(Classification::Secret));
            }
            Parsed::Ambiguous { candidates } => {
                assert!(
                    !candidates.is_empty(),
                    "reordering should at least surface candidates"
                );
            }
        }
    }

    #[test]
    fn decoder_honors_classification_floor_fr011() {
        let rx = DecoderRecognizer::new();
        // Input is "(U)" which canonicalizes to an UNCLASSIFIED
        // portion. With a Secret floor, the candidate must be
        // dropped.
        let cx = ParseContext {
            strict_evidence: false,
            classification_floor: Some(Classification::Secret as u8),
            preceded_by_whitespace: true,
            ..ParseContext::default()
        };
        match rx.recognize(b"(U)", 0, &*TEST_SCHEME, &cx) {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "UNCLASSIFIED below SECRET floor must produce zero candidates, got {}",
                candidates.len()
            ),
            Parsed::Unambiguous(m) => panic!(
                "expected zero-candidate, got Unambiguous({:?})",
                marking_classification(&m)
            ),
        }
    }

    #[test]
    fn decoder_classification_floor_allows_equal_or_above() {
        let rx = DecoderRecognizer::new();
        // (S//NF) with Confidential floor — SECRET exceeds floor.
        let cx = ParseContext {
            strict_evidence: false,
            classification_floor: Some(Classification::Confidential as u8),
            preceded_by_whitespace: true,
            ..ParseContext::default()
        };
        match rx.recognize(b"(S//NF)", 0, &*TEST_SCHEME, &cx) {
            Parsed::Unambiguous(m) => {
                assert_eq!(marking_classification(&m), Some(Classification::Secret));
            }
            other => panic!("expected Unambiguous, got {other:?}"),
        }
    }

    #[test]
    fn normalize_delimiters_collapses_garbled_slash() {
        let (out, _) = normalize_delimiters_and_case("S ∕∕ NOFORN");
        assert_eq!(out, "S//NOFORN");
    }

    #[test]
    fn normalize_delimiters_handles_double_spaced_slashes() {
        // PR #463 Copilot regression: pre-fix table-ordering left `"/ / "`
        // (4 byte) ahead of `" / / "` (5 byte), so the 4-byte rule consumed
        // the inner spaces before the 5-byte rule could match. Output was
        // `"S //NF"`. With longest-first ordering the 5-byte rule fires
        // first and collapses to canonical form.
        let (out, _) = normalize_delimiters_and_case("S / / NF");
        assert_eq!(out, "S//NF");
    }

    #[test]
    fn normalize_delimiters_converges_in_two_passes() {
        // PR #463 Copilot regression follow-up: even with longest-first
        // ordering, some inputs require a second pass. `"S / /NF"` first
        // matches the 3-byte `"/ /"` (positions 2-4) and yields
        // `"S //NF"`; the leading-space variant `" //"` only matches on
        // the next iteration. The fixpoint loop catches this.
        let (out, _) = normalize_delimiters_and_case("S / /NF");
        assert_eq!(out, "S//NF");
    }

    #[test]
    fn scan_token_captures_compound_with_hyphen() {
        assert_eq!(scan_token("SI-G ABCD"), 4); // "SI-G"
        assert_eq!(scan_token("HCS-P"), 5);
        assert_eq!(scan_token("SECRET//"), 6);
    }

    #[test]
    fn try_canonical_reorder_swaps_dissem_first_banner() {
        assert_eq!(
            try_canonical_reorder("NOFORN//SECRET"),
            Some("SECRET//NOFORN".to_owned())
        );
    }

    #[test]
    fn try_canonical_reorder_returns_none_when_already_canonical() {
        assert_eq!(try_canonical_reorder("SECRET//NOFORN"), None);
    }

    #[test]
    fn classify_segment_treats_sci_as_other_not_dissem() {
        // HCS and SI are SCI controls per CAPCO §A.6, not dissem.
        // Regression guard for PR #114 review — previously HCS was
        // in `DISSEMS`, which caused `try_canonical_reorder` to
        // move an HCS segment to the very end of the banner/portion
        // (past the dissem block) and corrupt canonicalization.
        // SCI segments must fall through to `SegmentClass::Other`
        // so the reorder helper places them between classification
        // and dissem per §A.6.
        assert_eq!(classify_segment("HCS"), SegmentClass::Other);
        assert_eq!(classify_segment("HCS-P"), SegmentClass::Other);
        assert_eq!(classify_segment("SI"), SegmentClass::Other);
        assert_eq!(classify_segment("SI-G"), SegmentClass::Other);
        assert_eq!(classify_segment("TK"), SegmentClass::Other);
    }

    #[test]
    fn classify_segment_non_ic_dissem_tokens() {
        // §H.9 abbreviations and long-title forms must classify as Dissem so
        // try_canonical_reorder places them after SCI, not in Other.
        // Regression guard for PR #256.
        for tok in &[
            "DS", "XD", "ND", "SBU", "SBU-NF", "LES", "LES-NF", "SSI", "LIMDIS", "EXDIS", "NODIS",
        ] {
            assert_eq!(
                classify_segment(tok),
                SegmentClass::Dissem,
                "classify_segment({tok:?}) should be Dissem"
            );
        }
        // Multi-word long-title forms.
        assert_eq!(
            classify_segment("LIMITED DISTRIBUTION"),
            SegmentClass::Dissem
        );
        assert_eq!(
            classify_segment("EXCLUSIVE DISTRIBUTION"),
            SegmentClass::Dissem
        );
        assert_eq!(classify_segment("NO DISTRIBUTION"), SegmentClass::Dissem);
        assert_eq!(
            classify_segment("LAW ENFORCEMENT SENSITIVE"),
            SegmentClass::Dissem
        );
        assert_eq!(
            classify_segment("SENSITIVE BUT UNCLASSIFIED"),
            SegmentClass::Dissem
        );
        assert_eq!(
            classify_segment("SENSITIVE SECURITY INFORMATION"),
            SegmentClass::Dissem
        );
    }

    #[test]
    fn classify_segment_restricted_data_is_not_classification() {
        // "RESTRICTED DATA" (AEA, §H.6) must not be mistaken for the NATO
        // RESTRICTED classification even though "RESTRICTED" is in CLASSIFICATIONS.
        // Bare "RESTRICTED" (NATO classification) must still be Classification.
        // Regression guard for PR #256.
        assert_eq!(classify_segment("RESTRICTED DATA"), SegmentClass::Other);
        assert_eq!(
            classify_segment("RESTRICTED DATA-CNWDI"),
            SegmentClass::Other
        );
        assert_eq!(classify_segment("RESTRICTED"), SegmentClass::Classification);
    }

    #[test]
    fn try_canonical_reorder_places_sci_between_classification_and_dissem() {
        // Dissem-first with an SCI segment in the middle — correct
        // canonical order is classification → SCI → dissem.
        assert_eq!(
            try_canonical_reorder("NOFORN//HCS-P//SECRET"),
            Some("SECRET//HCS-P//NOFORN".to_owned())
        );
    }

    #[test]
    fn meets_classification_floor_rejects_below_floor() {
        // Synthesize a marking via the decoder and check the floor
        // predicate directly.
        //
        // Issue #258: pre-#258 this used `(U)` (portion form), but
        // single-letter portions are now suppressed by the prose null
        // hypothesis. Switch to `UNCLASSIFIED` (banner form) — the
        // prose-side prior for the full word is at the Laplace floor
        // (zero hits in 134M prose-corpus words), so the marking-y
        // delta is huge and the candidate resolves cleanly. The unit
        // under test is `meets_classification_floor`, not the decoder
        // dispatch, so the choice of input shape is incidental.
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(u_marking) =
            rx.recognize(b"UNCLASSIFIED", 0, &*TEST_SCHEME, &deep_cx())
        else {
            panic!("UNCLASSIFIED should decode to unambiguous UNCLASSIFIED");
        };
        // U below S floor → rejected.
        assert!(!meets_classification_floor(
            &u_marking,
            Classification::Secret as u8
        ));
        // U meets U floor.
        assert!(meets_classification_floor(
            &u_marking,
            Classification::Unclassified as u8
        ));
    }

    // ----- SAR indicator-keyword structural repair (issue #133 PR 6) -----

    #[test]
    fn sar_indicator_repair_strips_one_letter_prefix() {
        // The canonical USAR-BP shape from the mangled corpus.
        assert_eq!(
            try_sar_indicator_repair(
                "SECRET//USAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN"
            ),
            Some("SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN".to_owned())
        );
    }

    #[test]
    fn sar_indicator_repair_strips_multi_letter_prefix() {
        // Two- and three-letter prefixes are still in the structural
        // window. `XYZ` isn't a CAPCO token or trigraph.
        assert_eq!(
            try_sar_indicator_repair("SECRET//ABSAR-BP//NOFORN"),
            Some("SECRET//SAR-BP//NOFORN".to_owned())
        );
        assert_eq!(
            try_sar_indicator_repair("SECRET//XYZSAR-BP//NOFORN"),
            Some("SECRET//SAR-BP//NOFORN".to_owned())
        );
    }

    #[test]
    fn sar_indicator_repair_strips_even_capco_token_prefix() {
        // The prefix-strip pass intentionally does NOT defend
        // against prefixes that spell a CAPCO token in isolation
        // (`U`, `S`, `R`, `C`, `TS`, `SI`, `USA`, …). Canonical
        // CAPCO never glues a classification token, SCI control,
        // or trigraph directly to `SAR-` without a `//` separator,
        // so the apparent prefix at a `//`/`(`/start boundary is
        // OCR/transcription drift regardless of whether the bytes
        // happen to spell a known token. An earlier defensive check
        // that refused to strip such prefixes broke the central
        // `USAR-` recovery case (`U` is the UNCLASSIFIED portion
        // form). Pinned here so a future "be more conservative"
        // PR reviews the rationale before re-adding the guard.
        assert_eq!(
            try_sar_indicator_repair("SECRET//USASAR-BP//NOFORN"),
            Some("SECRET//SAR-BP//NOFORN".to_owned()),
            "must strip USA at boundary even though USA is a trigraph",
        );
        assert_eq!(
            try_sar_indicator_repair("(USAR-BP)"),
            Some("(SAR-BP)".to_owned()),
            "boundary `(` must also trigger the strip pass",
        );
    }

    #[test]
    fn sar_indicator_repair_inserts_missing_hyphen_two_char_id() {
        // The canonical SARBP missing-hyphen shape.
        assert_eq!(
            try_sar_indicator_repair("TOP SECRET//SARBP//NOFORN"),
            Some("TOP SECRET//SAR-BP//NOFORN".to_owned())
        );
    }

    #[test]
    fn sar_indicator_repair_inserts_missing_hyphen_three_char_id() {
        // 3-char alphanumeric program identifier per §H.5 p100.
        assert_eq!(
            try_sar_indicator_repair("TOP SECRET//SARABC//NOFORN"),
            Some("TOP SECRET//SAR-ABC//NOFORN".to_owned())
        );
    }

    #[test]
    fn sar_indicator_repair_inserts_missing_hyphen_before_compound() {
        // `SARBP-J12` → `SAR-BP-J12`. The 2-char alnum run BP
        // terminates at the `-` delimiter; that's the missing-hyphen
        // pattern. The trailing `-J12` is preserved verbatim.
        assert_eq!(
            try_sar_indicator_repair("SECRET//SARBP-J12 J54//NOFORN"),
            Some("SECRET//SAR-BP-J12 J54//NOFORN".to_owned())
        );
    }

    #[test]
    fn sar_indicator_repair_no_op_on_canonical() {
        // Canonical SAR shapes must pass through with `None`.
        let cases: &[&str] = &[
            "SECRET//SAR-BP//NOFORN",
            "SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN",
            "TOP SECRET//SPECIAL ACCESS REQUIRED-BUTTER POPCORN//NOFORN",
            "SECRET//NOFORN",
        ];
        for input in cases {
            assert_eq!(
                try_sar_indicator_repair(input),
                None,
                "canonical input {input:?} must not be repaired"
            );
        }
    }

    #[test]
    fn sar_indicator_repair_skips_non_boundary_sar() {
        // `SAR` embedded mid-token (no boundary char before `S`)
        // is not the indicator — could be a SAR program identifier
        // happening to contain the letters. Don't touch.
        assert_eq!(
            try_sar_indicator_repair("SECRET//FOO-USAR-BP"),
            None,
            "non-boundary SAR is not the indicator keyword"
        );
    }

    #[test]
    fn sar_indicator_repair_skips_long_alnum_run() {
        // 4+ alphanumeric chars after SAR don't match the §H.5 p100
        // 2-3 char Abbrev-form identifier. The helper refuses to
        // insert a hyphen — inserting `SAR-ABCD` would be inventing
        // a malformed identifier.
        assert_eq!(
            try_sar_indicator_repair("SECRET//SARABCD//NOFORN"),
            None,
            "4-char alnum run violates §H.5 p100 2-3 char identifier"
        );
    }

    #[test]
    fn sar_indicator_repair_returns_none_when_no_sar_substring() {
        // Pre-check fast path: if `SAR` doesn't appear in the input
        // at all, no repair is possible.
        assert_eq!(
            try_sar_indicator_repair("TOP SECRET//SI-G ABCD//NOFORN"),
            None
        );
        assert_eq!(try_sar_indicator_repair(""), None);
        assert_eq!(try_sar_indicator_repair("UNCLASSIFIED"), None);
    }

    #[test]
    fn match_sar_prefix_detects_one_to_three_letter_prefix() {
        assert_eq!(match_sar_prefix(b"USAR-BP", 0), Some((1, 5)));
        assert_eq!(match_sar_prefix(b"ABSAR-BP", 0), Some((2, 6)));
        assert_eq!(match_sar_prefix(b"XYZSAR-BP", 0), Some((3, 7)));
    }

    #[test]
    fn match_sar_prefix_rejects_no_prefix_or_no_sar() {
        assert_eq!(match_sar_prefix(b"SAR-BP", 0), None);
        assert_eq!(match_sar_prefix(b"USAR", 0), None);
        assert_eq!(match_sar_prefix(b"USARBP", 0), None);
    }

    #[test]
    fn match_sar_missing_hyphen_detects_2_3_char_id() {
        assert_eq!(match_sar_missing_hyphen(b"SARBP/", 0), Some(5));
        assert_eq!(match_sar_missing_hyphen(b"SARABC ", 0), Some(6));
        // End-of-string also counts as a delim.
        assert_eq!(match_sar_missing_hyphen(b"SARBP", 0), Some(5));
    }

    #[test]
    fn match_sar_missing_hyphen_rejects_canonical_and_too_long() {
        // `SAR-` already canonical (alnum run is 0).
        assert_eq!(match_sar_missing_hyphen(b"SAR-BP", 0), None);
        // 4-char alnum run is outside the §H.5 p100 2-3 window.
        assert_eq!(match_sar_missing_hyphen(b"SARABCD/", 0), None);
        // 1-char alnum run is also outside the window.
        assert_eq!(match_sar_missing_hyphen(b"SARB/", 0), None);
    }

    #[test]
    fn match_sar_missing_hyphen_rejects_non_delim_following_char() {
        // Alnum run is in the §H.5 p100 2-3 window, but the byte
        // immediately after the run is non-alphanumeric AND not in
        // the delimiter set (`-`, `/`, ` `, `\t`, `\n`, `\r`).
        // Every non-delim non-alnum byte triggers the
        // `next_is_delim = false` branch and the helper returns
        // `None` — refusing to repair grammatically-suspicious
        // shapes (a SAR identifier doesn't terminate at `,`, `)`,
        // `;`, etc.). Direct-helper test because the higher-level
        // pinning in `try_sar_indicator_repair` only exercises a
        // subset of these via the boundary check upstream.
        let cases: &[&[u8]] = &[
            b"SARBP)",  // closing paren — same byte that ends a portion mark
            b"SARBP,",  // comma — common typo separator
            b"SARBP;",  // semicolon
            b"SARBP*",  // asterisk
            b"SARBP=",  // equals
            b"SARABC.", // period after 3-char id
            b"SARABC?", // question mark
        ];
        for input in cases {
            assert_eq!(
                match_sar_missing_hyphen(input, 0),
                None,
                "input {:?} has non-delim follower; helper must refuse repair",
                std::str::from_utf8(input).unwrap_or("<non-utf8>"),
            );
        }
    }

    #[test]
    fn sar_indicator_repair_skips_pattern_b_with_non_delim_follower() {
        // End-to-end pinning of the same `next_is_delim = false`
        // rejection through `try_sar_indicator_repair`. `SARBP)`
        // appears at a `//` boundary (so `at_boundary` is true and
        // Pattern B is attempted), the alnum run is 2, but `)` isn't
        // in the delim set — the helper falls through to the
        // verbatim-copy default. Without the rejection branch we'd
        // emit `SAR-BP)`, silently inventing a hyphen for a
        // grammatically-suspicious input.
        assert_eq!(
            try_sar_indicator_repair("SECRET//SARBP)//NOFORN"),
            None,
            "Pattern B must refuse to fire when the post-alnum char isn't a delim",
        );
    }

    // ----- Stray-character `/X/` recovery (issue #133 PR 7) -----

    #[test]
    fn try_collapse_stray_char_slash_emits_three_transforms() {
        // Each `/X/` match emits exactly three candidate bytes
        // (drop, right-attach, left-attach). This pins the contract
        // and makes any future scope expansion (multi-pass, extra
        // transforms) a deliberate, reviewable change.
        let result = try_collapse_stray_char_slash("AB/X/CD");
        assert_eq!(result.len(), 3, "expected 3 candidates; got {result:?}");
        assert_eq!(result[0], "AB//CD"); // drop X
        assert_eq!(result[1], "AB//XCD"); // right-attach X to CD
        assert_eq!(result[2], "ABX//CD"); // left-attach X to AB
    }

    #[test]
    fn try_collapse_stray_char_slash_returns_empty_when_no_pattern() {
        // Inputs without a `/X/` pattern produce no candidates.
        let cases: &[&str] = &[
            "SECRET",
            "SECRET//NOFORN",
            "SECRET//NOFORN//EXDIS",
            "(C)",
            "",
            // A `/` followed by 2+ alnum chars is NOT the pattern —
            // `/AB/` is a regular 2-char token between slashes.
            "SECRET/AB/CD",
            // `//` (canonical separator) doesn't match because the
            // single-char-between-slashes shape requires alnum at
            // bytes[i+1].
            "SECRET////NOFORN",
        ];
        for input in cases {
            assert!(
                try_collapse_stray_char_slash(input).is_empty(),
                "input {input:?} should not match /X/ pattern",
            );
        }
    }

    #[test]
    fn try_collapse_stray_char_slash_requires_alnum_boundary() {
        // The pattern requires alnum on both sides of `/X/`. Without
        // both, the recovery is semantically meaningless (no token
        // to attach X to / no token next to the strip).
        // Leading boundary missing: `/X/Y` at position 0 has no
        // alnum at i-1.
        assert!(try_collapse_stray_char_slash("/X/Y").is_empty());
        // Trailing boundary missing: `Y/X/` has no alnum at i+3.
        assert!(try_collapse_stray_char_slash("Y/X/").is_empty());
        // Both alnum: matches.
        assert_eq!(
            try_collapse_stray_char_slash("Y/X/Z").len(),
            3,
            "alnum on both sides should match"
        );
    }

    // ----- REL TO structural repair (issue #133 PR 9) -----

    #[test]
    fn rel_to_header_normalize_fixes_rel_ot_transposition() {
        // Pattern 1: `REL OT ` (TO → OT) → `REL TO `.
        let result = try_rel_to_header_normalize("SECRET//REL OT USA, AUS, GBR");
        assert_eq!(
            result.as_deref(),
            Some("SECRET//REL TO USA, AUS, GBR"),
            "REL OT must rewrite to REL TO at //-boundary",
        );
    }

    #[test]
    fn rel_to_header_normalize_fixes_relt_o_token_boundary() {
        // Pattern 2: `RELT O ` (T migrated from REL to start of next
        // token) → `REL TO `. The fuzzy pass would otherwise rewrite
        // `RELT` (4 chars) → `REL` (in-vocab DissemControl, distance
        // 1) and silently drop USA from the strict parse.
        let result = try_rel_to_header_normalize("SECRET//RELT O USA, AUS, GBR");
        assert_eq!(
            result.as_deref(),
            Some("SECRET//REL TO USA, AUS, GBR"),
            "RELT O must rewrite to REL TO at //-boundary",
        );
    }

    #[test]
    fn rel_to_header_normalize_returns_none_on_canonical() {
        // Canonical `REL TO ` (and texts without REL at all) round-
        // trip unchanged.
        assert!(try_rel_to_header_normalize("SECRET//REL TO USA, AUS, GBR").is_none());
        assert!(try_rel_to_header_normalize("SECRET//NOFORN").is_none());
        assert!(try_rel_to_header_normalize("").is_none());
    }

    #[test]
    fn rel_to_header_normalize_requires_token_boundary() {
        // The pattern must not fire when embedded inside a longer
        // alphanumeric run. Without the boundary check, `XREL OT Y`
        // would match the substring `REL OT` even though the leading
        // `X` makes the whole thing a single 6-char token.
        assert!(try_rel_to_header_normalize("XREL OT Y").is_none());
        assert!(try_rel_to_header_normalize("SOMETHINGRELT O Y").is_none());
    }

    #[test]
    fn rel_to_entry_normalize_joins_a_us_to_aus() {
        // Pattern 3: 4-char entry `A US` joins to AUS only when the
        // joined 3-letter string is a known trigraph. AUS is a
        // trigraph; A alone is not.
        let result = try_rel_to_entry_normalize("SECRET//REL TO USA,A US, GBR");
        // The replacement preserves the entry's leading whitespace
        // (none here), so the rewritten block is `USA,AUS, GBR`.
        assert_eq!(
            result.as_deref(),
            Some("SECRET//REL TO USA,AUS, GBR"),
            "A US should join to AUS when is_trigraph(AUS) holds",
        );
    }

    #[test]
    fn rel_to_entry_normalize_swaps_au_comma_s_to_aus_comma() {
        // Pattern 4: `<2-upper>,<1-upper><space>` swaps to
        // `<3-upper joined>,` only when the joined trigraph is
        // valid AND the 2-letter prefix alone is not a trigraph.
        let result = try_rel_to_entry_normalize("SECRET//REL TO USA, AU,S GBR");
        assert_eq!(
            result.as_deref(),
            Some("SECRET//REL TO USA, AUS, GBR"),
            "AU,S should swap to AUS, when is_trigraph(AUS) holds and AU is not a trigraph",
        );
    }

    #[test]
    fn rel_to_entry_normalize_does_not_corrupt_eu_comma_pattern() {
        // EU is itself a valid 2-char trigraph entry. Pattern 4 must
        // not fire on `EU,X ` because `is_trigraph(EU)` is true —
        // this guards the rule "only fix when the prefix alone is
        // invalid". (Even though `EUX` may not be a trigraph and
        // wouldn't pass the join-is-trigraph guard either, the
        // prefix-is-trigraph check is the cleaner discriminator.)
        let result = try_rel_to_entry_normalize("SECRET//REL TO USA, EU, GBR");
        assert!(
            result.is_none(),
            "canonical EU entry must round-trip unchanged",
        );
    }

    #[test]
    fn rel_to_entry_normalize_returns_none_outside_rel_to() {
        // No REL TO header → no entry-pass fixes. The patterns are
        // scoped to inside REL TO blocks specifically.
        assert!(try_rel_to_entry_normalize("SECRET//SI/TK//NOFORN").is_none());
        assert!(try_rel_to_entry_normalize("").is_none());
    }

    #[test]
    fn rel_to_structural_repair_short_circuits_without_rel() {
        // Pre-check: text without `REL` returns None immediately,
        // skipping the byte walks.
        assert!(try_rel_to_structural_repair("SECRET//NOFORN").is_none());
        assert!(try_rel_to_structural_repair("(C)").is_none());
        assert!(try_rel_to_structural_repair("").is_none());
    }

    // ----- SCI delimiter recovery (issue #198, #133 PR 10) -----

    #[test]
    fn sci_delimiter_repair_concatenated_compound_hcsp() {
        // Pattern A: `HCSP` (registered compound `HCS-P` with hyphen
        // missing) → `HCS-P`.
        let result = try_sci_delimiter_repair("SECRET//HCSP//NOFORN");
        assert_eq!(
            result.as_deref(),
            Some("SECRET//HCS-P//NOFORN"),
            "HCSP must rewrite to HCS-P (CVE-registered compound)",
        );
    }

    #[test]
    fn sci_delimiter_repair_concatenated_compound_hcso() {
        // Pattern A: HCSO → HCS-O.
        let result = try_sci_delimiter_repair("SECRET//HCSO//NOFORN");
        assert_eq!(result.as_deref(), Some("SECRET//HCS-O//NOFORN"));
    }

    #[test]
    fn sci_delimiter_repair_concatenated_compound_sig() {
        // Pattern A: SIG → SI-G. The CVE list has SI-G; G is a
        // compartment of SI per §A.6 p16.
        let result = try_sci_delimiter_repair("SECRET//SIG//NOFORN");
        assert_eq!(result.as_deref(), Some("SECRET//SI-G//NOFORN"));
    }

    #[test]
    fn sci_delimiter_repair_concatenated_compound_tkkand() {
        // Pattern A: TKKAND → TK-KAND. Tests that the longer
        // concatenated forms (6 chars) are matched correctly.
        let result = try_sci_delimiter_repair("SECRET//TKKAND//NOFORN");
        assert_eq!(result.as_deref(), Some("SECRET//TK-KAND//NOFORN"));
    }

    #[test]
    fn sci_delimiter_repair_schema_coverage_bur_compounds() {
        // Pattern A is schema-driven via `SciControl::parse`, so it
        // covers every CVE compound automatically — including the
        // BUR-* family that an earlier hand-maintained list omitted.
        // Locks in the schema-derived contract: any future ODNI
        // schema bump that adds new compounds is auto-covered without
        // changes to this file.
        assert_eq!(
            try_sci_delimiter_repair("SECRET//BURBLG//NOFORN").as_deref(),
            Some("SECRET//BUR-BLG//NOFORN"),
        );
        assert_eq!(
            try_sci_delimiter_repair("SECRET//BURDTP//NOFORN").as_deref(),
            Some("SECRET//BUR-DTP//NOFORN"),
        );
        assert_eq!(
            try_sci_delimiter_repair("SECRET//BURWRG//NOFORN").as_deref(),
            Some("SECRET//BUR-WRG//NOFORN"),
        );
    }

    #[test]
    fn sci_delimiter_repair_missing_slash_sitk() {
        // Pattern B: SITK → SI/TK. Per §A.6 p16 + p194 example,
        // multiple control systems within an SCI category use `/`.
        let result = try_sci_delimiter_repair("SECRET//SITK//NOFORN");
        assert_eq!(
            result.as_deref(),
            Some("SECRET//SI/TK//NOFORN"),
            "SITK must rewrite to SI/TK (two bare control systems concatenated)",
        );
    }

    #[test]
    fn sci_delimiter_repair_missing_slash_hcssi() {
        // Pattern B: HCSSI → HCS/SI. Tests 3+2 split (HCS is len 3,
        // SI is len 2).
        let result = try_sci_delimiter_repair("SECRET//HCSSI//NOFORN");
        assert_eq!(result.as_deref(), Some("SECRET//HCS/SI//NOFORN"));
    }

    #[test]
    fn sci_delimiter_repair_wrong_delimiter_si_dash_tk() {
        // Pattern C: SI-TK → SI/TK. The whole token is not a CVE
        // compound, both halves are bare CS, so `-` is wrong.
        let result = try_sci_delimiter_repair("SECRET//SI-TK//NOFORN");
        assert_eq!(
            result.as_deref(),
            Some("SECRET//SI/TK//NOFORN"),
            "SI-TK must rewrite to SI/TK (two bare CS, `-` is for control-compartment)",
        );
    }

    #[test]
    fn sci_delimiter_repair_leaves_registered_compound_alone() {
        // Pattern C must NOT fire on registered compounds. SI-G is in
        // CVEnumISMSCIControls.xml — `-` is the correct separator.
        assert!(try_sci_delimiter_repair("SECRET//SI-G//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("SECRET//HCS-P//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("SECRET//TK-KAND//NOFORN").is_none());
    }

    #[test]
    fn sci_delimiter_repair_returns_none_on_canonical() {
        // Already-canonical inputs round-trip unchanged.
        assert!(try_sci_delimiter_repair("SECRET//SI/TK//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("SECRET//SI//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("SECRET//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("").is_none());
    }

    #[test]
    fn sci_delimiter_repair_does_not_fire_on_word_substring() {
        // SIGMA contains "SIG" as a substring but is a single token
        // — Pattern A requires whole-token equality, not contains.
        assert!(try_sci_delimiter_repair("SIGMA").is_none());
        // SITE, SITS — same protection.
        assert!(try_sci_delimiter_repair("SITE").is_none());
        // SIGNAL — contains SIG; whole token is not in Pattern A.
        assert!(try_sci_delimiter_repair("SIGNAL").is_none());
    }

    #[test]
    fn sci_delimiter_repair_short_circuits_without_sci_root() {
        // Pre-check: no SCI control system substring → no repair.
        assert!(try_sci_delimiter_repair("CONFIDENTIAL//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("(C)").is_none());
        assert!(try_sci_delimiter_repair("").is_none());
    }

    #[test]
    fn sci_delimiter_repair_does_not_panic_on_non_ascii() {
        // The function must not panic on multi-byte UTF-8 input. The
        // SCI vocabulary is pure ASCII, so any non-ASCII input is
        // unmatchable — bail early rather than risk a byte-offset
        // slice landing mid-codepoint. Inputs intentionally chosen
        // to exercise both the outer scanner (`try_sci_delimiter_repair`)
        // and the inner per-token classifier (`repair_sci_token`).
        assert!(try_sci_delimiter_repair("SECRET//SI/TK//日本語").is_none());
        assert!(try_sci_delimiter_repair("Ω SI TK").is_none());
        assert!(try_sci_delimiter_repair("こんにちは").is_none());
        // Direct call to the per-token helper with non-ASCII content.
        assert!(repair_sci_token("SI日").is_none());
        assert!(repair_sci_token("日本").is_none());
    }

    #[test]
    fn repair_sci_token_rejects_partial_decompositions() {
        // HCSI = HCS+I (I not bare) or H+CSI (neither bare) — no
        // valid Pattern B decomposition.
        assert!(repair_sci_token("HCSI").is_none());
        // ABCDE — random, no valid CS decomposition.
        assert!(repair_sci_token("ABCDE").is_none());
        // BUR alone — bare CS by itself, len 3, fails Pattern B's
        // 4..=6 length check, no `-`, not in Pattern A. Returns None.
        assert!(repair_sci_token("BUR").is_none());
    }

    #[test]
    fn try_collapse_stray_char_slash_processes_only_first_match() {
        // PR 7 scope: only the first `/X/` is processed. Multi-
        // pattern inputs need a future multi-pass extension.
        let result = try_collapse_stray_char_slash("A/X/B/Y/C");
        assert_eq!(result.len(), 3);
        // Each candidate carries only the first transform — the
        // second `/Y/` pattern is left in place verbatim.
        assert_eq!(result[0], "A//B/Y/C"); // drop first X
        assert_eq!(result[1], "A//XB/Y/C"); // right-attach first X
        assert_eq!(result[2], "AX//B/Y/C"); // left-attach first X
    }

    #[test]
    fn decoder_recovers_drop_stray_char() {
        // End-to-end: `SECRET//NOFORN/R/EXDIS` resolves to the
        // canonical `SECRET//NOFORN//EXDIS` via the drop-X transform.
        // The right-attach (`SECRET//NOFORN//REXDIS` — REXDIS unknown)
        // and left-attach (`SECRET//NOFORNR//EXDIS` — NOFORNR unknown)
        // candidates are dropped by step 3a's Unknown-token filter.
        // Pinned per `tests/fixtures/mangled/typo/7885156a2c2c125f.json`.
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(marking) =
            rx.recognize(b"SECRET//NOFORN/R/EXDIS", 0, &*TEST_SCHEME, &deep_cx())
        else {
            panic!("`/R/` between NOFORN and EXDIS must resolve via drop-X");
        };
        assert_eq!(
            marking
                .0
                .classification
                .as_ref()
                .map(|c| c.effective_level()),
            Some(Classification::Secret),
        );
        assert!(
            marking
                .0
                .dissem_iter()
                .any(|d| matches!(d, marque_ism::DissemControl::Nf)),
            "NOFORN must survive; attrs = {:?}",
            marking.0,
        );
        assert!(
            marking
                .0
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Exdis)),
            "EXDIS must survive; attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn decoder_recovers_right_attach_stray_char() {
        // End-to-end: `TOP SECRET//SI/N/OFORN` resolves to the
        // canonical `TOP SECRET//SI//NOFORN` via right-attach (the
        // `N` was the leading char of NOFORN). The drop candidate
        // (`TOP SECRET//SI//OFORN` — OFORN unknown) and left-attach
        // (`TOP SECRET//SIN//OFORN` — both unknown) are dropped by
        // step 3a's Unknown-token filter. Pinned per
        // `tests/fixtures/mangled/typo/2cb13fe4682ff31c.json`.
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(marking) =
            rx.recognize(b"TOP SECRET//SI/N/OFORN", 0, &*TEST_SCHEME, &deep_cx())
        else {
            panic!("`/N/` before OFORN must resolve via right-attach");
        };
        assert_eq!(
            marking
                .0
                .classification
                .as_ref()
                .map(|c| c.effective_level()),
            Some(Classification::TopSecret),
        );
        assert!(
            marking
                .0
                .sci_controls
                .iter()
                .any(|c| matches!(c, marque_ism::SciControl::Si)),
            "SI must survive; attrs = {:?}",
            marking.0,
        );
        assert!(
            marking
                .0
                .dissem_iter()
                .any(|d| matches!(d, marque_ism::DissemControl::Nf)),
            "NOFORN must be reconstructed; attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn decoder_recovers_left_attach_stray_char() {
        // End-to-end: `SECRE/T/REL TO USA, AUS, GBR` resolves to the
        // canonical `SECRET//REL TO USA, AUS, GBR` via left-attach
        // (the `T` was the trailing char of SECRET). The drop
        // (`SECRE//REL TO ...` — SECRE unknown) and right-attach
        // (`SECRE//TREL TO ...` — both unknown) are dropped by
        // step 3a. Pinned per
        // `tests/fixtures/mangled/typo/cff1d0ac74e901c3.json`.
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(marking) = rx.recognize(
            b"SECRE/T/REL TO USA, AUS, GBR",
            0,
            &*TEST_SCHEME,
            &deep_cx(),
        ) else {
            panic!("`/T/` after SECRE must resolve via left-attach");
        };
        assert_eq!(
            marking
                .0
                .classification
                .as_ref()
                .map(|c| c.effective_level()),
            Some(Classification::Secret),
        );
        assert_eq!(
            marking.0.rel_to.len(),
            3,
            "REL TO must carry 3 trigraphs (USA, AUS, GBR); attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn decoder_recovers_usar_prefix_via_sar_indicator_repair() {
        // End-to-end recognizer test: the canonical USAR-BP fixture
        // shape from the mangled corpus must resolve unambiguously
        // to a SECRET marking with a SAR block. Pinned per
        // `tests/fixtures/mangled/typo/d04f45f7a4f5a8b4.json`.
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(marking) = rx.recognize(
            b"SECRET//USAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN",
            0,
            &*TEST_SCHEME,
            &deep_cx(),
        ) else {
            panic!("USAR-BP-... must resolve via SAR indicator repair");
        };
        assert_eq!(
            marking
                .0
                .classification
                .as_ref()
                .map(|c| c.effective_level()),
            Some(Classification::Secret),
        );
        assert!(
            marking.0.sar_markings.is_some(),
            "SAR block must be present after USAR→SAR repair; attrs = {:?}",
            marking.0,
        );
        assert!(
            marking
                .0
                .dissem_iter()
                .any(|d| matches!(d, marque_ism::DissemControl::Nf)),
            "NOFORN must survive; attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn decoder_recovers_sarbp_missing_hyphen_via_sar_indicator_repair() {
        // End-to-end: `SARBP` (no hyphen) → `SAR-BP` (canonical) per
        // §H.5 p100. Pinned per
        // `tests/fixtures/mangled/typo/fbf5ed813c109c14.json`.
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(marking) =
            rx.recognize(b"TOP SECRET//SARBP//NOFORN", 0, &*TEST_SCHEME, &deep_cx())
        else {
            panic!("SARBP must resolve via SAR indicator repair");
        };
        assert_eq!(
            marking
                .0
                .classification
                .as_ref()
                .map(|c| c.effective_level()),
            Some(Classification::TopSecret),
        );
        let sar = marking
            .0
            .sar_markings
            .as_ref()
            .expect("SAR block must be present");
        assert_eq!(sar.programs.len(), 1, "exactly one program; got {sar:?}");
        assert_eq!(
            &*sar.programs[0].identifier, "BP",
            "program identifier must be `BP` after hyphen insertion; got {sar:?}",
        );
    }

    #[test]
    fn decoder_recovers_spcial_via_extended_correction_vocab() {
        // `SPCIAL` (typo in `SPECIAL`) — issue #133 PR 6 vocab
        // addition. The fuzzy matcher now finds `SPECIAL` at edit
        // distance 1, the strict SAR parser then matches the
        // `SPECIAL ACCESS REQUIRED-BUTTER POPCORN` indicator
        // literally. Pinned per
        // `tests/fixtures/mangled/typo/1f75ddd89b432949.json`.
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(marking) = rx.recognize(
            b"TOP SECRET//SPCIAL ACCESS REQUIRED-BUTTER POPCORN//NOFORN",
            0,
            &*TEST_SCHEME,
            &deep_cx(),
        ) else {
            panic!("SPCIAL must fuzzy-correct to SPECIAL");
        };
        assert_eq!(
            marking
                .0
                .classification
                .as_ref()
                .map(|c| c.effective_level()),
            Some(Classification::TopSecret),
        );
        let sar = marking
            .0
            .sar_markings
            .as_ref()
            .expect("SAR block must be present");
        assert_eq!(
            &*sar.programs[0].identifier, "BUTTER POPCORN",
            "Full-form program identifier must round-trip; got {sar:?}",
        );
    }

    // ----- NATO longhand fold unit tests (T130) -----
    //
    // These tests exercise the segment-walking logic of `fold_nato_segment`
    // and `try_nato_fold` directly. End-to-end decode tests live in
    // `crates/engine/tests/decoder_recovery.rs` (T130 blocks).
    //
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.

    #[test]
    fn fold_nato_segment_abbrev_s_yields_ns() {
        assert_eq!(
            fold_nato_segment("NATO S", MarkingType::Portion).as_deref(),
            Some("NS"),
            "NATO S must fold to NS"
        );
    }

    #[test]
    fn fold_nato_segment_abbrev_ts_yields_cts() {
        assert_eq!(
            fold_nato_segment("NATO TS", MarkingType::Portion).as_deref(),
            Some("CTS"),
            "NATO TS must fold to CTS (COSMIC TOP SECRET)"
        );
    }

    #[test]
    fn fold_nato_segment_full_word_top_secret_yields_cts() {
        assert_eq!(
            fold_nato_segment("NATO TOP SECRET", MarkingType::Portion).as_deref(),
            Some("CTS"),
            "NATO TOP SECRET must fold to CTS"
        );
    }

    #[test]
    fn fold_nato_segment_returns_none_for_non_nato_segment() {
        // Segment-leading guard: segments not starting with "NATO " must
        // not be folded.
        assert!(
            fold_nato_segment("NS", MarkingType::Portion).is_none(),
            "canonical NATO abbrev must not re-fold"
        );
        assert!(
            fold_nato_segment("NOFORN", MarkingType::Portion).is_none(),
            "non-NATO segment must not fold"
        );
        assert!(
            fold_nato_segment("SI", MarkingType::Portion).is_none(),
            "SCI token must not fold"
        );
        assert!(
            fold_nato_segment("REL TO USA, NATO", MarkingType::Portion).is_none(),
            "NATO-in-list must not fold"
        );
    }

    #[test]
    fn fold_nato_segment_returns_none_for_empty() {
        assert!(
            fold_nato_segment("", MarkingType::Portion).is_none(),
            "empty segment must not fold"
        );
    }

    #[test]
    fn try_nato_fold_portion_without_leading_slash_gets_slash_added() {
        // `(NATO S)` → inner = `NATO S` → segments = [`NATO S`]
        // → fold to `NS` → no leading empty → add `//` → `(//NS)`
        assert_eq!(
            try_nato_fold("(NATO S)", MarkingType::Portion).as_deref(),
            Some("(//NS)"),
            "fold must add leading // for non-US classification position"
        );
    }

    #[test]
    fn try_nato_fold_portion_with_leading_slash_preserves_it() {
        // `(//NATO S//NF)` → inner = `//NATO S//NF` → segments = [``, `NATO S`, `NF`]
        // → fold to [``, `NS`, `NF`] → had_leading_empty=true → no extra //
        // → `(//NS//NF)`
        assert_eq!(
            try_nato_fold("(//NATO S//NF)", MarkingType::Portion).as_deref(),
            Some("(//NS//NF)"),
            "fold must preserve existing leading //"
        );
    }

    #[test]
    fn try_nato_fold_returns_none_for_canonical_input() {
        // Already canonical: `(//NS//NF)` — no `NATO ` segment → None
        assert!(
            try_nato_fold("(//NS//NF)", MarkingType::Portion).is_none(),
            "canonical input must return None (idempotent)"
        );
    }

    #[test]
    fn try_nato_fold_banner_abbreviation_folds_to_long_form() {
        // FIX-1: banner kind now supported. Abbreviation → long form.
        // `NATO S//NOFORN` is the banner abbreviation form; the strict parser
        // accepts `NATO SECRET//NOFORN` (long form) but NOT `NATO S//NOFORN`.
        // The fold expands the abbreviation to the long form and prepends `//`
        // per §A.6 p15.
        assert_eq!(
            try_nato_fold("NATO S//NOFORN", MarkingType::Banner).as_deref(),
            Some("//NATO SECRET//NOFORN"),
            "banner abbreviation must fold to long form with // prefix"
        );
        assert_eq!(
            try_nato_fold("NATO TS//NOFORN", MarkingType::Banner).as_deref(),
            Some("//COSMIC TOP SECRET//NOFORN"),
            "NATO TS banner must fold to COSMIC TOP SECRET with // prefix"
        );
    }

    #[test]
    fn try_nato_fold_banner_long_form_is_idempotent() {
        // `NATO SECRET//NOFORN` is already canonical banner form.
        // After FIX-1 the fold handles banner kind, but canonical inputs
        // must return None (idempotent) — otherwise every pass through the
        // decoder would fire the SupersededToken feature on already-canonical inputs.
        // `NATO SECRET` → `fold_nato_segment` → `banner_str() = "NATO SECRET"` = trimmed
        // → idempotence guard returns None → `any_changed = false` → outer None.
        assert!(
            try_nato_fold("NATO SECRET//NOFORN", MarkingType::Banner).is_none(),
            "canonical banner long-form must be idempotent (no fold needed)"
        );
        assert!(
            try_nato_fold("COSMIC TOP SECRET//NOFORN", MarkingType::Banner).is_none(),
            "canonical COSMIC TOP SECRET must be idempotent"
        );
    }

    #[test]
    fn try_nato_fold_banner_without_leading_slash_gets_slash_added() {
        // `NATO U` (bare, no trailing dissem) folds to `//NATO UNCLASSIFIED`
        assert_eq!(
            try_nato_fold("NATO U", MarkingType::Banner).as_deref(),
            Some("//NATO UNCLASSIFIED"),
            "banner NATO U must fold to //NATO UNCLASSIFIED"
        );
    }

    #[test]
    fn try_nato_fold_cab_kind_returns_none() {
        // CAB authority lines don't carry NATO classifications.
        assert!(
            try_nato_fold("NATO SECRET//NOFORN", MarkingType::Cab).is_none(),
            "Cab kind must always return None"
        );
    }

    #[test]
    fn fold_nato_segment_returns_none_for_atomal_compound() {
        // `NATO SECRET ATOMAL` is a legitimate compound the strict parser
        // canonicalizes (PR 9c.1 T134: bare `NatoSecret` class + AEA
        // `Atomal` companion). The fold MUST NOT fire on it — otherwise
        // the suffix gets truncated and recovery regresses.
        //
        // Regression guard for the FIX-A correctness fix in the PR 8
        // round-2 reviewer response. Citation: CAPCO-2016 §H.7 p122
        // (ATOMAL as AEA-axis marking, worked example).
        assert!(
            fold_nato_segment("NATO SECRET ATOMAL", MarkingType::Portion).is_none(),
            "fold must not fire on NATO SECRET ATOMAL (strict parser canonicalizes)"
        );
        assert!(
            fold_nato_segment("NATO CONFIDENTIAL ATOMAL", MarkingType::Portion).is_none(),
            "fold must not fire on NATO CONFIDENTIAL ATOMAL"
        );
        assert!(
            fold_nato_segment("NATO TOP SECRET ATOMAL", MarkingType::Portion).is_none(),
            "fold must not fire on NATO TOP SECRET ATOMAL"
        );
    }

    #[test]
    fn fold_nato_segment_returns_none_for_bohemia_balk() {
        // Hyphen-separated NATO compounds (`NATO TOP SECRET-BOHEMIA`,
        // `NATO TOP SECRET-BALK`) are also out of scope for the fold;
        // the strict parser canonicalizes them via PR 9c.1 T134 into
        // bare `CosmicTopSecret` class + SCI `NatoSap` companion
        // (CAPCO-2016 §G.2 p40 + §H.7 p127).
        //
        // Regression guard for FIX-A. Citation: CAPCO-2016 §G.2 p40 +
        // §H.7 p127.
        assert!(
            fold_nato_segment("NATO TOP SECRET-BOHEMIA", MarkingType::Portion).is_none(),
            "fold must not fire on NATO TOP SECRET-BOHEMIA (strict parser canonicalizes)"
        );
        assert!(
            fold_nato_segment("NATO TOP SECRET-BALK", MarkingType::Portion).is_none(),
            "fold must not fire on NATO TOP SECRET-BALK (strict parser canonicalizes)"
        );
    }
}
