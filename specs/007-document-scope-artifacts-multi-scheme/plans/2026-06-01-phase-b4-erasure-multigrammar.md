# Phase B4 — Object-Safe Erasure + MultiGrammarEngine + Strip Defaults

**Status:** SIGNED OFF (2026-06-01) · **Date:** 2026-06-01 · **Author:** oversight (007)
**Base:** `main` `af57e473` (post B3.4) · **Worktree:** `.worktrees/phase-b`

---

## 1. Where we are

B3 is complete: `Engine<S, R>` is generic over the scheme and recognizer, the
lint/fix pipeline is generic, and as of B3.4 a second scheme is genuinely
**constructible** (`with_clock_and_recognizer` stores the passed `scheme: S`
instead of discarding it). What remains to close Phase B is the original B4
scope from the master plan (`2026-05-31-phase-b-engine-generification.md` §5,
PR-B4) and tasks.md T028 / T028b / T029:

1. **Object-safe `ErasedEngine`** — `MarkingScheme` has associated types and is
   not object-safe, so heterogeneous schemes cannot co-reside as
   `Vec<Engine<S>>`. The erasure shim is the load-bearing co-residence design.
2. **`MultiGrammarEngine`** — a registry of `Box<dyn ErasedEngine>` that runs
   each grammar's single-scheme rules independently.
3. **Strip the `= CapcoScheme` / `= EngineRecognizer` default type params** —
   the "scaffold, then strip" technique's final step.

This phase also folds in the small **engine-minted scheme-literal** cleanup
deferred from B3.3b/B3.4.

---

## 2. Authoritative inputs

- **tasks.md T028 / T028b / T029** (verbatim):
  - **T028** — Object-safe `ErasedEngine` trait + blanket
    `impl<S: MarkingScheme> ErasedEngine for Engine<S>`. Erase `lint`/`resolve`/`claims`
    to `&[u8]` + grammar-erased `Diagnostic` (contracts/multi-scheme.md, C2).
    Tests: two distinct concrete `S` coexist behind `Box<dyn ErasedEngine>`;
    grammar tag round-trips on each diagnostic.
  - **T028b** — `ErasedEngine` working spike (BEFORE Phase E): a real blanket
    impl + minimal `CoherenceRegistry` that compiles and round-trips a grammar
    tag; confirm erasure boxes **at most once per scheme per document** (not per
    span/diagnostic); capture dispatch overhead in a Phase-B smoke bench.
  - **T029** — `MultiGrammarEngine` skeleton holding `Vec<Box<dyn ErasedEngine>>`
    (**no coherence rules yet** — Phase E; **no translator registry** — `Translate`
    cut, research D7). Tests: two grammars register; single-scheme rules run
    independently.
- **contracts/multi-scheme.md C2** — sketches the *end-state* trait with four
  methods (`grammar_id`, `lint_erased`, `resolve_erased`, `claims`) plus a
  `coherence: CoherenceRegistry` field on `MultiGrammarEngine`.

### 2a. Reality-check against the contract (what exists today)

The C2 sketch is the Phase-B-through-Phase-E *target*. Three of its four trait
methods and the `coherence` field depend on surfaces **that do not exist yet**:

| C2 element | Underlying surface | Status today |
|------------|--------------------|--------------|
| `lint_erased` | `Engine::lint*` | **exists** ✅ |
| `resolve_erased` → `ErasedResolved` | scope-`resolve` surface | **does not exist** (later phase) |
| `claims` → `Claim` (D8 ownership routing) | `Claim` type, ownership routing | **does not exist** (later phase) |
| `coherence: CoherenceRegistry` | `CoherenceRule<A,B>` | **Phase E** (T029 says "no coherence rules yet") |

Building `resolve_erased` / `claims` / `CoherenceRegistry` now would require
`unimplemented!()` stubs in delivered code — which the coding standards forbid
("no stub implementations", "no not-implemented stubs") and which YAGNI
counsels against. The de-risking goal T028b actually states — *prove
object-safety, round-trip a grammar tag, confirm boxing-once* — is fully
achieved by `grammar_id` + `lint_erased` alone.

