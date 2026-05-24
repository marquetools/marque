// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Citation parsing.
//!
//! A citation reference in marque source has the canonical form
//! `CAPCO-2016 §X.Y pNN` or `CAPCO-2016 §X.Y pp NN–MM`, where:
//!
//! - `X` is a single uppercase letter A–H (the normative section range);
//!   I/J/K are non-normative (history, examples, acronyms) and rejected.
//! - `Y` is a positive integer (subsection number).
//! - `NN` / `MM` are positive integer page numbers.
//!
//! The `CAPCO-2016 ` prefix is optional: a bare `§X.Y pNN` is still a
//! valid citation in this codebase because the source is implicit.
//!
//! Some citations omit the page number entirely (e.g., `§K.2` or
//! `§G.1 Table 4`). Those are still extracted as occurrences but only
//! the section is validated against the source — page resolution
//! requires a page anchor.
//!
//! Page-range syntax accepts either a hyphen `-` or an en-dash `–` /
//! em-dash `—` between the page numbers (the markdown source mixes
//! them). Both endpoints must resolve per `correctness.md` CHK038.
//!
//! What this module does NOT do:
//!
//! - It does not validate that a parsed citation resolves; that is the
//!   resolver's job (`resolver.rs`).
//! - It does not detect citation occurrences in source files; that is
//!   the AST scanner's job (`scanner.rs`). This module operates on
//!   already-extracted text fragments — a string-literal value, a
//!   doc-comment line, etc.

use std::fmt;

/// One CAPCO citation, parsed.
///
/// `section` is always present. `subsection` is `None` for letter-only
/// references like `§F` (which is valid for sections without numbered
/// subdivisions). `pages` is `None` for citations that omit a page
/// anchor (e.g., `§K.2`, `§G.1 Table 4`). When present, `pages` is
/// `(start, end)` where `end == start` for single-page form `pNN`.
///
/// The parser does NOT enforce the citation rules here: `subsection:
/// None` is a shape this struct can hold, and the resolver decides whether
/// it's lawful for the cited section. The bare-numeric-section
/// rejection (`§NN` without letter) is enforced as a separate
/// `CitationFind::BareSection` variant — that case never produces a
/// `Citation` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    pub section: char,
    pub subsection: Option<u32>,
    pub pages: Option<(u32, u32)>,
    /// Verbatim text from the source for diagnostic rendering.
    pub raw: String,
}

impl fmt::Display for Citation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.subsection {
            Some(n) => write!(f, "§{}.{}", self.section, n)?,
            None => write!(f, "§{}", self.section)?,
        }
        match self.pages {
            None => Ok(()),
            Some((a, b)) if a == b => write!(f, " p{a}"),
            Some((a, b)) => write!(f, " pp {a}–{b}"),
        }
    }
}

