<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque-rules

Trait definitions for the Marque rule system — the contract every rule crate implements.

`marque-rules` defines the types and traits that bind the engine to its
rules. It contains no rule implementations; those live in downstream crates
such as `marque-capco`. Keeping the trait crate small and dependency-light
is what allows rule crates to be swapped without touching the engine.

## Role in Marque

```
marque-scheme (leaf trait surface: MarkingScheme, Lattice, …)
   ↑
marque-rules (rule traits, generic over S: MarkingScheme)
   ↑                ↑
marque-capco    marque-engine
(implements)    (orchestrates)
```

The engine depends on `marque-rules` and on whatever rule crates the binary
chooses to register. Rule crates depend on `marque-rules`, `marque-ism`, and
`marque-scheme` — the last because the generic `Rule<S>` API references scheme
types (`S::Canonical`, `S::Projected`, `FixIntent<S>`) directly. `marque-scheme`
stays the graph leaf (Constitution VII), so this edge keeps the graph acyclic.

## Public API

| Type | Role |
|---|---|
| `Rule<S>` | The trait every rule implements, generic over the marking scheme `S`. Stateless; given `S::Canonical` attributes plus a `RuleContext<'_, S>`, returns `Vec<Diagnostic<S>>`. |
| `RuleSet` | A bundle of rules exposed by a rule crate, with a schema version. |
| `RuleId` | Stable rule identifier (e.g., `"E002"`). |
| `Severity` | `Off` / `Suggest` / `Info` / `Warn` / `Error` / `Fix`. Configurable per rule. Defined in `marque-scheme`, re-exported here. |
| `Diagnostic` | A violation: rule, severity, span, message, citation, optional fix. |
| `FixProposal` | A proposed edit with `confidence: f32` and `FixSource` provenance. |
| `AppliedFix` | A `FixProposal` promoted by the engine, with timestamp + classifier id. The audit record. |
| `RuleContext` | Position context (`Zone`, `DocumentPosition`, per-page portion snapshot, page-level `ProjectedMarking`) and corrections map handed to `Rule::check`. |

## Usage

A minimal rule:

```rust,ignore
use marque_rules::{Diagnostic, Rule, RuleContext, RuleId, Severity};
use marque_scheme::MarkingScheme;

struct AlwaysWarn;

// Rules are generic over the marking scheme `S`. A concrete rule crate
// (e.g. `marque-capco`) implements `Rule<CapcoScheme>`; the trait itself
// stays scheme-agnostic so the engine can host more than one scheme.
impl<S: MarkingScheme> Rule<S> for AlwaysWarn {
    fn id(&self) -> RuleId {
        RuleId::new("example", "demo.always-warn")
    }
    fn name(&self) -> &'static str {
        "always-warn"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, _attrs: &S::Canonical, _ctx: &RuleContext<'_, S>) -> Vec<Diagnostic<S>> {
        // A real rule inspects `attrs` / `ctx` and returns diagnostics,
        // often carrying a `FixProposal`. This stub fires nothing.
        Vec::new()
    }
}
```

## Architectural invariants

Two contracts in this crate are enforced by convention, not the type system —
violating them is a compliance bug, not just a style issue:

1. **`AppliedFix::__engine_promote` is engine-only in production code.**
   Rule crates and CLI code must never construct `AppliedFix` directly
   in production paths. They produce `FixProposal` values; only
   `marque_engine::Engine::fix` may promote them. Bypassing this skips
   the confidence-threshold gate, the fix-ordering sort, and the
   overlap guard, and corrupts the audit log.

   Test code (`#[cfg(test)]` modules, `tests/` integration files,
   `dev-dependencies`-gated test-utility crates) MAY call
   `__engine_promote` to fabricate synthetic `AppliedFix` fixtures
   for testing audit emitters, sentinel checks, and renderers
   without a full `Engine`. The carve-out is scoped per Constitution
   V Principle V — see the doc comment on `__engine_promote` for the
   three constraints.
2. **`FixProposal` is pure data.** No timestamps, no classifier identity,
   no runtime context. That purity is what makes rule output snapshot-
   testable without a clock or user identity.

See the "Architectural Invariants" section of the workspace
[`CLAUDE.md`](../../CLAUDE.md) for the full list.

## Features

| Feature | Default | Effect |
|---|---|---|
| `serde` | off | `Serialize` / `Deserialize` for diagnostic and fix types. |

## WASM compatibility

WASM-safe. Pure types, no I/O, no threads.

## License

Marque License 1.0 (`LicenseRef-MarqueLicense-1.0`). See [LICENSE.md](./LICENSE.md).
