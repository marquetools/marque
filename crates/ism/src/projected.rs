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
use crate::canonical::CanonicalAttrs;
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

    /// DISPLAY ONLY list (intersection-with-common-element across
    /// portions per CAPCO-2016 §D.2 Table 3 row 25; also row 26
    /// when a portion carries REL TO with overlap). Empty when no
    /// portion contributes DISPLAY ONLY. NOFORN-superseded (DISPLAY
    /// ONLY is mutually exclusive with NOFORN per §H.8 p163).
    pub display_only_to: Box<[CountryCode]>,

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

    /// Construct a `ProjectedMarking` (page scope) from a
    /// [`CanonicalAttrs`] value produced by the lattice path of
    /// `CapcoScheme::project(Scope::Page, ...)`.
    ///
    /// This is the type bridge installed at PR 4b-D for the hot-path
    /// flip: the engine drives `page_marking_arc` through
    /// `scheme.project(Scope::Page, ...) -> CapcoMarking`, then uses
    /// this constructor to project the resulting `CanonicalAttrs` into
    /// the engine-facing `ProjectedMarking` shape that banner-
    /// validation rules consume via `RuleContext::page_marking`.
    ///
    /// The bridge lives in `marque-ism` because `ProjectedMarking` is
    /// `#[non_exhaustive]` — its constructor MUST live in the type's
    /// home crate so cross-crate callers cannot bypass field-addition
    /// migrations (Constitution Principle VII).
    ///
    /// Production callers (Copilot R2 #12 — the previous doc claim
    /// that `CapcoScheme::project` calls this was wrong):
    ///
    /// - `marque_engine::project_page_marking` — the engine fast-path
    ///   helper that wraps `CapcoScheme::project_from_page_context`'s
    ///   `CanonicalAttrs` output into a `ProjectedMarking` for
    ///   `RuleContext::page_marking`.
    /// - `crates/engine/benches/profile_project.rs` — phase-attribution
    ///   benchmark.
    ///
    /// `CapcoScheme::project` itself returns a `CapcoMarking`
    /// (`CapcoScheme::Marking`), not a `ProjectedMarking` — the bridge
    /// is engine-side, not scheme-side.
    ///
    /// # Field mapping
    ///
    /// Every field on [`ProjectedMarking`] takes its value verbatim
    /// from the corresponding [`CanonicalAttrs`] field. `scope` is set
    /// to [`Scope::Page`] (this constructor is page-projection only;
    /// document- and diff-scoped projections will land their own
    /// constructors when those code paths come online). `provenance`
    /// is [`ProjectionProvenance::default()`] — per-portion span
    /// attribution lands when the projection pipeline grows a
    /// contribution-tracking layer (out of scope for PR 4b-D).
    ///
    /// CAB-only fields on `CanonicalAttrs` (`classified_by`,
    /// `derived_from`, `declass_exemption`, `token_spans`) are
    /// intentionally absent from `ProjectedMarking` per the type-level
    /// "page aggregate, not a CAB" contract.
    ///
    /// # Lifecycle
    ///
    /// PR 4b-D wires the engine to call this on the hot path. PR 4b-E
    /// retires [`crate::PageContext::project`] in favor of this
    /// constructor + the scheme's lattice path; until then, both
    /// constructors coexist and the parity gate at
    /// `crates/capco/tests/page_context_lattice_parity.rs` enforces
    /// agreement on the documented-divergence set.
    pub fn from_canonical(attrs: CanonicalAttrs) -> ProjectedMarking {
        ProjectedMarking {
            scope: Scope::Page,
            classification: attrs.classification,
            sci_controls: attrs.sci_controls,
            sci_markings: attrs.sci_markings,
            sar_markings: attrs.sar_markings,
            aea_markings: attrs.aea_markings,
            fgi_marker: attrs.fgi_marker,
            dissem_us: attrs.dissem_us,
            dissem_nato: attrs.dissem_nato,
            non_ic_dissem: attrs.non_ic_dissem,
            rel_to: attrs.rel_to,
            display_only_to: attrs.display_only_to,
            declassify_on: attrs.declassify_on,
            provenance: ProjectionProvenance::default(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::{
        Classification, CountryCode, DissemControl, FgiMarker, MarkingClassification,
        NatoClassification, NonIcDissem, SarIndicator, SarMarking, SarProgram, SciControl,
    };

    fn usa() -> CountryCode {
        CountryCode::try_new(b"USA").expect("trigraph")
    }

    fn gbr() -> CountryCode {
        CountryCode::try_new(b"GBR").expect("trigraph")
    }

    #[test]
    fn from_canonical_empty_attrs_round_trip() {
        let attrs = CanonicalAttrs::default();
        let p = ProjectedMarking::from_canonical(attrs);
        assert_eq!(p.scope, Scope::Page);
        assert!(p.classification.is_none());
        assert!(p.sci_controls.is_empty());
        assert!(p.sci_markings.is_empty());
        assert!(p.sar_markings.is_none());
        assert!(p.aea_markings.is_empty());
        assert!(p.fgi_marker.is_none());
        assert!(p.dissem_us.is_empty());
        assert!(p.dissem_nato.is_empty());
        assert!(p.non_ic_dissem.is_empty());
        assert!(p.rel_to.is_empty());
        assert!(p.display_only_to.is_empty());
        assert!(p.declassify_on.is_none());
        assert_eq!(p.provenance, ProjectionProvenance::default());
    }

    #[test]
    fn from_canonical_preserves_us_classification() {
        let attrs = CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert!(matches!(
            p.classification,
            Some(MarkingClassification::Us(Classification::Secret))
        ));
    }

    #[test]
    fn from_canonical_preserves_nato_classification() {
        // §H.7 pp123-125: pure-NATO pages keep the Nato variant on
        // the lattice path. The bridge MUST NOT collapse it.
        let attrs = CanonicalAttrs {
            classification: Some(MarkingClassification::Nato(NatoClassification::NatoSecret)),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert!(matches!(
            p.classification,
            Some(MarkingClassification::Nato(NatoClassification::NatoSecret))
        ));
    }

    #[test]
    fn from_canonical_preserves_joint_classification() {
        // §H.3 p56: pure-JOINT pages preserve the Joint variant. The
        // bridge MUST NOT flatten to Us(_).
        let joint = crate::attrs::JointClassification {
            level: Classification::Secret,
            countries: Box::new([usa(), gbr()]),
        };
        let attrs = CanonicalAttrs {
            classification: Some(MarkingClassification::Joint(joint)),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        match p.classification {
            Some(MarkingClassification::Joint(j)) => {
                assert_eq!(j.level, Classification::Secret);
                assert_eq!(j.countries.len(), 2);
            }
            other => panic!("expected Joint, got {other:?}"),
        }
    }

    #[test]
    fn from_canonical_preserves_sar_marking() {
        let program = SarProgram::new("EXP", Box::new([]));
        let sar = SarMarking::new(SarIndicator::Abbrev, Box::new([program]));
        let attrs = CanonicalAttrs {
            sar_markings: Some(sar.clone()),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert_eq!(p.sar_markings, Some(sar));
    }

    #[test]
    fn from_canonical_preserves_fgi_marker() {
        // FGI marker — explicit foreign-source-acknowledged page.
        let attrs = CanonicalAttrs {
            fgi_marker: FgiMarker::acknowledged([gbr()]),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert!(matches!(p.fgi_marker, Some(FgiMarker::Acknowledged { .. })));
    }

    #[test]
    fn from_canonical_preserves_dissem_axes_and_rel_to() {
        let attrs = CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_us: vec![DissemControl::Nf].into_boxed_slice(),
            dissem_nato: vec![DissemControl::Oc].into_boxed_slice(),
            non_ic_dissem: vec![NonIcDissem::Nodis].into_boxed_slice(),
            rel_to: vec![usa(), gbr()].into_boxed_slice(),
            sci_controls: vec![SciControl::Si].into_boxed_slice(),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert_eq!(p.dissem_us.as_ref(), &[DissemControl::Nf]);
        assert_eq!(p.dissem_nato.as_ref(), &[DissemControl::Oc]);
        assert_eq!(p.non_ic_dissem.as_ref(), &[NonIcDissem::Nodis]);
        assert_eq!(p.rel_to.as_ref(), &[usa(), gbr()]);
        assert_eq!(p.sci_controls.as_ref(), &[SciControl::Si]);
    }

    #[test]
    fn from_canonical_preserves_declassify_on() {
        use crate::date::IsmDate;
        let date = IsmDate::Date(2030, 1, 1);
        let attrs = CanonicalAttrs {
            declassify_on: Some(date.clone()),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert_eq!(p.declassify_on, Some(date));
    }

    // PR 4b-D.2 Copilot R1 #9 — preserve `display_only_to` across the
    // bridge. The pre-fix test surface covered `rel_to` but not the
    // parallel `display_only_to` country-list axis, leaving a
    // type-renaming regression undetectable.

    #[test]
    fn from_canonical_preserves_display_only_to_single_country() {
        let attrs = CanonicalAttrs {
            display_only_to: vec![gbr()].into_boxed_slice(),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert_eq!(p.display_only_to.as_ref(), &[gbr()]);
    }

    #[test]
    fn from_canonical_preserves_display_only_to_multiple_countries() {
        let attrs = CanonicalAttrs {
            display_only_to: vec![usa(), gbr()].into_boxed_slice(),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert_eq!(p.display_only_to.as_ref(), &[usa(), gbr()]);
    }

    #[test]
    fn from_canonical_preserves_display_only_to_empty() {
        let attrs = CanonicalAttrs::default();
        let p = ProjectedMarking::from_canonical(attrs);
        assert!(p.display_only_to.is_empty());
    }

    #[test]
    fn from_canonical_drops_cab_only_fields_silently() {
        // CAB fields on CanonicalAttrs are not part of ProjectedMarking
        // (page aggregate, not a CAB). The bridge MUST compile and run
        // without referencing them; we exercise the path by populating
        // them and asserting the projection's surface is the same as
        // an attrs with those fields cleared.
        let attrs_with_cab = CanonicalAttrs {
            classified_by: Some("classifier-id".to_string().into_boxed_str()),
            derived_from: Some("source-doc".to_string().into_boxed_str()),
            ..CanonicalAttrs::default()
        };
        let attrs_without_cab = CanonicalAttrs::default();

        let p_with = ProjectedMarking::from_canonical(attrs_with_cab);
        let p_without = ProjectedMarking::from_canonical(attrs_without_cab);
        assert_eq!(p_with, p_without);
    }
}
