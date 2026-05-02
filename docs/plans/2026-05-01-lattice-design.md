<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Lattice design — per-category join semantics

**Date:** 2026-05-01
**Status:** stub — gates PR 4 of `2026-05-02-engine-refactor-consolidated.md`
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
5. **Cross-axis dominance fixtures** (added 2026-05-02 per
   `2026-05-02-engine-refactor-consolidated.md` §11.1) where the
   category's values interact with another category's dominance:
   FOUO eviction by classification > U AND by non-FD&R dissem; FGI
   banner roll-up #276; SCI cross-system canonicalization; AEA
   exemption commingling with classification. The §9 acceptance
   checklist enumerates each fixture; PR 3.7 fill-in places each
   one in its primary category section (§§2–8) so a reviewer
   verifying coverage category-by-category sees the fixtures
   alongside the rest of the category's deliverables, not in a
   separate global pile.

**Related docs:**
- `2026-05-02-engine-refactor-consolidated.md` (drives this gate; supersedes the deleted 2026-05-01 draft)
- `2026-04-19-recursive-lattice-and-decoder.md` §3 (existing
  `SciSet`/`SarSet`/`FgiSet` lattice work; equal-depth meet policy)
- `2026-04-17-marking-scheme-lattice-design.md` §0–2 (Phase A
  scaffolding; problem statement)

---

## 0a. Primary and secondary actors

The doc serves three actors with different needs; design choices
favor the primary.

- **Primary**: a future rule author writing a new lattice predicate
  (e.g., for a new dissem token, a new SCI sub-comp shape, a new
  cross-axis dominance constraint). The primary actor needs each
  category section to read as a working reference: formal join
  semantics, worked examples covering edge cases the citation
  calls out, fixture names that demonstrate the laws, and an
  algorithmic sketch when the join is non-trivial. The §3
  Resolution 2026-05-02 shape is what serves this actor — every
  other §§2–8 section reaches that density before PR 4 can land.
- **Secondary — PR 4 reviewer.** Needs the §9 acceptance checklist
  + per-category review template (`crates/capco/tests/lattice/REVIEW_TEMPLATE.md`)
  + the four cross-axis adversarial fixture packs (authored at PR 3.7) to
  sign off PR 4 with structural confidence rather than tired-
  reviewer pattern matching.
- **Secondary — future scheme implementer** (`marque-cui`,
  partner-national schemes, etc.). Reads to understand which
  lattice constructors apply to which axis shapes, where cross-
  axis dominance lives, what shape a `Constraint` vs.
  `PageRewrite` takes. CAPCO-first means this actor is documented
  but not load-bearing for this doc; the trait surface is
  semver-unstable until scheme #2 arrives (per `2026-05-02-engine-
  refactor-consolidated.md` §3.10).

When the three actors' needs conflict, the primary actor wins.

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
  distinction), §F (legacy/deprecated), §H.9 (NODIS/EXDIS),
  §H.7 + §D.2 Table 3 row 23 (tetragraph expansion in common-LIST
  roll-up), §H.8 p157 (EYES deprecation; 1 Oct 2017 waiver expiry).
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

### Resolution (2026-05-02): dissem axis is a product

The dissem axis is **not** a flat `SupersessionSet<DissemTokenId>`
with token-level country-list payloads. It is a **product of typed
axes**, matching the field layout `IsmAttributes` already carries
(`crates/ism/src/attrs.rs:55-122`):

```
DissemAxis ≅
    SupersessionSet<DissemTokenId>      // {NF, OC, OC-USGOV, IMC, PROPIN,
                                        //  FOUO, RELIDO, FISA, DSEN, RSEN, ...}
  × IntersectSet<CountryCode>           // rel_to (existing field)
  × IntersectSet<CountryCode>           // display_only (NEW field — PR 4)
  × IntersectSet<Trigraph>              // eyes (NEW field — PR 4; deprecated;
                                        //   restricted to Five-Eyes members)
  × SupersessionSet<NonIcDissemTokenId> // {NODIS, EXDIS, SBU, SBU-NF,
                                        //  LES, LES-NF, SSI, LIMDIS}
