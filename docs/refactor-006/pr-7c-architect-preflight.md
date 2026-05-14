<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7c Architect Pre-flight — Pre-Pass-1 Cache + FR-023 Disambiguation + `PrecedingFixPenalty`

> **STATUS UPDATE (2026-05-14)**: The two BLOCKING findings this preflight surfaced
> have been resolved by the PM. **D-7.18: the `marque-mvp-3 → marque-1.0` audit-schema
> bump is REMOVED from PR 7c scope** and deferred to a dedicated PR 3c.2. **D-7.19:
> `PrecedingFixPenalty` is engine-applied at the pass-2 confidence-threshold gate**, not
> rule-applied. **D-7.20 / D-7.21**: see PM decisions doc. The audit-schema sections of
> this preflight (§3.E, §4.B, §7) are now informational background for PR 3c.2; ignore
> them for PR 7c implementation. The branch name `refactor-006-pr-7c-rulecontext-fr023-audit-bump`
> reflects the original scope and will be renamed for PR 7c work to drop `audit-bump`.

> Tactical design lock for PR 7c (T080–T085). Branch off post-7b `staging`. Quality bar:
> "will we want to maintain this in 5 years?"

---

## 1. TL;DR

PR 7c lands the last conceptual delta of the PR-7 series:

1. `RuleContext<'a>` gains a lifetime carrying `pub pre_pass_1_attrs: Option<&'a CanonicalAttrs>`.
2. Pre-pass-1 attrs cache `SmallVec<[(Span, CanonicalAttrs); 4]>` on `TwoPassFixer::run`'s
   stack, populated only for markings whose span overlaps a pass-1 fix span (R-4).
3. **FR-023 full disambiguation** in `run_pass2_whole_marking`: same `(RuleId, Span)` already
   in `pass1_applied_keys` → drop the pass-2 diagnostic; different rule on a previously-
   fixed marking → fire with `pre_pass_1_attrs` populated.
4. **I-18 overlap demotion**: pass-2 diagnostics whose span overlaps any `pass1_applied_keys`
   span → demote `Severity::{Error,Warn,Fix}` to `Severity::Suggest`. The pass-1 fix already
   shipped; the pass-2 finding surfaces as a non-promoting suggestion.
5. `FeatureId::PrecedingFixPenalty` variant fills the reserved slot
   (`crates/rules/src/confidence.rs:224`); `as_str` arm + `KNOWN_FEATURES_FOR_TEST` table
   (`:304`); doc comment at `:198` updated.
6. **Penalty consumer**: engine-applied at the pass-2 dispatch boundary, NOT per-rule. E003
   is retired (PR 3c.B Commit 6 — `rules.rs:19,143`), so the PM message's "E003 applies the
   penalty" wording is stale; the penalty is now a generic engine mechanism — see §3.D and
   D-7.19.
7. Property + Layer-3-invariant tests at `crates/engine/tests/two_pass_invariants.rs`
   (FR-022/FR-023) and `.../fix_invariants.rs` (I-1, I-2, I-4, I-18, I-19).
8. `crates/engine/benches/fix_10kb.rs` Criterion bench — two-bench shape per D-7.11.
9. **Audit-schema label flip `marque-mvp-3 → marque-1.0`**. PM message asserts "single-shot
   LABEL flip — no accept-list, no structural envelope change." **This contradicts FR-035,
   the consolidated plan §4 line 335, D-7.10, audit-record.md §1+, and PR 3c.B Commit 10
   plan §2.1.** See §7. PM resolution required BEFORE the bump commit lands.

---

## 2. Scope summary (what / why / risk)

### 2.A `RuleContext<'a>` (T080 prerequisite)
**What**: `pub struct RuleContext<'a>` at `crates/rules/src/lib.rs:296`; new field
`pub pre_pass_1_attrs: Option<&'a CanonicalAttrs>`; trait method
`Rule::check(..., ctx: &RuleContext<'_>)`.
**Why**: `CanonicalAttrs` is owned (verified `crates/ism/src/canonical.rs:63-64`); the
cache lives in stack scope; the borrow's lifetime must be elidable. D-7.7 locks this.
**Risk**: 27-block mechanical signature change across `crates/capco/src/rules.rs` +
`rules_declarative.rs`; bodies unchanged (`'_` elision).

