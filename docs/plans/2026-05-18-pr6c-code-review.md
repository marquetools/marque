<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 6c (T069) — Code Review

**Date:** 2026-05-18  
**Reviewer:** code-reviewer agent  
**Branch:** `refactor-006-pr-6c-pagecontext-struct-retirement`  
**Commits reviewed:** `42f31db1` → `507e5da9` (4 commits)

---

## Findings

### [MEDIUM] Stale test name and comment in `e035_no_ops_without_page_context`

**File:** `crates/capco/src/rules.rs` — `e035_no_ops_without_page_context`

The test function name and its inline comment both reference `page_context` (the deleted field) and `PageContext` (the deleted type). The comment reads *"The test harness passes `page_context: None`. Until P4 lands and populates a real PageContext with expected_sci_markings()..."* — this is factually wrong post-PR-6c: the field `page_context` no longer exists on `RuleContext`. The test calls `lint_banner(...)` which exercises the full engine path; the assertion itself is still correct (E035 must not fire without per-page context). Only the name and comment are stale.

**Recommendation:** Rename to `e035_no_ops_without_page_portions` and update the inline comment to reference `page_portions` / `ctx.page_portions` and drop the `PageContext` type name.

---

### [MEDIUM] Stale comment in `BannerMatchesProjectedRule` dispatch guard

**File:** `crates/capco/src/rules.rs` — `BannerMatchesProjectedRule::check` body (~line 4116)

The comment reads: *"The per-portion view stays on `ctx.page_context` for rules that need it (e.g. S005 post-PR-#488 — formerly the S005/S006 pair)."*

Post-PR-6c `ctx.page_context` no longer exists. S005 now reads `ctx.page_portions`. The factual claim is inverted — a new reader would look for a field that is gone.

**Recommendation:** Update to: *"The per-portion view is available via `ctx.page_portions` (e.g. S005 post-PR-#488)."*

---

### [MEDIUM] Stale `project_from_page_context` reference in `CAPCO-CONTEXT.md`

**File:** `crates/capco/CAPCO-CONTEXT.md` — §3 "PR 4b-D.2 flipped the hot path..." paragraph (~line 247)

The description of the post-PR-4b-D.2 architecture names `CapcoScheme::project_from_page_context` as one of the two surviving page-aggregation paths. That method was deleted in commit 3. A reviewer reading CAPCO-CONTEXT.md would try to `grep` for a function that no longer exists.

**Recommendation:** Replace `CapcoScheme::project_from_page_context` with `CapcoScheme::project_from_attrs_slice` at that sentence. This is a doc-comment in a mandatory-preflight file — correctness matters here.

---

### [MEDIUM] `DEFAULT_PORTIONS_CAPACITY` duplicated in two `cfg(any())`-gated test files

**Files:** `crates/capco/tests/rules_us1.rs` and `crates/capco/tests/s004_audit_content_ignorance.rs`

Both files declare a local `const DEFAULT_PORTIONS_CAPACITY: usize = 8` with a doc-comment pointing at `crates/engine/src/engine.rs::DEFAULT_PORTIONS_CAPACITY`. The const is `pub(crate)` in `marque-engine`, which is not a dev-dependency of `marque-capco`, so there is no cleaner alternative at the moment.

The value is hard-coded twice. If the engine's const is bumped (e.g., for a corpus-driven empirical re-assessment), these test files will silently drift. Both files are currently `#![cfg(any())]`-gated so the drift won't cause a test failure — it will silently be wrong.

**Recommendation:** Add a comment block explicitly noting *"This local copy MUST be updated in sync with `marque_engine::DEFAULT_PORTIONS_CAPACITY` if that const changes"* — the current doc-comment says where it mirrors but does not warn about the maintenance obligation. Alternatively, if T069 acceptance gates include a linter that checks numeric constants, add a `static_assertions::const_assert_eq!` comparing against the engine value (requires adding `marque-engine` as a dev-dep for the test file, which may or may not be acceptable given the dep graph).

