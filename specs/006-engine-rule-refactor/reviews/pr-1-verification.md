<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 1 verification record (T019)

**Source plan**: `docs/plans/2026-05-02-engine-refactor-consolidated.md` PR-1 row.
**Spec**: `specs/006-engine-rule-refactor/spec.md` FR-029.
**Task**: `specs/006-engine-rule-refactor/tasks.md` T019.

## Scope of T019

> Verify single-pass forward splice (PR #277 / #278 already landed): run
> `fix_throughput` Criterion bench; confirm R² ≥ 0.9 against PR-0 baseline
> (FR-029; PR-1).

PR 1's deliverable (the splice rewrite + the bench wiring) shipped in
PR #278; T019 is the verification step.

## What is verified

| Item | State | Evidence |
|------|-------|----------|
| Single-pass forward splice landed in `Engine::fix_inner` | **Verified** | Commit `9d5e3112 perf(engine): fix quadratic fix-apply path — replace Vec::splice with single forward pass (#278)`; merged to `main` 2026-05-02. |
| `fix_throughput` Criterion bench exists | **Verified** | `crates/engine/benches/fix_throughput.rs` (sweeps 1 MB → 100 MB, sample_size = 10, throughput = `Throughput::Bytes`). |
| `fix_throughput` wired into `scripts/bench-check.sh` (`check_fix_throughput`) | **Verified** | `scripts/bench-check.sh` defines `check_fix_throughput` and the R² ≥ 0.9 gate is keyed off `fix_throughput.r_squared_min` in `benches/baseline.json`. |
| `R² ≥ 0.9` gate currently enforced in CI | **Not verified — deferred** | The gate's call site in `bench-check.sh` was commented out by commit `bd5b84de chore: Disable fix_throughput test until we resolve the underlying issue` (2026-05-03). Per-bench verification cannot be completed until that disable is reverted. |

## Why the gate verification is deferred

The maintainer disabled the gate with the in-line comment
`# fix_throughput disabled while we work out the scaling bug` and a commit
message stating the disable is temporary "until we resolve the underlying
issue." A local run of `cargo bench -p marque-engine --bench fix_throughput`
on the verifier's hardware confirms the bench currently estimates >5 minutes
per iteration at the 5 MB sweep point — Criterion reports
`Unable to complete 10 samples in 5.0s. You may wish to increase target time
to 303.9s.` That observation is consistent with the maintainer's "scaling
bug" disable and is not consistent with the post-#278 linear-path
expectation (~50 ms / 5 MB at ~100 MB/s).

The single-pass splice itself **is** in place (the `9d5e3112` patch is on
`main` and `staging`); what is unverified right now is whether the bench
output produces R² ≥ 0.9 across the size sweep, because the bench's larger
sizes are unable to complete in a reasonable wall-clock window.

This is an as-found state, not a regression introduced by this PR or by the
in-progress refactor PRs (PR 0.5 / 0.6 / etc.).

## Recommended follow-up

A standing tracking issue should record:

1. The disabled `check_fix_throughput` call in `scripts/bench-check.sh:756`.
2. The local-bench symptom (>5 minute estimate at 5 MB).
3. Resolution criteria: either (a) restore the bench's ability to finish at
   100 MB and re-enable the gate, or (b) shrink the sweep to a size range
   where R² is still meaningful and the bench finishes in CI's job-time
   budget, then re-enable.

T019 stays `[ ]` in `tasks.md` until the gate is re-enabled and observed
green — flipping it before then would record a verification that did not
actually run.

## Status

- T019: deferred (splice landed; perf-gate verification awaits gate
  re-enable per the recommended follow-up above).
- PR 1's user-visible artifact (the splice + bench wiring) shipped in #278
  and is in production. The deferral is on the *gate enforcement*, not on
  the *splice correctness*.
