<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7b — Rust Specialist Pre-flight

**Scope**: T076–T079 (TwoPassFixer, forward-pass splice, R002, exit code, D1 surface).
**Branch**: `refactor-006-pr-7b-two-pass-r002`
**Date**: 2026-05-13

---

## TL;DR

Seven non-obvious traps for 7b:

1. `TwoPassFixer<'engine>` needs only the `'engine` lifetime — do not add
   more; the struct is born and dies inside a single `spawn_blocking`.
2. The forward-pass buffer algorithm already exists at `engine.rs:1544–1574`;
   copy it exactly for pass-1 (`apply_pass1_fixes`).
3. Short-circuit re-parse when `pass1_applied.is_empty()` by moving
   `parsed_markings` directly — no `Cow`, no clone needed.
4. R002 is a `Diagnostic`, not an `AppliedFix`. Must not touch
   `__engine_promote`. Follow `build_decoder_diagnostic`'s shape.
5. `MessageArgs.contributing_rule_ids` field addition requires updating the
   destructuring-pin test at `crates/rules/tests/message_args_closed_set.rs`
   or the build breaks with `E0027`.
6. `EX_DIAG_ERROR = 1` and `EX_R002_PARTIAL = 3` are NOT numerically ordered
   by severity — the batch reducer must use an explicit precedence chain,
   not `max()`.
7. C001 dual-path idempotency is safe by construction; add one test to lock it.

---

## Q1 — `TwoPassFixer<'engine>` struct definition

`TwoPassFixer` is constructed and fully consumed inside `Engine::fix_inner`,
which is called from `spawn_blocking`. It needs only the `'engine` borrow
of the `Engine`. The rule-set partition indices (`pass1_rule_indices`,
`pass2_rule_indices`) already live on `Engine`; mutable state accumulates in
local variables inside each method.

```rust
struct TwoPassFixer<'engine> {
    engine:    &'engine Engine,
    source:    &'engine [u8],
    mode:      FixMode,
    threshold: f32,
    deadline:  Option<Instant>,
}
```

No additional lifetime parameters. No `Drop` impl needed — the struct borrows
everything. `TwoPassFixer` itself does not need to be `Send`; the `Arc<Engine>`
it dereferences satisfies the closure's `Send` requirement.

---

## Q2 — Forward-pass buffer construction

The pattern already exists at `engine.rs:1544–1574`. Copy it:

```rust
fn apply_pass1_fixes(source: &[u8], fixes: &[SynthesizedFix]) -> Vec<u8> {
    // fixes are in (span.end DESC, span.start DESC) order from FR-016 sort;
    // .rev() gives ascending order for the left-to-right walk.
    let extra: usize = fixes
        .iter()
        .map(|f| f.replacement.len().saturating_sub(f.span.end - f.span.start))
        .sum();
    let mut buf = Vec::with_capacity(source.len() + extra);
    let mut cursor = 0usize;
    for fix in fixes.iter().rev() {
        debug_assert!(
            fix.span.start >= cursor,
            "overlapping pass-1 fix: cursor={cursor}, span={:?}", fix.span
        );
        buf.extend_from_slice(&source[cursor..fix.span.start]);
        buf.extend_from_slice(fix.replacement.as_bytes());
        cursor = fix.span.end;
    }
    buf.extend_from_slice(&source[cursor..]);
    buf
}
```

The `debug_assert!` catches overlap violations (impossible under C-1, but
cheap to verify in CI). In release, a violation would silently corrupt output,
so if you add a release guard use `tracing::error!` + return `source.to_vec()`
rather than a silent continue.

Do NOT use `Vec::splice` (used in `apply_text_corrections`). Splice is
O(N × M); the forward-pass pattern is O(N + M).

---

## Q3 — Short-circuit re-parse

`CanonicalAttrs` is owned (no `<'src>` lifetime — confirmed
`crates/ism/src/canonical.rs:64`). After the C001 re-lint, `parsed_markings`
is owned by `fix_inner`'s stack frame. Move it in both branches:

```rust
let pass2_markings: Vec<(Span, CanonicalAttrs)> = if pass1_applied.is_empty() {
    // Skip re-parse; coordinate space is unchanged.
    parsed_markings   // move — original binding consumed
} else {
    let (_, new_markings) =
        self.lint_with_options_internal(&post_pass1_buf, &lint_opts);
    new_markings
    // On parse failure: emit R002, return early (see Q4).
};
```

No `Cow`. No clone. Both branches produce the same owned type.

---

## Q4 — R002 emission

R002 is a `Diagnostic`, not an `AppliedFix`. It does not touch
`__engine_promote`. Parallel to `build_decoder_diagnostic`:

