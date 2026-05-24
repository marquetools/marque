// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CanonicalAttrs` — the owned, post-canonical marking representation
//! that rules consume.
//!
//! Constructed from `ParsedAttrs<'_>` via `MarkingScheme::canonicalize`
//! — the sole authorized public route. A scheme decides
//! what canonicalization means (case folding, deprecated-token
//! migration, etc.) and rule crates do not own the choice. The
//! CAPCO/ISM implementation lives in
//! `marque_capco::CapcoScheme::canonicalize`.
//!
//! The promote-callsite-lint enforces the sole-path invariant at
//! signature shape: any other function shaped
//! `fn(ParsedAttrs<'_>) -> CanonicalAttrs` outside the trait method
//! is a CI error.
//!
//! # Why owned
//!
//! Rules need attrs that outlive the source byte buffer (e.g., the
//! engine's per-page accumulator collects per-portion attrs across
//! the whole page before banner-validation rules consume the
//! aggregate; the source buffer of an early portion may have been
//! freed by then). Having `CanonicalAttrs` own its data simplifies
//! the lifetimes that flow through the engine without forcing every
//! rule signature to carry an `'src` parameter.
//!
//! # Field shape
//!
//! - IC dissem is split into `dissem_us` and `dissem_nato`. The
//!   attribution is performed by
//!   [`crate::dissem_attribution::attribute_dissems`] on the
//!   `ParsedAttrs` side; `MarkingScheme::canonicalize` is a pure
//!   structural rename and does not re-run attribution.
//! - `sci_controls` (the CVE projection) is a compatibility view
//!   scheduled for removal once no rule reads it.
//! - `fgi_marker` carries the `FgiMarker::SourceConcealed |
//!   Acknowledged` discriminant.

use crate::attrs::{
    AeaMarking, Classification, CountryCode, DeclassExemption, DissemControl, FgiMarker,
    MarkingClassification, NonIcDissem, SarMarking, SciControl, SciMarking, TokenSpan,
};
use crate::date::IsmDate;

/// Owned, canonical-form attributes. The pivot type rules consume.
///
/// # Block ordering (CAPCO)
///
/// Field order mirrors CAPCO block sequence: classification → SCI →
/// SAR → AEA → FGI → IC dissem → non-IC dissem → REL TO → CAB. This
/// is documentation-only; rules dispatch on field name, not order.
///
/// **Exhaustive**: the struct intentionally exposes every field for
/// brace construction outside `marque-ism`. The structural rename body
/// lives in `CapcoScheme::canonicalize` and in four `marque-core` test
/// helpers — both sets need to construct `CanonicalAttrs` literally.
/// `MarkingScheme::canonicalize` remains the sole production
/// `ParsedAttrs → CanonicalAttrs` constructor; the promote-callsite-lint
/// flags any other signature shape outside that trait method.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalAttrs {
    /// US/FGI/NATO/JOINT classification, or `None` when the parser
    /// found no classification. Must remain `Option<_>` — a missing
    /// classification is a representable state, not a hardcoded default.
    pub classification: Option<MarkingClassification>,

    /// SCI controls (CVE projection). Compatibility view per CLAUDE.md;
    /// new rules SHOULD read `sci_markings` instead.
    pub sci_controls: Box<[SciControl]>,

    /// Structural SCI markings — authoritative for compartments and
    /// sub-compartments per CAPCO §A.6.
    pub sci_markings: Box<[SciMarking]>,

    /// SAR block, at most one per marking per §A.6. Cardinality is
    /// `Option`, not `Box<[]>`.
    ///
    /// Field name preserves the existing `IsmAttributes::sar_markings`
    /// (plural) form for a purely structural rename.
    pub sar_markings: Option<SarMarking>,

    /// AEA markings (RD/FRD/CNWDI/SIGMA/UCNI/TFNI) per §H.6.
    pub aea_markings: Box<[AeaMarking]>,

    /// FGI marker in a US-classified marking, carrying the
    /// `SourceConcealed | Acknowledged` discriminant.
    pub fgi_marker: Option<FgiMarker>,

    /// US-attributed IC dissemination controls. See
    /// [`crate::ParsedAttrs::dissem_us`] for the CAPCO-2016 p41
    /// reciprocity rule that drives attribution. When both fields
    /// could apply, US wins;
    /// [`Self::dissem_nato`] populates only when the marking has no
    /// US classification axis.
    pub dissem_us: Box<[DissemControl]>,

    /// NATO-attributed IC dissemination controls. Populated only when
    /// [`Self::classification`] is
    /// [`MarkingClassification::Nato`](crate::MarkingClassification::Nato)
    /// — see [`crate::ParsedAttrs::dissem_nato`].
    pub dissem_nato: Box<[DissemControl]>,

    /// Non-IC dissemination controls (CAPCO §H.9).
    pub non_ic_dissem: Box<[NonIcDissem]>,

    /// REL TO country / country-group codes.
    pub rel_to: Box<[CountryCode]>,

    /// DISPLAY ONLY country / country-group codes per CAPCO-2016
    /// §H.8 p163. Parallel to [`Self::rel_to`] but a *disclosure*
    /// decision (foreign recipient may view without retaining a
    /// copy) rather than a *release* decision (recipient may retain).
    /// USA is NOT required in this list — release to US recipients
    /// is implicit; the list names only the foreign audience that
    /// may view.
    pub display_only_to: Box<[CountryCode]>,

    /// Declassification date from CAB (typed precision tier).
    pub declassify_on: Option<IsmDate>,

    /// Free-text "Classified By" identifier from CAB.
    pub classified_by: Option<Box<str>>,

    /// Free-text "Derived From" source from CAB.
    pub derived_from: Option<Box<str>>,

    /// Declassification exemption code from CAB.
    pub declass_exemption: Option<DeclassExemption>,

    /// Per-token byte spans into the original source buffer. Reused
    /// verbatim from `IsmAttributes::token_spans`. Used by rules that
    /// need byte-precise diagnostic spans (E001, E002, E003, ...).
    pub token_spans: Box<[TokenSpan]>,
}

impl CanonicalAttrs {
    /// Convenience accessor: returns the US classification level if
    /// this marking uses the US or Conflict classification system.
    /// Pure-FGI / NATO / JOINT markings return `None`.
    ///
    /// Mirrors the prior `IsmAttributes::us_classification` exactly so
    /// existing rule call sites compile unchanged after the type rename.
    pub fn us_classification(&self) -> Option<Classification> {
        match self.classification {
            Some(MarkingClassification::Us(c)) => Some(c),
            Some(MarkingClassification::Conflict { us, .. }) => Some(us),
            _ => None,
        }
    }

    /// Iterate every IC dissem control across both namespace fields
    /// ([`Self::dissem_us`] then [`Self::dissem_nato`]).
    ///
    /// Use this when the consumer cares about "any IC dissem regardless
    /// of namespace" (e.g., the renderer, the
    /// `is_nontrivial_marking` decoder check, the
    /// `expected_dissem_*` rollup feed). When the consumer cares
    /// specifically about US-attributed or NATO-attributed dissems
    /// (e.g., a future cross-system translator, an
    /// audit-provenance trace), read the underlying fields directly.
    ///
    /// The returned iterator is `Clone` so multi-pass consumers do not
    /// need to re-construct it.
    pub fn dissem_iter(&self) -> impl Iterator<Item = &DissemControl> + Clone {
        self.dissem_us.iter().chain(self.dissem_nato.iter())
    }
}
