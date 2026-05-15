// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CanonicalAttrs` — the owned, post-canonical marking representation
//! that rules consume.
//!
//! Constructed from `ParsedAttrs<'_>` exactly two ways:
//!
//! 1. **PR 3a transitional**: [`from_parsed_unchecked`] — a `pub
//!    #[doc(hidden)]` adapter that performs the structural
//!    rename without applying any canonicalization rules. PR 3a's
//!    invariant is byte-identical behavior on every fixture; the
//!    adapter exists to thread the new types through the engine
//!    without churning rule semantics. PR 3c deletes this function.
//!
//! 2. **Post-PR-3c canonical path**: `MarkingScheme::canonicalize`,
//!    the only authorized public route. A scheme decides what
//!    canonicalization means (case folding, deprecated-token
//!    migration, etc.) and rule crates do not own the choice.
//!
//! # Why owned
//!
//! Rules need attrs that outlive the source byte buffer (e.g.,
//! `PageContext` accumulates per-portion attrs across the whole page
//! before banner-validation rules consume the aggregate; the source
//! buffer of an early portion may have been freed by then). Having
//! `CanonicalAttrs` own its data simplifies the lifetimes that flow
//! through the engine without forcing every rule signature to carry
//! an `'src` parameter.
//!
//! # Field shape
//!
//! Mirrors `IsmAttributes` exactly at PR 3a — same field names, same
//! types, same semantics. Subsequent PRs reshape:
//!
//! - **PR 9b (FR-046, T132)** split the prior single `dissem_controls`
//!   field into `dissem_us` and `dissem_nato`. The attribution is
//!   performed by [`crate::dissem_attribution::attribute_dissems`] on
//!   the `ParsedAttrs` side; [`from_parsed_unchecked`] is a pure
//!   structural rename and does not re-run attribution.
//! - **PR 3c** may migrate `sci_controls` (the CVE projection) to a
//!   `SciSet`-only shape if no rule reads `sci_controls` post-collapse
//!   (CLAUDE.md "compatibility view scheduled for removal").
//! - **PR 2 (FR-017)** introduces `FgiMarker::SourceConcealed |
//!   Acknowledged`. PR 3a uses the existing flat `FgiMarker`.
//!
//! Holding the existing field shape at PR 3a is what keeps the change
//! byte-identical and independently revertable.

use crate::attrs::{
    AeaMarking, Classification, CountryCode, DeclassExemption, DissemControl, FgiMarker,
    MarkingClassification, NonIcDissem, SarMarking, SciControl, SciMarking, TokenSpan,
};
use crate::date::IsmDate;
use crate::parsed::ParsedAttrs;

/// Owned, canonical-form attributes. The pivot type rules consume.
///
/// # Block ordering (CAPCO)
///
/// Field order mirrors CAPCO block sequence: classification → SCI →
/// SAR → AEA → FGI → IC dissem → non-IC dissem → REL TO → CAB. This
/// is documentation-only; rules dispatch on field name, not order.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalAttrs {
    /// US/FGI/NATO/JOINT classification, or `None` when the parser
    /// found no classification. **FR-007**: must remain `Option<_>` —
    /// the `MarkingClassification::Us` hardcode at
    /// `crates/capco/src/scheme.rs:365` is PR 5's deletion target,
    /// not PR 3a's.
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
    /// (plural) form to keep PR 3a a structural rename only. PR 3c can
    /// rename to singular when shape narrowing happens.
    pub sar_markings: Option<SarMarking>,

    /// AEA markings (RD/FRD/CNWDI/SIGMA/UCNI/TFNI) per §H.6.
    pub aea_markings: Box<[AeaMarking]>,

    /// FGI marker in a US-classified marking. Flat shape at PR 3a;
    /// PR 2 introduces the `SourceConcealed | Acknowledged`
    /// discriminant (FR-017).
    pub fgi_marker: Option<FgiMarker>,

    /// US-attributed IC dissemination controls. See
    /// [`crate::ParsedAttrs::dissem_us`] for the CAPCO-2016 p41
    /// reciprocity rule that drives attribution (PR 9b / FR-046 /
    /// T132). When both fields could apply, US wins;
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

