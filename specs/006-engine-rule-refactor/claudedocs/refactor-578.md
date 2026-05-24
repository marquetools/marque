# Refactor Plan: Issue #578

## Objective
Consolidate the redundant Declarative rule wrappers from `crates/capco/src/rules_declarative.rs` directly into the `Constraint` definitions in the CAPCO constraint catalog (`core_catalog.rs` and others), allowing the existing `Engine` constraint bridge to emit these violations automatically.

## Background & Motivation
The file `crates/capco/src/rules_declarative.rs` contains ~17 redundant structs (e.g., `DeclarativeBareHcsRule`, `DeclarativeJointRelToRule`) that act as thin boilerplate over scheme constraints. Each wrapper checks for violations using `violations_for`, looks up token spans from `CanonicalAttrs`, and manually constructs a `Diagnostic` with a specific `Severity` and `FixIntent`.

By moving `span` and `severity` logic into the `Constraint` definitions, we can eliminate ~2,500 lines of boilerplate, reduce WASM module size, and simplify adding new constraints without needing to create new wrapper rules.

## Proposed Solution

1. **Extend `marque_scheme::Constraint` Enum**:
   Add `severity: Option<Severity>` to the dyadic constraint variants (`Conflicts`, `ConflictsWithFamily`, `Requires`).
   The generic `Custom` variant doesn't need schema changes because its evaluator (`evaluate_custom_by_attrs`) already constructs and returns `ConstraintViolation` envelopes where `span` and `severity` can be manually populated.

2. **Extend `marque_scheme::MarkingScheme` Trait**:
   Add a method `fn token_span(&self, marking: &Self::Marking, token: &TokenRef) -> Option<Span>` to allow the scheme to look up the source byte span for a token.
   The default implementation returns `None`.

3. **Update `marque_scheme::constraint::evaluate`**:
   Update the generic evaluator to use `scheme.token_span(marking, &left)` to populate the `span` field of the returned `ConstraintViolation` when evaluating `Conflicts` and `Requires` variants. Use the `severity` field from the constraint definition to populate `ConstraintViolation::severity`.

4. **Implement `token_span` in `CapcoScheme`**:
   Implement the new method to look up spans from `CanonicalAttrs::token_spans` based on the `TokenRef`.

5. **Update CAPCO Catalog (`core_catalog.rs` etc.)**:
   Populate `severity: Some(Severity::Error)` (or `Warn` / `Fix`) in the `Conflicts` and `Requires` catalog rows.
   For `Custom` constraints (e.g., E010, E012, E014), update their helper functions in `crates/capco/src/scheme/constraints/helpers.rs` to compute and return the appropriate `span` and `severity` within the `ConstraintViolation`.

6. **Migrate FixIntents**:
   Populate `fix_intent_by_name` in `crates/capco/src/scheme/adapter.rs` to return the `FixIntent` for specific constraints (like E054 `relido-conflicts-noforn`) that currently emit fixes in their wrapper rules.

7. **Delete `rules_declarative.rs` & Clean Up `rules.rs`**:
   Remove the ~17 redundant wrappers from `rules_declarative.rs`.
   Unregister them from `CapcoRuleSet::new()` in `rules.rs`.
   Ensure `has_diagnostic_constraints` remains `true`.

## Alternatives Considered
- Converting all constraints to `Custom` to handle span logic inside `evaluate_custom_by_attrs`. This was rejected because it loses the algebraic semantics and structure of dyadic constraints like `Conflicts` and `Requires`.
- Using `engine.rs` to evaluate the span after the fact. This was rejected because `ConstraintViolation` is scheme-agnostic and the engine does not know how to map constraint identifiers to specific tokens without scheme context.

## Implementation Steps
1. Modify `crates/scheme/src/scheme.rs` to add `token_span` to `MarkingScheme`.
2. Modify `crates/scheme/src/constraint.rs` to update `Constraint` variants and `evaluate`.
3. Modify `crates/capco/src/scheme/adapter.rs` and `marking_scheme_impl.rs` to implement `token_span` and `fix_intent_by_name`.
4. Update `crates/capco/src/scheme/constraints/core_catalog.rs` and `helpers.rs` to include `severity` and compute spans.
5. Delete the wrappers in `crates/capco/src/rules_declarative.rs` and unregister them in `crates/capco/src/rules.rs`.
6. Run `cargo test` and `cargo clippy` to fix any fallout in tests.

## Verification & Testing
- Run all project tests using `cargo test`.
- Specifically ensure `crates/capco/tests/rules_us1.rs` and other integration tests continue to pass. The exact-rule-ID-set pin might need updating if we retire some specific explicit rule IDs, but the `Engine` constraint bridge should seamlessly emit the same diagnostics.
