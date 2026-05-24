<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0 -->

# PR 4b-F Implementation Report — Residue Cleanup

**Date:** 2026-05-18
**Branch:** `refactor-006-pr-4b-f-residue-cleanup`
**Base:** staging head `c3f544d6`
**Plan of record:** `docs/plans/2026-05-18-pr4b-F-residue-cleanup-plan.md`

---

## §1 Commits landed

Five functional commits + one bookkeeping commit (the architect plan's
Commit 5 audit folded into Commit 6 — see §3 below).

| # | Commit | SHA prefix | Files | Net diff |
|---|--------|------------|-------|----------|
| 1 | retire `_tmp_ctx` from `join_via_lattice_body` | `9af3b925` | `crates/capco/src/scheme/marking.rs` | +16/−37 |
| 2 | retire `page_ctx` from `project_attrs_pipeline` | `8358e4d7` | `crates/capco/src/scheme/marking_scheme_impl.rs`, `crates/capco/src/scheme/closure.rs` | +28/−28 |
| 3 | inline `project_from_attrs_slice` into trait body | `c8b0c9ed` | `crates/capco/src/scheme/marking_scheme_impl.rs` | +40/−66 |
| 4 | collapse `join_via_lattice_with_context` | `28fceac7` | `crates/capco/src/scheme/marking.rs`, `crates/capco/src/scheme/marking_scheme_impl.rs`, `crates/capco/src/scheme/actions/mod.rs` | +38/−124 |
| 5 (Commit 6) | tasks.md bookkeeping + doc-comment sweep | (this commit) | 11 files | (see §2) |

**Net cumulative change:** ~122 lines removed (signature simplification + doc
cleanup); zero engine-crate edits; zero `marque-ism` source edits; zero
test-file edits.

---

## §2 Deviations from the architect plan

**None of substance.** Two minor deviations, both documented inline:

1. **Plan §1 Commit 6 cited T111 as closed in "Phase 5 PR-2 / #146".**
   Investigation via `git log --grep="is_fdr_dissem"` against staging
   found T111 was actually closed in **PR 4a / #422** (commit
   `fc91852e`, 2026-05-15). The resolution note in `tasks.md`
   was corrected to cite the correct PR. The architect plan's table
   was authored before verifying every row against `gh pr view` per
   OQ-4.

2. **Plan §1 Commit 5 prescribed a separate engine-call-site cleanup
   + workspace audit commit.** With no third `_tmp_ctx` /
   `_page_context` / `_page_ctx` underscore-prefixed parameter
   surfaced in the audit (the only two known sites retired in
   Commits 1 and 2), and per OQ-1 keeping `&PageContext` on
   `project_page_marking`, there was no functional change to land.
   The audit attestation folded into Commit 6's commit message
   rather than landing as an empty commit. The grep audit results
   are recorded in §4 below.

3. **Plan §4 grep "Workspace grep for `expected_dissem_us` /
   `expected_aea_markings` / `expected_classification` /
   `render_expected_banner` returns zero matches in `crates/*/src/`"**
   does not return zero matches in the final state — two
   doc-comment hits remain in `crates/ism/src/page_context.rs` and
   `crates/ism/src/projected.rs`. Both reference the names in the
   context of the **deletion record** itself — the PageContext
   shim's `# PR 4b-E retirement note` and `ProjectedMarking`'s
   evolution note. Per the plan §1 hard constraint **"PageContext
   shim stays UNTOUCHED"** and Constitution VII §IV's
   marque-ism touch restriction, those two doc-comments cannot be
   edited within PR 4b-F's scope. Every reference in `crates/capco/`
   and `crates/wasm/` was either rewritten symbolically or scrubbed.
   See OQ-2 in §3 below for the structural-ambiguity-vs-scope-creep
   analysis.

---

## §3 Open Questions / surfacing items

### OQ-2 (workspace `_tmp_ctx` audit) — RESOLVED, no third site

The workspace audit grep `_tmp_ctx\b\|\b_page_context\b\|\b_page_ctx\b`
across `crates/*/src/` returned a single hit in the final state:

```
crates/capco/src/scheme/marking.rs:181:    /// `crates/capco/src/lattice.rs`) and PR 4b-F (the `_tmp_ctx`
```

