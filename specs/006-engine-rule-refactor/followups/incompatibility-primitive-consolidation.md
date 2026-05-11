---
date: 2026-05-11
status: tracked deferral from PR 3c.B (engine + rule architecture refactor)
parent: specs/006-engine-rule-refactor/
covers: unified `Constraint::Incompatible` primitive consolidating conflict-family rules
trigger: Stage-4 rule-count consolidation drive (PR 5+) toward the 8–18 band
authors: synthesized from 2026-05-11 marque-lattice-consultant session
references:
  - .claude/skills/marque-lattice-consultant/references/security-lattice.md §6 (supersession algebra), §7 (intersection-with-blackball)
  - .claude/skills/marque-lattice-consultant/references/marque-applied.md §3 (PR 3b stall walkthrough), §6 (rewrite convergence), §7 (NOFORN-clears-REL-TO)
  - .claude/skills/marque-lattice-consultant/references/frames-locales.md §9 (non-lattice diagnosis)
---

# `Constraint::Incompatible` Primitive Consolidation (Deferred)

## Status

**Deferred from PR 3c.B. Not blocking.** Current conflict-family rules
migrate through PR 3c.B Sub-PRs 8.B/8.C/8.D/8.E/8.F under their existing
per-family shapes (`FactRemove`, `FactAdd`, `PageRewrite`). This file
tracks the Stage-4 consolidation that unifies them into a single
declarative primitive.

The 2026-05-11 marque-lattice-consultant session established that the
conflict-family rules across three structurally-similar groups —
JOINT (E016, E036), NODIS/EXDIS (E037, E039, E041), and
RELIDO/REL-TO/NOFORN (E054, E055, E056, E057) — share an underlying
"token-level incompatibility relation." The user's framing ("type
incompatibility / ejection — these can't be in the same lattice") is
structurally correct; the formal taxonomy below decomposes it into the
specific sub-shapes the primitive must accommodate.

## The taxonomy

### Category A — Supersession rewrite with policy decision

One token strictly wins; the other(s) are removed via a deterministic
rewrite. Formal home: supersession-quotient on the powerset of dissem
tokens (`security-lattice.md` §6 framing 1) + intersection-with-blackball
(`security-lattice.md` §7) for the recipient-list cases.

Three sub-shapes:

**A.1 — Single-fact removal.** `Remove(loser, Scope)`. The common case.
Examples:
- NODIS supersedes EXDIS (E041, Scope::Portion)
- NOFORN clears REL TO (E054, Scope::Portion; E039 page-rewrite form is
  the same rule at Scope::Page — *scope-agnostic* per user confirmation)
- ORCON dominates RELIDO (E056)
- DISPLAYONLY dominates RELIDO (E055)
- RELIDO conflict variants (E057)

**A.2 — Chain-removal cluster.** `Remove(SmallVec<FactRef<S>>, Scope)`
where multiple losers are removed atomically. The only known case:
- RD > FRD > TFNI (E024) per AEA §H.6. RD removes both FRD *and* TFNI
  in one atomic policy decision; multi-emit would corrupt the audit log
  by representing one decision as three repairs.

The SmallVec extension to `ReplacementIntent::FactRemove` is the *only*
near-term prerequisite blocked by this consolidation. It is tracked
separately as a GitHub issue (see `## Near-term prerequisite` below)
and lands before Sub-PR 8.C.

**A.3 — Transmute via equivalence-table.** Decomposes to
`Remove(loser) ⊕ Add(equivalent)` in one atomic audit promotion. The
losing token is replaced by its policy-equivalent counterpart from a
domain-vocabulary lookup. The known case:
- JOINT + RESTRICTED → JOINT + CONFIDENTIAL (E016, eventually fed by
  a foreign-equivalence vocabulary table — sourcing TBD per Open
  Question 1 below; candidate authoritative source is CAPCO-2016
  Appendix A §4 / Five Eyes Marking Comparisons, currently not
  vendored. CAPCO-2016 §H.3 p56 itself does NOT publish this
  equivalence — it only establishes that RESTRICTED is not a US
  classification level.)

A.3 does *not* require a new `ReplacementIntent` variant — the existing
`FactRemove` + `FactAdd` compose to it, *provided the engine treats the
cluster as one repair in the audit log*. That clause is the load-bearing
engine-side requirement; it parallels the A.2 atomic-cluster requirement
and may share the same engine mechanism.

