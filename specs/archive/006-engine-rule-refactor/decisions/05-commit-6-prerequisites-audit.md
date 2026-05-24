# Decision 05 — PR 3c.B Commit 6 prerequisites audit

**Status:** Resolved
**Date:** 2026-05-11
**Author:** Engine team (pre-flight pass for PR 3c.B)

## Context

The architect's pre-flight review of PR 3c.B raised three HIGH findings,
ostensibly blockers for Commit 6 (form-bucket migration):

1. `fix_intent_to_legacy_proposal` helper has a signature foot-gun —
   `(&FixIntent<S>, Span, RuleId) -> FixProposal` does not have access
   to scheme + projection + scratch buffer required for the
   `Recanonicalize` arm of the conversion. Inline into `fix_inner` or
   widen the helper?
2. Per-page `ProjectedMarking` retention path — `fix_inner` needs the
   projection to materialize `Recanonicalize` fixes; current engine
   discards per-page projection state after lint completes.
3. `render_scratch` scratch buffer at `crates/engine/src/engine.rs:428`
   is allocated + `.clear()`-ed per page but never written to. The
   `#[allow(dead_code)]` is paired with a forward-looking comment
   naming Commit 6 as first consumer — but no consumer is wired here.

## Audit

All three findings concern engine-side **Recanonicalize dispatch** —
the path the engine takes at fix-application time to materialize
`ReplacementIntent::Recanonicalize { scope }` into byte-precise output.

Per Path C of the consolidated plan
(`docs/plans/2026-05-10-pr3c-consolidated-plan.md` lines 100–175),
migrated rules through Commits 2–9 use **dual-population**: the rule
emits both `Diagnostic.fix` (byte-precise legacy `FixProposal`,
synthesized inside the rule body at lint time) AND
`Diagnostic.fix_intent` (structural `FixIntent` carrying the new
recognizer-vocabulary shape). The engine pulls bytes from
`Diagnostic.fix` for the splice and uses `fix_intent` only for audit
shape via the paired-promotion `AppliedFix::__engine_promote` path.

For `Recanonicalize`, the rule body — at lint time — computes the
canonical bytes via an **in-rule helper** (`canonicalize_trigraph_list`
+ `dedup_country_codes` in `crates/capco/src/rules.rs`) that produces
byte-identical output to what `render_canonical` would produce for the
matching axis / scope, then stuffs the bytes in the
`FixProposal.replacement` field. The two paths converge byte-for-byte
because the helper is the same source-of-truth canonicalization the
renderer's REL TO axis uses; they remain parallel implementations
through Commit 9 and merge at Commit 10's atomic cutover (the rule-side
helpers retire and the engine routes through `render_canonical` at
audit-promote time). No engine-side render at fix time is required for
the dual-population transition window.

The three architect findings therefore belong to **Commit 10's atomic
schema cutover** — the moment dual-population retires and the engine
becomes the sole authority on per-scope render materialization. At
that point:

- Finding 1 resolves by retiring `fix_intent_to_legacy_proposal`
  entirely (the dual-population path no longer needs a synthesizer —
  intent-only is the steady state).
- Finding 2 resolves by `Engine::fix_inner` consuming
  `ProjectedMarking` from a per-page snapshot stashed during lint
  (the design committed to in
  `crates/scheme/src/scheme.rs:241-248`).
- Finding 3 resolves by wiring the per-page scratch into the new
  `render_canonical` call site at fix-application time.

## Decision

Commit 6 proceeds **without** the prep work the architect's findings
seemed to require. Specifically:

- E002 (USA-not-first / USA-missing) and S003 (JOINT-USA-first
  convention) migrate to **dual-population**: emit
  `Diagnostic.fix_intent = Some(FixIntent::Recanonicalize { ... })`
  AND `Diagnostic.fix = Some(FixProposal { replacement: <bytes from
  rule's in-rule canonicalization helper —
  `canonicalize_trigraph_list` + `dedup_country_codes`, byte-identical
  to what `render_canonical` would produce on the matching axis /
  scope; the helpers retire at Commit 10's atomic cutover>, ... })`.
- 13 form rules + the E060 walker retire entirely (renderer absorbs
  them by construction; no residual emit needed).
- `fix_intent_to_legacy_proposal` stays `unimplemented!()`. The
  forward-looking docstring naming Commit 6 as first consumer is now
  inaccurate (Commit 6's dual-population path bypasses the helper);
  the docstring is updated in Commit 6 to name Commit 10 instead, but
  the `unimplemented!()` body is preserved.
- `render_scratch` at `engine.rs:428` stays allocated but unused. The
  pairing of `#[allow(dead_code)]` + `unimplemented!()`-style
  comments is preserved through Commit 9; Commit 10 either wires it
  or retires it (depending on whether engine-side render dispatch
  uses a per-page-scratch or per-fix-scratch).
- LintResult shape does not change in Commit 6 (no
  `page_projections` field added). Commit 10 adds it if needed.

This decision is consistent with:

- Path C amendment to Decision 9 (dual-population through Commit 9,
  atomic cutover at Commit 10).
- The `Recanonicalize` variant docstring's "Rules NEVER carry the
  `ProjectedMarking`" rule — dual-population doesn't carry the
  projection in the intent payload; the rule renders into a byte
  string and discards the projection.
- Constitution V Principle V — `AppliedFix.__engine_promote` is
  still the only constructor; the dual-population path uses
  `__engine_promote` (paired) with both arms populated.

## Verification

- Commit 6 lands with all baseline tests passing.
- No new fixture mutation needed for Recanonicalize beyond what
  rules' existing tests already cover (the rule's old `fix` field
  carries the same bytes, just produced via `render_canonical`
  instead of the rule's pre-Commit-6 hand-rolled canonicalization).
- Architect findings #1, #2, #3 carried forward to Commit 10's
  scope; reconfirmed during Commit 10's pre-flight.
