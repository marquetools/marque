// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `ProjectedMarking` — the engine-facing output of
//! `MarkingScheme::project(Scope::Page, ...)`.
//!
//! `ProjectedMarking` is the production page-roll-up shape that
//! banner/CAB rules consume via `RuleContext::page_marking`. The engine
//! drives the projection through
//! `CapcoScheme::project_from_attrs_slice` +
//! `ProjectedMarking::from_canonical`. The per-axis classification
//! rollup is `Option<MarkingClassification>` so a pure-foreign page
//! produces `None` rather than a hardcoded US default.
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
/// `classification: Option<MarkingClassification>` preserves foreign
/// provenance; `fgi_marker` survives the projection alongside
/// classification rather than being collapsed into it.
///
/// `ProjectedMarking` is the production page-roll-up shape banner/CAB
/// rules consume via `RuleContext::page_marking`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedMarking {
    /// Which scope produced this projection. The engine reads this to
    /// dispatch banner-validation vs. document-level rules.
    pub scope: Scope,

    /// Aggregated classification. `None` when no portion contributed a
    /// US classification — pure-foreign pages produce this case.
    pub classification: Option<MarkingClassification>,

    /// SCI controls (CVE projection of `sci_markings`).
    pub sci_controls: Box<[SciControl]>,

    /// Structural SCI markings (compartments + sub-compartments).
    pub sci_markings: Box<[SciMarking]>,

    /// SAR block, at most one per banner per §A.6. Field name aligns
    /// with `CanonicalAttrs::sar_markings` (plural form) so the
    /// projection wiring does not need name-mapping glue.
    pub sar_markings: Option<SarMarking>,

    /// AEA markings.
    pub aea_markings: Box<[AeaMarking]>,

    /// FGI marker. Survives projection so banner roll-up retains
    /// foreign provenance (FR-008, #261).
    pub fgi_marker: Option<FgiMarker>,

    /// US-attributed IC dissemination controls in the page rollup. See
    /// [`crate::CanonicalAttrs::dissem_us`] for the CAPCO-2016 p41
    /// reciprocity rule. Page roll-up unions each namespace
    /// independently via the per-axis lattice helpers
    /// `DissemSet::from_attrs_iter` / `NatoDissemSet::from_attrs_iter`
    /// in `marque-capco::lattice` (this leaf crate cannot intra-doc-link
    /// into the downstream domain crate; the names are stable and
    /// greppable).
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
    /// This is the type bridge for the hot path: the engine drives
    /// `page_marking_arc` through
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
    /// Production callers:
    ///
    /// - `marque_engine::project_page_marking` — the engine fast-path
    ///   helper that wraps `CapcoScheme::project_from_attrs_slice`'s
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
    /// attribution would require the projection pipeline to grow a
    /// contribution-tracking layer.
    ///
    /// CAB-only fields on `CanonicalAttrs` (`classified_by`,
    /// `derived_from`, `declass_exemption`, `token_spans`) are
    /// intentionally absent from `ProjectedMarking` per the type-level
    /// "page aggregate, not a CAB" contract.
    ///
    /// # Parity
    ///
    /// The engine calls this on the hot path alongside the scheme's
    /// lattice path. The parity gate at
    /// `crates/capco/tests/lattice_vs_scheme_parity.rs` enforces
    /// agreement on the documented-divergence set across the two
    /// surfaces (`project_via_lattice` and `project_via_scheme`).
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

    /// Returns `true` iff the page-aggregate classification is
    /// `Some(MarkingClassification::Nato(_))` and no portion contributes
    /// Foreign Government Information.
    ///
    /// This is the `ProjectedMarking`-side predicate consumed by
    /// `marque-capco`'s bare-NATO-requires-`REL TO USA, NATO` rule
    /// to silence the bare-NATO → `REL TO USA, NATO` suggestion on
    /// documents that are wholly NATO-owned. It reads the post-lattice
    /// page aggregate
    /// (`self.classification`, `self.fgi_marker`) instead of looping
    /// over per-portion attrs. The post-lattice semantic is the
    /// forward direction; the divergence does not affect current
    /// S007 dispatch because portion rules do not yet receive a
    /// populated `page_marking` (see fr048 trip-wire test); revisit
    /// when the engine plumbs page state to portion-rule dispatch.
    ///
    /// # Predicate truth conditions
    ///
    /// - **Pure-NATO page** (e.g., 3× `(//NS)`): `classification` is
    ///   `Some(MarkingClassification::Nato(_))`, `fgi_marker` is `None`
    ///   → `true`.
    /// - **Mixed US + NATO portions**: `classification` is `Some(Us(_))`
    ///   (the lattice's banner-roll-up of US + NATO in a US-authored
    ///   document), `fgi_marker` is `None` → `false`.
    /// - **Mixed JOINT + NATO portions**: `classification` is
    ///   `Some(Joint(_))` (or higher US after JOINT derivative use) →
    ///   `false`.
    /// - **NATO + FGI portions**: `fgi_marker` is `Some(_)` → `false`.
    /// - **Empty page**: `classification` is `None` → `false`.
    ///
    /// # Authority
    ///
    /// The solely-NATO carve-out is derived **by extension** from
    /// CAPCO-2016 §H.7 p127's Notional Example 2
    /// (`(//CTS//BOHEMIA//REL TO USA, NATO)`), which establishes
    /// `REL TO USA, NATO` as the canonical form for NATO content in
    /// US-authored documents — the marker is unnecessary when the
    /// entire document is NATO-owned (alliance ownership is implicit).
    /// The S007 doc-block (`crates/capco/src/rules.rs` —
    /// `BareNatoRequiresRelToRule`) frames this same extension.
    pub fn is_solely_nato_classified(&self) -> bool {
        matches!(
            self.classification,
            Some(crate::attrs::MarkingClassification::Nato(_))
        ) && self.fgi_marker.is_none()
    }
}

