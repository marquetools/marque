# Contract: Shared Constraint Evaluator

**Crate:** `marque-scheme`
**Surface type:** Free function + trait extension
**Phase:** C
**Spec refs:** FR-001, FR-002, SC-005

## Intent

A single evaluator operates over `&[Constraint]` for any scheme `S`. Adapters (`marque-capco`, future `marque-cui`) declare their constraints as data; the evaluator is inherited, not reimplemented.

`Constraint` is a non-generic enum (`Conflicts`, `Requires`, `Implies`, `Supersedes`, `Custom(&'static str)`) already present in `crates/scheme/src/constraint.rs` since Phase B. Phase C keeps that shape verbatim — no redesign. The evaluator is a free function parameterized over `S: MarkingScheme`; constraints themselves do not carry `<S>` because every variant operates through `S::Marking` / `S::Category` projections that the scheme exposes.

## Surface

```rust
pub fn evaluate<S: MarkingScheme>(
    constraints: &[Constraint],
    marking: &S::Marking,
    scheme: &S,
) -> Vec<ConstraintViolation>;
```

`MarkingScheme::validate` calls `evaluate(self.constraints(), marking, self)` and appends any scheme-specific non-constraint validations afterward.

## Contract

- **Determinism:** Given the same `constraints` slice and `marking`, `evaluate` returns the same `Vec<ConstraintViolation>` regardless of call site or thread.
- **Ordering:** Violations returned in constraint-declaration order within a single evaluation pass.
- **No mutation:** `evaluate` mutates nothing; the input slice and marking are borrowed immutably.
- **No allocation on hot path beyond the output `Vec`:** Implementations MUST NOT allocate per-constraint intermediate state.
- **Cite-on-violation:** Every `ConstraintViolation` carries a `&'static str` `citation` field pointing at the authoritative-source passage declared alongside the triggering constraint (see `CapcoScheme::constraint_citations()` lookup). Citation storage follows foundational-plan §7a — `&'static str` section references, NOT a structured `SourceCitation` type.

## Failure modes

None inherent. A malformed constraint (e.g., `Requires` pointing at a category that doesn't exist in `S`) is a scheme-construction error caught at `Engine::new`, not at `evaluate` time.

## Test scenarios

1. Constraint evaluator returns identical output for two `marque-capco` markings that differ only in category-join order (determinism).
2. Constraint evaluator on a `MarkingScheme` with zero constraints returns empty `Vec`.
3. A `Conflicts` constraint triggered by both sides being present returns exactly one violation carrying a `&'static str` `citation` matching the declared authoritative-source passage.
