<!-- 
SPDX-FileCopyrightText: 2026 Knitli Inc 

SPDX-License-Identifier: MIT OR Apache-2.0
-->
<!--   
Jules:  Note that the current year is 2026. That is not an error. You should check the actual current date before recording a date in this log. Remember that your training was over a year ago so 2026 might 'feel' like the future, but it is the present.
-->
# Bolt Journal

## 2026-04-23 - BTreeSet Bulk Insertion
**Learning:** Manual nested loops using `.insert()` on `BTreeSet` for nested structures (like compartments and sub-compartments) prevents bulk allocation optimizations and increases redundant traversals.
**Action:** Use `.extend()` combined with iterator chains (`.map()` or `.cloned()`) when populating sets or collections from nested structures to leverage iterator optimizations and bulk insertions.

## 2026-04-23 - SPDX License Headers
**Learning** New files in this repository require SPDX license headers. Documentation and config files are `MIT OR Apache-2.0` while source code are `LicenseRef-MarqueLicense-1.0`.
**Action:** When creating a new file, ensure it has license and copyright headers in the SPDX format.

## 2026-04-28 - Performance PRs Require Committed Benchmarks
**Learning:** Constitution Principle I (Uncompromising Performance) requires every performance decision to be backed by a Criterion benchmark **committed to the repo**. Microbenchmarks run locally and "cleaned up before submission" do not satisfy this — the next reviewer cannot reproduce them, and the next refactor cannot detect a regression against them. PRs whose justification is a perf claim without a reproducible bench will be closed regardless of whether the change itself is correct.
**Action:** When proposing a performance optimization, add a Criterion bench under the relevant crate's `benches/` directory (e.g., `crates/capco/benches/`, `crates/engine/benches/`) that exercises the changed code path, commit it in the same PR, and quote its `cargo bench` numbers in the PR body. If the change is too small to measure end-to-end (sub-1% against the SC-001 16ms p95 budget), say so explicitly and frame it as a code-quality cleanup rather than a perf PR.

## 2026-05-18 - Missing performance impact measurements
**Learning:** All PRs presented as performance improvements require measurable impact numbers. A code change might look faster (e.g. `extend()` instead of manual loop `insert()`), but if there is no measurable impact it is not considered a performance PR. Submitting such an optimization without providing benchmarks violates the Uncompromising Performance principle.
**Action:** Always provide reproducible Criterion benchmark numbers. If the change has negligible impact but improves code quality, label the PR as a 'code-quality cleanup', not a performance improvement, or do not create a PR if asked to only provide performance improvements.
