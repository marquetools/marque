// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::CapcoScheme;
use crate::scheme::{
    CAT_AEA, CAT_CLASSIFICATION, CAT_DECLASSIFY_ON, CAT_DISSEM, CAT_FGI_MARKER,
    CAT_JOINT_CLASSIFICATION, CAT_NON_US_CLASSIFICATION, CAT_REL_TO, CAT_SAR, CAT_SCI,
};
use marque_scheme::{CategoryId, Vocabulary};

fn vocab() -> CapcoScheme {
    CapcoScheme::new()
}

#[test]
fn fgi_country_token_admits_trigraphs() {
    let v = vocab();
    assert!(v.shape_admits(CAT_FGI_MARKER, b"USA"));
    assert!(v.shape_admits(CAT_FGI_MARKER, b"GBR"));
    assert!(v.shape_admits(CAT_FGI_MARKER, b"JPN"));
}

#[test]
fn fgi_country_token_admits_tetragraphs() {
    let v = vocab();
    assert!(v.shape_admits(CAT_FGI_MARKER, b"NATO"));
    assert!(v.shape_admits(CAT_FGI_MARKER, b"FVEY"));
    assert!(v.shape_admits(CAT_FGI_MARKER, b"ISAF"));
    assert!(v.shape_admits(CAT_FGI_MARKER, b"ACGU"));
}

#[test]
fn fgi_country_token_admits_two_letter_exception() {
    let v = vocab();
    assert!(v.shape_admits(CAT_FGI_MARKER, b"EU"));
}

#[test]
fn fgi_country_token_rejects_lowercase() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"usa"));
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"Usa"));
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"nato"));
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"NaTO"));
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"eu"));
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"Eu"));
}

#[test]
fn fgi_country_token_rejects_wrong_length() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_FGI_MARKER, b""));
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"U"));
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"USAGB"));
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"AUSTRALIA_GROUP"));
}

#[test]
fn fgi_country_token_rejects_digits() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"123"));
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"US1"));
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"NAT0"));
}

#[test]
fn rel_to_admits_shape_eligible_codes() {
    let v = vocab();
    assert!(v.shape_admits(CAT_REL_TO, b"USA"));
    assert!(v.shape_admits(CAT_REL_TO, b"GBR"));
    assert!(v.shape_admits(CAT_REL_TO, b"NATO"));
    assert!(v.shape_admits(CAT_REL_TO, b"FVEY"));
    assert!(v.shape_admits(CAT_REL_TO, b"EU"));
}

#[test]
fn rel_to_admits_registered_long_codes() {
    let v = vocab();
    assert!(v.shape_admits(CAT_REL_TO, b"AUSTRALIA_GROUP"));
}

#[test]
fn rel_to_rejects_arbitrary_long_codes_not_in_registry() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_REL_TO, b"ARBITRARY_LONG_CODE"));
    assert!(!v.shape_admits(CAT_REL_TO, b"USAGB"));
    assert!(!v.shape_admits(CAT_REL_TO, b"FAKE_GROUP"));
}

#[test]
fn rel_to_rejects_invalid_inputs() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_REL_TO, b"usa"));
    assert!(!v.shape_admits(CAT_REL_TO, b"nato"));
    assert!(!v.shape_admits(CAT_REL_TO, b"eu"));
    assert!(!v.shape_admits(CAT_REL_TO, b"australia_group"));
    assert!(!v.shape_admits(CAT_REL_TO, b"123"));
    assert!(!v.shape_admits(CAT_REL_TO, b"U"));
    assert!(!v.shape_admits(CAT_REL_TO, b""));
}

#[test]
fn fgi_marker_rejects_australia_group_class_codes() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_FGI_MARKER, b"AUSTRALIA_GROUP"));
}

#[test]
fn sar_admits_two_or_three_alnum() {
    let v = vocab();
    assert!(v.shape_admits(CAT_SAR, b"BP"));
    assert!(v.shape_admits(CAT_SAR, b"BPB"));
    assert!(v.shape_admits(CAT_SAR, b"XR"));
    assert!(v.shape_admits(CAT_SAR, b"99"));
    assert!(v.shape_admits(CAT_SAR, b"A1"));
}

#[test]
fn sar_rejects_wrong_length() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_SAR, b""));
    assert!(!v.shape_admits(CAT_SAR, b"B"));
    assert!(!v.shape_admits(CAT_SAR, b"BPBP"));
}

#[test]
fn sar_rejects_lowercase_open_vocab_shape_is_validation() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_SAR, b"bp"));
    assert!(!v.shape_admits(CAT_SAR, b"Bp"));
}

#[test]
fn sar_rejects_punctuation() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_SAR, b"B-"));
    assert!(!v.shape_admits(CAT_SAR, b"B P"));
}

#[test]
fn sci_admits_two_or_three_alnum_compartment() {
    let v = vocab();
    assert!(v.shape_admits(CAT_SCI, b"BP"));
    assert!(v.shape_admits(CAT_SCI, b"GBP"));
}

#[test]
fn sci_rejects_single_char_compartment() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_SCI, b"G"));
}

#[test]
fn sci_admits_known_cve_compounds() {
    let v = vocab();
    assert!(v.shape_admits(CAT_SCI, b"BUR-BLG"));
}

#[test]
fn sci_rejects_empty() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_SCI, b""));
}

