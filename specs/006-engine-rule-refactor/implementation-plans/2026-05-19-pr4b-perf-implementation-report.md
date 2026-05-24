<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-perf closeout — implementation report (2026-05-19)

> One-page summary of what the implementer agent measured, what the
> top findings are, how the deliverables map to the PM contract
> (`docs/plans/2026-05-19-pr4b-perf-pm-decisions.md` D-1 through D-10),
> and deviations from the contract. The diagnosis itself lives at
> `docs/perf/2026-05-19-diagnosis.md` (357 lines); this report does
> not duplicate it.

## What was measured

Three reference points, same WSL2 dev host, single-calendar-day capture:

| Tag | SHA | Date |
|---|---|---|
| `pre-pr4` | `18cef6c9` | 2026-05-15 (PR 9c.2, last commit before PR-4a) |
| `mid-flip` | `ebbefda0` | 2026-05-18 (PR 4b-D.2 hot-path flip) |
| `head` | `81694384` | 2026-05-19 (current branch tip) |

Captures per checkpoint (load-bearing for the diagnosis):

- `cargo bench -p marque-engine --bench lint_latency -- '^lint_10kb$'`
- `cargo bench -p marque-engine --bench lint_latency -- '^decoder_10kb_one_mangled_region$'`
- `cargo bench -p marque-engine --bench profile_project` (mid-flip + head only — bench was added in PR 4b-D.2 commit 7)
- `cargo bloat --release -p marque --crates -n 30` (pre-pr4 + head)
- `cargo bloat --release -p marque -n 50` (pre-pr4 + head, top-function attribution)
- `bash tools/wasm-size-check.sh` (pre-pr4 + head, pre-opt WASM artifact size)
- `wasm-opt -O3 --enable-bulk-memory ... ` (pre-pr4 + head, post-opt minimal config)
- p99 computed from `target/criterion/<bench>/new/sample.json`

Profiling tools attempted but unable to produce attributable output:

- `cargo-flamegraph`: not installed in worktree PATH; WSL2 sandbox
  requires `perf_event_paranoid` tweak (sudo prompt). Documented
  as candidate OTHER-1.
- `samply`: not installed.
- `twiggy monos` (WASM): returned 0 rows at all wasm-pack profile
  configurations (release-web, profiling, dev) — name section stripped
  by `--monomorphize` + `--strip-toolchain-annotations`. Documented
  as candidate MO-3 + W-MO-1.

## Top 3 findings

1. **Native `lint_10kb` cumulative regression is +80% mean / +106% p99**,
   not the +140% the user recalled. The "~1.7ms" figure matches the
   *mid-flip p99* (1691µs), captured at PR 4b-D.2 merge before PR
   4b-E + 4b-F + 6c retired the residue PageContext machinery. **HEAD
   is faster than mid-flip** by ~16%, demonstrating PR 4b-E + 4b-F +
   6c *did* recover headroom — just not enough to close the gap with
   pre-pr4.

