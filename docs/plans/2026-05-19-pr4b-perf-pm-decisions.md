<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-perf closeout — PM Decisions

**Date:** 2026-05-19
**PM:** Adam (bashandbone)
**Scope:** Diagnosis + measurement + remediation roadmap for the cumulative
perf regression spanning PR 3c → current HEAD. **NOT** an optimization PR.
**Branch:** `refactor-006-pr-4b-perf-closeout` off `origin/staging` @ `81694384`.

## Anchor documents (preflight outputs)

- `docs/plans/2026-05-19-pr4b-perf-preflight-performance.md` — methodology + tooling.
- `docs/plans/2026-05-19-pr4b-perf-preflight-architect.md` — PR shape + plan format + maintenance gates.
- `docs/plans/2026-05-19-pr4b-perf-preflight-attribution.md` — PR-by-PR walkdown.

## Headline framing

The cumulative perf regression is **the deliverable**, not a defect. The user
acknowledged at session open: "we're not going to pass". The PR will fail
`bench-check` and `wasm-size-check` against current baselines; the absolute
constitutional ceilings (SC-001 16ms, SC-002 18ms) are **not** violated. This
PR ships:

1. Profiling artifacts (text summaries + numeric tables; raw SVGs ephemeral).
2. A written attribution narrative resolving the contradictions flagged by
   the attribution walkdown.
3. A ranked remediation roadmap with EXECUTE / INVESTIGATE tiers.
4. CI gate disposition that surfaces the regression without masking it.
5. One long-term maintenance gate that prevents recurrence.

Zero production hot-path edits.

## Decisions

### D-1: PR shape — diagnosis-only, single PR, multi-PR remediation lane

**Accept** the architect's "diagnosis-first, multi-PR" recommendation.

- THIS PR delivers: measurement + diagnosis + remediation plan + CI gate edit
  + one PR-template addition.
- FOLLOW-UP PRs (separate, one per candidate) execute the remediation
  candidates ranked in this PR's plan. Each follow-up carries its own
  bench-delta evidence in PR body and clears `bench-check` for the
  candidate's stated savings.
- Diagnosis is **not** split per-axis. Lattice / dispatch / monomorphization
  interact; a single narrative is load-bearing.

### D-2: Diagnosis output — single ranked findings document

**Accept** the architect's format. Single document at
`docs/perf/2026-05-19-diagnosis.md` containing:

1. **Reference range section** — pre-PR-4 baseline SHA `18cef6c9` (2026-05-15)
   and current HEAD `81694384`. Numeric capture per D-7.
2. **Attribution narrative** — resolution of the three contradictions
   flagged by the attribution walkdown (PR #498 attribution to 4b-B/C,
   PR 4b-E recovery shortfall, WASM measurement basis).
3. **Hot-path map** — flamegraph-derived top-frames by inclusive % at HEAD,
   text-only summary (no SVG embedded in markdown).
4. **WASM-size attribution** — twiggy `monos` + `cargo bloat` summaries.
5. **Ranked remediation table** with the per-candidate fields specified in
   the architect preflight:
   - `id` (category-prefixed: DI / LA / MO / CO / CA / HOT / OTHER)
   - `title`
   - `axis_touched` (lattice / dispatch / monomorphization / closure / etc.)
   - `evidence` (mandatory; empty `evidence` auto-routes to INVESTIGATE)
   - `expected_savings_us` (RANGE not point)
   - `expected_savings_wasm_kb`
   - `risk_class` (LOW / MED / HIGH)
   - `complexity` (S / M / L)
   - `dependencies`
   - `correctness_argument` (CAPCO §X.Y pNN if grammar-touching)
   - `tier` (EXECUTE / INVESTIGATE)
   - `score` (`expected_savings × confidence ÷ risk`)
6. **Open questions** — explicit list of unresolved attributions to be
   followed up in dedicated INVESTIGATE-tier PRs.

Hard scope: findings document ≤ 800 lines, ranked table ≤ 25 candidates
(EXECUTE + INVESTIGATE combined). Over-long is a signal of unfocused
diagnosis, not thoroughness.

### D-3: CI gate strategy — branch-prefix-filtered drift-gate skip

**Adopt the perf-engineer's Option A**, extended to cover the WASM gate.

Implementer edits two scripts and one workflow:

- `scripts/bench-check.sh`: respect existing `MARQUE_BENCH_SKIP_REGRESSION=1`
  env override (already implemented). No script edit required.
- `tools/wasm-size-check.sh`: add analogous `MARQUE_WASM_SKIP_REGRESSION=1`
  env override that skips the +5% gate but **keeps** the build failure modes
  active.
- `.github/workflows/ci.yml`: set both env vars on jobs that run these gates,
  conditional on `github.head_ref` or `github.ref` matching
  `refs/heads/refactor-006-pr-4b-perf-closeout` (exact branch match, not
  prefix match — prefix match risks shadowing future perf-related branches
  silently).