### 2.B Pre-pass-1 attrs cache
**What**: `SmallVec<[(Span, CanonicalAttrs); 4]>` populated in
`TwoPassFixer::populate_pre_pass_1_cache(...) -> SmallVec<...>`. Only entries for markings
whose span overlaps ≥1 pass-1 fix span. `(marking_span, attrs)` so per-rule lookup is
linear scan over ≤4 entries.
**Why**: R-4 / FR-023. Without it, pass-2 cannot disambiguate "rule fires because pass-1
exposed a new defect" from "rule fires retroactively because the predicate-against-
pre-reshape was already true."
**Risk**: `SmallVec<T>::Drop` is NOT `#[may_dangle]` (unlike `Vec`); same trap class as
7b's `engine.rs:1838-1841` `SmallVec<&Diagnostic>` ordering issue. The cache owns
`CanonicalAttrs` by value, but `RuleContext<'a>` constructed from the cache holds
`&'a CanonicalAttrs` — drop order must place every `RuleContext` borrow before the cache.
Mitigation: construct `RuleContext` per-rule inside the dispatch loop and let it drop
when the rule's `check` returns. `CanonicalAttrs::clone()` per cached overlap is
unavoidable (post-pass-1 attrs live in a different coordinate space).

### 2.C FR-023 full disambiguation
**What**: After a pass-2 rule fires, check `(diag.rule, diag.span) in pass1_applied_keys`.
If yes → drop the diagnostic. If no → keep it. The rule had `pre_pass_1_attrs` populated
during `check`, so its emit is by construction a different defect from pass-1's.
**Why**: 7b's `partition_diags_by_phase` (engine.rs:1842-1846) feeds pass-2 only fresh
post-pass-1 lint diagnostics — that closes the simple case. PR 7c closes the harder case:
the rule DID see a diagnostic post-pass-1 because the reshape exposed it, but it's the
same predicate the pass-1 rule already triggered.
**Risk**: `(RuleId, Span)` equality is the contract. Within-version stability holds; FR-049
rule-ID freeze begins at PR 10, so cross-version concerns are out of scope.

### 2.D I-18 overlap demotion
**What**: After `synthesize_fixes` produces pass-2 diagnostics but BEFORE `sort_and_c1_dedup`,
walk each diagnostic; if `pass1_applied_keys.iter().any(|(_, p1_span)| spans_overlap(d.span,
*p1_span))` AND `d.severity in {Error, Warn, Fix}` → demote to `Severity::Suggest`.
**Why**: FR-022 forbids overlapping `AppliedFix` spans. Pass-1's fix already promoted;
pass-2's MUST NOT. But the diagnostic is still useful — `Suggest` makes it visible without
auto-applying or escalating exit code (FR-042).
**Risk**: Renderer + audit log unchanged (`Suggest` already shipped in 7b; I-4 says no
`Suggest` promotes; FR-042 says `Suggest` does NOT trigger `EX_DIAG_WARN`).

### 2.E `FeatureId::PrecedingFixPenalty` variant
**What**: Add variant to `FeatureId` at `crates/rules/src/confidence.rs:224` (end of enum).
Add `as_str` arm at `:250` returning `"PrecedingFixPenalty"`. Add row
`(FeatureId::PrecedingFixPenalty, "PrecedingFixPenalty")` to `KNOWN_FEATURES_FOR_TEST` at
`:304`. Update doc comment at `:198` per D-7.10.
**Why**: D-7.10. Slot reserved in `marque-mvp-3` per PR 3c.B Commit 10.
**Risk**: Compile-time exhaustiveness on `as_str` catches one drift; the per-variant table
catches the other.

### 2.F Penalty consumer (engine-applied)
**What**: At the pass-2 threshold gate, before promoting a `FixIntent` to `AppliedFix`,
if the diagnostic's marking had `pre_pass_1_attrs.is_some()`, the engine splices a
`FeatureContribution { id: FeatureId::PrecedingFixPenalty, delta: -0.10 }` into
`FixIntent.confidence.features` and recomputes `Confidence.rule` (multiply-in semantic per
`confidence.rs:193`). Then the threshold gate runs.
**Why**: D-7.10 recommends `-0.10` magnitude. Engine-applied keeps rule crates stateless
(Constitution VI). The PM message says "E003 applies the penalty" but E003 is retired
(`crates/capco/src/rules.rs:19,143`); engine-applied is the only viable shape post-3c.B-6.
**Risk**: Mutation point is the only new write to a diagnostic's `Confidence`
post-emission. It MUST happen before threshold gating (otherwise the penalty wouldn't
affect auto-apply). The `PRECEDING_FIX_PENALTY_DELTA: f32 = -0.10` const lives at
`crates/engine/src/engine.rs` adjacent to the consumer.

