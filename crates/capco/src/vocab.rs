// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO-specific vocabulary tables.
//!
//! # Tetragraph expansion
//!
//! CAPCO's REL TO lists can carry **tetragraphs** (four-letter
//! country-group codes) in addition to three-letter trigraphs. The
//! tetragraphs expand to constituent trigraphs before intersection —
//! a portion releasable to `FVEY` is releasable to every Five Eyes
//! nation.
//!
//! Issue #183 PR-B: this module is now a thin re-export over
//! [`marque_ism::TETRAGRAPH_MEMBERS`] /
//! [`marque_ism::lookup_tetragraph_members`] — the canonical
//! membership table emitted by `marque-ism::build.rs` from
//! hand-curated CAPCO Register data plus any org-specific extensions
//! declared in `crates/ism/country_extensions.toml`. Pre-PR-B this
//! crate and `marque-ism::page_context` carried two private copies of
//! the FVEY/ACGU table that drifted independently; consolidating to
//! one source eliminates the drift.
//!
//! # Why `&'static [&'static str]` (and not typed `CountryCode`)
//!
//! The consumer is code that already has a code-string side-buffer
//! for comparison; bridging through the typed form would force the
//! lookup to re-encode on every intersection. A downstream user who
//! wants typed [`marque_ism::CountryCode`] values can
//! `CountryCode::try_new` from each entry.

/// Five Eyes: AUS, CAN, GBR, NZL, USA.
///
/// CAPCO Register defines FVEY as the Australia / Canada / United
/// Kingdom / New Zealand / United States community. Convenience
/// re-export of the row served by
/// [`marque_ism::lookup_tetragraph_members`] for the `"FVEY"` key.
pub const FVEY: &[&str] = &["AUS", "CAN", "GBR", "NZL", "USA"];

/// Four Eyes minus New Zealand: AUS, CAN, GBR, USA.
///
/// Convenience re-export of the row served by
/// [`marque_ism::lookup_tetragraph_members`] for the `"ACGU"` key.
pub const ACGU: &[&str] = &["AUS", "CAN", "GBR", "USA"];

/// NATO tetragraph expansion — **intentionally empty / opaque**.
///
/// NATO membership is treaty-driven and changes over time; the
/// canonical member list is **not** emitted by `marque-ism`'s
/// tetragraph table — `lookup_tetragraph_members("NATO")` returns
/// `None`, and `REL TO NATO` therefore composes as an opaque atom
/// in intersection.
///
/// A future NATO scheme adapter (tracked alongside the Phase F
/// NATO classification lattice) will land the membership table;
/// once it does, this module's documentation should switch to
/// reference that source instead of describing the gap.
pub const NATO: &[&str] = &[];

/// Look up a tetragraph's constituent trigraphs. Returns `None` for
/// unknown / opaque codes (NATO and operation-specific tetragraphs
/// like RSMA / ISAF / KFOR) and for trigraphs (which have no
/// expansion).
///
/// Issue #183 PR-B: thin wrapper around the canonical generated
/// table in `marque-ism`. The pre-PR-B `match` arms on
/// `FVEY`/`ACGU`/(NATO-opaque-via-`_`) are replaced by a single
/// `binary_search`-backed lookup, so extension-defined tetragraphs
/// are picked up automatically.
pub fn expand_tetragraph(code: &str) -> Option<&'static [&'static str]> {
    marque_ism::lookup_tetragraph_members(code)
}

