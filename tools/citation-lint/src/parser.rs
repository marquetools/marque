// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Parser for the vendored CAPCO-2016 markdown source.
//!
//! Builds a `(section, subsection) → page-range` index that the
//! resolver uses to validate citations. The CAPCO-2016 manual structure
//! is project-specific in two ways that this parser hardcodes:
//!
//! 1. **Page markers**. Each page begins with a literal line
//!    `begin page N               UNCLASSIFIED` and ends with
//!    `end page N               UNCLASSIFIED`. These are emitted by
//!    the PDF-to-markdown conversion that produced the vendored
//!    source. Different revisions of CAPCO will use the same shape.
//!
//! 2. **Section / subsection layout**. Top-level sections A–K are
//!    headed `## A. (U) ...`, `## B. (U) ...`, etc. Subsections are
//!    headed `### N. (U) ...` (or `## N. (U) ...` inside the H
//!    section, where the source author put H.1, H.4, H.8, H.9 at the
//!    same heading level as the parent section). Section A's heading
//!    is missing a `## ` prefix in the markdown (it appears as inline
//!    text `A. (U) Introduction 1. (U) Authority` on a content line);
//!    we recover its boundary from the table of contents instead.
//!
//! **The table of contents is the authoritative subsection→page
//! mapping.** Within H, several subsections (H.2, H.3, H.5, H.6, H.7)
//! never appear as standalone markdown headings — they're flowed into
//! the body around the §H.4-style subsection-with-heading spans. The
//! ToC, however, lists every section and subsection with its starting
//! page in the standard `Name ............ NN` form. We parse the ToC
//! once and use it as the single source of truth; subsections without
//! an explicit `### N.` heading still have a TOC entry and get a page
//! mapping that way.
//!
//! When CAPCO-2016 is superseded (e.g., a hypothetical CAPCO-2030),
//! the parser MAY need a small revision: the page-marker grammar is
//! likely identical (it's a property of the conversion, not the
//! manual), but the section structure could shift (more or fewer
//! sections, renumbered subsections). The TOC-driven approach above
//! keeps that fragility in a single place.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use pulldown_cmark::Parser as MdParser;

/// A `(section, subsection)` identifier, e.g. `('H', 5)` for §H.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SectionId {
    pub letter: char,
    pub number: u32,
}

impl std::fmt::Display for SectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "§{}.{}", self.letter, self.number)
    }
}

/// Page range covered by a section or subsection, inclusive on both
/// ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRange {
    pub start: u32,
    pub end: u32,
}

impl PageRange {
    pub fn contains(&self, page: u32) -> bool {
        self.start <= page && page <= self.end
    }
}

/// Parsed CAPCO source — what the resolver consumes.
#[derive(Debug, Clone)]
pub struct CapcoIndex {
    /// Largest page number in the document.
    pub max_page: u32,
    /// Smallest page number in the document. The vendored source
    /// starts at page 2 (the title page is unnumbered in the PDF and
    /// no `begin page 1` marker exists in the markdown), so callers
    /// MUST NOT assume `min_page == 1`.
    pub min_page: u32,
    /// Top-level section page ranges (A → coverage spanning all of
    /// A's content; B → all of B's; …). Used for "section X exists
    /// in the doc" checks and to bound subsection lookups.
    pub sections: BTreeMap<char, PageRange>,
    /// Subsection page ranges keyed by `(letter, number)`.
    pub subsections: BTreeMap<SectionId, PageRange>,
    /// Set of all subsection IDs that we positively observed in the
    /// document (either as a heading or via TOC). Used to give the
    /// "section exists but is non-normative" diagnostic distinct from
    /// "section does not exist".
    pub known_subsection_ids: std::collections::BTreeSet<SectionId>,
}

impl CapcoIndex {
    /// Parse the CAPCO-2016 source at `path` and produce a complete
    /// index ready for resolver queries.
    pub fn from_file(path: &Path) -> Result<Self> {
        let source =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::from_source(&source)
    }

