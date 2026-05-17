---
date: 2026-05-10
status: tracked deferral from PR 3c (engine + rule architecture refactor) — updated 2026-05-17 (PR #488) to reflect S006 retirement
parent: specs/006-engine-rule-refactor/
covers: structural diagnostic channel for advisory markings
trigger: first concrete consumer needs structurally-distinct admonition emission
authors: synthesized from architecture.md §3.0.b, decisions/02-04*.md, rule-body-audit.md; S006-retirement annotations from PR #488 (2026-05-17)
---

> **PR #488 update (2026-05-17).** S006 was retired entirely — the
> historical S005/S006 Suggest/Info split was an engine workaround
> (per-rule severity overwrite), NOT §-grounded; CAPCO-2016 §H.8 +
> §D.2 Table 3 rule 21 apply uniformly to REL TO atom-semantics.
> The collapse leaves S005 as the sole survivor at `Phase::PageFinalization`.
> The admonition channel remains the documented long-term home for the
> per-emission severity signal the collapse temporarily forecloses;
> when the channel lands, the post-#488 single-S005 rule will split
> per-emission into the original Suggest/Info semantics without
> re-introducing two registered rules.

# Admonition Channel (Deferred)

## Status

**Deferred from PR 3c. Not blocking.** Current admonition rules continue
to emit through the existing `Diagnostic` + `Severity::Warn` channel,
which is functionally correct. This file is the tracked deferral so the
decision doesn't vanish into "to-do later."

## What it is (when built)

A separate diagnostic emission stream for advisory markings that do
not map to the fact-set delta vocabulary
(`FactAdd` / `FactRemove` / `Recanonicalize`). The architecture
restatement (`architecture.md` §"The §3.0.b purpose split") names
"admonition emitter (separate channel)" as the structural home for
rules like:

- **RD warning** — Atomic Energy Act §H.6 advisory ("This document
  contains Restricted Data...")
- **RAWFISA notice** — Foreign Intelligence Surveillance Act §H.8
  handling advisory
- **IMCON SAT warning** — Controlled Imagery §H.4 handling advisory

These are not divergence detections and have no fix proposals; they
are provenance / handling notices the consumer of the document
(printer, viewer, downstream system) is expected to surface.

## Why deferred from PR 3c

1. **No current consumer needs the structural distinction.** The two
   in-tree admonition rules (W002 commingling caution per
   `rule-body-audit.md` line 75; W034 SCI custom-control info per
   line 62) and the three audit-noted future rules (RD/RAWFISA/
   IMCON-SAT) emit `Severity::Warn` today with no fix; the existing
   diagnostic stream carries them correctly. No CLI / WASM / server
   consumer distinguishes admonition from rule-firing today;
   nothing breaks by deferring.
2. **The bridge isn't built.** S005 / S006
   (`decisions/02-catalog-shape.md` D4) need both an admonition
   channel AND a renderer-side uncertainty-band mechanism for the
   `rel_to`-opaque-tetragraph case. Building one without the other
   is premature.
3. **PR 3c.B is already the largest PR in the refactor.** Adding a
   new diagnostic channel design pushes it past the reviewability
   threshold the 2-PR split was set up to respect.
4. **`decisions/03-empirical-concerns.md` D10** explicitly bucketed
   the admonition channel as out-of-scope for PR 3c. This file
   ratifies that decision and gives it a tracked home.

## In-scope-after-PR-3c state

Until the trigger fires, the admonition rules sit at:

- **W002, W034** — keep emitting via `Severity::Warn` on the
  standard diagnostic stream. No source change.
- **S005** — Post-PR-#488, dispatched at `Phase::PageFinalization`
  (single Suggest-severity rule). Pre-PR-#488 this entry read
  "S005, S006 — remain provisional `Constraint::Custom` per D4";
  S006 was retired in PR #488 as not §-grounded (see PR #488 update
  banner at the top of this file). The retirement-target comment in
  source still names admonition as the eventual home: when the
  channel lands, the single S005 rule splits per-emission into
  the original Suggest/Info semantics without re-introducing two
  registered rules.
- **RD warning, RAWFISA notice, IMCON SAT warning** (three audit-
  named future rules) — **DO NOT add to the rule catalog before
  the channel exists.** They are admonition-shape from day one;
  routing them through the regular diagnostic stream would set the
  wrong precedent and complicate the future migration.

## Trigger for the future PR

Spec and build the channel when ONE of these conditions arrives:

1. **A concrete consumer needs the structural distinction** — e.g.,
   a DLP filter, compliance auditor, print pipeline, or
   classification-handling tool needs to consume "this document
   contains RD" as a non-diagnostic event separate from rule-
   firing.
2. **A second admonition-shape rule needs to be added to the
   catalog** — e.g., a NATO advisory, a partner-national advisory,
   or one of the three audit-named future rules above. The
   marginal cost of routing it through `Severity::Warn` at that
   point exceeds the cost of specifying the channel.
3. **PR 3d or later spans a related architectural surface** —
   e.g., the recognizer-uncertainty surface (S005/S006 retirement)
   gets specced; admonition can ride alongside as a sister
   channel.

## When built — design questions

These are deferred until the trigger fires. The future spec must
answer each:

1. **Channel separation.** Is admonition a separate stream
   alongside `Diagnostic` / `AppliedFix`, or a third `Severity`
   variant (`Severity::Admonition`)? G13 audit-content-ignorance
   (Constitution V Principle V) applies regardless — admonition
   payloads carry token canonicals, category IDs, citations;
   never document bytes.
2. **Engine plumbing.** Does `Engine::lint` return admonitions
   separately from `diagnostics`, or do consumers filter on
   severity? CLI / WASM / server treat them identically?
3. **Rule API surface.** Do admonition rules implement
   `Rule<S>`, or a sister trait `AdmonitionRule<S>`? The latter
   forces the structural distinction at the type level; the former
   keeps the catalog uniform.
4. **Severity-model interaction.** Are admonitions configurable
   (`Severity::Off` to suppress)? The CAPCO context — RD warning
   is mandated by §H.6 — argues no; the operational context
   (CI pipelines may want to suppress non-blocking warnings)
   argues yes.
5. **Audit recording.** Are admonitions audited like fixes are?
   Could ride the existing audit stream (rule fired, no fix
   applied) or get a separate audit channel. Same
   `MARQUE_AUDIT_SCHEMA` either way; the schema is additive.

## Constitution alignment

- **Principle V (audit-first).** No content leakage. Admonitions
  carry structural information only — token canonicals, category
  IDs, citations, spans for location, BLAKE3 digests for
  reference. Never document bytes.
- **Principle VI (dataflow pipeline).** Admonition emission is a
  per-rule phase output; the channel is orthogonal to the four-
  stage pipeline, not a new pipeline stage.
- **Principle VII (crate discipline).** Admonition channel lives
  in `marque-rules` alongside `Diagnostic` and `FixIntent`. No
  new crate, no new graph edge.
- **Principle VIII (citation fidelity).** Admonition messages cite
  their authority just as rule diagnostics do — no exemption.

## References

- `specs/006-engine-rule-refactor/architecture.md` §"The §3.0.b
  purpose split" — admonition row in the table.
- `specs/006-engine-rule-refactor/rule-body-audit.md` —
  W002 (line 75), W034 (line 62), S005 (line 56), S006 (line 57),
  audit-named future rules in "What this implies" (line 161).
- `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md`
  D4 — provisional `Constraint::Custom` for E005/S005/S006 with
  admonition channel as their retirement target.
- `specs/006-engine-rule-refactor/decisions/03-empirical-concerns.md`
  D10 — admonition channel explicitly out-of-scope for PR 3c.
- CAPCO-2016 §H.6 (RD warning), §H.8 (RAWFISA notice), §H.4
  (IMCON SAT warning) — primary sources for the three audit-named
  future rules.
