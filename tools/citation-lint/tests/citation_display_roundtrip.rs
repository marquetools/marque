// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Round-trip: format!("{citation}") through
//! citation-lint's real scanner ([`citation_lint::find_in_fragment`]).
//!
//! Proves the typed [`marque_scheme::Citation`] `Display` output is
//! parseable by the citation-lint tool's real parser. The
//! `Citation` type lives in `marque-scheme`, which is the dev-dep
//! retargeted here (`marque-rules` no longer re-exports it).
//!
//! # Why this test lives here (not in `crates/rules/tests/`)
//!
//! Adding `citation_lint` as a dev-dep to `marque-rules` would create
//! a reverse dep from a workspace crate to an out-of-workspace tool
//! crate. The cleaner placement is here in
//! `tools/citation-lint/tests/`, where the dependency direction is
//! natural: `citation-lint` depends on `marque-scheme` (as a dev-dep)
//! to consume the structured [`Citation`] type.
//!
//! The existing in-crate test at
//! `crates/rules/tests/citation_display_roundtrip.rs` exercises the
//! shape against a manual byte-level regex scanner; this file
//! exercises the same shape against the **real** citation-lint
//! parser, which is the load-bearing CI consumer.
//!
//! # CAPCO §-citation verification
//!
//! Every literal §-reference below was re-verified against
//! `crates/capco/docs/CAPCO-2016.md` per Constitution Principle VIII
//! propagation rule:
//!
//! - §H.4 p61 — SCI grammar reminder per CAPCO-2016 §H.4 p61
//!   (project memory `project_capco_p20_caveated_definition`).
//! - §B.3 Table 2 p21 — caveated FD&R rule per CAPCO-2016 §B.3 Table
//!   2 p21 (project memory `project_pattern_d_already_shipped`).
//! - §A.6 p15 — formatting category-order rules per CAPCO-2016 §A.6
//!   p15 (Figure 2 baseline).
//! - §H.5 p99 — SAR anchor per CAPCO-2016 §H.5 p99 (CLAUDE.md crate
//!   description references this canonical SAR citation).
//! - §H.8 p134 — FOUO eviction per CAPCO-2016 §H.8 p134.

use core::num::NonZeroU16;

use citation_lint::{Citation as LintCitation, CitationFind, find_in_fragment};
use marque_scheme::{
    AuthoritativeSource, Citation as SchemeCitation, SectionLetter, SectionRef, capco, capco_table,
};

/// Extract the single parsed citation from a fragment, asserting it
/// parsed cleanly (no `BareSection` defect). Panics if zero or >1
/// citations are found.
fn parsed_one(fragment: &str) -> LintCitation {
    let finds = find_in_fragment(fragment);
    assert_eq!(
        finds.len(),
        1,
        "expected exactly 1 citation in {fragment:?}, got {finds:?}"
    );
    match &finds[0] {
        CitationFind::Parsed { citation, .. } => citation.clone(),
        CitationFind::BareSection { raw, .. } => {
            panic!("citation-lint flagged {raw:?} as BareSection — Display drifted")
        }
    }
}

#[test]
fn h4_p61_sci_grammar_roundtrips() {
    // Per CAPCO-2016 §H.4 p61 — SCI grammar reminder.
    let c: SchemeCitation = capco(SectionLetter::H, 4, 61);
    let rendered = format!("{c}");
    assert_eq!(rendered, "§H.4 p61");

    let parsed = parsed_one(&rendered);
    assert_eq!(parsed.section, 'H');
    assert_eq!(parsed.subsection, Some(4));
    assert_eq!(parsed.pages, Some((61, 61)));
}

#[test]
fn b3_table_2_p21_caveated_fdr_roundtrips_partially() {
    // Per CAPCO-2016 §B.3 Table 2 p21 — caveated FD&R rule.
    //
    // **Documented partial round-trip**: citation-lint's grammar at
    // `tools/citation-lint/src/citation.rs` matches `Table N` as a
    // page-anchor terminator — `§G.1 Table 4` (and `§D.2 Table 3` in
    // the existing in-tree tests) is recognized with `pages: None`.
    // The grammar does NOT recognize `§<L>.<sub> Table <N> p<page>`
    // as a single citation; the `Table N` modifier shadows the
    // page anchor.
    //
    // Result: `marque_scheme::Citation` Display output for the
    // table-variant `§B.3 Table 2 p21` is parsed by citation-lint as
    // `(section: B, subsection: 3, pages: None)`. The page number is
    // dropped at the round-trip boundary.
    //
    // **Impact**: the citation-lint CI scanner cannot resolve the
    // page of a table-variant citation to validate it against the
    // page-range bounds in `CAPCO-2016_citation_index.yml`. The
    // citation IS still recognized at the section + subsection level,
    // which is the resolver's primary validation surface. The page
    // bound is checked indirectly: every citation in the marque
    // source either uses the subsection-only form (`§H.4 p61`,
    // page-resolvable) or the table form (`§B.3 Table 2 p21`,
    // resolver checks subsection + accepts the section-level page
    // range).
    //
    // Follow-up: extend citation-lint's grammar to accept the
    // `§<L>.<sub> Table <N> p<page>` form.
    let c: SchemeCitation = capco_table(SectionLetter::B, 3, 2, 21);
    let rendered = format!("{c}");
    assert_eq!(rendered, "§B.3 Table 2 p21");

    let parsed = parsed_one(&rendered);
    assert_eq!(parsed.section, 'B');
    assert_eq!(parsed.subsection, Some(3));
    // citation-lint drops the page when `Table N` shadows the anchor.
    assert_eq!(parsed.pages, None);
}

