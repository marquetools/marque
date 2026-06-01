// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::{FixIntent, FixSource, Message, Recognition, RuleId, Severity};
use marque_scheme::{Citation, MarkingScheme, Span};
use smol_str::SmolStr;

/// A single diagnostic emitted by a rule check.
///
/// # Generic over the marking scheme
///
/// `Diagnostic<S>` carries a scheme-typed [`FixIntent<S>`] in its
/// `fix` field — the sole fix-emission channel. The engine-side
/// `AppliedFix<S>` audit record is reshaped separately; the rule-side
/// `Diagnostic<S>.fix` channel is independent of that shape.
#[non_exhaustive]
#[derive(Debug)]
pub struct Diagnostic<S: MarkingScheme> {
    pub rule: RuleId,
    pub severity: Severity,
    /// Byte span in the original source buffer.
    pub span: Span,
    /// Optional marking-scope span (full portion or banner) when the
    /// `span` field points at a sub-region (e.g., a single token).
    /// Rules whose [`Self::fix`] payload is a `FixIntent` set this
    /// from [`crate::RuleContext::candidate_span`] so the engine's
    /// intent-synthesis path knows which scope-bytes to re-render
    /// via [`marque_scheme::MarkingScheme::apply_intent`] +
    /// [`marque_scheme::MarkingScheme::render_canonical`].
    ///
    /// `None` when the diagnostic's `span` already covers the full
    /// scope.
    pub candidate_span: Option<Span>,
    /// Closed-template description of the violation.
    ///
    /// A closed-template [`Message`] rather than a free-form string,
    /// closing the `format!`-into-`Diagnostic.message` leak channel
    /// called out by Constitution V Principle V. Audit emitters render
    /// via [`Message::template`] + [`Message::args`] accessors; there
    /// is no `Display` impl on [`Message`] (the compile-fail doctest at
    /// `message.rs` pins that absence).
    pub message: Message,
    /// Typed citation to the authoritative source passage.
    ///
    /// A typed [`Citation`]. [`Citation::Display`] emits the canonical
    /// citation-lint regex form (`§<L>[.<sub>] [Table <N>] p<page>`)
    /// for CAPCO citations and a bare `[<source>]` tag for non-CAPCO
    /// sentinels ([`marque_scheme::AuthoritativeSource::Config`] /
    /// [`marque_scheme::AuthoritativeSource::EngineInternal`]).
    pub citation: Citation,
    /// Structural fix intent, if the rule can generate one. `None`
    /// for diagnostics that consciously decline to propose a fix
    /// (e.g. opaque-uncertain reductions), for informational
    /// diagnostics, or for text-correction diagnostics (which carry
    /// their replacement bytes in [`Self::text_correction`] instead).
    pub fix: Option<FixIntent<S>>,
    /// Engine-applied byte-substitution payload (the C001 corrections-map
    /// path, plus the closely-shaped E006 deprecation-migration path).
    ///
    /// Carries the canonical replacement bytes plus the fix's
    /// provenance (`source`, `confidence`, `migration_ref`) so the
    /// engine's `apply_text_corrections` path can promote the fix
    /// with the rule's true provenance instead of hardcoding
    /// `FixSource::CorrectionsMap` for every text-correction. The
    /// replacement bytes are corpus-derived canonical tokens (e.g.
    /// `"SECRET"` replacing the typo `"SERCET"`, or `"NOFORN"`
    /// replacing the deprecated `"FOUO"`) — on Constitution V's
    /// permitted-identifier list. Never carries original document
    /// bytes.
    pub text_correction: Option<TextCorrection>,
    /// Decoder-recognized canonical bytes for the marking span.
    ///
    /// Issue #699: surface what the decoder recognized when the
    /// `DecoderRecognizer` emits R001 (`engine:recognition.decoder-
    /// recognized`) so users can see the canonical form in `marque
    /// check` output without running `marque fix` and diffing bytes.
    /// Populated by marque-capco's `build_decoder_diagnostic` helper from
    /// `DecoderProvenance::canonical_bytes` after both the UTF-8-validity
    /// and no-op-rewrite gates pass — implying every emitted R001
    /// carries `Some(_)` and no other rule emission populates the
    /// field today.
    ///
    /// # Surface scope
    ///
    /// This field is decoder-side by design. Other rules emitting
    /// `Recanonicalize` intents (E064 EYES → REL TO, E065 deprecated
    /// SCI long-form, E066 ATOMAL / NATO-SAP, E060 ordering) keep
    /// `recognized_canonical: None`; their canonical bytes are
    /// synthesized by the engine's intent-application path at
    /// promotion time. A future PR may opt those rules in if there
    /// is a concrete user-facing rendering need — but the entry
    /// point is here, not in those rules' bodies.
    ///
    /// # Constitution Principle II
    ///
    /// Stored as [`secrecy::SecretSlice<u8>`] (the alias for the
    /// unsized `SecretBox<[u8]>`) so the canonical bytes wipe on drop
    /// and every readout site goes through `expose_secret()` —
    /// greppable for security review, returning `&[u8]` directly.
    /// This is the same wrapper that backs `FixResult.source`; the
    /// lint and fix output surfaces use one shared content-bearing
    /// type. Readout sites today: the CLI human renderer
    /// (`render_human`), the CLI NDJSON projection
    /// (`diagnostic_to_json`), and the WASM NDJSON mirror.
    ///
    /// # Constitution V Principle V (audit-content-ignorance)
    ///
    /// The field is **lint-side only**. It MUST NOT be serialized
    /// into `AppliedFix<S>` or any audit-record JSON projection;
    /// audit records continue to carry the BLAKE3 digest of the
    /// canonical bytes plus the structural `Recanonicalize` intent,
    /// never the bytes themselves. The asymmetry is pinned by the
    /// `lint_carries_recognized_canonical_fix_audit_does_not` test
    /// in `crates/engine/tests/recognized_canonical_lint_vs_fix.rs`.
    pub recognized_canonical: Option<secrecy::SecretSlice<u8>>,
}

