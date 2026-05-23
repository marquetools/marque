// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 2/3: token extraction and structural parsing.
//!
//! Takes [`MarkingCandidate`] spans from the scanner and produces
//! [`marque_ism::ParsedAttrs`]. The engine then runs
//! `MarkingScheme::canonicalize` — the trait route, the sole
//! production path; for CAPCO that is
//! `marque_capco::CapcoScheme::canonicalize` — to land owned
//! [`marque_ism::CanonicalAttrs`] for rule consumption.
//!
//! # Phase 2 — Token Extraction
//! A compile-time Aho-Corasick automaton (built from CVE token list in marque-capco)
//! runs over each candidate span, identifying known tokens and their positions.
//! Unrecognized tokens within a candidate boundary are themselves diagnostics.
//!
//! # Phase 3 — Structural Parsing
//! Token sequence → ParsedAttrs<'src>. Validates ordering and block structure.
//! Produces `ParseError` for structural violations; these feed into the rule engine
//! as diagnostics with associated fixes.
//!
//! Note: the Aho-Corasick automaton is injected via `TokenSet` to keep marque-core
//! free of a direct dependency on marque-capco's generated data.

use crate::error::CoreError;
use marque_ism::attrs::{
    AeaMarking, Classification, CountryCode, DeclassExemption, DissemControl, FgiClassification,
    FgiMarker, ForeignClassification, JointClassification, MarkingClassification,
    NatoClassification, NatoSap, NonIcDissem, SarCompartment, SarIndicator, SarMarking, SarProgram,
    SciCompartment, SciControl, SciControlBare, SciControlSystem, SciMarking, TokenKind, TokenSpan,
};
use marque_ism::date::IsmDate;
use marque_ism::is_bare_cve_value;
use marque_ism::parsed::{
    ParsedAea, ParsedAttrs, ParsedClassification, ParsedDeclassifyOn, ParsedDisplayOnlyEntry,
    ParsedDissem, ParsedFgiMarker, ParsedNonIcDissem, ParsedRelToEntry, ParsedSarMarking,
    ParsedSciMarking, SourceOrigin,
};
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::TokenSet;
use marque_scheme::Span;
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::str::FromStr;

/// Parse result for a single candidate.
///
/// Carries a borrow into the original source bytes via `attrs` (each
/// `Parsed*<'src>` wrapper retains its source slice). Short-lived: the
/// engine immediately canonicalizes to `CanonicalAttrs` via
/// `MarkingScheme::canonicalize` (the sole production path;
/// the CAPCO override lives in `CapcoScheme::canonicalize`).
#[derive(Debug)]
pub struct ParsedMarking<'src> {
    pub attrs: ParsedAttrs<'src>,
    pub source_span: Span,
    pub kind: MarkingType,
}

/// Phase 2+3 parser. Stateless; call [`Parser::parse`] per candidate.
pub struct Parser<'t> {
    tokens: &'t dyn TokenSet,
    /// IC dissem attribution fallback for portions with no
    /// classification axis. The post-parse `attribute_dissems` pass
    /// uses it; CAPCO callers leave this at the
    /// default ([`DefaultOrigin::Us`]) so that no-context portions
    /// attribute their dissems to `dissem_us`. A future
    /// foreign-origin-dominant scheme can override via
    /// [`Self::with_default_origin`].
    default_origin: marque_ism::DefaultOrigin,
}

impl<'t> Parser<'t> {
    pub fn new(tokens: &'t dyn TokenSet) -> Self {
        Self {
            tokens,
            default_origin: marque_ism::DefaultOrigin::Us,
        }
    }

    /// Override the no-classification-context fallback for IC dissem
    /// attribution. CAPCO's
    /// [`marque_ism::DefaultOrigin::Us`] is the default; pass
    /// [`marque_ism::DefaultOrigin::Nato`] for a foreign-origin
    /// dominant context.
    pub fn with_default_origin(mut self, origin: marque_ism::DefaultOrigin) -> Self {
        self.default_origin = origin;
        self
    }

