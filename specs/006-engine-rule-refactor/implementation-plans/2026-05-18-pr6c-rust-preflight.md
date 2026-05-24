<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 6c (T069) — Rust-mechanics preflight risk register

**Companion to** `docs/plans/2026-05-18-pr6c-pagecontext-retirement-plan.md` (architect strategic plan, 2026-05-18).
**Branch:** `refactor-006-pr-6c-pagecontext-struct-retirement` off `origin/staging` @ `ed879a18`.
**Scope:** tactical Rust risks only — strategy / commit decomposition / OQ resolution stays with the architect plan.

This register assumes OQ-1 = **B** (full deletion — the architect-recommended path) and OQ-3 = **A** (`Option<Arc<Box<[CanonicalAttrs]>>>`). Risks below would shift under OQ-1 = A (newtype retention); flagged inline.

---

## 1. API breakage surface 🟡 medium

### 1.a Cross-crate `pub` items disappearing

| Item | Defined in | Cross-crate callers |
|---|---|---|
| `marque_ism::PageContext` struct | `crates/ism/src/page_context.rs` | `marque-rules` (`RuleContext.page_context`), `marque-engine` (`Engine::lint_inner`, `dispatch_page_finalization`, `project_page_marking`, `check_portions_unchanged`), `marque-capco` (`CapcoScheme::project_from_page_context`, S005, W004), 3 `marque-capco` integration tests, 1 `marque-engine` bench |
| `marque_ism::PageContext::new` / `Default` / `Clone` | same | engine + 5 test/bench fixtures |
| `marque_ism::PageContext::add_portion` | same | engine `Engine::lint_inner` only |
| `marque_ism::PageContext::portion_count` | same | none in production; bench/test only |
| `marque_ism::PageContext::is_empty` | same | engine dispatch guards |
| `marque_ism::PageContext::portions` | same | S005 (`analyze_uncertain_reduction`), W004 (`JointDisunityCollapseRule::check_pf`), `CapcoScheme::project_from_page_context`, debug-sentinel |
| `pub use page_context::PageContext` re-export in `crates/ism/src/lib.rs` | `marque-ism` | all callers above use the re-export, not the `page_context::` path |
| `marque_rules::RuleContext.page_context` field | `crates/rules/src/lib.rs` | engine constructs, S005/W004 read |
| `marque_rules::RuleContext::with_page_context` setter | same | engine `Engine::lint_inner`, downstream test fixtures |
| `marque_capco::CapcoScheme::project_from_page_context` | `crates/capco/src/scheme/marking_scheme_impl.rs` | engine `project_page_marking` only |

### 1.b Within-crate `pub(crate)` items

- `check_portions_unchanged` in `marque-engine` is `pub(crate)` — signature change (`&PageContext` → `&[CanonicalAttrs]`) is internal-only, but it has 4 unit tests at the bottom of `engine.rs` (`sentinel_tests::check_portions_unchanged_*`) that construct slices directly today — no churn there.

### 1.c No downstream external consumers

This is a pre-1.0 crate; nothing outside the workspace depends on `marque-ism` / `marque-rules` / `marque-engine` / `marque-capco`. The "API breakage" framing matters only to the in-tree call graph above, all of which the 4-commit plan covers.

---

## 2. `Arc<PageContext>` lifetime + threading 🟢 low

The architect plan correctly identifies the right migration target: `Option<Arc<Box<[CanonicalAttrs]>>>` on `RuleContext`.

Findings:

- `RuleContext<'a>` is **already lifetime-parameterized** (`crates/rules/src/lib.rs::RuleContext<'a>`) for `pre_pass_1_attrs: Option<&'a CanonicalAttrs>`. The `'a` is well-established; no new lifetime introduction needed.
- The engine passes `&RuleContext` into `Rule::check` (see `marque-rules` `Rule::check` signature). Owned-context dispatch never appears.
- Today's `Arc<PageContext>` is engine-owned, cloned cheaply across consecutive banner/CAB candidates on the same page via `page_context_arc.get_or_insert_with(|| Arc::new(page_context.clone()))` (`engine.rs::lint_inner`). The `Arc::clone` is a refcount bump; the underlying `PageContext::clone` (manual impl preserving the issue #430 pre-size) runs **once per page** at first banner/CAB use, not per rule.
- Switching to `Option<Arc<Box<[CanonicalAttrs]>>>` preserves the same Arc-cache discipline. The bridge is one extra `to_vec().into_boxed_slice()` per page at the snapshot point (negligible vs the existing `Vec::clone()` that path already pays — `Box<[T]>` allocation is one allocation regardless of capacity).
- Borrow form (`&'a [CanonicalAttrs]`) was correctly rejected in OQ-3 because it defeats the page-shared Arc-cache discipline (every banner candidate would re-borrow the live `Vec`, and on the next `add_portion` push to that Vec the borrow invariants break for any retained ctx).

