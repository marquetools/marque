// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` constraint catalog + `build_categories`/`build_constraints`
//! free constructors. Lifted from the monolithic `scheme.rs` per the issue
//! #466 split plan (`claudedocs/refactor-466/split_proposal.md`, Risk 1
//! Option 2).
//!
//! See [`build_constraints`] for the catalog and per-row authority.

use marque_scheme::{
    AggregationOp, Cardinality, Category, Constraint, IntraOrdering, TokenRef,
};

use super::*;
use super::actions::emit_companion_required;
use super::predicates::{
    class_floor_anchor_span, rel_to_covers,
};

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
            // as `Custom` so Phase B leaves
            // `PageContext::expected_sar_marking` in place rather
            // than substituting a naive union reducer.
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
            // so Phase B does not silently replace
            // `PageContext::expected_aea_markings` with a naive
            // union reducer.
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
            // this for Phase B so the engine does not silently
            // replace `PageContext::expected_fgi_marker` with a
            // plain union.
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
            // supersession is enforced today by
            // `PageContext::expected_rel_to()` (which clears REL TO
            // when any NOFORN is present) and by the
            // `Constraint::Conflicts(NOFORN, REL_TO)` check below.
            // Phase C will model cross-category supersession
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
pub(crate) fn build_constraints() -> Vec<Constraint> {
    // The CAPCO declarative constraint catalog. Every entry's
    // `label` cites a verified passage in
    // `crates/capco/docs/CAPCO-2016.md`; non-normative sections
    // (§I-K — history, examples, acronym list) are NOT valid
    // citation targets. See Constitution VIII and the project
    // memory entry "CAPCO doc structure".
    //
    // T035 (2026-04-21) wired runtime evaluation through this
    // catalog: dyadic variants dispatch via the generic evaluator
    // (`crate::constraint::evaluate`) using
    // [`Self::satisfies`]; `Custom` variants dispatch through
    // [`Self::evaluate_custom`] to scheme-private predicate
    // helpers below. The hand-written `Rule` impls in
    // `crate::rules` that previously enforced these invariants
    // are retired in the same PR; `crate::rules_declarative`
    // hosts thin wrappers that call `scheme.validate()` and
    // construct `Diagnostic` values with byte-identical
    // message/span/fix output.
    //
    // T035b audit (2026-04-21): E017, E018, and E019 were
    // retired as over-restrictive relative to CAPCO-2016 §H.3
    // pp 56–57:
    //
    // - §H.3 p57 lists "FGI, IC and Non-IC dissemination
    //   control markings (excluding NOFORN)" among markings
    //   JOINT "may be used with, as appropriate"
    // - §H.3 p57 names only two explicit exclusions:
    //   HCS markings and NOFORN markings
    // - §H.3 p57 cross-references §H.7 for FGI content marker
    //   syntax on JOINT documents — FGI marker presence is a
    //   content indicator, not a competing classification type
    //
    // The JOINT+NOFORN exclusion is caught indirectly: E014
    // requires JOINT to carry REL TO, and
    // `capco/noforn-conflicts-rel-to` fires when NOFORN and REL
    // TO co-occur. The JOINT+HCS exclusion has no such indirect
    // coverage, so it gets its own catalog entry below as E036.
    vec![
        // ---- E010: HCS subsystem rules (CAPCO-2016 §H.4) -----
        //
        // Bare HCS is legacy; HCS-O requires ORCON; HCS-P
        // requires ORCON or ORCON-USGOV; HCS-O/P require S or
        // TS. The full sub-rule set lives in
        // `hcs_system_constraints` because the predicate is
        // n-ary and emits multiple violations per offending
        // marking (one per failing sub-rule).
        Constraint::Custom {
            name: "E010/HCS-system-constraints",
            label: "CAPCO-2016 §H.4 pp 62-66",
        },
        // ---- E012: dual classification (CAPCO-2016 §H.3 p55) -
        //
        // §H.3 p55: "The US, non-US, and JOINT
        // classification markings are mutually exclusive – a
        // banner line or portion mark may contain only one type
        // and value for the classification marking."
        //
        // Custom (not Conflicts) because the predicate inspects
        // a single field — `MarkingClassification::Conflict {
        // us, foreign }` — that the parser populates when it
        // encounters two systems in one marking.
        Constraint::Custom {
            name: "E012/dual-classification",
            label: "CAPCO-2016 §H.3 p55",
        },
        // ---- E014: JOINT requires REL TO coverage (§H.3 p57) -
        //
        // §H.3 p57 (Relationship(s) to Other Markings): "Requires
        // REL TO USA, LIST". Every JOINT participant MUST also
        // appear in the marking's REL TO list. Custom (not
        // Requires) because the check is iterative across all
        // JOINT countries.
        Constraint::Custom {
            name: "E014/joint-requires-rel-to-coverage",
            label: "CAPCO-2016 §H.3 p57",
        },
        // ---- E015: non-US requires dissem (§H.7 + §B.3) ------
        //
        // FGI markings require explicit foreign release per
        // §H.7 pp 122–123 (FGI marking template + sharing-
        // agreement basis) and §B.3 p20 paragraph d (FD&R
        // markings on FGI in IC DAPs); JOINT requires REL TO
        // per §H.3 p57. The simplified dyadic predicate
        // "non-US classification + empty dissem" captures the
        // common-case violation. The narrower per-system
        // requirements (FGI-specific, JOINT-specific) are
        // separately enforced by E014 and by the existing
        // hand-written rules.
        Constraint::Requires {
            name: "E015/non-us-requires-dissem",
            left: TokenRef::AnyInCategory(CAT_NON_US_CLASSIFICATION),
            right: TokenRef::AnyInCategory(CAT_DISSEM),
            label: "CAPCO-2016 §H.7 p122 + §B.3 p20",
        },
        // ---- E016: JOINT conflicts RESTRICTED (§H.3 p56) -----
        //
        // §H.3 p56 (Relationship(s) to Other Markings): "May not
        // be used with RESTRICTED. (Note: the US is always a
        // JOINT marking owner/producer; and RESTRICTED is not an
        // authorized US classification marking.)"
        Constraint::Conflicts {
            name: "E016/joint-conflicts-restricted",
            left: TokenRef::Token(TOK_JOINT),
            right: TokenRef::Token(TOK_RESTRICTED),
            label: "CAPCO-2016 §H.3 p56",
        },
        // ---- E036: JOINT conflicts HCS markings (§H.3 p57) ---
        //
        // §H.3 p57 (Relationship(s) to Other Markings): "May not
        // be used with the HCS markings or NOFORN markings."
        // Same page reinforces: JOINT may use "SCI (excluding HCS
        // markings), SAP, AEA, FGI, IC and Non-IC dissemination
        // control markings (excluding NOFORN)".
        //
        // The JOINT-NOFORN exclusion is already caught indirectly
        // by `capco/noforn-conflicts-rel-to` + E014's REL TO
        // requirement (NOFORN in a JOINT document either conflicts
        // with the required REL TO or leaves REL TO empty). The
        // HCS exclusion has no such indirect coverage, so it
        // gets its own catalog entry.
        //
        // Supersedes the retired E017/E018/E019 which over-
        // restricted JOINT against FGI content markers, arbitrary
        // IC dissem, and non-IC dissem respectively. Those rules
        // forbade combinations §H.3 p57 explicitly permits.
        // See T035b retirement commit and project memory
        // `feedback_audit_predicates_against_source.md`.
        Constraint::Conflicts {
            name: "E036/joint-conflicts-hcs",
            left: TokenRef::Token(TOK_JOINT),
            right: TokenRef::Token(TOK_HCS),
            label: "CAPCO-2016 §H.3 p57",
        },
        // ---- E021: AEA requires NOFORN (§H.6 p104) -----------
        //
        // §H.6 RD entry p104: "Is always used with NOFORN
        // unless a sharing agreement has been established per
        // the Atomic Energy Act. (Ref. Sections 123 and 144 of
        // the Atomic Energy Act, and DoD Instruction 5030.14.)".
        // The "always used with NOFORN" requirement applies to
        // RD, FRD (§H.6 p111), and TFNI (§H.6 p120) — not UCNI
        // (DOD UCNI §H.6 p116, DOE UCNI §H.6 p118 carry no such
        // requirement) and not to any future AEA entry added to
        // the category.
        // Custom (not `Requires { left: AnyInCategory(CAT_AEA) }`)
        // because that dyadic shape would sweep UCNI in: a valid
        // `U//UCNI` marking would incorrectly require NOFORN.
        Constraint::Custom {
            name: "E021/aea-requires-noforn",
            label: "CAPCO-2016 §H.6 p104",
        },
        // ---- §H.6 p106 CNWDI subset-of-RD: enforced by data
        //      model, NO Constraint row needed -----------------
        //
        // CNWDI is structurally a `bool` field on
        // `AeaMarking::Rd(RdBlock { cnwdi })` in `marque-ism`'s
        // type system. There is no `AeaMarking::Cnwdi` variant.
        // A portion that bears CNWDI necessarily bears RD
        // because CNWDI presence is gated by the surrounding
        // `Rd(...)` variant.
        //
        // The `TOK_CNWDI` sentinel is satisfied only by
        // `AeaMarking::Rd(rd) if rd.cnwdi` (see `satisfies_attrs`
        // earlier in this file); `TOK_RD` is satisfied by any
        // `AeaMarking::Rd(_)`. The two are not independently
        // settable — `TOK_CNWDI` strictly implies `TOK_RD` at
        // the predicate level. An earlier draft of PR 4b-A
        // added a `Constraint::Requires { TOK_CNWDI, TOK_RD }`
        // row to enforce the §H.6 p106 "subset of RD" rule, but
        // Copilot review caught that the row is unreachable —
        // it can never fire because the right-hand side is
        // necessarily true whenever the left-hand side is true.
        //
        // The §H.6 p106 invariant therefore lives at the data-
        // model level rather than the constraint-catalog level.
        // See `docs/plans/2026-05-01-lattice-design.md` §7.5
        // "Cross-axis constraints" for the §-cited record of
        // this decision.
        //
        // If a future change to `AeaMarking` ever splits CNWDI
        // into a sibling variant (decoupling it from `Rd`), the
        // §H.6 p106 enforcement MUST be re-introduced as a
        // Constraint::Requires or equivalent — and the
        // satisfies_attrs predicate for `TOK_CNWDI` MUST be
        // amended to no longer match through the `Rd(...)`
        // variant.
        // ---- E022 retired in PR 3b.D (T026d) -----------------
        //
        // The CNWDI classification floor moved into the class-
        // floor catalog block below as
        // `E058/CNWDI-classification-floor`. The legacy
        // `E022/CNWDI-classification-floor` entry that previously
        // lived here is removed because (a) the catalog walker
        // emits the diagnostic via `E058/...`, and (b) keeping the
        // `E022/...` entry alongside the `E058/...` entry produced
        // a dead duplicate constraint row that never fires (the
        // dispatch in `evaluate_custom_by_attrs` no longer routes
        // to a predicate for it). Per
        // `feedback_pre_users_no_deprecation_phasing.md`, no
        // alias map is preserved.

        // ---- E024: RD precedence (§H.6 p104) -----------------
        //
        // §H.6 RD entry p104: "If RD, FRD, and TFNI
        // portions are in a document, the RD takes precedence
        // and is conveyed in the banner line." Custom (not
        // Supersedes) because Supersedes is a banner-rollup
        // hint that doesn't fire diagnostics; the per-portion
        // commingling violation is what E024 reports. The
        // banner-rollup Supersedes entries are intentionally
        // deferred until Phase E wires them through
        // `project(Scope::Page, ...)`.
        Constraint::Custom {
            name: "E024/rd-precedence",
            label: "CAPCO-2016 §H.6 p104",
        },
        // ---- E025 retired in PR 3b.D (T026d) -----------------
        //
        // The UCNI ceiling invariant moved into the class-floor
        // catalog block below as TWO rows
        // (`E058/DOD-UCNI-classification-ceiling` at §H.6 p116 and
        // `E058/DOE-UCNI-classification-ceiling` at §H.6 p118),
        // split per PM decision #1 so each variant carries its
        // own §H.6 sub-page citation. The legacy
        // `E025/ucni-conflicts-classification` aggregated entry
        // that previously lived here is removed for the same
        // reason as the E022 entry above (the dispatch in
        // `evaluate_custom_by_attrs` no longer routes to a
        // predicate for it).

        // ---- W002: US + FGI commingling (§H.7 p124) ----------
        //
        // §H.7 p124: documents not marked per ICD 206
        // "must segregate the FGI from US portions." Custom (not
        // Conflicts) because the rule is portion-only — the
        // wrapper filters by `RuleContext::marking_type` after
        // the predicate fires.
        Constraint::Custom {
            name: "W002/us-commingled-with-fgi",
            label: "CAPCO-2016 §H.7 p124",
        },
        // ---- capco/noforn-conflicts-rel-to (§H.8 p145) -------
        //
        // §H.8 NOFORN entry p145: "Cannot be used with
        // REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY." This is
        // the portion-level exclusion; the page-rewrite that
        // clears REL TO when NOFORN is present at page scope is
        // declared separately in `build_page_rewrites`.
        Constraint::Conflicts {
            name: "capco/noforn-conflicts-rel-to",
            left: TokenRef::Token(TOK_NOFORN),
            right: TokenRef::AnyInCategory(CAT_REL_TO),
            label: "CAPCO-2016 §H.8 p145",
        },
        // ---- capco/joint-requires-usa (§H.3 p55) -------------
        //
        // §H.3 p55: "USA is always included in the
        // JOINT marking [LIST], as USA is always a
        // co-owner/producer." Plus REL TO must include USA per
        // §H.3 p57 (REL TO USA, LIST requirement). Custom (not Requires) because USA
        // must appear in BOTH `joint.countries` AND `rel_to` —
        // a coupled predicate that doesn't decompose cleanly
        // into a single TokenRef pair.
        Constraint::Custom {
            name: "capco/joint-requires-usa",
            label: "CAPCO-2016 §H.3 p55",
        },
        // ---- E037: NODIS ⊥ EXDIS (§H.9 p172 + p174) ----------
        //
        // §H.9 EXDIS entry (p172) and NODIS entry (p174) both
        // state the same mutual-exclusion invariant: NODIS and
        // EXDIS MUST NOT coexist on the same information ("EXDIS
        // and NODIS markings cannot be used together" / "NODIS
        // and EXDIS markings cannot be used together"). A portion
        // (or banner) carrying both is malformed.
        //
        // Modeled as a dyadic `Conflicts` constraint — the
        // symmetric shape fits built-in Conflicts exactly, no
        // cross-category coupling, no level comparison.
        Constraint::Conflicts {
            name: "E037/nodis-conflicts-exdis",
            left: TokenRef::Token(TOK_NODIS),
            right: TokenRef::Token(TOK_EXDIS),
            label: "CAPCO-2016 §H.9 p172 + p174",
        },
        // ---- E038: NODIS / EXDIS require NOFORN (§H.9) -------
        //
        // §H.9 EXDIS entry (p172) and NODIS entry (p174) both
        // state "Requires NOFORN" in their Relationship(s) to
        // Other Markings. A marking carrying NODIS or EXDIS
        // without NOFORN is a violation of both template entries.
        //
        // Custom (not two separate `Requires` constraints)
        // because the rule emits a SINGLE diagnostic ID — E038 —
        // and the dispatch layer in `rules_declarative.rs`
        // works by filtering violations by constraint `name`.
        // Splitting into two `Requires` constraints would create
        // two distinct violation names for one rule ID and force
        // the wrapper to OR them. Folding the disjunction into a
        // single Custom predicate keeps the wrapper trivial.
        Constraint::Custom {
            name: "E038/nodis-or-exdis-requires-noforn",
            label: "CAPCO-2016 §H.9 p172 + p174",
        },
        // ---- E054: RELIDO ⊥ NOFORN (§H.8 p154) ------------------
        //
        // §H.8 RELIDO entry p154, Relationship(s) to Other Markings:
        // "Cannot be used with NOFORN or DISPLAY ONLY."
        //
        // PR 3.7 update: this row STAYS as an enumerated `Conflicts`
        // (reverted from Stage D's compaction). The wrapper layer
        // (`rules_declarative.rs::E054RelidoConflictsNoforn`) dispatches
        // by name through `violations_for(attrs, "E054/...")`; without
        // an enumerated row here, the wrapper silently emits no
        // diagnostics. PR 4 (T112) will rebuild the wrapper dispatch
        // to be family-aware and then retire this row. Per plan rev 1
        // §0 "Non-scope (deferred to PR 4): RELIDO Conflicts compaction".
        Constraint::Conflicts {
            name: "E054/relido-conflicts-noforn",
            left: TokenRef::Token(TOK_RELIDO),
            right: TokenRef::Token(TOK_NOFORN),
            label: "CAPCO-2016 §H.8 p154",
        },
        // ---- E055: RELIDO ⊥ DISPLAY ONLY (§H.8 p154) ------------
        //
        // §H.8 RELIDO entry p154, same Relationship(s) prose.
        Constraint::Conflicts {
            name: "E055/relido-conflicts-display-only",
            left: TokenRef::Token(TOK_RELIDO),
            right: TokenRef::Token(TOK_DISPLAY_ONLY),
            label: "CAPCO-2016 §H.8 p154",
        },
        // ---- E056: ORCON ⊥ RELIDO (§H.8 p136) -------------------
        //
        // §H.8 ORCON entry p136: "May not be used with RELIDO."
        Constraint::Conflicts {
            name: "E056/orcon-conflicts-relido",
            left: TokenRef::Token(TOK_ORCON),
            right: TokenRef::Token(TOK_RELIDO),
            label: "CAPCO-2016 §H.8 p136",
        },
        // ---- E057: ORCON-USGOV ⊥ RELIDO (§H.8 p140) -------------
        //
        // §H.8 ORCON-USGOV entry p140: same exclusion as ORCON.
        Constraint::Conflicts {
            name: "E057/orcon-usgov-conflicts-relido",
            left: TokenRef::Token(TOK_ORCON_USGOV),
            right: TokenRef::Token(TOK_RELIDO),
            label: "CAPCO-2016 §H.8 p140",
        },
        // NOTE — ConflictsWithFamily primitive showcase removed in PR 3.7 rev 3.
        //
        // An earlier rev added two additive `ConflictsWithFamily` rows
        // (`capco/relido-conflicts-fdr-family` and
        // `capco/orcon-family-conflicts-relido`) alongside the
        // enumerated E054/E055/E056/E057 rows above as a "primitive
        // showcase". Copilot PR 3.7 review pass 3 surfaced that this
        // shape causes `CapcoScheme::validate()` to emit DOUBLE
        // diagnostics for any input that triggers both the enumerated
        // row and the family row (the same matching pair appears once
        // per row). The primitive is already exercised on a stub scheme
        // by `crates/scheme/tests/proptest_constraint_rhs_family_distributive.rs`;
        // the CAPCO catalog does not need active family-row entries to
        // validate the primitive. PR 4 (T112) lands the actual
        // compaction (delete E054-E057 enumerated rows AND add the
        // family rows AND rewire `rules_declarative.rs` wrappers to
        // dispatch by family-row name) as one coordinated change.
        // ================================================================
        // PR 3b.D (T026d) — class-floor catalog (§3.4.6)
        // ================================================================
        //
        // Per-marking classification floors per `marque-applied.md`
        // §3.4.6: presence of marking M requires the page's
        // classification level to be at least F(M). This is *not* part
        // of the lattice axis itself (the class chain is
        // `OrdMax(TS > CTS > S > NS > C > NC > R > NR > U > NU)`); it
        // is a *constraint* over the joint fact-set: the page is
        // malformed if M is present and the class level is below F(M).
        //
        // # Why Constraint::Custom (architectural choice — Option A)
        //
        // Class-floor RHS is "classification level ≥ F(M)" — a
        // partial-order threshold over the OrdMax classification
        // chain, not a token-presence assertion. The existing
        // `Constraint::Requires` shape is dyadic token-presence; the
        // class-floor predicate doesn't fit. PR 3.7 (T108b) may
        // revisit and re-classify to a primitive form
        // (e.g., `TokenRef::ClassAtLeast(ClassLevel)` or
        // `Constraint::ClassFloor`) once that primitive lands in
        // marque-scheme. See
        // `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md`
        // §3 for the architectural rationale.
        //
        // # Why family granularity (~26 rows, not ~38)
        //
        // The §3.4.6 author wrote at family granularity (HCS-[comp][sub],
        // SI-[comp], TK, RD-SG, etc. — pattern-matching family rows,
        // not enumerated per-template rows). Family granularity is
        // deliberate: clean lattice algebra, stable ImplTable shape
        // that survives PR 3.7's closure-operator landing without
        // re-shaping, uniform §-citation discipline. Family-pattern
        // matching is implemented in the predicate body
        // (`class_floor_catalog_eval`) — each predicate iterates the
        // relevant axis (`attrs.sci_markings`, `attrs.aea_markings`,
        // etc.) looking for any token matching the family.
        //
        // # Per-row name and walker rule-ID
        //
        // The single walker `DeclarativeClassFloorRule` (rule ID
        // `E058`) emits all diagnostics. Each catalog row's `name`
        // takes one of two forms:
        //
        //   - `E058/<purpose>` for rows that REPLACE a retired
        //     legacy rule. Specifically:
        //     `E058/CNWDI-classification-floor` (replaces retired
        //     E022), `E058/SAR-classification-floor` (replaces
        //     retired E027), `E058/DOD-UCNI-classification-ceiling`
        //     and `E058/DOE-UCNI-classification-ceiling` (replace
        //     retired E025; split per PM decision so each carries
        //     its own §H.6 sub-page citation).
        //   - `class-floor/<marking>` for rows with no retired-rule
        //     predecessor (e.g., `class-floor/HCS-comp-sub`,
        //     `class-floor/SI-comp`, `class-floor/BALK`,
        //     `class-floor/passthrough-BUR`).
        //
        // Per-row identification flows via the catalog's `name`
        // field into `ConstraintViolation.constraint_label` and is
        // referenced in `Diagnostic.message` for human-readable
        // identification.
        //
        // Severity-config compatibility for the legacy IDs (E022,
        // E025, E027) is intentionally NOT preserved. Per project
        // memory `feedback_pre_users_no_deprecation_phasing.md`:
        // marque is pre-users, so we don't carry alias maps,
        // retained namespaces, or phased deprecation.
        // `.marque.toml` files keying class-floor severity
        // overrides MUST use `E058` (walker-level) — there's no
        // per-row severity-override surface in PR D.
        //
        // # Citation methodology
        //
        // Each row's `label` carries the §3.4.6 author's chosen
        // citation. Some rows cite operative-authority pages
        // (precedence rules, FD&R-supersession anchors, AEA-chain
        // references) rather than the marking-template-body page; the
        // §3.4.6 author's choice is authoritative per
        // `marque-applied.md` line 783-808. The marking-body floor
        // language is verifiable in the H.x section body of each
        // marking; see the planning doc §2 for the verification
        // matrix.
        //
        // ---- §2.1 Floor TS — single classification level (5 rows) -
        Constraint::Custom {
            name: "class-floor/HCS-comp-sub",
            label: "CAPCO-2016 §H.4",
        },
        Constraint::Custom {
            name: "class-floor/SI-comp",
            label: "CAPCO-2016 §H.4",
        },
        Constraint::Custom {
            name: "class-floor/TK-BLFH",
            label: "CAPCO-2016 §H.4",
        },
        // PR 9c.1 T134: citation tightened from "§H.7 Appendix B"
        // to "§G.2 p40". §G.2 p40 is the authoritative anchor —
        // CAPCO-2016 Table 5 (ARH by Registered Marking) lists
        // BALK / BOHEMIA at p40 as registered NATO control
        // markings; the December 2010 history note at §H.7 line
        // 4702 confirms they are control markings (not
        // classifications). The §H.7 Appendix B reference was an
        // imprecise pre-PR-9c.1 anchor; the manual's actual
        // Appendix B is the NATO classification ladder
        // appendix, not the BALK/BOHEMIA registration.
        Constraint::Custom {
            name: "class-floor/BALK",
            label: "CAPCO-2016 §G.2 p40",
        },
        Constraint::Custom {
            name: "class-floor/BOHEMIA",
            label: "CAPCO-2016 §G.2 p40",
        },
        // ---- §2.2 Floor S — TS-or-S allowed (8 rows) --------------
        Constraint::Custom {
            name: "class-floor/HCS-comp",
            label: "CAPCO-2016 §H.4",
        },
        Constraint::Custom {
            name: "class-floor/RSV-comp",
            label: "CAPCO-2016 §H.4",
        },
        Constraint::Custom {
            name: "class-floor/TK",
            label: "CAPCO-2016 §H.4",
        },
        Constraint::Custom {
            name: "class-floor/RD-SG",
            label: "CAPCO-2016 §H.6 p113",
        },
        Constraint::Custom {
            name: "class-floor/FRD-SG",
            label: "CAPCO-2016 §H.6 p113",
        },
        // CNWDI — replaces retired E022. Per PM directive #5 + the
        // PR 3b.D planning doc §5.2, catalog row names use the
        // walker-prefixed form `E058/<suffix>`. Per
        // `feedback_pre_users_no_deprecation_phasing.md` (marque is
        // pre-users), severity-config back-compat for the retiring
        // E022 rule ID is not preserved — users keying `.marque.toml`
        // at `E022` will need to migrate to `E058`.
        Constraint::Custom {
            name: "E058/CNWDI-classification-floor",
            label: "CAPCO-2016 §H.6 p104",
        },
        Constraint::Custom {
            name: "class-floor/RSEN",
            label: "CAPCO-2016 §H.8 p149",
        },
        Constraint::Custom {
            name: "class-floor/IMCON",
            label: "CAPCO-2016 §H.8 p144",
        },
        // ---- §2.3 Floor C — any classified level (8 rows) --------
        Constraint::Custom {
            name: "class-floor/SI",
            label: "CAPCO-2016 §H.4",
        },
        // SAR — replaces retired E027. Walker-prefixed name per PM
        // directive #5.
        Constraint::Custom {
            name: "E058/SAR-classification-floor",
            label: "CAPCO-2016 §H.5",
        },
        Constraint::Custom {
            name: "class-floor/RD",
            label: "CAPCO-2016 §H.6 p104",
        },
        Constraint::Custom {
            name: "class-floor/FRD",
            label: "CAPCO-2016 §H.6 p104",
        },
        Constraint::Custom {
            name: "class-floor/TFNI",
            label: "CAPCO-2016 §H.6 p107",
        },
        // PR 9c.1 T134: citation tightened from "§H.7 Appendix B"
        // to "§H.7 p122". §H.7 p122 is the worked example showing
        // ATOMAL in the AEA axis: `SECRET//RD/ATOMAL//FGI NATO//
        // NOFORN` — the direct, structurally-grounded citation for
        // the canonical AEA-axis placement (paralleling §H.6's
        // RD/CNWDI worked-example citations).
        Constraint::Custom {
            name: "class-floor/ATOMAL",
            label: "CAPCO-2016 §H.7 p122",
        },
        Constraint::Custom {
            name: "class-floor/ORCON",
            label: "CAPCO-2016 §H.8 p136",
        },
        Constraint::Custom {
            name: "class-floor/EYES-ONLY",
            label: "CAPCO-2016 §H.8 p152",
        },
        // ---- §2.4 Floor =U — UNCLASSIFIED-only (2 rows; UCNI split) -
        //
        // Replaces retired `DeclarativeUcniClassificationRule` (E025).
        // Split per PM decision into two rows (DOD UCNI and DOE UCNI)
        // so each row carries its own §H.6 sub-page citation. Both
        // use the walker-prefixed name `E058/<suffix>`.
        Constraint::Custom {
            name: "E058/DOD-UCNI-classification-ceiling",
            label: "CAPCO-2016 §H.6 p116",
        },
        Constraint::Custom {
            name: "E058/DOE-UCNI-classification-ceiling",
            label: "CAPCO-2016 §H.6 p118",
        },
        // ---- §2.6 Unknown-floor passthrough (4 rows) -------------
        //
        // Per `marque-applied.md` §3.4.6 unknown-floor sub-catalog +
        // §3.7 passthrough policy. Provisional `F(M) = C` (minimal
        // classified). Severity Warn (per §3.4.6 Q-3.4.6b) — fired by
        // the walker at the per-row severity stored in the catalog
        // table.
        Constraint::Custom {
            name: "class-floor/passthrough-BUR",
            label: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
        },
        Constraint::Custom {
            name: "class-floor/passthrough-HCS-X",
            label: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
        },
        Constraint::Custom {
            name: "class-floor/passthrough-KLM",
            label: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
        },
        Constraint::Custom {
            name: "class-floor/passthrough-MVL",
            label: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
        },
        // ================================================================
        // PR 3b.E (T026e) — SCI per-system catalog (§H.4)
        // ================================================================
        //
        // Per-SCI-system companion-required / forbid-companion
        // invariants per CAPCO-2016 §H.4. Five rows at family
        // granularity covering the §H.4 invariants that PR 3b.D's
        // class-floor catalog does NOT already cover (companion-
        // required: ORCON, NOFORN; forbid-companion: ORCON-USGOV).
        // The class-floor portions of the retired E044/E045/E046/
        // E048/E049/E050 rules are absorbed by PR 3b.D's class-floor
        // rows and are not duplicated here.
        //
        // # Why Constraint::Custom (architectural choice)
        //
        // The §H.4 invariants are companion-presence (ORCON, NOFORN)
        // + companion-forbid (ORCON-USGOV) + per-row fix-shape
        // (zero-width insertion at the end of the IC dissem block,
        // or a span replacement on the dominated token) — none of
        // which fit the existing primitive surface. PR 4 (per-
        // category Lattice impls per Stage 3 of plan.md:263) MAY
        // revisit and re-classify to a `CompanionRequired<Set>` /
        // `Forbid<Set>` primitive on `marque-scheme` when those
        // primitives land. The walker stays until that retirement.
        // See `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`
        // §3 for the rule-by-rule analysis; tasks.md T026e for the
        // walker landing.
        //
        // # Per-row name and walker rule-ID
        //
        // The single walker `DeclarativeSciPerSystemRule` (rule ID
        // `E059`) emits all diagnostics. Each catalog row's `name`
        // takes the `sci-per-system/<purpose>` form. Per project
        // memory `feedback_pre_users_no_deprecation_phasing.md`
        // (marque is pre-users), severity-config back-compat for
        // the retiring E042–E051 rule IDs is not preserved — users
        // keying `.marque.toml` at any of `E042`..`E051` must
        // migrate to `E059`.
        Constraint::Custom {
            name: "sci-per-system/HCS-O-companions",
            label: "CAPCO-2016 §H.4 p64",
        },
        Constraint::Custom {
            name: "sci-per-system/HCS-P-NOFORN",
            label: "CAPCO-2016 §H.4 p66",
        },
        Constraint::Custom {
            name: "sci-per-system/HCS-P-sub-companions",
            label: "CAPCO-2016 §H.4 p68",
        },
        Constraint::Custom {
            name: "sci-per-system/SI-G-companions",
            label: "CAPCO-2016 §H.4 p80",
        },
        Constraint::Custom {
            name: "sci-per-system/TK-compartment-NOFORN",
            label: "CAPCO-2016 §H.4 p87 + p91 + p95",
        },
    ]
}