    /// Parse a single scanner candidate into [`ParsedAttrs`].
    pub fn parse<'src>(
        &self,
        candidate: &MarkingCandidate,
        source: &'src [u8],
    ) -> Result<ParsedMarking<'src>, CoreError> {
        let text = candidate
            .span
            .as_str(source)
            .map_err(|_| CoreError::InvalidUtf8(candidate.span))?;
        match candidate.kind {
            MarkingType::Portion => self.parse_portion(text, candidate),
            MarkingType::Banner => self.parse_banner(text, candidate),
            MarkingType::Cab => self.parse_cab(text, candidate),
            // PageBreak candidates are scanner-emitted boundaries with no
            // parsable content. Engine::lint filters them out before calling
            // `parse`; reaching this arm is a programming error in the
            // pipeline, so a `MalformedMarking` is the right surface.
            MarkingType::PageBreak => Err(CoreError::MalformedMarking(
                "page-break candidate must not be parsed".to_owned(),
            )),
            // PageFinalization candidates are engine-synthesized
            // dispatch markers, never scanner-emitted. They reach
            // `Phase::PageFinalization` rules through the engine's
            // synthetic-dispatch path and never enter the parser.
            // Reaching this arm is a programming error in the
            // pipeline, same as `PageBreak`; surface
            // `MalformedMarking` so the failure stays loud rather
            // than silently parsing as something else.
            MarkingType::PageFinalization => Err(CoreError::MalformedMarking(
                "page-finalization candidate must not be parsed".to_owned(),
            )),
            // `MarkingType` is `#[non_exhaustive]` (issue #461). A
            // future variant should fail loudly at parse time so the
            // pipeline maintainer sees it explicitly; do NOT
            // silently drop into a permissive arm.
            _ => Err(CoreError::MalformedMarking(
                "unsupported candidate kind for parser".to_owned(),
            )),
        }
    }

    fn parse_portion<'src>(
        &self,
        text: &'src str,
        candidate: &MarkingCandidate,
    ) -> Result<ParsedMarking<'src>, CoreError> {
        // Strip outer parentheses: "(TS//SI//NF)" -> "TS//SI//NF"
        // The inner-string offset is `candidate.span.start + 1` because
        // the leading `(` is one byte (verified ASCII by the scanner).
        let inner = text
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| CoreError::MalformedMarking(text.to_owned()))?;

        let attrs = self.parse_marking_string(
            inner,
            MarkingType::Portion,
            candidate.span.start + 1,
            SourceOrigin::Portion,
        )?;
        Ok(ParsedMarking {
            attrs,
            source_span: candidate.span,
            kind: MarkingType::Portion,
        })
    }

    fn parse_banner<'src>(
        &self,
        text: &'src str,
        candidate: &MarkingCandidate,
    ) -> Result<ParsedMarking<'src>, CoreError> {
        // For banner candidates, `text` is the full line bytes from the
        // scanner. `text.trim()` may consume leading whitespace, which
        // shifts the per-token offsets. Compute the leading whitespace
        // length so we can add it to candidate.span.start.
        let trimmed = text.trim_start();
        let lead_ws = text.len() - trimmed.len();
        let trimmed = trimmed.trim_end();
        let attrs = self.parse_marking_string(
            trimmed,
            MarkingType::Banner,
            candidate.span.start + lead_ws,
            SourceOrigin::Banner,
        )?;
        Ok(ParsedMarking {
            attrs,
            source_span: candidate.span,
            kind: MarkingType::Banner,
        })
    }

    fn parse_cab<'src>(
        &self,
        text: &'src str,
        candidate: &MarkingCandidate,
    ) -> Result<ParsedMarking<'src>, CoreError> {
        // CAB is line-structured: "Classified By: ...\nDerived From: ...\nDeclassify On: ..."
        // CAB markings borrow `&'src str` for free-text fields directly so the
        // canonicalizer's round-trip property sees the bytes the
        // parser actually consumed.
        let mut classified_by: Option<&'src str> = None;
        let mut derived_from: Option<&'src str> = None;
        let mut declassify_on: Option<ParsedDeclassifyOn<'src>> = None;
        let mut declass_exemption: Option<DeclassExemption> = None;

        for line in text.lines() {
            if let Some(val) = line.strip_prefix("Classified By:") {
                classified_by = Some(val.trim());
            } else if let Some(val) = line.strip_prefix("Derived From:") {
                derived_from = Some(val.trim());
            } else if let Some(val) = line.strip_prefix("Declassify On:") {
                let s = val.trim();
                if let Some(exemption) = DeclassExemption::parse(s) {
                    declass_exemption = Some(exemption);
                } else {
                    // Attempt to parse as a typed IsmDate (YYYY, YYYYMMDD,
                    // YYYY-MM-DD, etc.). Unrecognized strings are silently
                    // dropped rather than stored as raw text, since the field
                    // is now typed.
                    if let Ok(date) = IsmDate::from_str(s) {
                        // `s` is the trimmed value of the `Declassify On:`
                        // line — a subslice of `text`, the candidate's
                        // backing `&str`. Span is full-width over `s` so
                        // `bytes`/`span` agree and round-trip is exact.
                        // The pointer-arithmetic offset is safe here: `s`
                        // is borrowed from `text` and Rust guarantees
                        // `s.as_ptr() >= text.as_ptr()` for a slice
                        // relationship that holds by the construction
                        // chain `text.lines() → strip_prefix → trim`.
                        let abs_start =
                            candidate.span.start + (s.as_ptr() as usize - text.as_ptr() as usize);
                        declassify_on = Some(ParsedDeclassifyOn::new(
                            date,
                            s,
                            Span::new(abs_start, abs_start + s.len()),
                        ));
                    }
                }
            }
        }

        Ok(ParsedMarking {
            attrs: ParsedAttrs::new(
                None,
                Box::new([]),
                Box::new([]),
                None,
                Box::new([]),
                None,
                Box::new([]), // dissem_us
                Box::new([]), // dissem_nato
                Box::new([]),
                Box::new([]),
                Box::new([]), // display_only_to (CAB has no DISPLAY ONLY axis)
                declassify_on,
                classified_by,
                derived_from,
                declass_exemption,
                Box::new([]),
                SourceOrigin::Cab,
            ),
            source_span: candidate.span,
            kind: MarkingType::Cab,
        })
    }
}

