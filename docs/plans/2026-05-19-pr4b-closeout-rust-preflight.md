# PR 4b Closeout — Rust Pre-flight Report

**Date**: 2026-05-19  
**Branch**: `refactor-006-pr-4b-closeout`  
**Base tip**: `5d3415cd` (staging)  
**Worktree**: `crates/marque/.claude/worktrees/pr-4b-closeout/`  
**Reviewer**: rust-specialist preflight agent

---

## Pre-flight Chain Results

All four checks pass GREEN against HEAD.

| Check | Command | Result |
|-------|---------|--------|
| Compile | `cargo check --workspace` | PASS |
| Lint | `cargo +stable clippy -p marque-capco -- -D warnings` | PASS (0 warnings) |
| Format | `cargo +stable fmt --check` | PASS |
| Tests | `cargo +stable test --workspace` | PASS (0 failures) |

Note: `cargo +stable` used throughout as a CI-proxy per project memory
`feedback_clippy_nightly_vs_stable_drift` — local toolchain is nightly 0.1.97; CI is stable.

---

## §1 Risk Register — Per Lattice Type

Each row answers: (a) which trait bounds are claimed, (b) is join-only status intentional or inadvertent, (c) risk if a MeetSemilattice impl is erroneously added in a future PR.

### Lattice Types with Both Halves (Full Lattice via Blanket Impl)

| Type | JoinSemilattice | MeetSemilattice | Bounded | Risk of Spurious Meet |
|------|:-:|:-:|---------|----------------------|
| `SciSet` | Y | Y | No — open-vocab SCI systems | Low: proptest absorption laws cover this |
| `SarSet` | Y | Y | No — open-vocab SAR programs | Low: proptest absorption laws cover this |
| `FgiSet` | Y | Y | No — open-vocab FGI producers | Low: proptest absorption laws cover this |
| `AeaSet` | Y | Y | No — open AEA future extension | Low: proptest laws in `category_lattice_laws.rs` |
| `ClassificationLattice` | Y | Y | Yes — bounded 5-level OrdMax chain | None: bounded, both halves correct |
| `NatoClassLattice` | Y | Y | Yes — bounded 5-level NATO chain | None: bounded, both halves correct |
| `DeclassifyOnLattice` | Y | Y | No — MaxDate semilattice, no finite top | Low: meet is well-defined (minimum date) |
| `NatoDissemSet` | Y | Y | No | Low: simple union/intersection |
| `RelToBlock` | Y | Y | No — Bottom/Lattice/Empty/NofornSuperseded | Medium: the `Empty` variant from C-2 split makes meet non-trivial; proptest covers this |

### Join-Only Types (Observational State — PR #538 Audit Verified)

These three types implement ONLY `JoinSemilattice`. The meet operation is either undefined or non-idempotent due to observational state encoded in the variant.

| Type | Join-only Reason | Risk if Meet Added |
|------|-----------------|-------------------|
| `DissemSet` | `relido_observed_unanimous` field: unanimity is a running join-side observation that cannot be derived from the structural meet of two sets | HIGH — a meet impl would need to somehow combine two unanimity observations; the only safe choice (AND-ing them) is not the correct semantics per §H.8 pp155-156 and would silently produce wrong banner roll-up |
| `JointSet` | `Mixed` and `DisunityCollapse` variants: the C-3 split made join associative by encoding the absorbing JOINT+non-JOINT state into `DisunityCollapse`; meet would need to define what "lowest JOINT state" means, which is not specified by §H.3 and would be speculation | HIGH — risk of silently producing wrong `//JOINT` banner behavior |
| `DisplayOnlyBlock` | Structural join-only accumulator: `DisplayOnlyBlock::join` unions the set of DISPLAY ONLY targets; meet is the intersection, but there is no policy basis for intersecting display-only audiences across portions per §H.8 | MEDIUM — less policy-critical than DissemSet/JointSet, but still unspecified |

**Action**: Lock all three with `assert_not_impl_any!` in `§2` below.

