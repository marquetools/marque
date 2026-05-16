// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` page-rewrite catalog (PR 4b-C; PR 3c.B Sub-PR 8.F / 8.F.2;
//! PR 4b-A). Lifted from the monolithic `scheme.rs` per the issue #466
//! split plan (`claudedocs/refactor-466/split_proposal.md`, Risk 1 Option 2).
//!
//! See [`build_page_rewrites`] for the full inventory and per-row authority.

use marque_scheme::{
    CategoryAction, CategoryPredicate, FactRef, PageRewrite, ReplacementIntent, Scope,
};

use super::actions::{noop_action, strip_dod_ucni_action, strip_doe_ucni_action};
use super::predicates::{
    dod_ucni_classified_trigger, dod_ucni_promotes_noforn_trigger, doe_ucni_classified_trigger,
    doe_ucni_promotes_noforn_trigger, fouo_classified_trigger,
    fouo_with_non_fdr_other_control_trigger, limdis_classified_trigger, never_fires,
    sbu_classified_trigger,
};
use super::*;

/// Construct CAPCO's `PageRewrite` table.
///
/// **23 rewrites, in six groups** (post-PR-4b-C, 006 T112; PR
/// 4b-A landed group 4; PR 3c.B Sub-PR 8.F / 8.F.2 landed group
/// 3; PR 4b-C landed groups 5 + 6 as Pattern-C + Pattern-B
/// declarative rows that own the §H.6 / §H.8 / §H.9 strip-plus-
/// promote semantics):
///
/// 1. **Pattern-A NOFORN-supremacy (4):** the §H.9 family (landed by
///    PR 3c.B-8.F) — `capco/{nodis,exdis}-implies-noforn` (§H.9 p174 /
///    §H.9 p172) and `capco/{sbu-nf,les-nf}-implies-noforn`
///    (§H.9 p178 / §H.9 p185). All four are wired predicates that
///    fire today via `scheme.project(Scope::Page, ...)`.
/// 2. **PR 4b-C Pattern-C strip rows (7):** §H.6 / §H.8 / §H.9
///    classification-driven strips of UNCLASSIFIED-only controls
///    plus the §H.6 NOFORN-promotion siblings —
///    `capco/limdis-evicted-by-classified` (§H.9 p170),
///    `capco/sbu-evicted-by-classified` (§H.9 p176), four UCNI
///    rows declared **promote-before-strip** so the NOFORN-
///    promotion predicate observes UCNI before the strip
///    removes it (`capco/{dod,doe}-ucni-{promotes-noforn-when-
///    classified, evicted-by-classified}` at §H.6 p116 / p118),
///    and `capco/fouo-evicted-by-classified` (§H.8 p134
///    classified sub-clause).
/// 3. **PR 4b-C Pattern-B structural FOUO-eviction (2):**
///    `capco/classification-evicts-fouo` +
///    `capco/non-fdr-control-evicts-fouo`, both at §H.8 p134.
///    The two rows quote the same §H.8 p134 umbrella passage
///    but cite distinct sub-clauses (classified-document vs
///    UNCLASSIFIED with other dissemination controls).
/// 4. **Active wired rows (1):** `capco/noforn-clears-rel-to`
///    (`Contains` predicate + `Clear` action). Cited at §D.2
///    Table 3 + §H.8 p145. First PageRewrite to land in the
///    catalog; canonical worked example in
///    `crates/capco/README.md`.
/// 5. **DISPLAY-ONLY / FD&R-family (1):**
///    `capco/noforn-clears-fdr-family` per DISPLAY ONLY Phase 2
///    landing at §D.2 Table 3 row 2 + §H.8 p154 + §H.8 p157.
/// 6. **Phase-3 transmutation stubs (8):** the §3.4.1 / §3.4.3
///    transmutation roster from `marque-applied.md` (consultant
///    Entry 6 split into 6a + 6b for D13 single-citation
///    discipline). Each declares a `Custom(never_fires)` trigger
///    and a `Custom(noop_action)` body — Phase 3 does not drive
///    page roll-up through `scheme.project()` for these, so the
///    trigger pins to `false` and the action body is empty. The
///    `reads` / `writes` annotations are what the Kahn scheduler
///    consumes (T031–T032) to validate dataflow ordering; the
///    runtime semantics still live in the hand-coded
///    [`PageContext`] aggregator. Phase D / Phase E replaces the
///    `Custom` bodies with real predicates and transforms.
///
/// # `reads` semantics — narrow form
///
/// `reads` declares **true dataflow dependencies only**: axes
/// whose post-rewrite state this rewrite consumes from another
/// rewrite. Axes the trigger only pattern-matches against
/// (predicate-scan reads) are documented in the per-entry
/// doc-comment but excluded from the `reads` slice. Inflating
/// `reads` with predicate-scan axes manufactures false cycles in
/// the scheduler's dependency graph: the engine scheduler at
/// `crates/engine/src/scheduler.rs:78-95` only skips
/// *same-rewrite* self-edges (`producer_idx == idx`), so two
/// independent rewrites that each read AND write the same axis
/// produce a mutual edge in both directions and abort
/// `Engine::new` with `RewriteCycle`. Predicate-scan axes go in
/// the doc-comment with the explicit phrase "predicate scans X
/// for Y"; if Phase D/E discovers a real dataflow dependency on
/// a documented predicate-scan axis, the corresponding `reads`
/// annotation can be re-introduced and the scheduler's DAG will
/// reflect it.
///
/// The eight Phase-3 stubs (in topological order):
///
/// 1. `capco/frd-sigma-consolidates-into-rd-sigma` (§H.6 p113) —
///    AEA-only, independent.
/// 2. `capco/fgi-rollup-on-us-contact` (§H.7 p122) — bare-FGI
///    rollup on US-class contact.
/// 3. `capco/fgi-restricted-rollup-on-us-contact` (§H.7 p122) —
///    bare-FGI-R contact rolls FGI list (class lift is
///    parser-side per §3.4.1 Note (i)).
/// 4. `capco/joint-cross-class-rollup` (§H.3 p57) — JOINT [list]
///    on non-US-class contact rolls FGI [non-US JOINT members].
/// 5. `capco/us-presence-promotes-bare-fgi-attribution`
///    (§H.7 p122) — idempotent FGI cleanup; runs after entries
///    1–3 (consumes their FGI_MARKER output, the one structural
///    FGI_MARKER read in the table).
/// 6. `capco/orcon-nato-to-us-orcon-on-us-contact` (§H.8 p136) —
///    ORCON-NATO transmutes to US ORCON on US-class contact.
/// 7. `capco/sbu-nf-transmutes-on-classified-contact`
///    (§H.9 p178) — SBU-NF transmutes on classified contact.
/// 8. `capco/les-nf-transmutes-on-classified-contact`
///    (§H.9 p185) — LES-NF transmutes on classified contact.
///
/// Source: `marque-applied.md` §3.4.1 + §3.4.3. Declaration order
/// is one valid total ordering of the rewrite vector (it groups
/// `noforn-clears-rel-to` first as the canonical worked example,
/// followed by entries 4, 1, 2, 3, 7, 5, 6a, 6b in the order
/// they appear in the consultant roster). It is **not** the
/// scheduler's topological order — `noforn-clears-rel-to` reads
/// `CAT_DISSEM` which entries 5/6a/6b write, so the scheduler
/// orders it AFTER those entries. `Engine::new` runs Kahn's
/// algorithm at construction; runtime execution order is
/// determined by the scheduler, not by this `Vec` order.
///
/// [`CategoryPredicate::Contains`]: marque_scheme::CategoryPredicate::Contains
/// [`CategoryAction::Clear`]: marque_scheme::CategoryAction::Clear
/// [`Engine::lint`]: marque_engine::Engine::lint
pub(crate) fn build_page_rewrites() -> Vec<PageRewrite<CapcoScheme>> {
    // `capco/noforn-clears-rel-to` reads `CAT_DISSEM` to look for
    // NOFORN and writes `CAT_REL_TO` to clear it. The CAT_DISSEM
    // read is a real dataflow dependency on entries 5/6a/6b,
    // which write CAT_DISSEM (ORCON-NATO → ORCON, SBU-NF/LES-NF
    // transmutations) — the scheduler must order this rewrite
    // AFTER those entries so the clearer sees the post-
    // transmutation NOFORN state. The CAT_REL_TO read is a
    // self-edge (skipped by the scheduler at
    // `crates/engine/src/scheduler.rs:84-87`), retained as
    // defensive ordering for future REL-TO writers.
    //
    // (REL TO appearing as its own category — rather than as a
    // dissem-control subtype — is an artifact of `CanonicalAttrs`
    // modeling country-list resolution separately; the rewrite
    // semantics treat it as a first-class category that
    // producers can write.)
    const NF_READS: &[marque_scheme::CategoryId] = &[CAT_DISSEM, CAT_REL_TO];
    const NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_REL_TO];

    // `capco/noforn-clears-fdr-family` reads CAT_DISSEM (to find
    // both the NOFORN trigger and the RELIDO / EYES / DISPLAY ONLY
    // targets) and writes CAT_DISSEM (the multi-fact FactRemove
    // removes the FD&R-family tokens from the same category).
    // Self-edge skipped per the scheduler. Same DAG sibling
    // position as `capco/noforn-clears-rel-to`: both read
    // CAT_DISSEM (post `*-implies-noforn` writes) and operate on
    // axes the *-implies-noforn entries don't touch.
    const NF_CLEARS_FDR_FAMILY_READS: &[marque_scheme::CategoryId] = &[CAT_DISSEM];
    const NF_CLEARS_FDR_FAMILY_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // Entry 4 (consultant §3.4.1 #4): FRD-SIGMA consolidates into
    // RD-SIGMA. Within-axis transform on CAT_AEA — reads and
    // writes the same axis (self-edge skipped per
    // `crates/engine/src/scheduler.rs:84-87`). Topologically
    // independent of every other entry.
    const E4_READS: &[marque_scheme::CategoryId] = &[CAT_AEA];
    const E4_WRITES: &[marque_scheme::CategoryId] = &[CAT_AEA];

    // Entry 1 (consultant §3.4.1 #1): bare-FGI rollup on US
    // contact. Narrow-form reads: CLASS only. Predicate-scan of
    // CAT_FGI_MARKER (for bare-FGI atoms) is documented in the
    // per-entry doc-comment, not in `reads`; declaring it would
    // cycle against entries 2 and 3 (each writes FGI_MARKER and
    // would read it through their own predicate-scan). Reciprocal
    // class raise is parser-side per §3.4.1 Note (i), so CLASS is
    // not in `writes`.
    const E1_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const E1_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

    // Entry 2 (consultant §3.4.1 #2): bare-FGI-R rollup on US
    // contact. Narrow-form reads: CLASS only (see Entry 1 note
    // on predicate-scan vs dataflow reads). Class lift to ≥ C is
    // parser-side per §3.4.1 Note (i).
    const E2_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const E2_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

    // Entry 3 (consultant §3.4.1 #3): JOINT cross-class rollup.
    // Reads CLASS plus JOINT_CLASSIFICATION (the trigger
    // axis — the JOINT scan IS the read, no predicate-scan
    // doc-comment needed). Writes FGI_MARKER only — §H.3 p57
    // is explicit that JOINT does NOT carry forward to the
    // banner line in US documents, so this rewrite consumes
    // JOINT state without writing it back; class lift is
    // parser-side per §3.4.1 Note (i).
    const E3_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION, CAT_JOINT_CLASSIFICATION];
    const E3_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

    // Entry 7 (consultant §3.4.1 #7): US-presence promotes bare
    // FGI attribution. The CAT_FGI_MARKER read IS structural
    // here — entry 7 consumes the post-rewrite FGI state
    // produced by entries 1, 2, 3 and idempotently promotes any
    // remaining `bare(_, C, _)` to `⊤(C)`. This is the one
    // entry whose FGI_MARKER read is a real dataflow dep, not a
    // predicate-scan artifact, so it stays in `reads`.
    const E7_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION, CAT_FGI_MARKER];
    const E7_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

    // Entry 5 (consultant §3.4.1 #5): ORCON-NATO transmutes to
    // US ORCON on US-class contact. Narrow-form reads: CLASS
    // only. Predicate-scan of CAT_DISSEM (for ORCON-NATO) is
    // doc-comment only; declaring it would cycle against
    // entries 6a/6b (each writes DISSEM and would read it
    // through their own predicate-scan).
    const E5_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const E5_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // Entry 6a (consultant §3.4.1 #6, split per D13): SBU-NF
    // transmutes on classified contact. Narrow-form reads:
    // CLASS only (see Entry 5 note on predicate-scan vs
    // dataflow reads — predicate also scans `non_ic_dissem`
    // field for SBU-NF). Per Phase-3 pragmatic mapping
    // (plan §8 Q1), the non-IC dissem axis is folded into
    // CAT_DISSEM until Phase D/E exposes a separate
    // `CAT_NON_IC_DISSEM`.
    const E6A_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const E6A_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // Entry 6b (consultant §3.4.1 #6, split per D13): LES-NF
    // transmutes on classified contact. Same narrow-form +
    // axis-mapping pragmatism as Entry 6a. Cited at §H.9 p185
    // (LES-NF is its own §H.9 subsection p185–186, distinct
    // from SBU-NF p178).
    const E6B_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const E6B_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // PR 3c.B Sub-PR 8.F — Pattern A NOFORN-supremacy: NODIS and EXDIS.
    //
    // Both rewrites read `CAT_NON_IC_DISSEM` (to detect the
    // NODIS / EXDIS token) and write `CAT_DISSEM` (to add NOFORN).
    // The `reads = [CAT_NON_IC_DISSEM]` / `writes = [CAT_DISSEM]`
    // dataflow annotations make the Kahn scheduler order these two
    // rewrites BEFORE `capco/noforn-clears-rel-to`, which reads
    // `CAT_DISSEM`. This guarantees that once a NODIS or EXDIS
    // portion is seen on the page, NOFORN is in the projected dissem
    // state before the clearer runs — so the REL TO axis is correctly
    // cleared in the same projection pass.
    //
    // No existing rewrite writes `CAT_NON_IC_DISSEM`, so the new
    // rewrites have no upstream producers on that axis and can run
    // in declaration order relative to each other.
    //
    // FUTURE (Pattern A SCI follow-on): 5 more `*-implies-noforn`
    // rewrites for SCI systems (HCS-O / HCS-P-sub / TK-IDIT /
    // TK-BLFH / TK-KAND per §H.4 p64 / p68 / p87 / p91 / p95)
    // will read `CAT_SCI` and write `CAT_DISSEM`. They are a
    // structural peer of these two entries but land in a follow-on
    // sub-PR (8.F.2 or Stage-4 SCI NOFORN-implication PR) after
    // `capco_category_contains` is extended for `CAT_SCI` + token
    // dispatch.
    //
    // Runtime execution gap (design spec §5): these rewrites are
    // scheduler-validated (Engine::new validates intent payloads +
    // topological ordering) but execution-deferred (`Engine::lint` /
    // `Engine::fix` drives banner-validation through
    // `marque_ism::PageContext` directly, not through
    // `scheme.project`). Callers that invoke
    // `scheme.project(Scope::Page, …)` directly see the full
    // declarative effect today. Engine-level effect lands when
    // Phase D/E wires the banner-validation path through
    // `scheme.project`.
    const NODIS_IMPLIES_NF_READS: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];
    const NODIS_IMPLIES_NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];
    const EXDIS_IMPLIES_NF_READS: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];
    const EXDIS_IMPLIES_NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // PR 3c.B Sub-PR 8.F.2 — SBU-NF and LES-NF Pattern A axes.
    // Same axis-flow as the 8.F NODIS/EXDIS pair: reads
    // `CAT_NON_IC_DISSEM` (to detect the SBU-NF / LES-NF token)
    // and writes `CAT_DISSEM` (to add NOFORN). Both new entries
    // join the same DISSEM-writer cohort and are ordered BEFORE
    // `capco/noforn-clears-rel-to` (DISSEM-reader) by the Kahn
    // scheduler.
    const SBU_NF_IMPLIES_NF_READS: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];
    const SBU_NF_IMPLIES_NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];
    const LES_NF_IMPLIES_NF_READS: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];
    const LES_NF_IMPLIES_NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

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

    // PR 4b-C Commit 4 — Pattern-B structural FOUO eviction.
    //
    // classification-evicts-fouo: same axes as PATTERN_C_FOUO —
    // reads `[CAT_CLASSIFICATION]` only; writes `[CAT_DISSEM]`.
    // CAT_DISSEM is intentionally NOT in `reads` even though the
    // predicate scans it for FOUO, because the existing
    // `capco/noforn-clears-fdr-family` row already reads + writes
    // CAT_DISSEM (the scheduler accepts that as a 1-row self-
    // edge); declaring another reads-DISSEM/writes-DISSEM row
    // creates a 2-row cycle (Kahn rejects). The FOUO-presence
    // scan lives in the `fouo_classified_trigger` Custom
    // predicate body. §H.8 p134 (FOUO-in-classified clause).
    //
    // Plan §3.4 risk #4 resolution: predicate-scan-vs-dataflow
    // convention, identical to the Pattern-C rows in Commit 3.
    const PATTERN_B_CLASS_FOUO_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const PATTERN_B_CLASS_FOUO_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // non-fdr-control-evicts-fouo: reads NON_IC, AEA, SAR
    // (the three "other control" surfaces whose presence is the
    // load-bearing trigger); writes DISSEM (FactRemove FOUO).
    // §H.8 p134 (FOUO-with-other-non-FD&R clause).
    //
    // CAT_DISSEM is intentionally NOT in `reads` even though the
    // predicate scans it for non-FD&R-other-than-FOUO tokens.
    // Declaring it would create a 2-row cycle with the existing
    // `capco/noforn-clears-fdr-family` row (which reads + writes
    // CAT_DISSEM; the scheduler accepts that as a 1-row self-edge
    // but rejects a 2-row reads-DISSEM/writes-DISSEM cycle).
    // The DISSEM-presence scan lives in the
    // `fouo_with_non_fdr_other_control_trigger` Custom predicate
    // body. Plan §3.4 risk #4 (same-axis self-reference)
    // resolution: predicate-scan-vs-dataflow convention,
    // identical to the Pattern-C rows in Commit 3.
    const PATTERN_B_NON_FDR_READS: &[marque_scheme::CategoryId] =
        &[CAT_NON_IC_DISSEM, CAT_AEA, CAT_SAR];
    const PATTERN_B_NON_FDR_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    vec![
        // PR 3c.B Sub-PR 8.F — `capco/nodis-implies-noforn`.
        //
        // CAPCO-2016 §H.9 p174 (NO DISTRIBUTION, Relationship(s) to
        // Other Markings):
        //   "- May be used with TOP SECRET, SECRET, CONFIDENTIAL,
        //      or UNCLASSIFIED.
        //    - NODIS and EXDIS markings cannot be used together.
        //    - Requires NOFORN."
        //
        // The "Requires NOFORN." line is the operative authority for
        // this rewrite. The NODIS entry's "Precedence Rules for Banner
        // Line Guidance" (p174) further states: "REL TO is not
        // authorized in the banner line if any portion contains NODIS
        // information. In this case, NOFORN would convey in the banner
        // line." — confirming NOFORN as the foreign-release vehicle
        // when NODIS is present.
        //
        // Trigger: `Contains(CAT_NON_IC_DISSEM, TOK_NODIS)` — fires
        // when any portion on the page carries NODIS in its
        // `non_ic_dissem` axis. Resolved by the
        // `capco_category_contains` extension in PR 3c.B Sub-PR 8.F.
        //
        // Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`
        // — adds NOFORN to the projected page dissem axis. Monotone-
        // additive: FactAdd with an already-present token is a
        // per-intent no-op (IntentInapplicable, silent) per the
        // idempotence policy in `apply_fact_add` (scheme.rs:624-639).
        //
        // Axis annotations: reads `[CAT_NON_IC_DISSEM]`, writes
        // `[CAT_DISSEM]`. The Kahn scheduler (engine/src/scheduler.rs)
        // places this rewrite BEFORE `capco/noforn-clears-rel-to`
        // (which reads CAT_DISSEM) so the REL TO axis is correctly
        // cleared in the same projection pass when NODIS is present.
        // Declaration order here also respects this invariant: the two
        // `*-implies-noforn` entries appear before `noforn-clears-rel-to`
        // in the vec so `project`'s sequential scan sees them first.
        //
        // Classification-agnostic: §H.9 p174 says "May be used with
        // TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED" — the
        // trigger predicate is classification-agnostic and fires at
        // any classification level, including UNCLASSIFIED.
        //
        // FUTURE (SCI Pattern A follow-on): 5 more `*-implies-noforn`
        // rewrites reading `CAT_SCI` / writing `CAT_DISSEM` will land
        // alongside this entry in a follow-on sub-PR after
        // `capco_category_contains` is extended for `CAT_SCI` dispatch
        // (§H.4 p64 / p68 / p87 / p91 / p95).
        //
        // Runtime execution gap: this rewrite is scheduler-validated
        // (Engine::new validates the intent payload + topological
        // ordering) but execution-deferred (`Engine::lint` / `Engine::fix`
        // drives banner-validation through PageContext directly). Effect
        // is visible through `scheme.project(Scope::Page, …)`. Engine-
        // level effect lands when Phase D/E wires banner-validation
        // through `scheme.project`.
        PageRewrite::declarative(
            "capco/nodis-implies-noforn",
            "CAPCO-2016 §H.9 p174",
            CategoryPredicate::Contains {
                category: CAT_NON_IC_DISSEM,
                token: TOK_NODIS,
            },
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            NODIS_IMPLIES_NF_READS,
            NODIS_IMPLIES_NF_WRITES,
        ),
        // PR 3c.B Sub-PR 8.F — `capco/exdis-implies-noforn`.
        //
        // CAPCO-2016 §H.9 p172 (EXCLUSIVE DISTRIBUTION, Relationship(s)
        // to Other Markings):
        //   "- May be used with TOP SECRET, SECRET, CONFIDENTIAL,
        //      or UNCLASSIFIED.
        //    - EXDIS and NODIS markings cannot be used together.
        //    - Requires NOFORN."
        //
        // The "Requires NOFORN." line is the operative authority for
        // this rewrite. The EXDIS entry's "Precedence Rules for Banner
        // Line Guidance" (p172) further states: "REL TO is not
        // authorized in the banner line if any portion contains EXDIS
        // information. In this case, NOFORN would convey in the banner
        // line." — confirming NOFORN as the foreign-release vehicle
        // when EXDIS is present.
        //
        // Trigger: `Contains(CAT_NON_IC_DISSEM, TOK_EXDIS)` — fires
        // when any portion on the page carries EXDIS in its
        // `non_ic_dissem` axis.
        //
        // Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`
        // — adds NOFORN to the projected page dissem axis. Same
        // monotone-additive + idempotence policy as the NODIS entry.
        //
        // Axis annotations: reads `[CAT_NON_IC_DISSEM]`, writes
        // `[CAT_DISSEM]`. Scheduler ordering: same sibling position
        // as `capco/nodis-implies-noforn` — both are DISSEM-writers
        // ordered before `noforn-clears-rel-to` (DISSEM-reader).
        // The two `*-implies-noforn` entries are DAG siblings (no
        // ordering dependency between them). Declaration order here
        // also respects this invariant: both appear before
        // `noforn-clears-rel-to` in the vec.
        //
        // Classification-agnostic: §H.9 p172 says "May be used with
        // TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED" — same
        // as the NODIS entry.
        //
        // Note: §H.9 p172 specifies "EXDIS and NODIS markings cannot
        // be used together." — the NODIS ⊥ EXDIS conflict is already
        // enforced by E037 (stays registered per design spec §5 Option
        // R2). Under malformed input where both appear simultaneously,
        // both rewrites fire; the second FactAdd hits the idempotence
        // no-op path (NOFORN already present), producing exactly one
        // NOFORN with no panic.
        //
        // FUTURE: see the NODIS entry doc-comment for the SCI Pattern A
        // follow-on note.
        //
        // Runtime execution gap: see the NODIS entry doc-comment.
        PageRewrite::declarative(
            "capco/exdis-implies-noforn",
            "CAPCO-2016 §H.9 p172",
            CategoryPredicate::Contains {
                category: CAT_NON_IC_DISSEM,
                token: TOK_EXDIS,
            },
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            EXDIS_IMPLIES_NF_READS,
            EXDIS_IMPLIES_NF_WRITES,
        ),
        // PR 3c.B Sub-PR 8.F.2 — `capco/sbu-nf-implies-noforn`.
        //
        // CAPCO-2016 §H.9 p178 (SBU-NF) does NOT contain a "Requires NOFORN."
        // sentence — unlike NODIS (§H.9 p174) and EXDIS (§H.9 p172). The NF
        // implication is derived from three structural anchors:
        //   (a) Banner-form heading at `CAPCO-2016.md:4388-4398`: the marking's
        //       Authorized Banner Line Marking Title literally names it
        //       "SENSITIVE BUT UNCLASSIFIED NOFORN"; portion mark is `SBU-NF`.
        //       NOFORN is a structural component of the marking's identity.
        //   (b) Commingling Rule at `CAPCO-2016.md:4420-4421`: confirms NOFORN
        //       persists after transmutation strips the SBU half — even when
        //       the source token is dropped, the NF must remain in the portion.
        //       Verbatim: "The SBU-NF marking is conveyed in the portion
        //       mark only if the commingled portion is unclassified and there
        //       is no other NOFORN information included in the portion. If
        //       there is other NOFORN information in the commingled portion,
        //       the 'SBU' marking is used and a NOFORN marking is added,
        //       e.g., (U//NF//SBU)." And p4421: "If the portion is
        //       classified, the classification level of the portion
        //       adequately protects the SBU information, so SBU is not
        //       reflected in the portion mark; however a NOFORN marking
        //       must be added to the portion mark, e.g., (C//NF)."
        //   (c) §D.2 Table 3 row 3-5 at `CAPCO-2016.md:590-595`: lists NOFORN
        //       as the FD&R banner consequence for SBU-NF. Back-reference
        //       confirms the page-level dissem-axis invariant.
        //
        // Trigger: `Contains(CAT_NON_IC_DISSEM, TOK_SBU_NF)` — fires
        // when any portion on the page carries SBU-NF in its
        // `non_ic_dissem` axis. Resolved by the
        // `capco_category_contains` extension in PR 3c.B Sub-PR 8.F.2.
        //
        // Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`
        // — adds NOFORN to the projected page dissem axis. Monotone-
        // additive: FactAdd with an already-present token is a
        // per-intent no-op (IntentInapplicable, silent) per the
        // idempotence policy in `apply_fact_add`'s `CAT_DISSEM` arm
        // (the `if category == CAT_DISSEM` block; the
        // `attrs.dissem_iter().any(|d| d == &target)` check returns
        // `IntentInapplicable`). NOT the unmatched-arm fallthrough at
        // the bottom of `apply_fact_add`, which is forward-
        // compatibility only — see the TODO at the CAT_NON_IC_DISSEM
        // arm of `apply_fact_remove`. Line numbers omitted because
        // they drift with refactors; grep `apply_fact_add` to find
        // the current location.
        //
        // Axis annotations: reads `[CAT_NON_IC_DISSEM]`, writes
        // `[CAT_DISSEM]`. The Kahn scheduler places this rewrite
        // BEFORE `capco/noforn-clears-rel-to` (which reads
        // CAT_DISSEM) so the REL TO axis is correctly cleared in the
        // same projection pass when SBU-NF is present.
        //
        // Classification: §H.9 p178 at `:4410` says SBU-NF "May only
        // be used with UNCLASSIFIED" — but the trigger predicate is
        // classification-agnostic (it scans the `non_ic_dissem`
        // axis only). On malformed classified input `(C//SBU-NF)`,
        // Pattern A still fires defensively; the eventual Pattern C
        // `classified-strips-sbu` rewrite will canonicalize the
        // portion to `(C//NF)` per the §H.9 Commingling Rule.
        //
        // FUTURE (SCI Pattern A follow-on): see the NODIS entry
        // doc-comment for the SCI follow-on (§H.4 p64/p68/p87/p91/p95).
        //
        // Runtime execution gap: see the NODIS entry doc-comment.
        // Scheduler-validated but execution-deferred; visible through
        // `scheme.project(Scope::Page, …)`.
        PageRewrite::declarative(
            "capco/sbu-nf-implies-noforn",
            "CAPCO-2016 §H.9 p178",
            CategoryPredicate::Contains {
                category: CAT_NON_IC_DISSEM,
                token: TOK_SBU_NF,
            },
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            SBU_NF_IMPLIES_NF_READS,
            SBU_NF_IMPLIES_NF_WRITES,
        ),
        // PR 3c.B Sub-PR 8.F.2 — `capco/les-nf-implies-noforn`.
        //
        // CAPCO-2016 §H.9 p185 (LES-NF) does NOT contain a "Requires NOFORN."
        // sentence — unlike NODIS (§H.9 p174) and EXDIS (§H.9 p172). The NF
        // implication is derived from three structural anchors:
        //   (a) Banner-form heading at `CAPCO-2016.md:4532-4542`: the marking's
        //       Authorized Banner Line Marking Title literally names it
        //       "LAW ENFORCEMENT SENSITIVE NOFORN"; portion mark is `LES-NF`.
        //       NOFORN is a structural component of the marking's identity.
        //   (b) Precedence Rules for Banner Line Guidance at `CAPCO-2016.md:4558`:
        //       "When a classified document contains portions of U//LES- NF,
        //       the 'LES' marking is used in the banner line and the NOFORN
        //       marking is applied as a Dissemination Control Marking. For
        //       example: SECRET//NOFORN//LES."
        //       // note: source has whitespace OCR artifact "LES- NF" rendered
        //       // with a space; canonical token is LES-NF.
        //       Confirms NOFORN materializes on the projected page dissem
        //       axis even when the LES-NF source token is consolidated into
        //       its LES form by transmutation.
        //   (c) §D.2 Table 3 rows 6-8 at `CAPCO-2016.md:590-595`: lists NOFORN
        //       as the FD&R banner consequence for LES-NF. Back-reference
        //       confirms the page-level dissem-axis invariant.
        //
        // Trigger: `Contains(CAT_NON_IC_DISSEM, TOK_LES_NF)` — fires
        // when any portion on the page carries LES-NF in its
        // `non_ic_dissem` axis. Resolved by the
        // `capco_category_contains` extension in PR 3c.B Sub-PR 8.F.2.
        //
        // Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`
        // — adds NOFORN to the projected page dissem axis. Same
        // monotone-additive + idempotence policy as the SBU-NF entry.
        //
        // Axis annotations: reads `[CAT_NON_IC_DISSEM]`, writes
        // `[CAT_DISSEM]`. Scheduler ordering: same sibling position
        // as the other three `*-implies-noforn` entries — all four
        // are DISSEM-writers ordered before `noforn-clears-rel-to`
        // (DISSEM-reader). The four entries are DAG siblings (no
        // ordering dependency between them).
        //
        // Classification: §H.9 p185's Relationship(s) field at
        // `:4554` says LES-NF "May be used with TOP SECRET, SECRET,
        // CONFIDENTIAL, or UNCLASSIFIED." Unlike SBU-NF, LES-NF is
        // valid at any classification level. Pattern A fires
        // regardless.
        //
        // Source-doc internal contradiction: the same §H.9 p185 entry
        // at `:4552` (Additional Marking Instructions field) reads
        // "Applicable only to unclassified information" — which
        // appears to conflict with the Relationship(s) enumeration
        // at `:4554`. The Relationship(s) field governs behavioral
        // scope (it explicitly enumerates the permitted classification
        // levels) and is the authority for §H.9 entries. The
        // `:4552` line appears to be a vestigial paste from the
        // sibling LES entry (`:4471`, where LES IS unclassified-
        // only) — it is internally inconsistent with `:4554` AND
        // with the Precedence Rule at `:4558` which describes the
        // canonical `SECRET//NOFORN//LES` form for classified docs.
        // `NonIcDissem`'s implementation at `crates/ism/src/attrs.rs`
        // (LesNf variant doc-comment) makes the same `:4554`-governs
        // determination. A future ODNI manual revision may resolve
        // the `:4552` artifact; for now Pattern A defers to `:4554`.
        //
        // FUTURE: see the NODIS entry doc-comment for the SCI
        // Pattern A follow-on note.
        //
        // Runtime execution gap: see the NODIS entry doc-comment.
        PageRewrite::declarative(
            "capco/les-nf-implies-noforn",
            "CAPCO-2016 §H.9 p185",
            CategoryPredicate::Contains {
                category: CAT_NON_IC_DISSEM,
                token: TOK_LES_NF,
            },
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            LES_NF_IMPLIES_NF_READS,
            LES_NF_IMPLIES_NF_WRITES,
        ),
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
        // conveyed" clause (the load-bearing pre-fix bug in
        // `PageContext::expected_aea_markings` — see Commit 2's
        // regression test).
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
            "CAPCO-2016 §H.9 p170",
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
            "CAPCO-2016 §H.9 p176",
            CategoryPredicate::Custom(sbu_classified_trigger),
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_SBU)],
                scope: Scope::Page,
            }),
            PATTERN_C_SBU_READS,
            PATTERN_C_SBU_WRITES,
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
            "CAPCO-2016 §H.6 p116",
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
            "CAPCO-2016 §H.6 p116",
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
            "CAPCO-2016 §H.6 p118",
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
            "CAPCO-2016 §H.6 p118",
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
            "CAPCO-2016 §H.8 p134",
            CategoryPredicate::Custom(fouo_classified_trigger),
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_FOUO)],
                scope: Scope::Page,
            }),
            PATTERN_C_FOUO_READS,
            PATTERN_C_FOUO_WRITES,
        ),
        // ===============================================================
        // PR 4b-C Commit 4 — Pattern-B structural FOUO eviction (2 rows)
        // ===============================================================
        //
        // Pattern B is the second half of the §H.8 p134 FOUO
        // Precedence Rules for Banner Line Guidance. §H.8 p134
        // verbatim, "FOUO in an unclassified document" sub-clause:
        //   "FOUO is not conveyed in the banner line if the document
        //    is UNCLASSIFIED with FOUO and other dissemination
        //    control markings, excluding any FD&R markings."
        //
        // PM Correction A (2026-05-16) replaced the original
        // ~10-row per-trigger FOUO-eviction matrix with two
        // structural rows. The "other dissemination control
        // markings" set is the union of CAT_DISSEM (non-FD&R IC
        // dissem tokens), CAT_NON_IC_DISSEM (LIMDIS / SBU / SSI /
        // LES / NODIS / EXDIS / NNPI / SbuNf / LesNf — every
        // non-IC dissem token), CAT_AEA (RD / FRD / TFNI / UCNI /
        // ATOMAL), and CAT_SAR (any program identifier). The
        // "non-FD&R" qualifier reduces to "not in the broad
        // `FDR_DOMINATORS` membership set" — see
        // `is_fdr_dissem_token` helper. Critically the helper
        // uses `Vocabulary::is_fdr_dissem` semantics (which
        // INCLUDES RELIDO) — NOT `is_fdr_dominator` (which
        // EXCLUDES RELIDO; that helper answers the conflict-
        // dominator question, not the FD&R-membership question).
        // See `scheme.rs:5018-5039` doc-comment on `FDR_DOMINATORS`
        // for the distinction.
        //
        // Row 1 `capco/classification-evicts-fouo` overlaps with
        // Commit 3 row 7 (`capco/fouo-evicted-by-classified`) on
        // their FactRemove target. The overlap is intentional:
        // both rows produce the same FactRemove[TOK_FOUO] payload
        // on a classified page carrying FOUO; the second
        // invocation hits `apply_fact_remove`'s
        // `IntentInapplicable` arm (token already absent) and is
        // a per-intent no-op. Per Plan §3 the two rows have
        // distinct citation threads: Commit 3 row 7 cites only
        // §H.8 p134's "FOUO in a classified document" sub-clause;
        // this row cites §H.8 p134's overall umbrella rule that
        // combines both the classified-strip AND the
        // unclassified-with-other-controls strip. Keeping the two
        // rows separate preserves D13 single-§-citation
        // discipline at the per-row level even though both rows
        // ultimately quote the same §H.8 p134 passage.
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`
        // §H.8 p134 (full FOUO Precedence Rules passage).

        // Pattern-B row 1: `capco/classification-evicts-fouo`.
        //
        // §H.8 p134 (FOUO Precedence Rules for Banner Line
        // Guidance, classified-document sub-clause): "FOUO in a
        // classified document: When a classified document
        // contains portions of FOUO information, the FOUO marking
        // is not used in the banner line."
        //
        // Structurally identical to Commit 3 row 7
        // (`capco/fouo-evicted-by-classified`); both rows produce
        // the same FactRemove[TOK_FOUO] payload. Carried as a
        // separate Pattern-B row so the §H.8 p134 umbrella rule
        // — which contains BOTH the classified-strip clause AND
        // the unclassified-with-other-controls strip clause — has
        // a single Pattern-B citation thread distinct from the
        // Pattern-C dedicated row's narrower citation. FactRemove
        // is idempotent; the second invocation on a page where
        // Commit 3 row 7 already fired is a per-intent no-op via
        // `apply_fact_remove`'s `IntentInapplicable` arm.
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/classification-evicts-fouo",
            "CAPCO-2016 §H.8 p134",
            CategoryPredicate::Custom(fouo_classified_trigger),
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_FOUO)],
                scope: Scope::Page,
            }),
            PATTERN_B_CLASS_FOUO_READS,
            PATTERN_B_CLASS_FOUO_WRITES,
        ),
        // Pattern-B row 2: `capco/non-fdr-control-evicts-fouo`.
        //
        // §H.8 p134 (FOUO Precedence Rules for Banner Line
        // Guidance, unclassified-document sub-clause): "FOUO is
        // not conveyed in the banner line if the document is
        // UNCLASSIFIED with FOUO and other dissemination control
        // markings, excluding any FD&R markings."
        //
        // The §H.8 p134 wording lists "other dissemination
        // control markings" without a classification gate — the
        // sub-clause heads its own bullet under "FOUO in an
        // unclassified document". On unclassified pages where
        // FOUO appears alongside any non-FD&R control on any
        // axis (CAT_DISSEM, CAT_NON_IC_DISSEM, CAT_AEA, CAT_SAR),
        // FOUO is stripped from the banner.
        //
        // The trigger predicate
        // `fouo_with_non_fdr_other_control_trigger` checks the
        // four axes: dissem non-FD&R-other-than-FOUO, non-IC
        // dissem non-empty, AEA non-empty, SAR set. AEA markings
        // (RD / FRD / TFNI / UCNI / ATOMAL) are atomic-energy
        // controls, not FD&R markings; SAR identifiers are
        // program markings, not FD&R markings; non-IC dissem
        // tokens (LIMDIS / LES / SBU / SSI / NODIS / EXDIS /
        // NNPI / SbuNf / LesNf) are non-FD&R by construction —
        // none appears in `FDR_DOMINATORS`.
        //
        // No classification gate: the §H.8 p134 sub-clause
        // applies at any classification level — at classified
        // levels, Pattern-B row 1 / Commit 3 row 7 also fires
        // and the two are idempotent siblings.
        //
        // Axis annotations: reads `[CAT_NON_IC_DISSEM, CAT_AEA,
        // CAT_SAR]` (three "other control" surfaces); writes
        // `[CAT_DISSEM]` (FactRemove FOUO). CAT_DISSEM is
        // intentionally NOT in `reads` even though the predicate
        // also scans it — the existing
        // `capco/noforn-clears-fdr-family` row reads + writes
        // CAT_DISSEM (the scheduler accepts that as a 1-row
        // self-edge); adding another reads-DISSEM/writes-DISSEM
        // row creates a 2-row cycle that Kahn's algorithm
        // rejects. The DISSEM-presence scan lives in
        // `fouo_with_non_fdr_other_control_trigger` (the Custom
        // predicate body). Plan §3.4 risk #4 resolution:
        // predicate-scan-vs-dataflow convention, identical to
        // the Pattern-C rows in Commit 3.
        //
        // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
        PageRewrite::custom(
            "capco/non-fdr-control-evicts-fouo",
            "CAPCO-2016 §H.8 p134",
            CategoryPredicate::Custom(fouo_with_non_fdr_other_control_trigger),
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_FOUO)],
                scope: Scope::Page,
            }),
            PATTERN_B_NON_FDR_READS,
            PATTERN_B_NON_FDR_WRITES,
        ),
        // §D.2 Table 3 (FD&R Markings Precedence Rules for Banner
        // Line Roll-Up) Rule #2 specifies that NOFORN supersedes
        // REL TO at banner scope; the §H.8 NOFORN entry (p145)
        // back-references this table via "Refer to Section D.2.,
        // Table 3 FD&R Markings Precedence Rules for Banner Line
        // Roll-Up for guidance" in its Precedence Rules section.
        //
        // Declaration order note: this entry is placed AFTER the
        // `*-implies-noforn` entries (PR 3c.B Sub-PR 8.F + 8.F.2)
        // which write CAT_DISSEM. The Kahn scheduler also enforces
        // this ordering via the `reads/writes` dataflow annotations;
        // matching the declaration order to the topological order
        // ensures both `scheme.project(Scope::Page, …)` (which
        // iterates declaration order) and the scheduler-driven
        // execution path (Phase D/E) produce the same result.
        PageRewrite::declarative(
            "capco/noforn-clears-rel-to",
            "CAPCO-2016 §D.2 Table 3 + §H.8 p145",
            CategoryPredicate::Contains {
                category: CAT_DISSEM,
                token: TOK_NOFORN,
            },
            CategoryAction::Clear {
                category: CAT_REL_TO,
            },
            NF_READS,
            NF_WRITES,
        ),
        // `capco/noforn-clears-fdr-family` — NOFORN supersedes
        // every other FD&R-class dissem token at banner scope.
        //
        // §D.2 Table 3 rows 1 + 2: "NF + no other FD&R markings →
        // NOFORN" / "NF + any other FD&R marking ... → NOFORN".
        // Row 2's enumeration covers REL TO, RELIDO, USA/[LIST]
        // EYES ONLY, and DISPLAY ONLY explicitly. §H.8 p154 (RELIDO
        // entry) and §H.8 p157-158 (EYES ONLY entry) make the same
        // exclusion at the marking-relationship level.
        //
        // When NF and any of these other FD&R tokens end up
        // together in the projected CAT_DISSEM (e.g., one portion
        // carries the other-FD&R token and another carries NF, or
        // a `*-implies-noforn` rewrite adds NF after
        // `page_context_to_attrs` unions an FD&R portion in), the
        // banner roll-up must keep NF and drop the other tokens.
        // The PageContext-direct path (`expected_dissem_us` Step 6)
        // handles this for callers that read PageContext accessors
        // directly; this PageRewrite mirrors the same policy for
        // `scheme.project(Scope::Page, …)` callers.
        //
        // The companion `capco/noforn-clears-rel-to` rewrite covers
        // the REL TO country-list axis (CAT_REL_TO); this rewrite
        // covers the CAT_DISSEM tokens. There is no `TOK_REL`
        // constant for the bare `REL` dissem marker (CAPCO uses
        // the country list in CAT_REL_TO as the canonical form),
        // so the bare-`Rel` case is handled only at the
        // PageContext layer where the DissemControl enum is
        // visible.
        //
        // Trigger: `Contains(CAT_DISSEM, TOK_NOFORN)` — fires when
        // NOFORN is in the projected page dissem axis (either via
        // direct portion union or via a `*-implies-noforn` rewrite
        // upstream in declaration order).
        //
        // Action: `Intent(FactRemove { [TOK_RELIDO, TOK_EYES,
        // TOK_DISPLAY_ONLY], Scope::Page })` — surgically removes
        // each FD&R-family token from CAT_DISSEM. Idempotent:
        // FactRemove of an absent token is a per-intent no-op
        // (IntentInapplicable, silent), so most pages experience
        // no effect.
        //
        // Axis annotations: reads `[CAT_DISSEM]`, writes
        // `[CAT_DISSEM]` (self-edge skipped per the scheduler).
        // DAG sibling of `capco/noforn-clears-rel-to`: both read
        // CAT_DISSEM after the `*-implies-noforn` writers and
        // operate on disjoint targets (REL TO country axis vs
        // CAT_DISSEM FD&R tokens).
        PageRewrite::declarative(
            "capco/noforn-clears-fdr-family",
            "CAPCO-2016 §D.2 Table 3 row 2 + §H.8 p154 + §H.8 p157",
            CategoryPredicate::Contains {
                category: CAT_DISSEM,
                token: TOK_NOFORN,
            },
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![
                    FactRef::Cve(TOK_RELIDO),
                    FactRef::Cve(TOK_DISPLAY_ONLY),
                    FactRef::Cve(TOK_EYES),
                ],
                scope: Scope::Page,
            }),
            NF_CLEARS_FDR_FAMILY_READS,
            NF_CLEARS_FDR_FAMILY_WRITES,
        ),
        // Entry 4 — `capco/frd-sigma-consolidates-into-rd-sigma`.
        //
        // CAPCO-2016 §H.6 states this same precedence rule from
        // two complementary vantages — the RD-SIGMA subsection
        // (§H.6 p108-109) and the FRD-SIGMA subsection (§H.6
        // p113). The two passages are mutual references with
        // identical operational content:
        //
        //   §H.6 p109 (top-of-page continuation from the p108
        //   RD-SIGMA Precedence Rules block): "If both RD and FRD
        //   SIGMA [#] portions are in a document, the RD-SIGMA [#]
        //   marking takes precedence over the FRD-SIGMA [#]
        //   marking in the banner line and all SIGMA numbers are
        //   listed in the RD-SIGMA [#] marking in the banner line,
        //   regardless of whether the information was RD or FRD."
        //
        //   §H.6 p113 (FRD-SIGMA Precedence Rules for Banner Line
        //   Guidance): "If both RD and FRD SIGMA [#] portions are
        //   in a document, the RD-SIGMA [#] marking takes
        //   precedence over the FRD-SIGMA [#] marking in the
        //   banner line and all SIGMA numbers are listed in the
        //   banner line RD-SIGMA [#] marking, regardless of
        //   whether the information was RD or FRD."
        //
        // Within-axis transform — drops FRD-SIGMA atoms from
        // CAT_AEA and folds their numbers into the surviving
        // RD-SIGMA atom. The `lattice::AeaSet` `Product`
        // composition implements this as the union of axis 3
        // (SIGMA numbers) when axis 1's supersession join lands
        // on `Rd`; see `docs/plans/2026-05-01-lattice-design.md`
        // §7.5 Example 1 for the worked end-to-end case.
        //
        // PR 4b-A (this row's doc-comment update) cites BOTH
        // §H.6 p108-109 and §H.6 p113 in this comment so future
        // readers find the rule from whichever subsection they
        // open first. The row's `citation` field stays
        // `§H.6 p113` (the original landing's citation) — the
        // double-citation lives in this doc-comment, not in the
        // citation string, because Marque's audit emitter reads
        // the citation field as a single token. The brief's
        // working name (`capco/rd-coalesces-sigmas`, §H.6 p108)
        // refers to the same rewrite as this row — same algebra,
        // same axis, same body, mutually-cited subsections.
        //
        // Monotonicity: shrinking on CAT_AEA (FRD-SIGMA atoms
        // dropped). Sound under fixed topological order.
        //
        // Phase-3 stub: trigger is `never_fires` and action is
        // `noop_action` because runtime dispatch stays in
        // `PageContext` until Phase D/E (specifically, PR 4b-B
        // wires the runtime `AeaSet`-driven mutation through
        // `CapcoScheme::project(Scope::Page, ...)`). Only the
        // `reads` / `writes` annotations are consumed (by the
        // scheduler). Topologically independent of every other
        // entry: the AEA axis is otherwise un-written.
        PageRewrite::custom(
            "capco/frd-sigma-consolidates-into-rd-sigma",
            "CAPCO-2016 §H.6 p113",
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E4_READS,
            E4_WRITES,
        ),
        // Entry 1 — `capco/fgi-rollup-on-us-contact`.
        // §H.7 p122 (Precedence Rules for Banner Line Guidance):
        // "If any document contains portions of both source-
        // concealed FGI ... and source-acknowledged FGI ..., then
        // only the 'FGI' marking without the source
        // trigraph(s)/tetragraph(s) must appear in the banner
        // line." Trigger surface is bare-FGI portion contacting
        // US-class; effect is FGI banner rollup. Reciprocal
        // class raise is performed at portion-parse-time per
        // `marque-applied.md` §3.4.1 Note (i), NOT as a rewrite
        // transform — CLASS is not in `writes`.
        //
        // Monotonicity: monotone-additive on FGI axis (concealed
        // wins over acknowledged; acknowledged unions). CLASS
        // not mutated by this rewrite.
        //
        // Predicate scans `CAT_FGI_MARKER` for bare-FGI atoms.
        // The scan axis is documented here, not in `reads`:
        // entries 1, 2, 3 each trigger on disjoint portion-level
        // patterns and each writes `CAT_FGI_MARKER`; declaring
        // FGI_MARKER as a read here would manufacture a
        // false-cycle against entries 2 and 3. The scheduler's
        // coarse "writes determines order" model is sufficient
        // because the three rewrites' FGI outputs are
        // commutative shape-modifications. If Phase D/E
        // discovers a real dataflow dep on the FGI state, add
        // FGI_MARKER to `reads` then.
        //
        // Shared §-citation with Entry 7 is admissible under
        // D13: this entry is the rollup TRIGGER (bare-FGI
        // contacts US-class); Entry 7 is the idempotent
        // generalization that runs after 1–3 settle.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/fgi-rollup-on-us-contact",
            "CAPCO-2016 §H.7 p122",
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E1_READS,
            E1_WRITES,
        ),
        // Entry 2 — `capco/fgi-restricted-rollup-on-us-contact`.
        // §H.7 p122 (Relationship(s) to Other Markings): FGI
        // "may be used with TOP SECRET, SECRET, CONFIDENTIAL,
        // RESTRICTED, UNCLASSIFIED, and other designators ...
        // applied by the non-US originator". Combined with the
        // p123 rollup contract (quoted under Entry 1), bare-
        // FGI-R contacting US-class rolls FGI attribution to
        // `[list]`. Class lift to ≥ C (RESTRICTED is not an
        // authorized US classification, so the reciprocal raise
        // floors at C) is parser-side per
        // `marque-applied.md` §3.4.1 Note (i), NOT a rewrite
        // transform — CLASS is not in `writes`.
        //
        // Monotonicity: monotone-additive on FGI axis
        // (R-classified countries union into the trigraph list).
        // Class lift is parser-side and monotone (R → C is
        // upward only).
        //
        // Predicate scans `CAT_FGI_MARKER` for bare-FGI-R atoms.
        // Same predicate-scan-vs-dataflow convention as Entry 1
        // (see Entry 1 doc-comment); FGI_MARKER excluded from
        // `reads` to avoid manufactured cycles against entries
        // 1 and 3.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/fgi-restricted-rollup-on-us-contact",
            "CAPCO-2016 §H.7 p122",
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E2_READS,
            E2_WRITES,
        ),
        // Entry 3 — `capco/joint-cross-class-rollup`.
        // §H.3 p57 (Derivative Use, banner-line construction):
        // "Highest classification level of all portions,
        // expressed as a US classification marking. ... The
        // FGI marking including all trigraph/tetragraph codes
        // identified in the JOINT portion(s). REL TO, including
        // all common non-US country trigraph/tetragraph codes
        // identified in the JOINT portions, unless a portion is
        // marked NOFORN, in which case the NOFORN marking must
        // appear in the banner line." JOINT [list] contacting a
        // non-US-class portion rolls FGI attribution to list
        // the non-US JOINT members; banner class is the
        // highest-US-class of all portions, established
        // parser-side per §H.3 p57 + `marque-applied.md`
        // §3.4.1 Note (i) — JOINT does NOT carry forward to the
        // banner line in US documents, so this rewrite consumes
        // JOINT state without writing it back, and CLASS is not
        // in `writes`.
        //
        // Monotonicity: monotone-additive on FGI axis (non-US
        // JOINT members union in). Class lift is parser-side
        // and monotone.
        //
        // No predicate-scan note: the `JOINT_CLASSIFICATION`
        // read IS the trigger axis (§H.3 p57 names JOINT
        // explicitly), so it stays in `reads` as a real
        // dataflow read of the page-level JOINT state.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/joint-cross-class-rollup",
            "CAPCO-2016 §H.3 p57",
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E3_READS,
            E3_WRITES,
        ),
        // Entry 7 — `capco/us-presence-promotes-bare-fgi-attribution`.
        // §H.7 p122 (Precedence Rules for Banner Line Guidance,
        // quoted under Entry 1) establishes both the trigger and
        // the post-rollup-cleanup contracts. This entry is the
        // idempotent generalization: after entries 1–3 consolidate
        // FGI state, any remaining `bare(_, C, _)` FGI attribution
        // is promoted to a fully-rolled-up `⊤(C)` form.
        //
        // Monotonicity: monotone-additive. `bare(_, C, _) → ⊤(C)`
        // is a join-monotone `FgiSet` promotion; idempotent on
        // already-promoted state.
        //
        // No predicate-scan note: the `CAT_FGI_MARKER` read here
        // IS a real dataflow dependency on entries 1, 2, 3 —
        // entry 7 consumes their post-rewrite FGI state and
        // promotes any remaining `bare(_, C, _)` attribution.
        // This is the one entry in the table whose FGI_MARKER
        // read is structural, not a predicate-scan artifact, so
        // it stays in `reads` and the scheduler orders entry 7
        // after 1, 2, 3.
        //
        // Shared §-citation with Entry 1 is admissible under
        // D13: Entry 1 is the trigger (bare-FGI contacts
        // US-class); this entry is the idempotent cleanup that
        // runs after 1–3 settle.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/us-presence-promotes-bare-fgi-attribution",
            "CAPCO-2016 §H.7 p122",
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E7_READS,
            E7_WRITES,
        ),
        // Entry 5 — `capco/orcon-nato-to-us-orcon-on-us-contact`.
        // §H.8 p136 (ORCON Precedence Rules for Banner Line
        // Guidance): "If ORCON and ORCON-USGOV portions are in a
        // document, ORCON takes precedence and is conveyed in
        // the banner line." ORCON-NATO (CAPCO-2016 §G p40,
        // Register Table 5 cross-reference to Appendix B NATO
        // protective markings: "ORCON (NATO dissemination control
        // marking) ... See US ORCON ARH requirements") maps onto
        // the same precedence surface — ORCON-NATO contacting
        // US-class transmutes to US ORCON in the page dissem
        // axis. Per D13, the §H.8 p136 cite is the primary
        // anchor; the Appendix B mapping (line 895) is the
        // supplementary reference for ORCON-NATO ↔ US ORCON
        // equivalence.
        //
        // Monotonicity: mixed — drops ORCON-NATO (shrinking) and
        // adds ORCON (additive). Sound under fixed topological
        // order.
        //
        // Predicate scans `CAT_DISSEM` for ORCON-NATO. The scan
        // axis is documented here, not in `reads`: entries 5,
        // 6a, 6b each trigger on disjoint dissem-token patterns
        // and each writes `CAT_DISSEM`; declaring DISSEM as a
        // read here would manufacture a false-cycle against
        // 6a and 6b. The DISSEM-writers are commutative
        // shape-modifications on the page dissem set, so the
        // scheduler's "writes determines order" model is
        // sufficient.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/orcon-nato-to-us-orcon-on-us-contact",
            "CAPCO-2016 §H.8 p136",
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E5_READS,
            E5_WRITES,
        ),
        // Entry 6a — `capco/sbu-nf-transmutes-on-classified-contact`.
        // §H.9 p178 (SBU-NF Commingling Rule(s) Within a
        // Portion): "The SBU-NF marking is conveyed in the
        // portion mark only if the commingled portion is
        // unclassified and there is no other NOFORN information
        // included in the portion. If there is other NOFORN
        // information in the commingled portion, the 'SBU'
        // marking is used and a NOFORN marking is added, e.g.,
        // (U//NF//SBU)." Class > U drops SBU-NF entirely; class
        // = U replaces SBU-NF with NOFORN + SBU.
        //
        // Monotonicity: mixed — shrinking on class > U;
        // mostly-additive on class = U. Sound under fixed
        // topological order.
        //
        // Predicate scans `CAT_DISSEM` (and the
        // `CanonicalAttrs.non_ic_dissem` field) for SBU-NF.
        // Same predicate-scan-vs-dataflow convention as
        // Entry 5 (see Entry 5 doc-comment); DISSEM excluded
        // from `reads` to avoid manufactured cycles against
        // entries 5 and 6b.
        //
        // Phase-3 axis-mapping pragmatic (plan §8 Q1): SBU/SBU-NF
        // live in `CanonicalAttrs.non_ic_dissem` but no
        // `CAT_NON_IC_DISSEM` CategoryId is exposed yet, so the
        // write axis is `CAT_DISSEM`. Phase D/E may add the
        // separate axis.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/sbu-nf-transmutes-on-classified-contact",
            "CAPCO-2016 §H.9 p178",
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E6A_READS,
            E6A_WRITES,
        ),
        // Entry 6b — `capco/les-nf-transmutes-on-classified-contact`.
        // §H.9 p185 (LES-NF Precedence Rules for Banner Line
        // Guidance): "When a
        // classified document contains portions of U//LES-NF,
        // the 'LES' marking is used in the banner line and the
        // NOFORN marking is applied as a Dissemination Control
        // Marking. For example: SECRET//NOFORN//LES." LES-NF
        // transmutes to NOFORN + LES; banner consolidates as
        // `[class]//NOFORN//LES`.
        //
        // Monotonicity: monotone-additive on the dissem axis
        // (NOFORN and LES both added; LES-NF dropped is the
        // input-side projection of the transmutation, not a
        // separate axis shrink). Sound under fixed topological
        // order.
        //
        // Predicate scans `CAT_DISSEM` (and the
        // `CanonicalAttrs.non_ic_dissem` field) for LES-NF.
        // Same predicate-scan-vs-dataflow convention as
        // Entry 5 / 6a; DISSEM excluded from `reads` to avoid
        // manufactured cycles against entries 5 and 6a.
        //
        // Phase-3 axis-mapping pragmatic (plan §8 Q1): same
        // CAT_DISSEM fold as Entry 6a.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/les-nf-transmutes-on-classified-contact",
            "CAPCO-2016 §H.9 p185",
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E6B_READS,
            E6B_WRITES,
        ),
    ]
}
