// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Core constraint rows: dyadic Conflicts / Requires / Custom rows
//! covering E010 through E057 (plus `capco/joint-requires-usa`).
//!
//! Row order is load-bearing for the predicate evaluator's
//! tiebreakers; the entries below preserve the exact pre-split
//! ordering.

use marque_scheme::{Constraint, SectionLetter, Severity, TokenRef, capco};

use super::super::*;

/// The first chunk of the CAPCO constraint catalog.
///
/// Returns dyadic Conflicts / Requires / Custom rows in declaration
/// order. The class-floor and SCI per-system catalogs are appended
/// by [`build_constraints`](super::build_constraints).
///
/// The CAPCO declarative constraint catalog. Every entry's
/// `label` cites a verified passage in
/// `crates/capco/docs/CAPCO-2016.md`; the non-normative tail
/// (sections I-K — history, examples, acronym list) are NOT
/// valid citation targets. See Constitution VIII and the project
/// memory entry "CAPCO doc structure".
///
/// Runtime evaluation runs through this catalog: dyadic variants
/// dispatch via the generic evaluator (`crate::constraint::evaluate`)
/// using [`Self::satisfies`]; `Custom` variants dispatch through
/// [`Self::evaluate_custom`] to scheme-private predicate helpers below.
/// The engine's scheme-adapter bridge (`crate::scheme::adapter`) hosts
/// the dispatch that calls `scheme.validate()` and constructs
/// `Diagnostic` values.
///
/// No JOINT-incompatibility constraints are declared for IC / Non-IC
/// dissemination control markings — that would be over-restrictive
/// relative to CAPCO-2016 §H.3 pp 56–57:
///
/// - §H.3 p57 lists "FGI, IC and Non-IC dissemination
///   control markings (excluding NOFORN)" among markings
///   JOINT "may be used with, as appropriate"
/// - §H.3 p57 names only two explicit exclusions:
///   HCS markings and NOFORN markings
/// - §H.3 p57 cross-references §H.7 for FGI content marker
///   syntax on JOINT documents — FGI marker presence is a
///   content indicator, not a competing classification type
///
/// The JOINT+NOFORN exclusion is caught indirectly: E014
/// requires JOINT to carry REL TO, and
/// `capco/noforn-conflicts-rel-to` fires when NOFORN and REL
/// TO co-occur. The JOINT+HCS exclusion has no such indirect
/// coverage, so it gets its own catalog entry below as E036.
pub(super) fn core_constraints() -> Vec<Constraint> {
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
            name: "portion.sci.hcs-system-constraints",
            label: capco(SectionLetter::H, 4, 62),
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
            name: "portion.classification.dual-classification",
            label: capco(SectionLetter::H, 3, 55),
        },
        // ---- E014: JOINT requires REL TO coverage (§H.3 p57) -
        //
        // §H.3 p57 (Relationship(s) to Other Markings): "Requires
        // REL TO USA, LIST". Every JOINT participant MUST also
        // appear in the marking's REL TO list. Custom (not
        // Requires) because the check is iterative across all
        // JOINT countries.
        Constraint::Custom {
            name: "portion.classification.joint-requires-rel-to-coverage",
            label: capco(SectionLetter::H, 3, 57),
        },
        // ---- W005: REL TO expands beyond JOINT participants (§H.3 p57) --
        //
        // §H.3 p57 ("[LIST]" superset semantics): a classifier may expand
        // REL TO beyond JOINT co-owners. Marque cannot distinguish intentional
        // expansion from authoring error — surfaces as Warn (no auto-fix).
        // Reverse of E014: E014 flags JOINT participants missing from REL TO
        // (auto-fix); W005 flags REL TO entries beyond JOINT (advisory only).
        //
        // Audit emission: typed `MessageTemplate::RelToExpandsBeyondJoint`
        // resolved via `CapcoScheme::message_by_name` (closing #666).
        Constraint::Custom {
            name: "portion.classification.rel-to-not-in-joint-coverage",
            label: capco(SectionLetter::H, 3, 57),
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
            name: "portion.classification.non-us-requires-dissem",
            left: TokenRef::AnyInCategory(CAT_NON_US_CLASSIFICATION),
            right: TokenRef::AnyInCategory(CAT_DISSEM),
            label: capco(SectionLetter::H, 7, 122),
            severity: Some(Severity::Error),
        },
        // ---- E016: JOINT conflicts RESTRICTED (§H.3 p56) -----
        //
        // §H.3 p56 (Relationship(s) to Other Markings): "May not
        // be used with RESTRICTED. (Note: the US is always a
        // JOINT marking owner/producer; and RESTRICTED is not an
        // authorized US classification marking.)"
        Constraint::Conflicts {
            name: "portion.classification.joint-conflicts-restricted",
            left: TokenRef::Token(TOK_JOINT),
            right: TokenRef::Token(TOK_RESTRICTED),
            label: capco(SectionLetter::H, 3, 56),
            severity: Some(Severity::Error),
            span_anchor: None,
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
        // This is the only JOINT-incompatibility constraint: JOINT
        // conflicts with HCS. JOINT is NOT forbidden against FGI content
        // markers, arbitrary IC dissem, or non-IC dissem — §H.3 p57
        // explicitly permits those combinations. See project memory
        // `feedback_audit_predicates_against_source.md`.
        Constraint::Conflicts {
            name: "portion.classification.joint-conflicts-hcs",
            left: TokenRef::Token(TOK_JOINT),
            right: TokenRef::Token(TOK_HCS),
            label: capco(SectionLetter::H, 3, 57),
            severity: Some(Severity::Error),
            span_anchor: Some(TokenRef::Token(TOK_HCS)),
        },
        // ---- E021: RD/FRD requires NOFORN (§H.6 p104 + p111) -
        //
        // §H.6 RD entry p104: "Is always used with NOFORN
        // unless a sharing agreement has been established per
        // the Atomic Energy Act. (Ref. Sections 123 and 144 of
        // the Atomic Energy Act, and DoD Instruction 5030.14.)".
        // §H.6 FRD entry p111: same "always used with NOFORN
        // unless a sharing agreement" clause. The scope is RD
        // and FRD ONLY — TFNI (§H.6 p120) and UCNI variants
        // (DOD UCNI §H.6 p116, DOE UCNI §H.6 p118) carry no
        // such requirement.
        //
        // #559 close-out (PM decision 2026-05-19): row renamed
        // from `E021/aea-requires-noforn` (misleading "aea-"
        // prefix; the predicate is and has always been RD/FRD
        // only). Severity dropped from `Fix` to `Warn` and the
        // §123/§144 sharing-agreement carve-out is now
        // byte-observable: a portion that already carries
        // `REL TO` or `RELIDO` is evidence that the author has
        // made a release decision under some sharing
        // instrument, so the warning suppresses. See helper
        // doc on `e021_rd_frd_requires_noforn` for the
        // carve-out's pragmatic-substitute rationale.
        //
        // Custom (not `Requires { left: AnyInCategory(CAT_AEA) }`)
        // because that dyadic shape would sweep UCNI in: a valid
        // `U//UCNI` marking would incorrectly require NOFORN.
        Constraint::Custom {
            name: "portion.aea.rd-frd-requires-noforn",
            label: capco(SectionLetter::H, 6, 104),
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
        // the predicate level. A `Constraint::Requires { TOK_CNWDI,
        // TOK_RD }` row enforcing the §H.6 p106 "subset of RD" rule
        // would be unreachable — it could never fire because the
        // right-hand side is necessarily true whenever the left-hand
        // side is true.
        //
        // The §H.6 p106 invariant therefore lives at the data-model
        // level rather than the constraint-catalog level.
        //
        // If a future change to `AeaMarking` ever splits CNWDI
        // into a sibling variant (decoupling it from `Rd`), the
        // §H.6 p106 enforcement MUST be re-introduced as a
        // Constraint::Requires or equivalent — and the
        // satisfies_attrs predicate for `TOK_CNWDI` MUST be
        // amended to no longer match through the `Rd(...)`
        // variant.
        // The CNWDI classification floor lives in the class-floor
        // catalog block below, not as a core constraint row.

        // ---- RD precedence (§H.6 p104) -----------------
        //
        // §H.6 RD entry p104: "If RD, FRD, and TFNI
        // portions are in a document, the RD takes precedence
        // and is conveyed in the banner line." Custom (not
        // Supersedes) because Supersedes is a banner-rollup
        // hint that doesn't fire diagnostics; the per-portion
        // commingling violation is what this row reports. The
        // banner-rollup Supersedes entries are deferred until they
        // are wired through `project(Scope::Page, ...)`.
        Constraint::Custom {
            name: "portion.aea.rd-precedence",
            label: capco(SectionLetter::H, 6, 104),
        },
        // ---- FRD precedence over TFNI (§H.6 p120) ------
        //
        // §H.6 TFNI subsection p120: "If the TFNI marking is
        // contained in any portion of a document that contains
        // portions of RD and/or FRD, the RD or FRD takes
        // precedence." Same page on commingling: "If TFNI is
        // commingled with RD or FRD within a portion, the RD or
        // FRD takes precedence and 'RD' or 'FRD,' as
        // appropriate, is annotated in the portion mark."
        //
        // Sibling of the RD-precedence row: that row covers RD>FRD and
        // RD>TFNI; this row carries the FRD>TFNI leg so the "FRD
        // supersedes TFNI" decision has its own audit lineage
        // independent of RD presence (#559).
        Constraint::Custom {
            name: "portion.aea.frd-tfni-precedence",
            label: capco(SectionLetter::H, 6, 120),
        },
        // The UCNI ceiling invariant lives in the class-floor catalog
        // block below as TWO rows (DOD-UCNI-classification-ceiling at
        // §H.6 p116 and DOE-UCNI-classification-ceiling at §H.6 p118),
        // split so each variant carries its own §H.6 sub-page citation.

        // ---- W002: retired in the PR closing #470 ------------
        //
        // The §H.7 p124 segregation rule the row was modeling is
        // conditional on ICD-206 status — a document-level
        // property the engine cannot determine from a portion.
        // CAPCO-2016 §H.7 p123 lines 3051-3065 (vendored at
        // `crates/capco/docs/CAPCO-2016.md`) explicitly authorize
        // the `(US-CLASS//FGI [LIST]//NF)` shape under the
        // "Example Portion Mark (when sources are acknowledged,
        // but not segregated from US)" entry. The predicate
        // matched every authorized portion in the corpus,
        // emitting noise without a useful action.
        //
        // ---- capco/noforn-conflicts-rel-to (§H.8 p145) -------
        //
        // §H.8 NOFORN entry p145: "Cannot be used with
        // REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY." This is
        // the portion-level exclusion; the page-rewrite that
        // clears REL TO when NOFORN is present at page scope is
        // declared separately in `build_page_rewrites`.
        Constraint::Conflicts {
            name: "portion.dissem.noforn-conflicts-rel-to",
            left: TokenRef::Token(TOK_NOFORN),
            right: TokenRef::AnyInCategory(CAT_REL_TO),
            label: capco(SectionLetter::H, 8, 145),
            severity: Some(Severity::Error),
            span_anchor: None,
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
            name: "portion.classification.joint-requires-usa",
            label: capco(SectionLetter::H, 3, 55),
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
            name: "portion.dissem.nodis-conflicts-exdis",
            left: TokenRef::Token(TOK_NODIS),
            right: TokenRef::Token(TOK_EXDIS),
            label: capco(SectionLetter::H, 9, 172),
            severity: Some(Severity::Error),
            span_anchor: Some(TokenRef::Token(TOK_NODIS)),
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
        // and the dispatch layer in the scheme-adapter bridge
        // (`crate::scheme::adapter`) works by filtering violations by
        // constraint `name`. Splitting into two `Requires` constraints
        // would create two distinct violation names for one rule ID and
        // force the bridge to OR them. Folding the disjunction into a
        // single Custom predicate keeps the bridge trivial.
        Constraint::Custom {
            name: "portion.dissem.nodis-or-exdis-requires-noforn",
            label: capco(SectionLetter::H, 9, 172),
        },
        // ---- E054: RELIDO ⊥ NOFORN (§H.8 p154) ------------------
        //
        // §H.8 RELIDO entry p154, Relationship(s) to Other Markings:
        // "Cannot be used with NOFORN or DISPLAY ONLY."
        //
        // This row is an enumerated `Conflicts`. The scheme-adapter
        // bridge (`crate::scheme::adapter::CapcoScheme::fix_intent_by_name`)
        // dispatches by the constraint name
        // `"portion.dissem.relido-conflicts-noforn"`; without an
        // enumerated row here, the bridge would silently emit no
        // diagnostics.
        Constraint::Conflicts {
            name: "portion.dissem.relido-conflicts-noforn",
            left: TokenRef::Token(TOK_RELIDO),
            right: TokenRef::Token(TOK_NOFORN),
            label: capco(SectionLetter::H, 8, 154),
            severity: Some(Severity::Error),
            span_anchor: Some(TokenRef::Token(TOK_RELIDO)),
        },
        // The three RELIDO-exclusion pairs live in
        // `crates/capco/src/scheme/rewrites/relido_clears.rs` as
        // subtractive PageRewrites, not as core constraint rows:
        //
        //   `capco/display-only-clears-relido` (§H.8 p154)
        //   `capco/orcon-clears-relido`        (§H.8 p136)
        //   `capco/orcon-usgov-clears-relido`  (§H.8 p140)
        //
        // Each fires at `Scope::Page` and emits a `FactRemove(RELIDO)`
        // intent at the right scope for cross-portion supersession
        // (e.g., ORCON on portion A and RELIDO on portion B is missed by
        // a per-portion Conflicts gate). Per Marque convention,
        // dissem-axis conflicts emit subtractive fixes: the engine
        // guides the author toward a canonical resolution (RELIDO removed
        // when a stronger originator decision is on the page) rather than
        // just flagging the conflict.
        //
        // The DISPLAY ONLY clears-RELIDO rewrite requires
        // `satisfies(TOK_DISPLAY_ONLY)` to recognize the canonical wire
        // form: the parser routes `DISPLAY ONLY [LIST]` into
        // `attrs.display_only_to` (a country-list axis parallel to
        // `attrs.rel_to`) without setting the `DissemControl::Displayonly`
        // variant in `dissem_us`, so a `Contains(CAT_DISSEM,
        // TOK_DISPLAY_ONLY)` trigger would silently no-op on the
        // canonical input unless the predicate ORs both axes (#618).
        //
        // The RELIDO ⊥ NOFORN row below stays a Conflicts row because
        // the companion `capco/noforn-clears-fdr-family` PageRewrite in
        // `noforn_clears.rs` already covers the page-scope eviction — the
        // Conflicts row surfaces the per-portion form as an Error for
        // user visibility on the source line that triggered the conflict.
        // NOTE — the REL TO trigraph suggest rule is NOT a catalog row.
        // Its replacement string is a corpus-derived candidate computed
        // during evaluation — the bridge's
        // `fix_intent_by_name(name, attrs, marking_type)` shape cannot
        // produce that candidate without re-running the evaluator. The
        // walker rule `RelToTrigraphSuggestRule` therefore stays
        // registered in `CapcoRuleSet::new()` and owns both the
        // predicate and the `text_correction` emission.
        //
        // NOTE — the catalog uses enumerated `Conflicts` rows, not
        // additive `ConflictsWithFamily` rows. Adding a family row
        // alongside an enumerated row would make `CapcoScheme::validate()`
        // emit DOUBLE diagnostics for any input that triggers both (the
        // same matching pair appears once per row). The
        // `ConflictsWithFamily` primitive is exercised on a stub scheme
        // by `crates/scheme/tests/proptest_constraint_rhs_family_distributive.rs`;
        // the CAPCO catalog does not need active family-row entries.
    ]
}