**Send + Sync preservation.** `Arc<Box<[CanonicalAttrs]>>` is `Send + Sync` iff `CanonicalAttrs: Send + Sync`, which is asserted today (`crates/ism/tests/send_sync.rs::assert_impl_all!(CanonicalAttrs: Send, Sync)`). That assertion **MUST survive PR 6c** (the migration removes `PageContext` from that file but `CanonicalAttrs` stays — and is now the load-bearing axiom). Risk #3 expands.

**`BatchEngine` impact.** `BatchEngine` dispatches per-document work to `tokio::task::spawn_blocking` — `Send` is required on the moved closures. The closure captures `Arc<dyn Rule<CapcoScheme>>` (already `Send + Sync` per `marque-rules::tests/send_sync.rs`) and `Arc<RuleSet<...>>`. `RuleContext` itself is constructed **inside** the spawn_blocking closure per candidate — it never crosses a task boundary as a value. So `RuleContext: Send + Sync` is not strictly required by today's engine; nonetheless, making the field type `Arc<Box<[CanonicalAttrs]>>` keeps the door open for cross-task `RuleContext` passing without a refactor.

---

## 3. `assert_impl_all!` / compile-time checks 🟡 medium

Exhaustive inventory of compile-time pins touching `PageContext` / `RuleContext`:

| Site | Pin | Disposition under PR 6c |
|---|---|---|
| `crates/ism/tests/send_sync.rs::assert_impl_all!(PageContext: Send, Sync)` | LOAD-BEARING for today | **REMOVE** in commit 4 (struct deletion); architect plan §6 covers |
| `crates/ism/tests/send_sync.rs::assert_impl_all!(CanonicalAttrs: Send, Sync)` | LOAD-BEARING for today AND post-6c (the new field type is `Arc<Box<[CanonicalAttrs]>>` so this is the axiom) | **KEEP**; add a doc-comment naming PR 6c as the new dependent if PM wants belt-and-suspenders |
| `crates/rules/tests/send_sync.rs::assert_impl_all!(Box<dyn Rule<StubScheme>>: Send, Sync)` (+ 3 sibling) | Trait-object form for `Rule` / `RuleSet` — independent of `RuleContext` field shape | **KEEP unchanged** |

**Gap the architect did not call out.** There is **no compile-time pin on `RuleContext: Send + Sync` today.** The architect plan §6 commit 4 says "Verify `RuleContext: Send + Sync` has a compile-time assert elsewhere (add if not present)." It is not present anywhere; `RuleContext<'a>` cannot use `assert_impl_all!` directly because the macro requires `'static` types. The proven pattern: add a `for<'a>` HRTB wrapper or a separate function bound check:

```rust
fn _rule_context_is_send_sync<'a>()
where
    marque_rules::RuleContext<'a>: Send + Sync,
{}
```

Add this in commit 4 to `crates/rules/tests/send_sync.rs`. Recommendation: pin `RuleContext` here, alongside the existing `Rule`/`RuleSet` pins; the file already imports the right machinery and lives in the right crate.

**No `const _: () = ` blocks mention `PageContext` or `RuleContext`** — `rg "const _:" crates/ism crates/rules crates/engine` returns zero relevant hits.

---

## 4. Test-fixture migration 🟢 low

Exhaustive `PageContext::new()` / `PageContext {` construction sites (verified via grep):