/// Find every citation occurrence in a fragment of text.
///
/// Returns offsets relative to the start of `fragment` so a caller
/// holding a span for `fragment` can produce file-coordinates by
/// adding the inner offset.
///
/// The grammar this recognizes (matched left-to-right, longest match
/// per starting position):
///
/// ```text
/// citation := '§' SECTION '.' SUBSECTION (PAGE_ANCHOR)?
/// SECTION  := [A-K]
/// SUBSECTION := [0-9]+
/// PAGE_ANCHOR := WS+ ('p' PAGE | 'pp' WS+ PAGE WS* DASH WS* PAGE)
/// PAGE := [0-9]+
/// DASH := '-' | '–' | '—'
/// WS := space | tab
/// ```
///
/// A leading `CAPCO-2016` token is not required by the grammar — the
/// `§` symbol is the unambiguous anchor. This is intentional: about a
/// third of in-source citations are written `§H.5 p99` without the
/// document prefix because it's implicit in the codebase.
///
/// Rejection of bare `§NN` (no subsection letter) is performed here
/// at the parser level: a `§` followed directly by digits without an
/// intermediate `[A-K] '.'` is recorded as a `BareSection` defect
/// candidate (the resolver still gets a chance at it for richer
/// diagnostics, but the section-letter test fails immediately).
///
/// Detection of the retired legacy `line NNNN` form lives in
/// `scanner.rs::find_legacy_line_form` because that pattern doesn't
/// share the `§`-prefixed grammar. Keeping it out of this parser
/// avoids polluting the citation grammar with a heuristic.
pub fn find_in_fragment(fragment: &str) -> Vec<CitationFind> {
    let bytes = fragment.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        // The `§` character is encoded as the two bytes 0xC2 0xA7 in UTF-8.
        // Match the full byte sequence rather than scanning byte-by-byte for
        // 0xA7 (which would mis-fire inside other multi-byte sequences).
        if bytes[i] == 0xC2 && i + 1 < bytes.len() && bytes[i + 1] == 0xA7 {
            let section_start = i;
            let after_section_sigil = i + 2;
            // Skip optional whitespace between § and the section letter.
            // Some source files write `§ H.5`; tolerate it.
            let mut j = after_section_sigil;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            if let Some((find, consumed_to)) = parse_after_sigil(fragment, section_start, j) {
                out.push(find);
                i = consumed_to;
                continue;
            }
            // No valid citation began here; advance past the sigil and
            // continue scanning. The bare-section / no-subsection cases
            // are handled inside `parse_after_sigil` and emit a find
            // with `BareSection`-classifiable shape; falling here means
            // there was no recognizable section letter or digit at all
            // (e.g., `§§` or `§ followed by punctuation`).
            i = after_section_sigil;
            continue;
        }
        i += 1;
    }
    out
}

/// What `find_in_fragment` returns for each occurrence: either a fully
/// parsed citation or a structural defect that the scanner should
/// surface (e.g., `§5 p99` with no subsection letter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationFind {
    Parsed {
        citation: Citation,
        /// Byte offset of `§` in the fragment.
        offset: usize,
    },
    /// A `§` followed by a bare number (no subsection letter). This is
    /// rejected: a bare section without a letter is ambiguous because
    /// the document evolves and the same number recurs across sections.
    BareSection { offset: usize, raw: String },
}

impl CitationFind {
    pub fn offset(&self) -> usize {
        match self {
            CitationFind::Parsed { offset, .. } | CitationFind::BareSection { offset, .. } => {
                *offset
            }
        }
    }
}

/// Parse what follows the `§` (and any intervening whitespace).
/// `start_offset` is the byte offset of `§` in `fragment`; `cursor` is
/// the offset of the first non-whitespace byte after `§`.
///
/// Returns the `CitationFind` and the byte offset just past the matched
/// citation (so the outer scanner can resume past it).
fn parse_after_sigil(
    fragment: &str,
    start_offset: usize,
    cursor: usize,
) -> Option<(CitationFind, usize)> {
    let bytes = fragment.as_bytes();
    if cursor >= bytes.len() {
        return None;
    }
    // Section letter must be ASCII A–Z. Range check against A–K
    // happens at the resolver layer so that out-of-range letters
    // (L, M, …) get the friendlier "non-normative or unknown
    // section" diagnostic instead of being silently dropped here.
    let section_byte = bytes[cursor];
    if !section_byte.is_ascii_alphabetic() || !section_byte.is_ascii_uppercase() {
        // Bare `§NN` form — no section letter. Capture the digits for
        // the diagnostic, then stop.
        if section_byte.is_ascii_digit() {
            let digit_start = cursor;
            let mut k = cursor;
            while k < bytes.len() && bytes[k].is_ascii_digit() {
                k += 1;
            }
            let raw_end = k;
            // Best-effort: include the bare digits in the raw, prefixed
            // by `§` for human readability.
            let raw = format!("§{}", &fragment[digit_start..raw_end]);
            return Some((
                CitationFind::BareSection {
                    offset: start_offset,
                    raw,
                },
                raw_end,
            ));
        }
        return None;
    }
    let section = section_byte as char;
    let after_letter = cursor + 1;
    let (subsection, after_subsection) =
        if after_letter < bytes.len() && bytes[after_letter] == b'.' {
            let after_dot = after_letter + 1;
            if after_dot < bytes.len() && bytes[after_dot].is_ascii_digit() {
                let mut k = after_dot;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                let n: u32 = fragment[after_dot..k].parse().ok()?;
                (Some(n), k)
            } else {
                // `§A.` with no digits — malformed; treat as bare letter
                // (we walked past the `.` but found no number).
                (None, after_letter)
            }
        } else {
            // `§A` (letter only). Lawful for sections without numbered
            // subsections; resolver handles the legality check.
            (None, after_letter)
        };
    // Look for an optional page anchor (only meaningful when we have
    // at least the section letter; either with or without subsection).
    let (pages, raw_end) = match parse_page_anchor(fragment, after_subsection) {
        Some((pages, end)) => (Some(pages), end),
        None => (None, after_subsection),
    };
    let raw = fragment[start_offset..raw_end].to_string();
    Some((
        CitationFind::Parsed {
            citation: Citation {
                section,
                subsection,
                pages,
                raw,
            },
            offset: start_offset,
        },
        raw_end,
    ))
}

