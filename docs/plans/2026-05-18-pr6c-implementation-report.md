<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 6c (T069) — implementation report

**Date:** 2026-05-18
**Branch:** `refactor-006-pr-6c-pagecontext-struct-retirement`
**Base:** `origin/staging` @ `480fccf8` (merge-base; staging has since
advanced to `840700c3` with PR #544, which does NOT touch the PageContext
surface).

> **Pre-fixup snapshot.** This report captures the state at the end of
> commit 4 (`507e5da9`), immediately after the implementer's local
> verification chain completed. Commits landed after this report was
> written are NOT reflected in the file/LOC accounting, commit list,
> or GPG attestation below — they are documented separately in their
> own commit messages and in the PR description body. As of PR
> submission the branch carries:
>
> - `42f31db1` — commit 1 (`marque-rules`)
> - `16124a3c` — commit 2 (`marque-capco` + minimal engine bridge)
> - `eda09358` — commit 3 (`marque-engine` accumulator inline)
> - `507e5da9` — commit 4 (`marque-ism` deletion + Send+Sync re-pin)
> - `89e6b96b` — commit 5 (reviewer fix-ups + tasks.md tick T069 +
>   4 reviewer reports committed under `docs/plans/`)
> - `2f35b875` — merge from `origin/staging` (PR #544 RELIDO closeout,
>   no PageContext surface touched)
> - `4b0fd889` — commit 6 (`cargo fmt`)
> - `89e6b96b` follow-up: Copilot suppressed-low-confidence review fix-ups
>
> All commits in the branch are GPG-signed. See the PR description
> for the consolidated test plan and the per-commit summary table
> that supersedes the §"Commit summary" table below.
**Companions:**
- Architect strategic plan: `docs/plans/2026-05-18-pr6c-pagecontext-retirement-plan.md`
- Rust risk register: `docs/plans/2026-05-18-pr6c-rust-preflight.md`
- PM-decisions addendum: `docs/plans/2026-05-18-pr6c-pm-decisions.md`

## Commit summary

| SHA | Title |
|---|---|
| `42f31db1` | refactor(rules): PR 6c commit 1 — add page_portions field on RuleContext |
| `16124a3c` | refactor(capco): PR 6c commit 2 — migrate S005 + W004 reads to page_portions |
| `eda09358` | refactor(engine): PR 6c commit 3 — inline accumulator, delete page_context field |
| `507e5da9` | refactor(ism): PR 6c commit 4 — delete page_context.rs + Send+Sync pin |

All four commits GPG-signed (`G` per `git log --pretty=format:"%G?"`).

## Final verification

- `cargo +stable clippy --workspace --all-targets -- -D warnings`: **CLEAN**.
- `cargo test --workspace`: **ALL PASS** (zero non-OK `test result:` lines across the workspace).
- Constitution VIII §-citation gate (`git log -p 42f31db1^..HEAD | rg '^\+.*§[A-Z]\.[0-9]+ p[0-9]+' | wc -l`): **0** new citations.
- All four commits GPG-signed.

## Files touched (categorized)

### Production code (15 files)

- `crates/ism/src/page_context.rs` — **deleted** (275 lines)
- `crates/ism/src/lib.rs` — `pub mod page_context` + `pub use PageContext` re-export dropped
- `crates/ism/src/{attrs,canonical,parsed,projected,sar_sort,span}.rs` — doc-comment drift cleanup
- `crates/rules/src/lib.rs` — added `page_portions` field + `with_page_portions` setter (commit 1); deleted `page_context` field + `with_page_context` setter (commit 3); doc-comment cleanup
- `crates/engine/src/engine.rs` — inline `Vec<CanonicalAttrs>` accumulator at `lint_inner`, `DEFAULT_PORTIONS_CAPACITY = 8` const, signature flattening on `dispatch_page_finalization` / `project_page_marking`, error-message update on `check_portions_unchanged`, two test renames (`page_context_resets_observably_across_form_feed` → `page_portions_reset_observably_across_form_feed`; `page_context_lint_starts_fresh_on_each_call` → `page_portions_lint_starts_fresh_on_each_call`)
- `crates/capco/src/rules.rs` — S005 (`analyze_uncertain_reduction`) + W004 (`JointDisunityCollapseRule::check`) migrated to `ctx.page_portions`
- `crates/capco/src/scheme/marking_scheme_impl.rs` — `project_from_page_context` renamed to `project_from_attrs_slice`; transitional shim retained for commit 2, deleted at commit 3
- `crates/capco/src/scheme/{marking,actions/strip}.rs` — doc-comment drift cleanup
- `crates/scheme/src/scheme.rs` — single doc-comment reference updated

### Tests (3 files)

