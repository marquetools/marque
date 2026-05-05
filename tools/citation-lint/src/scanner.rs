// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! AST-based scanner that extracts every citation occurrence from a
//! Rust source file.
//!
//! The scanner walks `crates/*/src/**/*.rs` workspace-wide (FR-018
//! scope: every domain rule crate, current and future) and feeds the
//! contents through `syn` for an AST parse. Three classes of
//! occurrence are extracted:
//!
//! 1. **Struct field literals** named `citation:`, `message:`, or
//!    `constraint_label:` whose value is a string literal. This
//!    catches the common cases — `Constraint { citation: "...", ... }`,
//!    `Diagnostic { message: "...", ... }`, etc.
//! 2. **Generic string literals**. Any string literal anywhere in the
//!    source. We deliberately err on the side of false positives here
//!    — a `§X.Y pNN`-shaped fragment in a string literal should be a
//!    citation regardless of context, and the resolver disambiguates
//!    by attempting to resolve it.
//! 3. **Doc-comment attributes**. `#[doc = "..."]` (which `syn`
//!    surfaces uniformly with `///` and `//!`) often contains
//!    citations in rule documentation.
//!
//! The legacy `line NNNN` form is detected by a separate textual
//! pass (`find_legacy_line_form`) because it doesn't share the
//! `§`-prefixed grammar — it's a retired citation shape, not a
//! malformed citation. The pass runs over the raw source, NOT the
//! AST, because a legacy-form citation in a comment doesn't reach
//! `syn::Attribute` (line comments are stripped before AST parse).
//!
//! Why AST scanning instead of regex over source: a regex match for
//! `§X.Y pNN` would false-positive on `cfg!`-gated code, on strings
//! constructed via `format!` argument lists, on comments adjacent to
//! disabled blocks, and on doc-test code fences. The AST gives us
//! exactly the surfaces a maintainer would care about — values
//! reachable by the rule machinery — and excludes the dead corners.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use proc_macro2::Span;
use syn::visit::Visit;
use syn::{Attribute, Expr, ExprLit, FieldValue, Lit, Member};
use walkdir::WalkDir;

use crate::citation::{CitationFind, find_in_fragment};
use crate::diagnostic::{Defect, DefectClass, SourceKind};

/// Workspace-relative scan: walks every `crates/*/src/**/*.rs` file
/// under `workspace_dir` and records every citation occurrence.
///
/// Returns `(occurrences, legacy_form_defects)`. The two are kept
/// separate so the caller can resolve occurrences against the source
/// index and emit the legacy-form defects directly without a
/// resolution step.
pub fn scan_workspace(workspace_dir: &Path) -> Result<(Vec<Occurrence>, Vec<Defect>)> {
    let mut occurrences = Vec::new();
    let mut legacy_defects = Vec::new();
    let crates_dir = workspace_dir.join("crates");
    if !crates_dir.is_dir() {
        anyhow::bail!(
            "workspace_dir {} does not contain a `crates/` directory; expected a marque workspace root",
            workspace_dir.display()
        );
    }
    // Build the candidate member-directory set: every entry under
    // `crates/` plus every top-level workspace-root sibling dir that
    // contains a `Cargo.toml` (e.g., `marque/` for the CLI binary,
    // and any future top-level workspace member). We then scan each
    // member's `src/` subtree.
    //
    // Tests under `<member>/tests/` are NOT in scope for citation
    // lint — citations belong in rule sources, not test fixtures.
    //
    // Out-of-scope by construction:
    // - `tools/` — out-of-workspace per Constitution III; contains
    //   the citation-lint binary itself plus other dev tooling.
    // - Hidden dirs (`.git/`, `.worktrees/`), `target/`, `node_modules/`,
    //   build artifacts.
    let mut member_dirs: Vec<PathBuf> = Vec::new();
    // Two-level: every entry under `crates/`.
    for entry in fs::read_dir(&crates_dir)
        .with_context(|| format!("reading {}", crates_dir.display()))?
        .filter_map(Result::ok)
    {
        let p = entry.path();
        if p.is_dir() {
            member_dirs.push(p);
        }
    }
    // One-level: every workspace-root sibling that has a Cargo.toml
    // and is not a known out-of-scope path. We deliberately do NOT
    // parse `[workspace.members]` from the root Cargo.toml here —
    // the directory-presence check is sufficient and avoids dragging
    // a TOML parser into this binary for something the filesystem
    // already tells us.
    let skip_top_level: &[&str] = &["crates", "tools", "target", "docs", "site", "tests", "benches"];
    for entry in fs::read_dir(workspace_dir)
        .with_context(|| format!("reading {}", workspace_dir.display()))?
        .filter_map(Result::ok)
    {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if name.starts_with('.') || skip_top_level.contains(&name) {
            continue;
        }
        if p.join("Cargo.toml").is_file() {
            member_dirs.push(p);
        }
    }
    // Sort for reproducible output.
    member_dirs.sort();
    for member_dir in member_dirs {
        let src_dir = member_dir.join("src");
        if !src_dir.is_dir() {
            continue;
        }
        let mut files: Vec<PathBuf> = WalkDir::new(&src_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_type().is_file()
                    && e.path().extension().and_then(|s| s.to_str()) == Some("rs")
                    && !e.path().components().any(|c| c.as_os_str() == "target")
            })
            .map(walkdir::DirEntry::into_path)
            .collect();
        files.sort();
        for file in files {
            scan_file(&file, &mut occurrences, &mut legacy_defects)?;
        }
    }
    Ok((occurrences, legacy_defects))
}

