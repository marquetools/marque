# PM Decisions: PR 4b Umbrella Closeout

**Date**: 2026-05-19
**Branch**: `refactor-006-pr-4b-closeout` (worktree `/home/knitli/marque/.claude/worktrees/pr-4b-closeout/`, off `origin/staging@5d3415cd`)
**Base PR**: `staging`
**Status**: LOCKED 2026-05-19 — implementer proceeds with this contract.

**Predecessor plans**:
- `docs/plans/2026-05-19-pr4b-closeout-architect-plan.md` (system-architect preflight)
- `docs/plans/2026-05-19-pr4b-closeout-rust-preflight.md` (ecc:rust-reviewer preflight)

**Template reference**: `docs/plans/2026-05-08-pr3b-closeout-T027-T028-T029-plan.md` (PR 3b umbrella closeout — the canonical bookkeeping-closeout shape this PR mirrors)

---

## ERRATA / corrections (applied 2026-05-19 after first implementer pre-flight verification)

The first implementation agent verified counts at HEAD and surfaced three drift-points in this doc:

1. **Task IDs T130-T134 are TAKEN** — `tasks.md:399-406` has T130 (PR-8 masking-pin removal) + T131-T134 (PR-9 parser/dissem/banner/ATOMAL work). Free range is **T142-T146**. References in §1 OQ-6 and §3.5 and §7 are corrected below.

2. **`Constraint::Custom` count at HEAD = 39, not ~36.** The five SCI-per-system + 27 class-floor + 7 core-catalog Custom entries total 39. The RELIDO E054-E057 wrappers are `Constraint::Conflicts`, NOT `Constraint::Custom` — they are NOT counted in the `post_4b_lattice_inventory_pin.rs` Constraint::Custom catalog. (The runtime pin §3.2 caches this as 39 entries to assert.)

3. **`send_sync.rs` PR 4b-E assertion subject is `CanonicalAttrs`, NOT `PageContext`.** PR 6c / T069 retired `PageContext`; the assertion target was retargeted during PR 4b-E review fix-up. The engine-crate ledger entry in §4 attestation skeleton is corrected below.

These corrections do NOT change the closeout's scope or the deliverable count; they refine the exact numbers / names the implementer asserts in the new tests and the attestation table.

---

## 0. Scope summary

PR 4b umbrella closeout is **bookkeeping**: zero rule-logic edits, zero engine-crate edits (Constitution VII scheme-adoption boundary). Two new test files + one Cargo.toml dev-dep addition + spec-doc bookkeeping + one CLAUDE.md doc-nit fix.

This is NOT documentation-only. The compile-time static_assertions + runtime exact-set pin are the load-bearing safety value of the closeout — they catch drift classes the parity gate + corpus-regression cannot.

---

## 1. OQ resolutions

### Architect OQs

**OQ-1 (pin file location) — TWO FILES**:
- `crates/capco/tests/lattice_static_assertions.rs` (compile-time, mirror `crates/ism/tests/send_sync.rs` precedent)
- `crates/capco/tests/post_4b_lattice_inventory_pin.rs` (runtime exact-set, mirror `post_3b_registration_pin.rs`)

Rationale: compile-time and runtime catch disjoint drift classes (per rust-preflight §4 taxonomy). Two files mirror the existing `send_sync.rs` (compile-time) / `post_3b_registration_pin.rs` (runtime) separation; same naming pattern lowers reviewer onboarding cost.

**OQ-2 (`static_assertions` dev-dep) — ADD**:
- Add to `crates/capco/Cargo.toml` `[dev-dependencies]` as `static_assertions = { workspace = true }`
- The workspace entry already exists (verified via `grep -nE 'static_assertions' crates/ism/Cargo.toml`). One-line addition.

**OQ-3 (38-vs-39 pin drift) — RESOLVED, NO ACTION**:
- Pin GREEN at HEAD (`cargo +stable test -p marque-capco --test post_3b_registration_pin` passes).
- Architect's awk regex overcounted by matching `attrs.rel_to = Box::new([...])` slice constructors. The actual `rule_set.rules().len()` is 38; pin's `EXPECTED_RULE_IDS` contains the 38 IDs including `"W004"` (added by PR 4b-B).
- No closeout action on the pin itself.

