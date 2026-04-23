// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Single source of truth for banner ↔ portion marking form mappings.
//!
//! The CAPCO Register (CAPCO-2016 §G.1 Table 4, lines 821–841) defines three
//! columns per marking:
//!
//! - **Marking Title** (full descriptive name, e.g., "NOT RELEASABLE TO FOREIGN NATIONALS")
//! - **Banner Line Abbreviation** (e.g., "NOFORN")
//! - **Portion Mark** (e.g., "NF")
//!
//! For most markings, banner and portion forms are identical (e.g., HCS, FISA,
//! RELIDO). This module only tracks entries where the forms *differ*, since
//! those are the ones E001 (banner uses portion abbreviation) and E009 (portion
//! uses banner expansion) need to detect and correct.
//!
//! Per CAPCO-2016 §A.6 line 317, a banner line may spell out the Marking Title
//! OR use the Authorized Abbreviation — both are valid. Detection of the long
//! title in a banner is driven by the [`MarkingForm::title`] field and owned
//! by the S001 `prefer-banner-abbreviation` style rule. `title == banner` when
//! a marking has no distinct abbreviation (e.g., `DEA SENSITIVE`, whose
//! Register row shows `None` under the abbreviation column); S001 must not
//! fire on those.
//!
//! Classification levels (TOP SECRET ↔ TS, etc.) are handled separately by
//! [`crate::Classification::banner_str`] / [`crate::Classification::portion_str`]
//! because they follow a different structural pattern (banners use full words
//! with no abbreviation, not a shortened form).
//!
//! # Maintenance
//!
//! This table is hand-maintained from the CAPCO Register. The ODNI CVE XML
//! schemas only carry the portion-form codes; banner abbreviations and titles
//! are a CAPCO marking convention not encoded in the XML. When ODNI publishes
//! a new register, update this table and bump the schema version in
//! `crates/ism/Cargo.toml`.

/// A marking where the banner-line abbreviation differs from the portion mark.
///
/// Fields correspond to the three columns of CAPCO-2016 §G.1 Table 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkingForm {
    /// Long "Authorized Banner Line Marking Title" form (§G.1 Table 4,
    /// column 1), e.g., "NOT RELEASABLE TO FOREIGN NATIONALS".
    ///
    /// Equals [`Self::banner`] when the Register lists `None` under the
    /// abbreviation column (e.g., `DEA SENSITIVE`), meaning the marking has
    /// no distinct abbreviation. S001 uses `title != banner` to gate its
    /// fix proposal.
    pub title: &'static str,
    /// "Authorized Banner Line Abbreviation" form (§G.1 Table 4, column 2),
    /// e.g., "NOFORN", "ORCON". Equals [`Self::title`] when the Register
    /// lists no distinct abbreviation.
    pub banner: &'static str,
    /// "Authorized Portion Mark" form (§G.1 Table 4, column 3), e.g., "NF",
    /// "OC".
    pub portion: &'static str,
}

