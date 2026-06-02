# Phase C2 (T031) — Engine document-scope accumulator

Branch `007-phase-c2` off `origin/main` (contains C1, PR #847). **Engine-crate PR**
(`marque-engine` only). Synthesized from parallel system-architect + rust-specialist
tactical plans; both converged.

## Goal

Stand up a document-scope rollup accumulator above the per-page accumulator in the
engine's per-document lint loop. A document boundary is the **input boundary** (one
`lint_inner` call = one document); the accumulator is a fresh stack local re-initialized
each call. At every page boundary the closing page's canonical rollup folds into the
document accumulator via C1's `MarkingScheme::canonical_document_join`, never a naive
re-union — so observational state (RELIDO-unanimity, NOFORN-supersession, JointSet
disunity-collapse) survives the page→document fold for free, exactly as it survives the
portion→page fold.

This PR is plumbing + tests. The document rollup is **surfaced** (so the two required
invariant tests can assert on it) but not yet **consumed** by a document-finalization
dispatch — that is C4 (`resolve_document`). YAGNI: do not build C4's consumer here.

## Key decisions (both planners agreed)

1. **Incremental, not batch.** Carry a single running `doc_join_acc: S::Canonical`,
   folded via `canonical_document_join(&[mem::take(&mut doc_join_acc), page_rollup.clone()])`
   at each boundary. Holds exactly one accumulated canonical for the whole document
   (Constitution II), mirroring the portion→page `mem::take` discipline (the #306/#674
   O(N) streaming fix) one scope up. The batch `DocumentContext::from_pages` is NOT the
   engine path — it requires holding a `Vec<S::Canonical>` of all page rollups, the exact
   allocation Constitution II forbids. `from_pages` stays the **reference fold** for the
   cross-check test.

2. **Defer `DocumentContext<S>` construction in the engine; carry bare `S::Canonical`.**
   `DocumentContext<S>` requires `S: SchemeArtifacts` (a `MarkingScheme` supertrait, not a
   blanket impl). `Engine<S, R>`'s lint impl block is bounded `S: MarkingScheme +
   ConstraintBridge` and the engine crate never names `SchemeArtifacts` today. Constructing
   or returning `DocumentContext<S>` would force a new `S: SchemeArtifacts` bound onto the
   engine's central lint impl — purely to wrap a value whose `artifacts` field is empty
   until C6 (FrontMarking). The accumulator's *state* is `S::Canonical`; the
   `DocumentContext` *envelope* is C4/C6's concern. The bare `S::Canonical` needs ZERO new
   bound (`Clone + Default` for the fold is already subsumed by the sites' existing
   `Clone + Default + PartialEq`). **`DocumentContext` IS still exercised in C2 — in the
   tests**, where the concrete `CapcoScheme: SchemeArtifacts` lets the cross-check test
   build `DocumentContext::from_pages(&scheme, &page_rollups).rollup` as the reference fold.
   This honors T031's "DocumentContext accumulator" intent without contaminating the engine
   generic surface.

   *Deviation from T031 literal text* ("DocumentContext shape in marque-ism … reusing
   DissemSet/JointSet"): recorded in Constitution Check below. Same class of principled
   deviation as C1 (which placed the type in marque-scheme, not marque-ism, on Constitution
   VII grounds). C2 constructs no `DocumentContext` in production code and edits no leaf crate.

3. **No `marque-scheme` / `marque-ism` / `marque-capco` / `marque-core` / `marque-rules`
   edit.** Pure `marque-engine` PR. C2 calls only `canonical_document_join` (shipped in C1)
   and surfaces `S::Canonical`. No forced `from_rollup` constructor.

## Integration sites (all `marque-engine`, verified against worktree)

### A. Init — `pipeline.rs` ~line 161

Add one local beside the page accumulators:

```rust
let mut doc_join_acc: S::Canonical = <S::Canonical>::default();
```

Fresh `Default` per `lint_inner` call = the "fresh per input" guarantee (no `mem::take`
at init; it is a fresh bottom).

### B. Page-boundary fold — inside `handle_page_break_candidate` (`lint_helpers.rs`)

Add a `doc_join_acc: &mut S::Canonical` parameter (placed adjacent to `page_join_acc`;
the existing `#[allow(clippy::too_many_arguments)]` already covers it). The fold must read
the closing page's `page_join_acc` **before** the reset block (lines 113–119). Insert,
**guarded on non-empty portions** (symmetry with the dispatch guard at line 89 and the EOD
guard), immediately before line 113:

```rust
// Fold the closing page's canonical rollup into the document
// accumulator before the per-page reset below. The page join is a
// genuine semilattice join for lattice schemes (CapcoScheme), so the
// page→document fold is order-independent (research D12 / LV3); the
// default scheme gets last-page-wins.
if !page_portions.is_empty() {
    *doc_join_acc = engine.scheme.canonical_document_join(&[
        std::mem::take(doc_join_acc),
        page_join_acc.clone(),
    ]);
}
```

Keep the existing reset block (113–119) intact — `page_join_acc.clone()` for the fold,
then `*page_join_acc = default()` resets as before. One clone per page boundary (rare),
matching the per-portion `recognized.attrs.clone()` pattern; clarity over the
micro-optimization of folding via `mem::take(page_join_acc)`. The fold sits after
`dispatch_page_finalization` succeeds and before the unconditional reset, inside the
"reset is unconditional once in this branch" region — preserving the Constitution VI
malformed-page-break invariant (a malformed page-break that reached this branch still
folds page N and resets for N+1).

Caller update (`pipeline.rs` ~line 191): pass `&mut doc_join_acc` after `&mut page_join_acc`.
`&mut page_join_acc` and `&mut doc_join_acc` are disjoint borrows of two distinct locals.

### C. EOD fold — `pipeline.rs` after the end-of-document finalization `if` (~line 360)

The EOD `dispatch_page_finalization` reads `&page_join_acc` (line 340); that borrow ends
at the `if` block's close. Fold **after** that block, guarded identically:

```rust
// Fold the final (un-page-broken) page rollup into the document
// accumulator — catches trailing portions that never reached a
// PageBreak boundary. Guarded on non-empty portions exactly as the
// EOD finalization dispatch above is.
if !page_portions.is_empty() {
    doc_join_acc = self.scheme.canonical_document_join(&[
        std::mem::take(&mut doc_join_acc),
        page_join_acc.clone(),
    ]);
}
```

`page_join_acc` is not reset at EOD (document is ending); the clone is the only way to feed
the fold (one allocation per document, negligible).

### D. Surface the rollup — widen only `lint_with_options_internal_with_source` to a 3-tuple

Return type `(LintResult<S>, Vec<(Span, S::Marking)>)` → `(LintResult<S>, Vec<(Span,
S::Marking)>, S::Canonical)`. `S::Canonical` is already a fully-capable named associated
type in this signature's scope — **no new bound.**

Return-expression updates inside `_with_source`:
- Pre-init top deadline guard (line 142, before `doc_join_acc` exists): third element
  `<S::Canonical>::default()`.
- The three post-init truncation/deadline returns (≈168, 199, 349): third element
  `std::mem::take(&mut doc_join_acc)` — surface the **partial** rollup (pages folded before
  truncation). `LintResult.truncated = true` already flags incompleteness; returning the
  partial is more honest than erasing already-folded pages to `default()`. C4 must treat a
  truncated lint's rollup as partial (note for C4, do not solve here).
- Success tail: third element `doc_join_acc`.

Caller ripple (all in-crate):
- `lint_with_options_internal_with_cache` (line 116) forwards `_with_source`; keep its
  2-tuple signature and drop the third element: `let (r, m, _) = self.…; (r, m)`.
- `lint_with_input_context` StructuredField arm (line 82): `… .0` is unaffected by adding
  a `.2` — no change.
- `fix_impl.rs` call sites of `_with_source` (≈104, 124, 269): destructure the 3-tuple,
  discarding the rollup (`let (lint, markings, _doc) = …`); in the 124 `if/else` both arms
  must produce matching arity — carry the discard through the `else` too.
- `lint_with_options_internal` (line 100), `lint`, `lint_with_options`: unchanged.

Public API surface is **unchanged**. Tests reach the rollup by calling `_with_source`
directly (as `tests/part2.rs` already does for the 2-tuple).

## Tests (`#[cfg(test)]` in the engine in-crate test module; `CapcoEngine`)

1. **`fresh_document_rollup_per_input`** — two sequential `_with_source` calls on distinct
   multi-page inputs (different top classification levels). Assert call 2's rollup reflects
   only call 2 and `roll_b != roll_a` (no bleed). Pins Constitution VI fresh-per-input.

2. **`malformed_page_break_does_not_block_document_fold`** — input with a real portion on
   page N, then a degenerate page-break region (e.g. `\f` adjacent to garbage / a
   form-feed with no surrounding portion text), then page N+1's portion. Assert: (i) the
   document rollup absorbs page N's rollup (fold ran despite the malformed break — it sits
   before the unconditional reset); (ii) page N+1 starts clean; (iii) final rollup =
   join(rollup_N, rollup_N+1). The C2 analogue of the PageContext-reset invariant test,
   lifted to document scope.

