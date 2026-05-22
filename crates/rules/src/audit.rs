// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `marque-2.0` audit-record types (was `marque-1.0` pre-T044).
//!
//! This module ships the audit-record-side types reshaped for the
//! `marque-1.0` audit schema cutover landing at PR 3c.2.D, then
//! re-pinned to `marque-2.0` at T044 to carry the `RuleId`
//! `(scheme, predicate_id)` 2-tuple form on the audit-record wire:
//!
//! - [`Discriminant`] — closed `Strict | Decoder` provenance discriminator.
//!   Derived at audit-emit time from [`crate::FixSource`] per PM-D-7
//!   (`docs/plans/2026-05-20-pr3c2-d-pm-decisions.md`).
//! - [`AppliedReplacement`] — `{ canonical, confidence, bytes_digest }`
//!   payload inside an [`AppliedFixDetail`]. Replaces the v1
//!   `AppliedFixProposal::FixIntent` arm for the marking-side audit-
//!   record path.
//! - [`AppliedFixDetail`] — `{ replacement, original_span,
//!   original_digest }`. The marking-side fix-detail substructure on
//!   the `marque-2.0` envelope (was `marque-1.0` pre-T044).
//! - [`AppliedTextCorrection`] — separate audit-record type for the
//!   C001 / `[corrections]`-map path. Disjoint from [`crate::AppliedFix`]
//!   by construction so the G13 boundary (Constitution V Principle V)
//!   is checkable at compile time.
//! - [`AuditLine`] — sum type preserving cross-record promotion order
//!   between marking-fix and text-correction NDJSON lines.
//!
//! # Status (PR 3c.2.D / D2 commit boundary)
//!
//! All types land **alongside** the existing v1 types in this crate.
//! v1 [`crate::AppliedFix`] and [`crate::AppliedFixProposal`] are
//! unchanged at D2; the consumer migration (engine emit, CLI / WASM
//! renderers, test fixtures) lands in D3–D6, and the atomic schema
//! flip with v1 deletion lands in D7. This module is **purely
//! additive** at D2 — no consumer is wired through it yet.
//!
//! # Constitution VII (crate discipline)
//!
//! `blake3` dep added to `marque-rules` per PM-D-6 (NOT to
//! `marque-scheme` — leaf-crate minimal-dep posture preserved). The
//! digest is computed at promotion time inside the engine's
//! `__engine_promote` body and threaded through these types as a
//! `Blake3Hash` value (`Copy`-sized; renderer materializes the
//! `"blake3:<hex>"` audit-emit string only at the NDJSON projection
//! boundary).

use std::sync::Arc;
use std::time::SystemTime;

use marque_scheme::{Canonical, MarkingScheme, Span};
use smol_str::SmolStr;

use crate::confidence::Confidence;
use crate::fix_intent::FixIntent;
use crate::message::{Blake3Hash, Message};
use crate::{EnginePromotionToken, FixSource, RuleId};
use marque_scheme::Severity;

// ---------------------------------------------------------------------------
// Discriminant
// ---------------------------------------------------------------------------

/// Replacement provenance discriminator.
///
/// Distinguishes strict-recognizer-derived fixes from decoder-fallback
/// fixes per `specs/006-engine-rule-refactor/contracts/audit-record.md`
/// `marque-2.0` shape (was `marque-1.0` pre-T044).
///
/// The `Strict` arm covers every [`crate::FixSource`] value that comes
/// from a deterministic-parse path
/// ([`crate::FixSource::BuiltinRule`], [`crate::FixSource::MigrationTable`]);
/// the `Decoder` arm covers probabilistic recognition
/// ([`crate::FixSource::DecoderPosterior`],
/// [`crate::FixSource::DecoderClassificationHeuristic`]).
/// [`crate::FixSource::CorrectionsMap`] does NOT map to a `Discriminant`
/// — it routes to an [`AppliedTextCorrection`] line instead, per PM-D-4.
///
/// # Why closed (no `#[non_exhaustive]`)
///
/// Adding a variant requires a coordinated audit-schema bump. The
/// `MARQUE_AUDIT_SCHEMA` accept-list at `crates/engine/build.rs` MUST
/// bump in lockstep with any new variant. Matches the closed-set
/// discipline on [`crate::MessageTemplate`].
///
/// # Wire form
///
/// [`Self::as_str`] returns the JSON wire value. Pinned by
/// `crates/rules/tests/discriminant_audit_string.rs`.
///
/// **No `From<FixSource> for Discriminant` impl.** The 5-to-2 collapse
/// is not total ([`crate::FixSource::CorrectionsMap`] routes to
/// [`AppliedTextCorrection`], not to a discriminant). Conversion lives
/// at the engine's audit-emit projection (PM-D-7).
///
/// ```compile_fail
/// # use marque_rules::audit::Discriminant;
/// // Closed enum — no third variant.
/// let _ = Discriminant::TextCorrection;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Discriminant {
    /// Strict recognizer produced this fix from a deterministic
    /// canonical lookup. `confidence.recognition = 1.0`.
    Strict,
    /// Decoder produced this fix from a probabilistic posterior.
    /// `confidence.recognition < 1.0`.
    Decoder,
}

