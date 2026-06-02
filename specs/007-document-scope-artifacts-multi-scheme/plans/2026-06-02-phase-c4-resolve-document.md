# Phase C4 — `resolve_document` decoupled from fix (T033 / T033b / T034 / T034b)

**Branch** `007-phase-c4` (off merged main `11dd357a`, contains C1/C2/C3). **Engine + scheme PR.**
Synthesized 2026-06-02 from parallel system-architect + rust-specialist plans, reconciled
against verified as-built source (`fix_intent.rs`, `artifact.rs`, `provenance.rs`,
`derivation.rs`, `scheme.rs`, `pipeline.rs`, `output.rs`, `scheduler.rs`, `constructors.rs`).

## Scope

| Task | Delivers |
|------|----------|
| T033 (FR-011) | `resolve_document` runs ALWAYS (decoupled from fixing); firing-predicate gating w/ mode placeholder. Surfaced on `LintResult` so `lint()` (fixing off) carries it. |
| T033b (FR-014) | A `WhenMode` edge stays in the `Engine::new` DAG (already true post-C3) and is skipped ONLY at firing time inside `resolve_document` — never a topology swap. |
| T034 (FR-013) | Fixability follows derivability: absent + firing producing edge → `Fixability::Fixable` (+ derived value); absent + no producing edge → `Fixability::FlagOnly`. SC-007 paired harness. |
| T034b (US2 Sc.1) | `resolve_document` returns the actual DERIVED VALUE (`S::Canonical`) for an absent node with an inbound **rollup** edge. |

**Out of scope** (later PRs): C5/T035 (DecisionSink cascade `triggered_by`, FR-012); C6/T036
(reverse validation + `FrontMarking`, FR-015). `DerivationRelation::SourceDerived` (bundle
source-date-max) is **#823-DEFERRED** — do NOT implement.

## Resolved design cruxes