**OQ-4 (engine-crate touch authorization) — HARD NO**:
- Constitution VII (scheme-adoption boundary): closeout MUST NOT edit `marque-engine` / `marque-scheme` / `marque-core` / `marque-rules` / `marque-ism`. If any test file or assertion needs trait-method widening, propose the inherent-impl-accessor or hardcoded-list path instead. If genuinely impossible, file as follow-up PR per Constitution VII (scheme-adoption boundary) discipline.
- The four documented within-006 precedent touches (PR 4b-B Commit 2 / 4b-C Commit 5 / 4b-D.2 / 4b-D.3 / 4b-E) are part of the umbrella attestation ledger but the closeout itself adds no new precedent.

**OQ-5 (net-delta granularity) — PER-AXIS**:
- Track six separate running counts in the attestation table:
  - `JoinSemilattice` impl count
  - `MeetSemilattice` impl count
  - `BoundedJoinSemilattice` impl count
  - `BoundedMeetSemilattice` impl count
  - `PageRewrite` row count
  - `ClosureRule` row count
- Plus the registered-rule-set count (one row at the bottom — already 38 stable per OQ-3).

**OQ-6 (task ID nomenclature) — FRESH SEQUENTIAL IDS T142-T146** (corrected 2026-05-19 after first implementer pass verified):
- Use `T142` / `T143` / `T144` / `T145` for the four closeout deliverables (attestation, static-assert pin, runtime catalog pin, CI job).
- Use `T146` for the deferred SupersessionSet engine-crate pin (per OQ-RUST-4).
- Each task cross-references `T112` as the umbrella anchor in its description.
- Rationale: T130-T139 was the original recommendation but a pre-write verification revealed **T130-T134 are already taken** at `tasks.md` lines 399–406 (T130 = PR-8 masking-pin removal; T131-T134 = PR-9 parser/dissem/banner/ATOMAL work). Next free contiguous range at HEAD is T142+.

**OQ-7 (CI branch filter) — `refactor-006-pr-4b*` PREFIX-MATCH**:
- Mirror PR 3b T029's `refactor-006-pr-3b*` prefix-match pattern.
- All nine 4b sub-PRs are merged; the job is retroactive memorialization scope for any future 4b-prefixed hot-fix branch, same posture as PR 3b T029 had at merge time.

**OQ-8 (transmutation_stubs accounting) — ALL 27 ROWS COUNT**:
- The 27 `PageRewrite` rows include the 8 transmutation_stubs.rs rows.
- Document explicitly in the attestation table: "27 = 8 (transmutation_stubs) + 4 (Pattern-A NOFORN supremacy) + 7 (Pattern-C strip rows) + 2 (Pattern-B FOUO eviction) + 4 (group 5 noforn-clears + supersession adjuncts) + 1 (group 4 same-axis supersession #552) + 1 (sbu-nf-evicted-by-classified #541)" or whatever the actual breakdown is at HEAD. Implementer verifies the exact group counts against `crates/capco/src/scheme/rewrites/mod.rs:build_page_rewrites` doc-comment.

### Rust OQs

**OQ-RUST-1 (scope) — INCLUDE BOTH TEST FILES + Cargo.toml**:
- Compile-time `lattice_static_assertions.rs` is the primary safety value (catches D3 type-bound drift at build time, zero runtime cost).
- Runtime `post_4b_lattice_inventory_pin.rs` catches catalog-row drift (a `PageRewrite` row renamed at the same count, a `ClosureRule` row swapped for unrelated, etc.).
- Doc-only would be theatre; the user's "quality is everything" directive applies.

