// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`RelidoImpliedByClosureRule`] — RELIDO-implied-by-closure
//! Suggest rule.
//!
//! Byte-surfacing twin of the lattice-layer `CLOSURE_RELIDO_SCI` /
//! `CLOSURE_RELIDO_US_CLASS` closure rules. Authority:
//! CAPCO-2016 §H.8 p154 + §D.2 Table 3 rule 17. The wire predicate
//! ID lives on `RuleId::new(...)` — the single source of truth.

use marque_ism::{CanonicalAttrs, MarkingType};
use marque_rules::{
    Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Phase,
    Rule, RuleContext, RuleId, Severity,
};
use marque_scheme::{
    Citation, FactRef, MarkingScheme, ReplacementIntent, SectionLetter, capco, capco_table,
};

use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// Rule: S008 — RELIDO implied by closure (byte-surfacing twin of
//              CLOSURE_RELIDO_SCI / CLOSURE_RELIDO_US_CLASS).
//
// C1 per #559 close-out (PM decision 2026-05-19). The lattice-layer
// closures `CLOSURE_RELIDO_SCI` (§H.8 p154; CAT_SCI implies RELIDO
// absent FD&R suppressors) and `CLOSURE_RELIDO_US_CLASS` (issue #524
// Phase 3; US collateral classified implies RELIDO absent FD&R
// suppressors) already propagate the RELIDO fact at the lattice
// layer. This rule is the byte-level twin: when the closure would
// inject RELIDO into a portion that doesn't currently carry it, S008
// fires a `Severity::Suggest` diagnostic with a `FactAdd(RELIDO,
// Scope::Portion)` intent so the user-visible text can match the
// lattice-level state.
//
// Per S007's precedent (the byte-surfacing twin of
// `CLOSURE_REL_TO_USA_NATO` at §H.7 p127), the text-layer rule
// proposes the byte-level insertion; the lattice-layer closure stays
// out of the diagnostic surface.
//
// Authority: CAPCO-2016 §B.3 Table 2 p21 (trigger authority — the
// "Classified + uncaveated + on/after 28 June 2010 → Mark as RELIDO"
// row drives S008's "would the projection inject RELIDO?" check);
// §B.3 paragraph b p19 (FD&R-absent gate); §D.2 Table 3 rule 17
// (FD&R precedence for banner roll-up); §H.8 p154 (RELIDO marking
// template — defines what RELIDO means once present). Verified
// against `crates/capco/docs/CAPCO-2016.md` at authorship.
// ---------------------------------------------------------------------------

/// Confidence scalar emitted by S008 (`relido-implied-by-closure`)
/// alongside its `FactAdd` fix intent.
///
/// **Calibration.** Mirrors `BARE_NATO_REQUIRES_REL_TO_CONFIDENCE = 0.85` —
/// example/closure-derived guidance that ships at `Severity::Suggest`
/// with confidence high enough to clear a relaxed
/// `confidence_threshold` when paired with `[rules] S008 = "fix"`.
/// The §B.3 Table 2 p21 default-if-absent obligation + §H.8 p154
/// RELIDO template + §D.2 Table 3 rule 17 backing the post-#704
/// `default_fill::row{8,9}_should_fill` predicates is
/// defaulting-rule prose plus FD&R-defaults derivation, not
/// "MUST"-mandate prose; the suggest channel is the right home.
const SUGGEST_CONFIDENCE: f32 = 0.85;

/// Shared `CapcoScheme` used by S008's `check()` to apply the closure
/// fixpoint to a portion's marking. Constructed lazily — `CapcoScheme::new()`
/// runs the constraint/page-rewrite/closure-rule catalog build once,
/// then every `check()` call borrows the cached instance instead of
/// reconstructing it per-portion. Mirrors the `SCHEME` lazy-construction
/// pattern the former `rules_declarative` module used; keeping an
/// independent instance here meant S008 was never coupled to that
/// now-deleted file.
static SCHEME: std::sync::LazyLock<CapcoScheme> = std::sync::LazyLock::new(CapcoScheme::new);

