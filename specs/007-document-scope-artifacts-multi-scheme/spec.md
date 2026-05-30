# Feature Specification: Document-Scope Artifacts & Multi-Scheme Co-Residence

**Feature Branch**: `007-document-scope-artifacts-multi-scheme`
**Created**: 2026-05-30
**Status**: Draft
**Input**: Design memo `docs/plans/2026-05-29-document-scope-artifacts-and-multi-scheme.md` + issues #799, #641, #643, #645, #640, #266, #420, #128, #176 (in scope); #823, #824 (deferred, roughed-in).

> **Provenance note (Constitution VIII).** CAPCO/ISM claims here trace to
> `crates/capco/docs/CAPCO-2016.md` and the vendored ODNI ISM schemas. **CUI-specific
> claims are source-pending** — they are resolved authoritatively when the CUI grammar is
> implemented and the governing NARA/ISOO policies are held. This mirrors the memo's own
> disclaimer and is repeated wherever a CUI claim appears.

## Overview

`marque` today models the Classification Authority Block (CAB) as *part of a marking* — a
`MarkingType::Cab` candidate whose `parse_cab` output rides the same
`ParsedAttrs`/`CanonicalAttrs` struct as a portion marking. The tell is in
`crates/ism/src/projected.rs`: `ProjectedMarking` explicitly nulls the CAB-only fields with
the comment "a projected marking is a page aggregate, not a CAB." When a type's projection is
defined by which fields it drops, two types are wearing one struct.

