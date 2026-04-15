//! Phase 2/3: token extraction and structural parsing.
//!
//! Takes [`MarkingCandidate`] spans from the scanner and produces [`IsmAttributes`].
//!
//! # Phase 2 — Token Extraction
//! A compile-time Aho-Corasick automaton (built from CVE token list in marque-capco)
//! runs over each candidate span, identifying known tokens and their positions.
//! Unrecognized tokens within a candidate boundary are themselves diagnostics.
//!
//! # Phase 3 — Structural Parsing
//! Token sequence → IsmAttributes. Validates ordering and block structure.
//! Produces `ParseError` for structural violations; these feed into the rule engine
//! as diagnostics with associated fixes.
//!
//! Note: the Aho-Corasick automaton is injected via `TokenSet` to keep marque-core
//! free of a direct dependency on marque-capco's generated data.

use crate::error::CoreError;
use marque_ism::attrs::{
    AeaMarking, Classification, DeclassExemption, DissemControl, FgiClassification, FgiMarker,
    ForeignClassification, IsmAttributes, JointClassification, MarkingClassification,
    NatoClassification, NonIcDissem, SarCompartment, SarIndicator, SarMarking, SarProgram,
    SciCompartment, SciControl, SciControlBare, SciControlSystem, SciMarking, TokenKind, TokenSpan,
    Trigraph,
};
use marque_ism::is_bare_cve_value;
use marque_ism::span::{MarkingCandidate, MarkingType, Span};
use marque_ism::token_set::TokenSet;

/// Parse result for a single candidate.
#[derive(Debug)]
pub struct ParsedMarking {
    pub attrs: IsmAttributes,
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

    /// Parse a single scanner candidate into [`IsmAttributes`].
    pub fn parse(
        &self,
        candidate: &MarkingCandidate,
        source: &[u8],
    ) -> Result<ParsedMarking, CoreError> {
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

    fn parse_portion(
        &self,
        text: &str,
        candidate: &MarkingCandidate,
    ) -> Result<ParsedMarking, CoreError> {
        // Strip outer parentheses: "(TS//SI//NF)" -> "TS//SI//NF"
        // The inner-string offset is `candidate.span.start + 1` because
        // the leading `(` is one byte (verified ASCII by the scanner).
        let inner = text
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| CoreError::MalformedMarking(text.to_owned()))?;

        let attrs =
            self.parse_marking_string(inner, MarkingType::Portion, candidate.span.start + 1)?;
        Ok(ParsedMarking {
            attrs,
            source_span: candidate.span,
            kind: MarkingType::Portion,
        })
    }

