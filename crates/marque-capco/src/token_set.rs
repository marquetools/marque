//! Compile-time Aho-Corasick automaton over CVE token vocabulary.
//!
//! The automaton is built from all known CVE tokens at startup (via LazyLock)
//! and injected into the parser as a `TokenSet` implementation.
//!
//! TODO: Migrate to build.rs-generated token list once CVE parsing is wired.
//! Current implementation uses a hardcoded representative subset for scaffolding.

use aho_corasick::AhoCorasick;
use marque_core::parser::TokenSet;
use std::sync::LazyLock;

/// Known CVE tokens — placeholder subset until build.rs generates the full list.
/// Final list will contain every valid SCI control, SAR prefix, dissem control,
/// handling instruction, and country trigraph from the ODNI CVE XML.
static CVE_TOKENS: &[&str] = &[
    // Classification (full-word banner forms)
    "TOP SECRET", "SECRET", "CONFIDENTIAL", "UNCLASSIFIED",
    // SCI controls
    "SI", "TK", "HCS", "KDK", "RST",
    // Dissemination controls
    "NOFORN", "RELIDO", "FOUO", "ORCON", "PROPIN", "FISA", "DSEN", "LIMDIS",
    // Handling instructions
    "IMCON", "EYES ONLY",
    // Deprecated (retained for detection → migration rules)
    "NF", "LIMDIS",
    // Country trigraphs (representative subset — full list from CVE)
    "USA", "GBR", "AUS", "CAN", "NZL", "DEU", "FRA", "NLD",
];

static AUTOMATON: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::builder()
        .ascii_case_insensitive(false)  // markings are case-sensitive
        .build(CVE_TOKENS)
        .expect("CVE token automaton construction failed")
});

/// Known country trigraphs for fast lookup.
/// TODO: replace with phf map generated from CVE country codes.
static TRIGRAPHS: &[&str] = &[
    "USA", "GBR", "AUS", "CAN", "NZL", "DEU", "FRA", "NLD",
    "BEL", "DNK", "NOR", "SWE", "ESP", "ITA", "PRT", "POL",
    "CZE", "HUN", "ROU", "BGR", "HRV", "SVK", "SVN", "EST",
    "LVA", "LTU", "LUX", "GRC", "TUR", "JPN", "KOR", "ISR",
];

pub struct CapcoTokenSet;

impl TokenSet for CapcoTokenSet {
    fn canonicalize<'a>(&self, token: &'a str) -> Option<&'static str> {
        // Returns the canonical form if token matches a known CVE value.
        CVE_TOKENS.iter().copied().find(|&t| t == token)
    }

    fn is_trigraph(&self, token: &str) -> bool {
        TRIGRAPHS.contains(&token)
    }
}
