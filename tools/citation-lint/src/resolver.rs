// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Resolve a parsed citation against the CAPCO-2016 source index.
//!
//! Resolution semantics:
//!
//! - Section letter MUST be in the normative range A–H.
//! - §I, §J, §K exist in the document but are non-normative
//!   (history, examples, acronyms) and are NOT valid citation
//!   targets. They produce a `NonNormativeSection` defect.
//! - Section letters beyond §K are unknown — `UnknownSection`.
//! - Subsection number MUST resolve in the source's TOC.
//! - When a page anchor is present, BOTH endpoints of the range MUST
//!   resolve (CHK038). The lint surfaces a single defect for the
//!   range; if the start is in-range and the end is not, it is still
//!   one `PageOutOfRange` defect with the full cited range in the
//!   defect payload.
//! - Page anchor MUST be inside the cited subsection's page span.
//!   This is the test that catches §4-vs-§H.4 fabrications: a page
//!   like p62 falls inside §H.4 (which spans pp 60–73) but does not
//!   fall inside §4 (which doesn't exist as a top-level section).
//! - Page anchor MUST be inside the document's page range.

use crate::citation::Citation;
use crate::diagnostic::DefectClass;
use crate::parser::{CapcoIndex, SectionId};

/// The set of valid normative section letters per Constitution VIII +
/// project memory: §A through §H. §I, §J, §K are present in the
/// document but are non-normative.
const NORMATIVE_SECTIONS: &[char] = &['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'];

/// The set of non-normative section letters present in the document.
const NON_NORMATIVE_SECTIONS: &[char] = &['I', 'J', 'K'];

/// Resolve `citation` against `idx`. Returns `None` on success and
/// `Some(class)` on the first detected defect.
///
/// Order of checks is deliberate: classify by section first (cheapest +
/// most informative) before subsection-number checks before page-range
/// checks. A non-normative section with an out-of-range page is
/// reported as `NonNormativeSection`, not `PageOutOfRange` — the
/// underlying defect is the wrong section, not the wrong page.
pub fn resolve(citation: &Citation, idx: &CapcoIndex) -> Option<DefectClass> {
    // Section-letter checks first.
    if NON_NORMATIVE_SECTIONS.contains(&citation.section) {
        return Some(DefectClass::NonNormativeSection {
            letter: citation.section,
        });
    }
    if !NORMATIVE_SECTIONS.contains(&citation.section) {
        return Some(DefectClass::UnknownSection {
            letter: citation.section,
        });
    }
    // Letter-only path: `§X` with no subsection. Lawful only when
    // section X has no numbered subsections in the document. Other-
    // wise the citation is too imprecise to verify against a page.
    let Some(subsection) = citation.subsection else {
        let has_subsections = idx
            .subsections
            .keys()
            .any(|id| id.letter == citation.section);
        if has_subsections {
            return Some(DefectClass::LetterOnlyButSectionHasSubsections {
                letter: citation.section,
            });
        }
        // Letter-only is lawful for this section. Page-anchor check
        // still applies if one is present.
        if let Some((cited_start, cited_end)) = citation.pages {
            return resolve_letter_only_pages(citation.section, cited_start, cited_end, idx);
        }
        return None;
    };
    // Subsection-number check.
    let id = SectionId {
        letter: citation.section,
        number: subsection,
    };
    let Some(subsection_range) = idx.subsection(id) else {
        return Some(DefectClass::UnknownSubsection {
            letter: citation.section,
            number: subsection,
        });
    };
    // Page-anchor checks (only if a page anchor is present).
    let (cited_start, cited_end) = citation.pages?;
    // Document-range check first — a citation to p999 is always
    // wrong, regardless of which subsection it claims to be in.
    if cited_start > idx.max_page || cited_end > idx.max_page {
        return Some(DefectClass::PageOutOfDocument {
            max_page: idx.max_page,
            cited_page: cited_start.max(cited_end),
        });
    }
    // Subsection-range check (CHK038: both endpoints must resolve).
    if !subsection_range.contains(cited_start) || !subsection_range.contains(cited_end) {
        return Some(DefectClass::PageOutOfRange {
            section_start: subsection_range.start,
            section_end: subsection_range.end,
            cited_start,
            cited_end,
        });
    }
    None
}

