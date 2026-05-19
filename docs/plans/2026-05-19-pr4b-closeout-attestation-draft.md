<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b Umbrella Closeout — Attestation Draft

**Branch**: `refactor-006-pr-4b-closeout`
**Base**: `staging` (off `5d3415cd`)
**Status**: Draft for PR description body — reviewers may comment inline before PR open.

This document is the operative T142 deliverable: the umbrella attestation
aggregating per-sub-PR Constitution VIII / Constitution VII discipline
claims across the nine 4b sub-PRs. The closeout PR description embeds
sections (a) / (b) / (c) below.

---

## (a) Single-§ citation discipline (Constitution VIII)

Every §-citation listed below was re-verified at this PR's authorship
against `crates/capco/docs/CAPCO-2016.md` page anchors (the
`begin page <N>` markers). Per Constitution VIII, propagation of a
citation across attestation moves requires re-verification — that
discipline was applied here.

### Lattice impls (12 types × 25 trait impls in `crates/capco/src/lattice.rs`)

| Type | Join | Meet | BoundedJoin | BoundedMeet | Sub-PR | §-citation grounding |
|---|:-:|:-:|:-:|:-:|---|---|
| `SciSet` | ✓ | ✓ | — | — | pre-4b (carried over; in scope of the umbrella's PR #456 split) | §H.4 + §A.6 p15 grammar |
| `SarSet` | ✓ | ✓ | — | — | pre-4b | §H.5 pp99–102 |
| `FgiSet` | ✓ | ✓ | — | — | pre-4b + 4b-E `from_attrs_iter` | §H.7 p122 + p123 + p128 |
| `AeaSet` | ✓ | ✓ | — | — | **4b-A #426** | §H.6 pp103–121 + §G.2 Table 5 p40 + §H.7 p122 |
| `ClassificationLattice` | ✓ | ✓ | ✓ | ✓ | **4b-B #437** | §H.1 pp47–54 + §H.2 p55 + §H.7 pp123–125 |
| `NatoClassLattice` | ✓ | ✓ | ✓ | ✓ | **4b-B #437** | §H.2 p55 |
| `DeclassifyOnLattice` | ✓ | ✓ | — | — | **4b-B #437** | §H.6 p104 |
| `NatoDissemSet` | ✓ | ✓ | — | — | **4b-B #437** | §G.2 p41 (NATO reciprocity) |
| `RelToBlock` | ✓ | ✓ | — | — | **4b-B #437** + 4b-D.2 D24 proptest | §H.8 pp150–151 + §D.2 Table 3 rows 9–13 + §H.9 p172 + p174 |
| `DissemSet` | ✓ | — | — | — | **4b-B #437** (Join-only per PR #456) | §H.8 p136 + p140 + p145 + pp155–156 + §D.2 Table 3 |
| `JointSet` | ✓ | — | — | — | **4b-B #437** (Join-only per PR #456) | §H.3 p56 + §H.3 p57 + §H.7 p123 |
| `DisplayOnlyBlock` | ✓ | — | — | — | **4b-E #539** (Join-only per PR #538 audit) | §H.8 (DISPLAY ONLY axis grounding) |

**Aggregator helpers added by 4b-E #539** (NOT lattice types — accumulator
shape; included for inventory completeness):

| Helper | Role | §-citation |
|---|---|---|
| `NonIcDissemSet::from_attrs_iter` | Carries `needs_nf` per non-IC dissem closure → NOFORN | §H.9 p172 + p174 + p178 + p185 |
| `DeclassExemptionAccumulator` | Last-observed declassification exemption (non-commutative; `JoinSemilattice` impl explicitly dropped per `NonIcDissemSet` precedent) | §H.6 (last-observed exemption convention) |

**Total**: 12 `JoinSemilattice` + 9 `MeetSemilattice` + 2 `BoundedJoinSemilattice`
+ 2 `BoundedMeetSemilattice` = **25 trait impls across 12 lattice types**
plus 2 aggregator helpers.

