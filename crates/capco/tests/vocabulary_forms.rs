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
//! - `scheme.banner_abbreviation(t) == scheme.forms(t).banner_abbreviation`
//!   AND the explicit expected `Option<&str>` recorded in
//!   `EXPECTED_FORMS`. PR 3d.3 widened the table from a 3-tuple to a
//!   4-tuple specifically so the D1 banner_abbreviation flip on
//!   `RD` / `FRD` / `TFNI` (was `None` pre-3d, becomes
//!   `Some("RD")` / `Some("FRD")` / `Some("TFNI")` per FR-053) is
//!   pinned by an explicit `Option` — not via a tautological
//!   `form_set.banner_abbreviation == form_set.banner_abbreviation`
//!   comparison.
//!
//! The expected projection outputs are captured inline as a hand-
//! rolled
//! `&'static [(TokenId, &'static str, &'static str, Option<&'static str>)]`
//! table seeded from the FR-053-corrected behavior. Any future
//! refactor that changes the projection for an active sentinel must
//! update the expected table here in lock-step — the regression is
//! loud.
//!
//! ## What this does NOT pin
//!
//! `forms(t)` reads at every active sentinel return `&'static`
//! data — that invariant is exercised by the
//! `vocabulary_zero_alloc` test (gated on the `count-allocs`
//! feature). This file pins the projection equation; the zero-
//! allocation gate pins the storage model.

use marque_capco::CapcoScheme;
use marque_capco::active_sentinel_count;
use marque_capco::scheme::{
    TOK_ATOMAL, TOK_BALK, TOK_BOHEMIA, TOK_CNWDI, TOK_DCNI, TOK_EXDIS, TOK_FISA, TOK_FRD, TOK_HCS,
    TOK_HCS_O, TOK_HCS_P, TOK_NNPI, TOK_NODIS, TOK_NOFORN, TOK_ORCON_USGOV, TOK_RD, TOK_RESTRICTED,
    TOK_SI_G, TOK_SSI, TOK_TFNI, TOK_TK_BLFH, TOK_TK_IDIT, TOK_TK_KAND, TOK_UCNI,
};
use marque_scheme::{FormKind, TokenId, Vocabulary};

