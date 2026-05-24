# Issue #561 — `crates/capco/src/rules.rs` Split Plan

**Status**: architect plan, awaiting implementer
**Precedent**: PR #703 (lattice split, 5545 → 17 files)
**Scope**: mechanical refactor, zero behavior change, all CAPCO §-citations byte-identical
**Constraint**: each file ≤800 lines (target 200–400)
**HEAD**: `origin/staging` 8ef8b457 (`rules.rs` = 11,702 lines)

---

## 0. External-Consumer Audit (Decision 7 Up Front)

Greps across `crates/{engine,wasm,capco}/{src,tests}` for `marque_capco::rules::*` and `crate::rules::*`:

- **Single external entry**: `marque_capco::rules::CapcoRuleSet::new()` (consumed by `crates/engine/src/lib.rs:128` only).
- **Single intra-crate cross-module entry**: `crate::rules::{FixDiagnosticParams, make_fix_diagnostic}` consumed by `crates/capco/src/scheme/actions/companions.rs:112` (already `pub(crate)` at `rules.rs:5257` / `5281`).
- All individual rule structs (`MissingUsaTrigraphRule`, `DeclassifyMisplacedRule`, …) are crate-private and only consumed by the registry's `vec![Box::new(...)]` block.

Implication: `mod.rs` only needs `pub use` for `CapcoRuleSet`, `FixDiagnosticParams`, `make_fix_diagnostic`. Rule structs stay private to their submodule and are pulled into `registry.rs` via `use super::{dissem::*, ...}` or per-name imports.

---

## 1. Final Module Layout

`crates/capco/src/rules/` — domain-grouped submodules. Filenames mirror the lattice split's per-axis convention.

| File | Contents | Source lines | Est. file len |
|------|----------|--------------|---------------|
| `mod.rs` | Module declarations, `pub use` re-exports, module-level docs from current rules.rs:1–120 | 1–138 + new | ~150 |
| `registry.rs` | `CapcoRuleSet` struct, `Default` impl, `impl CapcoRuleSet::new()`, `impl RuleSet`, all retirement-history comments | 138–496 | ~360 |
| `helpers.rs` | `FixDiagnosticParams` + `make_fix_diagnostic` + `sar_block_span` + any 2+ consumer helpers; tests for each helper co-located | 5247–5337 + relocated | ~250 |
| `dissem.rs` | E006 (`DeprecatedDissemRule` + `is_dissem_replacement`), W003 (`NonIcInClassifiedBannerRule`) | 975–1121, 2600–2913 | ~470 |
| `rel_to.rs` | E002 (`MissingUsaTrigraphRule`), S009 (`PreferTetragraphCollapseRule`), S010 (`CollapseUniformRelPortionsRule` + `expand_rel_to_atomic`, `check_collapse_uniform_rel_portions`), E072 (`BareRelPortionDivergenceRule` + helper) | 539–850, 4687–5170 | ~750 |
| `rel_to_suggest.rs` | S004 (`RelToTrigraphSuggestRule` + `s004_*` helpers), S005 (`RelToOpaqueUncertainReductionSuggestRule` + `s005_*` helpers + `analyze_uncertain_reduction` + `sar_missing_programs`) | 1945–2328, 2914–3394 | ~760 |
| `sci.rs` | W034 (`SciCustomControlInfoRule`), E061 (`HcsBareAtConfidentialLegacyRemarkRule`), E062 (`HcsBareSuggestSubcompartmentRule`), E063 (`RsvBareRequiresCompartmentRule`), shared `sci_system_text` + `render_sci_block` SCI render helpers | 3395–3832, 5171–5339 | ~770 |
| `eyes.rs` | E064 (`EyesOnlyConvertToRelToRule` + `parse_eyes_trigraphs` + `build_rel_to_replacement`) | 3833–4202 | ~370 |
| `nato.rs` | S007 (`BareNatoRequiresRelToRule` + `build_bare_nato_rel_to_*` helpers), E066 (`LegacyNatoCompoundRemarkRule` + `is_legacy_nato_compound_text`) | 4203–4483, 6720–7011 | ~580 |
| `dissem_closure.rs` | S008 (`RelidoImpliedByClosureRule` + `static S008_SCHEME`) | 4484–4686 | ~210 |
| `joint.rs` | S003 (`JointUsaFirstRule`), W004 (`JointDisunityCollapseRule`) | 1682–1944, 7363–7545 | ~440 |
| `fgi.rs` | FgiOwnershipTrigraphSuggestRule, E071 (`FgiExplicitWithTrigraphRule` + `e071_*` helpers), E073 (`FgiInvalidOwnershipTokenRule` + `is_fgi_invalid_ownership_token`) | 2329–2599, 7546–8046 | ~770 |
| `nodis_exdis.rs` | E039 (`NodisExdisClearsBannerRelToRule`), E041 (`NodisSupersedesExdisInPortionRule` + intent helper) | 5340–5553, 6523–6719 | ~410 |
| `text_handling.rs` | E005 (`DeclassifyMisplacedRule`), E007 (`XShorthandDateRule` + `looks_like_deprecated_x_shorthand` + `is_repeated_sar_owned_by_e030`), E008 (`UnknownTokenRule`), C001 (`CorrectionsMapRule`) | 851–974, 1122–1681 | ~720 |
| `form_mismatch.rs` | PortionFormInBannerRule, BannerFormInPortionRule + shared `find_portion_form_in_banner`, `find_banner_form_in_portion`, `emit_form_mismatch` | 7012–7362 | ~360 |
| `banner/mod.rs` | `BannerMatchesProjectedRule` + `BannerCategoryRow` struct + `BANNER_CATEGORY_CATALOG` const + walker logic | 5546–5811 | ~270 |
| `banner/eval_sar.rs` | `evaluate_sar_banner_rollup` | 5812–5967 | ~160 |
| `banner/eval_sci.rs` | `evaluate_sci_banner_rollup` | 5968–6109 | ~150 |
| `banner/eval_non_ic_dissem.rs` | `evaluate_non_ic_dissem_banner_rollup` | 6110–6257 | ~150 |
| `banner/eval_classification.rs` | `evaluate_classification_banner_rollup` | 6258–6364 | ~110 |
| `banner/eval_fgi_marker.rs` | `evaluate_fgi_marker_banner_rollup` | 6365–6522 | ~160 |
| `rules/citation_cross_refs_tests.rs` | LIVE cross-ref tests (5 `#[test]`); per-module imports per §2 / preflight §2 | 11621–11702 | ~85 |
| **`_disabled_tests.rs`** (sibling of `rules/`, NOT inside it) | The `#[cfg(any())]`-gated dead test block from preflight §4, moved intact | 8046–11573 | ~3,525 (dead) |

