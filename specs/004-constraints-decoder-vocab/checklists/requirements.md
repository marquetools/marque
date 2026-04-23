# Specification Quality Checklist: Declarative Rule Expression, Probabilistic Recovery, and Full Vocabulary Metadata (Phases C–E)

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-04-20
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

### Validation review (iteration 1)

**Content Quality** — passes with judgment calls:

- The spec references several concrete type names (`Send + Sync`, `Ambiguous`, `(C)`, CLI flag opt-in shape). These are retained where removing them would obscure the acceptance scenario or where the term is already domain vocabulary (e.g., `(C)` is how the CAPCO manual refers to the CONFIDENTIAL portion marking). Framework-specific types (`Vec<T>`, trait/crate/module names that only exist in the plan) are avoided in the user-facing sections and appear only in file paths (`marque-engine`, `marque-scheme`, etc.) where referencing the crate graph is the shortest unambiguous description.
- Written for mixed stakeholders — compliance leadership (who understand the domain vocabulary like FOUO, NOFORN, SCI) and technical reviewers preparing the plan. Pure-business readability (e.g., a Fortune 500 procurement officer with no IC background) was not the target; the domain is narrow.

**Requirement Completeness** — passes cleanly:

- Zero [NEEDS CLARIFICATION] markers. The 2026-04-19 plan is unusually thorough on thresholds, gate criteria, and deliverable boundaries, so reasonable defaults cover every ambiguous corner without clarification questions.
- FR-001 through FR-023 are testable. Each either declares a capability ("MUST provide X") that can be verified by running the feature, or a negative constraint ("MUST NOT downgrade under condition Y") that can be verified by attempting the prohibited case.
- Success criteria mix quantitative (latency milliseconds, accuracy percentages, fixture cardinality) and qualitative (audit-record auditability, citation traceability) outcomes, all measurable.
- Edge cases explicitly cover the four non-obvious failures: grammar-template mismatch in the decoder, custom rewrite without axis annotations, cycle detection, audit schema migration.

**Feature Readiness** — passes cleanly:

- Three independently-testable user stories at P1/P2/P3. Each delivers value on its own and has its own acceptance scenarios.
- The P1 slice alone is a viable deliverable (Phase C ships constraints + rewrites; corpus stays byte-identical; work unblocks future scheme adoption).

### Constitution gate notes

- **Principle VIII (Authoritative Source Fidelity)** applies heavily in this spec. FR-021 and SC-009 encode the discipline explicitly. Plan phase must include citation-verification steps, not just "add citation" steps.
- **Principle IV (Two-Layer Rule Architecture + scheme-adoption PRs don't edit engine)** encoded as FR-022. Plan phase should flag any discovered engine gap as a separate predecessor PR before proceeding.
- **Principle III (WASM rejects semantic-surface-expanding runtime config)** encoded as FR-013 third clause. Plan phase should set up a compile-time check for this, not a runtime check.
- **Principle V (content-ignorance in audit records)** encoded as the Assumptions section's final bullet. Plan phase should include corpus-level integration tests that grep audit-record output for document text.
- **Principle VI (rule/recognizer impls Send + Sync, no global mutable state)** encoded as FR-023.

All items pass; no remaining updates required before `/speckit.clarify` or `/speckit.plan`.
