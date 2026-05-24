<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7a Rust Review

**Branch**: `refactor-006-pr-7-phase-tagged-pass-split`
**Commits in scope**: `7de14b00`, `20234e35`, `6bb53954`, `12ff9bb0`
**Reviewer**: rust-reviewer agent
**Date**: 2026-05-13
**Compared against**: `74309770` (parent on `origin/staging`)

> **Post-review resolution status** (annotation added 2026-05-13 by PM)
>
> All findings below have been addressed in commit `079974e7` (the
> reviewer-panel fixes commit). The review text below is preserved
> verbatim as the historical record:
>
> - **M-1 (docstring bleed at `engine.rs:2343`)** — RESOLVED. While
>   fixing the surface issue, the underlying cause turned out to be
>   bigger: a doc block documenting `canonicalize_rule_overrides`
>   was placed above `Pass1Indices` instead of above the function it
>   documented; Rust silently attached the entire block to the type
>   alias. The fix moves the doc to its real owner; the type aliases
>   now have their own (correct) doc blocks. Verified by `cargo doc`
>   rendering and `cargo +stable clippy --workspace --all-targets
>   -- -D warnings`.
> - **M-2 (test placement deviation from plan docs)** — RESOLVED.
>   Plan docs (`pr-7-pm-decisions.md` D-7.2, `pr-7-architect-plan.md`
>   §1, §8) corrected to record that the test correctly lives at
>   `crates/capco/tests/phase_assignment.rs` (Constitution VII forbids
>   `marque-rules` depending on `marque-capco`).
> - **L-1 (C001 dual-path forward obligation)** — CAPTURED for 7b
>   implementer brief; tracked in `pr-7-pm-decisions.md` D-7.13
>   "Forward obligations".

---

## Diagnostic Status

- CRITICAL: 0
- HIGH: 0
- MEDIUM: 2
- LOW: 1

---

## Approval Status

WARN — MEDIUM issues only. No CRITICAL or HIGH issues found. The PR is mergeable with the fixes described below noted as pre-7b obligations on the implementer.

---

## Findings (by severity)

### CRITICAL

None.

### HIGH

None.

### MEDIUM

#### M-1: Doc comment bleed on `Pass1Indices` / `Pass2Indices` type aliases (`crates/engine/src/engine.rs:2343`)

The `/// Pass-1 (Localized) rule-index partition...` doc comment for `type Pass1Indices` is placed immediately after the last `///` line of the `canonicalize_rule_overrides` function's doc block, with no blank line separator. Rustdoc parses a continuous run of `///` lines as a single doc block, so the `Pass1Indices` doc comment is silently appended to `canonicalize_rule_overrides`'s documentation and `type Pass1Indices` renders as an undocumented item.

The fix is one blank line between `/// the expected behavior.` and `/// Pass-1 (Localized) rule-index partition`. No behavior change; documentation-only defect. Flagged MEDIUM because it corrupts the documentation for the primary private function in `engine.rs` that maintainers will read when working on the pass-split in 7b and beyond.

Exact location: `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/crates/engine/src/engine.rs` lines 2341–2343.

#### M-2: `phase_assignment.rs` test lives in `crates/capco/tests/` but all three authoritative docs specify `crates/rules/tests/`

`pr-7-pm-decisions.md` D-7.2 (line 54), `pr-7-architect-plan.md` §1 (line 66) and §8 (line 841), and the implementing commit message all say `crates/rules/tests/phase_assignment.rs`. The test was placed at `crates/capco/tests/phase_assignment.rs`.

The placement in `crates/capco/tests/` is arguably more correct — the test exercises `CapcoRuleSet::new()` which is `marque-capco`'s public surface, and the existing `post_3b_registration_pin.rs` lives there as a direct precedent. However, the deviation from all three authoritative documents without a stated rationale introduces a maintenance discoverability gap: a reviewer following the plan to locate the drift backstop will not find it where the plan says it is.

**Proposed resolution**: Either (a) add a one-line comment in the test file's module doc explaining the placement rationale ("per the existing `crates/capco/tests/post_3b_registration_pin.rs` precedent, rule-registration tests live here rather than in `crates/rules/tests/`"), or (b) note this placement decision explicitly in the PR description so it is on record. The test itself is correct and the placement is functionally appropriate; this is a documentation/discoverability concern, not a correctness defect.

### LOW

