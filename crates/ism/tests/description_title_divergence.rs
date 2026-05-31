// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! ODNI `<Description>` ↔ CAPCO `MarkingForm.title` divergence pin.
//!
//! Walks every row in `MARKING_FORMS` and compares the row's
//! `title` against the ODNI Description text fetched from
//! `marque_ism::generated::vocabulary::lookup_token_metadata`. The
//! count of divergent rows is pinned by `EXPECTED_DIVERGENCES` —
//! the test fails (loudly, with the divergent value pairs) if the
//! count changes between schema versions.
//!
//! ## Why a count pin and not a "≥1" assertion
//!
//! The ODNI XML `<Description>` element is flat text whose body is the
//! ODNI long form; there's no nested `<title>` sub-element.
//! Comparing the trimmed Description against
//! `MarkingForm.title` is a direct string compare.
//!
//! The active ISM-v2022-DEC schema package surfaces nine divergent
//! rows (Class A typos, Class B prose definitions, Class C casing/
//! abbreviation surface differences; see `EXPECTED_DIVERGENCES`
//! doc-comment for the inventory). The test exact-count-pins the
//! divergence so a schema bump that introduces (or removes) a
//! divergent row lands deliberately, not silently.
//!
//! ## What happens when a divergence appears
//!
//! When ODNI revises a Description (or CAPCO revises a title), the
//! test fails with the (canonical, ODNI Description, CAPCO title)
//! triple. The implementer at that point:
//!
//! 1. Reads the new ODNI text against `crates/capco/docs/CAPCO-2016.md`
//!    to determine whether the divergence is a genuine alias or a
//!    typo on either side.
//! 2. If genuine: adds `description_title: Some("<ODNI text>")` to
//!    the matching `MarkingForm` row in
//!    `crates/ism/src/marking_forms.rs`, bumps
//!    `EXPECTED_DIVERGENCES` by 1, and — when the canonical is an
//!    active CAPCO sentinel — extends
//!    `recognized_aliases_for_canonical` in
//!    `crates/capco/src/vocabulary.rs` so `forms()` surfaces the
//!    alias. The round-trip test in
//!    `crates/capco/tests/vocabulary_forms.rs::recognized_aliases_pin_ism_description_divergences`
//!    pins the wired-through subset; this file pins the row-level
//!    data shape.
//! 3. If a typo: files an upstream ODNI / CAPCO bug or a marque
//!    citation-discipline correction.

use marque_ism::generated::vocabulary::lookup_token_metadata;
use marque_ism::marking_forms::MARKING_FORMS;

/// Number of `MARKING_FORMS` rows whose CAPCO `title` field
/// disagrees with the ODNI ISM CVE `<Description>` for the matching
/// canonical value.
///
/// This count must equal both
/// `MARKING_FORMS.iter().filter(|f| f.description_title.is_some()).count()`
/// (the data-shape pin asserted in
/// `description_title_field_populated_for_every_divergence` below)
/// AND the runtime walk count (the original
/// `description_title_divergence_count_matches_pin` test). When the
/// two diverge, the data shape and the runtime detection are out of
/// sync — either a row was set to `Some(_)` for a non-divergent
/// canonical (typo on data side) or a divergent row's
/// `description_title` was missed (typo on update side).
///
/// `9` for ISM-v2022-DEC paired with the active CAPCO §G.1 Table 4
/// transcription in `crates/ism/src/marking_forms.rs`. The
/// divergent rows fall into three classes (validated 2026-05-13):
///
/// **Class A — ODNI typos / casing.** Two cases where the ODNI
/// Description has a clear typographical error or non-canonical
/// casing relative to the CAPCO Register transcription:
/// - `SI-NK` (`"NONBOOK"` vs CAPCO `"SI-NONBOOK"`).
/// - `CNWDI` (`"Controled Nuclear Weapon Design Information Warning
///   statement"` — `Controled` is misspelled and the casing is
///   inconsistent with the rest of the CVE register).
///
/// **Class B — ODNI uses a long descriptive prose form CAPCO §G.1
/// Table 4 does not transcribe.** The CAPCO Register lists a
/// concise title; ODNI's `<Description>` adds a regulatory citation
/// or definition. The CAPCO transcription is correct per §G.1, but
/// the ODNI surface form is admissible on input via the
/// `FormKind::StandardDescriptionTitle` recognize-only channel when
/// `recognized_aliases` is populated:
/// - `FISA` (ODNI adds the "Foreign Intelligence Surveillance Act
///   ... unconsenting individuals ..." citation).
/// - `SSI` (ODNI adds the 49 C.F.R. citation).
/// - `NNPI` (ODNI adds the reactor-safety definition).
///
/// **Class C — Casing / abbreviation surface differences in
/// otherwise structurally equivalent titles.** ODNI uses mixed
/// case or expanded abbreviations CAPCO does not:
/// - `DCNI` (ODNI `"DoD CONTROLLED NUCLEAR INFORMATION"` vs CAPCO
///   `"DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION"` — note
///   CAPCO carries "UNCLASSIFIED" too).
/// - `UCNI` (ODNI `"DoE CONTROLLED NUCLEAR INFORMATION"` vs CAPCO
///   `"DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION"`).
/// - `OC-USGOV` (ODNI `"ORIGINATOR CONTROLLED US GOVERNMENT"` vs
///   CAPCO `"ORIGINATOR CONTROLLED-USGOV"`).
///
/// Each of these is recognized-on-input territory for a follow-on
/// PR (the `recognized_aliases` slot exists for exactly this
/// purpose). The CAPCO transcription is the canonical emission
/// form; the ODNI form is acceptable as an alias.
const EXPECTED_DIVERGENCES: usize = 9;

