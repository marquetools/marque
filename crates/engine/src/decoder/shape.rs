//! Shape predicates over decoder inputs and parsed markings.
//!
//! Everything in this module answers a "what shape are these bytes?" or
//! "what shape is this parsed marking?" question. Pure functions, no I/O,
//! no allocation on the hot path. Collected here so the surrounding
//! sub-modules can share a single home for shape-classification logic
//! instead of scattering predicates across `recognizer.rs`,
//! `candidates.rs`, and `dispatcher.rs`.

use marque_capco::CapcoMarking;
use marque_ism::{
    CanonicalAttrs, Classification, DissemControl, MarkingClassification, span::MarkingType,
};
use smallvec::SmallVec;

/// Infer a [`MarkingType`] from the shape of `bytes`.
///
/// Same heuristic as the strict recognizer — portion on leading `(`,
/// CAB on authority-head prefix, banner otherwise. Lives locally so
/// the decoder doesn't need to poke into `StrictRecognizer` internals.
pub(super) fn infer_marking_type(bytes: &[u8]) -> Option<MarkingType> {
    let first = bytes.iter().copied().find(|&b| !b.is_ascii_whitespace())?;
    if first == b'(' {
        return Some(MarkingType::Portion);
    }
    if is_cab_head(bytes) {
        return Some(MarkingType::Cab);
    }
    Some(MarkingType::Banner)
}

pub(super) fn is_cab_head(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let trimmed = text.trim_start();
    trimmed.starts_with("Classified By:")
        || trimmed.starts_with("Derived From:")
        || trimmed.starts_with("Declassify On:")
}

pub(super) fn is_fast_path_candidate_shape(kind: MarkingType, bytes: &[u8]) -> bool {
    if !matches!(kind, MarkingType::Portion) {
        return false;
    }
    if bytes.is_empty() {
        return false;
    }
    let mut start = 0usize;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while start < end && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if start >= end {
        return false;
    }
    let trimmed = &bytes[start..end];
    if trimmed.len() > 32 {
        return false;
    }
    if !(trimmed.first() == Some(&b'(') && trimmed.last() == Some(&b')')) {
        return false;
    }
    if trimmed
        .iter()
        .any(|b| matches!(b, b',' | b' ' | b'\t' | b'\n' | b'\r'))
    {
        return false;
    }
    let mut sep_count = 0usize;
    for w in trimmed.windows(2) {
        if w == b"//" {
            sep_count += 1;
            if sep_count > 1 {
                return false;
            }
        }
    }
    sep_count == 1
}

/// Decoder-only fast parse for the common US classification + dissem shape.
///
/// This avoids invoking the full strict parser for canonical attempts like
/// `(SECRET//NF)` and typo-shaped attempts like `(SERCET//NF)` where the
/// decoder already knows the shape is a simple portion/banner with an
/// optional slash-delimited dissem block. Any non-trivial form (non-US
/// prefix, extra `//` blocks, mixed-category slash blocks, REL TO/DISPLAY
/// ONLY, etc.) falls back to the full parser.
pub(super) fn try_fast_parse_us_class_and_dissem(
    kind: MarkingType,
    bytes: &[u8],
) -> Option<CapcoMarking> {
    if !matches!(kind, MarkingType::Portion | MarkingType::Banner) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?.trim();
    let body = match kind {
        MarkingType::Portion => text.strip_prefix('(')?.strip_suffix(')')?,
        MarkingType::Banner => text,
        _ => return None,
    };
    if body.is_empty() || body.starts_with("//") {
        return None;
    }

    let mut blocks = body.split("//");
    let class_block = blocks.next()?.trim();
    let dissem_block = blocks.next().map(str::trim);
    if blocks.next().is_some() || class_block.is_empty() {
        return None;
    }
    if class_block
        .chars()
        .any(|c| c.is_ascii_whitespace() || c == '/' || c == ',')
    {
        return None;
    }

    let dissem_us = if let Some(block) = dissem_block {
        if block.is_empty() {
            return None;
        }
        let mut out: SmallVec<[DissemControl; 4]> = SmallVec::new();
        for token in block.split('/') {
            let token = token.trim();
            if token.is_empty() || token.chars().any(|c| c.is_ascii_whitespace() || c == ',') {
                return None;
            }
            let token = marque_ism::marking_forms::banner_to_portion(token)
                .or_else(|| marque_ism::marking_forms::title_to_portion(token))
                .unwrap_or(token);
            let control = DissemControl::parse(token)?;
            out.push(control);
        }
        out.into_vec().into_boxed_slice()
    } else {
        Box::new([])
    };

    let mut attrs = CanonicalAttrs::default();
    attrs.classification =
        parse_simple_us_classification(class_block).map(MarkingClassification::Us);
    attrs.dissem_us = dissem_us;
    Some(CapcoMarking::new(attrs))
}

