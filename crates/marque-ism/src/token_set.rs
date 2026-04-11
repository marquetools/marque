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
        // TRIGRAPHS are in CVE-defined order (USA first, then alphabetical).
        // Use linear scan; the list is ~340 entries — fast enough for parsing.
        values::TRIGRAPHS.contains(&token)
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
}