2. **`marque_capco` native binary grew +182.2 KiB / +83.9%** (217.2 →
   399.4 KiB) across the umbrella. The two main contributors:
   `CapcoMarking::join_via_lattice` (57.9 KiB single function;
   replaces pre-pr4's `PageContext::project` at 44.2 KiB) and
   **5 distinct quicksort monomorphizations** in the per-axis lattice
   `to_marking`/`to_markings` projections (totaling ~77 KiB; pre-pr4
   had 2 quicksort monos totaling 21.9 KiB).

3. **WASM growth is +94 KB pre-opt / +90 KB post-opt** — modest at
   +7.7%/+7.9%, **not the +400 KB** the user reported. The
   discrepancy is unresolved; likely cross-host (WSL2 vs GHA) or
   mid-PR-4b-era measurement basis. Candidate OTHER-2 (GHA-side
   re-capture) is the recovery path.

## Mapping to PM contract

| PM decision | Deliverable in this PR | Notes |
|---|---|---|
| D-1 (PR shape: diagnosis-only, multi-PR remediation lane) | `docs/perf/2026-05-19-diagnosis.md` + 4 supporting artifacts; no `crates/*/src/**` edits | Verified via `git diff origin/staging -- 'crates/*/src/**' \| wc -l` (expected: 0) |
| D-2 (single ranked findings doc, 25-row max, per-candidate fields) | Diagnosis §5 table has 17 rows (1 EXECUTE, 16 INVESTIGATE incl. OTHER infra candidates — counts updated post-R1 review fixup; original draft had 6 EXECUTE / 10 INVESTIGATE, R1 reviewer flagged D-8 noise-floor violations and savings-vs-bench reconciliation mismatches that moved 5 rows from EXECUTE to INVESTIGATE) | Under cap |
| D-3 (env-var branch-conditional CI gate skip) | `tools/wasm-size-check.sh` (env override); `.github/workflows/ci.yml` (branch-conditional `MARQUE_BENCH_SKIP_REGRESSION` + `MARQUE_WASM_SKIP_REGRESSION`) | Both env injections use exact-branch match (`refs/heads/refactor-006-pr-4b-perf-closeout`), NOT prefix. Post-R2/R3 fixup the match covers both `github.ref` (push events) and `github.head_ref` (`pull_request` events), mirroring the existing branch-gate pattern at lines 84, 166, 249 of `ci.yml` — without the `head_ref` arm the skip would silently fail to fire on PR-event runs because `github.ref` on those is `refs/pull/N/merge`. |
| D-4 (WASM measurement basis pinned) | Diagnosis §2.3 + `2026-05-19-diagnosis/twiggy-monos-top20.md` | Both pre-opt and post-opt sizes captured at both checkpoints |
| D-5 (PR-template bench-delta block) | `.github/PULL_REQUEST_TEMPLATE.md` | 62 lines; bench-delta block fills in `lint_10kb` before/after + hardware |
| D-6 (profiling artifact home) | `docs/perf/2026-05-19-diagnosis.md` + `docs/perf/2026-05-19-diagnosis/{lint-flamegraph-top15,cargo-bloat-top20,twiggy-monos-top20,criterion-checkpoints}.md` | Text only, 68 KB total committed (under 100 KB budget) |
| D-7 (three reference points, same hardware) | pre-pr4 / mid-flip / head, all WSL2 dev | Mid-flip captured as part of D-7's unconditional triad; load-bearing for contradiction-2 resolution (see deviations §1). |
| D-8 (EXECUTE vs INVESTIGATE tier semantics) | Diagnosis §5 table populates `tier` per-row; OTHER-* candidates are pure infra/measurement | 1 EXECUTE-tier candidate (LA-1) post-R1 fixup; passes D-8 via WASM-floor arm (40-60 KB ≥ 30 KB). Total EXECUTE-tier estimated savings: ~5-20µs lint + 40-60 KB WASM. EXECUTE alone does not close the 453µs lint regression — closing it requires elevating INVESTIGATE candidates after flamegraph capture (OTHER-1). |
| D-9 (no new gated benches) | None added | Existing `profile_project.rs` sufficient |
| D-10 (PR-description shape) | PR body will be authored at submission with TL;DR pointer to diagnosis | Not in this PR's diff |

## Deviations from PM contract

None material. Three minor scope notes (post-R1/R2/R3 review
fixup updates also applied below):

1. **Mid-flip checkpoint captured.** D-7 lists three reference
   points (pre-pr4, mid-flip, head) as the unconditional scope and
   names 4b-B + 6c as conditional fourth/fifth checkpoints. The
   mid-flip capture is part of D-7's unconditional triad — not a
   deviation. (R3 review-2026-05-19 caught an earlier version of
   this note that conflated D-7's three-point scheme with the
   perf preflight's older four-point scheme; corrected here.)
   Capturing the mid-flip data was load-bearing for resolving
   contradiction 2 (the PR 4b-E recovery question) — without it,
   the diagnosis could not have demonstrated PageContext retirement
   actually did help.
2. **Flamegraph synthesized rather than measured.** Documented as
   candidate OTHER-1. The bloat + criterion + profile_project triad
   was sufficient to rank top candidates with documented confidence
   reductions; full flamegraph capture is a follow-up that unblocks
   the INVESTIGATE-tier candidates with `OTHER-1` in their
   `dependencies` column.
3. **`twiggy monos` returned 0 rows.** Documented as candidate MO-3
   + W-MO-1. Substitute attribution via native `cargo bloat` is
   load-bearing; the structure of the regression is shared across
   native and WASM. WASM-specific savings ranges are downgraded by
   one confidence tier (HIGH → MED) per the documented gap.

## Files touched (exact list)

| Path | Kind | Lines |
|---|---|---|
| `docs/perf/2026-05-19-diagnosis.md` | NEW (main findings doc) | 357 |
| `docs/perf/2026-05-19-diagnosis/criterion-checkpoints.md` | NEW (supporting numerics) | 145 |
| `docs/perf/2026-05-19-diagnosis/cargo-bloat-top20.md` | NEW (native attribution) | 113 |
| `docs/perf/2026-05-19-diagnosis/twiggy-monos-top20.md` | NEW (WASM attribution + gap) | 133 |
| `docs/perf/2026-05-19-diagnosis/lint-flamegraph-top15.md` | NEW (synthesized top-frames) | 108 |
| `docs/plans/2026-05-19-pr4b-perf-implementation-report.md` | NEW (this file) | ~ |
| `docs/plans/2026-05-19-pr4b-perf-pm-decisions.md` | NEW (PM contract, pre-existing) | — |
| `docs/plans/2026-05-19-pr4b-perf-preflight-architect.md` | NEW (architect preflight, pre-existing) | — |
| `docs/plans/2026-05-19-pr4b-perf-preflight-attribution.md` | NEW (attribution preflight, pre-existing) | — |
| `docs/plans/2026-05-19-pr4b-perf-preflight-performance.md` | NEW (perf preflight, pre-existing) | — |
| `tools/wasm-size-check.sh` | MOD (add `MARQUE_WASM_SKIP_REGRESSION` env override) | +14 |
| `.github/workflows/ci.yml` | MOD (branch-conditional env-var injection on `bench-check` + `wasm-size-check` steps) | +20 |
| `.github/PULL_REQUEST_TEMPLATE.md` | NEW (PR template with hot-path perf delta block) | 62 |

Note: `specs/006-engine-rule-refactor/tasks.md` was NOT modified —
the perf closeout does not map cleanly to a single 006 task ID (it
spans the whole PR-4-to-6 cumulative scope), and no STATUS notes
were required by the PM contract for this PR.

## Verification

- `git diff origin/staging -- 'crates/*/src/**' | wc -l` = 0 (verified).
- `du -sh docs/perf/` = 68 KB (under 100 KB cap).
- `wc -l docs/perf/2026-05-19-diagnosis.md` = 357 (under 800-line cap).
- `bash -n tools/wasm-size-check.sh` = OK (syntax).
- `MARQUE_WASM_SKIP_REGRESSION=1 bash tools/wasm-size-check.sh` = OK
  (env override exits 0 cleanly, prints `OK (skipped by env var on this branch)`).

## Provenance

Authored 2026-05-19 against branch `refactor-006-pr-4b-perf-closeout`
off `origin/staging` @ `81694384`. Implementation agent (this report's
author) followed the PM contract D-1 through D-10 + the two preflights
(performance + architect) + the attribution walkdown.
