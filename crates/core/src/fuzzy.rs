// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Vocabulary-aware fuzzy correction for CAPCO tokens.
//!
//! # Design
//!
//! CAPCO markings are built from a **closed vocabulary** of ~52 CVE tokens
//! (classification levels, SCI controls, dissemination controls, and a handful
//! of structural keywords). OCR and manual transcription errors produce
//! near-miss variants — `SERCET`, `NOFRON`, `CONFIDETIAL` — that no rule ever
//! fires on because the scanner never detects them as marking candidates.
//!
//! The approach here mirrors what makes [`typos`](https://github.com/crate-ci/typos)
//! so effective at eliminating false positives, adapted for the closed-world
//! property of CAPCO vocabulary:
//!
//! 1. **Closed-world validation first.** If the input token is already in the
//!    vocabulary, return `None` immediately — no correction needed, no false
//!    positive possible.
//!
//! 2. **Exhaustive near-miss search.** Because the vocabulary is tiny (~52
//!    tokens), computing Levenshtein edit distance to every known token is
//!    fast (microseconds).
//!
//! 3. **Ambiguity rejection.** If two or more vocabulary entries are equally
//!    close, the correction is ambiguous. Return `None` and let the engine
//!    surface it as a human-review item — exactly what `typos` does for words
//!    that could correct to multiple targets.
//!
//! 4. **Minimum-length guard.** Very short tokens (1-2 characters) are excluded
//!    from fuzzy matching because edit distance is semantically unreliable at
//!    that length. `C`, `S`, `U` are valid in context but look similar enough to
//!    dozens of other possibilities that any fuzzy suggestion would be noise.
//!    See [`MIN_FUZZY_LEN`] for the 2-char rationale (PR 7 SAR
//!    sub-compartment false-positives).
//!
//! 5. **Confidence scores.** Each `FuzzyCorrection` carries a base confidence
//!    derived from edit distance and token length. The calling engine multiplies
//!    this by a **context factor** (+0.10–0.15 when the token is inside a
//!    detected marking region) before comparing against the configured threshold.
//!
//! # Integration Points
//!
//! The [`FuzzyVocabMatcher`] is injected into the engine's pre-scanner step.
//! In the default configuration it operates after the AhoCorasick corrections
//! map pass — user-configured exact corrections take priority; the fuzzy matcher
//! handles residual OCR noise the exact map doesn't cover.
//!
//! **WASM-safe**: no I/O, no platform-specific code, and only small transient
//! heap allocations during edit-distance computation.

/// A correction candidate produced by [`FuzzyVocabMatcher::correct`].
///
/// Callers should multiply `confidence` by a context factor before comparing
/// against the engine threshold:
/// - Inside a detected marking region (between `//` boundaries or `(...)`):
///   multiply by `1.0 + 0.15 * context_strength` (practical range 1.10–1.15).
/// - In open prose with no structural signal: use `confidence` as-is.
#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyCorrection {
    /// The suggested canonical token from the vocabulary.
    pub token: &'static str,
    /// Levenshtein edit distance between the input and `token`.
    pub distance: u8,
    /// Base confidence score in `[0.0, 1.0]`.
    ///
    /// Derived from `distance` and token length. Does **not** include a
    /// context factor — callers should apply one before thresholding.
    pub confidence: f32,
}

/// Vocabulary-aware fuzzy corrector for a closed token set.
///
/// Construct once per engine session from the vocabulary slice exposed by
/// `marque_ism::TokenSet::correction_vocab`. The vocab must be sorted and
/// deduplicated: the "is already valid" fast path uses [`slice::binary_search`],
/// and the ambiguity check assumes each candidate appears at most once. For
/// [`marque_ism::CapcoTokenSet`] the invariant is enforced at the source —
/// `ALL_CVE_TOKENS` is emitted sorted and deduplicated by
/// `marque-ism/build.rs` and verified by `token_set::tests`.
///
/// # Example
/// ```
/// use marque_core::fuzzy::FuzzyVocabMatcher;
/// use marque_ism::CapcoTokenSet;
/// use marque_ism::token_set::TokenSet as _;
///
/// let vocab = CapcoTokenSet.correction_vocab();
/// let matcher = FuzzyVocabMatcher::new(vocab);
///
/// // "SERCET" is one transpose away from "SECRET"
/// let result = matcher.correct("SERCET");
/// assert_eq!(result.map(|c| c.token), Some("SECRET"));
///
/// // Known tokens → no correction
/// assert!(matcher.correct("SECRET").is_none());
///
/// // Too ambiguous → no correction
/// // (tokens equidistant from the input → ambiguous, return None)
/// ```
pub struct FuzzyVocabMatcher<'v> {
    vocab: &'v [&'static str],
}

