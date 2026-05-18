<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0 -->

# PR 4b-F Lattice-Semantic Review

**Date:** 2026-05-18
**Reviewer:** marque-lattice-consultant (lattice-algebra + projection-pipeline lens)
**Worktree:** `/home/knitli/marque/.claude/worktrees/pr-4b-f-residue-cleanup`
**Branch:** `refactor-006-pr-4b-f-residue-cleanup` (5 functional commits + 1 bookkeeping commit ahead of `staging`)
**Companions:** rust-reviewer (Rust idiom) + code-reviewer (overall quality), running in parallel.

---

## §1 Verdict

**APPROVED.**

Zero CRITICAL findings. Zero HIGH findings. Two LOW / NOTE-class observations
(documented in §9). The PR delivers exactly what the architect plan
describes: a pure-signature consolidation of the post-PR-4b-E residue,
with the same load-bearing lattice + projection semantics preserved.

The post-PR pipeline shape is structurally identical to the pre-PR pipeline.
The same per-axis lattice composition (10+ axes in the G-1 through G-9
ordering), the same closure call, the same topologically-ordered
PageRewrites. The dead `&PageContext` parameters are gone; the
read-only-attrs G13 sentinel is preserved verbatim; the same-slice
debug-assert that retires (Commit 4) is structurally vacuous post-Commits-1-3
because there is exactly one slice-derivation path through the engine
(`page_context.portions()` at the call boundary).

Parity gate: **74 / 74 fixtures pass** including all post-PR-4b-E
documented `dissem_us` divergences and the three historical convergence
cases. Full workspace `cargo test` clean.

---

## §2 Pipeline composition correctness (per §4.7.4)

The lattice-design plan §4.7.4 mandates:

```text
parse → join (per-axis lattice) → closure (Cl_supp) → PageRewrites → render
                                  ^ Kleene fixpoint  ^ topologically-ordered DAG
```

Verified in `CapcoScheme::project_attrs_pipeline`
(`crates/capco/src/scheme/marking_scheme_impl.rs:699-787`):

```rust
fn project_attrs_pipeline(&self, raw: &[CanonicalAttrs]) -> CanonicalAttrs {
    #[cfg(debug_assertions)]
    let raw_snapshot: Vec<CanonicalAttrs> = raw.to_vec();

    let joined = CapcoMarking::new(CapcoMarking::join_via_lattice(raw));
    let mut out = self.closure(joined);

    #[cfg(debug_assertions)]
    {
        if raw != raw_snapshot.as_slice() {
            panic!("closure() mutated ... §3 (e.1)", ...);  // counts-only
        }
    }

    for rw in &self.page_rewrites {
        // ... apply rewrites in topological order ...
    }
    out
}
```

**Sequence verified:** join → closure → PageRewrites, in that order, with
the read-only-attrs sentinel snapshotting `raw` BEFORE `closure()` and
asserting byte-identity AFTER. No double-closure. No closure-before-join.
No PageRewrite reorder. **PASS.**

### §2.1 Per-axis composition order in `join_via_lattice_body`

The G-1 through G-9 phase markers from PR 4b-B are preserved verbatim:
all 11 `// G-N` comment anchors that exist in `staging` survive in the
post-PR HEAD revision (verified by symmetric grep across both refs). The
axis composition order in the function body is also unchanged:

1. **Classification + JointSet** (G-3, G-9, G-9b — variant-preserving
   `OrdMax` with JointSet override) — `crates/capco/src/scheme/marking.rs:284-348`.
2. **SCI / SAR / AEA** (Axis 2-5, via `SciSet`, `SarSet`, `AeaSet`
   constructors) — `marking.rs:350-388`.
3. **FGI** (G-4, G-4b, G-4c, G-5 — JOINT-unanimity suppression,
   solely-non-US source-loss reconstruction, classification-derived +
   explicit marker merge) — `marking.rs:389-530`.