/// Every active sentinel TokenId with its expected
/// `(portion_form, banner_form, banner_abbreviation)` projection.
///
/// `banner_form` is derived per-row in the test loop from
/// `banner_abbreviation.unwrap_or(banner_title)`. `banner_abbreviation`
/// is pinned explicitly as the third element so the D1 / FR-053
/// semantic shift (RD / FRD / TFNI from `None` → `Some(banner)`) is
/// regression-checked against an explicit `Option`, not against
/// `form_set.banner_abbreviation` (which would be tautological).
///
/// ## D1 / FR-053 semantic recap
///
/// Pre-3d code derived `banner_abbreviation` from `banner != portion`.
/// PR 3d's corrected D1 semantic uses `banner != title` (CAPCO §G.1
/// Table 4 col 2 emptiness). The two predicates agree for rows where
/// the banner differs from both portion and title (NOFORN, NODIS,
/// EXDIS, UCNI) and for rows with no MARKING_FORMS entry
/// (canonical-collapse: HCS, RESTRICTED, CNWDI). They DISAGREE for
/// `RD`, `FRD`, `TFNI` — same-form rows where the title is a long
/// descriptive form CAPCO §G.1 Table 4 carries a banner abbreviation
/// for. The flip: pre-3d `None` → 3d `Some("RD")` / `Some("FRD")` /
/// `Some("TFNI")`.
const EXPECTED_FORMS: &[(TokenId, &str, &str, Option<&str>)] = &[
    // (token, expected portion, expected banner, expected banner_abbreviation)
    //
    // NOFORN row: title="NOT RELEASABLE TO FOREIGN NATIONALS",
    // banner="NOFORN", portion="NF". banner != title → Some("NOFORN").
    (TOK_NOFORN, "NF", "NOFORN", Some("NOFORN")),
    // RD row: title="RESTRICTED DATA", banner="RD", portion="RD".
    // banner != title → Some("RD") — the D1 flip.
    (TOK_RD, "RD", "RD", Some("RD")),
    // FRD row: title="FORMERLY RESTRICTED DATA", banner="FRD",
    // portion="FRD". banner != title → Some("FRD") — the D1 flip.
    (TOK_FRD, "FRD", "FRD", Some("FRD")),
    // TFNI row: title="TRANSCLASSIFIED FOREIGN NUCLEAR INFORMATION",
    // banner="TFNI", portion="TFNI". banner != title → Some("TFNI") —
    // the D1 flip.
    (TOK_TFNI, "TFNI", "TFNI", Some("TFNI")),
    // CNWDI sentinel's canonical is "RD-CNWDI" — no MARKING_FORMS
    // row matches (the bare CNWDI row exists at canonical "CNWDI",
    // unreachable through TOK_CNWDI). Canonical-collapse fallback:
    // banner_abbreviation=None.
    (TOK_CNWDI, "RD-CNWDI", "RD-CNWDI", None),
    // UCNI row: title="DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION",
    // banner="DOE UCNI", portion="UCNI". banner != title → Some("DOE UCNI").
    (TOK_UCNI, "UCNI", "DOE UCNI", Some("DOE UCNI")),
    // HCS canonical is "HCS"; no MARKING_FORMS row, canonical-collapse.
    (TOK_HCS, "HCS", "HCS", None),
    // RESTRICTED canonical is "R"; no MARKING_FORMS row, no
    // classification_form_set arm (per byte-identity preservation —
    // see `classification_form_set` doc). Canonical-collapse:
    // banner_abbreviation=None.
    (TOK_RESTRICTED, "R", "R", None),
    // NODIS row: title="NO DISTRIBUTION", banner="NODIS",
    // portion="ND". banner != title → Some("NODIS").
    (TOK_NODIS, "ND", "NODIS", Some("NODIS")),
    // EXDIS row: title="EXCLUSIVE DISTRIBUTION", banner="EXDIS",
    // portion="XD". banner != title → Some("EXDIS").
    (TOK_EXDIS, "XD", "EXDIS", Some("EXDIS")),
    // ----- Issue #407 sentinel additions -----
    //
    // OC-USGOV (ORCON-USGOV) row per §H.8 p139.
    // title="ORIGINATOR CONTROLLED-USGOV", banner="ORCON-USGOV",
    // portion="OC-USGOV". banner != title → Some("ORCON-USGOV").
    (
        TOK_ORCON_USGOV,
        "OC-USGOV",
        "ORCON-USGOV",
        Some("ORCON-USGOV"),
    ),
    // FISA row per §H.8 p161.
    // title="FOREIGN INTELLIGENCE SURVEILLANCE ACT", banner="FISA",
    // portion="FISA". banner != title → Some("FISA").
    (TOK_FISA, "FISA", "FISA", Some("FISA")),
    // SSI row per §H.9 p189.
    // title="SENSITIVE SECURITY INFORMATION", banner="SSI",
    // portion="SSI". banner != title → Some("SSI").
    (TOK_SSI, "SSI", "SSI", Some("SSI")),
    // NNPI row. NNPI has no CAPCO-2016 §-citation (registered in
    // ODNI ISM but governed by separate statutory authority — see
    // `crates/ism/src/attrs.rs::NonIcDissem::Nnpi` doc-comment).
    // title="NAVAL NUCLEAR PROPULSION INFORMATION", banner="NNPI",
    // portion="NNPI". banner != title → Some("NNPI").
    (TOK_NNPI, "NNPI", "NNPI", Some("NNPI")),
    // DCNI (DOD UCNI) row per §H.6 p116.
    // title="DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION",
    // banner="DOD UCNI", portion="DCNI". banner != title →
    // Some("DOD UCNI").
    (TOK_DCNI, "DCNI", "DOD UCNI", Some("DOD UCNI")),
    // ----- Issue #524 (Phase 1) per-compartment SCI sentinel additions -----
    //
    // All six rows are canonical-collapse: no MARKING_FORMS entry
    // exists for these compound forms, so `build_form_set` returns
    // `portion = banner_title = canonical` and
    // `banner_abbreviation = None`. The CAPCO Register §G.1 Table 4
    // describes the per-system parent rows (HCS, SI, TK) but the
    // §H.4 per-compartment templates do not introduce distinct
    // banner abbreviations — the canonical CVE value is used at
    // both portion and banner positions. Authority: §H.4 marking
    // templates (see TOK_* doc-comments in
    // `crates/capco/src/scheme/mod.rs`).
    (TOK_HCS_O, "HCS-O", "HCS-O", None),
    (TOK_HCS_P, "HCS-P", "HCS-P", None),
    (TOK_SI_G, "SI-G", "SI-G", None),
    (TOK_TK_BLFH, "TK-BLFH", "TK-BLFH", None),
    (TOK_TK_IDIT, "TK-IDIT", "TK-IDIT", None),
    (TOK_TK_KAND, "TK-KAND", "TK-KAND", None),
    // ----- Issue #660 NATO program sentinel additions -----
    //
    // ODNI publishes the CVE canonicals as `NATO-ATOMAL`/`NATO-BALK`/
    // `NATO-BOHEMIA` in `CVE_NON_US_CONTROLS`; CAPCO §G.1 Table 4 p37
    // registers them bare (`ATOMAL`/`BALK`/`BOHEMIA`) with no banner
    // abbreviation (col 2 empty → `banner_abbreviation = None`).
    // `nato_program_form_set` in `crates/capco/src/vocabulary.rs`
    // projects the CVE canonical onto the bare display form so the
    // `expected_portion` / `expected_banner` columns below carry the
    // §G.1 Table 4 p37 user-visible bare form, not the CVE canonical.
    //
    // The CVE canonical is reachable through `metadata().canonical`
    // (verified separately in
    // `crates/capco/tests/vocabulary_nato_programs.rs::nato_program_tokens_use_bare_display_forms`).
    //
    // Authority:
    //   - TOK_ATOMAL: §G.1 Table 4 p37 (registration); §H.7 p122
    //     (AEA-axis worked example).
    //   - TOK_BALK: §G.1 Table 4 p37; §G.2 p40 (Table 5 NATO SAP ARH).
    //   - TOK_BOHEMIA: §G.1 Table 4 p37; §G.2 p40; §H.7 p127
    //     (SCI-axis worked example).
    (TOK_ATOMAL, "ATOMAL", "ATOMAL", None),
    (TOK_BALK, "BALK", "BALK", None),
    (TOK_BOHEMIA, "BOHEMIA", "BOHEMIA", None),
];

