// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`Recognizer`] implementations for the engine's strict dispatch path.
//!
//! Phase-4 PR-2 (T058 + T063) wraps `marque-core`'s existing strict
//! parser behind the domain-neutral [`Recognizer`] trait so
//! [`crate::Engine::lint`] dispatches parsing through
//! `Arc<dyn Recognizer<S>>` instead of instantiating `Parser` inline.
//! Phase-4 PR-3 will add a `DecoderRecognizer` alongside this one;
//! both coexist behind the same trait object.
//!
//! ## Why this lives in `marque-engine`, not `marque-capco`
//!
//! Constitution VII forbids `marque-capco` from depending on
//! `marque-core`. `StrictRecognizer` wraps `marque_core::Parser` and
//! produces [`CapcoMarking`](marque_capco::CapcoMarking) values — it
//! therefore needs both chains, and the constitutional dep-graph
//! names `marque-engine` as the sole convergence crate. The scheme-
//! adapter pattern from Phase 3 stays intact: `marque-capco` owns
//! `CapcoScheme` / `CapcoMarking`; the engine owns dispatch.
//!
//! ## Span-offset contract
//!
//! The [`Recognizer`] trait contract is "given bytes, return a
//! [`Parsed<M>`] whose internal spans are relative to the input
//! bytes" (foundational-plan "spans are by offset into this buffer").
//! Rules in `marque-capco` expect source-relative spans, so the engine
//! shifts token spans after `recognize()` returns via
//! [`shift_token_spans`]. That post-processing is the natural seam —
//! the engine is the only code that sees both the full source buffer
//! and the candidate's source offset.
//!
//! ## Zero-candidate = no fabricated marking
//!
//! On a strict-parse failure the recognizer returns
//! `Parsed::Ambiguous { candidates: vec![] }` — the zero-candidate
//! form mandated by the trait contract (foundational-plan
//! line 609-612). Callers MUST treat that as "no plausible
//! interpretation," not as a silently-fabricated marking.

use marque_capco::{CapcoMarking, CapcoScheme};
use marque_core::Parser;
use marque_ism::{
    CapcoTokenSet, IsmAttributes,
    span::{MarkingCandidate, MarkingType, Span},
};
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{ParseContext, Recognizer};

/// Strict-path recognizer. Zero false positives by construction —
/// delegates to the existing [`Parser`], which only accepts the
/// CAPCO-2016 canonical grammar.
///
/// Stateless. Cheaply constructible; the engine holds a single
/// instance behind `Arc` for the lifetime of one [`crate::Engine`].
#[derive(Debug, Default, Clone, Copy)]
pub struct StrictRecognizer;

impl StrictRecognizer {
    /// Construct a strict-path recognizer.
    pub const fn new() -> Self {
        Self
    }
}

impl Recognizer<CapcoScheme> for StrictRecognizer {
    fn recognize(&self, bytes: &[u8], _cx: &ParseContext) -> Parsed<CapcoMarking> {
        // `_cx.strict_evidence` is always satisfied here — this
        // recognizer only emits candidates that hit the strict grammar.
        // `zone` / `position` are rule-side concerns, not parser input.
        let Some(kind) = infer_marking_type(bytes) else {
            return Parsed::Ambiguous {
                candidates: Vec::new(),
            };
        };
        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let leading_ws = if matches!(kind, MarkingType::Portion) {
            bytes.iter().take_while(|b| b.is_ascii_whitespace()).count()
        } else {
            0
        };
        let parse_bytes = &bytes[leading_ws..];
        let candidate = MarkingCandidate {
            span: Span::new(0, parse_bytes.len()),
            kind,
        };
        match parser.parse(&candidate, parse_bytes) {
            Ok(mut parsed) => {
                if leading_ws != 0 {
                    shift_token_spans(&mut parsed.attrs, leading_ws);
                }
                Parsed::Unambiguous(CapcoMarking(parsed.attrs))
            }
            Err(_) => Parsed::Ambiguous {
                candidates: Vec::new(),
            },
        }
    }
}

