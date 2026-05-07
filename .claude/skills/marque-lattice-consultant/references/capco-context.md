<!--
SPDX-FileCopyrightText: 2026 Adam Poulemanos

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# CAPCO-2016 Agent Context Helper

> **Vendored copy** (as of 2026-05-06) from
> `/home/knitli/marque/crates/capco/CAPCO-CONTEXT.md` — the marque source
> tree. The original is authoritative; this copy exists so the lattice
> consultant skill is self-contained without requiring the marque source
> tree to be open in context. **If this file disagrees with the original,
> the original wins** — re-vendor when investigating CAPCO-current
> questions. The original is updated when CAPCO-2016 schemas or rules
> change; there is no automatic sync to this copy. Per Constitution
> Principle VIII (Authoritative Source Fidelity), this dual-copy
> arrangement is acceptable only because every claim remains traceable to
> `crates/capco/docs/CAPCO-2016.md`, not to this snapshot.

> **Source.** Every claim here is traceable to the vendored authoritative
> source `crates/capco/docs/CAPCO-2016.md` (PDF original at
> `crates/capco/docs/original-refs/CAPCO-2016.pdf`). Citations cite
> `CAPCO-2016 §X.Y pNN`; verify against the markdown before propagating
> elsewhere. Per Constitution Principle VIII, a citation that cannot be
> traced to a real passage is a correctness defect.
>
> **Purpose.** This file is a curated baseline for any agent working on
> marque so that core CAPCO concepts — categories, FD&R, banner
> roll-up, marking order, per-marking metadata — are loaded without
> reading the full ~200-page manual.
>
> **Scope.** ISM/CAPCO is the MVP application of the marque rule
> engine, not its identity. This helper is for marque-capco /
> marque-ism work. Cross-domain engine work (marque-engine,
> marque-scheme) does not need this loaded.

---

## 1. IC Marking Categories and Separators (CAPCO-2016 Figure 2, p17)

The figure on page 17 fixes the *category lattice* (which category goes
where in a banner) and the *separator alphabet*. Both are normative.

### 1.1 Category order in a banner / portion

```
┌─ US Classification ──┬─ Non-US Classification ──┬─ Joint Classification ──┐
│   (mutually exclusive — exactly one of these three slots is filled)       │
└────────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
              ┌─ SCI Control System ─┬─ Special Access Program ─┬─ AEA Info ─┐
              └────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
                        ┌─ Foreign Government Information ─┐
                                  │
                                  ▼
                        ┌─ Dissemination Controls ─┐
                                  │
                                  ▼
                        ┌─ Non-IC Dissemination Controls ─┐
```

**Sample banner shape** (categories shown in order; FD&R appears within
the relevant dissem categories, never as a stand-alone category):

```
CLASSIFICATION//SCI₁-XXX/SCI₂//SAP//AEA//FGI XXX//DISSEM₁/DISSEM₂//DISSEM
            ^^                                                       ^^^^^^
   single classification slot                              non-IC dissem
```

### 1.2 Separator alphabet (identical for banner and portion)

