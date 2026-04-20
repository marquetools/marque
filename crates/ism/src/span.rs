// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Byte-offset spans into source buffers — zero-copy position tracking.

/// A byte-offset span into the original source buffer.
/// Never owns data; always references the original input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Construct a span. Panics in both debug and release builds if
    /// `start > end`, because such a span will inevitably panic later
    /// at slice time and the early panic gives a better error message.
    #[inline]
    pub fn new(start: usize, end: usize) -> Self {
        assert!(
            start <= end,
            "Span::new: start ({start}) must not exceed end ({end})"
        );
        Self { start, end }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Borrow the span's bytes from `source`. Panics if the span is
    /// out of bounds for `source` — use [`Span::try_as_slice`] when the
    /// caller cannot guarantee bounds.
    #[inline]
    pub fn as_slice<'a>(&self, source: &'a [u8]) -> &'a [u8] {
        &source[self.start..self.end]
    }

    /// Borrow the span's bytes from `source`, returning `None` if the
    /// span lies outside the buffer instead of panicking.
    #[inline]
    pub fn try_as_slice<'a>(&self, source: &'a [u8]) -> Option<&'a [u8]> {
        source.get(self.start..self.end)
    }

    /// Extract the spanned bytes as a UTF-8 string slice.
    ///
    /// Returns `Err` if the span does not cover valid UTF-8.
    /// Callers that know the source is ASCII can use `.unwrap()` in tests
    /// or `.expect("...")` with context.
    #[inline]
    pub fn as_str<'a>(&self, source: &'a [u8]) -> Result<&'a str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_slice(source))
    }
}

/// Classification marking candidate type, determined by scanner heuristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkingType {
    /// `(TS//SI//NF)` — parenthesized, typically at paragraph start.
    Portion,
    /// `TOP SECRET//SENSITIVE INTELLIGENCE//NOFORN` — standalone line.
    Banner,
    /// Multi-line Classification Authority Block (Classified By / Derived From / Declassify On).
    Cab,
    /// Document page break — `\f` (form feed) or `\n\n\n+` heuristic.
    /// Carries a zero-length span at the boundary offset. The engine uses
    /// this to reset its `PageContext` so banner/CAB rules on the next page
    /// see a fresh aggregate (Phase 3, plan §Task 1).
    PageBreak,
}

/// A scanner-identified candidate with its type and source span.
#[derive(Debug, Clone, Copy)]
pub struct MarkingCandidate {
    pub span: Span,
    pub kind: MarkingType,
}

/// Document zone — where in the document structure a marking appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Header,
    Footer,
    Body,
    /// Classification Authority Block (Classified By / Derived From / Declassify On).
    Cab,
}

/// Coarse position within the document (for banner detection heuristics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentPosition {
    Start,
    Body,
    End,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_new_accepts_equal_bounds() {
        let s = Span::new(5, 5);
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn span_new_accepts_normal_range() {
        let s = Span::new(2, 7);
        assert!(!s.is_empty());
        assert_eq!(s.len(), 5);
    }

    #[test]
    #[should_panic(expected = "Span::new")]
    fn span_new_panics_on_inverted_bounds() {
        let _ = Span::new(7, 2);
    }

    #[test]
    fn try_as_slice_returns_none_when_out_of_bounds() {
        let buf = b"hello";
        let s = Span::new(2, 100);
        assert!(s.try_as_slice(buf).is_none());
    }

    #[test]
    fn try_as_slice_returns_bytes_when_in_bounds() {
        let buf = b"hello";
        let s = Span::new(1, 4);
        assert_eq!(s.try_as_slice(buf), Some(&b"ell"[..]));
    }

    #[test]
    fn as_str_returns_utf8_slice() {
        let buf = b"abc";
        let s = Span::new(0, 3);
        assert_eq!(s.as_str(buf).unwrap(), "abc");
    }

    #[test]
    fn span_is_empty_returns_true_when_bounds_are_equal() {
        let s = Span::new(42, 42);
        assert!(s.is_empty());
    }

    #[test]
    fn span_is_empty_returns_false_when_bounds_differ() {
        let s = Span::new(42, 43);
        assert!(!s.is_empty());
    }
}