/// Shift every source-relative byte offset recorded inside `attrs` by
/// `delta`. Used by the engine to reconcile zero-origin spans produced
/// by a [`Recognizer`] (which sees only the candidate's slice of the
/// source) back to the full-source coordinates rules expect.
///
/// Only `IsmAttributes::token_spans` carries offsets today; if later
/// structural fields (SCI / SAR marker spans) start recording source
/// offsets, add the shift here — there is no alternative code path to
/// keep in sync.
///
/// Crate-visibility only: this is an engine-internal seam. The engine
/// is the only caller (PR-3's `DecoderRecognizer` will live in this
/// same crate and call it the same way). Exposing it outside the
/// crate would lock in an API surface before the `IsmAttributes`
/// span story is finished.
pub(crate) fn shift_token_spans(attrs: &mut IsmAttributes, delta: usize) {
    if delta == 0 {
        return;
    }
    for ts in attrs.token_spans.iter_mut() {
        ts.span = Span::new(ts.span.start + delta, ts.span.end + delta);
    }
}

/// Infer a [`MarkingType`] from the shape of `bytes`.
///
/// Mirrors the scanner's classification heuristic so the strict
/// recognizer can reconstruct the parse path from bytes alone.
/// Returns `None` only for empty input — the engine filters
/// zero-length candidates before this point, but the null-return
/// keeps the recognizer safe on hostile input.
fn infer_marking_type(bytes: &[u8]) -> Option<MarkingType> {
    let first = bytes.iter().copied().find(|&b| !b.is_ascii_whitespace())?;
    if first == b'(' {
        return Some(MarkingType::Portion);
    }
    if is_cab_head(bytes) {
        return Some(MarkingType::Cab);
    }
    Some(MarkingType::Banner)
}

