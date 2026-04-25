// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Trait-level tests for `impl Vocabulary<CapcoScheme> for CapcoScheme`
//! (Phase 5 PR-2, tasks T071 / T072 / T073 / T074 / T076 / T077).
//!
//! `marque-ism` ships the raw per-token tables (Phase 5 PR-1, tested
//! in `crates/ism/tests/vocabulary_tables.rs`); this file exercises
//! the `marque-scheme::Vocabulary<S>` adapter that composes them
//! into the trait surface.
//!
//! ## Token coverage scope
//!
//! `CapcoScheme::Token = TokenId` and the active sentinel set today
//! is ~14 hand-assigned ids in `crates/capco/src/scheme.rs` — the
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
//! Phase C extends both the sentinel set and the canonical-string
//! mapping to cover the full CVE-published token vocabulary; until
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
// T071 — every active token has authority + owner/producer + POC + forms.
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
// T072 — authority traces back to ODNI for every ISM token.
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
// T073 — active tokens have no deprecation metadata.
// ---------------------------------------------------------------------------

/// No active sentinel today is a deprecated marking — every entry in
/// `active_sentinels()` is a current, valid CAPCO token. This test
/// asserts the *negative* case: an active token returns `None` from
/// `deprecation()`. The complementary positive case (T074) lives
/// below; both arms of the `Option` are exercised between the two.
///
/// When Phase C extends the sentinel set to include deprecated tokens
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
// T074 — replacement is populated when a deprecation maps to a known token.
// ---------------------------------------------------------------------------

/// Pin-down test for the deprecation-replacement contract.
///
/// `Deprecation { since, replacement: Option<S::Token> }` carries
/// `replacement = Some(_)` when the deprecated token has a known
/// canonical successor in the active vocabulary, and `None` when
/// "no known replacement" (FR-017 — silence is informative; do not
/// rewrite into a token that does not exist).
///
/// The `MIGRATIONS` table in `marque-ism::generated::migrations`
/// today has two entries (`25X1-` → `25X1`, `50X1-` → `50X1-HUM`),
/// neither of which corresponds to a `CapcoScheme` sentinel — so no
/// active sentinel currently has a `Some(replacement)` deprecation.
/// This test asserts the structural property: every replacement that
/// IS populated must be a TokenId that resolves cleanly in the same
/// vocabulary (no dangling pointers). Phase C will extend the
/// sentinel set to include real deprecations and this test will
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
// T076 — FOUO remains an active dissem control after the FOUO→CUI removal.
// ---------------------------------------------------------------------------

/// Constructs a FOUO-bearing document and runs it through `Engine::lint`,
/// asserting no diagnostic suggests migrating FOUO to CUI. The legacy
/// `FOUO → CUI` migration entry was retired in Phase E
/// (`crates/ism/build.rs` doc-comment under `MIGRATIONS`); FOUO remains
/// enumerated in `CVEnumISMDissem.json` and is a current, valid CAPCO
/// dissem control. Any rule that proposes a CUI replacement on FOUO is
/// a regression of FR-020.
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

// T077 (zero-allocation regression gate) lives in its own integration
// file `tests/vocabulary_zero_alloc.rs`, gated at the FILE level on
// `#![cfg(feature = "count-allocs")]`. Isolating it to its own binary
// keeps the global counting allocator's measurements free of noise
// from the other integration tests in this file (which would
// otherwise allocate freely inside the same process and inflate the
// shared counter even with a `MEASURE_LOCK`).
