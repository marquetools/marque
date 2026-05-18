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

### D2 — PR 3.7 monolithic shape

**Decision**: PR 3.7 stays **monolithic** (no wave-split).

**Lands in**: `spec.md` Assumptions section amendment (PR 3.7 entry).

**Rationale**: Wave-splitting collides with PR 4's clean-break
property — PR 4 deletes `CapcoMarking::join`'s PageContext
delegation "with no equivalence shim," which forbids partial lattice
coverage. Cross-axis fixtures span categories (FOUO eviction depends
on classification AND dissem lattices) and cannot wave-isolate.
Monolithic shape preserves PR 4's clean break.

**Amendment (2026-05-13)**: The original D2 also required a **named
alternate owner** in the PR description as a bus-factor mitigation
for stall scenarios. That requirement is **retired** because marque
is a solo-driven project today — the bus-factor framing presupposed a
team context that doesn't apply. The alternate-owner gate served no
purpose when the same person is both primary and (de facto) alternate;
it would only add ceremony. Stall-recovery in the solo context
collapses to "PR sits open until the primary picks it back up." If
marque transitions to multi-contributor staffing the requirement can
be re-introduced via a follow-on amendment.

**Audibles permitted**: deadline slip beyond 2 weeks still warrants
self-review of scope and either an explicit re-commitment or a
strategic split. The "2 weeks" framing predates the solo-project
amendment but the underlying signal — "this PR has been open
unusually long; is something stuck?" — remains a useful sanity check
to self-apply.

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
TOML, not a freeform README). `scripts/bench-check.sh` reads it. The
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
the PR-0 baseline (R-5). Per-bench cumulative drift ≤10% is the
gate; per-PR contributions exceeding 6% are flagged for attribution.

**Bench-runner pin** (amended PR 0 review): the marque project runs
on standard GitHub Actions hosted runners (`ubuntu-latest`,
currently `ubuntu-24.04` — see
`https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md`).
Custom or rented bare-metal runners are out of budget. Bench-runner
"pinning" therefore degrades to:

- All bench captures run on `ubuntu-latest` GitHub-hosted runners.
- Image versions advance over the refactor's calendar window as
  GitHub rotates the runner pool; **the project explicitly accepts
  the resulting variance** (acknowledged by the bench-runner owner
  in the PR-0 review thread). Pinning to a specific image SHA
  would not eliminate variance because each build runs on a fresh
  shared VM with different co-tenants on the same image.
- The `bench_runner_owner` (D8 owner: `bashandbone`) is responsible
  for re-running the PR-0 baseline capture if a runner-image
  rotation produces clearly anomalous deltas, but is NOT obligated
  to reconcile every percent-level drift.

**Implications for FR-033 / FR-050 gates**:

- **FR-033** (>5% mean OR p99 regression backs out the originating
  change) — remains binding per-PR, with the standing caveat that a
  PR may legitimately re-test on a fresh runner if a single CI
  invocation produces a borderline reading. The
  `MARQUE_BENCH_SKIP_REGRESSION=1` escape hatch already documented
  in `scripts/bench-check.sh` is the canonical mechanism for the
  rare case of a confirmed runner-variance false positive.
- **FR-050** (cumulative drift ≤10% at PR 10) — the gate stays at
  the same threshold; whether shared-runner variance produces enough
  noise to make the gate flap or fail is empirical. The bench-runner
  owner has observed in prior project history that drift on this
  runner family routinely reaches 10% and "often tips into 11%" —
  that's a known baseline-quality signal, not necessarily a runtime
  regression. **Mitigation in the bench-runner owner's hands**:
  capture the PR-0 baseline by sampling at multiple times of day
  (including known-busy windows) and either (a) take the worst
  observed run as the baseline (conservative), (b) take the median
  across N captures (robust), or (c) take the slowest-decile
  per-bench across N captures (adversarial). The existing
  `scripts/capture-baselines.sh` runs ONE capture per invocation;
  multi-capture aggregation is currently a manual procedure the
  owner can re-run as needed. If the gate flaps in practice DESPITE
  a robust baseline, widen the tolerance in a follow-up amendment
  and document the runner-variance-attributed delta separately. Do
  NOT silently relax the gate without recording the rationale.

**Lands in**:
- `plan.md` Risk section / FR-033 enforcement note.
- PR 0 description records the bench-runner commitment as
  "GitHub Actions hosted `ubuntu-latest`, owner: bashandbone."

**Rationale**: hardware drift over the refactor's calendar window
(CI runner upgrades, kernel changes, runner capacity adjustments)
remains a real source of baseline shift independent of code. The
ideal mitigation is pinned hardware; the realized mitigation given
project budget is "same runner family, accept the variance, surface
clearly anomalous deltas via per-PR attribution." Per-PR attribution
keeps the diagnostic power of the gate even under variance — a 6%
single-PR contribution is detectable above runner noise; a 1% drift
that compounds across 10 PRs may not be, but is also less actionable.

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

