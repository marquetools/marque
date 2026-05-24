<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Implementation Plan: Engine + Rule Architecture Refactor

**Branch**: `006-engine-rule-refactor` | **Date**: 2026-05-03 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification at `/home/knitli/marque/specs/006-engine-rule-refactor/spec.md`
**Source of truth**: `docs/plans/2026-05-02-engine-refactor-consolidated.md` (PR sequence + invariants); `docs/plans/2026-05-01-lattice-design.md` (PR 3.7 deliverable).

## Summary

Land the post-murder-board consolidated engine refactor as a single ordered
PR sequence (PR 0 → PR 10). The keystone work splits the overloaded
`IsmAttributes` pivot type into three roles (`ParsedAttrs<'src>` /
`CanonicalAttrs` / `ProjectedMarking`); seals `Canonical<S>` with a
provenance-tagged constructor that locks the decoder out of open-vocab
canonicalization; introduces `FixIntent<S>` as the rule-emission API so
external rule crates never construct `Canonical<S>` directly; closes the
G13 (audit-record content-ignorance) leak channels by making them
unrepresentable rather than carve-out-enforced; drives page-level rollup
through `MarkingScheme::project(Scope::Page, ...)` and deletes
`PageContext`; phase-tags rules at registration with `Phase::Localized
| WholeMarking` and re-parses the buffer between passes; cuts over the
audit schema in two stages under clean-break (`marque-mvp-2 → marque-mvp-3`
at PR 3c.B Commit 10, then `marque-mvp-3 → marque-1.0` at PR 3c.2 — the
original "single cutover at PR 3c" plan was conservatively split when
PR 3c.B Commit 10 landed only the proposal sub-object envelope; the
remaining four structural commitments [Canonical wired into audit emit,
BLAKE3 digesting, closed MessageTemplate JSON, `from_parsed_unchecked`
delete] move to PR 3c.2 per FR-035a, added 2026-05-14);
adds three AST-based CI lints (citation, masking-pin, promote-callsite);
and gates PR 4's per-category `Lattice` impls behind PR 3.7 — a spike
that fills `2026-05-01-lattice-design.md` §§2–8 with the actual math
(§-citations, formal join semantics, worked examples, property fixtures,
cross-axis dominance fixtures) before any lattice impl lands.