**OQ-RUST-2 (PageRewrite ordering pin) — POSITIONAL LIST**:
- `crates/capco/src/scheme/rewrites/mod.rs:build_page_rewrites` doc-comment explicitly: *"Row order is load-bearing — the topological scheduler breaks ties on declaration order, so reordering would silently shift the rewrite schedule."*
- Sorted-set equality would miss reorder drift. Positional list catches the same drift classes as sorted-set PLUS reorder.
- Trade-off: false positives on intentional reorders. Accepted — intentional reorders are events worth blocking on PR review (Constitution VIII propagation discipline: a row's position is part of its operative claim).

**OQ-RUST-3 (pin drift fix scope) — N/A; PIN GREEN AT HEAD (see OQ-3)**.

**OQ-RUST-4 (SupersessionSet pin location) — DEFER**:
- `SupersessionSet` lives in `marque-scheme`. A compile-time pin in `crates/scheme/tests/` would constitute an engine-crate edit.
- Track in `specs/006-engine-rule-refactor/tasks.md` as a deferred follow-up task `T146` (engine-crate pin for `SupersessionSet` Join-only invariant, requires authorized engine-crate touch precedent). Note: an earlier draft referenced T134; corrected per the ERRATA at the top of this file (T130-T134 are taken by PR-8/9; the closeout range is T142-T146 with T146 reserved for this deferral).
- The 4b closeout focuses on `marque-capco` lattice impls; SupersessionSet's structural claim is already audited via PR #538's proptest.

**OQ-RUST-5 (Coverage discipline)** — not explicitly raised but implicit:
- Closeout PR is bookkeeping with new test files. New tests have 100% self-coverage by construction. The closeout adds no production-code paths needing coverage.
- No CodeCov action item expected.

---

## 2. Out-of-scope (firm exclusions)

- **Engine-crate edits.** Constitution VII (scheme-adoption boundary). If you discover the work needs an engine-crate edit (e.g., to expose a `MarkingScheme` accessor), STOP and file a follow-up PR.
- **`corpus_parity.rs` `#![cfg(any())]` revival.** This file has been dormant since PR 3c.B Commit 10 (legacy FixProposal-shape test disabled pending rewrite). The PR 3b closeout plan referenced it as the "count pin" but it stopped running months ago. The post-PR-3b exact-set pin in `post_3b_registration_pin.rs` is the sole active rule-count assertion at HEAD. Reviving / retiring `corpus_parity.rs` is a separate ecosystem health item — out of scope for this PR. File as follow-up if not already tracked.
- **Per-row `PageRewrite` `reads`/`writes` axis annotation correctness.** The positional list pin asserts the row exists at position N with name X — it does NOT assert the row's `reads`/`writes` annotations are correct. Semantic correctness is the parity gate's domain (`crates/capco/tests/lattice_vs_scheme_parity.rs`).
- **SupersessionSet trait-shape pin.** Deferred per OQ-RUST-4.
- **`MarkingScheme` trait-method visibility widening.** If `closure_rules()` / `page_rewrites()` / `constraints()` aren't already `pub` enough for `crates/capco/tests/` access, use inherent-impl accessors on `CapcoScheme` (also `pub` or `pub(crate)` — choose the narrower visibility that works).
- **CAPCO §-citation verification updates beyond the attestation table.** Citations in existing 4b sub-PR docs and CLAUDE.md "Recent Changes" entries STAY. The closeout attestation adds new claims; those new claims verify per Constitution VIII at authorship.

## 3. In-scope deliverables (the work to do)

### 3.1 New file: `crates/capco/tests/lattice_static_assertions.rs`

Compile-time `static_assertions` block covering every lattice impl claim:

```rust
// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Post-PR-4b compile-time lattice impl pin.
//!
//! Locks the exact set of `JoinSemilattice` / `MeetSemilattice` /
//! `BoundedJoinSemilattice` / `BoundedMeetSemilattice` impls on
//! `marque-capco` lattice types. Catches D3 drift class (silently
//! adding/removing a trait impl) at build time, complementing the
//! runtime exact-set pin at `post_4b_lattice_inventory_pin.rs`.
//!
//! Three types — `DissemSet`, `JointSet`, `DisplayOnlyBlock` — implement
//! only `JoinSemilattice` per PR #456's `Lattice` split and PR #538's
//! audit. The `assert_not_impl_any!` blocks below lock the Join-only
//! shape so an accidental `MeetSemilattice` addition is a build error.
//!
//! Authority: PR #456 lattice-split addendum
//! (`docs/plans/2026-05-01-lattice-design.md` §12); PR #538
//! observational-state-lattice audit (`decisions.md` D24 follow-up).

use marque_capco::lattice::{
    AeaSet, ClassificationLattice, DeclassifyOnLattice, DisplayOnlyBlock,
    DissemSet, FgiSet, JointSet, NatoClassLattice, NatoDissemSet, RelToBlock,
    SarSet, SciSet,
};
use marque_scheme::{BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice};
use static_assertions::{assert_impl_all, assert_not_impl_any};

// Types implementing both halves of the Lattice (PR 4b-B / 4b-E):
assert_impl_all!(SciSet: JoinSemilattice, MeetSemilattice);
assert_impl_all!(SarSet: JoinSemilattice, MeetSemilattice);
assert_impl_all!(FgiSet: JoinSemilattice, MeetSemilattice);
assert_impl_all!(AeaSet: JoinSemilattice, MeetSemilattice);
assert_impl_all!(NatoDissemSet: JoinSemilattice, MeetSemilattice);
assert_impl_all!(RelToBlock: JoinSemilattice, MeetSemilattice);
assert_impl_all!(DeclassifyOnLattice: JoinSemilattice, MeetSemilattice);

// Bounded types (ClassificationLattice + NatoClassLattice — PR 4b-B):
assert_impl_all!(ClassificationLattice: JoinSemilattice, MeetSemilattice, BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_impl_all!(NatoClassLattice: JoinSemilattice, MeetSemilattice, BoundedJoinSemilattice, BoundedMeetSemilattice);

// Join-only observational-state types (PR #456 / #538 — locking the shape):
assert_impl_all!(DissemSet: JoinSemilattice);
assert_not_impl_any!(DissemSet: MeetSemilattice);

assert_impl_all!(JointSet: JoinSemilattice);
assert_not_impl_any!(JointSet: MeetSemilattice);

assert_impl_all!(DisplayOnlyBlock: JoinSemilattice);
assert_not_impl_any!(DisplayOnlyBlock: MeetSemilattice);
```

Implementer verifies:
- Every type name exists with that exact path (`marque_capco::lattice::<Type>`).
- The `JoinSemilattice` / `MeetSemilattice` etc. paths re-export correctly from `marque_scheme`.
- All assertions compile (run `cargo +stable build --tests -p marque-capco`).

If a type doesn't exist or path is wrong, the build FAILS at compile time — which is the assertion's job.

### 3.2 New file: `crates/capco/tests/post_4b_lattice_inventory_pin.rs`

Runtime exact-set pin for the three data catalogs (PageRewrite + ClosureRule + Constraint::Custom labels). Triple-pin (raw len + BTreeSet count + missing/unexpected diff) per `post_3b_registration_pin.rs` precedent. **Positional list** for PageRewrite (not sorted-set, per OQ-RUST-2). Header doc-comment mirrors `post_3b_registration_pin.rs` shape with running-count derivation through the nine 4b sub-PRs.

Hardcoded expected lists (per `feedback_pub_doc_hidden_is_still_public_api` — don't widen public API for test access; hardcode the expected list and walk the existing trait surface). The implementer pulls expected names from `crates/capco/src/scheme/rewrites/*.rs` + `crates/capco/src/scheme/closure.rs` + the class-floor / SCI-per-system / RELIDO-conflicts catalogs.

### 3.3 `Cargo.toml` addition

`crates/capco/Cargo.toml` `[dev-dependencies]`:

```toml
static_assertions = { workspace = true }
```

Verified: workspace-level entry already exists; `crates/ism/Cargo.toml:76` uses the same form.

### 3.4 CI matrix entry

`.github/workflows/ci.yml`: new `pr-4b-corpus-regression` job, branch-filtered to `refactor-006-pr-4b*` prefix-match. Body mirrors `pr-3b-corpus-regression` job. Slot placement: between existing 4b-related jobs and before `masking-pin-lint`.

Verify slot via `grep -nE 'pr-3b-corpus-regression|masking-pin-lint' .github/workflows/ci.yml` at implementation time — line numbers WILL have drifted since PR 3b closeout landed.

### 3.5 Spec-doc bookkeeping

- `specs/006-engine-rule-refactor/tasks.md`: append five new tasks T142 / T143 / T144 / T145 / T146 in the appropriate section (under PR 4 series, after T119c):
  - **T142** — Umbrella attestation aggregated into PR description (a/b/c per Constitution VIII single-§-citation discipline + engine-crate-touch ledger + per-axis net-delta math)
  - **T143** — Compile-time `lattice_static_assertions.rs` pin
  - **T144** — Runtime `post_4b_lattice_inventory_pin.rs` pin
  - **T145** — `pr-4b-corpus-regression` CI job
  - **T146** (DEFERRED) — `SupersessionSet` engine-crate pin (out of closeout scope per OQ-RUST-4)
- `specs/006-engine-rule-refactor/plan.md`: annotation on PR 4 row noting umbrella-LANDED state with all nine sub-PR links.
- `CLAUDE.md` "Recent Changes": closeout entry summarizing the umbrella-landing, listing the nine sub-PRs.

### 3.6 CLAUDE.md doc-nit fix

Per rust-preflight finding: CLAUDE.md "Recent Changes" entry for PR 4b-E references `DeclassExemptionLattice`, but the type was renamed to `DeclassExemptionAccumulator` during PR 4b-E review fix-up (verified at `crates/capco/src/lattice.rs:3556 / 3531`). Fix the CLAUDE.md reference in the same closeout PR.

## 4. Attestation skeleton (for implementer to fill in)

PR description structure mirroring PR 3b closeout §2:

```
### Umbrella attestation (Constitution VIII + Constitution VII)

**(a) Single CAPCO-§ citation per declarative entry:**
[table — every lattice impl, every PageRewrite row, every ClosureRule row, with its operative §-citation re-verified against crates/capco/docs/CAPCO-2016.md at this PR's authorship]

**(b) Engine-crate touch ledger** (Constitution VII (scheme-adoption boundary)):
- PR 4b-B Commit 2: [scope, justification line from original PR description]
- PR 4b-C Commit 5: [scope, justification]
- PR 4b-D.2: [scope, justification]
- PR 4b-D.3: [scope, justification]
- PR 4b-E: [scope, justification — `crates/ism/tests/send_sync.rs` add for Send+Sync compile-time check; note: the assertion subject is `CanonicalAttrs`, NOT `PageContext` (PageContext itself retired in PR 6c / T069; the send_sync.rs add must have been retargeted to `CanonicalAttrs` during PR 4b-E review fix-up)]

**(c) Per-axis net-delta math:**
| Axis | Pre-4b | 4b-A | 4b-B | 4b-C | 4b-D.0/.1/.2/.3 | 4b-E | 4b-F | Post-4b |
|------|--------|------|------|------|------------------|------|------|---------|
| JoinSemilattice impls (CAPCO) | ? | +AeaSet | +ClassificationLattice +NatoClassLattice +JointSet +DissemSet +NatoDissemSet +RelToBlock +DeclassifyOnLattice | 0 (declarative rewrites) | 0 / 0 / 0 / 0 | +DisplayOnlyBlock | 0 (residue cleanup) | 12 |
| MeetSemilattice impls (CAPCO) | ? | +AeaSet | +ClassificationLattice +NatoClassLattice +NatoDissemSet +RelToBlock +DeclassifyOnLattice | 0 | 0 / 0 / 0 / 0 | 0 | 0 | 9 |
| BoundedJoinSemilattice impls | 0 | 0 | +Classification +NatoClass | 0 | 0 / 0 / 0 / 0 | 0 | 0 | 2 |
| BoundedMeetSemilattice impls | 0 | 0 | +Classification +NatoClass | 0 | 0 / 0 / 0 / 0 | 0 | 0 | 2 |
| PageRewrite rows | ? | 0 | 0 | +9 | 0 / 0 / 0 / 0 | 0 | 0 | 27 (verify breakdown) |
| ClosureRule rows | ? | 0 | 0 | 0 | 0 / 0 / 0 / 0 | 0 | 0 | 10 (verify) |
| Registered rules | 38 | 0 | +W004 | 0 | 0 / 0 / 0 / 0 | 0 | 0 | 38 (verify — possible -1 +1 if a rule was retired then re-added) |
```

Implementer reconciles the pre-4b baselines by walking `git log staging --before "2026-05-15" -- crates/capco/src/lattice.rs crates/capco/src/scheme/rewrites/ crates/capco/src/scheme/closure.rs` and counting the impls/rows at the pre-4b-A tip.

## 5. Verification commands (pre-flight)

```bash
# Lattice impl inventory at HEAD
grep -nE '^impl(<.*>)? (JoinSemilattice|MeetSemilattice|BoundedJoinSemilattice|BoundedMeetSemilattice).*for' crates/capco/src/lattice.rs crates/capco/src/scheme/marking.rs

# PageRewrite count at HEAD (verify 27)
grep -cF 'PageRewrite {' crates/capco/src/scheme/rewrites/*.rs | grep -v ':0$'

# ClosureRule count at HEAD (verify 10)
grep -cF 'ClosureRule {' crates/capco/src/scheme/closure.rs

# Registered-rule count at HEAD (verify 38; pin already enforces this)
cargo +stable test -p marque-capco --test post_3b_registration_pin

# Existing test surface stays green
cargo +stable test -p marque-capco

# Type-system landscape — confirm static_assertions imports compile
cargo +stable build --tests -p marque-capco

# Clippy + fmt (CI proxy)
cargo +stable clippy --workspace --all-targets -- -D warnings
cargo +stable fmt --check

# Citation-lint clean (PR 5 R3 closure)
cargo run --manifest-path tools/citation-lint/Cargo.toml --release -- .
```

## 6. Constraints binding the implementer

- **Constitution VII (scheme-adoption boundary)**: zero `marque-engine` / `marque-scheme` / `marque-core` / `marque-rules` / `marque-ism` edits. If you discover one is needed, STOP and file a follow-up.
- **Constitution V (test-fixture carve-out)**: do NOT use `AppliedFix::__engine_promote` — bookkeeping doesn't construct audit records.
- **Constitution VIII (citation fidelity)**: every §-citation in the new attestation table verifies against `crates/capco/docs/CAPCO-2016.md` at authorship. No propagation from CAPCO-CONTEXT.md without re-verification.
- **Pre-users**: no deprecation phasing, no aliases, no schema bumps for back-compat. Rewrite freely if needed.
- **Citations format**: `§X.Y pNN` only. Never bare `§NN`. Never `line NNNN` (retired commit b340bec).
- **No line-number anchoring** in plan/commit references — symbolic refs (function name, section, test name) only.
- **Force-push discipline**: no `git push --force[-with-lease]` without explicit PM authorization (memory `feedback_no_unauthorized_force_push`). If the PR branch needs reconciliation, prefer `git pull --rebase`.
- **GPG-signing**: all commits GPG-signed. No `--no-verify`, no `--no-gpg-sign`.
- **Walked-adjacencies discipline**: when fixing a reviewer finding, walk the logical code paths adjacent to the fix and check whether they need the same treatment. "It builds and is all green" is necessary, not sufficient.

## 7. Implementer workflow

1. Read this PM-decisions doc + both preflight plans + the PR 3b closeout template.
2. Run §5 pre-flight verification commands; confirm all green.
3. Add the five T142-T146 task entries to `tasks.md`. Mark T142 as in-progress.
4. Write `crates/capco/tests/lattice_static_assertions.rs` (§3.1). Run `cargo +stable build --tests -p marque-capco` to confirm asserts compile.
5. Write `crates/capco/tests/post_4b_lattice_inventory_pin.rs` (§3.2). Run `cargo +stable test -p marque-capco --test post_4b_lattice_inventory_pin` to confirm the pin passes at HEAD.
6. Add `static_assertions` to `crates/capco/Cargo.toml` dev-dependencies (§3.3).
7. Add the `pr-4b-corpus-regression` CI job to `.github/workflows/ci.yml` (§3.4).
8. Update `tasks.md` (T142-T145 → [X]; T146 stays [ ] as deferred), `plan.md` (PR 4 row annotation), `CLAUDE.md` (Recent Changes entry + the DeclassExemption rename fix in §3.6).
9. Verify §5 pre-flight commands again — all green, fmt clean, clippy clean.
10. Stage all files; commit with descriptive multi-line message; push. PM dispatches the 3-reviewer pass.

Return a concise summary on completion: file paths created/modified, exact rule/impl/rewrite counts at HEAD, any deviations from this contract with rationale.