/// All markings where banner abbreviation ≠ portion mark.
///
/// Source: CAPCO Register (Implementation Manual for the IC, current edition).
///
/// Sections covered:
/// - §8  Dissemination Control Markings
/// - §9  Non-IC Dissemination Control Markings
/// - §6  Atomic Energy Act Information Markings (subset with differing forms)
///
/// Markings where banner = portion (e.g., FOUO, FISA, RELIDO, HCS, TK) are
/// intentionally omitted — they don't need form correction.
pub static MARKING_FORMS: &[MarkingForm] = &[
    // §8 Dissemination Control Markings
    //
    // Titles below are transcribed from CAPCO-2016 §G.1 Table 4 (lines
    // 821–841). Each row uses columns (Title | Abbreviation | Portion).
    MarkingForm {
        title: "NOT RELEASABLE TO FOREIGN NATIONALS",
        banner: "NOFORN",
        portion: "NF",
    },
    MarkingForm {
        title: "ORIGINATOR CONTROLLED-USGOV",
        banner: "ORCON-USGOV",
        portion: "OC-USGOV",
    },
    MarkingForm {
        title: "ORIGINATOR CONTROLLED",
        banner: "ORCON",
        portion: "OC",
    },
    MarkingForm {
        title: "CONTROLLED IMAGERY",
        banner: "IMCON",
        portion: "IMC",
    },
    MarkingForm {
        title: "CAUTION-PROPRIETARY INFORMATION INVOLVED",
        banner: "PROPIN",
        portion: "PR",
    },
    MarkingForm {
        title: "RISK SENSITIVE",
        banner: "RSEN",
        portion: "RS",
    },
    MarkingForm {
        // §G.1 Table 4 line 831: `| DEA SENSITIVE | None | DSEN |`. No
        // distinct banner abbreviation — `title == banner`. S001 must
        // skip this row (no substitution possible).
        title: "DEA SENSITIVE",
        banner: "DEA SENSITIVE",
        portion: "DSEN",
    },
    // §9 Non-IC Dissemination Control Markings
    MarkingForm {
        title: "LIMITED DISTRIBUTION",
        banner: "LIMDIS",
        portion: "DS",
    },
    MarkingForm {
        title: "EXCLUSIVE DISTRIBUTION",
        banner: "EXDIS",
        portion: "XD",
    },
    MarkingForm {
        title: "NO DISTRIBUTION",
        banner: "NODIS",
        portion: "ND",
    },
    MarkingForm {
        title: "SENSITIVE BUT UNCLASSIFIED NOFORN",
        banner: "SBU NOFORN",
        portion: "SBU-NF",
    },
    MarkingForm {
        title: "LAW ENFORCEMENT SENSITIVE NOFORN",
        banner: "LES NOFORN",
        portion: "LES-NF",
    },
    // §6 Atomic Energy Act Information Markings (differing forms only)
    MarkingForm {
        title: "DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION",
        banner: "DOD UCNI",
        portion: "DCNI",
    },
    MarkingForm {
        title: "DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION",
        banner: "DOE UCNI",
        portion: "UCNI",
    },
    // Note: SIGMA [##] ↔ SG [##] is parametric and handled separately
    // by the parser's pattern-matching path, not this static table.
];

/// Look up the portion-form abbreviation for a banner-form string.
///
/// Used by:
/// - E009 (portion-abbreviation): detects banner forms in portions, suggests abbreviation
/// - Parser (`parse_dissem_full_form`): accepts banner-form input and maps to CVE code
///
/// Returns `None` if the input is not a known banner form (i.e., it's already
/// the portion form, or it's not a recognized marking).
pub fn banner_to_portion(banner: &str) -> Option<&'static str> {
    MARKING_FORMS
        .iter()
        .find(|f| f.banner == banner)
        .map(|f| f.portion)
}

/// Look up the banner-form expansion for a portion-form abbreviation.
///
/// Used by:
/// - E001 (portion-mark-in-banner): detects portion marks used in banner lines, suggests banner abbreviation
///
/// Returns `None` if the input is not a known portion form that has a distinct
/// banner form (i.e., it's already the banner form, or banner = portion).
pub fn portion_to_banner(portion: &str) -> Option<&'static str> {
    MARKING_FORMS
        .iter()
        .find(|f| f.portion == portion)
        .map(|f| f.banner)
}

/// Look up the portion-form abbreviation for a long "Marking Title" string.
///
/// Used by:
/// - Parser (`parse_dissem_full_form`): accepts long-title input like
///   `"NOT RELEASABLE TO FOREIGN NATIONALS"` and maps to the same
///   `DissemControl` the abbreviation would produce.
///
/// Returns `None` if the input is not a known title, or if the marking has
/// no distinct banner abbreviation (`title == banner`). The second case
/// avoids shadowing the dedicated `banner_to_portion` path for inputs like
/// `"DEA SENSITIVE"`.
pub fn title_to_portion(title: &str) -> Option<&'static str> {
    MARKING_FORMS
        .iter()
        .find(|f| f.title == title && f.title != f.banner)
        .map(|f| f.portion)
}

