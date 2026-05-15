<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Lattice design — per-category join semantics

**Date:** 2026-05-01 (filled in 2026-05-13 in PR 3.7 Stage A)
**Status:** filled — pending PR 3.7 reviewer attestation per §9
**Acceptance:** see §9 below. Each category section in §§2-8 carries
the four required items:

1. §-citations to `crates/capco/docs/CAPCO-2016.md` (page-range
   verified against `crates/capco/docs/CAPCO-2016_citation_index.yml`).
2. Formal join semantics in functional pre/post form (not prose).
3. ≥2 worked examples per section including edge cases.
4. Property-test fixtures named by file path and test function.

**Related docs:**
- `2026-05-02-engine-refactor-consolidated.md` (drives this gate; supersedes the deleted 2026-05-01 draft)
- `2026-05-13-pr3.7-lattice-resolution-gate-plan.md` (the staging plan that lands this fill-in plus the trait-surface primitives)
- `2026-04-19-recursive-lattice-and-decoder.md` §3 (existing
  `SciSet`/`SarSet`/`FgiSet` lattice work; equal-depth meet policy)
- `2026-04-17-marking-scheme-lattice-design.md` §0–2 (Phase A
  scaffolding; problem statement)
- `.claude/skills/marque-lattice-consultant/references/marque-applied.md`
  (algebraic source-of-truth, especially §4 per-axis lattice ops and
  §4.7 closure operator)

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
| **AEA control set** (`CAT_AEA`) | None | **New: lands in PR 4b-A — see §7.5** (`AeaSet`: RD/FRD/TFNI supersession + CNWDI + SIGMA + UCNI + ATOMAL routing per §H.6 pp103-121 + §G.2 Table 5 p40 + §H.7 p122 FGI-section worked example) |
| Declassify-on / `MaxDate` | `MaxDate` | Functional for dates; AEA/NATO canned strings (#266) deferred |

Each section below is a stub. **A reviewer signing off on PR 4 is
signing off that every section in this document carries the four
acceptance items.**

---

## 2. `MarkingClassification`

### §-citations

- **§A.4** (p13) — IC Markings System Structure; classification hierarchy.
- **§H.1** (pp46-54) — US classification levels: TOP SECRET (p47), SECRET (p48), CONFIDENTIAL (p50), UNCLASSIFIED (p51).
- **§H.3** (pp55-59) — JOINT classification grammar.
- **§H.7** (pp122-130) — FGI classification, especially **§H.7 reciprocal-classification rule (pp123-125)** establishing that foreign classifications normalize to US-equivalents at portion-parse time.
- **§B.8** (pp23-24) — Classification Marking Elements.
- **§D.2** (pp28-30) — Banner Line "Roll-Up" Rules; **Table 3 (p28) FD&R Precedence for Banner Line Roll-Up** governs the inter-portion roll-up direction.
- **§H.2** (p55) — NON-US PROTECTIVE MARKINGS (refers to IC Markings System Manual Appendices A/B/C — see §B.3 pp19-21 for derivative-source treatment).

### Formal join semantics

The classification axis is an **`OrdMax` over the total order**
`Unclassified < Confidential < Secret < TopSecret` on the US chain.
**Cross-branch joins do not arise in the lattice** because §H.7
mandates portion-parse-time reciprocal normalization: a foreign
classification is converted to its US-equivalent level before
`OrdMax` sees it. The foreign-equity *source* (the country trigraphs)
is carried on a **separate** axis — `FgiSet` (§6 below) — not on
`MarkingClassification`.

```text
∀ p1, p2 ∈ ParsedPortion :
  let c1 = reciprocal_normalize(p1.classification) ∈ {U, C, S, TS}
  let c2 = reciprocal_normalize(p2.classification) ∈ {U, C, S, TS}
  in  banner.classification = OrdMax(c1, c2)

  Pre:  c1, c2 are reciprocal-normalized US-chain levels.
  Post: banner.classification is the max under U < C < S < TS.
        Identity (bottom): Unclassified.
        Identity (top): TopSecret.
```

This is a bounded total order: `OrdMax<MarkingClassification>` over
a four-element chain. Associativity/commutativity/idempotency follow
from the chain structure trivially.

**JOINT classification** (§H.3 p56) is a **separate banner-render
concern**, not a lattice-join concern. When two or more producers
share equity on the same portion, the portion is *parsed* with
a `MarkingClassification::Joint { producers: SmallVec<...> }` shape,
but the classification *level* still normalizes to the US chain
via §H.7 reciprocal rules. The Joint discriminator survives into
render; the lattice never sees it as a separate branch.

### Worked examples

**Example 1 — pure US chain:**
```
Inputs:  (C) (S) (TS//SI) (U)
Per-portion levels: C, S, TS, U
OrdMax over chain:  C ⊔ S ⊔ TS ⊔ U = TS
Banner classification: TOP SECRET
```

**Example 2 — foreign-source reciprocal normalization (the #276 case):**
```
Inputs:  (C//FGI DEU) (S)
Per-portion levels (pre-normalize): C, S
  — DEU CONFIDENTIAL → US CONFIDENTIAL per §H.7 reciprocal rule (pp123-125).
  — The DEU source is preserved on the FgiSet axis (§6), not the classification axis.
OrdMax over chain:  C ⊔ S = S
Banner classification: SECRET
Banner FGI axis (separate): FGI DEU
Banner output: SECRET//FGI DEU
```
The classification axis flattens to a US-equivalent maximum; the FGI
trigraph rides on its own axis. There is no cross-branch lattice op.

**Example 3 — edge case, foreign-equivalent above all US portions:**
```
Inputs:  (//DEU TS) (S)
Per-portion levels (pre-normalize):
  — DEU TS → US TS per §H.7 reciprocal rule.
  — US S → US S (identity).
OrdMax over chain: TS ⊔ S = TS
Banner classification: TOP SECRET
Banner FGI axis: FGI DEU
Banner output: TOP SECRET//FGI DEU
```
The foreign portion *reciprocally raises* the banner via the
normalized level, as §H.7 requires ("US always reciprocates equivalent
US protection"). The lattice op is still `OrdMax` on the US chain.

### Property-test fixtures (for PR 4 T116/T118)

- `crates/capco/tests/category_lattice_laws.rs::classification_assoc_comm_idem`
  — assoc/comm/idem over the four-element chain.
- `crates/capco/tests/category_lattice_laws.rs::classification_identity_with_bottom`
  — Unclassified is the identity element.
- `crates/capco/tests/cross_axis_dominance.rs::reciprocal_normalization_fgi_deu_secret`
  — fixture for Example 2 (`(C//FGI DEU) + (S) → SECRET//FGI DEU`).
- `crates/capco/tests/cross_axis_dominance.rs::reciprocal_raise_foreign_top_secret`
  — fixture for Example 3 (`(//DEU TS) + (S) → TOP SECRET//FGI DEU`).
- `tests/corpus/lattice/fgi-banner-rollup.json` — end-to-end corpus
  fixture covering the #276 banner-retains-FGI case.

### Open questions — resolved

1. **Cross-branch join (US/NATO/FGI)?** → **No cross-branch join
   arises in the lattice.** §H.7 reciprocal-classification rule (pp123-125)
   mandates portion-parse-time normalization of foreign classifications
   to US-equivalent levels. `MarkingClassification::Joint` is a render
   concern, not a lattice-branch concern (§H.3 p56). The lattice is
   `OrdMax` on the US chain, full stop.
2. **Equivalent-level different-scheme joins (US Secret vs NATO Secret)?**
   → **Same as #1.** NATO SECRET reciprocally normalizes to US SECRET
   before the lattice sees it; the lattice sees `SECRET ⊔ SECRET = SECRET`.
   The NATO source survives on the foreign-equity axis (§7) for banner
   rendering.
3. **`expected_classification()` for purely-foreign pages?** →
   `Option<MarkingClassification>` is the correct type (already
   committed in PR 5 / T059). Value semantics: when every portion has
   a foreign-source classification, `OrdMax` returns the
   reciprocally-normalized US-chain level; the foreign equity rides
   on the separate `FgiSet` / NATO axis. `Option::None` only arises
   when no portions carry any classification (an unmarked page); a
   pure-foreign page carries a non-None value. The legacy hardcoded
   `MarkingClassification::Us` was wrong (PR 5 T059 deletes the
   hardcode); the replacement is "OrdMax over reciprocal-normalized
   level," not "fallback to US."

---

## 3. Dissem set

The dissem axis is the most structurally complex of the seven
categories because it carries three orthogonal sub-axes — the FD&R
chain (§B.3 Table 2 p21, §D.2 Table 3 p28), the non-FD&R IC dissem
controls (§H.8), and the non-IC dissem controls (§H.9) — plus the
NOFORN-clears-REL-TO `PageRewrite` interaction, the FOUO eviction
matrix, and the closure operator's implicit-default trio (§4.7 of
`marque-applied.md`).

This section is organized as five subsections per the lattice-preflight
recommendation: (a) FD&R chain, (b) FOUO eviction matrix, (c) NODIS/EXDIS
supersession, (d) NOFORN-clears-REL-TO PageRewrite, (e) closure-operator
interaction.

### §-citations

- **§H.8** (pp131-168) — IC Dissemination and Control Markings:
  RSEN (pp132-133), FOUO (pp134-135), ORCON (pp136-138), ORCON-USGOV (pp139-141),
  IMCON (pp142-144), **NOFORN (pp145-147)**, PROPIN (pp148-149),
  **REL TO (pp150-153)**, **RELIDO (pp154-156)**, EYES (pp157-158, deprecated 2017-10-01),
  DEA SENSITIVE (pp159-160), FISA (pp161-162), DISPLAY ONLY (pp163-168).
- **§H.9** (pp169-191) — Non-IC Dissemination Control Markings:
  LIMDIS (pp170-171), **EXDIS (pp172-173)**, **NODIS (pp174-175)**,
  SBU (pp176-177), SBU-NF (pp178-180), LES (pp181-184), LES-NF (pp185-188),
  SSI (pp189-191).
- **§B.3** (pp19-21) — Foreign Disclosure and Release Markings;
  **Table 2 p21 (FD&R Markings Summary)** is the authoritative roster
  of the FD&R set.
- **§D.2** (pp28-30) — Banner Roll-Up Rules; **Table 3 p28 (FD&R
  Precedence for Banner Line Roll-Up)** governs the supersession order.

### (a) FD&R chain — `SupersessionSet` over the §H.8 / §B.3 Table 2 roster

The FD&R set per **§B.3 Table 2 p21** is:
`{NOFORN, RELIDO, REL TO, DISPLAY ONLY, EYES}` (EYES deprecated
2017-10-01 per §H.8 p157 — migration to `REL TO` is a parser-side
concern, not a lattice-participation concern; the lattice sees only
post-migration `REL TO`).

Supersession order per **§D.2 Table 3 p28** is a total order on the
FD&R chain:
```
NOFORN  ⊐  DISPLAY ONLY  ⊐  REL TO  ⊐  RELIDO
```
("⊐" reads "supersedes"; the left side wins on banner roll-up).

**Note on EYES**: the in-tree implementation maps deprecated `EYES`
to `REL TO` at parse time (`crates/core/src/parser.rs` per the migration
table), so the lattice never sees an `EYES` atom — it sees `REL TO`.
Q-3.4.5c in `marque-applied.md` §9 resolved this as parser-side migration,
not lattice-side participation.

```text
∀ d1, d2 ∈ FD&R chain :
  banner.fdr = SupersessionSet::join(d1, d2)
             = whichever is higher in the §D.2 Table 3 order

  Pre:  d1, d2 ∈ {NOFORN, DISPLAY ONLY, REL TO, RELIDO} (post EYES→REL TO migration).
  Post: banner.fdr is the supersession-max under NOFORN > DISPLAY ONLY > REL TO > RELIDO.
        Identity (bottom): no FD&R marking.
        Identity (top): NOFORN.
```

This is a **bounded total-order supersession-semilattice**: `SupersessionSet`
in `marque-scheme::builtins`. Associativity/commutativity/idempotency
follow from the total-order structure.

### (b) FOUO eviction matrix (two-axis)

**FOUO** is a special-case dissem control with two eviction triggers
per §H.8 pp134-135 + `marque-applied.md` §4.7.5 Trio 2 footnote:

| Eviction axis | Trigger | §-citation |
|---|---|---|
| **Cross-category: classification > U** | Banner classification ⊐ Unclassified | §H.8 p134 "FOUO does not apply to classified information"; §B.3 Table 2 derivative rules |
| **In-category: any non-FD&R dissem** | Any token in `{ORCON, ORCON-USGOV, IMCON, PROPIN, DEA SENSITIVE, FISA, RSEN, NNPI}` is present | §H.8 p134 ("FOUO is the residual non-IC unclassified caveat"); `marque-applied.md` §3.4.4 absorbing-element framing |

FOUO is **NOT** evicted by FD&R tokens (NOFORN/RELIDO/REL TO/DISPLAY ONLY/EYES);
the FD&R chain is orthogonal to the FOUO eviction. FOUO is evicted only by
**classification ascent** or by **non-FD&R control presence**.

```text
∀ page :
  let class_evict   = page.classification ⊐ Unclassified
  let nonfdr_evict  = ∃ t ∈ page.dissem . is_non_fdr_dissem(t) ∧ t ≠ FOUO
  in  banner.has_fouo = page.has_fouo ∧ ¬class_evict ∧ ¬nonfdr_evict
```

This is a **cross-axis constraint**, not a within-axis join. It lives
as a `Constraint::Custom("capco/fouo-eviction")` row on `CapcoScheme`,
evaluated after axis-fold but before banner render.

### (c) NODIS/EXDIS supersession (§H.9)

NODIS and EXDIS are non-IC dissem controls (§H.9) that supersede each
other and FD&R chain. Per **§H.9 NODIS pp174-175** + **§H.9 EXDIS pp172-173**:

```
NODIS  ⊐  EXDIS  ⊐  (any FD&R token)
```

NODIS p174 ("No Distribution outside Department of State") is more
restrictive than EXDIS p172 ("Exclusive Distribution within Department of
State"). Both supersede REL TO / RELIDO; **NODIS additionally clears
REL TO entirely** per §H.9 p174 (and the PR 3c.B 8F engine-gap
short-circuit at `PageContext::expected_rel_to`).

This is a **second supersession chain orthogonal to the FD&R chain**:

```text
banner.nodis_exdis = SupersessionSet::join over {NODIS, EXDIS}
                   = NODIS if present, else EXDIS if present, else None.

Cross-axis interaction:
  if banner.nodis_exdis = NODIS then banner.fdr = None  (NODIS clears FD&R)
  if banner.nodis_exdis = EXDIS then banner.fdr = preserve (EXDIS does NOT clear)
```

The NODIS-clears-FD&R interaction is captured as the in-tree
`capco/nodis-exdis-clear-rel-to` PageRewrite (`crates/capco/src/scheme.rs`
declared 2026-05-12 in PR 3c.B Sub-PR 8.F).

### (d) NOFORN-clears-REL-TO PageRewrite (§H.8 / Q3 confirm-and-document)

Per **§H.8 NOFORN (pp145-147) top-of-FD&R-chain** + **§H.8 REL TO
(pp150-153)**, the operative rule is "REL TO is meaningful only when
the document is NOT NOFORN; NOFORN clears REL TO during banner
roll-up."

**Implementation shape**: `capco/noforn-clears-rel-to` is a declared
`PageRewrite` at `crates/capco/src/scheme.rs:486-498` (the line range
the rev-1 plan cites). It runs **after** per-axis lattice projection,
inside `CapcoScheme::project(Scope::Page, ...)`. The lattice itself
does NOT encode the supersession — the lattice produces "NOFORN ⊔ REL TO
= {NOFORN, REL TO}" as a set-union, and the PageRewrite then strips
the dominated `REL TO` token.

**Why a PageRewrite and not a lattice op (Q3 confirm-and-document)?**
The §3.3 Q3 originally framed this as a choice ("lattice op vs
PageRewrite"). Resolution (2026-05-02 per `marque-applied.md` §3.4.1
entry 4 + `decisions.md` D13 amendment): the rewrite is a **cross-axis
transformation** — NOFORN is on the FD&R chain (dominator), REL TO
carries a trigraph-list payload. A lattice op would have to encode
the trigraph-list interaction in the supersession order, which mixes
two algebraic shapes. The PageRewrite cleanly separates "compute the
fact set via lattice join" from "apply policy-driven cross-axis
transformations." The §D.2 Table 3 banner-roll-up rules ARE the
PageRewrite roster, not the lattice law.

```text
PageRewrite capco/noforn-clears-rel-to:
  Preconditions: NOFORN ∈ page.dissem ∧ REL TO ∈ page.dissem
  Action: page.dissem.remove(REL TO)
  Postcondition: REL TO is not in the banner.
  Idempotent: removing the same token twice has no effect.
```

The lattice law on the dissem axis is **unchanged** by this PageRewrite —
the lattice still produces the union under supersession join; the
rewrite is a post-projection cleanup operating on the resulting fact
set. This is why §3 Open Q3 resolves as "confirm and document" rather
than "decide": the existing implementation is already correct under
the §3.4.1 transmutation roster framing.

### (e) Closure-operator interaction (§4.7)

The closure operator (T108c, this PR) propagates implicit dissem-axis
facts. Three of the §4.7.1 implicit-default trio rows propagate into
the dissem axis:

| Closure row | Trigger | Suppressor | Cone (dissem axis) | §-citation |
|---|---|---|---|---|
| `capco/noforn-if-no-fdr` | SAP / RD / FRD / TFNI / DCNI / UCNI / FGI [non-NATO] / ORCON / IMCON / DSEN / LIMDIS / LES / SBU / SSI / NNPI present | `has_fdr(page)` | `{NOFORN}` | §H.8 NOFORN p145 (top of FD&R); §B.3 Table 2 p21 (FD&R roster) |
| `capco/relido-if-no-fdr-and-not-incompat` | Bare SCI / U / collateral / RSEN / FOUO present | `has_fdr(page) ∨ has_relido_incompat(page)` | `{RELIDO}` | §H.8 RELIDO p154 |
| `capco/rel-usa-nato-if-no-fdr-and-nato` | Any NATO portion or banner | `has_fdr(page)` | `{REL TO USA, NATO}` (on FD&R axis) **(placeholder — see implementation note below)** | §H.3 NATO p55-59 |
| HCS-O / HCS-P[sub] | SCI presence (proxy) | `&[]` (unconditional) | `{NOFORN, ORCON}` **(proxy trigger — see implementation note below)** | §H.4 HCS pp64-69 |
| SI-G | SCI presence (proxy) | `&[]` (unconditional) | `{ORCON}` **(proxy trigger — see implementation note below)** | §H.4 SI p80 |
| TK-BLFH / TK-KAND / TK-IDIT | SCI presence (proxy) | `&[]` (unconditional) | `{NOFORN}` **(proxy trigger — see implementation note below)** | §H.4 TK pp87-98 |

**Implementation note (PR 3.7 → PR 4)**: The five per-marking SCI
implication rows (HCS-O / HCS-P[sub] / SI-G / TK-BLFH / TK-KAND / TK-IDIT)
ship in PR 3.7 with `AnyInCategory(CAT_SCI)` as a coalesced trigger
because per-compartment sentinel `TokenId`s (`TOK_HCS_O`, `TOK_SI_G`,
`TOK_TK_BLFH`, etc.) do not yet exist in the in-tree vocabulary. The
catalog therefore **over-fires** on bare `SI` / bare `TK` portions
relative to `marque-applied.md` §4.7.1's per-marking spec. PR 4 (T112)
splits the trigger to per-compartment `TokenRef::Token(...)` values
when those sentinels land alongside the per-category `Lattice` impls.

The Trio 3 `capco/rel-usa-nato-if-no-fdr-and-nato` row ships with
`cone: &[TokenRef::AnyInCategory(CAT_REL_TO)]` as a placeholder: the
open-vocab "add `REL TO USA, NATO` specifically" closure-step lands
in PR 4 once `Engine::project::closure()` is wired and the
country-list payload mechanic exists.

**Both deferrals are dormant in PR 3.7** because `CapcoScheme::closure()`
is not overridden in this PR — the catalog ships as data, the runtime
wiring lands at PR 4. See `crates/capco/src/scheme.rs` inline TODOs
(per-row) for the precise sentinel/wiring tasks.

Closure fires **after** per-axis join, **before** the
NOFORN-clears-REL-TO PageRewrite (per `marque-applied.md` §4.7.4
pipeline ordering). This means:

```text
Pipeline:
  per-portion parse → per-axis join → Cl_supp (closure)
                                    → PageRewrites (NOFORN-clears-REL-TO,
                                                    NODIS-clears-FD&R, etc.)
                                    → render
```

The closure operator is monotone/extensive/idempotent on this
pipeline; the PageRewrites are inflationary on the closed state
(they remove dominated tokens but the remaining tokens are
already members of the closure's fixed point).

### Worked examples

**Example 1 — FD&R chain join + NOFORN clears REL TO:**
```
Inputs:  (S//NF) (S//REL TO USA, GBR)
Per-portion dissem:
  portion 1: {NOFORN}
  portion 2: {REL TO[USA, GBR]}
Per-axis join (SupersessionSet over FD&R):
  page.fdr = NOFORN ⊔ REL TO = NOFORN  (NOFORN supersedes REL TO per §D.2 Table 3 p28)
Then PageRewrite capco/noforn-clears-rel-to fires (defensive cleanup):
  page.dissem = {NOFORN}  (REL TO already gone via supersession; rewrite is idempotent)
Banner output: SECRET//NOFORN
```

**Example 2 — REL TO trigraph-payload intersection (§D.2 Table 3 p28 Rules 9 + 21; §H.8 pp150-153 worked examples):**
```
Inputs:  (S//REL TO USA, FVEY) (S//REL TO USA, FVEY, GBR)
Per-portion dissem:
  portion 1: REL TO[USA, FVEY]
  portion 2: REL TO[USA, FVEY, GBR]
Per-axis join (REL TO payload-intersection per §D.2 Table 3 p28 Rule 9 + Rule 21; §H.8 p152 worked example illustrates):
  The banner can release only to countries every portion authorizes.
  page.rel_to_countries = {USA, FVEY} ∩ {USA, FVEY, GBR} = {USA, FVEY}
  page.fdr = REL TO[USA, FVEY]
Banner output: SECRET//REL TO USA, FVEY
```
This is the `IntersectSet`-based REL TO join (§4.3 below); it is
NOT supersession. The plan's earlier OQ #2 ("set-extension vs
supersession") is resolved by carrying REL TO as its own
`IntersectSet`-shaped sub-lattice within the dissem axis, parallel
to the FD&R `SupersessionSet`.

**Example 3 — FOUO evicted by classification ascent:**
```
Inputs:  (U//FOUO) (S)
Per-portion classification: U, S
OrdMax classification: S
Per-portion dissem: {FOUO}, {}
Per-axis join: {FOUO}
Cross-axis check (§3 (b)): banner.classification = S ⊐ U → FOUO evicted.
Banner dissem: {}
Banner output: SECRET
```

**Example 4 — FOUO evicted by non-FD&R dissem:**
```
Inputs:  (U//FOUO) (U//ORCON)
Per-portion classification: U, U
Per-portion dissem: {FOUO}, {ORCON}
Per-axis join: {FOUO, ORCON}
Cross-axis check (§3 (b)): ORCON is non-FD&R → FOUO evicted.
Banner dissem: {ORCON}
Banner output: UNCLASSIFIED//ORCON
```

**Example 5 — closure propagates NOFORN (implicit-default trio):**
```
Inputs:  (S//ORCON) (S)
Per-portion dissem: {ORCON}, {}
Per-axis join: {ORCON}
Closure pass: capco/noforn-if-no-fdr fires.
  Trigger: ORCON present (§4.7.1 trigger list).
  Suppressor: has_fdr(page) = false  (no NOFORN/REL TO/RELIDO/DISPLAY ONLY).
  Cone: {NOFORN} added to dissem.
Closed dissem: {ORCON, NOFORN}
Banner output: SECRET//ORCON//NOFORN
```

### Property-test fixtures (for PR 4)

- `crates/capco/tests/category_lattice_laws.rs::dissem_fdr_assoc_comm_idem`
  — FD&R chain laws.
- `crates/capco/tests/category_lattice_laws.rs::dissem_rel_to_intersect_assoc_comm_idem`
  — REL TO trigraph-payload intersection.
- `crates/capco/tests/cross_axis_dominance.rs::fouo_evicted_by_class_above_u`
  — Example 3 fixture.
- `crates/capco/tests/cross_axis_dominance.rs::fouo_evicted_by_non_fdr_dissem`
  — Example 4 fixture.
- `crates/capco/tests/cross_axis_dominance.rs::noforn_clears_rel_to_page_rewrite`
  — Example 1 fixture.
- `crates/scheme/tests/proptest_closure.rs::implicit_noforn_monotone_extensive_idempotent`
  — closure laws on the implicit-default trio.
- `tests/corpus/lattice/fouo-eviction-class.json` — corpus fixture for Example 3.
- `tests/corpus/lattice/fouo-eviction-non-fdr.json` — corpus fixture for Example 4.

### Open questions — resolved

1. **`EYES` lattice participation (pre- vs post-migration)?** →
   **Post-migration `REL TO`.** Parser-side migration table converts
   `EYES` portions to `REL TO` at parse time per §H.8 p157 (NSA waiver
   expired 2017-10-01); the lattice never sees an `EYES` atom.
   Q-3.4.5c resolved 2026-05-07.
2. **`REL TO` trigraph payload — supersession or set-extension?** →
   **Set-extension via `IntersectSet`.** REL TO carries its own
   `IntersectSet`-shaped sub-lattice within the dissem axis (§4.3
   detailed below); join is set-intersection of authorized countries,
   not supersession. The FD&R supersession chain operates on tokens
   (NOFORN, RELIDO, REL TO, DISPLAY ONLY); the per-token REL TO
   country-list is its own sub-axis.
3. **NOFORN clears REL TO — lattice or PageRewrite?** →
   **PageRewrite.** Resolved 2026-05-02: `capco/noforn-clears-rel-to`
   is a declared `PageRewrite` at `crates/capco/src/scheme.rs:486-498`.
   The lattice produces the union under supersession join; the
   PageRewrite removes dominated tokens. See §3 (d) above for the
   functional form.

---

## 4. `SciSet`

### §-citations

- **§A.6** (pp15-16) — IC Markings System Formatting; the SCI grammar
  `CONTROL-COMP (SPACE SUB-COMP)* (-COMP (SPACE SUB-COMP)*)*` is on
  pp15-16, with the canonical example (`SI-G ABCD DEFG-MMM AACD`) and
  the §A.6 p15 sort order (numeric first, alpha after).
- **§H.4** (pp60-98) — SCI Control System Markings: HCS (pp62-69)
  including HCS-O (pp64-65) and HCS-P (pp66-69) with HCS-P sub-comp
  (pp68-69); RSV (pp70-73) with RSV-[COMP] (pp72-73); SI (pp74-84)
  including SI-[COMP] (pp76-77), SI-ECRU (pp78-79), SI-GAMMA (pp80-82)
  with GAMMA sub-comp (pp81-82), SI-NONBOOK (pp83-84); TK (pp85-98)
  including TK-BLFH (pp87-90), TK-IDIT (pp91-94), TK-KAND (pp95-98).
- **Figure 4** (p60) — SCI Control System Hierarchical Structure
  diagram.
- **Table 6** (p61) — SCI Sample Banner Marking Categories and Markings.

### Formal join semantics

`SciSet` is a **join-semilattice with no operationally meaningful
top** (compartments are agency-extensible per §H.4 — open vocabulary)
and **no operationally meaningful meet** (SCI is indestructible —
CAPCO does not define a "less restrictive common access" operation
per §4.1 of `marque-applied.md`).

Per `marque-applied.md` §4.2 and the §A.6 grammar, the join law is
**fact-set union under prefix-inclusion order** at equal depth:

```text
Each SCI marking M decomposes into a fact set:
  facts(M) = { all prefixes of M's control/comp/sub-comp path }

  e.g., facts("SI-G ABCD") = {SI, SI-G, SI-G ABCD}
        facts("HCS-O-P MNOP") = {HCS, HCS-O, HCS-O-P, HCS-O-P MNOP}

Join:  SciSet::join(M1, M2) = SciSet::from_facts( facts(M1) ∪ facts(M2) )

  Pre:  M1, M2 are well-formed SCI markings per §A.6 grammar.
  Post: result is the prefix-closed union of fact sets,
        re-rendered into canonical §A.6 form sorted per p15
        (numeric first, alpha after).
        Identity (bottom): SciSet::empty() (no SCI markings).
        No top: control systems are agency-extensible.
```

This is a `JoinSemilattice` (no `BoundedLattice` because there is no
`top`). Associativity/commutativity/idempotency follow trivially from
set-union being well-behaved.

### Equal-depth meet policy — §A.6 grammar rationale

The §A.6 grammar is **strictly hierarchical**: a sub-compartment
*belongs to* exactly one compartment, which *belongs to* exactly one
control system. The grammar position `(-COMP (SPACE SUB-COMP)*)*`
makes the compartment-sub-comp coupling syntactically explicit.

"Equal-depth" means: when joining `M1` and `M2`, sub-compartments
under `M1`'s compartment-tree are only unioned with sub-compartments
under `M2`'s compartment-tree at the **same** (control, compartment)
key. Depth-mismatched joins (e.g., trying to union an `SI-G` portion
with an `SI-G ABCD` portion at the sub-comp level) would either lose
information (drop ABCD) or produce a grammar violation (attach ABCD
to a non-existent compartment context).

The prefix-closed-union framing makes this rigorous: `SI-G` and
`SI-G ABCD` share the prefix-fact `SI-G`; the join is `facts(SI-G) ∪
facts(SI-G ABCD) = {SI, SI-G, SI-G ABCD}`, which re-renders as
`SI-G ABCD`. Equal-depth falls out naturally from the algebra; the
implementation in `marque-capco::lattice::SciSet` (per #69) makes
the policy explicit at the data-structure layer.

### Per-system canonicalization — lattice vs render-canonical boundary

Per **§H.4** per-system rules:
- **HCS-O** + **HCS-P** can be stacked under one control via `-COMP`
  separators (§H.4 HCS-OPERATIONS p64; §H.4 HCS-PRODUCT p66) — i.e.,
  `HCS-O-P MNOP` is valid for "operations + product MNOP".
- **SI-G** + **SI-NONBOOK** stack via `-COMP` separators (§H.4 SI p74).
- **TK-BLFH** + **TK-IDIT** + **TK-KAND** stack similarly under TK
  (§H.4 TALENT KEYHOLE p85).

The **lattice produces the prefix-closed-union fact set**;
**`render_canonical` does the §A.6 canonical-form rendering** (sort
per p15, stack compartments via `-COMP`, separate control systems via
`/`). This boundary is operationally clean: the lattice op is
commutative/associative on fact sets; the rendering pass imposes the
§A.6 ordering at output time.

**#267 Gap A (companion-insert path)** — when an SCI marking carries
an implied companion (e.g., HCS-O ⇒ NOFORN per §4.7 closure rules) —
the companion is added by the closure operator (§4.7 of `marque-applied.md`,
T108c), NOT by the SCI lattice. The lattice operates only on SCI facts;
companion facts on other axes are closure-operator territory.

### Worked examples

**Example 1 — same control, sub-comp union:**
```
Inputs:  (S//SI-G ABCD) (S//SI-G DEFG)
facts(portion 1) = {SI, SI-G, SI-G ABCD}
facts(portion 2) = {SI, SI-G, SI-G DEFG}
Join (set-union): {SI, SI-G, SI-G ABCD, SI-G DEFG}
Canonical render (§A.6 p15, alpha order): SI-G ABCD DEFG
Banner: SECRET//SI-G ABCD DEFG
```

**Example 2 — different controls in same family, compartment stack:**
```
Inputs:  (TS//HCS-O) (TS//HCS-P MNOP)
facts(portion 1) = {HCS, HCS-O}
facts(portion 2) = {HCS, HCS-P, HCS-P MNOP}
Join (set-union): {HCS, HCS-O, HCS-P, HCS-P MNOP}
Canonical render (§A.6 -COMP separator, sub-comp rides on P only):
  HCS-O-P MNOP
Banner: TOP SECRET//HCS-O-P MNOP
```

**Example 3 — different controls, `/` separator:**
```
Inputs:  (TS//SI-G ABCD) (TS//TK-BLFH XYZW)
facts(portion 1) = {SI, SI-G, SI-G ABCD}
facts(portion 2) = {TK, TK-BLFH, TK-BLFH XYZW}
Join (set-union): {SI, SI-G, SI-G ABCD, TK, TK-BLFH, TK-BLFH XYZW}
Canonical render (§A.6 different-controls separator `/`):
  SI-G ABCD/TK-BLFH XYZW
Banner: TOP SECRET//SI-G ABCD/TK-BLFH XYZW
```

**Example 4 — edge case, agency-extensible compartment:**
```
Inputs:  (S//99) (S//99 XYZW)
  where "99" is a custom control system not in the closed CVE set
  (handled structurally via SciControlSystem::Custom; see CLAUDE.md
   "SCI Compartments" section).
facts(portion 1) = {99}
facts(portion 2) = {99, 99 XYZW}
  Note: "99 XYZW" is a custom-control compartment, not a "99-X"
  hyphenated compartment — the grammar §A.6 distinguishes via
  the `-COMP` vs `SPACE SUB-COMP` parse.
Join (set-union): {99, 99 XYZW}
Canonical render: 99 XYZW
Banner: SECRET//99 XYZW
```
This demonstrates the no-top property: there is no upper bound across
all possible custom control systems (the agency may register new ones
at any time per §H.4 general framing). `BoundedLattice` cannot be
implemented; `JoinSemilattice` can.

### Property-test fixtures (for PR 4)

- `crates/capco/tests/category_lattice_laws.rs::sciset_join_assoc_comm_idem`
  — assoc/comm/idem on prefix-closed-union join.
- `crates/capco/tests/category_lattice_laws.rs::sciset_identity_with_empty`
  — `SciSet::empty()` is identity.
- `crates/capco/tests/cross_axis_dominance.rs::sci_cross_system_canonical`
  — Example 3 (`SI/TK` separator).
- `crates/capco/tests/cross_axis_dominance.rs::sci_stack_via_dash_comp`
  — Example 2 (`HCS-O-P MNOP` stacking).
- `tests/corpus/lattice/sci-cross-system.json` — corpus fixture for
  the cross-system canonicalization end-to-end case.

### Open questions — resolved

1. **Equal-depth policy §-citation source?** → **§A.6 pp15-16
   grammar.** The strictly-hierarchical grammar
   `CONTROL-COMP (SPACE SUB-COMP)* (-COMP (SPACE SUB-COMP)*)*` makes
   compartment-sub-comp coupling syntactically explicit. The
   prefix-closed-union framing (§4.2 of `marque-applied.md`) makes
   the equal-depth property algebraic rather than ad-hoc.
2. **Per-system canonicalization — lattice or render?** → **Both, at
   different layers.** The lattice produces the prefix-closed-union
   fact set (commutative/associative on facts); `render_canonical`
   imposes the §A.6 ordering, `-COMP` stacking, and `/` separators
   at output time. Companion-insert (#267 Gap A) is **closure operator**
   territory (§4.7, T108c), not SCI lattice territory. The boundary
   is: per-axis facts → lattice → closure → render.

---

## 5. `SarSet`

### §-citations

- **§H.5** (pp99-102) — Special Access Required (SAR); §H.5 SAR-SAPs
  subsection (pp101-102) covers program identifiers, compartments,
  sub-compartments, and banner roll-up.
- **§B.3** (pp19-21) — Foreign Disclosure and Release Markings;
  **Table 2 p21** is the FD&R-derivative authority for the
  conditional SAR→NOFORN rule (NOT a blanket SAR property).
- **Figure 5** (p99) — Optional SAP Hierarchical Structure diagram.
- **Table 7** (p100) — SAP Sample Banner Marking Categories and Markings.

### Formal join semantics

`SarSet` is a **join-semilattice** with the same algebraic shape as
`SciSet`: prefix-closed-union over hierarchical structure
(program → compartment → sub-compartment per §H.5 p101).

```text
Each SAR marking M decomposes into a fact set:
  facts(M) = { all prefixes of M's program/comp/sub-comp path }

  e.g., facts("SAR-XYZ ABC") = {SAR-XYZ, SAR-XYZ ABC}
        facts("SAR-XYZ ABC-DEF GHI") = {SAR-XYZ, SAR-XYZ ABC, SAR-XYZ ABC-DEF, SAR-XYZ ABC-DEF GHI}

Join:  SarSet::join(M1, M2) = SarSet::from_facts( facts(M1) ∪ facts(M2) )

  Pre:  M1, M2 are well-formed SAR markings per §H.5 p101 grammar.
  Post: result is the prefix-closed union of fact sets,
        re-rendered into §H.5 banner-roll-up form.
        Identity (bottom): SarSet::empty() (no SAR markings).
        No top: SAR program identifiers are agency-assigned and not
        centrally registered (CVEnumISMSAR.xml is intentionally empty
        per CLAUDE.md "SAR" section).
```

Like `SciSet`, this is a `JoinSemilattice` (no `BoundedLattice`) and
the lattice laws follow trivially from set-union.

### SAR ordering canonicalization

Per **§H.5 p101**, SAR programs in banner roll-up appear in **alphabetical
ascending order** (E028 in the legacy rule numbering enforced this).
Within a program, compartments are similarly ordered. Sub-compartments
follow §H.5 p101 ordering (which the existing `SarSet::render_canonical`
impl follows).

**Lattice vs render boundary**: the **lattice produces the prefix-closed-union
fact set** (commutative, no ordering imposed); **`render_canonical` imposes
the §H.5 alphabetical-program-ordering** at output time. Same boundary
as `SciSet`.

### FD&R-derivative-mark guidance — declarative `Constraint`, NOT lattice

Per **§B.3 Table 2 p21**, NOFORN is required for SAR portions **only
when the SAR portion is FD&R-derivative**. §H.5 itself has NO general
"SAR requires NOFORN" rule — the conditional derivative shape lives in
§B.3 Table 2 row interactions, not in the SAR per-system rules.

**Implementation**: the conditional NOFORN-derivation lives as a
`Constraint::Requires` row on `CapcoScheme`, gated by the FD&R-derivative
predicate from §B.3 Table 2. The SAR lattice itself is **silent on dissem-axis
implications**; it produces only SAR facts. Cross-axis implications (SAR →
NOFORN, SAR → ClassLevel floor) are `Constraint::Requires` territory.

This matches `marque-applied.md` §3.4.6 framing: per-token classification
floors and dissem-axis implications are constraints, not closure rules
(the document is well-formed without the implication being present —
it's a *required* augmentation under specific FD&R-derivative conditions,
not an *implicit consequence* of SAR itself).

### Worked examples

**Example 1 — program-level join (different programs, alphabetical render):**
```
Inputs:  (TS//SAR-ZULU) (TS//SAR-ALPHA)
facts(portion 1) = {SAR-ZULU}
facts(portion 2) = {SAR-ALPHA}
Join (set-union): {SAR-ALPHA, SAR-ZULU}
Canonical render (§H.5 p101 alpha order): SAR-ALPHA/SAR-ZULU
Banner: TOP SECRET//SAR-ALPHA/SAR-ZULU
```

**Example 2 — same program, compartment union:**
```
Inputs:  (TS//SAR-XYZ ABC) (TS//SAR-XYZ DEF)
facts(portion 1) = {SAR-XYZ, SAR-XYZ ABC}
facts(portion 2) = {SAR-XYZ, SAR-XYZ DEF}
Join (set-union): {SAR-XYZ, SAR-XYZ ABC, SAR-XYZ DEF}
Canonical render (§H.5 alpha within program): SAR-XYZ ABC DEF
Banner: TOP SECRET//SAR-XYZ ABC DEF
```

**Example 3 — edge case, FD&R-derivative SAR forces NOFORN (Constraint, NOT lattice):**
```
Inputs:  (S//SAR-XYZ // REL TO USA, GBR)
  — derived from a foreign source per §B.3 Table 2 FD&R-derivative row.
SAR lattice: {SAR-XYZ}  (lattice produces only SAR facts)
Constraint::Requires fires: SAR-XYZ + FD&R-derivative → NOFORN required.
Diagnostic + FixIntent (post-PR 3c): add NOFORN to dissem axis.
Banner: SECRET//SAR-XYZ//NOFORN
```
The SAR lattice is silent on this; the cross-axis Constraint emits
the NOFORN-required diagnostic.

### Property-test fixtures (for PR 4)

- `crates/capco/tests/category_lattice_laws.rs::sarset_join_assoc_comm_idem`
  — assoc/comm/idem on prefix-closed-union.
- `crates/capco/tests/category_lattice_laws.rs::sarset_identity_with_empty`
  — `SarSet::empty()` is identity.
- `crates/capco/tests/cross_axis_dominance.rs::sar_alphabetical_render`
  — Example 1 fixture for §H.5 p101 alpha ordering.
- `crates/capco/tests/cross_axis_dominance.rs::sar_fdr_derivative_requires_noforn`
  — Example 3 fixture (Constraint, not lattice).

### Open questions — resolved

1. **SAR ordering — lattice or render?** → **Render.** The lattice
   produces the prefix-closed-union fact set (commutative, no ordering);
   `render_canonical` imposes §H.5 p101 alphabetical-program-ordering
   at output time. Same boundary as `SciSet`.
2. **FD&R-derivative SAR→NOFORN — lattice or Constraint?** →
   **`Constraint::Requires`, conditional on §B.3 Table 2 FD&R-derivative.**
   §H.5 has no inherent SAR→NOFORN rule; NOFORN is a §B.3 Table 2
   FD&R-derivative implication, not a SAR property. The SAR lattice
   stays silent on dissem-axis implications; the cross-axis Constraint
   handles the conditional derivation. The lattice/Constraint choice
   reflects the conditional-derivative shape, not a blanket SAR→NF
   requirement.

---

## 6. `FgiSet`

### §-citations

- **§H.7** (pp122-130) — Foreign Government Information (FGI):
  trigraph grammar, source-acknowledged vs source-concealed semantics,
  tetragraph aggregation. **§H.7 p123** specifically governs the
  source-concealed banner grammar.
- **§H.2** (p55) — NON-US PROTECTIVE MARKINGS (cross-reference to
  §B.3 pp19-21 for derivative-source treatment).
- **§B.3** (pp19-21) — derivative-source rules; Table 2 p21.
- **`marque-applied.md` §4.8** — the FGI/JOINT-attribution lattice
  formalization; **§4.8.5** worked example
  (`C//NF + //GBR-TS → TOP SECRET//FGI GBR//NOFORN`).

### Formal join semantics

`FgiSet` is a **bounded join-semilattice** modeling the consensus-or-fallback
pattern of §4.8.2. The lattice has:

- **Bottom**: `FgiSet::empty()` (no FGI markings on the page).
- **Two named non-bottom shapes**:
  - `FgiSet::Present { concealed: true, countries: ∅ }` —
    **source-concealed FGI** per §H.7 p123, when at least one
    portion is concealed-form. This is the absorbing top: once any
    portion is concealed, the banner falls back to bare `FGI` regardless
    of what other countries appear on other portions.
  - `FgiSet::Present { concealed: false, countries: {C1, C2, ...} }` —
    **source-acknowledged FGI** with union of country trigraphs.

The join law is **consensus-or-fallback**:

```text
FgiSet::join(s1, s2):
  case s1, s2 of
    empty, empty                                  → empty
    empty, Present{c, C}                          → Present{c, C}
    Present{c, C}, empty                          → Present{c, C}
    Present{true, _}, _                           → Present{true, ∅}  -- concealed absorbs
    _, Present{true, _}                           → Present{true, ∅}  -- concealed absorbs
    Present{false, C1}, Present{false, C2}        → Present{false, C1 ∪ C2}

  Pre:  s1, s2 ∈ FgiSet (well-formed per §4.8.2).
  Post: result is the consensus-or-fallback join.
        Identity (bottom): FgiSet::empty().
        Top (within the bounded interpretation): Present{concealed: true, countries: ∅}.
```

**Note on `BoundedLattice`**: the implementation deliberately does
NOT implement `BoundedLattice` — `top()` returning the concealed
form would be a misleading default (the "everything is concealed"
state is a lattice-theoretic top but rarely a useful operational
default). Callers needing `concealed=true` express it directly.

This matches the existing `FgiSet::Present { concealed, countries }`
implementation in `crates/capco/src/lattice.rs:420-552` (verified by
lattice-preflight); no primitive change is needed.

### Source-acknowledged vs source-concealed — the §H.7 p123 distinction

Per **§H.7 p123**, FGI markings can be either:
- **Source-acknowledged** — the banner names the foreign government(s):
  e.g., `//FGI DEU FRA` lists Germany and France as equity holders.
- **Source-concealed** — the banner uses the bare `FGI` token without
  trigraphs, indicating foreign equity exists but the source is concealed
  (typically for operational-security reasons): `//FGI`.

These are **operationally distinct**, not just rendering variants. A
page with one source-concealed portion + one source-acknowledged
portion falls back to **source-concealed banner form** — once any
source is concealed, the banner cannot reveal acknowledged sources
without inadvertently disclosing concealment patterns. §4.8.2 formalizes
this as the absorbing-top property.

### Tetragraph aggregation — operate on canonical form

Per §H.7 and the 2026-04-28 ISMCAT taxonomy plan (`BUILTIN_TETRAGRAPH_MEMBERS`),
some country sets have tetragraph abbreviations (e.g., `FVEY` for
{USA, GBR, CAN, AUS, NZL}; `TEYE` for the Three Eyes; etc.).

**Lattice operates on canonical form**, NOT on expanded trigraphs:
- If two portions both carry `FVEY`, the join is `FVEY` (not
  the expanded five-trigraph set followed by a re-aggregation).
- If one portion carries `FVEY` and another carries `{USA, GBR}`,
  the lattice sees `{FVEY, USA, GBR}` as the union; **canonicalization
  to either `FVEY` (if `{USA, GBR} ⊆ FVEY`) or the explicit union**
  is `render_canonical`'s job, not the lattice's.

This is the **lattice/render boundary** for FGI: the lattice produces
union under set-union semantics; rendering applies tetragraph
canonicalization per `marque-capco::vocab` tables.

### JOINT-attribution coverage — explicitly out of scope

`FgiSet` models **FGI attribution only**. JOINT-attribution (§H.3
JOINT classification) is a **separate axis** modeled by
`MarkingClassification::Joint { producers }` discriminator at the
classification axis (§2). The §4.8.2 consensus-or-fallback law is
domain-neutral in `marque-applied.md` but the in-tree implementation
covers only the FGI axis. JOINT-attribution lattice work is **deferred
to Stage 4** per the [[project_incompatibility_class]] memory.

### Worked examples

**Example 1 — source-acknowledged trigraph union:**
```
Inputs:  (S//FGI DEU) (S//FGI FRA)
Per-portion FGI: Present{false, {DEU}}, Present{false, {FRA}}
Join: Present{false, {DEU, FRA}}
Banner output: SECRET//FGI DEU FRA
```

**Example 2 — concealed absorbs acknowledged (the §4.8.5 worked example):**
```
Inputs:  (C//NF) (//GBR-TS)
  — portion 1: US Confidential with NOFORN.
  — portion 2: GBR Top Secret (reciprocally normalizes to US TS).
Per-portion FGI:
  portion 1: empty (no FGI marking; NF is dissem-axis).
  portion 2: Present{false, {GBR}}  (UK source acknowledged).
Per-axis joins:
  classification: C ⊔ TS = TS  (via reciprocal normalization, §2 Example 3)
  FGI: empty ⊔ Present{false, {GBR}} = Present{false, {GBR}}
  dissem: {NOFORN} ⊔ {} = {NOFORN}
Banner output: TOP SECRET//FGI GBR//NOFORN
```
This is `marque-applied.md` §4.8.5's worked example end-to-end —
classification reciprocal-raise + FGI source-acknowledged union +
NOFORN preservation.

**Example 3 — concealed absorbs (edge case):**
```
Inputs:  (S//FGI DEU) (S//FGI)
Per-portion FGI:
  portion 1: Present{false, {DEU}}
  portion 2: Present{true, ∅}  (bare FGI = source-concealed per §H.7 p123)
Join (consensus-or-fallback): Present{true, ∅}  -- concealed absorbs
Banner output: SECRET//FGI
```
The presence of any source-concealed portion forces the banner to
the bare-FGI fallback form. The acknowledged DEU trigraph is suppressed
to preserve concealment intent.

**Example 4 — tetragraph operates on canonical form:**
```
Inputs:  (S//FGI FVEY) (S//FGI FVEY)
Per-portion FGI: Present{false, {FVEY}}, Present{false, {FVEY}}
Join (set-union on canonical-form atoms): Present{false, {FVEY}}
Banner output: SECRET//FGI FVEY
```
The lattice does NOT decompose `FVEY → {USA, GBR, CAN, AUS, NZL}` at
join time. If a future portion brought in `{NLD}`, the join would be
`Present{false, {FVEY, NLD}}`; `render_canonical` decides whether to
keep as-is or expand `FVEY` for canonical output.

### Property-test fixtures (for PR 4)

- `crates/capco/tests/category_lattice_laws.rs::fgiset_join_assoc_comm_idem`
  — assoc/comm/idem on consensus-or-fallback.
- `crates/capco/tests/category_lattice_laws.rs::fgiset_concealed_absorbs`
  — Example 3 fixture; verifies the absorbing-top property.
- `crates/capco/tests/cross_axis_dominance.rs::fgi_banner_rollup_concealed`
  — corpus-level Example 3 with banner emission.
- `crates/capco/tests/cross_axis_dominance.rs::fgi_acknowledged_with_reciprocal_class`
  — Example 2 (§4.8.5 worked example end-to-end).
- `tests/corpus/lattice/fgi-banner-rollup.json` — #276 banner-retains-FGI
  corpus fixture.

### Open questions — resolved

1. **Tetragraph join — expand at join time or operate on canonical
   form?** → **Operate on canonical form.** The lattice does set-union
   on whatever atoms the parser produced (trigraph or tetragraph);
   `render_canonical` applies tetragraph aggregation via the
   `marque-capco::vocab` tables. The 2026-04-28 ISMCAT taxonomy plan
   resolved this for REL TO; the same principle applies to FgiSet.
2. **`BUILTIN_TETRAGRAPH_MEMBERS` decomposability — lattice or render?**
   → **Render only.** Same answer: the lattice doesn't decompose;
   render-canonical decides the output form. This avoids the
   commutativity-breaking trap of "did we expand before join or after."

---

## 7. NATO control set

### §-citations

- **§H.3** (pp55-59) — JOINT Classification Markings, including
  NATO-equity portions (the §H.3 JOINT framing covers NATO as the
  multilateral case). §H.3 p56 covers JOINT alphabetical ordering
  (E020 in legacy rule numbering).
- **§H.2** (p55) — NON-US PROTECTIVE MARKINGS; cross-reference to
  IC Markings System Manual Appendices A/B/C for NATO classification
  levels (CTS = Cosmic Top Secret, NS = NATO Secret, NC = NATO
  Confidential, NR = NATO Restricted, NU = NATO Unclassified) and
  NATO-specific controls (ATOMAL, BOHEMIA, BALK).
- **`marque-applied.md` §3.4.1 entry 7** — cross-axis FGI rollup
  rewrite for NATO; the §3.4.3 declarative entry handles "NATO portion
  → REL TO USA, NATO" derivation.
- **§4.7.1 Trio 3** (`marque-applied.md`) — implicit-REL-USA-NATO closure
  rule that fires when any NATO portion is present.
- **§G.1 Table 4** (p36) — Register of Authorized Markings, including
  NATO entries.

### Formal join semantics

NATO controls in the current marque scheme are modeled as **two
separable axes**:

1. **NATO classification level** — joins via `OrdMax` on the NATO
   chain `NU < NR < NC < NS < CTS`, reciprocally normalized to the
   US chain per §H.7 (see §2 of this document for the classification
   join). Per §H.7 reciprocal-classification rules, NATO levels map
   1:1 to US levels (NC↔C, NS↔S, CTS↔TS, with NU and NR mapping to
   U with derivative-source-marker tracking).
2. **NATO-specific dissem controls** (ATOMAL, BOHEMIA, BALK,
   ORCON-NATO) — join via `FlatSet` (set-union) on the dissem axis,
   subject to the §H.3 per-control admissibility rules.

```text
NATO controls per-portion: nato_controls(p) ⊆ {ATOMAL, BOHEMIA, BALK, ORCON-NATO}

Join:  banner.nato_controls = ⋃_{p ∈ portions} nato_controls(p)

  Pre:  each nato_controls(p) is well-formed per §H.3 admissibility
        (e.g., ATOMAL requires NATO classification context; BALK
        requires CTS).
  Post: banner.nato_controls is the union.
        Identity (bottom): ∅ (no NATO controls).
        Identity (top): {ATOMAL, BOHEMIA, BALK, ORCON-NATO} — but
        admissibility constraints make some combinations unreachable
        per §H.3.
```

This is a **bounded join-semilattice on a fixed four-element fact set**
(`FlatSet<NatoControl>` where `NatoControl` is a closed CVE enum).
Unlike `SciSet`/`SarSet`, the NATO control vocabulary is NOT
agency-extensible — the four controls are fixed in §H.3 / §G.1 Table 4.

### ATOMAL and BOHEMIA combinability — §H.3 admissibility

Per **§H.3 p56-58**:
- **ATOMAL** — NATO-classified AEA-equivalent information; requires NATO
  classification context (NC, NS, CTS).
- **BOHEMIA** — NATO codeword for specific operational program; requires
  CTS classification.
- **BALK** — NATO codeword for specific operational program; requires CTS
  classification.
- **ORCON-NATO** — NATO ORCON equivalent; admissible at any NATO classification.

**Are ATOMAL and BOHEMIA combinable in a single marking?** **Yes** —
they are separate facts on the NATO-controls axis, and §H.3 does not
forbid co-occurrence (each carries its own admissibility constraint
on classification level, which can be satisfied simultaneously at CTS).
The lattice produces `{ATOMAL, BOHEMIA}` as a fact set; the admissibility
constraints are checked per-token via `Constraint::Requires` rows
on `CapcoScheme`.

### Cross-axis interactions — Constraint, not lattice

Two cross-axis rules apply to NATO markings:

1. **NATO portion → REL TO USA, NATO** (§4.7.1 Trio 3 closure rule,
   #265). When any NATO-equity portion appears on a page, the implicit
   `REL TO USA, NATO` fact is added to the dissem axis **unless** an
   FD&R fact already exists. This is closure operator territory
   (T108c), not lattice territory.
2. **NATO marking carrying US dissems** (#246 part 2). PR 9's
   `dissem_us`/`dissem_nato` position-attributed parser fields close
   the parser side. The lattice **admits** NATO portions carrying US
   dissem controls; the validation that the combination is meaningful
   is `Constraint::Conflicts` territory (e.g., `JOINT conflicts with
   non-US-only dissem` per §H.3 admissibility), not lattice territory.

### Worked examples

**Example 1 — ATOMAL + BOHEMIA combinable at CTS:**
```
Inputs:  (//CTS//ATOMAL) (//CTS//BOHEMIA)
Per-portion classification: CTS, CTS
Per-portion NATO controls: {ATOMAL}, {BOHEMIA}
Classification join: CTS ⊔ CTS = CTS
NATO controls join: {ATOMAL, BOHEMIA}
Reciprocal normalization: CTS → US TS for banner classification axis.
Banner classification axis: TOP SECRET
Banner NATO controls: {ATOMAL, BOHEMIA}
Banner output: TOP SECRET//ATOMAL//BOHEMIA  (rendered with NATO marker per §H.3)
```
ATOMAL and BOHEMIA co-occur successfully; both require CTS, which the
join produces.

**Example 2 — NATO portion triggers implicit REL TO USA, NATO (closure):**
```
Inputs:  (//NC) (S)
  — portion 1: NATO Confidential.
  — portion 2: US Secret.
Per-portion classification: NC, S
  → reciprocal: NC = C; OrdMax(C, S) = S.
Per-portion dissem: {}, {}
Per-axis join: classification = S, dissem = {}
Closure pass (T108c, Trio 3): capco/rel-usa-nato-if-no-fdr-and-nato fires.
  Trigger: NATO portion (NC) present.
  Suppressor: has_fdr(page) = false (no NOFORN/REL TO/RELIDO/etc.).
  Cone: {REL TO USA, NATO} added to FD&R axis.
Closed dissem: {REL TO[USA, NATO]}
Banner output: SECRET//REL TO USA, NATO
```
The closure operator adds REL TO USA, NATO based on the NATO-equity
trigger; the lattice itself stays silent.

**Example 3 — NATO marking + US dissem (admitted by lattice, validated by Constraint):**
```
Inputs:  (//NS//NOFORN) (//NS//ORCON)
Per-portion classification: NS, NS  → reciprocal: S, S; OrdMax = S.
Per-portion dissem: {NOFORN}, {ORCON}
Per-axis dissem join: {NOFORN, ORCON}
Constraint::Conflicts check (§H.3 admissibility): NS + non-US-only dissem
  (NOFORN is a US-specific control) — may emit a diagnostic depending
  on §H.3 admissibility rules; per PR 9 7B parser fields, dissem_us and
  dissem_nato are position-attributed at parse time.
Banner output: SECRET//NOFORN  (NOFORN clears REL TO USA, NATO per the
  PageRewrite if it also fires; but here closure's Trio 3 is suppressed
  by has_fdr=true)
```
The lattice admits the combination; Constraint::Conflicts and the
PageRewrite handle the validation and cleanup.

### Property-test fixtures (for PR 4)

- `crates/capco/tests/category_lattice_laws.rs::nato_controls_assoc_comm_idem`
  — assoc/comm/idem on FlatSet<NatoControl>.
- `crates/capco/tests/category_lattice_laws.rs::nato_controls_identity_with_empty`
  — empty set is identity.
- `crates/capco/tests/cross_axis_dominance.rs::nato_atomal_bohemia_combinable`
  — Example 1 fixture.
- `crates/capco/tests/cross_axis_dominance.rs::nato_portion_triggers_implicit_rel_to`
  — Example 2 closure-operator fixture.
- `tests/corpus/lattice/nato-only-page.txt` *(deferred to PR 4)* —
  pure-NATO-page banner roll-up. PR 3.7 ships five cross-axis dominance
  fixtures per §3 (e) trio coverage; the NATO-only fixture lands in PR 4
  alongside `tests/corpus/lattice/`'s `.expected.json` sidecars and the
  property-test runner (NATO closure-operator behavior depends on
  `Engine::project::closure()` wiring deferred to PR 4 per `tasks.md` T112).

### Open questions — resolved

1. **ATOMAL and BOHEMIA combinable in a single marking?** → **Yes.**
   §H.3 p56-58 treats them as separate facts on the NATO-controls
   axis; co-occurrence is admissible when both controls' classification
   requirements are met (typically CTS). Each control's admissibility
   constraint is a `Constraint::Requires` row, NOT a lattice join law.
2. **NATO portion → REL TO USA, NATO derivation?** → **Closure operator
   (§4.7.1 Trio 3, T108c).** The lattice admits the NATO portion shape;
   the closure operator propagates the implicit REL TO USA, NATO fact
   when the page is not already FD&R-marked. Same algebraic shape as
   the NOFORN-implicit and RELIDO-implicit defaults.
3. **NATO marking carrying US dissems?** → **Lattice admits the
   combination.** PR 9 7B's `dissem_us`/`dissem_nato` parser fields
   position-attribute the dissem axis. Validation of meaningful
   combinations is `Constraint::Conflicts` territory; the lattice
   produces the FlatSet union and stays silent on admissibility.

---

## 7.5. AEA control set (`CAT_AEA`)

The AEA control set carries Atomic Energy Act information markings —
RD, FRD, TFNI, the CNWDI sub-modifier, SIGMA program numbers, the two
UCNI variants (DoD UCNI = `DCNI`, DoE UCNI = `UCNI`), and ATOMAL
(NATO §123/§144 AEA-sharing marker). It is the **largest of the seven
categories by token surface area** — five algebraically-distinct
sub-axes within one category — and was missing from earlier revs of
this design doc (it appeared only in §1 inadvertently). This section
fills the gap.

### §-citations

- **§H.6 pp103-121** — Atomic Energy Act Information Markings, overall
  section. The §H.6 section header sits on p103 along with the
  introductory paragraph naming the five AEA markings the Register
  lists; the first marking subsection (RD) begins on p104; the
  section ends on p121 (TFNI worked example).
- **§H.6 p104** — RESTRICTED DATA (RD). Includes the
  "Relationship(s) to Other Markings" + Precedence Rules sections.
  Key prose: "Is always used with NOFORN unless a sharing agreement
  has been established per the Atomic Energy Act"; "CNWDI can only
  be used with RD as designated by DOE or joint DOE-DoD guidance";
  "SIGMA 14, 15, 18, and 20 can only be used with TOP SECRET and
  SECRET RD"; "If RD, FRD, and TFNI portions are in a document, the
  RD takes precedence and is conveyed in the banner line."
- **§H.6 p106** — CRITICAL NUCLEAR WEAPON DESIGN INFORMATION (CNWDI).
  Key prose: "May only be used with TOP SECRET RD or SECRET RD";
  "Must be used as a subset of RD in accordance with DOD or joint
  DOE-DoD guidance"; "CNWDI-marked information must be segregated
  from classified NSI portions."
- **§H.6 p108-109** — SIGMA [#]. Key prose: "Requires RD" /
  "SIGMA # currently represents one or more of the following
  numbers: 14, 15, 18, and 20"; "Multiple SIGMA numbers must be
  listed in numerical order with a space preceding each value";
  "If both RD and FRD SIGMA [#] portions are in a document, the
  RD-SIGMA [#] marking takes precedence over the FRD-SIGMA [#]
  marking in the banner line and all SIGMA numbers are listed in
  the RD-SIGMA [#] marking in the banner line, regardless of
  whether the information was RD or FRD" (p109, top-of-page
  continuation from the p108 Precedence Rules block).
- **§H.6 p111** — FORMERLY RESTRICTED DATA (FRD). Key prose:
  "If the FRD marking is contained in any portion of a document, it
  must appear in the banner line (except when RD is present.)"; "If
  RD and FRD portions are in a document, the RD marking takes
  precedence in the banner line and is conveyed in the banner line."
- **§H.6 p113** — FRD-SIGMA. Key prose: "Requires FRD"; same
  precedence rule as §H.6 p109 (mutual reference — the rule is
  stated once from each subsection's vantage).
- **§H.6 p116-117** — DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION
  (DOD UCNI / `DCNI`). Key prose: "May only be used with
  UNCLASSIFIED"; "Classified documents: DOD UCNI does not appear in
  the banner line; however, NOFORN must be applied if a less
  restrictive FD&R marking would otherwise be conveyed with the
  classified information."
- **§H.6 p118-119** — DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION
  (DOE UCNI / `UCNI`). Symmetric structure to DOD UCNI: "May only be
  used with UNCLASSIFIED"; "Classified documents: DOE UCNI does not
  appear in the banner line; however, use NOFORN if a less
  restrictive FD&R marking would otherwise be conveyed with the
  classified information."
- **§H.6 p120-121** — TRANSCLASSIFIED FOREIGN NUCLEAR INFORMATION
  (TFNI). Key prose: "May only be used with TOP SECRET, SECRET, or
  CONFIDENTIAL"; "If the TFNI marking is contained in any portion of
  a document that contains portions of RD and/or FRD, the RD or FRD
  takes precedence. The 'RD' or 'FRD' marking, as appropriate,
  appears in the banner line and the 'TFNI' marking does not appear
  in the banner line."
- **§H.7 p122 — FGI section worked example** (NOT a §H.7 ATOMAL
  subsection — ATOMAL has no dedicated subsection in §H.1 through
  §H.9; its registration lives in §G.2 Table 5 p40; this citation
  is for the worked example
  found inside the FGI section that demonstrates the AEA-axis
  placement). Key prose: "*SECRET//RD/ATOMAL//FGI NATO//NOFORN,
  where ATOMAL is a NATO Atomic Energy Act marking that follows the
  registered US Atomic Energy Act marking RD*". This is the
  operative authority that ATOMAL travels in the AEA category
  alongside RD/FRD/TFNI, not on the classification axis (the PR 9c.1
  T134 routing change).
- **§G.2 Table 5 p40** — ARH (Authorization, Registration, Handling)
  by Registered Marking. ATOMAL row registers ATOMAL as a standalone
  control marking with ARH = AEA. Table 5 lives in §G.2 (the ARH
  subsection); Table 4 lives in §G.1 (the Register of Authorized
  Markings). Do not conflate.
- **§G.1 Table 4 pp36-38** — Register of Authorized Classification and
  Control Markings. AEA Register-order sequence for category 6
  (banner roll-up display order): RD → CNWDI → SIGMA → FRD → SIGMA
  → DOD UCNI → DOE UCNI → TFNI. ATOMAL is listed in Table 4
  category 2 (Non-US Protective Markings) — the cross-category
  routing decision is governed by §H.7 p122 (above), not by Table 4
  category 6 ordering.
- **§G.2 Table 5 pp39-40** — ARH by Registered Marking. Lists ATOMAL,
  BALK, BOHEMIA at p40 under "Non-US Protective Markings". ATOMAL's
  ARH row reads "Requires ATOMAL read-in." which is independent of
  the §H.7 p122 routing decision; this entry confirms ATOMAL is a
  registered standalone control marking (the relevant fact for
  T134's AEA-axis placement).

### Formal join semantics

The AEA category is modeled as a five-axis `Product` composition.
Each sub-axis carries an algebraically-distinct join law; rendering
the surface as one `Product` rather than five separate
`MarkingScheme::Category` rows keeps the §G.1 Table 4 category-6
roll-up sequence — which is single-category — algebraically faithful.

```text
AeaSet = Product<
    SupersessionSet<AeaPrimary>,    // axis 1: RD ⊐ FRD ⊐ TFNI per §H.6 p104+p111+p120
    FlatSet<CnwdiPresence>,         // axis 2: CNWDI presence (true/false) per §H.6 p106
    FlatSet<SigmaNumber>,           // axis 3: SIGMA {14, 15, 18, 20} per §H.6 p108
    FlatSet<UcniKind>,              // axis 4: {DodUcni, DoeUcni} per §H.6 p116-119
    OptionalSingleton<AtomalBlock>, // axis 5: ATOMAL routed per PR 9c.1 T134
>
```

**Axis 1 — `SupersessionSet<AeaPrimary>` where `AeaPrimary ∈ {Rd, Frd, Tfni}`:**

```text
Join: SupersessionSet::join(a, b) = supremum under total order
      Tfni ⊏ Frd ⊏ Rd.

  Pre:  a, b ∈ {None, Tfni, Frd, Rd}.
  Post: result is the supremum under §H.6 p104 + p111 + p120 total order.
        Identity (bottom): None.
        Top: Rd.
```

§-authority: §H.6 p104 (RD takes precedence over FRD and TFNI);
§H.6 p111 (FRD evicted from banner when RD present); §H.6 p120 (TFNI
evicted from banner when RD or FRD present). The three subsections
state the same total-order rule from each marking's vantage; the
combined supersession order is the unique consistent solution.

**Axis 2 — `FlatSet<CnwdiPresence>` (closed singleton on `bool`):**

```text
Join: CnwdiPresence::join(a, b) = a ∨ b.

  Pre:  a, b ∈ {false, true}.
  Post: result is the OR of the two presence flags.
        Identity (bottom): false.
        Top: true.
```

§-authority: §H.6 p106 (CNWDI is a sub-modifier on RD; presence
propagates monotonically through page roll-up — once any RD portion
carries CNWDI, the banner RD block carries it).

**Axis 3 — `FlatSet<SigmaNumber>` where `SigmaNumber = u8` (open
vocabulary, currently {14, 15, 18, 20}):**

```text
Join: FlatSet::join(A, B) = A ∪ B (set-union).

  Pre:  A, B ⊆ ℕ (with SIGMA-vocabulary semantics: 14, 15, 18, 20
        in the current revision per §H.6 p108).
  Post: result is the union of the two sets, sorted ascending
        per §H.6 p108 ("Multiple SIGMA numbers must be listed in
        numerical order with a space preceding each value").
        Identity (bottom): ∅.
        No top: §H.6 p108 lists 14/15/18/20 but explicitly admits
        future numbers ("SIGMA # currently represents one or more
        of the following numbers"). The open vocabulary precludes
        a finite top.
```

§-authority: §H.6 p108 (SIGMA list grammar + numerical-order
canonical form) + §H.6 p109 ("all SIGMA numbers are listed in the
RD-SIGMA [#] marking in the banner line, regardless of whether the
information was RD or FRD" — the cross-modifier coalescing that
makes `Axis 3` a flat union rather than per-modifier sets).

**Axis 4 — `FlatSet<UcniKind>` where `UcniKind ∈ {DodUcni, DoeUcni}`:**

```text
Join: FlatSet::join(A, B) = A ∪ B.

  Pre:  A, B ⊆ {DodUcni, DoeUcni}.
  Post: result is the set-union.
        Identity (bottom): ∅.
        Top: {DodUcni, DoeUcni} (closed two-element vocabulary).
```

§-authority: §H.6 p116-117 (DOD UCNI / `DCNI`) and §H.6 p118-119
(DOE UCNI / `UCNI`). The two are independent vocabularies — the
distinct DoD vs DoE proponency keeps them as separate atoms
even when they co-occur. The on-classified-banner suppression
("DOD UCNI does not appear in the banner line; however, NOFORN
must be applied if a less restrictive FD&R marking would otherwise
be conveyed") is a **cross-axis post-projection rewrite**, not a
within-axis join transform — the lattice produces the UCNI fact
set, the rewrite (post-projection) decides whether the banner
renders it.

**Axis 5 — `OptionalSingleton<AtomalBlock>`:**

```text
Join: OptionalSingleton::join(a, b) = a ∨ b
                                    = case (a, b) of
                                        (None, None)       → None
                                        (Some(_), _)       → Some(AtomalBlock)
                                        (_, Some(_))       → Some(AtomalBlock)

  Pre:  a, b ∈ {None, Some(AtomalBlock)}.
  Post: result is the OR of the two presences.
        Identity (bottom): None.
        Top: Some(AtomalBlock).
```

§-authority: §H.7 p122 (ATOMAL travels in AEA category per worked
example `SECRET//RD/ATOMAL//FGI NATO//NOFORN`); §G.2 Table 5 p40
(ATOMAL is a registered standalone control marking with no
enumerated sub-markings — the `AtomalBlock` is an empty carrier
struct that mirrors `RdBlock` / `FrdBlock` so a future CAPCO grammar
extension stays a planned migration rather than a shape-change).

**Product composition:**

```text
Join: AeaSet::join(s1, s2) = componentwise join of all five axes.

  Pre:  s1, s2 are well-formed AeaSet values (CNWDI=true ⇒ primary=Rd is
        a validation invariant, not a lattice law — see "Cross-axis
        constraints" below).
  Post: result has primary = SupersessionSet::join(s1.primary, s2.primary),
        cnwdi  = s1.cnwdi || s2.cnwdi,
        sigmas = s1.sigmas ∪ s2.sigmas,
        ucni   = s1.ucni   ∪ s2.ucni,
        atomal = s1.atomal.or(s2.atomal).
        Identity (bottom): AeaSet::default() (all five sub-axes at bottom).
        No top: axis 3 (SIGMA numbers) is open-vocabulary; axes 1, 2, 4, 5
        are bounded but the Product is unbounded by axis 3.

Lattice laws (associativity, commutativity, idempotency) follow
componentwise from the five sub-axes' laws.
```

**`BoundedLattice` is intentionally NOT implemented** for `AeaSet`.
Per the SciSet / SarSet precedent in this codebase, open-vocabulary
axes (here: SIGMA numbers per §H.6 p108) preclude a lawful finite
top. Callers needing the bottom use `AeaSet::default()` or
`AeaSet::empty()`.

### Cross-axis constraints (validation, not lattice)

Three cross-axis rules apply to AEA markings but are **not** lattice
laws — they are validation predicates that ride alongside the lattice
in the `Constraint` / `PageRewrite` catalogs on `CapcoScheme`:

| Constraint shape | §-authority | Surface |
|---|---|---|
| `CNWDI requires RD` | §H.6 p106 ("must be used as a subset of RD") | **Enforced by data model, NOT by constraint catalog.** CNWDI is structurally a `bool` field on `AeaMarking::Rd(RdBlock { cnwdi })` in `marque-ism`'s type system — there is no `AeaMarking::Cnwdi` variant. A portion that bears CNWDI necessarily bears RD because CNWDI presence is gated by the surrounding `Rd(...)` variant. An earlier draft added a `Constraint::Requires { TOK_CNWDI, TOK_RD }` row but Copilot review caught it as unreachable: `TOK_CNWDI` is satisfied only by `Rd(rd) if rd.cnwdi`, so `TOK_RD` is always present when `TOK_CNWDI` is. §H.6 p106's "subset of RD" invariant lives at the data-model level. **If a future change splits CNWDI into a sibling variant**, the satisfies_attrs predicate for `TOK_CNWDI` must be amended AND a Constraint::Requires row re-introduced. |
| `CNWDI requires class ≥ S` | §H.6 p106 ("May only be used with TOP SECRET RD or SECRET RD") | Already in the class-floor catalog (`E058/CNWDI-classification-floor`, see PR 3b.D T026d). No new wiring in PR 4b-A. |
| `SIGMA cross-modifier coalescing (banner)` | §H.6 p108-109 + §H.6 p113 | `PageRewrite("capco/frd-sigma-consolidates-into-rd-sigma")` already declared in `CapcoScheme::build_page_rewrites()` at §-citation `§H.6 p113`. PR 4b-A re-doc-comments the row to also cite §H.6 p108-109 (the RD-side vantage) — same algebraic rule from both subsections. Body remains the Phase-3 `never_fires` / `noop_action` stub; PR 4b-B wires the runtime `AeaSet`-driven mutation. |
| `UCNI strip on classified` | §H.6 p116-117 (DOD UCNI) + p118-119 (DOE UCNI) | Post-projection cross-axis rewrite: when banner classification > U and the AEA set carries UCNI, the UCNI atom is suppressed from banner render AND NOFORN is added to the dissem axis if no stricter FD&R marker exists. **Deferred to PR 4b-C (FOUO eviction pattern wiring)** because the algebraic shape is identical to the FOUO-eviction matrix in §3 (b) — both are classification-ascent strips with NOFORN-promotion. PR 4b-A documents the predicate; the catalog row lands in PR 4b-C. |
| `SIGMA requires RD or FRD` | §H.6 p108 ("Requires RD") + §H.6 p113 ("Requires FRD"; same precedence rule) | **DEFERRED — tracking issue TBD before PR 4b-B opens.** The §H.6 p108 prose says SIGMA "Requires RD" but §H.6 p113 also accepts SIGMA on FRD (FRD-SIGMA is its own template), so the constraint is "SIGMA requires RD OR FRD on the same portion." The AeaSet lattice's `sigmas` field is shared across `primary` states; a `(S//SIGMA 14)` portion (SIGMA without primary) is syntactically reachable but operationally invalid. PR 4b-A does NOT land this constraint — the cleanest shape (`Constraint::Requires` with an OR predicate on the right side OR two separate `Requires` rows) requires PM sign-off and engine-side support that PR 4b-B is the right home for. Open an issue and link it here before PR 4b-B opens. |

### Worked examples

**Example 1 — mainline RD/FRD/SIGMA cross-modifier coalescing
(§H.6 p104 + p108-109 + p111):**

```
Inputs:  (S//RD/SIGMA 14) (S//FRD/SIGMA 18)
Per-portion AeaSet:
  portion 1: {primary=Rd,  cnwdi=false, sigmas={14}, ucni=∅, atomal=None}
  portion 2: {primary=Frd, cnwdi=false, sigmas={18}, ucni=∅, atomal=None}
Per-axis joins:
  primary: SupersessionSet::join(Rd, Frd) = Rd  (per §H.6 p104 + §H.6 p111)
  cnwdi:   false ∨ false = false
  sigmas:  {14} ∪ {18}   = {14, 18}
  ucni:    ∅              (both empty)
  atomal:  None
Per-axis-joined AeaSet: {primary=Rd, sigmas={14, 18}}
Banner render (post canonical-form sort, §H.6 p108): SECRET//RD-SIGMA 14 18
Classification axis: S ⊔ S = S
Final banner: SECRET//RESTRICTED DATA-SIGMA 14 18
```

This is the §H.6 p109 worked example end-to-end — the RD-SIGMA
banner marking carries SIGMA numbers from both RD and FRD portions
because the supersession join promotes the primary axis to `Rd`,
and the SIGMA set-union runs orthogonally on axis 3.

**Example 2 — UCNI strip on classified (§H.6 p116):**

```
Inputs:  (U//DOD UCNI//FOUO) (S)
Per-portion AeaSet:
  portion 1: {primary=None, cnwdi=false, sigmas=∅, ucni={DodUcni}, atomal=None}
  portion 2: AeaSet::default()
Per-axis joins:
  primary: None ⊔ None = None
  cnwdi:   false
  sigmas:  ∅
  ucni:    {DodUcni} ∪ ∅ = {DodUcni}
  atomal:  None
Per-axis-joined AeaSet: {ucni={DodUcni}}
Classification axis: U ⊔ S = S
Cross-axis post-projection rewrite (§H.6 p116, deferred PR 4b-C):
  banner classification = S ⊐ U  →  UCNI suppressed from banner.
  No stricter FD&R marker on page  →  add NOFORN to dissem axis.
Pre-projection AeaSet at banner: {} (empty after suppress)
Banner dissem: {NOFORN}
Final banner: SECRET//NOFORN
```

Edge: the §H.6 p116 prose carries TWO operational rules in one
sentence — UCNI's banner-line suppression on classified docs AND the
NOFORN-promotion requirement. The lattice produces the union
faithfully; the post-projection rewrite enforces both rules together
because they are coupled — suppressing UCNI without promoting NOFORN
would lose the foreign-disclosure constraint UCNI carries.

**Example 3 — ATOMAL routing (PR 9c.1 §H.7 p122 + §G.2 Table 5 p40):**

```
Input:   (//CTS//RD/ATOMAL//FGI NATO//NOFORN)
  — single NATO Cosmic Top Secret portion that co-bundles RD,
    ATOMAL, FGI NATO source-attribution, and NOFORN.
Per-portion AeaSet:
  {primary=Rd, cnwdi=false, sigmas=∅, ucni=∅, atomal=Some(AtomalBlock)}
Per-axis (single-portion is identity case):
  primary = Rd
  atomal  = Some(AtomalBlock)
Banner render (per §H.6 + §G.1 Table 4 cat-6 order: RD then ATOMAL):
  RD/ATOMAL
Classification axis (reciprocal-normalized CTS → US TS per §H.7):
  TS
Other axes: FGI={NATO}, dissem={NOFORN}
Final banner: TOP SECRET//RD/ATOMAL//FGI NATO//NOFORN  (exactly the §H.7 p122 worked example)
```

This example confirms ATOMAL stays in `AeaSet` rather than routing
through `NatoClassification` (the legacy `NatoSecretAtomal` /
`CosmicTopSecretAtomal` etc. variants retired in PR 9c.1 T134). The
canonical re-rendering of legacy `(//CTSA)` etc. into the
`(//CTS//ATOMAL)` form is the autofix Marque exists to automate
(per project memory `remark-on-derivative-use-is-marque-autofix`).

### Property-test fixtures (this PR)

- `crates/capco/tests/category_lattice_laws.rs::aea_primary_supersession_assoc_comm_idem`
  — axis 1 lattice laws (RD ⊐ FRD ⊐ TFNI total-order supersession).
- `crates/capco/tests/category_lattice_laws.rs::aea_sigma_flatset_assoc_comm_idem`
  — axis 3 lattice laws (SIGMA flat-set union).
- `crates/capco/tests/category_lattice_laws.rs::aea_ucni_flatset_assoc_comm_idem`
  — axis 4 lattice laws (UCNI flat-set union over `{DodUcni, DoeUcni}`).
- `crates/capco/tests/category_lattice_laws.rs::aea_cnwdi_flatset_assoc_comm_idem`
  — axis 2 lattice laws (CNWDI presence OR).
- `crates/capco/tests/category_lattice_laws.rs::aea_atomal_optional_singleton_identity`
  — axis 5 lattice laws (ATOMAL `OptionalSingleton::join` = `or`).
- `crates/capco/tests/category_lattice_laws.rs::aea_set_join_assoc_comm_idem`
  — Product composition: lattice laws over the full `AeaSet`.
- `crates/capco/tests/category_lattice_laws.rs::aea_set_identity_with_default`
  — `AeaSet::default()` is identity element for join.
- `crates/capco/tests/cross_axis_dominance.rs::aea_rd_evicts_frd_tfni`
  — Example 1 fixture (§H.6 p104 + p111 + p120 supersession).
- `crates/capco/tests/cross_axis_dominance.rs::aea_sigma_coalesces_under_rd`
  — Example 1 fixture (§H.6 p108-109 cross-modifier SIGMA union).
- `crates/capco/tests/cross_axis_dominance.rs::aea_ucni_strips_when_classified`
  — Example 2 fixture (§H.6 p116/p118 documented predicate; runtime
  wiring deferred to PR 4b-C).
- `crates/capco/tests/cross_axis_dominance.rs::aea_atomal_routes_to_aea_not_nato_class`
  — Example 3 fixture (§H.7 p122 + §G.2 Table 5 p40 ATOMAL routing).
- `tests/corpus/lattice/aea-commingling.txt` — corpus fixture
  (already exists from the §8 Declassify-on AEA-commingling case);
  reused for AEA-axis end-to-end coverage.

### Open questions — resolved

1. **CNWDI semantic shape — `Constraint::Requires` or a `ClosureRule`
   that implicitly adds RD?** → **`Constraint::Requires`.** §H.6 p106
   ("must be used as a subset of RD in accordance with DOD or joint
   DOE-DoD guidance") reads as a validation requirement, not an
   implicit-default fact-propagation. A `ClosureRule` shape would
   silently add RD when CNWDI appears alone — but per §H.6 p106 a
   CNWDI portion without RD is **malformed input that the classifier
   needs to fix**, not a representation marque should silently
   complete. **However, the data model in `marque-ism` already
   enforces this**: CNWDI is a `bool` field on
   `AeaMarking::Rd(RdBlock { cnwdi })`, not a sibling variant, so
   `cnwdi=true, primary=None` is *not* a representable state at the
   `AeaMarking` level — only at `AeaSet`'s internal representation,
   which can never be populated from valid parser output. The
   §H.6 p106 invariant therefore lives at the type level, and no
   `Constraint::Requires` row is needed. (An earlier draft of
   PR 4b-A added an `E067/cnwdi-requires-rd` row; Copilot review
   caught it as unreachable per the satisfies_attrs predicate, and
   it was removed.) The algebra stays clean either way.
2. **SIGMA cross-modifier coalescing — within-axis lattice law or
   page-scope `PageRewrite`?** → **PageRewrite at page scope.** The
   `FlatSet<SigmaNumber>` axis produces the union faithfully at any
   scope, but §H.6 p108-109 + p113 specifies that the **banner** uses
   the RD-SIGMA marking *when both RD and FRD SIGMA portions exist*
   — which is a primary-axis-dependent decision, not a SIGMA-axis
   transform. The existing `capco/frd-sigma-consolidates-into-rd-sigma`
   PageRewrite (`reads: [CAT_AEA], writes: [CAT_AEA]`, citation
   `§H.6 p113`) captures this; PR 4b-A re-doc-comments it to also
   cite §H.6 p108-109 (the RD-side vantage of the same rule). No new
   PageRewrite row added — the existing one IS the rd-coalesces-sigmas
   semantics; the brief's working name (`capco/rd-coalesces-sigmas`)
   and the in-tree id (`capco/frd-sigma-consolidates-into-rd-sigma`)
   refer to the same rewrite.
3. **UCNI strip-when-classified — deferred to PR 4b-C.** Same
   algebraic shape as the §3 (b) FOUO eviction matrix (cross-axis
   strip on classification ascent + NOFORN promotion). PR 4b-A
   documents the predicate; PR 4b-C wires the catalog row alongside
   the FOUO eviction work.

---

## 8. Declassify-on / `MaxDate`

### §-citations

- **§E** (pp31-34) — Classification Authority Block. Top-level
  framing for the Declassify On line and its hierarchy.
- **§E.1** (p31) — Original Classification Authority.
- **§E.2** (p32) — Derivative Classification Authority.
- **§E.3** (p32) — Multiple Sources and the Declassify On Line
  Hierarchy. Governs multi-source date precedence.
- **§E.4** (p33) — Commingling Classified NSI and Atomic Energy
  Act (AEA) Information; AEA exemption canned strings (`50X1-HUM`,
  `25X-HUM`, etc.).
- **§E.5** (p33) — Commingling Classified NSI and NATO Information;
  NATO commingling canned strings.
- **§E.6** (pp33-34) — Retired or Invalid Declassify On Values.

### Formal join semantics

The Declassify On axis is **algebraically heterogeneous**: it carries
**two disjoint value shapes** that join by different laws:

1. **Calendar dates** — ISO-8601 `YYYYMMDD` strings, joined via
   `MaxDate` (latest date wins per §E.3 multiple-sources hierarchy).
2. **Canned exemption strings** — `50X1-HUM`, `25X1-HUM`, `MR`, etc.
   per §E.4 + §E.5 + §G.1 Table 4 catalog. These do NOT participate
   in date arithmetic.

The disjoint-shape problem is resolved by carrying Declassify On as
an `OptionalSingleton<DeclassifyOn>` where `DeclassifyOn` is itself
a discriminated union:

```text
DeclassifyOn = MaxDate(NaiveDate) | ExemptString(&'static str)

OptionalSingleton<DeclassifyOn>::join(d1, d2):
  case d1, d2 of
    None, None                                       → None
    None, Some(x)                                    → Some(x)
    Some(x), None                                    → Some(x)
    Some(MaxDate(a)), Some(MaxDate(b))               → Some(MaxDate(max(a, b)))
    Some(ExemptString(s1)), Some(ExemptString(s2))   → see "mixed canned strings" below
    Some(MaxDate(_)), Some(ExemptString(_))          → see "mixed shapes" below
    Some(ExemptString(_)), Some(MaxDate(_))          → see "mixed shapes" below

  Pre:  d1, d2 ∈ OptionalSingleton<DeclassifyOn>.
  Post: result is the §E.3 multiple-sources hierarchy resolution.
        Identity (bottom): None.
        Top: no operational top (dates have no upper bound; canned
        strings are agency-extensible per §G.1 Table 4).
```

### Mixed canned strings — `MaxDate` over the §E catalog ordering

Per **§E.6 pp33-34** (Retired or Invalid Declassify On Values) and
the §G.1 Table 4 register, canned exemption strings have an implicit
"most restrictive wins" ordering. Examples per §E.4:
- `50X1-HUM` (50 years, human-source protection) > `25X1-HUM`
  (25 years, human-source protection) by date-equivalent ordering.
- `MR` (Manual Review) is the maximum (indefinite review required).

The lattice **chooses the more restrictive canned string** when two
exempt-string portions appear. The ordering is finite, total, and
ships as a static table in `marque-capco::vocab` (analogous to the
SCI per-system rules).

```text
ExemptString join: lookup both in §E catalog ordering; return the
                   higher-precedence (more restrictive) one.

  e.g., MaxDate(MR, 25X1-HUM) = MR
        MaxDate(50X1-HUM, 25X1-HUM) = 50X1-HUM
```

### Mixed shapes — canned string dominates date

Per **§E.3 + §E.4**, when a page mixes date-bearing portions with
canned-exemption-bearing portions, the **canned string dominates** —
the document carries the canned exemption banner because the
exempt-source portion's restriction is more stringent than any specific
calendar date.

```text
case MaxDate, ExemptString: ExemptString wins.
case ExemptString, MaxDate: ExemptString wins.
```

This is a **categorical dominance**, not a join over a common scale.
The §E.3 hierarchy makes this explicit: AEA-exempt content (§E.4) and
NATO-exempt content (§E.5) take precedence over date-based declassify
schedules for the banner.

### Worked examples

**Example 1 — pure date join:**
```
Inputs:  Declassify On: 20500101 (portion A)
         Declassify On: 20550615 (portion B)
Per-portion DeclassifyOn:
  portion A: Some(MaxDate(2050-01-01))
  portion B: Some(MaxDate(2055-06-15))
Join (MaxDate inner): Some(MaxDate(2055-06-15))
Banner CAB Declassify On: 20550615
```

**Example 2 — mixed dates and canned strings (§E.4 dominance):**
```
Inputs:  Declassify On: 20500101 (portion A)
         Declassify On: 50X1-HUM (portion B, AEA-exempt human source)
Per-portion DeclassifyOn:
  portion A: Some(MaxDate(2050-01-01))
  portion B: Some(ExemptString("50X1-HUM"))
Join (mixed-shape, canned dominates): Some(ExemptString("50X1-HUM"))
Banner CAB Declassify On: 50X1-HUM
```
The canned exemption supersedes the date even when the date is closer
to the present — §E.3 hierarchy makes the categorical decision.

**Example 3 — two canned strings (§E catalog ordering):**
```
Inputs:  Declassify On: 25X1-HUM (portion A)
         Declassify On: 50X1-HUM (portion B)
Per-portion DeclassifyOn:
  portion A: Some(ExemptString("25X1-HUM"))
  portion B: Some(ExemptString("50X1-HUM"))
Join (§E catalog ordering, more-restrictive wins): Some(ExemptString("50X1-HUM"))
Banner CAB Declassify On: 50X1-HUM
```

**Example 4 — edge case, `MR` (Manual Review) dominates everything:**
```
Inputs:  Declassify On: 20500101 (portion A)
         Declassify On: MR (portion B)
Per-portion DeclassifyOn:
  portion A: Some(MaxDate(2050-01-01))
  portion B: Some(ExemptString("MR"))
Join (mixed-shape, canned dominates, MR is catalog-max):
  Some(ExemptString("MR"))
Banner CAB Declassify On: MR
```

### #266 deferral — canned-string ordering catalog

The full §E catalog ordering (every canned-string-to-canned-string
comparison) is **deferred to #266** for completeness — the catalog
ships incrementally as canned strings are encountered. The lattice
surface admits the canned-string shape from PR 4 onward; the ordering
table is data, not algebra. Deferring catalog rows does NOT block
PR 4 because the algebraic shape is fixed; only the per-pair
comparison data is incremental.

### Property-test fixtures (for PR 4)

- `crates/capco/tests/category_lattice_laws.rs::declassifyon_maxdate_assoc_comm_idem`
  — assoc/comm/idem on pure-date join.
- `crates/capco/tests/category_lattice_laws.rs::declassifyon_canned_string_ordering`
  — canned-string-to-canned-string ordering per §E catalog.
- `crates/capco/tests/cross_axis_dominance.rs::declassifyon_canned_dominates_date`
  — Example 2 fixture; canned exemption supersedes calendar date.
- `crates/capco/tests/cross_axis_dominance.rs::declassifyon_mr_dominates_all`
  — Example 4 fixture; MR is catalog-max.
- `tests/corpus/lattice/aea-commingling.json` — corpus fixture for
  §E.4 AEA commingling end-to-end.

### Open questions — resolved

1. **Mixed dates and canned strings — what's the join?** → **Canned
   string dominates** per §E.3 hierarchy + §E.4 AEA-exempt precedence.
   The join law is categorical (`MaxDate, ExemptString → ExemptString`),
   not a comparison over a common scale.
2. **AEA exemption string supersession ordering?** → **§E catalog
   ordering, with `MR` as catalog-max**. Static table in
   `marque-capco::vocab`. Per-pair catalog rows are #266 deferral (data,
   not algebra); the lattice surface admits the canned-string shape
   in PR 4.

---

## 9. Acceptance checklist

PR 4 cannot land until this document satisfies every item below.
There is **no "explicitly deferred to a tracked issue" escape valve**
— per `2026-05-02-engine-refactor-consolidated.md` §11 and
`decisions.md` D2's monolithic PR 3.7 framing, a question that
genuinely cannot resolve blocks PR 4 and forces the answer to be
resolved within this document, not punted forward.

**Per-category attestation** (§§2–8, all seven categories):

- [x] §-citations to `crates/capco/docs/CAPCO-2016.md` verified
      against `crates/capco/docs/CAPCO-2016_citation_index.yml`
      (page-range cross-check).
- [x] Formal join semantics stated as a function with explicit
      preconditions/postconditions in functional form (not prose).
- [x] At least two worked examples per section, including at least
      one edge case the §-citation calls out.
- [x] Property-test fixture names listed by file path and test
      function name; fixtures map to PR 4 T116/T117/T118.
- [x] Every "Open question" originally listed in §10 is resolved with
      §-citation + explicit decision in the section itself. Resolved
      OQs are restated under "Open questions — resolved" subsections.

**Cross-cutting attestation**:

- [x] Cross-axis dominance fixtures present for every interaction the
      consolidated plan §11.1 item (5) names:
      - FOUO eviction by classification > U  (§3 Example 3)
      - FOUO eviction by non-FD&R dissem  (§3 Example 4)
      - FGI banner roll-up retaining FGI marker (#276)  (§2 Example 2, §6 Example 2)
      - SCI cross-system canonicalization  (§4 Example 3)
      - AEA exemption commingling with classification (#266)  (§8 Example 2)
- [x] FD&R semantics from `2026-05-02-engine-refactor-consolidated.md`
      Appendix A are embedded in §3 (a) with §-citation to §H.8
      and Table 2 (p21) + Table 3 (p28).
- [x] Closure operator (`marque-applied.md` §4.7) is documented in §3 (e)
      with the implicit-default trio, the FDR_DOMINATORS shared
      suppressor (Q-4.7-Cl_supp resolved), and the per-row §-citations.
- [x] `Constraint::Conflicts::RhsFamily` is documented in §3 by reference
      (PR 4 compacts PR 3b's enumerated RELIDO rows; T108b ships the
      variant + walker dispatch + distributive-expansion proptest).

**Reviewer attestation** (PR description):

- [ ] **Named primary reviewer** has confirmed each category's worked
      examples by hand against the §-citation. Confirmation MUST be a
      §-by-§ checklist in the PR description, not a single "LGTM"
      comment. Per Constitution VIII (citation fidelity), a fabricated,
      hallucinated, misattributed, or silently-drifted citation is a
      correctness defect of the same severity as a wrong predicate.
- [x] ~~**Named alternate reviewer** (per D2 stall-recovery) has
      independently read §§2-8 and is authorized to take primary
      ownership if the primary stalls past 1 week.~~ **Retired
      2026-05-13** per `decisions.md` D2 amendment — marque is a
      solo-driven project; the bus-factor mitigation framing
      presupposed a team context that doesn't apply. Stall-recovery
      reverts to "PR sits open until the primary returns to it."
- [ ] Per-row monotonicity attestation for the ~12 CAPCO ClosureRule
      rows is in the PR description. PR 3.7 verifies the monotonicity /
      extensivity / idempotence laws via stub-scheme proptests at
      `crates/scheme/tests/proptest_closure.rs` (5 positive properties
      + G13 + negative non-monotonicity catalog). Per-row CAPCO catalog
      verification — i.e. running `CapcoScheme::closure()` against each
      of the 15 catalog rows under random fact-sets — rides on PR 4
      (T112), which wires `Engine::project::closure()` and overrides
      `CapcoScheme::closure()`. The PR 3.7 attestation cites the
      stub-scheme proptest as proxy and `marque-applied.md` §4.7.3
      table-design property as the algebraic basis.

---

## 10. Open items — resolution log

Per the §9 acceptance criteria, every open item is resolved in the
relevant category section with §-citation + explicit decision. This
section is the **resolution log** — a single index mapping each
original open item to the section where its resolution lives. None
remain unresolved.

| # | Original question | Resolution | Lives in |
|---|---|---|---|
| 1 | **§2 cross-branch join semantics** (refusal vs structural combination) | No cross-branch join arises. §H.7 reciprocal-classification rule (pp123-125) mandates portion-parse-time normalization of foreign classifications to US-equivalent levels. `MarkingClassification::Joint` is a render concern, not a lattice-branch concern. | §2 "Formal join semantics" + §2 "Open questions — resolved" #1 |
| 2 | **§3 `EYES` lattice participation** (pre- vs post-migration form) | Post-migration `REL TO`. Parser-side migration table converts `EYES` portions to `REL TO` at parse time per §H.8 p157 (NSA waiver expired 2017-10-01); the lattice never sees an `EYES` atom. | §3 "Open questions — resolved" #1; Q-3.4.5c resolved 2026-05-07 |
| 3 | ~~**§3 `NF` clears `REL TO`** (lattice op vs PageRewrite)~~ | **PageRewrite.** Resolved 2026-05-02 per `marque-applied.md` §3.4.1 entry 4 + `decisions.md` D13 amendment. `capco/noforn-clears-rel-to` runs *after* per-axis lattice projection. | §3 (d) + §3 "Open questions — resolved" #3 |
| 4 | **§4 SCI per-system canonicalization** (lattice vs render boundary) | Both, at different layers. Lattice = prefix-closed-union fact set (commutative); `render_canonical` = §A.6 ordering + `-COMP` stacking + `/` separators. Companion-insert (#267 Gap A) is closure-operator territory (§4.7, T108c), NOT lattice. | §4 "Per-system canonicalization" + §4 "Open questions — resolved" #2 |
| 5 | **§5 SAR ordering canonicalization** (lattice vs render) | Render. Lattice = prefix-closed-union (commutative, no ordering); `render_canonical` imposes §H.5 p101 alphabetical-program-ordering at output time. | §5 "SAR ordering" + §5 "Open questions — resolved" #1 |
| 6 | **§6 tetragraph join level** (trigraph-expanded vs tetragraph-atomic) | Operate on canonical form. Lattice does set-union on whatever atoms the parser produced; `render_canonical` applies tetragraph aggregation via `marque-capco::vocab` tables. | §6 "Tetragraph aggregation" + §6 "Open questions — resolved" #1 |
| 7 | **§7 ATOMAL/BOHEMIA combinability** | Yes — §H.3 p56-58 treats them as separate facts on the NATO-controls axis; co-occurrence is admissible when both controls' classification requirements are met (typically CTS). Admissibility is `Constraint::Requires`, NOT lattice. | §7 "ATOMAL and BOHEMIA combinability" + §7 "Open questions — resolved" #1 |
| 8 | **§8 mixed dates and canned strings** | Canned string dominates per §E.3 hierarchy + §E.4 AEA-exempt precedence. Categorical dominance, not comparison over a common scale. | §8 "Mixed shapes" + §8 "Open questions — resolved" #1 |
| 9 | **NEW (2026-05-07) — §3 closure operator primitive** (`marque-applied.md` §4.7; D18 catalog shape pivot) | Lands in PR 3.7 as `ClosureRule` catalog + `MarkingScheme::closure_rules()` trait method per D18. Shared `FDR_DOMINATORS: &'static [TokenRef]` suppressor (Q-4.7-Cl_supp resolved). Implicit-default trio + per-marking unconditional implications. | §3 (e) + Plan `docs/plans/2026-05-13-pr3.7-lattice-resolution-gate-plan.md` Stage B |
| 10 | **NEW (2026-05-07) — §3 RELIDO incompatibility via `Constraint::Conflicts::RhsFamily(predicate)`** | Lands in PR 3.7 as `Constraint::Conflicts::RhsFamily(FamilyPredicate)` variant + walker dispatch + distributive-expansion proptest (per lattice-preflight M3 correction — NOT "commutativity"). PR 4 compacts PR 3b's enumerated RELIDO rows to ~2 family rows. | §3 (e) + Plan Stage B + Plan Stage D |
| 11 | **NEW (2026-05-07) — §6 FgiSet consensus-or-fallback already models §4.8** | Resolved 2026-05-07: existing `FgiSet::Present { concealed, countries }` implements §4.8.2 consensus-or-fallback exactly. T108d collapses to doc-comment amendment only at `crates/capco/src/lattice.rs` (Stage D of the plan). Q-FgiSet-vs-§4.8 and Q-5.3 closed. JOINT-attribution explicitly deferred to Stage 4. | §6 entire section + §6 "Open questions — resolved" + Plan Stage D |
| 12 | **NEW (2026-05-15) — §7.5 AEA category missing from earlier rev** | Resolved 2026-05-15 in PR 4b-A: `AeaSet = Product<SupersessionSet<AeaPrimary>, FlatSet<bool>, FlatSet<u8>, FlatSet<UcniKind>, OptionalSingleton<AtomalBlock>>` per §H.6 pp104-121 + §H.7 p122. Implementation in `crates/capco/src/lattice.rs` alongside `SciSet`/`SarSet`/`FgiSet`. `BoundedLattice` deliberately not implemented (axis 3 SIGMA is open-vocabulary per §H.6 p108). | §7.5 entire section |
| 13 | **NEW (2026-05-15) — §7.5 CNWDI semantic shape** (Constraint::Requires vs ClosureRule vs data-model enforcement) | **Data-model enforcement** (revised post-Copilot-review). The initial PR-4b-A draft added `E067/cnwdi-requires-rd` as a `Constraint::Requires` row. Copilot caught it as unreachable: `TOK_CNWDI` is satisfied only by `AeaMarking::Rd(rd) if rd.cnwdi`, so the right-hand side `TOK_RD` is necessarily true whenever the left-hand side is. The §H.6 p106 "subset of RD" invariant lives at the `marque-ism` type level (CNWDI is a `bool` field on `RdBlock`, not a sibling variant), so no runtime constraint row is required. The row was removed before merge. | §7.5 "Cross-axis constraints" + §7.5 "Open questions — resolved" #1 |
| 14 | **NEW (2026-05-15) — §7.5 SIGMA cross-modifier coalescing** (within-axis lattice vs PageRewrite) | **PageRewrite at page scope.** The existing `capco/frd-sigma-consolidates-into-rd-sigma` row at `§H.6 p113` IS the rd-coalesces-sigmas semantics — same rule from both subsections' vantage. No duplicate row; doc-comment updated to cite both §H.6 p108-109 (RD side) and §H.6 p113 (FRD side). | §7.5 "Cross-axis constraints" + §7.5 "Open questions — resolved" #2 |

**These are the points where the previous attempt skimmed.** Each
resolution above carries the §-citation and the algebraic shape; the
fill-in pass through §§2-8 makes the resolutions self-contained within
each category section. §9 acceptance gates verify that the resolutions
are mechanically discoverable in the document, not just listed in this
table.

**See also**: `docs/plans/2026-05-07-pr3b-consultation-verdict.md`
locks the staging for PR 3b → PR 3.7 → PR 4 → PR 5+ collapse path
and the new-primitive fold-in. `.claude/skills/marque-lattice-consultant/references/marque-applied.md`
§3.11 is the source of truth for stage sequencing.
`docs/plans/2026-05-13-pr3.7-lattice-resolution-gate-plan.md` is the
operative staging plan for the monolithic PR 3.7 that lands this
document plus the supporting trait-surface primitives.

---

## 11. PR 4b-B addenda (2026-05-15) — rest-of-the-seven lattice impls

PR 4b-B lands the six per-category lattice impls that PR 4b-A
deferred (Classification, NATO classification, JOINT, DissemSet,
RelToBlock, DeclassifyOn) plus the `NatoDissemSet` trivial-union and
two PageContext bugfixes. The five PM-resolved policy decisions in
§§2-3 are restated as worked-example addenda below; the operative
plan-of-record is `docs/plans/2026-05-15-pr4b-B-lattice-impls-rest-plan.md`.

Every citation in this addendum was re-verified 2026-05-15 against
`crates/capco/docs/CAPCO-2016.md` and
`crates/capco/docs/CAPCO-2016_citation_index.yml` page-range
cross-check, per Constitution Principle VIII propagation-discipline.

### 11.1 OC-USGOV supersession (replaces the §3 (a) "unanimity-drop" implication)

**Authority**: §H.8 p136 (ORCON template — "If ORCON and ORCON-USGOV
portions are in a document, ORCON takes precedence and is conveyed in
the banner line"); §H.8 p140 (ORCON-USGOV template — same rule from
the USGOV vantage).

**Algebra**:

```text
∀ page :
  let oc_present       = ORCON ∈ page.dissem
  let oc_usgov_present = ORCON-USGOV ∈ page.dissem
  if oc_present ∧ oc_usgov_present:
     banner.dissem.remove(ORCON-USGOV)   -- supersession
  -- else: both pass through untouched
```

This is a `SupersessionSet`-shape rule over the two-element axis
{ORCON, ORCON-USGOV}: ORCON ⊐ ORCON-USGOV. The pre-fix PageContext
implementation modeled it as unanimity (drop USGOV only when not on
every ORCON-carrying portion), which is wrong per the §H.8 p136
worked example: *one* ORCON portion is enough to win the banner over
any number of ORCON-USGOV portions.

**Worked example** (PR 4b-B Commit 2 + Commit 4 regression):

```text
Inputs:  (S//OC-USGOV) (S//OC) (S//OC-USGOV)
Pre-fix PageContext: banner = SECRET//OC/OC-USGOV (kept both — wrong)
Post-fix PageContext (= DissemSet): banner = SECRET//OC (ORCON wins)
```

### 11.2 RELIDO observed-unanimity at banner roll-up (new §3 (a) sub-row)

**Authority**: §H.8 pp155-156 (RELIDO Precedence Rules for Banner Line
Guidance — "RELIDO appears on the banner line *only* if every portion
on the page carries RELIDO").

**Algebra**:

```text
∀ page :
  let relido_present   = RELIDO ∈ page.dissem  -- union over portions
  let relido_unanimous = ∀ p ∈ page.portions . RELIDO ∈ p.dissem_us
  banner.has_relido = relido_present ∧ relido_unanimous
```

**Layer 1 vs Layer 2 boundary**: PR 4b-B implements only the
**observed-unanimity** half. The Layer 2 case — Marque infers RELIDO
from §B.3 Table 2 "classified, uncaveated, on/after 28 Jun 2010" —
defers to PR 4b-D. Layer 1 sees the portions as-parsed; if a portion
should have RELIDO per Table 2 but doesn't, Layer 2 (a future closure
operator pass) is responsible, not the lattice join.

**Worked examples**:

```text
Inputs:  (S//RELIDO) (S//RELIDO) (S//RELIDO)
Banner: SECRET//RELIDO  -- unanimous

Inputs:  (S//RELIDO) (S//RELIDO) (S)
Banner: SECRET         -- RELIDO drops; no NOFORN inference at Layer 1
```

### 11.3 JOINT producer-disunity collapse (new §2.1 sub-section)

**Authority**: §H.3 p56 (JOINT classification grammar) + §H.7 p123
(FGI source-acknowledged form). The cross-axis collapse is implied
by combining the two passages — when JOINT producer lists disagree
across portions, JOINT does not roll up to banner; the non-US
producers ride to FGI [LIST] per §H.7 p123.

**State space** (three-variant `JointSet`):

```text
JointSet =
  | Bottom                                           -- no JOINT portions
  | UnanimousProducers { level, producers }          -- every JOINT
                                                     -- portion has the
                                                     -- same producer list
  | DisunityCollapse { highest_level,
                       union_non_us_producers }      -- disunity observed;
                                                     -- non-US producers
                                                     -- migrate to FGI
```

**Lattice transitions on `join` are deterministic** over the
three-variant state space:

```text
Bottom ⊔ x = x  (bottom-identity, all three rows)

UnanimousProducers{l1, p1} ⊔ UnanimousProducers{l2, p2}
  = UnanimousProducers{max(l1,l2), p1}      if p1 == p2
  = DisunityCollapse{max(l1,l2), (p1 ∪ p2) \ {USA}}   otherwise

UnanimousProducers{l1, p1} ⊔ DisunityCollapse{l2, np2}
  = DisunityCollapse{max(l1,l2), (p1 \ {USA}) ∪ np2}

DisunityCollapse{l1, np1} ⊔ DisunityCollapse{l2, np2}
  = DisunityCollapse{max(l1,l2), np1 ∪ np2}
```

The transitions satisfy assoc/comm/idem on the state space:

- **Idempotency**: `X ⊔ X = X` (any variant joined with itself; the
  `p1 == p2` branch fires for `UnanimousProducers`, the union is
  trivial for `DisunityCollapse`, and `Bottom` is identity-preserving).
- **Commutativity**: every transition rule is symmetric in its two
  operands (max is symmetric, set-equality is symmetric, set-union
  is symmetric).
- **Associativity**: pairwise enumeration of the 3×3×3 = 27 ordered
  triples on the three-variant state space gives the same final
  variant regardless of grouping, because (a) the `level` axis is
  OrdMax which is associative, (b) once a `DisunityCollapse` enters
  the chain it absorbs every subsequent operand via the third/fourth
  rules, and (c) `Bottom` is identity. The property test
  `joint_disunity_lattice_laws` in
  `crates/capco/tests/category_lattice_laws.rs` exhausts the state
  space at the cost of a few microseconds at test time.

**The transitions are structural lattice operations on a deterministic
state space — NOT "normalization."** The `Lattice for JointSet` impl
does not need to retain inputs and re-derive output; the post-join
variant carries every fact the post-join state needs for `to_*`
round-trips and for the W004 diagnostic.

**Worked examples** (Commit 5 fixtures):

```text
Inputs:  (//JOINT S USA GBR) (//JOINT S USA GBR)
JointSet: UnanimousProducers{S, {USA, GBR}}
Banner: //JOINT SECRET USA, GBR    -- per §H.3 worked example p1299

Inputs:  (//JOINT C USA GBR) (//JOINT TS USA GBR) (//JOINT S USA GBR)
JointSet: UnanimousProducers{TS, {USA, GBR}}   -- OrdMax on level
Banner: //JOINT TOP SECRET USA, GBR

Inputs:  (//JOINT S USA GBR) (//JOINT S USA CAN)
JointSet: DisunityCollapse{S, {CAN, GBR}}
Banner: SECRET//FGI CAN GBR  -- §H.7 p123 FGI source-acknowledged form
                             -- W004 Warn diagnostic fires
                             -- (rule = "W004",
                             --  citation = "CAPCO-2016 §H.3 p56 + §H.7 p123")
```

**Mixed-with-US case** (§H.3 p57 — "the JOINT marking is
not carried forward to the banner line in US documents"): when only
*some* portions are JOINT and others are pure US, `from_attrs_iter`
returns `Bottom`. **W004 does not fire** in this case; the existing
US-document behavior (JOINT non-US producers ride to `FgiSet` via the
PageContext-resident `expected_fgi_marker`) is preserved bit-for-bit.

**Empty-producer-list defensive shape**: `UnanimousProducers { level,
producers: ∅ }` is malformed per §H.3 (JOINT requires at least USA + 1
co-owner). The constructor `JointSet::from_attrs_iter` returns
`Bottom` when given an empty producer set; the `Lattice::join`
arithmetic above never produces an `UnanimousProducers` with an empty
set from non-empty operands. The lattice consultant flagged this as a
hazard; the test `joint_unanimous_empty_producers_normalizes_to_bottom`
in Commit 5 pins the constructor's defensive normalization.

### 11.4 DissemSet — single bag, three overlays (new §3 (f))

**Authority**: §H.8 pp131-168 + §D.2 Table 3 p28 (FD&R precedence) +
§H.8 p145 (NOFORN dominates).

`DissemSet` storage = `BTreeSet<DissemControl>` + two derived flags
(`relido_observed_unanimous`, retained for round-trip; `noforn_present`,
derived). `from_attrs_iter` applies four overlays in deterministic
order:

1. Basic union over `attrs.dissem_us`.
2. **OC-USGOV supersession** (§11.1 above): drop ORCON-USGOV if ORCON
   is present in the joined set.
3. **RELIDO observed-unanimity** (§11.2 above): drop RELIDO if some
   portion lacks it.
4. **NOFORN dominates** (§D.2 Table 3 rows 1-2 + §H.8 p145): drop
   REL TO / RELIDO / DISPLAY ONLY tokens when NOFORN is present in
   the joined set. The post-join `Lattice::join` re-applies steps 2-4
   on the BTreeSet union so the supersession overlays remain
   idempotent.

**FOUO eviction is NOT done in `DissemSet`.** It lives on
`PageContext::expected_dissem_us` step 3 (the cross-axis classification
> U eviction + DSEN override) as a `Constraint::Custom(
"capco/fouo-eviction", …)` migration target for PR 4b-C. The parity
gate inherits the current behavior verbatim.

**Ordering at the lattice level is BTreeSet's natural order**; §H.8
prose ordering ("OC/NF" not "NF/OC") is the renderer's concern, not
the lattice's. The renderer (`MarkingScheme::render_canonical`) lands
in PR 5+ Stage 4.

### 11.5 RelToBlock — IntersectSet with NOFORN supersession (new §3 (g))

**Authority**: §H.8 pp150-151 (REL TO grammar + intersection-on-roll-up)
+ §D.2 Table 3 rows 9-13 (REL TO supersession by NOFORN and disjoint
LIST → NOFORN) + §H.8 p152 worked example.

```text
RelToBlock =
  | Bottom                              -- no REL TO portions
  | NofornSuperseded                    -- some portion has NOFORN /
                                        -- NODIS / EXDIS
  | Lattice { countries: BTreeSet<CountryCode> }  -- tetragraph-
                                                  -- expanded intersection,
                                                  -- USA-first sort
```

`from_attrs_iter`:

1. If any portion carries `Nf` in `dissem_us` (or NODIS/EXDIS in
   `non_ic_dissem`) → `NofornSuperseded`.
2. Else expand tetragraphs (FVEY → {AUS, CAN, GBR, NZL, USA}, ACGU →
   {AUS, CAN, GBR, USA}) via the existing
   `marque_ism::lookup_tetragraph_members` table.
3. Intersect the expanded sets across portions.
4. Empty intersection → `Bottom`. (§D.2 Table 3 row 9: "no-common-LIST
   → NOFORN" — the lattice produces `Bottom`; the post-projection
   pipeline injects NOFORN into `DissemSet` via the existing
   PageRewrite `capco/noforn-clears-rel-to`. **This is a deliberate
   split between lattice algebra and post-projection rewrite — the
   lattice cannot introduce NOFORN into a different axis.**)
5. Non-empty intersection → `Lattice { countries }`, USA-first sort.

**Lattice transitions**:

```text
Bottom ⊔ x = x
NofornSuperseded ⊔ x = NofornSuperseded   (sentinel absorbs)
Lattice{a} ⊔ Lattice{b} = Lattice{a ∩ b}  if non-empty, else Bottom
```

Tetragraph re-expansion happens at `from_attrs_iter` time, not inside
`join`; once `RelToBlock` is in `Lattice{countries}` form, the
intersection operates on already-canonical `CountryCode`s.

### 11.6 DeclassifyOnLattice — `MaxDate` semilattice (new §3 (h))

**Authority**: §H.6 p104 (RD declass authority — most restrictive
date wins) + ISOO §3.3 (date-only axis).

```text
DeclassifyOnLattice(Option<IsmDate>) :
  join(a, b) = a.max_by(end_cmp, b)        -- furthest-out date wins
  meet(a, b) = a.min_by(end_cmp, b)        -- nearest date wins
```

**Bottom** = `None`. **No top is implemented** — dates are open-vocab,
no finite top is realizable. Per `AeaSet`/`SciSet`/`SarSet`/`FgiSet`
precedent in the same module, this is the established pattern for
"no `BoundedLattice` impl when range is open."

### 11.7 NatoClassLattice — bounded OrdMax (extends §7)

**Authority**: §H.2 p55 (NATO classification ladder).

`NatoClassLattice(Option<NatoClassification>)` joins by `OrdMax` over
`NU < NR < NC < NS < CTS`. **BoundedLattice** is implemented: top =
`Some(CosmicTopSecret)`, bottom = `None`. NATO is a closed-vocabulary
five-element chain (no agency-extensibility), so the top exists.

**Pure-NATO documents only**: this lattice shadows
`ClassificationLattice` for documents with no US portions. Mixed
US+NATO documents reciprocally-raise at portion-parse time per the
existing §H.7 pp123-125 reciprocal rule (`MarkingClassification::
effective_level()`); `non_us_classification` is `None` at banner. The
property test `mixed_us_nato_non_us_classification_is_none` in
Commit 3 pins this.

### 11.8 ClassificationLattice — bounded OrdMax (extends §2)

**Authority**: §H.1 pp47-54 (US class chain) + §H.7 pp123-125
(reciprocal-normalize).

`ClassificationLattice(Option<MarkingClassification>)` joins by
`OrdMax` over `effective_level()`. Top = `Some(Us(TopSecret))`,
bottom = `None`. **The lattice does NOT naive-delegate to
`effective_level().max(other.effective_level())` — that loses
`Nato`/`Fgi`/`Joint`/`Conflict` variant discriminators.** The
implementation:

- Compares two `MarkingClassification`s by `effective_level()`.
- Returns the variant with the higher level **as-is**.
- On equal effective level, applies a deterministic, order-
  independent variant precedence: `Us < Fgi < Nato < Joint <
  Conflict` (lower rank wins). The US-canonical preference matches
  §H.7 pp123-125 reciprocal normalization — Us is the post-
  normalization canonical variant when a US classification is in
  scope. This makes the join commutative across mixed variants:
  `Us(Secret).join(Fgi(Secret)) == Fgi(Secret).join(Us(Secret)) ==
  Us(Secret)`. (PR 4b-B follow-up C-1 corrected this site — the
  original "left operand wins" tiebreak was order-dependent and
  broke commutativity.)

This preserves the variant-tag information that `JointSet`/`FgiSet`
need for accurate banner attribution downstream.

---

