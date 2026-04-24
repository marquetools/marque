<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque-scheme

Domain-neutral trait surface for structured marking schemes.

A marking system is a **typed algebra over a bounded lattice**, with a
constraint predicate and a lossy projection operator, plus local
probabilistic disambiguation at a small number of decision points. This
crate carries the trait and data types that express that abstraction;
concrete schemes (`marque-capco`, future `marque-cui`, etc.) implement
`MarkingScheme` against their own `Marking` type.

See `docs/plans/2026-04-17-marking-scheme-lattice-design.md` in the
workspace root for the design discussion and phased migration sequence.

## Modules

- `lattice` — `Lattice`, `BoundedLattice` traits.
- `category` — `Category`, `AggregationOp`, `Cardinality`,
  `IntraOrdering`.
- `constraint` — `Constraint` enum, `ConstraintViolation`.
- `template` — `Template` for structural parsing (portion/banner/CAB
  wrapping, category presence).
- `projection` — `Projection` trait + default per-category reducer.
- `ambiguity` — `Parsed<M>`, `Candidate`, `EvidenceFeature`.
- `scheme` — `MarkingScheme` trait.

## Status

Phase A scaffolding. The trait and primitives are stable; concrete
adapters land as separate PRs (Phase B onwards).

## License

Marque License 1.0 (`LicenseRef-MarqueLicense-1.0`). See [LICENSE.md](./LICENSE.md).
