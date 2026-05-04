<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Contract: Engine Pipeline

**Lands at**: PR 6 (Scope::Page projection cutover) and PR 7 (phase-tagged pass split); built on PR 0..PR 3c foundations
**Spec FRs**: FR-006, FR-021, FR-022, FR-023, FR-024, FR-029, FR-030, FR-031, FR-032, FR-038, FR-041
**Source-plan refs**: §5 invariants register (I-1..I-19), §6 test strategy, §9 pass-split semantics
**Audience**: engine maintainers, rule-set authors, CI / bench operators.

---

## Pipeline overview

```text
Input bytes (&[u8])
   │
   ▼ Phase 1 — Scanner (marque-core::scanner)
SpanStream  (memchr SIMD; zero-allocation per candidate; Constitution II)
   │
   ▼ Phase 2 — Parser (marque-core::parser)
ParsedAttrs<'src>  (aho-corasick automaton; FR-015 shape_admits at four sites; FR-016 returns None on failure)
   │
   ▼ Phase 3 — Canonicalization (MarkingScheme::canonicalize)
CanonicalAttrs  (FR-007: classification: Option; FR-017: FgiMarker discriminant)
   │
   ┝━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
   ▼ Phase 4 — Rule evaluation (per-portion)                    │
   │                                                              ▼ Phase 5 — Page projection (Scope::Page)
   │                                                    MarkingScheme::project(Scope::Page, &portions)
Vec<Diagnostic>                                                   │
   │                                                              ▼
   │                                                    ProjectedMarking
   │                                                              │
   │                                                              ▼ Phase 5b — Banner-validation rules
   │                                                    Vec<Diagnostic>
   │                                                              │
   ▼━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
Vec<Diagnostic>
   │
   ▼ Phase 6 — Engine fix dispatch (Engine::fix_inner)
   │
   ├─ Pass 1: Phase::Localized rules' fixes
   │     │
   │     ▼ Single-pass forward splice (FR-029)
   │   post-pass-1 buffer
   │     │
   │     ▼ Re-parse
   │   ┌─────┴─────┐
   │   │           │
   │  OK         FAIL → R002 diagnostic (FR-024); pass-1 records retained;
   │   │                pass-2 skipped; return pass-1 buffer.
   │   ▼
   │ Pass 2: Phase::WholeMarking rules' fixes
   │     │  - I-18 non-overlap with pass-1 spans (FR-022)
   │     │  - I-19 reshape-aware re-validation (FR-023)
   │     ▼
   │   single-pass forward splice
   │     ▼
   │   final corrected buffer
   │     │
   ▼     ▼
Vec<AppliedFix>  (FR-002 content-ignorant; FR-026 (scheme, predicate-id) IDs; FR-035 schema marque-1.0)
```

---

## Phase boundaries (Constitution VI invariant)

Each phase is independently testable; no phase calls into another's
internals.

| Phase | Owns | Reads | Writes |
|---|---|---|---|
| 1 Scanner | `marque-core::scanner` | `&[u8]` | `SpanStream` |
| 2 Parser | `marque-core::parser` | `&[u8]`, `SpanStream` | `ParsedAttrs<'src>` |
| 3 Canonicalize | `MarkingScheme::canonicalize` | `ParsedAttrs<'src>` | `CanonicalAttrs` |
| 4 Rule eval (portion) | `Rule::evaluate` | `&CanonicalAttrs`, `&RuleContext` | `Vec<Diagnostic>` |
| 5 Page projection | `MarkingScheme::project(Scope::Page, ...)` | `&[CanonicalAttrs]` (portion canonical forms) | `ProjectedMarking` |
| 5b Banner-val rules | `Rule::evaluate` (banner phase) | `&ProjectedMarking`, `&RuleContext` | `Vec<Diagnostic>` |
| 6 Fix dispatch | `Engine::fix_inner` | `Vec<Diagnostic>`, `&[u8]` | `Vec<AppliedFix>`, corrected buffer |

Topological scheduler (`marque-engine::scheduler`, existing) orders
declarative `PageRewrite`s among themselves before banner-validation
rules see `ProjectedMarking`. Cycles fail at `Engine::new`
(`EngineConstructionError::RewriteCycle`); unannotated `Custom` axes
fail with `UnannotatedCustomAxes`.

