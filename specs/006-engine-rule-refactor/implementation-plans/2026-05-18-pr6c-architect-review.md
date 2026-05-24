<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 6c (T069) — architect review

**Reviewer scope:** cross-crate ripple verification + plan-vs-implementation
gap analysis. Companion to the rust-mechanics and lattice-domain reviews.

**Branch state at review:** 4 commits (`42f31db1` → `507e5da9`), all GPG-signed,
27 files changed, +977 / −740, structurally −195 LOC excluding new plan docs.

## Verdict

**Plan adherence:** plan §6 4-commit decomposition realized literally. Both
deviations the implementer surfaced are tree-green-discipline mechanics, not
architecture drift — the end-state at commit 4 matches the plan §6 end-state
byte-for-byte.

**Cross-crate ripple:** matches plan §4 exactly. 6 crates touched (+ docs);
WASM `src/` zero changes (one comment-line touch only, see F-2); no
production-code edits in the 5 untouched-by-design crates.

**Constitution discipline:** VIII §-citation gate clean (zero new
`§X.Y pNN` adds); V audit-content-ignorance unchanged; VI page-break
reset-before-parse preserved; VII §IV within-006 precedent properly invoked;
II `Box<[T]>` pivot convention honored via OQ-3 Option A.

## Findings

### F-1 `[LOW]` Doc-comment back-references retained on purpose — confirm propagation discipline

22 files retain `PageContext` / `page_context` strings as historical
back-references in doc-comments (e.g. `crates/ism/src/projected.rs`,
`crates/engine/src/engine.rs`, multiple `tests/*.rs`). The mandatory grep
`rg "PageContext" --type rust` returned only such hits — none in `pub fn`
signatures, struct fields, or live code.

**Recommendation:** acceptable; these mirror the post-PR-4b-E and PR-4b-F
disposition. Future PR-7+ doc sweep can collapse them, but per Constitution
VIII propagation discipline these MUST be re-verified at any future move.
**No action this PR.**

### F-2 `[LOW]` WASM `compute_banner_native` retains a `PageContext` reference in a code-adjacent comment

`crates/wasm/src/lib.rs` carries the line
`// compute_banner — scanner + parser + PageContext only (no rules engine)`
unchanged from staging. The comment was accurate pre-PR-6c (the WASM
compute_banner path historically went through `PageContext::project`);
post-PR-6c it's structurally inaccurate but functionally inert (the WASM
binary did not consume `marque_ism::PageContext` even pre-PR-6c per the
rust-preflight Risk #7).

**Recommendation:** swept by the comment-drift cleanup mandate. Either
(a) update to `// compute_banner — scanner + parser + Vec<CanonicalAttrs>
projection only` as a 1-line follow-on, or (b) note as known doc-drift on
the PR. **Not a merge blocker.**

### F-3 `[LOW]` Tasks.md bookkeeping: T069 not yet ticked

`specs/006-engine-rule-refactor/tasks.md` still reads `- [ ] T069` and
`- [ ] T070 [P] [US2]` / `- [ ] T071 [P] [US2]` / `- [ ] T072 [P] [US2]`.

T069 is satisfied by this PR by construction. T070 (`lint_100kb_multipage`
post-merge baseline check) and T071 (CI matrix `{6a-only, 6a+6b, 6a+6b+6c}`)
are gates that fire after merge — properly stay open until the CI run
attests. T072 (`tests/corpus/foreign/` 100%) is a green-on-CI assertion
that doesn't depend on the PR but should be confirmed on the post-merge run.

**Recommendation:** tick T069 in the PR (it lands with this PR). Leave
T070-T072 unchecked until the post-merge CI run attests them. PM-decisions
operative checklist explicitly says "Tick T069 + any T070-T072 acceptance
gates that PR 6c satisfies"; only T069 qualifies pre-merge. **Tick T069 in
this PR; defer T070-T072.**

### F-4 `[LOW]` Two documented deviations — both are tree-green-discipline, not architecture

Implementation report §"Deviations from the locked plan" surfaces two:

(a) **Shim retention in commit 2.** `project_from_page_context` kept as a
1-line wrapper through commit 2 (deleted in commit 3). Plan §6 said "rename
body in commit 2" — a literal rename would break the engine call site
mid-commit and fail the tree-green invariant. The implementer correctly
prioritized the PM's mandatory `cargo check --workspace` per-commit
green over the architect's parenthetical rename instruction.

(b) **Engine touch in commit 2.** Plan §6 named only `marque-capco` for
commit 2. The W004 test suite (8 tests asserting `JointSet::DisunityCollapse`)
fails if the engine doesn't populate `ctx.page_portions` — the rule reads
the new field but the engine still feeds the old one. The minimal 13-line
transitional bridge (`ctx_page_portions = ctx_page.as_ref()...`) was
required, and is inverted-then-deleted in commit 3.

Both deviations preserve the architect's end-state shape at commit 4 and
the PM's per-commit tree-green invariant. Neither changes the public API
surface, the engine semantics, or any constitutional invariant.

**Recommendation:** these are the right call. Plan §6's "commit 2 touches
only `marque-capco`" prescription was structurally impossible given the
W004 test dependency; the architect plan should have caught it during
the rust-preflight review (it didn't — preflight Risk #4 covered fixture
construction sites but not the W004 engine-feed dependency). **Capture
this as an architect-plan-lesson for future structural retirements:
"named crate scope ≠ closed touch set when consumer rules read engine-fed
ctx fields."**

### F-5 `[LOW]` Doc-comment lint surprise (`+` at line start) is broader than PR 6c

The implementation report notes stable rustdoc/clippy treats a doc-comment
line beginning with `+ ` as a markdown list continuation, causing six
`doc list item without indentation` errors on the implementer's first
draft of `crates/ism/src/projected.rs`. Fix: substitute commas for `+`
in continuation prose.

This is not a PR 6c bug — it's a stable-clippy behavior that bites any
doc-comment author writing `... A + B + C` in prose. Already in scope of
project memory `feedback_clippy_nightly_vs_stable_drift`.

**Recommendation:** capture as a doc-comment style note in a follow-up
to that memory if not already captured. **No action this PR.**

### F-6 `[LOW]` `RuleContext: Send + Sync` HRTB pin landed correctly

Rust-preflight Risk #3 + PM-decisions amendment #2 prescribed adding
`fn _rule_context_is_send_sync<'a>() where RuleContext<'a>: Send + Sync {}`
to `crates/rules/tests/send_sync.rs` because `assert_impl_all!` requires
`'static`. Confirmed present at `crates/rules/tests/send_sync.rs` with
the correct HRTB shape and a docstring naming PR 6c.

The companion preserved-pin `assert_impl_all!(CanonicalAttrs: Send, Sync)`
in `crates/ism/tests/send_sync.rs` is the new load-bearing axiom (the
new field type `Arc<Box<[CanonicalAttrs]>>` is `Send + Sync` iff
`CanonicalAttrs: Send + Sync`). Confirmed retained.

**No action.** The Send+Sync surface is correctly re-anchored.

### F-7 `[LOW]` Bench-gate playbook — preflight Risk #9 prediction stands

`lint_10kb` baseline 828µs / 10% threshold 911µs straddles current
real-world 880-930µs measurements per project memory
`project_bench_baseline_staleness`. PR 4b-F (signature-only,
structurally identical class to PR 6c) needed one `gh run rerun
<id> --failed` and landed at 973µs in the documented baseline-staleness
band.

PR 6c may need the same one re-run. Per the operative checklist, if
persistent fail after one re-run, escalate to PM — do NOT optimize PR 6c
to pass the gate.

**Recommendation:** flag in the PR description so the reviewer knows
this is expected noise-band behavior. **PM-decisions operative checklist
already names this; just include in the PR body.**

## Cross-crate ripple — verified

