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
//! ISM-specific structures (`CanonicalAttrs`, `Span`, etc.) leak
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
    /// `ParseContext::clone()` never allocates even when `as_of` is
    /// `Some` — the `Arc` clone is a single atomic increment. (The
    /// default dispatcher `StrictOrDecoderRecognizer` no longer clones
    /// per candidate, but consumers that do still benefit.)
    /// `marque-scheme` stays free of a runtime dependency on
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
    /// Byte offset of the candidate's span from the start of its
    /// containing line.
    ///
    /// `Some(0)` means the candidate begins at column 0; higher values
    /// mean further into the line. `None` means the engine could not
    /// determine — recognizers treat this as "no position-based
    /// evidence available" and skip the position-feature delta.
    ///
    /// Used by the decoder's [`LinePositionPenalty`](crate)/
    /// [`BulletAnchorBonus`](crate) features: a portion marking deep
    /// into a line of running text is overwhelmingly prose, while a
    /// portion following a bullet or enumeration anchor
    /// (`1B.a.3.(c)`, `* (S//NF)`, `(a) (S)`) is legitimate IC content.
    pub line_offset: Option<usize>,
    /// Up to 32 bytes of the same line preceding the candidate's span.
    ///
    /// `None` when no caller populated the field — direct
    /// `ParseContext::default()` callers (test code, direct WASM
    /// embedders, future schemes without source-byte access) leave
    /// this `None`; the engine's per-candidate loop in
    /// `Engine::lint_inner` always populates it with `Some(prefix)`
    /// (possibly empty). Carries the **trailing** bytes of the line
    /// prefix (closest to the candidate) when the on-line prefix is
    /// longer than 32 bytes — those bytes are what determines whether
    /// the candidate sits behind a bullet or section anchor.
    ///
    /// **Convention, not enforced**: when both fields are `Some(_)`,
    /// the engine guarantees `line_prefix.len() == min(line_offset,
    /// 32)` (the prefix is sliced from `source[line_start..span_start]`
    /// and then capped at 32 bytes). Hand-built `ParseContext` callers
    /// can in principle desync the two fields (e.g., set `line_offset:
    /// Some(0)` but stuff a non-empty `LinePrefix`); no decoder code
    /// today fires on the desync case, but downstream consumers should
    /// not rely on the invariant beyond "engine-populated values are
    /// consistent."
    ///
    /// Used by the decoder's [`BulletAnchorBonus`](crate) feature to
    /// cancel the line-position penalty when the prefix looks like a
    /// legitimate enumeration anchor (`1.`, `(a)`, `1B.a.3.`, `* `,
    /// `- `).
    pub line_prefix: Option<LinePrefix>,
    /// Whether the source bytes surrounding the candidate (within a
    /// short window before and after) are predominantly lowercase
    /// ASCII letters.
    ///
    /// Computed by the engine when constructing the per-candidate
    /// `ParseContext`. Recognizers without source-window access leave
    /// this `false`.
    ///
    /// Used by the decoder's
    /// [`LowercaseSurroundingContext`](crate) feature: a lowercase
    /// portion or banner candidate embedded in lowercase prose is
    /// overwhelmingly a prose glyph (`(s)`, `(c)`, parenthetical
    /// asides, copyright) rather than a real marking the decoder
    /// should recover. Archival all-caps documents short-circuit this
    /// feature naturally — the candidate itself would be uppercase, so
    /// the candidate-has-lowercase predicate the decoder pairs with
    /// this flag never trips.
    pub surrounding_is_lowercase: bool,
}

/// Up to 32 bytes of same-line context preceding a candidate.
///
/// Stack-only (`33`-byte inline buffer) so cloning a [`ParseContext`]
/// in the recognizer hot path (e.g. `StrictOrDecoderRecognizer`'s
/// strict→decoder fallback) never allocates. The byte slice is
/// addressed via [`LinePrefix::as_slice`]; the underlying
/// `[u8; 32]` is implementation detail.
///
/// When the actual line prefix is longer than 32 bytes, the
/// **trailing** 32 bytes are kept — the bytes closest to the
/// candidate's `(` are the ones that determine whether a bullet or
/// enumeration anchor precedes the marking. The leading bytes of a
/// long prefix carry no useful signal for the bullet/anchor
/// heuristic.
#[derive(Debug, Clone, Copy)]
pub struct LinePrefix {
    bytes: [u8; 32],
    len: u8,
}

impl LinePrefix {
    /// Construct an empty prefix (zero bytes, column-0 candidate).
    #[inline]
    pub const fn empty() -> Self {
        Self {
            bytes: [0; 32],
            len: 0,
        }
    }

    /// Build a `LinePrefix` from a slice. When `slice.len() > 32`,
    /// keeps the trailing 32 bytes — the bytes immediately preceding
    /// the candidate carry the bullet/anchor signal; leading bytes do
    /// not.
    pub fn from_slice(slice: &[u8]) -> Self {
        const CAP: usize = 32;
        let mut bytes = [0u8; CAP];
        let take = slice.len().min(CAP);
        // `take as u8` is safe today because `take ≤ CAP = 32 ≤
        // u8::MAX`. The `debug_assert!` keeps the invariant loud
        // in dev builds if `CAP` is ever raised past `u8::MAX`
        // without a coordinated `len` field-type widening.
        debug_assert!(take <= u8::MAX as usize);
        let src_start = slice.len() - take;
        bytes[..take].copy_from_slice(&slice[src_start..]);
        Self {
            bytes,
            len: take as u8,
        }
    }

    /// View the prefix bytes.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// Number of bytes stored.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// `true` if the prefix is empty (candidate is at column 0).
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl PartialEq for LinePrefix {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for LinePrefix {}

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
            line_offset: None,
            line_prefix: None,
            surrounding_is_lowercase: false,
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
    /// `bytes` is the raw input slice (zero-copy). `offset` is the byte
    /// position of `bytes[0]` in the original source buffer; recognizers
    /// MUST emit spans in absolute source coordinates (i.e., add `offset`
    /// to any zero-relative span the inner parser produces). This lets
    /// the engine deliver source-coordinate spans to rules without an
    /// O(token_spans) post-pass per recognized candidate.
    ///
    /// `offset` is a per-call argument (the candidate's source position)
    /// rather than a `ParseContext` field because `ParseContext` is the
    /// scheme-neutral *environment* shared across calls inside dispatch
    /// wrappers like the engine's strict-then-decoder recognizer; folding
    /// offset into the environment would corrupt that split.
    ///
    /// `scheme` is the marking scheme instance. Recognizers that need to
    /// call scheme methods (e.g., `MarkingScheme::canonicalize`) receive
    /// the scheme here rather than keeping a module-scope static
    /// (`LazyLock<CapcoScheme>`). The engine passes `&self.scheme`;
    /// direct recognizer callers (test code, WASM embedders) construct
    /// and pass their own instance. `scheme` is positioned before `cx`
    /// to group the data parameters (`bytes`, `offset`, `scheme`) before
    /// the environment parameter (`cx`).
    fn recognize(
        &self,
        bytes: &[u8],
        offset: usize,
        scheme: &S,
        cx: &ParseContext,
    ) -> Parsed<S::Marking>;
}
