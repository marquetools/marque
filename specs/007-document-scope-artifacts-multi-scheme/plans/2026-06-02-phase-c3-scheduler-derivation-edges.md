# Phase C3 — Scheduler folds `DerivationEdge` into the Kahn pass (T032 + T032b)

**Branch:** `007-phase-c3` (off merged main `0c3d042e`, which carries C2's accumulator + Phase H #848)
**Scope:** ENGINE-crate PR. Extend `crates/engine/src/scheduler.rs` to co-schedule
`DerivationEdge`s with `PageRewrite`s in one Kahn pass; reject cycles at `Engine::new`;
add the T032b under-annotation (stale-value-read) guard. **No edge *evaluation*** — that is
C4 (`resolve_document`). This PR only *orders* edges and *validates* the combined graph.

Synthesized from parallel system-architect + rust-specialist tactical plans. Where they
diverged, the resolution and rationale are recorded inline.

---

## Tasks (verbatim, tasks.md)

- **T032** [C]: "Extend `crates/engine/src/scheduler.rs` to schedule `DerivationEdge`s in the
  same Kahn pass as `PageRewrite`s; cycles rejected at `Engine::new`. Tests: cycle →
  `RewriteCycle`; writers-before-readers order."
- **T032b** [C]: "Scheduler mis-annotation guard — a `DerivationEdge` consuming a
  `PageRewrite`'s output but omitting that axis from its `reads` MUST be rejected (or proven
  scheduled-after); test that a known-axis under-annotation cannot produce a stale-value read."

## As-built reconciliation (spec vs. contract vs. code)

- **`DerivationEdge.reads/writes` is `&'static [CategoryId]`**, not `Box<[CategoryId]>` (the
  `contracts/document-artifact.md` sketch is illustrative — same C1-class deviation). Matches
  `PageRewrite`'s `&'static` slice shape exactly; the scheduler already walks `&'static` slices.
- **`DerivationEdge` is a plain non-generic struct** (`relation` + `citation` + `reads` +
  `writes` + `firing`) with no `Custom` trigger/action — so the `UnannotatedCustomAxes` check is
  structurally inapplicable to edges, and the `edges` parameter needs **no `S` bound**.
- **`MarkingScheme::derivation_edges() -> &[DerivationEdge]`** already exists (default `&[]`,
  adjacent to `page_rewrites()`). No scheme-surface edit. CAPCO keeps the default (no edges).
- **`RewriteId == EdgeId == &'static str`** — node identity in the union graph is homogeneous at
  the type level, so the discriminated `ScheduledStep` enum is load-bearing (an untagged
  `Box<[&'static str]>` would silently conflate a rewrite and an edge sharing an id string).

---

## Resolved design (the two cruxes the planners split on)

### Crux A — single producer, reuse `RewriteCycle` (chosen: architect's end-state)

**Decision:** ONE Kahn producer over the union; reuse the existing `RewriteCycle` variant with
its `members` field retyped to carry tagged nodes. Reject the two-producer / new-`RewriteEdgeCycle`
alternative.

Rationale: the contract says "a **single** Kahn's pass over the **union**." The two-producer
alternative (keep `schedule_rewrites` + add `schedule`, emit a second `RewriteEdgeCycle` variant)
was self-flagged by its own author as C4 cleanup debt (two producers, two cycle variants, a
double construction-time pass). The project is pre-users with an explicit no-deferred-debt,
rewrite-freely posture — land the clean shape now. `EngineConstructionError` is **not** a frozen
surface (only the audit schema `marque-3.2` and the lattice trait surface are frozen) and is not
`#[non_exhaustive]`, so retyping a field / changing a variant is free in-tree.

### Crux B — T032b guard is **edge-scoped** co-writer detection (regression-critical)

**Decision:** the guard fires only when a `DerivationEdge` is one of an *unordered co-writing
pair*. It MUST NOT fire on rewrite↔rewrite pairs.