**Decision (this plan, SIGNED OFF): B4 implements the subset of C2 whose
surfaces exist, plus `fix_erased`.** The trait ships with `grammar_id` +
`lint_erased` + `fix_erased` (fix is a real, existing surface; the user elected
to erase it too — Q1). `resolve_erased` / `claims` arrive in the phase that
lands their underlying surfaces; `CoherenceRegistry` arrives in Phase E. A doc
comment on `ErasedEngine` records the C2 end-state and why those methods are
absent, so the trait reads as a deliberate subset, not an oversight.

### 2b. `Diagnostic<S>` is almost entirely scheme-agnostic already

`crates/rules/src/diagnostic.rs`: the *only* `S`-parameterized field of
`Diagnostic<S>` is `fix: Option<FixIntent<S>>`. Every other field is already
scheme-neutral:

- `rule: RuleId` — a `(scheme: &'static str, predicate_id: &'static str)` 2-tuple
- `severity: Severity`, `span: Span`, `candidate_span: Option<Span>`
- `message: Message` (closed template), `citation: Citation`
- `text_correction: Option<TextCorrection>` (scheme-neutral bytes)
- `recognized_canonical: Option<SecretSlice<u8>>` (bytes)

So the grammar-erased projection is faithful and cheap: carry every field
except the typed `FixIntent<S>`. Rendering (`marque check` output) never prints
the typed intent — it prints severity / span / message / citation /
`recognized_canonical` — so dropping `FixIntent<S>` loses nothing the erased
*lint* surface needs. (Fix application, which *does* consume `FixIntent<S>`,
stays on the typed `Engine<S>` path and is out of the erased surface — see §4.)

---

## 3. Goal & non-goals

**Goal.** Land the object-safe co-residence seam so heterogeneous schemes can
sit behind one handle, strip the scaffold defaults, and remove the last
hardcoded scheme literal in engine-minted output. End state: `Box<dyn
ErasedEngine>` works for any `Engine<S, R>`; `MultiGrammarEngine` runs N
grammars' lint independently; `Engine` has no defaulted type param.

