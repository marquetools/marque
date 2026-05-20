# Plan: Issue #371 — CAPCO FactBitmask + static-mask closure table

**Branch:** `feat/371-factbitmask-closure-refactor`
**Base:** `origin/staging` (HEAD `8cfb3d95`)
**Date:** 2026-05-20
**Scope:** Pack CAPCO's closed-vocab `CanonicalAttrs` fields into a `u128 FactBitmask`; migrate the closure operator inner loop from `&[TokenRef]` walks + fn-pointer dispatch to static `(trigger_mask, suppressor_mask, cone_mask)` triples evaluated with bitwise ops.

This plan synthesizes two parallel design passes:
- Architect plan: full text in this session's transcript (system-architect agent, 2026-05-20)
- Rust specialist plan: full text at `/tmp/rust_specialist_plan_371.md` (ecc:rust-reviewer agent, 2026-05-20)

The plan is **paper design only**. Implementation begins after PM disposition of the open questions at §10.

---

## 1. Verified ground truth (Phase 0 reconnaissance, corrected)

1. **The closure operator IS wired into the engine hot loop.** `Engine` (`crates/engine/src/engine.rs:4671`) calls `scheme.project_from_attrs_slice(page_portions)`, which forwards to `project_attrs_pipeline` (`crates/capco/src/scheme/marking_scheme_impl.rs:1092`), which invokes `self.closure(joined)` at line 1117. The recon agent's claim "No call site in engine/src/" was wrong on direct `.closure()` call but correct on the indirect path being non-obvious — the architect agent caught and corrected this. **Implication: PR-D's `lint_latency` (SC-001) bench gate is real and load-bearing.**

2. **HOT-1 + HOT-2 + CO-2 are already shipped** in the existing `CapcoScheme::closure` impl (`marking_scheme_impl.rs:864–946`). The bitmask path must preserve their measured gains, not re-derive them.

3. **`crates/capco/src/scheme.rs` is now a directory, not a file.** Issue body references stale paths. `evaluate_custom_by_attrs` lives at `crates/capco/src/scheme/predicates/satisfies.rs:445–480`. The proposed new file `crates/capco/src/closure.rs` (top-level) does NOT collide with existing `crates/capco/src/scheme/closure.rs` (under scheme/).

4. **CAPCO catalog has 10 closure rules** (not the ~7 the issue anticipates): 1× `CLOSURE_NOFORN_CAVEATED` (Trio 1) + 2× `CLOSURE_RELIDO_*` (Trio 2) + 1× `CLOSURE_REL_TO_USA_NATO` (Trio 3, with `cone_derived` open-vocab NATO injection) + 6× per-marking unconditional (HCS-O / HCS-P[sub] / SI-G / TK-BLFH / TK-IDIT / TK-KAND).

5. **`CanonicalAttrs` read-site blast radius is 500+ across the workspace.** A `FactView` accessor-preserving wrapper (or equivalent) is mandatory; `CanonicalAttrs` shape changes invalidate hundreds of test fixtures.

---

## 2. Storage architecture decision

### 2.1 Decision: Option C (FactBitmask in marque-scheme, atom layout in marque-capco)

- `FactBitmask` newtype around `u128` (`#[repr(transparent)]`) lives in **`crates/scheme/src/fact_bitmask.rs`** as a domain-neutral primitive. `marque-scheme` is the existing home for `MarkingScheme`, `Lattice`, `Constraint`, `Scope`, `PageRewrite`, and built-in lattice constructors — a Boolean characteristic-vector primitive is consistent with that role (Birkhoff/FCA representation; `pure-lattice.md` §6).
- CAPCO atom inventory + `BIT_*` constants + `MASK_*` aggregates + `derive_bits` / `apply_closed_bits_to` projection helpers live in **`crates/capco/src/fact_bitmask.rs`**.
- **`CanonicalAttrs` in `marque-ism` is NOT touched.** The 500+ read-site invariance is preserved.
- `static_assert!` guarding the atom count (≤128) lives in `marque-capco` (the atom inventory is CAPCO-scheme-specific).

