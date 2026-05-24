<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 3b consultation verdict — staged collapse path

**Date:** 2026-05-07
**Status:** locked (decisions register entry: `specs/006-engine-rule-refactor/decisions.md` D13 amendment 2026-05-07)
**Source:** marque-lattice-consultant skill consultation against `specs/006-engine-rule-refactor/*` and `docs/plans/2026-05-01-lattice-design.md`.
**Algebraic justification:** `.claude/skills/marque-lattice-consultant/references/marque-applied.md` §3 (PR 3b stall walkthrough) and §3.11 (stage sequencing).

---

## 0. Why this doc exists

The PR 3b stall (recorded in `docs/plans/2026-05-01-lattice-design.md`
§0 and the bridge's §3) had two roots: insufficient order-theory
apparatus to carve the 56-rule catalog into algebraic primitives, and
no clear sequencing of which primitives were available at PR 3b time
versus which had to land first.

The bridge (`marque-applied.md`) closed the apparatus gap. This
verdict doc closes the sequencing gap. It is the dated decision
record; the algebra lives in the bridge; the operational acceptance
criteria live in `plan.md` D13 addendum + `tasks.md` T026a–T026f and
T108b–T108d.

## 1. Verdict on the bridge

**Verdict: (b) — partial match, sound algebra, wrong sequencing.**

Algebraic carving (a)-grade: §3.0.a "form is not shape," §3.0.b
"structure rules vs other-purpose rules," §3.4.1 transmutations as
`PageRewrite` entries, §3.4.3 cross-axis FGI rollup, §3.4.6 class-
floor catalog, §3.7.1 passthrough policy, §3.7.2 NNPI bounded-
confidence non-commit, §4.7 closure operator (monotone + extensive +
idempotent verified, suppression case checked per-row), §4.8 FGI/
JOINT consensus join-semilattice, §6 / §7 / §8 verdicts.

Sequencing pushback: §3.10.3's "path from 56 to ~9" implicitly
schedules Moves 1–7 inside PR 3b, but two moves (Move 6 closure
operator wiring; Move 3 family-predicate `RhsFamily` variant)
require new primitives that don't ship at PR 3b time.

Decision: re-sequence into four stages. PR 3b ships the
declarative-catalog moves over existing primitives only. PR 3.7
ships the new primitives (RhsFamily + closure operator). PR 4 wires
the closure operator's `ClosureRule` catalog (catalog shape pivoted
2026-05-11 per `decisions.md` D18 — Option C public catalog,
retiring the 2026-05-07 private-`ImplTable<S>` pin) and adds
per-category Lattice impls + property tests. PR 5+ moves
style/ordering to renderer correctness.

## 2. Resolved questions

| Q | Resolution |
|---|------------|
| **Q-3.9** — "single citation per rule": per declarative entry or per `impl Rule` block? | **Per declarative entry.** Walkers (3b.A banner, 3b.C RELIDO, 3b.D floors, 3b.E SCI per-system, 3b.F renderer fallback) are each one `impl Rule` block delegating to a catalog whose rows each carry their own §-citation. Constitution VIII (citation integrity is per-claim, not per-block). Locked in `plan.md` D13 + `decisions.md` D13 amendment. |
| **Q-3.4.2-timing** — where does `Constraint::Conflicts::RhsFamily(predicate)` land? | **PR 3.7** (T108b). PR 3b ships the enumerated form (~15–20 single-token rows per `marque-applied.md` §3.4.2 fallback table); compaction to 2 family rows lands in PR 4. |
| **Q-4.7-timing** — where does the `marque-applied.md` §4.7 closure operator primitive land? | **PR 3.7** (T108c). Implication tables ship with the primitive. CAPCO `ClosureRule` catalog hand-curated with §-citations per `marque-applied.md` §4.7.5. (Catalog shape pivoted 2026-05-11 from private `ImplTable<S>` to public `ClosureRule` — see `decisions.md` D18.) PR 4 wires the call site into `Engine::project` per §4.7.4 pipeline. |
| **Q-4.7-Cl_supp** — single shared FD&R suppressor or per-row suppressors? | **Single shared FD&R predicate** as primary; per-row override available for future implications that need it. Consistent with `marque-applied.md` §4.7.1 table-design unification. |
| **Q-Move-7-timing** — style/ordering → renderer at PR 5+? | **PR 5+, with a single fallback walker retained in PR 3b** (T026f). The renderer trait surface is a separate effort; PR 3b retains one "non-canonical input" diagnostic walker covering E020 / E023 / E028 / E033 until the renderer arrives. |