> **Superseded 2026-05-07** (initial amendment) **and 2026-05-07**
> (PR-3b-numeric-band retirement). The original D13 "56 → 8–18
> post-PR-3b" target and "single CAPCO-§ citation per rule" wording
> below are retained for historical reference but **superseded by
> the two-pass Amendment 2026-05-07 below**. Operative interpretation:
> source count is **59** (not 56); the PR-3b-proper numeric band is
> **retired** (the literal sub-move retirements deliver ~38–44; the
> per-sub-PR principle is "drive the count down within the sub-move's
> primitive scope," not "hit a band"); end-state target ~10 surviving
> rules across all four stages stays binding; "single citation"
> applies **per declarative catalog entry**, not per `impl Rule`
> block. Read the amendment, not this block, for binding acceptance
> criteria.

**Decision (original, superseded 2026-05-07)**: 56 → **8–18 rule
count band** post-PR-3b + qualitative gate. Each surviving rule
MUST satisfy:

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

#### D13 — Amendment 2026-05-07 (consultation verdict)

The marque-lattice-consultant pass on PR 3b (recorded at
`docs/plans/2026-05-07-pr3b-consultation-verdict.md`; algebraic
justification in `marque-applied.md` §3 + §3.11) re-baselines and
re-sequences D13 without removing it.

**Re-baseline.** Source rule count is **59** (ground-truth
`grep -c '^impl Rule for' rules.rs rules_declarative.rs
rules_sci_per_system.rs`, not the "~56" approximation the
lattice-design plan carried).

**Re-sequence (initial 2026-05-07).** The 8–18 band was originally
intended as the **end-state** acceptance target (post-Stage-4); the
**PR 3b proper** target was originally **13–18**, with collapse to
9–11 staged across PR 3.7, PR 4, and PR 5+. The full staging table
and the six PR 3b sub-moves (3b.A–3b.F) are pinned in `plan.md` D13
addendum.

**Re-baseline (subsequent 2026-05-07, retiring the PR-3b numeric
band).** The planning pass on T026a (PR 3b sub-move A) found that the
literal sub-move retirements deliver −15 to −21 rules across
3b.A–3b.F, landing at **~38–44 post-3b** — outside any 13–18 band by
construction. The 13–18 figure was an aspirational projection that
assumed aggressive walker-style consolidation beyond what the
authorized primitives in 3b.A–3b.F permit (e.g., compacting all 14
`rules_declarative.rs` entries into a single walker, which the bridge
§3.4 does not prescribe). Rather than relax the primitives' scope or
shed declarative-catalog discipline, the band itself retires. The
operative gate becomes per-sub-PR: each sub-move drives the count
down within what its authorized primitive scope (the bridge §3.4
moves) permits, declares the math in its PR description, and earns
team review only when retirements stray outside the bridge's
primitives. The end-state target stays at **~10 surviving rules**
across all four stages; Stage 3 (PR 4 per-category Lattice impls)
and Stage 4 (PR 5+ renderer) carry the heavy lifting toward that
target. The Stage-by-Stage expected ranges (~38–44 → ~32–40 →
~14–22 → ~10) live in `plan.md` D13 addendum as guidance, not
acceptance gates.

**Resolved sub-decisions**:

- **Q-3.9** ("single citation per rule" — does it mean per `impl Rule`
  block or per declarative entry?) → **per declarative entry**.
  Consolidated walkers (3b.A banner, 3b.C RELIDO, 3b.D floors, 3b.E
  SCI per-system) are each one `impl Rule` block delegating to a
  catalog whose rows each carry their own §-citation. Citation
  integrity per Constitution VIII is per-claim, not per-block.
- **Q-3.4.2-timing** (where does the family-predicate `Constraint::
  Conflicts::RhsFamily(predicate)` variant land?) → **fold into
  PR 3.7**. The lattice §-resolution spike already touches
  `marque-scheme`; one variant addition fits. PR 3b ships the
  enumerated form (~15–20 single-token rows); PR 4 compacts to 2
  family rows.
- **Q-4.7-timing** (where does the `marque-applied.md` §4.7 closure
  operator primitive land?) → **fold into PR 3.7**. Same reason.
  The implication tables and `proptest_closure.rs` ship with the
  primitive; PR 4 wires CAPCO's `ClosureRule` catalog and
  re-classifies closure-implied entries. (Catalog shape pivoted
  2026-05-11 from private `ImplTable<S>` to public `ClosureRule` —
  see D18.)
- **Q-Move-7-timing** (where does style/ordering → renderer move
  land?) → **PR 5+, with a single fallback walker retained in
  PR 3b**. The renderer trait surface is a separate effort; PR 3b
  retains one "non-canonical input" diagnostic walker covering
  E020 / E023 / E028 / E033 ordering checks until the renderer
  arrives.
- **Q-FgiSet-vs-§4.8** (does existing `FgiSet::Present { concealed,
  countries }` already model the §4.8 consensus-or-fallback pattern?)
  → **yes** (user confirmation 2026-05-07): the FGI category is the
  union of acknowledged foreign-government trigraphs unless any
  portion is source-concealed, in which case the banner falls back to
  bare `FGI`. The existing `FgiSet` join law (`concealed=true ∨ x =
  concealed=true`; `{countries: A} ∨ {countries: B} = {countries: A
  ∪ B}`) implements this exactly. **No new primitive needed**; T108d
  collapses to **doc-comment amendment only** at
  `crates/capco/src/lattice.rs` citing `marque-applied.md` §4.8 +
  CAPCO §H.7 / §H.7 p123.

**Open sub-decisions** (do NOT block PR 3b; flagged for follow-up):

- **Q-3.4.6a** (class-floor catalog: build-time generated from
  CVE/Schematron metadata, or hand-curated in `marque-capco`?) →
  **hand-curated in PR 3b**, with a 30-minute spike to inspect ODNI
  XML coverage tracked as a follow-up. If the schema carries floor
  data uniformly, swap to build-time at the next ODNI schema bump.

**Audibles permitted**: PR 3b reviewer may decline to land 3b.A
(banner walker) if the per-category Lattice impls landing in PR 4
preserve correctness via property-test-only coverage; in that case
the walker is skipped at 3b and the 5 banner-roll-up rules retire
in PR 4 directly. Documented as a 3b reviewer choice in the PR
description.

**Lands in**:
- `plan.md` D13 addendum (re-sequenced staging table + sub-moves).
- `tasks.md` T026 expansion to T026a–T026f; new T108b/T108c/T108d under
  PR 3.7 for `RhsFamily` variant + closure operator primitive +
  §4.8 `FgiSet` doc-comment amendment.
- `docs/plans/2026-05-07-pr3b-consultation-verdict.md` — dated
  decision record with the (a)/(b)/(c) verdicts.
- `marque-applied.md` §3.10.3 timing correction + new §3.11 stage-
  sequencing section.
- `docs/plans/2026-05-01-lattice-design.md` §10 amendments
  (closure operator + RhsFamily as PR 3.7 fill-in items).

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

- US1 AC1: `tests/corpus/mangled/sci_compartment_*.txt` +
  ≥1 fixture demonstrates the scenario.
- US2 AC1: `tests/corpus/foreign/pure_foreign_*.txt` + ≥1 fixture
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

### D17 — PR 3b.C scope correction: RELIDO Conflicts roster

**Decision**: PR 3b.C ships exactly **4** `Constraint::Conflicts` rows
(E054–E057), not the ~15–20 rows projected in the 2026-05-07 consultation
verdict line 82. The broader §3.4.2 family roster (RELIDO ⊥ {LES-NF,
SBU-NF, each FGI atom, each JOINT atom, each NATO atom}) is deferred to
**PR 3.7 (T108b)** where `Constraint::Conflicts::RhsFamily(predicate)` ships.

**Rationale (Constitution VIII)**: Re-verification of CAPCO-2016 against
the consultant's `marque-applied.md §3.4.2` roster surfaces only four pairs
with direct, re-traceable §-passage authority:

| LHS | RHS | Primary citation | CAPCO-2016.md line |
|-----|-----|------------------|--------------------|
| RELIDO | NOFORN | §H.8 p154 | 3808 |
| RELIDO | DISPLAY ONLY | §H.8 p154 | 3808 |
| ORCON | RELIDO | §H.8 p136 | 3363 |
| ORCON-USGOV | RELIDO | §H.8 p140 | 3444 |

The remaining ~11–16 pairs are structural inferences (the consultant's
"IDO has no authority over foreign equity" argument) without a verbatim
§-passage saying "may not be used with RELIDO." Constitution VIII §3
prohibits embedding citations that cannot be traced to a real passage —
fabricating fifteen specific §-citations would be a correctness defect.

At PR 3.7 the `RhsFamily(predicate)` variant ships. A single
family-predicate row (or two — one per grouping) can carry the structural
argument with one well-documented citation chain explaining the
IDO-vs-foreign-equity reasoning, satisfying Constitution VIII without
fabricating per-atom citations.

**Net rule delta**: 57 → 61 (+4). Net constraint delta: 15 → 19 (+4).

**Subtractive-fix direction (PM Addendum II, 2026-05-07; confidence
calibration 2026-05-08).** All four wrappers emit a `FixProposal` that
**removes RELIDO** from the dissem block (replacement = `""`, confidence
= 0.95, `FixSource::BuiltinRule`, `Severity::Error`). The 0.95 value
clears the engine's default `Config::confidence_threshold = 0.95`
(`crates/config/src/lib.rs:156`; auto-apply gate is `confidence >=
threshold`) so the fix auto-applies under default config — matching the
user-stated guidance behavior ("remove RELIDO and tell them why"). The
initial PM Addendum II §2 value of 0.9 was calibrated up after verifying
the threshold default; the 0.85–0.9 tier is reserved for conditional /
lower-confidence cases (`crates/capco/src/rules.rs:4465 / :4602 / :4962
/ :5173`), and 0.95 matches the established CAPCO convention for
definite, at-threshold, auto-apply fixes (`crates/capco/src/rules.rs:998
/ :1327 / :2622 / :2777 / :2853`). RELIDO is the unambiguous
remove-target because the
other token in each pair carries the binding §-cited authority (NOFORN
dominates per FD&R supersession; DISPLAY ONLY is a positive disclosure
decision; ORCON / ORCON-USGOV explicitly assert "may not be used with
RELIDO" on their §H.8 templates). The pattern applies to **dissem-axis
`Constraint::Conflicts`** rules only — non-dissem conflicts (classification
E012, JOINT cross-system, SCI grammar) remain "user resolves" because the
fix direction cannot be inferred without policy input. Constitution V
(audit-first) is preserved: `FixProposal` is pure data; the engine
snapshots runtime state into `AppliedFix` at promotion. See PM Addendum II
in `docs/plans/2026-05-07-pr3b-C-relido-conflicts-plan.md` for the full
rationale and user-correction context (Marque is a guidance tool for
dissem markings, not just a checker).

**Verdict-line-82 amendment**: see `docs/plans/2026-05-07-pr3b-consultation-verdict.md` line 82 (amended in this PR).

**Lands in**: PR 3b.C implementation, helper `compute_relido_removal_span`
in `crates/capco/src/rules_declarative.rs`, test count pin in
`crates/capco/tests/relido_conflicts.rs:capco_constraints_count_after_pr3b_c`,
and the helper-position tests
(`helper_first_position_consumes_trailing_slash`,
`helper_middle_position_consumes_preceding_slash`,
`helper_last_position_consumes_preceding_slash`,
`helper_returns_none_when_relido_absent`).

---

### D18 — T108c catalog shape: public `ClosureRule` (Option C), not private `ImplTable<S>`

**Decision**: PR 3.7 T108c ships the §4.7 closure operator as a
**public** catalog primitive `ClosureRule` in `marque-scheme` (sibling
to `Constraint`), accessed via a new `MarkingScheme::closure_rules()
-> &[ClosureRule]` trait method. The private `ImplTable<S>` /
`ImplRow<S>` shape pinned in the 2026-05-07 consultation verdict
(line 113–114, item 5 — "trait shape pinned to α with default no-op;
`ImplTable` as `&'static [ImplRow<S>]`") is **retired in favor of
Option C**: the closure rules are first-class catalog data, not an
engine-implementation-detail private structure.

**Rationale**:

1. **The bridge §3.0.b structure-vs-constraint distinction reads
   cleanly as two parallel catalogs.** Phase A "structure rules"
   (closure-shaped — adds facts when triggers fire) become
   `&[ClosureRule]`. Phase B "constraint rules" (validation —
   diagnoses violations of structural invariants) stay
   `&[Constraint]`. Both are inspectable by tooling
   (scheme-exploration UI, docs generator, catalog audit), both
   per-scheme, both data-form. A private `ImplTable<S>` would hide
   the closure rules from tooling and conflate "engine
   implementation detail" with "scheme's declared semantics."

2. **No fn-pointer trigger / suppressor bodies are needed for the
   CAPCO catalog.** The 2026-05-07 pin specified `ImplRow = {
   trigger: fn(&marking) -> bool, cone: ConeBuilder, suppressor:
   fn(&marking) -> bool }` — function pointers for the predicate
   bodies. Walking the bridge §4.7.1 implication list shows every
   row reduces to "presence of any token in a fixed set" (n-ary OR
   over `TokenRef`s); the function-pointer escape hatch is
   unnecessary. `triggers: &'static [TokenRef]` + `suppressors:
   &'static [TokenRef]` carry the same information in a
   data-inspectable form. If a future scheme needs a
   non-presence-shaped trigger, an `fn`-pointer variant can be
   added then (YAGNI).

3. **`Constraint::Implies` semantics are NOT promoted in this
   pivot.** `Constraint::Implies` remains a diagnostic-suppression
   hint (its current job — "if left is present, right is implied;
   the engine skips false missing-X diagnostics"). The closure
   operator's fact-propagation work flows through `ClosureRule`,
   not through `Implies`. The two catalogs are independent.
   **Superseded 2026-05-11 by D19 C**: a follow-on design pass
   surfaced that closure-first evaluation makes `Constraint::Implies`
   dead code — the implied fact is propagated before validation
   runs, so "missing X" never fires — and that the variant has
   zero in-tree CAPCO catalog rows. D19 C retires the variant
   cleanly.

4. **Engine call site is unchanged.** PR 4 (T112+) wires
   `Engine::project` to call `scheme.closure(marking)` per the
   §4.7.4 pipeline; the default `closure()` impl walks
   `closure_rules()` to fixpoint. Engine code doesn't need to know
   the catalog shape.

5. **Shared-suppressor design** (Q-4.7-Cl_supp resolution) is
   preserved verbatim — `FDR_DOMINATORS: &'static [TokenRef] = ...`
   is referenced by every trio row that shares the FD&R suppressor.
   One source of truth; rows reference by `&'static` slice
   identity, not by row-duplication.

**Decision pass context**: surfaced 2026-05-11 during a
lattice-consultant Topic-2-variant-shape exploration (Topics 1 + 2
in the closure-FCA-discuss worktree). User selected Option C +
delete `ImplTable` after walking three variant-shape alternatives
(A: enumerated single-trigger rows with shared suppressor pointer;
B: first-class n-ary variant inside `Constraint::*`; C: separate
catalog alongside `Constraint::*`). The consultant verdict file
captures the analysis.

**What changes for T108c**: trait method signatures + `ClosureRule`
type definition + `closure_rules()` content (no `ImplRow<S>` /
`ImplTable<S>` types introduced). Property-test obligations and
the implicit-default trio data are unchanged. Class-floor entries
still STAY in `Constraint::Custom` per §4.7.5.

**What changes for PR 4**: wiring reads from `scheme.closure_rules()`
or calls `scheme.closure()` (both public) at the §4.7.4 pipeline
slot. Closure-implied entries that were `Constraint::Custom` /
`Requires` rows in PR 3b flip to `ClosureRule` rows; the count
delta projection in §3 of the 2026-05-07 consultation verdict
(stage 2 row: "~−5 to −8 implication-shaped Requires entries flip
to closure entries") is unchanged.

**Lands in**:

- `tasks.md` T108c amended to specify `ClosureRule` (not
  `ImplTable<S>`); the trait surface gains `closure_rules()`
  alongside `closure()`.
- `docs/plans/2026-05-01-lattice-design.md` §9 amended.
- `docs/plans/2026-05-07-pr3b-consultation-verdict.md` §5 item 5
  ("trait shape pinned to α") annotated as superseded by D18.
- `docs/plans/2026-05-07-pr3b-C-relido-conflicts-plan.md` §8
  forward-reference updated.
- `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md` §2
  "stable ImplTable shape" wording updated to `ClosureRule`
  catalog.

---

### D19 — Topic 1 design pass: `AuditNote` shape + per-row closure severity + `Constraint::Implies` retirement

**Decision**: Three coordinated sub-decisions land alongside T108c's
`ClosureRule` catalog (D18), folded into PR 3.7 as sibling tasks
T108e / T108f / T108g.

- **A. `AuditNote` type** (T108e) — new audit-stream record for
  `ClosureRule` firings. Lives in `marque-rules` alongside
  `AppliedFix`. Carries `rule: RuleId`, `citation: &'static str`,
  `kind: AuditNoteKind`, engine-snapshotted runtime state
  (`timestamp`, `classifier_id`, `dry_run`), and a structural-only
  payload `AuditNoteStructural { row_name, cone: &'static [TokenId],
  scope: Scope, span: Option<Span> }`. G13 invariant preserved by
  construction: no document bytes traverse the audit pipeline —
  only TokenIds, byte offsets, and catalog row identifiers.
  Engine-promoted with `__engine_promote`-shaped sealing mirroring
  `AppliedFix` (same Constitution V Principle V scope; same
  test-fixture carve-out). Separate NDJSON line type
  (`{"type":"audit_note", ...}`) distinct from
  `{"type":"applied_fix", ...}`.

  `AuditNoteKind` is closed-set; **v1 ships `InferredFact` only**.
  Additional kinds (`SuppressedByFact`, `DisabledByConfig`) are
  deferred to a debug-tracing follow-up — engineer-facing tools,
  not load-bearing for compliance.

  `Confidence` propagates from the underlying parse/recognition
  into `AuditNote` (mirrors `AppliedFix.confidence`) so downstream
  audits can ask "how confident was the recognition step that fed
  this closure firing?"

- **B. Per-row severity for `ClosureRule` rows in `.marque.toml`**
  (T108f) — new `[closure_rules]` table in `.marque.toml`,
  **separate from `[rules]`**, keyed by `ClosureRule.name` (e.g.,
  `"capco/noforn-if-no-fdr"`). Same `Severity` enum and same
  per-row override mechanic as `[rules]`, but a distinct section
  rather than a shared keyspace — `RuleId` explicitly supports
  slash-containing IDs per the `crates/rules/src/lib.rs:84` doc
  ("E001", "capco/portion-mark-in-banner"), so a string-shape
  disambiguation (slashes vs bare alphanumeric) would not hold.
  Section isolation eliminates the collision risk entirely while
  preserving the "same severity override surface" the user
  requested (same enum, same per-row mechanic, same default-
  fallback semantics).

  Map per-row severity to closure-row semantics:

  | Severity | Behavior |
  |---|---|
  | `Off` | Row disabled; no firing; no propagation; no `AuditNote` |
  | `Suggest` | Fires; propagates; `AuditNote` at suggestion level |
  | `Info` | (default) Fires; propagates; `AuditNote` at info level |
  | `Warn` | Fires; propagates; `AuditNote` + `Diagnostic` (warn) surfaces |
  | `Error` | Fires; propagates; `AuditNote` + `Diagnostic` (error) surfaces |
  | `Fix` | **Rejected at config load** — closure firings are not byte-level fixes; load-time error points the user at `Info` / `Warn` / `Error` |

  Default per row is `Info`. Catalog rows declare
  `default_severity: Severity` on the `ClosureRule` struct itself
  (always present per T108c — closure rows are severity-aware at
  the catalog level; typically initialized to `Severity::Info`).
  The runtime override surface in `.marque.toml` reads the
  `[closure_rules]` table first; absent → falls back to
  `ClosureRule.default_severity`. `[rules]` is NOT consulted for
  closure-row severity — the section split is total, consistent
  with the keyspace-collision rationale above.

  Surface at `Warn` / `Error` produces a `Diagnostic` *in addition
  to* the `AuditNote` from T108e — the two streams serve different
  consumers (compliance reviewer vs. content author) and are not
  conflated.

- **C. `Constraint::Implies` retirement** (T108g) — the variant
  becomes dead code in the post-T108c world: `ClosureRule`
  propagates the implied fact; downstream `Requires` checks
  evaluate against the *closed* marking; "missing X" false
  positives disappear automatically. Grounded in code: zero
  in-tree `Constraint::Implies` catalog rows in CapcoScheme
  today (verified 2026-05-11). Only usage is the evaluator test
  stub at `crates/scheme/tests/evaluator.rs:301`.

  Retirement is a five-site surgical change (see T108g for the
  call-site list). No CAPCO catalog data touched. Marque is
  pre-users; no deprecation phasing per
  `feedback_pre_users_no_deprecation_phasing.md`.

  This narrowly supersedes D18 rationale bullet 3 — D18 preserved
  `Implies` because the catalog-shape pivot did not surface the
  redundancy question. D19 surfaces it: the redundancy IS
  terminal, retire cleanly.

  **Speculative-preservation rejected**: the only non-fact-
  propagation reading of `Implies` worth naming is "left implies
  right is the *preferred* rendered form but not required" — a
  styling / codec preference, not a structural constraint. That
  semantic belongs on a `Codec` / renderer trait surface
  introduced at PR 5+, not on `Constraint`. Don't preserve
  `Implies` today against hypothetical future use; re-add a
  purpose-built trait surface when the use case actually arrives.

**Rationale**:

1. **User-stated priors** (2026-05-11 design pass): "lean
   AuditNote type; same severity override surface" — directly
   drives sub-decisions A and B.
2. **G13 audit-content-ignorance preserved by construction** for
   sub-decision A — `AuditNote` payload is structural only
   (TokenIds, byte offsets, catalog row identifiers).
3. **Closure-first evaluation makes `Implies` dead code** —
   propagation runs before validation; suppression is automatic.
4. **No user-visible breaking change** — marque is pre-users; the
   `Constraint::Implies` variant has no production catalog rows
   to migrate.

**Decision pass context**: surfaced 2026-05-11 immediately after
D18, in the same lattice-consultant Topic-1 design pass
(closure-fca-discuss worktree). User selected option (1) —
"approve A/B/C as written" — after the design pass laid out the
three sub-decisions, the eight open questions, and the
recommendations.

**Lands in**:

- `tasks.md` T108e (`AuditNote` type + emission), T108f (per-row
  severity for `ClosureRule`), T108g (`Constraint::Implies`
  retirement) — three new sibling tasks under PR 3.7.
- This `decisions.md` D19 entry (above) + D18 rationale bullet 3
  supersession annotation (history preserved).

---

## D20 — S007 / NATO-closure-row layer separation (PR 4b-D)

**Decision**: When the NATO closure row activates on the hot path
in PR 4b-D, it injects `REL TO USA, NATO` silently at
`Severity::Info` at the lattice layer. S007
(`bare-nato-requires-rel-to-usa-nato`, `crates/capco/src/rules.rs:3578+`)
stays as the **text-layer surface** — `Severity::Suggest` with the
visible portion-edit byte diff (`(//NS)` → `(//NS//REL TO USA, NATO)`).

**Rejected alternatives**:

- **(b) Parallel `Severity::Suggest`** at the lattice layer —
  doubles the audit surface for the same inference (same fix
  proposed twice with different `source` fields).
- **(c) Inject NOFORN** per §B.3 Table 2 p21 (FGI-rule conforming) —
  contradicts the §H.7 p127 Notional Example 2 worked-example
  interpretation that motivates S007. User concern verbatim
  (2026-05-17): "(//NS) should never be NF".

**Why**:

1. **No double-audit on the same inference**. Closure-layer Info
   = lattice fact propagation (banner state); S007-layer Suggest
   = author-visible byte diff. Single concept per layer.
2. **Authority asymmetry preserved**. §H.7 p127's interpretive
   weight is example-derived, not MUST-prose; S007 ships at
   `Suggest` (confidence 0.85) precisely because the manual has
   no explicit prose mandating the implicit REL TO USA, NATO
   inference. Closure-layer `Info` matches that authority
   posture without claiming higher confidence than the source
   warrants.
3. **Solely-NATO carve-out preserved**. S007 clause 3 already
   silences the rule on solely-NATO docs (alliance ownership
   implicit). The closure row's Info-level injection on those
   same pages is structurally invisible to the audit-noise
   profile because Info diagnostics don't surface in the
   default `check` output the way `Suggest` diagnostics do.
4. **NOFORN-guard ownership preserved**. S007 clause 4 already
   defers to the `capco/noforn-conflicts-rel-to` page rewrite
   on portions carrying NOFORN. The closure row inherits the
   same conflict-resolution path via the rewrite scheduler — no
   redundant guard.

**Authority**: CAPCO-2016 §H.7 p127 Notional Example 2 (NATO
REL TO worked example, S007's authority base); §H.8 p145 (NOFORN
domination — owns the conflict path); §B.3 Table 2 p21 (the
FD&R-roster row rejected as the calibration target per
Constitution VIII — drawing NOFORN here would over-translate the
table-row authority).

**Lands in**:

- PR 4b-D NATO closure row construction (`crates/capco/src/scheme/closure.rs`
  CLOSURE_REL_TO_USA_NATO row, default_severity `Severity::Info`).
- S007 left untouched in PR 4b-D (existing `Severity::Suggest`
  default preserved).
- Issue #508 calibration question marked resolved.

---

## D21 — Closure-rule open-vocab cone shape: B3 sibling field (PR 4b-D.0)

**Decision**: Extend `marque_scheme::ClosureRule` with an optional
sibling field `cone_derived: Option<fn(&S::Marking) -> SmallVec<[FactRef<S>; 2]>>`
to express marking-derived cones (JOINT's partner-list-floor case).
The existing `cone: &'static [TokenRef]` field stays unchanged.

> **Addendum (2026-05-17, post-Copilot review of PR #514)**: this
> entry originally typed the derived cone as `SmallVec<[(CategoryId,
> TokenRef); 2]>`. Copilot review on PR #514 (the 4b-D.0 implementation
> PR) identified that the `TokenRef` carrier cannot express open-
> vocabulary facts — JOINT's `REL TO USA, GBR, JPN` partner-list cone
> needs `FactRef::OpenVocab(CapcoOpenVocabRef::CountryCode(_))` per
> the established pattern at `crates/capco/src/rules_declarative.rs:711-718`.
> The signature was redesigned to return `SmallVec<[FactRef<S>; 2]>`,
> dropping the pre-bound `CategoryId` (the closure executor now calls
> `scheme.category_of(&fact_ref)` to route — symmetric with the static
> path, which calls `scheme.token_category(token_id)`). All other D21
> reasoning — sibling field vs. enum, SmallVec inline-2 sizing, zero-
> touch on the 7 `CLOSURE_NOFORN_*` rows, sequencing — stands.

Concrete shape (post-addendum):

```rust
pub struct ClosureRule<S: MarkingScheme + ?Sized> {
    pub name: &'static str,
    pub label: &'static str,
    pub triggers: &'static [TokenRef],
    pub suppressors: &'static [TokenRef],
    pub cone: &'static [TokenRef],
    pub cone_derived: Option<
        fn(&S::Marking) -> smallvec::SmallVec<[FactRef<S>; 2]>
    >,
    pub default_severity: Severity,
}
```

`ClosureRule<S>` becomes generic over the scheme — unavoidable: any
shape that lets the cone read `S::Marking` requires it. The `?Sized`
bound mirrors `FactRef<S>`'s bound at
`crates/scheme/src/fix_intent.rs:63`; `Debug` / `Clone` are written
manually rather than derived so the bounds resolve through the
struct's fields without over-constraining to `S: Debug + Clone`
(the `CapcoScheme: !Clone` constraint would otherwise silently
prevent `ClosureRule<CapcoScheme>` from being cloned).

**Rejected alternative B2 (enum-replace `cone`)**:

```rust
// Rejected
pub enum ConeFact<S: MarkingScheme> {
    Static(TokenRef),
    DerivedFromMarking(fn(&S::Marking) -> SmallVec<...>),
}
pub struct ClosureRule<S: MarkingScheme> {
    pub cone: &'static [ConeFact<S>],
    ...
}
```

**Why B3 over B2**:

| Axis | B2 (enum) | B3 (sibling) |
|---|---|---|
| 7 shipped `CLOSURE_NOFORN_*` rows | Rewrap every cone entry as `ConeFact::Static(...)` | Zero touch |
| Closure executor hot path | Enum dispatch per row, every row | `cone_derived.is_some()` branch predicts to cold side (1 of 8 rows in CAPCO once JOINT lands) |
| Future scheme catalogs (CUI, NATO domain, partner-national) | Every static row pays enum dispatch | Static fast path stays `&[TokenRef]` walk |
| `ClosureRule<S>` generic | Required | Also required (`fn` refs `S::Marking`) — cost is shared |
| PR 4b-D.0 blast radius | Migrates every consumer + every catalog row | Additive `None`-default field; cones unchanged |
| YAGNI posture | Designs for one open-vocab consumer (JOINT) | Defers enum dispatch until a second open-vocab consumer surfaces |

**SmallVec inline-2 rationale**: matches the `marque-scheme`
`ReplacementIntent::FactRemove::facts` inline-2 `SmallVec` precedent
per issue #348 verbatim — keeping the cap aligned with the existing
in-tree convention is the right baseline. JOINT's typical partner
list (1-5 countries per §H.3 worked examples) will spill to the
heap for ≥3 entries; the doc-comment on `cone_derived` records
the explicit "bump to inline-4 or inline-8 if the eventual JOINT
row routinely produces ≥3 facts per firing" follow-up. `smol_str`
does NOT apply — `FactRef<S>` carries closed-CVE `TokenId` or
typed open-vocab refs (`S::OpenVocabRef`), no raw strings on the
cone-fact path.

**Sequencing implication (Constitution VII §IV)**:

PR 4b-D.0 (the engine-gap PR) lands first:

1. `ClosureRule<S>` generic propagation through `marque-scheme`
   and every consumer (engine executor wiring defers — no
   production caller exists in PR 4b-D.0; the catalog is
   inspected via `MarkingScheme::closure_rules()` only)
2. `cone_derived: Option<fn(...) -> SmallVec<[FactRef<S>; 2]>>`
   field defaulting `None`
3. Existing 7 `CLOSURE_NOFORN_*` rows zero-touch — only the type
   parameter propagates through the catalog (no rule uses
   `cone_derived`; the field is `None` everywhere)
4. Proves green against the corpus regression harness; no
   `CapcoScheme` semantic change

THEN PR 4b-D consumes it: NATO closure row (static cone) +
JOINT closure row (`cone_derived`) + FGI closure row (static
cone) + closure runtime activation + hot-path flip + S007
calibration per D20.

**Authority**: Constitution Principle VII §IV last paragraph —
"A scheme-adoption PR MUST NOT edit the engine crates
(`marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`,
`marque-ism`). If the scheme reveals an engine gap, the gap is
fixed first in a separate PR..."

**Lands in**:

- PR 4b-D.0 (new engine-gap PR): `marque-scheme::ClosureRule`
  shape change (`<S>` generic + `cone_derived` field), generic
  propagation through `marque-capco`'s closure catalog, smallvec
  dep already in `marque-scheme` workspace (no new dep). Engine
  executor wiring defers to PR 4b-D (no production caller for
  the closure operator exists in 4b-D.0).
- Issue #508 scope item 3 (open-vocab cone primitive) marked
  resolved with the B3 choice
- This `decisions.md` D21 entry (above)

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
| D13 | Qualitative per-declarative-entry gate; PR-3b numeric band retired 2026-05-07; end-state target ~10 surviving rules across stages 1–4 | `plan.md` PR 3b acceptance criteria + D13 two-pass amendment 2026-05-07 |
| D14 | Trait stabilization forcing function | `spec.md` Assumptions amendment |
| D15 | Fixture glob + count | `spec.md` US1 / US2 AC edits |
| D16 | Quarantine queue (cap=10) | `tools/flake-watch/` scaffold; `spec.md` FR-051 |
| D17 | PR 3b.C scope correction: RELIDO Conflicts roster pruned from ~15–20 to 4 rows under Constitution VIII; broader §3.4.2 family roster deferred to PR 3.7 T108b | `crates/capco/tests/relido_conflicts.rs` count pin; verdict line 82 amended |
| D18 | T108c catalog shape: public `ClosureRule` (Option C), not private `ImplTable<S>` | `tasks.md` T108c amended; `2026-05-07-pr3b-consultation-verdict.md` §5 item 5 superseded; `decisions.md` Q-4.7-timing wording updated |
| D19 | Topic 1 design pass: `AuditNote` audit-stream record + per-row severity for `ClosureRule` + `Constraint::Implies` retirement (narrowly supersedes D18 rationale bullet 3) | `tasks.md` T108e/T108f/T108g added; D18 bullet 3 annotated |
| D9b-1 | Two parallel slice fields for `dissem_us` / `dissem_nato` | PR 9b T132 shipped two `Box<[DissemControl]>` fields per FR-046. Future cross-system translation (memory `project_cross_system_translation.md`) and a hypothetical third namespace (FVEY-only, partner-national) would be cleaner with `Box<[NamespacedDissem]>`. Owner reviewed and chose to defer; revisit in PR 10+ if cross-system translation work surfaces the smell as concrete blocking pain. Reference: PR 9b preflight, 2026-05-14. |
| D20 | S007 / NATO-closure-row layer separation (PR 4b-D): closure injects `REL TO USA, NATO` silently at `Severity::Info` (lattice layer); S007 stays as the visible `Severity::Suggest` text-layer surface. Authority asymmetry preserved; option (c) NOFORN-injection rejected per user-stated invariant "(//NS) should never be NF" + §H.7 p127 worked-example interpretation. | `decisions.md` D20 (above); resolves issue #508 calibration question; PR 4b-D NATO closure row construction. |
| D21 | Closure-rule open-vocab cone shape: B3 sibling field `cone_derived: Option<fn(&S::Marking) -> SmallVec<[FactRef<S>; 2]>>` selected over B2 enum-replace. `ClosureRule<S>` generic required either way; B3 leaves the 7 shipped `CLOSURE_NOFORN_*` rows zero-touch and keeps the closed-vocab hot path tight. Return type is `FactRef<S>` (not `(CategoryId, TokenRef)`) so the derived path covers open-vocab facts like JOINT's REL TO partner-list — addendum applied post-Copilot review on PR #514, see D21 entry. SmallVec inline cap matches the `marque-scheme` `ReplacementIntent::FactRemove::facts` inline-2 precedent from #348; bump to inline-4 / inline-8 is a one-line change if the eventual JOINT row routinely produces ≥3 facts per firing. PR 4b-D.0 lands the trait change ahead of PR 4b-D per Constitution VII §IV. | `decisions.md` D21 (above); resolves issue #508 scope item 3; PR 4b-D.0 (new engine-gap PR) trait-surface change. |
| D22 | NOFORN-supersession at FactAdd injection site: when `apply_fact_add` inserts `NOFORN` into `CAT_DISSEM`, route through `DissemSet::with_noforn_injected` so the §H.8 p145 supersession overlay strips dominated FD&R controls (REL TO / RELIDO / DISPLAY ONLY / EYES ONLY) at the injection site. Pre-PR-4b-D.2 the path appended `Nf` to `dissem_us` without re-applying overlays, leaving `{Nf, Displayonly}` / `{Nf, Relido}` in the bag — invalid per §H.8 p145. Post-fix the injection is correct by construction, idempotent under re-insertion, and works equally for closure-driven and rule-driven FactAdd paths. Authority: §H.8 p145 + §D.2 Table 3 rows 1-2 + §H.8 p157. | `decisions.md` D22 (above); PR 4b-D.2 commit 3; `crates/capco/src/scheme/actions/intent.rs::apply_fact_add` CAT_DISSEM branch. |
| D23 | Closure-rewrite-application sentinel placement: the `#[cfg(debug_assertions)]` read-only-attrs sentinel for the closure operator's rewrite-application site lives inside `CapcoScheme::project(Scope::Page \| Document \| Diff, ...)` between the `join_via_lattice` composition and the closure invocation. Snapshots the raw per-portion `CanonicalAttrs` slice; asserts byte-identity after `closure()` returns. Sibling to the existing `dispatch_page_finalization` PageFinalization-rule sentinel (engine.rs); together they pin the §3 (e.1) read-only-attrs invariant across both engine-facing consumer surfaces (scheme-side projection + engine-side rule dispatch). Authority: `docs/plans/2026-05-01-lattice-design.md` §3 (e.1) read-only-attrs invariant. | `decisions.md` D23 (above); PR 4b-D.2 commit 3; `crates/capco/src/scheme/marking_scheme_impl.rs::project` Scope::Page arm. |

D1–D16 lock at PR 0. D17 / D18 / D19 / D9b-1 / D20 / D21 / D22 / D23
are post-PR-0 implementation decisions: D17 is a PR 3b.C scope
correction amending a consultation verdict projection; D18 is a
PR 3.7 T108c catalog-shape pivot from the 2026-05-07 trait-shape
pin to a public `ClosureRule` catalog (Option C); D19 is the
Topic 1 design pass that lands `AuditNote`, per-row severity for
`ClosureRule`, and the `Constraint::Implies` retirement as sibling
T108e/f/g tasks under PR 3.7; D9b-1 is the PR 9b T132 dissem-split
shape choice (two-parallel-fields over namespaced-tuple); D20 is
the PR 4b-D S007 / NATO-closure-row layer-separation calibration;
D21 is the PR 4b-D.0 / PR 4b-D `ClosureRule` open-vocab cone shape
(B3 sibling field over B2 enum-replace); D22 is the PR 4b-D.2
NOFORN-supersession-at-injection-site fix; D23 is the PR 4b-D.2
closure-rewrite-application sentinel placement (sibling to the
existing PageFinalization-rule sentinel). Subsequent PRs execute
against this register; amendments require a follow-up PR editing
this file.
