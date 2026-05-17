# Stage 2 PR B — `scheme/mod.rs` hub split

## Scope

Split the post-PR-A `crates/capco/src/scheme/mod.rs` (currently **3057 LOC**) into a small hub + 7 sibling modules, each under the 800 LOC ceiling. PR A already moved the 4 large leaves (`predicates/`, `actions/`, `constraints/`, `rewrites/`) into subdirectories; this PR carves out the remaining hub-resident code.

## Current state

`crates/capco/src/scheme/mod.rs` — **3057 LOC**, single file holding:

| Section | Lines | LOC | Content |
|---------|-------|-----|---------|
| Header / IDs | 1-331 | 331 | module docs, sub-mod decls, re-exports, `CAT_*`, `TOK_*` constants |
| `CapcoMarking` | 332-918 | 587 | struct + `PartialEq`/`Eq`/`From` + inherent (`new`, `join_via_lattice`) + `Lattice` |
| `CapcoOpenVocabRef` | 919-960 | 42 | enum + doc |
| `CapcoScheme` | 961-1054 | 94 | struct + `Debug`/`Default`/ctors |
| `CapcoParseError` | 1055-1069 | 15 | enum + variants |
| Second `impl CapcoScheme` | 1070-1405 | 336 | scheme-private predicate helpers |
| `impl MarkingScheme for CapcoScheme` | 1406-2042 | 637 | trait body (22 methods) |
| Closure rules + catalog | 2043-2291 | 249 | `FDR_DOMINATORS` + 7× `CLOSURE_NOFORN_*` + `CAPCO_CLOSURE_RULES` |
| Render table | 2292-2434 | 143 | `DissemFamilyMembership` + `AxisRenderRow` + `RENDER_TABLE` |
| Class-floor catalog | 2435-2894 | 460 | `ClassFloorPolicy` + `ClassFloorRow` + `CLASS_FLOOR_CATALOG` |
| SCI per-system catalog | 2895-3049 | 155 | `CompanionForm` + `RULE_E059` + `SciPerSystemKind` + `SciPerSystemRow` + `SCI_PER_SYSTEM_CATALOG` |
| Trailing `impl CapcoMarking` | 3050-3057 | 8 | small helper(s) |

The 4 leaves split in PR A (`predicates/`, `actions/`, `constraints/`, `rewrites/`) plus `shared.rs` and `tests.rs` remain **completely untouched** in this PR.

## Goals

1. Reduce `mod.rs` to a hub of declarations + re-exports + ID constants (~330 LOC).
2. Every new sibling module ≤ 800 LOC.
3. **Zero behavior change.** Public API surface byte-identical. Diagnostic output, fix proposals, audit records, citations all unchanged.
4. **Zero semantic edits.** Move text verbatim; rewrite imports only where the new module path requires it.

## Target layout

```
crates/capco/src/scheme/
├── mod.rs                       # ~331 LOC — docs, sub-mod decls, re-exports, CAT_* + TOK_* IDs
├── marking.rs                   # ~620 LOC — CapcoMarking + impls + CapcoOpenVocabRef
├── adapter.rs                   # ~445 LOC — CapcoScheme struct + Debug + Default + 2 inherent impls + CapcoParseError
├── marking_scheme_impl.rs       # ~637 LOC — impl MarkingScheme for CapcoScheme
├── closure.rs                   # ~249 LOC — FDR_DOMINATORS + 7× CLOSURE_NOFORN_* + CAPCO_CLOSURE_RULES
├── render.rs                    # ~143 LOC — DissemFamilyMembership + AxisRenderRow + RENDER_TABLE
├── class_floor.rs               # ~460 LOC — ClassFloorPolicy + ClassFloorRow + CLASS_FLOOR_CATALOG
├── sci_per_system.rs            # ~155 LOC — CompanionForm + RULE_E059 + SciPerSystemKind + SciPerSystemRow + SCI_PER_SYSTEM_CATALOG
├── shared.rs                    # UNTOUCHED (31 LOC)
├── tests.rs                     # UNTOUCHED (753 LOC)
├── actions/                     # UNTOUCHED (PR A)
├── constraints/                 # UNTOUCHED (PR A)
├── predicates/                  # UNTOUCHED (PR A)
└── rewrites/                    # UNTOUCHED (PR A)
```

### Per-file content (heuristic, refine if needed)

**`marking.rs`** (`pub(crate) mod marking;` in mod.rs, re-export `CapcoMarking` / `CapcoOpenVocabRef`):
- `pub struct CapcoMarking(...)`
- `impl PartialEq for CapcoMarking`
- `impl Eq for CapcoMarking`
- `impl From<CanonicalAttrs> for CapcoMarking`
- `impl CapcoMarking { fn new, fn join_via_lattice }` (the big 486-LOC inherent block — move whole, do not split internally)
- `impl Lattice for CapcoMarking`
- `pub enum CapcoOpenVocabRef`
- Trailing `impl CapcoMarking` block from lines 3050-3057 (fold into the main inherent block at the natural insertion point — no logic change)

