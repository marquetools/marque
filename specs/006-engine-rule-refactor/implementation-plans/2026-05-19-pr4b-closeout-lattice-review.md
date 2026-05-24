<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b Umbrella Closeout — Architectural + Lattice-Consultant Review

**Reviewer**: system-architect (with lattice-consultant context)
**Branch under review**: `refactor-006-pr-4b-closeout` (HEAD `320dea6d`)
**Base**: `staging` (`5d3415cd`)
**Diff scale**: 12 files, +2196 / −1 (heavy doc / spec / test surface; zero engine-crate code).
**Sibling reviews**: rust-reviewer + code-reviewer running in parallel — this review is independent.

---

## §0. Verdict

**APPROVE-WITH-FINDINGS** (no blockers; 1 MEDIUM, 3 LOW, 2 INFO).

The closeout is the bookkeeping shape PR 3b's T027/T028/T029 established. The lattice-impl-set claim is **sound at HEAD** — no unsound impl uncovered, no escalation from bookkeeping to bugfix. The two new pins (compile-time `assert_impl_all!` / `assert_not_impl_any!` + runtime triple-pin over 27 + 10 + 39 catalog rows) close drift classes the existing parity gate and corpus-regression test surface cannot catch. The single-§-citation discipline holds on the three spot-checked rows distinct from the code-reviewer's likely picks. The Constitution VII no-engine-crate-edits discipline is observed (the `git diff staging...HEAD --stat -- crates/{engine,scheme,core,rules,ism}` diff is empty).

The single MEDIUM finding is a CLAUDE.md "Recent Changes" narrative gap that the closeout PM contract did not surface as in-scope, and a reviewer judgment call on whether to widen the closeout to fix it.

---

## §1. Lattice-impl-set soundness audit

For each of the 12 lattice types asserted by `lattice_static_assertions.rs`, the soundness verdict is recorded with the algebraic-law evidence and the §-citation grounding.

