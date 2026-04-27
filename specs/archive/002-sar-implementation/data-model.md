<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# SAR Data Model

## Types (crate: `marque-ism`, module: `attrs`)

```rust
/// Complete SAR category block parsed from a marking.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SarMarking {
    /// The form of SAR indicator used in the source marking.
    pub indicator: SarIndicator,
    /// Programs in the order they appeared. Sort-order validation is
    /// performed by rule E028, not at parse time.
    pub programs: Box<[SarProgram]>,
}

/// Which SAR indicator form a marking uses. Banner lines may use either;
/// portion marks may only use `Abbrev` (rule E026 enforces this).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SarIndicator {
    /// `SAR-` (portion and banner).
    Abbrev,
    /// `SPECIAL ACCESS REQUIRED-` (banner only).
    Full,
}

/// A single Special Access Program with optional compartments.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SarProgram {
    /// Program identifier — 2–3 alphanumeric chars for the abbreviated
    /// form, or a spelled-out nickname for the full form. Stored as the
    /// raw text as it appeared in the source.
    pub identifier: Box<str>,
    /// Compartments in source order. May be empty.
    pub compartments: Box<[SarCompartment]>,
}

/// A compartment within a SAR program, optionally carrying
/// sub-compartments.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SarCompartment {
    /// Compartment identifier (alphanumeric).
    pub identifier: Box<str>,
    /// Sub-compartments in source order. May be empty.
    pub sub_compartments: Box<[Box<str>]>,
}
```

## Field change on `IsmAttributes`

```rust
// before
pub sar_identifiers: Box<[SarIdentifier]>,

// after
pub sar_markings: Option<SarMarking>,
```

Only one SAR block is permitted per marking per §A.6; `Option` (not `Vec`) is the correct cardinality. If a marking has two `//SAR-…//` blocks, the first is parsed as the canonical `SarMarking` and the second produces a `TokenKind::Unknown` block plus an E030 diagnostic.

## Token kinds added

```rust
TokenKind::SarIndicator,      // "SAR-" | "SPECIAL ACCESS REQUIRED-"
TokenKind::SarProgram,        // "BP" | "BUTTER POPCORN"
TokenKind::SarCompartment,    // "J12"
TokenKind::SarSubCompartment, // "J54"
```

`TokenKind::SarIdentifier` stays for one release cycle behind a `#[deprecated]` attribute; the parser stops emitting it. Downstream code (the `E003 misordered-blocks` ordinal mapping) switches to `TokenKind::SarIndicator` as the block anchor.

## Removed

- `SarIdentifier` enum (generated from empty `CVEnumISMSAR.xml`) — deleted from `build.rs` output and from `crate::generated::values`.
- The `SarIdentifier` re-export from `attrs.rs`.

## `PageContext` additions

```rust
impl PageContext {
    pub fn expected_sar_marking(&self) -> Option<SarMarking>;
}
```

Implementation merges all `portions: Vec<&IsmAttributes>` entries whose `sar_markings` is `Some`, unioning programs by identifier, compartments by identifier, and sub-compartments by identifier. Sort order for the rendered banner is the §H.5 canonical ordering (numeric first, then alpha, ascending). `render_expected_banner()` emits the SAR block between the SCI block and the AEA block, using `SAR-` (abbreviation) form regardless of input indicator, per the roll-up convention that the banner MUST contain every program but is permitted to use either form.

## Serialization

NDJSON output (diagnostic / audit records) refers to SAR tokens via their existing `TokenSpan` shape:

```json
{"kind":"SarIndicator","start":10,"end":14,"text":"SAR-"}
{"kind":"SarProgram","start":14,"end":16,"text":"BP"}
{"kind":"SarCompartment","start":17,"end":20,"text":"J12"}
{"kind":"SarSubCompartment","start":21,"end":24,"text":"J54"}
```

No separate SAR section in the diagnostic JSON schema — rules reference `span` into the source buffer like every other diagnostic.