#[test]
fn expected_forms_covers_full_active_sentinel_set() {
    // Couples `EXPECTED_FORMS` to the authoritative
    // `SENTINEL_TO_CANONICAL` table inside
    // `crates/capco/src/vocabulary.rs`. A future PR that adds a
    // sentinel without extending `EXPECTED_FORMS` fails here loudly
    // rather than silently leaving the new sentinel untested.
    assert_eq!(
        EXPECTED_FORMS.len(),
        active_sentinel_count(),
        "EXPECTED_FORMS row count ({}) disagrees with the active \
         CapcoScheme sentinel set size ({}). Update EXPECTED_FORMS \
         when adding/removing a sentinel in SENTINEL_TO_CANONICAL.",
        EXPECTED_FORMS.len(),
        active_sentinel_count(),
    );
}

#[test]
fn forms_round_trips_for_every_active_sentinel() {
    let scheme = CapcoScheme::new();

    for (token, expected_portion, expected_banner, expected_banner_abbreviation) in EXPECTED_FORMS {
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

        // Default-method projection #3: banner_abbreviation. Two
        // independent assertions — neither comparison is tautological:
        //   (a) `scheme.banner_abbreviation(token)` (the trait
        //       default-method projection) vs the EXPECTED_FORMS pin;
        //   (b) `form_set.banner_abbreviation` (the FormSet field) vs
        //       the EXPECTED_FORMS pin.
        // Pre-3d code (and the post-3d.2 review found) did
        // `scheme.banner_abbreviation(token) == form_set.banner_abbreviation`,
        // which both route through the same FormSet — the D1 flip on
        // RD / FRD / TFNI was therefore unpinned. The EXPECTED_FORMS
        // 4-tuple addition closes that gap.
        assert_eq!(
            scheme.banner_abbreviation(token),
            *expected_banner_abbreviation,
            "banner_abbreviation regression (trait projection) for {token:?}",
        );
        assert_eq!(
            form_set.banner_abbreviation, *expected_banner_abbreviation,
            "banner_abbreviation regression (FormSet field) for {token:?}",
        );
    }
}

