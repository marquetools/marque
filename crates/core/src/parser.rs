// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 2/3: token extraction and structural parsing.
//!
//! Takes [`MarkingCandidate`] spans from the scanner and produces
//! [`marque_ism::ParsedAttrs`]. The engine then runs
//! [`marque_ism::from_parsed_unchecked`] (PR 3a transitional path) or
//! `MarkingScheme::canonicalize` (post-PR-3c) to land owned
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
    NatoClassification, NonIcDissem, SarCompartment, SarIndicator, SarMarking, SarProgram,
    SciCompartment, SciControl, SciControlBare, SciControlSystem, SciMarking, TokenKind, TokenSpan,
};
use marque_ism::date::IsmDate;
use marque_ism::is_bare_cve_value;
use marque_ism::parsed::{
    ParsedAea, ParsedAttrs, ParsedClassification, ParsedDeclassifyOn, ParsedDissem,
    ParsedFgiMarker, ParsedNonIcDissem, ParsedRelToEntry, ParsedSarMarking, ParsedSciMarking,
    SourceOrigin,
};
use marque_ism::span::{MarkingCandidate, MarkingType, Span};
use marque_ism::token_set::TokenSet;
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::str::FromStr;

/// Parse result for a single candidate.
///
/// Carries a borrow into the original source bytes via `attrs` (each
/// `Parsed*<'src>` wrapper retains its source slice). Short-lived: the
/// engine immediately canonicalizes to `CanonicalAttrs` via
/// `marque_ism::from_parsed_unchecked` (PR 3a transitional path) or via
/// `MarkingScheme::canonicalize` (post-PR-3c).
#[derive(Debug)]
pub struct ParsedMarking<'src> {
    pub attrs: ParsedAttrs<'src>,
    pub source_span: Span,
    pub kind: MarkingType,
}

/// Phase 2+3 parser. Stateless; call [`Parser::parse`] per candidate.
pub struct Parser<'t> {
    tokens: &'t dyn TokenSet,
}

impl<'t> Parser<'t> {
    pub fn new(tokens: &'t dyn TokenSet) -> Self {
        Self { tokens }
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
        // canonicalizer's round-trip property (FR-019) sees the bytes the
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
                        // PR 3c may switch to an offset-tracking iterator
                        // if any consumer needs sub-line provenance; for
                        // PR 3a the byte position is recoverable from
                        // `bytes` itself when needed.
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
                Box::new([]),
                Box::new([]),
                Box::new([]),
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

    /// Parse a marking string (without outer parentheses) into [`ParsedAttrs`].
    /// Handles both portion form (abbreviated) and banner form (full words).
    ///
    /// `s_offset` is the absolute byte offset of `s` within the original
    /// source buffer. Phase 3 uses it to record per-token absolute spans on
    /// `ParsedAttrs::token_spans` so rules can point at byte-precise
    /// diagnostic locations.
    fn parse_marking_string<'src>(
        &self,
        s: &'src str,
        context: MarkingType,
        s_offset: usize,
        origin: SourceOrigin,
    ) -> Result<ParsedAttrs<'src>, CoreError> {
        if s.is_empty() {
            return Err(CoreError::MalformedMarking(s.to_owned()));
        }

        let mut classification: Option<ParsedClassification<'src>> = None;
        let mut fgi_marker: Option<ParsedFgiMarker<'src>> = None;
        let mut declassify_on: Option<ParsedDeclassifyOn<'src>> = None;
        let mut declass_exemption: Option<DeclassExemption> = None;
        let mut sar_markings: Option<ParsedSarMarking<'src>> = None;

        // Walk separator (`//`) positions inside `s`. Each block is the
        // substring between consecutive separators (or string ends). Track
        // both the block content and its inner offset so we can compute
        // per-token absolute spans.
        let separators: Vec<usize> = s.match_indices("//").map(|(i, _)| i).collect();
        let mut block_ranges: Vec<(usize, usize)> = Vec::with_capacity(separators.len() + 1);
        let mut prev_end = 0usize;
        for &sep_start in &separators {
            block_ranges.push((prev_end, sep_start));
            prev_end = sep_start + 2; // skip the `//`
        }
        block_ranges.push((prev_end, s.len()));

        let mut token_spans: Vec<TokenSpan> = Vec::new();

        let mut sci: Vec<SciControl> = Vec::new();
        let mut sci_markings: Vec<ParsedSciMarking<'src>> = Vec::new();
        // SAR: P2 wires the hand-written subparser. Only the FIRST SAR block
        // encountered populates `sar_markings`; any subsequent SAR block
        // is emitted as `TokenKind::Unknown` so rule E030 (indicator-repeat)
        // can flag the duplicate.
        let mut sar_captured = false;
        let mut aea: Vec<ParsedAea<'src>> = Vec::new();
        let mut dissem: Vec<ParsedDissem<'src>> = Vec::new();
        let mut non_ic: Vec<ParsedNonIcDissem<'src>> = Vec::new();
        let mut rel_to: Vec<ParsedRelToEntry<'src>> = Vec::new();

        // When the marking starts with `//`, block 0 is empty and the
        // classification is non-US (FGI, NATO, or JOINT). Block 1 carries
        // the foreign classification.
        let is_non_us = s.starts_with("//");

