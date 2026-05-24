// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Banner-roll-up walker (`BannerMatchesProjectedRule`).
//!
//! Asserts the observed banner / CAB candidate matches the page's
//! projected marking for each per-category roll-up: SAR, SCI, Non-IC
//! dissem, classification, and FGI marker.
//!
//! Each catalog row carries its own rule ID, citation, severity, and
//! `evaluate` fn — so emitted diagnostics keep distinct per-row rule
//! IDs for audit-stream continuity and the overlap-guard interaction.
//! The walker's own `id()` is a bookkeeping ID (the SAR roll-up tuple);
//! the rule loop tracks via the per-row IDs on each emitted
//! `Diagnostic`.
//!
//! Single-citation discipline: each catalog row carries ONE operative
//! banner-roll-up CAPCO-§ citation. Background §-references are
//! permitted in row documentation but are not counted as the row's
//! primary citation.
//!
//! Each `evaluate_*` fn takes an explicit `&ProjectedMarking`
//! parameter; the marking-type guard and the `ctx.page_marking.as_ref()`
//! guard live on the walker's `check`.

mod eval_classification;
mod eval_fgi_marker;
mod eval_non_ic_dissem;
mod eval_sar;
mod eval_sci;

use marque_ism::CanonicalAttrs;
use marque_rules::{Diagnostic, Phase, Rule, RuleContext, RuleId, Severity};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::scheme::CapcoScheme;

use eval_classification::evaluate_classification_banner_rollup;
use eval_fgi_marker::evaluate_fgi_marker_banner_rollup;
use eval_non_ic_dissem::evaluate_non_ic_dissem_banner_rollup;
use eval_sar::evaluate_sar_banner_rollup;
use eval_sci::evaluate_sci_banner_rollup;

/// Walker that asserts the banner / CAB candidate matches the page's
/// projected marking for each per-category roll-up. See the module-level
/// doc-comment for the design rationale.
pub(super) struct BannerMatchesProjectedRule;

/// Citations the [`BannerMatchesProjectedRule`] walker may emit on
/// diagnostics, one per catalog row in [`BANNER_CATEGORY_CATALOG`].
/// The walker registers under the SAR roll-up tuple (bookkeeping ID);
/// emitted diagnostics carry per-row IDs and per-row citations from
/// this list. See [`Rule::cited_authorities`] for the corpus-fidelity
/// gate contract.
const AUTHORITIES: &[Citation] = &[
    // SAR roll-up — §H.5 p101 "All unique SAPs contained in
    // portion marks must always appear in the banner line."
    capco(SectionLetter::H, 5, 101),
    // SCI roll-up — §H.4 p61 "Use the following syntax rules
    // for both portion marks and banner lines for all published and
    // unpublished SCI control systems."
    capco(SectionLetter::H, 4, 61),
    // Non-IC dissem roll-up — §H.9 p172 (EXDIS) with §H.9
    // p174 (NODIS) cross-reference; the typed Citation anchors at
    // p172. Both are operative per the walker's evaluator doc.
    capco(SectionLetter::H, 9, 172),
    // Banner classification mismatch — §H.7 p123 (Precedence
    // Rules for Banner Line Guidance + reciprocal classification).
    capco(SectionLetter::H, 7, 123),
    // Banner FGI marker mismatch — §H.7 p124 (FGI banner-line
    // roll-up + source-concealed-dominates rule).
    capco(SectionLetter::H, 7, 124),
];

impl Rule<CapcoScheme> for BannerMatchesProjectedRule {
    fn id(&self) -> RuleId {
        // Bookkeeping ID. Per-row IDs travel on emitted diagnostics for
        // audit traceability. The walker's registered tuple IS the SAR
        // roll-up tuple.
        RuleId::new("capco", "banner.banner-rollup.sar-portions-roll-up")
    }