---

## `Scope::Page` projection cutover (PR 6, FR-006)

PR 6 sub-divides into three commits to make the cutover measurable
and revertable:

### Commit 6a — projection behind feature flag

`MarkingScheme::project(Scope::Page, ...)` is wired but gated by a
build-time `cfg(feature = "scope_page_projection")` (default off).
`PageContext` remains the default page-rollup mechanism. Both code
paths are present; CI runs both.

### Commit 6b — bench both paths

`lint_100kb_multipage` Criterion bench runs against both code paths
and asserts:
- Projection-path latency ≤ `PageContext`-path baseline + 10%
  (FR-031).
- Both paths produce identical `Vec<Diagnostic>` on a fixture corpus
  (semantic equivalence test).

If either assertion fails: investigate (revert 6a if necessary).

### Commit 6c — flip default and delete `PageContext`

Default flips to projection; `cfg(feature = "scope_page_projection")`
removed; `PageContext` deleted (FR-006 — clean break, no equivalence
shim window). All page-rollup goes through projection. The
`lint_100kb_multipage` bench post-merge measures projection-only.

---

## Pass-split semantics (PR 7, FR-021..FR-024)

### Pass 1 — `Phase::Localized` rules

Pre-conditions:
- Each `Phase::Localized` rule's `FixIntent::target_span` is
  sub-token-only (enforced at `Engine::new` registration).
- Confidence-threshold filter applied (`Confidence::combined() ≥ threshold`).
- Non-overlap (C-1 guard, existing).

Action:
- Sort fixes by span (ascending start; descending start as tiebreaker
  for same-start to maintain deterministic ordering).
- Apply via single-pass forward splice (FR-029, R² ≥ 0.9 linear scaling).
- Construct `AppliedFix` v2 records via `Engine::fix_inner ::
  __engine_promote(...)`.

Post-conditions:
- post-pass-1 buffer is the input with pass-1 fixes spliced in.
- pass-1 `AppliedFix` records are appended (monotonic; I-5).

### Re-parse (between Pass 1 and Pass 2)

```text
let post_pass_1_attrs: Result<ParsedAttrs<'_>, ParseError> = parse(post_pass_1_buffer);
match post_pass_1_attrs {
    Ok(attrs) => proceed_to_pass_2(canonicalize(attrs)),
    Err(_) => emit_r002_and_return(),
}
```

Post-conditions on success: pass-1 reshape is reflected in `attrs`
(I-4); pass-2 rules see the post-rewrite token spans.

Post-conditions on failure (FR-024):
- Engine emits `R002` diagnostic with `contributing_pass1_fix_ids`
  populated from the pass-1 `AppliedFix` IDs.
- Pass-1 audit records are retained (the fixes happened; the audit log
  is honest about what was applied).
