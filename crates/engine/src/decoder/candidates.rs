//! Candidate byte generation — the master orchestrator.
//!
//! `generate_candidate_bytes` walks the input through every
//! preprocessing + recovery pass, emitting a bounded set of
//! canonical-byte attempts the strict parser can then parse.
//! `diagnostic_canonical_attempts` is the test-only feature-gated
//! accessor used by `tests/decoder_diagnostic.rs`.

use smallvec::SmallVec;

use super::K_MAX_CANDIDATES;
use super::heuristic::try_classification_heuristic_fix;
use super::normalize::{fuzzy_correct_tokens, normalize_delimiters_and_case};
use super::recovery::{
    try_add_non_us_prefix, try_canonical_reorder, try_collapse_stray_char_slash,
    try_insert_delimiter, try_nato_fold, try_rel_to_fuzzy_trigraph_candidates,
    try_rel_to_structural_repair, try_rel_to_usa_injection_candidates, try_sar_indicator_repair,
    try_sci_delimiter_repair,
};
use super::types::{CanonicalAttempt, FeatureEntry};

use marque_core::fuzzy::FuzzyVocabMatcher;
use marque_ism::{CapcoTokenSet, span::MarkingType, token_set::TokenSet as _};
use marque_rules::confidence::FeatureId;
use std::borrow::Cow;

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
pub(super) fn generate_candidate_bytes(bytes: &[u8]) -> SmallVec<[CanonicalAttempt; 4]> {
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
