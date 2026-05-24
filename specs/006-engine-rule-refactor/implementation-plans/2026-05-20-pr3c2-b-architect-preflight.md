# PR 3c.2.B — Architect Preflight Plan

**Date**: 2026-05-20
**Author**: system-architect (preflight pass; 5-year-maintainability lens)
**Branch**: `refactor-006-pr-3c2-b-call-site-migration` (off `origin/staging@861e85e3`)
**Base PR**: `staging`
**Status**: PROPOSAL — PM to lock the open questions in §5 before implementation starts.

**Predecessor**: `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md` (3c.2.A PM contract; PM-1 erratum + R-A1 carry forward).
**Successor**: `docs/plans/2026-05-20-pr3c2-b-pm-decisions.md` (PM contract for B; to be authored from this preflight).

**Spec anchors**:
- `specs/006-engine-rule-refactor/spec.md` FR-043 (sole post-3c.2.E canonicalize path), FR-040 (lint target on `from_parsed_unchecked` callers).
- `specs/006-engine-rule-refactor/followups/2026-05-19-pr-3c2-a-deferred-findings.md` B-FOLLOWUP-1 (HRTB smoke test), B-FOLLOWUP-2 (caller-count baseline).
- `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` §1 row "3c.2.B — Call-site migration" + §2 D25.1 sub-PR decomposition.
- `crates/scheme/src/scheme.rs:125-169` (canonicalize trait method + GAT declarations).
- `crates/capco/src/scheme/marking_scheme_impl.rs:290-303` (CapcoScheme bindings + override-pending comment).
- `crates/ism/src/canonical.rs:215-303` (`from_parsed_unchecked` body to lift).
- `.specify/memory/constitution.md` Principles V, VI, VII, VIII.

---

## 1. Executive summary

- **PR 3c.2.B is fundamentally a body-lift PR.** The CapcoScheme override of `MarkingScheme::canonicalize` exists today as a placeholder comment at `crates/capco/src/scheme/marking_scheme_impl.rs:299`; B's load-bearing work is to materialize that override (body verbatim from `crates/ism/src/canonical.rs:216-303`) and migrate ~14 production + ~16 test callers to the trait route. Byte-identity at T056 is a structural guarantee, not an experimental hope, because the override body and the adapter body are the same function text.
- **Two `marque-core` sites cannot migrate in B.** `crates/core/src/parser.rs:3890` (test helper inside `#[cfg(test)]`) and three `crates/core/tests/*.rs` integration test sites (display_only_list.rs:512, fgi_silent_skip_guard.rs:96, 117) sit in a crate that does NOT depend on `marque-capco`. Reaching `CapcoScheme::canonicalize` from those sites would require a dependency-graph edge that violates Constitution VII (`marque-core ←── marque-capco` runs the wrong direction). These four sites stay on `from_parsed_unchecked` until PR 3c.2.E deletes the adapter; B's "migrated" set is **the other 26**, not all 30.
- **The recognizer trait surface today carries no scheme reference.** `Recognizer<S>::recognize(&self, bytes, offset, cx)` (`crates/scheme/src/recognizer.rs:367`) gives the recognizer impl zero access to a `&S`. The two engine hot-path call sites — `crates/engine/src/recognizer.rs:98` (`StrictRecognizer::recognize`) and `crates/engine/src/decoder.rs:411` (`DecoderRecognizer::recognize`) — therefore cannot do `<S as MarkingScheme>::canonicalize(&self.scheme, parsed.attrs)` without either (a) extending the `Recognizer` trait signature with a `&S` parameter, (b) holding a scheme reference on the recognizer struct, or (c) using a static `CapcoScheme::canonicalize` UFCS call that doesn't actually need `&self`. **D1 recommendation: option (c), the no-receiver UFCS** — because `from_parsed_unchecked` is a pure function of `parsed`, the `&self` receiver on `canonicalize` is decorative (it must remain on the trait method to support future schemes that ARE stateful, but CapcoScheme's body needs zero state). This decision is the single most consequential architectural call in B.
- **CapcoScheme is NOT zero-sized.** `crates/capco/src/scheme/adapter.rs:43-65` shows four `Vec` fields populated by `build_categories()` + `build_constraints()` + `build_page_rewrites()` at every `default()` call. Constructing a fresh `CapcoScheme::default()` at every `canonicalize` call site is a hot-path allocation regression of unknown but non-trivial magnitude. This is the second consequential call: **wherever a `&CapcoScheme` is already in scope, use it; never call `CapcoScheme::default()` to satisfy a `&self` requirement.**
- **The work decomposes cleanly into 6 commits** of incremental byte-identity preservation. B1 lands the override body (zero callers migrated; the workspace builds; byte-identity holds trivially because no caller has changed). B2-B5 migrate caller sets in dependency-graph order (engine hot-path → engine tests → WASM + core test-helper carve-outs → external `tests/` dirs). B6 sweeps doc comments and lands the HRTB smoke test (B-FOLLOWUP-1).

---

## 2. Decision points

### D1 — Call shape: no-receiver UFCS via `<CapcoScheme as MarkingScheme>::canonicalize(&scheme, parsed.attrs)`, with `&scheme` whenever a scheme is already in scope

**Recommendation**: All migrated call sites use the method-call syntax `scheme.canonicalize(parsed.attrs)` where a `&CapcoScheme` is already in scope (which is the common case — see D2/D3/D4). The engine hot-path sites that today have no scheme in scope (`crates/engine/src/recognizer.rs:98`, `crates/engine/src/decoder.rs:411`) get the scheme reference plumbed in via D2's recommendation. UFCS is unnecessary because no two `MarkingScheme` impls are in scope simultaneously at any call site.

