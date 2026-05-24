# Implementation Plan: PR 3b.B — Eight Declarative `PageRewrite` Transmutations

**Target file**: `docs/plans/2026-05-07-pr3b-B-transmutations-plan.md`
**Branch**: `refactor-006-pr-3b-transmutations` (worktree ``)
**Scope**: Sub-task T026b of PR 3b. Declarative-data-only.

---

## 1. Executive Summary

PR 3b.B adds **eight new `PageRewrite` rows** to `CapcoScheme::build_page_rewrites()` (in `crates/capco/src/scheme.rs`) to make the cross-axis "transmutation on contact" semantics from CAPCO-2016 §H.3 / §H.6 / §H.7 / §H.8 / §H.9 visible to the engine's topological scheduler. Six are from the lattice consultant's §3.4.1 transmutation roster, with consultant-Entry-6 split into 6a (SBU-NF) + 6b (LES-NF) per D13 single-citation discipline; one is the §3.4.3 cross-axis FGI rollup. All eight use `CategoryPredicate::Custom(never_fires)` triggers — they are **scheduler-visible dataflow annotations only**, not active runtime rewrites. Runtime page-rewrite execution remains in the hand-coded `PageContext` aggregator until Phase D/E.

**Disposition of existing stubs.** Of the three existing `PageRewrite` rows, `capco/noforn-clears-rel-to` is retained verbatim (it is the only currently-active rewrite, well-grounded). The two existing Phase-3 stubs (`capco/joint-promotion`, `capco/fgi-absorption`) are **replaced** by the new entries 1–3 + 7, which model the same semantics with the consultant's §3.4 framing and a tighter source-citation chain.

**Net rewrite delta.** 3 existing → 1 retained + 8 new = **9 total rows** (net +6 rewrites), with 2 stub retirements absorbed by the §3.4.1 framing.

**Net rule delta target.** Realistic outcome is **0 rules retired** in this sub-PR. Runtime is still `PageContext`-driven; nothing the eight entries declare can confidently retire a hand-coded rule until Phase D/E activates dispatch. Recommendation: **land as pure-additive declarative data; do not retire any rule in this sub-PR.** The 3b consultation verdict's "−1 to −3" target for 3b.B is satisfied by the 2-stub retirement.

**D13 attestation.** Each of the eight new entries carries exactly one `§-citation` string verified against the vendored CAPCO-2016 source. Citations are quoted in §3 below for reviewer re-verification.

---

## 2. Disposition of the Existing Three Rewrites

| Existing rewrite | Disposition | Justification |
|---|---|---|
| `capco/noforn-clears-rel-to` (lines 486–498) | **Keep verbatim** | The only currently-active page rewrite (real `Contains` predicate, real `Clear` action). Cited at §D.2 Table 3 + §H.8 p145. The consultant's §3.4 calls it out as the canonical and uncontested example. No semantic overlap with the eight new entries. |
| `capco/joint-promotion` (lines 515–526) | **Retire — replaced by entries 1, 3, 7** | The consultant's marque-applied.md §3.4 explicitly calls this stub "stubbed at scheme.rs:515–526 pending Phase D/E." The §3.4.1 entries 1 (bare-FGI), 3 (JOINT cross-class), and 7 (US-presence FGI rollup) replace it with finer-grained, properly-cited transmutations. |
| `capco/fgi-absorption` (lines 537–548) | **Retire — replaced by entries 1, 7** | Same rationale: §3.4 names this as a Phase-D/E stub. The within-axis FGI absorption is exactly entry 7 plus entry 1. |

The `noforn-clears-rel-to` row stays at the top of the rewrite vector. Entries 1, 2, 3, 4, 5, 6a, 6b, 7 are appended after it. The two retired stubs are removed from the vector and the `JP_READS`, `JP_WRITES`, `FA_READS`, `FA_WRITES` const-slice declarations are deleted.

---

## 3. The Eight New Entries — Full Implementation Spec

### Entry 1: `capco/fgi-rollup-on-us-contact`

- **Summary**: Bare-FGI portion contacting a US-class portion forces page-level FGI rollup (concealed → bare; acknowledged → `FGI [list]`).
- **Citation string**: `"CAPCO-2016 §H.7 p123"`
- **Cited passage** (verified against `crates/capco/docs/CAPCO-2016.md` line 3099):
  > "If any document contains portions of both source-concealed FGI, e.g., '(//FGI S//REL TO USA, GBR)' and source-acknowledged FGI, e.g., '(//GBR S//REL TO USA, GBR)', then only the 'FGI' marking without the source trigraph(s)/tetragraph(s) must appear in the banner line."