    fn name(&self) -> &'static str {
        "banner-matches-projected"
    }

    fn default_severity(&self) -> Severity {
        // Per-row severities take precedence on emitted diagnostics; the
        // walker-level default severity is the strictest of the three
        // catalog rows so a config that uses `BannerMatchesProjectedRule`
        // as the override anchor cannot accidentally weaken any row below
        // its authoring intent.
        Severity::Error
    }
    /// Phase::WholeMarking: banner roll-up walker (SAR / SCI / Non-IC dissem
    /// / classification / FGI marker). Every row reads the page projection
    /// across all portions and compares against the banner; fixes (when
    /// emitted) span the banner candidate.
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
        use marque_ism::MarkingType;

        // Marking-type guard (≤3 branches per D13). CABs carry only
        // authority fields (Classified By / Derived From / Declassify
        // On) — they have no classification, SCI, dissem, or FGI
        // blocks — so every row evaluator would spuriously fire
        // "banner missing X block" with a placeholder (0,0) span.
        if !matches!(ctx.marking_type, MarkingType::Banner) {
            return vec![];
        }
        // Banner-validation rules read the rolled-up shape via
        // `ctx.page_marking` (the `ProjectedMarking` projection). The
        // per-portion view is available via `ctx.page_portions`.
        let Some(page) = ctx.page_marking.as_ref() else {
            return vec![];
        };
        // Dispatch loop.
        let mut diags = Vec::new();
        for row in BANNER_CATEGORY_CATALOG {
            diags.extend((row.evaluate)(attrs, page, row));
        }
        diags
    }

    /// Catalog (id, name) pairs the walker emits on diagnostics beyond
    /// its registered `id()` / `name()`. Required by the engine's
    /// `canonicalize_rule_overrides` path so a `.marque.toml`
    /// configuring a per-row ID (or its `sci-banner-rollup`-style name
    /// alias) is accepted at engine construction.
    ///
    /// Each pair is self-canonical: the catalog ID maps to itself, the
    /// catalog name maps to the catalog ID. This keeps per-row override
    /// scope independent of the walker's bookkeeping ID.
    fn additional_emitted_ids(&self) -> &'static [(&'static str, &'static str)] {
        // The first column is the canonical wire-string form
        // (`<scheme>:<predicate_id>`); the second column is the
        // descriptive `name()` alias users may also type in
        // `.marque.toml`.
        &[
            (
                "capco:banner.banner-rollup.sar-portions-roll-up",
                "sar-banner-rollup",
            ),
            (
                "capco:banner.banner-rollup.sci-portions-roll-up",
                "sci-banner-rollup",
            ),
            (
                "capco:banner.banner-rollup.non-ic-dissem-roll-up",
                "nodis-exdis-banner-rollup",
            ),
            // Foreign-banner mismatch rows on the same walker. Per-row
            // IDs travel on emitted diagnostics for audit traceability;
            // the additional-emitted-ids list lets `.marque.toml`
            // configure
            // `capco:banner.classification.mismatch-vs-projected = "warn"`
            // / `capco:banner.fgi.marker-mismatch-vs-projected = "warn"`
            // even though the walker's `id()` is the SAR roll-up tuple.
            (
                "capco:banner.classification.mismatch-vs-projected",
                "banner-classification-mismatch",
            ),
            (
                "capco:banner.fgi.marker-mismatch-vs-projected",
                "banner-fgi-marker-mismatch",
            ),
        ]
    }
}

/// One catalog row per banner-roll-up category. Ordering of rows controls
/// only the order of emitted diagnostics for a single banner candidate; it
/// does not affect correctness.
pub(super) struct BannerCategoryRow {
    /// Rule ID emitted on diagnostics from this row. Distinct from the
    /// walker's own `RuleId`, which is bookkeeping only — the audit
    /// stream and the overlap-guard tiebreaker both key on the per-row
    /// ID.
    pub(super) rule_id: RuleId,
    /// Per-row default severity. The walker copies this onto each emitted
    /// `Diagnostic`; the engine's severity-override layer can downgrade
    /// or upgrade per the user's `.marque.toml`.
    pub(super) severity: Severity,
    /// Pure function returning the diagnostics this row produces for
    /// the given banner attributes and page projection. Implemented as a
    /// fn pointer so the catalog can be a `const`.
    ///
    /// Receives `&ProjectedMarking` (the engine-facing rolled-up shape).
    /// Banner-validation rules don't need per-portion membership — the
    /// union/intersection/max math is already performed by the
    /// projection at the engine boundary.
    pub(super) evaluate: fn(
        &CanonicalAttrs,
        &marque_ism::ProjectedMarking,
        &BannerCategoryRow,
    ) -> Vec<Diagnostic<CapcoScheme>>,
}