impl Discriminant {
    /// Audit-emit wire string.
    ///
    /// Matches `contracts/audit-record.md` `marque-2.0`
    /// `replacement.discriminant` JSON field (was `marque-1.0`
    /// pre-T044). Pinned by
    /// `crates/rules/tests/discriminant_audit_string.rs` so a silent
    /// rename of either arm becomes a compile-time test failure.
    #[inline]
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Decoder => "decoder",
        }
    }
}

/// Map [`FixSource`] to a [`Discriminant`] for audit-record emission.
///
/// Per PM-D-7 (`docs/plans/2026-05-20-pr3c2-d-pm-decisions.md`) the
/// discriminant is derived at audit-emit time from the originating
/// fix's source, not threaded through promote-call arguments. The
/// 5-to-2 collapse table:
///
/// | `FixSource` variant            | `Discriminant` |
/// |---|---|
/// | `BuiltinRule`                  | `Strict`       |
/// | `MigrationTable`               | `Strict`       |
/// | `DecoderPosterior`             | `Decoder`      |
/// | `DecoderClassificationHeuristic` | `Decoder`    |
/// | `CorrectionsMap`               | **(panic)**    |
///
/// # Why panic on `CorrectionsMap`
///
/// Per PM-D-4, `FixSource::CorrectionsMap` is structurally outside
/// the marking-recognition discriminator — it routes to an
/// [`AppliedTextCorrection`] line, which is a separate NDJSON record
/// type and never carries a [`Discriminant`]. If a `CorrectionsMap`
/// source reaches this function the engine has bugged the routing
/// between `AppliedFix` and `AppliedTextCorrection`; we panic loudly
/// rather than silently return a default (which would hide the bug
/// and emit a wrong-shape audit record).
///
/// # Stability
///
/// Closed mapping. Adding a [`FixSource`] variant requires deciding
/// which arm it routes to and updating this match — the compiler
/// will refuse the build until that decision is made (FR-040 lint
/// + non-exhaustive-match warning).
///
/// Pinned by `crates/rules/tests/discriminant_from_source.rs`.
#[inline]
#[must_use]
pub const fn discriminant_from_source(source: FixSource) -> Discriminant {
    match source {
        FixSource::BuiltinRule | FixSource::MigrationTable => Discriminant::Strict,
        FixSource::DecoderPosterior | FixSource::DecoderClassificationHeuristic => {
            Discriminant::Decoder
        }
        FixSource::CorrectionsMap => panic!(
            "FixSource::CorrectionsMap routes to AppliedTextCorrection (PM-D-4 / D-D-3); \
             never to an AppliedFix Discriminant. \
             A CorrectionsMap source reaching discriminant_from_source means the engine \
             promoted a corrections-map fix through the marking-side AppliedFix path \
             instead of the AppliedTextCorrection path — bug in the engine's promote \
             dispatch."
        ),
    }
}

// ---------------------------------------------------------------------------
// AppliedReplacement<S>
// ---------------------------------------------------------------------------