/// One citation occurrence in source: either a parsed citation or a
/// bare-section structural defect that the resolver doesn't process
/// (it's a parser-level fault, not a resolution failure).
#[derive(Debug, Clone)]
pub struct Occurrence {
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
    pub source_kind: SourceKind,
    pub find: CitationFind,
}

/// Parse one `.rs` file and record every citation occurrence and
/// every legacy `line NNNN` defect.
fn scan_file(
    path: &Path,
    occurrences: &mut Vec<Occurrence>,
    legacy_defects: &mut Vec<Defect>,
) -> Result<()> {
    let source = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    // Legacy `line NNNN` form scan FIRST — runs textually because
    // line comments are stripped before AST parse.
    find_legacy_line_form(path, &source, legacy_defects);
    // AST scan.
    let parsed = syn::parse_file(&source).with_context(|| format!("parsing {}", path.display()))?;
    let mut visitor = CitationVisitor {
        file: path.to_path_buf(),
        occurrences: Vec::new(),
    };
    visitor.visit_file(&parsed);
    occurrences.append(&mut visitor.occurrences);
    Ok(())
}

/// Detect retired `line NNNN` citation form. Looks for the literal
/// pattern in source content (comments and string literals alike,
/// since the AST strips line comments). The pattern is intentionally
/// narrow: requires the literal `line ` (the word `line` followed by
/// exactly one space) followed by 3+ digits, and that the match is
/// adjacent to a citation context (`§` symbol or `CAPCO-2016` token
/// within the same line). Without the adjacency check, every
/// reference to "line 245" in unrelated prose comments would
/// false-positive.
///
/// We do not normalize whitespace before matching — the retired form
/// in the codebase always used a single space, and matching on
/// `\s+` would surface false positives where formatter wrap-points
/// land between `line` and the digit run in unrelated prose.
fn find_legacy_line_form(path: &Path, source: &str, out: &mut Vec<Defect>) {
    for (idx, line) in source.lines().enumerate() {
        // Cheap pre-filter: must contain a citation context anchor.
        if !line.contains('§') && !line.contains("CAPCO-2016") {
            continue;
        }
        // Find every `line NNNN` (3+ digits) in the line.
        let bytes = line.as_bytes();
        let mut i = 0;
        while i + 5 < bytes.len() {
            // Look for the literal byte sequence "line " (case-insensitive
            // l/L) at a word boundary. Word-boundary check: previous byte
            // must be non-alphabetic or this is column 0.
            let prev_is_boundary = i == 0 || !bytes[i - 1].is_ascii_alphabetic();
            let matches_line = (bytes[i] == b'l' || bytes[i] == b'L')
                && (bytes[i + 1] == b'i')
                && (bytes[i + 2] == b'n')
                && (bytes[i + 3] == b'e')
                && (bytes[i + 4] == b' ');
            if prev_is_boundary && matches_line {
                let mut k = i + 5;
                let digit_start = k;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                let digit_count = k - digit_start;
                if digit_count >= 3 {
                    let raw = &line[i..k];
                    out.push(Defect {
                        file: path.to_path_buf(),
                        line: (idx + 1) as u32,
                        column: (i + 1) as u32,
                        source_kind: SourceKind::RawText,
                        raw: raw.to_string(),
                        class: DefectClass::LegacyLineForm {
                            line_form: raw.to_string(),
                        },
                        recommended: Some(
                            "use page anchor `pNN` instead — line numbers retired in commit b340bec"
                                .into(),
                        ),
                    });
                }
                i = k;
                continue;
            }
            i += 1;
        }
    }
}

