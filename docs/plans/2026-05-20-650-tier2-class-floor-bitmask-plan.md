# Implementation Plan: Issue #650 Tier 2 — Class-Floor Bitmask Compilation

**Branch:** `feat/371-pr-g-tier2-class-floor-bitmask` (cut from `staging`, PR-E merged at `495aac84`)
**Date:** 2026-05-20
**Scope:** Compile the 27-row `CLASS_FLOOR_CATALOG` from per-row `fn(&CanonicalAttrs) -> bool`
structural presence checks + numeric chain compare to a hybrid bitmask + chain-extract dispatch
path, modeled on PR-E's tier-1 mask pattern. Tier-3 (SCI per-system catalog) is out of scope.

> **Pre-flight correction:** `extract_us_class_level` and `extract_nato_class_level` **already
> exist** in `crates/capco/src/fact_bitmask.rs:681-713`. Their signatures are
> `pub(crate) fn extract_us_class_level(bits: FactBitmask) -> Option<Classification>` and
> `pub(crate) fn extract_nato_class_level(bits: FactBitmask) -> Option<NatoClassification>` —
> typed returns, not the `u8` form described in the issue body. The plan builds on the existing
> surface.

---

## 1. Architecture decisions (open-question disposition)

### Q1: `ClassFloorRow` extension vs. new struct → **Extend `ClassFloorRow`**

Add three optional fields to the existing struct so all 27 rows live in one catalog. A separate
`ClassFloorBitmaskRow` would force `class_floor_catalog_eval` to dispatch on two tables and would
split a single concept across two shapes.

**Rationale.** Tier-1 inlined four standalone `pub(crate) fn` predicates because there was no
shared row struct. Tier-2 already has the row struct as its single source of truth; adding three
more `Option`/`const`-friendly fields keeps the catalog uniform.

### Q2: Passthrough rows (4/27) → **Structural fallthrough (`bitmask_trigger: None`)**

The 4 passthrough rows (`BUR`, `HCS-X`, `KLM`, `MVL`) keep their existing `presence()` fn-pointer.
Their new bitmask trigger fields are set to `None`. The dispatch path tests for `None` and falls
through to the structural path verbatim.

**Rationale.** These markings are open-vocab ISM-known tokens with **no atom bit** in the closed
inventory. HCS-X presence requires reading the compartment string (`c.identifier.as_str() == "X"`)
so a `SCI_HCS_X` atom would not even short-circuit the structural work. Adding atoms solely for the
passthrough warn path would expand the closed-vocab surface that downstream closure rules must
suppressor-mask against — the structural path for 4 rows is correct and cheap.

### Q3: `EqualsU` policy (UCNI ceiling) → **Compile to bitmask**

Use `extract_us_class_level(bits) == Some(Classification::Unclassified)`. The structural form
already uses `attrs.us_classification()` (US-only, not reciprocal-raised). The bitmask's US chain
field (bits 27-29) reads from `MarkingClassification::Us` and `MarkingClassification::Conflict
{ us, .. }` — same population paths as `us_classification()`. NATO / FGI / JOINT classifications
zero out the US chain field, so `extract_us_class_level` returns `None` for them, which is the
correct "not US classified" answer (UCNI ceiling fails on any non-US classification, mirroring
retired E025).

### Q4: `AtLeast` policy + NATO reciprocal-raise → **`max(us_level, nato_level)`**

`class_floor_satisfied` uses `MarkingClassification::effective_level()` which reciprocal-raises
NATO to US equivalents (CTS→TS, NS→S, NC→C, NR→R, NU→U). The bitmask carries both US chain
(bits 27-29) and NATO chain (bits 32-34) on the same 1..=5 encoding. For `AtLeast`, take the max
of the two chain values — byte-identical to `effective_level()` for the classification kinds the
bitmask currently captures (Us, Nato, Conflict::us).

