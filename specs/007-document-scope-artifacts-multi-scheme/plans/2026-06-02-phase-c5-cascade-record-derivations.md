# Phase C5 (T035) — Cascade-record derivations via `DecisionSink`

**Branch:** `007-phase-c5` (off merged main `b37e10fa`, after C4 #853).
**Maps:** FR-012 → T035 → SC-007 (cascade). Builds on C4's `resolve_document`.
**Crates touched:** `marque-scheme` (leaf, decision surface) + `marque-engine`.
This is an engine-instrumentation PR; editing the engine is in-scope (the
"scheme-adoption PR must not edit the engine" rule, Constitution IV, binds
domain scheme-adoption PRs, not the engine's own derivation layer).

## Goal

T035: "Cascade-record derivations via existing `DecisionSink`
(`DecisionEvent::triggered_by`). Tests: cascade chain reconstructs; G13 canary
green." FR-012: every derivation that **fires** MUST be recorded
content-ignorantly via the `DecisionSink` cascade, with `triggered_by` linking
which edge fired.

C4's `resolve_document` already builds `firing_edges: Vec<&DerivationEdge>` in
scheduled order (writers-before-readers) and filters on `firing_active`. C5
emits one content-ignorant `DecisionEvent` per firing edge, threading the
cascade through `triggered_by`.

## Design (both planners converged)

### marque-scheme (decision surface — always compiles; the `decision-tracing`
feature is scheme-side a no-op marker)

1. `crates/scheme/src/decision.rs`
   - Append `DecisionKind::Derived` as the LAST variant (after
     `Recanonicalized`). Reusing `Mutated` / `Evaluated*` / `Rewrite*` would
     mislabel — resolution is decoupled from fixing (no value mutates, #799),
     and a derivation is none of the rule/constraint/closure subsystems. The
     honest variant is correct; `DecisionKind` is not a frozen surface, and
     append preserves the `Ord`/`BTreeMap` rendering ("Don't reorder").
   - Append `DecisionSource::Derivation(&'static str)` as the LAST variant
     (after `RuleCheck`). Carries the edge's `EdgeId` (`= &'static str`) — the
     same content-neutral stable-label class as `RewriteId`/`Closure`/
     `RuleCheck`. Extend the content-ignorance doc (the `DecisionSource`
     doc-comment example list) to name `DerivationEdge` ids.
   - **Size pin holds:** `DecisionEvent` stays 56 bytes. The `&'static str`
     fat-pointer layout floor already exists in `DecisionSource`; a fieldless
     `DecisionKind` variant adds nothing. `const_assert_eq!(… == 56)` stays.

2. `crates/scheme/src/decision/sinks.rs`
   - `DECISION_KIND_COUNT: 8 → 9`.
   - `discriminant_index` (NOTE: actual fn name, not `kind_index`): add
     `DecisionKind::Derived => 8`. The exhaustive match is compiler-forced.
   - `KIND_ORDER`: append `DecisionKind::Derived` (length follows the count;
     mismatch is a compile error).
   - `DecisionSource` needs **no** change here (no dense-index machinery).

3. `crates/scheme/src/decision/tests.rs`
   - Append `DecisionKind::Derived` to the all-kinds array in
     `counting_sink_accumulates_totals_by_kind_category_and_portion`.
   - **Required follow-on:** that test's `total_events = 1000` no longer
     divides by 9 kinds. Set it to **900** (LCM(9 kinds, 5 categories,
     10 portions) = 90; 900/9=100, 900/5=180, 900/10=90 — all exact). Update
     the divisibility comment.
   - Add a unit assertion: `discriminant_index(Derived) == 8`,
     `KIND_ORDER[8] == Derived`, `KIND_ORDER.len() == DECISION_KIND_COUNT == 9`.
   - Serde: variant-additive, no `#[serde(...)]` attr; `Derived` → `"Derived"`,
     `Derivation("…")` → `{"Derivation":"…"}` (same shape as `Constraint`).

### marque-engine (emission)

4. `crates/engine/src/engine.rs` — add a feature-gated helper in the existing
   `#[cfg(feature = "decision-tracing")] impl<S, R> Engine<S, R>` block:

   ```rust
   #[cfg(feature = "decision-tracing")]
   fn record_derivation_cascade(&self, firing_edges: &[&marque_scheme::DerivationEdge]) {
       use marque_scheme::{DecisionEvent, DecisionKind, DecisionSite, DecisionSource};
       use marque_scheme::category::CategoryId;
       if !self.tracing_active() { return; }            // hot-path gate first
       let mut last_writer: std::collections::HashMap<CategoryId, u32> =
           std::collections::HashMap::new();
       self.with_sink(|sink| {                           // lock once for the whole cascade
           for edge in firing_edges {
               // trigger = most-recent edge that wrote a category THIS edge reads.
               // Computed BEFORE inserting this edge's own writes → no self-loop.
               let triggered_by = edge.reads.iter()
                   .filter_map(|c| last_writer.get(c).copied())
                   .max();
               let step = self.next_step.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
               sink.record(DecisionEvent {
                   step,
                   site: DecisionSite::Document,
                   category: edge.writes.first().copied().unwrap_or(CategoryId::MARKING),
                   kind: DecisionKind::Derived,
                   source: DecisionSource::Derivation(edge.id),
                   triggered_by,
               });
               for &w in edge.writes { last_writer.insert(w, step); }
           }
       });
   }
   ```

   - Step-capture: `with_sink` + direct `next_step.fetch_add` (mirrors the
     sanctioned `with_remapping_sink` idiom). Do **not** add an
     `emit_returning_step`; `emit` hides the step and locks per call.
     Borrow-clean: closure shares `&self` (for `next_step`); `sink` is disjoint.
   - Record **all** firing edges (FR-012 = "every derivation that fires"),
     not only edges mapping to a declared artifact kind. Emit during the
     `firing_edges` walk, once per edge — an edge writing multiple categories
     emits one event and updates `last_writer` for all its writes.

5. `crates/engine/src/engine/constructors.rs` — call site in `resolve_document`,
   immediately after `firing_edges` is collected, before the per-kind loop:

   ```rust
   #[cfg(feature = "decision-tracing")]
   self.record_derivation_cascade(&firing_edges);
   ```

   - OFF-feature: `firing_edges` is still consumed by the per-kind `producing`
     filter, so no `unused_variables`. The helper + imports don't compile.
   - Reword `resolve_document`'s "Pure:" doc sentence to "Value-pure: mutates
     no marking value and emits no diagnostics; when `decision-tracing` is on
     and a sink is installed, records content-ignorant observability
     `DecisionEvent`s through the engine's interior-mutable sink (the same
     surface every other engine decision point uses, Constitution V) — the
     recorded events never alter the returned `ResolvedDocument`."
   - Add a note that the cascade is a **tree projection of the DAG**: a single
     `triggered_by` parent attributes to the latest-arriving dependency (see
     CRUX below). Every edge still emits its own firing record.

## Cruxes (resolved)

- **triggered_by = `max()`** of read-categories' last-writer steps. Steps mint
  monotonically in scheduled order, so numeric max = most-recent writer.
  Deterministic (scheduled order fixed at `Engine::new`; no HashMap iteration
  order leaks). Chain A→B→C reconstructs exactly. **Diamond** (D reads Y,Z from
  B,C) attributes D to C only (`max(sB,sC)=sC` since writers-before-readers puts
  B before C) — the reconstruction is a **spanning tree of the DAG**, inherent
  to `triggered_by: Option<u32>` (one parent per event), not a defect. FR-012 is
  satisfied: every edge emits its own `Derivation(id)` firing record; only
  parent attribution is single-valued. Documented + tested intentionally.
- **Step counter:** `reset_decision_step_counter()` runs at lint entry;
  `resolve_document` runs at EOD in the same lint call after all per-portion/
  per-page emits. Cascade steps continue the document's monotone sequence; no
  reset inside `resolve_document` (would orphan earlier steps).
- **CAPCO no-op:** CAPCO declares no document artifacts → `resolve_document`
  early-returns before the helper. Zero derivation events. G13 stays green by
  construction.
- **G13:** the canary scans `Engine::fix` `AppliedFix` NDJSON, a different
  stream. C5 touches neither `AppliedFix` nor `Engine::fix`. No canary edit;
  must not regress.

## Test plan (all in a NEW file `crates/engine/tests/document_resolution_cascade.rs`
with `required-features = ["decision-tracing"]` + a `[[test]]` entry in
`crates/engine/Cargo.toml`)