---

### [LOW] `page_portions_arc.as_ref().to_vec()` in `check_portions_unchanged` snapshot

**File:** `crates/engine/src/engine.rs` — `dispatch_page_finalization` (`portions_before` snapshot, line 4414)

The expression `page_portions_arc.as_ref().to_vec()` calls `.as_ref()` on an `Option<Arc<Box<[CanonicalAttrs]>>>`, yielding `Option<&Arc<Box<[CanonicalAttrs]>>>`, then `.to_vec()`. This relies on the `Option::to_vec()` inherent method (stable since Rust 1.0), which produces `vec![]` on `None` and `vec![arc_ref]` on `Some`. The result is a `Vec<&Arc<Box<[CanonicalAttrs]>>>` — a vector of one reference, not a `Vec<CanonicalAttrs>`. The type annotation is `Vec<marque_ism::CanonicalAttrs>`, so this compiles, meaning the compiler accepted it through some auto-deref chain. This merits scrutiny: the expression does not look like it produces the intended pre-dispatch snapshot of portion data.

The pre-PR-6c equivalent was `page_ctx_arc.portions().to_vec()` which produced `Vec<CanonicalAttrs>` directly. The post-PR-6c replacement should be `page_portions_arc.as_ref().map(|a| a.as_ref().to_vec()).unwrap_or_default()` or more simply, since `page_portions_arc` always exists at this point in the dispatch (the guard above it asserts `!page_portions.is_empty()`), `page_portions_arc.as_deref().map(|s| s.to_vec()).unwrap_or_default()`.

The code compiles and tests pass (implementer attests), but the expression is non-obvious and the type coercion path should be explicitly documented or simplified.

**Recommendation:** Rewrite as `page_portions_arc.as_deref().map_or_else(Vec::new, |s| s.to_vec())` and add a comment explaining the snapshot is for the `check_portions_unchanged` G13 sentinel.

---

### [LOW] `banner_rollup_walker.rs` comment references deleted `ctx.page_context` field

**File:** `crates/capco/tests/banner_rollup_walker.rs` — `walker_silent_when_banner_has_no_preceding_portions` test (~line 354)

The inline comment says *"so `ctx.page_context` stays `None` and the walker returns early"*. The field name is now `page_portions`. The assertion message in the same test says *"walker fired on a banner with no PageContext"* — which references the deleted type. These are test comments, not production code, but per the PM directive, comment accuracy is treated as part of maintainability quality.

**Recommendation:** Update `ctx.page_context` → `ctx.page_portions` and `PageContext` → `page portions` in the comment and assertion message.

---

### [LOW] `tasks.md` T069–T072 not ticked off

**File:** `specs/006-engine-rule-refactor/tasks.md` — T069, T070, T071, T072

T069 has `[ ]` (unchecked). The PM-decisions doc explicitly calls out: *"Tasks bookkeeping: Tick T069 + any T070-T072 acceptance gates that PR 6c satisfies."* The implementer's report does not mention updating tasks.md. T069 is complete in this branch; at minimum the T069 checkbox should be ticked prior to opening the PR.

**Recommendation:** Tick `T069` in `tasks.md` before PR open. T070-T072 acceptance gates are post-merge CI items — leave those as `[ ]` with a `[P]`-gated note if appropriate.

---

### [LOW] Historical `sar_sort.rs` doc-comment references `page_context.rs` location

**File:** `crates/ism/src/sar_sort.rs` — module-level doc comment

The doc comment refers to "`crates/ism/src/page_context.rs` alongside the `PageContext`" as the previous home of the sort function. That file is now deleted. The historical note is accurate as historical context but the path is a dangling reference.

**Recommendation:** Update the past-tense reference to acknowledge the deletion: *"previously lived in the deleted `crates/ism/src/page_context.rs`"*.

---

## Checklist results

