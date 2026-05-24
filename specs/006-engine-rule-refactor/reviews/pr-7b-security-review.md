<!--
SPDX-FileCopyrightText: 2026 The marque Project Contributors
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7b Security Review — Two-Pass Fix Dispatch + R002

**Reviewer**: Security Reviewer (automated panel)
**Branch**: `refactor-006-pr-7b-two-pass-r002`
**Date**: 2026-05-13
**Scope**: Audit-record integrity, G13 content-ignorance, `__engine_promote` call-site
discipline, R002 emission, `MessageArgs.contributing_rule_ids`, WASM surface.

---

## Top-Line Verdict

**REQUEST CHANGES (BLOCKING)**

| Severity | Count |
|----------|-------|
| CRITICAL | 1     |
| HIGH     | 1     |
| MEDIUM   | 1     |
| LOW      | 1     |

---

## Finding 1 — CRITICAL: `promote-callsite-lint` CI gate fails; `__engine_promote` calls are in `impl TwoPassFixer`, not `impl Engine`

**File**: `crates/engine/src/engine.rs` lines 1999 and 2021
**CI lint**: `tools/promote-callsite-lint/` (FR-040, Constitution V Principle V)

### Reproduction

Running the lint from the workspace root produces:

```
error: PRC002: __engine_promote/__engine_construct called from non-engine, non-test code
       (FR-040; Constitution V Principle V) at ./crates/engine/src/engine.rs:1999:34
error: PRC002: __engine_promote/__engine_construct called from non-engine, non-test code
       (FR-040; Constitution V Principle V) at ./crates/engine/src/engine.rs:2021:34
```

### Root Cause

PR 7b extracts `TwoPassFixer` as a private helper struct in `engine.rs`. The two
`__engine_promote` calls that previously lived inside `Engine::fix_inner` (an
`impl Engine` method, which is on the lint allow-list at
`tools/promote-callsite-lint/src/callsite.rs:66`) now live inside
`TwoPassFixer::apply_kept_fixes` (an `impl TwoPassFixer` method). The lint's
classification rule at line 330:

```rust
Some("Engine") => ENGINE_METHOD_ALLOW_LIST.contains(&fr.name.as_str()),
Some(_) => false,
```

rejects any `impl` block whose `impl_self_type` is not exactly `"Engine"`.
`TwoPassFixer` is not `Engine`, so both calls are PRC002 failures.

### Impact

The CI gate is broken. The lint exists specifically to enforce that no code
outside `Engine::fix_inner` / `Engine::apply_text_corrections` can mint an
`AppliedFix` audit record (Constitution V Principle V). With the gate broken,
a future contributor can add a new `__engine_promote` call site anywhere in
`impl TwoPassFixer` methods without seeing a CI failure. The current two call
sites are functionally correct — they reach through `TwoPassFixer.engine` to
obtain the promotion token from `engine_promotion_token()`, and the fix ordering
and threshold gate logic is preserved in `apply_kept_fixes`. But the enforcement
mechanism is missing.

### Remediation

Two valid options; either is sufficient:

**Option A (preferred — minimal scope)**: Update the lint allow-list to accept
`TwoPassFixer::apply_kept_fixes` as an authorized production call site. In
`tools/promote-callsite-lint/src/callsite.rs`:

```rust
// New allow-list for impl TwoPassFixer production sites
const TWOPASSFIXER_METHOD_ALLOW_LIST: &[&str] = &["apply_kept_fixes"];

// In classify_and_emit:
Some("TwoPassFixer") => TWOPASSFIXER_METHOD_ALLOW_LIST.contains(&fr.name.as_str()),
```

The doc comment at `engine_promotion_token()` should be updated to name four
production call sites instead of three.

**Option B (higher confidence)**: Move `apply_kept_fixes` back onto `impl Engine`
as a private method so the existing `ENGINE_METHOD_ALLOW_LIST` covers it without
a lint change.

