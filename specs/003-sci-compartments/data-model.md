# SCI Data Model

## Types (crate: `marque-ism`, module: `attrs`)

```rust
/// A fully-parsed SCI category block entry. A banner or portion may
/// carry multiple `SciMarking` entries separated by `/` within one
/// SCI category block.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SciMarking {
    /// The control-system anchor. One of seven published bare systems
    /// or a structurally-parsed custom value.
    pub system: SciControlSystem,

    /// Compartments in source order. Sort-order validation is rule
    /// E033 (not parse-time).
    pub compartments: Box<[SciCompartment]>,

    /// If the `{system}-{first_compartment}` composite exactly matches
    /// an ODNI CVE value (e.g., `SI-G`, `HCS-P`, `TK-BLFH`), this
    /// records the enum variant. Only populated when the matching
    /// compartment has NO sub-compartments (subs imply the compound
    /// is a structural anchor, not a CVE atom). `None` otherwise.
    pub canonical_enum: Option<SciControl>,
}

/// Which kind of SCI control system this marking anchors on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SciControlSystem {
    /// One of the seven published bare control systems.
    Published(SciControlBare),
    /// An agency-allocated system matching `[A-Z0-9]{2,5}` (per the
    /// CAPCO-2016 §A.6 p15 `123` example). Stores the raw text as it
    /// appeared in the source.
    Custom(Box<str>),
}

/// The seven published bare SCI control systems from
/// `CVEnumISMSCIControls.xml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SciControlBare {
    Bur,
    Hcs,
    Klm,
    Mvl,
    Rsv,
    Si,
    Tk,
}

/// A single compartment under an SCI control system.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SciCompartment {
    /// Compartment identifier (alphanumeric).
    pub identifier: Box<str>,
    /// Sub-compartments in source order.
    pub sub_compartments: Box<[Box<str>]>,
}
```

## Field addition on `IsmAttributes`

```rust
// Existing — kept unchanged for back-compat.
pub sci_controls: Box<[SciControl]>,

// New — authoritative structural view.
pub sci_markings: Box<[SciMarking]>,
```

Both fields are populated by the parser. `sci_controls` remains the enum projection; `sci_markings` is the structural view. Rules may consume whichever is more convenient for their logic.

## Invariants

1. For each `SciMarking` with `canonical_enum: Some(ev)`, `sci_controls` contains `ev` at the corresponding position. If more than one marking's composite hits the same CVE value (pathological but representable), `sci_controls` contains duplicates consistent with `sci_markings` ordering.
2. `SciControlSystem::Custom(..)` is never populated with a value that matches one of the seven `SciControlBare` variants — the parser dispatches to `Published` first.
3. `SciControlBare` is decoupled from `SciControl` (which is CVE-derived and may grow variants over time). Build-time assert verifies the CVE contains all seven.
4. `#[non_exhaustive]` on `SciMarking` and `SciCompartment` blocks struct-literal construction from `marque-core` — `::new()` constructors are provided for the parser to use.

## Token kinds added

```rust
TokenKind::SciSystem,         // "SI", "123"
TokenKind::SciCompartment,    // "G" in "SI-G" — reused or added
TokenKind::SciSubCompartment, // "ABCD" in "SI-G ABCD"
```

`TokenKind::SciControl` remains for the existing CVE exact-match path.

## `SciControl` (CVE enum) behavior

Unchanged. `build.rs` continues to generate the enum from `CVEnumISMSCIControls.xml`. Parser callers still invoke `SciControl::parse(composite)` after structural parsing to populate `canonical_enum`.

## `PageContext` addition

```rust
impl PageContext {
    pub fn expected_sci_markings(&self) -> Box<[SciMarking]>;
}
```

Merges SCI markings across portions: union by `SciControlSystem` (deep equal on the enum + custom text), then per-system union of compartments by `identifier`, then per-compartment union of sub-compartments. Sort order follows §A.6 ascending (numeric first, alpha after).

`render_expected_banner()` uses `expected_sci_markings()` when it returns non-empty. When a marking has no compartments, renders as the bare system (`SI`, `HCS`, etc., or the `Custom` text). When compartments are present, renders `CTRL-COMP1 SUB SUB-COMP2 SUB` per §A.6 Figure 2 format.

## Serialization

NDJSON diagnostic/audit records refer to SCI tokens via existing `TokenSpan` plumbing:

```json
{"kind":"SciSystem","start":8,"end":10,"text":"SI"}
{"kind":"SciCompartment","start":11,"end":12,"text":"G"}
{"kind":"SciSubCompartment","start":13,"end":17,"text":"ABCD"}
{"kind":"SciSubCompartment","start":18,"end":22,"text":"DEFG"}
```

Existing `SciControl`-tagged tokens are still emitted for exact CVE matches alongside the structural tokens. Downstream consumers can read either form.

## Non-breaking migration

- Existing code that reads `IsmAttributes.sci_controls` continues to work unchanged.
- Existing rules (E010, E011) keep consuming the enum; their logic is audited for any latent dependency on the old "one control = one block" assumption and updated if found.
- Existing tests continue to pass without modification.
- This is an **additive** change. Minor-version bump (0.3.0 → 0.4.0 if shipped standalone; can share the SAR branch's 0.4.0 bump if they merge close together).
