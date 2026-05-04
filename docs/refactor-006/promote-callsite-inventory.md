<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Promote-callsite inventory (engine-refactor-006 PR 0)

**Source**: `specs/006-engine-rule-refactor/spec.md` FR-040; `specs/006-engine-rule-refactor/tasks.md` T009a; `.specify/memory/constitution.md` § Principle V (Audit-First Compliance, the "Test-fixture carve-out per Constitution V" three-constraint scope).

**Lint enforcement**: `tools/promote-callsite-lint/` walks `crates/*/src/**`, `crates/*/tests/**`, and (workspace-root) `tests/**` for `AppliedFix::__engine_promote(...)` and `EnginePromotionToken::__engine_construct()` calls. Production-code calls are allowed only inside `Engine::fix_inner` and the closely-coupled `Engine::apply_text_corrections` (the helper `engine_promotion_token()` they both invoke counts as a delegated production site, not a third surface). Test-code calls require a `// Test-fixture carve-out per Constitution V` comment within 5 lines of the call (the marker line itself is allowed to be the multi-line variant — only one occurrence of the canonical phrase needs to land in the 5-line window).

The lint additionally enforces the **D12 signature-shape extension** (R-11): any function with shape `*ParsedAttrs* → *CanonicalAttrs*` outside (a) `unsafe fn`, (b) `MarkingScheme::canonicalize`, or (c) the transitional `crates/ism/src/attrs.rs::from_parsed_unchecked` is flagged. At PR 0 HEAD this lint pass has zero matches because the `ParsedAttrs` and `CanonicalAttrs` types do not yet exist (they land at PR 3a, tasks.md T020 / T021).

**Disposition at PR 0 HEAD**: 3 production `__engine_promote` sites + 1 production `__engine_construct` helper site (all in `crates/engine/src/engine.rs`), 2 test-fixture `__engine_promote` sites + 2 test-fixture `__engine_construct` sites across two files, all carve-out-marked.

## Production call sites (allowed: inside `Engine::fix_inner` / `Engine::apply_text_corrections`)

### `crates/engine/src/engine.rs:1080` — `AppliedFix::__engine_promote`

- **Enclosing fn**: `Engine::fix_inner` (declared at line 861), inside the `FixMode::Apply` arm of the `match mode { ... }` (the `Apply`-mode audit-record-construction loop at lines 1073–1088).
- **Lint disposition**: ALLOWED per FR-040 production carve-out.
- **Token source**: the call's last argument is `engine_promotion_token()` — the file-private helper at line 1247 (see below) that mints the `EnginePromotionToken`.
- **Why this site exists**: this is the audit-promotion step for `Apply`-mode fixes after the forward-pass buffer rewrite at lines 1057–1065 has already run.

### `crates/engine/src/engine.rs:1099` — `AppliedFix::__engine_promote`

- **Enclosing fn**: `Engine::fix_inner` (declared at line 861), inside the `FixMode::DryRun` arm at lines 1092–1109.
- **Lint disposition**: ALLOWED per FR-040 production carve-out.
- **Token source**: `engine_promotion_token()`.
- **Why this site exists**: dry-run produces the same `applied` audit stream as `Apply` so the two modes' audit envelopes are byte-comparable; the only difference is `dry_run = true` and the source buffer is not mutated.

### `crates/engine/src/engine.rs:1211` — `AppliedFix::__engine_promote`

- **Enclosing fn**: `Engine::apply_text_corrections` (declared at line 1152), inside the C001 pre-scanner text-correction apply loop at lines 1209–1219.
- **Lint disposition**: ALLOWED per FR-040 production carve-out. **Note**: the prompt's prior reading of the inventory described all three production `__engine_promote` sites as living inside `Engine::fix_inner`; verified against the file at PR 0 HEAD, this third call is in a separate `Engine::apply_text_corrections` helper that `fix_inner` calls. Both are `impl Engine` methods on the same type and both run on the engine's audit-record-emission path; the FR-040 production carve-out covers both. The lint must whitelist both fn names (or, equivalently, whitelist any call whose dynamic call chain passes through `Engine::fix_inner`).
- **Token source**: `engine_promotion_token()`.
- **Why this site exists**: pass-1 text corrections (C001) run before the scanner so that downstream rules see corrected input; their fixes still need an audit record, and that record is emitted here.

### `crates/engine/src/engine.rs:1248` — `EnginePromotionToken::__engine_construct`