**All `rules/` files ≤800.** Tightest post-archaeology + post-dead-block extraction: `sci.rs` ~720 and `rel_to_suggest.rs` ~720. The dead block lives OUTSIDE `rules/` and does not count against the per-file budget — see Decision 2 revision.

**Live test mass remaining inside `rules/`**: only the 5 tests in `citation_cross_refs_tests.rs` (preflight §4 confirmed the 3,525-line `mod tests` block at line 8049 is `#[cfg(any())]`-dead; never compiles, never runs). No per-domain inline tests needed — they don't exist as live code.

---

## 2. Tests Strategy (REVISED — preflight §4 finding)

**Rust-specialist preflight §4 found the giant `mod tests` block at lines 8046–11573 is `#[cfg(any())]`-gated dead code** (~3,525 lines, 171 tests, never compiled, never run since PR 3c.B Commit 10 landed the mvp-2 → mvp-3 audit-schema cutover). The block's top-of-block comment reads `// PR 3c.B Commit 10: inline tests reading legacy FixProposal fields disabled pending rewrite.` — the rewrite never materialized.

**PM decision (2026-05-23)**: park the dead block in a quarantine file OUTSIDE the new `rules/` module tree. Follow-up disposition tracked as **issue #722** (`post-refactor`, `refactor`, `DEBT Payment`).

### 2.a Dead block — destination

**Chosen path**: `crates/capco/src/_disabled_tests.rs` (sibling of `rules/`, NOT inside it).

**Rationale**:
- Underscore-prefix is the universally-recognized "leave-me-alone" filesystem convention; a casual reader of `crates/capco/src/` sees `_disabled_tests.rs` and immediately knows it's not part of the active module surface. `rules_disabled_tests.rs` (the alternative) reads as part of the rules module surface and invites accidental modification.
- Lives outside `rules/` so the `rules/` tree itself remains constitution-compliant (every active file ≤800). The 3,525-line dead block doesn't pretend to comply because it's not in the active tree.
- Declared in `lib.rs` as `#[cfg(any())] mod _disabled_tests;` — the gate stays at the module-declaration site so the file is dead at the include layer (compiler never reads it).

**File top-of-file comment** must include:
```
// Quarantined dead test block from `rules.rs` pre-#561 split.
// `#[cfg(any())]` makes this permanently unreachable; preserved
// for disposition decision in issue #722.
// DO NOT add new tests here. New tests go in
// `crates/capco/tests/` integration files or `mod tests` blocks
// inside their rule's submodule.
```

The block moves **intact** — no per-rule splitting. Preflight §4 noted it references `marque_capco_test_support` (retired in PR 4b-E); splitting would scatter undeadable compile errors across 14 files if anyone later flipped the gate. Keeping it as one unit makes disposition (rewrite, port to integration tests, delete) a single coherent decision.

### 2.b Live tests — `citation_cross_refs_tests` (82 lines, 5 tests)

**Destination**: `crates/capco/src/rules/citation_cross_refs_tests.rs` (its own file under `rules/`, declared in `rules/mod.rs` as `#[cfg(test)] mod citation_cross_refs_tests;`).

**Import migration** (per preflight §2). The current top-of-block import:
```rust
use super::{E005_CROSS_REFS, E037_CROSS_REFS, E038_CROSS_REFS, E039_CROSS_REFS, S003_CROSS_REFS};
```
becomes per-submodule paths:
```rust
use super::text_handling::E005_CROSS_REFS;
use super::joint::S003_CROSS_REFS;
use super::nodis_exdis::{E037_CROSS_REFS, E038_CROSS_REFS, E039_CROSS_REFS};
```
This makes provenance explicit at the import site. Each cited const stays adjacent to its rule (per §3) at `pub(crate)` visibility — already the current visibility, no bump needed.

Preserve `#[cfg_attr(coverage_nightly, coverage(off))]` on the module declaration when moving (preflight §10).

### 2.c No per-rule inline test blocks

Decision: do NOT create per-domain `#[cfg(test)] mod tests` inside each rule submodule. The original architect plan recommended option (a) co-locate, but that recommendation assumed 171 LIVE tests to distribute. Preflight §4 proved those tests are dead. There is nothing to distribute.

Any new test work after the split lands in `crates/capco/tests/` integration files (the existing precedent — see `e071_fgi_explicit_with_trigraph.rs`, `bare_canonical_compound.rs`) or, when the disposition follow-up #722 resurrects the disabled block, into per-rule `mod tests` co-located with the rule.

---

## 3. Shared-Helper Allocation

