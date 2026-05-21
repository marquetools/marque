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
//! The [`Recognizer`] trait contract is "given bytes plus the byte
//! position of `bytes[0]` in the full source buffer (`offset`), return
//! a [`Parsed<M>`] whose internal spans are absolute coordinates into
//! the original source." Rules in `marque-capco` consume source-
//! relative spans directly; recognizers translate any zero-origin
//! spans their inner parser produces by calling [`shift_token_spans`]
//! before returning. The strict path additionally shifts past any
//! leading whitespace it stripped from `bytes`; the engine no longer
//! runs a post-`recognize()` shift over `token_spans` (issue #431).
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
    CanonicalAttrs, CapcoTokenSet, Classification, MarkingClassification,
    span::{MarkingCandidate, MarkingType, Span},
};
use marque_scheme::MarkingScheme;
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
    fn recognize(
        &self,
        bytes: &[u8],
        offset: usize,
        scheme: &CapcoScheme,
        _cx: &ParseContext,
    ) -> Parsed<CapcoMarking> {
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
            Ok(parsed) => {
                // Canonicalization seam: `MarkingScheme::canonicalize`
                // is the sole `ParsedAttrs → CanonicalAttrs` route per
                // FR-043 (PR 3c.2.E retired the transitional
                // `marque_ism::from_parsed_unchecked` adapter). The
                // recognizer receives the scheme via the `&S`
                // parameter threaded through `recognize()` after
                // `engine-S-generic-recognizer-cleanup` (#634) landed.
                let mut attrs = scheme.canonicalize(parsed.attrs);
                // Two shifts collapsed into one (issue #431):
                //   * `leading_ws` — the parser saw `parse_bytes`, which
                //     begins `leading_ws` bytes after `bytes[0]`, so its
                //     emitted spans are off by that amount relative to
                //     the caller's `bytes` slice.
                //   * `offset` — `bytes[0]` is `offset` bytes into the
                //     full source buffer; rules consume absolute source
                //     spans, so the engine used to apply this shift in a
                //     post-pass.
                // Composing both deltas here keeps `token_spans`
                // absolute on return and removes the engine post-pass.
                let total_shift = offset + leading_ws;
                if total_shift != 0 {
                    shift_token_spans(&mut attrs, total_shift);
                }
                let marking = CapcoMarking::new(attrs);
                // Reject `Us(Restricted)` markings. RESTRICTED is by
                // definition a non-US classification level — see
                // [`is_us_restricted`] for the full rationale and
                // why `fgi_marker.is_some()` does not redeem the
                // marking.
                if is_us_restricted(&marking) {
                    return Parsed::Ambiguous {
                        candidates: Vec::new(),
                    };
                }
                Parsed::Unambiguous(marking)
            }
            Err(_) => Parsed::Ambiguous {
                candidates: Vec::new(),
            },
        }
    }
}

/// True when the marking is classified as `Us(Restricted)`.
///
/// CAPCO §H.7: RESTRICTED is by definition a non-US classification
/// level. A US document cannot be RESTRICTED. Every legitimate
/// foreign-origin RESTRICTED form parses to a non-US variant of
/// [`MarkingClassification`] — `Fgi(Restricted)` for `(//DEU R)` or
/// `(//FGI DEU R)`, `Nato(NatoRestricted)` for `(//NR)` or fully-
/// spelled `(//NATO RESTRICTED)`, `Joint(...)` for shared-origin
/// markings — so those are unaffected by this predicate.
///
/// `Us(Restricted)` only appears when the strict parser blindly
/// mapped a leading `R` token onto the US classification axis
/// (`Classification::Restricted`'s portion abbreviation is `"R"`).
/// That mapping is the bug. Every shape that produces it — bare
/// `(R)`, `(R//NF)` (R first, dissem after), `R//USA, GBR` (banner
/// shape, R first), `RESTRICTED//FGI DEU//NOFORN` (long-form R
/// followed by a US-marking FGI block) — is invalid and must be
/// rejected.
///
/// **Why `fgi_marker.is_some()` does not redeem the marking.**
/// `fgi_marker` is the `FGI [LIST]` block parsed in *US-classified*
/// markings (e.g., `SECRET//FGI DEU//NOFORN` → `Us(Secret)` +
/// `fgi_marker: Some([DEU])`). The block annotates that a
/// US-classified document references foreign-government
/// information; it does not retroactively make the US-axis
/// classification valid. `Us(Restricted)` + any `fgi_marker` value
/// is still `Us(Restricted)`, still nonsense.
///
/// Used by both the strict recognizer and the decoder so the engine
/// never produces a `Us(Restricted)` marking, regardless of what
/// other tokens the input carried.
pub(crate) fn is_us_restricted(marking: &CapcoMarking) -> bool {
    matches!(
        marking.0.classification,
        Some(MarkingClassification::Us(Classification::Restricted))
    )
}

