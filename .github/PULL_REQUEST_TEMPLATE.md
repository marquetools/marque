<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

Fill in the sections below. Delete sections that don't apply, but keep
the "Hot-path perf delta" block if your PR touches any of the listed
engine paths — perf-touching PRs without measurement data are not
reviewable for regression risk.
-->

## Summary

<!-- One-paragraph description of what this PR does and why. -->

## Motivation

<!-- Link to the issue, lane plan, or PM contract this PR implements.
For umbrella sub-PRs, link the sub-PR plan in `docs/plans/`. -->

## Changes

<!-- Bullet list of the concrete changes. Cite §-references using
CAPCO-2016 §X.Y pNN form per Constitution VIII. -->

## Testing

<!-- How you verified the change. Include `cargo test` / `cargo clippy`
results. Note any new test fixtures added. -->

## Hot-path perf delta (engine-touching PRs only)

<!--
Required if this PR modifies:
  - `crates/engine/src/**`
  - `crates/scheme/src/**`
  - `crates/capco/src/**` — specifically `lattice.rs`, `scheme/marking.rs`,
    `scheme/closure.rs`, `scheme/marking_scheme_impl.rs`, `rules*.rs`
  - `crates/ism/src/**`
  - `crates/core/src/**`

Capture `lint_10kb` mean ± CI by running:

    cargo bench -p marque-engine --bench lint_latency -- '^lint_10kb$'

on this branch and on its base (`origin/staging` or the parent branch).
Same host, same calendar day. WSL2 dev numbers are fine for
local capture; CI re-capture lands the GHA-authoritative number.

Delete this entire section if the PR does NOT touch the listed paths.
-->

- `lint_10kb` before this branch: `X µs` (mean ± CI), hardware: `<profile>`
- `lint_10kb` after this branch:  `Y µs` (mean ± CI), hardware: `<profile>`
- Rationale: <one-line why any non-noise delta is acceptable, or "no
  measurable delta beyond CI noise band">

## Reviewer attestation

<!-- Optional. Fill in if the PR carries the marque pre-flight + reviewer
chain (architect / Rust / lattice / code review). Otherwise leave
blank or delete this section. -->
