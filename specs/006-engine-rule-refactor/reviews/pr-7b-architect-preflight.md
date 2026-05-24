<!--
SPDX-FileCopyrightText: 2026 The marque Project Contributors
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7b Architect Pre-Flight — Two-Pass Restructure + R002

> Tactical pre-flight verifying that the binding design from
> `pr-7-pm-decisions.md` and `pr-7-architect-plan.md` lands cleanly on
> the post-7a `staging` (commit `fd67cc8c`) tree. Scope: T076–T079
> plus PM-mandated additions (first-fire span-shape check,
> `EX_R002_PARTIAL`, BatchEngine worst-row-wins, D1 consumer surface).

## TL;DR

The locked PR-7b design is implementable on current `staging` without
new PM decisions. C001 is already `Phase::Localized`
(`crates/capco/src/rules.rs:1214-1216`) and mechanically idempotent
when re-dispatched against the post-pass-0 buffer because its `check`
body only emits when `ctx.corrections.get(token_text) != Some(text)`
— once pass-0 rewrites `SERCET → SECRET`, the lookup misses or hits
the M2 no-op guard at `rules.rs:1234-1237`. No engine special-case
needed (§1). Four load-bearing items: (i) build pass-1 buffer by
sorting FR-016 then `iter().rev()` over a pre-allocated `Vec<u8>`
per the reference at `engine.rs:1524-1604` — NOT `Vec::splice` (§2);
(ii) R002 is a `Diagnostic`, never an `AppliedFix` — no
`__engine_promote` call (§4); (iii) add
`pub contributing_rule_ids: SmallVec<[RuleId; 4]>` to `MessageArgs`
at `crates/rules/src/message.rs:381-416` (§6); (iv) add
`pub r002_fired: bool` to `FixResult` at
`crates/engine/src/output.rs:145-154` so WASM / IDE consumers detect
partial application without NDJSON parsing (§7).

---

## 1. C001 dual-path idempotency — the L-1 forward obligation

**Verified: mechanical no-op. Recommendation (a) from the prompt.**

C001's `check` body (`crates/capco/src/rules.rs:1217-1252`) walks
`attrs.token_spans` (skipping `Separator`), looks up
`token_span.text.as_str()` in `ctx.corrections`, and emits a
diagnostic only when `Some(replacement)` is found AND
`replacement != text` (the M2 no-op guard at line 1234-1237).

After pass-0 rewrites the source buffer, the pass-1 lint produces
fresh `CanonicalAttrs` whose `token_spans[i].text` reflects the
corrected bytes. The lookup against the same `ctx.corrections` map
either (a) misses entirely (typical — `SECRET` is not a corrections
key), (b) hits with `replacement == text` and the M2 guard drops
it, or (c) hits a cascading correction the user configured
(`OC → ORCON; ORCON → ORIGINATOR CONTROLLED`). Case (c) is
**correct behavior** — cascade support — and the re-parse between
pass-1 and pass-2 catches any unparseable cascade as R002.

**Recommendation**: ship as-is. Add one inline comment in
`TwoPassFixer::run_pass1_localized` noting that C001-as-rule
re-runs after C001-as-pre-pass-0, intentionally (cascade support).
No engine special-case.

**5-year-maintenance test**: The alternative (engine-side skip of
C001 in pass-1) would silently break cascade support, which is
exactly the "later" bug class the design is meant to prevent. The
mechanical idempotency costs zero engine code.

---

## 2. Pass-1 forward buffer construction (PM D-7.5)

The reference pattern is at `engine.rs:1524-1604` (the existing
`FixMode::Apply` block). PM D-7.5 calls it "forward-pass buffer
construction"; the actual idiom is: **sort fixes by FR-016 key
(span.end DESC, span.start DESC, rule_id ASC, replacement ASC),
then `iter().rev()` so boundaries are encountered in ASCENDING
span.start order, copying gaps left-to-right into a pre-allocated
`Vec<u8>`.** The reverse-iteration over reverse-sorted fixes IS
the forward pass over source bytes — the load-bearing loop is
`kept_fixes.iter().rev()` at `engine.rs:1562`.

**Pass-1 algorithm sketch** (in `TwoPassFixer::run_pass1_localized`):

