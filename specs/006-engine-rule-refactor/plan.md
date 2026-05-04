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
audit schema once (`marque-mvp-2 → marque-1.0`) under clean-break;
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
**Testing**: `cargo test` for unit/integration; `proptest` for lattice-law and two-pass-invariant property tests; `criterion` for benches with `tools/bench-check.sh` regression gates; `cargo-fuzz` for `Engine::lint` fuzz target (already wired); five-corpus regression sweep at `tests/corpus/{valid,mangled,prose,prose-positive,lattice}/` × {`StrictRecognizer`, `StrictOrDecoderRecognizer`} = 10 CI runs.
**Target Platform**: Native (Linux/macOS/Windows) for CLI / server / batch; WASM (browser, web worker) for `marque-wasm`. WASM-safe set: `marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`, `marque-capco` (Constitution III). The refactor preserves WASM safety end to end — every new type lands in a WASM-safe crate; `FixIntent<S>` adds a `marque-rules → marque-scheme` dep both of which are WASM-safe (Constitution VII; consolidated plan Appendix D).
**Project Type**: Rust workspace, multi-crate library + CLI + server + WASM bindings. Refactor scope spans 8 crates: `marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`, `marque-capco`, `marque-engine`, `marque-config`, `marque-wasm`. The workspace structure is fixed; this refactor changes types and call sites within existing crates, adds three CI tooling crates (`tools/citation-lint/`, `tools/masking-pin-lint/`, `tools/promote-callsite-lint/`) plus one transient discovery script (`tools/message-template-extract/`, run once before PR 3c per R-2 to seed the `MessageTemplate` enum and removed afterward), and introduces no new product crates.
**Performance Goals**: Preserve interactive p95 ≤ 16 ms on 10 KB inputs (existing SC-001 in CLAUDE.md, restated as spec SC-008); preserve linear scaling on `fix_throughput` (R² ≥ 0.9, FR-029); add p99 tail-percentile assertion (FR-030); multi-page projection within `PageContext` baseline + 10% (FR-031); two-pass re-parse cost within interactive budget (FR-032). Measurement-gated: >5% mean OR p99 regression backs out the change (FR-033).
**Constraints**: Zero heap allocation per scanner candidate (Constitution II). WASM-safe crate set must compile to WASM with `wasm-pack` without modification. WASM-safe crates must have zero runtime I/O (compile-time `build.rs` I/O permitted). The runtime-config restriction on WASM (Constitution III, last bullet) means decoder priors cannot be loaded at runtime in the WASM target — they bake at build time, which is already how `marque-priors-3` (PR 8) works. Audit records must remain content-ignorant (Constitution V Principle V / G13).
**Scale/Scope**: 8 product crates affected; 3 new tooling crates; ~200 corpus fixtures across 5 corpora; ~56 existing CAPCO rules collapse to ~10–13 (#263); ~ several hundred test-fixture `IsmAttributes { ... }` literals reshape across PR 3a/3b/3c. The PR sequence is 14 PRs (0, 0.5, 0.6, 1, 2, 3a, 3b, 3c, 3.7, 4, 5, 6, 7, 8, 9, 10) with the keystone window (3a→3b→3c) constituting the highest-blast-radius merge.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

The constitution at `.specify/memory/constitution.md` (v1.3.1, ratified
2026-03-12, last amended 2026-04-27) defines eight principles. Each is
evaluated against this refactor's spec.

| # | Principle | Spec coverage | Verdict |
|---|-----------|---------------|---------|
| I | Uncompromising performance | FR-029..FR-033, SC-008/SC-009; four Criterion benches gate the relevant PRs; measurement-gated rollback discipline preserved | **PASS** |
| II | Zero-copy / streaming core | Pivot split *strengthens* the invariant — `ParsedAttrs<'src>` carries `'src` lifetime, `FixProposal::original` becomes `Span` (not bytes, FR-004), `Box<[T]>` already used for collection fields | **PASS** |
| III | Format-agnostic core / WASM safety | All new types (`Canonical<S>`, `FixIntent<S>`, `ParsedAttrs`/`CanonicalAttrs`/`ProjectedMarking`, `MessageTemplate`, `Phase`) land in WASM-safe crates (`marque-ism`, `marque-rules`, `marque-scheme`, `marque-capco`); WASM build runs the same dispatcher as native | **PASS** |
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
│   │   └── ...                     # generated CVE enums, Span, Vocabulary metadata
│   └── build.rs                    # No semantic change; per-token is_fdr_dissem field added (FR-010)
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
│       ├── lib.rs                  # MarkingScheme: render_canonical(token, scope) → Canonical<S>; CanonicalConstructor<S> sealed trait (PR 3c)
│       ├── lattice.rs              # Existing built-in constructors (OrdMax, OrdMin, FlatSet, IntersectSet, SupersessionSet, ModeSet, MaxDate, OptionalSingleton, Product); Phase B already shipped
│       └── recognizer.rs           # Recognizer<S>: Send + Sync bound (PR 0, FR-038)
│
├── capco/                          # CAPCO Layer 2 rules + scheme adapter
│   ├── src/
│   │   ├── scheme.rs               # MarkingClassification::Us hardcode at :365 deleted (PR 5, FR-007); CapcoMarking::join PageContext delegation deleted (PR 4, FR-014); §4 fabrication cluster fixed (PR 0.6, FR-020)
│   │   ├── rules.rs                # ~56 rules collapse to ~10–13 (PR 3b, #263); doubled p150–151 p151 cluster fixed (PR 0.6); SIGMA archaeology fixed (PR 0.6); per-rule Phase declared (PR 7, FR-021)
│   │   └── lattice.rs              # SciSet / SarSet / FgiSet (existing); FgiSet renders without redundant FGI when trigraph present (PR 5, FR-008); FgiMarker::SourceConcealed | Acknowledged discriminant (PR 2, FR-017)
│   ├── docs/
│   │   └── CAPCO-2016.md           # Vendored authoritative source — single source of truth for citation-lint (FR-018)
│   ├── corpus/                     # Corpus-derived priors (input to PR 8 priors-bake)
│   └── tests/
│       ├── category_lattice_laws.rs        # NEW (PR 4) — assoc/comm/idem/identity per category (FR-011)
│       ├── cross_axis_dominance.rs         # NEW (PR 4) — FOUO eviction, FGI rollup, SCI canonicalization, AEA commingling (FR-012)
│       ├── parse_render_roundtrip.rs       # NEW (PR 2) — strict-path round-trip property (Layer 2)
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
tools/                              # NEW — three CI lint crates land in PR 0 / PR 0.5; one transient discovery script runs before PR 3c
├── masking-pin-lint/               # PR 0 (FR-039) — AST-based; GitHub API for issue state
├── promote-callsite-lint/          # PR 0 (FR-040) — AST-based; cfg(test) carve-out enumerated
├── citation-lint/                  # PR 0.5 (FR-018) — AST-based; parses CAPCO-2016.md vendored source
└── message-template-extract/       # PR 3c (R-2) — TRANSIENT one-shot discovery script; clusters Diagnostic::message format-arg literals to seed MessageTemplate enum; removed after PR 3c review accepts the curated enum (T030–T031)
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
├── prose/                          # Existing — Gutenberg + Federalist + Wikipedia; zero diagnostics
├── prose-positive/                 # NEW (PR 4) — true-positive markings in prose context MUST fire
├── lattice/                        # NEW (PR 3.7 → PR 4) — cross-axis fixture corpus
└── foreign/                        # NEW (PR 5) — pure_foreign_banner.json + JOINT/NATO fixtures (US2)
```

**Structure Decision**: Existing Rust workspace; refactor in place. The PR
sequence in `docs/plans/2026-05-02-engine-refactor-consolidated.md` §4 is the
authoritative implementation order. Each PR maps to specific FRs in the
spec via the per-crate change comments above. The keystone window (PR 3a /
3b / 3c) is the highest-blast-radius merge; the CI matrix during that
window runs corpus regression × {3a-only, 3a+3b, 3a+3b+3c} = 3 runs to
verify each subsequence is independently correct (SC-014).

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

Phase 0 → `research.md` (tactical implementation decisions resolved).
Phase 1 → `data-model.md`, `contracts/{fix-intent,audit-record,engine-pipeline}.md`, `quickstart.md`.
Agent context → CLAUDE.md updated by `.specify/scripts/bash/update-agent-context.sh claude`.

Phase 2 (`tasks.md`) is produced by `/speckit.tasks`, not this command.