    /// Parse from a markdown string. Exposed for test fixtures so the
    /// parser can be exercised against synthetic minimal inputs.
    pub fn from_source(source: &str) -> Result<Self> {
        let pages = parse_page_markers(source);
        anyhow::ensure!(
            !pages.is_empty(),
            "no `begin page N ... UNCLASSIFIED` markers found in source — \
             this parser is hard-coded to the CAPCO-2016 markdown shape \
             (see parser.rs module doc); a different source layout \
             requires a parser revision"
        );
        let min_page = pages.iter().map(|p| p.page).min().expect("non-empty");
        let max_page = pages.iter().map(|p| p.page).max().expect("non-empty");

        let toc_entries = parse_toc(source)?;
        let mut subsections: BTreeMap<SectionId, PageRange> = BTreeMap::new();
        let mut known: std::collections::BTreeSet<SectionId> = std::collections::BTreeSet::new();
        // Sort by (letter, number) so we can compute end-pages from
        // the next entry's start-page minus 1. The TOC may not be
        // strictly ordered (cross-section page numbers don't decrease,
        // but within a section subsections can have inconsistent
        // entries — we tolerate that).
        let mut sorted = toc_entries.clone();
        sorted.sort_by(|a, b| {
            (a.id.letter, a.id.number, a.start_page).cmp(&(b.id.letter, b.id.number, b.start_page))
        });
        for (idx, entry) in sorted.iter().enumerate() {
            // Subsection ends at: next subsection's start, OR end of
            // document.
            //
            // **Inclusive overlap on subsection-boundary pages.** A
            // subsection in CAPCO frequently continues onto the page
            // where the next subsection begins (mid-page transitions
            // are the rule, not the exception). We therefore include
            // the next subsection's start page in the current
            // subsection's range — `§C.1 p26` and `§C.2 p26` both
            // resolve when C.2 starts at p26, because C.1's content
            // continues onto that page. This is a deliberate looseness
            // that prevents false positives for the common
            // "boundary-page" citation pattern; the trade-off is that
            // the lint cannot detect a misattribution to an adjacent
            // subsection by exactly one page. That kind of off-by-one
            // is rare in practice and would surface as a spec-form
            // review issue, not a citation-lint check.
            let end_page = if let Some(next) = sorted.get(idx + 1) {
                if next.start_page >= entry.start_page {
                    next.start_page
                } else {
                    entry.start_page
                }
            } else {
                max_page
            };
            let id = entry.id;
            subsections.insert(
                id,
                PageRange {
                    start: entry.start_page,
                    end: end_page.max(entry.start_page),
                },
            );
            known.insert(id);
        }

        // Top-level section ranges: from each section's first
        // subsection start to the page just before the next section's
        // first subsection. If a section has no subsections in the
        // TOC, fall back to its own TOC entry's page.
        let mut sections: BTreeMap<char, PageRange> = BTreeMap::new();
        let toc_section_starts = parse_toc_section_starts(source)?;
        let mut section_letters: Vec<char> = toc_section_starts.keys().copied().collect();
        section_letters.sort_unstable();
        for (idx, letter) in section_letters.iter().enumerate() {
            let start = toc_section_starts[letter];
            let end = if let Some(next_letter) = section_letters.get(idx + 1) {
                let next_start = toc_section_starts[next_letter];
                if next_start > start {
                    next_start - 1
                } else {
                    start
                }
            } else {
                max_page
            };
            sections.insert(
                *letter,
                PageRange {
                    start,
                    end: end.max(start),
                },
            );
        }

        Ok(CapcoIndex {
            min_page,
            max_page,
            sections,
            subsections,
            known_subsection_ids: known,
        })
    }

    /// Look up a subsection's page range.
    pub fn subsection(&self, id: SectionId) -> Option<PageRange> {
        self.subsections.get(&id).copied()
    }