Retrieval pattern (confirmed from `decision_tracing.rs`): `with_decision_sink`
consumes the sink behind `Mutex<Box<dyn …>>` and does not hand it back. Define a
tiny `Inspectable { events: Arc<Mutex<Vec<DecisionEvent>>> }` sink (copy the
~15-line struct+impl from `decision_tracing.rs`), keep an `Arc` clone before
moving it in, read events after `lint`, reconstruct via
`RecordingSink::into_report_from_events(events.iter().copied().collect())`.

Stub: copy C4's `StubScheme`/`StubMarking`/`StubRecognizer`/`build` scaffolding
from `document_resolution.rs` into the new file (integration tests are separate
binaries). Add `CAT_X=CategoryId(1)`, `CAT_Y=CategoryId(2)`, `CAT_Z=CategoryId(3)`
and a `chained_edge(id, reads, writes)` helper. Three `Always`/`Rollup` edges:
A(reads `&[]`, writes `&[CAT_X]`), B(reads `&[CAT_X]`, writes `&[CAT_Y]`),
C(reads `&[CAT_Y]`, writes `&[CAT_Z]`). Map three `ArtifactKind`s to the three
categories so `document_artifacts()` is non-empty (else early-return). Confirm
three distinct `ArtifactKind` variants exist (grep before finalizing).

