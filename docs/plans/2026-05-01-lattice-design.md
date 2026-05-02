<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Lattice design — per-category join semantics

**Date:** 2026-05-01
**Status:** stub — gates PR 4 of `2026-05-01-engine-rule-architecture-refactor.md`
**Acceptance:** This document must be filled in and reviewed before
PR 4 implementation lands. Each category section below requires:

1. §-citations to `crates/capco/docs/CAPCO-2016.md` covering the
   marking grammar and any commingling/dominance/supersession rules.
2. Formal join semantics — how two values in the category combine,
   stated as a function with preconditions and postconditions.
3. Worked examples showing two non-trivial values joining and the
   result, including any edge cases the §-citation calls out.
4. Property-test fixtures (named test cases) covering associativity,
   commutativity, idempotency, identity-with-bottom for the category.

**Related docs:**
- `2026-05-01-engine-rule-architecture-refactor.md` (drives this gate)
- `2026-04-19-recursive-lattice-and-decoder.md` §3 (existing
  `SciSet`/`SarSet`/`FgiSet` lattice work; equal-depth meet policy)
- `2026-04-17-marking-scheme-lattice-design.md` §0–2 (Phase A
  scaffolding; problem statement)

---

## 0. Why this gate exists

The Phase B work (`marque-scheme`, `Lattice`, `BoundedLattice`,
`Constraint`, `Scope`, `PageRewrite`, topo scheduler, built-in
constructors) shipped the trait surface but not the per-category
math. `CapcoMarking::join` at `crates/capco/src/scheme.rs:188-247`
delegates to `PageContext` instead of doing component-wise lattice
joins per category. The Phase B doc was honest about the caveat
(§3.3 of `2026-04-19-recursive-lattice-and-decoder.md`); the math was
deferred.

The previous attempt at lattice unification (the 2026-04-17 doc and
the surrounding work) skimmed the complexity and ended up bandaged
until it became almost unused. The user's explicit concern
(2026-05-01 conversation): *"the lattice plan lacked full appreciation
for the complexity and intricacies of the system so it was bandaged
until it became almost unused."*

This doc forces the per-category math to be done up front, with
§-citations, before PR 4's `Lattice` impls land. If a category's
section here is not filled in, PR 4 cannot land that category's
property tests.

The acceptance criteria are not optional. A skim that fills in
sections without working through edge cases reproduces the failure
mode this gate is designed to prevent.

---

## 1. Categories requiring lattice impls

Per `CapcoScheme::categories()`:

| Category | Existing impl | Phase B status |
|---|---|---|
| `MarkingClassification` | `OrdMax` over US levels | Insufficient: no NATO/FGI/JOINT branches |
| Dissem set | `FlatSet`-ish via `IsmAttributes.dissem` | Insufficient: supersession rules not encoded |
| `SciSet` | `marque-capco::lattice::SciSet` | Functional; equal-depth meet policy from #69 |
| `SarSet` | `marque-capco::lattice::SarSet` | Functional |
| `FgiSet` | `marque-capco::lattice::FgiSet` | Functional but render-canonical drops redundant `FGI` token (PR 5) |
| NATO control set | None | New: ATOMAL/BOHEMIA additions in PR 9 (#246) |
| Declassify-on / `MaxDate` | `MaxDate` | Functional for dates; AEA/NATO canned strings (#266) deferred |

Each section below is a stub. **A reviewer signing off on PR 4 is
signing off that every section in this document carries the four
acceptance items.**

---

## 2. `MarkingClassification`

### Required content

- §-citations: §A.4 (classification levels), §H.7 (FGI), §H.3 (NATO),
  §H.6 (AEA — out of scope for join math but cite for completeness),
  §A.6 (banner roll-up rule).
- Join semantics: partial order with branches. US levels
  (`Unclassified < Confidential < Secret < TopSecret`) form one
  chain. NATO levels form another. FGI carries country trigraphs as
  a separate axis. JOINT spans multiple producers.
- Worked example for the #276 case: `(C//FGI DEU)` and `(S)` portions
  on the same page roll up to a banner classification that *retains*
  the FGI marker — the join can't flatten to `Us`.
- Property fixtures: assoc/comm/idem on US chain; identity-with-bottom
  (`Unclassified` is bottom for US chain); cross-branch joins
  (US ⊔ NATO, US ⊔ FGI, etc.) need explicit semantics.

### Open questions

1. Is the cross-branch join a refusal (returns `Top` / `Error`) or
   a structural combination (`MarkingClassification::Joint`)?
2. How does the join behave with classification-equivalent levels
   from different schemes (US Secret vs NATO Secret — same level
   number, different scheme)?
3. What does `expected_classification()` return when the page contains
   only foreign portions (#276)? `Option<MarkingClassification>`
   resolves the type, but the value semantics — what's the right
   representation for "this page is purely FGI" — needs §-citation.

---

## 3. Dissem set

### Required content

- §-citations: §H.8 (dissem-control markings, FD&R/non-FD&R
  distinction), §F (legacy/deprecated), §H.9 (NODIS/EXDIS).
- Join semantics: union with supersession. `SupersessionSet` from
  `marque-scheme` covers the surface; the per-token rules need
  enumeration. Examples:
  - `NF` (NOFORN) supersedes `REL TO` (NOFORN dominates).
  - `OC-USGOV` supersedes `OC` (USGOV is more restrictive).
  - `NODIS` supersedes `EXDIS` (per §H.9 p174).
  - Non-FD&R supersedes `FOUO` (per Appendix A of refactor doc).
- FOUO sub-section: two eviction axes (classification > U;
  any non-FD&R dissem). Cross-category constraint for the first;
  in-category supersession for the second. Worked fixtures from
  refactor doc Appendix A.
- `is_fdr_dissem` per-token metadata: enumerate the FD&R set
  (`REL TO`, `RELIDO`, `NOFORN`, `DISPLAY ONLY`, `EYES`-deprecated)
  and assert all others as non-FD&R.

### Open questions

1. Does `EYES` (deprecated → maps to `REL TO`) participate in the
   lattice as `EYES` or as the post-migration `REL TO`? Migration
   semantics intersect with lattice semantics here.
2. `REL TO` carries trigraph payload; supersession of `REL TO USA, FVEY`
   by `REL TO USA, FVEY, GBR` is set-extension not supersession.
   The lattice surface needs to distinguish "supersedes the token"
   from "extends the token's payload."
3. `NF` clears `REL TO` per §H.8 (it dominates). Existing
   `capco/noforn-clears-rel-to` `PageRewrite` (per CLAUDE.md) is the
   first declared rewrite. Does the in-category lattice handle this,
   or does the rewrite step before the lattice projection?

---

## 4. `SciSet`

### Required content

- §-citations: §A.6 (SCI grammar: control + compartment + sub-comp),
  §H.4 (per-system constraints).
- Join semantics: existing equal-depth meet policy from
  `marque-capco::lattice::SciSet` (#69). Document the policy
  explicitly here, not just by reference. Why "equal-depth" — what
  goes wrong with depth-mismatched joins?
- Worked example: `SI-G ABCD` ⊔ `SI-G DEFG` = `SI-G ABCD DEFG` (same
  control, same compartment, different sub-comps union).
- Property fixtures: assoc/comm/idem; bottom is `SciSet::empty()`.
  No `BoundedLattice` impl (control systems are open-vocabulary,
  no lawful finite top).

### Open questions

1. Equal-depth policy: cite the §A.6 passage that motivates it. The
   policy is plausible from grammar but the §-citation pins it.
2. Per-system canonicalization (HCS-O, HCS-O-P, HCS-P sub-comps,
   SI-G, SI-G sub-comps) interacts with #267 Gap A's companion-insert
   path. Does the lattice impl cover this, or does the
   `MarkingScheme::render_canonical` site after lattice projection?

---

## 5. `SarSet`

### Required content

- §-citations: §H.5 (SAR grammar; programs, compartments, sub-comps;
  banner roll-up).
- Join semantics: existing impl from `marque-capco::lattice::SarSet`.
  Document the policy.
- Worked example: program-level join, compartment-level join,
  sub-comp-level join.
- Property fixtures: bottom is `SarSet::empty()`. No `BoundedLattice`
  impl (SAR identifiers are agency-extensible open set).

### Open questions

1. SAR ordering canonicalization (alphabetic vs source order vs
   §H.5 prescribed ordering) — does the lattice produce canonical
   ordering or does `render_canonical` impose it?
2. NF requirement on SAR per §H.5 — handled by lattice or by
   declarative `Constraint`?

---

## 6. `FgiSet`

### Required content

- §-citations: §H.7 (FGI grammar; trigraphs, source-acknowledged vs
  source-concealed semantics; tetragraph aggregation).
- Join semantics: existing impl. Document the source-acknowledged /
  source-concealed distinction (§H.7 p126 — the bug in #280).
  `FgiMarker { countries: [] }` is *source-concealed FGI*, lawful
  per §H.7; `FgiMarker` after parse-failure is *parser corruption*.
  These collide on the same shape today; PR 2 returns `None` for
  the latter so the lattice never sees corrupted shape.
- Worked example: `FGI DEU` ⊔ `FGI FRA` = `FGI DEU FRA`; redundant
  `FGI` token elision in render-canonical (#261, PR 5).
- Property fixtures: bottom is `FgiSet::empty()`.

### Open questions

1. Tetragraph expansion (FVEY → individual trigraphs) interaction
   with lattice join. The `marque-capco::vocab` tetragraph tables
   (per CLAUDE.md) feed render-canonical. Does the lattice operate
   on expanded trigraphs or on tetragraph atoms?
2. `BUILTIN_TETRAGRAPH_MEMBERS` from the 2026-04-28 ISMCAT taxonomy
   plan informs which tetragraphs are decomposable. The lattice
   shouldn't decompose at join time — it should operate on canonical
   form (which `2026-04-28-tetragraph-taxonomy-and-uncertain-reduction.md`
   addressed for REL TO).

---

## 7. NATO control set

### Required content

- §-citations: §H.3 (NATO classification + controls; ATOMAL,
  BOHEMIA, others).
- Join semantics: NEW work (#246). Existing impl is none. ATOMAL
  and BOHEMIA are standalone control categories per §H.3.
- Worked example: `NATO SECRET ATOMAL` ⊔ `NATO SECRET BOHEMIA` —
  what's the result? Are these controls combinable?
- Property fixtures: TBD; depends on whether NATO controls form
  a meaningful lattice or are constrained by §H.3 to specific
  classifications.

### Open questions

1. Are ATOMAL and BOHEMIA combinable in a single marking? §H.3
   citation pins this.
2. NATO portion in US doc → REL TO USA, NATO derivation (#265).
   This is a declarative `Constraint`, not a lattice rule, but
   the lattice has to admit the NATO portion shape that triggers
   the constraint.
3. US dissem in non-US (NATO) marking (#246 part 2). PR 9's 7B
   `dissem_us`/`dissem_nato` position-attributed parser fields
   close the parser side; what does the lattice say about
   "NATO marking carrying US dissems"?

---

## 8. Declassify-on / `MaxDate`

### Required content

- §-citations: §C (declassification authority block), §C.4
  (AEA exemption canned strings; #266 deferred), §C.5 (NATO
  commingling canned strings; #266 deferred).
- Join semantics: `MaxDate` for dates (latest declass date wins on
  page roll-up). For canned strings (`50X1-HUM`, etc.) the join is
  not date-comparison.
- Worked example: page with portions A (`Declassify On: 2050-01-01`)
  and B (`Declassify On: 2055-06-15`) rolls up to banner
  `Declassify On: 2055-06-15`.
- Property fixtures: assoc/comm/idem on `MaxDate`; identity is
  the earliest representable date (or `Option::None` if no
  declass-on present).

### Open questions

1. Mixed dates and canned strings on the same page: what's the
   join? §C citation pins this.
2. AEA exemption strings (`50X1-HUM`, etc.) supersession ordering —
   #266 deferred, but the lattice surface has to admit the canned-
   string shape now or the deferral becomes a rewrite later.

---

## 9. Acceptance checklist

Before PR 4 lands, this document must satisfy:

- [ ] Every category section (§§2–8) has §-citations to
      `crates/capco/docs/CAPCO-2016.md`.
- [ ] Every category section has formal join semantics stated as a
      function with preconditions/postconditions, not prose.
- [ ] Every category section has at least two worked examples,
      including edge cases the §-citation calls out.
- [ ] Every category section names property-test fixtures by file
      and test name.
- [ ] Every "Open question" is either resolved (§-citation +
      explicit decision) or explicitly deferred to a tracked issue.
- [ ] FD&R semantics from refactor doc Appendix A are embedded in
      §3 (Dissem set) with §-citation to §H.8.
- [ ] Reviewer (named in PR description) has confirmed each
      category's worked examples by hand against the §-citation.

---

## 10. Open items requiring author input before fill-in

Items where the doc author needs the user (or a domain reviewer)
to make a call before the section can be filled:

1. **§2 cross-branch join semantics**: refusal vs structural combination.
2. **§3 `EYES` lattice participation**: pre- or post-migration form.
3. **§3 `NF` clears `REL TO`**: lattice op vs `PageRewrite`.
4. **§4 SCI per-system canonicalization**: lattice vs `render_canonical` boundary.
5. **§5 SAR ordering**: lattice canonical vs render canonical.
6. **§6 tetragraph join level**: trigraph-expanded vs tetragraph-atomic.
7. **§7 ATOMAL/BOHEMIA combinability**: §H.3 citation needed.
8. **§8 mixed dates and canned strings**: §C citation needed.

These are the points where the previous attempt skimmed. The fill-in
pass must resolve each before PR 4 implementation begins.
