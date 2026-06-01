//! Position-aware short-token classification heuristic.
//!
//! Distinct from the structural recovery passes — this is a
//! "guess the classification" helper that fires only on the leading
//! token of an input. It exists because `MIN_FUZZY_LEN = 3` blocks the
//! vocab-based fuzzy matcher from operating on 1- and 2-character
//! tokens, but the leading classification slot of a portion or banner
//! gives enough positional evidence to recover keyboard-adjacent
//! typos at high (but not certain) confidence.

use super::shape::is_cab_head;

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
/// # Recognition
///
/// The decoder tags this attempt's `CanonicalAttempt::fix_source`
/// with [`marque_rules::FixSource::DecoderClassificationHeuristic`].
/// The engine then (a) downgrades the diagnostic severity to
/// [`Severity::Warn`](marque_rules::Severity::Warn) — always-visible
/// in `--check`, exits non-zero — and (b) caps the sole surviving
/// `recognition` axis at `HEURISTIC_RECOGNITION_CAP = 0.95`, exactly
/// the default `confidence_threshold`, so a single-candidate
/// heuristic fix lands at-threshold rather than saturating above
/// it. See `marque_capco::HEURISTIC_RECOGNITION_CAP`
/// for the cap's authoritative doc.
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
/// **Length 3** — exactly one mapping:
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
/// - `TP` → `TOP` (corpus-attested keyboard typo where the middle
///   `O` was elided; bare `TP` has no other canonical CAPCO meaning).
/// - `TO` → `TOP` (same family — trailing `P` elided).
///
/// **Length 1**:
/// - `[A, W, E, Z]` → `S` (S-key neighbors; bare `S` is canonical)
/// - `[V, F]` → `C` (C-key neighbors; bare `C` is canonical)
/// - `[X]` → `S` (X is between C and S on QWERTY; default to the
///   higher classification per the design note)
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
/// `TOP SECRET` classification. The canonical short-circuit set
/// includes `TOP` because the length-3 `OTP→TOP` heuristic would
/// otherwise walk the heuristic path on every already-canonical
/// `TOP SECRET//...` input. The strict parser already accepts all
/// of these. See [`is_canonical_short_classification`] for the
/// implementation.
///
/// # CAB markings
///
/// Returns `None` when `text` looks like a CAB (Classification
/// Authority Block) — those are keyed authority lines, not
/// classification-leading shapes, and the heuristic would emit
/// nonsense if applied. The check mirrors `shape::is_cab_head`.
pub(super) fn try_classification_heuristic_fix(
    text: &str,
    input_source: marque_scheme::InputSource,
) -> Option<String> {
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

    // **Lone-input safety guard.** Skip the
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
    // Form-field input (`(YS)` typed into a portion-mark field) SHOULD
    // heuristic-fix at high confidence — a trusted caller asserts via
    // [`InputSource::StructuredField`] that the input IS a marking
    // attempt, so the lone-case false-positive concern (parenthetical
    // refs in prose) does not apply. The guard below is therefore
    // **conditional on [`InputSource::DocumentContent`]** (#176 / SC-010):
    // only document-content input takes the conservative lone-case path.
    // `StructuredField` skips the guard so a lone `(YS)` reaches the
    // heuristic. (`SchemaDocument` never reaches the decoder — adapters
    // own that path — so it falls in with the conservative default here
    // harmlessly.)
    //
    // Trailing whitespace doesn't count as "other content" — `(YS )` is
    // functionally equivalent to `(YS)` for the lone-case test.
    if input_source == marque_scheme::InputSource::DocumentContent {
        let has_other_marking_content = after_first_token.chars().any(|c| !c.is_whitespace())
            || after_first_seg.chars().any(|c| !c.is_whitespace());
        if !has_other_marking_content {
            return None;
        }
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
/// `TOP` is in the match set because the helper's whitespace
/// tokenizer treats `TOP` as a non-canonical token; without the
/// short-circuit the heuristic would fire on perfectly-canonical
/// `TOP SECRET//...` input — a no-op when the heuristic returned
/// `None` for length-3 inputs, but a latent footgun once the
/// length-3 arm starts returning `Some`. Recognizing bare
/// `TOP` as canonical short-circuits the heuristic on the
/// already-correct case.
fn is_canonical_short_classification(token: &str) -> bool {
    matches!(token, "U" | "R" | "C" | "S" | "TS" | "TOP")
}

/// 2-char keyboard-proximity rule. Two mappings:
///
/// 1. T-cluster + S-cluster pair → `TS` (the original rule).
/// 2. Specific `TP` / `TO` pair → `TOP`. These are corpus-attested
///    classification typos where the middle `O` (`TP`) or trailing
///    `P` (`TO`) was elided. Bare `TP` and `TO` have no other
///    canonical CAPCO meaning at the leading classification position
///    — `TP` isn't an SCI control or dissem, `TO` isn't either (the
///    `REL TO` keyword path lives inside the structural REL TO
///    parser, not here).
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

    // `TP` / `TO` → `TOP`. Tight pattern (literal pair, not
    // cluster) because broadening to e.g. `T[A-Z]` → `TOP` would
    // collide with too many real 2-char tokens in non-marking
    // prose. Anchored to T as the first byte and P / O as the
    // second.
    if first == b'T' && matches!(second, b'P' | b'O') {
        return Some("TOP");
    }

    None
}

/// 3-char keyboard-proximity rule. Maps a small
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
        // classification (S) per the design note —
        // false-negative cost (under-classified) > false-positive
        // cost (over-classified) for IC compliance work.
        b'X' => Some("S"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(unused_imports)]
mod tests {
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

    // #176 / SC-010 input-source aliases for the heuristic tests. `DC`
    // (DocumentContent) keeps the conservative lone-case guard; `SF`
    // (StructuredField) lifts it — a trusted caller asserting the input
    // IS a marking-shaped field.
    const DC: marque_scheme::InputSource = marque_scheme::InputSource::DocumentContent;
    const SF: marque_scheme::InputSource = marque_scheme::InputSource::StructuredField;

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
        assert_eq!(try_classification_heuristic_fix("(S//NF)", DC), None);
        assert_eq!(try_classification_heuristic_fix("(TS//NF)", DC), None);
        assert_eq!(try_classification_heuristic_fix("(C//NF)", DC), None);
        assert_eq!(try_classification_heuristic_fix("SECRET//NOFORN", DC), None);
    }

    #[test]
    fn heuristic_fixes_portion_form() {
        assert_eq!(
            try_classification_heuristic_fix("(YS//NF)", DC).as_deref(),
            Some("(TS//NF)")
        );
        assert_eq!(
            try_classification_heuristic_fix("(W//NF)", DC).as_deref(),
            Some("(S//NF)")
        );
        assert_eq!(
            try_classification_heuristic_fix("(F//NF)", DC).as_deref(),
            Some("(C//NF)")
        );
        // Lowercase first token (inside parens).
        assert_eq!(
            try_classification_heuristic_fix("(ys//NF)", DC).as_deref(),
            Some("(TS//NF)")
        );
    }

    #[test]
    fn heuristic_fixes_banner_form() {
        // Banner shapes don't have parens but otherwise behave the
        // same — leading classification token in the first segment.
        assert_eq!(
            try_classification_heuristic_fix("RS//NOFORN", DC).as_deref(),
            Some("TS//NOFORN")
        );
        assert_eq!(
            try_classification_heuristic_fix("X//NOFORN", DC).as_deref(),
            Some("S//NOFORN")
        );
    }

    #[test]
    fn heuristic_skips_cab_shape() {
        // CAB lines don't have a leading classification token. The
        // `is_cab_head` short-circuit at the top of the helper should
        // catch every CAB-keyword prefix.
        assert_eq!(
            try_classification_heuristic_fix("Classified By: foo", DC),
            None
        );
        assert_eq!(
            try_classification_heuristic_fix("Derived From: bar", DC),
            None
        );
        assert_eq!(
            try_classification_heuristic_fix("Declassify On: baz", DC),
            None
        );
    }

    #[test]
    fn heuristic_skips_long_token() {
        // 4+ char tokens fall through the length match arm — the
        // vocab fuzzy path handles them. 3-char tokens are mostly
        // handled by the vocab path too (bare `TOP` is in
        // `EXTENDED_CORRECTION_VOCAB`, so shapes like `TPP` and
        // `UOP` correct via dist-1 fuzzy); the 3-char heuristic is
        // intentionally narrow (only `OTP` → `TOP`) so unrelated
        // 3-char tokens like `YES` return None.
        assert_eq!(try_classification_heuristic_fix("(YES//NF)", DC), None);
        assert_eq!(try_classification_heuristic_fix("(SECT//NF)", DC), None);
        assert_eq!(try_classification_heuristic_fix("SECRET//NOFORN", DC), None);
    }

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
            let result = try_classification_heuristic_fix(input, DC);
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

    #[test]
    fn heuristic_recovers_tp_and_to_to_top_via_2char_rule() {
        // The 2-char heuristic also maps `TP`/`TO` → `TOP`. These are
        // corpus-attested classification typos where the middle `O`
        // (`TP`) or trailing `P` (`TO`) was elided. They must not
        // collide with the TS rule because neither `P` nor `O` is in
        // the S-cluster.
        let cases: &[(&str, &str)] = &[
            ("TP SECRET//NOFORN", "TOP SECRET//NOFORN"),
            ("TO SECRET//NOFORN", "TOP SECRET//NOFORN"),
            ("(TP//NF)", "(TOP//NF)"),
            ("(TO//NF)", "(TOP//NF)"),
        ];
        for (input, expected) in cases {
            let result = try_classification_heuristic_fix(input, DC);
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
        // Bare `TOP` belongs in the canonical-short set so the
        // classification heuristic doesn't fire on already-canonical
        // `TOP SECRET//...` input (whose first whitespace-token is
        // `TOP`). Without `TOP` in the set the length-3 heuristic
        // (`OTP → TOP`) would re-fire on canonical input.
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
        assert_eq!(try_classification_heuristic_fix("(B//NF)", DC), None);
        assert_eq!(try_classification_heuristic_fix("(QS//NF)", DC), None);
    }

    #[test]
    fn heuristic_skips_lone_inputs() {
        // Issues #133 / #176 lone-input safety guard. The heuristic
        // must NOT fire on inputs without marking-shape signals
        // beyond the leading token — auto-applying lone-case fixes
        // would surface as false positives on parenthetical refs
        // like `(A)`, `(W)`, `(F)` that are common in business
        // prose. The #133 corpus measurement found `A` alone has
        // 214,539 unrestricted body-text occurrences in the Enron
        // corpus vs 168 in marking-context — the lone-case FP rate
        // is ~3 orders of magnitude higher than the in-context rate.
        //
        // Form-field input (caller asserts the input IS a marking
        // attempt) DOES fire — the guard is now conditional on
        // `InputSource::DocumentContent` (#176 / SC-010). This test pins
        // the document-content branch; the StructuredField branch is
        // pinned by `sc010_input_source_confidence_matrix` below.
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
                try_classification_heuristic_fix(lone, DC),
                None,
                "lone DocumentContent input {lone:?} must not fire heuristic \
                 (#133 / #176 lone-input guard)"
            );
        }
    }

    #[test]
    fn sc010_input_source_confidence_matrix() {
        // SC-010 (#176): the lone-case heuristic guard is conditional on
        // the recognition-provenance axis. Pin the full 2×2 matrix —
        // {StructuredField, DocumentContent} × {lone, in-context} — at
        // the heuristic-fix unit boundary. The "confidence" axis here is
        // structural: a heuristic fix produced (`Some`) is the high-
        // confidence path the decoder later caps at
        // HEURISTIC_RECOGNITION_CAP = 0.95 and surfaces as a fix; `None`
        // is the suggestion-only / no-fix lone-DocumentContent path
        // (~0.50, below the auto-apply floor — research.md #176 matrix).
        //
        //                  | lone (`(YS)`)        | in-context (`(YS//NF)`)
        //   StructuredField| Some  (field 0.95)   | Some  (field 0.95)
        //   DocumentContent| None  (lone ~0.50)   | Some  (in-context high)

        // StructuredField × lone → fixes (trusted caller; guard lifted).
        assert_eq!(
            try_classification_heuristic_fix("(YS)", SF).as_deref(),
            Some("(TS)"),
            "StructuredField lone input MUST heuristic-fix (caller asserts it is a field; \
             #176 → field confidence cap 0.95)"
        );
        // StructuredField × in-context → fixes (same high-confidence).
        assert_eq!(
            try_classification_heuristic_fix("(YS//NF)", SF).as_deref(),
            Some("(TS//NF)"),
            "StructuredField in-context input MUST heuristic-fix"
        );
        // DocumentContent × lone → NO fix (conservative; ~0.50 lone).
        assert_eq!(
            try_classification_heuristic_fix("(YS)", DC),
            None,
            "DocumentContent lone input MUST NOT heuristic-fix (lone ~0.50, suggestion-only)"
        );
        // DocumentContent × in-context → fixes (`//` is marking-shape signal).
        assert_eq!(
            try_classification_heuristic_fix("(YS//NF)", DC).as_deref(),
            Some("(TS//NF)"),
            "DocumentContent in-context input MUST heuristic-fix (marking-shape signal present)"
        );
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
            let result = try_classification_heuristic_fix(input, DC);
            assert_eq!(
                result.as_deref(),
                Some(*expected),
                "input {input:?} should heuristic-fix to {expected:?} \
                 (marking signal present); got {result:?}"
            );
        }
    }
}