**Constitution principle**: V Principle V — engine-only production call-site
contract.

---

## Finding 2 — HIGH: `r002_does_not_mint_applied_fix` test is vacuous; R002 path never exercised

**File**: `crates/engine/tests/audit_completeness.rs` lines 204–229

### Description

The test explicitly acknowledges: "today no production Localized rule emits a
FixIntent that could trigger R002, so `r002_fired == false` here." The test body
verifies that `result.applied` contains no entry with `rule == "R002"`, but since
R002 never fires in the fixture, the loop iterates zero times and the assertion is
trivially satisfied. The test does not exercise the critical code path it is named
for — the path where pass-1 fires, post-splice re-parse returns zero markings, and
the engine must refrain from calling `__engine_promote` for R002.

The reason this is HIGH rather than CRITICAL: the code at `build_r002_diagnostic`
(lines 2683–2724) correctly does not call `__engine_promote` — this is verifiably
true by code inspection. The test's vacuousness means the protection is
documentation-only, not enforcement-level. A future refactor that introduces
`__engine_promote` into the R002 path would not be caught by this test until
R002 can actually fire.

### Remediation

Add a test fixture that forces the R002 trigger condition: a Localized-rule
FixIntent that, when applied, produces a post-splice buffer with zero recognized
markings. This requires either:

(a) A test-only `Phase::Localized` rule that emits a FixIntent replacing the
    entire marking with an unrecognized byte sequence, registered only in
    `#[cfg(test)]` via a test-only `CapcoRuleSet` constructor, or

(b) Direct unit testing of `build_r002_diagnostic` to assert it returns a
    `Diagnostic` (not an `AppliedFix`) and its `rule` field equals `R002_RULE_ID`.

Option (b) is lower effort and directly pins the no-promotion contract:

```rust
#[test]
fn build_r002_does_not_return_applied_fix_shape() {
    // b: direct unit test — verifies the return type is Diagnostic, not AppliedFix
    let ids: SmallVec<[RuleId; 4]> = smallvec![RuleId::new("C001")];
    let diag = build_r002_diagnostic(ids, Span::new(0, 42));
    assert_eq!(diag.rule.as_str(), "R002");
    assert!(diag.fix.is_none(), "R002 must carry no FixIntent");
    assert!(diag.text_correction.is_none(), "R002 must carry no TextCorrection");
}
```

**Constitution principle**: V Principle V — "Every applied fix MUST produce a
complete audit record." The complement is equally binding: every non-fix
synthetic diagnostic MUST NOT produce an audit record.

---

## Finding 3 — MEDIUM: G13 corpus-level gate does not cover `FixResult.remaining_diagnostics`

**File**: `crates/engine/tests/audit.rs`

### Description

Constitution V Principle V requires: "Corpus-level integration tests MUST verify
no document text appears verbatim in engine output streams." The existing corpus
sweep at `no_document_text_leaks_into_diagnostic_messages` (line 327) calls
`engine.lint()` and checks `result.diagnostics`. It does NOT call `engine.fix()`
and check `result.remaining_diagnostics`. When R002 fires, the R002 diagnostic
lands exclusively in `FixResult.remaining_diagnostics`, not in `LintResult.diagnostics`.

The existing sentinel check in `no_document_text_leaks_into_diagnostic_messages`
therefore provides no coverage for the R002 message content stream. The R002
message content (reviewed in Finding 5 below) contains only `RuleId` values and
is G13-clean by code inspection — but the corpus-level enforcement test does not
verify this invariant.

Additionally, the `no_document_text_leaks_into_diagnostic_messages` test checks
`result.diagnostics` from `engine.lint()`, which does not include the pre-existing
`Diagnostic.message` format-string interpolation leak channel present in rules
like E006 (line 762–764: `format!("{:?} is a deprecated dissemination control; replace with {:?}", token.text, entry.replacement)`). This is an acknowledged pre-existing issue scheduled for closure in PR 3c.2 and is out of scope for 7b — noted here for completeness only.

