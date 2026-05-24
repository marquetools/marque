<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-perf closeout — preflight (diagnosis + measurement + remediation roadmap)

> **Scope.** This file is the preflight plan for a **diagnosis-only** PR.
> No hot-path edits, no `benches/baseline.json` bumps, no production
> code changes. Deliverable PR ships: profiling artifacts, attribution
> analysis, optional new diagnostic instrumentation (only if existing
> benches can't isolate a question), and a written remediation roadmap.
> The PR **will** fail `bench-check` in CI; that failure is expected
> and acknowledged.
>
> **Trigger.** Per project memory `project_perf_baseline_pr5_trigger`:
> "If `lint_10kb` / `decoder_10kb` / wasm-size baselines haven't
> naturally fallen back by end of PR 5 (post-Stage-4-cleanup), user
> commissions dedicated perf-analysis/optimization work."
> PR 5 has now closed; the baselines have not fallen back. This PR
> opens that dedicated lane.

---

## 0. Constraints (binding)

- **Constitution V Principle V (G13, audit content-ignorance).** Profiling
  artifacts (flamegraphs, cargo-bloat reports, criterion outputs) MUST
  NOT contain document text drawn from the corpus. Inputs are
  built from the existing synthetic bench fixtures
  (`build_representative_input`, `build_decoder_input`, `build_prose_input`,
  `build_portion_dense_input`, `build_high_candidate_input`,
  `build_intent_heavy_input`, `build_sci_composite_dense_input`) and
  Lorem-ipsum prose. Corpus fixtures under `tests/corpus/valid/` and
  `tests/fixtures/mangled/` are out of scope for any artifact that
  lands in `docs/perf/`.
- **Constitution VII §IV.** Perf engineering may touch engine crates,
  but this specific PR is diagnosis-only — production edits move
  to a follow-up PR after the PM reviews findings.
- **No silent baseline bump.** `benches/baseline.json` and
  `tools/wasm-size-baseline.txt` are load-bearing. Any change to
  these files would mask the regression this PR is investigating.
  They are not touched.
- **Citations.** §X.Y references to CAPCO use `§X.Y pNN` form.
  No content from `crates/capco/docs/CAPCO-2016.md` is reproduced in
  profiling artifacts; the manual is the spec, not the input.
- **Pre-users (per `feedback_pre_users_no_deprecation_phasing`).** No
  deprecation phasing, no back-compat shims for measurement infra.
  If a bench is wrong, it gets fixed; it does not get "soft-retired".

---

## 1. Current perf state (load-bearing summary)

### 1.1 `lint_10kb` (SC-001 strict-path interactive bench)

| Anchor | Mean / Upper-CI | Source |
|---|---|---|
| Pre-PR-4 (user recollection, "sub-500µs") | < 500µs | User report, not baseline-pinned |
| GHA `ubuntu-latest`, 2026-04-26 (D8 widening, post pre-refactor) | 749µs mean / 753µs upper | superseded baseline in `benches/baseline.json` `_note` |
| GHA `ubuntu-latest`, 2026-05-17 (PR #498 re-capture) | 913µs mean / 914µs upper | current authoritative `benches/baseline.json::lint_10kb.upper_ci_us` |
| WSL2 dev, 2026-05-17 (PR 4b-D.2 perf-mitigation work) | 1033µs mean / 1036µs upper | `benches/baseline.json::lint_10kb._wsl2_dev_capture` |
| User report ("current expectation, cumulative PRs 4-6") | ~1.7ms | user statement, not yet pinned in baseline |

**Cumulative regression.** From the pre-PR-4 reference point to
current HEAD, `lint_10kb` has approximately **doubled to tripled**.
This is cumulative across the PR 4 umbrella (4a + 4b-A + 4b-B + 4b-C +
4b-D.0 + 4b-D.1 + 4b-D.2 + 4b-E), PR 5 (foreign banner correctness +
E068/E069/W004 catalog additions), and PR 6 (PageContext retirement,
T069 lattice composition). It is **not a single-PR regression**; no
single landed PR violated the +10% drift gate against its predecessor's
baseline. Per project memory `project_perf_regression_4_to_6`, the
cumulative shape is exactly the trigger condition the PM scheduled this
investigation for.

### 1.2 `decoder_10kb_one_mangled_region` (SC-002 decoder-path bench)

| Anchor | Mean / Upper-CI | Source |
|---|---|---|
| GHA `ubuntu-latest`, 2026-04-26 | 918µs mean / 919µs upper | superseded |
| GHA `ubuntu-latest`, 2026-05-17 | 1157µs mean / 1158µs upper | current authoritative |

Decoder-path delta is roughly +26% PR-to-PR; not as dramatic as
`lint_10kb` but the decoder path is structurally heavier so absolute
headroom against the 18ms SC-002 ceiling remains comfortable.

### 1.3 WASM artifact size

| Anchor | Bytes / MB |
|---|---|
| Pre-refactor (user recollection) | ~600 KB |
| Current baseline (`tools/wasm-size-baseline.txt`, release-web pre-`wasm-opt`) | 1,386,447 (≈1.32 MB) |
| User report "current state" (post-`wasm-opt`?) | ~1.6 MB |

WASM growth is **+730 KB to ~+1 MB**, depending on whether the
comparison is pre- or post-`wasm-opt`. The shipped artifact uses
`wasm-opt -O3`; the baseline measures the pre-opt artifact (catches
Rust-side bloat at the source).

### 1.4 Constitutional ceilings (NOT violated)

- SC-001: `lint_10kb` p99 ≤ 16ms. Headroom **~15ms** at current rate;
  not in danger of violation.
- SC-002: `decoder_10kb_one_mangled_region` p99 ≤ 18ms. Headroom
  **~16.8ms**; not in danger.

The cumulative regression is the **canary**, not the contract failure.
That distinction matters for CI gate disposition (§4).

---

## 2. Bench coverage assessment

### 2.1 Inventory of `crates/engine/benches/`

The workspace currently ships **10 bench files** with the following
hot-path coverage. (Status column: ⚪ advisory, 🟢 gated by
`scripts/bench-check.sh`, 🔵 informational with target only.)

| File | Bench function(s) | Hot path exercised | Status |
|---|---|---|---|
| `lint_latency.rs` | `lint_10kb` | Full `Engine::lint` strict path on 10KB mixed input | 🟢 SC-001 |
| `lint_latency.rs` | `decoder_10kb_one_mangled_region` | Full `Engine::lint` with one mangled portion forcing decoder dispatch | 🟢 SC-002 |
| `lint_latency.rs` | `lint_default_config` | Engine::lint, no overrides — pins severity-hoist baseline | ⚪ |
| `lint_latency.rs` | `lint_off_heavy_config` | Engine::lint with 22 rules at `Off` — pins severity-hoist win | ⚪ |
| `lint_latency.rs` | `lint_prose_heavy` | Pure-prose 10KB, scanner-pass cost only | ⚪ |
| `lint_latency.rs` | `lint_portion_dense` | 20+ portion markings in 10KB, PageContext + scanner allocator stress | ⚪ |
| `lint_latency.rs` | `lint_high_candidate_count` | 200 minimal portions `(S//NF)`, max (candidate × rule) cross-product | ⚪ |
| `lint_latency.rs` | `lint_intent_heavy_10kb` | JOINT portions firing E014 FactAdd intents | ⚪ |
| `lint_latency.rs` | `lint_parsed_markings_cache_population_stress` | 1000 candidates, cache-population stress | ⚪ |
| `lint_latency.rs` | `lint_sci_composite_dense` | SCI composite parsing (HCS-O / SI-G / TK-BLFH / ...) | ⚪ |
| `lint_latency.rs` | `decoder_deep_scan_mangled_10kb` | Mangled portion every ~500 bytes, decoder hot loop | ⚪ |
| `lint_latency.rs` | `decoder_clean_input_through_fallback_10kb` | Clean text with unknown SCI compartment forcing decoder | ⚪ |
| `lint_latency.rs` | `fix_parsed_markings_cache_stress` | Engine::fix on cache-stress fixture | ⚪ |
| `linear_scaling.rs` | `lint_scaling` | 1KB → 100KB input-size sweep, R² fit | 🟢 SC-005 |
| `fix_throughput.rs` | `fix_throughput` | 1MB → 100MB fix-apply throughput, R² fit | 🟢 |
| `fix_latency.rs` | `fix_single_e054_apply` / `_dry_run` / `_check` | Per-fix latency for one E054 fix | ⚪ |
| `fix_10kb.rs` | `fix_10kb_pass2_only` / `fix_10kb_two_pass` | Engine::fix two-pass pipeline on 10KB | 🔵 target-only |
| `deadline_overhead.rs` | `deadline_overhead_baseline` / `_with_deadline` | Per-candidate `Instant::now()` cost from cooperative cancellation | 🟢 |
| `decoder_10kb_rel_to_invariant.rs` | (issue #234 PR-B) | REL TO USA-injection decoder path | ⚪ |
| `decoder_trigraph_priors.rs` | (issue #233) | REL TO trigraph fuzzy-priors decoder path | ⚪ |
| `recognition_token_heavy.rs` | (issue #431) | Large-marking token-span post-pass cost | ⚪ |
| `profile_project.rs` | (PR 4b-D.2 commit 7) | **Phase attribution probe** — `join_via_lattice`, `closure`, `scheme.project`, `from_canonical`, `project_from_attrs_slice`, full `Engine::lint`, accumulator rebuild | ⚪ |

### 2.2 Coverage matrix vs the regression's suspected sources

The PR 4 + 5 + 6 changes touched eight distinct subsystems. The
matrix below maps each to the bench that most directly measures it,
with **coverage verdict**:

| Subsystem changed in PR 4-6 | Best existing bench | Coverage verdict |
|---|---|---|
| Per-axis lattice constructors (`SciSet` / `SarSet` / `FgiSet` / `AeaSet` / `ClassificationLattice` / `NatoClassLattice` / `JointSet` / `DissemSet` / `NatoDissemSet` / `RelToBlock` / `DeclassifyOnLattice`) | `profile_project.rs::join_via_lattice` (size sweep 1/5/10/25/50 portions) | ✅ adequate — explicit micro-bench exists |
| Closure operator (10 `ClosureRule` rows, Kleene fixpoint walk) | `profile_project.rs::closure` | ✅ adequate |
| PageRewrite topological scheduler + Kahn evaluation (27 rows) | `profile_project.rs::project` (trait-path) | ⚠️ partial — measures combined `project` but does not isolate scheduler dispatch cost from rewrite-body cost |
| Engine fast-path `project_from_attrs_slice` (PR 4b-D.2 commit 7) | `profile_project.rs::project_from_attrs_slice` | ✅ adequate |
| PageContext retirement / `CanonicalAttrs` flow (PR 6c T069) | `lint_portion_dense` + `profile_project.rs::Engine::lint` | ⚠️ partial — no isolated bench for the new `Arc<Box<[CanonicalAttrs]>>` cross-task dispatch shape |
| Parsed-markings cache (Vec vs prior HashMap, issue #432) | `lint_parsed_markings_cache_population_stress` | ✅ adequate |
| FixIntent emission + per-portion FactAdd accumulation | `lint_intent_heavy_10kb` | ✅ adequate |
| Rule-loop dispatch overhead from added rules (38 → 39) | `lint_high_candidate_count` | ⚠️ partial — measures dispatch cost but not the per-rule cost-attribution within `Engine::lint`'s rule loop |

### 2.3 New bench recommendations (conservative)

Per the brief, each new bench is a maintenance surface. I recommend
**zero new gated benches** (no new entries in `benches/baseline.json`)
and **at most two new advisory benches** added inside the existing
`lint_latency.rs` file (no new bench file):

1. **`lint_canonical_attrs_dispatch_stress`** (advisory, optional —
   only if attribution analysis below points to the `CanonicalAttrs`
   cross-task dispatch path as a regression source). Builds a high-
   page-count input (~50 page-break-separated regions) so the
   per-page `Arc<Box<[CanonicalAttrs]>>` allocation fires repeatedly.
   Justification gate: do not add unless flamegraph identifies
   `from_canonical` or `Arc::new` in the top 5 inclusive-time frames.

2. **`lint_pagerewrite_scheduler_only`** (advisory, optional — only if
   attribution points to scheduler dispatch vs rewrite-body cost as
   a load-bearing distinction). Input shape: maximize portion variety
   so all 27 PageRewrite rows fire at least once per page. Compares
   against `lint_portion_dense` (rewrite-body-light) to isolate
   scheduler cost.

**Default position: add neither.** The existing
`profile_project.rs` plus the planned flamegraph methodology (§3)
should be sufficient for attribution. The justification gates above
are deliberately strict.

---

## 3. Profiling methodology

All commands use absolute paths; the working directory for each
invocation is `/home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout`.

### 3.1 `cargo flamegraph` (Linux, WSL2 caveats)

Tool: `cargo-flamegraph` (https://github.com/flamegraph-rs/flamegraph).
Builds the bench in release mode, records via Linux `perf`, generates
an SVG. WSL2 requires elevated permissions for `perf`; WSL2 default
kernel may need `echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid`
for the session. Note this in the artifact metadata; GHA runners do
not require the tweak.

**Install (one-time).**

```bash
cargo install --locked flamegraph
sudo apt-get install -y linux-tools-generic  # WSL2 + Ubuntu hosts
# WSL2-only: relax perf_event_paranoid for the session
echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid
```

**Capture commands** (one flamegraph per benchmark function the
attribution analysis cares about):

```bash
# SC-001 strict path
cargo flamegraph -p marque-engine --bench lint_latency \
  --output /home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout/docs/perf/2026-05-19-flame-lint_10kb.svg \
  -- --bench '^lint_10kb$'

# SC-002 decoder path
cargo flamegraph -p marque-engine --bench lint_latency \
  --output /home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout/docs/perf/2026-05-19-flame-decoder_10kb_one_mangled_region.svg \
  -- --bench '^decoder_10kb_one_mangled_region$'

# Phase attribution
cargo flamegraph -p marque-engine --bench profile_project \
  --output /home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout/docs/perf/2026-05-19-flame-profile_project.svg \
  -- --bench 'project'
```

**What each tells us.** Inclusive-time frames near the top of the
stack point to the call sites doing the work (the lattice helpers /
closure walk / rewrite dispatch). Width = total time spent in that
frame; we look for **disproportionate widening** relative to the
pre-PR-4 baseline. Without a pre-PR-4 flamegraph to compare against,
single-snapshot flamegraphs are still useful: any frame consuming
>5% of total time is a candidate.

### 3.2 `cargo bloat` (binary size attribution)

Tool: `cargo-bloat`
(https://github.com/RazrFalcon/cargo-bloat). Native-target binary
attribution by crate and by function. For WASM, a separate tool
(`twiggy`, §3.3) is more accurate.

**Install.**

```bash
cargo install --locked cargo-bloat
```

**Native binary attribution (CLI crate as proxy for engine code size).**

```bash
cargo bloat --release -p marque --crates -n 30 \
  > /home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout/docs/perf/2026-05-19-bloat-cli-crates.txt

cargo bloat --release -p marque -n 50 \
  > /home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout/docs/perf/2026-05-19-bloat-cli-functions.txt
```

**What this tells us.** The `--crates` report ranks workspace + dep
crates by `.text` section bytes — surface a crate whose code size
exploded between pre-PR-4 and current HEAD. The `--functions` report
ranks individual functions; expect the `Vec::splice` /
`Engine::fix_inner` / generated CVE-lookup tables to dominate, and
look for outliers that should not be in the top 50.

### 3.3 `twiggy` (WASM dead-code + size attribution)

Tool: `twiggy` (https://rustwasm.github.io/twiggy/). Specifically
designed for WASM binary analysis: `monos`, `dominators`, `paths`,
`top` subcommands. Complements `cargo-bloat` because the WASM
target's optimization passes differ from native LLVM codegen.

**Install.**

```bash
cargo install --locked twiggy
```

**Capture (against pre-`wasm-opt` artifact, matching the baseline
gate).**

```bash
cd /home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout

# Build the WASM artifact at the same profile the baseline uses
wasm-pack build crates/wasm --target web --profile release-web

# Top 50 largest WASM functions
twiggy top -n 50 crates/wasm/pkg/marque_wasm_bg.wasm \
  > /home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout/docs/perf/2026-05-19-twiggy-top.txt

# Dominator tree (which functions retain how much code)
twiggy dominators crates/wasm/pkg/marque_wasm_bg.wasm \
  > /home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout/docs/perf/2026-05-19-twiggy-dominators.txt

# Monomorphizations (generic explosion — common source of size growth)
twiggy monos crates/wasm/pkg/marque_wasm_bg.wasm \
  > /home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout/docs/perf/2026-05-19-twiggy-monos.txt
```

**What this tells us.** `monos` is the load-bearing report for this
investigation. PR 4 introduced 12 lattice types implementing the same
trait; if each one is monomorphizing through `JoinSemilattice::join`
or `BoundedJoinSemilattice::bottom` for multiple instantiation sites,
the WASM binary grows linearly in lattice type count. `dominators`
identifies which functions are "retaining" large subtrees of dead-
look code; `top` is the eyeball-test ranking.

### 3.4 Criterion statistical reporting (tail-percentile)

Criterion already records p95 / p99 in `target/criterion/<bench>/new/
sample.json` for every bench run. The `scripts/bench-check.sh` script
has a partial p99 gate (absolute target only; drift gate not wired
because there is no captured `p99_us` baseline yet). For diagnosis,
we read `sample.json` directly:

```bash
cargo bench -p marque-engine --bench lint_latency -- '^lint_10kb$'

python3 -c "
import json, math
with open('/home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout/target/criterion/lint_10kb/new/sample.json') as f:
    s = json.load(f)
per_iter_us = sorted((t / i) / 1000.0 for i, t in zip(s['iters'], s['times']))
n = len(per_iter_us)
print(f'n={n}')
for pct in (50, 90, 95, 99, 99.5):
    idx = int(pct / 100 * (n - 1))
    print(f'p{pct} = {math.ceil(per_iter_us[idx])}µs')
"
```

This is hardware-cheap and gives the long-tail picture that the mean
hides — particularly relevant for the `Arc<dyn Vocabulary<S>>`
indirect dispatch costs introduced by PR 2 (per the `p99_note` in
`benches/baseline.json`).

### 3.5 `cargo asm` / `cargo expand` (spot-check, optional)

For specific call sites where the flamegraph suggests cost
attribution is non-obvious, `cargo-asm` lets us inspect the codegen
of a specific function. **Use sparingly** — the output is large and
only useful when a frame raises a specific question
("is this method getting devirtualized?"). Not part of the routine
capture.

---

## 4. Measurement plan

### 4.1 Reference measurements (mandatory)

Three reference points, named uniformly. **All three captured on the
same WSL2 dev host within one calendar day** so host noise is
controlled; results are NOT calibrated to GHA `ubuntu-latest`
(the WSL2 / GHA delta is documented in `benches/baseline.json::
reference_machine._dev_capture_note`). The reference machine's
identity, kernel, CPU, and capture timestamps are recorded in each
artifact's header line.

| Tag | Commit | Significance |
|---|---|---|
| `pre-pr4` | parent of `fc91852e` (PR 4a) — currently `18cef6c9` PR 9c.2 | The last commit before any PR 4 work landed. User recollection puts `lint_10kb` ≤ 500µs here. |
| `post-pr4` | `e53e4720` PR 4b-C | End of PR 4b umbrella; first measurement point after all lattice work landed |
| `post-pr5` | `e0ce3ec3` PR 5 foreign-banner correctness | End of PR 5 (E068/E069 catalog rows + W004) |
| `head` | current branch tip | Current state including PR 6 PageContext retirement |

Two **intermediate** checkpoints to bracket large regression jumps,
**captured only if the three reference points show >1.5x cumulative
delta** (avoids redundant work):

| Tag | Commit | Significance |
|---|---|---|
| `mid-pr4b-B` | `c9d8ef29` | After per-axis Lattice impls + JOINT W004 |
| `mid-pr4b-D2` | (lookup via `git log --grep="PR 4b-D.2"`) | After hot-path flip to `scheme.project(Scope::Page, ...)` |

### 4.2 Per-checkpoint capture sequence

Each checkpoint runs identically:

```bash
cd /home/knitli/marque/.claude/worktrees/pr-4b-perf-closeout
git checkout <commit>
git clean -dfx target/   # Avoid stale criterion baselines

# Routine bench sweep (all gated + advisory benches)
cargo bench -p marque-engine --bench lint_latency
cargo bench -p marque-engine --bench profile_project
cargo bench -p marque-engine --bench linear_scaling

# Native binary attribution
cargo bloat --release -p marque --crates -n 30 > /tmp/bloat-crates-<tag>.txt

# WASM size + attribution
wasm-pack build crates/wasm --target web --profile release-web
wc -c crates/wasm/pkg/marque_wasm_bg.wasm > /tmp/wasm-size-<tag>.txt
twiggy top -n 50 crates/wasm/pkg/marque_wasm_bg.wasm > /tmp/twiggy-top-<tag>.txt
twiggy monos crates/wasm/pkg/marque_wasm_bg.wasm > /tmp/twiggy-monos-<tag>.txt
```

Flamegraphs captured **only at `head` and `pre-pr4`** (two SVGs per
bench function of interest). Generating flamegraphs at every
intermediate checkpoint is wasteful; the bracketing pair plus the
intermediate criterion numerics are sufficient for attribution.

### 4.3 Artifact storage

All artifacts land in `docs/perf/`, **created in this PR**, file
naming `YYYY-MM-DD-<kind>-<bench>-<tag>.<ext>`:

```
docs/perf/
├── 2026-05-19-README.md                                  # Index + provenance
├── 2026-05-19-summary.md                                 # Findings summary
├── 2026-05-19-numerics-criterion.csv                     # All bench numerics (rows: bench × tag)
├── 2026-05-19-numerics-wasm-size.csv                     # WASM size by tag
├── 2026-05-19-flame-lint_10kb-head.svg
├── 2026-05-19-flame-lint_10kb-pre-pr4.svg
├── 2026-05-19-flame-decoder_10kb_one_mangled_region-head.svg
├── 2026-05-19-flame-decoder_10kb_one_mangled_region-pre-pr4.svg
├── 2026-05-19-flame-profile_project-head.svg
├── 2026-05-19-bloat-cli-crates-head.txt
├── 2026-05-19-bloat-cli-crates-pre-pr4.txt
├── 2026-05-19-bloat-cli-functions-head.txt
├── 2026-05-19-bloat-cli-functions-pre-pr4.txt
├── 2026-05-19-twiggy-top-head.txt
├── 2026-05-19-twiggy-top-pre-pr4.txt
├── 2026-05-19-twiggy-monos-head.txt
└── 2026-05-19-twiggy-monos-pre-pr4.txt
```

**Snapshot semantics, not perpetual storage.** The `2026-05-19-*`
prefix on every file makes the date-of-capture explicit. The next
perf-investigation PR creates a parallel `2026-XX-XX-*` set; old
artifacts are pruned (or moved to `docs/perf/archive/`) after the
remediation PR following this one merges. The `README.md` and
`summary.md` for each pass are exempt from pruning — they are the
historical record. Flamegraph SVGs and twiggy outputs are weighty but
text-compressible; total artifact footprint for this pass is expected
to be ~5-10 MB (10-15 SVGs at ~200-500 KB each, plus text reports).

### 4.4 Findings summary structure (`docs/perf/2026-05-19-summary.md`)

The deliverable summary follows a fixed structure so future perf
passes can pattern-match:

1. **Headline numbers.** Table of `lint_10kb` / `decoder_10kb` / WASM
   size at each tag. Identify the single biggest jump.
2. **Attribution by subsystem.** For each suspected source listed in
   §2.2, evidence from flamegraphs / bloat / twiggy / criterion
   numerics. Verdict: contributes / does not contribute / inconclusive.
3. **Top contributors (ranked).** Three biggest cost drivers, each
   with a one-paragraph proposed remediation and a confidence level
   (high / medium / low).
4. **Remediation roadmap.** Sequence of follow-up PRs, each scoped
   small enough to land independently with its own bench-check
   evidence. The roadmap is recommendations to the PM, not commitments
   — the PM decides scope and order.
5. **Open questions.** What the measurement could not resolve, and
   what additional instrumentation would resolve it.

---

## 5. CI gate disposition

This PR's bench-check job will fail. Three options were considered:

| Option | Mechanism | Future-regression detection |
|---|---|---|
| **A** | Set `MARQUE_BENCH_SKIP_REGRESSION=1` in the workflow for this PR's branch only | Drift gate disabled for this PR only; absolute SC-001 / SC-002 / linear-scaling gates still run. Reverts automatically when the branch merges or is closed. |
| **B** | Add a branch-prefix conditional in `scripts/bench-check.sh` | Permanent code edit; future perf-investigation PRs benefit but the branch-prefix string is a maintenance trap |
| **C** | Accept the red CI check; document in PR body | No mechanism change; the red check is "expected" and reviewers eyeball it |

**Recommendation: Option A with surgical scope.**

Concretely, add a step that exports `MARQUE_BENCH_SKIP_REGRESSION=1`
to the `bench-check` job in `.github/workflows/ci.yml` **only when**
the head ref matches `refs/heads/refactor-006-pr-4b*` or a
`refs/heads/perf/*` prefix:

```yaml
- name: scripts/bench-check.sh
  env:
    MARQUE_BENCH_SKIP_REGRESSION: ${{
      startsWith(github.head_ref, 'refactor-006-pr-4b') ||
      startsWith(github.head_ref, 'perf/')
      && '1' || '' }}
  run: bash scripts/bench-check.sh
```

**Why A and not B.** Option B requires touching the gate script
itself, which raises the question "what other branch prefixes does the
gate skip?" — drift target. Option A keeps the gate script untouched
and applies the override at the CI configuration layer, where branch
filtering is idiomatic.

**Why A and not C.** Red CI on a diagnosis PR is fine in principle,
but the `bench-check` job is `needs: check` — it runs after Format +
Lint completes — and its failure blocks the overall workflow
conclusion. Future contributors landing follow-up work on the same
branch would have to remember "ignore the red bench-check"; that's
a coordination cost the option-A env var eliminates.

**What stays enforced under Option A.**

- ✅ Absolute SC-001 ceiling (`lint_10kb` ≤ 16ms upper-CI). Not
  affected by `MARQUE_BENCH_SKIP_REGRESSION`.
- ✅ Absolute SC-002 ceiling (`decoder_10kb` ≤ 18ms upper-CI).
- ✅ Absolute SC-001 p99 ceiling (16ms applied to p99).
- ✅ Linear-scaling R² ≥ 0.9 (SC-005).
- ✅ Linear-scaling R² ≥ 0.9 for fix throughput.
- ✅ Deadline-overhead ratio ≤ 10%.
- ✅ WASM size 5% regression gate (separate job, separate gate;
  this PR does NOT touch WASM-side code, so this gate continues
  to enforce normally and would only flake if some build-time
  artifact in `docs/perf/` somehow leaked into the WASM target —
  which it cannot, because `docs/perf/` is not in any crate's
  `src/`).
- ❌ Drift gates (baseline + 10% / baseline + 5% p99) skipped on
  this branch.

**Constitutional guard.** Setting `MARQUE_BENCH_SKIP_REGRESSION=1`
unconditionally for the merge to `staging` / `main` would mask
future regressions silently — that's the failure mode this brief
explicitly forbids. Branch-prefix-filtering closes that channel:
the override applies only on the diagnosis branch, and the moment
the remediation PRs merge to `staging`, the drift gate runs again
against the (still-load-bearing) baseline.

---

## 6. Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| WSL2 measurement noise inflates apparent regression | Medium | Medium | Capture pre-PR-4 and HEAD on the **same** WSL2 host within one calendar day, document host identity / kernel / CPU in the README. Cross-reference against the GHA-pinned baseline numbers as a sanity check. |
| Pre-PR-4 commit (`18cef6c9`) does not build cleanly on current rust toolchain | Low | High | Spot-check `cargo build -p marque-engine --benches` at `pre-pr4` before starting the full capture. If broken, walk forward a few commits to find the nearest clean build; document the actual commit used. |
| `cargo flamegraph` requires kernel `perf` access that WSL2 / sandbox lacks | Medium | Medium | Fallback to `samply` (https://github.com/mstange/samply), a perf-free profiler that works in restricted environments. Document the substitution in the README. |
| `twiggy` cannot parse the WASM module (DWARF version mismatch, etc.) | Low | Low | Fallback to `wasm-objdump` for raw section sizes; lose function-level attribution but retain the by-section breakdown. |
| Profiling overhead distorts measurement | Low | Medium | Capture numerics with criterion **without** the profiler attached; capture flamegraphs separately. Do not conflate "criterion-measured time" and "time-as-seen-from-flamegraph". |
| Artifact storage in `docs/perf/` bloats the repo | Medium (over time) | Low | Prune previous-pass artifacts when the follow-up remediation PR merges, keeping only `README.md` + `summary.md` + the numerics CSVs. Document the pruning convention in `docs/perf/2026-05-19-README.md`. |
| Diagnosis surfaces a regression source the PM did not anticipate | Medium | Low | Findings summary lists all sources by evidence weight, not by assumption. PM decides remediation order independently. |
| One of the seven listed perf-investigation tools breaks before the PR lands | Low | Low | Each tool has a documented fallback; missing one of the seven does not block the PR. The flamegraph + criterion numerics + twiggy trio is the load-bearing combination — losing any of those three triggers a manual decision. |

---

## 7. Decision points for PM (≤5)

The PM (the user) must resolve these before implementation begins:

1. **CI gate disposition.** Confirm Option A
   (env-var branch-prefix filter in `.github/workflows/ci.yml`) over
   B (gate-script edit) or C (red CI). Affects scope of CI workflow
   touch in the deliverable PR.

2. **Artifact footprint policy.** Approve `docs/perf/` as the
   artifact home (vs. an out-of-tree gist / external storage), and
   the pruning convention "keep `README.md` + `summary.md` +
   numerics CSVs; prune SVGs and large text reports when the
   next perf-investigation pass lands". This determines whether the
   PR commits ~5-10 MB of measurement artifacts to git history
   permanently or just for the diagnosis window.

3. **Reference measurement scope.** Confirm the three reference
   points (`pre-pr4`, `post-pr4`, `head`) plus conditional
   intermediate checkpoints. Specifically: confirm that
   bracketing the intermediates only on a >1.5x cumulative delta
   trigger is acceptable (vs. measuring every PR boundary
   unconditionally, which roughly doubles capture time).

4. **Optional advisory bench gates.** Confirm the
   "default-no, justification-required" gates for
   `lint_canonical_attrs_dispatch_stress` and
   `lint_pagerewrite_scheduler_only`. If the PM prefers them added
   unconditionally (as instrumentation insurance for future passes),
   say so up front so they go into the deliverable PR rather than
   the follow-up remediation PR.

5. **Follow-up remediation lane.** Confirm the remediation roadmap
   format (§4.4 item 4 — sequence of independently-landing PRs,
   each with its own bench-check evidence). The alternative would be
   a single "big bang" remediation PR; the PM has signaled multi-PR
   preference in past lane plans (cf. the PR 4b sub-PR umbrella),
   so the recommendation defaults to multi-PR — but the PM should
   confirm.

---

## 8. Out-of-scope, deferred

These items are explicitly **NOT** in this PR's scope:

- Any production code edit in `crates/engine/`, `crates/scheme/`,
  `crates/core/`, `crates/rules/`, `crates/capco/`, `crates/ism/`,
  `crates/wasm/`. The PR is diagnosis-only per the brief.
- Re-capturing `benches/baseline.json` or
  `tools/wasm-size-baseline.txt`. These are load-bearing; bumping
  them would mask the regression under investigation. Re-capture
  happens **after** remediation PRs land, not before.
- Adding a Tracy / `tokio-console` integration to the engine. Both
  are heavy long-term observability infrastructure; this PR is a
  measurement snapshot, not a permanent observability install.
- BatchEngine perf (server-side, multi-document concurrent
  throughput). The cumulative regression manifests on single-document
  paths; batch perf is amplified but not differentiated, so a
  single-document investigation is sufficient.
- Decoder priors quality (false-positive rate, SC-003a precision).
  Tracked separately as #258 / #257; orthogonal to the perf
  investigation.

---

## 9. Provenance

This plan was authored 2026-05-19 against the `refactor-006-pr-4b-d-0-
closure-rule-generic` branch (current HEAD). It reflects the perf state
documented in `benches/baseline.json` after the 2026-05-17 PR #498
re-capture and supersedes no prior plan (this is the first dedicated
perf-investigation document in the project's history).

Authored as the preflight for the diagnosis-only "PR 4b-perf closeout"
PR. Implementation (artifact capture, summary authorship) happens in
a follow-up agent dispatch with this plan as the operative scope.