```rust
fn build_r002_diagnostic(
    contributing_rule_ids: SmallVec<[RuleId; 4]>,
    failure_span: Span,
) -> Diagnostic<CapcoScheme> {
    use marque_rules::message::{Message, MessageArgs, MessageTemplate};
    let args = MessageArgs {
        contributing_rule_ids,
        ..MessageArgs::default()
    };
    Diagnostic {
        rule: R002_RULE_ID.clone(),
        severity: Severity::Error,
        span: failure_span,
        candidate_span: None,
        message: Message::new(MessageTemplate::ReparseFailed, args),
        citation: "engine-synthetic",
        fix: None,
        text_correction: None,
    }
}
```

For `failure_span` when the parser cannot localize the failure site:
use `Span::new(0, post_pass1_buf.len())`. Document this sentinel in the
function's doc comment.

`R002_RULE_ID` must land as `RuleId`, not `&'static str`, per D-7.4:

```rust
// Adjacent to DECODER_RULE_ID at engine.rs:51.
// NOTE: DECODER_RULE_ID is &str for historical reasons; R002 corrects this.
// When the (scheme, predicate-id) 2-tuple RuleId form lands (post-PR-10),
// this becomes RuleId::new("engine", "r002.reparse-failed") per FR-044.
pub const R002_RULE_ID: RuleId = RuleId::new("R002");
```

---

## Q5 — `MessageArgs.contributing_rule_ids` field and SmallVec calibration

Add to `MessageArgs` in `crates/rules/src/message.rs`:

```rust
/// Contributing pass-1 fix rule IDs for [`MessageTemplate::ReparseFailed`].
/// Empty for all other templates. Inline-4 matches the 4-rule pass-1 set
/// (C001, E006, E007, S004) — no heap allocation even when all fire.
pub contributing_rule_ids: SmallVec<[RuleId; 4]>,
```

**Mandatory co-update** (`E0027` if missed):
`crates/rules/tests/message_args_closed_set.rs` uses an exhaustive
destructure pattern over `MessageArgs`. Adding the field without updating
the pattern breaks the build — this is the intended safety net. Add
`contributing_rule_ids` to the destructure and assert it equals `SmallVec::new()`
in the default case.

`MessageArgs::default()` — confirm the existing `derive(Default)` handles
`SmallVec` fields (check the `feature_ids` field precedent). If a manual
impl is needed, `SmallVec::new()` is the correct empty default.

**Inline-4 vs inline-2**: pass-1 has exactly 4 rules. Inline-4 costs 64 bytes
(4 × 16-byte `RuleId`) — one cache line. Never heap-allocates for the known
rule set. Use inline-4.

**`RuleId` size**: `RuleId(&'static str)` = 16 bytes on 64-bit
(`ptr + len` fat pointer). `SmallVec<[RuleId; 4]>` inline storage = 64 bytes.
Fits in one cache line.

---

## Q6 — C001 dual-path idempotency

`CorrectionsMapRule::check` (rules.rs:1217) iterates `attrs.token_spans`
and guards on `replacement == text` at line 1235. After pass-0 applies
`SERCET → SECRET`, the re-lint produces token spans with `.text = "SECRET"`.
`corrections.get("SECRET")` returns `None` (no entry for the correct form)
or hits the `replacement == text` guard. Pass-1 dispatch of C001 produces
zero diagnostics by construction.

No code change needed. The test that locks this behavior:

```rust
// crates/capco/tests/corrections_map.rs
#[test]
fn c001_pass1_dispatch_noop_after_pass0() {
    let source = b"(TS//SERCET//NF)";
    let engine = make_engine_with_correction("SERCET", "SECRET");
    let result = engine.fix(source, FixMode::Apply);
    let c001_count = result.applied.iter()
        .filter(|f| f.rule.as_str() == "C001").count();
    assert_eq!(c001_count, 1, "C001 fires exactly once (pass-0 only)");
}
```

Name this test explicitly in the PR description to clear the L-1 forward
obligation from `pr-7a-rust-review.md`.

---

## Q7 — Exit code placement and reduction

Add `EX_R002_PARTIAL = 3` adjacent to the existing constants at
`marque/src/main.rs:26-33`.

**The numeric-max trap**: `EX_DIAG_ERROR = 1` and `EX_R002_PARTIAL = 3`.
`max(1, 3) = 3`, so a batch row producing `EX_DIAG_ERROR` would be silently
overridden by a row producing `EX_R002_PARTIAL`. This is wrong — errors
are at least as severe as partial-progress.

Use an explicit precedence chain:

```rust
/// Precedence: EX_DIAG_ERROR > EX_R002_PARTIAL > EX_DIAG_WARN > EX_OK.
/// Numeric max is NOT the right operator (1 < 3 but error > partial).
fn merge_exit_code(current: i32, new_code: i32) -> i32 {
    match (current, new_code) {
        (EX_DIAG_ERROR, _) | (_, EX_DIAG_ERROR) => EX_DIAG_ERROR,
        (EX_R002_PARTIAL, _) | (_, EX_R002_PARTIAL) => EX_R002_PARTIAL,
        (EX_DIAG_WARN, _) | (_, EX_DIAG_WARN) => EX_DIAG_WARN,
        _ => EX_OK,
    }
}
```

Apply in the per-document reduction in `run_fix`. R002 detection uses
`FixResult::r002_fired` (Q8) or scans `remaining_diagnostics` for
`rule.as_str() == "R002"`.

---

## Q8 — BatchEngine worst-row-wins

`BatchEngine::fix_many_inner` returns a stream; the CLI's `run_fix` consumes
it per-row. No `BatchEngine::finalize_exit_code` exists yet — the reduction
is in the CLI loop. Add R002 detection to that loop using `merge_exit_code`
from Q7. The per-row exit code accumulator is local to the CLI task and
never shared across threads; no `Mutex` needed.

---

## Q9 — WASM `r002_fired` field

Add to `FixResult` at `crates/engine/src/output.rs`:

```rust
pub struct FixResult {
    pub source: Vec<u8>,
    pub applied: Vec<AppliedFix<CapcoScheme>>,
    pub remaining_diagnostics: Vec<Diagnostic<CapcoScheme>>,
    /// true when pass-1 re-parse failed (R002 emitted); source is
    /// the pass-1 buffer only — pass-2 did not run.
    pub r002_fired: bool,
}
```

Add to `FixResultJson` at `crates/wasm/src/lib.rs:523`:

```rust
#[derive(Debug, Serialize)]
struct FixResultJson {
    fixed_text: String,
    applied: Vec<Box<serde_json::value::RawValue>>,
    remaining: Vec<Box<serde_json::value::RawValue>>,
    r002_fired: bool,
}
```

WASM callers check `result.r002_fired` without parsing the diagnostic stream.
This satisfies D1's "detectable without NDJSON parsing" binding requirement.

---

## Q10 — Phase-span-shape check

```rust
#[inline]
fn span_is_within_marking(inner: Span, outer: Span) -> bool {
    inner.start >= outer.start && inner.end <= outer.end
}

// In pass-1 dispatch loop:
if !span_is_within_marking(fix.span, ctx.candidate_span) {
    tracing::error!(
        rule_id = %rule_id,
        "Phase::Localized rule emitted non-sub-token span; dropping fix"
    );
    debug_assert!(false, "Localized rule '{}' emitted span {:?} outside {:?}",
        rule_id, fix.span, ctx.candidate_span);
    continue;
}
```

`tracing` is already imported in `engine.rs`. `<=` on endpoint comparison is
correct — a fix exactly matching a token's boundaries is still sub-token.

---

## Q11 — Forward-compatibility for 7c

Do NOT pre-thread `RuleContext<'a>` in 7b. The 7c opening commit adds
the lifetime parameter and the `pre_pass_1_attrs` field, updating all 31
`impl Rule` blocks via a mechanical `&RuleContext` → `&RuleContext<'_>`
find-replace (rule bodies are unchanged because `'_` is elided at call
sites). Pre-threading `'a` in 7b forces the same 31-block change with no
functional benefit.

---

## Summary table

| Item | File | Shape |
|------|------|-------|
| `TwoPassFixer<'engine>` | `engine.rs` | 5-field struct, `'engine` only (Q1) |
| `apply_pass1_fixes` | `engine.rs` | Copy `engine.rs:1544–1574` pattern (Q2) |
| Short-circuit re-parse | `TwoPassFixer::run` | Move `parsed_markings`; branch on `is_empty()` (Q3) |
| `R002_RULE_ID: RuleId` | `engine.rs:51` | `pub const`, type `RuleId` not `&str` (Q4) |
| `build_r002_diagnostic` | `engine.rs` | Mirrors `build_decoder_diagnostic` (Q4) |
| `MessageArgs.contributing_rule_ids` | `message.rs` + `message_args_closed_set.rs` | `SmallVec<[RuleId; 4]>` + test update (Q5) |
| `merge_exit_code` | `main.rs` | Explicit precedence, not `max()` (Q7) |
| `FixResult.r002_fired` | `output.rs` | `pub r002_fired: bool` (Q9) |
| `FixResultJson.r002_fired` | `wasm/src/lib.rs` | Serialized field (Q9) |
| `span_is_within_marking` | `engine.rs` | Inline predicate for phase-span check (Q10) |
| C001 idempotency test | `capco/tests/corrections_map.rs` | New test, cited in PR (Q6) |
