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
    Classification, DeclassExemption, DissemControl, IsmAttributes, SarIdentifier, SciControl,
    TokenKind, TokenSpan, Trigraph,
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

        // First block is the classification.
        let mut sci: Vec<SciControl> = Vec::new();
        let mut sar: Vec<SarIdentifier> = Vec::new();
        let mut dissem: Vec<DissemControl> = Vec::new();
        let mut rel_to: Vec<Trigraph> = Vec::new();

        for (idx, &(rel_start, rel_end)) in block_ranges.iter().enumerate() {
            let raw = &s[rel_start..rel_end];
            // Trim ASCII whitespace and recover the trimmed-content offset
            // within the raw block, then add to the absolute s_offset.
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let trim_lead = raw.len() - raw.trim_start().len();
            let abs_start = s_offset + rel_start + trim_lead;
            let abs_end = abs_start + trimmed.len();
            let span = Span::new(abs_start, abs_end);

            if idx == 0 {
                // Classification block — track even when parse fails so the
                // span is available for diagnostics.
                attrs.classification = parse_classification(trimmed);
                token_spans.push(TokenSpan {
                    kind: TokenKind::Classification,
                    span,
                    text: trimmed.into(),
                });
                continue;
            }

            if trimmed.starts_with("REL TO") || trimmed.starts_with("REL ") {
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
            } else if let Some(ctrl) =
                DissemControl::parse(trimmed).or_else(|| parse_dissem_full_form(trimmed))
            {
                dissem.push(ctrl);
                token_spans.push(TokenSpan {
                    kind: TokenKind::DissemControl,
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
            } else if let Some(exemption) = DeclassExemption::parse(trimmed) {
                // Declass exemption codes (e.g., 25X1, 50X1-HUM) that appear
                // inside a banner or portion marking trigger E005 — they belong
                // in the CAB "Declassify On:" line, not in the marking string.
                attrs.declass_exemption = Some(exemption);
                token_spans.push(TokenSpan {
                    kind: TokenKind::DeclassExemption,
                    span,
                    text: trimmed.into(),
                });
            } else if is_declass_date(trimmed) {
                // Free-text declassification dates (YYYYMMDD or YYYY) that
                // appear inside a banner or portion also belong in the CAB.
                attrs.declassify_on = Some(trimmed.into());
                token_spans.push(TokenSpan {
                    kind: TokenKind::DeclassDate,
                    span,
                    text: trimmed.into(),
                });
            } else {
                // Unrecognized — Phase 3 records this as TokenKind::Unknown
                // so E008 can fire one diagnostic per unknown token without
                // re-parsing. E007 also walks Unknown tokens and looks each
                // up in the migration table to detect deprecated forms.
                token_spans.push(TokenSpan {
                    kind: TokenKind::Unknown,
                    span,
                    text: trimmed.into(),
                });
            }
        }

        attrs.sci_controls = sci.into_boxed_slice();
        attrs.sar_identifiers = sar.into_boxed_slice();
        attrs.dissem_controls = dissem.into_boxed_slice();
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
/// `"U"`) or banner form (`"TOP SECRET"`, `"SECRET"`, ...).
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
        "U" | "UNCLASSIFIED" => Some(Classification::Unclassified),
        _ => None,
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
fn parse_dissem_full_form(s: &str) -> Option<DissemControl> {
    let abbrev = match s {
        "NOFORN" => "NF",
        "ORCON" => "OC",
        "IMCON" => "IMC",
        "DEA SENSITIVE" => "DSEN",
        "PROPIN" => "PR",
        "RELIDO" => "RELIDO",
        _ => return None,
    };
    DissemControl::parse(abbrev)
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
}
