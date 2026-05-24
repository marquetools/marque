<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7 — Phase-Tagged Pass Split: Architect Plan

> Pre-flight architecture review for tasks T073–T085 (spec
> `specs/006-engine-rule-refactor`). Companion task IDs are referenced
> by number. Quality bar: "will we want to maintain this in 5 years?"

## 0. Executive Summary

PR 7 introduces a structural change to `Engine::fix_inner`: rules are
partitioned into `Phase::Localized` (sub-token rewrites, e.g. `OC →
ORCON`) and `Phase::WholeMarking` (rules that read full marking state,
e.g. E003 ordering, E007 USA-first), with a re-parse boundary between
the two passes. Three new invariants emerge:

- **I-18 (span non-overlap):** pass-2 promoted fix spans MUST NOT
  overlap pass-1 promoted fix spans (FR-022). Overlap-demote pass-2
  diagnostics to `Severity::Suggest`.
- **I-19 (reshape-aware re-validation):** a pass-1 fix that reshapes a
  marking MUST NOT cause pass-2 to re-fire on a defect that was already
  resolved (FR-023). Engine caches pre-pass-1 attrs and threads them
  through `RuleContext`.
- **R002 partial-progress contract:** if `parse(post_pass_1_buffer)`
  fails, retain pass-1 audit records, emit `R002`, return the pass-1
  buffer, do not run pass-2 (FR-024).

The change is structurally additive (no schema bump — slots reserved in
PR 3c) but composes with the **existing two-pass-for-C001** logic
already in `fix_inner` (lines 1338–1361). The first high-stakes
decision below is how that composes: C001 pass becomes a *third* pre-
pass, not a flag on pass-1.

Recommended sequencing: ship as **three sub-PRs (7a / 7b / 7c)** behind
one umbrella issue. Justification in §1.

---

## 1. Sub-PR sequence

I recommend **splitting PR 7 into three sub-PRs**, mirroring the PR 3b
umbrella precedent (six sub-moves under one tracking issue). Two of
the three sub-PRs are independently revertable; the third is the load-
bearing one.

### 7a — `Phase` plumbing + registration check (T073, T074, T075)
- Add `enum Phase { Localized, WholeMarking }` to `marque-rules`.
- Add `Rule::phase()` method (default `WholeMarking`; rationale below
  in §3.2). All 31 `impl Rule` blocks in `crates/capco/` declare their
  phase explicitly — no rule relies on the default in this PR; the
  default exists only for forward-compatibility with future schemes
  whose rules might be `WholeMarking`-by-construction.
- Add `EngineConstructionError::PhaseSpanShapeMismatch` to
  `crates/engine/src/errors.rs`.
- Engine registration-time validation: walk the rule set, classify
  each rule's `phase()`, and stash the partition in `Engine` state.
  The span-shape constraint (FR-021: `Phase::Localized` span is
  sub-token-only) is a registration-time assertion that we cannot
  exhaustively check at `Engine::new` (the span only materializes when
  a rule fires) — see §3.1 for the resolution. **This sub-PR does not
  change `fix_inner` behavior**: both phases run together in pass-2
  exactly as today. The partition is read but not dispatched on.
- Tests: `crates/capco/tests/phase_assignment.rs` — every `impl Rule`
  in the capco rule set has a declared `Phase`. **Placement correction**:
  the original `crates/rules/tests/` location would create a circular
  dependency (`marque-rules` is upstream of `marque-capco` per
  Constitution VII), so the allowlist of CAPCO rules lives in a
  `marque-capco` integration test instead. See `post_3b_registration_pin.rs`
  for the same pattern.
- **Revertable**: yes, cleanly. `Phase::Localized` is unread by the
  engine.

### 7b — Two-pass restructure + R002 (T076, T077, T078, T079)
- Restructure `Engine::fix_inner` into the genuine two-pass dispatch.
- Define `R002_RULE_ID` const in `crates/engine/src/engine.rs`
  alongside `DECODER_RULE_ID` (line 51).
- Wire `MessageTemplate::ReparseFailed` (already reserved at
  `crates/rules/src/message.rs:154`).
- Add `R002Diagnostic` minting helper in
  `crates/engine/src/engine.rs`, mirroring `build_decoder_diagnostic`
  (line ~2145).
- Implement R002 surface obligations per D1: `EX_R002_PARTIAL = 3` in
  `marque/src/main.rs` exit-code table.
- **Revertable**: in principle yes, but this is the load-bearing
  surgical commit. Reverting reverts the pass-split semantics; future
  PRs that depend on the I-18/I-19 invariants would have to be
  reverted too.

### 7c — Pre-pass-1 attrs cache + `PrecedingFixPenalty` + benches +
property tests (T080, T081, T082, T083, T084, T085)
- Add `RuleContext.pre_pass_1_attrs: Option<&'a CanonicalAttrs>` (see
  §3.3 — no `<'src>` parameter on `CanonicalAttrs`, the lifetime is
  the borrow's, not the bytes').
- Populate cache and dispatch `Phase::WholeMarking` rules against
  post-pass-1 attrs with the pre-pass cache attached.
- Implement FR-023 disambiguation in dispatch.
- Add `FeatureId::PrecedingFixPenalty` to `marque-rules` and apply to
  E003.
- Property tests (`crates/engine/tests/two_pass_invariants.rs`) and
  invariant tests (`crates/engine/tests/fix_invariants.rs`).
- New bench `benches/fix_10kb/` (or `crates/engine/benches/fix_10kb.rs`
  — see §6 below).

### Why three sub-PRs, not monolithic
- **Review surface area**: 7c is the bulk of the conceptual novelty
  (cache lifetime, disambiguation). Splitting lets a reviewer
  understand pass-split semantics from 7b in isolation before
  approving the cache shape.
