<!--
SPDX-FileCopyrightText: 2026 Adam Poulemanos
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# scheme.rs split proposal — issue #466

This is a *draft* split. The actual extraction is a follow-up PR. The goal here is to surface where the recommended layout from #466 (rewrites / constraints / predicates / actions / scheme proper) holds, and where the coupling graph contradicts it.

## LOC targets vs. 800-line ceiling

Two columns: **as-routed** counts every impl block whole (charged to whichever module the impl shell goes to); **post option-2** projects what the LOC distribution would look like if the giant `impl CapcoScheme` were broken into free `pub(crate) fn` builders living in their respective modules.

| Module | As-routed LOC | vs. ceiling | Post option-2 LOC | vs. ceiling | Assessment |
|---|---:|---|---:|---|---|
| `scheme.rs (proper)` | 2487 | 3.1× | 2552 | 3.2× | **over ceiling even after option-2 lift — needs sub-split** |
| `scheme/rewrites.rs` | 2297 | 2.9× | 1457 | 1.8× | **over ceiling even after option-2 lift — needs sub-split** |
| `scheme/constraints.rs` | 304 | 38% | 1103 | 1.4× | **over ceiling even after option-2 lift — needs sub-split** |
| `scheme/predicates.rs` | 1737 | 2.2× | 1737 | 2.2× | **over ceiling even after option-2 lift — needs sub-split** |
| `scheme/actions.rs` | 1177 | 1.5× | 1177 | 1.5× | **over ceiling even after option-2 lift — needs sub-split** |
| `scheme/shared.rs` | 15 | 2% | 3 | 0% | fits cleanly after option-2 lift |
| `scheme/tests.rs` | 788 | 98% | 776 | 97% | near ceiling post-lift, no growth headroom |

## Risks and deviations from #466's recommended layout

### Risk 1: a single `impl CapcoScheme` block hosts multiple build-* methods

- `impl CapcoScheme` at lines 2212–4508 (2297 LOC) hosts: `new` (8 LOC), `build_page_rewrites` (1333 LOC), `build_categories` (171 LOC), `build_constraints` (661 LOC)
- `impl CapcoScheme` at lines 4829–5151 (323 LOC) hosts: `evaluate_named_constraint` (87 LOC), `fix_intent_by_name` (10 LOC), `has_diagnostic_constraints` (3 LOC), `bridge_emitted_rule_ids` (6 LOC), `bridge_sci_per_system_diagnostics` (32 LOC)
- `impl CapcoScheme` at lines 8736–8780 (45 LOC) hosts: `with_rewrites` (8 LOC), `with_extra_rewrite_for_tests` (4 LOC)

Rust doesn't allow splitting one `impl Foo { ... }` block across files. Two clean fixes:

1. **Split into multiple `impl CapcoScheme` blocks** (Rust allows multiple inherent impl blocks per type) — one block per file. Each helper method becomes `pub(crate)` in its sibling module if cross-block calls exist; `new()` lives in `scheme.rs` proper and references the per-file builders.

2. **Lift builders out of `impl CapcoScheme` entirely** — make `build_page_rewrites()`, `build_constraints()`, `build_categories()` free `pub(crate) fn` items in their respective modules, and have `CapcoScheme::new()` call them as free functions. This is the cleaner shape for a refactor focused on file-size discipline.

Recommendation: option 2. It makes the file boundary structural (each module owns its own builder) instead of cosmetic (each module owns part of one impl block).

### Risk 2: hard-case blocks (no clean home)

The following non-impl blocks reach across module boundaries. Each is a candidate to keep in `scheme/shared.rs` as `pub(crate)`, or to inline at one call site if the caller is the only consumer.

- **`class_floor_emit`** (fn, lines 6977–7059, 83 LOC) — currently routed to `scheme/constraints.rs`; reaches into: scheme.rs (proper), scheme/predicates.rs.
  - doc: "Single source of truth for the class-floor catalog's"
  - **recommendation**: lift to `scheme/shared.rs` as `pub(crate) fn`; large enough that duplication isn't viable.
- **`emit_companion_insert`** (fn, lines 8136–8207, 72 LOC) — currently routed to `scheme/actions.rs`; reaches into: scheme.rs (proper), scheme/predicates.rs.
  - **recommendation**: lift to `scheme/shared.rs` as `pub(crate) fn`; large enough that duplication isn't viable.