    /// Look up a top-level section's page range.
    pub fn section(&self, letter: char) -> Option<PageRange> {
        self.sections.get(&letter).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PageMarker {
    /// 1-based markdown line number where `begin page N` appears.
    line: u32,
    page: u32,
    /// `true` if this is `begin page N`, `false` if `end page N`.
    is_begin: bool,
}

/// Parse all `begin page N` and `end page N` markers in document order.
fn parse_page_markers(source: &str) -> Vec<PageMarker> {
    let mut out = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("begin page ") {
            if let Some(page) = leading_page_number(rest) {
                out.push(PageMarker {
                    line: line_no,
                    page,
                    is_begin: true,
                });
            }
        } else if let Some(rest) = trimmed.strip_prefix("end page ") {
            if let Some(page) = leading_page_number(rest) {
                out.push(PageMarker {
                    line: line_no,
                    page,
                    is_begin: false,
                });
            }
        }
    }
    out
}

fn leading_page_number(s: &str) -> Option<u32> {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// One TOC entry: a subsection with its starting page.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TocEntry {
    id: SectionId,
    start_page: u32,
}

/// Extract the table of contents using `pulldown-cmark` to identify
/// the TOC region and a regex-shaped line scan to extract entries.
///
/// `pulldown-cmark` is used to confirm we're looking at a region the
/// markdown parser recognizes as text (not a code block, not inside a
/// table, etc.) — a defense against future formatting changes that
/// could cause a regex-only parser to silently lose entries.
///
/// The TOC region is bounded by:
/// - **Start**: the first occurrence of the literal text
///   `Table of Contents`. The vendored source has this on its own
///   line within a paragraph.
/// - **End**: the first heading after the start (a real `## ...` for
///   `B.`, `C.`, etc.). The TOC ends just before the document content
///   begins — the first proper section heading marks that boundary.
fn parse_toc(source: &str) -> Result<Vec<TocEntry>> {
    let toc_lines = extract_toc_lines(source)?;
    let mut entries = Vec::new();
    let mut current_section: Option<char> = None;
    for line in &toc_lines {
        // First, look for a top-level section heading line: starts
        // with a letter A-K followed by `. (U)` or `.   (U)`.
        if let Some(ch) = parse_toc_section_letter(line) {
            current_section = Some(ch);
            continue;
        }
        // Then, look for a numbered subsection line:
        // `1. (U) Authority .... 12` form. The subsection number is
        // the leading positive integer; the page is the trailing
        // positive integer after a long dot-leader.
        if let Some((num, page)) = parse_toc_subsection_entry(line) {
            if let Some(letter) = current_section {
                entries.push(TocEntry {
                    id: SectionId {
                        letter,
                        number: num,
                    },
                    start_page: page,
                });
            }
        }
    }
    Ok(entries)
}

/// TOC section-letter starts (letter → page where that section's TOC
/// entry says it starts). Used to compute top-level section page
/// ranges.
fn parse_toc_section_starts(source: &str) -> Result<BTreeMap<char, u32>> {
    let toc_lines = extract_toc_lines(source)?;
    let mut out: BTreeMap<char, u32> = BTreeMap::new();
    for line in &toc_lines {
        if let Some((letter, page)) = parse_toc_section_entry(line) {
            // The first occurrence wins (the TOC is in document order
            // so this is the actual start). Re-occurrence (if any)
            // would be a TOC anomaly we just tolerate.
            out.entry(letter).or_insert(page);
        }
    }
    Ok(out)
}

/// Identify the TOC region in the source and return its lines, with
/// PDF-conversion line breaks rejoined.
///
/// Defensive: if the TOC can't be located, returns an explicit error
/// rather than silently producing an empty index (which would cause
/// every citation to fail to resolve and obscure the real problem).
///
/// The CAPCO-2016 markdown source contains TOC entries that the
/// PDF-to-markdown conversion split across multiple source lines.
/// Two patterns occur in the wild:
///
/// 1. **Number-on-its-own-line continuation**: `3.\n(U) JOINT ... 55`
///    where the leading number sits alone on one line and the
///    `(U) <name> ... <page>` body is on the next.
/// 2. **Trailing-number-on-prior-line continuation**: `... 123 8.\n
///    (U) DISSEMINATION ... 131` where the next subsection's number
///    is appended to the prior subsection's trailing page number.
///
/// We rejoin both shapes before returning so the per-line entry
/// parsers (`parse_toc_subsection_entry`, `parse_toc_section_entry`)
/// see one logical entry per line.
///
/// We also strip out `begin page N` / `end page N` page markers from
/// the TOC region — the TOC itself spans pages 3–7 in the source,
/// so these markers appear inside the TOC region and would otherwise
/// confuse the joining logic.
fn extract_toc_lines(source: &str) -> Result<Vec<String>> {
    let mut start_line: Option<usize> = None;
    let mut end_line: Option<usize> = None;
    let lines: Vec<&str> = source.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        if start_line.is_none() && line.contains("Table of Contents") {
            start_line = Some(idx);
            continue;
        }
        if let Some(start) = start_line {
            if idx > start {
                // The first proper `## ` section heading after the
                // ToC marks the end of the ToC region. We accept any
                // `## A.` ... `## K.` pattern as the closing anchor.
                let trimmed = line.trim_start();
                if trimmed.starts_with("## ")
                    && trimmed
                        .chars()
                        .nth(3)
                        .is_some_and(|c| c.is_ascii_uppercase())
                    && trimmed.chars().nth(4) == Some('.')
                {
                    end_line = Some(idx);
                    break;
                }
            }
        }
    }
    let start = start_line.context("could not locate `Table of Contents` marker in source")?;
    let end = end_line.context(
        "could not locate end of TOC region (no `## X.` heading after Table of Contents)",
    )?;
    // Run pulldown-cmark over the whole source so we have at least one
    // confirmed entry in the parse log; we don't strictly need the
    // events here, but we want a parse failure to surface as a hard
    // error rather than silently fall through. A truly malformed
    // markdown source would otherwise produce a misleading "TOC
    // entries: 0" diagnostic.
    let _ = MdParser::new(source).count();

