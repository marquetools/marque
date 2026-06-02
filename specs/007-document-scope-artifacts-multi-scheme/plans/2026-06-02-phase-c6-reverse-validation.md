# Phase C6 (T036) — Reverse validation + "classified up to" FrontMarking

**Branch:** `007-phase-c6` (off merged `origin/main`, C5 = #854 squash `518a7998`).
**Closes:** Phase C checkpoint — "the two #799 motivating unwired-edge cases produce
diagnostics; derivations audited." (US1 portion-cascade case landed C1–C5; this PR lands the
US2 reverse-validation case.)

## Task (T036, verbatim)
Reverse validation + "classified up to" `FrontMarking` node via `DiffInput` at `Scope::Document`.
Tests: front-marking vs all-pages divergence reported (the #799 motivating `(TS//SI-G//OC/RELIDO)`
case now fires) — the reverse-validation half of SC-012 (FR-015).

(The task's "at `Scope::Document`" names the *conceptual* comparison scope. `DiffInput` carries no
`Scope` field — it holds `from` / `to` / `relation` — so the scope is implied by the operands the
caller supplies, not encoded in the type.)

## Load-bearing fact (the design crux)
`MarkingScheme::Marking` **intentionally carries no `JoinSemilattice`/`Eq` bound**
(per the `MarkingScheme::Marking` doc comment in `crates/scheme/src/scheme.rs` — cross-axis fold
is a projection, not a lattice op).
Therefore divergence **cannot** be computed in marking space. It runs in **`S::Canonical`**
space, which is unbounded at the trait level but `Clone + Default + Eq` at the engine use-site
(`CanonicalAttrs` derives all three; CapcoScheme overrides the bridges
`canonical_from_marking` / `canonical_page_join` / `canonical_rank`).

The bridge: `DiffInput<S::Marking>` operands → `canonical_from_marking` → `S::Canonical` →
compare via `canonical_document_join` (LUB) + `Eq`.

## Divergence algorithm (canonical space, full-axis, domain-neutral)
```
lub = scheme.canonical_document_join(&[front, rollup])   // needs Clone + Default
if front == rollup        -> Match                        // needs Eq
else if lub == front      -> FrontOverClaims              // front strictly dominates body
else                      -> FrontUnderClaims             // body exceeds front somewhere (incl. incomparable) — the security case
```
`(TS//SI-G//OC)` front vs `(TS//SI-G//OC/RELIDO)` body: `lub == body != front` ⇒ `FrontUnderClaims` — fires. ✓
An `Ambiguous` operand (DiffInput holds `Parsed<M>`) is uncomparable ⇒ `Unresolved` (honest; never a false Match/Under).

## Deliverables

### marque-scheme — new `crates/scheme/src/reverse.rs`
- `Divergence` (`#[non_exhaustive]`, `Copy`): `Match | FrontUnderClaims | FrontOverClaims | Unresolved`.
- `ReverseValidation<S: MarkingScheme + ?Sized>` { `divergence: Divergence`, `front: ResolvedArtifact<S>` };
  manual `Debug`/`Clone`/`PartialEq` bounded on `S::Canonical` (derive over-constrains — resolution.rs pattern).
- `pub fn divergence<S: MarkingScheme + ?Sized>(scheme: &S, front: &S::Canonical, rollup: &S::Canonical) -> Divergence
  where S::Canonical: Clone + Default + Eq` (the algorithm above; `Unresolved` is engine-side only).
- `lib.rs`: `pub mod reverse;` + `pub use reverse::{Divergence, ReverseValidation, divergence};`.
- Content-ignorant (Constitution V/G13): only enum tags + `ResolvedArtifact` (already clean).

### marque-engine — `crates/engine/src/engine/constructors.rs`
Extend the `impl<S,R> Engine<S,R> where S::Canonical: Clone` block bound to `Clone + Default + Eq`; add:
```
pub fn reverse_validate(&self, diff: &DiffInput<S::Marking>) -> ReverseValidation<S>
```
1. Project `diff.from` / `diff.to`: `Unambiguous(m) => Some(canonical_from_marking(m))`, else `None`.
2. Either `None` ⇒ verdict `Unresolved`; else `divergence(&*self.scheme, &front_c, &rollup_c)`.
3. FrontMarking node: reuse C4 — `self.resolve_document(&rollup_c)`, `find(kind == FrontMarking)`,
   else synthesize `FlagOnly` (CAPCO declares no FrontMarking artifact, so it synthesizes).
4. Return `ReverseValidation { divergence, front }`. `diff.relation` is provenance (not gated).

**Caller-invoked, NOT on `LintResult`** — the front marking is a caller-supplied operand the engine
does not auto-extract yet (Phase D parses the front line). Matches the `DiffInput` doctrine
("callers construct it explicitly; the engine does not fetch second markings", scope.rs). Hot path
unchanged.

## Decisions (reconciled from parallel architect + plan)
- D1 store `S::Canonical` (Marking has no Eq). D2 new `reverse.rs`. D3 directional via canonical join (not equality-only, not rank-only — rank misses same-level control divergence). D4 no decision-tracing (verdict ≠ edge firing; additive later). D5 not on LintResult. D6 take `DiffInput`; relation is informational. D7 ambiguous ⇒ `Unresolved`. D8 test stub `StubMarking(u32)` bitset, `Canonical = u32`, join = bitwise-OR.

## Tests (RED first)
Unit (scheme, bitset stub): equal⇒Match; superset⇒Over; subset⇒Under; incomparable⇒Under; variants distinct;
result carries verdict+node; eq distinguishes each field (manual-impl coverage); clone/debug content-ignorant.
Integration (engine `tests/reverse_validation.rs`, StubScheme bitset): `sc012_front_under_claim_reports_divergence`
(motivating); exact-match⇒no divergence; over-claim⇒over; ambiguous operand⇒Unresolved; CAPCO real-scheme
under-claim smoke (front node FlagOnly, verdict still computed) if cheap.

## Verify (Rust 1.89; no 1.85 installed)
`rustup run 1.89 cargo test -p marque-scheme` · `rustup run 1.89 cargo test -p marque-engine reverse_validation`
· `rustup run 1.89 cargo check --workspace --all-targets` · `rustup run stable cargo clippy --workspace --all-targets`
· `rustup run stable cargo fmt --check`

## Notes
- In-code comments: cite `#799` / CAPCO `§X.Y pNN` only; strip phase/T###/SC/FR anchors. `sc012_*` test names OK.
- Constitution IV "scheme-adoption PR must not edit engine" does NOT apply — this is a #799 domain-neutral
  engine-capability extension (like C1–C5), no CAPCO file edited.