/// Provenance + canonical-replacement payload inside an
/// [`AppliedFixDetail`].
///
/// Carries the engine-rendered [`Canonical<S>`] value (sealed
/// construction per `marque_scheme::canonical`), the originating
/// [`Confidence`] snapshot, and the BLAKE3 digest of the canonical
/// bytes. Replaces the v1 `AppliedFixProposal::FixIntent` arm for the
/// marking-side audit-record path.
///
/// # Field set per PM-D-7
///
/// Per PM-D-7 (`docs/plans/2026-05-20-pr3c2-d-pm-decisions.md`), this
/// type does NOT carry a `discriminant: Discriminant` field. The
/// discriminant is derived at audit-emit time from
/// [`AppliedFix::source`] via the 5-to-2 mapping; storing it here
/// would duplicate data already on the outer [`AppliedFix`].
///
/// # `bytes_digest` precomputation (PM-D-6)
///
/// The digest is precomputed at promotion time inside the engine's
/// `__engine_promote` body, NOT lazily at audit-emit time, so the
/// emit-loop cost stays constant and the digest cannot accidentally
/// detach from the canonical bytes that produced it. The
/// computation site is engine-internal and constructed in the same
/// body as the [`Canonical<S>`] itself — they cannot desync.
///
/// # Why no `#[non_exhaustive]`
///
/// Pure data struct; field set is the v2 shape per
/// `contracts/audit-record.md`. Adding a field is an audit-schema
/// bump (closed-set discipline; matches [`crate::MessageArgs`]).
/// External brace construction is blocked by [`Canonical<S>`] being
/// sealed via [`marque_scheme::EngineConstructor`] — `AppliedReplacement`
/// cannot be brace-constructed by an external crate even without
/// `#[non_exhaustive]`, because no public path exists to construct
/// the `canonical` field outside the engine.
///
/// # Why manual `Clone`
///
/// The derive macro over-constrains to `S: Clone`, breaking
/// `S = CapcoScheme` (intentionally non-`Clone`). The manual impl
/// only requires `S: MarkingScheme` because the actual cloned
/// payload ([`Canonical<S>`], [`Confidence`], [`Blake3Hash`]) all
/// support `Clone` without an `S: Clone` bound.
#[derive(Debug)]
pub struct AppliedReplacement<S: MarkingScheme> {
    /// The engine-rendered canonical replacement.
    pub canonical: Canonical<S>,
    /// Confidence snapshot at promotion time (cloned from the
    /// originating [`crate::FixIntent`]`.confidence`).
    pub confidence: Confidence,
    /// BLAKE3 digest of the rendered canonical bytes. Precomputed at
    /// promotion time per PM-D-6 to keep the audit-emit path
    /// allocation-free.
    pub bytes_digest: Blake3Hash,
}

impl<S: MarkingScheme> Clone for AppliedReplacement<S> {
    fn clone(&self) -> Self {
        Self {
            canonical: self.canonical.clone(),
            confidence: self.confidence.clone(),
            bytes_digest: self.bytes_digest,
        }
    }
}

// ---------------------------------------------------------------------------
// AppliedFixDetail<S>
// ---------------------------------------------------------------------------

/// The "marking" arm of an [`AppliedFix`] — replaces the
/// `AppliedFixProposal::FixIntent` variant of the pre-v2 envelope.
///
/// # Shape per contract
///
/// The `marque-2.0` audit-record contract at
/// `contracts/audit-record.md` (was `marque-1.0` pre-T044) shapes the JSON as
/// `{ "fix": { "replacement": {...}, "original_span": ...,
/// "original_digest": ... } }` — `fix` is a nested object, not a flat
/// field set. Matching the JSON shape at the type level (rather than
/// via custom `Serialize`) keeps the relationship debuggable and the
/// JSON projection trivial.
///
/// # `original_digest` (Constitution V Principle V / G13)
///
/// The pre-fix bytes themselves are NEVER stored — only the BLAKE3
/// digest. This is the audit anchor for "which bytes were rewritten"
/// without storing the bytes themselves. PM-D-6 places the digest
/// computation at promotion time inside the engine's
/// `__engine_promote` body so the digest and the bytes that produced
/// it cannot desync.
#[derive(Debug)]
pub struct AppliedFixDetail<S: MarkingScheme> {
    /// The canonical replacement payload + provenance.
    pub replacement: AppliedReplacement<S>,
    /// Byte span the fix targeted in the source buffer.
    pub original_span: Span,
    /// BLAKE3 digest of the pre-fix bytes at `original_span`.
    pub original_digest: Blake3Hash,
}

impl<S: MarkingScheme> Clone for AppliedFixDetail<S> {
    fn clone(&self) -> Self {
        Self {
            replacement: self.replacement.clone(),
            original_span: self.original_span,
            original_digest: self.original_digest,
        }
    }
}