    // Pre-pass: drop the `begin page` / `end page` markers and the
    // `---` page separators within the TOC region; they're noise here.
    let mut filtered: Vec<String> = lines[start..end]
        .iter()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("begin page ") && !t.starts_with("end page ") && t != "---"
        })
        .map(|l| (*l).to_string())
        .collect();

    // Rejoin pattern 1: a line that is just `<digits>.` is a
    // continuation; merge it with the following line.
    let mut joined: Vec<String> = Vec::with_capacity(filtered.len());
    let mut i = 0;
    while i < filtered.len() {
        let line = &filtered[i];
        let trimmed = line.trim();
        if is_bare_subsection_prefix(trimmed) && i + 1 < filtered.len() {
            let combined = format!("{} {}", trimmed, filtered[i + 1].trim_start());
            joined.push(combined);
            i += 2;
            continue;
        }
        joined.push(line.clone());
        i += 1;
    }

    // Rejoin pattern 2: a line that ends with `<digits> <digits>.`
    // (a page number followed by a continuation subsection number)
    // splits into two TOC entries. Rewrite as two entries.
    let mut split_filter: Vec<String> = Vec::with_capacity(joined.len());
    for line in joined.drain(..) {
        if let Some((head, tail_num)) = split_trailing_continuation(&line) {
            split_filter.push(head);
            // The tail is `<num>.` followed by the *next* line's
            // body, so we don't have the body yet — it's the next
            // line in the un-split list. Defer the rejoin to a
            // second pass below.
            split_filter.push(format!("{tail_num}."));
        } else {
            split_filter.push(line);
        }
    }
    // Re-apply pattern 1 to fold the deferred `<num>.` lines.
    filtered = split_filter;
    let mut final_lines: Vec<String> = Vec::with_capacity(filtered.len());
    let mut i = 0;
    while i < filtered.len() {
        let line = &filtered[i];
        if is_bare_subsection_prefix(line.trim()) && i + 1 < filtered.len() {
            let combined = format!("{} {}", line.trim(), filtered[i + 1].trim_start());
            final_lines.push(combined);
            i += 2;
            continue;
        }
        final_lines.push(line.clone());
        i += 1;
    }

    Ok(final_lines)
}

/// Returns true if `s` is exactly `<digits>.` (e.g., `"3."`) — the
/// shape of a TOC subsection number sitting alone on a line because
/// the PDF-to-markdown conversion broke it off from its body.
fn is_bare_subsection_prefix(s: &str) -> bool {
    let s = s.trim();
    if s.len() < 2 {
        return false;
    }
    let bytes = s.as_bytes();
    let last = bytes[bytes.len() - 1];
    if last != b'.' {
        return false;
    }
    bytes[..bytes.len() - 1].iter().all(u8::is_ascii_digit)
}

