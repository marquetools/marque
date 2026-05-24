<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-C: Pattern-B + C non-IC handling (strip rows + structural FOUO eviction + UCNI bugfix)

**Status**: executing — PM has resolved all four OQs. See "PM
Corrections (2026-05-16)" below for the authoritative scope; the
body of the plan that follows is the original drafted form preserved
for context. Where the corrections conflict with the body, the
corrections take precedence.

## PM Corrections (2026-05-16) — AUTHORITATIVE

The PM-confirmed scope for PR 4b-C differs from the original §0
defaults in three places. The implementing agent followed these
corrections; the §0 body below is preserved for context only.

### Correction A — Pattern B is structural, not per-trigger

The original §3.1 FOUO-eviction matrix enumerated ~10 per-trigger
rows (DSEN-evicts-FOUO / RAWFISA-evicts-FOUO / etc.). **The PM-
verified reading of §H.8 p134 verbatim** —

> "FOUO is not conveyed in the banner line if the document is
> UNCLASSIFIED with FOUO and other dissemination control markings,
> excluding any FD&R markings."

— collapses Pattern B to **2 structural `PageRewrite` rows**:

1. `capco/classification-evicts-fouo` — classification > U
   ∧ contains_fouo(dissem_us) → FactRemove FOUO from CAT_DISSEM.
   Cite: §H.8 p134.
2. `capco/non-fdr-control-evicts-fouo` — contains_fouo(dissem_us)
   ∧ ∃ other non-FD&R control in
   {dissem_us\{FOUO,FD&R}, non_ic_dissem, aea, sar} →
   FactRemove FOUO from CAT_DISSEM. Cite: §H.8 p134.

"Non-FD&R" = anything except {Nf, Relido, Displayonly, Rel, Eyes}.
Use `Vocabulary::is_fdr_dissem` (broad membership; INCLUDES RELIDO)
— NOT `is_fdr_dominator` (excludes RELIDO). The distinction is
documented in `crates/capco/src/scheme.rs:5018-5039`.

### Correction B — Pattern D deferred to PR 4b-D

OQ-4(C): PR 4b-C does NOT activate Pattern D. The 7 existing
`ClosureRule` rows at `scheme.rs:5092-5215` stay as data;
`CapcoScheme::closure()` stays unoverridden. Pattern D wiring lands
alongside the `Lattice::join` hot-path flip in PR 4b-D.
See lattice-design `§3 (e)` line 388 for the deferral target.

### Correction C — OQ defaults all confirmed

- OQ-1 Path A: land TOK_PROPIN / TOK_FISA / TOK_RAWFISA / TOK_NNPI
  sentinels in Commit 1.
- OQ-2 Path A: NODIS / EXDIS land as Pattern-B participants via the
  structural `capco/non-fdr-control-evicts-fouo` row (NOT as
  independent per-trigger rows).
- OQ-3 Path A: execution-deferred; new rows are scheduler-validated
  only, PageContext drives `Engine::lint` until PR 4b-D.

### Revised commit sequence (7 commits)

The §4 commit-by-commit plan below describes a sequence that uses
Commit 4a for Pattern D. With Pattern D deferred (Correction B),
Commit 4a is empty and the sequence collapses to:

1. **Commit 1** — Plan addenda + TOK_* sentinel additions
   (PROPIN/FISA/RAWFISA/NNPI). Closes issue #407.
2. **Commit 2** — Pre-existing UCNI NOFORN-promotion bug regression
   test (RED on pre-fix branch).
3. **Commit 3** — Pattern C strip rows (5 declarative rows, possibly
   7 if UCNI split is required).
4. **Commit 4** — Pattern B structural rows (2 declarative rows
   per Correction A).
5. **Commit 5** — Delete `PageContext` imperative branches (single
   source of truth; engine-crate touch per §7.B / PR 4b-B precedent).
6. **Commit 6** — Parity gate fixture additions
   (`page_context_lattice_parity.rs`).
7. **Commit 7** — Doc updates (CLAUDE.md / README / CAPCO-CONTEXT).

The original §4 (Commit 4a Pattern D, OQ-4(A)/(B)/(C) conditionals,
FOUO-matrix per-trigger rows) is superseded by the table above.

---

**Status**: drafted — four OQs in §0 require PM resolution before
execute. The implementing agent MUST NOT touch code until OQ-1
(NNPI / FISA / RAWFISA / PROPIN vocab gaps), OQ-2 (FOUO-trigger
overlap with existing `*-implies-noforn` rewrites), OQ-3
(execution-deferral parity with PR 4b-B), and OQ-4
(Pattern D as **structural PageRewrite** vs **closure-rule
catalog wiring** — the existing in-tree Trio 1 catalog at
`scheme.rs:5092-5215` already encodes Pattern D's algebra as 7
`ClosureRule` rows citing §B.3 Table 2 p21 but is **not runtime-
wired** — see §3.7 below) are PM-resolved.

**Filename note**: kept as `pattern-c-strip-rows-plan.md` because
the Pattern C strip rows (Commits 3 + 4) are still the dominant
LOC chunk; Pattern D is one row + one helper. Content now covers
all three patterns (Pattern B compound-NF guard from §3.5,
Pattern C strip rows + FOUO matrix from §§3.1-3.6, Pattern D
caveated-implies-NOFORN from §3.7).

**Branch**: `refactor-006-pr-4b-c-pattern-c-strip-rows` off
`origin/staging` tip (post-PR-4b-B merge — registered rule count 39).
**Companion**: extends `docs/plans/2026-05-01-lattice-design.md` §3 (b)
(FOUO eviction matrix wiring) and §7 row at line 1481 (UCNI strip on
classified — PR 4b-A "deferred to PR 4b-C" deferral closing here).

**Predecessor**: PR 4b-B (`docs/plans/2026-05-15-pr4b-B-lattice-impls-rest-plan.md`)
landed seven per-category lattice impls + two PageContext bugfixes
(OC-USGOV supersession + RELIDO observed-unanimity) + W004
joint-disunity Warn. PR 4b-C does **not** add a new lattice type; it
migrates four imperative PageContext branches into declarative
`PageRewrite` catalog rows, joining the four `*-implies-noforn` rows
landed in PR 3c.B Sub-PR 8.F + 8.F.2 + the post-axis
`capco/noforn-clears-rel-to` row. The declarative system **is** the
walker; the rows are data; the existing scheduler dispatches them.

## 0. PM-blocking items (target: zero — three open)

PR 4b-C cannot execute until PM resolves the following. Each blocks a
distinct scope-shape decision; defaults are stated and the worst-case
fallback for each is named.

### OQ-1 — Vocabulary gap on PROPIN / FISA / RAWFISA / NNPI

The PM-enumerated FOUO-eviction trigger list and the design-doc §3 (b)
trigger list together call out tokens with **no `TOK_*` sentinel in
the in-tree vocabulary**. Verified 2026-05-16 against
`crates/capco/src/scheme.rs` (lines 89-213) and `crates/capco/src/scheme.rs`
(`dissem_to_tok` arm at line 4872-4904):

