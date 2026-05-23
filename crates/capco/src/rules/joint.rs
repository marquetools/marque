// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! JOINT-classification rules.
//!
//! - [`JointUsaFirstRule`] (`capco:portion.joint.usa-first-style`)
//! - [`JointDisunityCollapseRule`]
//!   (`capco:page.joint.joint-disunity-collapse-to-fgi`)

use marque_ism::{CanonicalAttrs, MarkingClassification, TokenKind};
use marque_rules::{
    Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Phase,
    Rule, RuleContext, RuleId, Severity,
};
use marque_scheme::{Citation, RecanonScope, ReplacementIntent, SectionLetter, capco};

use crate::lattice::JointSet;
use crate::rules::helpers::canonicalize_trigraph_list;
use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// Rule: S003 — joint-usa-first (style)
// ---------------------------------------------------------------------------

/// S003: Prefer USA first in JOINT country lists.
///
/// # Authority: convention, not §H.3
///
/// CAPCO-2016 §H.3 p56 prescribes **pure alphabetical** order
/// for JOINT country lists ("Country trigraph codes are listed
/// alphabetically followed by tetragraph codes in alphabetical order").
/// The section has NO USA-first carve-out. Prior to PR #97 / T035c-18,
/// the pre-decomposition JOINT fix path (then E020, later folded into
/// E060 — both retired) incorrectly elevated USA to the front — that
/// was an authority-drift violation of Constitution VIII. #97 narrowed
/// the JOINT canonicalization path to pure alpha; PR 3c.B Commit 6
/// retired the rule-side path entirely into `render_classification.rs`.
///
/// However, every other US-authored country list **does** lead with
/// USA — REL TO §H.8 pp 150–151 is explicit ("After 'USA',
/// list the required one or more trigraph country codes..."). The IC
/// practice of rendering USA first in JOINT lists is a widespread
/// convention that extends this REL-TO pattern across all
/// country-list contexts, even where CAPCO is silent.
///
/// S003 encodes that convention as a **style rule** (`Severity::Info`
/// by default). It does not claim §H.3 authority; the rule doc and
/// diagnostic citation make the "convention, not mandate" framing
/// explicit. Orgs that want strict §H.3 conformance can disable S003
/// via `S003 = "off"` in `.marque.toml`. Orgs that want USA-first
/// auto-applied can configure `S003 = "fix"`.
///
/// # Predicate
///
/// Fires on a banner-context `MarkingClassification::Joint` when the
/// country list contains USA AND USA is NOT the first country. The
/// rule only fires on banners (matching S001/S002's banner-only
/// scope) — portion-form JOINT is rarely used, and applying
/// convention-based style to portions is a judgment call best
/// deferred.
///
/// # Interaction with E060 (JOINT row)
///
/// E060's JOINT row and S003 can both fire on the same JOINT list
/// when it is neither pure-alpha nor USA-first (e.g., `GBR USA AUS`
/// is not alpha AND not USA-first). Both fixes target the same
/// Classification token span:
///
/// - E060 (non-canonical input walker, JOINT row) fix: `AUS GBR USA`
///   (pure alpha per §H.3 p56).
/// - S003 fix: `USA AUS GBR` (USA first, rest alpha per convention).
///
/// Under FR-016's rule-id tiebreaker ("E060" < "S003" lexically),
/// E060 wins the overlap guard and applies. On re-lint, E060 is
/// silent (list now pure-alpha) and S003 still wants USA first;
/// running fix again converges to `USA AUS GBR`. Two passes. Orgs
/// that want single-pass USA-first convergence can disable E060
/// for JOINT (currently not configurable; would need a per-list-type
/// severity override — follow-up).
///
/// (Pre-PR-3b.F this was E020; PR 3b.F retired E020 into the E060
/// walker, which preserved the same fix shape and citation but
/// changed the rule-ID. PR 3c.B Commit 6 retired E060 into the
/// renderer.)
///
/// # Constitution V audit-content-ignorance
///
/// The message at lines below interpolates `joined_actual_str` and
/// `joined_canonical_str`, both derived from
/// `CountryCode::as_str()` over the *parsed* country list — those
/// strings are CVE-canonical trigraphs (`USA`, `GBR`, `CAN`, …)
/// drawn from the closed ODNI `CVEnumISMCATRelTo` set, **not**
/// document text. Post-Commit-10 the audit record carries no
/// document bytes for this path: the `Diagnostic.fix` field is a
/// `FixIntent` whose `replacement` is
/// `Recanonicalize { RecanonScope::Page }`, the `Confidence` is a
/// scalar, and the `Message` is a closed template + closed args.
/// The pre-Commit-10 `FixProposal.original` byte channel retired
/// with the `mvp-2 → mvp-3` schema flip; the structural payload
/// closes G13 by construction at this rule's emission site.
pub(crate) struct JointUsaFirstRule;