/// Shift every source-relative byte offset recorded inside `attrs` by
/// `delta`. Used by the engine to reconcile zero-origin spans produced
/// by a [`Recognizer`] (which sees only the candidate's slice of the
/// source) back to the full-source coordinates rules expect.
///
/// Only `CanonicalAttrs::token_spans` carries offsets today; if later
/// structural fields (SCI / SAR marker spans) start recording source
/// offsets, add the shift here — there is no alternative code path to
/// keep in sync.
///
/// Crate-visibility only: this is an engine-internal seam. The engine
/// is the only caller (PR-3's `DecoderRecognizer` will live in this
/// same crate and call it the same way). Exposing it outside the
/// crate would lock in an API surface before the `CanonicalAttrs`
/// span story is finished.
pub(crate) fn shift_token_spans(attrs: &mut CanonicalAttrs, delta: usize) {
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

/// CAB detection: the three line-initial CAPCO-2016 §E.1 / §E.2
/// authority heads. Rough but sufficient — the scanner already
/// filtered out anything that doesn't look like a classification
/// region, so byte-prefix matching on the known heads is reliable
/// here.
///
/// Heads recognized (with trailing colon, matching CAPCO-2016 §E.1 p31
/// and §E.2 p32 labels exactly):
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
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::sync::LazyLock;

    use super::*;

    /// Shared scheme instance for the test module. `CapcoScheme::new()`
    /// builds non-trivial `Vec` tables; borrowing `&*TEST_SCHEME` avoids
    /// repeated allocation across tests.
    static TEST_SCHEME: LazyLock<CapcoScheme> = LazyLock::new(CapcoScheme::new);

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
        match rx.recognize(b"(S//NF)", 0, &*TEST_SCHEME, &cx) {
            Parsed::Unambiguous(_) => {}
            other => panic!("expected Unambiguous, got {other:?}"),
        }
    }

    #[test]
    fn strict_recognizer_rejects_bare_restricted_portion() {
        // CAPCO §H.7: bare `(R)` is structurally indistinguishable from
        // a registered-mark glyph or list-item enumerator. RESTRICTED
        // requires foreign-origin context (FGI marker); without it the
        // strict path must NOT recognize the input as a marking, so
        // `is_us_restricted` collapses the marking to
        // zero-candidate Ambiguous.
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        match rx.recognize(b"(R)", 0, &*TEST_SCHEME, &cx) {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "bare (R) must be zero-candidate, got {} candidates",
                candidates.len()
            ),
            Parsed::Unambiguous(m) => panic!(
                "bare (R) must be rejected, got Unambiguous({:?})",
                m.0.classification
            ),
        }
    }

    #[test]
    fn strict_recognizer_rejects_restricted_with_dissem_only() {
        // `(R//NF)` parses to `Us(Restricted)` + `dissem_controls: [Nf]`,
        // no `fgi_marker`. Per CAPCO §H.7 the canonical form requires
        // a foreign-origin signal (FGI/tetragraph/trigraph) BEFORE the
        // R, not a dissem control AFTER. The predicate must reject so
        // a future refactor that loosened the FGI-marker check (e.g.,
        // by treating REL TO or NOFORN as foreign-origin evidence,
        // which they are not) is caught here.
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        match rx.recognize(b"(R//NF)", 0, &*TEST_SCHEME, &cx) {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "(R//NF) must be zero-candidate, got {} candidates",
                candidates.len()
            ),
            Parsed::Unambiguous(m) => panic!(
                "(R//NF) must be rejected — `Us(Restricted)` with dissem \
                 control but no FGI marker is invalid; got Unambiguous({:?})",
                m.0.classification
            ),
        }
    }

    #[test]
    fn strict_recognizer_rejects_restricted_with_rel_to_only() {
        // Banner-shape `R//USA, GBR` — same rejection rationale as
        // `(R//NF)`. REL TO populates `rel_to` but is not foreign-
        // origin evidence; `R` first is the bug-case `Us(Restricted)`.
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        match rx.recognize(b"R//USA, GBR", 0, &*TEST_SCHEME, &cx) {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "R//USA, GBR must be zero-candidate, got {} candidates",
                candidates.len()
            ),
            Parsed::Unambiguous(m) => panic!(
                "R//USA, GBR must be rejected — banner-shape \
                 `Us(Restricted)` with REL TO but no FGI marker is \
                 invalid; got Unambiguous({:?})",
                m.0.classification
            ),
        }
    }

    #[test]
    fn strict_recognizer_rejects_us_restricted_with_fgi_marker() {
        // `RESTRICTED//FGI DEU//NOFORN` is the parser shape that
        // led to the predicate's earlier `fgi_marker.is_none()`
        // hedge (PR #262 review). The strict parser sees `RESTRICTED`
        // first, lands `Us(Restricted)`, then parses the trailing
        // `FGI DEU` as the US-marking FGI block — producing
        // `classification: Us(Restricted), fgi_marker: Some([DEU])`.
        // The shape is still nonsense (a US doc cannot be RESTRICTED;
        // RESTRICTED is the foreign classification level), so the
        // recognizer must reject it. Pinning this case prevents a
        // future refactor from re-introducing an FGI-marker hedge
        // that would silently let `Us(Restricted)` slip through.
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        match rx.recognize(b"RESTRICTED//FGI DEU//NOFORN", 0, &*TEST_SCHEME, &cx) {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "RESTRICTED//FGI DEU//NOFORN must be zero-candidate, \
                 got {} candidates",
                candidates.len()
            ),
            Parsed::Unambiguous(m) => panic!(
                "RESTRICTED//FGI DEU//NOFORN must be rejected — an FGI \
                 marker block does not redeem a Us(Restricted) \
                 classification; got Unambiguous({:?}, fgi_marker={:?})",
                m.0.classification, m.0.fgi_marker
            ),
        }
    }

    #[test]
    fn strict_recognizer_accepts_fgi_axis_restricted() {
        // The legitimate foreign-origin RESTRICTED form `(//FGI R//NF)`
        // parses to `MarkingClassification::Fgi(level=Restricted)` —
        // the FGI classification axis, NOT `Us(Restricted)`. The
        // rejection predicate matches only on `Us(Restricted)`, so
        // this shape passes through and the strict recognizer
        // produces an Unambiguous marking. Real RESTRICTED markings
        // never reach the bug path the predicate gates against.
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        match rx.recognize(b"(//FGI R//NF)", 0, &*TEST_SCHEME, &cx) {
            Parsed::Unambiguous(m) => {
                assert!(
                    !is_us_restricted(&m),
                    "FGI-axis RESTRICTED must not match the bare-`Us(Restricted)` predicate; \
                     classification = {:?}",
                    m.0.classification,
                );
            }
            other => panic!("expected Unambiguous for `(//FGI R//NF)`, got {other:?}"),
        }
    }

    #[test]
    fn is_us_restricted_distinguishes_us_secret() {
        // Defensive: only `Us(Restricted)` triggers the rejection; other
        // US classifications (Secret, Confidential, Unclassified) are
        // unaffected because they are valid US-axis classifications
        // that don't require foreign-origin context.
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        let Parsed::Unambiguous(m) = rx.recognize(b"(S)", 0, &*TEST_SCHEME, &cx) else {
            panic!("(S) must parse to a SECRET portion");
        };
        assert!(
            !is_us_restricted(&m),
            "Us(Secret) must not match the bare-RESTRICTED predicate",
        );
    }

    #[test]
    fn strict_recognizer_returns_zero_candidate_on_parse_failure() {
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        // Missing closing paren — parser rejects; recognizer surfaces
        // zero-candidate Ambiguous per the trait contract.
        match rx.recognize(b"(S//NF", 0, &*TEST_SCHEME, &cx) {
            Parsed::Ambiguous { candidates } => assert!(candidates.is_empty()),
            other => panic!("expected zero-candidate Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn recognize_emits_zero_relative_spans_at_offset_zero() {
        // Issue #431: at `offset = 0` the recognizer's emitted
        // `token_spans` should land inside `bytes.len()` — i.e. the
        // spans are zero-relative because no source shift applied.
        // Pins the absolute-span contract's base case so a future
        // refactor that accidentally shifts by some non-zero default
        // breaks this test instead of silently misplacing diagnostics
        // for the most common engine-pin case (`StrictRecognizer`
        // called with `offset = 0`).
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        let input: &[u8] = b"(S//NF)";
        let Parsed::Unambiguous(marking) = rx.recognize(input, 0, &*TEST_SCHEME, &cx) else {
            panic!("strict parse should succeed");
        };
        assert!(
            !marking.0.token_spans.is_empty(),
            "expected at least one token span from `(S//NF)` — empty token_spans \
             would silently pass the loop below and prove nothing about the contract"
        );
        for ts in marking.0.token_spans.iter() {
            assert!(
                ts.span.start < input.len(),
                "span start {} must be inside bytes (len={}) at offset=0",
                ts.span.start,
                input.len(),
            );
            assert!(
                ts.span.end <= input.len(),
                "span end {} must be inside bytes (len={}) at offset=0",
                ts.span.end,
                input.len(),
            );
        }
    }

    #[test]
    fn recognize_emits_absolute_spans_at_nonzero_offset() {
        // Issue #431: when called with `offset = N`, every emitted
        // span must equal the corresponding `offset = 0` span shifted
        // by `N`. This is the absolute-source-coordinate contract the
        // engine relies on for its zero-post-pass behavior.
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        let Parsed::Unambiguous(at_zero) = rx.recognize(b"(S//NF)", 0, &*TEST_SCHEME, &cx) else {
            panic!("strict parse should succeed at offset=0");
        };
        let Parsed::Unambiguous(at_100) = rx.recognize(b"(S//NF)", 100, &*TEST_SCHEME, &cx) else {
            panic!("strict parse should succeed at offset=100");
        };
        assert_eq!(at_zero.0.token_spans.len(), at_100.0.token_spans.len());
        for (z, h) in at_zero
            .0
            .token_spans
            .iter()
            .zip(at_100.0.token_spans.iter())
        {
            assert_eq!(h.span.start, z.span.start + 100);
            assert_eq!(h.span.end, z.span.end + 100);
        }
    }

    #[test]
    fn recognize_handles_offset_plus_leading_whitespace() {
        // Issue #431 compound-shift gotcha: portion candidates strip
        // any leading ASCII whitespace before parsing, so the parser
        // emits spans relative to the post-strip slice. The recognizer
        // must compose BOTH the source-position `offset` AND the
        // leading-whitespace delta into a single shift on return —
        // skipping either produces an off-by-leading-whitespace bug
        // that this test catches. With `offset = 50` and 2 leading
        // spaces, every emitted span should land at
        // `50 + 2 + <inner offset>`.
        let rx = StrictRecognizer::new();
        let cx = ParseContext::default();
        // Reference: zero-offset, zero-leading-whitespace baseline
        // tells us where the parser puts each token without any shift.
        let Parsed::Unambiguous(baseline) = rx.recognize(b"(S//NF)", 0, &*TEST_SCHEME, &cx) else {
            panic!("baseline strict parse should succeed");
        };
        // Now run with leading whitespace AND a non-zero offset; both
        // deltas must be applied.
        let Parsed::Unambiguous(shifted) = rx.recognize(b"  (S//NF)", 50, &*TEST_SCHEME, &cx)
        else {
            panic!("leading-ws strict parse should succeed");
        };
        assert_eq!(baseline.0.token_spans.len(), shifted.0.token_spans.len());
        for (base, moved) in baseline
            .0
            .token_spans
            .iter()
            .zip(shifted.0.token_spans.iter())
        {
            assert_eq!(
                moved.span.start,
                base.span.start + 52,
                "expected shift = offset (50) + leading_ws (2) = 52"
            );
            assert_eq!(
                moved.span.end,
                base.span.end + 52,
                "expected shift = offset (50) + leading_ws (2) = 52"
            );
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