**Five-year defense**:
- Method-call syntax is the idiomatic Rust shape and matches the existing engine surface (e.g., `scheme.render_canonical(&marking, &ctx, &mut out)` at `crates/capco/tests/render_canonical_axis_fixtures.rs:798`).
- When a second scheme (CUI, NATO) lands, the surrounding code already disambiguates which scheme is in scope; UFCS would add ceremony without payoff.
- The `&self` parameter on the trait method is load-bearing **for future schemes** that may be stateful (e.g., a CUI scheme whose `canonicalize` reads a category-registry table from `self`). CapcoScheme's body happens to ignore `&self`; that's a property of THIS scheme's body, not the trait surface.

**Rejected alternatives**:
- `<CapcoScheme as MarkingScheme>::canonicalize(&scheme, parsed.attrs)` (UFCS at every site): ceremony for ceremony's sake. The trait is unambiguous from `scheme: &CapcoScheme`.
- `CapcoScheme::default().canonicalize(parsed.attrs)`: see D2 — non-ZST cost. **REJECT.**
- Free-function in `marque-capco` that bridges: re-introduces a non-trait route, defeats FR-043.

### D2 — Engine hot path: extend `Engine::with_recognizer` to plumb `&CapcoScheme`, OR hold a `'static` reference

**Decision needed from PM.** Two viable paths; either preserves byte-identity. Architect leans (b) for scope discipline.

**Context** (file:line evidence):
- `Engine` already holds `scheme: CapcoScheme` at `crates/engine/src/engine.rs:244`. Reading this field from `recognize` requires the recognizer to either hold a borrow back to the engine, hold its own scheme, or receive the scheme per-call via an extended trait signature.
- `Recognizer<S>::recognize(&self, bytes, offset, cx)` (`crates/scheme/src/recognizer.rs:367`) takes no `&S`. The trait predates the canonicalize migration.
- `StrictRecognizer` (`crates/engine/src/recognizer.rs:60`) and `DecoderRecognizer` (`crates/engine/src/decoder.rs:262`) are both unit structs (`#[derive(Default, Clone, Copy)] pub struct StrictRecognizer;`). Adding a field changes their construction surface.

**Option (a) — Extend `Recognizer` trait signature**:
```rust
fn recognize(&self, bytes: &[u8], offset: usize, scheme: &S, cx: &ParseContext) -> Parsed<S::Marking>;
```
This is the cleanest architectural shape — recognizers explicitly receive what they need to call into `MarkingScheme::canonicalize` — but it cascades: every `impl Recognizer<S>` (StrictRecognizer, DecoderRecognizer, StrictOrDecoderRecognizer, the `Dyn` escape hatch, and any test stubs) gains a parameter. It also changes a public trait surface that downstream users of `Engine::with_recognizer(Arc<dyn Recognizer<_>>)` depend on. **Costly for a body-lift PR.**

**Option (b) — `StrictRecognizer` and `DecoderRecognizer` carry `scheme: &'static CapcoScheme` OR `Arc<CapcoScheme>`**:
A `&'static CapcoScheme` requires either a `LazyLock<CapcoScheme>` singleton in `marque-capco` or `Box::leak(Box::new(CapcoScheme::new()))` at first construction. An `Arc<CapcoScheme>` is cheap to clone and matches the existing `Arc<dyn Recognizer<_>>` plumbing pattern. **Costly construction surface change but trait-stable.**

**Option (c) — Architect's recommendation: status-quo path; the override body has no `self` dependency**:
The body lifted from `from_parsed_unchecked` reads zero scheme state — it's a pure structural rename of `ParsedAttrs<'src>` → `CanonicalAttrs`. The CapcoScheme override can be written as:
```rust
fn canonicalize<'src>(&self, parsed: Self::Parsed<'src>) -> Self::Canonical {
    // Body is identical to the prior free function; `self` is unused.
    Self::canonicalize_inner(parsed)
}