**Non-goals (this phase).**
- No coherence rules / `CoherenceRegistry` (Phase E).
- No `resolve_erased` / `claims` (their surfaces don't exist yet).
- No `Translate` (cut — research D7, tracked as #829).
- No second *production* scheme — co-residence is proven with `CapcoScheme` +
  the `StubScheme` test double.
- No audit-schema change (`marque-3.2` frozen; CAPCO output byte-identical).
- No perf regression past p95 ≤ 2 ms strict/decoder on 10 KB (Constitution I).

---

## 4. Design

### 4.1 `ErasedDiagnostic` + `ErasedLintResult` (new, `marque-engine`)

```rust
/// A grammar-erased projection of `Diagnostic<S>` — every field except the
/// scheme-typed `FixIntent<S>`, which the erased lint surface does not render.
#[non_exhaustive]
pub struct ErasedDiagnostic {
    pub rule: RuleId,
    pub severity: Severity,
    pub span: Span,
    pub candidate_span: Option<Span>,
    pub message: Message,
    pub citation: Citation,
    pub text_correction: Option<TextCorrection>,
    pub recognized_canonical: Option<secrecy::SecretSlice<u8>>,
    pub has_fix: bool,           // whether the typed Diagnostic carried a FixIntent
}

/// Grammar-tagged, scheme-agnostic lint result. The `grammar_id` tag
/// re-associates every diagnostic in `diagnostics` with the scheme that
/// produced them (all share one grammar — one `lint_erased` call = one scheme).
#[non_exhaustive]
pub struct ErasedLintResult {
    pub grammar_id: &'static str,
    pub diagnostics: Vec<ErasedDiagnostic>,
    pub truncated: bool,
    pub candidates_processed: usize,
    pub candidates_total: usize,
    pub recognized_marking_count: usize,
}
```

`ErasedLintResult` mirrors `LintResult<S>`'s scheme-agnostic fields and adds the
grammar tag. It exposes the same count accessors the CLI/server need
(`error_count` / `warn_count` / `fix_count` / `is_clean`).

**Tag placement.** The contract says "tagged `Diagnostic`." We tag at the
*result* level (`ErasedLintResult.grammar_id`), not per-diagnostic, because
every diagnostic from one `lint_erased` call comes from one scheme — a
per-diagnostic tag would be N copies of the same `&'static str`. (Note: a
rule-emitted diagnostic's `rule.scheme()` already names its grammar, but
engine-minted sentinels use `scheme = "engine"`, so `rule.scheme()` is not a
reliable grammar tag — the result-level tag is.) Documented inline.

### 4.2 `ErasedEngine` trait (new, `marque-engine`)

```rust
/// Object-safe façade over a concrete `Engine<S, R>`. `MarkingScheme` is not
/// object-safe (associated types), so this shim is the co-residence seam:
/// it erases the scheme to bytes in / grammar-tagged diagnostics out, and the
/// concrete `S` re-emerges only inside the blanket impl.
///
/// # Subset of contracts/multi-scheme.md C2
///
/// C2's end-state trait also declares `resolve_erased` and `claims`, and pairs
/// the registry with a `CoherenceRegistry`. Those depend on the scope-resolve
/// surface, the `Claim` ownership-routing type, and Phase-E coherence rules —
/// none of which exist yet. This trait ships the subset whose surfaces exist
/// (`grammar_id` + `lint_erased`); the rest land with their surfaces.
pub trait ErasedEngine: Send + Sync {
    fn grammar_id(&self) -> &'static str;
    fn lint_erased(&self, input: &[u8], ctx: &InputContext<'_>) -> ErasedLintResult;
    fn fix_erased(&self, input: &[u8], mode: FixMode) -> ErasedFixResult;
}
```

`Send + Sync` matches the C2 signature and the engine's existing `Send + Sync`
discipline (Constitution VI) — `BatchEngine` already requires it.

`fix_erased` (Q1) erases `FixResult<S>` to a grammar-tagged, scheme-agnostic
form. The S-typed pieces of `FixResult<S>` are `audit_lines: Vec<AuditLine<S>>`
and `remaining_diagnostics: Vec<Diagnostic<S>>`; the erasure pre-renders the
audit lines to NDJSON `Vec<String>` (via the existing `audit_line_to_ndjson`)
and projects the diagnostics to `ErasedDiagnostic`. `source: SecretSlice<u8>`,
`r002_fired: bool`, and `session_metadata: SessionMetadata` are already
scheme-agnostic and carry over verbatim. Uses the infallible `Engine::fix`.

```rust
/// Grammar-erased projection of `FixResult<S>`.
#[non_exhaustive]
pub struct ErasedFixResult {
    pub grammar_id: &'static str,
    pub source: secrecy::SecretSlice<u8>,
    pub audit_ndjson: Vec<String>,            // AuditLine<S> pre-rendered, scheme-agnostic
    pub remaining_diagnostics: Vec<ErasedDiagnostic>,
    pub r002_fired: bool,
    pub session_metadata: SessionMetadata,
}
```

### 4.3 Blanket impl (new, `marque-engine`)

```rust
impl<S, R> ErasedEngine for Engine<S, R>
where
    S: MarkingScheme + ConstraintBridge,
    S::Canonical: Clone + Default + PartialEq,
    R: Recognizer<S>,
{
    fn grammar_id(&self) -> &'static str { self.scheme().scheme_id() }

    fn lint_erased(&self, input: &[u8], ctx: &InputContext<'_>) -> ErasedLintResult {
        let typed = self.lint_with_input_context(input, &LintOptions::default(), ctx);
        ErasedLintResult::from_typed(self.scheme().scheme_id(), typed)
    }
}
```

`from_typed` projects `LintResult<S>` → `ErasedLintResult` field-by-field,
mapping each `Diagnostic<S>` to `ErasedDiagnostic` (drop `fix`, set
`has_fix = typed.fix.is_some()`). The bounds match the lint pipeline's existing
bounds (B3.3b). **Boxing-once invariant**: erasure happens at the
`Engine<S>` → `Box<dyn ErasedEngine>` boundary (constructed once per scheme and
held in the registry); each `lint_erased` call is a single vtable dispatch, no
per-candidate or per-diagnostic boxing.

### 4.4 `MultiGrammarEngine` (new, `marque-engine`)

```rust
/// Registry of co-resident grammars. Phase-B skeleton: runs each grammar's
/// single-scheme lint independently. No coherence rules (Phase E), no
/// translator (cut — #829).
pub struct MultiGrammarEngine {
    engines: Vec<Box<dyn ErasedEngine>>,
}

impl MultiGrammarEngine {
    pub fn new() -> Self { Self { engines: Vec::new() } }
    pub fn register(&mut self, engine: Box<dyn ErasedEngine>) { self.engines.push(engine); }
    pub fn grammar_ids(&self) -> impl Iterator<Item = &'static str> + '_ { /* ... */ }

    /// Lint `input` through every registered grammar independently, returning
    /// one grammar-tagged result per grammar (no cross-grammar coherence yet).
    pub fn lint(&self, input: &[u8], ctx: &InputContext<'_>) -> Vec<ErasedLintResult> {
        self.engines.iter().map(|e| e.lint_erased(input, ctx)).collect()
    }
}
```

No `CoherenceRegistry` field — omitted per YAGNI until Phase E lands coherence
rules. The doc comment records the C2 end-state field.

### 4.5 Strip the defaults

Remove `= CapcoScheme` from `struct Engine` and `= EngineRecognizer` from its
`R` param. Recon confirms **zero external breakage**: every call site
(`marque/src/main.rs`, `crates/server`, `crates/wasm`, all benches, all
out-of-crate integration tests) already passes `marque_engine::default_scheme()`
to `Engine::new`, and `Engine::new` is a concrete `impl Engine<CapcoScheme,
EngineRecognizer>` method (B3.4) that resolves `S`/`R` independent of the struct
default. The `CapcoEngine` alias stays fully-qualified.

**Open decision (Q3):** whether to also strip the `= CapcoScheme` default from
the *output* types (`LintResult`, `FixResult`, `EngineError`) and the
`pub(super)` fix-pass types (`Pass0/1/2Result`, `TwoPassFixer`, `AppliedTuple`,
`PrePass1Cache`, `Pass1/2DiagRefs`). The master plan only mandates stripping the
*Engine* default. Output-type defaults are ergonomic for the CAPCO common case
(CLI/server/WASM annotate `LintResult` bare) and don't hide an *engine* scheme
assumption. Recommendation: **strip `Engine` + `R` only; keep the output-type
defaults.** The internal `pub(super)` defaults can be stripped or kept — they're
impl detail; lean keep to minimize churn.

### 4.6 Engine-minted scheme-literal cleanup

**Recon correction.** The B3.4 notes recorded "4 engine-minted
`RuleId::new(\"capco\", …)` sites." Re-inspection shows **3 of the 4 are
`#[cfg(test)]` test fixtures** in `output.rs` (the `info_count_*` test —
legitimately CAPCO-flavored test data, not production). The only confirmed
*production* engine-minted hardcoded scheme literal is the corrections-map C001
diagnostic in `pipeline.rs` (`marking.correction.token-typo`), where
`self.scheme` is in scope.

**Fix:** route the scheme half through `self.scheme.scheme_id()`:
`RuleId::new(self.scheme.scheme_id(), "marking.correction.token-typo")`. At
`S = CapcoScheme`, `scheme_id()` returns `"capco"` → **byte-identical** wire
string → audit-parity and config-key resolution unchanged; correct for any other
scheme.

**B4.1 will grep the full production set** (`RuleId::new("<literal>"` outside
`#[cfg(test)]`) before changing anything, and convert every confirmed production
engine-minted site. Test-fixture `RuleId::new("capco", …)` and `RuleId::new("test", …)`
stay as-is (they are not engine-minted output). Rule-crate diagnostics emitting
`RuleId::new("capco", …)` are correct (a CAPCO rule *should* say "capco") and are
out of scope.

---

## 5. PR split

| PR | Scope | Risk |
|----|-------|------|
| **B4.1 — scheme-neutrality cleanup** | Strip `Engine` + `R` defaults (and decided output-type defaults); fix production engine-minted scheme literal(s) via `self.scheme.scheme_id()`. CAPCO byte-identical. | Low — mechanical + 1 one-liner |
| **B4.2 — erasure + registry** | `ErasedDiagnostic` / `ErasedLintResult` / `ErasedEngine` + blanket impl + `MultiGrammarEngine` + smoke bench + two-scheme tests (T028/T028b/T029). | Medium — the object-safety design |

B4.1 lands first (proves no site silently assumed CapcoScheme + removes the last
literal), B4.2 builds the erasure surface on the clean base. Keeping them
separate keeps B4.2's diff purely the new surface, not muddied with rename churn.

**Open decision (Q2):** 2 PRs (above) vs folding everything into 1 vs splitting
B4.2 into T028/b (trait+spike) and T029 (registry). Recommendation: **2 PRs** —
the registry is ~30 lines and its two-scheme test *is* the object-safety proof,
so T028/T029 belong together; B4.1 is genuinely separable.

---

## 6. Test strategy

- **Object-safety (T028b):** a `Box<dyn ErasedEngine>` coercion in a test +
  `static_assertions::assert_obj_safe!(ErasedEngine)`.
- **Two-scheme co-residence (T028/T029):** construct `Engine<CapcoScheme, _>` and
  `Engine<StubScheme, StubRecognizer>`, box both, register in one
  `MultiGrammarEngine`, lint a CAPCO portion through it, assert: (a) two results,
  (b) `grammar_id` tags are `"capco"` and `"stub"`, (c) the CAPCO engine produces
  the expected diagnostics and the stub produces none — each grammar's rules run
  independently.
- **Grammar-tag round-trip:** assert each `ErasedLintResult.grammar_id` matches
  the boxed engine's `scheme().scheme_id()`.
- **Boxing-once:** the test holds each `Box<dyn ErasedEngine>` for the whole
  document and lints through the shared reference — structurally demonstrates one
  box per scheme per document.
- **Parity gates (every PR):** `audit_v3_0_parity.rs`, G13 canary, corpus
  accuracy (≥95% lint/fix), post-3b registration pin.
- **Perf:** interactive-latency bench unchanged (erasure is opt-in; the typed
  `Engine<S>` hot path is untouched). A small `multi_grammar` smoke bench
  captures dispatch overhead per T028b (informational, not a gate).
- **WASM:** `wasm-pack` / `cargo check --target wasm32` — erasure lives in
  `marque-engine` (not WASM-safe), so this confirms nothing leaked into the safe
  set.

---

## 7. Invariants preserved

1. CAPCO output byte-identical (audit-parity + G13 + corpus).
2. Boxing ≤ 1 per scheme per document (erasure at the `Engine<S>` → `dyn`
   boundary, never per-candidate).
3. WASM-safe set unchanged.
4. Typed `Engine<S>` hot path untouched — erasure is a separate façade, perf-neutral.
5. Constitution IV scheme-adoption rule does NOT apply (this is
   engine-generification, not scheme-adoption — one-line note in each PR body).
6. `marque-3.2` audit schema frozen.

---

## 8. Process

- rust-reviewer + code-reviewer dispatched **before** PR-open on both PRs.
- Commit / PR bodies carry **no** Claude / Co-Authored-By / 🤖 attribution.
- I open PRs; the user merges.
- Doc-accuracy sweep discipline (the recurring B3 lesson): when generifying or
  adding a façade, grep *all* sibling docs in each touched file for stale
  concrete-type / scheme-name references in the *same* pass — function-level docs
  included, prose comments included.

---

## 9. Decisions (SIGNED OFF 2026-06-01)

1. **Q1 — `ErasedEngine` method surface.** ✅ **`grammar_id` + `lint_erased` +
   `fix_erased`.** Erase the fix surface too (it exists and is real). `resolve` /
   `claims` deferred until their surfaces land; `CoherenceRegistry` to Phase E.
2. **Q2 — PR split.** ✅ **2 PRs** — B4.1 scheme-neutrality cleanup → B4.2 erasure
   + registry.
3. **Q3 — default-stripping scope.** ✅ **Strip `Engine` + `R` defaults only.**
   Keep the `= CapcoScheme` defaults on `LintResult` / `FixResult` / `EngineError`
   and the internal fix-pass types (ergonomic for the CAPCO common case; they
   don't hide an engine scheme assumption).
