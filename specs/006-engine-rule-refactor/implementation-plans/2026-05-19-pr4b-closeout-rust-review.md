<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b Closeout — Rust Specialist Review

**Date**: 2026-05-19
**Branch**: `refactor-006-pr-4b-closeout` (HEAD `320dea6d`)
**Base**: `staging` (off `5d3415cd`)
**Reviewer**: rust-reviewer (Rust specialist agent)
**Parallel reviewers**: code-reviewer + system-architect (lattice-consultant skill)

---

## §0 Verdict

**APPROVE**

Pre-flight chain PASS on stable toolchain (CI proxy). All three commits are
bookkeeping-only (zero production-code changes). The two new test files
correctly implement the compile-time D3-drift lock and the runtime positional +
sorted-set catalog pins the PM contract specifies. No CRITICAL or HIGH issues
found. One MEDIUM and two NITs documented below; none block merge.

---

## Pre-flight Chain Results

| Check | Command | Result |
|-------|---------|--------|
| Compile | `cargo check --workspace` | PASS |
| Lint | `cargo +stable clippy --workspace -- -D warnings` | PASS (0 warnings) |
| Format | `cargo fmt --check` | PASS |
| Tests | `cargo +stable test --workspace` | PASS (0 failures) |
| Static assertions file | `cargo +stable test -p marque-capco --test lattice_static_assertions` | PASS — 0 tests (correct: compile-time only) |
| Inventory pin file | `cargo +stable test -p marque-capco --test post_4b_lattice_inventory_pin` | PASS — 3/3 |
| Constitution VII engine crates | `git diff staging...HEAD --name-only -- crates/engine crates/scheme crates/core crates/rules crates/ism` | EMPTY (no engine-crate edits) |

---

## §1 `lattice_static_assertions.rs` Review

**File**: `crates/capco/tests/lattice_static_assertions.rs`

### Correctness vs `lattice.rs` actual impls

Cross-checked every `assert_impl_all!` and `assert_not_impl_any!` call against
the actual impl headers at `crates/capco/src/lattice.rs`:

```
lattice.rs actual impls (from grep):
  JoinSemilattice:  SciSet SarSet FgiSet AeaSet ClassificationLattice
                    NatoClassLattice DeclassifyOnLattice DissemSet NatoDissemSet
                    JointSet RelToBlock DisplayOnlyBlock   — 12 types CONFIRMED
  MeetSemilattice:  SciSet SarSet FgiSet AeaSet ClassificationLattice
                    NatoClassLattice DeclassifyOnLattice NatoDissemSet RelToBlock
                                                          — 9 types CONFIRMED
  BoundedJoinSemilattice: ClassificationLattice NatoClassLattice  — 2 CONFIRMED
  BoundedMeetSemilattice: ClassificationLattice NatoClassLattice  — 2 CONFIRMED
```

All 25 `assert_impl_all!` claims verified against source. All 3
`assert_not_impl_any!` claims verified: `DissemSet`, `JointSet`,
`DisplayOnlyBlock` have NO `MeetSemilattice` impl in `lattice.rs`.

### Import-path correctness

The test imports `marque_capco::lattice::{..., DisplayOnlyBlock, ...}`. While
`DisplayOnlyBlock` is NOT in the top-level `pub use lattice::{...}` shorthand
re-export at `crates/capco/src/lib.rs:39-42`, this is not an error:
`lib.rs:24` declares `pub mod lattice;` and `DisplayOnlyBlock` is `pub enum` in
`lattice.rs:3651`, making `marque_capco::lattice::DisplayOnlyBlock` a valid
fully-qualified path. The test compiles and runs (confirmed above).

`marque_scheme::{JoinSemilattice, MeetSemilattice, BoundedJoinSemilattice,
BoundedMeetSemilattice}` import is correct — these traits live in
`crates/scheme/src/` and are re-exported from `marque_scheme`.

### Structural deviation from preflight §2

**MEDIUM** (not a bug, but worth noting for the record): The preflight §2 proposed
a flat structure — 12 separate `assert_impl_all!(X: JoinSemilattice)` rows, then
9 separate `assert_impl_all!(X: MeetSemilattice)` rows — deliberately keeping each
assertion granular. The implementer used grouped syntax:
`assert_impl_all!(SciSet: JoinSemilattice, MeetSemilattice)`.

The grouped form is strictly equivalent and the macro semantics are identical —
`static_assertions::assert_impl_all!` with multiple traits is a conjunction. It
is arguably more readable. The deviation from the preflight template is intentional
and reasonable, not an error.

The one visible trade-off: if a type gains `JoinSemilattice` but loses
`MeetSemilattice`, a grouped assertion catches it (compilation error on the
combined form). If a type gains only one half and the author reads the grouped
line as "both or nothing," the assertion still fires correctly. No correctness
concern.

