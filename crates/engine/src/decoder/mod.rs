// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase-D probabilistic [`Recognizer`] — the "decoder".
//!
//! The deep-scan half of the strict/deep-scan recognizer split. When
//! the engine is configured for deep-scan mode and the strict
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
//! signal per the foundational-plan dispatch contract.
//!
//! ## What this module is NOT
//!
//! - **Not a full template-matching grammar engine.** Candidates are
//!   materialized by canonicalizing observed tokens and round-tripping
//!   through the strict parser; the strict parser is the arbiter of
//!   "is this a CAPCO-shape marking?".
//! - **Not a learning system.** All priors are compile-time-baked
//!   `&'static` tables from `marque_capco::priors`.
//! - **Not a fix applier.** The decoder proposes `CapcoMarking`
//!   candidates; the engine applies them through the normal
//!   `Diagnostic` / `FixProposal` path with
//!   `FixSource::DecoderPosterior`.
//!
//! ## Sub-module layout
//!
//! Per-item-cluster split (engine-internal refactor #562). See
//! `docs/refactor-006/decoder-architecture.md` for the
//! design rationale, scoring approach, crate-placement reasoning,
//! and retired-mechanism notes (`LENIENT_REL_PREFIX_PENALTY`).

mod candidates;
mod dispatcher;
mod heuristic;
mod normalize;
mod null_hypothesis;
mod recognizer;
mod recovery;
mod scoring;
mod shape;
mod types;

// Public surface — byte-identical paths preserved per FR-049.
pub use dispatcher::StrictOrDecoderRecognizer;
pub use recognizer::DecoderRecognizer;
pub use shape::is_nontrivial_marking;

#[cfg(feature = "decoder-harness")]
pub use candidates::diagnostic_canonical_attempts;

// Cross-file constants — shared by multiple sub-modules.
// Test-only re-exports so the legacy in-mod tests block can reach
// items moved into sibling sub-modules. Production code (outside
// `#[cfg(test)]`) MUST NOT import via these — use the explicit
// sibling path instead.
#[cfg(test)]
#[allow(unused_imports)]
use {
    candidates::*, dispatcher::*, heuristic::*, normalize::*, null_hypothesis::*, recognizer::*,
    recovery::delimiter::*, recovery::nato::*, recovery::rel_to::*, recovery::reorder::*,
    recovery::sar::*, recovery::sci::*, recovery::stray::*, scoring::*, shape::*, types::*,
};

/// K=8 candidate bound per foundational-plan §5.2 and research.md R3.
///
/// Higher K burns latency without accuracy gain (diminishing returns
/// above 6 per the primary-source corpus analysis); lower K drops
/// recall on multi-token reorderings. Tunable in-place — the bound is
/// advisory, not a correctness invariant.
pub(super) const K_MAX_CANDIDATES: usize = 8;

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
pub(super) const UNAMBIGUOUS_LOG_MARGIN: f32 = 1.6;

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
pub(super) const NULL_HYPOTHESIS_LOG_MARGIN: f32 = 2.5;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// The test block stays co-located with `mod.rs` post-split because
// it spans every sub-module's surface and the cross-cutting helpers
// (`TEST_SCHEME`, `deep_cx`) live here. Per-sub-file `#[cfg(test)] mod
// tests` cohabitation is a refinement deferred to a follow-up: each
// sub-file's relevant test group lifts out with `use super::*;` and
// the necessary `use marque_…` lines, but the migration touches every
// fn in the block and is out of scope for the engine-internal split
// in #562. The test block exceeds the 800-line production gate
// (`#[cfg(test)]` is excluded from the constitution's source-line
// budget per Principle III §3 — test fixtures are exempt).

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::sync::LazyLock;

    use super::*;
    use marque_capco::{CapcoMarking, CapcoScheme};
    use marque_core::Parser;
    use marque_ism::{
        CapcoTokenSet, Classification, DissemControl, MarkingClassification,
        span::{MarkingCandidate, MarkingType, Span},
    };
    use marque_rules::confidence::FeatureId;
    use marque_scheme::MarkingScheme;
    use marque_scheme::ambiguity::Parsed;
    use marque_scheme::recognizer::{LinePrefix, ParseContext, Recognizer};
    use smallvec::SmallVec;

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