impl<'v> FuzzyVocabMatcher<'v> {
    /// Create a new matcher over `vocab`.
    ///
    /// `vocab` must be the sorted, deduplicated CVE token slice returned by
    /// [`TokenSet::correction_vocab`]. Construction is `O(1)` — the slice is
    /// not copied or indexed. The slice itself may live on the caller's
    /// `TokenSet` implementation (e.g., a `Vec<&'static str>` field), but each
    /// entry must be `&'static str` so that [`FuzzyCorrection::token`] — which
    /// borrows directly from the vocabulary — outlives the matcher.
    pub fn new(vocab: &'v [&'static str]) -> Self {
        Self { vocab }
    }

    /// Attempt to find a fuzzy correction for an unknown `token`.
    ///
    /// Returns `None` when:
    /// - `token` is already a known vocabulary entry (no correction needed).
    /// - `token` is too short (< `MIN_FUZZY_LEN` bytes).
    /// - No vocabulary entry is within [`MAX_EDIT_DISTANCE`] edits.
    /// - Multiple vocabulary entries tie at the closest distance (ambiguous).
    ///
    /// Returns `Some(FuzzyCorrection)` only when the correction is unambiguous.
    ///
    /// # ASCII invariant
    ///
    /// Length checks and the underlying edit-distance computation both operate
    /// on byte counts. The CAPCO vocabulary is pure ASCII (classification
    /// levels, SCI/dissem/SAR tokens, etc.), so byte count and character count
    /// coincide for every expected input. Non-ASCII input is compared byte-wise
    /// and will not produce meaningful corrections — which is the intended
    /// behavior, since no non-ASCII candidate exists in the closed vocab.
    pub fn correct(&self, token: &str) -> Option<FuzzyCorrection> {
        // Fast path: token is already valid → nothing to correct.
        if self.vocab.binary_search(&token).is_ok() {
            return None;
        }

        let token_len = token.len();

        // Very short tokens are too noisy for edit-distance correction.
        if token_len < MIN_FUZZY_LEN {
            return None;
        }

        // Scratch buffers reused across every candidate in this call. The
        // rolling two-row array needs `shorter + 1` slots, where
        // `shorter = min(token.len(), candidate.len())`. The length-diff
        // filter below guarantees `|token.len() - candidate.len()| <=
        // MAX_EDIT_DISTANCE`, so `shorter <= token_len`, and a single
        // allocation of `token_len + 1` fits every candidate. This avoids
        // the ~2 heap allocations per candidate (~100 per correction attempt)
        // that allocating inside the levenshtein helper would incur.
        let scratch_len = token_len + 1;
        let mut prev = vec![0u8; scratch_len];
        let mut curr = vec![0u8; scratch_len];

        let mut best_dist = u8::MAX;
        let mut best_token: Option<&'static str> = None;
        let mut ambiguous = false;

        for &candidate in self.vocab {
            let cand_len = candidate.len();

            // Skip candidates whose length difference alone exceeds our bound.
            // This is a fast filter before the full Levenshtein computation.
            let len_diff = token_len.abs_diff(cand_len);
            if len_diff > MAX_EDIT_DISTANCE as usize {
                continue;
            }

            let d = levenshtein_with_scratch(token, candidate, &mut prev, &mut curr);

            if d < best_dist {
                best_dist = d;
                best_token = Some(candidate);
                ambiguous = false;
            } else if d == best_dist {
                // Another candidate at the same distance — correction is
                // ambiguous; do not auto-suggest.
                ambiguous = true;
            }
        }

        if ambiguous || best_dist > MAX_EDIT_DISTANCE {
            return None;
        }

        let token = best_token?;
        let confidence = correction_confidence(best_dist, token_len);

        // Confidence too low to be meaningful — treat as no match.
        if confidence < MIN_USEFUL_CONFIDENCE {
            return None;
        }

        Some(FuzzyCorrection {
            token,
            distance: best_dist,
            confidence,
        })
    }

    /// Return every vocabulary entry within
    /// [`MAX_EDIT_DISTANCE`] of `token`, paired with its distance.
    ///
    /// Behaves like [`Self::correct`] but does NOT collapse ambiguous
    /// matches to `None`. The decoder uses this when the caller needs
    /// to score multiple candidates against a downstream prior — for
    /// REL TO trigraph fuzzy recovery, the corpus-weighted log-prior
    /// breaks ties that the matcher itself cannot (issue #233).
    /// Fast-paths the same as [`Self::correct`]:
    ///
    /// - `token` is already in vocab → returns an empty vec.
    /// - `token.len() < MIN_FUZZY_LEN` → returns an empty vec.
    ///
    /// Output is ordered by ascending distance, then by the
    /// vocabulary's lexicographic order (because the iteration walks
    /// the sorted vocab slice). Capped by [`MAX_EDIT_DISTANCE`] so a
    /// single call cannot run away on a tiny vocab; the priors-bake
    /// vocabulary stays well bounded in practice.
    pub fn correct_all(&self, token: &str) -> Vec<FuzzyCorrection> {
        self.correct_all_with_floor(token, MIN_USEFUL_CONFIDENCE)
    }

    /// Like [`Self::correct_all`] but with a caller-controlled
    /// confidence floor.
    ///
    /// The default floor (`MIN_USEFUL_CONFIDENCE` = 0.45) excludes
    /// distance-2 corrections of 3-char inputs, which is the right
    /// safety policy for the standard fuzzy path because those
    /// corrections are too speculative without surrounding context.
    /// The decoder's REL TO trigraph expansion (issue #233) supplies
    /// surrounding context — the candidate goes through the strict
    /// REL TO parser, the resulting marking has a corpus-weighted
    /// trigraph prior, and the decoder's `UNAMBIGUOUS_LOG_MARGIN`
    /// breaks ties at score time. Lowering the floor for that
    /// specific call site is what lets a typo like `ASU → AUS`
    /// (distance 2 in plain Levenshtein) reach the scorer.
    ///
    /// Callers passing a floor of `0.0` get every match within
    /// [`MAX_EDIT_DISTANCE`].
    pub fn correct_all_with_floor(
        &self,
        token: &str,
        confidence_floor: f32,
    ) -> Vec<FuzzyCorrection> {
        if self.vocab.binary_search(&token).is_ok() {
            return Vec::new();
        }

        let token_len = token.len();
        if token_len < MIN_FUZZY_LEN {
            return Vec::new();
        }

        let scratch_len = token_len + 1;
        let mut prev = vec![0u8; scratch_len];
        let mut curr = vec![0u8; scratch_len];

        let mut hits: Vec<FuzzyCorrection> = Vec::new();
        for &candidate in self.vocab {
            let cand_len = candidate.len();
            let len_diff = token_len.abs_diff(cand_len);
            if len_diff > MAX_EDIT_DISTANCE as usize {
                continue;
            }
            let d = levenshtein_with_scratch(token, candidate, &mut prev, &mut curr);
            if d > MAX_EDIT_DISTANCE {
                continue;
            }
            let confidence = correction_confidence(d, token_len);
            if confidence < confidence_floor {
                continue;
            }
            hits.push(FuzzyCorrection {
                token: candidate,
                distance: d,
                confidence,
            });
        }
        // Sort ascending by distance; preserve vocab order within a
        // distance band (vocab is already sorted, so the secondary
        // ordering falls out of the iteration without a re-sort).
        hits.sort_by_key(|h| h.distance);
        hits
    }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum input token length for fuzzy matching.
