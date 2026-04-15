# marque-ism

ISM vocabulary types and generated CVE enums for marque.

This crate is the leaf dependency in the marque workspace. It owns the canonical parsed-marking representation (`IsmAttributes`), zero-copy position types (`Span`), page-level aggregation (`PageContext`), and the closed Rust enums generated at build time from ODNI ISM schemas.

This crate implements the ISM vocabulary model *for* the marque rule engine. For the engine itself, see `marque-engine`. For the CAPCO rule implementations that consume this vocabulary, see `marque-capco`.

## Role in Marque

`marque-ism` is the pivot type. Every source format normalizes to `IsmAttributes` before rule validation runs. It sits at the bottom of the dependency graph:

```
marque-ism  ←  marque-core (scanner/parser)
            ←  marque-capco (rules consume generated predicates)
            ←  marque-engine, marque-wasm, ... (re-export via core)
```

**WASM-safe**: no I/O, no format dependencies, no platform-specific code. All schema parsing happens in `build.rs` at compile time.

## Code Generation

`build.rs` parses ODNI ISM specification files from `schemas/ISM-v2022-DEC/` and emits three modules into `OUT_DIR`, included via `src/generated.rs`:

| File | Contents |
|------|----------|
| `values.rs` | CVE enumeration types — closed Rust enums + lookup tables |
| `validators.rs` | Schematron-derived validation predicates |
| `migrations.rs` | Deprecated marking → replacement mappings |

Source XML/XSD files consumed:

- `CVE_ISM/CVEnumISMClassificationAll.xml` — classification levels
- `CVE_ISM/CVEnumISMSCIControls.xml` — SCI controls
- `CVE_ISM/CVEnumISMDissem.xml` — dissemination controls (with deprecation markers)
- `CVE_ISM/CVEnumISMSAR.xml` — SAR identifiers
- `CVE_ISM/CVEnumISMExemptFrom.xml` — declassification exemptions
- `CVE_ISMCAT/CVEGenerated/CVEnumISMCATRelTo.xsd` — country trigraphs

The active schema version is pinned in `Cargo.toml` under `[package.metadata.marque] ism-schema-version` and re-exported as `SCHEMA_VERSION`. `build.rs` asserts the on-disk schema matches this pin. Bump intentionally when ODNI publishes a new package.

## Public Types

| Type | Purpose |
|------|---------|
| `IsmAttributes` | Canonical parsed marking. Fields use `Box<[T]>` to avoid over-allocation. |
| `Span` | Byte offset range into the original source buffer. Never copies. |
| `MarkingCandidate`, `MarkingType` | Scanner output (Portion, Banner, CAB, PageBreak). |
| `Zone`, `DocumentPosition` | Structural context. Both are `Option`-typed in `RuleContext`. |
| `PageContext` | Page-level aggregation: `max()` for classification, union for SCI/SAR/dissem, intersection (with NOFORN supersession) for `REL TO`. Reset at scanner-emitted page-break candidates. |
| `Classification`, `SciControl`, `DissemControl`, `SarIdentifier`, `Trigraph`, … | Generated CVE enums. |
| `CapcoTokenSet` | Aho-Corasick automaton over CVE token list. |

## Usage

```rust
use marque_ism::{IsmAttributes, Classification, SCHEMA_VERSION};

assert_eq!(SCHEMA_VERSION, "ISM-v2022-DEC");

let attrs = IsmAttributes::default();
assert_eq!(attrs.classification, Classification::Unclassified);
```

## Features

| Feature | Effect |
|---------|--------|
| `serde` | Derives `Serialize`/`Deserialize` on public types. |

## WASM Compatibility

WASM-safe. No runtime I/O. All schema work runs in `build.rs` on the host.

## License

Apache-2.0
