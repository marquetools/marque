// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! JOINT-classification rules.
//!
//! - [`JointUsaFirstRule`] — USA-first style convention for JOINT
//!   country lists (Info severity by default).
//! - [`JointDisunityCollapseRule`] — page-level JOINT-disunity →
//!   FGI collapse Warn diagnostic.
//!
//! Predicate IDs live on each rule's `RuleId::new(...)` — the wire
//! string is the single source of truth.

use marque_ism::{CanonicalAttrs, MarkingClassification, TokenKind};
use marque_rules::{
    Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Phase, Recognition,
    Rule, RuleContext, RuleId, Severity,
};
use marque_scheme::{Citation, RecanonScope, ReplacementIntent, SectionLetter, capco};

use crate::lattice::JointSet;
use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// Rule: joint-usa-first (style)
// ---------------------------------------------------------------------------

/// Prefer USA first in JOINT country lists.
///
/// # Authority: convention, not §H.3
///
/// CAPCO-2016 §H.3 p56 prescribes **pure alphabetical** order
/// for JOINT country lists ("Country trigraph codes are listed
/// alphabetically followed by tetragraph codes in alphabetical order").
/// The section has NO USA-first carve-out. JOINT country-list
/// canonicalization (pure-alpha per §H.3 p56) lives in the renderer
/// (`render_classification.rs`), not in a rule.
///
/// However, every other US-authored country list **does** lead with
/// USA — REL TO §H.8 pp 150–151 is explicit ("After 'USA',
/// list the required one or more trigraph country codes..."). The IC
/// practice of rendering USA first in JOINT lists is a widespread
/// convention that extends this REL-TO pattern across all
/// country-list contexts, even where CAPCO is silent.
///
/// This rule encodes that convention as a **style rule** (`Severity::Info`
/// by default). It does not claim §H.3 authority; the rule doc and
/// diagnostic citation make the "convention, not mandate" framing
/// explicit. Orgs that want strict §H.3 conformance can disable the rule
/// in `.marque.toml`; orgs that want USA-first auto-applied can set it to
/// `"fix"`.
///
/// # Predicate
///
/// Fires on a banner-context `MarkingClassification::Joint` when the
/// country list contains USA AND USA is NOT the first country. The
/// rule only fires on banners — portion-form JOINT is rarely used, and
/// applying convention-based style to portions is a judgment call best
/// deferred.
///
/// # Interaction with the renderer's canonical JOINT ordering
///
/// JOINT country-list canonicalization is a renderer concern, not a
/// rule one: `MarkingScheme::render_canonical` emits
/// **pure-alphabetical** order per §H.3 p56. This rule is the only one
/// that touches JOINT ordering, layering the IC USA-first *convention*
/// above the renderer default. The two compose cleanly because the rule
/// runs at the rule layer and the renderer runs at fix-application time:
///
/// - The rule fires first (Info by default) on a banner JOINT list that
///   contains USA out of first position, emitting a
///   `Recanonicalize { RecanonScope::Page }` fix intent.
/// - If the fix is applied (org configures `"fix"`), the engine invokes
///   `MarkingScheme::render_canonical`, which re-renders the JOINT
///   block; the convention-bearing intent reaches the renderer as the
///   canonical target, so the downstream canonical form respects
///   USA-first.
/// - If the rule is configured off, no JOINT-ordering rule fires and the
///   renderer produces pure-alphabetical order per §H.3 p56 — the
///   strict-conformance default.
///
/// No competing JOINT-ordering rule exists, so no rule-id overlap guard
/// or fix-ordering tiebreaker applies to this list type; this is the
/// sole owner of the USA-first convention layer.
///
/// # Audit content-ignorance
///
/// The diagnostic and fix-intent messages carry no document bytes: the
/// `Diagnostic.fix` field is a `FixIntent` whose `replacement` is
/// `Recanonicalize { RecanonScope::Page }`, the `Recognition` is a
/// scalar, and the `Message` is a closed template + closed args. The
/// structural payload keeps document text out of the audit record by
/// construction (Constitution V).
pub(super) struct JointUsaFirstRule;

