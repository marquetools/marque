// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::scheme::{
    CAT_AEA, CAT_CLASSIFICATION, CAT_DISSEM, CAT_FGI_MARKER, CAT_JOINT_CLASSIFICATION,
    CAT_NON_US_CLASSIFICATION, CAT_REL_TO, CAT_SAR, CAT_SCI,
};
use marque_ism::generated::vocabulary::{TokenMetadataEntry, lookup_token_metadata};
use marque_ism::marking_forms::banner_to_portion;
use marque_scheme::CategoryId;

enum CveFileSet {
    UsClassification,
    NonUsControls,
    Dissem,
    AtomicEnergy,
}

impl CveFileSet {
    #[inline]
    fn contains(&self, entry: &TokenMetadataEntry) -> bool {
        let name = entry.cve_file.const_name;
        match self {
            Self::UsClassification => name == "CVE_CLASSIFICATION_ALL",
            Self::NonUsControls => name == "CVE_NON_US_CONTROLS",
            Self::Dissem => name == "CVE_DISSEM" || name == "CVE_NON_IC",
            Self::AtomicEnergy => name == "CVE_ATOMIC_ENERGY_MARKINGS",
        }
    }
}

fn admits_closed_cve(bytes: &[u8], set: &CveFileSet) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let Ok(s) = std::str::from_utf8(bytes) else {
        return false;
    };

    if let Some(entry) = lookup_token_metadata(s) {
        if set.contains(entry) {
            return true;
        }
    }
    if matches!(set, CveFileSet::UsClassification) && classification_banner_to_portion(s).is_some()
    {
        return true;
    }
    if let Some(portion) = banner_to_portion(s) {
        if let Some(entry) = lookup_token_metadata(portion) {
            return set.contains(entry);
        }
    }
    false
}

#[inline]
fn classification_banner_to_portion(s: &str) -> Option<&'static str> {
    match s {
        "TOP SECRET" => Some("TS"),
        "SECRET" => Some("S"),
        "CONFIDENTIAL" => Some("C"),
        "UNCLASSIFIED" => Some("U"),
        "RESTRICTED" => Some("R"),
        _ => None,
    }
}

#[inline]
fn shape_country_token(bytes: &[u8]) -> bool {
    marque_ism::CountryCode::admits_country_token(bytes)
}

#[inline]
fn shape_or_registered_rel_to_token(bytes: &[u8]) -> bool {
    if shape_country_token(bytes) {
        return true;
    }
    let Ok(s) = std::str::from_utf8(bytes) else {
        return false;
    };
    marque_ism::TRIGRAPHS.binary_search(&s).is_ok()
}

#[inline]
fn shape_sar_program_id(bytes: &[u8]) -> bool {
    marque_ism::SarProgram::admits_program_id_abbrev(bytes)
}

#[inline]
fn shape_sci_compartment(bytes: &[u8]) -> bool {
    matches!(bytes.len(), 2 | 3) && bytes.iter().all(u8::is_ascii_alphanumeric)
}

fn admits_sci(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        if let Some(entry) = lookup_token_metadata(s) {
            if entry.cve_file.const_name == "CVE_SCI_CONTROLS" {
                return true;
            }
        }
    }
    shape_sci_compartment(bytes)
}

#[inline]
pub(super) fn shape_admits(category: CategoryId, bytes: &[u8]) -> bool {
    match category {
        CAT_CLASSIFICATION => admits_closed_cve(bytes, &CveFileSet::UsClassification),
        CAT_NON_US_CLASSIFICATION => admits_closed_cve(bytes, &CveFileSet::NonUsControls),
        CAT_JOINT_CLASSIFICATION => admits_closed_cve(bytes, &CveFileSet::UsClassification),
        CAT_SCI => admits_sci(bytes),
        CAT_SAR => shape_sar_program_id(bytes),
        CAT_AEA => admits_closed_cve(bytes, &CveFileSet::AtomicEnergy),
        CAT_FGI_MARKER => shape_country_token(bytes),
        CAT_REL_TO => shape_or_registered_rel_to_token(bytes),
        CAT_DISSEM => admits_closed_cve(bytes, &CveFileSet::Dissem),
        _ => false,
    }
}
