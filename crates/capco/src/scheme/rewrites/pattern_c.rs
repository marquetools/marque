// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 4b-C Commit 3 — Pattern-C strip rows + UCNI NOFORN-promotion
//! siblings. Lifted from the monolithic `rewrites.rs` per the issue
//! #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).
//!
//! Declaration order is load-bearing: promote-before-strip for each
//! UCNI pair so the promote predicate sees UCNI before the strip
//! removes it.

use marque_scheme::{
    CategoryAction, CategoryPredicate, FactRef, PageRewrite, ReplacementIntent, Scope,
    SectionLetter, capco,
};

use super::super::actions::{strip_dod_ucni_action, strip_doe_ucni_action};
use super::super::predicates::{
    dod_ucni_classified_trigger, dod_ucni_promotes_noforn_trigger, doe_ucni_classified_trigger,
    doe_ucni_promotes_noforn_trigger, fouo_classified_trigger, limdis_classified_trigger,
    sbu_classified_trigger, sbu_nf_classified_trigger,
};
use super::super::*;

/// The seven Pattern-C strip + UCNI-NOFORN-promote rows in
/// declaration order: LIMDIS, SBU, DOD UCNI (promote then strip),
/// DOE UCNI (promote then strip), FOUO.
pub(super) fn pattern_c_rows() -> Vec<PageRewrite<CapcoScheme>> {
    // PR 4b-C Commit 3 — Pattern-C strip rows.
    //
    // FOUO classified-strip: reads classification (gate) only;
    // writes DISSEM (FactRemove FOUO). The FOUO-presence scan
    // lives in the `fouo_classified_trigger` Custom predicate body
    // — declaring CAT_DISSEM as a read here would manufacture a
    // same-axis self-reference cycle in Kahn's algorithm (the row
    // is a DISSEM-writer). §H.8 p134.
    //
    // Plan §3.4 risk #4 resolution: predicate-scan-vs-dataflow
    // convention (same approach taken by PR 3b.B entries 5 / 6a / 6b).
    const PATTERN_C_FOUO_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const PATTERN_C_FOUO_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // LIMDIS / SBU classified-strip: reads classification only;
    // writes NON_IC (FactRemove). The NON_IC-presence scan lives
    // in each Custom predicate body — same-axis self-reference
    // avoidance (plan §3.4 risk #4). §H.9 p170 / p176.
    const PATTERN_C_LIMDIS_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const PATTERN_C_LIMDIS_WRITES: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];
    const PATTERN_C_SBU_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const PATTERN_C_SBU_WRITES: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];
    // #541 — SBU-NF classified-strip. Same shape as the SBU row
    // above (`CAT_CLASSIFICATION` read, `CAT_NON_IC_DISSEM` write);
    // the SBU-NF-presence scan lives in the
    // `sbu_nf_classified_trigger` Custom predicate body — same-axis
    // self-reference avoidance (plan §3.4 risk #4).
    // §H.9 p178 (Commingling Rule(s) Within a Portion).
    const PATTERN_C_SBU_NF_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const PATTERN_C_SBU_NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];

    // UCNI strip: reads classification only; writes AEA (Custom
    // action removing DodUcni / DoeUcni variant only). AEA-presence
    // scan lives in the Custom predicate body. §H.6 p116-117 (DOD
    // UCNI) + §H.6 p118-119 (DOE UCNI).
    const PATTERN_C_UCNI_STRIP_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const PATTERN_C_UCNI_STRIP_WRITES: &[marque_scheme::CategoryId] = &[CAT_AEA];

    // UCNI NOFORN promotion: reads classification + AEA (UCNI
    // presence), writes DISSEM (FactAdd NOFORN). The "no stricter
    // FD&R marker" suppression lives in the Custom predicate body
    // (`dod_ucni_promotes_noforn_trigger` checks `dissem_has_noforn`)
    // so we DO NOT declare CAT_DISSEM as a read; otherwise Kahn's
    // algorithm would see this row as both reading and writing
    // CAT_DISSEM, manufacturing a same-axis self-reference that the
    // engine rejects. §H.6 p116 / p118.
    const PATTERN_C_UCNI_PROMOTE_READS: &[marque_scheme::CategoryId] =
        &[CAT_CLASSIFICATION, CAT_AEA];
    const PATTERN_C_UCNI_PROMOTE_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    vec![
        // ===============================================================
        // PR 4b-C Commit 3 — Pattern-C strip rows (5 rows + 2 promotes)
        // ===============================================================
        //
        // Pattern C: classification-driven strip of UNCLASSIFIED-only
        // controls. The CAPCO authority lives across §H.6 (DOD/DOE
        // UCNI), §H.8 (FOUO), and §H.9 (LIMDIS, SBU). Each row carries
        // its own §-citation thread per D13 single-§-citation
        // discipline. The five strip rows fire when a classified
        // portion appears on the page alongside the U-only control;
        // the two UCNI rows additionally promote NOFORN per §H.6's
        // explicit "less restrictive FD&R marking would otherwise be
        // conveyed" clause (the load-bearing pre-fix bug that the
        // imperative pre-PR-4b-C UCNI classified-strip silently
        // dropped — see Commit 2's regression test
        // `pattern_c_dod_ucni_classified_strip_promotes_noforn`).
        //
        // Trigger shape: all seven rows use `CategoryPredicate::Custom`
        // because `Contains` cannot express the
        // `classification > Unclassified` gate. Each `Custom` predicate
        // carries explicit `reads` / `writes` axis annotations per
        // Constitution VII §IV (the engine rejects unannotated
        // `Custom` axes with `EngineConstructionError::UnannotatedCustomAxes`).
        //
        // Scheduler ordering: every strip + promote row writes either
        // CAT_AEA, CAT_NON_IC_DISSEM, or CAT_DISSEM. All seven rows
        // are ordered BEFORE `capco/noforn-clears-rel-to` (DISSEM-
        // reader) by the Kahn scheduler. The two UCNI promote rows
        // would self-reference DISSEM if `reads` included
        // CAT_DISSEM, so the FD&R-suppressor scan lives in the
        // predicate body (`dod_ucni_promotes_noforn_trigger`) instead
        // — declaring DISSEM only in `writes` avoids the
        // manufactured-cycle case the plan §3.4 risk #4 names.
        //
        // Runtime execution gap: the seven rows are scheduler-
        // validated (Engine::new validates the intent payloads +
        // topological ordering) but execution-deferred. `Engine::lint`
        // continues to drive banner validation through `PageContext`
        // until PR 4b-D wires the lattice path. The post-Commit-5
        // single source of truth is `scheme.project(Scope::Page, ...)`,
        // which fires these rows.
        //
        // §3.5 compound-NF guard: `TOK_SBU` triggers match ONLY
        // `NonIcDissem::Sbu` (the bare variant). `NonIcDissem::SbuNf`
        // is a distinct variant carrying NOFORN identity via the
        // existing `capco/sbu-nf-implies-noforn` rewrite; Pattern C
        // MUST NOT strip the compound variants. The
        // `sbu_classified_trigger` predicate explicitly matches
        // `NonIcDissem::Sbu` (not `SbuNf`).
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`
        // (each §-citation re-verified at authorship per
        // Constitution VIII).

        // Pattern-C row 1: `capco/limdis-evicted-by-classified`.
        //
        // §H.9 p170 (LIMITED DISTRIBUTION, Precedence Rules for
        // Banner Line Guidance): "When a document contains LIMDIS
        // and classified portions, LIMDIS is not used in the
        // banner line."
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/limdis-evicted-by-classified",
            capco(SectionLetter::H, 9, 170),
            CategoryPredicate::Custom(limdis_classified_trigger),
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_LIMDIS)],
                scope: Scope::Page,
            }),
            PATTERN_C_LIMDIS_READS,
            PATTERN_C_LIMDIS_WRITES,
        ),
        // Pattern-C row 2: `capco/sbu-evicted-by-classified`.
        //
        // §H.9 p176 (SENSITIVE BUT UNCLASSIFIED, Precedence Rules
        // for Banner Line Guidance): "When a document contains SBU
        // and classified portions, SBU is not used in the banner
        // line."
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/sbu-evicted-by-classified",
            capco(SectionLetter::H, 9, 176),
            CategoryPredicate::Custom(sbu_classified_trigger),
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_SBU)],
                scope: Scope::Page,
            }),
            PATTERN_C_SBU_READS,
            PATTERN_C_SBU_WRITES,
        ),
        // Pattern-C row 2b: `capco/sbu-nf-evicted-by-classified` (#541).
        //
        // §H.9 p178 (SBU NOFORN Commingling Rule(s) Within a
        // Portion): "If the portion is classified, the
        // classification level of the portion adequately protects
        // the SBU information, so SBU is not reflected in the
        // portion mark; however a NOFORN marking must be added to
        // the portion mark, e.g., (C//NF)."
        //
        // The §3.5 compound-NF guard previously excluded SBU-NF
        // from Pattern-C strip handling on the (correct) reasoning
        // that the parallel `capco/sbu-nf-implies-noforn` Pattern-A
        // row carries NF identity separately. The carve-out for
        // SBU-NF on classified pages comes from §H.9 p178's
        // explicit "SBU is not reflected" prescription — the SBU-NF
        // compound token MUST be stripped to converge to (C//NF)
        // rather than (C//SBU-NF) or (C//NF//SBU). See
        // `sbu_nf_classified_trigger` and `apply_fact_remove`'s
        // CAT_NON_IC_DISSEM arm for the §3.5 invariant's revised
        // shape.
        //
        // The asymmetric LES-NF case (§H.9 p185 explicitly retains
        // LES on classified pages) does NOT get a
        // parallel `capco/les-nf-evicted-by-classified` row — LES
        // survives classification by regulatory design (see
        // `NonIcDissemSet`'s type-level doc-comment for the
        // legal-process / originator-control rationale). The
        // `pattern_c_les_in_classified_propagates_to_banner` fixture
        // in `crates/capco/tests/lattice_vs_scheme_parity.rs` is the
        // regression gate against accidentally adding such a row.
        //
        // Co-fires with `capco/sbu-nf-implies-noforn` (Pattern-A):
        // that row writes CAT_DISSEM (FactAdd NOFORN), this row
        // writes CAT_NON_IC_DISSEM (FactRemove SBU-NF) — different
        // axes, no scheduler conflict.
        //
        // verified 2026-05-18 against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/sbu-nf-evicted-by-classified",
            capco(SectionLetter::H, 9, 178),
            CategoryPredicate::Custom(sbu_nf_classified_trigger),
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_SBU_NF)],
                scope: Scope::Page,
            }),
            PATTERN_C_SBU_NF_READS,
            PATTERN_C_SBU_NF_WRITES,
        ),
        // Pattern-C row 3: `capco/dod-ucni-promotes-noforn-when-classified`.
        //
        // §H.6 p116 (DOD UCNI / DCNI, Precedence Rules for Banner
        // Line Guidance): "Classified documents: DOD UCNI does not
        // appear in the banner line; however, NOFORN must be
        // applied if a less restrictive FD&R marking would
        // otherwise be conveyed with the classified information."
        //
        // The strip-vs-promote split (this row plus
        // `capco/dod-ucni-evicted-by-classified` below) reflects
        // the `CategoryAction::Intent` single-intent carrier —
        // one FactAdd on DISSEM + one custom-action strip on AEA
        // cannot be combined in a single row. **Declaration
        // order matters at runtime** (the project loop applies
        // rewrites in declaration order against the mutating
        // state): the promote row MUST appear BEFORE the strip
        // row, because the promote's trigger reads
        // `attrs.aea_markings` (via `has_dod_ucni`) and would
        // observe an empty axis if the strip had already fired.
        // The scheduler's Kahn ordering is consistent with this:
        // the promote row writes CAT_DISSEM while the strip row
        // writes CAT_AEA, so both are independent of the other's
        // axis writes and their relative declaration order
        // governs runtime. The topological scheduler makes no
        // ordering guarantee between sibling rows sharing
        // identical `reads` / `writes` axes (see
        // `crates/engine/src/scheduler.rs` `schedule_rewrites` —
        // edges form only between distinct read/write axis pairs,
        // and Kahn seeds the frontier with in-degree-0 nodes in
        // declaration order); this pair is intentionally
        // sibling-position-ordered in the declaration `Vec`
        // because the runtime evaluator walks the scheduler-
        // produced slice in index order. Pins:
        // `pin_ucni_promote_before_strip_declaration_order` in
        // `crates/capco/tests/page_context_lattice_parity.rs`.
        //
        // Predicate body `dod_ucni_promotes_noforn_trigger` checks
        // `!dissem_has_noforn(m)` so the promotion suppresses when
        // NOFORN is already present (§H.6 p116's "less restrictive
        // FD&R marking would otherwise be conveyed" condition).
        // The check lives in the predicate body so we DO NOT
        // declare CAT_DISSEM as a read, preventing a same-axis
        // self-reference (Plan §3.4 risk #4 resolution).
        //
        // Action: FactAdd TOK_NOFORN, Scope::Page. Idempotent via
        // `apply_fact_add`'s CAT_DISSEM arm — if NOFORN is somehow
        // already present (e.g., via a parallel FactAdd intent),
        // the add is a per-intent no-op.
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/dod-ucni-promotes-noforn-when-classified",
            capco(SectionLetter::H, 6, 116),
            CategoryPredicate::Custom(dod_ucni_promotes_noforn_trigger),
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            PATTERN_C_UCNI_PROMOTE_READS,
            PATTERN_C_UCNI_PROMOTE_WRITES,
        ),
        // Pattern-C row 4: `capco/dod-ucni-evicted-by-classified`.
        //
        // §H.6 p116 (DOD UCNI / DCNI, same passage as row 3): the
        // strip half of the strip-vs-promote split. Declared AFTER
        // the promote row so the promote sees UCNI before this row
        // strips it.
        //
        // Custom action `strip_dod_ucni_action` removes only the
        // DodUcni variant; `apply_fact_remove`'s CAT_AEA arm does
        // not yet handle UCNI variant discrimination (TOK_UCNI is
        // a single sentinel covering both DodUcni and DoeUcni;
        // separating them would require a sentinel-payload extension
        // out of scope for PR 4b-C). The Custom-action route works
        // around this cleanly and the same path lands the DOE row
        // (row 6).
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/dod-ucni-evicted-by-classified",
            capco(SectionLetter::H, 6, 116),
            CategoryPredicate::Custom(dod_ucni_classified_trigger),
            CategoryAction::Custom(strip_dod_ucni_action),
            PATTERN_C_UCNI_STRIP_READS,
            PATTERN_C_UCNI_STRIP_WRITES,
        ),
        // Pattern-C row 5: `capco/doe-ucni-promotes-noforn-when-classified`.
        //
        // §H.6 p118 (DOE UCNI, Precedence Rules for Banner Line
        // Guidance): "Classified documents: DOE UCNI does not
        // appear in the banner line; however, use NOFORN if a less
        // restrictive FD&R marking would otherwise be conveyed
        // with the classified information." Mirrors §H.6 p116
        // (DOD UCNI) verbatim with `use NOFORN` / `NOFORN must be
        // applied` as the only wording variation.
        //
        // Same promote-before-strip declaration order as the DOD
        // UCNI pair (rows 3 + 4).
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/doe-ucni-promotes-noforn-when-classified",
            capco(SectionLetter::H, 6, 118),
            CategoryPredicate::Custom(doe_ucni_promotes_noforn_trigger),
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            PATTERN_C_UCNI_PROMOTE_READS,
            PATTERN_C_UCNI_PROMOTE_WRITES,
        ),
        // Pattern-C row 6: `capco/doe-ucni-evicted-by-classified`.
        //
        // §H.6 p118 (DOE UCNI, same passage as row 5). Strip half
        // of the strip-vs-promote split; declared after the
        // promote row so the promote sees DoeUcni before this row
        // strips it.
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/doe-ucni-evicted-by-classified",
            capco(SectionLetter::H, 6, 118),
            CategoryPredicate::Custom(doe_ucni_classified_trigger),
            CategoryAction::Custom(strip_doe_ucni_action),
            PATTERN_C_UCNI_STRIP_READS,
            PATTERN_C_UCNI_STRIP_WRITES,
        ),
        // Pattern-C row 7: `capco/fouo-evicted-by-classified`.
        //
        // §H.8 p134 (FOUO Precedence Rules for Banner Line Guidance):
        // "FOUO in a classified document:
        //  - When a classified document contains portions of FOUO
        //    information, the FOUO marking is not used in the
        //    banner line."
        //
        // Pattern-B's `capco/non-fdr-control-evicts-fouo` row (PR 4b-C
        // Commit 4) covers the complementary "U + other non-FD&R
        // control" case from the same §H.8 p134 passage; the two
        // rows are scheduler-siblings (both write CAT_DISSEM
        // FactRemove FOUO) and their FactRemove intents are
        // idempotent — running both on a `(S//FOUO + other-non-FDR)`
        // page is a per-intent no-op on the second invocation.
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/fouo-evicted-by-classified",
            capco(SectionLetter::H, 8, 134),
            CategoryPredicate::Custom(fouo_classified_trigger),
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_FOUO)],
                scope: Scope::Page,
            }),
            PATTERN_C_FOUO_READS,
            PATTERN_C_FOUO_WRITES,
        ),
    ]
}