| Token | Storage | `TOK_*` exists? | `CategoryPredicate::Contains` usable? |
|---|---|---|---|
| FOUO | `DissemControl::Fouo` | yes (`TOK_FOUO` = 134) | yes |
| DSEN | `DissemControl::Dsen` | yes (`TOK_DSEN` = 132) | yes |
| RSEN | `DissemControl::Rs` | yes (`TOK_RSEN` = 133) | yes |
| IMCON | `DissemControl::Imc` | yes (`TOK_IMCON` = 131) | yes |
| ORCON | `DissemControl::Oc` | yes (`TOK_ORCON` = 126) | yes |
| ORCON-USGOV | `DissemControl::OcUsgov` | yes (`TOK_ORCON_USGOV` = 127) | yes |
| **PROPIN** | `DissemControl::Pr` | **no** | **no** |
| **FISA** | `DissemControl::Fisa` | **no** | **no** |
| **RAWFISA** | `DissemControl::Rawfisa` | **no** | **no** |
| **NNPI** | `NonIcDissem::Nnpi` | **no — explicit TODO(#407)** | **no** |
| SSI | `NonIcDissem::Ssi` | yes (`TOK_SSI` = 138) | yes |
| LIMDIS | `NonIcDissem::Limdis` | yes (`TOK_LIMDIS` = 135) | yes |
| SBU | `NonIcDissem::Sbu` | yes (`TOK_SBU` = 137) | yes |
| LES | `NonIcDissem::Les` | yes (`TOK_LES` = 136) | yes |

The `scheme.rs:4885` comment names the gap explicitly: "Variants
without TOK_* sentinels yet: Rel, Pr, Rawfisa, Fisa,
ExemptFromIcd501Discovery." Line 193 names the NNPI gap with
issue `#407`. Without sentinels, `CategoryPredicate::Contains {
CAT_DISSEM, TOK_PROPIN }` (and friends) cannot be authored.

**Decision required (PM)**: pick ONE path.

- **(A) Land the missing sentinels in PR 4b-C as Commit 1.** Add
  `TOK_PROPIN` / `TOK_FISA` / `TOK_RAWFISA` / `TOK_NNPI`, extend the
  `dissem_to_tok` / `non_ic_dissem_to_tok` match arms, extend
  `satisfies_attrs` (`scheme.rs:3816+`), extend
  `token_to_category_id` (`scheme.rs:1140-1160`). This is a four-
  token surface bump touching ~30 LOC across two helpers + one
  predicate dispatch. Adds 4 catalog rows.
- **(B) Ship PR 4b-C with the in-tree subset (FOUO + DSEN + RSEN +
  IMCON + ORCON + ORCON-USGOV + SSI + LIMDIS + SBU + LES) only.**
  Document the four missing-sentinel triggers in §3 (b) of the
  design doc as "deferred to PR 4b-C.1." Six FOUO-eviction rows
  ship; four defer.
- **(C) Defer PR 4b-C until issue #407 (NNPI) lands.** Worst-case
  fallback.

**Default**: (A). The sentinels are mechanical additions and the
gap-first rule for Constitution VII §IV is the precedent — see §7.B.
**Worst case if missed**: PR 4b-C ships path (B); the four deferred
triggers cannot fire and the §3 (b) trigger list overstates the
delivered coverage. Sentence-level grep on the implementing branch
(`grep -n 'TOK_PROPIN\|TOK_FISA\|TOK_RAWFISA\|TOK_NNPI'
crates/capco/src/scheme.rs`) catches the omission.

### OQ-2 — Trigger overlap with existing `*-implies-noforn` rewrites

The PM-named FOUO triggers include **NODIS and EXDIS**. Verified
2026-05-16 against `crates/capco/src/scheme.rs:2130-2207`: the
`capco/nodis-implies-noforn` and `capco/exdis-implies-noforn` rows
exist and fire (FactAdd NOFORN to CAT_DISSEM on Contains(CAT_NON_IC_DISSEM,
TOK_NODIS|TOK_EXDIS)). They add NOFORN; they do **not** strip
FOUO. FOUO + NOFORN can co-exist in a portion (NOFORN is FD&R; the
FOUO eviction rule is non-FD&R-only — §H.8 p134 verbatim: "FOUO is
not conveyed in the banner line if the document is UNCLASSIFIED with
FOUO and other dissemination control markings, excluding any FD&R
markings.").

**However**: by the time NODIS-implies-NOFORN fires, NOFORN is in
CAT_DISSEM, and the §H.8 p134 rule explicitly excludes FD&R markings
from the FOUO-eviction trigger set. So NODIS does **not** evict FOUO
transitively through NOFORN. NODIS does evict FOUO **directly** as a
non-IC dissem control (NODIS lives in CAT_NON_IC_DISSEM, not
CAT_DISSEM, but §H.8 p134's "other dissemination control markings"
covers both axes per its definition — see §A.3 p11 dissemination-
control taxonomy). The same logic applies to EXDIS.

**Decision required (PM)**: pick ONE.

- **(A) NODIS-evicts-FOUO and EXDIS-evicts-FOUO are independent
  Pattern-C rows.** They fire alongside the NOFORN-injection rows;
  scheduler order: `*-implies-noforn` rows write CAT_DISSEM,
  Pattern-C `*-evicts-fouo` rows write CAT_DISSEM (FactRemove FOUO).
  Both classes are CAT_DISSEM writers — DAG siblings (no ordering
  dependency between FactAdd and FactRemove on different tokens).
  Catalog ships ~8 FOUO-eviction rows (+2 for NODIS / EXDIS over OQ-1's
  six).
- **(B) NODIS-evicts-FOUO and EXDIS-evicts-FOUO inherit from the
  existing `*-implies-noforn` rewrites — fold the eviction into
  those rows.** The existing rows' actions become `Intent(FactAdd
  NOFORN) ∧ Intent(FactRemove FOUO)`. `CategoryAction` does not
  carry a "compound action" shape today, so this requires a scheme-
  trait extension that is **out of scope** for PR 4b-C.
- **(C) NODIS / EXDIS are NOT in the FOUO-eviction set.** §H.9
  NODIS/EXDIS portions are class-agnostic (§H.9 p172/p174); a
  `(U//FOUO//NODIS)` portion is syntactically permitted, and §H.9
  is silent on FOUO interaction. The §H.8 p134 rule's "other
  dissemination control markings, excluding any FD&R markings"
  could be read to cover NODIS / EXDIS as "non-FD&R dissem
  markings" — but §H.8 is the IC-dissem chapter and NODIS / EXDIS
  are non-IC dissem (§H.9). The taxonomy ambiguity is real.

**Default**: (A). The §H.8 p134 wording reads more naturally as "any
other dissemination control marking that is not on the FD&R chain,"
which includes the §H.9 non-IC dissem controls — NODIS, EXDIS, SBU,
LES, LIMDIS, SSI are all non-FD&R. Per design-doc §3 (b) the trigger
list is already cross-axis (it lists IMCON / DSEN / FISA / etc. from
§H.8 alongside NNPI which is §H.9-adjacent). **Worst case if
mis-resolved**: (A) over-evicts FOUO when (C) is correct — a
`(U//FOUO//NODIS)` portion loses FOUO in the banner when (C) says it
should keep it. The corpus contains no NODIS+FOUO fixtures
(`tests/corpus/valid/` grep on "NODIS\|EXDIS" intersected with "FOUO"
returns zero — verified 2026-05-16). The regression surface is
synthetic-only; the fix-forward is a one-row deletion in catalog.

### OQ-3 — Execution deferral parity with PR 4b-B

PR 4b-B's W004 + four `*-implies-noforn` rewrites + `noforn-clears-rel-to`
are **scheduler-validated but execution-deferred** until
`Lattice::join` flips to component-wise dispatch (PR 4b-D). The
PR 4b-B doc-comment block at `scheme.rs:2123-2129` is explicit: "this
rewrite is scheduler-validated (Engine::new validates the intent
payload + topological ordering) but execution-deferred (`Engine::lint`
/ `Engine::fix` drives banner-validation through PageContext directly).
Effect is visible through `scheme.project(Scope::Page, …)`. Engine-
level effect lands when Phase D/E wires banner-validation through
`scheme.project`."

PR 4b-C's Pattern-C rows inherit the same execution gap by design.
The catalog ships; `scheme.project` invokes the rows; PageContext
continues to drive `Engine::lint` / `Engine::fix` until PR 4b-D flips
the hot path.

**Decision required (PM)**: is the inherited deferral acceptable for
PR 4b-C?

- **(A) Yes — same deferral as PR 4b-B.** The rows ship, the
  PageContext branches are **migrated** (deleted in Commit 5), and
  the parity gate proves byte-identity. PageContext drives the
  engine; `scheme.project` drives any consumer that wires through
  the lattice path. **Risk**: a regression in `scheme.project`
  consumers would not surface in `Engine::lint` output today;
  caught only by the parity gate (which compares `scheme.project`
  output against the deleted PageContext output).
- **(B) No — flip the hot path in PR 4b-C.** Out of scope; this
  is PR 4b-D territory and explodes the PR by ~1500 LOC including
  the `Engine::lint` banner-validation refactor.
- **(C) Don't delete the PageContext branches — keep both paths
  alive until PR 4b-D.** The catalog rows duplicate the imperative
  logic; the parity gate proves both paths agree. **Risk**: two
  sources of truth for the same rule. The PR 4b-B precedent for
  the OC-USGOV / RELIDO bugfixes is to migrate to single source
  (the bugfixed PageContext branches stayed in PageContext but
  were not duplicated in DissemSet — instead DissemSet read
  PageContext). Pattern C is the inverse: imperative PageContext
  branches are being deleted in favor of declarative rows.

**Default**: (A). PR 4b-B's deferred-execution model is the in-tree
precedent. Path (C) violates single-source-of-truth and creates a
divergence opportunity. **Worst case if mis-resolved**: path (A) is
chosen and a `scheme.project` consumer regresses silently — the
parity gate fixtures in §8 catch this; the existing
`tests/page_context_lattice_parity.rs` (PR 4b-B, 51 fixtures) is the
template.

### OQ-4 — Pattern D shape: structural `PageRewrite` vs `ClosureRule` runtime-wiring

The PM brief specifies Pattern D as a single `PageRewrite::declarative`
row on `CapcoScheme` with a compound `Custom` predicate
(`has_caveat_marker ∧ ¬has_fdr_marker → FactAdd NOFORN`). Verified
2026-05-16 against `crates/capco/src/scheme.rs:5046-5215`: the
codebase **already contains 7 `ClosureRule` rows** that collectively
encode exactly this algebra:

| Closure row | Trigger | Suppressor | §-cite |
|---|---|---|---|
| `CLOSURE_NOFORN_SAR` | `AnyInCategory(CAT_SAR)` | `FDR_DOMINATORS` | §B.3 Table 2 p21 |
| `CLOSURE_NOFORN_AEA_RD` | `TOK_RD / TOK_FRD / TOK_TFNI` | `FDR_DOMINATORS` | §B.3 Table 2 p21 (+ §H.6) |
| `CLOSURE_NOFORN_UCNI` | `TOK_UCNI` (covers DodUcni + DoeUcni) | `FDR_DOMINATORS` | §B.3 Table 2 p21 |
| `CLOSURE_NOFORN_FGI` | `TOK_FGI_MARKER + AnyInCategory(CAT_FGI_MARKER)` | `FDR_DOMINATORS` | §H.7 p122 (+ §B.3 Table 2 p21) |
| `CLOSURE_NOFORN_ORCON` | `TOK_ORCON / TOK_ORCON_USGOV` | `FDR_DOMINATORS` | §B.3 Table 2 p21 (+ §H.8 p136/p139) |
| `CLOSURE_NOFORN_RSEN_IMCON_DSEN` | `TOK_RSEN / TOK_IMCON / TOK_DSEN` | `FDR_DOMINATORS` | §B.3 Table 2 p21 (+ §H.8 p132/p142/p159) |
| `CLOSURE_NOFORN_NONICCONTROLS` | `TOK_LIMDIS / TOK_LES / TOK_SBU / TOK_SSI` | `FDR_DOMINATORS` | §B.3 Table 2 p21 (+ §H.9 prose) |

`FDR_DOMINATORS` at `scheme.rs:5046-5062` enumerates
`{TOK_NOFORN, TOK_RELIDO, TOK_DISPLAY_ONLY, AnyInCategory(CAT_REL_TO), TOK_EYES}`
— exactly the FD&R set the PM brief specifies. The **algebra of
Pattern D is already in tree.** The gap is purely runtime wiring:
`crates/capco/src/scheme.rs:4766-4775` documents the deferral
verbatim: *"`CapcoScheme` does NOT override `MarkingScheme::closure()`
in [this PR] ... [the override + Kleene-fixpoint runtime] lands [in PR 4]."*

**Decision required (PM)**: pick ONE.

- **(A) Wire `CapcoScheme::closure()` override in PR 4b-C, runtime-
  activate the 7-row Trio 1 catalog.** This is the structurally
  correct landing: Pattern D's algebra already exists, the wiring is
  ~50-100 LOC (override + Kleene-fixpoint iteration via
  `marque_scheme::closure::MAX_CLOSURE_ITERATIONS`). The compound-
  predicate `PageRewrite` the PM brief sketches would **duplicate**
  Trio 1 — 7 rows worth of algebra collapsed into 1 monolithic row.
  Inferior in two ways: (i) loses per-trigger §-citation granularity
  (Trio 1 cites §H.6 separately from §H.7 separately from §B.3); (ii)
  loses the closure-operator's monotonicity / extensivity / idempotence
  proof obligations the lattice-design doc §3 (e) calls out
  (lines 373-407).
- **(B) Ship Pattern D as ONE new `PageRewrite::declarative` row
  with compound `Custom` predicate (PM brief's exact shape) AND
  leave the Trio 1 catalog as data-only — DO NOT activate the
  closure operator.** The PageRewrite row competes with PageContext
  (currently no NOFORN-from-caveat-default behavior) and with the
  yet-to-be-activated Trio 1 catalog. Three sources of truth for one
  algebra. Not recommended.
- **(C) Defer Pattern D to PR 4b-D** alongside the closure operator
  runtime wiring and the `Lattice::join` hot-path flip. PR 4b-C
  ships only Patterns B + C (strip rows + FOUO matrix + UCNI
  bugfix). The lattice-design doc §3 (e) already cites PR 4 as the
  closure-wiring home (line 388); this aligns with the doc.

**Default**: (C). The path-of-least-redirection that **respects the
existing design plan**: the 7-row Trio 1 catalog stays as data, the
closure-operator runtime activation moves to PR 4b-D where the
design doc already pins it, and PR 4b-C stays focused on the
imperative-branch migrations Patterns B + C cover. **Worst case if
mis-resolved**: under (A), the 50-100 LOC of closure-runtime wiring
arrives in PR 4b-C; under (B), three duplicated sources of truth ship
and someone has to clean up later.

**Reframing note** for the PM: the brief framed Pattern D as
"new" but it is **not** — it is **runtime activation** of work
already shipped. The §B.3 Table 2 p21 + §H.7 p122 citations the
brief calls out are **already** cited at `scheme.rs:5094 / 5109 /
5129 / 5143 / 5174 / 5189 / 5205`. The PM's structural-predicate
framing (`has_caveat_marker ∧ ¬has_fdr_marker → NOFORN`) is the
closure-operator algebra, not a separate `PageRewrite` rule. The
existing helper that answers `has_fdr_marker` is **NOT**
`is_fdr_dominator` (that one excludes RELIDO — answers "is this a
dominator OVER RELIDO?") but rather `Vocabulary::is_fdr_dissem`
(`crates/scheme/src/vocabulary.rs:382` + the `CapcoScheme`
override at `crates/capco/src/vocabulary.rs:1093`), which iterates
`FDR_DOMINATORS` per the doc-comment at `scheme.rs:5018-5039`.
The closure-rule mechanic reuses both via the
`suppressors: FDR_DOMINATORS` field.

---

## 1. Scope and non-scope

### 1.1 In scope

- **Pattern-C strip rows**: 5 `PageRewrite::declarative` rows on
  `CapcoScheme` — one per (token × eviction trigger) pair — covering
  the four PM-confirmed Pattern-C tokens minus LES (per §H.9 p181
  exclusion) plus the bugfixed UCNI promotion:
  - `capco/fouo-evicted-by-classified` (§H.8 p134)
  - `capco/limdis-evicted-by-classified` (§H.9 p170)
  - `capco/sbu-evicted-by-classified` (§H.9 p176)
  - `capco/dod-ucni-evicted-by-classified-with-noforn-promotion` (§H.6 p116-117)
  - `capco/doe-ucni-evicted-by-classified-with-noforn-promotion` (§H.6 p118-119)
- **FOUO-eviction matrix rows** (§H.8 p134 + design-doc §3 (b)): one
  row per non-FD&R dissem trigger. OQ-1 + OQ-2 govern the exact
  trigger count (best case 10 rows under OQ-1(A) + OQ-2(A); worst
  case 6 rows under OQ-1(B) + OQ-2(C)).
- **Pre-existing UCNI bug fix** at `crates/ism/src/page_context.rs:1085-1093`:
  the current `expected_aea_markings` strips UCNI silently when
  classified, without the §H.6 p116/p118 NOFORN-promotion clause.
  Replace the imperative branch with a Pattern-C declarative row +
  delete the buggy branch. **Single source of truth.**
- **`PageContext` imperative-branch deletions** (Commit 5):
  - `expected_dissem_us` step 3 (FOUO eviction, lines 594-599) →
    replaced by FOUO-eviction matrix rows.
  - `expected_aea_markings` UCNI strip (lines 1085-1093) → replaced
    by `*-ucni-evicted-by-classified-with-noforn-promotion` rows +
    bugfix.
  - `expected_non_ic_dissem` SBU-NF / LES-NF split lines 1230-1240
    are **NOT** deleted in PR 4b-C — they're a cross-axis migration
    (`NonIcDissem::SbuNf` → `NonIcDissem::Sbu` + `DissemControl::Nf`),
    not a strip. PR 4b-C does not touch them.
  - `expected_dissem_us` step 4 (NF injection from non-IC) lines
    600-605 is **NOT** deleted — this is a `FactAdd` from the §H.9
    SBU-NF / LES-NF / NODIS / EXDIS paths, already covered by the
    four existing `*-implies-noforn` rewrites. Per OQ-3 default
    (A), PageContext stays the imperative source; the rewrites
    are scheduler-validated parallel facts.
- **Parity gate fixtures**: additions to
  `crates/capco/tests/page_context_lattice_parity.rs` — one fixture
  per new catalog row + adversarial overlap fixtures
  (FOUO+ORCON+classified to exercise step-3-and-FOUO-matrix
  interaction).
- **§-citations** re-verified against `crates/capco/docs/CAPCO-2016.md`
  at the point of authorship for every doc-comment, row name, and
  design-doc addendum (Constitution VIII propagation re-verification).

- **Pattern D (caveated-implies-NOFORN)** — **conditionally in scope
  per OQ-4 resolution.** Under default OQ-4(C) Pattern D defers to PR
  4b-D alongside closure-operator runtime activation. Under OQ-4(A)
  PR 4b-C wires `CapcoScheme::closure()` and activates the 7-row Trio
  1 catalog already at `scheme.rs:5092-5215` (Commit 4a in §4). Under
  OQ-4(B) PR 4b-C ships one new `PageRewrite` row with compound
  `Custom` predicate AND leaves Trio 1 inert (Commit 4a alternative
  body in §4). **The default OQ-4(C) is in scope; (A) and (B) are
  conditional.**

### 1.2 Out of scope

- **LES eviction in classified.** Per §H.9 p181 verbatim
  ("LES marking always appears in the banner line if LES information
  ... is contained in the document, regardless of the document's
  classification level") LES is **not** classification-evicted.
  The PM brief explicitly excluded LES per the prior architect's
  finding. **PR 4b-C does NOT add a `capco/les-evicted-by-classified`
  row.**
- **Compound-NF non-stripping**: `NonIcDissem::SbuNf` and
  `NonIcDissem::LesNf` are **distinct enum variants** from `Sbu`
  and `Les` (verified `crates/ism/src/attrs.rs:1202-1218`); the
  Pattern-C strip rows target `TOK_SBU` and `TOK_LES`-equivalents
  only. The compound variants carry the NF identity per their
  banner-form titles (§H.9 p178 "SENSITIVE BUT UNCLASSIFIED
  NOFORN"; §H.9 p185 "LAW ENFORCEMENT SENSITIVE NOFORN") and
  the four existing `*-nf-implies-noforn` rewrites already wire
  the NOFORN-injection path. PR 4b-C MUST NOT strip the compound
  variants. The §3.5 invariant section below names this guard.
- **`Lattice::join` hot-path flip**: PR 4b-D territory. PR 4b-C
  inherits PR 4b-B's deferred-execution model (OQ-3).
- **New lattice types**: none. PR 4b-C is declarative-row migration.
- **Renderer canonicalization** (PR 5+ Stage 4): orthogonal.
- **CAT_SCI Pattern-A follow-on**: the PR 3c.B Sub-PR 8.F NODIS
  doc-comment names a future SCI Pattern A expansion (HCS-O / HCS-P
  / SI-G / TK NOFORN implications, §H.4). Out of scope for PR 4b-C.
- **NNPI sentinel + dispatch wiring (issue #407)** if OQ-1
  defaults to (B) — then a follow-on PR 4b-C.1 lands those four
  triggers later.

---

## 2. PM decisions baked in (do not re-litigate)

| # | Decision | Source | Where it manifests |
|---|---|---|---|
| 1 | **Full Pattern-C scope** — 4 Pattern-C tokens + multi-trigger FOUO. PM chose this over the narrow 3-row scope. | PM brief 2026-05-16 verbatim | Catalog rows in Commits 3 + 4 |
| 2 | **Per-trigger rows under D13 single-§-citation discipline.** Each row gets its own §-citation; no mega-rows like `capco/u-only-controls-strip`. | PM brief 2026-05-16 + PR 3b precedent | Commits 3 + 4 row names |
| 3 | **LES exclusion confirmed.** LES propagates per §H.9 p181 regardless of classification level. Not in Pattern C. | PM brief 2026-05-16 + §H.9 p181 verbatim | §1.2 out-of-scope + Commit 3 row list excludes `capco/les-evicted-by-classified` |
| 4 | **UCNI bugfix in-line.** Migrate strip + NOFORN promotion together. Delete the buggy `page_context.rs` UCNI branch (lines 1085-1093) in Commit 5. | PM brief 2026-05-16 + §H.6 p116/p118 verbatim | Commits 3 + 5 |
| 5 | **`PageRewrite::declarative` is the vehicle** — not `Constraint::Custom`. Confirmed by the four `*-implies-noforn` precedents at `scheme.rs:2130-2286`. | PM brief 2026-05-16 + in-tree precedent | Commit 3 + 4 row shape |
| 6 | **Engine-crate touch permitted** under PR 4b-B §7.B precedent (within-006 umbrella; bugfix-class deletions in `marque-ism`). Cite that precedent in §7.B below. | PM brief 2026-05-16 + PR 4b-B precedent | §7.B reasoning; Commit 5 PageContext deletions |
| 7 | **Compound-NF guard invariant** — Pattern-C strip rows target bare-token variants (`Sbu`, `Les`); compound variants (`SbuNf`, `LesNf`) are untouched. The four existing `*-nf-implies-noforn` rewrites are the canonical NF-injection path. | PM brief 2026-05-16 verbatim + `attrs.rs:1202-1218` enum split | §3.5 invariant + Commit 3 row predicates + Commit 4 row predicates |

---

## 3. Architectural shape and dependency graph deltas

No new crate edges. `marque-capco` already depends on `marque-ism`,
`marque-rules`, `marque-scheme`. PR 4b-C edits:

1. `crates/capco/src/scheme.rs` — catalog row additions in
   `build_page_rewrites()` (Commits 3 + 4); under OQ-1(A) also
   `TOK_*` constant additions + `dissem_to_tok` /
   `non_ic_dissem_to_tok` / `satisfies_attrs` /
   `token_to_category_id` extensions (Commit 1).
2. `crates/ism/src/page_context.rs` — imperative-branch deletions
   (Commit 5). **This is an engine-crate touch.** Same Constitution
   VII §IV reasoning as PR 4b-B §7.B applies; see §7.B below for
   the explicit invocation.
3. `crates/capco/src/lattice.rs` — `DissemSet::from_attrs_iter` /
   `AeaSet::from_attrs_iter` mirror updates so the lattice path
   stays in parity with the post-deletion PageContext (Commit 5
   sub-step). The bugfix UCNI NOFORN-promotion lives in `AeaSet`
   indirectly via the catalog row + the `Engine::project` post-
   axis composition; `AeaSet` itself stays pure-axis (no cross-
   axis NF injection).
4. `crates/capco/tests/page_context_lattice_parity.rs` — fixture
   additions (Commit 6).
5. `docs/plans/2026-05-01-lattice-design.md` — §3 (b) row-list
   completion + §7 row at line 1481 closure (UCNI deferral
   resolves) (Commit 1).

```text
existing edges (no changes):
  marque-scheme  ←── marque-ism  ←── marque-capco
                                       ↑
                                 (this PR's edits)
                                       ↓
                            (page_context.rs edits — same crate
                             touch precedent as PR 4b-B Commit 2)
```

### 3.1 New catalog rows and where they live

All rows land in `CapcoScheme::build_page_rewrites()` (the function
that produces the `&'static [PageRewrite<CapcoScheme>]` slice
consumed by the topological scheduler). Declaration order in the
vec mirrors the scheduler's topological order (writer-before-reader
for catalog cross-references; see §3.4 for the ordering proof).

#### Pattern-C strip rows (Commit 3) — 5 rows

| Row name | Trigger | Action | Reads / Writes | §-cite |
|---|---|---|---|---|
| `capco/fouo-evicted-by-classified` | `Custom: classification > U ∧ Contains(CAT_DISSEM, TOK_FOUO)` | `Intent(FactRemove { facts: [Cve(TOK_FOUO)], scope: Page })` | reads `[CAT_CLASSIFICATION, CAT_DISSEM]`, writes `[CAT_DISSEM]` | §H.8 p134 |
| `capco/limdis-evicted-by-classified` | `Custom: classification > U ∧ Contains(CAT_NON_IC_DISSEM, TOK_LIMDIS)` | `Intent(FactRemove { facts: [Cve(TOK_LIMDIS)], scope: Page })` | reads `[CAT_CLASSIFICATION, CAT_NON_IC_DISSEM]`, writes `[CAT_NON_IC_DISSEM]` | §H.9 p170 |
| `capco/sbu-evicted-by-classified` | `Custom: classification > U ∧ Contains(CAT_NON_IC_DISSEM, TOK_SBU)` | `Intent(FactRemove { facts: [Cve(TOK_SBU)], scope: Page })` | reads `[CAT_CLASSIFICATION, CAT_NON_IC_DISSEM]`, writes `[CAT_NON_IC_DISSEM]` | §H.9 p176 |
| `capco/dod-ucni-evicted-by-classified-promotes-noforn` | `Custom: classification > U ∧ Contains(CAT_AEA, TOK_UCNI[DodUcni])` | `Intent(FactRemove { facts: [...TOK_UCNI sentinel for DodUcni], scope: Page })` + the sibling `*-promotes-noforn` row at scheduler-next | reads `[CAT_CLASSIFICATION, CAT_AEA]`, writes `[CAT_AEA, CAT_DISSEM]` (see §3.2) | §H.6 p116-117 |
| `capco/doe-ucni-evicted-by-classified-promotes-noforn` | `Custom: classification > U ∧ Contains(CAT_AEA, TOK_UCNI[DoeUcni])` | same shape | same shape | §H.6 p118-119 |

`TOK_UCNI` today is a single sentinel covering both `DodUcni` and
`DoeUcni`; the §H.6 worked example on p116-119 treats them with
identical eviction semantics, so a single split-by-AeaMarking-payload
predicate (`Custom`) is required. **This is unavoidably a
`CategoryPredicate::Custom` row** (the in-tree `CategoryPredicate::Contains`
matches on `TokenId` granularity only); cite the §-citations on the
Custom shape's `name` + `citation` fields and on `reads`/`writes`
axis-annotations (Constitution VII §IV `Custom` axis-annotation
enforcement applies — `Engine::new` rejects unannotated `Custom`
rewrites with `EngineConstructionError::UnannotatedCustomAxes`).

#### FOUO-eviction matrix rows (Commit 4) — 5 to 10 rows depending on OQ-1 + OQ-2

Under **OQ-1(A) + OQ-2(A) — the default — 10 rows ship**:

| Row name | Trigger token | Token's storage axis | `TOK_*` | §-cite |
|---|---|---|---|---|
| `capco/orcon-evicts-fouo` | `TOK_ORCON` | CAT_DISSEM | exists | §H.8 p134 + §H.8 p136 |
| `capco/orcon-usgov-evicts-fouo` | `TOK_ORCON_USGOV` | CAT_DISSEM | exists | §H.8 p134 + §H.8 p139 |
| `capco/imcon-evicts-fouo` | `TOK_IMCON` | CAT_DISSEM | exists | §H.8 p134 + §H.8 p142 |
| `capco/propin-evicts-fouo` | **`TOK_PROPIN`** | CAT_DISSEM | **NEW (Commit 1)** | §H.8 p134 + §H.8 p148 |
| `capco/dsen-evicts-fouo` | `TOK_DSEN` | CAT_DISSEM | exists | §H.8 p134 + §H.8 p159 |
| `capco/fisa-evicts-fouo` | **`TOK_FISA`** | CAT_DISSEM | **NEW (Commit 1)** | §H.8 p134 + §H.8 p161 |
| `capco/rawfisa-evicts-fouo` | **`TOK_RAWFISA`** | CAT_DISSEM | **NEW (Commit 1)** | §H.8 p134 + §H.8 p161 (RAWFISA shares the FISA section) |
| `capco/rsen-evicts-fouo` | `TOK_RSEN` | CAT_DISSEM | exists | §H.8 p134 + §H.8 p132 |
| `capco/ssi-evicts-fouo` | `TOK_SSI` | CAT_NON_IC_DISSEM | exists | §H.8 p134 + §H.9 p189 |
| `capco/nnpi-evicts-fouo` | **`TOK_NNPI`** | CAT_NON_IC_DISSEM | **NEW (Commit 1)** | §H.8 p134 + `NonIcDissem::Nnpi` variant doc-comment in `crates/ism/src/attrs.rs` (NNPI banner-roll-up — propagates regardless of classification) |
| (OQ-2(A)) `capco/nodis-evicts-fouo` | `TOK_NODIS` | CAT_NON_IC_DISSEM | exists | §H.8 p134 + §H.9 p174 |
| (OQ-2(A)) `capco/exdis-evicts-fouo` | `TOK_EXDIS` | CAT_NON_IC_DISSEM | exists | §H.8 p134 + §H.9 p172 |

All rows: `CategoryPredicate::Contains { category: AXIS, token: TRIGGER }`
+ `CategoryAction::Intent(ReplacementIntent::fact_remove(FactRef::Cve(TOK_FOUO), Scope::Page))`
+ `reads: [AXIS]` + `writes: [CAT_DISSEM]` (FOUO is in CAT_DISSEM).

**Under OQ-1(B) the 4 NEW-sentinel rows defer to PR 4b-C.1; 6-8 rows
ship.** Under OQ-2(C) the NODIS / EXDIS rows are not added; 8-10 rows
ship depending on OQ-1.

**Total catalog additions under defaults**: 5 Pattern-C strip rows +
10-12 FOUO-eviction rows = **15-17 declarative rows**, joining the 5
existing rows (4 `*-implies-noforn` + `noforn-clears-rel-to`) in
`build_page_rewrites()` for a total of 20-22 catalog entries.

### 3.2 UCNI bugfix — strip + NOFORN promotion in a single declarative pair

The §H.6 p116/p118 verbatim rule (read directly from
`crates/capco/docs/CAPCO-2016.md` 2026-05-16) is two-part:
1. UCNI's banner-line suppression on classified docs (strip).
2. NOFORN-promotion when no stricter FD&R marker exists.

The lattice-design doc §7 row at line 1481 anticipates this and
explicitly defers it to PR 4b-C. The current PageContext
implementation at `page_context.rs:1085-1093` does (1) and silently
skips (2):

```rust
// PRE-FIX (page_context.rs:1085-1093) — verified 2026-05-16:
if !classified {
    if has_dod_ucni { result.push(AeaMarking::DodUcni); }
    if has_doe_ucni { result.push(AeaMarking::DoeUcni); }
}
```

When `classified = true`, UCNI is silently dropped. **No NOFORN
promotion fires.** This is the §H.6 violation in production today.

**Declarative-row pair** (Commit 3):

- `capco/dod-ucni-evicted-by-classified-promotes-noforn` —
  composite action: FactRemove DodUcni from CAT_AEA **and** FactAdd
  NOFORN to CAT_DISSEM (the latter idempotent via
  `apply_fact_add`'s CAT_DISSEM arm at `scheme.rs:1011`, FOUO matrix
  precedent). Since `CategoryAction::Intent` carries a single
  `ReplacementIntent`, the two facts split into **two rows** with a
  scheduler-ordered cross-row dependency:
  - Row A: `capco/dod-ucni-evicted-by-classified` —
    FactRemove DodUcni; reads `[CAT_CLASSIFICATION, CAT_AEA]`,
    writes `[CAT_AEA]`.
  - Row B: `capco/dod-ucni-promotes-noforn-when-classified` —
    `Contains(CAT_AEA, TOK_UCNI[DodUcni]) ∧ classification > U ∧
    no_stricter_fdr_marker(page)` → FactAdd NOFORN to CAT_DISSEM;
    reads `[CAT_CLASSIFICATION, CAT_AEA, CAT_DISSEM]`, writes
    `[CAT_DISSEM]`.
  - **Scheduler order**: Row B reads CAT_AEA pre-eviction (Row A
    has not yet fired on the *same* page-state — the scheduler
    runs each row against the **input** projection, not a
    progressive state); Row A and Row B are scheduler-siblings
    (no ordering dependency). The `no_stricter_fdr_marker(page)`
    predicate runs the same scan that the "FD&R intent +
    foreign-axes clear" Step 5 in `expected_dissem_us` does
    today (`page_context.rs:630-639` verbatim). **The
    predicate body is identical to PR 4b-B's
    `is_fdr_dominator` helper at `scheme.rs:4895` — reuse it.**

If a refactor of `CategoryAction` to carry compound actions lands
later (out of scope for PR 4b-C), the row-pair collapses to one
row; until then, the split is the in-tree shape.

Same shape for DoeUcni.

### 3.3 `CategoryPredicate::Custom` use — Constitution VII §IV axis-annotation enforcement

`CategoryPredicate::Contains { category, token }` covers only the
single-token-match shape. The UCNI rows require a compound predicate
(`classification > U ∧ Contains(CAT_AEA, TOK_UCNI[DodUcni])`); the
FOUO-eviction rows could in principle use `Contains` directly if the
classification gate is absent (FOUO eviction's classified-context
branch needs `classification > U`, but the non-FD&R-dissem-evicts-FOUO
branch fires at any classification level because §H.8 p134 only
gates the **first** of FOUO's two eviction conditions on
classification — the non-FD&R-dissem trigger is classification-
agnostic).

**Verified against §H.8 p134 verbatim 2026-05-16**:
- "FOUO in a classified document" (classification > U) — the
  classified-evicts-FOUO trigger.
- "FOUO is not conveyed in the banner line if the document is
  UNCLASSIFIED with FOUO and other dissemination control markings,
  excluding any FD&R markings" — classification = U + any
  non-FD&R dissem trigger.

These are **two distinct eviction conditions**, each with its own
row (the §3 (b) design-doc framing as "two axes" of the same
matrix). The classified-evicts-FOUO row uses `CategoryPredicate::Custom`
with axis annotations `reads: [CAT_CLASSIFICATION, CAT_DISSEM]`. The
non-FD&R-evicts-FOUO rows use `CategoryPredicate::Contains` (no
classification gate) — they fire at any classification level
including U.

All `Custom` predicates MUST carry explicit `reads`/`writes` axis
annotations; `Engine::new` rejects unannotated `Custom` rewrites
with `UnannotatedCustomAxes`. The PR 4b-B precedent for `Custom`
axis-annotation is the post-axis composition routing G-8 the
PR 4b-B doc names.

### 3.4 Topological scheduler ordering — cycle-check

PR 4b-C adds 15-17 new rows to a catalog of 5 existing rows. The
ordering DAG must remain acyclic. Verified 2026-05-16 by walking the
reads/writes annotations:

```text
Existing rows (PR 3c.B + PR 4b-B):
  capco/nodis-implies-noforn        : reads NON_IC_DISSEM   → writes DISSEM
  capco/exdis-implies-noforn        : reads NON_IC_DISSEM   → writes DISSEM
  capco/sbu-nf-implies-noforn       : reads NON_IC_DISSEM   → writes DISSEM
  capco/les-nf-implies-noforn       : reads NON_IC_DISSEM   → writes DISSEM
  capco/noforn-clears-rel-to        : reads DISSEM          → writes REL_TO

PR 4b-C strip rows (Commit 3):
  capco/fouo-evicted-by-classified  : reads CLASS+DISSEM    → writes DISSEM
  capco/limdis-evicted-by-classified: reads CLASS+NON_IC    → writes NON_IC
  capco/sbu-evicted-by-classified   : reads CLASS+NON_IC    → writes NON_IC
  capco/dod-ucni-evicted             : reads CLASS+AEA       → writes AEA
  capco/dod-ucni-promotes-noforn     : reads CLASS+AEA+DISSEM→ writes DISSEM
  capco/doe-ucni-evicted             : reads CLASS+AEA       → writes AEA
  capco/doe-ucni-promotes-noforn     : reads CLASS+AEA+DISSEM→ writes DISSEM

PR 4b-C FOUO-matrix rows (Commit 4):
  capco/{token}-evicts-fouo         : reads {axis}          → writes DISSEM
```

**Cycle check**: every PR 4b-C row writes either AEA, NON_IC_DISSEM,
or DISSEM. The reads-from-DISSEM rows are:
- existing `capco/noforn-clears-rel-to` (writes REL_TO, not DISSEM
  — no back-edge).
- new `capco/dod-ucni-promotes-noforn` + sibling (write DISSEM,
  read DISSEM — **same-axis self-reference**).

Same-axis self-reference (`reads: [X]` ∧ `writes: [X]`) is NOT a
cycle in Kahn's algorithm as the scheduler implements it — the
scheduler dispatches each row against the same input projection.
The `*-promotes-noforn` rows read CAT_DISSEM to check "no stricter
FD&R marker"; this is a predicate-side read, not a state-after-
write read. **Verified by inspecting `marque-engine::scheduler`
implementation** (a separate read of `crates/engine/src/scheduler.rs`
is implied at execute-time; the scheduler treats each row as a
function from input-projection to output-projection, not from
state to state). If the scheduler rejects same-axis self-reference,
the `*-promotes-noforn` rows must drop the DISSEM read annotation
and use a `Custom` predicate that scans the input projection
directly — which is what `is_fdr_dominator` does today anyway.

**No cycle risk identified.** The implementing agent re-verifies at
Commit 3 execute-time by running `cargo test -p marque-engine
scheduler::tests` and confirming the new catalog passes
`Engine::new`'s topological-validation.

### 3.5 Compound-NF non-stripping invariant

PM-named guard. Verified 2026-05-16 against
`crates/ism/src/attrs.rs:1202-1218`: `NonIcDissem::Sbu`,
`NonIcDissem::SbuNf`, `NonIcDissem::Les`, `NonIcDissem::LesNf` are
**four distinct enum variants**. The Pattern-C strip rows in §3.1
target `TOK_SBU` and `TOK_LES`-equivalents only.

**Token mapping**:
- `TOK_SBU` → `NonIcDissem::Sbu` (`scheme.rs:4915`).
- `TOK_SBU_NF` → `NonIcDissem::SbuNf` (`scheme.rs:4911`).
- `TOK_LES` → `NonIcDissem::Les` (`scheme.rs:4914`).
- `TOK_LES_NF` → `NonIcDissem::LesNf` (`scheme.rs:4912`).

`CategoryPredicate::Contains { category: CAT_NON_IC_DISSEM, token: TOK_SBU }`
fires only when a portion carries `NonIcDissem::Sbu`. A portion
carrying `NonIcDissem::SbuNf` does **not** trigger TOK_SBU. The
compound variants carry NOFORN via the four existing
`*-nf-implies-noforn` rewrites (PR 3c.B Sub-PR 8.F.2), which fire
on `TOK_SBU_NF` / `TOK_LES_NF` and add NOFORN to CAT_DISSEM
**without** stripping the SBU-NF / LES-NF token. PR 4b-C does not
touch this path.

**Invariant statement (encoded in catalog doc-comments)**: "Pattern-C
strip rows target bare-token variants only; compound NF variants
(`NonIcDissem::SbuNf`, `NonIcDissem::LesNf`) are untouched. The NF
identity carried by the compound variants is preserved by the
existing `capco/sbu-nf-implies-noforn` + `capco/les-nf-implies-noforn`
rewrites (PR 3c.B Sub-PR 8.F.2)."

**Regression-test**: `crates/capco/tests/page_context_lattice_parity.rs`
adds the fixture `pattern_c_sbu_nf_in_classified_preserves_noforn`:

- Input: `(S//SBU-NF) (S)` (forced classified by `(S)`; SBU-NF is
  syntactically restricted to UNCLASSIFIED per §H.9 p178 but the
  fixture treats it as malformed input to exercise the guard).
- Pre-PR-4b-C PageContext output (path A): banner contains NF
  (via the SBU-NF/LES-NF split + needs_nf injection).
- Post-PR-4b-C catalog output (path B): banner contains NF; the
  `capco/sbu-evicted-by-classified` Pattern-C row does NOT fire
  (the trigger is `TOK_SBU`, not `TOK_SBU_NF`); the
  `capco/sbu-nf-implies-noforn` PR 3c.B row fires unchanged.
- **Byte-identity asserted.**

### 3.7 Pattern D — caveated-implies-NOFORN

**Status**: conditional on OQ-4. This section documents the algebraic
shape and the three implementation paths so the implementing agent
can execute whichever one PM resolves.

**Authoritative citations (verified 2026-05-16 against
`crates/capco/docs/CAPCO-2016.md`)**:

- **§B.3 p20** carries the ICD-403 caveated/uncaveated definitions
  verbatim:
  > "Caveated" means bears no FD&R markings, but has one or more
  > AEA markings, SAP markings, and/or dissemination control
  > marking(s) (i.e., all IC and non-IC dissemination controls).
  > SCI controls are intentionally not listed. If only an SCI
  > marking is present, the information is considered uncaveated.
- **§B.3 Table 2 p21** is the **default-NOFORN** authority. Table 2
  row "Classified + caveated + on/after 28 June 2010" reads:
  > Mark as NOFORN in IC DAPs. Handle as NOFORN in other IC info;
  > marking encouraged but not required.
- The PM brief's "p21: caveated information defaults to NOFORN" is
  the Table 2 IC DAP / other-IC-info row. The DAP-vs-other-IC scope
  distinction matters for diagnostic-severity choice (DAP is
  prescriptive; other-IC is "encouraged"); per project memory
  `project_marque_assumes_modern_default_fdr.md` and the
  modern-default-FD&R framing, marque defaults to post-28-Jun-2010
  IC DAP scope where NOFORN is prescriptive. **Cite both p20 (caveat
  definition) and p21 Table 2 (NOFORN default).**
- **§H.7 p122** is the FGI-specific NOFORN-default authority (the
  CLOSURE_NOFORN_FGI cite in tree). FGI without explicit FD&R is
  NOFORN per the worked example `SECRET//RD/ATOMAL//FGI NATO//NOFORN`.

**Predicate structure (matches in-tree closure-rule algebra)**:

```text
has_caveat_marker(attrs) =
    !attrs.aea_markings.is_empty()                              -- AEA caveat
  ∨ !attrs.sar_markings.is_empty()                              -- SAR caveat
  ∨ !attrs.fgi_marker.is_some()                                 -- FGI caveat
  ∨ attrs.dissem_us.iter().any(|d| !is_fdr_dissem(d))           -- non-FD&R IC dissem
  ∨ !attrs.non_ic_dissem.is_empty()                             -- non-IC dissem
  ∧ ¬(SCI present alone — §B.3 p20: "If only an SCI marking is
       present, the information is considered uncaveated")

has_fdr_marker(attrs) =
    attrs.dissem_us.iter().any(d =>
        matches!(d, Nf | Relido | Displayonly | Rel | Eyes))
  ∨ !attrs.rel_to.is_empty()                                    -- REL TO is FD&R via CAT_REL_TO
```

The `is_fdr_dissem` helper is the **`Vocabulary::is_fdr_dissem`
trait method** (default impl at `crates/scheme/src/vocabulary.rs:382`;
CapcoScheme override at `crates/capco/src/vocabulary.rs:1093`),
which already iterates `FDR_DOMINATORS` per the doc-comment contract
at `scheme.rs:5018-5039`. **Reuse, do not reinvent.** Adding a new
`is_fdr_marker` helper would split the FD&R-membership question
into two divergent answers — exactly the failure mode the
`Vocabulary::is_fdr_dissem` doc-comment warns against (line 357:
"Do not delegate `is_fdr_dissem` through `is_fdr_dominator`").

**The SCI-uncaveated carve-out**: §B.3 p20 specifies that an
SCI-only portion is uncaveated. The Pattern D predicate must
therefore distinguish "SCI plus other caveat markers" (caveated)
from "SCI alone" (uncaveated). The in-tree Trio 1 catalog handles
this by **not enumerating SCI as a trigger** (verified 2026-05-16:
no closure rule cites `CAT_SCI` as a trigger; all 7 rows target
non-SCI categories). If a PageRewrite-shape Pattern D row uses
`Custom(has_caveat_marker)`, the predicate body MUST exclude SCI:
`has_caveat_marker(attrs) ∧ !is_sci_only(attrs)` where
`is_sci_only(attrs) = !attrs.sci_markings.is_empty() ∧
all_other_caveat_axes_empty(attrs)`. The closure-rule path
implicitly satisfies this by trigger enumeration.

#### Path A (OQ-4(A)): wire `CapcoScheme::closure()` — recommended structurally

- Override `MarkingScheme::closure()` in `CapcoScheme` per the
  doc-comment at `scheme.rs:4766-4775`.
- Body: Kleene-fixpoint iteration over `closure_rules()` (the 7-row
  catalog already at `scheme.rs:5092-5215`). The iteration cap
  `marque_scheme::closure::MAX_CLOSURE_ITERATIONS` is the in-tree
  monotonicity bound.
- Each `ClosureRule::triggers` is checked via
  `iter_present_tokens(attrs)`; each `ClosureRule::suppressors`
  checked the same way. If `any(triggers) ∧ ¬any(suppressors)`,
  add each `ClosureRule::cone` element to attrs (idempotent).
- Per-row `default_severity: Severity::Info` (existing field) —
  Pattern D emissions are informative, not Warn.
- The closure operator fires **between** per-axis join and the
  post-axis PageRewrites (lattice-design doc §3 (e) line 409-419).
- Composition with existing `*-implies-noforn` PageRewrites: those
  4 rewrites (NODIS / EXDIS / SBU-NF / LES-NF) fire on
  CAT_NON_IC_DISSEM tokens with `FactAdd NOFORN`. The closure pass
  fires next on the closed state. If a NODIS portion already added
  NOFORN via the PageRewrite, the closure's matching trigger
  (currently absent — NODIS / EXDIS are **not** in the Trio 1
  catalog) does not over-inject. The closure rule for non-IC
  controls (`CLOSURE_NOFORN_NONICCONTROLS`) covers LIMDIS / LES /
  SBU / SSI — overlapping with the SBU-NF / LES-NF
  `*-implies-noforn` rewrites, but those rewrites trigger on
  TOK_SBU_NF / TOK_LES_NF (compound variants); the closure rule
  triggers on TOK_SBU / TOK_LES (bare variants); **no double-
  injection** because the trigger tokens are disjoint per
  `attrs.rs:1202-1218` enum split (Pattern B §3.5 invariant).
- Verify at execute-time: the existing `*-implies-noforn`
  PageRewrites do NOT have closure-rule duplicates. Audit by
  cross-referencing the 4 PageRewrite triggers
  `{TOK_NODIS, TOK_EXDIS, TOK_SBU_NF, TOK_LES_NF}` against the 7
  closure-rule triggers (see §3.7 OQ-4 table). They are disjoint.
- **LOC estimate**: ~80-120 (closure() override + idempotence
  invariant proof + per-row activation test).

#### Path B (OQ-4(B)): one new `PageRewrite::declarative` row — PM brief's literal shape

- Add row `capco/caveated-implies-noforn` to `build_page_rewrites()`.
- Predicate: `CategoryPredicate::Custom(|attrs| has_caveat_marker(attrs) ∧ ¬has_fdr_marker(attrs) ∧ !is_sci_only(attrs))`.
- Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`.
- `reads: [CAT_AEA, CAT_SAR, CAT_FGI_MARKER, CAT_DISSEM, CAT_NON_IC_DISSEM, CAT_SCI]`,
  `writes: [CAT_DISSEM]`.
- The CAT_SCI read is for the SCI-uncaveated carve-out (predicate
  body needs to inspect `sci_markings`).
- Citation: `§B.3 p20 + §B.3 Table 2 p21 + §H.7 p122` on the
  single row (D13 single-§-citation discipline relaxation: this is
  unavoidably a multi-citation row because the algebra combines
  three source passages).
- **Loses Trio 1's per-trigger granularity**: instead of 7 rows each
  citing the per-category authority (§H.6 for AEA, §H.7 for FGI,
  §H.8 for ORCON/IMCON/DSEN, §H.9 for non-IC controls, §H.5 for
  SAR), Pattern D Path B is one mega-row citing only §B.3 Table 2
  p21 + §B.3 p20. Violates D13 single-§-citation discipline at the
  catalog level even if the row's own citation list is internally
  consistent. **Not recommended.**
- **LOC estimate**: ~80-120 (row + predicate body + helper + tests).

#### Path C (OQ-4(C)): defer to PR 4b-D — default

- Pattern D not in PR 4b-C scope. The §1.1 "conditional" bullet
  applies under OQ-4(C).
- The closure-operator runtime wiring + Pattern D activation lands
  in PR 4b-D, alongside the `Lattice::join` hot-path flip. This is
  the lattice-design doc §3 (e) line 388 reading.
- PR 4b-C stays focused on Patterns B + C (the imperative-branch
  migrations). The §B.3 Table 2 p21 + §B.3 p20 citations are
  introduced in the design-doc §3 (e) addendum (Commit 1) but no
  scheme.rs edits land for Pattern D.
- **LOC estimate**: ~0 (deferred).

### 3.8 Composition with `SbuNf` / `LesNf` cross-axis projection

Verified 2026-05-16 against `page_context.rs:1230-1240`: the
SbuNf / LesNf classified-context split fires **only when classified**
(`if classified { if seen.remove(&NonIcDissem::SbuNf) { seen.insert(NonIcDissem::Sbu); needs_nf = true; } }`).
In UNCLASSIFIED context, SbuNf and LesNf **stay as compound
variants** in non_ic_dissem and **do NOT project Nf into
dissem_us**. This matters for Pattern D in two ways:

1. **In classified context**: `(S//SBU-NF)` projects to
   `dissem_us = {Nf}` + `non_ic_dissem = {Sbu}` via the
   PageContext split. Pattern D's `has_fdr_marker` predicate
   returns `true` (Nf is in dissem_us); Pattern D does NOT fire.
   Correct algebraic outcome — the NF is already there.
2. **In unclassified context**: `(U//SBU-NF)` projects to
   `dissem_us = {}` + `non_ic_dissem = {SbuNf}`. Pattern D's
   `has_caveat_marker` returns `true` (non_ic_dissem non-empty);
   `has_fdr_marker` returns `false` (no Nf in dissem_us, no
   rel_to). Pattern D fires and adds NOFORN. **Outcome matches
   §H.9 p178**: SBU-NF carries NOFORN as part of its identity
   (the existing `capco/sbu-nf-implies-noforn` PageRewrite covers
   this case identically; Pattern D under Path A inherits without
   conflict because the closure rule for SBU-NF is **not** in
   Trio 1 — the closure catalog cites only TOK_SBU at
   `CLOSURE_NOFORN_NONICCONTROLS`, not TOK_SBU_NF).

**Conclusion: no latent bug exposed by Pattern D in the
SbuNf / LesNf projection.** The §3.5 compound-NF invariant +
the existing `*-implies-noforn` PageRewrites + the
PageContext classified-context split are all consistent.

### 3.6 Catalog declaration-order constraint

The existing `build_page_rewrites()` doc-comments (`scheme.rs:2108-2110`)
note: "Declaration order here also respects this invariant [topological
order]: the two `*-implies-noforn` entries appear before
`noforn-clears-rel-to` in the vec." PR 4b-C adds rows that write
DISSEM (most of them); they MUST be declared BEFORE the
`capco/noforn-clears-rel-to` row in the vec so `scheme.project`'s
sequential scan sees them first.

**Declaration order in the post-PR-4b-C vec**:

1. Existing: `capco/nodis-implies-noforn` (DISSEM writer, line 2130).
2. Existing: `capco/exdis-implies-noforn` (DISSEM writer, line 2194).
3. Existing: `capco/sbu-nf-implies-noforn` (DISSEM writer).
4. Existing: `capco/les-nf-implies-noforn` (DISSEM writer).
5. **NEW**: 10-12 Pattern-C strip rows in declaration order
   (NON_IC writers first, then AEA writers, then AEA-+-DISSEM
   writers, then DISSEM writer for FOUO-by-classified).
6. **NEW**: 10-12 FOUO-eviction-matrix rows (all DISSEM writers).
7. Existing: `capco/noforn-clears-rel-to` (REL_TO writer, line 2382).
8. Existing: `capco/noforn-clears-fdr-family` (DISSEM writer, line
   2396) — wait, this is a DISSEM writer after `noforn-clears-rel-to`.
   **Investigate at Commit 3 execute-time** whether
   `capco/noforn-clears-fdr-family` ordering is preserved.

---

## 4. Commit-by-commit plan

Seven commits, total ~700-900 LOC (lower end if OQ-1 defaults to B
and only 6 FOUO rows ship; higher end under defaults A + A).

### Commit 1 — Design-doc addenda + memory-correction note + (under OQ-1(A)) vocab sentinel additions

- **Files**:
  - `docs/plans/2026-05-01-lattice-design.md` (+~180 LOC).
  - `docs/plans/2026-05-16-pr4b-C-pattern-c-strip-rows-plan.md`
    (this file — already drafted).
  - **Under OQ-1(A) only**: `crates/capco/src/scheme.rs` (+~50 LOC
    for `TOK_PROPIN` / `TOK_FISA` / `TOK_RAWFISA` / `TOK_NNPI`
    constants + `dissem_to_tok` / `non_ic_dissem_to_tok` /
    `satisfies_attrs` / `token_to_category_id` extensions).
  - Memory note (out-of-tree): correction to
    `~/.claude/projects/-home-knitli-marque/memory/project_noforn_supremacy_composition.md`
    — LES is **excluded** from Pattern C per §H.9 p181 ("LES
    marking always appears in the banner line ... regardless of the
    document's classification level"). The memory currently lists
    LES as one of the Pattern C tokens; correct to "FOUO / LIMDIS /
    SBU / UCNI/DCNI" (4 tokens, not 5).
- **Subject**: `docs(plan): PR 4b-C addenda — Pattern-C strip rows + FOUO eviction matrix + UCNI bugfix + (optional) PROPIN/FISA/RAWFISA/NNPI sentinels`
- **Content (design-doc §3 (b))**:
  - Update §3 (b) trigger table to enumerate **each FOUO-eviction
    row** as its own row in the design-doc table, citing the
    per-token §-citation for each. Today's §3 (b) has one summary
    row; the new format mirrors §3 (a)'s OC-USGOV / RELIDO
    addenda from PR 4b-B Commit 1.
  - Add §3 (b) closing paragraph: "PR 4b-C wires the
    `capco/{token}-evicts-fouo` row roster as `CategoryPredicate::Contains`
    `PageRewrite::declarative` entries on `CapcoScheme`. The 5
    Pattern-C strip rows + 10-12 FOUO-matrix rows together
    replace the imperative
    `PageContext::expected_dissem_us` step 3 + `expected_aea_markings`
    UCNI strip branch. PageContext is the production path until
    PR 4b-D flips `Lattice::join`'s hot path." (OQ-3 default A.)
  - Close the §7 line-1481 deferral row: "UCNI strip on classified
    — landed in PR 4b-C as `capco/dod-ucni-evicted-by-classified-promotes-noforn`
    + `capco/doe-ucni-evicted-by-classified-promotes-noforn`. §H.6
    p116-117 + p118-119 verbatim verified 2026-05-16."
- **§-citations to verify** (Principle VIII propagation):
  re-open `crates/capco/docs/CAPCO-2016.md` and re-verify
  **every** page-number citation in the design-doc additions:
  §H.8 p134, §H.8 p132 (RSEN), §H.8 p136 (ORCON), §H.8 p139
  (ORCON-USGOV), §H.8 p142 (IMCON), §H.8 p148 (PROPIN), §H.8 p159
  (DSEN), §H.8 p161 (FISA/RAWFISA), §H.6 p116-117 (DodUcni), §H.6
  p118-119 (DoeUcni), §H.9 p170 (LIMDIS), §H.9 p172 (EXDIS), §H.9
  p174 (NODIS), §H.9 p176 (SBU), §H.9 p181 (LES — for the
  exclusion citation), §H.9 p189 (SSI).
- **Under OQ-1(A) vocab sentinel additions in Commit 1**:
  - New constants in `scheme.rs:189-213` block:
    - `pub const TOK_PROPIN: TokenId = TokenId(143);` (next free
      after `TOK_BOHEMIA = 142`).
    - `pub const TOK_FISA: TokenId = TokenId(144);`
    - `pub const TOK_RAWFISA: TokenId = TokenId(145);`
    - `pub const TOK_NNPI: TokenId = TokenId(146);`
  - Extend `dissem_to_tok` match arms (`scheme.rs:4872-4904`):
    - `DissemControl::Pr => Some(TOK_PROPIN),`
    - `DissemControl::Fisa => Some(TOK_FISA),`
    - `DissemControl::Rawfisa => Some(TOK_RAWFISA),`
  - Extend `non_ic_dissem_to_tok` match (`scheme.rs:4907-4920`):
    - `NonIcDissem::Nnpi => Some(TOK_NNPI),`
  - Extend `satisfies_attrs` arms (`scheme.rs:3816+`): add an arm
    each for the four new tokens, mirroring `TOK_FOUO` /
    `TOK_DSEN` shape.
  - Extend `token_to_category_id` (`scheme.rs:1134-1180`): add
    `TOK_PROPIN | TOK_FISA | TOK_RAWFISA` to the CAT_DISSEM arm,
    add `TOK_NNPI` to the CAT_NON_IC_DISSEM arm. The drift-guard
    comment at `scheme.rs:4887-4898` becomes outdated; update it.
  - Delete `crates/capco/src/scheme.rs:193` TODO comment
    (`TODO(#407): Add TOK_NNPI when the sentinel and satisfies_attrs
    arm land.`). Close issue #407 in the PR description.
- **Test fixtures**: none in Commit 1 (vocab additions covered by
  Commit 4's row-level tests).
- **Constitution checks**: V (no document text in design-doc
  examples — use category IDs / token canonicals); VIII (every
  citation re-verified at authorship).

### Commit 2 — Pre-existing UCNI bug regression test (RED)

- **Files**:
  - `crates/ism/src/page_context.rs` `#[cfg(test)]` module
    (+~60 LOC for `ucni_classified_strip_loses_noforn_promotion_regression` test).
- **Subject**: `test(ism): pre-PR-4b-C UCNI bug — strip without NOFORN promotion (§H.6 p116/p118)`
- **Content**:
  - New test asserts the **pre-fix** behavior is wrong:

    ```rust
    #[test]
    fn ucni_classified_strip_loses_noforn_promotion_regression() {
        // CAPCO-2016 §H.6 p116-117 (DOD UCNI Precedence Rules) +
        // p118-119 (DOE UCNI) verbatim: "When information bearing
        // [DOD/DOE UCNI] marking is incorporated in any classified
        // intelligence product, the [DOD/DOE UCNI] marking is no
        // longer used and the entire product must be evaluated for
        // appropriate dissemination controls (including NOFORN if no
        // stricter FD&R marker exists)."
        //
        // Pre-PR-4b-C: page_context.rs:1085-1093 strips UCNI silently
        // when classified WITHOUT the NOFORN-promotion clause. This
        // test pins the wrong behavior so Commit 5 (deletion +
        // declarative-row migration) shows the bug is gone.
        //
        // verified 2026-05-16 against CAPCO-2016.md (§H.6 DOD UCNI
        // p116-117 / §H.6 DOE UCNI p118-119 page ranges in
        // CAPCO-2016_citation_index.yml).

        let mut ctx = PageContext::new();
        ctx.add_portion(parse_portion("(U//DOD UCNI//FOUO)"));
        ctx.add_portion(parse_portion("(S)"));

        let aea = ctx.expected_aea_markings();
        let dissem = ctx.expected_dissem_us();

        // Pre-fix: UCNI silently dropped, NOFORN not injected.
        assert!(aea.is_empty(), "pre-fix bug: UCNI stripped on classified");
        assert!(
            !dissem.iter().any(|d| matches!(d, DissemControl::Nf)),
            "pre-fix bug: NOFORN should be promoted per §H.6 p116/p118 but is absent"
        );
    }
    ```

  - Marked `#[test]` (no `#[ignore]`). **Test PASSES on the
    pre-fix branch** (asserts the bug) and **FAILS on the
    post-fix branch** (after Commit 5 deletes the buggy branch).
    Commit 5 deletes this test alongside the buggy branch (the
    test is part of Commit 5's atomic before/after delta) and
    replaces it with the post-fix `ucni_classified_promotes_noforn_via_pattern_c`
    test that asserts the correct behavior. This is the same
    pre-fix / post-fix swap pattern PR 4b-B Commit 2 used for
    OC-USGOV.
- **Test fixtures**: the regression test above.
- **Constitution checks**: V (no document text — `parse_portion`
  is a canonical helper using token canonicals).

### Commit 3 — 5 Pattern-C strip rows + UCNI bugfix row pair

- **Files**:
  - `crates/capco/src/scheme.rs` (+~360 LOC in
    `build_page_rewrites()`).
  - `crates/capco/tests/category_lattice_laws.rs` (+~120 LOC for
    per-row catalog presence tests).
- **Subject**: `feat(capco): Pattern-C strip rows + UCNI bugfix declarative pair (006 T112 PR 4b-C Commit 3)`
- **Content** — 5 new `PageRewrite::declarative` entries in
  `build_page_rewrites()`, in declaration order per §3.6:

  Each row follows the PR 3c.B `capco/nodis-implies-noforn` doc-
  comment shape (~50 LOC of doc-comment + 7 LOC of row literal),
  citing the per-token §-citation verbatim from CAPCO-2016.md
  and naming the `verified 2026-05-16` propagation tag.

  Row-name list (final under defaults; OQ-1(B) drops the
  `*-noforn-promotes` rows' compound-action footprint
  proportionally):

  1. `capco/limdis-evicted-by-classified` —
     `Custom: classification > U ∧ Contains(CAT_NON_IC_DISSEM, TOK_LIMDIS)`
     → `Intent(FactRemove [Cve(TOK_LIMDIS)] Scope::Page)`.
  2. `capco/sbu-evicted-by-classified` —
     `Custom: classification > U ∧ Contains(CAT_NON_IC_DISSEM, TOK_SBU)`
     → `Intent(FactRemove [Cve(TOK_SBU)] Scope::Page)`.
  3. `capco/dod-ucni-evicted-by-classified` —
     `Custom: classification > U ∧ AeaMarkingHasDodUcni(page)`
     → `Intent(FactRemove [Cve(TOK_UCNI for DodUcni)] Scope::Page)`.
  4. `capco/doe-ucni-evicted-by-classified` —
     `Custom: classification > U ∧ AeaMarkingHasDoeUcni(page)`
     → `Intent(FactRemove [Cve(TOK_UCNI for DoeUcni)] Scope::Page)`.
  5. `capco/dod-ucni-promotes-noforn-when-classified` —
     `Custom: classification > U ∧ AeaMarkingHasDodUcni(page) ∧ ¬is_fdr_dominator_present(page)`
     → `Intent(FactAdd Cve(TOK_NOFORN) Scope::Page)`.
  6. `capco/doe-ucni-promotes-noforn-when-classified` —
     `Custom: classification > U ∧ AeaMarkingHasDoeUcni(page) ∧ ¬is_fdr_dominator_present(page)`
     → `Intent(FactAdd Cve(TOK_NOFORN) Scope::Page)`.
  7. `capco/fouo-evicted-by-classified` —
     `Custom: classification > U ∧ Contains(CAT_DISSEM, TOK_FOUO)`
     → `Intent(FactRemove [Cve(TOK_FOUO)] Scope::Page)`.

  (Note: 7 rows, not 5 — the UCNI bugfix splits into 2 evict-rows
  + 2 noforn-promotes-rows = 4 rows for UCNI alone. The §1.1 "5
  Pattern-C strip rows" count compressed UCNI into 2 conceptually;
  the catalog reality is 4. Total Pattern-C: 7 rows.)
- **Test fixtures (category_lattice_laws.rs)**:
  - `catalog_contains_fouo_evicted_by_classified_row`
  - `catalog_contains_limdis_evicted_by_classified_row`
  - `catalog_contains_sbu_evicted_by_classified_row`
  - `catalog_contains_dod_ucni_evicted_by_classified_row`
  - `catalog_contains_doe_ucni_evicted_by_classified_row`
  - `catalog_contains_dod_ucni_promotes_noforn_row`
  - `catalog_contains_doe_ucni_promotes_noforn_row`
- **§-citations verified**: §H.6 p116-117, §H.6 p118-119, §H.8
  p134, §H.9 p170, §H.9 p176. Each re-verified at the moment of
  writing the doc-comments per Principle VIII; tagged
  `verified 2026-05-16`.
- **Constitution checks**: V (doc-comments cite category IDs +
  token canonicals — no document text), VII §IV (engine-crate
  touch in Commit 5; Commit 3 is `marque-capco` only, no
  Constitution issue), VIII.

### Commit 4 — FOUO-eviction matrix rows

- **Files**:
  - `crates/capco/src/scheme.rs` (+~480 LOC under defaults; +~290
    LOC under OQ-1(B)).
  - `crates/capco/tests/category_lattice_laws.rs` (+~140 LOC).
- **Subject**: `feat(capco): FOUO-eviction matrix declarative rows — §H.8 p134 (006 T112 PR 4b-C Commit 4)`
- **Content** — 10 to 12 `PageRewrite::declarative` entries per
  §3.1 table, in declaration order per §3.6:

  Each row uses `CategoryPredicate::Contains { category: AXIS,
  token: TOK_TRIGGER }` + `CategoryAction::Intent(ReplacementIntent::fact_remove(
  FactRef::Cve(TOK_FOUO), Scope::Page))`. **No classification gate
  on these rows** — §H.8 p134 verbatim says "FOUO is not conveyed
  in the banner line if the document is UNCLASSIFIED with FOUO
  and other dissemination control markings, excluding any FD&R
  markings" — this is a U-banner trigger, not a class-gated
  trigger. The classified-evicts-FOUO case is covered by Commit 3
  Row 7 separately.

  Per-row doc-comment cites §H.8 p134 (the umbrella rule) **plus**
  the per-trigger §-citation (e.g., `§H.8 p134 + §H.8 p159` for
  DSEN). Verified `2026-05-16` propagation tag on each.
- **Test fixtures**:
  - One `catalog_contains_*_evicts_fouo_row` per matrix row (10-12
    cases).
  - `fouo_matrix_does_not_fire_on_fdr_tokens` — confirms
    NOFORN / REL TO / RELIDO / DISPLAY ONLY / EYES in CAT_DISSEM
    do NOT trigger FOUO eviction (the §H.8 p134 "excluding any
    FD&R markings" clause).
- **§-citations verified**: same set as Commit 1 design-doc
  additions; re-verified at doc-comment authorship per
  Principle VIII.
- **Constitution checks**: V, VIII.

### Commit 4a — Pattern D wiring (conditional on OQ-4(A) or OQ-4(B))

**Under OQ-4(C) — default — this commit is empty / not landed.**
The implementing agent ships only Commits 1-4 + 5-7 per §1.1.

#### Commit 4a body under OQ-4(A) — closure operator runtime wiring

- **Files**:
  - `crates/capco/src/scheme.rs` (+~80-120 LOC: `impl
    MarkingScheme::closure for CapcoScheme` override body +
    iteration cap + idempotence invariant).
  - `crates/scheme/src/closure.rs` if any trait-surface
    augmentation needed (verify at execute-time; the override
    should be sufficient without scheme-crate edits).
  - `crates/capco/tests/category_lattice_laws.rs` (+~120 LOC for
    7 per-row closure-firing tests).
  - `crates/capco/tests/page_context_lattice_parity.rs` (+~80 LOC
    for Pattern D parity fixtures — see Commit 6).
- **Subject**: `feat(capco): activate Trio 1 closure-rule catalog — caveated-implies-NOFORN per §B.3 Table 2 p21 (006 T112 PR 4b-C Commit 4a)`
- **Content**:
  - Override body (sketch):

    ```rust
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        // §B.3 Table 2 p21: caveated info defaults to NOFORN. §B.3
        // p20 (ICD 403): caveated = bears no FD&R + has AEA/SAR/
        // dissem-control (excluding SCI-only).
        //
        // Kleene-fixpoint over the 7-row Trio 1 catalog at
        // `scheme.rs:5092-5215`. Idempotent because every cone
        // intent (NOFORN) is monotone-additive on CAT_DISSEM per
        // `apply_fact_add`'s set-semantic arm.
        //
        // verified 2026-05-16 against CAPCO-2016.md (§B.3 pp20-21
        // page range in CAPCO-2016_citation_index.yml).
        let mut current = marking;
        for _iter in 0..marque_scheme::closure::MAX_CLOSURE_ITERATIONS {
            let mut changed = false;
            for rule in self.closure_rules() {
                if any_present(&current, rule.triggers)
                    && !any_present(&current, rule.suppressors)
                {
                    for cone_token in rule.cone {
                        if try_add_token(&mut current, cone_token) {
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                return current;
            }
        }
        // Reached the cap; this is the fixpoint per the
        // monotonicity proof at §4.7.3.
        current
    }
    ```

  - The `any_present(&attrs, &[TokenRef])` helper reuses the
    existing `iter_present_tokens` machinery (verify at execute-
    time).
  - The 7 `ClosureRule::name` fields already cite §-citations
    (verified 2026-05-16 at `scheme.rs:5094 / 5109 / 5129 / 5143 /
    5174 / 5189 / 5205`); no doc-comment edits needed unless the
    override body wants to cite §B.3 p20 + §B.3 Table 2 p21
    explicitly (recommended — propagation tag `verified
    2026-05-16` on the new override doc-comment).
- **Test fixtures (category_lattice_laws.rs)**: one per closure
  row (7 fixtures), each asserting that:
  - The trigger fires when the matching token is present + no
    FD&R suppressor present.
  - The cone (NOFORN) is added to dissem_us.
  - Re-running closure on the closed state is a no-op
    (idempotence).
- **§-citations verified**: §B.3 p20, §B.3 Table 2 p21, §H.5
  (SAR), §H.6 (AEA / UCNI), §H.7 (FGI), §H.8 (ORCON / IMCON /
  DSEN), §H.9 (LIMDIS / LES / SBU / SSI). All already cited in
  the existing 7 ClosureRule structs; re-verified at override-
  body authorship per Principle VIII.
- **Constitution checks**: V (no document text — predicate body
  reads attrs but never serializes input bytes), VII §IV
  (engine-crate touch — same precedent as Commit 5), VIII.

#### Commit 4a body under OQ-4(B) — one new compound-predicate PageRewrite row

- **Files**:
  - `crates/capco/src/scheme.rs` (+~80-120 LOC: one new
    `PageRewrite::declarative` row + helper functions
    `has_caveat_marker` / `is_sci_only` + a doc-comment block
    naming the override-vs-row trade-off).
  - `crates/capco/tests/category_lattice_laws.rs` (+~80 LOC for
    catalog-presence + predicate-correctness tests).
  - `crates/capco/tests/page_context_lattice_parity.rs` (+~80 LOC
    Pattern D fixtures — see Commit 6).
- **Subject**: `feat(capco): caveated-implies-NOFORN PageRewrite row — §B.3 Table 2 p21 (006 T112 PR 4b-C Commit 4a)`
- **Content**: one row per §3.7 Path B sketch.
- **Doc-comment MUST flag the duplication**: the row's body
  duplicates the Trio 1 catalog algebra. Cite the in-tree Trio 1
  rows at `scheme.rs:5092-5215` in the new row's doc-comment,
  noting the design-debt: "PR 4b-D consolidates this row with the
  Trio 1 catalog when `CapcoScheme::closure()` is wired."

### Commit 5 — PageContext imperative-branch deletions + post-fix UCNI test

- **Files**:
  - `crates/ism/src/page_context.rs` (−~50 LOC: delete
    `expected_dissem_us` step 3 lines 594-599;
    delete `expected_aea_markings` UCNI strip lines 1085-1093).
  - `crates/ism/src/page_context.rs` `#[cfg(test)]` module
    (Commit 2's regression test deleted + replaced by the
    post-fix correctness test `ucni_classified_promotes_noforn_via_pattern_c`).
  - `crates/capco/src/lattice.rs` (any necessary
    `DissemSet::from_attrs_iter` / `AeaSet::from_attrs_iter`
    mirror updates so the lattice path stays in parity with the
    post-deletion PageContext) — likely net-zero LOC because the
    lattice already mirrors only what PageContext does. **Verify
    at execute-time**.
- **Subject**: `refactor(ism): delete imperative FOUO/UCNI eviction branches — Pattern-C rows are single source of truth (006 T112 PR 4b-C Commit 5)`
- **Content**:
  - Delete `expected_dissem_us` step 3 lines 594-599 (the
    `if seen.contains(&DissemControl::Fouo) && (classified ||
    dsen_present) { seen.remove(...); }` branch). The Pattern-C
    rows now handle this declaratively. Inline replacement
    comment: `// FOUO eviction migrated to PageRewrite catalog
    rows: capco/{fouo-evicted-by-classified, orcon-evicts-fouo,
    orcon-usgov-evicts-fouo, imcon-evicts-fouo, propin-evicts-fouo,
    dsen-evicts-fouo, fisa-evicts-fouo, rawfisa-evicts-fouo,
    rsen-evicts-fouo, ssi-evicts-fouo, nnpi-evicts-fouo,
    nodis-evicts-fouo, exdis-evicts-fouo} (PR 4b-C T112). PageContext
    no longer evicts FOUO imperatively — scheme.project drives
    eviction via the catalog. § H.8 p134 verified 2026-05-16.`
  - Delete `expected_aea_markings` UCNI strip lines 1085-1093 (the
    `if !classified { if has_dod_ucni { ... } if has_doe_ucni { ... } }`
    branch). The Pattern-C UCNI rows + NOFORN-promotion rows handle
    this declaratively, including the §H.6 NOFORN-promotion the
    pre-fix code missed. Inline replacement: `// UCNI eviction +
    NOFORN promotion migrated to PageRewrite catalog rows:
    capco/{dod,doe}-ucni-evicted-by-classified +
    capco/{dod,doe}-ucni-promotes-noforn-when-classified
    (PR 4b-C T112). The pre-fix branch silently dropped UCNI
    without the §H.6 p116/p118 NOFORN-promotion clause; the
    catalog rows fix this. § H.6 p116-117 + § H.6 p118-119
    verified 2026-05-16.`
  - Replace the pre-fix Commit 2 regression test with the
    correctness assertion (`ucni_classified_promotes_noforn_via_pattern_c`):

    ```rust
    #[test]
    fn ucni_classified_promotes_noforn_via_pattern_c() {
        // §H.6 p116-117 + §H.6 p118-119 verified 2026-05-16:
        // when classified, UCNI is stripped AND NOFORN is added
        // unless a stricter FD&R marker is already present. The
        // pre-fix PageContext branch silently dropped UCNI without
        // promoting NOFORN (deleted in Commit 5). The post-fix
        // path runs through scheme.project's catalog rows:
        // capco/dod-ucni-evicted-by-classified + capco/dod-ucni-
        // promotes-noforn-when-classified.

        let scheme = CapcoScheme::new();
        let marking = scheme.project(Scope::Page, parse_doc("(U//DOD UCNI//FOUO) (S)"));

        // UCNI gone from CAT_AEA.
        assert!(marking.aea_markings.iter().all(|a| !matches!(a, AeaMarking::DodUcni)));
        // NOFORN injected into CAT_DISSEM.
        assert!(marking.dissem_us.iter().any(|d| matches!(d, DissemControl::Nf)));
    }
    ```

  - **No PageContext test for the FOUO deletion is needed** — the
    parity gate (Commit 6) covers FOUO eviction at every fixture.
- **Test fixtures**: post-fix correctness test above; PageContext-
  internal FOUO tests in `page_context.rs::tests` that pre-fix
  asserted "FOUO removed when classified" stay green because
  PageContext now passes its accumulator through to
  `scheme.project` which fires the catalog rows. **Verify
  at execute-time** which PageContext tests need to be
  re-keyed to read `scheme.project`'s output instead of
  `expected_dissem_us` directly. Estimate: 5-10 affected tests.
- **§-citations verified**: §H.6 p116-117, §H.6 p118-119, §H.8
  p134; re-verified at the deletion-comment authorship per
  Principle VIII.
- **Constitution checks**: V (no document text in deletion
  comments — only category IDs + page citations); VII §IV
  (engine-crate touch — explicit invocation in §7.B below);
  VIII.

### Commit 6 — Parity-gate fixture additions

- **Files**:
  - `crates/capco/tests/page_context_lattice_parity.rs` (+~300 LOC
    for ~15-20 new fixtures).
- **Subject**: `test(capco): parity-gate Pattern-C strip + FOUO matrix fixtures (006 T112 PR 4b-C Commit 6)`
- **Content** — fixture additions to the existing 51-fixture
  parity gate. Each fixture parses a multi-portion document, runs
  Path A (`PageContext::add_portion` + `page_context_to_attrs`)
  and Path B (`scheme.project(Scope::Page, ...)` + the catalog
  rows), and asserts byte-identity on the resulting
  `CanonicalAttrs`.

  Fixture roster (best-case under defaults — 18 fixtures):

  - `pattern_c_fouo_in_classified_strips_via_catalog` —
    `(U//FOUO) (S)` → FOUO not in banner.
  - `pattern_c_fouo_with_orcon_unclassified_strips_via_matrix` —
    `(U//FOUO) (U//ORCON)` → FOUO not in banner, ORCON is.
  - `pattern_c_fouo_with_orcon_classified_strips_both_class_and_matrix` —
    `(U//FOUO) (S//ORCON)` → FOUO not in banner via BOTH the
    classified-evicts-FOUO row AND the orcon-evicts-FOUO row.
    Asserts the two rows are scheduler-siblings (no ordering
    issue).
  - `pattern_c_fouo_with_relido_unclassified_does_not_strip` —
    `(U//FOUO) (U//RELIDO)` → FOUO STAYS in banner (RELIDO is
    FD&R, §H.8 p134 "excluding any FD&R markings").
  - `pattern_c_fouo_with_noforn_unclassified_does_not_strip` —
    `(U//FOUO) (U//NF)` → FOUO STAYS in banner (NOFORN is FD&R).
  - `pattern_c_fouo_with_propin_strips_via_matrix` — OQ-1(A) only.
  - `pattern_c_fouo_with_fisa_strips_via_matrix` — OQ-1(A) only.
  - `pattern_c_fouo_with_rawfisa_strips_via_matrix` — OQ-1(A) only.
  - `pattern_c_fouo_with_nnpi_strips_via_matrix` — OQ-1(A) only.
  - `pattern_c_fouo_with_dsen_strips_via_matrix` — always-on.
  - `pattern_c_fouo_with_ssi_strips_via_matrix` — always-on.
  - `pattern_c_fouo_with_nodis_strips_via_matrix` — OQ-2(A) only.
  - `pattern_c_fouo_with_exdis_strips_via_matrix` — OQ-2(A) only.
  - `pattern_c_limdis_in_classified_strips_via_catalog` —
    `(U//LIMDIS) (S)` → LIMDIS not in banner.
  - `pattern_c_sbu_in_classified_strips_via_catalog` —
    `(U//SBU) (S)` → SBU not in banner.
  - `pattern_c_sbu_nf_in_classified_preserves_noforn` —
    §3.5 invariant test: `(U//SBU-NF) (S)` → SBU-NF stays,
    NOFORN injected (existing `sbu-nf-implies-noforn` path).
  - `pattern_c_les_in_classified_propagates_to_banner` —
    `(U//LES) (S)` → LES in banner per §H.9 p181 (LES
    exclusion confirmation).
  - `pattern_c_dod_ucni_classified_strips_and_promotes_noforn` —
    `(U//DOD UCNI//FOUO) (S)` → UCNI gone, NOFORN injected.
  - `pattern_c_doe_ucni_classified_strips_and_promotes_noforn` —
    `(U//DOE UCNI//FOUO) (S)` → UCNI gone, NOFORN injected.
  - `pattern_c_dod_ucni_classified_with_existing_noforn_no_double_inject` —
    `(U//DOD UCNI//NF) (S)` → UCNI gone, NOFORN stays (one copy,
    idempotent via `apply_fact_add` CAT_DISSEM arm).

  **Under OQ-4(A) or OQ-4(B) — Pattern D fixtures** (7 additional
  cases, PM-specified):

  - `caveated_aea_only_implies_noforn` — `(S//RD)` → banner contains
    NOFORN. §B.3 Table 2 p21 + §H.6 p104. AEA-axis caveat with no
    FD&R marker.
  - `caveated_non_ic_implies_noforn` — `(U//LIMDIS)` → banner
    contains NOFORN. §B.3 Table 2 p21 + §H.9 p170. Non-IC dissem
    caveat with no FD&R marker. (Note: under Path A this fires via
    `CLOSURE_NOFORN_NONICCONTROLS`; under Path B the single
    compound predicate handles it.)
  - `caveated_with_rel_to_no_noforn` — `(S//RD/REL TO USA, GBR)` →
    banner unchanged, no Pattern-D-injected NOFORN. §B.3 Table 2
    p21 + §B.3 p20 (FD&R suppresses). REL TO is in
    `FDR_DOMINATORS::AnyInCategory(CAT_REL_TO)`.
  - `caveated_with_relido_no_noforn` — `(S//PROPIN//RELIDO)` →
    banner unchanged, no Pattern-D-injected NOFORN. RELIDO is in
    `FDR_DOMINATORS` (line 5048). **Under OQ-1(A) PROPIN sentinel
    exists; under OQ-1(B) this fixture defers or replaces PROPIN
    with FOUO** (which has TOK_FOUO).
  - `sci_only_uncaveated_no_noforn` — `(S//SI)` → banner unchanged.
    §B.3 p20 SCI-only carve-out: SCI alone is uncaveated.
    Critical regression-gate for the SCI-uncaveated predicate
    branch in `has_caveat_marker`.
  - `multiple_caveat_classes_single_noforn` — `(S//RD/PROPIN/LIMDIS)`
    → banner contains exactly one NOFORN (set-semantic via
    `apply_fact_add` idempotence). Pattern D's three triggers
    (AEA / IC dissem / non-IC dissem) all fire under closure but
    `FactAdd NOFORN` is idempotent.
  - `rsen_classified_caveated_implies_noforn` — `(S//RSEN)` →
    banner contains NOFORN. §B.3 Table 2 p21 + §H.8 p132. RSEN is
    classified-only per §H.8 p132 but is still a caveat marker.
    **Note**: this row was extended in the closure-runtime
    landing — RSEN now triggers alongside IMCON / DSEN via
    `CLOSURE_NOFORN_RSEN_IMCON_DSEN` (the §H.8 p132 + §B.3
    Table 2 p21 algebra applies). Originally surfaced as **Risk
    #9** below, now closed.

  Each fixture cites the §-page-number citation as an inline doc-
  comment on the test function. Helper:
  `assert_path_a_b_byte_identical(name, doc, citation)`.

- **§-citations verified**: same set as Commit 4 + Commit 3;
  per-fixture inline citations re-verified.
- **Constitution checks**: V (fixture doc-comments cite §§ +
  category IDs only — no document text).

### Commit 7 — Doc updates (CLAUDE.md / README / CAPCO-CONTEXT.md)

- **Files**:
  - `CLAUDE.md` (root) (+~12 LOC: PR 4b-C entry in "Recent Changes").
  - `crates/capco/README.md` (+~8 LOC: catalog row inventory
    update — Pattern-C rows + FOUO matrix added).
  - `crates/capco/CAPCO-CONTEXT.md` (+~10 LOC: §3.4 "marque gap"
    table closes the FOUO eviction matrix row + the UCNI bug row).
- **Subject**: `docs: PR 4b-C closure docs — CLAUDE.md / README / CAPCO-CONTEXT (006 T112 PR 4b-C Commit 7)`
- **Content**:
  - `CLAUDE.md` Recent Changes: cite §H.8 p134, §H.6 p116-117 +
    p118-119, §H.9 p170 / p176 / p181 (LES exclusion citation),
    §H.9 p189 (SSI). Name the row count (15-17 new rows depending
    on OQ-1 + OQ-2) and the rule count (39, unchanged — no new
    diagnostic; this is a refactor PR).
  - `crates/capco/README.md` catalog inventory: append the 15-17
    new rows under the existing 5-row catalog block.
  - `crates/capco/CAPCO-CONTEXT.md` §3.4: cross out the FOUO
    eviction matrix gap and the UCNI bugfix gap; add a one-line
    "closed in PR 4b-C" reference.

---

## 5. Sequencing

```text
[1] design doc addenda + (OQ-1A) vocab sentinels  ─┐
                                                    ├─ parallel
[2] pre-fix UCNI regression test                   ─┘
       │
       │ (Commit 2's regression test is RED on this branch but
       │  asserts the pre-fix bug; it serves as the swap-target
       │  for Commit 5's post-fix correctness test.)
       ▼
[3] Pattern-C strip rows + UCNI bugfix declarative pair
       │
       ▼
[4] FOUO-eviction matrix rows
       │
       │ (Under OQ-4(A) or (B), Commit 4a inserts here; under OQ-4(C)
       │  it's empty.)
       ▼
[4a] (conditional) Pattern D wiring — closure-operator override
     OR one compound-predicate PageRewrite row
       │
       ▼
[5] PageContext imperative-branch deletions + post-fix UCNI test
       │
       │ (Commit 5 swaps the Commit 2 RED regression test for the
       │  post-fix correctness test, atomically with the branch
       │  deletions.)
       ▼
[6] Parity-gate fixtures
       │
       ▼
[7] Doc updates
```

Commits 1 and 2 are independent and parallelizable. Commits 3 and 4
must land in **declaration order** (catalog declaration order
matters for the topological scheduler — see §3.6). Commits 3 and 4
may parallelize across separate `build_page_rewrites()` row inserts
if the implementing agent splits the function into multiple
`vec!`-extension calls, but the simplest path is sequential. Commit
5 depends on **both** Commits 3 and 4 because the deletion comments
in Commit 5 reference rows declared in Commits 3 and 4. Commit 6
depends on all of Commits 3, 4, and 5. Commit 7 is doc-only.

---

## 6. Identifier / naming reservations

### 6.1 Catalog row names

Following the PR 3c.B + PR 4b-B precedent (`capco/{token}-implies-noforn`,
`capco/noforn-clears-rel-to`), Pattern-C rows use the form
`capco/{token}-evicted-by-classified` (passive: the trigger acts on
the row's target) and FOUO-matrix rows use `capco/{trigger}-evicts-fouo`
(active: the trigger names what's being targeted; symmetric with the
existing `*-implies-noforn` shape). The two naming forms are
internally consistent because Pattern C is "X strips itself when Y";
FOUO matrix is "X strips FOUO."

The 17 new row names enumerated in §3.1 + §3.2 are reserved against
the existing 5 catalog rows. The implementing agent runs `grep -n
'capco/' crates/capco/src/scheme.rs` at Commit 3 execute-time to
confirm no name collision; the prefixes `*-evicted-by-classified`
and `*-evicts-fouo` do not exist in the current catalog (verified
2026-05-16).

### 6.2 Token sentinel IDs (under OQ-1(A))

- `TOK_PROPIN = TokenId(143)` (next free after `TOK_BOHEMIA = 142`).
- `TOK_FISA = TokenId(144)`.
- `TOK_RAWFISA = TokenId(145)`.
- `TOK_NNPI = TokenId(146)`.

Implementing agent runs `grep -rn 'TokenId(14[3-9]\|TokenId(15' crates/`
at Commit 1 execute-time to confirm no conflict.

### 6.3 No new rule IDs

PR 4b-C does NOT add any new `E###` / `W###` / `S###` rules. The
rule count stays at 39. Each `PageRewrite::declarative` row is
catalog data, not a registered rule. The `corpus_parity.rs` count
pin and `post_3b_registration_pin.rs` exact-set pin do not change.

---

## 7. Constitution checks

### 7.A Principle V Audit-First Compliance (G13)

PR 4b-C's catalog rows emit `FixIntent` payloads via the
`ReplacementIntent::FactRemove` and `ReplacementIntent::FactAdd`
shapes. **No diagnostic text contains document content** — the
FactRemove payload carries `FactRef::Cve(TOK_*)` (a TokenId,
category-ID category, and `Scope::Page`); the FactAdd payload
carries the same shape with `TOK_NOFORN`. The audit emitter at
`Engine::fix_inner` materializes these as canonical-form bytes via
the renderer, never input-document bytes. The same audit-content-
ignorance property the 4 existing `*-implies-noforn` rewrites have
applies unchanged.

No new diagnostic messages are added. The `proposal.original`
field stays at empty-string for these rows per the #259 precedent
(decoder-path / declarative-row migrations are cross-axis /
content-free; original input bytes do not appear).

### 7.B Principle VII Crate Discipline (engine-crate touches)

PR 4b-C touches `crates/ism/src/page_context.rs` in Commit 5
(imperative-branch deletions). This is an engine crate edit.

Per Constitution VII §IV final paragraph: *"A scheme-adoption PR
MUST NOT edit the engine crates."*

**Reasoning for not invoking the gap-first rule** (mirrors PR
4b-B §7.B verbatim):

1. The 006 engine-rule-refactor is, by definition, a refactor of
   the engine + rule architecture together. The Constitution VII
   §IV scheme-adoption restriction targets scheme-adoption PRs
   (e.g., "add CUI scheme on top of the unchanged engine") — not
   within-006 PRs.
2. PR 4a (commit `fc91852e`) edited `crates/scheme/src/vocabulary.rs`
   (engine crate `marque-scheme`) directly. PR 4b-B Commit 2
   edited `crates/ism/src/page_context.rs` (engine crate
   `marque-ism`) directly. These are the in-tree precedents
   within the 006 series.
3. The Commit 5 deletions remove **buggy** branches (the UCNI
   NOFORN-promotion missing per §H.6 p116/p118) and **stale**
   branches (the FOUO eviction that is now declaratively driven
   by the catalog). Bugfixes + dead-branch deletions in the
   engine crates land where they live.

**Gap-first alternative (NOT taken)**: split into PR 4b-C.0 (Commit
5 alone — the engine-crate deletions, against the corpus
regression harness) + PR 4b-C (Commits 1-4 + 6-7). Not chosen
because (a) the deletions are conceptually inseparable from the
declarative-row migration that replaces them — splitting forces
two corpus-regression cycles for a single semantic correction, (b)
the existing PR 4a + 4b-B precedents within 006 do it the other
way.

If the user wants gap-first applied: split per the structure
above. The internal sequencing inside PR 4b-C does not change.

### 7.C Principle VIII Authoritative Source Fidelity

Every new doc-comment, catalog row name, and design-doc row in PR
4b-C carries a `§X.Y pNN` citation re-verified at authorship.
**Verification procedure** the implementing agent MUST follow
(verbatim from PR 4b-B §7.C):

1. Open `crates/capco/docs/CAPCO-2016.md`.
2. For each citation in the doc-comment / row name / design-doc
   row being written: search the markdown for the page number,
   read the surrounding text, confirm the cited claim is present
   and accurate.
3. Add the propagation-trace tag `// verified 2026-05-16 against
   CAPCO-2016.md` to the source-code citation comment. Design-doc
   citations carry the same tag in Commit 1.
4. If a citation cannot be verified, **remove it**, do not leave
   it in place. The Constitution VIII clause is unambiguous: "A
   citation that cannot be traced to a real passage MUST be
   removed, not left in place pending follow-up."

The reviewer chain (§8.5) checks at least 10% of citations as a
sampling-validation pass. If any fabricated citation is found, the
PR closes with a request to re-verify ALL citations in the PR.

The cited §§ for PR 4b-C are:
- §H.8 p134 (FOUO eviction matrix umbrella rule).
- §H.8 p132 (RSEN), p136 (ORCON), p139 (ORCON-USGOV), p142 (IMCON),
  p148 (PROPIN), p159 (DSEN), p161 (FISA / RAWFISA).
- §H.9 p170 (LIMDIS), p172 (EXDIS), p174 (NODIS), p176 (SBU), p181
  (LES — exclusion confirmation), p189 (SSI).
- §H.6 p116-117 (DOD UCNI), p118-119 (DOE UCNI).

Each carries the propagation-trace tag at the citation site.

### 7.D Principle I Performance

The catalog gains 15-17 entries. The topological scheduler
(`marque-engine::scheduler`) runs Kahn's algorithm once at
`Engine::new` over the full catalog (currently 5 rows; post-PR-4b-C
20-22 rows). **One-time cost: ~4× larger DAG, but the DAG fits in
a single cache line and Kahn's algorithm is O(V+E) — measured cost
is negligible at construction time.**

Per-document cost: each row's predicate fires on every projection
pass. The `Contains` predicate is an `attrs.dissem_iter().any(...)`
scan (verified `scheme.rs:3818` shape). 15-17 new scans per
projection pass at p95 ~10µs each scan = ~150-170µs per projection
pass. **Below the SC-001 noise band.**

**Bench-staleness pre-flight** (per project memory
`project_bench_baseline_staleness.md`): `lint_10kb` baseline is
near-noise-margin (828µs upper-CI, 911µs threshold; PR 4b-B
measurements 594-613µs after the lattice rewrite). PR 4b-C does
NOT touch the hot path (PageContext stays imperative under OQ-3(A)
default); the catalog growth affects only `scheme.project` callers,
which are not on the `Engine::lint` hot path. **Expected delta:
near-zero on `lint_10kb`.**

If the bench fails on the first run, use `gh run rerun <id> --failed`
once before any other action per project memory
`project_bench_baseline_staleness.md`. If it persistently fails
after one re-run, the bench-baseline-refresh PR (separate, not
4b-C) lands first.

### 7.E Principle IV Two-Layer Rule Architecture

PR 4b-C adds zero hand-written rules. The declarative catalog rows
are Layer-2-declarative (per the PR 3c.B Sub-PR 8.F precedent at
`scheme.rs:2130`): they consume Layer-1-generated predicates
(`DissemControl::Pr`, etc.) and codify §H.8 / §H.6 / §H.9 rules
into a structured fact-rewrite. Two-layer split holds.

The vocab sentinel additions under OQ-1(A) are Layer-1 surface
work — adding `TOK_PROPIN` etc. to the static `TokenId` namespace
and extending the static `dissem_to_tok` / `token_to_category_id`
match arms. No generated-code change (those constants are
hand-written in `scheme.rs`; ODNI XML does not produce them).

---

## 8. Test scaffolding

### 8.1 New test files

None. All new tests land in:
- `crates/capco/tests/category_lattice_laws.rs` (Commits 3, 4).
- `crates/capco/tests/page_context_lattice_parity.rs` (Commit 6).
- `crates/ism/src/page_context.rs` `#[cfg(test)]` module (Commits 2, 5).

### 8.2 Extended test files

| File | Δ tests | Commits |
|---|---|---|
| `crates/capco/tests/category_lattice_laws.rs` | +17-19 catalog-presence tests (one per new row) | 3, 4 |
| `crates/capco/tests/page_context_lattice_parity.rs` | +15-20 fixtures | 6 |
| `crates/ism/src/page_context.rs::tests` | +1 RED regression test (Commit 2); −1 RED test + +1 GREEN correctness test (Commit 5) | 2, 5 |
| `crates/capco/tests/cross_axis_dominance.rs` | +1 fixture: `fouo_with_orcon_classified_strips_via_both_rows` (race-condition coverage between class-evicts-FOUO and orcon-evicts-FOUO rows) | 4 |

### 8.3 Corpus fixtures

`tests/corpus/valid/` is at 74 fixtures (per PR 4b-B note). The
parity-gate (Commit 6) iterates **all** of them as the baseline
byte-identity check. The 15-20 new synthetic fixtures in Commit 6
are inline in the test file (Rust string-literal multi-portion
fixtures), mirroring the PR 4b-B synthetic-fixture convention.

**No new corpus files in `tests/corpus/valid/`** — Pattern-C
fixtures are tangential to recognizer accuracy (the existing
corpus has limited FOUO+ORCON / UCNI+classified cases by
construction, since those are mostly malformed inputs).

### 8.4 Coverage target

- ≥80% line coverage on `crates/capco/src/scheme.rs:build_page_rewrites()`
  (the new rows specifically; the existing baseline already passes).
  `cargo llvm-cov --crate marque-capco --fail-under-lines 80`
  runs in CI; the implementing agent runs locally before PR-open.

### 8.5 Mandatory reviewer chain (BEFORE PR-open)

Per project memory `feedback_run_reviewer_before_pr_open.md` and
PR 4b-B §8.5 precedent, the reviewer agents run on the implementing
branch BEFORE `gh pr create`:

1. **rust-reviewer** — borrow-checker, `Send + Sync` invariants on
   new `PageRewrite::declarative` Custom predicates, no
   `OnceCell<Mutex<_>>` hidden state per Constitution VI.
2. **code-reviewer** — general quality, doc-comment §-citation
   propagation tags, naming consistency.
3. **capco-dissem-validator** — Commits 3 + 4 + 5 + 6 (FOUO matrix
   + Pattern-C + SBU/LIMDIS/SSI/NODIS/EXDIS triggers; the
   reviewer's domain).
4. **capco-classification-validator** — Commits 3 + 4 (FOUO
   classified-eviction row's classification predicate).

The chain MUST produce a clean attestation for every commit before
PR-open. Post-PR, Copilot review fires automatically; the chain
above is the load-bearing quality gate.

---

## 9. Risk register

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | **OQ-1 default (A) requires vocab sentinel additions in Commit 1.** If the implementing agent ships under OQ-1(B) instead and the §3 (b) trigger list overstates delivery, FOUO eviction by PROPIN / FISA / RAWFISA / NNPI silently fails. | Med | OQ-1 default is (A) and the design-doc §3 (b) table calls out the four NEW-sentinel rows explicitly; the reviewer chain (capco-dissem-validator) will catch the omission. **Worst case**: PR 4b-C.1 lands the four missing rows in a follow-on. |
| 2 | **OQ-2 default (A) over-evicts FOUO when NODIS / EXDIS are present.** If §H.8 p134 is read strictly as IC-dissem-only ("other dissemination control markings" = §H.8 IC dissem), NODIS / EXDIS (§H.9 non-IC) should NOT evict FOUO. The PM brief is silent on this. | Med | The §H.8 p134 wording reads more naturally as "any non-FD&R dissem"; the corpus has zero NODIS+FOUO / EXDIS+FOUO fixtures (verified 2026-05-16). If reviewer flags the over-eviction, the two rows are a one-line deletion. **Worst case**: PR 4b-C.2 deletes the two rows. |
| 3 | **OQ-3 default (A) inherits PR 4b-B's deferred-execution gap.** Catalog rows are scheduler-validated but execution-deferred until PR 4b-D flips `Lattice::join`. `Engine::lint` output stays driven by PageContext; the deletions in Commit 5 leave a parity-gate-only path for the migrated logic. If a `scheme.project` consumer regresses, the parity gate catches it; if a PageContext consumer regresses (after deletion), unit tests in `page_context.rs::tests` catch it. | Low | Same risk profile as PR 4b-B. The parity gate is the load-bearing test (15-20 new fixtures in Commit 6). |
| 4 | **Topological scheduler rejects same-axis self-reference.** The two `*-ucni-promotes-noforn-when-classified` rows read CAT_DISSEM (to scan for stricter FD&R markers) AND write CAT_DISSEM (to inject NOFORN). Kahn's algorithm in the in-tree scheduler may reject this. | Med | Verify at Commit 3 execute-time by running `cargo test -p marque-engine scheduler::tests`. If rejected, drop the DISSEM read annotation and move the FD&R-marker scan into the Custom predicate body (which reads the input projection directly, not the annotated axes). The `is_fdr_dominator` helper at `scheme.rs:4895` is the in-tree precedent for this. |
| 5 | **Bench `lint_10kb` regression** on the hot path. PR 4b-C does not touch `Engine::lint` (PageContext stays the production driver under OQ-3 default A), but the catalog grows ~4× — `Engine::new`'s scheduler validation adds runtime at construction. | Low | `Engine::new` runs once per process; the cost is O(V+E) in catalog size and below noise. PR 4b-B's lint_10kb baseline 594-613µs is the comparison point; PR 4b-C is expected at the same baseline ± noise. Use the `gh run rerun --failed` mitigation if needed. |
| 6 | **Compound-NF (`SbuNf` / `LesNf`) accidentally stripped** by the bare-token rows. Pattern C rule §3.5 invariant must hold. | High | The `CategoryPredicate::Contains` match is on `TokenId`, and `TOK_SBU ≠ TOK_SBU_NF` (verified `scheme.rs:4911 vs 4915`). The catalog row predicates are mechanical; the invariant holds by construction. Commit 6's `pattern_c_sbu_nf_in_classified_preserves_noforn` fixture is the regression gate. **However**: future refactors that unify `Sbu` and `SbuNf` into a single variant with a flag would break the invariant silently — the §3.5 invariant statement must live in the catalog doc-comment so it survives such a refactor. |
| 7 | **§H.8 p134 citation-discipline lapse**: the umbrella rule on p134 is cited by every FOUO-matrix row (10-12 rows). A single misquote propagates to every row. | Med | Re-verify §H.8 p134 verbatim at Commit 1 + Commit 4 authorship per Constitution VIII; the reviewer chain (code-reviewer + capco-dissem-validator) does the sampling check. |
| 8 | **Doc-comment LOC bloat**: each row gets a ~50-LOC doc-comment in the PR 3c.B precedent style. 15-17 new rows = ~800 LOC of doc-comments alone. The PR risks reviewer-fatigue review. | Low | The doc-comments are mechanical (each row's doc-comment follows the same template: §-citation, trigger, action, axis annotations, scheduler order, runtime execution gap, forward-pointer). Reviewers can pattern-match across them quickly. |
| 9 | **RSEN gap in Trio 1 catalog** (OQ-4(A) only): the original in-tree `CLOSURE_NOFORN_IMCON_DSEN` row covered IMCON + DSEN but NOT RSEN, even though §H.8 p132 + §B.3 Table 2 p21 algebra applies identically. The Pattern D fixture `rsen_classified_caveated_implies_noforn` would have failed under OQ-4(A) without a new closure row. | Closed | Closed by the closure-runtime landing: the row was renamed to `CLOSURE_NOFORN_RSEN_IMCON_DSEN` and extended with `TOK_RSEN` as a third trigger, suppressed by `FDR_DOMINATORS`, cone `[TOK_NOFORN]`, citation `§B.3 Table 2 p21 + §H.8 p132`. |
| 10 | **Pattern D `has_caveat_marker` SCI-uncaveated carve-out** (§B.3 p20 explicit: "If only an SCI marking is present, the information is considered uncaveated"). Under OQ-4(B) the compound predicate must encode the carve-out (`!is_sci_only(attrs)`); easy to miss. Under OQ-4(A) the carve-out is implicit (no closure row triggers on `CAT_SCI`). | Med | Commit 6 fixture `sci_only_uncaveated_no_noforn` is the regression-gate. The capco-classification-validator reviewer agent's pass MUST exercise this case explicitly. |
| 11 | **Citation drift on PM's "p21 → defaults to NOFORN" phrasing**: the PM brief cites "CAPCO-2016 p21" as authority for default-NOFORN. Verified 2026-05-16: p21 carries **Table 2** with the row "Classified + caveated + on/after 28 June 2010 → Mark as NOFORN in IC DAPs / Handle as NOFORN in other IC info." The default-NOFORN claim is correct for IC DAPs (prescriptive) but only "encouraged" for other IC info. Marque defaults to IC-DAP scope per project memory `project_marque_assumes_modern_default_fdr.md`, so the prescriptive read is the right one — but the citation MUST cite **§B.3 Table 2 p21** specifically, not p21 alone. | Low | Use full citation form `§B.3 Table 2 p21` in every Pattern D doc-comment (already the existing in-tree convention for the 7 closure rules at `scheme.rs:5094 et seq`). |

---

## 10. Open questions (deferred and PM-flagged)

The three OQs are surfaced in §0. They are PM-blocking; the
implementing agent MUST NOT execute until they are resolved.

No additional OQs beyond §0 emerged in plan authoring.

---

## 11. Acceptance checklist (for reviewer chain)

- [ ] §0 OQ-1, OQ-2, OQ-3, OQ-4 PM-resolved; resolutions recorded
      in the PR description.
- [ ] **Under OQ-4(A)**: Commit 4a wires `CapcoScheme::closure()`;
      the 7-row Trio 1 catalog activates at runtime; the
      Kleene-fixpoint iteration cap (`MAX_CLOSURE_ITERATIONS`) is
      respected; Risk #9 (RSEN gap) is resolved by adding
      `CLOSURE_NOFORN_RSEN` or by explicit deferral with citation.
- [ ] **Under OQ-4(B)**: Commit 4a ships ONE new
      `PageRewrite::declarative` row `capco/caveated-implies-noforn`
      with compound `Custom` predicate; doc-comment cites in-tree
      Trio 1 duplication as design-debt for PR 4b-D consolidation.
- [ ] **Under OQ-4(C) — default**: Commit 4a is empty; Pattern D
      defers to PR 4b-D; the design-doc §3 (e) addendum (Commit 1)
      cites §B.3 p20 + §B.3 Table 2 p21 as the deferred-PR
      authority.
- [ ] **Under OQ-4(A) or (B)**: Pattern D parity-gate fixtures
      (`caveated_aea_only_implies_noforn`,
      `caveated_non_ic_implies_noforn`,
      `caveated_with_rel_to_no_noforn`,
      `caveated_with_relido_no_noforn`,
      `sci_only_uncaveated_no_noforn`,
      `multiple_caveat_classes_single_noforn`,
      `rsen_classified_caveated_implies_noforn`) pass. The
      `sci_only_uncaveated_no_noforn` fixture is the load-bearing
      regression-gate for the §B.3 p20 SCI-uncaveated carve-out.
- [ ] Commit 1 design-doc §3 (b) table enumerates each FOUO-matrix
      row with its per-token §-citation; every citation
      re-verified at authorship per Principle VIII; propagation-
      trace tag `// verified 2026-05-16 against CAPCO-2016.md`
      present on every new citation in the design-doc + scheme.rs.
- [ ] **Under OQ-1(A)** Commit 1 adds `TOK_PROPIN` / `TOK_FISA` /
      `TOK_RAWFISA` / `TOK_NNPI` with extended `dissem_to_tok` /
      `non_ic_dissem_to_tok` / `satisfies_attrs` /
      `token_to_category_id` arms; the drift-guard comment at
      `scheme.rs:4887-4898` is updated and issue #407 is closed.
- [ ] Commit 2's RED regression test
      `ucni_classified_strip_loses_noforn_promotion_regression`
      asserts the pre-fix bug; the test PASSES on the pre-fix
      branch.
- [ ] Commit 3's 7 declarative rows (5 Pattern-C strip + 2 UCNI
      NOFORN-promotion) ship with §-citations and axis
      annotations; `Engine::new` accepts the catalog (no
      topological cycle, no unannotated `Custom` axes).
- [ ] Commit 4's 10-12 FOUO-matrix rows ship; the
      `fouo_matrix_does_not_fire_on_fdr_tokens` test confirms the
      §H.8 p134 "excluding any FD&R markings" clause is
      respected.
- [ ] Commit 5 deletes
      `expected_dissem_us` step 3 (FOUO eviction) +
      `expected_aea_markings` UCNI strip (UCNI + NOFORN
      promotion); the post-fix `ucni_classified_promotes_noforn_via_pattern_c`
      test PASSES on the post-fix branch.
- [ ] Commit 5's deletions do not regress any
      `page_context.rs::tests` test that touches FOUO / UCNI;
      affected tests re-key to read `scheme.project` output
      where needed.
- [ ] Commit 6 parity-gate fixtures pass — every fixture
      asserts Path A and Path B byte-identity on the resulting
      `CanonicalAttrs`.
- [ ] §3.5 invariant test
      `pattern_c_sbu_nf_in_classified_preserves_noforn` passes:
      Pattern-C strip rows do NOT touch `NonIcDissem::SbuNf` or
      `NonIcDissem::LesNf`.
- [ ] LES exclusion fixture
      `pattern_c_les_in_classified_propagates_to_banner` passes:
      `(U//LES) (S)` rolls LES up to the banner per §H.9 p181.
- [ ] Reviewer chain (rust-reviewer + code-reviewer +
      capco-dissem-validator + capco-classification-validator)
      ran BEFORE `gh pr create`; attestations recorded in PR
      description.
- [ ] `cargo llvm-cov --crate marque-capco --fail-under-lines 80`
      passes locally before PR-open.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] `cargo +stable clippy --workspace -- -D warnings` clean
      (per project memory `feedback_clippy_nightly_vs_stable_drift.md`).
- [ ] `cargo fmt --check` clean.
- [ ] CI bench `lint_10kb` passes (or noise-flakes once with
      `gh run rerun --failed` mitigation; persistent failure
      triggers a baseline-refresh PR first).
- [ ] PR description quotes Principle V (G13 audit-content-
      ignorance), Principle VII (engine-crate-touch reasoning),
      Principle VIII (citation-verification procedure), and
      lists every cited §X.Y pNN with the propagation tag.
- [ ] PR description records the rule count (unchanged at 39 —
      no new diagnostic).

---

## 12. Post-PR

Once 4b-C merges:

- **PR 4b-C.1** (conditional on OQ-1(B)): land
  `TOK_PROPIN` / `TOK_FISA` / `TOK_RAWFISA` / `TOK_NNPI` and the
  4 corresponding FOUO-matrix rows.
- **PR 4b-C.2** (conditional on OQ-2(C)): delete the
  `capco/nodis-evicts-fouo` and `capco/exdis-evicts-fouo` rows.
- **PR 4b-D** flips `Lattice::join`'s hot path to component-wise
  dispatch; PageContext retires as the production banner-validation
  driver. PR 4b-C's Commit 5 deletions become the first piece of
  PageContext to go; the rest follows in 4b-D + 4b-E.
- **PR 5+ Stage 4** absorbs the renderer
  (`MarkingScheme::render_canonical`), which retires the §H.5 /
  §H.4 / §H.8 ordering concerns currently flowing through the
  declarative-row catalog as cross-row sequencing.

The end-state target — ~10 surviving rules across all stages —
remains binding; PR 4b-C is a rule-count-neutral refactor (39 in,
39 out).

---

## Appendix A — File-by-file delta summary

Under defaults (OQ-1(A), OQ-2(A), OQ-3(A)):

| File | Δ LOC | Commits |
|---|---|---|
| `docs/plans/2026-05-01-lattice-design.md` | +~180 | 1 |
| `docs/plans/2026-05-16-pr4b-C-pattern-c-strip-rows-plan.md` (this file) | +~770 (new) | 0 |
| `crates/capco/src/scheme.rs` | +~50 (Commit 1 sentinels) + ~360 (Commit 3 Pattern-C rows) + ~480 (Commit 4 FOUO matrix) + 0/~120 (Commit 4a Pattern D under OQ-4(C)/(A)/(B)) | 1, 3, 4, 4a |
| `crates/capco/src/lattice.rs` | +~10 (Commit 5 mirror — `DissemSet::from_attrs_iter` post-deletion parity) | 5 |
| `crates/ism/src/page_context.rs` | −50 (deletions: 6 LOC FOUO step + 9 LOC UCNI branch + comment alignment) + ~80 (test swap) | 2, 5 |
| `crates/capco/tests/category_lattice_laws.rs` | +~260 | 3, 4 |
| `crates/capco/tests/cross_axis_dominance.rs` | +~30 | 4 |
| `crates/capco/tests/page_context_lattice_parity.rs` | +~300 | 6 |
| `crates/capco/CAPCO-CONTEXT.md` | +~10 | 7 |
| `crates/capco/README.md` | +~8 | 7 |
| `CLAUDE.md` | +~12 | 7 |

**Total net additions (defaults — OQ-4(C))**: ~1750 LOC (~770 plan
+ ~360 production code + ~530 tests + ~30 docs + ~60 incidental).

Under OQ-4(A): +~280 LOC (closure() override + Risk #9 RSEN row +
7 per-row tests + 7 Pattern D fixtures = ~80 + ~40 + ~80 + ~80).

Under OQ-4(B): +~280 LOC (one compound-predicate row + helpers + 7
per-trigger tests + 7 Pattern D fixtures = ~80 + ~40 + ~80 + ~80).

Under OQ-1(B): subtract ~50 LOC (Commit 1 sentinels) and ~190 LOC
(Commit 4 four-row reduction); under Pattern D Path B subtract
~10-20 LOC because the missing TOK_PROPIN etc. don't need
dispatch in the compound predicate (it iterates DissemControl
directly via `is_fdr_dissem`).

Under OQ-2(C): subtract ~95 LOC (Commit 4 two-row reduction).

---

## Appendix B — §-citation index

Citations introduced or propagated by PR 4b-C (each verified
2026-05-16 against `crates/capco/docs/CAPCO-2016.md` at the moment
of writing this plan):

| Citation | Used by | Authority |
|---|---|---|
| `§H.6 p116-117` | Commit 3 (DOD UCNI eviction + NOFORN promotion); Commit 1 design-doc §7 closure | DOD UCNI Precedence Rules |
| `§H.6 p118-119` | Commit 3 (DOE UCNI eviction + NOFORN promotion); Commit 1 design-doc §7 closure | DOE UCNI Precedence Rules |
| `§H.8 p132` | Commit 4 row `capco/rsen-evicts-fouo` doc-comment | RSEN trigger §-cite |
| `§H.8 p134` | Commit 4 every FOUO-matrix row + Commit 3 `capco/fouo-evicted-by-classified` + Commit 1 §3 (b) table | FOUO Precedence Rules (umbrella) |
| `§H.8 p136` | Commit 4 row `capco/orcon-evicts-fouo` doc-comment | ORCON trigger §-cite |
| `§H.8 p139` | Commit 4 row `capco/orcon-usgov-evicts-fouo` doc-comment | ORCON-USGOV trigger §-cite |
| `§H.8 p142` | Commit 4 row `capco/imcon-evicts-fouo` doc-comment | IMCON trigger §-cite |
| `§H.8 p148` | Commit 4 row `capco/propin-evicts-fouo` doc-comment | PROPIN trigger §-cite |
| `§H.8 p159` | Commit 4 row `capco/dsen-evicts-fouo` doc-comment | DSEN trigger §-cite |
| `§H.8 p161` | Commit 4 rows `capco/fisa-evicts-fouo` + `capco/rawfisa-evicts-fouo` | FISA / RAWFISA trigger §-cite |
| `§H.9 p170` | Commit 3 row `capco/limdis-evicted-by-classified` doc-comment | LIMDIS Precedence Rules |
| `§H.9 p172` | Commit 4 row `capco/exdis-evicts-fouo` (OQ-2(A)) | EXDIS §-cite |
| `§H.9 p174` | Commit 4 row `capco/nodis-evicts-fouo` (OQ-2(A)) | NODIS §-cite |
| `§H.9 p176` | Commit 3 row `capco/sbu-evicted-by-classified` doc-comment | SBU Precedence Rules |
| `§H.9 p181` | §1.2 LES-exclusion citation + Commit 6 fixture `pattern_c_les_in_classified_propagates_to_banner` doc-comment | LES propagates regardless of classification |
| `§H.9 p189` | Commit 4 row `capco/ssi-evicts-fouo` doc-comment | SSI §-cite |
| `§B.3 p20` (ICD 403 caveated/uncaveated definition) | Commit 1 design-doc §3 (e) addendum + Commit 4a closure override doc-comment OR Commit 4a PageRewrite row | Pattern D structural definition |
| `§B.3 Table 2 p21` | Commit 1 §3 (e) addendum + Commit 4a + all 7 in-tree Trio 1 ClosureRule labels at `scheme.rs:5094 et seq` | Pattern D default-NOFORN authority |
| `§H.7 p122` | Commit 4a (FGI closure rule) + already in `CLOSURE_NOFORN_FGI:5143` | FGI-specific NOFORN-default |

Each citation MUST be re-verified at the moment of authorship per
Constitution VIII propagation-discipline. The propagation-trace tag
`// verified 2026-05-16 against CAPCO-2016.md` accompanies every
in-source-tree occurrence.
