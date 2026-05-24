<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-B: Per-category Lattice impls (rest of the seven) + PageContext OC-USGOV bugfix + parity gate

**Status**: ready to execute.
**Branch**: `refactor-006-pr-4b-b-lattice-impls-rest` off `origin/staging`
tip `5ee497c2` (PR 4b-A: AEA control set Lattice + design doc §7.5).
**Companion**: extends `docs/plans/2026-05-01-lattice-design.md`
(§§2, 3, 7 already drafted; §3 (a) needs the OC-USGOV supersession
addendum; §3 (a) needs the RELIDO unanimity-banner-rollup addendum;
§2 needs the JOINT producer-disunity-collapse addendum). All five PM
decisions (JOINT roll-up unanimity / RELIDO unanimity / OC-USGOV
supersession / modern-default FD&R / JOINT-disunity collapse) are
baked in; this plan does NOT re-litigate them.

## 0. PM-blocking items (target: zero)

**None.** Five recurring concerns surfaced in scope and are all
PM-resolved per project memory:

1. OC-USGOV — supersession, not unanimity (`project_oc_usgov_is_supersession_not_unanimity.md`).
2. RELIDO banner roll-up — observed-RELIDO unanimity in 4b-B Layer 1;
   FD&R-inferred RELIDO defers to 4b-D (`project_relido_unanimity_banner_rollup.md`).
3. JOINT roll-up — unanimous-producer-list pass-through, disunity
   collapses to FGI [LIST] (`project_joint_banner_rollup_nuance.md`).
4. Default-modern FD&R rules — no archival-mode gate required
   (`project_marque_assumes_modern_default_fdr.md`).
5. JOINT-disunity-to-FGI — new Warn diagnostic, citation
   `§H.3 p56 + §H.7 p123` (`project_joint_banner_rollup_nuance.md`).

The plan flags two questions for the user as **OQ-1 / OQ-2** in §10;
neither blocks execution — defaults are stated and the implementing
agent proceeds under them unless overridden.

---

## 1. Scope and non-scope

### 1.1 In scope

- Six per-category Lattice impls in `crates/capco/src/lattice.rs`
  (`MarkingClassification`, `NonUsClassification`/NATO,
  `JointClassification`, `DissemSet` for US dissem + REL TO, `RelToBlock`,
  `DeclassifyOn`).