// ---------------------------------------------------------------------------
// T035 Custom-constraint helpers
// ---------------------------------------------------------------------------
//
// Each helper is the predicate body for a `Constraint::Custom` entry in
// `build_constraints`. The helpers do NOT reference `RuleContext` — only
// `CanonicalAttrs`. Per-context filtering (e.g., W002 portion-only) lives in
// the wrapper layer (`crate::rules_declarative`); the catalog represents
// "this marking is structurally inconsistent" without regard to where the
// marking appears.
//
// The returned `ConstraintViolation` populates `message` with text that the
// wrapper inspects when constructing the user-facing `Diagnostic`. The
// `constraint_label` and `citation` fields are overwritten by the caller
// (`marque_scheme::constraint::evaluate`'s `Custom` arm) so any placeholder
// values are fine — using the catalog name + label keeps the helpers
// self-documenting in isolation.

/// E012 — `MarkingClassification::Conflict` indicates the parser saw a US
/// classification AND a foreign classification in the same marking. CAPCO
/// §H.3 p55 forbids this ("The US, non-US, and JOINT classification
/// markings are mutually exclusive").
pub(crate) fn e012_dual_classification(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    if let Some(marque_ism::MarkingClassification::Conflict { us, foreign }) = &attrs.classification
    {
        let foreign_desc = match foreign.as_ref() {
            marque_ism::ForeignClassification::Nato(n) => format!("NATO ({})", n.banner_str()),
            marque_ism::ForeignClassification::Fgi(f) => {
                let countries: Vec<&str> = f.countries.iter().map(|c| c.as_str()).collect();
                if countries.is_empty() {
                    "FGI".to_owned()
                } else {
                    format!("FGI {}", countries.join(" "))
                }
            }
            marque_ism::ForeignClassification::Joint(j) => {
                let countries: Vec<&str> = j.countries.iter().map(|c| c.as_str()).collect();
                format!("JOINT {}", countries.join(" "))
            }
        };
        vec![ConstraintViolation {
            constraint_label: "E012/dual-classification",
            // The wrapper rebuilds the user-visible message from attrs;
            // the message here exists for catalog-level inspection and
            // tests. We surface `us` + `foreign_desc` so a test can
            // confirm both systems were observed.
            message: format!(
                "marking has both US ({}) and foreign ({}) classification",
                us.banner_str(),
                foreign_desc
            ),
            citation: "CAPCO-2016 §H.3 p55",
            span: None,
            severity: None,
        }]
    } else {
        Vec::new()
    }
}