        for (idx, &(rel_start, rel_end)) in block_ranges.iter().enumerate() {
            let raw = &s[rel_start..rel_end];
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let trim_lead = raw.len() - raw.trim_start().len();
            let abs_start = s_offset + rel_start + trim_lead;
            let abs_end = abs_start + trimmed.len();
            let span = Span::new(abs_start, abs_end);

            // ---------------------------------------------------------------
            // Block 0: US classification (or empty for non-US markings)
            // ---------------------------------------------------------------
            if idx == 0 && !is_non_us {
                if let Some(c) = parse_classification(trimmed) {
                    classification = Some(ParsedClassification::new(
                        MarkingClassification::Us(c),
                        trimmed,
                        span,
                    ));
                }
                token_spans.push(TokenSpan {
                    kind: TokenKind::Classification,
                    span,
                    text: trimmed.into(),
                });
                continue;
            }

            // ---------------------------------------------------------------
            // Block 1 when non-US: foreign classification
            // ---------------------------------------------------------------
            if idx == 1 && is_non_us {
                let parsed_cls = if let Some(nato) = parse_nato_classification(trimmed) {
                    Some(MarkingClassification::Nato(nato))
                } else if let Some(joint) = parse_joint_classification(trimmed) {
                    Some(MarkingClassification::Joint(joint))
                } else {
                    parse_fgi_classification(trimmed).map(MarkingClassification::Fgi)
                };
                if let Some(value) = parsed_cls {
                    classification = Some(ParsedClassification::new(value, trimmed, span));
                } else {
                    // Unrecognized non-US classification block.
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span,
                        text: trimmed.into(),
                    });
                    continue;
                }
                token_spans.push(TokenSpan {
                    kind: TokenKind::Classification,
                    span,
                    text: trimmed.into(),
                });
                continue;
            }

            // ---------------------------------------------------------------
            // Remaining blocks: controls, markers, and fallbacks
            // ---------------------------------------------------------------

            // SAR category block (must precede the other branches because a
            // SAR block such as `SAR-BP-J12/CD` contains `/` and would be
            // misrouted to the multi-slash fallback). §H.5 / §A.6.
            if trimmed.starts_with("SAR-") || trimmed.starts_with("SPECIAL ACCESS REQUIRED-") {
                if sar_captured {
                    // Second (or later) SAR block in this marking. Leave the
                    // whole block as Unknown so E030 (sar-indicator-repeat)
                    // can surface it in P3.
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span,
                        text: trimmed.into(),
                    });
                    continue;
                }
                if let Some((marking, sar_spans)) = parse_sar_category(trimmed, abs_start) {
                    sar_markings = Some(ParsedSarMarking::new(marking, trimmed, span));
                    token_spans.extend(sar_spans);
                    sar_captured = true;
                    continue;
                }
                // Grammar rejection (e.g., `SAR-` with nothing after): fall
                // through to the normal Unknown handling below.
                token_spans.push(TokenSpan {
                    kind: TokenKind::Unknown,
                    span,
                    text: trimmed.into(),
                });
                continue;
            }

            if trimmed.starts_with("REL TO") || trimmed.starts_with("REL ") {
                // Record the full block text before the individual trigraph tokens
                // so token_spans maintains a logical ordering (block → constituents).
                token_spans.push(TokenSpan {
                    kind: TokenKind::RelToBlock,
                    span,
                    text: trimmed.into(),
                });
                let parsed =
                    parse_rel_to_with_spans(trimmed, abs_start, self.tokens, &mut token_spans);
                rel_to.extend(parsed.countries);
                dissem.extend(parsed.trailing_dissem);
                non_ic.extend(parsed.trailing_non_ic);
            } else if (trimmed.contains('-')
                || trimmed.contains('/')
                || is_bare_cve_value(trimmed)
                // Standalone custom SCI control (e.g., `99` in §A.6 p16).
                // Require at least one digit so pure-alpha tokens (which
                // are far more likely to be typos, other-category markers
                // like `FGI`, or scanner-test garbage like `XYZZY`) keep
                // falling through to Unknown. Declass dates (4/8 digit)
                // and known non-SCI markers are also excluded.
                || (is_valid_custom_control(trimmed)
                    && trimmed.bytes().any(|b| b.is_ascii_digit())
                    && !is_known_non_sci_token(trimmed)
                    && !is_declass_date(trimmed)))
                && let Some(markings) = parse_sci_block(trimmed, abs_start, &mut token_spans)
            {
                // Structural SCI path (spec 003-sci-compartments §R2). Runs
                // before the exact-match path so compound/sub-compartment
                // forms like `SI-G ABCD` and `123/SI-G ABCD DEFG-MMM AACD`
                // are recognized. Projects canonical enum values into
                // `sci_controls` for back-compat with rules that read the
                // flat enum view (E010, E011).
                for marking in &markings {
                    if let Some(ctrl) = marking.canonical_enum {
                        sci.push(ctrl);
                    }
                }
                // Wrap each structural SCI marking with the source slice
                // for the full SCI sub-block. **Known PR 3a limitation
                // (Copilot review feedback):** when the block holds
                // multiple `/`-separated systems (e.g. `SI-G/TK-BLFH`),
                // every wrapper here gets the *same* `trimmed`+`span`
                // covering the whole block, not the per-system slice.
                // The structural subparser already records granular
                // per-system spans inside `token_spans` (kinds
                // `SciSystem` / `SciCompartment` / `SciSubCompartment`),
                // so per-marking provenance is recoverable today; the
                // wrapper-level redundancy is benign for PR 3a's
                // byte-identical-behavior gate. PR 3c picks up
                // per-marking byte slicing as part of FR-019 round-trip
                // work — `parse_sci_block` will return the per-chunk
                // slices alongside the parsed markings, and this site
                // pairs them 1:1.
                for marking in markings {
                    sci_markings.push(ParsedSciMarking::new(marking, trimmed, span));
                }
            } else if let Some(ctrl) = SciControl::parse(trimmed) {
                sci.push(ctrl);
                token_spans.push(TokenSpan {
                    kind: TokenKind::SciControl,
                    span,
                    text: trimmed.into(),
                });
            } else if trimmed.starts_with("FGI")
                && matches!(
                    classification.as_ref().map(|c| &c.value),
                    Some(MarkingClassification::Us(_))
                )
            {
                // FGI marker in a US-classified marking (e.g., SECRET//FGI DEU//NF).
                if let Some(marker) = parse_fgi_marker(trimmed) {
                    fgi_marker = Some(ParsedFgiMarker::new(marker, trimmed, span));
                    token_spans.push(TokenSpan {
                        kind: TokenKind::FgiMarker,
                        span,
                        text: trimmed.into(),
                    });
                }
            } else if let Some(ctrl) =
                DissemControl::parse(trimmed).or_else(|| parse_dissem_full_form(trimmed))
            {
                dissem.push(ParsedDissem::new(ctrl, trimmed, span));
                token_spans.push(TokenSpan {
                    kind: TokenKind::DissemControl,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(nic) = parse_non_ic_full_form(trimmed) {
                non_ic.push(ParsedNonIcDissem::new(nic, trimmed, span));
                token_spans.push(TokenSpan {
                    kind: TokenKind::NonIcDissem,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(aea_marking) = AeaMarking::parse(trimmed) {
                aea.push(ParsedAea::new(aea_marking, trimmed, span));
                token_spans.push(TokenSpan {
                    kind: TokenKind::AeaMarking,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(exemption) = DeclassExemption::parse(trimmed) {
                declass_exemption = Some(exemption);
                token_spans.push(TokenSpan {
                    kind: TokenKind::DeclassExemption,
                    span,
                    text: trimmed.into(),
                });
            } else if is_declass_date(trimmed) {
                if let Ok(date) = IsmDate::from_str(trimmed) {
                    declassify_on = Some(ParsedDeclassifyOn::new(date, trimmed, span));
                }
                token_spans.push(TokenSpan {
                    kind: TokenKind::DeclassDate,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(foreign) = try_parse_foreign_classification(trimmed) {
                // Conflict: a foreign classification in a marking that already
                // has a US classification. US wins at the greater of the two.
                let prior_us = match classification.as_ref().map(|c| &c.value) {
                    Some(MarkingClassification::Us(level)) => Some(*level),
                    _ => None,
                };
                if let Some(us_level) = prior_us {
                    let foreign_equiv = match &foreign {
                        ForeignClassification::Nato(n) => n.us_equivalent(),
                        ForeignClassification::Fgi(f) => f.level,
                        ForeignClassification::Joint(j) => j.level,
                    };
                    let max_level = us_level.max(foreign_equiv);
                    // Conflict provenance choice: bytes/span point at the
                    // FOREIGN block (`trimmed`/`span`), not the prior US
                    // block. The conflict is detected here, when parsing
                    // the second classification — pointing at the foreign
                    // block makes a future diagnostic ("dual classification
                    // — pick one") land on the offending position. The US
                    // block's own location is still recoverable via
                    // `token_spans` (its `TokenSpan` was pushed on the
                    // earlier iteration). A consumer that wants the full
                    // conflict region can compute it from
                    // `min(us.span.start, foreign.span.start)` to
                    // `max(us.span.end, foreign.span.end)`. PR 3c may
                    // promote this to a Conflict-specific span shape if a
                    // round-trip property test (FR-019) requires it.
                    classification = Some(ParsedClassification::new(
                        MarkingClassification::Conflict {
                            us: max_level,
                            foreign: Box::new(foreign),
                        },
                        trimmed,
                        span,
                    ));
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Classification,
                        span,
                        text: trimmed.into(),
                    });
                } else {
                    // No prior US classification — just Unknown.
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span,
                        text: trimmed.into(),
                    });
                }
            } else if trimmed.contains('/') && !trimmed.starts_with("REL") {
                // Multi-token block per CAPCO §D.1: multiple entries within a
                // **single category** are separated by `/` (e.g., "SI/TK", "NF/RD").
                // First, speculatively parse all sub-tokens. If all recognized sub-tokens
                // belong to the same category, commit them. If categories are mixed
                // (e.g., "SI/NF" — SCI + dissem in one block), the `/` is a stray
                // separator that should have been `//`; emit the whole block as Unknown
                // so E004 can detect and fix the missing `//`.

                #[derive(Clone, Copy, PartialEq, Eq)]
                enum SubKind {
                    Sci,
                    Dissem,
                    NonIc,
                    Aea,
                    Unknown,
                }

                struct SubResult<'a> {
                    kind: SubKind,
                    tok: &'a str,
                    span: Span,
                    // Parsed values — stored here before committing.
                    sci: Option<SciControl>,
                    dissem: Option<DissemControl>,
                    nic: Option<NonIcDissem>,
                    aea: Option<AeaMarking>,
                }

                let mut results: Vec<SubResult<'_>> = Vec::new();
                for (sub_off, sub_tok) in split_slash_with_offsets(trimmed) {
                    let sub_abs_start = abs_start + sub_off;
                    let sub_span = Span::new(sub_abs_start, sub_abs_start + sub_tok.len());
                    if let Some(ctrl) = SciControl::parse(sub_tok) {
                        results.push(SubResult {
                            kind: SubKind::Sci,
                            tok: sub_tok,
                            span: sub_span,
                            sci: Some(ctrl),
                            dissem: None,
                            nic: None,
                            aea: None,
                        });
                    } else if let Some(ctrl) =
                        DissemControl::parse(sub_tok).or_else(|| parse_dissem_full_form(sub_tok))
                    {
                        results.push(SubResult {
                            kind: SubKind::Dissem,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: Some(ctrl),
                            nic: None,
                            aea: None,
                        });
                    } else if let Some(nic) = parse_non_ic_full_form(sub_tok) {
                        results.push(SubResult {
                            kind: SubKind::NonIc,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: None,
                            nic: Some(nic),
                            aea: None,
                        });
                    } else if let Some(aea_marking) = AeaMarking::parse(sub_tok) {
                        results.push(SubResult {
                            kind: SubKind::Aea,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: None,
                            nic: None,
                            aea: Some(aea_marking),
                        });
                    } else {
                        results.push(SubResult {
                            kind: SubKind::Unknown,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: None,
                            nic: None,
                            aea: None,
                        });
                    }
                }

                // Check category consistency: all parsed (non-Unknown) sub-tokens
                // must share the same category for `/` to be a valid intra-block
                // separator. Mixed categories (e.g., SCI + dissem) mean the `/`
                // is a stray single-slash separator that should have been `//`.
                let first_parsed_kind = results
                    .iter()
                    .find(|r| r.kind != SubKind::Unknown)
                    .map(|r| r.kind);
                let all_same_category = first_parsed_kind.is_some_and(|first| {
                    results
                        .iter()
                        .filter(|r| r.kind != SubKind::Unknown)
                        .all(|r| r.kind == first)
                });

                if first_parsed_kind.is_some() && !all_same_category {
                    // Mixed categories: the `/` is a stray separator.
                    // Emit the whole block as Unknown so E004 can detect it.
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span,
                        text: trimmed.into(),
                    });
                } else {
                    // Same category (or all unknown): commit sub-token results.
                    for r in results {
                        match r.kind {
                            SubKind::Sci => {
                                sci.push(r.sci.unwrap());
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::SciControl,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                            SubKind::Dissem => {
                                dissem.push(ParsedDissem::new(r.dissem.unwrap(), r.tok, r.span));
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::DissemControl,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                            SubKind::NonIc => {
                                non_ic.push(ParsedNonIcDissem::new(r.nic.unwrap(), r.tok, r.span));
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::NonIcDissem,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                            SubKind::Aea => {
                                aea.push(ParsedAea::new(r.aea.unwrap(), r.tok, r.span));
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::AeaMarking,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                            SubKind::Unknown => {
                                // Unrecognized sub-token within a same-category block.
                                // E008 fires one diagnostic per Unknown span.
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::Unknown,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                        }
                    }
                }
            } else {
                token_spans.push(TokenSpan {
                    kind: TokenKind::Unknown,
                    span,
                    text: trimmed.into(),
                });
            }
        }

        // Record separator spans (Phase 3 needs them for E004). Push them
        // here alongside block tokens, then sort by start offset so the
        // final slice is in document (source) order.
        for &sep_start in &separators {
            token_spans.push(TokenSpan {
                kind: TokenKind::Separator,
                span: Span::new(s_offset + sep_start, s_offset + sep_start + 2),
                text: "//".into(),
            });
        }
        token_spans.sort_unstable_by_key(|ts| ts.span.start);

        let _ = context; // used for future context-aware validation

        Ok(ParsedAttrs::new(
            classification,
            sci_markings.into_boxed_slice(),
            sci.into_boxed_slice(),
            sar_markings,
            aea.into_boxed_slice(),
            fgi_marker,
            dissem.into_boxed_slice(),
            non_ic.into_boxed_slice(),
            rel_to.into_boxed_slice(),
            declassify_on,
            None,
            None,
            declass_exemption,
            token_spans.into_boxed_slice(),
            origin,
        ))
    }
}

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

/// Structural subparser for the SCI category block per CAPCO-2016 §A.6.
///
/// Grammar (spec 003-sci-compartments §R2):
///
/// ```text
/// SCI_BLOCK      := SCI_SYSTEM ("/" SCI_SYSTEM)*
/// SCI_SYSTEM     := CONTROL (-COMPARTMENT)*
/// CONTROL        := BARE_CONTROL | CUSTOM_CONTROL
/// BARE_CONTROL   := any bare CVE value (via is_bare_cve_value)
/// CUSTOM_CONTROL := [A-Z0-9]{2,5} (not matching a BARE_CONTROL)
/// COMPARTMENT    := COMP_ID (SPACE SUB_COMP)*
/// COMP_ID        := [A-Z0-9]+
/// SUB_COMP       := [A-Z0-9]+
/// ```
///
/// Returns `Some(markings)` on successful structural parse, `None` on any
/// grammar violation (dangling hyphens, leading hyphens, lowercase,
/// empty compartments, invalid custom shape). On `None`, the caller falls
/// back to the existing `SciControl::parse` exact-match path.
///
/// `canonical_enum` is populated via `format!("{ctrl}-{first_comp}").parse::<SciControl>()`
/// ONLY when the matching compartment has no sub-compartments — sub-comps
/// imply the compound is a structural anchor, not an atomic CVE value.
///
/// On success, emits TokenSpan entries (SciSystem / SciCompartment /
/// SciSubCompartment) at byte-precise offsets relative to `base`.
fn parse_sci_block(
    text: &str,
    base: usize,
    tokens: &mut Vec<TokenSpan>,
) -> Option<Vec<SciMarking>> {
    if text.is_empty() {
        return None;
    }

    // Buffer tokens into a local vec so we can discard them if any system
    // fails to parse (all-or-nothing success semantics per spec).
    let mut local_tokens: Vec<TokenSpan> = Vec::new();
    let mut markings: Vec<SciMarking> = Vec::new();

    // Split on `/` into per-system chunks, tracking byte offsets so each
    // TokenSpan's `span` is accurate relative to the original source.
    let mut chunk_start = 0usize;
    let chunks: Vec<(usize, &str)> = {
        let mut v = Vec::new();
        for (i, ch) in text.char_indices() {
            if ch == '/' {
                v.push((chunk_start, &text[chunk_start..i]));
                chunk_start = i + 1;
            }
        }
        v.push((chunk_start, &text[chunk_start..]));
        v
    };

    for (chunk_off, chunk) in chunks {
        // No trim — grammar is strict; whitespace inside a chunk is
        // meaningful only between sub-compartments (see below).
        if chunk.is_empty() {
            return None;
        }
        // Leading hyphen rejects immediately (e.g., `-SI`).
        if chunk.starts_with('-') {
            return None;
        }

        // Split chunk on first `-` into (control, rest). If no `-`, the
        // whole chunk is the control with no compartments.
        let (ctrl_str, rest_opt) = match chunk.find('-') {
            Some(i) => (&chunk[..i], Some(&chunk[i + 1..])),
            None => (chunk, None),
        };

        if ctrl_str.is_empty() {
            return None;
        }

        // Recognize control: bare CVE first, then custom [A-Z0-9]{2,5}.
        // A custom control must not collide with any other known category
        // (Dissem / NonIcDissem / Sar / Aea / DeclassExemption) — otherwise
        // a block like `SI/NF` would be mis-claimed as SCI instead of
        // flagged as a stray `/` by E004.
        let system: SciControlSystem = if let Some(bare) = SciControlBare::parse(ctrl_str) {
            SciControlSystem::Published(bare)
        } else if is_valid_custom_control(ctrl_str) && !is_known_non_sci_token(ctrl_str) {
            SciControlSystem::Custom(ctrl_str.into())
        } else {
            return None;
        };

        // Emit a block-level SciControl span covering the full system
        // chunk (control + compartments + sub-compartments), mirroring the
        // existing exact-match path so rule consumers (E010, E011, and
        // audit tooling that reads TokenKind::SciControl) continue to see
        // one span per marking. The granular SciSystem/SciCompartment/
        // SciSubCompartment spans below provide finer-grained structure
        // for spec 003 rules (E032–E035).
        let chunk_abs = base + chunk_off;
        local_tokens.push(TokenSpan {
            kind: TokenKind::SciControl,
            span: Span::new(chunk_abs, chunk_abs + chunk.len()),
            text: chunk.into(),
        });
        // Emit SciSystem token for the control identifier itself.
        let ctrl_abs = base + chunk_off;
        local_tokens.push(TokenSpan {
            kind: TokenKind::SciSystem,
            span: Span::new(ctrl_abs, ctrl_abs + ctrl_str.len()),
            text: ctrl_str.into(),
        });

        // Parse compartments. `rest` is the substring after the first `-`.
        // Each additional compartment is preceded by another `-`, and
        // sub-compartments within a compartment are space-separated.
        let mut compartments: Vec<SciCompartment> = Vec::new();
        if let Some(rest) = rest_opt {
            // Split `rest` on `-` into compartment segments. Strict grammar:
            // empty segment (trailing or consecutive hyphen) → reject.
            let rest_abs_base = base + chunk_off + ctrl_str.len() + 1; // +1 skips the `-`
            let mut seg_start = 0usize;
            let mut seg_offs: Vec<(usize, &str)> = Vec::new();
            for (i, ch) in rest.char_indices() {
                if ch == '-' {
                    seg_offs.push((seg_start, &rest[seg_start..i]));
                    seg_start = i + 1;
                }
            }
            seg_offs.push((seg_start, &rest[seg_start..]));

            for (seg_off, seg) in seg_offs {
                if seg.is_empty() {
                    return None; // dangling `-` or consecutive `--`
                }
                // Each compartment segment = COMP_ID (SPACE SUB_COMP)*
                // Split on space.
                let mut parts = seg.split(' ');
                let comp_id = parts.next().unwrap(); // at least one part
                if comp_id.is_empty() || !is_alnum_upper(comp_id) {
                    return None;
                }

                let comp_abs = rest_abs_base + seg_off;
                local_tokens.push(TokenSpan {
                    kind: TokenKind::SciCompartment,
                    span: Span::new(comp_abs, comp_abs + comp_id.len()),
                    text: comp_id.into(),
                });

                let mut subs: Vec<SmolStr> = Vec::new();
                // Track cursor within segment for sub-compartment offsets.
                let mut sub_cursor = comp_id.len() + 1; // +1 skips the space
                for sub in parts {
                    if sub.is_empty() || !is_alnum_upper(sub) {
                        return None;
                    }
                    let sub_abs = rest_abs_base + seg_off + sub_cursor;
                    local_tokens.push(TokenSpan {
                        kind: TokenKind::SciSubCompartment,
                        span: Span::new(sub_abs, sub_abs + sub.len()),
                        text: sub.into(),
                    });
                    subs.push(sub.into());
                    sub_cursor += sub.len() + 1;
                }

                compartments.push(SciCompartment::new(comp_id, subs.into_boxed_slice()));
            }
        }

        // canonical_enum population (per data-model §canonical_enum):
        // - No compartments → the bare control itself may be a CVE value
        //   (e.g., `SI`, `TK`, `HCS`). Preserves pre-spec behavior.
        // - One or more compartments → try `{ctrl}-{first_comp}` ONLY when
        //   the first compartment has no sub-compartments. Sub-comps mean
        //   the compound is a structural anchor, not an atomic CVE atom.
        let canonical_enum = if compartments.is_empty() {
            SciControl::parse(ctrl_str)
        } else {
            compartments
                .first()
                .filter(|c| c.sub_compartments.is_empty())
                .and_then(|c| {
                    let composite = format!("{}-{}", ctrl_str, c.identifier);
                    SciControl::parse(&composite)
                })
        };

        markings.push(SciMarking::new(
            system,
            compartments.into_boxed_slice(),
            canonical_enum,
        ));
    }

    tokens.extend(local_tokens);
    Some(markings)
}

/// Custom control shape check: `[A-Z0-9]{2,5}` per spec §R1. Must not match
/// a bare CVE value (caller dispatches to Published first, so this check is
/// strictly the shape constraint).
fn is_valid_custom_control(s: &str) -> bool {
    let len = s.len();
    (2..=5).contains(&len) && is_alnum_upper(s)
}

/// Returns true if `s` is non-empty and every byte is ASCII uppercase or digit.
fn is_alnum_upper(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// Guard for the SCI structural subparser: returns true if `s` is a known
/// non-SCI token (dissem, non-IC dissem, AEA marking, or declass exemption).
/// Prevents `parse_sci_block` from claiming mixed-category slash blocks
/// like `SI/NF` that should surface as stray-slash errors. SAR is
/// structural (not CVE-backed) and handled by `parse_sar_category`.
fn is_known_non_sci_token(s: &str) -> bool {
    DissemControl::parse(s).is_some()
        || parse_dissem_full_form(s).is_some()
        || parse_non_ic_full_form(s).is_some()
        || AeaMarking::parse(s).is_some()
        || DeclassExemption::parse(s).is_some()
}

/// Parse a NATO classification string in either banner form (`"NATO SECRET"`,
/// `"COSMIC TOP SECRET"`, etc.) or portion form (`"NS"`, `"CTS"`, etc.).
///
/// Includes SAP variants (ATOMAL, BOHEMIA, BALK). Longer patterns are checked
/// first to avoid prefix ambiguity (e.g., `"COSMIC TOP SECRET ATOMAL"` before
/// `"COSMIC TOP SECRET"`).
fn parse_nato_classification(s: &str) -> Option<NatoClassification> {
    // Check longer patterns first to avoid prefix matches.
    match s {
        // Banner forms (full words) — longer patterns first
        "COSMIC TOP SECRET ATOMAL" => Some(NatoClassification::CosmicTopSecretAtomal),
        "COSMIC TOP SECRET-BOHEMIA" => Some(NatoClassification::CosmicTopSecretBohemia),
        "COSMIC TOP SECRET-BALK" => Some(NatoClassification::CosmicTopSecretBalk),
        "COSMIC TOP SECRET" => Some(NatoClassification::CosmicTopSecret),
        "NATO SECRET ATOMAL" => Some(NatoClassification::NatoSecretAtomal),
        "NATO SECRET" => Some(NatoClassification::NatoSecret),
        "NATO CONFIDENTIAL ATOMAL" => Some(NatoClassification::NatoConfidentialAtomal),
        "NATO CONFIDENTIAL" => Some(NatoClassification::NatoConfidential),
        "NATO RESTRICTED" => Some(NatoClassification::NatoRestricted),
        "NATO UNCLASSIFIED" => Some(NatoClassification::NatoUnclassified),
        // Portion forms — primary (CAPCO Register)
        "CTSA" | "CTS-A" => Some(NatoClassification::CosmicTopSecretAtomal),
        "CTS-B" => Some(NatoClassification::CosmicTopSecretBohemia),
        "CTS-BALK" => Some(NatoClassification::CosmicTopSecretBalk),
        "CTS" => Some(NatoClassification::CosmicTopSecret),
        "NSAT" | "NS-A" => Some(NatoClassification::NatoSecretAtomal),
        "NS" => Some(NatoClassification::NatoSecret),
        "NCA" | "NC-A" => Some(NatoClassification::NatoConfidentialAtomal),
        "NC" => Some(NatoClassification::NatoConfidential),
        "NR" => Some(NatoClassification::NatoRestricted),
        "NU" => Some(NatoClassification::NatoUnclassified),
        _ => None,
    }
}

/// Parse a JOINT classification block: `"JOINT S USA GBR"` or `"JOINT SECRET USA GBR"`.
///
/// Format: `JOINT` + classification level + space-delimited country trigraphs.
/// Countries are space-delimited (NOT comma-delimited like REL TO).
fn parse_joint_classification(s: &str) -> Option<JointClassification> {
    let rest = s.strip_prefix("JOINT ")?;
    let mut tokens = rest.split_whitespace();

    // First token(s) after JOINT are the classification level.
    // Handle two-word levels like "TOP SECRET".
    let first = tokens.next()?;
    let (level, remaining_start) = if first == "TOP" {
        // Check if next token is "SECRET" to form "TOP SECRET"
        let mut peek_tokens = rest.split_whitespace();
        peek_tokens.next(); // skip "TOP"
        if peek_tokens.next() == Some("SECRET") {
            let level = parse_classification("TOP SECRET")?;
            // Skip past "TOP SECRET" — countries start after
            let after_ts = rest.find("SECRET").map(|i| i + "SECRET".len())?;
            (level, after_ts)
        } else {
            return None; // "TOP" alone is not a valid level
        }
    } else {
        let level = parse_classification(first)?;
        let after_level = rest.find(first).map(|i| i + first.len())?;
        (level, after_level)
    };

    // Remaining tokens are space-delimited country trigraphs.
    //
    // NOTE: JOINT classifications today drop non-3-byte tokens
    // silently (tetragraphs like NATO never appear in real JOINT
    // markings, but the parallel of issue #183's REL TO silent-drop
    // is tracked as deferred scope for PR-B / a future issue).
    let country_str = rest[remaining_start..].trim();
    let mut countries = Vec::new();
    for token in country_str.split_whitespace() {
        if token.len() == 3 {
            if let Some(t) = CountryCode::try_new(token.as_bytes()) {
                countries.push(t);
            }
        }
    }

    if countries.is_empty() {
        return None; // JOINT must have at least one country
    }

    Some(JointClassification {
        level,
        countries: countries.into(),
    })
}

/// Parse an FGI classification block: `"GBR S"`, `"DEU TS"`, `"GBR DEU S"`,
/// or `"FGI S"` (FGI as placeholder for unknown country).
///
/// Format: one or more country trigraphs (or "FGI") + classification level.
/// Countries are space-delimited. The last token is the classification level.
///
/// Returns `None` if no classification level is found (e.g., bare `"FGI"` with
/// no level — that's an error, not a valid FGI classification).
fn parse_fgi_classification(s: &str) -> Option<FgiClassification> {
    let tokens: Vec<&str> = s.split_whitespace().collect();
    if tokens.len() < 2 {
        return None; // Need at least country + level
    }

    // Last token is the classification level. Handle "TOP SECRET" as two tokens.
    let (level, country_end) = if tokens.len() >= 3
        && tokens[tokens.len() - 2] == "TOP"
        && tokens[tokens.len() - 1] == "SECRET"
    {
        (parse_classification("TOP SECRET")?, tokens.len() - 2)
    } else {
        (
            parse_classification(tokens[tokens.len() - 1])?,
            tokens.len() - 1,
        )
    };

    // Preceding tokens are country trigraphs (or "FGI" placeholder).
    let mut countries = Vec::new();
    for &token in &tokens[..country_end] {
        if token == "FGI" {
            // FGI as placeholder for unknown country — countries stays empty
            continue;
        }
        if token.len() == 3 {
            let t = CountryCode::try_new(token.as_bytes())?;
            countries.push(t);
        } else {
            return None; // Not a trigraph or "FGI"
        }
    }

    Some(FgiClassification {
        countries: countries.into(),
        level,
    })
}

/// Parse an FGI marker block in a US-classified marking.
///
/// This is the FGI block between SAR and dissem controls in a
/// US-classified marking (e.g., `SECRET//FGI DEU//NOFORN`). Not to
/// be confused with [`parse_fgi_classification`] which parses a
/// non-US classification.
///
/// # Three return cases (FR-015 / FR-016 closure, GH #280)
///
/// Per CAPCO-2016 §H.7 p123 the FGI marker has exactly two lawful
/// banner forms: bare `FGI` (source-concealed) and `FGI [LIST]`
/// (source-acknowledged). Anything else is a parse failure, not a
/// degraded lawful form. This function enforces that as three
/// disjoint return cases:
///
/// | Input shape | Return |
/// |-------------|--------|
/// | `"FGI"` exactly (no whitespace, no suffix) | `Some(SourceConcealed)` |
/// | `"FGI " + tokens` where every token is a 2-, 3-, or 4-letter ASCII upper code (registered exception / Annex B trigraph / Annex A tetragraph) | `Some(Acknowledged { countries })` |
/// | Anything else (malformed prefix, any token fails the country-token shape gate, OR empty list after `"FGI "`) | `None` |
///
/// The third row is the FR-016 closure: a post-failure shape MUST
/// be `None`, never a degraded `Some(SourceConcealed)`. The
/// transitional T094 fallback (`...unwrap_or(SourceConcealed)`) was
/// removed in T088+T093 so a parse failure surfaces honestly to the
/// rule layer instead of being silently re-cast as lawful
/// concealment. A diagnostic for malformed FGI input is the rule
/// layer's job; the parser's job is to refuse to mint a misleading
/// AST.
///
/// # Country-token shape gate
///
/// Token admission goes through
/// [`marque_ism::CountryCode::admits_country_token`] — the canonical
/// FGI/REL TO list-token shape predicate (3 ASCII upper letters for
/// an Annex B trigraph **or** 4 ASCII upper letters for an Annex A
/// tetragraph). This is the same predicate that
/// `Vocabulary<CapcoScheme>::shape_admits(CAT_FGI_MARKER, _)` calls,
/// so the parser admits exactly what the vocabulary surface
/// documents. Routing both surfaces through one symbol satisfies
/// FR-015 (admission via documented vocabulary surface) and CHK030
/// (no inline `is_ascii_alphanumeric` byte-class checks).
///
/// CAPCO-2016 §H.7 p123 spells out the shape: "Multiple FGI
/// trigraph country codes or tetragraph codes must be separated by
/// a single space ... example may appear as: `SECRET//FGI GBR JPN
/// NATO//REL TO USA, GBR, JPN, NATO`." The order invariant
/// (trigraphs alphabetic, then tetragraphs alphabetic) is rule-layer,
/// not admission — real-world inputs arrive in any order and a
/// dedicated rule normalizes them. Registry membership (Annex A
/// for tetragraphs, Annex B for trigraphs) is also rule-layer.
///
/// `CountryCode::try_new` is a strictly weaker predicate at this
/// site (it admits 2-15 byte values including digits and underscore
/// for `AX2` / `AX3` / `AUSTRALIA_GROUP`), so going through
/// `admits_country_token` first guarantees the subsequent `try_new`
/// succeeds; the construct call is therefore infallible. That
/// ordering is what lets the parser remain zero-allocation on the
/// failure path (Constitution Principle II): `?` returns `None`
/// immediately on any token-shape failure, no temporary allocation
/// needed.
///
/// # Edge cases
///
/// - `parse_fgi_marker("")` → `None` (empty input has no `FGI` prefix).
/// - `parse_fgi_marker("FGI")` → `Some(SourceConcealed)` — the bare
///   lawful concealed form.
/// - `parse_fgi_marker("FGI ")` (trailing whitespace, no tokens) →
///   `None`. The strict `"FGI "` prefix followed by zero tokens is
///   malformed input. Bare `"FGI"` (no trailing space) is the lawful
///   concealed form; the trailing space disambiguates the two
///   surfaces.
/// - `parse_fgi_marker("FGI deu")` → `None` (lowercase fails the
///   country-token shape gate; admission requires uniform ASCII upper).
/// - `parse_fgi_marker("FGI USA NATO")` →
///   `Some(Acknowledged { countries: [USA, NATO] })`. NATO is a
///   tetragraph; admitted at this site per §H.7 p123. Order
///   normalization (trigraph-then-tetragraph) is a rule-layer
///   concern; the parser preserves source order.
/// - `parse_fgi_marker("FGI USAGB")` → `None` (5-byte token rejected
///   by the shape gate; `AUSTRALIA_GROUP`-class codes are out of
///   scope here per the §H.7 "exception is granted" carve-out).
/// - `parse_fgi_marker("foo FGI USA")` → `None` (no `FGI ` prefix
///   on the input).
///
/// # Authority
///
/// CAPCO-2016 §H.7 p123 (FGI banner forms — concealed vs.
/// acknowledged; trigraph-OR-tetragraph list grammar) + §A.6 p16
/// ("Multiple FGI trigraph country codes or tetragraph codes must
/// be separated by a single space"). The country-token predicate's
/// authority chain is documented at
/// [`marque_ism::CountryCode::admits_country_token`].
fn parse_fgi_marker(s: &str) -> Option<FgiMarker> {
    // Case 1: bare `FGI` is the lawful source-concealed banner form.
    if s == "FGI" {
        return Some(FgiMarker::SourceConcealed);
    }

    // Case 2 / Case 3 dispatch: input must start with `FGI ` (with a
    // single space). `strip_prefix` returning `None` on missing
    // prefix is the Case 3 short-circuit for inputs like `"FGIDEU"`,
    // `"foo FGI USA"`, or anything else that doesn't lead with the
    // canonical separator.
    let rest = s.strip_prefix("FGI ")?;

    // Build the country list directly into the inline-4
    // `SmallVec` shape `FgiMarker::Acknowledged` carries — typical
    // FGI lists are ≤4 codes per CAPCO §H.7, so the common cases
    // (`FGI USA`, `FGI USA GBR`, the §H.7 canonical example
    // `FGI GBR JPN NATO`) stay heap-free. A longer list spills to
    // the heap in `SmallVec` itself once, matching what
    // `FgiMarker::acknowledged` would produce; the previous
    // intermediate `Vec` defeated the inline-storage optimization
    // by forcing one heap allocation on every acknowledged marker
    // before the constructor re-collected.
    let mut countries: SmallVec<[CountryCode; 4]> = SmallVec::new();
    for token in rest.split_whitespace() {
        // FR-015 admission: route every token through the canonical
        // FGI/REL TO list-token shape predicate. Trigraphs (3 ASCII
        // upper) and tetragraphs (4 ASCII upper) admit; anything
        // else (lowercase, digits, 5+-byte codes, junk) is a parse
        // failure that returns `None` — never silently dropped.
        if !CountryCode::admits_country_token(token.as_bytes()) {
            return None;
        }
        // `admits_country_token` (3 or 4 ASCII upper) is strictly
        // stronger than `try_new` (2-15 alphanumeric/underscore), so
        // this construction cannot fail. The `?` is here only as a
        // type-system safeguard; it is unreachable for any input
        // that passed the shape gate above.
        let code = CountryCode::try_new(token.as_bytes())?;
        countries.push(code);
    }

    // Case 3 closure: `"FGI "` followed by zero shape-admitted
    // tokens (e.g., trailing whitespace only, or input like
    // `"FGI \t"`). `FgiMarker::acknowledged` returns `None` on an
    // empty country list, which is exactly the FR-016 contract —
    // propagate it directly. This is the line that retired the
    // transitional `unwrap_or(SourceConcealed)` fallback (#280).
    FgiMarker::acknowledged(countries)
}

/// Attempt to parse a block as a foreign classification (NATO, JOINT, or FGI).
///
/// Used as a fallback in the block loop to detect conflict scenarios
/// (e.g., `SECRET//NATO SECRET//NOFORN`) where a foreign classification
/// appears alongside a US classification.
fn try_parse_foreign_classification(s: &str) -> Option<ForeignClassification> {
    if let Some(nato) = parse_nato_classification(s) {
        Some(ForeignClassification::Nato(nato))
    } else if let Some(joint) = parse_joint_classification(s) {
        Some(ForeignClassification::Joint(joint))
    } else {
        parse_fgi_classification(s).map(ForeignClassification::Fgi)
    }
}

/// Map a banner-form (full-word) dissemination control to its CVE
/// abbreviation form. The CVE only ships abbreviations (`NF`, `OC`, ...),
/// but banner markings use the full words (`NOFORN`, `ORCON`, ...) and the
/// parser must accept both. Phase 3 added this fallback so banner-form
/// markings parse cleanly into a typed `DissemControl`.
///
/// Rules that detect "banner uses portion abbreviation" (E001) read the
/// raw token span via `attrs.token_spans` and inspect the original bytes,
/// so this mapping does not lose the abbreviation-vs-full-word signal.
///
/// Mapping data sourced from [`marque_ism::marking_forms`].
fn parse_dissem_full_form(s: &str) -> Option<DissemControl> {
    // Accept both the Banner Line Abbreviation (e.g., "NOFORN") and the
    // long Marking Title (e.g., "NOT RELEASABLE TO FOREIGN NATIONALS")
    // per CAPCO-2016 §D.1 p27: "Any control markings in the banner
    // line may be spelled out per the 'Marking Title' ... or abbreviated
    // as per the 'Authorized Abbreviation' ... in accordance with the
    // Register". Long-title acceptance is what lets the S001 style rule
    // observe banner-form tokens that use the full title — without it
    // the parser would tag those as Unknown and E008 would fire instead.
    let portion = marque_ism::marking_forms::banner_to_portion(s)
        .or_else(|| marque_ism::marking_forms::title_to_portion(s))?;
    DissemControl::parse(portion)
}

/// Non-IC dissemination control parser covering both the Banner Line
/// Abbreviation (e.g., `"LIMDIS"`) and the long "Marking Title" form
/// (e.g., `"LIMITED DISTRIBUTION"`). Mirror of [`parse_dissem_full_form`]
/// for the §9 non-IC marking set so the S001 style rule can see title
/// tokens across both categories.
fn parse_non_ic_full_form(s: &str) -> Option<NonIcDissem> {
    NonIcDissem::parse(s).or_else(|| {
        let portion = marque_ism::marking_forms::title_to_portion(s)?;
        NonIcDissem::parse(portion)
    })
}

/// Return type for [`parse_rel_to_with_spans`].
///
/// Carries both the recognized country codes and any dissem/non-IC controls
/// that were appended to the last comma entry via an intra-segment `/`
/// separator (e.g., `REL TO USA, FVEY/NF` → countries=[USA, FVEY],
/// trailing_dissem=[NF]).
struct RelToParseResult<'src> {
    countries: Vec<ParsedRelToEntry<'src>>,
    trailing_dissem: Vec<ParsedDissem<'src>>,
    trailing_non_ic: Vec<ParsedNonIcDissem<'src>>,
}

/// Span-aware parse of a `REL TO ...` block. Records one
/// `TokenKind::RelToTrigraph` per recognized country code.
///
/// When a comma entry ends with `/<control>` — e.g., the last entry is
/// `FVEY/NF` instead of just `FVEY` — the function splits on the `/` and
/// parses the tail as additional dissem/non-IC controls. This handles the
/// CAPCO portion-mark convention where dissem controls in the same `//`-slot
/// are separated by `/` (e.g., `(TS//REL TO USA, FVEY/NF)` is valid). The
/// caller must extend its own `dissem`/`non_ic` vecs from the returned
/// `trailing_dissem` / `trailing_non_ic` fields.
///
/// `block_offset` is the absolute byte offset of `block` within the
/// original source buffer.
fn parse_rel_to_with_spans<'src>(
    block: &'src str,
    block_offset: usize,
    tokens: &dyn TokenSet,
    token_spans: &mut Vec<TokenSpan>,
) -> RelToParseResult<'src> {
    // Skip the "REL TO" / "REL" prefix to land on the trigraph list. We
    // need the offset of the *trigraph list* within `block` so that each
    // trigraph's absolute span can be computed.
    let prefix_skip = if let Some(rest) = block.strip_prefix("REL TO") {
        block.len() - rest.len()
    } else if let Some(rest) = block.strip_prefix("REL") {
        block.len() - rest.len()
    } else {
        0
    };
    let after_rel = &block[prefix_skip..];

    let mut countries: Vec<ParsedRelToEntry<'src>> = Vec::new();
    let mut trailing_dissem: Vec<ParsedDissem<'src>> = Vec::new();
    let mut trailing_non_ic: Vec<ParsedNonIcDissem<'src>> = Vec::new();
    // Walk comma-separated entries, tracking each entry's offset within
    // `after_rel` so we can land an absolute span on the trigraph itself
    // (not on any leading whitespace).
    let mut cursor = 0usize;
    for entry in after_rel.split(',') {
        let entry_start_in_after = cursor;
        // Advance past the entry and its trailing comma. On the final
        // iteration this steps one past the end of `after_rel`, but the
        // cursor is never read after the loop ends — the split iterator
        // drives loop termination, not the cursor. usize addition here
        // is bounded by the document size, so no overflow in practice.
        cursor += entry.len() + 1;

        let trim_lead = entry.len() - entry.trim_start().len();
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let abs_start = block_offset + prefix_skip + entry_start_in_after + trim_lead;

        // If the entry contains `/`, the part before the slash is the country
        // code and the part(s) after are additional dissem/non-IC controls
        // packed into the same `//`-slot (e.g., `FVEY/NF` in `REL TO USA,
        // FVEY/NF`). CAPCO portion-mark syntax uses `/` as the intra-segment
        // control separator within a `//`-delimited slot (§A.4 / §D.1).
        if let Some(slash_pos) = trimmed.find('/') {
            let country_part = trimmed[..slash_pos].trim();
            let tail = trimmed[slash_pos + 1..].trim();

            // Parse the country part (may be empty if the slash is leading).
            if !country_part.is_empty() {
                if tokens.is_trigraph(country_part) {
                    if let Some(t) = CountryCode::try_new(country_part.as_bytes()) {
                        let span = Span::new(abs_start, abs_start + country_part.len());
                        countries.push(ParsedRelToEntry::new(t, country_part, span));
                        token_spans.push(TokenSpan {
                            kind: TokenKind::RelToTrigraph,
                            span,
                            text: country_part.into(),
                        });
                    }
                } else {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span: Span::new(abs_start, abs_start + country_part.len()),
                        text: country_part.into(),
                    });
                }
            }

            // Parse each `/`-separated tail token as a dissem or non-IC control.
            let tail_base = abs_start + slash_pos + 1;
            let mut tail_cursor = 0usize;
            for part in tail.split('/') {
                let part_trim_lead = part.len() - part.trim_start().len();
                let part = part.trim();
                let part_abs = tail_base + tail_cursor + part_trim_lead;
                tail_cursor += part.len() + part_trim_lead + 1; // +1 for `/`
                if part.is_empty() {
                    continue;
                }
                if let Some(ctrl) =
                    DissemControl::parse(part).or_else(|| parse_dissem_full_form(part))
                {
                    let span = Span::new(part_abs, part_abs + part.len());
                    trailing_dissem.push(ParsedDissem::new(ctrl, part, span));
                    token_spans.push(TokenSpan {
                        kind: TokenKind::DissemControl,
                        span,
                        text: part.into(),
                    });
                } else if let Some(nic) = parse_non_ic_full_form(part) {
                    let span = Span::new(part_abs, part_abs + part.len());
                    trailing_non_ic.push(ParsedNonIcDissem::new(nic, part, span));
                    token_spans.push(TokenSpan {
                        kind: TokenKind::NonIcDissem,
                        span,
                        text: part.into(),
                    });
                } else {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span: Span::new(part_abs, part_abs + part.len()),
                        text: part.into(),
                    });
                }
            }
            continue;
        }

        if !tokens.is_trigraph(trimmed) {
            // Issue #233: emit an Unknown span for unrecognized
            // entries inside a REL TO block instead of silently
            // dropping them. The decoder's
            // ``DecoderRecognizer::recognize`` step 3a rejects any
            // candidate whose strict parse leaves Unknown spans,
            // which is what makes the fuzzy-trigraph expansion
            // (``try_rel_to_fuzzy_trigraph_candidates``) win the
            // score contest: the original "drop USB" candidate now
            // carries an Unknown span and is filtered out, leaving
            // the corpus-weighted log-prior to break ties between
            // the surviving fuzzy alternates (USA, UZB, …).
            //
            // Strict-path callers still see a clean ``rel_to`` slice
            // — the Unknown span is metadata for the decoder filter,
            // not a parser failure. Existing rules that walk
            // ``token_spans`` already handle ``TokenKind::Unknown``
            // (see E030 sar-indicator-repeat for the analogous
            // pattern at line ~263).
            token_spans.push(TokenSpan {
                kind: TokenKind::Unknown,
                span: Span::new(abs_start, abs_start + trimmed.len()),
                text: trimmed.into(),
            });
            continue;
        }
        // Issue #183: drop the historical `b.len() != 3` gate that
        // silently dropped tetragraphs (`FVEY`, `NATO`, `ACGU`, …)
        // and the longer registered codes (`EU`, `AUSTRALIA_GROUP`)
        // from `rel_to`. `is_trigraph` already covers the full
        // registered CVE recognition surface, including trigraphs,
        // tetragraphs, and longer special forms such as `EU` and
        // `AUSTRALIA_GROUP`; `CountryCode::try_new` accepts
        // 2..=16-byte codes in the CAPCO byte set, so any code that
        // passed `is_trigraph` will also pass `try_new` here.
        let Some(t) = CountryCode::try_new(trimmed.as_bytes()) else {
            continue;
        };
        let span = Span::new(abs_start, abs_start + trimmed.len());
        countries.push(ParsedRelToEntry::new(t, trimmed, span));
        token_spans.push(TokenSpan {
            kind: TokenKind::RelToTrigraph,
            span,
            text: trimmed.into(),
        });
    }
    RelToParseResult {
        countries,
        trailing_dissem,
        trailing_non_ic,
    }
}

// SCI controls, dissemination controls, SAR identifiers, and declass
// exemptions all parse via their generated `parse()` methods (see
// `parse_marking_string` above). The single hand-coded path is
// `parse_classification`, which is documented inline.

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

/// Splits `s` on `/` and returns `(offset, trimmed_token)` pairs where
/// `offset` is the byte offset of the trimmed token within `s`.
///
/// Used by the multi-token block fallback to handle CAPCO §D.1 blocks like
/// `"SI/TK"` or `"NF/LIMDIS"` where multiple entries share one `//` block.
fn split_slash_with_offsets(s: &str) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    let mut pos = 0usize;
    for part in s.split('/') {
        let trim_lead = part.len() - part.trim_start().len();
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            result.push((pos + trim_lead, trimmed));
        }
        pos += part.len() + 1; // +1 for the `/` separator
    }
    result
}