fn parse_simple_us_classification(token: &str) -> Option<Classification> {
    match token {
        "U" | "UNCLASSIFIED" => Some(Classification::Unclassified),
        "R" | "RESTRICTED" => Some(Classification::Restricted),
        "C" | "CONFIDENTIAL" => Some(Classification::Confidential),
        "S" | "SECRET" => Some(Classification::Secret),
        "TS" => Some(Classification::TopSecret),
        _ => None,
    }
}

/// Single-letter parenthetical portion shape (`(A)` … `(Z)`).
///
/// Companion to [`is_bare_classification_shape`] — the latter is the
/// closed canonical-token whitelist for the null-hypothesis bypass; this
/// predicate is the broader filter the prose-glue gate uses to short
/// circuit lookups. The two-letter inner case `(TS)` is outside this
/// predicate's scope on purpose — multi-letter classification abbrevs
/// are rare in prose and don't share the plural-suffix confusability
/// that drives the filter.
pub(super) fn is_single_letter_portion(bytes: &[u8]) -> bool {
    let trimmed = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map(|i| &bytes[i..])
        .unwrap_or(bytes);
    matches!(trimmed, [b'(', inner, b')'] if inner.is_ascii_alphabetic())
}

/// Bare-classification-shape whitelist for the null-hypothesis gate.
///
/// A portion-shaped input whose inner content is exactly a canonical
/// classification token — `(U)`, `(C)`, `(S)`, `(TS)`, `(R)`, or one of
/// the NATO portion abbreviations (`NU`, `NR`, `NC`, `NS`, `CTS`) —
/// is the strict-grammar shape of a valid classification portion. The
/// null-hypothesis filter MUST NOT suppress these: their byte form is
/// short enough that a prose-side prior derived from observed bytes
/// can outweigh the marking-side prior even when the strict grammar
/// unambiguously accepts the form (e.g., short single-letter tokens
/// have non-trivial prose mass as standalone parenthetical glyphs but
/// are also the *only* CAPCO portion shape that exists for those
/// classification levels).
///
/// The list is closed and byte-exact: leading/trailing whitespace
/// inside the parens (`( C )`) is not matched, mixed case (`(cts)`,
/// `(Ts)`) is not matched. This is intentional — case folding to a
/// canonical bare form happens in the decoder's canonicalization
/// stage; this gate operates on the raw observed bytes the caller
/// passed to `recognize`, before any case-fold, so a lowercase or
/// mixed-case input still goes through the null-hypothesis filter and
/// is suppressed when the prose hypothesis dominates.
///
/// Companion to [`has_double_slash`] in the score-time null gate
/// (`recognize` §5). Together they pass through (a) bare-classification
/// portion shapes the grammar uniquely accepts and (b) any portion
/// carrying a category separator (`//`) — the latter being a shape no
/// English prose convention produces.
pub(super) fn is_bare_classification_shape(bytes: &[u8]) -> bool {
    matches!(
        bytes,
        b"(U)"
            | b"(C)"
            | b"(S)"
            | b"(TS)"
            | b"(R)"
            | b"(NU)"
            | b"(NR)"
            | b"(NC)"
            | b"(NS)"
            | b"(CTS)"
    )
}

