<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-perf closeout — Performance-engineer review (R1)

> Reviewer 1 of 3, performance-engineering slot. Scope per the
> implementer's brief: methodological rigor + numerical-claim
> cross-check + CI gate sanity + adjacent-path discipline. R2 covers
> overall code quality / docs; R3 covers attribution narrative.

## Overall verdict

**APPROVE WITH FIXUPS.** The diagnosis is methodologically sound on
the load-bearing reconciliations (the "~1.7ms" recall → mid-flip p99
match is well-supported by the criterion checkpoints; the
attribution-narrative resolution of contradictions 1 + 2 is correct;
the cargo-bloat top-line numbers verify cleanly against the supporting
artifact). The CI gate edits are well-implemented and correctly
scoped. **Required fixups are bookkeeping defects in the remediation
table** (score-formula application inconsistency, EXECUTE-tier
candidates that violate D-8's noise-floor rule, savings estimates
that aren't reconciled against the per-stage bench data) plus one
genuine methodology gap (the WASM-size question is scoped too narrowly
relative to the user's actual concern). None of these block merge of
a diagnosis-only PR, but they should be corrected before the
remediation-lane PRs start consuming the table.

## Required fixups (BLOCKING — must address before merge)

### [F1] EXECUTE-tier rows violate D-8 noise-floor rule

D-8 specifies: "EXECUTE tier: candidates with `evidence` populated
AND `expected_savings` > noise floor (≥ 30µs lint OR ≥ 30 KB WASM)
AND `risk_class` ≤ MED."

Walking the EXECUTE-tier rows against this:

| Row | Lint µs (range) | WASM KB (range) | Risk | Both gates met? |
|---|---|---|---|---|
| LA-1 | 5-20 (below 30µs floor) | 40-60 (above 30KB floor) | LOW | OR-met ✅ |
| LA-2 | 30-80 ✅ | 0 | LOW | ✅ |
| LA-3 | 40-100 ✅ | 5-10 | LOW | ✅ |
| MO-1 | 0 | 30-40 ✅ | MED | ✅ |
| **MO-2** | **10-30 (below floor)** | **5-15 (below floor)** | MED | **❌ neither met** |
| **CO-1** | **20-60 fix-path** (off lint hot path) | **10-20 (below floor)** | MED | **❌ ambiguous** |

MO-2 fails both noise-floor gates and should be INVESTIGATE per the
rule the diagnosis itself cites in §5. CO-1 explicitly states "(fix
path; not lint hot path)" in its evidence — if `lint_10kb` doesn't
exercise it, the savings don't count against the diagnosis's
load-bearing regression. Both should be reclassified INVESTIGATE,
or the table preamble should be amended to explain why these two
escape the gate.

### [F2] Score-formula application is inconsistent across rows

Preamble: "score = (savings_midpoint_us × confidence_pct) /
risk_multiplier" where HIGH=1.0, MED=0.6, LOW=0.3, risk LOW=1,
MED=2, HIGH=4. The score column doesn't follow this consistently.
Sampling:

| Row | Stated values | Formula on midpoint | Listed score |
|---|---|---|---|
| LA-2 | 30-80 µs, evidence cites MED confidence, LOW risk | 55 × 0.6 / 1 = 33.0 | 33.0 ✅ |
| LA-3 | 40-100 µs, LOW risk | 70 × 0.6 / 1 = 42.0 | 42.0 ✅ (assumes MED confidence) |
| HOT-1 | 100-200 µs, MED conf, MED risk | 150 × 0.6 / 2 = 45.0 | 45.0 ✅ |
| **CO-1** | 20-60 µs, MED risk | 40 × 0.6 / 2 = 12.0 | **6.0 (off — appears to use lower bound 20)** |
| **MO-1** | 30-40 KB, MED risk | 35 × 0.6 / 2 = 10.5 | **21.0 (off — implies LOW risk × MED confidence)** |

The confidence axis is not in the table, and the row-by-row
substitution doesn't reconcile to a single formula. Either:
1. add a `confidence` column to the table so the math is auditable, or
2. document the per-row deviations (which bound — lower / midpoint /
   upper — each row used) in a follow-up note.

Without this, the score column is decorative, not load-bearing.

### [F3] Savings estimates are not reconciled against per-stage bench data

`profile_project::phase_a_join_via_lattice` measured 478ns per call
at HEAD. `lint_10kb` makes ~50 of these (per the
profile_project.rs:206-219 comment block: "the bench profiling
discovered that ~50 cache-miss calls happen with portions growing
monotonically"). Total aggregate phase_a contribution: ~24µs across a
1023µs `lint_10kb` mean.

LA-3 claims 40-100µs lint savings from a single-portion fast path.
That requires saving every per-call phase_a invocation at the high
end (or saving an unrelated cost not enumerated in evidence). For
single-portion calls specifically, even 100% savings on every n=1
call (probably ~10-20 of the 50) × 478ns ≈ 2-10µs. **The 40-100µs
estimate is upper-bounded at ~10µs by the available evidence.**

Similarly:
- `phase_b_closure` is 277.9ns per call × ~1 page = ~278ns per
  `lint_10kb`. HOT-1 claims 100-200µs savings; HOT-2 claims 50-150µs.
  Both are off by ~3 orders of magnitude against the per-stage data
  unless the closure operator fires far more frequently than the
  per-page interpretation. Reconcile the call frequency or downgrade
  the estimates.

Fixup: each EXECUTE candidate's `expected_savings_us` should be
reconciled with the relevant `profile_project` stage's call-frequency
estimate from `lint_10kb`. Where the stage data contradicts the
estimate, the estimate is wrong and should be re-derived.

### [F4] Scope mismatch — the user's WASM concern is plausibly cross-006, not cross-PR-4

The preflight (`docs/plans/2026-05-19-pr4b-perf-preflight-performance.md`
§1.3) explicitly anchors the user's WASM growth at **"~600 KB
pre-refactor" → "~1.6 MB current"** — a ~+1 MB delta. The diagnosis
scopes itself to `pre-pr4` (`18cef6c9`, 2026-05-15) → HEAD, finds
**+94 KB pre-opt**, and treats this as the WASM story.

**The diagnosis's scope misses the user's question.** Per D-7, the
three reference points are correct for the PR-4-to-6 cumulative
regression, but the WASM regression the user reported is cross-006,
not cross-PR-4. The +94 KB pre-pr4→HEAD WASM delta is the diagnosis
answer; the +1 MB pre-refactor→HEAD WASM growth that the user is
asking about is unmeasured here.

The diagnosis Open Question §6.2 punts this to OTHER-2 (GHA
re-capture), which is the right move for the cross-host slice — but
the cross-006 slice should also be flagged in §6 as a separate
unresolved question, with a candidate id (call it OTHER-3) for the
follow-up that captures a `pre-006` anchor.

## Recommended fixups (NICE-TO-HAVE — fine to defer)

### [R1] Adjacent-path discipline — enumerate ALL engine dyn-dispatch sites in MO-1/MO-2 evidence

MO-1 enumerates `Engine::with_clock::<CapcoScheme>` mono. MO-2
enumerates `Arc<dyn Recognizer<S>>` and `Arc<dyn Vocabulary<S>>`.
But the engine.rs holds at least four `dyn` dispatch sites:

- `recognizer: Arc<dyn Recognizer<CapcoScheme>>` (engine.rs:253) ✅ enumerated
- `rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>` (engine.rs:203) — **not enumerated**
- `clock: Box<dyn Clock>` (engine.rs:216) — **not enumerated**
- `Box<dyn RuleSet>` returned from `default_ruleset()` (lib.rs:119) — **not enumerated**

`Vec<Box<dyn RuleSet<S>>>` is iterated per-lint per-portion (`rules()`
returns slice → per-rule call). This is a more interesting hot-path
candidate than the `Vocabulary` doc-comment claim, because Vocabulary
doesn't appear to be invoked via `Arc<dyn>` anywhere in the engine
hot path (the `_p99_note` in `baseline.json` may be stale —
`shape_admits` is doc-referenced but not actually invoked from
`engine/src/**` in the current codebase).

Recommend adding RuleSet dispatch as a separate EXECUTE/INVESTIGATE
candidate (call it MO-4), and either backing the Vocabulary claim
with a real call-site path or downgrading MO-2's evidence text.

### [R2] Quicksort closure count is +5 net new, not 5 total

The cargo-bloat report shows 7 quicksort monomorphizations in
`marque_capco` at HEAD (5 new + 2 pre-existing
`evaluate_sar_banner_rollup` and `render_structural`). The
diagnosis's TL;DR §0.4 phrasing "5 new quicksort monomorphizations"
is correct; the implementation-report and §4 are consistent. But
LA-1's title says "Replace 5 SciSet/SarSet quicksort closures" —
LA-1 should clarify whether the title's "5" includes the
pre-existing 2 (in which case the replacement target is 7) or
just the 5 new ones (in which case it should say "5 NEW").
This is bookkeeping precision, not a correctness issue.

### [R3] The "p99 vs mean" disambiguation should appear in TL;DR

The diagnosis correctly distinguishes mean/upper-CI from p99 in the
reference table (§1), but the TL;DR §0.1 says "cumulative regression:
~+80% mean, +106% p99" without a leading sentence explaining
**which one** corresponds to user-facing latency (depends on
whether the user is recalling worst-case keystroke jitter — p99 —
or steady-state perceived throughput — mean). Add one clarifying
sentence: "Production-relevant percentile is p99 for interactive
keystroke latency; the +106% p99 delta is the user-facing number,
the +80% mean is the steady-state number."

## Methodology assessment

### What's solid

- **D-7 three-reference-point scope satisfied.** `pre-pr4` /
  `mid-flip` / `head` SHAs verified via git. The mid-flip capture
  resolves contradiction 2 (PR 4b-E recovery) load-bearingly —
  without it, the diagnosis couldn't have demonstrated PageContext
  retirement actually helped. Justified inclusion.
- **Same-hardware discipline maintained.** WSL2 dev for all three
  capture rounds, single calendar day. Cross-host noise vs GHA is
  acknowledged in §1 hardware note. Every numerical claim in
  §1 / §5 carries the WSL2 attribution.
- **Sample size discipline.** `lint_latency.rs` does NOT override
  `sample_size`, so Criterion's default of 100 applies. Implementer
  did not inflate (per D-7 mandate) nor deflate (no `sample_size(10)`
  override on this bench). ✅
- **Confidence-interval framing.** All `lint_10kb` numerics in
  §1 + criterion-checkpoints.md carry `(lower-mean-upper)` triples
  from Criterion. p99 values are computed from sample.json. The
  cumulative-delta math is on mean-to-mean and p99-to-p99 (not point
  estimates against CI bounds — defensible).
- **Headline numerical claims verify against supporting artifacts**
  (table below).

### What's a methodology gap

- **F3 above.** Savings estimates don't reconcile with the per-stage
  bench data the implementer captured. The bench evidence the diagnosis
  cites argues for smaller savings than the table claims.
- **F4 above.** WASM scope is too narrow for the user's actual concern.
- **Flamegraph synthesis.** Acknowledged transparently in
  §3 + lint-flamegraph-top15.md status note. The synthesis is
  rank-only (2-3% bands; explicit confidence reduction in §5 rows
  that depend on it). This is the right call given the WSL2 sudo
  blocker on `perf_event_paranoid`. The implementer's choice to
  document the gap rather than fabricate a flamegraph is correct
  — `OTHER-1` covers the recovery path. The diagnosis must be read
  with this gap firmly in mind, and it labels itself appropriately.

## Numerical claims cross-check

| Claim | Source artifact | Verified? |
|---|---|---|
| `lint_10kb` pre-pr4 mean = 569.6µs, p99 = 690.3µs | `criterion-checkpoints.md:41` | ✅ |
| `lint_10kb` mid-flip mean = 1218.8µs, p99 = 1691.4µs | `criterion-checkpoints.md:42` | ✅ |
| `lint_10kb` head mean = 1022.8µs, p99 = 1422.4µs | `criterion-checkpoints.md:43` | ✅ |
| Cumulative mean delta: +453µs / +80% | 1022.8 − 569.6 = 453.2µs / 569.6 → +79.6%. ≈ +80% | ✅ |
| Cumulative p99 delta: +732µs / +106% | 1422.4 − 690.3 = 732.1µs / 690.3 → +106.1% | ✅ |
| Recovery mid-flip → head mean: -16.1% | (1022.8 − 1218.8) / 1218.8 = -16.08% | ✅ |
| Recovery mid-flip → head p99: -15.9% | (1422.4 − 1691.4) / 1691.4 = -15.90% | ✅ |
| `marque_capco` 217.2 → 399.4 KiB / +182.2 / +83.9% | `cargo-bloat-top20.md:20` | ✅ |
| 5 new quicksort monomorphizations totaling ~77 KiB | bloat lines 8-12 = 15.6 + 15.6 + 15.6 + 15.6 + 15.5 = 77.9 KiB | ✅ |
| Pre-pr4 had 2 quicksort monos totaling 21.9 KiB | 12.2 (SAR banner) + 9.7 (SCI render) = 21.9 ✅ | ✅ |
| `CapcoMarking::join_via_lattice` 57.9 KiB | `cargo-bloat-top20.md:62` | ✅ |
| `<PageContext>::project` was 44.2 KiB at pre-pr4 | `cargo-bloat-top20.md:95` | ✅ |
| `<Engine>::with_clock::<CapcoScheme>` 40.4 → 46.6 / +6.2 KiB | `cargo-bloat-top20.md:64,96` | ✅ |
| `marque_engine` overall +19.6 KiB | `cargo-bloat-top20.md:21` | ✅ |
| `phase_b_closure` 75.1ns → 277.9ns, +270% | `criterion-checkpoints.md:92` | ✅ |
| `phase_g_project_n1` +38.9% | `criterion-checkpoints.md:97` | ✅ |
| `phase_i_join_n10/n25` -40% | `criterion-checkpoints.md:104-105` | ✅ |
| WASM pre-opt: 1,218,209 → 1,311,681 B (+93,472 / +7.67%) | `twiggy-monos-top20.md:122-124` | ✅ |
| WASM post-opt: 1,139,451 → 1,229,883 B (+90,432 / +7.93%) | `twiggy-monos-top20.md:122-124` | ✅ |
| Net engine-subgraph delta +127.9 KiB | (capco +182.2) + (engine +19.6) + (ism -83.4) + (core +9.5) = +127.9 ✅ | ✅ |
| "User reported ~1.7ms matches mid-flip p99 (1691µs)" | mid-flip p99 = 1691.4µs; rounded → 1.7ms ✅ | ✅ |
| "HEAD p99 = 1422µs" | criterion-checkpoints.md:43 | ✅ |

All headline numerical claims that have a supporting artifact verify.
None of the cross-checks failed. The numerical layer is the strongest
part of the diagnosis.

## CI gate edits review

### `tools/wasm-size-check.sh`

- Env-var override placed at **lines 144-149**, AFTER baseline read
  (line 132) and AFTER artifact-existence check (line 109). ✅
- Build-failure paths (rustc errors, codegen panics, missing artifact)
  still fail the gate — they're checked at lines 99-112, BEFORE the
  env var skip. ✅
- Skip message ("OK (skipped by env var on this branch)") is clear and
  prints the delta-in-bytes for diagnostic visibility. ✅
- Comment block mirrors the pattern at `scripts/bench-check.sh:31-36` /
  `:220-221` (env-override applies to drift gate only; build-failure
  + absolute-ceiling enforcement preserved). ✅

### `.github/workflows/ci.yml`

- Branch match is **exact** (`github.ref == 'refs/heads/refactor-006-pr-4b-perf-closeout'`),
  not prefix. ✅ This is the right call per the risk register —
  prefix match would silently shadow future perf-related branches.
- Env block is on the **step level** for `bench-check.sh` (line 832)
  and `wasm-size-check.sh` (line 544), so the env vars don't leak to
  any other step in the same job. ✅
- Both env vars compute to empty string when off-branch
  (`'' || ''` evaluation in GHA), and the scripts' `${VAR:-0}`
  defaulting keeps the gate active. ✅
- Comment block on both injection sites explicitly says "REMOVE this
  env block once the diagnosis branch merges" and links to the PM
  decisions doc. ✅ The revert path is clearly documented.

### Comparison to `bench-check.sh` skip pattern

The `MARQUE_WASM_SKIP_REGRESSION` implementation parallels the
existing `MARQUE_BENCH_SKIP_REGRESSION` pattern at
`scripts/bench-check.sh:64,231-249`. Both:
- Default off via `${VAR:-0}` semantics
- Print "skipped via ENV_VAR=1" in the threshold-label line
- Preserve absolute-ceiling enforcement regardless of the skip
- Maintain build-failure enforcement regardless of the skip

Mirror pattern verified. ✅

## Adjacent-path checks

### LA-1 — quicksort consolidation enumeration

The cargo-bloat report shows 5 quicksort monomorphizations at
`SciSet::to_markings` / `SarSet::to_marking` / nested closures. Source
file `crates/capco/src/lattice.rs` has **6 sort_by sites in those two
types** (lines 152, 159, 165 for SciSet; lines 338, 346, 353 for
SarSet), of which 5 produce distinct monomorphizations in the bloat
report. The closure deduplication is correct as far as it goes.

But the implementer did NOT enumerate the **other 11 sort_by sites
in `marque_capco` outside SciSet/SarSet** — `lattice.rs:3796` and 9
others in other files (caught via
`grep -rn "sort_by\|sort_unstable_by" crates/capco/src/`). Some are
likely cold paths or pre-existing; the implementer should
spot-check that the 5 they're consolidating are the actually-hot
ones, not just the bloat-largest ones. Bloat size ≠ runtime
frequency.

This is "spot-check pass" but not "exhaustive enumeration pass."
Acceptable for a diagnosis PR.

### MO-1 — `Engine::with_clock` mono enumeration

MO-1 candidate covers `Engine::with_clock::<CapcoScheme>` only. There
are no other generic-over-S engine entry points enumerated. Spot
check via grep: `grep -rn "with_clock\|<CapcoScheme>" crates/engine/src/`
shows `with_clock` is the only `<S>`-generic constructor; everything
else has S concretized at the trait-impl boundary. Adjacent-path
coverage adequate.

### MO-2 — dyn dispatch enumeration (covered in [R1])

See [R1]. The implementer enumerated 1 of at least 4 dyn dispatch
sites on the engine. The Vocabulary claim in particular appears to
be doc-comment-derived rather than call-site-derived.

### Adjacent path the diagnosis did NOT enumerate but should have

- `Vec<Box<dyn RuleSet<S>>>` iteration in `Engine::lint*` —
  potentially per-rule × per-portion dyn-call. See [R1].
- The `marque-ism` -83.4 KiB shrinkage — the diagnosis treats this
  as "offsetting the marque_capco growth" (cargo-bloat-top20.md:46-47),
  but doesn't enumerate **which** functions in `marque-ism` shrank.
  PR 6c retired `PageContext::project` (44.2 KiB) — that's roughly
  half of the shrinkage. Where did the other ~40 KiB come from? An
  enumeration would close the symmetry argument.

These two are recommendations, not required fixups. Diagnosis-only
PRs can be incomplete on enumeration if the load-bearing claims hold.

## What the implementer got RIGHT

1. **Transparent acknowledgment of the flamegraph synthesis gap.**
   `lint-flamegraph-top15.md` opens with "Status: synthesized, not
   measured" and consistently labels every confidence reduction. The
   implementer chose to document rather than fabricate. This is the
   correct call per the perf-engineer brief ("If they fabricated
   top-frames from intuition... flag it").
2. **The "~1.7ms = mid-flip p99 (1691µs)" reconciliation is
   well-supported.** The user-recall vs measurement reconciliation
   is the single most load-bearing claim in the diagnosis, and the
   number lines up cleanly with `criterion-checkpoints.md:42`. p99
   computation methodology (sample.json + python percentile) is
   documented in the preflight §3.4 reference.
3. **Contradiction 2 (PR 4b-E recovery) resolved correctly.** The
   diagnosis demonstrates PR 4b-E + 4b-F + 6c DID recover ~16% of
   headroom — the prior expectation in `baseline.json::_note` that
   it would close the full gap was over-optimistic, and the
   diagnosis explains why (closure rules + page-rewrite catalog
   growth ate part of the recovery). Per-stage profile_project
   data corroborates the structural claim cleanly.
4. **D-3 CI gate strategy implementation is precise.** Exact-branch
   match, step-level env injection, comment block documenting the
   revert path, mirror of the existing bench-check pattern. The
   reviewer-2 risk-register item ("CI gate skip-env-var becomes a
   precedent that gets copied silently") is well-mitigated by the
   exact-branch match and the explicit REMOVE-this-env-block comment.
5. **D-1 / D-6 scope discipline.** No `crates/*/src/**` edits
   (verified by `git diff origin/staging -- 'crates/*/src/**' | wc -l`
   = 0 per implementation-report.md verification line). All artifacts
   are text, under 100 KB budget (verified 68 KB). Diagnosis doc is
   357 lines, well under the 800-line cap.
6. **D-9 bench discipline.** No new gated benches added. The
   implementer stays inside `profile_project.rs` for per-stage
   attribution. Per-stage data is what enables the F3 reconciliation
   above — without `profile_project`, the savings estimates would
   be unmoored. (The fact that the estimates still need
   reconciliation is a F3 fixup; the data exists to do it.)

## Diagnostic verdict

The diagnosis is approve-with-fixups-ready. The four required fixups
are bookkeeping defects in the remediation table plus one scope-
flagging gap; none invalidates the load-bearing claims (the cumulative
regression magnitudes, the attribution narrative, the WSL2 reference
data). The diagnosis is a fair, honest, transparently-caveated
artifact suitable as the foundation for a multi-PR remediation lane.

The implementer correctly didn't fabricate a flamegraph, correctly
didn't widen scope to a baseline re-capture, and correctly named the
gap between native-bloat and WASM-monos attribution. The required
fixups should be straightforward to address: re-tier MO-2 and CO-1,
clarify the score-formula application, reconcile savings estimates
against per-stage bench data, and add OTHER-3 for the cross-006
WASM question.
