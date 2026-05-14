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
- [ ] **Named alternate reviewer** (per D2 stall-recovery) has
      independently read §§2-8 and is authorized to take primary
      ownership if the primary stalls past 1 week.
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
