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
        }
    }

    fn parse_portion(
        &self,
        text: &str,
        candidate: &MarkingCandidate,
    ) -> Result<ParsedMarking, CoreError> {
        // Strip outer parentheses: "(TS//SI//NF)" -> "TS//SI//NF"
        let inner = text
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| CoreError::MalformedMarking(text.to_owned()))?;

        let attrs = self.parse_marking_string(inner, MarkingType::Portion)?;
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
        let attrs = self.parse_marking_string(text.trim(), MarkingType::Banner)?;
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
    fn parse_marking_string(
        &self,
        s: &str,
        context: MarkingType,
    ) -> Result<IsmAttributes, CoreError> {
        let mut attrs = IsmAttributes::default();

        // Blocks are separated by `//`; the first block is always the classification.
        let blocks: Vec<&str> = s.split("//").collect();
        if blocks.is_empty() {
            return Err(CoreError::MalformedMarking(s.to_owned()));
        }

        // Parse classification (first block).
        attrs.classification = parse_classification(blocks[0].trim());

        // Parse subsequent blocks.
        let mut sci: Vec<SciControl> = Vec::new();
        let mut sar: Vec<SarIdentifier> = Vec::new();
        let mut dissem: Vec<DissemControl> = Vec::new();
        let mut rel_to = Vec::new();

        for block in &blocks[1..] {
            let block = block.trim();
            if block.starts_with("REL TO") || block.starts_with("REL ") {
                rel_to.extend(parse_rel_to(block, self.tokens));
            } else if let Some(ctrl) = SciControl::parse(block) {
                sci.push(ctrl);
            } else if let Some(ctrl) = DissemControl::parse(block) {
                dissem.push(ctrl);
            } else if let Some(sar_id) = SarIdentifier::parse(block) {
                sar.push(sar_id);
            } else if let Some(exemption) = DeclassExemption::parse(block) {
                // Declass exemption codes (e.g., 25X1, 50X1-HUM) that appear
                // inside a banner or portion marking trigger E005 — they belong
                // in the CAB "Declassify On:" line, not in the marking string.
                attrs.declass_exemption = Some(exemption);
            } else if is_declass_date(block) {
                // Free-text declassification dates (YYYYMMDD or YYYY) that
                // appear inside a banner or portion also belong in the CAB.
                attrs.declassify_on = Some(block.into());
            }
            // Other unrecognized tokens are silently dropped here.
            // The rules layer (E008) detects and reports them.
        }

        attrs.sci_controls = sci.into_boxed_slice();
        attrs.sar_identifiers = sar.into_boxed_slice();
        attrs.dissem_controls = dissem.into_boxed_slice();
        attrs.rel_to = rel_to.into_boxed_slice();

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

fn parse_rel_to(block: &str, tokens: &dyn TokenSet) -> Vec<Trigraph> {
    // "REL TO USA, GBR, AUS" or "REL USA, GBR"
    let after_rel = block
        .strip_prefix("REL TO")
        .or_else(|| block.strip_prefix("REL"))
        .unwrap_or(block)
        .trim();

    after_rel
        .split(',')
        .map(str::trim)
        .filter(|t| tokens.is_trigraph(t))
        .filter_map(|t| {
            let b = t.as_bytes();
            if b.len() == 3 {
                Trigraph::try_new([b[0], b[1], b[2]])
            } else {
                None
            }
        })
        .collect()
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
}