/// E014 — every JOINT participant must appear in the marking's REL TO list.
/// CAPCO §H.3 p57 ("Requires REL TO USA, LIST" relationship statement).
/// Tetragraphs in REL TO expand to their constituent trigraphs: a participant
/// covered by a tetragraph (e.g., GBR via FVEY) is considered present.
pub(crate) fn e014_joint_rel_to_coverage(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let joint = match &attrs.classification {
        Some(marque_ism::MarkingClassification::Joint(j)) => j,
        _ => return Vec::new(),
    };
    let missing: Vec<&str> = joint
        .countries
        .iter()
        .filter(|c| !rel_to_covers(&attrs.rel_to, c.as_str()))
        .map(|c| c.as_str())
        .collect();
    if missing.is_empty() {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E014/joint-requires-rel-to-coverage",
        message: format!(
            "JOINT participants [{}] must appear in REL TO list",
            missing.join(", ")
        ),
        citation: "CAPCO-2016 §H.3 p57",
        span: None,
        severity: None,
    }]
}

/// E021 — RD or FRD requires NOFORN (unless a sharing agreement under
/// Atomic Energy Act section 123 or 144 applies). CAPCO §H.6 p104 (RD)
/// + p111 (FRD).
///
/// Intentionally narrower than `AnyInCategory(CAT_AEA)`:
/// - **TFNI is excluded.** §H.6 p120 Relationship clause is silent on
///   NOFORN ("May only be used with TOP SECRET, SECRET, or
///   CONFIDENTIAL"); §H.6 p121 Notional Example 2 shows
///   `SECRET//TFNI//REL TO USA, ACGU` as a valid release-authorized
///   marking, and Note 4 ("TFNI may be shared with foreign partners
///   in accordance with existing DNI and IC element guidance") makes
///   the NOFORN requirement contextual, not categorical. Lumping
///   TFNI with RD/FRD would auto-rewrite valid release-authorized
///   TFNI markings — a Constitution VIII fidelity defect.
/// - **UCNI variants are excluded.** Neither DOE UCNI (§H.6 p116) nor
///   DoD UCNI (§H.6 p118) carries the NOFORN requirement.
pub(crate) fn e021_aea_requires_noforn(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let has_rd_or_frd = attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Rd(_) | marque_ism::AeaMarking::Frd(_)
        )
    });
    if !has_rd_or_frd {
        return Vec::new();
    }
    let has_noforn = attrs
        .dissem_iter()
        .any(|d| matches!(d, marque_ism::DissemControl::Nf));
    if has_noforn {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E021/aea-requires-noforn",
        message: "RD/FRD requires NOFORN unless a sharing agreement exists \
                  per the Atomic Energy Act"
            .to_owned(),
        citation: "CAPCO-2016 §H.6 p104 + p111",
        span: None,
        severity: None,
    }]
}

