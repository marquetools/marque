<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-perf closeout — diagnosis (2026-05-19)

> **PR shape.** Diagnosis-only. This document, the four supporting
> text artifacts under `./2026-05-19-diagnosis/`, plus a small set of
> CI / PR-template edits. No `crates/*/src/**` edits.
> **The regression is the deliverable**, not a defect. Constitutional
> ceilings SC-001 (`lint_10kb` p95 ≤ 16ms) and SC-002 (`decoder_10kb`
> p95 ≤ 18ms) are **not** violated.

> **Companion documents** (read together):
> - PM contract: `docs/plans/2026-05-19-pr4b-perf-pm-decisions.md`
> - Methodology preflight: `docs/plans/2026-05-19-pr4b-perf-preflight-performance.md`
> - PR-shape preflight: `docs/plans/2026-05-19-pr4b-perf-preflight-architect.md`
> - Attribution walkdown: `docs/plans/2026-05-19-pr4b-perf-preflight-attribution.md`

## 0. TL;DR

1. **Native `lint_10kb` cumulative regression: ~+80% mean, +106% p99** from
   `pre-pr4` (`18cef6c9`, 2026-05-15) to `head` (`81694384`, 2026-05-19),
   measured on the same WSL2 dev host within one calendar day. The
   production-relevant percentile for interactive keystroke latency is
   p99 (the +106% p99 delta is the user-facing number); +80% mean is
   the steady-state throughput number. The user reported "~1.7ms" —
   this matches the **mid-flip p99** at PR 4b-D.2 merge (1691µs),
   not HEAD (HEAD p99 = 1422µs).
2. **PR 4b-E + 4b-F + 6c (PageContext retirement) recovered ~16% headroom**
   from the mid-flip peak. The recovery is real, but did not pay back the
   pre-flip → mid-flip jump in full.
3. **WASM size: +94 KB / +7.7% pre-opt, +90 KB / +7.9% post-opt** over
   the PR-4-to-6 cumulative window. Not the +400 KB the user reported
   over the same window. The user's wider framing — ~600 KB pre-
   refactor → ~1.6 MB current — anchors pre-006 (pre-engine-rule-
   refactor), not pre-PR-4; the pre-006 → HEAD slice is unmeasured by
   this diagnosis and tracked as INVESTIGATE candidate OTHER-3.
4. **Top native driver: `CapcoMarking::join_via_lattice` (57.9 KiB
   single function) + 5 new quicksort monomorphizations on lattice
   projections (~77 KiB combined).** Pre-pr4's
   `PageContext::project` (44.2 KiB) was the single resolver; the post-
   flip path distributes the work across 10 lattice constructors, each
   with its own monomorphization.
5. **Top WASM driver: same root cause; twiggy monos could not resolve
   names** (stripped by `release-web` profile's `--monomorphize` and
   `--strip-toolchain-annotations` passes), so WASM-specific
   attribution is downgraded to MED confidence via the native bloat
   proxy.
6. **Remediation tier distribution (post-R1 review fixups):** 1 EXECUTE
   candidate (LA-1 — quicksort consolidation; 5-20µs lint, 40-60 KB
   WASM) and 16 INVESTIGATE candidates. **EXECUTE-tier alone does NOT
   close the lint regression** — LA-1 plausibly closes ~45-65% of the
   WASM regression but only ~1-4% of the 453µs lint regression. The
   remediation strategy depends on elevating INVESTIGATE candidates to
   EXECUTE after flamegraph capture (candidate OTHER-1) tightens their
   bench-supported savings estimates.

## 1. Reference range

| Tag | SHA | Date | Significance | lint_10kb mean | lint_10kb p99 | WASM pre-opt |
|---|---|---|---|---|---|---|
| `pre-pr4` | `18cef6c9` | 2026-05-15 | PR 9c.2 merge; last commit before PR 4a | 569.6µs (559-581 CI) | 690.3µs | 1,218,209 B |
| `mid-flip` | `ebbefda0` | 2026-05-18 | PR 4b-D.2 hot-path flip | 1218.8µs (1183-1261 CI) | **1691.4µs** | n/a (not captured) |
| `head` | `81694384` | 2026-05-19 | Current; post-4b-E + 4b-F + 6c + PR-5 | 1022.8µs (1006-1041 CI) | 1422.4µs | 1,311,681 B |

**Cumulative delta** (`pre-pr4` → `head`):
- Mean: **+453µs (+80%)** on `lint_10kb`.
- p99: **+732µs (+106%)** on `lint_10kb`.
- p99 headroom against SC-001 16ms ceiling: ~14.6ms.
- WASM pre-opt: **+93,472 B (+7.67%)**.
- WASM post-opt (minimal `-O3` + features, no `--monomorphize`):
  **+90,432 B (+7.93%)**.

