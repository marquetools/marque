# Contract: Page-Rewrite Scheduler

**Crate:** `marque-engine` (scheduler) + `marque-scheme` (types)
**Phase:** C
**Spec refs:** FR-003, FR-004, FR-005, FR-007, SC-005

## Intent

Page-level rewrites are scheduled deterministically by topological sort over their read/write axis dependencies, once, at `Engine::new`. Cycles and unannotated custom rewrites produce `Engine::new` errors, not per-document errors.

## Surface

```rust
pub enum EngineConstructionError {
    RewriteCycle { axis: CategoryId, members: &'static [RewriteId] },
    UnannotatedCustomAxes { rewrite: RewriteId },
    // ... existing variants
}

impl Engine {
    /// Existing signature gains a scheme parameter and becomes fallible.
    /// `rule_sets` is preserved from the pre-Phase-C signature (rule dispatch
    /// and marking-scheme grammar are distinct concerns — both must plumb
    /// through construction).
    pub fn new<S: MarkingScheme>(
        config: Config,
        rule_sets: Vec<Box<dyn RuleSet>>,
        scheme: S,
    ) -> Result<Self, EngineConstructionError>;
}
```

```rust
impl<S: MarkingScheme> PageRewrite<S> {
    pub const fn declarative(
        id: RewriteId,
        trigger: CategoryPredicate<S>,
        action: CategoryAction<S>,
        citation: &'static str,
    ) -> Self;

    pub const fn custom(
        id: RewriteId,
        trigger: CategoryPredicate<S>,
        action: CategoryAction<S>,
        reads: &'static [CategoryId],
        writes: &'static [CategoryId],
        citation: &'static str,
    ) -> Self;
}
```

**Citation type:** `&'static str` per foundational-plan line 943 and 984 — NOT a structured `SourceCitation`. Citation verification is a commit-time discipline (Constitution VIII + T089), not a type-level invariant.

**`const fn` caveat:** `const fn` composition with `S: MarkingScheme + ?Sized` is restricted on stable Rust because trait dispatch through `?Sized` is not const-evaluable. Two mitigations, either acceptable:
1. Implement the constructors as regular `fn` (not `const fn`) and let call sites build rewrites in a `OnceLock` or in scheme-level `&'static [PageRewrite<S>]` slices initialized at module load; OR
2. Implement as `const fn` with a `#[macro_rules_or_attribute]` helper that expands the axis derivation at the call site from the variant literal, sidestepping the `?Sized`-vs-const-fn interaction.
Mitigation (1) is the safer default; mitigation (2) is a performance follow-up if the scheduler proves allocation-sensitive.

## Contract

- **Declaration-order independence (FR-007):** Given a cycle-free rewrite set, permuting the declaration order MUST produce the same scheduled order and the same diagnostic output.
- **Cycle rejection (FR-004):** A cycle among the declared rewrites fails `Engine::new` with `RewriteCycle { axis, members }`. `members` is a slice (not a fixed-length array) because cycles longer than two rewrites are valid failure modes per foundational-plan line 1066. The error names every participating member.
- **Unannotated-custom rejection (FR-005):** A `PageRewrite::custom` missing `reads` or `writes` is a compile error (empty-slice sentinel intercepted by a `const fn` guard), or an `Engine::new` failure if the empty-slice check is runtime. Either is acceptable; the guarantee is "no runtime can observe such a rewrite fire."
- **Schedule produced once per `Engine`:** `Engine::new` runs the topological sort; per-document rewrite evaluation walks the pre-computed order.
- **Static annotations (`reads`/`writes` as `&'static [CategoryId]`):** No per-document allocation in the scheduler.

## Failure modes

| Error | Trigger | Test |
|---|---|---|
| `RewriteCycle` | Any read/write cycle in the declared rewrite set | Synthetic 4-rewrite cycle fixture in `marque-scheme` tests |
| `UnannotatedCustomAxes` | `PageRewrite::custom` with empty `reads` or `writes` | Unit test in `marque-scheme` |

## Test scenarios

1. **Declaration-order independence:** Given the Phase C rewrite set with NOFORN⊐REL-TO, JOINT-promotion, and FGI-absorption, permute the declaration order in `capco::scheme::rewrites()` and verify the scheduled run order is identical. Run the corpus accuracy harness; diagnostic output stays byte-identical across permutations.
2. **Real producer-consumer edge:** The Phase C rewrite set exercises a real read/write edge (JOINT-promotion writes `fgi`; FGI-absorption reads `fgi`). Verify the scheduler runs JOINT-promotion before FGI-absorption.
3. **Cycle error (≥2 members):** Construct a synthetic 4-rewrite set with A→B→C→D→A read/write edges. Verify `Engine::new` fails with `RewriteCycle { axis, members }` where `members` names every participating rewrite (not just the entry point) — the length can exceed 2.
4. **Custom without annotations:** A `PageRewrite::custom` with `reads = &[]` fails at `Engine::new` with `UnannotatedCustomAxes`.
