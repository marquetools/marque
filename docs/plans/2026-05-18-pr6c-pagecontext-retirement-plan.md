<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 6c (T069) — `PageContext` struct retirement: strategic plan

**Date:** 2026-05-18
**Branch:** `refactor-006-pr-6c-pagecontext-struct-retirement` (off `origin/staging` @ `ed879a18`)
**Source plan §4 row:** PR 6c (`docs/plans/2026-05-02-engine-refactor-consolidated.md` §4 + `specs/006-engine-rule-refactor/tasks.md` T069).

## 1. Surviving `PageContext` surface (post-4b-F)

`crates/ism/src/page_context.rs::PageContext`. Complete public surface:

| Item | Shape | Used by |
|---|---|---|
| `pub struct PageContext { portions: Vec<CanonicalAttrs> }` | one private field | engine + 2 rules |
| `impl Default` (pre-sized `DEFAULT_PORTIONS_CAPACITY = 8`) | issue #430 | engine |
| `impl Clone` (manual; preserves pre-size) | #430 | engine `Arc::new(page_context.clone())` |
| `pub fn new() -> Self` | wraps `Default` | engine + tests |
| `pub fn add_portion(&mut self, attrs: CanonicalAttrs)` | push | engine `lint_inner` |
| `pub fn portion_count(&self) -> usize` | `.len()` | tests / sentinel |
| `pub fn is_empty(&self) -> bool` | `.is_empty()` | engine dispatch guards |
| `pub fn portions(&self) -> &[CanonicalAttrs]` | borrow | 2 rules + sentinel |

No `expected_*` accessors (retired in PR 4b-E). No `project` method (retired in PR 4b-D.2 in favor of `CapcoScheme::project_from_page_context`). No `reset` method — engine assigns `page_context = PageContext::new()` at page-break boundaries (`engine.rs::lint_inner` ~ line 941). `Send + Sync` enforced via `assert_impl_all!(PageContext: Send, Sync)` at `crates/ism/tests/send_sync.rs`.

## 2. Consumers

### 2.a Rule reads of `ctx.page_context` (production)

Two `Phase::PageFinalization` rules in `crates/capco/src/rules.rs` read `.portions()` and nothing else:

- **S005 `rel-to-opaque-uncertain-reduction`** (`analyze_uncertain_reduction`) — issue #488; needs per-portion `rel_to` membership.
- **W004 `JointDisunityCollapseRule`** (`check_pf`) — issue #461; calls `JointSet::from_attrs_iter(page_ctx.portions())` because `DisunityCollapse` is structurally per-portion and not on `ProjectedMarking`.

Neither rule reads `portion_count()` / `is_empty()`. Migration: `&[CanonicalAttrs]` via a renamed `RuleContext` field.

### 2.b Engine construction (`crates/engine/src/engine.rs`)

- `Engine::lint_inner` — owns one accumulator per document pass; calls `add_portion`, `is_empty`, `clone` (into `Arc`), assigns `PageContext::new()` at PageBreak reset.
- `dispatch_page_finalization` — receives `&PageContext`, force-inits `Arc<PageContext>` for rules.
- `project_page_marking` — receives `&PageContext`, calls `CapcoScheme::project_from_page_context(page_context)`.
- `check_portions_unchanged` sentinel — snapshots `page_ctx_arc.portions()` pre/post PageFinalization dispatch (`#[cfg(debug_assertions)]`-only).

### 2.c Scheme-side consumer

`CapcoScheme::project_from_page_context(&PageContext)` (`crates/capco/src/scheme/marking_scheme_impl.rs`) is a 1-line forward: `self.project_attrs_pipeline(page_context.portions())`. Exists *only* to call `.portions()`.

### 2.d Test fixtures

5 files construct `PageContext` synthetically: `crates/ism/src/page_context.rs::shim_tests` (6 unit tests), `crates/capco/tests/rules_us1.rs`, `crates/capco/tests/s004_audit_content_ignorance.rs`, `crates/engine/benches/profile_project.rs`, `crates/ism/tests/send_sync.rs`. Migration is mechanical: `PageContext::new() + add_portion` chains become `vec![…]` literals.

### 2.e Comment / doc drift

~30 files mention `PageContext` only in doc comments and PR-history notes. Mechanical sweep post-retirement.

## 3. Open questions (PM-decidable)

