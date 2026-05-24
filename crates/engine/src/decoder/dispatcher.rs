//! [`StrictOrDecoderRecognizer`] — the default engine recognizer.
//!
//! Runs the strict path first, falls back to the decoder when the
//! strict parse is empty or incomplete. Within this recognizer,
//! dispatch is keyed off `ParseContext::strict_evidence` so callers
//! that need strict-only behavior (the interactive-latency benchmark,
//! tests asserting strict behavior) can opt in at the context level
//! instead of the recognizer level.

use marque_capco::{CapcoMarking, CapcoScheme};
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{ParseContext, Recognizer};

use crate::recognizer::StrictRecognizer;

use super::recognizer::DecoderRecognizer;
use super::shape::{infer_marking_type, strict_parse_is_complete};

// ---------------------------------------------------------------------------
// Strict + decoder dispatcher
// ---------------------------------------------------------------------------

/// Recognizer that runs the strict path first and falls back to the
/// decoder when the strict parse yields no meaningful attributes.
///
/// Default recognizer installed by [`crate::Engine::new`]. Callers
/// that need strict-only dispatch (the interactive-latency benchmark,
/// tests asserting strict behavior) install [`StrictRecognizer`]
/// explicitly via [`crate::Engine::with_recognizer`].
///
/// Within this recognizer, dispatch is keyed off
/// [`ParseContext::strict_evidence`]:
///
/// - `strict_evidence = true`: collapse to strict-only behavior. The
///   decoder is not called. The engine never sets this; it's reserved
///   for callers (e.g., test code) that construct a `ParseContext`
///   directly and want to drive only the strict half of the dispatcher.
/// - `strict_evidence = false` (the engine default): try strict first.
///   Fall back to the decoder when the strict result is either (a)
///   zero-candidate `Ambiguous` or (b) `Unambiguous` with an empty /
///   trivial [`CapcoMarking`] (no classification, no SCI, no dissem,
///   no FGI, etc.). The trivial-Unambiguous case matters because
///   `marque_core::Parser` is lenient: it accepts arbitrary
///   `BYTES//BYTES` shapes and returns `Ok` with an empty
///   `CanonicalAttrs` when nothing in the input is a recognized CVE
///   token. Treating such a result as a successful parse would leave
///   the decoder dormant on exactly the mangled inputs it exists to
///   recover (`SERCET//NOFORN`, `NOFORN//SECRET`, …). The dispatcher
///   passes the caller's [`ParseContext`] through to both inner
///   recognizers unmodified — [`StrictRecognizer::recognize`] ignores
///   every field of `ParseContext` (its parameter is `_cx`), and by
///   the time the dispatcher reaches the decoder leg the
///   `cx.strict_evidence` early return above has already established
///   that the flag is `false`, so the previous
///   clone-with-`strict_evidence`-override was redundant.
///
/// Other [`ParseContext`] fields (`zone`, `position`,
/// `classification_floor`) pass through unchanged.
#[derive(Debug, Default, Clone, Copy)]
pub struct StrictOrDecoderRecognizer {
    strict: StrictRecognizer,
    decoder: DecoderRecognizer,
}

impl StrictOrDecoderRecognizer {
    pub const fn new() -> Self {
        Self {
            strict: StrictRecognizer::new(),
            decoder: DecoderRecognizer::new(),
        }
    }
}

impl Recognizer<CapcoScheme> for StrictOrDecoderRecognizer {
    fn recognize(
        &self,
        bytes: &[u8],
        offset: usize,
        scheme: &CapcoScheme,
        cx: &ParseContext,
    ) -> Parsed<CapcoMarking> {
        // Pass `cx` through to the strict recognizer unmodified.
        // `StrictRecognizer::recognize` ignores every field of
        // `ParseContext` (its parameter is `_cx`), so cloning to
        // override `strict_evidence = true` would be pure overhead on
        // the strict-complete fast path — which is every candidate in
        // a well-formed document. Forward `offset` verbatim — inner
        // recognizers do the shift, the dispatcher never double-shifts
        // (issue #431).
        let strict_result = self.strict.recognize(bytes, offset, scheme, cx);

        // When the outer caller asked for strict-only via
        // `strict_evidence = true`, collapse to the strict result —
        // never call the decoder. The engine never sets this flag (it
        // installs `StrictRecognizer` directly via `with_recognizer`
        // when a strict-only mode is needed); this branch exists for
        // direct callers that construct a `ParseContext` themselves
        // (e.g., test code).
        if cx.strict_evidence {
            return strict_result;
        }

        // Infer the candidate kind from the byte shape so
        // `strict_parse_is_complete` can apply the right rule
        // (classification-requiring for portion/banner, CAB-field-
        // requiring for CAB). If inference fails the bytes are too
        // degenerate for either path — skip and return whatever the
        // strict path produced (most likely zero-candidate Ambiguous).
        let Some(kind) = infer_marking_type(bytes) else {
            return strict_result;
        };

        // Complete strict parse — take it, decoder not needed.
        if matches!(&strict_result, Parsed::Unambiguous(m) if strict_parse_is_complete(m, kind)) {
            return strict_result;
        }

        // Strict already produced non-empty candidates — keep them.
        if matches!(&strict_result, Parsed::Ambiguous { candidates } if !candidates.is_empty()) {
            return strict_result;
        }

        // Remaining cases: either an incomplete-but-Unambiguous strict parse
        // (partial attrs, `TokenKind::Unknown` spans, missing classification,
        // etc.) or a zero-candidate strict Ambiguous. Both warrant a decoder
        // attempt. Cases:
        //   (a) Truly empty attrs (`FROBNITZ//WIBBLE`) — zero-candidate strict.
        //   (b) Partial attrs (`(SERCET//NOFORN)` — NOFORN parsed, SERCET
        //       left in a Classification-kind span with
        //       `attrs.classification = None`) — incomplete Unambiguous.
        //
        // Pass `cx` directly: the `cx.strict_evidence` early return
        // above guarantees the flag is already `false`, so the
        // previous clone-with-override was redundant.
        let decoder_result = self.decoder.recognize(bytes, offset, scheme, cx);

        // Only adopt the decoder result when it produced an Unambiguous
        // marking. If the decoder is also uncertain, preserve the strict
        // result so rules can still fire on any partial attrs — avoiding
        // deep-scan silently reducing observability/diagnostics on
        // mangled input.
        match decoder_result {
            Parsed::Unambiguous(_) => decoder_result,
            _ => strict_result,
        }
    }
}