    fn parse_banner(
        &self,
        text: &str,
        candidate: &MarkingCandidate,
    ) -> Result<ParsedMarking, CoreError> {
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
        )?;
        Ok(ParsedMarking {
            attrs,
            source_span: candidate.span,
            kind: MarkingType::Banner,
        })
    }

    fn parse_cab(
        &self,
        text: &str,
        candidate: &MarkingCandidate,
    ) -> Result<ParsedMarking, CoreError> {
        // CAB is line-structured: "Classified By: ...\nDerived From: ...\nDeclassify On: ..."
        let mut attrs = IsmAttributes::default();

        for line in text.lines() {
            if let Some(val) = line.strip_prefix("Classified By:") {
                attrs.classified_by = Some(val.trim().into());
            } else if let Some(val) = line.strip_prefix("Derived From:") {
                attrs.derived_from = Some(val.trim().into());
            } else if let Some(val) = line.strip_prefix("Declassify On:") {
                let s = val.trim();
                if let Some(exemption) = DeclassExemption::parse(s) {
                    attrs.declass_exemption = Some(exemption);
                } else {
                    attrs.declassify_on = Some(s.into());
                }
            }
        }

        Ok(ParsedMarking {
            attrs,
            source_span: candidate.span,
            kind: MarkingType::Cab,
        })
    }

    /// Parse a marking string (without outer parentheses) into IsmAttributes.
    /// Handles both portion form (abbreviated) and banner form (full words).
    ///
    /// `s_offset` is the absolute byte offset of `s` within the original
    /// source buffer. Phase 3 uses it to record per-token absolute spans on
    /// `IsmAttributes::token_spans` so rules can point at byte-precise
    /// diagnostic locations.
    fn parse_marking_string(
        &self,
        s: &str,
        context: MarkingType,
        s_offset: usize,
    ) -> Result<IsmAttributes, CoreError> {
        let mut attrs = IsmAttributes::default();

        if s.is_empty() {
            return Err(CoreError::MalformedMarking(s.to_owned()));
        }

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
        let mut sci_markings: Vec<SciMarking> = Vec::new();
        // SAR: P2 wires the hand-written subparser. Only the FIRST SAR block
        // encountered populates `attrs.sar_markings`; any subsequent SAR block
        // is emitted as `TokenKind::Unknown` so rule E030 (indicator-repeat)
        // can flag the duplicate.
        let mut sar_captured = false;
        let mut aea: Vec<AeaMarking> = Vec::new();
        let mut dissem: Vec<DissemControl> = Vec::new();
        let mut non_ic: Vec<NonIcDissem> = Vec::new();
        let mut rel_to: Vec<Trigraph> = Vec::new();

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
                attrs.classification = parse_classification(trimmed).map(MarkingClassification::Us);
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
                if let Some(nato) = parse_nato_classification(trimmed) {
                    attrs.classification = Some(MarkingClassification::Nato(nato));
                } else if let Some(joint) = parse_joint_classification(trimmed) {
                    attrs.classification = Some(MarkingClassification::Joint(joint));
                } else if let Some(fgi) = parse_fgi_classification(trimmed) {
                    attrs.classification = Some(MarkingClassification::Fgi(fgi));
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
                    attrs.sar_markings = Some(marking);
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
                let parsed_trigraphs =
                    parse_rel_to_with_spans(trimmed, abs_start, self.tokens, &mut token_spans);
                rel_to.extend(parsed_trigraphs);
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
                sci_markings.extend(markings);
            } else if let Some(ctrl) = SciControl::parse(trimmed) {
                sci.push(ctrl);
                token_spans.push(TokenSpan {
                    kind: TokenKind::SciControl,
                    span,
                    text: trimmed.into(),
                });
            } else if trimmed.starts_with("FGI")
                && matches!(attrs.classification, Some(MarkingClassification::Us(_)))
            {
                // FGI marker in a US-classified marking (e.g., SECRET//FGI DEU//NF).
                if let Some(marker) = parse_fgi_marker(trimmed) {
                    attrs.fgi_marker = Some(marker);
                    token_spans.push(TokenSpan {
                        kind: TokenKind::FgiMarker,
                        span,
                        text: trimmed.into(),
                    });
                }
            } else if let Some(ctrl) =
                DissemControl::parse(trimmed).or_else(|| parse_dissem_full_form(trimmed))
            {
                dissem.push(ctrl);
                token_spans.push(TokenSpan {
                    kind: TokenKind::DissemControl,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(nic) = NonIcDissem::parse(trimmed) {
                non_ic.push(nic);
                token_spans.push(TokenSpan {
                    kind: TokenKind::NonIcDissem,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(aea_marking) = AeaMarking::parse(trimmed) {
                aea.push(aea_marking);
                token_spans.push(TokenSpan {
                    kind: TokenKind::AeaMarking,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(exemption) = DeclassExemption::parse(trimmed) {
                attrs.declass_exemption = Some(exemption);
                token_spans.push(TokenSpan {
                    kind: TokenKind::DeclassExemption,
                    span,
                    text: trimmed.into(),
                });
            } else if is_declass_date(trimmed) {
                attrs.declassify_on = Some(trimmed.into());
                token_spans.push(TokenSpan {
                    kind: TokenKind::DeclassDate,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(foreign) = try_parse_foreign_classification(trimmed) {
                // Conflict: a foreign classification in a marking that already
                // has a US classification. US wins at the greater of the two.
                if let Some(MarkingClassification::Us(us_level)) = attrs.classification {
                    let foreign_equiv = match &foreign {
                        ForeignClassification::Nato(n) => n.us_equivalent(),
                        ForeignClassification::Fgi(f) => f.level,
                        ForeignClassification::Joint(j) => j.level,
                    };
                    let max_level = us_level.max(foreign_equiv);
                    attrs.classification = Some(MarkingClassification::Conflict {
                        us: max_level,
                        foreign: Box::new(foreign),
                    });
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
                    } else if let Some(nic) = NonIcDissem::parse(sub_tok) {
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
                                dissem.push(r.dissem.unwrap());
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::DissemControl,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                            SubKind::NonIc => {
                                non_ic.push(r.nic.unwrap());
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::NonIcDissem,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                            SubKind::Aea => {
                                aea.push(r.aea.unwrap());
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

        attrs.sci_controls = sci.into_boxed_slice();
        attrs.sci_markings = sci_markings.into_boxed_slice();
        // `attrs.sar_markings` is populated inline by the SAR branch above
        // when the first SAR category block is encountered; otherwise it
        // defaults to `None` from `IsmAttributes::default()`. `sar_captured`
        // is read in that branch to gate duplicate-block detection.
        attrs.aea_markings = aea.into_boxed_slice();
        attrs.dissem_controls = dissem.into_boxed_slice();
        attrs.non_ic_dissem = non_ic.into_boxed_slice();
        attrs.rel_to = rel_to.into_boxed_slice();
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
        attrs.token_spans = token_spans.into_boxed_slice();

        let _ = context; // used for future context-aware validation

        Ok(attrs)
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

                let mut subs: Vec<Box<str>> = Vec::new();
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

                compartments.push(SciCompartment::new(comp_id.into(), subs.into_boxed_slice()));
            }
        }

        // canonical_enum population (per data-model §canonical_enum):
        // - No compartments → the bare control itself may be a CVE value
        //   (e.g., `SI`, `TK`, `HCS`). Preserves pre-spec behaviour.
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
        || NonIcDissem::parse(s).is_some()
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
    let country_str = rest[remaining_start..].trim();
    let mut countries = Vec::new();
    for token in country_str.split_whitespace() {
        if token.len() == 3 {
            if let Some(t) = Trigraph::try_new(token.as_bytes().try_into().ok()?) {
                countries.push(t);
            }
        }
        // Skip non-trigraph tokens (tetragraphs like NATO handled later)
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
            if let Some(t) = Trigraph::try_new(token.as_bytes().try_into().ok()?) {
                countries.push(t);
            } else {
                return None; // Invalid trigraph
            }
        } else {
            return None; // Not a trigraph or "FGI"
        }
    }

    Some(FgiClassification {
        countries: countries.into(),
        level,
    })
}

/// Parse an FGI marker block in a US-classified marking: `"FGI"` or `"FGI DEU"` or `"FGI DEU GBR"`.
///
/// This is the FGI block between SAR and dissem controls in a US-classified
/// marking (e.g., `SECRET//FGI DEU//NOFORN`). Not to be confused with
/// [`parse_fgi_classification`] which parses a non-US classification.
fn parse_fgi_marker(s: &str) -> Option<FgiMarker> {
    if s == "FGI" {
        return Some(FgiMarker {
            countries: Box::new([]),
        });
    }

    let rest = s.strip_prefix("FGI ")?;
    let mut countries = Vec::new();
    for token in rest.split_whitespace() {
        if token.len() == 3 {
            if let Some(t) = Trigraph::try_new(token.as_bytes().try_into().ok()?) {
                countries.push(t);
            }
        }
        // Skip non-trigraph tokens for now (tetragraphs like NATO)
    }

    Some(FgiMarker {
        countries: countries.into(),
    })
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
    let portion = marque_ism::marking_forms::banner_to_portion(s)?;
    DissemControl::parse(portion)
}

/// Span-aware parse of a `REL TO ...` block. Records one
/// `TokenKind::RelToTrigraph` per recognized country code.
///
/// `block_offset` is the absolute byte offset of `block` within the
/// original source buffer.
fn parse_rel_to_with_spans(
    block: &str,
    block_offset: usize,
    tokens: &dyn TokenSet,
    token_spans: &mut Vec<TokenSpan>,
) -> Vec<Trigraph> {
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

    let mut out: Vec<Trigraph> = Vec::new();
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
        if trimmed.is_empty() || !tokens.is_trigraph(trimmed) {
            continue;
        }
        let b = trimmed.as_bytes();
        if b.len() != 3 {
            continue;
        }
        let Some(t) = Trigraph::try_new([b[0], b[1], b[2]]) else {
            continue;
        };
        out.push(t);
        let abs_start = block_offset + prefix_skip + entry_start_in_after + trim_lead;
        token_spans.push(TokenSpan {
            kind: TokenKind::RelToTrigraph,
            span: Span::new(abs_start, abs_start + 3),
            text: trimmed.into(),
        });
    }
    out
}

// SCI controls, dissemination controls, SAR identifiers, and declass
// exemptions all parse via their generated `parse()` methods (see
// `parse_marking_string` above). The single hand-coded path is
// `parse_classification`, which is documented inline.

/// Returns `true` if `s` looks like an inline declassification date.
///
/// CAPCO allows `YYYYMMDD` (8-digit) or `YYYY` (4-digit, meaning declassify
/// at the start of that calendar year). Both forms are valid in a CAB but
/// are a violation (E005) if they appear directly in a banner or portion
/// marking string.
fn is_declass_date(s: &str) -> bool {
    let bytes = s.as_bytes();
    matches!(bytes.len(), 4 | 8) && bytes.iter().all(u8::is_ascii_digit)
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

        if let Some(program) = parse_sar_program(prog_chunk, program_base, indicator, &mut spans) {
            programs.push(program);
        } else {
            return None;
        }
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
    let prog_shape_ok = match indicator {
        // 2–3 alphanumeric chars.
        SarIndicator::Abbrev => {
            (2..=3).contains(&prog_id.len()) && prog_id.bytes().all(|b| b.is_ascii_alphanumeric())
        }
        // Uppercase ASCII letters with optional spaces; no digits, no
        // hyphens. Must contain at least one non-space byte.
        SarIndicator::Full => {
            prog_id.bytes().all(|b| b == b' ' || b.is_ascii_uppercase())
                && prog_id.bytes().any(|b| b != b' ')
        }
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
        if comp_id.is_empty() || !comp_id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return None;
        }
        let comp_abs_off = seg_off + comp_rel_off;
        spans.push(TokenSpan {
            kind: TokenKind::SarCompartment,
            span: Span::new(base + comp_abs_off, base + comp_abs_off + comp_id.len()),
            text: comp_id.into(),
        });

        let mut subs: Vec<Box<str>> = Vec::with_capacity(parts.len());
        for (sub_rel_off, sub_id) in parts {
            if sub_id.is_empty() || !sub_id.bytes().all(|b| b.is_ascii_alphanumeric()) {
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

        compartments.push(SarCompartment::new(comp_id.into(), subs.into_boxed_slice()));
    }

    Some(SarProgram::new(
        prog_id.into(),
        compartments.into_boxed_slice(),
    ))
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
mod tests {
    use super::*;
    use marque_ism::span::{MarkingCandidate, MarkingType, Span};
    use marque_ism::token_set::CapcoTokenSet;

    fn make_candidate(text: &[u8], kind: MarkingType, offset: usize) -> MarkingCandidate {
        MarkingCandidate {
            span: Span::new(offset, offset + text.len()),
            kind,
        }
    }

    fn parse_banner(text: &str) -> ParsedMarking {
        let source = text.as_bytes();
        let tokens = CapcoTokenSet;
        let parser = Parser::new(&tokens);
        let candidate = make_candidate(source, MarkingType::Banner, 0);
        parser
            .parse(&candidate, source)
            .expect("parse should succeed")
    }

    fn parse_portion(text: &str) -> ParsedMarking {
        let source = text.as_bytes();
        let tokens = CapcoTokenSet;
        let parser = Parser::new(&tokens);
        let candidate = make_candidate(source, MarkingType::Portion, 0);
        parser
            .parse(&candidate, source)
            .expect("parse should succeed")
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
            parsed.attrs.declassify_on.as_deref(),
            Some("20301231"),
            "declassify_on should be populated when YYYYMMDD appears in banner"
        );
    }

    #[test]
    fn banner_with_four_digit_year_populates_attrs() {
        let parsed = parse_banner("SECRET//2035");
        assert_eq!(parsed.attrs.declassify_on.as_deref(), Some("2035"));
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
        assert_eq!(parsed.attrs.rel_to[0], Trigraph::USA);
    }

    #[test]
    fn joint_banner_parses_correctly() {
        let parsed = parse_banner("//JOINT S USA GBR");
        match &parsed.attrs.classification {
            Some(MarkingClassification::Joint(j)) => {
                assert_eq!(j.level, Classification::Secret);
                assert_eq!(j.countries.len(), 2);
                assert_eq!(j.countries[0], Trigraph::USA);
                assert_eq!(j.countries[1].as_str(), "GBR");
            }
            other => panic!("expected Joint, got: {other:?}"),
        }
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
        assert_eq!(marker.countries.len(), 1);
        assert_eq!(marker.countries[0].as_str(), "DEU");
    }

    #[test]
    fn fgi_marker_no_countries() {
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
        assert!(marker.countries.is_empty());
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
        // `SI/TK` — two bare systems in one SCI block. Existing behaviour.
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
}

#[cfg(test)]
mod sar_parse_tests {
    //! Direct unit tests for [`parse_sar_category`] plus integration-level
    //! tests that exercise the dispatch from `parse_marking_string`.

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
    // Dispatch tests (through `parse_marking_string`)
    // ---------------------------------------------------------------------

    fn make_banner(text: &str) -> ParsedMarking {
        let source = text.as_bytes();
        let tokens = CapcoTokenSet;
        let parser = Parser::new(&tokens);
        let candidate = MarkingCandidate {
            span: Span::new(0, source.len()),
            kind: MarkingType::Banner,
        };
        parser.parse(&candidate, source).expect("parse succeeds")
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
                .iter()
                .any(|d| *d == marque_ism::DissemControl::Nf),
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
            unknown_texts.iter().any(|t| *t == "SAR-CD"),
            "duplicate SAR block must be recorded as Unknown, got: {unknown_texts:?}",
        );
    }
}