4. **Dissem 6-7** (`DissemSet` + `NatoDissemSet`) — `marking.rs:532-540`.
5. **REL TO Axis 8** (capturing `is_noforn_superseded` + `is_empty_intersection`
   for the deferred banner-row-9 NOFORN injection) — `marking.rs:541-562`.
6. **DeclassifyOn + DeclassExemption Axis 9** — `marking.rs:564-574`.
7. **NonIcDissem Axis 10** (carrying `needs_nf` forward) — `marking.rs:576-588`.
8. **DisplayOnly** (consuming `rel_to_block` + `needs_nf` per §D.2 Table 3
   rows 18-20 + 25-27) — `marking.rs:590-604`.
9. **`out.rel_to = rel_to_block.into_boxed_slice()`** (deferred from step
   5 so `DisplayOnlyBlock` can borrow `rel_to_block`) — `marking.rs:606-611`.
10. **G-8 cross-axis NOFORN-supersession rendezvous**
    (`DissemSet::with_noforn_injected` for NofornSuperseded ∨
    EmptyIntersection ∨ needs_nf) — `marking.rs:613-646`.

Cross-axis state flow that is load-bearing per CAPCO §G.1 Table 4 p38 +
§H.7 pp123-125 + §H.3 p57 + §H.8 p145: **all preserved.** No phase
deletion. The signature edit (removing `_tmp_ctx: &PageContext`) does not
disturb the body composition. **PASS.**

### §2.2 Closure-call ordering and read-only-attrs sentinel

The `#[cfg(debug_assertions)] let raw_snapshot: Vec<CanonicalAttrs> = raw.to_vec();`
followed by `panic!` on `raw != raw_snapshot.as_slice()` is preserved
verbatim across Commit 2's rename (`project_attrs_pipeline_with_context`
→ `project_attrs_pipeline`). The G13 panic message stays counts-only
(`raw_snapshot.len()` and `raw.len()`, never content). **PASS** —
matches the §3 (e.1) read-only-attrs invariant per the lattice-design
plan and Constitution V Principle V (G13).

### §2.3 PageRewrites topological ordering

The `for rw in &self.page_rewrites` loop iterates `self.page_rewrites`,
which is a `Vec<PageRewrite<S>>` whose order is fixed at
`Engine::new` time by Kahn's algorithm in
`crates/engine/src/scheduler.rs` (writes-before-reads topological sort).
PR 4b-F does not edit any scheduler or rewrite-catalog code, so the
ordering is unchanged. **PASS.**

---

## §3 Closure rules + PageRewrite catalog preservation

### §3.1 Closure rules (Pattern D)

The 7 `CLOSURE_NOFORN_*` rows at `crates/capco/src/scheme/closure.rs`
per memory `project_pattern_d_already_shipped` were consolidated to a
single `CLOSURE_NOFORN_CAVEATED` row (the algebraic union, per D18) in
a prior PR. PR 4b-F edits `closure.rs` ONLY at a doc-comment renaming
`project_attrs_pipeline_with_context` → `project_attrs_pipeline` (4 lines
in the `noforn_clears_rows()` rationale). The `CLOSURE_NOFORN_CAVEATED`
`const` block at `closure.rs:340+` is untouched. The closure-rules
registration via `CapcoScheme::closure_rules() = &[CLOSURE_NOFORN_CAVEATED, ...]`
(per closure.rs:735) is untouched. **PASS.**

The closure operator's monotone / extensive / idempotent invariants
(Cousot-Cousot fixpoint discipline; lattice-design plan §3 (e)) ride on
the per-row data, not on the call signature — so the signature
simplification cannot break the closure laws.

### §3.2 PageRewrite catalog

