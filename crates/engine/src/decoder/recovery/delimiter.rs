//! Missing-`//` delimiter insertion.
//!
//! CAPCO requires `//` between category segments — `CLASSIFICATION//SCI//
//! SAR//DISSEM`. Real-world transcription often substitutes whitespace
//! for one or more `//` separators, producing inputs the strict parser
//! can't recover. This pass walks the input left-to-right and inserts
//! `//` at whitespace gaps that separate two distinct CAPCO segments.

// ---------------------------------------------------------------------------
// Missing-delimiter insertion (issue #133 PR 3)
// ---------------------------------------------------------------------------

/// Try to insert missing `//` segment separators at category-transition
/// boundaries.
///
/// CAPCO grammar requires `//` between segments —
/// `CLASSIFICATION//SCI_BLOCK//SAR_BLOCK//DISSEM_BLOCK`. Real-world
/// transcription frequently substitutes whitespace for one or more
/// `//` separators, producing inputs the strict parser cannot
/// recover (`SECRET//NOFORN EXDIS` strict-parses as
/// `classification: Secret, dissem: [Nf]` with `EXDIS` left as
/// `TokenKind::Unknown`; the decoder's step-3a Unknown-span filter
/// then discards the candidate).
///
/// This helper walks the input left-to-right and inserts `//` at
/// whitespace gaps that separate two distinct CAPCO segments. Two
/// rules drive insertion:
///
/// 1. **Classification → next segment.** Tokens at the start of the
///    input are classification-context (`U`, `R`, `C`, `S`, `TS`,
///    `UNCLASSIFIED`, …, plus the `TOP SECRET` two-word
///    classification). The first non-classification token after the
///    classification phrase, when no `//` has been emitted yet,
///    triggers `//` insertion before it. Covers the
///    `TOP SECRET HCS-P INTEL OPS//ORCON/NOFORN` / `SECRET REL TO
///    USA, AUS, GBR` family.
///
/// 2. **Hard-splitter dissem long-form.** A small set of unambiguous
///    long-form dissem control tokens (`NOFORN`, `ORCON`,
///    `ORCON-USGOV`, `PROPIN`, `IMCON`, `RELIDO`, `RSEN`,
///    `EYESONLY`, `EXDIS`, `NODIS`, `LIMDIS`, `FOUO`, `FISA`,
///    `DSEN`) ALWAYS start a new segment when they appear after a
///    whitespace gap, regardless of preceding context — these
///    tokens have no in-segment role inside SCI/SAR/REL TO
///    blocks. Covers the `NOFORN EXDIS` / `... SI NOFORN` /
///    `... HCS-P INTEL OPS ORCON/NOFORN` family. The full set is
///    pinned by [`is_hard_splitter_covers_documented_long_forms`].
///
/// Exceptions (do NOT insert):
///
/// - `SBU NOFORN` / `LES NOFORN` — non-IC dissem **banner long
///   forms** for `NonIcDissem::SbuNf` / `NonIcDissem::LesNf`. When
///   the previous token is `SBU` or `LES`, treat `NOFORN` as part
///   of the multi-word atom.
///
/// Returns `None` when no insertion was made — the caller should
/// not emit a duplicate of the input.
///
/// # Bounded
///
/// Hard-capped at [`MAX_DELIMITER_INSERTIONS`] insertions per call.
/// More than four insertions in a single marking is suspicious and
/// likely indicates the input isn't a CAPCO marking at all (or the
/// helper is wrong); rather than emit a wildly-rewritten candidate,
/// we cap and let the result strict-parse on the partial rewrite.
///
/// # SCI / SAR / SPECIAL-ACCESS-REQUIRED coverage
///
/// The PR-3-era doc note here used to defer SCI-starter (`TOP SECRET
/// SI ...`), SAR-prefix (`TOP SECRET SAR-BP ...`), and
/// `SPECIAL ACCESS REQUIRED-...` insertion to a follow-up. That defer
/// was based on a misread: rule 1 (classification → next segment)
/// already fires on every one of those shapes because
/// [`is_classification_token`] includes `TOP` and
/// [`is_classification_continuation`] handles the `TOP → SECRET`
/// special case, so the helper produces the canonical bytes for all
/// 17 MissingDelimiter fixtures in the SC-004 corpus. The remaining
/// 2/17 failures pre-PR-5 were a SCORING contest, not a missing
/// rewrite — handled by [`HARD_SPLITTER_ABSORPTION_PENALTY`] in
/// [`score_candidate`], not here.
pub(crate) fn try_insert_delimiter(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut result = String::with_capacity(text.len() + 8);
    let mut insertions = 0;

    let mut prev_token: Option<&str> = None;
    let mut in_classification = true;
    let mut seen_double_slash = false;

    let mut i = 0;
    while i < bytes.len() {
        // Existing `//` delimiter — copy and reset state.
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            result.push_str("//");
            seen_double_slash = true;
            in_classification = false;
            prev_token = None;
            i += 2;
            continue;
        }

        // Whitespace run — collect, then look at next token.
        if bytes[i].is_ascii_whitespace() {
            let ws_start = i;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let ws = &text[ws_start..i];

            // Find the next token (alnum + internal `-`) starting at `i`.
            let token_start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
                i += 1;
            }
            if token_start == i {
                // Whitespace then non-token character (e.g., `,` or `/` or end).
                // Just copy the whitespace and continue.
                result.push_str(ws);
                continue;
            }
            let next_token = &text[token_start..i];

            let should_insert = decide_insert_delimiter(
                prev_token,
                next_token,
                in_classification,
                seen_double_slash,
            );

            if should_insert && insertions < MAX_DELIMITER_INSERTIONS {
                result.push_str("//");
                insertions += 1;
                seen_double_slash = true;
                in_classification = false;
            } else {
                result.push_str(ws);
            }
            result.push_str(next_token);

            // Update state.
            if !is_classification_continuation(next_token, prev_token) {
                in_classification = false;
            }
            prev_token = Some(next_token);
            continue;
        }

        // Non-whitespace, non-`//` character — likely a `/` (single
        // slash, used as intra-segment separator e.g.
        // `ORCON/NOFORN`), comma, paren, or part of a token. Copy
        // verbatim and continue. Tokens that contain only alnum + `-`
        // are handled in the whitespace branch via the lookahead;
        // the leading-token-at-position-0 case enters here.
        let other_start = i;
        // Take a token (alnum + internal `-`) if at one.
        if bytes[i].is_ascii_alphanumeric() {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
                i += 1;
            }
            let leading_token = &text[other_start..i];
            result.push_str(leading_token);
            // Update prev_token / classification state for the
            // leading token (no insertion possible at position 0).
            if !is_classification_continuation(leading_token, prev_token) {
                in_classification = false;
            }
            prev_token = Some(leading_token);
            continue;
        }

        // Single non-token character (`/`, `(`, `)`, `,`, or any
        // non-ASCII character — e.g., a stray `∕` that the upstream
        // delimiter normalizer didn't catch). Preserve the original
        // UTF-8 character verbatim instead of doing `bytes[i] as
        // char`, which would corrupt multi-byte sequences by emitting
        // each byte as a separate Latin-1 codepoint.
        let ch = text[i..]
            .chars()
            .next()
            .expect("byte index must remain on a char boundary");
        result.push(ch);
        i += ch.len_utf8();
    }

    if insertions == 0 { None } else { Some(result) }
}

