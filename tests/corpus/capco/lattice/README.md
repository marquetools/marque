<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Cross-axis dominance corpus (`tests/corpus/lattice/`)

Fixture data for cross-axis dominance property tests. Each `.txt` file
corresponds to a worked example from the lattice-design notes. Each has
an `.expected.json` sidecar pinning the post-lattice expected
diagnostics; `crates/capco/tests/lattice_corpus_runner.rs` runs the
fixtures against the per-category `Lattice` impls.

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
in lieu of portions + banner. The property-test runner dispatches on
the fixture's structural shape (presence of a CAB header vs.
portion-marking pattern).

The five fixtures cover four cross-axis dominance classes the
lattice-design doc names: classification × dissem (FOUO eviction
two-axis matrix), within-dissem (FOUO + non-FD&R), classification +
FGI + closure (multi-axis composition with ORCON ⇒ NOFORN closure),
SCI cross-system within-axis, and AEA × calendar-date heterogeneous
join. NOFORN-clears-REL-TO (the PageRewrite path) is not covered by an
in-tree fixture here.

## Why this subdirectory is not scanned by the accuracy harness

Per `crates/test-utils/src/lib.rs::fixtures_in`, the corpus walker is
parameterized by subdirectory name. The accuracy harness
(`crates/engine/tests/corpus_accuracy.rs`) scans `valid/`, `invalid/`,
`prose/`, and `mangled/` only. `lattice/` is intentionally absent from
it because these fixtures need property-test interpretation against the
per-category `Lattice` impls, not accuracy-matching against the
strict-path engine. `crates/capco/tests/lattice_corpus_runner.rs`
handles them instead, verifying each against its worked example.