// ---------------------------------------------------------------------------
// AppliedFix<S> — v2 outer type (marque-2.0 audit-record shape;
//                                  was marque-1.0 pre-T044)
// ---------------------------------------------------------------------------

/// `marque-2.0` audit-record marking-side type (v2 shape; was
/// `marque-1.0` pre-T044).
///
/// The marking-side complement of [`AppliedTextCorrection`]: a
/// promoted [`FixIntent<S>`] with the engine-rendered
/// [`Canonical<S>`] payload, the BLAKE3 digests of both the pre-fix
/// and post-fix bytes, and the runtime context the engine snapshots
/// at promotion time. Serializes to the
/// `{ "type": "applied_fix", ... }` NDJSON line per
/// `specs/006-engine-rule-refactor/contracts/audit-record.md` body §.
///
/// # Transient module location
///
/// This is the v2 reshape of [`crate::AppliedFix`]. During the PR
/// 3c.2.D window (D2–D7) both v1 (at the crate root) and v2 (here)
/// coexist; D7 atomically deletes v1 and promotes this type to the
/// crate root per `docs/plans/2026-05-20-pr3c2-d-pm-decisions.md`
/// PM-D-9. Consumers within `marque-engine` work directly with this
/// module path during the window; consumers outside the engine
/// (CLI render, WASM render, test fixtures) migrate in D-A3 / D-A4.
///
/// # Field set per PM-D-11
///
/// **Added vs v1**:
/// - `severity: Severity` — top-level snapshot from the originating
///   [`crate::Diagnostic`]. Contract emits `"severity": "..."` at top
///   level.
/// - `message: Message` — top-level snapshot. Contract emits
///   `{"message": {"template": "...", "args": {...}}}` at top level.
///
/// **Removed vs v1**:
/// - `proposal: AppliedFixProposal<S>` — replaced by `fix:
///   AppliedFixDetail<S>` (text corrections move to
///   [`AppliedTextCorrection`] per PM-D-4).
/// - `confidence: Confidence` — moved into
///   `fix.replacement.confidence`. Avoids the duplication the JSON
///   contract would otherwise force (the field already lives inside
///   the `fix` sub-object).
/// - `migration_ref: Option<&'static str>` — removed. The
///   `marque-2.0` contract does not emit a top-level `migration_ref`;
///   PR 3c.2.C's typed `Citation` on [`crate::Diagnostic`] supersedes
///   it as the citation-provenance channel.
///
/// **Retained**: `rule`, `span`, `source`, `timestamp`,
/// `classifier_id`, `dry_run`, `input`.
///
/// # `#[non_exhaustive]`
///
/// Reserves grow-path for future hash-axis additions (e.g., a
/// canonical-pre-image digest). Combined with the engine-only
/// [`Self::__engine_promote`] constructor, external code cannot
/// brace-construct an `AppliedFix` regardless.
///
/// # Why manual `Clone`
///
/// The derive macro over-constrains to `S: Clone`, which breaks
/// `S = CapcoScheme` (intentionally non-`Clone`). The manual impl
/// only requires `S: MarkingScheme`.
///
/// # Construction
///
/// Engine-only via [`Self::__engine_promote`], sealed by
/// [`EnginePromotionToken`] (FR-040 lint-enforced).
///
/// # Compile-fail invariants (PR 3c.2.D)
///
/// **No `Default for AppliedFix<S>` impl** — would defeat the
/// engine-only seal in `__engine_promote`.
///
/// ```compile_fail
/// # struct StubScheme;
/// # impl marque_scheme::MarkingScheme for StubScheme {
/// #     type Token = ();
/// #     type Marking = ();
/// #     type ParseError = std::convert::Infallible;
/// #     type OpenVocabRef = ();
/// #     fn name(&self) -> &str { "stub" }
/// #     fn schema_version(&self) -> &str { "v0" }
/// #     fn categories(&self) -> &[marque_scheme::Category] { &[] }
/// #     fn constraints(&self) -> &[marque_scheme::Constraint] { &[] }
/// #     fn templates(&self) -> &[marque_scheme::Template] { &[] }
/// #     fn parse(&self, _: &str) -> Result<marque_scheme::Parsed<Self::Marking>, Self::ParseError> { Ok(marque_scheme::Parsed::Unambiguous(())) }
/// #     fn render_canonical(&self, _: &Self::Marking, _: &marque_scheme::RenderContext, _: &mut dyn std::fmt::Write) -> std::fmt::Result { Ok(()) }
/// #     fn canonicalize<'src>(&self, _: marque_ism::ParsedAttrs<'src>) -> marque_ism::CanonicalAttrs { unimplemented!() }
/// #     fn apply_intent(&self, _: &mut Self::Marking, _: &marque_scheme::ReplacementIntent<Self>) -> Result<(), marque_scheme::ApplyIntentError> { Ok(()) }
/// # }
/// let _: marque_rules::audit::AppliedFix<StubScheme> = Default::default();
/// ```
///
/// **External crates cannot brace-construct** (the
/// `#[non_exhaustive]` attribute + every field's
/// engine-promotion-only construction path both block external
/// brace patterns).
///
/// ```compile_fail
/// # use marque_rules::audit::AppliedFix;
/// # use marque_rules::RuleId;
/// # use marque_scheme::{Severity, Span};
/// let _: AppliedFix<()> = AppliedFix {
///     rule: RuleId::new("E001"),
///     severity: Severity::Error,
///     span: Span::new(0, 0),
///     // ... other fields omitted; #[non_exhaustive] rejects this
///     // brace pattern at the doctest crate boundary regardless of
///     // field-list completeness.
/// };
/// ```
///
/// **`__engine_promote_text_correction` relocates to
/// [`AppliedTextCorrection`] at v2** (PM-D-4 marking-vs-text-correction
/// split). The old method-resolution path
/// `AppliedFix::__engine_promote_text_correction(...)` is gone — the
/// marking-side [`AppliedFix`] carries no text-correction
/// constructor. A future regression that re-adds the method on
/// `AppliedFix` (collapsing the type-level G13 boundary) is caught
/// at `cargo test --doc` time, an earlier gate than the FR-040
/// promote-callsite lint.
///
/// Rust preflight §7.7. Defense-in-depth: the FR-040 lint's
/// exact-equality matcher (PR 3c.2.D fixup F-1) catches calls to the
/// `AppliedTextCorrection` constructor regardless of receiver; this
/// doctest catches the type-system relocation directly.
///
/// ```compile_fail
/// # use marque_rules::audit::AppliedFix;
/// // No `__engine_promote_text_correction` exists on `AppliedFix<()>`
/// // — the constructor lives on `AppliedTextCorrection`.
/// let _ = AppliedFix::<()>::__engine_promote_text_correction();
/// ```
#[non_exhaustive]
#[derive(Debug)]
pub struct AppliedFix<S: MarkingScheme> {
    /// Rule ID. Snapshot at the top level so audit emitters don't
    /// have to descend into `fix.replacement` for the audit-cardinality
    /// field.
    pub rule: RuleId,
    /// Severity at promotion time (snapshot from
    /// [`crate::Diagnostic::severity`]; survives the lint-post-pass
    /// severity rewrite at FR-008 / D-7.6).
    pub severity: Severity,
    /// Byte span in the original source buffer.
    pub span: Span,
    /// The marking-side fix detail (replacement + digest +
    /// original_span). v2 replacement for the v1
    /// `proposal: AppliedFixProposal<S>` envelope's `FixIntent` arm.
    pub fix: AppliedFixDetail<S>,
    /// Provenance of the originating rule emission. The renderer
    /// projects this to the wire-format [`Discriminant`] via
    /// [`discriminant_from_source`] at audit-emit time (PM-D-7).
    pub source: FixSource,
    /// Diagnostic message — closed-template, closed-args. Snapshot
    /// from [`crate::Diagnostic::message`]. Audit emitters render
    /// via [`Message::template`] + [`Message::args`].
    pub message: Message,
    /// Timestamp of application (clock-injected).
    pub timestamp: SystemTime,
    /// Classifier identity from runtime config. `None` if
    /// not configured.
    pub classifier_id: Option<Arc<str>>,
    /// `true` if produced under `--dry-run` (FR-006).
    pub dry_run: bool,
    /// Caller-supplied input identifier (file path, `-` for stdin,
    /// `None` if N/A).
    pub input: Option<Arc<str>>,
}