- Add a parallel `CapcoMarking::join_via_lattice` method
  (`crates/capco/src/scheme.rs`) using component-wise per-category
  joins. The production `Lattice::join` continues to delegate to
  PageContext at PR 4b-B; the hot-path flip lands in PR 4b-D once
  the corpus-parity gate is fully wired (M-4 PR 4b-B follow-up
  correction: plan originally said "replace", actual scope is "add
  parallel path + parity-gate the two").
- OC-USGOV supersession **bugfix** in `crates/ism/src/page_context.rs`
  (the `expected_dissem_us` "unanimity-drop" branch is wrong; replace
  with supersession).
- JOINT producer-unanimity-or-collapse-to-FGI logic, including the new
  Warn diagnostic (`W004` per §6.3 rule-ID note).
- Parity gate at `crates/capco/tests/page_context_lattice_parity.rs`
  covering 35 `#[test]` fixtures total — 29 byte-identity parity
  cases + 6 documented-divergence fixtures (post C-9 / G-9 follow-up;
  PR 4b-B follow-up M-4: corpus-fixture coverage is deferred to PR
  4b-D when the hot path flips). The six divergences are enumerated
  in `crates/capco/CAPCO-CONTEXT.md` §3 with §-citations.
- §-citations for every new doc-comment, diagnostic, and design-doc
  row re-verified against `crates/capco/docs/CAPCO-2016.md` at the
  point of authorship (Principle VIII, propagation re-verification).

### 1.2 Out of scope

- **Closure operator (`§3 (e)` / Trio rows)** — Layer 2 FD&R-inferred
  defaults including RELIDO-if-no-FDR. Lands in PR 4b-D per
  `project_relido_unanimity_banner_rollup.md`. 4b-B Layer 1 covers
  **observed** RELIDO unanimity only.
- **FOUO eviction matrix (§3 (b))** — cross-axis. Already lives on
  `PageContext::expected_dissem_us` step 3 (DSEN override + classified-
  context) and stays there. The DissemSet lattice does not re-encode
  it; the parity gate inherits the current behavior. A `Constraint::Custom(
  "capco/fouo-eviction", …)` row migration is PR 4b-C territory per
  the design-doc §3 (b).
- **NODIS/EXDIS clears REL TO** — already a PageRewrite + an engine-
  gap short-circuit at `expected_rel_to` (line 538 `any_noforn` and
  `needs_nf` branches). The new `RelToBlock::Lattice` preserves
  byte-identity with the existing path; the PageRewrite continues to
  fire at the `CapcoScheme::project` layer.
- **JOINT-as-its-own-banner-axis rendering** — this PR's
  `JointClassification` lattice produces the joined fact set + the
  collapse-to-FGI Warn; the **render** decision (when to emit
  `//JOINT [class] [LIST]` vs. when to flatten to FGI) is renderer-
  canonical-form territory and belongs to the future `MarkingScheme::
  render_canonical` trait surface (PR 5+ Stage 4). PR 4b-B emits
  whatever banner the existing `page_context_to_attrs` produces for
  the unanimous case; the disunity case carries the Warn diagnostic +
  the FGI-attributed fact set so the renderer has unambiguous input.
- **`marque-1.0` audit-schema cutover** — PR 3c.2 territory, unrelated.
- **§H.6 AEA "Declassify on" canned-string exception** — design-doc §1
  "deferred" row, issue #266.

---

## 2. PM decisions baked in (do not re-litigate)

| # | Decision | Source | Where it manifests |
|---|---|---|---|
| 1 | OC-USGOV is supersession (ORCON ⊐ ORCON-USGOV); one OC-USGOV portion is sufficient to roll up if no portion carries ORCON | `project_oc_usgov_is_supersession_not_unanimity.md` (§H.8 p136/p140) | Commit 2 (PageContext bugfix); DissemSet `from_dissem_set` step 2 |
| 2 | RELIDO Layer 1 = observed-unanimity (banner only if every portion carries RELIDO); FD&R-inferred RELIDO defers to 4b-D | `project_relido_unanimity_banner_rollup.md` (§H.8 pp155-156) | Commit 4 (DissemSet); design-doc §3 (a) addendum |
| 3 | JOINT rolls up if every portion is JOINT **and** every portion has the same producer list. Disunity → drop JOINT, transmute non-US producers into a union FGI [LIST], emit Warn | `project_joint_banner_rollup_nuance.md` (§H.3 p56 + §H.7 p123) | Commit 5 (JointClassification + W004) |
| 4 | Default-assume modern (post-28-Jun-2010) content; no archival-mode gate | `project_marque_assumes_modern_default_fdr.md` | Commit 4 (RELIDO unanimity wires without date gate) |
| 5 | JOINT-disunity Warn cites category IDs + producer trigraphs only (no document text per Principle V G13) | `feedback_dissem_conflicts_emit_subtractive_fix.md` + Constitution V | Commit 5 W004 diagnostic message |

---

## 3. Architectural shape and dependency graph deltas

No new crate edges. `marque-capco` already depends on `marque-ism`,
`marque-rules`, `marque-scheme`. The OC-USGOV bugfix is **inside
`marque-ism`** (Commit 2). The rest of the work lives in
`marque-capco`. Constitution VII §IV (scheme-adoption PRs MUST NOT
edit engine crates) is **inapplicable** here because PR 4b-B is part
of the 006 engine-rule-refactor by definition; PR 4a precedent
already edited `crates/scheme/src/vocabulary.rs`. See §7.B for the
explicit Constitution-VII §IV reasoning.

```text
existing edges (no changes):
  marque-scheme  ←── marque-ism  ←── marque-capco
                                       ↑
                                 (this PR's edits)
```

The new lattice impls follow the PR 4b-A `AeaSet` precedent —
freestanding lattice types in `crates/capco/src/lattice.rs`, each
implementing `marque_scheme::Lattice`, with `from_*` / `to_*`
round-trip with the corresponding `marque-ism` storage type.

### 3.1 New types and where they live

| Type | Module path | Lattice op | BoundedLattice? | §-cite |
|---|---|---|---|---|
| `ClassificationLattice` (wrapper around `MarkingClassification`) | `crates/capco/src/lattice.rs` | `OrdMax` over US chain `U < C < S < TS` after §H.7 reciprocal normalize | Yes (top = `TopSecret`, bottom = `Unclassified`) | §H.1 pp47-54 + §H.7 pp123-125 |
| `NatoClassLattice` (wrapper around `Option<NatoClassification>`) | `crates/capco/src/lattice.rs` | `OptionalSingleton<NatoClassification>` with `OrdMax` over `NU < NR < NC < NS < CTS` | Yes (top = `CTS`, bottom = `None`) | §H.2 p55 |
| `JointSet` (wrapper around `Option<JointClassification>` + a disunity flag) | `crates/capco/src/lattice.rs` | Custom: producer-list `IntersectSet` with disunity-collapse-to-FGI | No (open producer vocabulary via `CountryCode`) | §H.3 p56 + §H.3 pp55-59 worked examples |
| `DissemSet` (wrapper around `BTreeSet<DissemControl>` + observed-RELIDO unanimity flag) | `crates/capco/src/lattice.rs` | `FlatSet` over `DissemControl` with per-token supersession overlays | No (open vocabulary) | §H.8 pp131-168 + §D.2 Table 3 |
| `RelToBlock` (wrapper around `BTreeSet<CountryCode>` + NOFORN supersession sentinel) | `crates/capco/src/lattice.rs` | `IntersectSet` over `CountryCode` with NOFORN supersession | No (open `CountryCode` vocabulary) | §H.8 pp150-151 + §D.2 Table 3 rows 9-13 |
| `DeclassifyOnLattice` (wrapper around `Option<IsmDate>`) | `crates/capco/src/lattice.rs` | `MaxDate` (built-in from `marque-scheme`) | Yes (bottom = `None`, no finite top) — semilattice only | §H.6 p104 + ISOO §3.3 (date-only axis) |

`NonUsClassification` does **not** get its own dedicated lattice
type. It lives as a sub-axis of `ClassificationLattice` because
`MarkingClassification::Nato(_)` and `MarkingClassification::Fgi(_)`
**both** reciprocally normalize to the US chain at portion-parse time
(§H.7 pp123-125), and the foreign-equity preservation flows to the
**separate** `FgiSet` axis (already implemented in PR 4b-A's
predecessor `FgiSet`). Per the design-doc §2 OQ #1 resolution: "No
cross-branch join arises in the lattice." `NatoClassLattice` exists
**only** for pure-NATO documents (no US portions) — it shadows the
US chain so an all-NATO page can carry NS in the banner without
flattening to US SECRET. The cross-axis decision (which of the two
ranks the banner) is resolved at render time by checking whether any
portion is US-originated.

### 3.2 `CapcoMarking::join` rewrite

The current `Lattice for CapcoMarking::join` (line 311-329) delegates
to `PageContext::add_portion` + `page_context_to_attrs`. PR 4b-B
replaces it with **component-wise per-category joins** delegating to
the per-category lattice types:

```text
fn join(&self, other) -> CapcoMarking:
  classification = ClassificationLattice::from(self).join(...)
  nato_class     = NatoClassLattice::from(self).join(...)
  joint          = JointSet::from(self).join(...)
  sci            = SciSet::from(self).join(...)     [PR 4b-A]
  sar            = SarSet::from(self).join(...)     [PR 4b-A]
  aea            = AeaSet::from(self).join(...)     [PR 4b-A]
  fgi            = FgiSet::from(self).join(...)     [PR 4b-A]
  dissem_us      = DissemSet::from(self).join(...)
  dissem_nato    = NatoDissemSet::from(self).join(...) [trivial union — see §3.4]
  rel_to         = RelToBlock::from(self).join(...)
  declassify_on  = DeclassifyOnLattice::from(self).join(...)
  non_ic_dissem  = (left to PageContext::expected_non_ic_dissem — see §3.3)
  → assemble CanonicalAttrs from per-axis output
```

`CapcoScheme::project(Scope::Page, ...)` (`scheme.rs:3746`) keeps the
PageContext delegation **for one more PR** as a parity-gate
control surface. PR 4b-D flips `project` to use the per-category
lattice joins; PR 4b-B installs the joins and the parity gate
proves they produce byte-identity output on every corpus fixture.

### 3.3 `non_ic_dissem` deliberately not lattice-impl'd in 4b-B

`PageContext::expected_non_ic_dissem` (line 868-onward) carries
classification-gated splitting (SBU-NF → SBU + NF in classified
docs) **and** the NODIS/EXDIS → NF injection. This is two-axis cross-
talk that the §3 (b) and §3 (c) of the design doc tag as
`Constraint::Custom` material, not lattice material. PR 4b-B does
not migrate it; the parity gate's `CapcoMarking::join` includes a
PageContext-delegation arm for `non_ic_dissem` exclusively, marked
`// PR 4b-B intentional residue per design-doc §3 (b)+(c)`. PR 4b-C
collapses it.

### 3.4 `dissem_nato` is a trivial union

`expected_dissem_nato` (page_context.rs:501) is plain set-union with
no exceptions per CAPCO-2016 p41 (NATO contributes only ORCON and
REL TO; the US-context exceptions don't apply). A `NatoDissemSet`
wrapper is one screen of code: `FlatSet<DissemControl>`, no per-
token rules. Lands as a sub-commit in Commit 4.

---

## 4. Commit-by-commit plan

Eleven commits, total ~1750 LOC added net of test-fixture lines.
Sequencing rationale at §5.

### Commit 1 — Design-doc §§2/3/7 addenda + §2.1 new (JOINT collapse) + §3 (a) RELIDO/OC-USGOV addenda

- **Files**: `docs/plans/2026-05-01-lattice-design.md` (+~280 LOC).
- **Subject**: `docs(plan): PR 4b-B addenda — OC-USGOV supersession, RELIDO observed-unanimity, JOINT disunity collapse, RelToBlock semantics, DeclassifyOn semilattice`
- **Content**:
  - §2 new sub-section "Joint producer disunity → FGI collapse"
    with two new worked examples (unanimous + disunity), §-cite
    `§H.3 p56 + §H.7 p123` + §-cite for the W004 diagnostic.
  - §3 (a) addendum "OC-USGOV supersession (not unanimity)" —
    explicitly retire the design-doc-implied unanimity model;
    cite §H.8 p136/p140; pin the post-fix Layer-1 algebra in a worked
    example.
  - §3 (a) addendum "RELIDO observed-unanimity at banner roll-up";
    cite §H.8 pp155-156; explicit boundary "PR 4b-B Layer 1 covers
    observed; PR 4b-D Layer 2 adds the FD&R-inferred RELIDO from
    §B.3 Table 2."
  - New §3 (f) "DissemSet — single bag, three overlays" — observed-
    RELIDO unanimity + OC-USGOV supersession + per-§H.8 ordering;
    note that ordering is renderer concern.
  - New §3 (g) "RelToBlock — `IntersectSet` with NOFORN supersession"
    — cite §H.8 pp150-151 + §D.2 Table 3 rows 9-13.
  - New §3 (h) "DeclassifyOn — `MaxDate` semilattice (no top)" —
    cite §H.6 p104 + ISOO §3.3.
  - Update §1 (categories requiring lattice impls) table from "Phase B
    landed §§4,5,6,7 + AEA in 4b-A" to "all seven (+ DeclassifyOn) by
    end of 4b-B."
- **§-citations to verify (Principle VIII propagation)**: re-open
  `crates/capco/docs/CAPCO-2016.md` and re-check **every page-number
  citation in the addenda** before committing. §H.3 p56, §H.7 p123,
  §H.8 p136/p140, §H.8 pp155-156, §H.8 pp150-151, §D.2 Table 3,
  §H.6 p104. The 2026-05-15 propagation-trace tag goes in each new
  citation's surrounding doc-comment ("verified 2026-05-15 against
  CAPCO-2016.md") so the reviewer can re-run the check at PR-open.
- **Test fixtures**: none (doc-only commit).

### Commit 2 — PageContext OC-USGOV supersession bugfix

- **Files**:
  - `crates/ism/src/page_context.rs` (-15 / +25 LOC in `expected_dissem_us` step 2)
  - `crates/ism/src/page_context.rs` tests (+~80 LOC for regression test + the existing unanimity-drop test gets retired)
- **Subject**: `fix(ism): OC-USGOV is supersession, not unanimity (§H.8 p136/p140)`
- **Content**:
  - Replace lines 457-471 of `expected_dissem_us`:
    - **Old logic** ("if any OC-carrying portion lacks USGOV, drop USGOV"):

      ```rust
      if seen.contains(&DissemControl::OcUsgov) {
          let oc_portions: Vec<_> = self.portions.iter()
              .filter(|a| a.dissem_us.contains(&DissemControl::Oc))
              .collect();
          if !oc_portions.is_empty() {
              let all_have_usgov = oc_portions.iter()
                  .all(|a| a.dissem_us.contains(&DissemControl::OcUsgov));
              if !all_have_usgov {
                  seen.remove(&DissemControl::OcUsgov);
              }
          }
      }
      ```

    - **New logic** ("ORCON dominates ORCON-USGOV; remove USGOV if ORCON is present"):

      ```rust
      // §H.8 p136 + p140: ORCON dominates ORCON-USGOV (USGOV is the
      // narrower constituency; ORCON is the broader one). One portion
      // with ORCON wins the banner over any number of portions with
      // OC-USGOV — supersession, not unanimity. Project memory
      // `project_oc_usgov_is_supersession_not_unanimity.md`.
      if seen.contains(&DissemControl::Oc) && seen.contains(&DissemControl::OcUsgov) {
          seen.remove(&DissemControl::OcUsgov);
      }
      ```

  - Citation header: `// CAPCO-2016 §H.8 p136 (ORCON) + §H.8 p140
    (ORCON-USGOV: "ORIGINATOR CONTROLLED-USGOV"). Verified
    2026-05-15.`
  - Retire the existing `pre_fix_oc_usgov_unanimity_drop` test in
    `page_context.rs` (line ~1310 if present — search and delete
    the unanimity-drop assertion) **only** after the replacement
    `oc_usgov_supersession` test is in place and passing.
- **Test fixtures**: new test functions:
  - `oc_usgov_supersession_one_orcon_drops_usgov` (corpus-style 2-portion fixture)
  - `oc_usgov_rolls_up_when_no_orcon_in_any_portion` (3-portion fixture)
  - `oc_usgov_unanimity_no_longer_required_regression` (pre-fix expected behavior MUST fail)
- **Risk**: **Constitution VII §IV touches `marque-ism`.** Engine
  crate. Per the precedent set by PR 4a editing
  `crates/scheme/src/vocabulary.rs` (engine crate) and the framing
  in this prompt's "PM Decisions Baked In" note, this is part of the
  006 engine-rule-refactor and the gap-first rule does not apply at
  the within-006 PR granularity. See §7.B for the explicit reasoning
  and the gap-first alternative (sub-commit 2.0) that is **not
  taken**.

### Commit 3 — `ClassificationLattice` + `NatoClassLattice` + `DeclassifyOnLattice`

- **Files**:
  - `crates/capco/src/lattice.rs` (+~280 LOC: 3 new types + 3 from/to + 3 Lattice impls + doc-comments)
  - `crates/capco/src/lib.rs` (+3 `pub use` lines)
  - `crates/capco/tests/category_lattice_laws.rs` (+~150 LOC: 3 law-suites)
- **Subject**: `feat(capco): ClassificationLattice / NatoClassLattice / DeclassifyOnLattice (006 T112 PR 4b-B Commits 3)`
- **Content**:
  - `ClassificationLattice(Option<MarkingClassification>)`
    - `join`: `OrdMax` over `Option<MarkingClassification::effective_level()>`.
      `None` is bottom-identity. `Joint` and `Conflict` variants are
      treated as their `effective_level()` for the lattice op; the
      original discriminator survives in `JointSet`/`FgiSet`. Wraps
      the existing `MarkingClassification::effective_level()` so
      §H.7 reciprocal-normalize behavior is inherited (already-tested
      at the parser layer).
    - `BoundedLattice`: top = `Some(MarkingClassification::Us(Classification::TopSecret))`.
  - `NatoClassLattice(Option<NatoClassification>)`
    - `join`: `OrdMax` over `NU < NR < NC < NS < CTS` (NATO chain
      order is already on `NatoClassification` via derive).
    - `BoundedLattice`: top = `CTS`. Bottom = `None`.
  - `DeclassifyOnLattice(Option<IsmDate>)`
    - `join`: `max_by(end_cmp)` on `Option<IsmDate>`. Bottom-identity =
      `None`.
    - No `BoundedLattice` impl. The cite-frame: dates are open-vocab —
      a finite top is not realizable.
- **Test fixtures (`category_lattice_laws.rs`)**:
  - `classification_chain_assoc_comm_idem` (parametrized over the 4
    US levels + None).
  - `classification_identity_with_bottom` (None ⊔ x = x).
  - `nato_chain_assoc_comm_idem` (5 NATO levels + None).
  - `nato_identity_with_bottom` and `nato_top_absorbs`.
  - `declassify_on_max_assoc_comm_idem` (proptest over `IsmDate`
    generator already in tree).
- **§-citations verified**: §H.1 pp47-54, §H.2 p55, §H.6 p104.
- **Constitution checks**: V (no document text in doc-comments — use
  category IDs / token canonicals), VIII (every citation re-verified
  at authorship).

### Commit 4 — `DissemSet` + `NatoDissemSet`

- **Files**:
  - `crates/capco/src/lattice.rs` (+~400 LOC)
  - `crates/capco/src/lib.rs` (+2 `pub use`)
  - `crates/capco/tests/category_lattice_laws.rs` (+~180 LOC)
- **Subject**: `feat(capco): DissemSet + NatoDissemSet with observed-RELIDO unanimity (006 T112 PR 4b-B Commit 4)`
- **Content**:
  - `DissemSet` storage = `BTreeSet<DissemControl>` + `relido_observed_unanimity_flag: bool`
    + `oc_usgov_pending: bool` (deferred-drop marker — see §6.3 below).
  - `from_attrs_iter(portions: &[CanonicalAttrs]) -> Self`:
    1. Initialize `seen` from union of `attrs.dissem_us` over portions.
    2. **OC-USGOV step**: if `seen` contains both `Oc` and `OcUsgov`,
       remove `OcUsgov` (supersession, per project memory). **Mirrors
       the Commit 2 PageContext fix.** Cite `§H.8 p136 + p140`.
    3. **RELIDO observed-unanimity step**: if `seen` contains
       `Relido`, check whether **every** portion carries `Relido`. If
       yes, `relido_observed_unanimity = true` (the banner gets RELIDO).
       If no, remove `Relido` from `seen` (the banner does NOT get
       RELIDO from observation alone; Layer 2's FD&R-inferred RELIDO
       lands in 4b-D). Cite `§H.8 pp155-156`.
    4. **NOFORN dominates** sub-step: if `seen` contains `Nf`, **and**
       `seen` contains `RelTo` or `Relido` or `DisplayOnly`, remove
       the dominated tokens. (This mirrors the existing PageRewrite at
       `capco/noforn-clears-rel-to` plus its sibling rewrites.) Cite
       `§D.2 Table 3 rows 1-2 + §H.8 p145`.
    5. FOUO eviction is **NOT** done here; PageContext step 3 keeps it.
  - `Lattice::join`: BTreeSet union, then re-apply steps 2-4 (this
    matters: joining two `DissemSet`s already-stepped on different
    portion subsets requires re-running the supersession overlays
    because the overlay decisions depend on the **combined** set).
    Idempotent because re-applying steps 2-4 to already-stepped
    output is a no-op.
  - `Lattice::meet`: BTreeSet intersection (no overlays — meet over a
    bag-with-supersession is set-theoretic intersection only).
  - `to_box_slice() -> Box<[DissemControl]>`: returns the BTreeSet in
    its natural order (per-§H.8 prose ordering is the renderer's
    job, not the lattice's).
  - `NatoDissemSet`: trivial `FlatSet<DissemControl>` wrapper. No
    overlays. `BoundedLattice` is NOT implemented (NATO dissem
    vocabulary is closed at 2 elements today — `ORCON-NATO` +
    `REL TO` — but the underlying `DissemControl` enum is shared
    with US dissem so the namespace bound is loose; bottom = empty
    set, top is unsafe to claim).
- **Test fixtures**:
  - `dissem_basic_union_assoc_comm_idem` (proptest, 4 levels).
  - `dissem_oc_usgov_supersession_mirrors_pagecontext`
    (parametrized — same fixture as Commit 2's regression).
  - `dissem_relido_observed_unanimity_pass`
    (3-portion all-RELIDO → banner gets RELIDO).
  - `dissem_relido_observed_unanimity_fail`
    (3-portion 2-of-3 RELIDO → banner does NOT get RELIDO).
  - `dissem_relido_layer1_does_not_infer`
    (1-portion uncaveated post-2010 classified, no RELIDO in
    portion → no banner RELIDO. The FD&R-inferred case is 4b-D.)
  - `dissem_noforn_clears_rel_to_and_relido_and_displayonly`
    (mixed 3-portion fixture).
  - `nato_dissem_set_plain_union` (2-element vocab test).
  - Property test: `dissem_set_lattice_laws_idempotent_associative`
    (proptest with arbitrary `DissemSet` operands).
- **§-citations verified**: §H.8 p136 + p140, §H.8 pp155-156,
  §D.2 Table 3, §H.8 p145.
- **Constitution checks**: V (DissemSet doc-comments cite category
  IDs `CAT_DISSEM` + token canonicals `TOK_NOFORN` etc. — no document
  text), VIII.

### Commit 5 — `JointSet` + `W004 joint-disunity-collapse` Warn diagnostic

- **Files**:
  - `crates/capco/src/lattice.rs` (+~300 LOC)
  - `crates/capco/src/lib.rs` (+1 `pub use`)
  - `crates/capco/src/rules.rs` (+~100 LOC for the new `W004` rule)
  - `crates/capco/tests/category_lattice_laws.rs` (+~120 LOC)
  - `crates/capco/tests/joint_disunity_collapse.rs` (new file, +~180 LOC)
- **Subject**: `feat(capco): JointSet + W004 joint-disunity-collapse-to-FGI (006 T112 PR 4b-B Commit 5)`
- **Content**:
  - `JointSet` storage:

    ```text
    enum JointSet {
        Bottom,                                  // no JOINT portions seen
        UnanimousProducers {                     // every portion is JOINT
                                                 // with the same producer list
            level: Classification,               // OrdMax across portions
            producers: BTreeSet<CountryCode>,    // unanimous list (USA always in)
        },
        DisunityCollapse {                       // disunity observed
            highest_level: Classification,
            union_non_us_producers: BTreeSet<CountryCode>,  // ride to FgiSet on render
        },
    }
    ```

  - `from_attrs_iter(portions: &[CanonicalAttrs]) -> Self`:
    1. If no portion has `MarkingClassification::Joint(_)` → `Bottom`.
    2. If **every** portion is `Joint(_)`: check whether every
       portion's `countries` list is **identical** (set equality);
       if yes → `UnanimousProducers { level=OrdMax, producers=∩ }`;
       if no → `DisunityCollapse { highest_level, union_non_us_producers }`.
    3. If **some but not all** portions are `Joint(_)`: this is the
       US-document case from §H.3 p57 ("The JOINT marking
       is not carried forward to the banner line in US documents") —
       JOINT does **not** roll up. Returns `Bottom`; the JOINT non-US
       producers ride to `FgiSet` via the existing `expected_fgi()`
       path. **No W004 fires** in this case (existing US-document
       behavior preserved).
  - `Lattice::join`: composes per the JointSet variants:
    - `Bottom ⊔ x = x`.
    - `UnanimousProducers + UnanimousProducers` (same producer set,
      same level after `OrdMax`) → stays `UnanimousProducers`.
    - Any other combination → `DisunityCollapse` with the union of
      non-US producers and `OrdMax` level.
  - `to_marking_classification(&self) -> Option<MarkingClassification>`:
    - `Bottom` → None.
    - `UnanimousProducers { level, producers }` → `Some(Joint(JointClassification { level, countries }))`.
    - `DisunityCollapse { highest_level, .. }` → `Some(Us(highest_level))`.
      (Non-US producers ride to FgiSet, not back to MarkingClassification.)
  - `disunity_collapse_non_us_producers(&self) -> Option<&BTreeSet<CountryCode>>`:
    accessor used by the `W004` rule + `CapcoMarking::join` rewrite
    to attribute non-US producers to FGI on collapse.
- **`W004` rule** in `crates/capco/src/rules.rs`:
  - Severity: `Warn`. Per `feedback_dissem_conflicts_emit_subtractive_fix.md`,
    JOINT disunity is a subtractive-fix case (remove JOINT, add FGI),
    so it ships with a `FixProposal` not "just a warning."
  - Diagnostic shape (Constitution V Principle V G13-compliant):

    ```text
    rule = "W004"
    severity = Warn
    citation = "CAPCO-2016 §H.3 p56 + §H.7 p123"
    message_kind = JointDisunityCollapse
    message_template = "joint-disunity-collapse: portion(s) {portion_ids}
                        carry distinct JOINT producer lists; banner cannot
                        roll up JOINT. Migrating non-US producers to FGI
                        [LIST]."
    ```

    The placeholders are **token canonicals only**: `portion_ids` is a
    list of `Span` byte offsets (G13-permitted) and the rendered
    producer trigraphs (G13-permitted as canonical tokens). **No
    document text appears in the message.** The template is part of
    the closed `MessageTemplate` enum (per PR 3c.2's planned
    closure; for now, until 3c.2 lands, the rule emits the message
    as a `String` and re-keys onto the enum during the 3c.2
    cutover).
  - `FixProposal`:
    - Span: the JOINT portion mark (all of them, sorted lex; the
      engine applies in reverse-span order per FR-016).
    - Replacement intent: `ReplacementIntent::FactRemove` of the
      `JOINT` category from each disunity portion + `FactAdd` of
      `FGI [non-US producers]` to the FGI axis. Cross-axis, so this
      would be a `text_correction` route per the `marque-applied.md`
      §3.10 Move 7 + PR 9a T135a Commit 5 precedent (EYES → REL TO
      conversion was the first cross-axis migration on text_correction
      route).
    - Confidence: 0.85 (matches the W003 / E064 baselines; cross-
      axis migrations are inherently judgment calls).
    - Source: `FixSource::Walker { walker = "W004" }`.
    - **PR 4b-B follow-up H-1 — declined in scope.** The fix payload
      lands later. JOINT-disunity is a multi-span page-level
      transformation: removing the JOINT block from each portion
      AND emitting a new banner-shaped FGI [LIST] elsewhere is
      cross-axis AND multi-span; `text_correction` is single-span
      and `ReplacementIntent` is single-axis. The
      `MarkingScheme::render_canonical` trait surface (PR 5+ Stage 4)
      is the correct home for this transformation. W004 ships as
      Warn-only today; the audit trail surfaces the transformation
      so users can act on it.
- **Test fixtures (`joint_disunity_collapse.rs`)**:
  - `joint_unanimous_two_portions_same_producers_passes_through`
    — `(//JOINT S USA GBR) (//JOINT S USA GBR)` → banner
    `//JOINT SECRET USA, GBR` (per §H.3 worked example p1299).
  - `joint_unanimous_three_portions_different_levels`
    — `(//JOINT C USA GBR) (//JOINT TS USA GBR) (//JOINT S USA GBR)`
    → banner `//JOINT TOP SECRET USA, GBR` (OrdMax).
  - `joint_disunity_two_portions_different_producers_collapses_to_fgi`
    — `(//JOINT S USA GBR) (//JOINT S USA CAN)` → banner
    `SECRET//FGI CAN GBR` + W004 Warn diagnostic.
  - `joint_disunity_warn_diagnostic_carries_no_document_text` —
    parses the W004 diagnostic message and asserts no token from the
    input portion text (other than the producer trigraphs already in
    the canonical CountryCode vocabulary) appears in the message.
  - `joint_mixed_with_us_portions_no_w004_fires` — `(//JOINT S USA GBR) (S)`
    → existing US-document behavior (JOINT non-US producers ride to
    FgiSet via §H.3 p57). No W004.
- **§-citations verified**: §H.3 p56, §H.3 pp55-59 worked examples
  (esp. p1299), §H.3 p57, §H.7 p123. Each re-verified at
  the moment of writing the doc-comments per Principle VIII.
- **Constitution checks**: V (W004 message audit-record content
  ignorance), VIII.

### Commit 6 — `RelToBlock` (formal type)

- **Files**:
  - `crates/capco/src/lattice.rs` (+~220 LOC)
  - `crates/capco/src/lib.rs` (+1 `pub use`)
  - `crates/capco/tests/category_lattice_laws.rs` (+~90 LOC)
- **Subject**: `feat(capco): RelToBlock IntersectSet with NOFORN supersession (006 T112 PR 4b-B Commit 6)`
- **Content**:
  - `RelToBlock` storage:

    ```text
    enum RelToBlock {
        Bottom,                                  // no REL TO portions
        NofornSuperseded,                        // some portion had NOFORN
                                                 // (effectively bottom + sentinel)
        Lattice {
            countries: BTreeSet<CountryCode>,    // after IntersectSet + tetragraph
                                                 // expansion + USA-first sort
        },
    }
    ```

  - `from_attrs_iter(portions: &[CanonicalAttrs]) -> Self`:
    1. If any portion has `Nf` in `dissem_us` or NODIS/EXDIS in
       `non_ic_dissem` → `NofornSuperseded`.
    2. Else collect REL TO lists across portions; expand tetragraphs
       (`FVEY`, `ACGU`, etc.) — reuse existing `expand_tetragraph`
       from page_context.rs.
    3. Intersect the expanded sets; USA-first sort.
    4. Empty intersection → `Bottom`. (Note: §D.2 Table 3 row 9
       says no-common-LIST → NOFORN. The lattice produces `Bottom`;
       the post-projection pipeline injects NOFORN into DissemSet
       via the existing PageRewrite. **Documented in §3 (g) of the
       design doc as a deliberate split.**)
    5. Non-empty intersection → `Lattice { countries }`.
  - `Lattice::join`: composes the variants:
    - `Bottom ⊔ x = x`.
    - `NofornSuperseded ⊔ anything = NofornSuperseded`.
    - `Lattice { a } ⊔ Lattice { b } = Lattice { a ∩ b }` after
      tetragraph re-expansion. If `a ∩ b = ∅` → `Bottom`.
- **Test fixtures**:
  - `rel_to_block_intersection_common_list` (matches §H.8 p152
    worked example).
  - `rel_to_block_noforn_supersedes` (any-portion-NOFORN → all-empty).
  - `rel_to_block_empty_intersection_returns_bottom` (no common
    LIST — PageRewrite will inject NF in the next stage).
  - `rel_to_block_tetragraph_expansion_fvey_acgu` (existing fixture
    convention).
  - `rel_to_block_usa_first_ordering` (canonical render order).
  - Law tests: assoc/comm/idem on `IntersectSet<CountryCode>`.
- **§-citations verified**: §H.8 pp150-151, §H.8 p152 worked
  example, §D.2 Table 3 rows 9-13.

### Commit 7 — Replace `CapcoMarking::Lattice::join` with component-wise dispatch

- **Files**:
  - `crates/capco/src/scheme.rs` (-30 / +85 LOC; line 311-329 + surrounding "Phase A caveat" comment)
- **Subject**: `refactor(capco): CapcoMarking::join uses per-category lattice impls (006 T112 PR 4b-B Commit 7)`
- **Content**:
  - Delete the "Phase A caveat" comment block (lines 283-310) — the
    caveat is now resolved.
  - Replace `Lattice for CapcoMarking::join` body with:

    ```rust
    fn join(&self, other: &Self) -> Self {
        let portions = [&self.0, &other.0];
        let portions_slice: &[CanonicalAttrs] = &portions
            .iter()
            .map(|&p| p.clone())
            .collect::<Vec<_>>();
        let mut out = CanonicalAttrs::default();

        out.classification = ClassificationLattice::from_attrs_iter(portions_slice)
            .into_marking_classification();
        out.sci_markings  = SciSet::from_attrs_iter(portions_slice).to_markings();
        out.sci_controls  = sci_compat_view(&out.sci_markings);     // existing helper
        out.sar_markings  = SarSet::from_attrs_iter(portions_slice).to_markings();
        out.aea_markings  = AeaSet::from_attrs_iter(portions_slice).to_markings();
        out.fgi_marker    = FgiSet::from_attrs_iter(portions_slice).to_fgi_marker();
        out.dissem_us     = DissemSet::from_attrs_iter(portions_slice).into_boxed_slice();
        out.dissem_nato   = NatoDissemSet::from_attrs_iter(portions_slice).into_boxed_slice();
        out.rel_to        = RelToBlock::from_attrs_iter(portions_slice).into_boxed_slice();
        out.declassify_on = DeclassifyOnLattice::from_attrs_iter(portions_slice).into_inner();

        // The two residues — non_ic_dissem and the JOINT producer-list /
        // FGI cross-axis migration — flow through PageContext for now
        // (see §3.3 + §3 of the plan; collapsed in PR 4b-C / 4b-D).
        let mut ctx = PageContext::new();
        for p in portions_slice {
            ctx.add_portion(p.clone());
        }
        let (non_ic, _) = ctx.expected_non_ic_dissem();
        out.non_ic_dissem = non_ic.into_boxed_slice();
        // JOINT cross-axis migration on disunity: thread through here.
        // ...

        CapcoMarking::new(out)
    }
    ```

  - `meet` similarly switches to component-wise per-category meet
    delegation. The existing partial `meet` (classification + SCI +
    dissem only) becomes a full meet across all category axes.
  - Add an inline `// PR 4b-B post-condition` comment naming the
    parity-gate test that proves byte-identity with the old
    PageContext path.
- **Test fixtures**: none (Commit 8 is the gate).

### Commit 8 — Parity-gate test harness

- **Files**:
  - `crates/capco/tests/page_context_lattice_parity.rs` (new file, ~700 LOC).
- **Subject**: `test(capco): PageContext vs lattice parity gate (006 T112 PR 4b-B Commit 8)`
- **Content**:
  - Iterate every fixture in `tests/corpus/valid/*.txt` plus a
    synthetic-fixture suite (~30 cases) covering:
    - OC-USGOV: 6 cases (one-orcon-many-usgov / many-orcon-one-usgov / mix /
      pure-OC / pure-USGOV / no-OC-no-USGOV).
    - JOINT: 8 cases (unanimous-2 / unanimous-3 /
      disunity-different-producers / disunity-different-levels /
      mixed-with-US-portions / single-JOINT / FGI-only-no-JOINT /
      empty).
    - RELIDO: 6 cases (unanimous-all-portions / mixed / single-portion
      with-RELIDO / RELIDO+NF mixed / RELIDO+REL-TO / NODIS-clears).
    - DissemSet ordering: 4 cases (within-§H.8 group sorting).
    - REL TO trigraph payload: 4 cases (intersect-common /
      intersect-empty / tetragraph-FVEY / USA-first sort).
    - Classification + JOINT: 2 cases (max(JOINT-S, S) → S,
      max(JOINT-TS, S) → TS).
  - For each fixture:
    1. Parse the document via the strict recognizer (existing
       `corpus_parity.rs` shape).
    2. Run **path A** = `PageContext::add_portion` over all portions,
       then `page_context_to_attrs` (existing). Record the resulting
       banner-shape `CanonicalAttrs`.
    3. Run **path B** = `CapcoMarking::Lattice::join`-fold over all
       portions (new path). Record the resulting `CanonicalAttrs`.
    4. **Assert byte-identity** between A and B EXCEPT for:
       - OC-USGOV: A's pre-fix behavior is "drop USGOV on disunity";
         B's behavior is "drop USGOV when OC is present." The
         expected divergence is documented inline with `§H.8 p136 +
         p140` citation and the test marks it as expected.
         **Workaround**: the parity gate uses the **post-Commit-2-
         fix** PageContext as path A, so this asymmetry disappears
         after Commit 2 lands. (See sequencing § 5: Commit 2 before
         Commit 8.)
       - JOINT-disunity: B carries the W004 diagnostic + the FGI
         migration; A does not (A's PageContext flattens JOINT to
         FGI silently via `expected_fgi`). The parity gate treats
         B's behavior as the expected output and pins the divergence
         with `§H.3 p56 + §H.7 p123` citation.
       - RELIDO-non-unanimity: A retains RELIDO when any portion
         has it (current PageContext bug — RELIDO unanimity is not
         implemented in PageContext today either). B drops RELIDO
         on non-unanimity. The parity gate treats B as the expected
         output (it's the §H.8 pp155-156 spec); A is wrong.
         **OQ-1: should this PageContext bug also be fixed in Commit
         2's footprint, or kept until PR 4b-C?** Default: fix in
         Commit 2 as a second branch (RELIDO unanimity bugfix
         alongside the OC-USGOV bugfix); cite `§H.8 pp155-156`. If
         the implementing agent wants to keep Commit 2 to a single
         bugfix, the second branch defers to Commit 8.5 (sub-commit
         between 8 and 9).
  - Helper assertions:
    - `assert_byte_identity_after_oc_usgov_fix(a, b, fixture_id, citation)`
    - `assert_documented_divergence_joint_disunity(a, b, fixture_id, citation)`
    - `assert_relido_unanimity_b_wins(a, b, fixture_id, citation)`
  - Failure mode: each helper carries an inline citation comment;
    a fixture-specific failure prints the citation alongside the
    diff so a reviewer can locate the authoritative passage without
    leaving the test output.
- **Coverage target**: ≥80% on the new lattice impls (see §8 below).

### Commit 9 — Wire the `W004` rule into `CapcoRuleSet`

- **Files**:
  - `crates/capco/src/rules.rs` (+~15 LOC: registration call)
  - `crates/capco/src/rules.rs` count pin update (~3 LOC)
  - `crates/capco/tests/post_3b_registration_pin.rs` (+W004 expected entry)
- **Subject**: `feat(capco): register W004 joint-disunity-collapse (006 T112 PR 4b-B Commit 9)`
- **Content**:
  - Add `JointDisunityCollapseRule` registration to `CapcoRuleSet::new()`.
  - Bump expected rule count from 38 → 39 in
    `corpus_parity.rs` count pin + add W004 to `post_3b_registration_pin.rs`.
    (Plan written before the PR 9c.2 S007 registration shifted the
    baseline from 36 to 38; the actual delta is 38 → 39. PR 4b-B
    follow-up M-5 corrected this site.)
  - W004 severity defaults to `Warn`; configurable in `.marque.toml`
    via standard `[rules]` table.
- **§-citations verified**: §H.3 p56 + §H.7 p123 (re-checked at the
  Rule registration site).

### Commit 10 — Bench guard + bench-baseline pre-flight note

- **Files**:
  - `crates/engine/benches/lint_latency.rs` (+~10 LOC: comment block
    noting the lattice-rewrite micro-bench at the function-entry).
- **Subject**: `bench(engine): note PR 4b-B lattice rewrite on lint_latency (006 T112 PR 4b-B Commit 10)`
- **Content**:
  - `lint_10kb` is the load-bearing SC-001 gate. The lattice rewrite
    is per-document on the hot path. The bench is **not** expected
    to regress because (a) BTreeSet over `DissemControl` is
    same-order-of-magnitude as the existing `BTreeSet` in
    `PageContext::expected_dissem_us`, (b) the OC-USGOV branch
    simplifies from O(n) over `oc_portions.iter()` to O(1) set-
    containment check, (c) RelToBlock's `IntersectSet` over an
    expanded country-code set has identical cost shape to the
    existing `expected_rel_to` intersection.
  - **Bench-baseline staleness pre-flight** (per project memory
    `project_bench_baseline_staleness.md`): the `lint_10kb` baseline
    is 828µs upper-CI; current measurements land 880-930µs putting
    noise-band PRs over the line. PR 4b-B's lattice rewrite **may
    push the mean** into the gate-fail band even without semantic
    regression. **Procedure**: open PR with `gh pr create`, watch
    the first bench-check run, and if it fails on `lint_10kb` only
    (no other bench, no test failures), use `gh run rerun <id>
    --failed` once before any other action. If it persistently fails
    after one re-run, the bench-baseline-refresh PR (separate, not
    4b-B) lands first per the gap-register convention.

### Commit 11 — README + CAPCO-CONTEXT.md doc updates

- **Files**:
  - `crates/capco/CAPCO-CONTEXT.md` (+~15 LOC: §3.4 "marque gap"
    table updates).
  - `crates/capco/README.md` (+~10 LOC: lattice-types inventory).
  - `CLAUDE.md` (root) (+~12 LOC: PR 4b-B entry in "Recent Changes").
- **Subject**: `docs: PR 4b-B closure docs — CAPCO-CONTEXT §3.4 / README / CLAUDE.md (006 T112 PR 4b-B Commit 11)`
- **Content**:
  - Update `CAPCO-CONTEXT.md` §3.4 "marque gap (current, 2026-05-02)"
    table: cross out OC-USGOV unanimity-drop and replace with
    "supersession (4b-B Commit 2)"; cross out RELIDO Layer 1 and
    replace with "observed-unanimity (4b-B Commit 4)"; note JOINT
    disunity collapse (4b-B Commit 5).
  - Update `crates/capco/README.md` lattice-types inventory:
    add `ClassificationLattice / NatoClassLattice / JointSet /
    DissemSet / NatoDissemSet / RelToBlock / DeclassifyOnLattice`.
  - Add a `## PR 4b-B (006 T112, 2026-05-15)` row to `CLAUDE.md`
    Recent Changes section. Cite `§H.8 p136/p140`, `§H.8 pp155-156`,
    `§H.3 p56 + §H.7 p123`, `§H.8 pp150-151`.

---

## 5. Sequencing

```text
[1] design doc addenda            ─┐
                                   ├─ parallel
[2] PageContext OC-USGOV bugfix   ─┘
       │
       │ (Commit 2 lands first because Commit 8's parity gate
       │  depends on the post-fix PageContext as path A; otherwise
       │  the parity gate's OC-USGOV divergence becomes load-bearing
       │  and the test fails to express its real assertion.)
       ▼
[3] ClassificationLattice / NatoClassLattice / DeclassifyOnLattice
       │
       ├─ parallel ─ [4] DissemSet / NatoDissemSet
       │
       └─ parallel ─ [5] JointSet / W004
       │
       │              [6] RelToBlock
       │
       ▼
[7] CapcoMarking::Lattice::join rewrite
       │
       ▼
[8] Parity-gate test harness
       │
       ▼
[9] Register W004 + count pin bump
       │
       ▼
[10] Bench note
       │
       ▼
[11] Docs closure
```

Commits 1 and 2 are independent (1 is doc-only; 2 is the
`marque-ism` bugfix). They land **first** because:
- Commit 1 sets the §-citation tree the per-category lattice
  doc-comments will propagate from (Principle VIII propagation
  re-verification).
- Commit 2 makes PageContext consistent with the new lattice
  semantics so the parity gate has a clean "path A."

Commits 3-6 can theoretically interleave but I recommend the
**serial order shown** because each adds a `pub use` line to
`crates/capco/src/lib.rs`; serial avoids merge-conflict friction in
the implementing agent's flow.

Commit 7 (CapcoMarking::join rewrite) depends on **all of 3-6**.

Commit 8 (parity gate) depends on Commit 7.

Commits 9-11 depend on Commit 8 (the gate must pass before we
register a new rule or update bench/docs).

---

## 6. Identifier / naming reservations

### 6.1 Rule ID
- **`W004`** for the joint-disunity-collapse rule. Last `W###`
  allocated in tree is `W003` (per `CapcoRuleSet::new()`); see
  the registration site. **OQ-2: confirm the next available `W###`
  is `W004` by running `grep -rn 'W00[0-9]\b' crates/capco/src/`
  before the Commit 9 PR.** If conflict found, bump to `W005`.

### 6.2 Lattice type names
- `ClassificationLattice`, `NatoClassLattice`, `JointSet`, `DissemSet`,
  `NatoDissemSet`, `RelToBlock`, `DeclassifyOnLattice`. The
  `*Lattice` suffix mirrors the existing `MaxDate`, `OrdMax`, etc.;
  the `*Set` suffix mirrors `SciSet`/`SarSet`/`FgiSet`/`AeaSet`. The
  `*Block` suffix on `RelToBlock` reflects the fact that REL TO is a
  payload-bearing single category (mirrors `RdBlock`/`FrdBlock`/
  `AtomalBlock` from `marque-ism`).

### 6.3 W004 diagnostic message-kind enum slot
- `MessageKind::JointDisunityCollapse` — closed-enum slot on the
  closed `MessageKind` enum. PR 3c.2 will lift this into the closed
  `MessageTemplate` JSON serialization; until then, the diagnostic
  emits as `String`-form with a stable prefix for regex matching by
  the parity-gate harness.

---

## 7. Constitution checks

### 7.A Principle V Audit-First Compliance (G13)

The W004 diagnostic message **MUST NOT** contain document text.
Permitted identifiers in the W004 message: `Span` byte-offset lists,
`CountryCode` canonical trigraphs, category IDs (`CAT_JOINT`,
`CAT_FGI`), token canonicals. The `joint_disunity_warn_diagnostic_carries_no_document_text`
test in Commit 5 asserts this by parsing the message and forbidding
any byte sequence from the test fixture's portion text (other than
the producer trigraphs themselves, which are in the canonical
`CountryCode` vocabulary). The same constraint applies to the
`FixProposal.replacement` field — replacement bytes must be either
canonical tokens (`FGI`, `REL TO`, `USA`) or canonical trigraphs,
never input-document bytes.

The W004 `FixProposal` carries a `text_correction`-route migration
(cross-axis), so the standard `ReplacementIntent::FactRemove` audit
shape applies: `audit.proposal.original = ""` (per the #259
precedent — empty for cross-axis text_correction route), and
`audit.proposal.replacement` is the canonical-form bytes the
renderer produces for the FGI-migrated banner.

### 7.B Principle VII Crate Discipline (engine-crate touches)

PR 4b-B touches `crates/ism/src/page_context.rs` (engine crate
`marque-ism`). Per Constitution VII §IV final paragraph: *"A
scheme-adoption PR MUST NOT edit the engine crates."*

**Reasoning for not invoking the gap-first rule**:

1. The 006 engine-rule-refactor is, by definition, a refactor of
   the engine + rule architecture together. The constitution VII §IV
   scheme-adoption restriction targets scheme-adoption PRs (e.g.,
   "add CUI scheme on top of the unchanged engine") — not within-006
   PRs.
2. PR 4a (commit `fc91852e` — `Vocabulary<S>::is_fdr_dissem trait
   method`) edited `crates/scheme/src/vocabulary.rs` (engine crate
   `marque-scheme`) **directly** without invoking gap-first. That is
   the in-tree precedent within the 006 series.
3. The OC-USGOV bugfix is a **bug** — current PageContext output is
   wrong per §H.8 p136/p140 (project memory
   `project_oc_usgov_is_supersession_not_unanimity.md`). Bugfixes in
   the engine crates land where they live; the gap-first rule is for
   feature reveals, not for correctness regressions.

**Gap-first alternative (NOT taken)**: a sub-commit `2.0` =
PageContext bugfix-only PR in `marque-ism` against the corpus
regression harness, then `2.0.1 = the rest`. Not chosen because (a)
the bugfix is conceptually inseparable from the supersession
semantic that DissemSet encodes in Commit 4; splitting would force
two corpus-regression cycles for a single semantic correction, (b)
the existing PR 4a precedent within 006 does it the other way.

If the user wants gap-first applied: split the plan into 4b-B.0
(Commits 1+2 alone) and 4b-B (Commits 3-11). The internal sequencing
inside 4b-B does not change.

### 7.C Principle VIII Authoritative Source Fidelity

Every new doc-comment, diagnostic message, and design-doc row in PR
4b-B carries a `§X.Y pNN` citation re-verified at authorship.
**Verification procedure** the implementing agent MUST follow:

1. Open `crates/capco/docs/CAPCO-2016.md`.
2. For each citation in the doc-comment / diagnostic / design-doc
   row being written: search the markdown for the page number, read
   the surrounding text, confirm the cited claim is present and
   accurate.
3. Add the propagation-trace tag `// verified 2026-05-15 against
   CAPCO-2016.md` to the source-code citation comment. The plan-side
   citations carry the same tag in the design-doc commit (Commit 1).
4. If a citation cannot be verified, **remove it**, do not leave it
   in place. The Constitution VIII clause is unambiguous: "A citation
   that cannot be traced to a real passage MUST be removed, not left
   in place pending follow-up."

The reviewer chain (rust-reviewer + code-reviewer +
capco-classification-validator + capco-dissem-validator — see §8
below) will check at least 10% of citations against the source as a
sampling-validation pass. If any fabricated citation is found, the
PR closes with a request to re-verify ALL citations in the PR (per
the propagation-discipline lapse principle).

### 7.D Principle I Performance

`lint_10kb` SC-001 gate at p95 ≤ 16 ms governs. The lattice
rewrite is per-document on the hot path. **Expected delta**:
neutral or marginally improved.

- OC-USGOV branch simplifies from O(n) `oc_portions.iter()` to
  O(1) set containment.
- BTreeSet over `DissemControl` is ~same cost as the existing
  `BTreeSet` in `PageContext::expected_dissem_us`.
- `IntersectSet`-based REL TO has identical asymptotic cost to
  the existing intersection over expanded sets.
- The new `Lattice::join` adds 7 component-wise sub-joins; each is
  O(per-axis-vocabulary). Total fixed-overhead is a small constant
  per portion (~7 × constant).

**Bench-staleness pre-flight**: project memory
`project_bench_baseline_staleness.md` says the baseline is
near-noise-margin. Commit 10's bench-note + Commit 12 (post-PR
bench-baseline-refresh if needed) is the standard procedure.

### 7.E Principle IV Two-Layer Rule Architecture

W004 is a Layer 2 hand-written rule (`crates/capco/src/rules.rs`)
consuming Layer 1 predicates (`JointSet` is a `marque-capco`
in-tree type, but the JOINT producer-list data flows from the
generated `marque-ism::JointClassification` Layer 1 storage). The
two-layer split holds.

---

## 8. Test scaffolding

### 8.1 New test files
- `crates/capco/tests/page_context_lattice_parity.rs` — Commit 8.
- `crates/capco/tests/joint_disunity_collapse.rs` — Commit 5.

### 8.2 Extended test files
- `crates/capco/tests/category_lattice_laws.rs` — +3 law-suites
  (Classification / NATO class / DeclassifyOn — Commit 3), +1
  law-suite (DissemSet — Commit 4), +1 law-suite (JointSet — Commit
  5), +1 law-suite (RelToBlock — Commit 6).
- `crates/capco/tests/cross_axis_dominance.rs` — +2 fixtures
  (joint-disunity-collapse + OC-USGOV-supersession-cross-with-ORCON-USGOV).
- `crates/capco/tests/post_3b_registration_pin.rs` — +W004
  expected-entry (Commit 9).
- `crates/ism/src/page_context.rs` `#[cfg(test)]` module — +3
  regression tests for OC-USGOV supersession (Commit 2).

### 8.3 Corpus fixtures
- `tests/corpus/valid/` is already at 74 fixtures; the parity gate
  (Commit 8) iterates **all** of them. The synthetic-fixture suite
  in Commit 8 is inline in the test file (Rust string-literal
  multi-portion fixtures), not new corpus files. The reason: corpus
  fixtures are also exercised by the strict-recognizer and the
  decoder; the parity gate's synthetic cases test the specific
  lattice-vs-PageContext divergences which are tangential to the
  recognizer harness.

### 8.4 Coverage target
- ≥80% line coverage on `crates/capco/src/lattice.rs` (new lattice
  impls); ≥90% on the `from_attrs_iter` + `to_*` round-trip
  functions (these are the parity-gate's correctness gate, so they
  need stronger coverage). `cargo llvm-cov --fail-under-lines 80
  --crate marque-capco` runs in CI; the implementing agent runs
  locally before PR-open.

### 8.5 Mandatory reviewer chain (BEFORE PR-open)
Per the project memory `feedback_run_reviewer_before_pr_open.md`,
the reviewer agents run on the implementing branch BEFORE
`gh pr create`:

1. **rust-reviewer** — borrow-checker, idiom, error-handling,
   `Send + Sync` for the new lattice types.
2. **code-reviewer** — general quality, file-org, naming.
3. **capco-classification-validator** — Commits 3 + 5 + 7 (anything
   touching `MarkingClassification` / `JointClassification`).
4. **capco-dissem-validator** — Commits 4 + 5 + 6 + 7 (anything
   touching `DissemSet` / `RelToBlock` / `DissemControl`).

The chain MUST produce a clean attestation for every commit before
PR-open. Post-PR, Copilot review fires automatically; the chain
above is the load-bearing quality gate (per the same project
memory).

---

## 9. Risk register

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | **Constitution VII §IV reading**: PR 4b-B edits `marque-ism` (engine crate). Reviewer flags "scheme-adoption PR must not edit engine crates." | Low | §7.B carries the explicit Constitution-VII §IV reasoning + the PR 4a precedent reference. If the user wants gap-first applied, the 4b-B.0 / 4b-B split is one mechanical move (Commits 1+2 alone vs Commits 3-11). |
| 2 | **Parity-gate divergence ambiguity**: the OC-USGOV bugfix changes one side of the gate. If Commit 2 lands AFTER Commit 8 in the merge stream (e.g., due to a rebase reordering), the gate fails on every OC-USGOV fixture. | Med | Sequencing in §5 explicitly puts Commit 2 BEFORE Commit 8. The implementing agent must respect this order. The parity-gate harness checks the PageContext output post-fix (Commit 2's expected behavior); a pre-fix PageContext fails the gate. |
| 3 | **JOINT-disunity-to-FGI citation discipline**: §H.3 p56 + §H.7 p123 are the cited authorities. §H.3 p56 covers JOINT roll-up; §H.7 p123 covers FGI source-acknowledged form. The cross-axis migration the W004 fix performs is implied by combining the two passages, not stated in one place. | Med | Cite **both** in the W004 diagnostic and the design-doc §2 addendum. Pin the citation against the worked examples on §H.3 p1299 + §H.3 pp1303-1305 + §H.3 p1312 + §H.3 p1314 which jointly establish that disunity → FGI is the de-facto canonical form. The capco-classification-validator agent's review pass is the final check. |
| 4 | **RELIDO unanimity Layer 1 vs Layer 2 split**: 4b-B implements observed-unanimity. 4b-D adds FD&R-inferred RELIDO from §B.3 Table 2. If the user expects 4b-B to ship the closure-style FD&R-inferred RELIDO ("classified uncaveated post-2010 → suggest RELIDO"), it will appear absent. | Low | Explicit §3 (a) addendum in the design-doc (Commit 1) names the boundary. The Commit 4 `dissem_relido_layer1_does_not_infer` test pins the boundary at code level. Project memory `project_relido_unanimity_banner_rollup.md` confirms this split. |
| 5 | **Bench regression on lint_10kb**: the lattice rewrite is on the hot path. Bench baseline is near-noise-margin (828µs upper-CI, 911µs threshold; current measurements 880-930µs). | Med | Commit 10 carries the pre-flight bench-staleness note. The gate may fail on the first run; one `gh run rerun <id> --failed` is the standard mitigation per project memory `project_bench_baseline_staleness.md`. If it persistently fails after one re-run, the bench-baseline-refresh PR (separate, not 4b-B) lands first. |
| 6 | **JOINT-disunity Warn fires on US-document JOINT portions** (current PageContext behavior absorbs JOINT into FGI silently for these cases per §H.3 p57). If JointSet `from_attrs_iter` doesn't correctly distinguish "all-JOINT page" from "JOINT-among-US-portions page," W004 fires spuriously. | High | The `JointSet::from_attrs_iter` step 3 explicitly returns `Bottom` (no W004) for the mixed-with-US case. The Commit 5 test `joint_mixed_with_us_portions_no_w004_fires` is the regression gate. The capco-classification-validator agent's pass MUST exercise this case. |
| 7 | **DissemSet's "re-apply overlays" approach to join idempotency** is non-obvious. A future contributor reading `Lattice::join` may break it by removing the re-apply step. | Low | Commit 4's `dissem_set_lattice_laws_idempotent_associative` proptest catches the regression at compile-time-ish (proptest with arbitrary operands). Doc-comment names the re-apply step explicitly. |
| 8 | **DeclassifyOn `MaxDate` semilattice but no BoundedLattice impl** — some downstream code may expect a top. Currently AeaSet, SciSet, SarSet, FgiSet have set a precedent for "no BoundedLattice when vocab is open"; DeclassifyOn extends to "no BoundedLattice when range is open." | Low | Doc-comment + design-doc §3 (h) name this explicitly. The pattern is established. |

---

## 10. Open questions

**OQ-1**: Should the RELIDO PageContext bugfix (drop RELIDO when not
unanimous) land in Commit 2 alongside the OC-USGOV fix, or in a
separate Commit 8.5 between the lattice work (Commits 3-7) and the
parity gate (Commit 8)?

- **Default**: include in Commit 2 (2 branches: OC-USGOV +
  RELIDO-unanimity). This minimizes the parity-gate divergence
  surface and treats the bugfix-symmetry as one logical unit.
- **Alternative**: keep Commit 2 tight to OC-USGOV; RELIDO-unanimity
  lands in Commit 8.5 (between the lattice rewrite Commit 7 and the
  parity gate Commit 8).

The implementing agent proceeds with the **default** unless the user
overrides at plan-review.

**OQ-2**: Confirm the next available `W###` is `W004` (not `W005`+).

- **Default**: `grep -rn 'W00[0-9]\b' crates/capco/src/ crates/rules/src/ crates/scheme/src/`
  at the start of Commit 9. If no conflict, use `W004`. If conflict,
  bump to the lowest free `W###` and update §6.1, §6.3, and the
  Commit 5+9+11 doc comments + citations consistently.
- **Risk if missed**: registration-pin test
  (`post_3b_registration_pin.rs`) fails the regression gate. Not
  load-bearing for correctness — caught at CI test-run.

The implementing agent runs the grep at Commit 9-start and
self-resolves.

---

## 11. Acceptance checklist (for reviewer chain)

- [ ] Commit 1 design-doc §§2.1 / 3 (a) addenda / 3 (f-h) sections
      cite §H.8 p136/p140, §H.8 pp155-156, §H.3 p56, §H.7 p123,
      §H.8 pp150-151, §D.2 Table 3, §H.6 p104; every citation
      re-verified at authorship per Principle VIII.
- [ ] Commit 2 retires the OC-USGOV unanimity-drop test and replaces
      it with the supersession test; the regression test
      `oc_usgov_unanimity_no_longer_required_regression` fails on
      the pre-fix branch and passes on the post-fix branch.
- [ ] Commits 3-6 each add `Lattice` law tests (assoc / comm /
      idem) via proptest; `BoundedLattice` impls have `top ⊔ x = top`
      and `top ⊓ x = x` law tests where applicable.
- [ ] Commit 7's `CapcoMarking::Lattice::join` rewrite preserves
      byte-identity with the post-fix PageContext path on every
      `tests/corpus/valid/*.txt` fixture (Commit 8 is the gate).
- [ ] Commit 5's W004 diagnostic carries no document text
      (`joint_disunity_warn_diagnostic_carries_no_document_text`
      test). Audit-record content-ignorance per Constitution V G13.
- [ ] Commit 8 parity gate covers 35 `#[test]` fixtures total — 29
      byte-identity parity cases + 6 documented-divergence fixtures
      (PR 4b-B follow-up M-4 + G-1..G-9); each divergence carries an
      inline `§X.Y pNN` citation. The six divergences are enumerated
      in `crates/capco/CAPCO-CONTEXT.md` §3. Corpus-fixture coverage
      is deferred to PR 4b-D when the project() hot path flips.
- [ ] Commit 9 bumps the rule count pin (38 → 39 — corrected M-5 in
      PR 4b-B follow-up) and adds W004 to the exact-set pin.
- [ ] Reviewer chain (rust-reviewer + code-reviewer +
      capco-classification-validator + capco-dissem-validator) ran
      BEFORE `gh pr create`; attestations recorded in PR description.
- [ ] `cargo llvm-cov --crate marque-capco --fail-under-lines 80`
      passes locally before PR-open.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] `cargo +stable clippy --workspace -- -D warnings` clean
      (per project memory `feedback_clippy_nightly_vs_stable_drift.md`).
- [ ] `cargo fmt --check` clean.
- [ ] CI bench `lint_10kb` passes (or, if it noise-flakes once,
      a documented `gh run rerun --failed` brings it green; if
      persistent failure, baseline-refresh PR lands first per
      Risk #5).
- [ ] PR description quotes Principle V (G13), Principle VII (engine-
      crate-touch reasoning), Principle VIII (citation-verification
      procedure), and lists every cited §X.Y pNN with the propagation
      tag.

---

## 12. Post-PR

Once 4b-B merges:

- **4b-C** absorbs FOUO eviction matrix (§3 (b)) + non_ic_dissem
  cross-axis cleanup into the lattice-driven `project()` path.
- **4b-D** adds the closure operator (`§3 (e)`) — the implicit-
  default trio including FD&R-inferred RELIDO. Closes the boundary
  named in §3 (a) addendum.
- **PR 5+ Stage 4** absorbs the renderer (`MarkingScheme::render_canonical`),
  which retires the W004 cross-axis migration's `text_correction`
  route in favor of the renderer driving the FGI-attribution.

The end-state target — ~10 surviving rules across all stages —
remains binding; 4b-B's W004 addition is offset by the renderer-
absorbing retirements scheduled in PR 5+.

---

## Appendix A — File-by-file delta summary

| File | Δ LOC | Commits |
|---|---|---|
| `docs/plans/2026-05-01-lattice-design.md` | +~280 | 1 |
| `docs/plans/2026-05-15-pr4b-B-lattice-impls-rest-plan.md` (this file) | +~770 (new) | 0 |
| `crates/ism/src/page_context.rs` | +~25 / -15 / +~80 (tests) | 2 |
| `crates/capco/src/lattice.rs` | +~1200 | 3, 4, 5, 6 |
| `crates/capco/src/lib.rs` | +~7 | 3, 4, 5, 6 |
| `crates/capco/src/scheme.rs` | +~85 / -30 | 7 |
| `crates/capco/src/rules.rs` | +~115 | 5, 9 |
| `crates/capco/tests/category_lattice_laws.rs` | +~540 | 3, 4, 5, 6 |
| `crates/capco/tests/cross_axis_dominance.rs` | +~80 | 5 |
| `crates/capco/tests/joint_disunity_collapse.rs` | +~180 (new) | 5 |
| `crates/capco/tests/page_context_lattice_parity.rs` | +~700 (new) | 8 |
| `crates/capco/tests/post_3b_registration_pin.rs` | +~5 | 9 |
| `crates/capco/CAPCO-CONTEXT.md` | +~15 | 11 |
| `crates/capco/README.md` | +~10 | 11 |
| `CLAUDE.md` | +~12 | 11 |
| `crates/engine/benches/lint_latency.rs` | +~10 | 10 |

**Total net additions**: ~4100 LOC (of which ~1800 LOC is tests/
fixtures and ~770 LOC is plans/docs).

---

## Appendix B — §-citation index

Citations introduced or propagated by PR 4b-B (each verified
2026-05-15 against `crates/capco/docs/CAPCO-2016.md` at the moment
of writing this plan):

| Citation | Used by | Authority |
|---|---|---|
| `§A.4 p13` | Commit 3 doc-comment | classification hierarchy |
| `§D.2 Table 3 pp28-30` | Commits 4, 6 | FD&R precedence |
| `§D.2 Table 3 rows 1-2` | Commit 4 (NOFORN dominates) | NOFORN supersession |
| `§D.2 Table 3 rows 9-13` | Commit 6 (RelToBlock) | REL TO supersession |
| `§H.1 pp47-54` | Commit 3 (ClassificationLattice) | US class chain |
| `§H.2 p55` | Commit 3 (NatoClassLattice) | NATO class chain |
| `§H.3 p56` | Commits 1, 5 (JointSet, W004) | JOINT roll-up |
| `§H.3 p57` | Commit 5 (mixed-with-US-portions) | "JOINT not carried forward in US documents" |
| `§H.3 pp1299, 1303-1305, 1312, 1314` | Commit 5 (test fixtures) | JOINT worked examples |
| `§H.6 p104` | Commits 3, 1 (DeclassifyOn) | RD primary + AEA precedence |
| `§H.6 pp103-121` | Commit 3 (AEA — PR 4b-A precedent reuse) | AEA section |
| `§H.7 p123` | Commits 1, 5 (FGI source-acknowledged) | FGI form |
| `§H.7 pp123-125` | Commit 3 (Classification reciprocal normalize) | §H.7 reciprocal rule |
| `§H.8 p136` | Commits 1, 2, 4 (OC-USGOV supersession) | ORCON |
| `§H.8 p140` | Commits 1, 2, 4 (OC-USGOV supersession) | ORCON-USGOV |
| `§H.8 p145` | Commit 4 (NOFORN dominates) | NOFORN |
| `§H.8 pp150-151` | Commit 6 (RelToBlock) | REL TO |
| `§H.8 p152 worked example` | Commit 6 test fixture | REL TO intersection example |
| `§H.8 pp155-156` | Commits 1, 4 (RELIDO unanimity) | RELIDO |
| `ISOO §3.3` | Commit 3 (DeclassifyOn) | date-only axis (out-of-tree citation; included only for cross-reference, not as primary source) |

Each citation MUST be re-verified at the moment of authorship per
Constitution VIII propagation-discipline. The propagation-trace tag
in source code is `// verified 2026-05-15 against CAPCO-2016.md`.