///
/// Tokens shorter than this are excluded. Single- and two-char tokens
/// are too noisy for context-free edit-distance correction:
///
/// - **Single-char**: `S`, `C`, `U`, `R` are valid classification CAPCO
///   abbreviations and at edit distance 1 from too many other
///   single-char tokens to produce reliable corrections.
/// - **Two-char**: tried briefly during issue #133 PR 7 to recover
///   `UK → TK`-style typos, but reverted because of false-positive
///   collisions with SAR identifier sub-compartment letters. Most
///   visibly, the canonical Enron-corpus SAR fixture
///   `SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN`
///   has `RB` as a standalone 2-char token (sub-compartment of XRA),
///   and `RB` is at edit distance 1 from `RS` (the IC dissem portion
///   form for RSEN) — so 2-char fuzzy silently converted SAR
///   sub-compartment letters into dissem controls. The fuzzy
///   matcher has no context awareness to know "we're inside a SAR
///   block, skip identifier-shaped tokens", so the safer choice is
///   to keep 2-char tokens out of fuzzy and address the
///   `UK→TK`-style cases via a context-aware structural pass in
///   the decoder.
pub const MIN_FUZZY_LEN: usize = 3;

/// Maximum Levenshtein edit distance considered for a correction.
///
/// Edit distance 2 covers:
/// - Transpositions (`SERCET` → `SECRET`, distance 2).
/// - Single-char substitutions + insertions (`CONFIDETIAL` → `CONFIDENTIAL`,
///   distance 1).
/// - Two-step corrections for heavily OCR'd text (`SECRECT` → `SECRET`,
///   distance 2).
///
/// Distance 3+ produces too many false positives against short CAPCO tokens
/// and is therefore excluded.
pub const MAX_EDIT_DISTANCE: u8 = 2;

