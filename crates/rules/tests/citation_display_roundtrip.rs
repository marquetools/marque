// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Golden + property tests for [`marque_scheme::Citation`] `Display`
//! (PR 10.A.1 moved `Citation` from `marque-rules` to `marque-scheme`
//! so the scheme-level catalog rows can carry typed citations without
//! inverting the crate dependency graph).
//!
//! Lands in PR 3c.2.A per `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md`
//! PM-7. The golden tests pin the exact citation-lint regex shape
//! against representative CAPCO citation forms; the property test
//! asserts that `format!("{citation}")` matches the citation-lint
//! regex shape for arbitrary valid `Citation` constructions.
//!
//! Citation-lint at `tools/citation-lint/` is the load-bearing
//! consumer: PR 3c.2.C migrates `Diagnostic.citation: &'static str` to
//! `Diagnostic.citation: Citation` and citation-lint's AST scanner
//! reads the structured value. If `Display` ever drifts from the
//! citation-lint expected shape, the round-trip property test fails
//! before C ever reaches CI.
//!
//! # CAPCO §-citation verification
//!
//! Every literal CAPCO §-reference below was re-verified against
//! `crates/capco/docs/CAPCO-2016.md` at PR 3c.2.A authorship per
//! Constitution Principle VIII. The verifications:
//!
//! - §H.4 p61 — SCI grammar reminder per CAPCO-2016 §H.4 p61
//!   (compartment 2–3 alpha, sub-compartment 4–6 alnum; multi-value
//!   separators `/` between control systems, `-` between control and
//!   compartment, ` ` between sub-compartments).
//! - §B.3 Table 2 p21 — caveated FD&R rule per CAPCO-2016 §B.3 Table
//!   2 p21 (classified + caveated + post-28-Jun-2010 → NOFORN). Project
//!   memory `project_capco_p20_caveated_definition` anchors this
//!   citation shape.
//! - §H.5 p99 — SAR per CAPCO-2016 §H.5 (anchor citation).
//! - §A.6 p15 — formatting category-order rules per CAPCO-2016 §A.6
//!   p15 (Figure 2 baseline).
//! - §H.8 p134 — FOUO eviction per CAPCO-2016 §H.8 p134 (FOUO is not
//!   conveyed in the banner line if the document is UNCLASSIFIED with
//!   FOUO and other non-FD&R dissemination control markings).

use core::num::{NonZeroU8, NonZeroU16};
use marque_scheme::{AuthoritativeSource, Citation, SectionLetter, SectionRef};

/// Helper: construct a Citation against CAPCO-2016 succinctly.
fn capco(letter: SectionLetter, sub: u8, page: u16) -> Citation {
    Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(letter).with_subsection(NonZeroU8::new(sub).unwrap()),
        NonZeroU16::new(page).unwrap(),
    )
}

fn capco_with_table(letter: SectionLetter, sub: u8, table: u8, page: u16) -> Citation {
    Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(letter)
            .with_subsection(NonZeroU8::new(sub).unwrap())
            .with_table(NonZeroU8::new(table).unwrap()),
        NonZeroU16::new(page).unwrap(),
    )
}

// ---------------------------------------------------------------------------
// Golden tests — exact citation-lint regex shape
// ---------------------------------------------------------------------------

#[test]
fn display_subsection_only_h4_p61() {
    // Per CAPCO-2016 §H.4 p61 — SCI grammar reminder. Verified at PR
    // 3c.2.A authorship.
    let c = capco(SectionLetter::H, 4, 61);
    assert_eq!(format!("{c}"), "§H.4 p61");
}

#[test]
fn display_subsection_plus_table_b3_table_2_p21() {
    // Per CAPCO-2016 §B.3 Table 2 p21 — caveated FD&R rule. Verified
    // at PR 3c.2.A authorship; project memory
    // `project_capco_p20_caveated_definition` anchors this exact form.
    let c = capco_with_table(SectionLetter::B, 3, 2, 21);
    assert_eq!(format!("{c}"), "§B.3 Table 2 p21");
}

#[test]
fn display_capco_a6_p15_formatting() {
    // Per CAPCO-2016 §A.6 p15 — Figure 2 baseline (one of the most-
    // cited CAPCO sections in marque source per the citation index).
    // Verified at PR 3c.2.A authorship.
    let c = capco(SectionLetter::A, 6, 15);
    assert_eq!(format!("{c}"), "§A.6 p15");
}

#[test]
fn display_capco_h5_p99_sar_anchor() {
    // Per CAPCO-2016 §H.5 p99 — SAR (Special Access Program) anchor.
    // Verified at PR 3c.2.A authorship; cited in CLAUDE.md crate
    // descriptions as the canonical SAR reference.
    let c = capco(SectionLetter::H, 5, 99);
    assert_eq!(format!("{c}"), "§H.5 p99");
}

#[test]
fn display_capco_h8_p134_fouo_eviction() {
    // Per CAPCO-2016 §H.8 p134 — FOUO eviction rule. Verified at PR
    // 3c.2.A authorship; project memory
    // `project_noforn_supremacy_composition` anchors this citation
    // (Pattern B — classification or any non-FD&R control evicts FOUO).
    let c = capco(SectionLetter::H, 8, 134);
    assert_eq!(format!("{c}"), "§H.8 p134");
}

#[test]
fn display_bare_section_letter_no_subsection() {
    // `§<L>` shape (no subsection). Today no CAPCO citation in the
    // catalog uses this form (every CAPCO citation has at least a
    // subsection), but the shape is representable so future grammars
    // whose top-level sections aren't subdivided can use it.
    let c = Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(SectionLetter::H),
        NonZeroU16::new(60).unwrap(),
    );
    assert_eq!(format!("{c}"), "§H p60");
}

