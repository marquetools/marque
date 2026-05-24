<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 6c (T069) — PM decisions addendum

**Date:** 2026-05-18
**Companions:**
- Strategic plan: `docs/plans/2026-05-18-pr6c-pagecontext-retirement-plan.md`
- Rust risk register: `docs/plans/2026-05-18-pr6c-rust-preflight.md`

This addendum locks the architect's 7 OQs to single answers + folds the rust-specialist's 3 architect-amendment findings into the operative plan. The implementer follows the architect plan §6 commit decomposition + §7 checklist + the rust-specialist §12 checklist, **with the adjustments below**.

## Locked decisions

| OQ | Decision | Source |
|---|---|---|
| **OQ-1** | **B — full delete.** `PageContext` struct retired entirely. Engine inlines `Vec<CanonicalAttrs>` accumulator with `DEFAULT_PORTIONS_CAPACITY = 8` const moved alongside. | PM, 2026-05-18 |
| **OQ-2** | N/A (only fires under OQ-1=A). | — |
| **OQ-3** | **A — `Option<Arc<Box<[CanonicalAttrs]>>>` on `RuleContext`.** Preserves page-shared Arc-cache discipline; mirrors Constitution II `Box<[T]>` pivot-field convention. | Architect + rust-specialist concur |
| **OQ-4** | Fold issue #430 pre-size into PR 6c. Const moves with the engine accumulator. No follow-up issue. | Architect §3 OQ-4 recommendation |
| **OQ-5** | Test-fixture migration lands atomically within the relevant per-crate commit (not a separate sweep). | Architect §3 OQ-5 recommendation |
| **OQ-6** | Page-break reset semantics: engine-internal change only. Constitution VI reset-before-parse invariant preserved. Existing `page_context_resets_observably_across_form_feed` test renames to drop "page_context" but keeps the assertion. | Architect §3 OQ-6 + Constitution VI |
| **OQ-7** | Proceed without constitutional amendment. PR description cites within-006 engine-crate-touch precedent (PR 4b-B Commit 2 / 4b-C Commit 5 / 4b-D.2 / 4b-D.3 / 4b-E / 4b-F) explicitly. | PM, 2026-05-18 |

## Architect-plan amendments (folded from rust-specialist)