/// Does the input contain a `//` category separator anywhere in its
/// bytes? Used by the null-hypothesis gate.
///
/// A portion or banner shape containing `//` is by construction not a
/// prose accident: English prose convention has no use for adjacent
/// double-slashes inside parentheses or at line position. The
/// presence of `//` is sufficient evidence that the input intends to
/// be a marking; the score-time null filter passes such candidates
/// through without the prose-vs-marking comparison.
///
/// Byte-windowed search — no allocation. Linear in `bytes.len()`,
/// trivially short for portion/banner shapes.
pub(super) fn has_double_slash(bytes: &[u8]) -> bool {
    bytes.windows(2).any(|w| w == b"//")
}

/// True if the parsed marking carries any non-trivial classification or
/// control signal — i.e., the strict parser found something worth
/// emitting through the engine.
///
/// **Not part of the public API.** Marked `#[doc(hidden)]` so it
/// stays off rustdoc surfaces and `cargo doc` output — the
/// `pub` modifier exists solely so `crates/engine/tests/` can
/// reach it across the integration-test crate boundary
/// (Rust integration tests live in a separate crate and so
/// `pub(crate)` is not visible to them). Downstream consumers
/// MUST NOT depend on this signature; it can change at any
/// time alongside `CapcoMarking` evolution. The supported way
/// to ask "is this marking non-trivial?" is to run
/// [`Engine::lint`](crate::Engine::lint) and inspect its emitted
/// diagnostics — the engine applies this filter internally and
/// surfaces only non-trivial markings to the rule layer.
#[doc(hidden)]
pub fn is_nontrivial_marking(marking: &CapcoMarking) -> bool {
    let a = &marking.0;
    a.classification.is_some()
        || !a.sci_controls.is_empty()
        || a.sar_markings.is_some()
        || !a.aea_markings.is_empty()
        || a.fgi_marker.is_some()
        // Walk the unified dissem_iter — a marking with
        // any dissem on either namespace is non-trivial.
        || a.dissem_iter().next().is_some()
        || !a.non_ic_dissem.is_empty()
        || !a.rel_to.is_empty()
        || a.classified_by.is_some()
        || a.derived_from.is_some()
        || a.declassify_on.is_some()
        || a.declass_exemption.is_some()
}

