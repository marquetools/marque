<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7a Code Review

**Reviewer**: General code-quality reviewer  
**Branch**: `refactor-006-pr-7-phase-tagged-pass-split`  
**Commits in scope**: `7de14b00`, `20234e35`, `6bb53954`, `12ff9bb0`  
**Reference documents**: `pr-7-pm-decisions.md` (D-7.1 – D-7.3), `pr-7-architect-plan.md` §1–§3.2, `specs/006-engine-rule-refactor/spec.md` FR-021  
**Date**: 2026-05-13

> **Post-review resolution status** (annotation added 2026-05-13 by PM)
>
> All MEDIUM findings below have been addressed in commit `079974e7`
> (the reviewer-panel fixes commit). Review text preserved verbatim:
>
> - **MEDIUM-1 ("28 of 31" in PM doc D-7.2)** — RESOLVED. Decisions doc
>   now reads "27 of 31; 4 are Localized — C001, E006, E007, S004".
>   The same number propagated to `pr-7-architect-plan.md` §3.2 (which
>   Copilot independently flagged on the PR).
> - **MEDIUM-2 (test placement contradicts plan docs)** — RESOLVED.
>   The test correctly lives at `crates/capco/tests/phase_assignment.rs`
>   (Constitution VII forbids `marque-rules` depending on `marque-capco`).
>   Plan docs corrected to record the constitutionally required path.
> - **MEDIUM-3 (`SmallVec` import inconsistency, pre-existing)** —
>   DEFERRED to 7b per scope discipline; captured in
>   `pr-7-pm-decisions.md` D-7.13.
> - **LOW (narrative "Commit 7.4" reference)** — RESOLVED. The
>   reference was replaced with a stable pointer to
>   `crates/capco/tests/phase_assignment.rs` (the canonical
>   per-rule allowlist).

---

## Approval Status

**APPROVE** — No CRITICAL or HIGH issues. Three MEDIUM observations and one LOW note documented below.

---

## Findings (by severity)

### CRITICAL

None.

### HIGH

None.

### MEDIUM

**[MEDIUM] Count discrepancy in PM decisions doc vs. code (documentation stale)**

`docs/refactor-006/pr-7-pm-decisions.md` D-7.2 states: _"Most rules are whole-marking by construction (28 of 31)."_ The actual allowlist in `crates/capco/tests/phase_assignment.rs` has 27 WholeMarking + 4 Localized = 31 total. The Phase enum doc comment in `crates/rules/src/lib.rs` line 1028 correctly says "27 of 31". The code is correct; the PM decisions doc is wrong by 1.

This is a documentation inconsistency only — no code impact. However, the PM decisions doc is a stable reference artifact that reviewers will read for future 7b/7c work. A stale number there will cause confusion.

Recommended fix: update `pr-7-pm-decisions.md` D-7.2 to say "27 of 31" before 7b lands.

**[MEDIUM] Test file location deviates from both the architect plan and PM decisions doc**

Both `pr-7-architect-plan.md` line 66 and `pr-7-pm-decisions.md` line 54 and 402 specify the drift backstop as `crates/rules/tests/phase_assignment.rs`. The implementation placed it in `crates/capco/tests/phase_assignment.rs`.

The implementation is architecturally correct. `crates/rules` does not depend on `crates/capco` (by design — that would create a circular dependency per Constitution VII), so the test cannot live in `crates/rules/tests/` — it calls `CapcoRuleSet::new()`, which requires the `marque-capco` crate. The placement in `crates/capco/tests/` is the only sound choice.

The plans contain a structural error. No code change is needed, but both plan documents should be corrected to read `crates/capco/tests/phase_assignment.rs` so future contributors reading the plans do not question the deviation.

**[MEDIUM] `SmallVec` import inconsistency in `crates/engine/src/engine.rs`**

The PR adds `SmallVec` to the `use marque_rules::{...}` import at line 21 (via the re-export in `marque-rules`). However, pre-existing code in the same file uses the direct namespace `smallvec::SmallVec::new()` at lines 2204, 2706, and 3026. The new type aliases `Pass1Indices` and `Pass2Indices` use the imported name.