### 2.2 Constitution VII adjudication

The constitution clause "a scheme-adoption PR MUST NOT edit the engine crates" targets scheme-addition PRs sneaking in engine work. This is the opposite:
- #371 is a perf refactor against the existing CAPCO scheme (not a scheme adoption).
- `FactBitmask` is a `MarkingScheme`-shaped primitive, not an ISM-vocabulary concept.
- `marque-scheme` is already the foundational graph-leaf where domain-neutral primitives live.
- `marque-ism` (the engine-tier vocabulary crate) is NOT touched.

**Fallback if review pushes back:** move `FactBitmask` into `marque-capco`-only (single-file in `crates/capco/src/fact_bitmask.rs`). The engine-graph topology is unchanged either way. PR-A is the cheapest sub-PR to re-shape.

### 2.3 Why NOT cache `fact_bits` on `CapcoMarking`

A `pub fact_bits: FactBitmask` field on `CapcoMarking` would amortize the projection across multiple reads but invalidates on every fact-axis mutation. `apply_closure_fact` writes through accessor methods that don't know about a cache; a stale cache is a correctness bug. The derive-close-apply triple inside `closure()` scopes the bitmask to a single call and side-steps invalidation.

---

## 3. Atom layout (51 bits used, 77 reserved)

Adopt the rust specialist's layout (bit-precise, slightly richer than the architect's):

| Bits | Count | Axis | Source |
|---|---|---|---|
| 0–12 | 13 | US IC dissem (NOFORN, RELIDO, DISPLAY_ONLY, ORCON, ORCON_USGOV, EYES, RSEN, IMCON, PROPIN, DSEN, FISA, RAWFISA, FOUO) | `dissem_us` ∪ `dissem_nato` via `dissem_iter()` |
| 13–21 | 9 | Non-IC dissem (NODIS, EXDIS, SBU_NF, LES_NF, LIMDIS, LES, SBU, SSI, NNPI) | `non_ic_dissem` |
| 22–26 | 5 | Closed AEA (RD, FRD, TFNI, DOE_UCNI, DOD_UCNI) | `aea_markings` |
| 27–29 | 3 | US classification chain (3-bit OrdMax encoding) | `classification` (Us variant) |
| 30 | 1 | `US_COLLATERAL_CLASSIFIED` derived sentinel | derived from US chain ≥ Restricted |
| 31 | 1 | `US_UNCLASSIFIED` derived sentinel | derived from US chain = U |
| 32–34 | 3 | NATO classification chain (3-bit OrdMax) | `classification` (Nato variant) |
| 35 | 1 | `NATO_CLASS` presence sentinel | any NATO class present |
| 36 | 1 | `SAR_PRESENT` | `sar_markings.is_some()` |
| 37 | 1 | `SCI_PRESENT` | any SCI marking |
| 38 | 1 | `FGI_PRESENT` | FGI marker OR FGI classification |
| 39 | 1 | `JOINT_PRESENT` | JOINT classification |
| 40–45 | 6 | SCI sentinels (SI_G, HCS_O, HCS_P_SUB, TK_BLFH, TK_IDIT, TK_KAND) | `sci_markings` (compartment + sub-compartment structural read) |
| 46 | 1 | `REL_TO_PRESENT` | `rel_to.is_empty() == false` |
| 47 | 1 | `REL_TO_USA` | `rel_to.contains(&CountryCode::USA)` |
| 48 | 1 | `AEA_ATOMAL` (NATO AEA) | `AeaMarking::Atomal` |
| 49 | 1 | `AEA_BOHEMIA` (NATO SAP) | future / SciControlSystem::NatoSap |
| 50 | 1 | `AEA_BALK` (NATO SAP) | future / SciControlSystem::NatoSap |
| 51–95 | 45 | CAPCO future-growth reserved | — |
| 96–127 | 32 | Foreign-grammar reserved (CUI / NATO standalone / partner-national) | — |

