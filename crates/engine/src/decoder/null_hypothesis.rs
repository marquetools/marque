//! Score-time prose null hypothesis.
//!
//! The decoder dispatches a candidate to the rule layer only when its
//! marking-side posterior beats the prose-side null hypothesis by at
//! least [`super::NULL_HYPOTHESIS_LOG_MARGIN`]. This module owns the
//! prose-prior computation, the line-position / bullet-anchor /
//! lowercase-context feature extractor, and the constants that tune
//! all three.

use marque_ism::span::MarkingType;
use marque_rules::confidence::FeatureId;
use marque_scheme::recognizer::ParseContext;
use smallvec::SmallVec;

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
pub(crate) const OBSERVED_UNKNOWN_PROSE_LOG_PRIOR: f32 = -7.0;

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
pub(crate) fn observed_prose_log_prior(bytes: &[u8]) -> f32 {
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
pub(crate) fn looks_like_bullet_anchor(prefix: &[u8]) -> bool {
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
pub(crate) fn candidate_has_lowercase(bytes: &[u8]) -> bool {
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
pub(crate) const CONTEXT_FEATURE_MAX: usize = 2;

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
pub(crate) fn compute_context_features(
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