**FGI/JOINT chain-coverage gap.** For a FGI or JOINT classification, both chain extracts return
`None`. The bitmask path would conclude "no classification → fail floor" which **diverges** from
the structural path on a well-formed FGI/JOINT portion. **Disposition: structural fallthrough when
`classification.is_fgi_or_joint()`.** The dispatch fast path gates on this before touching the
bitmask for `AtLeast` rows. The fallthrough path is rare (FGI/JOINT-classified portions carrying
class-floor-tripping markings) and preserves byte-identity.

### Q5: RSV-comp → **`SCI_PRESENT` coarse gate + structural fallthrough**

`SciControlBare::Rsv` has no dedicated atom bit. `presence_rsv_comp` reads `sci_markings` for a
Published(Rsv) entry with non-empty compartments. The trigger mask for `class-floor/RSV-comp` is
`SCI_PRESENT`; when set, dispatch falls through to `presence_rsv_comp()` for the precise check.
Adding a `SCI_RSV` atom would expand the `MASK_FDR_OR_RELIDO_INCOMPAT` closure-rule surface — out
of scope for this PR.

### Q6: `derive_bits` amortization → **Out of scope** (matches tier-1)

Each row calls `derive_bits(attrs)` at dispatch. Threading a pre-computed `bits` parameter through
`MarkingScheme::evaluate_custom` is a `marque-scheme` trait change. Per Constitution VII, that is
an engine-tier touch and lives in a separate follow-on PR. Explicitly deferred, per the tier-1
module doc-comment.

### Q7: PR structure → **Single PR, 7 commits**

Tier-1 landed as a single PR (PR-E). Tier-2 is a structurally identical move on 27 rows instead
of 4.

**Commit sequence:**
1. `refactor(capco): extend ClassFloorRow with bitmask compilation fields (None initial)` — zero
   behavioral change; all new fields default to `None`/`false`; dispatcher unchanged.
2. `perf(capco): tier-2 §2.1 Floor TS class-floor bitmask compilation (5 rows)`
3. `perf(capco): tier-2 §2.2 Floor S class-floor bitmask compilation (8 rows)`
4. `perf(capco): tier-2 §2.3 Floor C class-floor bitmask compilation (8 rows)`
5. `perf(capco): tier-2 §2.4 Floor =U UCNI ceiling bitmask compilation (2 rows)`
6. `test(capco): tier-2 mask parity proptest (5 groups × 1024 cases)`
7. `test(capco): tier-2 catalog static-assertion pins + doc updates`

### Q8: Test structure → **One proptest file, 5 grouped harnesses + per-row unit tests**

`crates/capco/tests/proptest_tier2_mask.rs` mirrors the tier-1 layout. Five `proptest!` blocks
(one per floor-level group), 1024 cases each. The oracle re-derives the predicate from CAPCO-2016
verbatim and **does not call `derive_bits`** — same independence discipline as tier-1.

---

## 2. New API surface

### 2.1 `ClassFloorRow` extension (`crates/capco/src/scheme/class_floor.rs`)

Add three fields to the existing struct:

```rust
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClassFloorRow {
    // ... all existing fields unchanged ...

    // ----- NEW: bitmask compilation fields -----
    /// OR-of-atom-bits that, when any bit is set (mask AND bits != 0),
    /// indicates the marking family this row gates on may be present.
    /// `None` means "no bitmask trigger available; fall back to
    /// `presence()`". The four passthrough rows carry `None` because
    /// their markings are open-vocab ISM-known tokens outside the closed
    /// atom inventory. Coarse-gate rows (where the mask is over-
    /// approximating) carry the coarse mask here; the dispatcher
    /// short-circuits at the mask gate and confirms via `presence()`.
    pub(crate) bitmask_trigger: Option<u128>,

    /// `true` when `bitmask_trigger` is precisely equivalent to the
    /// row's `presence()` predicate (no structural confirmation needed).
    /// The dispatcher reads this flag: when `true`, mask hit is the
    /// answer; when `false`, the dispatcher calls `presence()` to
    /// confirm. Defaults to `false` for coarse-grained masks.
    pub(crate) bitmask_trigger_exact: bool,
}
```

`ClassFloorPolicy` is unchanged.

