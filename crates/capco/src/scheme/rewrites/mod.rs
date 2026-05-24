// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` page-rewrite catalog.
//!
//! See [`build_page_rewrites`] for the full inventory and per-row authority.
//!
//! The catalog is split into per-pattern helper files; [`build_page_rewrites`]
//! concatenates them in group order. **Row order is load-bearing** — the
//! topological scheduler breaks ties on declaration order, so reordering
//! would silently shift the rewrite schedule.

use marque_scheme::PageRewrite;

use super::CapcoScheme;

mod noforn_clears;
mod pattern_a;
mod pattern_b;
mod pattern_c;
mod relido_clears;
mod supersession;
mod transmutation_stubs;

/// Construct CAPCO's `PageRewrite` table.
///
/// **30 rewrites, in seven groups** matching the per-pattern helper
/// files concatenated by [`build_page_rewrites`]. Group counts
/// (4 + 8 + 2 + 2 + 3 + 3 + 8 = 30) match the
/// [`EXPECTED_PAGE_REWRITES`] inventory pin at
/// `crates/capco/tests/post_4b_lattice_inventory_pin.rs`.
///
/// [`EXPECTED_PAGE_REWRITES`]: ../../../../../crates/capco/tests/post_4b_lattice_inventory_pin.rs
///
/// 1. **Pattern-A NOFORN-supremacy (4):** the §H.9 family —
///    `capco/{nodis,exdis}-implies-noforn` (§H.9 p174 / §H.9 p172) and
///    `capco/{sbu-nf,les-nf}-implies-noforn` (§H.9 p178 / §H.9 p185).
///    All four are wired predicates that fire today via
///    `scheme.project(Scope::Page, ...)`.
/// 2. **Pattern-C strip rows (8):** §H.6 / §H.8 / §H.9
///    classification-driven strips of UNCLASSIFIED-only controls
///    plus the §H.6 NOFORN-promotion siblings —
///    `capco/limdis-evicted-by-classified` (§H.9 p170),
///    `capco/sbu-evicted-by-classified` (§H.9 p176),
///    `capco/sbu-nf-evicted-by-classified` (#541; §H.9 p178), four
///    UCNI rows declared **promote-before-strip** so the NOFORN-
///    promotion predicate observes UCNI before the strip removes
///    it (`capco/{dod,doe}-ucni-{promotes-noforn-when-classified,
///    evicted-by-classified}` at §H.6 p116 / p118), and
///    `capco/fouo-evicted-by-classified` (§H.8 p134 classified
///    sub-clause).
/// 3. **Pattern-B structural FOUO-eviction (2):**
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
/// 5. **`noforn_clears` (3):** NOFORN-supersedes-FD&R rows at
///    page scope. `capco/noforn-clears-rel-to` (§D.2 Table 3 +
///    §H.8 p145) — `Contains(CAT_DISSEM, NOFORN)` + `Clear(CAT_REL_TO)`
///    (canonical worked example in `crates/capco/README.md`).
///    `capco/noforn-clears-fdr-family` (§D.2 Table 3 row 2 +
///    §H.8 p154 + §H.8 p157) — strips DISPLAY ONLY / RELIDO /
///    EYES tokens from `dissem_us` via FactRemove.
///    `capco/noforn-clears-display-only-to` (§H.8 p145 + §D.2 Table 3
///    rows 1-2) clears `attrs.display_only_to`, the country-list
///    sibling of `attrs.rel_to`. The three rows together close the
///    NOFORN-dominates-FD&R surface.
/// 6. **`relido_clears` (3):** RELIDO eviction by a stronger
///    dissem axis at page scope — Marque's subtractive resolution
///    of the §H.8 RELIDO incompatibility clauses.
///    `capco/display-only-clears-relido` (§H.8 p154 — DISPLAY
///    ONLY entry's RELIDO-incompatibility clause) requires
///    `satisfies(TOK_DISPLAY_ONLY)` +
///    `capco_category_contains(CAT_DISSEM, TOK_DISPLAY_ONLY)`
///    to recognize the canonical `display_only_to` parser axis;
///    `capco/orcon-clears-relido` (§H.8 p136 — *"May not be used
///    with RELIDO"*) and `capco/orcon-usgov-clears-relido`
///    (§H.8 p140 — same exclusion). All three rows use
///    `Contains(CAT_DISSEM, <dominator>)` triggers with
///    `Intent(FactRemove([TOK_RELIDO], Scope::Page))` actions
///    and empty `reads` (cycle-avoidance per the narrow-form
///    rule below — four siblings reading + writing CAT_DISSEM
///    would form a 3-row cycle with `noforn-clears-fdr-family`).
/// 7. **Transmutation stubs (8):** the transmutation roster.
///    Each declares a `Custom(never_fires)` trigger and a
///    `Custom(noop_action)` body — page roll-up does not yet drive
///    these through `scheme.project()`, so the trigger pins to
///    `false` and the action body is empty. The `reads` / `writes`
///    annotations are what the Kahn scheduler consumes to validate
///    dataflow ordering; the runtime semantics still live in the
///    hand-coded aggregator. A future change replaces the `Custom`
///    bodies with real predicates and transforms.
///
/// # `reads` semantics — narrow form
///
/// `reads` declares **true dataflow dependencies only**: axes
/// whose post-rewrite state this rewrite consumes from another
/// rewrite. Axes the trigger only pattern-matches against
/// (predicate-scan reads) are documented in the per-entry
/// doc-comment but excluded from the `reads` slice. Inflating
/// `reads` with predicate-scan axes manufactures false cycles in
/// the scheduler's dependency graph: the engine scheduler
/// (`crates/engine/src/scheduler.rs`) only skips *same-rewrite*
/// self-edges (`producer_idx == idx`), so two independent rewrites
/// that each read AND write the same axis produce a mutual edge in
/// both directions and abort `Engine::new` with `RewriteCycle`.
/// Predicate-scan axes go in the doc-comment with the explicit phrase
/// "predicate scans X for Y"; if a real dataflow dependency on a
/// documented predicate-scan axis is later discovered, the
/// corresponding `reads` annotation can be re-introduced and the
/// scheduler's DAG will reflect it.
///
/// The eight transmutation stubs (in topological order):
///
/// 1. `capco/frd-sigma-consolidates-into-rd-sigma` (§H.6 p113) —
///    AEA-only, independent.
/// 2. `capco/fgi-rollup-on-us-contact` (§H.7 p122) — bare-FGI
///    rollup on US-class contact.
/// 3. `capco/fgi-restricted-rollup-on-us-contact` (§H.7 p122) —
///    bare-FGI-R contact rolls FGI list (class lift is
///    parser-side).
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
/// Declaration order is one valid total ordering of the rewrite
/// vector. It is **not** the scheduler's topological order —
/// `noforn-clears-rel-to` reads `CAT_DISSEM` which the transmutation
/// entries write, so the scheduler orders it after them. `Engine::new`
/// runs Kahn's algorithm at construction; runtime execution order is
/// determined by the scheduler, not by this `Vec` order.
///
/// # Declaration order
///
/// [`build_page_rewrites`] concatenates the per-pattern helper outputs
/// in group order: Pattern A first (NODIS, EXDIS, SBU-NF,
/// LES-NF), then Pattern C (LIMDIS, SBU, SBU-NF, DOD UCNI
/// promote+strip, DOE UCNI promote+strip, FOUO), then Pattern B
/// (classification-evicts-fouo, non-fdr-control-evicts-fouo),
/// then the #552 same-axis supersession pair (sbu-nf-supersedes-
/// sbu, les-nf-supersedes-les), then the NOFORN clear-rows
/// (noforn-clears-rel-to, noforn-clears-fdr-family,
/// noforn-clears-display-only-to), then the #559 / #618 RELIDO
/// clear-rows (display-only-clears-relido, orcon-clears-relido,
/// orcon-usgov-clears-relido — placed after `noforn_clears` so
/// `noforn-clears-fdr-family` strips RELIDO first on
/// NOFORN-bearing pages and these rows only fire on the
/// non-NOFORN-but-RELIDO-incompatible cases that motivated #559),
/// then the eight Phase-3 transmutation stubs.
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
    // DISPLAY ONLY / ORCON / ORCON-USGOV → RELIDO eviction rows
    // (#559, #618). Placed after `noforn_clears` because the
    // `capco/noforn-clears-fdr-family` row strips RELIDO when NOFORN is
    // present — running it first means these rows are no-ops on
    // NOFORN-bearing pages and only fire on the non-NOFORN-but-RELIDO-
    // incompatible cases. The DISPLAY ONLY row requires
    // `satisfies(TOK_DISPLAY_ONLY)` to recognize the canonical
    // `display_only_to` parser axis.
    out.extend(relido_clears::relido_clears_rows());
    out.extend(transmutation_stubs::transmutation_stub_rows());
    out
}
