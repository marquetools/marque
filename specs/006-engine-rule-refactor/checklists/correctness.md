<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Correctness Requirements Checklist: Engine + Rule Architecture Refactor

**Purpose**: Validate that requirements covering refactor *correctness* —
lattice algebra & page roll-up, two-pass apply, open-vocabulary parser,
citation fidelity (mechanical), and semantic agreement with the CAPCO
ruleset — are complete, clear, consistent, and measurable. Companion to
`requirements.md` (which validates spec form); this file validates spec
substance against CAPCO-2016 and the known-defect register.

**Created**: 2026-05-04
**Feature**: [spec.md](../spec.md)
**Source**: `crates/capco/CAPCO-CONTEXT.md` (curated CAPCO baseline);
`crates/capco/docs/CAPCO-2016.md` (authoritative); spec FR/SC/D refs.

**Focus areas** — Q1: B (lattice/rollup, US2+US6) + C (two-pass + open-vocab,
US3+US4) + D (citations, US5) + Semantic agreement with CAPCO ruleset (new).
**Audience / depth** — Q2: PR-author sanity (light) for most items;
**reviewer gate** for items tagged `[GATE]` — these MUST clear before the
P1 PRs (0.6 citation fix, 3a/3b/3c keystone, 5/6 foreign banner, 3.7 lattice
spike) merge.
**Must-haves** — Q3: known-defect channels in §6 are forced items
(#106, #246, #257, #261, #265, #271, #276, R001 message,
`build_decoder_diagnostic` carve-out, four citation-defect classes from
FR-020).

> **Convention**. Items follow "unit tests for English" — each asks a
> question about whether the *requirements text* is well-written, not
> whether the implementation works. Tags: `[Completeness]` `[Clarity]`
> `[Consistency]` `[Coverage]` `[Measurability]` `[Edge Case]` `[Gap]`
> `[Conflict]` `[Ambiguity]` `[Traceability]` `[GATE]`. References
> `[Spec §FR-NNN]` cite the spec; `[CAPCO-CONTEXT §X.Y]` cite the
> agent helper; `[CAPCO §X.Y pNN]` cite the manual; `[GH #NNN]` cite
> the issue tracker.

---

## 1. Lattice Algebra & Page-Level Rollup (US2, US6)