/// AST visitor that records every citation-bearing source position.
struct CitationVisitor {
    file: PathBuf,
    occurrences: Vec<Occurrence>,
}

impl CitationVisitor {
    /// Extract citations from `text` at AST `span`, classified as
    /// `kind`.
    ///
    /// CAPCO-context filtering: a citation found inside a generic
    /// string literal or doc comment is only treated as a CAPCO
    /// citation if the surrounding context indicates CAPCO. Without
    /// this, `§3 of the Phase B design doc` and `Spec 005 §R3` would
    /// be misclassified as CAPCO-citation defects (FR-018 scope is
    /// "cited authority", which means CAPCO-2016 references in this
    /// codebase, not arbitrary `§`-prefixed cross-references).
    ///
    /// `citation:`, `message:`, and `constraint_label:` field values
    /// are always treated as CAPCO context — the FR-018 wording
    /// explicitly enumerates these as the CAPCO-citation surfaces.
    /// Other surfaces require an explicit CAPCO anchor:
    ///
    /// - The literal `CAPCO-2016` token in `text`, OR
    /// - A `§<NORMATIVE_LETTER>.<digits>` form in `text` (the section
    ///   letter is the CAPCO signal — letters A–K are the CAPCO
    ///   range, and a fully-qualified `§A.5` shape doesn't appear in
    ///   internal plan docs).
    fn record(&mut self, text: &str, span: Span, kind: SourceKind) {
        let finds = find_in_fragment(text);
        let is_implicit_capco = matches!(
            kind,
            SourceKind::CitationField | SourceKind::MessageField | SourceKind::ConstraintLabel
        );
        let is_capco_context = is_implicit_capco || text_indicates_capco(text);
        for find in finds {
            // Skip non-CAPCO bare-section finds in non-CAPCO contexts.
            // A `§3` in a doc comment without CAPCO anchor is almost
            // certainly an internal-plan reference, not an FR-018
            // defect. Resolved citations (i.e., `§X.Y` form) are
            // still recorded so the resolver gets a chance — a
            // `§Z.5` in a doc-comment is plausibly a real defect
            // (someone tried to cite CAPCO with the wrong letter)
            // and we want it surfaced.
            if !is_capco_context {
                if matches!(find, CitationFind::BareSection { .. }) {
                    continue;
                }
                // Even for parsed finds, if the context isn't CAPCO
                // and the section letter is outside A-K, treat it as
                // an internal cross-reference and skip.
                if let CitationFind::Parsed { citation, .. } = &find {
                    if !is_normative_or_known(citation.section) {
                        continue;
                    }
                }
            }
            let (line, column) = compute_line_col(span, find.offset(), text);
            self.occurrences.push(Occurrence {
                file: self.file.clone(),
                line,
                column,
                source_kind: kind,
                find,
            });
        }
        // Also detect doubled page-anchor form here. The pattern is
        // `p<digits>(-|–)<digits> p<digits>` where the trailing page
        // matches the second page in the range — the FR-020 known
        // defect (`p150–151 p151`). We detect it textually because
        // it's a textual artifact rather than a structural
        // mis-resolution. The `pp NN-MM pMM` form is NOT currently in
        // scope; if it appears in a future catalog, extend
        // `find_doubled_page_anchor` to cover it.
        if let Some((match_offset, suspect)) = find_doubled_page_anchor(text) {
            // Use compute_line_col so the diagnostic points at the
            // start of the matched substring, not the start of the
            // enclosing literal — the catalog is more useful when the
            // column lands on the actual `pNN-MM pMM` text.
            let (line, column) = compute_line_col(span, match_offset, text);
            self.occurrences.push(Occurrence {
                file: self.file.clone(),
                line,
                column,
                source_kind: kind,
                // Re-encode as a synthetic "find" that the resolver
                // ignores; the catalog emitter will surface it.
                find: CitationFind::BareSection {
                    offset: match_offset,
                    raw: format!("__doubled_page_anchor__:{suspect}"),
                },
            });
        }
    }