**What stays enforced regardless of skip env vars:**
- SC-001 16ms absolute ceiling on `lint_10kb` `target_upper_ci_us`.
- SC-002 18ms absolute ceiling on `decoder_10kb_one_mangled_region`.
- SC-005 R² ≥ 0.9 linear scaling gate.
- Deadline overhead gate.
- WASM build succeeding (rustc errors, codegen panics, target issues).
- All non-perf CI: tests, clippy, fmt, citation lint, doc check.

If the absolute SC-001/SC-002 ceiling is violated, the gate fails regardless
— defense-in-depth.

### D-4: WASM measurement basis — pin pre-opt vs post-opt explicitly

The attribution walkdown's third contradiction (user-reported ~1.6 MB vs
pinned 1.38 MB baseline) is likely a measurement-basis mismatch.
`tools/wasm-size-check.sh` measures the **pre-`wasm-opt`** artifact
(`pkg/marque_wasm_bg.wasm` from `release-web` profile, before the integrated
opt step which the dev env currently fails at). CI's main `wasm` job runs
`wasm-pack build --profiling` and post-processes with `wasm-opt -O3` to
produce the **ship artifact**, which is smaller.

**Decision:** the diagnosis document MUST capture both numbers at HEAD AND
at pre-PR-4 reference, clearly labeled. Whichever number the user is seeing
(~1.6 MB), the diagnosis pins it to a measurement basis. The remediation
plan's `expected_savings_wasm_kb` field is normalized to the **pre-opt**
basis (matching the existing baseline.txt), because that's where Rust-side
bloat surfaces uncorrupted by `wasm-opt`'s post-processing.

### D-5: Long-term maintenance gates — one in THIS PR, two deferred

The architect proposed three gates. **One** lands in THIS PR; the other two
are deferred to dedicated follow-up PRs to keep this PR's scope clean.

**Lands in THIS PR:**
- **PR-template bench-delta block** at `.github/PULL_REQUEST_TEMPLATE.md`
  (new file). Adds a mandatory checkbox / fillable field for engine-touching
  PRs: "If this PR touches the lint hot path, paste the local `lint_10kb`
  numbers from before-and-after this branch + a one-line rationale." Crews
  catch perf regressions at PR-author time, not at PR-#-N-closeout time.
  Low-cost, no new CI logic. The template change is enforced socially in
  review, not gated by CI — a hard CI gate on PR-description content is
  more Goodhart than signal.

**Deferred to follow-up PRs:**
- **Cumulative +25% regression alert** (informational, not blocking) — fires
  when current `lint_10kb` exceeds the *captured* baseline by 25%, regardless
  of intervening PRs. Requires new CI logic; bundle with the post-diagnosis
  baseline re-capture PR.
- **WASM-monos CI annotation** — twiggy `monos` report emitted as a non-blocking
  PR-comment annotation showing top-10 monomorphizations and the delta vs.
  baseline. Requires a new GitHub Action step + twiggy in CI; bundle with
  the first WASM-optimization PR that uses it.

### D-6: Profiling artifact home — text summaries committed, raw artifacts ephemeral

`docs/perf/` is the home for:

- `2026-05-19-diagnosis.md` — the findings document (text only).
- `2026-05-19-diagnosis/` (subdirectory) — supporting text artifacts:
  flamegraph top-N tables, cargo-bloat output dumps, twiggy monos top-N
  tables, criterion JSON snippets — all text-based, no binary blobs.

**Raw artifacts (flamegraph SVGs, full criterion HTML reports, full bloat
dumps with all symbols) are ephemeral.** The implementer produces them,
extracts the load-bearing top-N data into the committed text summaries,
and discards the raw artifacts. If a future reviewer wants to regenerate
them, the diagnosis document MUST include the exact commands used so they
can be reproduced.

Repo size budget for `docs/perf/` for THIS PR: **< 100 KB total committed.**

### D-7: Attribution methodology — three reference points, same hardware

**Accept** the perf-engineer's three-reference-point scope:
- `pre-pr4`: SHA `18cef6c9` (PR 9c.2, 2026-05-15).
- `head`: SHA `81694384` (current).
- One intermediate: SHA at PR 4b-D.2 merge (`ebbefda0`) — the hot-path flip
  is the load-bearing structural change per the attribution walkdown.

**Conditional fourth and fifth** at the implementer's discretion if the
three-point delta doesn't resolve a contradiction: 4b-B merge (lattice
landings) and 6c merge (PageContext retirement).

**Hardware:** WSL2 dev. Fast iteration, controlled noise. GHA re-capture is
out of scope for this PR — the perf engineer's preflight is clear that the
GHA capture happens AFTER the remediation lands, when there's an actual
new baseline to commit. THIS PR's mission is diagnosis, not re-baselining.

**Samples per checkpoint:** Criterion default `sample_size = 100` is fine
for relative deltas at 5-10% precision; do not raise. The diagnosis tolerates
host noise — the cumulative delta is so large (sub-500µs → ~1.7ms) that
percent-level noise doesn't change conclusions.