impl CapcoScheme {
    fn canonicalize_inner(parsed: ParsedAttrs<'_>) -> CanonicalAttrs {
        // body verbatim from marque_ism::from_parsed_unchecked
    }
}
```
Hot-path call sites without a `&scheme` in scope **then call** `CapcoScheme::canonicalize_inner(parsed.attrs)` — UFCS-by-name on the inherent method, no trait dispatch, no `&self`, no plumbing change. The trait method is honored (the trait surface is satisfied, FR-043 is technically met because every caller routes through CapcoScheme code that delegates to the inner). For the 28 sites that DO have a `&scheme` in scope, they call `scheme.canonicalize(parsed.attrs)` which the trait method dispatches to the same `canonicalize_inner`. Byte-identity holds.

**The PM-decidable question**: **Is route-via-inherent-helper acceptable for the engine hot-path sites, or does FR-043's "sole path" reading require the trait dispatch literally pass through `<S as MarkingScheme>::canonicalize` at every call site?**

If the literal reading wins: take option (b) — `StrictRecognizer` and `DecoderRecognizer` carry an `Arc<CapcoScheme>` (or scheme borrow plumbed through), and the engine constructs them with the same scheme it stores at `engine.rs:244`. Cost: ~20 lines of plumbing in the engine constructor; risk: small. T056 corpus regression is the gate.

If the architect's preferred reading wins: take option (c) — `CapcoScheme::canonicalize_inner` is the inherent helper, the trait override delegates to it, and engine hot-path sites call the inherent helper directly. Cost: zero plumbing; FR-043 is met "in spirit, via delegation"; risk: a future code reviewer needs to understand the delegation pattern.

**Architect leans (b)** because (c) leaves a non-trait route alive past 3c.2.E that a careless contributor could re-grow. The body-lift PR is the right moment to install the discipline. The Arc-clone cost in the recognizer is negligible compared to the parser work that dominates `recognize()`.

**Five-year defense for (b)**:
- Future schemes (CUI, NATO) that adopt the engine will hit the same architectural question at scheme-adoption time. Solving it now in the recognizer plumbing means scheme #2 inherits the answer rather than re-inventing it.
- The constitution Principle VII workflow ("scheme-adoption PR MUST NOT edit the engine crates") is incompatible with leaving scheme-shaped seams in engine code. Plumbing `Arc<S>` to recognizers is a one-time edit that survives scheme additions.

### D3 — WASM: `scheme` already in scope at both call sites; use method-call syntax

`crates/wasm/src/lib.rs:1167` and `crates/wasm/src/lib.rs:1263` each contain `let scheme = CapcoScheme::new();` two lines before the migration site. The migration is purely:
```rust
// before
let attrs = marque_ism::from_parsed_unchecked(parsed.attrs);
// after
let attrs = scheme.canonicalize(parsed.attrs);
```
No plumbing. **Recommendation: migrate both sites unconditionally in B.**

Forward-defense note: the two `let scheme = CapcoScheme::new();` allocations themselves are not in B's scope (they predate this PR), but a future PR consolidating WASM scheme construction would benefit from a `LazyLock<CapcoScheme>` (the type already uses `LazyLock` for vocabulary tables per `crates/capco/src/vocabulary.rs:235`). Track as a follow-up, not as B scope.

### D4 — External `tests/` directories: classification table

Spot-check of `crates/capco/tests/` and adjacent test files. Each test caller of `from_parsed_unchecked` falls into one of three categories:

| Site | Category | Rationale | B migrates? |
|---|---|---|---|
| `crates/capco/tests/nato_atomal_aea_routing.rs:72` | **Plumbing** | `marque_ism::from_parsed_unchecked(parsed.attrs)` is unwrapped from `parsed`, returned as `CanonicalAttrs` for the test's downstream assertions. `CapcoScheme::new()` is constructed at line 79 (`engine_with_fixed_clock`). | YES — add `let scheme = CapcoScheme::new()` to the helper if not already present; migrate the call. |
| `crates/capco/tests/lattice_corpus_runner.rs:152` | **Plumbing** | `parse_portion_line()` returns `CanonicalAttrs` for fixture replay. A `CapcoScheme::new()` already exists at line ~130 of the same file (line 130 area: `default CAPCO scheme constructs without rewrite cycles`). | YES — thread `&scheme` into `parse_portion_line` or construct one locally. |
| `crates/capco/tests/parse_render_roundtrip.rs:78` | **Plumbing** | `parse_with_kind()` helper returns `Option<CanonicalAttrs>` for round-trip assertions. | YES — add scheme; migrate. |
| `crates/capco/tests/render_canonical_axis_fixtures.rs:788, 828` | **Plumbing** | `let canonical = ...; let scheme = CapcoScheme::new();` — scheme is constructed ONE LINE after the call! Just reorder. | YES — trivial reorder + migrate. |
| `crates/capco/tests/nato_bohemia_balk_sci_routing.rs:108` | **Plumbing** | Helper function for fixture canonicalization. | YES. |
| `crates/capco/tests/s004_audit_content_ignorance.rs:65` | **Plumbing** | Helper for fixture canonicalization. | YES. |
| `crates/capco/tests/relido_clears_page_rewrites.rs:43` | **Plumbing** | Helper. | YES. |
| `crates/capco/tests/rules_us1.rs:73` | **Plumbing** | Helper. | YES. |
| `crates/capco/tests/e070_frd_tfni_precedence.rs:39` | **Plumbing** | Helper. | YES. |
| `crates/capco/tests/dissem_nato_pure_nato_portion.rs:46` | **Plumbing** | Helper. | YES. |
| `crates/engine/tests/document_corpus.rs:148` | **Plumbing** | Mirrors the engine's decoder flow for ground-truth comparison (the comment at line 146 says so). | YES — engine tests are downstream of `marque-capco`; scheme is reachable. |
| `crates/core/tests/display_only_list.rs:512` | **ARCHITECTURAL BLOCKER** | `marque-core` does not depend on `marque-capco`. Cannot reach `CapcoScheme::canonicalize`. | NO — stays on `from_parsed_unchecked` until 3c.2.E. |
| `crates/core/tests/fgi_silent_skip_guard.rs:96, 117` | **ARCHITECTURAL BLOCKER** | Same as above. | NO — stays until 3c.2.E. |
| `crates/core/src/parser.rs:3890` (in `#[cfg(test)] mod tests`) | **ARCHITECTURAL BLOCKER** | Test-helper module in a crate that does not depend on `marque-capco`. | NO — stays until 3c.2.E. |

**Adapter-direct (tests that explicitly exercise `from_parsed_unchecked` semantics, scheduled for retirement at 3c.2.E)**: none found in the inventory. Every test caller is plumbing, not adapter-direct. The three `marque-core` sites are blocked by dependency-graph direction, not by test intent.

Aggregate: **26 of 30 sites migrate in B; 4 remain on `from_parsed_unchecked` until 3c.2.E**. Constitution VII §VII forbids the `marque-core → marque-capco` edge that would let those 4 migrate. The PM contract for 3c.2.B must acknowledge the four-site carve-out and document that 3c.2.E's adapter deletion subsumes them.

