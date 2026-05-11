---
date: 2026-05-11
status: tracked deferral from PR 3c.B Commit 9 (engine + rule architecture refactor)
parent: specs/006-engine-rule-refactor/
covers: extending `MarkingScheme::evaluate_custom` to accept rule context (marking_type, page_context)
trigger: Stage-4 trait-surface extension PR — gated on `render_canonical` (for E005) and admonition emitter channel (for S005/S006) landing on the trait surface
authors: synthesized from PR 3c.B Commit 9 preflight (3-reviewer chain, 2026-05-11), decisions/02-catalog-shape.md D4
references:
  - specs/006-engine-rule-refactor/decisions/02-catalog-shape.md D4 (Path A fallback)
  - specs/006-engine-rule-refactor/followups/admonition-channel.md (S005/S006 retirement target)
  - specs/006-engine-rule-refactor/architecture.md §"What this commits us to" (render_canonical)
  - crates/scheme/src/scheme.rs:124-130 (evaluate_custom signature)
  - crates/engine/src/engine.rs:864-866 (constraint-catalog bridge call site)
---

# `MarkingScheme::evaluate_custom` Context Extension (Deferred)

## Status

**Deferred from PR 3c.B Commit 9. Not blocking Commit 9 itself.** Three
no-clean-fit rules (E005, S005, S006) stay as registered rules in
`crates/capco/src/rules.rs` under Path A of
`decisions/02-catalog-shape.md` D4. This file tracks the trait-surface
extension that is the structural prerequisite for migrating them off
hand-written `Rule` impls.

## Problem statement

`MarkingScheme::evaluate_custom` (defined at
`crates/scheme/src/scheme.rs:124-130`) takes only `&Self::Marking`:

```rust
fn evaluate_custom(
    &self,
    _name: &'static str,
    _marking: &Self::Marking,
) -> Vec<ConstraintViolation> {
    Vec::new()
}
```

`Self::Marking` for `CapcoScheme` is `CapcoMarking`, which wraps
`CanonicalAttrs` — token-level vocabulary state, no positional or
page-level context. The engine's constraint-catalog bridge at
`crates/engine/src/engine.rs:864-866` constructs the marking from
per-candidate `CanonicalAttrs` and calls `scheme.validate(...)`; the
`RuleContext` available at that call site is not passed through.

Three rules need context the predicate cannot reach:

| Rule | Context field needed | Why |
|---|---|---|
| **E005** (declassify-misplaced) | `RuleContext.marking_type` | Must skip `MarkingType::Cab` (declass legitimately lives in CAB). Without the gate the predicate fires on every CAB candidate carrying `declassify_on`. |
| **S005** (rel-to-uncertain-suggest) | `RuleContext.page_context` | Entire detection (`analyze_uncertain_reduction`) reads `page.portions()`, computes a page-level REL TO atom intersection, and decides the Suggest vs Info branch from page-wide banner state. |
| **S006** (rel-to-uncertain-info) | `RuleContext.page_context` | Shares `analyze_uncertain_reduction` with S005; same dependency. |

`evaluate_custom` has no path to either field today.

## Proposed extension shapes (open — pick at design time)

Surfacing options rather than committing prematurely. Pick during the
extension PR's design step.

**Option A — extend `evaluate_custom` signature with `&RuleContext`.**

```rust
fn evaluate_custom(
    &self,
    _name: &'static str,
    _marking: &Self::Marking,
    _ctx: &RuleContext,
) -> Vec<ConstraintViolation> {
    Vec::new()
}
```

Simplest mechanically. Forces a crate-graph edge from `marque-scheme`
to `marque-rules` (see "Crate-graph implications" below). Every existing
override in domain rule crates churns by one arg.

**Option B — introduce a stripped `ConstraintContext` subset.**

```rust
pub struct ConstraintContext<'a> {
    pub marking_type: MarkingType,
    pub page_context: Option<&'a PageContext>,
}

fn evaluate_custom(
    &self,
    _name: &'static str,
    _marking: &Self::Marking,
    _ctx: ConstraintContext<'_>,
) -> Vec<ConstraintViolation> {
    Vec::new()
}
```

Keeps the leaf-crate dependency surface smaller — `ConstraintContext` would
live in `marque-scheme` and bind only the two fields actually consumed by
constraints. `RuleContext` (in `marque-rules`) becomes a superset that
projects to `ConstraintContext` at the call site. Cost: a new type, a
project-to-stripped step at every call.

**Option C — split into `evaluate_custom_with_context`.**

Keep `evaluate_custom(name, marking)` (signature unchanged); add
`evaluate_custom_with_context(name, marking, ctx)` for context-aware
rows. Catalog rows declare which variant they want; the constraint
evaluator dispatches accordingly. Cost: two parallel evaluator paths;
catalog rows grow a kind discriminator.

