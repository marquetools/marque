
**Date:** 2026-05-18
**Author:** PR 4b-E preflight architect
**Status:** Plan draft for PM review — no source edits performed. Reviewer must resolve §10 open questions before implementation begins.

---

## 1. Scope statement

PR 4b-E retires `PageContext`'s `expected_*` accessor surface, the `render_expected_banner` family, and `PageContext::project()` now that PR 4b-D.2 flipped `Engine::lint`'s hot path to drive page aggregation through `scheme.project(Scope::Page, ...)` and PR 4b-D.3 migrated S007 to `ProjectedMarking`. The PR also retires `crates/capco/src/scheme/actions/page_context.rs::page_context_to_attrs` (parallel dead-code helper post-4b-D.2). What survives in `marque-ism` is a trimmed shim — `Default` / `Clone` / `new` / `add_portion` / `portion_count` / `is_empty` / `portions()` plus the free `sar_sort_key` helper — needed by W004 and S005 (per-portion `CanonicalAttrs` consumers) and by the engine's `PageContext` accumulator that backs `scheme.project_from_page_context`. **Full `PageContext` removal stays deferred** to T069 / consolidated-plan PR-6c. Because the lattice path in `crates/capco/src/scheme/marking.rs::join_via_lattice_with_context` still calls `tmp_ctx.expected_sci_controls()` / `expected_fgi_marker()` / `expected_declass_exemption()` / `expected_non_ic_dissem()` / `expected_display_only()` as **residue-axis accessors**, those callsites must migrate to inlined / lattice-native logic *as part of this PR* or the deletion cannot land. That migration is the load-bearing structural commit and the largest single piece of work in 4b-E.

---

## 2. Method-by-method deletion inventory

The 17 `expected_*` methods at `crates/ism/src/page_context.rs` plus the supporting surface. Line numbers verified 2026-05-18 against worktree HEAD; re-grep at implementation time. "Migration target" describes where each caller's logic moves; "Status" notes whether the method itself dies or survives.