// Manual Clone — see the v1 [`crate::AppliedFix`] impl for the
// rationale (S: MarkingScheme, not S: Clone). The actual cloned
// payload (`AppliedFixDetail`, `Message`, `Arc<str>`) all support
// `Clone` without an `S: Clone` bound.
impl<S: MarkingScheme> Clone for AppliedFix<S> {
    fn clone(&self) -> Self {
        Self {
            rule: self.rule.clone(),
            severity: self.severity,
            span: self.span,
            fix: self.fix.clone(),
            source: self.source,
            message: self.message.clone(),
            timestamp: self.timestamp,
            classifier_id: self.classifier_id.clone(),
            dry_run: self.dry_run,
            input: self.input.clone(),
        }
    }
}

impl<S: MarkingScheme> AppliedFix<S> {
    /// Engine-only promotion path for the v2 audit record.
    ///
    /// # Reserved name (FR-040 lint contract)
    ///
    /// `__engine_promote` is reserved by the marque project — the
    /// `tools/promote-callsite-lint/` CI lint matches by last
    /// segment, regardless of receiver type or qualifier form. The
    /// lint covers both this v2 method on
    /// [`crate::audit::AppliedFix`] AND the v1 method on
    /// [`crate::AppliedFix`] (crate-root) with the same matcher
    /// pattern. See [`crate::AppliedFix::__engine_promote`] for the
    /// full FR-040 contract definition; the same engine-only
    /// production carve-out and Constitution V Principle V
    /// test-fixture carve-out apply verbatim.
    ///
    /// # Parameters
    ///
    /// The engine wires the v2 audit record by passing in:
    ///
    /// - `rule` / `severity` / `span`: snapshotted from the
    ///   originating [`crate::Diagnostic`].
    /// - `intent`: the rule-emitted [`FixIntent<S>`] whose `source` +
    ///   `confidence` + `message` snapshot onto the audit record.
    /// - `original_bytes`: the pre-fix byte slice (the engine reads
    ///   `source[span]`). Hashed inline to produce the
    ///   `original_digest` per Constitution V Principle V — the
    ///   bytes themselves are never stored.
    /// - `canonical`: the engine-rendered [`Canonical<S>`] payload
    ///   produced via [`marque_scheme::canonical::EngineConstructor::build_open_vocab`]
    ///   (open-vocab path) or [`Canonical::from_cve`] (closed-CVE
    ///   path). The constructor hashes `canonical.bytes()` inline to
    ///   produce the `bytes_digest`.
    /// - `timestamp` / `classifier_id` / `dry_run` / `input`:
    ///   clock-injected runtime context.
    ///
    /// # PM-D-6 digest computation
    ///
    /// Both BLAKE3 digests are computed inside this function body
    /// from the parameters passed in by the engine. Engine never
    /// materializes them itself — the constructor is the single
    /// source of truth so the digest and the bytes that produced it
    /// cannot desync.
    ///
    /// # PM-D-7 discriminant derivation
    ///
    /// `Discriminant` is NOT a constructor parameter. The renderer
    /// derives it from `self.source` via
    /// [`discriminant_from_source`] at audit-emit time (closed
    /// 5-to-2 mapping).
    //
    // `clippy::too_many_arguments` allowed because every parameter
    // carries engine-only runtime context the seal must capture
    // atomically. Refactoring into a struct argument would shift the
    // API surface without reducing the count visible at the engine
    // call site.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __engine_promote(
        rule: RuleId,
        severity: Severity,
        span: Span,
        intent: FixIntent<S>,
        original_bytes: &[u8],
        canonical: Canonical<S>,
        timestamp: SystemTime,
        classifier_id: Option<Arc<str>>,
        dry_run: bool,
        input: Option<Arc<str>>,
        _token: EnginePromotionToken,
    ) -> Self {
        // Constitution V Principle V (G13): hash inline, never store
        // the bytes. The `&[u8]` view borrows for the duration of
        // this function body only; the digest survives in
        // `AppliedFixDetail.original_digest`.
        let original_digest = blake3::hash(original_bytes);
        let bytes_digest = blake3::hash(canonical.bytes().as_bytes());

        let replacement = AppliedReplacement {
            canonical,
            confidence: intent.confidence,
            bytes_digest,
        };
        let fix = AppliedFixDetail {
            replacement,
            original_span: span,
            original_digest,
        };
        Self {
            rule,
            severity,
            span,
            fix,
            source: intent.source,
            message: intent.message,
            timestamp,
            classifier_id,
            dry_run,
            input,
        }
    }

    /// Wire-format [`Discriminant`] for this record.
    ///
    /// Derived from [`Self::source`] per
    /// [`discriminant_from_source`] (PM-D-7). Audit emitters call
    /// this at NDJSON projection time; the value is not stored on
    /// the record.
    #[inline]
    #[must_use]
    pub fn discriminant(&self) -> Discriminant {
        discriminant_from_source(self.source)
    }
}