**Option D — some combination.** E.g., Option B + per-row capability
opt-in (catalog row declares "I read marking_type", "I read
page_context", or "I read neither"), so the evaluator can avoid building
`ConstraintContext` for rows that don't need it. Premature optimization
risk; flag for design-time benchmarking only.

## Crate-graph implications

Constitution VII §IV pins `marque-scheme` as the workspace leaf
(directionality: `marque-ism` MAY depend on `marque-scheme` but not
vice versa; `marque-scheme` MUST NOT depend on `marque-ism`,
`marque-core`, `marque-rules`, or any domain crate).

`RuleContext` lives in `marque-rules` (`crates/rules/src/lib.rs:219`).
Options A and B both invite a question:

- **Option A** (passing `&RuleContext` directly) requires either:
  (i) `marque-scheme` depending on `marque-rules` — violates VII §IV
  directionality, or
  (ii) `RuleContext` moving down into `marque-scheme` — a leaf-crate
  expansion that would re-locate the type's home and force every
  rule consumer to re-import.

- **Option B** (`ConstraintContext` lives in `marque-scheme`) keeps the
  graph clean: the new type is leaf-local, and `RuleContext` projects
  to `ConstraintContext` at the engine bridge. Likely cleanest under
  the current directionality rule.

- **Option C** has the same dependency question as Option A but for
  the second method only.

The cleanest path under VII §IV is likely Option B. Confirm during the
extension PR.

## Affected rules and migration shapes

Each rule's eventual landing depends on a different trait-surface piece:

**E005 → `Recanonicalize { scope: Scope::Document }`** once
`render_canonical` (also deferred per
`architecture.md` §"What this commits us to") lands at document scope
with the ability to place declass in the CAB by construction. Authority:
CAPCO-2016 §E.1 p31 (Declassify On is a CAB line — single-value mandate)
+ §E.2 p32 (derivative-classification reaffirmation) + §D.1 p27 (banner
categories enumerate classification + control markings; declass is
conspicuously absent — negative-inference citation). The
`evaluate_custom` extension is a near-term prerequisite IF E005
migrates to a catalog row in the interim; if E005 migrates straight to
the renderer path when `render_canonical` lands, the context extension
is unnecessary for E005 specifically.

**S005 / S006 → admonition emitter channel** (per
`followups/admonition-channel.md`). The Suggest/Info severity split is
not §-grounded — §H.8 + ISMCAT Tetragraph Taxonomy treat REL TO via
pure set-membership language; nothing in CAPCO-2016 distinguishes
"active validation" from "consistent case." The split exists because
`marque_engine::Engine::lint` overwrites every emitted diagnostic's
severity with the rule's configured/default severity, so a single rule
cannot stably emit at two severities. The admonition channel collapses
this into one signal with per-emission severity. The context extension
is a hard prerequisite for S005/S006 migrating to a catalog row at all —
the entire body of `analyze_uncertain_reduction` is page-context-dependent.

## Trigger condition

Activate this followup when BOTH of these are true:

1. A concrete PR lands `render_canonical` on the `MarkingScheme` trait
   surface (the E005 retirement vehicle), AND
2. The admonition emitter channel is specced and built per
   `followups/admonition-channel.md` (the S005/S006 retirement vehicle).

If only (1) lands first, E005 may migrate via the renderer path without
this extension being necessary; S005/S006 stay at Path A and this
followup is partially fulfilled. If only (2) lands first, S005/S006
may migrate via the admonition channel without this extension being
necessary; E005 stays at Path A.

The extension is only mandatory if either rule migrates to a
`Constraint::Custom` catalog row in the interim before its primary
retirement vehicle lands.

## Open questions

1. **`RuleContext` home.** Stay in `marque-rules`? Move to
   `marque-scheme`? Introduce `ConstraintContext` as a stripped
   leaf-local type? (See "Crate-graph implications" above; Option B
   appears cleanest.)
2. **Optionality of `page_context`.** `RuleContext.page_context` is
   `Option<Arc<PageContext>>` today. Catalog rows that read it must
   handle `None` (early-out, treat as no portions). Does the constraint
   evaluator need a per-row opt-in declaration ("this row reads
   page_context"), or do all context-aware rows always receive whatever
   the engine bridge has?
3. **Default-implementation compatibility.** Adding a parameter to a
   trait method with a default body breaks every downstream `impl
   MarkingScheme for ...` that overrides it. Audit the workspace +
   `marque-applied.md` for known external implementors before locking
   the signature.
4. **Existing `Custom` rows that don't need context.** Today's class-floor
   and SCI-per-system catalogs route through `evaluate_custom_by_attrs`
   (`crates/capco/src/scheme.rs:1995`) with no context. Do they all stay
   context-free, or does the extension consolidate the evaluator into
   one context-aware path with `_ctx` ignored in the non-context rows?
5. **Interaction with `marque-applied.md` §3.5/§3.6 renderer plans.** If
   the renderer absorbs E005 directly when `render_canonical` lands,
   does that change the priority of the extension itself? (Likely yes —
   the extension is then only S005/S006-driven, which the admonition
   channel may eliminate independently.)

## Non-goals

- This followup does NOT design the admonition channel. That's covered
  by `followups/admonition-channel.md`.
- This followup does NOT design `render_canonical`. That's part of the
  Stage-3 (PR 4) / Stage-4 (PR 5+) renderer track per `marque-applied.md`.
- This followup does NOT propose moving `RuleContext` unilaterally —
  the home question is open and resolved at design time of the
  extension PR.