Per the architect plan, PR 4b-F's doc-comment sweep in
`rewrites/noforn_clears.rs` and `rewrites/pattern_c.rs` is pure-doc.
Verified by inspection — only doc-comment lines are edited; the catalog
data (`PageRewrite { id, trigger, action, reads, writes, ... }` rows) is
byte-identical. The 9 declarative PageRewrite rows landed in PR 4b-C
(7 Pattern-C strip rows + 2 Pattern-B FOUO-eviction rows) are preserved.
**PASS.**

---

## §4 Parity gate result

Ran `cargo test --test lattice_vs_scheme_parity -p marque-capco`.

**Result: 74 passed; 0 failed; 0 ignored.** (`finished in 0.00s` — the
parity-gate fixtures are pure-Rust, no I/O.)

The parity gate's file `crates/capco/tests/lattice_vs_scheme_parity.rs`
is byte-identical to staging (`git diff staging..HEAD -- crates/capco/tests/lattice_vs_scheme_parity.rs`
returns zero lines). The two compared paths
(`project_via_lattice` = `CapcoMarking::join_via_lattice(portions)` and
`project_via_scheme` = `scheme.project(Scope::Page, &markings)`) now both
flow through `CapcoScheme::project_attrs_pipeline` post-PR. The
documented 12 `dissem_us` divergence fixtures (per the post-PR-4b-E
divergence inventory annotated with `§B.3 Table 2 p21` — the
`CLOSURE_NOFORN_CAVEATED` rule firing on the scheme side) all pass with
the correct expected outputs encoded. **PASS.**

Workspace-wide `cargo test --workspace` also clean (zero failures across
~80 test targets; 3 pre-existing ignored tests; full breakdown in §5
implementation report).

---

## §5 `JoinSemilattice` impl status (PR #456 + Copilot R1 D24)

