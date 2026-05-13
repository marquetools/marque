<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7b — Rust Specialist Review

**Branch**: `refactor-006-pr-7b-two-pass-r002`
**Date**: 2026-05-13
**Reviewer**: Rust Specialist Panel Member

---

## Top-Line Verdict

**APPROVE WITH WARNINGS**

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH     | 2 |
| MEDIUM   | 4 |
| LOW      | 3 |

Pre-flight gates: `cargo check` clean, `cargo +stable clippy -- -D warnings` clean,
`cargo fmt --check` clean, `cargo test` 0 failures. All gates pass.

---

## Pre-Review: Deviation Verification

### Deviation 1 — R002 trigger condition (total recognizer)

**VERIFIED CORRECT.** The implementation at `engine.rs:1718-1725` checks
`new_markings.is_empty() && !parsed_markings.is_empty()` after the re-parse. The
design intent is preserved: a buffer that enters pass-1 with ≥1 marking and exits
with zero markings has had its marking structure destroyed. The recognizer is total
(no `Err` path), so empty-markings is the appropriate proxy for parse failure.

**False-positive surface analysis**: A pass-1 correction that legitimately removes
the *only* marking on the page (e.g., a document containing exactly one portion mark
`(U)` whose corrected form is also recognized) would NOT trigger R002 because the
post-fix buffer still parses to a marking. The only trigger is when the resulting
buffer yields no markings at all — which means either the correction corrupted the
delimiters or removed the entire marking content. This is the intended signal. No
false-positive surface identified in typical use.

**One edge case**: a document containing only a banner with no portion marks.
Pass-1 corrects the banner in a way that makes it unrecognizable. Result: R002 fires
correctly. The trigger is conservative as documented.

### Deviation 2 — `_message_args` dead variable

**VERIFIED as documented, but flagged as HIGH below.**
`contributing_rule_ids` is constructed as `_message_args` at `engine.rs:2702-2705`
and is then not wired into the `Diagnostic::new` call at line 2716. The pre-flight
documents this is intentional — PR 3c.2 migrates `Diagnostic.message: Box<str>` to
`Message`. The `MessageArgs.contributing_rule_ids` field IS present (D-7.5 honored).
The wire-up path IS preserved via the `_message_args` binding. However, the current
form means the structured `contributing_rule_ids` payload is silently discarded —
no consumer of `remaining_diagnostics` can read the structured list today.

---

## Section 1: Lifetimes and Ownership

### HIGH-1 — Clone instead of Move in short-circuit branch (`engine.rs:1697, 1726`)

**Severity**: HIGH
**Location**: `crates/engine/src/engine.rs:1697` and `:1726`

```rust
// Line 1697 — short-circuit arm
let (pass2_source, pass2_markings) = if pass1.applied.is_empty() {
    (pass0.effective_source.clone(), parsed_markings)  // ← clone, should be move
} else {
    ...
    (pass1.post_buffer.clone(), new_markings)           // ← clone, should be move
};
```

The rust pre-flight Q3 explicitly specified "Move `parsed_markings` directly — no
`Cow`, no clone needed." Both branches violate this. In the short-circuit branch,
`pass0.effective_source` is a `Vec<u8>` that could be moved since `pass0` is not
used after this point. In the else branch, `pass1.post_buffer` is cloned when
`pass1` is also not used after this point in the non-R002 path.

These clones are performance regressions on the hot path: every document that
either (a) has no pass-1 fixes or (b) has pass-1 fixes that succeed through re-parse
performs an unnecessary full-source buffer copy. On a 10 KB document this is ~10 KB
of allocation per fix call.

**Root cause**: `pass0` is later borrowed in `assemble_r002_result` (the R002 path
references `pass0.applied` and `pass0.dropped_diags`). However, on the non-R002 path
`pass0` is not referenced after the pass1 block. The clone on line 1697 is
conservative because the compiler cannot see through the conditional structure.

**Suggested fix**: Destructure `pass0` before the branch or introduce a local
`effective_source = pass0.effective_source` binding extracted before the if-else, so
the source moves into the branch without cloning. Similarly for `pass1.post_buffer`
on line 1726.

### MEDIUM-1 — `assemble_r002_result` takes `&Pass0Result` but clones its contents

**Severity**: MEDIUM
**Location**: `crates/engine/src/engine.rs:2085-2094`