    fn record_attribute(&mut self, attr: &Attribute) {
        // `#[doc = "..."]` (also produced by `///` and `//!`).
        if !attr.path().is_ident("doc") {
            return;
        }
        if let Ok(name_value) = attr.meta.require_name_value() {
            if let Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) = &name_value.value
            {
                let value = s.value();
                self.record(&value, s.span(), SourceKind::DocComment);
            }
        }
    }
}

impl<'ast> Visit<'ast> for CitationVisitor {
    fn visit_field_value(&mut self, node: &'ast FieldValue) {
        // `Foo { citation: "CAPCO-2016 §X.Y pNN", ... }`. Match by
        // member ident.
        let member_name = match &node.member {
            Member::Named(ident) => ident.to_string(),
            Member::Unnamed(_) => String::new(),
        };
        let kind = match member_name.as_str() {
            "citation" => Some(SourceKind::CitationField),
            "message" => Some(SourceKind::MessageField),
            "constraint_label" => Some(SourceKind::ConstraintLabel),
            _ => None,
        };
        if let Some(kind) = kind {
            if let Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) = &node.expr
            {
                let value = s.value();
                self.record(&value, s.span(), kind);
                // Returning here would skip nested field values; keep walking.
            }
        }
        syn::visit::visit_field_value(self, node);
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        // Catch-all for citations in string literals not covered by
        // the `FieldValue` visitor above. The cost of double-counting
        // is dedup'd by the (file, line, column, raw) tuple at the
        // diagnostic emission stage; we'd rather over-scan than miss.
        //
        // We use `SourceKind::StringLiteral` here, which `record`
        // will overwrite with a more specific kind at `visit_field_value`
        // time only if both visitors fire in the right order. In
        // practice, `visit_field_value` runs first for field-value
        // literals, so the field-value kind wins.
        let value = node.value();
        if value.contains('§') {
            // Cheap pre-filter: only string literals that mention the
            // section sigil produce citation finds. Avoids walking
            // every string in the codebase.
            //
            // Track whether this position has already been recorded
            // from a `FieldValue` visit so we don't double-count.
            // The dedup check happens at the file-level emission
            // stage (see `lib.rs::lint_workspace`), keyed on
            // (file, line, column, raw).
            self.record(&value, node.span(), SourceKind::StringLiteral);
        }
        syn::visit::visit_lit_str(self, node);
    }

    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        self.record_attribute(attr);
        syn::visit::visit_attribute(self, attr);
    }
}