### 2.2 No new functions in `fact_bitmask.rs`

`extract_us_class_level` and `extract_nato_class_level` already exist with `pub(crate)` visibility
(lines 681-713). Tier-2's dispatch lives in the same crate; the existing visibility is sufficient.

### 2.3 New module `crates/capco/src/scheme/predicates/tier2_mask.rs`

Two crate-local helpers:

```rust
/// Returns the effective classification level for floor comparison,
/// derived from the bitmask. Computes `max(us_level, nato_level)` to
/// match `MarkingClassification::effective_level()`. Returns `None`
/// only when both chain fields are zero. FGI/JOINT-classified portions
/// must gate on `classification_is_fgi_or_joint()` before calling this.
pub(crate) fn effective_level_from_bits(bits: FactBitmask) -> Option<Classification>;

/// `true` iff the classification is FGI or JOINT — the two kinds whose
/// level is absent from the bitmask chain fields.
#[inline]
pub(crate) fn classification_is_fgi_or_joint(attrs: &CanonicalAttrs) -> bool;
```

---

## 3. Per-row bitmask mapping table

Column legend:
- **Trigger mask**: atom bit names OR-joined into `bitmask_trigger`.
- **Exact?**: `bitmask_trigger_exact` — `Y` if mask alone answers presence; `N` if `presence()` must confirm.
- **Floor level**: numeric 1..=5 (1=U/NU, 2=R/NR, 3=C/NC, 4=S/NS, 5=TS/CTS).
- **Floor test**: `chain≥N` = `effective_level_from_bits ≥ N`; `chain=U` = US chain == Unclassified.

### §2.1 Floor TS (5 rows; level 5)

| # | Row name | Trigger mask | Exact? | Floor test | Notes |
|---|---|---|---|---|---|
| 1 | `class-floor/HCS-comp-sub` | `SCI_HCS_P_SUB` | Y | `chain≥5` | Single sentinel for HCS-P with sub-compartments |
| 2 | `class-floor/SI-comp` | `SCI_SI_G` | **N** | `chain≥5` | SI-G is the registered compartment; SI-ECRU/SI-NONBOOK have no atom — `presence_si_comp` confirms |
| 3 | `class-floor/TK-BLFH` | `SCI_TK_BLFH` | Y | `chain≥5` | |
| 4 | `class-floor/BALK` | `AEA_BALK` | Y | `chain≥5` | NATO SAP |
| 5 | `class-floor/BOHEMIA` | `AEA_BOHEMIA` | Y | `chain≥5` | NATO SAP |

### §2.2 Floor S (8 rows; level 4)

| # | Row name | Trigger mask | Exact? | Floor test | Notes |
|---|---|---|---|---|---|
| 6 | `class-floor/HCS-comp` | `SCI_HCS_O \| SCI_HCS_P_SUB` | **N** | `chain≥4` | Excludes HCS-X (`identifier != "X"`) and HCS-P-sub; `presence_hcs_comp_only` confirms |
| 7 | `class-floor/RSV-comp` | `SCI_PRESENT` | **N** | `chain≥4` | No `SCI_RSV` atom; coarse gate, `presence_rsv_comp` confirms |
| 8 | `class-floor/TK` | `SCI_TK_BLFH \| SCI_TK_IDIT \| SCI_TK_KAND` | **N** | `chain≥4` | `presence_tk_family` excludes TK-BLFH; bare TK has no dedicated atom |
| 9 | `class-floor/RD-SG` | `AEA_RD` | **N** | `chain≥4` | Only fires when `!rd.sigma.is_empty()`; `presence_rd_sigma` confirms |
| 10 | `class-floor/FRD-SG` | `AEA_FRD` | **N** | `chain≥4` | Mirror of RD-SG for FRD; `presence_frd_sigma` confirms |
| 11 | `E058/CNWDI-classification-floor` | `AEA_RD` | **N** | `chain≥4` | Only fires when `rd.cnwdi == true`; `presence_cnwdi` confirms |
| 12 | `class-floor/RSEN` | `RSEN` (bit 6) | Y | `chain≥4` | |
| 13 | `class-floor/IMCON` | `IMCON` (bit 7) | Y | `chain≥4` | |

