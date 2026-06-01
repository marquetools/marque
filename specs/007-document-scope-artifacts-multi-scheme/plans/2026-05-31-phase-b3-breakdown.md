# Phase B PR-B3 — Engine generification long pole (breakdown for sign-off)

**Status:** DRAFT for sign-off · **Date:** 2026-05-31 · base `main` `c8d770d4` (post-B2)
**Parent plan:** `2026-05-31-phase-b-engine-generification.md`

> Writing-only plan. No code, no agents launched until you approve the shape.

---

## 1. Where B2 left it

B2 (#836, merged) made the **struct** generic: `Engine<S: MarkingScheme = CapcoScheme>`
with `rule_sets: Vec<Box<dyn RuleSet<S>>>` and `scheme: S`. But **every `impl` is
`impl Engine<CapcoScheme>`**, the recognizer is `CapcoScheme`-bound, and the constructor
still `drop(scheme)` + `CapcoScheme::new()`. So the engine is generic in *shape* only.

B3 is the swing that makes `Engine<S>` actually work for an arbitrary `S`, and closes the
scheme-discard.

## 2. Real coupling inventory (measured on `c8d770d4`)

| Symbol (concrete CAPCO type) | → generic target | engine/src refs | files |
|------------------------------|------------------|-----------------|-------|
| `CanonicalAttrs` | `S::Canonical` | 116 | 20 |
| `CapcoMarking` | `S::Marking` | 88 | 24 |
| `CapcoScheme` | `S` | 287 | 40 |
| decoder dir coupled | — | — | 18 of 35 files |

Assoc-type bindings (so the mapping is unambiguous):
`CapcoScheme::Canonical = CanonicalAttrs`, `Marking = CapcoMarking`,
`Parsed<'src> = ParsedAttrs<'src>`, `Token = TokenId`.

Also folded into B3 (deferred from B1): `RuleContext<'a>` → `RuleContext<'a, S>` and
`Rule::check(&CanonicalAttrs, …)` → `check(&S::Canonical, …)`. This is *why* it's in B3:
the field migration is only substantive alongside the `S::Canonical` threading.

## 3. The crux: how does the recognizer generify?  (DESIGN FORK — needs your call)

Today `recognizer: EngineRecognizer`, an enum with `Strict(StrictRecognizer)`,
`StrictOrDecoder(StrictOrDecoderRecognizer)`, `Dyn(Arc<dyn Recognizer<CapcoScheme>>)`. The
two concrete variants are a **monomorphization fast-path** that exists specifically for
Constitution I (p95 ≤ 2 ms): they avoid a vtable call per candidate. The concrete
recognizers (`StrictRecognizer`, `DecoderRecognizer`) are inherently CAPCO — they parse
CAPCO markings.

A generic `Engine<S>` needs a recognizer for `S`. Two shapes:

**Fork A — trait-object recognizer.** `recognizer: Arc<dyn Recognizer<S>>`. Simplest; one
type param. **Cost:** the default CAPCO path loses its monomorphized enum and pays a vtable
call per candidate. Must be measured against the interactive-latency bench — if it regresses
past p95 ≤ 2 ms it's a non-starter as-is.

**Fork B — generic recognizer param.** `Engine<S, R: Recognizer<S> = EngineRecognizer>`.
Keeps the monomorphized enum for `Engine<CapcoScheme, EngineRecognizer>` (default → zero
perf change), and an arbitrary `S` supplies its own `R`. **Cost:** a second type param
threads through every `impl` and call site; `EngineRecognizer` itself must become
`EngineRecognizer<S>` (its `Strict`/`StrictOrDecoder` variants stay gated to the CAPCO
recognizers, so for non-CAPCO `S` only the `Dyn` variant is usable — or the default param
makes this invisible for CAPCO callers).

**Recommendation:** **Fork B.** It preserves the perf fast-path the constitution mandates,
and the second type param is defaulted so CAPCO call sites stay unchanged (same trick as
B2's `S = CapcoScheme`). Fork A is simpler but bets the latency budget on a vtable call we
deliberately engineered away. I want your call before building either.

## 4. Closing the scheme-discard

Once `Engine<S>` is real, the constructor stops needing `bridge_scheme = CapcoScheme::new()`
+ `drop(scheme)` — it stores the user's `scheme: S` and the bridge (`constraint_rule_id`,
`bridge_emitted_rule_ids`) reads it generically. This is the user-visible payoff of B3 and
removes the documented silent-discard footgun. The bridge currently hardcodes
`RuleId::new("capco", …)` (bridge.rs:153) — that becomes `scheme.constraint_rule_id(label)`
(the trait method B1 added, returning the `(scheme, predicate)` 2-tuple). One real
behavior change to verify: a caller-customized `CapcoScheme` is now honored, not dropped.

## 5. Sub-PR split (recommendation)

B3 is large but **not atomic** — it stages cleanly behind the still-present `= CapcoScheme`
default param (nothing external moves until B4 strips it):

- **B3.1 — `MarkingScheme::Projected` + `RuleContext<'a, S>` + `Rule::check(&S::Canonical, &RuleContext<'_, S>)`.**
  CORRECTION (was wrongly scoped "WASM-safe, no engine edit"): changing the `Rule::check`
  *trait method* signature is a breaking change every implementor follows — and the engine
  SRC both constructs `RuleContext` and hosts `cfg(test)` `Rule` impls, so B3.1 MUST edit
  engine SRC (mechanical retypes only; the decoder/recognizer/perf work stays in B3.2+).
  - `RuleContext<'a, S>` has **NO default param** (mirrors `Diagnostic<S>`, which has none),
    because `marque-rules` does NOT depend on `marque-capco` and cannot name `CapcoScheme`.
    Downstream sites that wrote `RuleContext<'_>` become `RuleContext<'_, CapcoScheme>`.
  - `RuleContext` real shape (9 fields, `#[derive(Debug, Clone)] #[non_exhaustive]`):
    `marking_type, zone, position, candidate_span: Span, page_portions:
    Option<Arc<Box<[CanonicalAttrs]>>>, page_marking: Option<Arc<ProjectedMarking>>,
    page_banner_span: Option<Span>, corrections: Option<Arc<HashMap<String,String>>>,
    pre_pass_1_attrs: Option<&'a CanonicalAttrs>` + 7 `with_*` builders. Generify ONLY the
    three scheme-typed fields (`page_portions`→`S::Canonical`, `page_marking`→`S::Projected`,
    `pre_pass_1_attrs`→`S::Canonical`); `page_banner_span`/`corrections`/`candidate_span`
    stay concrete. `#[derive(Debug,Clone)]` injects spurious `S: Debug/Clone` → hand-write
    both impls bounded on the assoc types, not `S`.
  - Engine SRC edit inventory (bounded, all mechanical): `lint_helpers.rs:293`
    (`RuleContext::new`+builders) & `:345/:347` (`rule.check`); `page_context.rs:270` &
    `:352/:354`; `cfg(test)` `Rule` impls at `engine/tests.rs:246/387/426`,
    `tests/part1.rs:198/255/420`, `part2.rs:223`, `part3.rs:623`, `part4.rs:27`.
  - ~17 capco rule files (`impl Rule<CapcoScheme>`) + ~29 `MarkingScheme` impls bind
    `type Projected` (Capco→`ProjectedMarking`, all stubs→`()`).
  Still the right LEAD slice — it de-risks the `S::Canonical`/`S::Projected` mapping across
  the whole rule surface before the decoder/recognizer perf work. NOT engine-edit-free, but
  engine edits are mechanical retypes, no logic change.
- **B3.2 — recognizer generification** (Fork A or B). Engine-only. The perf-gated piece —
  interactive-latency bench before/after is a hard gate here.
- **B3.3 — thread `S::Canonical`/`S::Marking`/`S` through dispatch + lint + fix** (the 116 +
  88 + 287 refs). Move `impl Engine<CapcoScheme>` → `impl<S: MarkingScheme> Engine<S>`
  method-group by method-group. The long middle.
- **B3.4 — close the scheme-discard** (constructor stores `scheme: S`; bridge reads it
  generically). Small but behavior-affecting; its own commit so the audit-parity diff is
  isolated.

Each sub-PR keeps the full suite green and the default param intact. If you'd rather one big
B3 PR, that's viable too but harder to review and to bisect if a perf gate trips.

## 6. Gates (every sub-PR)

- `cargo test --workspace` green; **audit-parity** (`audit_v3_0_parity`) + **G13
  content-ignorance canary** unchanged (B3 touches types, not audit content).
- corpus accuracy harness ≥ 95% per-rule.
- **interactive-latency bench** at B3.2 and B3.3 — the perf fork lives or dies here.
- `cargo vet` (dep graph unchanged, so likely skipped — but `cargo vet fmt` discipline per
  the #835 gotcha).
- rust-reviewer + code-reviewer before each PR-open; **verify on Rust 1.89** (CI floor) in
  addition to local stable.

## DECISIONS (signed off 2026-05-31)

- **Recognizer: Fork B** — `Engine<S, R: Recognizer<S> = EngineRecognizer>`. Keeps the
  monomorphized CAPCO fast-path; defaulted `R` keeps call sites unchanged. (`EngineRecognizer`
  becomes generic where needed; its `Strict`/`StrictOrDecoder` variants stay CAPCO-gated.)
- **Structure: 4-way split** B3.1 → B3.4, lead with B3.1.
- B3.1 first (RuleContext<'a, S> + Rule::check(&S::Canonical), WASM-safe, no engine change).
- **page_marking: add `type Projected` to `MarkingScheme`** (signed off). `MarkingScheme`
  gains `type Projected: Send + Sync + 'static` (adjust bounds as the compiler requires —
  `RuleContext` derives `Debug`/`Clone`). `CapcoScheme::Projected = ProjectedMarking`.
  `RuleContext` carries `page_marking: Option<Arc<S::Projected>>`,
  `page_portions: Option<Arc<Box<[S::Canonical]>>>`, `pre_pass_1_attrs: Option<&'a S::Canonical>`.
  `MarkingScheme` is NOT a protected stable surface (only audit-schema + lattice-trait are,
  per the pre-users policy), so this additive assoc type is fine. NOTE the 17 type-position
  `ProjectedMarking` uses in capco rules (6 banner `eval_*` fns take `&ProjectedMarking`);
  capco `Rule` impls are concrete `impl Rule<CapcoScheme>`, so inside them
  `S::Projected = ProjectedMarking` and they keep naming the concrete type — minimal churn.
  Every test-fixture stub scheme (adoption_readiness, scheduler, test-utils stub_scheme,
  default_scheme_id, codec_surface, send_sync, evaluator, page_rewrite, canonical, input,
  fix_intent) must add `type Projected = ...`. `RuleContext<'a, S = CapcoScheme>` keeps a
  DEFAULTED param so engine `RuleContext::new(...)` call sites infer CapcoScheme and stay
  unchanged → B3.1 needs NO engine edit.

## 7. Open questions for you

1. **Recognizer fork A vs B** (§3) — I recommend B (preserves perf fast-path). Your call.
2. **One B3 PR or the 4-way split** (§5) — I recommend the split (reviewable, bisectable,
   isolates the perf-gated piece). Your call.
3. **B3.1 first?** Landing `RuleContext<S>`/`Rule::check<S>` first (WASM-safe, no engine
   change) de-risks the `S::Canonical` mapping cheaply. OK to lead with it?