- **`emit_hcs_o_companions`** (fn, lines 8297–8363, 67 LOC) — currently routed to `scheme/actions.rs`; reaches into: scheme.rs (proper), scheme/predicates.rs.
  - doc: "Row #1 — HCS-O companions: requires ORCON + NOFORN, forbids"
  - **recommendation**: lift to `scheme/shared.rs` as `pub(crate) fn`; large enough that duplication isn't viable.
- **`emit_hcs_p_sub_companions`** (fn, lines 8365–8420, 56 LOC) — currently routed to `scheme/actions.rs`; reaches into: scheme.rs (proper), scheme/predicates.rs.
  - doc: "Row #3 — HCS-P sub-compartment companions: requires ORCON, forbids"
  - **recommendation**: lift to `scheme/shared.rs` as `pub(crate) fn`; large enough that duplication isn't viable.
- **`emit_si_g_companions`** (fn, lines 8422–8474, 53 LOC) — currently routed to `scheme/actions.rs`; reaches into: scheme.rs (proper), scheme/predicates.rs.
  - doc: "Row #4 — SI-G companions: requires ORCON, forbids ORCON-USGOV."
  - **recommendation**: lift to `scheme/shared.rs` as `pub(crate) fn`; large enough that duplication isn't viable.
- **`emit_companion_required`** (fn, lines 8480–8557, 78 LOC) — currently routed to `scheme/actions.rs`; reaches into: scheme.rs (proper), scheme/predicates.rs.
  - doc: "Single-token companion insertion. Used by `CompanionRequired`-kind"
  - **recommendation**: lift to `scheme/shared.rs` as `pub(crate) fn`; large enough that duplication isn't viable.
- **`sci_per_system_emit`** (fn, lines 8584–8616, 33 LOC) — currently routed to `scheme/constraints.rs`; reaches into: scheme.rs (proper), scheme/actions.rs.
  - doc: "Single source of truth for the SCI per-system catalog's emit logic."
  - **recommendation**: lift to `scheme/shared.rs` as `pub(crate) fn`; large enough that duplication isn't viable.
- **`SCI_PER_SYSTEM_CATALOG`** (const, lines 8665–8721, 57 LOC) — currently routed to `scheme.rs (proper)`; reaches into: scheme/actions.rs, scheme/predicates.rs.
  - **recommendation**: lift to `scheme/shared.rs` as `pub(crate) fn`; large enough that duplication isn't viable.

### Risk 3: #466's LOC estimates vs. measured

Issue #466 estimates and what we actually measure (post option-2):

| Module | #466 estimate | Measured (post option-2) | Verdict |
|---|---|---:|---|
| `scheme/rewrites.rs` | 2000–3000 LOC | 1457 | under low-end — may be too small for a dedicated file |
| `scheme/constraints.rs` | 1500–2000 LOC | 1103 | under low-end — may be too small for a dedicated file |
| `scheme/predicates.rs` | 500–1000 LOC | 1737 | **over high-end — sub-split recommended** |
| `scheme/actions.rs` | 500–800 LOC | 1177 | **over high-end — sub-split recommended** |
| `scheme.rs (proper)` | 500–1000 LOC | 2552 | **over high-end — sub-split recommended** |

Several modules exceed even the high-end estimate. Each over-ceiling module needs a concrete sub-split plan. Candidate sub-splits, by module:

- **`scheme.rs (proper)`** (2552 LOC post-lift)
  - largest items:
    - `join_via_lattice` (397 LOC, lines 348–744)
    - `CLASS_FLOOR_CATALOG` (344 LOC, lines 7499–7842)
    - `build_categories` (171 LOC, lines 3675–3845)
    - `project` (145 LOC, lines 5328–5472)
    - `RENDER_TABLE` (72 LOC, lines 6307–6378)
    - `render_canonical` (67 LOC, lines 5501–5567)
  - scheme.rs is too big at 2.5×; pull token/category id constants into `scheme/ids.rs` (cuts ~120 LOC), and consider moving the `impl MarkingScheme for CapcoScheme` block to `scheme/marking_scheme_impl.rs` (cuts ~552 LOC). What remains (constants + struct defs + `new()` + the small Debug/Default/PartialEq/Eq/From/Lattice impls) fits under the ceiling.