- Pass 2 does not run.
- Engine returns the pass-1 buffer as the corrected document.
- Return shape carries the union of pass-1 `AppliedFix` and the R002
  `Diagnostic` (no `AppliedFix` for R002 — it's diagnostic-only).

#### R002 surfacing semantics (consumer-surface contract — D1)

§9.4 specifies engine-side semantics; this section specifies what
each consumer surface MUST do with R002. Per **decision D1** in
`decisions.md`:

| Consumer | Surface contract |
|----------|------------------|
| **CLI** (`marque check` / `marque fix`) | Distinct exit code `EX_R002_PARTIAL` (numeric value chosen at PR 7 implementation; documented in `marque/src/main.rs` exit-code table). Distinct from `EX_DIAG_WARN` and from regular fix-failure. The CLI prints the R002 diagnostic on stderr with a clear "partial application" indicator. |
| **WASM** (`marque-wasm`) | Typed return shape signaling partial application — either `LintResult { partial: true, .. }` flag or a typed `Result` variant. The binding constraint: consumers MUST be able to detect R002 without parsing NDJSON. Format choice (flag vs. typed variant) is implementer's call at PR 7. |
| **IDE plugins** | Documented contract: plugins MUST inspect the R002 diagnostic before applying the returned buffer. The buffer is the post-pass-1 state; applying without inspection silently splices pass-1 fixes into the user's editor — destructive without consent. The IDE-plugin reference implementation MUST refuse the partial buffer or prompt the user. |
| **`BatchEngine`** | Per-row R002 surfaces in the row's individual result. The batch exit code is **worst-row-wins**: any row hitting R002 raises the batch exit code to `EX_R002_PARTIAL`. Per-row records remain individually inspectable (`id`-correlatable per the existing completion-order contract). |

Rationale: the engine's "honest about partial progress" property (§9.4)
is meaningful only if consumers have a mechanical signal to act on. The
IDE-plugin failure mode (silent partial-buffer application to a user's
editor) is destructive without an explicit refuse-or-prompt contract.

### Pass 2 — `Phase::WholeMarking` rules

Pre-conditions:
- Each `Phase::WholeMarking` rule's `FixIntent::target_span` covers a
  full marking (enforced at `Engine::new` registration).
- post-pass-1 buffer parsed successfully → fresh `CanonicalAttrs`.
- Pre-pass-1 attrs cache (`SmallVec<[CanonicalAttrs<'src>; 4]>` per
  R-4) is in scope; populated only for rules whose span overlaps a
  pass-1 fix.

Action per rule call:
- `RuleContext.pre_pass_1_attrs` is `Some(&pre_attrs)` if the rule's
  span overlaps any pass-1 fix span; otherwise `None`.
- Rule evaluates against post-pass-1 `CanonicalAttrs`.
- I-19 reshape-aware re-validation (FR-023):
  - If `pre_pass_1_attrs.is_some()` and the rule's predicate held
    against `pre_pass_1_attrs.unwrap()` → check the disambiguation:
    - Same `RuleId` (or same `(scheme, predicate-id)` key) as the
      overlapping pass-1 fix → DO NOT re-fire.
    - Different rule → fire (different predicate that pass-1 didn't
      address).
- I-18 non-overlap (FR-022): pass-2 fix spans MUST NOT overlap any
  pass-1 fix span. Engine filters at the dispatch step; overlapping
  pass-2 diagnostics demote to `Severity::Suggest` (FR-042 — new
  variant introduced by this refactor; not auto-applied; non-blocking
  exit code; serializes as `"suggest"` per `contracts/audit-record.md`)
  rather than being silently dropped — the user sees the suggestion
  in CLI / IDE output.

Post-conditions:
- pass-2 `AppliedFix` records are appended to the audit log after
  pass-1 records.
- Final corrected buffer is post-pass-1 buffer with pass-2 fixes
  spliced in.

---

## Engine construction (PR 0, FR-038; PR 7, FR-021)

`Engine::new(scheme, recognizer, rule_set, config) -> Result<Engine,
EngineConstructionError>` performs all once-at-construction validation:

1. **`Send + Sync` static assertion** (FR-038): `static_assertions::assert_impl_all!(Rule: Send + Sync)` and `assert_impl_all!(Recognizer<S>: Send + Sync)`. Compile-fail if violated.
2. **Phase span-shape registration check** (FR-021): for each rule in `rule_set`, inspect declared `Phase` and assert that the rule's span-shape contract is consistent. Implementation can be partial (compile-time shape check via type-level encoding where possible; runtime check at `Engine::new` for cases not amenable to compile-time check). Violations return `EngineConstructionError::PhaseSpanShapeMismatch`.
3. **Topological PageRewrite scheduling**: existing — sort by `(reads, writes)` axes; cycles → `EngineConstructionError::RewriteCycle`; unannotated `Custom` axes → `EngineConstructionError::UnannotatedCustomAxes`.
4. **Decoder dispatcher install**: existing — install `StrictOrDecoderRecognizer` as default unless caller passed `with_recognizer(StrictRecognizer)` (existing `Engine::with_recognizer` API, preserved through this refactor).

---

## Performance budgets (FR-029..FR-033)

| Bench | Lands | Gate |
|---|---|---|
| `fix_throughput` (R² ≥ 0.9) | already landed (PR #278) | linear scaling preserved (FR-029) |
| `lint_latency` (SC-008 with p99 added) | PR 2 | p95 ≤ 16 ms; p99 ≤ baseline + 5% (FR-030) |
| `lint_100kb_multipage` | PR 6 | projection-path latency ≤ baseline + 10% (FR-031) |
| `fix_10kb` | PR 7 | two-pass overhead within p95 ≤ 16 ms budget (FR-032) |

Measurement-gating discipline (FR-033): >5% mean OR p99 regression on
any bench backs out the originating change. Pre-refactor baselines
captured at PR 0 per R-5; subsequent PRs assert against
`benches/baselines/2026-05-pre-refactor.json`.

---

## Synthetic engine diagnostics (FR-041)

R001 (decoder recognition) and R002 (re-parse failure) are minted by
`marque-engine`, not by rule crates. They appear in the `Diagnostic`
stream alongside rule-emitted diagnostics; they use the reserved
top-level scheme `"engine"` with predicate IDs in `rNNN.<descriptor>`
form (for example, `r001.decoder-recognized` and
`r002.reparse-failed`); their messages use closed `MessageTemplate`
variants (`DecoderRecognized`, `ReparseFailed`).

R001 lands today (existing); R002 lands at PR 7 (FR-024).
Centralizing engine-synthetic IDs into `marque-rules` is noted as a
separate refactor in plan §9.4 — not in scope for 006.

---

## Concurrency contract (Constitution VI)

`BatchEngine` (existing) wraps `Engine` behind `Arc` and uses
`recoco-utils::ConcurrencyController` for row + byte semaphore
backpressure. CPU-bound work runs on `tokio::task::spawn_blocking`.
Results stream out in completion order, not submission order.

The refactor preserves these properties:
- `Rule: Send + Sync` static-asserted (FR-038) so rules are safe to
  share across spawned tasks.
- `Recognizer<S>: Send + Sync` static-asserted similarly.
- Per-document state (pre-pass-1 attrs cache, intermediate buffers) is
  stack-frame-local to `Engine::fix_inner` (R-4) — no cross-document
  shared mutable state.
- `MarkingScheme` and `Vocabulary<S>` are `Send + Sync + 'static`
  (per-trait bound) and immutable; the constitution's Principle VI
  prohibition on hidden global mutable state is preserved.

---

## Test strategy (consolidated plan §6)

Six-layer property-test architecture (Layer 0 added per **decision D10**
in `decisions.md`):

0. **Layer 0 — type-system compile-fail tests** (PR 0 + PR 3c, gates FR-001 / FR-003 / FR-005). Run via `trybuild`. Demonstrate that:
   - No public `Box<str> → Canonical` constructor exists for closed-CVE tokens (FR-001).
   - `Diagnostic::message` cannot be constructed with `format!`-interpolated input bytes (FR-003).
   - `AppliedFix::__engine_promote` cannot be called from outside `Engine::fix_inner` in `cfg(not(test))` code (FR-005, complementing the AST lint at FR-040).
   Layer 0 runs **per-PR** (not per-save — full compile per case is slow). The workspace pins a `rust-toolchain.toml` and an exact `trybuild` version to keep expected stderr stable across compiler upgrades; MSRV bumps trigger Layer-0 maintenance as accepted cost.
1. **Layer 1 — lattice law tests per category** (PR 4, gates I-17): assoc/comm/idem/identity at `crates/capco/tests/category_lattice_laws.rs`; cross-axis dominance at `crates/capco/tests/cross_axis_dominance.rs`.
2. **Layer 2 — parse–render round-trip** (PR 2): strict-path round-trip at `crates/capco/tests/parse_render_roundtrip.rs`.
3. **Layer 3 — per-pass fix invariants** (PRs 3c + 7, gates I-1, I-2, I-4, I-18, I-19): `crates/engine/tests/fix_invariants.rs`; deterministic NDJSON canary scan replaces `core_error_isolation.rs`'s masking pin once PR 3c lands.
4. **Layer 4 — corpus regression sweeps** (PR 4 onward): five corpora × two recognizers = ten CI runs (`tests/corpus/{valid,mangled,prose,prose-positive,lattice}/`).
5. **Layer 5 — citation lint** (PR 0.5 skeleton + PR 10 maturation).

Each layer's pass condition gates the relevant FRs and SCs in the
spec. Layer 0 is the keystone evidence for the type-system invariants
(sealed-construction, message-channel closure, engine-only promotion);
Layer 3's canary scan is the construction-of-record for SC-001.