### `PageRewrite` rows (27 total, positional order)

Assembly order in `build_page_rewrites()`:
**pattern_a → pattern_c → pattern_b → supersession → noforn_clears → transmutation_stubs**

| # | Row ID | Group | §-citation |
|---|---|---|---|
| 1 | `capco/nodis-implies-noforn` | pattern_a | §H.9 p174 |
| 2 | `capco/exdis-implies-noforn` | pattern_a | §H.9 p172 |
| 3 | `capco/sbu-nf-implies-noforn` | pattern_a | §H.9 p178 |
| 4 | `capco/les-nf-implies-noforn` | pattern_a | §H.9 p185 |
| 5 | `capco/limdis-evicted-by-classified` | pattern_c | §H.9 p170 |
| 6 | `capco/sbu-evicted-by-classified` | pattern_c | §H.9 p176 |
| 7 | `capco/sbu-nf-evicted-by-classified` | pattern_c | §H.9 p178 |
| 8 | `capco/dod-ucni-promotes-noforn-when-classified` | pattern_c | §H.6 p116 |
| 9 | `capco/dod-ucni-evicted-by-classified` | pattern_c | §H.6 p116 |
| 10 | `capco/doe-ucni-promotes-noforn-when-classified` | pattern_c | §H.6 p118 |
| 11 | `capco/doe-ucni-evicted-by-classified` | pattern_c | §H.6 p118 |
| 12 | `capco/fouo-evicted-by-classified` | pattern_c | §H.8 p134 (classified sub-clause) |
| 13 | `capco/classification-evicts-fouo` | pattern_b | §H.8 p134 (classified document sub-clause) |
| 14 | `capco/non-fdr-control-evicts-fouo` | pattern_b | §H.8 p134 (non-FD&R control sub-clause) |
| 15 | `capco/sbu-nf-supersedes-sbu` | supersession | §H.9 p178 |
| 16 | `capco/les-nf-supersedes-les` | supersession | §H.9 p185 |
| 17 | `capco/noforn-clears-rel-to` | noforn_clears | §H.8 p145 + §D.2 Table 3 |
| 18 | `capco/noforn-clears-fdr-family` | noforn_clears | §H.8 p145 |
| 19 | `capco/noforn-clears-display-only-to` | noforn_clears | §H.8 p145 |
| 20 | `capco/frd-sigma-consolidates-into-rd-sigma` | transmutation_stubs (Phase-3) | §H.6 p108 |
| 21 | `capco/fgi-rollup-on-us-contact` | transmutation_stubs (Phase-3) | §H.7 p122 + p128 |
| 22 | `capco/fgi-restricted-rollup-on-us-contact` | transmutation_stubs (Phase-3) | §H.7 p122 + p128 |
| 23 | `capco/joint-cross-class-rollup` | transmutation_stubs (Phase-3) | §H.3 p57 |
| 24 | `capco/us-presence-promotes-bare-fgi-attribution` | transmutation_stubs (Phase-3) | §H.7 p123 |
| 25 | `capco/orcon-nato-to-us-orcon-on-us-contact` | transmutation_stubs (Phase-3) | §H.8 p136 + §G.2 p41 |
| 26 | `capco/sbu-nf-transmutes-on-classified-contact` | transmutation_stubs (Phase-3) | §H.9 p178 |
| 27 | `capco/les-nf-transmutes-on-classified-contact` | transmutation_stubs (Phase-3) | §H.9 p185 |

**Phase-3 stubs note** (rows 20-27): the 8 `transmutation_stubs.rs`
entries carry `never_fires` / `noop_action` placeholder bodies and are
declared in `build_page_rewrites()` for declaration ordering against
Kahn's algorithm. They are pre-existing Phase B catalog declarations,
not delivered by PR 4b, but are included in the closed structural pin
per OQ-8.

### `ClosureRule` rows (10 total, positional walk order)

