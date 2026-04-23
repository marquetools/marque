// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Single source of truth for banner ↔ portion marking form mappings.
//!
//! The CAPCO Register defines three columns per marking:
//! - **Marking Title** (full descriptive name, e.g., "NOT RELEASABLE TO FOREIGN NATIONALS")
//! - **Banner Line Abbreviation** (e.g., "NOFORN")
//! - **Portion Mark** (e.g., "NF")
//!
//! For most markings, banner and portion forms are identical (e.g., HCS, FISA,
//! RELIDO). This module only tracks entries where the forms *differ*, since
//! those are the ones E001 (banner uses portion abbreviation) and E009 (portion
//! uses banner expansion) need to detect and correct.
//!
//! Classification levels (TOP SECRET ↔ TS, etc.) are handled separately by
//! [`crate::Classification::banner_str`] / [`crate::Classification::portion_str`]
//! because they follow a different structural pattern (banners use full words
//! with no abbreviation, not a shortened form).
//!
//! # Maintenance
//!
//! This table is hand-maintained from the CAPCO Register. The ODNI CVE XML
//! schemas only carry the portion-form codes; banner abbreviations are a CAPCO
//! marking convention not encoded in the XML. When ODNI publishes a new
//! register, update this table and bump the schema version in
//! `crates/ism/Cargo.toml`.

/// A marking where the banner-line abbreviation differs from the portion mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkingForm {
    /// Text used in banner marking lines (e.g., "NOFORN", "ORCON").
    pub banner: &'static str,
    /// Text used in portion markings (e.g., "NF", "OC").
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
    MarkingForm {
        banner: "NOFORN",
        portion: "NF",
    },
    MarkingForm {
        banner: "ORCON-USGOV",
        portion: "OC-USGOV",
    },
    MarkingForm {
        banner: "ORCON",
        portion: "OC",
    },
    MarkingForm {
        banner: "IMCON",
        portion: "IMC",
    },
    MarkingForm {
        banner: "PROPIN",
        portion: "PR",
    },
    MarkingForm {
        banner: "RSEN",
        portion: "RS",
    },
    MarkingForm {
        banner: "DEA SENSITIVE",
        portion: "DSEN",
    },
    // §9 Non-IC Dissemination Control Markings
    MarkingForm {
        banner: "LIMDIS",
        portion: "DS",
    },
    MarkingForm {
        banner: "EXDIS",
        portion: "XD",
    },
    MarkingForm {
        banner: "NODIS",
        portion: "ND",
    },
    MarkingForm {
        banner: "SBU NOFORN",
        portion: "SBU-NF",
    },
    MarkingForm {
        banner: "LES NOFORN",
        portion: "LES-NF",
    },
    // §6 Atomic Energy Act Information Markings (differing forms only)
    MarkingForm {
        banner: "DOD UCNI",
        portion: "DCNI",
    },
    MarkingForm {
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
}