const BANNER_CATEGORY_CATALOG: &[BannerCategoryRow] = &[
    // SAR — §H.5 p101: "Unique SAPs contained in portion marks must
    // always appear in the banner line." Banner hierarchy depiction
    // (compartments / sub-compartments) is optional per §H.5 p101 +
    // p99; the walker matches by program identifier only. Severity
    // `Fix` because the with-block case has a deterministic zero-width
    // insertion fix; the no-block case escalates to `Error` inside the
    // evaluator (banner-positioning a new SAR block from rule context
    // alone is unsafe).
    BannerCategoryRow {
        rule_id: RuleId::new("capco", "banner.banner-rollup.sar-portions-roll-up"),
        severity: Severity::Fix,
        evaluate: evaluate_sar_banner_rollup,
    },
    // SCI — per-system "Precedence Rules for Banner Line Guidance" in
    // §H.4 (e.g. HCS p62, SI p74, TK p85; one of 18 identical
    // instances): "All unique SCI markings contained in the portion
    // marks must always appear in the banner line." Unlike SAR, §H.4
    // contains no hierarchy-optional carve-out, so compartments and
    // sub-compartments are also rolled up.
    BannerCategoryRow {
        rule_id: RuleId::new("capco", "banner.banner-rollup.sci-portions-roll-up"),
        severity: Severity::Error,
        evaluate: evaluate_sci_banner_rollup,
    },
    // Non-IC dissem — §H.9 p174 (NODIS) and §H.9 p172 (EXDIS): NODIS
    // takes priority over EXDIS, and either token, if present in any
    // portion, must roll up to the banner. Both passages are the
    // operative supersession-and-roll-up rule for this category.
    BannerCategoryRow {
        rule_id: RuleId::new("capco", "banner.banner-rollup.non-ic-dissem-roll-up"),
        severity: Severity::Error,
        evaluate: evaluate_non_ic_dissem_banner_rollup,
    },
    // Banner classification mismatch.
    //
    // Fires when the observed banner's classification disagrees with
    // the projected page-level classification (Us/Fgi/Nato/Joint/
    // Conflict variant or effective level). Severity `Error`, no fix:
    // cross-axis byte-positioning a missing or wrong classification
    // block from rule context alone is unsafe; deterministic fix
    // requires renderer-level coordination not yet wired. The
    // renderer produces canonical output via `fix`; `lint` surfaces
    // the mismatch only.
    //
    // Authority: CAPCO-2016 §H.7 pp123-125 (reciprocal classification
    // grammar — `(U) Precedence Rules for Banner Line Guidance` on
    // p124 covers the FGI / classification ladder roll-up; the
    // worked examples on pp126-129 anchor the cross-axis
    // composition).
    BannerCategoryRow {
        rule_id: RuleId::new("capco", "banner.classification.mismatch-vs-projected"),
        severity: Severity::Error,
        evaluate: evaluate_classification_banner_rollup,
    },
    // Banner FGI marker mismatch.
    //
    // Fires when the observed banner's FGI marker disagrees with the
    // projected page-level FGI marker (presence/absence; concealed vs
    // acknowledged variant). Severity `Error`, no fix — same
    // safety rationale as the classification mismatch row.
    //
    // Authority: CAPCO-2016 §H.7 p124 — *"Use FGI + Register, Annex
    // B trigraph country code(s) and/or Register Annex A tetragraph
    // code(s) in the banner line, unless the very fact that the
    // information is foreign government information must be
    // concealed."* Plus the source-concealed-dominates rule on the
    // same page: *"If any document contains portions of both
    // source-concealed FGI ... and source-acknowledged FGI, then
    // only the 'FGI' marking without the source trigraph(s)/
    // tetragraph(s) must appear in the banner line."* The §H.7 p127
    // worked example (`TOP SECRET//BOHEMIA//FGI AUS CAN DEU NATO//
    // NOFORN`) and §H.7 p129 worked example (`TOP SECRET//FGI CAN
    // DEU//NOFORN`) anchor the projection.
    BannerCategoryRow {
        rule_id: RuleId::new("capco", "banner.fgi.marker-mismatch-vs-projected"),
        severity: Severity::Error,
        evaluate: evaluate_fgi_marker_banner_rollup,
    },
];
