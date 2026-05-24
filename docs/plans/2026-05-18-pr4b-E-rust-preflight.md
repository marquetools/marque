<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0 -->

# PR 4b-E Rust Preflight — PageContext expected_*/renderer deletion

**Date:** 2026-05-18  
**Reviewer role:** Read-only risk/sanity report  
**Branch:** `refactor-006-pr-4b-d-0-closure-rule-generic` (post-PR-4b-D.3)

---

## 1. Workspace Dependency Graph

### Symbol map for slated-for-deletion items

The following `pub fn` items in `crates/ism/src/page_context.rs` are slated for deletion. Their crate-level re-export path is `marque_ism::<name>` (via `pub use page_context::…` in `crates/ism/src/lib.rs:56`).

| Method / Free Fn | Home module | Cross-crate callers found? |
|---|---|---|
| `expected_classification` | `page_context.rs` | Yes — `crates/capco/tests/page_context_lattice_parity.rs:54`, `crates/capco/tests/scheme_equivalence.rs:55` |
| `expected_sci_controls` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:55`, `scheme_equivalence.rs:80` |
| `expected_sci_markings` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:56`, `scheme_equivalence.rs:1261` |
| `expected_sar_marking` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:57` |
| `expected_aea_markings` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:58` |
| `expected_fgi_marker` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:59` |
| `expected_dissem_us` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:60`, `scheme_equivalence.rs:205` |
| `expected_dissem_nato` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:61` |
| `expected_rel_to` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:62`, `scheme_equivalence.rs:121`, `tetragraph_consolidation.rs:220`, `pattern_a_nodis_exdis_page_context_alignment.rs` (multiple), `s005_pagefinalization.rs:235` |
| `expected_display_only` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:73`, `scheme_equivalence.rs:166` |
| `expected_declassify_on` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:63` |
| `expected_declass_exemption` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:64`, **`crates/wasm/src/lib.rs:1296`** |
| `expected_non_ic_dissem` | `page_context.rs` | Yes — `page_context_lattice_parity.rs:65`, `crates/capco/src/scheme/actions/page_context.rs:38` |
| `is_classified` | `page_context.rs` | Yes — **`crates/wasm/src/lib.rs:1277`** |
| `is_solely_nato_classified` | `page_context.rs` | Yes (but S007 migrated to `ProjectedMarking::is_solely_nato_classified` in PR 4b-D.3; legacy call at `rules.rs:3651` is now on `page_marking`, NOT on `page_context`) |
| `render_expected_banner` | `page_context.rs` | Yes — **`crates/wasm/src/lib.rs:1181`** (in `compute_banner_native`) |
| `project` | `page_context.rs` | Yes — **`crates/capco/src/rules.rs:9105`** (test driver) |
| `expand_tetragraph` (private fn) | `page_context.rs` | No external callers (private `fn`); delegates to `crate::lookup_tetragraph_members` |
| `page_context_to_attrs` | `crates/capco/src/scheme/actions/page_context.rs` | No external callers; `#[allow(dead_code)]` already applied; PR 4b-D.2 retired all production call sites. |

### `sar_sort_key` exposure path — confirmed correct

`sar_sort_key` is `pub fn` in `page_context.rs` and re-exported from `crates/ism/src/lib.rs:56` as `pub use page_context::{PageContext, sar_sort_key}`. Callers in `crates/capco/src/lattice.rs` reach it as `marque_ism::sar_sort_key(...)`. In `crates/capco/src/rules.rs`, it is imported at the top-level (`use marque_ism::sar_sort_key`) and used at rules.rs:4251, 8454-8470. Post-deletion of `expected_*`, the re-export line becomes `pub use page_context::{PageContext, sar_sort_key}` — unchanged. **No module-path issue; `sar_sort_key` survives as a free function in `page_context.rs` and stays re-exported.** No downgrade of `pub` → `pub(crate)` is required.

### Visibility-downgrade analysis