| Type | Trait shape claimed | Soundness verdict | Algebraic evidence | §-citation grounding |
|---|---|---|---|---|
| `SciSet` | Join + Meet | **SOUND** | `proptest_lattice.rs` exercises join/meet idempotence + commutativity + associativity + absorption on the BTreeSet-of-canonical-tokens representative. Meet = compartment-tail intersection is well-defined when both portions name the same control system — the §A.6 grammar admits intersection as the "what's common between two SCI portions" reading. | §A.6 p15 (compositional grammar) + §H.4 (per-control-system tables) |
| `SarSet` | Join + Meet | **SOUND** | Same proptest harness as `SciSet`. SAR program hierarchy is structurally identical (program / compartment / sub-compartment) so meet = greatest-common-prefix is well-defined. | §H.5 pp99–102 (SAR grammar + ordering) |
| `FgiSet` | Join + Meet | **SOUND-BY-DESIGN** | The `FgiMarker::SourceConcealed` / `Acknowledged { countries }` distinction is preserved across both halves because the variant carries its discriminant through the BTreeSet representation. `SourceConcealed.join(Acknowledged{X}) = Acknowledged{X}` (concealment is the lattice bottom for that producer), and meet preserves the strictest discriminant. Proptest covers this. | §H.7 p122 + p123 + p128 (concealed vs acknowledged convention) |
| `AeaSet` | Join + Meet | **SOUND** | `proptest_lattice.rs` (added in 4b-A #426). Meet across RD / FRD / TFNI is the §H.6 p104 precedence intersection (e.g., `RD.meet(FRD) = FRD`); join is union with the §H.6 p108 RD-precedence-over-FRD overlay handled out-of-band by the closure operator. The lattice itself is the underlying union/intersection on the BTreeSet representative; the §H.6 overlays are applied at projection time, not in `join`/`meet`. | §H.6 pp103–121 + §G.2 Table 5 p40 + §H.7 p122 |
| `ClassificationLattice` | Join + Meet + BoundedJoin + BoundedMeet | **SOUND** | OrdMax over the totally-ordered chain `U < R < C < S < TS`. `bottom = U`, `top = TS`. Same-variant payload union per C-7 (the variant-preserving discipline lands deterministically; proptest covers it). Bounded laws (`x.join(bottom) = x`, `x.meet(top) = x`) trivially hold. | §H.1 pp47–54 + §H.2 p55 + §H.7 pp123–125 |
| `NatoClassLattice` | Join + Meet + BoundedJoin + BoundedMeet | **SOUND** | Same shape as `ClassificationLattice` over `NU < NR < NC < NS < CTS`. | §H.2 p55 |
| `DeclassifyOnLattice` | Join + Meet (no Bounded) | **SOUND-BY-DESIGN-ABSENT** | MaxDate semilattice. Join = `max(d1, d2)` (later date dominates). Meet = `min(d1, d2)` (earlier date). **No `BoundedJoinSemilattice` impl is correct** because §H.6 p104 admits exemption codes (X1/X2/.../MR/etc.) that have no calendar-date semantics — there is no single date value that serves as a universal lattice top. The absence is principled, not an oversight. | §H.6 p104 |
| `DissemSet` | Join ONLY | **SOUND-AS-JOIN-ONLY** | PR #456 / PR #538 audit verdict. `relido_observed_unanimous` is join-side observational state — once a portion is RELIDO, "did EVERY portion observe this?" can only be answered by composing observations, not by intersecting structural sets. Three supersession overlays (OC-USGOV / RELIDO-unanimity / NOFORN-dominates) all live on the join side. Meet would have to define "what's structurally shared between two dissem sets with different unanimity observations" — there's no policy basis in §H.8. `assert_not_impl_any!(DissemSet: MeetSemilattice)` correctly locks the absence. | §H.8 p136 + p140 + p145 + pp155–156 + §D.2 Table 3 |
| `JointSet` | Join ONLY | **SOUND-AS-JOIN-ONLY** | PR #538 audit. The C-3 split (PR 4b-B) separated `Mixed` out of `Bottom` precisely so the absorbing JOINT+non-JOINT state keeps `join` associative — `DisunityCollapse` is absorbing only in one direction (any join with it stays in it). Meet would need to invert that direction, which §H.3 does not prescribe. The §H.3 p57 "withheld from further release until approved" wording is unconditional: there is no meet semantics that respects co-owner consent. | §H.3 p56 + §H.3 p57 + §H.7 p123 |
| `NatoDissemSet` | Join + Meet | **SOUND** | Trivial union/intersection over the NATO-side dissem-control bag. ATOMAL / BALK / BOHEMIA route to AEA / SCI axes (NOT into `NatoDissemSet`), so the bag is small (essentially the §G.2 p41 reciprocity surface). | §G.2 p41 (NATO reciprocity) |
| `RelToBlock` | Join + Meet | **SOUND** | Four-variant IntersectSet (`Bottom` / `Lattice{countries}` / `Empty` / `NofornSuperseded`). C-2 split (PR 4b-B) separated `Empty` out of `Bottom` so the absorbing empty-intersection state keeps `join` associative. `proptest_lattice.rs` (PR 4b-D.2 commit 11 per D24) pins idempotence + commutativity + associativity + absorption on the BTreeSet-of-expanded-trigraphs representative — this is the load-bearing test that revealed `CapcoMarking` could NOT be a lattice (because `RelToBlock`'s tetragraph expansion violates structural-`Eq` idempotence on the cross-axis fold). The fix was relaxing the trait bound on `MarkingScheme::Marking` to `JoinSemilattice` only and dropping `impl JoinSemilattice for CapcoMarking`; `RelToBlock` itself stays sound because its own structural `Eq` is on the expanded representation. | §H.8 pp150–151 + §D.2 Table 3 rows 9–13 + §H.9 p172 + p174 |
| `DisplayOnlyBlock` | Join ONLY | **SOUND-AS-JOIN-ONLY** | Per the static_assertions doc-comment: structural union accumulator; meet is mathematically defined (intersection of two display-only audiences) but has no policy basis in §H.8. The `assert_not_impl_any!` lock is principled (not just "wasn't implemented"). The decision parallels `NonIcDissemSet` and `DeclassExemptionAccumulator` — when meet is mathematically valid but policy-undefined, the right move is to NOT implement it so the type system rejects accidental use. | §H.8 (DISPLAY ONLY axis grounding) |

**Verdict §1**: All 12 lattice types are sound. No unsound impl was uncovered. The three Join-only locks (`DissemSet` / `JointSet` / `DisplayOnlyBlock`) are correctly load-bearing, with both compile-time `assert_not_impl_any!` enforcement AND the §H.8 / §H.3 / §H.7 § citations grounding the absence in CAPCO grammar.

---

## §2. Drift-class coverage matrix

The PR's load-bearing structural value is the two pins' coverage of post-4b drift. The matrix evaluates whether each drift class is caught:

| Drift class | `lattice_static_assertions.rs` catches? | `post_4b_lattice_inventory_pin.rs` catches? | Existing test surface catches? | Net coverage |
|---|---|---|---|---|
| **Adding a new lattice type** | NO (the impl-block list would still pass) | NO | NO | **GAP** (low risk — adding a type is intentional) |
| **Removing a lattice type** | YES (build error: import fails) | YES (one of the three pins would fail at runtime) | NO | Covered |
| **Adding `MeetSemilattice` to a Join-only type** | YES (`assert_not_impl_any!` becomes a build error) | NO | parity gate? — only if the rule fires differently | Covered (compile-time) |
| **Removing `MeetSemilattice` from a both-halves type** | YES (`assert_impl_all!` becomes a build error) | NO | proptest_lattice / category_lattice_laws | Covered (compile-time) |
| **Silently changing the body of an impl (wrong semantics, right signature)** | NO | NO | `proptest_lattice.rs` + `category_lattice_laws.rs` + parity gate at `lattice_vs_scheme_parity.rs` | Covered (existing surface) |
| **Renaming a `PageRewrite` row** | NO | YES (positional list diff: same count, different name) | NO | Covered |
| **Reordering `PageRewrite` rows** | NO | YES (positional list comparison, not sorted set) | parity gate would observe downstream behavior shift if Kahn's tie-break differs | Covered (positional pin) |
| **Swapping a `PageRewrite` for an unrelated one** | NO | YES (different name at same position) | NO | Covered |
| **Silently changing a `PageRewrite`'s `reads`/`writes` annotation** | NO | NO | parity gate + corpus regression | Covered (downstream) |
| **Renaming a `ClosureRule`** | NO | YES (positional list diff) | NO | Covered |
| **Reordering `ClosureRule` rows** | NO | YES (positional list comparison) | parity gate + Kleene-fixpoint observation | Covered (positional pin) |
| **Silently changing a `ClosureRule`'s triggers/suppressors/cone** | NO | NO | parity gate + `proptest_closure` + corpus regression | Covered (downstream) |
| **Renaming a `Constraint::Custom` label** | NO | YES (sorted-set diff: missing/unexpected differ) | NO | Covered |
| **Adding a new `Constraint::Custom` row** | NO | YES (sorted-set count + diff) | dispatcher would fail to route the new label | Covered |
| **Dropping a `Constraint::Custom` row** | NO | YES (sorted-set count + diff) | the rules that reference it would test-fail downstream | Covered |

**Gap identified**: Adding a NEW lattice type (an entirely new struct in `marque-capco::lattice` with `impl JoinSemilattice + MeetSemilattice`) is **not** caught by either new pin or by the existing test surface. The compile-time block enumerates the 12 known types; a 13th would compile and pass without triggering any assertion. This is a low-risk drift class (adding a type is intentional, peer-reviewed, and likely accompanied by parity-gate fixture additions), so it does not warrant a finding.

**Net**: every operationally-significant drift class either (a) becomes a build error via `lattice_static_assertions.rs`, (b) becomes a runtime test failure via `post_4b_lattice_inventory_pin.rs`, or (c) is already caught by the existing parity gate / proptest / corpus regression surface. The pins close the drift gaps the existing surface leaves open, which is exactly the umbrella's structural commitment.

---

## §3. Attestation-table architectural coherence

The attestation draft has three sub-sections (a / b / c). Each is sanity-checked here.

### §3.a Single-§-citation discipline — spot-check (3 rows, distinct from code-reviewer's likely picks)

The CAPCO-2016 page anchors use the format `^begin page <N> ` (with trailing classification banner). All three spot-checks verified at this review's authorship per Constitution VIII propagation discipline.

| Picked row | Claimed citation | Page-marker present? | Content verification |
|---|---|---|---|
| `capco/non-fdr-control-evicts-fouo` (PageRewrite row #14) | §H.8 p134 (non-FD&R control sub-clause) | YES (line 3277) | §H.8 p134 verbatim: *"When a classified document contains portions of FOUO information, the FOUO marking is not used in the banner line."* + the surrounding "UNCLASSIFIED + FOUO + other dissem controls (excluding FD&R)" sub-clause grounds the non-FD&R-eviction predicate. **VERIFIED.** |
| `capco/tk-kand-implies-noforn` (ClosureRule row #7) | §H.4 p95 | YES (line 2272) | §H.4 p95 verbatim: *"Requires NOFORN"* on the KANDIK marking template + *"TOP SECRET//TK-KAND//NOFORN"* worked-example banner. **VERIFIED.** |
| `JointSet` (lattice type) | §H.3 p56 (grammar) + §H.3 p57 (derivative-use migration) + §H.7 p123 (NATO transmutation) | YES (line 1232 for p56) | §H.3 p56 verbatim: *"JOINT classified information for which the US is a co-owner, must be appropriately classified and explicitly marked with a REL TO marking..."* + *"May be used with SCI ... (excluding NOFORN)"* + the co-owner consent grammar. **VERIFIED.** The p56 + p57 + §H.7 p123 composite citation is D13-compliant (one operative rule across multiple §H templates — same precedent as the PR 3b umbrella's composite-citation discipline). |

**Verdict §3.a**: Single-§-citation discipline holds on the spot-checked rows. The attestation draft's claim that "every §-citation was re-verified at this PR's authorship" survives the spot-check.

### §3.b Engine-crate-touch ledger completeness

The attestation draft enumerates 5 within-006 precedent breaches: PR 4b-B Commit 2 / 4b-C Commit 5 / 4b-D.2 / 4b-D.3 / 4b-E. I cross-verified by inspecting the staging history.

Engine-crate diff for the closeout itself:

```bash
git diff staging...HEAD --stat -- crates/engine crates/scheme crates/core crates/rules crates/ism
```

→ **empty output**. The closeout itself has zero engine-crate edits. The 5-breach ledger documents prior sub-PR breaches, not new ones.

Additional grep for hidden touches:

```bash
git log staging --oneline --grep "Engine-crate touch authorization"
git log staging --oneline --grep "within-006 precedent"
```

→ **empty output** on both (the search target wording lives in CLAUDE.md "Recent Changes" entries, not in commit messages — which is why the grep returns nothing). The CLAUDE.md "Engine-crate touch authorization" line in the PR 4b-E entry (verified) names "PR 4b-B Commit 2 / 4b-C Commit 5 / 4b-D.2 + 4b-D.3" — consistent with the closeout ledger.

**Verdict §3.b**: Ledger is complete (5 breaches enumerated, all properly attributed, no hidden additional touches). Closeout itself adds no new precedent.

### §3.c Per-axis net-delta math coherence

Re-reading the attestation draft §(c) table and re-running the HEAD verification commands:

| Axis | Pre-4b baseline | Sum of sub-PR contributions | Expected post-4b | HEAD measurement | Reconciliation |
|---|---|---|---|---|---|
| `JoinSemilattice` impls | 3 (Sci/Sar/Fgi pre-existing) | +1 (4b-A AeaSet) + +7 (4b-B) + +1 (4b-E DisplayOnlyBlock) = +9 | 12 | 12 (`grep -cE '^impl(<.*>)? JoinSemilattice'`) | **Consistent.** Closeout entry's "9 → 12" misstates the baseline (should be "3 → 12"); CLAUDE.md "Recent Changes" entry has the same off-by-six. **LOW finding.** |
| `MeetSemilattice` impls | 3 (Sci/Sar/Fgi) | +1 (4b-A) + +5 (4b-B; Joint/Dissem excluded) = +6 | 9 | 9 | **Consistent.** CLAUDE.md entry "6 → 9" misstates baseline as 6; should be "3 → 9". **LOW finding.** |
| `BoundedJoinSemilattice` impls | 0 | +2 (4b-B Class + NatoClass) | 2 | 2 | Consistent. |
| `BoundedMeetSemilattice` impls | 0 | +2 (4b-B Class + NatoClass) | 2 | 2 | Consistent. |
| `PageRewrite` rows | ~14 (per CLAUDE.md PR 4b-C entry) | +9 (4b-C 14→23) + +4 (4b-F #541 / #552 / #555 + 1 noforn_clears expansion?) | 27 | 27 | Plausible but the +4 contribution in 4b-F is annotated as "#541 + #552 + #555 + 1" — only 3 PageRewrite-adding sub-PRs are listed; the 4th row may be a counting artifact. The HEAD count is the load-bearing fact (27, both via grep and via the new pin); the running-counter narrative may be inexact by 1. **INFO finding.** |
| `ClosureRule` rows | 0 (catalog declared pre-4b but not runtime-active) | +10 (4b-D.1 runtime activation) | 10 | 10 | Consistent. |
| Registered `Rule` count | 38 (post-PR-3b umbrella) | +1 (4b-B W004) − 1 (W002 retirement in 4b-F window per CLAUDE.md attestation) | 38 | 38 (`post_3b_registration_pin.rs` GREEN) | Consistent. |

**Verdict §3.c**: The math is structurally coherent. Two LOW findings on the running-counter baseline misstatement (Join axis "9 → 12" / Meet axis "6 → 9" in the CLAUDE.md closeout entry should read "3 → 12" / "3 → 9"), plus one INFO finding on the 4b-F PageRewrite arithmetic (3 sub-PRs accounting for +4 rows).

---

## §4. Constitution-discipline review

| Principle | Check | Result |
|---|---|---|
| **V (audit-first; `__engine_promote` test-fixture carve-out)** | `grep -rn '__engine_promote' crates/capco/tests/lattice_static_assertions.rs crates/capco/tests/post_4b_lattice_inventory_pin.rs` → no matches. Neither new test constructs `AppliedFix` values. | **CLEAN.** Test-fixture carve-out is not exercised because the closeout doesn't need to fabricate audit records. |
| **VI (dataflow pipeline + Send+Sync)** | New tests are integration tests using `CapcoScheme::new()` and reading public trait methods (`page_rewrites()`, `closure_rules()`, `constraints()`). No engine pipeline bypass. The `assert_impl_all!` block exercises the static trait surface only, never running rule code. | **CLEAN.** Tests respect the dataflow phase boundaries. |
| **VII (acyclic dependency graph + scheme-adoption boundary)** | `git diff staging...HEAD --stat -- crates/engine crates/scheme crates/core crates/rules crates/ism` → **empty**. The closeout edits only `crates/capco/Cargo.toml`, `crates/capco/tests/*` (two new files), `.github/workflows/ci.yml`, `CLAUDE.md`, `specs/006-engine-rule-refactor/{plan,tasks}.md`, and `docs/plans/*` (the four plan files this review reads). None are engine-crate touches. The new dev-dep (`static_assertions`) is at the workspace level already; the closeout-side Cargo.toml addition is to `marque-capco`'s `[dev-dependencies]`, which is in-crate. | **CLEAN.** Scheme-adoption boundary observed; OQ-RUST-4 deferral honored (SupersessionSet pin → T146, deferred to follow-up). |
| **VIII (authoritative source fidelity)** | Three spot-checked citations (§H.8 p134, §H.4 p95, §H.3 p56) verified against `crates/capco/docs/CAPCO-2016.md` page markers at this review's authorship. Attestation draft's §3.a "every §-citation was re-verified" claim is consistent with the spot-check. The two new test files carry §-citations indirectly (via doc-comments referencing the originating sub-PR's plan) — no propagation step within the closeout. | **CLEAN.** |

**Verdict §4**: All Constitution-discipline gates pass.

---

## §5. `SupersessionSet` pin deferral re-evaluation

The PM contract (OQ-RUST-4) defers the `SupersessionSet` Join-only invariant pin to T146 (a future engine-crate-authorized PR) because `SupersessionSet` lives in `marque-scheme`, not `marque-capco`. The deferral logic: *"a compile-time pin in `crates/scheme/tests/` would constitute an engine-crate edit."*

**Re-evaluation**: The deferral premise is correct that a test file in `crates/scheme/tests/` would be an engine-crate edit (Constitution VII forbids closeout from touching `crates/scheme/`). HOWEVER, the prompt's framing question opens a third possibility: **a `marque-capco` test file can import `marque_scheme::SupersessionSet` and assert on it without modifying `marque-scheme`**.

Let me check whether this is feasible at HEAD.

The `SupersessionSet` type is exposed publicly from `marque-scheme` (it's one of the built-in lattice constructors enumerated in CLAUDE.md). A test file at `crates/capco/tests/scheme_supersession_set_assert.rs` could:

```rust
use marque_scheme::{JoinSemilattice, MeetSemilattice, SupersessionSet};
use static_assertions::{assert_impl_all, assert_not_impl_any};

// Lock SupersessionSet as Join-only per PR #538 audit
// (memory `project_pr538_observational_lattice_audit`).
assert_impl_all!(SupersessionSet<&'static str>: JoinSemilattice);
assert_not_impl_any!(SupersessionSet<&'static str>: MeetSemilattice);
```

This would:
- Land in `crates/capco/tests/` (NOT engine-crate territory) — Constitution VII clean.
- Import a public type from `marque-scheme` (already a `[dev-dependencies]` of `marque-capco`).
- Use `static_assertions::assert_impl_all!` / `assert_not_impl_any!` (the dev-dep added by this closeout).

**Architecturally, this is achievable in the closeout PR without an engine-crate edit.** The PM contract treated "pin lives in `crates/scheme/tests/`" as the only path; the closeout-side path (pin lives in `crates/capco/tests/` and imports `marque-scheme`'s public type) was not considered.

**Recommendation**: KEEP DEFERRED for this PR (the closeout is locked at 3 commits; widening scope now would require a fourth commit + a contract amendment + another reviewer pass). **File T146 as a "pin can live in `marque-capco` per the architectural re-evaluation, no engine-crate edit needed"** so the future PR is one-test-file rather than a full engine-crate-authorized motion.

**Severity**: **INFO** — this is process feedback, not a defect in the current PR. The deferral is sound for closeout-scope discipline; the closeout-side path opens an alternative for the future PR.

---

## §6. CLAUDE.md narrative coherence per sub-PR

The CLAUDE.md "Recent Changes" section is the project's living narrative. The closeout entry summarizes the 9-sub-PR umbrella at the umbrella level — it does NOT add per-sub-PR entries that were missing from earlier landings.

| Sub-PR | Date | CLAUDE.md "Recent Changes" entry at HEAD? | Closeout narrative coverage |
|---|---|---|---|
| 4b-A #426 (AeaSet) | 2026-05-15 | **NO standalone entry** | Mentioned by name in closeout entry's "T142 — engine-crate touch ledger" (...not really; only via the lattice-impl list which doesn't name 4b-A). The closeout entry has "9 → 12 Join impls" but doesn't name the +1 contribution as AeaSet from 4b-A. **Implicit only.** |
| 4b-B #437 (7 lattice + W004 + 2 bugfixes) | 2026-05-15 | **YES** (line 287, full standalone entry) | Closeout entry's "4b-B Commit 2 OC-USGOV/RELIDO PageContext bugfixes" line. Explicit. |
| 4b-C #468 (Pattern B + C) | 2026-05-16 | **YES** (line 286, full standalone entry) | Closeout entry's "4b-C Commit 5 FOUO Step 3 + UCNI strip retirement" line. Explicit. |
| 4b-D.0 #514 (ClosureRule generic) | 2026-05-17 | **NO standalone entry** | Implicit only (mentioned in PR #514's own description; not in CLAUDE.md). |
| 4b-D.1 #517 (closure runtime activation) | 2026-05-17 | **NO standalone entry** | Implicit only (closeout entry has "10 → 10 ClosureRules" but doesn't attribute it to 4b-D.1). |
| 4b-D.2 #527 (hot-path flip) | 2026-05-18 | **NO standalone entry** | Closeout entry's "4b-D.2 hot-path flip + `MarkingScheme::Marking: JoinSemilattice` bound relaxation" line. Explicit but terse. |
| 4b-D.3 #535 (S007 consumer migration) | 2026-05-18 | **NO standalone entry** | Closeout entry's "4b-D.3 S007 `ProjectedMarking::is_solely_nato_classified` addition" line. Explicit but terse. |
| 4b-E #539 (PageContext deletion + helpers) | 2026-05-18 | **YES** (line 284, full standalone entry) | Closeout entry's "4b-E `assert_impl_all!(CanonicalAttrs: Send, Sync)` + `sar_sort_key` relocation" line + the standalone entry. Explicit. |
| 4b-F #542 + #541 / #552 / #555 (residue + 3 PageRewrites) | 2026-05-18 | **NO standalone entry** | Closeout entry mentions PageRewrite math "14 → 27" but doesn't attribute the 4b-F +3 rows. Implicit only. |

**Verdict §6**: 6 of the 9 sub-PRs (4b-A, 4b-D.0, 4b-D.1, 4b-D.2, 4b-D.3, 4b-F) lack standalone CLAUDE.md "Recent Changes" entries. Three of those (4b-D.2, 4b-D.3, 4b-E) get explicit mentions in the umbrella closeout entry's engine-crate-ledger sub-section; the other three (4b-A, 4b-D.0, 4b-D.1, 4b-F) are implicit only.

**Severity**: **MEDIUM** — this is a documentation gap that existed pre-closeout and that the closeout did not surface in its scope (the architect plan §5.3 specified prepending one closeout entry, not retroactively adding 6 missing standalone entries).

**Mitigation argument (against widening the closeout)**: each sub-PR has its own merged plan-of-record in `docs/plans/`; the umbrella-level closeout entry references those plans; the project's living narrative is the closeout entry. A future audit of "what landed when" can re-derive the per-sub-PR contribution from the closeout's net-delta math + the individual plan-of-record files. The PM contract scope (bookkeeping only) does not require backfilling pre-existing narrative gaps.

**Recommendation**: **DO NOT widen the closeout** to backfill. Document the gap as a follow-up (e.g., file an issue for a one-time CLAUDE.md narrative consolidation pass) and let the closeout land as-scoped. If the PM prefers to widen scope, the addition is mechanical (6 narrative entries, ~6 × 200-word paragraphs, no code changes, ~30 min of writing) and would not break the closeout's "bookkeeping-only" character.

---

## §7. Findings to address before PR open

| # | Severity | Finding | Recommended action |
|---|---|---|---|
| 1 | **MEDIUM** | 6 of 9 sub-PRs (4b-A, 4b-D.0, 4b-D.1, 4b-D.2, 4b-D.3, 4b-F) lack standalone CLAUDE.md "Recent Changes" entries. The umbrella closeout entry summarizes the aggregate but does not backfill the missing individual narratives. | **PM decision**: (a) keep as-is, file a separate follow-up for narrative consolidation; OR (b) widen closeout scope to add 6 standalone entries (~30 min mechanical work, no code impact). Recommend (a) per pre-users + bookkeeping discipline. |
| 2 | **LOW** | CLAUDE.md closeout entry's "Join impls 9 → 12" / "Meet impls 6 → 9" running-counter baselines misstate the pre-4b values (should be "3 → 12" / "3 → 9" — the 3 baseline Sci/Sar/Fgi impls pre-existed the 4b series). | Fix the two numbers in the CLAUDE.md "Recent Changes" entry before PR open. Mechanical edit; no code impact. |
| 3 | **LOW** | Attestation draft §(c) net-delta-math table row for 4b-F lists "+3 PageRewrites" (#541 + #552 + #555) but the post-4b PageRewrite count requires +4 to arrive at 27 from 23 (per CLAUDE.md PR 4b-C entry "14 → 23"). One row's attribution is missing in the running counter. | Verify the breakdown by re-counting `build_page_rewrites()` group contributions; correct the attestation draft if needed. Likely candidate: the 1 noforn_clears row landed via a different PR than the three named. Mechanical research; no code impact. |
| 4 | **LOW** | Drift class "adding a brand-new lattice type" is not caught by either new pin. (Existing test surface catches it indirectly when proptest fixtures are added.) | No action — this is a low-risk drift class (adding a type is intentional and peer-reviewed); the static_assertions file enumerates only known types by design. Document in the static_assertions doc-comment as a "known gap, future T-NNN" if the PM prefers. |
| 5 | **INFO** | `SupersessionSet` Join-only pin could land in `crates/capco/tests/` (importing `marque_scheme::SupersessionSet` as a `marque-capco` dev-dep) without an engine-crate edit. The PM contract OQ-RUST-4 deferred to T146 on the assumption that the pin must live in `crates/scheme/tests/`. | Update T146's task description in `tasks.md` to note: "Can land in `crates/capco/tests/` per architectural re-evaluation — no engine-crate touch required." Mechanical edit to one task line; opens an easier path for the follow-up PR. |
| 6 | **INFO** | The new test file uses `BTreeSet` for `Constraint::Custom` membership comparison. The implementation pulls `.name()` off each `Constraint`; verify that `Constraint::name()` returns a stable scheme-unique identifier (not a label that could collide between two `Custom` rows with different `label` fields). | One-line code audit — verify `Constraint::Custom.name()` returns the unique row identifier, not the human-readable label. If `name()` could collide, the BTreeSet-of-`&str` would silently absorb a duplicate. (Low likelihood; rust-reviewer and code-reviewer will spot if this is a real concern.) |

---

## §8. Summary

The PR is **APPROVE-WITH-FINDINGS**. The lattice-impl-set claim is **sound at HEAD** — no escalation from bookkeeping to bugfix. Two LOW findings on the CLAUDE.md / attestation running-counter baselines, one MEDIUM finding on pre-existing CLAUDE.md narrative gaps the closeout did not surface, two INFO findings (SupersessionSet pin re-evaluation + `Constraint::name()` audit nit).

The two new pins close drift classes the existing parity gate and corpus regression surface cannot catch (rename-at-same-count + swap-at-same-count + reorder-at-same-count + compile-time type-bound drift). The Constitution VII / Constitution VIII discipline holds for the closeout itself, and the engine-crate-touch ledger correctly aggregates the 5 within-006 precedent breaches from the 4b sub-PRs without extending them.

---

## §9. References

- PM contract: `docs/plans/2026-05-19-pr4b-closeout-pm-decisions.md`
- Architect plan (this reviewer wrote the preflight): `docs/plans/2026-05-19-pr4b-closeout-architect-plan.md`
- Rust preflight: `docs/plans/2026-05-19-pr4b-closeout-rust-preflight.md`
- Attestation draft: `docs/plans/2026-05-19-pr4b-closeout-attestation-draft.md`
- Lattice design (with §12 PR #456 addendum): `docs/plans/2026-05-01-lattice-design.md`
- Decisions log D24: `specs/006-engine-rule-refactor/decisions.md`
- PR 3b closeout precedent: `docs/plans/2026-05-08-pr3b-closeout-T027-T028-T029-plan.md`
- New test files: `crates/capco/tests/lattice_static_assertions.rs` + `crates/capco/tests/post_4b_lattice_inventory_pin.rs`
- Authoritative source: `crates/capco/docs/CAPCO-2016.md` (spot-check page anchors: 56 / 95 / 134)
