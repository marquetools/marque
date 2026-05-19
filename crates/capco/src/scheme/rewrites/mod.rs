// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` page-rewrite catalog (PR 4b-C; PR 3c.B Sub-PR 8.F / 8.F.2;
//! PR 4b-A). Lifted from the monolithic `scheme.rs` per the issue #466
//! split plan (`claudedocs/refactor-466/split_proposal.md`, Risk 1 Option 2).
//!
//! See [`build_page_rewrites`] for the full inventory and per-row authority.
//!
//! Stage 2 PR A (issue #466) sub-split this leaf into per-pattern files
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`). Each helper
//! returns its rows in the same order they appear in the pre-split
//! catalog; [`build_page_rewrites`] concatenates them in the original
//! group order. **Row order is load-bearing** — the topological
//! scheduler breaks ties on declaration order, so reordering would
//! silently shift the rewrite schedule.

use marque_scheme::PageRewrite;

use super::CapcoScheme;

mod noforn_clears;
mod pattern_a;
mod pattern_b;
mod pattern_c;
mod supersession;
mod transmutation_stubs;

/// Construct CAPCO's `PageRewrite` table.
///
/// **27 rewrites, in seven groups** (post-#552; post-PR-4b-D.2
/// Copilot R1 #2; PR 4b-C landed groups 2 + 3 as Pattern-C +
/// Pattern-B declarative rows owning the §H.6 / §H.8 / §H.9
/// strip-plus-promote semantics; PR 4b-A landed group 5; PR 3c.B
/// Sub-PR 8.F / 8.F.2 landed group 1; PR 4b-D.2 added
/// `capco/noforn-clears-display-only-to` to group 5; #541 added
/// `capco/sbu-nf-evicted-by-classified` to group 2; #552 added
/// the new group 4 same-axis supersession pair):
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
/// 4. **#552 same-axis supersession (2):**
///    `capco/sbu-nf-supersedes-sbu` (§H.9 p178 SBU NOFORN
///    Precedence Rules — "When a document contains both SBU-NF
///    and SBU portions, SBU NOFORN supersedes SBU in the
///    banner line") + `capco/les-nf-supersedes-les` (§H.9 p185
///    LES NOFORN banner-form heading + Notional Example Page 1
///    — LES-NF compound carries the LES family marker on the
///    unclassified banner so bare LES is redundant on
///    co-presence). Both rows: predicate scans
///    `CAT_NON_IC_DISSEM`, FactRemove writes
///    `CAT_NON_IC_DISSEM`; predicate-scan kept OUT of `reads`
///    per the narrow-form rule below.
/// 5. **Active wired rows (1):** `capco/noforn-clears-rel-to`
///    (`Contains` predicate + `Clear` action). Cited at §D.2
///    Table 3 + §H.8 p145. First PageRewrite to land in the
///    catalog; canonical worked example in
///    `crates/capco/README.md`.
/// 6. **DISPLAY-ONLY / FD&R-family (2):**
///    `capco/noforn-clears-fdr-family` (strips DISPLAY ONLY /
///    RELIDO / EYES tokens from `dissem_us`) at §D.2 Table 3
///    row 2 + §H.8 p154 + §H.8 p157, plus
///    `capco/noforn-clears-display-only-to` (PR 4b-D.2 Copilot R1
///    #2 — clears `attrs.display_only_to`, the country-list
///    sibling of `attrs.rel_to`) at §H.8 p145 + §D.2 Table 3
///    rows 1-2. The two rows together close the parallel REL TO /
///    DISPLAY ONLY axes.
/// 7. **Phase-3 transmutation stubs (8):** the §3.4.1 / §3.4.3
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
/// # Declaration order (post-split)
///
/// Stage 2 PR A (issue #466) sub-split this leaf into per-pattern
/// helper files; [`build_page_rewrites`] concatenates the helper
/// outputs in the same order the rows appear in the pre-split
/// monolithic catalog: Pattern A first (NODIS, EXDIS, SBU-NF,
/// LES-NF), then Pattern C (LIMDIS, SBU, SBU-NF, DOD UCNI
/// promote+strip, DOE UCNI promote+strip, FOUO), then Pattern B
/// (classification-evicts-fouo, non-fdr-control-evicts-fouo),
/// then the #552 same-axis supersession pair (sbu-nf-supersedes-
/// sbu, les-nf-supersedes-les), then the active NOFORN clear-rows
/// (noforn-clears-rel-to, noforn-clears-fdr-family,
/// noforn-clears-display-only-to), then the eight Phase-3
/// transmutation stubs.
///
/// [`CategoryPredicate::Contains`]: marque_scheme::CategoryPredicate::Contains
/// [`CategoryAction::Clear`]: marque_scheme::CategoryAction::Clear
/// [`Engine::lint`]: marque_engine::Engine::lint
pub(crate) fn build_page_rewrites() -> Vec<PageRewrite<CapcoScheme>> {
    let mut out = pattern_a::pattern_a_rows();
    out.extend(pattern_c::pattern_c_rows());
    out.extend(pattern_b::pattern_b_rows());
    out.extend(supersession::supersession_rows());
    out.extend(noforn_clears::noforn_clears_rows());
    out.extend(transmutation_stubs::transmutation_stub_rows());
    out
}