// ===========================================================================
// SAR subparser (§H.5 / §A.6)
// ===========================================================================

/// Parse a single SAR category block.
///
/// `block_text` is the full block text (everything between `//` separators)
/// INCLUDING the `SAR-` or `SPECIAL ACCESS REQUIRED-` indicator prefix.
/// `base` is the absolute byte offset in the original source where
/// `block_text` starts.
///
/// Returns `Some((marking, spans))` when `block_text` starts with a recognized
/// SAR indicator AND the remainder is grammatically non-empty. Each returned
/// [`TokenSpan`] carries absolute byte offsets into the source.
///
/// Grammar (see spec `specs/002-sar-implementation/spec.md` §R2):
///
/// ```text
/// SAR_BLOCK    := INDICATOR PROGRAM ("/" PROGRAM)*
/// INDICATOR    := "SAR-" | "SPECIAL ACCESS REQUIRED-"
/// PROGRAM      := PROG_ID ( "-" COMPARTMENT )?
/// COMPARTMENT  := COMP_ID (" " SUB_COMP)*
/// PROG_ID      := [A-Z0-9]{2,3}           (SAR- form)
///               | [A-Z ]+                  (full-indicator form)
/// COMP_ID      := [A-Z0-9]+
/// SUB_COMP     := [A-Z0-9]+
/// ```
///
/// Rejection returns `None`:
/// - `SAR` without trailing hyphen.
/// - `SAR-` with an empty program identifier.
/// - A `//` sequence inside `block_text` (should not happen — the outer
///   category-block splitter would have handed us two separate blocks —
///   but we reject defensively).
/// - Empty string.
///
/// Ordering, classification, and roll-up constraints are NOT enforced here;
/// they are rule-layer (P3/P4) concerns.
fn parse_sar_category(block_text: &str, base: usize) -> Option<(SarMarking, Vec<TokenSpan>)> {
    // Defensive: `//` would mean the outer splitter gave us more than one
    // block. Refuse so the caller can record the text as Unknown and let
    // E030 handle it separately.
    if block_text.contains("//") {
        return None;
    }

    // Identify the indicator variant. Longer prefix first so `SPECIAL
    // ACCESS REQUIRED-` wins over any putative `SAR-` substring.
    let (indicator, indicator_lit) = if block_text.starts_with("SPECIAL ACCESS REQUIRED-") {
        (SarIndicator::Full, "SPECIAL ACCESS REQUIRED-")
    } else if block_text.starts_with("SAR-") {
        (SarIndicator::Abbrev, "SAR-")
    } else {
        return None;
    };
    let rest_offset = indicator_lit.len();
    let rest = &block_text[rest_offset..];
    if rest.is_empty() {
        return None;
    }

    let mut spans: Vec<TokenSpan> = Vec::new();

    // Record the indicator span (does NOT include the first character of
    // the program identifier — only the literal `SAR-` / `SPECIAL ACCESS
    // REQUIRED-` including the trailing hyphen).
    spans.push(TokenSpan {
        kind: TokenKind::SarIndicator,
        span: Span::new(base, base + indicator_lit.len()),
        text: indicator_lit.into(),
    });

    let mut programs: Vec<SarProgram> = Vec::new();

    // Split the remainder on `/` into program chunks. Each chunk is a
    // `PROGRAM` production: `PROG_ID` optionally followed by `-COMPARTMENT`.
    let mut chunk_offset = rest_offset; // offset within block_text
    for (i, prog_chunk) in rest.split('/').enumerate() {
        if i > 0 {
            chunk_offset += 1; // account for the `/` just consumed
        }
        let program_base = base + chunk_offset;

        let program = parse_sar_program(prog_chunk, program_base, indicator, &mut spans)?;
        programs.push(program);
        chunk_offset += prog_chunk.len();
    }

    if programs.is_empty() {
        return None;
    }

    Some((
        SarMarking::new(indicator, programs.into_boxed_slice()),
        spans,
    ))
}