### Aggregator Helpers (No Lattice Impls — Intentional)

| Type | Role | Lattice Impl? |
|------|------|:---:|
| `NonIcDissemSet` | Accumulator for §B.3 non-IC dissem → NOFORN closure; wraps a bag of tokens; NOT a lattice because the key query is "any token present" not join/meet | No |
| `DeclassExemptionAccumulator` | Accumulator for last-observed declassification exemption; non-commutative by nature (later observation wins) — `DeclassExemptionLattice` join impl was dropped in PR 4b-E review because last-observed is not associative | No |

**Risk**: Both types are internal to `crates/capco`. A future PR adding `JoinSemilattice for NonIcDissemSet` would be a category error — `NonIcDissemSet` is a query accumulator, not a state type. `DeclassExemptionAccumulator` was explicitly de-latticed; re-adding would reintroduce the non-commutativity defect.

---

## §2 Compile-Time Pin Candidates (`static_assertions`)

### Proposed File

`crates/capco/tests/lattice_static_assertions.rs`

### Cargo.toml Change Required

`crates/capco/Cargo.toml` does not yet include `static_assertions` in `[dev-dependencies]`. Add:

```toml
# In [dev-dependencies] section of crates/capco/Cargo.toml
static_assertions = { workspace = true }
```

Precedent: `crates/ism/Cargo.toml` uses `static_assertions = { workspace = true }` for `crates/ism/tests/send_sync.rs`. Workspace pin is `static_assertions = "1.1.0"`.

### Full Pin Block

```rust
//! Compile-time lattice trait inventory pin for PR 4b closeout.
//!
//! Every `assert_impl_all!` asserts a trait bound that MUST hold per the
//! PR #456 JoinSemilattice + MeetSemilattice split design.
//!
//! Every `assert_not_impl_any!` locks the Join-only invariant for
//! observational-state types audited in PR #538.
//!
//! Adding MeetSemilattice for DissemSet, JointSet, or DisplayOnlyBlock
//! is prohibited — see §1 risk register in
//! docs/plans/2026-05-19-pr4b-closeout-rust-preflight.md.

use marque_capco::lattice::{
    AeaSet, ClassificationLattice, DeclassifyOnLattice, DisplayOnlyBlock,
    DissemSet, FgiSet, JointSet, NatoClassLattice, NatoDissemSet,
    RelToBlock, SarSet, SciSet,
};
use marque_scheme::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};
use static_assertions::{assert_impl_all, assert_not_impl_any};

// --- JoinSemilattice (all 12 lattice types) ---
assert_impl_all!(SciSet: JoinSemilattice);
assert_impl_all!(SarSet: JoinSemilattice);
assert_impl_all!(FgiSet: JoinSemilattice);
assert_impl_all!(AeaSet: JoinSemilattice);
assert_impl_all!(ClassificationLattice: JoinSemilattice);
assert_impl_all!(NatoClassLattice: JoinSemilattice);
assert_impl_all!(DeclassifyOnLattice: JoinSemilattice);
assert_impl_all!(NatoDissemSet: JoinSemilattice);
assert_impl_all!(RelToBlock: JoinSemilattice);
assert_impl_all!(DissemSet: JoinSemilattice);
assert_impl_all!(JointSet: JoinSemilattice);
assert_impl_all!(DisplayOnlyBlock: JoinSemilattice);

// --- MeetSemilattice (9 full-lattice types; Join-only types excluded) ---
assert_impl_all!(SciSet: MeetSemilattice);
assert_impl_all!(SarSet: MeetSemilattice);
assert_impl_all!(FgiSet: MeetSemilattice);
assert_impl_all!(AeaSet: MeetSemilattice);
assert_impl_all!(ClassificationLattice: MeetSemilattice);
assert_impl_all!(NatoClassLattice: MeetSemilattice);
assert_impl_all!(DeclassifyOnLattice: MeetSemilattice);
assert_impl_all!(NatoDissemSet: MeetSemilattice);
assert_impl_all!(RelToBlock: MeetSemilattice);

// --- Lock Join-only invariant (PR #538 audit verdict: hold) ---
// DissemSet: relido_observed_unanimous is a join-side observation; meet is undefined
// JointSet: Mixed/DisunityCollapse absorbing-state variants make meet unspecified
// DisplayOnlyBlock: display-only audience intersection has no policy basis per §H.8
assert_not_impl_any!(DissemSet: MeetSemilattice);
assert_not_impl_any!(JointSet: MeetSemilattice);
assert_not_impl_any!(DisplayOnlyBlock: MeetSemilattice);

// --- BoundedLattice (ClassificationLattice + NatoClassLattice only) ---
// SciSet/SarSet: open-vocab, no lawful finite top (Constitution §4b note)
// DeclassifyOnLattice: MaxDate semilattice, no finite top
assert_impl_all!(ClassificationLattice: BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_impl_all!(NatoClassLattice: BoundedJoinSemilattice, BoundedMeetSemilattice);
```

