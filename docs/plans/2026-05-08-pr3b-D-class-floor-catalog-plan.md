<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Implementation Plan: PR 3b.D — `marque-applied.md` §3.4.6 Class-Floor Catalog (T026d)

**Target file**: `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md`
**Branch**: `refactor-006-pr-3b-class-floor` (worktree `/home/knitli/marque-pr3b-D/`, off `origin/staging` at `765ec964` — PR 3b.C just merged)
**Base PR**: against `staging` (NOT `main`).
**Predecessors landed in staging**: PR 3b.A (#319 banner walker), PR 3b.B (#320 transmutations), PR 3b.C (#321 RELIDO conflicts).
**Status**: PM-APPROVED (third pass). Architectural option A locked; row-count locked to §3.4.6 family granularity (~26 rows); all five remaining open items resolved per PM update 2026-05-08. Implementation may proceed.

---

## 0. One-paragraph summary (third pass — granularity locked)

PR 3b.D lands the `marque-applied.md` §3.4.6 per-marking classification floor catalog onto `CapcoScheme` as **~26 declarative `Constraint::Custom` rows at family granularity** matching the §3.4.6 author's table directly. The architectural choice is locked to **Option A — `Constraint::Custom` rows, dispatched through `evaluate_custom_by_attrs`** under Constitution VII §IV (no edits to `crates/scheme/src/constraint.rs` from a scheme-adoption PR; primitive additions like `TokenRef::ClassAtLeast` are PR 3.7's lane via T108b). Family-pattern matching is implemented in the predicate body (e.g., "any HCS-* with subcompartments" iterates `attrs.sci_markings` looking for the family pattern) — no marking-template-level row explosion; one `Constraint::Custom { name, label }` per §3.4.6 author's family entry. The plan retires three pure class-floor rules (E022 CNWDI, E025 UCNI, E027 SAR) and explicitly does NOT retire the six SCI per-system rules (E044 / E045 / E046 / E048 / E049 / E050) because they bundle floor-checking with companion-marking enforcement and the `build_class_upgrade_fix` actionable-upgrade path; class promotion is FixIntent-territory in PR 3c (post-PR D), and the per-system rules retire as a unit under PR 3b.E (T026e). One walker `Rule::id() = "E058"` (next free E### ≥ E058 per running-sequence convention; E058 is unused in the current ruleset). Net rule delta: **3 retired + 1 walker added = net −2; running count 55 → 53.**

The PM 2026-05-08 update corrected a citation-methodology error in pass 1 (the four §3.4.6 page citations RSEN p149 / IMCON p144 / TFNI p107 / EYES p152 are NOT defects — they are operative-authority pages chosen by the §3.4.6 author, accepted as authoritative per "marque-applied.md is the §3.4.6 source of truth") and rejected a row-count expansion proposal in pass 2 (38 rows at marking-template granularity → 26 rows at family granularity per §3.4.6 fidelity). All citations and rows are intact; the granularity is now locked to the §3.4.6 author's choice.

---

## 1. Architectural option (LOCKED — Option A)

**Approved by PM 2026-05-08.** Land ~26 entries as `Constraint::Custom` rows with per-row dispatch in `evaluate_custom_by_attrs`, generalizing the existing E022 pattern.

Reasoning:
1. **Constitution VII §IV.** Scheme-adoption PRs MUST NOT edit `crates/engine`, `crates/scheme`, `crates/core`, `crates/rules`, `crates/ism`. Adding a `TokenRef::ClassAtLeast(ClassLevel)` variant would touch `crates/scheme/src/constraint.rs`, violating this. PR 3.7 owns the primitive question via T108b/T108c.
2. **Existing-pattern fit.** The catalog already contains `E022/CNWDI-classification-floor` declared as `Constraint::Custom` (`scheme.rs:1246`), with the predicate body in `e022_cnwdi_floor` (`scheme.rs:2083`). Generalizing to ~26 family-grouped rows is the natural extension. The Constraint enum's docstring at `constraint.rs:106-114` explicitly cites "CNWDI requires classification ≥ S" as the canonical Custom case.
3. **Stage gating.** Consultation verdict §1: "PR 3b ships the declarative-catalog moves over existing primitives only." T026d's "Requires rows" phrase describes the *semantic shape* (the catalog's logical content), not a strict primitive-reuse constraint at PR 3b. Calling the entries `Custom` in PR 3b is the consultation verdict's pre-approved primitive choice.

**Forward link to PR 3.7.** The planning doc carries an in-source section comment in `scheme.rs` next to the class-floor catalog block:

> ```text
> Class-floor catalog rows are declared as Constraint::Custom because the
> RHS of each row is "classification level ≥ F(M)" — a partial-order
> threshold over the OrdMax classification chain, not a token-presence
> assertion. PR 3.7 (T108b) may revisit and re-classify to a primitive
> form (e.g., TokenRef::ClassAtLeast(ClassLevel) or Constraint::ClassFloor)
> once that primitive lands in marque-scheme. See
> docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md §3.3 for the
> architectural rationale; tasks.md T108b for the primitive landing.
> ```

