// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` category table. Lifted from the monolithic
//! `constraints.rs` per the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_scheme::{AggregationOp, Cardinality, Category, IntraOrdering};

use super::super::*;

/// Build the scheme's category table.
///
/// (U) The IC marking system has nine categories of classification and control markings:
/// 1. US Classification Markings
/// 2. Non-US Protective Markings
/// 3. Joint Classification Markings
/// 4. Sensitive Compartmented Information (SCI) Control System Markings – used by the IC to identify information that has special access requirements not met by classification level, alone
/// 5. Special Access Program (SAP) Markings – used primarily by non-IC departments and agencies to identify information that has special access requirements not met by classification level, alone
/// 6. Atomic Energy Act (AEA) Information Markings – used to identify information regarding nuclear matters
/// 7. Foreign Government Information (FGI) Markings – used to identify information from a foreign source
/// 8. Dissemination Control Marking – IC markings used to identify the expansion or limitation on distribution
/// 9. Non-Intelligence Community Dissemination Control Markings – non-IC markings used to identify the expansion or limitation on further distribution
pub(crate) fn build_categories() -> Vec<Category> {
    vec![
        // US classifications are a core category with a well-defined hierarchy, so `Max` is the natural aggregation.
        // NOTE: `Classification` includes 3 distinct categories that cannot co-occur in the same portion or banner:
        //  - U.S. classification level (e.g. CONFIDENTIAL, SECRET, TOP SECRET) or UNCLASSIFIED (if no classification)
        //  - Non-U.S. classification (e.g. //GBR SECRET, //CAN CONFIDENTIAL, //NATO UNCLASSIFIED etc.).  Non-U.S. classification may also be `RESTRICTED`, between UNCLASSIFIED and CONFIDENTIAL.
        //  - JOINT classification (e.g. //JOINT USA CAN SECRET, //JOINT USA DEU FRA CONFIDENTIAL, etc.) JOINT must always include a REL TO dissemination control that minimally includes the JOINT members (e.g. //JOINT USA CAN SECRET must have at least USA and CAN in REL TO) resulting in: `//JOINT USA CAN SECRET//REL TO USA, CAN` or as a portion `(//JOINT USA CAN S//REL TO USA, CAN)`
        //
        // **A marking can only include one of these three categories** -- they are mutually exclusive.
        //
        // In banner rollup (and rarely in portions), if any portion carries a U.S. classification, the non-U.S. JOINT members and non-U.S. origin countries are moved to the FGI category in the banner as a flat union (with a caveat, see FGI)
        //
        // A simple way to think about non-U.S. and JOINT classifications beginning with `//` is that it indicates the separation of the occluded U.S. classification category
        // It's the category separator that is still required to separate from the 'invisible' U.S. classification category that precedes it.
        Category {
            id: CAT_CLASSIFICATION,
            name: "classification",
            ordering_rank: 0,
            cardinality: Cardinality::One,
            aggregation: AggregationOp::Max,
            intra_ordering: IntraOrdering::AsWritten,
            expansion: None,
        },
        // Non-US classification
        // NATO information falls into this category but has its own tokens
        //   (e.g. //NATO COSMIC TOP SECRET, (//CTS), //NATO SECRET, (//NS), etc.)
        Category {
            id: CAT_NON_US_CLASSIFICATION,
            name: "non_us_classification",
            ordering_rank: 5,
            cardinality: Cardinality::One,
            aggregation: AggregationOp::Max,
            intra_ordering: IntraOrdering::AsWritten,
            expansion: None,
        },
        // JOINT classification connotes that each partner produced the information jointly and has a stake in its protection.
        Category {
            id: CAT_JOINT_CLASSIFICATION,
            name: "joint_classification",
            ordering_rank: 6,
            cardinality: Cardinality::One,
            aggregation: AggregationOp::Max,
            intra_ordering: IntraOrdering::AsWritten,
            expansion: None,
        },
        // SCI is plain union. It can be complicated by compartments
        // and subcompartments. There can be multiple of both compartments and subcompartments.
        // The relationships are hierarchical (i.e. SCI Control -> Compartment --> Subcompartment), and the rollup
        // preserves that hierarchy.
        // CAPCO names several Controls, some compartments and subcompartments. These are the most common ones,
        // but all three levels can have agency or program specific extensions that the scheme must support without requiring code changes.
        // There are some rules to these extensions:
        //  - Controls in their most-common abbreviated form are never more than 3 characters (e.g. HCS, SI, TK, etc.)
        Category {
            id: CAT_SCI,
            name: "sci",
            ordering_rank: 10,
            cardinality: Cardinality::Many,
            aggregation: AggregationOp::Union,
            intra_ordering: IntraOrdering::NumericThenAlpha,
            expansion: None,
        },
        Category {
            id: CAT_SAR,
            name: "sar",
            ordering_rank: 20,
            cardinality: Cardinality::Optional,
            // SAR rollup is structural (programs carry
            // compartments, compartments carry sub-compartments per
            // §H.5) and not expressible as a flat token union. Flag
            // as `Custom` so the engine routes through the lattice
            // constructor `SarSet::from_markings`
            // (`crates/capco/src/lattice.rs`) rather than
            // substituting a naive union reducer. Pre-PR-4b-E this
            // routed through the retired
            // `PageContext::expected_sar_marking` accessor.
            aggregation: AggregationOp::Custom,
            intra_ordering: IntraOrdering::NumericThenAlpha,
            expansion: None,
        },
        Category {
            id: CAT_AEA,
            name: "aea",
            ordering_rank: 30,
            cardinality: Cardinality::Many,
            // AEA rollup is not a plain union: RD precedes FRD and
            // TFNI (RD absorbs FRD when both are present), SIGMA
            // compartments merge numerically across RD blocks, and
            // UCNI drops in classified documents. Flag as `Custom`
            // so the engine routes through the lattice constructor
            // `AeaSet::from_markings` (`crates/capco/src/lattice.rs`)
            // rather than substituting a naive union reducer.
            // Pre-PR-4b-E this routed through the retired
            // `PageContext::expected_aea_markings` accessor.
            aggregation: AggregationOp::Custom,
            intra_ordering: IntraOrdering::AsWritten,
            expansion: None,
        },
        Category {
            id: CAT_FGI_MARKER,
            name: "fgi_marker",
            ordering_rank: 40,
            cardinality: Cardinality::Optional,
            // FGI rollup has non-trivial semantics: source-concealed
            // FGI supersedes source-acknowledged FGI (revealing the
            // country list would compromise the concealed source),
            // and the marker changes shape when multiple origin
            // countries contribute. `AggregationOp::Custom` flags
            // this so the engine routes through the lattice
            // constructor `FgiSet::from_attrs_iter`
            // (`crates/capco/src/lattice.rs`) rather than
            // substituting a plain union. Pre-PR-4b-E this routed
            // through the retired `PageContext::expected_fgi_marker`
            // accessor.
            //
            // When multiple source-acknowledged FGIs combine, they
            // are a space delimited union in alphabetical order.
            // When a JOINT marker is superseded by a U.S. classification
            // The non-U.S. JOINT members are moved to the FGI marker.
            //
            // NOTE: The FGI category indicates *origin* and says nothing
            // about *releasability*. FGI should still propagate with NOFORN
            // and some FGI *originates* as NOFORN. Meaning the country
            // requested the information *not* get shared back to them
            // (i.e. to another part of their government)
            aggregation: AggregationOp::Custom,
            intra_ordering: IntraOrdering::Alphabetical,
            expansion: None,
        },
        Category {
            id: CAT_DISSEM,
            name: "dissem",
            ordering_rank: 50,
            cardinality: Cardinality::Many,
            // Plain union at category granularity. NOFORN ⊐ REL TO
            // is a *cross*-category supersession — NOFORN lives in
            // dissem, REL TO in `rel_to` — and
            // `UnionWithSupersession` is only expressive within a
            // single category's token set. The cross-category
            // supersession is enforced today by the
            // `RelToBlock::from_attrs_iter` lattice constructor in
            // `crates/capco/src/lattice.rs` (which collapses to the
            // `NofornSuperseded` sentinel variant when any NOFORN is
            // present) and by the
            // `Constraint::Conflicts(NOFORN, REL_TO)` check below.
            // Pre-PR-4b-E the cross-category supersession was
            // enforced by the retired `PageContext::expected_rel_to`
            // accessor. Phase C will model cross-category supersession
            // explicitly (e.g. as a new `Constraint::Supersedes`
            // variant that spans categories).
            aggregation: AggregationOp::Union,
            intra_ordering: IntraOrdering::Alphabetical,
            expansion: None,
        },
        // NOTE: REL TO is not its own category; it's a dissemination control.
        // CanonicalAttrs models it as a separate field because it's a list of countries that must be compared as a set for supersession and conflict rules.
        // The list is comma delimited and may consist of country trigraphs or organizational/operational tetragraphs (e.g. FVEY, NATO).
        // USA **must** always be present and first, other entries are alphabetical.
        Category {
            id: CAT_REL_TO,
            name: "rel_to",
            ordering_rank: 60,
            cardinality: Cardinality::Many,
            aggregation: AggregationOp::Intersect,
            intra_ordering: IntraOrdering::FixedFirst {
                first: TOK_USA,
                rest: Box::new(IntraOrdering::Alphabetical),
            },
            // Phase A leaves the expansion table empty; Phase B
            // wires the FVEY/NATO/ACGU → {USA, GBR, ...} map in.
            expansion: None,
        },
        Category {
            id: CAT_DECLASSIFY_ON,
            name: "declassify_on",
            ordering_rank: 70,
            cardinality: Cardinality::Optional,
            aggregation: AggregationOp::MaxDate,
            intra_ordering: IntraOrdering::AsWritten,
            expansion: None,
        },
    ]
}
