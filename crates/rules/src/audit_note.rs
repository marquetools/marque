// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Audit-stream record for `ClosureRule` firings.
//!
//! `AuditNote` is the audit-stream counterpart to `AppliedFix` —
//! `AppliedFix` records that the engine applied a byte-level fix;
//! `AuditNote` records that the engine inferred (or suppressed, in
//! future variants) a fact via the §4.7 closure operator.
//!
//! Per Constitution V Principle V (audit content-ignorance / G13):
//! `AuditNote` carries ONLY token canonicals, category IDs, span
//! offsets, and catalog row identifiers. Document content NEVER appears
//! in an audit record.
//!
//! Per `decisions.md` D19 A, AuditNote is **engine-promoted only**.
//! The `__engine_promote` constructor reuses `EnginePromotionToken`
//! (the same seal as `AppliedFix::__engine_promote`); the FR-040
//! `tools/promote-callsite-lint` catches every `__engine_promote`-shaped
//! call site, including AuditNote's, by last-path-segment matching.
//! See `AppliedFix::__engine_promote` for the full engine-only contract.
//!
//! **Note on production status**: PR 3.7 lands the type definition and
//! the `__engine_promote` sealed constructor ONLY. No NDJSON projection
//! helper, no renderer dispatch, no production engine call-site land
//! in PR 3.7. The production engine **construct-site** lands in PR 4
//! (alongside `Engine::project::closure()` wiring); the NDJSON renderer
//! dispatch + `"type"` discriminator land in the audit-schema-bump
//! precursor PR per the plan rev 1.1 retargeting (the bump itself is
//! also non-scope for PR 7c per `decisions.md` D-7.18 — see PR #412).
//! PR 3.7's `AuditNote` is exercised via the Stage C.5 test-fixture
//! carve-out integration test
//! (`crates/engine/tests/audit_note_sealing_carve_out.rs`).

use std::sync::Arc;
use std::time::SystemTime;

use marque_ism::Span;
use marque_scheme::{MarkingScheme, Scope, TokenId, TokenRef};

use crate::{Confidence, EnginePromotionToken, RuleId};

/// Kind discriminator for `AuditNote`. v1 ships `InferredFact` only.
///
/// The two terms operate at different layers:
///
/// - `#[non_exhaustive]` is the Rust source-level contract: downstream
///   crates MUST use a wildcard `_ =>` arm when matching on this enum,
///   so an internal addition of a new variant in a later marque release
///   does not break downstream compile.
/// - The `MARQUE_AUDIT_SCHEMA` env-pinned schema is the wire-level
///   contract: the *set* of variants permitted at a given schema
///   version is closed by build-time validation. Adding a variant
///   requires a coordinated schema bump (currently `marque-mvp-3`;
///   a future precursor PR bumps to `marque-1.0` per the PR 3.7 plan
///   §1.2 rev 1.1) so that downstream NDJSON consumers can dispatch on
///   schema version without per-variant introspection.
///
/// Both contracts apply together: `#[non_exhaustive]` covers the
/// source layer, the schema bump covers the wire layer. They are
/// orthogonal, not in tension.
///
/// Per D19 A, deferred kinds (`SuppressedByFact`, `DisabledByConfig`)
/// are engineer-facing tools, not load-bearing for compliance, and
/// will land in a debug-tracing follow-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuditNoteKind {
    /// A `ClosureRule` fired and added a fact to the marking.
    /// The structural payload carries `row_name`, `cone`, `scope`,
    /// and the firing's span (if available); `suppressed_by` is `None`
    /// for `InferredFact` (the future `SuppressedByFact` kind populates it).
    InferredFact,
}