- **Enclosing fn**: `engine_promotion_token() -> EnginePromotionToken` (declared at line 1247), the file-private helper called from each of the three production `__engine_promote` sites above.
- **Lint disposition**: ALLOWED per FR-040 production carve-out — this is the *single* place inside `marque-engine` where the engine grants itself promotion privilege; the doc comment at lines 1229–1245 explicitly notes this centralization is what makes "where does the engine decide to promote?" a one-grep question.
- **Why this site exists**: the helper exists so that adding a fourth promotion site forces a deliberate decision to thread through this function rather than minting a token ad-hoc next to the new `__engine_promote` call.

## Test-fixture call sites (Constitution V Principle V carve-out)

### `crates/engine/tests/audit.rs:356` and `:362`

- **Calls**:
  - `AppliedFix::__engine_promote(...)` at line 356 (multi-line argument list spans lines 356–363).
  - `EnginePromotionToken::__engine_construct()` at line 362 (passed as the final positional argument).
  - Both are inside `fn fabricate_leaky_fix() -> AppliedFix` declared at line 333.
- **Carve-out comments present** (multiple, layered):
  - **Doc-comment carve-out** at lines 326–332 above the fn declaration: `/// Test-fixture carve-out per Constitution V Principle V: this fabricated AppliedFix is the input to check_fixes_clean's G13 sentinel sweep, exists only inside the tests/ tree, and is never spliced into a real audit stream. Engine production paths remain the only route to a real AppliedFix in cfg(not(test)) code; see the doc comment on AppliedFix::__engine_promote for the three-constraint definition of the carve-out.`
  - **Inline carve-out** at lines 349–354 (multi-line `//` comment) immediately above the `__engine_promote` call, plus a redundant short-form repetition at line 355: `// Test-fixture carve-out per Constitution V`.
  - The line-355 short form sits 1 line above the `__engine_promote` call (line 356) and 7 lines above the `__engine_construct` call (line 362). The lint's 5-line window measured from line 355 covers line 356 cleanly. For line 362 the closest carve-out marker within 5 lines is at line 357+ — the inline multi-line block at lines 349–354 sits 8 lines above. **However**, the canonical "Test-fixture carve-out per Constitution V" phrase appears at line 355, which is 7 lines above line 362; that's outside a strict 5-line window measured from the call. The implementer should verify the lint's window-counting policy: if the lint counts from the marker phrase to the call's *first* line (not its closing paren), line 355 → line 362 = 7 lines is outside 5; the safest fix is to either (a) tighten the call onto fewer lines so line 362 moves into the window, or (b) add a second short-form `// Test-fixture carve-out per Constitution V` marker immediately before line 362. **Flag**: the layered comment structure is robust enough for human readers but is brittle against the lint's mechanical 5-line window; recommend adjustment in the same PR that lands the lint.
- **Three-constraint scope verification** (Constitution V Principle V):
  1. The call site lives in `crates/engine/tests/audit.rs` — that's a `tests/` integration file, properly `cfg(test)`-gated by Cargo. Constraint 1 (test-only call sites, never `cfg(not(test))`-reachable) is satisfied.
  2. The synthesized `AppliedFix` is consumed by `sentinel_check_panics_on_synthetic_leak` at lines 366–375, which is `#[should_panic(expected = "G13 violation")]`-marked. The test calls `fabricate_leaky_fix()` and feeds the result to `check_fixes_clean(&[leaky], "synthetic self-test")` (the G13 sentinel sweep at lines 149–155). The fabricated fix is **never** spliced into a real `FixResult.applied` stream — it is the input to a checker, exactly the carve-out's stated purpose. Constraint 2 (never commingled with engine-promoted output) is satisfied. *(Verification source: lines 366–375 of `crates/engine/tests/audit.rs`, read directly.)*
  3. The construction is exercising the audit-emission machinery's failure mode: a regression that emptied `PROSE_SENTINELS` or short-circuited `assert_clean` would cause this `#[should_panic]` test to flip to *passing* without panicking, surfacing the regression. No CLI / batch / bench helper imports `fabricate_leaky_fix` — confirmed by `grep -r 'fabricate_leaky_fix' crates/ --include='*.rs'` returning matches only inside `audit.rs` itself. Constraint 3 (test-fixture *construction* only, not "convenience" `AppliedFix` minting for non-test code) is satisfied.

