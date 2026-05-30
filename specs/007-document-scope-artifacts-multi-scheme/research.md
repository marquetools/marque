# Research & Resolved Decisions

This file records the design decisions the feature rests on, each with its rationale and source.
Per Constitution VIII every resolved claim is traceable. Decisions are drawn from the design memo
(`docs/plans/2026-05-29-document-scope-artifacts-and-multi-scheme.md`), the #641 coupling
taxonomy, and a re-run of the marque-lattice-consultant skill (2026-05-30) for the releasability
question.

## D1 — Marking instance vs. document artifact are two types, not one

**Decision**: Split "marking" into the portion/page marking-instance (CVE-token grammar, lattice
rollup, present-parse detection) and the document artifact (its own grammar, derived/required/
passthrough composition, present-parse OR absence-detect OR fuzzy-decode detection).

**Evidence**: `crates/ism/src/projected.rs` defines `ProjectedMarking` by which CAB-only fields
it nulls (`classified_by`, `derived_from`, `declass_exemption`, `token_spans`) with the comment
"a projected marking is a page aggregate, not a CAB." A type whose projection is defined by the
fields it drops is two types in one struct. `parse_cab` (`crates/core/src/parser.rs`) emits a
`CanonicalAttrs` tagged `MarkingType::Cab` — the same struct as a portion.