mod canonicalizers;
mod dissem;
mod fgi;
mod marking;
mod nato;
mod sar;
mod sci;
mod separator;

#[allow(unused_imports)]
use canonicalizers::*;
#[allow(unused_imports)]
use dissem::*;
#[allow(unused_imports)]
use fgi::*;
#[allow(unused_imports)]
use nato::*;
#[allow(unused_imports)]
use sar::*;
#[allow(unused_imports)]
use sci::*;
#[allow(unused_imports)]
use separator::*;

/// Parse a classification string in either portion form (`"TS"`, `"S"`, `"C"`,
/// `"R"`, `"U"`) or banner form (`"TOP SECRET"`, `"SECRET"`, ...).
///
/// Includes RESTRICTED/R for foreign-origin markings (between U and C).
///
/// Note: `Classification` is hand-written in `marque-ism::attrs` rather than
/// generated from the CVE because the CVE only ships single-letter abbreviations
/// and the tool needs both forms. Other CVE-derived enums (`SciControl`,
/// `DissemControl`, `DeclassExemption`) go through their generated `parse()`
/// methods. SAR is structural (not CVE-backed) and handled separately.
fn parse_classification(s: &str) -> Option<Classification> {
    match s {
        "TS" | "TOP SECRET" => Some(Classification::TopSecret),
        "S" | "SECRET" => Some(Classification::Secret),
        "C" | "CONFIDENTIAL" => Some(Classification::Confidential),
        "R" | "RESTRICTED" => Some(Classification::Restricted),
        "U" | "UNCLASSIFIED" => Some(Classification::Unclassified),
        _ => None,
    }
}

/// Returns `true` if `s` looks like a syntactically and calendrically valid
/// inline declassification date.
///
/// CAPCO allows `YYYYMMDD` (8-digit) or `YYYY` (4-digit, meaning declassify
/// at the start of that calendar year). Both forms are valid in a CAB but
/// are a violation (E005) if they appear directly in a banner or portion
/// marking string.
///
/// Only strings that round-trip through [`IsmDate::from_str`] successfully
/// are accepted. This rejects impossible dates like `20301340` (month 13 /
/// day 40) that look like dates but would silently set `declassify_on` to
/// `None` and prevent E005 from firing.
fn is_declass_date(s: &str) -> bool {
    let bytes = s.as_bytes();
    if !matches!(bytes.len(), 4 | 8) || !bytes.iter().all(u8::is_ascii_digit) {
        return false;
    }
    IsmDate::from_str(s).is_ok()
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests;

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod sar_parse_tests;