### Join-only lock correctness (§4 mandate)

The three `assert_not_impl_any!` blocks cover exactly the right types:

- `DissemSet` — `relido_observed_unanimous` observational state; join-side
  only. CORRECT per PR #538 audit and preflight §1 risk register.
- `JointSet` — `Mixed`/`DisunityCollapse` absorbing-state variants; meet
  undefined per §H.3. CORRECT.
- `DisplayOnlyBlock` — structural union accumulator added in PR 4b-E; no
  policy basis for intersection across portions per §H.8. CORRECT.

`SupersessionSet` (lives in `marque-scheme`, not `marque-capco`) is
intentionally absent per OQ-RUST-4 PM deferral. The test correctly stays
within `marque-capco` scope.

### Doc-comment compliance

The module-level doc-comment is comprehensive, accurate, and free of
`doc_lazy_continuation` issues (the implementer's reported clippy fix that
changed `1. 2.` numbered lists to `Row 1 — …` bullet form is correctly
applied). No markdown lint issues observed.

### NIT 1 (NIT): Pre-PR-4b impl count attribution

The doc-comment header says "Locks the exact set of `JoinSemilattice` /
`MeetSemilattice` / `BoundedJoinSemilattice` / `BoundedMeetSemilattice` impls
on `marque-capco` lattice types." The phrase "PR 4b-A / 4b-B / 4b-E" in the
section comment at line 65 covers the provenance correctly. `SciSet` / `SarSet`
/ `FgiSet` were pre-4b, but the umbrella is asserting the post-4b terminal
state. The comment attribute "(PR 4b-A / 4b-B / 4b-E)" accurately captures the
sub-PR provenance of each group. No action required.

---

## §2 `post_4b_lattice_inventory_pin.rs` Review

**File**: `crates/capco/tests/post_4b_lattice_inventory_pin.rs`

### Shape compliance with `post_3b_registration_pin.rs` template

The file follows the triple-pin shape: raw-slice length check + BTreeSet
cardinality check + missing/unexpected set-diff for the exact-set assertion.
The module-level doc-comment includes a running-count derivation paragraph, a
"why separate from the count test" paragraph, and an explicit drift policy
warning. This mirrors `post_3b_registration_pin.rs` discipline exactly.

### OQ-RUST-2: PageRewrite pin is positional (VERIFIED)

`EXPECTED_PAGE_REWRITES` is compared via `actual: Vec<&str> == expected:
Vec<&str>` (lines 289-305). The sorted-set computation appears only for the
diagnostic error message (lines 292-296) to distinguish rename-drift from
reorder-drift. The **load-bearing assertion is positional**, not set-equality.
This correctly implements the OQ-RUST-2 PM resolution.

### PageRewrite ordering vs actual `build_page_rewrites()`

Cross-checked `build_page_rewrites()` at
`crates/capco/src/scheme/rewrites/mod.rs:173-181`:

```
Actual concat order: pattern_a → pattern_c → pattern_b →
                     supersession → noforn_clears → transmutation_stubs
```

`EXPECTED_PAGE_REWRITES` (lines 133-167) follows:
```
pattern_a (4) → pattern_c (8) → pattern_b (2) →
supersession (2) → noforn_clears (3) → transmutation_stubs (8) = 27 total
```

