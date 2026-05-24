<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Plan-of-Record: PR 4b Umbrella Closeout — T112-closeout / pin / CI

**Branch**: `refactor-006-pr-4b-closeout`
**Worktree**: `/home/knitli/marque/.claude/worktrees/pr-4b-closeout/` (off `origin/staging` at `5d3415cd`)
**Base PR**: against `staging` (NOT `main`)
**Predecessors landed in `staging`** (nine sub-PRs, not six — the 4b-D series sub-split into four):

| Sub-PR | # | Title | Merged |
|---|---|---|---|
| 4b-A | #426 | AEA control set Lattice + design doc §7.5 (006 T112 partial) | 2026-05-15 |
| 4b-B | #437 | per-category Lattice impls + JOINT W004 + OC-USGOV+RELIDO PageContext bugfix (006 T112 close-out) | 2026-05-16 |
| 4b-C | #468 | Pattern B + Pattern C declarative PageRewrite migration + UCNI NOFORN-promotion bugfix (006 T112) | 2026-05-16 |
| 4b-D.0 | #514 | ClosureRule generic + cone_derived (engine-gap before 4b-D) | 2026-05-17 |
| 4b-D.1 | #517 | closure operator runtime activation | 2026-05-17 |
| 4b-D.2 | #527 | hot-path flip (Engine::project + JoinSemilattice::join) | 2026-05-18 |
| 4b-D.3 | #535 | consumer migration (S007 to ProjectedMarking) | 2026-05-18 |
| 4b-E | #539 | PageContext expected_*/renderer deletion + 5 new lattice helpers | 2026-05-18 |
| 4b-F | #542 | retire `&PageContext` residue parameters + close PR-4 tasks bookkeeping | 2026-05-18 |

**Status**: ARCHITECT-DRAFT 2026-05-19, awaiting PM resolution of §6 OQs.

---

## 0. One-paragraph summary