| Method (`page_context.rs`) | Callers found (production + test) | Migration target | Post-deletion status |
|---|---|---|---|
| `expected_classification()` @306 | `marking.rs` (via `tmp_ctx`); `actions/page_context.rs` (dead-code post-4b-D.2); `proptest_page_context.rs`; `rollup_golden.rs`; `scheme_equivalence.rs`; internal tests in `page_context.rs` `#[cfg(test)] mod tests` | Replace with inline `portions.iter().filter_map(|a| a.classification.as_ref().map(|c| c.effective_level())).max()` at each surviving callsite, OR (preferred) read `ProjectedMarking.classification.as_ref().map(|c| c.effective_level())` where the caller has a `ProjectedMarking` | **Deleted** |
| `is_solely_nato_classified()` @353 | `page_context.rs` internal tests | Twin already lives on `ProjectedMarking` (`crates/ism/src/projected.rs:271`); S007 migrated to it at PR 4b-D.3 (`rules.rs:3650`); zero production consumers remain | **Deleted** (twin survives) |
| `expected_sci_controls()` @362 | `marking.rs` line 462 (lattice path — `tmp_ctx.expected_sci_controls()`); `actions/page_context.rs:43` (dead); `proptest_page_context.rs`; `rollup_golden.rs`; `scheme_equivalence.rs:80, 1226, 1261`; internal tests | `marking.rs:462` already builds `SciSet` from `sci_markings_concat` and calls `sci_set.to_markings()` on line 456 — the parallel `tmp_ctx.expected_sci_controls()` call exists ONLY to populate the back-compat `sci_controls: Box<[SciControl]>` flat-CVE projection field. Replace with a free `sci_controls_from_markings(&out.sci_markings)` helper in `marque-capco::lattice` (project `SciMarking[]` → flat `Vec<SciControl>` by reading `m.system_published_cve()` per marking) so the lattice path stops touching PageContext for this axis | **Deleted** |
| `expected_sci_markings()` @384 | `actions/page_context.rs:44`; `rollup_golden.rs`; `scheme_equivalence.rs`; internal tests | Already replaced by `SciSet::to_markings()` in `marking.rs:456`. Test-only callers migrate to `SciSet::from_markings(...).to_markings()` direct calls | **Deleted** |
| `expected_sar_marking()` @443 | `actions/page_context.rs:45`; `rollup_golden.rs`; internal tests | Already replaced by `SarSet::join` loop @ `marking.rs:467-472`. Test callers migrate to `SarSet::from_marking(...).join(...).to_marking()` | **Deleted** |
| `expected_dissem_us()` @532 | `actions/page_context.rs:51`; `proptest_page_context.rs:184`; `rollup_golden.rs` (many); `scheme_equivalence.rs:205`; `pattern_a_nodis_exdis_page_context_alignment.rs`; internal tests | Already replaced by `DissemSet::from_attrs_iter(portions)` @ `marking.rs:626` (plus `with_noforn_injected` cross-axis overlay). Test callers migrate to `DissemSet::from_attrs_iter(...).into_boxed_slice()` direct call | **Deleted** |
| `expected_dissem_nato()` @726 | `actions/page_context.rs:52`; `proptest_page_context.rs:232`; internal tests | Already replaced by `NatoDissemSet::from_attrs_iter(portions)` @ `marking.rs:627`. Test callers migrate similarly | **Deleted** |
| `expected_rel_to()` @756 | `actions/page_context.rs:53`; `proptest_page_context.rs:253`; `rollup_golden.rs:303, 323`; `scheme_equivalence.rs:121, 202`; `pattern_a_nodis_exdis_page_context_alignment.rs`; `tetragraph_consolidation.rs:220`; **`rules.rs:2502` (S005 `analyze_uncertain_reduction`)**; internal tests | (a) Lattice path already replaced by `RelToBlock::from_attrs_iter(...)` @ `marking.rs:630`. (b) S005's read needs `RelToBlock`-equivalent intersection; recommendation: call `RelToBlock::from_attrs_iter(page_ctx.portions()).into_boxed_slice()` from S005 directly (the slice the rule needs is already what `RelToBlock` produces post-NOFORN-supersession check, and S005 owns its own NOFORN bail above). | **Deleted** |
| `expected_display_only()` @881 | `actions/page_context.rs:66`; `marking.rs:689` (`tmp_ctx.expected_display_only()` — lattice-path residue); `rollup_golden.rs` (none directly — the file uses `expected_dissem_us`); internal tests | Residue axis. The `marking.rs:689` callsite is the load-bearing one. Lift the §D.2 Table 3 rows 18-20/25-27 intersection logic — already documented at `page_context.rs:881-996` — into a new `marque-capco::lattice::DisplayOnlyBlock` lattice type parallel to `RelToBlock`; the doc-comment at `marking.rs:671-680` literally says "A dedicated `DisplayOnlyBlock` lattice (parallel to RelToBlock) is queued for the same PR-cycle as the PageContext deletion." 4b-E **is** that PR-cycle | **Deleted** |
| `expected_declassify_on()` @1000 | `actions/page_context.rs:71`; `rollup_golden.rs`; internal tests | Already replaced by `DeclassifyOnLattice::from_attrs_iter(portions).into_inner()` @ `marking.rs:653`. Test callers migrate similarly | **Deleted** |
| `expected_declass_exemption()` @1015 | `actions/page_context.rs:72`; `marking.rs:654` (`tmp_ctx.expected_declass_exemption()` — residue); **`wasm/src/lib.rs:1296` (CAB)**; `scheme_equivalence.rs` (none — `page_context_lattice_parity.rs:64` uses it); `page_context_lattice_parity.rs:64` | Residue axis carrying explicit Phase 3 TODO. Options for the lattice-path residue: (a) inline the last-observed `portions.iter().filter_map(\|a\| a.declass_exemption).next_back()` at the callsite (cheap, mirrors today's implementation); (b) add a `DeclassExemptionLattice::from_attrs_iter` constructor returning the last-observed exemption (cleaner). Recommend (b) for symmetry with sibling lattice types. WASM CAB callsite — see §3 Decision 1 | **Deleted** |
| `is_classified()` @1034 | `wasm/src/lib.rs:1277` (CAB-only); internal tests | The implementation is a 2-line predicate over `expected_classification()`. Re-implementable inline at the WASM callsite as `!page_marking.classification.as_ref().map(\|c\| c.effective_level()).is_some_and(\|l\| l == Classification::Unclassified)` — or trivially `page_marking.classification.as_ref().is_some_and(\|c\| c.effective_level() > Classification::Unclassified)`. See §3 Decision 1 for the WASM CAB strategy | **Deleted** |
| `expected_aea_markings()` @1052 | `actions/page_context.rs:46`; `rollup_golden.rs:53, 89, 116`; internal tests | Already replaced by `AeaSet::from_markings(&aea_markings_concat).to_markings()` @ `marking.rs:478`. Test callers migrate similarly | **Deleted** |
| `expected_fgi_marker()` @1193 | `actions/page_context.rs:47`; `marking.rs:616` (`tmp_ctx.expected_fgi_marker()` — non-G-4b branch residue); `rollup_golden.rs:425, 442`; internal tests | The `marking.rs:616` callsite is residue used **only** when not solely-non-US (G-4b branch). The PageContext implementation walks portions and unions NATO/JOINT/FGI-class trigraphs + per-portion `fgi_marker`. The lattice path already has `FgiSet::from_marker` accumulating `fgi_acc` from per-portion markers; the missing piece is the classification-derived producer union. Lift that into `FgiSet::from_attrs_iter(portions)` (new constructor; mirrors `RelToBlock`/`DissemSet` shape) and replace `tmp_ctx.expected_fgi_marker()` with it. Test callers migrate similarly | **Deleted** |
| `expected_non_ic_dissem()` @1303 | `actions/page_context.rs:38`; `marking.rs:668` (`tmp_ctx.expected_non_ic_dissem()` — residue); `marking.rs:689` indirect via `needs_nf`; `rules.rs:2493` (S005); `rules.rs:7925`; `rollup_golden.rs:134, 152`; `proptest_page_context.rs`; `pattern_a_*` test; internal tests | Returns `(Vec<NonIcDissem>, bool needs_nf)`. The `bool` (NOFORN-injection flag) is consumed at `marking.rs:668` and inside `expected_display_only` / `expected_rel_to`. Lift into a `NonIcDissemSet::from_attrs_iter(portions) -> (Box<[NonIcDissem]>, bool)` lattice helper; mirror semantics exactly per CAPCO-2016 §H.9 p174 (NODIS) / p172 (EXDIS) / p178 (SBU-NF) / p185 (LES-NF). S005's `rules.rs:2493` read migrates to call this helper directly | **Deleted** |
| `render_expected_banner()` @1369 | `wasm/src/lib.rs:1181` (`compute_banner_native`); `rollup_golden.rs:350, 389`; internal tests | This is the banner-string renderer, NOT a projection. Two valid paths: (a) inline at the WASM `compute_banner_native` callsite by calling `scheme.render_canonical(&CapcoMarking::new(projected_attrs), Scope::Page, ...)` — the canonical replacement; (b) preserve a private renderer helper inside `marque-capco`. The trait-level `MarkingScheme::render_canonical(scope=Scope::Page, ...)` is the right destination per the consolidated plan and per the doc-comment at `marking_scheme_impl.rs:280-286` (byte-identity contract with `render_banner` already pinned). | **Deleted** — WASM callsite migrates to `scheme.render_canonical(..., Scope::Page, ...)` |
| `project()` @243 | `rules.rs:9105` (test driver); commented reference at `engine.rs:4271, 4193` | Already retired from the engine hot path at PR 4b-D.2. Test driver migrates to `CapcoScheme::project_from_page_context` + `ProjectedMarking::from_canonical` — see §3 Decision 2 | **Deleted** |
| `expand_tetragraph` (private helper, ~line 1611) | None — duplicate of `crates/capco/src/vocab.rs:87` `expand_tetragraph` | The CAPCO copy is the live one (`lattice.rs` already calls `marque_capco::vocab::expand_tetragraph`). The page_context.rs copy is dead | **Deleted** (dead duplicate) |
| `render_sci_markings_block` / `render_sar_block` (private helpers under `render_expected_banner`) | Internal to `render_expected_banner` | Dies with `render_expected_banner`. The CAPCO `crates/capco/src/render.rs` per-axis renderers are the equivalents on the surviving path | **Deleted** |
| **Internal `#[cfg(test)] mod tests` block @1622-3617** | Self-contained | See §6 below — bulk deletion with selective rewrites | **Deleted with carve-outs** |
| **Survivors** | | | |
| `Default`, `Clone`, `new`, `add_portion`, `portion_count`, `is_empty`, `portions()` @156-294 | Engine's `PageContext` accumulator (`engine.rs:791, 1717, 1719, 1750`); W004 (`rules.rs:5153`); S005 (`rules.rs:2444, 2486`); test fixtures | Survive untouched. These are the trimmed shim the PR scope statement names | **Kept** |
| Free `sar_sort_key` @84 | `crates/capco/src/rules.rs:113, 4251, 8454-8470`; `crates/capco/src/lattice.rs` 6 sites | See §3 Decision 4. Move to its own module is the recommendation | **Kept (and recommended to relocate)** |

The `marque-ism::PageContext` field surface (`portions: Vec<CanonicalAttrs>`) is unchanged — the type is still the engine's per-page accumulator.

---

## 3. Decision points

### Decision 1: WASM CAB strategy

The deferral marker at `crates/wasm/src/lib.rs:1280-1295` (in `generate_cab_native`) names `expected_declass_exemption` as the load-bearing CAB-only read that PR 4b-E must resolve. The wider CAB build also reads `is_classified()` (`lib.rs:1277`) and calls `render_expected_banner()` from `compute_banner_native` (`lib.rs:1181`).

**CAB-only attrs in scope.** Per the deferral marker, the projection-excluded fields are `declass_exemption`, `classified_by`, `derived_from`, `token_spans`. Verified: `ProjectedMarking` (`crates/ism/src/projected.rs:60-128`) excludes all four by design ("a projected marking is a banner / page aggregate, not a CAB", line 27-30). `is_classified()` is NOT CAB-only — it's a derived predicate over `classification`, which `ProjectedMarking` DOES expose.

**Option (a) — Inline per-portion accumulator at the WASM callsite.** The `generate_cab_native` body already walks parsed candidates in a loop, accumulating `found_declass_date` and `found_declass_exemption` from per-portion `attrs` directly (`lib.rs:1253-1266`). Adding a parallel "if `declass_exemption is None`, fall through to `portions.iter().filter_map(|a| a.declass_exemption).next_back()`" using a `Vec<CanonicalAttrs>` accumulator the function already populates via `page_context.add_portion(attrs)` (`lib.rs:1268`) is a 3-line addition. `is_classified` becomes `page_marking.classification.is_some_and(|c| c.effective_level() > Classification::Unclassified)` against a `ProjectedMarking` built via `marque_engine::project_page_marking` (or the CAPCO scheme directly). `compute_banner_native` migrates to `scheme.render_canonical(&CapcoMarking::new(projected_attrs), Scope::Page, ...)`.

  - **Pros:** zero new types, zero new crate surface, no cross-PR coupling. Constitution Principle II "minimize what Marque holds" satisfied — the WASM module already owns the portion vec for the duration of CAB construction. Adding a separate `CabProjection` type would just be a typed wrapper around the same data.
  - **Cons:** the `last-observed` semantic for `declass_exemption` becomes inline at a non-marque-ism callsite. If the (currently-deferred) Phase 3 fix that switches to "exemption with longest default retention duration" lands later, the WASM callsite has to be updated in parallel — or the helper migrates back into marque-ism / marque-capco at that point. Acceptable: this is a single 5-line callsite and the parallel migration is mechanical.
  - **WASM binary size:** smallest of the three options. No new types, no new trait bodies. The `is_classified()` migration shaves the indirect call.

**Option (b) — `CabProjection` type alongside `ProjectedMarking`.** A new type in `marque-ism` exposing `declass_exemption: Option<DeclassExemption>` + `classified_by: Option<Box<str>>` + `derived_from: Option<Box<str>>` (the CAB-bound fields). Constructor consumes `&[CanonicalAttrs]` and projects under `Scope::Document` semantics. Threads through `marque-engine` symmetrically to `ProjectedMarking`.

  - **Pros:** symmetric with `ProjectedMarking`; future Phase-J `DecisionRecord` / Phase-K `CleaningRecord` per Constitution V Principle V can target it cleanly; survives an eventual `Scope::Document` projection cutover; aligns with the consolidated plan's `Scope` enum.
  - **Cons:** cross-PR coupling — `CabProjection` properly belongs alongside a `Scope::Document` projection in `marque-scheme`, which is not in 4b-E's authorization. Building it now risks landing a type whose contract drifts before its consumer matures. The deferral marker explicitly enumerates (a) and (b) as the future PR's choice — not a 4b-E commitment.
  - **WASM binary size:** marginally larger (one new struct + constructor in the WASM-safe set). Acceptable.

**Phase-J / Phase-K alignment.** Constitution V Principle V says "Permitted identifiers in audit output are: token canonicals, category IDs, span offsets, digests (BLAKE3 of content), posterior scalars, and enumerated feature labels." `classified_by` and `derived_from` are free-form text fields — option (b)'s wrapper struct does NOT make them G13-compliant; both options expose the same fields and rely on the WASM CAB build being a CALLER-facing surface that delivers content back to the caller (not an audit-record surface). Phase J/K invariants do not bear on this decision today.

**Recommendation: Option (a) — inline at the WASM callsite.** YAGNI-aligned. Option (b)'s type is **not yet justified by any consumer outside `generate_cab_native`** — and the only sister consumer that might justify it (a `Scope::Document` projection cutover) is post-PR-10 territory per the consolidated plan. Building it now would either land it with no test coverage beyond the WASM build (premature), or wait for the document-projection cutover (same as deferring). The deferral marker stays in place pointing at a follow-up issue that lands when the consumer arrives.

  - **Cross-PR coupling:** none. Option (a) reaches a stable end state. Option (b) defers to a follow-up.

  - **Open question for PM:** confirm option (a) — see §10 OQ-1.

### Decision 2: Test driver in `rules.rs:9105`

The test driver `run(source)` in `crates/capco/src/rules.rs:9050-9131` builds a `page_context` accumulator and at line 9105 calls `page_context.project()` to construct `ctx_page_marking`. The driver's comment claims it "mirrors the engine's lazy projection for the test driver." **That claim is stale post-PR-4b-D.2** — the engine no longer calls `page_context.project()`; it calls `project_page_marking(&self.scheme, &page_context)` (`engine.rs:1219`) which routes through `CapcoScheme::project_from_page_context` + `ProjectedMarking::from_canonical`.

Verified: `run` is `pub(crate) fn run(source: &[u8]) -> Vec<Diagnostic<CapcoScheme>>` in a `#[cfg(test)] mod test_support` block. It has access to `let scheme = CapcoScheme::new()` if invoked — `CapcoScheme::new()` is a `pub fn` (per the trait-impl module).

**Rewrite plan.** Replace line 9105 with:

```rust
// PR 4b-E: mirror the post-PR-4b-D.2 engine path exactly. The engine
// drives page-marking aggregation through CapcoScheme::project_from_
// page_context, NOT PageContext::project (retired in this PR). See
// crates/engine/src/engine.rs::project_page_marking for the production
// shape this mirrors.
let ctx_page_marking = if parsed.kind != MarkingType::Portion && !page_context.is_empty() {
    let scheme = CapcoScheme::new();  // or hoist outside the loop
    let projected_attrs = scheme.project_from_page_context(&page_context);
    Some(Arc::new(ProjectedMarking::from_canonical(projected_attrs)))
} else {
    None
};
```

Hoist `let scheme = CapcoScheme::new()` above the candidate loop — `CapcoScheme::new()` does non-trivial setup (constraint catalog, page-rewrite scheduler, etc.) and constructing once per portion would distort the test driver's profile. The engine itself caches `self.scheme` for the same reason.

  - **Open question for PM:** the `pub(crate) fn run` lives in a `#[cfg(test)]` block; the test driver is only used by the in-file `#[cfg(any())]`-gated tests (the legacy `FixProposal`-bearing block per `rules.rs:5222`). It is also called from `lint_banner` / `lint_portion` (`rules.rs:9124, 9128`) which are themselves `pub(crate)` in the test block. **It looks like the driver may have no surviving callers post-PR-3c.B.** Confirm whether to (a) rewrite as above (preferred — keeps it useful for future tests), or (b) delete it entirely under the same retirement logic as the `#[cfg(any())]` test block — see §10 OQ-2.

### Decision 3: Internal `page_context.rs` unit tests

The `#[cfg(test)] mod tests` block spans roughly lines 1622-3617 (verify range with `grep -n "^mod tests" page_context.rs` before edit). The tests exercise every `expected_*` method and `render_expected_banner` across ~120 `#[test]` functions.

**Sub-decision (a) — what surviving semantic coverage do these tests carry that the parity gate (`crates/capco/tests/page_context_lattice_parity.rs`, 74 fixtures + 16+ `project_via_scheme` declarative-row fixtures) does NOT already cover?**

Verified by category:

- **Classification roll-up:** parity gate has fixtures for OC-USGOV (6), RELIDO (4), REL TO (4), JOINT (4), classification max (2), NOFORN (2), Pattern-B/C (16+). Internal tests cover the same algebraic surface against `expected_classification`. The parity gate is strictly stronger because it compares THREE paths (PageContext + lattice + scheme). **Coverage delta: zero new semantic coverage in internal tests.**

- **SCI roll-up:** parity gate has indirect coverage via classification fixtures but no dedicated SCI fixtures. Internal tests at `page_context.rs:1783-2956` cover SCI markings union, compartment merging, numeric-before-alpha ordering, canonical-enum population, structural banner rendering. **Coverage delta: SCI roll-up semantics.** Migration target: `crates/capco/tests/sci_set_lattice_laws.rs` (new file or extend `category_lattice_laws.rs`), testing `SciSet::from_markings(...).to_markings()` and the canonical-enum projection — same inputs, same expected outputs.

- **SAR roll-up:** internal tests at `page_context.rs:2971-3085` cover empty case, single program, multi-portion merge, compartment merge, numeric-before-alpha. **Coverage delta: SAR roll-up semantics.** Migration target: extend `category_lattice_laws.rs` with `SarSet::from_marking(...).join(...).to_marking()` tests.

- **AEA roll-up:** internal tests at `page_context.rs:1949-2095` cover RD/FRD/TFNI/UCNI/ATOMAL union, SIGMA aggregation, UCNI classified-strip transitional behavior. **Coverage delta: AEA roll-up semantics + UCNI transition test.** Migration: extend `category_lattice_laws.rs` with `AeaSet::from_markings(...).to_markings()` tests; the UCNI transitional test should be moved to the parity gate (PR 4b-C already pins the post-strip semantic at `pattern_c_dod_ucni_classified_strip_promotes_noforn`).

- **Dissem / RELIDO / OC-USGOV / FOUO:** internal tests at `page_context.rs:2426-2670` cover the supersession overlays. **Coverage delta: largely redundant with parity gate**, but the parity-gate fixtures compose into single-output assertions; the internal tests assert individual overlay invariants. Migration: lift overlay-specific tests into `crates/capco/tests/dissem_set_lattice_laws.rs` or extend `category_lattice_laws.rs`.

- **REL TO + DISPLAY ONLY:** internal tests at `page_context.rs:3163-3617` cover §D.2 Table 3 rows 11/16-20/25-27. Parity gate covers row 9, 21, 23 via tetragraph fixtures. **Coverage delta: rows 11/16-20/25-27.** Migration: extend `crates/capco/tests/category_lattice_laws.rs` (or new `rel_to_display_only_laws.rs`) with `RelToBlock::from_attrs_iter` + the new `DisplayOnlyBlock::from_attrs_iter` fixtures.

- **FGI marker:** internal tests at `page_context.rs:2182-2222` cover source-concealed supersession and acknowledged union. **Coverage delta: FGI source-concealed behavior.** The parity gate covers G-4b/G-4c (FGI synthesis on solely-non-US pages). Migration: extend `category_lattice_laws.rs` with `FgiSet::from_attrs_iter(...)` tests (the new constructor lifted in Decision §2 row for `expected_fgi_marker`).

- **`is_classified`:** internal tests at `page_context.rs:1933-1947`. **Coverage delta: 2-line predicate.** Migration: extend `projected.rs` `#[cfg(test)] mod tests` with `is_classified_via_projected` test using the same fixtures.

- **`is_solely_nato_classified`:** internal tests at `page_context.rs:1665-1739`. **Coverage delta: zero — twin tests already exist in `projected.rs:478-519` covering the same 4 invariants.** Just delete the page_context.rs side.

- **`render_expected_banner`:** internal tests at `page_context.rs:2670-3613` cover banner serialization across SCI / SAR / DISPLAY ONLY / classification axes. **Coverage delta: byte-level banner output.** Migration: the trait's `render_canonical(scope=Scope::Page)` is the destination; its byte-identity contract with the legacy `render_banner` is already pinned per the doc-comment at `marking_scheme_impl.rs:280-286`. Lift the test bodies to `crates/capco/tests/render_canonical_default_chain.rs` (the file the doc-comment names) — the migration is mechanical (input portions → projected attrs → render_canonical → expected banner string).

**Sub-decision (b) — for each surviving test, what API to target?** All migrations target `marque-capco::lattice::*::from_attrs_iter(...)` + the trait-level `MarkingScheme::project` and `MarkingScheme::render_canonical`. The lattice types and the scheme trait are the post-4b-E source of truth; tests written against PageContext were always (per `page_context.rs:103-118`) a transitional shape.

**Sub-decision (c) — coverage delta on the surviving PageContext shim.** The shim retains 7 methods: `Default::default`, `Clone::clone`, `new`, `add_portion`, `portion_count`, `is_empty`, `portions`. Currently no internal test exercises these as isolated units (they're exercised transitively through `expected_*` tests). New tests owed for ≥80% coverage on the shim:

  1. `default_produces_empty_with_default_capacity` — pin issue #430's pre-size invariant
  2. `clone_preserves_default_capacity_invariant` — pin the manual `Clone` impl at `page_context.rs:164-188`
  3. `add_portion_grows_portions_in_document_order` — pin `add_portion`'s ordering contract
  4. `portion_count_matches_add_portion_calls` — basic counter sanity
  5. `is_empty_after_new_then_false_after_add` — basic predicate sanity
  6. `portions_borrows_slice_in_document_order` — pin `portions()`'s borrow contract

All six fit in <50 lines combined. Land them in a `#[cfg(test)] mod shim_tests` block inside the trimmed `page_context.rs`.

### Decision 4: `sar_sort_key` location

`sar_sort_key` is a 9-line free function (`page_context.rs:84`). External callers: `crates/capco/src/lattice.rs` (6 sites — SCI compartment / SAR program / sub-compartment sort), `crates/capco/src/rules.rs` (1 use site + 4 tests). Pre-PR-4b-E this helper sits inside a `page_context` module that is "page-context's helper" by convention but is structurally an orthogonal sort-key.

**Recommendation: move to `crates/ism/src/sar_sort.rs` (new top-level module in `marque-ism`).** Re-export from `marque-ism::lib.rs` to preserve the public API at `marque_ism::sar_sort_key`. Reasons:

1. Future T069 (full PageContext removal) becomes a single-file deletion instead of "delete page_context.rs except for this one helper that has 6 external callers." The cleaner the trimmed shim, the easier T069 lands.
2. `sar_sort_key` is domain-neutral over `&str` — it has zero coupling to `PageContext` semantics. Its current location is historical (the SAR PR added it where SAR roll-up was implemented).
3. Cost: one `pub use` re-export to preserve `marque_ism::sar_sort_key` callers, plus the new module file. No semantic change.

Alternative: leave it inline in the trimmed `page_context.rs`. **Rejected** because it commits the shim to outliving the file by name, complicating T069.

### Decision 5: Parity gate disposition

`crates/capco/tests/page_context_lattice_parity.rs` is currently a 2500+-line gate comparing three projection paths:

- `project_via_page_context` (calls `PageContext` `expected_*` accessors directly @ line 48-79)
- `project_via_lattice` (calls `CapcoMarking::join_via_lattice` @ line 81-83)
- `project_via_scheme` (calls `scheme.project(Scope::Page, ...)` @ line 97-101)

Post-PR-4b-D.2, `project_via_scheme` is the production reference; `project_via_page_context` is a legacy comparison. Post-PR-4b-E, **`project_via_page_context` no longer compiles** (every `expected_*` method it calls is gone).

**Plan:**

1. **Delete `project_via_page_context`** (lines 48-79). All 45+ `assert_byte_identity(..., project_via_page_context(&portions), project_via_lattice(&portions), &[])` calls become `assert_byte_identity(..., project_via_lattice(&portions), project_via_scheme(&portions), &[])` — comparing the lattice path against the scheme path (which composes lattice + closure + page-rewrites). This is the meaningful comparison post-4b-E: the lattice path verifies join semantics; the scheme path verifies the full pipeline including rewrite catalog.
2. **Documented divergences re-validate.** The three documented active divergences (G-3 pure-NATO, joint_unanimous_two_portions, joint_single_portion_no_us) compared PageContext-vs-lattice. Post-4b-E both compared sides converge (no PageContext side exists). **Each of these three fixtures should converge to byte-identity** (G-3: lattice path and scheme path both preserve `Nato(_)`; joint cases: both preserve `Joint(_)`). If they don't converge, that's a real bug surfaced by the deletion — investigate.
3. **`project_via_scheme`-only fixtures (lines 1979+)** are unchanged — they assert the Pattern-B/C declarative-row strip behavior on `scheme.project` output.
4. **Rename the file** to `crates/capco/tests/lattice_vs_scheme_parity.rs`. The old name encoded the now-retired PageContext comparison.
5. **The §3 documented-divergence list in `crates/capco/CAPCO-CONTEXT.md` lines 261-282** needs an updated note: post-4b-E, parity is `project_via_lattice` ↔ `project_via_scheme`; the three former divergences collapse.

**Alternative considered:** delete the parity gate entirely, since the comparison it pins (PageContext vs lattice) goes away. **Rejected** because the `lattice` ↔ `scheme.project` comparison is the load-bearing regression catch for any future PageRewrite catalog edit — it pins that the declarative catalog produces the right post-lattice + closure behavior. Keeping the gate is consistent with consolidated-plan invariant I-12 ("`Scope::Page` projection is the source of truth").

### Decision 6: Constitution VII §IV engine-touch authorization

PR 4b-E touches:

- **`marque-ism`** — bulk deletion of `expected_*` machinery + `is_classified` + `render_expected_banner` + `project` + `expand_tetragraph` duplicate; possible `sar_sort_key` relocation.
- **`marque-wasm`** — `compute_banner_native` migrates to `scheme.render_canonical`; `generate_cab_native` inline the per-portion accumulator (option a).
- **`marque-capco`** — `scheme/marking.rs::join_via_lattice_with_context` migrates `tmp_ctx.expected_*` callsites to lattice-native helpers; `scheme/actions/page_context.rs` file deletion (entire module); `rules.rs:9105` test driver rewrite; `rules.rs:2493, 2502` S005 callsite migrations.
- **`marque-engine`** — comment-only references at `engine.rs:4193, 4271, 4271` updated; no logic changes (the production path already routes through `scheme.project_from_page_context` post-4b-D.2).
- **Test files** — `rollup_golden.rs`, `proptest_page_context.rs`, `scheme_equivalence.rs`, `pattern_a_nodis_exdis_page_context_alignment.rs`, `tetragraph_consolidation.rs`, `s004_audit_content_ignorance.rs`, `rules_us1.rs`, `page_context_lattice_parity.rs` migrate.

Constitution Principle VII §IV: "A scheme-adoption PR MUST NOT edit the engine crates (`marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`, `marque-ism`)." **This is NOT a scheme-adoption PR.** It is the closure of the engine refactor itself (consolidated plan PR 6 collapsed under the clean-break — see consolidated-plan §4 PR-6 row, line 335).

Within-006 bugfix-class precedent for engine-crate touches:

- **PR 4b-B Commit 2** (CLAUDE.md "Recent Changes"): "Two PageContext bugfixes landed atomically in Commit 2: OC-USGOV supersession ... and RELIDO observed-unanimity at banner roll-up." Both edited `marque-ism::page_context.rs` directly.
- **PR 4b-C Commit 5** (CLAUDE.md "Recent Changes"): "retired two imperative PageContext branches (FOUO Step 3 at `expected_dissem_us:594-599` + UCNI strip at `expected_aea_markings:1085-1093`)." Direct `marque-ism::page_context.rs` edit.
- **PR 4b-D.2 + 4b-D.3** (consolidated plan §4 PR-6 row): drove `Engine::lint` through `scheme.project(Scope::Page, ...)` and migrated S007 — `marque-engine`, `marque-ism`, and `marque-capco` edits in lock-step.

**Authorization argument for the PR description:**

> PR 4b-E closes the structural commitment of the consolidated plan's PR 6: retiring `PageContext`'s `expected_*` machinery now that the hot path has flipped at PR 4b-D.2. Constitution Principle VII §IV blocks **scheme-adoption** PRs from editing engine crates; this PR is the engine refactor itself, not a scheme adoption. The within-006 bugfix-class precedent is established by PR 4b-B Commit 2 (OC-USGOV + RELIDO PageContext bugfixes in `marque-ism`), PR 4b-C Commit 5 (FOUO + UCNI PageContext branch retirements in `marque-ism`), and PR 4b-D.2/.3 (Engine hot-path flip + S007 migration across `marque-engine`/`marque-ism`/`marque-capco`). Every edit here is in service of the same structural cleanup; no new scheme is being adopted. The PageRewrite catalog (the scheme surface) is unchanged.

### Decision 7: Helper dead-code review (walk-adjacent)

Beyond the inventoried 17 `expected_*` methods, the file contains:

- **`render_sci_markings_block`** (private fn under `render_expected_banner`) — dies with `render_expected_banner`.
- **`render_sar_block`** (private fn under `render_expected_banner`) — dies with `render_expected_banner`.
- **`expand_tetragraph`** (~line 1611, private) — dead duplicate per inventory table; `marque-capco::vocab::expand_tetragraph` is the live copy.
- **`SmolStr` import / `smol_str` crate dep** (line 71) — verify whether the only usage is inside the deleted code. If so, drop `smol_str = ...` from `crates/ism/Cargo.toml`.
- **`SciControlSystem` import / `SciCompartment` / `SciCompartmentSubcompartment` types and the helper structs at the bottom of the file (`from_system` / `text` / `into_system` @ 1510-1531)** — these are private helpers under `expected_sci_markings`. Die with the parent.
- **`use` imports** at lines 62-71 — many will become dead-imports post-deletion. The implementer trims at PR landing.

The implementer must run `cargo check -p marque-ism` after each major deletion to catch dead-code lints.

---

## 4. Constitution authorization argument

See §3 Decision 6 above for the operative argument. Inserting verbatim into the PR description body:

> **Constitution VII §IV authorization.** PR 4b-E touches `marque-ism` (PageContext `expected_*` deletion + renderer helper deletion), `marque-wasm` (CAB callsite migration), `marque-capco` (lattice-path residue migration), and `marque-engine` (comment-only). Per the consolidated plan §4 row PR-6 ("`PageContext` deleted at PR 6 merge — was PR 10, collapsed here under clean break"), this is the structural close of the engine refactor and not a scheme-adoption PR. The within-006 bugfix-class precedent for engine-crate edits is established by PR 4b-B Commit 2, PR 4b-C Commit 5, and PR 4b-D.2/.3 — all of which edited `marque-ism::page_context.rs` directly under the same authorization shape. No new scheme is being adopted; the PageRewrite catalog on `CapcoScheme` is unchanged.

---

## 5. Parity gate disposition plan

See §3 Decision 5 for the full plan. Summary:

- Delete `fn project_via_page_context` (`page_context_lattice_parity.rs:48-79`).
- Migrate ~50 `assert_byte_identity(.., project_via_page_context, project_via_lattice, &[])` calls to `assert_byte_identity(.., project_via_lattice, project_via_scheme, &[])`.
- Re-validate three previously-documented divergences (G-3 pure-NATO, joint_unanimous_two_portions, joint_single_portion_no_us) for convergence; expected outcome is byte-identity.
- Rename file to `crates/capco/tests/lattice_vs_scheme_parity.rs`.
- Update `crates/capco/CAPCO-CONTEXT.md` §3 divergence list (lines 261-282) to reflect post-4b-E parity shape.
- `project_via_scheme`-only fixtures (Pattern-B / Pattern-C declarative-row tests starting line 1979) are unchanged.

---

## 6. Test coverage plan

### What survives unchanged

- `page_context_lattice_parity.rs` (renamed) — see §5.
- `projected.rs::#[cfg(test)] mod tests` — already covers `from_canonical` + `is_solely_nato_classified` for the surviving surface.

### What gets rewritten against `ProjectedMarking` / lattice constructors

- **`crates/ism/tests/rollup_golden.rs`** (XSpec-derived golden tests) — every `ctx.expected_*` call migrates to either (a) a direct lattice helper call (`AeaSet::from_markings`, `DissemSet::from_attrs_iter`, etc.) or (b) `CapcoScheme::project_from_page_context` + reads from the resulting `CanonicalAttrs`. The golden semantics are unchanged — only the API the test calls.
- **`crates/ism/tests/proptest_page_context.rs`** — same migration. The proptest invariants pin lattice algebra; the API moves to lattice helpers.
- **`crates/capco/tests/scheme_equivalence.rs`** — already migrating; `ctx.expected_*` reads in this file were always comparing PageContext-equivalence; post-4b-E they compare lattice-vs-scheme equivalence (subsumed by `page_context_lattice_parity.rs`'s rename). Three options: (a) absorb file into the renamed parity gate, (b) rewrite each test against lattice helpers, (c) delete duplicated fixtures. Recommendation: (a) — absorb. The two files have overlapping intent.
- **`crates/capco/tests/pattern_a_nodis_exdis_page_context_alignment.rs`** — this test pins that the NODIS/EXDIS PageRewrite + the `expected_*` short-circuit produce consistent output. Post-4b-E the `expected_*` side is gone; the test rewrites as a single-path assertion against `scheme.project(Scope::Page, ...)`.
- **`crates/capco/tests/tetragraph_consolidation.rs`** — `ctx.expected_rel_to()` reads migrate to `RelToBlock::from_attrs_iter(...).into_boxed_slice()`.
- **`crates/capco/tests/rules_us1.rs` + `s004_audit_content_ignorance.rs`** — these allocate a `PageContext` and call `add_portion` / `is_empty` / `portions` only. **Unchanged** — they use the surviving shim surface.

### Internal `page_context.rs` tests

Per §3 Decision 3:

- Delete the ~120 `#[test]` functions targeting `expected_*` / `render_expected_banner`.
- Add 6 new `mod shim_tests` tests for the surviving shim surface (≥80% coverage on `new` / `add_portion` / `portion_count` / `is_empty` / `portions` / `Default` / `Clone`).
- Lift unique semantic coverage from the deleted tests into category-specific lattice law test files in `crates/capco/tests/` per the table in §3 Decision 3 (a):
  - SCI roll-up → `crates/capco/tests/sci_set_lattice_laws.rs` (or extend `category_lattice_laws.rs`)
  - SAR roll-up → extend `category_lattice_laws.rs`
  - AEA roll-up → extend `category_lattice_laws.rs`
  - Dissem overlays → `crates/capco/tests/dissem_set_lattice_laws.rs` or extend `category_lattice_laws.rs`
  - REL TO + DISPLAY ONLY → `crates/capco/tests/rel_to_display_only_laws.rs` (new — to cover the new `DisplayOnlyBlock`)
  - FGI marker → extend `category_lattice_laws.rs`
  - `render_expected_banner` byte-output → `crates/capco/tests/render_canonical_default_chain.rs` (the file the doc-comment at `marking_scheme_impl.rs:280-286` already names)

### Coverage gate

After migration, `cargo llvm-cov -p marque-ism` should report ≥80% line coverage on the trimmed `page_context.rs`. The shim surface is small (~50 lines including doc-comments) and the six new tests should reach near-100% on the surviving functions.

---

## 7. Walk-adjacent-paths register

Per the §2 inventory, the comment / doc-comment / test / twin sites that must be touched alongside each deletion:

| Deleted item | Comment refs to update | Doc-comment refs | Twin / parallel impl |
|---|---|---|---|
| `expected_classification` | `engine.rs:4193, 4271` (mentions `PageContext::project`); `page_context.rs:18` aggregation-rules table; `marking.rs:445` "max" classification comment | `ProjectedMarking.classification` doc-comment (`projected.rs:66-67`) — note "no longer needs the cross-reference to `PageContext::expected_classification`" | `ClassificationLattice::from_attrs_iter` @ `crates/capco/src/lattice.rs` |
| `is_solely_nato_classified` | `claudedocs/pr9c2-architect-preflight.md:46` — historical doc, leave alone | `projected.rs:217-270` doc-comment — drop the "equivalence note" paragraph about walking `self.portions` | `ProjectedMarking::is_solely_nato_classified` @ `projected.rs:271` |
| `expected_sci_controls/markings` | `specs/archive/003-sci-compartments/tasks.md` (historical) | `marking.rs:458-462` doc-comment ("Compatibility view: sci_controls is the flat CVE-enum projection") — update to name the new free helper | `SciSet::from_markings` / `to_markings` |
| `expected_sar_marking` | `marking.rs:464-466` doc-comment naming the field | `lattice.rs:149, 155, 162, 334, 342, 350` `sar_sort_key` callers (only refs, no edit needed) | `SarSet::from_marking` / `join` / `to_marking` |
| `expected_dissem_us/nato` | `actions/page_context.rs:48-52` (file deleted); `marking.rs:620-627` (already replaces); `scheme/marking.rs:660` G-6 comment | None | `DissemSet::from_attrs_iter` / `NatoDissemSet::from_attrs_iter` |
| `expected_rel_to` | `rules.rs:2498` ("`PageContext::expected_rel_to` already does tetragraph expansion") — update to name `RelToBlock::from_attrs_iter`; `lattice.rs:2086-2091` (lattice-path residue note) | `page_context.rs:30-38` aggregation rules table doc | `RelToBlock::from_attrs_iter` |
| `expected_display_only` | `actions/page_context.rs:54-65` (file deleted); `marking.rs:671-693` doc-comment | `page_context.rs:40-50` aggregation rules table doc | New `DisplayOnlyBlock::from_attrs_iter` (this PR creates it) |
| `expected_declassify_on` | `marking.rs:651-653` ("rides as last-observed per the existing PageContext semantic for now — Phase 3 TODO at page_context.rs:639") — update; the page_context.rs:639 line ref dies with the deletion | `page_context.rs:53-59` declassification note | `DeclassifyOnLattice::from_attrs_iter` |
| `expected_declass_exemption` | `marking.rs:654, 668` callers (this PR fixes); `lattice.rs:2085` ("PageContext reads `expected_non_ic_dissem`'s `needs_nf` second-tuple element directly") | `wasm/src/lib.rs:1283-1291` deferral marker (this PR resolves) | None (new `DeclassExemptionLattice` helper this PR may create per Decision §2 row b) |
| `is_classified` | `wasm/src/lib.rs:1277` (this PR migrates); `engine.rs:912, 1750` (test for is_empty, not is_classified — but adjacent); `lattice.rs:2080` doc | `page_context.rs:1030-1037` doc | None — predicate computable from `ProjectedMarking.classification` |
| `expected_aea_markings` | `rollup_golden.rs:73` UCNI transitional comment; `page_context.rs:1052-1093` body | `page_context.rs:1043-1051` doc | `AeaSet::from_markings` / `to_markings` |
| `expected_fgi_marker` | `marking.rs:480-617` G-4b/G-4c branch comments — update to name new `FgiSet::from_attrs_iter` | `page_context.rs:1193-1300` doc | New `FgiSet::from_attrs_iter` (this PR creates it) |
| `expected_non_ic_dissem` | `marking.rs:660-669` G-6 comment; `actions/page_context.rs:38-90` (file deleted); `rules.rs:2493, 7925` | `page_context.rs:1303-1500` doc; `lattice.rs:2082-2091` residue-axis doc | New `NonIcDissemSet::from_attrs_iter` returning `(set, bool)` |
| `render_expected_banner` | `wasm/src/lib.rs:1180-1183` (this PR migrates); `engine.rs:4192-4214` PageFinalization dispatch doc | `page_context.rs:1369-1611` doc | `MarkingScheme::render_canonical(scope=Scope::Page)` |
| `project` | `engine.rs:4193, 4271, 1207`; `rules.rs:9105` (this PR migrates); `projected.rs:188-194` "Lifecycle" note | `page_context.rs:212-270` doc — entire `project()` doc-block dies | `CapcoScheme::project_from_page_context` + `ProjectedMarking::from_canonical` |
| `actions/page_context.rs::page_context_to_attrs` | None (file deletion) | `crates/capco/tests/page_context_lattice_parity.rs:67-72` mentions "mirrors page_context_to_attrs" — update to remove the mirror reference | None — dead helper |

The implementer must run a final `grep -rn "expected_" crates/ docs/ specs/` after deletions to catch references the table missed; comment / doc references in historical plan docs (`docs/plans/2026-04-28-*`, `docs/plans/2026-05-12-*`, `specs/archive/003-*`) are historical and may stay as artifacts of their original PR's context.

---

## 8. Risk register

1. **The test driver at `rules.rs:9105` may be entirely dead post-PR-3c.B.** The `#[cfg(any())] #[cfg(test)] mod tests` block at `rules.rs:5222` (gated by the legacy `FixProposal` field migration) is the only known caller. If `pub(crate) fn run` has no live callers in the current build, the rewrite plan in §3 Decision 2 is wasted effort and the driver should be deleted instead. PM verification needed — see §10 OQ-2.

2. **The lattice path's residue dependencies on `tmp_ctx.expected_*` are the load-bearing risk.** `marking.rs:462, 616, 654, 668, 689` each consume a `PageContext` for an axis the lattice **does not yet model independently**. Five new lattice helpers must land in 4b-E:
   - `sci_controls_from_markings(&[SciMarking]) -> Box<[SciControl]>` (free helper)
   - `FgiSet::from_attrs_iter(portions) -> FgiSet` (constructor — currently `FgiSet::from_marker` is per-portion only)
   - `DeclassExemptionLattice::from_attrs_iter(portions) -> Option<DeclassExemption>` (new lattice; or inline)
   - `NonIcDissemSet::from_attrs_iter(portions) -> (Box<[NonIcDissem]>, bool)` (new lattice; bundles the `needs_nf` cross-axis injection signal)
   - `DisplayOnlyBlock::from_attrs_iter(portions) -> DisplayOnlyBlock` (new lattice — parallel to `RelToBlock`)
   Each requires a §-citation block and ≥1 lattice-law test. **None of these are mentioned in the PR scope statement.** They are load-bearing for the deletion. If any one is omitted, the residue-axis migration cannot land and the deletion is blocked.

3. **`DisplayOnlyBlock` constructor scope.** Per `marking.rs:671-680`, "A dedicated `DisplayOnlyBlock` lattice (parallel to RelToBlock) is queued for the same PR-cycle as the PageContext deletion." This PR is that PR-cycle. The DISPLAY ONLY axis logic at `page_context.rs:881-996` is **non-trivial** — §D.2 Table 3 rows 18-20 + 25-27 + USA-subtraction + banner-REL-TO subtraction. A naive lift risks regressing the parity gate's `display_only_*` fixtures. The implementer should sequence this lattice landing **before** the `expected_display_only` deletion (separate commit; see §9).

4. **Test count delta.** The internal `page_context.rs` tests are ~120 functions. Migrating semantic coverage to lattice tests AND adding the 6 shim tests AND rewriting `rollup_golden.rs` / `proptest_page_context.rs` / `pattern_a_*` / `tetragraph_*` is a substantial test refactor. PM should expect ~150 test-file edits across the PR.

5. **`smol_str` crate dependency removal.** The PageContext type imports `SmolStr` at line 71. Verify (post-deletion) whether other items in `marque-ism` use `SmolStr`; if not, drop the crate dep. Dependency-graph change — Constitution VII §IV consistent with the bugfix-class precedent.

6. **`scheme_equivalence.rs` decision is folded into the parity gate.** If the PM elects to keep `scheme_equivalence.rs` separate rather than absorbing into the renamed parity file (Decision §6 sub-decision (c)), additional test-rewrite work lands. Default absorption is the simpler outcome.

7. **Coverage gate on `page_context.rs`.** Post-deletion the file is ~150 lines (shim + sar_sort_key remaining or moved + use imports). The 6 new shim tests should clear 80% on `cargo llvm-cov`; if coverage falls short due to `Default` / `Clone` impls being skipped, add a direct `pre_size_invariant_on_default` and `pre_size_invariant_on_clone` test.

8. **Bench drift.** `crates/engine/benches/profile_project.rs:178, 213, 232` uses `PageContext::new` + `add_portion`. These reads stay on the surviving shim — no migration needed. **No bench regression expected.** Confirm with `bench-check.sh` post-merge.

9. **WASM size regression risk.** Option (a) for the CAB strategy adds inline accumulator logic to `generate_cab_native`. Verify `wasm-pack build --release` size delta is non-positive (the deletions should shrink the WASM binary; the inline additions are <10 SLOC).

10. **The S005 NOFORN-bail at `rules.rs:2486-2492`** reads `page.portions()` directly; **migration target unchanged**. The `expected_non_ic_dissem` read at line 2493 needs to migrate to the new `NonIcDissemSet::from_attrs_iter(...)` constructor (risk #2 above).

---

## 9. Commit decomposition recommendation

PR 4b-E is large but cleaves cleanly along the residue-axis migration boundary. Recommended seven-commit sequence:

- **Commit 1 — Plan addenda + new lattice constructors (skeleton).** Land empty (or minimally-functional) `DisplayOnlyBlock`, `FgiSet::from_attrs_iter`, `NonIcDissemSet::from_attrs_iter`, `sci_controls_from_markings`, `DeclassExemptionLattice::from_attrs_iter` (or inline-helper) with one §-citation block + one happy-path test each. Wire none into production code yet. This is the load-bearing structural commit; everything downstream depends on it.

- **Commit 2 — Lattice residue migration.** Rewire `marking.rs::join_via_lattice_with_context` (lines 462, 616, 654, 668, 689) to consume the new constructors from Commit 1. Delete `actions/page_context.rs::page_context_to_attrs` (entire file). The lattice path no longer touches `tmp_ctx.expected_*`; `tmp_ctx` is only used to bracket the `add_portion` accumulator for caller-side `PageContext` reuse (and even that goes away after Commit 5). Run `crates/capco/tests/page_context_lattice_parity.rs` in its pre-rename shape — all PageContext-vs-lattice byte-identity assertions must still pass (because the new constructors are byte-equivalent to the `expected_*` they replace). Any divergence here is the load-bearing regression catch.

- **Commit 3 — WASM CAB migration.** `compute_banner_native` calls `scheme.render_canonical(Scope::Page)` instead of `render_expected_banner`. `generate_cab_native` inlines the per-portion accumulator for `declass_exemption` (option a per Decision §1) and replaces `is_classified()` with `page_marking.classification.is_some_and(|c| c.effective_level() > Unclassified)`. WASM tests pass.

- **Commit 4 — Production migrations (capco rules).** `rules.rs:2493` (S005 non-IC dissem read) migrates to `NonIcDissemSet::from_attrs_iter`. `rules.rs:2502` (S005 rel_to read) migrates to `RelToBlock::from_attrs_iter`. `rules.rs:9105` (test driver) migrates to `scheme.project_from_page_context + ProjectedMarking::from_canonical`. S005 + W004 callsites for `page.portions()` are unchanged. Rules + corpus tests pass.

- **Commit 5 — `page_context.rs` deletion.** Delete every `expected_*` method, `is_classified`, `is_solely_nato_classified`, `render_expected_banner`, `render_sci_markings_block`, `render_sar_block`, `project`, `expand_tetragraph` duplicate, and the internal `#[cfg(test)] mod tests` block (~lines 1622-3617). The shim retains `Default` / `Clone` / `new` / `add_portion` / `portion_count` / `is_empty` / `portions()`. `cargo check -p marque-ism` must pass; this is the moment the deletion actually lands.

- **Commit 6 — Test-file migrations.** `rollup_golden.rs` / `proptest_page_context.rs` / `pattern_a_nodis_exdis_page_context_alignment.rs` / `tetragraph_consolidation.rs` migrated against lattice helpers per §6. `scheme_equivalence.rs` absorbed into the renamed parity gate. New `shim_tests` mod in `page_context.rs` with the 6 tests from Decision §3 sub-decision (c).

- **Commit 7 — Parity gate rename + divergence convergence + sar_sort_key relocation + dead-import sweep.** Rename `page_context_lattice_parity.rs` to `lattice_vs_scheme_parity.rs`; delete `project_via_page_context`; convert all `assert_byte_identity` calls to lattice-vs-scheme comparisons; re-validate the three documented divergences for convergence. Relocate `sar_sort_key` to `crates/ism/src/sar_sort.rs` with re-export. Update `crates/capco/CAPCO-CONTEXT.md` §3 divergence list. Final `cargo clippy --workspace -- -D warnings`; trim dead imports. Update doc-comment cross-references per §7.

**Why this order:** Commit 1 establishes the type-level contracts; Commit 2 wires them; Commit 3-4 migrate consumers; Commit 5 is the load-bearing deletion (it can't compile until 1-4 have landed); Commit 6 cleans up tests; Commit 7 is hygiene. Each commit is independently reviewable; only Commit 5 is irreversible-by-revert.

**Split criterion:** if Commit 1 + Commit 2 turn out to exceed ~800 lines combined, split Commit 1 into one commit per new lattice helper (5 sub-commits). Don't split Commit 2 — atomicity matters for the lattice-path correctness assertion.

---

## 10. Open questions for PM

Each item is sized for one decision. Resolve before implementation begins.

**OQ-1 (Decision §1, WASM CAB strategy).** Confirm option (a) — inline per-portion accumulator at the WASM `generate_cab_native` callsite for `declass_exemption`; `is_classified()` migrates to the `ProjectedMarking.classification.effective_level() > Unclassified` predicate. The alternative is option (b) — a new `CabProjection` type in `marque-ism`. Recommendation is (a). **PM choice: (a) / (b) / other?**

**OQ-2 (Decision §2, test driver in `rules.rs:9105`).** The `pub(crate) fn run` test driver only appears to be called from the `#[cfg(any())] #[cfg(test)] mod tests` block at `rules.rs:5222`, which is `cfg(any())`-disabled pending the `marque-mvp-3 → marque-1.0` `FixProposal` migration (PR 3c.2 territory). **Is the driver dead?** If yes, delete it instead of rewriting (saves Commit 4 ~30 lines). **PM choice: rewrite / delete / verify-first?**

**OQ-3 (Decision §3 sub-decision (c), `scheme_equivalence.rs` absorption).** Absorb `scheme_equivalence.rs` into the renamed `lattice_vs_scheme_parity.rs`, or keep separate? Default recommendation is absorb (same intent post-4b-E). **PM choice: absorb / keep / decide-during-implementation?**

**OQ-4 (Decision §4, `sar_sort_key` relocation).** Move `sar_sort_key` to `crates/ism/src/sar_sort.rs` with re-export from `marque_ism::lib.rs`, or leave inline in the trimmed `page_context.rs`? Recommendation: relocate (T069-readiness). **PM choice: relocate / inline / defer?**

**OQ-5 (Risk #2, new lattice helpers in scope).** PR scope explicitly names PageContext deletion + WASM-CAB migration. The lattice-path residue migration requires 5 new lattice helpers (`DisplayOnlyBlock`, `FgiSet::from_attrs_iter`, `NonIcDissemSet::from_attrs_iter`, `DeclassExemptionLattice` or inline, `sci_controls_from_markings`). **Confirm these are in scope for 4b-E** — if not, the deletion is blocked and must defer until they land in a separate PR (which would be a regression on the consolidated plan's "PageContext deleted at PR 6 merge — clean break" commitment). Recommendation: include in scope. **PM choice: include in scope / split into PR 4b-E.1 / other?**

**OQ-6 (Risk #3, `DisplayOnlyBlock` separate commit).** Land `DisplayOnlyBlock` (the new dedicated lattice for the DISPLAY ONLY axis per §D.2 Table 3 rows 18-20 + 25-27) as its own pre-deletion commit (separate from the other four new helpers in Commit 1)? It's the largest of the new helpers (~80 lines of body + tests). Recommendation: yes — split it out so the §-citation block + ≥1 test land independently reviewable. **PM choice: split / fold-into-Commit-1?**

**OQ-7 (Decision §5, divergence convergence).** Post-PR-4b-E the three documented divergences (G-3 pure-NATO, joint_unanimous_two_portions, joint_single_portion_no_us) should converge to byte-identity because both compared sides are now lattice-path-derived. **If any of them doesn't converge, is the PR blocked, or is the divergence accepted as a new documented divergence on the lattice side?** Recommendation: blocking — a genuine non-convergence here means a real correctness defect in either `project_via_lattice` or `project_via_scheme` and should be investigated, not papered over. **PM choice: blocking / accept-as-documented / decide-during-implementation?**

**OQ-8 (Test-file scope).** Confirm the test files in §6 "rewritten" list are the complete set. Implementer should re-grep `expected_` / `render_expected_banner` / `is_classified` after Commit 5 to catch missed files. **Should the implementer return for plan revision if the grep finds undocumented call sites, or proceed with case-by-case judgment?** Recommendation: proceed with case-by-case judgment, but commit-message-call out any unexpected migrations.

---

# Summary

PR 4b-E retires PageContext's `expected_*` machinery + renderer methods now that PR 4b-D.2 flipped the hot path to `scheme.project(Scope::Page, ...)`. The deletion is **NOT** a simple inventory removal because `crates/capco/src/scheme/marking.rs::join_via_lattice_with_context` still calls `tmp_ctx.expected_sci_controls/fgi_marker/declass_exemption/non_ic_dissem/display_only` as residue-axis accessors. Five new lattice helpers must land in this PR to migrate the residue: `DisplayOnlyBlock::from_attrs_iter`, `FgiSet::from_attrs_iter`, `NonIcDissemSet::from_attrs_iter`, `sci_controls_from_markings`, and a `DeclassExemptionLattice` (or inline equivalent). The WASM CAB build inlines a per-portion accumulator at `generate_cab_native` (option a; option b's `CabProjection` type deferred). The `rules.rs:9105` test driver rewrites against `CapcoScheme::project_from_page_context + ProjectedMarking::from_canonical`; it may instead be dead code (PM verification). The parity gate `page_context_lattice_parity.rs` renames to `lattice_vs_scheme_parity.rs`, deletes `project_via_page_context`, and converts to `project_via_lattice ↔ project_via_scheme`; the three documented divergences (G-3 + two joint cases) should converge. `sar_sort_key` moves to its own module for T069-readiness. ~120 internal page_context.rs tests delete; semantic coverage migrates to category-specific lattice law files; 6 new shim tests cover the surviving `Default/Clone/new/add_portion/portion_count/is_empty/portions()` surface.

Authorization rests on the consolidated plan §4 PR-6 row + the within-006 bugfix-class precedent (PR 4b-B Commit 2, PR 4b-C Commit 5, PR 4b-D.2/.3) — this is the engine refactor's structural close, not a scheme-adoption PR.

Recommended commit decomposition: seven commits (new helpers → lattice residue migration → WASM CAB → capco rule migrations → page_context.rs deletion → test-file migrations → hygiene + parity rename). Commit 5 is the only irreversible-by-revert commit.

**Open questions for PM** (in priority order):
1. OQ-5 — confirm 5 new lattice helpers are in scope for 4b-E (load-bearing for the deletion)
2. OQ-1 — WASM CAB option (a) inline accumulator vs (b) new `CabProjection` type
3. OQ-2 — `rules.rs:9105` test driver: rewrite or delete (verify dead-code claim)
4. OQ-7 — divergence convergence: blocking vs documented divergence
5. OQ-3 — absorb `scheme_equivalence.rs` into renamed parity gate
6. OQ-4 — relocate `sar_sort_key` to its own module for T069-readiness
7. OQ-6 — split `DisplayOnlyBlock` into its own pre-deletion commit
8. OQ-8 — re-grep protocol after Commit 5 if undocumented call sites surface

Files I expect the implementer to touch (paths absolute):

- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/ism/src/page_context.rs` (bulk deletion + 6 new shim tests)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/ism/src/sar_sort.rs` (new file; OQ-4)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/ism/src/lib.rs` (re-export updates)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/ism/tests/rollup_golden.rs` (rewrite against lattice helpers)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/ism/tests/proptest_page_context.rs` (rewrite)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/src/scheme/marking.rs` (residue-axis migration; lines 462, 616, 654, 668, 689)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/src/scheme/actions/page_context.rs` (file deletion)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/src/lattice.rs` (5 new lattice helpers + `sar_sort_key` callsite re-import)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/src/rules.rs` (lines 2493, 2502, 9105 + import update at line 113)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/tests/page_context_lattice_parity.rs` → rename to `lattice_vs_scheme_parity.rs` (parity-gate rewrite)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/tests/scheme_equivalence.rs` (absorb OR rewrite per OQ-3)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/tests/pattern_a_nodis_exdis_page_context_alignment.rs` (rewrite)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/tests/tetragraph_consolidation.rs` (rewrite)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/tests/category_lattice_laws.rs` (extend for lifted semantic coverage)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/tests/render_canonical_default_chain.rs` (extend for `render_expected_banner` byte-output coverage)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/capco/CAPCO-CONTEXT.md` (§3 divergence-list update lines 261-282)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/wasm/src/lib.rs` (CAB callsite migration at lines 1153-1183, 1277, 1296)
- `/home/knitli/marque/.claude/worktrees/pr-4b-d-hotpath-flip/crates/engine/src/engine.rs` (comment-only updates at lines 4193, 4271, 1207)