/// Parse an optional page anchor starting at `cursor`. Returns
/// `Some((start, end), end_offset)` on match or `None` if no page
/// anchor is present.
fn parse_page_anchor(fragment: &str, cursor: usize) -> Option<((u32, u32), usize)> {
    let bytes = fragment.as_bytes();
    // Need at least one whitespace between the subsection digit and
    // the `p`-prefix; without it, expressions like `§H.5p99` are
    // not standard form and we don't recognize them. Project memory:
    // citations cite §X.Y pNN with a space separator.
    if cursor >= bytes.len() {
        return None;
    }
    let mut j = cursor;
    let saw_ws = j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t');
    while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
        j += 1;
    }
    if !saw_ws {
        return None;
    }
    if j >= bytes.len() || bytes[j] != b'p' {
        return None;
    }
    // `pp` (range) or `p` (single page)?
    let is_range = j + 1 < bytes.len() && bytes[j + 1] == b'p';
    let mut k = if is_range { j + 2 } else { j + 1 };
    // Optional whitespace between `pp` and the first page number.
    while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
        k += 1;
    }
    if k >= bytes.len() || !bytes[k].is_ascii_digit() {
        return None;
    }
    let first_start = k;
    while k < bytes.len() && bytes[k].is_ascii_digit() {
        k += 1;
    }
    let first: u32 = fragment[first_start..k].parse().ok()?;
    if !is_range {
        return Some(((first, first), k));
    }
    // Range: skip optional whitespace, then a dash (-, –, —), then optional whitespace.
    while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
        k += 1;
    }
    let dash_consumed = consume_dash(bytes, k)?;
    let mut k = dash_consumed;
    while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
        k += 1;
    }
    if k >= bytes.len() || !bytes[k].is_ascii_digit() {
        return None;
    }
    let second_start = k;
    while k < bytes.len() && bytes[k].is_ascii_digit() {
        k += 1;
    }
    let second: u32 = fragment[second_start..k].parse().ok()?;
    Some(((first, second), k))
}