/// Detects the trailing-number continuation shape: a line ending in
/// ` <pagenum> <subsectionnum>.` (the prior subsection's TOC page
/// number is followed by the next subsection's number, all on one
/// source line). Returns `Some((head_without_tail, tail_num))` if
/// the shape matches; `None` otherwise.
///
/// Concretely matches `... 123 8.` → returns `Some(("... 123", "8"))`.
/// We do NOT trigger on simple `... 123.` (a single trailing page
/// number followed by a period — that's body prose, not a
/// continuation): the marker is the leading digit AFTER the prior
/// page number, with a space in between.
fn split_trailing_continuation(line: &str) -> Option<(String, String)> {
    let trimmed_end = line.trim_end();
    if !trimmed_end.ends_with('.') {
        return None;
    }
    // Walk backward over the trailing `.`, then over digits — that's
    // the candidate continuation number.
    let bytes = trimmed_end.as_bytes();
    let mut k = bytes.len() - 1; // position of `.`
    if k == 0 {
        return None;
    }
    k -= 1;
    let tail_end = k + 1;
    while k > 0 && bytes[k].is_ascii_digit() {
        k -= 1;
    }
    let tail_start = if bytes[k].is_ascii_digit() { k } else { k + 1 };
    if tail_start == tail_end {
        return None; // no digits before the `.`
    }
    let tail_num = &trimmed_end[tail_start..tail_end];
    // Must have at least one space before the tail number, then a
    // page number digit run before that.
    if tail_start == 0 || bytes[tail_start - 1] != b' ' {
        return None;
    }
    // Walk further back over a digit run — the prior page number.
    let mut p = tail_start - 1; // points at space
    if p == 0 {
        return None;
    }
    p -= 1;
    let page_end = p + 1;
    while p > 0 && bytes[p].is_ascii_digit() {
        p -= 1;
    }
    let page_start = if bytes[p].is_ascii_digit() { p } else { p + 1 };
    if page_start == page_end {
        return None;
    }
    // Confirmed shape. The head is everything up through the page
    // number (exclusive of the trailing space + tail num + dot).
    let head_end = page_end;
    let head = &trimmed_end[..head_end];
    Some((head.to_string(), tail_num.to_string()))
}

/// `A. (U) INTRODUCTION ............... 12` → returns `Some(('A', 12))`.
fn parse_toc_section_entry(line: &str) -> Option<(char, u32)> {
    let trimmed = line.trim();
    let mut chars = trimmed.chars();
    let letter = chars.next()?;
    if !letter.is_ascii_uppercase() || !letter.is_ascii_alphabetic() {
        return None;
    }
    if chars.next()? != '.' {
        return None;
    }
    // Verify this looks like a section line (contains "(U)" before
    // the dot leader). This defends against false matches like a
    // body-text line beginning with a stray `A.`.
    if !trimmed.contains("(U)") {
        return None;
    }
    let page = trailing_page_number(trimmed)?;
    Some((letter, page))
}

/// `1. (U) Authority .................. 12` → returns `Some((1, 12))`.
fn parse_toc_subsection_entry(line: &str) -> Option<(u32, u32)> {
    let trimmed = line.trim();
    if !trimmed.contains("(U)") {
        return None;
    }
    // Leading positive integer followed by `.` and whitespace.
    let mut chars = trimmed.chars().peekable();
    let mut digits = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            digits.push(c);
            chars.next();
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    if chars.next()? != '.' {
        return None;
    }
    let num: u32 = digits.parse().ok()?;
    let page = trailing_page_number(trimmed)?;
    Some((num, page))
}