The inconsistency is purely cosmetic — `marque_rules::SmallVec` and `smallvec::SmallVec` are the same type — and clippy passes clean. Nevertheless, a 5-year maintainer reading this file will wonder why some uses qualify with `smallvec::` and others do not. The pre-existing inconsistency predates this PR (lines 2204/2706 were already there), so no regression was introduced. Flagged as a medium note for the 7b cleanup checklist; the `smallvec::SmallVec::new()` sites should migrate to the re-exported name so the whole file uses one namespace.

### LOW

**[LOW] `Engine` doc comment references `PR-3c.B Commit 7.4` without a git hash**

`crates/engine/src/engine.rs` lines 169-170 (in the `pass1_rule_indices` doc comment) says: _"4 of 31 in the CAPCO ruleset post-PR-3c.B Commit 7.4: C001, E006, E007, S004."_ The phrase `PR-3c.B Commit 7.4` is a narrative reference, not a git SHA or tag. The commit `a2fbf12b` ("PR 3c.B Commit 7.4: retire E059") exists in history, so the reference is real. But without a hash, a developer bisecting 18 months from now must grep git log to locate it. Low severity because the rule IDs listed are the canonical ground truth; the commit reference is context only.

---

## Spot-Check Summary

### Four Localized rules

| Rule | Struct | Rationale comment quality | Phase assessment |
|------|--------|---------------------------|-----------------|
| C001 | `CorrectionsMapRule` | Excellent — explains both the token-level span shape and the architectural note that C001 also runs as a pre-pass-0 text scan. The distinction between the rule-dispatch channel and the pre-pass-0 channel is load-bearing for 7b reviewers. | Correct. Single-token correction-map replacement; span never crosses token boundary. |
| E006 | `DeprecatedDissemRule` | Good — names the migration-table mechanism and the example `LIMDIS → LIMITED DISTRIBUTION`. | Correct. The rule walks `token_spans` and fires one `Diagnostic` per deprecated token. |
| E007 | `XShorthandDateRule` | Good — names both the migration-table path and the pattern-strip derivation (`25X1- → 25X1`). | Correct. Same single-token walker shape. |
| S004 | `RelToTrigraphSuggestRule` | Good — explicitly notes that `Severity::Suggest` means the engine never auto-promotes, then adds: _"the phase declaration governs dispatch even for suggest-only rules."_ That sentence is the key insight a future maintainer needs — S004's pass-1 slot is latent (nothing is auto-applied in pass-1 for Suggest-severity), but the classification is still meaningful for dispatch bookkeeping in 7b. | Correct. Single-trigraph text correction; span is one token. |

### Three defensive WholeMarking choices examined

**E008 (UnknownTokenRule)** — `WholeMarking` despite firing on a single `Unknown` span. The rationale explains the cross-token suppression condition: `attrs.sar_markings.is_some()` determines whether a repeated-SAR shape is suppressed. This cross-token read is real (verified in `rules.rs` lines 1093-1106). Classifying it as `WholeMarking` with the D-7.2 "conservative dispatch" rationale is the correct call for a no-fix diagnostic.

**E053 (NofornRelToConflictRule / `DeclarativeNofornRelToConflictRule`)** — `WholeMarking`. The phase comment explains both paths: portion-scope emits `FactRemove(REL_TO)` with `candidate_span` for engine re-render; banner/CAB scope emits no-fix and delegates to the `capco/noforn-clears-rel-to` PageRewrite. The banner path is intrinsically whole-marking by construction. Classifying the portion path as WholeMarking is conservative but sound — the `FactRemove` intent re-renders the full portion, not just the NOFORN token's span.

**E014 (DeclarativeDualClassificationRule)** — `WholeMarking`. Rationale cites `§H.3` mutual exclusion across the classification axis (US vs. non-US vs. JOINT). The fix intent is empty (`fix_intent: None`) because the cross-axis renormalization requires classifier input. This is a correct WholeMarking: the rule reads the entire `MarkingClassification` + `ForeignClassification` axes and has no token-scoped fix.

---

## Constitution Compliance

### Principle V (Audit-First Compliance)

`AppliedFix` struct: no new fields in this PR (confirmed by `git diff 74309770..HEAD -- crates/rules/src/lib.rs` showing only doc-comment additions).  
`MessageArgs` struct: `crates/rules/src/message.rs` has no diff in this PR.  
`FixProposal` struct: no changes.  
`Phase` declaration affects dispatch categorization only; it does not appear in audit records. Constitution V satisfied.