- `crates/ism/tests/send_sync.rs` — dropped `assert_impl_all!(PageContext: Send, Sync)`; kept `CanonicalAttrs: Send, Sync` (load-bearing axiom)
- `crates/rules/tests/send_sync.rs` — **added** HRTB `fn _rule_context_is_send_sync<'a>() where RuleContext<'a>: Send + Sync {}` per rust-preflight Risk #3 / PM-decisions amendment #2
- `crates/capco/tests/rules_us1.rs` + `crates/capco/tests/s004_audit_content_ignorance.rs` — fixture migration from `PageContext::new()` / `add_portion` to inline `Vec<CanonicalAttrs>` + `Arc<Box<[CanonicalAttrs]>>` snapshot

### Benches (1 file)

- `crates/engine/benches/profile_project.rs` — 3 sites migrated (Phase E `engine_project_path`, Phase G scaling at `n ∈ {1, 5, 10, 25, 50}`, Phase H tmp_ctx rebuild); bench names preserved so historical `lint_10kb` / `phase_g_project_nN` baselines remain comparable

### Docs (5 files)

- `crates/{ism,rules,engine,capco}/README.md` — structural prose updates
- `docs/plans/2026-05-18-pr6c-pagecontext-retirement-plan.md` (new, 135 lines) — architect plan
- `docs/plans/2026-05-18-pr6c-pm-decisions.md` (new, 78 lines) — PM-decisions addendum
- `docs/plans/2026-05-18-pr6c-rust-preflight.md` (new, 219 lines) — rust risk register

## LOC delta

`git diff $(git merge-base origin/staging HEAD)..HEAD --shortstat`:

```
27 files changed, 977 insertions(+), 740 deletions(-)
```

Net `+237` LOC, but the structural picture is the opposite — 275 lines of
`page_context.rs` deleted + ~150 lines of `RuleContext.page_context` field
+ `with_page_context` setter deleted in the rules / engine touch. The `+`
side is dominated by the three new planning docs (432 lines combined) and
the PR 6c commit-message-block doc comments inside the touched source
files. Excluding the three planning docs, the structural delta is
`-195 LOC` (production code shrinks).

## Deviations from the locked plan

**Architect plan §6 commit 2 + PM-decisions operative checklist commit 2.** The
plan called for "rename `project_from_page_context` body to
`project_from_attrs_slice(&[CanonicalAttrs])`" in commit 2. A literal
rename would break the engine (the only caller, at this commit) and fail
the per-commit tree-green requirement.

Resolution: kept `project_from_page_context` as a thin one-line shim in
commit 2 delegating to the new `project_from_attrs_slice`. Engine call
sites continue to call the old name. Commit 3 migrates the engine to
`project_from_attrs_slice` and deletes the shim at the end of commit 3.

Net effect: identical to the planned end-state at commit 4; the path
through commits 2 and 3 stays tree-green per the PM's mandatory invariant.
Reviewed against the PM-decisions doc's "Each commit MUST land the tree
green" requirement and against the OQ-5 / OQ-3 framing.

**Architect plan §6 commit 2 engine touch.** The plan's commit 2 spec
named only `marque-capco`. But: `S005` and `W004` read `ctx.page_portions`
under PageFinalization dispatch, which is populated by
`dispatch_page_finalization` in `marque-engine`. Without engine
population, `cargo test -p marque-capco` for W004's test suite fails (8
W004 tests asserting `JointSet::DisunityCollapse` on per-page portions
get `None` and silently bail). Resolution: commit 2 also adds the
minimal engine bridge (derive `ctx_page_portions` from the same Arc'd
`PageContext` snapshot the existing `ctx_page` already drives). Commit 3
inverts: inline accumulator becomes the source; `PageContext`-derived
path is removed.

Both deviations preserve the architect's end-state shape and the PM's
per-commit tree-green invariant; both are documented in the relevant
commit messages.

## Surprises (things the preflight missed)

**Doc-comment lint failure on `+` at line start.** Stable rustdoc/clippy
treats a doc-comment line starting with `+ ` as a continuation of a
markdown list item. My initial edit to `projected.rs` rewrote a paragraph
as `Post-PR-4b-D.2 hot-path flip + PR 4b-E expected-accessor deletion + PR 6c PageContext retirement, ...` — the second `+ PR 6c` triggered six
`doc list item without indentation` errors. Fix: rewrite the paragraph
as `Post-PR-4b-D.2 hot-path flip, PR 4b-E expected-accessor deletion,
and PR 6c PageContext retirement, ...`. No additional code changes.

**Citation-gate false-positive on stale staging.** `git diff
staging..HEAD` reports §-citation additions that are actually deletions
*in the other direction* (staging-side commits not yet in the branch).
The accurate gate is `git log -p 42f31db1^..HEAD | rg '^\+.*§[A-Z]\.[0-9]+ p[0-9]+'`,
which scans my four commits' diffs directly. Both forms now report `0`.

## End

PR 6c (T069) lands locally as four atomically-revertable commits, every
commit GPG-signed, every commit per-target-crate tree-green, every commit
following Constitution VIII §-citation discipline. Ready for the
PM's 3-reviewer pass + PR open.
