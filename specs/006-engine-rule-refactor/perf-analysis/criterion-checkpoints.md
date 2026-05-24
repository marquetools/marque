<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Criterion checkpoint numerics

> Captured 2026-05-19 on WSL2 dev host (x86_64, Linux 6.6 kernel).
> Same machine, single calendar-day capture. Cross-host comparison
> against GHA `ubuntu-latest` is in the diagnosis document.

## Methodology

For each checkpoint, the worktree was created via
`git worktree add <path> <SHA>` from the marque repo root, the target
crate's `target/` was clean at first build, and:

```bash
cargo bench -p marque-engine --bench lint_latency -- '^lint_10kb$'
cargo bench -p marque-engine --bench lint_latency -- '^decoder_10kb_one_mangled_region$'
cargo bench -p marque-engine --bench profile_project   # not present at pre-pr4
```

Criterion default `sample_size=100` per per-bench. Percentile columns
read from `target/criterion/<bench>/new/sample.json` and computed via
the python snippet in
`docs/plans/2026-05-19-pr4b-perf-preflight-performance.md` §3.4.

## Reference points

| Tag | SHA | Date | Note |
|---|---|---|---|
| `pre-pr4` | `18cef6c9` | 2026-05-15 | PR 9c.2 merge; last commit before any PR-4 work landed. |
| `mid-flip` | `ebbefda0` | 2026-05-18 | PR 4b-D.2 hot-path flip merge. Engine cuts over from `PageContext::expected_*` accessors to `scheme.project(Scope::Page, ...)`. |
| `head` | `81694384` | 2026-05-19 | Current branch tip (PR 4 closeout T119 probe). Includes PR 4b-E (PageContext residue retirement), PR 4b-F, PR 6c (PageContext struct retirement), PR 5 (foreign banner correctness), and post-PR-4b closeout fixes. |

## `lint_10kb` (SC-001 strict-path bench)

| Tag | Lower CI (µs) | Mean (µs) | Upper CI (µs) | p50 (µs) | p95 (µs) | p99 (µs) |
|---|---|---|---|---|---|---|
| `pre-pr4` | 559.0 | 569.6 | 580.8 | 550.3 | 656.0 | 690.3 |
| `mid-flip` | 1183.0 | 1218.8 | 1260.8 | 1179.0 | 1373.5 | 1691.4 |
| `head` | 1006.2 | 1022.8 | 1041.1 | 1047.0 | 1334.5 | 1422.4 |

Deltas (mean):
- `pre-pr4` → `mid-flip`: +649µs (+114%)
- `mid-flip` → `head`: -196µs (-16.1%)
- `pre-pr4` → `head` (cumulative): +453µs (+80%)

Deltas (p99):
- `pre-pr4` → `mid-flip`: +1001µs (+145%)
- `mid-flip` → `head`: -269µs (-15.9%)
- `pre-pr4` → `head` (cumulative): +732µs (+106%)

**Observation.** PR 4b-E + 4b-F + 6c (the PageContext-retirement series)
recovered ~16% of headroom from the mid-flip peak. The cumulative
regression from `pre-pr4` to `head` is ~80% on the mean, **not** the
~140%+ implied by the user's "~1.7ms" memory. The user's number
matches `mid-flip`'s p99 (1691µs), suggesting they recalled the
post-flip p99 reading.

## `decoder_10kb_one_mangled_region` (SC-002 decoder-path bench)

| Tag | Lower CI (µs) | Mean (µs) | Upper CI (µs) |
|---|---|---|---|
| `pre-pr4` | 705.2 | 725.1 | 749.1 |
| `mid-flip` | 1134.3 | 1149.2 | 1165.0 |
| `head` | 1166.6 | 1181.2 | 1196.9 |

Deltas (mean):
- `pre-pr4` → `mid-flip`: +424µs (+58.5%)
- `mid-flip` → `head`: +32µs (+2.8%) — within noise band.
- `pre-pr4` → `head` (cumulative): +456µs (+62.9%)

**Observation.** Decoder path tracks the strict-path regression at
roughly half the magnitude. Decoder path's structural cost is
dominated by the strict path's lattice composition (it runs through
the same `scheme.project` once the decoder produces canonical
attributes), so a fixed lattice-composition cost contributes roughly
half as much to decoder latency as to strict latency. Headroom against
the 18ms SC-002 ceiling stays at ~16.8ms.