1. **Commit 2 fixture list correction.** Architect §6 commit 2 names `tests/fr048_bare_nato_rel_to.rs` as a fixture migration target. **That file does not construct `PageContext`**; it references the type only in a doc comment (which gets swept under §6 commit 4's comment-drift cleanup). **Strike `fr048_bare_nato_rel_to.rs` from the commit 2 per-file fixture migration list.** The actual fixture migration targets are: `crates/capco/tests/rules_us1.rs` and `crates/capco/tests/s004_audit_content_ignorance.rs` (2 files, ~4 sites).

2. **Commit 4 Send+Sync pin addition.** Architect §6 commit 4 says "Verify `RuleContext: Send + Sync` has a compile-time assert elsewhere (add if not present)." The answer is **not present anywhere today**. `assert_impl_all!` requires `'static` so `RuleContext<'a>` can't go through that macro directly. Add an HRTB function-bound check to `crates/rules/tests/send_sync.rs` in commit 4:

   ```rust
   fn _rule_context_is_send_sync<'a>()
   where
       marque_rules::RuleContext<'a>: Send + Sync,
   {}
   ```

   This is 3 lines + a `use` line. Closes the Send+Sync gap that PR 6c silently opens by removing the `assert_impl_all!(PageContext: Send, Sync)` pin.

3. **Bench-gate playbook (architect §7 checklist addition).** `lint_10kb` baseline (828µs) is stale; current measurements straddle the 911µs threshold. PR 6c is structurally similar to PR 4b-F which needed one `gh run rerun <id> --failed`. Add to the architect §7 implementer checklist:

   > - [ ] If `lint_10kb` Criterion bench fails on first CI run AND no other bench regresses: `gh run rerun <id> --failed` once (standard noise-band mitigation per `project_bench_baseline_staleness`). If still failing after the re-run, STOP and escalate to PM — do NOT optimize PR 6c to pass the gate (it is structural-only / semantics-preserving).

## Operative checklist (consolidated)

Single source of truth for the implementer. Supersedes the per-document checklists where they overlap.

- [ ] **Pre-flight reading (mandatory):** `crates/capco/CAPCO-CONTEXT.md` (full), architect strategic plan, rust risk register, this PM-decisions doc, Constitution Principles II / V / VI / VII / VIII.

- [ ] **Commit 1 — `marque-rules`:** Add `page_portions: Option<Arc<Box<[CanonicalAttrs]>>>` field + `with_page_portions` setter to `RuleContext`. **Keep the existing `page_context: Option<Arc<PageContext>>` field for now** (dual-field, tree stays green this commit). `cargo +stable clippy -p marque-rules -- -D warnings` + `cargo test -p marque-rules` green.

- [ ] **Commit 2 — `marque-capco`:** Migrate S005 (`analyze_uncertain_reduction`) + W004 (`JointDisunityCollapseRule::check_pf`) read sites from `ctx.page_context.portions()` → `ctx.page_portions`. Rename `CapcoScheme::project_from_page_context(&PageContext)` body to `project_from_attrs_slice(&[CanonicalAttrs])`. Migrate 2 test fixtures: `tests/rules_us1.rs`, `tests/s004_audit_content_ignorance.rs` (NOT `fr048_bare_nato_rel_to.rs`). `cargo +stable clippy -p marque-capco -- -D warnings` + `cargo test -p marque-capco` green; `tests/lattice_vs_scheme_parity.rs` green.

- [ ] **Commit 3 — `marque-engine`:** Accumulator inlined to `Vec<CanonicalAttrs>` with `DEFAULT_PORTIONS_CAPACITY = 8` const. `dispatch_page_finalization` + `project_page_marking` take `&[CanonicalAttrs]`. Engine constructs `Arc<Box<[CanonicalAttrs]>>` snapshot at first banner/CAB use (mirrors today's lazy `Arc::new(page_context.clone())`). `check_portions_unchanged` sentinel re-targets at slice (G13 message stays counts-only). Engine tests `page_context_resets_observably_across_form_feed` + `page_context_lint_starts_fresh_on_each_call` rename to drop "page_context" (keep assertions). `benches/profile_project.rs` migrates 3 sites. **Delete the old `RuleContext.page_context` field + `with_page_context` setter** at this commit's end (touches `marque-rules` again — fine, single commit lands tree-green). `cargo +stable clippy -p marque-engine -- -D warnings` + `cargo test -p marque-engine` green.

- [ ] **Commit 4 — `marque-ism` deletion + Send+Sync pin:** Delete `crates/ism/src/page_context.rs`. Drop `pub use page_context::PageContext` from `crates/ism/src/lib.rs`. Drop `assert_impl_all!(PageContext: Send, Sync)` from `crates/ism/tests/send_sync.rs`. **ADD** the HRTB `RuleContext: Send + Sync` check to `crates/rules/tests/send_sync.rs` (amendment #2 above). Re-grep `PageContext` workspace-wide; clean any remaining doc-comment drift. `cargo +stable clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` green.

- [ ] **Final Constitution VIII gate:** `git diff staging..HEAD -- '*.rs' '*.md' | rg '§[A-Z]\.[0-9]+ p[0-9]+'` → expect zero new hits. Any hit MUST be justified inline or removed (memory `project_capco_doc_structure`).

- [ ] **CI bench:** if `lint_10kb` marginal-fails, `gh run rerun <id> --failed` once. If persistent, escalate to PM — do NOT optimize PR 6c.

- [ ] **PR description:** Cite within-006 engine-crate-touch precedent (PR 4b-B Commit 2 / 4b-C Commit 5 / 4b-D.2 / 4b-D.3 / 4b-E / 4b-F). State explicitly that PR 6c is structural-only; zero new §-citations; T069 closes; FR-006 / I-12 invariant cleared.

- [ ] **Tasks bookkeeping:** Tick T069 + any T070-T072 acceptance gates that PR 6c satisfies in `specs/006-engine-rule-refactor/tasks.md`.

## Out of scope (don't touch)

- Constitution II hardening of `derived_from` / `classified_by` `Box<str>` fields (separate concern, deferred).
- Issue #461 per-portion-span enhancement (separate post-006 PR).
- `CanonicalAttrs` shape changes.
- `MarkingType::PageBreak` scanner heuristic.
- Audit-schema bump (stays at `marque-mvp-3`).
- Any new banner/CAB validation logic.
- Any new CAPCO §-citation.

End.