/// S003 secondary CAPCO §-citations.
///
/// PR 10.A.1 Commit 4: the migration to typed `Citation` collapsed the
/// pre-migration string form `"CAPCO-2016 §H.3 p56 + §H.8 pp 150-151 (IC convention)"`
/// into a single `capco(SectionLetter::H, 3, 56)` value on the emitted
/// diagnostic. The cross-reference to §H.8 p150 (REL TO USA-first
/// convention — the analogous IC convention S003 layers above §H.3's
/// pure-alpha JOINT default) survived in the rule's doc-comment but
/// was un-checked. This constant pins the dropped cross-reference
/// structurally.
///
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1
/// Commit 4 authorship per Constitution VIII propagation rule: §H.8
/// pp 150-151 establish the REL TO USA-first convention ("USA first,
/// remaining trigraphs alphabetical") that S003 ports to JOINT
/// classifications. Anchor citation uses p150 since typed `Citation`
/// holds a single page; the range "pp 150-151" lives in the rule
/// doc-comment.
///
/// `#[allow(dead_code)]`: see [`E005_CROSS_REFS`] for the rationale —
/// this is rule-authoritative metadata read by
/// `citation_cross_refs_tests` (bottom of this file). The runtime
/// `Rule::cited_authorities()` surface reads [`S003_AUTHORITIES`]
/// instead, which combines the primary `§H.3 p56` anchor with the
/// `S003_CROSS_REFS` cross-references in one slice.
#[allow(dead_code)]
pub(crate) const S003_CROSS_REFS: &[Citation] = &[capco(SectionLetter::H, 8, 150)];

/// Citations S003 may emit on diagnostics. Primary anchor §H.3 p56
/// (the JOINT pure-alpha rule the IC convention layers above) plus
/// the §H.8 p150 REL TO precedent S003 ports forward. See
/// [`Rule::cited_authorities`] for the F.1 gate contract.
const S003_AUTHORITIES: &[Citation] = &[
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
        S003_AUTHORITIES
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

        // Canonicalize: USA first, remaining trigraphs alphabetical.
        let canonical = canonicalize_trigraph_list(&j.countries, true);

        // Locate the `Classification` token to anchor the diagnostic
        // span; the replacement-bytes computation retired with the
        // mvp-3 cutover (the engine's `render_canonical` produces
        // canonical JOINT bytes at fix-application time).
        let Some(classification_tok) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
        else {
            return vec![];
        };

        // G13: drop the runtime country lists from the message;
        // they appeared in the format!-built string but are document
        // bytes by way of `j.countries.iter().map(|t| t.as_str())`.
        // The typed `Message` identifies the ordering-violation class
        // for the JOINT axis.
        let _ = canonical; // canonical is consumed by the fix_intent path below
        let message = Message::new(
            MessageTemplate::NonCanonicalOrder,
            MessageArgs {
                category: Some(crate::scheme::CAT_JOINT_CLASSIFICATION),
                ..MessageArgs::default()
            },
        );

        // PR 3c.B Commit 10 — structural FixIntent only. JOINT
        // classification rendering is a page-scope concern (the
        // banner-line classification axis); the convention is layered
        // above the renderer's §H.3 pure-alpha default.
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
            },
            confidence: Confidence::strict(1.0),
            feature_ids: Default::default(),
            message: Message::new(MessageTemplate::NonCanonicalOrder, MessageArgs::default()),
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
// W004 — JOINT producer-disunity collapse (issue #461 — Phase::PageFinalization)
// ===========================================================================
//
// Authority (verified 2026-05-16 against CAPCO-2016.md):
// - §H.3 p57 (JOINT not carried to banner in US documents — Derivative
//   Use bullets specify the FGI [LIST] migration trigger).
// - §H.7 p123 (FGI source-acknowledged form — the grammar the
//   migrated producers render under).
//
// Constitution V Principle V G13: the W004 diagnostic message MUST
// NOT contain document text. Permitted identifiers: `CountryCode`
// canonical trigraphs (vocabulary atoms), `Span` byte offsets,
// category IDs. The message template below uses placeholders only
// — no input bytes are interpolated.

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
/// documents") do **NOT** fire W004. That case is `JointSet::Mixed`
/// (C-3 PR 4b-B follow-up — was `Bottom` pre-split) and is handled
/// by the existing PageContext-resident `expected_fgi_marker` path;
/// no W004 diagnostic emits on `Mixed`.
///
/// Severity: `Warn` (per `feedback_dissem_conflicts_emit_subtractive_fix.md`,
/// JOINT disunity is a subtractive-fix case).
///
/// **Fix payload deferred.** The cross-axis JOINT → FGI [LIST]
/// migration is a renderer-canonical concern, not a single-span text
/// replacement: a JOINT-disunity page has no banner JOINT block to
/// rewrite (§H.3 p57 says JOINT does not roll up to the banner), the
/// fix would have to edit each portion AND emit a new banner-shaped
/// FGI [LIST] elsewhere, and `Diagnostic::text_correction` /
/// `ReplacementIntent::FactAdd` / `FactRemove` / `Recanonicalize` are
/// all single-axis-scoped. The `MarkingScheme::render_canonical`
/// trait surface (PR 5+ Stage 4) is the right home for this
/// transformation. The W004 diagnostic surfaces the transformation
/// today so users have an audit trail without an auto-applied fix.
///
/// Authority: §H.3 p57 (JOINT not carried to banner — Derivative
/// Use bullets specify the FGI [LIST] migration trigger) + §H.7 p123
/// (FGI grammar). Verified 2026-05-16 against
/// `crates/capco/docs/CAPCO-2016.md`.
///
/// **Phase: PageFinalization (issue #461).** Pre-#461 W004 declared
/// `Phase::WholeMarking` and gated on `MarkingType::Banner`, which
/// produced a documented false-negative on banner-first layouts (no
/// closing banner → no Banner candidate ever runs against a
/// non-empty PageContext) AND a 6th-pass false-positive on Mixed
/// pages when the rule was briefly extended to Portion candidates
/// (intermediate snapshot misread as DisunityCollapse before the
/// final non-JOINT portion arrived). Phase::PageFinalization closes
/// both: the engine dispatches W004 once per page on the page-level
/// fixpoint snapshot, so banner-first layouts fire via the
/// end-of-document path and Mixed-page false-positives don't recur.
pub(crate) struct JointDisunityCollapseRule;

