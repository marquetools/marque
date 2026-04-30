// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Marking recognizers.
//!
//! A [`Recognizer`] is the pluggable first stage of the engine — it
//! turns a byte slice and a small [`ParseContext`] into a
//! [`Parsed<S::Marking>`](Parsed). The engine dispatches through
//! `Box<dyn Recognizer<S>>` so a strict-path recognizer (zero-FP,
//! header-only) and a deep-scan probabilistic recognizer (the Phase D
//! decoder) can coexist behind the same call site — see
//! `docs/plans/2026-04-16-probabilistic-recognition.md` for the full
//! design.
//!
//! The trait is deliberately **domain-neutral**: it depends only on
//! the scheme's `Marking` type plus the `Parsed` / `Candidate` /
//! `EvidenceFeature` primitives already in [`crate::ambiguity`]. No
//! ISM-specific structures (`IsmAttributes`, `Span`, etc.) leak
//! through. Scheme adapters wrap their concrete parsers as
//! `impl Recognizer<S>` (Phase 4 / task T058 for `StrictRecognizer`
//! and T061 for `DecoderRecognizer`).
//!
//! # Zero-candidate is not silent fallthrough
//!
//! When a recognizer finds no plausible interpretation, the answer is
//! `Parsed::Ambiguous { candidates: vec![] }` — never `Unambiguous`
//! with a sentinel. This keeps the engine from acting on a silently
//! fabricated marking (foundational-plan line 609-612). The decoder
//! inspects the candidate set to decide whether to surface a
//! recognition diagnostic, not to invent one.

use std::sync::Arc;

use crate::ambiguity::Parsed;
use crate::scheme::MarkingScheme;

/// Where in the surrounding document the recognizer is being run.
///
/// Scheme-neutral equivalent of [`marque_ism::Zone`]. Recognizers that
/// need to know whether they're looking at a banner in a header vs. a
/// portion in body text read this field rather than the ISM-specific
/// type. Keeping the enum here lets non-CAPCO schemes (CUI, NATO,
/// future frameworks) reuse the same recognizer surface without
/// pulling in `marque-ism`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// Document header (first `N` lines, or structurally-marked header).
    Header,
    /// Document footer.
    Footer,
    /// Body text.
    Body,
    /// Classification Authority Block (Classified By / Derived From /
    /// Declassify On).
    Cab,
}

/// Coarse position within the document.
///
/// Scheme-neutral equivalent of [`marque_ism::DocumentPosition`]. Used
/// by banner-detection heuristics and corpus-analysis tooling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentPosition {
    Start,
    Body,
    End,
}

/// Context a [`Recognizer`] reads alongside the bytes it recognizes.
///
/// The fields are all `Option` because the engine cannot always prove
/// a zone or a document position up front — a naked `&[u8]` the WASM
/// callers pass in has neither. Recognizers MUST handle `None` without
/// hardcoding a default; that is the FR-023 contract (Constitution VI,
/// Phase-3 Phase-typing invariants — see `CLAUDE.md`).
///
/// `strict_evidence` is the signal the engine sets to `true` when it
/// wants the recognizer to take the zero-false-positive strict path
/// (SC-001). The Phase-D `DecoderRecognizer` returns an empty
/// candidate set when `strict_evidence` is set and the input does not
/// match the strict grammar — that's how strict-path latency stays
/// linear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseContext {
    /// When `true`, the recognizer must not emit probabilistic
    /// candidates — only parses that hit the strict grammar.
    pub strict_evidence: bool,
    /// Document zone the recognizer is running in, when known.
    pub zone: Option<Zone>,
    /// Coarse document position, when known.
    pub position: Option<DocumentPosition>,
    /// Minimum classification rank established by strict-path evidence
    /// elsewhere in the document. Recognizers that need to honor a
    /// classification-level floor (e.g., FR-011: if any portion on the
    /// page is CONFIDENTIAL-or-higher, `(C)` must not resolve to a
    /// below-CONFIDENTIAL candidate) read this field; strict
    /// recognizers ignore it.
    ///
    /// The rank encoding is scheme-specific — the trait can't know
    /// what a classification axis even looks like. For CAPCO / ISM the
    /// convention matches `marque_ism::Classification as u8`
    /// (`Unclassified=0, Restricted=1, Confidential=2, Secret=3,
    /// TopSecret=4`). A non-ISM scheme using this field must document
    /// its own encoding and stay consistent within the scheme.
    ///
    /// `None` when the engine has not established a strict floor for
    /// the current region (e.g., isolated single-region recognition,
    /// or the page has no strict-path portion seen yet).
    pub classification_floor: Option<u8>,
    /// Reference date for temporal membership queries (Phase 3 plumbing).
    ///
    /// When set, rules that evaluate time-limited memberships (e.g.,
    /// tetragraph membership active as of a particular date) use this as the
    /// evaluation anchor instead of the current wall-clock time.
    ///
    /// Stored as an ISO 8601 date string wrapped in `Arc<str>` so that
    /// `ParseContext::clone()` in the recognizer hot path (e.g.
    /// `StrictOrDecoderRecognizer` uses `..cx.clone()`) never allocates
    /// even when `as_of` is `Some` — the `Arc` clone is a single atomic
    /// increment. `marque-scheme` stays free of a runtime dependency on
    /// `marque-ism`; callers in `marque-capco`/`marque-engine` can parse
    /// it with `marque_ism::IsmDate::from_str`.
    ///
    /// Currently `None` everywhere — no behavior change until the
    /// membership-uncertain diagnostic (issue #206) is implemented.
    pub as_of: Option<Arc<str>>,
    /// Whether the byte immediately preceding the candidate's source
    /// span is whitespace (or the candidate sits at offset 0).
    ///
    /// Decoder-path heuristic for separating real single-letter portion
    /// markings (`(s)`, `(c)`) from prose glyphs glued to a word like
    /// `letter(s)` or `function(c)`. The strict recognizer ignores this
    /// flag — it only matters for the probabilistic recovery path,
    /// where a bare `(s)` candidate that is glued to a preceding word
    /// is overwhelmingly a plural-suffix and not a marking the decoder
    /// should canonicalize.
    ///
    /// Convention: `true` at the start of the source buffer
    /// (`start == 0`). Column zero is structurally similar to
    /// whitespace-preceded — banners and centered captions both start
    /// there — and the boolean default avoids pushing tri-state
    /// handling onto every reader.
    ///
    /// Bullets / numbered lists / `(a)` enumeration markers are not a
    /// problem for this heuristic: they always have whitespace between
    /// the marker and a following marking (`1. (S)`, `* (S//NF)`,
    /// `(a) (S)` all have a space before the `(` of the marking).
    pub preceded_by_whitespace: bool,
}