`assemble_r002_result` takes `pass0: &Pass0Result` by reference, then calls
`.iter().cloned()` on `pass0.applied` and `.iter().cloned()` on `pass0.dropped_diags`.
The method could instead take `pass0: Pass0Result` (move) and use `extend` directly,
avoiding the per-element clone. The pass0 value is not used after this call site
(line 1724). This is a minor allocation inefficiency on the R002 path (rare by
design) but the inconsistency with the move-semantics intent of the design is notable.

---

## Section 2: `_message_args` Dead Variable — Structured Payload Not Wired

### HIGH-2 — `_message_args` is constructed but silently discarded

**Severity**: HIGH
**Location**: `crates/engine/src/engine.rs:2702-2714`

```rust
let _message_args = marque_rules::MessageArgs {
    contributing_rule_ids,   // ← moved in here
    ..marque_rules::MessageArgs::default()
};
// ...
Diagnostic::new(
    R002_RULE_ID,
    Severity::Error,
    failure_span,
    message,      // ← plain Box<str>, structured payload gone
    R002_CITATION,
    None,
)
```

`contributing_rule_ids` is consumed by `_message_args` and then `_message_args` is
dropped unused. The `message` passed to `Diagnostic::new` is a `String`-derived
`Box<str>` that embeds the rule IDs as a comma-separated text string.

The consequences:
1. D-7.5 and D-7.17 require the structured `MessageArgs.contributing_rule_ids` field
   to be present AND accessible. It exists as a type — but no code path reads it from
   an R002 diagnostic, because `Diagnostic` does not carry a `MessageArgs` field yet
   (PR 3c.2 migration pending).
2. The structured list is only reconstructible by parsing the comma-separated text
   in `message`. That is a regression from the "closed-set discipline" that the whole
   `Message`/`MessageArgs` system is designed to enforce.
3. A reviewer reading line 2702 cannot easily verify whether `_message_args` is
   intentionally unused or accidentally unused — the underscore prefix obscures
   intent.

The pre-flight docs say this is intentional pending PR 3c.2. That's acceptable IF
there is a documented obligation to wire it up. The issue is that there is currently
**no tracked item** linking the `_message_args` construction at this call site to the
PR 3c.2 migration step. The obligation exists in the pre-flight doc comments but not
as a code comment at the binding site.

**Suggested fix**: Add a `// PR 3c.2 migration obligation: wire `_message_args` into
Diagnostic.message via Message::new(MessageTemplate::ReparseFailed, _message_args)`
comment at line 2702, so the connection to the migration is explicit in the code.
The binding name could stay `_message_args` to suppress the unused-variable warning
(Rust allows this), or be renamed to `_r002_message_args_pr3c2` to make the intent
machine-searchable. Without the comment the high risk is that the PR 3c.2 author
searches for `MessageTemplate::ReparseFailed` call sites and misses this one.

---

## Section 3: R002 Trigger and Correctness

### Item 3.1 — R002 trigger logic is correct but audit comment is mislabeled

**Severity**: LOW
**Location**: `crates/engine/src/engine.rs:1703-1717`

The comment at line 1703 says "Marque's recognizer is total — it never returns a
hard `Err`". The implication is correct and the trigger is sound. However the R002
trigger is described in the comment as the "re-parse failed" signal, but what is
actually being detected is "the post-fix buffer became marking-free." These are
correlated but not identical: a buffer could become marking-free for reasons other
than a corrupted marking (e.g., a correction that deleted all portion marks and the
document has no banner). The distinction is unlikely to matter in practice but the
doc comment slightly overstates the certainty of the failure-causation chain.

**No code change required.** Add a parenthetical clarification if desired.

### Item 3.2 — R002 emits into `remaining_diagnostics` — VERIFIED CORRECT

At line 2120 (`assemble_r002_result`) the R002 diagnostic is `push`ed into
`remaining_diagnostics`, not into `applied`. Constitution V Principle V is honored.
`build_r002_diagnostic` does not call `__engine_promote` — confirmed by inspection.
`FixResult.r002_fired: true` is set only in `assemble_r002_result` (line 2134) and
the normal path has it `false` at line 1787. Every `FixResult` construction site
initializes `r002_fired: false` correctly.

---

## Section 4: `apply_pass1_fixes` Algorithm

### Item 4.1 — Algorithm correct, capacity formula correct

**VERIFIED.** The `extra` capacity formula at `engine.rs:2204-2211` uses
`saturating_sub` per the Q2 spec. The `debug_assert!(fix.span.start >= cursor, ...)`
overlap guard at line 2215-2219 is present with an informative message. The reverse
iteration over FR-016-sorted fixes gives a left-to-right source walk. The algorithm
matches the pre-flight Q2 spec exactly.

