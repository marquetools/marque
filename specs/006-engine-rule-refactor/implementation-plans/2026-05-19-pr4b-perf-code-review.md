<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-perf closeout — Code-reviewer review (R2)

**Reviewer:** R2 (overall quality / scope / infrastructure / citations)
**Date:** 2026-05-19
**Artifacts reviewed:** diagnosis doc (358 lines), 4 supporting artifacts, PM contract,
implementation report, `.github/PULL_REQUEST_TEMPLATE.md`, `.github/workflows/ci.yml` diff,
`tools/wasm-size-check.sh` diff. All reads performed on branch
`refactor-006-pr-4b-perf-closeout` off `origin/staging`.

---

## Overall verdict

**APPROVE WITH FIXUPS**

One blocking issue (CI env-var scope mismatch) plus two recommended fixups. The
diagnostic artifacts are genuinely useful, the scope discipline is clean, and the
infrastructure additions are structurally sound — the blocking issue is a narrow
technical defect that invalidates the CI gate skip rather than the analysis.

---

## Required fixups (BLOCKING)

### [F1] CI env-var skip won't fire on pull_request events (blocking)

**File:** `.github/workflows/ci.yml`, lines 544 and 832
**Issue:**
Both `MARQUE_BENCH_SKIP_REGRESSION` and `MARQUE_WASM_SKIP_REGRESSION` env vars
use `github.ref` for the branch match:

```yaml
MARQUE_BENCH_SKIP_REGRESSION: ${{ github.ref == 'refs/heads/refactor-006-pr-4b-perf-closeout' && '1' || '' }}
```

On `pull_request` events (the typical CI run when this PR is submitted to GitHub),
`github.ref` is `refs/pull/N/merge` — not the source branch name. The expression
will always evaluate to `''`, and the drift gate will NOT be skipped. The entire
purpose of this CI edit — letting the diagnosis branch ship without flapping the
gate — fails silently.

The correct form mirrors the project's own established pattern (used at lines 84,
165-166, 248-249 of the same file):

```yaml
MARQUE_BENCH_SKIP_REGRESSION: ${{ (github.ref == 'refs/heads/refactor-006-pr-4b-perf-closeout' || github.head_ref == 'refactor-006-pr-4b-perf-closeout') && '1' || '' }}
```

Apply the same dual-check to `MARQUE_WASM_SKIP_REGRESSION`. The PM contract (D-3)
explicitly requires an exact branch match; adding `github.head_ref` doesn't
compromise that — it makes the same exact match work for the PR-event context.

**Fix:** Add `|| github.head_ref == 'refactor-006-pr-4b-perf-closeout'` to both
env-var expressions. Two-line change in `.github/workflows/ci.yml`.

---

## Recommended fixups (NICE-TO-HAVE)

### [R1] Implementation report misstates EXECUTE/INVESTIGATE counts

**File:** `docs/plans/2026-05-19-pr4b-perf-implementation-report.md`, lines 77 and 83
**Issue:**
The implementation report's PM-contract-conformance table contains two inconsistent
counts of the same table:

- D-2 row: "10 EXECUTE, 7 INVESTIGATE incl. OTHER infra candidates"
- D-8 row: "7 EXECUTE-tier with score ≥ 6"
- Actual diagnosis table: **6 EXECUTE, 10 INVESTIGATE** (17 rows total)

Both stated counts are wrong. A reviewer relying on the implementation report as a
summary document would walk away with an incorrect picture of the tier distribution.
This doesn't affect the primary diagnosis doc (which is accurate), but it degrades
the implementation report's value as a one-page summary.

**Fix:** Correct D-2 row to "6 EXECUTE, 10 INVESTIGATE" and D-8 row to "6 EXECUTE-tier
all with score ≥ 6". One edit, low effort.

### [R2] CAPCO §-citation in cargo-bloat-top20.md lacks page numbers