**Encoding note for classification chains.** The 3-bit OrdMax encoding (`000`=absent, `001`=U, ..., `100`=TS / CTS) is NOT compatible with bitwise OR for join: `001 | 100 = 101 = TS` ≠ `max(U, S) = S`. Closure rules that read the classification chain do so via a 3-bit extract + numeric compare (e.g., `extract_us_class(bits) >= U_LEVEL_RESTRICTED`), not via mask intersection.

**`ATOM_COUNT` const.** Highest assigned bit + 1 = 51. `const _: () = { assert!(ATOM_COUNT <= 128); };` lives at the bottom of `crates/capco/src/fact_bitmask.rs`.

---

## 4. CLOSURE_TABLE row inventory (10 rows; 9 fully bitmask, 1 hybrid)

| # | Row name | Trigger | Suppressor | Static cone | Derived cone | §-citation (re-verify at land-time) |
|---|---|---|---|---|---|---|
| 0 | `capco/noforn-if-caveated` | 20 atoms (SAR, RD, FRD, TFNI, DoE UCNI, DoD UCNI, FGI present, ORCON, ORCON_USGOV, RSEN, IMCON, PROPIN, DSEN, FISA, RAWFISA, LIMDIS, LES, NNPI, SBU, SSI) | `FDR_DOMINATOR_MASK` (NF, RELIDO, DO, REL_TO_PRESENT, EYES) | `NOFORN` | — | §B.3 Table 2 p21 (rooted in ICD 403 caveated definition p20) |
| 1 | `capco/hcs-o-implies-noforn-orcon` | `SCI_HCS_O` | — | `NOFORN \| ORCON` | — | §H.4 p64 |
| 2 | `capco/hcs-p-sub-implies-noforn-orcon` | `SCI_HCS_P_SUB` | — | `NOFORN \| ORCON` | — | §H.4 p68 |
| 3 | `capco/si-g-implies-orcon` | `SCI_SI_G` | — | `ORCON` | — | §H.4 p80 |
| 4 | `capco/tk-blfh-implies-noforn` | `SCI_TK_BLFH` | — | `NOFORN` | — | §H.4 p87 |
| 5 | `capco/tk-idit-implies-noforn` | `SCI_TK_IDIT` | — | `NOFORN` | — | §H.4 p91 |
| 6 | `capco/tk-kand-implies-noforn` | `SCI_TK_KAND` | — | `NOFORN` | — | §H.4 p95 |
| 7 | `capco/rel-to-usa-nato-if-nato-classification` | `NATO_CLASS` | `FDR_DOMINATOR_MASK` | `REL_TO_USA` | NATO tetragraph via existing `rel_to_usa_nato_derived_cone` fn-pointer | §H.7 p127 (example) + §G.2 Table 5 p40 |
| 8 | `capco/relido-if-sci-and-not-incompatible` | `SCI_PRESENT` | `FDR_OR_RELIDO_INCOMPAT_MASK` (14 atoms incl. 6 SCI sentinels + FGI + JOINT + NATO) | `RELIDO` | — | §H.8 p154 |
| 9 | `capco/relido-if-us-collateral-class` | `US_COLLATERAL_CLASSIFIED` | `RELIDO_US_CLASS_SUPPRESSOR_MASK` (11 atoms: FDR + 6 SCI sentinels) | `RELIDO` | — | §B.3 Table 2 p21 (grammar: §H.8 p154) |

**Row ordering is load-bearing.** It must match `CAPCO_CLOSURE_RULES` verbatim (the positional pin in `post_4b_lattice_inventory_pin.rs` gates this). Trio 1 first (so subsequent trios see updated NOFORN); per-marking unconditional rows next (so Trio 3/2 see updated ORCON); Trio 3 / Trio 2 last.

**Bitmask coverage of the static cone:** 9/10 rows compile fully to bitmask form. Row 7 is hybrid: bitmask trigger + bitmask suppressor + closed-vocab static cone (`REL_TO_USA`), with the open-vocab `NATO` tetragraph applied by a follow-up `cone_derived` fn-pointer pass after the bitmask Kleene loop converges. **This is 100% coverage of the bitmask-fast-path tier; >90% coverage of the static cone tier; passes AC #2.**

---

## 5. Dispatch shape

