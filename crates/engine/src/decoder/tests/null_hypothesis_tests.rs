// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Tests for `decoder/null_hypothesis.rs`. Carved into a parallel
//! file because the test surface (~440 lines) plus the production
//! body (549 lines) would push the combined file over the
//! 800-line gate. Reached from `null_hypothesis.rs` via
//! `#[path = "tests/null_hypothesis_tests.rs"]`.

use std::sync::LazyLock;

use marque_capco::{CapcoMarking, CapcoScheme};
use marque_core::Parser;
use marque_ism::{
    CapcoTokenSet, Classification, DissemControl, MarkingClassification,
    span::{MarkingCandidate, MarkingType, Span},
};
use marque_rules::recognition::FeatureId;
use marque_scheme::MarkingScheme;
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{LinePrefix, ParseContext, Recognizer};
use smallvec::SmallVec;

use super::*;
use crate::decoder::DecoderRecognizer;
use crate::decoder::test_helpers::{TEST_SCHEME, deep_cx};

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
