// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! NODIS / EXDIS rules (CAPCO-2016 §H.9).
//!
//! - [`NodisExdisClearsBannerRelToRule`]
//!   (`capco:banner.dissem.nodis-exdis-clears-rel-to`)
//! - [`NodisSupersedesExdisInPortionRule`]
//!   (`capco:portion.dissem.nodis-supersedes-exdis-in-portion`)

use marque_ism::{CanonicalAttrs, TokenKind, TokenSpan};
use marque_rules::{
    Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Phase,
    Rule, RuleContext, RuleId, Severity,
};
use marque_scheme::{
    Citation, FactRef, ReplacementIntent, Scope, SectionLetter, capco,
};

use crate::scheme::CapcoScheme;

// ===========================================================================
// T035c-21 PR-B: NODIS / EXDIS page-level + portion-level rules (§H.9)
// ===========================================================================
//
// Three hand-written rules that can't ride the declarative-constraint
// path. E039 and E040 read `ctx.page_marking` (the composite
// `ProjectedMarking` projection — banner-validation surface, PR 9b
// T133 / FR-006); E041 is portion-only and reads its dispatch attrs
// directly. None of the three has a single-span text replacement the
// declarative path can synthesize:
//
//   E039  — REL TO not authorized in banner when any portion has NODIS
//           or EXDIS. No fix (removing REL TO from a banner is multi-
//           span and requires human judgment on what to convey instead).
//   E040  — Banner must roll up NODIS (or EXDIS if no NODIS anywhere).
//           Insertion fix when banner already has a Non-IC dissem
//           category block; no-fix Error otherwise.
//   E041  — In a portion with both NODIS and EXDIS, NODIS supersedes
//           EXDIS. Warn-severity with no fix. Portion-only; the
//           banner case is owned by E037 (mutual exclusion, Error).
//           See the in-rule "# No auto-fix" section for why the
//           supersession is not auto-applied.

// ---------------------------------------------------------------------------
// Rule: E039 — REL TO not allowed in banner when portion has NODIS/EXDIS
// ---------------------------------------------------------------------------

/// Fires when the banner's REL TO list is populated and any portion on
/// the page carries NODIS or EXDIS.
///
/// Authority:
/// - **CAPCO-2016 §H.9 p172** (EXDIS): *"REL TO is not
///   authorized in the banner line if any portion contains EXDIS
///   information. In this case, NOFORN would convey in the banner
///   line."*
/// - **CAPCO-2016 §H.9 p174** (NODIS): same for NODIS.
///
/// # Why no fix
///
/// Removing REL TO from a banner is multi-span (the whole RelToBlock
/// comes out, along with its `//` separators), and the replacement
/// depends on whether the user wants to convert to NOFORN-only (the
/// source suggests) or take some other action. Emit an `Error`
/// diagnostic with no fix; the user decides manually.
pub(crate) struct NodisExdisClearsBannerRelToRule;

/// E037 secondary CAPCO §-citations.
///
/// PR 10.A.1 Commit 4: the migration to typed `Citation` collapsed the
/// pre-migration string form `"CAPCO-2016 §H.9 p172 + p174"` into a
/// single `capco(SectionLetter::H, 9, 172)` value on the diagnostic
/// emitted by the declarative `Conflicts` row at
/// `crates/capco/src/scheme/constraints/core_catalog.rs::core_constraints()`
/// (search for `"portion.dissem.nodis-conflicts-exdis"`). The cross-reference
/// to p174 (NODIS authority — the mutual-exclusion rule is stated
/// verbatim on both sides) survived in the catalog row's doc-comment
/// but was un-checked.
///
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1
/// Commit 4 authorship per Constitution VIII propagation rule: §H.9
/// p174 (NODIS Relationship(s) to Other Markings) states "NODIS and
/// EXDIS markings cannot be used together", mirroring §H.9 p172's
/// EXDIS-side wording. Both passages are operative for E037.
///
/// `#[allow(dead_code)]`: see [`DECLASSIFY_MISPLACED_CROSS_REFS`] for the rationale.
#[allow(dead_code)]
pub(crate) const NODIS_EXDIS_MUTEX_CROSS_REFS: &[Citation] = &[capco(SectionLetter::H, 9, 174)];

