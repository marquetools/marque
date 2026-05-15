// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 1: candidate detection — finds potential classification markings in a byte buffer.
//!
//! Uses `memchr` for SIMD-accelerated boundary detection. Zero heap allocation
//! for inputs producing ≤16 candidates; the output
//! `SmallVec<[MarkingCandidate; 16]>` keeps its buffer inline. Never invokes
//! the parser.
//!
//! # Strategy
//! - Portion candidates: scan for `(` with `memchr`, walk to `)`, apply
//!   lightweight heuristics (minimum length, ASCII uppercase content).
//! - Banner candidates: scan for lines whose trimmed content begins with a
//!   known classification prefix (UNCLASSIFIED, CONFIDENTIAL, SECRET, TOP SECRET).
//! - CAB candidates: scan for "Classified By:" label, walk to end of block.

use marque_ism::span::{MarkingCandidate, MarkingType, Span};
use memchr::memchr_iter;
use smallvec::SmallVec;

/// Phase 1 scanner. Stateless; call [`Scanner::scan`] on any byte buffer.
pub struct Scanner;

impl Scanner {
    /// Scan `source` for classification marking candidates.
    ///
    /// Returns candidates in source order. Zero heap allocation for the
    /// typical case (≤16 candidates); past 16, the `SmallVec` spills to a
    /// heap-allocated buffer proportional to the candidate count, not
    /// source length.
    pub fn scan(source: &[u8]) -> SmallVec<[MarkingCandidate; 16]> {
        let mut candidates: SmallVec<[MarkingCandidate; 16]> = SmallVec::new();

        Self::scan_portions(source, &mut candidates);
        Self::scan_banners(source, &mut candidates);
        Self::scan_cab(source, &mut candidates);
        Self::scan_page_breaks(source, &mut candidates);

        // Sort by `(start, kind_priority)`. PageBreak gets priority 0 so
        // it sorts before any content candidate at the same offset — the
        // engine's PageContext reset must run before a co-located banner
        // or portion is processed, otherwise the reset is defeated by an
        // unstable secondary order.
        candidates.sort_unstable_by(|a, b| {
            a.span
                .start
                .cmp(&b.span.start)
                .then_with(|| kind_sort_priority(a.kind).cmp(&kind_sort_priority(b.kind)))
        });
        candidates
    }

    /// Phase 3 — emit a `MarkingType::PageBreak` candidate at every form-feed
    /// (`\f`) byte and at the third consecutive `\n` of a `\n\n\n+` run.
    /// The engine uses these to reset `PageContext` so banner/CAB rules on
    /// the next page see a fresh aggregate.
    ///
    /// PageBreak spans are zero-length and carry no parsable content; the
    /// parser will reject them, so the engine must filter them out *before*
    /// calling `parser.parse`.
    fn scan_page_breaks(source: &[u8], out: &mut SmallVec<[MarkingCandidate; 16]>) {
        // Form-feed: every `\f` is a hard page break in pretty much every
        // ASCII document convention. `memchr` strides over the buffer via
        // SIMD; matches the rest of the scanner's idiom.
        for pos in memchr_iter(b'\x0c', source) {
            out.push(MarkingCandidate {
                span: Span::new(pos, pos),
                kind: MarkingType::PageBreak,
            });
        }
        // Three-or-more consecutive `\n` is a soft page break under our
        // heuristic. `memchr_iter` strides over newlines via SIMD; we use
        // the bytes between consecutive newlines to decide whether the run
        // continues. A gap of only `\r` bytes (covers `\r\n\r\n` CRLF runs)
        // counts as continuous; any other byte breaks the run, so a single
        // blank gap between paragraphs (`\n\n`) does NOT trip the reset.
        //
        // Run-emission semantics intentionally match the prior byte-iter
        // loop: we emit on the THIRD newline of a run (equality, not `>=`)
        // and do not reset after emit, so a longer run (`\n\n\n\n\n+`)
        // still emits exactly one PageBreak — at the third newline.
        let mut run = 0usize;
        let mut prev_pos = None;
        for pos in memchr_iter(b'\n', source) {
            let continuous = match prev_pos {
                Some(p) => source[p + 1..pos].iter().all(|&b| b == b'\r'),
                None => true,
            };
            if continuous {
                run += 1;
            } else {
                run = 1;
            }
            if run == 3 {
                out.push(MarkingCandidate {
                    span: Span::new(pos, pos),
                    kind: MarkingType::PageBreak,
                });
            }
            prev_pos = Some(pos);
        }
    }