```

**Why `IntersectSet` for country lists.** The country-list axes
order by *restrictiveness*: a smaller country list is more
restrictive (releasable to fewer parties is the tighter
constraint). Banner roll-up takes the most-restrictive consensus,
which is the **intersection** across portions. `Lattice::join` on
these axes is therefore set intersection, not union. This matches
what `PageContext::expected_rel_to`
(`crates/ism/src/page_context.rs:482`) already computes
imperatively today; PR 4 promotes the math from imperative to
lattice-owned. `IntersectSet` from `marque-scheme` is the
constructor.

**Tetragraph / group-code expansion at insert; re-fold at render.**
The country-list axes operate on **expanded trigraph atoms**.
Group designations (FVEY, ACGU, TEYE, NATO, EU, AUSTRALIA_GROUP,
…) decompose to their member trigraphs at insert time using the
membership tables in `marque-capco::vocab` (see CLAUDE.md
"tetragraph expansion tables"). `MarkingScheme::render_canonical`
re-folds greedily by descending membership size: when the
post-intersection trigraph set fully contains a decomposable
group's membership, render emits the group form. **Authority:
§D.2 Table 3 row 23.**

Group designations split into two classes per ISMCAT:
- **Decomposable** — known trigraph membership; fold-eligible
  (FVEY, ACGU, TEYE, NATO, EU, AUSTRALIA_GROUP, ...).
- **Opaque** — operation-specific or shape-extensible codes
  (KFOR, ISAF, RSMA, NATO operation codes, ...); pass through as
  atoms, never folded into. Survive intersection only when present
  in every portion's list.

Membership tables are **schema-pinned** per Constitution Principle
IV. ODNI ISM-v2022-DEC's NATO membership is what governs the fold,
not contemporary real-world NATO. Schema bump = membership change
= deliberate migration, never silent.

**Country sort key (single source of truth).** `render_canonical`
and the `Phase::WholeMarking` Register-order validator (Gap C, PR
7) both consume:

```rust
fn country_sort_key(c: &CountryCode) -> (u8, u8, &str) {
    if c.as_str() == "USA" { return (0, 0, "USA"); }   // §H.8: USA first
    let kind_rank = match c.kind() {
        CountryKind::Country => 1,                     // trigraphs
        CountryKind::Group   => 2,                     // group designations
    };
    (1, kind_rank, c.as_str())
}
```

`CountryCode::kind()` is a method on `CountryCode` consulting a
build-time-generated `phf::Set<&'static str>` (`KNOWN_GROUP_CODES`)
from the ISMCAT XML. **Length-based detection is wrong** — group
designations include 2-char codes (EU) and longer (AUSTRALIA_GROUP)
as well as 4-char codes (FVEY, NATO). Unknown codes default to
`Country` (open-vocabulary fallback).

**EYES participates as its own axis pre-migration.** EYES is
encoded as a token in `dissem_controls` plus a separate
`eyes: Box<[Trigraph]>` field on `IsmAttributes` (parallel to
`rel_to`, with the type system enforcing the Five-Eyes-trigraph
restriction per §H.8 p157). The deprecation does **not** happen at
parse time — that would lose audit provenance.

A `Phase::Localized` rule `S006 eyes-migrates-to-rel-to` fires on
EYES token presence. Confidence = 1.0 (the 1 Oct 2017 markings
waiver is expired; the deprecation is unambiguous). Citation
§H.8 p157. The fix proposal: drop `EYES` from `dissem_controls`,
clear the `eyes` axis, add `REL TO` to `dissem_controls`, copy the
trigraphs into `rel_to`. The delimiter normalization (`/` → `, `)
falls out of `render_canonical`. `Engine::fix_inner` promotes the
proposal to `AppliedFix` with `original = "USA/CAN/GBR EYES ONLY"`,
`replacement = "REL TO USA, CAN, GBR"`, classifier ID, timestamp —
**the audit record is the historical trail**.

Under `--preserve-historical-form` mode (consolidated plan PR 9),
the rule's severity flips to diagnostic-only; the EYES axis stays
populated; the lattice projection sees EYES as an independent axis
with its own `IntersectSet`. Cross-axis constraints (`EYES`
conflicts with `NOFORN` and `REL TO`, allowed with `RELIDO`) land
as declarative `Constraint`s on `CapcoScheme`.

### Open questions

1. ~~Does `EYES` (deprecated → maps to `REL TO`) participate in the
   lattice as `EYES` or as the post-migration `REL TO`?~~
   **Resolved (2026-05-02):** EYES participates as its own typed
   `IntersectSet<Trigraph>` axis pre-migration. A `Phase::Localized`
   rule `S006 eyes-migrates-to-rel-to` emits a 1.0-confidence
   `FixProposal`; the resulting `AppliedFix` is the audit trail.
   `--preserve-historical-form` flips the rule to diagnostic-only,
   and the lattice operates on the EYES axis directly. See
   "Resolution" block above and the §S006 entry the PR 3.7 fill-in
   adds to `crates/capco/src/rules.rs`.
