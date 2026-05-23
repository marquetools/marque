<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# F.1 corpus-fidelity gate — citation coverage report

This document is the reviewer-facing trace for every entry in
`EXPECTED_UNCOVERED` in `crates/capco/tests/citation_fidelity.rs`.
Each `<a id="...">` anchor below is referenced by exactly one
whitelist row; removing or renaming an anchor requires updating the
corresponding `EXPECTED_UNCOVERED` row (and vice versa).

## Surface counts (post-PR-10.A.2)

| Component | Count |
|-----------|-------|
| `CapcoScheme::constraints()` | 46 declared |
| `CapcoScheme::page_rewrites()` | 30 declared |
| `CapcoScheme::closure_rules()` (residual fn-pointer catalog) | 1 declared |
| Hand-written `Rule::cited_authorities()` overrides | 24 rules |
| **Declared citations (unique)** | **55** |
| **Harvested citations (over full corpus)** | **46** |
| **`EXPECTED_UNCOVERED` whitelist** | **10** |

> The 10-row bitmask `CLOSURE_TABLE` (see `crates/capco/src/scheme/closure_table.rs`) is iterated by `scheme.closure_inventory()` for tooling discovery, NOT by `scheme.closure_rules()`. The F.1 gate iterates `closure_rules()` (the residual fn-pointer catalog, 1 row); closure citations enter the declared set primarily through their bytewise-twin Rule `cited_authorities()` overrides (e.g., `S007`, `S008`).

## Whitelist contract

A citation enters `EXPECTED_UNCOVERED` only when one of the
following structural properties holds:

1. **Declared citation whose carrying primitive does not emit a
   `Diagnostic` at runtime.** Two sub-shapes share this property;
   per-row labels use the sub-shape number `(1a)` or `(1b)` so a
   reader scanning a row's `Property:` line lands on the right
   sub-shape without re-reading the family header.
   - *(1a) PageRewrite-side:* PageRewrites operate at projection
     time; they mutate the projected page-level marking but do
     not themselves emit a `Diagnostic` carrying their declared
     citation. The citation shows up in tooling and in the
     catalog inventory, but never on the engine's diagnostic
     stream.
   - *(1b) Engine-bridge-suppressed:* A `Constraint` that fires
     (the predicate evaluates true) but whose
     `ConstraintViolation` carries `span: None` and/or
     `severity: None`. The bridge at
     `engine.rs::bridge_constraint_diagnostic` requires both to
     be `Some` before producing a user-visible diagnostic;
     advisory `ConstraintViolation`s are logged via
     `tracing::trace!` only. E070 (`§H.6 p120`) was the
     representative case (closed by #661 — the predicate now
     populates both fields). No surviving whitelist entries
     exercise this sub-shape today, but the taxonomy is retained
     so a future helper adding an advisory-only violation has a
     documented carve-out path.

   Both sub-shapes share the structural invariant: no fixture
   can harvest the citation regardless of input, because the
   carrying primitive does not flow through the
   `Diagnostic.citation` slot. Removal of any (1a) entry
   requires changing the carrying primitive (e.g., adding a
   Diagnostic-emitting twin rule); removal of a (1b) entry
   requires fixing the engine bridge for advisory violations.
2. **Closure rule citation that has no byte-surfacing twin rule.**
   Same architecture as (1) — closures inject facts into a
   marking, no `Diagnostic` emission. Closures that DO have a
   byte-surfacing twin (e.g., `CLOSURE_RELIDO_SCI` surfaced as
   `S008`, `CLOSURE_REL_TO_USA_NATO` surfaced as `S007`) flow
   through the harvested set via that twin and do NOT need a
   whitelist entry.
3. **Cross-reference pin not used at the primary `Diagnostic.citation`
   slot.** Some rules declare a secondary `*_CROSS_REFS` constant
   pinning a §-citation that's the operative cross-reference but
   not the rule's primary anchor. The cross-ref ships in
   `Rule::cited_authorities()` so the catalog declaration is
   visible, but the engine's emitted `Diagnostic.citation` carries
   the primary anchor only (typed `Citation` holds one passage).
