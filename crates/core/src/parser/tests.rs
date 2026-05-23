use super::*;
use marque_ism::CanonicalAttrs;
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_scheme::Span;

/// Test-helper output: a [`ParsedMarking`] post-canonicalization,
/// so existing assertions on the typed `attrs.classification` /
/// `attrs.dissem_us` / `attrs.dissem_nato` shape continue to work
/// without per-test edits.
///
/// Test-fixture carve-out per Constitution V Principle V — the
/// structural rename is inlined here only to construct test inputs
/// whose shape mirrors the engine's post-recognition view.
/// `marque-core` cannot dev-depend on `marque-capco` (Constitution
/// VII), so the trait route `CapcoScheme::canonicalize` is
/// unreachable from here. The inlined body mirrors the override's
/// field mapping and output semantics — including the §G.2 p41 /
/// PR 9b T132 debug-assert — but is not a literal byte-for-byte
/// copy (control flow + locals differ, `From::from` returns
/// `Self` rather than the override's `CanonicalAttrs`).
/// FR-040 PRC100 stays satisfied because the enclosing
/// `From::from` signature is `(ParsedMarking) -> Self`, not
/// `(ParsedAttrs) -> CanonicalAttrs`.
pub(super) struct CanonicalParsed {
    pub attrs: CanonicalAttrs,
    #[allow(dead_code)] // tests inspect attrs only; kept for parity
    pub source_span: Span,
    #[allow(dead_code)]
    pub kind: MarkingType,
}

impl<'src> From<ParsedMarking<'src>> for CanonicalParsed {
    fn from(p: ParsedMarking<'src>) -> Self {
        let marque_ism::ParsedAttrs {
            classification,
            sci_markings,
            sci_controls,
            sar_markings,
            aea_markings,
            fgi_marker,
            dissem_us,
            dissem_nato,
            non_ic_dissem,
            rel_to,
            display_only_to,
            declassify_on,
            classified_by,
            derived_from,
            declass_exemption,
            token_spans,
            source_bytes_origin: _,
        } = p.attrs;
        let attrs = CanonicalAttrs {
            classification: classification.map(|c| c.value),
            sci_controls,
            sci_markings: Vec::from(sci_markings)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            sar_markings: sar_markings.map(|q| q.value),
            aea_markings: Vec::from(aea_markings)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            fgi_marker: fgi_marker.map(|q| q.value),
            dissem_us: Vec::from(dissem_us)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            dissem_nato: Vec::from(dissem_nato)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            non_ic_dissem: Vec::from(non_ic_dissem)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            rel_to: Vec::from(rel_to)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            display_only_to: Vec::from(display_only_to)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            declassify_on: declassify_on.map(|q| q.value),
            classified_by: classified_by.map(Box::<str>::from),
            derived_from: derived_from.map(Box::<str>::from),
            declass_exemption,
            token_spans,
        };

        // Mirror the PR 9b (T132) invariant guard carried by
        // `CapcoScheme::canonicalize`. `attribute_dissems` is the
        // single source of truth; this debug-only assertion catches
        // a future bug where attribution is skipped or a hand-built
        // `ParsedAttrs` is fed in with both fields populated.
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                attrs.dissem_nato.is_empty() || attrs.us_classification().is_none(),
                "dissem_nato populated alongside US classification — \
                 attribute_dissems was skipped or bypassed. CAPCO-2016 p41 \
                 reciprocity rule violated."
            );
        }

        Self {
            attrs,
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

#[path = "tests/basic_tests.rs"]
mod basic_tests;

#[path = "tests/nato_fgi_tests.rs"]
mod nato_fgi_tests;

#[path = "tests/fgi_span_tests.rs"]
mod fgi_span_tests;

#[path = "tests/controls_tests.rs"]
mod controls_tests;

#[path = "tests/deprecated_long_form_tests.rs"]
mod deprecated_long_form_tests;
