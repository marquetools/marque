<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Flamegraph hot-path attribution (measured)

> **Status: measured.** Captured 2026-05-19 on WSL2 dev host
> (`Linux 6.6.114.1-microsoft-standard-WSL2`) via `samply 0.13.1`
> at 997 Hz against staging tip
> [`535d1f48`](https://github.com/marquetools/marque/commit/535d1f48)
> (PR #600 / LA-1 merged). Supersedes the prior synthesized
> top-15 (preserved verbatim in §6 below). Closes #583.
>
> **Two capture passes were taken.** The first (debug-assertion-on)
> overstated `CanonicalAttrs` lifecycle cost by including
> debug-only `raw.to_vec()` snapshots that don't exist in release
> builds. The second (debug-assertion-off, release semantics, see
> methodology §1) is the load-bearing capture for production
> attribution and powers the tables below. The debug-assert capture
> is preserved in §7 for methodological comparison and as a
> contamination case study.
>
> Capture and post-processing scripts are committed under
> [`tools/perf/`](../../../tools/perf/). Raw `.json.gz` samply
> profiles are ephemeral per PM contract D-6 and are not committed.

## 1. Methodology

```bash
# 1. Rebuild benches with symbols AND debug-assertions OFF.
#    `[profile.bench]` inherits `release` but sets
#    `debug-assertions = true` (and `debug = true`, `strip = "symbols"`
#    is inherited from release). The first two need overriding to get
#    release-equivalent semantics with symbols preserved.
CARGO_PROFILE_BENCH_STRIP=none \
CARGO_PROFILE_BENCH_DEBUG_ASSERTIONS=false \
CARGO_PROFILE_BENCH_OVERFLOW_CHECKS=false \
    cargo bench --bench lint_latency --no-run
CARGO_PROFILE_BENCH_STRIP=none \
CARGO_PROFILE_BENCH_DEBUG_ASSERTIONS=false \
CARGO_PROFILE_BENCH_OVERFLOW_CHECKS=false \
    cargo bench --bench profile_project --no-run

# 2. Relax WSL2's perf_event sandbox (one-time, resets on host restart).
sudo sh -c 'echo 1 > /proc/sys/kernel/perf_event_paranoid'

# 3. Capture (10s per bench at 997 Hz, presymbolicated sidecar).
samply record --save-only --no-open --unstable-presymbolicate -r 997 \
    -o /tmp/flamegraph-583/<NAME>.json.gz \
    -- target/release/deps/<BENCH_BIN> --bench --profile-time 10 '^<FILTER>$'

# 4. Convert + extract top-N (scripts in tools/perf/).
tools/perf/samply-to-folded.py /tmp/flamegraph-583/<NAME>.json.gz \
    --syms /tmp/flamegraph-583/<NAME>.json.syms.json \
    --thread <BENCH_BIN_PREFIX> \
| tools/perf/top-n-inclusive.py 15 '<LABEL>' --root '<CUTOFF_FRAME_SUBSTR>'
```

- **Sampler**: `samply` (perf_event_open syscalls; no `perf` binary
  required on WSL2). 997 Hz is a standard anti-aliasing rate that
  avoids harmonics with the 1000 Hz system timer; `perf record`'s
  default is 4000 Hz.
- **Why debug-assertions must be off**: the workspace's
  `[profile.bench]` has `debug-assertions = true`, which activates
  three load-bearing `raw.to_vec()` snapshots used to verify
  PageRewrite content-ignorance and lattice immutability invariants
  (see `marking_scheme_impl.rs:717`, `engine.rs:4434`,
  `canonical.rs:294`). These snapshots inflate `to_vec` and
  `drop_in_place::<Vec<...>>` inclusive percentages by ~12-18
  percentage points combined on `lint_10kb` — the gap between the
  debug-assert and release captures is the empirical proof of the
  contamination (§10).
- **Symbolication**: `--unstable-presymbolicate` writes a
  `.syms.json` sidecar with per-library RVA → symbol mappings.
  Resolves ~95% of frames; ~5% remain as `fun_<rva>` placeholders
  (see §7 for the parent-stack analysis that identifies these as
  allocator-adjacent code, not parser-adjacent as initially
  hypothesized).
- **Rank 1 is always 100% by methodology** — it's the root frame
  used for the `--root` cutoff (each top-N table is rooted to drop
  criterion harness and libc bootstrap frames). It is the cutoff
  itself, not a finding.
- **Inclusive time**: each frame's inclusive time is the sum of
  sample counts for every (truncated) stack containing that frame
  divided by the total samples in the rooted set. A frame appearing
  multiple times in one stack (recursion) is credited once.

> ⚠ **Inclusive-sum caveat (load-bearing).** Inclusive percentages
> are NOT additive across frames in a parent-child call chain.
> `Vec::clone` calls `T::clone` per element, so stacks containing
> the outer `Vec::clone` also contain `T::clone` — adding them
> double-counts the overlap. The CLONE-1 finding (§6.1) computes
> the true union of the CanonicalAttrs lifecycle frames using
> `tools/perf/union.py`, NOT the naïve sum of inclusive %s.

## 2. Top 15 inclusive frames — `lint_10kb` (release; rooted at `Engine::lint`)

_Total samples in stacks containing `lint_with_options_internal_with_cache`: 9,987 · Truncated folded stacks: 815 · 10s @ 997 Hz · release semantics (debug-assertions=off)_

| Rank | Inclusive % | Samples | Frame |
|---:|---:|---:|---|
| 1 | 100.00% | 9,987 | `<marque_engine::engine::Engine>::lint_with_options_internal_with_cache` (root cutoff) |
| 2 | 39.09% | 3,904 | `<marque_capco::scheme::adapter::CapcoScheme>::project_from_attrs_slice` |
| 3 | 35.61% | 3,556 | `<marque_capco::scheme::marking::CapcoMarking>::join_via_lattice` |
| 4 | 14.45% | 1,443 | `malloc` |
| 5 | 13.16% | 1,314 | `<marque_ism::canonical::CanonicalAttrs as core::clone::Clone>::clone` |
| 6 | 12.11% | 1,209 | `fun_ab570` (allocator-internal — see §4) |
| 7 | 12.03% | 1,201 | `<marque_engine::recognizer::StrictRecognizer ...>::recognize` |
| 8 | 10.92% | 1,091 | `<marque_ism::canonical::CanonicalAttrs as core::clone::Clone>::clone.1960` |
| 9 | 10.82% | 1,081 | `<marque_core::parser::Parser>::parse` |
| 10 | 10.56% | 1,055 | `fun_1a1340` (allocator/clone-internal — see §4) |
| 11 | 10.08% | 1,007 | `<marque_core::parser::Parser>::parse_marking_string` |
| 12 | 7.43% | 742 | `_libc_free` |
| 13 | 5.97% | 596 | `core::ptr::drop_in_place::<marque_ism::canonical::CanonicalAttrs>` |
| 14 | 4.83% | 482 | `<alloc::sync::Arc<alloc::boxed::Box<[CanonicalAttrs]>>>::drop_slow` |
| 15 | 4.25% | 424 | `marque_capco::scheme::predicates::satisfies::evaluate_custom_by_attrs` |

## 3. Top 15 inclusive frames — `decoder_10kb_one_mangled_region` (release; rooted at `Engine::lint`)

_Total samples in stacks containing `lint_with_options_internal_with_cache`: 9,805 · Truncated folded stacks: 856 · 10s @ 997 Hz · release semantics_

| Rank | Inclusive % | Samples | Frame |
|---:|---:|---:|---|
| 1 | 100.00% | 9,805 | `<marque_engine::engine::Engine>::lint_with_options_internal_with_cache` (root cutoff) |
| 2 | 33.74% | 3,308 | `<marque_capco::scheme::adapter::CapcoScheme>::project_from_attrs_slice` |
| 3 | 30.52% | 2,992 | `<marque_capco::scheme::marking::CapcoMarking>::join_via_lattice` |
| 4 | 24.83% | 2,435 | `<...StrictOrDecoderRecognizer...>::recognize` (dispatcher) |
| 5 | 13.79% | 1,352 | `<marque_engine::decoder::DecoderRecognizer ...>::recognize` |
| 6 | 12.27% | 1,203 | `malloc` |
| 7 | 11.68% | 1,145 | `<marque_core::parser::Parser>::parse` |
| 8 | 10.98% | 1,077 | `<marque_core::parser::Parser>::parse_marking_string` |
| 9 | 10.77% | 1,056 | `<marque_ism::canonical::CanonicalAttrs as core::clone::Clone>::clone` |
| 10 | 10.64% | 1,043 | `<marque_engine::recognizer::StrictRecognizer ...>::recognize` |
| 11 | 10.15% | 995 | `marque_engine::decoder::generate_candidate_bytes` |
| 12 | 10.12% | 992 | `fun_1a1340` |
| 13 | 9.89% | 970 | `<marque_ism::canonical::CanonicalAttrs as core::clone::Clone>::clone.1960` |
| 14 | 9.76% | 957 | `fun_ab570` |
| 15 | 6.97% | 683 | `_libc_free` |

## 4. Top 15 inclusive frames — `profile_project` (release; rooted at `phase_attribution`, all 6 phases mixed)

_Total samples in stacks containing `phase_attribution`: 31,187 · Truncated folded stacks: 1,221 · 5s/phase × 6 phases @ 997 Hz · release semantics_

| Rank | Inclusive % | Samples | Frame |
|---:|---:|---:|---|
| 1 | 100.00% | 31,187 | `profile_project::phase_attribution` (root cutoff) |
| 2 | 67.92% | 21,181 | `c_with_alloca` (criterion bench loop wrapper — **harness, not engine**) |
| 3 | 38.55% | 12,023 | `<marque_capco::scheme::marking::CapcoMarking>::join_via_lattice` |
| 4 | 36.00% | 11,227 | `<marque_capco::scheme::adapter::CapcoScheme>::project_attrs_pipeline` |
| 5 | 20.45% | 6,379 | `<...CapcoScheme... MarkingScheme>::closure` |
| 6 | 19.88% | 6,199 | `<criterion::bencher::Bencher>::iter::<...{closure#3}>` (`phase_d_from_canonical`) |
| 7 | 19.88% | 6,199 | `<criterion::routine::Function...>::profile` (closure#3) |
| 8 | 16.77% | 5,230 | `<criterion::routine::Function...>::profile` (closure#2 = `phase_c_scheme_project`) |
| 9 | 16.77% | 5,229 | `<criterion::bencher::Bencher>::iter::<...{closure#2}>` |
| 10 | 16.53% | 5,155 | `<criterion::bencher::Bencher>::iter::<...{closure#0}>` (`phase_a_join_via_lattice`) |
| 11 | 16.33% | 5,093 | `<...CapcoScheme... MarkingScheme>::project` (high-level entry) |
| 12 | 15.97% | 4,981 | `<criterion::routine::Function...>::profile` (closure#4 = `phase_e_engine_project_path`) |
| 13 | 15.97% | 4,980 | `<criterion::bencher::Bencher>::iter::<...{closure#4}>` |
| 14 | 15.54% | 4,847 | `<criterion::bencher::Bencher>::iter::<...{closure#5}>` (`phase_f_engine_lint_full`) |
| 15 | 15.54% | 4,847 | `<criterion::routine::Function...>::profile` (closure#5) |

## 5. What the measurement confirms (vs. synthesis)

1. **The synthesis correctly identified the top three engine-internal
   hot spots** by category (parser, engine entry, lattice composition)
   though the magnitudes were off. `Parser::parse_marking_string`
   landed at rank 11 (10.08%) on lint, not rank 1 (the synthesis
   estimate of 15-20% was high; the relative ranking against
   lattice frames was inverted).
2. **`Engine::with_clock` confirmed NOT on the per-call hot path.**
   Doesn't appear in any of the three top-15 tables. Construction
   tax is amortized across the bench loop as predicted.
   **DI-3 resolved as false positive.**
3. **`Engine::two_pass_fix` confirmed off the lint path.** 0% on
   both lint and decoder benches as predicted.

## 6. What the measurement contradicts (new findings)

### 6.1 CLONE-1 — `CanonicalAttrs` lifecycle is the largest measurable surface (but smaller than the naïve sum)

The headline finding the synthesis missed. The CanonicalAttrs
lifecycle frames on `lint_10kb` release:

| Frame | Inclusive % | Samples |
|---|---:|---:|
| `CanonicalAttrs::clone` (rank 5) | 13.16% | 1,314 |
| `CanonicalAttrs::clone.1960` (rank 8, second mono from a distinct callsite) | 10.92% | 1,091 |
| `drop_in_place::<CanonicalAttrs>` (rank 13) | 5.97% | 596 |
| `Arc<Box<[CanonicalAttrs]>>::drop_slow` (rank 14) | 4.83% | 482 |

⚠ **These are NOT inclusive-disjoint.** `Arc::drop_slow` is the
parent of `drop_in_place::<CanonicalAttrs>` for slice destruction;
naïve summation double-counts the overlap.

**Empirically measured union** (via `tools/perf/union.py` over the
clone, clone.1960, and `drop_in_place::<CanonicalAttrs>` frames):

```
Total samples (rooted): 9987
Union count: 3001 (30.05%)
Per-frame inclusive counts:
  CanonicalAttrs as core::clone::Clone (both monos)  2405  (24.08%)
  drop_in_place::<CanonicalAttrs>                     596   (5.97%)
Sum-of-inclusive (the over-counting figure):  3001 (30.05%)
Overlap:                                         0 samples (0.00%)
```

The two `Clone` monomorphizations (`clone` rank 5 and `clone.1960`
rank 8) are at structurally disjoint callsites and **are** additive
(24.08% combined). `drop_in_place` is also disjoint at the
stack-bottom level. So the **honest CanonicalAttrs-lifecycle
inclusive on lint_10kb release is 30.05%**, not 51%.

Adding `Arc::drop_slow` (which contains `drop_in_place::<CanonicalAttrs>`
as a sub-call — they overlap) brings the upper bound to ~31-32%,
not materially different.

Pairing this with `malloc` (14.45%) and `_libc_free` (7.43%) — neither
exclusive to the CanonicalAttrs lifecycle, but a substantial
fraction is — the heap-pressure story remains the load-bearing
finding. The lattice composition pipeline allocates and clones
heavily. **CLONE-1 stays an EXECUTE-tier candidate** but with the
corrected savings estimate (see §8).

### 6.2 `CapcoScheme::project_from_attrs_slice` is the single biggest engine hot frame

Measured 39.09% on `lint_10kb` and 33.74% on the decoder bench.
Synthesis ranked this at 3-5%; the measured ratio is ~10× larger.
This makes the lattice-projection pipeline (not just the join) the
load-bearing remediation target.

### 6.3 `CapcoScheme::closure` is inlined on the lint hot path

Surprising at first read: `CapcoScheme::closure` appears at 20.45%
on `profile_project` but at **0.00% on `lint_10kb`** release. The
closure operator's cost has been inlined into its parents —
`join_via_lattice` and `project_from_attrs_slice` — and doesn't
surface as a separate frame in the lint bench.

The implication is that the 20.45% measurement on `profile_project`
is a **per-call cost of the closure-operator entry point through
bench scaffolding**, NOT a discrete cost line on the lint path. The
work IS happening on lint (folded into the lattice frames above);
HOT-2's specific intervention target — eliminating per-iteration
`Vec::clone` in the closure-rule walker — would reduce some fraction
of the lattice composition cost, but the magnitude is not bounded
by the 20.45% number. **HOT-2 stays INVESTIGATE** rather than
lifting to EXECUTE; the savings estimate cannot be defended without
a closer-grained capture (e.g., MSR-level profile_project per-phase
breakdown of `phase_b_closure` allocator events).

### 6.4 Recognizer dispatch is at the noise floor

`StrictRecognizer::recognize` at 12.03% on lint. The
`Arc<dyn Recognizer<S>>` indirection itself is some fraction of that
(single-call indirection cost). Realistic devirtualization savings
are ~5-10µs lint mean, below D-8's 30µs noise floor.
**MO-2 stays INVESTIGATE** with a tighter (smaller) savings estimate.

### 6.5 Page-rewrite scheduler and parsed-markings cache are below the noise floor

Neither `PageRewrite` dispatch frames nor `parsed_markings` cache
rebuild frames appear in any of the three top-15 tables.
**CO-2 and CA-1 are soft-closed** in §5 of the parent diagnosis.

Caveat: top-15 cutoff is 4.25% inclusive on lint. A frame at 3-4%
would clear D-8's 30µs floor while still falling below the cutoff.
"No measured surface" means "≤ 4.25% inclusive on this capture",
not "zero cost."

## 7. Unresolved frames — `fun_ab570` and `fun_1a1340` (allocator-adjacent, not parser-adjacent)

Two `fun_<rva>` placeholders survived samply's
`--unstable-presymbolicate` symbolication: `fun_ab570` (12.11% lint
release) and `fun_1a1340` (10.56% lint release). Initial hypothesis
based on rustc symbol-name proximity was that these were parser /
Teddy SIMD frames. **Parent-stack inspection refutes that.**

Top parent stacks of `fun_ab570` on lint_10kb (release capture):

```
... → CapcoScheme::project_from_attrs_slice
    → SmallVec::reserve_one_unchecked → realloc → fun_acf50 → fun_ab570

... → CanonicalAttrs::to_vec → CanonicalAttrs::clone.1960
    → malloc → fun_ab570 → fun_a9ad0
```

Top parent stacks of `fun_1a1340` on lint_10kb:

```
... → CanonicalAttrs::to_vec → CanonicalAttrs::clone.1960 → fun_1a1340

... → CanonicalAttrs::to_vec → fun_1a1340

... → StrictRecognizer::recognize → from_parsed_unchecked → fun_1a1340
```

Both are called from `malloc` / `realloc` / `clone` / `to_vec` paths
— **allocator-internal code** (likely glibc allocator page management
or a Rust-side internal `realloc` helper), NOT parser SIMD scanning
as the initial Teddy-attribution hypothesis suggested. The
combined ~22.7% inclusive that these unresolved frames carry is
part of the same heap-pressure budget that CLONE-1 targets, not a
distinct unaccounted surface.

This honest correction strengthens CLONE-1's case rather than
weakening it: even after the union math, the actually-eliminable
heap surface is at least 30% (CanonicalAttrs lifecycle) and
plausibly higher when allocator-internal trampolines are included.

## 8. Re-evaluation summary for §5 of the parent diagnosis

See [`../2026-05-19-diagnosis.md` §5](../2026-05-19-diagnosis.md#5-ranked-remediation-table)
for the updated ranked table. Headline changes (post-#583, release
semantics):

| Candidate | Pre-measurement tier | Post-measurement verdict |
|---|---|---|
| **CLONE-1** (new) | n/a | **EXECUTE** — CanonicalAttrs lifecycle union 30.05% measured inclusive on `lint_10kb` release; structural sharing / clone-elimination is the single highest-leverage remediation. Savings estimate **60-180µs lint mean** (was 100-300 in the debug-build draft — corrected against release union). |
| **HOT-2** (closure `Vec::clone`) | INVESTIGATE | **STAYS INVESTIGATE.** Closure operator is 0% as a discrete frame on `lint_10kb` (inlined into lattice frames); the 20.45% on `profile_project` is a per-call-through-bench-scaffolding measurement, not a lint-time signal. The work is real but its discrete savings is not directly bounded. |
| **DI-3** (Engine::with_clock leak) | INVESTIGATE | **RESOLVE — false positive** (synthesis prediction was correct). |
| **CO-2** (PageRewrite mask) | INVESTIGATE | **Soft-close — no measured surface ≥ 4.25% inclusive.** |
| **CA-1** (parsed_markings cache) | INVESTIGATE | **Soft-close — no measured surface ≥ 4.25% inclusive.** |
| **LA-3** (single-portion fast path) | INVESTIGATE | **Stay INVESTIGATE.** Bench input is ~200 markings in **one** PageContext (the `\n\n` separators don't trip the scanner's `\n\n\n+`/`\f` page-break heuristic), so LA-3's target — single-portion pages — is **unmeasured by this bench**, not just weakly measured. Promotion blocked on a real-corpus portion-distribution capture (separate issue). |
| **MO-2** (dyn devirtualization) | INVESTIGATE | **Stay INVESTIGATE** with tighter estimate. Recognizer dispatch overhead is a fraction of `StrictRecognizer::recognize` (12.03%); devirt saves the vtable indirection per call, not the recognize() body. ~5-10µs lint mean, below D-8 30µs floor. |
| **OTHER-1** (this PR) | INVESTIGATE | **RESOLVED.** |

## 9. §5.1 reconciliation with #579 / #580 (refinement)

Two perf-related issues filed outside #582's umbrella were
reconciled against the measurement:

**#579** (refactor `parse_marking_string` into CategoryRegistry):
parser at rank 11 (10.08%) on lint, NOT the "hottest path."
Refactor is worth doing for **maintainability and Stage-4+
extensibility** (1000-line monolith blocks adding CUI / NATO / FGI
schemes) but performance return is ~1-3% lint mean. Recommend
re-framing as tech-debt rather than perf-recovery.

**#580** (decoder fast-path to skip strict re-parses): the decoder
bench fires the 16-attempt strict-parse **exactly once** (one
mangled region in the bench input). The measured parser-inclusive
delta over the strict-only lint bench is small (10.98% vs 10.08%,
+0.9 pp), consistent with one fire. The diagnosis's earlier draft
overstated the refutation; the correct read is "16× re-parse is a
real cost per mangled region, but the decoder_10kb bench measures
only one region." Documents with N mangled regions pay ~16N strict
parses; #580's savings forecast should be re-derived **per-region**,
not per-document. The decoder-bench-as-currently-shaped does not
measure the worst case for #580's claim.

Neither issue's framing surfaces the CanonicalAttrs lifecycle cost
(CLONE-1). The dominant lever remains lattice + heap, not
parser / decoder dispatch — but #580's parser re-run cost scales
linearly with mangled-region count in a way the decoder_10kb bench
does not exercise.

**Recommended sequencing**: CLONE-1 first; revisit #579 as
maintainability/extensibility with modest perf side-benefit;
revisit #580 with a multi-mangled-region bench fixture before
committing to its savings claim.

## §10. Methodology side note — the original debug-assertion-on capture

A first capture pass was taken before the debug-assertion confound
was identified. That pass produced these load-bearing differences
on `lint_10kb` (preserved for comparison):

| Frame | Debug-on (first pass) | Release (load-bearing) | Δ |
|---|---:|---:|---:|
| `CanonicalAttrs::clone` | 19.26% | 13.16% | −6.10 pp |
| `CanonicalAttrs::to_vec` | **11.85%** | **not in top-15** (≤ 4.25%) | ≥ −7.6 pp |
| `drop_in_place::<Vec<CanonicalAttrs>>` | **6.01%** | **not in top-15** | ≥ −1.8 pp |
| `clone.2014`/`.1960` (2nd mono) | 8.11% | 10.92% | +2.81 pp |
| Naïve sum (the wrong number) | 51.22% | 30.05% | — |
| **True union (the right number)** | 40.39% | 30.05% | −10.34 pp |

The debug-assertion overhead is concentrated in `to_vec` and the
matching `Vec` drop — both completely absent from the release-build
hot path. Three load-bearing `raw.to_vec()` snapshots gated by
`#[cfg(debug_assertions)]` at `marking_scheme_impl.rs:717`,
`engine.rs:4434`, and `canonical.rs:294` produce this contamination.
**Future captures should always set `CARGO_PROFILE_BENCH_DEBUG_ASSERTIONS=false`.**

## §11. The prior synthesized top-15 (preserved verbatim)

> This section is preserved as a historical comparison point — the
> measured tables above supersede it. The synthesis methodology is
> useful as a back-stop for environments where flamegraph capture
> is impractical.

### Methodology (synthesis)

Each row's inclusive-time % is computed as:

```
inclusive_% = (stage_mean_ns_or_us × per_call_frequency_estimate)
              / lint_10kb_total_mean_us × 100
```

Where `lint_10kb_total_mean_us` at HEAD = 1022.8 µs. Per-call
frequencies are inferred from the bench input shape
(`build_representative_input`, 10 KB synthetic input with ~10-15
portion markings + prose, established in `lint_latency.rs`).

This is a **load-bearing approximation**, not a measurement.
Numbers to the right of the decimal should be treated as ranking
signal, not absolute attribution.

> **Bench input correction.** The "10-15 portion markings + prose"
> framing in the synthesis methodology was wrong: the bench block
> has 2 banners + 2 portions × ~50 repetitions = **~200 markings
> in a single PageContext** (the `\n\n` separators don't trip the
> scanner's `\n\n\n+`/`\f` page-break heuristic, so all markings
> accumulate into one page). This is a more extreme test of the
> per-portion lattice cost than the synthesis assumed.

### Top 15 inclusive-time frames at HEAD (synthesized)

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