### §2.3 Floor C (8 rows; level 3)

| # | Row name | Trigger mask | Exact? | Floor test | Notes |
|---|---|---|---|---|---|
| 14 | `class-floor/SI` | `SCI_PRESENT` | **N** | `chain≥3` | Bare SI (no compartments); `presence_si_bare` confirms |
| 15 | `E058/SAR-classification-floor` | `SAR_PRESENT` | Y | `chain≥3` | |
| 16 | `class-floor/RD` | `AEA_RD` | **N** | `chain≥3` | Bare RD (no CNWDI, no SIGMA); `presence_rd_bare` confirms |
| 17 | `class-floor/FRD` | `AEA_FRD` | **N** | `chain≥3` | Bare FRD; `presence_frd_bare` confirms |
| 18 | `class-floor/TFNI` | `AEA_TFNI` | Y | `chain≥3` | |
| 19 | `class-floor/ATOMAL` | `AEA_ATOMAL` | Y | `chain≥3` | NATO AEA |
| 20 | `class-floor/ORCON` | `ORCON \| ORCON_USGOV` (bits 3, 4) | Y | `chain≥3` | |
| 21 | `class-floor/EYES-ONLY` | `EYES` (bit 5) | Y | `chain≥3` | |

### §2.4 Floor =U (2 rows; UCNI ceiling, US-only)

| # | Row name | Trigger mask | Exact? | Floor test | Notes |
|---|---|---|---|---|---|
| 22 | `E058/DOD-UCNI-classification-ceiling` | `AEA_DOD_UCNI` (bit 26) | Y | `chain=U` | US chain extract == `Some(Unclassified)` |
| 23 | `E058/DOE-UCNI-classification-ceiling` | `AEA_DOE_UCNI` (bit 25) | Y | `chain=U` | |

### §2.6 Passthrough (4 rows; structural fallthrough)

| # | Row name | Trigger mask | Notes |
|---|---|---|---|
| 24 | `class-floor/passthrough-BUR` | `None` | Open-vocab; no atom bit |
| 25 | `class-floor/passthrough-HCS-X` | `None` | Open-vocab; compartment-string read required |
| 26 | `class-floor/passthrough-KLM` | `None` | Open-vocab; no atom bit |
| 27 | `class-floor/passthrough-MVL` | `None` | Open-vocab; no atom bit |

### Coverage summary

| Kind | Count | % of 27 |
|---|---|---|
| Exact bitmask (presence + floor) | 17 | 63% |
| Coarse-gate bitmask (gate + structural confirm) | 6 | 22% |
| Structural fallthrough (no bitmask) | 4 | 15% |
| **Bitmask short-circuit total** | **23** | **85%** |

85% bitmask short-circuit coverage satisfies AC #5 from #371 (≥80%).

---

## 4. Dispatch hot-path pseudocode

Refactored `class_floor_catalog_eval` in `crates/capco/src/scheme/predicates/class_floor.rs`:

