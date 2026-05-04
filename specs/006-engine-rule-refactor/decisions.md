<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Decision Register: Engine + Rule Architecture Refactor

**Branch**: `006-engine-rule-refactor` | **Date**: 2026-05-04
**Companion to**: [spec.md](./spec.md), [plan.md](./plan.md),
[research.md](./research.md), [contracts/](./contracts/).
**Source**: panel-review pass on the spec set against
`docs/plans/2026-05-02-engine-refactor-consolidated.md`.

These 16 decisions resolve open questions surfaced during panel review.
Every decision lands in PR 0 (or its setup) before PR 1 begins. Some are
spec / plan / contract edits; others ship as scaffolds (TOML files,
lint extensions, attestation templates). When PR 0 lands, every
decision below is encoded in source-tree artifacts or in the
spec / plan / contract text — none are left to reviewer judgment at
later PRs.

**Status convention**: each decision is **locked** — binding for the
refactor's planned execution. Audibles during implementation are
permitted but require a follow-up PR amending this register, not a
silent drift.

The naming convention `D##` is parallel to `R-##` in `research.md`
(implementation tactics) and `FR-###` / `SC-###` in `spec.md`
(requirements / success criteria). Decisions here are
**process / contract** decisions, distinct from R-## (implementation)
and FR-### (functional requirements).

---

## Critical decisions (block PR 0)

### D1 — R002 consumer-surface contract

**Decision**: The engine returns the pass-1 buffer on R002 (existing
behavior, source plan §9.4). NEW consumer-surface obligations:

- **CLI**: distinct exit code `EX_R002_PARTIAL` (numeric value chosen
  at PR 7 implementation; documented in `marque/src/main.rs` exit-code
  table). Distinct from `EX_DIAG_WARN` and from regular fix-failure.
- **WASM**: typed return shape signaling partial application
  (e.g., `LintResult { partial: true, .. }` or a typed `Result`
  variant). Consumers MUST be able to detect R002 without parsing
  NDJSON.
- **IDE plugins**: documented contract that plugins MUST inspect the
  R002 diagnostic before applying the returned buffer. The buffer is
  the post-pass-1 state; applying without inspection silently splices
  pass-1 fixes into the user's editor.
- **`BatchEngine`**: per-row R002 surfaces in the row's individual
  result; the batch exit code is **worst-row-wins** (any row hitting
  R002 raises the batch exit code to `EX_R002_PARTIAL`).

**Lands in**: `contracts/engine-pipeline.md` new section "R002
surfacing semantics (consumer-surface contract)".

**Rationale**: §9.4 specifies engine-side semantics only. The
consumer surface is unstated — and the IDE-plugin failure mode
(silent partial-buffer application) is destructive without an
explicit contract.

**Audibles permitted**: WASM API shape (typed `Result` variant vs.
flag on `LintResult`) is implementer's call at PR 7; the binding
constraint is detection without JSON parsing.

---

### D2 — PR 3.7 stall-recovery

**Decision**: PR 3.7 stays **monolithic** (no wave-split). NEW
requirement: a **named alternate owner** in the PR description who
has independently read §§2–8 of `2026-05-01-lattice-design.md`
*before PR 3c merges*. The alternate takes ownership without
escalation if the primary owner stalls past 1 week.

**Lands in**: `spec.md` Assumptions section amendment (PR 3.7 entry).

**Rationale**: Wave-splitting collides with PR 4's clean-break
property — PR 4 deletes `CapcoMarking::join`'s PageContext
delegation "with no equivalence shim," which forbids partial lattice
coverage. Cross-axis fixtures span categories (FOUO eviction depends
on classification AND dissem lattices) and cannot wave-isolate.
Bus-factor mitigation via alternate owner is cheaper than scope
mitigation via split, without compromising PR 4.

**Audibles permitted**: deadline slip beyond 2 weeks still requires
explicit team review (existing constraint preserved).

---

### D3 — NDJSON consumer discoverability

**Decision**: `marque --version` exposes the active audit schema
name. The per-record `"schema"` field (already mandatory per source
plan §10.2.1) is the discriminator for streaming consumers. No new
NDJSON format additions.

**Lands in**: `contracts/audit-record.md` new section "Schema
discoverability".

**Rationale**: Clean-break philosophy precludes a reader crate. The
`"schema"` field is already structurally present in every record;
surfacing the active schema in `--version` lets shell-script and
log-aggregation consumers detect mismatch without inspecting
records.

**Audibles permitted**: format of `--version` output (JSON vs.
human-readable line) is implementer's call at PR 3c; binding
constraint is that the schema name appears.

---

### D4 — No-downstream-consumers tripwire

**Decision**: Manual attestation only. **No CI grep.** PR 0 and
PR 3c PR descriptions MUST include a date-stamped, channel-listed,
person-signed attestation that no external consumers exist.