Walk order in `CAPCO_CLOSURE_RULES` is load-bearing: per-marking
implication cones populate `working` before RELIDO suppressor checks
fire in the same Kleene iteration (see `closure.rs::CAPCO_CLOSURE_RULES`
doc-comment for the full rationale).

| # | Closure ID | §-citation |
|---|---|---|
| 1 | `capco/noforn-if-caveated` | §B.3 Table 2 p21 (caveated → NOFORN, post 28 June 2010) |
| 2 | `capco/hcs-o-implies-noforn-orcon` | §H.4 p64 |
| 3 | `capco/hcs-p-sub-implies-noforn-orcon` | §H.4 p68 |
| 4 | `capco/si-g-implies-orcon` | §H.4 p80 |
| 5 | `capco/tk-blfh-implies-noforn` | §H.4 p87 |
| 6 | `capco/tk-idit-implies-noforn` | §H.4 p91 |
| 7 | `capco/tk-kand-implies-noforn` | §H.4 p95 |
| 8 | `capco/rel-to-usa-nato-if-nato-classification` | §G.2 p41 + §H.7 p127 (NATO REL TO portion-level) |
| 9 | `capco/relido-if-sci-and-not-incompatible` | §H.8 pp155–156 (RELIDO observed-unanimity on SCI portions) |
| 10 | `capco/relido-if-us-collateral-class` | §H.8 pp155–156 (RELIDO observed-unanimity on US-classified portions) |

### `Constraint::Custom` rows (39 total)

Pinned as a sorted set in T144 since constraint evaluation order is not
engine-observable; only membership matters. Catalog breakdown: 7
core-catalog + 27 class-floor + 5 SCI-per-system. The four RELIDO
E054-E057 rows are `Constraint::Conflicts` (NOT `Custom`) and are
pinned by their own catalog test, not by this closeout's inventory pin.

Per-row §-citations live in each catalog declaration's `label` field
(verified by the `tools/citation-lint/` CI gate). The PR-level
re-verification claim is: every `Constraint::Custom` row in the three
catalog files carries a CAPCO-2016 §X.Y pNN citation in its `label`
field, and the literal page anchor exists in `crates/capco/docs/CAPCO-2016.md`.

### W004 new rule (registered count 38 → 39 in 4b-B; net 38 post-4b-F after W002 retirement)

| Rule ID | Citation |
|---|---|
| W004 `joint-disunity-collapse-to-FGI` | §H.3 p57 + §H.7 p123 (CV-4 PR 4b-B 8th-pass updated from §H.3 p56) |

---

## (b) Engine-crate touch ledger (Constitution VII §IV)

Constitution VII §IV blocks scheme-adoption PRs from editing
`marque-engine` / `marque-scheme` / `marque-core` / `marque-rules` /
`marque-ism`. The 4b umbrella series invoked the within-006 precedent
**five times**. Each is documented here for the closeout reviewer to
audit:

| # | Sub-PR | Commit/Scope | Crate(s) touched | Justification |
|---|---|---|---|---|
| 1 | **4b-B #437** | Commit 2 — OC-USGOV supersession + RELIDO observed-unanimity PageContext bugfixes per §H.8 p136 + p140 + pp155–156 | `marque-ism` (`PageContext::expected_dissem_us`) | Bugfix-class deletions in `marque-ism`; no new scheme adopted. Existing PageContext "unanimity drop" was wrong per §H.8 p136 worked example (ORCON ⊐ ORCON-USGOV). Within-006 precedent. |
| 2 | **4b-C #468** | Commit 5 — retire FOUO Step 3 + UCNI strip branches from `expected_*` accessors | `marque-ism` (`PageContext::expected_dissem_us`, `expected_aea_markings`) | Bugfix-class deletions superseded by declarative Pattern-B / Pattern-C `PageRewrite` rows. PageContext remained transitional banner-validation driver until 4b-D wired `scheme.project(Scope::Page, …)`. Within-006 precedent. |
| 3 | **4b-D.2 #527** | Hot-path flip: `Engine::project` reads `MarkingScheme::project(Scope::Page, …)`; drop `impl JoinSemilattice for CapcoMarking` per Copilot R1 D24; relax `MarkingScheme::Marking: JoinSemilattice` trait bound | `marque-engine` (`engine.rs::project_from_page_context` hot path); `marque-scheme` (`MarkingScheme::Marking` bound; `DiffInput<M>` bound relaxation) | Hot-path flip from `PageContext::expected_*` to `scheme.project(Scope::Page, …)` is the umbrella's load-bearing semantic claim. Trait-bound relaxations were surgical fixes per D24. Within-006 precedent — load-bearing for the umbrella's hot-path commitment. |
| 4 | **4b-D.3 #535** | S007 consumer migration: read `ProjectedMarking::is_solely_nato_classified` instead of `PageContext::is_solely_nato_classified` | `marque-ism` (added `ProjectedMarking::is_solely_nato_classified` accessor) | The S007 rule needed a `ProjectedMarking`-shaped accessor; adding it to `marque-ism` was cleaner than threading a closure through the rule. Within-006 precedent. |
| 5 | **4b-E #539** | `assert_impl_all!(CanonicalAttrs: Send, Sync)` compile-time check + relocate `sar_sort_key` to `crates/ism/src/sar_sort.rs` (T069 readiness) | `marque-ism` (`tests/send_sync.rs`, `src/sar_sort.rs`) | Constitution VI Send+Sync compile-time check on the foundational type (initially targeted `PageContext`; retargeted to `CanonicalAttrs` during PR 4b-E review fix-up; PR 6c / T069 then retired `PageContext` entirely so `CanonicalAttrs` is the surviving target). `sar_sort_key` relocation was T069 readiness work. Within-006 precedent. |

**Total within-006 precedent breaches**: **5**. Each was documented in
the originating sub-PR's "Engine-crate touch authorization" line per the
CLAUDE.md "Recent Changes" record at landing time. The closeout's
contribution is to aggregate the ledger into one reviewable surface.

**Closeout itself is zero engine-crate edits.** The closeout PR is
bookkeeping; per Constitution VII §IV the closeout cannot claim within-006
precedent because it is not scheme-adoption work. The two new test files
land in `crates/capco/tests/`, the new dev-dep lands in
`crates/capco/Cargo.toml`, the new CI job lands in
`.github/workflows/ci.yml`, and the spec-doc / CLAUDE.md edits land in
repo-root / specs/ — none of which is an engine-crate touch.

---

## (c) Per-axis net-delta math (running counter through nine sub-PRs)

| Step | Sub-PR | Join | Meet | BoundedJoin | BoundedMeet | PageRewrite | ClosureRule | Registered Rules |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Pre-4b baseline | — | 3 (Sci/Sar/Fgi) | 3 (Sci/Sar/Fgi) | 0 | 0 | ~14 (pre-Pattern-B/C) | 0 (catalog declared, not runtime-activated) | 38 |
| **4b-A #426** | AeaSet | +1 | +1 | 0 | 0 | 0 | 0 | 0 |
| **4b-B #437** | 7 lattice types + W004 | +7 (Class / NatoClass / Joint / Dissem / NatoDissem / RelToBlock / DeclassifyOn) | +5 (Class / NatoClass / NatoDissem / RelToBlock / DeclassifyOn — Joint + Dissem are Join-only per PR #456) | +2 (Class + NatoClass) | +2 (Class + NatoClass) | 0 | 0 | +1 (W004; 38 → 39) |
| **4b-C #468** | Pattern-B + Pattern-C declarative rows | 0 | 0 | 0 | 0 | +9 (Pattern-B 2 + Pattern-C 7; CLAUDE.md entry: "14 → 23") | 0 | 0 |
| **4b-D.0 #514** | `ClosureRule` generic + `cone_derived` | 0 | 0 | 0 | 0 | 0 | 0 (catalog declared, not activated) | 0 |
| **4b-D.1 #517** | closure operator runtime activation | 0 | 0 | 0 | 0 | 0 | +10 (runtime activation via `CapcoScheme::closure_rules()` override) | 0 |
| **4b-D.2 #527** | hot-path flip + D24 | 0 (dropped `impl JoinSemilattice for CapcoMarking`) | 0 (also dropped `impl MeetSemilattice for CapcoMarking`) | 0 | 0 | 0 | 0 | 0 |
| **4b-D.3 #535** | S007 consumer migration | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| **4b-E #539** | PageContext deletion + helpers | +1 (DisplayOnlyBlock) | 0 | 0 | 0 | 0 | 0 | 0 |
| **4b-F #542 + #541 / #552 / #555** | residue cleanup + 3 PageRewrites | 0 | 0 | 0 | 0 | +4 (`sbu-nf-evicted-by-classified` #541 + `sbu-nf-supersedes-sbu` + `les-nf-supersedes-les` #552/#555 — assembling final 27) | 0 | -1 (W002 retired in PR closing #470 landed in 4b-F window; 39 → 38) |
| **Post-4b-F terminal state** | — | **12** | **9** | **2** | **2** | **27** | **10** | **38** |