/// Minimum confidence score below which a correction is suppressed.
///
/// This threshold prevents low-quality guesses from entering the pipeline
/// even when they satisfy the edit-distance bound — for example, a
/// distance-2 correction of a 4-character input has so many possible sources
/// that the prior is too weak to act on.
const MIN_USEFUL_CONFIDENCE: f32 = 0.45;

// ---------------------------------------------------------------------------
// Confidence scoring
// ---------------------------------------------------------------------------

/// Derive a base confidence score from edit distance and token length.
///
/// # Rationale
///
/// The prior that an edit-distance-1 deviation from a known CAPCO token is a
/// typo is much higher for long tokens than for short ones:
/// - `NOFRON` → `NOFORN` (distance 1, length 6): almost certainly a typo.
/// - `SB` → `SI` (distance 1, length 2): could be many things.
///
/// The formula is:
/// ```text
/// base = match distance {
///     1 => 0.55 + 0.05 * min(token_len, 6).saturating_sub(3)  →  [0.55, 0.70]
///     2 => 0.40 + 0.05 * min(token_len, 8).saturating_sub(5)  →  [0.40, 0.55]
///     _ => 0.0
/// }
/// ```
/// These are intentionally conservative. The engine's context factor
/// (marking-region signal) raises the effective confidence at call time.
fn correction_confidence(distance: u8, token_len: usize) -> f32 {
    match distance {
        0 => 1.0, // exact match — should not reach this path
        1 => {
            // 0.55 base, +0.05 per char over 3 (capped at 6 chars → 0.70).
            let bonus = (token_len.min(6).saturating_sub(3)) as f32 * 0.05;
            0.55 + bonus
        }
        2 => {
            // 0.40 base, +0.05 per char over 5 (capped at 8 chars → 0.55).
            let bonus = (token_len.min(8).saturating_sub(5)) as f32 * 0.05;
            0.40 + bonus
        }
        _ => 0.0,
    }
}

// ---------------------------------------------------------------------------
// Levenshtein edit distance
// ---------------------------------------------------------------------------

