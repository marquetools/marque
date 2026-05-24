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
//! Issue #183 PR-B made this module a thin re-export over
//! [`marque_ism::TETRAGRAPH_MEMBERS`] / [`marque_ism::lookup_tetragraph_members`].
//! This crate and `marque-ism::page_context` carried two private
//! copies of the FVEY/ACGU table that drifted independently; consolidating
//! to one source eliminated the drift.
//!
//! Issue #208 then swapped the backing data source from a hand-curated
//! `BUILTIN_TETRAGRAPH_MEMBERS` slice to the ODNI ISMCAT V2022-NOV
//! Tetragraph Taxonomy (`ism_ismcat::package_root() /
//! Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml`, vendored via the
//! `ism-ismcat` build-dependency) parsed at build time, and added
//! [`marque_ism::is_decomposable`] —
//! the three-state ODNI-authoritative discriminator surfaced through
//! [`is_decomposable_tetragraph`] below for issue #206's S005 rule.
//! Org-specific extensions declared in `crates/ism/country_extensions.toml`
//! continue to layer on top of the taxonomy data.
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
/// Members are sorted ASCII-alphabetical — the ODNI taxonomy XML
/// lists them in publication order (`AUS, CAN, NZL, GBR, USA`), but
/// when FVEY appears in REL TO it's a single token and the
/// constituent order has no semantic weight. Banner roll-up re-sorts
/// the expanded set per CAPCO §H.8 (USA first, then trigraph-alpha,
/// tetragraph-alpha) regardless.
pub const FVEY: &[&str] = &["AUS", "CAN", "GBR", "NZL", "USA"];

/// Four Eyes minus New Zealand: AUS, CAN, GBR, USA.
///
/// Convenience re-export of the row served by
/// [`marque_ism::lookup_tetragraph_members`] for the `"ACGU"` key.
/// Members sorted ASCII-alphabetical (taxonomy XML order happens to
/// agree here).
pub const ACGU: &[&str] = &["AUS", "CAN", "GBR", "USA"];

/// Look up a tetragraph's constituent trigraphs. Returns `None` for
/// codes that don't expand via either of the two sources backing
/// [`marque_ism::lookup_tetragraph_members`]:
///
/// - **Taxonomy entries outside the ISMCAT `decomposable="Yes"` set**:
///   `decomposable="No"` atoms (`EU`, `GCCH`, `KFOR`, …),
///   `decomposable="NA"` deprecated codes (`RSMA`, `ISAF`, `MCFI`, …),
///   and trigraphs (which have no tetragraph expansion).
/// - **Codes absent from both** the ODNI taxonomy and
///   `country_extensions.toml` — fully unknown.
///
/// A code that is **absent from the ODNI taxonomy** can still return
/// `Some(_)` if `country_extensions.toml` declares it with non-empty
/// `members`. Use [`is_decomposable_tetragraph`] for the three-state
/// ODNI-authoritative discriminator that distinguishes "ODNI says it's
/// decomposable" from "an extension claims members" — relevant to
/// issue #206's S005 rule, which fires on extension-claimed expansion
/// precisely because it depends on org-local data ODNI didn't bless.
///
/// Issue #208: thin wrapper around the canonical generated table in
/// `marque-ism`, built from the ISMCAT V2022-NOV Tetragraph Taxonomy.
/// The pre-issue-208 `match` arms on hand-curated `FVEY`/`ACGU` are
/// replaced by a single `binary_search`-backed lookup, so taxonomy
/// codes (NATO, AUSTRALIA_GROUP, …) and extension-defined tetragraphs
/// are picked up automatically.
pub fn expand_tetragraph(code: &str) -> Option<&'static [&'static str]> {
    marque_ism::lookup_tetragraph_members(code)
}

/// Three-state ISMCAT decomposability discriminator.
///
/// Returns:
///
/// - `Some(true)` — ODNI taxonomy `decomposable="Yes"` (24 codes in
///   V2022-NOV, e.g. `FVEY`, `ACGU`, `NATO`, `AUSTRALIA_GROUP`).
/// - `Some(false)` — ODNI taxonomy `decomposable="No"` — atom by
///   authority (19 codes, e.g. `EU`, `GCCH`, `KFOR`).
/// - `None` — ODNI taxonomy `decomposable="NA"` — deprecated;
///   membership suppressed, OCA-deferred, or recursive (18 codes,
///   e.g. `RSMA`, `ISAF`, `MCFI`); OR code absent from taxonomy
///   entirely (org-fork extensions, unknown codes, trigraphs).
///
/// Issue #208 / #206: this is the discriminator S005's silent-loss
/// diagnostic depends on. Routes through [`marque_ism::is_decomposable`]
/// so the dependency arrow stays pointed at `marque-ism`; rule code
/// in this crate does not reach across to query it directly.
pub fn is_decomposable_tetragraph(code: &str) -> Option<bool> {
    marque_ism::is_decomposable(code)
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
    fn expand_tetragraph_nato_returns_members() {
        // Issue #208: NATO is now decomposable=Yes in the ISMCAT
        // V2022-NOV taxonomy with a materialized 30-trigraph member
        // list (the pre-issue-208 "opaque NATO" behavior was a gap,
        // not a deliberate design).
        let members =
            expand_tetragraph("NATO").expect("NATO is decomposable=Yes in ISMCAT V2022-NOV");
        assert!(!members.is_empty(), "NATO members must not be empty");
        for m in members {
            assert_eq!(m.len(), 3, "NATO member not a trigraph: {m}");
            assert!(
                m.chars().all(|c| c.is_ascii_uppercase()),
                "NATO member not uppercase: {m}"
            );
        }
    }

    #[test]
    fn is_decomposable_eu_returns_false() {
        // EU is decomposable="No" in the ISMCAT taxonomy — atom by
        // authority. The diagnostic discriminator distinguishes this
        // (Some(false)) from a deprecated / absent code (None) so
        // S005 can stay silent on EU intersections.
        assert_eq!(is_decomposable_tetragraph("EU"), Some(false));
    }

    #[test]
    fn is_decomposable_fvey_returns_true() {
        assert_eq!(is_decomposable_tetragraph("FVEY"), Some(true));
        assert_eq!(is_decomposable_tetragraph("ACGU"), Some(true));
        assert_eq!(is_decomposable_tetragraph("NATO"), Some(true));
    }

    #[test]
    fn is_decomposable_deprecated_returns_none() {
        // RSMA / ISAF / MCFI are decomposable="NA" (deprecated) in
        // the ISMCAT taxonomy — None means "membership uncertain"
        // for S005's silent-loss diagnostic.
        assert_eq!(is_decomposable_tetragraph("RSMA"), None);
        assert_eq!(is_decomposable_tetragraph("ISAF"), None);
        assert_eq!(is_decomposable_tetragraph("MCFI"), None);
    }

    #[test]
    fn is_decomposable_unknown_returns_none() {
        // Code absent from the taxonomy entirely — same None as
        // deprecated, distinguishable via TETRAGRAPH_PROVENANCE if
        // a consumer needs the distinction.
        assert_eq!(is_decomposable_tetragraph("XYZW"), None);
        // Trigraph (atomic country code) — undefined for tetragraph
        // expansion, expected None.
        assert_eq!(is_decomposable_tetragraph("USA"), None);
    }

    #[test]
    fn expand_tetragraph_unknown() {
        assert!(expand_tetragraph("XYZW").is_none());
    }
}
