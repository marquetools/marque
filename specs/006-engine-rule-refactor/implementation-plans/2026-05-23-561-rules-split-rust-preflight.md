<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0 -->

# Issue #561 — `rules.rs` Split Rust-Specific Preflight

**Branch**: `worktree-561-rules-rs-split` | **HEAD**: 8ef8b457
**Purpose**: Flag Rust gotchas before implementer is dispatched; feeds back into the architect plan.
**Scope**: `crates/capco/src/rules.rs` (11,702 lines) → `crates/capco/src/rules/` submodule

---

## 1. `pub(super)` Sufficiency for Rule Structs

**Verdict**: `pub(super)` is sufficient for all rule structs in `registry.rs`.

- All rule structs are currently file-private. After split, `pub(super) struct FooRule;` makes each
  struct visible to the `rules` module, including `registry.rs` (a sibling under `rules/`).
- **Lattice precedent confirmed**: `crates/capco/src/lattice/sci.rs` uses `use super::helpers::{...}`
  to access `pub(super)` items from `helpers.rs` — sibling-to-sibling `pub(super)` access is live
  and working.
- `registry.rs` imports: `use super::dissem::DeprecatedDissemRule;` (etc.) — no visibility bump needed
  beyond `pub(super)`.
- Exception: `BannerMatchesProjectedRule` is already `pub(crate)` at line 5546. Preserve as-is.

## 2. Import Path Changes After Move

**Verdict**: minimal churn. All production code uses `crate::` absolute paths.

Scanning lines 122–136 of rules.rs confirms all top-level imports are `crate::*` absolute paths.
No `super::` or `self::` in non-test production code. After split:

- `crate::rules::CapcoRuleSet` — unchanged (re-exported from `rules/mod.rs`).
- `crate::rules::{FixDiagnosticParams, make_fix_diagnostic}` — unchanged (re-exported from
  `rules/mod.rs` via `pub(crate) use helpers::{...}`).
- `crate::rules_declarative::*` in `registry.rs` (line 151) and `text_handling.rs` (line 1443) —
  these use `crate::` and are unaffected.

**One change required**: `citation_cross_refs_tests` (lines 11621–11702) uses:
```rust
use super::{E005_CROSS_REFS, E037_CROSS_REFS, E038_CROSS_REFS, E039_CROSS_REFS, S003_CROSS_REFS};
```
After split, `super` is `rules/mod.rs` not `rules.rs`. The consts are in separate files.

**Fix**: update the import in `citation_cross_refs_tests` to per-module paths:
```rust
use super::text_handling::E005_CROSS_REFS;
use super::joint::S003_CROSS_REFS;
use super::nodis_exdis::{E037_CROSS_REFS, E038_CROSS_REFS, E039_CROSS_REFS};
```
OR re-export them all through `rules/mod.rs` with `pub(crate) use`. The per-module path form is
preferred — it makes provenance explicit and matches the architect plan's "each const stays adjacent
to its rule" guidance.

## 3. `static S008_SCHEME: LazyLock<CapcoScheme>` Thread Safety

**Verdict**: moves cleanly to `rules/dissem_closure.rs`. No thread-safety concern.

- `std::sync::LazyLock` is `Sync + Send` when `T: Sync` — `CapcoScheme` implements both
  (enforced by the `assert_impl_all!` suite in `crates/capco/tests/`).
- `S008_SCHEME` at line 4495 is referenced only at lines 4507, 4615, 4623 — all within the
  `RelidoImpliedByClosureRule` impl block. Zero external consumers (confirmed via grep).
- Keep file-private. No visibility bump needed.

## 4. Dead Test Block — Critical Finding

**The entire `mod tests` block (lines 8049–11573, ~3,525 lines) is permanently gated by `#[cfg(any())]` at line 8046.**

`#[cfg(any())]` always evaluates false — this block is dead code by construction. `cargo test` never
compiles or runs any of its 171 tests.

Consequences for the split:

- **Do NOT split this block per rule** — it is dead code. Move it as a unit to a `rules/dead_tests.rs`
  or leave it in `rules/mod.rs` behind the same `#[cfg(any())]` gate. Moving it intact is safer
  than splitting into files that will never compile-checked.
- The dead block references `marque_capco_test_support` — a module that no longer exists in
  `Cargo.toml` (retired per PR 4b-E). Splitting it per-file would surface compile errors when/if
  someone removes the `cfg(any())` gate.