| Helper | Consumers | Home |
|--------|-----------|------|
| `FixDiagnosticParams`, `make_fix_diagnostic` | E006, E007, E008, C001, banner walker, plus `scheme/actions/companions.rs` (cross-module) | `rules/helpers.rs`, stay `pub(crate)` |
| `sar_block_span` | banner walker + scheme bridges | `rules/helpers.rs`, `pub(crate)` |
| `is_dissem_replacement` | E006 only | `rules/dissem.rs`, file-private |
| `s004_*` (`edit_distance`, `candidate_covered_by_block`, `message`) | S004 only | `rules/rel_to_suggest.rs`, file-private |
| `s005_*` + `analyze_uncertain_reduction` + `sar_missing_programs` | S005 only | `rules/rel_to_suggest.rs`, file-private |
| `sci_system_text`, `render_sci_block` | banner SCI evaluator + sci.rs rules. Preflight §12 grep-confirmed NO scheme/render-crate consumers. | `rules/sci.rs`, `pub(super)`. Access from `rules/banner/eval_sci.rs` (grandchild) via `use crate::rules::sci::render_sci_block;` — preflight §12 confirmed `pub(super)` on a `rules::sci` item is visible to all `rules` submodules including grandchildren. |
| `find_portion_form_in_banner`, `find_banner_form_in_portion`, `emit_form_mismatch` | PortionFormInBanner + BannerFormInPortion only | `rules/form_mismatch.rs`, file-private |
| `e071_*` (5 helpers) | E071 only | `rules/fgi.rs`, file-private |
| `parse_eyes_trigraphs`, `build_rel_to_replacement` | E064 only | `rules/eyes.rs`, file-private |
| `is_fgi_invalid_ownership_token`, `canonicalize_trigraph_list`, `dedup_country_codes` | mixed: rule + scheme (existing `pub(crate)`) | keep `pub(crate)`, place in `rules/helpers.rs` |
| `is_repeated_sar_owned_by_e030`, `looks_like_deprecated_x_shorthand` | E007 only | `rules/text_handling.rs`, file-private |
| Const tables `E005_CROSS_REFS`, `S003_CROSS_REFS`, `E037_CROSS_REFS`, `E038_CROSS_REFS`, `E039_CROSS_REFS` | per-rule `cited_authorities()` returns + the citation cross-ref tests | each stays adjacent to its rule (file-private inside the rule's submodule); `citation_cross_refs_tests` uses `use crate::rules::{rel_to::E002_CROSS_REFS, ...}` — bump these to `pub(super)` or `pub(crate)` as needed. **Default `pub(crate)`** so the cross-refs test module can import them with one `use` group. |

Hard rule for implementer: prefer file-private (`fn foo` with no `pub`) unless the symbol crosses a module boundary. Bump to `pub(super)` for siblings, `pub(crate)` for cross-crate-module, `pub` only if it was already `pub` in source.

---

## 4. `static S008_SCHEME: LazyLock<CapcoScheme>` Placement

Confirmed: only consumer is `RelidoImpliedByClosureRule` (S008) body itself (`rules.rs:4495`). No tests reference it.

Moves with the rule to `rules/dissem_closure.rs`. Stays file-private.

---

## 5. `BANNER_CATEGORY_CATALOG` Block (~833 Lines) — Sub-Sub-Module

This block is over budget by itself. The clean shape: a `rules/banner/` sub-submodule (mirrors `crates/capco/src/lattice/` per-axis split).

```
rules/banner/
├── mod.rs                    (walker + catalog + BannerCategoryRow struct, ~270 lines)
├── eval_sar.rs               (~160 lines)
├── eval_sci.rs               (~150 lines)
├── eval_non_ic_dissem.rs     (~150 lines)
├── eval_classification.rs    (~110 lines)
└── eval_fgi_marker.rs        (~160 lines)
```

`BANNER_CATEGORY_CATALOG` (the `&[BannerCategoryRow]` const) and the `BannerCategoryRow` struct stay in `banner/mod.rs`. **Per preflight §12**:
- `BannerCategoryRow` MUST be `pub(super)` in `banner/mod.rs` (it currently has no `pub`, line 5689). The struct appears in the `row: &BannerCategoryRow` parameter of every `evaluate_*` function; without `pub(super)` the child `eval_*.rs` files won't compile.
- Each `evaluate_*_banner_rollup` function MUST be `pub(super)` in its `eval_*.rs` file so the `BANNER_CATEGORY_CATALOG` const initializer in `banner/mod.rs` can name it as a fn pointer.

Tests for banner walker (substantial — the BANNER_CATEGORY_CATALOG span owns most test mass per the doc comment at lines 11198+) go in `rules/banner/tests.rs` as `#[cfg(test)] mod tests` referenced from `banner/mod.rs`. Tests for individual evaluators co-locate inside each `eval_*.rs` file.

This is the cleaner 5-year-maintenance answer than per-axis files at `rules/` top level — banner roll-up is genuinely one rule with five axis-specialized evaluators, and the catalog row ordering is load-bearing (see PR 3b.A and the post_3b registration pin). Keeping them under `banner/` makes that relationship visible.

---

## 6. Visibility Audit

After the split, `registry.rs` needs the rule structs visible. Two options:

**Recommended**: `rules/<file>.rs` declares `pub(super) struct FooRule;` (visible to siblings); `registry.rs` does `use super::{dissem::DeprecatedDissemRule, dissem::NonIcInClassifiedBannerRule, ...};`.

**Alternative**: each submodule does `pub(super) use self::FooRule;`. More boilerplate, no gain.

Symbols that need wider visibility after move:
- `FixDiagnosticParams`, `make_fix_diagnostic` — keep `pub(crate)` (cross-module consumer in `scheme/actions/companions.rs`).
- `BannerMatchesProjectedRule` — already `pub(crate)` (likely a tests consumer); preserve.
- `canonicalize_trigraph_list`, `dedup_country_codes`, `sar_block_span`, `is_fgi_invalid_ownership_token` — already `pub(crate)`; preserve.
- All `*_CROSS_REFS` consts — already `pub(crate)`; preserve.
- All rule structs (e.g., `MissingUsaTrigraphRule`) — bump from private to `pub(super)`. **No other change.**

Implementer pre-flight: `cargo check -p marque-capco` after each module move surfaces any missing visibility.

---

## 7. Module Re-Export Shape (`rules/mod.rs`)

```rust
// Module declarations (alphabetic, matches lattice precedent)
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

// Public surface — preserved verbatim from pre-split rules.rs
pub use registry::CapcoRuleSet;

// Crate-internal surface — required by scheme/actions/companions.rs
pub(crate) use helpers::{FixDiagnosticParams, make_fix_diagnostic, sar_block_span};

// Citation cross-refs test lives in mod.rs to retain its lines 11621–11702 placement
#[cfg(test)]
mod citation_cross_refs;

// Module-level docs (the rule-ID-history comments at lines 1–120) move here verbatim.
```

The lattice `mod.rs` precedent (`pub use aea::{AeaPrimary, AeaSet, UcniKind}; …`) re-exports concrete types because lattice types are part of the public surface; rules.rs's only public type is `CapcoRuleSet`, so we re-export only that.

---

## 8. Implementation Sequencing — SUPERSEDED

> **Note**: §8 below is the pre-preflight sequencing draft. It has been superseded by **§11.3 + the "Implementer's Checklist — Final Form" at the bottom of this document**. The Final Form integrates Decision 10 archaeology (Stage A), Decision 2.a dead-block quarantine (Stage B), and the preflight §12 ordering correction (banner moves SECOND, not last). Implementer: follow the Final Form checklist; §8 is preserved only for design-history context.

Each checkpoint must end with `cargo check -p marque-capco` passing.

1. **Create directory and skeleton**:
   - `mkdir crates/capco/src/rules` (was a file; rename `rules.rs` → `rules_legacy.rs` first to free the path).
   - `mv crates/capco/src/rules.rs crates/capco/src/rules_legacy.rs`
   - In `crates/capco/src/lib.rs`, change `pub mod rules;` to `pub mod rules; mod rules_legacy;` and add `pub use rules_legacy as rules;` as a temporary alias. **Verify**: `cargo check -p marque-capco` green.
   - Create empty `crates/capco/src/rules/mod.rs` (no contents yet).
   - **Backup discipline**: do the rename in a single commit so the diff stays reviewable.

2. **Move `helpers.rs` first** (smallest, lowest risk, unblocks dependents):
   - Cut `FixDiagnosticParams` + `make_fix_diagnostic` + `sar_block_span` from `rules_legacy.rs` into `rules/helpers.rs`.
   - In `rules/mod.rs`: add `pub(crate) mod helpers; pub(crate) use helpers::*;` (temporary; tighten in step 17).
   - In `rules_legacy.rs`: replace cut block with `use crate::rules::helpers::*;`.
   - `cargo check -p marque-capco` + `cargo test -p marque-capco --no-run`.

3. **Move `dissem.rs`** (E006 + W003): cut both structs + `is_dissem_replacement` + their tests + their `*_CROSS_REFS`. Register import in `rules/mod.rs`. In `rules_legacy.rs::CapcoRuleSet::new()` change `Box::new(DeprecatedDissemRule)` → `Box::new(crate::rules::dissem::DeprecatedDissemRule)` (and same for W003). `cargo check`. `cargo test -p marque-capco`.

4. **Repeat the pattern** for each module in this order (chosen for lowest cross-dependency risk first):
   - `text_handling.rs` (E005, E007, E008, C001 — high test mass, isolated)
   - `joint.rs` (S003, W004)
   - `rel_to.rs` (E002, S009, S010, E072)
   - `rel_to_suggest.rs` (S004, S005)
   - `sci.rs` (W034, E061, E062, E063 + sci helpers)
   - `eyes.rs` (E064)
   - `nato.rs` (S007, E066)
   - `dissem_closure.rs` (S008 + `S008_SCHEME` static)
   - `fgi.rs` (FgiOwnershipTrigraphSuggestRule, E071, E073)
   - `nodis_exdis.rs` (E039, E041)
   - `form_mismatch.rs` (PortionFormInBanner + BannerFormInPortion + helpers)
   - `banner/` sub-submodule last — biggest, highest test mass.

5. **Move tests** alongside each rule in the same commit as that rule. After each move: `cargo test -p marque-capco`.

6. **Move `citation_cross_refs_tests` (lines 11621–11702) into `rules/citation_cross_refs.rs`** as the second-to-last step. Most `*_CROSS_REFS` consts will need `pub(crate)`.

7. **Final: move registry**. The `CapcoRuleSet` struct + `new()` + `RuleSet` impl + all retirement-history comments → `rules/registry.rs`. Imports in `registry.rs` are explicit per-name (`use super::dissem::{DeprecatedDissemRule, NonIcInClassifiedBannerRule}; use super::rel_to::MissingUsaTrigraphRule; …`).

8. **Delete `rules_legacy.rs`** once empty. Remove alias from `lib.rs`. `cargo check -p marque-capco` + `cargo test -p marque-capco` + `cargo test --workspace` (catch engine + WASM consumers) + `cargo clippy -p marque-capco -- -D warnings` + `cargo fmt -p marque-capco`.

9. **Run the post_3b registration pin** explicitly: `cargo test -p marque-capco --test post_3b_registration_pin` (or equivalent). This is the hard regression gate.

10. **Citation byte-identity verification** (Constitution VIII): `git diff origin/staging -- crates/capco/src/rules.rs crates/capco/src/rules/ | grep -E '§|p[0-9]+|CAPCO-2016'` — every hit MUST be a relocation, never a textual change.

This sequencing **works** with Rust's module system because at each checkpoint, `rules_legacy.rs` re-exports moved symbols (or registers them in `CapcoRuleSet::new()` via fully-qualified paths), keeping the public API stable. The alias-and-extract pattern was used in PR #703 for the lattice split and is the validated approach.

---

## 9. PM Risks / Decision Points

1. **Banner sub-submodule (`rules/banner/`) is the only place the issue's flat-file suggestion fails.** The 833-line catalog cannot live in one file; splitting per-axis is the only ≤800 answer that maintains 5-year readability. The issue's `rules/banner.rs` line should be read as `rules/banner/`. **No PM blocker** — this is the only structurally-tricky area; everything else is straightforward translation.

2. **Test co-location vs. module size**: the rule submodules average ~400 lines of rule code; adding ~200 lines of inline tests pushes the upper end of `sci.rs` (~770) and `rel_to_suggest.rs` (~760) close to the 800-line ceiling. If the implementer hits a hard cap during the move, the fallback is per-domain `tests/foo_tests.rs` siblings (still co-located, but separate file). No PM decision required ahead of time — flag at the time it happens.

3. **Punt candidates: none.** Each rule moves independently, each commit stays green, no sub-area needs deferral.

4. **The issue's pre-T044 rule-ID groupings are STALE** (T044 migration landed 2026-05-22). The architect plan above uses the actual HEAD rule set, not the issue's `rules/sci.rs (E032-E035, E061-E063)` style description. **No PM decision required** — the issue text is a sketch, the plan supersedes it.

5. **No engine-crate touches anywhere.** Confirmed via grep — all changes stay inside `crates/capco/src/`. Constitution Principle VII intact.

---

## Decision 10: Legacy-ID Archaeology Relocation

User clarification mid-preflight: extensive legacy-ID provenance comments don't belong inline. Bundle into the same PR (not a follow-up). Three archaeology classes identified.

### 10.1 Destination layout

```
crates/capco/docs/archaeology/
├── README.md              # one paragraph: scope + relationship to legacy-rule-id-map.md
├── retirement-history.md  # the top-of-file //! block (rules.rs:14–120), organized by retirement PR
└── rule-id-cross-refs.md  # extracted inline cross-ref comments, grouped by live rule
```

**Three documents, not more.** A per-rule provenance file would bloat the directory; a single archaeology.md would lose the structural distinction between "retirement-from-registry" history (10.1a) and "rule-X-still-mentions-retired-rule-Y" cross-refs (10.1b).

**Relationship to `docs/refactor-006/legacy-rule-id-map.md`** — that file is the T044 wire-string ↔ legacy-ID mapping (114 rows, identifier translation). `archaeology/` is retirement *provenance* (which PR retired which rule, what it migrated into). README cross-links both directions; no content duplication.

### 10.2 Load-bearing vs. archaeology distinction

I sample-read 5 inline cross-ref comments:

| Line | Comment fragment | Class |
|------|------------------|-------|
| 1037 | `// E009, now migrated to the wire strings cited above` | **Archaeology** — pure history, move out. |
| 1399 | `// E008 suppresses). The relevant gates inside` | **Load-bearing** — describes live cross-rule co-firing behavior. Keep with one-line summary + pointer. |
| 1461 | `// W001 retired in T035c-14. See registration-site comment in` | **Archaeology** — pure pointer, move out. |
| 1615 | `// E060 — both retired) incorrectly elevated USA to the front — that` | **Load-bearing** — documents why current code does X (defends against past-behavior regression). Keep; summarize to ≤2 lines + pointer. |
| 1655 | `// E060 wins the overlap guard and applies. On re-lint, E060 is` | **Load-bearing** — describes live FR-016 overlap-guard behavior. Even though E060 is retired, the comment explains observable engine behavior. Keep verbatim; archaeology cross-ref optional. |

**Conservative bias**: any comment that explains *why current code does X* stays. Any comment that only narrates "this rule was retired in PR Y, see Z" moves out. When ambiguous, keep inline with `(See docs/archaeology/rule-id-cross-refs.md for full history)`.

### 10.3 Const + helper renames

`const E0##_AUTHORITIES` and `const E0##_CROSS_REFS` are file-private to `rules.rs` today (grep-confirmed; the two `_AUTHORITIES` survivors in `rules_declarative.rs` are out of refactor scope). After the split:

- **One rule per file** → `const AUTHORITIES: &[Citation] = ...`, `const CROSS_REFS: &[Citation] = ...`. File-private.
- **Multiple rules per file** → descriptive suffix matching the predicate-ID slug: `HCS_BARE_C_AUTHORITIES` / `HCS_BARE_SUB_AUTHORITIES` / `RSV_BARE_AUTHORITIES` in `sci.rs`; `PORTION_FORM_AUTHORITIES` / `BANNER_FORM_AUTHORITIES` in `form_mismatch.rs`. Same for `_CROSS_REFS`.
- **`E071Containment` enum + `e071_*` helpers** → `Containment` enum, drop `e071_` prefix. File-private to `fgi.rs`. Grep-confirmed no external consumer (one integration test uses `e071_on_banner` as a *local variable name*, not an import — unaffected).

### 10.4 File-size impact (revised)

Archaeology extraction removes ~120 lines from the top-of-file block + ~30–80 lines of inline retirement-narration scattered through the rule bodies. Revised tightest estimates:

| File | Pre-archaeology | Post-archaeology |
|------|-----------------|------------------|
| `mod.rs` | ~150 | ~80 (top `//!` block extracted) |
| `registry.rs` | ~360 | ~280 (PR #578 retirement narratives extracted) |
| `sci.rs` | ~770 | ~720 |
| `rel_to_suggest.rs` | ~760 | ~720 |

All other files lose 10–30 lines; none cross the 800 ceiling at any stage.

### 10.5 Risk register

Workspace grep for `\bE0[0-9][0-9]\b` / `\bW00[1-9]\b` / `\bS00[0-9]\b` hits 25+ files outside `rules.rs`: engine tests, audit consumers, README, scheme bridges. **All hits are docstring/test-name/audit-stream textual references that name a rule by legacy ID semantically** — none import a Rust symbol whose name embeds the legacy ID (other than the file-private `E0##_AUTHORITIES` consts inside `rules.rs` itself). No rename breakage. `legacy-rule-id-map.md` is the canonical translation table and stays put unchanged.

Audit-stream `Diagnostic.rule` field carries the new wire-string IDs already (post-T044, 2026-05-22); the legacy IDs in audit consumers are pre-T044 fossils, semantically frozen by FR-049 and outside refactor scope.

### 10.6 Implementer instructions (semantic-preservation gate)

"No behavior change" now also means "no archaeology semantic change." The implementer **moves** retirement-history comments, never rewrites them. If a retirement entry contradicts another source (e.g., the top-of-file block says E040 was retired in PR X but the inline narrative cites PR Y), the implementer **flags for PM review** rather than picking one — the contradiction is itself a data point that may need investigation. Same rule for citation drift: any CAPCO-2016 §-citation appearing in extracted archaeology MUST survive byte-identical (Constitution VIII applies to archaeology citations too — they were verified once and that verification must transit the move).

---

## Implementer's Checklist — Archaeology Additions — SUPERSEDED

> **Note**: This section's archaeology-extraction steps have been integrated into Stage A of the Final Form checklist below. Preserved for design-history context.

Insert these steps into §8 sequencing:

- **New Step 0** (before "create directory and skeleton"): extract archaeology FIRST.
  - `mkdir -p crates/capco/docs/archaeology`
  - Write `archaeology/README.md` with the §10.1 orientation paragraph + cross-link to `docs/refactor-006/legacy-rule-id-map.md`.
  - Cut the `//!` retirement-history block (rules.rs:14–120, the `E001 = retired ...` through `C001 = corrections-map typo` lines) into `archaeology/retirement-history.md`. Organize by retirement PR (PR 3c.B Commit 6, PR 3b.F, T035c-14, etc.) — chronological ordering surfaces the migration arc more clearly than ID ordering.
  - Walk the file top-to-bottom, classify each inline comment per §10.2's load-bearing/archaeology distinction. Move archaeology-class comments to `archaeology/rule-id-cross-refs.md`, grouped by **live rule** that the cross-ref documented. Replace at the original site with a one-line `(See docs/archaeology/rule-id-cross-refs.md#<live-rule-anchor>)` pointer ONLY if the surrounding context loses meaning without it; otherwise just delete.
  - Commit: `refactor(capco): #561 — extract legacy-ID archaeology to docs/archaeology/`
  - **Verify**: `cargo check -p marque-capco` green (comments are not load-bearing for compilation; this should always pass).
  - **Constitution VIII gate**: `git diff HEAD~1 -- crates/capco/src/rules.rs crates/capco/docs/archaeology/ | grep -E '§|p[0-9]+|CAPCO-2016'` — every hit must be a relocation, never a textual change.

- **New Step 6.5** (after `rules/` directory complete, before `rules_legacy.rs` deletion): rename `const E0##_AUTHORITIES` → `const AUTHORITIES` (or descriptive multi-rule name per §10.3). Same pass renames `E071Containment` → `Containment` and `e071_*` helpers. Each rename = its own commit so the diff stays trivially reviewable. `cargo check` + `cargo test -p marque-capco` between renames.

- **Step 8.10 update**: the Constitution VIII gate now also covers `crates/capco/docs/archaeology/*.md`. Updated grep: `git diff origin/staging -- crates/capco/src/rules.rs crates/capco/src/rules/ crates/capco/docs/archaeology/ | grep -E '§|p[0-9]+|CAPCO-2016'`.

- **Step 8.11 NEW — semantic-preservation attestation**: implementer attests in the PR body that no retirement-history entry was edited during the move; any contradictions found were filed to PM, not silently corrected.

---

## 11. Rust-Specialist Preflight Integration (Implementer-Critical)

Items from `docs/plans/2026-05-23-561-rules-split-rust-preflight.md` that affect implementation. All integrated; nothing dissented.

### 11.1 `#[allow(dead_code)]` MUST travel with moved items (preflight §9)

The following `pub(crate)` consts currently carry `#[allow(dead_code)]` because their only consumer is inside the `#[cfg(any())]`-dead test block. After Decision 2's quarantine move, the dead block lives in `_disabled_tests.rs` (sibling, never compiled) — but the lint behavior is identical, so the attributes MUST move with the items:

| Item | Source line | Destination |
|------|-------------|-------------|
| `dedup_country_codes` | 2816 (`#[allow(dead_code)]` at 2815) | `rules/helpers.rs` |
| `S003_CROSS_REFS` | 1710 (attribute on block) | `rules/joint.rs` |
| `E037_CROSS_REFS` | 5362 (attr at 5360) | `rules/nodis_exdis.rs` |
| `E038_CROSS_REFS` | 5381 (attr at 5379) | `rules/nodis_exdis.rs` |
| `E039_CROSS_REFS` | 5402 (attr at 5400) | `rules/nodis_exdis.rs` |

Omitting the attribute after the move causes `cargo clippy -- -D warnings` to fail. **Easy to miss in mechanical-move review** — the implementer checklist calls this out at Step 6.6.

`E005_CROSS_REFS` does NOT carry `#[allow(dead_code)]` (consumed by the live `citation_cross_refs_tests`).

### 11.2 SPDX headers on every new file (preflight §12)

Every new `.rs` file in `rules/`, `rules/banner/`, and the new `_disabled_tests.rs` MUST start with:
```
// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
```
CI REUSE check fails otherwise. Canonical form: `crates/capco/src/lattice/sci.rs` lines 1–3.

### 11.3 Preflight implementer sequencing (§12) supersedes the architect plan's §8 ordering

Preflight §12 recommends:
1. `helpers.rs` first (most-depended-upon within `rules/`)
2. `banner/mod.rs` + all `banner/eval_*.rs` together (the catalog references the eval fns by value — they must arrive in the same checkpoint)
3. Domain rule files in any order
4. `registry.rs` last
5. `mod.rs` declarations + re-exports added incrementally

The architect plan §8 had banner as the LAST module; preflight §12's reordering (banner second) is the correct shape because the `BANNER_CATEGORY_CATALOG` initializer cannot compile without all five `evaluate_*` functions visible simultaneously. **Defer to preflight ordering**.

### 11.4 Final crate-level invariant (implementer-attested at PR-open)

After the split:
- `cargo test --workspace --all-features` is byte-equivalent to pre-split output (same pass/fail set, same test count, same diagnostic output).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` is green.
- `cargo fmt --workspace --check` is clean.
- The `post_3b_registration_pin` test (the exact-rule-ID-set pin from `crates/capco/tests/post_3b_registration_pin.rs`) passes unchanged — same rules registered in the same order producing the same wire-string IDs.

---

## Implementer's Checklist — Final Form (Implementer-Ready)

Each step is independently verifiable. Numbered checkpoints assert green `cargo check -p marque-capco` unless otherwise noted.

**Pre-flight (no code changes)**:
- [ ] Read `docs/plans/2026-05-23-561-rules-split-rust-preflight.md` cover-to-cover.
- [ ] Read this plan cover-to-cover, including Decisions 1–10 and §11.
- [ ] Confirm worktree HEAD: `git rev-parse HEAD` matches `8ef8b457` or a clean descendant.

**Stage A — Archaeology (Decision 10)** — extract BEFORE split:
- [ ] Step A1: `mkdir -p crates/capco/docs/archaeology`. Write `README.md` per §10.1 with cross-link to `docs/refactor-006/legacy-rule-id-map.md`.
- [ ] Step A2: Cut top-of-file `//!` retirement block (rules.rs:14–120) → `archaeology/retirement-history.md`, organized by retirement PR.
- [ ] Step A3: Walk rules.rs top-to-bottom; classify inline retirement comments per §10.2; move archaeology-class comments to `archaeology/rule-id-cross-refs.md` grouped by live rule. Leave load-bearing comments in place (optionally add `(See docs/archaeology/rule-id-cross-refs.md)` pointer).
- [ ] Step A4: `cargo check -p marque-capco` green; commit `refactor(capco): #561 — extract legacy-ID archaeology`.

**Stage B — Dead-block quarantine (Decision 2.a)** — extract BEFORE split:
- [ ] Step B1: Cut the `#[cfg(any())]`-gated block at rules.rs:8046–11573 (full block, intact, do NOT split per rule).
- [ ] Step B2: Create `crates/capco/src/_disabled_tests.rs` with the SPDX header + the §2.a top-of-file comment + the moved block.
- [ ] Step B3: In `crates/capco/src/lib.rs`, add `#[cfg(any())] mod _disabled_tests;` near `pub mod rules;`.
- [ ] Step B4: `cargo check -p marque-capco` + `cargo test -p marque-capco` green. Commit `refactor(capco): #561 — quarantine dead test block to _disabled_tests.rs (#722)`.

**Stage C — File rename + skeleton**:
- [ ] Step C1: `git mv crates/capco/src/rules.rs crates/capco/src/rules_legacy.rs`. In `lib.rs`, change `pub mod rules;` to `mod rules_legacy; pub use rules_legacy as rules;`. `cargo check -p marque-capco` green. Commit.
- [ ] Step C2: Create `crates/capco/src/rules/mod.rs` empty (SPDX header only). `cargo check` green.

**Stage D — Move modules** (preflight §12 ordering):
- [ ] Step D1: Move `helpers.rs` first (`FixDiagnosticParams`, `make_fix_diagnostic`, `sar_block_span`, `dedup_country_codes`, `canonicalize_trigraph_list`, `is_fgi_invalid_ownership_token` per §3). Each retains current `pub(crate)` visibility. Carry `#[allow(dead_code)]` per §11.1 where present. `cargo check` + `cargo test -p marque-capco` green. Commit.
- [ ] Step D2: Move `banner/mod.rs` + all five `banner/eval_*.rs` together in ONE commit (preflight §12: catalog references eval fns by value; they must arrive simultaneously). Apply `BannerCategoryRow` `pub(super)` per §5; apply `pub(super)` to each `evaluate_*` fn. `cargo check` + `cargo test` green. Commit.
- [ ] Step D3: Move each domain file in any order: `dissem.rs`, `text_handling.rs`, `joint.rs`, `rel_to.rs`, `rel_to_suggest.rs`, `sci.rs`, `eyes.rs`, `nato.rs`, `dissem_closure.rs`, `fgi.rs`, `nodis_exdis.rs`, `form_mismatch.rs`. ONE COMMIT PER FILE. Carry `#[allow(dead_code)]` per §11.1 onto `S003_CROSS_REFS`, `E037_CROSS_REFS`, `E038_CROSS_REFS`, `E039_CROSS_REFS` at their new homes. Each rule struct → `pub(super)`. Helpers → file-private unless §3 says otherwise. After each commit: `cargo check` + `cargo test -p marque-capco` green.
- [ ] Step D4: Move `citation_cross_refs_tests` (rules.rs lines 11621–11702) → `rules/citation_cross_refs_tests.rs`. Update imports per §2.b. Preserve `#[cfg_attr(coverage_nightly, coverage(off))]`. `cargo test -p marque-capco --test '*' && cargo test -p marque-capco` green.
- [ ] Step D5: Move `CapcoRuleSet` + `Default` impl + `new()` + `RuleSet` impl + all retirement-history comments → `rules/registry.rs`. Imports: explicit per-name (`use super::dissem::DeprecatedDissemRule;` etc.). `cargo check` + full test suite green. Commit.

**Stage E — Cleanup**:
- [ ] Step E1: Verify `rules_legacy.rs` is empty (or contains only `use` re-exports). Delete the file; remove the `pub use rules_legacy as rules;` alias in `lib.rs`; replace with `pub mod rules;`. `cargo check` + full test suite green. Commit.
- [ ] Step E2: Rename consts per Decision 10.3: `E0##_AUTHORITIES` → `AUTHORITIES` (single-rule files) or descriptive predicate-slug name (multi-rule files: `HCS_BARE_C_AUTHORITIES`, `PORTION_FORM_AUTHORITIES`, etc.). `E0##_CROSS_REFS` follows the same pattern. ONE COMMIT (whole-workspace rename). `cargo check` + `cargo clippy --workspace --all-targets --all-features -- -D warnings` green.
- [ ] Step E3: Rename `E071Containment` → `Containment`, drop `e071_` prefix from helpers. File-private to `rules/fgi.rs`. ONE COMMIT. `cargo check` + `cargo clippy` green.

**Stage F — Final verification**:
- [ ] Step F1: `cargo test --workspace --all-features` green. Test count byte-equivalent to pre-split (run the same command on `origin/staging` baseline + diff the counts).
- [ ] Step F2: `cargo clippy --workspace --all-targets --all-features -- -D warnings` green.
- [ ] Step F3: `cargo fmt --workspace --check` clean.
- [ ] Step F4: `cargo test -p marque-capco --test post_3b_registration_pin` green (exact-rule-ID-set pin — hard regression gate).
- [ ] Step F5: **Constitution VIII gate** — `git diff origin/staging -- crates/capco/src/rules.rs crates/capco/src/rules/ crates/capco/src/_disabled_tests.rs crates/capco/docs/archaeology/ | grep -E '§|p[0-9]+|CAPCO-2016'`. Every hit MUST be a relocation, never a textual change.
- [ ] Step F6: Verify every new `.rs` file carries the SPDX header (§11.2). One-liner: `for f in $(find crates/capco/src/rules crates/capco/src/_disabled_tests.rs -name '*.rs'); do head -3 "$f" | grep -q 'SPDX-License-Identifier' || echo "MISSING: $f"; done`.
- [ ] Step F7: Verify every file in `rules/` is ≤800 lines: `wc -l crates/capco/src/rules/**/*.rs | awk '$1 > 800 {print}'` returns empty.

**PR open**:
- [ ] PR body attests: (a) Stage F1–F7 all green; (b) no retirement-history entry was edited during archaeology extraction (semantic-preservation per §10.6); (c) any contradictions found during archaeology classification were filed to PM, not silently corrected; (d) issue #722 is referenced in `_disabled_tests.rs` top-of-file comment.

---

## Citation Verification Statement (Constitution VIII) — Expanded

I sampled `crates/capco/docs/CAPCO-2016.md` for the major §-anchors referenced in this plan: §A.6 (block ordering grammar), §D.1 (separators), §D.2 Table 3 (closure rules), §H.3 (JOINT), §H.4 (SCI per-system), §H.5 (SAR), §H.6 (AEA / RD / FRD / TFNI / UCNI / DCNI), §H.7 (FGI / NATO), §H.8 (dissem / REL TO / NOFORN / EYES / RELIDO), §H.9 (NODIS / EXDIS / LIMDIS / SBU). All section labels and page numbers cited in `rules.rs` doc-comments and pointed to in the line-ranges above resolve to real passages in the manual.

**This plan introduces zero new citations**; every existing citation must survive the move byte-identical across THREE surfaces:

1. **Live rule code** (`crates/capco/src/rules/**/*.rs`) — doc-comments on rule structs, `cited_authorities()` returns, the `*_CROSS_REFS` and `*_AUTHORITIES` const tables.
2. **Moved retirement-history block** (`crates/capco/docs/archaeology/retirement-history.md` + `rule-id-cross-refs.md`) — every CAPCO §-citation that lived inside the top-of-file `//!` block or in extracted inline comments must transit byte-identical. Citations were verified once when they were authored; that verification must survive the mechanical move.
3. **`citation_cross_refs_tests` constants** (`rules/citation_cross_refs_tests.rs`) — the `E005_CROSS_REFS`, `S003_CROSS_REFS`, `E037_CROSS_REFS`, `E038_CROSS_REFS`, `E039_CROSS_REFS` consts traverse module boundaries during the move; their `Citation` values (the `capco_section(...)` and `capco(...)` constructor calls with their `§`/page-number literals) must be byte-identical at the destination.

The dead-block `_disabled_tests.rs` (Decision 2.a) MAY contain stale citations; preflight §4 confirmed `marque_capco_test_support` references are stale. That file is dead code under `#[cfg(any())]` and is OUT OF SCOPE for citation verification per the disposition decision filed to issue #722.

The implementer's Stage F5 gate (`git diff` filtered for `§|p[0-9]+|CAPCO-2016` tokens across all three surfaces) catches drift mechanically. Constitution VIII non-negotiable.