- **`reads`**: `&[CAT_CLASSIFICATION]` — narrow form. Predicate would scan `CAT_FGI_MARKER` for bare-FGI atoms; documented in doc-comment, not in `reads` to avoid mutual cycles among entries 1/2/3 per §4.
- **`writes`**: `&[CAT_FGI_MARKER]` — single-axis write. Reciprocal-raise of class is performed at portion-parse-time per §3.4.1 Note (i), NOT as a rewrite transform; CLASS is not in `writes`.
- **Predicate**: `CategoryPredicate::Custom(never_fires)` — Phase-3 stub.
- **Action**: `CategoryAction::Custom(noop_action)` — Phase-3 no-op. Add a `noop_action` helper alongside the existing `never_fires` (the previous `identity_promote` helper, used only by the retired `joint-promotion` / `fgi-absorption` stubs, is removed in this PR as dead code).
- **Monotonicity**: **Monotone-additive** on FGI axis (concealed wins; acknowledged unions). CLASS not mutated by this rewrite. Sound under fixed topological order per `marque-applied.md` §6 Framing 3.
- **Phase 3 stub justification**: trigger is `never_fires` because runtime dispatch stays in `PageContext` until Phase D/E; the `Custom` action body is a no-op for the same reason — the `reads` / `writes` annotations are what the scheduler consumes.

### Entry 2: `capco/fgi-restricted-rollup-on-us-contact`

- **Summary**: Bare-FGI-R portion contacting US-class portion lifts US class to ≥ C (RESTRICTED reciprocal-raise) and rolls FGI attribution to `[list]`.
- **Citation string**: `"CAPCO-2016 §H.7 p123"`
- **Cited passage** (verified against CAPCO-2016.md line 3096):
  > "May be used with TOP SECRET, SECRET, CONFIDENTIAL, RESTRICTED, UNCLASSIFIED, and other designators applied to sensitive information as identified in the Manual Appendix A, Enclosure 1 (e.g., Non-National Security Classification markings) applied by the non-US originator (unique markings may reveal source country)."
  Plus the page-123 prose at line 3099 (the FGI banner-rollup discipline; quoted under Entry 1) which establishes the rollup contract.
- **`reads`**: `&[CAT_CLASSIFICATION]` — narrow form. Predicate scans `CAT_FGI_MARKER` for bare-FGI-R atoms (doc-comment only).
- **`writes`**: `&[CAT_FGI_MARKER]` — class lift to ≥ C is parser-side per Note (i).
- **Predicate**: `CategoryPredicate::Custom(never_fires)`.
- **Action**: `CategoryAction::Custom(noop_action)`.
- **Monotonicity**: **Monotone-additive** on FGI axis (R-classified countries union into the trigraph list). Class lift is parser-side and monotone (R → C is upward only).

### Entry 3: `capco/joint-cross-class-rollup`

- **Summary**: JOINT [list] portion contacting a non-US-class portion produces banner US-class with FGI [non-US JOINT members].
- **Citation string**: `"CAPCO-2016 §H.3 p57"`
- **Cited passage** (verified against CAPCO-2016.md lines 1288–1290):
  > "Highest classification level of all portions, expressed as a US classification marking. Note: The JOINT marking is not carried forward to the banner line in US documents, but remains for applicable portions. The FGI marking including all trigraph/tetragraph codes identified in the JOINT portion(s). REL TO, including all common non-US country trigraph/tetragraph codes identified in the JOINT portions, unless a portion is marked NOFORN, in which case the NOFORN marking must appear in the banner line."
- **`reads`**: `&[CAT_CLASSIFICATION, CAT_JOINT_CLASSIFICATION]` — narrow form. JOINT scan IS the trigger read; FGI_MARKER is the write target only.
- **`writes`**: `&[CAT_FGI_MARKER]` — class lift is parser-side; JOINT marking does not roll up to banner per §H.3 p57 (so JOINT is not written either).
- **Predicate**: `CategoryPredicate::Custom(never_fires)`.
- **Action**: `CategoryAction::Custom(noop_action)`.
- **Monotonicity**: **Monotone-additive** on FGI axis (non-US JOINT members union in).

