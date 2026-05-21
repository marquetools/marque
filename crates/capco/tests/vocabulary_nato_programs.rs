// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #660 — `Vocabulary<CapcoScheme>` resolution for the NATO
//! program sentinels (`TOK_ATOMAL` / `TOK_BALK` / `TOK_BOHEMIA`).
//!
//! Pre-#660 these sentinels were registered in
//! `crates/capco/src/scheme/mod.rs` and consumed by E066
//! (`LegacyNatoCompoundRemarkRule` — re-marks legacy NATO compound
//! text per CAPCO-2016 §G.2 p40 + §H.7 p122 + §H.7 p127) but were
//! missing from `SENTINEL_TO_CANONICAL` in
//! `crates/capco/src/vocabulary.rs`. Any caller that invoked
//! `canonical_for` / `entry_for` / `forms` / `metadata` on the three
//! sentinels would have panicked in `canonical_for` ("TokenId has no
//! canonical CVE value"), blocking corpus fixtures whose lint→fix→audit
//! path touches the canonical-CVE-lookup for these tokens.
//!
//! Issue #660 fixed the gap by:
//! 1. Adding the three sentinels to `SENTINEL_TO_CANONICAL` with the
//!    ODNI CVE canonicals `NATO-ATOMAL` / `NATO-BALK` / `NATO-BOHEMIA`
//!    (published in `CVE_NON_US_CONTROLS`).
//! 2. Adding `nato_program_form_set` (mirrors `classification_form_set`)
//!    that projects the prefixed CVE canonical onto the bare §G.1
//!    Table 4 p36 display form (`"ATOMAL"` / `"BALK"` / `"BOHEMIA"`)
//!    for the `forms()` accessor.
//!
//! ## What this pins
//!
//! - **Non-panic resolution**: `metadata` / `forms` / `banner_form` /
//!   `portion_form` / `banner_abbreviation` / `authority` /
//!   `owner_producer` / `point_of_contact` / `deprecation` on each of
//!   the three sentinels does NOT panic.
//! - **Bare §G.1 Table 4 p36 display form**: `forms(t).portion` and
//!   `forms(t).banner_title` return the bare display form
//!   (`"ATOMAL"` / `"BALK"` / `"BOHEMIA"`), NOT the `NATO-`-prefixed
//!   CVE canonical.
//! - **CVE canonical reachable via metadata**: `metadata(t).canonical`
//!   exposes the prefixed CVE canonical (`"NATO-ATOMAL"`, etc.) —
//!   the two diverge by design.
//! - **No banner abbreviation**: per §G.1 Table 4 p36 col 2
//!   (empty for NATO programs), `banner_abbreviation` is `None`.
//!
//! ## Citation discipline (Constitution VIII)
//!
//! Each §-citation below has been verified directly against
//! `crates/capco/docs/CAPCO-2016.md` at this PR's authorship — not
//! propagated from `CAPCO-CONTEXT.md`, the issue body, or any other
//! in-tree comment.

use marque_capco::CapcoScheme;
use marque_capco::scheme::{TOK_ATOMAL, TOK_BALK, TOK_BOHEMIA};
use marque_scheme::{TokenId, Vocabulary};

/// The three NATO program sentinels and their expected projections.
///
/// Tuple shape: `(token, bare_display_form, cve_canonical)`.
/// - `bare_display_form` = §G.1 Table 4 p36 portion / banner-title
///   column (no banner abbreviation per col 2 emptiness).
/// - `cve_canonical` = ODNI `CVE_NON_US_CONTROLS` entry value
///   (verified against the build-time-generated
///   `target/debug/build/marque-ism-*/out/vocabulary.rs` —
///   `TokenMetadataEntry { value: "NATO-ATOMAL"|"NATO-BALK"|
///   "NATO-BOHEMIA", cve_file: &CVE_NON_US_CONTROLS }`).
fn nato_program_sentinels() -> &'static [(TokenId, &'static str, &'static str)] {
    &[
        (TOK_ATOMAL, "ATOMAL", "NATO-ATOMAL"),
        (TOK_BALK, "BALK", "NATO-BALK"),
        (TOK_BOHEMIA, "BOHEMIA", "NATO-BOHEMIA"),
    ]
}

