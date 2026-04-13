# PR Review: #3 — feat: marque MVP foundation — spec, codegen, engine pipeline

**Reviewed**: 2026-04-10
**Author**: @bashandbone (Adam Poulemanos)
**Branch**: `001-marque-mvp` → `main`
**Decision**: **APPROVE** with comments

## Summary

Phase 2 foundational delta — 25 files, +2312/-479. The earlier commits (spec, Phase 1 workspace restructure, Phase 1 review fixes) already merged via PR #1, so this PR is the pure Phase 2 code. An exhaustive local review of these exact files was performed before the commit landed (4 CRITICAL + 10 HIGH + 10 MEDIUM + 9 LOW), with every finding resolved. This PR review is a **second independent pass** focused on interactions and subtleties that the first pass may have missed — the overlap guard correctness, `from_utf8_unchecked` soundness, `Send`/`Sync` bounds, and edge cases around `fix_with_threshold`. All three correctness focus areas came back clean.

## Validation Results

| Check | Result |
|---|---|
| Lint (`cargo clippy --workspace --all-targets -- -D warnings`) | Pass — zero warnings |
| Tests (`cargo test --workspace`) | Pass — 45 unit + 1 doctest, zero failures |
| Format (`cargo fmt --check`) | Pass |
| Build (`cargo check --workspace`) | Pass |
| GitHub mergeability | CLEAN / MERGEABLE |

## Findings

### CRITICAL

None.

### HIGH

**H-1 — `BatchError` does not distinguish panic from cancellation**
`crates/marque-engine/src/batch.rs:48–77`

`BatchError::TaskFailed(JoinError)` wraps `tokio::task::JoinError` whole, but `JoinError` can represent either a panic (application bug) or a cancellation (runtime shutdown — not a bug). Callers in CI pipelines need to distinguish these to decide whether to alert or retry. `Display` collapses both into `"batch task failed: {e}"` and the only way to tell them apart is to `source()` + downcast — undocumented and non-obvious.

Suggested fix: forward the discriminators through the wrapper.
```rust
impl BatchError {
    pub fn is_panic(&self) -> bool {
        match self { Self::TaskFailed(e) => e.is_panic() }
    }
    pub fn is_cancelled(&self) -> bool {
        match self { Self::TaskFailed(e) => e.is_cancelled() }
    }
}
```

**H-2 — `AppliedFix::__engine_promote` is `pub` `#[doc(hidden)]` — audit record integrity guarantee is convention-only**
`crates/marque-rules/src/lib.rs:246–261`

The doc comment explicitly admits: "enforced by convention and code review, not by the type system." Any downstream crate that depends on `marque-rules` can construct an `AppliedFix` directly, bypassing the engine's confidence-threshold gate, the FR-016 sort, and the C-1 overlap guard — and inject arbitrary audit records. For a security tool whose audit log is the compliance output, this is a meaningful integrity gap. The `__` prefix is a discouragement, not a guarantee.

This is a known architectural limitation from the Phase 2 design (the type must live in `marque-rules` because `marque-engine` depends on it, not the reverse), and a full fix would restructure crate boundaries. Recommended: document this explicitly as a known risk in `CLAUDE.md` under architectural invariants so future crate authors know the boundary is not load-bearing.

**H-3 — Public `fix_with_threshold` has no test for INFINITY / NEG_INFINITY rejection**
`crates/marque-engine/src/engine.rs:146–149`

The validation `!(0.0..=1.0).contains(&value) || value.is_nan()` correctly rejects `f32::INFINITY` and `f32::NEG_INFINITY`, but the test suite (`fix_with_threshold_rejects_nan`, `fix_with_threshold_rejects_out_of_range`) only checks `NaN`, `-0.1`, and `1.1`. Because this is a public API surface, a future refactor could silently regress the INFINITY rejection path without any test catching it.

Suggested fix: add two one-liners.
```rust
#[test]
fn fix_with_threshold_rejects_infinity() {
    let engine = engine_with(vec![]);
    assert!(matches!(
        engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(f32::INFINITY)),
        Err(InvalidThreshold(_))
    ));
    assert!(matches!(
        engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(f32::NEG_INFINITY)),
        Err(InvalidThreshold(_))
    ));
}
```

### MEDIUM

**M-1 — `resolve_idents` has no lower-bound assertion on CVE file contents**
`crates/marque-ism/build.rs:192–209`

If `CVEnumISMDissem.xml` were ever empty (bad schema copy, partial ODNI update), `emit_enum` would emit a valid-but-empty `DissemControl` enum and all dissem rules would silently fire zero diagnostics. Recommended: add a whitelist of required-nonzero CVE files and assert each has at least one entry at build time. `SarIdentifier` already has a special empty-handling code path, so the whitelist would be everything *except* SAR.

**M-2 — `resolve_idents` collision error message is non-actionable**
`crates/marque-ism/build.rs:204`

On a duplicate-ident collision the message is `"CVE values produce duplicate identifier {ident:?} (one of them is {value:?}). to_rust_ident needs disambiguation."` — but it only names one of the two colliding values, so a future maintainer can't see which pair collided. Suggested fix: change `seen` from `HashSet<String>` to `HashMap<String, String>` mapping `ident → first_value`, and include both values in the panic message.

**M-3 — `TRIGRAPHS` slice is unsorted so `is_trigraph` uses linear scan**
`crates/marque-ism/build.rs` (`parse_xsd_trigraphs`) + `token_set.rs:44`