- The doc comments at lines 8476–8478, 8784–8785, 10310–10311, 10356–10357, 10435–10436, 10512–10513
  say "assertion in citation_cross_refs_tests (bottom of this file)" — these comments must be
  updated to reflect the new file path.

**The ONLY live test in rules.rs** is `citation_cross_refs_tests` at lines 11621–11702 (82 lines,
5 tests, all `#[test]` under `#[cfg(test)]`).

## 5. Const and Static Name Collision Risk

**Verdict**: no risk. All `*_AUTHORITIES` and `*_CROSS_REFS` consts are already rule-prefixed.

Scanned all `const` and `static` items in rules.rs. Every const name carries its rule prefix
(`E005_CROSS_REFS`, `S003_CROSS_REFS`, `E037_CROSS_REFS`, etc.) or is unambiguously scoped
(`BANNER_CATEGORY_CATALOG` lives only in the banner block). No name appears twice with different
type or different value. No collision risk across the planned files.

## 6. `rules/mod.rs` Re-Export Shape

The following must be present in `rules/mod.rs` for zero breakage:

```rust
// Module declarations
mod banner;
mod dissem;
mod dissem_closure;
mod eyes;
mod fgi;
mod form_mismatch;
mod helpers;
mod joint;
mod nato;
mod nodis_exdis;
mod registry;
mod rel_to;
mod rel_to_suggest;
mod sci;
mod text_handling;

// Public surface (unchanged from pre-split lib.rs → rules.rs chain)
pub use registry::CapcoRuleSet;

// Crate-internal surface required by scheme/actions/companions.rs
pub(crate) use helpers::{FixDiagnosticParams, make_fix_diagnostic, sar_block_span};

// Live citation cross-refs tests
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod citation_cross_refs_tests;
```

Note: `sar_block_span` is confirmed rules-only (used at line 5890 in the SAR banner evaluator,
no scheme consumers) but is already `pub(crate)`. No harm keeping it in the re-export; architectplan
includes it.

## 7. Trait Object Boxing and Concrete Type Visibility

**Verdict**: `pub(super)` is sufficient for all rule structs used in `Box::new(Struct)`.

`Box<dyn Rule<CapcoScheme>>` erases the concrete type at the call site. The only requirement is that
`Box::new(FooRule)` compiles, which requires `FooRule` to be visible at the call site (`registry.rs`)
— which `pub(super)` provides.

The trait bound `Rule<CapcoScheme>: Sized` (for the `Box::new` call) does not require `pub`; it only
requires the type to be nameable. No rule struct needs to be `pub`.

## 8. `LazyLock` / Statics and Test Parallelism

**No concern.** `std::sync::LazyLock` initializes once per process; `cargo test` runs each test file
in a separate process by default. Even with `#[tokio::test]` or multi-threaded test runners, the
`LazyLock` guarantee holds — the initialization closure runs at most once per process regardless of
how many threads race for it.

One `LazyLock` exists in rules.rs: `S008_SCHEME` at line 4495. After it moves to `dissem_closure.rs`,
it is the only global static in that file and requires no special handling.

## 9. `#[cfg(test)]` / `#[cfg(any())]` in the Split

**Two distinct test regions to handle differently:**

**Region A** — `#[cfg(any())]` block (lines 8046–11573): permanently dead. Move as a single unit.
Do not restructure or split by rule. Preserve the gate. Keep the dead references to
`marque_capco_test_support` undisturbed.

**Region B** — `citation_cross_refs_tests` (lines 11621–11702): live. Move to
`rules/citation_cross_refs_tests.rs` (or inline in `rules/mod.rs`). Update the `use super::{...}`
import as described in §2.

**`#[allow(dead_code)]` must travel with its items:**

The following `pub(crate)` consts each carry `#[allow(dead_code)]` because their only consumer is
inside Region A (the dead test block). The attributes suppress the Rust dead-code lint:

- `dedup_country_codes` (line 2816): `#[allow(dead_code)]` at line 2815
- `S003_CROSS_REFS` (line 1710): `#[allow(dead_code)]` at line 1703-block
- `E037_CROSS_REFS` (line 5362): `#[allow(dead_code)]` at line 5360
- `E038_CROSS_REFS` (line 5381): `#[allow(dead_code)]` at line 5379
- `E039_CROSS_REFS` (line 5402): `#[allow(dead_code)]` at line 5400

When these move to their respective submodules, the `#[allow(dead_code)]` must be present on the
moved item. Without it, the dead-code lint fires and `cargo clippy -- -D warnings` will fail.