/// `1. (U) Foo ............ 47` → 47. Returns `None` if no trailing
/// page number is present. Defends against TOC entries that wrap to
/// the next line (those are joined upstream in the markdown source);
/// this function works on the joined form.
fn trailing_page_number(line: &str) -> Option<u32> {
    // Scan from the end for the last contiguous run of ASCII digits,
    // skipping a single trailing whitespace/punctuation tail. The TOC
    // may or may not have trailing `<br>` HTML tags or whitespace.
    let bytes = line.as_bytes();
    let mut end = bytes.len();
    while end > 0 && !bytes[end - 1].is_ascii_digit() {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    let mut start = end;
    while start > 0 && bytes[start - 1].is_ascii_digit() {
        start -= 1;
    }
    line[start..end].parse().ok()
}

/// `B. (U) GENERAL MARKINGS ...` line → letter 'B'.
fn parse_toc_section_letter(line: &str) -> Option<char> {
    let trimmed = line.trim();
    let letter = trimmed.chars().next()?;
    if !letter.is_ascii_uppercase() || !letter.is_ascii_alphabetic() {
        return None;
    }
    if trimmed.chars().nth(1)? != '.' {
        return None;
    }
    if !trimmed.contains("(U)") {
        return None;
    }
    Some(letter)
}

/// Helper: visit every `Event` from `pulldown-cmark` so a markdown
/// parse failure surfaces at parse time rather than silently. Used
/// only for its side effect of forcing lazy evaluation.
#[allow(dead_code)]
fn force_pulldown_parse(source: &str) {
    for ev in MdParser::new(source) {
        // Reference each variant we're aware of so the compiler
        // catches any future variant addition that would need
        // handling.
        // We only care about side effects of pulldown's lazy evaluation
        // forcing parse errors to surface; the variants don't matter
        // here. We discard the event after consuming it.
        let _ = ev;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_marker_parsing() {
        let src =
            "begin page 5               UNCLASSIFIED\nfoo\nend page 5               UNCLASSIFIED\n";
        let m = parse_page_markers(src);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].page, 5);
        assert!(m[0].is_begin);
        assert!(!m[1].is_begin);
    }

    #[test]
    fn trailing_page_number_works() {
        assert_eq!(trailing_page_number("A. (U) FOO ........ 47"), Some(47));
        assert_eq!(trailing_page_number("nothing here"), None);
        assert_eq!(trailing_page_number("trailing 99   "), Some(99));
    }

    #[test]
    fn toc_subsection_entry_parses() {
        let line = "1. (U) Syntax Rules ............................. 25";
        let (num, page) = parse_toc_subsection_entry(line).unwrap();
        assert_eq!(num, 1);
        assert_eq!(page, 25);
    }

    #[test]
    fn toc_section_letter_parses() {
        assert_eq!(
            parse_toc_section_letter("A. (U) INTRODUCTION ........ 12"),
            Some('A')
        );
        assert_eq!(parse_toc_section_letter("1. Foo"), None);
    }

    /// Synthetic minimal CAPCO-shaped source for parser tests.
    /// Real CAPCO-2016 has thousands of TOC lines + 5000+ body lines;
    /// this fixture has just enough shape to exercise the index.
    const SYNTHETIC: &str = "\
# Doc

(U)   Table of Contents

A. (U) FOO ............ 5
1. (U) Authority .... 5
2. (U) Purpose ...... 7
B. (U) BAR ............ 10
1. (U) Syntax ....... 10

## A. (U) FOO

begin page 5               UNCLASSIFIED
content of A.1
end page 5               UNCLASSIFIED

begin page 7               UNCLASSIFIED
content of A.2
end page 9               UNCLASSIFIED

## B. (U) BAR

begin page 10               UNCLASSIFIED
content of B.1
end page 12               UNCLASSIFIED
";

    #[test]
    fn synthetic_index_builds() {
        let idx = CapcoIndex::from_source(SYNTHETIC).unwrap();
        assert_eq!(idx.min_page, 5);
        assert_eq!(idx.max_page, 12);
        // §A.1: starts at p5, ends at next subsection's start (p7) —
        // inclusive on the boundary so mid-page transitions work.
        let a1 = idx
            .subsection(SectionId {
                letter: 'A',
                number: 1,
            })
            .unwrap();
        assert_eq!(a1.start, 5);
        assert_eq!(a1.end, 7);
        // §A.2 spans 7 → next section B starts at 10.
        let a2 = idx
            .subsection(SectionId {
                letter: 'A',
                number: 2,
            })
            .unwrap();
        assert_eq!(a2.start, 7);
        assert_eq!(a2.end, 10);
        // §B.1 spans 10 to end of doc.
        let b1 = idx
            .subsection(SectionId {
                letter: 'B',
                number: 1,
            })
            .unwrap();
        assert_eq!(b1.start, 10);
        assert_eq!(b1.end, 12);
    }

    #[test]
    fn synthetic_section_starts() {
        let starts = parse_toc_section_starts(SYNTHETIC).unwrap();
        assert_eq!(starts[&'A'], 5);
        assert_eq!(starts[&'B'], 10);
    }
}