The PR-4b umbrella collapses the broken `PageContext`-based banner-rollup into a per-category `Lattice` foundation across **nine** functional sub-PRs (4b-A through 4b-F, with 4b-D itself sub-split into 4b-D.{0,1,2,3}). The umbrella delivered 13 per-category lattice impls in `marque-capco::lattice` (9 with `JoinSemilattice + MeetSemilattice`, 4 with `JoinSemilattice`-only per PR #456 trait split), plus 2 aggregator helpers (`NonIcDissemSet`, `DeclassExemptionAccumulator`), 27 declarative `PageRewrite` rows in 6 groups, 10 `ClosureRule` rows on `CapcoScheme`, one new Warn rule (`W004` from 4b-B; registered rule count moved 38 → 39), and finally flipped the production hot path from `PageContext::expected_*` accessors to `MarkingScheme::project(Scope::Page, ...)` (4b-D.2), deleting ~3457 lines of `PageContext` renderer/accessor surface (4b-E) and the last `&PageContext` residue parameters from the lattice-fold body chain (4b-F). The closeout sub-PR delivers bookkeeping only: **(T112-closeout-a)** the umbrella reviewer attestation aggregating per-sub-PR §-citation discipline + engine-crate touch ledger + net-lattice-delta math; **(T112-closeout-b)** an exact-state pin at `crates/capco/tests/post_4b_lattice_inventory_pin.rs` complementing the existing count-pin shape; **(T112-closeout-c)** a `pr-4b-corpus-regression` CI job branch-filtered to `refactor-006-pr-4b*` mirroring the T029 / PR-3b precedent. Zero rule-logic edits, zero engine-crate edits beyond documented within-006 precedent (Constitution VII §IV), zero plan-doc rewrites beyond marking new task IDs DONE.

---

## 1. Sub-PR scope

### 1.1 In scope

1. **T112-closeout-a — Umbrella attestation work.** A PR-description block aggregating the three attestation buckets each 4b sub-PR already declared individually:
   - **(a) single CAPCO-§ citation discipline** per lattice impl, per `PageRewrite` row, per `ClosureRule` row (D13 discipline; composite-citation rows are D13-compliant when the §-pair names one operative rule across multiple §H templates);
   - **(b) engine-crate touch ledger** documenting the FIVE within-006 precedent breaches of Constitution VII §IV (PR 4b-B Commit 2 / 4b-C Commit 5 / 4b-D.2 / 4b-D.3 / 4b-E `assert_impl_all!(PageContext: Send, Sync)`), each with sub-PR + scope + justification;
   - **(c) net-lattice-delta math** as a running counter from pre-4b baseline through each sub-PR's contribution to the post-4b-F state (per-axis: `JoinSemilattice` impls, `MeetSemilattice` impls, `Bounded*` impls, aggregator helpers, `PageRewrite` rows, `ClosureRule` rows, registered `Rule` count).

2. **T112-closeout-b — Exact-state pin.** One new integration test at `crates/capco/tests/post_4b_lattice_inventory_pin.rs` that pins the **exact set** of post-4b lattice claims:
   - exact set of `PageRewrite::name` values returned by `CapcoScheme::page_rewrites()` (27 names);
   - exact set of `ClosureRule::name` values in `CAPCO_CLOSURE_RULES` exposed via `CapcoScheme::closure_rules()` (10 names);
   - `static_assertions::assert_impl_all!` block over the 13 claimed lattice types (verifies both halves where claimed by the umbrella, only join-half where 4b documented join-only per PR #456).
   This complements the existing count-shaped pin pattern at `corpus_parity.rs` (which catches "a row was added/removed") with the orthogonal drift class "a row got renamed at the same count" / "a row got swapped for an unrelated row at the same count" — exactly the post-4b structural commitment.

3. **T112-closeout-c — CI prefix-match job.** A new `pr-4b-corpus-regression` job in `.github/workflows/ci.yml`, slotted between `pr-3b-corpus-regression` (line 161+) and `masking-pin-lint` (line 235+), mirroring T029's body verbatim with a branch filter `refactor-006-pr-4b*` covering the umbrella (if it exists), all nine sub-PR branches, and the closeout branch.

4. **Spec-doc bookkeeping.**
   - `specs/006-engine-rule-refactor/tasks.md`: ADD three new closeout task IDs (T112-closeout-a / -b / -c, or equivalent — see OQ-6) and check them off in the closeout commit; T112/T113/T114/T115 already `[X]` (closed in their respective sub-PRs).
   - `specs/006-engine-rule-refactor/plan.md`: annotation on the PR 4 row noting "**PR 4b umbrella — LANDED 2026-05-{15→18}** (sub-PRs #426 / #437 / #468 / #514 / #517 / #527 / #535 / #539 / #542; closeout #NNN)" mirroring the 3b umbrella annotation shape.
   - `CLAUDE.md` "Recent Changes": prepend a closeout entry summarizing the three deliverables; "Current Status" cosmetic update to reflect post-umbrella state.

### 1.2 Explicitly out of scope

- **Any rule-logic change.** Citations, severities, fix shapes, span behavior — all nine sub-PRs already attested those individually. The closeout PR is bookkeeping; if the closeout reviewer surfaces a real logic defect during attestation, it returns to a separate sub-PR (don't backsmuggle fixes into closeout).

- **Engine-crate edits.** Constitution VII §IV: scheme-adoption PRs MUST NOT edit `marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`, `marque-ism`. The closeout PR is bookkeeping atop a scheme-adoption umbrella and MUST observe the same discipline. No edits to those five crates. **If a closeout-side gap requires touching one of those crates**, it is filed as a follow-up PR — never absorbed into closeout (per OQ-4 default).

- **Deprecation-phasing for retired surfaces.** Per memory `feedback_pre_users_no_deprecation_phasing.md`, marque is pre-users; rewrite freely. The retired `PageContext::expected_*` accessor surface, the dropped `impl JoinSemilattice for CapcoMarking`, the deleted `join_via_lattice_with_context` chain — none of these need alias preservation, schema-bump back-compat, or deprecation comments.

- **Plan.md PR 4 row rewrites beyond the umbrella-LANDED annotation.** Don't extend the §"PR 4" prose. The umbrella landing annotation is one line per the existing PR 3b precedent.

- **T116-T119 follow-on work** (property tests at `category_lattice_laws.rs`, cross-axis dominance fixtures at `cross_axis_dominance.rs`, `tests/corpus/lattice/`, `tests/corpus/prose-positive/`). These are independent PR-4 tasks still `[ ]` in `tasks.md`; they are NOT closeout work. The closeout is umbrella-bookkeeping; T116-T119 are downstream consumers of the umbrella's deliverables.

- **PR 3c.2** (the `marque-mvp-3 → marque-1.0` audit-schema cutover). Deferred per the 2026-05-14 PM decision documented in CLAUDE.md "Recent Changes"; orthogonal to PR 4b.

- **The retired numeric band.** PR 3b's closeout retired a 13-18 / 38-44 numeric band gate for sub-move scope. PR 4b never had a numeric-band gate — it had a per-PR functional-shape gate documented in each sub-PR's plan-of-record. No band retirement to perform; nothing to memorialize except "no numeric band was claimed."

### 1.3 Why this is the umbrella-completing PR

PR-4b umbrella consists of nine functional sub-PRs (each its own lattice-impl / `PageRewrite` / `ClosureRule` / hot-path-flip / deletion move) plus three closeout deliverables (T112-closeout-a attestation, -b exact-state pin, -c CI job). All nine functional sub-PRs have merged. The three closeout deliverables cannot land alongside any sub-PR — they aggregate facts that only become true after the last sub-PR merges (the umbrella net-delta math; the exact set of post-4b lattice impls / `PageRewrite` rows / `ClosureRule` rows; the CI branch list).

**PR 4b-F partially absorbed PR-4 tasks bookkeeping** — T112 / T113 / T114 / T115 are already `[X]` post-4b-F (verified at `tasks.md:338-341`). What 4b-F did NOT do (and what justifies this closeout PR existing rather than collapsing into 4b-F):
- 4b-F closed the four task checkboxes only; it did not perform the umbrella-level attestation that aggregates the discipline claims across all nine sub-PRs.
- 4b-F did not add the umbrella-level exact-state pin (the existing per-sub-PR catalog-pins and parity gates pin behavior, but not the umbrella's structural commitment "the closed set of 27 PageRewrite rows + 10 ClosureRule rows + 13 lattice-impl types is what PR-4b umbrella delivered").
- 4b-F did not declare the prefix-match CI job (which mirrors PR 3b's T029 precedent for keystone subsequence verification).

Once the closeout merges, PR 4b is done and the next umbrella scope (PR 5 / Stage 4 renderer work, or PR 4 follow-on T116-T119 lattice-law harness) can begin without umbrella-bookkeeping debt.

---

## 2. T112-closeout-a — Umbrella attestation

### 2.1 Lattice-impl inventory (post-4b-F)

Verification command (re-run pre-flight):

```bash
grep -nE '^impl(<.*>)? (JoinSemilattice|MeetSemilattice|BoundedJoinSemilattice|BoundedMeetSemilattice) for [A-Z]' \
  crates/capco/src/lattice.rs
```

Confirmed inventory at `5d3415cd` — 25 impls across 14 types:

| Type | Join | Meet | BoundedJoin | BoundedMeet | Sub-PR | §-citation grounding |
|---|---|---|---|---|---|---|
| `SciSet` | ✓ | ✓ | — | — | pre-4b (carried over) | §H.4 §A.6 p15 grammar |
| `SarSet` | ✓ | ✓ | — | — | pre-4b (carried over) | §H.5 pp99-102 |
| `FgiSet` | ✓ | ✓ | — | — | pre-4b (carried over) + 4b-E `from_attrs_iter` | §H.7 p122 p123 p128 |
| `AeaSet` | ✓ | ✓ | — | — | **4b-A #426** | §H.6 pp103-121 + §G.2 Table 5 p40 + §H.7 p122 |
| `ClassificationLattice` | ✓ | ✓ | ✓ | ✓ | **4b-B #437** | §H.1 pp47-54 + §H.2 p55 + §H.7 pp123-125 |
| `NatoClassLattice` | ✓ | ✓ | ✓ | ✓ | **4b-B #437** | §H.2 p55 |
| `DeclassifyOnLattice` | ✓ | ✓ | — | — | **4b-B #437** | §H.6 p104 |
| `NatoDissemSet` | ✓ | ✓ | — | — | **4b-B #437** | §G.2 p41 (reciprocity) |
| `RelToBlock` | ✓ | ✓ | — | — | **4b-B #437** + 4b-D.2 #527 D24 proptest | §H.8 pp150-151 + §D.2 Table 3 rows 9-13 + §H.9 p172/p174 |
| `DissemSet` | ✓ | — | — | — | **4b-B #437** (Join-only per PR #456) | §H.8 p136/p140/p145/pp155-156 + §D.2 Table 3 |
| `JointSet` | ✓ | — | — | — | **4b-B #437** (Join-only per PR #456) | §H.3 p56 + §H.3 p57 + §H.7 p123 |
| `DisplayOnlyBlock` | ✓ | — | — | — | **4b-E #539** | §H.8 (DISPLAY ONLY axis grounding) |
| `NonIcDissemSet` | — | — | — | — | **4b-E #539** (aggregator helper, NOT a lattice element) | §H.9 p172/p174/p178/p185 |
| `DeclassExemptionAccumulator` | — | — | — | — | **4b-E #539** (aggregator helper, NOT a lattice element) | §H.6 (last-observed exemption) |

**Totals**: 13 lattice-element types + 2 aggregator helpers. 12 `JoinSemilattice` impls + 9 `MeetSemilattice` impls + 2 `BoundedJoinSemilattice` impls + 2 `BoundedMeetSemilattice` impls = 25 trait impls.

**Note on `SupersessionSet`**: lives in `marque-scheme`, not `marque-capco`; the umbrella's lattice claim is for the `marque-capco` per-axis types only. `SupersessionSet`'s join-only status was already verified by PR #538's proptest audit (memory `project_pr538_observational_lattice_audit`) before the 4b series began.

**Note on aggregator helpers**: the doc-comment at `crates/capco/src/lattice.rs:3528` explicitly documents the precedent: "drop the `JoinSemilattice` impl" for `NonIcDissemSet` because it's a per-portion aggregator that aggregates observational state, not a per-axis lattice element. `DeclassExemptionAccumulator` follows the same precedent. These appear in the inventory for completeness but are NOT lattice types under PR 4b's structural claim.

### 2.2 PageRewrite catalog (post-4b-F)

Verification command:

```bash
grep -cE 'PageRewrite::(declarative|custom)' crates/capco/src/scheme/rewrites/*.rs
```

Confirmed: 27 rows across 6 group files, composed in `build_page_rewrites()` (`crates/capco/src/scheme/rewrites/mod.rs::build_page_rewrites`):

| Group file | Row count | Group authority |
|---|---|---|
| `pattern_a.rs` | 4 | "X implies NOFORN" (Pattern A: non-IC dissem + SCI subset) per §H.8 p134 + §H.9 |
| `pattern_b.rs` | 2 | "X evicts FOUO" (Pattern B: classification OR any non-FD&R control) per §H.8 p134 (added by 4b-C) |
| `pattern_c.rs` | 8 | "X strips when classified" (Pattern C: FOUO/SBU/LIMDIS/UCNI/DCNI with promote-before-strip ordering) per §H.6 pp116-118 + §H.8 p134 + §H.9 p170/p176 (added by 4b-C) |
| `supersession.rs` | 2 | same-axis supersession (sbu-nf-supersedes-sbu / les-nf-supersedes-les) per §H.9 p178/p185 (added by #552/#555) |
| `noforn_clears.rs` | 3 | NOFORN clears REL TO / FD&R family / DISPLAY ONLY per §H.8 p145 |
| `transmutation_stubs.rs` | 8 | Phase-3 transmutation stubs (`never_fires` / `noop_action` placeholders) per Phase B |

Composition order in `build_page_rewrites()` is load-bearing — the topological scheduler breaks ties on declaration order, so reordering would silently shift the rewrite schedule. The catalog-author intent is documented at `crates/capco/src/scheme/rewrites/mod.rs::build_page_rewrites` doc-comment.

### 2.3 ClosureRule catalog (post-4b-F)

Verification command:

```bash
grep -cE '^const CLOSURE_[A-Z_]+: ClosureRule' crates/capco/src/scheme/closure.rs
```

Confirmed: 10 closure rows in `CAPCO_CLOSURE_RULES` at `crates/capco/src/scheme/closure.rs::CAPCO_CLOSURE_RULES`:

| Row name | Authority | Wired by |
|---|---|---|
| `CLOSURE_NOFORN_CAVEATED` | §B.3 Table 2 p21 | 4b-D.1 #517 (runtime activation) |
| `CLOSURE_REL_TO_USA_NATO` | §H.7 p127 + FR-048 (NATO REL TO portion-level) | 4b-D.1 #517 |
| `CLOSURE_HCS_O_IMPLIES_NF_OC` | §H.4 p64 | 4b-D.1 #517 |
| `CLOSURE_HCS_P_SUB_IMPLIES_NF_OC` | §H.4 p68 | 4b-D.1 #517 |
| `CLOSURE_SI_G_IMPLIES_OC` | §H.4 p80 | 4b-D.1 #517 |
| `CLOSURE_TK_BLFH_IMPLIES_NF` | §H.4 p87 | 4b-D.1 #517 |
| `CLOSURE_TK_IDIT_IMPLIES_NF` | §H.4 p91 | 4b-D.1 #517 |
| `CLOSURE_TK_KAND_IMPLIES_NF` | §H.4 p95 | 4b-D.1 #517 |
| `CLOSURE_RELIDO_SCI` | §H.4 + §H.8 p155-156 (RELIDO observed-unanimity on SCI portions) | 4b-D.1 #517 |
| `CLOSURE_RELIDO_US_CLASS` | §H.8 p155-156 (RELIDO observed-unanimity on US-classified portions) | 4b-D.1 #517 |

**Note on memory `project_pattern_d_already_shipped`**: Pattern D's "caveated → NOFORN" mechanism shipped as 7 closure rules pre-PR-4b-D (the `CLOSURE_NOFORN_*` family at `scheme.rs:5046-5215`); 4b-D.1 #517 was the **runtime activation** of the existing catalog via the `CapcoScheme::closure()` override (per lattice-design §3 (e)), not the addition of new rows. The 10 rows above represent the post-activation state, with 4 of the 7 pre-existing `*-implies-noforn` `PageRewrite` rows likely subsumed (mentioned in the memory; verify in PR-4b-F implementation report for the actual subsumption count if challenged).

### 2.4 Single CAPCO-§ citation discipline (attestation a)

**Verification approach**: re-grep each cited page anchor against `crates/capco/docs/CAPCO-2016.md` (`begin page <N>` markers) and commit the verification transcript inside the PR description. The `tools/citation-lint/` AST tool already runs in CI as a gate; the manual re-grep is the reviewer's evidence that they re-verified at point of attestation per Constitution VIII (citations propagated through attestation require re-verification).

**Per-row D13 single-§-citation status**:
- 13 lattice-impl types each carry one operative §-citation in their doc-comments (composite citations for `RelToBlock` and `FgiSet` are D13-compliant per the PR 3b precedent — a §-pair naming one operative rule across multiple §H templates is one citation, not multiple).
- 27 `PageRewrite` rows each carry one operative §-citation in their declaration body (the eight `transmutation_stubs.rs` rows are Phase-3 placeholders citing Phase B groundwork — they are stubs, not active rewrites; their stub-status is documented in the file header).
- 10 `ClosureRule` rows each carry one operative §-citation in their `const` declaration.
- 1 new Warn rule W004 carries §H.3 p57 + §H.7 p123 (composite per CV-4 PR 4b-B 8th-pass; one operative rule across two §H templates).

**Action items in T112-closeout-a attestation**:
- Re-grep transcript: each cited §X.Y pNN form verified against `crates/capco/docs/CAPCO-2016.md` page anchors.
- Confirm no row carries a bare `§NN` form (FR-018); the citation-lint AST tool already enforces.
- Confirm no `line NNNN` form remains (memory `feedback_citations_use_page_numbers`); existing `tools/citation-lint/` enforces.

### 2.5 Engine-crate touch ledger (attestation b)

Constitution VII §IV blocks scheme-adoption PRs from editing `marque-engine` / `marque-scheme` / `marque-core` / `marque-rules` / `marque-ism`. The 4b series breached this discipline **five** times with explicit within-006 precedent documentation in each sub-PR. The closeout aggregates the ledger:

| Sub-PR | Commit/Scope | Crate(s) touched | Justification recorded |
|---|---|---|---|
| **4b-B** | Commit 2 (OC-USGOV supersession + RELIDO observed-unanimity PageContext bugfixes) | `marque-ism` (`PageContext::expected_dissem_us`) | Bugfix-class deletions in `marque-ism`; no new scheme adopted. Within-006 precedent. |
| **4b-C** | Commit 5 (retire FOUO Step 3 + UCNI strip branches from `expected_*`) | `marque-ism` (`PageContext::expected_dissem_us`, `expected_aea_markings`) | Bugfix-class deletions; the two branches were superseded by the new declarative rows. Within-006 precedent. |
| **4b-D.2** | hot-path flip — `Engine::project` + drop `impl JoinSemilattice for CapcoMarking` per Copilot R1 D24 | `marque-engine` (`engine.rs::project_from_page_context` hot path) + `marque-scheme` (`MarkingScheme::Marking: JoinSemilattice` bound relaxation, `DiffInput<M>` bound relaxation) | The hot-path flip from `PageContext::expected_*` to `scheme.project(Scope::Page, ...)` is the umbrella's load-bearing semantic claim; the trait-bound relaxations were surgical fixes per D24. Within-006 precedent — load-bearing for the umbrella's hot-path commitment. |
| **4b-D.3** | consumer migration — S007 reads `ProjectedMarking` instead of `PageContext::is_solely_nato_classified` | `marque-ism` (`ProjectedMarking::is_solely_nato_classified` added) | The S007 rule needed a `ProjectedMarking`-shaped accessor; adding it to `marque-ism` was cleaner than threading a closure through the rule. Within-006 precedent. |
| **4b-E** | `assert_impl_all!(PageContext: Send, Sync)` compile-time check + relocate `sar_sort_key` to `crates/ism/src/sar_sort.rs` (T069 readiness) | `marque-ism` (`tests/send_sync.rs`, `src/sar_sort.rs`) | Constitution VI Send+Sync compile-time check; T069 readiness for the eventual `PageContext` struct retirement. Within-006 precedent. |

**Total within-006 precedent breaches**: 5. Each was explicitly documented in the originating sub-PR's "Engine-crate touch authorization" line per the CLAUDE.md "Recent Changes" record. The closeout's contribution is to aggregate the ledger into one reviewable surface.

**Closeout itself MUST be zero engine-crate edits** (Constitution VII §IV, no within-006 precedent claim available — the closeout is bookkeeping, not scheme-adoption work; OQ-4 default).

### 2.6 Net-lattice-delta math (attestation c)

Per-axis running counter from the pre-4b baseline through each sub-PR's contribution:

| Step | Sub-PR | Join impls | Meet impls | Bounded* impls | Aggregator helpers | PageRewrite rows | ClosureRule rows | Registered Rule count |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Pre-4b baseline | — | 3 (Sci/Sar/Fgi) | 3 (Sci/Sar/Fgi) | 0 | 0 | ~14 (pre-Pattern-B/C) | 0 (catalog declared, not runtime-activated) | 38 |
| **4b-A #426** (AeaSet) | +1 | +1 | 0 | 0 | 0 | 0 | 0 | 0 |
| **4b-B #437** (7 lattice types + W004) | +7 (Class/NatoClass/Joint/Dissem/NatoDissem/RelToBlock/DeclassifyOn) | +5 (Class/NatoClass/NatoDissem/RelToBlock/DeclassifyOn — Joint+Dissem are join-only per PR #456) | +2 each (Class/NatoClass Bounded) | 0 | 0 | 0 | +1 (W004) |
| **4b-C #468** (Pattern B + C) | 0 | 0 | 0 | 0 | +9 (Pattern B 2 + Pattern C 7; "14 → 23" per the CLAUDE.md Recent Changes entry — re-verify the exact delta at attestation) | 0 | 0 |
| **4b-D.0 #514** (ClosureRule generic) | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| **4b-D.1 #517** (closure runtime activation) | 0 | 0 | 0 | 0 | 0 | +10 (runtime activation of pre-existing catalog) | 0 |
| **4b-D.2 #527** (hot-path flip) | 0 (dropped `impl JoinSemilattice for CapcoMarking` per D24) | 0 (also dropped `impl MeetSemilattice for CapcoMarking`) | 0 | 0 | 0 | 0 | 0 |
| **4b-D.3 #535** (S007 consumer migration) | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| **4b-E #539** (PageContext deletion + 5 helpers) | +1 (DisplayOnlyBlock) | 0 | 0 | +2 (NonIcDissemSet, DeclassExemptionAccumulator) | 0 | 0 | 0 |
| **4b-F #542** (residue cleanup + bookkeeping) | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| **Post-4b-F terminal state** | **12** | **9** | **4** | **2** | **27** | **10** | **39** | |

**Terminal-state verification commands** (re-run pre-flight):

```bash
# Lattice impls
grep -cE '^impl(<.*>)? JoinSemilattice for [A-Z]' crates/capco/src/lattice.rs       # Expected: 12
grep -cE '^impl(<.*>)? MeetSemilattice for [A-Z]' crates/capco/src/lattice.rs       # Expected: 9
grep -cE '^impl(<.*>)? BoundedJoinSemilattice for [A-Z]' crates/capco/src/lattice.rs # Expected: 2
grep -cE '^impl(<.*>)? BoundedMeetSemilattice for [A-Z]' crates/capco/src/lattice.rs # Expected: 2

# PageRewrite rows
grep -cE 'PageRewrite::(declarative|custom)' crates/capco/src/scheme/rewrites/*.rs   # Expected: 27 (4+2+8+2+3+8)

# ClosureRule rows
grep -cE '^const CLOSURE_[A-Z_]+: ClosureRule' crates/capco/src/scheme/closure.rs    # Expected: 10

# Registered rule count
awk '/^impl CapcoRuleSet \{/,/^impl RuleSet for CapcoRuleSet \{/' crates/capco/src/rules.rs | grep -cE "Box::new\(" # Expected: 39
```

**Note on the registered-rule count drift**: the existing pin at `crates/capco/tests/post_3b_registration_pin.rs` asserts 38, but raw `Box::new` count is 39 (W004 was added in 4b-B per CLAUDE.md "registered rule count 38 → 39"). The misnomer is innocuous — the pin's doc-comment text is post-PR-#470, post-PR-5, post-#488; the body has rolled forward. **However**: I cannot find an obvious step where `EXPECTED_RULE_IDS` was extended to 39 by adding `"W004"`. **This is a pre-existing inconsistency that closeout SHOULD verify and resolve in T112-closeout-a's attestation transcript, NOT in this PR's body** (OQ-3 default — see §6). If the pin actually fails at HEAD on `5d3415cd`, that's a closeout-side observation worth documenting but not fixing here; if it passes, then `EXPECTED_RULE_IDS` already has `"W004"` and the doc-comment count text just needs a one-line update in a separate trivial commit (still no rule-logic change).

### 2.7 ≤3-branches-per-`impl Rule` (D13 attestation b carry-over)

The PR 3b umbrella's "≤3 branches per `impl Rule`" gate was a Stage-1 retirement attestation. PR 4b's structural commitment is different — it's about declarative-`PageRewrite` rows replacing imperative banner-rollup code, not about consolidating procedural rule bodies. The ≤3-branch claim **does not carry over** as an attestation axis for PR 4b.

What 4b's analogous gate IS: **"per-PR functional-shape gate"** — each 4b sub-PR documented in its plan-of-record exactly which lattice types / `PageRewrite` rows / `ClosureRule` rows it added, with no row introduced outside that scope. Verification: each sub-PR's "PM-approved 4-pass" or "Copilot R1/R2 review" in the merged plan-of-record confirms the scope was held. The closeout PR's attestation simply notes "no 4b sub-PR breached its functional-shape scope" with a pointer to each sub-PR's plan doc.

### 2.8 Decision: does T112-closeout-a require any code edits?

**Verdict: zero code edits.** The post-4b-F state in `staging` (HEAD `5d3415cd`) already satisfies (a)/(b)/(c) verification at the commands above. T112-closeout-a is **pure PR-description attestation** — the reviewer drafts §2.1-2.7 above into the umbrella closeout PR description and re-verifies each claim against the merged tree at branch HEAD.

If the closeout reviewer surfaces drift during pre-flight (e.g., the 38-vs-39 pin inconsistency in §2.6's note actually fails), each finding is enumerated as a **closeout-side gap** and either fixed in this PR (only if text-only — a doc-comment count update, a comment correction) or punted to a follow-up PR (if logic-touching). The closeout PR's body documents the disposition.

---

## 3. T112-closeout-b — Exact-state pin

### 3.1 Drift class this catches

The existing test infrastructure (`corpus_parity.rs::rule_count_reflects_registration_changes`, `crates/capco/tests/scheme_equivalence.rs` per-PR-4b-E renamed `lattice_vs_scheme_parity.rs`, the various per-sub-PR catalog-pin tests) pins **counts** and **behavior**, not the **closed set of names**. The umbrella's structural commitment — "the closed set of 27 PageRewrite rows + 10 ClosureRule rows + 13 lattice-element types is what PR-4b umbrella delivered" — has no existing test that catches:
- a `PageRewrite` row renamed at the same count;
- a `ClosureRule` row swapped for a different row at the same count;
- a `JoinSemilattice` impl moved off one type and onto another at the same impl-count.

The exact-state pin closes that gap, mirroring the precedent set by `post_3b_registration_pin.rs` for the rule-ID set.

### 3.2 Proposed pin: `crates/capco/tests/post_4b_lattice_inventory_pin.rs`

**File location**: `crates/capco/tests/post_4b_lattice_inventory_pin.rs` (new file).

**Test body** (sketch — implement only after PM approval per OQ-1):

```rust
//! Post-PR-4b umbrella lattice + rewrite + closure inventory pin.
//!
//! Asserts the exact sets of:
//!  * 12 `JoinSemilattice` impls + 9 `MeetSemilattice` impls + 2
//!    `BoundedJoinSemilattice`/`BoundedMeetSemilattice` impls in
//!    `marque-capco::lattice` (via `static_assertions::assert_impl_all!`);
//!  * 27 `PageRewrite::name` values returned by
//!    `<CapcoScheme as MarkingScheme>::page_rewrites()`;
//!  * 10 `ClosureRule::name` values returned by
//!    `<CapcoScheme as MarkingScheme>::closure_rules()`.
//!
//! ## Why this exists
//!
//! The umbrella's structural commitment is the closed set of these
//! lattice / rewrite / closure surfaces. Per-sub-PR catalog-pins
//! pin per-row behavior; this pin catches the orthogonal
//! "renamed at the same count" / "swapped at the same count"
//! drift class. Bumping this test requires intentional review;
//! do not silently edit `EXPECTED_PAGE_REWRITES`,
//! `EXPECTED_CLOSURE_RULES`, or the `assert_impl_all!` block to
//! make CI green.
//!
//! Authority: `docs/plans/2026-05-19-pr4b-closeout-architect-plan.md`
//! §3.2; per-sub-PR catalog-pins (`class_floor_catalog.rs`,
//! `sci_per_system_catalog.rs`, parity gate at
//! `lattice_vs_scheme_parity.rs`) cover per-row behavior.

use marque_capco::lattice::{
    AeaSet, ClassificationLattice, DeclassifyOnLattice, DisplayOnlyBlock,
    DissemSet, FgiSet, JointSet, NatoClassLattice, NatoDissemSet,
    RelToBlock, SarSet, SciSet,
};
use marque_capco::CapcoScheme;
use marque_scheme::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice,
    MarkingScheme, MeetSemilattice,
};
use static_assertions::assert_impl_all;
use std::collections::BTreeSet;

// ---- Compile-time lattice-impl pin ----

// 12 types with JoinSemilattice
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

// 9 types with MeetSemilattice (DissemSet, JointSet, DisplayOnlyBlock are join-only per PR #456)
assert_impl_all!(SciSet: MeetSemilattice);
assert_impl_all!(SarSet: MeetSemilattice);
assert_impl_all!(FgiSet: MeetSemilattice);
assert_impl_all!(AeaSet: MeetSemilattice);
assert_impl_all!(ClassificationLattice: MeetSemilattice);
assert_impl_all!(NatoClassLattice: MeetSemilattice);
assert_impl_all!(DeclassifyOnLattice: MeetSemilattice);
assert_impl_all!(NatoDissemSet: MeetSemilattice);
assert_impl_all!(RelToBlock: MeetSemilattice);

// 2 types with Bounded* halves
assert_impl_all!(ClassificationLattice: BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_impl_all!(NatoClassLattice: BoundedJoinSemilattice, BoundedMeetSemilattice);

// ---- Runtime PageRewrite + ClosureRule pin ----

/// Closed set of 27 `PageRewrite::name` values returned by
/// `<CapcoScheme as MarkingScheme>::page_rewrites()` post-4b-F.
const EXPECTED_PAGE_REWRITES: &[&str] = &[
    // pattern_a.rs (4 rows — list names by re-reading the file at attestation time)
    // pattern_b.rs (2 rows)
    // pattern_c.rs (8 rows)
    // supersession.rs (2 rows)
    // noforn_clears.rs (3 rows)
    // transmutation_stubs.rs (8 rows)
    // [Fill at implementation time by grep'ing PageRewrite::declarative/custom names]
];

/// Closed set of 10 `ClosureRule::name` values exposed by
/// `<CapcoScheme as MarkingScheme>::closure_rules()` post-4b-F.
const EXPECTED_CLOSURE_RULES: &[&str] = &[
    // CLOSURE_NOFORN_CAVEATED, CLOSURE_REL_TO_USA_NATO,
    // CLOSURE_HCS_O_IMPLIES_NF_OC, CLOSURE_HCS_P_SUB_IMPLIES_NF_OC,
    // CLOSURE_SI_G_IMPLIES_OC, CLOSURE_TK_BLFH_IMPLIES_NF,
    // CLOSURE_TK_IDIT_IMPLIES_NF, CLOSURE_TK_KAND_IMPLIES_NF,
    // CLOSURE_RELIDO_SCI, CLOSURE_RELIDO_US_CLASS
    // [Fill at implementation time by reading each ClosureRule::name field]
];

#[test]
fn post_4b_declares_exact_27_page_rewrites() {
    let scheme = CapcoScheme::new();
    let actual: BTreeSet<&str> = scheme.page_rewrites().iter().map(|r| r.name).collect();
    let expected: BTreeSet<&str> = EXPECTED_PAGE_REWRITES.iter().copied().collect();
    assert_eq!(actual.len(), 27, "post-4b PageRewrite count drift: {actual:?}");
    let missing: Vec<_> = expected.iter().filter(|n| !actual.contains(*n)).collect();
    let unexpected: Vec<_> = actual.iter().filter(|n| !expected.contains(n)).collect();
    assert!(
        missing.is_empty() && unexpected.is_empty(),
        "post-4b PageRewrite name set drifted. Missing: {missing:?}. \
         Unexpected: {unexpected:?}. Bumping this test requires \
         intentional review.",
    );
}

#[test]
fn post_4b_declares_exact_10_closure_rules() {
    let scheme = CapcoScheme::new();
    let actual: BTreeSet<&str> = scheme.closure_rules().iter().map(|r| r.name).collect();
    let expected: BTreeSet<&str> = EXPECTED_CLOSURE_RULES.iter().copied().collect();
    assert_eq!(actual.len(), 10, "post-4b ClosureRule count drift: {actual:?}");
    let missing: Vec<_> = expected.iter().filter(|n| !actual.contains(*n)).collect();
    let unexpected: Vec<_> = actual.iter().filter(|n| !expected.contains(n)).collect();
    assert!(
        missing.is_empty() && unexpected.is_empty(),
        "post-4b ClosureRule name set drifted. Missing: {missing:?}. \
         Unexpected: {unexpected:?}. Bumping this test requires \
         intentional review.",
    );
}
```

**Why a new file**: same rationale as `post_3b_registration_pin.rs` — discoverability from filename alone for the audience (umbrella reviewers, future PR-5 / Stage-4 authors). The file is `marque-capco`-internal so it doesn't drag a new `static_assertions` dependency through the workspace if one doesn't already exist (verify in pre-flight; if `static_assertions` isn't already a `[dev-dependencies]` of `marque-capco`, add it as a trivial Cargo.toml edit per OQ-2's recommendation).

**Crate placement**: `crates/capco/tests/` — same as the existing pins, same crate as `CapcoScheme::page_rewrites()` / `closure_rules()` / the per-type lattice impls. **Constitution VII §IV-clean**: zero edits to `crates/{engine,scheme,core,rules,ism}`.

### 3.3 Recommendation

**Both pin shapes** (compile-time `assert_impl_all!` block + runtime exact-set tests for `PageRewrite` / `ClosureRule` names), mirroring the PR 3b precedent of separating count-pins from exact-set pins.

The `assert_impl_all!` block is load-bearing because it catches the "lattice impl removed from one type and added to a different type with the same name elsewhere" drift class — a runtime test cannot inspect the trait's impl-graph cheaply, but the compile-time assertion does.

The runtime exact-set tests are load-bearing because `PageRewrite` and `ClosureRule` rows are data values whose names are only inspectable at runtime through the trait surface (`MarkingScheme::page_rewrites()` returns a `Vec<PageRewrite<Self>>`; the names live inside each row's `name: &'static str` field).

---

## 4. T112-closeout-c — CI prefix-match job

### 4.1 Precedent

PR 3b's T029 added a `pr-3b-corpus-regression` CI job at `.github/workflows/ci.yml:161-227`, branch-filtered to `refactor-006-pr-3b*` via `startsWith(github.ref, ...)`. The job body is identical to T025's PR-3a-only sweep: three corpus suites (`corpus_parity` in `marque-capco`, `corpus_accuracy` in `marque-engine`, `corpus_provenance` in `marque`), plus the Phase 4 gated suites with `decoder-harness` + `corpus-override` features. SC-014 interpretation per the 3b plan: each keystone PR gets ONE job (T025 for 3a, T029 for 3b, this new job for 4b).

### 4.2 Recommendation: `pr-4b-corpus-regression` job

**Decision: YES, declare the job.** Three reasons:
1. **Precedent**: T025 (3a) + T029 (3b) means the third keystone PR in the engine-rule-refactor sequence (4b) should get its analogue. Not declaring it would be an asymmetry that the next reviewer working in this surface would have to re-derive a reason for.
2. **Retroactive coverage**: even though all nine 4b sub-PRs have merged, the prefix-match filter `refactor-006-pr-4b*` covers any future hot-fix branch with the `refactor-006-pr-4b` prefix (cherry-pick fixes to a 4b-prefixed branch, regression detection on follow-up patches), giving the umbrella the same revertability guarantee as 3a / 3b.
3. **Bench-gate independence**: the corpus regression sweep is orthogonal to the existing `lint_10kb` / `decoder_10kb` Criterion gates; both should run on a closeout-branch CI run.

### 4.3 Job body

Identical to T029's body (`.github/workflows/ci.yml:161-227`), with the branch filter changed from `refactor-006-pr-3b*` to `refactor-006-pr-4b*`:

```yaml
pr-4b-corpus-regression:
  name: PR 4b corpus regression (T112-closeout-c)
  needs: check
  if: |
    startsWith(github.ref, 'refs/heads/refactor-006-pr-4b') ||
    startsWith(github.head_ref, 'refactor-006-pr-4b')
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@0c366fd6a839edf440554fa01a7085ccba70ac98
    - uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9
      with:
        toolchain: "nightly"
    - uses: Swatinem/rust-cache@3d9a6aef83b697b1d758490972e37a033aee461f
    - name: cargo test corpus regression suites
      run: |
        cargo test -p marque-capco --test corpus_parity \
          --features marque-engine/corpus-override
        cargo test -p marque-engine --test corpus_accuracy \
          --features marque-engine/corpus-override
        cargo test -p marque --test corpus_provenance \
          --features marque-engine/corpus-override
    - name: cargo test (Phase 4 gated suites)
      run: |
        cargo test \
          -p marque-engine -p marque \
          --features marque-engine/decoder-harness,marque-engine/corpus-override,marque/corpus-override
```

Plus a comment block above the job mirroring T029's structure, citing SC-014 + the umbrella subsequence semantics: "T025 / T029 / T112-closeout-c — corpus regression sweep × {3a / 3b / 4b} subsequence."

### 4.4 Slot location

Insert as a new job between line 227 (end of `pr-3b-corpus-regression`) and line 233 (the `# FR-039 — masking-pin lint.` comment block intro). The file grows by ~55 lines (the job body is comparable to T029).

---

## 5. Spec-doc bookkeeping

### 5.1 `specs/006-engine-rule-refactor/tasks.md` — add 3 closeout task IDs, mark DONE

T112 / T113 / T114 / T115 are ALREADY `[X]` (closed in 4b-A through 4b-F); no checkbox flips needed for those. New tasks to ADD (per OQ-6 default — mirror the T027/T028/T029 placement pattern):

- **T112-closeout-a** [US6] PR 4b umbrella closeout: attestation aggregating §-citation discipline + engine-crate touch ledger + net-lattice-delta math (FR-018; PR-4b)
- **T112-closeout-b** [US6] PR 4b umbrella closeout: exact-state pin at `crates/capco/tests/post_4b_lattice_inventory_pin.rs` covering 12 Join + 9 Meet + 2 Bounded × 2 lattice impls + 27 PageRewrite names + 10 ClosureRule names (FR-018; PR-4b)
- **T112-closeout-c** [US6] PR 4b umbrella closeout: `pr-4b-corpus-regression` CI job with `refactor-006-pr-4b*` prefix-match filter (SC-014; PR-4b)

Slot location in `tasks.md`: after T119c (the existing last task in Phase 8 / US6), or grouped after T115 (the last 4b-direct task) — see OQ-6 for placement preference.

The closeout commit MUST flip the three new IDs from `[ ]` to `[X]` in the same commit body that contains the new file + new CI job + attestation prose.

### 5.2 `specs/006-engine-rule-refactor/plan.md` — umbrella-LANDED annotation

Locate the PR 4 row in the plan body (search `^### PR 4` or `PR 4b` if PR 4 was subdivided). Append a one-line annotation mirroring the 3b precedent:

> **PR 4b umbrella — LANDED 2026-05-{15→18}** (sub-PRs #426 / #437 / #468 / #514 / #517 / #527 / #535 / #539 / #542; closeout #NNN aggregating T112-closeout-a / -b / -c).

If `plan.md` doesn't have a row-by-row PR-4 structure (the earlier grep showed only top-level `## Summary` / `## PR 0 Absorption` / `## Phase artifacts` headings; no `### PR 4` ATX heading), the annotation lands inline in the PR-4 prose section at whatever §-level the document carries PR-4 in.

### 5.3 `CLAUDE.md` "Recent Changes" — closeout entry

Prepend a closeout entry to `CLAUDE.md:282` "Recent Changes" mirroring the pattern of existing 4b-{A,B,C,D,E,F} entries:

> - **PR 4b umbrella closeout** (006 T112-closeout, 2026-05-NN): bookkeeping-only PR aggregating the nine-sub-PR umbrella attestation. **T112-closeout-a** — single-§-citation discipline (D13) verified across 13 lattice-impl types + 27 PageRewrite rows + 10 ClosureRule rows + 1 new Warn rule W004; engine-crate touch ledger documents 5 within-006 Constitution VII §IV precedent breaches (4b-B Commit 2 / 4b-C Commit 5 / 4b-D.2 / 4b-D.3 / 4b-E); net-lattice-delta math from pre-4b baseline through post-4b-F terminal state (12 Join + 9 Meet + 4 Bounded* impls; 27 PageRewrite rows; 10 ClosureRule rows; 39 registered rules). **T112-closeout-b** — new exact-state pin at `crates/capco/tests/post_4b_lattice_inventory_pin.rs` (compile-time `assert_impl_all!` block + runtime PageRewrite / ClosureRule name-set assertions). **T112-closeout-c** — new `pr-4b-corpus-regression` CI job with `refactor-006-pr-4b*` prefix-match filter, mirroring T025 (3a) / T029 (3b). Zero rule-logic edits; zero engine-crate edits (Constitution VII §IV — closeout is bookkeeping, not scheme adoption). See `docs/plans/2026-05-19-pr4b-closeout-architect-plan.md`.

### 5.4 `CLAUDE.md` "Current Status" — cosmetic update

Update `CLAUDE.md:261` "39 registered CAPCO rules post-PR-4b-B" → "39 registered CAPCO rules post-PR-4b (umbrella complete)" — same cosmetic pattern as the PR 3b closeout. Signals umbrella-completed state to the next session's agents.

---

## 6. Open questions for PM

**(OQ-1) Pin file location: new file or extend an existing pin?**
- Default recommended: **new file** at `crates/capco/tests/post_4b_lattice_inventory_pin.rs` (per §3.2 rationale — discoverability from filename alone for umbrella audience; the existing `post_3b_registration_pin.rs` precedent confirms this is the project's preferred shape for umbrella structural pins).
- Alternative: extend `post_3b_registration_pin.rs` (rename to `post_pr_umbrella_pins.rs` or similar). Rejected because conflating two umbrella's attestations into one file makes the assertion's audience less discoverable.
- Impact: stylistic only; both options are Constitution-clean.

**(OQ-2) `static_assertions` dependency: add as new dev-dep or rely on existing?**
- Default recommended: **verify in pre-flight whether `static_assertions` is already a `[dev-dependencies]` of `marque-capco`** (PR 4b-E added an `assert_impl_all!` in `crates/ism/tests/send_sync.rs` so the workspace has the crate; the question is whether `marque-capco` already declares it). If yes, no Cargo.toml edit needed; if no, add it as a one-line Cargo.toml edit in the closeout PR.
- Alternative: rely solely on runtime `TypeId`-based reflection. Rejected because `assert_impl_all!` is the precedent already in-tree at `crates/ism/tests/send_sync.rs:N` and gives compile-time guarantees the runtime check cannot.
- Impact: trivial Cargo.toml edit if needed; not a Constitution VII §IV concern (Cargo.toml edits on the closeout crate are scheme-adoption-internal, not engine-crate edits).

**(OQ-3) Pre-existing 38-vs-39 inconsistency in `post_3b_registration_pin.rs`: fix in closeout or punt to follow-up?**
- Default recommended: **verify in pre-flight whether the pin actually passes at HEAD `5d3415cd`**. If it passes, the doc-comment text is out of sync with the body (cosmetic — fix in the closeout commit, no rule-logic change). If it fails, that's a closeout-side observation — fix in a SEPARATE trivial commit on the closeout branch, never inside the umbrella-attestation commit (so the attestation commit stays bookkeeping-only).
- Alternative: leave it for a follow-up PR. Rejected because the inconsistency is a citation-fidelity issue (the pin's own doc-comment is wrong about post-state, which violates the audit-trail discipline that PR 4b's closeout exists to attest).
- Impact: zero if pre-flight passes; one trivial fix commit if pre-flight fails.

**(OQ-4) Authorize a closeout-side engine-crate touch if a closeout-side gap requires one?**
- Default recommended: **NO**. Closeout MUST be zero engine-crate edits. The 4b umbrella series already breached Constitution VII §IV five times with explicit within-006 precedent; the closeout cannot claim within-006 precedent because it's not scheme-adoption work, it's bookkeeping. If pre-flight surfaces an engine-crate edit need, file it as a follow-up PR with its own attestation.
- Alternative: pre-authorize a sixth within-006 precedent line for closeout. Rejected because closeout's purpose is to memorialize the umbrella's discipline — it cannot also breach that discipline. Surface any engine-crate need as a follow-up.
- Impact: closeout PR scope discipline.

**(OQ-5) Net-lattice-delta math granularity: per-axis or aggregate?**
- Default recommended: **per-axis** per §2.6 (separate columns for Join impls, Meet impls, Bounded* impls, aggregator helpers, PageRewrite rows, ClosureRule rows, registered Rule count). Mirrors the 3b precedent of "net rule delta" being a multi-column running counter, not a single number.
- Alternative: aggregate to a single number per row ("total lattice-related deltas"). Rejected because the umbrella's structural commitment is per-axis (an umbrella that added 0 PageRewrite rows but 7 lattice impls is meaningfully different from one that added 7 PageRewrite rows and 0 lattice impls).
- Impact: §2.6 table column count.

**(OQ-6) Closeout task ID nomenclature in `tasks.md`: `T112-closeout-{a,b,c}` / `T119d-e-f` / something else?**
- Default recommended: **`T112-closeout-a` / `T112-closeout-b` / `T112-closeout-c`** — explicit umbrella-closeout naming that signals the relationship to T112. The PR 3b precedent (T027/T028/T029) used flat numbering, but T112's own naming convention (T112 / T112a / T112b would be the natural extension) collides with the existing T112-T119 sequence already heavily annotated.
- Alternative 1: `T119d` / `T119e` / `T119f` — extend the existing trailing T119* numbering. Rejected because T119a-c are predicate-coverage catalog work, not umbrella closeout — collision of semantic groupings.
- Alternative 2: `T120` / `T121` / `T122` — fresh sequential. Rejected because it loses the "umbrella closeout for PR 4b" semantic the IDs should carry forward.
- Impact: stylistic; either option works.

**(OQ-7) Closeout PR branch filter: `refactor-006-pr-4b*` or enumerate explicitly?**
- Default recommended: **prefix-match `refactor-006-pr-4b*`** mirroring the T029 precedent. The umbrella naming convention is namespace-clean (each sub-PR's branch carried the `refactor-006-pr-4b-{A,B,C,D-{0,1,2,3},E,F}` form); any future 4b-followup branch is in scope by construction without an explicit ci.yml edit.
- Alternative: enumerate explicitly (`refactor-006-pr-4b-a` / `refactor-006-pr-4b-b` / etc.). Rejected for the same reason as T029 — more brittle, requires editing every future sub-PR-branch name into ci.yml.
- Impact: §4.3 `if:` condition shape.

**(OQ-8) Should the `transmutation_stubs.rs` 8 rows count as "delivered by PR 4b" in net-delta math, or as "pre-existing Phase B stubs"?**
- Default recommended: **count them as pre-existing Phase B stubs** (the file header documents them as "Phase-3 transmutation stubs" with `never_fires` / `noop_action` placeholders — they're declared but not runtime-active). The umbrella's PageRewrite delivery is 19 active rows (4 + 2 + 8 + 2 + 3 = 19), with 8 additional pre-existing stubs visible in `build_page_rewrites()` for declaration ordering but not contributing semantic behavior.
- Alternative: count all 27 as "delivered by PR 4b umbrella." Rejected because the stubs were declared before the 4b series began (per Phase B).
- Impact: §2.6 PageRewrite row column. The exact-state pin (T112-closeout-b) MUST include all 27 names regardless of stub status — the structural commitment is on the closed name set, not on active-vs-stub status.

---

## 7. Verification commands

Pre-flight (run before opening PR; mirror PR 3b §"Acceptance criteria" item #1):

```bash
# Lattice impl count — verify §2.1 / §2.6 terminal-state claims
grep -cE '^impl(<.*>)? JoinSemilattice for [A-Z]' crates/capco/src/lattice.rs       # Expected: 12
grep -cE '^impl(<.*>)? MeetSemilattice for [A-Z]' crates/capco/src/lattice.rs       # Expected: 9
grep -cE '^impl(<.*>)? BoundedJoinSemilattice for [A-Z]' crates/capco/src/lattice.rs # Expected: 2
grep -cE '^impl(<.*>)? BoundedMeetSemilattice for [A-Z]' crates/capco/src/lattice.rs # Expected: 2

# PageRewrite row count
grep -cE 'PageRewrite::(declarative|custom)' crates/capco/src/scheme/rewrites/*.rs  # Expected: 27

# ClosureRule row count
grep -cE '^const CLOSURE_[A-Z_]+: ClosureRule' crates/capco/src/scheme/closure.rs   # Expected: 10

# Registered rule count
awk '/^impl CapcoRuleSet \{/,/^impl RuleSet for CapcoRuleSet \{/' crates/capco/src/rules.rs | grep -cE "Box::new\("  # Expected: 39

# Engine-crate touch ledger evidence
git log --oneline --all --grep='Engine-crate touch authorization' | head -10
git log --oneline --all --grep='within-006 precedent' | head -10

# OQ-3: pin pre-flight
cargo test -p marque-capco --test post_3b_registration_pin

# Citation-lint AST gate (existing CI; runs pre-flight too)
cd tools/citation-lint && cargo run

# Full pre-PR-open chain
cargo +stable check --workspace
cargo +stable clippy --workspace --all-targets -- -D warnings
cargo +stable fmt --check
cargo +stable nextest run --workspace --profile ci
bash scripts/bench-check.sh    # lint_10kb / decoder_10kb_one_mangled_region / lint_scaling / deadline_overhead within baseline+10%
```

**Reviewer dispatch before PR open** (per memory `feedback_run_reviewer_before_pr_open`):

Dispatch `rust-reviewer` + `code-reviewer` in parallel against the closeout-branch HEAD. Reviewer attestation points:
- (a) Constitution VII §IV no-engine-crate-edits — diff against `crates/{engine,scheme,core,rules,ism}/` is empty;
- (b) Constitution V Principle V no-new-`__engine_promote`-callers — diff against `AppliedFix::__engine_promote` callers is empty;
- (c) Constitution VIII citation fidelity — every §-citation in the new pin file, the attestation prose, and the spec-doc updates verifies against `crates/capco/docs/CAPCO-2016.md` page anchors;
- (d) The new exact-state pin catches the targeted drift class (rename-at-same-count + swap-at-same-count); verify by manually swapping one `PageRewrite::name` in a test branch and confirming the pin fails;
- (e) The new CI job branch filter actually fires on the closeout branch; verifiable by checking the closeout PR's CI run-list once opened.

---

## 8. Critical files for implementation

- `/home/knitli/marque/.claude/worktrees/pr-4b-closeout/.github/workflows/ci.yml` (insert new job between line 227 and line 233)
- `/home/knitli/marque/.claude/worktrees/pr-4b-closeout/crates/capco/tests/post_4b_lattice_inventory_pin.rs` (NEW)
- `/home/knitli/marque/.claude/worktrees/pr-4b-closeout/specs/006-engine-rule-refactor/tasks.md` (add three closeout task IDs per §5.1; check off in same commit)
- `/home/knitli/marque/.claude/worktrees/pr-4b-closeout/specs/006-engine-rule-refactor/plan.md` (umbrella-LANDED annotation per §5.2)
- `/home/knitli/marque/.claude/worktrees/pr-4b-closeout/CLAUDE.md` (Recent Changes prepend per §5.3; Current Status cosmetic update per §5.4)
- `/home/knitli/marque/.claude/worktrees/pr-4b-closeout/crates/capco/Cargo.toml` (only if OQ-2 pre-flight shows `static_assertions` is not yet a dev-dep — one-line edit)

---

## 9. Decisions deferred to PM

Eight open questions in §6. None are blocking; all are stylistic / scope-confirmation items resolvable in plan-review without further exploration. The architect recommendation defaults summarized:

1. OQ-1 — pin file location: **new file**
2. OQ-2 — `static_assertions` dep: **verify pre-flight, add if missing**
3. OQ-3 — pre-existing 38-vs-39 inconsistency: **verify pre-flight, fix in separate trivial commit if pin fails**
4. OQ-4 — closeout-side engine-crate touch: **NO; file follow-up if needed**
5. OQ-5 — net-delta granularity: **per-axis**
6. OQ-6 — task ID nomenclature: **`T112-closeout-{a,b,c}`**
7. OQ-7 — branch filter: **prefix-match `refactor-006-pr-4b*`**
8. OQ-8 — transmutation_stubs accounting: **pre-existing Phase B stubs, but include in exact-state pin's closed name set**

---

## 10. Constitution gate

- **Constitution V Principle V (audit-first)**: closeout introduces no new `__engine_promote` call sites; the `assert_impl_all!` and runtime tests in T112-closeout-b's pin don't construct `AppliedFix` values.
- **Constitution VII §IV (acyclic graph + scheme-adoption boundary)**: closeout is zero engine-crate edits. Five within-006 precedent breaches by the 4b umbrella sub-PRs are documented in §2.5's ledger but NOT extended by closeout.
- **Constitution VIII (citation fidelity)**: every §-citation in the attestation prose, the spec-doc updates, the CLAUDE.md entry, and the exact-state pin's doc-comment is verified at point of authorship via §7 pre-flight commands; the citation-lint AST gate enforces in CI.
- **Pre-users / no deprecation phasing** (`feedback_pre_users_no_deprecation_phasing`): no alias maps, no schema-bump-for-back-compat, no deprecation comments. The closeout memorializes the umbrella's discipline; the umbrella itself rewrote freely.
- **Solo-driven** (`project_solo_driven`): no named-reviewer-as-hard-gate mechanism in this plan. The multi-agent reviewer chain (`rust-reviewer` + `code-reviewer` per §7) IS the load-bearing quality gate; Copilot review is reactive and supplemental.
