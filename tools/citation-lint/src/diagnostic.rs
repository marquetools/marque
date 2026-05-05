// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Diagnostic types and rendering.
//!
//! Each defect found by the lint produces one `Defect` value. Defects
//! are sorted deterministically before rendering so the output is
//! identical across runs (important for the catalog file consumed by
//! PR 0.6).
//!
//! The classification taxonomy is **closed** by design: a new defect
//! class requires extending the `DefectClass` enum, which is a
//! deliberate change reviewers can see in the diff. Coarse "bucket"
//! categories that hide what's actually wrong are not useful as a
//! merge gate input.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// One citation defect.
///
/// Sort order is `(file, line, column, class)` — stable across runs
/// because the underlying scanner walks files in alphabetical order
/// and emits hits in source order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Defect {
    pub file: PathBuf,
    /// 1-indexed source line.
    pub line: u32,
    /// 1-indexed source column where the citation begins.
    pub column: u32,
    /// Source kind: which AST surface the citation was extracted from.
    pub source_kind: SourceKind,
    /// Verbatim citation text from the source, for human display.
    pub raw: String,
    /// Why this is a defect.
    pub class: DefectClass,
    /// Optional remediation hint (e.g., "should be `§H.4`"). `None`
    /// when the lint cannot infer a correction.
    pub recommended: Option<String>,
}

/// Where the citation was extracted from. The mapping back to AST
/// surface helps a reviewer understand the diagnostic context — a
/// `§4` in a `citation:` field is often a different kind of defect
/// than a `§4` in a doc-comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// `citation: "CAPCO-2016 §X.Y pNN"` struct field literal.
    CitationField,
    /// `message: "..."` struct field literal.
    MessageField,
    /// `constraint_label: "..."` struct field literal.
    ConstraintLabel,
    /// `///` outer doc-comment, `//!` inner doc-comment, or
    /// `#[doc = "..."]` attribute.
    DocComment,
    /// String literal anywhere else (used for any other top-level
    /// strings the scanner picks up).
    StringLiteral,
}

/// Closed classification of citation defects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefectClass {
    /// `§NN` form without subsection letter (e.g., `§4 p99`).
    /// FR-018-rejected: a bare numeric section is ambiguous because
    /// the document has multiple sections that contain the same
    /// subsection number (e.g., §1 of A, §1 of B, §1 of G all
    /// exist).
    BareSection,
    /// `§X` form (letter only) for a section that DOES have numbered
    /// subsections in the source. `§F` is currently lawful because
    /// §F has no subsections; `§A` would not be (§A has subsections
    /// 1–7 in 2016).
    LetterOnlyButSectionHasSubsections { letter: char },
    /// Section letter is outside the normative range (A–H). The
    /// non-normative ranges are §I (banner-line history), §J (marking
    /// examples), §K (acronym list); anything beyond §K is unknown.
    NonNormativeSection { letter: char },
    /// Section letter is not present in the document at all.
    UnknownSection { letter: char },
    /// Subsection number is not registered for this section. The
    /// section exists but the specific subsection number does not
    /// resolve in the source.
    UnknownSubsection { letter: char, number: u32 },
    /// Page anchor falls outside the section's actual page range.
    /// A common variant of this is the §H.4-vs-§4 fabrication: the
    /// rule cites a real page but with the wrong section letter, and
    /// the page is outside the cited section.
    PageOutOfRange {
        section_start: u32,
        section_end: u32,
        cited_start: u32,
        cited_end: u32,
    },
    /// Page anchor falls outside the document's page range entirely.
    PageOutOfDocument { max_page: u32, cited_page: u32 },
    /// Doubled page-anchor form, e.g., `§H.8 p150–151 p151`. The
    /// scanner detects this textually because it is an FR-020 known
    /// defect class. The duplicated page reference is the trailing
    /// `pNN` after a complete `pp NN–MM` form.
    DoubledPageAnchor { suspect: String },
    /// Retired `line NNNN` citation form (project retired this in
    /// commit `b340bec` — page numbers only).
    LegacyLineForm { line_form: String },
}

impl DefectClass {
    /// One-line human description for the catalog and stderr.
    pub fn summary(&self) -> String {
        match self {
            DefectClass::BareSection => "bare section without subsection letter".into(),
            DefectClass::LetterOnlyButSectionHasSubsections { letter } => format!(
                "§{letter} cited without subsection number; §{letter} has numbered subsections, so a specific subsection must be cited"
            ),
            DefectClass::NonNormativeSection { letter } => {
                format!(
                    "non-normative §{letter} (history/examples/acronyms — not a citation target)"
                )
            }
            DefectClass::UnknownSection { letter } => format!("unknown section §{letter}"),
            DefectClass::UnknownSubsection { letter, number } => {
                format!("unknown subsection §{letter}.{number}")
            }
            DefectClass::PageOutOfRange {
                section_start,
                section_end,
                cited_start,
                cited_end,
            } => format!(
                "page out of section range (section spans pp {section_start}–{section_end}, citation cites pp {cited_start}–{cited_end})"
            ),
            DefectClass::PageOutOfDocument {
                max_page,
                cited_page,
            } => {
                format!("page p{cited_page} exceeds document max p{max_page}")
            }
            DefectClass::DoubledPageAnchor { suspect } => {
                format!("doubled page anchor: {suspect:?}")
            }
            DefectClass::LegacyLineForm { line_form } => {
                format!("retired `line NNNN` citation form: {line_form:?}")
            }
        }
    }

    /// Stable identifier used in JSON / catalog output.
    pub fn class_id(&self) -> &'static str {
        match self {
            DefectClass::BareSection => "bare-section",
            DefectClass::LetterOnlyButSectionHasSubsections { .. } => {
                "letter-only-needs-subsection"
            }
            DefectClass::NonNormativeSection { .. } => "non-normative-section",
            DefectClass::UnknownSection { .. } => "unknown-section",
            DefectClass::UnknownSubsection { .. } => "unknown-subsection",
            DefectClass::PageOutOfRange { .. } => "page-out-of-range",
            DefectClass::PageOutOfDocument { .. } => "page-out-of-document",
            DefectClass::DoubledPageAnchor { .. } => "doubled-page-anchor",
            DefectClass::LegacyLineForm { .. } => "legacy-line-form",
        }
    }
}

/// Render a defect as a single CI-style stderr line:
/// `path:line:col: <class_id>: <summary>: <raw>`
pub fn render_stderr(d: &Defect) -> String {
    format!(
        "{}:{}:{}: {}: {}: {:?}",
        d.file.display(),
        d.line,
        d.column,
        d.class.class_id(),
        d.class.summary(),
        d.raw,
    )
}

/// Sort defects deterministically. Required for reproducibility (the
/// catalog file is consumed by PR 0.6 and committed; nondeterministic
/// ordering would produce phantom diffs).
pub fn sort(defects: &mut [Defect]) {
    defects.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.class.class_id().cmp(b.class.class_id()))
    });
}
