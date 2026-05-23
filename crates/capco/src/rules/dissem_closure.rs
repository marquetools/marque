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
    Citation, FactRef, MarkingScheme, ReplacementIntent, Scope, SectionLetter, capco,
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
// Authority: CAPCO-2016 §H.8 p154 (RELIDO template) + §D.2 Table 3
// rule 17 (FD&R defaults for caveated content; verified against
// `crates/capco/docs/CAPCO-2016.md`).
// ---------------------------------------------------------------------------

/// Confidence scalar emitted by S008 (`relido-implied-by-closure`)
/// alongside its `FactAdd` fix intent.
///
/// **Calibration.** Mirrors `BARE_NATO_REQUIRES_REL_TO_CONFIDENCE = 0.85` —
/// example/closure-derived guidance that ships at `Severity::Suggest`
/// with confidence high enough to clear a relaxed
/// `confidence_threshold` when paired with `[rules] S008 = "fix"`.
/// The §H.8 p154 RELIDO template + §D.2 Table 3 rule 17 backing the
/// CLOSURE_RELIDO_SCI / CLOSURE_RELIDO_US_CLASS rows is template-
/// prose plus FD&R-defaults derivation, not "MUST"-mandate prose; the
/// suggest channel is the right home.
const SUGGEST_CONFIDENCE: f32 = 0.85;

/// Shared `CapcoScheme` used by S008's `check()` to apply the closure
/// fixpoint to a portion's marking. Constructed lazily — `CapcoScheme::new()`
/// runs the constraint/page-rewrite/closure-rule catalog build once,
/// then every `check()` call borrows the cached instance instead of
/// reconstructing it per-portion. Mirrors the `SCHEME` pattern in
/// `rules_declarative.rs` (the wrapper-layer file slated for retirement
/// in the post-#578 refactor); having an independent instance here
/// survives that retirement without coupling S008 to a file being
/// deleted.
static SCHEME: std::sync::LazyLock<CapcoScheme> = std::sync::LazyLock::new(CapcoScheme::new);

/// Rule **S008** — `relido-implied-by-closure`.
///
/// Fires on a portion whose closure-applied projection carries RELIDO
/// in `dissem_us` AND whose source text does NOT already carry RELIDO.
/// Emits a `Severity::Suggest` diagnostic with a
/// `FactAdd(TOK_RELIDO, Scope::Portion)` intent at confidence
/// [`SUGGEST_CONFIDENCE`].
///
/// # Closure-based trigger detection
///
/// The rule runs `SCHEME.closure(marking)` and compares the
/// post-closure dissem axis against the pre-closure state. This is
/// more robust than hand-rolling the closure trigger / suppressor
/// logic because:
///
/// - `CLOSURE_RELIDO_SCI` triggers on `CAT_SCI` presence and
///   suppresses on `FDR_OR_RELIDO_INCOMPAT` (NOFORN / RELIDO / REL TO
///   / DISPLAY ONLY / EYES plus six per-compartment SCI sentinels
///   plus FGI / JOINT / NATO classification — at least 14 distinct
///   tokens with subtle interactions).
/// - `CLOSURE_RELIDO_US_CLASS` triggers on US collateral classification
///   and suppresses on `RELIDO_US_CLASS_SUPPRESSORS` (the same FD&R
///   dominators plus six per-compartment SCI sentinels).
/// - Both closures interact with `with_noforn_injected` (the §H.8
///   p145 supersession overlay) which strips RELIDO whenever NOFORN
///   appears at any iteration.
///
/// Replicating that decision tree inline would double the source-of-
/// truth surface for the same policy decision. Calling
/// `scheme.closure(...)` once and reading the result keeps S008
/// aligned with the closure catalog by construction. The short-
/// circuit in `CapcoScheme::closure` (line 561,
/// `any_closure_trigger_fires`) returns the input identically when
/// no trigger fires, so the cost on non-triggering portions is bounded
/// to a single trigger check.
///
/// # Early-return clauses (in order)
///
/// 1. **Portion-only**: `ctx.marking_type != MarkingType::Portion`.
///    Banner roll-up flows automatically once each portion carries
///    RELIDO; firing on a banner would double-report.
/// 2. **RELIDO already present**: `attrs.dissem_us` contains
///    `DissemControl::Relido`. Nothing to suggest.
/// 3. **Closure does not inject RELIDO**: the closure short-circuited
///    on `any_closure_trigger_fires`, OR a suppressor blocked the
///    cone, OR the with_noforn_injected overlay stripped it. No
///    diagnostic — the lattice-layer decided RELIDO is not implied.
///
/// # Fix shape
///
/// `ReplacementIntent::FactAdd { token: FactRef::Cve(TOK_RELIDO),
/// scope: Scope::Portion }` — the engine's renderer composes the
/// post-add marking back into bytes. This is the same intent shape
/// E021 uses for `FactAdd(NOFORN)` (see
/// `rules_declarative.rs::aea_noforn_add_intent`); the engine handles
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
const AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 8, 154)];

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

        // Clause 1: portion-only.
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        // Clause 2: RELIDO already present — nothing to suggest. Cheap
        // check that short-circuits before the closure call.
        if attrs
            .dissem_iter()
            .any(|d| matches!(d, DissemControl::Relido))
        {
            return vec![];
        }

        // Clause 3: run the closure and check the post-closure state.
        // `SCHEME.closure(marking)` short-circuits via
        // `any_closure_trigger_fires` when no closure rule's trigger
        // fires (the bench-corpus typical case), returning the input
        // marking identically. When a trigger fires, the fixpoint
        // loop converges in 1–2 iterations on real-world inputs
        // (proptest harness pins MAX_CLOSURE_ITERATIONS as the
        // worst-case cap).
        let marking = CapcoMarking::new(attrs.clone());
        let closed = SCHEME.closure(marking);
        let closure_adds_relido = closed
            .0
            .dissem_us
            .iter()
            .any(|d| matches!(d, DissemControl::Relido));
        if !closure_adds_relido {
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
            // Typed Citation anchors at §H.8 p154 (RELIDO grammar);
            // the §D.2 Table 3 row-17 cross-reference lives in the
            // rule doc comment.
            capco(SectionLetter::H, 8, 154),
            fix_intent,
        )]
    }
}