**Attestation template** (copy into the PR description):

> I have searched the following channels for marque consumers as of
> `[YYYY-MM-DD]`:
>
> - GitHub issues / discussions / forks (org `marquetools`)
> - npm download stats for `@marque/wasm` and related packages
> - crates.io download stats for `marque-*` crates
> - partner-org integration team contacts (last 60 days)
> - internal Slack / email channels (last 60 days)
>
> Found: `[NONE / list of consumers]`. The team has not been
> contacted by any external integration team in the past 60 days.
>
> Signed: `[PERSON]` — `[DATE]`.

**Lands in**: `plan.md` PR 0 acceptance criteria; this register's
template above is the authoritative form.

**Rationale**: A grep-based check across the named channels is fake
rigor — most channels have no API or are private. Manual attestation
with date, channel list, and signature is a stronger signal because
the attestation is owned. Self-attestation is the actual mechanism;
adding mechanical theatre dilutes the signal.

---

### D5 — PR 3c rollback if mangled-corpus accuracy regresses

**Decision**: Pre-commit to `research.md` R-8 decision tree as
**binding**. The decision is encoded in
`tests/corpus/mangled/threshold.toml` (D7), not in PR 3c review
notes. If post-PR-3c accuracy lands <0.80 and the loss is not
K-Option-2-attributable per R-8, **revert PR 3a / 3b / 3c as a
unit**.

