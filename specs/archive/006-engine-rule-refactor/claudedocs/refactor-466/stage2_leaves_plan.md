<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Issue #466 Stage 2 — PR A: leaf-module sub-splits

**Scope:** sub-split the four post-Stage-1 leaf modules under
`crates/capco/src/scheme/` whose LOC still exceeds the 800-line ceiling
established by the workspace's coding-style rule. `mod.rs` is OUT OF SCOPE
for this PR (PR B will handle the hub separately).

**Branch:** `refactor-466-stage2-leaves` (already created off
`staging`).

## Current state (pre-split)

```
  1266 crates/capco/src/scheme/actions.rs
  1201 crates/capco/src/scheme/constraints.rs
  3057 crates/capco/src/scheme/mod.rs        ← out of scope (PR B)
  1936 crates/capco/src/scheme/predicates.rs
  1462 crates/capco/src/scheme/rewrites.rs
    31 crates/capco/src/scheme/shared.rs     ← already under ceiling
   753 crates/capco/src/scheme/tests.rs      ← already under ceiling
```

## Goals

1. Every leaf sub-module ≤ 800 LOC (firm) — split into more
   sub-modules as needed.
2. Zero behavior change. `cargo test --workspace --no-fail-fast` must
   match staging post-PR-480 (2339 passed / 0 failed / 4 ignored).
3. Zero public-API change. Every `pub` and `pub(crate)` name keeps
   its exact import path as observed at HEAD of branch (i.e.,
   `super::predicates::is_fdr_dominator` etc.) via re-exports from
   each new sub-directory's `mod.rs`.
4. Every CAPCO §-citation moves byte-identical. Run
   `rg -c '§[A-Z]+\.[0-9]+(\.[0-9]+)? p[0-9]+' crates/capco/src/scheme/`
   on the new tree and confirm the total matches HEAD's total.

## Per-leaf target layout

Each leaf becomes a directory; its current single-file form is
deleted in the same change set. The agent picks the exact function
groupings after reading the source — these are starting heuristics.

### `predicates.rs` (1936 LOC) → `predicates/` directory

Heuristic groupings (agent may adjust if cohesion suggests otherwise):

| New sub-module | Anchor content (line ranges from HEAD) | Rough LOC |
|----------------|----------------------------------------|-----------|
| `predicates/mod.rs` | imports + re-exports + `use super::constraints::{...}` hoist | ~50 |
| `predicates/token_routing.rs` | `capco_token_category`, `dissem_to_tok`, `dissem_token_id_for_form`, `dissem_token_span`, `infer_companion_form` | ~250 |
| `predicates/dissem.rs` | `never_fires`, `is_classified`, `dissem_has_noforn`, `dissem_has_non_fdr_other_than_fouo`, `is_fdr_dissem_token`, `is_fdr_dominator`, `is_orcon_family`, `dissem_family_of`, `rel_to_covers`, all `*_trigger` fns (FOUO/LIMDIS/SBU/UCNI), `has_dod_ucni`, `has_doe_ucni`, `joint_requires_usa` | ~500 |
| `predicates/satisfies.rs` | `satisfies_attrs`, `evaluate_custom_by_attrs`, `collect_present_tokens`, `first_sci_span`, `us_level`, `last_dissem_span` | ~500 |
| `predicates/class_floor.rs` | `class_floor_*` helpers (`is_class_floor_catalog_name`, `class_floor_row_by_name`, `class_floor_anchor_span`, `first_span_of_optional`, `class_floor_catalog_eval`, `class_floor_satisfied`), `hcs_system_constraints` | ~350 |
| `predicates/presence.rs` | all `presence_*` fns (HCS/SI/TK/RSV/RD/FRD/TFNI/UCNI/SAR/RSEN/IMCON/ORCON/EYES/BALK/BOHEMIA/ATOMAL/passthrough_*), SCI bare-presence helpers (`anchors_on`, `has_compartment`, `compartment_has_sub`, `is_tk_noforn_compartment`), SCI-per-system presence (`presence_hcs_o`, `presence_hcs_p_any`, `presence_hcs_p_sub`, `presence_si_g`, `presence_tk_compartment_noforn`), `sci_per_system_*` catalog dispatchers | ~580 |

### `actions.rs` (1266 LOC) → `actions/` directory