/// E038 — NODIS / EXDIS require NOFORN. CAPCO-2016 §H.9 p172
/// (EXDIS: "Requires NOFORN") and p174 (NODIS: "Requires NOFORN").
/// Emits a single ConstraintViolation when the marking carries NODIS
/// or EXDIS without NOFORN present.
pub(crate) fn e038_dos_dissem_requires_noforn(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let has_nodis_or_exdis = attrs.non_ic_dissem.iter().any(|d| {
        matches!(
            d,
            marque_ism::NonIcDissem::Nodis | marque_ism::NonIcDissem::Exdis
        )
    });
    if !has_nodis_or_exdis {
        return Vec::new();
    }
    let has_noforn = attrs
        .dissem_iter()
        .any(|d| matches!(d, marque_ism::DissemControl::Nf));
    if has_noforn {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E038/nodis-or-exdis-requires-noforn",
        message: "NODIS and EXDIS may be used only with NOFORN information".to_owned(),
        citation: "CAPCO-2016 §H.9 p172 + p174",
        span: None,
        severity: None,
    }]
}

/// E024 — RD takes precedence over FRD/TFNI. Fires when RD AND any of
/// (FRD, TFNI) are present. The wrapper enumerates per-element to emit one
/// `Diagnostic` per offending marking with byte-precise spans; this helper
/// emits ONE `ConstraintViolation` whose presence signals the wrapper to do
/// that work. CAPCO §H.6 p104.
pub(crate) fn e024_rd_precedence(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let has_rd = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Rd(_)));
    if !has_rd {
        return Vec::new();
    }
    let has_superseded = attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Frd(_) | marque_ism::AeaMarking::Tfni
        )
    });
    if !has_superseded {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E024/rd-precedence",
        message: "RD takes precedence over FRD/TFNI; FRD/TFNI should not appear alongside RD"
            .to_owned(),
        citation: "CAPCO-2016 §H.6 p104",
        span: None,
        severity: None,
    }]
}