### Remediation

Extend `no_document_text_leaks_into_diagnostic_messages` (or add a companion test)
to also run `engine.fix()` on each corpus fixture and apply the prose sentinel
check to `result.remaining_diagnostics`:

```rust
let fix_result = engine.fix(source, FixMode::Apply);
for d in &fix_result.remaining_diagnostics {
    for sentinel in PROSE_SENTINELS {
        assert!(!d.message.contains(sentinel), ...);
    }
}
```

This is low effort and closes the corpus-level gap for the R002 output stream.
The vacuity guard (fix result should produce at least one remaining diagnostic
from the corpus) is inherently satisfied by the invalid-fixture set.

**Constitution principle**: V Principle V (G13 invariant, corpus-level test
requirement).

---

## Finding 4 — LOW: `r002_does_not_mint_applied_fix` test checks string literal `"R002"` not `R002_RULE_ID` constant

**File**: `crates/engine/tests/audit_completeness.rs` line 225

### Description

```rust
assert_ne!(
    fix.rule.as_str(),
    "R002",
    "R002 must never appear as an AppliedFix"
);
```

The assertion compares against the hard-coded string literal `"R002"` rather than
`R002_RULE_ID` (the constant defined in `engine.rs`). If the constant's value
changes in a future rename (e.g., to reflect the engine-synthetic namespace
`("engine", "r002.reparse-failed")` referenced in `MessageTemplate::ReparseFailed`'s
doc comment), the string literal would silently pass while `R002_RULE_ID` would
catch the drift.

### Remediation

```rust
// Import R002_RULE_ID from the engine crate and use it here
assert_ne!(
    fix.rule.as_str(),
    marque_engine::R002_RULE_ID.as_str(),
    "R002 must never appear as an AppliedFix"
);
```

If `R002_RULE_ID` is not currently re-exported from `marque_engine`, adding
`pub use crate::engine::R002_RULE_ID;` to `lib.rs` is the correct fix.

**Constitution principle**: VIII (citation stability / identifier drift).

---

## Positive Findings — No Issues

The following items were reviewed and found compliant:

### `__engine_promote` rule-crate isolation

`grep -rn "__engine_promote" crates/capco/src/ crates/rules/src/` returns no call
sites (only doc comments and references). No rule crate calls `__engine_promote`
in production code. Compliant.

### R002 emission path — no `__engine_promote` call

`build_r002_diagnostic` at lines 2683–2724 returns a `Diagnostic<CapcoScheme>`.
It does not call `__engine_promote`, `__engine_promote_text_correction`, or
`engine_promotion_token`. The R002 result path (`assemble_r002_result`) pushes
the R002 diagnostic into `remaining_diagnostics`, never into `applied`. Compliant.

### `MessageArgs.contributing_rule_ids` field type

The field is `SmallVec<[RuleId; 4]>` — confirmed in `crates/rules/src/message.rs`
line 436. `RuleId` is on Constitution V's permitted-identifier list. The
destructuring-pin test at `crates/rules/tests/message_args_closed_set.rs` correctly
asserts the type `SmallVec<[RuleId; 4]>` and the closed-set E0027 enforcement is
in place. Compliant.

### `MessageArgs` closed-set discipline

The compile-fail doctests on `MessageArgs` in `message.rs` pin the absence of
`String`, `&str`, `Vec<u8>`, `From<&str>`, and `From<String>`. The new
`contributing_rule_ids` field passes the destructuring pin test at
`message_args_closed_set.rs:56`. No `String` or `Vec<u8>` slipped in. Compliant.

### R002 message content — G13 compliance

`build_r002_diagnostic` constructs `rule_list` by calling `id.as_str()` on each
`RuleId` in `contributing_rule_ids`. The resulting message strings are:
- `"post-pass-1 buffer failed to re-parse; pass-2 skipped"` (empty case)
- `"post-pass-1 buffer failed to re-parse after applying pass-1 fixes from C001; pass-2 skipped"` (example)

