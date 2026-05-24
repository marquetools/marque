<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 6c (T069) — Rust mechanics review

**Date:** 2026-05-18  
**Reviewer:** rust-reviewer agent  
**Branch:** `refactor-006-pr-6c-pagecontext-struct-retirement`  
**Commits:** `42f31db1`..`507e5da9`

Pre-flight: `cargo check --workspace` CLEAN · `cargo +stable clippy --workspace --all-targets -- -D warnings` CLEAN · `cargo test --workspace` ALL PASS · `cargo doc --workspace --no-deps` zero warnings.

---

## Findings

### [LOW] M1 — Inconsistent snapshot constructor between the two call sites

**Symbols:** `Engine::lint_inner` line ~1217 vs `dispatch_page_finalization` line ~4333  
**Files:** `crates/engine/src/engine.rs`

The two `get_or_insert_with` closures that build the `Arc<Box<[CanonicalAttrs]>>` snapshot use different idioms:

- Main loop (line 1217): `page_portions.clone().into_boxed_slice()`
- `dispatch_page_finalization` (line 4333): `page_portions.to_vec().into_boxed_slice()`

Both are semantically correct and produce a single allocation (verified: `clone()` on a `Vec` where the source has `len == capacity` produces `len == capacity` in the result, so `into_boxed_slice` does not shrink). However the inconsistency is a readability debt. The `to_vec()` form (used in `dispatch_page_finalization`) is the idiomatic slice-to-vec conversion and is slightly more expressive of intent when the source is `&[T]` rather than `Vec<T>`. The main loop's source is `Vec<T>` (not a slice), so `clone()` is equally valid. This is stylistic, not a correctness issue, but should be noted for future consistency.

**Recommendation:** Normalize to one form (either is fine). If normalizing, prefer `page_portions.to_vec().into_boxed_slice()` at both sites to make the "clone-and-shrink" intent explicit regardless of whether the source is `Vec<T>` or `&[T]`.

---

### [LOW] M2 — Stale `PageContext` mentions in test string literals and comments not cleaned up

**Files:** `crates/capco/tests/banner_rollup_walker.rs` (line 362), `crates/capco/src/rules.rs` (line 8544), `crates/ism/Cargo.toml` (line 69)

Three stale references survive the sweep:

1. `banner_rollup_walker.rs::no_banner_rollup_without_page_context`: assertion string contains `"no PageContext"` — the test name and error string still reference `PageContext` semantically. The test is correct (it validates the empty-page guard), but the wording is stale post-retirement.

2. `rules.rs::e035_no_ops_without_page_context` (test name + assertion string at line 8544): the test is valid, the wording describes the old mechanism. No behavioral issue.

3. `crates/ism/Cargo.toml` comment at line 69: `"# PR 4b-E: compile-time \`Send + Sync\` checks on \`PageContext\` and"` — the `PageContext` type was retired in this PR; the comment now describes a type that no longer exists in the crate.

The implementation report's commit 4 checklist says "Re-grep `PageContext` workspace-wide; clean any remaining doc-comment drift" — these three sites were missed.

**Recommendation:** Update the `Cargo.toml` comment (line 69) to remove the `PageContext` reference (`CanonicalAttrs` is the only type being checked post-PR-6c). The test string literals and names are lower priority but should be updated in the same pass.

---

### [LOW] M3 — `DEFAULT_PORTIONS_CAPACITY` redeclared in test fixture

**File:** `crates/capco/tests/rules_us1.rs` (line 36)

The test fixture declares `const DEFAULT_PORTIONS_CAPACITY: usize = 8;` locally rather than importing from `marque_engine`. This is not a defect — `marque-capco` tests cannot import `marque-engine` (that would violate Constitution VII crate directionality). However the duplication means a future capacity change must update two sites. The comment at line 33 cross-references the engine constant, which makes the relationship traceable.

**Recommendation:** No code change required. The cross-reference comment is sufficient documentation. Consider adding `// Must match marque_engine::engine::DEFAULT_PORTIONS_CAPACITY` to the test constant in a future cleanup pass.

---

## Scope-specific verification results

**OQ-3 — `Arc<Box<[CanonicalAttrs]>>` correctness.**  
The snapshot is built exactly once per page via `get_or_insert_with` at both the main banner/CAB dispatch site and inside `dispatch_page_finalization`. Both sites mutate the same outer `page_portions_arc: Option<Arc<Box<[CanonicalAttrs]>>>` through a `&mut` reference. The local shadow in `dispatch_page_finalization` (line 4332 — `let page_portions_arc = ...clone()`) correctly separates the `Option`-mutating initialization from the local handle used inside the function body. Arc-cache discipline is preserved: one allocation per page at first banner/CAB use.

