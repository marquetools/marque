// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use marque_scheme::{AuthoritativeSource, Citation, SectionLetter, SectionRef};

/// Provenance of a fix proposal — where the fix recommendation originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixSource {
    /// Hand-written Layer 2 CAPCO rule.
    BuiltinRule,
    /// User `[corrections]` entry (FR-009).
    CorrectionsMap,
    /// Deterministic deprecated-marking conversion (FR-004a).
    MigrationTable,
    /// Probabilistic decoder produced this fix from a recognition
    /// candidate's posterior (Phase D, see
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
    /// See issue #133 PR 2.
    ///
    /// The heuristic is inherently less certain than a fuzzy-vocab
    /// match because the inference is "this token is keyboard-
    /// adjacent to a known classification" rather than "this token
    /// is edit-distance ≤ 2 from a known canonical token in a
    /// closed vocabulary." The engine therefore (a) emits the
    /// diagnostic at [`crate::Severity::Warn`] (the fix-and-warn pattern —
    /// always visible, non-zero exit code in `--check`), and
    /// (b) caps [`crate::Confidence::rule`] at `0.80` so `combined ≤ 0.80`
    /// stays below the default `confidence_threshold` of `0.95`.
    /// The fix only auto-applies when the user has explicitly
    /// lowered the threshold to opt into the heuristic's bar.
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
/// PR 3c.2.C C5 migrated the type from `&'static str` → [`Citation`]
/// per `docs/plans/2026-05-20-pr3c2-c-pm-decisions.md` PM-C-4. The
/// C2 transitional `CORRECTIONS_MAP_CITATION_TYPED` alias was
/// consolidated into this canonical name.
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
// AppliedFix (= Audit Record) — v2 / marque-1.0
//
// Post PR 3c.2.D the canonical `AppliedFix<S>` type is the v2 shape in
// `marque_rules::audit::AppliedFix` (re-exported through this crate's
// prelude). The pre-cutover `AppliedFixProposal<S>` envelope retired
// atomically with the schema flip. See `crates/rules/src/audit.rs`
// for the active type definition + engine-promotion constructor.
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
/// Reused by `AuditNote::__engine_promote` (T108e) under the same
/// engine-only contract.
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
    /// # Reserved name (FR-040 lint contract)
    ///
    /// As with [`AppliedFix::__engine_promote`], the function name
    /// `__engine_construct` is reserved by the marque project. The
    /// `tools/promote-callsite-lint/` CI lint flags every call
    /// expression whose path's last segment is `__engine_construct`
    /// regardless of leading qualifier (qualified, fully-qualified,
    /// `Self::`, aliased, UFCS). Defining or calling another
    /// function with this exact name elsewhere will fail the lint.
    /// The `__` prefix + `#[doc(hidden)]` attribute reinforce the
    /// reserved status; see [`AppliedFix::__engine_promote`] for the
    /// full contract and the rationale for last-segment matching.
    ///
    /// # Engine-only contract (production code)
    ///
    /// Only `marque-engine` may call this in production code. The
    /// same three-constraint test-fixture carve-out from
    /// [`AppliedFix::__engine_promote`] applies here verbatim — see
    /// that constructor's doc comment for the binding definition.
    /// Outside the engine, calling this from `cfg(not(test))` code
    /// violates Constitution V Principle V.
    #[doc(hidden)]
    #[inline]
    pub const fn __engine_construct() -> Self {
        Self { _seal: () }
    }
}
