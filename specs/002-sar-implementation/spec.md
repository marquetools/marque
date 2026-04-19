<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# SAR (Special Access Required) Implementation

**Status**: Draft
**Branch**: `feat/sar-implementation`
**Authority**: CAPCO Register and Manual (31 December 2016), §H.5 (pp 99–102) and §A.6 (pp 15–17)

## Problem

`marque-ism` exposes a `SarIdentifier` enum generated from ODNI's `CVEnumISMSAR.xml`, which ships empty in the public ISM-v2022-DEC package. The parser calls `SarIdentifier::parse(..)` which always returns `None`, so **no SAR token is ever recognized** at runtime. `IsmAttributes.sar_identifiers` is a field that is never populated. Zero CAPCO rules validate SAR syntax, ordering, classification constraints, or banner roll-up. The only SAR awareness in the engine is block-ordering ordinal 2 in `E003 misordered-blocks`, which is unreachable because the parser never emits `TokenKind::SarIdentifier`.

## Why an enum cannot model SAR

SAR program identifiers are agency-assigned codewords (`BP`, `CD`, `XR`, or spelled-out forms like `BUTTER POPCORN`), not a centrally registered vocabulary. `CVEnumISMSAR.xml` is intentionally empty because the codewords are not publicly enumerable. Every other CAPCO category we validate (`SciControl`, `DissemControl`, `DeclassExemption`) has a finite ODNI-published enumeration. SAR does not, so the code-generation pipeline is the wrong tool for this category. SAR identifiers must be **structurally** validated (shape, ordering, classification constraint, roll-up) rather than **membership** validated (is this identifier in a known set).

## Scope

This spec covers the full Register-§H.5 behavior for banner and portion markings:

1. Recognize `//SAR-` and `//SPECIAL ACCESS REQUIRED-` category indicators.
2. Parse program identifiers, compartments, and sub-compartments per the §A.6 syntax grammar.
3. Validate five syntax and constraint rules (E026–E030) plus one page-level roll-up rule (E031).
4. Render correct SAR banner strings from portion roll-up via `PageContext`.
5. Wire SAR into the corpus accuracy harness (SC-002 / SC-003 ≥95% per rule).

Out of scope for this phase:

- SAR–FGI or SAR–NATO commingling rules.
- Non-IC SAR sourcing (derivative-use NOFORN rule under §H.5 footnote).
- SAP-Compartmented / Waived SAP subcategories — the register calls these out (p100, "compartmented effort" indicator) but the MVP only needs the base `SAR-` form.

## Requirements

### R1 — Category indicator

The parser MUST recognize a category block opening with either:

- `SAR-` (banner abbreviation, portion required)
- `SPECIAL ACCESS REQUIRED-` (banner full form)

The trailing hyphen is part of the indicator, not a separator (§H.5 p100 Syntax Rules, bullet 3).

### R2 — Grammar

Within a SAR category block:

```
SAR_BLOCK    := INDICATOR PROGRAM ("/" PROGRAM)*
INDICATOR    := "SAR-" | "SPECIAL ACCESS REQUIRED-"
PROGRAM      := PROG_ID ( "-" COMPARTMENT )?
COMPARTMENT  := COMP_ID (" " SUB_COMP)*
PROG_ID      := [A-Z0-9]{2,3}   (SAR- form)
              | [A-Z ]+          (SPECIAL ACCESS REQUIRED- form; nickname)
COMP_ID      := [A-Z0-9]+
SUB_COMP     := [A-Z0-9]+
```

**Separators** (§A.6 Figure 2):

| Separator | Role |
|-----------|------|
| `/` | program/program boundary within the block |
| `-` | links compartment to its program identifier |
| (space) | separates multi-value compartments AND sub-compartments |

The SAR indicator MUST NOT be repeated when multiple programs appear (§H.5 p100 Syntax Rules, bullet 5).

### R3 — Classification constraint