The sister `ALL_CVE_TOKENS` went through a `BTreeSet` so `canonicalize` could use `binary_search`. `TRIGRAPHS` was left in XSD document order and `is_trigraph` still calls `.contains()` (~340-entry linear scan per token in parsed marking). The fix is symmetrical to H-8 from the first-pass review: sort at emit time, use `binary_search`. Low hot-path impact today but opportunistic cleanup.

**M-4 — Threshold filter path has no test coverage at the threshold boundary**
`crates/marque-engine/src/engine.rs` (tests)

All engine test proposals use `confidence = 1.0`, which always passes the `>= 0.95` default threshold. There is no test that a proposal with `confidence = 0.94` is excluded under the default threshold or that a proposal with exactly `0.95` is included. The filtering path (`filter(|f| f.confidence >= threshold)`) could silently regress (e.g., `>` instead of `>=`) without any test catching it.

**M-5 — Zero-length-span filter path has no dedicated test**
`crates/marque-engine/src/engine.rs:163`

`fix_inner` filters out fixes where `f.span.is_empty()`. This is the guard that masks the Phase 2 `Span::new(0, 0)` placeholder from current CAPCO rules, and the C-1 overlap guard is designed to take over once Phase 3 wires real spans. A test that verifies a zero-length fix is filtered would pin this contract explicitly and catch a future refactor that accidentally drops the guard.

### LOW

**L-1** — `BatchOptions.max_concurrent_docs` drives both `ConcurrencyController::max_inflight_rows` and the `buffer_unordered` cap; doc comment only mentions the latter. (`batch.rs:85`)

**L-2** — `merge_user_into` does not treat empty-string `classifier_id` as absent; an empty string from `.marque.local.toml` would overwrite a populated value from another layer via last-write-wins. Minor; caller controls config. (`config/lib.rs:263–278`)

**L-3** — `Severity` doc describes a future `.max()` merge-semantics design, but the current `merge_project_into` uses last-write-wins — a local `"error"` can overwrite a project `"off"`. The comment creates a documentation expectation that diverges from the implementation. Either update the comment or implement the `.max()` merge. (`rules/lib.rs:60–67`)

**L-4** — `engine_with` tests use `Config::default()` which pins `confidence_threshold = 0.95`, but all test proposals use `1.0` confidence. No test exercises the `Config`-supplied threshold path; all the threshold testing goes through `fix_with_threshold` overrides. Closely related to M-4. (`engine.rs:349–362`)

## Files Reviewed

All 25 files in the PR diff were read in full:

- `Cargo.lock` (Modified)
- `crates/marque-capco/src/rules.rs` (Modified)
- `crates/marque-config/Cargo.toml` (Modified)
- `crates/marque-config/src/lib.rs` (Modified)
- `crates/marque-core/src/attrs.rs` (Modified)
- `crates/marque-core/src/parser.rs` (Modified)
- `crates/marque-core/src/scanner.rs` (Modified)
- `crates/marque-core/src/span.rs` (Modified)
- `crates/marque-engine/src/batch.rs` (Modified)
- `crates/marque-engine/src/clock.rs` (Added)
- `crates/marque-engine/src/engine.rs` (Modified)
- `crates/marque-engine/src/lib.rs` (Modified)
- `crates/marque-engine/src/output.rs` (Modified)
- `crates/marque-engine/src/pipeline.rs` (Modified)
- `crates/marque-extract/src/lib.rs` (Modified)
- `crates/marque-ism/Cargo.toml` (Modified)
- `crates/marque-ism/build.rs` (Modified)
- `crates/marque-ism/src/attrs.rs` (Modified)
- `crates/marque-ism/src/lib.rs` (Modified)
- `crates/marque-ism/src/span.rs` (Modified)
- `crates/marque-ism/src/token_set.rs` (Modified)
- `crates/marque-rules/src/lib.rs` (Modified)
- `crates/marque-server/src/main.rs` (Modified)
- `crates/marque-wasm/src/lib.rs` (Modified)
- `marque/src/main.rs` (Modified)

## Focus-Area Results

The second-pass review explicitly tested these concerns — all clean:

| Concern | Result |
|---|---|
| Overlap guard correctness under FR-016 reverse-end sort | ✓ Sound. Walked through adjacent, contained, identical, and overlapping span cases — all handled correctly by `fix.span.end <= previous.span.start` |
| `Trigraph::as_str` `from_utf8_unchecked` soundness | ✓ Sound. No construction path outside `try_new` (ASCII validator) or the `USA` const |
| `Severity::Off` gate at the rule loop | ✓ Correctly short-circuits before any diagnostic is produced |
| `Send`/`Sync` on `Engine` under `Arc` in `BatchEngine` | ✓ `Config: Clone + Send`, `Vec<Box<dyn RuleSet>>` with `Send + Sync`, `Box<dyn Clock>` with `Send + Sync` — all correct |
| `-0.0` / subnormal handling in `fix_with_threshold` | ✓ Accepts `-0.0` (IEEE equal to `+0.0`) and subnormals — correct |

## Decision

**APPROVE** with comments.

No blocking issues. The three HIGH findings are:
- **H-1 (BatchError API)**: a genuine API-design refinement worth a quick follow-up
- **H-2 (__engine_promote)**: a documented architectural limitation that should be explicitly noted in CLAUDE.md but does not block merge
- **H-3 (INFINITY test gap)**: two-line test addition

None of the findings should block Phase 3. H-1 and H-3 can land in the next fix pass or the Phase 3 opening commit; H-2 is a documentation update. The MEDIUM items are worthwhile follow-ups but are polish, not correctness.