4. **Synthetic non-CAPCO `Citation` sentinels.** Two non-CAPCO
   citation sources exist:
   `AuthoritativeSource::Config` (`[config]`, used by
   `CORRECTIONS_MAP_CITATION`) and `AuthoritativeSource::EngineInternal`
   (`[engine-internal]`, used by R002 re-parse failure). Both
   render distinctively (`[config]` / `[engine-internal]`) and
   neither is exercised by a `Config::default()` engine because:
   the corrections-map path needs a `[corrections]` table the
   default config doesn't have; R002 fires only on engine-internal
   re-parse anomalies that don't occur on well-formed corpus
   fixtures.
5. **Rule whose trigger conditions are not present anywhere in the
   current corpus AND no MIGRATIONS table entry that would
   activate it.** For E006 the migration table carries no dissem
   replacements today (the entries that exist all route to E007's
   `Severity::Error` declass-shorthand path), so E006 never fires.
   This is data-dependent rather than structurally-unreachable —
   if a future migration entry routes a deprecated dissem token
   through E006, this whitelist row MUST be removed (assertion
   (c) of the F.1 gate fires when a previously-whitelisted
   citation becomes covered).

Property 1 and 2 are **architectural** — the catalog primitive
intentionally does not emit a diagnostic. Properties 3, 4, and 5
are **data-dependent** — they hold for the current rule body,
catalog, and corpus, but a future change can flip the row to
"covered" automatically (assertion (c) catches that and forces a
whitelist removal).

## Per-row justifications

<a id="config-sentinel"></a>

### `[config]` — `AuthoritativeSource::Config`

Property: (4) — Synthetic non-CAPCO sentinel.

The C001 corrections-map rule (`CorrectionsMapRule` in
`crates/capco/src/rules.rs`) emits `CORRECTIONS_MAP_CITATION` on
its diagnostics. The `AuthoritativeSource::Config` sentinel
identifies user-defined typo replacements as a non-CAPCO
provenance class so auditors can distinguish C001 fixes from
CAPCO-authoritative fixes. C001 fires only when the engine is
constructed with a non-empty `[corrections]` table in
`.marque.toml`; the F.1 gate uses `Config::default()` which
carries an empty corrections map. The `corrections_map_typo*`
fixtures in `tests/corpus/invalid/` exercise C001 via the
dedicated `c001_corrections_map_accuracy` harness in
`crates/engine/tests/corpus_accuracy.rs` (which constructs an
engine with explicit corrections); the F.1 gate runs a
default-config engine to keep the surface uniform across all
fixtures.

To remove this whitelist entry: add a probe test in
`citation_fidelity.rs` that constructs an `Engine` with a
corrections table covering at least one corpus fixture, and
union its harvested set into the main gate's harvested set.

<a id="engine-internal-sentinel"></a>

### `[engine-internal]` — `AuthoritativeSource::EngineInternal`

Property: (4) — Synthetic non-CAPCO sentinel.

The R002 re-parse-failure diagnostic is emitted by the engine
itself when a fix-pass produces output that fails to re-parse.
It carries the `AuthoritativeSource::EngineInternal` sentinel
because no CAPCO passage governs it; it's a defensive integrity
check. R002 does not fire on well-formed fixtures by
construction — the rule's whole purpose is to surface engine
bugs that produce ill-formed output during the fix loop.

To remove this whitelist entry: deliberately construct a
fixture whose lint→fix→re-parse trajectory triggers R002 (an
engine-internal regression marker). This is a future-PR exercise
parallel to the engine's deliberate fault-injection harnesses.

<a id="d1-p27-cross-ref"></a>

### `§D.1 p27` — RETIRED (issue #677, 2026-05-22)

Property: (3) was — Cross-reference pin not used as primary
citation. This carve-out is **retired**.

Pre-#677 state: `E005_CROSS_REFS` pinned the §D.1 p27 reference as
a secondary cross-reference (banner-syntax categories exclude
declassification — the negative-inference complement to §E.1 p31's
positive "Declassify On is a CAB line" rule), but E005's emitted
`Diagnostic.citation` was `§E.1 p31`, so §D.1 p27 never reached
the corpus harvester.

Post-#677 state: issue #677's `PortionFormInBannerRule` emits
`§D.1 p27` as its **primary** citation — the "Any control markings
in the banner line may be spelled out per the 'Marking Title' or
abbreviated as per the 'Authorized Abbreviation'" passage at
line 560 is the direct authority for the form-mismatch
diagnostic. Corpus fixtures under `tests/corpus/invalid/677_*.txt`
exercise the new rule, so the citation is now harvested by the
F.1 gate. The `EXPECTED_UNCOVERED` entry was removed; the gate
now treats the coverage as authoritative.