/// Citations W004 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const W004_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 3, 57)];

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
    /// Phase::PageFinalization (issue #461): observes the
    /// page-level fixpoint snapshot of the classification axis. The
    /// engine dispatches this rule once per page at every
    /// scanner-emitted `MarkingType::PageBreak` BEFORE the
    /// PageContext reset, plus once at end-of-document. The
    /// pre-#461 Banner-only firing produced a documented
    /// false-negative on banner-first layouts (closed by the EOD
    /// path); the 6th-pass Portion-firing experiment produced a
    /// Mixed-page false-positive (not recur under PageFinalization
    /// because the rule fires exactly once per page on the closed
    /// state).
    fn phase(&self) -> Phase {
        Phase::PageFinalization
    }
    /// Trusted: implementation is a pure read-only check over
    /// `JointSet::from_attrs_iter`'s deterministic state machine plus
    /// a `format!` message synthesis using only `CountryCode` canonical
    /// trigraphs and a fixed §-citation. No mutable global state, no
    /// I/O, no allocation that could fail unexpectedly; the rule is
    /// safe to skip `catch_unwind` per PR #448.
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        W004_AUTHORITIES
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
        // PR 4b-D.3 note (2026-05-18): W004 intentionally reads the
        // per-portion attrs slice rather than `ctx.page_marking`.
        // `JointSet::from_attrs_iter` requires the per-portion
        // `CanonicalAttrs` slice that `ProjectedMarking` does not
        // expose (the JointSet `DisunityCollapse` state is structurally
        // per-portion). Lifting `JointSet`'s derived state onto
        // `ProjectedMarking` is post-PR-6c future work.
        //
        // PR 6c migration (T069): read `ctx.page_portions` (the
        // `Box<[CanonicalAttrs]>` slice snapshot) instead of the
        // retired `ctx.page_context` / `PageContext::portions()`
        // accessor pair.
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
        // no document text leaks per Constitution V G13.
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
        // Re-verified 2026-05-16 against
        // `crates/capco/docs/CAPCO-2016.md`.
        // G13: drop the runtime `producers_str` interpolation. The
        // typed `MessageTemplate::BannerRollupMismatch` with
        // `category=CAT_JOINT_CLASSIFICATION` identifies the
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