### `crates/rules/tests/engine_promotion_seal.rs:46` and `:47`

**Note**: this site was not in the prompt's enumerated inventory; discovered via `grep -rn '__engine_promote\|__engine_construct' crates/ --include='*.rs'` during verification. Including it here so the lint's whitelist is complete.

- **Calls**:
  - `EnginePromotionToken::__engine_construct()` at line 46.
  - `AppliedFix::__engine_promote(...)` at line 47 (multi-line argument list spans lines 47–54).
  - Both are inside `fn documented_door_can_mint_token_from_outside_marque_rules` at line 31, the only test in the file.
- **Carve-out comment present**: inline multi-line `//` carve-out at lines 32–36 immediately inside the test fn, opening with `// Test-fixture carve-out per Constitution V Principle V:` (line 32). Lines 32 → 46/47 = 14/15 lines apart; the carve-out's canonical phrase is well outside the lint's 5-line window. **Flag**: the comment is at the top of the fn, separated from the calls by a multi-line `FixProposal::new(...)` construction (lines 37–45). Same remediation options as the `audit.rs` site: tighten or duplicate the marker phrase immediately above line 46.
- **Three-constraint scope verification** (Constitution V Principle V):
  1. The call site lives in `crates/rules/tests/engine_promotion_seal.rs` — a `tests/` integration file, `cfg(test)`-gated. Constraint 1 satisfied.
  2. The synthesized `AppliedFix` is asserted-on at lines 55–56 (`assert_eq!(applied.proposal.rule.as_str(), "E001"); assert!(!applied.dry_run);`) and never returned from the test fn. The file's module doc comment (lines 5–23) explicitly frames this as an acceptance test for the type-level seal: it pins the documented door (`__engine_construct()` works from outside `marque-rules`) while a sibling `compile_fail` doctest in `crates/rules/src/lib.rs` pins the brace-construct path being unreachable. Constraint 2 satisfied — no commingling with engine output.
  3. The construction's purpose is verifying the engine-only door is usable across the crate boundary (gap register #5 acceptance per the file's doc comment). This is test-fixture construction, not convenience minting. Constraint 3 satisfied.

## D12 signature-shape extension status at PR 0 HEAD

`grep -rEn 'fn .*ParsedAttrs.*-> .*CanonicalAttrs' /home/user/marque/crates/ --include='*.rs'` returns **zero matches** at PR 0 HEAD — the `ParsedAttrs` and `CanonicalAttrs` types are introduced at PR 3a (tasks.md T020 / T021). The lint's signature-shape pass therefore has nothing to flag at this PR's HEAD; the whitelist for (a) `unsafe fn`, (b) `MarkingScheme::canonicalize` impls, and (c) the transitional `crates/ism/src/attrs.rs::from_parsed_unchecked` exists preemptively for PR 3a's landing.

## Verification at PR 0 HEAD

After PR 0 lands (with `tools/promote-callsite-lint/` operational), verify:

```sh
cargo run --manifest-path tools/promote-callsite-lint/Cargo.toml --release -- \
    --workspace-dir . --all
```

Expected output: zero errors, modulo the two flagged comment-window issues called out above (resolve in the same PR that lands the lint, either by tightening the call layout or duplicating the canonical phrase immediately before each call).

Cross-check the production whitelist:

```sh
grep -n 'AppliedFix::__engine_promote\|EnginePromotionToken::__engine_construct' \
    /home/user/marque/crates/engine/src/engine.rs
```

Expected: exactly four lines reported — 1080, 1099, 1211, 1248. Any additional production-side hit indicates a fourth promotion surface that bypassed the `engine_promotion_token()` helper centralization (Constitution V Principle V), which the lint must reject regardless of carve-out comments.

Cross-check the test-fixture surface:

```sh
grep -rn 'AppliedFix::__engine_promote\|EnginePromotionToken::__engine_construct' \
    /home/user/marque/crates/ --include='*.rs' \
    | grep -v 'crates/engine/src/engine.rs' \
    | grep -v 'crates/rules/src/lib.rs'
```

Expected: four lines — `crates/engine/tests/audit.rs:356`, `crates/engine/tests/audit.rs:362`, `crates/rules/tests/engine_promotion_seal.rs:46`, `crates/rules/tests/engine_promotion_seal.rs:47`. Any additional test-side hit must arrive with a Constitution V Principle V carve-out comment within the lint's window or fail the lint.