// ---------------------------------------------------------------------------
// AppliedTextCorrection
// ---------------------------------------------------------------------------

/// Engine-internal text-correction audit record (C001 /
/// `[corrections]` map, and the closely-shaped E006 deprecation-
/// migration path).
///
/// Distinct from [`AppliedFix`] (marking-side) — text corrections run
/// pre-scanner and carry corpus-derived canonical replacement strings
/// rather than [`Canonical<S>`] payloads. Per PM-D-4
/// (`docs/plans/2026-05-20-pr3c2-d-pm-decisions.md`) the type split
/// makes the G13 boundary (Constitution V Principle V) checkable at
/// compile time: a marking-side record carries token canonicals +
/// category IDs + BLAKE3 digests + confidence scalars; a text-
/// correction record carries a corpus-derived `SmolStr` replacement.
///
/// # Not generic over the scheme
///
/// `AppliedTextCorrection` is NOT generic over `S`. The text-
/// correction path operates on raw bytes pre-scanner; no scheme-
/// typed payload is involved.
///
/// # Constitution V Principle V — `__engine_promote_text_correction`
///
/// Construction is sealed by [`EnginePromotionToken`] (the same seal
/// used by [`AppliedFix::__engine_promote`]). Production code MUST
/// call [`Self::__engine_promote_text_correction`] only from inside
/// `Engine::apply_text_corrections`. The full three-constraint test-
/// fixture carve-out from [`AppliedFix::__engine_promote`]'s doc
/// applies here verbatim.
///
/// The function name `__engine_promote_text_correction` is reserved
/// by the FR-040 promote-callsite lint — last-segment match by name,
/// regardless of receiver type. The lint catches calls to this method
/// AND calls to [`AppliedFix::__engine_promote_text_correction`] (v1
/// path) with the same matcher pattern.
///
/// # Compile-fail invariant (PM-D-4)
///
/// **`AppliedTextCorrection` is not coercible to `AppliedFix<S>`.**
/// The two audit-record types are disjoint by construction; no
/// `From` / `Into` impl exists between them. Type-level enforcement
/// of the G13 marking-vs-text-correction split.
///
/// ```compile_fail
/// # use marque_rules::{AppliedFix, AppliedTextCorrection};
/// fn _convert<S: marque_scheme::MarkingScheme>(t: AppliedTextCorrection) -> AppliedFix<S> {
///     t.into()
/// }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct AppliedTextCorrection {
    /// Rule ID (typically `C001` for `[corrections]`-map matches;
    /// rule-emitted text corrections carry their own ID).
    pub rule: RuleId,
    /// Severity at promotion time.
    pub severity: Severity,
    /// Byte span the correction targeted in the source buffer.
    pub span: Span,
    /// BLAKE3 digest of the pre-correction bytes at `span`.
    pub original_digest: Blake3Hash,
    /// Canonical replacement bytes — corpus-derived token canonical
    /// (Constitution V Principle V's permitted-identifier list).
    pub replacement: SmolStr,
    /// Provenance.
    pub source: FixSource,
    /// Confidence snapshot.
    pub confidence: Confidence,
    /// Migration reference (§-citation, for E006 deprecation path);
    /// `None` for C001 corrections-map matches.
    pub migration_ref: Option<&'static str>,
    /// Diagnostic message — closed template, closed args. Text-
    /// correction records emit [`crate::MessageTemplate::CorrectionsApplied`]
    /// (C001) or [`crate::MessageTemplate::SupersededToken`] (E006).
    pub message: Message,
    /// Timestamp of application.
    pub timestamp: SystemTime,
    /// Classifier identity.
    pub classifier_id: Option<Arc<str>>,
    /// Dry-run flag.
    pub dry_run: bool,
    /// Caller-supplied input identifier.
    pub input: Option<Arc<str>>,
}

