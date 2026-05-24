<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-perf closeout — Attribution / root-cause review (R3)

> Reviewer 3 of 3. Scope per dispatch: verify the attribution narrative
> and contradiction-resolution discipline. Focus on whether the
> diagnosis's causal claims survive merge-history cross-check and
> whether each EXECUTE-tier candidate is actually traceable to a cause
> the narrative identifies.

## Overall verdict

**APPROVE WITH FIXUPS.**

The attribution narrative is **substantively defensible**. The three
contradictions flagged by the attribution preflight are each either
resolved against checkpoint evidence (Contradictions 1 and 2) or
cleanly escalated to a named INVESTIGATE candidate (Contradiction 3).
The diagnosis exhibits genuine falsification discipline — three of the
user's recalled numbers are explicitly disproved on this host
(~1.7ms ≠ HEAD mean, ~1.6 MB ≠ HEAD WASM, +400 KB WASM growth ≠ +94
KB measured). The merge-history cross-check confirms the structural
claim: PR 4b-B added the lattice trait impls' source code at
`c9d8ef29`; PR 4b-D.2 at `ebbefda0` made them reachable on the hot
path. Pre-flip `engine.rs` has 0 occurrences of `join_via_lattice` or
`scheme.project`; post-flip has 5.

What blocks unconditional approval is a small set of cosmetic /
numerical defects that don't affect the diagnosis's load-bearing
conclusions but do undermine the artifact's credibility as a
reviewable record:

1. The CI workflow's branch-conditional env-var injection at
   `.github/workflows/ci.yml:544` and `:832` checks only
   `github.ref`, not also `github.head_ref`. For `pull_request`
   triggered workflow runs (which this workflow uses, line 10),
   `github.ref` is `refs/pull/N/merge`, never
   `refs/heads/refactor-006-pr-4b-perf-closeout`. The env var will
   evaluate to empty string on every PR run, the skip will not
   activate, and the gates will fail with normal thresholds. This
   directly contradicts PM contract D-3 which specifies
   "conditional on `github.head_ref` or `github.ref` matching".
   The other branch-gates in this file (lines 84, 166, 249) use
   `||` to cover both.