/// Secondary CAPCO §-citation for this rule.
///
/// The typed `Citation` on the emitted diagnostic carries one passage
/// (§H.3 p56). This constant pins the cross-reference to §H.8 p150 (the
/// REL TO USA-first convention this rule ports to JOINT classifications)
/// structurally, so a regression that loses the connection still fails a
/// test rather than only mutating a doc-comment.
///
/// §H.8 pp 150-151 establish the REL TO USA-first convention ("USA
/// first, remaining trigraphs alphabetical"). The anchor uses p150 since
/// typed `Citation` holds a single page; the range "pp 150-151" lives in
/// the rule doc-comment.
///
/// `#[allow(dead_code)]`: see [`DECLASSIFY_MISPLACED_CROSS_REFS`] for the
/// rationale — this is rule-authoritative metadata read by
/// `citation_cross_refs_tests` (bottom of this file). The runtime
/// `Rule::cited_authorities()` surface reads [`JOINT_USA_FIRST_AUTHORITIES`]
/// instead, which combines the primary `§H.3 p56` anchor with the
/// `JOINT_USA_FIRST_CROSS_REFS` cross-references in one slice.
#[allow(dead_code)]
pub(crate) const JOINT_USA_FIRST_CROSS_REFS: &[Citation] = &[capco(SectionLetter::H, 8, 150)];

/// Citations this rule may emit on diagnostics. Primary anchor §H.3 p56
/// (the JOINT pure-alpha rule the IC convention layers above) plus
/// the §H.8 p150 REL TO precedent it ports forward. See
/// [`Rule::cited_authorities`] for the corpus-fidelity gate contract.
const JOINT_USA_FIRST_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::H, 3, 56),
    capco(SectionLetter::H, 8, 150),
];

impl Rule<CapcoScheme> for JointUsaFirstRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.classification.joint-usa-first-style")
    }
    fn name(&self) -> &'static str {
        "joint-usa-first"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    /// Phase::WholeMarking: emits `ReplacementIntent::Recanonicalize`
    /// at `RecanonScope::Page`; the engine re-renders the JOINT
    /// classification across the candidate scope.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        JOINT_USA_FIRST_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{CountryCode, MarkingType};
        if ctx.marking_type != MarkingType::Banner {
            return vec![];
        }
        let Some(MarkingClassification::Joint(j)) = &attrs.classification else {
            return vec![];
        };
        if j.countries.len() < 2 {
            // Single-country JOINT (or zero) can't have USA out of
            // first position meaningfully.
            return vec![];
        }
        if !j.countries.contains(&CountryCode::USA) {
            // JOINT without USA is anomalous per §H.3 p163 but
            // not S003's concern. Let other rules flag it.
            return vec![];
        }
        if j.countries.first() == Some(&CountryCode::USA) {
            return vec![];
        }

        // Locate the `Classification` token to anchor the diagnostic
        // span; `MarkingScheme::render_canonical` produces the canonical
        // JOINT bytes at fix-application time, so no replacement bytes
        // are computed here.
        let Some(classification_tok) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
        else {
            return vec![];
        };

        // Audit content-ignorance: the message carries no runtime
        // country lists (those would be document bytes). The typed
        // `Message` identifies the ordering-violation class for the
        // JOINT axis.
        //
        // Build the category-bearing `Message` once and clone it for
        // both the parent diagnostic and the `FixIntent` (#739). Both
        // describe the same JOINT-axis ordering violation, so they
        // mirror the same (template, args) pair — matching the
        // FixIntent-mirrors-parent convention in `nato.rs`
        // (`WrongTokenForm` + `token`) and `dissem_closure.rs`
        // (`RequiredByPresence`). Dropping `category` from only the
        // FixIntent message would lose the JOINT axis context for any
        // consumer that reads `FixIntent.message` rather than the
        // parent `Diagnostic.message`.
        //
        // `CAT_JOINT_CLASSIFICATION` is a `CategoryId` constant — a
        // permitted audit identifier, not document content.
        // `MessageTemplate::NonCanonicalOrder` documents `category` as
        // its arg ("which axis is out of order"), so the category is
        // meant to flow through (Constitution V).
        let message = Message::new(
            MessageTemplate::NonCanonicalOrder,
            MessageArgs {
                category: Some(crate::scheme::CAT_JOINT_CLASSIFICATION),
                ..MessageArgs::default()
            },
        );

        // Structural FixIntent only. JOINT classification rendering is a
        // page-scope concern (the banner-line classification axis); the
        // convention is layered above the renderer's §H.3 pure-alpha
        // default.
        //
        // Citation: §H.3 p56 prescribes pure alphabetical for JOINT
        // with no USA-first carve-out; S003 encodes the IC convention
        // observed across REL TO (§H.8 pp 150-151). Typed Citation
        // anchors at §H.3 p56; the cross-reference to §H.8 lives in
        // the rule doc comment.
        let citation = capco(SectionLetter::H, 3, 56);
        let fix_intent = FixIntent {
            replacement: ReplacementIntent::Recanonicalize {
                scope: RecanonScope::Page,
                prior: None,
            },
            confidence: Recognition::strict(),
            feature_ids: Default::default(),
            // #739: mirror the parent diagnostic's category-bearing
            // message so the JOINT axis context survives in the
            // FixIntent's audit-record message.
            message: message.clone(),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            classification_tok.span,
            ctx.candidate_span,
            message,
            citation,
            fix_intent,
        )]
    }
}

