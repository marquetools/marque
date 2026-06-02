# Phase C1 (T030) — `DocumentContext` shape + page→document fold

**Branch:** `007-phase-c1` (off main `77301dd1`)
**Scope:** task **T030** only. Pure leaf types + the lawful page→document fold + tests.
**No engine edits, no scheduler edits, no resolve/fix.** Those are C2/C3/C4.

Synthesized from two parallel tactical plans (system-architect + rust specialist),
both of which converged on the placement decision below.

---

## 1. The decision: placement + what "analogue of PageContext" means now

T030's literal text says *"`DocumentContext` shape in `marque-ism`"* and reuse
`DissemSet`/`JointSet`. That wording predates the Phase-B refactor and **cannot be
taken literally**:

- There is **no `struct PageContext`** left in the tree. The page roll-up became the
  generic lattice fold `MarkingScheme::canonical_page_join(&[S::Canonical]) -> S::Canonical`
  (`crates/scheme/src/scheme.rs`, default `portions.last().cloned().unwrap_or_default()`;
  CAPCO overrides → `CapcoMarking::join_via_lattice`). The CLAUDE.md "PageContext
  (marque-ism)" line is itself stale.
- `DissemSet`/`JointSet` live in `marque_capco::lattice`, and **`marque-ism` MUST NOT
  depend on `marque-capco`** (Constitution VII — graph inversion). A literal
  marque-ism `DocumentContext` naming those types does not type-check.

**Resolution (Constitution VII wins over stale task text; recorded as an intentional
deviation):** `DocumentContext<S>` is a **generic leaf type in `marque-scheme`**, naming
only `S::Canonical` and `DocumentArtifact<S>`. The `DissemSet`/`JointSet` behavior stays
entirely inside `CapcoScheme::canonical_page_join → join_via_lattice`, reached through the
trait. This mirrors how Phase B already made the page fold generic (`PageFinalizationContext<'a,S>`,
`dispatch_page_finalization<S>`) so the engine never names a scheme type.

**"Analogue of PageContext" = the same canonical-space semilattice fold, one scope up
(pages → document).** Not a struct to clone — a fold to re-apply.

---

## 2. Why observational state survives the fold (the core correctness property)

The page→document fold is `canonical_page_join` applied over the per-page accumulators:

```
page_acc[i]   = canonical_page_join(portions_of_page_i)     // already done by the engine
doc_rollup    = canonical_document_join([page_acc[0..n]])   // == canonical_page_join, one level up
```

This is lawful by LV3 (research D12): `join` is associative + commutative + idempotent,
so a fold of page-joins equals a flat fold of all portions — order and grouping do not
matter.

RELIDO-unanimity and NOFORN-supersession survive **for free**, because the fold routes
through the same `join_via_lattice`, which reconstructs `DissemSet`/`JointSet` internally:

- **RELIDO-unanimity**: `relido_observed_unanimous` aggregates as boolean AND across joined
  operands (`crates/capco/src/lattice/dissem.rs`). All-pages-RELIDO ⇒ all `true` ⇒ survives;
  one page not-unanimous ⇒ `true && false` ⇒ Overlay 2 drops RELIDO at the document banner
  (§H.8 pp155-156). A naive token re-union would wrongly keep RELIDO.
- **NOFORN-supersession**: Overlay 3 re-runs inside every `join` (§H.8 p145 + §D.2 Table 3):
  once any page contributes NOFORN, `Rel`/`Relido`/`Displayonly`/`Eyes` are stripped. A naive
  re-union would re-admit the dominated tokens from non-NOFORN pages.

**Implementation guardrail (CRITICAL):** the document fold MUST go through
`canonical_document_join`/`canonical_page_join`. It MUST NOT set-union the flat
`CanonicalAttrs` `Box<[T]>` fields — that is the "naive re-union" LV3 forbids and is what
the tests pin.

---

## 3. Surface to add

### 3a. `marque-scheme` — new defaulted fold method (`crates/scheme/src/scheme.rs`)

Next to `canonical_page_join`, additive (defaulted method on the frozen trait; no new
required surface):