/// Hard cap on the number of `//` insertions per call. More than 4
/// in a single marking is very suspicious — real markings rarely
/// have that many segments at all. The cap prevents the helper
/// from rewriting non-marking prose that happens to contain
/// splitter words.
const MAX_DELIMITER_INSERTIONS: usize = 4;

/// Decide whether to insert `//` at a whitespace gap before
/// `next_token`. See [`try_insert_delimiter`] doc for the rules.
fn decide_insert_delimiter(
    prev_token: Option<&str>,
    next_token: &str,
    in_classification: bool,
    seen_double_slash: bool,
) -> bool {
    // Multi-word atom exceptions: don't split between SBU/LES and
    // their NOFORN companion (banner long forms for NonIcDissem
    // SbuNf/LesNf).
    if next_token == "NOFORN" && matches!(prev_token, Some("SBU") | Some("LES")) {
        return false;
    }

    // Rule 1: classification → next segment. The first non-
    // classification token after the classification phrase, when no
    // `//` has been emitted yet.
    if in_classification && !seen_double_slash && !is_classification_token(next_token) {
        return true;
    }

    // Rule 2: hard-splitter dissem long-form. These tokens always
    // start a new segment when they appear after whitespace.
    is_hard_splitter(next_token)
}

/// True when `token` is a classification short or long form that
/// can appear in classification context.
fn is_classification_token(token: &str) -> bool {
    matches!(
        token,
        "U" | "R"
            | "C"
            | "S"
            | "TS"
            | "TOP"
            | "UNCLASSIFIED"
            | "RESTRICTED"
            | "CONFIDENTIAL"
            | "SECRET"
    )
}

/// True when `next_token` continues the classification phrase from
/// `prev_token`. Specifically: `TOP SECRET` is the only multi-word
/// classification CAPCO recognizes; `SECRET` after `TOP` continues
/// the classification.
fn is_classification_continuation(next_token: &str, prev_token: Option<&str>) -> bool {
    if next_token == "SECRET" && prev_token == Some("TOP") {
        return true;
    }
    is_classification_token(next_token)
}

/// True when `token` is an unambiguous segment-starting dissem
/// long-form. These tokens have no in-segment role inside SCI / SAR /
/// REL TO blocks, so seeing one after whitespace always indicates a
/// missing `//` separator. Pinned by
/// `try_insert_delimiter_inserts_before_long_form_dissem`.
///
/// Excluded from this set:
///
/// - 2-char short forms (`NF`, `OC`, `PR`, `IMC`, `RS`) — could
///   collide with SAR compartment / sub-compartment naming.
/// - SCI starters (`SI`, `HCS`, `TK`, `KDK`) — 2-3 char tokens that
///   appear in compartment context.
/// - SAR prefixes (`SAR-*`) — handled in v2 with classification-
///   context lookahead.
pub(crate) fn is_hard_splitter(token: &str) -> bool {
    matches!(
        token,
        "NOFORN"
            | "ORCON"
            | "ORCON-USGOV"
            | "PROPIN"
            | "IMCON"
            | "RELIDO"
            | "RSEN"
            | "EYESONLY"
            | "FOUO"
            | "FISA"
            | "DSEN"
            | "EXDIS"
            | "NODIS"
            | "LIMDIS"
    )
}