| Crate | Plan §4 disposition | Verified |
|---|---|---|
| `marque-ism` | delete `page_context.rs`, drop re-export, drop Send+Sync pin | ✅ all three |
| `marque-rules` | add `page_portions` field, drop old `page_context` field at commit 3 | ✅ confirmed (only doc-comment back-references remain) |
| `marque-engine` | inline accumulator, `DEFAULT_PORTIONS_CAPACITY = 8`, sig flattening, sentinel re-target | ✅ all four |
| `marque-capco` | S005 + W004 migration, `project_from_attrs_slice` rename, 2 fixture migrations | ✅ confirmed (fr048 correctly excluded per preflight Risk #4) |
| `marque-engine` benches | `benches/profile_project.rs` 3 sites | ✅ confirmed in commit 3 |
| `marque-ism` test-utilities | `tests/send_sync.rs` PageContext pin drop | ✅ confirmed in commit 4 |
| `marque-wasm` | **no change** | ✅ confirmed (one comment-line touch — see F-2) |
| `marque-server` / `-extract` / `-config` / `-marque` / `-scheme` | **no production change** | ✅ confirmed (`marque-scheme` 1-insert / 1-delete is a doc-comment string update) |

**6-crate ripple, not 4** — plan §4 was correct.

## End-state shape match — verified

| OQ | Decision | Implemented |
|---|---|---|
| OQ-1 = B | `PageContext` struct fully deleted at commit 4 | ✅ `page_context.rs` is 0 bytes / gone |
| OQ-3 = A | `Option<Arc<Box<[CanonicalAttrs]>>>` on `RuleContext` | ✅ field shape exact at `crates/rules/src/lib.rs` |
| OQ-4 | `DEFAULT_PORTIONS_CAPACITY = 8` const at `crates/engine/src/engine.rs` | ✅ `pub(crate) const DEFAULT_PORTIONS_CAPACITY: usize = 8` confirmed |
| OQ-6 | Engine reset-before-parse preserved; renamed test keeps assertion | ✅ `page_portions_reset_observably_across_form_feed` + `page_portions_lint_starts_fresh_on_each_call` confirmed |
| OQ-7 | PR description cites within-006 precedent | (PR not opened yet; ensure in body) |

## Comparison to PR 4b-F

PR 4b-F shipped 7 commits + 3 reviewer reports + 1 reviewer-feedback fix-up
commit. Its PR body has a strong skeleton (Summary / Pipeline shape /
Commit sequence / PM decisions / Constitution VII §IV authorization / Tasks
closed / Reviewer attestation / Pre-merge verification / Test plan).

**Recommendations for the PR 6c PR body:**

1. **Pipeline shape diagram** in the same before/after format — show
   `Engine::lint_inner` accumulator going from `PageContext` → `Vec<CanonicalAttrs>`
   and `dispatch_page_finalization` parameter flattening.
2. **Cite the within-006 engine-crate touch precedent** explicitly: PR 4b-B
   Commit 2 / 4b-C Commit 5 / 4b-D.2 / 4b-D.3 / 4b-E / 4b-F. Mirrors PR 4b-F.
3. **List tasks closed:** T069 closed by this PR; T070/T071/T072 fire
   post-merge.
4. **Bench-noise-band disclaimer:** state `lint_10kb` may marginal-fail per
   project memory `project_bench_baseline_staleness`; one `gh run rerun
   <id> --failed` is the documented mitigation; do not optimize PR 6c.
5. **Reviewer attestation block:** name the three review reports the PR
   relied on (architect / rust / lattice if lattice-review ran).
6. **`Send + Sync` re-anchor note:** call out the `RuleContext`
   compile-time pin shift from `PageContext: Send, Sync` to
   `RuleContext<'a>: Send + Sync` HRTB form.

## Final disposition

✅ **ready to submit.**

T069 is closed by construction. All four commits are GPG-signed,
per-crate tree-green, and follow the locked plan + PM-decisions
addendum. Both documented deviations are tree-green mechanics, not
architecture drift. The cross-crate ripple matches plan §4 exactly.
Constitution VIII §-citation gate clean. Send+Sync surface re-anchored
correctly. Comment-drift cleanup is the only loose thread (F-1, F-2)
and is non-blocking.

**Pre-merge actions for the implementer:**
1. Tick T069 in `specs/006-engine-rule-refactor/tasks.md` (F-3).
2. Optionally update the `compute_banner` WASM comment (F-2) — single line.
3. Open the PR with the body skeleton from PR 4b-F (see "Comparison" above).