```text
pass1_fixes = synthesize_fixes(..., post_c001_attrs, ...)
              |> filter(rule.phase() == Phase::Localized)
              |> filter(span_is_sub_token(diag.span, marking_attrs))
              |> sort by FR-016 key
              |> C-1 overlap-dedup walk
extra = Σ saturating_sub(replacement.len(), span.end - span.start)
buf = Vec::with_capacity(effective_source.len() + extra)
last_end = 0
for fix in kept_fixes.iter().rev():
    buf.extend_from_slice(&effective_source[last_end..fix.span.start])
    buf.extend_from_slice(fix.replacement.as_bytes())
    last_end = fix.span.end
buf.extend_from_slice(&effective_source[last_end..])
```

**Cross-rule C-1 overlap — structural behavior change**: the
existing C-1 dedup at `engine.rs:1466-1477` operates over *all*
synthesized fixes regardless of phase. Pass-1's C-1 dedup MUST run
on the `Phase::Localized` partition only; pass-2 runs its own C-1
on the `Phase::WholeMarking` partition. **Two independent dedup
walks, one per phase.** This preserves I-1 because the partitions
are disjoint by rule, so the union of winners has no rule-and-span
collision.

**5-year-maintenance test**: A future ordering rule emitting
sub-token fixes could conflict with C001 on the same span. The
per-phase dedup gives the engine a clear answer ("localized fires;
whole-marking sees post-pass-1 attrs"); a shared dedup walk would
silently merge two semantically-different repair channels.

---

## 3. Short-circuit when `pass1_applied.is_empty()` (PM D-7.5)

**Concrete branch shape** in `TwoPassFixer::run`:

```text
let pass0 = self.run_pass0_c001(source);
let (lint, attrs) = self.lint_with_options_internal(&pass0.effective_source, ...);
let pass1 = self.run_pass1_localized(&pass0, &lint, &attrs);

let (buffer, post_attrs) = if pass1.applied.is_empty() {
    (pass0.effective_source, attrs)  // reuse — no buffer change, no re-parse
} else {
    let buf = pass1.post_pass_1_buffer;
    match self.try_reparse(&buf) {
        Ok(new_attrs) => (buf, new_attrs),
        Err(failure_span) => {
            let r002 = self.build_r002_diagnostic(&pass1.applied, failure_span);
            return Ok(FixResult {
                source: buf, applied: pass1.applied,
                remaining_diagnostics: vec![r002], r002_fired: true });
        }
    }
};

let pre_cache = self.populate_pre_pass_1_cache(&pass1, &attrs);  // 7c-scope; empty when short-circuited
let pass2 = self.run_pass2_whole_marking(&buffer, &post_attrs, &pre_cache, ...);
```

The pre-pass-1 attrs cache hook (T080, 7c-scope) lives at the
single point of decision; the short-circuit leaves it empty, which
means every `RuleContext.pre_pass_1_attrs` is `None` in pass-2 —
correct semantics ("no pass-1 fix preceded this marking, no FR-023
disambiguation needed"). PR 7b lays down the trampoline; 7c only
adds the cache population body.

**5-year-maintenance test**: The bench split (T085's
`fix_10kb_pass2_only` vs `fix_10kb_two_pass`) makes regression
toward "always re-parse for invariant safety" visible. The
short-circuit is the load-bearing optimization.

---

## 4. R002 audit-record shape

**R002 does NOT emit an `AppliedFix`.** Per Constitution V Principle
V: R002 is a *diagnostic*, not a fix. It carries no replacement
bytes, no intent, no fix proposal. The contributing pass-1 fixes
DO produce `AppliedFix` records; R002 sits alongside them in
`FixResult.remaining_diagnostics`.

**Concrete shape** (in `build_r002_diagnostic`, adjacent to
`build_decoder_diagnostic` at `engine.rs:2132`):

```text
Diagnostic<CapcoScheme> {
    rule: RuleId::new("R002"),
    severity: Severity::Error,        // FR-024 — re-parse failure is always Error
    span: failure_span,                // see §5 for what this is
    candidate_span: None,
    message: Message::new(
        MessageTemplate::ReparseFailed,
        MessageArgs {
            contributing_rule_ids: pass1_applied.iter()
                .map(|af| af.rule.clone()).collect(),
            ..MessageArgs::default()
        }),
    citation: "engine-synthetic",      // mirrors DECODER_CITATION; R002 has no CAPCO §
    fix: None,
    text_correction: None,
}
```

`__engine_promote` is **NOT** called for R002. The audit log
(`FixResult.applied`) contains only pass-0 + pass-1 promoted
fixes. Constitution V G13 check: `RuleId` and `Span` are both on
the permitted-identifier list; no document bytes flow through R002.

**5-year-maintenance test**: A future reader sees three engine
call sites for `__engine_promote` (pass-0 text-corrections,
pass-1 fix promotion, pass-2 fix promotion) — and R002 absent
from that list. The `tools/promote-callsite-lint/` CI lint
catches drift toward "R002 should mint an AppliedFix."

---

## 5. First-fire phase-span-shape check (PM D-7.2 replacement for T075)

**Sub-token shape definition**: `diag_span` is sub-token when it
is strictly contained within a single parsed `TokenSpan.span`:

```text
fn span_is_sub_token(diag_span: Span, attrs: &CanonicalAttrs) -> bool {
    attrs.token_spans.iter().any(|tok|
        tok.span.start <= diag_span.start && diag_span.end <= tok.span.end)
}
```

Empty `token_spans` (degenerate parse) → fails → fix dropped. Desired
behavior.

**Position**: in `TwoPassFixer::run_pass1_localized`, between
`synthesize_fixes` and the FR-016 sort, BEFORE C-1 dedup. The filter
pipeline:

```text
let pass1_fixes: Vec<SynthesizedFix> = synthesized_fixes.into_iter()
    .filter(|sf| {
        if self.engine.rule_phase_for(&sf.rule) != Phase::Localized { return false; }
        let attrs = lookup_marking_attrs_containing(&attrs_list, sf.span);
        let in_shape = attrs.map_or(false, |a| span_is_sub_token(sf.span, a));
        if !in_shape {
            debug_assert!(in_shape,
                "Phase::Localized rule {} emitted out-of-shape span ({:?}); dropping",
                sf.rule, sf.span);
            tracing::error!(rule_id = %sf.rule, span = ?sf.span,
                marking_span = ?attrs.map(|a| a.candidate_span),
                "Phase::Localized rule emitted out-of-shape span; dropping fix");
            return false;
        }
        true
    }).collect();
```

`debug_assert!` panics in `cfg(debug_assertions)` (CI catches);
`tracing::error!` always fires; the audit stream records nothing
for a dropped fix (no `AppliedFix`). Parallel to
`rule_panic_isolation.rs`.

**Data-flow note**: the marking-containment lookup is a linear scan
over `parsed_markings` (already in scope post-lint). The typical
document has <100 markings; an out-of-shape fix is a defect path,
not a hot path. No binary-search optimization needed in 7b.

**5-year-maintenance test**: PR 7a's
`crates/capco/tests/phase_assignment.rs` allowlist test fails
first if a rule isn't enumerated; the first-fire `debug_assert!`
fails second if emissions don't match declared phase. Two gates,
both load-bearing.

---

## 6. `MessageArgs.contributing_rule_ids` field placement

**Location**: `crates/rules/src/message.rs:381-416`, as a sibling
to `feature_ids`:

```text
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MessageArgs {
    pub token: Option<TokenId>,
    pub category: Option<CategoryId>,
    pub span: Option<Span>,
    pub digest: Option<Blake3Hash>,
    pub confidence: Option<Confidence>,
    pub expected_token: Option<TokenId>,
    pub actual_token: Option<TokenId>,
    pub feature_ids: SmallVec<[FeatureId; 4]>,
    /// Contributing pass-1 fix rule IDs for R002 (`MessageTemplate::
    /// ReparseFailed`) diagnostics. Empty for every other variant.
    /// Per FR-024 + Constitution V G13 (RuleId permitted).
    pub contributing_rule_ids: SmallVec<[RuleId; 4]>,
}
```

**Derives**: `Default`, `Clone`, `PartialEq`, `Debug` already on
the struct. `RuleId` is `#[derive(Debug, Clone, PartialEq, Eq,
Hash)]`. No new derive needed.

**Closed-set discipline preserved**: the existing compile-fail
tests enforce absence of `String` / `Vec<u8>` / free-form
constructors. Adding `SmallVec<[RuleId; 4]>` satisfies G13
permitted-identifier closure — no compile-fail test needs an
update.

**Audit emitter**: the NDJSON emitter MUST skip the field when
empty (typical for non-R002 messages). Use
`#[serde(skip_serializing_if = "SmallVec::is_empty")]` if the
emitter uses serde derives.