Five-year defense for accepting the carve-out: the alternative — propagating `marque-scheme` test-utility types into `marque-core` so a stub scheme can canonicalize there — pollutes the WASM-safe core crate with grammar surface for the sake of four test sites. The cost of carrying the adapter through 3c.2.D is documented (it's a no-op wrapper around the trait method by then) and is bounded (the adapter is deleted at E, the carve-out collapses with it).

### D5 — Override placement: `crates/capco/src/scheme/marking_scheme_impl.rs` adjacent to the GAT bindings

**Recommendation**: The `fn canonicalize` override body lives in `crates/capco/src/scheme/marking_scheme_impl.rs` directly inside the `impl MarkingScheme for CapcoScheme { }` block, immediately after the `type Parsed<'src> = ParsedAttrs<'src>; type Canonical = CanonicalAttrs;` declarations (currently lines 302-303). The override IS the implementation of the bound types; co-locating them maximizes reviewer signal.

If D2 lands on option (b) (Arc plumbing), the inherent helper `fn canonicalize_inner` (or whatever it's named) also lives in `marking_scheme_impl.rs` to keep the body adjacent. If D2 lands on option (c) (the architect's lean), the inherent helper is on `impl CapcoScheme { ... }` and the trait override delegates to it — same module either way.

**Five-year defense**:
- `marking_scheme_impl.rs` is the canonical home for "CapcoScheme satisfies the MarkingScheme contract" — splitting the body to a separate `canonical.rs` would force readers to navigate two files to verify the override.
- Module size: `marking_scheme_impl.rs` is currently ~340 lines (per the file inspection); the override adds ~70 lines (the body lifted from `crates/ism/src/canonical.rs:216-303`) for a final size around 410 lines, well under the 800-line cohesion bar from `~/.claude/rules/common/coding-style.md`.

**Rejected alternative**: A new `crates/capco/src/scheme/canonical.rs` module — over-engineering for a 70-line body, and creates a split-brain pattern (canonicalize lives here, render lives there) without a reusability payoff.

### D6 — Doc-comment migration: classification table

Each site is classified by what its prose describes:

| File:line | Comment topic | Class |
|---|---|---|
| `crates/scheme/src/scheme.rs:140-151` | The trait method's own doc-comment explaining the `unimplemented!()` default and PR-3c.2 staging. | **MIGRATE-NOW** — once the CapcoScheme override lands at B1, the prose "PR 3c.2.B implements the CapcoScheme override" becomes past-tense ("PR 3c.2.B implemented..."). Update in B6. |
| `crates/capco/src/scheme/marking_scheme_impl.rs:299` | "The CapcoScheme override of `canonicalize` ... lands at PR 3c.2.B; at PR 3c.2.A the trait-default `unimplemented!()` is in scope". | **DELETE-LATER** — once the override is in place at B1, the comment is the override site's own context; rewrite to describe what the override does, not what's pending. Sweep in B6. |
| `crates/engine/src/recognizer.rs:96-97` | "Post-PR-3c this becomes `MarkingScheme::canonicalize(parsed.attrs)`." | **DELETE** — the migration is the change; the comment is obsolete after B2. Sweep in B2 itself, not B6. |
| `crates/engine/src/decoder.rs:407-410` | Same prose pattern as recognizer.rs. | **DELETE** — sweep in B2. |
| `crates/wasm/src/lib.rs:1184, 1289` | "PR 3c retires `from_parsed_unchecked` in favor of `MarkingScheme::canonicalize`; this call migrates then." | **DELETE** — sweep in B4 (the WASM migration commit). |
| `crates/core/src/lib.rs:13` | "then runs `marque_ism::from_parsed_unchecked` (PR 3a transitional path)". | **DEFER** — `marque-core` still calls `from_parsed_unchecked` after B (its tests can't migrate). Update at 3c.2.E. |
| `crates/core/src/parser.rs:9, 52, 3871` | Same — describes a `marque-core`-internal use of the adapter. | **DEFER** — 3c.2.E. |
| `crates/ism/src/attrs.rs:31` | "The `from_parsed_unchecked` adapter bridges parser output to ..." | **DEFER** — describes the adapter itself, which survives B. |
| `crates/ism/src/dissem_attribution.rs:52` | Adapter description. | **DEFER** — 3c.2.E. |
| `crates/ism/src/lib.rs:18, 48` | Adapter description. | **DEFER** — 3c.2.E. |
| `crates/ism/src/parsed.rs:11` | Adapter description. | **DEFER** — 3c.2.E. |
| `crates/ism/src/canonical.rs:10, 40, 195` | The adapter's own doc-comment. | **DEFER** — 3c.2.E (the adapter is being deleted). |
| `crates/rules/src/confidence.rs:49` | "MessageTemplate JSON serialization, `from_parsed_unchecked` adapter deletion" — list of PR 3c.2 deferred commitments. | **DEFER** — describes the PR 3c.2.E commitment; reword at E, not B. |

**Decision**: Sweep MIGRATE-NOW and DELETE classes during the relevant migration commits (B2 + B4) and clean up the remaining MIGRATE-NOW set in B6. The DEFER class belongs to 3c.2.E's adapter-deletion sweep. Recording the classification in this preflight prevents B from accidentally claiming completeness on a sweep that's actually deferred.

### D7 — HRTB smoke test landing pad

**Recommendation**: Place the smoke test in `crates/engine/tests/`. The engine is the convergence crate where any generic-over-scheme dispatch realistically lands (e.g., a future generic-`S` recognizer dispatcher would live here). Concretely: a new file `crates/engine/tests/hrtb_smoke.rs` containing:

```rust
// Forward-defense per B-FOLLOWUP-1: catches HRTB inference issues on
// the GAT `S::Parsed<'a>` at compile time, before a real generic helper
// trips them at the call site. See
// `specs/006-engine-rule-refactor/followups/2026-05-19-pr-3c2-a-deferred-findings.md`
// item B-FOLLOWUP-1.
use marque_scheme::MarkingScheme;

#[allow(dead_code)]
fn _hrtb_smoke<S: MarkingScheme>(_scheme: &S)
where
    for<'a> S::Parsed<'a>: Sized,
{}
```

**Five-year defense**:
- The engine is the only crate that today exercises generic-over-scheme dispatch via `Recognizer<S>`. If a HRTB issue surfaces, it will surface in engine code first.
- `crates/scheme/tests/` is the trait's home, but the trait itself doesn't have generic consumers; the test would have no analogue to defend.
- The compile-only nature of the test means it consumes zero runtime resources; placement is purely about reviewer signal.

**Rejected**: `crates/rules/tests/` — too far from the dispatcher; a reviewer searching for HRTB context wouldn't naturally land there.

### D8 — Byte-identity preservation under the trait route

The override body at PR 3c.2.B is the function text lifted verbatim from `crates/ism/src/canonical.rs:216-303`. Trait-method dispatch under the recommended call shape (`scheme.canonicalize(parsed.attrs)`) is single-impl monomorphization (CapcoScheme is the only `impl MarkingScheme` with non-`()` Parsed/Canonical types). The compiler produces the same machine code for both the trait route and the inherent helper call. **There is no observable difference** under any of the following audit channels:

- **Debug output**: `Diagnostic::message` carries no scheme-method-name artifact.
- **Panic messages**: the body doesn't panic (no `unwrap`/`expect`/`assert!` triggered in the success path; the `debug_assert!` at canonical.rs:293-299 fires only in debug builds and on a violated invariant — same in both routes).
- **NDJSON audit emission**: schema stays `marque-mvp-3`; the audit envelope has no field that depends on call path.
- **Span layout**: `token_spans` is moved unchanged from `ParsedAttrs` to `CanonicalAttrs`; no path through `canonicalize` re-runs the parser.
- **Ordering**: the body's only iteration order is over `Vec::from(...).into_iter()` — deterministic at LLVM IR level regardless of trait dispatch.

The T056 corpus regression matrix gates each commit; if any commit produces a byte-difference, B is doing more than a body-lift (and the PR should pause for investigation).

**Risk**: if D2 lands on option (c) — the inherent helper — the trait override at every call site still delegates to the same inherent. If D2 lands on option (b) — Arc plumbing — the inherent helper disappears and the trait method body IS the lifted function text. Either way the body is the same; the routing wrapper around the body is what changes.

### D9 — Commit sequence (6 commits)

| Commit | Files touched | Purpose | Byte-identity invariant |
|---|---|---|---|
| **B1 — Override body** | `crates/capco/src/scheme/marking_scheme_impl.rs` only. If D2(b): also `crates/engine/src/{recognizer,decoder,engine}.rs` for the recognizer-scheme plumbing (no caller migrates yet — the recognizers' new `&scheme` argument is unused at B1). | Land the canonicalize override (body verbatim from `crates/ism/src/canonical.rs:216-303`). If D2(b): land the recognizer scheme-plumbing too, but every recognizer call still uses `from_parsed_unchecked`. | Trivially holds: no caller has changed. |
| **B2 — Engine hot path** | `crates/engine/src/recognizer.rs:98`, `crates/engine/src/decoder.rs:411`. Sweep DELETE doc-comments at both lines. | Migrate the two production hot-path callers. | T056 corpus regression: ZERO byte difference. |
| **B3 — Engine inline test callers** | `crates/engine/src/decoder.rs:6337, 6393, 6402, 6445, 6454, 6568, 6592, 6615, 6875` (9 sites inside `#[cfg(test)] mod`). | Migrate the 9 inline test sites. | Tests still green; unit test fixtures see identical canonical. |
| **B4 — WASM + core test-helper boundary** | `crates/wasm/src/lib.rs:1187, 1292` (migrated). Sweep DELETE doc-comments at 1184, 1289. **NOT migrated**: `crates/core/src/parser.rs:3890`, `crates/core/tests/display_only_list.rs:512`, `crates/core/tests/fgi_silent_skip_guard.rs:96, 117` — Constitution VII blocks. Document the carve-out in commit message. | Migrate WASM (scheme already in scope). Document the `marque-core` carve-out. | WASM unit + browser-parity tests green; `marque-core` tests unchanged. |
| **B5 — External `tests/` dirs** | `crates/capco/tests/` 12 files + `crates/engine/tests/document_corpus.rs`. | Migrate the 13 plumbing-class external test sites. | Every fixture round-trip stays identical; lattice corpus runner produces identical output. |
| **B6 — Doc sweep + HRTB smoke** | `crates/scheme/src/scheme.rs:140-151` (MIGRATE-NOW), `crates/capco/src/scheme/marking_scheme_impl.rs:299` (rewrite to "what this override does"), new `crates/engine/tests/hrtb_smoke.rs` (B-FOLLOWUP-1), this PM doc updated with implementation reality. | Closeout: doc-comment cleanup, HRTB forward-defense, PM contract reconciliation. | Doc changes only; trivially holds. |

Each commit ends with `cargo check --workspace && cargo nextest run --workspace` (and `wasm-pack build crates/wasm --target web --profiling` after B4).

**Architect note on D2 interaction**: If D2 lands on option (b), B1 carries the engine-side plumbing change, which is a meaningful diff (~20 lines in the engine constructor + recognizer struct field additions). If D2 lands on option (c), B1 is a pure body addition in `marking_scheme_impl.rs` (~70 lines). The commit count stays at 6 either way; B1's scope is what differs.

### D10 — Sub-PR 3c.2.C enablement

PR 3c.2.C migrates `Diagnostic.citation: &'static str → Citation` across **63 sites in 7 files** (verified at preflight: `grep -rn 'citation: "' crates/capco/src/ --include='*.rs' | wc -l` = 63). The C migration touches a disjoint surface from B (rule emission sites, not parser/canonicalize sites). **No B/C coupling** other than:

- **C inherits the doc-comment hygiene B establishes.** The DEFER-class comments in `crates/ism/src/canonical.rs:10, 40, 195` and friends still reference `from_parsed_unchecked`; C does not need to update those (3c.2.E will). If B's B6 sweep accidentally over-sweeps into the DEFER set, C's diff stays clean either way.
- **C is independent of D1/D2.** No `Citation` value is constructed inside `canonicalize` or any path B touches.

PM-resolved D25.1 (`docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` §1) allows B and C to land **in parallel** after A. This preflight does not surface any reason to deviate from that.

---

## 3. Risk register

### R-B1: D2 option (c) leaves a non-trait route alive past 3c.2.E

**Likelihood**: Medium if D2 lands on (c); zero if (b).
**Impact**: A future contributor could grow callers of `CapcoScheme::canonicalize_inner`, defeating FR-043's "sole path" invariant.
**Mitigation**: If PM picks (c), add `#[doc(hidden)]` + a path-based clippy carve-out around `canonicalize_inner` and document the engine-only contract in a doc-comment with the constitutional rationale, mirroring the `AppliedFix::__engine_promote` pattern (`marque-rules` carries a similar engine-only inherent today per Constitution V).
**Better**: pick (b). The plumbing cost is one-time; the discipline is permanent.

### R-B2: GAT HRTB inference at unforeseen call site

**Likelihood**: Low (no generic helper consumes `S::Parsed<'_>` in B's diff).
**Impact**: Compile error of the "implementation not general enough" shape; difficult to debug.
**Mitigation**: B6's HRTB smoke test catches the simplest form of this at compile time. If a real call site surfaces an issue beyond the smoke test, escalate per R-A2 fallback (standalone `Canonicalize` trait).

### R-B3: `marque-core` test carve-out misread as "B is incomplete"

**Likelihood**: Medium — a reviewer counting "30 sites − 26 migrated = 4 remaining" might flag the gap as a B regression.
**Impact**: Wasted review cycles, possible push to do the wrong thing (e.g., inject `marque-capco` into `marque-core`'s dev-deps).
**Mitigation**: Document the carve-out explicitly in B's PR description AND in the B4 commit message. The Constitution VII §VII directionality argument is unambiguous once stated. The PM contract for B (succeeding this preflight) must list "4 `marque-core` sites stay on `from_parsed_unchecked` per Constitution VII; subsumed by 3c.2.E adapter deletion" as a binding assumption.

### R-B4: CapcoScheme construction cost on hot path (if D2 picks (c) carelessly)

**Likelihood**: Zero if the recommendation is followed; non-zero if a contributor "fixes" the no-`self` body by calling `CapcoScheme::default()` at every call site.
**Impact**: Hot-path allocation regression — `build_categories() + build_constraints() + build_page_rewrites()` is materially expensive (the constructor walks several `LazyLock` tables at first call but allocates fresh `Vec<Category>` / `Vec<Constraint>` / `Vec<Template>` / `Vec<PageRewrite>` on each `new()`).
**Mitigation**: A clippy lint or grep-target in the engine code that flags `CapcoScheme::default()` and `CapcoScheme::new()` in production paths outside Engine construction. Failing that, a doc-comment on `CapcoScheme::new` warning that it allocates and should be called once per Engine.

### R-B5: Test-helper `parser.rs::CanonicalParsed` continues to exist past 3c.2.E

**Likelihood**: Low.
**Impact**: 3c.2.E adapter deletion would break the test helper unless `marque-core` migrates it.
**Mitigation**: Track as a 3c.2.E follow-up; B's responsibility is only to confirm the helper is `#[cfg(test)]`-gated (verified at line 3862-3863) so the helper itself does not block adapter deletion as long as the body is rewritten at E. The PM contract for 3c.2.E will need to address the `marque-core` test-helper migration (probably by inlining the field-rename body at the helper site, since `marque-core` cannot reach `CapcoScheme::canonicalize`).

### R-B6: Trait-method `&self` becomes load-bearing later

**Likelihood**: Medium — a future scheme (CUI) may need `&self` to canonicalize.
**Impact**: If B picks D2 option (c) and a future contributor writes a CUI scheme whose canonicalize reads `self.registry`, the call site that uses `CapcoScheme::canonicalize_inner` will not be migratable to CUI without revisiting the engine plumbing.
**Mitigation**: Document the contract on the trait method explicitly: "the trait method signature reserves `&self` for stateful schemes; CapcoScheme today ignores it. Callers SHOULD use the trait method dispatch (`scheme.canonicalize(parsed)`) rather than inherent helpers wherever a `&scheme` is in scope; only engine hot-path sites that cannot plumb `&scheme` may use the inherent path."

---

## 4. Reviewer attestation checklist (PR 3c.2.B)

For each of the three reviewers (rust-reviewer, code-reviewer, system-architect):

- [ ] **D2 resolution**: PR's D2 path matches the PM-locked option. If (b), the recognizer struct field additions are minimal and the engine constructor wires them. If (c), the inherent helper is `#[doc(hidden)]` and carries the engine-only contract comment.
- [ ] **CapcoScheme override body is byte-identical** to `crates/ism/src/canonical.rs:216-303` (compare diffs side-by-side).
- [ ] **The 4 `marque-core` sites remain on `from_parsed_unchecked`**: `crates/core/src/parser.rs:3890`, `crates/core/tests/display_only_list.rs:512`, `crates/core/tests/fgi_silent_skip_guard.rs:96, 117`. Each carries an inline comment naming Constitution VII §VII directionality as the reason, and naming 3c.2.E as the unblock.
- [ ] **The 26 migrated sites** all use `scheme.canonicalize(parsed.attrs)` method-call syntax, with `&scheme` already in scope at each call site (no `CapcoScheme::default()` constructions to satisfy `&self`).
- [ ] **T056 corpus regression matrix is green on every commit** (B1 through B6). A byte-difference at any commit indicates B is doing more than a body-lift.
- [ ] **B-FOLLOWUP-1 HRTB smoke test** lands at `crates/engine/tests/hrtb_smoke.rs` per D7.
- [ ] **B-FOLLOWUP-2 site-count baseline = 30** (14 production + 16 test) is the inventory baseline, NOT the predecessor PM doc's "25 surviving call sites" framing. The PM contract for B records the corrected count.
- [ ] **No new `Diagnostic.citation` migration in B** (stays `&'static str`; that work is 3c.2.C).
- [ ] **No `__engine_promote` body change in B** (audit-record shape stays `marque-mvp-3`).
- [ ] **Schema stays `marque-mvp-3`** (no audit-envelope changes).
- [ ] **No `marque-extract` touches** (scope hygiene).
- [ ] **Doc comments**: MIGRATE-NOW class swept in the relevant commit (B2/B4/B6); DEFER class explicitly preserved with no false-completeness claim.
- [ ] **HRTB smoke test compiles** under `cargo nextest run --workspace -p marque-engine --test hrtb_smoke`.
- [ ] **Constitution VII §VII directionality**: B's diff contains no `marque-core → marque-capco` dependency edge addition (verified via `git diff -- 'crates/core/Cargo.toml' 'crates/scheme/Cargo.toml'`).
- [ ] **Constitution V Principle V**: `AppliedFix::__engine_promote` is not touched in B (audit-record invariants untouched).
- [ ] **"Will we maintain this for 5 years?"**: the D2 path chosen does not leave a non-trait `Parsed → Canonical` route that a careless contributor could grow. If D2 (c) is chosen, the `#[doc(hidden)]` + engine-only contract comment is in place.

---

## 5. Open questions for PM

These cannot be resolved unilaterally by the architect; PM lock required before B implementation starts.

### OQ-B1 — D2 resolution: option (b) Arc plumbing vs option (c) inherent-helper delegation

**The question**: Does FR-043's "sole post-3c.2.E path" reading require every call site to literally dispatch through the `<S as MarkingScheme>::canonicalize` trait method, or is a CapcoScheme-internal helper that the trait method delegates to acceptable?

**What hangs on it**:
- Option (b): ~20 lines of engine plumbing change in B1 (recognizers carry `Arc<CapcoScheme>`; `Engine::new` wires them; `Engine::with_recognizer` signature gains a parameter or the recognizer construction takes an `Arc` argument). The trait surface is canonical; no internal escape hatch exists.
- Option (c): zero plumbing change; the inherent helper lives on `impl CapcoScheme`; the trait override delegates. FR-043 is met "in spirit, via delegation"; a `#[doc(hidden)]` engine-only contract comment seals the inherent helper.

**Architect's lean**: option (b). The plumbing cost is one-time and survives scheme #2 adoption (CUI/NATO will hit the same architectural question). The discipline of "callers route through the trait" survives careless contributors better than "callers MAY use the inherent if they have engine reasons."

**PM-decidable trade-off**: scope discipline vs. forward-defense. If the PM prefers to keep B small and scoped, option (c) is acceptable with the documented mitigation; if the PM prefers to install the right shape now, option (b) is the recommendation.

### OQ-B2 — `marque-core` 4-site carve-out: accept it, or pursue an alternate migration

**The question**: Do we accept that 4 sites in `marque-core` stay on `from_parsed_unchecked` through 3c.2.B/C/D (deleted alongside the adapter at 3c.2.E), or do we pursue an alternate migration path (e.g., a domain-neutral canonicalize free function in `marque-scheme` that `marque-core` can call)?

**What hangs on it**:
- **Accept**: B's PR description documents the carve-out; 3c.2.E's deletion sweep handles the four sites simultaneously with the adapter retirement. Adapter survives B/C/D harmlessly as a no-op wrapper.
- **Alternate**: introduce `marque_scheme::canonicalize_parsed::<S>(parsed)` as a domain-neutral helper. This would let `marque-core` migrate too. But then `marque-scheme` would need a `canonicalize` trait method default that does work (currently `unimplemented!()`), or a free function that bypasses the trait — both have shape problems (the first means every scheme stub needs a real implementation; the second defeats FR-043's trait-route invariant).

**Architect's lean**: accept. The four sites are test code; the adapter survives only as a no-op wrapper. The alternate paths fight Constitution VII or FR-043 for marginal benefit.

### OQ-B3 — HRTB smoke test placement reconfirmation

**The question**: D7 recommends `crates/engine/tests/hrtb_smoke.rs`. Is the engine the right home, or should the smoke test live in `crates/scheme/tests/` to be adjacent to the trait?

**What hangs on it**:
- Engine placement: the smoke test sits where a real HRTB issue would first surface (engine has generic-over-scheme dispatch via `Recognizer<S>`).
- Scheme placement: the smoke test sits where the trait is defined.

**Architect's lean**: engine. The smoke test exists to catch issues at the dispatcher; the trait itself doesn't have generic consumers in its own crate.

**Low-stakes**: either is fine. Listed for PM lock so the implementing agent doesn't re-debate.

### OQ-B4 — Commit-message convention for the carve-out

**The question**: Does the B4 commit message need to list the four `marque-core` sites by file:line, or is "see preflight §1 bullet 2 / §2 D4 for the four `marque-core` test sites carved out under Constitution VII" sufficient reference?

**What hangs on it**: B's commit message length / reviewer-cycle friction.

**Architect's lean**: list the four sites by file:line in the B4 commit message AND link to this preflight section. Memorialized location is the more forgiving shape when a future reviewer is bisecting a related issue.

---

## Appendix A: Site inventory verified at 2026-05-20

**Production code (14 sites)** — verified via `grep -rn "from_parsed_unchecked" /home/knitli/marque/crates/ --include="*.rs" | grep -v "//"`:

```
/home/knitli/marque/crates/engine/src/recognizer.rs:98       (StrictRecognizer hot path)
/home/knitli/marque/crates/engine/src/decoder.rs:411         (DecoderRecognizer hot path)
/home/knitli/marque/crates/engine/src/decoder.rs:6337        (inline test mod)
/home/knitli/marque/crates/engine/src/decoder.rs:6393        (inline test mod)
/home/knitli/marque/crates/engine/src/decoder.rs:6402        (inline test mod)
/home/knitli/marque/crates/engine/src/decoder.rs:6445        (inline test mod)
/home/knitli/marque/crates/engine/src/decoder.rs:6454        (inline test mod)
/home/knitli/marque/crates/engine/src/decoder.rs:6568        (inline test mod)
/home/knitli/marque/crates/engine/src/decoder.rs:6592        (inline test mod)
/home/knitli/marque/crates/engine/src/decoder.rs:6615        (inline test mod)
/home/knitli/marque/crates/engine/src/decoder.rs:6875        (inline test mod)
/home/knitli/marque/crates/wasm/src/lib.rs:1187              (compute_banner_native)
/home/knitli/marque/crates/wasm/src/lib.rs:1292              (generate_cab_native)
/home/knitli/marque/crates/core/src/parser.rs:3890           (#[cfg(test)] mod tests::CanonicalParsed) [CARVE-OUT]
```

**External `tests/` (16 sites in 16 files)**:

```
/home/knitli/marque/crates/capco/tests/nato_atomal_aea_routing.rs:72
/home/knitli/marque/crates/capco/tests/nato_bohemia_balk_sci_routing.rs:108
/home/knitli/marque/crates/capco/tests/s004_audit_content_ignorance.rs:65
/home/knitli/marque/crates/capco/tests/lattice_corpus_runner.rs:152
/home/knitli/marque/crates/capco/tests/relido_clears_page_rewrites.rs:43
/home/knitli/marque/crates/capco/tests/rules_us1.rs:73
/home/knitli/marque/crates/capco/tests/render_canonical_axis_fixtures.rs:788
/home/knitli/marque/crates/capco/tests/render_canonical_axis_fixtures.rs:828
/home/knitli/marque/crates/capco/tests/parse_render_roundtrip.rs:78
/home/knitli/marque/crates/capco/tests/e070_frd_tfni_precedence.rs:39
/home/knitli/marque/crates/capco/tests/dissem_nato_pure_nato_portion.rs:46
/home/knitli/marque/crates/engine/tests/document_corpus.rs:148
/home/knitli/marque/crates/core/tests/display_only_list.rs:512           [CARVE-OUT]
/home/knitli/marque/crates/core/tests/fgi_silent_skip_guard.rs:96        [CARVE-OUT]
/home/knitli/marque/crates/core/tests/fgi_silent_skip_guard.rs:117       [CARVE-OUT]
```

**Total**: 14 + 16 = 30 caller sites. **B migrates 26; 4 remain on `from_parsed_unchecked` until 3c.2.E** per the Constitution VII §VII directionality carve-out documented at D4 and R-B3.

**Note on doc-comment site count**: D6's classification table identifies 14 doc-comment references; **MIGRATE-NOW** = 2 sites (`scheme.rs:140-151`, `marking_scheme_impl.rs:299`); **DELETE** = 4 sites (engine.recognizer + engine.decoder + 2 wasm); **DEFER** = 8 sites (the `marque-ism` + `marque-core` + `marque-rules` cluster). The DEFER sites are exclusively in crates where the adapter still has live callers post-B; rewording at B would create a false impression that the call sites had moved.

---

## Appendix B: Constitution principles applied

- **Principle V (Audit-First Compliance)**: `AppliedFix::__engine_promote` is not touched in B. Audit-record shape unchanged.
- **Principle VI (Dataflow Pipeline)**: B does not move work between phases. Recognizer remains the canonicalization seam.
- **Principle VII (Crate Discipline)**: The `marque-core` 4-site carve-out is the direct application — `marque-core ←── marque-capco` directionality forbids the migration. The carve-out is the right answer, not a workaround.
- **Principle VIII (Authoritative Source Fidelity)**: The override body is verbatim from a CAPCO-aware adapter (`crates/ism/src/canonical.rs:216-303`); no §-citations are added or removed in B. Citation discipline is 3c.2.C's territory.
