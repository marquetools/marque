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
/// # Confidence
///
/// The decoder tags this attempt's `CanonicalAttempt::fix_source`
/// with [`marque_rules::FixSource::DecoderClassificationHeuristic`].
/// The engine then (a) downgrades the diagnostic severity to
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
pub(crate) fn try_classification_heuristic_fix(text: &str) -> Option<String> {
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
    // Form-field input (`(YS)` typed into a portion-mark field)
    // SHOULD heuristic-fix at high confidence — the caller knows the
    // input is a marking attempt — but we don't yet have an input-
    // source signal to distinguish form-field from document-content.
    // When an input-source signal on `ParseContext` lands, this
    // safety guard becomes conditional on
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
/// `TOP` is in the match set because the helper's whitespace
/// tokenizer treats `TOP` as a non-canonical token; without the
/// short-circuit the heuristic would fire on perfectly-canonical
/// `TOP SECRET//...` input — a no-op when the heuristic returned
/// `None` for length-3 inputs, but a latent footgun once the
/// length-3 arm starts returning `Some`. Recognizing bare
/// `TOP` as canonical short-circuits the heuristic on the
/// already-correct case.
pub(crate) fn is_canonical_short_classification(token: &str) -> bool {
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
pub(crate) fn try_2char_classification_heuristic(token: &str) -> Option<&'static str> {
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
pub(crate) fn try_3char_classification_heuristic(token: &str) -> Option<&'static str> {
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
pub(crate) fn try_1char_classification_heuristic(token: &str) -> Option<&'static str> {
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
