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

**Decision**: An artifact node carries one of `Present(parsed) | PresentNonCanonical |
PresentNotRequired | AbsentButRequired | AbsentNotRequired`. The fifth state
(`PresentNotRequired`) completes the presence × requirement product (cross-ref D12/LV1). The
differing detection signatures (present-parse vs. absence-detect vs. fuzzy-decode) live inside
each node's recognizer; the graph stays uniform.

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
computed?" — max-over-source-dates, methodology, canned §E.4/§E.5 string, OCA-authored; drives the
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
in the CUI block. Authority for the IC-side mechanics: CAPCO-2016 §H.8 (Dissemination Control
Markings — REL TO p150, RELIDO p154, NOFORN p145, supersession). **The CUI-side mapping (which
controls are "non-IC", the LDC value ordering) is source-pending** until the CUI grammar's
governing policy is held.

**Boundedness (LV4, 2026-05-30 re-consult)**: `Product<CuiReleasability, CapcoIcDissem>`
implements `JoinSemilattice` only — plus `BoundedJoinSemilattice` iff both factors have a
bottom (they do: the empty release-set bottom, `SciSet::empty` precedent). It MUST NOT implement
`BoundedLattice`/`BoundedMeetSemilattice`: a `Product` has a top iff *both* factors do, and
`CuiReleasability`'s LDC set is agency-extensible/open (no lawful finite top), exactly as
`SciSet`/`SarSet` are (CLAUDE.md SCI-canonical-storage note). The monotone NOFORN closure is
additive and finite-height (injects NOFORN once, never removes) so it reaches a Kleene fixpoint
without requiring a complete lattice or a top — `CLOSURE_NOFORN_NONICCONTROLS` already runs on a
no-top open set. **No lattice-trait-surface change**: the existing
`JoinSemilattice`/`MeetSemilattice` split (PR #502) and `Product` constructor cover this.

**Phase-E landing (FR-026)**: this construction is validated in Phase E against a **synthetic
test-only `StubScheme`** (an invented non-IC control), NOT a real CUI grammar. The worked
example's CUI side stays source-pending; encoding an unverified FEDCON⇒NOFORN fixture in a
passing test would violate Constitution VIII.

**Open question for the user (carried)**: `CuiReleasability`'s internal lattice shape (LDC value
ordering) is source-pending — flagged in `data-model.md`, not fixed now.

## D7 — Model (b): reconciliation in `marque-engine`, leaf stays domain-neutral

**Decision**: Two per-scheme releasability lattices reconciled at the document node in
`marque-engine`. Reject model (a) (a shared `Releasability` constructor in `marque-scheme`).

**Rationale (memo "Open items (a) vs (b)")**: (a) risks accreting a near-domain concept into the
leaf. The engine is the only crate allowed to know two schemes (Constitution VII). `Product` +
closure are already leaf primitives, so model (b) needs no new leaf algebra.

**`Translate` is cut from this feature.** Model (b) reconciles two per-scheme lattices at the
document node via `Product`+closure — it never translates one scheme's canonical into the other's.
A `Translate<A, B>` trait therefore has no consumer in co-residence; its only real uses
(cross-system marking translation and ISM→DoD XML round-trips) are deferred. Shipping it now would
be speculative surface (YAGNI; project policy on pre-users rewrite-freely). Co-residence keeps only
`CoherenceRule`. `Translate` lands with the deferred cross-system path, **tracked as #829**
(blocker for ISM→DoD XML: `InputAdapter::adapt → canonical → Translate → Codec::encode`).

## D8 — Relocate, don't evict

**Decision**: A misplaced token with a home at another scope/surface is relocated, not evicted;
no scheme silently drops a token a co-active scheme (or another scope) still owns. The engine must
inspect any token a scheme *rejects* and ask whether a co-active scheme claims it before
junk-recovery consumes it.