**Verification commands** (re-run pre-PR-merge):

```bash
# Lattice impls
grep -cE '^impl(<.*>)? JoinSemilattice for [A-Z]'        crates/capco/src/lattice.rs   # 12
grep -cE '^impl(<.*>)? MeetSemilattice for [A-Z]'        crates/capco/src/lattice.rs   # 9
grep -cE '^impl(<.*>)? BoundedJoinSemilattice for [A-Z]' crates/capco/src/lattice.rs   # 2
grep -cE '^impl(<.*>)? BoundedMeetSemilattice for [A-Z]' crates/capco/src/lattice.rs   # 2

# PageRewrite rows
grep -cE 'PageRewrite::(declarative|custom)' crates/capco/src/scheme/rewrites/*.rs     # 27

# ClosureRule rows
grep -cE '^const CLOSURE_[A-Z_]+: ClosureRule' crates/capco/src/scheme/closure.rs      # 10

# Constraint::Custom rows
grep -cE '^\s+Constraint::Custom \{$' crates/capco/src/scheme/constraints/*.rs         # 39 (7+27+5)

# Registered rule count
cargo +stable test -p marque-capco --test post_3b_registration_pin                     # 38, GREEN

# This PR's new pins
cargo +stable test -p marque-capco --test lattice_static_assertions                    # compile-time only
cargo +stable test -p marque-capco --test post_4b_lattice_inventory_pin                # 3 tests GREEN
```

---

## Closeout PR-level discipline

- **Zero rule-logic edits.** All changes are bookkeeping: two new test
  files, one new dev-dep, one new CI job, three spec-doc / CLAUDE.md
  edits.
- **Zero engine-crate edits.** Constitution VII §IV scheme-adoption
  boundary observed for the closeout itself.
- **Single-§ citation discipline.** Every §-citation in this attestation
  was re-verified at this PR's authorship against
  `crates/capco/docs/CAPCO-2016.md` page anchors per Constitution VIII.
- **Pre-users.** No deprecation phasing; no alias maps; no schema bumps
  for back-compat (`feedback_pre_users_no_deprecation_phasing`).
- **Symbolic refs.** Commit messages and PR description use function /
  section / test names; not file:line (`feedback_avoid_line_number_anchoring`).
  CAPCO §-citations with `pNN` are the canonical exception.

---

## References

- PM contract: `docs/plans/2026-05-19-pr4b-closeout-pm-decisions.md`
- Architect plan: `docs/plans/2026-05-19-pr4b-closeout-architect-plan.md`
- Rust preflight: `docs/plans/2026-05-19-pr4b-closeout-rust-preflight.md`
- PR 3b umbrella precedent: `docs/plans/2026-05-08-pr3b-closeout-T027-T028-T029-plan.md`