fn resolve_letter_only_pages(
    letter: char,
    cited_start: u32,
    cited_end: u32,
    idx: &CapcoIndex,
) -> Option<DefectClass> {
    let section_range = idx.section(letter)?;
    if cited_start > idx.max_page || cited_end > idx.max_page {
        return Some(DefectClass::PageOutOfDocument {
            max_page: idx.max_page,
            cited_page: cited_start.max(cited_end),
        });
    }
    if !section_range.contains(cited_start) || !section_range.contains(cited_end) {
        return Some(DefectClass::PageOutOfRange {
            section_start: section_range.start,
            section_end: section_range.end,
            cited_start,
            cited_end,
        });
    }
    None
}

/// Suggest a corrected citation when one is computable. This is a
/// best-effort hint; the lint does not commit to its accuracy. The
/// implementer fixing a defect is expected to re-verify against the
/// source.
///
/// Handles four `DefectClass` variants in two suggestion strategies:
///
/// 1. **Section-shape defects** (`BareSection`, `UnknownSection`,
///    `NonNormativeSection`) — when a page anchor is present, find
///    the unique normative subsection whose page range covers the
///    cited page and suggest that subsection.
/// 2. **`PageOutOfRange`** — find the subsection whose page range
///    actually contains the cited pages, regardless of which
///    subsection the author wrote, and suggest it.
///
/// All four variants share the same lookup primitive
/// (`find_unique_containing_subsection`); the doc previously
/// described "two suggestion classes" — the distinction is two
/// strategies over four `DefectClass` variants. Other variants
/// (`LegacyLineForm`, doubled-page-anchor synthetic) deliberately
/// return `None` here; their guidance is encoded directly at the
/// scanner emission site.
pub fn suggest_correction(
    citation: &Citation,
    class: &DefectClass,
    idx: &CapcoIndex,
) -> Option<String> {
    match class {
        DefectClass::BareSection
        | DefectClass::UnknownSection { .. }
        | DefectClass::NonNormativeSection { .. } => {
            // If a page anchor is present, find the unique subsection
            // that contains it.
            let (start, end) = citation.pages?;
            find_unique_containing_subsection(start, end, idx).map(format_suggestion)
        }
        DefectClass::PageOutOfRange { .. } => {
            // Same approach: find the subsection that actually
            // contains the cited pages, regardless of which one the
            // author wrote.
            let (start, end) = citation.pages?;
            find_unique_containing_subsection(start, end, idx).map(format_suggestion)
        }
        _ => None,
    }
}

fn format_suggestion((id, range): (SectionId, crate::parser::PageRange)) -> String {
    if range.start == range.end {
        format!("{} p{}", id, range.start)
    } else {
        format!("{} pp {}–{}", id, range.start, range.end)
    }
}