**Snapshot construction cost.**  
`page_portions.clone().into_boxed_slice()` (line 1217) allocates once (clone produces `len == capacity`; `into_boxed_slice` does not reallocate). `page_portions.to_vec().into_boxed_slice()` (line 4333) is identical in cost. The pre-PR-6c `PageContext::clone()` also allocated once (the manual impl used `Vec::with_capacity(cap) + extend`). Net change: zero extra allocations vs pre-PR-6c. Finding M1 is stylistic only.

**`DEFAULT_PORTIONS_CAPACITY = 8` const.**  
Lives in `crates/engine/src/engine.rs` at module level as `pub(crate)`. Doc-comment names issue #430. Both accumulator sites (`lint_inner` startup + page-break reset) use it. Correct per plan.

**HRTB Send+Sync check.**  
`fn _rule_context_is_send_sync<'a>() where RuleContext<'a>: Send + Sync {}` is present in `crates/rules/tests/send_sync.rs` (line 133). It is a function (not const fn), takes `'a` as a lifetime parameter, and the `where` bound requires `RuleContext<'a>: Send + Sync` for any `'a`. The test file is compiled as part of `cargo test -p marque-rules`; the bound is exercised at compile time. Correct per PM-decisions amendment #2.

**Lifetime hygiene.**  
`page_portions: Option<Arc<Box<[CanonicalAttrs]>>>` is fully owned — no new lifetime annotation needed, no borrow of `'a` from the page slice. The `'a` lifetime on `RuleContext<'a>` remains scoped solely to `pre_pass_1_attrs: Option<&'a CanonicalAttrs>`. No regression.

**`check_portions_unchanged` sentinel.**  
The error message at `crates/engine/src/engine.rs::check_portions_unchanged` is counts-only (before len, after len, rule count). The `g13_compliant` assertion test (`check_portions_unchanged_error_message_is_g13_compliant`) remains intact, exercising the sentinel value `MARQUE-PR-490-G13-CANARY-XYZZY-7F3A1B2C` embedded in `classified_by`. G13 compliance verified.

**Engine accumulator pre-sizing.**  
`Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY)` appears at both: (a) `lint_inner` startup (line 818), and (b) every `MarkingType::PageBreak` reset (line 969). Constitution VI reset-before-parse invariant: the reset at line 969 occurs AFTER `dispatch_page_finalization` returns (or is skipped when `page_portions.is_empty()`) and BEFORE the `continue` that skips to the next candidate. A malformed page-break candidate would hit the `continue` at line 985 without accumulating into `page_portions`. The invariant holds.

**Test rename without behavior change.**  
Both renamed tests (`page_portions_reset_observably_across_form_feed`, `page_portions_lint_starts_fresh_on_each_call`) use `ctx.page_portions.as_ref().map(|pp| pp.as_ref().len())` in `ContextRecorderRule::check`. The assertion bodies test identical semantic properties (page-break reset observability, cross-call isolation). Verified by reading the full test bodies at lines 5576-5668.

**Adjacent callsites — stale references.**  
Workspace grep for `PageContext` in non-comment production code: zero hits. Grep for `with_page_context` in production code: zero hits. Grep for `project_from_page_context` in production code: zero hits — only appears in doc comments describing the PR-6c rename history. No stale callers in `marque-server`, `marque-extract`, `marque-wasm`, or the CLI binary.

**Constitution VII directionality.**  
`marque-rules` depends on `marque-ism` (pre-existing, `CanonicalAttrs` was already in `pre_pass_1_attrs`). `marque-scheme` depends on no domain crate. No new backward edges introduced. Acyclic graph preserved.

**Per-commit tree-green discipline.**  
Commit 1: dual-field (both `page_context` + `page_portions` present on `RuleContext`) — tree-green by construction. Commit 2: included a minimal engine bridge to keep `cargo test -p marque-capco` green for W004's PageFinalization test suite (documented deviation in implementation report). Commit 3: old field removed, engine migrated. Commit 4: `page_context.rs` deleted. All four compile cleanly per `cargo check --workspace`.

---

## Verdict

**✅ Ready to submit.**

No CRITICAL or HIGH issues found. Three LOW findings (M1 snapshot-constructor inconsistency, M2 three stale `PageContext` string references, M3 duplicated capacity constant). M2 includes one Cargo.toml comment that is strictly stale (references a deleted type) — it is the highest-priority LOW item but is not a correctness or safety issue. All mandatory checks pass (clippy clean, all tests green, doc build clean). Architecture, Arc-cache discipline, Constitution VI reset invariant, G13 compliance, Send+Sync coverage, and crate directionality all verified correct.

**Suggested follow-up (not blocking):** address M2 Cargo.toml comment in a single-line patch before the PR description is finalized. M1 and M3 are deferred.