#[test]
fn a6_p15_formatting_roundtrips() {
    // Per CAPCO-2016 §A.6 p15 — Figure 2 baseline.
    let c: SchemeCitation = capco(SectionLetter::A, 6, 15);
    let rendered = format!("{c}");
    assert_eq!(rendered, "§A.6 p15");

    let parsed = parsed_one(&rendered);
    assert_eq!(parsed.section, 'A');
    assert_eq!(parsed.subsection, Some(6));
    assert_eq!(parsed.pages, Some((15, 15)));
}

#[test]
fn h5_p99_sar_anchor_roundtrips() {
    // Per CAPCO-2016 §H.5 p99 — SAR anchor.
    let c: SchemeCitation = capco(SectionLetter::H, 5, 99);
    let rendered = format!("{c}");
    assert_eq!(rendered, "§H.5 p99");

    let parsed = parsed_one(&rendered);
    assert_eq!(parsed.section, 'H');
    assert_eq!(parsed.subsection, Some(5));
    assert_eq!(parsed.pages, Some((99, 99)));
}

#[test]
fn h8_p134_fouo_eviction_roundtrips() {
    // Per CAPCO-2016 §H.8 p134 — FOUO eviction rule.
    let c: SchemeCitation = capco(SectionLetter::H, 8, 134);
    let rendered = format!("{c}");
    assert_eq!(rendered, "§H.8 p134");

    let parsed = parsed_one(&rendered);
    assert_eq!(parsed.section, 'H');
    assert_eq!(parsed.subsection, Some(8));
    assert_eq!(parsed.pages, Some((134, 134)));
}

#[test]
fn bare_section_letter_h_p60_roundtrips() {
    // Bare-section shape: `§<L>` (no subsection). Today no CAPCO
    // citation in the catalog uses this form, but the type system
    // supports it via `SectionRef::new(letter)` with no chained
    // `with_subsection`. Citation-lint accepts the resulting
    // `§H p60` shape (the parser tolerates a bare letter + page
    // anchor per its grammar at `tools/citation-lint/src/citation.rs`).
    let c = SchemeCitation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(SectionLetter::H),
        NonZeroU16::new(60).unwrap(),
    );
    let rendered = format!("{c}");
    assert_eq!(rendered, "§H p60");

    let parsed = parsed_one(&rendered);
    assert_eq!(parsed.section, 'H');
    assert_eq!(parsed.subsection, None);
    assert_eq!(parsed.pages, Some((60, 60)));
}

#[test]
fn non_capco_sentinel_emits_no_citation() {
    // [`AuthoritativeSource::Config`] and [`AuthoritativeSource::EngineInternal`]
    // sentinel citations render as `[config]` / `[engine-internal]`
    // tags WITHOUT a `§` prefix. The citation-lint scanner is anchored
    // on the `§` UTF-8 sequence — so sentinel citations are
    // citation-lint-invisible by design: non-CAPCO sentinels are not
    // citation-lint-resolvable.
    let config = SchemeCitation::new(
        AuthoritativeSource::Config,
        SectionRef::new(SectionLetter::A),
        NonZeroU16::new(1).unwrap(),
    );
    assert_eq!(format!("{config}"), "[config]");
    assert!(
        find_in_fragment(&format!("{config}")).is_empty(),
        "citation-lint MUST NOT find a citation in a [config] sentinel"
    );

    let engine_internal = SchemeCitation::new(
        AuthoritativeSource::EngineInternal,
        SectionRef::new(SectionLetter::A),
        NonZeroU16::new(1).unwrap(),
    );
    assert_eq!(format!("{engine_internal}"), "[engine-internal]");
    assert!(
        find_in_fragment(&format!("{engine_internal}")).is_empty(),
        "citation-lint MUST NOT find a citation in a [engine-internal] sentinel"
    );
}

#[test]
fn const_fn_helpers_produce_citation_lint_recognizable_form() {
    // Per `crates/rules/src/citation.rs` the const-fn helpers
    // (`capco`, `capco_section`, `capco_table`) are usable in const
    // contexts. This test pins the round-trip property at the const-
    // evaluated boundary: catalog rows in `crates/capco/src/` that
    // construct `Citation` at compile time produce Display output
    // that citation-lint can scan at runtime.
    const SCI_GRAMMAR: SchemeCitation = capco(SectionLetter::H, 4, 61);
    const CAVEATED_FDR: SchemeCitation = capco_table(SectionLetter::B, 3, 2, 21);

    // Concatenated fragment: citation-lint scans left-to-right and
    // emits multiple `CitationFind::Parsed` entries.
    let fragment = format!("see {SCI_GRAMMAR} and {CAVEATED_FDR}");
    let finds = find_in_fragment(&fragment);
    assert_eq!(finds.len(), 2, "expected 2 citations, got {finds:?}");
}