- [ ] CHK001 - Are foreign-only banner requirements separately specified for FGI-only, NATO-only, and JOINT US/foreign pages, with each page class producing a distinct expected-banner shape? [Completeness, Spec §US2 / SC-002] [GATE]
- [ ] CHK002 - Does FR-007's prohibition on hardcoding `MarkingClassification::Us` apply to **every** projection function in the codebase, not just the named site at `crates/capco/src/scheme.rs:365`? [Completeness, Spec §FR-007 / GH #276] [GATE]
- [ ] CHK003 - Is `expected_classification() -> Option<MarkingClassification>` consistent with the SC-002 acceptance criterion that pure-foreign pages produce a representable absent-US result? [Consistency, Spec §FR-007 / SC-002]
- [ ] CHK004 - Does the FGI banner roll-up requirement distinguish *concealed* (`FGI`, no list, per §H.7 p123 "when source must be concealed") from *acknowledged* (`FGI [LIST]`, country trigraphs)? [Completeness, Spec §FR-008 / FR-017 / CAPCO-CONTEXT §5.6]
- [ ] CHK005 - Is the rule "mixing concealed + acknowledged FGI portions on the same page → bare `FGI` banner (no list)" specified? [Gap, CAPCO §D.2 + §H.7 p123 / CAPCO-CONTEXT §3.1]
- [ ] CHK006 - Is the JOINT non-rollup behavior specified — that JOINT portions stay at portion and the banner becomes the highest US class with FGI [LIST] + REL TO union, NOT `JOINT [class] [LIST]` in the banner of US documents? [Gap, CAPCO §H.3 p56 / CAPCO-CONTEXT §5.2] [GATE]
- [ ] CHK007 - Are the four lattice laws (associativity, commutativity, idempotency, identity-with-bottom) named individually with property-test acceptance criteria, or treated as one bundled requirement? [Measurability, Spec §FR-011 / SC-004]
- [ ] CHK008 - Is the cross-axis dominance fixture set complete — does FR-012 enumerate FOUO eviction by classification > U, FOUO eviction by non-FD&R dissem, FGI banner roll-up, SCI cross-system canonicalization, AND AEA exemption commingling? [Completeness, Spec §FR-012 / SC-003]
- [ ] CHK009 - Does FR-014 explicitly state that no PR in the keystone sequence (3a / 3b / 3c) consumes `CapcoMarking::join`'s `PageContext` delegation, so the no-shim deletion is safe to land in one cutover? [Consistency, Spec §FR-014]
- [ ] CHK010 - Are formal join semantics (precondition / postcondition functional form) required for *every* category in `2026-05-01-lattice-design.md` §§2–8, or does the spec permit prose-only entries? [Clarity, Spec §FR-013] [GATE]
- [ ] CHK011 - Is FOUO's category-membership requirement (U-only, supersedes within dissem set when non-FD&R dissem present) consistent with CAPCO §H.8 p134 ("U only" + "rolls up on U docs only when no other dissem present")? [Consistency, Spec §FR-009 / CAPCO §H.8 p134]
- [ ] CHK012 - Is the FD&R-vs-non-FD&R distinction in FR-009 quantified as `Vocabulary<S>` metadata, OR is the boundary specified inline (creating a drift risk between FR-009 and FR-010)? [Clarity, Spec §FR-009 / FR-010]
- [ ] CHK013 - Are scope-shape requirements specified for `MarkingScheme::project(Scope, ...)` such that `Scope::Page`, `Scope::Document`, and any future `Scope::Section` cannot collapse into one `Scope::Whatever`? [Coverage, Spec §FR-006]
- [ ] CHK014 - Is the §3.3a "equal-depth meet policy" for `SciSet`/`SarSet`/`FgiSet` lattices documented in lattice-design §§2–8 with worked examples for unequal-depth inputs? [Coverage, Spec §FR-013 / CLAUDE.md "Phase B canonicalization"]

## 2. Two-Pass Apply Quality (US3)

- [ ] CHK015 - Is "no `Phase::Both` escape hatch" stated as a hard rule (a rule needing both phases registers two entries), or is the "no Both" requirement softened by allowing per-rule overrides? [Clarity, Spec §FR-021] [GATE]
- [ ] CHK016 - Is the disambiguation rule for `Phase::WholeMarking` re-validation specified for *every* combinatoric branch (predicate holds against pre-pass-1 only / post-pass-1 only / both / neither)? [Completeness, Spec §FR-023]
- [ ] CHK017 - Is the "same `RuleId` (or same `(scheme, predicate-id)` key) → do not re-fire" disambiguation consistent with FR-026's rule-ID migration (during PR 4–10 a rename could alias previously-distinct predicates)? [Consistency, Spec §FR-023 / FR-026 / FR-049]
- [ ] CHK018 - Is the R002 (pass-1 fix produced unparseable buffer) message-template, span shape, audit-record shape, and exit-code contract specified per D1's consumer surface? [Completeness, Spec §FR-024 / D1] [GATE]
- [ ] CHK019 - Is the "retain pass-1 audit records, return pass-1 buffer as the corrected document, do not run pass-2" sequence atomic — is partial-failure recovery between those three steps specified? [Edge Case, Spec §FR-024]
- [ ] CHK020 - Is `Severity::Suggest`'s relationship to the FR-008 invariant "an `Off`-severity diagnostic is unrepresentable" clarified — does a rule registered at `Suggest` always produce an advisory diagnostic (firing), distinct from `Off` (non-firing)? [Coverage, Spec §FR-042 / FR-008, Gap]
- [ ] CHK021 - Is the NDJSON serialization shape for `Severity::Suggest` (`"suggest"` lowercase, advisory exit-code semantics) specified consistently across CLI, WASM, and server consumers — or stated for one channel and assumed for the others? [Consistency, Spec §FR-042 / D1]
- [ ] CHK022 - Is the "S₁ ∩ S₂ MUST be ∅ for any pass-1/pass-2 promoted-fix span pair" invariant specified with property-test fixtures covering all fix-ordering permutations? [Measurability, Spec §FR-022 / SC-007]
- [ ] CHK023 - Are the contents of `RuleContext.pre_pass_1_attrs` for demoted-to-`Suggest` pass-2 fixes specified — does the surfaced suggestion display the pre-pass-1 view, post-pass-1 view, or both? [Gap, Spec §FR-022 / FR-023 / FR-042]
- [ ] CHK024 - Is the `RuleContext.pre_pass_1_attrs: Option<&CanonicalAttrs<'src>>` field's lifetime relative to pass-2 evaluation specified (cached from pass-0; supplied for the duration of pass-2 only)? [Clarity, Spec §FR-023]