| File | Sites | Migration shape |
|---|---|---|
| `crates/ism/src/page_context.rs::shim_tests` | 6 (`new()`, `default()`, `add_portion` chains) | **DELETED with the file** in commit 4 — the shim_tests are tests OF `PageContext`; they evaporate with the struct |
| `crates/capco/tests/rules_us1.rs` | 2 (`PageContext::new()` at engine-driver loop) | `Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY)` literal + reuse-on-reset pattern, OR `let mut p: Vec<CanonicalAttrs> = Vec::new(); p.clear()` on PageBreak |
| `crates/capco/tests/s004_audit_content_ignorance.rs` | 2 (same shape as `rules_us1.rs`) | same |
| `crates/engine/benches/profile_project.rs` | 3 (`PageContext::new()` for accumulator-shaped benches) | `Vec::with_capacity(8)` literal; bench will run faster (or noise-band identical), not slower |

The architect plan mentions `tests/fr048_bare_nato_rel_to.rs` in §6 commit 2 — **that file does NOT construct `PageContext`**, only references it in a doc comment. Architect drift; not a blocker. Strike from the commit 2 task list.

**Total: 7 construction sites across 3 files** (post-commit-4 deletion of `page_context.rs`).

---

## 5. Constitution II `secrecy` / `zeroize` 🟢 low

`PageContext.portions: Vec<CanonicalAttrs>` is **not** wrapped in `SecretBox` / `Zeroizing` today. `CanonicalAttrs` is `Box<[T]>` + `Option<Box<str>>` + plain enums; no `SecretBox` fields.

Constitution II requires content-bearing buffers Marque **owns** to wipe on drop. `CanonicalAttrs.derived_from: Option<Box<str>>` and `classified_by: Option<Box<str>>` are the only `Box<str>` fields, and they are **CAB-derived free-text strings** — content-adjacent but not document body. They are not wiped today.

PR 6c migrates the container from `Vec<CanonicalAttrs>` (inside `PageContext`) to `Vec<CanonicalAttrs>` (inside `Engine::lint_inner`) to `Box<[CanonicalAttrs]>` (inside `Arc` on `RuleContext`). **Same content, same drop semantics, same wipe footprint** — neither the buffer nor its lifetime crosses a Constitution II boundary that changes.

**No action.** Constitution II hardening of `derived_from` / `classified_by` is a separate concern, deferred (it would require an audit of every read site for `expose_secret()` discipline). PR 6c is structurally orthogonal.

---

## 6. Constitution V G13 content-ignorance 🟢 low

`PageContext` does **not** impl `Display`. It derives `Debug` only. `CanonicalAttrs` does the same.

The audit emitter does **not** route `PageContext` or `CanonicalAttrs` through `Debug` in production paths — I verified by greping for `Debug::fmt` / `{:?}` on these types in `marque-engine` audit code: no hits in non-test code.

Post-PR-6c the field type changes from `Option<Arc<PageContext>>` (Debug-derived) to `Option<Arc<Box<[CanonicalAttrs]>>>` (slice debug). `Box<[T]>` auto-derives `Debug` and prints `[T; T; T]` — the **same content footprint** as `PageContext { portions: [T; T; T] }` minus the wrapping struct name. G13 risk is structurally identical.

The single load-bearing G13 site is `check_portions_unchanged`'s error message (`engine.rs::check_portions_unchanged`), which is **already content-ignorant** (counts only — `sentinel_tests::check_portions_unchanged_error_message_is_g13_compliant` pins this). Signature migration from `&PageContext` → `&[CanonicalAttrs]` does not change the error-message body.

**No action.**

---

## 7. WASM impact 🟢 low

Grep confirms `marque-wasm` references `PageContext` **only in comments**, not in code (`crates/wasm/src/lib.rs` lines mentioning `PageContext` are all `//`-prefix). The WASM target builds `Vec<CapcoMarking>` directly through the scheme's surface (`compute_banner_native`).

Constitution III §3 "WASM target MUST NOT accept runtime configuration that expands the engine's semantic surface" does not bite — PR 6c is pure structural type churn, no semantic surface change, no new recognizer codepath.

The architect plan §4 "**No change** … `marque-wasm`" is correct. **No action.**

---

## 8. Send/Sync regression risk 🟡 medium

Covered partially in Risk #3. The specific scenario:

- Today: `assert_impl_all!(PageContext: Send, Sync)` exists. The migration **removes** it (struct gone).
- Today: `assert_impl_all!(CanonicalAttrs: Send, Sync)` exists and stays.
- Today: no pin on `RuleContext: Send + Sync`.
- Today: pins on `Box<dyn Rule<S>>: Send + Sync` and `Box<dyn RuleSet<S>>: Send + Sync` exist.

Post-PR-6c the new field type `Option<Arc<Box<[CanonicalAttrs]>>>` is `Send + Sync` iff `CanonicalAttrs: Send + Sync` (`Arc<T>: Send + Sync` iff `T: Send + Sync`; `Box<[T]>: Send + Sync` iff `T: Send + Sync`). The existing `CanonicalAttrs` pin is what makes the new field shape sound. **Pin survives; sound.**

**Recommendation (preflight finding the architect missed):** add the HRTB `RuleContext: Send + Sync` check named in Risk #3 in commit 4. Cost: 3 lines, no runtime impact, closes the gap the architect plan §6 commit 4 acknowledges but does not fully prescribe.

---

## 9. Bench gate — `lint_10kb` 🔴 high (prediction: marginal-fail on first run)

Per project memory `project_bench_baseline_staleness`:

- `lint_10kb` baseline: **828µs**
- 10% threshold: **911µs**
- Current real-world measurements: **880-930µs** — straddles the threshold.

PR 6c is structurally similar to PR 4b-F (signature-only `&PageContext` → `&[CanonicalAttrs]` parameter shuffling, no semantic change). PR 4b-F merged 2026-05-18; its bench measurements landed within tolerance after a `gh run rerun <id> --failed`.

**Prediction:** marginal-fail on first CI run with ~50/50 probability. The `Arc<PageContext>::clone()` → `Arc<Box<[CanonicalAttrs]>>::clone()` swap **may even be faster** (one `to_vec().into_boxed_slice()` is the same cost as the manual `PageContext::clone()` impl which already does `Vec::with_capacity(cap)` + `extend`).

**Playbook (codified in `docs/plans/2026-05-15-pr4b-B-lattice-impls-rest-plan.md` §10):**

1. First CI failure: `gh run rerun <id> --failed` (one re-run is standard mitigation for noise-band PRs).
2. If persistent fail after one re-run AND no other bench regresses: bench-baseline-refresh PR (separate, not PR 6c) lands first.
3. **Do not** attempt to "optimize" PR 6c to pass the gate. The PR is structural-only; semantics-preserving.

Per project memory `project_perf_baseline_pr5_trigger`, baselines may need an analyst pass at end of PR 5 if they haven't naturally fallen back. PR 6c lands in the same window; if the second re-run fails, escalate to PM, do not paper over.

---

## 10. Citations 🟢 low

Architect plan §5 commits to **zero new `§X.Y pNN`** in PR 6c. Verified scan:

- Architect plan body: zero `§X.Y pNN` citations introduced.
- Existing PageContext doc comments propagated into the deleted file: zero `§` citations (the module doc cites `MarkingType::PageBreak` and Constitution VI, both internal refs, no CAPCO §-cites).
- The four commits' anticipated diffs: zero anticipated `§X.Y pNN` adds.

**Constitution VIII gate:** the implementer should `git diff --diff-filter=A -- '*.rs' | rg '§[A-Z]\.[0-9]+ p[0-9]+'` on each commit; expect zero hits. Any hit must be justified or removed.

---

## 11. Clippy nightly-vs-stable drift 🟡 medium

Per project memory `feedback_clippy_nightly_vs_stable_drift`: local clippy is nightly, CI is stable. Some lints fire on stable but not local (e.g., `clippy::const_is_empty`).

**Specific risk for PR 6c:** the migration introduces `Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY)` literal at the engine accumulator site. Stable clippy may emit `clippy::useless_vec` if a downstream consumer-side allocation rewrites the path; unlikely but worth a `cargo +stable clippy --workspace --all-targets -- -D warnings` local sanity check before each push.