**5-year-maintenance test**: The closed-set type discipline keeps
R002 from drifting into a free-form "and here's what went wrong"
string field.

---

## 7. D1 consumer surface — `contracts/engine-pipeline.md` insertion

The contract file already has a stub "R002 surfacing semantics"
section at `contracts/engine-pipeline.md:169-185`. PR 7b's job is to
flesh it out with concrete values:

1. **When R002 fires**: `parse(post_pass_1_buffer)` returns `Err`.
   Specifically, pass-1 produced ≥1 applied fix AND the post-splice
   buffer is unparseable. Re-parse uses
   `lint_with_options_internal`, which goes through the engine's
   installed `Recognizer` — dispatcher behavior matches the
   original lint pass.

2. **Audit-record consumer view**: `FixResult.applied` contains
   pass-0 corrections + pass-1 `AppliedFix` records.
   `FixResult.remaining_diagnostics` contains the R002 diagnostic
   plus any un-fixed pass-1 diagnostics. `FixResult.source` is
   the post-pass-1 buffer (not the original). Pass-2 skipped.

3. **CLI exit code**: `EX_R002_PARTIAL = 3` per PM D-7.8 at
   `marque/src/main.rs:26-33`. Exit-code branch at line 900-904
   gains a higher-priority test: **`if result.r002_fired { return
   EX_R002_PARTIAL; }` BEFORE the `has_errors` / `has_warns`
   chain**. Numerically `3` sits between `EX_DIAG_WARN = 2` and
   the sysexits range at 64, so worst-row-wins ordering works.