<a id="f-p35-deprecated-dissem"></a>

### `§F p35` — E006 deprecated-dissem rule

Property: (5) — Rule trigger conditions not exercisable from
the current MIGRATIONS table.

E006 (`DeprecatedDissemRule` in `crates/capco/src/rules.rs`)
walks Unknown tokens, looks each up in the MIGRATIONS table at
`crates/ism/build.rs`, and fires when the replacement is in
`is_dissem_replacement` (RELIDO / NOFORN / ORCON / IMCON /
DEA SENSITIVE / PROPIN). The current MIGRATIONS table has only
two entries — `25X1-` → `25X1` and `50X1-` → `50X1-HUM` — both
of which are declass-shorthand entries routed to E007 (filtered
out by E006's `!is_dissem_replacement` guard).

E006 exists for future expansion: if ODNI publishes a deprecated
dissem token migration (e.g., the historical `WNINTEL` → `RELIDO`
migration cycle), the MIGRATIONS row goes in and E006 fires
without further rule changes. Today no such row exists.

To remove this whitelist entry: either add a deprecated-dissem
MIGRATIONS row (data change in `crates/ism/build.rs`, requires
authoritative-source justification per Constitution VIII), or
expand E006's scope to fire on bare `LIMDIS`/`FOUO`/etc. via a
hand-written predicate (rule body change). Both are scope-creep
relative to PR 10.A.2.

<a id="h8-p150-suggest-rules-gap"></a>

### `§H.8 p150` — REL TO Suggest-rule coverage gap (T044)

Declared by the REL TO Suggest-severity rules:

- `S003` (deprecated-marking suggest) cross-ref via `S003_CROSS_REFS`
- `S004` (`RelToTrigraphSuggestRule`) primary authority via `S004_AUTHORITIES`
- `S005` (`RelToOpaqueUncertainReductionSuggestRule`) primary citation `S005_CITATION`
- `S010` (`CollapseUniformRelToPortionsSuggestRule`) primary citation `S010_CITATION`

No current corpus fixture exercises any of these rules. They are
Suggest-severity and default-Off in `.marque.toml`; the corpus
does not currently carry trigraph-typo / collapse-uniform /
uncertain-reduction fixtures.

The latent gap was previously masked by `E002` (Error-severity,
exercised by the `missing_usa_trigraph` fixtures) which until
T044 declared and emitted `§H.8 p150` too. T044 moved `E002` to
the more precise `§H.8 p151` per Constitution VIII (the verbatim
USA-first rule lives in the Additional Marking Instructions block
on p151; p150 is the section anchor for the REL TO marking template
generally). With E002 no longer covering p150, the Suggest-rule gap
surfaced as an F.1 gate failure.

The whitelist row is a deliberate structural carve-out per F.1
contract clause 1: the Suggest rules' carrying primitives do emit
diagnostics (text_correction shape with `cited_authorities()` →
`[§H.8 p150]`), but they don't fire on any current corpus fixture.
This is **NOT** "the citation is bogus" — it's "no fixture
currently triggers the rule that emits it."

**Resolution path**: file a follow-up PR adding S003/S004/S005/S010
fixtures (typo'd trigraph, uniform REL TO portions across a page,
uncertain reduction signal). When such fixtures land and the gate
harvests `§H.8 p150` from their emissions, this whitelist row is
removed (assertion (c) of the gate would otherwise fire). Tracking:
unfiled at T044 merge; will be opened against the corpus expansion
backlog in the post-T044 follow-up sweep.

<a id="h6-p120-frd-tfni"></a>

### `§H.6 p120` — E070 FRD/TFNI precedence (closed by #661)

Closed by PR `fix/661-e070-frd-tfni-bridge`. Retained here so the
prior whitelist anchor resolves for git-history readers; the
corresponding row was removed from `EXPECTED_UNCOVERED` in the
same PR (assertion (c) of the F.1 gate would have fired
otherwise — fixture coverage now matches the declared catalog).

Resolution (Path A from the original issue body): `e070_frd_tfni_precedence`
at `crates/capco/src/scheme/predicates/tier1_mask.rs` was
updated to populate both `span` and `severity` on the emitted
`ConstraintViolation`. `severity` is `Severity::Fix` mirroring
`e024_rd_precedence` (the resolution — drop TFNI when FRD is
present in the same portion — is unambiguous). `span` anchors on
the dominated TFNI token per §H.6 p120's "the 'TFNI' marking
does not appear in the banner line" wording; the inline walk
filters `attrs.token_spans` on `TokenKind::AeaMarking ∧ text ==
"TFNI"` so the span lands on TFNI rather than the first AEA
marking in source order (which would be FRD when both are
present). Fixture
`tests/corpus/invalid/e070_frd_tfni_precedence.txt` (portion
form `(S//FRD/TFNI//NOFORN)`) exercises the now-harvested path.

Path B (generalize the engine bridge to surface advisory
`(None, None)` violations) remains a deferred follow-on if a
future dyadic helper genuinely needs the advisory channel
without populated span/severity. Today every helper in
`tier1_mask.rs` populates both fields.

<a id="h8-p134-fouo-eviction"></a>

### `§H.8 p134` — FOUO eviction PageRewrite rows

Property: (1a) — PageRewrite-side; no `Diagnostic` emission.

Two PageRewrite rows in
`crates/capco/src/scheme/rewrites/pattern_b.rs` declare §H.8
p134 (search by row-name string, which is stable across edits):

- `capco/classification-evicts-fouo` (classified-document
  sub-clause, row 1 in `pattern_b_rows()`).
- `capco/non-fdr-control-evicts-fouo` (UNCLASSIFIED-with-other-
  non-FD&R-control sub-clause, row 2 in `pattern_b_rows()`).

Both operate at projection time via `CapcoScheme::project(Scope::Page,
...)` — they mutate the projected `ProjectedMarking` (removing
FOUO from the projected dissem set) but do not emit any
diagnostic carrying §H.8 p134.

The PageRewrite catalog is the declarative surface for these
transformations; the rendered banner reflects the post-rewrite
state, but Marque does not currently emit a "your FOUO was
silently evicted" notice. A future Stage-4 admonition channel
(noted in `marque-applied.md`) would surface these rewrites as
informational diagnostics — at which point this whitelist row
would be removed (assertion (c) firing).

<a id="h8-p140-oc-usgov-supersession"></a>

### `§H.8 p140` — OC-USGOV clears RELIDO PageRewrite

Property: (1a) — PageRewrite-side; no `Diagnostic` emission.

The §H.8 p140 anchor is carried by the `capco/orcon-usgov-clears-relido`
PageRewrite in
`crates/capco/src/scheme/rewrites/relido_clears.rs` (within
`relido_clears_rows()`; rule-id pin E057, pre-#559). Same
architectural property as `§H.8 p134` — projection-time mutation
only, no `Diagnostic` emission. The rewrite removes `TOK_RELIDO` from the page when
`TOK_ORCON_USGOV` is present, per CAPCO-2016 §H.8 p140 (ORCON-
USGOV entry, "Relationship(s) to Other Markings": ORCON-USGOV
*"May not be used with RELIDO."*).

Path correction during PR 10.A.2 reviewer fix-pass: the earlier
entry pointed at a `DissemSet::with_oc_usgov_supersession`
helper that does not exist (no such symbol in
`crates/capco/src/lattice.rs`). The actual source of §H.8 p140
is the PageRewrite row above.

<a id="h9-p170-limdis-eviction"></a>

### `§H.9 p170` — LIMDIS-evicted-by-classified PageRewrite

Property: (1a) — PageRewrite-side; no `Diagnostic` emission.

`capco/limdis-evicted-by-classified` PageRewrite (row 1) in
`crates/capco/src/scheme/rewrites/pattern_c.rs::pattern_c_rows()`.
Operates at projection time; the engine harvests the W003
diagnostic at §H.9 p169 (`NonIcInClassifiedBannerRule`) for the
same underlying violation, so the user gets a diagnostic — just
not one carrying §H.9 p170.

<a id="h9-p176-sbu-eviction"></a>

### `§H.9 p176` — SBU-evicted-by-classified PageRewrite

Property: (1a) — PageRewrite-side; no `Diagnostic` emission.

Companion to §H.9 p170; same architectural property.
`capco/sbu-evicted-by-classified` PageRewrite (row 2) in
`crates/capco/src/scheme/rewrites/pattern_c.rs::pattern_c_rows()`.
W003 at §H.9 p169 surfaces the SBU-in-classified-banner
violation; the PageRewrite's §H.9 p176 anchor is the catalog
declaration visible to tooling.

<a id="h9-p178-sbu-nf-supersession"></a>

### `§H.9 p178` — SBU-NF supersession PageRewrites

Property: (1a) — PageRewrite-side; no `Diagnostic` emission.

Two PageRewrite rows declare §H.9 p178:

- `capco/sbu-nf-implies-noforn` in
  `crates/capco/src/scheme/rewrites/pattern_a.rs` (NOFORN
  injection on SBU-NF presence — search by row-name string,
  which is stable across edits).
- `capco/sbu-nf-supersedes-sbu` in
  `crates/capco/src/scheme/rewrites/supersession.rs` (within
  `supersession_rows()`; drops bare `SBU` when `SBU-NF` is also
  present).

Both are projection-time rewrites that the renderer reflects in
canonical output; no diagnostic emission.

<a id="h9-p185-les-nf-supersession"></a>

### `§H.9 p185` — LES-NF supersession PageRewrites

Property: (1a) — PageRewrite-side; no `Diagnostic` emission.

Companion to §H.9 p178 for the LES-NF supersession family. Two
PageRewrite rows declare §H.9 p185:

- `capco/les-nf-implies-noforn` in
  `crates/capco/src/scheme/rewrites/pattern_a.rs` (NOFORN
  injection on LES-NF presence — search by row-name string,
  which is stable across edits).
- `capco/les-nf-supersedes-les` in
  `crates/capco/src/scheme/rewrites/supersession.rs` (within
  `supersession_rows()`; drops bare `LES` when `LES-NF` is also
  present).

Same architectural property as the SBU-NF entry.

<a id="b3-table2-p21-s008-trigger-authority-not-emitted"></a>

### `§B.3 Table 2 p21` — S008 trigger authority (declared, not emitted)

Declared by `S008_AUTHORITIES` in `crates/capco/src/rules.rs` as the
primary trigger authority for the `S008
relido-implied-by-closure` rule. The cited table row —
"Classified + uncaveated + on/after 28 June 2010 → Mark as
RELIDO" — is the §-spec obligation that drives S008's
`Severity::Suggest` emission when the project pipeline would
inject implicit RELIDO via `default_fill::row{8,9}_should_fill`.

The per-`Diagnostic` Citation field is single-valued by API shape
(`Diagnostic::with_fix_at_span` takes a single `Citation`). S008
emits `§H.8 p154` (RELIDO marking template — what RELIDO means
once present) in that single-Citation slot because it's the
marking-template anchor a reviewer will most directly use to
interpret the diagnostic. `§B.3 Table 2 p21` IS exercised
end-to-end by every S008-firing fixture; it's just not the
per-Diagnostic emission target.

This is a deliberate authority-slice / emission-slot decoupling
introduced post-#704 review-cycle resolution Fix 2: the
authority slice honestly reports all the §-anchors S008's
behavior depends on; emission picks the single most reviewer-
relevant anchor.

**Resolution path**: when per-Diagnostic emission becomes
multi-Citation (a future trait-surface change that's out of
scope for #704), S008 would emit both `§B.3 Table 2 p21` and
`§H.8 p154` and this whitelist row retires. Tracking: filed
against the multi-Citation emission backlog at #704 merge.

## Removing a whitelist entry

When a fixture starts exercising one of the citations above, the
F.1 gate's assertion (c) fires — `EXPECTED_UNCOVERED ∩ harvested
== ∅`. The failure message names the now-covered citation. To
resolve:

1. Confirm the new coverage is legitimate (the fixture exercises
   a real authoritative-source passage at the cited §-anchor).
2. Remove the matching row from `EXPECTED_UNCOVERED` in
   `crates/capco/tests/citation_fidelity.rs`.
3. Remove the corresponding `<a id="...">` paragraph from this
   document.
4. Cite the fixture and the change set in the commit message.

## Adding a whitelist entry

When a new rule or catalog row declares a citation that no
fixture can exercise:

1. Confirm the structural reason via the property taxonomy above.
   If the citation is data-dependent (property 5), add a TODO
   for follow-up coverage rather than locking it into the
   whitelist permanently.
2. Add a paragraph under "Per-row justifications" with a unique
   `<a id="...">` anchor.
3. Add the matching `(citation, "anchor-id")` row to
   `EXPECTED_UNCOVERED`.
4. Re-verify the §-citation against
   `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
