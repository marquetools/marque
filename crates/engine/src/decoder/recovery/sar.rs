// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SAR indicator-keyword structural repair.
//!
//! Two structural patterns near the `SAR-` indicator keyword
//! (CAPCO-2016 §H.5 p100): stray-prefix strip (`USAR-` → `SAR-`,
//! preserving the rest of the program identifier) and missing-hyphen
//! insertion (`SARBP` → `SAR-BP`). Both operate on the indicator
//! keyword only — never on the agency-assigned program identifier
//! that follows.

// ---------------------------------------------------------------------------
// SAR indicator-keyword structural repair
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
pub(in crate::decoder) fn try_sar_indicator_repair(text: &str) -> Option<String> {
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
        // `SPCIAL` (typo in `SPECIAL`) — issue #133 vocab
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
}