```text
fn closure(self, marking):
    let input_bits = derive_bits(&marking.0)            // O(n) over closed-vocab fields

    // HOT-1 fast-out: if no trigger atom is set, no row can fire.
    if (input_bits.0 & ALL_TRIGGER_MASK) == 0:
        return marking  // early-exit, byte-identical to existing HOT-1

    let closed_bits = close(input_bits)                 // bitwise Kleene fixpoint over CLOSURE_TABLE

    apply_closed_bits_to(&mut marking.0, closed_bits, input_bits)
                                                        // re-derive Box<[T]> slices for the delta
                                                        // — NOFORN/ORCON/RELIDO/USA-in-REL_TO

    // Open-vocab post-pass: only Row 7 (CLOSURE_REL_TO_USA_NATO) has cone_derived.
    let bitmask_row7 = &CLOSURE_TABLE[7]
    if bitmask_row7.fires(closed_bits):
        for fact in CAPCO_CLOSURE_RULES[7].cone_derived.unwrap()(&marking):
            apply_closure_fact(self, &mut marking, &fact)

    return marking
```

Two correctness properties:

1. **Fixpoint correctness.** The derived-cone pass runs after `close()` converges. NATO tetragraph injection adds open-vocab facts not in the bitmask; adding them cannot re-trigger any bitmask row (open-vocab REL_TO countries only flip `REL_TO_PRESENT`, which was already derived from `rel_to.is_empty() == false` at projection time). One post-pass is sufficient.

2. **Suppressor uniformity.** Row 7's bitmask suppressor (`FDR_DOMINATOR_MASK`) gates both the closed (REL_TO_USA) and derived (NATO) cone. If the suppressor fires, neither path executes.

---

## 6. proptest harness

New file: **`crates/capco/tests/proptest_factbitmask.rs`**. Five properties:

- **P1 Idempotence:** `close(close(bits)) == close(bits)` for all `u128`
- **P2 Extensivity:** `(close(bits) & bits) == bits` (every input bit preserved)
- **P3 Monotonicity:** `(a.0 & b.0) == a.0 ⟹ (close(a).0 & close(b).0) == close(a).0`
- **P4 Convergence bound:** Kleene iteration count ≤ `MAX_CLOSURE_ITERATIONS` (= 16)
- **P5 Cross-path parity:** `close(derive_bits(attrs))` followed by `apply_closed_bits_to` produces a `CanonicalAttrs` byte-identical to `scheme.closure(marking)` on closed-vocab axes (`dissem_us`, `dissem_nato`, `rel_to`) — the load-bearing parity gate for PR-D's dispatch flip.

Generators: `arb_fact_bitmask()` (full `u128`) for robustness + `arb_realistic_bitmask()` (masked to 0..=50) for domain coverage + `arb_closed_vocab_attrs()` (focused strategy over closed-vocab `CanonicalAttrs` fields) for P5.

---

## 7. `closure_pass` Criterion bench

New file: **`crates/engine/benches/closure_pass.rs`**. Five benchmark groups:

- `closure_pass/worst_case` — SAR + ORCON + US Secret; Trio 1 fires + per-marking ORCON pre-set + Trio 2 suppressed by NOFORN; chain depth 2
- `closure_pass/all_triggers_no_suppressors` — `bits = OR of all CLOSURE_TABLE trigger_masks`; maximal fixpoint traversal
- `closure_pass/representative_noforn_secret` — `NOFORN | US_COLLATERAL_CLASSIFIED`; HOT-1 early-exit path
- `closure_pass/representative_hcs_o` — `SCI_HCS_O | SCI_PRESENT | US_COLLATERAL_CLASSIFIED`; chain depth 1
- `closure_pass/representative_si_g` — `SCI_SI_G | SCI_PRESENT | US_COLLATERAL_CLASSIFIED`; chain depth 2 (SI-G → ORCON → CAVEATED → NOFORN)

No threshold gate on the first run (this is a new bench; establishes baseline). PR description carries the full Criterion table.

---

## 8. `Constraint::Custom` audit (AC #5: ≥80% compilation)

