// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use marque_scheme::{AuthoritativeSource, Citation, SectionLetter, SectionRef};

/// Provenance of a fix proposal — where the fix recommendation originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixSource {
    /// Hand-written Layer 2 CAPCO rule.
    BuiltinRule,
    /// User `[corrections]` entry.
    CorrectionsMap,
    /// Deterministic deprecated-marking conversion.
    MigrationTable,
    /// Probabilistic decoder produced this fix from a recognition
    /// candidate's posterior (see
    /// `docs/plans/2026-04-16-probabilistic-recognition.md`). Paired
    /// with a non-trivial `features` list in
    /// [`crate::FixIntent::confidence`] so auditors can reconstruct the
    /// scoring path.
    DecoderPosterior,
    /// Decoder produced this fix via a position-aware short-token
    /// classification heuristic — a keyboard-proximity table applied
    /// to the leading classification slot of a portion or banner
    /// marking when the token is too short for vocab-based fuzzy
    /// matching (e.g., `(YS//NF) → (TS//NF)`, `(W//NF) → (S//NF)`).
    /// See issue #133.
    ///
    /// The heuristic is inherently less certain than a fuzzy-vocab
    /// match because the inference is "this token is keyboard-
    /// adjacent to a known classification" rather than "this token
    /// is edit-distance ≤ 2 from a known canonical token in a
    /// closed vocabulary." The engine therefore (a) emits the
    /// diagnostic at [`crate::Severity::Warn`] (the fix-and-warn pattern —
    /// always visible, non-zero exit code in `--check`), and
    /// (b) caps the sole surviving recognition axis at
    /// `HEURISTIC_RECOGNITION_CAP = 0.95` — exactly the default
    /// `confidence_threshold` — so a single-candidate heuristic fix
    /// lands at-threshold rather than saturating above it. See
    /// `marque-engine::engine::synthesis::HEURISTIC_RECOGNITION_CAP`
    /// for the cap's authoritative doc.
    DecoderClassificationHeuristic,
}

/// Canonical [`Citation`] for diagnostics whose authority is the user's
/// `[corrections]` config entry (C001 and the engine's pre-scanner text-scan
/// path). C001 is not a CAPCO rule — no CAPCO passage governs user-defined
/// typo replacements — so the citation uses the
/// [`AuthoritativeSource::Config`] sentinel and renders as `[config]`.
///
/// Holding the value in one place prevents silent drift between the
/// rule-pipeline emission site in `marque-capco` and the pre-scanner
/// emission site in `marque-engine`; both paths produce the same
/// audit-record shape.
///
pub const CORRECTIONS_MAP_CITATION: Citation = Citation::new(
    AuthoritativeSource::Config,
    SectionRef::new(SectionLetter::A),
    // Niche-sentinel page value — never rendered (Display elides
    // section/page when source is non-CAPCO).
    match core::num::NonZeroU16::new(1) {
        Some(n) => n,
        None => unreachable!(),
    },
);

// ---------------------------------------------------------------------------
// AppliedFix (= Audit Record)
//
// The canonical `AppliedFix<S>` type lives in
// `marque_rules::audit::AppliedFix` (re-exported through this crate's
// prelude). See `crates/rules/src/audit.rs` for the active type
// definition + engine-promotion constructor.
// ---------------------------------------------------------------------------

/// Engine-only proof-of-construction token for [`crate::AppliedFix::__engine_promote`].
///
/// `AppliedFix::__engine_promote` accepts an `EnginePromotionToken`; the
/// only way to obtain one is [`EnginePromotionToken::__engine_construct`].
/// Because the token's sole field is private to `marque-rules`, no
/// external crate can brace-construct one, and the constructor is
/// `#[doc(hidden)]` and named to make the bypass intent obvious at the
/// call site.
///
/// Reused by `AuditNote::__engine_promote` under the same engine-only
/// contract.
///
/// This is the type-level seal for Constitution V Principle V's
/// engine-only contract on audit-record promotion. See
/// [`crate::AppliedFix::__engine_promote`] for the binding contract and the
/// test-fixture carve-out.
///
/// # Compile-fail proof of the seal
///
/// External crates cannot brace-construct an `EnginePromotionToken`
/// because the `_seal` field is private to `marque-rules`. Doctests
/// compile as separate crates against the library's public API, so
/// the following snippet is rejected by the compiler — which is what
/// `compile_fail` asserts:
///
/// ```compile_fail
/// // External crates see `EnginePromotionToken` but not `_seal`,
/// // so brace-construction is rejected. Bypass requires calling
/// // `EnginePromotionToken::__engine_construct()`, which is the
/// // single auditable bypass surface.
/// let _token = marque_rules::EnginePromotionToken { _seal: () };
/// ```
#[derive(Debug)]
pub struct EnginePromotionToken {
    _seal: (),
}

impl EnginePromotionToken {
    /// Mint an [`EnginePromotionToken`].
    ///
    /// # Reserved name (promote-callsite lint contract)
    ///
    /// As with [`crate::AppliedFix::__engine_promote`], the function name
    /// `__engine_construct` is reserved by the marque project. The
    /// `tools/promote-callsite-lint/` CI lint flags every call
    /// expression whose path's last segment is `__engine_construct`
    /// regardless of leading qualifier (qualified, fully-qualified,
    /// `Self::`, aliased, UFCS). Defining or calling another
    /// function with this exact name elsewhere will fail the lint.
    /// The `__` prefix + `#[doc(hidden)]` attribute reinforce the
    /// reserved status; see [`crate::AppliedFix::__engine_promote`] for the
    /// full contract and the rationale for last-segment matching.
    ///
    /// # Engine-only contract (production code)
    ///
    /// Only `marque-engine` may call this in production code. The
    /// same three-constraint test-fixture carve-out from
    /// [`crate::AppliedFix::__engine_promote`] applies here verbatim — see
    /// that constructor's doc comment for the binding definition.
    /// Outside the engine, calling this from `cfg(not(test))` code
    /// violates Constitution V Principle V.
    #[doc(hidden)]
    #[inline]
    pub const fn __engine_construct() -> Self {
        Self { _seal: () }
    }
}
