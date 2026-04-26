// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO-specific vocabulary tables.
//!
//! # Tetragraph expansion
//!
//! CAPCO's REL TO lists can carry **tetragraphs** (four-letter country
//! group codes) in addition to three-letter trigraphs. The tetragraphs
//! expand to constituent trigraphs before intersection — a portion
//! releasable to `FVEY` is releasable to every Five Eyes nation. The
//! membership table below is hand-curated from the CAPCO Register
//! and related community documentation — *not* derived from ODNI CVE
//! XML. The CVE files list tetragraphs as valid tokens but do not
//! record their membership.
//!
//! Consumers: [`marque_ism::PageContext::expected_rel_to`] today calls
//! a private helper with the same data; this module is the canonical
//! home. Phase C will migrate the `PageContext` helper to import from
//! here (requires a `marque-ism ← marque-capco` dep edge that's not
//! yet established). The tables stay in sync by unit tests in this
//! crate until that migration lands.
//!
//! # Why `&'static [&'static str]` (and not trigraphs)
//!
//! The consumer is code that already has a trigraph-string side-buffer
//! for comparison; bridging through `[u8; 3]` would force the lookup
//! to re-encode on every intersection. A downstream user who wants
//! typed `Trigraph` values can `Trigraph::try_new` from each entry.

/// Five Eyes: AUS, CAN, GBR, NZL, USA.
///
/// CAPCO Register defines FVEY as the Australia / Canada / United
/// Kingdom / New Zealand / United States community.
pub const FVEY: &[&str] = &["AUS", "CAN", "GBR", "NZL", "USA"];

/// Four Eyes minus New Zealand: AUS, CAN, GBR, USA.
///
/// Used for intelligence-sharing arrangements that exclude NZL.
pub const ACGU: &[&str] = &["AUS", "CAN", "GBR", "USA"];

/// NATO tetragraph expansion — **intentionally empty**.
///
/// NATO membership is treaty-driven and changes over time; the full
/// list (30 members as of this writing) lives in the CVE country
/// trigraph table, not here. When a `REL TO NATO` portion composes
/// against a `REL TO USA, GBR` portion, the NATO expansion should
/// resolve to every current NATO member — but that list is outside
/// the CVE scope marque ships, so for now `NATO` stays opaque to the
/// expansion function (portions marked REL TO NATO intersect with
/// REL TO USA, GBR as the empty set unless downstream code has a
/// runtime NATO member list).
///
/// Phase F's NATO scheme adapter will land the membership table
/// alongside the NATO classification lattice; `marque-capco` will
/// then defer to that source.
pub const NATO: &[&str] = &[];

/// Look up a tetragraph's constituent trigraphs. Returns `None` for
/// unknown or non-tetragraph codes (pass-through).
pub fn expand_tetragraph(code: &str) -> Option<&'static [&'static str]> {
    match code {
        "FVEY" => Some(FVEY),
        "ACGU" => Some(ACGU),
        // NATO stays opaque per the module docs until the Phase F
        // adapter lands a canonical membership table. We could return
        // `Some(NATO)` here (empty slice) but that would make any
        // `REL TO NATO ⋂ REL TO USA` compose to empty — returning
        // `None` preserves NATO as an opaque pass-through token
        // instead.
        _ => None,
    }
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
}