/// Lattice trace + per-portion contribution record for a
/// [`ProjectedMarking`].
///
/// # Why a struct, not a typedef
///
/// Banner rules need both the source-portion spans (for diagnostic
/// pointers) and a lattice-operation summary ("which join produced this
/// SCI compartment set?"). A struct keeps room for both fields without
/// a later breaking change.
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

    // `is_solely_nato_classified` predicate. These tests exercise the
    // four invariants the doc-comment relies on; the S007 callsite in
    // `crates/capco/src/rules.rs` is the load-bearing consumer.

    #[test]
    fn is_solely_nato_classified_true_on_pure_nato() {
        // classification = Some(Nato(_)), fgi_marker = None
        let attrs = CanonicalAttrs {
            classification: Some(MarkingClassification::Nato(NatoClassification::NatoSecret)),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert!(p.is_solely_nato_classified());
    }

    #[test]
    fn is_solely_nato_classified_false_on_us_classification() {
        // Mixed US+NATO reciprocal-raises to Us(_); predicate must be false.
        let attrs = CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert!(!p.is_solely_nato_classified());
    }

    #[test]
    fn is_solely_nato_classified_false_when_fgi_present() {
        // FgiMarker::acknowledged enforces non-empty list — see
        // attrs.rs §H.7 p123.
        let fgi = FgiMarker::acknowledged([gbr()]).expect("non-empty list");
        let attrs = CanonicalAttrs {
            classification: Some(MarkingClassification::Nato(NatoClassification::NatoSecret)),
            fgi_marker: Some(fgi),
            ..CanonicalAttrs::default()
        };
        let p = ProjectedMarking::from_canonical(attrs);
        assert!(!p.is_solely_nato_classified());
    }

    #[test]
    fn is_solely_nato_classified_false_when_classification_absent() {
        let attrs = CanonicalAttrs::default();
        let p = ProjectedMarking::from_canonical(attrs);
        assert!(!p.is_solely_nato_classified());
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
