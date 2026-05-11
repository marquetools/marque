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
marque-rules (traits)
   ↑                ↑
marque-capco    marque-engine
(implements)    (orchestrates)
```

The engine depends on `marque-rules` and on whatever rule crates the binary
chooses to register. Rule crates depend only on `marque-rules` and
`marque-ism`.

## Public API

| Type | Role |
|---|---|
| `Rule` | The trait every rule implements. Stateless; given parsed attributes plus a `RuleContext`, returns `Vec<Diagnostic>`. |
| `RuleSet` | A bundle of rules exposed by a rule crate, with a schema version. |
| `RuleId` | Stable rule identifier (e.g., `"E002"`). |
| `Severity` | `Off` / `Warn` / `Error` / `Fix`. Configurable per rule. |
| `Diagnostic` | A violation: rule, severity, span, message, citation, optional fix. |
| `FixProposal` | A proposed edit with `confidence: f32` and `FixSource` provenance. |
| `AppliedFix` | A `FixProposal` promoted by the engine, with timestamp + classifier id. The audit record. |
| `RuleContext` | Position context (`Zone`, `DocumentPosition`, `PageContext`) and corrections map handed to `Rule::check`. |

## Usage

A minimal rule:

```rust
use marque_ism::{CanonicalAttrs, Span};
use marque_rules::{Diagnostic, Rule, RuleContext, RuleId, Severity};

struct AlwaysFire;

impl Rule for AlwaysFire {
    fn id(&self) -> RuleId { RuleId::new("X001") }
    fn name(&self) -> &'static str { "always-fire" }
    fn default_severity(&self) -> Severity { Severity::Warn }

    fn check(&self, _attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        vec![Diagnostic::new(
            self.id(),
            Severity::Warn,
            Span::new(0, 0),
            "example diagnostic",
            "EXAMPLE-§1",
            None,
        )]
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