**Mid-flip → head** (PR 4b-E + 4b-F + 6c recovery):
- Mean: **-196µs (-16.1%)**. Real recovery.
- p99: **-269µs (-15.9%)**. Same recovery shape.

Hardware: WSL2 dev host (AMD64 Linux 6.6.114.1-microsoft-standard-WSL2).
Single-day capture controls cross-host noise. Within-host noise from
criterion's CI is the residual signal floor.

Detailed numerics: `./2026-05-19-diagnosis/criterion-checkpoints.md`.

## 2. Attribution narrative

Three contradictions were flagged by
`docs/plans/2026-05-19-pr4b-perf-preflight-attribution.md`. Each is
resolved (or escalated to INVESTIGATE) below.

### 2.1 Contradiction 1: PR #498's attribution to 4b-B/C lattice landings

The PR #498 commit message attributed its 828→914µs baseline jump
to "intervening staging merges (PR 4b-B/C lattice landings, decoder
priors, parser/recognizer additions)" — but PR 4b-B/C are
**pre-flip**; the lattices were only consumed in tests at that point.
Either the attribution was diplomatic, or there's pre-flip lattice
cost not accounted for.

**Resolution: the attribution was diplomatic, but defensibly so.**
The 4b-B/C-era lattice code added:

1. Per-axis `*::from_attrs_iter` constructors (build paths the runtime
   would later exercise, but at 4b-B were exercised only in tests).
2. Static data tables: variant payload tables for `ClassificationLattice`,
   supersession overlays for `DissemSet`, `RelToBlock`, etc.
3. Trait impls for `JoinSemilattice + MeetSemilattice` on 7 new lattice
   types, multiplying the `impl Lattice` blanket marker count by ~3×
   (3 → 10 types).

At 4b-B merge, items (1) and (3) were dead code on the production hot
path (only `tests/*` exercised them). Item (2) was live static data
in the binary regardless. The native binary cost of items (1) and
(3) would be near-zero — LLVM's dead-code elimination should have
removed them.

**But:** the `cargo bloat` snapshot at HEAD shows
`marque_capco` 217.2 → 399.4 KiB, of which a sizable fraction is the
trait impl bodies (`<*::from_attrs_iter>`, `<*::join>`, `<*::meet>`,
`<*::to_marking>`, `<*::to_markings>`). The 4b-B trait impls became
*reachable* only at PR 4b-D.2 merge (the flip), and only then did the
~+90-100 KiB binary growth materialize. The reachability path runs
through `CapcoMarking::join_via_lattice` (57.9 KiB), which composes
all 10 lattice types in one body.

So: PR 4b-B/C added the *source code* for the cost; PR 4b-D.2 added
the *reachability*. PR #498's attribution to "4b-B/C" is technically
correct but misleadingly compressed — the cost manifests only after
4b-D.2. **A precise attribution would say: "the structural work added
in 4b-B/C, made reachable by 4b-D.2."**

**Confidence: MED-HIGH.** Confirmed by cargo bloat at pre-pr4 (no
lattice trait impls in `marque_capco` top-50) vs head (all 5 quicksort
monomorphizations from `to_marking`/`to_markings` are top-15 entries).

### 2.2 Contradiction 2: PR 4b-E expected to retire residue cost, but did it?

The `_note` in `benches/baseline.json::lint_10kb` explicitly states
PR 4b-E (the PageContext residue retirement, -3457 LOC) "will retire
the remaining residue-axis tmp_ctx requirement, expected to bring the
GHA value back down." The user reports it didn't.

**Resolution: PR 4b-E + 4b-F + 6c DID recover headroom, but only ~16%
of the post-flip regression.** The pre-flip → mid-flip jump was
+114% on the mean (570 → 1219 µs); the mid-flip → head recovery was
-16.1% (1219 → 1023 µs). After the recovery, the residual regression
is +80% from pre-pr4 baseline.

The expectation in the `_note` (that PR 4b-E would close the gap)
was over-optimistic. Three reasons:

1. **Post-PR-4b-D.2 catalog growth ate part of the headroom.** Six
   closure-rule rows landed after PR 4b-D.2 (PRs #519, #521, #529,
   #540, #544, #548). `profile_project::phase_b_closure` measures
   75ns at mid-flip → 278ns at head, **+270%** in absolute terms.
   This is the closure operator's per-call floor rising as catalog
   rows accumulate. Empty-cone short-circuit caps the worst case;
   the typical case pays the new floor.
2. **PageRewrite catalog grew 14 → 27 rows** between mid-flip and
   head (Pattern-B/C strips, Pattern-A NOFORN-supremacy rows,
   per-compartment SCI additions, PR-5 banner-rollup rows). Each row
   is a constant-time predicate eval per page; aggregate cost scales
   linearly.
3. **`Engine::with_clock::<CapcoScheme>` grew +6.2 KiB** (40.4 →
   46.6 KiB), and `marque_engine` overall grew +19.6 KiB across the
   umbrella. Some construction-time cost may have leaked into the
   per-call path (INVESTIGATE: this is candidate DI-3).

The recovery from PR 4b-E + 4b-F + 6c was structurally sound (the
post-flip path is now ~50% more O(n) walks per call than pre-flip,
but the per-walk cost dropped). The recovery just couldn't close
the gap because the gap also widened from post-flip catalog growth.

**Confidence: HIGH.** Confirmed by per-stage `profile_project`
deltas in `./2026-05-19-diagnosis/criterion-checkpoints.md`. The
`phase_i_join_n10/n25` improvement (-40%) corroborates the
PageContext retirement's per-portion savings; `phase_b_closure`
+270% corroborates the closure-rule growth.

### 2.3 Contradiction 3: WASM measurement basis (1.6 MB user vs 1.38 MB baseline)

`tools/wasm-size-baseline.txt` = 1,386,447 B (pre-opt). User-reported
"~1.6 MB". The CI ships an `-O3`-opt'd artifact that's smaller still.

**Resolution: HEAD WASM on WSL2 dev is 1,311,681 B (pre-opt) /
1,229,883 B (post-opt minimal). User's ~1.6 MB does not reproduce
on either basis on this host.**

Three candidate explanations:

1. **Cross-host build divergence.** GHA `ubuntu-latest` may produce
   a meaningfully different artifact size than WSL2 dev — observed
   ~100 KB delta in `tools/wasm-size-check.sh`'s header comment.
   GHA build at HEAD could be ~1.4 MB pre-opt, possibly larger
   post-`wasm-opt` once the integrated pipeline runs (different
   `wasm-opt` version + different feature flags). Verify with a
   GHA-side re-capture once this PR's PR-template / env-var
   discipline is in place.
2. **Mid-PR-4b-era measurement.** User may have captured at a
   checkpoint *during* the PR 4b umbrella, before 4b-E + 6c reduced
   the artifact. The growth pattern shows there was an intermediate
   peak. A 4b-D.2-era WASM capture (similar to the lint_10kb p99)
   could plausibly have been ~1.6 MB.
3. **Different `wasm-opt` configuration.** User's `wasm-opt`
   invocation may have differed (no `--monomorphize`, different
   `-O` level, different feature flags). The current `release-web`
   profile in `crates/wasm/Cargo.toml`'s
   `[package.metadata.wasm-pack.profile.release-web]` block runs
   `-O3 -O3 -O4` with `--monomorphize` and `--converge`; a different
   configuration could leave +400 KB unrecovered.

**Resolution path: pin the measurement basis in
`tools/wasm-size-check.sh` (already done — measures pre-opt) and
add a CI-side post-opt size annotation in a follow-up gate.** This
PR's `MARQUE_WASM_SKIP_REGRESSION` env var lets the diagnosis
branch ship without flapping the gate; the follow-up PR re-captures
the baseline on GHA after remediation.

**Confidence: MED.** WSL2 measurements don't directly refute the
user's number — they only fail to reproduce it. INVESTIGATE
candidate OTHER-2 covers the GHA-side re-capture.

## 3. Hot-path map at HEAD

See `./2026-05-19-diagnosis/lint-flamegraph-top15.md` for the
synthesized top-15. Headline summary:

- **Top 6 frames** (estimated 13-21% combined inclusive time): all
  added or significantly grown post-pre-pr4. Parsing path
  (`Parser::parse_marking_string`, `parse_rel_to_with_spans`,
  memchr scanner) is unchanged across the umbrella.
- **`CapcoMarking::join_via_lattice` (rank 3)**: the lattice
  composition resolver. Per-page call on the post-flip hot path. 57.9
  KiB native function size; 5 dependent quicksort monos add ~77 KiB.
- **Closure-operator Kleene-fixpoint walk (rank 5)**: per-page,
  ~278ns floor at head (was 75ns at mid-flip). 10+ closure rules
  with predicate + action closures.
- **`evaluate_custom_by_attrs` (rank 10)**: 39 `Constraint::Custom`
  rows dispatched per-portion per-rule.

Flamegraph capture deferred (no `cargo-flamegraph` or `samply` in
the worktree PATH; WSL2 sandbox requires sudo-tweaked
`perf_event_paranoid`). Remediation candidate OTHER-1 covers the
install and re-capture.

## 4. WASM-size attribution

See `./2026-05-19-diagnosis/twiggy-monos-top20.md` for the gap
documentation. Headline:

- **`twiggy monos` returned 0 rows** at all three wasm-pack profile
  configurations on the WSL2 dev host. Name section stripped by
  build pipeline.
- **Substitute attribution via native `cargo bloat`:** ranks the
  same 5 quicksort monomorphizations and the `join_via_lattice`
  function as the top deltas. WASM-specific confidence reduced to
  MED for this PR; HIGH after a names-preserving WASM build lands
  (candidate MO-3 in §5).
- **Net engine-subgraph delta in native binary**: +127.9 KiB across
  marque_capco + marque_engine + marque_ism + marque_core. WASM
  growth is +93 KB, consistent with the engine-subgraph delta
  modulated by WASM's smaller per-instruction encoding.
- `marque_capco` jumped **+182.2 KiB / +83.9%** on its own (217.2 →
  399.4 KiB native), the largest single-crate contribution.

**Scope of this WASM attribution.** The reference range used here is
`pre-pr4` (`18cef6c9`, PR 9c.2, 2026-05-15) → HEAD — i.e., the
PR-4-to-6 cumulative window. The user's framing of the WASM
regression was wider than this: **"~600 KB pre-refactor → ~1.6 MB
current = ~+1 MB regression"** anchors at pre-006 (pre-engine-rule-
refactor), not pre-PR-4. The +94 KB pre-opt / +90 KB post-opt
delta this diagnosis measures is the **PR-4-to-6 slice** of that
wider regression. The gap between pre-006 (which would be a SHA on
`staging` just before the `006-engine-rule-refactor` branch first
landed) and PR 9c.2 is **unmeasured** by this diagnosis and tracked
as INVESTIGATE candidate OTHER-3 (§5). A future reader who wants
the pre-006 → HEAD WASM picture should treat this section as the
PR-4-to-6 segment of that path, not the whole path.

