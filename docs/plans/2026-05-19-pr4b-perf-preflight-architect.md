<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-perf closeout — architect preflight (scope, remediation structure, maintenance contract)

> **Companion document.** This file is the architect's complement to the
> performance-engineer preflight at
> `docs/plans/2026-05-19-pr4b-perf-preflight-performance.md` (the "perf
> preflight"). The perf preflight covers *how* to measure: tools,
> commands, artifact layout, statistical methodology. This file covers
> *what shape* the PR takes, *how* the deliverable plan is structured
> so future PRs can execute against it, and *what process gates* keep
> us from being in this position again. Read both together.
>
> **Scope of this preflight.** This is for the diagnosis-only PR
> ("PR 4b-perf closeout") whose deliverable is profiling artifacts +
> attribution analysis + remediation roadmap. **Optimization PRs are
> downstream of this one**, scoped against the roadmap this PR
> produces.

---

## 0. Constraints (binding)

These bind both the diagnosis PR and every downstream optimization PR
the roadmap proposes. Repeating them here so the roadmap downstream
inherits them by reference, not by accident.

- **Constitution I (Uncompromising Performance).** SC-001 (`lint_10kb`
  p95 ≤ 16ms) and SC-002 (`decoder_10kb` p95 ≤ 18ms) are absolute
  ceilings. The remediation plan MUST commit to staying under both at
  every PR boundary, not just at the end of the lane. No optimization
  PR is allowed to "temporarily push closer to the ceiling" on the
  promise of a follow-up.
- **Constitution V Principle V G13 (audit content-ignorance).** No
  profiling artifact (flamegraph SVG, twiggy report, cargo-bloat
  output, criterion sample.json, remediation candidate description)
  may quote document text from `tests/corpus/valid/` or
  `tests/fixtures/mangled/`. Synthetic / Lorem-ipsum / token-canonical
  references only. This applies to the remediation plan's
  per-candidate cost estimates too — if a candidate's expected
  saving is grounded in "this input triggers behavior X", the input
  description is *category-level* ("a 10KB document with N portion
  markings of variety V") not byte-level.
- **Constitution VII §IV (scheme-adoption boundary).** The diagnosis
  PR is bench-only / docs-only and does NOT touch engine crates.
  Each subsequent optimization PR MAY touch engine crates (perf
  engineering is the explicit carve-out in §IV); each such PR carries
  its own bench-check evidence and follows the standard reviewer
  chain.
- **Constitution VIII (Authoritative Source Fidelity).** Where the
  remediation plan cites CAPCO behavior to justify retaining or
  restructuring a hot-path branch, the citation uses `§X.Y pNN` form
  and the page reference is re-verified against
  `crates/capco/docs/CAPCO-2016.md` at the time the plan lands. No
  drifted citations propagated by reflex into the roadmap.
- **Pre-users.** No deprecation phasing, no migration shims, no
  "v2-of-trait-side-by-side". If an optimization requires changing
  a trait surface, the trait surface changes. (Per project memory
  `feedback_pre_users_no_deprecation_phasing`.)
- **Engine-only audit promotion (Principle V).** Any optimization
  candidate that involves changing audit-record construction
  (e.g., avoiding repeated `Box::new` on `AppliedFix`) MUST preserve
  `AppliedFix::__engine_promote`'s engine-only invariant in
  production code paths. Test-fixture carve-out unchanged.

---

## 1. Recommended PR scope shape

### 1.1 What goes into THIS PR (the diagnosis PR)

- All profiling artifacts captured per the perf preflight §4.3
  (flamegraphs, bloat reports, twiggy reports, criterion numerics).
- The `docs/perf/2026-05-19-summary.md` findings narrative.
- The `docs/perf/2026-05-19-README.md` index + provenance.
- The `docs/perf/2026-05-19-numerics-*.csv` quantitative data.
- The remediation roadmap (this PR's primary intellectual output —
  format prescribed in §3 below).
- The CI workflow edit per perf preflight §5 (Option A,
  `MARQUE_BENCH_SKIP_REGRESSION` env var, branch-prefix filtered).
- Zero edits to engine crates (`marque-engine`, `marque-scheme`,
  `marque-core`, `marque-rules`, `marque-capco`, `marque-ism`).
- Zero edits to `benches/baseline.json` or
  `tools/wasm-size-baseline.txt` (the load-bearing pins).
- The two advisory bench additions
  (`lint_canonical_attrs_dispatch_stress`,
  `lint_pagerewrite_scheduler_only`) ONLY if the flamegraph
  attribution justifies them per the perf preflight §2.3 gates;
  default-no.

### 1.2 What goes into the follow-up PRs (the optimization lane)

Each remediation candidate from the roadmap is a separately-merging
PR. The default lane shape is **multi-PR**, mirroring the PR 4b sub-PR
umbrella pattern the PM has demonstrated preference for in earlier
lanes (PR 3b sub-A through 3b sub-F, PR 4b sub-A through 4b sub-F).

Each optimization PR carries:

- A scoped change to engine crates implementing one remediation
  candidate (or a tight bundle of dependent candidates).
- A `benches/baseline.json` update IF and ONLY IF the optimization
  produces a measured improvement on a gated bench, captured on the
  same GHA `ubuntu-latest` runner profile the existing baseline uses
  (per `reference_machine.profile`). The baseline only moves
  *downward* on optimization PRs — never upward.
- A bench-check evidence block in the PR description (mean delta,
  p99 delta, before/after numerics, no marketing language per
  `PRINCIPLES.md` "Professional Honesty").
- Adherence to the reviewer chain established by the 4b umbrella
  (architect plan + Rust preflight + lattice review + Rust review +
  code review where applicable).

### 1.3 Why diagnosis-first, not a big-bang fix-it PR

Three reasons, in order of weight:

1. **The biggest cost driver may not be the obvious one.** The user's
   stated suspicion is that lattice composition + dispatch indirection
   are the primary causes. The cumulative shape is consistent with
   that hypothesis, but the cumulative shape is *also* consistent
   with monomorphization bloat (12 lattice types × generic methods
   on `Lattice`), redundant axis composition (multiple lattice types
   walking the same `parsed_markings` slice), or a cache-invalidation
   pattern in the Vec-backed parsed-markings cache. Without
   attribution, an optimization PR is gambling on which suspicion is
   right. A bench-check failure on the wrong gamble costs more agent
   cycles than capturing flamegraphs once.
2. **Constitution V Principle V G13 cuts both ways.** Audit
   content-ignorance is the project's grammar discipline for what
   crosses the boundary into a downstream artifact. Optimization
   candidates have the same property at the design boundary: a
   candidate proposing "skip lattice composition when only one
   portion is present" needs to demonstrate the predicate is
   correctness-preserving, not just measurement-improving. Diagnosis
   produces the evidence base that subsequent reviewer chains can
   anchor their correctness arguments against.
3. **The PM has a documented escalation gate at PR 5 close**
   (`project_perf_baseline_pr5_trigger`). The escalation is "dedicated
   perf-analysis / optimization work", explicitly two activities.
   Bundling them collapses the gate into a single decision; splitting
   them respects the gate's structure.

### 1.4 Diagnosis report — single document, not split per axis

The findings summary is a **single** `docs/perf/2026-05-19-summary.md`
document, not separate reports for lattice / dispatch / monos /
WASM-specific. Three reasons:

- **Cross-axis interactions are load-bearing.** Lattice composition
  cost and monomorphization bloat are not independent — if the same
  lattice type instantiates `join` from N call sites, both axes are
  driven by the same root cause (the number of lattice types ×
  the number of axes they compose on). Splitting the writeup would
  duplicate the root-cause discussion and risk drift between copies.
- **The deliverable is a roadmap, not an inventory.** The PM consumes
  the report to decide order-of-attack. A single ranked list with
  per-candidate confidence is more decision-useful than four
  independent inventories.
- **Future perf passes pattern-match against this one.** The
  `docs/perf/<date>-summary.md` structure (perf preflight §4.4) is
  designed for repeat use. A single-document precedent makes the
  next pass cheaper to author.

The numerics CSVs are split (criterion vs WASM) for grep-ability;
that's a tabular-data concern, not a narrative concern.

---

## 2. Attribution methodology (architect-level)

The perf preflight covers tools and capture commands. This section
covers *meta-methodology*: how we know an attribution is credible,
not just measured.

### 2.1 Granularity: PR-level by default, commit-level on exception

**Default: PR-level.** Each numbered PR (`pre-pr4`, `post-pr4-B`,
`post-pr4-C`, etc.) gets one capture. Within-PR commit granularity
is **not** captured unless the PR-boundary deltas show a single PR
contributing >40% of the total cumulative regression. The threshold
is set high deliberately: bisecting within a PR is high agent-cost
and low informational gain — the PR's own merge commit already
bundles its design intent into one reviewable unit.

**Exception trigger.** If `post-pr4b-D` shows a >40% jump over
`post-pr4b-C`, the perf preflight already calls for an intermediate
checkpoint at `mid-pr4b-D2`. Beyond that, if `mid-pr4b-D2 → post-pr4b-D`
itself shows a >40% jump, escalate to within-PR commit-level capture
(the PR 4b-D series had three perf-mitigation commits — commits 6, 7,
8 — that bear isolating).

### 2.2 Signal vs noise — sample sizing

Criterion's default sample size (100 measurements per bench) gives
~1-2% noise band on the lint_latency benches on GHA, ~3-5% on WSL2.
For PR-boundary attribution, we need to distinguish jumps of >10%
to call them load-bearing; the default sample size is sufficient.

**Exception:** the `lint_10kb` mean has historically flapped within
±2% on shared GHA runners (per the 2026-05-05 D8 widening note in
`benches/baseline.json`). For ambiguous deltas in the 5-15% range,
bump criterion's sample size for that specific checkpoint to 500
via `--sample-size 500` and re-run twice; if both runs land on the
same side of the previous checkpoint's mean, the delta is real.

**Hardware control.** The perf preflight specifies all reference
captures on the same WSL2 dev host within one calendar day. This
controls *across-host* noise. Within-host noise across captures is
defeated by criterion's CI; we don't need to control for it
separately.

### 2.3 WASM size attribution

`twiggy monos` is the load-bearing report. The diagnostic question is
**not** "did the WASM binary grow?" (we know it grew). The diagnostic
question is "which subsystem's monomorphization count grew?". The
candidate axes:

- 12 lattice types × `join` / `meet` / `bottom` / `top` methods
  (BoundedJoin where applicable). Each `impl Lattice` carries its
  own monomorphization unless inlined.
- Generic helpers parameterized on `S: MarkingScheme` (closure rule
  generic refactor in PR 4b-D series). Each instantiation site is a
  separate mono.
- Audit-emission path generics (`AuditEmitter<R>` / similar). If the
  audit path got more generic to support multiple emitters, each
  emitter instantiation is a mono.
- Codec trait surface (Phase 5 T078, pinned but no concrete impls in
  tree). Should contribute zero monos — useful sanity check; if
  twiggy shows Codec-related entries, the surface leaked into the
  reachable graph somehow.

**Diff methodology.** `twiggy diff` (if available between pre/post
WASM modules) is the cleanest comparison; if not, `twiggy monos`
output at each checkpoint into a CSV, joined by function-name-prefix.
Surface the top 10 monos that gained the most bytes between
`pre-pr4` and `head`.

### 2.4 Engine-side dispatch indirection

`Arc<dyn Recognizer<S>>` was introduced in PR 3a (perf preflight §1.1
PR list). The `Vocabulary<S>` trait Arc'd dispatch landed in PR 2
(Phase 5 PR-2 per CLAUDE.md), with the documented p99 cost in
`benches/baseline.json::lint_10kb._p99_note`. Two questions to answer:

1. **Is the indirect call cost showing up in mean or just p99?** The
   `p99_note` says vtable misses surface at the tail. Mean attribution
   needs the flamegraph; p99 attribution needs the per-sample data
   from `target/criterion/<bench>/new/sample.json` (perf preflight
   §3.4).
2. **Does the dispatch site have a single concrete impl at runtime?**
   `marque-engine`'s default installs `StrictOrDecoderRecognizer`
   and `Vocabulary<S>` has exactly one impl (`CapcoScheme`).
   Devirtualization should kick in — *if it doesn't*, the flamegraph
   will show the indirect call frame consuming non-trivial time, and
   the candidate "swap `Arc<dyn>` for a generic parameter at the
   engine boundary" becomes high-confidence.

### 2.5 Per-portion lattice-composition cost scaling

The expected lattice cost is `O(portions × lattice_types × axes)`.
For a 10KB input with ~20 portions and 12 lattice types, that's
~240-300 per-portion ops on the hot path. The
`profile_project.rs::join_via_lattice` micro-bench (size sweep
1/5/10/25/50 portions) is the primary attribution surface — fit the
sweep to a linear model; if R² < 0.95, the cost is super-linear,
which points to nested allocation or quadratic behavior in a
specific lattice type.

### 2.6 Confounds

Two confounds the methodology has to acknowledge up front:

- **The pre-PR-4 commit (`18cef6c9` per perf preflight §4.1) is on
  rust-toolchain 1.85; current HEAD may be on a later patch.** Cargo
  pins the major version. Capture each checkpoint with the *same*
  cargo / rustc combo (the worktree's `rust-toolchain.toml` if
  present, else explicit `rustup default 1.85` for the duration).
  If toolchain drift is the regression source, that's not a marque
  bug; document it and move on.
- **The `Arc<dyn Vocabulary<S>>` documented p99 cost from PR 2 is
  baked into all checkpoints from `post-pr4` onward.** Subtracting
  it cleanly from later regressions requires knowing the PR 2 cost
  in isolation. Pragmatic answer: don't subtract; document the
  PR-2 cost as a known baseline-shift in the summary, and rank
  later regressions against the post-PR-2 floor (not the pre-PR-2
  floor).

---

## 3. Remediation plan structure (the deliverable shape)

This is the form the deliverable `docs/perf/2026-05-19-summary.md §4
Remediation roadmap` takes. The structure is prescriptive — the
implementation agent fills cells, not headers.

### 3.1 Format: ranked table, single sheet

A single table, sorted by `expected savings × confidence ÷ risk`
(higher → earlier in the roadmap). Not a decision tree, not a
prose narrative. The PM consumes this to assign optimization PRs;
the format optimizes for that decision.

### 3.2 Per-candidate fields

| Field | Constraint | Example |
|---|---|---|
| `id` | Stable identifier, prefixed by category (DI = dispatch indirection, LA = lattice allocation, MO = monomorphization, CO = redundant composition, CA = caching, HOT = hot-path closure, OTHER) | `LA-3` |
| `title` | ≤ 70 chars, no marketing | "Avoid `SciSet` allocation when no SCI portions present" |
| `axis_touched` | Which subsystem from perf preflight §2.2 matrix | "lattice composition" |
| `evidence` | What artifact pinpointed this (flamegraph frame name + width %, twiggy mono entry, criterion delta) | "`SciSet::join` 8% inclusive in flame-head; absent in flame-pre-pr4" |
| `expected_savings_us` | Range, e.g. "30-80µs on `lint_10kb`". RANGE not point estimate. If unmeasurable, write "TBD-instrument" | "30-80µs" |
| `expected_savings_wasm_kb` | Range. Zero if not applicable | "0" |
| `risk_class` | LOW / MED / HIGH. LOW = pure-allocation / pure-mono optimization; MED = trait surface change; HIGH = correctness-adjacent (e.g., axis fold restructure) | "LOW" |
| `complexity` | S / M / L. S ≤ 50 LOC, M ≤ 250 LOC, L > 250 LOC | "S" |
| `dependencies` | List of other `id`s this depends on. May be empty | "[]" or `["LA-1"]` |
| `correctness_argument` | One sentence why this is correctness-preserving. Cites CAPCO `§X.Y pNN` if grammar-touching | "SciSet projection is identity when `parsed_markings.iter().all(|p| p.sci_markings.is_empty())`; no semantic change" |
| `tier` | EXECUTE / INVESTIGATE | "EXECUTE" |
| `score` | `expected_savings_us_midpoint × confidence_pct ÷ risk_multiplier` where confidence is high=1.0, med=0.6, low=0.3 and risk multiplier LOW=1, MED=2, HIGH=4 | "27.5" |

### 3.3 EXECUTE vs INVESTIGATE tier

The roadmap distinguishes two tiers:

- **EXECUTE.** Evidence is strong enough that an optimization PR can
  be opened immediately. The PR's design intent fits in one paragraph;
  the bench-check evidence will resolve "did it work" objectively.
- **INVESTIGATE.** Suspicion is high but evidence is partial. The
  follow-up PR is a *measurement* PR (add a targeted bench, capture
  a flamegraph against a more-constrained fixture), not an
  *optimization* PR. The measurement PR's deliverable is the next
  EXECUTE candidate (or proof that the suspicion was wrong).

This split prevents the gambling failure mode in §1.3 reason 1 — an
optimization PR opened on an INVESTIGATE candidate is an
under-evidenced fix-it. Tier-mixing is allowed within the
roadmap; the rank order interleaves EXECUTE and INVESTIGATE candidates
by score.

### 3.4 Candidate categories (the implementation agent's prompt seed)

The implementation agent should expect to populate candidates in at
least these categories (not all categories will have candidates; some
may have many):

- **DI (dispatch indirection).** `Arc<dyn Recognizer<S>>`,
  `Arc<dyn Vocabulary<S>>`, any other `dyn` site found in the hot
  path. Optimization shape: generic parameter at engine boundary,
  monomorphize at the binary surface.
- **LA (lattice allocation).** Per-axis lattice types that allocate
  (SmallVec spill, Box<[T]> construction, set-internal growth)
  when their axis is empty for the current page. Optimization shape:
  early-out predicates, share allocations across pages where
  semantically safe.
- **MO (monomorphization).** Generic methods that instantiate at
  many call sites in the WASM binary. Optimization shape: extract
  non-generic inner functions, replace generic implementations with
  `inline(never) + concrete dispatch`.
- **CO (redundant composition).** Subsystems walking the same
  per-portion slice multiple times when one pass would do (e.g.,
  PageRewrite scheduler + closure operator + per-axis lattice join
  all touching `parsed_markings`). Optimization shape: pass fusion.
- **CA (caching).** Cache-invalidation patterns in the
  `parsed_markings` Vec cache (issue #432) or audit-construction
  Box::new patterns. Optimization shape: structural sharing,
  Arc-based fan-out.
- **HOT (hot-path closures).** Closures captured by reference in
  hot loops (closure rule walker, rewrite scheduler). Optimization
  shape: eliminate closure layer or move to monomorphized function.
- **OTHER.** Anything that doesn't fit above; the field is open so
  the implementation agent isn't forced to mis-categorize a real
  finding.

### 3.5 Uncertainty handling

If the implementation agent cannot quantify a candidate's expected
savings (no isolated bench, no flamegraph frame attributable), the
candidate goes in INVESTIGATE tier with a populated `evidence` field
("suspected from architectural changes in PR 4b-A: 7 lattice types
added simultaneously, but no isolated micro-bench exists for any of
them individually") and a populated `expected_savings_us` field
reading "TBD-instrument: add bench `<name>` and re-capture".

INVESTIGATE-tier candidates do NOT block EXECUTE-tier candidates that
share their axis; the roadmap is parallel-actionable wherever
candidates have empty `dependencies`.

### 3.6 What the roadmap is NOT

It is not:

- A commitment from the PM to ship every candidate.
- A timeline. No "Q2 2026 / Q3 2026" framing — the project is solo-
  driven (`project_solo_driven`) and timeline commitments don't
  apply.
- A guarantee that EXECUTE-tier candidates are easy. The risk class
  + complexity fields carry the difficulty signal independently.
- A replacement for individual PR design — each optimization PR
  still gets its own architect plan / Rust preflight / reviewer
  chain. The roadmap is the *seed*, not the *spec*.

---

## 4. Long-term maintenance contract

What process gates prevent us from being in this position again?
The PR 4-5-6 cumulative regression slipped through because each
individual PR was below the +10% drift gate. The gate's signal was
correctly weak (each PR was correctly individually-acceptable); what
was missing was the cumulative signal.

### 4.1 Recommended gates (ranked by load-bearing)

**Gate 1 (load-bearing): cumulative-regression alert.** Beyond the
existing per-PR +10% drift gate, add a cumulative gate that fires
when the current bench-check measurement exceeds the *prior* baseline
*captured commit* by more than +25% — independent of how many PRs
have landed since. The alert is informational only (not blocking)
because the +25% threshold is wide enough that hitting it is a
deliberate signal, not noise.

Concretely: `scripts/bench-check.sh` reads
`benches/baseline.json::lint_10kb.upper_ci_us` and the
`benches/baseline.json::reference_machine.date_captured` field. When
the current measurement exceeds the captured baseline by >25%,
print a CI annotation "cumulative regression > 25% since baseline
captured YYYY-MM-DD; consider triggering a perf-investigation pass".
Does not block the build.

Implementation cost: ~20 LOC in `scripts/bench-check.sh`. Maintenance
cost: zero per PR.

**Gate 2 (load-bearing): mandatory bench delta in every PR
description.** Every PR that lands a change to engine crates carries
a structured bench-delta block in its description. Format prescribed
by `.github/pull_request_template.md`:

```
## Bench delta

- lint_10kb: <prev mean → current mean> (<delta_pct>%)
- decoder_10kb_one_mangled_region: <prev → current> (<delta_pct>%)
- WASM size: <prev bytes → current bytes> (<delta_pct>%)

Hardware: <profile>
Source: <local capture | CI run URL>
```

If the PR touches no engine crates (e.g., docs-only, test-only),
write "N/A — no engine-crate changes" and explicitly cite the paths
to demonstrate. Discipline mirrors the existing `## Test plan`
section that the project already enforces.

Implementation cost: PR template edit (~30 lines). Enforcement
cost: reviewer discipline; an unfilled bench-delta block is a
review comment, not a CI gate.

**Gate 3 (load-bearing): WASM-monos report deltas in CI.** Add a
job `wasm-monos-report` in `.github/workflows/ci.yml` that runs
`twiggy monos` on the PR's WASM artifact and on `origin/staging`'s
WASM artifact, then diffs by function-name. Output the top 10
deltas as a CI annotation. Non-blocking; informational.

The diagnostic value of `twiggy monos` is highest when the diff is
small (an unexpected new mono entry from a generic-explosion bug is
immediately visible). Running it on every PR means we catch generic-
explosion regressions before they accumulate. Implementation cost:
~50 lines of workflow YAML + ~30 lines in a `scripts/wasm-monos-
diff.sh` helper.

### 4.2 Recommended gates (ranked by maintenance cost)

**Gate 4 (lower priority): quarterly baseline re-capture cadence.**
Schedule a quarterly reviewer-attested baseline re-capture. The
attestation states "the bump from $previous to $new represents an
accepted cost of features X, Y, Z — not a silent escape valve from
the regression gate". The attestation lands in
`benches/baseline.json::reference_machine._note` as a chronological
log entry.

Without this, the temptation under future cumulative pressure will
be to bump the baseline once "to clear the gate", and the silent
escape valve manifests. With the attestation discipline, every
upward bump is paired with an explicit reviewer signoff and a
recoverable trail.

Implementation cost: process gate, not code. Enforce via a calendar
reminder or a recurring GitHub issue. Maintenance cost: ~30 min/
quarter to author the attestation.

**Gate 5 (lower priority): `docs/perf/` directory as historical
record.** The `docs/perf/<date>-summary.md` documents accumulate as
a chronological perf history. The deliverable PR's pruning
convention (perf preflight §4.3) keeps the directory bounded
(summaries retained, weighty artifacts pruned). Each future perf
investigation pass adds a `<date>-*` sub-collection.

This gate is purely the *commitment to keep doing this*, not a
specific mechanism. The mechanism is the documentation pattern the
diagnosis PR establishes.

### 4.3 Anti-recommendations

Things NOT to add as gates:

- **Per-PR mandatory flamegraph capture.** Too expensive per PR;
  generates artifact churn; produces signal only when something is
  actually wrong. Reserve for investigations.
- **Sub-25% cumulative-regression hard-blocking gate.** A hard block
  at low thresholds will trip on legitimate feature work and
  pressure reviewers to silently bump the baseline (Goodhart's law
  on the gate itself). The +25% informational threshold is wide
  enough to be deliberate, not noise.
- **Tracy / tokio-console permanent install.** Long-term
  observability infrastructure with non-trivial overhead. The
  project's perf model is "measure cleanly, optimize, re-measure"
  not "always-on instrumentation". Reserve for production server
  deployments if and when the server crate ships at scale.
- **External performance dashboard (Grafana / CI metrics aggregator).**
  Same reasoning as Tracy — the project is solo-driven and pre-users.
  The criterion + bench-check + `docs/perf/` triad is sufficient for
  the project's actual scale.

---

## 5. Risk register (top 5)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| **R1: Diagnosis identifies the wrong primary cause; optimization PRs land against the wrong axis.** The capture methodology might miss a cost driver hidden behind another (e.g., generic monomorphization masking lattice-allocation cost in the inclusive-time flame). | Medium | High | Multi-axis evidence: every candidate cites at least one of (flamegraph frame, twiggy mono entry, criterion micro-bench delta). Single-evidence candidates default to INVESTIGATE tier. Re-measure after each optimization PR; if the expected savings don't materialize, the candidate's evidence was wrong and the next candidate gets re-scored. |
| **R2: Pre-PR-4 reference commit (`18cef6c9` per perf preflight) fails to build cleanly on current toolchain, contaminating the cumulative-delta math.** | Low | High | Spot-check `cargo build -p marque-engine --benches` at `pre-pr4` before the full capture pass. If broken, walk forward to first clean build, document in `docs/perf/2026-05-19-README.md` provenance section. The cumulative delta becomes "from earliest clean build" rather than "from pre-PR-4", with the offset explicitly named. |
| **R3: Roadmap candidates are populated speculatively (no evidence) and reviewers approve them anyway, leading to optimization PRs that don't move the needle.** Solo-driven project + reviewer-chain discipline = high reviewer trust + low cross-check. | Medium | Medium | The `evidence` field is mandatory in the per-candidate table. Empty `evidence` → INVESTIGATE tier automatic (no EXECUTE without an artifact reference). Architect / code-review chain on each optimization PR is the second checkpoint. |
| **R4: Cumulative-regression alert (gate 1) misfires on legitimate feature complexity additions, creating gate-fatigue and reviewer pressure to silently re-baseline.** | Medium | Medium | The +25% threshold is wide; the alert is informational not blocking; the quarterly attestation gate (gate 4) is the recovery path when a re-baseline is legitimately warranted. Set up the alert with explicit "ignore if reviewer signs off in PR description" semantics. |
| **R5: Optimization PRs accumulate technical debt by individually optimizing axes that should be jointly restructured.** Cumulative perf wins paired with cumulative architectural debt = next refactor's setup cost. | Low | Medium | The roadmap's `dependencies` field surfaces axis-coupling explicitly. Cross-axis optimizations (e.g., DI + CO together) land as one PR even if individually-scoped EXECUTE candidates exist for each, because their `dependencies` chain forces the order. Reviewer architect chain on each optimization PR validates this discipline. |

---

## 6. Decision points for PM (≤5)

The PM (the user) resolves these before the diagnosis PR opens for
implementation. The perf preflight's §7 PM-decision list covers
measurement-mechanics questions (CI gate disposition, artifact
storage policy, reference-measurement scope, optional bench gates,
multi-PR vs big-bang lane). The list below covers
architect-structural questions; the two preflights do not overlap.

1. **Maintenance-gate adoption scope.** Confirm which of the five
   recommended gates land in the diagnosis PR (gates 1, 2, 3 are
   modest CI / template edits; gate 4 is a calendar commitment;
   gate 5 is a documentation pattern). My recommendation: gates 1-3
   land in the diagnosis PR alongside the artifacts; gates 4-5
   become operational policy without a code edit. PM may prefer to
   defer all gate-additions to a follow-up PR.

2. **Roadmap format confirmation.** Approve the single-table /
   per-candidate-fields / EXECUTE-vs-INVESTIGATE-tier structure in
   §3 above. The alternative shape would be a per-axis decision
   tree or a prose narrative; the recommended shape optimizes for
   PM-decision use, not author convenience. If the PM has different
   downstream consumers in mind, the shape should adapt up front.

3. **Roadmap commitment semantics.** Confirm the roadmap is
   *recommendations*, not *commitments* — each downstream
   optimization PR is opened only on explicit PM dispatch, not
   automatically from the EXECUTE tier. (My default reading.)

4. **Reviewer chain for optimization PRs.** Confirm the optimization
   PRs each carry the standard chain (architect plan + Rust preflight
   + lattice review where lattice-touching + Rust review + code
   review where applicable). Setting expectation up front prevents
   later optimization PRs from short-cycling the reviewer discipline
   under perf pressure.

5. **WASM-side regression policy.** WASM size has grown +730 KB to
   ~+1 MB cumulative. Confirm WASM-size regressions are in scope for
   the remediation roadmap (alongside `lint_10kb` and `decoder_10kb`
   latency), and not deferred to a separate WASM-only investigation
   pass. The diagnostic instrumentation overlap is significant; my
   recommendation is to keep them combined, but the PM may prefer
   to scope this PR to native latency only.

---

## 7. Cross-references

- Perf preflight (companion):
  `docs/plans/2026-05-19-pr4b-perf-preflight-performance.md` —
  covers tooling, capture commands, artifact layout, statistical
  methodology, CI gate disposition, in-scope deferrals.
- Trigger memory: `project_perf_baseline_pr5_trigger` — PR 5 close
  is the documented escalation gate; this lane opens that.
- Cumulative-regression context:
  `project_perf_regression_4_to_6` — the cumulative shape (sub-500µs
  → ~1.7ms) is not a single-PR regression, no individual PR violated
  the drift gate, and the cumulative trend is the canary not the
  contract failure.
- Bench-baseline staleness context:
  `project_bench_baseline_staleness` — the existing baseline (912/
  913/914) is near the noise envelope.
- Constitution Principle I (Uncompromising Performance) — SC-001
  and SC-002 absolute ceilings bind every downstream optimization
  PR.
- Constitution Principle V (Audit-First Compliance), G13 invariant
  — audit content-ignorance applies to perf artifacts and remediation-
  plan candidate descriptions.
- Constitution Principle VII §IV — perf engineering carve-out
  permits engine-crate touches in optimization PRs; the diagnosis
  PR remains scheme-adoption-neutral.

---

## 8. Provenance

Authored 2026-05-19 on branch `refactor-006-pr-4b-d-0-closure-rule-
generic`, as the architect's preflight for the "PR 4b-perf closeout"
diagnosis PR. Pairs with
`docs/plans/2026-05-19-pr4b-perf-preflight-performance.md` (perf
preflight, same date, same branch). Implementation of the
deliverable (artifact capture, summary authorship, roadmap
authoring) happens in a follow-up agent dispatch with both
preflights as the operative scope.

Supersedes no prior plan. The PR 4b umbrella closeout
(`docs/plans/2026-05-19-pr4b-closeout-architect-plan.md`) is a
separate, sibling lane — closeout aggregates the umbrella's
within-006 precedent attestation; this lane addresses the cumulative
perf regression the umbrella generated.