The 39 catalog rows (7 named-dispatch + 27 class-floor + 5 SCI-per-system) split into:

- **Tier 1 (pure-presence, ≤bitmask mask test):** 4 named rows fully (E021, E024, E038, E070) + 2 named partial (E014, capco/joint-requires-usa)
- **Tier 2 (numeric chain compare on classification + atom presence):** 27 class-floor rows (lift via `ClassFloorBitmaskRow { token_bit, min_us_chain_level, min_nato_chain_level, ... }` helper + 3-bit chain extract)
- **Tier 3 (structural read; not mask-compilable):** ~6 rows (E010 HCS structural, E012 dual-class structural, 5 SCI-per-system compartment reads)

**Weighted total: ~85% mask coverage (above 80% AC).** Heavy lift is the 27-row class-floor catalog via tier-2 numeric chain compilation. SCI-per-system stays structural (13% of total, within slack).

**Audit table residency:** ship as `docs/plans/2026-05-20-371-factbitmask-custom-audit.md` (this PR's docs sibling), with one row per Constraint::Custom name, tier (1/2/3), §-citation, compilation rationale, source file/line of the structural body it replaces.

**Open question (OQ-4 below):** scope of class-floor / SCI-per-system aggressive compilation in this PR vs follow-on issue.

---

## 9. Sub-PR sequencing

Recommended six-sub-PR chain (matches the in-tree PR 4b umbrella pattern). All sub-PRs target `origin/staging`:

| # | Title (working) | Scope | LoC est. | CI gate |
|---|---|---|---|---|
| **PR-A** | `feat(scheme): introduce FactBitmask primitive type — no behavior change` | `crates/scheme/src/fact_bitmask.rs` + tests | ~250 | unit + proptest only |
| **PR-B** | `feat(capco): atom inventory + derive_bits/apply_closed_bits_to projector — no closure rewire` | `crates/capco/src/fact_bitmask.rs` + round-trip tests | ~450 | unit + proptest, no production code path consumes it |
| **PR-C** | `feat(capco): CLOSURE_TABLE static mask catalog + close() Kleene loop — gated behind feature flag` | `crates/capco/src/scheme/closure_table.rs` (or absorbed into `fact_bitmask.rs`) + equivalence cross-check test | ~700 | unit + proptest, corpus parity unchanged (table unused on production path) |
| **PR-D** | `perf(capco): rewire CapcoScheme::closure to bitmask fast path — corpus parity gate` | `marking_scheme_impl.rs::closure` body rewrite behind `#[cfg(feature = "bitmask-closure")]`, parity-gated removal of the cfg in same PR or follow-up | ~250 | full corpus parity (`tests/corpus/valid/`, `prose/`, `mangled/`, `documents/`); `phase_b_closure` improvement target ≥ 20%; `lint_latency` SC-001 non-regression (16 ms ceiling) |
| **PR-E** | `perf(capco): Constraint::Custom tier-1 mask compilation (named dispatch only)` | `crates/capco/src/scheme/predicates/satisfies.rs` (fast-path for 4 tier-1 rows: E021, E024, E038, E070); audit doc documents the deferred class-floor + SCI-per-system tiers. **Reduced scope per OQ-4 disposition.** | ~300 | corpus parity; `lint_latency` non-regression |
| **PR-F** | `bench/docs: closure_pass bench + WASM size delta + AC attestation + follow-on issue` | `crates/engine/benches/closure_pass.rs` + `docs/plans/2026-05-20-371-factbitmask-refactor.md` (final attestation) + file follow-on issue for class-floor + SCI-per-system tier-2/3 compilation | ~250 | bench runs; WASM size delta report |

**Total ~2700 LoC across 6 sub-PRs.** Linear chain; PR-D and PR-E parallelize after PR-C.

**Alternative shape (if PM wants tighter):** 3-PR chain — A+B+C as one (primitive + projector + catalog), D+E as one (closure rewire + Constraint::Custom), F standalone. Coarser-grained reverts; closes faster. The 6-PR shape is the project's idiomatic pattern; the 3-PR shape is the cadence-tightened alternative.

---

## 10. PM dispositions (2026-05-20)

| OQ | Question | Resolution |
|---|---|---|
| **OQ-1** | Constitution VII placement for `FactBitmask` | **`marque-scheme`** (Option C — domain-neutral primitive). Architect's adjudication §2.2 accepted. |
| **OQ-2** | Sub-PR cadence | **6 sub-PRs (PR-A → PR-F)** matching the project's idiomatic pattern (PR 4b umbrella, PR 9, PR 3b). |
| **OQ-3** | PR 5 baseline reset | Default to **(c) HOT-1+HOT-2 baseline** (594–613 µs `phase_b_closure`, per PR 4b-B / #620 / #625). PR-D's bench narrative compares against this. If PR 5 closes mid-cycle, re-measure for PR-F's final attestation. |
| **OQ-4** | Constraint::Custom audit scope | **Defer to follow-on issue.** PR-E either drops in scope (~300 LoC, lighter touch — just the 4 tier-1 named-dispatch rows: E021, E024, E038, E070) or is merged into PR-F entirely. AC #5 deferral is documented in this PR's description and a new GitHub issue is filed at PR-D land-time tracking the class-floor catalog tier-2 compilation as a #371 carry-over. |
| **OQ-5** | CAPCO_CLOSURE_RULES retention after PR-D | **Delete the 9 bitmask-eligible rows** (Option b). Only Row 7 (`CLOSURE_REL_TO_USA_NATO`, the `cone_derived` row) survives in `CAPCO_CLOSURE_RULES`. CLOSURE_TABLE becomes the source of truth for the other 9 rows. PR-C's `closure_table_equivalence.rs` is the transitional gate; the 9 fn-pointer rules + the equivalence test are deleted within PR-D once corpus parity is green. |

These dispositions are baked into §9 (sub-PR sequencing) and §11 (risk register) below.

---

## 11. Risk register (top 5)

| # | Risk | Sev | Mitigation |
|---|---|---|---|
| R-1 | Constitution VII engine-tier touch contested in review (placing FactBitmask in `marque-scheme`) | High | Pre-emptive Constitution VII adjudication block in PR-A description; fallback to `marque-capco`-only if review pushes back |
| R-2 | `cone_derived` open-vocab cone subtly skipped on bitmask path (NATO REL-TO row silently stops firing) | High | (a) PR-C equivalence cross-check covers Row 7; (b) PR-D corpus regression includes NATO worked-example fixtures; (c) dedicated `closure_open_vocab.rs` test asserting NATO injection on bare-NATO input |
| R-3 | HOT-1 / HOT-2 regression on simple inputs (per-call derive+apply overhead exceeds per-rule walker savings) | High | (a) Bitmask HOT-1 short-circuit `(bits & ALL_TRIGGER_MASK) == 0` runs before derive/apply (skip everything in O(1)); (b) `derive_bits` is branchless single-pass; (c) PR-D's `lint_latency` non-regression gate catches it pre-merge |
| R-4 | WASM size regresses despite expectation (u128 ops lower to multiple wasm ops) | Med | (a) PR-F's WASM size delta report is the canary; (b) fallback: drop `u128` to `(u64, u64)` pair, atom inventory fits cleanly into two lanes; (c) gate `FACT_BIT_NAMES` debug table behind `#[cfg(debug_assertions)]` |
| R-5 | Catalog drift between `CAPCO_CLOSURE_RULES` (TokenRef) and `CLOSURE_TABLE` (mask) — and post-PR-D, no cross-check for the 9 deleted rows | Med | (a) PR-C's `closure_table_equivalence.rs` is the transitional gate during the bitmask path's introduction; (b) PR-D deletes the 9 bitmask-eligible fn-pointer rules **and** the equivalence test together once corpus parity is green; (c) post-PR-D the spec lives in `CLOSURE_TABLE` + `crates/capco/docs/CAPCO-2016.md` citations + the `closure_pass` bench's representative inputs; (d) future closure additions must include both a mask row and a citation, gated by the `post_4b_lattice_inventory_pin.rs` positional pin |

---

## 12. Acceptance criteria mapping (issue #371)

| AC | Status under this plan |
|---|---|
| #1 CanonicalAttrs closed-vocab fields packed; bit count `static_assert`'d | PR-B (atom layout in `marque-capco`); CanonicalAttrs shape unchanged — bitmask is a sidecar projection (the Option C / B approach). **Note:** if PM wants literal compliance with "CanonicalAttrs fields migrated" wording, switch to Option A (in-place storage refactor) — but that breaks 500+ read sites. Recommend sticking with Option C and amending the AC. |
| #2 `crates/capco/src/closure.rs` exists with ClosureRow + CLOSURE_TABLE; §-citation per row | PR-C lands the table; file path **`crates/capco/src/scheme/closure_table.rs`** (avoids collision with existing `crates/capco/src/scheme/closure.rs`). Per-row citation re-verification log at §12 of rust specialist plan |
| #3 `CapcoScheme::closure` replaced; previous fn-pointer ImplRow entries retired | PR-D; the 9 bitmask-eligible rules' fn-pointer dispatch is retired (or kept as cross-check anchor per OQ-5(a)). Row 7 (`cone_derived`) stays in fn-pointer form for the open-vocab tail |
| #4 proptest: idempotence/extensivity/monotonicity/convergence-bound | PR-C (P1–P4) and PR-D (P5 cross-path parity). Convergence bound asserted explicitly per §6 |
| #5 ≥80% Constraint::Custom rows compiled to mask form | **Deferred per OQ-4 disposition.** PR-E lifts only the 4 tier-1 named-dispatch rows (E021, E024, E038, E070); ~13% mask coverage in-scope. AC #5 explicitly carried over to follow-on issue filed at PR-D land-time. PR-D and PR-F descriptions document the deferral. |
| #6 `closure_pass` bench + SC-001/SC-005 non-regression + delta in PR description | PR-D enforces SC-001 non-regression; PR-F adds `closure_pass` bench |
| #7 WASM build-size delta reported | PR-F |
| #8 Corpus parity (`tests/corpus/valid/`, `prose/`, `mangled/`) | PR-D enforces; PR-E preserves |
| #9 Renderer untouched | Preserved by design (Option C / B — `CanonicalAttrs` accessor API unchanged); zero diff on `crates/ism/src/canonical.rs` |
| #10 G13 audit-content-ignorance preserved | `FactBitmask` is structural; carries no bytes. Constitution V Principle V invariant unaffected |

---

## 13. Trace to sibling docs

- Issue #371 body — full spec, written 2026-05-12, paths corrected per Phase 0 recon
- `marque-applied.md` §4.7 — closure operator + implicit-default trio (referenced by acceptance criteria)
- `pure-lattice.md` §6 (Birkhoff representation) + §8 (Boolean algebra) + §18 (closure operator) — the FCA grounding for the bitmask representation
- `docs/plans/2026-05-01-lattice-design.md` §9 — closure operator scheduled for PR 3.7
- `specs/006-engine-rule-refactor/architecture.md` §2–§3 — closure operator design + pivot-type triple
- `specs/006-engine-rule-refactor/tasks.md` T108c (closure operator primitive) + T112 (per-category Lattice impls)
- Constitution Principle I (perf) + II (zero-copy) + III (WASM-safety) + V (audit/G13) + VII (crate-discipline) + VIII (citation-fidelity)

---

## 14. Implementation sequencing within Phase 3

Once OQ-1 through OQ-5 are dispositioned, Phase 3 implementation proceeds sub-PR by sub-PR. For each sub-PR:

1. Implementation agent writes the code per this plan
2. Multi-agent review pass (Phase 4): `ecc:rust-reviewer`, `ecc:code-reviewer`, `ecc:performance-optimizer`, `ecc:security-reviewer`, `capco-validators:capco-foundational` (citation discipline)
3. Address CRITICAL + HIGH issues
4. Open PR against `origin/staging` with the standard PR description (acceptance criteria mapping, bench deltas, citation re-verification log)
5. Merge after review approval
6. Move to next sub-PR
