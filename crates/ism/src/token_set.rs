// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-time Aho-Corasick automaton over CVE token vocabulary.
//!
//! The automaton is built from all known CVE tokens at startup (via LazyLock)
//! and injected into the parser as a `TokenSet` implementation.

use aho_corasick::AhoCorasick;
use std::sync::LazyLock;

use crate::generated::values;

/// Minimal interface the parser needs from the token set.
/// Implemented by `CapcoTokenSet`; injected at engine init.
pub trait TokenSet: Send + Sync {
    /// Returns the canonical token string if `token` is a known CVE value.
    fn canonicalize(&self, token: &str) -> Option<&'static str>;

    /// Returns true if `token` is a known country trigraph.
    fn is_trigraph(&self, token: &str) -> bool;

    /// Returns the vocabulary slice used for fuzzy correction lookups.
    ///
    /// This is the token vocabulary against which unknown tokens are compared
    /// by the `marque_core::fuzzy` module. Must be sorted and deduplicated
    /// (binary search is used for the "is already valid" check).
    ///
    /// The default implementation returns an empty slice, disabling fuzzy
    /// correction for external `TokenSet` implementors that do not override it.
    fn correction_vocab(&self) -> &'static [&'static str] {
        &[]
    }
}

/// Aho-Corasick automaton over all CVE tokens — built once from generated data.
static AUTOMATON: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::builder()
        .ascii_case_insensitive(false) // markings are case-sensitive
        .build(values::ALL_CVE_TOKENS)
        .expect("CVE token automaton construction failed")
});

pub struct CapcoTokenSet;

impl TokenSet for CapcoTokenSet {
    fn canonicalize(&self, token: &str) -> Option<&'static str> {
        // `ALL_CVE_TOKENS` is emitted sorted and deduplicated by build.rs,
        // so an O(log n) binary search is correct and faster than the
        // previous O(n) linear scan.
        values::ALL_CVE_TOKENS
            .binary_search(&token)
            .ok()
            .map(|i| values::ALL_CVE_TOKENS[i])
    }

    fn is_trigraph(&self, token: &str) -> bool {
        // TRIGRAPHS is emitted sorted and deduplicated by build.rs, so
        // binary_search is O(log n) over ~340 entries instead of the old
        // O(n) `.contains()` linear scan. Hot path for every REL TO parse.
        values::TRIGRAPHS.binary_search(&token).is_ok()
    }

    fn correction_vocab(&self) -> &'static [&'static str] {
        values::ALL_CVE_TOKENS
    }
}

impl CapcoTokenSet {
    /// Returns a reference to the Aho-Corasick automaton built from all CVE tokens.
    /// Reserved for Phase 2 multi-pattern matching when per-token spans are wired.
    #[allow(dead_code)]
    pub(crate) fn automaton() -> &'static AhoCorasick {
        &AUTOMATON
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn all_cve_tokens_are_sorted_and_unique() {
        let tokens = values::ALL_CVE_TOKENS;
        for window in tokens.windows(2) {
            assert!(
                window[0] < window[1],
                "ALL_CVE_TOKENS is not strictly sorted: {:?} >= {:?}",
                window[0],
                window[1],
            );
        }
    }

    #[test]
    fn trigraphs_are_sorted_and_unique() {
        // `is_trigraph` relies on binary_search, so the slice must be
        // strictly-sorted. If a future ODNI XSD update shuffles the order,
        // build.rs collects into a BTreeSet and this test catches any
        // regression of that contract.
        let trigraphs = values::TRIGRAPHS;
        for window in trigraphs.windows(2) {
            assert!(
                window[0] < window[1],
                "TRIGRAPHS is not strictly sorted: {:?} >= {:?}",
                window[0],
                window[1],
            );
        }
    }

    #[test]
    fn canonicalize_returns_known_token() {
        let set = CapcoTokenSet;
        // SECRET is in the banner-words we always emit.
        assert_eq!(set.canonicalize("SECRET"), Some("SECRET"));
    }

    #[test]
    fn canonicalize_returns_none_for_unknown() {
        let set = CapcoTokenSet;
        assert_eq!(set.canonicalize("BANANAPHONE"), None);
    }

    #[test]
    fn usa_is_a_known_trigraph() {
        let set = CapcoTokenSet;
        assert!(set.is_trigraph("USA"));
    }

    #[test]
    fn unknown_string_is_not_a_trigraph() {
        let set = CapcoTokenSet;
        assert!(!set.is_trigraph("XYZ_NOT_A_COUNTRY"));
    }

    #[test]
    fn correction_vocab_returns_sorted_nonempty_slice() {
        let vocab = CapcoTokenSet.correction_vocab();
        assert!(!vocab.is_empty(), "correction vocab must not be empty");
        for window in vocab.windows(2) {
            assert!(
                window[0] < window[1],
                "correction_vocab must be strictly sorted: {:?} >= {:?}",
                window[0],
                window[1],
            );
        }
    }

    #[test]
    fn correction_vocab_contains_core_classification_tokens() {
        let vocab = CapcoTokenSet.correction_vocab();
        for expected in &["SECRET", "CONFIDENTIAL", "UNCLASSIFIED"] {
            assert!(
                vocab.binary_search(expected).is_ok(),
                "correction_vocab must contain {expected:?}"
            );
        }
    }
}