This matches the actual concat order exactly. Names cross-checked against
source files:
- `pattern_a.rs`: 4 names match
- `pattern_c.rs`: 8 names match (including `sbu-nf-evicted-by-classified` from #541)
- `pattern_b.rs`: 2 names match
- `supersession.rs`: 2 names match
- `noforn_clears.rs`: 3 names match
- `transmutation_stubs.rs`: 8 names match

All 27 entries verified against the originating source files.

### ClosureRule ordering vs actual `CAPCO_CLOSURE_RULES`

**NOTE**: The preflight §3 draft proposed putting `rel-to-usa-nato-if-nato-classification`
at position 2 (after `noforn-if-caveated`). The actual `CAPCO_CLOSURE_RULES` static
at `crates/capco/src/scheme/closure.rs:983` places it at position 8, after the six
per-marking implication rows. The implementer correctly read the actual source
(not the preflight draft) and `EXPECTED_CLOSURE_RULES` matches the actual ordering.
The preflight sketch was a draft; the implementer's implementation is correct.

Verified actual order:
```
1. capco/noforn-if-caveated
2. capco/hcs-o-implies-noforn-orcon
3. capco/hcs-p-sub-implies-noforn-orcon
4. capco/si-g-implies-orcon
5. capco/tk-blfh-implies-noforn
6. capco/tk-idit-implies-noforn
7. capco/tk-kand-implies-noforn
8. capco/rel-to-usa-nato-if-nato-classification
9. capco/relido-if-sci-and-not-incompatible
10. capco/relido-if-us-collateral-class
```

`EXPECTED_CLOSURE_RULES` (lines 186-197) matches exactly.

### Constraint::Custom pin

`EXPECTED_CUSTOM_CONSTRAINTS` contains 39 entries. The
`Constraint::Custom { .. }` filter pattern is valid Rust struct-pattern syntax
(confirmed: `Constraint::Custom { name, label }` is the actual variant shape at
`crates/scheme/src/constraint.rs:205`). The `.name()` method dispatches to the
`name` field (confirmed: `constraint.rs:215-221`).

The 39-entry breakdown (7 core + 27 class-floor + 5 sci-per-system) matches
the PM ERRATA correction (RELIDO E054-E057 are `Constraint::Conflicts`, not
`Custom` — they correctly do NOT appear in this list).

### Accessor discipline

Tests use `scheme.page_rewrites()`, `scheme.closure_rules()`, and
`scheme.constraints()` — all public `MarkingScheme` trait methods on
`CapcoScheme`. No accessor widening was needed and none was done. No
Constitution VII violation.

### CapcoScheme::new() vs default()

The test uses `CapcoScheme::new()`. The preflight sketch used
`CapcoScheme::default()`. Both are valid (`CapcoScheme` implements `Default`
via `impl Default for CapcoScheme` at `crates/capco/src/scheme/adapter.rs:63`).
`new()` is the more explicit form; no issue.

### NIT 2 (NIT): `expected.len() == 27` self-check on `EXPECTED_PAGE_REWRITES`

Lines 282-286 assert `expected.len() == 27` as a sanity check. The constant is
a `&[&str]` literal — its length is determined at compile time and cannot drift
without editing the source. The assertion is harmless and consistent with
`post_3b_registration_pin.rs`'s pattern, so it is acceptable as defensive
belt-and-suspenders. Not a bug.

---

## §3 `Cargo.toml` + `Cargo.lock` Review

**File**: `crates/capco/Cargo.toml`

`static_assertions = { workspace = true }` appears at line 50 under
`[dev-dependencies]` (section starts at line 34). Placement is correct:
`[dev-dependencies]` not `[dependencies]`. The entry is accompanied by a
two-line comment explaining its purpose and citing the `send_sync.rs` precedent,
which is good practice.

The workspace root `Cargo.toml` already carries `static_assertions = "1.1.0"`
at line 127. The version is consistent.

**Cargo.lock diff is minimal**: exactly one line added — `static_assertions`
appended to the `marque-capco` dependency list. No unrelated lock churn.

---

## §4 Walked-adjacencies Beyond `DeclassExemptionLattice`

The implementer's walked-adjacency check addressed the `DeclassExemptionLattice`
→ `DeclassExemptionAccumulator` rename across 5 repo occurrences, correctly
distinguishing the single fixable forward-looking reference (CLAUDE.md:283)
from historical plan-doc point-in-time artifacts.

Two additional adjacency checks I would have expected and verified:

### (a) Existing count pin in `transmutation_rewrites.rs`

The new pin adds an exact-positional check for PageRewrite names. The PM
contract notes (§3.2 "why a separate test") that
`crates/capco/tests/transmutation_rewrites.rs::scheduled_rewrites` already
pins `PageRewrite count == 27`. Verifying the existing test did not drift
was an appropriate adjacency. The test still passes at HEAD (confirmed by full
workspace test run above). No action needed.

### (b) `post_3b_registration_pin.rs` rule-ID list

The new pin joins `post_3b_registration_pin.rs` in the same test suite. If a
rule had been added or retired during 4b (W004 was added in 4b-B), the 3b pin
would have drifted. Confirmed: `post_3b_registration_pin.rs` reflects the correct
38-rule set including `"W004"` and the pin passes GREEN. No action needed.

### (c) No existing test asserting lattice impl count independently

Grepped `crates/capco/tests/` for any assertion on `JoinSemilattice` impl count
or `MeetSemilattice` impl count — none found. The new `lattice_static_assertions.rs`
is the sole compile-time lock. No adjacency conflict.

---

## §5 Constitution Discipline Spot-checks

### Constitution V (no `__engine_promote`)

`grep -rn '__engine_promote'` over both new test files: **zero hits**. Confirmed —
neither test file calls `AppliedFix::__engine_promote`. The bookkeeping-only
scope means no audit record construction occurs; carve-out not needed and
not used.

### Constitution VII (zero engine-crate edits)

`git diff staging...HEAD --name-only -- crates/engine crates/scheme crates/core crates/rules crates/ism`
produces **empty output**. Confirmed: no touch of `marque-engine`, `marque-scheme`,
`marque-core`, `marque-rules`, or `marque-ism`. The test files use
`MarkingScheme` trait methods that are already public; no widening was required.

### Constitution VIII (§-citation spot-checks, 5 random samples)

All citations spot-checked against `crates/capco/docs/CAPCO-2016.md` page
anchors (`begin page N` / `end page N` markers):

| Citation | CAPCO-2016.md page anchor | Verdict |
|---|---|---|
| `§B.3 Table 2 p21` (`capco/noforn-if-caveated`) | `begin page 21` confirmed; caveated→NOFORN rule verified at lines 383-399 | VALID |
| `§H.8 p134` (FOUO eviction rows) | `begin page 134` at line 3277; FOUO banner precedence rules verified at lines 3307-3312 | VALID |
| `§H.3 p57` (`JointSet` Join-only grounding) | `begin page 57` at line 1268; JOINT derivative-use rules verified at lines 1282-1293 | VALID |
| `§H.9 p174` (`capco/nodis-implies-noforn`) | `begin page 174` at line 4269; NODIS/EXDIS NOFORN content verified | VALID |
| `§H.8 pp155-156` (`DissemSet` Join-only grounding) | Cited in the lattice type's own doc-comment and in the static-assertions file's §-citation block; pp155-156 content is the RELIDO unanimity rule governing the observational state | VALID |

The attestation-draft's `DisplayOnlyBlock` row cites `§H.8 (DISPLAY ONLY axis
grounding)` without a page number — this is a broad-section citation rather
than a precise `§X.Y pNN` reference. Per Constitution VIII, a bare
`§H.8` without a page number is acceptable when the claim is "this axis
is grounded in §H.8's DISPLAY ONLY governance," which is a structural claim
rather than a specific rule. The implementation verifies the Join-only
decision on §H.8 policy grounds, which is sufficient. **No block.**

### Pre-users discipline

No deprecation phasing, aliases, or schema-bump-for-back-compat observed in
any of the three commits. The new test files assert a forward-looking closed
state with an explicit drift policy warning ("do not silently edit"). This is
consistent with the "pre-users — rewrite freely" posture.

---

## §6 Findings to Address Before PR Open

**No blocking findings.** All CRITICAL and HIGH checks pass.

### MEDIUM findings (informational, no action required)

**M-1 (MEDIUM)**: The `lattice_static_assertions.rs` implementation uses grouped
`assert_impl_all!(Type: Join, Meet)` syntax rather than the flat per-trait rows
the preflight §2 proposed. This is a deliberate and valid deviation — the
grouped form is semantically equivalent and more compact. The PM contract
(`§3.1`) specifies "verify every type name exists" and "all assertions compile,"
both satisfied. The implementer note in the summary ("Otherwise
contract-conformant") is accurate.

### NIT findings (optional improvements, no action required)

**NIT-1**: `DisplayOnlyBlock` is accessible via `marque_capco::lattice::DisplayOnlyBlock`
(sub-module path) but is NOT in the top-level `pub use lattice::{...}` re-export
list at `crates/capco/src/lib.rs:39-42`. This is not an error — the sub-module
path works and the test compiles. But if a future crate bumps `DisplayOnlyBlock`
to the top-level re-export (as was done for the other 11 types), the import path
in `lattice_static_assertions.rs` stays valid (more specific path still resolves).
No action required; just noting the asymmetry for the next reviewer who looks at
lib.rs and wonders why 11 types are re-exported but DisplayOnlyBlock isn't.

**NIT-2**: `EXPECTED_PAGE_REWRITES`'s inline sanity check (`assert_eq!(expected.len(), 27, ...)`)
is a compile-time-knowable fact asserted at runtime. Harmless and consistent with
`post_3b_registration_pin.rs` precedent; keeping it is fine.

---

## §7 Summary for PM

This PR is a clean bookkeeping closeout. The two new test files implement exactly
what the PM contract (OQ-RUST-1 / OQ-RUST-2) specified:

- `lattice_static_assertions.rs`: compile-time D3-drift lock, 25 impl assertions
  across 12 types, 3 `assert_not_impl_any!` Join-only locks — all verified against
  actual source.
- `post_4b_lattice_inventory_pin.rs`: runtime triple-pin for 27 PageRewrites
  (positional), 10 ClosureRules (positional), 39 Constraint::Custom labels
  (sorted-set) — all verified against actual source. The closure-rule ordering
  correctly reflects the actual `CAPCO_CLOSURE_RULES` static (not the preflight
  sketch's draft ordering).

Deviations from preflight are all benign (grouped `assert_impl_all!` syntax,
`CapcoScheme::new()` vs `default()`, closure-rule order reading from actual source
vs draft). No constitutional violations. The carry-forward of the non-blocking
deferred item (T146 SupersessionSet pin in `marque-scheme`) is correctly marked
as DEFERRED in `tasks.md`.