/// Find the unique normative subsection whose page range fully
/// contains `[start..=end]`. Returns `None` if zero or multiple
/// subsections match — only an unambiguous suggestion is useful.
fn find_unique_containing_subsection(
    start: u32,
    end: u32,
    idx: &CapcoIndex,
) -> Option<(SectionId, crate::parser::PageRange)> {
    let mut hit: Option<(SectionId, crate::parser::PageRange)> = None;
    for (id, range) in &idx.subsections {
        if !NORMATIVE_SECTIONS.contains(&id.letter) {
            continue;
        }
        if range.contains(start) && range.contains(end) {
            if hit.is_some() {
                return None; // multiple matches → ambiguous
            }
            hit = Some((*id, *range));
        }
    }
    hit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::citation::Citation;
    use crate::parser::CapcoIndex;

    const SYNTHETIC: &str = "\
# Doc

(U)   Table of Contents

A. (U) FOO ............ 5
1. (U) Authority .... 5
2. (U) Purpose ...... 7
B. (U) BAR ............ 10
1. (U) Syntax ....... 10
H. (U) MANUAL ......... 20
4. (U) SCI ........... 20
5. (U) SAP ........... 25
I. (U) HISTORY ........ 30
1. (U) Changes ....... 30

## A. (U) FOO

begin page 5               UNCLASSIFIED
end page 5               UNCLASSIFIED

begin page 7               UNCLASSIFIED
end page 9               UNCLASSIFIED

## B. (U) BAR

begin page 10               UNCLASSIFIED
end page 19               UNCLASSIFIED

## H. (U) MANUAL

begin page 20               UNCLASSIFIED
end page 24               UNCLASSIFIED

begin page 25               UNCLASSIFIED
end page 29               UNCLASSIFIED

## I. (U) HISTORY

begin page 30               UNCLASSIFIED
end page 30               UNCLASSIFIED
";

    fn cite(section: char, subsection: u32, pages: Option<(u32, u32)>) -> Citation {
        Citation {
            section,
            subsection: Some(subsection),
            pages,
            raw: format!("§{section}.{subsection}"),
        }
    }

    fn cite_letter_only(section: char, pages: Option<(u32, u32)>) -> Citation {
        Citation {
            section,
            subsection: None,
            pages,
            raw: format!("§{section}"),
        }
    }

    #[test]
    fn valid_citation_resolves() {
        let idx = CapcoIndex::from_source(SYNTHETIC).unwrap();
        let c = cite('H', 4, Some((22, 22)));
        assert!(resolve(&c, &idx).is_none());
    }

    #[test]
    fn non_normative_section_rejected() {
        let idx = CapcoIndex::from_source(SYNTHETIC).unwrap();
        let c = cite('I', 1, None);
        let d = resolve(&c, &idx).unwrap();
        assert!(matches!(
            d,
            DefectClass::NonNormativeSection { letter: 'I' }
        ));
    }

    #[test]
    fn unknown_section_rejected() {
        let idx = CapcoIndex::from_source(SYNTHETIC).unwrap();
        let c = cite('Z', 1, None);
        let d = resolve(&c, &idx).unwrap();
        assert!(matches!(d, DefectClass::UnknownSection { letter: 'Z' }));
    }

    #[test]
    fn unknown_subsection_rejected() {
        let idx = CapcoIndex::from_source(SYNTHETIC).unwrap();
        let c = cite('A', 99, None);
        let d = resolve(&c, &idx).unwrap();
        assert!(matches!(
            d,
            DefectClass::UnknownSubsection {
                letter: 'A',
                number: 99
            }
        ));
    }

    #[test]
    fn page_out_of_range_within_section() {
        let idx = CapcoIndex::from_source(SYNTHETIC).unwrap();
        // §H.4 spans pp 20-25 inclusive (boundary overlap with H.5
        // which starts at p25). p26 is past that — a real defect.
        let c = cite('H', 4, Some((26, 26)));
        let d = resolve(&c, &idx).unwrap();
        assert!(matches!(d, DefectClass::PageOutOfRange { .. }));
    }

    #[test]
    fn page_out_of_document() {
        let idx = CapcoIndex::from_source(SYNTHETIC).unwrap();
        let c = cite('H', 4, Some((999, 999)));
        let d = resolve(&c, &idx).unwrap();
        assert!(matches!(d, DefectClass::PageOutOfDocument { .. }));
    }

    #[test]
    fn page_range_both_endpoints_must_resolve() {
        let idx = CapcoIndex::from_source(SYNTHETIC).unwrap();
        // §H.4 spans 20-25 (with boundary overlap to H.5 at p25).
        // Range pp 22-26 is partly in, partly out — p26 is past the
        // boundary.
        let c = cite('H', 4, Some((22, 26)));
        let d = resolve(&c, &idx).unwrap();
        assert!(matches!(d, DefectClass::PageOutOfRange { .. }));
    }

    #[test]
    fn suggestion_for_bare_section_with_known_page() {
        let idx = CapcoIndex::from_source(SYNTHETIC).unwrap();
        let c = Citation {
            section: 'A',
            subsection: Some(4),
            pages: Some((22, 22)),
            raw: "§4 p22".into(),
        };
        let suggestion =
            suggest_correction(&c, &DefectClass::BareSection, &idx).expect("should suggest");
        assert!(suggestion.contains("§H.4"), "got {suggestion:?}");
    }

    #[test]
    fn letter_only_for_section_with_subsections_rejected() {
        let idx = CapcoIndex::from_source(SYNTHETIC).unwrap();
        // §A has subsections 1, 2 in the synthetic. `§A` alone is rejected.
        let c = cite_letter_only('A', None);
        let d = resolve(&c, &idx).unwrap();
        assert!(matches!(
            d,
            DefectClass::LetterOnlyButSectionHasSubsections { letter: 'A' }
        ));
    }
}