/// Parse a single `PROGRAM` production.
///
/// `chunk` is everything between adjacent `/` separators (or between the
/// indicator and the next `/`, or the tail of the block). `base` is the
/// absolute offset of `chunk[0]` in the source buffer. `indicator` drives
/// the shape of the program identifier only; compartment and
/// sub-compartment parsing is identical for both indicator forms.
///
/// Grammar: `PROG_ID ( "-" COMPARTMENT )? ( "-" COMPARTMENT )* `, where
/// `COMPARTMENT` is `COMP_ID (" " SUB_COMP)*`. `PROG_ID` shape is:
///
/// - **Abbrev** (`SAR-`): 2–3 alphanumeric characters.
/// - **Full** (`SPECIAL ACCESS REQUIRED-`): one or more uppercase ASCII
///   letters, optionally with spaces. Hyphens are NOT permitted inside
///   the program identifier for the full form — the first `-` always
///   marks the program/compartment boundary (CAPCO-2016 §H.5 p100).
///
/// Canonical example per §H.5 p100: `SAR-BP-J12 J54-K15/CD-...` decomposes
/// BP as two compartments `J12` (with sub-compartment `J54`) and `K15`.
/// Within one program the sequence alternates:
///   `PROG "-" COMP (" " SUB)* ( "-" COMP (" " SUB)* )*`
///
/// # Shape gates
///
/// Token admission goes through the documented `marque-ism`
/// predicates rather than inline byte-class checks
/// (FR-015 / CHK030):
///
/// - Program identifier (Abbrev): [`SarProgram::admits_program_id_abbrev`]
///   — 2-3 ASCII alnum.
/// - Program identifier (Full): [`SarProgram::admits_program_id_full`]
///   — uppercase ASCII letters with optional spaces, must contain
///   at least one non-space byte; hyphens and digits rejected.
/// - Compartment identifier: [`SarCompartment::admits_identifier`]
///   — ≥1 ASCII alnum.
/// - Sub-compartment identifier: [`SarCompartment::admits_identifier`]
///   (same predicate; CAPCO-2016 §H.5 pp 99-100 places both grammar
///   positions under one rule).
///
/// Routing the parser through the same predicates the
/// `Vocabulary<CapcoScheme>::shape_admits(CAT_SAR, _)` arm calls
/// pins the parser's accept set to the documented vocabulary
/// surface. This satisfies FR-015 (admission via documented
/// vocabulary surface) and CHK030 (no inline `is_ascii_alphanumeric`
/// byte-class checks). The same pattern is used at
/// [`parse_fgi_marker`] for FGI trigraph admission.
fn parse_sar_program(
    chunk: &str,
    base: usize,
    indicator: SarIndicator,
    spans: &mut Vec<TokenSpan>,
) -> Option<SarProgram> {
    if chunk.is_empty() {
        return None;
    }

    // Split the chunk on `-`. The first segment is the program identifier;
    // each subsequent segment is a compartment (with optional space-joined
    // sub-compartments).
    let mut segments = split_with_offsets(chunk, '-');
    if segments.is_empty() {
        return None;
    }

    // Program identifier: first segment. Shape check depends on indicator.
    let (prog_off, prog_id) = segments.remove(0);
    if prog_id.is_empty() {
        return None;
    }
    // FR-015 admission: route the program identifier shape gate
    // through the canonical `marque-ism` predicates, one per
    // indicator form. Both predicates are pure / allocation-free
    // (Constitution Principle II) and carry their CAPCO-2016 §H.5
    // citations alongside the predicate body — keeping the gate
    // single-sited prevents drift between the parser and the
    // `Vocabulary<CapcoScheme>::shape_admits(CAT_SAR, _)` admission
    // surface (CHK030). Mirrors the FGI marker site at
    // [`parse_fgi_marker`] which routes through
    // [`CountryCode::admits_country_token`].
    let prog_shape_ok = match indicator {
        // §H.5 p101: "A program identifier abbreviation is the two
        // or three-character designator for the program."
        // §H.5 p99: "SAR program identifiers are alphanumeric values."
        SarIndicator::Abbrev => SarProgram::admits_program_id_abbrev(prog_id.as_bytes()),
        // §H.5 p101 + Table 7 §H.5 p100: full nickname is uppercase
        // letters with optional spaces (no digits, no hyphens). The
        // hyphen exclusion is load-bearing — the first hyphen after
        // the indicator literal always marks the program/compartment
        // boundary at this parser site.
        SarIndicator::Full => SarProgram::admits_program_id_full(prog_id.as_bytes()),
    };
    if !prog_shape_ok {
        return None;
    }
    spans.push(TokenSpan {
        kind: TokenKind::SarProgram,
        span: Span::new(base + prog_off, base + prog_off + prog_id.len()),
        text: prog_id.into(),
    });

    // Remaining segments: each is a compartment, possibly with
    // space-separated sub-compartments.
    let mut compartments: Vec<SarCompartment> = Vec::with_capacity(segments.len());
    for (seg_off, seg) in segments {
        if seg.is_empty() {
            return None;
        }
        // Split segment on ` ` — first token is compartment, rest are subs.
        let mut parts = split_with_offsets(seg, ' ');
        let (comp_rel_off, comp_id) = parts.remove(0);
        // FR-015 admission: compartment identifier shape gated
        // through the canonical `marque-ism` predicate.
        // CAPCO-2016 §H.5 pp 99-100: "SAR program identifiers are
        // alphanumeric values"; the surrounding prose applies the
        // same rule to compartments and sub-compartments. Length
        // bound is ≥1 (manual silent on upper bound; marque admits
        // length 1+, with the divergence documented at the
        // predicate). Same predicate handles the sub-compartment
        // case below (T090 / T091).
        if !SarCompartment::admits_identifier(comp_id.as_bytes()) {
            return None;
        }
        let comp_abs_off = seg_off + comp_rel_off;
        spans.push(TokenSpan {
            kind: TokenKind::SarCompartment,
            span: Span::new(base + comp_abs_off, base + comp_abs_off + comp_id.len()),
            text: comp_id.into(),
        });

        let mut subs: Vec<SmolStr> = Vec::with_capacity(parts.len());
        for (sub_rel_off, sub_id) in parts {
            // FR-015 admission: sub-compartment identifier shape
            // gated through the same canonical predicate as the
            // compartment slot. CAPCO-2016 §H.5 pp 99-100 places
            // both grammar positions under one rule (alphanumeric
            // values, no character-class or length distinction);
            // a single predicate admits both correctly (T091).
            if !SarCompartment::admits_identifier(sub_id.as_bytes()) {
                return None;
            }
            let sub_abs_off = seg_off + sub_rel_off;
            spans.push(TokenSpan {
                kind: TokenKind::SarSubCompartment,
                span: Span::new(base + sub_abs_off, base + sub_abs_off + sub_id.len()),
                text: sub_id.into(),
            });
            subs.push(sub_id.into());
        }

        compartments.push(SarCompartment::new(comp_id, subs.into_boxed_slice()));
    }

    Some(SarProgram::new(prog_id, compartments.into_boxed_slice()))
}

