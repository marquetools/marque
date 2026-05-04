<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Specification Quality Checklist: Engine + Rule Architecture Refactor

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-05-03
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

> **Note on "no implementation details"**: This is a refactor of an existing
> Rust workspace. The spec uses internal type names (`Canonical<S>`,
> `FixIntent<S>`, `MarkingScheme`, `PageContext`, `Phase::Localized`) and
> source-file anchors (`engine.rs::build_decoder_diagnostic`,
> `parser.rs:1011-1024`) because these are the **user-observable surface**
> for the layered users this spec serves: the rule author whose API is
> `FixIntent<S>`, the maintainer whose pivot type was `IsmAttributes` and
> is becoming `ParsedAttrs`/`CanonicalAttrs`/`ProjectedMarking`, and the
> compliance auditor whose audit-record schema field shapes are the
> deliverable. The spec does not prescribe how `Canonical<S>`'s internal
> bytes are stored or how `MarkingScheme::project` is implemented — those
> remain plan/implementation concerns. Treat this checklist item as a
> graded pass for an internal-platform refactor spec, not as a fail.

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)

> **Note on "technology-agnostic" success criteria**: SC-008/SC-009
> reference Criterion benches and specific corpus paths. These are
> measurement *artifacts* (where the verification happens), not
> measurement *targets* (what is being verified). The targets — p95 ≤ 16
> ms, p99 within baseline + 5%, multi-page projection within baseline +
> 10%, R² ≥ 0.9 linear scaling — are technology-agnostic. The artifact
> references are necessary for verifiability (Spec template guidance:
> "verifiable without knowing implementation details" — the targets are
> verifiable without the bench mechanics; the artifacts are how the team
> verifies them in CI).

- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

> **Note on "no implementation details leak"**: Same caveat as Content
> Quality above. The spec describes the user-observable contract of an
> internal-platform refactor; the names of types, source files, and CI
> tools are the contract surface for the rule author, maintainer, and
> compliance auditor users.

## User Story Coverage Map

Each user story maps to functional requirements and success criteria:

| User Story | Priority | FRs | SCs |
|---|---|---|---|
| US1 — Audit records carry no document content | P1 | FR-001..FR-005, FR-027..FR-028 | SC-001, SC-012 |
| US2 — Page-level rollup correct for foreign markings | P1 | FR-006..FR-010 | SC-002 |
| US3 — Pass-1 / pass-2 don't corrupt each other | P2 | FR-021..FR-024, FR-041 | SC-007, SC-009 |
| US4 — Open-vocab not silently corrupted | P2 | FR-015..FR-017 | SC-011 |
| US5 — Citations mechanically verifiable | P2 | FR-018..FR-020 | SC-005, SC-006 |
| US6 — Lattice laws hold | P2 | FR-011..FR-014 | SC-003, SC-004 |
| US7 — Performance preserved | P3 | FR-029..FR-033 | SC-008, SC-009 |
| US8 — PRs independently revertable | P3 | (process; supported by FR-005, FR-038..FR-040) | SC-013, SC-014 |

All P1 user stories are independently testable — US1 (audit canary scan)
and US2 (foreign-banner corpus) deliver value if landed alone. US3..US6
are P2 because they fix correctness defects with smaller blast radius
than US1/US2; each also independently testable. US7/US8 are P3 because
they are non-functional / process properties that gate the P1/P2 work
rather than delivering user value directly.

## Functional Requirements Coverage Map

| Theme | FRs |
|---|---|
| Audit-record integrity (G13) | FR-001..FR-005 |
| Page-level rollup correctness | FR-006..FR-010 |
| Lattice law compliance | FR-011..FR-014 |
| Open-vocabulary parser correctness | FR-015..FR-017 |
| Citation fidelity | FR-018..FR-020 |
| Two-pass apply correctness | FR-021..FR-024 |
| Rule emission API | FR-025..FR-026 |
| Decoder constraints | FR-027..FR-028 |
| Performance budgets preserved | FR-029..FR-033 |
| Audit schema cutover (clean break) | FR-034..FR-037 |
| Process and test discipline | FR-038..FR-041 |

## Notes

- Items marked complete after first validation pass; no `[NEEDS CLARIFICATION]`
  markers were emitted because the source plans (2026-05-02 consolidated +
  2026-05-01 lattice) are post-murder-board and post-user-decision-pass —
  substantive scope and approach decisions are resolved.
- One known deferred decision is documented as an Edge Case + reflected in
  SC-010: the mangled-corpus accuracy baseline may shift when the decoder
  is locked out of open-vocabulary canonicalization (PR 3c). This is
  flagged as an explicit deferral, not a hidden one — implementation-time
  tactical decision recorded in PR 3c review notes, not a scope question
  for this spec.
- The spec deliberately does not enumerate every CAPCO §-citation that
  individual rules touch; that level of detail belongs to PR 3.7's lattice
  spike and PR 0.6's citation-defect fix. The spec's job is to require
  that all §-citations be mechanically verifiable (FR-018) and that every
  cited authority have a corpus fixture (FR-019), not to enumerate them.
- The lattice design doc (`docs/plans/2026-05-01-lattice-design.md`) is
  treated as a *deliverable inside* this spec (FR-013), not as a separate
  feature. PR 3.7 is the gating PR that completes that deliverable before
  PR 4 can land.
