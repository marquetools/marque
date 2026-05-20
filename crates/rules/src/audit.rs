// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `marque-1.0` audit-record v2 types.
//!
//! This module ships the audit-record-side types reshaped for the
//! `marque-1.0` audit schema cutover landing at PR 3c.2.D:
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
//!   the `marque-1.0` envelope.
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
use crate::message::{Blake3Hash, Message};
use crate::{AppliedFix, EnginePromotionToken, FixSource, RuleId};
use marque_scheme::Severity;

// ---------------------------------------------------------------------------
// Discriminant
// ---------------------------------------------------------------------------

/// Replacement provenance discriminator.
///
/// Distinguishes strict-recognizer-derived fixes from decoder-fallback
/// fixes per `specs/006-engine-rule-refactor/contracts/audit-record.md`
/// `marque-1.0` shape.
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
    /// Matches `contracts/audit-record.md` `marque-1.0`
    /// `replacement.discriminant` JSON field. Pinned by
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
/// The `marque-1.0` audit-record contract at
/// `contracts/audit-record.md` shapes the JSON as
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
/// Per `contracts/audit-record.md` `marque-1.0` shape, each arm
/// projects to its own NDJSON line type
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