**Rationale**: Detecting missing artifacts (#420) and deriving artifacts (#266/#823) both require
the artifact to be its own node with its own detection signature and a dependency relation to
markings. Keeping them fused makes both impossible to express cleanly.

## D2 — Absence is a node state, not a separate rule family

**Decision**: An artifact node carries one of `Present(parsed) | AbsentButRequired |
AbsentNotRequired | PresentNonCanonical`. The differing detection signatures (present-parse vs.
absence-detect vs. fuzzy-decode) live inside each node's recognizer; the graph stays uniform.

**Rationale (memo "The derivation DAG")**: A separate "missing-X" rule family would re-implement
the dependency logic per artifact. A node-state enum lets one derivation graph drive both
"derive the value" and "detect it's absent," and makes fixability follow derivability uniformly
(D4).

## D3 — Derivation is a static DAG; mode is a firing predicate, never a topology swap

**Decision**: The derivation topology is static, validated once at `Engine::new` by extending the
existing `marque-engine::scheduler` (Kahn's algorithm over `reads`/`writes`, writers before
readers, cycles rejected). Conditional edges (including deployment-mode-gated ones, e.g. the
historical "declassify code as trailing banner element" edge that exists only in era/archival
mode) are modeled as a **firing predicate on an always-declared edge**.

**Rationale**: A mode that swapped the topology would defeat the construction-time cycle check
(`crates/engine/src/scheduler.rs` rejects cycles at `Engine::new`). Keeping every edge declared
and gating only its firing preserves that guarantee. This matches the existing `PageRewrite`
predicate model and the `DecisionSink` cascade (`crates/scheme/src/decision.rs`,
`DecisionEvent::triggered_by`), which are already content-ignorant and feature-gated.

## D4 — Fixability follows derivability

**Decision**: An absent node with an inbound derivation edge can be filled (fix); one without can
only be flagged.

- missing **banner** → rollup edge from portions exists → **fix**
- missing **notice** → canonical text known → **fix**
- missing **portion mark** (#420) → no edge can invent content → **error / warn**

**Rationale (memo "Fixability follows derivability")**: This is the single rule that decides
fix-vs-flag for every artifact uniformly, replacing per-rule judgment calls.

## D5 — Two orthogonal provenance axes

**Decision**: Keep **recognition provenance** (adapter property: "how sure am I this span *is*
this node?" — structure-read certain, prose ambiguous; licenses fix-assertiveness; this is #176's
`InputSource`) separate from **value derivation** (DAG-node property: "how was this node's *value*
computed?" — max-over-source-dates, methodology, canned §C.4/§C.5 string, OCA-authored; drives the
derivation record and emit-if-absent).

**Rationale (memo "Two provenance axes")**: A node can be `derived` regardless of how its inputs
were recognized, and recognized at low confidence regardless of how its value is derived.
Conflating them into one "provenance" scalar loses the ability to be *assertive that a
contradiction exists while non-assertive about the fix* — the property the `(S//CUI)` case needs.

## D6 — Releasability: `Product` + monotone closure, not a shared meet *(lattice-consultant verdict, 2026-05-30)*

**Decision**: The CUI ∥ CAPCO releasability relationship is
`Product<CuiReleasability, CapcoIcDissem>` joined **componentwise**, plus a **monotone
cross-component closure** that injects NOFORN into the IC component when the CUI component carries
a non-IC control (FEDCON ⇒ NOFORN). Each regime renders its own projection (the classified banner
shows the IC-expressible floor; the CUI block shows the precise `LDC`). The single-shared-
`DissemSet`-reconciled-by-meet alternative is **rejected**.

**Verdict: (a) exact match.** Re-confirmed via the marque-lattice-consultant skill (2026-05-30):

| Claim | Verdict | Citation |
|-------|---------|----------|
| Combine is a **join** (LUB / most-restrictive floor), not a meet | ✓ | marque orients restriction upward; `IntersectSet`/`DissemSet` are join-oriented and deliberately join-only — a literal `meet` is uncallable. `marque-applied.md` §IntersectSet; `security-lattice.md` §6–§7 (supersession / intersection-with-blackball). |
| Componentwise join is lawful for free | ✓ | `pure-lattice.md` §11 (product lattice: lattice iff each factor is, ops coordinatewise). `Product<A,B>` already ships (`crates/scheme/src/builtins.rs`). |
| FEDCON⇒NOFORN is a **cross-axis closure**, obligation = monotonicity | ✓ | `marque-applied.md` §4.7 (closure operator: monotone + extensive + idempotent, adds facts across axes; matches `pure-lattice.md` §18). It is the `CLOSURE_NOFORN_NONICCONTROLS` precedent lifted across schemes. Closure is additive (injects NOFORN, never removes) → preserves Kleene-fixpoint monotonicity. NOT a meet/join; carries no meet-laws. |
| Escrow forbids collapse to one scalar | ✓ | Banner side is lossy (NOFORN can't reconstruct which non-IC control produced it). `Product` *forces* two retained components. A shared `DissemSet` would also require one `&'static` supersession table spanning both vocabularies — `SupersessionSet::join`'s table-consistency precondition — a standing cross-scheme coupling hazard. |
| Product + closure are existing leaf primitives → leaf stays domain-neutral | ✓ | Both are in `marque-scheme::builtins`/§4.7. Cross-scheme wiring lives in `marque-engine` (model b). |

**Worked example (memo)**: `CUI//FEDCON` + `C//RELIDO` → banner `CONFIDENTIAL//NOFORN` + CUI block
`LDC: FEDCON`. FEDCON is non-IC → cross-component closure floors the IC foreign posture to NOFORN;
that NOFORN supersedes the portion's RELIDO under the IC dissem join; FEDCON is escrowed verbatim
in the CUI block. Authority for the IC-side mechanics: CAPCO-2016 §H.8 (RELIDO/NOFORN supersession)
and §H.7 (REL TO / NATO worked examples). **The CUI-side mapping (which controls are "non-IC",
the LDC value ordering) is source-pending** until the CUI grammar's governing policy is held.

**Open question for the user (carried)**: `CuiReleasability`'s internal lattice shape (LDC value
ordering) is source-pending — flagged in `data-model.md`, not fixed now.

## D7 — Model (b): reconciliation in `marque-engine`, leaf stays domain-neutral

**Decision**: Two per-scheme releasability lattices reconciled at the document node in
`marque-engine`. Reject model (a) (a shared `Releasability` constructor in `marque-scheme`).

**Rationale (memo "Open items (a) vs (b)")**: (a) risks accreting a near-domain concept into the
leaf. The engine is the only crate allowed to know two schemes (Constitution VII). `Product` +
closure are already leaf primitives, so model (b) needs no new leaf algebra.

## D8 — Relocate, don't evict

**Decision**: A misplaced token with a home at another scope/surface is relocated, not evicted;
no scheme silently drops a token a co-active scheme (or another scope) still owns. The engine must
inspect any token a scheme *rejects* and ask whether a co-active scheme claims it before
junk-recovery consumes it.

**Rationale (memo "Unintentional commingling")**: The intra-scheme `(S//FOUO)` eviction
("classification evicts FOUO") is wrong for CUI because CUI has a document-scope home (the block);
eviction would lose it. Without the portion-scope ownership check, CAPCO's existing banner/portion
junk-recovery would swallow `CUI` as trailing junk — silent marking loss.

**`(S//CUI)` resolution flow**: strict fails under both grammars → decoder surfaces token set
`{S, CUI}` → engine recognizes mutually-exclusive grammars → emits a portion-scope cross-grammar
conflict (error), **no auto-fix**; the suggestion is structural (relocate CUI to document scope),
human-confirmed. Recognition confidence is *high* (we are sure the bytes say `S//CUI`); resolution
confidence is *low* (we cannot know intent). Marque is **assertive that a contradiction exists,
never assertive about the fix** — D5's two axes earn their keep here.

## D9 — Reversibility classes and the deferred boundary (#824)

**Decision (deferred realization; Phase-0 reserves the fields)**: Two reversal classes.
1. **Token-level fixes** (`NF → NOFORN`, recanonicalize, relocate) are self-reversible from the
   audit log alone — canonical tokens are on the G13 allow-list and are stored.
2. **Free-form text corrections** (`SERCET → SECRET`) cannot store the pre-text without breaking
   content-ignorance; reversible only against the caller's retained original (Constitution II:
   Marque wipes the buffers it owns).

Inverting a **substitution** is a token swap (#824); inverting a **derivation** (source-derived
`Declassify On`, #823) is a recomputation recorded via the cascade — two mechanisms, two kinds of
change. Realization is an additive `marque-3.x` audit-schema bump that preserves the G13 canary
(`crates/engine/tests/audit_g13_canary.rs`). **Mode-gated apply (#645 M3)**: interactive editing
may apply-and-rewind; network-boundary/egress audit still blocks — a ledger rewind does not
un-transmit a document that already left with a wrong marking.

**Rationale**: The fields must be reserved now (Phase 0) so the later bump is additive, not
breaking. `ReplacementIntent::FactRemove` already carries the facts it removes (the inverse of an
add); `FactAdd` carries what it added; `Recanonicalize` and the new relocate disposition do not
yet carry pre-state — those gain reserved fields in Phase 0.

## D10 — `#641` tier triage → phase mapping

The #641 audit found CAPCO coupling at four severity tiers. This feature maps them as follows
(detail in `data-model.md`):

| Tier | Items | Phase |
|------|-------|-------|
| T1 (architecture blockers) | T1-1/T1-2 `Rule::check`/`RuleContext` generification; T1-3 `Engine<S>`; T1-4 constraint→rule-id delegation; T1-5/T1-6 scan/parse strategy; T1-7 `Translate`/`CoherenceRule`; T1-8 `InputAdapter` | B (T1-1..6), A (T1-8), E (T1-7) |
| T2 (structural friction) | `LintResult`/`FixResult`, `Sink`, `MessageTemplate`/`FeatureId` `#[non_exhaustive]`+`Grammar`, defaults move, decoder citation | B |
| T3 (naming coupling) | `Zone::Cab`→`Custom`/`StructuralBlock`, `classification_floor`→`rank_floor`, `OwnerProducerKind`/`FormSet`/`FormKind`/`EmissionForm` renames, `render_portion`/`render_banner`→`render_item`/`render_summary`, `is_fdr_dissem`→`IcMarkingVocabulary` sub-trait | B |
| T4 (entry/config) | `Config.grammar_schema`, CLI/server/WASM grammar registration, health schema version, citation helper relocation | B/F |

**Note**: T3 renames are additive-with-deprecation where possible to avoid a flag-day break; the
stable-API surface (CLAUDE.md) requires a coordinated audit-schema consideration for any audit-side
change, which the renames here avoid (they are recognizer/render-side, not audit-record-side).

## D11 — `Scope::Bundle` is an additive enum variant (not `#[non_exhaustive]`)

**Decision**: Add a `Bundle` variant to `Scope` (`crates/scheme/src/scope.rs`).

**Evidence**: `Scope` is deliberately *not* `#[non_exhaustive]` — its doc comment states "the
variant set is fixed by the design doc; scheme authors don't introduce new scopes... a future
scheme needing a new scope adds the variant via a minor-version bump." `RecanonScope` already has
`Document`. Adding `Bundle` is exactly the anticipated minor-version additive variant; exhaustive
matchers see it at compile time. This reserves the #823 bundle→document derivation edge.

## Sources consulted

- `docs/plans/2026-05-29-document-scope-artifacts-and-multi-scheme.md` (design memo).
- Issues #799, #641, #643, #645, #640, #266, #420, #128, #176, #823, #824.
- `crates/capco/docs/CAPCO-2016.md` — §C.4 p33 (RD/FRD/TFNI canned string, verified line 683),
  §C.5 p33 (NATO canned string, verified line 687), §H.7/§H.8 (releasability worked examples).
- marque-lattice-consultant skill references: `pure-lattice.md` §11/§18, `security-lattice.md`
  §6/§7, `marque-applied.md` §4.7/§IntersectSet/§SupersessionSet (2026-05-30 consultation).
- Current code seams: `crates/ism/src/projected.rs`, `crates/scheme/src/scope.rs`,
  `crates/scheme/src/recognizer.rs`, `crates/scheme/src/decision.rs`,
  `crates/scheme/src/fix_intent.rs`, `crates/engine/src/scheduler.rs`,
  `crates/engine/src/engine/page_context.rs`, `crates/core/src/parser.rs`.