After deleting `expected_*`, `is_classified`, `is_solely_nato_classified`, `render_expected_banner`, and `project`, the only surviving public surface on `PageContext` (other than `sar_sort_key`) is: `new`, `Default`, `Clone`, `add_portion`, `portion_count`, `is_empty`, and `portions`. No previously-public-to-workspace symbol needs a `pub` → `pub(crate)` downgrade that would break current callers.

---

## 2. Hidden Caller Graph

### Benchmarks (`crates/engine/benches/`)

**`profile_project.rs`** — constructs `PageContext::new()` (lines 178, 213, 232), calls `page_context.add_portion(...)`, and calls `scheme.project_from_page_context(&page_context)`. Also calls `scheme.project(Scope::Page, ...)` directly. The `project_from_page_context` fast-path survives PR 4b-E (it is on `CapcoScheme`, not on `PageContext`). The `PageContext::new()` + `add_portion` + `is_empty` + `portions` surface is in the retained shape — no issue. However, `profile_project.rs:10` doc comment says "the engine fast-path `project_from_page_context + from_canonical`" — the bench does NOT call any `expected_*` method directly, so no action needed.

**`lint_latency.rs`** — references `PageContext` only in comments (lines 33, 43, 368, 373, 376, 408, 436). No live calls to `expected_*`. Safe.

### Fuzz targets

**`crates/engine/fuzz/fuzz_targets/lint.rs`** — no calls to `PageContext::expected_*`, `is_classified`, or `render_expected_banner`. The fuzz harness goes through `Engine::lint` which manages `PageContext` internally. Safe.

### Examples directories

No `examples/` directories found under `crates/` with `.rs` files that reference `PageContext::expected_*`.

### Doc tests in `///` comments in `page_context.rs`

