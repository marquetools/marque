<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7b Code Review — Two-Pass Fix Dispatch + R002

**Reviewer**: code-quality panel (independent quality review)
**Branch**: `refactor-006-pr-7b-two-pass-r002`
**Date**: 2026-05-13
**Files reviewed**: 15 changed, +2426 / -853

---

## Top-line Verdict

**APPROVE WITH WARNINGS**

| Severity | Count |
|----------|-------|
| CRITICAL | 0     |
| HIGH     | 3     |
| MEDIUM   | 3     |
| LOW      | 4     |

Three HIGH issues require resolution before merge. None are blockers that
invalidate the architecture, but two are stale `#[allow(dead_code)]`
annotations that leave misleading compiler-suppression in place, and one is a
`build_r002_diagnostic` function whose `_message_args` binding constructs a
`MessageArgs` value and then drops it without wiring it into the returned
`Diagnostic`.

---

## Findings

### HIGH

---

**[HIGH-1] `#[allow(dead_code)]` on `pass1_rule_indices` not removed when the field became live**

File: `crates/engine/src/engine.rs:216–217`

The PR-7a comment block at lines 207–215 states "Removing the allow when 7b
consumes these fields is part of that PR's checklist." PR 7b does consume
`pass1_rule_indices` — `TwoPassFixer::localized_rule_id_set` reads it at
line 2048. The `#[allow(dead_code)]` was not removed. This is a stale
suppression that now hides nothing but misleads the next reader into thinking
the field is still dead.

`pass2_rule_indices` is another matter: it is stored and not consumed in 7b
(pass-2 dispatch uses `pass2_diags` from the diagnostic partition, not the
index partition). The `#[allow(dead_code)]` on `pass2_rule_indices` is correct
for 7b's scope and needs a doc comment update to remove the 7a-framing ("part
of that PR's checklist" is stale since only 7b, not 7c, was responsible for
`pass1`).

Fix: Remove `#[allow(dead_code)]` on `pass1_rule_indices` (lines 216–217).
Update the comment on `pass2_rule_indices` to note it is consumed in 7c when
pass-2 also dispatches on the index partition, not "in 7b."

---

**[HIGH-2] `build_r002_diagnostic` constructs `MessageArgs.contributing_rule_ids` into `_message_args` and discards it**

File: `crates/engine/src/engine.rs:2699–2724`

```rust
let _message_args = marque_rules::MessageArgs {
    contributing_rule_ids,
    ..marque_rules::MessageArgs::default()
};

let message = if rule_list.is_empty() {
    "post-pass-1 buffer failed to re-parse; pass-2 skipped".to_string()
} else {
    format!("post-pass-1 buffer failed to re-parse after applying \
             pass-1 fixes from {rule_list}; pass-2 skipped")
};

Diagnostic::new(
    R002_RULE_ID,
    Severity::Error,
    failure_span,
    message,          // Box<str>, NOT Message
    R002_CITATION,
    None,
)
```

The `_message_args` binding is constructed and immediately dropped. Its value
never flows into the returned `Diagnostic` — the diagnostic carries a plain
`Box<str>` message, not the `MessageArgs` struct that the design (D-7.5, FR-024)
says must carry `contributing_rule_ids` for the audit boundary.

The design rationale is explicitly documented on lines 2698–2701: "PR 3c.2
will migrate `Diagnostic.message: Box<str>` to `Message` and wire the field
through the audit emitter; PR 7b ships the typed field so the migration is
purely additive." That rationale is sound — PR 3c.2 is the migration point.
However, the code as written silently drops the `contributing_rule_ids`
from the diagnostic payload entirely. A consumer who reads
`remaining_diagnostics` for the R002 entry and wants the contributing rule IDs
gets nothing today.