#### L-1: C001 dual-path note in `phase()` doc comment is forward-looking but not fully pre-emptive for 7b

C001's `fn phase(&self) -> Phase` doc comment correctly identifies the dual-path architecture and says "The phase tag governs the rule-dispatch path; the pre-pass-0 path is a separate channel that bypasses rule dispatch entirely." This is accurate for 7a (partition stored, not dispatched). However, when 7b lands and pass-1 dispatch goes live, C001 will appear in `pass1_rule_indices` and the 7b implementer must ensure that pass-1 dispatch excludes C001 (or that the post-pass-0 attrs contain no remaining C001 matches, making pass-1 dispatch a no-op for C001). Neither outcome is guaranteed by the current structure — the partition includes C001 in pass-1 unconditionally.

This is LOW because the concern belongs to 7b and the doc comment does flag the dual-path; the current PR's behavior is correct. The 7b checklist should include: "confirm pass-1 dispatch of C001 is either excluded by construction or is idempotent after pass-0." A note in the checklist at `pr-7-pm-decisions.md` D-7.6's implementation notes section would close this.

---

## Per-rule Phase Verification

The following table covers the 4 Localized rules and 3 defensive WholeMarking choices the review prompt flagged. All verify against the rules' `check()` bodies and the fix span analysis.

| Rule | Declared Phase | Span Evidence | Assessment |
|------|---------------|---------------|------------|
| **C001** `CorrectionsMapRule` | Localized | `check()` iterates `attrs.token_spans`, emits `make_fix_diagnostic` with `span: token_span.span` — strictly one `TokenSpan` per diagnostic. Every fix span is the span of a single `TokenSpan`. | PASS |
| **E006** `DeprecatedDissemRule` | Localized | `check()` iterates `attrs.token_spans`, emits `make_fix_diagnostic` with `span: token.span` — one token per diagnostic. Migration-table hit walks a single `DissemControl` or `Unknown` token; the span is that token's span only. | PASS |
| **E007** `XShorthandDateRule` | Localized | `check()` iterates `attrs.token_spans` filtering for `Unknown` tokens. Both Path 1 (migration table) and Path 2 (pattern strip) emit `make_fix_diagnostic` with `span: token.span` — the span of the single `Unknown` token the rule walked. Pattern strip of trailing `-` produces a new string but the span remains the original token span, not an expanded span. | PASS |
| **S004** `RelToTrigraphSuggestRule` | Localized | `check()` pulls `trigraph_spans.get(idx)` — one `RelToTrigraph` token per `rel_to` entry — and emits `Diagnostic::text_correction` with `span: span_token.span`. Single-token span per diagnostic. `Severity::Suggest` means the engine never auto-promotes, but the phase declaration is still correct and important: it governs dispatch even for suggest-only rules. | PASS |
| **E008** `UnknownTokenRule` (defensive WholeMarking) | WholeMarking | `check()` reads `attrs.sar_markings.is_some()` — cross-token SAR-suppression state — before deciding which `Unknown` tokens to emit diagnostics for. The firing decision is not purely per-token; suppression reads a parsed structural result from a different token class. The implementer's defensive `WholeMarking` choice is justified: even though each emitted diagnostic points at a single `Unknown` token span, the *decision whether to emit* reads cross-token state. | JUSTIFIED — WholeMarking is correct |
| **S005** `RelToOpaqueUncertainReductionSuggestRule` (defensive WholeMarking) | WholeMarking | `analyze_uncertain_reduction()` (shared backend for S005/S006) reads `page.portions()` — the full page-level REL TO accumulation across all portions — and computes atom-semantics intersection across multiple portions. Both the firing decision and the span-selection logic are list-scoped, not per-token. | JUSTIFIED — WholeMarking is correct; no Localized case exists |
| **S006** `RelToOpaqueUncertainReductionInfoRule` (defensive WholeMarking) | WholeMarking | Same backend (`analyze_uncertain_reduction`) as S005, filtered to the Info branch. Identical list-scoped decision. | JUSTIFIED — same rationale as S005 |

Summary: All 4 Localized phase assignments are correct. All 3 defensive WholeMarking choices are justified. No Localized rule emits a fix whose span extends beyond the single token the rule walked.

---

## CHK015 Gate Clearance

