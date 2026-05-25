# Marking-Compiler Edge Census

**Date:** 2026-05-25
**Purpose:** Test the "marking compiler" framing by classifying every CAPCO rule, constraint, rewrite, and closure edge into a token-relation taxonomy — `implies` / typed-`expels` / floor-as-imply / irreducibly-procedural — and running four correctness checks over the resulting graph: implies-acyclicity, expel-typing, no-retraction, and commit-boundary.

> Status: synthesis over three grounded extractions (hand-written rules, declarative constraints, rewrites+closure+FactBitmask). Taxonomy assignment and the four checks are this author's analysis. **Two items require source/code verification before any migration — see §6.** Row counts are as the extractions reported (32 rules, ~47 constraint rows, ~30 rewrites, 6 wired + 4 deferred closure rows); they have not been independently re-counted line-by-line.

---

## 1. The model under test

A marking resolves through two token-relation operators plus a lattice order, in stratified order:

1. **Recognize → commit.** Lexical id; for unknowns, heuristic/decoder → *commit* a token resolution, recording confidence. Probability lives here and nowhere downstream.
2. **`implies` (monotone, additive).** Saturate implied tokens into the IR. Least fixpoint; must be acyclic.
3. **floors = `implies` on the ordered classification axis.** Token ⇒ classification ≥ L. Join-ward, monotone.
4. **`expels` (non-monotone, typed).** Remove tokens, keyed on a token property or a named token — never on co-occurrence. Runs in a stratum *after* implies (the #704 stratification).
5. **Render.** Reorder, delimiters, canonical form.

Then elevate to page/doc scope and re-run the same operators at coarser granularity.

The claim being tested: **almost everything in the rule/constraint/rewrite/closure surface is one of these two operators (or a floor), and the residue is (a) pattern→value rewrites and (b) genuinely procedural page state machines.**

---

## 2. The edge graph

### 2.1 `implies` edges (monotone, additive)

| Trigger | ⇒ adds | Source | Currently lives in |
|---|---|---|---|
| HCS-O | NOFORN, ORCON | §H.4 p64 | closure_table row 1 **+** SCI per-system catalog **+** E010 custom |
| HCS-P | NOFORN | §H.4 p66 | SCI per-system catalog |
| HCS-P-SUB | NOFORN, ORCON | §H.4 p68 | closure_table row 2 **+** SCI per-system |
| SI-G | ORCON | §H.4 p80 | closure_table row 3 **+** SCI per-system |
| TK-BLFH / TK-IDIT / TK-KAND | NOFORN | §H.4 p87/91/95 | closure_table rows 4–6 |
| TK-[compartment] | NOFORN | §H.4 p87 | SCI per-system catalog |
| NODIS | NOFORN | §H.9 p174 | Pattern-A rewrite **+** E038 constraint |
| EXDIS | NOFORN | §H.9 p172 | Pattern-A rewrite **+** E038 constraint |
| SBU-NF | NOFORN | §H.9 p178 | Pattern-A rewrite |
| LES-NF | NOFORN | §H.9 p185 | Pattern-A rewrite |
| DOD-UCNI + classified | NOFORN | §H.6 p116 | Pattern-C rewrite (promote) |
| DOE-UCNI + classified | NOFORN | §H.6 p118 | Pattern-C rewrite (promote) |
| RD / FRD (¬FD&R) | NOFORN | §H.6 p104/p111 | E021 custom (FactAdd 0.95) |
| non-US classification | dissem | §H.7 p122 | E015 Requires (detect-only) |
| caveated (¬FD&R) | NOFORN | §B.3 Table 2 p21 | default_fill row 0 (deferred) |
| NATO classification (¬REL/NOFORN) | REL_TO_USA | §H.7 p127 | default_fill row 7 (deferred) |
| SCI generic (¬FD&R, ¬FGI/JOINT/NATO) | RELIDO | §H.4 templates | default_fill row 8 (deferred) |
| US-collateral classified (¬FD&R/SCI) | RELIDO | §B.3 Table 2 p21 | default_fill row 9 (deferred) |
| JOINT | USA ∈ rel_to | §H.3 p55/57 | E014 / JOINT-USA custom (FactAdd) |
| **27 floor rows** | classification ≥ {C,S,TS} | class_floor_catalog | floor-as-imply on ordered axis |

### 2.2 `expels` edges (non-monotone, typed)

| Trigger | − removes | Key type | Source | Lives in |
|---|---|---|---|---|
| NOFORN | REL_TO | FD&R family + named | §H.8 p145 | noforn-clears rewrite **+** Conflicts constraint |
| NOFORN | RELIDO, DISPLAY_ONLY, EYES | FD&R family | §H.8 p154 | noforn-clears-fdr-family rewrite **+** E054 (FactRemove 0.95) |
| NOFORN | DISPLAY_ONLY_TO | named | §H.8 p145 | noforn-clears rewrite |
| DISPLAY_ONLY / ORCON / ORCON-USGOV | RELIDO | named | §H.8 p136/140/154 | relido-clears rewrites |
| classified | LIMDIS, SBU, SBU-NF, DOD-UCNI, DOE-UCNI, FOUO | classified gate + named token | §H.6/H.8/H.9 | 6 Pattern-C rewrites |
| classification OR non-FD&R control | FOUO | `dissem_has_non_fdr_other_than_fouo` property | §H.8 p134 | 2 Pattern-B rewrites |
| SBU-NF | SBU | named pair | §H.9 p178 | supersession rewrite |
| LES-NF | LES | named pair | §H.9 p185 | supersession rewrite |
| NODIS | EXDIS | named pair (**direction disputed — §6.1**) | §H.9 p172/174 | E041 rule (FactRemove EXDIS) **+** E037 Conflicts (no fix) |
| RD | FRD, TFNI | named (precedence) | §H.6 p104 | E024 custom (detect-only) |
| FRD | TFNI | named (precedence) | §H.6 p120 | E070 custom (FactRemove 1.0) |

### 2.3 Symmetric conflicts with **no** canonical direction (stay detect-only)

| Pair | Source | Why no direction |
|---|---|---|
| JOINT ⊥ RESTRICTED (E016) | §H.3 p56 | author must drop one; context-dependent |
| JOINT ⊥ HCS (E036) | §H.3 p57 | no single canonical resolution |

These are the legitimate residue of symmetric `Conflicts`: the relation genuinely doesn't know which token wins, so detect-only with no fix is correct.

---

## 3. The duplication map (the "two implementations" question, answered)

One **logical** relation is physically scattered across up to four surfaces, because the current vocabulary forces a different expression depending on whether the relation *detects*, *adds*, *removes*, or *surfaces-with-fix*:

| Logical relation | Surfaces it lives in | Count |
|---|---|---|
| **NODIS/EXDIS** | E037 Conflicts (detect) + E041 rule (expel EXDIS) + E038 Requires-NOFORN (constraint) + Pattern-A nodis/exdis-implies-noforn (2 rewrites) | **5** |
| **HCS family** | E010 custom + SCI per-system catalog (rows) + closure_table rows 1–2 + E061/E062 hand rules | **4** |
| **NOFORN/REL TO** | Conflicts constraint + noforn-clears-rel-to rewrite | 2 |
| **NOFORN/RELIDO** | E054 Conflicts (FactRemove) + noforn-clears-fdr-family rewrite | 2 |
| **SBU-NF** | W003 rule (flag) + sbu-nf-evicted-by-classified + sbu-nf-implies-noforn + sbu-nf-supersedes-sbu (3 rewrites) | 4 |
| **RELIDO defaults** | default_fill rows 8/9 + S008 rule (surfaces the same closure as a Suggest) | 2 |

**Verdict:** not two parallel engines, but **one relation per concept fragmented across four mechanisms**. The fragmentation is forced by the vocabulary: `Conflicts` is symmetric (can't carry a fix → spawns a hand-written directional partner), `Requires` is additive-but-detect-only (the add happens elsewhere in closure), `PageRewrite` rewrites silently (no diagnostic → a fix that needs a message spawns a hand rule), and `Rule` carries the diagnostic+fix. A token-keyed `implies`/`expels` pair collapses all four into one declaration per relation.

---

### 3.1 Empirical: the portion-realization gap (verified 2026-05-25)

The fragmentation has a sharper, tested consequence. The `implies`/`expels` algebra is **complete in the page projection** (`project(Scope::Page)` = closure + default_fill + rewrites), but its realization as **portion-level diagnostics/fixes is hand-curated per edge** — an edge fires at portion scope only if someone wrote a mirror `Rule`/constraint. Measured against `Engine::fix` / `Engine::lint` (test `crates/capco/tests/sci_caveated_relido_resolution.rs`):

| Input (portion) | `fix` output | `lint` diagnostics | Edge |
|---|---|---|---|
| `(S//SI//NF/RELIDO)` | `(S//SI//NF)` | 1 — E054 Error | RELIDO⊥NOFORN **has** a portion mirror (§H.8 p154) |
| `(TS//SI-G//OC/RELIDO)` | unchanged | **0** | RELIDO⊥ORCON (§H.8 p136) is **page-rewrite-only**, no mirror |
| `(TS//SI-G//OC)` | unchanged | **0** | caveated⇒NOFORN is default_fill-only; only RELIDO-implication got S008 |
| `(S//SI//RELIDO)` | unchanged | 0 | correct — bare SCI ⇒ RELIDO already canonical |
| `(TS//SI-G//OC/REL TO USA, FVEY)` | unchanged | 0 | correct — REL TO retained, suppresses NOFORN default |

The two `OC` cases produce **zero diagnostics** — the contradiction/obligation isn't even detected at portion scope, because the resolution that handles it lives only in the page projection and never round-trips to portion text. This is the diagnostic-bearing-rewrite gap made concrete: page rewrites are silent, so a resolution they perform is invisible at portion scope unless a human also hand-wrote a mirror rule. A unified pass type (page rewrite that also emits a portion fix) realizes every edge uniformly.

## 4. Taxonomy of the 32 hand-written rules

| Bucket | Count | Rules | Disposition |
|---|---|---|---|
| **Boolean invariant** (detect or directional-expel) | ~9 | E005, E008, E039, E041, E061, E063, W003, W034, E073 | → constraint rows / typed-expels (several already duplicated) |
| **Pattern→value rewrite** (lookup / recanonicalize / reorder / corpus) | ~14 | E002, E006, E007, C001, E062, E064, E065, E066, E067, S004, S007, FGI-suggest, +2 form-mismatch | → **diagnostic-bearing rewrite** (the merge of silent-PageRewrite and fix-Rule) |
| **Irreducibly procedural** (page state machine / set algebra / projection) | 8 | E031, S005, S008, S009, S010, E072, W004, E071 | → stays code (S008 is a procedural wrapper around a declarative closure) |

The middle bucket is the architectural finding: **pattern→value rewrites have no declarative home today** because `PageRewrite` is silent and `Rule` isn't a scheduled pass. They're the residue forced into hand-code by the rewrite/diagnostic split.

---

## 5. The four checks

### CHECK 1 — Is the `implies` graph acyclic? **PASS (strongly).**
Trigger set = {SCI compartments, non-IC dissem, AEA, classification, NATO, JOINT}. Target set = {NOFORN, ORCON, RELIDO, REL_TO_USA, USA, class-level}. **The two sets are disjoint** — no implied token is itself an implies-trigger. The graph is therefore not merely acyclic but **depth-1**: closure reaches fixpoint in a single pass, no iteration. This matches the wired `closure_table` being a flat union.

**One hazard at the implies/implies interaction — see §6.2.** Two *mutually-exclusive* targets (NOFORN and RELIDO, which conflict) can both be implied by a plain SCI marking (caveated⇒NOFORN row 0 **and** SCI⇒RELIDO row 8). Depth-1 acyclicity does **not** by itself prevent producing a NOFORN+RELIDO conflict; that requires the gates to be mutually exclusive or precedence-ordered. Needs verification.

### CHECK 2 — Is `expels` fully typed (property/token-keyed, never co-occurrence)? **PASS, with a reuse gap.**
Every expel edge keys on either a named token (`−EXDIS`, `−TFNI`, `−RELIDO`) or a family predicate (`is_fdr_dissem` / `MASK_FDR_DOMINATORS`, `dissem_has_non_fdr_other_than_fouo`). None key on proximity or "things near X." **The SBU//NF safety property holds:** the classified-strip edges remove exactly `−SBU` / `−SBU-NF` and never touch NOFORN; NOFORN is only ever removed by the FD&R-family expel that NOFORN *itself* triggers. NOFORN is never collateral damage.

**Gap:** the "unclassified-only" class is expressed as **6 separate per-token Pattern-C rewrites** rather than one property-keyed expel over a tagged set. There is no single `unclassified_only` property tag; it's implicit and duplicated. CUI (with its own unclassified-only set) would re-author all six. The reusable form is one `expels-when-classified: {set tagged unclassified-only}`.

### CHECK 3 — Any retraction-requiring edge? (Does an implied token become unlawful when its implier is expelled?) **PASS.**
Walked every expelled token against what it implies:
- **SBU-NF** expelled by classified → implied NOFORN; NOFORN lawful at classified. ✓ (the canonical case)
- **DOD/DOE-UCNI** simultaneously promote NOFORN and are evicted by classified; NOFORN lawful without UCNI. ✓
- **NODIS/EXDIS** imply NOFORN; expelling one leaves the other implying NOFORN; classified doesn't expel them. ✓
- **RELIDO, REL_TO, FOUO, LIMDIS, SBU, LES** when expelled imply nothing additive. ✓

Every implies-target is a handling/release flag (NOFORN, ORCON, RELIDO, REL_TO_USA) lawful across classification levels and independent of the specific implier. **The no-provenance, materialize-then-evaluate IR is licensed.** SBU//NF is representative, not special.

**Dependency:** CHECK 3's pass requires the UCNI promote+evict (and all imply/expel pairs) to evaluate gates against **committed input**, materialized in stratified order (implies stratum, then expels stratum). This is exactly what #704 enforced (closure stays monotone; suppressors relocated to post-close default-fill) and what the topological scheduler does for the rewrite stratum (promote writes DISSEM reading AEA; evict writes AEA → scheduler orders promote before the AEA-evict). The model is **already implemented as stratified passes** — the pipeline describes what the code grew into.

### CHECK 4 — Does the commit boundary hold (no posterior read by the deterministic algebra)? **PASS.**
closure / default_fill / rewrites / constraints all read `FactBitmask` / `CanonicalAttrs` structural fields — no confidence. The corpus-prior rules (S004, S007, FGI-suggest, E062) read `country_code_log_prior` but emit **Suggest only** (fixed 0.5–0.85), never auto-apply, and never mutate the IR the algebra reads. They are correctly isolated as a frontend recognition-aid stratum outside the deterministic core.

---

## 6. Verification — both resolved (2026-05-25)

### 6.1 NODIS/EXDIS direction — RESOLVED: NODIS expels EXDIS
NO DISTRIBUTION ⊐ EXCLUSIVE DISTRIBUTION — NODIS is the more restrictive control, so it supersedes. The code (`nodis-supersedes-exdis-in-portion` → `FactRemove(EXDIS)`) is correct; the extraction's "EXDIS subsumes NODIS" gloss was wrong. Unified edge: **`NODIS ⇒ −EXDIS`** (§H.9 p172/174). No code change.

### 6.2 caveated⇒NOFORN vs SCI⇒RELIDO — RESOLVED: not a hazard; model + implementation both correct
**Ruling:** all bare SCI controls deliberately imply RELIDO (uncaveated, release-eligible by designated official); SCI compartments/sub-compartments are the *reducers* to NOFORN. `(S//SI)` ⇒ `(S//SI//RELIDO)` is valid; **SI does not imply NOFORN.**

Verified directly against `crates/capco/src/scheme/default_fill.rs`:
- `ROW0_CAVEATED_TRIGGERS` (lines 140–159) enumerates exactly 20 atoms (SAR, AEA family, FGI, caveated IC dissem ORCON/ORCON-USGOV/RSEN/IMCON/PROPIN/DSEN/FISA/RAWFISA, non-IC LIMDIS/LES/NNPI/SBU/SSI). **`SCI_PRESENT` (bit 37) is deliberately absent** — bare SCI is uncaveated (doc comment line 204–206 cites §B.3 p20 Note).
- Row 8 (`ROW8_SCI_PRESENT_TRIGGER`, line 165) fires `SCI_PRESENT ⇒ +RELIDO`, gated `MASK_FDR_OR_RELIDO_INCOMPAT == 0`.

Confluence holds at every path:
| Input | Path | Result |
|---|---|---|
| bare `SI` | SCI_PRESENT ∉ caveated; Row 8 fires | **+RELIDO** |
| `SI-G` | compartment sentinel ∈ incompat mask → Row 8 gate-suppressed; compartment ⇒ ORCON ⇒ caveated NOFORN | **NOFORN** (test `row8_suppressed_on_sci_plus_si_g_compartment`, line 481) |
| `S//SI//ORCON` | Row 0 (+NOFORN) and Row 8 (+RELIDO) both fire on frozen snapshot; NOFORN-in-delta supersession overlay in `apply_closed_bits_to` strips RELIDO (§H.8 p145) | **NOFORN** |

The 6.2 hazard was an artifact of the extraction summarizing the caveated set as "(20 atoms)" without enumerating them; SCI is correctly excluded.

**Residual (test-coverage only, not a defect):** the `S//SI//ORCON` path (bare SCI + caveated control → both rows fire → supersession strips RELIDO) appears covered only indirectly; the directly-tested case is SI-G gate-exclusion. A targeted test exercising the supersession-strip path would close the coverage gap.

---

## 7. Recommendation

Declare two token-relation primitives as **vocabulary data on the token**:

- `implies: [tokens]` — monotone, depth-1, saturated into the IR before evaluation.
- `expels: { property | tokens }` — typed, runs in the post-implies stratum.
- floors are `implies` on the ordered classification axis.

This absorbs: all 6 closure rows + 4 default-fill rows + all Pattern-A promotes + all Requires constraints + 27 floors (→ `implies`); all directional Conflicts (E037, E054, NOFORN/REL TO, E024, E070) + all Pattern-B/C strips + supersession + noforn-clears + relido-clears (→ typed `expels`). It eliminates the 5-way NODIS/EXDIS and 4-way HCS fragmentation.

It leaves three things genuinely separate:
1. **Diagnostic-bearing rewrites** — merge silent `PageRewrite` with fix-carrying `Rule` into one pass type so pattern→value rewrites (recanonicalization, form-fix, reorder, corpus-suggest) have a home (~14 rules).
2. **Irreducibly procedural** page state machines (8 rules).
3. **Symmetric no-direction conflicts** (E016, E036) — detect-only.

**CUI payoff:** `implies` / `expels` / floors as vocabulary data means CUI declares its token relations as *data* and inherits the evaluator, scheduler, stratification, and FactBitmask machinery unchanged. Only CUI's pattern→value rewrites and procedural rules are net-new code. The ~9 boolean-invariant CAPCO rules currently hand-written are exactly the per-grammar tax CUI would otherwise pay again.

---

## 8. Forward direction — resolution architecture (RFC #799)

The portion-realization gap (§3.1) opens onto a larger reframe, captured as RFC **#799**. Direction agreed 2026-05-25:

- **Resolution is decoupled from fixing.** Resolve every portion always (even with fixing off — Marque knows what it *should be*); fixing is optional *application* of the resolution to text.
- **No silent global mutation.** Page rewrites today mutate the projected marking with no record. A derivation ("RELIDO expelled because ORCON supersedes"; "NOFORN added because caveated") is a decision that should be recorded, not invisible (Constitution V).
- **Scope hierarchy, forward + reverse:** portion → page → document → bundle/collection.
  - Banners are *derivative of portions* — derive when portions exist; fall back to standalone banner validation when they don't.
  - Page-level banner is the default (granular = better usability); **document-level banner derivation** (the more common real-world practice) is an optional **style rule, off by default**.
  - Reverse validation: check an overall/banner against all markings on all pages.
- **Document "classified up to" front marking** must be resolved and validated.
- **Bundle/collection conditional classification** (e.g. unclassified email body + classified attachments → "overall SECRET//NOFORN; UNCLASSIFIED when separated").
- **Pipeline order:** resolve portions → resolve banner (page's or document's resolved portions, per settings) → resolve bundle/front-page overall. Never revisit portions unless they change; if banners-per-page is set, derive the overall from banners without reconsulting portions.

The `implies`/`expels` unification (§7) is the per-token-relation substrate; #799 is the scope/lifecycle architecture above it. The two motivating cases are committed as `#[ignore]` spec-fixtures in `crates/capco/tests/sci_caveated_relido_resolution.rs`, flipping to passing when portion-level realization lands.