/// Structural-only payload for an `AuditNote`, satisfying the
/// Constitution V Principle V content-ignorance invariant (G13).
///
/// Permitted identifiers: token canonicals (`TokenId`), category IDs
/// (transitively via `TokenId` lookup), span offsets, catalog row
/// names (`&'static`), enumerated `Scope` value. Document content
/// (bytes, message-arg free-form text) is FORBIDDEN.
///
/// `suppressed_by` is a forward-compatibility slot per the PR 3.7 plan
/// (lattice-preflight M4): when the deferred `SuppressedByFact` kind
/// lands, it populates `suppressed_by` with the TokenIds of suppressing
/// facts. For `InferredFact`, `suppressed_by` is always `None`.
#[derive(Debug, Clone)]
pub struct AuditNoteStructural {
    /// The `ClosureRule.name` that fired (e.g., `"capco/noforn-if-no-fdr"`).
    pub row_name: &'static str,
    /// The closure rule's **declared cone slice** — a verbatim reference
    /// to the `ClosureRule.cone` of the firing rule. This is the
    /// catalog declaration, NOT necessarily the set of facts newly
    /// added by this firing: if the marking already carried some
    /// cone members before closure ran, the audit note still cites
    /// the full declared cone. A downstream auditor needing
    /// "which facts did this firing materially add" must diff the
    /// pre- and post-closure marking; the cone field is the
    /// declaration the auditor would consult to understand the
    /// rule's intent.
    ///
    /// Per Copilot PR 3.7 review pass 3: this distinction is now
    /// explicit in the field doc to prevent over-attribution by
    /// downstream tooling that reads `cone` and assumes "all of
    /// these were newly inferred."
    ///
    /// `&'static [TokenRef]` matches `ClosureRule.cone`'s shape so an
    /// audit note can represent every cone shape declared in a
    /// `closure_rules()` catalog, including category-scoped cones like
    /// `AnyInCategory(CAT_REL_TO)`. Stays G13-pure: `TokenRef` carries
    /// only `TokenId` / `CategoryId` integers, never document bytes.
    pub cone: &'static [TokenRef],
    /// The scope at which the firing applied (Portion / Page / Document).
    pub scope: Scope,
    /// Source-position anchor for the firing, when one is available.
    /// `None` when the firing is a whole-marking fact-set property with
    /// no single blameable token.
    pub span: Option<Span>,
    /// Forward-compat slot for the future `SuppressedByFact` kind
    /// (PR 3.7 plan lattice-preflight M4). Always `None` for v1's
    /// `InferredFact` kind.
    pub suppressed_by: Option<Box<[TokenId]>>,
}

/// A single audit-stream record for a closure-operator firing.
///
/// `AuditNote` is the audit counterpart to `AppliedFix`. Together they
/// form the audit stream: `AppliedFix` for byte-level fixes,
/// `AuditNote` for fact-level inferences. The two streams serve
/// different consumers (compliance reviewers + content authors) and
/// are not conflated; the NDJSON emission carries a `"type"`
/// discriminator to distinguish them (the discriminator + dispatch
/// renderer land in PR 7's `marque-1.0` audit schema cutover).
#[derive(Debug)]
pub struct AuditNote<S: MarkingScheme> {
    /// The closure rule's RuleId (e.g., scheme="capco", predicate_id="noforn-if-no-fdr").
    pub rule: RuleId,
    /// Authoritative-source citation verbatim from the closure rule's `label`.
    pub citation: &'static str,
    /// Discriminator (v1: `InferredFact` only).
    pub kind: AuditNoteKind,
    /// Timestamp of emission (clock-injected by the engine).
    pub timestamp: SystemTime,
    /// Classifier identity from runtime config. `None` if not configured.
    pub classifier_id: Option<Arc<str>>,
    /// `true` if produced under `--dry-run`.
    pub dry_run: bool,
    /// Structural payload (G13 content-ignorant).
    pub structural: AuditNoteStructural,
    /// Confidence propagated from the underlying parse/recognition step.
    /// Mirrors `AppliedFix.confidence` semantics; closure firings carry
    /// the recognition confidence of the trigger fact that caused the firing.
    pub confidence: Confidence,
    /// Scheme phantom; AuditNote is parameterized over MarkingScheme so
    /// downstream tooling can dispatch on scheme identity if needed.
    _scheme: std::marker::PhantomData<S>,
}

