// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Library entry point for the `citation-lint` binary.
//!
//! See `src/main.rs` for the CLI driver and `README.md` for design.

#![deny(rust_2018_idioms)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    // The CI tool runs on 64-bit hosts; usize→u32 casts on file/line
    // counts are bounded by file size in practice. Allow for the
    // ergonomic gain over `u32::try_from(...).unwrap_or(0)` everywhere.
    clippy::cast_possible_truncation,
    // The pedantic doc-markdown lint complains about reasonable
    // identifier-shaped phrases like `(file_line, file_column)`
    // appearing in prose; adding backticks everywhere reduces
    // legibility for what's already a comment.
    clippy::doc_markdown,
    // `must_use` on every accessor is noise for a private binary.
    clippy::must_use_candidate
)]

pub mod catalog;
pub mod citation;
pub mod diagnostic;
pub mod parser;
pub mod resolver;
pub mod scanner;

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;

pub use citation::{Citation, CitationFind, find_in_fragment};
pub use diagnostic::{Defect, DefectClass, SourceKind};
pub use parser::{CapcoIndex, PageRange, SectionId};
pub use resolver::{resolve, suggest_correction};
pub use scanner::{Occurrence, scan_workspace};

/// Run the full lint over `workspace_dir`. Loads
/// `crates/capco/docs/CAPCO-2016.md` as the source of truth.
///
/// Returns the deterministically-sorted defect list. The catalog
/// file is written by the caller (the binary), not here, so the
/// library can be exercised by tests without touching disk.
pub fn lint_workspace(workspace_dir: &Path) -> Result<Vec<Defect>> {
    let capco_path = workspace_dir
        .join("crates")
        .join("capco")
        .join("docs")
        .join("CAPCO-2016.md");
    let idx = CapcoIndex::from_file(&capco_path)?;
    let (occurrences, mut defects) = scan_workspace(workspace_dir)?;
    // Deduplicate (file, line, column, raw) tuples — the AST
    // visitor double-fires for `FieldValue` literals (also caught by
    // `LitStr` visitor). We pick the most specific source kind.
    let mut seen: BTreeSet<(std::path::PathBuf, u32, u32, String)> = BTreeSet::new();
    let mut deduped: Vec<Occurrence> = Vec::with_capacity(occurrences.len());
    // Sort so that more-specific source kinds come first; the dedup
    // keeps the first occurrence of each (file, line, col, raw).
    let mut occurrences = occurrences;
    occurrences.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(
                source_kind_specificity(a.source_kind).cmp(&source_kind_specificity(b.source_kind)),
            )
    });
    for occ in occurrences {
        let raw = match &occ.find {
            CitationFind::Parsed { citation, .. } => citation.raw.clone(),
            CitationFind::BareSection { raw, .. } => raw.clone(),
        };
        let key = (occ.file.clone(), occ.line, occ.column, raw);
        if seen.insert(key) {
            deduped.push(occ);
        }
    }
    // Resolve each occurrence and emit a defect when applicable.
    for occ in &deduped {
        match &occ.find {
            CitationFind::Parsed { citation, .. } => {
                if let Some(class) = resolve(citation, &idx) {
                    let recommended = suggest_correction(citation, &class, &idx);
                    defects.push(Defect {
                        file: occ.file.clone(),
                        line: occ.line,
                        column: occ.column,
                        source_kind: occ.source_kind,
                        raw: citation.raw.clone(),
                        class,
                        recommended,
                    });
                }
            }
            CitationFind::BareSection { raw, .. } => {
                // Two flavors: real bare-section defects and the
                // synthetic `__doubled_page_anchor__:...` marker the
                // scanner injects for FR-020 detection.
                if let Some(suspect) = raw.strip_prefix("__doubled_page_anchor__:") {
                    defects.push(Defect {
                        file: occ.file.clone(),
                        line: occ.line,
                        column: occ.column,
                        source_kind: occ.source_kind,
                        raw: suspect.to_string(),
                        class: DefectClass::DoubledPageAnchor {
                            suspect: suspect.to_string(),
                        },
                        recommended: Some(
                            "remove the trailing `pNN` — `pp NN-MM` already covers both pages"
                                .into(),
                        ),
                    });
                } else {
                    defects.push(Defect {
                        file: occ.file.clone(),
                        line: occ.line,
                        column: occ.column,
                        source_kind: occ.source_kind,
                        raw: raw.clone(),
                        class: DefectClass::BareSection,
                        recommended: None,
                    });
                }
            }
        }
    }
    diagnostic::sort(&mut defects);
    Ok(defects)
}

/// Specificity ordering for source kinds — more specific kinds win
/// during dedup. Citation field > Message field > Constraint label >
/// Doc comment > String literal > Raw text. `RawText` is least
/// specific because it represents pre-AST raw-line scans (e.g., the
/// legacy-line-form detector) that cannot disambiguate between line
/// comments, doc comments, and string literals. AST-derived kinds
/// always win when both surface the same defect.
fn source_kind_specificity(kind: SourceKind) -> u8 {
    match kind {
        SourceKind::CitationField => 0,
        SourceKind::MessageField => 1,
        SourceKind::ConstraintLabel => 2,
        SourceKind::DocComment => 3,
        SourceKind::StringLiteral => 4,
        SourceKind::RawText => 5,
    }
}