CHK015 requires: "no rule registers under both phases via twin structs sharing a backend module." Verified: the `Phase` enum has exactly two variants (`Localized`, `WholeMarking`) with no `Phase::Both` escape hatch. The enum doc comment in `crates/rules/src/lib.rs:222` explicitly prohibits `Phase::Both`. No twin-struct pairs were found in the diff that share a backend module with different `Phase` declarations. The `EXPECTED_PHASES` allowlist in `phase_assignment.rs` assigns each rule ID exactly once. CHK015: CLEARED for 7a.

CHK018 (R002 contract) is scoped to 7b per `pr-7-pm-decisions.md` D-7.14. Not evaluated in this review.

---

## Lint + Test Confirmation

### `cargo check --workspace`

Clean. All 7 workspace members check without errors or warnings.

### `cargo +stable clippy --workspace --all-targets -- -D warnings`

Clean. No warnings emitted on stable clippy. The `#[allow(dead_code)]` annotations on `pass1_rule_indices` and `pass2_rule_indices` in `engine.rs` are per-field (not blanket) and suppress the expected unused-field warnings for PR 7a. The annotations carry a comment naming the reason ("populated in 7a; consumed in 7b"). Clippy accepts this without complaint.

The patterns flagged in `pr-7-rust-review.md` §7 as stable-vs-nightly divergence risks were checked:
- `clippy::match_single_binding`: all `fn phase(&self) -> Phase { Phase::WholeMarking }` bodies are direct returns, not single-arm matches. No warning.
- `clippy::const_is_empty`: the `Phase` enum and `additional_emitted_ids` default return no const-context empty slices that would trigger this.
- `clippy::missing_const_for_fn`: `fn phase(&self) -> Phase` is not `const fn`. Stable clippy did not suggest this for the trait-method implementations on non-ZST rule structs. No warning.

### `cargo fmt --check`

Clean. No formatting differences.

### `cargo test --workspace`

All tests pass. The new `phase_assignment` test binary ran 2 tests (`every_registered_rule_declares_expected_phase` and `allowlist_partitions_match_engine_partition_arithmetic`), both passing.

---

## Additional Notes

**Scope discipline**: Verified. The 4 commits touch only:
- `crates/rules/src/lib.rs` — `Phase` enum and `Rule::phase()` default
- `crates/capco/src/rules.rs` and `crates/capco/src/rules_declarative.rs` — per-rule `fn phase()` overrides
- `crates/engine/src/engine.rs` — partition fields + `partition_rules_by_phase` function + type aliases
- `crates/capco/tests/phase_assignment.rs` — new drift backstop

No changes to `Engine::fix_inner`, `MessageTemplate::ReparseFailed`, `FeatureId::PrecedingFixPenalty`, `MARQUE_AUDIT_SCHEMA`, or `RuleContext` shape. Those are 7b/7c scope and are absent from this diff.

**`Phase` enum derives**: `Debug, Clone, Copy, PartialEq, Eq, Hash` — correct set for a two-variant fieldless enum used in collections and comparisons. `Serialize`/`Deserialize` are absent, which is correct — `Phase` is a rule-internal dispatch tag, not an audit-record field.

**Default `WholeMarking` rationale**: The `fn phase(&self) -> Phase` trait method doc comment explicitly cites D-7.2 from `docs/refactor-006/pr-7-pm-decisions.md` and explains the conservative-default reasoning. The inline-size choices for `Pass1Indices` (`[(usize, usize); 4]`) and `Pass2Indices` (`[(usize, usize); 32]`) are documented with rule-count rationale in the field doc comment.

**`partition_rules_by_phase` function**: Clean two-level loop over `rule_sets[i].rules()[j]`, exhaustive `match` over `Phase`'s two variants, no wildcard arm. Adding a third variant would force a compile error here — correct design.

**`phase_assignment.rs` test shape**: The test correctly implements the drift backstop described in D-7.2. The primary test (`every_registered_rule_declares_expected_phase`) uses `BTreeMap` for deterministic diff output on failure, includes a cardinality fast-fail before the per-rule diff, and produces an informative failure message that names all three failure modes (missing from registration, missing from allowlist, phase mismatch). The secondary test (`allowlist_partitions_match_engine_partition_arithmetic`) provides an independent counting view that would catch a hand-merge double-count error that the primary test's duplicate-row guard already handles. This is belt-and-suspenders and does no harm. The module doc includes a clear drift policy: "do NOT silently edit EXPECTED_PHASES to make CI green."