- **Revertability under FR-049 (US8)**: an issue surfaced in 7c (e.g.,
  the cache costs more than the bench gate allows) can be reverted
  without losing the registration check or R002 surface.
- **CI surface**: 7a is a near-pure-additive type change. Running CI
  on a green 7a establishes that the new `phase()` method doesn't
  break the existing rule set. 7b's CI green confirms re-parse path
  works *before* we conditionally cache attrs.

Trade-off accepted: three CI runs instead of one; three review cycles.
Worth it on a load-bearing change to the engine hot path.

---

## 2. File-Level Change Inventory

### T073 — add `Phase` enum and `Rule::phase()`
- `crates/rules/src/lib.rs` (Rule trait — line 962): add `phase()`
  method.
- New module: either a tiny module in `lib.rs` (3 lines) or a separate
  `crates/rules/src/phase.rs`. **Recommendation:** keep in `lib.rs`
  at module level (next to `RuleId` / `Severity`) — `Phase` is part of
  the trait surface, not a feature; a separate file overstates the
  type's footprint. See §3.1 below.

### T074 — declare `Phase` per rule
- `crates/capco/src/rules.rs` — every `impl Rule for X` block (≈20
  rules in this file).
- `crates/capco/src/rules_declarative.rs` — walker rules (≈6 entries).
- `crates/capco/src/rules_sci_per_system.rs` — already-collapsed.
  Walker rule is `WholeMarking`.
- `crates/capco/src/rules_banner_match.rs` (or wherever
  `BannerMatchesProjectedRule` lives post-PR-3b.A) —
  `Phase::WholeMarking`.
- A rule needing both phases (rare; FR-021 explicitly forbids
  `Phase::Both`) **registers two entries with separate `RuleId`s**.
  As of today, no CAPCO rule needs both. Document this in the PR
  description if a rule looks like it might, so the reviewer can
  challenge the design.

### T075 — `EngineConstructionError::PhaseSpanShapeMismatch`
- `crates/engine/src/errors.rs` — add variant alongside
  `RewriteCycle`, `UnannotatedCustomAxes` (line 53).
- Exit code: `EX_UNAVAILABLE` (69) — same as other developer / rule-
  author errors. See §3.6 for shape.
- **Registration-time vs first-fire check**: a span check cannot be
  static (rules emit dynamic spans). `Engine::new` cannot enforce the
  shape exhaustively. We have two options; see §3.1.

### T076 — `fix_inner` restructure
- `crates/engine/src/engine.rs` — current `fix_inner` (line 1309) is
  ≈340 lines. The restructure adds pass-1 dispatch + re-parse + pass-2
  dispatch. The C001 corrections-map two-pass (lines 1338–1361) is
  preserved as a **pre-pass 0** (see §3.5).

### T077 — re-parse-failure path
- `crates/engine/src/engine.rs` — new function
  `build_r002_diagnostic` adjacent to `build_decoder_diagnostic`
  (~line 2145).
- Return type stays `Result<FixResult, EngineError>` — R002 is NOT an
  `EngineError`. R002 is a successful return with diagnostics
  attached. The buffer returned IS the pass-1 buffer. See §3.7.

### T078 — `R002_RULE_ID` const
- `crates/engine/src/engine.rs` near line 51 alongside
  `DECODER_RULE_ID`.
- Encoding decision: see §3.4.

### T079 — `MessageTemplate::ReparseFailed` wiring
- `crates/rules/src/message.rs` — variant already exists at line 154.
- `MessageArgs` — review needed: per the current shape (line 380), the
  args struct has `token`, `category`, `span`, `digest`, `confidence`,
  `expected_token`, `actual_token`, `feature_ids`. For R002, we need
  the *contributing pass-1 fix rule IDs*. Options:
  - (a) reuse `feature_ids: SmallVec<[FeatureId; 4]>` — store rule
    identifiers as `FeatureId` variants. Inappropriate — `FeatureId`
    is decoder-feature-space, not rule-space.
  - (b) add a new field `contributing_rule_ids: SmallVec<[RuleId; 4]>`
    to `MessageArgs`. **Recommended.** `RuleId` is already on
    Constitution V's permitted-identifier list (it's an enumerated
    identifier).
- Compile-fail doctests on `MessageArgs` enforcing no `String` / no
  `&str` need updating? No — they enforce inadmissible field shapes,
  not that the existing set is closed. Adding a `RuleId` field is
  permitted.

### T080 — pre-pass-1 attrs cache
- `crates/rules/src/lib.rs` — `RuleContext` struct (line 225): add
  `pub pre_pass_1_attrs: Option<&'a CanonicalAttrs>`. Lifetime
  decision in §3.3.
- `crates/engine/src/engine.rs::fix_inner` — populate the cache.

### T081 — pass-2 dispatch
- `crates/engine/src/engine.rs::fix_inner` — the core change.
- Reuses `lint_with_options_internal` to dispatch pass-2 rules against
  post-pass-1 parsed markings.

### T082 — `FeatureId::PrecedingFixPenalty`
- `crates/rules/src/confidence.rs` — add variant at line 224 (end of
  enum), update `as_str` arm (line 250).
- `confidence.rs:198` doc comment — update: "New variants MUST bump
  the audit schema version" is no longer accurate; the slot was
  reserved. Add a note: "PrecedingFixPenalty filled a slot reserved
  at PR 3c.B; adding further variants requires a schema bump."
- Apply in `crates/capco/src/rules.rs` for E003 confidence reduction
  when `RuleContext.pre_pass_1_attrs.is_some()` (i.e., the pass-2
  marking sits behind a pass-1 fix).

