//! Text-preprocessing for decoder inputs.
//!
//! Two passes:
//! 1. [`normalize_delimiters_and_case`] — collapse fullwidth / spaced
//!    slash variants to canonical `//` and uppercase ASCII alpha.
//! 2. [`fuzzy_correct_tokens`] — per-token fuzzy correction against
//!    the closed CAPCO vocabulary, plus superseded-token replacement.
//!
//! Together these produce the normalized byte string that
//! `candidates.rs::generate_candidate_bytes` feeds to the recovery
//! pipeline. Pure functions; no I/O.

use std::borrow::Cow;

use marque_core::fuzzy::FuzzyVocabMatcher;
use marque_ism::{CapcoTokenSet, token_set::TokenSet as _};
use marque_rules::confidence::FeatureId;
use smallvec::SmallVec;

use super::types::FeatureEntry;

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
pub(super) fn normalize_delimiters_and_case(
    text: &str,
) -> (Cow<'_, str>, SmallVec<[FeatureEntry; 4]>) {
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

    // Short-circuit on the canonical-input common case. If
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
pub(super) fn fuzzy_correct_tokens<'a>(
    text: &'a str,
    matcher: &FuzzyVocabMatcher<'_>,
) -> (Cow<'a, str>, SmallVec<[FeatureEntry; 4]>) {
    let mut features: SmallVec<[FeatureEntry; 4]> = SmallVec::new();

    // Lazy-alloc output. `out` stays `None` while every
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
        if CapcoTokenSet.canonicalize(token).is_some() || CapcoTokenSet.is_country_code(token) {
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
    use marque_rules::confidence::FeatureId;
    use marque_scheme::MarkingScheme;
    use marque_scheme::ambiguity::Parsed;
    use marque_scheme::recognizer::{LinePrefix, ParseContext, Recognizer};
    use smallvec::SmallVec;

    use super::*;
    use crate::decoder::DecoderRecognizer;
    use crate::decoder::test_helpers::{TEST_SCHEME, deep_cx};

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
}