/// W002 — US classification + FGI marker is commingling. Always fires when
/// both are present; the wrapper filters by `RuleContext::marking_type ==
/// Portion`. CAPCO §H.7 lines 8254-8268.
pub(crate) fn w002_us_commingled_with_fgi(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    if attrs.us_classification().is_none() || attrs.fgi_marker.is_none() {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "W002/us-commingled-with-fgi",
        message: "portion mark comingles US classification with FGI; \
                  consider splitting into separate US and foreign paragraphs"
            .to_owned(),
        citation: "CAPCO-2016 §H.7 p124",
        span: None,
        severity: None,
    }]
}

/// Single source of truth for the class-floor catalog's
/// presence-check + floor-satisfaction-check + diagnostic message
/// shape. PR D R3.1 (R3 C2) consolidated the walker hot-path and the
/// trait/validate path here so a citation, message-text, or
/// floor-comparison change to one row cannot silently diverge between
/// emitters. Post PR 3c.B Commit 7.3 the walker is retired and the
/// engine's constraint-catalog bridge is the sole emitter — but the
/// convergence shape stays for any future second emitter path.
///
/// Returns `None` when the row's predicate does not fire (presence
/// false OR floor satisfied). Returns `Some(ConstraintViolation)`
/// when the row fires; the violation carries the row's `name` as
/// `constraint_label`, the formatted diagnostic message, and the
/// row's `citation` verbatim — matching the
/// `marque_scheme::constraint::evaluate` Custom-arm contract.
///
/// The diagnostic message uses the *effective* classification level
/// (reciprocal-raised for NATO / FGI / JOINT classifications via
/// [`marque_ism::MarkingClassification::effective_level`]) so a
/// portion classified `//NATO SECRET//ATOMAL` reports `SECRET` —
/// not `unknown` — even though `attrs.us_classification()` returns
/// `None` for non-US classification kinds. This is the C1 fix from
/// PR #324 R1; see [`class_floor_satisfied`] doc for the AtLeast vs
/// EqualsU split.
///
/// # Span and severity (PR 3c.B Commit 7.3)
///
/// `span` and `severity` are populated here so the engine's
/// constraint-catalog bridge can surface the violation as a
/// user-facing `Diagnostic` without going through the retired
/// `DeclarativeClassFloorRule` walker:
///   - `span` resolves via [`class_floor_anchor_span`] (lifted from
///     the walker in this commit) so the diagnostic squiggle anchors
///     at the marking token, not the classification token (PM
///     directive #2).
///   - `severity` is the row's authoring intent (`Error` for
///     enumerated rows; `Warn` for passthrough rows per
///     `marque-applied.md` §3.4.6 Q-3.4.6b).
pub(crate) fn class_floor_emit(
    attrs: &marque_ism::CanonicalAttrs,
    row: &ClassFloorRow,
) -> Option<ConstraintViolation> {
    if !(row.presence)(attrs) {
        return None;
    }
    if class_floor_satisfied(attrs, row.policy) {
        return None;
    }
    let level_str = attrs
        .classification
        .as_ref()
        .map(|c| c.effective_level().banner_str())
        .unwrap_or("unknown");
    let message = if row.passthrough {
        format!(
            "{} is known from ISM but not enumerated in CAPCO-2016; provisional classification \
             floor is C (classified). Verify against the current ODNI manual; current \
             classification is {level_str}. (See marque-applied.md §3.7 passthrough policy.)",
            row.marking_label
        )
    } else {
        match row.policy {
            ClassFloorPolicy::AtLeast(floor) => format!(
                "{} requires classification ≥ {} ({}); current classification is {level_str}",
                row.marking_label,
                floor.banner_str(),
                row.citation
            ),
            ClassFloorPolicy::EqualsU => format!(
                "{} may only be used with UNCLASSIFIED information ({}); current classification \
                 is {level_str}",
                row.marking_label, row.citation
            ),
        }
    };
    Some(ConstraintViolation {
        constraint_label: row.name,
        message,
        citation: row.citation,
        span: Some(class_floor_anchor_span(attrs, row)),
        severity: Some(row.severity),
    })
}

