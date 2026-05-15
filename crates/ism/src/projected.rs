// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `ProjectedMarking` — the **intended** post-PR-6 engine-facing output
//! of `MarkingScheme::project(Scope::Page, ...)` once the `Scope::Page`
//! projection cutover lands. Defined at PR 3a, **not wired** at PR 3a;
//! PR 6 wires it.
//!
//! Today's `MarkingScheme::project` (in `marque-scheme`) returns
//! `Self::Marking` (scheme-specific). PR 6's cutover changes the
//! engine-facing call path so banner-validation rules consume
//! `&ProjectedMarking` instead of reaching through `PageContext`. PR 5
//! widens `expected_classification` to `Option<MarkingClassification>`
//! ahead of that wiring (FR-007). This type is defined at PR 3a so
//! both PRs have a stable target without a separate type-system
//! change.
//!
//! At PR 3a no engine call site reads or writes `ProjectedMarking` —
//! `PageContext::expected_*` continues to drive page roll-up. The type
//! is `pub` and its `dead_code` is suppressed only when the workspace
//! lints flag it (Risk #6 in the PR 3a design doc).
//!
//! # Field shape
//!
//! Mirrors `CanonicalAttrs` for the fields that participate in page
//! roll-up plus a `scope` discriminant and a `provenance` trace.
//! Fields not relevant to projection (`classified_by`, `derived_from`,
//! `declass_exemption`, `token_spans`) are absent — a projected marking
//! is a banner / page aggregate, not a CAB.

use crate::attrs::{
    AeaMarking, CountryCode, DissemControl, FgiMarker, MarkingClassification, NonIcDissem,
    SarMarking, SciControl, SciMarking,
};
use crate::date::IsmDate;
use crate::span::Span;
use marque_scheme::Scope;

/// Output of a `MarkingScheme::project(scope, ...)` call.
///
/// PR 3a defines the shape; PR 6 wires the engine to consume it.
/// Banner-validation rules migrate to `&ProjectedMarking` at PR 9.
///
/// **FR-007 + FR-008**: `classification: Option<MarkingClassification>`
/// preserves foreign provenance; `fgi_marker` survives the projection
/// alongside classification rather than being collapsed into it.
///
/// # PR-3a scope note
///
/// `ProjectedMarking` is defined but not constructed at PR 3a —
/// `PageContext::expected_*` continues to drive page roll-up. The type
/// is `pub` so `dead_code` does not fire across the workspace; per the
/// design's Risk #6 a targeted `#[allow(dead_code)]` is reserved should
/// the workspace lints flag it. PR 6 turns `ProjectedMarking` into a
/// real consumer.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedMarking {
    /// Which scope produced this projection. The engine reads this to
    /// dispatch banner-validation vs. document-level rules.
    pub scope: Scope,

    /// Aggregated classification. `None` when no portion contributed a
    /// US classification — pure-foreign pages produce this case
    /// post-PR-5.
    pub classification: Option<MarkingClassification>,

    /// SCI controls (CVE projection of `sci_markings`).
    pub sci_controls: Box<[SciControl]>,

    /// Structural SCI markings (compartments + sub-compartments).
    pub sci_markings: Box<[SciMarking]>,

    /// SAR block, at most one per banner per §A.6. Field name aligns
    /// with `CanonicalAttrs::sar_markings` (plural form preserved from
    /// the pre-PR-3a `IsmAttributes` shape) so PR 6's projection
    /// wiring does not need name-mapping glue.
    pub sar_markings: Option<SarMarking>,

    /// AEA markings.
    pub aea_markings: Box<[AeaMarking]>,

    /// FGI marker. Survives projection so banner roll-up retains
    /// foreign provenance (FR-008, #261).
    pub fgi_marker: Option<FgiMarker>,

    /// US-attributed IC dissemination controls in the page rollup.
    /// PR 9b (FR-046 / T132) split the prior single
    /// `dissem_controls` field; see
    /// [`crate::CanonicalAttrs::dissem_us`] for the CAPCO-2016 p41
    /// reciprocity rule. Page roll-up unions each namespace
    /// independently (see [`PageContext::expected_dissem_us`] /
    /// [`PageContext::expected_dissem_nato`]).
    ///
    /// [`PageContext::expected_dissem_us`]: crate::PageContext::expected_dissem_us
    /// [`PageContext::expected_dissem_nato`]: crate::PageContext::expected_dissem_nato
    pub dissem_us: Box<[DissemControl]>,

    /// NATO-attributed IC dissemination controls in the page rollup.
    /// Populated only when at least one portion contributed dissem
    /// tokens under [`crate::MarkingClassification::Nato`].
    pub dissem_nato: Box<[DissemControl]>,

    /// Non-IC dissemination controls.
    pub non_ic_dissem: Box<[NonIcDissem]>,

    /// REL TO list (intersection across portions, NOFORN-superseded).
    pub rel_to: Box<[CountryCode]>,

    /// Most-conservative declassification date (max-end across
    /// portions).
    pub declassify_on: Option<IsmDate>,

    /// Trace of which portions and lattice operations contributed.
    /// Used by banner-validation rules (E035 SCI roll-up, E031 SAR
    /// roll-up, etc.) to point a diagnostic at the offending
    /// per-portion span.
    pub provenance: ProjectionProvenance,
}

impl ProjectedMarking {
    /// Iterate every IC dissem control across both namespace fields
    /// ([`Self::dissem_us`] then [`Self::dissem_nato`]).
    ///
    /// Mirrors [`crate::CanonicalAttrs::dissem_iter`]. Use when the
    /// consumer cares about "any dissem regardless of namespace"; read
    /// the underlying fields directly for namespace-aware logic.
    pub fn dissem_iter(&self) -> impl Iterator<Item = &DissemControl> + Clone {
        self.dissem_us.iter().chain(self.dissem_nato.iter())
    }
}

/// Lattice trace + per-portion contribution record for a
/// [`ProjectedMarking`].
///
/// Defined at PR 3a as an empty-default placeholder. PR 6 fills in the
/// fields consumed by banner-validation rules. The shape is reserved so
/// PR 6 doesn't require a separate type-system change.
///
/// # Why a struct, not a typedef
///
/// Banner rules need both the source-portion spans (for diagnostic
/// pointers) and a lattice-operation summary ("which join produced this
/// SCI compartment set?"). Splitting them into a struct now avoids a
/// later breaking-change PR.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectionProvenance {
    /// Source-portion spans that contributed to this projection.
    /// Used by E035 (SCI banner roll-up) to point diagnostics at the
    /// offending portion when the banner is missing a compartment.
    pub contributing_portion_spans: Box<[Span]>,
}