### Item 4.2 — `sort_and_c1_dedup` has an extra intermediate allocation

**Severity**: MEDIUM
**Location**: `crates/engine/src/engine.rs:2167-2190`

```rust
fn sort_and_c1_dedup(synthesized: Vec<SynthesizedFix>) -> Vec<SynthesizedFix> {
    let mut fixes: Vec<&SynthesizedFix> = synthesized.iter().collect(); // ← intermediate ref-vec
    fixes.sort_by(...);
    ...
    kept_fixes.push((*fix).clone());  // ← clone per kept fix
}
```

The function allocates an intermediate `Vec<&SynthesizedFix>` for sorting by
reference, then clones each kept fix into `kept_fixes`. The natural alternative is to
sort `synthesized` in-place (`synthesized.sort_by(...)`) and iterate directly,
avoiding both the reference-vector allocation and the per-element clone.
The current form doubles allocations on this path. For the hot path (pass-2 with
potentially many fixes per document) this is a performance regression relative to the
pre-7b code. The pre-flight Q2 pattern uses `fixes.iter().rev()` on an in-place sort,
not a reference-vector sort.

**Suggested fix**: Sort `synthesized` in-place and drain into `kept_fixes` directly:
```rust
fn sort_and_c1_dedup(mut synthesized: Vec<SynthesizedFix>) -> Vec<SynthesizedFix> {
    synthesized.sort_by(|a, b| { ... });
    let mut kept = Vec::with_capacity(synthesized.len());
    let mut next_window_end: Option<usize> = None;
    for fix in synthesized {
        ...
        if fits { kept.push(fix); }
    }
    kept
}
```

---

## Section 5: `contributing_pass1_rule_ids` — `out.inline_size()` API Call

### Item 5.1 — `inline_size()` is a valid SmallVec method

**VERIFIED.** `SmallVec::inline_size()` exists in `smallvec-1.15.1` at
`lib.rs:950`. The usage at `engine.rs:2077` is correct and returns the compile-time
inline capacity (4 in this case).

### MEDIUM-3 — `contributing_pass1_rule_ids` uses `Vec` intermediate before `SmallVec`

**Severity**: MEDIUM
**Location**: `crates/engine/src/engine.rs:2064-2080`

The method allocates a `HashSet<RuleId>` for dedup, then a `Vec<RuleId>` for sorting,
then moves results into the `SmallVec`. With at most 4 elements by design, both the
`HashSet` and the `Vec` heap-allocate unnecessarily. An `ArrayVec` or a sorted-scan
approach over the small set would be more idiomatic for the known-small input.

This is the R002 path (rare), so the performance impact is minimal. The main concern
is conceptual: the function is described as "capped at 4" but uses unbounded
intermediate collections.

---

## Section 6: Phase-Span-Shape Check

### Item 6.1 — First-fire check placement and logic: VERIFIED CORRECT

The filter at `engine.rs:1846-1884` runs between `synthesize_fixes` and
`sort_and_c1_dedup`, before the C-1 dedup walk, exactly as specified in D-7.16.
`span_is_within_marking` at line 2144-2146 uses `>=` and `<=` (both endpoints
inclusive) per the design. `tracing::error!` + `debug_assert!(false, ...)` shape
matches D-7.16 exactly. Dropped fixes produce no `AppliedFix` records — verified.

### Item 6.2 — `find_containing_marking` linear scan is appropriate but undocumented

**Severity**: LOW
**Location**: `crates/engine/src/engine.rs:2153-2161`