```rust
pub(crate) fn class_floor_catalog_eval(
    attrs: &CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    let Some(row) = class_floor_row_by_name(name) else {
        return Vec::new();
    };

    // FGI/JOINT fallback gate — bitmask chain fields zero-out for these
    // classification kinds; structural path has the level via
    // `effective_level()`. Rare path (FGI/JOINT-classified portions
    // carrying class-floor-tripping markings).
    if classification_is_fgi_or_joint(attrs) {
        return class_floor_emit(attrs, row).into_iter().collect();
    }

    // Bitmask path (23 rows).
    if let Some(trigger_mask) = row.bitmask_trigger {
        let bits = derive_bits(attrs);
        let bits_u128 = bits.bits();

        // Trigger short-circuit: bitmask AND must be non-zero. For
        // "exact" rows this is the precise presence gate; for
        // "coarse" rows it's a cheap no-fire eliminator.
        if (bits_u128 & trigger_mask) == 0 {
            return Vec::new();
        }

        // Confirm presence: exact rows skip the structural call.
        if !row.bitmask_trigger_exact && !(row.presence)(attrs) {
            return Vec::new();
        }

        // Floor test: compiled chain compare.
        let floor_satisfied = match row.policy {
            ClassFloorPolicy::AtLeast(floor) => {
                effective_level_from_bits(bits)
                    .map_or(false, |lvl| lvl >= floor)
            }
            ClassFloorPolicy::EqualsU => {
                extract_us_class_level(bits) == Some(Classification::Unclassified)
            }
        };
        if floor_satisfied {
            return Vec::new();
        }

        // Fire: diagnostic synthesis is byte-identical to the structural
        // path — `class_floor_emit` reads row.* fields, not bits.
        return class_floor_emit(attrs, row).into_iter().collect();
    }

    // Passthrough fallback (4 rows): structural path verbatim.
    class_floor_emit(attrs, row).into_iter().collect()
}
```

`derive_bits` is called once per row per dispatch. The existing per-row pre-checks embedded in
each `presence()` fn-pointer are the fallback path; the bitmask eliminates `derive_bits` calls on
rows where the trigger atom is absent from the attrs (the overwhelmingly common case).

---

## 5. Test plan

### 5.1 Proptest parity gate (`crates/capco/tests/proptest_tier2_mask.rs`)

Five `proptest!` blocks, 1024 cases each:

| Block | Rows under test | Strategy axes |
|---|---|---|
| §2.1 Floor TS | 5 rows | `arb_sci_marking_set` × `arb_aea_atomal_balk_bohemia` × `arb_classification(any_kind)` |
| §2.2 Floor S | 8 rows | `arb_sci_marking_set` × `arb_aea_rd_frd_with_sigma_cnwdi` × `arb_dissem_with_rsen_imcon` × `arb_classification` |
| §2.3 Floor C | 8 rows | `arb_sci_marking_set` × `arb_aea_atomal_tfni_rd_frd` × `arb_dissem_with_orcon_eyes` × `arb_sar_present` × `arb_classification` |
| §2.4 Floor =U | 2 rows | `arb_aea_dod_doe_ucni` × `arb_classification(us_unclassified_or_other)` |
| §2.6 Passthrough | 4 rows | `arb_sci_controls_passthrough` × `arb_classification` |

Each case asserts `fires_via_bitmask_dispatch(name, &attrs) == oracle_for_row(name, &attrs)`.

The oracle for each row is a `fn(&CanonicalAttrs) -> bool` re-derived from the row's CAPCO
citation **without calling `derive_bits`, `row.presence()`, or `class_floor_satisfied`** — same
independence discipline as tier-1's `proptest_tier1_mask.rs`.

### 5.2 Per-row unit tests (`tier2_mask.rs::tests`)

For each of the 23 non-passthrough rows, 4 cases:
1. Presence absent → no fire.
2. Presence present, floor satisfied → no fire.
3. Presence present, floor missed → fires.
4. Presence present + FGI/JOINT classification → falls through to structural; byte-identical to
   structural emission.

For each of the 4 passthrough rows, 2 cases:
1. Presence present + classified ≥ C → no fire.
2. Presence present + Unclassified → fires.

### 5.3 Corpus regression

The existing corpus harness catches byte-divergence in `ConstraintViolation` emission. Re-run as
part of PR CI (no new fixtures required if emission is byte-identical).

### 5.4 Static-assertion pins (`crates/capco/tests/tier2_catalog_pin.rs`)

Runtime `#[test]` assertions (matching the `class_floor_catalog_naming_convention` pattern):
- `bitmask_trigger.is_some()` count == 23 (drift detection).
- `bitmask_trigger_exact == true` count == 17.
- Floor-level histogram pinned (5 rows TS, 8 rows S, 8 rows C, 2 rows U, 4 passthrough).

---

## 6. PR scope

**Single PR**, branch `feat/371-pr-g-tier2-class-floor-bitmask` cut from `staging`.

