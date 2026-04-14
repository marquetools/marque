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
    ForeignClassification, FrdBlock, IsmAttributes, JointClassification, MarkingClassification,
    NatoClassification, NonIcDissem, RdBlock, SarIdentifier, SciControl, TokenKind, TokenSpan,
    Trigraph,
};
// Note: unused import warnings for SarIdentifier are expected until the SAR CVE
// has entries. The type is used in from_str() which returns None for now.
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
        let mut sar: Vec<SarIdentifier> = Vec::new();
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
                attrs.classification =
                    parse_classification(trimmed).map(MarkingClassification::Us);
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
            } else if let Some(ctrl) = SciControl::parse(trimmed) {
                sci.push(ctrl);
                token_spans.push(TokenSpan {
                    kind: TokenKind::SciControl,
                    span,
                    text: trimmed.into(),
                });
            } else if trimmed.starts_with("FGI")
                && matches!(
                    attrs.classification,
                    Some(MarkingClassification::Us(_))
                )
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
            } else if let Some(sar_id) = SarIdentifier::parse(trimmed) {
                sar.push(sar_id);
                token_spans.push(TokenSpan {
                    kind: TokenKind::SarIdentifier,
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
                    Sar,
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
                    sar: Option<SarIdentifier>,
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
                            sar: None,
                            aea: None,
                        });
                    } else if let Some(ctrl) = DissemControl::parse(sub_tok)
                        .or_else(|| parse_dissem_full_form(sub_tok))
                    {
                        results.push(SubResult {
                            kind: SubKind::Dissem,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: Some(ctrl),
                            nic: None,
                            sar: None,
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
                            sar: None,
                            aea: None,
                        });
                    } else if let Some(sar_id) = SarIdentifier::parse(sub_tok) {
                        results.push(SubResult {
                            kind: SubKind::Sar,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: None,
                            nic: None,
                            sar: Some(sar_id),
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
                            sar: None,
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
                            sar: None,
                            aea: None,
                        });
                    }
                }

                // Check category consistency: all parsed (non-Unknown) sub-tokens
                // must share the same category for `/` to be a valid intra-block
                // separator. Mixed categories (e.g., SCI + dissem) mean the `/`
                // is a stray single-slash separator that should have been `//`.
                let parsed_kinds: Vec<SubKind> = results
                    .iter()
                    .filter(|r| r.kind != SubKind::Unknown)
                    .map(|r| r.kind)
                    .collect();
                let all_same_category = !parsed_kinds.is_empty()
                    && parsed_kinds.windows(2).all(|w| w[0] == w[1]);

                if !parsed_kinds.is_empty() && !all_same_category {
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
                            SubKind::Sar => {
                                sar.push(r.sar.unwrap());
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::SarIdentifier,
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
        attrs.sar_identifiers = sar.into_boxed_slice();
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
/// `DissemControl`, `SarIdentifier`, `DeclassExemption`) go through their
/// generated `parse()` methods.
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
        (
            parse_classification("TOP SECRET")?,
            tokens.len() - 2,
        )
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
    } else if let Some(fgi) = parse_fgi_classification(s) {
        Some(ForeignClassification::Fgi(fgi))
    } else {
        None
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
                assert!(f.countries.is_empty(), "FGI placeholder should have no countries");
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
                || matches!(parsed.attrs.classification, Some(MarkingClassification::Us(_))),
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
        let marker = parsed.attrs.fgi_marker.as_ref().expect("should have FGI marker");
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
        let marker = parsed.attrs.fgi_marker.as_ref().expect("should have FGI marker");
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
        assert_eq!(
            parsed.attrs.non_ic_dissem[0],
            NonIcDissem::Limdis,
        );
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
        assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);   // DS = LIMDIS
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
            AeaMarking::Rd(RdBlock::default()),
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
            AeaMarking::Frd(FrdBlock::default()),
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
            parsed.attrs.token_spans.iter().all(|t| t.kind != TokenKind::Unknown),
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
        assert!(dissem.contains(&DissemControl::Relido), "must contain RELIDO");
    }

    #[test]
    fn unrecognized_slash_token_emits_unknown() {
        // An unknown token like "XYZZY" in a slash block → Unknown span.
        let parsed = parse_portion("(S//XYZZY)");
        assert!(
            parsed.attrs.token_spans.iter().any(|t| t.kind == TokenKind::Unknown),
            "XYZZY must produce Unknown span"
        );
    }
}