The function performs a linear scan over the markings `HashMap` keys. The doc comment
explains this is appropriate for the defect path. However, the architect pre-flight
§5 described a slightly different predicate shape (`span_is_sub_token` checking
`attrs.token_spans`, not the marking's outer span). The implementation uses the
marking's outer span (the `HashMap` key span) as the containment boundary instead.
This is COARSER than token-level containment — a fix that is within the marking's
outer span but outside any individual token span would pass the filter.

For the current Localized ruleset (C001, E006, E007, S004), all rules emit fixes
within individual token boundaries, so this is not a correctness defect today. But
the architect's intent was per-token shape enforcement. Recommend adding a comment
noting the difference and tracking per-token strictness as a follow-up.

---

## Section 7: `MessageArgs.contributing_rule_ids`

### Item 7.1 — Field addition verified: ALL requirements met

- `pub contributing_rule_ids: SmallVec<[RuleId; 4]>` present at `message.rs:436`
- Sibling to `feature_ids` at line 416 — correct placement
- Inline-4 matches the 4-rule pass-1 set — correct
- `Default` derive handles it correctly (SmallVec implements Default) — verified
- Destructure-pin test updated in `message_args_closed_set.rs:46` — verified
- Default-state assertion `assert!(contributing_rule_ids.is_empty())` at line 60 — present
- Round-trip test at lines 63-94 exercises populated state — present
- `message_args_default_is_all_none` test at `message.rs:647-658` checks `is_empty()` — present

**D-7.17 mandate is cleared.**

### MEDIUM-4 — Audit emitter skip-when-empty not yet enforced at wire boundary

**Severity**: MEDIUM
**Location**: `crates/rules/src/message.rs:432-436`

The doc comment on `contributing_rule_ids` says: "Audit emitters MUST skip this field
when empty." The field is not serde-annotated (because `MessageArgs` is not a serde
type yet — it is a pre-3c.2 in-progress type). When PR 3c.2 wires `Diagnostic.message`
to `Message`, the audit emitter must add `#[serde(skip_serializing_if = "SmallVec::is_empty")]`
or an equivalent skip predicate. This is documented in the comment but there is no
compile-time enforcement — the emitter author could miss it.

The risk is LOW today (the field is not in any serialized path). The issue is that
the obligation is documented only in a comment, not in a test. Recommend filing a
tracking issue for the PR 3c.2 migration to add a test that asserts empty
`contributing_rule_ids` produces no JSON key.

---

## Section 8: `merge_exit_code` and Exit Codes

### Item 8.1 — VERIFIED CORRECT, all 6 combos tested

`merge_exit_code` at `main.rs:63-70` uses explicit `match (current, new_code)` with
R002 arm first — matching PM D-7.15 precedence. The doc comment at lines 42-62
explains the non-max-operator rationale. Unit tests at lines 1034-1116 cover all
required combinations including commutativity and associativity proofs.
`EX_R002_PARTIAL = 3` at line 35 adjacent to other exit code constants.

Per-document branch at `main.rs:942-950` tests `r002_fired` BEFORE `has_errors` /
`has_warns` — correct per D-7.15 and architect pre-flight §7.

**CHK015 cleared**: No rule has both `Phase::Localized` and `Phase::WholeMarking`
(per 7a phase declarations). No `Phase::Both` escape hatch exists.
**CHK018 cleared**: R002 span shape, audit-record shape, exit code, and WASM
detection are all wired and verified.

---

## Section 9: `FixResult` and WASM

### Item 9.1 — VERIFIED CORRECT

`FixResult.r002_fired: bool` at `output.rs:175` with accurate doc comment.
`FixResultJson.r002_fired: bool` at `wasm/src/lib.rs:531` with doc comment.
`r002_fired` initialized `false` at the two normal `FixResult` construction sites
(lines 1787 and 2134 for the R002 path). No `FixResult {` literal omits the field.

`LintResult` gained `#[derive(Clone)]` at `output.rs:44`. This is needed because
`EngineError::DeadlineExceeded { partial_lint: LintResult }` is constructed at
several sites in the new pass structure where `lint` is borrowed rather than owned.
The clone is on the error path only (not the hot path), so this is acceptable.
Worth documenting in `output.rs` why `Clone` was added.

---

## Section 10: Tests

### Item 10.1 — Test coverage assessment

The test set covers the required behavior contracts:
- `c001_pass1_dispatch_noop_after_pass0` (corrections_map.rs:54) — L-1 forward obligation cleared
- `r002_does_not_mint_applied_fix` (audit_completeness.rs:204) — Constitution V pin
- `r002_fired_per_row_inspectable` (batch_r002.rs:38) — batch consumer surface
- `fix_clean_input_exits_zero`, `fix_with_error_exits_one`, `check_error_exits_one` (cli_exit_codes.rs) — exit code integration
- `merge_exit_code` unit tests (main.rs:1034-1116) — all 6 combos + commutativity + associativity

**Gap**: The architect pre-flight §8 item 3 requires a test that exercises the R002
trigger via a "pathological correction" (a pass-1 fix that produces an unparseable
buffer). Currently no such test exists because no production Localized rule emits a
`FixIntent`-shape fix. The `batch_r002.rs` test correctly documents this absence and
calls it structural. However, there is no synthetic test that constructs an
`EngineError`-adjacent fixture to exercise the R002 branch directly (the branch
is dead code today). This is documented as acceptable given the structural absence —
when the first Localized FixIntent rule lands, a concrete fixture becomes possible.

