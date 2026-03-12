//! Phase 2/3: token extraction and structural parsing.
//!
//! Takes [`Candidate`] spans from the scanner and produces [`IsmAttributes`].
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

use crate::attrs::{Classification, DeclassOn, IsmAttributes, Trigraph};
use crate::error::CoreError;
use crate::span::{Candidate, MarkingType, Span};

/// Parse result for a single candidate.
#[derive(Debug)]
pub struct ParsedMarking {
    pub attrs: IsmAttributes,
    pub source_span: Span,
    pub kind: MarkingType,
}

/// Minimal interface the parser needs from the token set.
/// Implemented by marque-capco's generated automaton; injected at engine init.
pub trait TokenSet: Send + Sync {
    /// Returns the canonical token string if `token` is a known CVE value.
    fn canonicalize<'a>(&self, token: &'a str) -> Option<&'static str>;

    /// Returns true if `token` is a known country trigraph.
    fn is_trigraph(&self, token: &str) -> bool;
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
    pub fn parse(&self, candidate: &Candidate, source: &[u8]) -> Result<ParsedMarking, CoreError> {
        let text = candidate.span.as_str(source);
        match candidate.kind {
            MarkingType::Portion => self.parse_portion(text, candidate),
            MarkingType::Banner  => self.parse_banner(text, candidate),
            MarkingType::Cab     => self.parse_cab(text, candidate),
        }
    }

    fn parse_portion(&self, text: &str, candidate: &Candidate) -> Result<ParsedMarking, CoreError> {
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

    fn parse_banner(&self, text: &str, candidate: &Candidate) -> Result<ParsedMarking, CoreError> {
        let attrs = self.parse_marking_string(text.trim(), MarkingType::Banner)?;
        Ok(ParsedMarking {
            attrs,
            source_span: candidate.span,
            kind: MarkingType::Banner,
        })
    }

    fn parse_cab(&self, text: &str, candidate: &Candidate) -> Result<ParsedMarking, CoreError> {
        // CAB is line-structured: "Classified By: ...\nDerived From: ...\nDeclassify On: ..."
        let mut attrs = IsmAttributes::default();

        for line in text.lines() {
            if let Some(val) = line.strip_prefix("Classified By:") {
                attrs.classified_by = Some(val.trim().to_owned());
            } else if let Some(val) = line.strip_prefix("Derived From:") {
                attrs.derived_from = Some(val.trim().to_owned());
            } else if let Some(val) = line.strip_prefix("Declassify On:") {
                attrs.declassify_on = Some(parse_declass_on(val.trim()));
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
        let mut sci = Vec::new();
        let mut sar = Vec::new();
        let mut dissem = Vec::new();
        let mut rel_to = Vec::new();

        for block in &blocks[1..] {
            let block = block.trim();
            if block.starts_with("REL TO") || block.starts_with("REL ") {
                rel_to.extend(parse_rel_to(block, self.tokens));
            } else if is_sci_control(block) {
                sci.push(block.to_owned());
            } else if is_sar_identifier(block) {
                sar.push(block.to_owned());
            } else {
                dissem.push(block.to_owned());
            }
        }

        attrs.sci_controls   = sci.into_boxed_slice();
        attrs.sar_identifiers = sar.into_boxed_slice();
        attrs.dissem_controls = dissem.into_boxed_slice();
        attrs.rel_to          = rel_to.into_boxed_slice();

        let _ = context; // used for future context-aware validation

        Ok(attrs)
    }
}

fn parse_classification(s: &str) -> Option<Classification> {
    match s {
        "TS" | "TOP SECRET"    => Some(Classification::TopSecret),
        "S"  | "SECRET"        => Some(Classification::Secret),
        "C"  | "CONFIDENTIAL"  => Some(Classification::Confidential),
        "U"  | "UNCLASSIFIED"  => Some(Classification::Unclassified),
        _                      => None,
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
                Some(Trigraph([b[0], b[1], b[2]]))
            } else {
                None
            }
        })
        .collect()
}

/// Heuristic: SCI controls typically start with known prefixes.
/// Full validation done by marque-capco rules against CVE.
fn is_sci_control(s: &str) -> bool {
    matches!(s, "SI" | "TK" | "HCS" | "KDK" | "RST")
        || s.starts_with("SI-")
        || s.starts_with("TK-")
        || s.starts_with("HCS-")
}

/// Heuristic: SAR identifiers are typically 3–10 uppercase chars.
/// Full validation done by marque-capco rules.
fn is_sar_identifier(s: &str) -> bool {
    s.len() >= 3
        && s.len() <= 15
        && s.chars().all(|c| c.is_uppercase() || c == '-')
        && !is_known_dissem(s)
}

fn is_known_dissem(s: &str) -> bool {
    matches!(
        s,
        "NOFORN" | "NF" | "RELIDO" | "FOUO" | "ORCON" | "PROPIN"
            | "FISA" | "DSEN" | "LIMDIS" | "IMC" | "EYES ONLY"
    )
}

fn parse_declass_on(s: &str) -> DeclassOn {
    // Exemptions start with X, DN, etc.
    if s.starts_with('X') || s.starts_with("DN") {
        DeclassOn::Exemption(s.to_owned())
    } else if s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        DeclassOn::Date(s.to_owned())
    } else {
        DeclassOn::Event(s.to_owned())
    }
}