## 3. Open-Vocabulary Parser Quality (US4)

- [ ] CHK025 - Is the inventory of four open-vocabulary admission sites in `marque-core/parser.rs` complete (`:1453`, `:1481`, `:1493`, `:1011-1024`), and is each site's migration target named individually? [Completeness, Spec §FR-015]
- [ ] CHK026 - Is the `shape_admits` admission contract specified by *what predicate the function must satisfy* — character classes, length bounds, prefix/suffix, locale — not just by the call sites that route through it? [Clarity, Spec §FR-015] [GATE]
- [ ] CHK027 - Is the requirement "`parse_fgi_marker` returns `None` (not `Some` with degraded structure) on shape failure" consistent with FR-017's `FgiMarker` discriminant requirement (post-failure shape MUST be unrepresentable)? [Consistency, Spec §FR-016 / FR-017]
- [ ] CHK028 - Does FR-017 specify `FgiMarker` such that the `countries: []` collision shape is *type-system-unrepresentable* (variant-discriminated), not just discouraged by review? [Clarity, Spec §FR-017 / SC-011] [GATE]
- [ ] CHK029 - Is the audit boundary for "rules using `countries.is_empty()` MUST be migrated" specified — which rule crates, which rule files, what evidence of migration completeness? [Completeness, Spec §FR-017]
- [ ] CHK030 - Does FR-015's CI grep guard against re-introduction of `is_ascii_alphanumeric()` in parser open-vocab admission paths specify the regex pattern and file scope, or only the intent? [Gap, Spec §FR-015]
- [ ] CHK031 - Is `shape_admits` required to be `Send + Sync` and cross-thread-safe, given `Arc<dyn Vocabulary<S>>` precludes cross-crate devirtualization (FR-030 implication)? [Coverage, Spec §FR-015 / FR-030]

## 4. Citation Fidelity (Mechanical, US5)

