# Implementation Report: Phase 5 — Configurable Rule Severity and Corrections Map

## Summary
Implemented User Story 3 (T052-T060): configurable per-rule severity overrides, organization-specific corrections map with rule C001, classifier-identity plumbing into audit records, and comprehensive test suites verifying the four-layer config precedence chain, hard-fail validators, SC-006 classifier-id guard, and SC-002a corpus provenance scan.

## Assessment vs Reality

| Metric | Predicted (Plan) | Actual |
|---|---|---|
| Complexity | Medium | Medium |
| Confidence | 8/10 | 9/10 (most infrastructure pre-existed) |
| Files Changed | ~15 | 11 files (6 created, 5 updated) |

## Tasks Completed

| # | Task | Status | Notes |
|---|---|---|---|
| 1 | Add corrections map to RuleContext | Complete | Added `corrections: Option<Arc<HashMap<String, String>>>` |
| 2 | Wire corrections map from Config through Engine | Complete | Arc wrapping in Engine::lint() |
| 3 | Implement C001 corrections-map rule | Complete | CorrectionsMapRule with FixSource::CorrectionsMap |
| 4 | Register C001 in CapcoRuleSet | Complete | 10th rule registered |
| 5 | Wire --explain-config JSON output | Complete | Already implemented in prior phase |
| 6-7 | Config precedence + hard-fail tests | Complete | 13 tests covering T052+T053 |
| 8 | Corrections-map precedence tests | Complete | 7 tests covering T054+T060 |
| 9 | Corpus fixtures + SC-006 + provenance tests | Complete | 4 tests covering T055+T055a |
| 10 | CLI config integration tests | Complete | 9 tests covering end-to-end config |
| 11 | Final validation | Complete | clippy, fmt, test suite all green |

## Validation Results

| Level | Status | Notes |
|---|---|---|
| Static Analysis | Pass | cargo clippy --workspace -- -D warnings: zero warnings |
| Unit Tests | Pass | 214 tests (33 new), zero failures |
| Build | Pass | cargo check --workspace clean |
| Formatting | Pass | cargo fmt --check clean |
| Integration | Pass | CLI config tests all green |

## Files Changed

| File | Action | Lines |
|---|---|---|
| `crates/marque-rules/src/lib.rs` | UPDATED | +3 (corrections field + import) |
| `crates/marque-engine/src/engine.rs` | UPDATED | +8 (corrections Arc wiring) |
| `crates/marque-capco/src/rules.rs` | UPDATED | +55 (C001 rule + registration) |
| `crates/marque-capco/Cargo.toml` | UPDATED | +2 (dev-deps) |
| `crates/marque-config/Cargo.toml` | UPDATED | +3 (dev-deps) |
| `crates/marque-config/tests/precedence.rs` | CREATED | +298 (T052+T053) |
| `crates/marque-capco/tests/corrections_map.rs` | CREATED | +174 (T054+T060) |
| `marque/tests/no_classifier_id_in_commits.rs` | CREATED | +142 (T055) |
| `marque/tests/corpus_provenance.rs` | CREATED | +157 (T055a) |
| `marque/tests/cli_config.rs` | CREATED | +220 (CLI config tests) |
| `crates/marque-capco/tests/rules_us1.rs` | UPDATED | +1 (corrections field) |

## Deviations from Plan

1. **Task 5 (--explain-config)**: Already fully implemented in prior phase. No code change needed — only verified.
2. **TOML layout**: `confidence_threshold` is a top-level key, not under `[capco]`. Tests adjusted to write it before any `[table]` header.
3. **SC-006 allowlist**: Test sentinel values (LOCAL-42, ENV-99, LEAKED-42, from-root, from-sub) needed explicit allowlisting in the classifier-id scanner.
4. **Corpus provenance**: `.gitkeep` files needed to be added to the registered pattern list.

## Issues Encountered

1. **f32 JSON round-trip**: `confidence_threshold` (f32) round-trips through JSON as f64 with precision loss (0.95 → 0.949999988079071). Fixed by using approximate comparison in test.
2. **TOML table scoping**: Fields placed after `[capco]` header are parsed as part of that table, not top-level. Tests restructured to place `confidence_threshold` before any `[table]` header.

## Tests Written

| Test File | Tests | Coverage |
|---|---|---|
| `crates/marque-config/tests/precedence.rs` | 13 | T052 precedence chain + T053 hard-fail scenarios |
| `crates/marque-capco/tests/corrections_map.rs` | 7 | T054 FR-009 precedence + T060 classifier_id |
| `marque/tests/no_classifier_id_in_commits.rs` | 1 | T055 SC-006 guard |
| `marque/tests/corpus_provenance.rs` | 3 | T055a SC-002a corpus provenance |
| `marque/tests/cli_config.rs` | 9 | --explain-config, severity override, classifier_id, corrections |

## Next Steps
- [ ] Code review via `/code-review`
- [ ] Create PR via `/prp-pr`