/// Sentinels expected to surface a non-empty `recognized_aliases`
/// slice via `forms()`, with the verbatim expected
/// `(FormKind, &'static str)` pair.
///
/// Coverage: only sentinels whose CAPCO canonical (per
/// `SENTINEL_TO_CANONICAL`) matches a `MARKING_FORMS` row with
/// `description_title: Some(_)`. Post-#407 the set has six entries:
/// `TOK_UCNI` (DoE form), `TOK_DCNI` (DoD form), `TOK_ORCON_USGOV`,
/// `TOK_FISA`, `TOK_SSI`, and `TOK_NNPI`. Each row's ODNI Description
/// diverges from CAPCO's authorized title in either casing,
/// abbreviation, or regulatory citation prose; see the per-row
/// `MARKING_FORMS` entries in `crates/ism/src/marking_forms.rs`.
///
/// `TOK_CNWDI`'s `MARKING_FORMS` row (portion="CNWDI", divergent
/// description "Controled Nuclear Weapon Design Information Warning
/// statement") is intentionally NOT in this table — `TOK_CNWDI`'s
/// canonical is `"RD-CNWDI"` (the AEA compound), which doesn't
/// match the bare `"CNWDI"` row. The divergence still surfaces via
/// `crates/ism/tests/description_title_divergence.rs` walking
/// `MARKING_FORMS` directly; it just isn't reachable through
/// `forms(TOK_CNWDI)`.
const EXPECTED_ALIASES: &[(TokenId, &[(FormKind, &str)])] = &[
    (
        TOK_UCNI,
        &[(
            FormKind::IsmDescriptionTitle,
            "DoE CONTROLLED NUCLEAR INFORMATION",
        )],
    ),
    (
        TOK_DCNI,
        &[(
            FormKind::IsmDescriptionTitle,
            "DoD CONTROLLED NUCLEAR INFORMATION",
        )],
    ),
    (
        TOK_ORCON_USGOV,
        &[(
            FormKind::IsmDescriptionTitle,
            "ORIGINATOR CONTROLLED US GOVERNMENT",
        )],
    ),
    (
        TOK_FISA,
        &[(
            FormKind::IsmDescriptionTitle,
            "Foreign Intelligence Surveillance Act. Related to unclassified \
             and declassified information that is collected from \
             unconsenting individuals under the authority of the Foreign \
             Intelligence Surveillance Act (FISA).",
        )],
    ),
    (
        TOK_SSI,
        &[(
            FormKind::IsmDescriptionTitle,
            "Sensitive Security Information. As defined in 49 C.F.R. Part \
             15.5, Sensitive Security Information is information obtained \
             or developed in the conduct of security activities, including \
             research and development, the disclosure of which DOT has \
             determined would constitute an unwarranted invasion of \
             privacy, reveal trade secrets or privileged or confidential \
             information, or be detrimental to transportation safety. As \
             defined in 49 C.F.R. Part 1520.5, Sensitive Security \
             Information is information obtained or developed in the \
             conduct of security activities, including research and \
             development, the disclosure of which DHS/TSA has determined \
             would, among other things, be detrimental to the security \
             of transportation.",
        )],
    ),
    (
        TOK_NNPI,
        &[(
            FormKind::IsmDescriptionTitle,
            "Naval Nuclear Propulsion Information. Related to the safety \
             of reactors and associated naval nuclear propulsion plants, \
             and control of radiation and radioactivity associated with \
             naval nuclear propulsion activities, including prescribing \
             and enforcing standards and regulations for these areas as \
             they affect the environment and the safety and health of \
             workers, operators, and the general public.",
        )],
    ),
];