This is the central payload of R002 (D-7.5: "FR-024 names the contributing IDs
as part of the R002 payload") being silently voided at runtime. The comment
documents the intent but the code violates it.

Fix options:
1. Keep `_message_args` but rename to `message_args`, add a
   `// PR 3c.2: wire into Diagnostic.message when the field type migrates`
   comment, and ensure it is held (not dropped) somewhere on the struct until
   the migration point. But `Diagnostic` currently has no `message_args` field,
   so this cannot be stored without a struct change.
2. Simpler and honest: drop `_message_args` entirely and document in the
   function's doc comment that the structured payload is not yet wired because
   the `Diagnostic.message` migration (PR 3c.2) has not landed. The
   `contributing_rule_ids` are still embedded in the human-readable message
   string `rule_list` for diagnostic rendering. Add a `// TODO(PR-3c.2)` with a
   reference to the issue or PR so the migration does not get lost.

The current state is misleading: it looks like the field is wired when it is
not.

---

**[HIGH-3] Stale PR-7a doc comment still present on `pass1_rule_indices` / `pass2_rule_indices` fields after 7b consumed them**

File: `crates/engine/src/engine.rs:207–220`

The doc comment reads "PR 7a behavior. Stored but unused — both phases still
run together in pass-2 exactly as before. The partition is READ but UNUSED."
`pass1_rule_indices` IS now used (HIGH-1 above). The doc comment is factually
incorrect as of 7b's merge. The 7a framing should be updated to reflect 7b's
actual dispatch behavior.

Fix: Update the `///` doc comment to describe the current state ("pass-1
indices consumed by `TwoPassFixer::localized_rule_id_set`; pass-2 indices
consumed in PR 7c").

---

### MEDIUM

---

**[MEDIUM-1] Unnecessary `.clone()` in the non-R002 happy path at line 1726**

File: `crates/engine/src/engine.rs:1726`

```rust
(pass1.post_buffer.clone(), new_markings)
```

This clone is unnecessary. `pass1` is an owned `Pass1Result` that is
not used again after this arm — `pass1.applied_keys` was already consumed
into the `applied_keys` set, and after the R002 early-return check at line
1724, `pass1.post_buffer` is the only remaining field needed. The code
could be written as:

```rust
let buffer = pass1.post_buffer;
(buffer, new_markings)
```

The rust pre-flight (Q3) explicitly named this pattern: "Moving it in both
branches keeps both arms producing the same owned type — no `Cow`, no clone."
The short-circuit branch (line 1697) correctly moves `pass0.effective_source`
but the re-parse branch clones `pass1.post_buffer` instead of moving it.
At current document sizes the cost is negligible, but the clone is non-obvious
given the comment at line 1691–1695 explaining the no-clone design choice.

---

**[MEDIUM-2] `remaining_diagnostics` filter logic duplicated between `run()` and `assemble_r002_result()`**

Files: `crates/engine/src/engine.rs:1760–1774` and `2102–2116`

The predicate `fix_applied = if d.fix.is_some() { ... } else if
d.text_correction.is_some() { ... } else { false }` appears verbatim in both
`TwoPassFixer::run` (the success path) and `TwoPassFixer::assemble_r002_result`
(the R002 path). Duplication of non-trivial predicate logic is a maintenance
risk: a future change to the `candidate_span` fallback semantics must land in
both places.

Extract to a `fn is_fix_applied(d: &Diagnostic<CapcoScheme>, applied_keys:
&HashSet<(RuleId, Span)>) -> bool` free function or a method on `TwoPassFixer`
so both paths share one implementation.

---

**[MEDIUM-3] `sort_and_c1_dedup` allocates a `Vec<&SynthesizedFix>` reference-vec before cloning**

File: `crates/engine/src/engine.rs:2167–2190`

```rust
let mut fixes: Vec<&SynthesizedFix> = synthesized.iter().collect();
fixes.sort_by(|a, b| { ... });
let mut kept_fixes: Vec<SynthesizedFix> = Vec::with_capacity(fixes.len());
...
    kept_fixes.push((*fix).clone());
```

The function takes ownership of `synthesized: Vec<SynthesizedFix>`, builds a
reference-vec of `&SynthesizedFix`, sorts the references, then clones each
kept entry into `kept_fixes`. This means every kept `SynthesizedFix` is cloned.
For pass-1 the set is small (at most a few fixes per marking), but as a
general pattern it is inefficient and the double-indirection is confusing.

Sort `synthesized` in place (it is owned), walk it, and move kept entries into
the result vec:

```rust
synthesized.sort_by(|a, b| { ... });
let mut kept: Vec<SynthesizedFix> = Vec::with_capacity(synthesized.len());
let mut next_window_end: Option<usize> = None;
for fix in synthesized {                // ownership transfer, no clone
    let fits = next_window_end.is_none_or(|b| fix.span.end <= b);
    if fits {
        next_window_end = Some(fix.span.start);
        kept.push(fix);
    }
}
kept
```

This eliminates the intermediate reference-vec and all the per-entry clones.
At present input sizes it is not a correctness issue, but the clone-heavy path
is a latent performance regression as document density increases.

---

### LOW

---

**[LOW-1] `contributing_pass1_rule_ids` builds a `HashSet` + `Vec` + sorts + takes from `inline_size()` — three-pass dedup when one would suffice**

File: `crates/engine/src/engine.rs:2064–2081`

The function is correct but allocates two collections to deduplicate-then-sort
IDs that are already constrained to at most 4 entries (the inline capacity of
the result). The `HashSet` allocation could be replaced with a membership
check on the accumulator vec directly (O(N^2) over ≤4 elements is fine) and
the sort-then-take pattern eliminates the `ids.into_iter().take(out.inline_size())`
call that reads the inline size of an empty SmallVec. This is a micro-concern
for a function on a failure path, but the current code looks more complex than
the constraints justify.

---

**[LOW-2] `apply_pass1_fixes` function name is misleading for its actual scope**

File: `crates/engine/src/engine.rs:2203`

The function is called by both `run_pass1_localized` (line 1990) and
`run_pass2_whole_marking` (line 1990 — through `apply_kept_fixes`). The name
"apply_pass1_fixes" implies it is pass-1-specific. It is the shared
forward-buffer construction helper for both passes. Renaming to
`apply_sorted_fixes_forward` or `build_fixed_buffer` would make the
shared-helper intent clear and prevent a future reader from thinking it must
not be called from pass-2.

---

**[LOW-3] `r002_fired` doc comment on `FixResult` is consumer-oriented in two places but not in a third**

File: `crates/engine/src/output.rs:155–175`

The field-level doc comment is thorough and consumer-facing. The WASM wrapper
at `crates/wasm/src/lib.rs:527–531` has a shorter but adequate comment.
The `build_r002_diagnostic` function doc (`crates/engine/src/engine.rs:2648–
2683`) mentions `r002_fired` in passing but does not link to the `FixResult`
contract section where the consumer obligations are specified. A cross-reference
`/// Sets [`FixResult::r002_fired`] to `true` on the returned result` in
`build_r002_diagnostic`'s doc would help the reader trace from the emitter to
the surface.

---

**[LOW-4] `run_fix` in `marque/src/main.rs` does not cover `EX_DIAG_WARN = 2` integration path in `cli_exit_codes.rs`**

File: `marque/tests/cli_exit_codes.rs:63–83`

The test `fix_with_warning_only_exits_two` bails early if `banner_with_info_only.txt`
does not exist (line 73), leaving the `EX_DIAG_WARN` integration path untested.
The unit tests in `exit_code_tests` (in `main.rs`) cover the reduction
algebraically, but there is no integration test that actually exercises the
`EX_DIAG_WARN = 2` path through the binary. The D-7.15 spec requires 6
exit-code cases; 5 are present in unit form, 1 integration case (warn-only →
2) has a documented absent fixture. This is low severity because the unit test
fully covers the reduction, but the integration gap should be tracked.

---

## Design Decision Compliance Audit

### R002 trigger heuristic (deferred issue)

The R002 trigger at `engine.rs:1718–1720` uses "pre-pass-1 had markings AND
post-pass-1 has zero markings" as the re-parse failure signal. As documented in
the code, this is a sound proxy for the current recognizer design (total, not
returning `Err`). However, there is an unresolved false-negative: a document
where pass-1 corrupts one marking but leaves others intact will not trigger R002
because `new_markings.is_empty()` is `false`. The guard is described as
"conservative" (no false positives on partial cleanups), but the false-negative
class is not documented in the code. This is a known limitation that should be
captured — in the doc comment at the trigger site or in a follow-up issue —
so a future Localized rule author does not assume R002 catches all splice
corruption.

### `corrections_map.rs` deletion audit — SAFE

The pre-state file began with `#![cfg(any())]` — a gating attribute that
compiles the entire module dead (`cfg(any())` is always false). The 579 lines
were therefore completely excluded from the test binary at origin/staging.
Zero live coverage was dropped. The replacement file provides two live,
well-named tests (`c001_pass1_dispatch_noop_after_pass0`,
`c001_self_correction_filtered_at_pass0`) that are stronger than the legacy
content because they exercise the M2 no-op guard through the engine boundary
rather than through a direct rule-pipeline shim. The deletion is legitimate.

### CHK015 gate clearance

No `Phase::Both` escape hatch exists. The `Phase` enum has exactly two
variants (`Localized`, `WholeMarking`); no rule registers twice. Gate passes.

### CHK018 gate (partial)

1. `MessageTemplate::ReparseFailed` exists and is wired in the template enum.
   However, `build_r002_diagnostic` does not construct a `Message` value —
   it uses a plain `format!` string. The `_message_args` dropping issue
   (HIGH-2) means item 1 is declared but not functionally active.
2. R002 span shape: `Span::new(0, post_pass_1_buffer.len())` — a document-wide
   sentinel. Documented in `build_r002_diagnostic`'s doc comment. Adequate
   for the PR-7b scope.
3. Audit-record shape: `MessageArgs.contributing_rule_ids` is added correctly
   at the type level. It is not wired into the returned `Diagnostic` (HIGH-2).
4. Exit code: `EX_R002_PARTIAL = 3` is correct. WASM detection satisfied.
   BatchEngine per-row inspection satisfied.

Gate conditionally passes on items 2, 4; HIGH-2 must be resolved to clear
item 1 and 3.

---

## 5-Year Maintenance Posture

The architecture is sound. `TwoPassFixer` as a named struct with explicit pass
methods is reviewable and the five-line trampoline in `fix_inner` is the right
shape. The `merge_exit_code` unit-test bank is comprehensive and the
algebraic properties (associativity, commutativity) are verified — a future
R003 signal slots in mechanically by extending one `match` arm. The
`corrections_map.rs` replacement is a clean upgrade.

The two maintenance risks to monitor:

1. **Stale PR-7a prose in doc comments** (HIGH-1, HIGH-3). This PR adds new
   behavior but left scaffolding comments written for PR-7a in place. Five
   years from now those comments will be noise — a reader will not know which
   state the field is actually in. Keeping the doc comments current with the
   code's actual behavior is load-bearing documentation hygiene for this
   codebase.

2. **`_message_args` drop** (HIGH-2). The `contributing_rule_ids` payload is
   the audit-visible payload of R002. If PR 3c.2 lands without being pointed
   at this gap, the migration will leave R002's contributing IDs missing from
   the NDJSON audit stream. A comment is not sufficient here; the migration
   hook must be explicit (a `// TODO(pr-3c.2)` with issue reference or a note
   in `docs/refactor-006/pr-7b-open-items.md`).

The code is otherwise clean: no `TODO`/`FIXME` in the new engine code, no
secrets, no unguarded `unwrap()` in production paths, the `debug_assert!` +
`tracing::error!` pattern for span-shape violations is correct, and the
Constitution V audit-record integrity invariant is verifiably upheld (R002
produces a `Diagnostic`, never an `AppliedFix`).
