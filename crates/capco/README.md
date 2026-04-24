<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque-capco

CAPCO Layer 2 rule implementations for marque.

This crate provides hand-written rule implementations that consume the generated CVE predicates from `marque-ism` and produce enriched `Diagnostic` values — classifying *why* a violation occurred, attaching CAPCO citations, and emitting `FixProposal` values with confidence scores.

This is one of two crates where CAPCO/ISM is the headline; everything else in the workspace is general-purpose. For the engine that runs these rules, see `marque-engine`. For the vocabulary types they consume, see `marque-ism`.

## Role in Marque

Marque uses a two-layer rule architecture:

- **Layer 1 (generated)**: `marque-ism/build.rs` parses ODNI ISM schemas at build time and emits binary valid/invalid predicates.
- **Layer 2 (this crate)**: hand-written `Rule` implementations that consume Layer 1 predicates, classify the violation reason, decide whether to propose a fix, and cite the relevant CAPCO section.

Rule structs are zero-size and stateless. All config-dependent behavior (severity overrides, confidence threshold, classifier identity) is handled by the engine. Fixes are returned as `FixProposal` (pure data) — the engine snapshots runtime state into `AppliedFix` at promotion time. Rule crates must never construct `AppliedFix` directly.

## Rule Inventory

54 rules currently implemented: errors `E001`–`E051` (core CAPCO, SAR, SCI, NODIS/EXDIS, per-SCI-system constraints; `E017`/`E018`/`E019` retired in T035b), style `S001`–`S003`, warnings `W002`–`W003` (`W001` retired in T035c-14), corrections `C001`. ID prefix encodes default severity (`E` = error, `W` = warning, `S` = style/info, `C` = correction). Use `CapcoRuleSet::new()` or the `capco_rules()` entry point to obtain the full set.

The E042–E051 cluster uses the **fix-and-warn** pattern: `Severity::Warn` paired with a `FixProposal` — the fix is applied when confidence clears threshold, AND the warn diagnostic stays in the output so the user sees exactly what was corrected and can override if the intent was actually different. See [`rules_sci_per_system`](src/rules_sci_per_system.rs) module doc for the rationale.

## Usage

```rust
use marque_capco::{capco_rules, SCHEMA_VERSION};

let rules = capco_rules();
println!("CAPCO rules compiled against {SCHEMA_VERSION}");
// Hand `rules` to marque_engine::Engine to lint a document.
```

## Adding a New Rule

1. Add a zero-size struct in `src/rules.rs` implementing `marque_rules::Rule`.
2. Choose an ID with the right severity prefix: `E###` error, `W###` warning, `C###` correction.
3. Register it in `CapcoRuleSet::new()`.
4. Return `FixProposal` values for fixable violations; set `confidence` honestly. The engine's threshold gate decides what is auto-applied.
5. Cite the CAPCO section in the `Diagnostic::citation` field.

## Schema Versioning

The active ISM schema version is pinned in `marque-ism/Cargo.toml` under `[package.metadata.marque] ism-schema-version` and re-exported here as `SCHEMA_VERSION`. Bump intentionally when ODNI publishes spec updates.

## License

Apache-2.0