/// E038 secondary CAPCO §-citations.
///
/// PR 10.A.1 Commit 4: identical mechanism to [`NODIS_EXDIS_MUTEX_CROSS_REFS`] —
/// the declarative `Custom` row at
/// `core_constraints()::"portion.dissem.nodis-or-exdis-requires-noforn"` carries
/// only the primary §H.9 p172 anchor. The cross-reference to p174
/// (NODIS "Requires NOFORN") survived in the catalog row's
/// doc-comment but was un-checked.
///
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1
/// Commit 4 authorship per Constitution VIII propagation rule: §H.9
/// p174 (NODIS Relationship(s) to Other Markings) carries the same
/// "Requires NOFORN" clause that the rule's primary citation at p172
/// establishes for EXDIS. Both passages are operative for E038.
///
/// `#[allow(dead_code)]`: see [`DECLASSIFY_MISPLACED_CROSS_REFS`] for the rationale.
#[allow(dead_code)]
pub(crate) const NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS: &[Citation] = &[capco(SectionLetter::H, 9, 174)];

/// E039 secondary CAPCO §-citations.
///
/// PR 10.A.1 Commit 4: the migration to typed `Citation` collapsed the
/// pre-migration string form `"CAPCO-2016 §H.9 p172 + p174 (NODIS)"`
/// into a single `capco(SectionLetter::H, 9, 172)` value on the emitted
/// diagnostic. The cross-reference to p174 (NODIS authority — the
/// EXDIS rule at p172 is mirrored verbatim for NODIS at p174) survived
/// in the rule's doc-comment but was un-checked. This constant pins
/// the dropped cross-reference structurally.
///
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1
/// Commit 4 authorship per Constitution VIII propagation rule: §H.9
/// p174 (NODIS Relationship(s) to Other Markings) carries the same
/// "REL TO not authorized in banner when portion contains NODIS"
/// rule that the rule's primary citation at p172 establishes for
/// EXDIS. Both passages are operative for E039.
///
/// `#[allow(dead_code)]`: see [`DECLASSIFY_MISPLACED_CROSS_REFS`] for the rationale.
#[allow(dead_code)]
pub(crate) const NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS: &[Citation] = &[capco(SectionLetter::H, 9, 174)];

/// Citations E039 may emit on diagnostics. Combines the primary
/// `Diagnostic.citation` value (§H.9 p172 — EXDIS) with the
/// [`NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS`] cross-references (§H.9 p174 — NODIS). See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const NODIS_EXDIS_CLEARS_BANNER_REL_TO_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::H, 9, 172),
    capco(SectionLetter::H, 9, 174),
];

impl Rule<CapcoScheme> for NodisExdisClearsBannerRelToRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.nodis-exdis-clears-banner-rel-to")
    }
    fn name(&self) -> &'static str {
        "nodis-exdis-clears-banner-rel-to"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::WholeMarking: banner-scope decision combining the
    /// banner's REL TO list with the page-context expected non-IC
    /// dissem set. No fix (removing REL TO from a banner is multi-span
    /// and policy-dependent).
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        NODIS_EXDIS_CLEARS_BANNER_REL_TO_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{MarkingType, NonIcDissem};

        // Banner-only (and CAB, since CABs can carry REL TO). Portion
        // candidates are the input; they don't trigger on themselves.
        if !matches!(ctx.marking_type, MarkingType::Banner | MarkingType::Cab) {
            return vec![];
        }

        if attrs.rel_to.is_empty() {
            return vec![];
        }

        // PR 9b (T133): banner-validation reads `ctx.page_marking`
        // (the `ProjectedMarking`) instead of going through
        // `PageContext::expected_non_ic_dissem`. The projection's
        // `non_ic_dissem` field carries the same supersession-
        // resolved roll-up.
        let Some(page) = ctx.page_marking.as_ref() else {
            return vec![];
        };

        let has_nodis_or_exdis = page
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, NonIcDissem::Nodis | NonIcDissem::Exdis));
        if !has_nodis_or_exdis {
            return vec![];
        }

        // Point at the first RelToBlock span so the user sees exactly where
        // the offending REL TO is.
        //
        // Parser invariant (verified against `parse_rel_to_with_spans`, the
        // sole producer of `rel_to` entries): every push into `rel_to` is
        // immediately preceded by a `TokenKind::RelToBlock` `TokenSpan` push
        // at the two call sites in `marque-core::parser`. The
        // `attrs.rel_to.is_empty()` guard above means we reach this site
        // only when `rel_to` is non-empty, therefore the `find()` MUST
        // succeed. The `else` arm is defense-in-depth against future parser
        // changes that would violate the invariant; uses the same let-else
        // shape as S010, with a `debug_assert!` on the invariant itself
        // (not on a constant) so dev/test builds panic loud if the parser
        // ever drops the RelToBlock span while keeping `rel_to` populated.
        debug_assert!(
            attrs
                .token_spans
                .iter()
                .any(|t| t.kind == TokenKind::RelToBlock),
            "E039: candidate with non-empty rel_to has no RelToBlock token span \
             (parser invariant violation; see parse_rel_to_with_spans call sites \
             in marque-core::parser)"
        );
        let Some(block) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::RelToBlock)
        else {
            return vec![];
        };
        let span = block.span;

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            Message::new(
                MessageTemplate::ConflictsWith,
                MessageArgs {
                    category: Some(crate::scheme::CAT_REL_TO),
                    ..MessageArgs::default()
                },
            ),
            // Typed Citation anchors at §H.9 p172 (EXDIS grammar);
            // the §H.9 p174 (NODIS) cross-reference lives in the
            // rule doc comment.
            capco(SectionLetter::H, 9, 172),
            None,
        )]
    }
}


