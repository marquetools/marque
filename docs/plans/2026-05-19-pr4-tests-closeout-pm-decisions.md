<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4-tests closeout — PM decisions

**Date**: 2026-05-19
**Branch (from)**: `refactor-006-pr-4b-closeout` (current worktree HEAD after PR #557 merge)
**Branch (new)**: `refactor-006-pr-4-test-closeout` (this PR)
**Target**: `staging`
**Spec tasks**: T116, T117, T117a, T118, T119 in `specs/006-engine-rule-refactor/tasks.md` lines 342-346
**Out of scope**: T119a/b/c (#307 umbrella + #552/#554 sub-PRs; tracked separately per user's chart)

## Scope

Five-task closeout of the PR-4 test surface that PR 4b-A landed AEA-only:

- **T116** — `crates/capco/tests/category_lattice_laws.rs` per-category lattice law tests. **Gap is 4-5 modules**, not 12: SciSet, SarSet, NonIcDissemSet, DisplayOnlyBlock + FgiSet join-side extension. Existing file already has modules for the other 8 lattice types (ClassificationLattice, NatoClassLattice, DeclassifyOnLattice, DissemSet, NatoDissemSet, JointSet, RelToBlock, FgiSet meet-side).
- **T117** — `crates/capco/tests/cross_axis_dominance.rs`. 4 new `#[test]` blocks for the 4 non-AEA fixture classes (FOUO-eviction-class, FOUO-eviction-non-fdr, FGI-banner-rollup, SCI-cross-system).
- **T117a** — 1 new `#[test]` in same file: US-reciprocates-equivalent (mirrors `tests/corpus/foreign/mixed_us_foreign_rollup.expected.json` at the property-test level).
- **T118** — new `crates/capco/tests/lattice_corpus_runner.rs` + 5 `.expected.json` sidecars in `tests/corpus/lattice/`. Runner dispatches on fixture shape (CAB-commingling vs portions-banner).
- **T119** — probe-first wiring of `tests/corpus/documents/marked/*.md` (CIA CREST corpus, 40 fixtures) into the engine.lint precision gate. **Probe stays in-tree as `#[ignore]`-gated diagnostic**; assertion gate added only if probe is clean.

## PM decisions (D-series)

### D-1. Single PR, not split
~700-730 LOC across 5 modified + 6 new files. All five tasks share the lattice test surface; splitting adds review overhead without independent revertability gain. Mirrors PR 4b closeout's umbrella shape.

### D-2. T116 reduced scope — accept rust-specialist's refinement
Architect pushed back hard on PM's initial "compile-time pin substitutes for runtime laws" lean (correct push-back: `assert_impl_all!` catches trait-impl shape drift but not algebraic-law behavior drift; a wrong `join()` body still compiles). Rust-specialist then verified that 8 of the 12 lattice types **already have** law-test modules in `category_lattice_laws.rs` — user's "done in spirit, just unchecked" framing was substantially correct.

**Final scope**: 4 new `mod` blocks (`sci_set`, `sar_set`, `non_ic_dissem_set`, `display_only_block`) + 1 join-side extension to existing `fgi_set_concealed_top` module. Each new module asserts assoc + comm + idem + identity-with-bottom on the lattice type:
- **SciSet** — `proptest` strategy (open-vocab system identifiers × open compartments × open sub-compartments; capped at ≤3×3×3 to keep runtime <5s under default `proptest::Config`).
- **SarSet** — `proptest` strategy (open program IDs × bounded compartments × bounded sub-compartments).
- **NonIcDissemSet** — brute-force enumeration (closed-vocab `DissemControl` non-IC variants).
- **DisplayOnlyBlock** — hybrid: enum-variant brute-force + small proptest for country payloads.
- **FgiSet join-side** — 3-4 hand-picked tests asserting `Concealed ⊔ Acknowledged{...} = Concealed` per §H.7 p123 (concealed-dominates).

Each module's doc-comment cites the §-passage governing the lattice algebra. Constitution VIII verification at point of authorship.

### D-3. T118 sidecar = parallel type, not `ExpectedFixture` extension
Existing `marque_test_utils::ExpectedFixture` (defined at `crates/test-utils/src/lib.rs:96`) carries `{diagnostics, ground_truth}`. The lattice runner needs an additional `expected_banner: String` field to assert byte-identity between `scheme.project(Scope::Page, ...).render_banner()` and the fixture's trailing banner line. Two options:

- (a) Add an optional `expected_banner: Option<String>` to `ExpectedFixture` — would force every existing `.expected.json` in `valid/`/`invalid/`/`foreign/` to either carry or default the field.
- (b) Define a parallel `LatticeExpectedFixture` type local to the runner module — keeps `ExpectedFixture` stable for the documents corpus contract.

**Decision: (b).** Cleaner contract separation. Risk: the lattice sidecar's `_note` field still carries the §-citation (Constitution VIII propagation discipline) so reviewers can map sidecar to source even without the unified type.

Sidecar shape:
```json
{
  "_note": "PR 4 (006 T118) — <fixture class>. CAPCO-2016 §X.Y pNN: <quotation>",
  "expected_banner": "TOP SECRET//FGI GBR//NOFORN",
  "diagnostics": []
}
```

### D-4. T118 runner location: `crates/capco/tests/lattice_corpus_runner.rs`
Rationale: runner calls `CapcoScheme::project(Scope::Page, ...)` directly; `marque-capco` is the natural home (parallel to `cross_axis_dominance.rs`, `lattice_vs_scheme_parity.rs`, `lattice_static_assertions.rs`, `post_4b_lattice_inventory_pin.rs`). Avoids polluting `crates/engine/tests/corpus_accuracy.rs` (its 95% accuracy-gate logic is orthogonal). Reuses `marque_test_utils::fixtures_in("lattice")` for the walker — no new test-utils helper needed.

### D-5. T118 CAB-shape dispatch
Detect via first **non-blank, non-`#`-comment** line starting with one of `Classified By:`/`Derived From:`/`Declassify On:` (case-insensitive, leading-whitespace tolerant). Confirmed against current fixtures: `aea-commingling.txt` line 1 = `Classified By: First Reviewer` (fires); the other 4 fixtures all start with `(` (portion form — does not fire). Markdown-comment hardening (`#` line skip) protects against future fixture annotations.

```rust
enum FixtureShape { CabCommingling, PortionsBanner }
fn classify_shape(source: &[u8]) -> FixtureShape { /* skip blank + # lines, then prefix-match */ }
```

### D-6. T119 probe ordering — probe first, then gate
40 CIA CREST documents (`tests/corpus/documents/marked/*.md`) have **never been run through `Engine::lint`** — only `Scanner::scan` via `document_corpus.rs::scanner_counts_match_ground_truth`. The `.expected.json` sidecars' `"diagnostics": []` claim is unverified against current rule output. Three failure modes possible:
- (a) Fixtures emit unexpected diagnostics → real engine bugs OR stale ground-truth claims (Constitution VIII — neither tolerated silently)
- (b) Per-document precision logic differs from `prose/`-style zero-diagnostic gate
- (c) Performance: 40 multipage documents may materially exceed `prose/`'s ~8 short fixtures

**Implementation order**:
1. Land probe as `#[ignore]`-gated test (`probe_documents_lint_clean`) in `crates/capco/tests/lattice_corpus_runner.rs`. Prints per-document diagnostic counts to stdout via `println!`. Stays in-tree post-merge as regression replay surface.
2. PM runs probe manually (`cargo test -p marque-capco --test lattice_corpus_runner -- --ignored --nocapture`), reads output.
3. **If clean (40/40 zero-diagnostic)**: add `precision_documents_zero_diagnostics` assertion gate to `crates/engine/tests/corpus_accuracy.rs` mirroring `precision_prose_zero_diagnostics`. Mark T119 [X].
4. **If drift (any fixture emits diagnostics)**: file follow-up issue with per-document triage, mark T119 [ ] (deferred) with issue link, ship the probe surface (still useful as manual diagnostic).

The probe-first ordering avoids merging an assertion that could mask real engine bugs.

### D-7. Constitution VII boundary: clean
Audit:

| Target | Modify | Engine-crate? |
|--------|--------|---------------|
| `crates/capco/tests/cross_axis_dominance.rs` | T117 + T117a | No (tests/) |
| `crates/capco/tests/category_lattice_laws.rs` | T116 (5 modules) | No (tests/) |
| `crates/capco/tests/lattice_corpus_runner.rs` | T118 new | No (tests/) |
| `tests/corpus/lattice/*.expected.json` | T118 new (×5) | No (corpus data) |
| `crates/capco/Cargo.toml` | dev-dep add (`serde_json`) | No (TOML only) |
| `crates/engine/tests/corpus_accuracy.rs` | T119 gate (if probe clean) | No (tests/) |
| `specs/006-engine-rule-refactor/tasks.md` | bookkeeping | No |
| `CLAUDE.md` | Recent Changes | No |

Zero edits required in `crates/{engine,scheme,core,rules,ism,capco}/src/`. No new `pub` exports needed — all target lattice types already `pub` in `pub mod lattice`. **Pre-push verification**: `git diff main -- 'crates/*/src/**'` returns empty.

### D-8. Citation discipline — Constitution VIII at point of authorship
Per project memory `feedback_audit_predicates_against_source`: every `§X.Y pNN` citation in new test doc-comments, sidecar `_note` fields, and the closeout's CLAUDE.md entry MUST be verified against `crates/capco/docs/CAPCO-2016.md` at point of authorship. Propagation between PM doc → implementer → reviewer is in-scope for Constitution VIII's "propagation requires re-verification" clause.

**Checkpoints**:
- T118 sidecars: 5 verifications (one per fixture's §-citation)
- T117 doc-comments: 4 verifications (one per new test)
- T117a doc-comment: 1 verification (§H.7 pp123-129)
- T116 doc-comments: 5 verifications (one per new module, plus FgiSet extension)
- CLAUDE.md propagation: re-verify any citation moved from a doc-comment

### D-9. Perf-regression handling — non-blocking, note in PR body
Per memory `project_perf_regression_4_to_6`: the cumulative `lint_10kb` regression (~914µs → ~1.7ms) across PRs 4-6 is real but accumulated, not introduced by this PR. This PR adds NO hot-path code (tests only). The `Performance regression gates` CI job may flag the existing regression — note in PR body, cite the memory, do not block.

SC-001 (16ms ceiling) not violated; the bench-check gate is the canary, not the contract.

## Open questions resolved (OQ-series)

| OQ | Status | Decision |
|----|--------|----------|
| OQ-1 keep T117 separate from T118 | Resolved | YES — different audit purposes (§-citation traceability vs regression catch) |
| OQ-2 sidecar JSON schema | Resolved | Parallel `LatticeExpectedFixture` type with `expected_banner` (D-3) |
| OQ-3 T118 runner location | Resolved | `crates/capco/tests/lattice_corpus_runner.rs` (D-4) |
| OQ-4 CAB-shape dispatch | Resolved | First non-blank/non-`#`-comment line prefix match (D-5) |
| OQ-5 T119 walker constitution boundary | Resolved | Test-only, no engine src/ touches; reuses existing `marked_document_fixtures()` |
| OQ-6 T116 lean confirmation | Resolved | Push-back accepted; reduced scope per rust-specialist's gap inventory (D-2) |
| OQ-7 Constitution VII boundary | Resolved | Clean (D-7) |
| OQ-8 walked-adjacency T119 | Resolved | Probe-first ordering (D-6) |

## ERRATA — corrections applied at PM-decision time

None at this stage. (Plan is fresh; the architect + rust-specialist outputs converged on the refined scope after one round.)

## Standing project directives binding throughout

- "Quality is everything... will we want to maintain this for 5 years?" — informs T116 expansion accept (D-2).
- Constitution VIII: every §-citation verified against `crates/capco/docs/CAPCO-2016.md` at point of authorship; propagation re-verification (D-8).
- Constitution VII: scheme-adoption boundary — confirmed clean for this test-only PR (D-7).
- Marque is pre-users — no deprecation phasing, no back-compat shims.
- Citations: `§X.Y pNN` form only (memory `feedback_citations_use_page_numbers`).
- All PRs against `staging`.
- Cumulative perf-regression (PRs-4-to-6) acknowledged but not blocking (D-9).
- GPG-signed commits; never `--no-gpg-sign`.
- No force-pushes without explicit user authorization (memory `feedback_no_unauthorized_force_push`).
- Multi-agent reviewer chain (rust + code-reviewer + lattice-consultant) is the load-bearing quality gate; Copilot suppressed low-confidence comments to be checked (user directive).

## Implementation order

The implementer agent should execute in this order to minimize redo risk:

1. **T118 fixtures + runner skeleton** — author 5 `.expected.json` sidecars (best-effort initial guess; runner reveals actual output if assertions misalign), then runner with both dispatch paths.
2. **T117 tests** — 4 new `#[test]` blocks. Same `scheme.project(Scope::Page, ...)` idiom as `lattice_vs_scheme_parity.rs:114-118`.
3. **T117a test** — 1 new `#[test]` block. Mirrors mixed_us_foreign_rollup ground truth.
4. **T119 probe** — `#[ignore]`-gated `probe_documents_lint_clean` in the runner file.
5. **T116 modules** — 4 new `mod` blocks + FgiSet extension. Last because it's the largest single chunk and any drift caught by earlier steps may inform smoke-test design.
6. **`serde_json` dev-dep add** — `crates/capco/Cargo.toml` `[dev-dependencies]`.
7. **Run probe manually**: `cargo test -p marque-capco --test lattice_corpus_runner -- --ignored --nocapture`. PM reads output, makes T119 gate-vs-defer call.
8. **T119 gate (conditional)** — if probe clean, add `precision_documents_zero_diagnostics` to `corpus_accuracy.rs`.
9. **Bookkeeping** — tasks.md `[X]` for T116/T117/T117a/T118, T119 `[X]` or `[ ]` per probe outcome; CLAUDE.md Recent Changes paragraph.

## Estimated PR shape

- **New files (6)**:
  - `crates/capco/tests/lattice_corpus_runner.rs` (~250 LOC including probe)
  - `tests/corpus/lattice/fouo-eviction-class.expected.json`
  - `tests/corpus/lattice/fouo-eviction-non-fdr.expected.json`
  - `tests/corpus/lattice/fgi-banner-rollup.expected.json`
  - `tests/corpus/lattice/sci-cross-system.expected.json`
  - `tests/corpus/lattice/aea-commingling.expected.json`
- **Modified files (5-6)**:
  - `crates/capco/tests/cross_axis_dominance.rs` (+~320 LOC; 5 new tests)
  - `crates/capco/tests/category_lattice_laws.rs` (+~150 LOC; 4 new modules + FgiSet extension)
  - `crates/capco/Cargo.toml` (+1 line)
  - `specs/006-engine-rule-refactor/tasks.md` (~5 line changes)
  - `CLAUDE.md` (~10 line change)
  - `crates/engine/tests/corpus_accuracy.rs` (+~30 LOC, conditional on T119 probe clean)
- **Total ~700-730 LOC delta** across 11-12 file touches.

## Risk register (from architect + rust-specialist preflight)

1. **T119 / documents corpus regression — HIGHEST RISK** (D-6). 40 fixtures' `.expected.json` claims are unverified against current `Engine::lint`. Probe-first ordering mitigates; if drift found, T119 ships deferred.
2. **SciSet rendering ambiguity in T117 SCI-cross-system test** — within-category render order pinned at `crates/capco/src/render/render_sci.rs:54-98`: SI before TK (alpha tiebreak), `/` separator between systems. Expected output `TOP SECRET//SI-G ABCD/TK-BLFH XYZW` confirmed against actual. Hazard: future render-order changes break the assertion silently.
3. **JointSet T116 smoke test inputs** — `JointSet::join`'s associativity riding on PR 4b-B C-3 split (Mixed out of Bottom) requires inputs that exercise the absorbing transition, not just trivial `Bottom + Bottom`. T116 must select inputs that hit Mixed.
4. **FGI banner roll-up — `fgi-banner-rollup.txt` divergence risk** — `lattice_vs_scheme_parity.rs` has 12 documented `dissem_us` divergences from `CLOSURE_NOFORN_CAVEATED`. The fgi-banner-rollup runner may surface a diagnostic the parity gate is silent on. Compare both gates before merging.
5. **`aea-commingling.txt` CAB-shape diagnostic count unknown** — fixture mixes calendar `Declassify On` with canned `N/A for AEA portions` string. Per §E.4 the canned string dominates; if `Engine::lint` over this fixture today emits E055/E056/E054 the sidecar must encode them. Run before pinning.
6. **`from_parsed_unchecked` test-fixture carve-out comment** — every call site must carry the inline carve-out comment per Constitution V Principle V (CLAUDE.md L1185+). Forgetting trips reviewer attention even if `cfg(test)` enforcement passes.
7. **`MarkingScheme` trait-in-scope** — `scheme.project()` requires `use marque_scheme::MarkingScheme as _` AND `use marque_scheme::Scope` imports. Missing the trait import gives a confusing "no method `project`" error.
8. **`Box<[T]>` vs `Vec<T>`** — `CanonicalAttrs` collection fields are `Box<[T]>` per Constitution II. Assertions compare `attrs.dissem_us.as_ref() == &[...]` not `attrs.dissem_us == vec![...]`.
9. **`MarkingClassification::Us(level)` vs bare `Classification`** — hand-built T117 fixtures must use `MarkingClassification::*` variants, not bare `Classification::*`. Easy to get wrong if test author works from §-references only.
10. **proptest cases × open-vocab state space** — SciSet/SarSet open-vocab strategies cap at ≤3×3×3 to keep runtime <5s under default `proptest::Config::default().cases = 256`.

## Approval

PM signs off on:
- Scope (D-1 single PR)
- Architecture (D-2 through D-6)
- Boundaries (D-7 constitution clean; D-8 citation discipline; D-9 perf non-blocking)
- Risk register (10 entries from preflight; mitigated as noted)

**Branch**: `refactor-006-pr-4-test-closeout` from `staging`.
**Implementer brief**: this document + the architect's plan + the rust-specialist's hazard register + `crates/capco/CAPCO-CONTEXT.md` (full content).

— Adam Poulemanos (PM)
2026-05-19
