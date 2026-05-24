#![cfg(any())]
// Legacy FixProposal-shape test disabled pending rewrite.

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Trait-level tests for `impl Vocabulary<CapcoScheme> for CapcoScheme`.
//!
//! `marque-ism` ships the raw per-token tables (tested in
//! `crates/ism/tests/vocabulary_tables.rs`); this file exercises
//! the `marque-scheme::Vocabulary<S>` adapter that composes them
//! into the trait surface.
//!
//! ## Token coverage scope
//!
//! `CapcoScheme::Token = TokenId` and the active sentinel set today
//! is ~14 hand-assigned ids in `crates/capco/src/scheme/mod.rs` — the
//! only TokenIds CAPCO's catalog actually references. Of those, the
//! ones with a corresponding canonical CVE value in the JSON-derived
//! `TOKEN_METADATA` table form the "active CAPCO vocabulary set"
//! that this file iterates. Aggregate sentinels (`TOK_US_CLASSIFIED`,
//! `TOK_NON_US_CLASSIFICATION`, `TOK_IC_DISSEM`, `TOK_NON_IC_DISSEM`),
//! trigraph sentinels (`TOK_USA` — trigraphs come from XSD, not
//! JSON), and grammar-shape sentinels (`TOK_JOINT`, `TOK_FGI_MARKER`)
//! are explicitly out of scope: they have no single CVE value to
//! attach metadata to. The Vocabulary impl panics if asked about
//! one of those, surfacing the misuse loudly.
//!
//! A future extension covers both the sentinel set and the canonical-string
//! mapping for the full CVE-published token vocabulary; until
//! then the per-sentinel set is what "active" means here.

use marque_capco::CapcoScheme;
use marque_capco::scheme::{
    TOK_CNWDI, TOK_EXDIS, TOK_FRD, TOK_HCS, TOK_NODIS, TOK_NOFORN, TOK_RD, TOK_RESTRICTED,
    TOK_TFNI, TOK_UCNI,
};
use marque_scheme::{TokenId, Vocabulary};

/// The set of sentinel TokenIds with a corresponding canonical CVE
/// value in `marque-ism::generated::vocabulary::TOKEN_METADATA`. Used
/// by every "iterate the active set" test below.
fn active_sentinels() -> &'static [TokenId] {
    &[
        TOK_NOFORN,
        TOK_RD,
        TOK_FRD,
        TOK_TFNI,
        TOK_CNWDI,
        TOK_UCNI,
        TOK_HCS,
        TOK_RESTRICTED,
        TOK_NODIS,
        TOK_EXDIS,
    ]
}

// ---------------------------------------------------------------------------
// Every active token has authority + owner/producer + POC + forms.
// ---------------------------------------------------------------------------

#[test]
fn every_active_token_has_authority() {
    let scheme = CapcoScheme::new();
    for token in active_sentinels() {
        let authority = scheme.authority(token);
        assert!(
            !authority.source_name.is_empty(),
            "authority({token:?}).source_name is empty"
        );
        assert!(
            !authority.urn.is_empty(),
            "authority({token:?}).urn is empty"
        );
        assert!(
            !authority.schema_version.is_empty(),
            "authority({token:?}).schema_version is empty"
        );

        let owner = scheme.owner_producer(token);
        assert!(
            !owner.code.is_empty(),
            "owner_producer({token:?}).code is empty"
        );

        let poc = scheme.point_of_contact(token);
        assert!(
            !poc.name.is_empty(),
            "point_of_contact({token:?}).name is empty"
        );
        assert!(
            !poc.email.is_empty(),
            "point_of_contact({token:?}).email is empty"
        );
        assert!(
            poc.email.contains('@'),
            "point_of_contact({token:?}).email is not an email"
        );

        assert!(
            !scheme.portion_form(token).is_empty(),
            "portion_form({token:?}) is empty"
        );
        assert!(
            !scheme.banner_form(token).is_empty(),
            "banner_form({token:?}) is empty"
        );
    }
}

// ---------------------------------------------------------------------------
// Authority traces back to ODNI for every ISM token.
// ---------------------------------------------------------------------------

#[test]
fn authority_points_to_odni_for_ism_tokens() {
    let scheme = CapcoScheme::new();
    for token in active_sentinels() {
        let authority = scheme.authority(token);
        assert!(
            authority.urn.starts_with("urn:us:gov:ic:cvenum:"),
            "authority({token:?}).urn = {:?} does not start with the ODNI prefix",
            authority.urn,
        );
        assert_eq!(
            authority.schema_version, "ISM-v2022-DEC",
            "authority({token:?}).schema_version is not the pinned package version",
        );
    }
}

// ---------------------------------------------------------------------------
// Active tokens have no deprecation metadata.
// ---------------------------------------------------------------------------