**File:** `docs/perf/2026-05-19-diagnosis/cargo-bloat-top20.md`, bottom of the
by-function table's note
**Issue:**
The note reads: "each per-axis lattice's `to_marking` / `to_markings` projection
sorts canonically (per CAPCO §H.4 / §H.5 / §H.8 ordering rules)."

Constitution VIII requires `§X.Y pNN` form (section + page). The cited sections
exist and the claim is correct (§H.4 p61, §H.5 p99-100, §H.8 p136 for the
respective canonical ordering rules), but without page numbers the citation isn't
re-verifiable per the stated standard. This is a supporting artifact rather than
a rule body or plan, so the blast radius is low — but the project standard applies.

**Fix:** Update to `per CAPCO §H.4 p61 / §H.5 pp99-100 / §H.8 p136 ordering rules`.

---

## PM contract conformance

| Decision | Status | Notes |
|----------|--------|-------|
| D-1 | PASS | Single diagnosis PR. Zero `crates/*/src/**` edits confirmed (`git diff origin/staging -- 'crates/*/src/**' \| wc -l` = 0). Multi-PR remediation lane established. |
| D-2 | PASS with note | All 6 required subsections present (§0–§6). 358-line main doc under 800-line cap. 17 rows under 25-row cap. All per-candidate fields populated. See [R1] for implementation-report count mismatch. |
| D-3 | FAIL (F1) | `tools/wasm-size-check.sh` script edit is correct and well-placed. CI env-var injection uses `github.ref` only — will not fire on `pull_request` events. See [F1]. |
| D-4 | PASS | Both pre-opt and post-opt WASM sizes captured at both checkpoints. `twiggy-monos-top20.md` §"WASM byte-size summary" documents both bases clearly. The user's ~1.6 MB figure is addressed with three candidate explanations and a clear recovery path (OTHER-2). |
| D-5 | PASS | `.github/PULL_REQUEST_TEMPLATE.md` (62 lines) lands in this PR. Cumulative regression alert and WASM-monos CI annotation correctly deferred. |
| D-6 | PASS | `docs/perf/` total = 49 KB (files) / ~68 KB on disk — under 100 KB cap. All artifacts text-only; no binary blobs. |
| D-7 | PASS | Three reference points (pre-pr4, mid-flip, head), same WSL2 host, single calendar day. Mid-flip captured unconditionally — deviation is justified (cumulative delta is >1.5× so conditional fires; and without mid-flip the contradiction 2 resolution would be weaker). Deviation documented transparently. |
| D-8 | PASS with note | EXECUTE/INVESTIGATE tier semantics applied. HOT-1 (score 45.0, MED risk) is placed in INVESTIGATE rather than EXECUTE because flamegraph confirmation is absent — a defensible call given the evidence-required gate. See [R1] for count discrepancy in the implementation report. |
| D-9 | PASS | `scripts/bench-check.sh` unchanged (verified via diff). |
| D-10 | DEFERRED (expected) | PR body not yet authored. Implementation report previews required content (headline acknowledgment, pointers, CI failure list, attestation paragraph). Correct per the contract. |

---

## Document quality assessment

**Readability cold-read:** A reviewer unfamiliar with marque can follow the structure.
The TL;DR in §0 is effective. The three-contradiction structure of §2 is the
right organizing principle for an attribution narrative — each contradiction gets
its own heading, a resolution, and a confidence label.

**D-2 structure completeness:** All five required subsections are present:
- §1 = reference range ✓
- §2 = attribution narrative ✓
- §3 = hot-path map ✓
- §4 = WASM attribution ✓
- §5 = ranked remediation table ✓
- §6 = open questions ✓

