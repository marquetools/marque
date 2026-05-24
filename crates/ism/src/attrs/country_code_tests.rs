// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use super::CountryCode;

#[test]
fn try_new_accepts_two_byte_eu() {
    let eu = CountryCode::try_new(b"EU").unwrap();
    assert_eq!(eu.as_str(), "EU");
    assert_eq!(eu.len(), 2);
}

#[test]
fn try_new_accepts_three_byte_trigraph() {
    let usa = CountryCode::try_new(b"USA").unwrap();
    assert_eq!(usa, CountryCode::USA);
    assert_eq!(usa.as_str(), "USA");
}

#[test]
fn try_new_accepts_four_byte_tetragraph() {
    let fvey = CountryCode::try_new(b"FVEY").unwrap();
    assert_eq!(fvey.as_str(), "FVEY");
    assert_eq!(fvey.len(), 4);
}

#[test]
fn try_new_accepts_australia_group_with_underscore() {
    let ag = CountryCode::try_new(b"AUSTRALIA_GROUP").unwrap();
    assert_eq!(ag.as_str(), "AUSTRALIA_GROUP");
    assert_eq!(ag.len(), 15);
}

#[test]
fn try_new_accepts_digits_in_ax2_ax3() {
    assert_eq!(CountryCode::try_new(b"AX2").unwrap().as_str(), "AX2");
    assert_eq!(CountryCode::try_new(b"AX3").unwrap().as_str(), "AX3");
}

#[test]
fn try_new_rejects_too_short() {
    assert!(CountryCode::try_new(b"").is_none());
    assert!(CountryCode::try_new(b"X").is_none());
}

#[test]
fn try_new_rejects_too_long() {
    // 17 bytes — one over capacity.
    assert!(CountryCode::try_new(b"ABCDEFGHIJKLMNOPQ").is_none());
}

#[test]
fn try_new_rejects_lowercase() {
    assert!(CountryCode::try_new(b"usa").is_none());
    assert!(CountryCode::try_new(b"Fvey").is_none());
}

#[test]
fn try_new_rejects_non_ascii() {
    // 'É' is two UTF-8 bytes (0xC3 0x89); first byte fails the
    // is_valid_byte check.
    let bytes = "ÉU".as_bytes();
    assert!(CountryCode::try_new(bytes).is_none());
}

#[test]
fn ord_matches_str_lex_for_mixed_lengths() {
    let eu = CountryCode::try_new(b"EU").unwrap();
    let aus = CountryCode::try_new(b"AUS").unwrap();
    let usa = CountryCode::USA;
    let usab = CountryCode::try_new(b"USAB").unwrap();
    let mut all = [eu, aus, usa, usab];
    all.sort();
    assert_eq!(all[0].as_str(), "AUS");
    assert_eq!(all[1].as_str(), "EU");
    assert_eq!(all[2].as_str(), "USA");
    assert_eq!(all[3].as_str(), "USAB");
}

#[test]
fn copy_semantics_preserved() {
    let original = CountryCode::USA;
    let copy = original;
    // Both still usable — `Copy` not `Move`.
    assert_eq!(original, copy);
    assert_eq!(original.as_str(), copy.as_str());
}

#[test]
fn display_renders_active_bytes_only() {
    // Display impl writes the active byte slice; the zero
    // padding past `len` must never reach the formatter.
    let usa = CountryCode::USA;
    let fvey = CountryCode::try_new(b"FVEY").unwrap();
    let ag = CountryCode::try_new(b"AUSTRALIA_GROUP").unwrap();
    assert_eq!(format!("{usa}"), "USA");
    assert_eq!(format!("{fvey}"), "FVEY");
    assert_eq!(format!("{ag}"), "AUSTRALIA_GROUP");
}

#[test]
fn as_bytes_excludes_zero_padding() {
    let usa = CountryCode::USA;
    assert_eq!(usa.as_bytes(), b"USA");
    let fvey = CountryCode::try_new(b"FVEY").unwrap();
    assert_eq!(fvey.as_bytes(), b"FVEY");
}

#[test]
fn is_empty_invariant_always_false() {
    // `try_new` rejects `len < 2`, so a constructed `CountryCode`
    // is never empty. `is_empty` exists only to satisfy clippy's
    // `len_without_is_empty`; pin the invariant so a future
    // refactor that loosens `try_new` is forced to revisit it.
    assert!(!CountryCode::USA.is_empty());
    assert!(!CountryCode::try_new(b"EU").unwrap().is_empty());
    assert!(!CountryCode::try_new(b"AUSTRALIA_GROUP").unwrap().is_empty());
}