// ---------------------------------------------------------------------------
// Rule: E041 — Portion-level NODIS supersedes EXDIS
// ---------------------------------------------------------------------------

/// Fires when a portion carries BOTH NODIS and EXDIS. Emits a
/// `Warn`-severity diagnostic pointing at the EXDIS token and an
/// intent-only `FactRemove(EXDIS, Scope::Portion)` fix that the
/// engine auto-applies via the synthesis path. Per the supersession
/// rule in §H.9, NODIS survives and EXDIS is removed.
///
/// Authority:
/// - **CAPCO-2016 §H.9 p172** (EXDIS Commingling): *"When a
///   portion contains both EXDIS and NODIS information, NODIS (ND)
///   supersedes EXDIS (XD) in the portion mark."*
/// - **CAPCO-2016 §H.9 p174** (NODIS Commingling): *"If a
///   portion contains both NODIS and EXDIS information, NODIS (ND)
///   supersedes EXDIS (XD) in the portion mark."*
///
/// # Scope
///
/// Portion-only per both source passages ("in the portion mark").
/// The banner-level mutual exclusion is E037's territory — it fires
/// as `Error` there with no fix because banner-level resolution
/// depends on which portions carry which token (see E040's roll-up
/// rule for how the banner should be composed).
///
/// # Interaction with E037
///
/// E037 also fires in portion context (it's a general "NODIS and
/// EXDIS cannot coexist" rule per §H.9 p172 + p174). When a portion
/// has both tokens, both rules fire:
/// - E037 (`Error`, no fix) states the violation.
/// - E041 (`Warn`, intent-only `FactRemove`) states the supersession
///   rule: NODIS wins, so EXDIS is removed from the portion marking.
///
/// E037 emits no `FixProposal`, so the FR-016 deterministic ordering
/// (lex-min rule id wins on overlap) does not block E041's fix from
/// applying — E041 is the only diagnostic in the candidate-span group
/// that contributes an intent. After the engine applies E041, re-linting
/// the resulting portion clears both diagnostics.
///
/// # Severity and auto-fix surface
///
/// `Warn` default severity. The engine's intent-only synthesis path
/// auto-applies the fix for every severity *except* `Severity::Suggest`
/// (see `crates/engine/src/engine.rs::synthesize_intent_only_fixes`),
/// so the default emission auto-fixes. Orgs that want to surface
/// the supersession without applying it can configure
/// `E041 = "suggest"` in `.marque.toml`; orgs that want the violation
/// promoted to an error can configure `E041 = "error"`.
///
/// # Auto-fix mechanism (PR 3c.B Sub-PR 8.E.2 — unblocks E041, primary rule named in #106)
///
/// Pre-PR-3c.B-Sub-PR-8.E.2 this rule shipped as a no-fix diagnostic.
/// The blocker was that a byte-precise legacy [`FixProposal`] would
/// need to splice EXDIS *plus* an adjacent within-category `/`
/// separator, but the parser only emits `TokenKind::Separator` for
/// between-category `//` delimiters — within-category `/` bytes are
/// gap bytes that no `TokenSpan` covers. Constructing the legacy
/// proposal from rule-level position info risked over-running on
/// edge inputs and corrupting the audit record per Constitution V.
///
/// The intent-only emission path obviates that gap. The rule emits
/// `FixIntent { ReplacementIntent::FactRemove { TOK_EXDIS, Portion } }`
/// alongside the rule's `RuleContext::candidate_span` (the full
/// portion span, including the parentheses). The engine's
/// `synthesize_intent_only_fixes` calls `CapcoScheme::apply_intent`
/// to remove EXDIS from the marking's `non_ic_dissem` axis, then
/// re-renders the portion via `MarkingScheme::render_canonical`
/// (delegated to `render_portion`). The synthesized
/// `FixProposal.span` covers the full candidate, so the
/// within-category `/` byte is replaced as part of the re-rendered
/// portion — no parser change required. Issue #106 remains open as
/// a tracking ticket for any future rule that genuinely needs
/// byte-precise within-category separator info (i.e., a rule that
/// cannot route through re-rendering); E041 itself no longer
/// blocks on it.
pub(crate) struct NodisSupersedesExdisInPortionRule;