2. The EXECUTE-tier savings math in §5 of the diagnosis ("75 + 70
   + 30 + 40 + 5 + 105 µs = 325µs lower bound", "~440 µs upper
   bound") does not match the per-row numbers in the same table.
   Walking the rows yields ~105µs lower / ~290µs upper. The
   325/440 figures appear to be an off-the-cuff estimate that
   wasn't reconciled against the table after the table was
   finalized.
3. The implementer's report claims a "D-7 made the intermediate
   checkpoints conditional on >1.5x cumulative delta" deviation
   — D-7 says the opposite. The third reference point (mid-flip)
   is unconditional in D-7; only 4b-B and 6c were conditional. The
   1.5x language is from the perf preflight's earlier four-point
   scheme, which D-7 superseded with a three-point scheme.

None of these undermine the diagnosis's conclusions — the
narrative is right; the connective tissue is sloppy in places.

## Required fixups (BLOCKING)

- [F1] **Fix the CI branch-gate condition.** Change
  `.github/workflows/ci.yml:544` and `:832` to:

  ```yaml
  MARQUE_WASM_SKIP_REGRESSION: ${{ (github.ref == 'refs/heads/refactor-006-pr-4b-perf-closeout' || github.head_ref == 'refactor-006-pr-4b-perf-closeout') && '1' || '' }}
  ```

  Mirror for `MARQUE_BENCH_SKIP_REGRESSION` at `:832`. Without this,
  the env-var skip will never fire on PR-triggered runs (the workflow
  is triggered on `pull_request` per line 10; `github.ref` for those
  runs is `refs/pull/N/merge`). This is a direct violation of PM
  contract D-3, which explicitly required `head_ref OR ref`. The
  three other branch-gates in the same workflow (lines 84, 166, 249)
  follow the correct `||` pattern, so the implementer demonstrably
  knew this; the omission is mechanical, not architectural.
  **Severity: HIGH** — the deliverable (a PR that visibly fails
  bench-check & wasm-size-check without the gates blocking merge)
  works only if the skip fires. Without F1, the PR fails the gates
  AND blocks merge.

## Recommended fixups (NICE-TO-HAVE)

- [R1] **Reconcile the EXECUTE savings math.** In §5 of the
  diagnosis, the claimed totals "75 + 70 + 30 + 40 + 5 + 105 µs =
  325µs lower bound" and "~440 µs upper" do not arithmetically
  match the rows. Walking the EXECUTE-tier rows: LA-1 lower 5 /
  upper 20; LA-2 lower 30 / upper 80; LA-3 lower 40 / upper 100;
  MO-1 lower 0 / upper 0 (WASM-only candidate, 0µs lint savings
  by its own row); MO-2 lower 10 / upper 30; CO-1 lower 20 /
  upper 60 (and CO-1 is fix-path, not lint hot-path, per its own
  row caveat). Sum: lower 5+30+40+0+10+20 = **105µs** (or 85µs
  excluding CO-1 fix-path); upper 20+80+100+0+30+60 = **290µs**
  (or 230µs excluding CO-1). Either correct the table or rederive
  the totals. The implementer's intent appears to be "round
  bands per EXECUTE candidate" but the numbers should at least
  bound the row values. Severity: MEDIUM — the 325µs vs 105µs
  delta is the difference between "EXECUTE candidates alone close
  ~70% of the regression" and "EXECUTE candidates alone close
  ~25% of the regression"; the second is true and matters for
  the remediation strategy.
- [R2] **Correct the deviations note in the implementer report.**
  `docs/plans/2026-05-19-pr4b-perf-implementation-report.md:91`
  claims "D-7 made the intermediate checkpoints conditional on
  >1.5x cumulative delta". D-7 didn't say that; the perf preflight
  did, in the older four-point scheme that D-7 superseded. The
  mid-flip checkpoint is unconditional under D-7. This is a
  cosmetic blemish in the report — not in the diagnosis itself —
  but it makes the reviewer chain harder to follow.
- [R3] **Tighten the MO-1 candidate's WASM savings range.**
  MO-1's `expected_savings_wasm_kb` is 30-40 KB, but the supporting
  bloat evidence in §4 + the cargo-bloat-top20.md notes show the
  native delta for `Engine::with_clock::<CapcoScheme>` is +6.2 KiB
  (40.4 → 46.6 KiB pre-pr4 → head). The 30-40 KB savings number
  presumably refers to the whole monomorphized function size,
  not the regression contribution. The candidate is defensible
  but the 30-40 KB number doesn't trace cleanly to the cited
  evidence. Either clarify the basis or tighten the range to
  match the delta. Severity: LOW.

## Contradiction resolution review

### Contradiction 1: PR #498 / 4b-B/C attribution

The attribution preflight flagged that PR #498's commit message
attributed the 828→914µs `lint_10kb` baseline jump to "intervening
staging merges (PR 4b-B/C lattice landings, decoder priors,
parser/recognizer additions)" — but PR 4b-B/C are *pre-flip*; the
lattices were consumed only in tests until PR 4b-D.2 flipped the
hot path.

**Implementer's resolution** (diagnosis §2.1): "the attribution was
diplomatic, but defensibly so. PR 4b-B/C added the *source code* for
the cost; PR 4b-D.2 added the *reachability*. A precise attribution
would say: 'the structural work added in 4b-B/C, made reachable by
4b-D.2.'"

**Evidence cited**:
- cargo bloat at pre-pr4 has no lattice trait impls in top-50
  (`marque_capco` 217.2 KiB, no `from_attrs_iter`/`to_marking`/`to_markings`
  monos visible).
- cargo bloat at head has all 5 quicksort monomorphizations from
  `to_marking`/`to_markings` as top-15 entries (bloat-doc lines
  62-72).

**My cross-check via `git log` + `git show`:**
- `git diff --stat c9d8ef29~..c9d8ef29 -- 'crates/capco/src/lattice.rs'`
  shows `+2510 / -200` lines at PR 4b-B's own diff. Lattice trait
  impl source code lands at this merge.
- `git show c9d8ef29:crates/engine/src/engine.rs | grep -c
  'join_via_lattice\|scheme.project'` returns **0**.
- `git show ebbefda0:crates/engine/src/engine.rs | grep -c
  'join_via_lattice\|scheme.project'` returns **5**.

The merge history confirms: at PR 4b-B's merge, the engine does
not call into the lattice traits at all. PR 4b-D.2 is the
reachability event. The diagnosis's "diplomatic but defensible"
verdict holds — PR #498's commit message wasn't wrong; it was
compressed in a way that lost the temporal nuance.

**Verdict: RESOLVED.** Confidence MED-HIGH per the diagnosis.
Merge-history cross-check agrees with the diagnosis's framing.

### Contradiction 2: PR 4b-E recovery shortfall

The `_note` in `benches/baseline.json::lint_10kb` (verified — see my
own check via `grep` on the baseline.json file) explicitly stated PR
4b-E "will retire the remaining residue-axis tmp_ctx requirement,
expected to bring the GHA value back down." The user reports it
didn't.

**Implementer's resolution** (diagnosis §2.2): "PR 4b-E + 4b-F + 6c
DID recover headroom, but only ~16% of the post-flip regression.
The pre-flip → mid-flip jump was +114% on the mean (570 → 1219µs);
the mid-flip → head recovery was -16.1% (1219 → 1023µs)."

**Three reasons offered for the under-recovery**:
1. Post-PR-4b-D.2 closure-rule catalog growth (6 rows added in
   #519, #521, #529, #540, #544, #548). profile_project
   phase_b_closure 75ns mid-flip → 278ns head, +270%.
2. PageRewrite catalog grew 14 → 27 rows.
3. Engine::with_clock grew +6.2 KiB, suspicion of construction
   tax leakage (escalated to INVESTIGATE DI-3).

**Evidence cited**:
- profile_project per-stage numerics in
  `criterion-checkpoints.md`. The phase_b_closure +270% claim
  reconciles to the table (75.1ns mid-flip → 277.9ns head).
- phase_i_join_n10/n25 -40% improvement corroborates PR 4b-E +
  6c's per-portion savings.

**My cross-check**:
- The mid-flip → head delta -196µs (-16.1%) is real per criterion
  numerics. The +114% pre-flip → mid-flip is also real.
- The "post-4b-D.2 catalog growth ate the headroom" hypothesis is
  testable but not actually tested. The profile_project
  phase_b_closure +270% is a +203ns absolute increase. That's
  ~0.02% of `lint_10kb`'s 1023µs — much smaller than the missing
  recovery headroom (which is ~250-450µs depending on whether
  you target the +114% jump being fully retired). The closure-
  growth narrative is consistent with the directional regression
  but doesn't quantitatively explain the gap; it just identifies
  one contributor.
- The PageRewrite catalog 14 → 27 growth IS visible via merge
  history (PR 4b-C added 9 rows; PR 5 + post-4b fixes added more).
  Per-row cost isn't directly measured but is bounded by the
  topological scheduler's per-call dispatch which runs at
  construction.

**Verdict: RESOLVED with caveat.** The contradiction is resolved
in the directional sense — PR 4b-E + 4b-F + 6c DID recover
headroom (-16%), they just couldn't close the +114% post-flip jump
because growth happened concurrently. The diagnosis appropriately
flags HOT-2 + DI-3 + OTHER-1 as the INVESTIGATE candidates that
would tighten the attribution. Confidence HIGH per the diagnosis.
I would have wanted the diagnosis to be more explicit that the
+270% closure-floor delta is quantitatively small relative to the
recovery shortfall — the diagnosis lets a reader assume it's
load-bearing when it's only one of several contributors. R1's perf
review may have caught this.

### Contradiction 3: WASM measurement basis

`tools/wasm-size-baseline.txt` = 1,386,447 B (pre-opt). User reports
~1.6 MB. The CI ships a smaller `wasm-opt -O3`'d artifact.

**Implementer's resolution** (diagnosis §2.3): "HEAD WASM on WSL2
dev is 1,311,681 B (pre-opt) / 1,229,883 B (post-opt minimal).
User's ~1.6 MB does not reproduce on either basis on this host."

Three candidate explanations offered (cross-host divergence,
mid-PR-4b-era measurement, different wasm-opt config). None
selected as definitive. Resolution path: pin both bases in the
diagnosis, ship the GHA re-capture as INVESTIGATE OTHER-2.

**Evidence cited**:
- WSL2 dev capture at pre-pr4 = 1,218,209 B; head = 1,311,681 B;
  delta +93,472 B (+7.67%) pre-opt.
- Post-opt minimal: 1,139,451 → 1,229,883 = +90,432 B (+7.93%).
- baseline.json `_dev_capture_note` confirms ~100 KB delta
  between WSL2 and GHA observed in prior captures.

**My cross-check**:
- The pre-opt measurement basis is explicit in
  `tools/wasm-size-check.sh` header (lines 27-37); the diagnosis
  correctly identifies that the gate measures pre-opt.
- No claim that the user's 1.6 MB number is wrong — only that it
  doesn't reproduce on this host. The diagnosis honestly cannot
  reconcile it without the GHA capture.

**Verdict: ESCALATED.** This is a clean escalation, not a fudge.
The diagnosis pins both measurement bases (pre-opt and post-opt),
notes the gap, and explicitly routes the GHA-side reconciliation
to candidate OTHER-2 with a one-line plan ("Runs the existing
`scripts/capture-baselines.sh` on GHA. No code change."). PM
contract D-4 explicitly authorized "pin a measurement basis" + a
follow-up gate. The escalation matches D-4's intent.

## Implementer's specific claims — cross-check

| Claim | Evidence cited | Holds up? |
|-------|----------------|-----------|
| `marque_capco` native +182 KiB / +84% (217→399 KiB) | cargo-bloat-top20.md by-crate table row 2 | ✅ Direct readout |
| `CapcoMarking::join_via_lattice` 57.9 KiB dominant single function | cargo-bloat-top20.md row 2 in by-function table | ✅ Direct readout |
| Replaces pre-pr4 `PageContext::project` 44.2 KiB | cargo-bloat-top20.md pre-pr4 spot-check row 2 | ✅ Verified — PageContext was in marque_ism pre-pr4 (`git ls-tree 18cef6c9 -- 'crates/ism/src/page_context.rs'` returns a blob); deleted in PR 6c |
| 5 quicksort monomorphizations sum ~77 KiB | rows 8-12 of bloat-doc table, 15.6+15.6+15.6+15.6+15.5 | ✅ Sum is 77.9 KiB |
| Pre-pr4 had 2 quicksort monos at 22 KiB | pre-pr4 spot-check rows for `evaluate_sar_banner_rollup` (12.2) + `render_structural` (9.7) = 21.9 KiB | ✅ |
| PR 4b-E + 4b-F + 6c recovered ~16% headroom from mid-flip | criterion-checkpoints.md: 1218.8 → 1022.8µs = -16.1% | ✅ |
| HEAD p50 1047µs, HEAD p99 1422µs | criterion-checkpoints.md row 3 | ✅ |
| User's ~1.7ms matches mid-flip p99 1691µs | criterion-checkpoints.md row 2 p99 column | ✅ Plausible match (1691 ≈ 1700) — alternative explanations (`Engine::fix` path heavier than lint, or different bench host) not surfaced |
| Pre-flip path 0 occurrences of `join_via_lattice`/`scheme.project` in engine.rs | implicit in narrative §2.1 | ✅ Verified via `git show c9d8ef29:crates/engine/src/engine.rs \| grep -c` — returns 0 |
| Post-flip path 5 occurrences | implicit in narrative §2.1 | ✅ Verified — returns 5 |
| EXECUTE total 325µs lower / 440µs upper | §5 "Notes on the table" paragraph | ❌ Row sums are ~105µs lower / ~290µs upper; the 325/440 numbers don't reconcile |

## Cause-to-fix linkage (EXECUTE candidates)

| Candidate | Linkage |
|-----------|---------|
| **LA-1** (consolidate 5 quicksort monos) | Strong. Narrative §2.1 + §4 + cargo-bloat-top20 rows 8-12 identify the 5 quicksort monos by name; candidate proposes the consolidation. ✅ |
| **LA-2** (skip empty-axis lattice projections) | Strong. Narrative §2.2 phase_g_n1 +38.9% identifies the fixed per-call cost growth; candidate proposes the empty-axis short-circuit. profile_project evidence cited inline. ✅ |
| **LA-3** (single-portion fast path on `join_via_lattice`) | Strong. profile_project phase_a 478ns + phase_g_project_n1 704ns at HEAD are cited; candidate proposes the identity case. The correctness argument (`join(x) = x` for any x in a join-semilattice) is sound. ✅ |
| **MO-1** (de-monomorphize `Engine::with_clock`) | **Weaker**. Bloat-doc shows `Engine::with_clock::<CapcoScheme>` +6.2 KiB delta. Candidate claims 30-40 KB WASM savings; this number doesn't trace cleanly to the +6.2 KiB regression delta. The candidate is defensible as a generic optimization but doesn't directly attack the pre-pr4 → head regression. The narrative would be tighter if it said "this is an opportunistic win, not a regression-driver fix". ⚠ |
| **MO-2** (devirtualize `Arc<dyn Recognizer/Vocabulary>`) | Adequate. Narrative cites baseline.json `_p99_note` for the p99 vtable miss attribution. Note: the `Arc<dyn Vocabulary>` dispatch landed in PR 3d / Phase 5, **pre** the pre-pr4 → head delta. The diagnosis correctly frames this as opportunistic, not regression-driver. ✅ |
| **CO-1** (synthesize_fixes enum) | Adequate. Narrative cites the `<TwoPassFixer>::run` 39.3 KiB bloat. Caveat: this is the **fix path**, not lint hot path; the candidate's row says so explicitly. ✅ but tangential to the lint_10kb regression. |

No EXECUTE candidate is unsupported by the narrative. MO-1 is the
weakest linkage and would benefit from R3's tightening.

## Falsification discipline

**Hypotheses tested and rejected:**

1. **User-reported ~1.7ms = HEAD mean.** Rejected: HEAD mean
   is 1023µs; mid-flip p99 1691µs is the matching reading.
   (criterion-checkpoints.md row 2.)
2. **User-reported +400 KB WASM growth.** Rejected: measured +94
   KB pre-opt / +90 KB post-opt. (twiggy-monos-top20.md /
   diagnosis §1.)
3. **PR 4b-E was expected to close the gap (per
   baseline.json::_note).** Rejected: recovery was 16%, not the
   100% the `_note` implied. (criterion-checkpoints.md +
   diagnosis §2.2.)
4. **PR 4b-B/C attributions were direct (PR #498 commit message).**
   Rejected and re-framed: 4b-B/C added source code, 4b-D.2 added
   reachability. (diagnosis §2.1 + my own merge-history check.)

**Hypotheses confirmed:**

1. PR 4b-D.2 is the load-bearing structural cost center.
2. The post-flip path is structurally more expensive than the
   pre-flip path (~50% more O(n) walks per call).
3. PageContext retirement (PR 4b-E + 4b-F + 6c) did improve
   per-portion lattice setup (phase_i_join_n10/n25 -40%).
4. The 5 quicksort monomorphizations are new at HEAD.

**Apparent confirmation bias**: Minimal but present. The diagnosis
doesn't seriously entertain "the user is measuring on a different
host" as an alternative to "the user is remembering mid-flip p99"
for the ~1.7ms reading — both are equally plausible without further
data. This is a soft bias toward the more flattering interpretation
(the cumulative regression is "only" +80%, not "+140% as the user
reported"). I'd want the diagnosis to surface both possibilities and
let the GHA re-capture (OTHER-2) discriminate. Severity LOW —
doesn't affect remediation strategy.

## Plausible drivers NOT in the narrative

Per the dispatch's adjacency-check list:

- **`marque_engine` binary growth attribution.** The bloat-doc by-crate
  table row 3 shows marque_engine +19.6 KiB / +8.2% across the
  umbrella. The diagnosis treats this as background; doesn't decompose
  it. Some of that may be lattice-trait-impl monomorphization that
  pulled into the engine, some may be the `Engine::with_clock` mono
  growth. Acceptable to leave at this granularity given the
  marque_capco growth dominates by 10×. **NOT a hole.**
- **`marque_scheme` growth.** Not in the by-crate top-20 (consistent
  with trait-heavy crates where most code monomorphizes into
  consumers). The implementer's report doesn't address this; it's
  the correct null behavior. **NOT a hole.**
- **`Arc<dyn Recognizer<CapcoScheme>>` dispatch from PR 3a-3c.**
  Covered by MO-2 with explicit caveat that this is pre-umbrella
  (the `Arc<dyn>` dispatch landed in PR 3a/3c, not in 4b). The
  diagnosis correctly treats it as opportunistic, not a regression
  driver. **NOT a hole.**
- **`Vocabulary<S>` trait indirection from PR 3d / Phase 5.** Also
  covered by MO-2. **NOT a hole.**
- **`wasm-opt` measurement-basis as candidate.** Discussed in
  diagnosis §2.3 as one of three alternative explanations, plus
  W-MO-1 as the candidate to build a separate WASM artifact for
  monos audit. **NOT a hole.**
- **Closure-operator per-iteration allocation pattern.** Flagged as
  HOT-2 INVESTIGATE candidate with explicit "+270% absolute is
  suspicious" — could be allocator-driven, not catalog-driven. The
  diagnosis is appropriately cautious about confirmation. **NOT a
  hole.**

The narrative is reasonably comprehensive given the constraint that
flamegraph capture failed (cargo-flamegraph / samply / kernel-perf
unavailable on WSL2 sandbox without sudo). OTHER-1 documents the gap
and the unblock path.

## What the implementer got RIGHT

1. **Honest about the synthesized flamegraph.** The
   lint-flamegraph-top15.md file explicitly labels itself
   "synthesized from criterion + bloat" — no pretense that real
   `perf` data was captured. Inclusive-% ranges are given as
   bands (e.g., "~6-10%") not point estimates. Confidence levels
   are downgraded one tier (HIGH → MED) per the gap. This is
   exactly the discipline a root-cause investigator wants to see.
2. **Refused to over-conclude on the user's ~1.6 MB / ~1.7ms
   memories.** Stated that those numbers DON'T reproduce on this
   host without claiming they're wrong. Routed reconciliation to
   GHA re-capture (OTHER-2). PM contract D-4 explicitly authorized
   this.
3. **Zero `crates/*/src/**` edits.** Verified via my own `git
   diff origin/staging -- 'crates/*/src/**' | wc -l` → 0.
   Constitution VII §IV scheme-adoption boundary observed.
4. **Caveat discipline on cause-to-fix linkages.** CO-1 explicitly
   labels itself "fix path; not lint hot path". MO-2 explicitly
   labels itself "10-30µs (p99 only)". The diagnosis doesn't paper
   over the fact that some EXECUTE candidates aren't regression
   drivers per se — they're opportunistic wins identified while
   looking at the bloat numbers.
5. **Constitution VIII trivially satisfied.** No `§X.Y pNN`
   citations in the remediation plan because no candidate alters
   grammar behavior. The implementer's claim "(no `§X.Y pNN`
   citations appear in this table because no candidate alters
   grammar behavior)" was verified against each EXECUTE candidate
   — LA-1 cites the existing canonical-order semantics defined in
   `sar_sort_key`; LA-2 / LA-3 invoke lattice identity properties;
   MO-1 / MO-2 / CO-1 are pure dispatch refactors. None touch
   CAPCO grammar.
6. **Both bloat and criterion data committed as text-only.** No
   binary blobs, no SVGs. PM contract D-6 satisfied; 68 KB total
   committed (under 100 KB budget per my own `du -sh
   docs/perf/`).
7. **PR template lands.** D-5 deliverable: ~62 lines, fillable
   bench-delta block. Verified existing at `.github/PULL_REQUEST_TEMPLATE.md`.

## Provenance

Reviewer 3 of 3. Authored 2026-05-19 against branch
`refactor-006-pr-4b-perf-closeout` off `origin/staging` @ `81694384`.
Verification commands run from worktree root; merge-history
cross-checks via `git log --oneline 18cef6c9..81694384 -- ...`
and `git show <sha>:<path>`. Hardware not load-bearing for this
review (reviewing text artifacts and merge history only; no
bench re-captures).