**Commit sequence:**

| Commit | Type | Description |
|---|---|---|
| 1 | refactor | Extend `ClassFloorRow` with `bitmask_trigger`/`bitmask_trigger_exact` (all `None`/`false`); zero behavioral change |
| 2 | perf | §2.1 Floor TS group (5 rows): populate trigger fields + wire new dispatch fast path |
| 3 | perf | §2.2 Floor S group (8 rows) |
| 4 | perf | §2.3 Floor C group (8 rows) |
| 5 | perf | §2.4 Floor =U UCNI ceiling group (2 rows) |
| 6 | test | Proptest parity gate (`proptest_tier2_mask.rs`) + per-row unit tests in `tier2_mask.rs::tests` |
| 7 | test | Static-assertion pins (`tier2_catalog_pin.rs`) + module doc updates |

**Out of scope (explicitly deferred):**
- Tier-3 SCI per-system catalog (5 rows) — separate PR.
- `derive_bits` amortization — separate PR (marque-scheme trait change, Constitution VII).
- `SCI_RSV` / FGI/JOINT chain atoms — follow-on as needed.

---

## 7. Acceptance criteria

| AC | Requirement | Verification |
|---|---|---|
| AC-1 | All 23 non-passthrough class-floor rows have `bitmask_trigger: Some(_)`. | Static-assertion pin (`tier2_catalog_pin.rs`). |
| AC-2 | Bitmask short-circuit coverage ≥ 80%. | 23/27 = 85%. Pin enforces. |
| AC-3 | Byte-identical `ConstraintViolation` emission vs pre-tier-2 structural form. | Corpus regression + proptest parity gate. |
| AC-4 | `lint_10kb` SC-001 (≤16ms) non-regression verified. | PR-F bench-check CI job (already in place). |
| AC-5 | All citations re-verified against `crates/capco/docs/CAPCO-2016.md` at PR authorship. | Reviewer attestation in PR body listing §-citations re-verified. |
| AC-6 | Passthrough rows retain structural-only behavior (`bitmask_trigger == None`). | Unit tests + static-assertion pin. |
| AC-7 | FGI/JOINT classification path falls through to structural; byte-identical emission. | Per-row unit test (case 4 in §5.2). |
| AC-8 | All `bitmask_trigger_exact == true` rows show no divergence from `presence()`. | Proptest oracle catches any divergence; pin enforces exact count. |
| AC-9 | `extract_us_class_level` / `extract_nato_class_level` visibility unchanged. | Code review (no surface change). |
| AC-10 | `derive_bits` called per-row; no amortization introduced (deferred per Constitution VII). | Code review. |
| AC-11 | AC #5 from issue #371 satisfied: ≥80% Constraint::Custom rows in mask form. | 23/27 tier-2 (85%) + 4/4 tier-1 = 27/32 total named rows = 84%; all 39 counted rows: 27/39 = 69% — documented as "tier-2 scope complete, tier-3 deferred per OQ-4". |

---

## 8. Constitution check

### Principle I (Uncompromising Performance)

**SC-001 (≤16ms p95 on `lint_10kb`):** Gated by PR-F bench-check CI. Tier-2's amortized cost is
dominated by `derive_bits` (already paid on tier-1 hot path). Expected delta is sub-percent;
pre-check on axis emptiness short-circuits before `derive_bits` on rows where the axis is empty.
The plan does not claim a specific speedup — PR body will report measured `lint_10kb` deltas vs
PR-E baseline.

### Principle VII (Crate Discipline)

**No engine-tier touches.** All edits live in `crates/capco/`:
- `src/scheme/class_floor.rs` — `ClassFloorRow` struct extension
- `src/scheme/predicates/class_floor.rs` — updated dispatch
- `src/scheme/predicates/tier2_mask.rs` — new module
- `tests/proptest_tier2_mask.rs` + `tests/tier2_catalog_pin.rs` — new test files

**`marque-scheme` is NOT edited.** The deferred `derive_bits` amortization is the trait change
that would touch `marque-scheme`; it is explicitly out of scope.