/// Payload for an engine-applied byte-substitution fix.
///
/// Populated on [`Diagnostic::text_correction`] by rules whose repair
/// is a literal byte substitution that the engine applies atomically
/// in its pre-scanner pass. Carries the rule's provenance so the
/// engine's promotion path produces a faithful audit record (without
/// silently overwriting `FixSource` / `Recognition` / `migration_ref`).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TextCorrection {
    /// Canonical replacement bytes. On Constitution V's permitted-
    /// identifier list (token canonicals from a closed vocabulary).
    pub replacement: SmolStr,
    /// Provenance of the fix.
    pub source: FixSource,
    /// Recognition-axis confidence. Threshold-gated like any other
    /// fix in the engine's promotion path.
    pub confidence: Recognition,
    /// Reference to the migration document or CAPCO row justifying
    /// this fix (e.g., a `§F p…` cite for E006 deprecations).
    pub migration_ref: Option<&'static str>,
}

// Manual Clone for Diagnostic<S> — see the parallel Clone impl on
// `AppliedFix<S>` for the rationale. The derive would over-constrain
// to `S: Clone`; the manual impl works for any well-formed scheme
// because the only S-typed payload is `FixIntent<S>` (which is
// `Clone` by its own derive).
impl<S: MarkingScheme> Clone for Diagnostic<S> {
    fn clone(&self) -> Self {
        // `SecretSlice<u8>` blocks Clone by design (Constitution
        // Principle II — content-bearing buffers must not silently
        // duplicate without an auditable readout). We expose-and-rewrap:
        // every Diagnostic clone is a sanctioned readout of the
        // decoder-recognized canonical bytes, and the new SecretSlice
        // wipes on drop just like the original.
        let recognized_canonical = self.recognized_canonical.as_ref().map(|sb| {
            // Principle II readout — Diagnostic clone path (issue #699).
            // `expose_secret()` on a `SecretSlice<u8>` returns `&[u8]`;
            // `Box::from(&[u8])` produces a fresh `Box<[u8]>`; and
            // `SecretBox::new(Box<[u8]>)` wraps it back up. `SecretSlice<u8>`
            // is the type alias for `SecretBox<[u8]>` — there is no
            // separate `SecretSlice::new` constructor.
            secrecy::SecretBox::new(Box::from(secrecy::ExposeSecret::expose_secret(sb)))
        });
        Self {
            rule: self.rule,
            severity: self.severity,
            span: self.span,
            candidate_span: self.candidate_span,
            message: self.message.clone(),
            citation: self.citation,
            fix: self.fix.clone(),
            text_correction: self.text_correction.clone(),
            recognized_canonical,
        }
    }
}

impl<S: MarkingScheme> Diagnostic<S> {
    /// Construct a new diagnostic carrying a structural
    /// [`FixIntent<S>`] (or `None`).
    ///
    /// Alias for [`Self::with_fix`] kept for call-site ergonomics:
    /// `Diagnostic::new(...)` reads more naturally than
    /// `Diagnostic::with_fix(...)` when the `fix` arg is `None`.
    /// Behavior is identical to [`Self::with_fix`].
    pub fn new(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: Message,
        citation: Citation,
        fix: Option<FixIntent<S>>,
    ) -> Self {
        Self::with_fix(rule, severity, span, message, citation, fix)
    }

