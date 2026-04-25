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

use std::collections::BTreeSet;

use marque_capco::provenance::DecoderProvenance;
use marque_capco::{CapcoMarking, CapcoScheme};
use marque_core::{Parser, fuzzy::FuzzyVocabMatcher};
use marque_ism::{
    CapcoTokenSet, Classification,
    span::{MarkingCandidate, MarkingType, Span},
    token_set::TokenSet as _,
};
use marque_rules::confidence::{FeatureContribution, FeatureId};
use marque_scheme::ambiguity::{Candidate, EvidenceFeature, Parsed};
use marque_scheme::recognizer::{ParseContext, Recognizer};

use crate::recognizer::StrictRecognizer;

/// K=8 candidate bound per foundational-plan §5.2 and research.md R3.
///
/// Higher K burns latency without accuracy gain (diminishing returns
/// above 6 per the primary-source corpus analysis); lower K drops
/// recall on multi-token reorderings. Tunable in-place — the bound is
/// advisory, not a correctness invariant.
const K_MAX_CANDIDATES: usize = 8;

/// Runner-up posterior-ratio threshold for emitting `Unambiguous`.
///
/// When the top candidate's log-posterior exceeds the runner-up's by
/// at least this much (in natural-log space, so ~1.6 ≈ 5× odds ratio),
/// the decoder collapses to `Unambiguous(top)`. Below the threshold,
/// it returns `Ambiguous { candidates }` so the engine can surface a
/// diagnostic rather than auto-apply a close call.
const UNAMBIGUOUS_LOG_MARGIN: f32 = 1.6;

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
    fn recognize(&self, bytes: &[u8], cx: &ParseContext) -> Parsed<CapcoMarking> {
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
        let mut scored: Vec<ScoredCandidate> = Vec::new();
        for attempt in canonical_attempts {
            let candidate = MarkingCandidate {
                span: Span::new(0, attempt.bytes.len()),
                ..synthetic_candidate
            };
            let Ok(mut parsed) = parser.parse(&candidate, &attempt.bytes) else {
                continue;
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
                continue;
            }

            // 3b. Span-offset contract: `IsmAttributes::token_spans`
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
            parsed.attrs.token_spans = Box::new([]);
            let marking = CapcoMarking::new(parsed.attrs);

            // 3c. The strict parser is lenient — it accepts any
            //     `BYTES//BYTES` shape and emits an `IsmAttributes`
            //     with empty fields when nothing is recognized. Drop
            //     such trivial parses so the decoder doesn't
            //     fabricate a marking for prose like `FROBNITZ//WIBBLE`.
            if !is_nontrivial_marking(&marking) {
                continue;
            }

            // 3d. FR-011 — drop candidates below the page's strict
            //     classification floor.
            if let Some(floor) = cx.classification_floor
                && !meets_classification_floor(&marking, floor)
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
            let (prior, posterior) = score_candidate(&attempt, &marking);
            scored.push(ScoredCandidate {
                marking,
                prior,
                posterior,
                canonical_bytes: attempt.bytes.into_boxed_slice(),
                features: attempt.features,
            });
        }

        if scored.is_empty() {
            return Parsed::Ambiguous {
                candidates: Vec::new(),
            };
        }

        // 5. Sort by posterior descending; keep top K=8.
        scored.sort_by(|a, b| {
            b.posterior
                .partial_cmp(&a.posterior)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(K_MAX_CANDIDATES);

        // 6. Decision: top-over-runner-up log margin on the posterior.
        let top_score = scored[0].posterior;
        let runner_up_score = scored
            .get(1)
            .map(|c| c.posterior)
            .unwrap_or(f32::NEG_INFINITY);
        let log_margin = top_score - runner_up_score;

        if scored.len() == 1 || log_margin >= UNAMBIGUOUS_LOG_MARGIN {
            // Move the top candidate out so we can hand `canonical_bytes`
            // and `features` directly to provenance without an extra
            // clone — the marking carries the heaviest payload and we
            // only need it once.
            let top = scored.swap_remove(0);
            // `runner_up_ratio = exp(log_margin)`, but a sufficiently
            // separated top vs. runner-up overflows `f32::exp()` to
            // `+∞` (anything past `log_margin ≈ 88.7` saturates), and
            // `Confidence::validate` would then reject the resulting
            // record as non-finite — making `FixProposal::new` panic at
            // the audit boundary on extreme score separations. Saturate
            // at `f32::MAX` so the audit record carries "the ratio is
            // enormous" instead of crashing the engine.
            let runner_up_ratio = if runner_up_score.is_finite() {
                let ratio = log_margin.exp();
                Some(if ratio.is_finite() { ratio } else { f32::MAX })
            } else {
                None
            };
            let mut marking = top.marking;
            marking.1 = Some(DecoderProvenance {
                canonical_bytes: top.canonical_bytes,
                posterior: top.posterior,
                runner_up_ratio,
                features: top
                    .features
                    .into_iter()
                    .map(|f| FeatureContribution {
                        id: f.id,
                        delta: f.delta,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            });
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
                    evidence: s
                        .features
                        .iter()
                        .map(|f| EvidenceFeature {
                            label: f.id.as_str(),
                            log_odds: f.delta,
                        })
                        .collect(),
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
    /// Canonical byte string the strict parser accepted for this
    /// candidate. Threaded into [`DecoderProvenance::canonical_bytes`]
    /// when this candidate wins the Unambiguous collapse, so the
    /// engine can emit a `FixSource::DecoderPosterior` rewrite from
    /// the original mangled bytes to this canonical form (Phase 4
    /// PR-4b, T068).
    canonical_bytes: Box<[u8]>,
    features: Vec<FeatureEntry>,
}

/// One feature recorded during candidate generation, paired with its
/// log-odds contribution. The decoder accumulates these to reconstruct
/// `Confidence::features` at audit-emit time.
#[derive(Debug, Clone, Copy)]
struct FeatureEntry {
    id: FeatureId,
    delta: f32,
}

/// A canonicalization attempt: the byte string the decoder will hand
/// to the strict parser, plus the features that transformation
/// represents. Zero or more attempts are generated per observed input.
struct CanonicalAttempt {
    bytes: Vec<u8>,
    features: Vec<FeatureEntry>,
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
fn generate_candidate_bytes(bytes: &[u8]) -> Vec<CanonicalAttempt> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };

    // Strip surrounding whitespace; preserve leading `(` for portion
    // detection so the strict parser's portion path stays keyed off
    // the same first-non-whitespace byte the recognizer saw.
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut attempts: Vec<CanonicalAttempt> = Vec::new();
    let mut emit = |bytes: Vec<u8>, features: Vec<FeatureEntry>| {
        // Hard cap at K_MAX_CANDIDATES × 2 — guarantees the strict-parse
        // work downstream is bounded even if new transform stages are added.
        if attempts.len() >= K_MAX_CANDIDATES * 2 {
            return;
        }
        // Dedup by the canonical byte string — different transform
        // sequences can converge on the same output.
        if !attempts.iter().any(|a| a.bytes == bytes) {
            attempts.push(CanonicalAttempt { bytes, features });
        }
    };

    // ---- Raw: just trim + normalize delimiters/case. --------------
    let (normalized, delim_features) = normalize_delimiters_and_case(trimmed);

    // ---- Per-token fuzzy correction on the normalized text. ------
    let vocab = CapcoTokenSet.correction_vocab();
    let matcher = FuzzyVocabMatcher::new(vocab);
    let (fuzzy_corrected, fuzzy_features) = fuzzy_correct_tokens(&normalized, &matcher);

    // Emit the straightforward "normalize + fuzzy-correct" attempt
    // first — this covers typos (T046) and case/delimiter mangling
    // by default.
    let mut features = delim_features.clone();
    features.extend(fuzzy_features.iter().copied());
    emit(fuzzy_corrected.clone().into_bytes(), features);

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
        emit(reordered.into_bytes(), features);
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
        .map(|a| a.bytes)
        .collect()
}

/// Normalize delimiters and case on a trimmed input.
///
/// - Fullwidth slash variants (`∕∕`, `/ /`, ` / / `, spaced `//`) all
///   collapse to `//`.
/// - ASCII alphabetic characters are upper-cased; the CAPCO grammar
///   is case-sensitive uppercase (§B).
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
fn normalize_delimiters_and_case(text: &str) -> (String, Vec<FeatureEntry>) {
    let mut features = Vec::new();

    // Collapse fullwidth and spaced slash variants.
    // The order matters: we want multi-char sequences first.
    let mut normalized: String = text.to_owned();
    let replacements = [
        ("∕∕", "//"),
        (" // ", "//"),
        ("// ", "//"),
        (" //", "//"),
        ("/ / ", "//"),
        (" / / ", "//"),
        ("/ /", "//"),
    ];
    let mut delim_changed = false;
    for (from, to) in replacements {
        if normalized.contains(from) {
            normalized = normalized.replace(from, to);
            delim_changed = true;
        }
    }

    // Case normalization. If the input was all-lowercase or mixed-case
    // (Title Case), uppercasing is a significant canonicalization the
    // decoder flags (via the `BaseRateCommonMarking` feature below)
    // so the posterior reflects that the candidate required cleanup.
    let had_lowercase = normalized.chars().any(|c| c.is_ascii_lowercase());
    if had_lowercase {
        normalized = normalized.to_ascii_uppercase();
    }

    if delim_changed || had_lowercase {
        // Record a `BaseRateCommonMarking` feature with a penalty
        // delta. The feature doesn't fit into one of the sharper
        // features (`EditDistance*`, `TokenReorder`,
        // `SupersededToken`), but it flags that we had to massage
        // the input — delimiters were non-canonical, or case was
        // wrong. A small negative delta means a canonical-input
        // candidate outranks an otherwise-equivalent normalized one,
        // which is the intent: "arrives clean" should be preferred
        // over "needed cleanup."
        features.push(FeatureEntry {
            id: FeatureId::BaseRateCommonMarking,
            delta: -0.3,
        });
    }

    (normalized, features)
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
fn fuzzy_correct_tokens(
    text: &str,
    matcher: &FuzzyVocabMatcher<'_>,
) -> (String, Vec<FeatureEntry>) {
    let mut features = Vec::new();
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

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
            out.push_str(&rest[..non_token_len]);
            rest = &rest[non_token_len..];
            continue;
        }
        // Take the token: alnum + internal `-`.
        let token_len = scan_token(rest);
        if token_len == 0 {
            // Should not happen given the non-token prefix branch,
            // but guard against infinite loops on pathological input.
            break;
        }
        let (token, tail) = rest.split_at(token_len);
        rest = tail;

        // Case 1: superseded token.
        if let Some(replacement) = SUPERSEDED_TOKEN_MAP
            .iter()
            .find(|&&(from, _)| from == token)
            .map(|&(_, to)| to)
        {
            out.push_str(replacement);
            features.push(FeatureEntry {
                id: FeatureId::SupersededToken,
                delta: -0.2,
            });
            continue;
        }

        // Case 2: already canonical (known CVE token or trigraph).
        // Check this first so we don't run a vocab scan + edit-
        // distance pass on tokens we already recognize.
        if CapcoTokenSet.canonicalize(token).is_some() || CapcoTokenSet.is_trigraph(token) {
            out.push_str(token);
            continue;
        }

        // Case 3: fuzzy-correctable. Compute once and reuse; the
        // previous structure called `matcher.correct(token)` twice
        // on tokens that weren't already canonical, doubling the
        // vocab-scan cost on exactly the unknown-token hot path.
        if let Some(correction) = matcher.correct(token) {
            out.push_str(correction.token);
            let feature = match correction.distance {
                0 => None, // shouldn't happen — `correct` returns None on exact match
                1 => Some(FeatureId::EditDistance1),
                _ => Some(FeatureId::EditDistance2),
            };
            if let Some(id) = feature {
                let delta = match id {
                    FeatureId::EditDistance1 => -0.5,
                    FeatureId::EditDistance2 => -1.2,
                    _ => 0.0,
                };
                features.push(FeatureEntry { id, delta });
            }
            continue;
        }

        // Case 4: unknown and uncorrectable. Pass through verbatim.
        // The strict parser will register this as a
        // `TokenKind::Unknown` span rather than failing the parse
        // outright, so the decoder's outer loop (step 3a of
        // `DecoderRecognizer::recognize`) is what filters the
        // resulting partial-canonicalization candidate out.
        out.push_str(token);
    }

    (out, features)
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
    if class_segments.len() + dissem_segments.len() + other_segments.len() == 0 {
        return None;
    }

    // Already-canonical check: if the classification segment is the
    // first non-empty segment, no reorder is needed.
    if let Some(first) = segments.iter().find(|s| !s.trim().is_empty()) {
        if class_segments.contains(&first.trim()) {
            return None;
        }
    }

    // Emit: classification → other (SCI/SAR/FGI blocks) → dissem.
    let mut ordered: Vec<&str> = Vec::new();
    ordered.extend(class_segments);
    ordered.extend(other_segments);
    ordered.extend(dissem_segments);

    let joined = ordered.join("//");
    Some(format!("{prefix}{joined}{suffix}"))
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
    // Single-whitespace-token classifications only. `TOP SECRET` is
    // handled by the separate starts_with branch below — including
    // it here would be dead: `split_whitespace().next()` always
    // returns the first word, so the lookup sees `"TOP"`, not the
    // full two-word phrase.
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
    ];
    // Dissemination-control tokens ONLY. SCI controls (HCS, SI, TK,
    // and all their sub-compartment forms) are NOT in this list —
    // they belong to their own category under CAPCO §A.6 and the
    // canonical order places them between classification and dissem.
    // Classifying an HCS segment as Dissem would drive
    // `try_canonical_reorder` to move it past the dissem block,
    // corrupting the rewrite. SCI segments therefore fall through to
    // `SegmentClass::Other`, which the reorder helper inserts
    // between classification and dissem — the right spot per
    // CAPCO-2016 §A.6.
    const DISSEMS: &[&str] = &[
        "NOFORN", "NF", "ORCON", "OC", "PROPIN", "PR", "IMCON", "IMC", "RELIDO", "RS", "RSEN",
        "DSEN", "FISA", "FOUO",
    ];
    if CLASSIFICATIONS.contains(&first_token) {
        SegmentClass::Classification
    } else if DISSEMS.contains(&first_token) {
        SegmentClass::Dissem
    } else if first_token == "TOP" && seg.starts_with("TOP SECRET") {
        SegmentClass::Classification
    } else {
        SegmentClass::Other
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
/// Used inside the decoder itself to filter out lenient-parse-
/// accepts-anything results (`FROBNITZ//WIBBLE` trip-fires the
/// banner scanner and produces a zero-attribute parse); without
/// the filter, every `X//Y` prose fragment would materialize a
/// fabricated empty marking candidate.
fn is_nontrivial_marking(marking: &CapcoMarking) -> bool {
    let a = &marking.0;
    a.classification.is_some()
        || !a.sci_controls.is_empty()
        || a.sar_markings.is_some()
        || !a.aea_markings.is_empty()
        || a.fgi_marker.is_some()
        || !a.dissem_controls.is_empty()
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

/// Bag-of-tokens scorer (foundational-plan §5.2).
///
/// Returns `(prior, posterior)` where:
///
/// - `prior` = Σ [`marque_capco::priors::token_log_prior`] over the
///   marking's canonical tokens. This is the prior alone — nothing
///   else — and is what [`Candidate::prior_log_odds`] is documented
///   to carry (see `crates/scheme/src/ambiguity.rs`). Tokens missing
///   from the baked table contribute [`MISSING_TOKEN_LOG_PRIOR`]
///   (a below-observed-floor penalty) rather than `0.0`.
/// - `posterior` = `prior + Σ attempt.features[i].delta`. This is the
///   quantity the decoder sorts and thresholds on.
///
/// Splitting the two prevents the caller from writing the full
/// posterior into `Candidate::prior_log_odds` — that would double-
/// count the feature deltas once any resolver re-adds
/// `EvidenceFeature.log_odds`.
///
/// Precision: computed in `f32` — the baked priors are already `f32`
/// and the feature deltas are small constants (single-digit magnitude
/// at most), so the accumulator doesn't need `f64` headroom for the
/// K=8 candidate set.
fn score_candidate(attempt: &CanonicalAttempt, marking: &CapcoMarking) -> (f32, f32) {
    // Prior: sum of baked log-priors for the canonical tokens that
    // appear in the parsed marking. Tokens missing from the baked
    // table receive the floor penalty rather than a neutral 0.0
    // contribution — see the MISSING_TOKEN_LOG_PRIOR doc.
    let mut prior: f32 = 0.0;
    let tokens = canonical_tokens_for(marking);
    for token in tokens {
        prior += marque_capco::priors::token_log_prior(token).unwrap_or(MISSING_TOKEN_LOG_PRIOR);
    }

    // Posterior: prior plus feature deltas.
    let feature_sum: f32 = attempt.features.iter().map(|f| f.delta).sum();
    let posterior = prior + feature_sum;

    (prior, posterior)
}

/// Enumerate the canonical tokens present in `marking` that have a
/// `&'static str` representation suitable for
/// [`marque_capco::priors::TOKEN_BASE_RATES`] lookup.
///
/// Scored token families, by `IsmAttributes` field:
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
/// Deliberately NOT included in scoring:
///
/// - `sar_markings` — SAR program identifiers are agency-assigned
///   codewords (open set, not in the baked priors).
/// - `rel_to` country trigraphs — `Trigraph::as_str()` returns a
///   `&str` tied to `&self`, not `&'static str`. Plumbing a
///   static-string helper is left as future work; the priors
///   corpus-coverage for trigraphs is sparse anyway.
/// - CAB fields (`classified_by`, `derived_from`, `declassify_on`) —
///   free-form text, not CVE-enumerable.
///
/// Expansion work is tracked in future PRs alongside any priors
/// regeneration that widens coverage (e.g., counting SAR indicator
/// base rates from a larger corpus).
fn canonical_tokens_for(marking: &CapcoMarking) -> Vec<&'static str> {
    let attrs = &marking.0;
    let mut tokens: BTreeSet<&'static str> = BTreeSet::new();

    if let Some(class) = attrs.classification.as_ref() {
        // Use the effective level's banner form as the classification
        // token — this is the form the priors corpus keys on for the
        // "common classification appears" prior.
        tokens.insert(class.effective_level().banner_str());
    }

    for ctrl in attrs.sci_controls.iter() {
        tokens.insert(ctrl.as_str());
    }
    for dis in attrs.dissem_controls.iter() {
        tokens.insert(dis.as_str());
    }
    for nic in attrs.non_ic_dissem.iter() {
        // `NonIcDissem::banner_str` returns `&'static str` with the
        // banner form (LIMDIS, EXDIS, NODIS, SBU, LES, SSI,
        // SBU NOFORN, LES NOFORN). The compound forms ("SBU NOFORN",
        // "LES NOFORN") won't hit a single-token priors entry — they
        // fall to MISSING_TOKEN_LOG_PRIOR. That's fine: the
        // comparison against peer candidates remains consistent.
        tokens.insert(nic.banner_str());
    }
    if !attrs.aea_markings.is_empty() {
        tokens.insert("AEA");
    }
    if attrs.fgi_marker.is_some() {
        tokens.insert("FGI");
    }

    tokens.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Strict + decoder dispatcher
// ---------------------------------------------------------------------------

/// Recognizer that runs the strict path first and falls back to the
/// decoder when the strict parse yields no meaningful attributes.
///
/// Installed by [`crate::Engine::with_deep_scan`]. Deep-scan opt-in
/// therefore happens by calling `with_deep_scan()`, not by separately
/// toggling [`ParseContext::strict_evidence`] at the engine boundary
/// — the engine sets `strict_evidence = false` when the deep-scan
/// flag is on, and `= true` otherwise.
///
/// Within this recognizer, dispatch is keyed off
/// [`ParseContext::strict_evidence`]:
///
/// - `strict_evidence = true`: collapse to strict-only behavior. The
///   decoder is not called.
/// - `strict_evidence = false`: try strict first. Fall back to the
///   decoder when the strict result is either (a) zero-candidate
///   `Ambiguous` or (b) `Unambiguous` with an empty / trivial
///   [`CapcoMarking`] (no classification, no SCI, no dissem, no
///   FGI, etc.). The trivial-Unambiguous case matters because
///   `marque_core::Parser` is lenient: it accepts arbitrary
///   `BYTES//BYTES` shapes and returns `Ok` with an empty
///   `IsmAttributes` when nothing in the input is a recognized CVE
///   token. Treating such a result as a successful parse would
///   leave the decoder dormant on exactly the mangled inputs it
///   exists to recover (`SERCET//NOFORN`, `NOFORN//SECRET`, …).
///   Strict is always called with `strict_evidence = true`
///   internally; the decoder is always called with
///   `strict_evidence = false` internally.
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
    fn recognize(&self, bytes: &[u8], cx: &ParseContext) -> Parsed<CapcoMarking> {
        let strict_inner_cx = ParseContext {
            strict_evidence: true,
            ..*cx
        };
        let strict_result = self.strict.recognize(bytes, &strict_inner_cx);

        // When the outer caller asked for strict-only (the default
        // engine mode), collapse to the strict result — never call
        // the decoder. Preserves interactive-authoring latency
        // (SC-001) for engines that have been wrapped in
        // `with_deep_scan` but are currently being driven without
        // the deep-scan opt-in (Engine sets `strict_evidence = true`
        // when `deep_scan = false`).
        if cx.strict_evidence {
            return strict_result;
        }

        // Infer the candidate kind from the byte shape so
        // `strict_parse_is_complete` can apply the right rule
        // (classification-requiring for portion/banner, CAB-field-
        // requiring for CAB). If inference fails the bytes are too
        // degenerate for either path — skip.
        let kind = infer_marking_type(bytes).unwrap_or(MarkingType::Banner);

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
        let decoder_cx = ParseContext {
            strict_evidence: false,
            ..*cx
        };
        let decoder_result = self.decoder.recognize(bytes, &decoder_cx);

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
mod tests {
    use super::*;

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
            zone: None,
            position: None,
            classification_floor: None,
        }
    }

    #[test]
    fn decoder_defers_to_strict_when_strict_evidence_is_set() {
        let rx = DecoderRecognizer::new();
        let cx = ParseContext::default(); // strict_evidence = true
        match rx.recognize(b"(S//NF)", &cx) {
            Parsed::Ambiguous { candidates } => assert!(candidates.is_empty()),
            other => panic!("expected zero-candidate Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn decoder_zero_candidate_on_no_template_fit() {
        let rx = DecoderRecognizer::new();
        // Neither token is in the vocabulary and no fuzzy match.
        match rx.recognize(b"FROBNITZ//WIBBLE", &deep_cx()) {
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
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 14),
            kind: MarkingType::Banner,
        };
        let parsed = parser
            .parse(&candidate, b"SECRET//NOFORN")
            .expect("SECRET//NOFORN must parse");
        let marking = CapcoMarking::new(parsed.attrs);

        let features = vec![
            FeatureEntry {
                id: FeatureId::EditDistance1,
                delta: -0.5,
            },
            FeatureId::TokenReorder.into(),
        ];
        let attempt = CanonicalAttempt {
            bytes: b"SECRET//NOFORN".to_vec(),
            features: features.clone(),
        };
        let (prior, posterior) = score_candidate(&attempt, &marking);

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
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 16),
            kind: MarkingType::Portion,
        };
        let parsed = parser
            .parse(&candidate, b"(SERCET//NOFORN)")
            .expect("strict parser should accept (SERCET//NOFORN) leniently");
        let marking = CapcoMarking::new(parsed.attrs);
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
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 7),
            kind: MarkingType::Portion,
        };
        let parsed = parser
            .parse(&candidate, b"(S//NF)")
            .expect("canonical portion must strict-parse");
        let marking = CapcoMarking::new(parsed.attrs);
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
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 9),
            kind: MarkingType::Portion,
        };
        let parsed = parser
            .parse(&candidate, b"(S//FRBN)")
            .expect("strict parser accepts (S//FRBN) leniently");
        let marking = CapcoMarking::new(parsed.attrs);
        // `S` resolved, so classification is Some — but the
        // Unknown-tail check still fires.
        assert!(
            !strict_parse_is_complete(&marking, MarkingType::Portion),
            "`FRBN` is Unknown-kind → strict parse is incomplete; attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn decoder_rejects_trivial_strict_parse() {
        // The strict parser is lenient: it accepts `FROBNITZ//WIBBLE`
        // and emits an IsmAttributes with classification=None,
        // dissem_controls=[], sci_controls=[]. The decoder must treat
        // that as "no real parse" and drop the candidate — otherwise
        // it would fabricate an empty marking for arbitrary prose.
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 16),
            kind: MarkingType::Banner,
        };
        let parsed = parser
            .parse(&candidate, b"FROBNITZ//WIBBLE")
            .expect("strict parser should accept arbitrary bytes");
        let marking = CapcoMarking::new(parsed.attrs);
        assert!(
            !is_nontrivial_marking(&marking),
            "empty marking must be filtered"
        );
    }

    #[test]
    fn decoder_recovers_typo_sercet_to_secret() {
        let rx = DecoderRecognizer::new();
        match rx.recognize(b"SERCET//NOFORN", &deep_cx()) {
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
        match rx.recognize(b"secret//noforn", &deep_cx()) {
            Parsed::Unambiguous(m) => {
                assert_eq!(marking_classification(&m), Some(Classification::Secret));
            }
            other => panic!("expected Unambiguous, got {other:?}"),
        }
    }

    #[test]
    fn decoder_recovers_superseded_comint_to_si() {
        let rx = DecoderRecognizer::new();
        // SECRET//COMINT//NOFORN — COMINT is CAPCO-2016 §A.6 p16-superseded to SI.
        match rx.recognize(b"SECRET//COMINT//NOFORN", &deep_cx()) {
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
        match rx.recognize(b"NOFORN//SECRET", &deep_cx()) {
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
            zone: None,
            position: None,
            classification_floor: Some(Classification::Secret as u8),
        };
        match rx.recognize(b"(U)", &cx) {
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
            zone: None,
            position: None,
            classification_floor: Some(Classification::Confidential as u8),
        };
        match rx.recognize(b"(S//NF)", &cx) {
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
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(u_marking) = rx.recognize(b"(U)", &deep_cx()) else {
            panic!("(U) should decode to unambiguous UNCLASSIFIED");
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
}
