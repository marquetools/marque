//! Phase 1: candidate detection — finds potential classification markings in a byte buffer.
//!
//! Uses `memchr` for SIMD-accelerated boundary detection. Zero heap allocation
//! beyond the output `Vec<Candidate>`. Never invokes the parser.
//!
//! # Strategy
//! - Portion candidates: scan for `(` with `memchr`, walk to `)`, apply
//!   lightweight heuristics (minimum length, ASCII uppercase content).
//! - Banner candidates: scan for lines whose trimmed content begins with a
//!   known classification prefix (UNCLASSIFIED, CONFIDENTIAL, SECRET, TOP SECRET).
//! - CAB candidates: scan for "Classified By:" label, walk to end of block.

use marque_ism::span::{Candidate, MarkingType, Span};
use memchr::memchr_iter;

/// Phase 1 scanner. Stateless; call [`Scanner::scan`] on any byte buffer.
pub struct Scanner;

impl Scanner {
    /// Scan `source` for classification marking candidates.
    ///
    /// Returns candidates in source order. Allocation is proportional to
    /// the number of candidates found, not source length.
    pub fn scan(source: &[u8]) -> Vec<Candidate> {
        let mut candidates = Vec::new();

        Self::scan_portions(source, &mut candidates);
        Self::scan_banners(source, &mut candidates);
        Self::scan_cab(source, &mut candidates);

        // Sort by span start for deterministic ordering.
        candidates.sort_unstable_by_key(|c| c.span.start);
        candidates
    }

    fn scan_portions(source: &[u8], out: &mut Vec<Candidate>) {
        // Find every `(` and walk forward to the matching `)`.
        for start in memchr_iter(b'(', source) {
            if let Some(end) = find_portion_end(source, start) {
                let span = Span::new(start, end + 1);
                // Heuristic gate: minimum length `(U)` = 3, max reasonable = 256
                if span.len() >= 3 && span.len() <= 256 {
                    out.push(Candidate {
                        span,
                        kind: MarkingType::Portion,
                    });
                }
            }
        }
    }

    fn scan_banners(source: &[u8], out: &mut Vec<Candidate>) {
        // Classification prefixes that can start a banner line (full-word form only).
        const BANNER_PREFIXES: &[&[u8]] =
            &[b"TOP SECRET", b"SECRET", b"CONFIDENTIAL", b"UNCLASSIFIED"];

        for line in source.split(|&b| b == b'\n') {
            let trimmed = trim_ascii(line);
            if BANNER_PREFIXES.iter().any(|p| trimmed.starts_with(p)) {
                // Compute span relative to full source buffer.
                let start = line.as_ptr() as usize - source.as_ptr() as usize;
                let end = start + line.len();
                out.push(Candidate {
                    span: Span::new(start, end),
                    kind: MarkingType::Banner,
                });
            }
        }
    }

    fn scan_cab(source: &[u8], out: &mut Vec<Candidate>) {
        const CAB_LABEL: &[u8] = b"Classified By:";
        if let Some(pos) = find_subsequence(source, CAB_LABEL) {
            // Walk forward to find end of CAB block (blank line or end of source).
            let end = find_cab_end(source, pos);
            out.push(Candidate {
                span: Span::new(pos, end),
                kind: MarkingType::Cab,
            });
        }
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
    let s = s.strip_prefix(b" ").unwrap_or(s);
    s.strip_suffix(b" ").unwrap_or(s)
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
        assert_eq!(candidates[0].span.as_str(src), "(TS//SI//NF)");
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
}