impl AppliedTextCorrection {
    /// Engine-only promotion path for text corrections.
    ///
    /// # Reserved name (FR-040 lint contract)
    ///
    /// The function name `__engine_promote_text_correction` is
    /// reserved by the marque project — last-segment matching applies
    /// regardless of the receiver type. See
    /// [`AppliedFix::__engine_promote`] for the full FR-040 contract
    /// definition.
    ///
    /// # Engine-only contract (production code) — same as
    /// [`AppliedFix::__engine_promote`]
    ///
    /// Production callers MUST call this only from inside
    /// `Engine::apply_text_corrections`. The three-constraint test-
    /// fixture carve-out (cfg(test)-scoped, never commingled with
    /// engine output, only for test-fixture *construction*) applies
    /// here verbatim — see that constructor's doc comment for the
    /// binding definition.
    ///
    /// `clippy::too_many_arguments` allowed because every parameter
    /// carries engine-only runtime context the seal must capture
    /// atomically; refactoring into a struct argument would shift
    /// the API surface without reducing the parameter count visible
    /// at the engine call site.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __engine_promote_text_correction(
        rule: RuleId,
        severity: Severity,
        span: Span,
        original_digest: Blake3Hash,
        replacement: SmolStr,
        source: FixSource,
        confidence: Confidence,
        migration_ref: Option<&'static str>,
        message: Message,
        timestamp: SystemTime,
        classifier_id: Option<Arc<str>>,
        dry_run: bool,
        input: Option<Arc<str>>,
        _token: EnginePromotionToken,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            original_digest,
            replacement,
            source,
            confidence,
            migration_ref,
            message,
            timestamp,
            classifier_id,
            dry_run,
            input,
        }
    }
}

