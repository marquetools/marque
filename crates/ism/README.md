<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque-ism

ISM vocabulary types and generated CVE enums for marque.

This crate is the foundational vocabulary crate of the marque workspace. It owns the pivot-type triple (`ParsedAttrs<'src>`, `CanonicalAttrs`, `ProjectedMarking`), zero-copy position types (`Span`), page-level aggregation (`PageContext`), and the closed Rust enums generated at build time from ODNI ISM schemas. `marque-ism` depends on `marque-scheme` (one-way edge — `ProjectedMarking::scope` carries `marque_scheme::Scope`); the consolidated plan's Appendix D anticipated this edge and Constitution VII v1.4.0 codified it.

This crate implements the ISM vocabulary model *for* the marque rule engine. For the engine itself, see `marque-engine`. For the CAPCO rule implementations that consume this vocabulary, see `marque-capco`.

## Role in Marque

`marque-ism` is the pivot type. Every source format normalizes to `CanonicalAttrs` before rule validation runs. It sits at the bottom of the dependency graph:

```
marque-ism  ←  marque-core (scanner/parser)
            ←  marque-capco (rules consume generated predicates)
            ←  marque-engine, marque-wasm, ... (re-export via core)
```

**WASM-safe**: no I/O, no format dependencies, no platform-specific code. All schema parsing happens in `build.rs` at compile time.

## Code Generation

`build.rs` parses ODNI ISM specification files at compile time and emits three modules into `OUT_DIR`, included via `src/generated.rs`:

| File | Contents |
|------|----------|
| `values.rs` | CVE enumeration types — closed Rust enums + lookup tables |
| `validators.rs` | Schematron-derived validation predicates |
| `migrations.rs` | Deprecated marking → replacement mappings |

Source XML/XSD files consumed (resolved via build-dependencies):