/// Single source of truth for the SCI per-system catalog's emit logic.
/// Post-PR-3c.B-Commit-7.4 the engine's constraint-catalog bridge
/// (`CapcoScheme::bridge_sci_per_system_diagnostics`) is the only
/// production caller; the legacy walker `DeclarativeSciPerSystemRule`
/// retired in 7.4 and the trait/validate path
/// (`sci_per_system_catalog_eval`) emits `ConstraintViolation` envelopes
/// without `FixProposal` for non-bridge consumers.
///
/// `#[inline]` because the bridge's hot path is the bench-gate-relevant
/// one and the emit dispatch is a 2-arm match on a `Copy` enum field —
/// inlining lets the compiler hoist the row's presence predicate +
/// kind dispatch into the catalog-walk loop.
///
/// Returns an empty `Vec` when the row's presence predicate doesn't fire
/// or when no diagnostic is warranted; otherwise returns one or more
/// `Diagnostic` values per the row's emit logic.
#[inline]
pub(crate) fn sci_per_system_emit(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_ism::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    if !(row.presence)(attrs) {
        return Vec::new();
    }
    match row.kind {
        SciPerSystemKind::CompanionRequired { dissem, token_name } => {
            emit_companion_required(attrs, candidate_span, fix_scope, row, dissem, token_name)
        }
        SciPerSystemKind::Custom(emit_fn) => emit_fn(attrs, candidate_span, fix_scope, row),
    }
}