Note: `SupersessionSet` lives in `marque-scheme`, not `marque-capco`. Its Join-only assertion belongs in `crates/scheme/tests/` (see §7 OQ-RUST-4).

---

## §3 Runtime Exact-State Pin Candidates

### Proposed File

`crates/capco/tests/post_4b_lattice_inventory_pin.rs`

This file follows the precedent established by `crates/capco/tests/post_3b_registration_pin.rs`: a runtime integration test that hardcodes the complete expected set and fails if any name is added, removed, or renamed without an intentional update.

### PageRewrite Name Pin (27 rows)

Exact `name:` field strings, ordered by source file group (consistent with `marking_scheme_impl.rs` concat order):

```rust
const EXPECTED_PAGE_REWRITE_NAMES: &[&str] = &[
    // pattern_a.rs — §B.3 Table 2 p21 / §H.9 p172-176 implies-noforn
    "capco/nodis-implies-noforn",
    "capco/exdis-implies-noforn",
    "capco/sbu-nf-implies-noforn",
    "capco/les-nf-implies-noforn",
    // pattern_b.rs — §H.8 p134 FOUO eviction
    "capco/classification-evicts-fouo",
    "capco/non-fdr-control-evicts-fouo",
    // pattern_c.rs — §H.9/§H.8/§H.6 classified-strip semantics
    "capco/limdis-evicted-by-classified",
    "capco/sbu-evicted-by-classified",
    "capco/sbu-nf-evicted-by-classified",
    "capco/dod-ucni-promotes-noforn-when-classified",
    "capco/dod-ucni-evicted-by-classified",
    "capco/doe-ucni-promotes-noforn-when-classified",
    "capco/doe-ucni-evicted-by-classified",
    "capco/fouo-evicted-by-classified",
    // supersession.rs — SBU-NF supersedes SBU; LES-NF supersedes LES
    "capco/sbu-nf-supersedes-sbu",
    "capco/les-nf-supersedes-les",
    // noforn_clears.rs — §H.8 NOFORN supersession of REL TO / FDR / DISPLAY ONLY
    "capco/noforn-clears-rel-to",
    "capco/noforn-clears-fdr-family",
    "capco/noforn-clears-display-only-to",
    // transmutation_stubs.rs — Stage 4+ deferred transmutation rows (stub bodies)
    "capco/frd-sigma-consolidates-into-rd-sigma",
    "capco/fgi-rollup-on-us-contact",
    "capco/fgi-restricted-rollup-on-us-contact",
    "capco/joint-cross-class-rollup",
    "capco/us-presence-promotes-bare-fgi-attribution",
    "capco/orcon-nato-to-us-orcon-on-us-contact",
    "capco/sbu-nf-transmutes-on-classified-contact",
    "capco/les-nf-transmutes-on-classified-contact",
];
```

### ClosureRule Name Pin (10 rows)

Exact `name:` field strings from `crates/capco/src/scheme/closure.rs`:

```rust
const EXPECTED_CLOSURE_RULE_NAMES: &[&str] = &[
    // §B.3 Table 2 p21 — caveated → NOFORN
    "capco/noforn-if-caveated",
    // §G.2 / §H.7 — NATO classification → REL TO USA, NATO
    "capco/rel-to-usa-nato-if-nato-classification",
    // §H.4 SCI per-system closure rules
    "capco/hcs-o-implies-noforn-orcon",
    "capco/hcs-p-sub-implies-noforn-orcon",
    "capco/si-g-implies-orcon",
    "capco/tk-blfh-implies-noforn",
    "capco/tk-idit-implies-noforn",
    "capco/tk-kand-implies-noforn",
    // §H.8 RELIDO closure conditions
    "capco/relido-if-sci-and-not-incompatible",
    "capco/relido-if-us-collateral-class",
];
```

### Test Body Sketch

```rust
#[test]
fn post_pr_4b_page_rewrite_inventory_pin() {
    use marque_capco::CapcoScheme;
    use marque_scheme::MarkingScheme;
    let scheme = CapcoScheme::default();
    let actual: Vec<&str> = scheme.page_rewrites().iter().map(|r| r.name).collect();
    let mut actual_sorted = actual.clone();
    actual_sorted.sort_unstable();
    let mut expected_sorted = EXPECTED_PAGE_REWRITE_NAMES.to_vec();
    expected_sorted.sort_unstable();
    assert_eq!(
        actual_sorted, expected_sorted,
        "PageRewrite inventory changed — update EXPECTED_PAGE_REWRITE_NAMES and this test"
    );
    assert_eq!(actual.len(), 27, "PageRewrite count changed from 27");
}

#[test]
fn post_pr_4b_closure_rule_inventory_pin() {
    use marque_capco::CapcoScheme;
    use marque_scheme::MarkingScheme;
    let scheme = CapcoScheme::default();
    let actual: Vec<&str> = scheme.closure_rules().iter().map(|r| r.name).collect();
    let mut actual_sorted = actual.clone();
    actual_sorted.sort_unstable();
    let mut expected_sorted = EXPECTED_CLOSURE_RULE_NAMES.to_vec();
    expected_sorted.sort_unstable();
    assert_eq!(
        actual_sorted, expected_sorted,
        "ClosureRule inventory changed — update EXPECTED_CLOSURE_RULE_NAMES and this test"
    );
    assert_eq!(actual.len(), 10, "ClosureRule count changed from 10");
}
```

No accessor widening needed. `MarkingScheme::page_rewrites()` and `closure_rules()` are already public trait methods on `CapcoScheme`.

---

## §4 Drift-Class Taxonomy

Four drift classes that could cause a closeout pin to fail silently or noisily:

### D1 — Rename Drift
A `name:` string changes in a rewrite or closure rule definition without updating the pin. Caught by: runtime exact-set comparison (sorted set equality, not count-only). Symptom: one name disappears, one appears — count stays stable but set differs.

### D2 — Count Drift
A row is added to `page_rewrites()` or `closure_rules()` or `CapcoRuleSet::new()` without updating the pin. Caught by: both count-only (`assert_eq!(len, N)`) and exact-set comparison. The `transmutation_rewrites.rs` test at `scheduled_rewrites().len() == 27` provides a redundant count pin for PageRewrites via the topological scheduler.

### D3 — Type-Bound Drift
A `MeetSemilattice` impl is added to a Join-only type (DissemSet, JointSet, DisplayOnlyBlock) or a `JoinSemilattice` impl is accidentally removed. Caught by: compile-time `assert_impl_all!` and `assert_not_impl_any!` in `lattice_static_assertions.rs`. This is a silent drift class in the absence of static assertions — tests keep passing, behavior changes.

### D4 — Dead-Code Masking
A pin test exists but the file has `#![cfg(any())]` at the top, making all tests dead code. This is the exact failure mode in `crates/capco/tests/corpus_parity.rs` (see §5). New pin test files MUST NOT have an `#![cfg(any())]` gate. Verify: `cargo test -p marque-capco --test <file>` should report tests collected, not zero.