**Jargon legibility:** "Kleene-fixpoint walk" appears without a definition in §3
and in the criterion-checkpoints doc. For a cold reader not already familiar with
the closure-operator architecture, this term would require a side-trip to
`docs/plans/2026-05-01-lattice-design.md`. A one-sentence gloss ("each page
triggers a fixed-point iteration over the closure-rule catalog until no new
markings are added") would make §3 more self-contained. This is below the
threshold for a fixup note given the diagnosis doc references companion
documents explicitly.

**Synthesized flamegraph:** The `lint-flamegraph-top15.md` file is honest and
well-labeled about being synthesized. The methodology section's formula is
explicit. The "What the synthesis cannot resolve" section manages expectations
correctly. No issue here — the implementer handled the tooling gap professionally.

**Score formula:** The scoring formula in §5's preamble
(`score = (savings_midpoint_us × confidence_pct) / risk_multiplier`) is
documented inline, which is correct for a self-contained document. Readers
can replicate the scores.

**Provenance section (§8):** Correctly states no corpus document text appears
in artifacts, cites the bench input helper, and notes raw criterion data was
deleted before commit. Satisfies Constitution V Principle V G13.

---

## Citation discipline

**Remediation table:** Correct. The diagnosis doc §5 notes section explicitly
states "no `§X.Y pNN` citations appear in this table because no candidate alters
grammar behavior." This is accurate for all 17 rows — no candidate is grammar-
touching, so the Constitution VIII citation discipline is trivially satisfied at
the table level.

**`cargo-bloat-top20.md` §H reference:** Three bare `§H.X` section references
without page numbers appear in the note after the by-function table. See [R2].

**`criterion-checkpoints.md` §4.7 reference:** The reference "#529 §4.7 Trio 1"
is a PR plan section reference, not a CAPCO-2016 citation. The notation could
be confused with CAPCO-style citations, but it is clearly in the context of a
list of PR numbers ("PRs that added closure-rule rows between mid-flip and head").
Not a Constitution VIII violation; the reference is to a plan document, not the
CAPCO manual.

**PR template Changes section:** Correctly instructs authors to use `§X.Y pNN`
form per Constitution VIII. Appropriate guidance for the template context.

---

## Infrastructure edits

### `tools/wasm-size-check.sh`

The placement is correct: after baseline-file read (line 132), before the
comparison logic. The comment block clearly explains what is and isn't skipped
(only the drift gate; build failures and artifact-not-produced still fail).
The pattern parallels `scripts/bench-check.sh`'s existing `MARQUE_BENCH_SKIP_REGRESSION`
override, which is the right consistency anchor. The script syntax verified
clean (`bash -n` per implementation report).

One observation: the skip block computes `DELTA` and prints it even on the skip
path — this is correct behavior, giving the CI log a visible size delta for the
branch even though the gate is skipped.

### `.github/workflows/ci.yml`

**Comment quality:** Both comment blocks accurately describe what is still enforced
and what is skipped, reference the PM contract by section (D-3), and include
explicit REMOVE instructions. This is the right template for a temporary CI
override.

**Placement:** The env var is set at the correct scope level — per-step `env:`
rather than job-level `env:` — which is correct. A job-level env would apply to
all steps including unrelated ones; per-step scoping is tighter.

**Branch match — see [F1]:** The `github.ref` expression is the critical gap.

**Adjacent CI consideration:** The `pr-4b-corpus-regression` job (lines 244-249)
matches `startsWith(github.ref, 'refs/heads/refactor-006-pr-4b')` and
`startsWith(github.head_ref, 'refactor-006-pr-4b')`. This branch name matches
the prefix, so the corpus-regression job WILL fire on this branch. This is expected
and correct — the diagnosis PR does not change any rule logic, so the corpus suite
should pass cleanly. No additional CI disposition needed for this job.

No other gates (codecov, citation lint, doc check) need branch-specific
disposition — they are unaffected by a docs-and-CI-only PR.

### `.github/PULL_REQUEST_TEMPLATE.md`

**Structure:** Standard four sections (Summary, Motivation, Changes, Testing) +
the Hot-path perf delta block. 62 lines — under 100-line limit.

**Scope trigger:** The "engine-touching PRs only" scoping is explicit, with a
specific list of path prefixes. The "Delete this entire section if the PR does
NOT touch the listed paths" instruction is present and clear.

**Bot compatibility:** No existing `.github/ISSUE_TEMPLATE/` directory exists
on this branch; the PR template is the only template file in `.github/`. No
Dependabot, Copilot, or other bot configuration files reference PR template
format — the template is purely markdown consumed by the GitHub PR creation UI
and is bot-agnostic.

**Gap considered:** The template has no automated CI gate enforcing the bench-delta
block — per PM contract D-5, enforcement is social (review checklist), not
automated. The template's comment block makes this clear. This is the correct
scope choice for this PR; a CI gate on PR description content was explicitly
rejected in the PM contract ("a hard CI gate on PR-description content is more
Goodhart than signal").

---

## Commit hygiene

Both commits are GPG-signed with Adam Poulemanos's EdDSA key (`95F2033D...`),
status "Good signature" from the key marked [ultimate]. No `--no-gpg-sign`
bypass. Commit messages follow the project's `type(scope): description` convention:

- `docs(perf): PR 4b-perf closeout — diagnosis + remediation roadmap`
- `ci(perf): env-var drift-gate skip + PR template for PR 4b-perf closeout`

Separation of the docs commit and the CI/infrastructure commit is correct practice.
No force-push evidence in branch history.

---

## Adjacent-path checks

**PR template vs. existing templates:** No `.github/ISSUE_TEMPLATE/` directory
exists. No other PR templates exist. The new file is the sole template and will
apply to all PR types — the "Delete this section if it does not apply" instruction
in the bench-delta block is the correct mitigation.

**PR template vs. bot tooling:** Dependabot PRs use the standard Dependabot
PR body (not a template); the template won't interfere. The agent
prompts/configs under `.github/agents/` and `.github/prompts/` are Copilot
Workspace configs — these are triggered by Copilot's own UI, not by PR template
content. No interaction concern.

**CI gate skip and other jobs:** The `pr-4b-corpus-regression` job fires on this
branch (it matches `startsWith(..., 'refactor-006-pr-4b')`) and has no skip
mechanism — appropriately so, since this PR adds no rule logic. Citation lint,
doc check, clippy, and test jobs all run normally. Only `bench-check` and
`wasm-size-check` have the skip mechanism, which is the correct, minimal scope.

**`MARQUE_WASM_SKIP_REGRESSION` env-var vs. `MARQUE_BENCH_SKIP_REGRESSION`
consistency:** The wasm-size-check.sh script's `MARQUE_WASM_SKIP_REGRESSION`
override mirrors the existing bench-check.sh's `MARQUE_BENCH_SKIP_REGRESSION`
pattern correctly. The comment block in both places is symmetric.

---

## What the implementer got RIGHT

1. **Scope discipline is exemplary.** Zero `crates/*/src/**` edits. The diff is
   bounded exactly to the PM contract's specified paths. No creep.

2. **Contradiction-resolution structure.** Using three named contradictions as the
   organizing principle of §2 makes the attribution narrative navigable and
   auditable. Each contradiction has a resolution and a confidence label.

3. **Honest about measurement gaps.** The synthesized flamegraph is labeled as
   synthesized throughout. The twiggy monos gap is documented in its own
   dedicated artifact (`twiggy-monos-top20.md`). Confidence reductions are
   applied consistently. No cargo-culting of uncertain numbers as definitive.

4. **The wasm-size-check.sh skip is correctly placed.** The override is added
   after the artifact check and before the comparison, keeping build failures
   hard. The delta is still computed and printed. This is precisely what the PM
   contract required.

5. **Per-stage `profile_project` data.** The criterion-checkpoints.md's
   phase_a/b/c/d/e/f/g/h/i per-stage breakdown makes the remediation table's
   evidence column traceable to actual measurements rather than narrative. The
   +270% closure-floor finding and the -40% join improvement are specific,
   quantified, and reproducible from the documented bench commands.

6. **GPG commit signing maintained.** Both commits are properly signed per
   project requirements.