#[test]
fn description_title_divergence_count_matches_pin() {
    // Walk every MARKING_FORMS row, look up its canonical (we try
    // both portion and banner since rows are keyed on whichever
    // form is the CVE canonical), and compare ODNI Description
    // against CAPCO title.
    let mut divergent: Vec<(&'static str, &'static str, &'static str)> = Vec::new();

    for row in MARKING_FORMS {
        // Try portion first, then banner. The CVE canonical is
        // exactly one of the two for every active row; matching
        // either side picks up the row's ODNI counterpart.
        let entry =
            lookup_token_metadata(row.portion).or_else(|| lookup_token_metadata(row.banner));

        let Some(entry) = entry else {
            // CAPCO row with no ODNI counterpart — common for §G.1
            // Table 4 documentation rows (ATOMAL, BALK, BOHEMIA)
            // that anchor the Register but have no ODNI CVE entry.
            // Cannot diverge from an absent source.
            continue;
        };

        let odni_desc = entry.description.trim();
        if odni_desc.is_empty() {
            // No description text on the ODNI side — vacuously
            // cannot diverge.
            continue;
        }
        if odni_desc != row.title {
            divergent.push((entry.value, odni_desc, row.title));
        }
    }

    assert_eq!(
        divergent.len(),
        EXPECTED_DIVERGENCES,
        "ODNI Description / CAPCO title divergence count changed. \
         Divergent rows:\n{}\n\
         Update EXPECTED_DIVERGENCES in this file after handling \
         each divergent case per the file-level workflow.",
        divergent
            .iter()
            .map(|(v, o, c)| format!("  {v:?}: odni={o:?} capco={c:?}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

#[test]
fn description_title_field_populated_for_every_divergence() {
    // Closes the data-shape ↔ runtime-detection loop. The
    // `description_title_divergence_count_matches_pin` test walks
    // `MARKING_FORMS` at runtime and compares each row's `title` to
    // the looked-up ODNI `<Description>`. This sibling test pins the
    // DATA SHAPE: every divergent row must carry
    // `description_title: Some(odni_desc)`, and the count of `Some`
    // entries must equal `EXPECTED_DIVERGENCES`.
    //
    // Without this pin, a future contributor could add an
    // intentionally-divergent row but forget to populate
    // `description_title` — the runtime test passes, the
    // `crates/capco/tests/vocabulary_forms.rs` round-trip never sees
    // the divergence, and the alias channel goes silently missing.
    let populated = MARKING_FORMS
        .iter()
        .filter(|f| f.description_title.is_some())
        .count();

    assert_eq!(
        populated, EXPECTED_DIVERGENCES,
        "MARKING_FORMS rows with description_title=Some(_) count ({populated}) \
         disagrees with EXPECTED_DIVERGENCES ({EXPECTED_DIVERGENCES}). \
         Either a non-divergent row was populated by mistake, or a \
         divergent row's `description_title` is still `None`.",
    );

    // Per-row consistency: every `Some(text)` must equal the ODNI
    // Description for the same canonical. The runtime walk above
    // already detects divergence between CAPCO `title` and ODNI
    // Description; this pin asserts that when we DO populate
    // `description_title`, the bytes match what the ODNI side
    // actually publishes — no stale or hand-transcribed-wrong text.
    for row in MARKING_FORMS {
        let Some(ism_title) = row.description_title else {
            continue;
        };
        let entry = lookup_token_metadata(row.portion)
            .or_else(|| lookup_token_metadata(row.banner))
            .unwrap_or_else(|| {
                panic!(
                    "MarkingForm row with description_title=Some(_) has \
                     neither portion={:?} nor banner={:?} in TOKEN_METADATA — \
                     unreachable via the runtime divergence walk above. \
                     Remove the description_title or fix the row keys.",
                    row.portion, row.banner,
                )
            });
        let odni_desc = entry.description.trim();
        assert_eq!(
            ism_title, odni_desc,
            "description_title for canonical {:?} disagrees with the \
             ODNI <Description>. Re-fetch the verbatim ODNI text and \
             update the row.",
            entry.value,
        );
    }
}