/// Transitional adapter — converts `ParsedAttrs<'_>` into
/// [`CanonicalAttrs`] by structural rename only.
///
/// **`#[doc(hidden)] pub`** because the data-model.md spec (and FR-043)
/// require it to be cross-crate-callable but visibly project-internal.
/// The `_unchecked` suffix follows the Rust-stdlib convention: a path
/// that *exists* but is not the public-API path you should reach for.
///
/// # PR-3c lifecycle
///
/// This function deletes at PR 3c, when `MarkingScheme::canonicalize`
/// becomes the sole `ParsedAttrs → CanonicalAttrs` constructor (FR-043).
/// FR-040's `_unchecked`-shape signature lint (R-11 in `research.md`)
/// flags any function matching `fn(...ParsedAttrs<'_>...) ->
/// CanonicalAttrs` outside `MarkingScheme::canonicalize`; the adapter
/// here is whitelisted via path-based carve-out
/// (`crates/ism/src/canonical.rs::from_parsed_unchecked`) for the
/// duration of the keystone window. The carve-out auto-removes when 3c
/// lands and the function is deleted.
///
/// # Semantics
///
/// **Byte-identical to PR-3a-pre behavior.** Every field is moved
/// across without transformation — no case folding, no deprecated-token
/// migration, no canonicalization. The function name's `_unchecked`
/// suffix names this exact gap: a real `canonicalize` impl would do
/// more work; this adapter does none.
///
/// # Why it isn't `From<ParsedAttrs<'_>> for CanonicalAttrs`
///
/// FR-040's lint targets `fn(...ParsedAttrs<'_>...) -> CanonicalAttrs`
/// signatures regardless of name. Implementing `From` would generate a
/// lint-flagging `fn from(_: ParsedAttrs<'_>) -> Self` synthesized
/// signature; whitelisting it would dilute the lint. A free function
/// with a deliberately-unwieldy name is the right shape for "yes,
/// this exists; no, you should not reach for it casually."
#[doc(hidden)]
pub fn from_parsed_unchecked(parsed: ParsedAttrs<'_>) -> CanonicalAttrs {
    let ParsedAttrs {
        classification,
        sci_markings,
        sci_controls,
        sar_markings,
        aea_markings,
        fgi_marker,
        dissem_us,
        dissem_nato,
        non_ic_dissem,
        rel_to,
        declassify_on,
        classified_by,
        derived_from,
        declass_exemption,
        token_spans,
        source_bytes_origin: _, // discarded; not on CanonicalAttrs
    } = parsed;

    let out = CanonicalAttrs {
        classification: classification.map(|c| c.value),
        sci_controls,
        sci_markings: Vec::from(sci_markings)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        sar_markings: sar_markings.map(|p| p.value),
        aea_markings: Vec::from(aea_markings)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        fgi_marker: fgi_marker.map(|p| p.value),
        // PR 9b (T132): preserve the parser-side attribution. The
        // attribution function lives on the `ParsedAttrs` side; this
        // adapter is a pure structural rename and must not re-run it.
        dissem_us: Vec::from(dissem_us)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        dissem_nato: Vec::from(dissem_nato)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        non_ic_dissem: Vec::from(non_ic_dissem)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        rel_to: Vec::from(rel_to)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        declassify_on: declassify_on.map(|p| p.value),
        classified_by: classified_by.map(Box::<str>::from),
        derived_from: derived_from.map(Box::<str>::from),
        declass_exemption,
        token_spans,
    };

    // PR 9b (T132) invariant insurance. `attribute_dissems` is the
    // single source of truth; this debug-only assertion catches a
    // future bug where attribution is skipped or the canonical
    // adapter is fed a hand-built `ParsedAttrs` with both fields
    // populated.
    #[cfg(debug_assertions)]
    {
        debug_assert!(
            out.dissem_nato.is_empty() || out.us_classification().is_none(),
            "dissem_nato populated alongside US classification — \
             attribute_dissems was skipped or bypassed. CAPCO-2016 p41 \
             reciprocity rule violated."
        );
    }

    out
}