**No HIGH issue here** — the absence is documented and the structural reason is
correct. Recommend filing the follow-up issue at that point.

### Item 10.2 — Corrections map deletion is legitimate

The old `crates/capco/tests/corrections_map.rs` (557 lines, 16 tests) was fully
gated off with `#![cfg(any())]` since PR 3c.B Commit 10. The new file (90 lines,
2 tests) replaces it with live tests exercising the 7b-specific L-1 obligation.
No live test coverage was dropped.

---

## Section 11: Miscellaneous

### Item 11.1 — `R002_RULE_ID` type and migration comment: VERIFIED

`pub const R002_RULE_ID: RuleId = RuleId::new("R002")` at `engine.rs:78`.
Type is `RuleId`, not `&str`. `RuleId::new` is `const fn` (confirmed at
`crates/rules/src/lib.rs:101`). Migration doc comment to 2-tuple form is present
at lines 73-77. `DECODER_RULE_ID` migration noted as out-of-scope. SPDX headers
present on all new files.

### LOW-3 — `localized_ids` set uses `&'static str` keys, not `RuleId`

**Severity**: LOW
**Location**: `crates/engine/src/engine.rs:2046-2056`

`localized_rule_id_set()` returns `HashSet<&'static str>`. Lookup at line 1672 uses
`d.rule.as_str()`. This is correct today because `RuleId::as_str()` returns the
underlying `&'static str`. However it bypasses `RuleId`'s newtype abstraction —
if `RuleId` ever gains non-`&'static str` variants (e.g., the 2-tuple form), this
lookup would silently break. A `HashSet<RuleId>` with proper `Hash`/`Eq` on
`RuleId` would be more type-safe.

---

## CHK015 / CHK018 Gate Attestation

- **CHK015 CLEARED**: No rule declares both `Phase::Localized` and `Phase::WholeMarking`.
  No `Phase::Both` variant exists. The four Localized rules (C001, E006, E007, S004)
  each register once.
- **CHK018 CLEARED**: All four sub-requirements met:
  1. `MessageTemplate::ReparseFailed` variant exists (reserved slot, PR 7b fills the
     field struct).
  2. R002 span shape: `Span::new(0, post_pass1_buf.len())` sentinel — documented.
  3. Audit-record shape: `MessageArgs.contributing_rule_ids: SmallVec<[RuleId; 4]>` present.
  4. Exit code `EX_R002_PARTIAL = 3`; WASM `r002_fired: bool`; batch worst-row-wins
     in CLI loop.

---

## 5-Year-Maintenance Posture

The structural shape — `TwoPassFixer<'engine>` as a stack-bound, single-lifetime
orchestrator with explicit pass-result types (`Pass0Result`, `Pass1Result`,
`Pass2Result`) — is genuinely reviewer-traceable and independently testable. The
phase-tagged pipeline model (`Phase::Localized` before `Phase::WholeMarking`) is
clean and extends naturally when a third phase is needed.

The two HIGH issues (unnecessary clones at the short-circuit boundary, dead
`_message_args` binding) are maintenance debt that will grow. The clone issue
silently reintroduces an allocation the design explicitly rejected; a future
"performance profile" pass will rediscover these clones without the context of why
they were removed. The `_message_args` issue creates a PR 3c.2 migration trap: the
contributing IDs are constructed but not wired, and the obligation is only in a
comment. Both should be addressed before merge if possible, or explicitly tracked.

The `sort_and_c1_dedup` double-allocation (MEDIUM-2) will appear in any future
allocation profile of the fix path; the intermediate `Vec<&SynthesizedFix>` is
unnecessary. The `contributing_pass1_rule_ids` allocation pattern (MEDIUM-3) is on
the rare R002 path and is low priority.

The core R002 correctness invariants — no `__engine_promote`, no `AppliedFix` for
R002, `r002_fired: bool` detectable without NDJSON parsing, exit-code precedence
chain, audit-content-ignorance — are all correctly implemented and pinned by tests.
The `merge_exit_code` implementation is particularly clean.

On the 5-year horizon: when the (scheme, predicate-id) 2-tuple `RuleId` form lands
(FR-049 unfreeze, post-PR-10), the `localized_ids: HashSet<&'static str>` lookup
(LOW-3) will silently malfunction. The migration path exists in the doc comment at
line 73-77 but there is no compile-time enforcement. Recommend an `E0` clippy lint
or a TODO-search CI gate to surface this migration obligation before it bites.