    /// Construct a new diagnostic carrying a structural
    /// [`FixIntent<S>`] (or `None`).
    ///
    /// The sole fix-attached constructor. Rules that emit a
    /// diagnostic with no fix pass `None`.
    pub fn with_fix(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: Message,
        citation: Citation,
        fix: Option<FixIntent<S>>,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            candidate_span: None,
            message,
            citation,
            fix,
            text_correction: None,
            recognized_canonical: None,
        }
    }

    /// Construct a new diagnostic carrying a structural
    /// [`FixIntent<S>`] anchored at a marking-scope span.
    ///
    /// Identical to [`Self::with_fix`] but also populates
    /// [`Self::candidate_span`] from [`crate::RuleContext::candidate_span`].
    /// Use when:
    ///
    /// - The diagnostic's `span` points at a *sub-region* of the
    ///   marking (e.g., a single token within a portion) — the
    ///   sub-span tells the user *where* the violation is, but the
    ///   engine needs the full marking-scope span to replace the
    ///   re-rendered output.
    /// - The rule emits a `Recanonicalize` or per-fact intent that
    ///   the engine synthesizes the replacement bytes for at
    ///   promotion time.
    pub fn with_fix_at_span(
        rule: RuleId,
        severity: Severity,
        span: Span,
        candidate_span: Span,
        message: Message,
        citation: Citation,
        fix: FixIntent<S>,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            candidate_span: Some(candidate_span),
            message,
            citation,
            fix: Some(fix),
            text_correction: None,
            recognized_canonical: None,
        }
    }

    /// Construct a new C001 text-correction diagnostic.
    ///
    /// Used by the engine's pre-scanner aho-corasick scan and by
    /// the CAPCO `CorrectionsMapRule`. The replacement bytes are
    /// carried in [`Self::text_correction`]; the engine's
    /// `apply_text_corrections` reads this field and promotes it
    /// to an [`crate::AppliedTextCorrection`] via
    /// [`crate::AppliedTextCorrection::__engine_promote_text_correction`].
    // 9 args is the irreducible carrying capacity of a text-correction
    // diagnostic: id/severity/span/message/citation for the diagnostic
    // surface + replacement/source/confidence/migration_ref for the
    // engine's promotion path. Constructing this via a builder would
    // shift the same parameter count onto the builder's `.with_*`
    // methods without reducing it.
    #[allow(clippy::too_many_arguments)]
    pub fn text_correction(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: Message,
        citation: Citation,
        replacement: impl Into<SmolStr>,
        source: FixSource,
        confidence: Recognition,
        migration_ref: Option<&'static str>,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            candidate_span: None,
            message,
            citation,
            fix: None,
            text_correction: Some(TextCorrection {
                replacement: replacement.into(),
                source,
                confidence,
                migration_ref,
            }),
            recognized_canonical: None,
        }
    }

    /// Attach the decoder-recognized canonical bytes to this
    /// diagnostic (issue #699 builder).
    ///
    /// Used exclusively by marque-capco's `build_decoder_diagnostic`
    /// helper today; surfaced as a `pub` builder so a future rule
    /// that needs to publish a recognized canonical form to the
    /// user-facing renderers can opt in without re-touching the
    /// struct's field initialization.
    ///
    /// # Constitution Principle II
    ///
    /// The argument is a [`secrecy::SecretSlice<u8>`] so the bytes
    /// wipe on drop. Every `expose_secret()` readout site (CLI human
    /// renderer, NDJSON projection, WASM mirror) is a grep target
    /// for security review.
    ///
    /// # Constitution V Principle V (audit-content-ignorance)
    ///
    /// Recognized-canonical bytes are **lint-side only**. The audit
    /// envelope continues to carry the BLAKE3 digest of the canonical
    /// form plus the structural `Recanonicalize` intent, never the
    /// bytes themselves — `AppliedFix<S>` has no analogous field.
    /// The asymmetry is pinned by
    /// `lint_carries_recognized_canonical_fix_audit_does_not` in
    /// `crates/engine/tests/recognized_canonical_lint_vs_fix.rs`.
    ///
    /// # Replacement semantics
    ///
    /// This is a setter, not a merge. Passing `None` **clears** any
    /// existing `Some` value the diagnostic held previously, and
    /// passing `Some(_)` replaces any existing value. Callers
    /// constructing a `Diagnostic` should typically pass `Some(_)`
    /// once at construction time; the `None` form exists for the
    /// "explicitly clear after a builder chain" case rather than as
    /// a default.
    #[must_use]
    pub fn with_recognized_canonical(
        mut self,
        canonical: Option<secrecy::SecretSlice<u8>>,
    ) -> Self {
        self.recognized_canonical = canonical;
        self
    }

    /// Construct an informational diagnostic that carries no fix
    /// payload.
    ///
    /// For rules that emit diagnostics at `Info`, `Warn`, `Error`,
    /// or `Suggest` without proposing a repair. Equivalent to
    /// [`Self::with_fix`] with `fix: None` but reads more cleanly
    /// at call sites.
    pub fn info(
        rule: RuleId,
        severity: Severity,
        span: Span,
        message: Message,
        citation: Citation,
    ) -> Self {
        Self::with_fix(rule, severity, span, message, citation, None)
    }
}