---

## §5 The 38-vs-39 Drift Verdict in `post_3b_registration_pin.rs`

### Verdict: PIN GREEN AT HEAD — No Fix Required

**Background**: The architect plan (`docs/plans/2026-05-19-pr4b-closeout-architect-plan.md`) flagged a potential 38-vs-39 discrepancy based on an awk command:

```bash
awk '/^impl CapcoRuleSet \{/,/^impl RuleSet for CapcoRuleSet \{/' \
    crates/capco/src/rules.rs | grep -cE "Box::new\("
```

**Root cause of overcount**: The awk range is too broad. It includes a line at `attrs.rel_to = Box::new([` that is NOT a rule registration — it is a `Box<[T]>` slice constructor for an ISM attribute field used during a context-building block inside `CapcoRuleSet::new()`. This caused the awk command to output 39 instead of 38.

**Actual state verified by Python-accurate parsing**: 38 rule registrations confirmed.

**Test state**: `post_3b_registration_pin.rs` function `post_pr_470_registers_exact_38_rule_ids` asserts `raw_len == 38` and enumerates 38 entries in `EXPECTED_RULE_IDS`. W004 (`"W004"`) is correctly included at the entry alongside `"W034"`. Test passes GREEN at HEAD.

**The 38-rule set includes**: W004 `JointDisunityCollapseRule` (added PR 4b-B), which is the last registration in `CapcoRuleSet::new()`. W002 was retired (count 39 → 38); the retirement closed the delta the architect plan was tracking.

**Conclusion**: No update to `post_3b_registration_pin.rs` is needed for the closeout. The pin is correct and passing.

### Note on `corpus_parity.rs`