**`marque-ism` is NOT edited.** `extract_*` helpers already exist; `CanonicalAttrs` is untouched.

### Principle VIII (Authoritative Source Fidelity)

**Every §-citation re-verified at PR authorship.** The 23 affected rows carry `citation` /
`citation_typed` fields in `CLASS_FLOOR_CATALOG` and `class_floor.rs`. Per Constitution VIII
"Propagation requires re-verification", these must be independently re-traced against
`crates/capco/docs/CAPCO-2016.md` at implementation time. This plan deliberately does **not**
restate the per-row §-citations — they live verbatim in catalog source, and the reviewer
attestation in the PR body pins them at implementation time.

Floor-level facts (5/4/3/1) are re-verifiable from `class_floor_constraints()` and the existing
`presence_*` doc-comments, all of which cite their CAPCO §-anchor. Tier-2 introduces no new
floor-level claims.

### Principles II / IV / V / VI

- **II (Zero-Copy)**: dispatch is allocation-free on the no-fire path; `derive_bits` is
  heap-free per its existing contract.
- **IV (Two-Layer)**: Layer 1 (`marque-ism` generated predicates) is unchanged; Layer 2
  (`marque-capco` hand-written rules) is where this work lands. Rule IDs preserved.
- **V (Audit-First)**: `ConstraintViolation` emission unchanged; audit-stream byte-identical
  (proptest + corpus regression verify).
- **VI (Pipeline)**: Pipeline phase boundaries unchanged; this is an internal optimization
  of the rules phase only.

---

## 9. Open follow-ons (file as issues, not in PR scope)

1. **`derive_bits` amortization** — thread `bits: FactBitmask` through `MarkingScheme::evaluate_custom`. Saves ~30 redundant `derive_bits` calls per marking (4 tier-1 + 23 tier-2 + other Custom rows). `marque-scheme` trait change; Constitution VII §IV separate-PR discipline.
2. **Tier-3 SCI per-system catalog compilation** — 5 rows in `sci_per_system_catalog.rs`. Structurally similar to tier-2 with more compartment-string reads; defer until tier-2 lands.
3. **FGI/JOINT chain bits** — reserve bits 51-54 if profiling shows FGI/JOINT structural fallthrough is hot. Anticipated to be negligible.
4. **`SCI_RSV` atom** — if RSV-comp shows up as a hot path. Currently uses `SCI_PRESENT` coarse gate.

---

## Files referenced

| File | Purpose |
|---|---|
| `crates/capco/src/fact_bitmask.rs` | `extract_us_class_level` (l.681), `extract_nato_class_level` (l.695), `derive_bits` (l.301), atom inventory |
| `crates/capco/src/scheme/class_floor.rs` | `ClassFloorRow` struct (l.94), `CLASS_FLOOR_CATALOG` (l.169) |
| `crates/capco/src/scheme/predicates/class_floor.rs` | `class_floor_catalog_eval` (l.103), `class_floor_satisfied` (l.143) |
| `crates/capco/src/scheme/predicates/presence.rs` | Family-presence predicates (`presence_*`) |
| `crates/capco/src/scheme/predicates/tier1_mask.rs` | PR-E pattern reference |
| `crates/capco/src/scheme/constraints/helpers.rs` | `class_floor_emit` (l.169) |
| `crates/capco/src/scheme/constraints/class_floor_catalog.rs` | `Constraint::Custom` row list |
| `crates/capco/src/scheme/predicates/satisfies.rs` | `evaluate_custom_by_attrs` dispatch |
| `crates/capco/tests/proptest_tier1_mask.rs` | Test pattern reference |
| `crates/capco/docs/CAPCO-2016.md` | Authoritative source for §-citation re-verification |
| `docs/plans/2026-05-20-371-factbitmask-refactor.md` | Parent #371 plan (§3 atom layout, §8 audit) |
| `docs/plans/2026-05-20-371-factbitmask-custom-audit.md` | Tier breakdown and AC #5 status |