This is the narrow case of a general problem. A document carries several **document-scoped
artifacts** that are not portion/banner markings — the CAB, the `Declassify On` value,
document-level notice/warning statements, and non-CAPCO "second banner line" caveats. They
share a buffer (and sometimes a grammar) with the markings, but not a structure, a scope, or a
detection signature. Detecting *missing* artifacts (#420) and *deriving* artifacts from
markings (#266, #823) both require modeling them as distinct typed nodes with their own
detection and a dependency graph between them.

Layered on top: a second grammar (CUI) must co-reside with CAPCO on one document, with
cross-scheme reconciliation at two scopes. The infrastructure changes required for both
(generic `Engine`/`Rule`, input adapters, mode taxonomy, per-grammar corpus) are tracked in
#641/#643/#645/#640.

The feature is delivered as a **phased program** (see `plan.md`). Each user story below is an
independently shippable slice.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Document artifacts are distinct typed nodes (Priority: P1)

A classification reviewer runs `marque check` on a document with a CAB, a `Declassify On`
line, and a US-Person notice. Today the CAB is parsed onto the marking pivot type and the
notice/declassify-line are opaque text. The reviewer needs each artifact recognized as its
own typed thing, with a clear state: present-and-canonical, present-but-malformed,
absent-but-required, or absent-and-not-required.

**Why this priority**: This is the keystone. Every downstream story (derivation, CUI block,
canned strings, missing-mark detection) presupposes that document artifacts are first-class
nodes distinct from markings. Without it they keep accreting onto the marking struct.

**Independent Test**: Feed a document whose CAB is structurally present but whose
`Declassify On` is malformed; assert the engine reports the CAB node as `Present` and the
declassify node as `PresentNonCanonical`, distinctly from any portion-marking diagnostic.

**Acceptance Scenarios**:
1. **Given** a document with a well-formed CAB, **When** linted, **Then** the CAB artifact
   node reports `Present` and no longer occupies the marking pivot type's CAB-only fields.
2. **Given** a document missing a required notice, **When** linted, **Then** that artifact's
   node reports `AbsentButRequired` and a diagnostic fires; an artifact not required for the
   document's markings reports `AbsentNotRequired` and fires nothing.
3. **Given** a single-page document, **When** projected at `Scope::Document`, **Then** a
   `DocumentContext` aggregate exists (analogue of `PageContext`) carrying the artifact nodes.

---

### User Story 2 - Forward + reverse derivation at document scope (Priority: P1)

A reviewer has a document whose portions imply a banner and whose markings imply obligations
(e.g. an AEA portion requires a specific `Declassify On` string). They want the engine to
*resolve* what the document should be — always, even with fixing off — and to record each
derivation as an auditable decision, not a silent mutation (#799). Where an absent node has an
inbound derivation edge, the engine can fill it (fix); where it does not, the engine can only
flag it.

**Why this priority**: #799's core finding — resolutions realize at portion scope only where a
human hand-wrote a mirror rule, and page rewrites mutate silently with no audit trail. The
derivation DAG generalizes the existing `PageRewrite` scheduler and `DecisionSink` cascade into
the load-bearing mechanism.

**Independent Test**: Feed `(TS//SI-G//OC/RELIDO)` (an unwired edge per #799); assert a
diagnostic now fires at portion scope and the cascade records the derivation edge that fired,
content-ignorantly.

**Acceptance Scenarios**:
1. **Given** portions present and no banner, **When** resolved, **Then** the banner node is
   derived from the portions (inbound rollup edge) and is **fixable**.
2. **Given** a missing portion mark (#420), **When** resolved, **Then** no edge can invent the
   content, so the node is flagged (error/warn), not filled.
3. **Given** any derivation fires, **When** the cascade is inspected, **Then**
   `DecisionEvent::triggered_by` records which edge fired (content-ignorant).
4. **Given** a document "classified up to" front marking, **When** validated in reverse against
   all pages' markings, **Then** divergence is reported.

---

### User Story 3 - CUI co-resides with CAPCO on one document (Priority: P2)

A document has classified CAPCO portions and CUI-controlled content. The CUI designation block
(`Controlled By`/`CUI Category`/`LDC`/`POC`) is required even when every portion is classified,
and it is structurally distinct from a CAB. The releasability relationship between the two
schemes must combine correctly (e.g. `CUI//FEDCON` + `C//RELIDO` → banner `CONFIDENTIAL//NOFORN`
plus a CUI block with `LDC: FEDCON`). A token jammed into the wrong portion (`(S//CUI)`) must
be a flagged cross-grammar conflict — relocate-not-evict — never silently dropped.

> **Source-pending**: the CUI block field set, the LDC value ordering, and the FEDCON⇒NOFORN
> mapping are resolved at CUI-grammar implementation against NARA/ISOO policy.

**Why this priority**: This is the near-term driver behind #641's infrastructure work, but it
depends on the generic `Engine`/`Rule` surface (Story-4-adjacent) and the document-scope layer
(Story 2) landing first.

**Independent Test**: With both schemes registered, feed `CUI//FEDCON` + `C//RELIDO` in
distinct portions; assert the banner floors to `CONFIDENTIAL//NOFORN`, the CUI block escrows
`LDC: FEDCON`, and the RELIDO is superseded — no information silently lost.

**Acceptance Scenarios**:
1. **Given** CUI and CAPCO content in **different** portions, **When** resolved, **Then** the
   releasability combines as a componentwise join with a monotone NOFORN closure; each regime
   renders its own projection.
2. **Given** `(S//CUI)` (both grammars in **one** portion), **When** resolved, **Then** the
   engine emits a portion-scope cross-grammar conflict (error), **no auto-fix**; the suggestion
   is to relocate the CUI signal to document scope, human-confirmed.
3. **Given** a token a scheme rejects, **When** junk-recovery runs, **Then** the engine asks
   whether a co-active scheme claims it before recovery eats it (no silent marking loss).

---

### User Story 4 - Structured, schema-typed, and hybrid inputs (Priority: P2)

A caller has a marking value from a web-form field, an ISM XML attribute, or a hybrid
XML-wrapper-plus-text-body. Today every input is forced through the raw-text scanner, which
mis-calibrates confidence and cannot reach typed-attribute markings at all. The caller needs an
`InputAdapter` path that produces canonical form directly and a recognition-provenance signal
that licenses assertive fixes on inputs the caller asserts *are* markings (#643, #176).

**Why this priority**: Unblocks schema-to-schema translation and is a prerequisite for the
deferred #823 source-metadata ingestion. Independent of the document-scope layer but feeds it.

**Independent Test**: Feed the same `(YS)` value as `DocumentContent` vs. `StructuredField`;
assert the lone-case heuristic fires assertively only for `StructuredField` (#176 matrix).

**Acceptance Scenarios**:
1. **Given** a `SchemaDocument` input, **When** adapted, **Then** the scanner and recognizer are
   bypassed and canonical form is produced field-by-field.
2. **Given** a hybrid document, **When** adapted, **Then** a `StructuredDocument` with a
   metadata layer and a body layer is produced; cross-layer coherence is checkable; fixes carry
   the right `RepairKind` (schema-attribute vs. text-span).
3. **Given** `InputSource::StructuredField`, **When** an ambiguous lone marking is recognized,
   **Then** confidence calibration matches the #176 matrix.

---

### User Story 5 - Operational mode taxonomy (Priority: P3)

A SIEM/CI/network-boundary operator needs a bulk severity baseline (audit-only), zone/axis fix
targeting, a deployment-context defaults profile, and temporal/archival processing — without
enumerating every rule (#645).

**Why this priority**: Usability/safety layer above the per-rule severity primitive that
already exists. Independent of co-residence; interacts with reversibility (#824 M3).

**Independent Test**: Set `severity_cap = "suggest"`; assert every `Fix`-default rule is capped
to `Suggest` and no fix auto-applies, while per-rule overrides still win.

**Acceptance Scenarios**:
1. **Given** `[engine] severity_cap = "suggest"`, **When** fixing, **Then** no fix auto-applies
   and per-rule `[rules]` overrides still take precedence.
2. **Given** `fix_zones = ["body"]`, **When** fixing, **Then** banner/CAB-zone fixes are not
   promoted though diagnostics still emit for all zones.
3. **Given** `deployment = "archival"` + an `as_of` date + `ArchivalIntent::ValidateForEra`,
   **When** processing, **Then** rules postdating `as_of` are suppressed and no rewrites apply.

---

### User Story 6 - Concrete document-artifact rules (Priority: P3)

A reviewer processes a document commingling NSI with AEA (RD/FRD/TFNI) or NATO portions; the
CAB `Declassify On` line MUST carry the §E.4/§E.5 canned string. Separately, a reviewer needs
missing portion-marks and banners detected (#420). Both are concrete validators built on the
node-state + derivation model.

**Why this priority**: These are the first end-to-end proofs that the node model works, but they
depend on the CAB node (Story 1) and the derivation layer (Story 2).

**Independent Test**: Feed a document with an RD portion and a date-bearing `Declassify On`;
assert the canned-string rule fires and proposes the §E.4 string at high confidence.

**Acceptance Scenarios**:
1. **Given** any RD/FRD/TFNI portion, **When** linted, **Then** the `Declassify On` node is
   required to contain "N/A to [RD/FRD/TFNI, as appropriate] portions. See source list for NSI
   portions." (CAPCO-2016 §E.4 p33); a date/event triggers a fix proposal.
2. **Given** any NATO portion in a US-classified document, **When** linted, **Then** the
   `Declassify On` node is required to contain "N/A to NATO portions. See source list for NSI
   portions." (§E.5 p33).
3. **Given** a paragraph with no portion mark in an otherwise-marked document, **When** linted,
   **Then** a missing-portion-mark diagnostic fires (and is suppressed for an entirely-`(U)`
   document per #420).

---

### Edge Cases

- `(S//CUI)` (unintentional commingling): category error, relocate-not-evict, no auto-fix.
- Both AEA and NATO portions present: both §E.4 and §E.5 canned annotations apply (combined form
  per CAPCO-2016 §E.4 p33: "N/A to [RD/FRD/TFNI, as appropriate] [and NATO, if appropriate] portions. See source list for NSI portions.").
- Single-page commingled document: NSI source list may appear at bottom, separate from CAB
  (§E.4 p33) — flag for the implementer, out of scope for basic detection.
- Pure-NATO document (no US CAB): §E.5 does not apply (no US CAB exists). A §E.5 string that
  *is* present in such a document is reported as `PresentNotRequired`, not `Present` (FR-002).
- Entirely-`(U)` document: portion marks not required (#420).
- Malformed page-break candidate: `DocumentContext`/`PageContext` reset must occur BEFORE parse
  (existing invariant, preserved).
- Document boundary in a multi-document buffer: a *document* boundary is the **input boundary**
  (one `lint`/`fix` call = one document = one `id` in batch), NOT a scanner-emitted candidate.
  The scanner emits `MarkingType::PageBreak` only; `DocumentContext` is constructed fresh per
  input and resets only at the page boundaries within that input. There is no in-buffer
  document-delimiter heuristic (a concatenated multi-document buffer is a caller error, not a
  supported input). See `contracts/document-artifact.md`.

## Requirements *(mandatory)*

### Functional Requirements

**Document-artifact model (US1)**
- **FR-001**: Document-scoped artifacts MUST be modeled as typed nodes distinct from the
  marking pivot type; CAB MUST be decoupled from `CanonicalAttrs`/`ProjectedMarking`.
- **FR-002**: Each artifact node MUST carry one of five states: `Present(parsed)`,
  `PresentNonCanonical(parsed)`, `AbsentButRequired`, `AbsentNotRequired`,
  `PresentNotRequired(parsed)`. The state set is the product of two orthogonal axes —
  *presence* (absent / present-canonical / present-non-canonical) × *requirement*
  (required / not-required) — with the absent×{required,not-required} and
  present×{required,not-required} cells enumerated. `PresentNotRequired` is the
  present-but-superfluous cell (e.g. a §E.5 NATO `Declassify On` string in a pure-NATO
  document where §E.5 does not apply); it keeps "present-but-should-not-be" a node state
  rather than an out-of-band rule, preserving the absence-is-a-state uniformity (research
  D2). `ArtifactState` is a status enum, NOT a lattice — one recognizer produces one
  state; states of the same node are never joined (lattice-consultant verdict, research
  D12).
- **FR-003**: A document-scope aggregate (`DocumentContext`) MUST exist as the analogue of
  `PageContext`, holding artifact nodes; it MUST reset at scanner page/document boundaries
  consistent with the existing `PageContext` reset invariant.

**Derivation (US2)**
- **FR-010**: Derivation relationships MUST be modeled as a static DAG over scope-tagged facts,
  validated once at `Engine::new` (extending `marque-engine::scheduler` — writers before
  readers, cycles rejected).
- **FR-011**: Resolution MUST be decoupled from fixing — the engine resolves what a document
  *should be* regardless of whether fixing is enabled.
- **FR-012**: Every derivation that fires MUST be recorded content-ignorantly via the
  `DecisionSink` cascade (`DecisionEvent::triggered_by`).
- **FR-013**: Fixability MUST follow derivability: an absent node with an inbound derivation edge
  is fixable; one without is flag-only.
- **FR-014**: Edge firing MAY be conditional, including gated by deployment mode; mode MUST be a
  firing predicate on an always-declared edge, never a topology swap.
- **FR-015**: The engine MUST support reverse validation (overall/front-marking vs. all pages'
  markings) and a document "classified up to" front marking.

**Co-residence (US3)** *(CUI specifics source-pending)*
- **FR-020**: The document container MUST be scheme-**set**-parameterized, not mono-`S`.
- **FR-021**: Cross-scheme reconciliation MUST occur at two scopes: portion-scope ownership
  routing AND document-scope releasability.
- **FR-022**: The releasability relationship MUST be modeled as a componentwise join over a
  `Product` of the two schemes' dissem axes plus a monotone cross-component NOFORN closure
  (lattice-consultant verdict, see `research.md` D6); each regime renders its own projection.
  The `Product` MUST implement `JoinSemilattice` only (plus `BoundedJoinSemilattice` when both
  factors have a bottom) and MUST NOT implement `BoundedLattice` — `CuiReleasability`'s LDC set
  is agency-extensible/open (no lawful finite top), exactly as `SciSet`/`SarSet` already are
  (research D6 amendment). No new lattice-trait surface is introduced.
- **FR-023**: A token a scheme rejects MUST be offered to co-active schemes before junk-recovery
  consumes it (no silent marking loss); misplaced tokens with a home elsewhere MUST be
  relocated, not evicted. **Contention precedence** when more than one scheme is co-active:
  (a) if exactly one co-active scheme **accepts** the rejected token, it is routed there;
  (b) if **two or more** schemes accept it in the same portion, that is the mutually-exclusive
  cross-grammar case → portion-scope conflict (FR-024), never a silent pick; (c) if **no**
  scheme accepts it, it falls to junk-recovery with the unchanged `DocumentContent` confidence.
  Scheme registration order MUST NOT be a tie-breaker (it would make resolution
  registration-order-dependent and non-deterministic across callers); ties are always surfaced
  as conflicts, not resolved by precedence.
- **FR-024**: `(S//CUI)` MUST produce a portion-scope cross-grammar conflict with no auto-fix;
  the relocate suggestion is human-confirmed.
- **FR-025**: `marque-scheme` MUST remain domain-neutral (Constitution VII); cross-scheme
  reconciliation lives in `marque-engine` (model b).
- **FR-026**: Phase E (co-residence) MUST land against a **synthetic test-only second scheme**
  (`StubScheme`, an invented non-IC control with no claimed NARA/ISOO/CAPCO authority), NOT a
  real CUI grammar. Co-residence acceptance MUST exercise the `Product`+closure *mechanism* and
  the relocate-not-evict *flow* without asserting any real CUI semantic (the FEDCON⇒NOFORN
  mapping and LDC ordering remain source-pending until the `marque-cui` crate holds its
  governing policy). Encoding an unverified CUI mapping in a passing test fixture is a
  Constitution VIII violation; the synthetic scheme is how Phase E ships without committing one.

**Input boundary (US4)**
- **FR-030**: An `InputAdapter` trait MUST exist in `marque-scheme` producing canonical form (or
  a multi-layer `StructuredDocument`) without the scanner/parser for structured/schema/hybrid
  inputs.
- **FR-031**: `InputSource` (#176) MUST be promoted to `marque-scheme` and carried via
  `InputContext`; recognition provenance MUST license fix-assertiveness per the #176 matrix.
  **WASM stance (Constitution III)**: `InputSource` raises the recognizer's lone-case posterior
  and licenses assertive fixes, so a WASM caller supplying `StructuredField` from behind
  postMessage would be caller-provided posterior modulation on an uninspected trust boundary —
  precisely what Constitution III forbids the WASM target from accepting at runtime. Therefore
  the WASM build MUST pin `InputSource::DocumentContent` (compiled-in default; the
  `--input-source`/per-request opt-in exists for CLI and server only, where the caller is a
  trusted operator). The WASM API MUST NOT expose an `InputSource` parameter.
- **FR-032**: Corrections MUST carry a `RepairKind` (text-span vs. schema-attribute vs.
  structured-emit) so schema inputs are corrected type-safely.

**Mode taxonomy (US5)**
- **FR-040**: `[engine] severity_cap` MUST cap all rule severities, with per-rule overrides
  still winning.
- **FR-041**: Rules MAY declare `target_zones`; `[engine] fix_zones` MUST gate fix promotion by
  zone (diagnostics still emit for all zones).
- **FR-042**: A `DeploymentContext` (interactive/batch/boundary/archival) MUST provide an
  overridable defaults profile.
- **FR-043**: `as_of` MUST be wired end-to-end (engine → recognizer → rule context);
  `ArchivalIntent` (Update/PreserveWithMetadata/ValidateForEra) and `GrammarEra` MUST gate
  era-aware processing.

**Concrete rules (US6)**
- **FR-050**: Presence of any RD/FRD/TFNI portion MUST require the §E.4 canned `Declassify On`
  string; presence of NATO portions in a US-classified document MUST require the §E.5 string;
  both apply when both are present. Fixes are high-confidence (literal mandated strings).
- **FR-051**: Missing portion-marks and banners MUST be detected (#420), with the
  entirely-`(U)` document exemption.
- **FR-052**: #128 second-banner-line caveats MUST be modeled; they are recognized as the same
  releasability-escrow surface as the CUI `LDC` value set (source-pending vocabulary).

**Reversibility & derivation generation — deferred, roughed-in**
- **FR-060** *(#824, deferred)*: Every `ReplacementIntent`/`TextCorrection` variant MUST be able
  to carry its inverse in audit-permitted terms (token canonicals, category IDs, span offsets,
  BLAKE3 digests); the Phase-0 type surface MUST reserve these fields. Token-level fixes are
  self-reversible from the audit log; free-form text corrections are reversible only against the
  caller's retained original. Realization is an additive `marque-3.x` audit-schema bump.
- **FR-061** *(#823, deferred)*: The declassify-on / derived-from node MUST reserve a
  bundle-scope inbound edge (set of source documents → this document's CAB) even before the
  source-metadata adapter ships. A `Scope::Bundle` variant MUST exist.

### Key Entities

See `data-model.md` for the full type catalog. Summary:
- **DocumentArtifact** — a scope-tagged, typed node with an `ArtifactState`; the CAB,
  declassify-on, notice, and caveat-layer are instances.
- **ArtifactState** — `Present | PresentNonCanonical | AbsentButRequired | AbsentNotRequired |
  PresentNotRequired` (presence × requirement product; status enum, not a lattice).
- **DerivationEdge** — an inbound relation (rollup, requirement, source-derivation) into a node.
- **DocumentContext** — document-scope aggregate (analogue of `PageContext`); rolls page-level
  joins up to document scope, reusing the observational-state lattice types (`DissemSet`,
  `JointSet`) so unanimity/supersession survive the fold (research D12 / LV3).
- **DeclassifyOn value** — `OrdMax<DeclassInstruction>`: a SINGLE-VALUED chain (§E.3 p32 "Only a
  single value"; §E.4/§E.5 the canned N/A string REPLACES any date). `DeclassInstruction` is one
  enum with a hand-written total `Ord` over the §E.3 9-tier hierarchy (tier, furthest protection
  date, lowest exemption number); bottom = Unset, top = the single `Commingled` tier-1 point. The
  AEA/NATO/combined string choice is a render concern keyed on AEA-present/NATO-present flags, not a
  sub-lattice (research D12/LV2, corrected 2026-05-30). Rejects the earlier `Product`-of-two-axes
  model.
- **RecognitionProvenance** / **ValueDerivation** — the two orthogonal provenance axes.
- **InputAdapter / StructuredDocument / DocumentLayer / RepairKind / InputSource** — the input
  boundary.
- **Scheme-set container / ErasedScheme / CoherenceRule** — multi-scheme co-residence.
  (`Translate` is **cut from this feature** — it has no in-scope consumer; the deferred
  cross-system path adds it later. See `contracts/multi-scheme.md`.)

## Success Criteria *(mandatory)*

These operationalize the memo's "what the refactor must honor now" list.

- **SC-001**: A CAB no longer occupies any field on `CanonicalAttrs`/`ProjectedMarking`,
  verified by a type-level test asserting the pivot type has no CAB fields plus a node-type test
  asserting the CAB parses to a `DocumentArtifact`. (The "page aggregate, not a CAB" null-out
  comment and the fields it described are removed as a consequence, not as the criterion.)
- **SC-002**: A document container holds scheme-tagged layers whose schemes may differ; a test
  registers two schemes and routes portions to each. (FR-020)
- **SC-003**: Cross-scheme reconciliation is demonstrably present at **both** scopes
  (portion-ownership routing test + document-releasability join test), using the synthetic
  `StubScheme` (FR-026) — the join test asserts the `Product`+closure *mechanism* (a non-IC
  control floors the IC component to NOFORN), NOT any real CUI mapping. (FR-021/022/026)
- **SC-004**: A misplaced token with a home elsewhere is relocated (or flagged), never silently
  discarded — verified by a `(S//CUI)` no-silent-loss test. (FR-023/024)
- **SC-005**: The declassify-on/derived-from node exposes a reserved bundle-scope inbound edge
  and `Scope::Bundle` exists, with no source-metadata adapter required to compile. (FR-061)
- **SC-006**: Every `ReplacementIntent`/`TextCorrection` variant has a reserved place to carry
  its inverse, verified by a round-trip test for the token-level fixes whose pre-state is
  populated (`FactAdd`/`FactRemove` always; `Recanonicalize`/`Relocate` when `prior` is
  `Some(_)`). A `Recanonicalize { prior: None }` is explicitly out of round-trip scope until the
  #824 migration populates it. The G13 content-ignorance canary
  (`crates/engine/tests/audit_g13_canary.rs`) still passes. (FR-060)
- **SC-007**: At least one absent-node-with-inbound-edge case is filled (fix) and one
  absent-node-without-edge case is flag-only, in the same harness. (FR-013)
- **SC-008a** *(single-scheme, no-regression)*: Interactive-latency p95 ≤ 2 ms (the retired 16 ms
  placeholder) and linear fix throughput gates still pass for single-scheme CAPCO processing — no
  regression from the new document-scope pass, the derivation-DAG evaluation, or the #420 absence
  pass (Constitution I). A dedicated benchmark covers the #420 whole-document absence scan, with its
  own p95 ≤ 1 ms target / 2 ms absolute max, independent of resolution, since detecting *missing*
  marks is new O(blocks) work not present today.
- **SC-008b** *(multi-scheme, budgeted)*: Co-residence runs N scheme engines plus the
  reconciliation pass on the hot path. A new `multi_scheme_latency` benchmark establishes the
  two-scheme p95 budget (the single-scheme 2 ms gate is NOT assumed to hold unchanged under the
  O(schemes) multiplier; SC-008b commits an ABSOLUTE two-scheme p95 ceiling of 4 ms (= 2 × the 2 ms
  single-scheme ceiling) as the CI gate (not a self-ratifying "measured budget")), and regressions
  against it fail CI. (Constitution I; the multiplier is measured, not assumed.)
- **SC-009**: §E.4/§E.5 canned-string rules fire on AEA/NATO commingling with citations
  verified against `crates/capco/docs/CAPCO-2016.md`. (FR-050)
- **SC-010** *(input boundary, US4)*: The same `(YS)` value fed as `InputSource::StructuredField`
  recovers assertively (cap 0.95) while as a lone `DocumentContent` it stays a low-confidence
  suggestion (~0.50), per the #176 matrix; a `SchemaDocument` input bypasses scanner+recognizer
  and produces canonical form field-by-field; the WASM build rejects any non-`DocumentContent`
  `InputSource` (FR-031 WASM stance). (FR-030/031/032)
- **SC-011** *(mode taxonomy, US5)*: With `[engine] severity_cap = "suggest"` no `Fix`-default
  rule auto-applies while per-rule `[rules]` overrides still win; with `fix_zones = ["body"]`
  banner/CAB-zone fixes are not promoted though diagnostics still emit for all zones; with
  `deployment = "archival"` + `as_of` + `ArchivalIntent::ValidateForEra` rules postdating `as_of`
  are suppressed and no rewrites apply. (FR-040/041/042/043)
- **SC-012** *(reverse validation + caveat layer, US2/US6)*: A document "classified up to" front
  marking validated in reverse against all pages' markings reports divergence (FR-015); a #128
  second-banner-line caveat is recognized on its own line as the same releasability-escrow
  surface as the CUI `LDC` value set (FR-052), distinct from the dissem block.

## Out of Scope / Deferred

- **#823 (ICD 206 source-list generation)** — deferred. Gated on the structured source-metadata
  `InputAdapter` (US4). This feature only **reserves** the bundle-scope inbound edge and
  `Scope::Bundle` (FR-061).
- **#824 (reversible applied fixes)** — deferred for realization. This feature **reserves** the
  pre-state fields on fix-intent variants (FR-060); the additive `marque-3.x` audit-schema bump
  and the reversal pass land later.
- Full CUI grammar (`marque-cui` crate), its vocabulary, LDC ordering, and the
  FEDCON⇒NOFORN-specific mapping — source-pending; this feature lands only the domain-neutral
  co-residence infrastructure, exercised by a synthetic `StubScheme` (FR-026).
- The `Translate<A, B>` cross-scheme translation trait — **cut from this feature** (YAGNI),
  tracked in **#829** as a blocker for ISM→DoD XML round-trips. D1/D6 reject translate-then-join
  for releasability, and `Translate`'s only real consumers (cross-system marking translation,
  ISM→DoD XML round-trips) are deferred. Co-residence uses `CoherenceRule` only. `Translate` lands
  with the deferred cross-system path (#829).
- `marque-extract` Kreuzberg backend (licensing-gated, unrelated).
- Server auth/logging middleware.
