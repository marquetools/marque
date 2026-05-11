---
agent: architect (preflight on `pr3c-c-commit7`)
date: 2026-05-11
scope: PR 3c.B Commit 7 — subcommit boundaries after preflight review
inputs:
  - docs/plans/2026-05-10-pr3c-consolidated-plan.md §"Commit 7" (lines 864-936)
  - specs/006-engine-rule-refactor/decisions/02-catalog-shape.md (Decision 2)
  - crates/scheme/src/constraint.rs (target of trait edit)
  - crates/engine/src/engine.rs (target of bridge insertion)
  - crates/capco/src/scheme.rs (target of catalog wiring)
discipline:
  - Read-only investigation; no code changes inside this file's authoring loop.
  - Re-verifies the plan's Commit-7 scope against the actual code state on staging
    (post-PR-3c.B-Commit-6 merge).
---

# Decision 6 — Commit 7 subdivision and engine-bridge gap

## Summary

The consolidated plan (`docs/plans/2026-05-10-pr3c-consolidated-plan.md`
§"Commit 7", lines 864-936) prescribes three subcommits inside PR 3c.B
Commit 7:

1. **7.1** — `ConstraintViolation` extension on `marque-scheme` (trait
   edit lands in isolation).
2. **7.2** — E058 decomposition (27 class-floor rows; delete
   `DeclarativeClassFloorRule`).
3. **7.3** — E059 decomposition (5 SCI per-system rows; delete
   `DeclarativeSciPerSystemRule`).

The preflight architect review against the actual repo state surfaced
**five gaps** in that plan. The most consequential is that the
engine's `lint()` path does not call `scheme.validate(...)` today —
the shared `marque_scheme::constraint::evaluate` walker is exercised
only by `crates/capco/tests/scheme_equivalence.rs` as a parity check
against the walker. Inlining the catalog rows in 7.2 / 7.3 without
first wiring the bridge would silently disable the 32 class-floor +
SCI per-system constraints.

This decision **modifies** the Commit 7 plan with five amendments and
re-numbers the subcommits accordingly. The end-state goals are
unchanged (~1700 LoC net, two walkers retired, 32 catalog rows fire
through the constraint catalog).

---

## Amendment 1 — corpus_parity delta math

**Plan claim** (line 909): "net effect on `corpus_parity.rs:170` is
+30 (each row is its own rule ID per
`decisions/02-catalog-shape.md` D2 lock)."

**Correct math**: `corpus_parity.rs:170` and
`post_3b_registration_pin.rs:44-48` both pin
`rule_set.rules().len()`. The 32 catalog rows are **not** registered
`Rule` impls — they're `Constraint::Custom` entries on
`scheme.constraints()`, a separate surface. Deleting the two walker
rules is **net -2** (33 → 31), not +30. `EXPECTED_RULE_IDS` in the
pin file loses `"E058"` and `"E059"`; the count comment math becomes
`47 - 16 = 31`.

The `corpus_parity.rs:170` pin shrinks to 31 in 7.3 (when
`DeclarativeClassFloorRule` is deleted) and again to 30 — actually 31
holds because both walkers retire — let me re-check: 33 - 2 = 31
final.

---

## Amendment 2 — engine-bridge gap

**Plan claim** (implicit): inlining the catalog rows is sufficient
for them to start emitting diagnostics through the bridge.

**Reality**: the engine `lint()` path at `crates/engine/src/engine.rs:614`
iterates `rule_set.rules()` and gets `Vec<Diagnostic>` back directly.
The `scheme.validate(...)` path is not called anywhere in the
production lint path — only in the equivalence-test parity check.
Without wiring the bridge, inlining the catalog rows silently
disables 32 constraints.

**Fix**: add an explicit subcommit (the new 7.2 in this re-numbering)
that wires the engine bridge — calls `self.scheme.validate(...)` per
candidate, maps non-`None`-span/severity violations to `Diagnostic`.
Lands cold because no catalog row populates the fields yet; first
fires when 7.3 wires E058.

---

## Amendment 3 — `Option<Span>` / `Option<Severity>`, not bare

**Plan claim** (line 877-880): "extend `ConstraintViolation` from its
current minimal shape to carry: `span: Span` and `severity: Severity`."

**Reality**: a required-field shape breaks ~25 in-tree construction
sites that emit dyadic `Conflicts` / `Requires` violations with no
natural span or severity from the constraint declaration alone (the
declared dyadic constraint is `(left, right)` — no anchor token, no
default severity policy).

**Fix**: use `Option<Span>` and `Option<Severity>` per Decision 2 §3
(which originally proposed this shape). The dyadic arms in
`crate::constraint::evaluate` pass `None`; only catalog rows that
commit to a user-facing diagnostic populate the fields.

---

## Amendment 4 — no `FixIntent` on `ConstraintViolation`; scheme-side helper instead

