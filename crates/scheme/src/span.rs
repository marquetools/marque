// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Byte-offset spans into source buffers — zero-copy position tracking.
//!
//! Lives in `marque-scheme` (the foundation leaf) so that
//! [`crate::constraint::ConstraintViolation`] and similar scheme-layer
//! types can carry source positions without taking a dependency on
//! `marque-ism` (which would violate Constitution VII —
//! `marque-scheme` is the only true graph leaf).
//!
//! `marque-ism` re-exports `Span` from this module, so existing
//! consumers (`crates/ism/src/span.rs`, `crates/core/`, etc.) continue
//! to use `marque_ism::Span` unchanged. The single definition lives
//! here; the re-export preserves back-compat across the workspace.

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
