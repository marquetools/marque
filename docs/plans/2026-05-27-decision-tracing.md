# Decision Tracing Instrumentation (`DecisionSink`)

> **Status (2026-05-28)**: Landed in [PR #810](https://github.com/marquetools/marque/pull/810) (merged as commit `900d1a3a` on `main`). Bench-check.sh ratio gate and this doc both land in [PR #811](https://github.com/marquetools/marque/pull/811).
>
> **Amended 2026-05-27** (post pre-implementation checks): closure-dispatch insertion point corrected to reflect post-#704 architecture (bitmask Kleene fixpoint + `apply_default_fill` + `apply_supersession_overlays`, not a per-rule loop). PR-conflict check clean.

## Context

People dismiss the difficulty of classification marking with "what's so hard about stamping SECRET on something?" The truthful answer is **combinatorial cascade**: every portion in a moderately complex document forces a human to evaluate ~8 category axes (classification, SCI, SAR, dissem, FGI, NATO, AEA, declass), constraint-check those tokens against every other token, and then re-evaluate the same set of axes at banner roll-up — and any edit to any portion triggers a cascade of re-evaluations across every other portion that shares affected axes.

A 50-portion document with 4 tokens/portion is in the low tens of thousands of human-equivalent marking decisions. We don't currently measure this. The engine has the structural machinery to count it (per-portion attribute evaluation, the topologically-scheduled `PageRewrite` graph, closure rules, banner roll-up) — we just need to wire an opt-in `DecisionSink` through the pipeline.

Two outputs:
1. **Counts** for the headline number ("this doc required N portion-axis decisions").
2. **Cascade trace** for the demo replay ("change one portion, watch 14 downstream markings flip"), narrated in plain English without document content.

Off by default, zero hot-path cost, content-ignorant (preserves Constitution V audit invariant: only portion indices, category IDs, enum kinds, and edge labels — never document text).

## Scope

**In scope:**
- `DecisionSink` trait + `NoopSink` (ZST default) + `CountingSink` + `RecordingSink` in `marque-scheme`.
- `DecisionEvent` type with stable IDs already present on every axis (no new ID types needed).
- Insertion at 6 pipeline sites: parser axis evaluation, per-portion rule dispatch, constraint bridge, page-rewrite execution, closure-rule execution, banner roll-up.
- `Engine::with_decision_sink(sink)` builder; `decision-tracing` feature flag on `marque-engine` + `marque-scheme`.
- `marque trace <file>` CLI subcommand with `--format=summary|ndjson|narrate`.
- Paired Criterion bench proving NoopSink adds no measurable overhead vs. baseline.

**Deferred:**
- `BatchEngine` integration — single-doc `Engine` first; batch later if asked.
- Visual rendering (HTML/SVG cascade graphs).
- Adding new closure rules — this PR instruments the existing post-#704 bitmask Kleene fixpoint (`CLOSURE_TABLE`), the `apply_default_fill` stage, and the `apply_supersession_overlays` stage by diffing before/after state at each boundary. It does not change closure semantics.

## Design

### Core types (`crates/scheme/src/decision.rs`, new file)

```rust
pub trait DecisionSink {
    fn record(&mut self, event: DecisionEvent);
}

pub struct NoopSink;
impl DecisionSink for NoopSink {
    #[inline(always)]
    fn record(&mut self, _: DecisionEvent) {}
}

pub struct DecisionEvent {
    pub step: u32,
    pub site: DecisionSite,
    pub category: CategoryId,          // reuse marque_scheme::CategoryId(u32)
    pub kind: DecisionKind,
    pub source: DecisionSource,
    pub triggered_by: Option<u32>,     // edge into prior event.step
}

pub enum DecisionSite {
    Portion(u32),
    Banner,
    Page(u32),
    Document,
}

pub enum DecisionKind {
    Evaluated,              // axis considered, no-op (the bulk of the count)
    EvaluatedSubstantive,   // axis actually inspected (rule body ran, not just dispatched)
    Mutated,
    ConstraintFired,
    RewriteScheduled,       // parent event for a multi-portion rewrite
    RewriteApplied,         // child event, one per affected portion
    ClosureFired,
    Recanonicalized,
}

pub enum DecisionSource {
    Parser,
    Constraint(&'static str),    // Constraint.name()
    PageRewrite(&'static str),   // PageRewrite.id
    Closure(&'static str),       // CLOSURE_TABLE row name (monotone implication)
    DefaultFill(&'static str),   // apply_default_fill row name (non-monotone "absent → presumed")
    Supersession(&'static str),  // apply_supersession_overlays overlay name (explicit-but-dominated)
    BannerRollup,
    RuleCheck(&'static str),     // Rule::id().predicate_id
}
```

IDs are `&'static str` everywhere because `Constraint.name()`, `PageRewrite.id`, and `ClosureRule.name` are already that type. `CategoryId(u32)` already exists. No new ID infrastructure.

### Sink implementations (`crates/scheme/src/decision/sinks.rs`)

- **`NoopSink`** — ZST with `#[inline(always)]` empty `record`. The engine's default sink when no observer is installed. Because the engine carries `Mutex<Box<dyn SyncDecisionSink>>`, each `emit()` call on the NoopSink path still incurs three residual ops: `AtomicU32::fetch_add` on the per-document step counter, `Mutex::lock` on the sink, and one vtable call to the empty `record` body. These are nanosecond-scale, which is what the 2% ratio gate budgets against the no-feature path (where the engine field is compiled out entirely).
- **`CountingSink`** — three running tallies, no per-event allocation: `by_kind` as a dense `[u64; DECISION_KIND_COUNT]` array indexed by `DecisionKind` discriminant, `by_category` as a `BTreeMap<CategoryId, u64>` (sparse — categories are scheme-extensible, no dense bound), `by_portion` as a `Vec<u64>` that grows lazily as new portion indices appear. Reports via `into_report()` which converts the dense `by_kind` array into a `BTreeMap` keyed by variant.
- **`RecordingSink`** — `Vec<DecisionEvent>` push per event. Allocates; only used in instrumentation runs. Reports via `into_report()` which walks `triggered_by` edges to reconstruct `CascadeChain`s.

### Report types

```rust
pub struct DecisionReport {
    pub total: u64,
    pub by_category: BTreeMap<CategoryId, u64>,
    pub by_kind: BTreeMap<DecisionKind, u64>,
    pub by_portion: Vec<u64>,
    pub cascade_chains: Vec<CascadeChain>,  // empty for CountingSink
    pub max_cascade_depth: u32,
}

pub struct CascadeChain {
    pub root_event: u32,
    pub root_site: DecisionSite,
    pub events: Vec<u32>,
    pub depth: u32,
}
```

`BTreeMap` is std-only; no new dependency on `marque-scheme` beyond `smallvec` + optional `serde`.

### Insertion points

| # | Site | File | Anchor | Emits |
|---|------|------|--------|-------|
| 1 | Per-portion attribute evaluation | `crates/engine/src/engine/lint_helpers.rs` `dispatch_rules_for_marking` | before/after `rule.check(attrs, &ctx)` at the per-axis level | `Evaluated` per axis on portion; `EvaluatedSubstantive` when rule body actually inspects the axis |
| 2 | Per-rule dispatch | same fn, the `rule.check` call | each rule firing | `RuleCheck` source, `Mutated` if `FixProposal` emitted |
| 3 | Constraint bridge | `crates/engine/src/engine/bridge.rs` | each constraint evaluation | `ConstraintFired` on match, `Evaluated` otherwise |
| 4 | Page-rewrite execution | `crates/capco/src/scheme/marking_scheme_impl.rs` rewrite loop | each `for rw in &self.page_rewrites` iteration | one `RewriteScheduled` parent + N `RewriteApplied` children (one per affected portion) |
| 5a | Closure operator (`close()`) | `crates/capco/src/scheme/marking_scheme_impl.rs` — instrument at the boundary in `project_attrs_pipeline`, not inside `close()` itself | diff input vs. output `FactBitmask`; for each added bit, map via static lookup to the `CLOSURE_TABLE` row that produced it | `ClosureFired` with `DecisionSource::Closure(row.name)` per added bit; no event when bitmask unchanged |
| 5b | `apply_default_fill` | `crates/capco/src/scheme/default_fill.rs` | diff before/after at the `project_attrs_pipeline` boundary; for each non-monotone "absent → presumed" rule that fired (caveated→NOFORN, NATO→REL TO USA NATO, SCI→RELIDO, US-class→RELIDO), emit one event | `Mutated` with `DecisionSource::DefaultFill(rule.name)` |
| 5c | `apply_supersession_overlays` | `crates/capco/src/lattice/dissem/` and `crates/capco/src/lattice/rel_to/` (overlay methods on `DissemSet` / `RelToBlock`) | diff before/after; for each axis cleared by §H.8 p145 NOFORN-dominates handling | `Mutated` with `DecisionSource::Supersession(overlay.name)` |
| 6 | Banner roll-up | `crates/engine/src/engine/page_context.rs` `dispatch_page_finalization` | each axis projection | `Evaluated` per axis × page; `Mutated` if banner differs from per-portion union |

**Wrinkle**: insertion points 4 and 5a/5b/5c are in `marque-capco`, not `marque-engine`. The sink threads through `MarkingScheme::project_with_sink` / `closure_with_sink` trait methods. Since the trait surface lives in `marque-scheme`, this is a default-delegating method addition (existing schemes don't break).

**Closure instrumentation strategy**: the bitmask Kleene fixpoint inside `close()` and the inner loops of `apply_default_fill` / `apply_supersession_overlays` stay untouched — instrumentation happens at the boundaries in `project_attrs_pipeline`, by diffing the marking state before and after each stage. The CLOSURE_TABLE row catalog provides a static bit→row-name lookup (`bit_to_row_name`) so emitted events carry the rule name without re-running the trigger logic. This keeps the hot path clean (no `&mut dyn DecisionSink` parameter inside `close()`) while still producing per-rule cascade narration.

### Engine wiring

`Engine` gains:
```rust
pub fn with_decision_sink<S: SyncDecisionSink + 'static>(mut self, sink: S) -> Self
```

Internally the engine carries `Mutex<Box<dyn SyncDecisionSink>>` (default `NoopSink`) and an `AtomicU32` per-document step counter. Sinks installed at scheme-projection call sites get wrapped in a `StepRemappingSink` adapter that maintains a per-call `local → global` step ID map so scheme-emitted and engine-emitted step IDs don't collide.

### Feature gating

New feature on both `marque-scheme` and `marque-engine`: `decision-tracing`. Default off. When off:
- `DecisionSink` trait + types still compile (so external callers can write code that conditionally uses them).
- `Engine::with_decision_sink` is `#[cfg(feature = "decision-tracing")]`-gated.
- The engine's `sink` field and `next_step` counter are compiled out entirely.

The CLI subcommand (`marque trace`) and the bench (`decision_tracing_overhead`) are both gated `required-features = ["decision-tracing"]` so they only compile when the feature is enabled.

Rationale: the trait surface needs to be permanently visible (for the CLI subcommand to compile against), but the engine threading is gated to guarantee SC-001 isn't affected in production builds.

### CLI subcommand (`marque/src/main.rs`)

New `Command::Trace` variant mirroring `Check`/`Fix` patterns:

```
marque trace <file> [--format=summary|ndjson|narrate]
```

- `summary` (default) — human-readable: total decisions, top categories by count, max cascade depth, count of cascade chains > depth 3.
- `ndjson` — one `DecisionEvent` per stdout line for downstream tooling.
- `narrate` — walks recorded cascade chains and emits plain-English: *"Portion 3 dissem axis: NOFORN added (rule capco:portion.dissem.requires-noforn). Triggered: closure CLOSURE_NOFORN_NONICCONTROLS on portion 7 (had LIMDIS). Triggered: banner roll-up REL TO axis recomputed across all 50 portions. Triggered: 4 portions' REL TO revised."* No document content — only structural labels.

The `narrate` form is the demo asset. Drives the rhetorical purpose.

## Verification

1. **Unit tests** (`crates/scheme/src/decision/tests.rs`):
   - `NoopSink::record` compiles to zero instructions (assert via `std::mem::size_of::<NoopSink>() == 0`).
   - `CountingSink` accumulates correctly across 1000 synthetic events.
   - `RecordingSink::into_report` reconstructs cascade chains correctly from a 3-level synthetic edge graph.

2. **Integration test** (`crates/engine/tests/decision_tracing.rs`):
   - Lint a known multi-portion fixture with `RecordingSink`.
   - Assert: total decision count > 100 (sanity floor), max cascade depth ≥ 2, every recorded `DecisionEvent.site` resolves to a real portion in the input, no event leaks document content (assert event fields are only IDs and indices — match the G13 canary at `crates/engine/tests/audit_g13_canary.rs`).

3. **Zero-overhead regression bench** (`crates/engine/benches/decision_tracing_overhead.rs`):
   - Pattern: copy `crates/engine/benches/deadline_overhead.rs` paired-bench structure.
   - Two benches: `decision_tracing_overhead_baseline` (feature ON, default NoopSink) and `decision_tracing_overhead_with_recording_sink` (feature ON, RecordingSink installed).
   - Gate: ratio ≤ 1.02 — `decision_tracing_overhead_baseline` mean over `lint_10kb` (no-feature) mean must be < 2%. Wired into `scripts/bench-check.sh::check_decision_tracing_overhead`; baseline entry `decision_tracing_overhead.max_ratio_pct` in `benches/baseline.json`.

4. **CLI smoke test** (`marque/tests/trace_cli.rs`):
   - `marque trace <fixture> --format=summary` returns exit 0 and emits a summary containing "decisions".
   - `marque trace <fixture> --format=ndjson | wc -l` matches the recorded event count.
   - `marque trace <fixture> --format=narrate` does not contain any byte sequence from the input (content-ignorance check).

5. **Demo replay** — manual: run `marque trace` against a 50-portion fixture with at least one NOFORN-implying token, verify the narrated cascade is plain English and persuasive. This is the rhetorical acceptance test.

## Files modified

- New: `crates/scheme/src/decision.rs`, `crates/scheme/src/decision/sinks.rs`, `crates/scheme/src/decision/report.rs`, `crates/scheme/src/decision/tests.rs`
- Modified: `crates/scheme/src/lib.rs` (re-exports), `crates/scheme/src/scheme.rs` (added `project_with_sink` / `closure_with_sink` trait methods with default-delegation), `crates/scheme/src/category.rs`, `crates/scheme/Cargo.toml` (new `decision-tracing` feature)
- Modified: `crates/engine/src/engine.rs` (engine struct fields, builder, `StepRemappingSink` adapter), `crates/engine/src/engine/lint_helpers.rs`, `crates/engine/src/engine/bridge.rs`, `crates/engine/src/engine/page_context.rs`, `crates/engine/src/engine/pipeline.rs`, `crates/engine/src/engine/constructors.rs`, `crates/engine/Cargo.toml` (new `decision-tracing` feature)
- Modified: `crates/capco/src/scheme/marking_scheme_impl.rs` (`project_with_sink`, `closure_with_sink`, `project_from_attrs_slice_with_sink`, `project_attrs_pipeline_with_sink` with closure-diff / default-fill-diff / supersession-diff / page-rewrite fan-out), `crates/capco/src/scheme/closure_table.rs` (`bit_to_row_name` reverse-lookup), `crates/capco/Cargo.toml` (feature proxy)
- New: `marque/src/trace.rs` (subcommand handler), modified: `marque/src/main.rs` (`Trace` variant), `marque/Cargo.toml`
- New benches + tests: `crates/engine/benches/decision_tracing_overhead.rs`, `crates/engine/tests/decision_tracing.rs`, `crates/engine/tests/decision_tracing_smoke.rs`, `marque/tests/trace_cli.rs`

## Open items (carried forward)

1. **Granularity of `EvaluatedSubstantive`** — strict-vs-loose distinction is captured in the type (two distinct kinds), but the actual "did this rule's body actually inspect this axis" signal requires rule cooperation. v1 ships with conservative emission (`EvaluatedSubstantive` only when rule emits a diagnostic or fix); v2 could let rules self-report.

2. **`BatchEngine` integration** — single-doc `Engine` is wired. The batch path is unwired; threading a `Mutex<Box<dyn SyncDecisionSink>>` through `BatchEngine` is straightforward but was deliberately out of scope for v1.

3. **Visual rendering** — narrated cascade chains in plain English ship in v1; HTML/SVG cascade-graph rendering for the demo asset is downstream tooling, not engine work.

## Resolved during pre-implementation checks (2026-05-27)

- ~~Closure-rule dispatch location~~ → Found. Post-#704 there's no per-rule loop; closure is a bitmask Kleene fixpoint with three distinct post-close stages (`close()`, `apply_default_fill`, `apply_supersession_overlays`). Plan amended to instrument all three at the `project_attrs_pipeline` boundary via before/after diff.
- ~~PR ordering against current work~~ → Clean. Only open PR was #809 (Sentinel cache-control header suggestion, unrelated). No conflicts with `dispatch_rules_for_marking` or the capco rewrite loop.
- ~~`bit_to_row_name` lookup~~ → Implemented as a `pub(crate)` static reverse-lookup in `crates/capco/src/scheme/closure_table.rs`.