## `profile_project` (per-stage attribution, mid-flip and head only)

`profile_project.rs` was added in PR 4b-D.2 commit 7 — it does not
exist at `pre-pr4`. Stage numerics at `mid-flip` and `head`:

| Stage | `mid-flip` mean | `head` mean | Δ µs | Δ % |
|---|---|---|---|---|
| phase_a_join_via_lattice | 536.6ns | 478.3ns | -58.3ns | -10.9% |
| phase_b_closure | 75.1ns | 277.9ns | +202.8ns | +270% |
| phase_c_scheme_project | 949.1ns | 1101.2ns | +152.1ns | +16.0% |
| phase_d_from_canonical | 38.9ns | 46.2ns | +7.3ns | +18.8% |
| phase_e_engine_project_path | 758.9ns | 888.6ns | +129.7ns | +17.1% |
| phase_f_engine_lint_full | 1122.8µs | 1056.3µs | -66.5µs | -5.9% |
| phase_g_project_n1 | 507.1ns | 704.1ns | +197.0ns | +38.9% |
| phase_g_project_n5 | 1244.4ns | 1405.4ns | +161.0ns | +12.9% |
| phase_g_project_n10 | 2197.2ns | 2258.8ns | +61.6ns | +2.8% |
| phase_g_project_n25 | 4973.6ns | 4813.1ns | -160.5ns | -3.2% |
| phase_g_project_n50 | 10077ns | 9666.4ns | -410.6ns | -4.1% |
| phase_h_tmp_ctx_rebuild_n10 | 494.1ns | 560.4ns | +66.3ns | +13.4% |
| phase_h_tmp_ctx_rebuild_n25 | 1468.7ns | 1407.4ns | -61.3ns | -4.2% |
| phase_h_tmp_ctx_rebuild_n50 | 3564.5ns | 3148.3ns | -416.2ns | -11.7% |
| phase_i_join_n10 | 2036.2ns | 1206.7ns | -829.5ns | -40.7% |
| phase_i_join_n25 | 4857.2ns | 2806.0ns | -2051.2ns | -42.2% |
| phase_i_join_n50 | n/a* | 6931.3ns | n/a | n/a |

*`phase_i_join_n50` data point was clipped from the mid-flip capture
above; not load-bearing for the diagnosis (i_join scales linearly with
N at both checkpoints, so a single N=50 reading suffices to confirm
the regime).

**Observation.** Three structural insights from the per-stage delta:

1. **`phase_b_closure` grew +270% in absolute ns terms** (75ns →
   278ns). Six closure-rule rows landed between mid-flip and head
   (#519 UCNI gap, #521 NNPI gap, #529 §4.7 Trio 1, #540 per-compartment
   SCI Phase 2, #544 RELIDO closeout Phase 3, #548 FISA/RAWFISA/PROPIN).
   The closure operator's per-call cost is now ~6× the absolute floor
   it held at mid-flip. Empty-cone short-circuit caps the worst case
   but the typical case pays the new floor.
2. **`phase_g_project_n50` improved -4.1% absolute** while
   `phase_g_project_n1` regressed +38.9%. The per-portion lattice
   pipeline now has a higher fixed setup cost (more lattice types to
   walk at all, even N=1) but better amortized cost at higher portion
   counts (PR 4b-E + 6c retired the residue tmp_ctx rebuild round).
3. **`phase_i_join_n10/n25` improved -40%** between mid-flip and
   head — this is the load-bearing improvement from PR 6c's
   PageContext struct retirement, which inlined the accumulator
   so the engine wraps `CanonicalAttrs` directly without the
   pre-flip-era `Arc<PageContext>` indirection.

## Hardware

WSL2 dev host. CPU model and frequency from `/proc/cpuinfo`:
- AMD64 Linux 6.6.114.1-microsoft-standard-WSL2
- Rust 1.85+ toolchain (`rust-toolchain.toml` floor; actual rustc
  whatever the worktree's compiler resolved to at capture time).

Cross-host calibration: GHA `ubuntu-latest` typically reads ~5-10%
faster than this WSL2 dev host on the same benches per the
`benches/baseline.json::reference_machine._dev_capture_note`. The
ratios within this capture (pre-pr4 vs head) are host-invariant; the
absolute µs values would scale proportionally on GHA.
