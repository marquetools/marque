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
pub(crate) fn try_sar_indicator_repair(text: &str) -> Option<String> {
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
pub(crate) fn match_sar_prefix(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
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
pub(crate) fn match_sar_missing_hyphen(bytes: &[u8], i: usize) -> Option<usize> {
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