4. **WASM detection without NDJSON parsing**: add
   `pub r002_fired: bool` to `FixResult` at
   `crates/engine/src/output.rs:145-154`. Default false; engine
   sets true in R002 emission branch. WASM bindings serialize as
   a top-level JS-object boolean. JS / IDE consumers check
   `result.r002_fired` before touching `result.source`. Zero
   NDJSON parsing.

5. **BatchEngine worst-row-wins**: `BatchEngine::fix_many` yields
   per-row `Result<FixResult, BatchError>` in completion order
   (`batch.rs:437-456`). Aggregation lives in the **caller**
   (CLI / server). Per-row `r002_fired` is the signal; CLI batch
   driver maintains `max(exit_code, row_exit_code)` and an
   R002 row contributes 3. **No change to `BatchEngine`
   itself.**

**Why `r002_fired: bool` and not a `Result` variant**: (a)
`Result<FixResult, EngineError>` is reserved for unrecoverable
errors; R002 is recoverable (pass-1 buffer is valid partial
output). (b) Boolean serializes at the WASM serde boundary
without exhaustive matching. (c) IDE plugins detect it with a
single property read.

**5-year-maintenance test**: A second synthetic engine error
(R003) adds a second boolean. Manageable until three exist;
refactor to a `partial_state: PartialState` enum at that point.
YAGNI — do not pre-build the enum.

---

## 8. Test scope for PR 7b

Minimum to land safely (full property + Layer-3 invariant tests
are 7c per T083/T084):

1. **`tests/fix_pipeline.rs` (extend)** — short-circuit: a clean
   fixture with zero `Phase::Localized` triggers produces
   `result.r002_fired == false` and `result.source` byte-equals
   the pass-0 output. Negative test for the re-parse path.

2. **`tests/fix_pipeline.rs` (new test)** — forward-buffer
   correctness for >1 pass-1 fix: a portion with two C001
   corrections in one marking; snapshot the output buffer.
   Exercises C-1 dedup + FR-016 sort + reverse-iteration walk.

3. **`tests/fix_pipeline.rs` (new test)** — re-parse failure
   produces R002. Synthesize an unparseable post-pass-1 buffer
   via a pathological correction (`SECRET → ///`); assert
   `r002_fired == true`, one R002 diagnostic in
   `remaining_diagnostics`, contributing pass-1 fixes present in
   `result.applied`, and `MessageArgs.contributing_rule_ids`
   matches the applied set.

4. **`tests/audit_completeness.rs` (extend)** — R002 does NOT
   mint an `AppliedFix`. Same fixture as (3); assert no
   `AppliedFix` in `result.applied` carries `rule ==
   RuleId::new("R002")`. Pins Constitution V Principle V.

5. **`marque/tests/cli_exit_codes.rs` (new or extend)** —
   `EX_R002_PARTIAL = 3` from `marque fix` CLI happy path.
   Spawn `marque fix --in-place` on a fixture-3 file; assert
   process exit == 3 AND stderr NDJSON contains R002.

6. **`crates/engine/tests/batch_r002.rs` (new)** — per-row
   `r002_fired` is inspectable at the batch boundary. Submit a
   mixed batch (one R002 row + clean rows); assert each row's
   `FixResult.r002_fired` matches expectation. Exit-code
   aggregation is CLI-side, not tested here.

