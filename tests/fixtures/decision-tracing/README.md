<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Decision-tracing demo fixtures

Hand-curated fixtures for the `marque trace` subcommand (gated behind
`--features decision-tracing`). They are not generated artifacts and are
not consumed by the corpus accuracy harness — their purpose is to drive
the `DecisionSink` instrumentation through all three of its emission
layers.

## The three emission layers

`marque trace` emits `DecisionEvent`s from three distinct points in the
pipeline. Understanding which fixture exercises which layer is the whole
purpose of having two files:

1. **Rule layer (per portion).** Every registered rule fires once per
   portion at dispatch time in the engine. Rules that mutate emit
   `Mutated` events with `DecisionSource::RuleCheck(<predicate_id>)`.
   The `relido-implied-by-closure` rule is the canonical example: it
   inspects a portion's dissem and SCI state and adds RELIDO when the
   closure-table predicate says it's implicit.
2. **Scheme layer (per page-join).** `CapcoScheme::project_attrs_pipeline_with_sink`
   runs a five-stage pipeline (`join_via_lattice → close →
   apply_default_fill → apply_supersession_overlays → page-rewrites`)
   once on the joined page accumulator and emits events by diffing the
   bitmask between stages and by fanning out per page-rewrite. This is
   where `DecisionSource::Closure`, `DefaultFill`, `Supersession`, and
   `PageRewrite` come from. Per-portion projection (`Scope::Portion`)
   is identity by design (see `CapcoScheme::project` in
   `crates/capco/src/scheme/marking_scheme_impl.rs`) — there is no
   per-portion pipeline to instrument.
3. **Engine bridge / finalization layer.** The constraint bridge
   (`crates/engine/src/engine/bridge.rs`) emits `Constraint` events
   when a `MarkingScheme::Constraint` matches; banner-rollup and
   page-finalization paths (`crates/engine/src/engine/lint_helpers.rs`,
   `crates/engine/src/engine/pipeline.rs`) emit `BannerRollup` events.

The scheme-layer **bitmask-diff** sources (`Closure`, `DefaultFill`,
`Supersession`) are structurally rare on dense multi-portion documents:
once the page-join has folded N portions, every implied bit is
typically already set by *some* portion, so `close → default-fill →
supersession` produces an empty delta and emits nothing. That collapse
is correct behavior, not a wiring bug. The `PageRewrite` fan-out
emissions still fire on dense documents because page rewrites run
unconditionally per rewrite in catalog/declaration order along the
scheme's `page_rewrites` slice (the engine's topological scheduler in
`marque-engine::scheduler` runs once at `Engine::new` to detect cycles
and validate axis annotations; the per-page emission order at the
scheme layer is the scheme's own catalog order, distinct from the
engine's construction-time topological order).

## `cascade-demo.txt`

A 39-portion synthetic intelligence assessment shaped to look like a
real document. The fixture contains no page breaks; the scanner sees
it as a single page. Drives the headline "this required N machine-
equivalent marking decisions" number (~1,200 events on this fixture
— roughly 39 portions × the registered rule count plus banner-level
dispatch).

Exercises:
- The full rule layer (one `Evaluated` event per registered rule per
  portion, plus banner-level evaluations).
- Rule-layer `RuleCheck`-source `Mutated` events from
  `relido-implied-by-closure` (across SCI / FGI / ATOMAL portions),
  `uses-banner-form` (where legacy banner-form tokens appear in
  portions), and the AEA recanonicalization rules.
- Engine-bridge `Constraint`-source events including
  `rd-frd-requires-noforn`, `joint-requires-rel-to-coverage`,
  `joint-requires-usa`, `non-us-requires-dissem`, `floor-si-comp`,
  `si-g-companions`, `hcs-system-constraints`, `ceiling-doe-ucni`,
  `dual-classification`.
- Scheme-layer `PageRewrite`-source Scheduled/Applied pairs for the
  `doe-ucni-promotes-noforn-when-classified` → `doe-ucni-evicted-by-
  classified` → `noforn-clears-rel-to` → `noforn-clears-fdr-family` →
  `noforn-clears-display-only-to` chain triggered by the `(C//UCNI)`
  portion.

Does **not** exercise: scheme-layer **bitmask-diff** sources
(`Closure` / `DefaultFill` / `Supersession`). The page-join collapses
every cone bit those stages would add. To see those firings, run
`cascade-isolation.txt`.

```bash
cargo run -p marque --features decision-tracing -- trace \
    tests/fixtures/decision-tracing/cascade-demo.txt --format summary
```

## `cascade-isolation.txt`

Four single-portion pages separated by form-feed (`\f`) page breaks. Each
page is engineered so the page-join accumulator carries exactly the bits
needed to fire one specific scheme-layer source. The form-feed bytes
reset `PageContext` between pages so the joins stay isolated.

- **Page 1** `(S//ORCON)` → `DefaultFill("capco:default-fill.dissem.caveated-implies-noforn")`
- **Page 2** `(TS//SI)` → `DefaultFill("capco:default-fill.dissem.sci-implies-relido")`
- **Page 3** `(S//REL TO USA, GBR//NF)` → `Supersession("capco:supersession.dissem.h8-p145-overlays")`
- **Page 4** `(TS//SI-G)` → `Closure(<closure-table catalog entry>)` and
  `DefaultFill("capco:default-fill.dissem.caveated-implies-noforn")`

The Page-4 `Closure` source attribution is a **bit-attribution
label**, not a causal-row label. On `(TS//SI-G)`, Row 3
(`si-g-implies-orcon`) is the closure row whose `trigger_mask` is
actually satisfied — Row 1 (`hcs-o-implies-noforn-orcon`) has an HCS-O
trigger that this portion does not match. But the `bit_to_row_name`
reverse-lookup in `crates/capco/src/scheme/closure_table.rs` walks
the catalog in declaration order and returns the first row whose
add-set contains the flipped bit, which for ORCON is Row 1. The
returned string answers "which CLOSURE_TABLE row catalogs this cone
bit?", not "which row's trigger predicate fired?". This is a
documented post-#704 attribution choice; a reader debugging "why does
my SI-G page name HCS-O?" should look at the catalog ordering, not
the trigger semantics.

```bash
cargo run -p marque --features decision-tracing -- trace \
    tests/fixtures/decision-tracing/cascade-isolation.txt --format ndjson | \
    jq -s '[.[] | select(.source | (type) == "object" and (keys[0] |
           test("Closure|Default|Supersession")))]'
```

## What the fixtures do NOT do

- They are not part of the corpus accuracy harness (`tests/corpus/`).
- They are not part of any 80%-coverage line. They live to demonstrate
  and smoke-test the `marque trace` surface.
- They are not generated. Edit them by hand when the instrumentation
  surface changes.
- They do not establish any byte-identity invariant against the engine
  pipeline. Their event totals (~1,200 on cascade-demo, ~249 on
  cascade-isolation) will drift naturally as rules and constraints are
  added or retired.
