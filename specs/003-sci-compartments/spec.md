# SCI Compartments and Sub-Compartments

**Status**: Draft
**Branch**: `feat/sci-compartments`
**Authority**: CAPCO Register and Manual (31 December 2016), §A.6 (pp 15–17) and §H.4 (pp 60–98)
**Related**: `feat/sar-implementation` (#18) introduced the same structural approach for SAR; SCI shares the pattern but must coexist with a partially-populated ODNI CVE.

## Problem

`marque-core`'s parser recognizes only exact matches of the ODNI `CVEnumISMSCIControls.xml` values (`SciControl::parse(trimmed)`). That CVE enumerates 17 values: 7 bare control systems (`BUR, HCS, KLM, MVL, RSV, SI, TK`) and 10 pre-registered compound forms (`HCS-O, HCS-P, HCS-X, KLM-R, SI-EU, SI-G, SI-NK, TK-BLFH, TK-IDIT, TK-KAND`).

Probe against CAPCO-2016 §A.6 p15 canonical example:

| Input | Expected | Actual |
|-------|----------|--------|
| `SI` | `SciControl::Si` | ✅ parses |
| `SI-G` | `SciControl::SiG` (published compound) | ✅ parses |
| `SI-G ABCD` | SI-G with sub-compartment `ABCD` | ❌ Unknown |
| `HCS-P SOMECOMP` | HCS-P with sub-compartment `SOMECOMP` | ❌ Unknown |
| `123/SI-G ABCD DEFG-MMM AACD` (manual's literal example) | `123` (bare SCI system) and `SI` with two compartments, G with subs ABCD/DEFG and MMM with sub AACD | ❌❌ both whole blocks Unknown |

The parser has no grammar for the CAPCO §A.6 p15 specification:

> TOP SECRET//123/SI-G ABCD DEFG-MMM AACD//ORCON/NOFORN — where 123 and SI are SCI control systems, G and MMM are SI compartments, ABCD and DEFG are sub-compartments of G, and AACD is a sub-compartment of MMM.

Every SCI marking with sub-compartments or unpublished compartments lands in the `Unknown` bucket and fires E008, obscuring whatever structural issues actually exist.

## Why the CVE approach is incomplete (but not wrong)

Unlike SAR (where the CVE is entirely empty because codewords are agency-assigned), SCI's CVE is *partially* populated. Published compound values like `SI-G` and `HCS-P` must continue to round-trip identically. The fix is a **hybrid** model: recognize bare control systems as structural anchors, parse compartment/sub-compartment syntax structurally, and where a composite happens to match an enumerated CVE value use that value as the canonical identity.

## Scope

This spec covers full §A.6 SCI grammar support for banner and portion markings:

1. Recognize the 7 bare SCI control systems as structural anchors.
2. Parse `CONTROL (-COMP (SPACE SUB)*)* ` grammar per §A.6 p15 and §A.6 Figure 2.
3. Preserve pre-registered compound recognition (`SI-G`, `HCS-P`, etc.) for rules that need it.
4. Support unpublished (agency-allocated) control systems alphanumerically (the manual's `123` example).
5. Update `E010 bare-hcs` and `E011 missing-non-us-prefix` to consume the new structural type.
6. Page-level roll-up in `PageContext` for SCI parallel to the SAR one.
7. New rules where the grammar introduces new failure modes (see §Rules).

**Out of scope:**

- SCI Non-US prefix handling for compartments (beyond what E011 already does on bare controls). Non-US SCI retains an FGI-like semantic that the current data model handles at the control-system level; extending it to compartments is a separate future concern.
- Full validation of compartment-to-system relationships (whether a given compartment is legal under a given system). ODNI does not publish this mapping in a machine-readable form; out of scope.
- Fully classified (non-published) compound forms beyond what the grammar permits structurally.

## Requirements

### R1 — Control system recognition

The parser MUST recognize the bare control systems from the CVE as structural anchors for the grammar: `BUR, HCS, KLM, MVL, RSV, SI, TK`.

Additionally, the parser MUST accept unpublished control systems that match the alphanumeric shape `[A-Z0-9]{2,5}` (per the manual's `123` example on p15) — these are recognized structurally but carry a separate `CustomControl` tag so rules can warn if needed.

### R2 — Grammar

```
SCI_BLOCK        := SCI_SYSTEM ("/" SCI_SYSTEM)*
SCI_SYSTEM       := CONTROL (-COMPARTMENT)*
CONTROL          := BARE_CONTROL | CUSTOM_CONTROL
BARE_CONTROL     := "BUR" | "HCS" | "KLM" | "MVL" | "RSV" | "SI" | "TK"
CUSTOM_CONTROL   := [A-Z0-9]{2,5}    (not matching a BARE_CONTROL)
COMPARTMENT      := COMP_ID (SPACE SUB_COMP)*
COMP_ID          := [A-Z0-9]+
SUB_COMP         := [A-Z0-9]+
```

Multiple control systems in one SCI block are `/`-separated (per §A.6: "Multiple SCI control systems must be separated by a single forward slash").

### R3 — Sort order

Within each hierarchical level (control systems, compartments, sub-compartments), values MUST be listed in ascending order with numbered values first, followed by alphabetic values (§A.6 p15). Out-of-order is a rule violation (E032, see below), not a parser error.

### R4 — Pre-registered compound preservation

When the full composite matches an enumerated CVE value (e.g., `SI-G`, `HCS-P`, `TK-BLFH`), the parser MUST record it as both (a) the structural decomposition AND (b) the enum variant for back-compat with existing rules (E010 `bare-hcs`, E011 `missing-non-us-prefix`). Rules inspecting the canonical enum continue to work; rules inspecting compartment structure gain new reach.

### R5 — Banner/portion parity

SCI grammar is identical in banner and portion markings (unlike SAR, where `SAR-` must be used in portions). No separate portion-form rule is needed.

### R6 — Banner roll-up

Every unique compartment and sub-compartment appearing in any portion marking on a page MUST also appear in the banner line's SCI block for that control system (§A.6 p15 implies this via the aggregate example; explicitly confirmed by §D.2 Banner Line Roll-Up Rules).

## Data Model

Replace the flat vector with a structural type. Two-field hybrid keeps existing rules working:

```rust
// Before
pub struct IsmAttributes {
    pub sci_controls: Box<[SciControl]>,
    // ...
}

// After
pub struct IsmAttributes {
    pub sci_markings: Box<[SciMarking]>,
    // Convenience view maintained for back-compat: the ENUM variant
    // projection for each marking whose composite matches a CVE value.
    // Empty when all markings are structurally-parsed customs/subs.
    pub sci_controls: Box<[SciControl]>,
    // ...
}
```

New types in `marque-ism::attrs`:

```rust
pub struct SciMarking {
    /// Control-system anchor. Custom controls use `SciControlSystem::Custom(Box<str>)`.
    pub system: SciControlSystem,
    /// Compartments in source order. Sort-order validation is E032, not parse-time.
    pub compartments: Box<[SciCompartment]>,
    /// If the full composite matched a CVE value, the enum variant is preserved
    /// here. Populated for `SI-G`, `HCS-P`, etc.; `None` for structural-only forms.
    pub canonical_enum: Option<SciControl>,
}

pub enum SciControlSystem {
    /// One of the 7 bare published control systems (BUR/HCS/KLM/MVL/RSV/SI/TK).
    Published(SciControlBare),
    /// Agency-allocated unpublished system matching `[A-Z0-9]{2,5}`. Parser
    /// stores the raw text.
    Custom(Box<str>),
}

pub enum SciControlBare { Bur, Hcs, Klm, Mvl, Rsv, Si, Tk }

pub struct SciCompartment {
    pub identifier: Box<str>,
    pub sub_compartments: Box<[Box<str>]>,
}
```

`SciControl` (the existing enum) stays as the CVE-derived vocabulary. `SciControlBare` is added alongside it as a structural subset. The parser populates BOTH `sci_markings` (structural, always) and `sci_controls` (enum projection, when composite matches).

Token kinds added (parallel to SAR):

```rust
TokenKind::SciSystem,          // "SI", "HCS", "123"
TokenKind::SciCompartment,     // "G" in "SI-G"; stays as existing behavior for enumerated compounds
TokenKind::SciSubCompartment,  // "ABCD" in "SI-G ABCD"
```

## Rules

| ID | Name | Section | Summary |
|----|------|---------|---------|
| E010 (existing) | `bare-hcs` | §H.4 | Bare `HCS` requires `-O`/`-P`/`-X`. Logic unchanged; consume either `sci_controls` (enum path) or `sci_markings` (structural path — flag when `system == Hcs && compartments.is_empty()`). |
| E011 (existing) | `missing-non-us-prefix` | §H.4 | No behavior change; continues to read the enum projection. |
| E032 (new) | `sci-system-order` | §A.6 p15 | Control systems within one block must be ascending (numeric first, then alpha). Fix: reorder. Confidence 0.85. |
| E033 (new) | `sci-compartment-order` | §A.6 p15 | Compartments within a system ascending; sub-compartments within a compartment ascending. Fix: reorder. Confidence 0.85. |
| E034 (new) | `sci-custom-control-warning` | §A.6 p15 | Warns when a CustomControl shape appears (e.g., `123`). Not an error — unpublished controls are legal — but warrants human review that it matches the agency's allocation. Severity: Warn. No fix. |
| E035 (new) | `sci-banner-rollup` | §D.2 + §A.6 | Banner SCI block must contain every compartment/sub-compartment present in preceding portions for each control system. Mirror of SAR's E031. Fix: replace banner SCI block with rolled-up form. Confidence 0.9. |

E008 (`unrecognized-token`) gets a skip filter extension: Unknown tokens whose structural form matches the SCI grammar are handled by the new parser path, not E008.

## Non-requirements

- No rule validates whether a given `COMP_ID` is legal under a given `CONTROL`. ODNI does not publish that mapping.
- No rule validates that unpublished systems (CustomControl) are registered with the correct agency — out-of-band concern.
- No attempt to reconstruct pre-registered compound enum variants from structurally-parsed input: if the user writes `SI-G`, we record the enum *and* the structural decomposition; if they write `SI-UNPUBLISHED`, we record only the structure.

## Migration impact on existing code

- `IsmAttributes.sci_controls` keeps its current shape and semantics (back-compat). Rules reading it continue to work unchanged.
- `IsmAttributes.sci_markings` is new and authoritative for structural access.
- Existing SCI corpus fixtures continue to pass unchanged (`SECRET//SI//NOFORN`, `TOP SECRET//HCS-P//NOFORN`, `(TS//SI/TK//NF)`, etc.).
- `marque-ism` minor version bump (0.3.0 → 0.4.0 after SAR; or coordinate so SCI lands in the same 0.4.0 bump if both merge close together).

## Success criteria

- SC-SCI-1: The CAPCO-2016 §A.6 p15 canonical example `TOP SECRET//123/SI-G ABCD DEFG-MMM AACD//ORCON/NOFORN` parses with:
  - 2 `SciMarking` entries (`123` custom, `SI` published with compartments G [ABCD, DEFG] and MMM [AACD])
  - `sci_controls` containing `SiG` (CVE match preserved)
  - Zero `Unknown` tokens in the SCI block
  - Zero diagnostics on a valid fixture
- SC-SCI-2: All existing SCI tests continue to pass unchanged.
- SC-SCI-3: Each new rule (E032–E035) hits ≥95% accuracy on its corpus fixtures.
- SC-SCI-4: `PageContext::expected_sci_markings()` rolls compartments/sub-compartments across portions correctly.
- SC-SCI-5: WASM parity (SC-008): browser build produces byte-identical NDJSON for new SCI fixtures.