7. **`contracts/engine-pipeline.md` parity** — no automated
   parity check exists today. **File a follow-up issue**: add a
   harness that grep-checks `EX_R002_PARTIAL` numeric value in
   `main.rs` against the value in `engine-pipeline.md`. For 7b,
   the reviewer attestation hand-clears CHK018 per PM D-7.14.

Property tests (`two_pass_invariants.rs`, FR-022/FR-023) need
the pre-pass-1 attrs cache and are 7c scope. PR 7b's tests
above are functional unit tests.

---

## Tactical hand-off checklist

All PM decisions needed are documented in `pr-7-pm-decisions.md`
(D-7.4, D-7.5, D-7.6, D-7.8, D-7.9, D-7.12). **No new PM
decisions required from this pre-flight.**

- [ ] §1: Ship C001 as-is. Add an inline comment in
  `TwoPassFixer::run_pass1_localized` naming the M2 no-op guard
  at `rules.rs:1234-1237` as the load-bearing idempotency
  property. No engine special-case.
- [ ] §2: Build pass-1 buffer with FR-016 sort + `iter().rev()`
  (reference: `engine.rs:1524-1604`). NOT `Vec::splice`.
  Separate per-phase C-1 dedup walks.
- [ ] §3: `if pass1.applied.is_empty() { skip re-parse }`
  branch. Lay down the pre-pass-1 cache hook adjacent to this
  branch so 7c is purely additive.
- [ ] §4: R002 emits via `build_r002_diagnostic` (parallel to
  `build_decoder_diagnostic` at `engine.rs:2132`). NO
  `__engine_promote` call. R002 lives in
  `FixResult.remaining_diagnostics`.
- [ ] §5: First-fire span-shape filter on pass-1 fixes,
  before FR-016 sort. `debug_assert!` + `tracing::error!` +
  drop.
- [ ] §6: Add `pub contributing_rule_ids: SmallVec<[RuleId; 4]>`
  to `MessageArgs` at `crates/rules/src/message.rs:381-416`.
  Audit emitter skips when empty.
- [ ] §7: Add `pub r002_fired: bool` to `FixResult` at
  `crates/engine/src/output.rs:145-154`. Flesh out
  `contracts/engine-pipeline.md:169-185` with concrete values.
  CLI exit-code branch at `marque/src/main.rs:900-904` tests
  `r002_fired` BEFORE `has_errors` / `has_warns`.
- [ ] §8: Land tests (1)-(6). File follow-up issue for
  contract-parity test.
- [ ] CHK015 + CHK018: clear per PM D-7.14. CHK015 (no
  `Phase::Both`) is verified by inspecting C001/E006/E007/S004
  — none registers twice.

The 5-year-maintenance posture: short-circuit (§3), per-phase
C-1 dedup walks (§2), `r002_fired: bool` (§7), C001
idempotency (§1) — each named load-bearing here so a future
"simpler control flow" regression trips review.

---

## Files touched in PR 7b

- `crates/engine/src/engine.rs` — `fix_inner` → 5-line
  trampoline; new `TwoPassFixer`; `R002_RULE_ID` const adjacent
  to `DECODER_RULE_ID` at line 51; `build_r002_diagnostic`
  adjacent to `build_decoder_diagnostic` at line 2132
- `crates/engine/src/output.rs` — `FixResult.r002_fired: bool`
  at line 145-154
- `crates/rules/src/message.rs` — `MessageArgs.contributing_rule_ids`
  at line 381-416
- `marque/src/main.rs` — `EX_R002_PARTIAL = 3` at line 26-33;
  exit-code branch at line 900-904
- `specs/006-engine-rule-refactor/contracts/engine-pipeline.md`
  — flesh out "R002 surfacing semantics" at line 169-185
- `crates/engine/tests/fix_pipeline.rs` — tests (1)(2)(3)
- `crates/engine/tests/audit_completeness.rs` — test (4)
- `marque/tests/cli_exit_codes.rs` — test (5)
- `crates/engine/tests/batch_r002.rs` (new) — test (6)
- `crates/wasm/src/lib.rs` — expose `r002_fired` in the JS
  result struct (verify binding shape)

**No edits expected** in `crates/rules/src/lib.rs` (the `Rule`
trait surface landed in 7a), `crates/scheme/`,
`crates/capco/src/` (the 4 `Phase::Localized` declarations from
7a are sufficient), `crates/ism/`, or `crates/core/`.