The technical approach is type-system-first: every invariant the
spec mandates becomes either a compile-time guarantee (sealed
constructors, `Send + Sync` static assertions, `Phase` registration
checks), an AST-based CI lint (citations, masking pins, promote
call sites), or a property test (lattice laws, two-pass invariants,
audit canary scan). Comment-propagated invariants are deliberately
dismantled because the murder-board diagnosed them as the failure mode
producing the issue cadence.

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024); workspace `rust-version = "1.85"` floor pinned in workspace `Cargo.toml` per Constitution Technology Stack.
**Primary Dependencies**: `tokio` (async runtime, `BatchEngine`), `axum` + `tower` (server middleware), `memchr` 2 (Phase 1 SIMD scanner), `aho-corasick` 1 (Phase 2 token matching, native + WASM), `quick-xml` (build-time ODNI XSD/Schematron), `serde` + `serde_json` (build-time JSON sidecar), `phf` (compile-time replacement lookup), `criterion` 0.8 (benches), `static_assertions` (compile-time `Send + Sync` checks — FR-038), `blake3` (audit-record digests — FR-002/FR-004), `heed` (LMDB, planned v0.2 cache; not in scope here), `wasm-pack` (WASM target).
**New build-time dependencies introduced by this refactor**: `syn` + `proc-macro2` for the three AST-based CI lints (`tools/citation-lint/`, `tools/masking-pin-lint/`, `tools/promote-callsite-lint/`); `octocrab` (or equivalent) for the masking-pin lint's mandatory GitHub-API issue-state check (FR-039).
**Storage**: N/A on the hot path. Build-time cache via Cargo `OUT_DIR`. The planned LMDB `LintResult` cache is out of scope for this refactor.
**Testing**: `cargo test` for unit/integration; `proptest` for lattice-law and two-pass-invariant property tests; `criterion` for benches with `scripts/bench-check.sh` regression gates; `cargo-fuzz` for `Engine::lint` fuzz target (already wired); five-corpus regression sweep at `tests/corpus/{valid,mangled,prose,prose-positive,lattice}/` × {`StrictRecognizer`, `StrictOrDecoderRecognizer`} = 10 CI runs.
**Target Platform**: Native (Linux/macOS/Windows) for CLI / server / batch; WASM (browser, web worker) for `marque-wasm`. WASM-safe set: `marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`, `marque-capco` (Constitution III). The refactor preserves WASM safety end to end — every new type lands in a WASM-safe crate; `FixIntent<S>` adds a `marque-rules → marque-scheme` dep both of which are WASM-safe (Constitution VII; consolidated plan Appendix D).
**Project Type**: Rust workspace, multi-crate library + CLI + server + WASM bindings. Refactor scope spans 8 crates: `marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`, `marque-capco`, `marque-engine`, `marque-config`, `marque-wasm`. The workspace structure is fixed; this refactor changes types and call sites within existing crates, adds three CI tooling crates (`tools/citation-lint/`, `tools/masking-pin-lint/`, `tools/promote-callsite-lint/`) plus one transient discovery script (`tools/message-template-extract/`, run once before PR 3c per R-2 to seed the `MessageTemplate` enum and removed afterward), and introduces no new product crates.
**Performance Goals**: Preserve interactive p95 ≤ 16 ms on 10 KB inputs (existing SC-001 in CLAUDE.md, restated as spec SC-008); preserve linear scaling on `fix_throughput` (R² ≥ 0.9, FR-029); add p99 tail-percentile assertion (FR-030); multi-page projection within `PageContext` baseline + 10% (FR-031); two-pass re-parse cost within interactive budget (FR-032). Measurement-gated: >5% mean OR p99 regression backs out the change (FR-033).
**Constraints**: Zero heap allocation per scanner candidate (Constitution II). WASM-safe crate set must compile to WASM with `wasm-pack` without modification. WASM-safe crates must have zero runtime I/O (compile-time `build.rs` I/O permitted). The runtime-config restriction on WASM (Constitution III, last bullet) means decoder priors cannot be loaded at runtime in the WASM target — they bake at build time, which is already how `marque-priors-3` (PR 8) works. Audit records must remain content-ignorant (Constitution V Principle V / G13).
**Scale/Scope**: 8 product crates affected; 3 new tooling crates; ~200 corpus fixtures across 5 corpora; ~56 existing CAPCO rules collapse to ~10–13 (#263); ~ several hundred test-fixture `IsmAttributes { ... }` literals reshape across PR 3a/3b/3c. The PR sequence is 18 PRs (0, 0.5, 0.6, 1, 2, 3a, 3b, 3c, 3.7, 4, 5, 6, 7, 8, 9a, 9b, 9c, 10) with the keystone window (3a→3b→3c) constituting the highest-blast-radius merge.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

The constitution at `.specify/memory/constitution.md` (v1.3.1, ratified
2026-03-12, last amended 2026-04-27) defines eight principles. Each is
evaluated against this refactor's spec.

| # | Principle | Spec coverage | Verdict |
|---|-----------|---------------|---------|
| I | Uncompromising performance | FR-029..FR-033, SC-008/SC-009; four Criterion benches gate the relevant PRs; measurement-gated rollback discipline preserved | **PASS** |
| II | Zero-copy / streaming core | Pivot split *strengthens* the invariant — `ParsedAttrs<'src>` carries `'src` lifetime, `FixProposal::original` becomes `Span` (not bytes, FR-004), `Box<[T]>` already used for collection fields | **PASS** |
| III | Format-agnostic core / WASM safety | All new types (`Canonical<S>`, `FixIntent<S>`, `RenderContext`/`EmissionForm` per PR 3c.2 / FR-052, `FormSet`/`FormKind` per PR 3d / FR-053, `Deprecation` validity-window fields per PR 3d / FR-054, `ParsedAttrs`/`CanonicalAttrs`/`ProjectedMarking`, `MessageTemplate`, `Phase`) land in WASM-safe crates (`marque-ism`, `marque-rules`, `marque-scheme`, `marque-capco`); WASM build runs the same dispatcher as native; PR 3d additions tracked under WASM size budget (≤ 5%, T058i) | **PASS** |
| IV | Two-layer rule architecture | FR-025/FR-026 codify rule emission via `FixIntent<S>` while preserving Layer 1 generated predicates / Layer 2 hand-written rule split; FR-021 makes `Phase` an explicit registration tag | **PASS** |
| V | Audit-first / G13 | The keystone correctness property of this entire refactor. FR-001..FR-005 (sealed `Canonical`, `MessageTemplate`-only messages, `Span`-only original, engine-only promotion) + FR-027/FR-028 (decoder open-vocab lockout + carve-out delete) + FR-040 (promote-callsite lint) close it. Test-fixture carve-out preserved per Principle V's three constraints | **PASS** |
| VI | Dataflow pipeline model | FR-006 (Scope::Page projection replaces `PageContext`), FR-021..FR-024 (phase-tagged two-pass with R002 partial-progress diagnostic), FR-038 (Send + Sync), FR-041 (engine mints synthetic diagnostics) | **PASS** |
| VII | Crate discipline / acyclic deps | One graph change at PR 3c: `marque-rules` gains a `marque-scheme` dep so `FixIntent<S>` can reference scheme-defined types. Both crates are WASM-safe; graph remains acyclic; `cargo check --workspace` passes (consolidated plan Appendix D updated graph) | **PASS** |
| VIII | Authoritative source fidelity | FR-018 (citation lint scope: `citation:`/`message:`/`constraint_label:`/doc-comment), FR-019 (corpus fixture per cited authority), FR-020 (preemptive defect fix in PR 0.6); SC-005/SC-006 measure | **PASS** |

**Constitution Check verdict**: PASS — no violations, no Complexity Tracking
entries required.

## Project Structure

### Documentation (this feature)

```text
specs/006-engine-rule-refactor/
├── spec.md                 # Feature spec (already locked, /speckit.specify output)
├── plan.md                 # This file (/speckit.plan output)
├── research.md             # Phase 0: tactical implementation research
├── data-model.md           # Phase 1: type-system shapes for new entities
├── quickstart.md           # Phase 1: orientation for contributors during the refactor
├── contracts/
│   ├── fix-intent.md       # Phase 1: rule-emission API contract
│   ├── audit-record.md     # Phase 1: post-cutover NDJSON shape contract
│   └── engine-pipeline.md  # Phase 1: dataflow + phase-split + R002 contract
├── checklists/
│   └── requirements.md     # Spec quality validation (already written)
└── tasks.md                # Phase 2: produced by /speckit.tasks (NOT this command)
```

### Source Code (repository root)

This is a Rust workspace with crates already in place. No new product crates
land in this refactor; three tooling crates land for CI lints. Per-crate scope
of change:

```text
crates/
├── ism/                            # Pivot crate — vocabulary + generated CVE
│   ├── src/
│   │   ├── attrs.rs                # IsmAttributes (current); split into ParsedAttrs/CanonicalAttrs/ProjectedMarking (PR 3a)
│   │   ├── canonical.rs            # NEW (PR 3c) — Canonical<S> with sealed constructors + TokenSource
│   │   ├── message.rs              # NEW (PR 3c) — MessageTemplate enum + MessageArgs closed-set
│   │   ├── marking_forms.rs        # MarkingForm gains description_title field (PR 3d, FR-053)
│   │   └── ...                     # generated CVE enums, Span, Vocabulary metadata
│   └── build.rs                    # Per-token is_fdr_dissem field (PR 4, FR-010); ODNI Description.title harvested into recognized_aliases when divergent from CAPCO long-title (PR 3d, FR-053); Deprecation valid_from/valid_until populated from XSD annotations + migration table (PR 3d, FR-054)
│
├── core/                           # Scanner + parser
│   └── src/parser.rs               # Four open-vocab admission sites migrate to shape_admits (PR 2, FR-015): three is_ascii_alphanumeric() checks (:1453/:1481/:1493) + FGI trigraph silent-skip (:1011-1024); FGI returns None on shape failure (FR-016)
│
├── rules/                          # Trait surface only
│   └── src/
│       ├── lib.rs                  # Rule trait gains phase() (PR 7, FR-021); FixProposal.original becomes Span (PR 3c, FR-004)
│       ├── fix_intent.rs           # NEW (PR 3c) — FixIntent<S> rule-emission API
│       ├── applied_fix.rs          # AppliedFix v2: (scheme, predicate-id) rule ID, Canonical<S>-typed replacement, MessageTemplate-typed message (PR 3c)
│       └── diagnostic.rs           # Diagnostic v2: Message { template, args } (PR 3c, FR-003)
│
├── scheme/                         # Domain-neutral trait surface (no domain vocab)
│   └── src/
│       ├── scheme.rs               # MarkingScheme: render_canonical(&FixIntent<S>, &RenderContext) → Canonical<S>; RenderContext { scope, emission_form, schema_version }; #[non_exhaustive] EmissionForm { Auto, Portion, BannerTitle, BannerAbbreviation } per CAPCO §G.1 Table 4 (PR 3c.2, FR-052); CanonicalConstructor<S> sealed trait (PR 3c.1)
│       ├── vocabulary.rs           # Vocabulary<S>::forms() returning &'static FormSet; per-form methods become defaults (banner_form() = banner_abbreviation.unwrap_or(banner_title)); FormSet { portion, banner_title, banner_abbreviation, recognized_aliases } + #[non_exhaustive] FormKind (PR 3d, FR-053); Deprecation<Token> gains valid_from/valid_until (PR 3d, FR-054); shape_admits + is_fdr_dissem (PR 2 / PR 4, FR-015 / FR-010)
│       ├── lattice.rs              # Existing built-in constructors (OrdMax, OrdMin, FlatSet, IntersectSet, SupersessionSet, ModeSet, MaxDate, OptionalSingleton, Product); Phase B already shipped
│       └── recognizer.rs           # Recognizer<S>: Send + Sync bound (PR 0, FR-038)
│
├── capco/                          # CAPCO Layer 2 rules + scheme adapter
│   ├── src/
│   │   ├── scheme.rs               # render_canonical impl honors RenderContext.emission_form (PR 3c.2, FR-052); MarkingClassification::Us hardcode at :365 deleted (PR 5, FR-007); CapcoMarking::join PageContext delegation deleted (PR 4, FR-014); §4 fabrication cluster fixed (PR 0.6, FR-020)
│   │   ├── vocabulary.rs           # impl Vocabulary<CapcoScheme>: forms() composes FormSet from MARKING_FORMS + per-token build-time records (PR 3d, FR-053)
│   │   ├── rules.rs                # ~56 rules collapse to ~10–13 (PR 3b, #263); doubled p150–151 p151 cluster fixed (PR 0.6); SIGMA archaeology fixed (PR 0.6); per-rule Phase declared (PR 7, FR-021); FixIntent emission sites pass EmissionForm::Auto by default (PR 3c.2, FR-052)
│   │   └── lattice.rs              # SciSet / SarSet / FgiSet (existing); FgiSet renders without redundant FGI when trigraph present (PR 5, FR-008); FgiMarker::SourceConcealed | Acknowledged discriminant (PR 2, FR-017)
│   ├── docs/
│   │   └── CAPCO-2016.md           # Vendored authoritative source — single source of truth for citation-lint (FR-018)
│   ├── corpus/                     # Corpus-derived priors (input to PR 8 priors-bake)
│   └── tests/
│       ├── category_lattice_laws.rs        # NEW (PR 4) — assoc/comm/idem/identity per category (FR-011)
│       ├── cross_axis_dominance.rs         # NEW (PR 4) — FOUO eviction, FGI rollup, SCI canonicalization, AEA commingling (FR-012)
│       ├── parse_render_roundtrip.rs       # NEW (PR 2) — strict-path round-trip property (Layer 2)
│       ├── render_canonical_emission_form.rs # NEW (PR 3c.2) — EmissionForm selector covers Auto/Portion/Banner/BannerAbbreviated/LongTitle (FR-052)
│       ├── vocabulary_forms.rs             # NEW (PR 3d) — forms() round-trip with per-form accessors (FR-053)
│       └── citation_fidelity.rs            # NEW (PR 0.5 skeleton, PR 10 maturation) — F.1 corpus fixture per cited authority (FR-019)
│
├── engine/                         # Pipeline orchestration
│   ├── src/
│   │   ├── engine.rs               # build_decoder_diagnostic carve-out deleted (PR 3c, FR-028); page-rollup driven by Scope::Page projection (PR 6); two-pass dispatch with re-parse + R002 (PR 7); engine.rs:1389 format! interpolation deleted (PR 3c, FR-003)
│   │   ├── scheduler.rs            # Existing topological scheduler (Phase 3 of #69); no semantic change
│   │   └── recognizer/             # StrictRecognizer / DecoderRecognizer / StrictOrDecoderRecognizer (PR #259); decoder open-vocab lockout (PR 3c, FR-027)
│   └── tests/
│       ├── two_pass_invariants.rs  # NEW (PR 7) — I-18 non-overlap + I-19 reshape-aware (SC-007)
│       └── fix_invariants.rs       # NEW (PR 3c + PR 7) — Layer 3 per-pass invariants
│
├── config/                         # Layered config — no scope change
├── extract/                        # Stub kreuzberg backend — out of scope
├── wasm/                           # WASM bindings — exercised by SC-008 parity
└── server/                         # axum REST microservice — no scope change here
```

```text
tools/                              # NEW — three CI lint crates land in PR 0 / PR 0.5; one transient discovery script runs before PR 3c; flake-watch + threshold artifact land in PR 0 per decisions.md
├── masking-pin-lint/               # PR 0 (FR-039) — AST-based; GitHub API w/ 5s timeout + daily-cache fallback per D11
├── promote-callsite-lint/          # PR 0 (FR-040) — AST-based; cfg(test) carve-out enumerated; signature-shape extension per D12 (catches ParsedAttrs→CanonicalAttrs outside MarkingScheme::canonicalize, unsafe fn whitelisted)
├── citation-lint/                  # PR 0.5 (FR-018) — AST-based; parses CAPCO-2016.md vendored source
├── message-template-extract/       # PR 3c (R-2) — TRANSIENT one-shot discovery script; T030 runs it to cluster Diagnostic::message format-arg literals and seed the curated MessageTemplate enum, and T031 deletes tools/message-template-extract/ after PR 3c review accepts that enum
└── flake-watch/                    # PR 0 (FR-051, D16) — quarantine-queue tracker; cap=10; PR-merge gate when cap exceeded
```

```text
benches/
├── fix_throughput/                 # Already landed (PR #278)
├── lint_latency/                   # Existing SC-001 bench; p99 assertion added (PR 2, FR-030)
├── fix_10kb/                       # NEW (PR 7) — two-pass re-parse cost (FR-032)
└── lint_100kb_multipage/           # NEW (PR 6) — projection cutover (FR-031)
```

```text
tests/corpus/                       # Corpus regression sweep — five corpora × two recognizers
├── valid/                          # Existing — zero auto-applied fixes target
├── mangled/                        # Existing — ≥0.85 fix accuracy (SC-010 may re-anchor)
│   └── threshold.toml              # NEW (PR 0, D7) — structured artifact encoding chosen R-8 branch + threshold value; bench-check.sh reads it
├── prose/                          # Existing — Gutenberg + Federalist + Wikipedia; zero diagnostics
├── prose-positive/                 # NEW (PR 4) — true-positive markings in prose context MUST fire
├── lattice/                        # NEW (PR 3.7 → PR 4) — cross-axis fixture corpus
└── foreign/                        # NEW (PR 5) — pure_foreign_*.txt, fgi_banner_*.txt, nato_only_*.txt, joint_*.txt with `.expected.json` sidecars (US2; D15 glob+count)
```

**Structure Decision**: Existing Rust workspace; refactor in place. The PR
sequence in `docs/plans/2026-05-02-engine-refactor-consolidated.md` §4 is the
authoritative implementation order. Each PR maps to specific FRs in the
spec via the per-crate change comments above. The keystone window (PR 3a /
3b / 3c) is the highest-blast-radius merge; the CI matrix during that
window runs corpus regression × {3a-only, 3a+3b, 3a+3b+3c} = 3 runs to
verify each subsequence is independently correct (SC-014).

## PR 0 Absorption (decisions D1–D16)

PR 0 absorbs the full decision register from
[`decisions.md`](./decisions.md). The original PR 0 scope (`static_assertions`
+ masking-pin lint + promote-callsite lint) expands to the union below.
Every entry below MUST land in PR 0 (or its directly-coupled
sub-PR setup); none are deferred to subsequent refactor PRs.

**Scope of this spec PR vs. PR 0 implementation**: this spec PR
locks the decisions in `decisions.md` and lands the **spec / plan /
contract / research edits** plus the **markdown scaffolds** for
artifacts whose schema needed pinning (`tests/corpus/mangled/threshold.toml`,
`tools/flake-watch/README.md`, `tools/flake-watch/issues.md`). The
**operational artifacts** — Rust lint crates at `tools/masking-pin-lint/`
/ `tools/promote-callsite-lint/` / `tools/citation-lint/`, the
`rust-toolchain.toml` workspace pin, the `trybuild` dev-dependency, the
flake-watch CI workflow — land in subsequent PR 0 implementation
commits on top of these locked decisions. The "Where" column below
distinguishes the two: entries citing a spec / plan / contract path
(`spec.md`, `decisions.md`, etc.) land here; entries citing a tooling
path (`tools/<crate>/`, workspace files) land in PR 0 implementation.

| # | Deliverable | Where |
|---|-------------|-------|
| D1 | "R002 surfacing semantics" consumer-surface contract | `contracts/engine-pipeline.md` |
| D2 | PR 3.7 alternate-owner requirement | `spec.md` Assumptions (already landed in this PR's edits) |
| D3 | `marque --version` schema-name surfacing + cutover-changelog wording | `contracts/audit-record.md` |
| D4 | No-consumers attestation template (used in PR 0 + PR 3c PR descriptions) | `decisions.md` D4 (template); referenced from PR 0 / PR 3c PR description templates |
| D5 | SC-010 binding R-8 wording; threshold-artifact reference | `spec.md` SC-010 (landed); `research.md` R-8 (amended in this PR) |
| D6 | FR-049 — predicate-id stability freeze begins at PR 10 merge | `spec.md` (landed) |
| D7 | `tests/corpus/mangled/threshold.toml` scaffold (schema + empty values) | new file |
| D8 | FR-050 — cumulative bench-drift gate at PR 10. PR-0-review amendment: hardware pinning beyond `ubuntu-latest` is out of project budget; `decisions.md` D8 documents the realized constraint and the bench-runner owner (`bashandbone`) | `spec.md` (landed); `decisions.md` D8 (amended); PR 0 description records the runner-family commitment |
| D9 | R-9 in research — PR 9 → 9a / 9b / 9c | `research.md` |
| D10 | Layer 0 in test taxonomy + `rust-toolchain.toml` + trybuild version pin | `contracts/engine-pipeline.md`; workspace toolchain file (verify) |
| D11 | R-10 in research — masking-pin cache-with-fallback | `research.md`; `tools/masking-pin-lint/` design note |
| D12 | R-11 in research — `_unchecked`-by-signature lint extension; FR-040 amendment | `research.md`; `spec.md` FR-040 (landed) |
| D13 | PR 3b acceptance criteria (qualitative gate per declarative entry; end-state target ~10 surviving rules across stages 1–4) — amended 2026-05-07; per-PR-3b numeric band retired 2026-05-07 | this section (below) + reviewer attestation requirement |
| D14 | Trait stabilization forcing function in Assumptions | `spec.md` (landed) |
| D15 | US2 Independent Test → glob+count | `spec.md` (landed) |
| D16 | FR-051 — flake-watch quarantine queue (cap=10) | `spec.md` (landed); `tools/flake-watch/` scaffold |

**PR 3b acceptance criteria addendum (D13, amended 2026-05-07 per
`docs/plans/2026-05-07-pr3b-consultation-verdict.md`)**:

The post-collapse rule count is the **first** of four staged collapse
waves. PR 3b proper lands the **declarative-catalog moves** (existing
primitives only); subsequent compaction lands across PR 3.7 / PR 4 /
PR 5+ alongside the lattice §-resolution spike, the per-category
Lattice impls, and the renderer correctness work. Original D13 wording
targeted "8–18 band post-PR-3b"; the consultation verdict re-baselined
the source count to **59** (`grep -c '^impl Rule for' rules.rs
rules_declarative.rs rules_sci_per_system.rs`, not the "~56"
approximation the lattice plan carried) and re-sequenced the moves
across stages. A subsequent re-evaluation **2026-05-07** retired the
PR-3b-proper numeric band (originally "13–18") after the planning pass
on T026a found that the literal sub-move retirements deliver −15 to
−21 rules, landing at ~38–44 post-3b — outside any 13–18 band by
construction. The per-sub-PR principle is now **drive the count down
within what the sub-move's primitive scope authorizes**, not "hit a
band." End-state target across all four stages remains ~10 surviving
rules, with the heavy compaction lifting in Stage 3 (PR 4 per-category
Lattice impls retire entire walker classes) and Stage 4 (PR 5+ renderer
absorbs ordering / style rules).

| Stage | PR | Expected surviving rules | Acceptance gate |
|---|---|---|---|
| Pre-collapse (today) | — | **59** | — |
| Stage 1 (PR 3b proper — declarative-catalog) | PR 3b | **~38–44** | Qualitative gate (per-sub-move attestation; see below). Numeric band retired. |
| Stage 2 (new primitives + catalog compaction) | PR 3.7 | ~32–40 (RELIDO compacts to 2 family rows; closure operator absorbs implication rows) | PR 3.7 acceptance (T108) |
| Stage 3 (per-category Lattice impls + closure wiring) | PR 4 | ~14–22 (banner walker retires; SCI per-system walker retires into per-category Lattice impls) | PR 4 acceptance (T111+) |
| Stage 4 (renderer correctness + RELOPT round-trip) | PR 5+ | **~10** | PR 5+ acceptance |

**PR 3b sub-moves** (each independently committable inside PR 3b; see
T026a–T026f in `tasks.md`):

- **3b.A** — banner roll-up rules (E031 SAR, E035 SCI, E040 Non-IC
  dissem — the literal `impl Rule` blocks in `rules.rs`; spec-text
  E034 / E045 / FGI / classification banner rules are out of scope:
  no current `RuleId::new("E034")` exists in the live ruleset — the
  archived spec planned it but it landed as `W034`
  `SciCustomControlInfoRule`, which is per-system not banner-rollup;
  E045 is per-system and belongs to T026e; FGI / classification
  banner rollup have no current `impl Rule` block to retire) collapse to ONE generic walker over a
  per-category catalog. The walker consumes existing
  `PageContext::expected_*()` accessors (NOT `MarkingScheme::project`,
  which still delegates back through `PageContext` and which awaits
  `ProjectedMarking` becoming a real consumer in PR 6). Each catalog
  row preserves its source rule's surgical fix-emission machinery
  (zero-width insertion span vs error fallback, C-1 overlap-guard
  interaction with E028/E029) and emits diagnostics with per-row rule
  IDs to preserve audit-stream traceability across the walker
  boundary. Net delta: −2 rules (3 retired, 1 walker added). The
  walker retires when PR 4's per-category Lattice impls + property
  tests land (Stage 3).
- **3b.B** — `marque-applied.md` §3.4.1 transmutation roster (6
  entries) + §3.4.3 cross-axis FGI rollup (1 entry) ship as 7
  declarative `PageRewrite` rows. Topological scheduler annotates
  reads/writes per entry.
- **3b.C** — 4 directly-cited RELIDO `Constraint::Conflicts` rows
  (E054–E057; single-token RHS only; D17 scope correction). The
  ~15–20 projection from the consultation verdict was revised:
  re-verification found only 4 pairs with verbatim §H.8 authority;
  the broader §3.4.2 family roster defers to T108b (PR 3.7) where
  `RhsFamily(predicate)` lands — see `decisions.md D17`.
- **3b.D** — **LANDED 2026-05-08** (T026d, #324). Landed
  `marque-applied.md` §3.4.6 per-token classification-floor catalog
  as 27 `Constraint::Custom("class-floor/...", ...)` rows on
  `CapcoScheme` (catalog-pin: `catalog_declares_27_class_floor_rows`;
  the +1 vs the plan-doc's "~26" is the UCNI split) (Constitution VII §IV blocks scheme-adoption PRs
  from adding new `Constraint` variants; the canonical-Custom
  precedent set by `E022/CNWDI-classification-floor` was generalized
  to the 26-row catalog). Walker `DeclarativeClassFloorRule` (rule
  ID `E058`) dispatches over the catalog with a 3-layer hot-path
  optimization. Closure-implied requirements (`marque-applied.md`
  §4.7.5) stay as `Custom` floor rows in PR 3b; the closure
  operator primitive (Stage 2.B in `marque-applied.md` §3.11) lands
  in PR 3.7 and re-classifies the implication-shaped entries in
  PR 4. Net rule delta: 3 retired (E022/E025/E027) + 1 walker
  added = net −2. Running registered-rule count: 61 → 59. See
  `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md`.
- **3b.E** — **LANDED 2026-05-08** (T026e). Collapsed the 10 rules in
  the now-deleted `crates/capco/src/rules_sci_per_system.rs` into ONE
  `DeclarativeSciPerSystemRule` walker (rule ID `E059`) dispatching
  over a 5-row `Constraint::Custom("sci-per-system/...", ...)` catalog
  at §H.4 family granularity (HCS-O companions, HCS-P NOFORN, HCS-P
  sub-compartment companions, SI-G companions, TK compartment NOFORN);
  per-row `CAPCO-2016 §H.4 pXX` citation. The class-floor portions of
  the retired rules are absorbed by PR 3b.D's class-floor catalog;
  no class-floor rows are added in 3b.E. Net rule delta: −9 (10
  retired + 1 walker added). Running registered-rule count: 59 → 50.
  See `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`.
- **3b.F** — non-canonical input fallback walker covering E020 /
  E023 / E028 / E033 ordering checks (REL TO leads with USA, AEA
  SIGMA numeric sort, SAR alphabetic, SCI numeric-then-alpha) as a
  single `impl Rule` block. **Status: LANDED 2026-05-08.**
  `DeclarativeNonCanonicalInputRule` (rule ID `E060`) dispatches
  over a 5-row private `NON_CANONICAL_CATALOG` inside
  `crates/capco/src/rules_declarative.rs`. Per-row §-citations:
  REL TO USA-first alpha (§H.8 p150-151), JOINT alpha (§H.3 p56),
  AEA SIGMA numeric sort (§H.6 p108), SAR program ascending alpha
  (§H.5 p99), SCI compartment + sub-compartment numeric-then-alpha
  (§H.4 p61). Net rule delta: −3 (4 retired + 1 walker added).
  Running registered-rule count: 50 → 47. Walker retires when the
  Phase C renderer trait surface lands in PR 5+ (Stage 4) and
  absorbs canonical-form rendering. See
  `docs/plans/2026-05-08-pr3b-F-non-canonical-input-walker-plan.md`.
- **3b umbrella — LANDED 2026-05-08** (T027 + T028 + T029,
  closeout sub-PR #328 aggregating 3b.A #319 / 3b.B #320 /
  3b.C #321 / 3b.D #324 / 3b.E #326 / 3b.F #327). Closeout
  bookkeeping only — zero rule-logic edits. T027: per-sub-move
  reviewer attestation (D13 single-§-citation discipline,
  ≤3-branch `impl Rule` bodies, net-rule-delta math 59 → 47)
  aggregated in the umbrella PR description. T028:
  exact-rule-ID-set pin landed at
  `crates/capco/tests/post_3b_registration_pin.rs`,
  complementing the existing count pin in
  `corpus_parity.rs:170-194` by catching rename-at-same-count
  + delete-and-add-at-same-count drift. T029: new
  `pr-3b-corpus-regression` CI job in `.github/workflows/ci.yml`
  mirroring T025's body, branch-filtered to
  `refactor-006-pr-3b*`. Constitution VII §IV-clean (no edits to
  `crates/{engine,scheme,core,rules,ism}`). See
  `docs/plans/2026-05-08-pr3b-closeout-T027-T028-T029-plan.md`.
- **4b umbrella — LANDED 2026-05-{15→18}** (T142 + T143 + T144 +
  T145, closeout aggregating 4b-A #426 / 4b-B #437 / 4b-C #468 /
  4b-D.0 #514 / 4b-D.1 #517 / 4b-D.2 #527 / 4b-D.3 #535 / 4b-E
  #539 / 4b-F #542). Closeout bookkeeping only — zero rule-logic
  edits; zero engine-crate edits (Constitution VII
  scheme-adoption boundary). T142: umbrella attestation
  aggregating (a) single-§-citation discipline across 12 lattice
  types + 27 `PageRewrite` rows + 10 `ClosureRule` rows + W004,
  (b) engine-crate touch ledger documenting the five within-006
  precedent breaches (4b-B Commit 2 / 4b-C Commit 5 / 4b-D.2 /
  4b-D.3 / 4b-E), (c) per-axis net-delta math from pre-4b
  baseline through post-4b-F terminal state (12 Join + 9 Meet +
  2 BoundedJoin + 2 BoundedMeet impls; 27 PageRewrite rows; 10
  ClosureRule rows; 39 `Constraint::Custom` rows; 38 registered
  rules with W004 added and W002 retired). T143: compile-time
  pin at `crates/capco/tests/lattice_static_assertions.rs`
  locking 12 + 9 + 2 + 2 trait impls plus the Join-only invariant
  for DissemSet / JointSet / DisplayOnlyBlock (issue #456 / PR #502
  split + PR #538 audit). T144: runtime triple-pin at
  `crates/capco/tests/post_4b_lattice_inventory_pin.rs` covering
  positional 27 `PageRewrite` names + positional 10 `ClosureRule`
  names + sorted-set 39 `Constraint::Custom` labels. T145:
  `pr-4b-corpus-regression` CI job branch-filtered to
  `refactor-006-pr-4b*`, mirroring T029's body. See
  `docs/plans/2026-05-19-pr4b-closeout-pm-decisions.md`.

**Reviewer attestation requirements** (each sub-PR's PR description
declares a–c against the sub-move it lands; the umbrella PR-3b
aggregates):

1. **Single CAPCO-§ citation per declarative catalog entry** (NOT
   per `impl Rule` block). Resolves Q-3.9 from `marque-applied.md`
   §9. The consolidated walkers (3b.A banner, 3b.C RELIDO, 3b.D
   floors, 3b.E SCI per-system, 3b.F non-canonical input) are each
   one `impl Rule` block that
   delegates to a catalog; each catalog row carries its own
   §-citation. Cited per Constitution VIII (citation integrity is
   per-claim, not per-block); the discipline is unaffected by where
   the citation textually lives so long as it is verifiable.
2. Predicate body of every `impl Rule` block has ≤3 internal
   branches (count `match`/`if` arms in the body of the rule's
   `evaluate` impl; count nested `match` / `if` separately).
   Walkers' dispatch on catalog-entry kind counts as ≤3 when the
   catalog stays homogeneous (`Conflicts` / `Requires` / `Custom`).
3. **Net rule delta and running count** declared in the PR
   description with the math (e.g., "3b.A: 3 retired + 1 walker =
   net −2; running count 59 → 57"). The PR-3b-proper numeric band
   is retired; the gate is "drive the count down within what the
   sub-move's primitive scope authorizes" — sub-PRs that retire 0
   rules (because their primitives are additive, not consolidating)
   are still acceptable when the additions ship the declarative
   catalog the bridge §3.4 prescribes. Aggressive consolidation
   beyond what the bridge prescribes (e.g., compacting
   `rules_declarative.rs` further than `marque-applied.md` §3.4
   authorizes) requires team review before merge — the gate is
   "stay within the sub-move's authorized primitive scope," not "hit
   a numeric target." End-state target ~10 surviving rules across
   all four stages remains binding; the heavy lifting toward that
   target lands in Stage 3 (PR 4) and Stage 4 (PR 5+).

**Algebraic justification**: `marque-applied.md` §3 (PR 3b stall
walkthrough — bucket carving + Phase A/B/C overlay) and §3.11 (stage
sequencing) are the source of truth for "this rule collapses to that
primitive." The (a)/(b)/(c) verdicts and per-move catalog citations
from `pure-lattice.md` / `security-lattice.md` / `abstract-interp.md`
/ `frames-locales.md` justify each collapse target. The consultation
verdict at `docs/plans/2026-05-07-pr3b-consultation-verdict.md` is
the dated decision record.

**Risk register (refactor-006 specific)**:

| Risk | Mitigation | Owner |
|------|-----------|-------|
| Cumulative bench drift across PR 1–10 invisibly exceeds the per-PR FR-033 envelope | FR-050 end-state assertion at PR 10 against PR-0 baseline. Per-PR contributions >6% are flagged for attribution. **Project explicitly accepts shared-runner variance** per the D8 amendment (no custom-runner budget); the gate stays at 10% cumulative and may need a follow-up tolerance widening if shared-runner variance makes it flap empirically | bench-runner owner (`bashandbone`) |
| Hardware drift between PR 0 baseline capture and PR 10 acceptance invalidates comparison | Bench captures run on GitHub Actions `ubuntu-latest` hosted runners — runner-pool image rotations are not within project control. The bench-runner owner re-runs the PR-0 baseline if a rotation produces clearly anomalous deltas, but is NOT obligated to reconcile every percent-level drift. See `decisions.md` D8 for the full constraint and rationale | bench-runner owner (`bashandbone`) |
| PR 3.7 stalls and PRs 4–10 cascade-stall | Named alternate owner (D2) with §§2–8 read-through completed before PR 3c merges; 1-week stall trigger for alternate handoff without escalation | PR 3.7 primary owner + alternate |
| PR 3c lands but mangled-corpus accuracy regresses below 0.80 | Binding R-8 decision tree (D5); `threshold.toml` (D7) records branch taken; <0.80 + non-K-Option-2 attribution → revert 3a/3b/3c as a unit | PR 3c reviewer |
| Test flakes accumulate silently and erode CI signal | FR-051 quarantine queue with cap=10 (D16); cap exceedance blocks merges until triage clears | flake-watch triage owner (rotating) |
| External consumer attaches between PR 0 and PR 3c, invalidating clean-break Assumption | Manual attestation at PR 0 + PR 3c (D4); 60-day no-contact window in attestation template | PR-author (self-attest) |

## Complexity Tracking

> No Constitution gates failed. No entries required.

The refactor adds three CI tooling crates (citation-lint, masking-pin-lint,
promote-callsite-lint), which superficially looks like complexity inflation,
but each closes a class of defect the murder board diagnosed as
*comment-propagated invariant rot* — strictly cheaper than continuing to
catch citations / pin discipline / promote-callsite drift in PR review.
The lints are gated through the existing CI infrastructure, depend only on
`syn`/`proc-macro2` (and `octocrab` for the masking-pin lint's mandatory
GitHub-API check), and serve a single purpose each (no scope creep). They
are infrastructure, not product surface.

## Phase artifacts

Phase 0 → `research.md` (tactical implementation decisions resolved); `decisions.md` (process / contract decisions D1–D16, locked at PR 0).
Phase 1 → `data-model.md`, `contracts/{fix-intent,audit-record,engine-pipeline}.md`, `quickstart.md`.
Agent context → CLAUDE.md updated by `.specify/scripts/bash/update-agent-context.sh claude`.

Phase 2 (`tasks.md`) is produced by `/speckit.tasks`, not this command.