/// Rule **S008** — `relido-implied-by-closure`.
///
/// Fires on a portion whose closure-applied projection carries RELIDO
/// in `dissem_us` AND whose source text does NOT already carry RELIDO.
/// Emits a `Severity::Suggest` diagnostic with a
/// `FactAdd(TOK_RELIDO, Scope::Portion)` intent at confidence
/// [`SUGGEST_CONFIDENCE`].
///
/// # Project-based trigger detection
///
/// The rule runs `SCHEME.project(Scope::Page, &[marking])` over
/// a single-portion page and compares the post-pipeline dissem axis
/// against the input. This routes through the full pipeline (per-axis
/// join + close() + default_fill + supersession overlay + page
/// rewrites), which is the post-#704 canonical observable state.
/// Calling `closure()` directly would observe only the per-marking
/// unconditional implications (Rows 1-6 of `CLOSURE_TABLE`) — the
/// RELIDO defaults retired from close() to
/// `default_fill::row{8,9}_should_fill` because they are
/// "default if absent" rules per §B.3 paragraph b p19's "NOT
/// MARKED PREVIOUSLY" gate (non-monotone by §-design and unable
/// to live in a closure operator that honors the
/// `MarkingScheme::closure` monotone contract). Using `project()`
/// keeps S008 aligned with the engine's final-state semantic by
/// construction.
///
/// The post-#704 pipeline interacts with two default-fill
/// predicates in canonical order:
///
/// - `default_fill::row8_should_fill`
///   (`capco:closure.dissem.relido-if-sci-and-not-incompatible`)
///   gates on `(post_close ∩ SCI_PRESENT != 0) ∧ (post_close ∩
///   MASK_FDR_OR_RELIDO_INCOMPAT == 0)`; when both hold,
///   `apply_default_fill` adds RELIDO. The supersession overlay
///   then strips it if NOFORN is observed.
/// - `default_fill::row9_should_fill`
///   (`capco:closure.dissem.relido-if-us-collateral-class`) gates
///   on `(post_close ∩ US_COLLATERAL_CLASSIFIED != 0) ∧
///   (post_close ∩ MASK_RELIDO_US_CLASS_SUPPRESSORS == 0)`; same
///   overlay strip pathway when NOFORN is present.
///
/// Both gates' FD&R-absent test (`MASK_FDR_OR_RELIDO_INCOMPAT` /
/// `MASK_RELIDO_US_CLASS_SUPPRESSORS` include the NOFORN bit per
/// §B.3.a p19) means the predicates SKIP entirely on NOFORN-
/// present inputs — `apply_default_fill` never adds RELIDO there,
/// so the supersession overlay's NOFORN-dominates strip is a
/// no-op in that case. The overlay still fires correctly when
/// RELIDO is user-explicit on the input alongside NOFORN
/// (input-explicit §H.8 p145 contradiction).
///
/// # Early-return clauses (in order)
///
/// 1. **Portion-only**: `ctx.marking_type != MarkingType::Portion`.
///    Banner roll-up flows automatically once each portion carries
///    RELIDO; firing on a banner would double-report.
/// 2. **RELIDO already present**: `attrs.dissem_us` contains
///    `DissemControl::Relido`. Nothing to suggest.
/// 3. **Projection does not inject RELIDO**: the post-#704 pipeline
///    (close + default_fill + supersession overlay + page rewrites)
///    decided RELIDO is not implied on this portion — either no
///    Row-8/Row-9 trigger atom is present, OR the default-fill
///    gate's FD&R-absent test failed, OR a downstream page-rewrite
///    cleared RELIDO. No diagnostic.
///
/// # Algebraic redundancy retired (issue #713)
///
/// Prior to #713 this `check()` also carried two pre-`project()`
/// fast-paths — Clause 2b (input-explicit FD&R short-circuit) and
/// Clause 2c (non-US classification short-circuit) — that mirrored
/// `default_fill::row{8,9}_should_fill`'s gate predicates at the rule
/// layer. They were a pure optimization: algebraically Clause 3's
/// `project()` call reaches the same conclusion because the same
/// `MASK_FDR_OR_RELIDO_INCOMPAT` / `MASK_RELIDO_US_CLASS_SUPPRESSORS`
/// bits gate Row 8 and Row 9 unconditionally inside
/// `apply_default_fill`. Both fast-paths were retired in #713 to
/// eliminate the algebraic-drift risk that a future §B.3 / §H.8
/// revision (Constitution VIII migration) would require updating two
/// code paths in lock-step. The single source of truth is now the
/// default-fill stage; this rule remains the byte-surfacing Suggest
/// channel for §B.3 Table 2 p21.
///
/// `DISPLAY ONLY` silence flows through the same path post-#618:
/// `MASK_RELIDO_US_CLASS_SUPPRESSORS ⊇ MASK_FDR_DOMINATORS` includes
/// the `DISPLAY_ONLY` bit (the closure's `satisfies(TOK_DISPLAY_ONLY)`
/// predicate scans both `dissem_iter()` and `display_only_to` since
/// #618), so Row 9 skips and Clause 3 reads
/// `projection_adds_relido == false`.
///
/// # Fix shape
///
/// `ReplacementIntent::FactAdd { token: FactRef::Cve(TOK_RELIDO),
/// scope: Scope::Portion }` — the engine's renderer composes the
/// post-add marking back into bytes. This is the same intent shape
/// E021 uses for `FactAdd(NOFORN)` (synthesized by the scheme-adapter
/// bridge `crate::scheme::adapter::CapcoScheme::fix_intent_by_name` for
/// `"portion.aea.rd-frd-requires-noforn"`); the engine handles
/// canonical placement (per-axis sort) and separator insertion. No
/// manual splice logic in this rule, in contrast to S007 which crosses
/// a token boundary (RelToBlock vs Classification).
///
/// # `Phase::WholeMarking`
///
/// The `FactAdd` intent is whole-marking-scope: the engine re-renders
/// the portion from canonical attrs after applying the fact. The
/// candidate_span tells the engine which region to replace.
///
/// # G13 audit-content-ignorance
///
/// The diagnostic message is a `&'static`-derived string. No
/// `format!` interpolation of input bytes. The fact added
/// (`TOK_RELIDO`) is a `TokenId`, not a byte sequence — Constitution
/// V Principle V (G13).
pub(super) struct RelidoImpliedByClosureRule;