// Manual Clone (mirroring AppliedFix<S>'s rationale at lib.rs:429):
// the scheme S is never cloned; what matters is that scheme-internal
// types stay Clone-able. AuditNote currently carries only S-agnostic
// payload, so derive(Clone) would work — but we mirror AppliedFix's
// pattern for consistency in case future fields couple to S::OpenVocabRef.
impl<S: MarkingScheme> Clone for AuditNote<S> {
    fn clone(&self) -> Self {
        Self {
            rule: self.rule.clone(),
            citation: self.citation,
            kind: self.kind,
            timestamp: self.timestamp,
            classifier_id: self.classifier_id.clone(),
            dry_run: self.dry_run,
            structural: self.structural.clone(),
            confidence: self.confidence.clone(),
            _scheme: std::marker::PhantomData,
        }
    }
}

impl<S: MarkingScheme> AuditNote<S> {
    /// Promote a closure firing to an `AuditNote` with runtime context.
    ///
    /// # Reserved name (FR-040 lint contract)
    ///
    /// The function name `__engine_promote` is reserved by the marque
    /// project. The `tools/promote-callsite-lint/` CI lint (FR-040) flags
    /// every call site whose path's last segment is `__engine_promote`,
    /// regardless of leading qualifier. See `AppliedFix::__engine_promote`
    /// for the full lint contract — this constructor inherits the same
    /// constraints.
    ///
    /// # Engine-only contract (production code)
    ///
    /// In production code, this MUST only be called from
    /// `Engine::fix_inner` or `Engine::apply_text_corrections` (or
    /// future allow-listed engine helpers). Rule crates and CLI code
    /// must never construct `AuditNote` directly.
    ///
    /// # Type-level seal
    ///
    /// The `_token: EnginePromotionToken` parameter reuses the same
    /// type-level seal as `AppliedFix::__engine_promote`. See that
    /// function's doc for the full seal rationale; AuditNote does not
    /// re-derive a parallel seal.
    ///
    /// Test code MAY call this directly (and mint a token via
    /// `EnginePromotionToken::__engine_construct`) per the Constitution V
    /// Principle V test-fixture carve-out, when constructing synthetic
    /// AuditNote fixtures for emitter / renderer / sentinel tests.
    /// Each test call site MUST carry an inline comment naming the
    /// carve-out.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __engine_promote(
        rule: RuleId,
        citation: &'static str,
        kind: AuditNoteKind,
        timestamp: SystemTime,
        classifier_id: Option<Arc<str>>,
        dry_run: bool,
        structural: AuditNoteStructural,
        confidence: Confidence,
        _token: EnginePromotionToken,
    ) -> Self {
        // Per-kind invariant enforcement (D19 A v1):
        //   - `InferredFact` MUST carry `suppressed_by: None` (suppression
        //     is the future `SuppressedByFact` kind's territory).
        //
        // Per Copilot PR 3.7 review pass 3 ("suppressed_by invariant
        // unenforced"): make the documented invariant load-bearing
        // rather than purely documentary. `debug_assert!` rather
        // than `assert!` because the wire-format schema's `kind`
        // field also expresses the constraint (a v1 audit consumer
        // sees `kind: "InferredFact"` and knows `suppressed_by` is
        // semantically empty regardless of the field value), and a
        // hard panic in production would bring down the engine on
        // a misconfigured callsite. Test builds catch the misuse;
        // release builds tolerate it as a content-ignorant byte
        // anomaly that the schema layer absorbs.
        debug_assert!(
            !(matches!(kind, AuditNoteKind::InferredFact) && structural.suppressed_by.is_some()),
            "AuditNoteKind::InferredFact must have suppressed_by = None per D19 A v1; \
             populating suppressed_by is reserved for the future SuppressedByFact kind."
        );
        Self {
            rule,
            citation,
            kind,
            timestamp,
            classifier_id,
            dry_run,
            structural,
            confidence,
            _scheme: std::marker::PhantomData,
        }
    }
}