This makes the choice auditable and the migration path discoverable for the next agent.

---

## 2. The verified §3.4.6 catalog at family granularity (~26 entries)

Verification methodology (corrected per PM 2026-05-08 update):

- **Read each marking's H.x section body** in `crates/capco/docs/CAPCO-2016.md` and confirm the floor-assertion language exists ("Applicable only to TS / S / C / U", "May only be used with TS or S", etc.). The vendored markdown carries `begin page NNN` / `end page NNN` anchors so PDF page numbers can be located when they fall within the marking's body. When `marque-applied.md` cites a page that is the operative authority (precedence-rules section, FD&R-supersession anchor, AEA-chain reference) rather than the marking-template-body page, the citation is accepted as authoritative — `marque-applied.md` is the §3.4.6 source of truth.
- **Per Constitution VIII**, every catalog entry's citation must be re-verifiable. For PR D, "re-verifiable" means: (a) the marking's H.x section in `crates/capco/docs/CAPCO-2016.md` clearly contains floor-assertion prose for the cited floor, AND (b) the cited page exists in the manual (the `begin page NNN` anchor is present), OR (c) for `§H.7 Appendix B` citations, the appendix is named in the manual's ToC + referenced multiple times in the body. The appendix-body vendoring gap is acknowledged but does not invalidate the citation.

### 2.1 Floor `TS` — single classification level (5 family rows)

| # | Family pattern | Floor F(M) | Citation | Marking-body floor language (verbatim sample) |
|---|---|---|---|---|
| 1 | `HCS-[comp][sub]` (HCS with full compartment + subcompartment, e.g., HCS-P [SUB]) | TS | `CAPCO-2016 §H.4` | "May only be used with TOP SECRET. Requires HCS-P, ORCON, and NOFORN." (line 1558, PDF p68 — HCS-P [SUB] template; family covers all HCS subcompartments) |
| 2 | `SI-[comp]` (any SI compartment — SI-G, SI-ECRU, SI-NONBOOK, SI-[any], SI-G [SUB]) | TS | `CAPCO-2016 §H.4` | "Applicable only to Top Secret information. May only be used with TOP SECRET." (lines 1759 / 1763 PDF p76 SI-[COMP] template; lines 1862 / 1865 PDF p80 SI-G template; lines 1812 / 1816 PDF p78 SI-ECRU template) |
| 3 | `TK-BLFH` (TK-BLUEFISH, including TK-BLFH [SUB]) | TS | `CAPCO-2016 §H.4` | "Applicable only to Top Secret information. May only be used with TOP SECRET." (lines 2072 / 2076 PDF p87 TK-BLFH; lines 2127-2131 PDF p89 TK-BLFH [SUB]) |
| 4 | `BALK` (NATO/CTS) | TS (via CTS reciprocal-raise per `marque-applied.md` §3.4.1 Note (i)) | `CAPCO-2016 §H.7 Appendix B` | NATO program; class is reciprocal-raised. `marque-applied.md` §3.4.6 line 790. (Appendix B body not vendored — see §2.5.) |
| 5 | `BOHEMIA` (NATO/CTS) | TS (via CTS reciprocal-raise) | `CAPCO-2016 §H.7 Appendix B` | Same as BALK. `marque-applied.md` §3.4.6 line 791. |

### 2.2 Floor `S` — TS-or-S allowed (8 family rows)

