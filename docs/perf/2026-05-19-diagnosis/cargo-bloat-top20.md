<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# `cargo bloat` native binary attribution

> Captured 2026-05-19 on WSL2 dev host. Source: `cargo bloat --release
> -p marque --crates -n 30` and `cargo bloat --release -p marque -n 50`.
> Native CLI binary used as proxy for engine code size; `marque-extract`
> and CLI-only crates dropped in comparison since the engine code path
> dominates.

## By-crate bytes in `.text` (top 20)

| Rank | Crate | pre-pr4 KiB | head KiB | Δ KiB | Δ % |
|---|---|---|---|---|---|
| 1 | std | 755.3 | 778.9 | +23.6 | +3.1 |
| 2 | marque_capco | 217.2 | **399.4** | **+182.2** | **+83.9** |
| 3 | marque_engine | 237.9 | 257.5 | +19.6 | +8.2 |
| 4 | clap_builder | 229.0 | 229.1 | +0.1 | 0.0 |
| 5 | regex_syntax | 145.5 | 145.5 | 0.0 | 0.0 |
| 6 | aho_corasick | 144.5 | 144.5 | 0.0 | 0.0 |
| 7 | marque | 134.0 | 134.1 | +0.1 | 0.0 |
| 8 | regex_automata | 121.3 | 121.3 | 0.0 | 0.0 |
| 9 | marque_ism | **122.0** | **38.6** | **-83.4** | **-68.4** |
| 10 | marque_core | 81.1 | 90.6 | +9.5 | +11.7 |
| 11 | tokio | 70.0 | 70.0 | 0.0 | 0.0 |
| 12 | tracing_subscriber | 61.7 | 61.7 | 0.0 | 0.0 |
| 13 | toml | 58.6 | 58.6 | 0.0 | 0.0 |
| 14 | smallvec | 35.5 | 43.4 | +7.9 | +22.3 |
| 15 | marque_config | 40.7 | 40.7 | 0.0 | 0.0 |
| 16 | toml_parser | 31.2 | 31.8 | +0.6 | +1.9 |
| 17 | serde_json | 15.1 | 15.8 | +0.7 | +4.6 |
| 18 | memchr | 12.4 | 12.4 | 0.0 | 0.0 |
| 19 | heck | 8.1 | 8.1 | 0.0 | 0.0 |
| 20 | cc | 8.0 | 7.9 | -0.1 | -1.2 |

**Net engine subgraph delta (marque_capco + marque_engine +
marque_ism + marque_core):**
- pre-pr4: 658.2 KiB
- head: 786.1 KiB
- Δ: **+127.9 KiB** (+19.4%)

The marque_ism shrinkage (-83.4 KiB) is largely offsetting the
marque_capco growth (+182.2 KiB). Net engine-side: +127.9 KiB.

`smallvec` (+22.3% / +7.9 KiB) — third-largest jumper after capco
and engine; consistent with additional `SmallVec` instantiations
from the per-axis lattice constructors (each `*::from_attrs_iter`
returns a `Box<[T]>` but uses `SmallVec` internally during accumulation).

## By-function bytes in `.text` (head, top 25 marque-relevant)

Filtered to engine/capco/core/rules — toolchain and ecosystem
functions dropped.