- [ ] CHK032 - Are the four pre-existing citation-defect classes from FR-020 (a) `§4` fabrications, (b) doubled `p150–151 p151`, (c) SIGMA cross-revision archaeology, (d) two-sided HCS-P predicate — each specified with both the *defect* AND the *corrected target*? [Completeness, Spec §FR-020] [GATE]
- [ ] CHK033 - Is FR-018's enumeration of citation locations (`citation:` fields, `message:` strings, `constraint_label:` strings, doc-comment `§X.Y`) complete — does it cover NDJSON-serialized audit records, error-stream messages, and any other surface where a `§X.Y pNN` reference might appear? [Coverage, Spec §FR-018, Gap]
- [ ] CHK034 - Is the rejection of bare `§NN` (without subsection) consistent with the line-anchor assumption that line numbers are not stable (Spec §Assumptions "Build-time line-number anchors will drift")? [Consistency, Spec §FR-018 / Assumptions]
- [ ] CHK035 - Is "every Constraint/PageRewrite/Rule cited authority MUST have ≥1 corpus fixture" measurable per-citation, with a CI gate that maps each cited `§X.Y pNN` to a fixture file path? [Measurability, Spec §FR-019 / SC-006]
- [ ] CHK036 - Is the §I–K boundary specified — that CAPCO §I (history), §J (examples), §K (acronyms) are NOT valid citation targets even though they are present in the manual? [Gap, Spec §FR-018]
- [ ] CHK037 - Does the AST-based citation-lint specification cover *misattribution* (citation resolves to a real passage, but claims A about §X.Y when §X.Y says B), or only *non-resolution*? [Coverage, Spec §FR-018 / SC-005, Gap]
- [ ] CHK038 - Is the citation-lint's behavior for citations that span multiple pages (e.g., `§H.7 pp 122–130`) specified — must each end of the range resolve, or only the first page? [Edge Case, Spec §FR-018, Gap]
- [ ] CHK039 - Is the requirement for citation re-verification on propagation (Constitution Principle VIII: "Propagation requires re-verification") reflected in the citation-lint behavior — does the lint distinguish a citation moved into a new context from a citation written in place? [Gap, Constitution VIII]

## 5. Semantic Agreement with CAPCO Ruleset (Tangential to Citations)

> Citation fidelity (§4) tests whether `§X.Y pNN` resolves and is well-formed.
> This section tests whether what the spec **says** about the rule actually
> matches what the cited section **states** — i.e., whether the requirements
> would produce CAPCO-correct output even if every citation were perfectly
> formed.

### 5.1 FD&R (Table 2 + Table 3)

- [ ] CHK040 - Is FD&R Table 2 (CAPCO §B.3 pp 21–22) fully encoded — all seven row conditions including the 28 Jun 2010 pivot — mapped to a Warn-level rule shape with confidence levels reflecting source dictation strength? [Completeness, CAPCO-CONTEXT §2 / CAPCO §B.3] [GATE]
- [ ] CHK041 - Does the spec specify how `Authority::Originated` is read from the CAB so the 28 Jun 2010 pivot is decidable from document data, not inferred from text? [Clarity, CAPCO-CONTEXT §2 "Pivot date"]
- [ ] CHK042 - Is the caveats list (ORCON, ORCON-USGOV, IMCON, PROPIN, FISA, DEA SENSITIVE, RSEN, FOUO, non-IC dissems) specified as a closed set so a future dissem addition cannot silently change the "caveated" predicate? [Completeness, CAPCO-CONTEXT §2 "Caveats"]
- [ ] CHK043 - Is the "apply-with-fix permissible only when date pivot is unambiguous AND caveat status decidable from portion text" requirement stated as a CI-checkable guard, not as guidance? [Clarity, CAPCO-CONTEXT §2 "Marque encoding"]
- [ ] CHK044 - Is Table 3 rule 17 (RELIDO + no-FD&R → ambiguous NOFORN-or-RELIDO depending on origination date + non-FD&R caveats) addressed with explicit ambiguity handling, or treated as a tracked deferral? [Gap, CAPCO-CONTEXT §3.2 row 17 / §3.4 marque gap] [GATE]
- [ ] CHK045 - Is Table 3 rule 23 (TEYE/ACGU/FVEY tetragraph **expansion** during common-LIST roll-up) specified with a named rule-emission path, or omitted? [Coverage, CAPCO-CONTEXT §3.4 marque gap] [GATE]
- [ ] CHK046 - Is Table 3 rule 26 (cross-axis "REL TO + DISPLAY ONLY → DISPLAY ONLY [common LIST]" because release implies disclosure) specified? [Gap, CAPCO-CONTEXT §3.4 marque gap] [GATE]
- [ ] CHK047 - Is Table 3 rule 27 (dual-channel REL TO/DISPLAY ONLY composition where each channel has its own common LIST) specified? [Gap, CAPCO-CONTEXT §3.4 marque gap] [GATE]