// ---------------------------------------------------------------------------
// Property test — `format!("{citation}")` matches the citation-lint regex
// ---------------------------------------------------------------------------

use proptest::prelude::*;

/// Citation-lint regex form (mirrors the shape parsed by
/// `tools/citation-lint/src/scanner.rs`).
///
/// Matches:
///   `§<L>[.<sub>][ Table <N>] p<page>`
///
/// where `<L>` is `A..=H`, every numeric component is `1..=255` (page
/// is `1..=65535`).
fn matches_citation_lint_form(s: &str) -> bool {
    // Manual byte-level scan keeps the test free of an additional
    // `regex` dev-dep.
    let bytes = s.as_bytes();
    let mut i = 0;

    // `§` is UTF-8 0xC2 0xA7 (2 bytes).
    if bytes.len() < 2 || bytes[0] != 0xC2 || bytes[1] != 0xA7 {
        return false;
    }
    i += 2;

    // <Letter> in A..=H
    if i >= bytes.len() {
        return false;
    }
    let letter = bytes[i];
    if !(b'A'..=b'H').contains(&letter) {
        return false;
    }
    i += 1;

    // [. <subsection>]
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return false;
        }
    }

    // [ Table <N>]
    const TABLE_PREFIX: &[u8] = b" Table ";
    if bytes[i..].starts_with(TABLE_PREFIX) {
        i += TABLE_PREFIX.len();
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return false;
        }
    }

    // ` p<page>`
    if i >= bytes.len() || bytes[i] != b' ' {
        return false;
    }
    i += 1;
    if i >= bytes.len() || bytes[i] != b'p' {
        return false;
    }
    i += 1;
    let start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == start {
        return false;
    }

    // Must be fully consumed
    i == bytes.len()
}

fn arb_section_letter() -> impl Strategy<Value = SectionLetter> {
    prop_oneof![
        Just(SectionLetter::A),
        Just(SectionLetter::B),
        Just(SectionLetter::C),
        Just(SectionLetter::D),
        Just(SectionLetter::E),
        Just(SectionLetter::F),
        Just(SectionLetter::G),
        Just(SectionLetter::H),
    ]
}

fn arb_section_ref() -> impl Strategy<Value = SectionRef> {
    (
        arb_section_letter(),
        proptest::option::of(1u8..=255),
        proptest::option::of(1u8..=255),
    )
        .prop_map(|(letter, sub, table)| {
            let mut r = SectionRef::new(letter);
            if let Some(s) = sub {
                r = r.with_subsection(NonZeroU8::new(s).unwrap());
            }
            if let Some(t) = table {
                r = r.with_table(NonZeroU8::new(t).unwrap());
            }
            r
        })
}

fn arb_citation() -> impl Strategy<Value = Citation> {
    (arb_section_ref(), 1u16..=65535).prop_map(|(section, page)| {
        Citation::new(
            AuthoritativeSource::Capco2016,
            section,
            NonZeroU16::new(page).unwrap(),
        )
    })
}

proptest! {
    /// `format!("{citation}")` MUST match the citation-lint regex form
    /// for any well-formed `Citation`. This is the load-bearing
    /// round-trip with citation-lint: PR 3c.2.C's
    /// `Diagnostic.citation: &'static str → Citation` migration relies
    /// on `Display` output being shape-correct so the lint can scan
    /// structured values rather than re-derive structure from a
    /// regex.
    #[test]
    fn display_matches_citation_lint_form(c in arb_citation()) {
        let s = format!("{c}");
        prop_assert!(
            matches_citation_lint_form(&s),
            "Citation Display output {:?} does not match citation-lint regex form",
            s
        );
    }
}

#[cfg(test)]
mod scan_self_tests {
    //! Round-trip sanity for the regex scanner used by the property
    //! test. If this scanner accepts something the citation-lint AST
    //! parser would reject, the property test underfires.

    use super::matches_citation_lint_form;

    #[test]
    fn scanner_accepts_known_good_forms() {
        assert!(matches_citation_lint_form("§H.4 p61"));
        assert!(matches_citation_lint_form("§B.3 Table 2 p21"));
        assert!(matches_citation_lint_form("§A.6 p15"));
        assert!(matches_citation_lint_form("§H p60"));
    }

    #[test]
    fn scanner_rejects_known_bad_forms() {
        // Line-number anchor — retired per project memory
        // `feedback_citations_use_page_numbers`.
        assert!(!matches_citation_lint_form("§H.4 line1234"));
        // Bare section letter (no page).
        assert!(!matches_citation_lint_form("§H"));
        // Non-normative section letter (§I-K excluded).
        assert!(!matches_citation_lint_form("§I.1 p1"));
        // Missing § sigil.
        assert!(!matches_citation_lint_form("H.4 p61"));
        // Trailing garbage.
        assert!(!matches_citation_lint_form("§H.4 p61 extra"));
    }

    #[test]
    fn scanner_accepts_table_without_subsection() {
        // The Citation type allows constructing `SectionRef::new(H)
        // .with_table(2)` (subsection = None, table = Some(2)). No
        // CAPCO citation in the catalog uses this shape today (every
        // table cites lives inside a subsection — §B.3 Table 2,
        // §D.2 Table 3, etc.), but the type system does not forbid it,
        // and Display emits `§H Table 2 p21` which is shape-grammatical
        // under the citation-lint regex form.
        assert!(matches_citation_lint_form("§H Table 2 p21"));
    }
}