    fn scan_portions(source: &[u8], out: &mut SmallVec<[MarkingCandidate; 16]>) {
        // Find every `(` and walk forward to the matching `)`.
        for start in memchr_iter(b'(', source) {
            if let Some(end) = find_portion_end(source, start) {
                let span = Span::new(start, end + 1);
                // Heuristic gate: minimum length `(U)` = 3, max reasonable = 256
                if span.len() >= 3 && span.len() <= 256 {
                    out.push(MarkingCandidate {
                        span,
                        kind: MarkingType::Portion,
                    });
                }
            }
        }
    }

    fn scan_banners(source: &[u8], out: &mut SmallVec<[MarkingCandidate; 16]>) {
        // Classification prefixes that can start a banner line.
        // Full-form US classifications are listed first. Abbreviated US forms
        // (`TS//`, `S//`, `C//`, `U//`) are included so rules like E001 (portion
        // abbreviation in banner context) can fire on abbreviated banners.
        // `//` detects non-US classifications (FGI, NATO, JOINT) where the
        // US classification slot is empty. `RESTRICTED` supports foreign-origin
        // markings with the RESTRICTED level.
        const BANNER_PREFIXES: &[&[u8]] = &[
            b"TOP SECRET",
            b"COSMIC TOP SECRET",
            b"TS//",
            b"SECRET",
            b"S//",
            b"CONFIDENTIAL",
            b"C//",
            b"RESTRICTED",
            b"UNCLASSIFIED",
            b"U//",
            b"//",
            // NATO longhand banner forms — the strict parser accepts
            // `NATO SECRET` / `COSMIC TOP SECRET` etc.; these prefixes
            // allow the decoder to recover abbreviated forms like
            // `NATO S` / `NATO TS` via `try_nato_fold`. Without this,
            // the decoder never sees these candidates.
            // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
            b"NATO ",
        ];

        for line in source.split(|&b| b == b'\n') {
            let trimmed = trim_ascii(line);
            if BANNER_PREFIXES.iter().any(|p| trimmed.starts_with(p)) {
                // `line` is a subslice produced by split(), so its pointer lies
                // within `source`. Subtraction yields the byte offset safely.
                let start = line.as_ptr() as usize - source.as_ptr() as usize;
                let end = start + line.len();
                out.push(MarkingCandidate {
                    span: Span::new(start, end),
                    kind: MarkingType::Banner,
                });
            }
        }
    }

    fn scan_cab(source: &[u8], out: &mut SmallVec<[MarkingCandidate; 16]>) {
        const CAB_LABEL: &[u8] = b"Classified By:";
        let mut search_from = 0;
        while let Some(rel) = find_subsequence(&source[search_from..], CAB_LABEL) {
            let pos = search_from + rel;
            let end = find_cab_end(source, pos);
            out.push(MarkingCandidate {
                span: Span::new(pos, end),
                kind: MarkingType::Cab,
            });
            search_from = end;
        }
    }
}

/// Sort priority for `MarkingCandidate` kinds at equal start offsets.
/// PageBreak sorts first so the engine's `PageContext` reset runs before
/// any co-located content candidate is processed (banner/portion/CAB at
/// the same byte offset as a page break — an edge case, but hardened).
fn kind_sort_priority(kind: MarkingType) -> u8 {
    match kind {
        MarkingType::PageBreak => 0,
        _ => 1,
    }
}