- From the [`ism`](https://crates.io/crates/ism) crate (`ism::package_root()`):
  - `CVE/ISM/CVEnumISMClassificationAll.xml` — classification levels
  - `CVE/ISM/CVEnumISMSCIControls.xml` — SCI controls
  - `CVE/ISM/CVEnumISMDissem.xml` — dissemination controls (with deprecation markers)
  - `CVE/ISM/CVEnumISMSAR.xml` — SAR identifiers (intentionally empty in public ODNI packages; see migration note)
  - `CVE/ISM/CVEnumISMExemptFrom.xml` — declassification exemptions
  - `CVE/ISM/CVEnumISM*.json` — JSON sidecars for per-token vocabulary metadata
  - `Schematron/ISM/ISM_XML.sch` — Schematron rules
- From the [`ism-ismcat`](https://crates.io/crates/ism-ismcat) crate (`ism_ismcat::package_root()`):
  - `Schema/ISMCAT/CVEGenerated/CVEnumISMCATRelTo.xsd` — country trigraphs
  - `Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml` — tetragraph membership

Both crates are vendored ODNI snapshots from
[`marquetools/ism-data`](https://github.com/marquetools/ism-data) with
SHA-256 manifest verification at the consumer's compile time.

`Cargo.toml` pins three independent versions intentionally:

| Pin | Meaning |
|-----|---------|
| `ism-schema-version` | Upstream ODNI ISM package version label (e.g. `ISM-v2022-DEC`) |
| `ism-data-version` | Snapshot version of the `ism-data` workspace (e.g. `20230609.0.0`) |
| `ismcat-tetra-version` | ISMCAT Tetragraph Taxonomy revision (e.g. `2022-NOV`, independent of the ISM bundle) |

`build.rs` cross-checks all three at compile time. Bump in lock-step
when ODNI publishes updates and the ism-data workspace is re-vendored.

## Public Types

| Type | Purpose |
|------|---------|
| `ParsedAttrs<'src>` | Borrowed parser output. Each token retains a `&'src str` slice into the source buffer for FR-019 round-trip. |
| `CanonicalAttrs` | Owned post-canonical pivot. What rules consume. Fields use `Box<[T]>` to avoid over-allocation. |
| `from_parsed_unchecked` | `#[doc(hidden)]` transitional adapter that converts `ParsedAttrs<'_>` → `CanonicalAttrs` by structural rename only. PR 3c replaces this with `MarkingScheme::canonicalize`. |
| `Span` | Byte offset range into the original source buffer. Never copies. |
| `MarkingCandidate`, `MarkingType` | Scanner output (Portion, Banner, CAB, PageBreak). |
| `Zone`, `DocumentPosition` | Structural context. Both are `Option`-typed in `RuleContext`. |
| `PageContext` | Page-level aggregation: `max()` for classification, union for SCI/SAR/dissem, intersection (with NOFORN supersession) for `REL TO`. Reset at scanner-emitted page-break candidates. |
| `Classification`, `SciControl`, `DissemControl`, `Trigraph`, … | Generated CVE enums. |
| `SarMarking`, `SarIndicator`, `SarProgram`, `SarCompartment` | Structural SAR types (not CVE-derived — see migration note below). |
| `CapcoTokenSet` | Aho-Corasick automaton over CVE token list. |

## Usage

```rust
use marque_ism::{CanonicalAttrs, SCHEMA_VERSION};

assert_eq!(SCHEMA_VERSION, "ISM-v2022-DEC");

// CanonicalAttrs::default() leaves classification as None. Rule code
// matches on `Some(...)` paths and treats `None` as the absent /
// unparseable case. Several construction paths produce `None`:
// strict-parse failure on a malformed marking, an empty marking, the
// lattice-bottom seed for projection accumulators, etc. — the type
// stays honest about absence rather than forcing a sentinel value.
let attrs = CanonicalAttrs::default();
assert!(attrs.classification.is_none());
```

## Features

| Feature | Effect |
|---------|--------|
| `serde` | Derives `Serialize`/`Deserialize` on public types. |

## WASM Compatibility

WASM-safe. No runtime I/O. All schema work runs in `build.rs` on the host.

## Migration Notes

### PR 3a: `IsmAttributes` → pivot-type triple

Pre-PR-3a a single owned struct named `IsmAttributes` lived in `attrs.rs` and served both as the parser output and the rule-consumption form. PR 3a splits that role across three types:

- `ParsedAttrs<'src>` (in `parsed.rs`) — borrowed parser output. Nine thin `Parsed*<'src>` wrappers (`ParsedClassification`, `ParsedSciMarking`, `ParsedSarMarking`, `ParsedFgiMarker`, `ParsedDissem`, `ParsedNonIcDissem`, `ParsedRelToEntry`, `ParsedDeclassifyOn`, `ParsedAea`) retain `&'src str` slices into the source buffer so the canonicalizer (PR 3c) can compute round-trip properties (FR-019) without re-borrowing.
- `CanonicalAttrs` (in `canonical.rs`) — owned post-canonical pivot type rules consume. Field shape mirrors the prior `IsmAttributes` exactly.
- `ProjectedMarking` (in `projected.rs`) — output of `MarkingScheme::project(scope, ...)`. Defined at PR 3a; PR 6 wires the engine to consume it. The type carries `scope: Scope` (from `marque-scheme`); the `marque-ism → marque-scheme` edge this introduces is anticipated by the consolidated plan's Appendix D and Constitution VII v1.4.0.

The `from_parsed_unchecked(ParsedAttrs<'_>) -> CanonicalAttrs` adapter is the transitional bridge through the keystone window; PR 3c replaces it with `MarkingScheme::canonicalize`.

### 0.3.0: `SarIdentifier` → `SarMarking`

Prior versions exposed `SarIdentifier` as a CVE-derived enum re-exported from `attrs`. The ODNI `CVEnumISMSAR.xml` is empty in all published ISM packages because SAR program identifiers are agency-assigned codewords, not centrally registered. The enum was therefore a typed placeholder that never matched anything at runtime.

`0.3.0` replaces the enum with a structural `SarMarking` type carrying programs, compartments, and sub-compartments per CAPCO-2016 §H.5 syntax. The pivot type's `sar_identifiers: Box<[SarIdentifier]>` is now `sar_markings: Option<SarMarking>`. The `SarIdentifier` enum has been removed from code generation; `TokenKind::SarIdentifier` remains as a `#[deprecated]` back-compat variant that the parser no longer emits, joined by new `TokenKind::SarIndicator`, `SarProgram`, `SarCompartment`, and `SarSubCompartment` variants.

### 0.x.0 (SCI compartments)

Additive. New `sci_markings: Box<[SciMarking]>` field on the pivot type provides structural SCI access (control system + compartments + sub-compartments per CAPCO-2016 §A.6); existing `sci_controls: Box<[SciControl]>` is preserved as a CVE enum projection for back-compat. Non-breaking.

## License

Marque License 1.0 (`LicenseRef-MarqueLicense-1.0`). See [LICENSE.md](./LICENSE.md).