/// CAB detection: the three line-initial CAPCO-2016 §E authority
/// heads. Rough but sufficient — the scanner already filtered out
/// anything that doesn't look like a classification region, so
/// byte-prefix matching on the known heads is reliable here.
///
/// Heads recognized (with trailing colon, matching CAPCO-2016 §E
/// labels exactly):
///
/// - `Classified By:` — §E.1 p31 (Original) and §E.2 p32 (Derivative);
///   always the first line of a CAB, and what the `marque-core`
///   scanner keys off of.
/// - `Derived From:` — §E.2 p32, derivative-classification CABs.
/// - `Declassify On:` — §E.1 p31 and §E.2 p32, both classification
///   paths.
///
/// The §E.1 original-classification `Classification Reason:` head is
/// intentionally not matched here — CAPCO-2016 §E.1 p31 spells that
/// label in full, and a bare `Reason:` prefix is not an authorized
/// CAPCO CAB label (it would collide with unrelated "Reason: ..."
/// text in prose). The scanner emits CAB candidates anchored on
/// `Classified By:`, so this helper is only ever reached on bytes
/// the scanner already classified as CAB-shaped; the non-head lines
/// (including `Classification Reason:`) live inside the candidate
/// body.
fn is_cab_head(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let trimmed = text.trim_start();
    trimmed.starts_with("Classified By:")
        || trimmed.starts_with("Derived From:")
        || trimmed.starts_with("Declassify On:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_marking_type_portion_on_leading_paren() {
        assert_eq!(infer_marking_type(b"(TS//SI)"), Some(MarkingType::Portion));
        // Leading whitespace is tolerated — scanners may hand over
        // candidates with a small amount of leading whitespace.
        assert_eq!(infer_marking_type(b"  (S//NF)"), Some(MarkingType::Portion));
    }

    #[test]
    fn infer_marking_type_cab_on_authority_head() {
        assert_eq!(
            infer_marking_type(b"Classified By: X\nDerived From: Y"),
            Some(MarkingType::Cab)
        );
        assert_eq!(
            infer_marking_type(b"Declassify On: 20350101"),
            Some(MarkingType::Cab)
        );
    }

    #[test]
    fn infer_marking_type_bare_reason_prefix_is_not_cab() {
        // CAPCO-2016 §E.1 p31 spells the original-classification head
        // as "Classification Reason:", not bare "Reason:". A bare
        // "Reason:" prefix is indistinguishable from unrelated prose
        // text ("Reason: the quick brown fox…") and must fall through
        // to the Banner classification, not be promoted to CAB.
        assert_eq!(
            infer_marking_type(b"Reason: 1.4(c)"),
            Some(MarkingType::Banner),
        );
    }

    #[test]
    fn infer_marking_type_banner_otherwise() {
        assert_eq!(
            infer_marking_type(b"TOP SECRET//NOFORN"),
            Some(MarkingType::Banner)
        );
    }

    #[test]
    fn infer_marking_type_empty_input_returns_none() {
        assert_eq!(infer_marking_type(b""), None);
        assert_eq!(infer_marking_type(b"   "), None);
    }

    #[test]
    fn strict_recognizer_resolves_portion_unambiguously() {
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        match rx.recognize(b"(S//NF)", &cx) {
            Parsed::Unambiguous(_) => {}
            other => panic!("expected Unambiguous, got {other:?}"),
        }
    }

    #[test]
    fn strict_recognizer_returns_zero_candidate_on_parse_failure() {
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        // Missing closing paren — parser rejects; recognizer surfaces
        // zero-candidate Ambiguous per the trait contract.
        match rx.recognize(b"(S//NF", &cx) {
            Parsed::Ambiguous { candidates } => assert!(candidates.is_empty()),
            other => panic!("expected zero-candidate Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn shift_token_spans_is_identity_for_zero_delta() {
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        let Parsed::Unambiguous(mut marking) = rx.recognize(b"(S//NF)", &cx) else {
            panic!("strict parse should succeed");
        };
        let before: Vec<Span> = marking.0.token_spans.iter().map(|t| t.span).collect();
        shift_token_spans(&mut marking.0, 0);
        let after: Vec<Span> = marking.0.token_spans.iter().map(|t| t.span).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn shift_token_spans_shifts_by_delta() {
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        let Parsed::Unambiguous(mut marking) = rx.recognize(b"(S//NF)", &cx) else {
            panic!("strict parse should succeed");
        };
        let before: Vec<(usize, usize)> = marking
            .0
            .token_spans
            .iter()
            .map(|t| (t.span.start, t.span.end))
            .collect();
        shift_token_spans(&mut marking.0, 100);
        let after: Vec<(usize, usize)> = marking
            .0
            .token_spans
            .iter()
            .map(|t| (t.span.start, t.span.end))
            .collect();
        for (b, a) in before.iter().zip(after.iter()) {
            assert_eq!(a.0, b.0 + 100);
            assert_eq!(a.1, b.1 + 100);
        }
    }

    #[test]
    fn strict_recognizer_is_send_sync_as_trait_object() {
        // Compile-time assertion: the exact `Arc<dyn Recognizer<…>>`
        // storage Engine holds must be `Send + Sync` so `BatchEngine`
        // workers can share one instance (Constitution VI, FR-023).
        // Also assert the concrete `StrictRecognizer` and `Box<dyn …>`
        // directly so a regression in either the impl or the storage
        // choice trips this gate — the `Recognizer: Send + Sync`
        // super-bound would make a bare `Box<dyn …>` check
        // self-satisfying and hide a real `StrictRecognizer` regression.
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<StrictRecognizer>();
        assert_send_sync::<std::sync::Arc<dyn Recognizer<CapcoScheme>>>();
        assert_send_sync::<Box<dyn Recognizer<CapcoScheme>>>();
    }
}