No doc-test code blocks (` ```rust `) were found inside `///` doc comments in `crates/ism/src/page_context.rs`. `cargo test --doc -p marque-ism` will not fail due to deleted methods.

### Server (`crates/server/src/`)

No references to `PageContext::expected_*`, `is_classified`, or `render_expected_banner` found. The server goes through `Engine`. Safe.

### Extract (`crates/extract/src/`)

No references. Safe.

### Tools (`tools/`)

No references to `PageContext::expected_*`, `is_classified`, or `render_expected_banner` in any tool source. Safe.

### Tests at workspace root (`tests/`)

No workspace-root `tests/` directory found. Safe.

### CRITICAL WASM CALLERS — require implementer action

Two live WASM call sites in `crates/wasm/src/lib.rs` invoke methods slated for deletion:

1. **`compute_banner_native` at lib.rs:1180–1182**: calls `page_context.render_expected_banner()`. This function scans text for portions, accumulates `PageContext`, and returns the rendered banner string. The replacement strategy (noted in the doc comment at lib.rs:1149) is to switch to `CapcoScheme::project(Scope::Page, ...)` + a renderer. The doc comment at lib.rs:1168–1176 also notes the `from_parsed_unchecked` transitional adapter call; PR 4b-E must address both.

2. **`generate_cab_native` at lib.rs:1277**: calls `page_context.is_classified()` to gate CAB generation. The PR 4b-D.3 doc comment at lib.rs:1283–1291 already anticipates this: "PR 4b-E will either (a) inline the per-portion accumulator here, or (b) introduce a separate `CabProjection` type for CAB-only roll-up." The `expected_declass_exemption()` call at lib.rs:1296 is a second slated-for-deletion method in this same WASM function.

**These two WASM functions collectively consume `render_expected_banner`, `is_classified`, and `expected_declass_exemption`. The implementer must migrate all three before PR 4b-E can compile.**

### CRITICAL TEST DRIVER CALLER — requires action

**`crates/capco/src/rules.rs:9105`**: the test driver calls `page_context.project()` inside a `#[cfg(test)]` block. This is the only caller of `PageContext::project()` other than `page_context.rs` itself. The test driver must be migrated to use `project_page_marking(&scheme, &page_context)` (the engine fast-path helper) or `scheme.project(Scope::Page, ...)` with a `from_canonical` bridge. This is a test-only caller, so the migration does not affect production behavior.

### Parity gate test file — bulk caller

**`crates/capco/tests/page_context_lattice_parity.rs`**: the `project_via_page_context` helper at line 48–79 calls every `expected_*` method directly. This entire helper function is slated for deletion along with the parity-gate direction inversion per CAPCO-CONTEXT.md §3. Confirmed: PR 4b-E retires the divergent side (`project_via_page_context`) and the 3 remaining active-divergence fixtures. The `project_via_scheme` helper survives as the production path.

### `crates/capco/tests/scheme_equivalence.rs`

Calls `ctx.expected_classification()`, `ctx.expected_sci_controls()`, `ctx.expected_rel_to()`, `ctx.expected_dissem_us()`, `ctx.expected_display_only()` — at lines 55, 80, 121, 166, 202–220, 1261. These tests verify equivalence between the two projection paths; after PR 4b-E retires the `expected_*` path, these tests either get deleted or retargeted to verify `scheme.project(Scope::Page, ...)` output directly.

### `crates/capco/tests/pattern_a_nodis_exdis_page_context_alignment.rs`

Multiple calls to `ctx.expected_rel_to()` at lines 162, 166, 205, 209, 255, 259. The file header explains these are "Route B" parity checks alongside `scheme.project`. After PR 4b-E, this dual-route alignment test either gets collapsed to scheme-only or deleted. **This entire test file is a deletee candidate.**

### `crates/capco/tests/tetragraph_consolidation.rs`

Line 220 calls `ctx.expected_rel_to()`. The test is a round-trip parity check for FVEY tetragraph expansion through `expected_rel_to`. Post-deletion, this test must be rewritten against `scheme.project(Scope::Page, ...)`.

---

## 3. Send + Sync Invariants

### Current `PageContext` Send + Sync status

`PageContext` is `Debug` with a manual `Clone` (both confirmed in `page_context.rs:128–188`). Its only field is `portions: Vec<CanonicalAttrs>`. There is NO `Arc<Mutex<_>>`, `Rc<_>`, or `RefCell<_>` in `PageContext` itself.

`CanonicalAttrs` derives `#[derive(Debug, Clone, Default, PartialEq, Eq)]` at `canonical.rs:66`. Its fields are all owned types: `Box<[T]>` slices, `Option<Box<str>>`, `Option<IsmDate>`, `Option<SmolStr>`, etc. `SmolStr` is `Send + Sync`. All `Box<[T]>` fields use generated enum types (all `Copy` or derived-only enums). `CanonicalAttrs: Send + Sync` by structural inference — no interior mutability, no raw pointers, no thread-unsafe field types.

The doc comment on `PageContext` at lines 125–127 states "PageContext is not `Sync` — the engine builds it sequentially during a single document pass." This is a documentation claim, NOT a `!Sync` impl. The actual type IS `Sync` by structural analysis (all fields are `Send + Sync`). The doc comment is architecturally correct guidance about intended usage, not a type-system constraint.

### Post-trim (`{ portions: Box<[CanonicalAttrs]> }`) analysis

Per the PR scope, the trimmed form replaces `Vec<CanonicalAttrs>` with `Box<[CanonicalAttrs]>` for the publicly-read `portions()` field. `Box<[CanonicalAttrs]>` is `Send + Sync` if `CanonicalAttrs: Send + Sync`, which it is. The trimmed `PageContext` remains `Send + Sync`. Custom `Clone` and `Default` would need to be revisited if the field type changes from `Vec` to `Box<[_]>`, but the trimmed form should keep `Vec<CanonicalAttrs>` internally for `add_portion` to work correctly — the `Box<[_]>` snapshot is what `portions()` returns, not the internal storage. Implementer must decide which internal representation to use.

### Static assertions gap

No `static_assertions::assert_impl_all!(PageContext: Send, Sync)` exists anywhere in the workspace. The `crates/rules/tests/send_sync.rs` file pins `Rule` and `RuleSet` trait objects only. **MEDIUM RISK: PR 4b-E should add `assert_impl_all!(PageContext: Send, Sync)` and `assert_impl_all!(CanonicalAttrs: Send, Sync)` in a test or at the engine boundary to lock the thread-safety contract.** The CAPCO-CONTEXT.md §3 does note that the `static_assertions::assert_type_eq_all!` at `crates/engine/tests/content_zeroize.rs` exists for the zeroize contract; a parallel `assert_impl_all!` for `PageContext` belongs in `crates/ism/tests/` or alongside the `RuleContext.page_context: Option<Arc<PageContext>>` field.

---

## 4. Constitution V Principle V G13 (Content-Ignorance) Post-Deletion

### Content-bearing field analysis

After trimming, `PageContext { portions: Vec<CanonicalAttrs> }` still carries content-bearing data: `CanonicalAttrs` holds `classified_by: Option<Box<str>>`, `derived_from: Option<Box<str>>`, and token spans. The `portions` field is populated from document-extracted marking data.

**Constitution II lifecycle check:** `PageContext` does NOT implement `Drop` with a zeroize call. Neither `CanonicalAttrs` nor `Vec<CanonicalAttrs>` uses `zeroize::Zeroizing<_>` or `secrecy::SecretBox<_>`. The current code has no wipe-on-drop for `PageContext`. This was present pre-PR-4b-E; the deletion does not make it worse, but it was already a gap. Constitution II applies here: "content-bearing buffers Marque owns MUST wipe on drop." `PageContext.portions` is Marque-owned content; a `Drop` impl calling `zeroize` on the `Vec<CanonicalAttrs>` buffer is owed. **This is a pre-existing gap, not introduced by PR 4b-E, but PR 4b-E is the refactor that touches `PageContext`'s structure — it is the natural time to address it.** Flag for PM decision: scope this into 4b-E or create a tracking issue.

### Visibility exposure check

No deletion in the `expected_*` removal exposes a previously-hidden content-bearing field. The `portions` field is already exposed via `pub fn portions(&self) -> &[CanonicalAttrs]` and that accessor stays. No `pub` field that was previously non-public gets inadvertently widened.

### `cfg(test)` orphans post-deletion

The `project_via_page_context` helper in `page_context_lattice_parity.rs` is already test-only. After deletion of `expected_*`, the entire `project_via_page_context` function becomes orphaned (no callers in the test file reference it once the fixtures calling it are deleted). The implementer must delete the function, not just its body.

`scheme_equivalence.rs` has an entire test `display_only_parity_with_page_context_expected_display_only` that references `ctx.expected_display_only()`. Post-deletion that test is uncompilable and must be deleted or retargeted.

`pattern_a_nodis_exdis_page_context_alignment.rs` is predominantly test code exercising `PageContext::expected_rel_to()` as "Route B." Post-deletion, most tests in this file collapse. The implementer should delete the file or rewrite the Route-B assertions against `scheme.project(Scope::Page, ...)`.

---

## 5. Constitution VII §IV Engine-Touch Authorization

### Precedent argument

Constitution VII §IV states: "A scheme-adoption PR MUST NOT edit the engine crates (`marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`, `marque-ism`). If the scheme reveals an engine gap, the gap is fixed first in a separate PR."

The within-006 engine-touch precedent established across prior PRs:
- **PR 4b-B** (Commit 2): bugfix-class deletions in `marque-ism` (imperative FOUO + UCNI branches); engine-crate touch authorized as bugfix within the 006 refactor.
- **PR 4b-C** (Commit 5): retired two imperative PageContext branches in `marque-ism::expected_dissem_us` and `expected_aea_markings`; same within-006 bugfix-class authorization.
- **PR 4b-D.2/D.3**: hot-path flip in `marque-engine`; PR-specific authorization.

**PR 4b-E's touch surface:**
- `crates/ism/src/page_context.rs` — deletion of `expected_*` methods, `is_classified`, `is_solely_nato_classified`, `render_expected_banner`, `project`. This is `marque-ism`, not `marque-engine`. Authorized under within-006 precedent.
- `crates/wasm/src/lib.rs` — migration of `compute_banner_native` and `generate_cab_native`. `marque-wasm` is NOT in the Constitution VII §IV restricted set (the restricted set is `marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`, `marque-ism`). WASM is an integration surface; edits here are unrestricted.
- `crates/capco/src/scheme/actions/page_context.rs` — deletion of `page_context_to_attrs`. `marque-capco` is unrestricted.
- `crates/capco/src/rules.rs:9105` — migration of test driver's `page_context.project()` call. Test-only code in `marque-capco`; unrestricted.

**Verdict:** PR 4b-E's touch surface does not exceed the within-006 precedent. The `marque-ism` deletions are the same class as PR 4b-C Commit 5. No `marque-engine` edits are required by the stated PR scope unless the `project_page_marking` fast-path function body (in `engine.rs`) needs updating — but that function calls `CapcoScheme::project_from_page_context`, not `PageContext::project()`, so it is unaffected.

**One caveat:** the parity-gate tests in `crates/capco/tests/page_context_lattice_parity.rs` still reference `PageContext` via `project_via_page_context`. Deleting that helper is within `marque-capco`; no engine-crate touch needed.

---

## 6. `#[non_exhaustive]` / `#[doc(hidden)]` Audit

### `RuleContext` post-deletion shape

`RuleContext` is `#[non_exhaustive]` (confirmed at `crates/rules/src/lib.rs:408`). After PR 4b-E:

- `page_context: Option<Arc<PageContext>>` — **must be RETAINED.** S005 (`analyze_uncertain_reduction`) and W004 (`JointDisunityCollapseRule`) both read `ctx.page_context.as_ref()?.portions()`. The PR scope explicitly states "Trimmed `PageContext { portions: Box<[CanonicalAttrs]> }` exposing `portions()` is retained for S005 and W004." The field stays.
- `page_marking: Option<Arc<ProjectedMarking>>` — retained; S007 reads it.

### `with_page_context` and `with_page_marking` builder methods

`pub fn with_page_context` at `crates/rules/src/lib.rs:572` and `pub fn with_page_marking` at 578. These are `pub` because `RuleContext` is constructed by `marque-engine` (different crate from `marque-rules`). The engine MUST call these builders; making them `pub(crate)` would break the engine's call sites. They must remain `pub`.

**No visibility changes are warranted for these builders.** The `#[non_exhaustive]` attribute already prevents external construction of `RuleContext` directly; the `pub` builders are the intended cross-crate construction API.

### `page_context.project()` on `PageContext` — delete cleanly

`PageContext::project()` (the method that returns `ProjectedMarking`) at `page_context.rs:243` is used in only one location post-PR-4b-D.3: the test driver at `rules.rs:9105`. Post-deletion, the test driver must call `project_page_marking(&scheme, &page_context)` instead. The function signature `pub fn project(&self) -> ProjectedMarking` must be deleted entirely; it is not used on the production engine path (which uses the engine's `project_page_marking` helper wrapping `CapcoScheme::project_from_page_context`).

---

## 7. Cargo Workspace Surface

### Line-count reduction estimate

`crates/ism/src/page_context.rs` is currently 3616 lines. The methods slated for deletion span the majority of the `impl PageContext` block. Rough accounting:
- `expected_classification`: ~10 lines
- `is_solely_nato_classified`: ~15 lines
- `expected_sci_controls`: ~10 lines
- `expected_sci_markings`: ~60 lines
- `expected_sar_marking`: ~65 lines
- `expected_dissem_us`: ~150 lines (heavy logic)
- `expected_dissem_nato`: ~30 lines
- `expected_fgi_marker`: ~30 lines
- `expected_aea_markings`: ~200 lines (heavy logic with UCNI branch comments)
- `expected_rel_to`: ~80 lines
- `expected_display_only`: ~100 lines
- `expected_declassify_on`: ~15 lines
- `expected_declass_exemption`: ~20 lines
- `expected_non_ic_dissem`: ~120 lines
- `is_classified`: ~10 lines
- `render_expected_banner`: ~200 lines (complex renderer)
- `project()`: ~30 lines
- Internal helpers (`expand_tetragraph` as private fn): ~5 lines
- Associated tests (89 `#[test]` functions, many exercising `expected_*`): ~1400 lines

**Estimated reduction: ~2500 lines.** Surviving file would be approximately 1100–1200 lines.

### Module consolidation recommendation

At ~1100–1200 lines, `page_context.rs` remains above the 800-line guidance but within the regime where it is a coherent module (it owns `PageContext`, `DEFAULT_PORTIONS_CAPACITY`, `sar_sort_key`, and the supporting `SystemKey` enum). **Recommendation: keep as `page_context.rs`.** The file's single responsibility — page-level portion accumulation + the `sar_sort_key` sort helper — is cohesive. The surviving internal `SystemKey` enum is a private implementation detail of the now-deleted `expected_sci_markings`; after that method is deleted, `SystemKey` becomes orphaned dead code and must be deleted alongside it.

### `[features]` audit

`crates/ism/Cargo.toml` has no feature flags that gate `PageContext` methods. No feature-flagged tests become orphaned.

---

## 8. Coverage Discipline

### Surviving public surface after PR 4b-E

| Item | Current test coverage in `page_context.rs #[cfg(test)]` | Gap? |
|---|---|---|
| `PageContext::new()` | Covered implicitly by every test that constructs one | No |
| `Default::default()` | Covered via `new()` equivalence | No |
| `Clone` (manual impl) | Covered by `clone_preserves_capacity_at_min_8_portions` and related tests | No |
| `add_portion` | Covered by all accumulator tests | No |
| `portion_count` | Currently implicitly covered; not separately exercised | Gap — needs a dedicated `portion_count_returns_len` test |
| `is_empty` | Explicitly tested (returns `false` after add_portion) | Confirmed covered |
| `portions()` | Used in `S005` and `W004` — should have a unit test asserting the slice contents match what was added | Gap — no dedicated test for `portions()` slice identity |
| `sar_sort_key` free fn | Explicitly tested at `rules.rs:8454–8470` (3 tests) | Covered |

**Summary of coverage gaps on surviving surface:**
- `portion_count` — no dedicated test; add `fn portion_count_reflects_add_portion_calls`.
- `portions()` slice identity — no dedicated test; add `fn portions_returns_added_attrs_in_order`.

These are small additions. The implementer should add them as part of the PR.

---

## 9. Walk-Adjacent-Paths Discipline

The following is a complete grep-inventory of every doc-comment, error-message string, plan file, README, and module-level comment that names a method slated for deletion by string.

### In `docs/plans/`

- `docs/plans/2026-05-15-pr4b-B-lattice-impls-rest-plan.md`: mentions `page_context_to_attrs` at lines 94, 166, 676. These are historical notes; no implementer action required but the plan doc does describe `page_context_to_attrs` as a live function — it should not be updated (plans are immutable records).
- `docs/plans/2026-05-12-pr3c-b-8f-engine-gap-nodis-exdis-rel-to-shortcircuit-plan.md`: mentions `render_expected_banner` at lines 25, 53, 140, 180, 244; `page_context_to_attrs` at multiple lines; `expected_rel_to`, `expected_non_ic_dissem` — all historical. No action required; historical plans stay.
- `docs/plans/2026-05-16-pr4b-C-pattern-c-strip-rows-plan.md`: mentions `page_context_to_attrs`, `expected_aea_markings`, `expected_dissem_us`, `render_expected_banner` throughout. Historical plan. No action required.
- `docs/plans/2026-05-02-engine-refactor-consolidated.md`: mentions `page_context_to_attrs`. Historical plan.

### In `crates/capco/README.md`

`crates/capco/README.md` (lines 45, 47) contains live prose referencing `expected_aea_markings` by name: "the pre-PR-4b-C `expected_aea_markings` bug" and "PageContext remains the transitional banner-validation driver until PR 4b-D wires...". After PR 4b-E, the statement "PageContext remains the transitional banner-validation driver" is stale. **Action required: update `crates/capco/README.md` to reflect that PR 4b-E retired the `expected_*` machinery.** This is the CAPCO-2016 CLAUDE.md §"Recent Changes" equivalent for the crate.

### In `crates/scheme/src/page_rewrite.rs`

Line 17 contains: `//! inside `PageContext::expected_rel_to` — means tooling...`. After `expected_rel_to` is deleted, this comment in `page_rewrite.rs` becomes a dangling reference. **Action required: update the comment to reference the scheme-level projection instead.**

### In `crates/scheme/src/builtins.rs`

Line 556: `/// `PageContext::expected_declassify_on`). Bottom is the absent date`. This is a doc comment on `MaxDate`. After `expected_declassify_on` is deleted, the parenthetical is a dangling reference. **Action required: update to reference `DeclassifyOnLattice` or `CapcoScheme::project` instead.**

### In `crates/capco/src/lattice.rs`

Lines 66, 2038, 2054, 2058, 2264, 2400, 2457, 2510, 2533: multiple doc comments reference `PageContext::expected_sci_markings`, `PageContext::expected_dissem_us`, `PageContext::expected_dissem_nato`, `PageContext::expected_fgi_marker`. Post-deletion these are dangling references. **Action required: update affected doc comments in `lattice.rs` to reference `CapcoScheme::project(Scope::Page, ...)` or the lattice types directly.**

### In `crates/capco/src/scheme/constraints/categories.rs`

Lines 96, 112, 129, 157: comments referencing `PageContext::expected_sar_marking`, `PageContext::expected_aea_markings`, `PageContext::expected_fgi_marker`, `PageContext::expected_rel_to()`. Post-deletion, these are dangling comment references. **Action required: update.**

### In `crates/capco/src/scheme/rewrites/pattern_c.rs`

Lines 86, 111: references `PageContext::expected_aea_markings`. **Action required: update.**

### In `crates/capco/src/render/render_declassify.rs`

Lines 16, 23, 32: doc module comment references `PageContext::expected_declassify_on` and `PageContext::render_expected_banner`. **Action required: update.**

### In `crates/ism/src/projected.rs`

Line 95–96: a doc comment on the `dissem_us` field: "`PageContext::expected_dissem_us`" cross-reference. Line 98 similarly for `dissem_nato`. **Action required: update to reference the lattice aggregation path.**

### In `crates/capco/tests/`

- `scheme_equivalence.rs:134–176`: text discusses "`page_context_to_attrs`" and `PageContext::expected_display_only`. These are in test-file comments; they may be deleted with the tests that use them.
- `pattern_a_nodis_exdis_page_context_alignment.rs` module-level comment (lines 9–51): entire file discusses `PageContext::expected_rel_to()` as Route B. **This entire file is a deletee candidate.**
- `tetragraph_consolidation.rs:201`: comment referencing `expected_rel_to`. If the test at line 220 is rewritten, the comment is updated with it.

---

## 10. Clippy / Lint Posture

Running `cargo +stable clippy -p marque-ism --no-deps -- -D warnings` after deletion would surface:

1. **`dead_code` on `SystemKey` enum**: `SystemKey` at `page_context.rs:~1500–1600` is an internal helper type used exclusively by `expected_sci_markings`. After that method is deleted, `SystemKey` becomes unreachable from any live code path. It must be deleted alongside `expected_sci_markings`.

2. **`dead_code` on private `expand_tetragraph` fn**: `expand_tetragraph` at `page_context.rs:1611` is used only by `expected_rel_to` and `expected_display_only`. After those are deleted, `expand_tetragraph` is dead. It must be deleted. (Its canonical counterpart `marque_capco::vocab::expand_tetragraph` is unaffected — it is public and has external callers.)

3. **`dead_code` on `page_context_to_attrs`**: already marked `#[allow(dead_code)]` at `crates/capco/src/scheme/actions/page_context.rs:30`. After PR 4b-E this function has no callers (even the parity-gate reference is to its shape, not the function itself). **Delete the function and remove the `#[allow(dead_code)]` annotation.**

4. **`unused_imports`**: `crates/ism/src/page_context.rs` imports `SarCompartment`, `SarIndicator`, `SarMarking`, `SarProgram`, `SciCompartment`, `SciControl`, `SciControlSystem`, `SciMarking`, `SmolStr`, `FgiMarker`, `NonIcDissem`, `DeclassExemption` etc. Several of these are used ONLY in `expected_*` methods. After deletion, imports for `DeclassExemption`, `SarCompartment`, `SarIndicator`, `SarProgram`, `SciCompartment`, `SmolStr`, possibly `DissemControl`, `NonIcDissem`, and `FgiMarker` may become unused. `cargo +stable clippy` will flag these. The implementer must audit and trim the import block after deleting methods.

5. **`unused_imports` in `crates/capco/src/scheme/actions/page_context.rs`**: after deletion of the file, all its imports are deleted with it. No residual issue.

6. **`unused_imports` in test files**: each test file that called `expected_*` methods imports `marque_ism::PageContext`. After the methods are deleted and the tests rewritten or deleted, stale imports will be flagged. Use `cargo +stable clippy -p marque-capco --tests -- -D warnings` to surface these.

---

## Summary of Highest-Risk Items

| Rank | Item | Severity | Required Before Merge |
|---|---|---|---|
| 1 | **WASM `compute_banner_native` calls `render_expected_banner`** (`lib.rs:1181`) | CRITICAL | Yes — won't compile |
| 2 | **WASM `generate_cab_native` calls `is_classified` and `expected_declass_exemption`** (`lib.rs:1277, 1296`) | CRITICAL | Yes — won't compile; also note the `CabProjection` decision deferred from PR 4b-D.3 must be resolved here |
| 3 | **Test driver `page_context.project()` call** (`rules.rs:9105`) | HIGH | Yes — won't compile in `cfg(test)` |
| 4 | **Dangling doc-comment references in `lattice.rs`, `builtins.rs`, `page_rewrite.rs`, `projected.rs`, `render_declassify.rs`, `categories.rs`, `pattern_c.rs`** | HIGH | Functional (won't cause compile failures), but creates stale citations that violate Constitution VIII's "propagation requires re-verification" principle |
| 5 | **`SystemKey` and private `expand_tetragraph` become dead code** after `expected_sci_markings` and `expected_rel_to` deletion | HIGH | Yes — `cargo +stable clippy -D warnings` will fail |
| 6 | **`crates/capco/README.md` "PageContext remains transitional driver" statement** becomes stale | MEDIUM | Update at same time as PR |
| 7 | **No `assert_impl_all!(PageContext: Send, Sync)` static assertion** | MEDIUM | Add in PR |
| 8 | **Missing `portion_count` and `portions()` unit tests** on surviving surface | MEDIUM | Add in PR |
| 9 | **`page_context_to_attrs` `#[allow(dead_code)]` annotation** — function should be deleted entirely, not suppressed | MEDIUM | Delete, don't just silence |
| 10 | **Constitution II zeroize gap for `PageContext.portions`** — pre-existing, but PR 4b-E is the structural moment to address it | MEDIUM | PM decision: scope in or track |