impl Default for ParseContext {
    /// Default context: strict path, no zone / position evidence, no
    /// strict classification floor, no temporal anchor, and a default
    /// of "preceded by whitespace" (matches start-of-buffer convention,
    /// which is the safer default — a recognizer that doesn't know its
    /// position behaves the same as one at column zero).
    ///
    /// **`ParseContext` defaults to `strict_evidence: true` regardless
    /// of how the engine dispatches.** Two layers, two different
    /// defaults:
    ///
    /// - **`ParseContext` default** (this `impl Default`): strict-only.
    ///   Direct callers of a `Recognizer` (test code, fixture
    ///   construction) get the conservative answer — no probabilistic
    ///   candidates — unless they explicitly set `strict_evidence: false`.
    /// - **`Engine` dispatch default** (`Engine::new` →
    ///   `StrictOrDecoderRecognizer`): strict-first / decoder-fallback.
    ///   The engine populates `ParseContext` per-candidate with
    ///   `strict_evidence: false` so its installed dispatcher can fall
    ///   back to the decoder on a strict-parse zero-candidate. Callers
    ///   that need strict-only `Engine` behavior install
    ///   `StrictRecognizer` via `Engine::with_recognizer` — that swaps
    ///   the recognizer object, not this `ParseContext` flag.
    ///
    /// Don't infer engine behavior from this default: the
    /// `strict_evidence` flag is consumed by the recognizer object the
    /// engine has installed, not by the engine itself.
    fn default() -> Self {
        Self {
            strict_evidence: true,
            zone: None,
            position: None,
            classification_floor: None,
            as_of: None,
            preceded_by_whitespace: true,
        }
    }
}

/// A pluggable marking recognizer.
///
/// Implementations MUST be `Send + Sync` so the engine can hold them
/// behind `Arc` inside `BatchEngine` without re-instantiation per
/// document (Constitution VI, FR-023).
///
/// # Contract
///
/// - Return `Parsed::Unambiguous(m)` when the input has exactly one
///   plausible interpretation with posterior ≥ the strict-path floor
///   (FR-011) or, in deep-scan mode, above the decoder's
///   configured threshold.
/// - Return `Parsed::Ambiguous { candidates }` with **≥ 2** candidates
///   when the input is genuinely ambiguous (the canonical `(C)`
///   copyright-vs-CONFIDENTIAL case is the primary producer here).
/// - Return `Parsed::Ambiguous { candidates: vec![] }` when no
///   candidate clears the recognition floor. **Never** return
///   `Unambiguous` with a sentinel marking to signal "not found" —
///   that is the silent-fallthrough anti-pattern the recognition plan
///   forbids (foundational-plan line 609-612).
pub trait Recognizer<S: MarkingScheme + ?Sized>: Send + Sync {
    /// Recognize a marking from raw bytes.
    ///
    /// `bytes` is the raw input slice (zero-copy; spans are by offset
    /// into this buffer — see [`crate::template`] for how templates
    /// position sub-spans).
    fn recognize(&self, bytes: &[u8], cx: &ParseContext) -> Parsed<S::Marking>;
}