### D-8: Remediation plan tier semantics

- **EXECUTE tier**: candidates with `evidence` populated AND `expected_savings`
  > noise floor (≥ 30µs lint or ≥ 30 KB WASM) AND `risk_class` ≤ MED.
  These are ready for follow-up PRs to claim.
- **INVESTIGATE tier**: everything else. Candidates with empty evidence,
  cost below noise floor, or HIGH risk. Each INVESTIGATE candidate gets a
  one-line "what investigation would unlock it" note. INVESTIGATE candidates
  do NOT block EXECUTE candidates — they're parallel work streams.

Roadmap is **recommendations, not commitments**. The PR body must state
this explicitly so future agents don't treat the remediation table as an
implicit PRD.

### D-9: Advisory bench gates — default NO

The implementer MUST NOT add new gated benches to `scripts/bench-check.sh`
without explicit PM authorization in a follow-up message. The existing
`profile_project.rs` already provides per-stage micro-bench coverage
(`join_via_lattice`, `closure`, `scheme.project`, `from_canonical`); use
those for attribution. Per perf-engineer preflight: "be conservative — each
new bench is a maintenance surface."

If the implementer finds a genuinely-uncovered hot path during diagnosis
(unlikely given current bench coverage), the recommendation goes into the
remediation plan's INVESTIGATE tier, not into THIS PR's bench set.

### D-10: PR description shape

PR body MUST include:

1. **Headline acknowledgment**: "This PR FAILS `bench-check` and
   `wasm-size-check`. The regression is the deliverable, not a defect.
   Absolute constitutional ceilings (SC-001 16ms, SC-002 18ms) NOT violated."
2. **Pointer to `docs/perf/2026-05-19-diagnosis.md`** with a 5-line TL;DR.
3. **Pointer to the ranked remediation table** with a 3-row preview of the
   highest-scoring EXECUTE-tier candidates.
4. **Explicit list of CI checks expected to fail** and why each one is
   expected.
5. **Standard PM-cycle attestation paragraph** (preflight → PM decisions
   → implementer → 3-reviewer → submit), citing this file and the three
   preflight docs.

## Implementer brief constraints

- Constitution V Principle V G13: profiling artifacts MUST NOT contain
  document text from the corpus. Token canonicals + spans + digests only.
  Synthetic 10KB inputs (the existing benches' fixtures) are fine.
- Constitution VII §IV: this PR is diagnosis-only. **Zero edits to
  `crates/*/src/**`.** The implementer's diff is bounded to: `docs/perf/`,
  `docs/plans/`, `scripts/bench-check.sh` (no edit needed; already supports
  env override), `tools/wasm-size-check.sh` (add env override), `.github/`
  (PR template + ci.yml branch-conditional env var injection), and
  `specs/006-engine-rule-refactor/tasks.md` (STATUS notes if applicable).
  Any `crates/*/src/**` touch is a stop-the-line event requiring PM consult.
- Constitution VIII: every CAPCO citation in the remediation plan uses
  `§X.Y pNN` form, page numbers only.
- Pre-users (memory `feedback_pre_users_no_deprecation_phasing`): no
  deprecation phasing semantics in any added infrastructure.
- No `git push --force[-with-lease]` without explicit PM authorization
  (memory `feedback_no_unauthorized_force_push`).
- GPG signing required on all commits; never `--no-gpg-sign`.

## Risk register

| Risk | Mitigation |
|------|-----------|
| Agent treats "diagnosis only" loosely and starts editing `crates/*/src/**`. | Brief explicitly; reviewer-3 verifies `git diff origin/staging -- 'crates/*/src/**' | wc -l == 0`. |
| Profiling on WSL2 produces noisy numbers that flap conclusions. | Same-hardware deltas, not absolute numbers, drive the diagnosis. State hardware in every measurement. |
| Diagnosis sprawls into a 2000-line dump. | Hard caps: 800-line findings doc, 25-row ranked table, 100 KB `docs/perf/`. |
| Attribution contradictions don't resolve — we don't know what actually happened. | Explicitly flag each unresolved contradiction in `## Open questions`; assign each to a specific INVESTIGATE-tier candidate. |
| CI gate skip-env-var becomes a precedent that gets copied silently to other branches. | Exact branch match (not prefix) in `ci.yml`; the env-var setting reverts the moment the branch merges. Reviewer-2 checks. |
| PR-template addition triggers spurious CI churn on unrelated PRs. | Template is a markdown file; it cannot break CI. The bench-delta block is fill-in-the-blank, no automated check parses it. |

## Decision points STILL open for PM

None at this time. All preflight decision points are resolved above.

## Approval

This PM contract approves the implementer to proceed under the constraints
above. Implementer should output a one-page implementation report at PR
submission summarizing what was measured, what the top findings are, and
how the deliverables map to this contract's D-1 through D-10.

— PM, 2026-05-19