Rationale (closes a latent regression neither planner fully nailed): the existing scheduler
tolerates two `PageRewrite`s writing the same category with no read forcing order (declaration
order wins). CAPCO ships **30 rewrites** (`transmutation_rewrites.rs`, `corpus_parity.rs` assert
the count by building the real engine). A guard that fired on rewrite↔rewrite co-writers could
reject CAPCO at `Engine::new` — a regression. The task wording ("a `DerivationEdge` consuming a
`PageRewrite`'s output") confirms edge-scoping is the correct reading. The existing CAPCO
engine-construction tests are the over-fire safety net (they must keep passing unchanged).

---

## New types (in `crates/engine/src/scheduler.rs`)

```rust
/// A node in the combined rewrite + derivation-edge schedule.
/// The discriminant is load-bearing: RewriteId and EdgeId are both
/// &'static str, so an untagged order could conflate a rewrite and an
/// edge that happen to share an id string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledStep {
    PageRewrite(RewriteId),
    DerivationEdge(EdgeId),
}
```

`pub use scheduler::ScheduledStep;` from `crates/engine/src/lib.rs` — it appears in the public
`EngineConstructionError::RewriteCycle.members` field, so it must be a public type.

## Producer & projection

```rust
/// THE producer: one Kahn pass over the union of page rewrites and
/// derivation edges. Returns the tagged topological order.
pub fn schedule_steps<S>(
    rewrites: &[PageRewrite<S>],
    edges: &[DerivationEdge],
) -> Result<Box<[ScheduledStep]>, EngineConstructionError>
where
    S: MarkingScheme + ?Sized,

/// Rewrites-only projection of a combined order (keeps every
/// PageRewrite step, in combined-pass order).
fn project_rewrites(order: &[ScheduledStep]) -> Box<[RewriteId]>
```

`schedule_rewrites` is **retained as a thin wrapper** —
`schedule_steps(rewrites, &[]).map(|o| project_rewrites(&o))` — so the 4 existing unit tests at
`scheduler.rs:534-566` stay byte-identical (regression signal that the projection is exact). It
is a legitimate rewrites-only convenience; document it as such.

### Unified-index graph

- Index space: `0..R` = rewrites (declaration order), `R..R+E` = edges (declaration order).
  `let r = rewrites.len(); let n = r + edges.len();`.
- Private accessors `node_reads(rewrites, edges, r, i)` / `node_writes(...)` return
  `&'static [CategoryId]` for node `i` (rewrite if `i < r`, else edge `i - r`). Both payload
  types are `&'static [CategoryId]`, so no lifetime gymnastics.
- `writers: BTreeMap<CategoryId, Vec<usize>>` and `successors: Vec<BTreeSet<usize>>` build
  exactly as today but over all `n` nodes. Determinism preserved (BTree iteration sorted, indices
  declaration-ordered, frontier seeds `0..n`; rewrites ahead of edges on ties). Self-edge skip
  (`producer_idx == idx`) carries over and now also covers an edge that reads+writes the same
  category.
- `tarjan_sccs(n, &successors)` is reused **unchanged** (fully index-generic).
- `cycle_axis` generalized to read via the `node_reads`/`node_writes` accessors over mixed
  indices; cycle `members` mapped back to `ScheduledStep` (`i < r ⇒ PageRewrite`, else
  `DerivationEdge`).

### Construction-site wiring (`constructors.rs:59-60`, single source of truth)

```rust
validate_intent_rewrites(&scheme, scheme.page_rewrites())?;
let scheduled_steps = schedule_steps(scheme.page_rewrites(), scheme.derivation_edges())?;
let scheduled_rewrites = project_rewrites(&scheduled_steps);
```

Both cached on `Engine`. `scheduled_rewrites: Box<[RewriteId]>` is unchanged in type and (for
CAPCO, which has no edges) byte-identical in value. Add a sibling field
`scheduled_steps: Box<[ScheduledStep]>` (doc-comment: the union order consumed by C4
document-scope resolution; cycle + co-writer guards covering it run once here). Add accessor
`pub fn scheduled_steps(&self) -> &[ScheduledStep]` mirroring `scheduled_rewrites()`.

> Deriving `scheduled_rewrites` from the union projection (not a separate edge-free pass) is the
> single-source-of-truth choice: for a future edge-bearing scheme the rewrites order then honestly
> reflects edge-induced transitive ordering, instead of two orders that could disagree. CAPCO has
> no edges ⇒ byte-identical today.

## T032b guard — precise predicate

Set algebra over `CategoryId`. For each unordered pair `(A, B)` of distinct nodes where **at
least one is a `DerivationEdge`** (never rewrite↔rewrite), let `Shared = A.writes ∩ B.writes`:

- If `Shared = ∅` → not a hazard (skip). *(Independent edges/rewrites never false-positive.)*
- If `Shared ≠ ∅`:
  - **Ordered / OK ("proven scheduled-after"):** `Shared ∩ B.reads ≠ ∅` (B reads a shared axis ⇒
    `A → B` edge exists) **or** `Shared ∩ A.reads ≠ ∅` (`B → A` exists). An explicit read forces a
    deterministic order, so no stale read.
  - **Unordered / REJECT:** otherwise — two producers of a shared category with no read forcing
    order ⇒ the final value depends on an arbitrary declaration-order tiebreak (a latent
    stale/clobbered read). Emit `AmbiguousCoWriter`.

Run the guard as a pre-pass (after the custom-axis check, before/with graph build) so the
actionable per-axis error surfaces before a graph-shaped cycle error — mirroring the existing
validate-first rationale at `constructors.rs:48-59`.

**Honest scope (state in the variant doc-comment — do NOT overclaim):**
- **Catches:** the annotation-inconsistency form — a node (edge) that co-writes a category another
  node writes but omits the matching `reads`. This is the realistic scheme-author slip and the
  exact case the T032b anchor test exercises.
- **Cannot catch (genuinely semantic, undetectable at this layer):** an edge whose *body* consumes
  a category that appears in neither its `reads` nor its `writes`. `DerivationRelation` is an
  opaque `#[non_exhaustive]` enum carrying no introspectable category payload — same
  un-detectability that already forces the `PageRewrite::custom` author-must-annotate contract.
  The guard *reduces*, does not *eliminate*, annotation-error risk. Escalate rather than fake
  full semantic detection (it is provably impossible here).

## New error variant + retyped field (`errors.rs`)

```rust
// retype existing field — members now tagged so a mixed cycle is honest
RewriteCycle {
    axis: CategoryId,
    members: Box<[ScheduledStep]>,   // was Box<[RewriteId]>
},

/// A derivation edge co-writes `axis` with another node (rewrite or
/// edge) but no read forces a deterministic order between them — a
/// stale-value read hazard (T032b). Usual cause: the edge omitted the
/// consumed axis from its `reads` (under-annotation). Scheme-author
/// defect → EX_UNAVAILABLE (69).
///
/// Detects the annotation-inconsistency form only: it cannot detect an
/// edge whose body semantically consumes a category absent from BOTH
/// `reads` and `writes`, because the scheduler does not introspect edge
/// bodies.
AmbiguousCoWriter {
    axis: CategoryId,
    nodes: Box<[ScheduledStep]>,
},
```

- `exit_code()`: `AmbiguousCoWriter` joins the `69`/`EX_UNAVAILABLE` arm (scheme-author defect,
  same class as `RewriteCycle` / `UnannotatedCustomAxes`).
- `Display`: `RewriteCycle` → `"rewrite/derivation cycle on category {axis:?}: {members:?}"`
  (drop the pure-"page-rewrite" wording — it would mislabel an edge). `AmbiguousCoWriter` →
  `"derivation edge co-writes category {axis:?} with no read forcing order — stale-value read \
  hazard: {nodes:?}; declare the consuming edge's reads"`. `{members:?}`/`{nodes:?}` render via
  `ScheduledStep`'s derived `Debug` (`[PageRewrite("a"), DerivationEdge("e")]` — kind visible).

## Edge custom-axis / empty-axis / firing handling

- **Custom axes:** edges have no `Custom` variant; do not extend `rewrite_is_custom` /
  `UnannotatedCustomAxes` to them. Edge nodes are `is_custom = false` by construction.
- **Empty-axis edge (`reads=[]`, `writes=[]`): TOLERATE.** A no-op in the dataflow graph (no
  `writers` entry, no successor edge; schedules in declaration order). Consistent with the existing
  tolerance for empty-axis *declarative* rewrites; firing-gated / `CannedString` edges legitimately
  have no page-scope dataflow (their effect is at C4 eval time). Cannot trip the guard
  (`writes=[]` ⇒ no shared-write) or a cycle. Pin with a test.
- **Firing predicate is scheduling-irrelevant (research D3):** `schedule_steps` never reads
  `edge.firing`. A `WhenMode("…")` edge is built into the graph, cycle-checked, and guard-checked
  identically to `Always` (topology is static; firing gates only C4 eval). Pin with a test
  (a `WhenMode` edge in a cycle still trips `RewriteCycle`).

---

## File-by-file change list

1. **`crates/engine/src/scheduler.rs`** (primary):
   - `use marque_scheme::{DerivationEdge, EdgeId};` added to imports.
   - Add `pub enum ScheduledStep`.
   - Add `schedule_steps<S>(rewrites, edges)` — custom-axis check (rewrites only) → T032b
     edge-scoped co-writer guard → unified-index graph build (via `node_reads`/`node_writes`) →
     Kahn → `tarjan_sccs` (reused) → mixed-index `cycle_axis` + `RewriteCycle{members:
     ScheduledStep}`.
   - Add `project_rewrites(&[ScheduledStep]) -> Box<[RewriteId]>`.
   - Reduce `schedule_rewrites` to the thin wrapper (delegates to `schedule_steps(.., &[])` +
     project). Keep `tarjan_sccs`, `rewrite_is_custom`, `validate_intent_rewrites`,
     `intent_fact_refs` byte-identical. Generalize `cycle_axis` to the unified accessors.
   - Add the guard fn (e.g. `reject_ambiguous_cowriters`).
   - Extend test module: add `derivation_edge(id, reads, writes, firing)` builder mirroring
     `declarative`; add new unit tests (below). The 4 existing `schedule_rewrites` unit tests stay
     untouched.

2. **`crates/engine/src/errors.rs`**:
   - Retype `RewriteCycle.members` → `Box<[ScheduledStep]>`; update its doc + Display.
   - Add `AmbiguousCoWriter`; add `exit_code()` arm (69) + Display arm.
   - `use crate::scheduler::ScheduledStep;`.
   - Update the in-file `RewriteCycle` test fixture (`errors.rs:384`).

3. **`crates/engine/src/engine.rs`**: add field `scheduled_steps: Box<[ScheduledStep]>` (doc
   mirrors `scheduled_rewrites`); import `ScheduledStep`. (Scheduler import line already pulls
   `schedule_rewrites, validate_intent_rewrites` — add `schedule_steps`, `project_rewrites`,
   `ScheduledStep`.)

4. **`crates/engine/src/engine/constructors.rs`**: rewire lines 59-62 to the single
   `schedule_steps(...)` + `project_rewrites(...)`; add `scheduled_steps,` to the struct literal
   (~line 132); add `pub fn scheduled_steps(&self)` accessor near `scheduled_rewrites()` (~321).

5. **`crates/engine/src/lib.rs`**: `pub use scheduler::ScheduledStep;`.

6. **`crates/engine/src/engine/tests/part3.rs`**: update the `RewriteCycle` constructor (line 293)
   to tagged members; add `AmbiguousCoWriter` exit-code/Display tests.

7. **`crates/engine/tests/scheduler.rs`** (integration): update the ~6 `RewriteCycle { members, .. }`
   match arms (lines ~256/299/337/372) to compare `ScheduledStep` members (e.g.
   `members.contains(&ScheduledStep::PageRewrite("id"))`). The `scheduled_rewrites()` *order/count*
   assertions stay unchanged (the real regression signal).

**No edits** to `marque-scheme` (`derivation_edges()` already present), `marque-capco` (keeps
default empty edges; its construction/`scheduled_rewrites().len()` tests are the over-fire safety
net), or any rule body. Engine-only PR.

## TDD-ordered test list (RED first)

Error-variant tests (`errors.rs` + `part3.rs`):
1. `rewrite_cycle_members_are_scheduled_steps` — construct with tagged members; Display renders
   kinds. (RED until retype.)
2. `ambiguous_co_writer_exit_code_is_unavailable` (69) + `..._display_names_axis_and_nodes`. (RED
   until variant exists.)

Scheduler unit tests (`scheduler.rs`, extend `StubScheme` + `derivation_edge` builder):
3. The 4 existing `schedule_rewrites` tests — stay GREEN unchanged (projection byte-identical).
4. `edges_only_preserves_declaration_order` — independent edges → tagged order = declaration order.
5. `rewrite_writer_before_edge_reader` — R writes X, E reads X → `[PageRewrite("r"),
   DerivationEdge("e")]`.
6. `edge_writer_before_rewrite_reader` — E writes X, R reads X → edge precedes rewrite.
7. `edge_to_edge_ordering` — A writes X, B reads X → A before B.
8. `union_cycle_with_edge_member_is_rewrite_cycle` — R writes X reads Y; E writes Y reads X →
   `RewriteCycle`, members include both a `PageRewrite` and a `DerivationEdge`; axis ∈ {X,Y}.
9. `when_mode_edge_still_participates_in_cycle_check` — same cycle, edge `WhenMode("strict")` →
   still `RewriteCycle` (firing irrelevant).
10. `cowriter_without_explicit_read_is_ambiguous` — R writes Y; E writes Y, `reads=[]` →
    `AmbiguousCoWriter{axis: Y}`. **T032b stale-value anchor.**
11. `cowriter_with_explicit_read_is_ok` — E also `reads=[Y]` → `Ok`, edge after rewrite. **Proven
    scheduled-after companion.**
12. `rewrite_rewrite_cowriter_is_not_guarded` — two rewrites co-write Y, neither reads it → `Ok`
    (edge-scoping; guards CAPCO from regression).
13. `independent_edge_and_rewrite_dont_false_positive` — R writes X, E writes Y (disjoint) → `Ok`.
14. `empty_axis_edge_is_tolerated` — edge `reads=[]`, `writes=[]` → schedules, no error.
15. `rewrites_only_projection_matches_schedule_rewrites` — edge-free input:
    `project_rewrites(schedule_steps(rw, &[])) == schedule_rewrites(rw)`.

Engine-construction (`engine/tests.rs` / part3):
16. `derivation_edges_cycle_checked_at_engine_new` — a stub scheme overriding `derivation_edges()`
    with a cyclic edge set → `Engine::with_clock_and_recognizer` returns `Err(RewriteCycle)`;
    CAPCO `scheduled_rewrites()` equivalence (`engine/tests.rs:156-157`) stays GREEN.

CAPCO safety net (no new test needed): `transmutation_rewrites.rs` /
`corpus_parity.rs` building the real engine and asserting rewrite counts must stay GREEN —
proves the guard does not over-fire on the real 30-rewrite scheme.

---

## Constitution Check

- **I (perf):** all new work is construction-time, amortized once at `Engine::new`, over ≤ low-tens
  of nodes; co-writer reachability gated on shared-write pairs; zero hot-path cost. ✅
- **VI (Send+Sync, no global state):** `ScheduledStep` is `Copy` plain data; `schedule_steps` is a
  pure function; cached order is immutable; no `static mut`/hidden cache. ✅
- **VII (acyclic graph / leaf discipline):** scheduler stays in `marque-engine`; `DerivationEdge`/
  `EdgeId` come from the `marque-scheme` leaf (already a dep). No new deps; no edge inversion. ✅
- **VIII (citations):** no new citations authored; `DerivationEdge` carries its `Citation`
  untouched; the `RewriteCycle` Display reword *removes* an inaccuracy (calling an edge a
  "rewrite"). ✅
- **IV (scheme-adoption ≠ engine edit):** this is the inverse — an engine-crate PR landing the gap
  before any scheme declares edges (CAPCO stays at default `&[]`). Correct sequencing. ✅
- **Frozen surfaces:** audit schema (`marque-3.2`) + lattice trait surface untouched.
  `EngineConstructionError` is not frozen and not `#[non_exhaustive]`; retyping a field / adding a
  variant is free in-tree (pre-users, no deprecation phasing). Note it in the PR body for the
  record. ✅

## Risks / open questions

- **`RewriteCycle.members` retype touches ~6 in-tree match sites** (all enumerated above). The
  *order/count* assertions — the real regression signal — are unaffected; only cycle-error-shape
  matchers get the mechanical `ScheduledStep` update. Confirmed blast radius via workspace grep.
- **Guard over-fire on CAPCO** — mitigated by edge-scoping + the existing CAPCO construction tests
  as the safety net. If those break, the guard is mis-scoped.
- **Two cycle entry points eliminated** — single producer means no `RewriteEdgeCycle` debt and no
  C4 cleanup item.
- **Verification:** `rustup run 1.89 cargo check -p marque-engine --tests`; clippy proxy
  `rustup run stable cargo clippy -p marque-engine --tests`; then `rustup run stable cargo fmt`
  (local-nightly-vs-CI-stable rustfmt drift); re-verify any LSP-flagged error against a fresh
  compile (stale-diagnostic gotcha). Also run `-p marque-capco` tests (the over-fire net).
