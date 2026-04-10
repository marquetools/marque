//! Byte-offset spans into source buffers — zero-copy position tracking.

/// A byte-offset span into the original source buffer.
/// Never owns data; always references the original input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[inline]
    pub fn new(start: usize, end: usize) -> Self {
        debug_assert!(start <= end, "span start must not exceed end");
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

    #[inline]
    pub fn as_slice<'a>(&self, source: &'a [u8]) -> &'a [u8] {
        &source[self.start..self.end]
    }

    #[inline]
    pub fn as_str<'a>(&self, source: &'a [u8]) -> &'a str {
        // SAFETY: scanner only produces spans over valid UTF-8 ASCII ranges
        std::str::from_utf8(self.as_slice(source)).expect("span must cover valid UTF-8")
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
}

/// A scanner-identified candidate with its type and source span.
#[derive(Debug, Clone, Copy)]
pub struct Candidate {
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