### Principle VI (Dataflow Pipeline)

`Engine::fix_inner` is unchanged. The new `partition_rules_by_phase` function runs once at `Engine::with_clock` time and caches the result on the `Engine` struct. The cached partition is read-only data on an immutable field pair (`pass1_rule_indices`, `pass2_rule_indices`). No dispatch state mutation, no pipeline phase boundary crossed. Constitution VI satisfied.

The `#[allow(dead_code)]` attributes on the two fields are scoped tightly to those fields (lines 185 and 188) and carry an explicit comment naming the PR and checklist item for removal in 7b. This is correct practice.

### Principle VIII (Authoritative Source Fidelity)

Phase rationale comments in rules that include CAPCO citations were spot-checked against `crates/capco/docs/CAPCO-2016.md`:

| Citation in phase() comment | Page content | Status |
|------------------------------|-------------|--------|
| `§H.8 p145` — NOFORN cannot be used with RELIDO (E054) | p145 = NOFORN entry; states "Cannot be used with REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY" | Verified |
| `§H.8 p154` — RELIDO cannot be used with DISPLAY ONLY (E055) | p154 = RELIDO entry; states "Cannot be used with NOFORN or DISPLAY ONLY" | Verified |
| `§H.8 p136` — ORCON may not be used with RELIDO (E056) | p136 = ORCON entry; states "May not be used with RELIDO" | Verified |
| `§H.8 p140` — ORCON-USGOV may not be used with RELIDO (E057) | p140 = ORCON-USGOV entry; states "May not be used with RELIDO" | Verified |
| `§H.3` — mutual exclusion (E014, E016, E010 phase comments) | p55-57 = JOINT section; mutual-exclusion language present | Verified |
| `§H.7 / §B.3` — non-US classification requires dissem (E015) | p122 = FGI commingling; §B.3 p20 = FD&R basis | Verified (pre-existing citation in surrounding code) |
| `§H.9` — NODIS/EXDIS mutual exclusion (E037) | p172 = EXDIS entry; p174 = NODIS entry; mutual-exclusion language present | Verified |

No fabricated or unverifiable citations detected. Constitution VIII satisfied for the citations reviewed.

---

## Test Confirmation

```
cargo fmt --check --all         → clean (no output, zero exit)
cargo +stable clippy --workspace --all-targets -- -D warnings → clean (no warnings)
cargo test --workspace          → all test results OK, 0 failures
cargo test --package marque-capco --test phase_assignment → 2 passed, 0 failed
```

The two tests in `phase_assignment.rs` are well-structured:

1. `every_registered_rule_declares_expected_phase` — BTreeMap for deterministic failure output; duplicate-row guard on `EXPECTED_PHASES`; three-category failure message (missing from registration, missing from allowlist, phase mismatch); cardinality fast-fail before per-rule diff. Informative failure messages throughout.

2. `allowlist_partitions_match_engine_partition_arithmetic` — independent counting check that catches double-counting from a hand-merge into both sections. This redundant check is a good investment for a 5-year maintainability horizon — the first test would catch duplicate rows, but the second catches the count-math failure mode cleanly.

Allowlist ordering: sorted by phase then by rule ID within phase (Localized first, WholeMarking second; alpha within each group). The comment at lines 545-548 explains the ordering rationale. Diff-friendly for review.

No `TODO`, `FIXME`, or `unimplemented!` calls in any new code. No debug `println!` or `dbg!` calls.

---

## Review Summary

| Severity | Count | Status |
|----------|-------|--------|
| CRITICAL | 0     | pass   |
| HIGH     | 0     | pass   |
| MEDIUM   | 3     | warn   |
| LOW      | 1     | note   |

**Verdict: APPROVE** — No CRITICAL or HIGH issues. The three MEDIUM items are documentation corrections (stale count in PM decisions doc, plan file location error) and a pre-existing import inconsistency in engine.rs. None block merge. Recommended actions before 7b lands: correct the "28 of 31" count to "27 of 31" in `pr-7-pm-decisions.md` D-7.2 and note the test file location correction in both plan documents.