2. ~~`REL TO` carries trigraph payload; supersession of
   `REL TO USA, FVEY` by `REL TO USA, FVEY, GBR` is set-extension
   not supersession. The lattice surface needs to distinguish
   "supersedes the token" from "extends the token's payload."~~
   **Resolved (2026-05-02):** the framing was wrong. REL TO is not
   a token-with-payload in `dissem_controls`; it is a separate
   `IntersectSet<CountryCode>` axis indexed by REL TO's presence in
   the flat dissem set. There is no element-level payload to
   extend, so the supersession-vs-extension dichotomy does not
   arise. See "Resolution" block above for the product shape.
3. ~~`NF` clears `REL TO` per §H.8 (it dominates). Does the in-category
   lattice handle this, or does the rewrite step before the lattice
   projection?~~ **Resolved (2026-05-02): confirm-and-document, not
   decide.** `capco/noforn-clears-rel-to` is already a declared
   `PageRewrite` and runs *after* projection inside
   `CapcoScheme::project(Scope::Page, ...)` (see
   `crates/capco/src/scheme.rs::project` body). The PR 3.7 fill-in for
   §3 documents this dispatch shape with the §H.8 §-citation; the lattice
   itself does not encode the supersession.

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

1. ~~Tetragraph expansion (FVEY → individual trigraphs) interaction
   with lattice join. The `marque-capco::vocab` tetragraph tables
   (per CLAUDE.md) feed render-canonical. Does the lattice operate
   on expanded trigraphs or on tetragraph atoms?~~
   **Resolved (2026-05-02): operate on expanded trigraphs.** Same
   resolution as §3's REL TO/DISPLAY ONLY/EYES axes — see §3
   "Resolution (2026-05-02): dissem axis is a product" for the
   algorithm (greedy size-descending re-fold of decomposable
   groups; opaque codes pass through; schema-pinned membership
   tables; `country_sort_key` shared with `render_canonical` and
   the Register-order validator). FGI LIST uses the same sort key
   and the same fold algorithm.
2. ~~`BUILTIN_TETRAGRAPH_MEMBERS` from the 2026-04-28 ISMCAT taxonomy
   plan informs which tetragraphs are decomposable. The lattice
   shouldn't decompose at join time — it should operate on canonical
   form (which `2026-04-28-tetragraph-taxonomy-and-uncertain-reduction.md`
   addressed for REL TO).~~
   **Resolved (2026-05-02): folded into Q1's resolution above.**
   The lattice operates on expanded trigraph atoms (decomposing at
   insert time); `render_canonical` re-folds. The 2026-04-28
   plan's `BUILTIN_TETRAGRAPH_MEMBERS` decomposable/opaque
   distinction governs which group codes are fold-eligible.

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