#[test]
fn recognized_aliases_consistency_with_marking_forms_description_title() {
    // PR 3d.3 wiring-consistency invariant. For every active
    // sentinel `t`:
    //   - If `MARKING_FORMS` has a row matching `canonical_for(t)`
    //     AND that row carries `description_title: Some(ism_title)`,
    //     then `forms(t).recognized_aliases` MUST contain a
    //     `(FormKind::IsmDescriptionTitle, ism_title)` entry.
    //   - Else, `forms(t).recognized_aliases` MUST be empty (or
    //     contain only non-IsmDescriptionTitle entries — future
    //     `HistoricalAlias` channel).
    //
    // A future PR that adds `description_title: Some(_)` to a row
    // matching an active sentinel without extending
    // `recognized_aliases_for_canonical` in
    // `crates/capco/src/vocabulary.rs` fails here loudly.
    use marque_ism::marking_forms::MARKING_FORMS;

    let scheme = CapcoScheme::new();

    for (token, expected_portion, _, _) in EXPECTED_FORMS {
        let form_set = scheme.forms(token);

        // Find the MARKING_FORMS row matching this sentinel's
        // canonical portion. We use `expected_portion` (the
        // EXPECTED_FORMS row's portion column) as the canonical —
        // it equals `canonical_for(token)` for every row whose
        // canonical resolves through `MARKING_FORMS` directly.
        // Sentinels with no MARKING_FORMS row (HCS, RESTRICTED,
        // CNWDI via canonical-collapse) will find no match here,
        // which is the correct behavior — they have no
        // description_title to alias.
        let row = MARKING_FORMS
            .iter()
            .find(|f| f.portion == *expected_portion || f.banner == *expected_portion);

        match row.and_then(|r| r.description_title) {
            Some(ism_title) => {
                let found = form_set.recognized_aliases.iter().any(|(kind, alias)| {
                    matches!(kind, FormKind::IsmDescriptionTitle) && *alias == ism_title
                });
                assert!(
                    found,
                    "MARKING_FORMS row for sentinel {token:?} carries \
                     description_title={ism_title:?} but \
                     forms(t).recognized_aliases does not include an \
                     IsmDescriptionTitle entry for it. Extend \
                     `recognized_aliases_for_canonical` in \
                     crates/capco/src/vocabulary.rs.",
                );
            }
            None => {
                let stray = form_set
                    .recognized_aliases
                    .iter()
                    .find(|(kind, _)| matches!(kind, FormKind::IsmDescriptionTitle));
                assert!(
                    stray.is_none(),
                    "Sentinel {token:?} has no MARKING_FORMS row with \
                     description_title=Some(_) but its FormSet carries \
                     an IsmDescriptionTitle alias: {stray:?}. Either \
                     populate the MARKING_FORMS row or remove the \
                     stray entry from `recognized_aliases_for_canonical`.",
                );
            }
        }
    }
}

#[test]
fn recognized_aliases_pin_ism_description_divergences() {
    // PR 3d.3 closes the T058h spec checkpoint
    // (`tasks.md:188`): "the divergent ISM title surfaces in
    // `recognized_aliases` with `FormKind::IsmDescriptionTitle`".
    //
    // For every active sentinel:
    //   - If the sentinel is in EXPECTED_ALIASES, assert byte-identity
    //     against the pinned `(FormKind, &str)` pair list.
    //   - Otherwise, assert `recognized_aliases` is empty.
    //
    // The previous PR 3d.2 test `recognized_aliases_empty_at_pr_3d`
    // asserted the OPPOSITE of T058h (universal emptiness); it is
    // retired in favor of this round-trip test.
    let scheme = CapcoScheme::new();
    for (token, _, _, _) in EXPECTED_FORMS {
        let form_set = scheme.forms(token);
        let expected = EXPECTED_ALIASES
            .iter()
            .find(|(t, _)| t == token)
            .map(|(_, aliases)| *aliases)
            .unwrap_or(&[]);
        assert_eq!(
            form_set.recognized_aliases, expected,
            "recognized_aliases mismatch for {token:?}",
        );
    }
}