### Category B — Genuine mutual exclusion (no policy decision)

Both tokens conflict but neither dominates; resolution is human (pick
one or restructure the marking). Formal home: **not a lattice operation**
— it's a grammar-admissibility check per `frames-locales.md` §9. The
result is *no marking*, not a different marking.

Known cases:
- JOINT + HCS (E036) per §H.3 p57 — HCS is US-only-classified
  information; JOINT explicitly claims co-production. Per user note:
  "JOINT classifications are exceedingly rare and largely exclusive to
  DOD; HCS is CIA-only, an agency which doesn't have the word 'JOINT'
  in its vocabulary" — so this combination is academic in practice.

Category B emits `Severity::Error` + a non-applied
`Severity::Suggest` companion diagnostic (e.g., "did you mean
SECRET//HCS-P//REL TO [LIST]?"). The engine's auto-apply filter at
`engine.rs:1378` excludes `Severity::Suggest` by construction, so no
auto-repair happens; the suggestion text appears in human-readable
output only.

## The umbrella primitive

```rust
// In marque-scheme, alongside Constraint::Custom:
enum IncompatResolution<S: MarkingScheme> {
    // Categories A.1 + A.2 — atomic removal of one or more losers
    Remove {
        facts: SmallVec<[FactRef<S>; 2]>,
        scope: Scope,
    },
    // Category A.3 — atomic transmute via equivalence-table.
    // Decomposes to Remove(loser) ⊕ Add(equivalent) at evaluation time;
    // engine must treat both halves as one audit repair.
    Transmute {
        loser: FactRef<S>,
        equivalent_resolver: TransmuteResolver<S>,
        scope: Scope,
    },
    // Category B — emit diagnostic; optional non-applied suggestion
    Reject {
        suggest: Option<SuggestText>,
    },
}

// Catalog row, on CapcoScheme:
struct IncompatRow<S: MarkingScheme> {
    name: &'static str,        // e.g., "conflict/noforn-clears-rel-to"
    when_both_present: (FactRef<S>, FactRef<S>),
    resolution: IncompatResolution<S>,
    citation: &'static str,    // CAPCO §-citation, D13 discipline
}
```

`TransmuteResolver<S>` is a function pointer or trait-object that
consults a domain-vocabulary table (the foreign-disclosure-equivalence
map for the JOINT+RESTRICTED case). The resolver lives in
`marque-capco`; the trait surface lives in `marque-scheme`. This
mirrors the PR 3b.D/E walker pattern: declarative primitive in the
scheme crate, catalog rows + domain vocabulary in the rule crate.

## What this consolidates

Sub-PR 8.B/8.C/8.D/8.E/8.F land their migrations under the *current*
per-family shapes:

| Sub-PR | Rules | Current shape (post-3c.B) |
|---|---|---|
| 8.B | E016, E036 | `Diagnostic { fix: None }` (no auto-fix) |
| 8.C | E024 | `FactRemove(SmallVec<2>, Scope::Portion)` (post-prerequisite) |
| 8.D | E010/E012/E014/E015/E038/E053 | `FactAdd(_, _)` — *Requires* bucket, not part of this consolidation |
| 8.E | E037 | `Diagnostic { fix: None }` (no auto-fix) — Stage-4 target `Reject {}` (Category B) |
| 8.E | E041 | `Diagnostic { fix: None }` (no auto-fix) — Stage-4 target `FactRemove(EXDIS, Scope::Portion)` (Category A.1, blocked on parser within-category-separator gap) |
| 8.F | E039 retirement (W003 separate) | absorbed into E031 banner walker |

Stage-4 consolidation lowers all of these (except 8.D Requires-bucket,
which is its own family) plus the 8.A migrations (E055/E056) plus the
beachhead-PR migrations (E054, E057) into rows on an
`INCOMPAT_CATALOG: &'static [IncompatRow<CapcoScheme>]`, dispatched by
a single `DeclarativeIncompatRule` walker mirroring the PR 3b.A banner
walker / 3b.D class-floor walker / 3b.E SCI-per-system walker shape.

Approximate rule-count delta: **~11 rules retired, 1 walker added →
net −10 toward the Stage-4 8–18 target band** (per
`marque-applied.md` §3.10).

## Near-term prerequisite (separate tracking)

The SmallVec extension to `ReplacementIntent::FactRemove` is the only
piece of this consolidation that has a near-term consumer (Sub-PR 8.C).
It is tracked as a standalone GitHub issue with its own PR landing
before 8.C — *not* under this Stage-4 followup. The two are separable:

- **Near-term (this PR cycle):** SmallVec extension, lands before 8.C.
  Concrete, scoped, ~50 LoC engine change + per-call-site migration.
- **Stage-4 (PR 5+):** `Constraint::Incompatible` umbrella primitive
  consolidating all conflict-family rules. Architectural, scoped to
  the rule-count consolidation drive, ~500–800 LoC.

## Trigger condition

Activate this followup when:

1. PR 3c.B has merged (FixIntent infrastructure stable).
2. Stage-3 (PR 4) walker decomposition has run its course and the
   remaining rules form a stable population for consolidation analysis.
3. Rule-count drive enters Stage-4 with target band 8–18 rules and the
   conflict-family is among the highest-row populations.

If a new conflict-family rule lands during PRs 4–5 (e.g., a new SCI
control-system conflict or a new dissem incompatibility), it should
land in its per-family shape *with a comment referencing this
followup*, so the Stage-4 migration absorbs it cleanly when the time
comes.

## Open questions (for Stage-4 planning, not for now)

1. **Foreign-equivalence vocabulary sourcing.** The A.3 case requires a
   policy-equivalence table (UK RESTRICTED ↔ US CONFIDENTIAL, etc.).
   CAPCO-2016 §H.3 p56 establishes only that RESTRICTED is not a US
   classification level — it does NOT publish an equivalence table.
   The authoritative table appears in CAPCO-2016 Appendix A §4 (Five
   Eyes Marking Comparisons), which is not currently vendored in
   `crates/capco/docs/`. Sourcing candidates: (a) hand-curated
   `&'static` table in `marque-capco::vocab` transcribed from
   Appendix A §4; (b) Appendix A §4 vendored and parsed at build
   time, if ODNI publishes it in a machine-readable form (currently
   human-readable PDF only — vendoring requires re-licensing
   verification); (c) generated from a hypothetical ISMCAT sidecar
   if ODNI ever publishes one. Pre-Stage-4: confirm sourcing and
   verify Constitution VIII attribution of any embedded equivalence
   text. Bilateral disclosure-policy tables (non-CAPCO sources) are
   secondary alternatives if Appendix A §4 proves un-vendorable.
2. **A.3 single-country JOINT branch.** User noted: when the JOINT list
   has only one non-US partner, the suggest path should *also* offer
   "or did you mean `//GBR RESTRICTED//REL TO USA, GBR`?" (the
   FGI-non-US-attribution alternative). This is a second
   `Severity::Suggest` companion diagnostic, separate from the A.3
   auto-applied fix. Pre-Stage-4: confirm the catalog row shape can
   carry a conditional suggest (single-country branch only) or
   whether it's a separate row keyed on JOINT-list cardinality.
3. **Engine atomic-cluster mechanism.** A.2 and A.3 both require the
   engine to treat a multi-fact change as one audit repair. Is this
   one mechanism (general atomic-cluster promotion) or two
   (SmallVec-specific for A.2, decomposition-specific for A.3)?
   Pre-Stage-4: design.
4. **FRD > TFNI strictness.** User noted "fairly sure FRD dominates
   TFNI too." Pre-Stage-4 (or pre-8.C): verify against CAPCO-2016
   §H.6 + `capco-context.md` §H.6 — does RD-removes-FRD-AND-TFNI
   compose from RD-removes-FRD + FRD-removes-TFNI (two cluster rows
   with same scope), or is it one declared cluster `RD > {FRD, TFNI}`?
   Affects catalog row count and audit-record shape.

## Non-goals

- This followup does *not* consolidate the **Requires bucket** (8.D:
  E010/E012/E014/E015/E038/E053 — `FactAdd` shape). Requires has a
  different formal structure (it's not incompatibility — it's
  *implication*: "if X present, Y must be present"). A parallel
  `Constraint::Requires` umbrella may be appropriate but is a
  separate followup.
- This followup does *not* address the **passthrough** / **NNPI**
  bounded-confidence paths in `marque-applied.md` §3.7. Those are
  cross-cutting concerns that compose with this primitive but don't
  drive its shape.
- This followup does *not* replace `Constraint::Custom` — that
  primitive remains the escape hatch for rules whose semantics genuinely
  don't fit the umbrella (per `decisions/02-catalog-shape.md` D4).