| # | Family pattern | Floor F(M) | Citation | Marking-body floor language (verbatim sample) |
|---|---|---|---|---|
| 6 | `HCS-[comp]` (HCS with compartment, no subcompartment — HCS-O, HCS-P bare) | S | `CAPCO-2016 §H.4` | "May only be used with TOP SECRET or SECRET. Requires ORCON and NOFORN." (line 1446 PDF p64 HCS-O); "May be used with TOP SECRET or SECRET." (line 1501 PDF p66 HCS-P) |
| 7 | `RSV-[comp]` (RESERVE compartment) | S | `CAPCO-2016 §H.4` | "May only be used with TOP SECRET or SECRET. Requires RESERVE." (line 1662 PDF p72 RSV-[COMP]) |
| 8 | `TK` (bare and bare-compartment forms — TK, TK-IDIT, TK-IDIT [SUB], TK-KAND, TK-KAND [SUB]; explicitly excludes TK-BLFH which is row #3 above) | S | `CAPCO-2016 §H.4` | "May only be used with TOP SECRET or SECRET." (line 2021 PDF p85 TK; line 2190 PDF p91 TK-IDIT; line 2304 PDF p95 TK-KAND) |
| 9 | `RD-SG` (RD-SIGMA, any #) | S | `CAPCO-2016 §H.6 p113` | "Applicable only to Top Secret and Secret RD information. May only be used with TOP SECRET or SECRET." (lines 2650 / 2656 PDF p108 RD-SIGMA — note: §3.4.6 cites p113 as the operative authority page, accepted per the methodology in §2 preamble.) |
| 10 | `FRD-SG` (FRD-SIGMA, any #) | S | `CAPCO-2016 §H.6 p113` | "Applicable only to Top Secret and Secret FRD information. May only be used with TOP SECRET or SECRET." (lines 2793 / 2798 PDF p113 FRD-SIGMA — directly cited per §3.4.6) |
| 11 | `RD-CNWDI` | S | `CAPCO-2016 §H.6 p104` | "Applicable only to Top Secret or Secret RD information. May only be used with TOP SECRET RD or SECRET RD." (lines 2582 / 2585 PDF p106 CNWDI — note: §3.4.6 cites p104 as the operative RD-chain authority page, accepted per methodology.) |
| 12 | `RSEN` | S | `CAPCO-2016 §H.8 p149` | Marking-body floor language at line 3247 (PDF p132): "Applicable only to Top Secret or Secret information. May only be used with TOP SECRET or SECRET." Per §3.4.6 author, authoritative citation is §H.8 p149 (operative FD&R / NOFORN-family precedence anchor; `marque-applied.md` §3.4.6 Notes column: "Always-rolls-up + class floor"). |
| 13 | `IMCON` | S | `CAPCO-2016 §H.8 p144` | Marking-body floor language at lines 3505 / 3509 (PDF p142): "Applicable only to TOP SECRET and SECRET information. ... May only be used with TOP SECRET or SECRET." Per §3.4.6 author, authoritative citation is §H.8 p144 (operative §H.8 always-rolls-up grounding). |

### 2.3 Floor `C` — any classified level (TS / S / C) (8 family rows)

| # | Family pattern | Floor F(M) | Citation | Marking-body floor language (verbatim sample) |
|---|---|---|---|---|
| 14 | `SI` (bare control) | C | `CAPCO-2016 §H.4` | "Applicable only to classified information. ... May only be used with TOP SECRET, SECRET, or CONFIDENTIAL." (lines 1706 / 1708 PDF p74 SI) |
| 15 | `SAP` / `SAR` (any program) | C | `CAPCO-2016 §H.5` | "Applicable only to classified information. ... May only be used with TOP SECRET, SECRET, or CONFIDENTIAL." (lines 2452 / 2456 PDF p101 — retires E027) |
| 16 | `RD` (bare) | C | `CAPCO-2016 §H.6 p104` | "Applicable only to classified information. ... May only be used with TOP SECRET, SECRET, or CONFIDENTIAL." (lines 2517 / 2523 PDF p104 RD — directly cited per §3.4.6) |
| 17 | `FRD` (bare) | C | `CAPCO-2016 §H.6 p104` | Marking-body floor language at line 2730 (PDF p111): "May only be used with TOP SECRET, SECRET, or CONFIDENTIAL." Per §3.4.6 author, authoritative citation is §H.6 p104 (RD-chain operative authority; `marque-applied.md` §3.4.6 Notes column: "Same as RD"). |
| 18 | `TFNI` | C | `CAPCO-2016 §H.6 p107` | Marking-body floor language at line 2983 (PDF p120): "Applicable only to classified information. ... May only be used with TOP SECRET, SECRET, or CONFIDENTIAL." Per §3.4.6 author, authoritative citation is §H.6 p107 (AEA-chain anchor; `marque-applied.md` §3.4.6 Notes column: "AEA chain bottom but still classified"). |
| 19 | `ATOMAL` (NATO) | C | `CAPCO-2016 §H.7 Appendix B` | NATO AEA-equivalent; class follows reciprocal raise. `marque-applied.md` §3.4.6 line 805. |
| 20 | `ORCON` (US — covers both bare ORCON and ORCON-USGOV per §3.4.6 single family entry; family predicate fires on either dissem control variant) | C | `CAPCO-2016 §H.8 p136` | "Applicable only to classified information. ... May only be used with TOP SECRET, SECRET, or CONFIDENTIAL." (lines 3356 / 3360 PDF p136 ORCON; lines 3431 / 3435 PDF p139 ORCON-USGOV; both grounded at §3.4.6 line 806/807 → §H.8 p136 operative authority) |
| 21 | `EYES` / `[LIST] EYES ONLY` | C | `CAPCO-2016 §H.8 p152` | Marking-body floor language at line 3873 (PDF p157): "Applicable to only classified information. ... May only be used with TOP SECRET, SECRET and CONFIDENTIAL." Per §3.4.6 author, authoritative citation is §H.8 p152 (operative REL TO/FD&R precedence anchor; `marque-applied.md` §3.4.6 Notes column: "EYES-style FD&R requires classification"). |

### 2.4 Floor `=U` — UNCLASSIFIED-only (ceiling) — split into 2 rows per PM open-item decision #1

PM decision: split DOD UCNI and DOE UCNI into separate rows so each has its own §H.6 sub-page citation. The §3.4.6 author's table groups them; the implementation splits because each marking has its own H.6 page anchor.

| # | Family pattern | Required class | Citation | Marking-body language (verbatim) |
|---|---|---|---|---|
| 22 | `DOD UCNI` (DCNI portion) | =U | `CAPCO-2016 §H.6 p116` | "Applicable only to unclassified information. ... May only be used with UNCLASSIFIED." (lines 2864 / 2867 PDF p116) |
| 23 | `DOE UCNI` (UCNI portion) | =U | `CAPCO-2016 §H.6 p118` | "Applicable only to unclassified information. ... May only be used with UNCLASSIFIED." (lines 2920 / 2924 PDF p118) |

The existing E025 single-rule predicate is split into two catalog predicates (`e025/dod-ucni-conflicts-classification`, `e025/doe-ucni-conflicts-classification`) so the per-row citation is precise. The retired-rule ID `E025` becomes a name-prefix routing two rows; the diagnostic emitter selects the matching predicate and emits one diagnostic per offending UCNI variant present.

### 2.5 NATO Appendix B citation note

Rows 4 (BALK), 5 (BOHEMIA), 19 (ATOMAL) cite `CAPCO-2016 §H.7 Appendix B`. The vendored markdown does not carry the appendix body content, but:

- Appendix B is named in CAPCO-2016 ToC (line 303): "Appendix B - NATO Protective Markings".
- The appendix is referenced 9× in the body (lines 303, 765, 778, 844, 846, 1210, 1259, 3032, 3090).
- The marking templates for ATOMAL / BALK / BOHEMIA are registered in the §G Register (lines 775-777).
- `marque-applied.md` itself cites `§H.7 Appendix B` for these atoms in §3.4.2 (line 618).

The planning doc and per-row `Diagnostic.citation` field carry `CAPCO-2016 §H.7 Appendix B` verbatim. An in-source comment in `scheme.rs` notes the appendix-body vendoring gap and links to a follow-up: "vendor CAPCO-2016 Appendix B body OR re-derive the NATO floor from `marque-applied.md` §3.4.1 Note (i) CTS reciprocal-raise." Path A (keep `§H.7 Appendix B`) is the §3.4.6 author's choice; switching mid-catalog would create asymmetry.

### 2.6 Unknown-floor passthrough sub-catalog (4 family rows per §3.4.6 author's grouping)

Per `marque-applied.md` §3.4.6 unknown-floor sub-catalog (lines 810-819) + §3.7 passthrough policy. Provisional `F(M) = C` (minimal classified). Severity Warn per PM open-item decision #4.

| # | Family pattern (covers all leaf tokens listed) | Provisional floor | Citation form |
|---|---|---|---|
| 24 | `BUR` family (`BUR`, `BUR-BLG`, `BUR-WRG`, `BUR-DTP`) | C (provisional) | `marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped` |
| 25 | `HCS-X` | C (provisional) | `marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped` |
| 26 | `KLM` family (`KLM` / `KLAMATH`, `KLM-R`) | C (provisional) | `marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped` |
| 27 | `MVL` (`MVL` / `MARVEL`) | C (provisional) | `marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped` |

Grouping at the family level (4 rows for ~9 leaf tokens) preserves the §3.4.6 author's grouping choice exactly.

### 2.7 Final tally — 27 rows at family granularity

| Group | Row count | Note |
|---|---|---|
| Floor TS (§2.1) | 5 | §3.4.6 lines 787-791 |
| Floor S (§2.2) | 8 | §3.4.6 lines 792-799 |
| Floor C (§2.3) | 8 | §3.4.6 lines 800-808 (ORCON family combines bare + USGOV per §3.4.6) |
| Floor =U (§2.4) | 2 | UCNI split per PM decision #1 |
| Unknown-floor passthrough (§2.6) | 4 | §3.4.6 lines 810-819 |
| **Total** | **27 rows** | within "~26" PM target |

**Rejection of marking-template granularity (38 rows from pass 2).** The pass-2 plan proposed expanding `SI-[comp]` family into 4 rows (`SI-[COMP]`, `SI-ECRU`, `SI-G`, `SI-NONBOOK`), `TK` family into 5 rows, `HCS-[comp]` family into 2 rows, etc. — net 38 rows. PM rejected this expansion: `marque-applied.md` is the §3.4.6 source of truth, and the §3.4.6 author chose family granularity deliberately (clean lattice algebra; stable ImplTable shape that survives PR 3.7's closure-operator landing without re-shaping; uniform §-citation discipline mixing family-level "§H.4" with specific "§H.6 p113" per the author's design). Family-pattern matching is fully feasible in `evaluate_custom_by_attrs` — the predicate body iterates `attrs.sci_markings` (or the relevant axis) looking for tokens that match the family pattern (e.g., "any HCS-* with subcompartments"). The catalog row's `name` is the family identifier; the `label` is the §3.4.6 author's assigned citation.

The 27-row count vs. PM's "~26" target reflects the UCNI split (PM decision #1) which adds one row to the §3.4.6 author's 22+4=26.

---

## 3. Architectural choice analysis (audit trail)

[Preserved for the planning audit trail. PM-approved as Option A; no decision-point.]

### 3.1 The tension

T026d (verbatim, `tasks.md:105`): "Land `marque-applied.md` §3.4.6 per-token classification floor catalog as ~25 `Constraint::Requires` rows on `CapcoScheme` ..."

The existing `Constraint::Requires` shape (`crates/scheme/src/constraint.rs:79`): `if scheme.satisfies(marking, left) && !scheme.satisfies(marking, right) → violation`. Both sides resolve to `TokenRef::Token(id)` or `TokenRef::AnyInCategory(cat)` — token-presence semantics.

Class-floor semantics is "if X is present, classification level ≥ F(X)" — a partial-order threshold over the OrdMax classification chain (TS > CTS > S > NS > C > NC > R > NR > U > NU), not a token-presence check.

The existing E022 (CNWDI floor S) implements this by declaring the row as `Constraint::Custom` and putting the level-comparison predicate in `e022_cnwdi_floor`. The Constraint enum's docstring at `constraint.rs:106-114` explicitly cites this case as the canonical Custom escape hatch.

### 3.2 Options (compressed)

- **Option A — `Constraint::Custom` rows (LOCKED).** Generalize the E022 pattern. Constitutional fit, existing-pattern fit, audit-stream traceability. Spec text says "Requires" but consultation verdict §1 explicitly authorizes "ships the declarative-catalog moves over existing primitives only" — Custom is the existing-primitive choice for class-floor.
- **Option B — Add `TokenRef::ClassAtLeast(ClassLevel)` variant.** Reads cleanly, but Constitution VII §IV violation. Routed to PR 3.7 (T108b).
- **Option C — Re-purpose `TokenRef::Token` for class tokens.** Semantically polluted; rejected.
- **Option D — Hybrid: A with section-comment migration note.** Operationally identical to A; the comment is the discoverability hook. **This is the actual recommendation, rolled up under "Option A" for brevity.**

### 3.3 Recommendation (locked)

Option A / D. The forward-link section comment in `scheme.rs` names PR 3.7 as the migration vehicle.

---

## 4. Existing class-floor-shaped rules (retirement analysis)

### 4.1 Retiring under PR 3b.D — 3 rules

These three rules are pure class-floor checks (predicate is exactly "if M present, class ≥ L") with no companion-marking insertion or class-upgrade fix logic. They retire cleanly as catalog rows.

| Rule | File:line | What it does | Catalog row replacing it |
|---|---|---|---|
| **E022** `cnwdi-constraint` | `rules_declarative.rs:642-676` (wrapper) + `scheme.rs:2083-2109` (predicate) | "CNWDI may only be used with TOP SECRET or SECRET RD" | row #11 `RD-CNWDI` (the new walker fires it) |
| **E025** `ucni-classification` | `rules_declarative.rs:734-762` (wrapper) + `scheme.rs:e025_ucni_classification` (predicate) | "DOD/DOE UCNI may only be used with UNCLASSIFIED" | rows #22 + #23 (split per PM decision #1) |
| **E027** `sar-classification` | `rules.rs:4338-4378` (still hand-written, NOT yet declarative) | "SAR markings may only be used with TS, S, or C" | row #15 `SAP` |

**Net delta from retirements: −3 `impl Rule` blocks.** Per `feedback_pre_users_no_deprecation_phasing.md`: no alias maps or retained-namespace shims; rewrite freely.

### 4.2 NOT retiring under PR 3b.D — 6 SCI per-system rules

These rules bundle class-floor checking with companion-marking enforcement and `build_class_upgrade_fix` actionable fix paths. They retire as a unit under PR 3b.E (T026e — separate sub-PR after PR D, per `plan.md:304-306`).

| Rule | File:line | What it does (only class-floor portion is in scope for §3.4.6) |
|---|---|---|
| **E044** `hcs-p-subcompartment-top-secret` | `rules_sci_per_system.rs:412-511` | TS-only floor + ORCON requirement + ORCON-USGOV forbidden |
| **E045** `hcs-classification-ceiling` | `rules_sci_per_system.rs:529-581` | TS-or-S floor for HCS-O / bare HCS-P (no-fix Warn — range-ceiling ambiguity) |
| **E046** `si-compartment-top-secret` | `rules_sci_per_system.rs:601-650` | TS-only floor for any SI compartment |
| **E048** `rsv-classification-ceiling` | `rules_sci_per_system.rs:744-784` | TS-or-S floor for RSV (bare or compartmented; no-fix Warn) |
| **E049** `tk-classification-ceiling` | `rules_sci_per_system.rs:801-845` | TS-or-S floor for TK (no-fix Warn) |
| **E050** `tk-blfh-top-secret` | `rules_sci_per_system.rs:860-908` | TS-only floor for TK-BLFH |

**Why these don't retire in PR D**: per T026d, "the constraint walker emits a diagnostic and `FixIntent` (post-PR 3c) proposes the class promotion." These six rules use `build_class_upgrade_fix` to construct an inline `FixProposal` that rewrites the classification token (e.g., `S` → `TS`). That fix machinery is exactly what PR 3c (T034+) sealed canonical primitives + PR 4 wires into the engine via FixIntent. Class promotion is FixIntent-territory, not constraint-walker-territory.

**Overlap policy during the PR-D-to-PR-3b.E window** (PM decision #5): **let both fire**. The PR 3b.D catalog fires diagnostics for `class-floor/HCS-P-sub`, `class-floor/SI-comp`, `class-floor/TK-BLFH`, etc. (all carrying rule ID `E058` with per-row `name` field for differentiation). These overlap with E044 / E046 / E050's diagnostics but **do NOT carry FixProposals** in PR D. The per-system rule carries the fix; the catalog row carries the §-cited descriptor. PR 3b.E (T026e) retires the per-system rules and the catalog row becomes the sole emitter — its reviewer-attestation includes "no orphaned class-floor enforcement after E044-E050 retirement; the catalog row covers it."

### 4.3 NOT retiring under PR 3b.D — companion-bundling rules

| Rule | File:line | What it does |
|---|---|---|
| **E042** `hcs-o-companions` | `rules_sci_per_system.rs:243-339` | HCS-O ORCON+NOFORN companion + ORCON-USGOV forbidden (class floor → E045) |
| **E043** `hcs-p-requires-noforn` | `rules_sci_per_system.rs:347-404` | HCS-P NOFORN companion (no class-floor) |
| **E047** `si-gamma-companions` | `rules_sci_per_system.rs:664-730` | SI-G ORCON companion + ORCON-USGOV forbidden (class floor → E046) |
| **E051** `tk-compartment-requires-noforn` | `rules_sci_per_system.rs:928-967` | TK-{BLFH,IDIT,KAND} NOFORN companion (no class-floor) |

These retire under T026e, NOT here.

### 4.4 Rule-count math for the PR description

- Pre-collapse: 59 `impl Rule` blocks (per `decisions.md` D13 + verdict §4).
- After 3b.A (banner walker): −2 → 57.
- After 3b.B (transmutations): mostly additive, midpoint −2 → 55.
- After 3b.C (RELIDO conflicts): 0 → 55.

**PR D delta**: +1 walker (E058) − 3 retired (E022, E025, E027) = **net −2; running count 55 → 53.**

Catalog deltas (separate from `impl Rule` count): **+27 `Constraint::Custom` rows on `CapcoScheme`** at family granularity. Each row carries its own `name` (used as the per-row identifier in the diagnostic message) and §-citation.

The PR description shows the math:

> PR D: 3 retired (E022 / E025 / E027) + 1 walker added (E058 `DeclarativeClassFloorRule`) = net −2 `impl Rule` blocks; running count 55 → 53.
> Catalog deltas: +27 `Constraint::Custom` rows on `CapcoScheme` at §3.4.6 family granularity (per-row §-citation).
> Within the consultation verdict §4 "0 to −2" projection band for PR D.

---

## 5. Implementation outline

### 5.1 Files touched

- `crates/capco/src/scheme.rs`:
  - Add 27 `Constraint::Custom` declarations to the `CapcoScheme::new` catalog under a new section header "Class-floor catalog (§3.4.6)" with the in-source forward-link comment per §1.
  - Per-row dispatch in a new helper `class_floor_catalog_eval(name, attrs) -> Vec<ConstraintViolation>` invoked from a single `name.starts_with("class-floor/")` arm in `evaluate_custom_by_attrs`. The helper holds a static table `[(ConstraintName, marking_predicate, ClassFloor, CitationStr)]`. Family-pattern predicates iterate the relevant axis (`attrs.sci_markings`, `attrs.aea_markings`, `attrs.dissem_controls`, etc.) looking for any token matching the family.
  - Retired rule predicates (`e022_cnwdi_floor`, the split UCNI predicates, plus a new `e027_sar_classification` extracted from `rules.rs:4338`) become catalog table entries — no separate dispatch arms in `evaluate_custom_by_attrs` for them; their original `name` strings (`E022/CNWDI-classification-floor`, `E025/ucni-conflicts-classification`, `E027/sar-classification`) are part of the catalog under the `class-floor/` prefix, so the existing fast-path (`evaluate_named_constraint`) keeps working.

- `crates/capco/src/rules_declarative.rs`:
  - Add one new `pub(crate) struct DeclarativeClassFloorRule` + `impl Rule` block. `id() → RuleId::new("E058")`. `check` walks the catalog by name-prefix (`class-floor/`) and converts each `ConstraintViolation` into a `Diagnostic` with rule-ID `E058`. Per-row identification lives in the diagnostic message (citing the family identifier) and in the catalog row's `name` field.
  - Retire (delete) `DeclarativeCnwdiConstraintRule` (E022 wrapper) and `DeclarativeUcniClassificationRule` (E025 wrapper) — predicate bodies move into the catalog's static-table form.

- `crates/capco/src/rules.rs`:
  - Retire (delete) `SarClassificationRule` (E027) — predicate body extracted into the catalog.
  - Replace the two retired declarative rule-set entries + the one retired hand-written entry with the single new `DeclarativeClassFloorRule` entry in the `register` order.
  - Update the rule-list doc comment at the top of `rules.rs`.

- `crates/capco/tests/class_floor_catalog.rs` (new):
  - Per-row behavior triplet (~81 tests for 27 rows): `test_<row>_fires_below_floor`, `test_<row>_does_not_fire_at_or_above_floor`, `test_<row>_does_not_fire_when_marking_absent`.
  - E022/E025/E027 anti-regression tests: re-run the existing `rules.rs` unit tests against the catalog walker; output asserts `Diagnostic.rule == E058` AND the diagnostic message identifies the catalog row by name.
  - Severity::Off override test (FR-008): when `.marque.toml [rules] E058 = "off"`, the catalog walker is skipped entirely — no class-floor diagnostics emitted for any row. Per FR-008, an `Off`-severity diagnostic is unrepresentable.
  - Passthrough Warn-severity tests: BUR / HCS-X / KLM / MVL fire at Warn with the §3.7 diagnostic message.
  - Catalog name-uniqueness snapshot.
  - Citation-fidelity test (per PR 3b.C precedent at `tests/citation_fidelity.rs`): every catalog row's `label` parses cleanly + matches one of the verified citation strings in §2 of this plan.

- `crates/capco/README.md`:
  - Update rule-inventory paragraph: E022, E025, E027 retire; E058 added (the new walker).
  - Bump rule count.

### 5.2 Walker rule-ID and per-row identification (PM decision #5)

**Single walker rule-ID: `E058`.** All 27 catalog rows emit diagnostics with `Diagnostic.rule = RuleId::new("E058")`. Per-row identification is via:
- The `Constraint::Custom { name }` field carries a stable per-row identifier (e.g., `"class-floor/HCS-comp-sub"`, `"class-floor/SI-comp"`, `"class-floor/RD-CNWDI"`, `"class-floor/E022/CNWDI-classification-floor"` for the retiring-rule preserved-name case).
- The `Diagnostic.message` text incorporates the family identifier so a reviewer reading the diagnostic stream sees which row fired.
- The `ConstraintViolation.constraint_label` propagates per-row for downstream audit-stream consumers.

This matches PR 3b.A's banner walker pattern (one walker rule, per-category catalog rows differentiated by name).

Severity-config: `[rules] E058 = "off"` toggles the entire walker off (FR-008-correct). Per-row severity-override is NOT supported in PR D — that would require either a per-row rule ID (rejected by PM) or a config-surface extension that isn't in scope here. If a user needs per-row override, they can suppress E058 globally and supplement with org-specific rule sets in PR 3.7+ when the primitive cleanup lands.

### 5.3 Severity defaults (PM decisions #3 + #4)

- Enumerated rows (§2.1, §2.2, §2.3, §2.4, plus NATO rows in §2.1/§2.3): `Severity::Error`. Matches existing E022.
- Unknown-floor passthrough rows (§2.6): `Severity::Warn` per `marque-applied.md` §3.4.6 Q-3.4.6b and `feedback_pre_users_no_deprecation_phasing.md`.

The walker emits diagnostics at the catalog row's declared severity. To support the mixed Error/Warn, the catalog table stores `Severity` per row, and the walker reads it when constructing the `Diagnostic`. The `Rule::default_severity()` for the walker itself is `Severity::Error` (matches the most-restrictive enumerated rows; the Warn passthrough rows override per-row when emitting).

**FR-008 interaction**: when `.marque.toml [rules] E058 = "off"`, the engine skips the walker entirely (per Constitution Principle IV: an `Off`-severity diagnostic is unrepresentable). Per-row Warn rows are reachable only when E058 is Warn-or-above.

### 5.4 Diagnostic message shape

Uniform across the catalog:

```
"<MARKING> requires classification ≥ <FLOOR> (CAPCO-2016 §X.Y pNN); current classification is <CURRENT>"
```

For UCNI ceiling rows:

```
"DOD UCNI may only be used with UNCLASSIFIED information (CAPCO-2016 §H.6 p116); current classification is <CURRENT>"
"DOE UCNI may only be used with UNCLASSIFIED information (CAPCO-2016 §H.6 p118); current classification is <CURRENT>"
```

For NATO rows:

```
"<MARKING> requires classification ≥ <FLOOR> (CAPCO-2016 §H.7 Appendix B; reciprocal-class-raise per marque-applied.md §3.4.1 Note (i)); current classification is <CURRENT>"
```

For passthrough-Warn rows:

```
"<MARKING> is known from ISM but not enumerated in CAPCO-2016; provisional classification floor is C (classified). \
 Verify against the current ODNI manual; current classification is <CURRENT>. (See marque-applied.md §3.7 passthrough policy.)"
```

The `Diagnostic.citation` field carries the verbatim `§X.Y pNN` (or `§H.7 Appendix B`) string from §2.

### 5.5 Span anchoring (PM decision #2)

Diagnostic span = the marking token's span (HCS-O token span, SI compartment span, etc.), NOT the classification token. The marking is what triggered the diagnostic; the classification is the secondary reference. Diagnostic UX puts the squiggle under the offending presence.

For family rows that match multiple sub-tokens in the same portion (e.g., `SI-[comp]` family fires once for `SI-G ABCD`): the span anchors at the first matching token of the family in document order, taken from `attrs.token_spans`.

### 5.6 Custom-dispatch perf

`evaluate_custom_by_attrs` adds one `name.starts_with("class-floor/")` short-circuit at the top:

```rust
fn evaluate_custom_by_attrs(attrs: &CanonicalAttrs, name: &'static str) -> Vec<ConstraintViolation> {
    if name.starts_with("class-floor/") {
        return class_floor_catalog_eval(attrs, name);
    }
    match name {
        // existing 9 entries unchanged (E010, E012, E014, E021, E024, W002, capco/joint-requires-usa, E038)
    }
}
```

Note: `E022/CNWDI-classification-floor`, `E025/ucni-conflicts-classification`, `E027/sar-classification` are removed from the explicit `match` arm because their predicates are now part of the catalog's static table under the `class-floor/` prefix. The catalog stores their original constraint-name strings as the per-row `name` so audit-stream rule-ID continuity is preserved (Diagnostic.rule changes to E058 — the walker — but the per-row identifier in the catalog row's name and the diagnostic message text retains "E022", "E025", "E027" markers for downstream tooling).

`class_floor_catalog_eval` is a static-table lookup; one classification comparison per row that fires; one `ConstraintViolation` if violated. Linear scan over a 27-entry table — branch-prediction-friendly, ≪1µs per call at this scale.

### 5.7 Test coverage

- ≥ 80% line coverage on the new `class_floor_catalog_eval` helper (Constitution VII testing rules).
- All existing E022 / E025 / E027 unit tests pass against the new walker (assert `Diagnostic.rule == E058` AND message text identifies the matching catalog row).
- Citation-fidelity test passes (every catalog row's citation parseable + tied to a real CAPCO-2016 anchor or `§H.7 Appendix B`).
- Snapshot of full catalog rule-name set, asserting no collisions and stable iteration order.

### 5.8 PR description format

T027 reviewer-attestation:

(a) Per-declarative-entry citation: 27 catalog rows, each with verified `CAPCO-2016 §X.Y pNN` (or `§H.7 Appendix B`) citation per §2 of this plan. The four `marque-applied.md` §3.4.6 page citations (RSEN p149, IMCON p144, TFNI p107, EYES p152) are accepted as authoritative per the operative-authority methodology — see §2 preamble.

(b) ≤ 3 branches per `impl Rule` block: the new `DeclarativeClassFloorRule.check` body has the catalog walk (1 branch) + the diagnostic emission (1 branch). Catalog-internal dispatch is in `class_floor_catalog_eval`. ≤ 3 satisfied.

(c) Net rule delta and running count: "PR D: 3 retired (E022, E025, E027) + 1 walker added (E058) = net −2; running count 55 → 53. Catalog deltas: +27 `Constraint::Custom` rows at §3.4.6 family granularity."

---

## 6. Open items — ALL RESOLVED per PM 2026-05-08 update

1. ~~UCNI single-vs-split~~: **split** into 2 rows (rows #22, #23). Per-row §H.6 sub-page citation.
2. ~~Span anchor~~: **marking token**, not classification token. PR 3b.C precedent.
3. ~~Severity for enumerated rows~~: **Error**. Matches existing E022.
4. ~~Severity for passthrough rows~~: **Warn**. Per §3.4.6 Q-3.4.6b.
5. ~~Walker rule-ID convention~~: **single walker `E058`** (next free E### ≥ E058 confirmed by inventory grep — E001-E057 present; E058 unused; E034 also unused but consultation-verdict convention favors running-sequence). Per-row identification via catalog `name` field + diagnostic message text.

PM decision #6: SCI per-system overlap — **let both fire** during PR-D-to-PR-3b.E window. Documented in §4.2.

---

## 7. Acceptance criteria

PR 3b.D is mergeable when:

1. The verified §3.4.6 catalog (27 entries per §2 at family granularity) lands as `Constraint::Custom` rows on `CapcoScheme`.
2. Each row has a verified citation; citation-fidelity tests pass.
3. E022, E025, E027 are retired; their behavior tests still pass against the catalog walker (assertions updated to `Diagnostic.rule == E058` + message text identifies the row).
4. `DeclarativeClassFloorRule` walks the catalog and emits diagnostics with rule-ID E058 and per-row identifying name in the message.
5. `cargo check / clippy --workspace -- -D warnings / fmt --check / test -p marque-capco` clean.
6. `wasm-pack build crates/wasm --target web` clean.
7. `cargo doc --no-deps --workspace` clean.
8. Coverage on `class_floor_catalog_eval` ≥ 80%.
9. PR description includes T027 attestation (a)/(b)/(c).
10. CI passes (Masking-pin caveat retired post-#258 closure in staging).
11. GPG-signed commits.

---

## 8. Implementation status: APPROVED to proceed

PM approved 2026-05-08. All open items resolved per §6. Proceeding to implementation.