Before PR 4 lands, this document must satisfy the criteria below.
**The bar for §§2–8 density is the §3 Resolution 2026-05-02 shape**;
see `2026-05-02-engine-refactor-consolidated.md` §11.1 for the
canonical specification (formal join + algorithmic detail when non-
trivial + worked examples + property-fixture names + cross-axis
fixtures + open-question resolution). Reviewer signs off via the
per-category template at `crates/capco/tests/lattice/REVIEW_TEMPLATE.md`,
one filled template per category, including the adversarial-fixture
run (sign-off is "I ran the four cross-axis adversarial packs and
they failed against the synthetic violations as expected," not "I
read the section").

- [ ] Every category section (§§2–8) has §-citations to
      `crates/capco/docs/CAPCO-2016.md`.
- [ ] Every category section has formal join semantics stated as a
      function with preconditions/postconditions, not prose.
- [ ] Every category section has at least two worked examples,
      including edge cases the §-citation calls out.
- [ ] Every category section names property-test fixtures by file
      and test name.
- [ ] Every "Open question" listed under §10 (Open items requiring
      author input before fill-in) is resolved (§-citation + explicit
      decision). **No "explicitly deferred to a tracked issue" escape
      valve for §10 items** — per `2026-05-02-engine-refactor-consolidated.md`
      §11. A §10 question that genuinely cannot resolve blocks PR 4.
      Pre-existing in-text deferrals to tracked issues that name a
      specific scope-cut (e.g., §8's `#266 deferred` for AEA / NATO
      canned-string supersession ordering) are NOT subject to this
      rule — they are documented scope cuts, not unresolved gate
      questions, and were already accepted before the consolidated
      plan landed. PR 3.7 fill-in confirms each such scope cut
      remains intentional and updates §8 / §10 to make the
      distinction explicit (scope cut vs. gate question).
- [ ] Cross-axis dominance fixtures are present in their primary
      category sections (per consolidated plan §11.1 item (5)). The
      mapping (PR 3.7 fill-in places fixtures in the named section;
      additional secondary touchpoints land as cross-references):
      - §2 (`MarkingClassification`) — **FOUO eviction by
        classification > U** (cross-touchpoint to §3); **FGI banner
        roll-up #276** (cross-touchpoint to §6 `FgiSet`).
      - §3 (Dissem set) — **FOUO eviction by non-FD&R dissem**
        (in-category supersession; cross-touchpoint to §2 for the
        eviction direction). **REL TO / DISPLAY ONLY / EYES axis
        fixtures** (per §3 Resolution 2026-05-02):
        `rel_to_ordering_basic.json` (trigraph-before-group sort);
        `rel_to_ordering_authored_wrong.json` (Register-order
        validator catches author-introduced wrong order);
        `rel_to_full_fvey_fold.json` (full-membership fold);
        `rel_to_acgu_subset_fold.json` (greedy picks largest
        available — FVEY can't fold without NZL, ACGU folds);
        `rel_to_acgu_with_leftover.json` (group plus leftover
        trigraph, sort order); `rel_to_full_nato_fold.json`
        (schema-pinned NATO membership; fixture regenerates on
        schema bump); `rel_to_eu_fold.json` (2-char group code,
        confirms `kind`-based sort not length-based);
        `rel_to_australia_group_fold.json` (longer group code);
        `rel_to_eyes_migrates.json` (EYES → REL TO migration with
        `AppliedFix` audit record); `rel_to_eyes_preserve_historical.json`
        (`--preserve-historical-form` mode flips rule to
        diagnostic-only).
      - §4 (`SciSet`) — **SCI cross-system canonicalization**
        (HCS-O / HCS-O-P / SI-G interactions; #267 Gap A).
      - §5 (`SarSet`) — no primary cross-axis fixture today; revisit
        if SAR ordering interacts with classification or dissem
        beyond what §H.5 already enumerates.
      - §6 (`FgiSet`) — **FGI banner roll-up #276** (primary site;
        §2 cross-reference). FGI LIST tetragraph fold reuses the
        algorithm from §3; one fixture verifies parity:
        `fgi_list_fold_parity.json`.
      - §7 (NATO control set) — no primary cross-axis fixture in
        this round; deferred until ATOMAL/BOHEMIA work in PR 9.
      - §8 (Declassify-on / `MaxDate`) — **AEA exemption
        commingling with classification** (cross-touchpoint to §2
        for the classification axis).
      A fixture appears once (in its primary section); secondary
      touchpoints carry a one-line cross-reference, not a duplicate.
- [ ] FD&R semantics from refactor doc Appendix A are embedded in
      §3 (Dissem set) with §-citation to §H.8.
- [ ] Reviewer (named in PR description) has confirmed each
      category's worked examples by hand against the §-citation.

---

## 10. Open items requiring author input before fill-in

Items where the doc author needs the user (or a domain reviewer)
to make a call before the section can be filled. Each item carries
a `Status:` tag making the deferral kind explicit:

- `Status: open_gate (<criteria_to_close>)` — must resolve before
  PR 4 lands. PR 3.7 fill-in closes with §-citation + explicit
  decision. Failure to close blocks PR 4.
- `Status: scope_cut (<reason>)` — accepted scope cut, intentional;
  does not block PR 4. The "no escape valve" rule does not apply
  to these.
- `Status: resolved (YYYY-MM-DD)` — closed in this revision; left
  in the doc for genealogy.

Citation-lint at PR 0.5 flags any deferral phrasing in this section
that lacks one of the three tag forms (per consolidated plan §11.2).

1. **§2 cross-branch join semantics**: refusal vs structural combination.
   `Status: open_gate (decide between Top/Error refusal vs.
   MarkingClassification::Joint structural combination, with §A.4
   / §H.3 / §H.7 §-citation; ship worked examples for each branch
   pair the decision admits)`.
2. ~~**§3 `EYES` lattice participation**: pre- or post-migration form.~~
   `Status: resolved (2026-05-02)` — see §3 Resolution block: EYES
   is its own typed `IntersectSet<Trigraph>` axis pre-migration;
   the `eyes-migrates-to-rel-to` `Phase::Localized` rule produces
   the `AppliedFix` audit trail; `--preserve-historical-form` flips
   it to diagnostic-only.
3. ~~**§3 `NF` clears `REL TO`**: lattice op vs `PageRewrite`.~~
   `Status: resolved (2026-05-02)` — confirm-and-document; PR 3.7
   fill-in cites §H.8; lattice does not encode the supersession.
4. **§4 SCI per-system canonicalization**: lattice vs
   `render_canonical` boundary.
   `Status: open_gate (cite §A.6 / §H.4 passage governing per-
   system compartment canonicalization; decide whether HCS-O /
   HCS-O-P / SI-G interactions live in the lattice impl or in
   render-canonical; #267 Gap A interacts)`.
5. **§5 SAR ordering**: lattice canonical vs render canonical.
   `Status: open_gate (cite §H.5 SAR ordering passage; decide
   whether SAR sort discipline lives in Ord on SarProgram /
   SarCompartment or in render-canonical only)`.
6. ~~**§6 tetragraph join level**: trigraph-expanded vs tetragraph-atomic.~~
   `Status: resolved (2026-05-02)` — see §3 Resolution block:
   lattice operates on expanded trigraph atoms; group designations
   decompose at insert; `render_canonical` re-folds greedily by
   descending membership size; opaque codes pass through; schema-
   pinned membership tables.
7. **§7 ATOMAL/BOHEMIA combinability**: §H.3 citation needed.
   `Status: open_gate (cite §H.3 ATOMAL / BOHEMIA passage; answer
   whether the two combine in a single marking and what the join
   produces; required before PR 9's NATO control-set lattice
   work)`.
8. **§8 mixed dates and canned strings**: §C citation needed.
   `Status: open_gate (cite §C passage on mixed dates + canned
   strings on same page; specify the join shape; the AEA / NATO
   canned-string supersession ordering itself stays scope_cut
   per the §8 in-text deferral to #266)`.
9. **§3 `display_only` axis on `IsmAttributes`** (NEW 2026-05-02):
   `IsmAttributes` today has `rel_to: Box<[CountryCode]>` but no
   parallel `display_only` field. Per §3 Resolution, the dissem
   axis is a product over `rel_to`, `display_only`, `eyes`. PR 4
   adds the `display_only: Box<[CountryCode]>` field on
   `IsmAttributes`.
   `Status: open_gate (confirm field addition is a PR 4
   deliverable, not a prerequisite earlier; recommendation per
   2026-05-02 conferral: PR 4, since the field has no consumer
   until lattice impls land)`.
10. **§3 `country_sort_key` placement and `CountryCode::kind()`
    contract** (NEW 2026-05-02): the sort key consumed by
    `render_canonical` and the Register-order validator lives at
    `marque-ism::country` (universal across schemes). `kind()`
    consults a build-time-generated `phf::Set<&'static str>` of
    known group codes from ISMCAT XML.
    `Status: open_gate (confirm which PR generates the table;
    recommendation: PR 5, co-locating with the rest of the
    Vocabulary<S> build-time metadata — is_fdr_dissem,
    is_caveat_token, register_order)`.
11. **§3 cross-axis `PageRewrite` enumeration** (NEW 2026-05-02):
    the §3 Resolution names two new declarative `PageRewrite`s
    that fall out of the product shape:
    `empty-rel-to-becomes-noforn` (Table 3 rules 9, 11, 13, 16,
    19, 20 — `IntersectSet` produces empty common LIST → drop the
    REL TO/DISPLAY ONLY token, add NF) and
    `display-only-subsumes-rel-to` (Table 3 rule 26 — DISPLAY ONLY
    + REL TO with common LIST → DISPLAY ONLY [common LIST] because
    release implies disclosure).
    `Status: open_gate (confirm reads/writes axis annotations
    for the topo scheduler; cycle-free dispatch with the
    existing noforn-clears-rel-to rewrite; PR slot: PR 9, which
    already declares missing PageRewrites)`.

These are the points where the previous attempt skimmed. The fill-in
pass must close every `open_gate:` before PR 4 implementation begins;
`scope_cut:` items remain documented but do not block.
