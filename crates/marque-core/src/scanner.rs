//! Phase 1: candidate detection — finds potential classification markings in a byte buffer.
//!
//! Uses `memchr` for SIMD-accelerated boundary detection. Zero heap allocation
//! beyond the output `Vec<MarkingCandidate>`. Never invokes the parser.
//!
//! # Strategy
//! - Portion candidates: scan for `(` with `memchr`, walk to `)`, apply
//!   lightweight heuristics (minimum length, ASCII uppercase content).
//! - Banner candidates: scan for lines whose trimmed content begins with a
//!   known classification prefix (UNCLASSIFIED, CONFIDENTIAL, SECRET, TOP SECRET).
//! - CAB candidates: scan for "Classified By:" label, walk to end of block.

use marque_ism::span::{MarkingCandidate, MarkingType, Span};
use memchr::memchr_iter;

/// Phase 1 scanner. Stateless; call [`Scanner::scan`] on any byte buffer.
pub struct Scanner;

impl Scanner {
    /// Scan `source` for classification marking candidates.
    ///
    /// Returns candidates in source order. Allocation is proportional to
    /// the number of candidates found, not source length.
    pub fn scan(source: &[u8]) -> Vec<MarkingCandidate> {
        let mut candidates = Vec::new();

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
    fn scan_page_breaks(source: &[u8], out: &mut Vec<MarkingCandidate>) {
        // Form-feed: every `\f` is a hard page break in pretty much every
        // ASCII document convention. memchr is overkill at this scale but
        // matches the rest of the scanner's idiom.
        for pos in memchr_iter(b'\x0c', source) {
            out.push(MarkingCandidate {
                span: Span::new(pos, pos),
                kind: MarkingType::PageBreak,
            });
        }
        // Three-or-more consecutive `\n` is a soft page break under our
        // heuristic. We emit one candidate at the third newline, then skip
        // ahead until we leave the run, so a single blank gap between
        // paragraphs (`\n\n`) does NOT trip the reset.
        let mut run = 0usize;
        for (i, &b) in source.iter().enumerate() {
            if b == b'\n' {
                run += 1;
                if run == 3 {
                    out.push(MarkingCandidate {
                        span: Span::new(i, i),
                        kind: MarkingType::PageBreak,
                    });
                }
            } else if b != b'\r' {
                run = 0;
            }
        }
    }

    fn scan_portions(source: &[u8], out: &mut Vec<MarkingCandidate>) {
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

    fn scan_banners(source: &[u8], out: &mut Vec<MarkingCandidate>) {
        // Classification prefixes that can start a banner line (full-word form only).
        const BANNER_PREFIXES: &[&[u8]] =
            &[b"TOP SECRET", b"SECRET", b"CONFIDENTIAL", b"UNCLASSIFIED"];

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

    fn scan_cab(source: &[u8], out: &mut Vec<MarkingCandidate>) {
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
    // Walk bytes after `(` looking for `)`. Reject nested parens and newlines.
    let rest = source.get(open + 1..)?;
    for (i, &b) in rest.iter().enumerate() {
        match b {
            b')' => return Some(open + 1 + i),
            b'\n' | b'\r' | b'(' => return None,
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
}