// ---------------------------------------------------------------------------
// AuditLine<S>
// ---------------------------------------------------------------------------

/// Single line in the engine's NDJSON audit-record stream.
///
/// Two arms preserve the FR-016 promotion-order invariant across the
/// marking-fix channel and the text-correction channel: an audit
/// reader walking the stream sees marking fixes and text corrections
/// in the order the engine promoted them, without consumer-side
/// timestamp merge logic.
///
/// Per `contracts/audit-record.md` `marque-2.0` shape (was
/// `marque-1.0` pre-T044), each arm projects to its own NDJSON line type
/// (`{"type": "applied_fix", ...}` vs
/// `{"type": "text_correction", ...}`).
///
/// # `#[non_exhaustive]`
///
/// Reserves grow-path for a future engine-internal audit-line variant
/// (e.g., an [`crate::AuditNote`]-bearing arm if/when the audit-note
/// stream merges into this one). Consumer crates always include a
/// wildcard arm.
///
/// # Why manual `Clone`
///
/// Same rationale as [`AppliedReplacement::clone`] / [`AppliedFix::clone`]:
/// the derive over-constrains to `S: Clone`.
#[non_exhaustive]
#[derive(Debug)]
pub enum AuditLine<S: MarkingScheme> {
    /// Marking-side audit record.
    AppliedFix(AppliedFix<S>),
    /// Text-correction audit record (C001 / E006 paths).
    TextCorrection(AppliedTextCorrection),
}

impl<S: MarkingScheme> Clone for AuditLine<S> {
    fn clone(&self) -> Self {
        match self {
            Self::AppliedFix(f) => Self::AppliedFix(f.clone()),
            Self::TextCorrection(t) => Self::TextCorrection(t.clone()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — unit coverage for the new types (PM-D-1 / >80% coverage)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn discriminant_as_str_matches_wire() {
        // Wire-form pin parallel to crates/rules/tests/discriminant_audit_string.rs;
        // this in-module test guarantees the pin even when the
        // integration test binary is not compiled (e.g., `cargo build`
        // without `--tests`).
        assert_eq!(Discriminant::Strict.as_str(), "strict");
        assert_eq!(Discriminant::Decoder.as_str(), "decoder");
    }

    #[test]
    fn discriminant_is_copy_eq_hash() {
        let s: Discriminant = Discriminant::Strict;
        let s_copy: Discriminant = s; // Copy
        assert_eq!(s, s_copy); // PartialEq + Eq
        // Hash bound surfaces via using it in a HashSet/HashMap key.
        let mut set: std::collections::HashSet<Discriminant> = std::collections::HashSet::new();
        set.insert(Discriminant::Strict);
        set.insert(Discriminant::Decoder);
        set.insert(Discriminant::Strict); // duplicate
        assert_eq!(set.len(), 2);
    }
}