/// Citations S008 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
///
/// Authority order matches the doc-comment: §B.3 Table 2 p21 is
/// the trigger authority (the default-if-absent obligation that
/// drives the RELIDO injection S008 surfaces); §H.8 p154 is the
/// secondary authority (RELIDO marking template — what RELIDO
/// means once present).
const AUTHORITIES: &[Citation] = &[
    capco_table(SectionLetter::B, 3, 2, 21),
    capco(SectionLetter::H, 8, 154),
];

impl Rule<CapcoScheme> for RelidoImpliedByClosureRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.dissem.relido-implied-by-closure")
    }
    fn name(&self) -> &'static str {
        "relido-implied-by-closure"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
    }
    /// `Phase::WholeMarking`: `FactAdd` intent is whole-marking-scope.
    /// The engine re-renders the full portion from canonical attrs.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use crate::scheme::{CapcoMarking, TOK_RELIDO};
        use marque_ism::DissemControl;
        use marque_scheme::Scope;

        // Clause 1: portion-only.
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        // Clause 2: RELIDO already present — nothing to suggest. Cheap
        // check that short-circuits before the project call.
        if attrs
            .dissem_iter()
            .any(|d| matches!(d, DissemControl::Relido))
        {
            return vec![];
        }

        // Clause 3: run project(Scope::Page) and check the post-pipeline
        // state. Issue #704 retired the `CLOSURE_TABLE` suppressor_mask
        // architecture (which violated the closure operator's algebraic
        // monotonicity); the §H.8 p145 / §B.3.a p19 FD&R supersession
        // semantics moved to `CapcoScheme::apply_supersession_overlays`,
        // which runs after `closure()` returns. S008's "would closure
        // inject RELIDO?" inspection therefore needs the post-overlay
        // state — calling `scheme.closure()` directly would observe the
        // pre-overlay state (RELIDO added by Row 9 even when NOFORN is
        // in input, only to be stripped by the overlay), causing S008
        // to suggest RELIDO that the page projection would immediately
        // strip. `project(Scope::Page, &[marking])` over a single-
        // portion page exercises the full pipeline (join + closure +
        // default_fill + supersession overlay + page rewrites) and gives
        // S008 the canonical observable post-projection state.
        let marking = CapcoMarking::new(attrs.clone());
        let projected = SCHEME.project(Scope::Page, &[marking]);
        let projection_adds_relido = projected
            .0
            .dissem_us
            .iter()
            .any(|d| matches!(d, DissemControl::Relido));
        if !projection_adds_relido {
            return vec![];
        }

        // Build the FactAdd intent. Scope::Portion matches E021's
        // FactAdd(NOFORN) precedent and the closure-rule cone (cone
        // operates on per-portion dissem axis).
        let fix_intent = FixIntent {
            replacement: ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_RELIDO),
                scope: Scope::Portion,
            },
            confidence: Confidence::strict(SUGGEST_CONFIDENCE),
            feature_ids: Default::default(),
            message: Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };

        // Diagnostic span: anchor at the candidate (whole-portion)
        // since the suggestion is "add RELIDO to this portion." No
        // sub-token span is more informative than the marking-scope
        // span for an add-fact suggestion.
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            ctx.candidate_span,
            ctx.candidate_span,
            Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
            // Typed Citation anchors at §H.8 p154 (RELIDO marking
            // template — what RELIDO means once present). The
            // primary trigger authority §B.3 Table 2 p21 lives in
            // the rule's `AUTHORITIES` slice + doc comment;
            // emission stays at §H.8 p154 because per-Diagnostic
            // emission is single-Citation by API shape and §H.8
            // p154 is the marking-template anchor a reader will
            // most directly use to interpret "what is RELIDO". The
            // F.1 corpus-fidelity gate's EXPECTED_UNCOVERED list
            // carries §B.3 Table 2 p21 for S008 — the trigger
            // citation is declared in the rule's authority slice
            // but not emitted in the per-Diagnostic Citation
            // field, by intent.
            capco(SectionLetter::H, 8, 154),
            fix_intent,
        )]
    }
}