| Separator | Role | Example |
|-----------|------|---------|
| `//` | separates **marking categories** | `SECRET//SI//NOFORN` (class // SCI // dissem) |
| `/`  | separates **multiple values within one category** | `OC/NF` (two dissem controls); `SAR-BP/CD` (two SAPs) |
| `-`  | links a **marking to a sub-marking** | `SI-G` (SI control + GAMMA compartment); `RD-SIGMA 20` |
| ` ` (space) | separates **multiple sub-markings** and **trigraph/tetragraph codes in FGI/JOINT** | `SI-G ABCD EFGH`; `FGI GBR JPN`; `JOINT S CAN GBR USA` |
| `,` (comma + space) | separates **multiple trigraph/tetragraph codes in REL TO and DISPLAY ONLY** | `REL TO USA, CAN, GBR` |

**Mnemonic for delimiter conflation (real-world bug class).** Country
lists are space-separated in JOINT and FGI but comma-separated in REL
TO and DISPLAY ONLY. EYES ONLY (NSA SIGINT, deprecated) used `/`
between countries, which collides with the dissem-category separator.
SAR previously required `SAR-` repeated before each program (legacy);
the modern form is `SAR-A/B/C`. Historical-batch correction must
tolerate these.

> Authority: CAPCO-2016 §A.6 (Formatting) pp 15–17, Figure 2 p17.

---

## 2. FD&R Markings Summary (CAPCO-2016 §B.3 Table 2, pp 21-22)

Foreign Disclosure & Release. Determines what foreign-disclosure
marking a portion **must** carry, vs. what marking is **encouraged
but not required**. **For marque, every "marking encouraged but not
required" cell is the default trigger condition for a Warn-severity
rule that proposes the encouraged marking.**

| Portion content | IC DAP (mandatory) | Other IC info (encouraged, marque Warn) |
|-----------------|---------------------|------------------------------------------|
| Classified, **uncaveated**, on/after **28 Jun 2010** | Mark `RELIDO` | Handle as `RELIDO`; suggest `RELIDO` |
| Classified, **caveated**, on/after **28 Jun 2010** | Mark `NOFORN` | Handle as `NOFORN`; suggest `NOFORN` |
| Classified, uncaveated/caveated, **prior to 28 Jun 2010** | Mark `NOFORN` | Handle as `NOFORN`; suggest `NOFORN` |
| FGI without FD&R markings | Mark `NOFORN` | Handle as `NOFORN`; suggest `NOFORN` |
| Unclassified + caveated **IC** info (excludes DoD/DOE UCNI, DSEN, non-IC dissems) | Encouraged but not required | Handle as marked if present; else handle per banner |
| Unclassified + caveated **non-IC** info (incl. DoD/DOE UCNI, DSEN, non-IC dissems) | Mark per source's overall classification | Handle as marked if present; else handle per banner |
| Unclassified, uncaveated | Per internal agency procedures | n/a |

**Pivot date.** 28 Jun 2010 splits "caveated → NOFORN" (older) from
"uncaveated → RELIDO" (newer). A correct rule MUST read
`Authority::Originated` from the CAB; it cannot infer the pivot from
the document text alone.

**Caveats** (a portion is "caveated" if it bears) per §B.2 / §H.8:
ORCON / ORCON-USGOV, IMCON, PROPIN, FISA, DEA SENSITIVE, RSEN, FOUO,
or any non-IC dissem (LIMDIS, EXDIS, NODIS, SBU, SBU-NF, LES, LES-NF,
SSI). NOFORN/REL TO/RELIDO/EYES ONLY/DISPLAY ONLY are themselves FD&R
markings, not the upstream "caveat" trigger.

**Marque encoding.** `marque-capco` SHOULD model the seven Table-2
rows as Warn-level rules with a `confidence` reflecting how
strongly the source dictates the suggestion. Apply-with-fix is
permissible only when the date pivot is unambiguous (CAB present and
parsed) and the caveat status is decidable from the portion text;
otherwise emit a Warn diagnostic with no fix.

> Authority: CAPCO-2016 §B.3 Table 2, pp 21–22.

---

## 3. Banner Line Roll-Up Rules (CAPCO-2016 §D.2, pp 28-30)

The banner line is computed by aggregating all portions on the page.

### 3.1 General roll-up algorithm (§D.2 prose, p28)

1. **Classification.** Take the maximum classification level across
   all portions. Exception: ISOO §2001.13(c) / §2001.24(g)
   classification-by-compilation may set the banner higher than the
   max — driven by classifier judgment, not text.
2. **SCI / SAP / AEA.** Repeat every **unique** SCI, SAP, and AEA
   marking from the portions in the banner. Where SCI and SAP
   compartments collide on the same digraph/trigraph, prefix with
   `//SAR-` to disambiguate the SAP category.
3. **FGI.** If **all** portions have *unconcealed* FGI sources (e.g.
   `(//GBR S)`), banner is `FGI [LIST]` with the union of country
   trigraphs/tetragraphs. If **any** portion has *concealed* FGI
   (e.g. `(//FGI S)`), banner is the bare `FGI` (no list).
4. **Dissem / Non-IC Dissem.** Repeat every unique and *most
   restrictive* IC and non-IC dissem control. FD&R precedence is
   Table 3 below; precedence for other dissems is the per-marking
   §H.8 templates.

### 3.2 FD&R Precedence Table 3 (§D.2, pp 28-30)

Twenty-seven scenarios. Read as: "if some portion has X and other
portions have Y, the banner FD&R becomes Z".

| # | Some portion(s) have | Other portion(s) have | Banner FD&R |
|---|----------------------|------------------------|-------------|
| 1 | NF | no FD&R | NOFORN |
| 2 | NF | REL TO / RELIDO / USA-LIST EYES / DISPLAY ONLY | NOFORN |
| 3 | NF | SBU-NF | NOFORN (IC dissem) |
| 4 | no FD&R | SBU-NF | NOFORN (IC dissem) |
| 5 | mix → NOFORN | SBU-NF | NOFORN (IC dissem) |
| 6 | NF | LES-NF | NOFORN (IC dissem) |
| 7 | no FD&R | LES-NF | NOFORN (IC dissem) |
| 8 | mix → NOFORN | LES-NF | NOFORN (IC dissem) |
| 9 | REL TO [USA,LIST] | REL TO [USA,LIST] (no common LIST) | NOFORN |
| 10 | REL TO | RELIDO | NOFORN |
| 11 | REL TO | DISPLAY ONLY (no common LIST) | NOFORN |
| 12 | REL TO / RELIDO | no FD&R | NOFORN |
| 13 | REL TO | USA/LIST EYES (no common LIST) | NOFORN |
| 14 | REL TO | SBU-NF | NOFORN (IC dissem) |
| 15 | REL TO | LES-NF | NOFORN (IC dissem) |
| 16 | REL TO | no FD&R | NOFORN |
| 17 | RELIDO | no FD&R | **NOFORN or RELIDO** (depends on origination date + non-FD&R caveats; cf. Table 2) |
| 18 | RELIDO | DISPLAY ONLY | NOFORN |
| 19 | DISPLAY ONLY | no FD&R | NOFORN |
| 20 | DISPLAY ONLY | DISPLAY ONLY (no common LIST) | NOFORN |
| 21 | REL TO | REL TO (with common LIST) | REL TO [USA, common LIST] |
| 22 | REL TO | USA/LIST EYES (with common LIST) | REL TO [USA, common LIST] |
| 23 | REL TO USA, TEYE / ACGU / FVEY | REL TO [USA, LIST] | REL TO [USA, LIST] (TEYE/ACGU/FVEY *expanded* for common roll-up) |
| 24 | RELIDO | RELIDO | RELIDO |
| 25 | DISPLAY ONLY | DISPLAY ONLY (with common LIST) | DISPLAY ONLY [common LIST] |
| 26 | DISPLAY ONLY | REL TO (with common LIST) | DISPLAY ONLY [common LIST] (release implies disclosure) |
| 27 | REL TO/DISPLAY ONLY | REL TO/DISPLAY ONLY (with common LISTs in each) | REL TO [common]/DISPLAY ONLY [common] |

**Marque gap (current, 2026-05-02).** Our `PageRewrite` layer covers
the symmetric meet for SCI / SAP / AEA categories and the supersession
for `NOFORN clears REL TO` (rule 1+2). The FD&R rows that are NOT yet
fully implemented:

- Rule 17: RELIDO date-pivot + non-FD&R-caveat ambiguity (depends on
  Table 2 lookup).
- Rule 23: `TEYE / ACGU / FVEY` tetragraph **expansion** during
  common-LIST roll-up.
- Rule 26: cross-axis "REL TO + DISPLAY ONLY → DISPLAY ONLY when
  release-implies-disclosure".
- Rule 27: dual-channel REL TO/DISPLAY ONLY composition where each
  channel has its own common-LIST.

These are tracked against the lattice / engine refactor at
`docs/plans/2026-05-01-lattice-design.md`.

> Authority: CAPCO-2016 §D.2 + Table 3, pp 28–30.

---

## 4. Marking Order: Register Table 4 (CAPCO-2016 §G.1, pp 36-38)

The Register's listed sequence **is** the required order in banner
and portion. Authority: §G.1 p36 — *"All markings used in a banner
line and portion mark must be in accordance with the values listed
in the Register, ... and follow the order in which they appear in
this list."*

### 4.1 Top-level groups (in required order)

1. **US Classification Markings** — TS / S / C / U
2. **Non-US Protective Markings** — Appendix A (non-US class), B (NATO), C (UN)
3. **JOINT Classification Markings** (US is co-owner) — `JOINT TS/S/C/U [LIST]`
4. **SCI Control System Markings** — HCS, RESERVE, SI (with G, ECRU,
   NONBOOK), TALENT KEYHOLE (with BLUEFISH, IDITAROD, KANDIK), and
   their compartments / sub-compartments
5. **Special Access Program Markings** — `SAR-[program]`
6. **Atomic Energy Act Information Markings** — RD (with CNWDI,
   SIGMA), FRD (with SIGMA), DOD UCNI, DOE UCNI, TFNI
7. **Foreign Government Information Markings** — `FGI [LIST]` /
   `FGI` (concealed), with FGI-internal SAP and dissem nesting
8. **Dissemination Control Markings** (IC) — RSEN, FOUO, ORCON,
   ORCON-USGOV, IMCON, NOFORN, PROPIN, REL TO, RELIDO, USA/LIST
   EYES ONLY, DEA SENSITIVE, FISA, DISPLAY ONLY
9. **Non-IC Dissemination Control Markings** — LIMDIS, EXDIS,
   NODIS, SBU, SBU NOFORN, LES, LES NOFORN, SSI

### 4.2 Within-group ordering

Within group 8 (IC dissem) and group 9 (non-IC dissem), multiple
entries are listed **in the order they appear in the Register**
(§H.8 / §H.9 prose), separated by `/` with no interjected space.
That ordering is fixed — `OC/NF` is correct, `NF/OC` is not, even
though both name the same set.

For SCI / SAP, within-category multi-value ordering is
**ascending sort** (numeric first, then alphabetic), separated per
the §A.6 separator alphabet (`/` for control systems, `-` for
compartments, ` ` for sub-compartments).

### 4.3 Marque gap (current, 2026-05-02)

We validate **presence** of correct markings and **inter-category**
order via the `//`-separated category sequence, but we do **not**
yet enforce the **within-group dissem ordering** or the
**ascending-sort** rule for SCI/SAP multi-values across the entire
banner. This is a candidate for a new `W###`-class rule per
`docs/plans/2026-05-02-engine-refactor-consolidated.md` PR-3c.

> Authority: CAPCO-2016 §G.1 + Table 4, pp 36–38.

---

## 5. Section H Per-Marking Matrix

Compact reference for every marking template in §H. For each
marking: subsection, page, banner title (long form), banner
abbreviation, portion mark, sponsor + citation basis, key
relationships, banner-precedence summary, commingling summary.

Read **"requires X"** as "this marking MUST co-occur with X in
the same portion / banner". Read **"cannot use with X"** as
"this marking and X are mutually exclusive within a portion /
banner". `<class>` placeholder = TS / S / C / U.

### 5.1 §H.1 US Classification (pp 47–54)

| Marking | § | p | Banner Title | Banner Abbr | Portion | Sponsor / Basis | Relationships | Banner precedence | Commingling |
|---|---|---|---|---|---|---|---|---|---|
| TOP SECRET | H.1 | 47 | TOP SECRET | (none) | TS | OCA / EO 13526 §1.2(a) | excludes US-U/C/S, all non-US, all JOINT | dominates S/C/U; always rolls up | combinable with lower class; TS conveys in portion mark; OK with SCI/SAP/AEA/FGI/dissem |
| SECRET | H.1 | 48 | SECRET | (none) | S | OCA / EO 13526 §1.2(a) | excludes US-U/C/TS, all non-US, all JOINT | dominates U/C | combinable with lower class; OK with SCI/SAP/AEA/FGI/dissem |
| CONFIDENTIAL | H.1 | 50 | CONFIDENTIAL | (none) | C | OCA / EO 13526 §1.2(a) | excludes US-U/S/TS, all non-US, all JOINT | dominates U | combinable with lower class; OK with SCI/SAP/AEA/FGI/dissem |
| UNCLASSIFIED | H.1 | 51 | UNCLASSIFIED | (none) | U | EO 13526 §1.6(c) | excludes US/non-US/JOINT C/S/TS; FD&R optional on U | rolls up only if all portions are U; FD&R rules at §B.3 govern caveated U | combinable with higher class but U does not appear in higher-class portion mark; OK with AEA/FGI/dissem |

### 5.2 §H.3 JOINT Classification (pp 55–59)

| Marking | § | p | Banner Title | Banner Abbr | Portion | Sponsor / Basis | Relationships | Banner precedence | Commingling |
|---|---|---|---|---|---|---|---|---|---|
| JOINT (US co-owner) | H.3 | 56 | `//JOINT [class] [LIST]` | (none) | `//JOINT [class] [LIST]//REL TO [USA, LIST]` (or `//REL` if list matches) | Respective countries / EO 13526 §6.1(s)(2) | TS/S/C/U only (not RESTRICTED); requires `REL TO USA, LIST`; combinable with SCI **excluding HCS**, SAP, AEA, FGI, IC + non-IC dissem **excluding NOFORN** | JOINT marking at portion stays at portion; **does not roll up** to banner in US documents — banner becomes the highest US class with FGI [LIST] + REL TO union | JOINT portions must be segregated from US portions unless ICD 206 source-citation applies; if FGI inside, see §H.7 |

### 5.3 §H.4 SCI Control Systems (pp 60–98)

| Marking | § | p | Banner Title | Banner Abbr | Portion | Sponsor / Basis | Relationships | Banner precedence | Commingling |
|---|---|---|---|---|---|---|---|---|---|
| HCS (legacy) | H.4 | 62 | HCS | HCS | HCS | DNI / EO 13526 §4.3 | new content must use HCS-O / HCS-P; legacy HCS retained only on machine-to-machine carry | all unique SCI roll up | legacy HCS may combine; portion must include HCS-O / HCS-P / HCS-O-P |
| HCS-O (Operations) | H.4 | 64 | O | O | O | DNI / EO 13526 §4.3 | TS/S only; **requires ORCON + NOFORN**; not with ORCON-USGOV | all unique SCI roll up | combinable inc. HCS-P; HCS-O conveys in portion |
| HCS-P (Product) | H.4 | 66 | P | P | P | DNI / EO 13526 §4.3 | TS/S; **requires NOFORN**; ORCON or ORCON-USGOV permitted | all unique SCI roll up | combinable inc. HCS-O; HCS-P conveys in portion |
| HCS-P [SUB-COMPARTMENT] | H.4 | 68 | P [SUB] (≤6 alnum) | P [SUB] | P [SUB] | DNI / EO 13526 §4.3 | TS only; **requires HCS-P + ORCON + NOFORN**; not with ORCON-USGOV | all unique SCI roll up | combinable inc. HCS-O; HCS-P [SUB] conveys in portion |
| RESERVE | H.4 | 70 | RESERVE | RSV | RSV | DNI / DCI memo 10 Jan 2005 (NRO) | TS/S only; requires associated compartment | all unique SCI roll up | combinable; RSV-[COMP] conveys in portion |
| RSV-[COMPARTMENT] | H.4 | 72 | RESERVE-[COMP] (3 alnum) | RSV-[COMP] | RSV-[COMP] | DNI / DCI memo 10 Jan 2005 (NRO) | TS/S only; **requires RESERVE** | all unique SCI roll up | combinable; RSV-[COMP] conveys in portion |
| SI | H.4 | 74 | SI | SI | SI | DNI / NSA Title I §105(b)(1) | TS/S/C only | all unique SCI roll up | combinable; SI conveys in portion |
| SI-[COMPARTMENT] | H.4 | 76 | SI-[COMP] (2-3 alpha) | SI-[COMP] | SI-[COMP] | DNI / NSA Title I §105(b)(1) | **TS only**; **requires SI**; ECI grouping markings are NOT used in banner/portion | multi-compartment alphabetical, hyphen-separated | combinable; SI-[COMP] conveys in portion |
| SI-ECRU | H.4 | 78 | SI-ECRU | SI-EU | SI-EU | DNI / NSA Title I §105(b)(1) | TS only; requires SI + ECRU; ECI program retired into SI | all unique SCI roll up | combinable; SI-EU conveys in portion |
| SI-GAMMA | H.4 | 80 | GAMMA | G | G | DNI / NSA Title I §105(b)(1) | TS only; requires SI + ORCON; not with ORCON-USGOV | all unique SCI roll up | combinable (excl ORCON-USGOV); SI-G conveys in portion |
| SI-G [SUB-COMPARTMENT] | H.4 | 81 | G [SUB] (4 alpha) | G [SUB] | G [SUB] | DNI / NSA Title I §105(b)(1) | TS only; requires SI + G + ORCON; not with ORCON-USGOV | multi-G alphabetical, space-separated; all unique SCI roll up | combinable (excl ORCON-USGOV); SI-G [SUB] conveys in portion |
| SI-NONBOOK | H.4 | 83 | SI-NONBOOK | SI-NK | SI-NK | DNI / NSA Title I §105(b)(1) | TS only; requires SI + NONBOOK; ECI program retired into SI | all unique SCI roll up | combinable; SI-NK conveys in portion |
| TALENT KEYHOLE | H.4 | 85 | TALENT KEYHOLE | TK | TK | DNI / WH memo 26 Aug 1960 | TS/S only; may require RSEN for imagery | all unique SCI roll up | combinable; TK conveys in portion |
| TK-BLUEFISH | H.4 | 87 | BLUEFISH | BLFH | BLFH | DNI / TK Policy | **TS only**; requires TK + NOFORN; KDK legacy → TK-BLFH on new content | all unique SCI roll up | combinable with caveated info; TK-BLFH conveys in portion |
| TK-BLFH [SUB-COMPARTMENT] | H.4 | 89 | BLUEFISH [SUB] (≤6 alnum) | BLFH [SUB] | BLFH [SUB] | DNI / TK Policy | TS only; requires TK-BLFH + NOFORN; KDK legacy carries forward as TK-BLFH | all unique SCI roll up | combinable; TK-BLFH [SUB] conveys in portion |
| TK-IDITAROD | H.4 | 91 | IDITAROD | IDIT | IDIT | DNI / TK Policy | TS/S only; requires TK + NOFORN; KDK legacy → TK-IDIT on new content | all unique SCI roll up | combinable; TK-IDIT conveys in portion |
| TK-IDIT [SUB-COMPARTMENT] | H.4 | 93 | IDITAROD [SUB] (≤6 alnum) | IDIT [SUB] | IDIT [SUB] | DNI / TK Policy | TS/S only; requires TK-IDIT + NOFORN | all unique SCI roll up | combinable; TK-IDIT [SUB] conveys in portion |
| TK-KANDIK | H.4 | 95 | KANDIK | KAND | KAND | DNI / TK Policy | TS/S only; requires TK + NOFORN; KDK legacy → TK-KAND on new content | all unique SCI roll up | combinable; TK-KAND conveys in portion |
| TK-KAND [SUB-COMPARTMENT] | H.4 | 97 | KANDIK [SUB] (≤6 alnum) | KAND [SUB] | KAND [SUB] | DNI / TK Policy | TS/S only; requires TK-KAND + NOFORN | all unique SCI roll up | combinable; TK-KAND [SUB] conveys in portion |

**SCI grammar reminder.** Compartment is 2–3 alpha (SI), or 3 alnum
(RSV); sub-compartment is 4–6 alnum depending on system (4 for SI-G,
≤6 for HCS-P / TK sub-compartments). Multi-value separators per §A.6
+ §H.4 syntax (p61): `/` between control systems, `-` between control
and compartment, ` ` between sub-compartments. Numbered values sort
before alphabetic.

### 5.4 §H.5 Special Access Program (pp 99–102)

| Marking | § | p | Banner Title | Banner Abbr | Portion | Sponsor / Basis | Relationships | Banner precedence | Commingling |
|---|---|---|---|---|---|---|---|---|---|
| SAR | H.5 | 101 | `SPECIAL ACCESS REQUIRED-[program]` | `SAR-[program]` or `SAR-[program-abbr]` | `SAR-[program-abbr]` | DNI/DoD/DOE/DoS/DHS/AG / EO 13526 §4.3 | TS/S/C only; programs alphanumeric; no SAR-prefix repeat for multi-program | unique SAPs always roll up to banner | hierarchical depiction below program is optional and operational; portion only needs program |

**SAR grammar reminder** (§H.5 prose pp 99–100). Program identifier
is 2–3 char abbreviation. Hierarchy: program → compartment → sub-
compartment. Within a level, multi-value ascending sort (numeric
first). Multi-program separator is `/` (no SAR- repeat). Compartment
linker is `-`. Sub-compartment linker is space.

### 5.5 §H.6 Atomic Energy Act Information (pp 103–121)

| Marking | § | p | Banner Title | Banner Abbr | Portion | Sponsor / Basis | Relationships | Banner precedence | Commingling |
|---|---|---|---|---|---|---|---|---|---|
| RESTRICTED DATA | H.6 | 104 | RESTRICTED DATA | RD | RD | DOE / Atomic Energy Act §141–143 | TS/S/C only; **always with NOFORN** unless §123/§144 sharing agreement; CNWDI requires RD; SIGMA 14/15/18/20 only with TS/S RD | If any RD portion present, RD appears in banner; **RD evicts FRD and TFNI from the banner** when all three present (same eviction rule as portion, §H.6 p104); RD warning statement only | RD takes precedence over FRD/TFNI in portion; ideally RD into separate annex |
| CNWDI | H.6 | 106 | CRITICAL NUCLEAR WEAPON DESIGN INFORMATION | CNWDI | CNWDI | DoD / DoD 5210.2 | TS RD or S RD only; subset of RD per DoD-DOE guidance | CNWDI always rolls up to banner | CNWDI segregates from non-CNWDI portions; both RD warning + CNWDI identifying statement on first page (separate text boxes) |
| RD-SIGMA [#] | H.6 | 108 | SIGMA [#] | (none) | SG [#] | DOE / Atomic Energy Act §141–143 | TS/S RD only; requires RD; current SIGMAs: 14, 15, 18, 20 | unique SIGMAs roll up; **RD-SIGMA wins over FRD-SIGMA** in banner; all SIGMA numbers are listed under RD-SIGMA banner regardless of source | separate annex preferred; not commingled with REL TO portion unless equivalent positive release; RD-SIGMA wins over FRD-SIGMA in portion |
| FORMERLY RESTRICTED DATA | H.6 | 111 | FORMERLY RESTRICTED DATA | FRD | FRD | DOE + DoD / AEA §141–143 | TS/S/C only; always with NOFORN unless §123/§144 sharing | FRD appears in banner if no RD portion present; **fully evicted from banner if any RD portion present** (RD warning replaces FRD warning, §H.6 p104) | RD wins over FRD in portion; FRD into separate annex preferred |
| FRD-SIGMA [#] | H.6 | 113 | SIGMA [#] | (none) | SG [#] | DOE / AEA §141–143 | TS/S FRD only; requires FRD; SIGMAs 14/15/18/20 only | unique SIGMAs roll up; RD-SIGMA wins over FRD-SIGMA in banner | separate annex preferred; FRD-SIGMA must NOT commingle with REL TO unless equivalent positive release |
| DOD UCNI | H.6 | 116 | DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION | DOD UCNI | DCNI | DoD / AEA | **U only**; not on classified content; CUI re-evaluation candidate (14 Nov 2016) | rolls up only on U documents; on classified docs DOD UCNI does NOT appear in banner but NOFORN must be applied if FD&R less restrictive | with classified non-UCNI: DCNI portion mark NOT used (class adequately protects); apply NF if FD&R less restrictive |
| DOE UCNI | H.6 | 118 | DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION | DOE UCNI | UCNI | DOE / AEA §148 | U only; CUI re-evaluation candidate | rolls up on U docs; on classified docs DOE UCNI does NOT appear in banner; NOFORN must be applied if FD&R less restrictive | same as DOD UCNI: UCNI portion mark NOT used in classified portions; NF if needed |
| TFNI | H.6 | 120 | TRANSCLASSIFIED FOREIGN NUCLEAR INFORMATION | TFNI | TFNI | DOE + DNI / AEA §142e + 32CFR2001 §2001.24(i) | TS/S/C only | TFNI appears in banner if no RD/FRD portion present; **fully evicted from banner if any RD or FRD portion present** (§H.6 p104); special "Declassify On" annotation required regardless | RD or FRD takes precedence in portion; TFNI ideally not commingled |

### 5.6 §H.7 Foreign Government Information (pp 122–130)

| Marking | § | p | Banner Title | Banner Abbr | Portion | Sponsor / Basis | Relationships | Banner precedence | Commingling |
|---|---|---|---|---|---|---|---|---|---|
| FGI (acknowledged) | H.7 | 123 | `FOREIGN GOVERNMENT INFORMATION [LIST]` | `FGI [LIST]` | `[LIST] [non-US class]` or `NATO portion` (segregated) / `FGI [non-US class]` (commingled) | Respective country / EO 13526 §1.6(e) + §6.1(s) | TS/S/C/U/RESTRICTED + non-US designators per Appendix A | FGI [LIST] in banner unless concealed required; **mixing concealed + acknowledged → FGI without LIST** | ICD 206 docs may commingle FGI with US; non-ICD-206 must segregate; concealed and acknowledged must not mix in same portion |
| FGI (concealed) | H.7 | 123 | `FOREIGN GOVERNMENT INFORMATION` | `FGI` | `//FGI [non-US class]` (segregated) or `(class//FGI)` (commingled) | Respective country / EO 13526 | same as acknowledged but country list omitted; do NOT include trigraphs in portion | FGI without LIST in banner | as above |

**FGI grammar reminder** (§H.7 prose). LIST = trigraph(s) +
tetragraph(s) + NATO codes. Multi-country alphabetical, **single-
space separated** (NOT comma-separated, even though country lists in
REL TO are). FGI portion mark **always starts with `//`** because the
foreign source is its own classification authority.

### 5.7 §H.8 IC Dissemination Controls (pp 131–168)

Within-banner ordering inside this category follows the prose
sequence below.

| Marking | § | p | Banner Title | Banner Abbr | Portion | Sponsor / Basis | Relationships | Banner precedence | Commingling |
|---|---|---|---|---|---|---|---|---|---|
| RSEN | H.8 | 132 | RISK SENSITIVE | RSEN | RS | NGA / NSG | TS/S only; pairs with TK | always rolls up if any portion | combinable; RS conveys in portion |
| FOUO | H.8 | 134 | FOR OFFICIAL USE ONLY | FOUO | FOUO | Various / agency-specific (CUI re-evaluation candidate) | **U only**; portion-mark required when present | rolls up on U docs **only when no other dissem present** (other than FD&R); on classified docs FOUO does not appear in banner | combinable; FOUO in portion only when banner-precedence rule says so |
| ORCON | H.8 | 136 | ORIGINATOR CONTROLLED | ORCON | OC | DNI / NSA-1947 §103(c)(5) | TS/S/C only; **mutually exclusive with ORCON-USGOV**; OK with NOFORN/REL TO/DISPLAY ONLY; **not with RELIDO** | ORCON wins over ORCON-USGOV in banner | combinable inc. ORCON-USGOV (OC wins in portion); REL TO + ORCON = explicit positive release decision |
| ORCON-USGOV | H.8 | 139 | ORIGINATOR CONTROLLED-USGOV | ORCON-USGOV | OC-USGOV | DNI / NSA-1947 | TS/S/C only; **not with ORCON / RELIDO / SI-G / SI-G [SUB] / HCS-O / HCS-P [SUB]**; OK with HCS-P (no [SUB]); OK with NOFORN/REL TO/DISPLAY ONLY | ORCON wins over ORCON-USGOV in banner | OC wins in portion when both present |
| IMCON | H.8 | 142 | CONTROLLED IMAGERY | IMCON | IMC | DNI / NSA-1947 §103(c)(5) | TS/S only; OK with REL TO or NOFORN (NOFORN release requires SATP) | IMCON always rolls up; with NOFORN portion → `[class]//IMCON/NOFORN` | combinable; IMC conveys in portion |
| NOFORN | H.8 | 145 | NOT RELEASABLE TO FOREIGN NATIONALS | NOFORN | NF | DNI / NSA-1947 §103(c)(5) | TS/S/C/U; **mutually exclusive with REL TO / RELIDO / EYES ONLY / DISPLAY ONLY** | per Table 3 — NOFORN dominates other FD&R | NF conveys in portion |
| PROPIN | H.8 | 148 | CAUTION-PROPRIETARY INFORMATION INVOLVED | PROPIN | PR | DNI / 18 USC 1905 | TS/S/C/U | always rolls up; **PROPIN wins over FOUO in banner** | combinable; PR conveys in portion |
| REL TO | H.8 | 150 | `AUTHORIZED FOR RELEASE TO [USA, LIST]` | `REL TO [USA, LIST]` | `REL TO [USA, LIST]` (full) or `REL` (when same as banner) | DNI / NSA-1947 §103(c)(5) | TS/S/C/U; **not with NOFORN or EYES ONLY**; OK with RELIDO / DISPLAY ONLY; AEA-specific rules per §H.6 | per Table 3 (rules 21–23) | REL TO conveys only if **all** info in portion releasable to same LIST |
| RELIDO | H.8 | 154 | RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL | RELIDO | RELIDO | DNI / NSA-1947 §103(c)(5) | TS/S/C/U; OK alone or with REL TO; **not with NOFORN / DISPLAY ONLY** | per Table 3 (rule 17 has date-pivot ambiguity) | RELIDO in portion only when all combined info carries RELIDO decision |
| EYES ONLY | H.8 | 157 | `USA/[LIST] EYES ONLY` | (none) | EYES (or full form if portion's LIST ≠ banner's) | NSA / CSS Manual 1-52 | **NSA only, deprecated** — markings waiver expired 1 Oct 2017; TS/S/C; not with NOFORN/REL TO; OK with RELIDO; LIST = Five Eyes country trigraphs | per Table 3 | when extracting EYES portions into new docs, convert to REL TO |
| DEA SENSITIVE | H.8 | 159 | DEA SENSITIVE | (none) | DSEN | DEA / Planning & Inspection Manual ch.86 | TS/S/C/U; CUI re-evaluation candidate | always rolls up; **DSEN wins over FOUO** | DSEN wins over FOUO in portion; ICD-206 commingling with non-DSEN allowed |
| FISA | H.8 | 161 | FOREIGN INTELLIGENCE SURVEILLANCE ACT | FISA | FISA | DNI / 50 USC ch.36 | TS/S/C/U | always rolls up | combinable; FISA in portion |
| DISPLAY ONLY | H.8 | 163 | `DISPLAY ONLY [LIST]` | (none) | `DISPLAY ONLY [LIST]` | DNI / NSA-1947 | TS/S/C/U; **not with RELIDO or NOFORN**; OK with REL TO under DNI-authorized circumstances | per Table 3 (rules 25–27) | OK with REL TO if all info in portion approved through same originator FD&R channels |

### 5.8 §H.9 Non-IC Dissemination Controls (pp 169–191)

| Marking | § | p | Banner Title | Banner Abbr | Portion | Sponsor / Basis | Relationships | Banner precedence | Commingling |
|---|---|---|---|---|---|---|---|---|---|
| LIMDIS | H.9 | 170 | LIMITED DISTRIBUTION | LIMDIS | DS | NGA / 10 USC §455 | **U only**; CUI re-evaluation candidate | always rolls up on U docs; **LIMDIS wins over FOUO** in banner; on classified docs LIMDIS NOT in banner | NOT combinable with non-LIMDIS U / specific copyrighted / FOUO |
| EXDIS | H.9 | 172 | EXCLUSIVE DISTRIBUTION | EXDIS | XD | DoS / 5 FAH-2 §H-442.6 | TS/S/C/U; **not with NODIS**; **requires NOFORN** | NODIS wins over EXDIS in banner; EXDIS wins over SBU/SBU-NF/FOUO in U docs; REL TO not authorized in banner if any EXDIS portion | EXDIS supersedes SBU/SBU-NF/FOUO in portion; NODIS wins over EXDIS in portion |
| NODIS | H.9 | 174 | NO DISTRIBUTION | NODIS | ND | DoS / 5 FAH-2 §H-442.3 | TS/S/C/U; **not with EXDIS**; **requires NOFORN** | NODIS always rolls up if present; **NODIS wins over EXDIS** in banner; REL TO not authorized in banner if any NODIS portion; NODIS wins over SBU/SBU-NF/FOUO in U docs | NODIS supersedes SBU/SBU-NF/FOUO and EXDIS in portion |
| SBU | H.9 | 176 | SENSITIVE BUT UNCLASSIFIED | SBU | SBU | DoS / 12 FAM §540 | **U only**; CUI re-evaluation candidate | rolls up on U docs; **SBU wins over FOUO** in banner; on classified docs SBU NOT in banner | SBU wins over FOUO in portion; on classified portion the class adequately protects SBU (SBU not reflected) |
| SBU-NF | H.9 | 178 | SENSITIVE BUT UNCLASSIFIED NOFORN | SBU NOFORN | SBU-NF | DoS / 12 FAM §540 | **U only**; CUI re-evaluation candidate | SBU-NF wins over SBU and FOUO in banner | SBU-NF wins over FOUO in portion; commingled with other NOFORN portions → portion mark becomes `(U//NF//SBU)`; classified commingled portion uses class + NF |
| LES | H.9 | 181 | LAW ENFORCEMENT SENSITIVE | LES | LES | Various / agency-specific (CUI re-evaluation candidate) | TS/S/C/U; OK with REL TO USA, LIST if originator authorized | always rolls up regardless of class; **LES wins over FOUO** in banner | LES wins over FOUO in portion; commingled non-LES allowed under ICD 206 |
| LES-NF | H.9 | 185 | LAW ENFORCEMENT SENSITIVE NOFORN | LES NOFORN | LES-NF | Various / agency-specific (CUI re-evaluation candidate) | TS/S/C/U | LES marking always in banner; on classified docs `[class]//NOFORN//LES`; LES-NF wins over FOUO in U docs | LES-NF wins over FOUO in portion; commingled with other NOFORN → portion mark becomes `(S//NF//LES)` form |
| SSI | H.9 | 189 | SENSITIVE SECURITY INFORMATION | SSI | SSI | DHS / 49 USC 114 + 40119 (CUI re-evaluation candidate) | TS/S/C/U; OK with IC FD&R; REL TO / DISPLAY ONLY only with originator authorization | always rolls up regardless of class; **SSI wins over FOUO** in banner | SSI wins over FOUO in portion; ICD-206 commingling allowed |

> Authority: CAPCO-2016 §H.1–§H.9, pp 47–191. Verify each row
> against the manual before quoting elsewhere.

---

## 6. Loading order and update discipline

This file is **derivative**. The authoritative source is the
markdown / PDF in `crates/capco/docs/`. When CAPCO-2016 is
superseded by a later revision (a planned migration per Constitution
Principle VIII), every page reference, relationship, and rule above
must be re-verified against the new source — not silently
text-substituted. Update the section heading citation, then the
per-row citations, then the marque-encoding gap callouts in §3.4
and §4.3.

Per Constitution Principle IV, ODNI ISM schema package version is
pinned in `crates/ism/Cargo.toml [package.metadata.marque]
ism-schema-version`. The package version and the CAPCO-2016 manual
version are tracked separately because ODNI ships them on different
cadences.