## 3. Open questions (do NOT block PR 3b; tracked for follow-up)

| Q | Status | Owner / next step |
|---|--------|-------------------|
| **Q-3.4.6a** — class-floor catalog: build-time generated or hand-curated? | **Hand-curated in PR 3b**, with a 30-minute spike (next sprint) to inspect ODNI XML coverage in `crates/ism/schemas/ISM-v2022-DEC/Schema/IC-ISM.xsd`. If schema carries floor data uniformly, swap to build-time at next ODNI bump. | PR 3b implementer (spike); PR 3.7 reviewer (decision record) |
| ~~**Q-FgiSet-vs-§4.8**~~ — does existing `FgiSet::Present` already model §4.8 consensus-or-fallback? | **Resolved 2026-05-07** (user confirmation): yes. The FGI category (as distinct from "non-U.S. portions for non-ICD-206-compliant portions") is the **union of FGI / non-U.S. joint trigraphs unless any portion is unacknowledged, in which case the banner falls back to bare `FGI`**. This matches the bridge §4.8.2 join law applied to `FgiSet::Present { concealed, countries }`: `concealed=true ∨ x = concealed=true` (concealed wins; bare-FGI fallback) and `{countries: A} ∨ {countries: B} = {countries: A ∪ B}` (acknowledged trigraph union). T108d collapses to **doc-comment amendment only** — no new primitive. The bridge §4.8 framing reproduces the existing `FgiSet` semantics; the doc comment at `crates/capco/src/lattice.rs` should cite §4.8 and CAPCO §H.7 as the formal name for what the implementation already does. | PR 3.7 implementer (doc update only) |
| **Q-3.4.5a/b/c** — RELOPT auto-collapse step bound, monotone projection, EYES-alone historical recognition | **Defer to PR 5+.** Auto-collapse (b) is a renderer feature; not in scope before the renderer trait. Round-trip (a) is a parser correctness obligation tracked separately. | PR 5+ implementer |
| **Q-6.5** — Phase-D/E `joint-promotion` and `fgi-absorption` formal monotonicity verification | **Yes, verify per-rewrite in the PR description.** Even though the scheduler doesn't *require* monotonicity, verifying it unlocks confluence and reorderability. | Phase-D/E implementer |
| **Q-3.4.2 family-predicate engine PR scope** | Resolved as T108b; landing alongside T108c keeps the lattice-spike PR coherent. | — |

## 4. Staging (the executable plan)

The full staging table lives in `plan.md` D13 addendum. Summary
here for the dated record (re-baselined 2026-05-07; numeric band
retired in favor of qualitative per-sub-PR gates):

| Stage | PR | Deliverables | Expected surviving rules |
|---|---|---|---|
| Pre-collapse | — | 59 `impl Rule` blocks across `rules.rs:35` + `rules_declarative.rs:14` + `rules_sci_per_system.rs:10` | **59** |
| **Stage 1 (PR 3b proper)** | PR 3b (T026a–T026f) | 3b.A banner walker (3 → 1, net −2), 3b.B 7 PageRewrites (mostly additive, ~−1 to −3), 3b.C 4 RELIDO Conflicts rows (single-token RHS, all directly §-cited; broader §3.4.2 family roster deferred to PR 3.7 T108b under Constitution VIII), 3b.D ~25 class-floor Requires rows (mostly additive, ~0 to −2), 3b.E SCI per-system walker (10 → 1, net −9), 3b.F renderer fallback walker (4 → 1, net −3) | **~38–44** |
| **Stage 2 (new primitives + catalog compaction)** | PR 3.7 (T108b, T108c, T108d) | RhsFamily Conflicts variant (RELIDO compacts to 2 family rows: −13 to −18), closure operator primitive (~−5 to −8 implication-shaped Requires entries flip to closure entries), FGI consensus verification (doc-comment only, 0 delta) | **~32–40** |
| **Stage 3 (lattice impls + closure wiring)** | PR 4 (T111+) | Per-category Lattice impls retire entire walker classes (banner walker → property-test-only; SCI per-system walker → per-category Lattice impls), closure-implied Requires entries flip into the closure operator, RELIDO compaction completes | **~14–22** |
| **Stage 4 (renderer + RELOPT round-trip)** | PR 5+ | Renderer trait surface absorbs E020 / E023 / E028 / E033; RELOPT round-trip parser obligation lands; 3b.F walker retires | **~10** |