```rust
/// Join a slice of per-*page* canonical rollups into a single *document*
/// rollup — [`Self::canonical_page_join`] one scope up (pages, not portions).
///
/// The default delegates to `canonical_page_join`: page→document is the
/// identical semilattice join (research D12/LV3), so a scheme that does not
/// distinguish the two scopes inherits correct behavior. CAPCO does not
/// override — `join_via_lattice` is scope-agnostic over `&[CanonicalAttrs]`,
/// so routing page accumulators through it preserves DissemSet unanimity,
/// JointSet disunity collapse, and NOFORN supersession across the page→doc
/// fold. The `Clone + Default` bound is on the method (not the associated
/// type) to keep the trait additive.
fn canonical_document_join(&self, pages: &[Self::Canonical]) -> Self::Canonical
where
    Self::Canonical: Clone + Default,
{
    self.canonical_page_join(pages)
}
```

### 3b. `marque-scheme` — `DocumentContext<S>` (`crates/scheme/src/document_context.rs`, NEW)

```rust
pub struct DocumentContext<S: SchemeArtifacts + ?Sized> {
    /// Document-level canonical rollup (join over the page rollups).
    pub rollup: S::Canonical,
    /// Document-scoped artifact nodes (CAB, declassify instruction, notices,
    /// front marking). `Box<[T]>` (not `Vec`) per Constitution II — built once
    /// at document finalization, never grown; mirrors `DocumentArtifact::inbound`.
    pub artifacts: Box<[DocumentArtifact<S>]>,
}
```

- **Bound `S: SchemeArtifacts + ?Sized`** is forced by the `artifacts` field (mirrors
  `DocumentArtifact<S>` at `artifact.rs:124`). `SchemeArtifacts: MarkingScheme`, so
  `S::Canonical` is in scope.
- **Hand-write `Debug`/`Clone`/`PartialEq`** bounded on the field projections
  (`S::Canonical` + `S::ArtifactPayload`), verbatim mirror of the `DocumentArtifact` impls
  at `artifact.rs:142/158/174`. Do **NOT** `#[derive]` — a blanket derive emits spurious
  `where S: Debug/Clone/...` bounds (the B3.3b `LintResult<S>` lesson).
- **No `Default`** on the container — a document context is always built by folding ≥1 page;
  the empty case is handled inside the fold (`unwrap_or_default` on `rollup`). Advertising
  `Default` would force `S::Canonical: Default` on the container for no caller.
- **Constructor:**

  ```rust
  impl<S: SchemeArtifacts + ?Sized> DocumentContext<S> {
      /// Fold per-page canonical rollups into the document rollup. The fold
      /// IS `canonical_document_join`; artifact nodes are populated later
      /// (C2 engine accumulator).
      pub fn from_pages(scheme: &S, pages: &[S::Canonical]) -> Self
      where S::Canonical: Clone + Default {
          Self { rollup: scheme.canonical_document_join(pages), artifacts: Box::new([]) }
      }
  }
  ```

### 3c. `marque-scheme` — re-export (`crates/scheme/src/lib.rs`)

`mod document_context;` + `pub use document_context::DocumentContext;` (mirror the existing
`pub use artifact::...`).

### 3d. No CAPCO source change

`canonical_document_join` default-delegates; `join_via_lattice` is scope-agnostic. No edit
to `crates/capco/src/scheme/marking.rs` or `marking_scheme_impl.rs`.

---

## 4. Tests (TDD — write RED first)

### 4a. Shell-type tests (`crates/scheme/src/document_context.rs` `#[cfg(test)]`)

Reuse the existing `ArtifactScheme` fake (`artifact.rs:303`, `ArtifactPayload = u32`,
already impls `SchemeArtifacts`).

- `document_context_debug_clone_eq` — construct over `ArtifactScheme`, assert `clone() == self`,
  `Debug` names `DocumentContext`. Proves the manual impls compile with field-projection
  bounds (no spurious `S: Trait`).
- `document_context_artifacts_is_boxed_slice` — type-level assertion the field is
  `Box<[DocumentArtifact<S>]>` (Constitution II).
- `from_pages_empty_is_default_rollup` / `from_pages_single_is_identity` over a tiny stub
  whose `canonical_page_join` is the default — confirms `from_pages` wiring.

### 4b. Algebra tests (`crates/capco/tests/document_rollup.rs`, NEW)