/// Human-readable names for a small set of CAPCO country trigraphs.
///
/// **Scope is intentionally narrow**: covers only the trigraphs S004
/// (`rel-to-trigraph-suggest`, issue #235 / #186 PR-3) and likely
/// follow-on suggest-channel rules need to render in diagnostic
/// messages — partner trigraphs commonly seen in REL TO blocks plus
/// every trigraph that appears in the four #186 ambiguous fixtures
/// (USB → USA?, AUT → AUS?, ASU → AUS?, SA → USA?). This is **not** a
/// vendored ISO 3166 dataset; expanding the table to the full
/// CVE_ISMCAT trigraph set is a future build-time codegen step
/// (the names live in `crates/ism/schemas/ISM-v2022-DEC/CVE_ISMCAT/`
/// XSD annotations) — when that lands, this hand table should be
/// retired in favor of the generated form.
///
/// Sources:
/// - `crates/ism/schemas/ISM-v2022-DEC/CVE_ISMCAT/CVEGenerated/CVEnumISMCATRelTo.xsd`
///   for trigraph→country-name pairs (the authoritative schema annotates
///   each entry with its `xs:annotation/xs:documentation`).
/// - ISO 3166-1 alpha-3 spec for the codes themselves.
///
/// The table MUST stay sorted by trigraph (binary-search-backed
/// `country_name`); the unit test `country_names_sorted` enforces
/// this. Adding a row requires keeping the alphabetical order.
pub(crate) const COUNTRY_NAMES: &[(&str, &str)] = &[
    ("ASM", "American Samoa"),
    ("AUS", "Australia"),
    ("AUT", "Austria"),
    ("CAN", "Canada"),
    ("DEU", "Germany"),
    ("FRA", "France"),
    ("GBR", "United Kingdom"),
    ("JPN", "Japan"),
    ("KOR", "Republic of Korea"),
    ("NZL", "New Zealand"),
    ("USA", "United States"),
    ("UZB", "Uzbekistan"),
];

/// Look up a country trigraph's English name from [`COUNTRY_NAMES`].
///
/// Returns `None` for trigraphs not present in the small hand-curated
/// table — callers (S004) fall back to a generic message form
/// ("`{trigraph}` may be a typo; did you mean `{candidate}`?") in
/// that case.
pub(crate) fn country_name(trigraph: &str) -> Option<&'static str> {
    COUNTRY_NAMES
        .binary_search_by_key(&trigraph, |&(t, _)| t)
        .ok()
        .map(|i| COUNTRY_NAMES[i].1)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn fvey_has_five_members_alphabetical() {
        assert_eq!(FVEY, &["AUS", "CAN", "GBR", "NZL", "USA"]);
    }

    #[test]
    fn acgu_has_four_members_minus_nzl() {
        assert_eq!(ACGU, &["AUS", "CAN", "GBR", "USA"]);
        assert!(!ACGU.contains(&"NZL"));
    }

    #[test]
    fn fvey_is_acgu_plus_nzl() {
        let mut fvey_without_nzl: Vec<&str> =
            FVEY.iter().copied().filter(|&c| c != "NZL").collect();
        fvey_without_nzl.sort();
        assert_eq!(&fvey_without_nzl[..], ACGU);
    }

    #[test]
    fn fvey_members_all_uppercase_trigraphs() {
        for &m in FVEY {
            assert_eq!(m.len(), 3, "FVEY member not 3 chars: {m}");
            assert!(
                m.chars().all(|c| c.is_ascii_uppercase()),
                "FVEY member not uppercase: {m}"
            );
        }
    }

    #[test]
    fn expand_tetragraph_known() {
        assert_eq!(expand_tetragraph("FVEY"), Some(FVEY));
        assert_eq!(expand_tetragraph("ACGU"), Some(ACGU));
    }

    #[test]
    fn expand_tetragraph_trigraph_returns_none() {
        assert!(expand_tetragraph("USA").is_none());
    }

    #[test]
    fn expand_tetragraph_nato_is_opaque_pass_through() {
        // NATO stays opaque until Phase F lands the membership table.
        assert!(expand_tetragraph("NATO").is_none());
    }

    #[test]
    fn expand_tetragraph_unknown() {
        assert!(expand_tetragraph("XYZW").is_none());
    }

    #[test]
    fn country_names_sorted() {
        // Binary search in `country_name` requires the table to be
        // sorted by trigraph. Adding a new row out of order would
        // produce silent lookup failures rather than a build error,
        // so this test acts as the regression guard.
        for pair in COUNTRY_NAMES.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "COUNTRY_NAMES not sorted: {} before {}",
                pair[0].0,
                pair[1].0,
            );
        }
    }

    #[test]
    fn country_name_lookup_works_for_known_codes() {
        // The four #186 ambiguous fixtures' relevant trigraphs must
        // resolve to readable English names so S004 messages stay
        // informative.
        assert_eq!(country_name("USA"), Some("United States"));
        assert_eq!(country_name("AUS"), Some("Australia"));
        assert_eq!(country_name("AUT"), Some("Austria"));
        assert_eq!(country_name("ASM"), Some("American Samoa"));
        assert_eq!(country_name("UZB"), Some("Uzbekistan"));
    }

    #[test]
    fn country_name_returns_none_for_unknown() {
        assert!(country_name("XYZ").is_none());
    }
}
