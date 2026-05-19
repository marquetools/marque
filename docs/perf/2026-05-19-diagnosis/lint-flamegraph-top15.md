<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Flamegraph hot-path attribution (synthesized from criterion + bloat)

> **Status: synthesized, not measured.** Captured 2026-05-19 on WSL2
> dev host. Neither `cargo-flamegraph` nor `samply` was installed in
> the worktree's PATH at investigation start, and WSL2's
> `perf_event_paranoid` sandbox would have required interactive sudo
> to enable kernel `perf` access. Per the perf-engineer preflight
> §3.1's documented fallback path, this file substitutes a synthesized
> top-15 inclusive-time ranking derived from `profile_project`
> per-stage numerics + `cargo bloat` per-function sizes.

## Methodology

Each row's inclusive-time % is computed as:

```
inclusive_% = (stage_mean_ns_or_us × per_call_frequency_estimate)
              / lint_10kb_total_mean_us × 100
```

Where `lint_10kb_total_mean_us` at HEAD = 1022.8 µs. Per-call
frequencies are inferred from the bench input shape
(`build_representative_input`, 10 KB synthetic input with ~10-15
portion markings + prose, established in `lint_latency.rs`).

This is a **load-bearing approximation**, not a measurement. Numbers
to the right of the decimal should be treated as ranking signal, not
absolute attribution. The remediation plan's confidence levels reflect
this — flamegraph-derived evidence is downgraded by one tier (HIGH →
MED) where it would otherwise be load-bearing.

## Top 15 inclusive-time frames at HEAD (synthesized)

| Rank | Frame | Inclusive % (synth) | Confidence | Source |
|---|---|---|---|---|
| 1 | `Parser::parse_marking_string` | ~15-20% | MED | bloat: 54.2 KiB top-3 single function; per-portion call. Phase 2 token matching is the entry cost. |
| 2 | `Engine::lint_with_options_internal_with_cache` (frame-only cost, excluding recursive subcalls) | ~8-12% | MED | bloat 27.9 KiB; one call per `lint_10kb` invocation; orchestrates the per-portion loop. |
| 3 | `CapcoMarking::join_via_lattice` (via `scheme.project`) | ~6-10% | MED-HIGH | bloat 57.9 KiB (largest single capco function); per-page call on the post-flip hot path. profile_project phase_a measures 478ns × portion_count. |
| 4 | `CapcoScheme::project_attrs_pipeline` | ~3-5% | MED | bloat 16.4 KiB; per-page call wrapping the lattice composition. |
| 5 | Closure operator Kleene-fixpoint walk (`Phase::PageFinalization`) | ~2-4% | MED-HIGH | profile_project phase_b: 278ns mean (was 75ns at mid-flip, +270%). Empty-cone short-circuit caps worst case; per-call adds the new floor. |
| 6 | 5 × quicksort monomorphizations (lattice `to_marking` projections) | ~2-4% | MED | bloat: 5 distinct 12-16 KiB monomorphizations summing to ~77 KiB; sorts run per-axis per-page. |
| 7 | `Engine::with_clock::<CapcoScheme>` (frame-only) | ~3-4% | MED | bloat 46.6 KiB; construction-only cost. **Should not appear** in per-lint hot path — this presence in inclusive-% likely indicates engine-construction tax leaking into per-lint cost (re-check). |
| 8 | `Engine::two_pass_fix::run` (only relevant to `Engine::fix`, not lint) | ~0-1% | LOW | bloat 39.3 KiB; not in `lint_10kb` path. Listed for completeness; should be 0% in flamegraph. |
| 9 | `Decoder::generate_candidate_bytes` (only relevant to decoder dispatch) | ~0-3% | LOW | bloat 62.6 KiB (largest engine function). Strict-path lint shouldn't reach decoder; `decoder_10kb` runs this. |
| 10 | `evaluate_custom_by_attrs` (Constraint::Custom dispatcher) | ~1-3% | MED | bloat 11.0 KiB; per-rule × per-portion call frequency. ~39 `Constraint::Custom` rows. |
| 11 | Scanner `memchr` candidate detection (Phase 1) | ~3-6% | HIGH | structural — SIMD scanner runs once per byte. memchr crate (12.4 KiB) is unchanged across umbrella. |
| 12 | Page-rewrite scheduler walk | ~1-3% | MED | 27 PageRewrite rows × 1 page; topological dispatch is O(rows) per call. |
| 13 | `parse_rel_to_with_spans` | ~1-2% | MED | bloat 9.0 KiB; called by Parser path for REL TO portions. |
| 14 | Vocabulary `Arc<dyn>` dispatch (e.g., `shape_admits`) | ~0.5-2% | MED | per `baseline.json::lint_10kb._p99_note`, this is the surfaces-at-tail cost. Mean impact is small. |
| 15 | `DecoderRecognizer::recognize` (only when decoder fires) | ~0-3% | LOW | bloat 15.6 KiB; not exercised on `lint_10kb` strict path. |

## What the synthesis confirms

1. **Top 6 frames at HEAD are all new or significantly larger than at
   pre-pr4.** The parsing path (parse_marking_string, parse_rel_to_with_spans,
   memchr) is unchanged. The lattice / closure / page-rewrite pipeline is
   the structural addition, accounting for an estimated 13-21% of
   inclusive time.
2. **The quicksort monomorphizations matter.** Five separate ~15 KiB
   monomorphizations is +55 KiB native binary growth that the WASM
   monos report would have shown if names were preserved. Each one
   sorts per-axis per-page; estimated 2-4% combined inclusive time
   on the bench is consistent with their bloat profile.
3. **`Engine::with_clock::<CapcoScheme>` showing in the per-call
   inclusive list is suspicious.** Engine construction should amortize
   across the bench loop (Criterion warmup + measurement uses a single
   pre-built engine). Either this is bloat misattribution (likely; the
   46.6 KiB is "all code generated for this monomorphization" not
   "per-call cost") or there's a per-call construction tax that
   should be investigated (INVESTIGATE-tier candidate DI-3 in the
   remediation plan).

## What the synthesis cannot resolve

- **Absolute inclusive-% to within 1%.** A real flamegraph would give
  this; the synthesized numbers are ranges with 2-3% bands.
- **Indirect-call cost from `Arc<dyn Vocabulary<S>>` (PR 2 documented
  cost).** The p99 attribution is in
  `benches/baseline.json::lint_10kb._p99_note` but a flamegraph
  would have shown the vtable miss frames directly.
- **Per-portion lattice setup cost vs per-page roll-up.** The
  profile_project `phase_g_project_n1` vs `n50` sweep gives a
  rough fit; a flamegraph would attribute the per-portion fixed
  cost (the +38.9% n1 regression vs mid-flip) to specific lattice
  constructors.

## Recommended remediation for the gap

**INVESTIGATE candidate OTHER-1**: install `cargo-flamegraph` and
re-run with `perf_event_paranoid` relaxed. The capture
recommendations in the perf-engineer preflight §3.1 are operative.
Cost: ~30 minutes to install + capture three flamegraph SVGs (pre-pr4,
mid-flip, head); benefit: replaces all MED-confidence rows above
with HIGH-confidence measured attributions.

The flamegraph capture is **deferred** to a follow-up PR rather than
held in this PR's scope because (a) the bloat + criterion + profile_project
triad is sufficient to rank the top candidates with documented
confidence reductions, and (b) the install is non-trivial on WSL2
(`linux-tools-generic` apt package + kernel sysctl change + sudo
prompt) which would have either expanded this PR's scope materially
or required interactive intervention this dispatch could not provide.