## 5. Ranked remediation table

Per PM contract D-2 (per-candidate fields) and D-8 (EXECUTE /
INVESTIGATE tiers). Scoring: `score = (savings_midpoint_us ×
confidence_pct) / risk_multiplier` where `confidence` is the
column-4 value translated to a coefficient (HIGH=1.0, MED=0.6,
LOW=0.3, N/A=0.0 for infrastructure rows with no own savings
claim), and the risk multiplier is LOW=1, MED=2, HIGH=4. The
`confidence` column itself is derived from evidence quality:
HIGH = bench-validated, narrow range, no instrumentation gap;
MED = structural argument with bench-data scaffolding (e.g.,
cargo bloat shows the regression delta explicitly, per-stage
micro-bench data backs the per-call cost); LOW = hypothesis with
no direct measurement (typically marked by `TBD-instrument`
evidence or by "needs flamegraph" caveats); N/A = infrastructure
or measurement-only candidate whose job is to unblock other
candidates with no savings claim of its own. For WASM-only
candidates `expected_savings_us = 0`, score uses
`(savings_midpoint_kb × confidence_pct) / risk_multiplier` and
is flagged W-prefix. Score is a rough ranking heuristic — scores
in the table may drift up to ~10-20% from a strict `(midpoint ×
conf) / risk` reproduction because the original scoring was
heuristic before the confidence column was made explicit; the
goal is audit-ability, not bit-exact recomputation.

> **READ FIRST — savings-estimate caveat (post-R1 reconciliation).**
> The `expected_savings_us` ranges in the table below are **headroom
> budgets**, not bench-validated estimates. The actual savings are
> bounded by the per-stage micro-bench data captured in
> `./2026-05-19-diagnosis/criterion-checkpoints.md`. Specifically:
> `phase_a_join_via_lattice` is 478ns per call at HEAD,
> `phase_b_closure` is 278ns per call, `phase_g_project_n1` is 704ns
> per call. Aggregate per-stage contribution to `lint_10kb` (1023µs
> mean) is on the order of single-digit-µs to low-tens-of-µs per
> stage. EXECUTE-tier candidates that claim savings > 2× the relevant
> per-stage stage-aggregate must be reconciled with the per-stage
> data before a follow-up PR claims the candidate. R1 review fixups
> applied 2026-05-19 reduced several rows below D-8's noise-floor
> gate (≥30µs lint OR ≥30KB WASM AND risk ≤MED); those rows moved
> from EXECUTE to INVESTIGATE. The remaining EXECUTE-tier row passes
> D-8 via the WASM-floor arm. Tighter savings estimates require
> flamegraph capture (OTHER-1) before any candidate claims a savings
> commitment in a follow-up PR.