- `sc007_cascade_chain_reconstructs_three_levels` — install `Inspectable`,
  `lint(b"text with no markings\n")` (StubRecognizer recognizes nothing → reaches
  EOD `resolve_document`). Filter to `kind == Derived` (exactly 3). Assert each
  carries `source == Derivation(<id>)`, `site == Document`, `category == CAT_X/Y/Z`;
  A `triggered_by == None`, B `== Some(A.step)`, C `== Some(B.step)`. Build
  `into_report_from_events` and assert one `CascadeChain`, `root == A.step`,
  `depth == 2`, `events.len() == 3`.
- `sc007_diamond_attributes_to_latest_dependency` — add edge D(reads
  `&[CAT_Y, CAT_Z]`, writes `&[CAT_W]`); assert D `triggered_by == Some(C.step)`
  (pins the tree-projection decision as an intentional, tested property).
- `sc007_derivation_emits_nothing_without_observer` — default `NoopSink` (no
  `with_decision_sink`); assert `LintResult.resolved_document` still resolves the
  artifacts correctly (emission is side-effect-free on the result; the
  `tracing_active` gate short-circuits).
- `capco_emits_no_derivation_events` — real `CapcoScheme` engine + `Inspectable`
  over a marked document; assert no `Derived`-kind event observed.
- G13: existing `audit_g13_canary.rs` unchanged, must stay green.
- Size pin: `const_assert_eq!(… == 56)` stays; index test (Step 3) pins the value.

## Verify (toolchain is 1.89; CI clippy via `rustup run stable`, NOT `cargo +stable`)

```
rustup run 1.89 cargo check -p marque-scheme -p marque-engine --tests
rustup run 1.89 cargo test  -p marque-scheme decision
rustup run 1.89 cargo test  -p marque-engine --features decision-tracing
rustup run 1.89 cargo test  -p marque-engine                 # OFF build + G13 green
rustup run stable cargo clippy -p marque-scheme -p marque-engine --all-targets -- -D warnings
rustup run stable cargo clippy -p marque-engine --all-targets --features decision-tracing -- -D warnings
rustup run stable cargo fmt --check
# Also confirm WASM-safe leaf still builds:
rustup run 1.89 cargo check -p marque-scheme
```

## Constitution check

- **V / G13:** every new field value is an ID/enum/`&'static str` EdgeId — no
  document content. Canary target (`Engine::fix`) untouched; CAPCO no-op test
  is the regression guard.
- **VI:** `last_writer` is per-invocation scratch; no `static mut`; `Send+Sync`
  preserved (no new fields).
- **VII:** additions stay in the scheme leaf; engine already depends on scheme.
  No new deps, no cycle.
- **III (WASM):** `marque-scheme` stays WASM-safe (plain enum variants;
  `decision-tracing` is engine-side).
- **VIII:** engine infra, not a CAPCO rule — no citation needed; the
  `DerivationEdge` already carries its own `Citation`, so edge provenance is
  preserved without duplicating it into the trace.