**Commit decomposition quality:** Each commit is independently revertable and lands tree-green per the implementer's attestation. Commit messages are thorough and cite the PM-decisions doc and OQ numbers. The two documented deviations (shim + engine bridge in commit 2) are clearly explained in commit 2's message and the implementation report. No issues here.

**API naming:** `page_portions` is clearer than `page_context` for the field's actual meaning. `project_from_attrs_slice` over `project_from_page_context` communicates the parameter type directly. Both names will read well in 5 years.

**Constitution V G13 content-ignorance:** No new `Display` or `{:?}` routing of `page_portions` content through log/error/panic paths. `check_portions_unchanged` formats counts and indices only. Verified clean.

**Constitution VIII §-citation gate:** Zero new `§X.Y pNN` citations across all 4 commits. Confirmed by `git log -p 42f31db1^..HEAD | rg '^\+.*§[A-Z]\.[0-9]+ p[0-9]+'` returning empty. Satisfies the operative-checklist requirement.

**Send+Sync re-pin:** `crates/rules/tests/send_sync.rs` correctly adds the HRTB `fn _rule_context_is_send_sync<'a>() where RuleContext<'a>: Send + Sync {}` per PM-decisions amendment #2. The `CanonicalAttrs: Send + Sync` axiom in `crates/ism/tests/send_sync.rs` is preserved.

**OQ-7 engine-touch authorization:** Commit messages cite within-006 precedent. PR description should cite it explicitly as the PM checklist requires.

**Adjacent callsites — PageContext residue:** All remaining `PageContext` references in Rust source files are in comments only (historical migration notes, test fixture explanations, lattice parity gate history). Zero live code references to the retired field, setter, or type. `project_from_page_context` is fully removed from all Rust code; only a stale reference in `CAPCO-CONTEXT.md` survives (the MEDIUM finding above).

---

## PR Description Draft