Per PR 4b-D.2 Commit 11 + D24, `impl JoinSemilattice for CapcoMarking`
and `impl MeetSemilattice for CapcoMarking` were both deleted because
the cross-axis fold violated `JoinSemilattice` structural-`Eq`
idempotence in the presence of per-axis normalization
(`RelToBlock`'s tetragraph expansion: `m != m.join(&m)` after expansion).

**Verified: PR 4b-F does NOT reintroduce these impls.** Workspace grep
for `impl JoinSemilattice for CapcoMarking | impl MeetSemilattice for CapcoMarking | impl Lattice for CapcoMarking`
returns only:

- `crates/capco/src/scheme/marking.rs:665-666` — a comment block in the
  `PR 4b-D.2 status note on the Lattice impl` documentation explaining
  the retirement (with the PR 4b-F doc-update preserving the historical
  record).
- `crates/capco/CAPCO-CONTEXT.md:241` — a CAPCO-CONTEXT note citing the
  D24 retirement.

Both are historical mentions, not live impls. **PASS.** The cross-axis
fold lives as the inherent method `CapcoMarking::join_via_lattice` and
PR 4b-F's collapse of the `_with_context` fast-path variant is sound
because both inherent methods called the same `join_via_lattice_body`
that now takes only `&[CanonicalAttrs]`.

Note: the `MarkingScheme::Marking` trait bound was also relaxed in PR
4b-D.2 Commit 11 (per memory `project_pr538_observational_lattice_audit`
and the lattice-design plan §12.4); the bound is now `JoinSemilattice`
instead of `Lattice`. PR 4b-F does not edit `crates/scheme/`, so the
relaxed bound is preserved.

---

## §6 Per-axis lattice impl preservation (T112 axes)

`crates/capco/src/lattice.rs` has 13 per-axis lattice types:

| Type | `JoinSemilattice` | `MeetSemilattice` | Source |
|---|---|---|---|
| `SciSet` | ✓ (l. 216) | ✓ (l. 239) | PR 4b-A |
| `SarSet` | ✓ (l. 380) | ✓ (l. 394) | PR 4b-A |
| `FgiSet` | ✓ (l. 557) | ✓ (l. 590) | PR 4b-A |
| `AeaSet` | ✓ (l. 1044) | ✓ (l. 1073) | PR 4b-A |
| `ClassificationLattice` | ✓ (l. 1662) | ✓ (l. 1704) | PR 4b-B |
| `NatoClassLattice` | ✓ (l. 1831) | ✓ (l. 1846) | PR 4b-B |
| `DeclassifyOnLattice` | ✓ (l. 1957) | ✓ (l. 1972) | PR 4b-B |
| `DissemSet` | ✓ (l. 2313) | — *(intentional, PR #456)* | PR 4b-B |
| `NatoDissemSet` | ✓ (l. 2410) | ✓ (l. 2418) | PR 4b-B |
| `JointSet` | ✓ (l. 2748) | — *(intentional, PR #456)* | PR 4b-B |
| `RelToBlock` | ✓ (l. 3074) | ✓ (l. 3100) | PR 4b-B |
| `DisplayOnlyBlock` | ✓ (l. 3712) | — *(intentional, PR 4b-E)* | PR 4b-E |
| `DeclassExemptionAccumulator` | — *(intentional, PR 4b-E)* | — *(intentional, PR 4b-E)* | PR 4b-E |
| `NonIcDissemSet` | — *(see §6.1)* | — *(see §6.1)* | PR 4b-E |

Doc-comment edits in `lattice.rs` are only at:

- The `DissemSet` doc-comment header (rewriting the post-PR-4b-E
  divergence inventory away from the retired `PageContext::expected_*`
  references).
- A line in `DissemSet::to_vec` doc updating the "compatibility with
  existing `PageContext::expected_dissem_us`-shaped APIs" comment.
- A line in `DissemSet::with_noforn_injected` doc updating the
  pre-G-8 reference.
- A line in `JointSet::from_attrs_iter` doc updating the malformed-portion
  PageContext reference to point at `ClassificationLattice`.

**No `impl JoinSemilattice` body or `impl MeetSemilattice` body is
edited.** **PASS.** Per-axis lattice laws (associativity / commutativity
/ idempotence / monotonicity) preserved by construction.

### §6.1 `NonIcDissemSet` and `DisplayOnlyBlock` lattice status

Both are projection accumulators rather than full lattices:

- `NonIcDissemSet` exposes `needs_nf` as a cross-axis injection signal
  consumed by `DissemSet::with_noforn_injected` at the rendezvous in
  `join_via_lattice_body`; the type is an accumulator helper, not a
  standalone lattice. Per memory `pr538-observational-lattice-audit`
  the type's role is observational state for the §H.9 p178/p185
  classification-gated NOFORN injection.
- `DisplayOnlyBlock` implements only `JoinSemilattice` (the meet over a
  display-only set with banner-REL-TO subtraction has no algebraic meet
  with a sensible identity).

The PR 4b-F implementation report (§7 Principle VI attestation) and the
plan §3 (b) "DeclassExemptionAccumulator non-lattice status" both reflect
this. **PASS.**

---

## §7 Same-slice contract elimination — semantic walk

The architect plan argues the same-slice debug-assert at
`join_via_lattice_with_context` (which Commit 4 retires) becomes vacuous
post-Commits-1-3 because there is exactly one derivation path for `raw`
from `page_context.portions()` at the engine boundary. I walked the call
graph to verify.

### §7.1 Trait body path (`MarkingScheme::project`)

`crates/capco/src/scheme/marking_scheme_impl.rs:252-254`:

```rust
let raw: Vec<CanonicalAttrs> = markings.iter().map(|m| m.0.clone()).collect();
let out_attrs = self.project_attrs_pipeline(&raw);
```

`raw` is a **fresh `Vec`** owned by the trait body. No `PageContext` is
involved. The `&raw` slice is the only slice that flows through
`project_attrs_pipeline` → `join_via_lattice` → `join_via_lattice_body`.
There is no parallel `&PageContext` for the body to drift from. **OK.**

### §7.2 Engine fast-path (`CapcoScheme::project_from_page_context`)

`crates/capco/src/scheme/marking_scheme_impl.rs:673-678`:

```rust
pub fn project_from_page_context(
    &self,
    page_context: &marque_ism::PageContext,
) -> CanonicalAttrs {
    self.project_attrs_pipeline(page_context.portions())
}
```

`page_context.portions()` is called **once at this boundary** to derive
`&[CanonicalAttrs]`. The returned slice flows to `project_attrs_pipeline`
→ `join_via_lattice` → `join_via_lattice_body`. There is no parallel
slice to drift from. **OK.**

### §7.3 Engine call site (`Engine::project_page_marking`)

`crates/engine/src/engine.rs:4504-4525`:

```rust
fn project_page_marking(
    scheme: &CapcoScheme,
    page_context: &marque_ism::PageContext,
) -> marque_ism::ProjectedMarking {
    let projected = scheme.project_from_page_context(page_context);
    marque_ism::ProjectedMarking::from_canonical(projected)
}
```

The engine passes `page_context` to `scheme.project_from_page_context`,
which itself derives `page_context.portions()` once. No third derivation
path. **OK.**

### §7.4 No third call site

Workspace grep for `project_attrs_pipeline` returns exactly two production
call sites (the two in `marking_scheme_impl.rs` documented above, lines
253 and 677) and two `join_via_lattice` production call sites (the
`marking.rs:169` body delegate and the call at `marking_scheme_impl.rs:723`).
The bench file `crates/engine/benches/profile_project.rs` has additional
call sites for benchmarking but does not construct a parallel slice that
could drift. **OK.**

**Conclusion: the same-slice contract is genuinely vacuous post-Commits-1-3.**
The runtime debug-assert was load-bearing only when multiple internal
layers each derived their own view from a `&PageContext`; after the
signature consolidation there is exactly one derivation site. Retiring
the assert in Commit 4 is safe. The structural property is documented
in the `project_from_page_context` doc-comment (lines 686-698) so a
future engineer reintroducing a parallel derivation path has a written
contract to violate before re-adding the runtime check at the new fork.

**PASS.**

---

## §8 G-marker comment survival audit

The `join_via_lattice_body` function body has inline `// G-N (PR 4b-B follow-up): ...`
markers across axis composition. I counted G-markers in both staging and
HEAD revisions:

| Revision | `grep -nE '// G-[0-9]' crates/capco/src/scheme/marking.rs \| wc -l` |
|---|---|
| `staging` | 11 |
| HEAD (PR 4b-F) | 11 |

Verbatim G-markers preserved (verified by inspection):

- `G-9 (PR 4b-B follow-up): Conflict always flattens ...` (l. 328)
- `G-4 (PR 4b-B follow-up): when JointSet is UnanimousProducers ...` (l. 396)
- `G-5 (PR 4b-B follow-up): when both an explicit FgiSet ...` (l. 402)
- `G-4b (PR 4b-B 7th-pass follow-up): solely-non-US page ...` (l. 418)
- `G-4c (PR 4b-B 9th-pass follow-up): blanket suppression ...` (l. 444)
- `G-8 (PR 4b-B follow-up): when NOFORN must be injected ...` (l. 616)

Plus 5 more G-marker references in the body and doc-comment block. Every
G-marker is preserved either verbatim or with only the surrounding
PageContext-reference doc text re-anchored to lattice-native helper
references (G-4b, G-4c). The `(PR 4b-B follow-up)` provenance label is
retained on every active marker. **PASS.**

---

## §9 Findings table

| Severity | File:Symbol | Description | Suggested fix |
|---|---|---|---|
| LOW | `crates/capco/src/scheme/actions/mod.rs:24` | Comment refers to "the `join_via_lattice` body" but the `_with_context` variant retired in Commit 4 — comment is correct but the symbol-reference shape could be tightened. | Optional. Reads cleanly as-is; the comment names the structural site. |
| NOTE | `crates/capco/src/scheme/marking.rs:181` | The `_tmp_ctx` historical-record reference in the `join_via_lattice_body` doc-comment is the sole surviving workspace-wide mention of the retired parameter. Implementation-report §3 OQ-2 documents this as intentional (a deletion record, not a live parameter). | None. Documented in implementation report. |

Zero CRITICAL. Zero HIGH. Zero MEDIUM.

The two LOW / NOTE-class observations are stylistic / archival, not
correctness issues. The implementation cleanly preserves every load-bearing
lattice + projection semantic.

---

## §10 Lattice-law spot-check

The lattice-consultant skill's proof-checker subagent is not needed for
this review because PR 4b-F is a **pure-signature change** — no
algebraic operation is added, removed, or modified. The per-axis lattice
impls in `crates/capco/src/lattice.rs` are untouched at the body level;
their associativity / commutativity / idempotence / absorption laws are
load-bearing for `category_lattice_laws.rs`'s 71 test cases, which all
pass.

The one spot-check I did want to verify: the `closure → PageRewrites`
ordering at `project_attrs_pipeline:723-738`. The pipeline post-PR is:

1. `let joined = CapcoMarking::new(join_via_lattice(raw))` — per-axis lattice composition.
2. `let mut out = self.closure(joined)` — monotone-extensive-idempotent closure operator (Cl_supp from `marque-applied.md` §4.7).
3. `for rw in &self.page_rewrites { ... }` — topologically-ordered DAG of PageRewrites (anti-monotone `Clear` / `FactRemove` actions are sound because the rewrite graph is acyclic per Kahn's algorithm at `Engine::new`).

This is the canonical §4.7.4 pipeline. The closure operator's
order-theoretic properties (monotone on the lattice's natural partial order;
extensive — adds facts but does not remove them; idempotent on the closed
state) are preserved by this ordering. The PageRewrites' termination
guarantee comes from the topological ordering (each rewrite fires at most
once per projection — there is no fixpoint loop at this layer, unlike
`closure`'s Kleene fixpoint). The implementation-report `§7 Principle VI`
attestation captures this correctly.

**Spot-check PASS.** No lattice-law concerns surface.

---

## §11 Bench gate

Implementation report §6 reports `lint_10kb` at 973µs against a staging
baseline that has drifted to 880-930µs (per memory
`project_bench_baseline_staleness`). The +17% vs the stale 828µs baseline
is not attributable to PR 4b-F — the inner `join_via_lattice_body` is
byte-identical (only its parameter list shrank by one parameter). The
constitutional ceiling SC-001 (16ms interactive p95) holds comfortably
at ~1ms. **No lattice-related perf concern.**

---

## §12 Summary

PR 4b-F is a clean signature consolidation that retires the dead
`&PageContext` parameters end-to-end. Every load-bearing lattice and
projection semantic is preserved:

- Pipeline ordering (join → closure → PageRewrites) — preserved.
- 10+ axis composition order — preserved.
- G-1 through G-9 cross-axis state flow — preserved.
- 7-row (collapsed-to-1) closure rule catalog — preserved.
- 9-row PageRewrite catalog (Pattern-B + Pattern-C) — preserved (data byte-identical).
- 13 per-axis lattice impl bodies — preserved.
- PR #456 `JoinSemilattice` / `MeetSemilattice` split — preserved.
- D24 `impl JoinSemilattice for CapcoMarking` retirement — preserved (not reintroduced).
- `DeclassExemptionAccumulator` non-lattice status — preserved.
- Read-only-attrs G13 sentinel — preserved verbatim.
- Same-slice contract — eliminated safely (genuinely vacuous post-Commits-1-3 per the call-graph walk in §7).
- Parity gate (74 fixtures) — passes.

**Verdict: APPROVED. Zero CRITICAL / HIGH findings.**

---

**End of review.**