// ===========================================================================
// JOINT producer-disunity collapse (issue #461)
// ===========================================================================
//
// Authority (verified against CAPCO-2016.md):
// - §H.3 p57 (JOINT not carried to banner in US documents — Derivative
//   Use bullets specify the FGI [LIST] migration trigger).
// - §H.7 p123 (FGI source-acknowledged form — the grammar the
//   migrated producers render under).
//
// Audit content-ignorance: the diagnostic message carries no document
// text. Permitted identifiers: `CountryCode` canonical trigraphs
// (vocabulary atoms), `Span` byte offsets, category IDs. The message
// template uses placeholders only (Constitution V).

/// JOINT producer-disunity collapse rule.
///
/// Fires when every portion on a page is JOINT-classified but the
/// portions disagree on their producer (country) list. The banner
/// cannot roll up the JOINT marking because the per-portion producer
/// lists don't share a unanimous set; per §H.3 p57 + §H.7 p123 the
/// non-US producers migrate to FGI [LIST], and JOINT is dropped from
/// the banner.
///
/// **Mixed JOINT + non-JOINT portions** (§H.3 p57 — "the JOINT
/// marking is not carried forward to the banner line in US
/// documents") do **NOT** fire this rule. That case is `JointSet::Mixed`
/// and is handled by the existing PageContext-resident
/// `expected_fgi_marker` path; no diagnostic emits on `Mixed`.
///
/// Severity: `Warn`. JOINT disunity is a subtractive-fix case.
///
/// **Fix payload deferred.** The cross-axis JOINT → FGI [LIST]
/// migration is a renderer-canonical concern, not a single-span text
/// replacement: a JOINT-disunity page has no banner JOINT block to
/// rewrite (§H.3 p57 says JOINT does not roll up to the banner), the
/// fix would have to edit each portion AND emit a new banner-shaped
/// FGI [LIST] elsewhere, and `Diagnostic::text_correction` /
/// `ReplacementIntent::FactAdd` / `FactRemove` / `Recanonicalize` are
/// all single-axis-scoped. The `MarkingScheme::render_canonical`
/// trait surface is the right home for this transformation. The
/// diagnostic surfaces the transformation today so users have an audit
/// trail without an auto-applied fix.
///
/// Authority: §H.3 p57 (JOINT not carried to banner — Derivative
/// Use bullets specify the FGI [LIST] migration trigger) + §H.7 p123
/// (FGI grammar). Verified against `crates/capco/docs/CAPCO-2016.md`.
///
/// **Phase: PageFinalization.** The engine dispatches this rule once
/// per page on the page-level fixpoint snapshot. Firing only on
/// `MarkingType::Banner` would miss banner-first layouts (no closing
/// banner means no Banner candidate runs against a non-empty
/// PageContext); firing on Portion candidates would misread an
/// intermediate snapshot as DisunityCollapse before the final
/// non-JOINT portion arrived. PageFinalization avoids both: the rule
/// fires exactly once per page on the closed state, so banner-first
/// layouts fire via the end-of-document path and Mixed-page
/// false-positives can't recur.
pub(super) struct JointDisunityCollapseRule;