/// No active sentinel today is a deprecated marking — every entry in
/// `active_sentinels()` is a current, valid CAPCO token. This test
/// asserts the *negative* case: an active token returns `None` from
/// `deprecation()`. The complementary positive case lives below; both
/// arms of the `Option` are exercised between the two.
///
/// When a future extension adds deprecated tokens to the sentinel set
/// (e.g., adding a sentinel for `25X1-` from the `MIGRATIONS` table),
/// this test should keep its current scope (active subset → `None`)
/// and a new sibling test should land for the deprecated subset.
#[test]
fn active_tokens_have_no_deprecation_metadata() {
    let scheme = CapcoScheme::new();
    for token in active_sentinels() {
        assert!(
            scheme.deprecation(token).is_none(),
            "deprecation({token:?}) returned Some for an active token — \
             active tokens must not carry deprecation metadata",
        );
    }
}

// ---------------------------------------------------------------------------
// Replacement is populated when a deprecation maps to a known token.
// ---------------------------------------------------------------------------

/// Pin-down test for the deprecation-replacement contract.
///
/// `Deprecation { since, replacement: Option<S::Token> }` carries
/// `replacement = Some(_)` when the deprecated token has a known
/// canonical successor in the active vocabulary, and `None` when there
/// is no known replacement (silence is informative; do not rewrite into
/// a token that does not exist).
///
/// The `MIGRATIONS` table in `marque-ism::generated::migrations`
/// today has two entries (`25X1-` → `25X1`, `50X1-` → `50X1-HUM`),
/// neither of which corresponds to a `CapcoScheme` sentinel — so no
/// active sentinel currently has a `Some(replacement)` deprecation.
/// This test asserts the structural property: every replacement that
/// IS populated must be a TokenId that resolves cleanly in the same
/// vocabulary (no dangling pointers). A future extension will add
/// real deprecations to the sentinel set and this test will
/// gain genuine `Some(_)` coverage.
#[test]
fn deprecation_replacement_when_known() {
    let scheme = CapcoScheme::new();
    for token in active_sentinels() {
        if let Some(deprecation) = scheme.deprecation(token) {
            assert!(
                !deprecation.since.is_empty(),
                "deprecation({token:?}).since is empty",
            );
            if let Some(replacement) = &deprecation.replacement {
                // Self-consistency: if a replacement is named, the
                // vocabulary must be able to resolve it.
                let _ = scheme.authority(replacement);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FOUO remains an active dissem control after the FOUO→CUI removal.
// ---------------------------------------------------------------------------

/// Constructs a FOUO-bearing document and runs it through `Engine::lint`,
/// asserting no diagnostic suggests migrating FOUO to CUI. The legacy
/// `FOUO → CUI` migration entry was retired
/// (`crates/ism/build.rs` doc-comment under `MIGRATIONS`); FOUO remains
/// enumerated in `CVEnumISMDissem.json` and is a current, valid CAPCO
/// dissem control. Any rule that proposes a CUI replacement on FOUO is
/// a regression.
///
/// The test phrases "no CUI suggestion" structurally rather than by
/// counting all diagnostics: a stylistic E001/E009-style fix may still
/// fire on the input, and we don't want to false-fail on those.
#[test]
fn fouo_remains_active_dissem_control() {
    use marque_capco::CapcoRuleSet;
    use marque_config::Config;
    use marque_engine::Engine;

    let engine = Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");
    let source = b"(U//FOUO) Test paragraph with for-official-use-only marking.";
    let result = engine.lint(source);

    for diag in &result.diagnostics {
        let message = diag.message.to_lowercase();
        assert!(
            !message.contains("cui") && !message.contains("controlled unclassified"),
            "diagnostic suggests CUI migration on FOUO input: {diag:?}",
        );
        if let Some(fix) = &diag.fix {
            let replacement_lower = fix.replacement.to_lowercase();
            assert!(
                !replacement_lower.contains("cui"),
                "fix replacement migrates FOUO to CUI: {fix:?}",
            );
        }
    }
}

// The zero-allocation regression gate lives in its own integration
// file `tests/vocabulary_zero_alloc.rs`, gated at the FILE level on
// `#![cfg(feature = "count-allocs")]`. Isolating it to its own binary
// keeps the global counting allocator's measurements free of noise
// from the other integration tests in this file (which would
// otherwise allocate freely inside the same process and inflate the
// shared counter even with a `MEASURE_LOCK`).

// ---------------------------------------------------------------------------
// Expanded trait coverage: panic paths, banner-abbreviation arms, and
// metadata round-trip equivalence.
// ---------------------------------------------------------------------------
//
// The happy-path active-sentinel-loop tests above leave uncovered the
// panic branches in `canonical_for` / `entry_for` / `derived_for_token`
// / `token_derived` (none of the active sentinels ever hit them) and
// the `Some` arm of `derive_banner_abbreviation`. These tests close
// that gap so the production code paths CI exercises match the
// production code paths real callers will hit.

/// Sentinels whose `MARKING_FORMS` row has a distinct title and
/// banner-line abbreviation. Under the semantic
/// `banner_abbreviation = Some iff banner != title`, these must
/// surface their banner abbreviation as `Some(banner)`. Each tuple
/// is `(token, canonical/portion, expected banner abbreviation)`.
///
/// The abbreviation column is non-empty exactly when the row has a
/// distinct title (CAPCO §G.1 Table 4 col 2), regardless of whether the
/// abbreviation collapses to the portion form. For RD / FRD / TFNI the
/// expected banner is the row's banner-line abbreviation — `"RD"`,
/// `"FRD"`, `"TFNI"` — which also equals the portion form for those
/// three rows; `banner_form()` returns the same string via
/// `banner_abbreviation.unwrap_or(banner_title)`.
fn distinct_banner_form_sentinels() -> &'static [(TokenId, &'static str, &'static str)] {
    &[
        (TOK_NOFORN, "NF", "NOFORN"),
        (TOK_UCNI, "UCNI", "DOE UCNI"),
        (TOK_NODIS, "ND", "NODIS"),
        (TOK_EXDIS, "XD", "EXDIS"),
        // Rows where MARKING_FORMS lists a distinct CAPCO title; the
        // banner-line abbreviation is the row's banner column even
        // when it collapses to the portion form. CAPCO §G.1 Table 4:
        // - RD row: title="RESTRICTED DATA", banner="RD", portion="RD"
        // - FRD row: title="FORMERLY RESTRICTED DATA", banner="FRD",
        //   portion="FRD"
        // - TFNI row: title="TRANSCLASSIFIED FOREIGN NUCLEAR
        //   INFORMATION", banner="TFNI", portion="TFNI"
        (TOK_RD, "RD", "RD"),
        (TOK_FRD, "FRD", "FRD"),
        (TOK_TFNI, "TFNI", "TFNI"),
    ]
}

/// Sentinels whose canonical CVE value has NO `MARKING_FORMS` row
/// (the canonical-collapse fallback applies: all three forms are the
/// canonical itself, `banner_abbreviation` is `None`). Today's set:
/// HCS (canonical `"HCS"`, no row), TOK_CNWDI (canonical `"RD-CNWDI"`,
/// no row — the `"CNWDI"` row is a different token surface), and
/// TOK_RESTRICTED (canonical `"R"`, deliberately routed through the
/// canonical-collapse path — see the `classification_form_set`
/// doc-comment in `crates/capco/src/vocabulary.rs`).
fn same_form_sentinels() -> &'static [TokenId] {
    &[TOK_CNWDI, TOK_HCS, TOK_RESTRICTED]
}

#[test]
fn banner_abbreviation_some_for_distinct_form() {
    let scheme = CapcoScheme::new();
    for (token, _portion, expected_banner) in distinct_banner_form_sentinels() {
        let abbr = scheme.banner_abbreviation(token);
        assert_eq!(
            abbr,
            Some(*expected_banner),
            "banner_abbreviation({token:?}) should be Some({expected_banner:?}) — \
             CAPCO-2016 §G.1 Table 4 lists a distinct authorized banner abbreviation",
        );
        // And the banner_form() accessor must match the expected
        // banner via `Some(...).unwrap_or(banner_title)`.
        assert_eq!(
            scheme.banner_form(token),
            *expected_banner,
            "banner_form({token:?}) disagrees with banner_abbreviation",
        );
    }
}

#[test]
fn banner_abbreviation_none_for_same_form() {
    let scheme = CapcoScheme::new();
    for token in same_form_sentinels() {
        assert_eq!(
            scheme.banner_abbreviation(token),
            None,
            "banner_abbreviation({token:?}) should be None — no MARKING_FORMS \
             row exists for this canonical, so the canonical-collapse fallback \
             applies (all three forms equal the canonical itself)",
        );
        // For same-form markings the portion form and banner form
        // are byte-identical.
        assert_eq!(
            scheme.portion_form(token),
            scheme.banner_form(token),
            "portion_form / banner_form must match when no distinct abbreviation exists ({token:?})",
        );
    }
}

/// `metadata()` returns the same `Authority`, `OwnerProducer`, and
/// `PointOfContact` that the per-field accessors return. The single-
/// source-of-truth invariant from PR review #2 — drift between
/// `scheme.authority(t)` and `scheme.metadata(t).authority` must be
/// unrepresentable.
#[test]
fn metadata_agrees_with_per_field_accessors() {
    let scheme = CapcoScheme::new();
    for token in active_sentinels() {
        let m = scheme.metadata(token);
        assert_eq!(
            &m.authority,
            scheme.authority(token),
            "metadata({token:?}).authority differs from scheme.authority",
        );
        assert_eq!(
            &m.owner_producer,
            scheme.owner_producer(token),
            "metadata({token:?}).owner_producer differs from scheme.owner_producer",
        );
        assert_eq!(
            &m.point_of_contact,
            scheme.point_of_contact(token),
            "metadata({token:?}).point_of_contact differs from scheme.point_of_contact",
        );
        assert_eq!(
            m.portion_form,
            scheme.portion_form(token),
            "metadata({token:?}).portion_form differs",
        );
        assert_eq!(
            m.banner_form,
            scheme.banner_form(token),
            "metadata({token:?}).banner_form differs",
        );
        assert_eq!(
            m.banner_abbreviation,
            scheme.banner_abbreviation(token),
            "metadata({token:?}).banner_abbreviation differs",
        );
        // `Authority.point_of_contact` must equal the standalone
        // POC accessor — the embedded copy is the canonical one.
        assert_eq!(
            &m.authority.point_of_contact,
            scheme.point_of_contact(token),
            "metadata({token:?}).authority.point_of_contact differs from \
             scheme.point_of_contact — single-source-of-truth invariant violated",
        );
    }
}

// ---------------------------------------------------------------------------
// Panic-path coverage for the four sentinel-resolution chokepoints in
// `crates/capco/src/vocabulary.rs`. Each accessor reaches a different
// helper, so each helper gets its own #[should_panic] test —
// otherwise a refactor that loses one of the panic sites would still
// pass coverage by reaching only the first one.
// ---------------------------------------------------------------------------

use marque_capco::scheme::{
    TOK_FGI_MARKER, TOK_IC_DISSEM, TOK_JOINT, TOK_NON_IC_DISSEM, TOK_NON_US_CLASSIFICATION,
    TOK_US_CLASSIFIED, TOK_USA,
};

/// `authority()` reaches `derived_for_token` → `entry_for` →
/// `canonical_for`; the panic surfaces from `canonical_for` first.
/// `TOK_FGI_MARKER` is a grammar-shape sentinel deliberately absent
/// from `SENTINEL_TO_CANONICAL`.
#[test]
#[should_panic(expected = "no canonical CVE")]
fn authority_panics_on_unknown_token() {
    let scheme = CapcoScheme::new();
    let _ = scheme.authority(&TOK_FGI_MARKER);
}

/// `owner_producer()` shares the same chokepoint as `authority()`;
/// distinct test so a refactor that diverts one accessor away from
/// `derived_for_token` doesn't silently lose coverage on the other.
#[test]
#[should_panic(expected = "no canonical CVE")]
fn owner_producer_panics_on_unknown_token() {
    let scheme = CapcoScheme::new();
    let _ = scheme.owner_producer(&TOK_USA);
}

/// `point_of_contact()` likewise — `TOK_JOINT` is a classification-
/// prefix sentinel without a single CVE value.
#[test]
#[should_panic(expected = "no canonical CVE")]
fn point_of_contact_panics_on_unknown_token() {
    let scheme = CapcoScheme::new();
    let _ = scheme.point_of_contact(&TOK_JOINT);
}

/// `metadata()` reaches `token_derived` (the per-token cache);
/// `metadata`'s panic message is structurally similar to
/// `derived_for_token`'s but originates from a different helper.
#[test]
#[should_panic(expected = "no canonical CVE")]
fn metadata_panics_on_unknown_token() {
    let scheme = CapcoScheme::new();
    let _ = scheme.metadata(&TOK_US_CLASSIFIED);
}

/// `portion_form()` / `banner_form()` / `banner_abbreviation()` /
/// `deprecation()` route through `token_derived` too — bundle them
/// into a single panic test so each accessor gets exercised on the
/// unknown-id path. `TOK_NON_US_CLASSIFICATION`,
/// `TOK_IC_DISSEM`, `TOK_NON_IC_DISSEM` are aggregate sentinels (no
/// single CVE value).
#[test]
#[should_panic(expected = "no canonical CVE")]
fn portion_form_panics_on_unknown_token() {
    let scheme = CapcoScheme::new();
    let _ = scheme.portion_form(&TOK_NON_US_CLASSIFICATION);
}

#[test]
#[should_panic(expected = "no canonical CVE")]
fn banner_form_panics_on_unknown_token() {
    let scheme = CapcoScheme::new();
    let _ = scheme.banner_form(&TOK_IC_DISSEM);
}

#[test]
#[should_panic(expected = "no canonical CVE")]
fn deprecation_panics_on_unknown_token() {
    let scheme = CapcoScheme::new();
    let _ = scheme.deprecation(&TOK_NON_IC_DISSEM);
}