/// Compute the Levenshtein edit distance between `a` and `b`.
///
/// Uses a two-row rolling-array approach — O(min(|a|, |b|)) space, O(|a|·|b|)
/// time. Input strings are short CAPCO tokens (2–20 chars), so this runs in
/// microseconds.
///
/// Returns the edit distance clamped to `u8::MAX` (any distance > 255 is
/// already far beyond the useful range for CAPCO corrections).
///
/// Operates on bytes: two characters are "equal" iff they have the same byte
/// representation. The CAPCO vocabulary is pure ASCII, so this matches a
/// character-level edit distance for every expected input.
///
/// This wrapper allocates two `Vec<u8>` buffers per call. On hot paths that
/// call Levenshtein for each of many candidates (e.g.,
/// [`FuzzyVocabMatcher::correct`]), prefer [`levenshtein_with_scratch`] and
/// reuse the scratch buffers across candidates.
#[cfg(test)]
pub(crate) fn levenshtein(a: &str, b: &str) -> u8 {
    let shorter = a.len().min(b.len());
    let mut prev = vec![0u8; shorter + 1];
    let mut curr = vec![0u8; shorter + 1];
    levenshtein_with_scratch(a, b, &mut prev, &mut curr)
}

/// Same as [`levenshtein`] but reuses caller-owned scratch buffers.
///
/// `prev_buf` and `curr_buf` must each have length strictly greater than
/// `min(a.len(), b.len())`; their contents are overwritten. In debug builds
/// this is enforced by a `debug_assert!`.
pub(crate) fn levenshtein_with_scratch(
    a: &str,
    b: &str,
    prev_buf: &mut [u8],
    curr_buf: &mut [u8],
) -> u8 {
    // Ensure `a` is the shorter string (minimizes inner-loop iterations).
    let (a, b) = if a.len() <= b.len() { (a, b) } else { (b, a) };

    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let n = a_bytes.len();
    let m = b_bytes.len();

    if n == 0 {
        return m.min(u8::MAX as usize) as u8;
    }

    debug_assert!(
        prev_buf.len() > n && curr_buf.len() > n,
        "scratch buffers must have len > min(a.len(), b.len())"
    );

    // Local mutable bindings so `mem::swap` can rotate which buffer is `prev`
    // and which is `curr` without moving bytes.
    let (mut prev, mut curr) = (prev_buf, curr_buf);

    // prev[i] holds the distance between a[0..i] and the previous b prefix
    // (before the loop: b[0..0], and at outer iteration j: b[0..j-1]).
    for (i, slot) in prev.iter_mut().enumerate().take(n + 1) {
        *slot = i.min(u8::MAX as usize) as u8;
    }

    for j in 1..=m {
        curr[0] = j.min(u8::MAX as usize) as u8;

        for i in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] {
                0u8
            } else {
                1u8
            };

            let del = prev[i].saturating_add(1);
            let ins = curr[i - 1].saturating_add(1);
            let sub = prev[i - 1].saturating_add(cost);
            curr[i] = del.min(ins).min(sub);
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

    // ----- levenshtein -----

    #[test]
    fn lev_identical_strings() {
        assert_eq!(levenshtein("SECRET", "SECRET"), 0);
    }

    #[test]
    fn lev_single_transpose() {
        // SERCET: R and C swapped — that's a substitution at position 3 and 4,
        // which is distance 2 in standard Levenshtein (no transposition op).
        // But the user's intuition is "one swap", which is distance 2.
        assert_eq!(levenshtein("SERCET", "SECRET"), 2);
    }

    #[test]
    fn lev_insertion() {
        // CONFIDETIAL is missing the second N → distance 1.
        assert_eq!(levenshtein("CONFIDETIAL", "CONFIDENTIAL"), 1);
    }

    #[test]
    fn lev_transposition() {
        // Adjacent transposition (O and R swapped) → distance 2 in standard
        // Levenshtein (two substitutions: position 3 O→R, position 4 R→O).
        assert_eq!(levenshtein("NOFORN", "NOFRON"), 2);
    }

    #[test]
    fn lev_empty_vs_nonempty() {
        assert_eq!(levenshtein("", "SECRET"), 6);
        assert_eq!(levenshtein("SECRET", ""), 6);
    }

    #[test]
    fn lev_symmetry() {
        assert_eq!(
            levenshtein("NOFORN", "NOFRON"),
            levenshtein("NOFRON", "NOFORN")
        );
    }

    // ----- FuzzyVocabMatcher -----

    /// A minimal offline vocabulary for tests — does not require a Cargo build.
    static TEST_VOCAB: &[&str] = &[
        "CONFIDENTIAL",
        "FOUO",
        "NOFORN",
        "SECRET",
        "TOP SECRET",
        "UNCLASSIFIED",
    ];

    fn matcher() -> FuzzyVocabMatcher<'static> {
        FuzzyVocabMatcher::new(TEST_VOCAB)
    }

    #[test]
    fn known_token_returns_none() {
        // "SECRET" is in vocab — no correction needed.
        assert!(matcher().correct("SECRET").is_none());
    }

    #[test]
    fn short_token_returns_none() {
        // Single-char inputs are below MIN_FUZZY_LEN.
        assert!(matcher().correct("S").is_none());
        // 2-char inputs are also below MIN_FUZZY_LEN — issue #133 PR 7
        // experimented with lowering this to 2 but reverted because
        // 2-char SAR identifier sub-compartments (most visibly `RB`
        // in the canonical Enron-corpus SAR fixture) collide with
        // `RS` (the RSEN portion form) at edit distance 1, silently
        // corrupting SAR sub-compartment text into dissem controls.
        // See `MIN_FUZZY_LEN` doc for the full PR-7 rationale.
        assert!(matcher().correct("NF").is_none());
    }

    #[test]
    fn confidetial_corrects_to_confidential() {
        // One missing character — should correct.
        let result = matcher().correct("CONFIDETIAL");
        assert_eq!(result.as_ref().map(|c| c.token), Some("CONFIDENTIAL"));
        // Distance should be 1.
        assert_eq!(result.map(|c| c.distance), Some(1));
    }

    #[test]
    fn nofron_corrects_to_noforn() {
        // Adjacent transposition (O and R swapped) → distance 2 in Levenshtein.
        let result = matcher().correct("NOFRON");
        assert_eq!(result.map(|c| c.token), Some("NOFORN"));
    }

    #[test]
    fn sercet_corrects_to_secret() {
        // Common transposition typo: R and C transposed in "SERCET" vs "SECRET".
        // Standard Levenshtein counts this as distance 2 (two substitutions).
        let result = matcher().correct("SERCET");
        assert_eq!(result.as_ref().map(|c| c.token), Some("SECRET"));
        let c = result.unwrap();
        // Distance-2, length-6 → confidence = 0.40 + 1*0.05 = 0.45.
        assert!(
            c.confidence > 0.44,
            "confidence should be non-trivial: {}",
            c.confidence
        );
    }

    #[test]
    fn confidence_increases_with_token_length() {
        // Longer tokens at distance 1 should have higher confidence.
        let short_conf = correction_confidence(1, 4); // e.g., FOUO-like
        let long_conf = correction_confidence(1, 12); // e.g., CONFIDENTIAL-like
        assert!(
            long_conf > short_conf,
            "expected long_conf {long_conf} > short_conf {short_conf}"
        );
    }

    #[test]
    fn completely_unrelated_string_returns_none() {
        // "BANANA" is distance > 2 from every test vocab entry.
        assert!(matcher().correct("BANANA").is_none());
    }

    #[test]
    fn ambiguous_corrections_return_none() {
        // Use a tiny local vocabulary where the input is tied between two
        // candidates within MAX_EDIT_DISTANCE, so None is returned because the
        // best correction is ambiguous rather than because all candidates are
        // filtered out for being too far away.
        let vocab = &["BOOK", "COOK"];
        let matcher = FuzzyVocabMatcher::new(vocab);
        assert!(matcher.correct("NOOK").is_none());
    }

    #[test]
    fn distance_2_edit_returns_correction_for_long_tokens() {
        // "UNCLASSIFEID" — two character errors in a long token.
        let result = matcher().correct("UNCLASSIFEID");
        assert_eq!(result.map(|c| c.token), Some("UNCLASSIFIED"));
    }

    #[test]
    fn correction_confidence_distance1_scales_with_length() {
        // Sanity-check the confidence formula directly.
        // Use approximate comparison to avoid f32 precision noise.
        let eps = 1e-5_f32;
        assert!((correction_confidence(1, 3) - 0.55).abs() < eps); // 0.55 + 0 bonus
        assert!((correction_confidence(1, 4) - 0.60).abs() < eps); // 0.55 + 1*0.05
        assert!((correction_confidence(1, 6) - 0.70).abs() < eps); // 0.55 + 3*0.05 (capped)
        assert!((correction_confidence(1, 12) - 0.70).abs() < eps); // capped at 6
    }

    // ----- Real-vocab regression tests (issue #133 root cause #1) -----
    //
    // These tests use `CapcoTokenSet::correction_vocab()` directly so a
    // future change that removed the banner long-form additions from
    // the extended vocab (or unintentionally narrowed it back to
    // `ALL_CVE_TOKENS`) would fail here, not silently regress the
    // SC-004 accuracy harness. The unit tests above use a minimal
    // local TEST_VOCAB and would not catch the regression.

    #[test]
    fn real_vocab_corrects_noforon_to_noforn() {
        // Issue #133 root cause #1, primary example from PR #136
        // diagnostic: `NOFORON` is one insertion away from `NOFORN`.
        // Before the long-form vocab fix this returned None because
        // `NOFORN` was not in the matcher's vocabulary at all.
        use marque_ism::CapcoTokenSet;
        use marque_ism::token_set::TokenSet as _;
        let vocab = CapcoTokenSet.correction_vocab();
        let matcher = FuzzyVocabMatcher::new(vocab);
        let result = matcher.correct("NOFORON");
        assert_eq!(result.as_ref().map(|c| c.token), Some("NOFORN"));
        assert_eq!(result.map(|c| c.distance), Some(1));
    }

    #[test]
    fn real_vocab_corrects_nofron_to_noforn() {
        // Adjacent transposition (R↔O): standard Levenshtein distance 2.
        use marque_ism::CapcoTokenSet;
        use marque_ism::token_set::TokenSet as _;
        let vocab = CapcoTokenSet.correction_vocab();
        let matcher = FuzzyVocabMatcher::new(vocab);
        let result = matcher.correct("NOFRON");
        assert_eq!(result.map(|c| c.token), Some("NOFORN"));
    }

    #[test]
    fn real_vocab_corrects_orcon_typo() {
        // Coverage for the §G.1 Table 4 ORCON / OC pair beyond NOFORN.
        use marque_ism::CapcoTokenSet;
        use marque_ism::token_set::TokenSet as _;
        let vocab = CapcoTokenSet.correction_vocab();
        let matcher = FuzzyVocabMatcher::new(vocab);
        let result = matcher.correct("ORCN");
        assert_eq!(result.as_ref().map(|c| c.token), Some("ORCON"));
    }

    #[test]
    fn real_vocab_emits_multi_word_banner_for_whitespace_free_typo() {
        // Pin the documented behavior of multi-word entries in
        // `EXTENDED_CORRECTION_VOCAB`. The fuzzy matcher CAN emit a
        // multi-word vocab entry as the correction for a
        // whitespace-free typo (here: `SBUNOFORN` → `SBU NOFORN` at
        // distance 1, single-character insertion of the space).
        // The strict parser then accepts the corrected form via
        // `parse_non_ic_full_form`, so the round-trip lands as the
        // expected `NonIcDissem::SbuNf`.
        //
        // Pinning this lets us word the doc comment on
        // `EXTENDED_CORRECTION_VOCAB` accurately — multi-word
        // entries are reachable, not "inert".
        use marque_ism::CapcoTokenSet;
        use marque_ism::token_set::TokenSet as _;
        let vocab = CapcoTokenSet.correction_vocab();
        let matcher = FuzzyVocabMatcher::new(vocab);
        let result = matcher.correct("SBUNOFORN");
        assert_eq!(result.as_ref().map(|c| c.token), Some("SBU NOFORN"));
        assert_eq!(result.map(|c| c.distance), Some(1));
    }

    #[test]
    fn correction_confidence_distance2_scales_with_length() {
        let eps = 1e-5_f32;
        assert!((correction_confidence(2, 5) - 0.40).abs() < eps); // 0.40 + 0 bonus
        assert!((correction_confidence(2, 6) - 0.45).abs() < eps); // 0.40 + 1*0.05
        assert!((correction_confidence(2, 8) - 0.55).abs() < eps); // 0.40 + 3*0.05 (capped)
        assert!((correction_confidence(2, 15) - 0.55).abs() < eps); // capped at 8
    }
}