**Plan claim** (implicit, in the E059 subcommit description): E059's
companion-insert fixes flow through the constraint catalog.

**Reality**: `FixIntent<S>` lives in `marque-rules`, and `marque-rules`
depends on `marque-scheme` (Constitution VII Appendix D — post-PR-3c.A
graph). Attaching a `fix_intent: Option<FixIntent<S>>` field to
`ConstraintViolation` (in `marque-scheme`) would invert the graph and
create a cycle.

**Fix**: add a scheme-side helper
`CapcoScheme::fix_intent_for(name: &str, attrs: &CanonicalAttrs) ->
Option<FixIntent<CapcoScheme>>` that the engine calls when
materializing a `Diagnostic` from a `ConstraintViolation`. The helper
returns `None` for everything in 7.2; 7.4 populates the five E059
rows. This is the side-table pattern the now-retiring walker rules
used internally, just relocated to the scheme.

---

## Amendment 5 — stage walker deletion behind an equivalence test

**Plan claim** (§Tests, lines 914-917): "each row's existing test
... becomes a constraint-violation test asserting the inlined
constraint fires on the same input."

**Stronger property**: keep the walker for one commit while the
bridge fires both paths and an extended `scheme_equivalence.rs` test
confirms byte-identical `Diagnostic` output. Delete the walker only
after equivalence is green. Matches the PR 3b precedent where
`scheme_equivalence.rs` was the safety net.

---

## Re-numbered subcommit sequence

| Old | New | Scope | Land posture |
|---|---|---|---|
| 7.1 | **7.1** | `ConstraintViolation { span: Option<Span>, severity: Option<Severity> }`; patch ~25 construction sites to pass `None`. | Cold (trait edit; no consumer populates fields). |
| (gap) | **7.2** | Engine bridge: `Engine::scheme: CapcoScheme` field, `self.scheme.validate(...)` per candidate, populated-violation → `Diagnostic`. Plus scheme-side `CapcoScheme::fix_intent_for(name, attrs)` helper (returns `None` in 7.2). | Cold (bridge fires no diagnostics because no catalog row populates fields). |
| 7.2 | **7.3** | E058 inline: 27 class-floor rows populate `Option<Span>` + `Option<Severity>` in `class_floor_emit` via lifted `class_floor_anchor_span` / `first_span_of_optional` helpers; equivalence test in `crates/capco/tests/scheme_equivalence.rs` asserts byte-identity with pre-retirement walker output; delete `DeclarativeClassFloorRule`; update `corpus_parity.rs` count from 33 → 32; remove `"E058"` from `EXPECTED_RULE_IDS`. | Hot (catalog rows start firing through the bridge). |
| 7.3 | **7.4** | E059 inline: 5 SCI per-system rows populate fields; `CapcoScheme::fix_intent_for(name, attrs)` returns the four companion-insert FactAdd intents and the one HCS-P-sub-vs-ORCON-USGOV FactRemove intent; equivalence test green; delete `DeclarativeSciPerSystemRule`; update `corpus_parity.rs` count 32 → 31; remove `"E059"` from `EXPECTED_RULE_IDS`. | Hot (catalog rows + fixes start firing). |

---

## PR boundary

The original plan (§Commit 7) called for all four subcommits in a
single PR. The preflight review's gap-analysis materially expanded the
work in 7.2 (new engine bridge + scheme-side helper) and split 7.3
into 7.3 + 7.4. To keep review surface tractable:

- **PR 1 of 2** (current branch `pr3c-c-commit7`): subcommits **7.1
  + 7.2** only. Cold-land foundations. ~229 LoC. Adds the trait
  surface and the engine bridge but retires no walker — the existing
  `DeclarativeClassFloorRule` and `DeclarativeSciPerSystemRule`
  continue to fire E058 / E059 diagnostics through the rule pipeline.

- **PR 2 of 2** (follow-up): subcommits **7.3 + 7.4**. Walker
  deletions. ~1500 LoC. Lands after PR 1 of 2 merges so the bridge is
  available when the walkers retire.

This is a deliberate divergence from the plan's single-PR shape,
authorized by the constraint that destructive moves (walker
deletion) be staged behind cold-land foundations and equivalence
tests.

---

## References

- `docs/plans/2026-05-10-pr3c-consolidated-plan.md` §"Commit 7"
  (lines 864-936) — original three-subcommit plan.
- `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md` §3
  — `Option<Span>` / `Option<Severity>` rationale.
- `crates/scheme/src/constraint.rs:155-170` — `ConstraintViolation`
  shape (post-7.1).
- `crates/engine/src/engine.rs:97-110` — `Engine::scheme` field (post-7.2).
- `crates/engine/src/engine.rs:777-860` — engine bridge (post-7.2).
- `crates/capco/src/scheme.rs:2137-2168` — `CapcoScheme::fix_intent_for`
  (post-7.2).