**OQ-1. Retain `PageContext` as a newtype around `Vec<CanonicalAttrs>` (A), or flatten entirely (B)?**
- (A) preserves a named API, leaves an expansion slot (per-portion spans per issue #461 mitigation), encapsulates the #430 pre-size, keeps `Send + Sync` a single grep target.
- (B) is the literal text of T069 ("delete `PageContext` struct"). Engine inlines `Vec<CanonicalAttrs>`; #430 pre-size becomes a free-standing const + `Vec::with_capacity` at the engine accumulator site. Lighter type surface.
- The surviving 4b-E module doc anticipates retirement-by-deletion (B). Recommend surfacing both; PM picks.

**OQ-2 (only if OQ-1 = A). Newtype lives in `marque-ism` (current) / `marque-rules` / `marque-scheme`?**
- `marque-scheme` would tie the WASM-safe leaf to a domain shape — wrong. `marque-rules` would invert the natural ownership (`CanonicalAttrs` lives in `marque-ism`). Constitution Principle VII §IV lists `marque-ism` as the foundational vocabulary crate owning the pivot. `PageContext` is a per-page bag of pivots — `marque-ism` is its rightful home. **Default: stay in `marque-ism` if OQ-1 = A.**

**OQ-3. New `RuleContext.page_portions` shape: `Option<Arc<Box<[CanonicalAttrs]>>>` (A) / `Option<Arc<Vec<CanonicalAttrs>>>` (B) / `Option<&'a [CanonicalAttrs]>` (C)?**
- (A) immutable snapshot; matches today's `Arc::new(page_context.clone())` semantics; mirrors Constitution II ("pivot fields use `Box<[T]>`, not `Vec`"). Engine freezes via `into_boxed_slice()` at first banner/CAB use.
- (B) keeps `Vec` headroom into the rule-context view (no semantic gain for an immutable snapshot).
- (C) breaks `BatchEngine`'s `Send + Sync` discipline if PageFinalization rules ever dispatch across `spawn_blocking`, AND defeats the same-page Arc-cache discipline at `engine.rs:1175-1185` (Arc::clone is a refcount bump; the per-portion borrow would force a re-snapshot).
- **Recommend (A).**

**OQ-4. Issue #430 pre-size: where does it land post-retirement?**
- Live accumulator inside `Engine::lint_inner`: `Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY)`; const moves with it. `Clone`-preserves-pre-size logic disappears under OQ-3 (A) because the `Box<[_]>` snapshot has no headroom concept — correct. Fold into PR 6c; no follow-up issue.

**OQ-5. Test-fixture migration in same commit as field rename, or separate?**
- 5 files, ~10 sites. Atomic update keeps the tree green per-commit. **Recommend single sweep within the relevant per-crate commit** (see §6).

**OQ-6. Page-break reset semantics — engine-internal change only?**
- Today `engine.rs:941`: `page_context = PageContext::new()` BEFORE attempting to parse the page-break candidate (Constitution VI). Post-retirement: `page_portions = Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY)`. Same offset, same invariant, no rule-visible change. Pin via the existing `page_context_resets_observably_across_form_feed` test (rename to drop "page_context").

**OQ-7. Constitution VII §IV engine-crate touch authorization.**
- PR 6c is a structural refactor, not a scheme adoption — §IV "scheme-adoption PR MUST NOT edit engine crates" does not bite directly. Within-006 engine-crate touch precedent: PR 4b-B Commit 2, PR 4b-C Commit 5, PR 4b-D.2 + 4b-D.3, PR 4b-E, PR 4b-F. **Cite explicitly in PR description; no constitutional amendment needed.**

## 4. Crate ripple (refute "4-crate" baseline)

| Crate | Change |
|---|---|
| `marque-ism` | Delete `src/page_context.rs` (or shrink under OQ-1 = A); update `lib.rs` re-export; update `tests/send_sync.rs`. |
| `marque-rules` | Rename `RuleContext.page_context` → `page_portions: Option<Arc<Box<[CanonicalAttrs]>>>` + setter rename; drop `marque_ism::PageContext` from public signature. |
| `marque-engine` | Accumulator inlined to `Vec<CanonicalAttrs>`; `dispatch_page_finalization` + `project_page_marking` take `&[CanonicalAttrs]`; `check_portions_unchanged` sentinel re-targets at slice; engine tests rename. |
| `marque-capco` | `project_from_page_context(&PageContext) → CanonicalAttrs` renames to `project_from_attrs_slice(&[CanonicalAttrs]) → CanonicalAttrs` (1-line body). S005 + W004 read sites migrate. Test fixtures in 3 files migrate. |
| `marque-engine` benches | `benches/profile_project.rs` updates 3 construction sites; comment drift in `benches/lint_latency.rs`. |
| **No change** | `marque-wasm` (`compute_banner_native` builds `Vec<CapcoMarking>` directly, no `PageContext` use), `marque-server`, `marque-extract`, `marque-config`, `marque` CLI, `marque-scheme` (trait surface doesn't reference `PageContext`). |

**6-crate ripple, not 4**: + engine benches + `marque-ism` test-utilities. WASM unaffected.

## 5. Scope boundary

PR 6c is **structural-only**.

- No new banner/CAB validation logic. S005 + W004 semantics unchanged.
- No `CanonicalAttrs` rename / shape change.
- No `MarkingType::PageBreak` heuristic change. Constitution VI invariant preserved.
- **Zero new CAPCO §-citations.** Constitution VIII gate: any `§X.Y pNN` citation appearing in PR 6c code, tests, or commit messages MUST be re-justified or removed. Existing citations in moved doc-comment text propagate verbatim and MUST be re-verified at the propagation point per Constitution VIII.
- No `Phase::PageFinalization` mechanism change. Issue #461 design preserved.
- No issue #461 per-portion-span enhancement (separate post-006 PR).
- No audit-schema bump. `marque-mvp-3` unchanged.

## 6. Commit decomposition (4 commits)

Each commit MUST land the tree green (`cargo check --workspace` + `cargo test -p <changed-crate>`).

1. **`marque-rules`**: add `page_portions: Option<Arc<Box<[CanonicalAttrs]>>>` field + `with_page_portions` setter as the canonical surface. Engine still constructs `PageContext` but feeds `Arc<Box<[_]>>` via `page_context.portions().to_vec().into_boxed_slice()` (or fold this prep into commit 2 if PM prefers).
2. **`marque-capco`**: S005 (`analyze_uncertain_reduction`) + W004 (`JointDisunityCollapseRule::check_pf`) consume `ctx.page_portions` instead of `ctx.page_context`. `CapcoScheme::project_from_page_context` renames to `project_from_attrs_slice(&[CanonicalAttrs])`. Test fixtures in `tests/rules_us1.rs`, `tests/s004_audit_content_ignorance.rs`, `tests/fr048_bare_nato_rel_to.rs` migrate. After this commit no `marque-capco` code reads `ctx.page_context`.
3. **`marque-engine`**: accumulator inlined to `Vec<CanonicalAttrs>` (pre-sized to `DEFAULT_PORTIONS_CAPACITY = 8`, OQ-4). `dispatch_page_finalization` + `project_page_marking` signatures take `&[CanonicalAttrs]`. Engine constructs `Arc<Box<[CanonicalAttrs]>>` snapshot from the live `Vec` at first banner/CAB use (mirrors today's lazy `Arc::new(page_context.clone())` discipline). `check_portions_unchanged` re-targets at slice. Engine tests `page_context_resets_observably_across_form_feed` + `page_context_lint_starts_fresh_on_each_call` rename to drop "page_context" but keep reset invariants. `benches/profile_project.rs` migrates.
4. **`marque-ism`**: delete `src/page_context.rs`. Drop `pub use PageContext` re-export from `lib.rs`. Drop `assert_impl_all!(PageContext: Send, Sync)` from `tests/send_sync.rs`. Re-grep `PageContext` workspace-wide; clean comment drift. Verify `RuleContext: Send + Sync` has a compile-time assert elsewhere (add if not present).

Each commit independently revertable.

## 7. Implementer checklist

Read constitution Principles IV / V / VI / VII / VIII (referenced in §3 OQ-7) and `crates/capco/CAPCO-CONTEXT.md` first.

- [ ] PM resolves OQ-1, OQ-3 (load-bearing; OQ-2 / 4 / 5 / 6 / 7 follow).
- [ ] **Commit 1** — `marque-rules`: add `page_portions` field + `with_page_portions` setter. `cargo check --workspace` + `cargo test -p marque-rules` green.
- [ ] **Commit 2** — `marque-capco`: migrate S005 + W004; rename `project_from_page_context` body to `&[CanonicalAttrs]`; migrate test fixtures (3 files). `cargo test -p marque-capco` green; `tests/lattice_vs_scheme_parity.rs` green.
- [ ] **Commit 3** — `marque-engine`: accumulator inline + `DEFAULT_PORTIONS_CAPACITY = 8`; `dispatch_page_finalization` + `project_page_marking` take `&[CanonicalAttrs]`; freeze-to-`Arc<Box<[CanonicalAttrs]>>` at first banner/CAB use; `check_portions_unchanged` re-targets; engine tests rename; `benches/profile_project.rs` migrates. `cargo test -p marque-engine` green; `lint_10kb` Criterion bench ≤ baseline + 10% (T070).
- [ ] **Commit 4** — `marque-ism`: delete `src/page_context.rs`; drop `pub use PageContext`; drop the Send+Sync assert; re-grep + clean comment drift. `cargo check --workspace` + `cargo test --workspace` green.
- [ ] Constitution VIII gate: re-grep new `§X.Y pNN` adds in commit diffs. Expect zero. Justify or remove any that appear.
- [ ] PR description cites within-006 engine-crate-touch precedent (OQ-7) explicitly.
- [ ] CI matrix per T071: `{6a-only, 6a+6b, 6a+6b+6c}` corpus regression green. T072: `tests/corpus/foreign/` 100%.
- [ ] FR-006 + I-12 (no `PageContext`-only paths exist) checkmark cleared on the invariants register.
