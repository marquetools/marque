---
name: capco-validate
description: Validate CAPCO/ISM rule citations, token predicates, and marking logic against authoritative sources before committing rules to marque. Use when adding new rules (E###/W###/C###), auditing token definitions, migrating rule logic, or verifying citations.
---

# CAPCO Validate

Orchestration guide for the 14 CAPCO/ISM validator agents in the `capco-validators` plugin. Each agent is a focused domain expert — this skill tells you which to invoke and when.

## When to Use This Skill

Invoke a validator whenever you:
- **Add a new rule** (E###, W###, C###) — validate the citation before opening a PR
- **Audit a token definition** — verify a CVE value against the ISM XML source
- **Migrate rule logic** — confirm equivalence between old and new implementations
- **Check a suspicious citation** — verify it's real and accurately reflects the source
- **Investigate a marking parse failure** — confirm what the source actually says

**You cannot skip validation and maintain citation integrity.** If a validator flags an issue, the rule does not ship.

## The 14 Validator Agents

### Invoke these for category-specific questions:

| Agent | Domain | CAPCO § | Use when... |
|-------|--------|---------|-------------|
| `capco-foundational` | Portion marks, banners, general syntax | §A–D | Syntax rules, marking requirements, general structure |
| `capco-sci-validator` | SCI controls, compartments, sub-compartments | §H.4, §A.6 | Anything involving HCS, SI, TK, SCI ordering |
| `capco-sar-validator` | SAR/SAP markings | §H.5 | SAP program identifiers, compartment hierarchy |
| `capco-dissem-validator` | Dissemination controls | §H.8–9 | REL TO, NOFORN, FOUO, FISA, RELIDO, OC |
| `capco-aea-validator` | Atomic Energy Act markings | §H.6 | RD, FRD, CNWDI, SIGMA, DCNI, UCNI, TFNI |
| `capco-fgi-validator` | Foreign Government Information | §H.7 | FGI trigraphs, tetragraphs, NATO/coalition codes |
| `capco-nato-validator` | JOINT/NATO markings | §H.3 | JOINT classification, NATO caveats, allied markings |
| `capco-classification-validator` | US classification levels | §H.1–2 | TOP SECRET, SECRET, CONFIDENTIAL, UNCLASSIFIED |
| `capco-declassification-validator` | CAB declassification guidance | §E | Declassify-on dates, 25X/50X exemptions, hierarchies |
| `capco-cui-validator` | CUI BASIC/SPECIFIED | §F | CUI category application, FOUO legacy status |
| `capco-legacy-validator` | Deprecated markings | §F | Obsolete marking detection, migration recommendations |

### Invoke these for cross-cutting questions:

| Agent | Domain | Use when... |
|-------|--------|-------------|
| `capco-declassification-markings-specialist` | Historical banner declassification syntax + modern CAB mappings | Converting old banner-line declassification rows to modern CAB format |
| `capco-cab-specialist` | CAB structure, authority chains, multi-source resolution | CAB derivation statements, authority precedence, commingling rules |
| `capco-deprecated-historical-specialist` | Legacy marking context, FOUO→CUI mappings, commingling rules | Why a marking was deprecated, what the modern equivalent is |

## Validation Workflows

### 1. Adding a new rule (E###)

```
Delegate to the appropriate category validator with:
- Rule ID and name
- CAPCO citation you intend to use (§X.Y page Z)
- What the rule checks for
- Test cases (should-pass and should-fail examples)

Expected response: PASS with citation verification, or FAIL with issues + recommended fixes.
```

### 2. Auditing a token definition

```
Delegate to the appropriate category validator with:
- Token name (e.g., DissemControl::NF)
- CVE source file (e.g., CVEnumISMDissem)
- Expected value string (e.g., "NF")
- Where it's used in the codebase

Expected response: confirms the value matches ISM-v2022-DEC, or flags a mismatch.
```

### 3. Migrating rule logic

```
Delegate to the appropriate category validator with:
- Original rule logic (brief description)
- New implementation
- CAPCO citation for the behavior
- Whether the logic is equivalent

Expected response: confirms equivalence, or identifies where they diverge.
```

### 4. Verifying a citation

```
Delegate to the appropriate category validator with:
- The citation as written (e.g., "CAPCO-2016 §H.4 p15")
- The claim it supports
- The rule or code that uses it

Expected response: confirms the citation is real and accurately reflects the source, or flags fabrication/drift.
```

## Authority & Citation Discipline

Every validator:
- Has the authoritative CAPCO section(s) embedded in its system prompt
- Has ISM CVE enumeration data included
- Will cite §X.Y page Z for every claim
- Will flag fabricated, drifted, or misattributed citations
- Will catch predicates that don't match the source

**You cannot ask a validator to approve something without source grounding.** If no citation exists in CAPCO-2016 or the ISM XML, the validator will say so.

## What Validators Are Not

Validators are single-purpose fact-checkers. They are **not**:
- Advisors on system architecture or design
- Writers of new rules (they validate existing ones)
- Documenters of the marque codebase

For those tasks, use the standard marque development process.

## Quick Selection Guide

```
Question involves SCI marking, compartments, sub-compartments?
  → capco-sci-validator

Question involves SAR/SAP programs or hierarchy?
  → capco-sar-validator

Question involves dissem controls (REL TO, NOFORN, FOUO, FISA)?
  → capco-dissem-validator

Question involves AEA (RD, FRD, SIGMA, nuclear markings)?
  → capco-aea-validator

Question involves NATO, JOINT, or allied markings?
  → capco-nato-validator

Question involves CAB structure, authority chains, or derivation?
  → capco-cab-specialist

Question involves historical banner declassification syntax or historical→modern mapping?
  → capco-declassification-markings-specialist

Question involves why a marking was deprecated or FOUO→CUI mapping?
  → capco-deprecated-historical-specialist

Question involves CUI BASIC or CUI SPECIFIED categories?
  → capco-cui-validator

Question involves US classification levels (TS/S/C/U)?
  → capco-classification-validator

Question involves declassify-on dates, 25X exemptions, or CAB declassification?
  → capco-declassification-validator

Question involves FGI country/org codes?
  → capco-fgi-validator

Question involves deprecated/obsolete markings?
  → capco-legacy-validator

Question involves portion mark syntax, banner format, general marking rules?
  → capco-foundational
```

## Authority

CAPCO-2016 (Intelligence Community Markings System Register and Manual)  
ISM-v2022-DEC (ODNI Information Security Marking schema package)