Note: `E005_CROSS_REFS` does NOT carry `#[allow(dead_code)]` — it is consumed by the live
`citation_cross_refs_tests` block. No attribute needed.

## 10. Coverage (`cargo-llvm-cov`) Impact

**No impact.** `cargo-llvm-cov` tracks coverage at source file + line granularity after the fact;
renaming/moving files changes the report's file-path column but does not break coverage data
collection or the `--fail-under-lines 80` gate.

The `#[cfg_attr(coverage_nightly, coverage(off))]` attribute at line 11622 travels with the
`citation_cross_refs_tests` module when moved. If the dead `cfg(any())` block carries any
`coverage(off)` markers (it does not — scanning confirmed none), they would move with the block.

## 11. CI Lint Tool Compatibility

**All three AST-scan lints automatically cover the `rules/` subdirectory. No path changes needed.**

**citation-lint** (`tools/citation-lint/src/scanner.rs`, lines 115–143):
Uses `WalkDir::new(&src_dir)` where `src_dir = crates/*/src`. WalkDir recurses into subdirectories.
`rules/` is a subdir of `capco/src/` — covered automatically. No path filters to update.

**promote-callsite-lint** (`.github/workflows/ci.yml`, line 387):
Scans for `__engine_promote`. No such call in rules.rs. No impact.

**masking-pin-lint** (`.github/workflows/ci.yml`, line 353):
Scans for `with_recognizer` call sites. No such pattern in rules.rs. No impact.

**audit-cleanup-check.sh**:
Checks for `crates/audit-reader/` and `marque_engine::reader::*`. Completely unaffected.

## 12. Additional Gotchas Not in the Architect Plan

**`BannerCategoryRow` must become `pub(super)` in `banner/mod.rs`.**

`BannerCategoryRow` is currently a private struct (no `pub`, line 5689). It appears in fn signatures
of all five `evaluate_*` functions (`row: &BannerCategoryRow` parameter). After split, these
functions live in `banner/eval_*.rs` (child modules of `banner/`). The struct must be `pub(super)` in
`banner/mod.rs` for child modules to name it in fn signatures.

Without this change: `fn evaluate_sar_banner_rollup(..., row: &BannerCategoryRow)` in
`banner/eval_sar.rs` fails to compile — `BannerCategoryRow` is private to `banner/mod.rs`.

**`sci_system_text` and `render_sci_block` are rules-only — confirmed.**

Grep result: no consumers outside `rules.rs`. Specifically, no calls in
`crates/capco/src/scheme/`, `crates/capco/src/render/`, or any external crate. These helpers belong
in `rules/sci.rs` with `pub(super)` visibility for the `banner/eval_sci.rs` submodule.

Access path: `banner/eval_sci.rs` is a grandchild of `rules/`. It needs `render_sci_block` from
`rules/sci.rs`. Access: `use super::super::sci::render_sci_block;` (or equivalently
`use crate::rules::sci::render_sci_block` if `sci` module is private). The architect plan's
`pub(super)` in `sci.rs` is visible to `rules/banner/eval_sci.rs` because `pub(super)` on an item
in `rules::sci` makes it visible to `rules` and all submodules of `rules` — including grandchildren.
Confirmed pattern: `lattice/sci.rs` accesses `pub(super)` items from `lattice/helpers.rs` as
`use super::helpers::Item`.

**`BANNER_CATEGORY_CATALOG` fn-pointer items need `pub(super)` in their eval files.**

The const at `banner/mod.rs` will reference `eval_sar::evaluate_sar_banner_rollup` etc. by value.
These functions must be `pub(super)` in their `eval_*.rs` files, not file-private. Otherwise the
const initializer in `banner/mod.rs` cannot name them.

**SPDX headers required on every new file.**

Every new `.rs` file in the split must carry:
```
// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
```
See `crates/capco/src/lattice/sci.rs` lines 1–3 as the canonical form. The REUSE tooling check in
CI will fail for any file missing this header.

**Implementer sequencing recommendation.**

Move and `cargo check -p marque-capco` after each file. The order that minimizes dangling-reference
errors:

1. `helpers.rs` first (most-depended-upon within rules)
2. `banner/mod.rs` + `banner/eval_*.rs` together (BANNER_CATEGORY_CATALOG + all eval_* fn pointers)
3. Domain rule files in any order (they only depend on helpers)
4. `registry.rs` last (depends on all rule structs from other files)
5. `mod.rs` shell: add declarations + re-exports incrementally

---

*Preflight complete. All findings are read-only analysis; no source files modified.*