/// True when the strict-parse result is complete enough that the
/// dispatcher should accept it and skip the decoder fallback.
///
/// The strict parser (`marque_core::Parser`) is lenient about
/// content: it categorizes tokens by *position* (the first token
/// inside `(...)` is marked as `TokenKind::Classification`
/// regardless of whether its text is a valid classification value),
/// and falls back to `TokenKind::Unknown` only for truly unplaceable
/// tokens. So a shape like `(SERCET//NOFORN)` parses to a marking
/// with `classification: None` (SERCET doesn't resolve to any
/// `Classification` variant), `dissem_controls: [Nf]` (NOFORN was
/// recognized), and a Classification-kind `TokenSpan` carrying the
/// literal text "SERCET". That result is *nontrivial* but also
/// *incomplete* — exactly the mangled-input case the decoder exists
/// to recover.
///
/// Predicate, kind-aware:
///
/// - [`MarkingType::Portion`] / [`MarkingType::Banner`]: complete
///   iff `classification.is_some()` AND no `TokenKind::Unknown`
///   spans survived. Both branches matter — SERCET→None catches
///   the classification-slot typo; the `Unknown` check catches
///   typos in the tail (e.g., `(S//FRBN)` where the classification
///   is fine but FRBN is mangled and lands as Unknown).
/// - [`MarkingType::Cab`]: complete iff any CAB field is present
///   (`classified_by` / `derived_from` / `declassify_on`).
///   CAB-kind input doesn't require a classification axis — an
///   isolated authority block stands on its own.
/// - Anything else: fall back to the generic nontrivial check.
pub(super) fn strict_parse_is_complete(marking: &CapcoMarking, kind: MarkingType) -> bool {
    use marque_ism::TokenKind;
    let attrs = &marking.0;
    match kind {
        MarkingType::Portion | MarkingType::Banner => {
            attrs.classification.is_some()
                && !attrs
                    .token_spans
                    .iter()
                    .any(|s| matches!(s.kind, TokenKind::Unknown))
        }
        MarkingType::Cab => {
            attrs.classified_by.is_some()
                || attrs.derived_from.is_some()
                || attrs.declassify_on.is_some()
                || attrs.declass_exemption.is_some()
        }
        _ => is_nontrivial_marking(marking),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(unused_imports)]
mod tests {
    use std::sync::LazyLock;

    use marque_capco::{CapcoMarking, CapcoScheme};
    use marque_core::Parser;
    use marque_ism::{
        CapcoTokenSet, Classification, DissemControl, MarkingClassification,
        span::{MarkingCandidate, MarkingType, Span},
    };
    use marque_rules::confidence::FeatureId;
    use marque_scheme::MarkingScheme;
    use marque_scheme::ambiguity::Parsed;
    use marque_scheme::recognizer::{LinePrefix, ParseContext, Recognizer};
    use smallvec::SmallVec;

    use super::*;
    use crate::decoder::DecoderRecognizer;
    use crate::decoder::test_helpers::{TEST_SCHEME, deep_cx};

    #[test]
    fn fast_path_parses_simple_us_class_and_dissem_shape() {
        let canonical = try_fast_parse_us_class_and_dissem(MarkingType::Portion, b"(SECRET//NF)")
            .expect("canonical simple portion should hit decoder fast-path");
        assert_eq!(
            canonical.0.classification,
            Some(MarkingClassification::Us(Classification::Secret))
        );
        assert_eq!(canonical.0.dissem_us.as_ref(), &[DissemControl::Nf]);
        assert!(canonical.0.token_spans.is_empty());

        // Intentional typo: the fast-path preserves strict-parser behavior for
        // unknown classification tokens by keeping `classification = None`
        // while still retaining known dissem controls.
        let marking = try_fast_parse_us_class_and_dissem(MarkingType::Portion, b"(SERCET//NF)")
            .expect("simple portion should hit decoder fast-path");
        assert_eq!(marking.0.classification, None);
        assert_eq!(marking.0.dissem_us.as_ref(), &[DissemControl::Nf]);
        assert!(marking.0.token_spans.is_empty());
    }

    #[test]
    fn fast_path_rejects_complex_or_mixed_category_shapes() {
        assert!(
            try_fast_parse_us_class_and_dissem(MarkingType::Portion, b"(S//SI/NF)").is_none(),
            "mixed SCI/dissem slash block must fall back to full strict parser",
        );
        assert!(
            try_fast_parse_us_class_and_dissem(MarkingType::Portion, b"(S//REL TO USA, GBR)")
                .is_none(),
            "REL TO block must fall back to full strict parser",
        );
    }

    #[test]
    fn strict_parse_is_complete_rejects_unknown_classification() {
        // This is the regression-guard for PR #114 review comment
        // on decoder.rs:946 — strict parse of `(SERCET//NOFORN)`
        // recognizes NOFORN but leaves `classification: None` because
        // SERCET doesn't resolve to any `Classification` variant.
        // Without the `strict_parse_is_complete` check, the
        // dispatcher would accept this as a complete strict result
        // and never fall through to the decoder.
        // Inline scheme per test for hermeticity.
        let scheme = CapcoScheme::new();
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 16),
            kind: MarkingType::Portion,
        };
        let parsed = parser
            .parse(&candidate, b"(SERCET//NOFORN)")
            .expect("strict parser should accept (SERCET//NOFORN) leniently");
        let marking = CapcoMarking::new(scheme.canonicalize(parsed.attrs));
        assert!(
            is_nontrivial_marking(&marking),
            "NOFORN survives as a dissem control → marking is nontrivial"
        );
        assert!(
            !strict_parse_is_complete(&marking, MarkingType::Portion),
            "SERCET left `classification: None` → strict parse is incomplete; \
             dispatcher must fall back to decoder. attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn strict_parse_is_complete_accepts_clean_marking() {
        // Inline scheme per test for hermeticity.
        let scheme = CapcoScheme::new();
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 7),
            kind: MarkingType::Portion,
        };
        let parsed = parser
            .parse(&candidate, b"(S//NF)")
            .expect("canonical portion must strict-parse");
        let marking = CapcoMarking::new(scheme.canonicalize(parsed.attrs));
        assert!(
            strict_parse_is_complete(&marking, MarkingType::Portion),
            "canonical (S//NF) must be accepted as complete; attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn strict_parse_is_complete_rejects_trailing_unknown_token() {
        // `(S//FRBN)` — classification parses (`S` → Secret) but the
        // tail token `FRBN` lands in an `Unknown` span. The
        // dispatcher must fall back so the decoder can resolve
        // `FRBN` → `NF` (or reject).
        // Inline scheme per test for hermeticity.
        let scheme = CapcoScheme::new();
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidate = MarkingCandidate {
            span: Span::new(0, 9),
            kind: MarkingType::Portion,
        };
        let parsed = parser
            .parse(&candidate, b"(S//FRBN)")
            .expect("strict parser accepts (S//FRBN) leniently");
        let marking = CapcoMarking::new(scheme.canonicalize(parsed.attrs));
        // `S` resolved, so classification is Some — but the
        // Unknown-tail check still fires.
        assert!(
            !strict_parse_is_complete(&marking, MarkingType::Portion),
            "`FRBN` is Unknown-kind → strict parse is incomplete; attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn is_bare_classification_shape_recognizes_whitelist() {
        // Issue #472: the 10-entry closed whitelist covers every
        // canonical CAPCO portion classification token. All entries
        // must match byte-exact.
        for s in &[
            b"(U)" as &[u8],
            b"(C)",
            b"(S)",
            b"(TS)",
            b"(R)",
            b"(NU)",
            b"(NR)",
            b"(NC)",
            b"(NS)",
            b"(CTS)",
        ] {
            assert!(
                is_bare_classification_shape(s),
                "whitelist entry {:?} must match",
                std::str::from_utf8(s).unwrap_or("<bytes>"),
            );
        }

        // Non-whitelist 3-letter all-caps acronyms — the prose-acronym
        // false-positive surface the gate is designed to suppress.
        for s in &[
            b"(CMS)" as &[u8],
            b"(MD)",
            b"(SI)",
            b"(CTs)", // mixed case — case-fold happens later, the gate runs on raw bytes
            b"(c)",   // lowercase fails the byte-exact match
            b"(s)",
            b"(u)",
            b"(C//NF)", // has `//`
            b"( C )",   // interior whitespace fails byte-exact
            b"(CT)",    // not on the canonical token set
        ] {
            assert!(
                !is_bare_classification_shape(s),
                "non-whitelist input {:?} must not match",
                std::str::from_utf8(s).unwrap_or("<bytes>"),
            );
        }
    }

    #[test]
    fn is_bare_classification_shape_is_byte_exact() {
        // Interior whitespace inside the parens (`( C )`, `(C )`,
        // `( C)`) does not match — that's intentional. Whitespace
        // tolerance happens elsewhere (the strict recognizer strips
        // leading whitespace on portion candidates), but this gate
        // operates on the raw observed bytes. Any whitespace-bearing
        // shape goes through the null-hypothesis filter so a
        // prose-shaped `( C )` mid-prose is correctly tested against
        // the observed prose prior.
        assert!(!is_bare_classification_shape(b"( C)"));
        assert!(!is_bare_classification_shape(b"(C )"));
        assert!(!is_bare_classification_shape(b"( C )"));
        assert!(!is_bare_classification_shape(b" (C)"));
        assert!(!is_bare_classification_shape(b"(C) "));
    }

    #[test]
    fn has_double_slash_detects_slash_slash() {
        // True cases: any input containing `//` anywhere.
        assert!(has_double_slash(b"(S//NF)"));
        assert!(has_double_slash(b"S//REL"));
        assert!(has_double_slash(b"//"));
        assert!(has_double_slash(b"prefix//suffix"));
        assert!(has_double_slash(b"SECRET//NOFORN"));

        // False cases: no `//` sequence.
        assert!(!has_double_slash(b"/"));
        assert!(!has_double_slash(b"(S)"));
        assert!(!has_double_slash(b"(S/NF)"));
        assert!(!has_double_slash(b""));
        assert!(!has_double_slash(b"/foo/bar/"));
    }
}