/// Look up the banner-line abbreviation for a long "Marking Title" string.
///
/// Used by:
/// - S001 (prefer-banner-abbreviation): detects long-title forms in banner
///   markings and proposes the abbreviation as a style fix.
///
/// Returns `None` when no substitution is possible — either the input is
/// unknown, or the marking has no distinct abbreviation (`title == banner`,
/// e.g., `DEA SENSITIVE`). The second case is deliberate: S001 must not
/// fire on rows where the Register lists no abbreviation.
pub fn title_to_banner(title: &str) -> Option<&'static str> {
    MARKING_FORMS
        .iter()
        .find(|f| f.title == title && f.title != f.banner)
        .map(|f| f.banner)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn banner_to_portion_known_entries() {
        assert_eq!(banner_to_portion("NOFORN"), Some("NF"));
        assert_eq!(banner_to_portion("ORCON"), Some("OC"));
        assert_eq!(banner_to_portion("IMCON"), Some("IMC"));
        assert_eq!(banner_to_portion("DEA SENSITIVE"), Some("DSEN"));
        assert_eq!(banner_to_portion("PROPIN"), Some("PR"));
        assert_eq!(banner_to_portion("RSEN"), Some("RS"));
        assert_eq!(banner_to_portion("LIMDIS"), Some("DS"));
        assert_eq!(banner_to_portion("EXDIS"), Some("XD"));
        assert_eq!(banner_to_portion("NODIS"), Some("ND"));
        assert_eq!(banner_to_portion("SBU NOFORN"), Some("SBU-NF"));
        assert_eq!(banner_to_portion("LES NOFORN"), Some("LES-NF"));
        assert_eq!(banner_to_portion("DOD UCNI"), Some("DCNI"));
        assert_eq!(banner_to_portion("DOE UCNI"), Some("UCNI"));
    }

    #[test]
    fn portion_to_banner_known_entries() {
        assert_eq!(portion_to_banner("NF"), Some("NOFORN"));
        assert_eq!(portion_to_banner("OC"), Some("ORCON"));
        assert_eq!(portion_to_banner("IMC"), Some("IMCON"));
        assert_eq!(portion_to_banner("DSEN"), Some("DEA SENSITIVE"));
        assert_eq!(portion_to_banner("PR"), Some("PROPIN"));
        assert_eq!(portion_to_banner("RS"), Some("RSEN"));
        assert_eq!(portion_to_banner("DS"), Some("LIMDIS"));
        assert_eq!(portion_to_banner("XD"), Some("EXDIS"));
        // spellchecker:ignore-next-line
        assert_eq!(portion_to_banner("ND"), Some("NODIS"));
        assert_eq!(portion_to_banner("SBU-NF"), Some("SBU NOFORN"));
        assert_eq!(portion_to_banner("LES-NF"), Some("LES NOFORN"));
        assert_eq!(portion_to_banner("DCNI"), Some("DOD UCNI"));
        assert_eq!(portion_to_banner("UCNI"), Some("DOE UCNI"));
    }

    #[test]
    fn banner_to_portion_returns_none_for_unknown() {
        assert_eq!(banner_to_portion("BANANAPHONE"), None);
    }

    #[test]
    fn portion_to_banner_returns_none_for_unknown() {
        assert_eq!(portion_to_banner("BANANAPHONE"), None);
    }

    #[test]
    fn banner_to_portion_returns_none_for_portion_form() {
        // Passing a portion form to banner_to_portion should not match.
        assert_eq!(banner_to_portion("NF"), None);
        assert_eq!(banner_to_portion("OC"), None);
    }

    #[test]
    fn portion_to_banner_returns_none_for_banner_form() {
        // Passing a banner form to portion_to_banner should not match.
        assert_eq!(portion_to_banner("NOFORN"), None);
        assert_eq!(portion_to_banner("ORCON"), None);
    }

    #[test]
    fn no_duplicate_banner_entries() {
        for (i, a) in MARKING_FORMS.iter().enumerate() {
            for (j, b) in MARKING_FORMS.iter().enumerate() {
                if i != j {
                    assert_ne!(a.banner, b.banner, "duplicate banner entry: {:?}", a.banner);
                }
            }
        }
    }

    #[test]
    fn no_duplicate_portion_entries() {
        for (i, a) in MARKING_FORMS.iter().enumerate() {
            for (j, b) in MARKING_FORMS.iter().enumerate() {
                if i != j {
                    assert_ne!(
                        a.portion, b.portion,
                        "duplicate portion entry: {:?}",
                        a.portion
                    );
                }
            }
        }
    }

    #[test]
    fn banner_and_portion_never_equal() {
        for f in MARKING_FORMS {
            assert_ne!(
                f.banner, f.portion,
                "marking form has identical banner and portion: {:?}",
                f.banner
            );
        }
    }

    // T035c-1b: title-column lookups for the S001 style rule and the
    // parser's long-title acceptance path.

    #[test]
    fn title_to_portion_known_entries() {
        assert_eq!(
            title_to_portion("NOT RELEASABLE TO FOREIGN NATIONALS"),
            Some("NF")
        );
        assert_eq!(title_to_portion("ORIGINATOR CONTROLLED"), Some("OC"));
        assert_eq!(title_to_portion("CONTROLLED IMAGERY"), Some("IMC"));
        assert_eq!(
            title_to_portion("CAUTION-PROPRIETARY INFORMATION INVOLVED"),
            Some("PR")
        );
        assert_eq!(title_to_portion("RISK SENSITIVE"), Some("RS"));
        assert_eq!(title_to_portion("LIMITED DISTRIBUTION"), Some("DS"));
        assert_eq!(title_to_portion("EXCLUSIVE DISTRIBUTION"), Some("XD"));
        assert_eq!(title_to_portion("NO DISTRIBUTION"), Some("ND"));
        assert_eq!(
            title_to_portion("SENSITIVE BUT UNCLASSIFIED NOFORN"),
            Some("SBU-NF")
        );
        assert_eq!(
            title_to_portion("LAW ENFORCEMENT SENSITIVE NOFORN"),
            Some("LES-NF")
        );
        assert_eq!(
            title_to_portion("DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION"),
            Some("DCNI")
        );
        assert_eq!(
            title_to_portion("DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION"),
            Some("UCNI")
        );
    }

    #[test]
    fn title_to_banner_known_entries() {
        assert_eq!(
            title_to_banner("NOT RELEASABLE TO FOREIGN NATIONALS"),
            Some("NOFORN")
        );
        assert_eq!(title_to_banner("ORIGINATOR CONTROLLED"), Some("ORCON"));
        assert_eq!(title_to_banner("CONTROLLED IMAGERY"), Some("IMCON"));
        assert_eq!(
            title_to_banner("CAUTION-PROPRIETARY INFORMATION INVOLVED"),
            Some("PROPIN")
        );
        assert_eq!(title_to_banner("RISK SENSITIVE"), Some("RSEN"));
        assert_eq!(title_to_banner("LIMITED DISTRIBUTION"), Some("LIMDIS"));
        assert_eq!(title_to_banner("EXCLUSIVE DISTRIBUTION"), Some("EXDIS"));
        assert_eq!(title_to_banner("NO DISTRIBUTION"), Some("NODIS"));
    }

    #[test]
    fn title_lookups_return_none_for_dea_sensitive() {
        // CAPCO-2016 §G.1 Table 4 line 831: DEA SENSITIVE has no
        // distinct banner abbreviation (`| DEA SENSITIVE | None | DSEN |`).
        // The `title == banner` guard in the lookups must skip this row
        // so S001 does not propose a no-op substitution and the parser
        // does not double-resolve the banner-form path.
        assert_eq!(title_to_portion("DEA SENSITIVE"), None);
        assert_eq!(title_to_banner("DEA SENSITIVE"), None);
    }

    #[test]
    fn title_lookups_return_none_for_unknown() {
        assert_eq!(title_to_portion("BANANAPHONE"), None);
        assert_eq!(title_to_banner("BANANAPHONE"), None);
        // A banner abbreviation string (not a title) must not match
        // title lookups.
        assert_eq!(title_to_portion("NOFORN"), None);
        assert_eq!(title_to_banner("NOFORN"), None);
    }

    #[test]
    fn no_duplicate_title_entries() {
        for (i, a) in MARKING_FORMS.iter().enumerate() {
            for (j, b) in MARKING_FORMS.iter().enumerate() {
                if i != j {
                    assert_ne!(a.title, b.title, "duplicate title entry: {:?}", a.title);
                }
            }
        }
    }

    #[test]
    fn dea_sensitive_is_the_only_title_equal_banner() {
        // Guards against future ODNI register changes that might
        // introduce new rows without a distinct abbreviation. If one
        // lands, update S001's pin-down tests and this guard.
        let same_form: Vec<&'static str> = MARKING_FORMS
            .iter()
            .filter(|f| f.title == f.banner)
            .map(|f| f.title)
            .collect();
        assert_eq!(
            same_form,
            vec!["DEA SENSITIVE"],
            "only DEA SENSITIVE should have `title == banner` today \
             (CAPCO-2016 §G.1 Table 4 line 831). If this fails, a new \
             row without a distinct abbreviation has been added — \
             update S001 tests accordingly."
        );
    }
}
