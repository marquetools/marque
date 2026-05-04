<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Feature Specification: Engine + Rule Architecture Refactor

**Feature Branch**: `006-engine-rule-refactor`
**Created**: 2026-05-03
**Status**: Draft
**Input**: User description: "Develop a proper spec for `docs/plans/2026-05-01-lattice-design.md` and `docs/plans/2026-05-02-engine-refactor-consolidated.md` (the latter is the primary)."
**Source plans**:
- Primary: `docs/plans/2026-05-02-engine-refactor-consolidated.md`
- Gating: `docs/plans/2026-05-01-lattice-design.md` (filled in by PR 3.7 inside the primary)
**Constitution gates**: I (performance), III (WASM safety), IV (two-layer rules), V (audit-first / G13), VI (dataflow pipeline), VII (acyclic dependencies), VIII (citation fidelity).

## User Scenarios & Testing *(mandatory)*

The "users" of this refactor are layered: the **compliance auditor** who reads
`AppliedFix` audit records to verify lawful fix application, the **rule author**
who extends the catalog without inadvertently bypassing safety, the
**end user** invoking marque (CLI / IDE / WASM embedder) who needs correct
diagnostics on real classified-marking corpora, and the **maintainer** of marque
on a 5-year horizon who needs invariants enforced by types rather than by
comment-propagated convention. Each user story below names which layer it
serves and why that layer's needs cannot be deferred.

### User Story 1 — Audit records carry no document content (Priority: P1)

A compliance auditor reviews an `AppliedFix` audit record produced by marque
and can verify, without reading the source document, that no document content
bytes — no input strings, no content-derived free-form text — appear anywhere
in the record. Permitted identifiers are limited to token canonicals, category
IDs, span offsets, BLAKE3 digests, posterior scalars, enumerated feature
labels, and enumerated message templates.