#[test]
fn classification_admits_portion_form() {
    let v = vocab();
    assert!(v.shape_admits(CAT_CLASSIFICATION, b"S"));
    assert!(v.shape_admits(CAT_CLASSIFICATION, b"TS"));
    assert!(v.shape_admits(CAT_CLASSIFICATION, b"C"));
    assert!(v.shape_admits(CAT_CLASSIFICATION, b"U"));
    assert!(v.shape_admits(CAT_CLASSIFICATION, b"R"));
}

#[test]
fn classification_admits_banner_form() {
    let v = vocab();
    assert!(v.shape_admits(CAT_CLASSIFICATION, b"SECRET"));
    assert!(v.shape_admits(CAT_CLASSIFICATION, b"TOP SECRET"));
    assert!(v.shape_admits(CAT_CLASSIFICATION, b"CONFIDENTIAL"));
    assert!(v.shape_admits(CAT_CLASSIFICATION, b"UNCLASSIFIED"));
    assert!(v.shape_admits(CAT_CLASSIFICATION, b"RESTRICTED"));
}

#[test]
fn classification_rejects_typos() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_CLASSIFICATION, b"SERCET"));
    assert!(!v.shape_admits(CAT_CLASSIFICATION, b"top secret"));
    assert!(!v.shape_admits(CAT_CLASSIFICATION, b""));
}

#[test]
fn classification_rejects_dissem_tokens() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_CLASSIFICATION, b"NF"));
    assert!(!v.shape_admits(CAT_CLASSIFICATION, b"NOFORN"));
}

#[test]
fn dissem_admits_ic_dissem_tokens() {
    let v = vocab();
    assert!(v.shape_admits(CAT_DISSEM, b"NF"));
    assert!(v.shape_admits(CAT_DISSEM, b"OC"));
    assert!(v.shape_admits(CAT_DISSEM, b"FOUO"));
    assert!(v.shape_admits(CAT_DISSEM, b"NOFORN"));
}

#[test]
fn dissem_admits_non_ic_dissem_tokens() {
    let v = vocab();
    assert!(v.shape_admits(CAT_DISSEM, b"ND"));
    assert!(v.shape_admits(CAT_DISSEM, b"XD"));
    assert!(v.shape_admits(CAT_DISSEM, b"DS"));
}

#[test]
fn dissem_rejects_classification_tokens() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_DISSEM, b"S"));
    assert!(!v.shape_admits(CAT_DISSEM, b"TS"));
}

#[test]
fn aea_admits_known_atomic_energy_tokens() {
    let v = vocab();
    assert!(v.shape_admits(CAT_AEA, b"RD"));
    assert!(v.shape_admits(CAT_AEA, b"FRD"));
    assert!(v.shape_admits(CAT_AEA, b"TFNI"));
    assert!(v.shape_admits(CAT_AEA, b"UCNI"));
    assert!(v.shape_admits(CAT_AEA, b"DCNI"));
}

#[test]
fn aea_rejects_dissem_tokens() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_AEA, b"NF"));
    assert!(!v.shape_admits(CAT_AEA, b"S"));
}

#[test]
fn joint_classification_admits_us_levels() {
    let v = vocab();
    assert!(v.shape_admits(CAT_JOINT_CLASSIFICATION, b"S"));
    assert!(v.shape_admits(CAT_JOINT_CLASSIFICATION, b"SECRET"));
    assert!(v.shape_admits(CAT_JOINT_CLASSIFICATION, b"TS"));
}

#[test]
fn non_us_classification_admits_nato_marks() {
    let v = vocab();
    assert!(v.shape_admits(CAT_NON_US_CLASSIFICATION, b"NATO-ATOMAL"));
    assert!(v.shape_admits(CAT_NON_US_CLASSIFICATION, b"NATO-BALK"));
}

#[test]
fn non_us_classification_rejects_us_tokens() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_NON_US_CLASSIFICATION, b"S"));
}

#[test]
fn declassify_on_returns_false_no_panic() {
    let v = vocab();
    assert!(!v.shape_admits(CAT_DECLASSIFY_ON, b"2030-01-01"));
}

#[test]
fn unknown_category_returns_false_no_panic() {
    let v = vocab();
    assert!(!v.shape_admits(CategoryId(9999), b"USA"));
    assert!(!v.shape_admits(CategoryId(9999), b""));
}

#[test]
fn empty_bytes_reject_for_every_category() {
    let v = vocab();
    for cat in [
        CAT_CLASSIFICATION,
        CAT_NON_US_CLASSIFICATION,
        CAT_JOINT_CLASSIFICATION,
        CAT_SCI,
        CAT_SAR,
        CAT_AEA,
        CAT_FGI_MARKER,
        CAT_DISSEM,
        CAT_REL_TO,
        CAT_DECLASSIFY_ON,
    ] {
        assert!(
            !v.shape_admits(cat, b""),
            "empty bytes must reject for category {:?}",
            cat
        );
    }
}

#[test]
fn non_utf8_bytes_reject_no_panic() {
    let v = vocab();
    let invalid = [0x80_u8, 0x80, 0x80];
    assert!(!v.shape_admits(CAT_CLASSIFICATION, &invalid));
    assert!(!v.shape_admits(CAT_DISSEM, &invalid));
    assert!(!v.shape_admits(CAT_SCI, &invalid));
}