3. **`incremental_doc_fold_matches_batch_from_pages`** — drive the engine's incremental
   `doc_acc`; independently collect each page's rollup into `Vec<S::Canonical>` and compute
   `DocumentContext::from_pages(&scheme, &pages).rollup` (C1's reference batch fold). Assert
   equality. Catches a wiring bug where the engine folds in the wrong order or drops a page,
   and empirically confirms associativity/commutativity of the CAPCO lattice at document
   scope. This is the test that exercises `DocumentContext` in C2.

4. **`single_page_document_yields_that_pages_rollup`** + **`empty_document_yields_default_rollup`**
   — edge coverage: single-page (no PageBreak, only the EOD fold fires) and empty (no
   candidates, rollup = `default()`).

Classification roll-up authority is **CAPCO-2016 §D.2 p28** (per C1); cite it only in the
test/comment that asserts max-classification roll-up across pages, not gratuitously.

## Constitution Check

| Principle | Status |
|---|---|
| I (perf ≤ 2 ms / 10 KB) | Fold fires only at page boundaries + one EOD — O(pages), zero per-candidate/per-portion cost. `mem::take` avoids accumulator clones. `lint_10kb` (single portion, no page-break) adds exactly one 2-element `canonical_document_join` at EOD. Must confirm bench-check green, not assume. |
| II (zero-copy / streaming) | Incremental single-`S::Canonical` accumulator (1 value, not N). `mem::take` mirrors #306/#674. No `Vec<S::Canonical>` of page rollups held. ✅ |
| IV (two-layer; engine-orchestration-last) | Pure orchestration in `marque-engine`; no rule logic, no generated-predicate change. Engine is the correct layer (Feature Development Sequence step 4). C2 is NOT a scheme-adoption PR, so editing engine crates is in-scope. ✅ |
| V (audit content-ignorance) | `S::Canonical` carries no free-form content; same canonical type already cached in `parsed_markings`. No new audit surface. ✅ |
| VI (pipeline / no global state) | Accumulator is a per-call stack local re-initialized each `lint_inner` (test 1 pins this). Fold before the unconditional page reset preserves the malformed-page-break invariant (test 2). `Send + Sync` unaffected (no interior mutability). ✅ |
| VII (crate discipline) | No new dependency edge; no `marque-scheme` edit. Crucially does NOT add `SchemeArtifacts` to the engine, keeping the engine's generic surface minimal. ✅ |

**T031 deviation (recorded):** T031's literal text says "DocumentContext shape in marque-ism
reusing DissemSet/JointSet." Post-Phase-B there is no `PageContext` struct (it became the
generic `canonical_page_join` fold) and `DissemSet`/`JointSet` live in marque-capco (marque-ism
cannot depend on marque-capco — Constitution VII inversion). C1 already resolved the type's home
to marque-scheme. C2 surfaces the bare `S::Canonical` rollup from the engine and exercises
`DocumentContext` only in tests, to avoid forcing `SchemeArtifacts` onto the engine's lint impl
block. The "accumulator" is the document-scope rollup state; the `DocumentContext` envelope
populates in C4 (`resolve_document`) / C6 (FrontMarking artifacts).

## Verification (worktree root)

1. `rustup run 1.89 cargo check -p marque-engine`
2. `rustup run 1.89 cargo check -p marque-engine --features decision-tracing` (fold insertions
   must sit OUTSIDE the adjacent `#[cfg(feature = "decision-tracing")]` blocks and compile in
   both states)
3. `rustup run stable cargo clippy -p marque-engine` (CI proxy; NOT `cargo +stable`)
4. `rustup run stable cargo clippy -p marque-engine --features decision-tracing`
5. `rustup run 1.89 cargo test -p marque-engine`
6. (sanity) `rustup run stable cargo check --workspace`

Steps 1–4 warning-clean before 5–6.

## Files

- `crates/engine/src/engine/pipeline.rs` — init local; `&mut doc_join_acc` in the page-break
  call; EOD fold; `_with_source` return widened to 3-tuple (four return expressions);
  `_with_cache` drop-forward.
- `crates/engine/src/engine/lint_helpers.rs` — `handle_page_break_candidate` +1 param; fold
  body before the reset.
- `crates/engine/src/engine/fix_impl.rs` — 3-tuple destructuring (≈104, 124, 269).
- `crates/engine/src/engine/tests/…` — new tests (1–4).
- No leaf-crate edits.