### Crux 1 — Artifact↔edge association: `artifact_category` on `MarkingScheme` (defaulted)
`document_artifacts()` returns bare `&[ArtifactKind]`; `derivation_edges()` keys by
`writes: &[CategoryId]`. Add **`MarkingScheme::artifact_category(&self, ArtifactKind) ->
Option<CategoryId>`** (default `None`) — the join key. An edge produces a node of `kind` iff
`edge.writes.contains(&scheme.artifact_category(kind))`. **On `MarkingScheme`, NOT
`SchemeArtifacts`**, so resolution stays on bare `MarkingScheme` (no `ArtifactPayload` bound, no
`ArtifactBridge` blanket-impl coherence risk). Additive defaulted method — same shape as the
Phase-0 `document_artifacts`/`derivation_edges` additions. This per-kind association is what makes
SC-007 honest: kind-A maps to category-A (its rollup edge writes category-A), kind-B maps to
category-B (no edge writes it) → A fixable, B flag-only. (A blanket "every producing edge → every
kind" association would wrongly make B fixable.)

### Crux 2 — `resolve_document` is pure; result surfaced on `LintResult`
- Pure-data result types live in **`marque-scheme`** (`crates/scheme/src/resolution.rs`), generic
  over `S: MarkingScheme + ?Sized`, carrying `Option<S::Canonical>` (NOT `S::ArtifactPayload` — C4
  has no present payload; mirrors C2's deliberate `SchemeArtifacts` deferral).
- `resolve_document` (the algorithm) is an **`Engine` method** (needs `scheduled_steps()` +
  `active_modes` + the rollup). It is **pure** (FR-011: no mutation) and emits no diagnostics.
- The lint pipeline computes it at EOD and surfaces it on **`LintResult<S>`** so "resolution
  present with fixing off" is observable via the normal `lint()` flow, and `fix()` inherits it for
  free (fix re-lints through the same internal pipeline).

### Crux 3 — Mode placeholder
Engine carries `active_modes: BTreeSet<&'static str>`, **empty by default**. `firing_active`:
`Always => true`; `WhenMode(m) => active_modes.contains(m)` (empty ⇒ never). No public
mode-setter ships (YAGNI; the real taxonomy is Phase F/#645). The positive "fires-when-active"
half is tested via an **engine-internal unit test** (`pub(crate)`/direct field set) — NOT a
`#[doc(hidden)] pub` setter (per project feedback: doc-hidden-pub is still public API).

### Crux 4 — Absence without a doc-artifact recognizer
Document-artifact present-parse (CAB-as-node) is **Phase D** (T042). In C4 ALL declared artifacts
are treated as absent; `resolve_document` consumes only kinds + edges + schedule +
`artifact_category` + active modes + the rollup. No Phase-D dependency. Present-state modeling
arrives with the recognizer in Phase D/C6.

### Crux 5 — NO fix-stream emission in C4 (honesty)
`ReplacementIntent` is a closed 4-variant fact-delta vocab (`FactAdd`/`FactRemove`/
`Recanonicalize`/`Relocate`). **None honestly represents "insert a rendered absent document
artifact."** `Recanonicalize` re-renders *existing* text; using it at a zero-width span for a
genuinely-absent node would be a semantically-wrong stub (violates coding-style "no stubs" +
Constitution VIII). Real placement + serialization is **Phase D** (T042/T044). So C4 ships at the
**resolution-classification level**: `resolve_document` returns `Fixability::{Fixable,FlagOnly}` +
`derived_value: Option<S::Canonical>` (the "fill"). The SC-007 harness asserts on the
`ResolvedDocument`. No engine sentinel `RuleId`s, no `MessageTemplate` additions (no audit-schema
concern), no insertion `FixIntent`. The translation of resolved nodes into emitted
fix-carrying diagnostics is deferred to Phase D (when a real artifact + placement + serializer
exist).

## Implementation steps (TDD; scheme crate first per Constitution dev-sequence)

### Step 1 — `marque-scheme`: `artifact_category` on `MarkingScheme`
`crates/scheme/src/scheme.rs`: add defaulted `fn artifact_category(&self, _kind: ArtifactKind) ->
Option<CategoryId> { None }` (doc: the join key between `document_artifacts()` and
`derivation_edges().writes`). Unit test: default returns `None` for every kind.

### Step 2 — `marque-scheme`: resolution result types (new `crates/scheme/src/resolution.rs`)
- `#[non_exhaustive] enum Fixability { Fixable, FlagOnly }` (Debug/Clone/Copy/PartialEq/Eq/Hash).
- `struct ResolvedArtifact<S: MarkingScheme + ?Sized> { kind: ArtifactKind, fixability: Fixability,
  derived_value: Option<S::Canonical>, fired_edges: Box<[EdgeId]> }` — hand-written
  Debug/Clone/PartialEq over `S::Canonical` bounds (mirror `DocumentArtifact` manual impls; a
  derive over-constrains `S`).
- `struct ResolvedDocument<S: MarkingScheme + ?Sized> { artifacts: Box<[ResolvedArtifact<S>]> }`
  with `artifacts()` accessor, `is_empty()`, hand-written Debug/Clone/PartialEq + manual `Default`
  (empty box — no `S: Default`).
- Wire `pub mod resolution;` + `pub use` in `crates/scheme/src/lib.rs`.
- Unit tests: `Fixability` variants distinct; `ResolvedArtifact` carries/omits derived_value;
  `ResolvedDocument` holds nodes + `is_empty`; manual Clone/PartialEq field-distinguishing pin
  (mirror `document_artifact_eq_distinguishes_each_field`); `Default` is empty. Use a local
  closed-vocab stub scheme (`Canonical = u32`) so derived_value is assertable.

### Step 3 — `marque-engine`: mode placeholder
- `crates/engine/src/engine.rs`: add `active_modes: std::collections::BTreeSet<&'static str>`.
- `crates/engine/src/engine/constructors.rs`: init `active_modes: BTreeSet::new()` in
  `with_clock_and_recognizer`; add `pub fn active_modes(&self) -> impl Iterator<Item=&'static str>`
  accessor + private `fn firing_active(&self, fp: FiringPredicate) -> bool` on the
  `impl<S: MarkingScheme, R: Recognizer<S>>` accessor block. Add a `pub(crate) fn
  set_active_modes_for_test(&mut self, …)` (crate-internal, for the unit test only).
- Confirm the existing `static_assertions` `Send + Sync` engine assertion still compiles
  (`BTreeSet<&'static str>` is `Send + Sync`).
- Engine-internal unit tests: `active_modes` default empty; `firing_active(Always)==true`;
  `firing_active(WhenMode("x"))==false` by default; after `set_active_modes_for_test(["x"])`,
  `firing_active(WhenMode("x"))==true` (the positive firing half of T033b).

### Step 4 — `marque-engine`: `resolve_document`
On the `impl<S: MarkingScheme, R: Recognizer<S>> Engine<S,R>` accessor block (add where-clause
`S::Canonical: Clone`):
```
pub fn resolve_document(&self, doc_rollup: &S::Canonical) -> ResolvedDocument<S>
```
Algorithm:
1. `let kinds = self.scheme.document_artifacts(); if kinds.is_empty() { return empty }` (CAPCO
   no-op at the source).
2. Build the firing edge set by walking `self.scheduled_steps()` in order; for each
   `ScheduledStep::DerivationEdge(id)` resolve `&DerivationEdge` from `derivation_edges()` and keep
   it iff `self.firing_active(edge.firing)`. (Walking `scheduled_steps` honors
   writers-before-readers; a `WhenMode` edge that doesn't fire is *skipped here*, never removed
   from the DAG — T033b.)
3. For each `kind`: `cat = self.scheme.artifact_category(kind)`; producing = firing edges whose
   `writes.contains(cat)` (skip kinds with `cat == None`). A node is `Fixable` iff ≥1 producing
   edge has a value-producing relation (**C4: `Rollup`**; `CannedString`/`Passthrough` join later,
   `SourceDerived` is #823-deferred); `derived_value = Some(doc_rollup.clone())` for a firing
   `Rollup` edge. Else `FlagOnly` + `None`. `fired_edges` = all firing producing edge ids.
4. Collect `ResolvedDocument`.

Engine unit tests (or the integration test in Step 6): T033 (resolution present, no fix run);
T033b skip-half (WhenMode edge in `scheduled_steps()` but node FlagOnly); T034b (Rollup →
`derived_value == Some(rollup)`); `source_derived_yields_no_value` deferral pin.

### Step 5 — `marque-engine`: surface on `LintResult` + pipeline wiring
- `crates/engine/src/output.rs`: add `pub resolved_document: ResolvedDocument<S>` to `LintResult`;
  update the manual `Default` (empty) and `Clone` impls; `#[derive(Debug)]` needs
  `ResolvedDocument<S>: Debug` (already satisfied via the manual impl + the `S::Canonical: Debug`
  the existing `Diagnostic<S>` field already requires).
- `crates/engine/src/engine/pipeline.rs`: in the SUCCESS return of
  `lint_with_options_internal_with_source` (after the `doc_join_acc` EOD fold, before/at the final
  `LintResult` build ~`:445`), set `resolved_document: self.resolve_document(&doc_join_acc)`. The
  truncation/deadline early returns leave it `Default` (empty) — a truncated lint has no complete
  resolution. (`resolve_document` is on the `R: Recognizer` block; the pipeline block has that
  bound. `artifact_category`/`document_artifacts`/`derivation_edges` are on `MarkingScheme`.)

### Step 6 — Integration tests + regression safety (`crates/engine/tests/document_resolution.rs`)
StubScheme modeled on `crates/engine/tests/scheduler.rs` (already has `derivation_edges()`,
`ConstraintBridge` empty impl, zero-candidate recognizer), extended with `document_artifacts()`,
`artifact_category()`, and a non-trivial `Canonical` for derived-value assertions.
- `resolution_present_with_fixing_off` (T033): `engine.lint(src)` →
  `result.resolved_document` non-empty.
- `when_mode_edge_in_dag_but_skipped` (T033b): WhenMode edge ⇒ `scheduled_steps()` contains the
  `DerivationEdge` AND the target node resolves `FlagOnly`.
- `sc007_paired_fixable_and_flag_only` (T034, **SC-007**): ONE scheme, two kinds — A (Rollup edge
  → category-A) ⇒ `Fixability::Fixable` + `derived_value.is_some()`; B (no edge) ⇒
  `Fixability::FlagOnly` + `derived_value.is_none()`. Both asserts, one harness.
- `rollup_node_returns_derived_value` (T034b): assert `derived_value == Some(expected_rollup)`.
- `capco_produces_empty_resolved_document` (regression): real `CapcoScheme` engine →
  `lint().resolved_document.is_empty()`.

**Un-edited over-fire safety net** (MUST pass unchanged): `crates/capco/tests/…transmutation_
rewrites…` (30-rewrite construction), `…corpus_parity…`, `crates/engine/tests/audit_g13_canary.rs`,
`crates/capco/tests/post_3b_registration_pin.rs`.

## Constitution Check
- **I**: resolution runs once/document at EOD; CAPCO path is an O(1) empty-slice no-op; off the
  per-candidate hot path. SC-001 preserved.
- **II**: `derived_value` is `S::Canonical` (structural, not document bytes); no new content buffer.
- **IV**: no generated-code change; `artifact_category` is a Layer-2 declarative hook; engine PR
  (the scheme-adoption "don't edit engine" clause is N/A).
- **V (G13)**: resolved nodes carry only `ArtifactKind`/`EdgeId`/`Fixability`/`S::Canonical`; the
  canonical lives lint-side only, never in an `AppliedFix`. `audit_g13_canary` stays green; no
  audit-side surface added.
- **VI**: `resolve_document` is pure `&self`, no global state, fresh per input (rollup is fresh
  `Default` per call). Phase-separable EOD stage.
- **VII**: pure-data types in leaf `marque-scheme`; logic in `marque-engine`. Acyclic preserved;
  `marque-scheme` gains no new dep.
- **VIII**: no fabricated citations (no engine-synthetic diagnostics emitted in C4).

## Sequencing & risks
- Land order: scheme (Steps 1–2) → engine (Steps 3–5) → tests (Step 6). `cargo check --workspace`
  green at each.
- Risk: adding a generic field to `LintResult` ripples through its manual `Default`/`Clone` + the
  `#[derive(Debug)]`. Verify `S::Canonical: Debug` is already implied by the existing
  `Diagnostic<S>` field (it is) so no new public bound appears.
- No `scheduler.rs` edits (C3 already schedules + cycle/co-writer-checks `WhenMode` edges). C4 only
  *reads* `scheduled_steps()`.
- Verification: `rustup run 1.89 cargo check -p marque-scheme -p marque-engine --tests`;
  `rustup run stable cargo clippy -p marque-scheme -p marque-engine --tests`; `cargo fmt`;
  targeted `cargo test` for `resolution`, `document_resolution`, `scheduler`, and the CAPCO nets.
- Pre-flight: rust-reviewer + code-reviewer BEFORE PR-open. User merges.
