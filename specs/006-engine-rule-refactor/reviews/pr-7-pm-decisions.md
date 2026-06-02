<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7 — PM Decisions (Reconciled)

> Reconciles the pre-flight architect plan (`pr-7-architect-plan.md`)
> and the rust review (`pr-7-rust-review.md`). Decision authority for
> the items below is the PR-7 PM (this document). Where the two
> reviewers disagree, the rationale records why.

**Sources:**
- Architect plan: `docs/refactor-006/pr-7-architect-plan.md`
- Rust review: `docs/refactor-006/pr-7-rust-review.md`
- Spec FRs: `specs/006-engine-rule-refactor/spec.md` (FR-021 … FR-024, FR-035, FR-041, FR-042, FR-044, FR-049)
- Source plan: `docs/plans/2026-05-02-engine-refactor-consolidated.md` §9
- Decision D1: `specs/006-engine-rule-refactor/decisions.md:37-67`
- Checklist: `specs/006-engine-rule-refactor/checklists/correctness.md` §2 (CHK015/CHK018 GATE)

---

## D-7.1 Sub-PR split — three commits behind one umbrella

**Decision**: Split PR 7 into **7a / 7b / 7c** mirroring the PR 3b precedent.

| Sub-PR | Scope | Revertable |
|--------|-------|------------|
| 7a | T073, T074 (phase plumbing + per-rule declarations) | yes — `phase()` unread by engine |
| 7b | T076–T079 (two-pass restructure + R002 + exit code + D1 consumer surface) | yes, but load-bearing |
| 7c | T080–T085 (pre-pass-1 cache + `PrecedingFixPenalty` + property/invariant tests + `fix_10kb` bench) | yes |

**T075** is **REMOVED FROM SCOPE** (see D-7.3). The engine construction
error variant for phase-span-shape mismatch is structurally impossible
to enforce at `Engine::new`; replaced with a first-fire path.

Rationale: review surface is smaller per sub-PR; 7c carries the
conceptual novelty (cache lifetime, FR-023 disambiguation) so it
should land in isolation. Three CI runs is the cost; worth it.

---

## D-7.2 `Rule::phase()` default

**Decision**: Default to `Phase::WholeMarking`. Required-method approach
rejected.

**Rationale (both reviewers agree)**:
- Most rules are whole-marking by construction (27 of 31; 4 are Localized — C001, E006, E007, S004).
- Failing to declare yields the safer default — a localized rule
  running in pass-2 is conservative (no I-19 false positive) whereas
  a whole-marking rule running in pass-1 violates the span-shape
  constraint and trips the first-fire check.
- Drift mitigation: `crates/capco/tests/phase_assignment.rs`
  enumerates every registered rule's declared phase against a
  hand-maintained allowlist. Adding a new rule without considering
  phase forces an allowlist edit — a "stop and think" gate without
  the 31-line boilerplate cost of required declarations.

  **Test placement note (corrected post-review):** the test lives in
  `crates/capco/tests/` (NOT `crates/rules/tests/` as the earlier
  draft of this doc + the architect plan §1/§8 said). The placement
  is constitutionally required: `marque-rules` cannot depend on
  `marque-capco` (Constitution VII — `marque-rules` is upstream of
  every rule crate), so the allowlist of CAPCO rules can only live in
  a `marque-capco` integration test. The `post_3b_registration_pin.rs`
  test at the same path is the precedent.

**Implementation note**: Add a doc comment on the trait method
explaining that the default is intentional, not accidental. Cite this
decisions doc.

---

## D-7.3 `PhaseSpanShapeMismatch` — drop the EngineConstructionError variant; use first-fire path

**Decision**: **Remove** `EngineConstructionError::PhaseSpanShapeMismatch`
from PR 7 scope. Replace with a first-fire check inside the engine's
fix dispatch loop.

**Rationale**: `Engine::new` cannot enforce span shape exhaustively
because spans only materialize when a rule fires (architect §3.1).
A static span-shape declaration on each rule would just be a
comment-propagated invariant — the same drift class as the citation
correctness defect the murder board originally surfaced.

**Implementation shape** (in `Engine::fix_inner`'s pass-1 dispatch
loop):
- `debug_assert!(span_is_sub_token(fix.span, marking_scope))` when
  the firing rule is `Phase::Localized` (panics in debug builds; CI
  catches violations).
- `tracing::error!(rule_id, span, "Phase::Localized rule emitted
  out-of-shape span; dropping fix")` in release builds.
- Drop the offending fix from the pass-1 set; engine continues. The
  audit log records what actually applied; nothing claims a
  not-applied fix.

This is parallel to the existing `rule_panic_isolation.rs` pattern
(`crates/engine/tests/rule_panic_isolation.rs`).