### 2.G `MARQUE_AUDIT_SCHEMA` label flip (CONTESTED — see §7)
**What** (PM-asserted):
- `crates/engine/build.rs:24-25`: `ACCEPTED = &["marque-1.0"]`, `DEFAULT = "marque-1.0"`
- `crates/engine/src/lib.rs:87` (string), `:98` (discriminant — rename or retire per §3.E)
- `crates/engine/tests/audit_schema_accept_list.rs:25,31,42,47`
- `crates/engine/src/text_correction.rs:8` (doc comment)
- `crates/rules/src/lib.rs:43,56,386,440,783` (doc comments)
- `crates/wasm/src/lib.rs:407,485,515`
- `marque/src/render.rs:42,370,374,384,400,594,616,623,648,651,666,1049,1064,1094,1113`
- `specs/006-engine-rule-refactor/contracts/audit-record.md:8,22` (Active schema heading)
- `CLAUDE.md` (audit schema paragraph)

**Why this is structurally suspect**: `audit-record.md` §1+ defines `marque-1.0` as
having 2-tuple `RuleId`, `Canonical<S>` provenance discriminant, BLAKE3 digests, closed
`MessageTemplate` JSON shape, no `original` bytes. **None of these have shipped**
(`RuleId` is still 1-tuple `&'static str` per `rules/src/lib.rs:95`; no BLAKE3 digesting;
`Diagnostic.message` is still `Box<str>` per 7b rust-review HIGH-2). PR 3c.B Commit 10
plan §2.1 EXPLICITLY says "Commit 10 does NOT land `marque-1.0`. It lands `marque-mvp-3`,
which is the intermediate bump… The `mvp-N → 1.0` renaming retires later."
**Risk**: A literal label flip says `marque-1.0` but emits `marque-mvp-3`'s envelope.
Downstream consumers parsing per `audit-record.md` §1+ would misread. See §7.

### 2.H/2.I/2.J Tests + bench (T083/T084/T085)
- `crates/engine/tests/two_pass_invariants.rs` — proptest: `i18_pass_spans_disjoint`,
  `i19_no_retroactive_refire`. Use stub-rule test-fixture carve-out (Constitution V Test-
  fixture carve-out) for deterministic two-pass scenarios. Reuse `proptest_engine.rs`
  generators where shape-compatible.
- `crates/engine/tests/fix_invariants.rs` — unit tests for I-1, I-2, I-4, I-18, I-19.
- `crates/engine/benches/fix_10kb.rs` — two functions: `fix_10kb_pass2_only` (no pass-1
  triggers; exercises short-circuit path) + `fix_10kb_two_pass` (both phases active).
  Gate: absolute `p95 ≤ 16 ms` (FR-030) + delta `p99 ≤ baseline.p99 * 1.05` (FR-033).
  `scripts/bench-check.sh` updated. Baseline captured against `origin/staging` tip
  BEFORE PR-7c commits land; stored at `benches/baselines/2026-XX-pre-pr7.json`.

---

## 3. Decision points