| id | title | axis_touched | evidence | expected_savings_us | expected_savings_wasm_kb | risk_class | complexity | confidence | dependencies | correctness_argument | tier | score |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| LA-1 | Replace 5 SciSet/SarSet quicksort closures with `SmolStr::cmp` Ord-based sort | lattice projection / monomorphization | bloat: 5 distinct ~15 KiB monos summing ~77 KiB; one-line closures `\|a, b\| a.cmp(b)` are textually identical and could share a single instantiation. Consolidating 5 → 1 mono removes 4×~15 KiB = ~60 KiB native; WASM proportional after `wasm-opt --monomorphize`. | 5-20 | 40-60 | LOW | S | MED | [] | `SmolStr::cmp` already implements `Ord` and respects CAPCO canonical-order semantics defined in `sar_sort_key` / `sci compartment sort`; no semantic change. Marque-canonicalization invariant unchanged. | EXECUTE | 7.5 |
| LA-2 | Skip `SciSet::to_markings` when `parsed_markings.iter().all(\|p\| p.sci_markings.is_empty())` | lattice projection | profile_project phase_g_n1 +38.9% vs mid-flip indicates fixed per-call cost grew; the empty-axis fast-path skips lattice ops the page can't need. Bench-bounded reduction: phase_g_n1 704ns × ~few-calls × empty-axis subset = single-digit µs aggregate. | 3-15 | 0 | LOW | S | MED | [] | SciSet projection is identity under empty input. Symmetric: applies to SarSet (no SAR portions), FgiSet (no FGI), AeaSet (no AEA). | INVESTIGATE (below D-8 noise floor; investigation: flamegraph capture would tighten the savings range and may surface aggregate empty-axis frequency higher than current estimate) | 5.4 |
| LA-3 | Skip `CapcoMarking::join_via_lattice` body when `portions.len() ≤ 1` (single-portion fast path) | lattice composition | profile_project phase_a_join_via_lattice 478ns at HEAD; phase_g_project_n1 704ns total. A single portion has nothing to join — `out = portions[0].clone()` is the identity case. **Structural reach extends beyond phase_a: short-circuiting `join_via_lattice` also avoids downstream closure-rule firings (phase_b) and post-join lattice composition (phase_c/phase_e) on single-portion pages.** Headroom budget, not bench-validated; reconciliation via flamegraph (OTHER-1) required for a tight savings estimate. | 5-15 | 5-10 | LOW | S | LOW | [OTHER-1] | Lattice identity: `join(x) = x` for any x in a join-semilattice. CAPCO grammar agnostic — applies to any scheme with a singleton page. | INVESTIGATE (below D-8 noise floor; investigation: confirm via flamegraph (OTHER-1) the typical-call single-portion subset and quantify aggregate savings) | 3.0 |
| MO-1 | Extract `Engine::with_clock` generic body to a non-generic inner function | monomorphization | bloat: `<Engine>::with_clock::<CapcoScheme>` 40.4 → 46.6 KiB (+6.2 KiB regression delta). Opportunistic win, not a regression driver — the candidate attacks the broader mono cost surface rather than the +6.2 KiB delta specifically. | 0 | 6-15 | MED | M | MED | [] | Engine construction logic is scheme-parametric only through enumerated trait calls; the body can move behind a `dyn Trait` boundary without semantic change. Devirtualization within a binary with a single concrete S keeps native runtime cost. | INVESTIGATE (below D-8 noise floor on reduced range; investigation: identify additional `<CapcoScheme>` monos beyond `with_clock` whose consolidation gets the cumulative WASM saving above the 30 KB floor) | 3.15 |
| MO-2 | Audit `Arc<dyn Recognizer<S>>` and `Arc<dyn Vocabulary<S>>` for devirtualization | monomorphization / dispatch | bloat: `<DecoderRecognizer>::recognize` 15.6 KiB; baseline.json `_p99_note` documents vtable misses surface at the tail. With one concrete Recognizer impl per binary today, the `dyn` could be replaced by a generic parameter at the engine boundary. p99-only lint claim is inferred from the `_p99_note`, not directly measured. | 10-30 (p99 only) | 5-15 | MED | M | LOW | [] | If only one concrete recognizer ships in a given binary (`StrictOrDecoderRecognizer` is `Engine::new`'s default; alternative is `StrictRecognizer` for benches), the `dyn` is over-abstraction. Replace with `impl Recognizer<S>` generic at the engine surface; same semantic behavior. | INVESTIGATE (below D-8 noise floor on both axes; investigation: real flamegraph capture (OTHER-1) would tighten the p99 vtable-miss attribution and might lift this above the 30µs lint floor at tail) | 3.0 |
| CO-1 | Replace `synthesize_fixes` per-portion `Box<dyn FactInfo>` with a small enum | monomorphization / redundant composition | bloat: 12.2 KiB. The `Box<dyn>` boxing per-fix in the fix path is observed in `<TwoPassFixer>::run` (39.3 KiB). Fix path; not lint hot path. | 20-60 (fix path; not lint hot path) | 10-20 | MED | M | LOW | [] | The closed set of FactInfo variants is finite (~5-7 today). An enum with `match` dispatch beats a vtable for closed sets. No semantic change. | INVESTIGATE (off lint hot path; fix-path savings don't count against the load-bearing `lint_10kb` regression. Investigation: add a `fix_10kb` bench to land remediation against a measurable target before claiming the candidate) | 6.0 |
| HOT-1 | Closure operator: early-exit on empty closure-rule-trigger axes | hot-path closure | profile_project phase_b_closure 75ns mid-flip → 278ns head (+270%). Empty-cone short-circuit exists per architect R-1 mitigation; investigate whether more aggressive axis-empty short-circuits reduce the floor further. **Bench-supported ceiling:** phase_b_closure is 278ns per call; per-`lint_10kb` aggregate is sub-µs to low-µs even at high call frequency. Original 100-200µs claim was off by ~2 orders of magnitude. | 1-5 | 0 | MED | M | MED | [] | Adding more pre-checks to the closure operator's per-call dispatch is correctness-preserving as long as the checks are sound (an axis with no eligible portions has no closure trigger to fire). | INVESTIGATE | 0.9 |
| LA-4 | `from_attrs_iter` constructors: avoid `SmallVec` spill when input is small (≤ 4) | lattice allocation | bloat: smallvec crate +7.9 KiB / +22.3% across umbrella. Per-axis `*::from_attrs_iter` likely spills `SmallVec` when accumulating beyond inline capacity. | 5-15 | 5-10 | LOW | S | MED | [] | `SmallVec`'s spill threshold is configurable. Inline capacity = 4 (typical max axes per portion); raise to 8 if profiling shows the typical-case spills. | INVESTIGATE | 6.0 |
| DI-3 | Audit `Engine::with_clock` for per-call construction tax leakage | dispatch indirection | bloat: `Engine::with_clock::<CapcoScheme>` 46.6 KiB at head (was 40.4 KiB at pre-pr4, +6.2 KiB). Synthesized flamegraph rank 7 inclusive-% suggests per-lint cost may include construction work; verify by flamegraph capture. | TBD-instrument: flamegraph capture | TBD | MED | S | LOW | [OTHER-1] | If construction tax is leaking into per-lint, the fix is moving setup into `Engine::new`'s amortizing path. Semantic-preserving. | INVESTIGATE | 0 |
| MO-3 | Build profile for names-preserving WASM (for `twiggy monos`) | monomorphization audit infra | twiggy monos returned 0 rows; release-web stripped names. Without WASM mono attribution, candidate MO-1 / MO-2 WASM-side savings are MED-confidence inferred from native bloat. | TBD-instrument: add `release-monoaudit` profile | TBD | LOW | S | LOW | [] | Build-system-only edit; no production code change. Affects diagnostic infrastructure only. | INVESTIGATE | 0 |
| HOT-2 | Closure-rule walker: avoid per-iteration `Vec::clone` on cone state | hot-path closure | phase_b_closure +270% absolute is suspicious — most likely the `cone` rebuild allocates per iteration. Confirm via flamegraph. **Bench-supported ceiling:** phase_b_closure aggregate is sub-µs to low-µs; original 50-150µs claim was off by ~2 orders of magnitude. | 1-5 | 0 | MED | M | LOW | [OTHER-1] | Closure operator's Kleene-fixpoint cone can be reused across iterations (or amortized via swap rather than clone). Semantics depend on monotonicity, which closure operators by definition satisfy. | INVESTIGATE | 0.45 |
| CO-2 | Page-rewrite scheduler: pre-compute per-page eligibility mask | redundant composition | 27 PageRewrite rows; many rows have `reads`/`writes` axes that the typical page doesn't touch. The scheduler dispatches all 27; an eligibility mask culls the predicate-eval count. **Bench-supported ceiling:** PageRewrite eval is part of phase_c/phase_e (~2µs/page aggregate); culling half = ~1µs aggregate. Original 30-80µs claim assumed wider-than-actual per-rewrite cost. | 1-5 | 0 | MED | M | LOW | [OTHER-1] | The mask is computed from per-page axis presence (which axes the parsed portions actually touch). Each row's `reads`/`writes` declares its axis dependency; cull at scheduler entry. | INVESTIGATE | 0.45 |
| CA-1 | Audit `parsed_markings` Vec cache for repeated allocations | caching | bloat unchanged on this axis. profile_project shows the cache has linear scaling at HEAD. Suspicion only; needs flamegraph. | TBD-instrument | 0 | MED | M | LOW | [OTHER-1] | If the cache is rebuilt per page when it should be reused, the fix is a structural-sharing pattern (Arc<Box<[T]>>). Semantic-preserving. | INVESTIGATE | 0 |
| OTHER-1 | Install `cargo-flamegraph` (or `samply`) and capture real flamegraphs | profiling infra | This PR's lint-flamegraph-top15.md is synthesized, not measured. ~30 min to install on WSL2 + capture 3 SVGs. | (unblocks others) | 0 | LOW | S | N/A | [] | Tooling install. No production code change. Unblocks DI-3, HOT-2, CO-2, CA-1, and tightens LA-3 / MO-2 estimates so they can be reconsidered for EXECUTE. | INVESTIGATE | 0 |
| OTHER-2 | GHA-side re-capture (lint_10kb + WASM + decoder_10kb at HEAD on `ubuntu-latest`) | measurement basis | WSL2 dev host doesn't reproduce user-reported ~1.6 MB WASM or ~1.7ms lint mean. GHA may. | (unblocks baseline.json update) | 0 | LOW | S | N/A | [] | Runs the existing `scripts/capture-baselines.sh` on GHA. No code change. Recaptures `benches/baseline.json` and `tools/wasm-size-baseline.txt` after remediation lands. | INVESTIGATE | 0 |
| OTHER-3 | Identify pre-006 WASM measurement anchor and capture pre-refactor baseline | measurement / scope | The user's framing of the WASM regression is "~600 KB pre-refactor → ~1.6 MB current = ~+1 MB regression". This diagnosis measured `pre-pr4` (`18cef6c9`, PR 9c.2 = mid-006) → HEAD = ~+94 KB pre-opt / ~+90 KB post-opt. The gap between PR 9c.2 and a pre-006 anchor (commit on `staging` just before the `006-engine-rule-refactor` branch first landed) is **unmeasured** by this diagnosis. The user's recall of ~+1 MB is plausible against a pre-006 anchor but unverified. | TBD-investigation; not a fix | TBD (investigation surfaces the gap, does not close it) | LOW | S | N/A | [] | Measurement-only; no production code change. Investigation: identify the pre-006 SHA on `staging` (last commit before the `006-engine-rule-refactor` branch merged); build `crates/wasm` at that revision via `wasm-pack build --target web --profile release-web` and `wasm-opt -O3` post-process; capture pre-opt + post-opt sizes; compare to current HEAD numbers. Resolves the cross-006 WASM scope gap that this diagnosis (PR 4 → HEAD scope) does not cover. | INVESTIGATE | 0 |
| W-MO-1 | `wasm-bindgen`/`wasm-opt` profile: keep names for monos audit only (build separate artifact) | WASM build infra | Same as MO-3 but specifically for the WASM mono audit. | 0 | 0 (instrumentation) | LOW | S | N/A | [MO-3] | Build-system-only. | INVESTIGATE | 0 |

**Notes on the table:**
- Score is **rough ranking signal**, not an SLA. After R1 review
  fixups (savings reconciled against per-stage bench data; rows
  below D-8 noise floor moved to INVESTIGATE), the EXECUTE tier
  contains a single candidate (LA-1) passing D-8 via the WASM-floor
  arm. The remaining 16 candidates are INVESTIGATE — they are
  parallel work streams the remediation lane can pursue, but each
  needs additional evidence (flamegraph capture via OTHER-1 most
  commonly) before claiming an EXECUTE commitment in a follow-up
  PR.
- **EXECUTE-tier estimated savings:** LA-1 only — lint **5-20µs**;
  native binary **~60 KiB** (5 monos → 1); WASM **40-60 KB** after
  consolidation. The EXECUTE tier alone closes ~1-4% of the 453 µs
  cumulative lint regression and ~45-65% of the +94 KB cumulative
  WASM regression.
- **EXECUTE-tier alone does not close the lint regression.** Closing
  the ~450µs lint gap requires elevating INVESTIGATE candidates to
  EXECUTE after flamegraph capture (OTHER-1) tightens the savings
  estimates. The structural candidates with the largest headroom
  budget are LA-3 (single-portion fast path, structurally reaches
  across phase_a/b/c/e), HOT-1/HOT-2 (closure operator floor),
  CO-2 (PageRewrite scheduler mask). Whether their bench-supported
  savings clear D-8's 30µs lint floor is the open question
  OTHER-1's flamegraph capture is designed to answer.
- No candidate touches CAPCO grammar semantics. Constitution VIII
  citation discipline trivially satisfied (no `§X.Y pNN` citations
  appear in this table because no candidate alters grammar
  behavior).

## 6. Open questions

Mapped to specific INVESTIGATE-tier candidate IDs per PM contract D-8.

1. **Real-flamegraph hot-path attribution** (current: synthesized).
   Unblocks DI-3, HOT-1, HOT-2, CO-2, CA-1, and tightens LA-3 / MO-2
   savings estimates so they can be reconsidered for EXECUTE.
   **→ OTHER-1**.
2. **GHA `ubuntu-latest` numerics at HEAD.** WSL2 numerics don't
   match user-reported HEAD state. **→ OTHER-2**.
3. **Pre-006 WASM measurement anchor.** This diagnosis measures
   the PR-4-to-6 cumulative slice (~+94 KB pre-opt). The user's
   framing of "~600 KB pre-refactor → ~1.6 MB current = ~+1 MB"
   anchors pre-006, not pre-PR-4. The gap between pre-006 and PR
   9c.2 is unmeasured here. **→ OTHER-3**.
4. **WASM monomorphization names.** Without names, WASM-specific
   savings are inferred from native bloat with MED confidence
   instead of HIGH. **→ MO-3 + W-MO-1**.
5. **Closure-operator per-iteration allocation pattern.** +270%
   floor growth is suspicious; could be allocator-driven rather
   than catalog-driven. **→ HOT-2** (depends on OTHER-1).
6. **`Arc<dyn Vocabulary<S>>` p99 cost attribution at HEAD.** The
   PR-2-era cost is baked in across the umbrella; cannot be cleanly
   subtracted from cumulative regression math. **→ MO-2** —
   reclassified to INVESTIGATE post-R1 fixup because the bench-
   supported lint savings are below D-8's 30µs floor; precise p99
   attribution requires per-call sample analysis from a flamegraph
   (OTHER-1 dependency).
7. **Per-portion lattice setup cost vs per-page roll-up.**
   profile_project phase_g sweep gives a rough fit; flamegraph
   would pin it. **→ LA-3** — reclassified to INVESTIGATE post-R1
   fixup because the bench-supported single-portion savings sit
   below D-8's 30µs floor on the per-stage data; the structural
   reach across phase_a/b/c/e suggests a wider headroom budget
   that OTHER-1's flamegraph can quantify.

## 7. What is NOT in this PR

- **No engine-crate edits.** Constitution VII §IV scheme-adoption
  boundary applies; this is a diagnosis lane, not a remediation lane.
- **No baseline updates.** `benches/baseline.json` and
  `tools/wasm-size-baseline.txt` stay pinned. The diagnosis PR
  surfaces the regression; the follow-up remediation PR(s)
  re-baseline after the gap closes.
- **No new bench files or new gated benches.** Per PM contract D-9:
  the existing `profile_project.rs` provides per-stage attribution;
  adding gated benches mid-diagnosis would create maintenance
  surface without proportional value.
- **No flamegraph SVGs committed.** Raw artifacts (when captured)
  are ephemeral; the load-bearing top-N tables are committed in
  `./2026-05-19-diagnosis/`.

## 8. Provenance

Authored 2026-05-19 against branch
`refactor-006-pr-4b-perf-closeout` off `origin/staging` @ `81694384`.
Captures executed on WSL2 dev host (single calendar day, single host).
Cross-host calibration deferred to GHA re-capture (OTHER-2).

All `lint_10kb` / `decoder_10kb` / `profile_project` measurements
land in `target/criterion/` and are deleted before commit (raw
artifacts ephemeral per PM contract D-6). The criterion-derived
numbers in this document are the load-bearing record.

No corpus document text appears in any artifact (Constitution V
Principle V G13 satisfied). Bench inputs are synthetic per
`crates/engine/benches/lint_latency.rs`'s `build_representative_input`
helper.