fn find_portion_end(source: &[u8], open: usize) -> Option<usize> {
    // Walk bytes after `(` looking for `)`. Reject anything that cannot
    // legitimately appear inside a single-line portion marking:
    //   - `\n` / `\r`: portion markings are always on a single line
    //   - `(`: nested parens are never valid
    //   - `\x0c` (form feed): a page-break control character cannot
    //     appear inside a portion. Rejecting it here keeps a
    //     PageBreak candidate from being shadowed by a spurious
    //     Portion that spans the form feed.
    let rest = source.get(open + 1..)?;
    for (i, &b) in rest.iter().enumerate() {
        match b {
            b')' => return Some(open + 1 + i),
            b'\n' | b'\r' | b'\x0c' | b'(' => return None,
            _ => {}
        }
    }
    None
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn find_cab_end(source: &[u8], start: usize) -> usize {
    // CAB ends at a blank line or EOF.
    let mut prev_newline = false;
    for (i, &b) in source[start..].iter().enumerate() {
        if b == b'\n' {
            if prev_newline {
                return start + i;
            }
            prev_newline = true;
        } else if b != b'\r' {
            prev_newline = false;
        }
    }
    source.len()
}

fn trim_ascii(s: &[u8]) -> &[u8] {
    // Use stdlib trim_ascii (stable since Rust 1.80) to strip all leading/trailing
    // ASCII whitespace including \r (handles CRLF line endings from split(b'\n')).
    s.trim_ascii()
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn detects_portion_marking() {
        let src = b"(TS//SI//NF) This paragraph is classified.";
        let candidates = Scanner::scan(src);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].kind, MarkingType::Portion);
        assert_eq!(candidates[0].span.as_str(src).unwrap(), "(TS//SI//NF)");
    }

    #[test]
    fn detects_banner() {
        let src = b"TOP SECRET//NOFORN\n\nSome content here.\n";
        let candidates = Scanner::scan(src);
        assert!(candidates.iter().any(|c| c.kind == MarkingType::Banner));
    }

    #[test]
    fn rejects_newline_in_portion() {
        let src = b"(TS\n//NF) not a real marking";
        let candidates = Scanner::scan(src);
        assert!(candidates.iter().all(|c| c.kind != MarkingType::Portion));
    }

    #[test]
    fn rejects_form_feed_in_portion() {
        // A `\f` inside `(...)` is never a valid single-line portion.
        // Without this rejection the portion candidate would span the
        // form feed and shadow the PageBreak candidate at that offset.
        let src = b"(TS\x0c//NF)";
        let candidates = Scanner::scan(src);
        assert!(
            candidates.iter().all(|c| c.kind != MarkingType::Portion),
            "form feed inside portion parens must not produce a Portion candidate"
        );
        // The PageBreak candidate at offset 3 should still be emitted.
        assert!(
            candidates
                .iter()
                .any(|c| c.kind == MarkingType::PageBreak && c.span.start == 3),
            "expected PageBreak at form-feed offset 3"
        );
    }

    #[test]
    fn detects_page_break_form_feed() {
        let src = b"page1\x0cpage2";
        let candidates = Scanner::scan(src);
        let breaks: Vec<_> = candidates
            .iter()
            .filter(|c| c.kind == MarkingType::PageBreak)
            .collect();
        assert_eq!(breaks.len(), 1);
        // Form feed sits at offset 5 in `b"page1\x0cpage2"`.
        assert_eq!(breaks[0].span.start, 5);
        assert_eq!(breaks[0].span.end, 5);
    }

    #[test]
    fn detects_page_break_blank_line_run() {
        let src = b"page1\n\n\npage2";
        let candidates = Scanner::scan(src);
        let breaks: Vec<_> = candidates
            .iter()
            .filter(|c| c.kind == MarkingType::PageBreak)
            .collect();
        // Exactly one PageBreak — emitted at the *third* newline (offset 7),
        // not one per `\n` in the run.
        assert_eq!(breaks.len(), 1);
        assert_eq!(breaks[0].span.start, 7);
    }

    #[test]
    fn double_newline_does_not_emit_page_break() {
        // A normal paragraph break (`\n\n`) must NOT trip the reset, otherwise
        // every paragraph in a multi-page document looks like a fresh page.
        let src = b"paragraph one\n\nparagraph two";
        let candidates = Scanner::scan(src);
        assert!(
            candidates.iter().all(|c| c.kind != MarkingType::PageBreak),
            "double newline should not produce a PageBreak candidate"
        );
    }

    #[test]
    fn detects_page_break_crlf_blank_line_run() {
        // CRLF-terminated paragraphs ("\r\n\r\n\r\n") are a three-newline run
        // where the inter-newline gaps are pure `\r` bytes. The scanner must
        // treat `\r`-only gaps as continuous so Windows-style line endings
        // produce the same PageBreak as Unix-style `\n\n\n`.
        //
        // Byte positions: p(0) a(1) g(2) e(3) 1(4) \r(5) \n(6) \r(7) \n(8)
        //                 \r(9) \n(10) p(11) ...
        // Third newline at offset 10.
        let src = b"page1\r\n\r\n\r\npage2";
        let candidates = Scanner::scan(src);
        let breaks: Vec<_> = candidates
            .iter()
            .filter(|c| c.kind == MarkingType::PageBreak)
            .collect();
        assert_eq!(breaks.len(), 1);
        assert_eq!(breaks[0].span.start, 10);
        assert_eq!(breaks[0].span.end, 10);
    }

    #[test]
    fn single_emit_on_six_newline_run() {
        // Regression guard: a longer run (six consecutive `\n`) must still
        // emit exactly ONE PageBreak, at the third newline. The equality
        // check on `run == 3` (not `>=`) is what preserves this property;
        // a careless "reset run after emit" refactor would emit twice on
        // a six-newline run. Pin the behavior so a future refactor that
        // introduces post-emit reset fails here.
        //
        // Byte positions: a(0) \n(1) \n(2) \n(3) \n(4) \n(5) \n(6) b(7).
        // Third newline at offset 3.
        let src = b"a\n\n\n\n\n\nb";
        let candidates = Scanner::scan(src);
        let breaks: Vec<_> = candidates
            .iter()
            .filter(|c| c.kind == MarkingType::PageBreak)
            .collect();
        assert_eq!(breaks.len(), 1);
        assert_eq!(breaks[0].span.start, 3);
        assert_eq!(breaks[0].span.end, 3);
    }

    #[test]
    fn empty_gap_between_newlines_counts_as_continuous() {
        // Adjacent newlines have an empty inter-newline gap. The
        // `\r`-transparency check (`source[p+1..pos].iter().all(|&b| b == b'\r')`)
        // returns vacuously `true` on an empty slice, which is the load-bearing
        // property that makes pure-LF `\n\n\n` emit a PageBreak. This pins the
        // vacuous-truth case explicitly so a future tighter predicate that
        // doesn't preserve it (e.g., `gap.is_empty() || gap.iter().all(...)`)
        // would still pass, but a buggier one (e.g., `!gap.is_empty() && ...`)
        // would fail here independently of the other page-break tests.
        let src = b"\n\n\n";
        let candidates = Scanner::scan(src);
        let breaks: Vec<_> = candidates
            .iter()
            .filter(|c| c.kind == MarkingType::PageBreak)
            .collect();
        assert_eq!(breaks.len(), 1);
        assert_eq!(breaks[0].span.start, 2);
        assert_eq!(breaks[0].span.end, 2);
    }

    #[test]
    fn page_break_sorts_before_co_located_content() {
        // Edge case: a banner line whose line start is at the same byte
        // offset as a form-feed candidate. The scanner emits both at
        // offset N — PageBreak (zero-length) and Banner (line span).
        // The sort must place PageBreak first so the engine reset runs
        // before the banner is processed.
        //
        // Construct `\fSECRET\n`: form-feed at 0, banner line 1..7.
        // The PageBreak lands at offset 0 with zero length; the banner
        // line scanner's offset is 1 (after the `\f`), so they are NOT
        // co-located in this case. Build a synthetic double-push case
        // by testing `kind_sort_priority` directly instead — simpler
        // and covers the sort key without fighting the scanner.
        assert_eq!(kind_sort_priority(MarkingType::PageBreak), 0);
        assert!(
            kind_sort_priority(MarkingType::PageBreak) < kind_sort_priority(MarkingType::Banner)
        );
        assert!(
            kind_sort_priority(MarkingType::PageBreak) < kind_sort_priority(MarkingType::Portion)
        );
        assert!(kind_sort_priority(MarkingType::PageBreak) < kind_sort_priority(MarkingType::Cab));
    }

    #[test]
    fn page_break_form_feed_inside_blank_run_emits_both() {
        // `\n\n\f\n\n` — the form feed itself is one PageBreak; the surrounding
        // newlines do not also trip the 3-newline heuristic because the run
        // is broken by the `\f`.
        let src = b"a\n\n\x0c\n\nb";
        let candidates = Scanner::scan(src);
        let breaks: Vec<_> = candidates
            .iter()
            .filter(|c| c.kind == MarkingType::PageBreak)
            .collect();
        assert_eq!(breaks.len(), 1, "only the form-feed should fire here");
    }

    // --- Non-US banner detection ---

    #[test]
    fn detects_non_us_banner_nato() {
        let src = b"//NATO SECRET//REL TO USA, GBR\n";
        let candidates = Scanner::scan(src);
        let banners: Vec<_> = candidates
            .iter()
            .filter(|c| c.kind == MarkingType::Banner)
            .collect();
        assert_eq!(banners.len(), 1);
    }

    #[test]
    fn detects_non_us_banner_portion_form() {
        let src = b"//NS//NF\n";
        let candidates = Scanner::scan(src);
        assert!(candidates.iter().any(|c| c.kind == MarkingType::Banner));
    }

    #[test]
    fn detects_restricted_banner() {
        let src = b"RESTRICTED//NF\n";
        let candidates = Scanner::scan(src);
        assert!(candidates.iter().any(|c| c.kind == MarkingType::Banner));
    }

    #[test]
    fn non_us_portion_detected_by_existing_scanner() {
        // Portions starting with (// should already be detected via `(`.
        let src = b"(//NS//REL TO USA, GBR)";
        let candidates = Scanner::scan(src);
        assert!(candidates.iter().any(|c| c.kind == MarkingType::Portion));
    }

    #[test]
    fn double_slash_mid_line_is_not_banner() {
        // `//` not at start of trimmed line should not produce a banner.
        let src = b"some text // not a marking\n";
        let candidates = Scanner::scan(src);
        assert!(
            candidates.iter().all(|c| c.kind != MarkingType::Banner),
            "// in middle of line should not produce a banner candidate"
        );
    }
}