### Entry 4: `capco/frd-sigma-consolidates-into-rd-sigma`

- **Summary**: FRD-SIGMA #_b co-occurring with RD-SIGMA #_a consolidates into a single RD-SIGMA tracking [#_a, #_b] in the banner.
- **Citation string**: `"CAPCO-2016 §H.6 p113"`
- **Cited passage** (verified against CAPCO-2016.md line 2804):
  > "If both RD and FRD SIGMA [#] portions are in a document, the RD-SIGMA [#] marking takes precedence over the FRD-SIGMA [#] marking in the banner line and all SIGMA numbers are listed in the banner line RD-SIGMA [#] marking, regardless of whether the information was RD or FRD."
- **`reads`**: `&[CAT_AEA]`
- **`writes`**: `&[CAT_AEA]`
- **Predicate**: `CategoryPredicate::Custom(never_fires)`.
- **Action**: `CategoryAction::Custom(noop_action)` — within-axis transform (drop FRD-SIGMA atoms, fold their #s into RD-SIGMA atom).
- **Monotonicity**: **Shrinking** — drops FRD-SIGMA atoms from `CAT_AEA`. Sound under fixed topological order.

### Entry 5: `capco/orcon-nato-to-us-orcon-on-us-contact`

- **Summary**: ORCON-NATO contacting US-class transmutes to US ORCON in the page dissem axis.
- **Citation string**: `"CAPCO-2016 §H.8 p136"`
- **Cited passage** (verified against CAPCO-2016.md line 3367):
  > "If ORCON and ORCON-USGOV portions are in a document, ORCON takes precedence and is conveyed in the banner line."
- **Citation note** (in doc-comment): ORCON-NATO is referenced in CAPCO-2016 line 895 ("ORCON (NATO dissemination control marking)") and Appendix B. The §H.8 p136 ORCON-precedence rule extends to ORCON-NATO via the Appendix B mapping. D13 single-citation discipline keeps `§H.8 p136` as the primary anchor; the Appendix B mapping is documented in the doc-comment.
- **`reads`**: `&[CAT_CLASSIFICATION]` — narrow form. Predicate scans `CAT_DISSEM` for ORCON-NATO; documented in doc-comment, not in `reads` to avoid mutual cycles among 5/6a/6b per §4.
- **`writes`**: `&[CAT_DISSEM]`.
- **Predicate**: `CategoryPredicate::Custom(never_fires)`.
- **Action**: `CategoryAction::Custom(noop_action)`.
- **Monotonicity**: **Mixed** — drops ORCON-NATO (shrinking) and adds ORCON (additive). Sound under fixed topological order.

### Entry 6a: `capco/sbu-nf-transmutes-on-classified-contact`

- **Summary**: SBU-NF portion contacting any IC marking transmutes — class > U drops SBU-NF entirely; class = U replaces SBU-NF with NOFORN + SBU.
- **Citation string**: `"CAPCO-2016 §H.9 p178"`
- **Cited passage** (verified against CAPCO-2016.md line 4420):
  > "The SBU-NF marking is conveyed in the portion mark only if the commingled portion is unclassified and there is no other NOFORN information included in the portion. If there is other NOFORN information in the commingled portion, the 'SBU' marking is used and a NOFORN marking is added, e.g., (U//NF//SBU)."
- **`reads`**: `&[CAT_CLASSIFICATION]` — narrow form. Predicate scans `CAT_DISSEM` (and the `non_ic_dissem` field) for SBU-NF; doc-comment only.
- **`writes`**: `&[CAT_DISSEM]` — Phase-3 pragmatic; non-IC dissem axis is not separately exposed (see §8 Q1 resolution).
- **Predicate**: `CategoryPredicate::Custom(never_fires)`.
- **Action**: `CategoryAction::Custom(noop_action)`.
- **Monotonicity**: **Mixed** — shrinking on class > U; mostly-additive on class = U.

### Entry 6b: `capco/les-nf-transmutes-on-classified-contact`

- **Summary**: LES-NF portion contacting any IC marking transmutes to NOFORN + LES; banner becomes `[class]//NOFORN//LES`.
- **Citation string**: `"CAPCO-2016 §H.9 p185"`
- **Cited passage** (verified against CAPCO-2016.md line 4604; §H.9 p185 notional Page 4 example):
  > Example shows banner "SECRET//NOFORN//LES" with portion marks "(S//NF)" and "(U//LES-NF)" — banner consolidates as `[class]//NOFORN//LES`.
- **`reads`**: `&[CAT_CLASSIFICATION]` — narrow form. Predicate scans `CAT_DISSEM` (and `non_ic_dissem`) for LES-NF; doc-comment only.
- **`writes`**: `&[CAT_DISSEM]`.
- **Predicate**: `CategoryPredicate::Custom(never_fires)`.
- **Action**: `CategoryAction::Custom(noop_action)`.
- **Monotonicity**: **Monotone-additive** on dissem axis.

### Entry 7: `capco/us-presence-promotes-bare-fgi-attribution`

- **Summary**: US-presence on the page promotes any `bare(_, C, _)` FGI attribution to a fully-rolled-up `⊤(C)` form (idempotent on already-promoted state).
- **Citation string**: `"CAPCO-2016 §H.7 p123"`
- **Cited passage**: Same as Entry 1 (line 3099); §H.7 p123 establishes both the trigger and the post-rollup-cleanup contracts. Two entries citing the same §-passage is admissible under D13 because each entry covers a distinct semantic surface (Entry 1 = bare-FGI portion contacts US-class trigger; Entry 7 = idempotent generalization that runs after entries 1–3 consolidate FGI state).
- **`reads`**: `&[CAT_CLASSIFICATION, CAT_FGI_MARKER]` — `CAT_FGI_MARKER` IS a real dataflow read here: entry 7 consumes the post-rewrite FGI state produced by entries 1, 2, 3 and idempotently promotes any remaining `bare(_, C, _)` to `⊤(C)`. This is the one entry whose FGI_MARKER read is structural, not predicate-scan.
- **`writes`**: `&[CAT_FGI_MARKER]` — within-axis cleanup.
- **Predicate**: `CategoryPredicate::Custom(never_fires)`.
- **Action**: `CategoryAction::Custom(noop_action)`.
- **Monotonicity**: **Monotone-additive** — `bare(_, C, _) → ⊤(C)` is a join-monotone `FgiSet` promotion. Idempotent on already-promoted state.

---

## 4. Dependency Graph

**Reads/writes semantics — narrow form (REVISED 2026-05-07).** The first-pass plan inflated `reads` to include every axis the rewrite *touches* (including predicate-scan axes). The marque scheduler at `crates/engine/src/scheduler.rs:78-95` skips only *same-rewrite* self-edges (`producer_idx == idx`); it treats every cross-rewrite same-axis read/write pair as a dataflow edge in BOTH directions, which manufactures cycles when independent rewrites (e.g., entries 1, 2, 3 — disjoint portion-level triggers, all touching `FGI_MARKER`) all declare reads of the axis they write to.

The fix: declare `reads` as **true dataflow dependencies only** — axes whose post-rewrite state this rewrite consumes from another rewrite. Predicate-scan reads (axes the trigger pattern-matches against) are documented in the doc-comment but excluded from the `reads` annotation. This is the same convention `noforn-clears-rel-to` follows (reads `[DISSEM, REL_TO]`: REL_TO is a self-edge skipped; DISSEM is a real dependency on the new entries 5/6a/6b that write DISSEM).

Apply the convention per-entry:

| Rewrite | Reads (narrow) | Writes | Predicate-scan axes (doc-comment only) |
|---|---|---|---|
| `capco/noforn-clears-rel-to` | `[DISSEM, REL_TO]` | `[REL_TO]` | (none — DISSEM is a real dep on 5/6a/6b) |
| Entry 1 `fgi-rollup-on-us-contact` | `[CLASS]` | `[FGI_MARKER]` | scans `FGI_MARKER` for bare-FGI atoms |
| Entry 2 `fgi-restricted-rollup-on-us-contact` | `[CLASS]` | `[FGI_MARKER]` | scans `FGI_MARKER` for bare-FGI-R atoms |
| Entry 3 `joint-cross-class-rollup` | `[CLASS, JOINT_CLASSIFICATION]` | `[FGI_MARKER]` | (none — JOINT scan IS the read) |
| Entry 4 `frd-sigma-consolidates-into-rd-sigma` | `[AEA]` | `[AEA]` | (self-edge — predicate scans AEA) |
| Entry 5 `orcon-nato-to-us-orcon-on-us-contact` | `[CLASS]` | `[DISSEM]` | scans `DISSEM` for ORCON-NATO |
| Entry 6a `sbu-nf-transmutes-on-classified-contact` | `[CLASS]` | `[DISSEM]` | scans `DISSEM` (and non-IC dissem) for SBU-NF |
| Entry 6b `les-nf-transmutes-on-classified-contact` | `[CLASS]` | `[DISSEM]` | scans `DISSEM` (and non-IC dissem) for LES-NF |
| Entry 7 `us-presence-promotes-bare-fgi-attribution` | `[CLASS, FGI_MARKER]` | `[FGI_MARKER]` | (FGI_MARKER read IS the real dep on 1/2/3) |

**Total**: 9 rows.

**Self-edges** (skipped per `scheduler.rs:84-87`): Entry 4 (AEA), Entry 7 (FGI_MARKER), `noforn-clears-rel-to` (REL_TO).

**Real inter-rewrite edges**:
- Entries 1, 2, 3 write `FGI_MARKER` → Entry 7 reads `FGI_MARKER`. **1, 2, 3 → 7.**
- Entries 5, 6a, 6b write `DISSEM` → `noforn-clears-rel-to` reads `DISSEM`. **5, 6a, 6b → noforn-clears-rel-to.**

**No cycles**:
- Entries 1, 2, 3 mutually: each writes `FGI_MARKER`, none reads `FGI_MARKER` (predicate-scan is doc-comment only). No mutual edges.
- Entries 5, 6a, 6b mutually: each writes `DISSEM`, none reads `DISSEM`. No mutual edges.
- CLASS is read by 1, 2, 3, 5, 6a, 6b, 7 but written by no rewrite (reciprocal-raise is parser-side per §3.4.1 Note (i), not a rewrite transform). CLASS reads are dataflow-orphan (no edges).
- JOINT_CLASSIFICATION is read by 3 only; nobody writes it (per §H.3 p57: "JOINT marking is not carried forward to the banner"). No edges.

**Topological order** (one valid ordering):
1. Entry 4 (AEA-only; independent).
2. Entries 1, 2, 3 (FGI rollups; mutually unconstrained — Kahn picks declaration order).
3. Entry 7 (FGI cleanup after 1, 2, 3).
4. Entries 5, 6a, 6b (DISSEM transmutations; mutually unconstrained).
5. `capco/noforn-clears-rel-to` (REL TO clear after DISSEM settles).

**Custom-axes invariant**: every entry has non-empty `reads` and `writes`; the scheduler's `UnannotatedCustomAxes` rejection does not fire.

**Phase D/E note**: when the runtime activates these rewrites, the predicate bodies will perform the documented predicate-scans on the post-projection page state. The scheduler's coarse "writes determines order" model is sufficient because each rewrite's effect is a *commutative* shape-modification on its write-axis (e.g., entries 1, 2, 3 each fold a disjoint portion-level pattern into FGI_MARKER; running them in any order produces the same fixpoint). If Phase D/E discovers a real ordering dependency (e.g., entry 2 needs entry 1's output), the corresponding `reads` annotation can be re-introduced and the scheduler's DAG will reflect it.

---

## 5. Net Rule Delta

| Rule | Subsumed by? | Disposition |
|---|---|---|
| `DeclarativeRdPrecedenceRule` (E024) | Partial — entry 4 covers SIGMA-extended forms only | **Keep**. E024 is portion-scope; entry 4 is page-scope. |
| `SigmaValidationRule` | No — validates SIGMA numbers + ordering | **Keep**. Orthogonal. |
| `DeclarativeAeaNofornRule` (E021) | No — constraint, not transmutation | **Keep**. |
| `DeclarativeJointRelToRule` (E014) | No — constraint, not banner construction | **Keep**. |
| `BannerMatchesProjectedRule` (3b.A walker) | No — banner-equality property test | **Keep**. |
| `JointUsaFirstRule` (S003) | No — style rule | **Keep**. |

**Verdict**: zero `Rule` impl retirements in PR 3b.B. The 3b consultation verdict's "−1 to −3" target is met by retiring the two existing `PageRewrite` stubs (`capco/joint-promotion`, `capco/fgi-absorption`).

---

## 6. Test Surface

### 6.1 New test file

`crates/capco/tests/transmutation_rewrites.rs` (~250 lines).

### 6.2 Per-entry unit tests (eight, one per entry)

For each new entry: assert `id`, `citation`, `reads`, `writes`, predicate variant matches `Custom`, action variant matches `Custom`. Behavior contract on the data, not runtime side effects.

### 6.3 Scheduler-acyclic test

`Engine::new` succeeds with the full nine-row rewrite table. Asserts no `RewriteCycle`, no `UnannotatedCustomAxes`.

### 6.4 Stub-retirement test

The two retired stubs (`capco/joint-promotion`, `capco/fgi-absorption`) MUST NOT appear in `scheme.page_rewrites()`.

### 6.5 Citation-fidelity test (existing harness)

If `crates/capco/tests/citation_fidelity.rs` exists, verify it walks `scheme.page_rewrites()` and validates `PageRewrite::citation` strings against the vendored CAPCO-2016 source. If it doesn't, extend it.

### 6.6 Regression

Full workspace test suite passes. Specifically check `crates/capco/tests/corpus_parity.rs` for any `page_rewrites().len() == N` assertion — bump from 3 to **9** if present.

### 6.7 Coverage

≥80% on lines added in `scheme.rs`. Behavior tests above cover authoring contracts.

---

## 7. Files Touched + Rough Line Counts

| File | Change | Approx. lines |
|---|---|---|
| `crates/capco/src/scheme.rs` | Add 8 `PageRewrite::custom(...)` rows (entries 1, 2, 3, 4, 5, 6a, 6b, 7) to `build_page_rewrites()`; remove `joint-promotion` and `fgi-absorption` stubs and their const slices; add `noop_action` helper alongside the existing `never_fires` (the previous `identity_promote` helper, used only by the retired stubs, is removed in this PR as dead code); update `build_page_rewrites()` doc comment to reflect the nine-row table | +~250 / −~50 (net +200) |
| `crates/capco/tests/transmutation_rewrites.rs` | New test file: 8 per-entry tests + scheduler-acyclic test + stub-retirement test + helper `lookup_rewrite()` | ~250 |
| `crates/capco/tests/corpus_parity.rs` | Possibly: bump `page_rewrites().len()` assertion (3 → 9) if such an assertion exists | ~1 |
| `crates/capco/tests/citation_fidelity.rs` | Possibly: extend to walk `scheme.page_rewrites()` if it doesn't already | ~10–20 |

**No changes** to: `marque-scheme/`, `marque-engine/`, `marque-core/`, `marque-rules/`, `marque-ism/`, `crates/capco/src/rules.rs`, `crates/capco/src/rules_declarative.rs`.

---

## 8. Open Questions — Resolved by Project Manager

(Original questions raised by the planning agent are below; PM resolutions are in the addendum at the end of this document.)

### Q1 (P0): No `CAT_NON_IC_DISSEM` CategoryId exists

Entries 6a (SBU-NF) and 6b (LES-NF) write to a non-IC dissem axis (the markings live in `CanonicalAttrs.non_ic_dissem`), but no `CAT_NON_IC_DISSEM` CategoryId is exposed. **Resolved**: use `CAT_DISSEM` as the write axis for both entries (option (a) — Phase-3 pragmatic). Document in doc-comments. Phase D/E may revisit.

### Q2 (P0): D13 single-citation rule for entry 6 (SBU-NF + LES-NF)

The consultant's §3.4.1 Entry 6 covers both with citations §H.9 p178 + §H.9 p185. **Resolved**: split into 6a + 6b, one citation each. The seven-entry roster becomes eight. Per D13, every declarative entry has exactly one §-citation.

### Q3 (P1): Scheduler self-edge handling

**Resolved**: `crates/engine/src/scheduler.rs:84-87` explicitly skips self-edges. No issue.

### Q4 (P1): Citation discrepancy for entry 5 (ORCON-NATO)

§H.8 p136 names ORCON / ORCON-USGOV precedence; ORCON-NATO is in Appendix B (line 895). **Resolved**: keep `§H.8 p136` as the primary `citation` arg; document the Appendix B mapping in the doc-comment.

### Q5 (P2): Action-shape choice for within-axis transforms

Entries 4 and 7 are within-axis. **Resolved**: use `Custom(noop_action)` (more honest than `Promote { from: X, to: X, .. }`). Phase D/E will rewrite either way.

### Q6 (P2): Monotonicity attestations

Each entry's doc-comment notes its monotonicity ahead of Phase D/E activation. No action required in PR 3b.B.

---

## 9. Acceptance Criteria

PR 3b.B is ready to merge when:

- [ ] `cargo check --workspace` passes.
- [ ] `cargo test --workspace` passes (1900+ tests; new transmutation_rewrites.rs ≥ 10 tests).
- [ ] `cargo clippy --workspace` produces no new warnings.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] Every new `PageRewrite` row's `citation` is verified against `crates/capco/docs/CAPCO-2016.md` (Constitution VIII).
- [ ] `Engine::new` succeeds with the nine-row rewrite table.
- [ ] The two retired stubs are removed from `build_page_rewrites()`.
- [ ] Doc-comments include §-citation, cited-passage paraphrase, monotonicity attestation, Phase-3 stub justification, and (entries 1, 2, 3) the reciprocal-raise note.
- [ ] No changes to `marque-scheme`, `marque-engine`, `marque-rules`, `marque-ism`, `marque-core`.
- [ ] No `Rule` impl additions or removals.

---

## 10. Out of Scope

- Runtime activation — Phase D/E.
- `marque-scheme` trait/constructor changes — separate engine PR.
- New `Constraint` variants — PR 3.7.
- Closure operator — PR 3.7.
- Retiring `capco/noforn-clears-rel-to`.
- Adding `CAT_NON_IC_DISSEM`.

---

## Project Manager Addendum (2026-05-07)

**Reads/writes narrowing (added during implementation)**: First-pass §3 entries declared inflated `reads` annotations covering predicate-scan axes alongside dataflow dependencies. The implementation agent caught this — the marque scheduler's self-edge skip (`scheduler.rs:84-87`) only covers `producer_idx == idx`, NOT cross-rewrite same-axis read/write pairs. With the original annotations, entries 1/2/3 (each reading + writing `FGI_MARKER`) and entries 5/6a/6b (each reading + writing `DISSEM`) formed mutual cycles that broke `Engine::new` for every default-engine test.

The fix: declare `reads` as **true dataflow dependencies only**. Predicate-scan axes are documented in each entry's doc-comment (with the explicit phrase "predicate scans X for Y") but excluded from the `reads` slice. §3 entries 1–7 and §4 dependency graph have been revised to reflect this.

This convention matches `noforn-clears-rel-to`'s pre-existing convention (its `[DISSEM, REL_TO]` reads always included a real dataflow dep on DISSEM — vacuous in the 3-row state, real in the 9-row state where 5/6a/6b write DISSEM). The narrowing makes the convention explicit and survives Phase D/E activation: when real predicate bodies replace `never_fires`, the predicate-scan axes are already documented and can be re-introduced as `reads` if a real dataflow dependency emerges.

**Resolution of Q1, Q2, Q4 + scheduler-self-edge confirmation**:

- **Q1** → Option (a): `CAT_DISSEM` as the write axis for SBU-NF/LES-NF. Document the imprecision in the doc-comments. Phase D/E (when CAT_NON_IC_DISSEM may be added) is the right migration point.
- **Q2** → Approved: split consultant Entry 6 into 6a (SBU-NF, §H.9 p178) and 6b (LES-NF, §H.9 p185). The eight-entry roster (1, 2, 3, 4, 5, 6a, 6b, 7) honors D13.
- **Q3** → Confirmed: `crates/engine/src/scheduler.rs:84-87` explicitly skips self-edges.
- **Q4** → Approved: keep `"CAPCO-2016 §H.8 p136"` as the primary citation; document ORCON-NATO Appendix B mapping in the doc-comment.
- **Q5** → Approved: `Custom(noop_action)` for within-axis entries (4, 7).

**Row count is 9 total** (1 retained `noforn-clears-rel-to` + 8 new). Any `page_rewrites().len()` assertion in tests should bump from 3 to 9.

**Quality bar reminder**: 5-year maintainability. Doc-comments are non-optional. Citations must be re-verifiable. The implementation agent must read the cited CAPCO-2016 lines and verify each citation before opening the PR.

End of plan.