/// Citations this rule may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the corpus-fidelity gate contract.
const JOINT_DISUNITY_COLLAPSE_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 3, 57)];

impl Rule<CapcoScheme> for JointDisunityCollapseRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.fgi.joint-disunity-collapses-to-fgi")
    }
    fn name(&self) -> &'static str {
        "joint-disunity-collapse"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::PageFinalization: observes the page-level fixpoint
    /// snapshot of the classification axis. The engine dispatches this
    /// rule once per page at every scanner-emitted
    /// `MarkingType::PageBreak` BEFORE the PageContext reset, plus once
    /// at end-of-document.
    fn phase(&self) -> Phase {
        Phase::PageFinalization
    }
    /// Trusted: implementation is a pure read-only check over
    /// `JointSet::from_attrs_iter`'s deterministic state machine plus
    /// a `format!` message synthesis using only `CountryCode` canonical
    /// trigraphs and a fixed §-citation. No mutable global state, no
    /// I/O, no allocation that could fail unexpectedly; the rule is
    /// safe to skip `catch_unwind`.
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        JOINT_DISUNITY_COLLAPSE_AUTHORITIES
    }
    fn check(&self, _attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Phase::PageFinalization invariant: the engine's
        // `dispatch_page_finalization` force-initializes
        // `ctx.page_portions` and `ctx.page_marking` before invoking
        // the rule. The defensive `.as_ref()?` early-return below is
        // belt-and-suspenders so the rule stays safe under future
        // engine refactors that might relax the invariant; it should
        // never fire in production.
        //
        // This rule intentionally reads the per-portion attrs slice
        // (`ctx.page_portions`) rather than `ctx.page_marking`.
        // `JointSet::from_attrs_iter` requires the per-portion
        // `CanonicalAttrs` slice that `ProjectedMarking` does not
        // expose (the JointSet `DisunityCollapse` state is structurally
        // per-portion).
        let Some(page_portions) = ctx.page_portions.as_ref() else {
            return vec![];
        };
        let portions: &[CanonicalAttrs] = page_portions.as_ref();

        let joint_set = JointSet::from_attrs_iter(portions);
        if !joint_set.is_disunity_collapse() {
            return vec![];
        }

        let Some(non_us) = joint_set.disunity_collapse_non_us_producers() else {
            return vec![];
        };

        // Render the producer set as canonical trigraphs, sorted
        // alphabetically. These are CountryCode vocabulary atoms;
        // no document text leaks (Constitution V).
        let producers_str: String = non_us
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        // Diagnostic span anchor: the engine passes a zero-length
        // `Span(boundary_offset, boundary_offset)` at the page-break
        // boundary (or `source.len()` for the EOD dispatch).
        // PageContext stores `Box<[CanonicalAttrs]>` for its portions
        // — no per-portion span is tracked. Per-portion span
        // precision would require extending the hot-path PageContext
        // data type for a single diagnostic with no fix, which the
        // PR brief judged scope-creep. The boundary anchor is the
        // best available pointer today; users joining "which page
        // had disunity?" map the byte offset to a page number via
        // their own document-position metadata.
        //
        // Authority: §H.3 p57 (Derivative Use bullets specify the
        // FGI [LIST] migration trigger) + §H.7 p123 (FGI grammar).
        // Verified against `crates/capco/docs/CAPCO-2016.md`.
        // The runtime `producers_str` is not interpolated into the
        // message. The typed `MessageTemplate::BannerRollupMismatch`
        // with `category=CAT_JOINT_CLASSIFICATION` identifies the
        // collapse-to-FGI class. Typed Citation anchors at §H.3 p57
        // (Derivative Use FGI [LIST] migration trigger); the §H.7
        // p123 FGI grammar reference lives in the rule doc comment.
        let _ = producers_str;
        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            ctx.candidate_span,
            Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs {
                    category: Some(crate::scheme::CAT_JOINT_CLASSIFICATION),
                    ..MessageArgs::default()
                },
            ),
            capco(SectionLetter::H, 3, 57),
            None,
        )]
    }
}