### T083 — property tests
- `crates/engine/tests/two_pass_invariants.rs` (new file). Uses
  `proptest`. Already a dev-dep? Let me check — `proptest_engine.rs`
  exists in the test directory, so yes.

### T084 — fix-invariants tests
- `crates/engine/tests/fix_invariants.rs` (new file).
- Covers Layer-3 invariants I-1, I-2, I-4, I-18, I-19. Consult
  consolidated plan §6 for I-1/I-2/I-4 definitions when implementing.

### T085 — `fix_10kb` Criterion bench
- `crates/engine/benches/fix_10kb.rs` (matches `lint_latency.rs` /
  `fix_latency.rs` co-located pattern). The task line names
  `benches/fix_10kb/` — the workspace pattern places benches inside
  the crate that owns the code under test, so prefer
  `crates/engine/benches/fix_10kb.rs`. Update `scripts/bench-check.sh`
  to gate on the new bench's p95 ≤ 16 ms (FR-030 / FR-032).

---

## 3. Interface Decisions (the high-stakes ones)

### 3.1 `Phase` enum location and registration enforcement

**Decision:** Put `Phase` in `crates/rules/src/lib.rs` (the load-
bearing trait surface). It is two variants and is part of every
`Rule` impl's contract.

**Registration check (T075):** The check is structurally impossible at
`Engine::new` because no rule has fired yet. Two viable resolutions:

- **(a) First-fire instrumentation** (recommended). The first time a
  `Phase::Localized` rule emits a diagnostic with a span wider than a
  single token, panic in debug mode and emit a `tracing::error!` +
  return a degenerate empty fix in release mode. This is the same
  shape as the existing "rule panic isolation" path
  (`crates/engine/tests/rule_panic_isolation.rs`). Add a property test
  that asserts all currently-registered `Phase::Localized` rules emit
  sub-token spans on the corpus regression set.
- **(b) Per-rule declared span shape**. Each rule declares
  `fn span_shape() -> SpanShape { SubToken | WholeMarking }` as part
  of the trait. Engine checks `shape() == phase().expected_shape()`
  at registration. **Rejected**: doubles the per-rule annotation
  burden for no clearer guarantee — span shape can still drift at
  runtime; the declared shape is just another comment-propagated
  invariant.

**Resolved:** Use (a). The `PhaseSpanShapeMismatch` error variant
fires at runtime on a violation, not at registration. Add a debug-
mode assert + a tracing::error + return an empty fix vector in release
mode (we never want the engine to panic in prod). Update T075 task
wording in the PR description: "registration check" → "first-fire
check".

### 3.2 `Rule::phase()` — required vs default-to-`WholeMarking`

**Decision:** Default to `WholeMarking`. Justification:

- Most rules in the catalog are whole-marking already (27 of 31; 4 are Localized — C001, E006, E007, S004).
- A rule that *fails to think about* phase will be `WholeMarking`,
  which is the safer default — pass-2 runs against post-pass-1 attrs,
  so the rule sees the same input shape as today.
- The drift risk (rule author adds a new localized rule but forgets to
  declare `Phase::Localized`) is caught by the corpus regression
  harness: a localized rule running in pass-2 would see post-pass-1
  attrs, not pre-pass-1. The test fixtures that exercised the rule's
  defect class would fail.

**Counterargument considered & rejected:** "all rules must declare to
prevent silent default" — costs ~31 trivial annotations once; the rule
author who needs to think about phase will be reviewing the rule
anyway. Better to spend reviewer attention on the 3 `Phase::Localized`
rules.

**Trade-off written in code comment** on the trait method so the
default is intentional, not accidental.

### 3.3 `RuleContext.pre_pass_1_attrs` lifetime

**Critical observation:** `CanonicalAttrs` is **owned**
(`crates/ism/src/canonical.rs:64`, no `<'src>` lifetime parameter).
The plan's hint `Option<&CanonicalAttrs<'src>>` is slightly
misformulated. The actual signature is:

```rust
pub pre_pass_1_attrs: Option<&'a CanonicalAttrs>
```

Where `'a` is `RuleContext`'s implicit lifetime. But `RuleContext`
today does NOT have a lifetime parameter (line 225 — `pub struct
RuleContext`). Three options:

- **(a)** Add `RuleContext<'a>` with a lifetime parameter. This
  ripples to every rule's `fn check(&self, attrs: &CanonicalAttrs,
  ctx: &RuleContext)` signature → `fn check(&self, attrs:
  &CanonicalAttrs, ctx: &RuleContext<'a>)`. Every rule body needs no
  edit (the lifetime is elided at call sites). The trait signature
  needs the lifetime parameter.
- **(b)** Store the cache as `Option<Arc<CanonicalAttrs>>` inside
  `RuleContext`. Adds one refcount bump per rule invocation per
  cached marking; eliminates the lifetime parameter. The cache lives
  for the duration of the entire `fix_inner` call regardless of when
  pass-2 finishes.
- **(c)** Store the cache as `Option<CanonicalAttrs>` (owned clone)
  inside `RuleContext`. Each pass-2 rule invocation clones the
  cached attrs. `CanonicalAttrs` has `Box<[T]>` fields; clone is
  O(token count). Wasteful on the hot path.

**Recommendation: (a)**. Adding a lifetime parameter to `RuleContext`
is a structurally correct surface change. The cost is one `<'a>`
annotation on the `Rule::check` trait method; rule bodies are
unchanged. (b) defeats the I-2 / Constitution VI invariant ("per-
invocation scratch allocations are allowed; hidden global state is
not") by introducing an Arc-shared cache that survives the trait
boundary; (c) costs an unnecessary clone.

**Lifetime arithmetic for the cache:**
- Pre-pass-1 attrs were parsed from the ORIGINAL `source: &[u8]`
  buffer. Their `token_spans: Box<[TokenSpan]>` field carries spans
  into that buffer.
- Pass-2 dispatch runs against `post_pass_1_buffer: &[u8]`. Its
  parsed attrs reference the new buffer.
- The pre-pass-1 attrs cache `SmallVec<[CanonicalAttrs; 4]>` is owned
  by the `fix_inner` stack frame. Both buffers (original + post-pass-
  1) outlive the cache because they are local to the function.
- `RuleContext<'a>` borrows the cached attrs through `&'a
  CanonicalAttrs`. The lifetime is "the duration of the pass-2 rule
  loop." This is straightforward; no lifetime gymnastics required
  because `CanonicalAttrs` is owned (the `token_spans` `Box<[T]>` is
  internal storage with no outer lifetime).

**Cache population:** populate per-marking only when the marking's
span overlaps a pass-1 fix span (R-4). The default case (most
markings don't overlap pass-1 fixes) keeps the cache small and the
overhead minimal. Use `SmallVec<[CanonicalAttrs; 4]>` for the inline-
4 capacity.

### 3.4 `R002_RULE_ID` encoding — 1-tuple string or 2-tuple

**Current state:** `RuleId` is `pub struct RuleId(&'static str)`,
constructed with `RuleId::new("R001")`. The (scheme, predicate-id)
2-tuple form from FR-044 and FR-026 is reserved by spec but has NOT
landed in PR 3c. FR-049 says the rule-ID stability freeze begins at
PR 10 merge; renames during PR 4–10 are permitted.

**Decision:** Use `RuleId::new("R002")` exactly as `DECODER_RULE_ID`
uses `RuleId::new("R001")`. The plan §9.4 specifies the future
2-tuple form `("engine", "r002.reparse-failed")` but the 2-tuple
shape on `RuleId` doesn't exist yet, and inventing it just for R002
would be a non-trivial cross-cutting change.

**Trade-off:** PR 10 will rename `R001` and `R002` together when the
2-tuple form lands. `docs/refactor-006/legacy-rule-id-map.md` (FR-049)
will record the rename.

**Alternative considered & rejected:** Encode the sentinel scheme as
a prefix string, e.g. `RuleId::new("engine.r002.reparse-failed")`.
This would carry the sentinel scheme through the audit emitter, but
NDJSON consumers would have to string-parse the rule field to detect
synthetic engine diagnostics. Cleaner to wait for the 2-tuple form.

**Document the future shape** in the const's doc comment so the
follow-up PR knows what to rename to:

```rust
/// R002 sentinel rule ID. Once the (scheme, predicate-id) 2-tuple
/// form lands (post-PR 7), this becomes ("engine", "r002.reparse-
/// failed") per FR-044.
const R002_RULE_ID: &str = "R002";
```

### 3.5 How C001 text-corrections compose with `Phase::Localized`

**Current `fix_inner` structure (lines 1338–1361):**
1. Lint original source.
2. If C001 corrections-map diagnostics exist with confidence ≥
   threshold, apply them to produce `effective_source`.
3. Re-lint `effective_source` (yielding `lint`, `parsed_markings`).
4. Synthesize fixes for all diagnostics in `lint`.
5. Single-pass forward splice.

**PR 7 introduces:**
1. Lint original source.
2. **(unchanged)** C001 text-correction apply → `effective_source`.
3. **(unchanged)** Re-lint `effective_source`.
4. **NEW:** Partition synthesized fixes into pass-1 (`Phase::Localized`)
   and pass-2 (`Phase::WholeMarking`).
5. Forward-splice **only pass-1 fixes** to produce
   `post_pass_1_buffer`.
6. Re-parse `post_pass_1_buffer`. On parse failure → R002 path.
7. Populate pre-pass-1 attrs cache.
8. Dispatch pass-2 rules against post-pass-1 parsed markings, with
   `pre_pass_1_attrs` populated for overlapping markings.
9. Pass-2 diagnostics overlapping pass-1 spans → demote to `Suggest`.
10. Forward-splice pass-2 fixes onto `post_pass_1_buffer` → final
    output.

**Decision:** C001 stays a third pre-pass (pass-0), as today. It runs
*before* the lint that produces phase-tagged diagnostics. **Reasoning:**

- C001 is a byte-level pre-scanner rewrite (`SERCET → SECRET`). The
  scanner never sees the malformed bytes; from the scanner's
  perspective, C001 happens "before parse."
- Folding C001 into pass-1 would conflate corpus-typo correction with
  rule-emission. The audit emitter and the renderer-canonical bridge
  treat the two paths differently
  (`AppliedFixProposal::TextCorrection` vs
  `AppliedFixProposal::FixIntent`).
- The lift-shift cost is minimal: `Engine::fix_inner` already has the
  shape; we add pass-1/pass-2 dispatch after the existing C001 logic.

**Audit ordering preserved:** the existing `all_applied = pass1_applied;
all_applied.extend(applied);` at line 1592 means C001 corrections
appear first in the audit log. Post-refactor: `all_applied = c001_applied;
all_applied.extend(pass1_applied); all_applied.extend(pass2_applied);`

### 3.6 `EngineConstructionError::PhaseSpanShapeMismatch` shape

Variant shape:

```rust
PhaseSpanShapeMismatch {
    rule_id: RuleId,
    declared_phase: Phase,
    /// The span the rule emitted that violated the declared phase's
    /// span-shape constraint.
    emitted_span: Span,
    /// The marking-scope span (typically the containing portion or
    /// banner) for context — pulled from `candidate_span` or the
    /// rule's `RuleContext.candidate_span`.
    scope_span: Span,
},
```

**Exit code:** `EX_UNAVAILABLE` (69) — same as
`UnannotatedCustomAxes`. This is a rule-author defect, not a user-
config defect.

**Fired by:** the first-fire check (§3.1), routed through `Engine`
state. Note that `EngineConstructionError` is fired only by
`Engine::new` today — so technically PhaseSpanShapeMismatch belongs
on a different error type. Two options:
- (a) Add to `EngineError` (the runtime error type, line 230+).
  Requires the variant to be returnable from `lint` and `fix`.
- (b) Promote first-fire to a `tracing::error!` + degenerate fix
  (drop the offending fix; do not return an error).

**Recommendation: (b)** — engine resilience. A panic-isolating engine
exists already (`rule_panic_isolation.rs`); a span-shape mismatch is a
similar developer defect that should not take down a fix run. The
debug-mode `debug_assert!` catches it in CI; the prod-mode
degenerate-fix-drop keeps the engine running. Remove
`EngineConstructionError::PhaseSpanShapeMismatch` from the task
scope; replace with `tracing::error!` + drop-fix logic. **Re-word
T075 in the PR description.**

### 3.7 `R002Diagnostic` shape

R002 is a `Diagnostic<CapcoScheme>` value, NOT a separate type. The
existing `Diagnostic` shape (line 716) accommodates it. The shape:

```rust
Diagnostic {
    rule: RuleId::new("R002"),
    severity: Severity::Error,  // re-parse failure is always Error
    span: failure_span,          // where the re-parse failed; defaults to whole post-pass-1 buffer if the parser can't localize
    candidate_span: None,
    message: <rendered MessageTemplate::ReparseFailed>,
    citation: "engine-synthetic",  // see decoder citation pattern, line 60
    fix: None,                   // R002 carries no fix
    text_correction: None,
}
```

The contributing pass-1 fix IDs live in `MessageArgs.contributing_rule_ids`
(new field per §2's T079 entry).

**Audit-record integrity (Constitution V):** R002 is a *diagnostic*,
not an `AppliedFix`. It does not promote through
`AppliedFix::__engine_promote`. The pass-1 fixes that contributed
DO promote (they applied successfully); R002 surfaces alongside them
in the diagnostic stream. The G13 invariant (no document content) is
satisfied trivially because R002's args carry only `RuleId`s and a
`Span`.

### 3.8 `EX_R002_PARTIAL` exit code numeric value

Current exit codes (`marque/src/main.rs:26-33`):
- `EX_OK = 0`
- `EX_DIAG_ERROR = 1`
- `EX_DIAG_WARN = 2`
- `EX_USAGE = 64`
- `EX_DATAERR = 65`
- `EX_UNAVAILABLE = 69`
- `EX_IOERR = 74`
- `EX_TEMPFAIL = 75`

**Recommendation: `EX_R002_PARTIAL = 3`.**

Rationale: R002 represents *partial success* — the document is
modified, the audit log is non-empty, but the engine could not
complete the full fix pipeline. Numerically adjacent to
`EX_DIAG_ERROR` and `EX_DIAG_WARN` is the right shape — it conveys
"the diagnostic stream is non-trivial" without colliding with
sysexits-style errors (64–78).

Update D1 wording: D1 says "distinct from `EX_DIAG_WARN` and from
regular fix-failure" — `3` satisfies both. Update
`contracts/engine-pipeline.md` per D1's "Lands in" clause.

**Worst-row-wins for BatchEngine (D1):** the batch driver tracks the
maximum exit code across rows; R002 in any row raises the batch exit
code to 3. Implementation lives in
`crates/engine/src/batch.rs` (existing batch driver — verify
location during implementation).

---

## 4. Risks and PM Decision Points

These need surfacing to the PM before the implementation agent starts
coding.

### 4.1 Performance risk: re-parse cost

**Concern:** pass-1 → re-parse → pass-2 adds an O(source_len) parse
to every `fix_inner` call, even when no pass-1 fixes apply.

**Mitigation:** **Skip the re-parse when pass-1 fixes is empty.** If
no `Phase::Localized` rule produced an applied fix, `post_pass_1_buffer
== effective_source` and the existing `parsed_markings` is still
valid. Use the same predicate pattern already in `fix_inner` for the
C001 skip (line 1356: `if !pass1_applied.is_empty()`).

**Bench target (FR-032):** `fix_10kb` p95 ≤ 16 ms total. Pre-refactor
baseline for `fix_throughput` is captured in `benches/baseline.json`
(p99 1346 µs per the CLAUDE.md entry). Two-pass re-parse on a 10 KB
input is ~1 ms; the budget has ~15 ms of headroom. **Safe by a wide
margin** unless the cache itself becomes hot.

**Decision needed from PM:** is the bench gate FR-030 (p95 ≤ 16 ms on
10 KB single-portion) or a delta gate (post-PR-7 ≤ pre-PR-7 + X%)?
The consolidated plan §3.6 says ">5% mean OR p99 regression backs out
the change" — that's the delta gate. PR 7 should comply with both:
absolute ≤ 16 ms AND delta ≤ 5% on `fix_10kb`. **The implementer
should capture both baselines on the same hardware (the GHA runner
per D8) and compare.**

### 4.2 Lifetime channel concern (resolved)

The plan hint `pre_pass_1_attrs: Option<&CanonicalAttrs<'src>>` is
misformulated — `CanonicalAttrs` is owned, no `<'src>` parameter.
See §3.3 for the resolution: use `Option<&'a CanonicalAttrs>` on
`RuleContext<'a>`. **No PM input needed; documented for the
implementer.**

### 4.3 C001 composability

See §3.5. **Resolved**: C001 is pass-0, runs as today, unchanged.
No PM decision needed.

### 4.4 `Severity::Suggest` cleanliness

**Concern raised in the prompt:** "a single rule's diagnostic can be
emitted at `Suggest` for one marking and `Error` for another in the
same lint result." Is this surface clean?

**Assessment:** yes, this is the existing semantic and PR 7 does not
introduce it. The lint-time `Suggest` demotion path (low-confidence
proposals) ALREADY produces the same surface — a rule emitting at
`Severity::Error` whose `confidence.combined() < threshold` is
rewritten to `Severity::Suggest` per the doc comment at
`crates/rules/src/lib.rs:176-181`.

PR 7 adds a third path producing `Suggest`: the overlap demotion
(FR-022). The surface is the same:

- NDJSON: each diagnostic carries its own `severity` field, lowercase
  string `"suggest"`. Consumers iterate diagnostics, not rules.
- Audit log: `AppliedFix` only records promoted fixes; demoted
  `Suggest` diagnostics do not promote. The audit log is clean.
- CLI rendering: the renderer (e.g.
  `marque/src/output_human.rs`) groups diagnostics by severity, not
  by rule. Output is coherent.

**No PM input needed.** Document in the PR description so a reviewer
who hasn't seen `Suggest` before doesn't get confused.

### 4.5 FR-023 disambiguation under future PR 4–10 rule-ID renames

CHK017 raises this: "same `RuleId` → do not re-fire" disambiguation
interacts with FR-049's rule-ID stability freeze starting at PR 10.
During PR 4–10 a rename could alias previously-distinct predicates.

**Assessment:** the disambiguation reads `RuleId` equality at runtime.
Within a single `fix_inner` call, the rule IDs are stable (the rule
set is built once at `Engine::new`). The PR 4–10 rename concern is
about *cross-version* stability; within-version equality always
holds. **No PM input needed.** Add a comment in the dispatch code
flagging the FR-049 future evolution.

### 4.6 R002 vs `EngineError::DeadlineExceeded` semantics

Both are partial-progress conditions. They differ:
- `DeadlineExceeded` returns `Err(EngineError)` with the partial
  lint; no FixResult is produced.
- R002 returns `Ok(FixResult)` with pass-1 fixes applied and an R002
  diagnostic.

**Decision:** keep them distinct. The shapes encode different
recovery strategies. Document in
`contracts/engine-pipeline.md` (per D1) so consumers can wire CLI
exit codes correctly.

---

## 5. Test strategy (T083, T084, T085)

### T083 — `two_pass_invariants.rs` (property tests)

```rust
proptest! {
    /// FR-022 / I-18: pass-1 and pass-2 promoted-fix spans are disjoint.
    #[test]
    fn i18_pass_spans_disjoint(input in arb_marking_corpus()) {
        let result = engine.fix(&input);
        let (p1, p2) = partition_applied_by_phase(&result.applied);
        for f1 in &p1 {
            for f2 in &p2 {
                prop_assert!(!spans_overlap(f1.span, f2.span));
            }
        }
    }

    /// FR-023 / I-19: a rule does not re-fire when its predicate held
    /// against pre-pass-1 attrs.
    #[test]
    fn i19_no_retroactive_refire(input in arb_marking_corpus()) {
        // Run a synthetic 2-pass dispatch with a stub rule that
        // tracks whether it fired pre-pass-1 vs post-pass-1.
        // Assert: if pre-pass-1 fired AND post-pass-1 attrs satisfy
        // the predicate, post-pass-1 does NOT emit a duplicate.
    }
}
```

Corpus inputs: synthesize realistic markings via the
`tests/corpus/valid/` strict-path fixtures plus targeted mangled-then-
fixable cases. Reuse the `proptest` generators from
`crates/engine/tests/proptest_engine.rs` if shape-compatible.

### T084 — `fix_invariants.rs` (Layer-3 invariants)

Per consolidated plan §6 (need to read §6 for full I-1/I-2/I-4
definitions; here is the structural shape):

- **I-1 (audit completeness):** every applied fix produces exactly
  one `AppliedFix` record.
- **I-2 (fix orderability):** kept fixes are total-ordered by
  (span.end DESC, span.start DESC, rule_id ASC, replacement ASC).
- **I-4 (suggest never promotes):** no `AppliedFix` exists with
  source severity `Suggest`.
- **I-18 (FR-022 non-overlap):** see T083.
- **I-19 (FR-023 reshape-aware):** see T083.

These are non-property unit tests; T083 is the property layer.

### T085 — `fix_10kb` Criterion bench

Bench input: a 10 KB document with **both** phase triggers active.

Synthesize:
- A run of ~100 portions, each ≈70 bytes. Each portion has at least
  one `OC` → `ORCON` style localized fix (`Phase::Localized`).
- Each banner triggers an E003-style ordering rule
  (`Phase::WholeMarking`).

Assert:
- `p95 ≤ 16 ms` (FR-030 / FR-032 absolute).
- `p99` ≤ `pre_refactor_baseline.p99 * 1.05` (FR-033 delta gate).

Capture pre-refactor baseline by running the bench on
`refactor-006-pr-7-phase-tagged-pass-split` parent (`origin/staging`)
before landing the pass-split. Store at
`benches/baselines/2026-XX-pre-pr7.json`.

---

## 6. Constitution-Check Gate

### V (Audit-First Compliance)
- **R002 emission**: `R002` is a diagnostic, not an `AppliedFix`. The
  pass-1 fixes that contributed DO promote — the audit log records
  what actually applied, which is honest about partial progress (per
  plan §9.4). ✓
- **`AppliedFix::__engine_promote` discipline**: pass-1 and pass-2
  both promote through `__engine_promote` from `Engine::fix_inner`.
  No new call sites in rule crates. ✓
- **G13 (content-ignorance)**: R002's `MessageArgs` carries
  `contributing_rule_ids: SmallVec<[RuleId; 4]>` and a `Span` —
  both on Constitution V's permitted-identifier list. No document
  bytes leak through R002. ✓
- **`EX_R002_PARTIAL = 3`**: distinct exit code makes partial
  application detectable without parsing NDJSON (D1). ✓

### VI (Dataflow Pipeline Model)
- **Phase stages independently testable**: pass-1 dispatch, re-parse,
  pass-2 dispatch each have a clear input/output contract. The
  `lint_with_options_internal` boundary is preserved. ✓
- **`PageContext` resets at page-break candidates**: the re-parse
  re-runs the lint pipeline including the PageBreak reset. ✓
- **No global mutable state**: `pre_pass_1_attrs` cache lives in
  `fix_inner`'s stack frame, borrowed into `RuleContext`. No
  `static mut`, no hidden Arc cache. ✓
- **`Send + Sync` preserved**: `Phase` is `Copy`, `RuleContext<'a>` is
  `Send + Sync` (no new state breaks this). ✓

### I (Performance)
- **SC-001 (p95 ≤ 16 ms)**: bench T085 enforces this. Re-parse skip
  when pass-1 empty preserves the no-fix path's existing performance.
  Cache populated only on overlap (R-4) keeps the overhead bounded.
- **FR-029 (linear scaling)**: pass-1 and pass-2 each scale linearly
  in fix count; total cost remains O(N) in N fixes. The re-parse adds
  O(M) in source length M, not in fix count. ✓
- **Risk:** the re-parse itself is the dominant new cost on documents
  with at least one pass-1 fix. Mitigation via the bench gate is
  measure-driven, not assumption-driven. ✓

---

## 7. "What Would Make Us Regret This in 5 Years?" List

### Regret 1: Inferred-from-comment first-fire span-shape check (§3.1)

**Obvious answer:** add a span-shape assertion to the trait so the
type system catches it at compile time.

**Why obvious answer is wrong:** spans are runtime-emitted. A type-
level shape declaration would just be another comment that drifts.

**Right answer:** keep the first-fire check + the corpus regression
harness + the AST-lint at registration. **But** add a `tracing::warn!`
at first-fire when in release mode, NOT a panic — engines that panic
in production fail audits.

### Regret 2: Synthetic engine diagnostic rule IDs as opaque strings

**Obvious answer:** keep `RuleId::new("R002")` and let PR 10 rename
when 2-tuple form lands.

**Why obvious answer is wrong on a 5-year horizon:** every R### rule
ID becomes a hardcoded string with no namespace. When PR 10's
2-tuple migration happens, every consumer of NDJSON output that
matched on `"rule": "R002"` breaks. The migration becomes a coordinated
flag day instead of an incremental change.

**Right answer:** Add a `pub const R002: RuleId = RuleId::new("R002");`
in `marque-engine` (or better, in `marque-rules` since `R001` already
lives in `marque-engine`). Treat the const as the single source of
truth. Build a `synthetic_engine_rule_ids()` -> `&'static [RuleId]`
function. PR 10 then has exactly two call sites to update, not
"every test that hardcodes the string." **Implementer should
centralize `R001` + `R002` into one module — even if it's a 6-line
file — during PR 7c.**

### Regret 3: The `Phase` enum has two variants today; will it ever have three?

**Obvious answer:** yes, add `Phase::Both` (FR-021 explicitly forbids
it but the temptation is real).

**Why obvious answer is wrong:** the murder board diagnosis behind
§9.1 is that `Phase::Both` collapses two distinct dispatch behaviors
into one declaration. A rule that needs both phases is two rules
sharing a backend module.

**Right answer:** add an extension method
`Rule::phase_companion(&self) -> Option<RuleId>` that returns the
other-phase rule's ID for rules that ship as a pair. This makes the
pair-registration discipline auditable — the registration check
verifies that for every rule declaring a `phase_companion`, the
companion rule exists in the registered set with the opposite
`Phase`. **Out of scope for PR 7** (no current rule needs it), but
the trait method has a one-line cost and prevents the 5-year drift
toward `Phase::Both`. **Defer to a follow-up PR** unless a real pair
emerges during T074.

### Regret 4: `pre_pass_1_attrs: Option<&CanonicalAttrs>` is a leaky abstraction

**Obvious answer:** ship it as designed — pass-2 rules can compare
pre and post attrs.

**Why obvious answer is risky long-term:** the cache says "this is what
the marking looked like before pass-1 applied." Future PRs that
introduce a third pass (e.g., a renderer-canonical-form pass-3) would
need `pre_pass_2_attrs`, `pre_pass_1_attrs`, ... — an unbounded
pattern.

**Right answer:** wrap the cache in a tiny named type:

```rust
/// Pre-rewrite snapshot of a marking's canonical attributes.
/// Used by `Phase::WholeMarking` rules to FR-023-disambiguate
/// reshape-induced false positives.
pub struct PreRewriteAttrs<'a> {
    pub attrs: &'a CanonicalAttrs,
    pub pass: PassId, // currently only `PassId::PrePass1`
}

pub enum PassId { PrePass1 }
```

The named-type pattern lets a future pass-3 add `PassId::PrePass2`
without changing the field name on `RuleContext`. **Implementer
discretion:** if the cost feels disproportionate for PR 7, ship the
raw `Option<&CanonicalAttrs>` and refactor in PR 8 — the refactor is
mechanical. Document the eventual rename in the code comment.

---

## 8. Implementation Checklist (per sub-PR)

### 7a checklist
- [ ] `enum Phase { Localized, WholeMarking }` in `marque-rules`.
- [ ] `Rule::phase()` method with `WholeMarking` default.
- [ ] All 31 `impl Rule` blocks declare phase explicitly.
- [ ] `crates/capco/tests/phase_assignment.rs` walks the rule set and
  asserts every rule declared its phase (compile-fail via Rust's own
  rules; a runtime test reads `rule.phase()` for each rule).
- [ ] PR description names every `Phase::Localized` rule
  (anticipated: 3 rules max — the OC/ORCON-class fixes).

### 7b checklist
- [ ] `fix_inner` restructure with C001 → pass-1 → re-parse → pass-2.
- [ ] `R002_RULE_ID` const adjacent to `DECODER_RULE_ID`.
- [ ] `build_r002_diagnostic` helper.
- [ ] `MessageArgs.contributing_rule_ids` field.
- [ ] `EX_R002_PARTIAL = 3` in `marque/src/main.rs`.
- [ ] BatchEngine worst-row-wins exit code (verify location).
- [ ] R002 surfaces in WASM return shape per D1 (detection without
  NDJSON parsing — verify with the WASM tests).
- [ ] `contracts/engine-pipeline.md` new section "R002 surfacing
  semantics" per D1.

### 7c checklist
- [ ] `RuleContext<'a>` lifetime parameter.
- [ ] `Rule::check(&self, attrs: &CanonicalAttrs, ctx:
  &RuleContext<'_>)` signature.
- [ ] Pre-pass-1 attrs cache (`SmallVec<[CanonicalAttrs; 4]>`).
- [ ] FR-023 disambiguation in pass-2 dispatch.
- [ ] `FeatureId::PrecedingFixPenalty` variant + `as_str` arm.
- [ ] E003 applies PrecedingFixPenalty when
  `ctx.pre_pass_1_attrs.is_some()`.
- [ ] `two_pass_invariants.rs` property tests.
- [ ] `fix_invariants.rs` Layer-3 invariants.
- [ ] `fix_10kb.rs` Criterion bench + baseline capture.
- [ ] `scripts/bench-check.sh` updated.

### Cross-PR
- [ ] No `MARQUE_AUDIT_SCHEMA` bump. The string stays
  `"marque-mvp-3"` (FR-035 explicitly forbids PR 7 from bumping;
  `PrecedingFixPenalty` and R002 fill reserved slots).
- [ ] Update `confidence.rs:198` doc comment ("new variants MUST bump
  the schema") to reflect the reserved-slot exception.
- [ ] Citation lint passes (no new cited authority introduced; R002 +
  the new `MessageTemplate::ReparseFailed` are engine-synthetic per
  message.rs:130).
- [ ] CHK015 + CHK018 cleared in PR 7b's reviewer attestation.
- [ ] PR 7c's bench result includes both absolute (≤ 16 ms p95) and
  delta (≤ 5%) gate measurements.

---

## 9. Files Referenced (Absolute Paths)

- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/specs/006-engine-rule-refactor/tasks.md:236-252`
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/specs/006-engine-rule-refactor/spec.md:368-372`, `:392`, `:399`, `:408-409`
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/specs/006-engine-rule-refactor/decisions.md:37-67`, `:179-198`
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/docs/plans/2026-05-02-engine-refactor-consolidated.md:750-885`
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/specs/006-engine-rule-refactor/checklists/correctness.md:62-71`
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/.specify/memory/constitution.md` (Principles I, V, VI)
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/crates/engine/src/engine.rs:51` (DECODER_RULE_ID),
  `:434` (lint_with_options_internal),
  `:1309` (fix_inner),
  `:1542` (AppliedFix::__engine_promote),
  `:2145` (build_decoder_diagnostic)
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/crates/engine/src/errors.rs:53` (EngineConstructionError)
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/crates/rules/src/lib.rs:95` (RuleId),
  `:204` (Severity re-export),
  `:225` (RuleContext),
  `:716` (Diagnostic),
  `:962` (Rule trait)
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/crates/rules/src/message.rs:135` (MessageTemplate),
  `:154` (ReparseFailed reserved slot),
  `:380` (MessageArgs)
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/crates/rules/src/confidence.rs:202` (FeatureId enum),
  `:241` (as_str)
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/crates/rules/src/fix_intent.rs:103` (FixIntent — note: no target_span field)
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/crates/ism/src/canonical.rs:64` (CanonicalAttrs — owned, no lifetime parameter)
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/marque/src/main.rs:26-33` (exit codes)
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/crates/engine/benches/` (bench directory)
- `/home/knitli/marque/.claude/worktrees/pr-7-phase-tagged-pass-split/crates/engine/tests/` (test directory)

---

## 10. Open Questions for PM Before Coding Starts

1. **Three sub-PRs or one?** (§1 — my recommendation: three.)
2. **`RuleId` encoding for R002**: 1-tuple `"R002"` now (renamed at PR 10)
   or build the 2-tuple shape in PR 7? (§3.4 — my recommendation: 1-tuple
   now, named const in `marque-rules`.)
3. **`EngineConstructionError::PhaseSpanShapeMismatch` or first-fire
   degenerate-fix-drop?** (§3.1, §3.6 — my recommendation: drop the
   error variant; use the first-fire path. Update T075's wording.)
4. **`EX_R002_PARTIAL = 3` numeric value** OK? (§3.8)
5. **Bench gate**: absolute (≤ 16 ms) OR delta (≤ 5%) OR both? (§4.1
   — my recommendation: both; capture both baselines.)
6. **Centralize R001 + R002 into one module** in `marque-rules` during
   7c, to prepare for PR 10's 2-tuple migration? (Regret #2 — my
   recommendation: yes.)