**Why this priority**: Constitution V Principle V (G13) treats audit-record
content-ignorance as a correctness property of the same severity as a wrong
predicate. The current implementation enforces it by carve-out comments
(`engine.rs::build_decoder_diagnostic`'s `proposal.original = ""` branch,
the R001 message that interpolates input bytes via `format!`, the
`provenance.canonical_bytes` field that may carry uppercased input segments
on the decoder path). One known leak is live (#257). The tool cannot be
deployed in IC/DoD contexts while audit-record integrity rests on
comment-propagated invariants.

**Independent Test**: A deterministic NDJSON canary scan over the full corpus
regression sweep (`tests/corpus/valid/`, `tests/corpus/mangled/`,
`tests/corpus/prose/`, `tests/corpus/prose-positive/`,
`tests/corpus/lattice/`) finds zero verbatim input bytes inside any
`AppliedFix` JSON serialization other than within span-offset numerals.
The scan reads `Engine::fix_inner`'s emitted `Vec<AppliedFix>` only;
test-fixture records constructed under the Constitution V carve-out are
explicitly excluded.

**Acceptance Scenarios**:

1. **Given** a corpus document `(TS//SI-G xyzz)` with a decoder-recognizable mangling, **When** marque applies fixes and emits the audit record, **Then** the JSON serialization contains no occurrence of the literal byte sequence `xyzz` (or any other input substring) outside span offsets, BLAKE3 digests, or enumerated identifier values.
2. **Given** a rule attempts to construct a `Canonical` value from `Box<str>` containing arbitrary bytes, **When** the project is built, **Then** the build fails to compile (no public `Box<str> → Canonical` constructor exists for closed-CVE tokens; open-vocab construction is restricted to sealed render call sites).
3. **Given** a diagnostic message describing decoder recognition, **When** the message is emitted, **Then** it carries a `MessageTemplate` enum value plus a closed-set arguments tuple — never a `format!`-interpolated string containing raw input.

---

### User Story 2 — Page-level rollup is correct for foreign and joint markings (Priority: P1)

A user submits a document containing only foreign portions (e.g., a page of
`(C//FGI DEU)` portions, a NATO-only page, or a JOINT US/UK page). The
banner classification produced by marque retains the foreign marker —
the page is not silently re-banner'd as a US classification, and FGI/NATO/JOINT
provenance survives the page-level join.

**Why this priority**: `page_context_to_attrs` at `crates/capco/src/scheme.rs:365`
hardcodes `MarkingClassification::Us`. A foreign-only document gets a
factually wrong banner today (#276). For a tool whose authority comes from
matching CAPCO-2016, this is a correctness defect at the same level as
fabricating a citation. It also blocks lawful use of marque on documents
produced under foreign-disclosure agreements.

**Independent Test**: A targeted corpus fixture set
(`tests/corpus/foreign/pure_foreign_banner.json`, FGI banner roll-up
fixtures, NATO-only page fixtures, JOINT page fixtures) lints to a banner
that retains foreign provenance in 100% of cases. The
`MarkingClassification::Us` hardcode is removed at the source line; CI
greps for re-introduction.

**Acceptance Scenarios**:

1. **Given** a page containing only `(C//FGI DEU)` portions, **When** marque computes the expected banner, **Then** the banner retains the FGI DEU marker — `expected_classification()` returns `Some(MarkingClassification::FgiDeu)`-equivalent or `None`, never silently `MarkingClassification::Us`.
2. **Given** a page mixing US `(S)` portions with `(C//FGI DEU)` portions, **When** marque computes the expected banner, **Then** the banner correctly captures both axes — joint provenance with the higher classification, FGI marker preserved.
3. **Given** the rule catalog is registered, **When** the engine boots, **Then** the page-level rollup pipeline drives through the lattice projection (`Scope::Page`), not through the legacy `PageContext` accumulator (which is deleted at the merge of the cutover PR).

---

### User Story 3 — Pass-1 token rewrites do not corrupt pass-2 rule input (Priority: P2)

A user submits a document where E001 fires (`OC → ORCON` localized rewrite)
on a portion that also has an E003 ordering issue. Marque applies the E001
fix, re-parses the modified buffer, then evaluates E003 against the post-rewrite
attributes. E003's confidence and message reflect the post-rewrite token
spans — not stale pre-rewrite spans — and the user sees an E003 suggestion
matching what the document now contains.

**Why this priority**: Today, E003 reads `attrs.token_spans` from the original
parse and cannot see the localized E001 rewrite (#272 / #273 / #274).
Confidence is wrong; suggested re-orderings are wrong. This is a correctness
defect that surfaces every time E001 fires on a portion E003 also flags.

**Independent Test**: Property tests at `crates/engine/tests/two_pass_invariants.rs`
shuffle pass-1 / pass-2 fix orderings and assert: (a) no overlap in
promoted `AppliedFix` records (I-18); (b) `Phase::WholeMarking` rules
re-validate against pre-pass-1 attributes before firing (I-19);
(c) `fix_10kb` Criterion bench shows the two-pass overhead is within the
SC-001 latency budget.

**Acceptance Scenarios**:

1. **Given** a portion `(OC, FOUO)` triggering both E001 (OC→ORCON) and an E003 ordering check, **When** marque applies fixes, **Then** the E001 fix lands first, the buffer re-parses, and the E003 diagnostic reflects the post-rewrite ORCON form (not stale OC).
2. **Given** a pass-1 fix that retroactively makes a pass-2 predicate hold (incidental satisfaction), **When** marque evaluates pass-2, **Then** the pass-2 rule re-validates against pre-pass-1 attributes and does not fire — preserving the "this defect was already fixed" semantic.
3. **Given** a pass-1 fix produces an unparseable buffer, **When** the engine re-parses for pass-2, **Then** marque emits an `R002 — pass-1 fix produced unparseable buffer` diagnostic, retains the pass-1 audit records, returns the pass-1 buffer as the corrected document, and does not run pass-2.

---

### User Story 4 — Open-vocabulary input is never silently corrupted (Priority: P2)

A user submits `(TS//FGI deu)` (lowercase trigraph — invalid). Today, the
parser silently returns `Some(FgiMarker { countries: [] })`, which collides
on shape with lawful source-concealed FGI per CAPCO §H.7 p126
(an `FgiMarker` with no country trigraphs is the *authentic* representation
of source-concealed FGI). After this refactor: invalid trigraph bytes
cause the parser to return `None`, and the data model carries a
`FgiMarker::SourceConcealed | Acknowledged { countries }` discriminant so
that lawful and corrupted shapes are unrepresentable as the same value.

**Why this priority**: Silent semantic corruption (#280) is the worst class
of marking-tool bug: marque appears to validate the input, downstream rules
read the corrupted shape as if it were lawful source-concealed FGI, and
the audit log records a confident validation. The user has no way to tell
the difference. The fix is mechanical (route the four open-vocabulary
admission sites in `parser.rs` through `Vocabulary<S>::shape_admits`) but
removing the shape-collision requires a discriminant introduction in the
data model.

**Independent Test**: `tests/parser/fgi_silent_skip_guard.rs` asserts that
the four cited parser sites (`parser.rs:1011-1024`, `:1453`, `:1481`,
`:1493`) return `None` for `shape_admits`-failing input. A rule audit
sweep confirms no `countries.is_empty()` matching pattern survives in
`marque-capco`. A parse–render round-trip property test
(`crates/capco/tests/parse_render_roundtrip.rs`) catches silent semantic
degradation across the full strict-path corpus.

**Acceptance Scenarios**:

1. **Given** input `(TS//FGI deu)`, **When** marque parses it, **Then** `parse_fgi_marker` returns `None` (rejecting the malformed trigraph) — not `Some(FgiMarker { countries: [] })`.
2. **Given** lawful source-concealed FGI input matching CAPCO §H.7 p126, **When** marque parses it, **Then** the parser produces `FgiMarker::SourceConcealed`, distinct from any post-failure shape.
3. **Given** any portion-form input across the strict-path corpus, **When** marque parses, renders, and re-parses, **Then** the second parse equals the first under the structural-equality relation that ignores `provenance.source_bytes` and confidence floats and allows whitespace/casing/ordering canonicalization.

---

### User Story 5 — Citations in rules are mechanically verifiable (Priority: P2)

A reviewer reading a rule's diagnostic message, doc comment, or
`citation:` field can mechanically verify (no manual spot-check) that
every `§X.Y pNN` reference points to a real passage in the vendored
authoritative source (`crates/capco/docs/CAPCO-2016.md`), in the
normative range (CAPCO §A–H), with a page number that falls within the
document. Citations that fabricate sections (e.g., the literal `§4`
references in the prior `crates/capco/src/scheme.rs` — CAPCO has only
§A–H normative), drift across propagation (the `p150–151 p151` doubling
at five sites in `rules.rs`), or attribute to wrong revision (the
SIGMA archaeology at `rules.rs:4053`) fail CI.

**Why this priority**: Constitution VIII explicitly treats citation
fabrication as a correctness defect of the same severity as a wrong
predicate, because a fabricated citation looks like documentation and
survives review precisely because reviewers trust that the underlying
claim has been checked. The HCS-P fabrication at `scheme.rs:1787`
demonstrates the failure mode reaches production. The tool cannot ship
fixes alongside lies about their provenance.

**Independent Test**: The citation lint at `tools/citation-lint/`
(AST-based, not regex) parses every `citation:` field, `message:`
string, `constraint_label:` string, and doc-comment `§X.Y` reference;
asserts §X.Y exists in the vendored source; asserts the page falls
within markdown offsets; asserts §X.Y lies in normative range §A–H;
rejects bare `§NN`. The F.1 corpus-fidelity gate
(`crates/capco/tests/citation_fidelity.rs`) requires ≥1 corpus fixture
per cited authority exercising the predicate against the canonical
example.

**Acceptance Scenarios**:

1. **Given** a rule whose diagnostic message cites `§Z.99 p999` (no such section exists), **When** CI runs, **Then** the citation lint fails with a specific error pointing at the offending line.
2. **Given** every existing citation across rules, messages, constraint labels, and doc comments, **When** PR 0.5 / 0.6 lands, **Then** every citation is mechanically verified against the vendored source and the four pre-existing citation defect classes (`§4` fabrications, `p150–151 p151` doublings, SIGMA cross-revision archaeology, HCS-P over-strict predicate if F.1 surfaces it) are corrected.
3. **Given** a constraint or page-rewrite definition with a §-citation, **When** the F.1 fidelity gate runs, **Then** ≥1 corpus fixture exercises the predicate against the canonical example from the cited passage; the absence of such a fixture fails CI.

---

### User Story 6 — Lattice projection laws hold for every marking category (Priority: P2)

A maintainer adding a new constraint or page rewrite can rely on
per-category lattice impls satisfying associativity, commutativity,
idempotency, and identity-with-bottom — and additionally satisfying
cross-axis dominance fixtures (FOUO eviction by classification > U AND
by non-FD&R dissem; FGI banner roll-up; SCI cross-system
canonicalization; AEA exemption commingling). The page-level join
(`Scope::Page` projection) is the single source of truth for what a
banner should look like; `CapcoMarking::join` no longer delegates to
the procedural `PageContext` accumulator.

**Why this priority**: The previous lattice attempt (the 2026-04-17
work) skimmed the per-category math and ended up bandaged until it was
almost unused. PR 4 cannot land that failure mode again. PR 3.7
(lattice §-resolution spike) gates PR 4 by requiring the lattice design
doc to be filled in with §-citations, formal join semantics, worked
examples, property fixtures, and cross-axis dominance fixtures — with
no "explicitly deferred to a tracked issue" escape valve.

**Independent Test**: Property tests at
`crates/capco/tests/category_lattice_laws.rs` (assoc/comm/idem/identity
per category) and `crates/capco/tests/cross_axis_dominance.rs`
(cross-axis fixtures) pass for every category in
`CapcoScheme::categories()`. The `tests/corpus/lattice/` corpus
regression sweep covers the cross-axis fixtures end-to-end.
PR 3.7 acceptance requires a named reviewer in the PR description who
has confirmed each category's worked examples by hand against the
§-citation.

**Acceptance Scenarios**:

1. **Given** any two values in any category's lattice, **When** their join is computed under any associativity / commutativity ordering, **Then** the result is identical (assoc/comm/idem property tests pass).
2. **Given** a portion `(U//FOUO)` adjacent to a portion `(C)`, **When** marque rolls up to the page, **Then** FOUO is evicted (classification > U cross-axis dominance).
3. **Given** a portion `(U//PR//FOUO)`, **When** marque validates, **Then** FOUO is evicted by PROPIN (non-FD&R dissem cross-axis dominance), per the worked-example fixtures in the consolidated plan Appendix A.

---

### User Story 7 — Performance is preserved through the refactor (Priority: P3)

A user invoking marque interactively (CLI, IDE plugin, WASM embedder)
sees no degradation in interactive latency despite the type-system
changes (sealed `Canonical`, `Vocabulary<S>::shape_admits` indirect
call through `Arc<dyn>`, two-pass apply with re-parse). The
SC-001 budget (p95 ≤ 16 ms) holds; multi-page projection latency
stays within the baseline + 10% envelope; the single-pass forward
splice continues to scale linearly (R² ≥ 0.9).

**Why this priority**: Constitution I makes performance the primary
value proposition. The refactor introduces several theoretical
regression vectors — `shape_admits` through `Arc<dyn>` precludes
cross-crate devirtualization, two-pass apply re-parses between
passes, `Scope::Page` projection replaces a hot procedural code path.
Each must be measured, not assumed.

**Independent Test**: Four Criterion benches gate the relevant PRs:
`fix_throughput` (R² ≥ 0.9, already landed in PR #278);
SC-001 with **p99 tail-percentile assertion** (PR 2 `shape_admits`
indirect-call cost); `fix_10kb` (PR 7 two-pass re-parse cost);
`lint_100kb_multipage` (PR 6 `Scope::Page` projection cutover ≤
baseline + 10%). The §3.6 measurement-gating discipline (>5% mean
OR p99 regression backs out the change) applies uniformly.

**Acceptance Scenarios**:

1. **Given** a 10 KB single-portion document, **When** marque lints it, **Then** p95 latency stays ≤ 16 ms and p99 stays within the pre-refactor baseline + 5%.
2. **Given** a 100 KB multi-page document, **When** marque lints it through the new `Scope::Page` projection path, **Then** latency stays within the pre-refactor `PageContext` baseline + 10%.
3. **Given** a 10 KB document with N fixes (N ranging 1..1000), **When** marque applies fixes via the single-pass forward splice, **Then** the fix-application time scales linearly in N (R² ≥ 0.9 in the `fix_throughput` regression bench).

---

### User Story 8 — Refactor PRs are independently revertable (Priority: P3)

A maintainer encountering a regression in any individual PR of the
refactor sequence can revert that PR alone — the keystone (PRs 3a, 3b,
3c) is split so that pivot-type-split, rule-collapse, and
discriminant/sealing/cutover are independent commits with independent
revert points. PR 4's lattice impls, PR 5's `expected_classification`
widening, PR 6's `Scope::Page` projection cutover (sub-divided into
6a/6b/6c), and PR 7's pass split are each scoped to a single
correctness property and a single revert.

**Why this priority**: Murder board (refactoring-expert / system-architect /
backend-architect) flagged PR 3 as too large to revert safely. The
clean-break philosophy lowers compatibility cost but raises blast-radius
risk if any single PR is non-revertable. Granular revertability is the
mitigation.

**Independent Test**: A CI matrix during the keystone window runs
corpus regression × {3a-only, 3a+3b, 3a+3b+3c} = 3 runs to verify each
subsequence is independently correct. PR 6 sub-commits 6a (projection
behind feature flag) / 6b (bench both paths) / 6c (flip default,
delete `PageContext`) each pass corpus regression independently.

**Acceptance Scenarios**:

1. **Given** PR 3a has merged but PR 3b has not, **When** the corpus regression sweep runs, **Then** all corpora pass — the `from_parsed_unchecked` transitional adapter sustains existing rule code.
2. **Given** PR 6a has merged (projection behind feature flag), **When** the multi-page bench runs, **Then** both `PageContext` and `Scope::Page` paths satisfy SC-008 / SC-009 budgets.
3. **Given** any single PR in the sequence is reverted, **When** CI runs, **Then** the workspace remains buildable, the test suite passes (with corresponding test-fixture revert), and no orphaned types / functions / dependencies survive.

---

### Edge Cases

- **Pass-1 fix produces unparseable buffer**: Engine emits `R002 — pass-1 fix produced unparseable buffer` diagnostic, retains pass-1 audit records (the fixes happened; the audit log is honest), does not run pass-2, returns the pass-1 buffer as the corrected document. Atomic rollback would lie about what the audit ledger said vs. what the document actually contained.
- **Decoder encounters open-vocabulary token (e.g., agency-private SCI sub-comp)**: Decoder is locked out of open-vocabulary canonicalization (§8.2). Output is `Parsed::Ambiguous` with diagnostic-only surfacing — no `FixProposal`. Trade-off (made explicit in §8.2): a strict-fail input `(TS//SI-G xyzz)` where `xyzz` is legitimate-but-private gets a diagnostic instead of a fix proposal. That is the correct behavior — auto-fixing what cannot be validated is the bug.
- **Pre-cutover audit records**: There are no pre-cutover records (no users, no deployment). Clean break: pre-cutover records are unreadable by post-cutover binaries; this is a type-level guarantee, not a runtime concern. No `marque-audit-reader` crate is scheduled.
- **Rule needs both `Phase::Localized` and `Phase::WholeMarking` detection**: Register two rule entries sharing a backend module — one per phase — each with its own `RuleId`. The dispatch contract stays single-valued at registration. The "this rule does two distinct jobs" cost is surfaced at the rule-set level where it can be reviewed, rather than hidden behind a `Phase::Both` escape hatch.
- **Issue tracked by a masking pin closes as duplicate**: Masking-pin lint follows `closed_as_duplicate_of` chains until it hits a final close (mandatory, not optional). Cascade-close-via-meta-issue is flagged at lint time so a tracked issue cannot silently disappear without the pin being removed.
- **Pass-1 fix retroactively satisfies a pass-2 predicate**: I-19 reshape-aware re-validation. `Phase::WholeMarking` rules whose span overlaps a pass-1 fix re-validate against pre-pass-1 attributes (cached from pass-0). If the predicate held against the pre-reshape attrs, it was a real defect that pass-1 incidentally fixed; pass-2 does not re-fire. Disambiguation by `(scheme, predicate-id)` keys when multiple rules touch the same span.
- **Rule from an external rule crate (e.g., future `marque-cui`) needs to emit a fix**: External rule crates emit `FixIntent<S>` values and never construct `Canonical<S>` directly. The engine — holding the sealed `CanonicalConstructor<S>` impl — renders `FixIntent<S>` to `Canonical<S>` on the rule's behalf. The closed-construction property holds across the workspace boundary that Constitution VII opens up for new rule-crate families.
- **Mangled-corpus accuracy baseline shifts under decoder open-vocab lockout**: Decoder lockout reduces fix recall on inputs whose mangled tokens were previously decoder-canonicalized. Mangled-corpus baseline (this spec's SC-010; tracked as SC-004 in CLAUDE.md and the consolidated source plan §8.2) re-anchors at PR 3c implementation; the threshold may be adjusted downward to reflect intentional lockout, OR the corpus may be re-curated to exclude open-vocab cases that were never legitimately fixable. Decision deferred to PR 3c review — flagged here as an explicit deferral, not a hidden one.

## Requirements *(mandatory)*

### Functional Requirements

#### Audit-record integrity (G13 → type invariant)

- **FR-001**: Every byte in `FixProposal::replacement` MUST trace to either (a) a closed-CVE token via `Canonical::from_cve(TokenId, Scope)` or (b) a sealed render call site via `Canonical::from_render(category, bytes, scope, &'static Location)`. No public `Box<str> → Canonical` constructor MAY exist.
- **FR-002**: Audit records MUST contain no document content bytes, no input substrings, and no content-derived free-form text. Permitted identifier types: token canonicals, category IDs, span offsets, BLAKE3 digests, posterior scalars, enumerated `FeatureId` labels, enumerated `MessageTemplate` labels.
- **FR-003**: Diagnostic messages MUST be constructed from a closed `MessageTemplate` enum plus a closed-set `MessageArgs` of permitted scalar/ID types. Arbitrary `format!`/`concat!`-style interpolation of input bytes into messages MUST be unrepresentable.
- **FR-004**: `FixProposal::original` MUST become a `Span` (not byte content). Audit emitter resolves to BLAKE3 digest; raw input bytes never enter the audit record.
- **FR-005**: `AppliedFix::__engine_promote` and equivalent promotion-token constructors MUST be reachable only from `Engine::fix_inner` in production code. Test-fixture construction is permitted under the Constitution V Principle V carve-out (`#[cfg(test)]` modules, `tests/` integration files, `dev-dependencies`-gated test-utility crates only; never `cfg(not(test))`-reachable). Each carve-out call site MUST carry an inline comment naming the carve-out (e.g., `// Test-fixture carve-out per Constitution V`); FR-040's AST lint enforces presence of the comment within 5 lines of the call.

#### Page-level rollup correctness

- **FR-006**: Page-level marking aggregation MUST flow through `MarkingScheme::project(Scope::Page, ...)`. `PageContext` MUST be deleted at the cutover PR's merge — no equivalence shim window.
- **FR-007**: `MarkingClassification::Us` MUST NOT be hardcoded in any projection function. `expected_classification()` MUST return `Option<MarkingClassification>` so that pure-foreign pages produce a representable absent-US result.
- **FR-008**: A page composed entirely of FGI portions MUST produce a banner that retains the FGI marker; render-canonical MUST drop the redundant `FGI` token only when a country trigraph is present (#261 falls out of FR-007).
- **FR-009**: FOUO MUST NOT appear in any marking with classification > Unclassified (cross-axis `Constraint`). FOUO MUST be evicted by any non-FD&R dissem token in the same dissem set (in-category `SupersessionSet`). FD&R dissems (`REL TO`, `RELIDO`, `NOFORN`/`NF`, `DISPLAY ONLY`, `EYES`-deprecated) preserve coexistence per Appendix A worked examples.
- **FR-010**: `is_fdr_dissem` MUST be a per-token `Vocabulary<S>` metadata field (one-field extension of the Phase-5 metadata surface).

#### Lattice law compliance

- **FR-011**: Every category in `CapcoScheme::categories()` MUST have a `Lattice` impl satisfying associativity, commutativity, idempotency, and identity-with-bottom — exercised by property tests at `crates/capco/tests/category_lattice_laws.rs`.
- **FR-012**: Every category whose values interact with another category's dominance MUST satisfy cross-axis dominance fixtures at `crates/capco/tests/cross_axis_dominance.rs`. Required cross-axis fixtures: FOUO eviction by classification > U AND by non-FD&R dissem; FGI banner roll-up (#276); SCI cross-system canonicalization; AEA exemption commingling with classification.
- **FR-013**: `2026-05-01-lattice-design.md` §§2–8 MUST be filled in with §-citations to `crates/capco/docs/CAPCO-2016.md`, formal join semantics (precondition / postcondition functional form, not prose), worked examples (≥2 per category, including edge cases the §-citation calls out), and property-test fixture file/test names. Every §10 open question MUST resolve to a §-citation + explicit decision; the "explicitly deferred to a tracked issue" escape valve MUST be removed from §9 acceptance.
- **FR-014**: `CapcoMarking::join`'s delegation to `PageContext` MUST be deleted with no equivalence shim (clean break) once per-category `Lattice` impls land.

#### Open-vocabulary parser correctness

- **FR-015**: Open-vocabulary parser slots MUST route through `Vocabulary<S>::shape_admits`. Four open-vocabulary admission sites in `marque-core/parser.rs` MUST migrate: three inline `is_ascii_alphanumeric()` byte-class checks (`:1453`, `:1481`, `:1493`) and the FGI trigraph silent-skip at `:1011-1024` (currently `if token.len() == 3 { CountryCode::try_new(...) }` rather than `is_ascii_alphanumeric`, but with the same fix shape — `shape_admits`-gated admission; the `None` return on shape failure is the FR-016 surface). CI grep MUST flag re-introduction of inline `is_ascii_alphanumeric()` in parser open-vocab admission paths.
- **FR-016**: `parse_fgi_marker` MUST return `None` (not `Some` with degraded structure) when post-prefix bytes fail `shape_admits`.
- **FR-017**: `FgiMarker` MUST discriminate `SourceConcealed` (lawful per CAPCO §H.7 p126, no country trigraphs) from `Acknowledged { countries }` (one or more validated trigraphs). The post-failure shape MUST be unrepresentable. Rules currently using `countries.is_empty()` MUST be audited and migrated.

#### Citation fidelity

- **FR-018**: Every cited authority — in `citation:` struct fields, `message:` strings, `constraint_label:` strings, and doc-comment `§X.Y` references — MUST resolve to a real passage in `crates/capco/docs/CAPCO-2016.md`, in the normative range (CAPCO §A–H), with a page number that falls within the document. Bare `§NN` references (without subsection) MUST be rejected.
- **FR-019**: Every `Constraint`/`PageRewrite`/`Rule` cited authority MUST have ≥1 corpus fixture at `crates/capco/tests/citation_fidelity.rs` exercising the predicate against the canonical example from the cited passage.
- **FR-020**: The pre-existing four citation-defect classes (the `§4` fabrications across multiple `scheme.rs` lines; the doubled `p150–151 p151` at five sites in `rules.rs`; the SIGMA cross-revision archaeology at `rules.rs:4053`; the HCS-P over-strict predicate at `scheme.rs:1839-1849` if F.1 surfaces it) MUST be corrected preemptively before the keystone refactor begins. Implementer re-greps line numbers at PR 0.6 time — defect classes are stable, line numbers are not.

#### Two-pass apply correctness

- **FR-021**: Each rule MUST declare `Phase::Localized | Phase::WholeMarking` at registration. Engine MUST enforce: `Phase::Localized` rule's `FixProposal::span` is sub-token-only; `Phase::WholeMarking` rule's span covers a full marking. A rule needing both phases MUST register two entries (one per phase) sharing a backend module — no `Phase::Both` escape hatch.
- **FR-022**: For any pass-1 `AppliedFix` with span S₁ and any pass-2 `AppliedFix` with span S₂, S₁ ∩ S₂ MUST be ∅. Pass-2 diagnostics overlapping pass-1 spans MUST demote to suggestions (not auto-applied).
- **FR-023**: `Phase::WholeMarking` rules whose span overlaps a pass-1 fix MUST re-validate against pre-pass-1 attributes (cached from pass-0; supplied via `RuleContext.pre_pass_1_attrs: Option<&CanonicalAttrs<'src>>`) before firing. Disambiguation when the predicate holds against both pre- and post-pass-1 attrs: same `RuleId` (or same `(scheme, predicate-id)` key) → do not re-fire; different rule → fire (different predicate that pass-1 didn't address).
- **FR-024**: When `parse(post_pass_1_buffer)` fails, marque MUST emit `R002 — pass-1 fix produced unparseable buffer` carrying the contributing pass-1 fix IDs, retain the pass-1 audit records, return the pass-1 buffer as the corrected document, and not run pass-2.
- **FR-042**: A new `Severity::Suggest` variant MUST be introduced in `marque-rules` to represent the FR-022 demotion target (pass-2 fix overlapping a pass-1 span). Semantics: surfaced in CLI / IDE / WASM output as a user-actionable suggestion; NOT auto-applied by `Engine::fix`; NDJSON serialization is the lowercase string `"suggest"`. The variant MUST sort below `Info` for severity-ordering purposes (it is advisory). Exit-code semantics: `Severity::Suggest` does NOT trigger `EX_DIAG_WARN` (suggestions are non-blocking), distinct from `Severity::Warn`. The full severity set post-refactor is `Off | Suggest | Info | Warn | Error | Fix`.

#### Rule emission API

- **FR-025**: Rules MUST emit `FixIntent<S>` values. The engine MUST be the only path that promotes intent into an `AppliedFix` record by rendering through `MarkingScheme::render_canonical`. External rule crates MUST emit `FixIntent<S>` and MUST NOT construct `Canonical<S>` directly; the engine holds the sealed `CanonicalConstructor<S>` impl.
- **FR-026**: Rule identifiers MUST migrate from `E###`/`W###`/`S###`/`C###` to `(scheme, predicate-id)` keys (e.g., `("capco", "banner.classification.usa-trigraph")`).
- **FR-043**: `MarkingScheme::canonicalize(parsed: ParsedAttrs<'_>) -> CanonicalAttrs` MUST be the single explicit trait-method path that converts parser output into the canonical form rules consume. PR 3a's `from_parsed_unchecked` adapter exists transitionally during the keystone window (3a → 3c) and MUST delete at PR 3c. Post-keystone, rule crates MUST consume `CanonicalAttrs` produced only via `MarkingScheme::canonicalize` — no public `ParsedAttrs → CanonicalAttrs` constructor MAY exist outside the trait. Canonicalization is a scheme decision; rule crates do not own it.

#### Decoder constraints

- **FR-027**: Decoder MUST NOT canonicalize open-vocabulary tokens. Decoder-recognized closed-CVE tokens produce `Canonical<S>` via `Canonical::from_cve(TokenId, ...)`. Decoder-recognized open-vocabulary tokens produce `Parsed::Ambiguous` with diagnostic-only output — no `FixProposal` is emitted.
- **FR-028**: The `engine.rs::build_decoder_diagnostic` carve-out (`proposal.original = ""` branch around the `FixProposal::new(..., "", replacement, ...)` call — currently `engine.rs:1369-1384`) MUST be deleted at the cutover PR.

#### Performance budgets (preserved through refactor)

- **FR-029**: Single-pass forward splice MUST replace per-fix `Vec::splice`. Fix application MUST scale linearly in input size and fix count (R² ≥ 0.9 in `fix_throughput` Criterion bench, gated by `bench-check.sh`).
- **FR-030**: The interactive latency budget (p95 ≤ 16 ms on 10 KB single-portion inputs) MUST be preserved through the refactor. PR 2's `shape_admits` indirect-call cost MUST be measured with a **p99 tail-percentile assertion**, not just mean — `Arc<dyn Vocabulary<S>>` precludes cross-crate devirtualization, so per-token vtable misses must be measured at the tail.
- **FR-031**: Multi-page projection latency (PR 6's `Scope::Page` cutover, measured by `lint_100kb_multipage` Criterion bench) MUST stay within `PageContext` baseline + 10%.
- **FR-032**: Two-pass re-parse cost (PR 7's pass split, measured by `fix_10kb` Criterion bench) MUST stay within the interactive latency budget (FR-030).
- **FR-033**: The §3.6 measurement-gating discipline MUST apply uniformly: >5% mean OR p99 regression on any of the four benches above MUST back out the change.

#### Audit schema cutover (clean break)

- **FR-034**: `MARQUE_AUDIT_SCHEMA` MUST validate against a single value at build time (no accept-list). The post-keystone schema name is `marque-1.0`; the `mvp-N` naming retires.
- **FR-035**: PR 3c MUST be a single audit-schema bump (`marque-mvp-2 → marque-1.0`) baking in: `FixReplacement::Strict | Decoder` discriminant; `Canonical<S>` provenance-tagged shape; `FixIntent<S>` audit fields; `(scheme, predicate-id)` rule-ID form; reserved slots for `FeatureId::PrecedingFixPenalty` and the R002 diagnostic class. PR 7 MUST NOT bump the schema (PrecedingFixPenalty and R002 fill reserved slots).
- **FR-036**: PR 8's `marque-priors-3` is a *priors-bake* schema bump, not an audit schema bump — it is independent of FR-035 per Phase D conventions.
- **FR-037**: No `marque-audit-reader` crate, no reader-only feature flag, no forward-readability commitment. Pre-cutover records MUST be unreadable by post-cutover binaries (type-level guarantee, not runtime concern; there are no pre-cutover records).

#### Process and test discipline

- **FR-038**: `Send + Sync` bounds on `Rule` and `Recognizer<S>` impls MUST be statically asserted via `static_assertions::assert_impl_all!` from `RuleSet::new()`.
- **FR-039**: Masking-pin discipline MUST be CI-enforced (AST-based lint at `tools/masking-pin-lint/`). Every `with_recognizer(StrictRecognizer)` test pin MUST carry either `// MASKING-PIN: tracks #NNN` (with an open tracked issue, GitHub-API verified, `closed_as_duplicate_of` chains followed mandatorily) or `// INTENTIONAL-STRICT: <reason>`. Unmarked pins MUST fail CI. A masking pin MUST be removed in the same PR that closes its tracked issue, with a regression test demonstrating fix necessity.
- **FR-040**: Promote-callsite discipline MUST be CI-enforced (AST-based lint at `tools/promote-callsite-lint/`). `AppliedFix::__engine_promote` and `EnginePromotionToken::__engine_construct` calls MUST originate from `Engine::fix_inner` in production code. The Constitution V Principle V carve-out for test fixtures (per FR-005) requires an inline comment at each call site naming the carve-out (e.g., `// Test-fixture carve-out per Constitution V`); the AST lint MUST verify the comment is present within 5 lines of the call and reject any unmarked carve-out site.
- **FR-041**: Synthetic engine diagnostics (R001 decoder recognition, R002 re-parse failure) MUST be minted by `marque-engine`, not by rule crates.
- **FR-044**: Synthetic engine diagnostics MUST carry the sentinel scheme `"engine"` in `(scheme, predicate-id)` form: `("engine", "r001.decoder-recognized")` for R001, `("engine", "r002.reparse-failed")` for R002. The `"engine"` scheme is reserved at PR 3c rule-ID retirement; it is not a valid `MarkingScheme` registration target. Rationale: R001/R002 are minted by the engine, not by a `MarkingScheme` impl — inheriting an active scheme's namespace (`("capco", "engine.r001.…")`) would lie about provenance. The sentinel keeps `("capco", …)` cleanly meaning "from a CAPCO rule" and is forward-compatible with future schemes (`("cui", …)`, `("nato", …)`, etc.).

#### PR 9 surface (parser separator spans, dissem position attribution, NATO marking handling)

- **FR-045**: Parser MUST track separator spans (`/`, `//`, whitespace boundaries) as first-class `Span` values in `ParsedAttrs<'src>` (#106). Required for the banner-validation rule reshape (FR-046) and downstream rendering. Separator spans MUST NOT carry token semantics — they are positional metadata only.
- **FR-046**: `ParsedAttrs<'src>`, `CanonicalAttrs`, and `ProjectedMarking` MUST split the single `dissem` field into position-attributed `dissem_us: Box<[DissemControl]>` and `dissem_nato: Box<[DissemControl]>` (#271 / 7B). Banner-validation rules MUST consume the split fields. Rationale: per CAPCO §H.8 / §H.9, US dissems and NATO dissems occupy distinct positions in the marking grammar; collapsing them loses position-attribution required for correct page-rollup.
- **FR-047**: Marque MUST recognize NATO-specific marking tokens — minimally ATOMAL and BOHEMIA (#246) — and MUST handle them as NATO-scope dissems via the FR-046 split. Closed-CVE values for these tokens land via the existing `Vocabulary<S>` build-time generation pipeline.
- **FR-048**: A NATO portion appearing in a US-classified document MUST trigger a declarative `Constraint` requiring `REL TO USA, NATO` derivation in the banner (#265). The constraint lands as data on `CapcoScheme` per the Phase B declarative-Constraint pattern (CLAUDE.md "Two-Layer Rule Architecture") — not as a procedural rule branch.

### Key Entities

- **`Canonical<S>`**: Provenance-tagged canonical replacement. `bytes: Box<str>` plus `source: TokenSource::Cve(TokenId) | OpenVocab { category, render_call_site }`. The only public constructor for closed-CVE tokens is `Canonical::from_cve(TokenId, Scope)`; open-vocab construction is restricted to `pub(crate) Canonical::from_render(...)` reachable only from `MarkingScheme::render_canonical` impls. External rule crates never construct `Canonical<S>` directly.
- **`FixIntent<S>`**: Rule-emission API. Rules emit `FixIntent<S>` values describing the fix in scheme-typed terms (target span, replacement category + token id or render directive, confidence axes); the engine renders to `Canonical<S>` and promotes to `AppliedFix` in `Engine::fix_inner`. Lands in PR 3c.
- **`AppliedFix`**: Audit record. Carries rule ID (`(scheme, predicate-id)` form), original span (no bytes), original BLAKE3 digest, `Canonical<S>` replacement (CVE-typed or open-vocab-typed), confidence (recognition × rule combined), timestamp, classifier ID (when present), dry-run flag. Constructed only by `Engine::fix_inner` (or test-fixture under carve-out).
- **`Diagnostic`**: Surface output. `message: Message { template: MessageTemplate, args: MessageArgs }` (closed enum + closed-set arg types), `severity`, `span`, `citation`, optional `FixProposal`.
- **`ParsedAttrs<'src>` / `CanonicalAttrs` / `ProjectedMarking`**: The pivot-type split (PR 3a). Replaces `IsmAttributes`'s overloaded role as parser output, post-canonical form, and page roll-up output. Rules consume `&CanonicalAttrs` (transitionally via `from_parsed_unchecked` adapter at PR 3a; directly post-PR 3c). Banner-validation rules consume `&ProjectedMarking`.
- **`Phase`**: Rule-registration tag — `Localized` (sub-token span only) or `WholeMarking` (full marking span). Engine enforces span-shape at registration; pass dispatch routes on this.
- **`Vocabulary<S>`**: Per-token metadata surface (existing). Extended by FR-015 (`shape_admits` for open-vocab admission) and FR-010 (`is_fdr_dissem` per-token field).
- **`Scope`**: Lattice projection scope — `Portion` vs `Page`. `MarkingScheme::project(Scope::Page, ...)` is the source of truth for banner rollup post-cutover.
- **`PageRewrite`**: Declarative page-level transformation (e.g., `capco/noforn-clears-rel-to`). Topologically scheduled by `marque-engine::scheduler` (writers before readers; cycles fail at `Engine::new`).
- **`MessageTemplate` / `MessageArgs`**: Closed enum of stable string templates plus closed-set arg types (TokenId, Span, BLAKE3, Confidence, FeatureId). The mechanism that makes audit-record content-ignorance a type invariant rather than a grep firewall.
- **`MASKING-PIN` / `INTENTIONAL-STRICT`**: Test-pin classification tags enforced by AST lint. Two masking pins inventoried (`core_error_isolation.rs` → #257, closes at PR 3c; `corpus_accuracy.rs` → #258, closes at PR 8). Five intentional-strict pins inventoried.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A deterministic NDJSON canary scan over the full five-corpus regression sweep finds **zero** verbatim input bytes inside any `Engine::fix_inner`-emitted `AppliedFix` JSON serialization (other than within span-offset numerals, BLAKE3 digests, or enumerated identifier values). Test-fixture records constructed under the Constitution V carve-out are excluded by construction.
- **SC-002**: A document containing only foreign portions (FGI-only page, NATO-only page, JOINT US/foreign page) produces a banner that correctly retains foreign markers in **100%** of corpus fixtures at `tests/corpus/foreign/` and `tests/corpus/lattice/`. Zero fixtures produce a silently-flattened US banner.
- **SC-003**: Cross-axis dominance fixtures at `crates/capco/tests/cross_axis_dominance.rs` (FOUO eviction by classification > U; FOUO eviction by non-FD&R dissem; FGI banner roll-up; SCI cross-system canonicalization; AEA exemption commingling with classification) pass at **100%** rate.
- **SC-004**: Every category in `CapcoScheme::categories()` passes lattice-law property tests (assoc/comm/idem/identity-with-bottom) at `crates/capco/tests/category_lattice_laws.rs` with **zero** failing properties.
- **SC-005**: The citation lint (AST-based, `tools/citation-lint/`) verifies **100%** of `§X.Y pNN` references across `citation:` fields, `message:` strings, `constraint_label:` strings, and doc-comment positions resolve to a real passage in the vendored authoritative source. Zero fabricated, drifted, or out-of-normative-range citations remain at the end of the refactor sequence.
- **SC-006**: F.1 corpus fidelity: **100%** of cited authorities have ≥1 corpus fixture exercising the predicate against the canonical example.
- **SC-007**: Two-pass invariant property tests (`crates/engine/tests/two_pass_invariants.rs`) pass under all fix-ordering permutations: zero overlapping spans across pass-1 / pass-2 promoted fixes; reshape-aware re-validation does not produce retroactive-satisfaction false positives.
- **SC-008**: Interactive latency budget preserved: p95 ≤ 16 ms and p99 within pre-refactor baseline + 5% on the 10 KB single-portion bench.
- **SC-009**: Multi-page projection (`lint_100kb_multipage` Criterion bench) within `PageContext` baseline + 10%; `fix_throughput` linear scaling R² ≥ 0.9; `fix_10kb` two-pass overhead within the SC-008 budget.
- **SC-010**: Mangled-corpus fix accuracy preserved at ≥ 0.85 OR intentionally re-anchored at PR 3c with a documented threshold and a re-curated corpus excluding open-vocab cases that were never legitimately fixable. Decision recorded in PR 3c review notes.
- **SC-011**: Open-vocabulary parser failures return `None` at all four cited parser sites; no `FgiMarker { countries: [] }` collision-shape values survive in either parser output or rule input. Verified by `tests/parser/fgi_silent_skip_guard.rs` plus a `marque-capco` rule audit confirming no surviving `countries.is_empty()` patterns.
- **SC-012**: Audit-record JSON for closed-CVE fixes is bit-for-bit reproducible from `(TokenId, Scope)` inputs (compile-fail tests demonstrate that `Box<str> → Canonical` paths do not exist for closed-CVE tokens; open-vocab fixes carry `render_call_site` provenance distinguishing them from CVE-typed canonicals).
- **SC-013**: At the end of the refactor sequence (PR 10 merge), zero MASKING-PIN tags reference issues that have closed; the AST-based masking-pin lint passes; the AST-based promote-callsite lint passes; both `static_assertions::assert_impl_all!` checks for `Send + Sync` bounds pass at workspace build.
- **SC-014**: Each PR in the keystone subsequence (3a, 3b, 3c) and PR 6 sub-commit sequence (6a, 6b, 6c) passes the corpus regression sweep independently in the CI matrix; any single PR is mechanically revertable without leaving orphaned types, functions, or dependencies.

## Assumptions

- **No downstream consumers of marque exist.** The clean-break philosophy
  (no audit-record reader compatibility, no `Vocabulary<S>` /
  `MarkingScheme` / `Codec<S>` semver stability, no `PageContext`
  equivalence shim) is predicated on this. The window for clean-break
  refactor closes when external consumers attach.
- **CAPCO is the only `MarkingScheme` in-tree until further notice.** A
  second scheme (CUI, NATO, partner-national) does not land during this
  refactor sequence. `Vocabulary<S>`, `MarkingScheme`, and `Codec<S>`
  ship `#[doc(hidden)] pub` semver-unstable; they will change on contact
  with scheme #2 and that is the accepted cost.
- **Constitution V Principle V's test-fixture carve-out remains in
  effect.** Test code (`#[cfg(test)]` / `tests/` / `dev-dependencies`-gated
  test-utility crates) MAY call `__engine_promote` to construct synthetic
  `AppliedFix` fixtures, scoped to test-fixture construction only,
  never commingled with engine output, never `cfg(not(test))`-reachable.
- **PR #277 / #278's single-pass forward splice has already landed.**
  The `fix_throughput` Criterion bench is wired into `bench-check.sh` and
  passes R² ≥ 0.9 at the start of this refactor sequence.
- **The third problem class (recognizer scoring quality) is not closed
  by this refactor.** Issues #258 (decoder prose null hypothesis) and
  #260 (decoder folds bare NATO levels) belong to recognizer scoring
  work and ship in PR 8 as standalone delivery, not as G13 closure.
  This spec does not claim closure of #258 or #260.
- **Issues #266 (CAB Declassify On canned strings for AEA / NATO
  commingling) and 7C (`Vocabulary<S>::TokenId` distinguishing US
  ORCON from NATO ORCON despite same surface) are deferred indefinitely.**
  They appear in Appendix C of the consolidated plan.
- **The lattice §-resolution spike (PR 3.7) gates PR 4 absolutely.**
  If PR 3.7 stalls, PRs 4–10 stall. Default owner is the consolidated-plan
  author or named successor in the PR description; default deadline is
  2 weeks from PR 3c merge; deadline slip requires explicit team review.
- **Build-time line-number anchors will drift.** Specific line-number
  references in `engine.rs:1369-1384`, `scheme.rs:1734/1783/1787/etc.`,
  `rules.rs:2022/2148/2609/2919/10142`, `parser.rs:1011-1024/:1453/:1481/:1493`
  are indicative — implementer re-greps at edit time. Defect classes are
  stable; line numbers are not.

## Out of Scope

- **`marque-audit-reader` crate or any forward-readability commitment.**
  Explicitly not scheduled. No downstream consumers; clean break.
- **A second `MarkingScheme` adoption** (e.g., a `marque-cui` rule crate).
  CAPCO-first; the trait surface is acknowledged semver-unstable until a
  second scheme attaches. The `marque-cui` placeholder at `crates/cui/`
  remains a stub through this refactor.
- **#266 CAB Declassify On canned strings (§C.4 AEA, §C.5 NATO).**
  Out of immediate scope per user direction; tracked separately.
- **#258 / #260 closure.** Recognizer scoring quality is a third
  problem class. PR 8 delivers priors and folding logic but does not
  claim closure of these issues.
- **`marque-extract` Kreuzberg backend implementation.** The format-
  extraction crate stays scaffolded with `Extractor` / `ExtractedDocument`
  surface; the Kreuzberg dependency stays commented-out pending a
  licensing decision. Format extraction is not on the critical path of
  this refactor.
- **`fast-typing` / per-keystroke optimization** beyond the existing
  debounce expectation on consumers. The decoder-default-on dispatch
  (PR #259) is the upstream of this expectation; this refactor does
  not change the dispatch shape.
- **Centralizing R001 / R002 / future synthetic engine diagnostic IDs
  into `marque-rules`.** Noted as a separate refactor in §9.4 of the
  primary plan; not in scope here.
- **8B citation-passage extraction** (extract the cited passage text
  alongside the §-citation for richer rendering). Demoted from the
  citation-fidelity gate composition; nice-to-have.

## Dependencies

- The vendored authoritative source `crates/capco/docs/CAPCO-2016.md`
  (and the PDF original at `crates/capco/docs/original-refs/CAPCO-2016.pdf`)
  is the single source of truth for citation verification (FR-018).
- The vendored ODNI ISM XML schemas at `crates/ism/schemas/ISM-v2022-DEC/`
  drive Layer 1 generated predicates consumed by Layer 2 rules
  (Constitution IV).
- The corpus harness at `crates/capco/corpus/` (corpus-derived priors) is
  the input for SC-010 baseline and PR 8's priors-bake.
- The Criterion bench harness (`fix_throughput` already landed; `fix_10kb`,
  `lint_100kb_multipage`, the SC-008 latency bench with p99 added by this
  refactor) wired into `tools/bench-check.sh` is the gate enforcement
  mechanism for FR-029 through FR-033.
- The `recoco-utils::ConcurrencyController` provides batch-engine
  semaphore backpressure; not modified by this refactor but a
  load-bearing dependency for `BatchEngine` correctness through the
  refactor.
- The `static_assertions` crate provides compile-time `Send + Sync`
  enforcement (FR-038); already a workspace dependency.
- The CI infrastructure must support AST-based lints at
  `tools/masking-pin-lint/`, `tools/promote-callsite-lint/`, and
  `tools/citation-lint/` (FR-039, FR-040, FR-018).