`crates/capco/tests/corpus_parity.rs` has `#![cfg(any())]` at line 1, making ALL tests in the file dead code. This includes a stale assertion of 23 PageRewrites (predating PR 4b-C's expansion to 27). This stale assertion is harmless because it never compiles into a test binary, but it is misleading. The live count pin is `transmutation_rewrites.rs` which asserts `scheduled_rewrites().len() == 27` and is active.

**Recommendation**: The closeout PR should note `corpus_parity.rs` dead status and track its eventual deletion or re-enablement as a follow-up. It is not a blocker.

---

## §6 Verification Commands

Run these in order from the workspace root (`/home/knitli/marque/.claude/worktrees/pr-4b-closeout/`):

```bash
# 1. Full pre-flight (stable toolchain = CI proxy)
cargo +stable check --workspace
cargo +stable clippy -p marque-capco -- -D warnings
cargo +stable fmt --check
cargo +stable test --workspace

# 2. Verify the rule registration pin specifically
cargo +stable test -p marque-capco --test post_3b_registration_pin -- --nocapture

# 3. Verify the live PageRewrite count pin
cargo +stable test -p marque-capco --test transmutation_rewrites post_pr_552_page_rewrite_count -- --nocapture

# 4. Verify the new static assertions file compiles (after adding it)
cargo +stable test -p marque-capco --test lattice_static_assertions

# 5. Verify the new inventory pin file (after adding it)
cargo +stable test -p marque-capco --test post_4b_lattice_inventory_pin -- --nocapture

# 6. Confirm corpus_parity.rs is dead code (should show 0 tests collected)
cargo +stable test -p marque-capco --test corpus_parity 2>&1 | grep -E "test result|running [0-9]+ test"

# 7. PropTest coverage for lattice laws
cargo +stable test -p marque-capco --test proptest_lattice -- --nocapture
cargo +stable test -p marque-capco --test category_lattice_laws -- --nocapture

# 8. Security audit (if cargo-audit present)
if command -v cargo-audit >/dev/null 2>&1; then cargo audit; else echo "cargo-audit not installed — skip"; fi
```

---

## §7 Open Questions for PM

### OQ-RUST-1: Closeout Deliverables Scope

**Question**: Does the closeout PR include the two new test files (`lattice_static_assertions.rs` and `post_4b_lattice_inventory_pin.rs`) plus the one-line `Cargo.toml` addition, or is the closeout documentation-only?

**Stakes**: The static assertions and inventory pins are the primary safety value of the closeout. Without them, future PRs that accidentally add `MeetSemilattice for DissemSet` or rename a PageRewrite row will compile and test green until a manual audit catches the drift. With them, the drift is caught at compile time (§2) or test time (§3).

**Constitution check**: The additions are `dev-dependencies` and `tests/` files only. No production code is touched. Constitution VII §IV "zero engine-crate edits" is satisfied.

### OQ-RUST-2: Ordering Requirement for PageRewrite Pin

**Question**: Should the runtime pin assert name-set equality (sorted comparison, order-independent) or ordered-list equality (exact positional match)?

**Stakes**: The topological scheduler `marque_engine::scheduler` deterministically orders rewrites by `reads`/`writes` axes. The `page_rewrites()` accessor returns the raw registration order. If the scheduler's output order matters for correctness (not just determinism), an ordered-list pin would catch registration-order regressions. A sorted-set pin catches only name drift.

**Recommendation**: Sorted-set equality for the closeout pin (simpler, less brittle to future registration reordering). If `scheduled_rewrites()` order is load-bearing, a separate ordered pin on the scheduler output is more appropriate and should be scoped to a dedicated test in `marque-engine`.

### OQ-RUST-3: 38-vs-39 Rule Count (RESOLVED)

**Verdict**: PIN GREEN AT HEAD. See §5 for full analysis. No PM decision required.

### OQ-RUST-4: SupersessionSet Join-Only Pin Location

**Question**: `SupersessionSet` lives in `marque-scheme` (not `marque-capco`). Its `assert_not_impl_any!(SupersessionSet: MeetSemilattice)` pin belongs in `crates/scheme/tests/`. Should this be added as part of the 4b closeout, or deferred to a scheme-layer closeout?

**Stakes**: PR #538 audited SupersessionSet and confirmed its Join-only status is sound (associativity, commutativity, idempotence, identity-with-bottom all verified by proptest). The pin would lock that verdict against future `marque-scheme` PRs. It is low-effort (1 file, 1 assert) but is technically out of scope for `marque-capco` closeout work.

### OQ-RUST-5: `DeclassExemptionAccumulator` Naming in CLAUDE.md

**Observation**: `CLAUDE.md` still references `DeclassExemptionLattice` in the "Recent Changes" section for PR 4b-E. The actual type was renamed to `DeclassExemptionAccumulator` when the `JoinSemilattice` impl was dropped (lattice dropped → name updated to reflect non-lattice role). This is a documentation nit, not a code issue.

**Recommendation**: Update `CLAUDE.md` Recent Changes entry for PR 4b-E to use the correct name `DeclassExemptionAccumulator`. Alternatively, carry forward in the PR body description.

---

## Implementation Checklist for Closeout PR

- [ ] Add `static_assertions = { workspace = true }` to `[dev-dependencies]` in `crates/capco/Cargo.toml`
- [ ] Create `crates/capco/tests/lattice_static_assertions.rs` with the §2 pin block
- [ ] Create `crates/capco/tests/post_4b_lattice_inventory_pin.rs` with the §3 pin tests
- [ ] Run verification chain (§6) — all steps must pass GREEN
- [ ] PM answer for OQ-RUST-1 (scope) before committing new test files
- [ ] PM answer for OQ-RUST-2 (ordering vs set equality) before writing `post_4b_lattice_inventory_pin.rs` body
- [ ] Address OQ-RUST-4 (SupersessionSet pin) if PM wants scheme-layer coverage in same PR
- [ ] Update CLAUDE.md `DeclassExemptionLattice` → `DeclassExemptionAccumulator` nit (OQ-RUST-5)

---

*Generated by rust-specialist preflight agent. All §-citations verified against `crates/capco/docs/CAPCO-2016.md`. No line-number anchors used — all references are function/test/section symbolic.*