### 5.2 Marking Order (Table 4)

- [ ] CHK048 - Is the within-group dissem ordering rule (§G.1 / §H.8 prose: `OC/NF` correct, `NF/OC` not — same set, fixed order) specified with a target rule slot or explicitly deferred to PR 9 / a tracked issue? [Gap, CAPCO-CONTEXT §4.2 / §4.3 marque gap]
- [ ] CHK049 - Is the within-category SCI/SAP ascending-sort rule (numeric first, alphabetic after — separators per §A.5) specified with a corpus fixture asserting deterministic order? [Coverage, CAPCO-CONTEXT §4.3 marque gap]
- [ ] CHK050 - Is the inter-category `//` ordering specified to follow Table 4's nine top-level groups exactly (US class → non-US class → JOINT → SCI → SAP → AEA → FGI → IC dissem → non-IC dissem)? [Completeness, CAPCO-CONTEXT §4.1]

### 5.3 Per-Marking Grammar (§H.4 SCI / §H.5 SAR / §H.6 AEA / §H.7 FGI / §H.8–§H.9 dissem)

- [ ] CHK051 - Is the SCI grammar (compartment 2–3 alpha for SI, 3 alnum for RSV; sub-compartment 4–6 alnum varies by system; separators `/` between control systems, `-` between control and compartment, ` ` between sub-compartments per §A.6 (Formatting) pp 15–17 + §H.4 p61) reflected in the parser shape requirement? [Completeness, CAPCO-CONTEXT §5.3 SCI grammar reminder] [GATE]
- [ ] CHK052 - Is the SAR multi-program separator (modern: `SAR-A/B/C`, no `SAR-` repeat per program) specified, AND legacy repeated-prefix tolerance addressed by historical-batch correction? [Coverage, CAPCO-CONTEXT §1.2 Mnemonic / §5.4]
- [ ] CHK053 - Is the SAR hierarchy (program 2–3 char abbr → compartment via `-` → sub-compartment via space; multi-value within level ascending sort numeric-first) reflected in parser shape and rule fixtures? [Completeness, CAPCO-CONTEXT §5.4 SAR grammar reminder]
- [ ] CHK054 - Is the AEA full-eviction precedence (RD evicts FRD and TFNI from both banner and portion per CAPCO §H.6 p104; RD-SIGMA evicts FRD-SIGMA at both levels) specified in cross-axis dominance fixtures? [Completeness, CAPCO-CONTEXT §5.5 / Spec §FR-012]
- [ ] CHK055 - Is the CNWDI requirement "TS RD or S RD only; subset of RD" + the dual statement obligation (RD warning + CNWDI identifying statement on first page, separate text boxes) specified? [Completeness, CAPCO-CONTEXT §5.5]
- [ ] CHK056 - Is the SIGMA "current set is 14, 15, 18, 20" restriction specified (so a portion claiming SIGMA-99 is rejected, not silently accepted)? [Coverage, CAPCO-CONTEXT §5.5]
- [ ] CHK057 - Is the DOD UCNI / DOE UCNI "U only; not on classified content; on classified docs the marking does NOT appear in banner but NOFORN must be applied if FD&R less restrictive" rule specified? [Completeness, CAPCO-CONTEXT §5.5]
- [ ] CHK058 - Is the FGI portion-mark "always starts with `//`" (because foreign source is its own classification authority) specified for the parser/render path? [Completeness, CAPCO-CONTEXT §5.6 FGI grammar reminder]
- [ ] CHK059 - Is the country-list separator distinction (`,` for REL TO / DISPLAY ONLY, ` ` for FGI / JOINT, deprecated `/` for EYES ONLY) specified at the parser level so a delimiter conflation cannot silently produce wrong markings? [Coverage, CAPCO-CONTEXT §1.2 "Mnemonic for delimiter conflation"] [GATE]
- [ ] CHK060 - Is the JOINT portion-mark requirement that the portion MUST always carry `REL TO USA, LIST` (or `REL` when the list matches the banner) per CAPCO §H.3 p56 specified, and is REL TO mandatory (not optional)? [Completeness, CAPCO-CONTEXT §5.2]
- [ ] CHK061 - Is the ORCON / ORCON-USGOV mutual-exclusion + "OC wins in portion" rule specified, including the cross-axis exclusions with HCS-O / HCS-P [SUB] / SI-G / SI-G [SUB]? [Completeness, CAPCO-CONTEXT §5.7 / CAPCO §H.8 p139]
- [ ] CHK062 - Is the HCS-O / HCS-P "requires NOFORN" + HCS-O "requires ORCON; not with ORCON-USGOV" predicate two-sidedness preserved (over-strict on optional ORCON AND under-strict on missing NOFORN — the FR-020 (d) defect)? [Consistency, Spec §FR-020 / CAPCO-CONTEXT §5.3]
- [ ] CHK063 - Is the §H.9 non-IC dissem precedence (NODIS wins over EXDIS; LIMDIS / SBU / SBU-NF / LES / LES-NF / SSI all win over FOUO) specified for both banner and portion? [Completeness, CAPCO-CONTEXT §5.8]
- [ ] CHK064 - Is the EXDIS / NODIS "requires NOFORN; REL TO not authorized in banner if any EXDIS/NODIS portion present" cross-axis rule specified? [Completeness, CAPCO-CONTEXT §5.8 / CAPCO §H.9 pp 172, 174]
- [ ] CHK065 - Is the EYES ONLY "NSA only, deprecated, markings waiver expired 1 Oct 2017" status specified — and "extracting EYES portions into new docs converts to REL TO" addressed as a fix path? [Coverage, CAPCO-CONTEXT §5.7 / CAPCO §H.8 p157]
- [ ] CHK066 - Is the ATOMAL / BOHEMIA categorization (ATOMAL = NATO AEA marking, BOHEMIA = NATO control-system marking — NOT `dissem_nato` values) consistent across FR-046 (`dissem_nato` reserved for true NATO dissems) and FR-047 (closed-CVE values via build-time generation)? [Consistency, Spec §FR-046 / FR-047 / GH #246]

## 6. Known-Defect Coverage (Forced Items)

- [ ] CHK067 - Is GH #257 (decoder canonicalization uppercases unrecognized middle tokens, leaks input bytes into `proposal.replacement`) addressed by a specific FR or noted as out-of-scope with a tracked-issue link? [Completeness, GH #257 / Spec §G13] [GATE]
- [ ] CHK068 - Is GH #276 (`page_context_to_attrs` hardcodes `MarkingClassification::Us` at `crates/capco/src/scheme.rs:365`) addressed by FR-007 with a CI grep gate against re-introduction? [Coverage, Spec §FR-007 / GH #276] [GATE]
- [ ] CHK069 - Is GH #261 (FGI banner emits redundant `FGI` token when country trigraph is present) addressed and consistent with FR-008's render-canonical guidance ("drop the redundant FGI token only when a country trigraph is present")? [Consistency, Spec §FR-008 / GH #261]
- [ ] CHK070 - Is GH #271 / 7B (single `dissem` field collapsed across US+NATO axes, losing position-attribution) addressed by FR-046 splitting into `dissem_us` and `dissem_nato`, AND are all banner-validation rules required to migrate to the split fields? [Coverage, Spec §FR-046 / GH #271]
- [ ] CHK071 - Is GH #246 (NATO-specific tokens ATOMAL / BOHEMIA not recognized) addressed in FR-047 with closed-CVE values landing via the `Vocabulary<S>` build-time generation pipeline (NOT inline string literals)? [Coverage, Spec §FR-047 / GH #246]
- [ ] CHK072 - Is GH #265 (NATO portion in US-classified document requires `REL TO USA, NATO` in banner) addressed by FR-048 as a *declarative* `Constraint` on `CapcoScheme`, not a procedural rule branch? [Coverage, Spec §FR-048 / GH #265]
- [ ] CHK073 - Is GH #106 (parser does not track separator spans) addressed by FR-045 with the position-only metadata constraint (separator spans MUST NOT carry token semantics)? [Clarity, Spec §FR-045 / GH #106]
- [ ] CHK074 - Is the R001 message leak channel (decoder recognition message interpolating raw bytes via `format!`) addressed by FR-003's `MessageTemplate` enum + closed-set `MessageArgs` requirement, such that arbitrary `format!` interpolation is unrepresentable? [Coverage, Spec §FR-003] [GATE]
- [ ] CHK075 - Is the `engine.rs::build_decoder_diagnostic` carve-out (`proposal.original = ""` branch around `engine.rs:1369-1384`) deletion specified at a binding PR cutover, with a precise removal target (FR-028 names the cutover PR)? [Clarity, Spec §FR-028]
- [ ] CHK076 - Is the `provenance.canonical_bytes` decoder-path contamination channel (uppercased input segments) addressed alongside #257 in the audit-record content-ignorance guarantee? [Coverage, Spec §FR-002 / G13]
- [ ] CHK077 - Are the four FR-020 citation-defect classes (a–d) specified with distinct corpus regression tests so a partial fix (e.g., correcting `§4` → `§H.4` but leaving the doubled `p150–151 p151`) cannot pass the gate? [Completeness, Spec §FR-020] [GATE]

## 7. Process Discipline & Audience Signal

> The PR sequence is the contract. These items test whether the
> requirements give a PR-author or a reviewer the signal they need to
> accept or reject a PR in the 0 → 0.5 → 0.6 → 1 → 2 → 3a → 3b → 3c →
> 5 → 6a → 6b → 6c → 7 → 3.7 → 4 → 8 → 9 → 10 sequence.

- [ ] CHK078 - Is the K-Option-2 attribution carve-out in SC-010 ("if loss is not K-Option-2-attributable per R-8, revert as a unit") itself measurable — what evidence makes a loss K-Option-2-attributable, and who decides at merge time? [Measurability, Spec §SC-010 / D5, Gap] [GATE]
- [ ] CHK079 - Is the PR 3.7 lattice-spike stall-recovery procedure (named alternate owner who has read §§2–8 of `2026-05-01-lattice-design.md` *before PR 3c merges*; alternate takes ownership without escalation if primary stalls past 1 week) specified before PR 3c can merge? [Completeness, Spec §Assumptions / D2] [GATE]
- [ ] CHK080 - Is the "PR 6 sub-commit sequence (6a / 6b / 6c) passes corpus regression independently in CI matrix" requirement consistent with the keystone-revertable property at SC-014? [Consistency, Spec §SC-014]
- [ ] CHK081 - Is the PR 0 baseline bench-hardware pin (rented bare-metal vs. dedicated GitHub-hosted runner spec) recorded as a binding decision in the PR 0 description, AND is the decision durable for the full refactor duration? [Completeness, Spec §FR-050 / D8]
- [ ] CHK082 - Is "rule-ID stability begins at PR 10 merge" specified with explicit allowance for predicate renames during PR 4–10, AND a requirement that the PR-10 merge commit updates `docs/refactor-006/legacy-rule-id-map.md`? [Clarity, Spec §FR-049 / D6]
- [ ] CHK083 - Is the per-PR cumulative bench-drift attribution rule (>6% per-PR contribution flagged for attribution; ≤10% cumulative end-state) measurable at PR-author granularity, not only at PR-10 retrospective? [Measurability, Spec §FR-050 / D8]
- [ ] CHK084 - Is the FR-051 flake-queue cap (10 entries) calibrated against any baseline flake rate, or chosen heuristically — and is the rationale for the chosen value recorded in D16? [Clarity, Spec §FR-051 / D16]
- [ ] CHK085 - Is the masking-pin discipline (`with_recognizer(StrictRecognizer)` test pins MUST carry `// MASKING-PIN: tracks #NNN` or `// INTENTIONAL-STRICT: <reason>`; unmarked pins fail CI; closed_as_duplicate_of chains followed mandatorily) consistent with FR-039's requirements? [Consistency, Spec §FR-039 / D11]

---

## Summary

- **Item count**: 85 items.
- **Coverage by section**: §1 lattice/rollup 14, §2 two-pass 10, §3 open-vocab 7, §4 citation mechanical 8, §5 CAPCO semantic agreement 27, §6 known defects 11, §7 process 8.
- **GATE items**: 22 items tagged `[GATE]` — these MUST clear before the corresponding P1 PR (0.6 citation fix, 3a/3b/3c keystone, 5 / 6 foreign banner, 3.7 lattice spike) merges. Authoritative GATE list (for unambiguous recounts): CHK001, CHK002, CHK006, CHK010, CHK015, CHK018, CHK026, CHK028, CHK032, CHK040, CHK044, CHK045, CHK046, CHK047, CHK051, CHK059, CHK067, CHK068, CHK074, CHK077, CHK078, CHK079. The four Table-3 §D.2 banner-precedence gaps (rules 17, 23, 26, 27 — CHK044/045/046/047) are uniformly gated since `Scope::Page` projection correctness depends on all four.
- **Traceability**: ≥80% items carry at least one of `[Spec §FR-NNN]`, `[CAPCO-CONTEXT §X.Y]`, `[CAPCO §X.Y pNN]`, or `[GH #NNN]`. Pure `[Gap]`-only items are flagged for follow-up resolution before the cited PR merges.
- **Out of scope** (not in this checklist): SC-008/SC-009 performance budgets (US7 — see `requirements.md`); audit-schema cutover mechanics (FR-034..FR-037 — covered by `requirements.md`); generic spec-form quality (`requirements.md`).

## Notes

- The "Semantic Agreement with CAPCO Ruleset" dimension (§5) is the
  tangential extension Q3 asked for. It tests whether the requirements
  text faithfully encodes what CAPCO actually says — distinct from §4,
  which tests whether the *citation strings* are mechanically valid.
  A spec can pass §4 (every `§X.Y pNN` resolves) and still fail §5
  (the resolved passage says something the requirement doesn't capture).
  Constitution Principle VIII covers both, but the failure mode is
  different and the audit needs both gates.
- §6 (known defects) is the must-have list per Q3. Each item names a
  specific GitHub issue, source-file anchor, or carve-out. Future
  defects added to the register should append items in §6 with the
  same `[GH #NNN]` tag pattern.
- `[GATE]` items are the reviewer-gate subset (Q2 "B for the most
  important PRs"). Non-`[GATE]` items are PR-author sanity (Q2 "A").
  A reviewer should clear all `[GATE]` items in the relevant section
  before approving; PR-author clears all items in the section the PR
  touches.
- **Revision 2026-05-04** post-independent review: merged redundant
  CHK006/CHK060 (JOINT non-rollup), normalized §5.1 GATE tags so
  Table-3 rules 17/23/26/27 are uniformly gated, sharpened five
  already-answered items (CHK020/021/033/078/084) into substantive
  Gap/Measurability checks, fixed CHK021's D3→D1 cross-reference,
  re-verified CHK054 against CAPCO-2016.md §H.6 p104 and rewrote it
  with the confirmed full-eviction language, fixed CHK051's §A.5 →
  §A.6 (Formatting) citation per `CAPCO-2016.md` ToC. Also corrected
  three §A.5 → §A.6 references and three RD/FRD/TFNI banner-cell
  rewrites in `crates/capco/CAPCO-CONTEXT.md` to match §H.6 p104
  ("RD takes precedence and is conveyed in the banner line. … use
  only the RD warning statement").
