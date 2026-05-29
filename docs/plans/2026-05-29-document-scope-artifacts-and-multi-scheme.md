# Document-Scope Artifacts & Multi-Scheme Co-Residence

> **Status (2026-05-29)**: Design memo — pre-refactor framing for the engine
> refactor that lands document context (#799). Captures a working session on
> how core/engine should model heterogeneous document-level marking artifacts
> (CAB, declassify-on, notices, non-CAPCO caveats) and how a second grammar
> (CUI) co-resides with CAPCO in one document. No code landed; this records the
> shape decisions, not the implementation plan. Companion to RFC #641 (CAPCO
> coupling in domain-neutral infrastructure) and RFC #799 (resolution
> architecture). Spawned issues #823 (ICD 206 source-list generation) and
> #824 (reversible applied fixes). CUI-specific claims here are
> **source-pending** per Constitution VIII — they are resolved authoritatively
> when the CUI grammar is implemented and the governing policies are held.

## Context

The Classification Authority Block (CAB) is currently modeled as *part of a
marking* — a `MarkingType::Cab` candidate whose `parse_cab` output rides the
same `ParsedAttrs` / `CanonicalAttrs` struct as a portion marking, tagged by a
kind discriminant. This is the wrong shape, and it generalizes to a wider
problem: a document carries several **document-scoped artifacts** that are not
portion/banner markings — the CAB, the `Declassify On` value, document-level
notice/warning statements, and non-CAPCO "second banner line" caveats (#128).
They share a buffer and (sometimes) a grammar with the markings, but not a
structure, a scope, or a detection signature. The current approach equates
them; detecting *missing* artifacts (#420) and *deriving* artifacts from
markings (#266) both need them modeled as distinct types with their own
detection and a dependency graph between them.

This memo is the intersection of several open issues that are all part of the
same refactor:

| Boundary | Issues | Generalizes |
|----------|--------|-------------|
| **Input** — how facts arrive | #643 / #641, #176 | Scanner is one of four adapters: raw-text-recognized, structured-field, structure-read (ISM XML), hybrid-layered |
| **Scope** — where facts compose | #799 | portion → page → **document** → bundle, forward + reverse, "derivations need records" |
| **Output / mode** — how facts render & under what policy | #645, #643 `Translate` | severity cap, per-artifact scope targeting, deployment context, era/archival, schema-to-schema emit |
| *(plumbing)* — corpus per grammar | #640 | per-grammar fixtures/priors so document-level artifacts get test corpora |

The document-scoped artifacts are the internal structure of the **document-scope
layer** that sits in the middle: its inputs come from the input boundary, its
outputs are gated by the mode boundary.

## What is actually bolted on (evidence)

- **CAB rides the marking pivot type.** `parse_cab` (`crates/core/src/parser.rs`)
  produces a `CanonicalAttrs` with CAB-only fields (`classified_by`,
  `derived_from`, `declass_exemption`, `declassify_on`, `token_spans`) on the
  same struct as a portion marking. The tell is in
  `crates/ism/src/projected.rs`: `ProjectedMarking` **explicitly nulls those
  fields out** with a comment that a projected marking is "a page aggregate,
  not a CAB." When a type's projection is defined by which fields it drops,
  there are two types wearing one struct.
- **`Scope::Document` already exists** (`crates/scheme/src/scope.rs`), alongside
  `Portion`, `Page`, `Diff`. The projection target is present; what #799 adds is
  the document-level *context aggregation* (the analogue of `PageContext`) that
  nothing currently populates.
- **Two half-built pieces of the dependency machinery already exist**: the
  topological `PageRewrite` scheduler (`crates/engine/src/scheduler.rs`, a
  *static* reads/writes DAG with cycle rejection at `Engine::new`) and the
  `DecisionSink` cascade (`crates/scheme/src/decision.rs`, *runtime*
  `DecisionEvent::triggered_by` derivation edges, content-ignorant,
  feature-gated). Neither is load-bearing for derivation yet; the shapes are
  already there.

## Reframe: marking instance vs. document artifact

"Marking" is overloaded into two things that share a buffer and a scheme but
nothing else:

| | Marking instance (today) | Document artifact (CAB, notice, caveat) |
|---|---|---|
| Scope | portion / page | document |
| Structure | CVE-token grammar | its own grammar (line-structured, prose, foreign) |
| Composition | lattice rollup | derived / required / passthrough |
| Detection | present-parse | present-parse **or** absence-detect **or** fuzzy-decode |

The user's trichotomy — artifacts that are *codependent / potentially
codependent / unrelated* — is precisely **the edge set of a derivation DAG**
over scope-tagged facts: unrelated = no edge; codependent = static edge;
potentially codependent = a **conditional** edge that fires only when a
triggering fact is present (e.g. a notice required iff marking X appears
anywhere — #420 / #128).

## The three planes

| Plane | Owns | Issues | Key property |
|-------|------|--------|--------------|
| **Acquisition** | `InputAdapter` → facts tagged with *recognition provenance* | #643 / #641 / #176 | Scanner is one of four adapters; provenance licenses fix-assertiveness |
| **Derivation** | document-scope fact DAG; nodes carry *value derivation*; the cascade is the reversible log | #799, #266, #420 | static topology (cycle-checked at `Engine::new`), mode-gated firing, runs forward *and* as a check |
| **Emission** | render canonical → many targets under mode policy; `Translate` and `Co-reside` | #645, #643, #128 | co-residence forces a scheme-*set* container, not mono-`S` |

## Two provenance axes (keep orthogonal)

A single "provenance" concept conflates two independent questions. Keeping them
separate is the spine of the design.

- **Recognition provenance** (adapter property) — *"how sure am I this span
  actually **is** this node?"* Structure-read (ISM XML attribute) = certain;
  prose = ambiguous. This licenses **fix-assertiveness on existing text** and is
  exactly #176 / #643's `InputSource`.
- **Value derivation** (DAG-node property) — *"how was this node's **value**
  computed?"* max-over-source-dates, methodology (HUMINT → `50X1-HUM`), the
  §C.4 / §C.5 canned RD/FRD string, OCA-authored. This drives the **derivation
  record (cascade) and emit-if-absent**.

A node can be `derived` regardless of how its inputs were *recognized*, and
recognized at low confidence regardless of how its value is *derived*.

## The derivation DAG (document-scope layer)

Nodes are scope-tagged facts: marking-rollup, CAB (original | derivative),
declassify-on, derived-from / source-list, notice-requirement (per token
trigger), foreign-caveat layer. Edges are derivation / requirement relations.

- **Topology is static**, validated once at `Engine::new` (extends the existing
  `PageRewrite` scheduler — writers before readers, cycles rejected).
- **Edge firing is conditional**, including gated by deployment mode (#645): the
  historical "declassify code as trailing banner element" edge exists only in
  era/archival mode. Mode is a *firing predicate on an always-declared edge* —
  never a topology swap, which would defeat the construction-time cycle check.
- **Absence is a node state**, not a separate rule family: `Present(parsed)` |
  `AbsentButRequired` | `AbsentNotRequired` | `PresentNonCanonical`. The
  differing detection signatures live inside each node's recognizer; the graph
  stays uniform.
- **Fixability follows derivability.** An absent node with an inbound derivation
  edge can be filled (fix); one without can only be flagged:
  - missing **banner** → rollup edge from portions exists → **fix**
  - missing **notice** → canonical text known → **fix**
  - missing **portion mark** (#420) → no edge can invent content → **error / warn**
- **The cascade is the derivation log.** `DecisionEvent::triggered_by` records
  which edges fired, content-ignorant — this is #799's reversible derivation
  record. `AppliedFix` stays lean (per #146's recoverable-by-composition
  precedent); the cascade carries the derivation graph.

## CAB specifics

- **"Two CAB versions" are two derivation paths into one node type, not two
  structures.** Derivative CAB = source-derived (#823). Original CAB =
  OCA-authored `Classified By` / reason / `Declassify On`, *not* derived. Same
  envelope, different inbound edges — which is exactly why bolting it onto the
  marking struct was wrong.
- **`Declassify On` is its own node with several provenances**: structural field
  (ISM XML), derived-max over sources (#823), §C.4 / §C.5 canned AEA/NATO string
  (#266), historical trailing-banner code. One node, multiple inbound edges.
  `ProjectedMarking.declassify_on` is already a max-date rollup — the seed.
- **The richest derivation crosses bundle → document scope** (#799 bundle level,
  #823): a *set of source documents* → this document's `Derived From: Multiple
  Sources` + ICD 206 source list + `Declassify On`. The refactor should reserve
  this inbound edge even though the source-metadata adapter ships later.
- **The derivation plane runs generatively, not only for validation.** "Drag
  your sources here → CAB + source list generated" is the same edge evaluated
  *forward as authoring*; run as a check it validates an existing CAB against
  its sources. Derivation nodes need a forward-evaluable API, not just a
  comparator.

## Co-residence: CUI ∥ CAPCO (decision: model b)

CUI is its own scheme implementation (a future `marque-cui` peer alongside
`marque-ism`, per Constitution VII). When commingled with CAPCO it is *mostly*
handled like an unclassified-only control (FOUO / SBU): it does **not** propagate
to the classified banner. But a document with any CUI content still requires the
**CUI designation indicator block** (fields: `Controlled By`, `CUI Category`,
`LDC`, `POC`) even when every portion is classified — and that block is
structurally distinct from a CAB.

Two cross-scheme relationships exist, not one:

- **Translate (A → B)** — network boundary, directed, one scheme in / another
  out (#643's `Translate<CapcoScheme, DodScheme>`).
- **Co-reside (A ∥ B)** — CUI's block *and* the CAPCO CAB on the same document,
  both authoritative, neither derived from the other.

**The co-residence join is exactly one shared lattice axis: releasability /
foreign-disclosure.** The two scheme DAGs are otherwise disjoint.

- Document releasability = **meet (most-restrictive)** over both schemes'
  contributions to that axis.
- Each regime **renders its own projection** of that meet: the classified banner
  shows the IC-expressible floor (NOFORN / REL TO / RELIDO); the CUI block shows
  the precise `LDC` (FEDCON / FED ONLY / NOCON / DL ONLY / DISPLAY ONLY / …).
- The same attribute can appear on **both** surfaces — the CUI block is a
  **releasability escrow** that preserves what the classified banner strips.
  This is cleaner than the IC's transmutation (SBU-NF): the precise control is
  retained instead of lost.

Worked example: `CUI//FEDCON` + `C//RELIDO` → banner `CONFIDENTIAL//NOFORN` plus
a CUI block with `LDC: FEDCON`. FEDCON is non-IC → it floors the classified
side's foreign posture to NOFORN by the existing
`CLOSURE_NOFORN_NONICCONTROLS` principle; that NOFORN supersedes the portion's
RELIDO; FEDCON itself is escrowed verbatim in the CUI block's `LDC`. **The
cross-scheme edge reuses dissem-lattice machinery the engine already has** —
it is not new lattice, just one axis shared across two schemes.

Two unifications fall out:

1. **#128 ≡ the CUI `LDC` value set.** The "second banner line" caveats (NOCON,
   ATTORNEY-CLIENT, …) are the LDC values plus a few. #128 and CUI commingling
   are one modeling problem — the releasability-escrow surface — whether or not
   a full CUI block is present.
2. **The join is a lattice statement** (shared axis, meet, two projections) —
   worth validating against the lattice consultant before implementation.

## Unintentional commingling: `(S//CUI)`

This is *not* the clean co-residence case (which assumes the two grammars live
in **different portions**). `(S//CUI)` jams both into one portion — a category
error, since CUI is unclassified-only.

- The intra-scheme analog already exists: `(S//FOUO)` ("classification evicts
  FOUO", Pattern C). But **eviction is wrong for CUI**, because CUI has a
  document-scope home (the block) and eviction would lose it. The disposition is
  **relocate, not evict**:

  > Relocate, don't evict — and never let one scheme silently drop a token a
  > co-active scheme (or another scope) still owns.

- This exposes a gap in model (b): cross-scheme reconciliation cannot live only
  at the document/releasability join. There is a **portion-scope ownership
  check** too. Without it, CAPCO's existing banner/portion junk-recovery would
  swallow `CUI` as trailing junk — silent marking loss. The engine (the only
  crate allowed to know two schemes) must inspect any token a scheme *rejects*
  and ask whether a co-active scheme claims it before recovery eats it.
- Resolution flow: strict fails under both grammars → the decoder surfaces the
  token set `{S, CUI}` → the engine recognizes the set spans mutually-exclusive
  grammars → emits a portion-scope **cross-grammar conflict** (error), **no
  auto-fix**; the suggestion is structural (relocate the CUI signal to document
  scope), human-confirmed.
- The two provenance axes earn their keep here: from a structured input,
  recognition confidence is *high* (we are sure the bytes say `S//CUI`) while
  resolution confidence is *low* (we cannot know intent). Marque is **assertive
  that a contradiction exists, never assertive about the fix.**

## Reversibility (#824)

The safety anxiety behind "never auto-fix a contested resolution" is
*irreversibility*. The fix is to make applied fixes reversible by recording each
fix's **inverse** in terms the audit surface already permits (token canonicals,
category IDs, span offsets, BLAKE3 digests) — an additive `marque-3.x` schema
bump, not a content-ignorance violation.

- **Two reversal classes.** Token-level fixes (`NF → NOFORN`, recanonicalize,
  relocate) are **self-reversible from the audit log alone**. Free-form text
  corrections (`SERCET → SECRET`) cannot store the pre-text without breaking
  content-ignorance, so they are reversible only against the **caller's
  retained original** (Marque wipes the buffers it owns, Constitution II).
- **Derivations vs. substitutions.** Inverting a substitution is a token swap
  (#824); inverting a derivation (source-derived `Declassify On`, #823) is a
  recomputation recorded via the cascade. Two mechanisms, two kinds of change.
- **Mode-gated apply (#645 M3).** Reversibility turns "never auto-apply" into a
  *deployment-mode* decision: interactive editing may apply-and-rewind; the
  **network-boundary / egress** audit still blocks, because a rewind in the
  ledger does not un-transmit a document that already left with a wrong marking.
  The harm at egress is not in the log.

## What the refactor must honor now (scheme-agnostic)

CUI rules ship with CUI; these are the constraints the *imminent* refactor must
honor regardless, or they are expensive to add later:

1. **The document container is scheme-*set*-parameterized, not mono-`S`.** It
   holds scheme-tagged layers whose schemes may differ (CUI ∥ CAPCO).
2. **Cross-scheme reconciliation at two scopes** — portion-scope ownership
   routing (who claims this token; is this a cross-grammar mix) *and*
   document-scope releasability meet.
3. **Relocate-not-evict** as the disposition for a misplaced token with a home
   at another scope/surface; never silently discard a token a scheme does not
   own.
4. **Reserve the bundle-scope inbound edge** on the declassify-on /
   derived-from node (#823) even before the source-metadata adapter exists.
5. **Design for reversibility** — every `FixIntent` / `TextCorrection` variant
   carries enough pre-state to invert (#824).
6. **Keep `marque-scheme` domain-neutral** (Constitution VII): the shared
   releasability axis is reconciled in `marque-engine`'s document-scope layer
   (model b), not by pulling a cross-scheme concept into the leaf.

## Open items

- **(a) vs (b) for the releasability axis.** Decision: **(b)** — two per-scheme
  releasability lattices reconciled at the document node, keeping the leaf
  clean. (a) — a shared `Releasability` lattice constructor in `marque-scheme` —
  is a viable "cross-lattice" abstraction but risks accreting a near-domain
  concept. Validate the meet's laws (associativity / commutativity / idempotence
  across two schemes' contributions) with the lattice consultant before
  committing.
- **CUI source-gating** (NODIS/EXDIS/SBU → CUI subsumption; UCNI/DCNI homes) is
  resolved authoritatively at CUI implementation time against the governing
  unclassified policies. FOUO stays an IC token, distinct and non-convertible.

## Related

- RFC #641 — CAPCO coupling in domain-neutral infrastructure (parent)
- RFC #799 — resolution architecture; scope hierarchy; audit-recorded derivations
- #643 — input adapters / `StructuredDocument` / `Translate`
- #645 — mode taxonomy
- #640 — per-grammar corpus/fixtures
- #266 — canned `Declassify On` strings for AEA / NATO commingling
- #420 — detect missing portion marks and banners
- #128 — second-banner-line caveats outside CAPCO scope
- #176 — structured-field input source
- #823 — ICD 206 source-list generation (new)
- #824 — reversible applied fixes (new)