/// Regression test for the original panic — the three NATO program
/// sentinels must resolve through `canonical_for` / `entry_for` /
/// `forms` / `metadata` / `authority` / `owner_producer` /
/// `point_of_contact` / `deprecation` without panicking.
///
/// Pre-#660 each of these calls would panic in `canonical_for`
/// because the sentinels were absent from `SENTINEL_TO_CANONICAL`.
#[test]
fn nato_program_tokens_resolve_without_panic() {
    let scheme = CapcoScheme::new();
    for (tok, _, _) in nato_program_sentinels() {
        // Each accessor reaches a different chokepoint in
        // `crates/capco/src/vocabulary.rs` — exercise them all so a
        // future regression that loses one helper's panic-fix is
        // caught.
        let _ = scheme.authority(tok);
        let _ = scheme.owner_producer(tok);
        let _ = scheme.point_of_contact(tok);
        let _ = scheme.deprecation(tok);
        let _ = scheme.metadata(tok);
        let _ = scheme.forms(tok);
        let _ = scheme.portion_form(tok);
        let _ = scheme.banner_form(tok);
        let _ = scheme.banner_abbreviation(tok);
    }
}

/// Pin the form-set output: portion / banner_title use the bare §G.1
/// Table 4 p36 display form, banner_abbreviation is `None`, and
/// `metadata().canonical` exposes the CVE canonical (the prefixed
/// form), preserving the divergence as a designed property.
///
/// A future "harmonization" PR that routes the bare display form
/// through `metadata().canonical` (or vice versa) would falsify the
/// audit-record contract: `metadata().canonical` is the audit-side
/// CVE-canonical identity (consumed by `qualified_token_label` →
/// `"aea.NATO-ATOMAL"` / `"sci.NATO-BALK"` / `"sci.NATO-BOHEMIA"`
/// per `crates/capco/src/scheme/predicates/token_routing.rs`), while
/// the bare form is the user-visible §G.1 Table 4 p36 marking text.
#[test]
fn nato_program_tokens_use_bare_display_forms() {
    let scheme = CapcoScheme::new();
    for (tok, bare, cve_canonical) in nato_program_sentinels() {
        let forms = scheme.forms(tok);
        assert_eq!(
            forms.portion, *bare,
            "{tok:?}: portion form must be bare §G.1 Table 4 p36 form, \
             not the `NATO-`-prefixed CVE canonical",
        );
        assert_eq!(
            forms.banner_title, *bare,
            "{tok:?}: banner_title must be bare §G.1 Table 4 p36 form",
        );
        assert_eq!(
            forms.banner_abbreviation, None,
            "{tok:?}: §G.1 Table 4 p36 col 2 is empty for NATO programs \
             (registration with no banner abbreviation)",
        );
        assert!(
            forms.recognized_aliases.is_empty(),
            "{tok:?}: no recognized_aliases — MARKING_FORMS row for the \
             bare form carries description_title: None per §G.1 Table 4 \
             p36",
        );

        // The default-method projections agree with the FormSet.
        assert_eq!(scheme.portion_form(tok), *bare);
        assert_eq!(scheme.banner_form(tok), *bare);
        assert_eq!(scheme.banner_abbreviation(tok), None);

        // metadata.canonical exposes the CVE canonical (NATO-prefixed),
        // not the display form — the divergence is the audit-record
        // contract that `qualified_token_label` then composes with the
        // category name (`aea` / `sci`).
        let meta = scheme.metadata(tok);
        assert_eq!(
            meta.canonical, *cve_canonical,
            "{tok:?}: metadata.canonical is the CVE canonical from \
             CVE_NON_US_CONTROLS, not the bare §G.1 Table 4 p36 form",
        );
        // But the per-form fields agree with the bare display form.
        assert_eq!(meta.portion_form, *bare);
        assert_eq!(meta.banner_form, *bare);
        assert_eq!(meta.banner_abbreviation, None);
    }
}

/// Authority chain pin — every NATO program sentinel routes through
/// the ODNI `CVE_NON_US_CONTROLS` file (the same file E066's
/// canonicalization references). This catches a future regression that
/// might route the sentinel through a different CVE file (e.g., a
/// hypothetical `CVE_NATO_PROGRAMS`).
#[test]
fn nato_program_tokens_route_through_odni_non_us_controls() {
    let scheme = CapcoScheme::new();
    for (tok, _, _) in nato_program_sentinels() {
        let authority = scheme.authority(tok);
        assert!(
            authority.urn.starts_with("urn:us:gov:ic:cvenum:"),
            "{tok:?}: authority.urn = {:?} does not start with the ODNI prefix \
             (expected via CVE_NON_US_CONTROLS)",
            authority.urn,
        );
    }
}