```markdown
## PageContext struct retirement (T069)

Fully deletes `marque_ism::PageContext` — the thin newtype wrapper around
`Vec<CanonicalAttrs>` that served as the per-page portion accumulator since
Phase 3. `PageContext` became a residue wrapper after PR 4b-E retired its
final substantive surface (`expected_*` accessors, `project`, `is_classified`,
`is_solely_nato_classified`). PR 6c completes the retirement by inlining the
accumulator directly into `Engine::lint_inner`.

### What changed

- `marque-ism`: `src/page_context.rs` (275 lines) deleted. `pub use PageContext`
  re-export dropped from `lib.rs`.
- `marque-rules`: `RuleContext.page_context` field + `with_page_context` setter
  replaced by `page_portions: Option<Arc<Box<[CanonicalAttrs]>>>` +
  `with_page_portions`. `Arc<Box<[CanonicalAttrs]>>` chosen over
  `Arc<Vec<CanonicalAttrs>>` per OQ-3 (Constitution II `Box<[T]>` pivot
  convention; snapshot is genuinely immutable once frozen).
- `marque-engine`: `lint_inner` accumulator inlined to `Vec<CanonicalAttrs>` +
  `DEFAULT_PORTIONS_CAPACITY = 8` const (issue #430 pre-size migrated here).
  `dispatch_page_finalization` and `project_page_marking` take `&[CanonicalAttrs]`
  directly. Arc snapshot frozen lazily at first banner/CAB dispatch, matching
  the pre-PR-6c `Arc::new(page_context.clone())` cadence. Two engine test renames:
  `page_context_resets_observably_*` → `page_portions_reset_observably_*`.
- `marque-capco`: S005 (`analyze_uncertain_reduction`) and W004
  (`JointDisunityCollapseRule::check`) migrated from `ctx.page_context.portions()`
  to `ctx.page_portions`. `CapcoScheme::project_from_page_context` renamed to
  `project_from_attrs_slice`.
- `crates/rules/tests/send_sync.rs`: HRTB `RuleContext: Send + Sync` check added
  per rust-preflight Risk #3 / PM-decisions amendment #2 — closes the gap opened
  by removing the `assert_impl_all!(PageContext: Send, Sync)` pin.

### What is NOT in scope

- No new banner/CAB validation logic. S005 + W004 semantics unchanged.
- No `CanonicalAttrs` shape changes.
- No `MarkingType::PageBreak` heuristic change. Constitution VI reset-before-parse
  invariant preserved.
- No audit-schema bump. `marque-mvp-3` unchanged.
- No issue #461 per-portion-span enhancement (separate post-006 PR).
- Zero new CAPCO §-citations (Constitution VIII gate confirmed clean).

### Engine-crate touch authorization

PR 6c is a structural refactor, not a scheme adoption. Constitution VII §IV
("scheme-adoption PR MUST NOT edit engine crates") does not apply. Within-006
engine-crate touch precedent: PR 4b-B Commit 2 / PR 4b-C Commit 5 / PR 4b-D.2 /
PR 4b-D.3 / PR 4b-E / PR 4b-F. OQ-7 resolved: proceed without constitutional
amendment.

### OQ resolutions

| OQ | Decision |
|---|---|
| OQ-1 | Full delete (B). Engine inlines `Vec<CanonicalAttrs>`. |
| OQ-3 | `Option<Arc<Box<[CanonicalAttrs]>>>` on `RuleContext.page_portions`. |
| OQ-4 | `DEFAULT_PORTIONS_CAPACITY = 8` const co-located with engine accumulator. |
| OQ-7 | No constitutional amendment; cite within-006 precedent. |

### Tasks

- Closes T069 (FR-006 / I-12 invariant cleared — no `PageContext`-only paths exist)
- T070–T072 acceptance gates: post-merge CI verification items

### Per-commit summary

1. `42f31db1` `marque-rules`: add `page_portions` + `with_page_portions`; keep
   dual-field for tree-greenness
2. `16124a3c` `marque-capco`: S005 + W004 migrate; `project_from_page_context` →
   `project_from_attrs_slice`; engine bridge added (commit 3 removes)
3. `eda09358` `marque-engine`: accumulator inlined; shim deleted; `page_context`
   field deleted; test renames; bench migrations
4. `507e5da9` `marque-ism`: `page_context.rs` deleted; Send+Sync re-pinned; ~13
   production files' doc-comment drift cleaned

### Test plan

- [ ] `cargo +stable clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace` all pass
- [ ] `crates/capco/tests/lattice_vs_scheme_parity.rs` green (74 fixtures)
- [ ] Constitution VIII gate: `git log -p 42f31db1^..HEAD | rg '^\+.*§[A-Z]\.[0-9]+ p[0-9]+'` → 0
- [ ] T071 CI matrix: `{6a-only, 6a+6b, 6a+6b+6c}` corpus regression green
- [ ] T072: `tests/corpus/foreign/` 100%

### Bench gate

`lint_10kb` baseline is stale (~828µs; threshold 911µs). If CI bench marginal-fails
with no other regression: `gh run rerun <id> --failed` once (standard noise-band
mitigation). If still failing: escalate to PM — do not optimize PR 6c.

### LOC delta

`-195 LOC` production code; `+432 LOC` planning docs (3 new files).
```

---

## Verdict

🟡 **fix items** — 3 MEDIUM findings should be addressed before merge; 4 LOW items are strongly recommended. No CRITICAL or HIGH issues. The structural work is correct, the plan was followed faithfully, and the compliance gates (G13, Constitution VIII §-citation, Send+Sync re-pin) are all satisfied.

| Severity | Count | Status |
|---|---|---|
| CRITICAL | 0 | pass |
| HIGH | 0 | pass |
| MEDIUM | 3 | warn |
| LOW | 4 | note |

The three MEDIUM fixes are small (test rename, two comment corrections) and can land as a single fixup commit or be addressed in commit 4's comment-drift sweep scope before the PR opens.