/// Citations E041 may emit on diagnostics. Primary anchor §H.9 p174
/// (NODIS — the dominating token); the §H.9 p172 (EXDIS) cross-
/// reference is also operative because both passages state the
/// supersession rule verbatim. See [`Rule::cited_authorities`] for
/// the F.1 corpus-fidelity gate contract.
const NODIS_SUPERSEDES_EXDIS_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::H, 9, 174),
    capco(SectionLetter::H, 9, 172),
];

impl Rule<CapcoScheme> for NodisSupersedesExdisInPortionRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.dissem.nodis-supersedes-exdis-in-portion")
    }
    fn name(&self) -> &'static str {
        "nodis-supersedes-exdis-in-portion"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: emits `ReplacementIntent::FactRemove` at
    /// `Scope::Portion`; the engine re-renders the full portion via
    /// `candidate_span`. Span shape is whole-marking by construction.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        NODIS_SUPERSEDES_EXDIS_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{MarkingType, NonIcDissem};

        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        let has_nodis = attrs
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, NonIcDissem::Nodis));
        let has_exdis = attrs
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, NonIcDissem::Exdis));
        if !(has_nodis && has_exdis) {
            return vec![];
        }

        // Locate the EXDIS token span for the diagnostic pointer.
        // The parser emits one `TokenKind::NonIcDissem` token per
        // entry in `attrs.non_ic_dissem` in source order.
        let non_ic_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::NonIcDissem)
            .collect();
        let Some(exdis_idx) = attrs
            .non_ic_dissem
            .iter()
            .position(|d| matches!(d, NonIcDissem::Exdis))
        else {
            return vec![];
        };
        let Some(exdis_span_tok) = non_ic_spans.get(exdis_idx) else {
            return vec![];
        };

        // PR 3c.B Sub-PR 8.E.2 — intent-only emission (unblocks E041 in #106).
        // The diagnostic's `span` points at the EXDIS token (the
        // user-facing pointer); `candidate_span` is the full portion
        // candidate so the engine's `synthesize_intent_only_fixes`
        // knows which scope-bytes to re-render after
        // `CapcoScheme::apply_intent` removes EXDIS from the
        // marking's non-IC-dissem axis. The within-category `/`
        // separator that previously blocked byte-precise splicing is
        // sidestepped because the engine replaces the full
        // candidate_span with the re-rendered output — no parser
        // change required (see the `# Auto-fix mechanism` section in
        // the rustdoc above for the issue-#106 sidestep rationale).
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            exdis_span_tok.span,
            ctx.candidate_span,
            Message::new(
                MessageTemplate::ConflictsWith,
                MessageArgs {
                    category: Some(crate::scheme::CAT_NON_IC_DISSEM),
                    ..MessageArgs::default()
                },
            ),
            // Typed Citation anchors at §H.9 p174 (NODIS — the
            // dominating token); §H.9 p172 (EXDIS) cross-referenced
            // in the rule doc comment.
            capco(SectionLetter::H, 9, 174),
            nodis_supersedes_exdis_intent(),
        )]
    }
}

/// Build the `FactRemove { EXDIS, Scope::Portion }` intent emitted by
/// `NodisSupersedesExdisInPortionRule`. EXDIS is the rejected token
/// per §H.9 p172 + p174 ("NODIS (ND) supersedes EXDIS (XD) in the
/// portion mark"). Scope is portion-only: the supersession rule
/// names the portion mark explicitly at both source passages.
///
/// Confidence is `Confidence::strict(1.0)` — the source is
/// unambiguous about which token survives, and the strict recognizer
/// path is what produced the parse that surfaced both tokens.
///
/// `feature_ids` uses `Default::default()` (empty `SmallVec`) to
/// stay consistent with the other strict-path intent builders in
/// this crate (see `relido_remove_intent` in `rules_declarative.rs`).
fn nodis_supersedes_exdis_intent() -> FixIntent<CapcoScheme> {
    use crate::scheme::{TOK_EXDIS, TOK_NODIS};
    FixIntent {
        replacement: ReplacementIntent::fact_remove(FactRef::Cve(TOK_EXDIS), Scope::Portion),
        confidence: Confidence::strict(1.0),
        feature_ids: Default::default(),
        // `ConflictsWith` (not `SupersededToken`): §H.9 mutual-exclusion
        // with a dominated + surviving token, NOT §F deprecation /
        // canonical-replacement. `token` = the dominated EXDIS;
        // `expected_token` = the surviving NODIS.
        message: Message::new(
            MessageTemplate::ConflictsWith,
            MessageArgs {
                token: Some(TOK_EXDIS),
                expected_token: Some(TOK_NODIS),
                ..MessageArgs::default()
            },
        ),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    }
}
