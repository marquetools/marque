<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR Review: #6 — feat(phase-5): configurable severity, corrections map C001, and classifier-id plumbing

**Reviewed**: 2026-04-12
**Author**: bashandbone (Adam Poulemanos)
**Branch**: 001-marque-mvp → main
**Decision**: APPROVE with comments

## Summary
Solid implementation of US3 (Phase 5). Core correctness properties (C001 logic, FR-009 precedence, severity override, classifier_id flow) are all correct and well-tested. The PR has been through two prior independent reviews (Rust specialist + QA specialist) with 14 findings addressed. One remaining HIGH maintainability issue (constructor duplication) and 3 MEDIUM items.

## Findings

### CRITICAL
None

### HIGH
1. **Duplicate corrections_arc construction** (`engine.rs:51-80`): `Engine::new` and `Engine::with_clock` contain identical 5-line corrections_arc init blocks. If the init logic changes, one copy will silently diverge. Fix: have `Engine::new` delegate to `Engine::with_clock`.

### MEDIUM
1. **ENV_MUTEX scope documentation** (`precedence.rs:28-34`): Mutex serializes threads within one test binary only. Comment should state this explicitly to prevent future authors from misapplying it across test binaries.
2. **`migration_ref: "corrections-map"` is not a citation** (`rules.rs:945`): Other rules use CAPCO section identifiers. C001 uses a source label. Consider `None` or aligning with `citation` value `"CONFIG:[corrections]"`.
3. **`Config::corrections` is pub** (`lib.rs:89`): Mutating after Engine construction leaves cached `corrections_arc` stale. Add doc comment warning or make field `pub(crate)`.

### LOW
1. `explain_config` test uses `contains` not exact array assertion
2. No multi-token marking test for C001 (only single-dissem fixtures)
3. `EnvGuard::drop` SAFETY comment is present but less formal than `set`
4. Test name `capco_rule_set_registers_all_phase3_rules` not updated for C001

## Validation Results

| Check | Result |
|---|---|
| Clippy | Pass |
| Tests | Pass (222) |
| Format | Pass |
| Build | Pass |

## Files Reviewed
- `crates/marque-rules/src/lib.rs` — Modified (RuleContext.corrections field)
- `crates/marque-engine/src/engine.rs` — Modified (corrections_arc struct field + wiring)
- `crates/marque-capco/src/rules.rs` — Modified (C001 rule + registration)
- `crates/marque-config/src/lib.rs` — Modified (empty env var guard)
- `marque/src/render.rs` — Modified (test-only comment)
- `crates/marque-config/tests/precedence.rs` — Added (16 tests)
- `crates/marque-capco/tests/corrections_map.rs` — Added (10 tests)
- `crates/marque-capco/tests/rules_us1.rs` — Modified (+1 field)
- `marque/tests/cli_config.rs` — Added (10 tests)
- `marque/tests/corpus_provenance.rs` — Added (4 tests)
- `marque/tests/no_classifier_id_in_commits.rs` — Added (1 test)
- `crates/marque-capco/Cargo.toml` — Modified (dev-deps)
- `crates/marque-config/Cargo.toml` — Modified (dev-deps)
- `Cargo.lock` — Modified