**Lands in**:
- `spec.md` SC-010 wording (remove "Decision recorded in PR 3c
  review notes"; reference R-8 + threshold.toml).
- `research.md` R-8 amendment (decision tree is binding, not
  deferred).

**Rationale**: Deferring the decision to PR 3c review notes leaves
the rollback policy in reviewer judgment at the moment when the
team is under merge-pressure. Pre-commit makes the policy
mechanical. Yes, this means the team accepts the cost of a 2–3 week
recovery cycle if the regression is real and not K-Option-2-attributable
— that cost is part of the clean-break philosophy.

**Audibles permitted**: the corpus-split design (case 3 of R-8) is
implementer's call at PR 3c; binding constraint is that the
threshold artifact records which R-8 branch was taken and why.

---

## High-value decisions

### D6 — Rule-ID stability rule

**Decision**: Predicate-id is stable within a major audit-schema
version. **Stability freeze begins at PR 10 merge** (not PR 3c).
Predicate renames during PR 4–10 are permitted; renames after
PR 10 require a coordinated `marque-2.0` schema bump.

**Lands in**: new FR in `spec.md` (FR-049).

**Rationale**: PR 3c is the schema bump but PRs 4–10 reshape
predicates through cross-axis fixtures, lattice work, and
banner-validation migration. Freezing at 3c forces "correct first
time" naming during peak design churn — exactly when team
understanding is least mature. Freeze at end-state preserves the
stability property auditors need without paying the cost during
refactor execution.

**Audibles permitted**: PR 10 merge can amend the legacy-rule-id
mapping doc (`docs/refactor-006/legacy-rule-id-map.md`, R-3) to
record any rename that happened during PR 4–10.

---

### D7 — SC-010 R-8 decision in CI artifact

**Decision**: `tests/corpus/mangled/threshold.toml` (structured
TOML, not a freeform README). `tools/bench-check.sh` reads it. The
file is created (with a documented schema) in PR 0; populated with
the actual threshold value and the chosen R-8 branch in PR 3c.

**Lands in**: `tests/corpus/mangled/threshold.toml` scaffold in
PR 0; spec SC-010 references it.

**Rationale**: README format drift breaks freeform parsers
silently. TOML is the standard structured-config artifact in the
workspace and parses with `toml` crate (already a workspace dep).
The TOML schema is small and stable.

---

### D8 — Cumulative bench drift assertion

**Decision**: At PR 10, re-run all per-PR bench comparisons against
the PR-0 baseline (R-5) on **pinned bench hardware**. Per-bench
cumulative drift ≤10% is the gate; per-PR contributions exceeding
6% are flagged for attribution. Bench hardware is **pinned for the
duration of the refactor** (decision recorded in PR 0).

**Lands in**:
- `plan.md` Risk section / FR-033 enforcement note.
- PR 0 description records the chosen bench-runner commitment
  (rented bare-metal vs. dedicated GitHub-hosted runner spec).

**Rationale**: Hardware drift over the refactor's calendar window
(CI runner upgrades, kernel changes, runner capacity adjustments)
can account for several percent of baseline shift independent of
code. Pinned hardware is the only honest comparison mechanism.
Per-PR attribution gives diagnostic power if the cumulative gate
fails — pinpoints which PR contributed most.

---

### D9 — PR 9 sub-divide

**Decision**: PR 9 → three sub-PRs:

- **9a** — parser separator spans (#106). Includes an internal
  acceptance test asserting the parser correctly identifies
  separator positions. Closes nothing in the issue tracker (pure
  infrastructure for 9b / 9c).
- **9b** — `dissem_us` / `dissem_nato` position-attributed split
  (#271). Depends on 9a (separator positions delimit US vs. NATO
  dissem regions). Banner-validation rules migrate to
  `&ProjectedMarking` here.
- **9c** — ATOMAL / BOHEMIA recognition (#246) +
  NATO-portion-in-US-doc declarative `Constraint` (#265).

**Lands in**: new R-9 in `research.md`; updates to `plan.md`
project structure and PR-table references.

**Rationale**: PR 9 currently bundles parser infrastructure +
data-model split + vocabulary additions across distinct
correctness properties. Sub-PRs mirror the PR 6 / PR 3 sub-division
discipline. 9a needs an internal acceptance test (parser correctly
identifies separator positions) so it doesn't ship as
"infrastructure with no consumer" — which would smell.

**Audibles permitted**: sub-PR ordering is fixed (9a → 9b → 9c by
dependency), but bundling 9b's banner-validation migration with 9c
instead is permissible if implementer finds it cleaner.

---

### D10 — Layer 0 (compile-fail tests) in test taxonomy

**Decision**: Layer 0 = compile-fail tests for FR-001 / FR-003 /
FR-005 type-system invariants. Run via `trybuild`. **Pinned
toolchain** (`rust-toolchain.toml` at workspace root) and **pinned
trybuild version** (`Cargo.toml` exact version specifier). Layer 0
runs per-PR, not per-save (slow; full compile per case).

**Lands in**:
- `contracts/engine-pipeline.md` test-strategy section amended to
  include Layer 0.
- `rust-toolchain.toml` and trybuild version pinned in PR 0 (if
  not already).

**Rationale**: Compile-fail tests are the keystone evidence that
sealed-construction invariants hold (FR-001's `Box<str>→Canonical`
unconstructable; FR-003's `format!`-into-Message unconstructable;
FR-005's `__engine_promote` engine-only). They deserve a first-class
slot in the test taxonomy, not inline mention. MSRV bumps trigger
Layer-0 maintenance — accepted cost.

---

### D11 — Masking-pin GitHub API caching

**Decision**: **Cache-with-fallback** strategy (NOT cache-only). At
PR-time, the lint attempts a GitHub API call with a 5-second
timeout. On API failure or rate-limit, fall back to a
daily-refreshed cache and emit a CI warning (not an error). A
scheduled job populates `tools/masking-pin-lint/cache/` daily.

**Lands in**: new R-10 in `research.md`; design note in
`tools/masking-pin-lint/` README at PR 0.

**Rationale**: Cache-only weakens the lint at the moment correctness
most depends on it (the PR that should remove a closed pin sees a
stale "still open" cache and merges with a stale pin). Cache-only
also has a race condition with the closure protocol (issue closes →
pin-removing PR opens → pre-cache-refresh, lint sees stale "open"
state and... actually correctly accepts pin removal because the cache
says open — but logs a misleading message). Cache-with-fallback is
the standard pattern: prefer fresh, accept stale on outage with
visible warning.

---

### D12 — `_unchecked` forbidden lint targets signature shape

**Decision**: The `tools/promote-callsite-lint/` lint (FR-040) is
extended to flag any function whose **signature shape** matches
"accepts `ParsedAttrs` and returns `CanonicalAttrs`" outside
`MarkingScheme::canonicalize`. The lint targets shape, not name.
`unsafe fn` is whitelisted (Rust stdlib uses `_unchecked` for
`unsafe` APIs). The transitional `from_parsed_unchecked` adapter is
exempted during the PR 3a–3c keystone window via a path-based
carve-out that auto-deletes when 3c lands.

**Lands in**: new R-11 in `research.md`; FR-040 amended in
`spec.md` to include the signature-shape extension.

**Rationale**: Naming-only lint is brittle (rename to
`from_parsed_raw` evades). Targeting signature shape catches
intent: any `ParsedAttrs → CanonicalAttrs` conversion outside
the trait method is the actual failure pattern. Rust stdlib uses
`_unchecked` extensively for `unsafe` APIs (`get_unchecked`,
`from_utf8_unchecked`); whitelist by `unsafe fn` keyword.

---

### D13 — Rule-collapse band

**Decision**: 56 → **8–18 rule count band** post-PR-3b + qualitative
gate. Each surviving rule MUST satisfy:

1. A single CAPCO-§ citation (no rule cites more than one §).
2. Predicate body has ≤3 internal branches (measured by
   `match`/`if`-arm count).
3. PR 3b reviewer attests both properties in the PR description.

Out-of-band counts (<8 or >18) require explicit team review.

**Lands in**: `plan.md` PR 3b acceptance criteria.

**Rationale**: Count alone is a proxy for design quality, not a
substitute. A team collapsing to 8 rules with brittle internal
branching is "in band" but worse than a team landing at 19 with
clean predicates. Pairing the count with a qualitative gate (single
citation, branch limit, reviewer attestation) prevents both failure
modes — under-collapse and over-collapse-with-brittle-branching.

---

## Strategic decisions

### D14 — Trait surface stabilization

**Decision**: `Vocabulary<S>`, `MarkingScheme`, `Codec<S>` trait
surfaces stabilize when **either** (a) a third in-tree consumer
arrives (CAPCO + scheme #2 + scheme #3), OR (b) **12 months elapse
post-scheme-#2-merge with zero breaking trait changes**, whichever
comes first. Until then, the surfaces remain `#[doc(hidden)] pub`
semver-unstable per source plan §3.10.

**Lands in**: `spec.md` Assumptions section amendment (existing
"`Vocabulary<S>`, `MarkingScheme`, `Codec<S>` ship semver-unstable"
clause extended with the forcing function).

**Rationale**: "6 months after scheme #2" without a forcing function
tends to never trigger — hidden APIs become permanent. Third
consumer is the natural API-stability test (a single second-scheme
adopter doesn't exercise enough surface). 12-month no-break window
is the time-based fallback for the case where scheme #3 takes a
long time to arrive.

---

### D15 — P1 fixture pinning in acceptance scenarios

**Decision**: Acceptance scenarios that reference fixture corpora
use a **path glob + count assertion**, not single-file naming.

Examples (replacing single-file references in current spec):

- US1 AC1: `tests/corpus/mangled/sci_compartment_*.{json,txt}` +
  ≥1 fixture demonstrates the scenario.
- US2 AC1: `tests/corpus/foreign/pure_foreign_*.json` + ≥1 fixture
  demonstrates the scenario.

**Lands in**: `spec.md` US1 / US2 acceptance scenarios edited.

**Rationale**: Single-file pinning breaks ACs silently when
fixtures are renamed or consolidated. Glob + count survives
reorganization while preserving verifiability — and discourages
the "one privileged canonical fixture" pattern that creates
asymmetry across a fixture set.

---

### D16 — Test-flake budget

**Decision**: **Zero documented flake budget** + **quarantine queue
capped at 10 tests**. Flaky tests get tagged
`#[ignore = "FLAKE-WATCH"]` (or equivalent for non-`#[test]`
harnesses) and tracked in `tools/flake-watch/issues.md`. Cap
exceedance (>10 entries in the queue) **blocks PR merges**.

**Lands in**: `tools/flake-watch/` scaffold in PR 0; new FR-051 in
`spec.md`.

**Rationale**: Documented flake percentage requires CI dashboarding
to measure (per-test re-run rate over time) — significant
infrastructure investment. Quarantine + cap converts flake into
explicit bookkeeping: known flakes are visible (in the queue), the
queue has a finite size, and exceeding the size blocks merges
forcing triage. This trades probabilistic budget for deterministic
queue management.

---

## PR 0 absorption summary

| # | Decision | PR-0 deliverable |
|---|---|---|
| D1 | R002 consumer-surface contract | `contracts/engine-pipeline.md` new R002 surfacing section |
| D2 | PR 3.7 alternate owner | `spec.md` Assumptions amendment |
| D3 | `marque --version` exposes schema | `contracts/audit-record.md` new section |
| D4 | Manual attestation only | `plan.md` PR 0 attestation template |
| D5 | R-8 decision tree binding | `spec.md` SC-010 wording; `research.md` R-8 amended |
| D6 | Rule-ID stability at PR 10 | `spec.md` new FR-049 |
| D7 | `threshold.toml` artifact | `tests/corpus/mangled/threshold.toml` scaffold |
| D8 | Cumulative drift + pinned hardware | `plan.md` Risk section + bench-runner pin |
| D9 | PR 9 → 9a / 9b / 9c | `research.md` new R-9 |
| D10 | Layer 0 + toolchain / trybuild pin | `contracts/engine-pipeline.md` test-strategy amended |
| D11 | Masking-pin cache-with-fallback | `research.md` new R-10 |
| D12 | `_unchecked` lint by signature shape | `research.md` new R-11; `spec.md` FR-040 amended |
| D13 | 8–18 band + qualitative gate | `plan.md` PR 3b acceptance criteria |
| D14 | Trait stabilization forcing function | `spec.md` Assumptions amendment |
| D15 | Fixture glob + count | `spec.md` US1 / US2 AC edits |
| D16 | Quarantine queue (cap=10) | `tools/flake-watch/` scaffold; `spec.md` FR-051 |

All 16 decisions lock at PR 0. Subsequent PRs execute against this
register; amendments require a follow-up PR editing this file.