**The PR-3b-proper numeric band is retired.** The literal sub-move
retirements deliver −15 to −21 rules across 3b.A–3b.F, landing at
~38–44. The earlier "13–18" Stage-1 figure was an aspirational
projection that assumed aggressive walker-style consolidation beyond
what the authorized primitives in 3b.A–3b.F permit. Rather than
relax the primitives' scope or shed declarative-catalog discipline,
the band itself retires. Per-sub-PR gate becomes: **drive the count
down within what the sub-move's authorized primitive scope permits**.
End-state target ~10 surviving rules across all four stages stays
binding; Stage 3 (PR 4 per-category Lattice impls) and Stage 4
(PR 5+ renderer) carry the heavy lifting toward that target.

## 5. Bridge corrections committed

The bridge (`.claude/skills/marque-lattice-consultant/references/
marque-applied.md`) absorbed five corrections from the consultation:

1. §3.10.3 retitled and re-baselined: "the path from **59 to
   ~10** across PRs 3b / 3.7 / 4 / 5+" with per-move PR-home tags.
2. §3.10.1 "abhors-company" row split into RELIDO family conflicts
   (already counted in §3.4.2) + class-conditional drops (folded
   into §3.4.1 PageRewrite roster).
3. §3.4.4 indestructibility marked as **a property of the algebra,
   not a job that adds rules**.
4. §3.4.5 (b) RELOPT auto-collapse explicitly deferred from PR 3b
   scope.
5. §4.7.6 trait shape pinned to α (`MarkingScheme` trait method) with
   default no-op; `ImplTable` as `&'static [ImplRow<S>]`.
   **Superseded 2026-05-11 by `decisions.md` D18**: the `ImplTable<S>`
   / `ImplRow<S>` private shape is retired in favor of Option C —
   a public `ClosureRule` catalog (sibling to `Constraint`) accessed
   via `MarkingScheme::closure_rules() -> &[ClosureRule]`, with a
   default `closure()` impl walking the catalog. `triggers` /
   `suppressors` are `&'static [TokenRef]` (n-ary OR), not
   `fn`-pointer predicate bodies. The trait-method-with-default-no-op
   pin from this verdict still holds; only the private-structure
   shape was retired.

The bridge's §3.11 (new) is the stage-sequencing section that maps
each Move to its home PR.

## 6. Cross-references

- `specs/006-engine-rule-refactor/plan.md` — D13 addendum (acceptance criteria)
- `specs/006-engine-rule-refactor/decisions.md` — D13 amendment 2026-05-07 (decision register)
- `specs/006-engine-rule-refactor/tasks.md` — T026a–T026f (PR 3b sub-moves), T108b–T108d (PR 3.7 primitives)
- `docs/plans/2026-05-01-lattice-design.md` — §10 (open items 9, 10 added; 3.7 fill-in scope expanded)
- `docs/plans/2026-05-02-engine-refactor-consolidated.md` — source plan; references PR 3b but the count target (~10–13) is superseded by this verdict's qualitative per-sub-PR gating (PR-3b numeric band retired 2026-05-07; end-state target ~10 surviving rules across stages 1–4 stays binding)
- `.claude/skills/marque-lattice-consultant/references/marque-applied.md` — algebraic justification (§3 PR 3b walkthrough, §3.11 stage sequencing, §4.7 closure operator, §4.8 FGI consensus lattice)

## 7. Audibles

- PR 3b reviewer MAY skip T026a (banner walker) if PR 4 reviewer
  accepts property-test-only coverage of the banner-equality
  invariant. Documented as a 3b reviewer choice in the PR
  description.
- PR 3b implementer MAY land T026a–T026f as separate sub-PRs
  (e.g., 3b1 / 3b2 / 3b3) if review bandwidth requires it; each
  sub-PR runs T029 CI matrix independently.
- PR 3.7 implementer T108d work: confirm against §4.8.5 worked
  example (resolved 2026-05-07: existing `FgiSet { concealed,
  countries }` already models §4.8) and update the doc comment at
  `crates/capco/src/lattice.rs` to cite `marque-applied.md` §4.8 +
  CAPCO §H.7 / §H.7 p123. **No new primitive on the table** —
  Q-FgiSet-vs-§4.8 is closed. This audible was originally framed as
  "MAY discover and skip" before user confirmation 2026-05-07; that
  hypothetical phrasing is superseded.

Audibles beyond these require an amendment to this verdict doc
(append a §8 with the dated audible) and a follow-up commit; do not
silently drift from the locked staging.