**Implementer must-do (Risk #12 below):** `cargo +stable clippy --workspace --all-targets -- -D warnings` after each commit, not just `cargo clippy`.

---

## 12. Pre-implementation must-do checklist

For the implementer (chronological, per-commit):

- [ ] **Commit 0 (preflight):** read CAPCO-CONTEXT.md + architect strategic plan + this register + Constitution Principles II, V, VI, VII, VIII.
- [ ] **Commit 1 (`marque-rules`):** add `page_portions: Option<Arc<Box<[CanonicalAttrs]>>>` field + `with_page_portions` setter. **Do NOT** delete the old `page_context` field yet — both coexist this commit so commit 1 lands tree-green. `cargo +stable clippy -p marque-rules -- -D warnings` + `cargo test -p marque-rules` green.
- [ ] **Commit 2 (`marque-capco`):** migrate S005 + W004 read sites; rename `CapcoScheme::project_from_page_context` body to accept `&[CanonicalAttrs]` (rename to `project_from_attrs_slice`); migrate 2 test fixtures (`rules_us1.rs`, `s004_audit_content_ignorance.rs` — **NOT** `fr048_bare_nato_rel_to.rs`, which doesn't construct `PageContext`). `cargo +stable clippy -p marque-capco -- -D warnings` + `cargo test -p marque-capco` green; `tests/lattice_vs_scheme_parity.rs` green.
- [ ] **Commit 3 (`marque-engine`):** accumulator inlined to `Vec<CanonicalAttrs>` (pre-sized to const `DEFAULT_PORTIONS_CAPACITY = 8`); `dispatch_page_finalization` + `project_page_marking` take `&[CanonicalAttrs]`; freeze-to-`Arc<Box<[CanonicalAttrs]>>` at first banner/CAB use; `check_portions_unchanged` re-targets; 2 engine tests rename (`page_context_resets_observably_across_form_feed`, `page_context_lint_starts_fresh_on_each_call`); `benches/profile_project.rs` migrates (3 sites). Old `page_context` field on `RuleContext` deleted at this commit's end. **Delete `with_page_context` setter** from `marque-rules` (commit 3 touches `marque-rules` again for the dual-field removal — this is fine; the architect plan §6 commit 1's "Engine still constructs `PageContext`" path becomes "engine no longer constructs `PageContext`" here). `cargo +stable clippy -p marque-engine -- -D warnings` + `cargo test -p marque-engine` green.
- [ ] **Commit 4 (`marque-ism` + send_sync fixup):** delete `crates/ism/src/page_context.rs`; drop `pub use page_context::PageContext` from `lib.rs`; drop `assert_impl_all!(PageContext: Send, Sync)` from `tests/send_sync.rs`; **ADD** an HRTB `RuleContext: Send + Sync` check in `crates/rules/tests/send_sync.rs` (per Risk #3 / #8). Re-grep `PageContext` workspace-wide; clean any remaining doc-comment drift in `marque-engine` / `marque-capco` / `marque-wasm` comments. `cargo +stable clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` green.
- [ ] **Final Constitution VIII gate:** `git diff staging..HEAD -- '*.rs' '*.md' | rg '§[A-Z]\.[0-9]+ p[0-9]+'` → expect zero new hits. Any hit: justify in-line or remove.
- [ ] **CI:** expect `lint_10kb` to be marginal — if it fails, `gh run rerun <id> --failed` once. If persistent, STOP and escalate (do not optimize PR 6c to pass).
- [ ] **PR description:** cite within-006 engine-crate-touch precedent (PR 4b-B Commit 2 / 4b-C Commit 5 / 4b-D.2 / 4b-D.3 / 4b-E / 4b-F) explicitly per architect §3 OQ-7.

---

## Architect-amendment-worthy findings

1. **`tests/fr048_bare_nato_rel_to.rs` in §6 commit 2 task list** — that file does not construct `PageContext`. Strike it from the commit's per-file migration list. (Risk #4)
2. **`RuleContext: Send + Sync` compile-time pin does not exist today** — architect §6 commit 4 says "verify, add if not present"; the answer is "not present" and the HRTB form is needed. Risk #3 prescribes the exact 3-line addition. (Risk #3 / #8)
3. **Bench-gate playbook** — architect plan does not document the `gh run rerun <id> --failed` mitigation for the stale `lint_10kb` baseline. Risk #9 codifies it. Worth a one-liner in §7 implementer checklist.

End.