| Rank | Crate | Function | KiB | % |
|---|---|---|---|---|
| 1 | marque_engine | `decoder::generate_candidate_bytes` | 62.6 | 2.2 |
| 2 | marque_capco | `<CapcoMarking>::join_via_lattice` | **57.9** | **2.1** |
| 3 | marque_core | `<Parser>::parse_marking_string` | 54.2 | 1.9 |
| 4 | marque_engine | `<Engine>::with_clock::<CapcoScheme>` | 46.6 | 1.7 |
| 5 | marque_engine | `<TwoPassFixer>::run` | 39.3 | 1.4 |
| 6 | marque_engine | `<Engine>::lint_with_options_internal_with_cache` | 27.9 | 1.0 |
| 7 | marque_capco | `<CapcoScheme>::project_attrs_pipeline` | 16.4 | 0.6 |
| 8 | marque_capco | `quicksort<&SmolStr, SarSet::to_marking::{closure#0}>` | 15.6 | 0.6 |
| 9 | marque_capco | `quicksort<&SmolStr, SciSet::to_markings::{closure#2}>` | 15.6 | 0.6 |
| 10 | marque_capco | `quicksort<&SmolStr, SarSet::to_marking::{closure#1}::{closure#0}>` | 15.6 | 0.6 |
| 11 | marque_capco | `quicksort<&SmolStr, SarSet::to_marking::{closure#1}::{closure#1}>` | 15.6 | 0.6 |
| 12 | marque_capco | `quicksort<(&SmolStr, &BTreeSet<SmolStr>), SciSet::to_markings::{closure#1}>` | 15.5 | 0.6 |
| 13 | marque_engine | `<DecoderRecognizer>::recognize` | 15.6 | 0.6 |
| 14 | marque_capco | `synthesize_fixes` (in engine, capco-typed) | 12.2 | 0.4 |
| 15 | marque_capco | `quicksort<&str, evaluate_sar_banner_rollup::{closure#0}>` | 12.2 | 0.4 |
| 16 | marque_capco | `evaluate_custom_by_attrs` | 11.0 | 0.4 |
| 17 | marque_capco | `<RelToOpaqueUncertainReductionSuggestRule>::check` | 10.9 | 0.4 |
| 18 | marque_capco | `<CapcoScheme>::new` | 10.8 | 0.4 |
| 19 | marque_capco | `quicksort<&SciMarking, render_structural::{closure#0}>` | 9.7 | 0.3 |
| 20 | marque_core | `parse_rel_to_with_spans` | 9.0 | 0.3 |

**Five distinct `quicksort` monomorphizations in `marque_capco`
totaling ~77 KiB.** Each per-axis lattice's `to_markings` /
`to_marking` projection sorts canonically (per CAPCO §H.4 / §H.5 /
§H.8 ordering rules). Pre-pr4 had only 2 quicksort monomorphizations
in `marque_capco` (the SAR banner-rollup quicksort, 12.2 KiB, and the
SCI render quicksort, 9.7 KiB — total 21.9 KiB). The +55 KiB delta is
new monomorphization, not algorithmic growth.

## pre-pr4 by-function spot check (marque-relevant entries)

| Crate | Function | KiB | Notes |
|---|---|---|---|
| marque_core | `<Parser>::parse_marking_string` | 59.9 | (was 54.2 KiB at HEAD, -5.7 KiB) |
| marque_ism | `<PageContext>::project` | **44.2** | **DELETED in PR 6c; replaced by `CapcoMarking::join_via_lattice` (57.9 KiB) + scheme.project_attrs_pipeline (16.4 KiB) on the new hot path** |
| marque_engine | `<Engine>::with_clock::<CapcoScheme>` | 40.4 | (was 46.6 KiB at HEAD, +6.2 KiB) |
| marque_engine | `<Engine>::lint_with_options_internal_with_cache` | 28.4 | (was 27.9 KiB at HEAD, -0.5 KiB — within noise) |
| marque_engine | `<StrictOrDecoderRecognizer>::recognize` | 14.4 | (was 15.6 KiB at HEAD, +1.2 KiB) |
| marque_capco | `<CapcoScheme>::apply_intent` | 12.4 | (not in HEAD top-50; absorbed by other paths) |
| marque_capco | `quicksort<&str, evaluate_sar_banner_rollup::{closure#0}>` | 12.2 | (still present at HEAD, unchanged) |
| marque_capco | `quicksort<&SciMarking, render_structural::{closure#0}>` | 9.7 | (still present at HEAD, unchanged) |

**Key replacement chain.**
- pre-pr4: `PageContext::project` (44.2 KiB) in marque_ism →
  amortizes the 13 `expected_*` accessors via a single resolver.
- head: `CapcoMarking::join_via_lattice` (57.9 KiB) in marque_capco
  composes 10 lattice types component-wise.

The per-function attribution shows the pre-flip path was *one*
specialized resolver carrying the merging logic; the post-flip path
distributes that logic across multiple lattice constructors, each of
which monomorphizes. The 5 new quicksort monos are the canonical-
ordering applications in `to_marking` / `to_markings` projections.
