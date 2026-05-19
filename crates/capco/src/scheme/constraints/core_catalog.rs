// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Core constraint rows: dyadic Conflicts / Requires / Custom rows
//! covering E010 through E057 (plus `capco/joint-requires-usa`).
//! Lifted from the monolithic `constraints.rs` per the issue #466
//! Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).
//!
//! Row order is load-bearing for the predicate evaluator's
//! tiebreakers; the entries below preserve the exact pre-split
//! ordering.

use marque_scheme::{Constraint, Severity, TokenRef};

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
/// T035 (2026-04-21) wired runtime evaluation through this
/// catalog: dyadic variants dispatch via the generic evaluator
/// (`crate::constraint::evaluate`) using
/// [`Self::satisfies`]; `Custom` variants dispatch through
/// [`Self::evaluate_custom`] to scheme-private predicate
/// helpers below. The hand-written `Rule` impls in
/// `crate::rules` that previously enforced these invariants
/// are retired in the same PR; `crate::rules_declarative`
/// hosts thin wrappers that call `scheme.validate()` and
/// construct `Diagnostic` values with byte-identical
/// message/span/fix output.
///
/// T035b audit (2026-04-21): E017, E018, and E019 were
/// retired as over-restrictive relative to CAPCO-2016 §H.3
/// pp 56–57:
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
            severity: Some(Severity::Error),
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
            severity: Some(Severity::Error),
            span_anchor: Some(TokenRef::Token(TOK_HCS)),
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
        // ---- E070: FRD precedence over TFNI (§H.6 p120) ------
        //
        // §H.6 TFNI subsection p120: "If the TFNI marking is
        // contained in any portion of a document that contains
        // portions of RD and/or FRD, the RD or FRD takes
        // precedence." Same page on commingling: "If TFNI is
        // commingled with RD or FRD within a portion, the RD or
        // FRD takes precedence and 'RD' or 'FRD,' as
        // appropriate, is annotated in the portion mark."
        //
        // Sibling of E024: E024 covers RD>FRD and RD>TFNI; this
        // row carries the FRD>TFNI leg so the policy decision
        // "FRD supersedes TFNI" has its own audit lineage
        // independent of RD presence. #559 close-out PM
        // decision 2026-05-19.
        Constraint::Custom {
            name: "E070/frd-tfni-precedence",
            label: "CAPCO-2016 §H.6 p120",
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
            name: "capco/noforn-conflicts-rel-to",
            left: TokenRef::Token(TOK_NOFORN),
            right: TokenRef::AnyInCategory(CAT_REL_TO),
            label: "CAPCO-2016 §H.8 p145",
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
            severity: Some(Severity::Error),
            span_anchor: Some(TokenRef::Token(TOK_RELIDO)),
        },
        // ---- E055: RELIDO ⊥ DISPLAY ONLY (§H.8 p154) ------------
        //
        // §H.8 RELIDO entry p154, same Relationship(s) prose.
        Constraint::Conflicts {
            name: "E055/relido-conflicts-display-only",
            left: TokenRef::Token(TOK_RELIDO),
            right: TokenRef::Token(TOK_DISPLAY_ONLY),
            label: "CAPCO-2016 §H.8 p154",
            severity: Some(Severity::Error),
            span_anchor: Some(TokenRef::Token(TOK_RELIDO)),
        },
        // ---- E056: ORCON ⊥ RELIDO (§H.8 p136) -------------------
        //
        // §H.8 ORCON entry p136: "May not be used with RELIDO."
        Constraint::Conflicts {
            name: "E056/orcon-conflicts-relido",
            left: TokenRef::Token(TOK_ORCON),
            right: TokenRef::Token(TOK_RELIDO),
            label: "CAPCO-2016 §H.8 p136",
            severity: Some(Severity::Error),
            span_anchor: Some(TokenRef::Token(TOK_RELIDO)),
        },
        // ---- E057: ORCON-USGOV ⊥ RELIDO (§H.8 p140) -------------
        //
        // §H.8 ORCON-USGOV entry p140: same exclusion as ORCON.
        Constraint::Conflicts {
            name: "E057/orcon-usgov-conflicts-relido",
            left: TokenRef::Token(TOK_ORCON_USGOV),
            right: TokenRef::Token(TOK_RELIDO),
            label: "CAPCO-2016 §H.8 p140",
            severity: Some(Severity::Fix),
            span_anchor: Some(TokenRef::Token(TOK_RELIDO)),
        },
        // NOTE — S004 (REL TO trigraph suggest) is NOT a catalog row.
        // The retired-rule consolidation in PR #578 attempted to move
        // S004 into the constraint-catalog bridge, but S004's
        // replacement string is a corpus-derived candidate computed
        // during evaluation — the bridge's
        // `fix_intent_by_name(name, attrs, marking_type)` shape
        // cannot produce that candidate without re-running the
        // evaluator. The walker rule `RelToTrigraphSuggestRule`
        // therefore stays registered in `CapcoRuleSet::new()` and
        // owns both the predicate and the `text_correction` emission.
        // See `crates/capco/src/rules.rs` S004 registration block for
        // the rationale.
        //
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
    ]
}