/// Compute (file_line, file_column) for a citation find.
///
/// `span` is the AST span of the enclosing string literal. `offset`
/// is the byte offset of the citation's `§` character within the
/// literal's content. We add the offset to the span start to get the
/// citation's source coordinates.
///
/// **Caveat**: the returned column reflects byte positions inside
/// the string-literal content, not the literal's source position
/// after escape decoding. For the diagnostics we produce that's
/// acceptable — a reviewer opening the file and looking at the line
/// will see the citation, even if the column is approximate. We use
/// `proc_macro2::Span::start()` directly which is line-accurate; the
/// column is the literal-content column with the literal's start-of-
/// content column added on.
fn compute_line_col(span: Span, offset: usize, text: &str) -> (u32, u32) {
    // Count newlines in `text[..offset]` to project the citation onto
    // the correct file line. The first byte of the literal is at
    // `span.start()`. A multi-line string literal places the citation
    // some lines below `span.start()`.
    let prefix = &text[..offset.min(text.len())];
    let nl_count = prefix.matches('\n').count() as u32;
    let line = u32::try_from(span.start().line)
        .unwrap_or(0)
        .saturating_add(nl_count);
    let column = if nl_count == 0 {
        // Same line as the literal's start. proc-macro2 columns are
        // 0-indexed; we present 1-indexed.
        u32::try_from(span.start().column)
            .unwrap_or(0)
            .saturating_add(u32::try_from(offset).unwrap_or(0))
            .saturating_add(2) // +2 to account for the leading `"` of the string literal
            .saturating_add(1) // 0-indexed → 1-indexed
    } else {
        // Citation begins on a continuation line; column is the byte
        // offset within that line (still 1-indexed).
        let last_nl = prefix.rfind('\n').unwrap_or(0);
        let col_within_line = offset - last_nl - 1;
        u32::try_from(col_within_line)
            .unwrap_or(0)
            .saturating_add(1)
    };
    (line, column)
}

/// Detect a doubled page anchor of the shape `pNN-MM pMM` /
/// `pNN–MM pMM`. Returns `(start_offset, matched_substring)` on
/// success — the offset is the byte index of the leading `p` in
/// `text`, suitable for `compute_line_col`. Returns `None` if no
/// match is found.
#[allow(clippy::many_single_char_names)]
fn find_doubled_page_anchor(text: &str) -> Option<(usize, String)> {
    // We look for: `p` <digits>+ <dash> <digits>+ <whitespace>+ `p`
    // <digits>+, where the trailing page is identical to the second
    // page in the range OR is otherwise redundant. The narrow form
    // matches FR-020's specific pattern (`p150–151 p151`). The
    // `pp NN-MM pMM` form is NOT in scope.
    let bytes = text.as_bytes();
    if bytes.len() < 8 {
        return None;
    }
    let mut i: usize = 0;
    while i < bytes.len() {
        if bytes[i] != b'p' {
            i += 1;
            continue;
        }
        // Parse `p` + digits.
        let p1_start = i;
        let mut k = i + 1;
        let d1_start = k;
        while k < bytes.len() && bytes[k].is_ascii_digit() {
            k += 1;
        }
        if k == d1_start {
            i += 1;
            continue;
        }
        // Optional dash.
        if let Some(dash_end) = consume_dash(bytes, k) {
            let d2_start = dash_end;
            let mut m = dash_end;
            while m < bytes.len() && bytes[m].is_ascii_digit() {
                m += 1;
            }
            if m == d2_start {
                i = k;
                continue;
            }
            let second_page = text[d2_start..m].to_string();
            // Skip whitespace.
            let mut n = m;
            while n < bytes.len() && (bytes[n] == b' ' || bytes[n] == b'\t') {
                n += 1;
            }
            // Look for `p` + digits matching `second_page`.
            if n < bytes.len() && bytes[n] == b'p' {
                let trailing_digit_start = n + 1;
                let mut t = trailing_digit_start;
                while t < bytes.len() && bytes[t].is_ascii_digit() {
                    t += 1;
                }
                if t > trailing_digit_start {
                    let trailing = &text[trailing_digit_start..t];
                    if trailing == second_page {
                        return Some((p1_start, text[p1_start..t].to_string()));
                    }
                }
            }
            i = m;
            continue;
        }
        i = k;
    }
    None
}