Real lattice axes (CAPCO). Use the existing `crates/capco` lattice/test-support helpers and
build page-rollup `CanonicalAttrs` via the same path the page fold uses. Each test cites the
governing CAPCO § (verify against `crates/capco/docs/CAPCO-2016.md` via the citation-index
YAML before landing — Constitution VIII).

| Test | Assertion | Citation |
|------|-----------|----------|
| `document_rollup_max_classification_across_pages` | `[SECRET, TOP_SECRET, CONFIDENTIAL]` ⇒ doc `TOP_SECRET` | §G.1 Table 4 p37 |
| `relido_unanimity_survives_all_pages_relido` | all pages RELIDO-unanimous ⇒ doc rollup keeps RELIDO | §H.8 pp155-156 |
| `relido_dropped_when_one_page_not_unanimous` | one page not-unanimous ⇒ doc drops RELIDO | §H.8 pp155-156 |
| `noforn_supersession_survives_page_to_doc_fold` | one page NOFORN + another REL/RELIDO/EYES ⇒ doc has NOFORN, drops dominated | §H.8 p145 + §D.2 Table 3 |
| `fold_is_order_independent` | permute 3-page slice (RELIDO/NOFORN/JOINT mix) ⇒ identical `CanonicalAttrs` every permutation | LV3 semilattice law |
| `empty_document_is_lattice_bottom` | `canonical_document_join(&[])` ⇒ `CanonicalAttrs::default()` | join identity |
| `single_page_fold_is_identity` | `canonical_document_join(&[page])` ⇒ that page (incl. one guard-tripping `Conflict` case) | join identity |
| `joint_mixed_absorbing_across_pages` | one page `JointSet::Mixed`, another `UnanimousProducers` ⇒ doc `Mixed` | §H.3 p57 |

---

## 5. Constitution Check gate

| Principle | Status |
|-----------|--------|
| I (perf ≤ 2ms) | No hot-path/engine change in C1. O(pages) fold at finalization is C2's cost. ✓ |
| II (zero-copy) | `from_pages` borrows `&[S::Canonical]`, returns one folded value; `Box<[T]>` artifacts. ✓ |
| III (WASM-safe) | Type lands in `marque-scheme` (leaf, WASM set), names only `S::Canonical`/`DocumentArtifact<S>`; no new dep; no marque-capco edge. Re-run wasm32 build. ✓ |
| V (audit G13) | Structural only; `ArtifactPayload = ()` for CAPCO today; no audit-side type touched. ✓ |
| VII (acyclic graph) | **The gate.** Placement in `marque-scheme` naming only `S::Canonical` preserves the graph. **Deviation from T030 "marque-ism" recorded here with VII rationale.** ✓ |
| VIII (citations) | Each algebra test cites its CAPCO §; verify against `CAPCO-2016.md` before landing. ✓ |

**Recorded deviations / follow-ups (solo-driven paper trail):**
1. T030 says marque-ism; placement moved to marque-scheme on Constitution VII grounds (above).
2. Contract sketch shows `document_artifacts() -> &[ArtifactDecl]` but landed signature is
   `-> &[ArtifactKind]` (`scheme.rs`). Not a C1 deliverable — noted as a later-C follow-up,
   not folded in here.

---

## 6. Files + gates

**Add:** `crates/scheme/src/document_context.rs`, `crates/capco/tests/document_rollup.rs`
**Modify:** `crates/scheme/src/lib.rs` (mod+re-export), `crates/scheme/src/scheme.rs`
(defaulted `canonical_document_join`)
**No change:** `crates/capco/src/scheme/marking.rs`, `marking_scheme_impl.rs`, any engine crate.

```bash
cargo fmt --all
rustup run stable cargo clippy -p marque-scheme --all-targets -- -D warnings
rustup run stable cargo clippy -p marque-capco  --all-targets -- -D warnings
rustup run 1.89 cargo test -p marque-scheme
rustup run 1.89 cargo test -p marque-capco
rustup run stable cargo build -p marque-scheme --target wasm32-unknown-unknown
rustup run stable cargo build -p marque-capco  --target wasm32-unknown-unknown
```

No corpus/audit/G13/perf gates triggered (no engine change, no audit surface change).