**Task list rewording**: T075 should be rewritten in the PR 7a
description as "First-fire phase-span-shape check (FR-021), not
Engine::new registration check; debug-assert in debug, tracing::error
+ drop-fix in release."

---

## D-7.4 `R002_RULE_ID` encoding — 1-tuple now, document the migration

**Decision**: `pub const R002_RULE_ID: RuleId = RuleId::new("R002");`
in `crates/engine/src/engine.rs` adjacent to
`const DECODER_RULE_ID: &str = "R001"` at line 51.

**Critical clarification**: `DECODER_RULE_ID` is currently `&'static str`,
not `RuleId`. The mismatch is historical (predates the `RuleId`
newtype migration). **R002 should land as `RuleId::new("R002")` for
correctness**, and the implementer should note (in a code comment)
that `DECODER_RULE_ID` ought to follow but is out of scope.

**Reject**: encoding the sentinel scheme as a flat string prefix
(`"engine.r002.reparse-failed"`). Consumers would have to string-parse
the rule field, and PR 10's 2-tuple migration would have a flag-day
break for every consumer that matched on the string.

**Reject**: centralizing R001 + R002 into `marque-rules` during PR 7
(architect Regret #2). Plan §9.4 explicitly says this centralization
is "a separate refactor not in scope for this plan." Per
[`feedback_double_check_the_plan`](https://github.com/anthropic-org/claude-code-memory) memory: the plan supersedes for the refactor.
Defer to a follow-up.

**Document in code comment**: "When the (scheme, predicate-id)
2-tuple `RuleId` form lands (post-PR-10 freeze), this becomes
`RuleId::new(\"engine\", \"r002.reparse-failed\")` per FR-044.
`docs/refactor-006/legacy-rule-id-map.md` records the rename."

---

## D-7.5 `MessageArgs.contributing_rule_ids` — add the field

**Decision**: Add `pub contributing_rule_ids: SmallVec<[RuleId; 4]>`
to `MessageArgs` in `crates/rules/src/message.rs`.

**Resolves disagreement**: Rust reviewer (§6.2) recommended keeping
contributing IDs on an engine-internal `R002Diagnostic` struct and
surfacing via `tracing::warn!`. PM-overruled because:
- T077 explicitly requires R002 to **carry** the IDs in the
  diagnostic, not log them — `tracing` is observability, not audit.
- FR-024 names the contributing IDs as part of the R002 payload; an
  audit-visible payload must live on the `Diagnostic`, not a side
  channel.
- `RuleId` is on Constitution V's permitted-identifier list
  (enumerated identifier, not document bytes).
- `MessageArgs` is closed-set on **types** (no `String`, no
  `Vec<u8>`), not on field count. Adding a `SmallVec<[RuleId; 4]>`
  field preserves the closed-set property.

**Implementation note**: Update the closed-set compile-fail
doctests at `crates/rules/tests/message_args_closed_set.rs` (if
they exist) to add a positive assertion for `RuleId`-typed fields.
If they don't exist, the field is permitted by absence of
contradicting tests.

---

## D-7.6 Pass composition — C001 stays as pass-0

**Decision** (both reviewers agree): The fix pipeline becomes
**three-stage**:

```
[Pass-0: C001 text-corrections (UNCHANGED)]
   ↓ effective_source
[Pass-1: Phase::Localized rule fixes — single forward-pass splice]
   ↓ post_pass_1_buffer
[Re-parse]
   ↓ post_pass_1_attrs OR R002 (re-parse failed)
[Pass-2: Phase::WholeMarking rule fixes, with pre_pass_1_attrs cache]
   ↓ final output buffer
```

**Audit ordering preserved**: applied-fix records emit in the order
`c001_applied; pass1_applied; pass2_applied` (extending the existing
`all_applied = pass1_applied; all_applied.extend(applied)` pattern
at `crates/engine/src/engine.rs:1592`).

**Short-circuit when pass-1 is empty** (Rust reviewer §5.1): if
`pass1_applied.is_empty()`, skip the re-parse and run pass-2
against the existing `parsed_markings` (the post-C001 lint result).
This preserves the no-fix path's existing performance.

---

## D-7.7 `RuleContext` lifetime parameter

**Decision**: `RuleContext` gains a lifetime parameter:
`pub struct RuleContext<'a>`.

**Rationale**: `CanonicalAttrs` is **owned** (verified at
`crates/ism/src/canonical.rs:64` — no `<'src>` parameter). The
pre-pass-1 attrs cache lives in `fix_inner`'s stack frame as
`SmallVec<[CanonicalAttrs; 4]>`; `RuleContext` borrows into the cache
via `Option<&'a CanonicalAttrs>`.

**Trait change**: `Rule::check(&self, attrs: &CanonicalAttrs, ctx:
&RuleContext<'_>) -> Vec<Diagnostic<S>>`. The `'_` elision means
rule bodies need NO edit; only the trait method signature changes.

**Cache shape**: `SmallVec<[CanonicalAttrs; 4]>` (owned values).
**NOT** `Arc<CanonicalAttrs>` (would introduce shared state across
the trait boundary, violating Constitution VI). **NOT**
`Box<[CanonicalAttrs]>` (heap allocation on every fix_inner call
even when no pass-1 fixes apply).

**Cache scope**: populated only for markings whose span overlaps a
pass-1 fix span (R-4). Use binary search against the markings list
(span-sorted post-scanner) → O(K log N) where K = pass-1 fix count,
N = marking count. **NOT** O(N) (would waste work on every marking
on dense documents).

**Future evolution (deferred)**: Rust reviewer §8.3 notes that when
the parse cache adopts `Arc<CanonicalAttrs>` (likely with the v0.2
LMDB incremental cache), this becomes a refcount bump instead of a
value copy. **[Superseded note: the v0.2 LMDB cache was later descoped — constitution v1.8.0. An `Arc<CanonicalAttrs>` parse store could still land independently, but no longer "alongside the LMDB cache."]** Architect Regret #4 names the future `PreRewriteAttrs`
wrapper type. Both are deferred to a follow-up PR; PR 7 ships the
raw `Option<&'a CanonicalAttrs>` with a doc comment naming the
future evolution.

---

## D-7.8 `EX_R002_PARTIAL = 3` exit code

**Decision**: `EX_R002_PARTIAL = 3` in
`marque/src/main.rs:26-33`.

**Rationale (architect §3.8)**: Numerically adjacent to existing
diagnostic exit codes (`EX_DIAG_ERROR = 1`, `EX_DIAG_WARN = 2`);
distinct from sysexits-style range (64–78). Detectable without
NDJSON parsing (D1's binding constraint).

**BatchEngine worst-row-wins**: any row hitting R002 raises the
batch exit code to 3.

**Lands in**:
- `marque/src/main.rs` exit-code table.
- `contracts/engine-pipeline.md` new section "R002 surfacing
  semantics (consumer-surface contract)" per D1's "Lands in" clause.

---

## D-7.9 `fix_inner` extraction into `TwoPassFixer`

**Decision**: Extract a `TwoPassFixer` struct in
`crates/engine/src/engine.rs` (Rust reviewer §4.1).

**Rationale**: `fix_inner` is currently 335 lines (1309–1644). PR 7
adds ~200 lines (pre-pass-1 cache population, pass-2 dispatch with
I-18/I-19 logic, R002 emission, phase-split partition). Without
extraction `fix_inner` reaches ~535 lines — beyond the 50-line
function limit by 10×.

**Shape**:
```rust
struct TwoPassFixer<'engine> {
    engine: &'engine Engine,
    source: &'engine [u8],
    mode: FixMode,
    threshold: f32,
    deadline: Option<Instant>,
}

impl<'engine> TwoPassFixer<'engine> {
    fn run(self) -> Result<FixResult, EngineError> { ... }
    fn run_pass0_c001(&self, ...) -> Pass0Result { ... }
    fn run_pass1_localized(&self, pass0: &Pass0Result, ...) -> Pass1Result { ... }
    fn try_reparse(&self, pass1: &Pass1Result) -> ReparseOutcome { ... }
    fn populate_pre_pass_1_cache(&self, pass1: &Pass1Result, ...) -> SmallVec<...> { ... }
    fn run_pass2_whole_marking(&self, ...) -> Pass2Result { ... }
    fn build_r002_diagnostic(&self, ...) -> Diagnostic<CapcoScheme> { ... }
}
```

`fix_inner` becomes a 5-line trampoline. Each method is independently
testable — important for T084 (per-pass invariant tests).

---

## D-7.10 `FeatureId::PrecedingFixPenalty` — fills reserved slot, no schema bump

**Decision**: Add `FeatureId::PrecedingFixPenalty` variant in
`crates/rules/src/confidence.rs:202-224`. **Do NOT** bump
`MARQUE_AUDIT_SCHEMA` (FR-035 explicitly forbids PR 7 bumping the
schema — the slot is reserved within `marque-mvp-3`).

**Resolves Rust reviewer §6.1 concern**: the doc comment at
`crates/rules/src/confidence.rs:198` currently says "New variants
MUST bump the audit schema version." Update the doc comment to
reflect the reserved-slot exception:

```rust
/// Closed enumeration of features the decoder can record.
///
/// New variants ordinarily require a coordinated bump of
/// `MARQUE_AUDIT_SCHEMA` (in `crates/engine/build.rs`). PR 3c
/// reserved a slot for `PrecedingFixPenalty` in `marque-mvp-3`,
/// so adding that one variant in PR 7 does not require a schema
/// bump. ANY OTHER variant addition still does.
```

**Update test table**: `confidence.rs:304` (`KNOWN_FEATURES_FOR_TEST`)
must add `(FeatureId::PrecedingFixPenalty, "PrecedingFixPenalty")`.
The compiler's exhaustiveness check catches the `as_str` arm but not
the test table.

**E003 application**: When `ctx.pre_pass_1_attrs.is_some()` (i.e.,
the pass-2 marking's span overlaps a pass-1 fix), E003's confidence
reduces by the `PrecedingFixPenalty` contribution. Exact magnitude
is the rule author's choice — recommend `-0.10` based on the same
calibration as other corpus-derived penalties (verify against
`marque-priors-2` baseline; if uncertain, leave a follow-up issue
to recalibrate).

---

## D-7.11 Bench gate — both absolute and delta

**Decision**: `crates/engine/benches/fix_10kb.rs` (NOT
`benches/fix_10kb/`; the workspace pattern places benches inside
the crate that owns the code under test).

**Gates**:
1. Absolute: `p95 ≤ 16 ms` (FR-030, SC-001 hard budget).
2. Delta: `p99 ≤ pre_pr7_baseline.p99 * 1.05` (FR-033, consolidated
   plan §3.6 ">5% regression backs out the change").

**Baseline capture**: before landing PR 7c, run the bench on the
parent commit (`origin/staging`) and capture
`benches/baselines/2026-XX-pre-pr7.json`. Use the same hardware (D8
pinned GHA runner spec).

**Bench shape** (Rust reviewer §5.3): two functions in the same
group — `fix_10kb_pass2_only` (no pass-1 triggers) and
`fix_10kb_two_pass` (with pass-1 triggers). Separate benches make
regressions in either path independently visible.

---

## D-7.12 Forward-pass buffer construction for pass-1 fix application

**Decision** (Rust reviewer §6.3): Pass-1 fix application uses the
**forward-pass buffer construction** from `fix_inner`'s existing
`FixMode::Apply` block (lines 1495–1526), NOT `apply_text_corrections`'s
`Vec::splice` loop (line 1748).

**Rationale**: `splice` is O(N × M) — each splice shifts every byte
after the splice point. Pass-1 in PR 7 may produce more fixes per
document than C001 typically does (rule-emitted Phase::Localized
fixes vs. user-config corrections), so the latency penalty is real.

**Pattern**: pre-allocate with `extra` bytes; copy gaps and
replacements in a single left-to-right pass. The existing code at
`engine.rs:1495-1526` is the reference.

---

## D-7.13 Items DEFERRED to follow-up PRs

Documented here so the implementer does NOT pull them in opportunistically.

| Item | Why deferred | Where named |
|------|--------------|-------------|
| Centralize R001 + R002 const into `marque-rules` | Plan §9.4 explicitly: "separate refactor not in scope" | Architect Regret #2 |
| `Rule::phase_companion()` method for paired rules | No current rule needs both phases; architect's defer | Architect Regret #3 |
| `PreRewriteAttrs` named-type wrapper around `Option<&'a CanonicalAttrs>` | Mechanical refactor; defer to PR 8+ | Architect Regret #4 |
| `Arc<CanonicalAttrs>` parse-cache shape | Pre-supposes v0.2 LMDB cache; not yet built — **[Superseded: LMDB cache descoped, constitution v1.8.0]** | Rust reviewer §8.3 |
| (Scheme, predicate-id) `RuleId` 2-tuple migration | FR-049 freeze begins at PR 10 merge | D-7.4 above |
| `DECODER_RULE_ID` migration from `&'static str` to `RuleId::new()` | Out of scope; flagged in code comment | D-7.4 above |

---

## D-7.14 CHK015 / CHK018 gate clearance

PR 7b's reviewer attestation MUST clear:

- **CHK015 [GATE]**: "Is 'no `Phase::Both` escape hatch' stated as a
  hard rule (a rule needing both phases registers two entries)?"
  → Verify in the T074 implementation: no rule has both
  `Phase::Localized` and `Phase::WholeMarking` registrations sharing
  a struct.
- **CHK018 [GATE]**: "Is R002 message-template, span shape,
  audit-record shape, and exit-code contract specified per D1?"
  → Verify all four:
    1. `MessageTemplate::ReparseFailed` wired (D-7.5).
    2. R002 span shape: full post-pass-1 buffer or localized failure
       region — implementer's discretion based on what the parser
       can localize. Document choice in the R002 helper's doc comment.
    3. Audit-record shape: `MessageArgs.contributing_rule_ids:
       SmallVec<[RuleId; 4]>` (D-7.5).
    4. Exit code: `EX_R002_PARTIAL = 3` (D-7.8); WASM detection
       without NDJSON parsing (D1); BatchEngine worst-row-wins.

---

## Summary — what the implementation agent gets

The implementer should follow this priority order:

**7a (T073, T074):**
- Add `Phase` enum to `crates/rules/src/lib.rs` (next to `Severity`).
- Add `Rule::phase()` with `WholeMarking` default.
- All 31 `impl Rule` blocks declare phase explicitly.
- `crates/capco/tests/phase_assignment.rs` enumerates registered rules' phases against an audit-controlled allowlist (placement explained at D-7.2).

**7b (T076–T079, with T075 reworded as first-fire check):**
- Extract `TwoPassFixer` struct in `crates/engine/src/engine.rs`.
- Restructure `fix_inner` to 5-line trampoline + `TwoPassFixer::run`.
- Three-stage pipeline: pass-0 C001 (unchanged) → pass-1 Phase::Localized → re-parse → pass-2 Phase::WholeMarking.
- First-fire phase-span-shape check (debug_assert + tracing::error + drop-fix).
- `R002_RULE_ID = RuleId::new("R002")` const adjacent to `DECODER_RULE_ID`.
- `build_r002_diagnostic` helper (parallel to `build_decoder_diagnostic`).
- `MessageArgs.contributing_rule_ids: SmallVec<[RuleId; 4]>` field.
- `EX_R002_PARTIAL = 3` exit code; BatchEngine worst-row-wins.
- D1 consumer-surface obligations: `contracts/engine-pipeline.md` new section; WASM detection without NDJSON parsing.
- Short-circuit re-parse when pass-1 is empty.
- Forward-pass buffer construction for pass-1 fix application.

**7c (T080–T085):**
- `RuleContext<'a>` lifetime parameter; `pub pre_pass_1_attrs: Option<&'a CanonicalAttrs>`.
- Pre-pass-1 attrs cache `SmallVec<[CanonicalAttrs; 4]>` on `TwoPassFixer` stack.
- Cache scope: O(K log N) population — binary search against span-sorted markings.
- FR-023 disambiguation: same `RuleId` → no re-fire; different rule → fire.
- I-18 overlap demotion: pass-2 diagnostics overlapping pass-1 spans → `Severity::Suggest`.
- `FeatureId::PrecedingFixPenalty` variant + `as_str` arm + test table; E003 applies it when `pre_pass_1_attrs.is_some()`.
- Update `confidence.rs:198` doc comment.
- `two_pass_invariants.rs` property tests (FR-022, FR-023).
- `fix_invariants.rs` Layer-3 invariants (I-1, I-2, I-4, I-18, I-19).
- `fix_10kb.rs` Criterion bench + baseline capture; both absolute and delta gates.
- `scripts/bench-check.sh` updated.

**No schema bump.** No centralization of R001/R002. No `phase_companion`. No `PreRewriteAttrs`. These are documented as deferred.

---

## D-7.15 Exit-code precedence (post-7b-preflight extension of D-7.6 / D-7.12)

**Decision**: Exit-code aggregation in both `marque/src/main.rs` and the
batch CLI loop uses an **explicit precedence chain**, NOT numeric `max()`.
The precedence is:

```text
EX_R002_PARTIAL (3)  >  EX_DIAG_ERROR (1)  >  EX_DIAG_WARN (2)  >  EX_OK (0)
```

R002 wins over generic error. Rationale: R002 is the rare, distinguished,
action-changing signal — pass-2 was skipped because pass-1 made the buffer
unparseable. A consumer seeing `EX_DIAG_ERROR` thinks "diagnostics found,
normal exit"; a consumer seeing `EX_R002_PARTIAL` thinks "something unusual
happened, investigate." When both signals are present in the same document
or batch, the user needs the R002 signal because it changes workflow.

The 7b architect pre-flight and rust pre-flight disagreed on this point:

- Architect (`pr-7b-architect-preflight.md` §7): "check `r002_fired`
  BEFORE the `has_errors` / `has_warns` chain" — R002 wins.
- Rust reviewer (`pr-7b-rust-preflight.md` Q7): `EX_DIAG_ERROR >
  EX_R002_PARTIAL > EX_DIAG_WARN > EX_OK` — generic error wins.

PM resolution: the architect's precedence is correct for surface ergonomics
and the rust reviewer is correct that `max()` is the wrong operator.
Implement the rust reviewer's `merge_exit_code` shape (explicit `match`,
no `max()`) with the architect's precedence ordering (R002 first).

**Concrete shape:**

```rust
fn merge_exit_code(current: i32, new_code: i32) -> i32 {
    match (current, new_code) {
        (EX_R002_PARTIAL, _) | (_, EX_R002_PARTIAL) => EX_R002_PARTIAL,
        (EX_DIAG_ERROR, _) | (_, EX_DIAG_ERROR) => EX_DIAG_ERROR,
        (EX_DIAG_WARN, _) | (_, EX_DIAG_WARN) => EX_DIAG_WARN,
        _ => EX_OK,
    }
}
```

Per-document branch in `run_fix`: test `result.r002_fired` BEFORE
`has_errors` / `has_warns`. Batch CLI loop: fold per-row codes through
`merge_exit_code`. Document the precedence in a `///` doc comment so a
future reader does not "fix" it to numeric `max()`.

**Test**: `marque/tests/cli_exit_codes.rs` covers: (a) clean run → 0,
(b) warnings only → 2, (c) errors only → 1, (d) R002 only → 3, (e) R002
+ errors → 3 (R002 wins), (f) batch with rows producing each → 3.

**5-year-maintenance posture**: the explicit precedence chain is the
load-bearing piece; the order is the policy. If a future R003-class
signal lands, extending the chain is mechanical — adding it ahead of
or behind R002 is the policy question for that PR.

---

## D-7.16 First-fire span-shape check placement

**Decision**: The first-fire span-shape check filters fixes
**before** the FR-016 sort and **before** C-1 overlap dedup, not
inside the pass-1 dispatch loop.

Rationale (architect pre-flight §5): filtering early is cleaner —
dropped fixes never enter the sort or the dedup, and the rejection
is recorded in the audit stream as "0 fixes from this rule for this
marking" rather than "1 fix dropped late." The dispatch loop
(rust pre-flight Q10) sees only pre-filtered fixes.

Implementation: place the filter immediately after
`synthesize_fixes` returns, inside `TwoPassFixer::run_pass1_localized`.
The predicate `span_is_within_marking(inner, outer) :=
inner.start >= outer.start && inner.end <= outer.end` (rust
pre-flight Q10) is correct; endpoints inclusive on both sides
because a fix exactly matching a token's boundaries is still
sub-token-shape.

```rust
let pass1_fixes: Vec<SynthesizedFix> = synthesized
    .into_iter()
    .filter(|sf| {
        if !span_is_within_marking(sf.span, sf.marking_span) {
            tracing::error!(rule_id = %sf.rule, span = ?sf.span,
                marking_span = ?sf.marking_span,
                "Phase::Localized rule emitted non-sub-token span; dropping fix");
            debug_assert!(false,
                "Localized rule '{}' emitted span {:?} outside marking {:?}",
                sf.rule, sf.span, sf.marking_span);
            return false;
        }
        true
    })
    .collect();
```

Position in `run_pass1_localized`: between `synthesize_fixes` and
the FR-016 sort, before C-1 dedup.

---

## D-7.17 `MessageArgs` destructure-pin update is mandatory in 7b

**Decision**: PR 7b commit that adds `MessageArgs.contributing_rule_ids`
MUST update `crates/rules/tests/message_args_closed_set.rs`'s
exhaustive destructure pattern in the SAME commit.

Rationale (rust pre-flight Q5): the closed-set destructure pattern
is the safety net that catches drift toward "and here's what went
wrong" string fields. Failing to update it produces `E0027` at
build time; that is the intended behavior — the build break IS the
gate. The reviewer panel should verify the test was updated, not
worked around.

The new destructure arm asserts `contributing_rule_ids == SmallVec::new()`
for `MessageArgs::default()` and asserts the populated form for the
R002 case.

---

## D-7.18 `marque-mvp-3 → marque-1.0` audit-schema bump deferred to its own PR (PR 3c.2)

**Decision**: PR 7c does **NOT** bump the audit schema. The
`marque-mvp-3 → marque-1.0` cutover defers to a new dedicated PR
(PR 3c.2) that lands the full structural delta atomically. PR 7c
remains on `marque-mvp-3` throughout.

**Context** (2026-05-14): A misread of the FR-035 / consolidated-plan
sequencing initially placed the `marque-1.0` bump inside PR 7c's scope.
The PR 7c architect + Rust pre-flights independently surfaced a BLOCKING
contradiction: the `marque-1.0` envelope in `contracts/audit-record.md`
§1+ is structurally distinct from `marque-mvp-3`, not just a renamed
label. The four structural commitments — `Canonical<S>` provenance
wired into audit emit, BLAKE3 audit-record digesting, closed
`MessageTemplate` JSON serialization, `from_parsed_unchecked` adapter
deletion — are **all unshipped reserved slots** in the current codebase
(2026-05-14 inventory across four parallel Explore agents confirms this:
no `blake3` Cargo dependency, no digest fields in `AppliedFix`, no
`message` field in `AuditRecordJsonV3`, 27 surviving `from_parsed_unchecked`
call sites). A "label flip" `mvp-3 → 1.0` without the structural backing
would emit `"schema": "marque-1.0"` on records lacking the four
commitments the contract defines — Constitution V (audit-first compliance)
and Constitution VIII (authoritative source fidelity) both forbid this.

Three resolution paths were considered:

| Option | What it means | Trade-off | Adopted? |
|---|---|---|---|
| A. Label-flip only | Rename `mvp-3 → 1.0` in `build.rs` + doc sites | Emits the `1.0` label without structural backing — violates contract + Constitution V/VIII | No |
| B. Full structural bump in 7c | Land all 4 commitments in 7c | Massive scope expansion; doubles or triples 7c's size; mixes concerns | No |
| C. Defer to PR 3c.2 (adopted) | 7c stays on `mvp-3`; dedicated PR lands `marque-1.0` cutover atomically | 7c stays scoped; `1.0` lands honestly when its structural commitments do | **YES** |

**Implementation impact on PR 7c**: removes the audit-schema bump from
T080-T085 scope entirely. `MARQUE_AUDIT_SCHEMA` remains pinned at
`marque-mvp-3`; `AUDIT_SCHEMA_IS_V3` const stays as-is. The new
`FeatureId::PrecedingFixPenalty` variant added in 7c fills a reserved
slot in `marque-mvp-3` (per D-7.10 — confirmed by the audit) and does
not itself require a schema bump.

**Out of FR-035a's scope**: the `(scheme, predicate-id)` 2-tuple `RuleId`
form defers further still, to its own post-PR-10 PR per FR-049 (stability
freeze begins at PR 10 merge; the 2-tuple change requires the freeze to
be unfrozen). PR 3c.2 ships `marque-1.0` with the 1-tuple `RuleId` form
intact (`"rule": "E054"` string in audit records).

The deferral was preceded by a pre-deferral audit confirming no
earlier-landed work depends on the bump being done. Findings recorded
in `pr-7c-architect-preflight.md` §7 and four parallel inventory reports
on the inception date.

---

## D-7.19 `FeatureId::PrecedingFixPenalty` is engine-applied, not rule-applied

**Decision**: The `PrecedingFixPenalty` contribution is applied by the
engine at the pass-2 confidence-threshold gate, **not** inside a rule's
`evaluate` body. The penalty fires for every pass-2 diagnostic whose
marking was reshaped by a pass-1 fix (`RuleContext.pre_pass_1_attrs.is_some()`
is True). Magnitude: `-0.10` (confirmed from D-7.10's recommendation;
recalibration tracked in D-7.21).

**Rationale** (replaces the E003 portion of D-7.10): E003
(`MisorderedBlocksRule`) was retired in PR 3b.F → E060 — verified at
`crates/capco/src/rules.rs:143` comment. Both T082 and D-7.10's text
"E003 applies the penalty" are stale relative to current state. Wiring
the penalty to E003 would be a silent no-op.

The penalty is a cross-rule structural fact ("this marking was
reshaped by a previous pass") that doesn't belong inside any one rule's
`evaluate` body. The engine already has the data on hand at the
threshold gate where it decides whether to promote a pass-2
`FixProposal` to an `AppliedFix`. The engine-side application keeps
rules stateless (Constitution IV) and centralizes the policy in one
place (Constitution VI).

**Application site** (architect pre-flight §3.D recommendation): the
`PRECEDING_FIX_PENALTY_DELTA: f32 = -0.10` constant lives in
`crates/engine/src/engine.rs` (or a sibling module). When `TwoPassFixer`
dispatches pass-2 and a diagnostic's span matches a pass-1-reshaped
marking, the engine applies the intended rule-axis adjustment to the
`Confidence` struct and also appends
`FeatureContribution { id: FeatureId::PrecedingFixPenalty, delta: -0.10 }`
to `features`; the audit envelope already carries a
`features: SmallVec<[FeatureContribution; 4]>` field per `marque-mvp-3`.

**FeatureId variant addition**: still required. The variant fills a
reserved slot per D-7.10 and the inventory audit. The doc comment at
`crates/rules/src/confidence.rs:198` updates per D-7.10, with the E003
wording replaced by "engine applies the penalty at the pass-2 threshold
gate."

---

## D-7.20 `AUDIT_SCHEMA_IS_V3` const stays as-is in PR 7c

**Decision**: The `AUDIT_SCHEMA_IS_V3` const at `crates/engine/src/lib.rs:98`
is **not** renamed or retired in PR 7c. It stays as the
forward-compat detection sentinel for `marque-mvp-3`.

**Rationale**: The architect pre-flight initially flagged this const's
fate as an open question (whether to rename to `AUDIT_SCHEMA_IS_V1_0`,
retire entirely, or keep). With D-7.18 deferring the `marque-1.0` cutover
to PR 3c.2, that decision belongs to PR 3c.2's scope, not PR 7c's. The
const continues to serve its existing purpose (build-time sentinel that
gates the renderer's `marque-mvp-3` JSON shape) and PR 7c does not touch
it.

PR 3c.2 will either rename it to `AUDIT_SCHEMA_IS_V1_0` (paired with the
label flip) or introduce a parallel sentinel and retire `_V3` once the
bump completes. The architect pre-flight's recommendation propagates
forward to PR 3c.2's design phase; not actionable in 7c.

---

## D-7.21 `PrecedingFixPenalty` magnitude recalibration follow-up tracked

**Decision**: The `-0.10` magnitude from D-7.10 + D-7.19 is the
inception value. Recalibration against corpus data is tracked as a
follow-up item, not blocking on PR 7c's merge.

**Rationale**: D-7.10 noted: "Exact magnitude is the rule author's
choice — recommend `-0.10` based on the same calibration as other
corpus-derived penalties (verify against `marque-priors-2` baseline;
if uncertain, leave a follow-up issue to recalibrate)." The
verification against `marque-priors-2` requires corpus runs that
extend beyond PR 7c's scope. PR 7c lands the variant + the
engine-side application + the inception `-0.10` magnitude;
recalibration is a separate corpus-data exercise.

Follow-up: a GitHub issue opens at PR 7c merge documenting (a) the
inception magnitude, (b) the corpus signals that should be measured
to calibrate it, (c) the file:line where the constant lives. The
issue ships with the PR 7c PM addendum so it cannot be silently
forgotten.

---

## D-7.22 `PrecedingFixPenalty` mechanism retired

**Decision** (2026-05-14, PM clarification): Retire the `PrecedingFixPenalty`
mechanism entirely. Remove the `FeatureId::PrecedingFixPenalty` variant, the
engine-applied multiplicative `rule` reduction, the `FeatureContribution` audit
trace, the `PRECEDING_FIX_PENALTY_DELTA = -0.10` constant, and the watchdog
test suite (`crates/engine/tests/preceding_fix_penalty.rs`).

**Supersedes** D-7.10 (`FeatureId::PrecedingFixPenalty` reservation),
D-7.19 (engine-applied penalty at pass-2 threshold gate),
D-7.21 (penalty magnitude recalibration follow-up).

**Rationale**:

The mechanism was misunderstanding-derived. The user's original concern — raised
in an earlier conversation that didn't survive into the active spec/plan
documents — was decoder-specific: when the decoder makes multiple corpus-inferred
changes on top of each other, the iterated posterior accumulation can create
a confidence-loop pathology. That concern is legitimate and remains open.
But it was captured in plan/spec docs as a generalized cross-pass penalty
mechanism, which is not what the user intended and has no evidence basis. The
remediation phase of PR 7c also independently confirmed the path is unreachable
under current real CAPCO inputs (all four `Phase::Localized` rules — C001, E006,
E007, S004 — emit via `Diagnostic::text_correction` through pass-0, not pass-1's
rule channel; `pass1.applied` is always empty in production; the cache never
populates; the penalty is dead code).

Two corroborating signals → no evidence basis + dead code → retire.

**What stays**:

- `RuleContext<'a>` lifetime parameter + `pub pre_pass_1_attrs: Option<&'a CanonicalAttrs>` field
- Pre-pass-1 attrs cache on `TwoPassFixer`
- FR-023 disambiguation (engine-side `pass1_applied_keys` lookup)
- I-18 overlap demotion (pass-2 diagnostics overlapping pass-1 spans → `Severity::Suggest`)

These are load-bearing for the two-pass model independent of the penalty.
The `pre_pass_1_attrs` field is the architectural signal "this marking was
reshaped by pass-1"; no current rule consumes it, but the lifetime parameter
has been threaded through every rule's `check` signature and removing it would
be viral churn for no gain.

**Open research item** (NOT scoped to any current PR):

The user's original decoder confidence-loop concern remains unaddressed.
A concrete statistical framing is the right approach — candidate framings
include KL-divergence-bounded posterior accumulation, a Bayesian-update floor
on iterated decoder applications, or a depth-limited recursion guard. This is
a separate research item; when a design lands, it will likely live in
`marque-engine`'s decoder path (`engine.rs::DecoderRecognizer`) rather than
as a generalized rule-context mechanism.

**Audit-schema implication**: the originally-planned `PrecedingFixPenalty` slot
reservation in `marque-mvp-3` was prose-only (in spec / plan / CLAUDE.md), not
a structural commitment in the audit envelope. No audit record has ever emitted
`PrecedingFixPenalty`. Removing the (closed) `FeatureId` variant is safe by
construction — no consumer breaks. No schema bump required.

---