No document bytes, no token text, no marking content. `RuleId` values are G13-
permitted identifiers ("enumerated feature labels" per Constitution V). Compliant.

### `FixResult.r002_fired` boolean — no information leak

The boolean is set from a structural branch condition (`post_pass1_had_no_markings
&& pre_pass1_had_markings`) and used only as a branch gate or direct WASM
serialization. No code path uses `r002_fired == true` to derive document content
into any output. Compliant.

### `FixResultJson.r002_fired` WASM exposure

The WASM `FixResultJson` struct at `crates/wasm/src/lib.rs:522` adds only
`r002_fired: bool`. No `MessageArgs.contributing_rule_ids` is serialized through
the WASM JSON boundary — `DiagnosticJson` at line 268 serializes only `rule`,
`severity`, `span`, `message`, `citation`, `fix`, none of which exposes
`contributing_rule_ids`. Compliant.

### Test-fixture carve-out compliance

All `__engine_promote` calls in test files carry the required
`"Test-fixture carve-out per Constitution V"` comment and live in `tests/`
integration files or `#[cfg(test)]` modules. No fabricated `AppliedFix` is
commingled with engine output. Verified in `crates/engine/tests/audit.rs:389`.
Compliant.

### No new `#[doc(hidden)] pub` additions

`grep -rn "doc(hidden)" crates/engine/src/ crates/rules/src/` shows only the
pre-existing `__engine_promote`, `__engine_promote_text_correction`, and
`EnginePromotionToken::__engine_construct` annotations. No new `#[doc(hidden)] pub`
functions were added in PR 7b. Compliant.

### Crate dependency graph — no new `marque-capco` dep in `marque-rules`

`crates/rules/Cargo.toml` does not depend on `marque-capco`. Constitution VII
one-directional graph preserved. Compliant.

### Cross-pass buffer isolation

Pass-2 receives `pass2_source` which is either `pass0.effective_source` (when
pass-1 applied no fixes, reusing the post-pass-0 buffer) or `pass1.post_buffer`
(the post-splice bytes after pass-1 fixes). In neither case does pass-2 see
pre-pass-0 source bytes after pass-1 mutations. The buffer isolation is correct.
Compliant.

### `SmallVec` spill — G13 compliance preserved on heap allocation

`SmallVec<[RuleId; 4]>` with more than four entries spills to the heap, but the
element type remains `RuleId` on both the inline path and the heap path. Heap
allocation does not change the G13 compliance of the field contents. Compliant.

---

## Compliance Posture Summary

The R002 diagnostic emission path is G13-clean and correctly avoids `__engine_promote`.
The `MessageArgs.contributing_rule_ids` field addition is type-correct and passes the
closed-set enforcement gate.

The blocking issue is the CI enforcement gap: the `promote-callsite-lint` gate now
reports PRC002 errors for the two `__engine_promote` calls that PR 7b moved into
`impl TwoPassFixer`. This is not a content-ignorance failure — the calls themselves
are structurally correct — but the enforcement mechanism is broken and must be
repaired before merge. Until the lint allow-list (or the call-site location) is
updated, the CI gate that prevents unauthorized `__engine_promote` usage elsewhere
in the codebase is not enforcing the full production surface.

The vacuous R002 test (HIGH) is a test-quality gap that leaves the audit-record
no-promotion contract for R002 as documentation-only rather than enforcement-level.
The G13 corpus gap (MEDIUM) leaves `FixResult.remaining_diagnostics` outside the
automated sentinel sweep.

**Merge recommendation**: Do not merge until Finding 1 (CRITICAL) and Finding 2
(HIGH) are resolved. Findings 3 and 4 may be deferred to a follow-up PR if the
team determines the risk is acceptable, but Finding 1 blocks CI and must land
before merge regardless.