| New sub-module | Anchor content | Rough LOC |
|----------------|----------------|-----------|
| `actions/mod.rs` | imports + re-exports + `use super::predicates::{...}` hoist | ~50 |
| `actions/intent.rs` | `apply_intent_to_marking`, `apply_fact_add`, `apply_fact_remove` | ~530 |
| `actions/category_ops.rs` | `capco_category_contains`, `capco_category_has_values`, `capco_category_clear`, `capco_category_replace`, `extract_foreign_sources`, `merge_fgi_markers`, `page_context_to_attrs` | ~430 |
| `actions/companions.rs` | `noop_action`, `strip_dod_ucni_action`, `strip_doe_ucni_action`, `emit_companion_insert`, `emit_hcs_o_companions`, `emit_hcs_p_sub_companions`, `emit_si_g_companions`, `emit_companion_required` | ~330 |

### `constraints.rs` (1201 LOC) → `constraints/` directory

The bulk is `build_constraints()` (lines 201-883 ≈ 680 LOC) — a single
`vec![Constraint { ... }, ...]` literal. Extract grouped rows into
helper fns that return `Vec<Constraint>`; `build_constraints()`
concatenates them. **Constraint ORDER MUST BE PRESERVED EXACTLY**
(the predicate evaluator may have order-dependent tiebreakers; treat
the input order as load-bearing until proven otherwise).

| New sub-module | Anchor content | Rough LOC |
|----------------|----------------|-----------|
| `constraints/mod.rs` | imports + `build_categories()` + `build_constraints()` (concatenating helpers in order) + re-exports | ~250 |
| `constraints/conflicts.rs` | the first chunks of `build_constraints` covering dissem-axis Conflicts, ConflictsWithFamily, ConflictsWithUnless rows | ≤500 |
| `constraints/families.rs` | remaining `build_constraints` chunks (requires/implies/supersedes rows, custom-named rows) | ≤500 |
| `constraints/helpers.rs` | `e012_dual_classification`, `e014_joint_rel_to_coverage`, `e021_aea_requires_noforn`, `e024_rd_precedence`, `e038_dos_dissem_requires_noforn`, `w002_us_commingled_with_fgi`, `class_floor_emit`, `sci_per_system_emit` | ~400 |

If `build_constraints` resists clean partition (one block of 800+ LOC
of related rows), keep the body in `mod.rs` and only extract `build_categories`
+ the helpers, leaving `mod.rs` under 800.

### `rewrites.rs` (1462 LOC) → `rewrites/` directory

The bulk is one `build_page_rewrites()` fn building a
`vec![PageRewrite { ... }, ...]`. Natural Pattern groupings visible
in the source comments:

| Comment marker (HEAD line #) | Theme |
|------------------------------|-------|
| L131-236 | Entry constants + `noforn-clears-*` + consultant §3.4.1 entries 1-7 (FGI rollups, ORCON-NATO transmute, SBU-NF / LES-NF transmute) |
| L237-276 | PR 3c.B Sub-PR 8.F — Pattern A NOFORN-supremacy: NODIS, EXDIS |
| L277-288 | PR 3c.B Sub-PR 8.F.2 — Pattern A: SBU-NF, LES-NF imply NOFORN |
| L289-330 | PR 4b-C Commit 3 — Pattern C strip rows (FOUO/LIMDIS/SBU/UCNI when classified) |
| L331-1462 | PR 4b-C Commit 4 — Pattern B FOUO eviction + tail rows |

| New sub-module | Anchor content | Rough LOC |
|----------------|----------------|-----------|
| `rewrites/mod.rs` | `build_page_rewrites()` (concatenates Vec results from helpers in order), shared const declarations if any need to outlive a sub-module | ~150 |
| `rewrites/consultant_entries.rs` | `noforn-clears-rel-to`, `noforn-clears-fdr-family`, entries 1-7 (FGI / ORCON-NATO / SBU-NF / LES-NF) | ≤600 |
| `rewrites/pattern_a.rs` | NODIS/EXDIS/SBU-NF/LES-NF NOFORN-implies rows | ≤500 |
| `rewrites/pattern_b_c.rs` | Pattern C strip rows + Pattern B FOUO eviction rows | ≤600 |

**Row order MUST be preserved exactly.** The scheduler topo-sort
breaks ties on input order; reordering rows can change rewrite
schedule without changing the source `name` strings and silently
shift behavior. The simplest preservation is: each helper returns
`Vec<PageRewrite<CapcoScheme>>` in the same order the rows appear at
HEAD, and `build_page_rewrites()` calls them in the same order the
groups appear at HEAD.

## Public API preservation

The leaf modules currently export the following names that must
remain reachable from `super::<leaf>::NAME` (so the cross-module
imports in `scheme/mod.rs`, `scheme/tests.rs`, and the other leaves
continue to compile unchanged):

Verified at HEAD via `rg "use super::(predicates|actions|constraints|rewrites)::" crates/capco/src/scheme/`:

- `super::predicates::` — `class_floor_anchor_span`, `rel_to_covers`,
  `capco_token_category`, `is_fdr_dominator`, `is_orcon_family`,
  `dissem_token_id_for_form`, `dissem_token_span`, `first_sci_span`,
  `infer_companion_form`, `last_dissem_span`, `us_level`,
  `dod_ucni_classified_trigger`, `dod_ucni_promotes_noforn_trigger`,
  `doe_ucni_classified_trigger`, `doe_ucni_promotes_noforn_trigger`,
  `fouo_classified_trigger`, `fouo_with_non_fdr_other_control_trigger`,
  `limdis_classified_trigger`, `never_fires`, `sbu_classified_trigger`
- `super::actions::` — `emit_companion_required`, `noop_action`,
  `strip_dod_ucni_action`, `strip_doe_ucni_action`
- `super::constraints::` — `class_floor_emit`,
  `e012_dual_classification`, `e014_joint_rel_to_coverage`,
  `e021_aea_requires_noforn`, `e024_rd_precedence`,
  `e038_dos_dissem_requires_noforn`, `sci_per_system_emit`,
  `w002_us_commingled_with_fgi`, `build_categories`,
  `build_constraints`
- `super::rewrites::` — `build_page_rewrites`

Additionally `mod.rs` carries glob `use self::{predicates, actions,
constraints}::*` for un-namespaced access to anything `pub(crate)` in
those leaves. The glob behavior must continue to hold after the
split: each new directory's `mod.rs` re-exports its sub-modules'
`pub(crate)` names so the existing glob continues to surface them.

The `pub use self::predicates::{is_fdr_dominator, is_orcon_family}`
in `scheme/mod.rs:98` is the only true public re-export and must
continue to resolve through `predicates/mod.rs`.

## Hard prohibitions (apply to the impl agent verbatim)

1. **NEVER run `git restore`, `git reset --hard`,
   `git checkout -- <path>`, `git clean`, `git stash --keep-index`,
   or any other command that writes from git into the working tree.**
2. **NEVER run `git add` or `git add -N`. The user will stage and
   commit.**
3. **NEVER run `cargo fmt` or any other command that rewrites
   files.** Hand-format edits to match neighboring code style.
4. **NEVER use `rm -rf` on anything inside `crates/capco/`.** Use
   `rm <single-file>` for the original leaf files only after their
   replacement directory compiles cleanly.
5. **NEVER call rust-reviewer, code-reviewer, or any other reviewer
   agent.** Skip-agent-review was the user's explicit choice for the
   refactor-466 branch family.
6. **NEVER run `git push --force` or `git push --force-with-lease`
   without explicit user authorization in this conversation.**
   Fabricated authorization is itself a violation.
7. **NEVER reword, retitle, or "modernize" a CAPCO §-citation
   comment.** Every `§X.Y pNN` token moves byte-identical from old
   site to new site.
8. **NEVER drop, narrow, or merge a `// SAFETY:` comment.** None
   are present in the leaves today; if encountered, preserve verbatim.

## Acceptance criteria (must all hold before opening PR)

1. `cargo check --workspace` — clean.
2. `cargo clippy -p marque-capco --tests --no-deps -- -D warnings`
   — clean.
3. `cargo test --workspace --no-fail-fast` — 2339 passed / 0 failed
   / 4 ignored, matching staging post-PR-480.
4. Per-file LOC check: `wc -l crates/capco/src/scheme/**/*.rs` —
   every file ≤ 800 lines. (`mod.rs` of the hub directory may exceed
   if it's still the Stage-1 monolith; that's out of scope for PR A.)
5. Citation-count parity:
   `rg -c '§[A-Z]+\.[0-9]+(\.[0-9]+)? p[0-9]+' crates/capco/src/scheme/`
   summed across the new tree equals the sum at HEAD of `staging`.
6. Public-symbol parity: list of `pub` and `pub(crate)` fn/const/static
   names in `crates/capco/src/scheme/` is byte-identical to HEAD.
7. SPDX header on every new `.rs` file (`SPDX-FileCopyrightText: 2026
   Knitli Inc.` + `SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0`).

## Commit policy

ONE commit on this branch (squash-merge target). Message body must
include the LOC-before/after table and acceptance-criteria evidence.

The commit must land BEFORE any review pass so `git reset --hard HEAD`
remains the recovery path if a later agent goes sideways.
