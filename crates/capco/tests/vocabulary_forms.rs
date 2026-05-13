// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3d (FR-053) — `Vocabulary<CapcoScheme>::forms()` round-trip
//! byte-identity test.
//!
//! Asserts that the new aggregated `FormSet` accessor and the
//! per-form default-method projections (`portion_form`,
//! `banner_form`, `banner_abbreviation`) agree, and that the
//! projections preserve the pre-3d output for every active sentinel
//! TokenId.
//!
//! ## What this pins
//!
//! - `scheme.portion_form(t) == scheme.forms(t).portion` for every
//!   active sentinel.
//! - `scheme.banner_form(t) ==
//!   scheme.forms(t).banner_abbreviation.unwrap_or(scheme.forms(t).banner_title)`
//!   — the FR-053 projection equation specified by T058d.
//! - `scheme.banner_abbreviation(t) == scheme.forms(t).banner_abbreviation`.
//!
//! The expected projection outputs are captured inline as a hand-
//! rolled `&'static [(TokenId, &'static str, &'static str)]` table
//! seeded from the pre-3d behavior. Any future refactor that
//! changes the projection for an active sentinel must update the
//! expected table here in lock-step — the regression is loud.
//!
//! ## What this does NOT pin
//!
//! `forms(t)` reads at every active sentinel return `&'static`
//! data — that invariant is exercised by the
//! `vocabulary_zero_alloc` test (gated on the `count-allocs`
//! feature). This file pins the projection equation; the zero-
//! allocation gate pins the storage model.

use marque_capco::CapcoScheme;
use marque_capco::scheme::{
    TOK_CNWDI, TOK_EXDIS, TOK_FRD, TOK_HCS, TOK_NODIS, TOK_NOFORN, TOK_RD, TOK_RESTRICTED,
    TOK_TFNI, TOK_UCNI,
};
use marque_scheme::{TokenId, Vocabulary};

/// Every active sentinel TokenId with its expected
/// `(portion_form, banner_form)` projection. `banner_abbreviation`
/// is derived per-row inside the test loop from these two values
/// plus the `forms(t).banner_title` — the test asserts the trait's
/// default-method projection matches the equation in T058d.
///
/// Seeded from the pre-3d behavior (commit
/// `d4664160 perf: SmallVec at parser scratch + renderer sort buffers`
/// state of `crates/capco/src/vocabulary.rs::derive_*`) plus the
/// FR-053 corrected `banner_abbreviation` semantic for RD / FRD /
/// TFNI (see `build_form_set` doc in
/// `crates/capco/src/vocabulary.rs`).
const EXPECTED_FORMS: &[(TokenId, &str, &str)] = &[
    // (token, expected portion, expected banner)
    (TOK_NOFORN, "NF", "NOFORN"),
    (TOK_RD, "RD", "RD"),
    (TOK_FRD, "FRD", "FRD"),
    (TOK_TFNI, "TFNI", "TFNI"),
    // CNWDI's canonical is "RD-CNWDI" — no MARKING_FORMS row,
    // canonical-collapse fallback applies.
    (TOK_CNWDI, "RD-CNWDI", "RD-CNWDI"),
    // UCNI canonical is "UCNI"; MARKING_FORMS row has
    // title="DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION",
    // banner="DOE UCNI", portion="UCNI".
    (TOK_UCNI, "UCNI", "DOE UCNI"),
    // HCS canonical is "HCS"; no MARKING_FORMS row, canonical-collapse.
    (TOK_HCS, "HCS", "HCS"),
    // RESTRICTED canonical is "R"; no MARKING_FORMS row, no
    // classification_form_set arm (per byte-identity preservation
    // — see `classification_form_set` doc). Canonical-collapse.
    (TOK_RESTRICTED, "R", "R"),
    // NODIS canonical is "ND"; MARKING_FORMS row has
    // title="NO DISTRIBUTION", banner="NODIS", portion="ND".
    (TOK_NODIS, "ND", "NODIS"),
    // EXDIS canonical is "XD"; MARKING_FORMS row has
    // title="EXCLUSIVE DISTRIBUTION", banner="EXDIS", portion="XD".
    (TOK_EXDIS, "XD", "EXDIS"),
];

#[test]
fn forms_round_trips_for_every_active_sentinel() {
    let scheme = CapcoScheme::new();

    for (token, expected_portion, expected_banner) in EXPECTED_FORMS {
        let form_set = scheme.forms(token);

        // Default-method projection #1: portion_form
        assert_eq!(
            scheme.portion_form(token),
            form_set.portion,
            "portion_form / forms.portion disagree for {token:?}",
        );
        assert_eq!(
            scheme.portion_form(token),
            *expected_portion,
            "portion_form regression for {token:?}",
        );

        // Default-method projection #2: banner_form per T058d's
        // exact equation
        let projected_banner = form_set
            .banner_abbreviation
            .unwrap_or(form_set.banner_title);
        assert_eq!(
            scheme.banner_form(token),
            projected_banner,
            "banner_form does not match \
             banner_abbreviation.unwrap_or(banner_title) for {token:?}",
        );
        assert_eq!(
            scheme.banner_form(token),
            *expected_banner,
            "banner_form regression for {token:?}",
        );

        // Default-method projection #3: banner_abbreviation
        assert_eq!(
            scheme.banner_abbreviation(token),
            form_set.banner_abbreviation,
            "banner_abbreviation / forms.banner_abbreviation disagree \
             for {token:?}",
        );
    }
}

#[test]
fn recognized_aliases_empty_at_pr_3d() {
    // PR 3d (FR-053) ships the `recognized_aliases` field plumbed
    // but unpopulated. The
    // `crates/ism/tests/description_title_divergence.rs` test pins
    // the count of ODNI Description vs CAPCO title divergences;
    // until a divergence appears, every form-set's aliases slice is
    // empty. This regression guard catches an unintentional
    // population at scheme-impl level.
    let scheme = CapcoScheme::new();
    for (token, _, _) in EXPECTED_FORMS {
        let form_set = scheme.forms(token);
        assert!(
            form_set.recognized_aliases.is_empty(),
            "recognized_aliases unexpectedly non-empty for {token:?}: {:?}",
            form_set.recognized_aliases,
        );
    }
}