/// Returns true if `text` contains a CAPCO-context anchor — either
/// the literal token `CAPCO-2016` (or `CAPCO `) or a fully-qualified
/// `§<A-K>.<digits>` form. The fully-qualified form is reliable
/// enough to act as an implicit CAPCO signal because internal plan
/// references in this codebase use letterless numerical sections
/// (`§3`, `§R3`, `§4.1` of internal plans).
fn text_indicates_capco(text: &str) -> bool {
    if text.contains("CAPCO-2016") || text.contains("CAPCO ") {
        return true;
    }
    // Fully-qualified `§<letter>.<digit>` where letter is A-K.
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == 0xC2 && bytes[i + 1] == 0xA7 {
            // Skip whitespace after `§`.
            let mut j = i + 2;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            if j + 2 < bytes.len()
                && bytes[j].is_ascii_uppercase()
                && (b'A'..=b'K').contains(&bytes[j])
                && bytes[j + 1] == b'.'
                && bytes[j + 2].is_ascii_digit()
            {
                return true;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    false
}

fn is_normative_or_known(letter: char) -> bool {
    matches!(letter, 'A'..='K')
}

fn consume_dash(bytes: &[u8], cursor: usize) -> Option<usize> {
    if cursor >= bytes.len() {
        return None;
    }
    if bytes[cursor] == b'-' {
        return Some(cursor + 1);
    }
    if cursor + 2 < bytes.len()
        && bytes[cursor] == 0xE2
        && bytes[cursor + 1] == 0x80
        && (bytes[cursor + 2] == 0x93 || bytes[cursor + 2] == 0x94)
    {
        return Some(cursor + 3);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_doubled_page_anchor() {
        let s = "§H.8 p150-151 p151";
        let (offset, found) = find_doubled_page_anchor(s).unwrap();
        assert!(found.contains("p150"), "got {found:?}");
        // Offset must point at the leading `p` of the matched
        // substring (byte index 5 in this fixture: `§` is 2 bytes,
        // `H.8 ` is 4 bytes, totaling 6 — but the literal `§` is 2
        // bytes UTF-8, `H.8 ` adds 4 ASCII bytes = 6, so the leading
        // `p` is at byte index 6). Re-verify if the fixture changes.
        assert_eq!(offset, 6, "offset should point at leading `p` of match");
    }

    #[test]
    fn detects_doubled_page_anchor_with_en_dash() {
        let s = "§H.8 p150–151 p151";
        let (_offset, found) = find_doubled_page_anchor(s).unwrap();
        assert!(found.contains("p150"), "got {found:?}");
    }

    #[test]
    fn does_not_doubled_when_pages_differ() {
        // `p149 p151` is not the doubled-anchor pattern — the trailing
        // page must equal the range-end. Prevents misclassifying a
        // legitimate "see also pX" follow-on.
        let s = "p149-151 p999";
        assert!(find_doubled_page_anchor(s).is_none());
    }

    #[test]
    fn legacy_line_form_detected() {
        let mut out = Vec::new();
        find_legacy_line_form(
            Path::new("test.rs"),
            "// CAPCO-2016 line 4140-4146 — over-restrictive\n",
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0].class, DefectClass::LegacyLineForm { .. }));
    }

    #[test]
    fn legacy_line_form_requires_citation_context() {
        // A bare `line 245` in code without a citation anchor should
        // not be flagged. Otherwise every error message that mentions
        // line numbers would false-positive.
        let mut out = Vec::new();
        find_legacy_line_form(
            Path::new("test.rs"),
            "// see line 245 of helpers.rs for details\n",
            &mut out,
        );
        assert!(out.is_empty(), "got {out:#?}");
    }
}