/// Split `s` on `delim`, returning `(offset_in_s, token)` pairs. Unlike
/// [`split_slash_with_offsets`], this preserves empty tokens so callers can
/// detect malformed input (e.g., `SAR--BP` → two segments, the first empty).
fn split_with_offsets(s: &str, delim: char) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    let mut pos = 0usize;
    let delim_len = delim.len_utf8();
    for part in s.split(delim) {
        result.push((pos, part));
        pos += part.len() + delim_len;
    }
    result
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_ism::CanonicalAttrs;
    use marque_ism::span::{MarkingCandidate, MarkingType, Span};
    use marque_ism::token_set::CapcoTokenSet;

    /// Test-helper output: a [`ParsedMarking`] post-`from_parsed_unchecked`,
    /// so existing assertions on the typed `attrs.classification` /
    /// `attrs.dissem_controls` shape continue to work without per-test
    /// edits during the PR 3a rename.
    ///
    /// Test-fixture carve-out per Constitution V Principle V — the
    /// adapter is invoked here only to construct test inputs whose
    /// shape mirrors the engine's post-recognition view.
    pub(super) struct CanonicalParsed {
        pub attrs: CanonicalAttrs,
        #[allow(dead_code)] // tests inspect attrs only; kept for parity
        pub source_span: Span,
        #[allow(dead_code)]
        pub kind: MarkingType,
    }

    impl<'src> From<ParsedMarking<'src>> for CanonicalParsed {
        fn from(p: ParsedMarking<'src>) -> Self {
            Self {
                attrs: marque_ism::from_parsed_unchecked(p.attrs),
                source_span: p.source_span,
                kind: p.kind,
            }
        }
    }

    fn make_candidate(text: &[u8], kind: MarkingType, offset: usize) -> MarkingCandidate {
        MarkingCandidate {
            span: Span::new(offset, offset + text.len()),
            kind,
        }
    }

    fn parse_banner(text: &str) -> CanonicalParsed {
        let source = text.as_bytes();
        let tokens = CapcoTokenSet;
        let parser = Parser::new(&tokens);
        let candidate = make_candidate(source, MarkingType::Banner, 0);
        parser
            .parse(&candidate, source)
            .expect("parse should succeed")
            .into()
    }

    fn parse_portion(text: &str) -> CanonicalParsed {
        let source = text.as_bytes();
        let tokens = CapcoTokenSet;
        let parser = Parser::new(&tokens);
        let candidate = make_candidate(source, MarkingType::Portion, 0);
        parser
            .parse(&candidate, source)
            .expect("parse should succeed")
            .into()
    }

    // --- declass exemption in banner (E005 detection) ---

    #[test]
    fn banner_with_declass_exemption_populates_attrs() {
        // A banner string that (incorrectly) contains a declass exemption code.
        // parse_marking_string must populate declass_exemption so E005 can fire.
        let parsed = parse_banner("SECRET//25X1//NOFORN");
        assert!(
            parsed.attrs.declass_exemption.is_some(),
            "declass_exemption should be populated when 25X1 appears in banner"
        );
        use marque_ism::DeclassExemption;
        assert_eq!(
            parsed.attrs.declass_exemption,
            Some(DeclassExemption::X25x1)
        );
    }

    #[test]
    fn portion_with_declass_exemption_populates_attrs() {
        let parsed = parse_portion("(SECRET//50X1-HUM)");
        assert!(parsed.attrs.declass_exemption.is_some());
    }

    // --- declass date in banner (E005 detection) ---

    #[test]
    fn banner_with_declass_date_populates_attrs() {
        let parsed = parse_banner("SECRET//20301231//NOFORN");
        assert_eq!(
            parsed.attrs.declassify_on,
            Some(marque_ism::IsmDate::Date(2030, 12, 31)),
            "declassify_on should be populated when YYYYMMDD appears in banner"
        );
    }

    #[test]
    fn banner_with_four_digit_year_populates_attrs() {
        let parsed = parse_banner("SECRET//2035");
        assert_eq!(
            parsed.attrs.declassify_on,
            Some(marque_ism::IsmDate::Year(2035))
        );
    }

    // --- normal banner (no declass tokens) ---

    #[test]
    fn banner_without_declass_leaves_fields_none() {
        let parsed = parse_banner("TOP SECRET//SI//NOFORN");
        assert!(parsed.attrs.declassify_on.is_none());
        assert!(parsed.attrs.declass_exemption.is_none());
    }

    // --- is_declass_date helper ---

    #[test]
    fn is_declass_date_accepts_yyyymmdd() {
        assert!(is_declass_date("20301231"));
    }

    #[test]
    fn is_declass_date_accepts_yyyy() {
        assert!(is_declass_date("2035"));
    }

    #[test]
    fn is_declass_date_rejects_non_digit() {
        assert!(!is_declass_date("2030X231"));
        assert!(!is_declass_date("YYYYMMDD"));
    }

    #[test]
    fn is_declass_date_rejects_wrong_length() {
        assert!(!is_declass_date("203012"));
        assert!(!is_declass_date("203012311"));
    }

    #[test]
    fn is_declass_date_rejects_impossible_calendar_dates() {
        // Month 13 is impossible.
        assert!(!is_declass_date("20301340"));
        // Day 0 is impossible.
        assert!(!is_declass_date("20300100"));
        // 2003-02-31 doesn't exist (February has at most 29 days).
        assert!(!is_declass_date("20030231"));
        // 2003-04-31 doesn't exist (April has 30 days).
        assert!(!is_declass_date("20030431"));
    }

    // --- token spans ---

    #[test]
    fn token_spans_track_offsets_in_banner() {
        let parsed = parse_banner("TOP SECRET//SI//NF");
        let kinds: Vec<TokenKind> = parsed.attrs.token_spans.iter().map(|t| t.kind).collect();
        // Two separators + classification + sci + dissem.
        assert!(kinds.contains(&TokenKind::Separator));
        assert!(kinds.contains(&TokenKind::Classification));
        assert!(kinds.contains(&TokenKind::SciControl));
        assert!(kinds.contains(&TokenKind::DissemControl));

        // Find each by kind and verify the byte slice matches.
        let src = b"TOP SECRET//SI//NF";
        let cls = parsed
            .attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
            .unwrap();
        assert_eq!(cls.span.as_str(src).unwrap(), "TOP SECRET");

        let sci = parsed
            .attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::SciControl)
            .unwrap();
        assert_eq!(sci.span.as_str(src).unwrap(), "SI");

        let dissem = parsed
            .attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::DissemControl)
            .unwrap();
        assert_eq!(dissem.span.as_str(src).unwrap(), "NF");
    }

    #[test]
    fn token_spans_strip_paren_in_portion() {
        let parsed = parse_portion("(SECRET//NF)");
        let src = b"(SECRET//NF)";
        let cls = parsed
            .attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
            .unwrap();
        // SECRET starts at byte 1 (after the open paren), runs to byte 7.
        assert_eq!(cls.span.start, 1);
        assert_eq!(cls.span.end, 7);
        assert_eq!(cls.span.as_str(src).unwrap(), "SECRET");

        let dissem = parsed
            .attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::DissemControl)
            .unwrap();
        // NF starts at byte 9 (after `SECRET//`).
        assert_eq!(dissem.span.start, 9);
        assert_eq!(dissem.span.end, 11);
    }

    #[test]
    fn token_spans_record_unknown_token() {
        let parsed = parse_banner("SECRET//XYZZY//NOFORN");
        let unknowns: Vec<&TokenSpan> = parsed
            .attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Unknown)
            .collect();
        assert_eq!(unknowns.len(), 1);
        assert_eq!(
            unknowns[0].span.as_str(b"SECRET//XYZZY//NOFORN").unwrap(),
            "XYZZY"
        );
    }

    #[test]
    fn token_spans_record_rel_to_trigraphs() {
        let parsed = parse_banner("SECRET//REL TO USA, GBR, AUS");
        let trigraphs: Vec<&TokenSpan> = parsed
            .attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToTrigraph)
            .collect();
        assert_eq!(trigraphs.len(), 3);
        let src = b"SECRET//REL TO USA, GBR, AUS";
        assert_eq!(trigraphs[0].span.as_str(src).unwrap(), "USA");
        assert_eq!(trigraphs[1].span.as_str(src).unwrap(), "GBR");
        assert_eq!(trigraphs[2].span.as_str(src).unwrap(), "AUS");
    }

    // -----------------------------------------------------------------------
    // Issue #183 PR-A — country-code widening: REL TO must preserve
    // tetragraphs (FVEY, NATO, ACGU, …), `EU`, and `AUSTRALIA_GROUP`.
    // Pre-PR-A, every non-3-byte token was silently dropped at the
    // `b.len() != 3` gate in `parse_rel_to_with_spans`, so a marking
    // like `(S//REL TO USA, FVEY, GBR)` arrived at the rule layer as
    // `rel_to: [USA, GBR]` — FVEY gone with no diagnostic.
    // -----------------------------------------------------------------------

    #[test]
    fn rel_to_preserves_tetragraph_fvey() {
        let parsed = parse_banner("SECRET//REL TO USA, FVEY, GBR");
        let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
        assert_eq!(
            codes,
            vec!["USA", "FVEY", "GBR"],
            "FVEY tetragraph must land in rel_to (issue #183 silent-drop fix)"
        );
    }

    #[test]
    fn rel_to_preserves_opaque_tetragraph_nato() {
        let parsed = parse_banner("SECRET//REL TO USA, NATO, GBR");
        let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
        assert_eq!(
            codes,
            vec!["USA", "NATO", "GBR"],
            "NATO is in CVE TRIGRAPHS recognition set; rel_to must preserve it \
             even though membership expansion is deferred to Phase F"
        );
    }

    #[test]
    fn rel_to_preserves_two_byte_eu() {
        let parsed = parse_banner("SECRET//REL TO USA, EU");
        let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
        assert_eq!(
            codes,
            vec!["USA", "EU"],
            "EU (2-byte CVE entry) must round-trip through the parser"
        );
    }

    #[test]
    fn rel_to_preserves_long_australia_group() {
        let parsed = parse_banner("SECRET//REL TO USA, AUSTRALIA_GROUP");
        let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
        assert_eq!(
            codes,
            vec!["USA", "AUSTRALIA_GROUP"],
            "AUSTRALIA_GROUP (15-byte CVE entry, contains underscore) \
             must round-trip through the parser"
        );
    }

    #[test]
    fn rel_to_token_span_widens_to_actual_code_length() {
        // Pre-PR-A the RelToTrigraph TokenSpan was hardcoded to 3
        // bytes (`Span::new(abs_start, abs_start + 3)`). Widening
        // matters because consumers — the E002 fix splice and
        // diagnostic underlines — read `span.as_str()` to anchor
        // their replacement / message at the exact source bytes.
        let parsed = parse_banner("SECRET//REL TO USA, FVEY, AUSTRALIA_GROUP");
        let trigraph_spans: Vec<&TokenSpan> = parsed
            .attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToTrigraph)
            .collect();
        let src = b"SECRET//REL TO USA, FVEY, AUSTRALIA_GROUP";
        assert_eq!(trigraph_spans[0].span.as_str(src).unwrap(), "USA");
        assert_eq!(trigraph_spans[1].span.as_str(src).unwrap(), "FVEY");
        assert_eq!(
            trigraph_spans[2].span.as_str(src).unwrap(),
            "AUSTRALIA_GROUP"
        );
    }

    #[test]
    fn rel_to_drops_unrecognized_token_silently() {
        // Defensive: tokens outside the CVE recognition set
        // (`is_trigraph` is false) are still skipped — we widened
        // recognition, not the gate. `XYZQ` is a 4-char string not
        // in the CVE TRIGRAPHS list.
        let parsed = parse_banner("SECRET//REL TO USA, XYZQ, GBR");
        let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
        assert_eq!(codes, vec!["USA", "GBR"]);
    }

    #[test]
    fn token_spans_record_separators() {
        let parsed = parse_banner("SECRET//NF");
        let seps: Vec<&TokenSpan> = parsed
            .attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Separator)
            .collect();
        assert_eq!(seps.len(), 1);
        let src = b"SECRET//NF";
        assert_eq!(seps[0].span.as_str(src).unwrap(), "//");
    }

    // -----------------------------------------------------------------------
    // Non-US classification parsing
    // -----------------------------------------------------------------------

    #[test]
    fn nato_banner_parses_all_variants() {
        for (input, expected) in [
            ("//NATO UNCLASSIFIED", NatoClassification::NatoUnclassified),
            ("//NATO RESTRICTED", NatoClassification::NatoRestricted),
            ("//NATO CONFIDENTIAL", NatoClassification::NatoConfidential),
            (
                "//NATO CONFIDENTIAL ATOMAL",
                NatoClassification::NatoConfidentialAtomal,
            ),
            ("//NATO SECRET", NatoClassification::NatoSecret),
            ("//NATO SECRET ATOMAL", NatoClassification::NatoSecretAtomal),
            ("//COSMIC TOP SECRET", NatoClassification::CosmicTopSecret),
            (
                "//COSMIC TOP SECRET ATOMAL",
                NatoClassification::CosmicTopSecretAtomal,
            ),
            (
                "//COSMIC TOP SECRET-BOHEMIA",
                NatoClassification::CosmicTopSecretBohemia,
            ),
            (
                "//COSMIC TOP SECRET-BALK",
                NatoClassification::CosmicTopSecretBalk,
            ),
        ] {
            let parsed = parse_banner(input);
            assert_eq!(
                parsed.attrs.classification,
                Some(MarkingClassification::Nato(expected)),
                "failed for banner: {input}"
            );
        }
    }

    #[test]
    fn nato_portion_parses_all_variants() {
        for (input, expected) in [
            ("(//NU)", NatoClassification::NatoUnclassified),
            ("(//NR)", NatoClassification::NatoRestricted),
            ("(//NC)", NatoClassification::NatoConfidential),
            ("(//NCA)", NatoClassification::NatoConfidentialAtomal),
            ("(//NC-A)", NatoClassification::NatoConfidentialAtomal),
            ("(//NS)", NatoClassification::NatoSecret),
            ("(//NSAT)", NatoClassification::NatoSecretAtomal),
            ("(//NS-A)", NatoClassification::NatoSecretAtomal),
            ("(//CTS)", NatoClassification::CosmicTopSecret),
            ("(//CTSA)", NatoClassification::CosmicTopSecretAtomal),
            ("(//CTS-A)", NatoClassification::CosmicTopSecretAtomal),
            ("(//CTS-B)", NatoClassification::CosmicTopSecretBohemia),
            ("(//CTS-BALK)", NatoClassification::CosmicTopSecretBalk),
        ] {
            let parsed = parse_portion(input);
            assert_eq!(
                parsed.attrs.classification,
                Some(MarkingClassification::Nato(expected)),
                "failed for portion: {input}"
            );
        }
    }

    #[test]
    fn nato_banner_with_rel_to() {
        let parsed = parse_banner("//NATO SECRET//REL TO USA, GBR");
        assert_eq!(
            parsed.attrs.classification,
            Some(MarkingClassification::Nato(NatoClassification::NatoSecret)),
        );
        assert_eq!(parsed.attrs.rel_to.len(), 2);
        assert_eq!(parsed.attrs.rel_to[0], CountryCode::USA);
    }

    #[test]
    fn joint_banner_parses_correctly() {
        let parsed = parse_banner("//JOINT S USA GBR");
        match &parsed.attrs.classification {
            Some(MarkingClassification::Joint(j)) => {
                assert_eq!(j.level, Classification::Secret);
                assert_eq!(j.countries.len(), 2);
                assert_eq!(j.countries[0], CountryCode::USA);
                assert_eq!(j.countries[1].as_str(), "GBR");
            }
            other => panic!("expected Joint, got: {other:?}"),
        }
    }

    #[test]
    fn joint_banner_parses_top_secret_multi_word_level() {
        // The JOINT parser has a separate two-token path for the
        // multi-word `TOP SECRET` level (vs. the single-token `S` /
        // `TS` / `C` / `U` abbreviations). Exercises lines 905-907
        // and 909 of `parse_joint_classification`.
        let parsed = parse_banner("//JOINT TOP SECRET USA GBR");
        match &parsed.attrs.classification {
            Some(MarkingClassification::Joint(j)) => {
                assert_eq!(j.level, Classification::TopSecret);
                assert_eq!(j.countries.len(), 2);
                assert_eq!(j.countries[0], CountryCode::USA);
                assert_eq!(j.countries[1].as_str(), "GBR");
            }
            other => panic!("expected Joint(TopSecret), got: {other:?}"),
        }
    }

    #[test]
    fn joint_banner_rejects_bare_top_without_secret() {
        // `TOP` alone is not a valid classification level — the
        // JOINT parser must return None and let the parent path
        // try other foreign-classification shapes. Exercises the
        // `else { return None; }` branch of the TOP-SECRET path.
        let parsed = parse_banner("//JOINT TOP USA GBR");
        assert!(
            !matches!(
                parsed.attrs.classification,
                Some(MarkingClassification::Joint(_))
            ),
            "bare TOP must not parse as a JOINT classification"
        );
    }

    #[test]
    fn joint_portion_with_rel_to() {
        let parsed = parse_portion("(//JOINT TS USA AUS GBR//REL TO USA, AUS, GBR)");
        match &parsed.attrs.classification {
            Some(MarkingClassification::Joint(j)) => {
                assert_eq!(j.level, Classification::TopSecret);
                assert_eq!(j.countries.len(), 3);
            }
            other => panic!("expected Joint, got: {other:?}"),
        }
        assert_eq!(parsed.attrs.rel_to.len(), 3);
    }

    #[test]
    fn fgi_single_country_parses() {
        let parsed = parse_portion("(//GBR S//NF)");
        match &parsed.attrs.classification {
            Some(MarkingClassification::Fgi(f)) => {
                assert_eq!(f.level, Classification::Secret);
                assert_eq!(f.countries.len(), 1);
                assert_eq!(f.countries[0].as_str(), "GBR");
            }
            other => panic!("expected Fgi, got: {other:?}"),
        }
    }

    #[test]
    fn fgi_multiple_countries_parses() {
        let parsed = parse_banner("//GBR DEU TS//NF");
        match &parsed.attrs.classification {
            Some(MarkingClassification::Fgi(f)) => {
                assert_eq!(f.level, Classification::TopSecret);
                assert_eq!(f.countries.len(), 2);
            }
            other => panic!("expected Fgi, got: {other:?}"),
        }
    }

    #[test]
    fn fgi_placeholder_country_parses() {
        // FGI as placeholder for unknown country + level
        let parsed = parse_portion("(//FGI S//NF)");
        match &parsed.attrs.classification {
            Some(MarkingClassification::Fgi(f)) => {
                assert_eq!(f.level, Classification::Secret);
                assert!(
                    f.countries.is_empty(),
                    "FGI placeholder should have no countries"
                );
            }
            other => panic!("expected Fgi, got: {other:?}"),
        }
    }

    #[test]
    fn fgi_non_uppercase_trigraph_rejected() {
        // `CountryCode::try_new` accepts ASCII uppercase letter,
        // ASCII digit, or underscore (issue #183 widened the byte
        // set to cover `AX2`/`AX3` and `AUSTRALIA_GROUP`). A 3-byte
        // token containing a lowercase letter still fails that
        // check and trips the `CountryCode::try_new(...)?` rejection
        // path in `parse_fgi_classification`.
        let parsed = parse_banner("//Gbr S//NF");
        assert!(
            !matches!(
                parsed.attrs.classification,
                Some(MarkingClassification::Fgi(_))
            ),
            "Gbr should not parse as a valid FGI classification: {:?}",
            parsed.attrs.classification,
        );
    }

    #[test]
    fn fgi_no_level_is_error() {
        // //FGI// with no classification level — classification should be None
        let parsed = parse_banner("//FGI//NF");
        assert!(
            parsed.attrs.classification.is_none()
                || matches!(
                    parsed.attrs.classification,
                    Some(MarkingClassification::Us(_))
                ),
            "bare FGI with no level should not produce a valid non-US classification: {:?}",
            parsed.attrs.classification,
        );
    }

    #[test]
    fn fgi_marker_in_us_marking() {
        let parsed = parse_banner("SECRET//FGI DEU//NOFORN");
        assert_eq!(
            parsed.attrs.classification,
            Some(MarkingClassification::Us(Classification::Secret)),
        );
        let marker = parsed
            .attrs
            .fgi_marker
            .as_ref()
            .expect("should have FGI marker");
        match marker {
            FgiMarker::Acknowledged { countries, .. } => {
                assert_eq!(countries.len(), 1);
                assert_eq!(countries[0].as_str(), "DEU");
            }
            FgiMarker::SourceConcealed => panic!("expected acknowledged variant"),
        }
    }

    #[test]
    fn fgi_marker_bare_is_source_concealed() {
        let parsed = parse_banner("SECRET//FGI//NOFORN");
        assert_eq!(
            parsed.attrs.classification,
            Some(MarkingClassification::Us(Classification::Secret)),
        );
        let marker = parsed
            .attrs
            .fgi_marker
            .as_ref()
            .expect("should have FGI marker");
        // CAPCO §H.7 p123: bare `FGI` is the lawful source-concealed
        // banner form, distinct from a parser failure.
        assert!(matches!(marker, FgiMarker::SourceConcealed));
    }

    // ---- T088 + T093: FR-015 / FR-016 closure for parse_fgi_marker ----
    //
    // GH #280 retired the transitional `unwrap_or(SourceConcealed)`
    // fallback. These tests pin the three lawful cases per CAPCO-2016
    // §H.7 p123 (the only two banner forms — concealed `FGI` and
    // acknowledged `FGI [LIST]`) plus the negative cases that map to
    // `None`. The parser is invoked via `parse_banner` here to
    // exercise the same call site (`crates/core/src/parser.rs:345`)
    // that the engine reaches in production; the public surface of
    // `parse_fgi_marker` itself is private to this module.

    #[test]
    fn fgi_marker_multi_country_acknowledged() {
        // Three-country list: tests that the SmallVec inline path
        // (4 codes) covers the typical case without heap allocation,
        // and that the parser admits each token through
        // `CountryCode::admits_fgi_trigraph`.
        let parsed = parse_banner("SECRET//FGI USA GBR JPN//NOFORN");
        let marker = parsed
            .attrs
            .fgi_marker
            .as_ref()
            .expect("should have FGI marker");
        match marker {
            FgiMarker::Acknowledged { countries, .. } => {
                assert_eq!(countries.len(), 3);
                assert_eq!(countries[0].as_str(), "USA");
                assert_eq!(countries[1].as_str(), "GBR");
                assert_eq!(countries[2].as_str(), "JPN");
            }
            FgiMarker::SourceConcealed => panic!("expected Acknowledged variant"),
        }
    }

    #[test]
    fn fgi_marker_lowercase_token_no_marker() {
        // Lowercase fails `admits_fgi_trigraph`; the previous
        // transitional behavior would have silently dropped the
        // token, producing an empty country list and falling back to
        // `SourceConcealed`. Post-T088: the parser returns `None`,
        // so `attrs.fgi_marker` is unset (CAPCO §H.7 p123 disallows
        // a degraded lawful form on shape failure).
        let parsed = parse_banner("SECRET//FGI deu//NOFORN");
        assert!(
            parsed.attrs.fgi_marker.is_none(),
            "lowercase trigraph must fail FGI marker shape gate (got {:?})",
            parsed.attrs.fgi_marker,
        );
    }

    #[test]
    fn fgi_marker_tetragraph_admits_per_capco_h7() {
        // CAPCO-2016 §H.7 p123 spells out the canonical example:
        // `SECRET//FGI GBR JPN NATO//REL TO USA, GBR, JPN, NATO`
        // — `NATO` is a 4-letter Annex A tetragraph admitted in the
        // same FGI list as the trigraphs. The PR #311 review caught
        // a regression where the parser narrowed admission to
        // `admits_fgi_trigraph` (3-only); the post-fix
        // `admits_country_token` widens to 2/3/4 ASCII upper,
        // matching the §H.7 grammar.
        let parsed = parse_banner("SECRET//FGI USA NATO//NOFORN");
        let marker = parsed
            .attrs
            .fgi_marker
            .as_ref()
            .expect("FGI USA NATO admits per §H.7 p123");
        match marker {
            FgiMarker::Acknowledged { countries, .. } => {
                assert_eq!(countries.len(), 2);
                let names: Vec<&str> = countries.iter().map(|c| c.as_str()).collect();
                assert!(names.contains(&"USA"), "USA must appear; got {names:?}");
                assert!(names.contains(&"NATO"), "NATO must appear; got {names:?}");
            }
            FgiMarker::SourceConcealed => panic!("expected Acknowledged([USA, NATO])"),
        }
    }

    #[test]
    fn fgi_marker_unregistered_trigraph_shape_admits_but_marker_records_it() {
        // `XYZ` is shape-admissible (3 ASCII upper) — `admits_fgi_trigraph`
        // is a *shape* gate, not a registry-membership gate. The CVE
        // table check (`is_trigraph` against the GENC trigraph
        // registry) lives in the rule layer (S### / E###), not the
        // parser. So `XYZ` parses as an Acknowledged country code
        // and a downstream rule flags the unknown trigraph.
        //
        // This pins the boundary: shape vs. registry. The earlier
        // (pre-T088) implementation also accepted `XYZ` because
        // `CountryCode::try_new` succeeds on 3 ASCII upper, so this
        // is not a regression — it's a confirmation that the gate's
        // semantics are scoped correctly.
        let parsed = parse_banner("SECRET//FGI XYZ//NOFORN");
        let marker = parsed
            .attrs
            .fgi_marker
            .as_ref()
            .expect("XYZ is shape-admissible; rule layer flags registry membership");
        match marker {
            FgiMarker::Acknowledged { countries, .. } => {
                assert_eq!(countries.len(), 1);
                assert_eq!(countries[0].as_str(), "XYZ");
            }
            FgiMarker::SourceConcealed => panic!("expected Acknowledged variant"),
        }
    }

    #[test]
    fn fgi_marker_direct_three_cases() {
        // Direct exercise of `parse_fgi_marker` at the same module
        // level, covering the three lawful return cases without the
        // banner wrapper. Pins behavior the public `parse_banner`
        // tests above route through indirectly.

        // Case 1: bare "FGI" → Some(SourceConcealed)
        assert!(matches!(
            parse_fgi_marker("FGI"),
            Some(FgiMarker::SourceConcealed),
        ));

        // Case 2: "FGI <trigraph>" → Some(Acknowledged)
        match parse_fgi_marker("FGI USA") {
            Some(FgiMarker::Acknowledged { countries, .. }) => {
                assert_eq!(countries.len(), 1);
                assert_eq!(countries[0].as_str(), "USA");
            }
            other => panic!("expected Acknowledged([USA]), got {other:?}"),
        }

        // Case 2 (multi): up to and beyond SmallVec inline capacity
        match parse_fgi_marker("FGI USA GBR DEU JPN FRA") {
            Some(FgiMarker::Acknowledged { countries, .. }) => {
                assert_eq!(countries.len(), 5);
                let names: Vec<&str> = countries.iter().map(|c| c.as_str()).collect();
                assert_eq!(names, ["USA", "GBR", "DEU", "JPN", "FRA"]);
            }
            other => panic!("expected 5-country Acknowledged, got {other:?}"),
        }

        // Case 3: empty input → None
        assert!(parse_fgi_marker("").is_none());

        // Case 3: lowercase token → None (FR-016 closure)
        assert!(parse_fgi_marker("FGI deu").is_none());

        // Case 2 (mixed shapes per §H.7 p123): trigraph + tetragraph
        // → Some(Acknowledged) with both countries. PR #311 review
        // caught the prior trigraph-only narrowing; post-fix
        // admission accepts the spec-canonical example.
        match parse_fgi_marker("FGI GBR JPN NATO") {
            Some(FgiMarker::Acknowledged { countries, .. }) => {
                assert_eq!(countries.len(), 3);
                let names: Vec<&str> = countries.iter().map(|c| c.as_str()).collect();
                assert_eq!(names, ["GBR", "JPN", "NATO"]);
            }
            other => panic!("expected 3-country Acknowledged([GBR, JPN, NATO]), got {other:?}"),
        }

        // Case 2 (2-letter EU exception per ISMCAT
        // CVEnumISMCATRelTo): bare `FGI EU` admits.
        match parse_fgi_marker("FGI EU") {
            Some(FgiMarker::Acknowledged { countries, .. }) => {
                assert_eq!(countries.len(), 1);
                assert_eq!(countries[0].as_str(), "EU");
            }
            other => panic!("expected Acknowledged([EU]), got {other:?}"),
        }

        // Case 2 (registry-unrecognized but shape-admissible
        // tetragraph): `ABCD` admits at the shape gate; rule layer
        // is responsible for flagging registry membership. Same
        // policy as `XYZ` for trigraphs (see
        // `fgi_marker_unregistered_trigraph_shape_admits_but_marker_records_it`).
        match parse_fgi_marker("FGI ABCD") {
            Some(FgiMarker::Acknowledged { countries, .. }) => {
                assert_eq!(countries.len(), 1);
                assert_eq!(countries[0].as_str(), "ABCD");
            }
            other => panic!("expected Acknowledged([ABCD]), got {other:?}"),
        }

        // Case 3: empty input → None
        assert!(parse_fgi_marker("").is_none());

        // Case 3: lowercase token → None (FR-016 closure)
        assert!(parse_fgi_marker("FGI deu").is_none());
        assert!(parse_fgi_marker("FGI nato").is_none());

        // Case 3: 5+-byte token rejects (out-of-scope of
        // `admits_country_token`; the §H.7 "exception is granted"
        // surface for AUSTRALIA_GROUP-class codes is not handled
        // at this gate).
        assert!(parse_fgi_marker("FGI USAGB").is_none());
        assert!(parse_fgi_marker("FGI AUSTRALIA_GROUP").is_none());

        // Case 3: trailing whitespace with no tokens → None
        assert!(parse_fgi_marker("FGI ").is_none());

        // Case 3: malformed prefix → None
        assert!(parse_fgi_marker("foo FGI USA").is_none());
        assert!(parse_fgi_marker("FGIDEU").is_none()); // no separator

        // Case 3: digits in any list-token slot → None
        assert!(parse_fgi_marker("FGI US1").is_none());
        assert!(parse_fgi_marker("FGI 123").is_none());
        assert!(parse_fgi_marker("FGI NAT0").is_none()); // 0 not O
    }

    #[test]
    fn fgi_marker_double_space_tolerated() {
        // CAPCO §A.6 p16 specifies "single space" as the canonical
        // separator, but `split_whitespace` in the parser
        // tolerates multi-space and tab between tokens. A separate
        // style rule (S###) can flag the non-canonical separator
        // if the project ever wants one; the parser's job is
        // admission, not style enforcement. Pin the tolerance so
        // a future split-on-single-space rewrite is forced to
        // notice this contract.
        match parse_fgi_marker("FGI  USA") {
            Some(FgiMarker::Acknowledged { countries, .. }) => {
                assert_eq!(countries.len(), 1);
                assert_eq!(countries[0].as_str(), "USA");
            }
            other => panic!("expected Acknowledged([USA]) for double-space input, got {other:?}"),
        }
    }

    #[test]
    fn conflict_us_and_nato() {
        let parsed = parse_banner("SECRET//NATO SECRET//NOFORN");
        match &parsed.attrs.classification {
            Some(MarkingClassification::Conflict { us, foreign }) => {
                assert_eq!(*us, Classification::Secret);
                assert!(matches!(
                    foreign.as_ref(),
                    ForeignClassification::Nato(NatoClassification::NatoSecret)
                ));
            }
            other => panic!("expected Conflict, got: {other:?}"),
        }
    }

    #[test]
    fn conflict_level_escalation() {
        // SECRET + COSMIC TOP SECRET → US escalates to TopSecret
        let parsed = parse_banner("SECRET//COSMIC TOP SECRET//NOFORN");
        match &parsed.attrs.classification {
            Some(MarkingClassification::Conflict { us, foreign }) => {
                assert_eq!(*us, Classification::TopSecret);
                assert!(matches!(
                    foreign.as_ref(),
                    ForeignClassification::Nato(NatoClassification::CosmicTopSecret)
                ));
            }
            other => panic!("expected Conflict with escalation, got: {other:?}"),
        }
    }

    #[test]
    fn restricted_classification_parses() {
        let parsed = parse_banner("RESTRICTED//NF");
        assert_eq!(
            parsed.attrs.classification,
            Some(MarkingClassification::Us(Classification::Restricted)),
        );
    }

    #[test]
    fn restricted_portion_parses() {
        let parsed = parse_portion("(R//NF)");
        assert_eq!(
            parsed.attrs.classification,
            Some(MarkingClassification::Us(Classification::Restricted)),
        );
    }

    // -----------------------------------------------------------------------
    // Non-IC dissemination controls
    // -----------------------------------------------------------------------

    #[test]
    fn non_ic_dissem_limdis_banner_form() {
        let parsed = parse_banner("UNCLASSIFIED//LIMDIS");
        assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);
        assert_eq!(parsed.attrs.non_ic_dissem[0], NonIcDissem::Limdis,);
    }

    #[test]
    fn non_ic_dissem_ds_portion_form() {
        let parsed = parse_portion("(U//DS)");
        assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);
        assert_eq!(parsed.attrs.non_ic_dissem[0], NonIcDissem::Limdis);
    }

    #[test]
    fn non_ic_dissem_les_nf() {
        let parsed = parse_portion("(U//LES-NF)");
        assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);
        assert_eq!(parsed.attrs.non_ic_dissem[0], NonIcDissem::LesNf);
        assert!(parsed.attrs.non_ic_dissem[0].carries_noforn());
    }

    #[test]
    fn non_ic_dissem_sbu_nf_banner() {
        let parsed = parse_banner("UNCLASSIFIED//SBU NOFORN");
        assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);
        assert_eq!(parsed.attrs.non_ic_dissem[0], NonIcDissem::SbuNf);
    }

    #[test]
    fn non_ic_dissem_not_confused_with_ic_dissem() {
        // SSI should be non-IC, not IC.
        let parsed = parse_portion("(U//SSI)");
        assert!(parsed.attrs.dissem_controls.is_empty());
        assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);
        assert_eq!(parsed.attrs.non_ic_dissem[0], NonIcDissem::Ssi);
    }

    #[test]
    fn non_ic_dissem_alongside_ic_dissem() {
        // Classified portion with both IC and non-IC dissem.
        let parsed = parse_portion("(C//NF//DS)");
        assert_eq!(parsed.attrs.dissem_controls.len(), 1); // NF
        assert_eq!(parsed.attrs.non_ic_dissem.len(), 1); // DS = LIMDIS
    }

    // -----------------------------------------------------------------------
    // Atomic Energy Act markings
    // -----------------------------------------------------------------------

    #[test]
    fn aea_rd_parses() {
        let parsed = parse_banner("TOP SECRET//RD//NOFORN");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        assert_eq!(
            parsed.attrs.aea_markings[0],
            AeaMarking::Rd(marque_ism::RdBlock::default()),
        );
    }

    #[test]
    fn aea_rd_cnwdi_compound() {
        // CNWDI is a hyphen-modifier of RD, not a separate // block.
        let parsed = parse_banner("SECRET//RD-CNWDI//NOFORN");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        match &parsed.attrs.aea_markings[0] {
            AeaMarking::Rd(rd) => {
                assert!(rd.cnwdi);
                assert!(rd.sigma.is_empty());
            }
            other => panic!("expected Rd with CNWDI, got: {other:?}"),
        }
    }

    #[test]
    fn aea_rd_sigma_compound() {
        // SIGMA is a hyphen-modifier: RD-SIGMA 20
        let parsed = parse_banner("SECRET//RD-SIGMA 20//NOFORN");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        match &parsed.attrs.aea_markings[0] {
            AeaMarking::Rd(rd) => {
                assert!(!rd.cnwdi);
                assert_eq!(&*rd.sigma, &[20]);
            }
            other => panic!("expected Rd with SIGMA, got: {other:?}"),
        }
    }

    #[test]
    fn aea_rd_cnwdi_sigma_compound() {
        let parsed = parse_banner("SECRET//RD-CNWDI-SIGMA 18 20//NOFORN");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        match &parsed.attrs.aea_markings[0] {
            AeaMarking::Rd(rd) => {
                assert!(rd.cnwdi);
                assert_eq!(&*rd.sigma, &[18, 20]);
            }
            other => panic!("expected Rd with CNWDI+SIGMA, got: {other:?}"),
        }
    }

    #[test]
    fn aea_rd_sigma_portion() {
        // Portion form uses SG instead of SIGMA.
        let parsed = parse_portion("(TS//RD-SG 14//NF)");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        match &parsed.attrs.aea_markings[0] {
            AeaMarking::Rd(rd) => {
                assert_eq!(&*rd.sigma, &[14]);
            }
            other => panic!("expected Rd with SG, got: {other:?}"),
        }
    }

    #[test]
    fn aea_frd_parses() {
        let parsed = parse_portion("(S//FRD//NF)");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        assert_eq!(
            parsed.attrs.aea_markings[0],
            AeaMarking::Frd(marque_ism::FrdBlock::default()),
        );
    }

    #[test]
    fn aea_frd_sigma_compound() {
        let parsed = parse_banner("SECRET//FRD-SIGMA 14//NOFORN");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        match &parsed.attrs.aea_markings[0] {
            AeaMarking::Frd(frd) => {
                assert_eq!(&*frd.sigma, &[14]);
            }
            other => panic!("expected Frd with SIGMA, got: {other:?}"),
        }
    }

    #[test]
    fn aea_dod_ucni_parses() {
        let parsed = parse_banner("UNCLASSIFIED//DOD UCNI");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        assert_eq!(parsed.attrs.aea_markings[0], AeaMarking::DodUcni);
    }

    #[test]
    fn aea_dcni_portion_parses() {
        let parsed = parse_portion("(U//DCNI)");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        assert_eq!(parsed.attrs.aea_markings[0], AeaMarking::DodUcni);
    }

    #[test]
    fn aea_tfni_parses() {
        let parsed = parse_banner("SECRET//TFNI//NOFORN");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        assert_eq!(parsed.attrs.aea_markings[0], AeaMarking::Tfni);
    }

    #[test]
    fn aea_rd_n_shorthand() {
        // DoD shorthand: RD-N means RD-CNWDI
        let parsed = parse_portion("(S//RD-N//NF)");
        assert_eq!(parsed.attrs.aea_markings.len(), 1);
        match &parsed.attrs.aea_markings[0] {
            AeaMarking::Rd(rd) => assert!(rd.cnwdi),
            other => panic!("expected Rd with CNWDI from RD-N, got: {other:?}"),
        }
    }

    // --- CAPCO §D.1 intra-block `/` separator ---

    #[test]
    fn slash_separated_sci_in_single_block_parses() {
        // CAPCO §D.1: multiple SCI controls in one block, `/`-separated.
        // "(TS//SI/TK//NF)" must produce sci_controls: [Si, Tk], NOT Unknown.
        use marque_ism::SciControl;
        let parsed = parse_portion("(TS//SI/TK//NF)");
        assert_eq!(
            parsed.attrs.sci_controls.as_ref(),
            &[SciControl::Si, SciControl::Tk],
            "SI/TK block must yield two SCI controls"
        );
        // No Unknown token spans
        assert!(
            parsed
                .attrs
                .token_spans
                .iter()
                .all(|t| t.kind != TokenKind::Unknown),
            "no Unknown spans expected: {:?}",
            parsed.attrs.token_spans
        );
    }

    #[test]
    fn slash_separated_sci_banner_parses() {
        // Same rule applies to banner markings.
        use marque_ism::SciControl;
        let parsed = parse_banner("TOP SECRET//SI/TK//NOFORN");
        assert_eq!(
            parsed.attrs.sci_controls.as_ref(),
            &[SciControl::Si, SciControl::Tk],
        );
    }

    #[test]
    fn slash_separated_dissem_in_single_block_parses() {
        // Dissem controls can also share a block: "NF/RD" in one // block.
        use marque_ism::DissemControl;
        let parsed = parse_banner("SECRET//SI//NF/RELIDO");
        let dissem: Vec<DissemControl> = parsed.attrs.dissem_controls.to_vec();
        assert!(dissem.contains(&DissemControl::Nf), "must contain NF");
        assert!(
            dissem.contains(&DissemControl::Relido),
            "must contain RELIDO"
        );
    }

    #[test]
    fn unrecognized_slash_token_emits_unknown() {
        // An unknown token like "XYZZY" in a slash block → Unknown span.
        let parsed = parse_portion("(S//XYZZY)");
        assert!(
            parsed
                .attrs
                .token_spans
                .iter()
                .any(|t| t.kind == TokenKind::Unknown),
            "XYZZY must produce Unknown span"
        );
    }

    // -----------------------------------------------------------------------
    // SCI structural subparser (spec 003-sci-compartments §R2 / P2)
    // -----------------------------------------------------------------------

    #[test]
    fn sci_bare_single_still_parses_via_structural_path() {
        // Regression: `(U//SI//NF)` existing happy path. Structural parser
        // claims `SI` (bare CVE) and projects to sci_controls for
        // back-compat with E010/E011.
        use marque_ism::{SciControl, SciControlBare, SciControlSystem};
        let parsed = parse_portion("(U//SI//NF)");
        assert_eq!(parsed.attrs.sci_controls.as_ref(), &[SciControl::Si]);
        assert_eq!(parsed.attrs.sci_markings.len(), 1);
        let m = &parsed.attrs.sci_markings[0];
        assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Si));
        assert!(m.compartments.is_empty());
        assert_eq!(m.canonical_enum, Some(SciControl::Si));
    }

    #[test]
    fn sci_published_compound_si_g_parses() {
        // `SI-G` is a pre-registered CVE composite; canonical_enum must be Some(SiG).
        use marque_ism::{SciControl, SciControlBare, SciControlSystem};
        let parsed = parse_banner("SECRET//SI-G//NOFORN");
        let m = &parsed.attrs.sci_markings[0];
        assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Si));
        assert_eq!(m.compartments.len(), 1);
        assert_eq!(m.compartments[0].identifier.as_ref(), "G");
        assert!(m.compartments[0].sub_compartments.is_empty());
        assert_eq!(m.canonical_enum, Some(SciControl::SiG));
        assert_eq!(parsed.attrs.sci_controls.as_ref(), &[SciControl::SiG]);
    }

    #[test]
    fn sci_published_compound_hcs_p_parses() {
        use marque_ism::{SciControl, SciControlBare, SciControlSystem};
        let parsed = parse_banner("TOP SECRET//HCS-P//NOFORN");
        let m = &parsed.attrs.sci_markings[0];
        assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Hcs));
        assert_eq!(m.compartments[0].identifier.as_ref(), "P");
        assert_eq!(m.canonical_enum, Some(SciControl::HcsP));
    }

    #[test]
    fn sci_bare_tk_parses() {
        use marque_ism::{SciControl, SciControlBare, SciControlSystem};
        let parsed = parse_banner("SECRET//TK//NOFORN");
        let m = &parsed.attrs.sci_markings[0];
        assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Tk));
        assert!(m.compartments.is_empty());
        assert_eq!(m.canonical_enum, Some(SciControl::Tk));
    }

    #[test]
    fn sci_multi_system_si_tk_parses() {
        // `SI/TK` — two bare systems in one SCI block. Existing behavior.
        use marque_ism::SciControl;
        let parsed = parse_portion("(TS//SI/TK//NF)");
        assert_eq!(
            parsed.attrs.sci_controls.as_ref(),
            &[SciControl::Si, SciControl::Tk]
        );
        assert_eq!(parsed.attrs.sci_markings.len(), 2);
    }

    #[test]
    fn sci_compound_with_sub_compartment_sets_canonical_none() {
        // `SI-G ABCD`: published system SI with compartment G and sub-comp
        // ABCD. Because the first compartment has sub-comps, canonical_enum
        // is None (the compound is a structural anchor, not an atomic CVE).
        use marque_ism::{SciControlBare, SciControlSystem};
        let parsed = parse_banner("SECRET//SI-G ABCD//NOFORN");
        assert_eq!(parsed.attrs.sci_markings.len(), 1);
        let m = &parsed.attrs.sci_markings[0];
        assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Si));
        assert_eq!(m.compartments.len(), 1);
        assert_eq!(m.compartments[0].identifier.as_ref(), "G");
        assert_eq!(m.compartments[0].sub_compartments.len(), 1);
        assert_eq!(m.compartments[0].sub_compartments[0].as_ref(), "ABCD");
        assert_eq!(m.canonical_enum, None);
        // sci_controls projection: no canonical_enum → no entry
        assert!(parsed.attrs.sci_controls.is_empty());
    }

    #[test]
    fn sci_capco_canonical_example_parses() {
        // CAPCO-2016 §A.6 p16 canonical example:
        //   TOP SECRET//123/SI-G ABCD DEFG-MMM AACD//ORCON/NOFORN
        use marque_ism::{SciControlBare, SciControlSystem};
        let parsed = parse_banner("TOP SECRET//123/SI-G ABCD DEFG-MMM AACD//ORCON/NOFORN");
        assert_eq!(parsed.attrs.sci_markings.len(), 2);
        // Marking 0: Custom("123"), no compartments.
        let m0 = &parsed.attrs.sci_markings[0];
        assert!(matches!(&m0.system, SciControlSystem::Custom(s) if s.as_ref() == "123"));
        assert!(m0.compartments.is_empty());
        assert_eq!(m0.canonical_enum, None);
        // Marking 1: Published(SI) with compartments G[ABCD, DEFG] and MMM[AACD].
        let m1 = &parsed.attrs.sci_markings[1];
        assert_eq!(m1.system, SciControlSystem::Published(SciControlBare::Si));
        assert_eq!(m1.compartments.len(), 2);
        assert_eq!(m1.compartments[0].identifier.as_ref(), "G");
        assert_eq!(m1.compartments[0].sub_compartments.len(), 2);
        assert_eq!(m1.compartments[0].sub_compartments[0].as_ref(), "ABCD");
        assert_eq!(m1.compartments[0].sub_compartments[1].as_ref(), "DEFG");
        assert_eq!(m1.compartments[1].identifier.as_ref(), "MMM");
        assert_eq!(m1.compartments[1].sub_compartments.len(), 1);
        assert_eq!(m1.compartments[1].sub_compartments[0].as_ref(), "AACD");
        // First compartment has sub-comps → canonical_enum is None.
        assert_eq!(m1.canonical_enum, None);
        // No Unknown spans in the SCI block.
        let sci_block_has_unknown = parsed
            .attrs
            .token_spans
            .iter()
            .any(|t| t.kind == TokenKind::Unknown);
        assert!(
            !sci_block_has_unknown,
            "canonical example must not produce Unknown tokens; got: {:?}",
            parsed.attrs.token_spans
        );
    }

    #[test]
    fn sci_custom_numeric_99_direct_parse() {
        // Direct unit test of parse_sci_block: `99` → Custom("99").
        // In dispatch, `99` alone wouldn't pass the containment gate; this
        // exercises the parser's custom-only happy path.
        use marque_ism::SciControlSystem;
        let mut tokens = Vec::new();
        let result = parse_sci_block("99", 0, &mut tokens).expect("99 must parse");
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0].system, SciControlSystem::Custom(s) if s.as_ref() == "99"));
        assert!(result[0].compartments.is_empty());
        assert_eq!(result[0].canonical_enum, None);
    }

    #[test]
    fn sci_structural_rejections_return_none() {
        // Dangling hyphen.
        let mut tokens = Vec::new();
        assert!(parse_sci_block("SI-", 0, &mut tokens).is_none());
        // Leading hyphen.
        let mut tokens = Vec::new();
        assert!(parse_sci_block("-SI", 0, &mut tokens).is_none());
        // Empty.
        let mut tokens = Vec::new();
        assert!(parse_sci_block("", 0, &mut tokens).is_none());
        // Lowercase.
        let mut tokens = Vec::new();
        assert!(parse_sci_block("si-g", 0, &mut tokens).is_none());
        // Consecutive hyphens.
        let mut tokens = Vec::new();
        assert!(parse_sci_block("SI--G", 0, &mut tokens).is_none());
        // Empty slash chunk.
        let mut tokens = Vec::new();
        assert!(parse_sci_block("SI/", 0, &mut tokens).is_none());
    }

    #[test]
    fn sci_mixed_category_slash_block_falls_through() {
        // `SI/NF` has `/` and gate passes, but parse_sci_block must reject
        // because NF is a known dissem control — otherwise E004's
        // stray-slash detection would stop working.
        let parsed = parse_banner("SECRET//SI/NF");
        // The SI/NF block should NOT be claimed by structural SCI; it must
        // fall through to the existing intra-block `/` splitter which in
        // turn flags the mixed-category slash as Unknown.
        let has_unknown_block = parsed
            .attrs
            .token_spans
            .iter()
            .any(|t| t.kind == TokenKind::Unknown);
        assert!(
            has_unknown_block,
            "SI/NF must surface as Unknown for E004; got: {:?}",
            parsed.attrs.token_spans
        );
    }

    #[test]
    fn sci_weird_sub_compartment_parses() {
        // `SI-G WEIRD FOO` — WEIRD and FOO both match [A-Z0-9]+ so the
        // grammar treats them as sub-compartments of G.
        use marque_ism::{SciControlBare, SciControlSystem};
        let parsed = parse_banner("SECRET//SI-G WEIRD FOO//NOFORN");
        let m = &parsed.attrs.sci_markings[0];
        assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Si));
        assert_eq!(m.compartments.len(), 1);
        assert_eq!(m.compartments[0].identifier.as_ref(), "G");
        assert_eq!(m.compartments[0].sub_compartments.len(), 2);
        assert_eq!(m.compartments[0].sub_compartments[0].as_ref(), "WEIRD");
        assert_eq!(m.compartments[0].sub_compartments[1].as_ref(), "FOO");
    }

    // -----------------------------------------------------------------------
    // CAB date parsing (parse_cab Declassify On: path)
    // -----------------------------------------------------------------------

    fn parse_cab_text(text: &str) -> CanonicalParsed {
        let source = text.as_bytes();
        let tokens = CapcoTokenSet;
        let parser = Parser::new(&tokens);
        let candidate = make_candidate(source, MarkingType::Cab, 0);
        parser
            .parse(&candidate, source)
            .expect("CAB parse should succeed")
            .into()
    }

    #[test]
    fn cab_declassify_on_yyyymmdd_populates_declassify_on() {
        let text = "Classified By: Jane Doe\nDeclassify On: 20301231";
        let parsed = parse_cab_text(text);
        assert_eq!(
            parsed.attrs.declassify_on,
            Some(marque_ism::IsmDate::Date(2030, 12, 31)),
            "YYYYMMDD in CAB should set declassify_on to Date"
        );
        assert!(parsed.attrs.declass_exemption.is_none());
    }

    #[test]
    fn cab_declassify_on_yyyy_populates_declassify_on() {
        let text = "Declassify On: 2035";
        let parsed = parse_cab_text(text);
        assert_eq!(
            parsed.attrs.declassify_on,
            Some(marque_ism::IsmDate::Year(2035)),
            "YYYY in CAB should set declassify_on to Year"
        );
    }

    #[test]
    fn cab_declassify_on_iso_date_populates_declassify_on() {
        // ISO hyphenated YYYY-MM-DD form is valid for the CAB "Declassify On:" line.
        let text = "Declassify On: 2030-12-31";
        let parsed = parse_cab_text(text);
        assert_eq!(
            parsed.attrs.declassify_on,
            Some(marque_ism::IsmDate::Date(2030, 12, 31)),
            "YYYY-MM-DD in CAB should set declassify_on to Date"
        );
    }

    #[test]
    fn cab_declassify_on_exemption_sets_exemption_not_date() {
        // A declassification exemption code must not be stored in declassify_on.
        let text = "Declassify On: 50X1-HUM";
        let parsed = parse_cab_text(text);
        assert!(
            parsed.attrs.declassify_on.is_none(),
            "exemption code must not set declassify_on"
        );
        assert!(
            parsed.attrs.declass_exemption.is_some(),
            "exemption code must set declass_exemption"
        );
    }

    #[test]
    fn cab_declassify_on_invalid_date_silently_ignored() {
        // Unrecognized strings are silently dropped — no panic, declassify_on stays None.
        let text = "Declassify On: UNRECOGNIZED";
        let parsed = parse_cab_text(text);
        assert!(
            parsed.attrs.declassify_on.is_none(),
            "unrecognized Declassify On value should leave declassify_on as None"
        );
        assert!(parsed.attrs.declass_exemption.is_none());
    }

    #[test]
    fn cab_classified_by_and_derived_from_populated() {
        let text = "Classified By: Jane Doe\nDerived From: SCG-2024\nDeclassify On: 20301231";
        let parsed = parse_cab_text(text);
        assert_eq!(
            parsed.attrs.classified_by.as_deref(),
            Some("Jane Doe"),
            "classified_by should be populated"
        );
        assert_eq!(
            parsed.attrs.derived_from.as_deref(),
            Some("SCG-2024"),
            "derived_from should be populated"
        );
        assert_eq!(
            parsed.attrs.declassify_on,
            Some(marque_ism::IsmDate::Date(2030, 12, 31))
        );
    }

    #[test]
    fn cab_without_declassify_on_leaves_both_none() {
        let text = "Classified By: Jane Doe\nDerived From: SCG-2024";
        let parsed = parse_cab_text(text);
        assert!(parsed.attrs.declassify_on.is_none());
        assert!(parsed.attrs.declass_exemption.is_none());
    }

    // -----------------------------------------------------------------------
    // Portion declass date (is_declass_date path in parse_marking_string)
    // -----------------------------------------------------------------------

    #[test]
    fn portion_with_yyyymmdd_sets_declassify_on() {
        // A portion that (erroneously) contains an inline declass date; the
        // parser must populate declassify_on so E005 can fire.
        let parsed = parse_portion("(SECRET//20301231//NOFORN)");
        assert_eq!(
            parsed.attrs.declassify_on,
            Some(marque_ism::IsmDate::Date(2030, 12, 31)),
            "YYYYMMDD in portion should set declassify_on"
        );
    }

    #[test]
    fn portion_with_yyyy_sets_declassify_on() {
        let parsed = parse_portion("(SECRET//2035)");
        assert_eq!(
            parsed.attrs.declassify_on,
            Some(marque_ism::IsmDate::Year(2035)),
            "YYYY in portion should set declassify_on"
        );
    }

    #[test]
    fn is_declass_date_rejects_leap_day_non_leap_year() {
        // 2003 is not a leap year; Feb 29 is impossible.
        assert!(!is_declass_date("20030229"));
    }

    #[test]
    fn is_declass_date_accepts_leap_day_in_leap_year() {
        assert!(is_declass_date("20040229")); // 2004 is a leap year
        assert!(is_declass_date("20000229")); // 2000 is a leap year
    }

    #[test]
    fn is_declass_date_rejects_day_zero() {
        assert!(!is_declass_date("20030100")); // day 0 is impossible
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod sar_parse_tests {
    //! Direct unit tests for [`parse_sar_category`] plus integration-level
    //! tests that exercise the dispatch from `parse_marking_string`.

    use super::tests::CanonicalParsed;
    use super::*;
    use marque_ism::span::{MarkingCandidate, MarkingType, Span};
    use marque_ism::token_set::CapcoTokenSet;

    // ---------------------------------------------------------------------
    // Direct subparser tests
    // ---------------------------------------------------------------------

    #[test]
    fn single_program_no_compartments() {
        let (marking, spans) = parse_sar_category("SAR-BP", 0).expect("grammar accepts SAR-BP");
        assert_eq!(marking.indicator, SarIndicator::Abbrev);
        assert_eq!(marking.programs.len(), 1);
        assert_eq!(&*marking.programs[0].identifier, "BP");
        assert_eq!(marking.programs[0].compartments.len(), 0);
        // Spans: one indicator + one program.
        assert_eq!(
            spans
                .iter()
                .filter(|s| s.kind == TokenKind::SarIndicator)
                .count(),
            1
        );
        assert_eq!(
            spans
                .iter()
                .filter(|s| s.kind == TokenKind::SarProgram)
                .count(),
            1
        );
    }

    #[test]
    fn three_programs_no_compartments() {
        let (marking, _) =
            parse_sar_category("SAR-BP/CD/XR", 0).expect("grammar accepts three programs");
        assert_eq!(marking.programs.len(), 3);
        let ids: Vec<&str> = marking.programs.iter().map(|p| &*p.identifier).collect();
        assert_eq!(ids, vec!["BP", "CD", "XR"]);
        for p in marking.programs.iter() {
            assert_eq!(p.compartments.len(), 0);
        }
    }

    #[test]
    fn program_with_single_compartment() {
        let (marking, _) = parse_sar_category("SAR-BP-J12", 0).expect("grammar accepts");
        assert_eq!(marking.programs.len(), 1);
        let p = &marking.programs[0];
        assert_eq!(&*p.identifier, "BP");
        assert_eq!(p.compartments.len(), 1);
        assert_eq!(&*p.compartments[0].identifier, "J12");
        assert_eq!(p.compartments[0].sub_compartments.len(), 0);
    }

    #[test]
    fn program_with_compartment_and_sub_compartment() {
        let (marking, _) = parse_sar_category("SAR-BP-J12 J54", 0).expect("grammar accepts");
        let p = &marking.programs[0];
        assert_eq!(p.compartments.len(), 1);
        let c = &p.compartments[0];
        assert_eq!(&*c.identifier, "J12");
        assert_eq!(c.sub_compartments.len(), 1);
        assert_eq!(&*c.sub_compartments[0], "J54");
    }

    #[test]
    fn canonical_h5_p100_multi_program_example() {
        // The §H.5 p100 canonical decomposition:
        //   BP → [J12 (+ J54), K15]
        //   CD → [YYY (+ 456, 689)]
        //   XR → [XRA (+ RB)]
        let block = "SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB";
        let (marking, spans) = parse_sar_category(block, 0).expect("grammar accepts");

        assert_eq!(marking.indicator, SarIndicator::Abbrev);
        assert_eq!(marking.programs.len(), 3);

        // BP
        let bp = &marking.programs[0];
        assert_eq!(&*bp.identifier, "BP");
        assert_eq!(bp.compartments.len(), 2);
        assert_eq!(&*bp.compartments[0].identifier, "J12");
        assert_eq!(
            bp.compartments[0]
                .sub_compartments
                .iter()
                .map(|s| &**s)
                .collect::<Vec<_>>(),
            vec!["J54"]
        );
        assert_eq!(&*bp.compartments[1].identifier, "K15");
        assert_eq!(bp.compartments[1].sub_compartments.len(), 0);

        // CD
        let cd = &marking.programs[1];
        assert_eq!(&*cd.identifier, "CD");
        assert_eq!(cd.compartments.len(), 1);
        assert_eq!(&*cd.compartments[0].identifier, "YYY");
        assert_eq!(
            cd.compartments[0]
                .sub_compartments
                .iter()
                .map(|s| &**s)
                .collect::<Vec<_>>(),
            vec!["456", "689"]
        );

        // XR
        let xr = &marking.programs[2];
        assert_eq!(&*xr.identifier, "XR");
        assert_eq!(xr.compartments.len(), 1);
        assert_eq!(&*xr.compartments[0].identifier, "XRA");
        assert_eq!(
            xr.compartments[0]
                .sub_compartments
                .iter()
                .map(|s| &**s)
                .collect::<Vec<_>>(),
            vec!["RB"]
        );

        // Spot-check span offsets: the indicator is at [0, 4) and the first
        // program "BP" is at [4, 6).
        let indicator = spans
            .iter()
            .find(|s| s.kind == TokenKind::SarIndicator)
            .unwrap();
        assert_eq!(indicator.span, Span::new(0, 4));
        assert_eq!(&*indicator.text, "SAR-");
        let first_prog = spans
            .iter()
            .find(|s| s.kind == TokenKind::SarProgram)
            .unwrap();
        assert_eq!(first_prog.span, Span::new(4, 6));
        assert_eq!(&*first_prog.text, "BP");
    }

    #[test]
    fn full_form_single_program_with_space() {
        // `SPECIAL ACCESS REQUIRED-BUTTER POPCORN` — full form allows spaces
        // inside the nickname. No compartment decomposition at the lexical
        // level (see spec §R2 ambiguity note).
        let (marking, spans) =
            parse_sar_category("SPECIAL ACCESS REQUIRED-BUTTER POPCORN", 0).unwrap();
        assert_eq!(marking.indicator, SarIndicator::Full);
        assert_eq!(marking.programs.len(), 1);
        assert_eq!(&*marking.programs[0].identifier, "BUTTER POPCORN");
        assert_eq!(marking.programs[0].compartments.len(), 0);

        // Indicator span is 24 bytes: `SPECIAL ACCESS REQUIRED-`.
        let indicator = spans
            .iter()
            .find(|s| s.kind == TokenKind::SarIndicator)
            .unwrap();
        assert_eq!(&*indicator.text, "SPECIAL ACCESS REQUIRED-");
        assert_eq!(indicator.span, Span::new(0, 24));
    }

    #[test]
    fn full_form_with_compartment_and_sub() {
        // The grammar permits compartments under a full-form program
        // identically to the abbreviated form. Program nickname may
        // contain spaces; compartments and sub-compartments are still
        // alphanumeric without spaces.
        let (marking, _spans) =
            parse_sar_category("SPECIAL ACCESS REQUIRED-BUTTER POPCORN-J12 J54", 0)
                .expect("grammar accepts full form with compartment");
        assert_eq!(marking.indicator, SarIndicator::Full);
        assert_eq!(marking.programs.len(), 1);
        let prog = &marking.programs[0];
        assert_eq!(&*prog.identifier, "BUTTER POPCORN");
        assert_eq!(prog.compartments.len(), 1);
        assert_eq!(&*prog.compartments[0].identifier, "J12");
        assert_eq!(prog.compartments[0].sub_compartments.len(), 1);
        assert_eq!(&*prog.compartments[0].sub_compartments[0], "J54");
    }

    #[test]
    fn full_form_rejects_digits_or_hyphens_in_nickname() {
        // Full-form nickname may only contain uppercase letters and
        // spaces; digits or hyphens inside the nickname are parsed as
        // compartment boundaries (hyphen) or as a shape violation
        // (digits).
        assert!(parse_sar_category("SPECIAL ACCESS REQUIRED-123", 0).is_none());
    }

    #[test]
    fn rejects_double_slash_inside_block() {
        // Defensive: the outer category-block splitter wouldn't hand us
        // `SAR-BP//CD` (it splits on `//` first). But if it somehow did,
        // `parse_sar_category` refuses because `//` is a category separator
        // that should never appear inside a single block. The caller
        // records the text as Unknown so E030 can flag the repeat form.
        assert!(parse_sar_category("SAR-BP//CD", 0).is_none());
    }

    #[test]
    fn rejects_missing_hyphen() {
        assert!(parse_sar_category("SAR", 0).is_none());
    }

    #[test]
    fn rejects_empty_program() {
        assert!(parse_sar_category("SAR-", 0).is_none());
    }

    #[test]
    fn rejects_empty_string() {
        assert!(parse_sar_category("", 0).is_none());
    }

    #[test]
    fn rejects_non_sar_prefix() {
        assert!(parse_sar_category("NOFORN", 0).is_none());
        assert!(parse_sar_category("SI", 0).is_none());
    }

    #[test]
    fn rejects_program_id_out_of_2_3_length() {
        // Single-char program id.
        assert!(parse_sar_category("SAR-B", 0).is_none());
        // Four-char program id.
        assert!(parse_sar_category("SAR-BPCD", 0).is_none());
    }

    // ---------------------------------------------------------------------
    // T089 / T090 / T091: FR-015 closure for parse_sar_program
    //
    // The parser-side admission for SAR program identifiers,
    // compartments, and sub-compartments routes through the
    // `marque-ism` predicates `SarProgram::admits_program_id_abbrev`,
    // `SarProgram::admits_program_id_full`, and
    // `SarCompartment::admits_identifier`. These tests pin the
    // accept/reject boundary at the parser dispatch level —
    // catching any future drift between the parser and the
    // single-source-of-truth predicates in `marque-ism::attrs`.
    // The predicates' own accept/reject sets are exhaustively
    // tested in `marque_ism::attrs::sar_shape_tests`; these tests
    // verify the parser actually calls them.
    // ---------------------------------------------------------------------

    #[test]
    fn t089_program_id_abbrev_length_boundary() {
        // FR-015 / T089 regression. The 2-3 alnum gate is the
        // most observable boundary; if the parser ever falls back
        // to a length-only or class-only check (a pre-T089 bug
        // mode), one of these assertions will fail.

        // Length 1 (below the 2-char minimum) — must reject.
        assert!(
            parse_sar_category("SAR-X", 0).is_none(),
            "single-char program id must reject (below the §H.5 p101 \
             2-3 char bound)",
        );

        // Length 2 (the lower bound) — must accept and produce a
        // single program with the abbreviated identifier.
        let (marking, _spans) = parse_sar_category("SAR-XY", 0)
            .expect("2-char abbrev program id must accept (§H.5 p101 lower bound)");
        assert_eq!(marking.indicator, SarIndicator::Abbrev);
        assert_eq!(marking.programs.len(), 1);
        assert_eq!(&*marking.programs[0].identifier, "XY");

        // Length 3 (the upper bound) — must accept.
        let (marking, _spans) =
            parse_sar_category("SAR-XYZ", 0).expect("3-char abbrev program id must accept");
        assert_eq!(&*marking.programs[0].identifier, "XYZ");

        // Length 4 (above the 3-char maximum) — must reject.
        assert!(
            parse_sar_category("SAR-XYZW", 0).is_none(),
            "4-char program id must reject (above the §H.5 p101 \
             2-3 char bound)",
        );

        // Lower-case alnum and digits remain admitted by the
        // predicate (style rule, not shape rule), matching the
        // predicate's documented behavior.
        let (marking, _spans) = parse_sar_category("SAR-bp", 0)
            .expect("lowercase abbrev program id must accept (style, not shape)");
        assert_eq!(&*marking.programs[0].identifier, "bp");
        let (marking, _spans) =
            parse_sar_category("SAR-99", 0).expect("digit-only abbrev id must accept");
        assert_eq!(&*marking.programs[0].identifier, "99");
    }

    #[test]
    fn t090_compartment_identifier_admission() {
        // FR-015 / T090 regression. `parse_sar_program` must
        // delegate compartment admission to
        // `SarCompartment::admits_identifier`, not an inline
        // length-and-class check. The accept set is "≥1 ASCII
        // alnum"; the reject set covers the empty-segment and
        // punctuation cases.

        // Empty compartment after the program/compartment hyphen
        // — `SAR-BP-` produces an empty trailing segment that must
        // reject (mirrors `parse_sar_program`'s segment-empty guard
        // even though `admits_identifier(b"")` would also reject).
        assert!(
            parse_sar_category("SAR-BP-", 0).is_none(),
            "trailing hyphen with empty compartment must reject",
        );

        // Single-character compartment — manual silent on lower
        // bound beyond ≥1; marque admits length 1+. Pins the
        // marque interpretation noted in the predicate's doc
        // comment.
        let (marking, _spans) = parse_sar_category("SAR-BP-1", 0)
            .expect("single-char compartment id must accept (marque interpretation of §H.5 p99)");
        assert_eq!(marking.programs.len(), 1);
        assert_eq!(marking.programs[0].compartments.len(), 1);
        assert_eq!(&*marking.programs[0].compartments[0].identifier, "1");

        // Multi-char alnum compartment — Table 7 §H.5 p100 examples.
        let (marking, _spans) =
            parse_sar_category("SAR-BP-J12", 0).expect("alnum compartment id must accept");
        assert_eq!(&*marking.programs[0].compartments[0].identifier, "J12");
    }

    #[test]
    fn t091_sub_compartment_identifier_admission() {
        // FR-015 / T091 regression. Sub-compartment admission goes
        // through the same `SarCompartment::admits_identifier`
        // predicate as the compartment slot — the manual places
        // both grammar positions under one rule
        // (CAPCO-2016 §H.5 pp 99-100).

        // Trailing space with no sub-compartment token — empty
        // sub-compartment must reject. `split_with_offsets(seg, ' ')`
        // produces an empty trailing token; `admits_identifier(b"")`
        // catches it.
        assert!(
            parse_sar_category("SAR-BP-J12 ", 0).is_none(),
            "trailing space with no sub-compartment token must reject",
        );

        // Single-char sub-compartment — admitted by the same
        // length-1+ rule.
        let (marking, _spans) = parse_sar_category("SAR-BP-J12 1", 0)
            .expect("single-char sub-compartment id must accept");
        let comp = &marking.programs[0].compartments[0];
        assert_eq!(comp.sub_compartments.len(), 1);
        assert_eq!(&*comp.sub_compartments[0], "1");

        // Multi-char alnum sub-compartment — Table 7 §H.5 p100.
        let (marking, _spans) =
            parse_sar_category("SAR-BP-J12 J54", 0).expect("alnum sub-compartment id must accept");
        let comp = &marking.programs[0].compartments[0];
        assert_eq!(&*comp.sub_compartments[0], "J54");

        // Punctuation in sub-compartment — must reject. The
        // grammar separators `-`, `/`, and ` ` cannot be tested
        // here: `-` and `/` are consumed at the compartment /
        // program level before sub-compartment admission runs,
        // and ` ` is itself the sub-compartment separator. Any
        // other punctuation byte has no role in §H.5 and reaches
        // `admits_identifier`, where it is rejected.
        assert!(
            parse_sar_category("SAR-BP-J12 J.54", 0).is_none(),
            "punctuation (`.`) in sub-compartment must reject",
        );
        assert!(
            parse_sar_category("SAR-BP-J12 J_54", 0).is_none(),
            "punctuation (`_`) in sub-compartment must reject",
        );
    }

    // ---------------------------------------------------------------------
    // Dispatch tests (through `parse_marking_string`)
    // ---------------------------------------------------------------------

    fn make_banner(text: &str) -> CanonicalParsed {
        let source = text.as_bytes();
        let tokens = CapcoTokenSet;
        let parser = Parser::new(&tokens);
        let candidate = MarkingCandidate {
            span: Span::new(0, source.len()),
            kind: MarkingType::Banner,
        };
        parser
            .parse(&candidate, source)
            .expect("parse succeeds")
            .into()
    }

    #[test]
    fn banner_dispatch_populates_sar_markings() {
        let parsed = make_banner("TOP SECRET//SAR-BP//NOFORN");
        let sar = parsed
            .attrs
            .sar_markings
            .as_ref()
            .expect("SAR block must populate sar_markings");
        assert_eq!(sar.programs.len(), 1);
        assert_eq!(&*sar.programs[0].identifier, "BP");

        // Token-span mix must include both the indicator and program token.
        let kinds: Vec<TokenKind> = parsed.attrs.token_spans.iter().map(|t| t.kind).collect();
        assert!(kinds.contains(&TokenKind::SarIndicator));
        assert!(kinds.contains(&TokenKind::SarProgram));

        // Dissem accumulator still populated: NOFORN is present.
        assert!(
            parsed
                .attrs
                .dissem_controls
                .contains(&marque_ism::DissemControl::Nf),
            "NOFORN must still be recognized after the SAR block"
        );
    }

    #[test]
    fn banner_dispatch_multi_program_canonical() {
        // The §H.5 p100 canonical line as a full banner.
        let parsed = make_banner("SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN");
        let sar = parsed.attrs.sar_markings.as_ref().expect("sar present");
        assert_eq!(sar.programs.len(), 3);
        let ids: Vec<&str> = sar.programs.iter().map(|p| &*p.identifier).collect();
        assert_eq!(ids, vec!["BP", "CD", "XR"]);

        // Token-span offsets are absolute into the banner string. Find the
        // SarIndicator and verify its byte slice.
        let src = parsed
            .attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::SarIndicator)
            .expect("SarIndicator span present");
        assert_eq!(&*src.text, "SAR-");
        // `SECRET//` is 8 bytes, so `SAR-` starts at offset 8.
        assert_eq!(src.span, Span::new(8, 12));
    }

    #[test]
    fn second_sar_block_becomes_unknown() {
        // Two SAR category blocks: the first populates `sar_markings`; the
        // second is left as `Unknown` so rule E030 can flag the repeat.
        let parsed = make_banner("SECRET//SAR-BP//SAR-CD//NOFORN");
        let sar = parsed
            .attrs
            .sar_markings
            .as_ref()
            .expect("first SAR block populates sar_markings");
        assert_eq!(sar.programs.len(), 1);
        assert_eq!(&*sar.programs[0].identifier, "BP");

        // The `SAR-CD` block must appear as an Unknown span.
        let unknown_texts: Vec<&str> = parsed
            .attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Unknown)
            .map(|t| &*t.text)
            .collect();
        assert!(
            unknown_texts.contains(&"SAR-CD"),
            "duplicate SAR block must be recorded as Unknown, got: {unknown_texts:?}",
        );
    }
}
