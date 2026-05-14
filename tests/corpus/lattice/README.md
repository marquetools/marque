<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Cross-axis dominance corpus (`tests/corpus/lattice/`)

Fixture data for cross-axis dominance property tests. Each `.txt` file
corresponds to a worked example from `docs/plans/2026-05-01-lattice-design.md`
§§2-8 (the lattice-design fill-in landed in PR 3.7 Stage A).

**Status (PR 3.7)**: data files only. PR 4 (per-category `Lattice` impls
+ property tests T116/T117/T118) wires this subdirectory into the
property-test runner and pins the post-lattice expected diagnostics as
`.expected.json` sidecars.

## Inventory

| Fixture | Worked-example reference | What it exercises |
|---|---|---|
| `fouo-eviction-class.txt` | §3 (b) Example 3 | FOUO evicted by classification > U (cross-axis: classification × dissem) |
| `fouo-eviction-non-fdr.txt` | §3 (b) Example 4 | FOUO evicted by non-FD&R dissem (within-axis: dissem-set rules) |
| `fgi-banner-rollup.txt` | §2 Example 2 / §6 Example 2 / §4.8.5 | FGI banner roll-up retains FGI marker on cross-classified pages (#276) |
| `sci-cross-system.txt` | §4 Example 3 | SCI cross-system canonicalization with `/` separator |
| `aea-commingling.txt` | §8 Example 2 / §E.4 | AEA exemption commingling (canned string dominates calendar date) |

## Format

The four FOUO / FGI / SCI fixtures are multi-line marking inputs:
- `(...)` — one or more portion markings (one per line if multi-portion)
- A banner line — the rolled-up classification banner expected after
  per-axis join + closure operator + PageRewrites

`aea-commingling.txt` uses a different shape because the §8 / §E.4
worked example is about Classification Authority Block (CAB)
commingling, not portion-marked banner roll-up. It contains
`Classified By` / `Derived From` / `Declassify On` metadata blocks
in lieu of portions + banner. PR 4's property-test runner is
expected to dispatch on the fixture's structural shape (presence of
a CAB header vs. portion-marking pattern) when it wires this
subdirectory in.

The five fixtures cover four cross-axis dominance classes the
lattice-design doc names: classification × dissem (FOUO eviction
two-axis matrix), within-dissem (FOUO + non-FD&R), classification +
FGI + closure (multi-axis composition with ORCON ⇒ NOFORN closure),
SCI cross-system within-axis, and AEA × calendar-date heterogeneous
join. NOFORN-clears-REL-TO (the PageRewrite path) is not yet covered
by an in-tree fixture — that case lands in PR 4 once
`Engine::project::closure()` is wired and the post-rewrite expected
state can be pinned in a `.expected.json` sidecar.

## Why this subdirectory is not yet scanned by the corpus harness

Per `crates/test-utils/src/lib.rs::fixtures_in`, the corpus walker is
parameterized by subdirectory name. The existing accuracy harness
(`crates/engine/tests/corpus_accuracy.rs`) scans `valid/`, `invalid/`,
`prose/`, and `mangled/` only. `lattice/` is intentionally absent from
the walker until PR 4 lands the property-test runner that knows how
to interpret these fixtures against the per-category `Lattice` impls.

This keeps the fixtures in-tree (so PR 4 can verify them against the
worked examples in `docs/plans/2026-05-01-lattice-design.md`) without
forcing the current strict-path engine to produce specific outputs
that the lattice impls haven't yet established.