#[test]
fn usa_constant_matches_try_new() {
    // `pub const USA` constructs via `try_new` in const context.
    // Pin the equivalence so a future change to either path
    // (e.g., adding a normalization step to `try_new` but not
    // the const constructor) breaks loudly.
    let runtime = CountryCode::try_new(b"USA").unwrap();
    assert_eq!(CountryCode::USA, runtime);
    assert_eq!(CountryCode::USA.as_bytes(), runtime.as_bytes());
    assert_eq!(CountryCode::USA.len(), runtime.len());
}

#[test]
fn nato_constant_matches_try_new() {
    // PR #505: `CountryCode::NATO` is forward-investment for the
    // NATO classification closure cone deferred to #508. Same
    // const-construction invariant as USA / AUS / CAN / GBR / NZL.
    let runtime = CountryCode::try_new(b"NATO").expect("NATO is a valid tetragraph");
    assert_eq!(CountryCode::NATO, runtime);
    assert_eq!(CountryCode::NATO.as_str(), "NATO");
    assert_eq!(CountryCode::NATO.as_bytes(), b"NATO");
    assert_eq!(CountryCode::NATO.len(), 4);
}

// ----------------------------------------------------------------
// admits_fgi_trigraph — Annex B trigraph shape predicate
// ----------------------------------------------------------------
//
// Admission closure: this predicate IS the documented FGI /
// REL-TO trigraph admission gate. Both the `Vocabulary<CapcoScheme>`
// adapter (`crates/capco/src/vocabulary.rs`) and the strict parser
// (`crates/core/src/parser.rs`) call into it. These tests pin the
// invariants both call sites depend on; a regression that loosens
// the predicate (e.g., admitting digits, lowercase, or tetragraphs)
// would silently broaden the accept set on both surfaces.

#[test]
fn admits_fgi_trigraph_accepts_three_uppercase_letters() {
    assert!(CountryCode::admits_fgi_trigraph(b"USA"));
    assert!(CountryCode::admits_fgi_trigraph(b"GBR"));
    assert!(CountryCode::admits_fgi_trigraph(b"DEU"));
    assert!(CountryCode::admits_fgi_trigraph(b"AUS"));
    assert!(CountryCode::admits_fgi_trigraph(b"JPN"));
}

#[test]
fn admits_fgi_trigraph_rejects_lowercase() {
    assert!(!CountryCode::admits_fgi_trigraph(b"usa"));
    assert!(!CountryCode::admits_fgi_trigraph(b"Usa"));
    assert!(!CountryCode::admits_fgi_trigraph(b"USa"));
}

#[test]
fn admits_fgi_trigraph_rejects_wrong_length() {
    assert!(!CountryCode::admits_fgi_trigraph(b""));
    assert!(!CountryCode::admits_fgi_trigraph(b"U"));
    assert!(!CountryCode::admits_fgi_trigraph(b"US"));
    assert!(!CountryCode::admits_fgi_trigraph(b"USAA"));
    // Tetragraphs (4-letter org codes) live in the separate
    // tetragraph table, not this trigraph predicate.
    assert!(!CountryCode::admits_fgi_trigraph(b"NATO"));
    assert!(!CountryCode::admits_fgi_trigraph(b"FVEY"));
    assert!(!CountryCode::admits_fgi_trigraph(b"ISAF"));
}

#[test]
fn admits_fgi_trigraph_rejects_digits() {
    // `CountryCode::try_new` accepts ASCII digits (for AX2 / AX3),
    // but the FGI trigraph predicate does not — Annex B GENC codes
    // are alpha-only.
    assert!(!CountryCode::admits_fgi_trigraph(b"AX2"));
    assert!(!CountryCode::admits_fgi_trigraph(b"123"));
    assert!(!CountryCode::admits_fgi_trigraph(b"US1"));
}

#[test]
fn admits_fgi_trigraph_rejects_underscore() {
    // `CountryCode::try_new` accepts underscore (for
    // AUSTRALIA_GROUP), but the FGI trigraph predicate rejects
    // every non-alpha byte.
    assert!(!CountryCode::admits_fgi_trigraph(b"US_"));
    assert!(!CountryCode::admits_fgi_trigraph(b"_US"));
}

#[test]
fn admits_fgi_trigraph_rejects_non_ascii() {
    // 'É' is two UTF-8 bytes (0xC3 0x89); first byte is not ASCII
    // uppercase. A 3-byte UTF-8 sequence must still fail because
    // every byte must individually be in `b'A'..=b'Z'`.
    let two_byte_e_acute = "ÉU".as_bytes(); // 3 bytes total
    assert_eq!(two_byte_e_acute.len(), 3);
    assert!(!CountryCode::admits_fgi_trigraph(two_byte_e_acute));
}

#[test]
fn admits_fgi_trigraph_implies_try_new() {
    // Property: if `admits_fgi_trigraph` accepts, then `try_new`
    // accepts (the trigraph predicate is strictly stronger). This
    // is what lets the parser gate by `admits_fgi_trigraph` and
    // construct via `try_new` without a redundant validation.
    for code in [b"USA", b"GBR", b"DEU", b"FRA", b"JPN"] {
        assert!(CountryCode::admits_fgi_trigraph(code));
        assert!(CountryCode::try_new(code).is_some());
    }
}

