// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Byte-offset spans into source buffers — zero-copy position tracking.
//!
//! [`Span`] itself lives in [`marque_scheme::Span`] so the scheme
//! layer's [`marque_scheme::constraint::ConstraintViolation`] (and
//! other scheme-layer types) can carry source positions without
//! taking a dependency on `marque-ism`, which would violate
//! Constitution VII (`marque-scheme` is the only true graph leaf).
//! The re-export here keeps every existing `marque_ism::Span` import
//! site unchanged.

pub use marque_scheme::Span;

/// Classification marking candidate type, determined by scanner heuristics
/// (plus one engine-synthesized variant — see [`MarkingType::PageFinalization`]).
///
/// **`#[non_exhaustive]`** (issue #461): a future scanner enhancement
/// or engine pass may introduce additional variants (e.g., a sentence-
/// boundary candidate, a header-region marker). Marking the enum
/// `#[non_exhaustive]` means a future variant addition is a
/// non-breaking change for downstream consumers, which matters
/// because every exhaustive-match site in the workspace tracks every
/// variant by construction (rust-reviewer §1 surfaces every site
/// requiring a wildcard arm when this enum grows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    /// Engine-synthesized page-finalization boundary — dispatched at
    /// every [`MarkingType::PageBreak`] BEFORE the `PageContext` reset,
    /// plus once at end-of-document. Never emitted by the scanner.
    /// Carries a zero-length `Span` at the boundary offset.
    ///
    /// Parsers MUST reject this kind — only `marque_rules::Phase::PageFinalization`
    /// rules dispatched by the engine consume it, and they read
    /// `RuleContext::page_context` / `RuleContext::page_marking` (the
    /// page-level fixpoint snapshot the engine attaches to the
    /// synthetic candidate), not `CanonicalAttrs` parsed from candidate
    /// bytes.
    ///
    /// The variant exists so PageFinalization dispatch is
    /// distinguishable from `PageBreak` (which carries the PageContext
    /// reset semantic) without overloading. The two MUST NOT be
    /// co-located in the candidate stream — `PageBreak` is the
    /// scanner-emitted reset boundary; `PageFinalization` is the
    /// engine-synthesized dispatch marker that runs strictly before
    /// the matching `PageBreak`'s reset.
    ///
    /// Issue #461 (PR refactor-006-pr-pagefinalization).
    PageFinalization,
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
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn span_new_accepts_equal_bounds() {
        let s = Span::new(5, 5);
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn span_new_accepts_normal_range() {
        let s = Span::new(2, 7);
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn span_len_calculates_correctly() {
        assert_eq!(Span::new(0, 0).len(), 0);
        assert_eq!(Span::new(0, 10).len(), 10);
        assert_eq!(Span::new(5, 10).len(), 5);
        assert_eq!(Span::new(100, 100).len(), 0);
        assert_eq!(Span::new(100, 250).len(), 150);
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

    #[test]
    fn as_slice_returns_bytes_when_in_bounds() {
        let buf = b"hello";
        let s = Span::new(1, 4);
        assert_eq!(s.as_slice(buf), b"ell");
    }

    #[test]
    #[should_panic]
    fn as_slice_panics_when_end_out_of_bounds() {
        let buf = b"hello";
        let s = Span::new(2, 100);
        let _ = s.as_slice(buf);
    }

    #[test]
    #[should_panic]
    fn as_slice_panics_when_start_out_of_bounds() {
        let buf = b"hello";
        let s = Span::new(100, 101);
        let _ = s.as_slice(buf);
    }
}
