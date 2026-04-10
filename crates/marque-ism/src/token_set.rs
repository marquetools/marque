//! Compile-time Aho-Corasick automaton over CVE token vocabulary.
//!
//! The automaton is built from all known CVE tokens at startup (via LazyLock)
//! and injected into the parser as a `TokenSet` implementation.
//!
//! TODO: Migrate to build.rs-generated token list once CVE parsing is wired.
//! Current implementation uses a hardcoded representative subset for scaffolding.

use aho_corasick::AhoCorasick;
use std::sync::LazyLock;

/// Minimal interface the parser needs from the token set.
/// Implemented by `CapcoTokenSet`; injected at engine init.
pub trait TokenSet: Send + Sync {
    /// Returns the canonical token string if `token` is a known CVE value.
    fn canonicalize(&self, token: &str) -> Option<&'static str>;

    /// Returns true if `token` is a known country trigraph.
    fn is_trigraph(&self, token: &str) -> bool;
}

/// Known CVE tokens — placeholder subset until build.rs generates the full list.
/// Final list will contain every valid SCI control, SAR prefix, dissem control,
/// handling instruction, and country trigraph from the ODNI CVE XML.
static CVE_TOKENS: &[&str] = &[
    // Classification (full-word banner forms)
    "TOP SECRET",
    "SECRET",
    "CONFIDENTIAL",
    "UNCLASSIFIED",
    // SCI controls
    "SI",
    "TK",
    "HCS",
    "KDK",
    "RST",
    // Dissemination controls
    "NOFORN",
    "RELIDO",
    "FOUO",
    "ORCON",
    "PROPIN",
    "FISA",
    "DSEN",
    "LIMDIS",
    // Handling instructions
    "IMCON",
    "EYES ONLY",
    // Deprecated (retained for detection → migration rules)
    "NF",
    // Country trigraphs (representative subset — full list from CVE)
    "USA",
    "GBR",
    "AUS",
    "CAN",
    "NZL",
    "DEU",
    "FRA",
    "NLD",
];

#[allow(dead_code)] // Will be used once parser wires through marque-ism
static AUTOMATON: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::builder()
        .ascii_case_insensitive(false) // markings are case-sensitive
        .build(CVE_TOKENS)
        .expect("CVE token automaton construction failed")
});

/// Known country trigraphs for fast lookup.
/// TODO: replace with phf map generated from CVE country codes.
static TRIGRAPHS: &[&str] = &[
    "USA", "GBR", "AUS", "CAN", "NZL", "DEU", "FRA", "NLD", "BEL", "DNK", "NOR", "SWE", "ESP",
    "ITA", "PRT", "POL", "CZE", "HUN", "ROU", "BGR", "HRV", "SVK", "SVN", "EST", "LVA", "LTU",
    "LUX", "GRC", "TUR", "JPN", "KOR", "ISR",
];

pub struct CapcoTokenSet;

impl TokenSet for CapcoTokenSet {
    fn canonicalize(&self, token: &str) -> Option<&'static str> {
        // Returns the canonical form if token matches a known CVE value.
        CVE_TOKENS.iter().copied().find(|&t| t == token)
    }

    fn is_trigraph(&self, token: &str) -> bool {
        TRIGRAPHS.contains(&token)
    }
}