SAR markings MAY only appear with classifications TOP SECRET, SECRET, or CONFIDENTIAL (§H.5 p101 "Relationship(s) to Other Markings"). `UNCLASSIFIED//SAR-…` is invalid.

### R4 — Ordering

Within each hierarchical level (programs, compartments, sub-compartments), values MUST be listed in ascending sort order with numbered values first, followed by alphabetic values (§H.5 p99 and §A.6 p16 SAP bullet 5).

### R5 — Banner vs portion form

Portion marks MUST use `SAR-` (abbreviation) only. Banner lines MAY use either form. (§H.5 p101 "Authorized Portion Mark".)

### R6 — Banner roll-up

Every unique SAR program identifier appearing in any portion marking on a page MUST also appear in the banner line's SAR block (§H.5 p101 "Precedence Rules for Banner Line Guidance"). Compartments and sub-compartments inherit the same roll-up semantics.

## Data Model

Replace the enum-typed field on `IsmAttributes`:

```rust
// before
pub sar_identifiers: Box<[SarIdentifier]>,

// after
pub sar_markings: Option<SarMarking>,
```

New types in `marque-ism::attrs`:

```rust
pub struct SarMarking {
    pub indicator: SarIndicator,
    pub programs: Box<[SarProgram]>,
}

pub enum SarIndicator { Abbrev, Full }

pub struct SarProgram {
    pub identifier: Box<str>,
    pub compartments: Box<[SarCompartment]>,
}

pub struct SarCompartment {
    pub identifier: Box<str>,
    pub sub_compartments: Box<[Box<str>]>,
}
```

The `SarIdentifier` enum is removed from `build.rs`. The placeholder `TokenKind::SarIdentifier` stays for back-compat with existing span metadata but gains companions: `TokenKind::SarIndicator`, `TokenKind::SarProgram`, `TokenKind::SarCompartment`, `TokenKind::SarSubCompartment`.

## Rules (E026–E031)

| ID | Name | Section | Summary |
|----|------|---------|---------|
| E026 | `sar-portion-form` | §H.5 | Portion must use `SAR-` abbrev, not `SPECIAL ACCESS REQUIRED-`. Fix: abbreviate. |
| E027 | `sar-classification` | §H.5 | SAR requires TS/S/C. Fire on `U//SAR-*`. Fix: none (needs human review). |
| E028 | `sar-program-order` | §H.5 | Programs must be ascending (numeric first, alpha after). Fix: reorder. |
| E029 | `sar-compartment-order` | §H.5 | Compartments within a program ascending; sub-compartments within a compartment ascending. Fix: reorder. |
| E030 | `sar-indicator-repeat` | §H.5 | `//SAR-BP//SAR-CD//` invalid; coalesce to `//SAR-BP/CD//`. Fix: coalesce. |
| E031 | `sar-banner-rollup` | §H.5 | Banner SAR block must contain every program/compartment/sub-compartment present in preceding portions. Fix: synthesize full SAR block from `PageContext`. |

All rules cite `CAPCO-2016 §H.5` (or `§A.6` for the two syntax rules where the formatting section is a better match).

## Non-requirements

- We do not attempt to validate whether a given program identifier is a real authorized program. That requires agency-specific knowledge unavailable to the engine.
- We do not emit a warning on single-use compartments below the sub-compartment level (§H.5 p100 explicitly forbids depicting hierarchy below that level).

## Success Criteria

- SC-SAR-1: Banner form `TOP SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN` parses into a `SarMarking` with three programs, nested compartments, zero diagnostics.
- SC-SAR-2: Every E026–E031 rule hits ≥95% accuracy on its corpus fixtures (parity with existing SC-002/SC-003 gates for E001–E025).
- SC-SAR-3: `PageContext::render_expected_banner()` emits the canonical §H.5 Table 7 example verbatim when fed the corresponding portions.
- SC-SAR-4: WASM parity (SC-008): browser build produces byte-identical NDJSON for SAR fixtures.
