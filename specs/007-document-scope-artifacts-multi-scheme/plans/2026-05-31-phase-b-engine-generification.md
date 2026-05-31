# Phase B — Engine Generification Plan

**Status:** DRAFT for sign-off · **Date:** 2026-05-31 · **Author:** oversight (007)
**Branch:** `007-phase-b` (worktree `.worktrees/phase-b`) · **Base:** needs rebase onto `main` (`ff33b23e`, post Phase A #833)

---

## 1. Where Phase B stands

Phase B is split into **cheap wins** (domain-neutral renames + trait-surface prep, no
`Engine<S>` change) and **B-core** (making the engine and decoder generic over the scheme).

**Cheap wins — DONE on `007-phase-b` (tip `3f51bafb`):**

| Commit | Task | What |
|--------|------|------|
| `b77890be` | T025 | additive `#[non_exhaustive]` + `Grammar` escape on `MessageTemplate` / `FeatureId` |
| `1b7f9de3` | T026 | `Zone::Cab → Zone::Custom` + `#[non_exhaustive]` |
| `069aa3a5` | T026 | `ParseContext::classification_floor → rank_floor` |
| `17abfa1b` | T026 | `FormSet` field + `FormKind` domain-neutral renames |
| `be106741` | T026 | `render_portion`/`render_banner` → `render_item`/`render_summary` |
| `4ff63c77` | T026 | move `is_fdr_dissem` to `IcMarkingVocabulary` sub-trait |
| `3f51bafb` | T022 | `scheme.constraint_rule_id` delegation |

The earlier-feared PR-1-agent corruption is **not present** in this log — the branch is
clean and linear on top of the Phase 0b merge (`395d11d1`).

**B-core — NOT started. The wall:**

- `crates/engine/src/engine.rs:229` — `pub struct Engine` is **concrete**, not generic:
  - `rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>` (line 231)
  - `scheme: CapcoScheme` (line 255)
  - recognizer resolves each candidate to a `CanonicalAttrs` (concrete pivot)
  - The field doc comment (lines 232–242) already names the target:
    *"Making `Engine<S>` truly generic over the scheme would replace this field with
    the user-supplied `S`."*
- Coupling magnitude (from prior-session grep — **re-confirm at PR-3 kickoff**, transport
  was buffering during this draft): ~271 `CapcoScheme` refs across ~38 engine `src` files;
  ~115 `CanonicalAttrs` refs; decoder ~17 of ~24 files coupled; `drop(scheme)` bridge at
  `constructors.rs:85`.
- `Rule<S>` / `RuleSet<S>` are **already generic** (`crates/rules/src/rule.rs:122`, `:269`)
  — T020 is partly landed. `RuleContext` is **not yet generic** (the remaining T020 work).

**Remaining B-core tasks:** T020 (`RuleContext<S>`), T021 (`Engine<S>`), T024 (decoder
threading), T028/T028b (`ErasedEngine` object-safety shim), T029 (`MultiGrammarEngine`).

---

## 2. Goal & non-goals

**Goal.** Make the engine pipeline generic over `S: MarkingScheme` so a second grammar
(CUI, NATO, partner-national) can be driven by the same `Engine` core, with `CapcoScheme`
becoming *one* instantiation rather than the hard-wired type. End state: `Engine<S>` is
genuinely generic; `Engine<CapcoScheme>` is the default and every existing call site keeps
working; an `ErasedEngine` trait object lets heterogeneous schemes co-reside behind one
handle (the `MultiGrammarEngine`).

**Non-goals (this phase).**
- No second scheme is *implemented* — Phase B proves the seam with `CapcoScheme` only.
- No audit-schema change. `marque-3.2` is frozen; generification must be wire-invariant.
- No lattice-trait-surface change. Frozen per project policy.
- No new WASM runtime-config surface (Constitution III — the Phase A pin holds).
- No perf regression past the constitutional ceilings (p95 ≤ 2 ms strict/decoder on 10 KB).

---

## 3. Invariants that must hold across every B-core PR

1. **`Engine<CapcoScheme>` call sites are source-unchanged.** Achieved via a default type
   parameter `Engine<S = CapcoScheme>` (the "scaffold, then strip" technique, §4). Every
   PR up to the final one keeps the default so the CLI / server / WASM / tests compile
   without edits; the default is stripped only when multi-scheme is genuinely exercised.
2. **Audit output byte-identical.** Corpus audit-parity tests (`audit_v3_0_parity.rs` and
   the G13 content-ignorance canary) must pass unchanged. Generification touches *types*,
   not *audit content*.
3. **Boxing-frequency invariant for `ErasedEngine`:** ≤ 1 box per scheme per document.
   Erasure happens at the `Engine<S>` → `dyn ErasedEngine` boundary, never per-candidate
   or per-rule. Monomorphized default dispatch (the `Recognizer` enum) stays monomorphized.
4. **WASM-safe set stays WASM-safe.** `marque-scheme/-ism/-core/-rules/-capco` compile to
   WASM unchanged. `Engine<S>` lives in `marque-engine` (not WASM-safe) so this is mostly a
   non-issue, but any trait-surface change that leaks into the WASM-safe crates must be
   checked with `wasm-pack`.
5. **Perf ceiling.** Run the interactive-latency bench (`StrictRecognizer` path) before/after
   PR-3 and PR-4. Generic monomorphization should be perf-neutral; verify, don't assume.
6. **Constitution IV scheme-adoption rule does NOT apply here.** That rule forbids a
   *scheme-adoption* PR from editing engine crates. Phase B is the inverse: it is the
   *engine-generification* work that a future scheme adoption depends on. It legitimately
   edits `marque-engine`/`marque-rules`. (Worth a one-line note in each PR body so a
   reviewer doesn't misfire the rule.)

---

## 4. Strategy: default-param scaffold, then strip

Per your call ("Default-param scaffold, then strip"):

1. Introduce `Engine<S = CapcoScheme>` with a defaulted type parameter. All existing
   `Engine`/`Engine::new()` references resolve to `Engine<CapcoScheme>` with zero edits.
2. Thread `S` through the struct fields and methods **incrementally**, replacing concrete
   `CapcoScheme` → `S` and `CanonicalAttrs` → `S::Canonical` (the scheme's canonical pivot
   associated type). Each step keeps the default, so the tree compiles green throughout.
3. Thread `S` through the decoder last (the long pole — most files, deepest coupling).
4. **Strip the default** (`Engine<S = CapcoScheme>` → `Engine<S>`) only in the final PR,
   once `MultiGrammarEngine` actually instantiates a second `S`. Stripping forces every
   call site to name its scheme — at that point we want the breakage, because it proves no
   site silently assumed CapcoScheme.

The pivot-type mapping `CanonicalAttrs → S::Canonical` is the crux. `CapcoScheme::Canonical`
= `CanonicalAttrs` today; the generification replaces the ~115 concrete uses with the
associated type. Where a use genuinely needs CAPCO-specific structure (it shouldn't, in the
engine), that's a finding — the engine must not reach into scheme-specific fields.

---

## 5. Sub-PR sequencing

> Rebase `007-phase-b` onto `main` (`ff33b23e`) **first** — it currently sits on the Phase 0b
> merge and is behind by Phase A. **Conflict risk:** Phase A added `ParseContext.input_source`
> and Phase B (T026) renamed `classification_floor → rank_floor` + `Zone::Cab → Zone::Custom`,
> both in `recognizer.rs`. Expect a small manual merge in `ParseContext` / `Zone`. Resolve,
> re-run the full test suite, then continue.

### PR-B1 — shared stub `MarkingScheme` (stub-only)  ·  small, low-risk  ·  ✅ LANDED
**Rescoped during implementation.** Originally "finish T020 (`RuleContext<S>`)". On
inspection, `RuleContext` is composed *entirely* of concrete `marque-ism` types —
`page_portions: Arc<Box<[CanonicalAttrs]>>`, `page_marking: Arc<ProjectedMarking>`,
`pre_pass_1_attrs: &CanonicalAttrs` — and `Rule<S>::check` takes `attrs: &CanonicalAttrs`
(concrete), so `Rule<S>` is generic only in its *output* (`Diagnostic<S>`). A
`RuleContext<S>` now is therefore either (a) a hollow `PhantomData` param threaded
through ~48 files for zero observable benefit, or (b) the substantive field migration to
`S::Canonical`/`S::Projected` — which *forces* `check`'s `attrs` to `&S::Canonical` and
**is** the B3 long pole (the ~115 `CanonicalAttrs` refs). No clean middle exists.

**`RuleContext<S>` moved to PR-B3**, where flipping its fields to associated types is
substantive rather than ceremonial, landing in the same change that migrates the decoder.

PR-B1 lands the genuinely-valuable, non-hollow half:
- New `crates/test-utils/src/stub_scheme.rs` — the smallest lawful `MarkingScheme` +
  `Vocabulary<S>`/`Recognizer<S>`/`Codec<S>`, lifted from the self-contained stub in
  `crates/scheme/tests/adoption_readiness.rs` and promoted to a shared, reusable module.
  Every later B PR instantiates this as the second `S`.
- `marque-test-utils` gains a normal dep on `marque-scheme` (WASM-safe leaf); the crate is
  `publish=false` and consumed only under `[dev-dependencies]`, so no shipping crate's
  normal dep graph or the WASM artifact is affected (Constitution VII/III preserved).
- `tests/stub_scheme_usable.rs` — consumer-side smoke test proving the stub drives the
  generic surface from outside `marque-scheme`.
- `adoption_readiness.rs` left untouched (refactoring it to consume the shared stub would
  introduce a dev-dependency back-cycle, since `marque-scheme` does not dev-dep
  `marque-test-utils`; the duplicated self-contained copy stays).
- Gate (met): `cargo check --workspace --all-targets`, `clippy -p marque-test-utils`,
  `fmt --check`, `cargo test -p marque-test-utils` — all green.

### PR-B2 — T022 finish + T021 scaffold (`Engine<S = CapcoScheme>`)  ·  medium
- Add the defaulted type parameter to `Engine` and its `impl` blocks. Replace the
  `scheme: CapcoScheme` field with `scheme: S`; replace `rule_sets: Vec<Box<dyn
  RuleSet<CapcoScheme>>>` with `Vec<Box<dyn RuleSet<S>>>`.
- Do **not** thread the decoder yet — keep a bounded `where S = CapcoScheme` shim at the
  decoder boundary (or an explicit `CapcoScheme`-only `impl` block) so the tree compiles.
- This is where `drop(scheme)` at `constructors.rs:85` either becomes a real field store or
  is removed.
- Gate: full suite green; audit-parity tests green; the default keeps all call sites intact.

### PR-B3 — T020 `RuleContext<S>` + T021/T024 decoder threading (`S::Canonical`)  ·  **long pole**
- **Now also carries T020** (moved from B1): flip `RuleContext`'s `page_portions` /
  `page_marking` / `pre_pass_1_attrs` fields and `Rule<S>::check`'s `attrs` parameter from
  concrete `CanonicalAttrs`/`ProjectedMarking` to `S::Canonical`/`S::Projected`. This is
  the same migration as the decoder threading, not a separable step — doing it here is what
  makes the type param substantive instead of a hollow `PhantomData`.
- Thread `S` / `S::Canonical` through the ~17 coupled decoder files + remaining engine src.
- Replace ~115 `CanonicalAttrs` → `S::Canonical`. Remove the PR-B2 decoder shim.
- This is the multi-day RED→GREEN swing. Recommend landing it behind the still-present
  default param so the *external* surface doesn't move — only internals generify.
- Gate: full suite; **interactive-latency bench before/after** (Invariant 5); audit-parity;
  G13 canary; `wasm-pack` (engine isn't WASM but verify nothing leaked into the safe set).
- Recommend dispatching `rust-reviewer` + `code-reviewer` on this PR **before** open (per
  project policy), given its size.

### PR-B4 — T028/T028b `ErasedEngine` + T029 `MultiGrammarEngine` + strip default  ·  medium
- Define the object-safe `ErasedEngine` trait (erase `S` behind `dyn`), honoring the
  ≤ 1-box-per-scheme-per-document boxing invariant (Invariant 3). T028b is the spike that
  proves object-safety; T028 is the real shim.
- `MultiGrammarEngine`: a registry of `Box<dyn ErasedEngine>` keyed by scheme, dispatching a
  document to the right erased engine.
- **Strip the `= CapcoScheme` default** here. This breaks any site that assumed CapcoScheme;
  fix each to name its scheme explicitly. The breakage is the proof.
- Gate: full suite; a new integration test instantiating two distinct `S` (CapcoScheme + a
  minimal stub scheme) through one `MultiGrammarEngine`; perf bench; reviewer chain.

---

## 6. Test strategy

- **Stub scheme.** PR-B1 introduces a minimal `MarkingScheme` test stub (smallest lawful
  impl) so generic code can be exercised against a *second* `S` from the start — otherwise
  "generic" code that only ever sees `CapcoScheme` can hide monomorphization assumptions.
- **Parity gates (unchanged, must stay green every PR):** `audit_v3_0_parity.rs`, G13
  content-ignorance canary, the corpus accuracy harness (≥ 95% lint/fix per-rule), the
  post-3b registration pin.
- **Perf gates:** `lint_10kb` / decoder interactive-latency bench at PR-B3 and PR-B4.
- **Object-safety:** a compile-test (`static_assertions` or a `dyn ErasedEngine` coercion in
  a test) locks T028b.

---

## 7. Open questions for sign-off

1. **Worktree hygiene.** B-core PR-B3 is large and long-running. Given the earlier
   concurrent-worktree oscillation, I propose: **at most one worktree-mutating agent at a
   time**, serial single commands, push after each commit. OK?
2. **Reviewer chain on PR-B3/B4.** Confirm you want `rust-reviewer` + `code-reviewer`
   dispatched *before* PR-open (not relying on Copilot's reactive pass), per the standing
   "run reviewer before PR-open" preference.
3. **Stub-scheme location.** Put the minimal test `MarkingScheme` in
   `crates/rules/tests/` (integration-only) vs. a `#[cfg(test)]` module vs. a
   `marque-test-utils` helper? I lean `marque-test-utils` so engine + rules tests share it.
4. **Strip timing.** Confirm stripping the default param in PR-B4 (not earlier). Keeping it
   through B3 is what protects the external surface during the long pole.
5. **tasks.md update.** I'll reconcile `tasks.md` (Phase B = T020–T029) to this 4-PR
   re-sequencing once you approve the shape — deferred until the plan is solid, per your call.

---

## 8. One-line recommendation

Land **PR-B1** (`RuleContext<S>`, small, WASM-safe) immediately after the rebase to validate
the seam, then proceed B2 → B3 → B4 serially. The risk is concentrated entirely in **PR-B3**
(decoder threading); everything else is mechanical.