- **`scheme/rewrites.rs`** (1457 LOC post-lift)
  - largest items:
    - `build_page_rewrites` (1333 LOC, lines 2329–3661)
  - sub-split candidates: by §-section (`rewrites/h6.rs` AEA, `rewrites/h8.rs` dissem, `rewrites/h9.rs` non-IC dissem) OR by pattern (`rewrites/pattern_a_noforn_supremacy.rs`, `rewrites/pattern_b_fouo_eviction.rs`, `rewrites/pattern_c_classified_strip.rs`, `rewrites/pattern_d_caveated_to_noforn.rs`). The pattern-based split aligns with the existing build_page_rewrites doc-comment grouping (see lines 2222–2273).

- **`scheme/constraints.rs`** (1103 LOC post-lift)
  - largest items:
    - `build_constraints` (661 LOC, lines 3847–4507)
    - `evaluate_named_constraint` (87 LOC, lines 4855–4941)
    - `class_floor_emit` (83 LOC, lines 6977–7059)
    - `e021_aea_requires_noforn` (42 LOC, lines 6480–6521)
    - `e012_dual_classification` (41 LOC, lines 6398–6438)
    - `sci_per_system_emit` (33 LOC, lines 8584–8616)
  - sub-split candidates: split `build_constraints` from its helpers (`e0XX_*` rule emitters into `constraints/rule_emitters.rs`, class-floor catalog into `constraints/class_floor.rs`, SCI per-system catalog into `constraints/sci_per_system.rs`).

- **`scheme/predicates.rs`** (1737 LOC post-lift)
  - largest items:
    - `satisfies_attrs` (249 LOC, lines 4538–4786)
    - `hcs_system_constraints` (211 LOC, lines 6630–6840)
    - `collect_present_tokens` (144 LOC, lines 5718–5861)
    - `capco_token_category` (77 LOC, lines 1139–1215)
    - `class_floor_satisfied` (53 LOC, lines 7124–7176)
    - `dissem_to_tok` (47 LOC, lines 2012–2058)
  - sub-split candidates: by predicate family — `predicates/presence.rs` (the ~25 `presence_*` fns), `predicates/triggers.rs` (the `*_trigger` family), `predicates/satisfies.rs` (`satisfies_attrs` + helpers), `predicates/class_floor.rs` (the class-floor catalog evaluator).

- **`scheme/actions.rs`** (1177 LOC post-lift)
  - largest items:
    - `apply_fact_remove` (234 LOC, lines 1470–1703)
    - `apply_fact_add` (159 LOC, lines 1310–1468)
    - `apply_intent_to_marking` (92 LOC, lines 1217–1308)
    - `extract_foreign_sources` (87 LOC, lines 747–833)
    - `emit_companion_required` (78 LOC, lines 8480–8557)
    - `capco_category_contains` (77 LOC, lines 1005–1081)
  - sub-split candidates: `actions/intent.rs` (`apply_intent_to_marking` + `apply_fact_add` + `apply_fact_remove`), `actions/category_ops.rs` (`capco_category_*` helpers), `actions/companions.rs` (`emit_*_companions` + `emit_companion_insert`), `actions/strip.rs` (`strip_*_ucni_action`).

### Risk 4: `pub` surface preservation

Issue #466's acceptance criteria forbid new `pub` symbols. The proposed split keeps every helper currently inside `impl CapcoScheme` as a free `pub(crate)` fn in the destination module (option 2 above). `pub(crate)` is fine; `pub` is not. Below is the list of currently-private free functions that the proposed split would need to elevate to `pub(crate)` so a sibling module can call them:

- `apply_intent_to_marking`
- `capco_category_clear`
- `capco_category_contains`
- `capco_category_has_values`
- `capco_category_replace`
- `class_floor_emit`
- `class_floor_satisfied`
- `dissem_token_id_for_form`
- `dod_ucni_classified_trigger`
- `dod_ucni_promotes_noforn_trigger`
- `doe_ucni_classified_trigger`
- `doe_ucni_promotes_noforn_trigger`
- `e012_dual_classification`
- `e014_joint_rel_to_coverage`
- `e021_aea_requires_noforn`
- `e024_rd_precedence`
- `e038_dos_dissem_requires_noforn`
- `emit_companion_required`
- `emit_hcs_o_companions`
- `emit_hcs_p_sub_companions`
- `emit_si_g_companions`
- `evaluate_custom_by_attrs`
- `extract_foreign_sources`
- `fouo_classified_trigger`
- `fouo_with_non_fdr_other_control_trigger`
- `limdis_classified_trigger`
- `merge_fgi_markers`
- `never_fires`
- `noop_action`
- `page_context_to_attrs`
- `presence_atomal`
- `presence_balk`
- `presence_bohemia`
- `presence_dod_ucni`
- `presence_doe_ucni`
- `presence_eyes_only`
- `presence_frd_bare`
- `presence_frd_sigma`
- `presence_hcs_comp_only`
- `presence_hcs_comp_sub`
- `presence_hcs_o`
- `presence_hcs_p_any`
- `presence_hcs_p_sub`
- `presence_imcon`
- `presence_orcon_family`
- `presence_passthrough_bur`
- `presence_passthrough_hcs_x`
- `presence_passthrough_klm`
- `presence_passthrough_mvl`
- `presence_rd_bare`
- `presence_rd_cnwdi`
- `presence_rd_sigma`
- `presence_rsen`
- `presence_rsv_comp`
- `presence_sar`
- `presence_si_bare`
- `presence_si_comp`
- `presence_si_g`
- `presence_tfni`
- `presence_tk_blfh`
- `presence_tk_compartment_noforn`
- `presence_tk_family`
- `satisfies_attrs`
- `sbu_classified_trigger`
- `strip_dod_ucni_action`
- `strip_doe_ucni_action`
- `w002_us_commingled_with_fgi`

## Per-module write-up

### `scheme.rs (proper)`

Target LOC: **2487** across 87 blocks.

Top blocks by LOC:
- `impl MarkingScheme for CapcoScheme` (impl, 552 LOC, lines 5165–5716)
- `impl CapcoMarking` (impl, 449 LOC, lines 297–745)
- `CLASS_FLOOR_CATALOG` (const, 344 LOC, lines 7499–7842)
- `impl CapcoScheme` (impl, 323 LOC, lines 4829–5151)
- `impl Lattice for CapcoMarking` (impl, 73 LOC, lines 917–989)

### `scheme/rewrites.rs`

Target LOC: **2297** across 1 blocks.

Top blocks by LOC:
- `impl CapcoScheme` (impl, 2297 LOC, lines 2212–4508)

### `scheme/constraints.rs`

Target LOC: **304** across 8 blocks.

Top blocks by LOC:
- `class_floor_emit` (fn, 83 LOC, lines 6977–7059)
- `e021_aea_requires_noforn` (fn, 42 LOC, lines 6480–6521)
- `e012_dual_classification` (fn, 41 LOC, lines 6398–6438)
- `sci_per_system_emit` (fn, 33 LOC, lines 8584–8616)
- `e024_rd_precedence` (fn, 31 LOC, lines 6552–6582)

### `scheme/predicates.rs`

Target LOC: **1737** across 77 blocks.

Top blocks by LOC:
- `satisfies_attrs` (fn, 249 LOC, lines 4538–4786)
- `hcs_system_constraints` (fn, 211 LOC, lines 6630–6840)
- `collect_present_tokens` (fn, 144 LOC, lines 5718–5861)
- `capco_token_category` (fn, 77 LOC, lines 1139–1215)
- `class_floor_satisfied` (fn, 53 LOC, lines 7124–7176)

### `scheme/actions.rs`

Target LOC: **1177** across 18 blocks.

Top blocks by LOC:
- `apply_fact_remove` (fn, 234 LOC, lines 1470–1703)
- `apply_fact_add` (fn, 159 LOC, lines 1310–1468)
- `apply_intent_to_marking` (fn, 92 LOC, lines 1217–1308)
- `extract_foreign_sources` (fn, 87 LOC, lines 747–833)
- `emit_companion_required` (fn, 78 LOC, lines 8480–8557)

### `scheme/shared.rs`

Target LOC: **15** across 1 blocks.

Top blocks by LOC:
- `impl CompanionForm` (impl, 15 LOC, lines 7903–7917)

### `scheme/tests.rs`

Target LOC: **788** across 2 blocks.

Top blocks by LOC:
- `tests` (mod, 743 LOC, lines 8782–9524)
- `impl CapcoScheme` (impl, 45 LOC, lines 8736–8780)