### 3.A `RuleContext<'a>` lifetime introduction
**Options**: (a) parameterize uniformly (every rule's `check` signature); (b) keep non-
generic + `Arc<CanonicalAttrs>` shim.
**Recommend (a)**. (b) requires hidden `Arc` global state → Constitution VI breach
(rust-review §5 flagged this explicitly). D-7.7 locks (a); Q11 of `pr-7b-rust-preflight.md`
promised mechanical 31-block change.

### 3.B Cache storage location
**Options**: (i) `TwoPassFixer` struct field; (ii) local in `run`, threaded into
`run_pass2_whole_marking` as arg; (iii) derive in `run_pass2_whole_marking`.
**Recommend (ii)**. (i) mixes per-document state with per-engine state (the struct's
existing fields are per-engine — see `engine.rs:1557`). (iii) re-derives on every call
and creates a second drift seam. `SmallVec<[(Span, CanonicalAttrs); 4]>`: the 4 mirrors
the Localized rule cap (C001/E006/E007/S004) — even one fire per rule maxes at 4 reshape
sites in the typical case; spill to heap is acceptable.

### 3.C I-18 demotion site
**Options**: (α) inside `partition_diags_by_phase` (engine.rs:2318); (β) in dispatch loop
during synthesis; (γ) in `run_pass2_whole_marking` after `synthesize_fixes` but before
`sort_and_c1_dedup`.
**Recommend (γ)**. (α) corrupts the partition's input — rules never see their unmodified
diagnostic. (β) tangles the synthesizer with pass-1 span awareness. (γ) is the clean seam:
demote between rule fire and C-1 dedup; the audit log naturally records the demoted
severity without special audit code.

### 3.D Penalty consumer shape
**Options**: (P) engine-applied at threshold gate with hardcoded
`PRECEDING_FIX_PENALTY_DELTA: f32 = -0.10`; (Q) per-rule opt-in (rules check
`ctx.pre_pass_1_attrs.is_some()` in `check`).
**Recommend (P)**. The PM message says "E003 applies it" but E003 is retired
(`rules.rs:19,143`); (Q) would require declaring which rules opt in, accreting per-rule
state, and is one drift seam for every rule that should remember to apply the penalty.
(P) is uniform, audit-deterministic ("every promoted fix on an overlap marking carries
the penalty contribution"), and YAGNI-safe — recalibration is a follow-up (D-7.21).
The penalty is a workflow signal, NOT a corpus-derived prior; it does NOT belong in
`crates/capco/corpus/priors.json`.

### 3.E `AUDIT_SCHEMA_IS_V3` rename/retire
**Options** (conditional on §7 Option I landing): (R) rename to `AUDIT_SCHEMA_IS_V1_0`;
(S) rename to `AUDIT_SCHEMA_IS_V1`; (T) retire entirely.
**Recommend (T)**. Single-entry accept-list makes the discriminant trivially true; the
sentinel sites at `marque/src/render.rs:623` and `crates/wasm/src/lib.rs:515` are empty
`let _ = ...;` checks. Replace each with a `const _: () = assert!(matches!(
AUDIT_SCHEMA_VERSION, "marque-1.0"));` if a build-time check is desired. If §7 Option II
lands (defer the bump), this decision is moot.

---

## 4. Sequencing within the PR

Five commits, in this order:

1. **Commit 1 — `RuleContext<'a>` lifetime (§2.A)**. Mechanical; 27-block trait-method
   signature change; rule bodies unchanged. CI green before commit 2.
2. **Commit 2 — Cache + FR-023 + I-18 (§2.B, §2.C, §2.D)**. Load-bearing. Touches
   `engine.rs:1842-1846` (seam) and `:2044-2085` (`run_pass2_whole_marking` body).
3. **Commit 3 — `PrecedingFixPenalty` variant + engine-applied consumer (§2.E, §2.F)**.
   `confidence.rs:198,224,250,304` + `engine.rs` penalty const + application site.
4. **Commit 4 — Tests + bench (§2.H/2.I/2.J)**. Three new files; baseline JSON captured
   against `origin/staging` tip; `scripts/bench-check.sh` updated.
5. **Commit 5 (CONDITIONAL — PM-resolution required per §7) — Schema label flip (§2.G)**.
   Lands LAST so it's the discardable commit. Diff is a label rewrite across ~15 binding
   sites. If PM directs Option II (defer), this commit doesn't land.

**Rationale**: Commits 1–4 are uncontested and land independently. Commit 5 carries the
§7 contradiction and should not couple with structural work. The new `PrecedingFixPenalty`
variant compiles cleanly under either `marque-mvp-3` or `marque-1.0`.

---

## 5. Risk register

| # | Risk | Mitigation |
|---|------|------------|
| R1 | `SmallVec<CanonicalAttrs>` Drop ordering — same trap class as 7b's `SmallVec<&Diagnostic>` (engine.rs:1838-1841) | Construct `RuleContext<'a>` per-rule inside `run_pass2_whole_marking`'s dispatch loop and let it drop when `check` returns. If the borrow-checker objects, add explicit `drop(pre_pass_1_cache)` at the end of the function. `SmallVec<T>::Drop` is NOT `#[may_dangle]`; cache must outlive every borrow it spawns. |
| R2 | Audit-schema rename misses across ~260 repo-wide mentions | Bind-site commit ONLY touches `build.rs` accept-list, consuming consts, emitter call-sites, regression-pin test, contract doc §0, CLAUDE.md. Plan/spec/archived-doc mentions describe history and stay. The pin test at `audit_schema_accept_list.rs:42-47` literally embeds `build.rs`'s string, so drift between them is impossible to ship. |
| R3 | Bench baseline capture timing | Document explicit baseline-capture step in commit 4's message + PR description. `scripts/bench-check.sh` MUST fail loudly when the baseline JSON is missing — no default-pass. |
| R4 | FR-023 test fixture construction (two-rule + same-marking scenarios) | Stub-rule approach in `tests/two_pass_invariants.rs` under Constitution V Test-fixture carve-out: declare two trivial test-only rules (one Localized, one WholeMarking with a conditional fire on `ctx.pre_pass_1_attrs`), register them on a test-only engine, assert the disambiguation behavior. Stub rules live in `crates/engine/tests/` only. |
| R5 | PM-contradiction on audit-schema bump | See §7. Block commit 5 on PM resolution; recommend Option II (defer). Do NOT land based on chat decision alone — `pr-7-pm-decisions.md` D-7.10 must be amended (or D-7.18 added) in the binding doc first. |
| R6 | `Severity::Suggest` consumer fallout — third path producing it (after lint-time decoder demotion and pass-2 confidence-threshold demotion) | Document the third path in PR description. Renderer + audit log + exit-code path are all unchanged — `Suggest` semantics were FR-042-shipped in 7b. |
| R7 | Penalty const recalibration drift | D-7.21 (proposed) tracks corpus-derived recalibration as follow-up. `PRECEDING_FIX_PENALTY_DELTA: f32 = -0.10` const must be loudly named and adjacent to the consumer site so a future recalibration PR finds it. |

---

## 6. Out-of-scope (do NOT pull in)

- No new rule. Penalty lands as engine-applied (§2.F).
- No change to `Recognizer` dispatch. `StrictOrDecoderRecognizer` stays the `Engine::new`
  default.
- No re-coverage of 7a/7b paths. The `partition_diags_by_phase` and `TwoPassFixer`
  structures land as in 7b; 7c modifies pass-2 dispatch body only.
- No `Rule::phase_companion()` (architect Regret #3 — deferred).
- No `PreRewriteAttrs` wrapper type (architect Regret #4 — deferred to PR 8+).
- No `RuleId` 2-tuple migration. FR-049 freeze begins at PR 10.
- No `DECODER_RULE_ID` type unification (still `&'static str` per D-7.4 deferral).
- No `MessageArgs` field additions. The 7b HIGH-2 dead-`_message_args` finding is a
  PR 3c.2 migration obligation, not a 7c task.
- No structural envelope changes to the audit record, even if §2.G lands (Option I
  explicitly says "no structural change").

---

## 7. Audit-schema bump contradiction — resolution required

**The contradiction:**

| Source | Statement |
|---|---|
| Spec FR-035 (`spec.md:399`) | "PR 7 MUST NOT bump the schema" |
| Consolidated plan §4 line 335 | "PR 7 does NOT bump the schema" |
| PM-D-7.10 (`pr-7-pm-decisions.md:287-307`) | "Do NOT bump `MARQUE_AUDIT_SCHEMA`" |
| PR 7c-prompt PM message | "Audit-schema bump `marque-mvp-3 → marque-1.0` … single-shot LABEL flip" |
| `audit-record.md` §1+ describing `marque-1.0` | 2-tuple `RuleId`, `Canonical<S>` provenance, BLAKE3 digests, closed `MessageTemplate` JSON, no `original` bytes |
| Actual code state | 1-tuple `RuleId`, no `Canonical<S>`, no BLAKE3, `Diagnostic.message: Box<str>` |
| PR 3c.B Commit 10 plan §2.1 | "Commit 10 does NOT land `marque-1.0`. It lands `marque-mvp-3`, which is the intermediate bump… The `mvp-N → 1.0` renaming retires later" |

**Three resolutions; recommend Option II.**

### Option I — Literal label flip (PM-message interpretation)
Flip the string only; envelope unchanged from `marque-mvp-3`. Update `audit-record.md`
§0 to acknowledge the literal-vs-structural divergence — that's a substantial doc
restructure (§1+ describes a different shape). **Risk**: downstream consumers reading
§1+ parse-fail or misread records.

### Option II — Defer the bump (RECOMMENDED)
Keep `marque-mvp-3` through PR 7c. Commits 1–4 land; commit 5 doesn't. Track the bump as
a follow-up PR (call it PR 7.5 or fold into PR 10) that lands the label flip atomically
with the structural envelope: 2-tuple `RuleId`, `Canonical<S>` provenance, BLAKE3, closed
JSON `MessageTemplate`. **Honors Constitution Principle V's spirit** (audit records honest
about their shape) and defers the contested decision to when the structural work catches
up.

### Option III — Bring up the structural envelope first
Land EVERY delta `audit-record.md` §1+ describes, then flip the label. **Out of scope for
PR 7c** — multi-PR expansion that contradicts the keystone sequencing.

**Implementer obligation**: Block commit 5 on PM written resolution. If Option I lands,
require a `pr-7-pm-decisions.md` amendment (D-7.10 superseded by D-7.18) AND a tracking
issue for the structural follow-up. **Do not land commit 5 based on chat decisions alone**;
the binding doc must change first.

---

## 8. Constitutional gate

### Principle V (Audit-First Compliance)
- `AppliedFix::__engine_promote` stays engine-only. The penalty mutation in §2.F runs
  inside the engine's pass-2 dispatch loop on in-flight diagnostics, BEFORE the threshold
  gate calls `__engine_promote`. No rule-crate touch.
- Content-ignorance (G13): `PrecedingFixPenalty` carries no document bytes —
  `FeatureContribution { id: FeatureId, delta: f32 }` at `confidence.rs:188` accepts only
  enum-typed identifiers + a scalar; both on Constitution V's permitted-identifier list.
- R002 stays non-promoting (PR 7b invariant unchanged).
- §7 Option I would create label dishonesty (`schema: "marque-1.0"` on a `marque-mvp-3`
  envelope); Option II preserves label-honesty.

### Principle I (Performance)
- `fix_10kb` bench is the SC-001 gate. Two-bench shape isolates the short-circuit path
  from the full two-pass path.
- Cache scope bounded by R-4: O(K) `CanonicalAttrs::clone()` calls where K = pass-1 fix
  count, capped at ~4 Localized rules × marking-count on dense documents.
- Penalty application is O(diag) post-rule-fire; `spans_overlap` predicate same as
  demotion + cache-population sites; ≤4-element `pass1_applied_keys` set in typical use.
- D-7.11 delta gate: `p99 ≤ baseline.p99 * 1.05`.

### Principle VI (Dataflow Pipeline Model)
- `RuleContext<'a>` lifetime applies uniformly across phases; the cache is engine-owned
  (stack-bound), borrowed into per-rule `RuleContext`. No `static mut`, no
  `OnceCell<Mutex<_>>`, no `Arc<CanonicalAttrs>` (Constitution VI "no hidden global
  state" preserved).
- Rules stay stateless: rule `check` bodies read `pre_pass_1_attrs` per-invocation and
  don't store it.
- Per-phase C-1 dedup already landed in 7b; demoted `Suggest` diagnostics drop out of the
  dedup walk because they have no fix to apply.

---

## 9. PM contract — existing + proposed

**Existing bindings**: D-7.1 (three-PR split — 7c is 3 of 3), D-7.7 (`RuleContext`
lifetime + `Option<&'a CanonicalAttrs>` + `SmallVec<[CanonicalAttrs; 4]>`; this preflight
refines to `SmallVec<[(Span, CanonicalAttrs); 4]>` so lookup is by span — additive
clarification), D-7.10 (`PrecedingFixPenalty` reserved slot, no schema bump), D-7.11
(bench shape).

**Proposed new decisions** (counts: **4 new D-7.NN**):

- **D-7.18 (BLOCKING) — Audit-schema label flip resolution**. PM message asserts
  `marque-mvp-3 → marque-1.0` label flip; this contradicts FR-035 / consolidated plan
  §4.335 / D-7.10. Three resolutions in §7. **Recommend Option II (defer)**. If PM
  selects Option I, `pr-7-pm-decisions.md` must be amended in the same commit that
  amends D-7.10.
- **D-7.19 — `PrecedingFixPenalty` consumer location**. PM message says "E003 applies the
  penalty," but E003 is retired (`rules.rs:19,143`). **Recommend engine-applied** with
  hardcoded `PRECEDING_FIX_PENALTY_DELTA: f32 = -0.10` per D-7.10 magnitude.
- **D-7.20 — `AUDIT_SCHEMA_IS_V3` const fate**. Conditional on §7 outcome.
  **Recommend retire** (§3.E Option T) if Option I lands; moot if Option II lands.
- **D-7.21 — `PrecedingFixPenalty` recalibration follow-up**. Track corpus-derived
  recalibration of the `-0.10` magnitude as a follow-up issue. NOT scoped into 7c. The
  corpus harness in `crates/capco/corpus/` is the natural site.

---

## 10. Files touched in PR 7c

### Modified (always)
- `crates/rules/src/lib.rs` — `RuleContext<'a>` (lines 296-324), trait method (line ~962).
- `crates/rules/src/confidence.rs` — variant (`:224`), `as_str` (`:250`), test table
  (`:304`), doc comment (`:198`).
- `crates/capco/src/rules.rs` + `rules_declarative.rs` — `&RuleContext` →
  `&RuleContext<'_>` across 27 `impl Rule` blocks; bodies unchanged.
- `crates/engine/src/engine.rs` — pass-2 dispatch body (`:2044-2085`); cache population
  fn; penalty application; FR-023 disambiguation; I-18 demotion;
  `PRECEDING_FIX_PENALTY_DELTA` const.
- `scripts/bench-check.sh` — `fix_10kb` gate.

### Modified (conditional on §7 Option I)
- `crates/engine/build.rs:24-25` (accept-list flip).
- `crates/engine/src/lib.rs:87,98` (schema const + discriminant).
- `crates/engine/src/text_correction.rs:8` (doc).
- `crates/rules/src/lib.rs:43,56,386,440,783` (docs).
- `crates/wasm/src/lib.rs:407,485,515`.
- `marque/src/render.rs:42,370,374,384,400,594,616,623,648,651,666,1049,1064,1094,1113`.
- `crates/engine/tests/audit_schema_accept_list.rs:25,31,42,47`.
- `specs/006-engine-rule-refactor/contracts/audit-record.md:8,22` (Active schema).
- `CLAUDE.md` (audit schema paragraph).

### New
- `crates/engine/tests/two_pass_invariants.rs` (T083).
- `crates/engine/tests/fix_invariants.rs` (T084).
- `crates/engine/benches/fix_10kb.rs` (T085).
- `benches/baselines/2026-XX-pre-pr7.json` (baseline capture, hand-committed alongside
  commit 4).

---

## 11. Tactical hand-off checklist

- [ ] §2.A: `RuleContext<'a>` lifetime parameterized; trait method updated;
  rule bodies use `'_` elision; Constitution VI preserved.
- [ ] §2.B: Stack-owned `SmallVec<[(Span, CanonicalAttrs); 4]>`; populated only on
  overlap; passed by ref into `run_pass2_whole_marking`. R1 drop-ordering verified.
- [ ] §2.C: FR-023 disambiguation drops same-`(RuleId, Span)` diagnostics in pass-2.
- [ ] §2.D: I-18 demotion AFTER `synthesize_fixes`, BEFORE `sort_and_c1_dedup`. Predicate:
  span-overlap with any `pass1_applied_keys` span.
- [ ] §2.E: `FeatureId::PrecedingFixPenalty` variant + `as_str` + test-table row.
- [ ] §2.F: Engine-applied penalty; magnitude `-0.10`; const at `engine.rs` adjacent to
  consumer.
- [ ] §2.G + §7: Schema label flip BLOCKED on PM resolution. Recommended Option II
  (defer).
- [ ] §2.H/§2.I: Property + Layer-3-invariant tests in commit 4. Stub-rule fixtures
  under Constitution V Test-fixture carve-out.
- [ ] §2.J: `fix_10kb.rs` bench + baseline capture + `bench-check.sh` update in commit 4.
- [ ] §3.D: `PRECEDING_FIX_PENALTY_DELTA = -0.10` const loudly named. D-7.21 tracking
  follow-up filed.

5-year-maintenance posture: cache lifetime is structural not historical; FR-023
disambiguation is testable against synthetic two-rule fixtures; the penalty const is
the only knob and is named loud enough that a future calibration PR finds it. The §7
contradiction, if resolved as Option II, preserves the structural label-honesty
Constitution Principle V's spirit requires.
