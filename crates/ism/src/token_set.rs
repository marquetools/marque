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
    /// The returned slice is borrowed from the implementor, which allows
    /// implementations to hold the vocabulary on `self` (e.g., in a `Vec`
    /// built at construction time) rather than in a global static. Each
    /// entry is `&'static str` because the fuzzy matcher returns canonical
    /// tokens with `'static` lifetime in `FuzzyCorrection::token`.
    ///
    /// The default implementation returns an empty slice, disabling fuzzy
    /// correction for external `TokenSet` implementors that do not override it.
    fn correction_vocab(&self) -> &[&'static str] {
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

    fn correction_vocab(&self) -> &[&'static str] {
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

    #[test]
    fn correction_vocab_excludes_non_ic_dissem_caveats() {
        // Regression guard for the non-IC dissem deny-list invariant.
        // ODNI's `CVEnumISMDissem.xml` is a UNION enum bundling IC
        // dissem controls (CAPCO source 1) with the ISOO CUI Registry
        // caveat tail (AC, AWP, DL_ONLY, FED_ONLY, FEDCON, NOCON) and
        // the DOD-SAP `WAIVED` entry. CAPCO-2016 line 283 explicitly
        // disclaims caveats from its scope. The `build.rs` of
        // `marque-ism` deny-lists those seven tokens so they never
        // enter the IC `DissemControl` enum or `ALL_CVE_TOKENS`. This
        // test pins that invariant — a future schema-update bump that
        // re-introduces them, or a deny-list typo, fails here loudly
        // rather than silently broadening the CAPCO grammar to accept
        // caveats as IC dissem controls.
        //
        // Tracking issue for the broader caveat / second-banner-line
        // data model: github.com/marquetools/marque#128.
        let vocab = CapcoTokenSet.correction_vocab();
        for forbidden in &[
            "WAIVED", "AC", "AWP", "DL_ONLY", "FED_ONLY", "FEDCON", "NOCON",
        ] {
            assert!(
                vocab.binary_search(forbidden).is_err(),
                "correction_vocab MUST NOT contain {forbidden:?} — \
                 it is a non-IC caveat (CAPCO-2016 line 283 \
                 disclaimer) that should be filtered by build.rs's \
                 NON_IC_DISSEM_DENY_LIST"
            );
        }
    }

    #[test]
    fn correction_vocab_keeps_ic_dissem_controls() {
        // Companion to `correction_vocab_excludes_non_ic_dissem_caveats`:
        // make sure the deny-list didn't take a real IC dissem control
        // with it. Every entry below appears in CAPCO-2016 §A.5 page 38
        // as an IC dissem (or §H.8 for the per-marking detail page);
        // RAWFISA + EXEMPT_FROM_ICD501_DISCOVERY are post-CAPCO-2016
        // additions in the live ICRM XML, kept by the deny-list-rather-
        // than-allowlist approach so future IC additions flow through
        // automatically.
        let vocab = CapcoTokenSet.correction_vocab();
        for expected in &[
            "RS",
            "FOUO",
            "OC",
            "OC-USGOV",
            "IMC",
            "NF",
            "PR",
            "REL",
            "RELIDO",
            "EYES",
            "DSEN",
            "RAWFISA",
            "FISA",
            "DISPLAYONLY",
            "EXEMPT_FROM_ICD501_DISCOVERY",
        ] {
            assert!(
                vocab.binary_search(expected).is_ok(),
                "correction_vocab MUST contain {expected:?} — \
                 IC dissem control per CAPCO-2016 §A.5 / §H.8 or \
                 a post-2016 ICRM addition"
            );
        }
    }
}