**Rationale (memo "Unintentional commingling")**: The intra-scheme `(S//FOUO)` eviction
("classification evicts FOUO") is wrong for CUI because CUI has a document-scope home (the block);
eviction would lose it. Without the portion-scope ownership check, CAPCO's existing banner/portion
junk-recovery would swallow `CUI` as trailing junk — silent marking loss.

**Contention precedence (FR-023)**: when ≥2 schemes are co-active and a token is rejected by the
owning scheme, the engine routes by acceptance count, not by registration order: exactly-one
acceptor → route there; two-or-more acceptors → cross-grammar conflict (the mutually-exclusive
case); zero acceptors → junk-recovery at unchanged `DocumentContent` confidence. Registration
order MUST NOT tie-break — a registration-order tie-break makes resolution non-deterministic
across callers (CLI/server/WASM register schemes in different orders). Ties surface as conflicts.

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
| T1 (architecture blockers) | T1-1/T1-2 `Rule::check`/`RuleContext` generification; T1-3 `Engine<S>`; T1-4 constraint→rule-id delegation; T1-5/T1-6 scan/parse strategy; T1-7 `CoherenceRule` (the `Translate` half of T1-7 is **cut → #829**); T1-8 `InputAdapter` | B (T1-1..6), A (T1-8), E (T1-7) |
| T2 (structural friction) | `LintResult`/`FixResult`, `Sink`, `MessageTemplate`/`FeatureId` `#[non_exhaustive]`+`Grammar`, defaults move, decoder citation | B |
| T3 (naming coupling) | `Zone::Cab`→`Custom`, `classification_floor`→`rank_floor`, `OwnerProducerKind`/`FormSet`/`FormKind`/`EmissionForm` renames, `render_portion`/`render_banner`→`render_item`/`render_summary`, `is_fdr_dissem`→`IcMarkingVocabulary` sub-trait | B |
| T4 (entry/config) | `Config.grammar_schema`, CLI/server/WASM grammar registration, health schema version, citation helper relocation | B/F |

**Note (rewrite-freely posture)**: marque is pre-users, so T3 renames are **straight breaking
renames** — no deprecation shims, no retained aliases (project policy: rewrite freely). The two
surfaces that stay stable regardless are (1) the **audit-record schema** (`MARQUE_AUDIT_SCHEMA`,
a committed stable surface per CLAUDE.md — any audit-side change is a coordinated additive bump),
and (2) the **lattice trait surface** (`JoinSemilattice`/`MeetSemilattice`/`Bounded*`/`Product`
et al. — kept stable by user direction; this feature introduces no lattice-trait change, see
D12). The T3 renames are recognizer/render-side, touch neither stable surface, and so land as
plain breaking edits in the single Phase-0/B breaking window (D13).

## D11 — `Scope::Bundle` is an additive enum variant (not `#[non_exhaustive]`)

**Decision**: Add a `Bundle` variant to `Scope` (`crates/scheme/src/scope.rs`).

**Evidence**: `Scope` is deliberately *not* `#[non_exhaustive]` — its doc comment states "the
variant set is fixed by the design doc; scheme authors don't introduce new scopes... a future
scheme needing a new scope adds the variant via a minor-version bump." `RecanonScope` already has
`Document`. Adding `Bundle` is exactly the anticipated minor-version additive variant; exhaustive
matchers see it at compile time. This reserves the #823 bundle→document derivation edge.

## D12 — Lattice verdicts on the new constructions *(lattice-consultant re-consult, 2026-05-30)*

Re-ran the marque-lattice-consultant framework against the constructions this feature adds.
**Headline: no lattice-trait-surface change is required** — every construction is covered by the
existing constructors (`OrdMax`/`MaxDate`, `FlatSet`, `IntersectSet`, `SupersessionSet`,
`Product`, the `JoinSemilattice`/`MeetSemilattice` split). Four verdicts:

- **LV1 — `ArtifactState` is a status enum, not a lattice.** It is the product of two orthogonal
  axes — *presence* (absent / present-canonical / present-non-canonical) × *requirement*
  (required / not-required). A node has exactly one state, produced by one recognizer + the
  requirement check; states of the same node are never joined, so there is no meet/join to
  define. The fifth state `PresentNotRequired` fills the present×not-required cell that the
  four-state model omitted — adding it *completes* the product rather than introducing a new
  algebra. The requirement axis itself is a trivial 2-element boolean join (required iff *any*
  inbound Requirement edge fires); lawful, no surface. **Verdict: (c) not a lattice problem —
  and the fifth state is the right fix.**

- **LV2 — the `Declassify On` value is a single-valued chain `OrdMax<DeclassInstruction>`, not a
  `Product`.** CAPCO-2016 §E.3 p32 is explicit: "Only a single value must be used on the
  'Declassify On' line." §E.4/§E.5 p33 say the commingling N/A string *replaces* any date or
  event — there is **one slot**, filled by **one total order**. The value space is therefore the
  chain `OrdMax<DeclassInstruction>`, where `DeclassInstruction` is a *single* enum spanning the
  full §E.3 precedence hierarchy with a hand-written **TOTAL** `Ord` keyed lexicographically on
  `(tier 1–9, resolved-protection-date via IsmDate::end_cmp, lowest exemption number)`:
  - **bottom = `Unset`/absent** (the join identity);
  - **top = the single `Commingled` tier-1 point** ("N/A … see source list", no date), which
    "takes precedence over all" (§E.3 p32) — so the carrier implements `BoundedJoinSemilattice`
    (lawful here: a *closed finite* hierarchy with a genuine maximum, unlike the open
    `SciSet`/`SarSet` which have no top).

  §E.3 precedence runs most-restrictive / longest-protection first: (1) N/A-commingling [no date];
  (2) 50X1-HUM / 50X2-WMD [lowest number on tie]; (3) 50X1–50X9 with date/event [furthest date,
  then lowest number]; (4) 25X1, EO 12951; (5) 25X1–25X9 with date/event; (6) 25X1–25X9 without
  date [compute 50yr-from-source]; (7) specific date ≤25yr; (8) event <10yr; (9) calculated 25yr
  fallback.

  **The AEA-only / NATO-only / combined choice among the §E.4/§E.5 N/A strings is a RENDER
  concern**, keyed on the document's AEA-present / NATO-present flags (the planned T070 presence
  flags), NOT a sub-lattice inside `DeclassInstruction`. Tier 1 is the single `Commingled` lattice
  point; which exact canned string renders is downstream. This keeps the order total and `OrdMax`
  valid. The construction generalizes the existing date-only
  `crates/capco/src/lattice/declassify_on.rs` (`Option<IsmDate>` with `max_by(end_cmp)`) to the
  full 9-tier carrier.

  **Both earlier `Product` models are REJECTED as category errors:** (a)
  `Product<DeclassInstruction, CannedAnnotationSet>` (this draft's earlier text) and (b)
  `Product<MaxDate, FlatSet<ExemptionCode>>` (an audit suggestion). `Product::join` joins factors
  independently, which makes the *illegal* "a date and a canned string coexist" state
  representable; and `FlatSet` models accumulating incomparable atoms, but §E.3 exemptions
  **compete** in one total order while §E.4 says the canned string **replaces** the date. There is
  one slot, one total order — there is no separate `CannedAnnotationSet`/`FlatSet` axis. The
  earlier "a date and a canned string don't compete for one slot" claim is factually wrong and is
  removed.

  **Verdict: (a) exact match to `OrdMax` over a chain — a chain is automatically a distributive
  lattice; cite the marque-lattice-consultant re-consult (2026-05-30) and §E.3 p32 / §E.4 §E.5
  p33.** Test obligation: the structural "no join/meet impl exists" test does NOT catch a non-total
  or order-dependent `Ord`, so the implementer MUST prove `Ord` totality / antisymmetry /
  transitivity across all 9 tiers, prove `OrdMax` join idempotence / commutativity / associativity
  plus bottom-identity and top-absorption, and pin §E.3 worked-precedence oracle fixtures (e.g.
  `50X1-HUM ⊔ 25X-dated == 50X1-HUM`; `Commingled ⊔ anything == Commingled`), each citing its §E.3
  source.

- **LV3 — page→document rollup is lawful by associativity/commutativity/idempotence.**
  `DocumentContext` applies the same join ops as `PageContext` one level up: folding page-level
  joins into a document-level join is just a semilattice fold over a larger index set, so
  page-processing order and grouping do not matter. **Constraint to honor**: the document rollup
  MUST reuse the observational-state lattice types (`DissemSet` with `relido_observed_unanimous`,
  `JointSet`) rather than a naive re-union — otherwise RELIDO-unanimity (memory
  `relido-unanimity-banner-rollup`) and NOFORN-supersession would not survive the page→doc fold.
  **Verdict: (a) exact match, with the observational-types constraint flagged for the
  implementer.**

- **LV4 — releasability `Product` stays `JoinSemilattice`-only, never `BoundedLattice`.** See
  D6 "Boundedness" — `CuiReleasability` is agency-extensible/open (no top), so the `Product` has
  no top; the additive monotone NOFORN closure converges without one. Mirrors `SciSet`/`SarSet`.
  **Verdict: (a) lawful, with the no-`BoundedLattice` constraint stated.**

## D13 — Rewrite-freely posture; single Phase-0/B breaking window

**Decision**: marque is pre-users (project policy), so this feature does **not** use
deprecation shims, retained aliases, or "reserve-the-seam-now-so-the-later-bump-is-additive"
ceremony at the *source* level. Source-breaking changes (the `ReplacementIntent` edit + new
`Relocate` variant, the T3 renames, the `Rule<S>`/`Engine<S>` generification, removing CAB
fields from `CanonicalAttrs`) land as plain breaking edits in **one breaking window spanning
Phase 0 and Phase B**. Phase 0 is still the blocking foundation for ordering purposes; it simply
*is* a breaking window rather than pretending to be purely additive.

**Two exceptions stay stable**: (1) the **audit-record schema** — still a committed stable
surface (CLAUDE.md); #824 realization remains an additive `marque-3.x` bump, and the reserved
pre-state fields exist because the *audit* surface (not the source surface) must evolve
additively. (2) the **lattice trait surface** — kept stable by user direction; D12 confirms no
change is needed.

**Rationale**: aligns with the project's recorded stance (no deprecation phasing pre-users) and
removes the self-contradiction in the earlier draft, where the "additive" Phase-0 nonetheless
contained a source-breaking `ReplacementIntent` edit. The `#[non_exhaustive]` attributes that
remain are kept only where they buy genuine future-proofing for the *audit/stable* surfaces
(`MessageTemplate`/`FeatureId`), not as source-compat ceremony.

## Sources consulted

- `docs/plans/2026-05-29-document-scope-artifacts-and-multi-scheme.md` (design memo).
- Issues #799, #641, #643, #645, #640, #266, #420, #128, #176, #823, #824.
- `crates/capco/docs/CAPCO-2016.md` — §E.4 p33 (RD/FRD/TFNI canned string), §E.5 p33 (NATO canned
  string), §H.8 (Dissemination Control Markings — releasability).
- marque-lattice-consultant skill references: `pure-lattice.md` §11/§18, `security-lattice.md`
  §6/§7, `marque-applied.md` §4.7/§IntersectSet/§SupersessionSet (2026-05-30 consultation).
- Current code seams: `crates/ism/src/projected.rs`, `crates/scheme/src/scope.rs`,
  `crates/scheme/src/recognizer.rs`, `crates/scheme/src/decision.rs`,
  `crates/scheme/src/fix_intent.rs`, `crates/engine/src/scheduler.rs`,
  `crates/engine/src/engine/page_context.rs`, `crates/core/src/parser.rs`.
