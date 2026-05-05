// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `ProjectedMarking` — the output of `MarkingScheme::project(scope,
//! ...)`. Defined at PR 3a, **not wired** at PR 3a; PR 6 cuts over.
//!
//! At PR 3a the type exists so PR 5's `expected_classification` widening
//! and PR 6's `Scope::Page` cutover have a stable target. No engine call
//! site reads or writes `ProjectedMarking` here — `PageContext::expected_*`
//! continues to drive page roll-up.
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

    /// IC dissemination controls. Single field at PR 3a/PR 6; PR 9
    /// splits into `dissem_us` + `dissem_nato`.
    pub dissem_controls: Box<[DissemControl]>,

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