/// Consume an ASCII hyphen, en-dash, or em-dash. Returns the byte
/// offset just past the dash, or `None` if no dash is present.
fn consume_dash(bytes: &[u8], cursor: usize) -> Option<usize> {
    if cursor >= bytes.len() {
        return None;
    }
    if bytes[cursor] == b'-' {
        return Some(cursor + 1);
    }
    // En-dash (U+2013): 0xE2 0x80 0x93. Em-dash (U+2014): 0xE2 0x80 0x94.
    if cursor + 2 < bytes.len()
        && bytes[cursor] == 0xE2
        && bytes[cursor + 1] == 0x80
        && (bytes[cursor + 2] == 0x93 || bytes[cursor + 2] == 0x94)
    {
        return Some(cursor + 3);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed_only(input: &str) -> Vec<Citation> {
        find_in_fragment(input)
            .into_iter()
            .filter_map(|f| match f {
                CitationFind::Parsed { citation, .. } => Some(citation),
                CitationFind::BareSection { .. } => None,
            })
            .collect()
    }

    #[test]
    fn single_page_form() {
        let v = parsed_only("CAPCO-2016 §H.5 p99");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section, 'H');
        assert_eq!(v[0].subsection, Some(5));
        assert_eq!(v[0].pages, Some((99, 99)));
    }

    #[test]
    fn page_range_with_en_dash() {
        let v = parsed_only("§H.7 pp 122–130");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].pages, Some((122, 130)));
    }

    #[test]
    fn page_range_with_hyphen() {
        let v = parsed_only("§H.7 pp 122-130");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].pages, Some((122, 130)));
    }

    #[test]
    fn no_page_anchor() {
        let v = parsed_only("§G.1 Table 4");
        assert_eq!(v.len(), 1);
        assert!(v[0].pages.is_none());
    }

    #[test]
    fn multiple_in_one_string() {
        let v = parsed_only("§D.2 Table 3 + §H.8 p145");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].section, 'D');
        assert_eq!(v[0].subsection, Some(2));
        assert_eq!(v[1].section, 'H');
        assert_eq!(v[1].pages, Some((145, 145)));
    }

    #[test]
    fn bare_section_form_is_flagged() {
        let v = find_in_fragment("§4 p99");
        assert_eq!(v.len(), 1);
        match &v[0] {
            CitationFind::BareSection { raw, .. } => assert_eq!(raw, "§4"),
            CitationFind::Parsed { .. } => panic!("expected BareSection, got {:?}", v[0]),
        }
    }

    #[test]
    fn section_letter_with_no_subsection() {
        // Letter-only references are now Parsed with `subsection: None`.
        // The resolver decides whether the cited section permits a
        // letter-only reference.
        let v = parsed_only("§A foo bar");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section, 'A');
        assert!(v[0].subsection.is_none());
    }

    #[test]
    fn ignores_section_in_prose() {
        let v = parsed_only("see §H.4 p64 — HCS-O Relationship(s) blah");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section, 'H');
        assert_eq!(v[0].subsection, Some(4));
        assert_eq!(v[0].pages, Some((64, 64)));
    }

    #[test]
    fn out_of_range_letter_is_still_parsed() {
        // L is out of A–K but the parser doesn't enforce that — the
        // resolver does. Verify the parser doesn't drop it silently.
        let v = parsed_only("§L.3 p10");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section, 'L');
    }

    #[test]
    fn lowercase_section_rejected() {
        // Lowercase section letter is a malformed citation — neither a
        // BareSection nor a parsed match. We surface nothing because
        // there's no actionable diagnostic for a lowercase letter that
        // doesn't match the project's citation convention.
        let v = find_in_fragment("§h.5 p99");
        assert!(v.is_empty(), "expected nothing, got {v:?}");
    }

    #[test]
    fn space_after_sigil_tolerated() {
        let v = parsed_only("§ H.5 p99");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section, 'H');
    }

    #[test]
    fn raw_text_preserved() {
        let v = parsed_only("§H.5 p99");
        assert_eq!(v[0].raw, "§H.5 p99");
        let v = parsed_only("§K.2");
        assert_eq!(v[0].raw, "§K.2");
    }

    #[test]
    fn back_to_back_citations() {
        // Make sure the scanner advances past each match cleanly.
        let v = parsed_only("§A.1 p1§B.2 p2");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].section, 'A');
        assert_eq!(v[1].section, 'B');
    }
}