**`adapter.rs`** (`pub(crate) mod adapter;` in mod.rs, re-export `CapcoScheme` / `CapcoParseError`):
- `pub struct CapcoScheme { ... }`
- `impl std::fmt::Debug for CapcoScheme`
- `impl Default for CapcoScheme`
- `impl CapcoScheme { fn new ... }` (first inherent impl — ctors)
- `impl CapcoScheme { ... }` (second inherent impl — small helpers)
- `pub enum CapcoParseError`
- `impl CapcoScheme { ... }` (third inherent impl — scheme-private predicate helpers, ~336 LOC)

If the predicate-helpers inherent impl is heavy enough to push `adapter.rs` over 800 LOC, split it into `adapter.rs` (ctors/Debug/Default + first 2 inherent blocks + `CapcoParseError`) and `adapter_helpers.rs` (third inherent impl). The split point is the doc comment marking the predicate-helper block. Decide based on actual LOC after move — don't pre-split if not needed.

**`marking_scheme_impl.rs`**:
- `impl MarkingScheme for CapcoScheme { ... }` — 22 trait methods, move whole

**`closure.rs`**:
- `pub(crate) static FDR_DOMINATORS: &[TokenRef]`
- 7× `const CLOSURE_NOFORN_*: ClosureRule`
- `static CAPCO_CLOSURE_RULES: &[ClosureRule]`

**`render.rs`**:
- `pub(crate) enum DissemFamilyMembership`
- `pub(crate) struct AxisRenderRow`
- `pub(crate) const RENDER_TABLE: &[AxisRenderRow]`

**`class_floor.rs`**:
- `pub(crate) enum ClassFloorPolicy`
- `pub(crate) struct ClassFloorRow`
- `const CLASS_FLOOR_CATALOG: &[ClassFloorRow]`
- Any `pub(crate) fn is_class_floor_catalog_name(...)` dispatch helper referenced by the doc comment (if present in mod.rs near the catalog; move with it)

**`sci_per_system.rs`**:
- `pub(crate) enum CompanionForm`
- `const RULE_E059: marque_rules::RuleId`
- `pub(crate) enum SciPerSystemKind`
- `pub(crate) struct SciPerSystemRow`
- `const SCI_PER_SYSTEM_CATALOG: &[SciPerSystemRow]`
- Any `pub(crate) fn is_sci_per_system_catalog_name(...)` dispatch helper

**`mod.rs`** (after split):
- Module-level docs (`//! ...`)
- `pub(crate) mod actions; constraints; predicates; rewrites; shared;` (existing)
- `pub(crate) mod marking; adapter; marking_scheme_impl; closure; render; class_floor; sci_per_system;` (new)
- `#[cfg(test)] mod tests;`
- `pub(crate) use self::predicates::{capco_token_category, rel_to_covers};` (existing)
- `pub use self::predicates::{is_fdr_dominator, is_orcon_family};` (existing)
- `pub use self::marking::{CapcoMarking, CapcoOpenVocabRef};` (new)
- `pub use self::adapter::{CapcoScheme, CapcoParseError};` (new)
- Any `pub(crate) use` lines needed so internal modules can name the moved items by the established path
- All `pub const CAT_*` category-ID constants (lines 123-138)
- All `pub const TOK_*` token-ID constants (lines 148-297)

## Public API preservation (binding)

These symbols MUST remain importable at exactly the paths they hold today:

- `marque_capco::CapcoMarking` (re-exported from `lib.rs`)
- `marque_capco::CapcoOpenVocabRef` (re-exported from `lib.rs`)
- `marque_capco::CapcoScheme` (re-exported from `lib.rs`)
- `marque_capco::scheme::CapcoParseError` (NOT crate-root re-exported; reachable via the `scheme` module path only)
- `marque_capco::CAT_*` (all 11 category-ID constants)
- `marque_capco::TOK_*` (all token-ID constants — see lines 148-297 of pre-split mod.rs for the canonical list)
- `marque_capco::is_fdr_dominator`
- `marque_capco::is_orcon_family`
- Any other `pub use self::predicates::{...}` already in mod.rs (preserve verbatim)

The pre-split public-symbol enumeration is the contract. Run this check before and after to confirm byte-identical parity:

```bash
# Pre-split baseline (run on a clean staging checkout):
rg '^pub (fn|struct|enum|const|static|type|trait|use)' \
   crates/capco/src/scheme/mod.rs \
   | grep -oE 'pub (fn|struct|enum|const|static|type|trait|use) [A-Za-z_][A-Za-z0-9_]*' \
   | sort -u > /tmp/modrs_public_baseline.txt

# Post-split verification (must include re-exports from new sub-modules):
rg '^pub (fn|struct|enum|const|static|type|trait|use)' \
   crates/capco/src/scheme/mod.rs \
   crates/capco/src/scheme/marking.rs \
   crates/capco/src/scheme/adapter.rs \
   crates/capco/src/scheme/marking_scheme_impl.rs \
   crates/capco/src/scheme/closure.rs \
   crates/capco/src/scheme/render.rs \
   crates/capco/src/scheme/class_floor.rs \
   crates/capco/src/scheme/sci_per_system.rs \
   | grep -oE 'pub (fn|struct|enum|const|static|type|trait|use) [A-Za-z_][A-Za-z0-9_]*' \
   | sort -u > /tmp/modrs_public_after.txt

diff /tmp/modrs_public_baseline.txt /tmp/modrs_public_after.txt
# Expected: zero output (perfect parity).
```