// -------------------------------------------------------------------
// admits_country_token — full FGI/REL TO list-token shape predicate
//
// Per CAPCO-2016 §H.7 p122, the FGI list grammar admits BOTH 3-letter
// Annex B trigraphs and 4-letter Annex A tetragraphs (canonical
// example: `SECRET//FGI GBR JPN NATO`). The §H.8 REL TO surface
// accepts the same shape. These tests pin the trigraph-or-tetragraph
// contract so a regression to "trigraph-only" (PR #311 review
// finding, GH #280 fix-cousin) is caught at unit-test scope.
// -------------------------------------------------------------------

#[test]
fn admits_country_token_accepts_trigraphs() {
    assert!(CountryCode::admits_country_token(b"USA"));
    assert!(CountryCode::admits_country_token(b"GBR"));
    assert!(CountryCode::admits_country_token(b"DEU"));
    assert!(CountryCode::admits_country_token(b"AUS"));
    assert!(CountryCode::admits_country_token(b"JPN"));
}

#[test]
fn admits_country_token_accepts_tetragraphs() {
    // Per CAPCO-2016 §H.7 p122 ("Multiple FGI trigraph country
    // codes or tetragraph codes must be separated by a single
    // space ... example may appear as: SECRET//FGI GBR JPN
    // NATO//REL TO USA, GBR, JPN, NATO."), tetragraphs admit at
    // the same shape gate as trigraphs.
    assert!(CountryCode::admits_country_token(b"NATO"));
    assert!(CountryCode::admits_country_token(b"FVEY"));
    assert!(CountryCode::admits_country_token(b"ISAF"));
    assert!(CountryCode::admits_country_token(b"ACGU"));
    assert!(CountryCode::admits_country_token(b"TEYE"));
}

#[test]
fn admits_country_token_accepts_two_letter_exception() {
    // ODNI ISMCAT `CVEnumISMCATRelTo` ships `EU` as a registered
    // 2-letter exception code; pre-PR-2 REL TO admission accepted
    // it via the union TRIGRAPHS table. The shape gate must not
    // narrow that surface — registry membership of any 2-letter
    // code other than `EU` is a rule-layer concern, not a
    // shape concern.
    assert!(CountryCode::admits_country_token(b"EU"));
}

#[test]
fn admits_country_token_rejects_lowercase() {
    assert!(!CountryCode::admits_country_token(b"usa"));
    assert!(!CountryCode::admits_country_token(b"nato"));
    assert!(!CountryCode::admits_country_token(b"Nato"));
    assert!(!CountryCode::admits_country_token(b"NaTO"));
    assert!(!CountryCode::admits_country_token(b"eu"));
    assert!(!CountryCode::admits_country_token(b"Eu"));
}

#[test]
fn admits_country_token_rejects_digits() {
    assert!(!CountryCode::admits_country_token(b"US1"));
    assert!(!CountryCode::admits_country_token(b"NAT0")); // 0 not O
    assert!(!CountryCode::admits_country_token(b"123"));
    assert!(!CountryCode::admits_country_token(b"1234"));
    assert!(!CountryCode::admits_country_token(b"E1"));
}

#[test]
fn admits_country_token_rejects_wrong_length() {
    assert!(!CountryCode::admits_country_token(b""));
    assert!(!CountryCode::admits_country_token(b"U"));
    // 5-letter+ codes (`AUSTRALIA_GROUP` is 15) explicitly
    // out of scope per the predicate's "exception is granted"
    // carve-out — these are admitted via `try_new`, not at
    // this gate.
    assert!(!CountryCode::admits_country_token(b"USAGB"));
    assert!(!CountryCode::admits_country_token(b"AUSTRALIA_GROUP"));
}

#[test]
fn admits_country_token_rejects_underscore_and_punctuation() {
    // `try_new` admits underscore; this gate does not.
    assert!(!CountryCode::admits_country_token(b"US_"));
    assert!(!CountryCode::admits_country_token(b"NAT_"));
    assert!(!CountryCode::admits_country_token(b"USA "));
    assert!(!CountryCode::admits_country_token(b"USA-"));
    assert!(!CountryCode::admits_country_token(b"E_"));
}

#[test]
fn admits_country_token_supersets_admits_fgi_trigraph() {
    // Property: every `admits_fgi_trigraph` accept is also an
    // `admits_country_token` accept. Pins the strictly-broader
    // contract so a future predicate edit can't silently invert
    // the relationship.
    for code in [&b"USA"[..], b"GBR", b"DEU", b"FRA", b"JPN", b"AUS", b"CAN"] {
        assert!(CountryCode::admits_fgi_trigraph(code));
        assert!(CountryCode::admits_country_token(code));
    }
}