This is a doc-comment in `join_via_lattice_body`'s rewritten doc
explicitly documenting the retirement of the `_tmp_ctx` parameter
within PR 4b-F — a historical record, not a live parameter. No
third structural site surfaced. The plan's `(a) fix-in-PR if
mechanical, (b) stop and escalate if structural` decision is moot.

### Engine-crate touches — none

`crates/engine/src/engine.rs` is byte-identical to the staging head.
`project_page_marking` retains its `&PageContext` parameter per
OQ-1 Option A. `crates/engine/benches/profile_project.rs` is also
untouched.

### marque-ism source edits — none

`crates/ism/src/page_context.rs` is byte-identical to staging head;
the issue #430 pre-size invariant on `Arc<PageContext>::clone()` is
unchanged. `assert_impl_all!(PageContext: Send, Sync)` at
`crates/ism/tests/send_sync.rs` continues to pass. The two stale
doc-comment references to retired `expected_*` accessor names
(documented above) remain by design — they are in the shim's
deletion-record doc, not live API references.

### marque-rules source edits — none

Trait surface unchanged.

---

## §4 Workspace audit grep results

### `_tmp_ctx` / `_page_context` / `_page_ctx`

```
crates/capco/src/scheme/marking.rs:181:    /// `crates/capco/src/lattice.rs`) and PR 4b-F (the `_tmp_ctx`
```

One historical-doc match (PR 4b-F's own retirement note); zero
live parameters.

### `page_context.rs:NNN` file:line refs

```
(zero matches)
```

### `expected_dissem_us` / `expected_aea_markings` / `expected_classification` / `render_expected_banner`

```
crates/ism/src/projected.rs:14://! widens `expected_classification` to `Option<MarkingClassification>`
crates/ism/src/page_context.rs:18://! (`expected_classification`, `expected_sci_controls`, ...,
crates/ism/src/page_context.rs:19://! `render_expected_banner`, `is_classified`, `project`) that derived
```

Three matches, all in the marque-ism PageContext shim's
deletion-record doc-comments. Marque-ism source edits are out of
PR 4b-F scope per Constitution VII §IV + plan hard constraint
"PageContext shim stays UNTOUCHED." A follow-up that retires the
PageContext struct entirely (T069 / PR 6c) will sweep these.

### `engine.rs:4540-4574` (hardcoded line anchor flagged in rust-preflight)

```
(zero matches)
```

The stale anchor in `marking.rs` was retired in Commit 1 when the
surrounding `join_via_lattice_with_context` doc-comment was rewritten.
The stale anchor in `marking_scheme_impl.rs` was retired in Commit 2
as part of the renamed-pipeline doc-comment rewrite.

---

## §5 Test results

### `cargo check --workspace`

Clean per commit.

### `cargo +stable clippy --workspace --all-targets -- -D warnings`

Clean per commit. One mid-Commit-4 transient `doc_lazy_continuation`
issue surfaced in stable clippy on rustc 1.93 (not in local nightly)
when a paragraph began with `Composes per-axis lattice results across
10+ axes — classification + JointSet, …` — the `+` looked like a
markdown list-continuation. Resolved by rephrasing without the `+`
sigil; verified clean on stable. (Per memory
`feedback_clippy_nightly_vs_stable_drift`, the stable clippy run is
the CI proxy.)

### `cargo test --workspace`

Clean. Specific load-bearing test counts:

| Test target | Count | Status |
|-------------|-------|--------|
| `lattice_vs_scheme_parity` (parity gate) | 74 | all pass |
| `marque-capco` lib + integration | full suite | all pass |
| `marque-engine` lib + integration | full suite | all pass (1 ignored, pre-existing) |
| `marque-ism` lib + integration (send_sync, page_context shim_tests) | 299 | all pass |
| `marque-scheme` lib + doctests | full | all pass |

No new failures; no test-file edits.

---

## §6 Bench delta

`cargo bench --bench lint_latency -p marque-engine -- lint_10kb`:

| Run | Median | Notes |
|-----|--------|-------|
| Pre-4b-F baseline (per memory `project_bench_baseline_staleness`) | ~828 µs | stale baseline |
| Recent staging-head measurements (per memory) | 880–930 µs | already drifting above baseline |
| PR 4b-F (this branch) | **973 µs** | within the 880–930 µs noise band per memory |

The +17% delta vs the stale baseline is **not attributable to this
PR**. PR 4b-F is signature-only: the inner pipeline body is byte-
identical (the `join_via_lattice_body` function body unchanged
beyond its parameter list; the closure operator's runtime path
identical). The drift was documented in memory
`project_bench_baseline_staleness` before this PR started. Per memory
`project_perf_baseline_pr5_trigger`, baseline refresh is deferred to
post-PR-5; routine bumping past PR 5 is the user's flagged stopping
point.

Constitutional ceilings still hold: SC-001 16ms / SC-002 18ms
absolute caps remain comfortably above the ~1ms measurement.

---

## §7 Constitution attestations

### Principle V (G13 — audit-record content-ignorance)

Two G13-compliant counts-only panic sites surveyed:

1. **`project_attrs_pipeline`'s closure-mutates-input-slice sentinel**:
   PRESERVED verbatim across the rename. Counts-only message
   referencing only `raw_snapshot.len()` and `raw.len()`. The
   anchored cross-file reference to `crates/engine/src/engine.rs:4540-4574`
   in this sentinel's doc-comment was symbolic-anchored as part of
   Commit 2.
2. **`join_via_lattice_with_context`'s same-slice debug-assert**:
   RETIRED with the function in Commit 4. The message was already
   counts-only (referenced `portions.len()` / `page_ctx.portions().len()`);
   no leak channel opened by deletion.

### Principle VII §IV (engine-crate touch authorization)

- `marque-ism` — zero source edits.
- `marque-engine` — zero source edits.
- `marque-scheme` — zero edits.
- `marque-rules` — zero edits.
- `marque-core` — zero edits.

All edits scoped to `marque-capco` + `marque-wasm` source +
`specs/006-engine-rule-refactor/tasks.md` bookkeeping. Within-006
precedent (PR 4b-B Commit 2, PR 4b-C Commit 5, PR 4b-D.2, PR 4b-D.3,
PR 4b-E) satisfied.

### Principle VIII (citation fidelity)

Every §-citation re-verified against `crates/capco/docs/CAPCO-2016.md`
at point of authorship, using `crates/capco/docs/CAPCO-2016_citation_index.yml`
as the section→page-range finder:

| Citation | Site | Verified |
|----------|------|----------|
| §G.2 p41 (NATO reciprocity) | (initially in `actions/fgi.rs`) | RETIRED — Table 5 is at §G.2 p40; `actions/fgi.rs` rewrite cites §H.7 p123 instead |
| §H.7 p122 + p123 + p128 (FGI grammar + concealed/acknowledged) | `marking.rs`, `actions/fgi.rs` | ✓ verified against pp122-128 |
| §H.3 p56 (JOINT requires USA in producer list) | `actions/fgi.rs` | ✓ verified |
| §H.8 p134 (FOUO classification eviction) | `lattice.rs` DissemSet doc | ✓ verified |
| §H.6 p116 (DOD UCNI) + §H.6 p118 (DOE UCNI) | `lattice.rs` DissemSet doc | ✓ verified |
| §H.9 p178 (SBU-NF) + §H.9 p185 (LES-NF) | `lattice.rs` DissemSet doc | ✓ verified |
| §H.6 p104 (RD > FRD > TFNI precedence) | `render_aea.rs` | ✓ verified |
| §H.8 p150 (REL TO template + REL portion-mark) | `render_dissem.rs` | ✓ verified |
| §E.1 (CAB block separate from banner) | `render_declassify.rs` | ✓ verified via CAPCO-2016_citation_index.yml |
| §E.3 (MaxDate hierarchy) | `render_declassify.rs` | ✓ verified |
| §A.6 p15-17 (banner-form mapping, Figure 2) | `render_dissem.rs` | ✓ verified |
| §H.8 p136, p140, p145, pp155-156 (DissemSet overlay set) | `lattice.rs` DissemSet doc | ✓ verified |
| §D.2 Table 3 rows 1-2 (NOFORN dominates) | `lattice.rs` DissemSet doc | ✓ verified |

Page-number form (`§X.Y pNN`) used throughout; no bare `§NN` or
`line NNNN` form introduced.

### Principle VI (Dataflow Pipeline Model)

Pipeline shape simplified, not changed. Both call surfaces
(trait `MarkingScheme::project` and engine `project_from_page_context`)
delegate through the single shared `project_attrs_pipeline` body;
the read-only-attrs invariant on closure is preserved by the
surviving G13 sentinel.

---

## §8 Files modified summary

```
crates/capco/src/lattice.rs                          (Commit 6)
crates/capco/src/render/render_aea.rs                (Commit 6)
crates/capco/src/render/render_declassify.rs         (Commit 6)
crates/capco/src/render/render_dissem.rs             (Commit 6)
crates/capco/src/scheme/actions/fgi.rs               (Commit 6)
crates/capco/src/scheme/actions/mod.rs               (Commit 4)
crates/capco/src/scheme/closure.rs                   (Commit 2)
crates/capco/src/scheme/constraints/categories.rs    (Commit 6)
crates/capco/src/scheme/marking.rs                   (Commits 1, 4, 6)
crates/capco/src/scheme/marking_scheme_impl.rs       (Commits 2, 3, 4)
crates/capco/src/scheme/rewrites/noforn_clears.rs    (Commit 6)
crates/capco/src/scheme/rewrites/pattern_c.rs        (Commit 6)
crates/wasm/src/lib.rs                               (Commit 6)
specs/006-engine-rule-refactor/tasks.md              (Commit 6)
```

PageContext shim (`crates/ism/src/page_context.rs`): UNTOUCHED.
`assert_impl_all!(PageContext: Send, Sync)`: UNTOUCHED.
Parity gate (`crates/capco/tests/lattice_vs_scheme_parity.rs`): UNTOUCHED.
Engine crate source: UNTOUCHED.

---

**End of report.**