## Hard prohibitions

These are non-negotiable. Violating any one of them is grounds for rejecting the change.

1. **NEVER run `git restore`, `git reset --hard`, `git checkout -- <path>`, `git clean`, `git stash --keep-index`, or any other command that writes from git into the working tree.**
2. **NEVER run `git add` or `git add -N`. The implementer stages and commits at session end.**
3. **NEVER run `cargo fmt` or any other command that rewrites files during the move.** (Post-verification `cargo fmt -p marque-capco` as the final cleanup pass is permitted — but only after every other gate passes and only as the very last commit.)
4. **NEVER delete `crates/capco/src/scheme/mod.rs` until every new sibling file exists, the crate compiles, all tests pass, and citation-lint is clean.** Rewrite `mod.rs` in place — empty out the moved sections, replace with `pub(crate) mod <name>;` declarations and `pub use self::<name>::{...};` re-exports.
5. **NEVER use `rm -rf` on anything inside `crates/capco/`.**
6. **NEVER run `git push --force[-with-lease]`.** This is a sub-agent prohibition; stop and request explicit authorization from the user if you believe a force-push is needed.
7. **NEVER edit semantic content.** No logic changes, no comment edits (other than necessary path corrections like `[`Self::satisfies`]` doc-links if the impl moves), no citation changes, no severity tweaks, no rule additions/removals. The Constitution VIII citation discipline (every `§X.Y pNN` byte-identical) is a hard gate.
8. **NEVER split internal function bodies.** The 486-LOC `join_via_lattice` and the 637-LOC `impl MarkingScheme` trait body each move as one unit. If you find yourself wanting to "tidy up" by splitting a function or extracting a helper, stop — that is out of scope and would expand the diff blast radius.
9. **NEVER touch the four PR-A leaf directories (`actions/`, `constraints/`, `predicates/`, `rewrites/`) or `shared.rs` or `tests.rs`.** They are stable. Only the moved sections from `mod.rs` may land in new sibling files.

## Acceptance criteria

Before commit:

- [ ] `cargo check --workspace` passes (warnings allowed only if they pre-exist on staging).
- [ ] `cargo test -p marque-capco` passes.
- [ ] `cargo test --workspace` passes.
- [ ] `cargo clippy -p marque-capco -- -D warnings` passes.
- [ ] `cargo run -p citation-lint --release -- .` reports **0 defects**.
- [ ] `wc -l crates/capco/src/scheme/*.rs` shows every file ≤ 800 LOC.
- [ ] Public-symbol parity check (the diff block above) produces **zero output**.
- [ ] CAPCO §-citation count is byte-identical to staging:
  ```
  rg -o 'CAPCO-2016 §[A-Z]\.[0-9]+ p[0-9]+' crates/capco/src/scheme/ | wc -l
  ```
  Compare against the same command on `origin/staging` — counts must match exactly.

## Commit policy

Single squash-mergeable PR. Two commits are acceptable:

1. `refactor(capco): Stage 2 PR B — split scheme/mod.rs into per-section sibling modules (#466)` — the move
2. `style(capco): apply cargo fmt to scheme/ hub-split sub-modules` — the post-fmt cleanup (only if needed)

PR title: `refactor(capco): Stage 2 PR B — split scheme/mod.rs hub into per-section modules (#466)`

PR description should call out:
- 8 new sibling files (or 7 if `adapter_helpers.rs` is skipped)
- LOC table (before/after)
- Public-symbol parity output (zero diff)
- Citation count parity (matches staging)
- All other Stage 2 leaves untouched

## Notes for the impl agent

- The 4 PR-A leaf modules already use `use super::super::*;` or similar to reach back into `mod.rs` for IDs, constants, and re-exports. After the hub split, those imports must continue to resolve. The hub's public re-export surface (the `pub use self::<sibling>::{...};` lines) is what makes that work — be deliberate about which symbols need `pub use` vs `pub(crate) use`.
- The `impl MarkingScheme for CapcoScheme` in `marking_scheme_impl.rs` will need to import many helpers from sibling modules (closure rules, render table, etc.). Use `use super::closure::CAPCO_CLOSURE_RULES;` style — not `use crate::scheme::closure::...;` — to keep the new modules self-describing.
- If you discover that splitting `adapter.rs` requires also splitting the second inherent `impl CapcoScheme` block (the 336-LOC predicate-helper block), make that split early — don't try to keep it in one file just to match the heuristic table. The 800-LOC ceiling is the binding constraint.
- The `module_inception` clippy warning that fired on `scheme/tests.rs` (fixed in #480) is why the new file holding `CapcoScheme` is named `adapter.rs`, not `scheme.rs`. Do not rename to `scheme.rs` — clippy will fail in CI.
