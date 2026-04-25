# Phase 4 (US2) Code Review — Multi-Agent Synthesis

**Branch**: `phase4review` (worktree at `/home/knitli/marque/.worktrees/phase4review`)
**Reviewed against**: `specs/004-constraints-decoder-vocab/{tasks.md,spec.md,contracts/}` and `.specify/memory/constitution.md`
**Reviewers**: 5 specialized agents dispatched in parallel + direct verification of cross-cutting CI lanes
**Date**: 2026-04-25

## Decision: REQUEST CHANGES

Phase 4 is feature-complete and the implementation is largely sound — `cargo check`, `cargo clippy --all-targets -- -D warnings`, and the full default-feature test suite (1152 tests) are green. WASM correctly rejects `--features corpus-override` at the Cargo layer. The corpus-override threat boundary holds across all three channels (body / header / query, including percent-encoded variants).

There are **four HIGH findings**, two of which are latent bugs whose values happen to be correct today but violate documented contracts, plus one CI-coverage gap that leaves several Phase 4 regression gates dormant. None are runtime CRITICAL — production traffic produces correct output today — but each represents a regression hazard that will silently bite a future change.

The acknowledged accuracy gap (T057: decoder ~53% vs SC-004 ≥85%, GH issue #133) is correctly handled by the harness's two-gate split (`#[ignore]` target + always-on regression floor), and the `delta=0.0` placeholder for `CorpusOverrideInEffect` is documented in tasks.md as audit-trail-only pending a coordinated audit-schema bump.

## Severity Summary

| Severity | Count | Disposition |
|----------|-------|-------------|
| CRITICAL | 0 | — |
| HIGH | 4 | Must fix before merge |
| MEDIUM | 11 | Should fix before merge |
| LOW | 7 | Optional |
| **False positives** | 1 | Recorded for audit trail |

## HIGH Findings

### H1 — v2 audit emitter reads `proposal.*` instead of top-level snapshot

**File**: `marque/src/render.rs:466, 470`
**Contract**: `crates/rules/src/lib.rs:330-339` (doc on `AppliedFix`): "the v2 schema reads the **top-level** fields"
**Reporter**: Agent D (verified directly by overseer at lib.rs:342-396 + render.rs:464-486)

`applied_fix_to_audit_json_v2` reads `fix.proposal.confidence` (line 466) and `fix.proposal.source` (line 470) instead of the top-level `fix.confidence` / `fix.source` snapshot fields that `__engine_promote` populates specifically for v2 consumption (lib.rs:385-386). Today the values are identical copies, so output is correct — but the doc comment at lib.rs:330-334 explicitly anticipates that "a future phase may adjust these per region-context before snapshotting." The first such phase will silently emit unmodified proposal values.

**Fix** (one-liner each):
```rust
// render.rs:466
let c = &fix.confidence;
// render.rs:470
source: fix_source_str(fix.source),
```

### H2 — Divergent `FeatureId` label registries on the audit wire

**Files**: `crates/engine/src/decoder.rs:409-418` (`feature_label`) vs `crates/rules/src/confidence.rs:220-230` (`FeatureId::as_str`)
**Contract**: `confidence.rs:209` doc comment: "This is the **single source of truth**" for FeatureId serialization
**Reporter**: Agent A

`feature_label()` in `decoder.rs` is a second match arm on `FeatureId` with snake_case wire labels (`"edit_distance_1"`, `"token_reorder"`), while `FeatureId::as_str()` — declared the canonical registry — emits PascalCase (`"EditDistance1"`, `"TokenReorder"`). `feature_label()` is called at decoder.rs:351 to populate `EvidenceFeature::label`. The current `StrictOrDecoderRecognizer` dispatcher discards `Ambiguous` results from the decoder (decoder.rs:1259-1267), so this code path is unreachable at runtime today — but a dispatcher policy change (e.g., surfacing decoder ambiguity as a separate diagnostic class) would make the wire-format divergence immediate.

**Fix**: Delete `feature_label()` entirely. Replace the call at decoder.rs:351 with `f.id.as_str()`. The trait surface already requires `EvidenceFeature::label: &'static str`, which `as_str` returns.

### H3 — Decoder-accuracy and corpus-override regression suites are dormant in CI

**Files**: `.github/workflows/ci.yml:64` (sole test invocation: `cargo nextest run --workspace --profile ci`)
**Reporters**: Agent C + direct CI inspection by overseer

The CI test job uses default features only. The following test suites are file-level gated on features that are NOT in any default feature set and are NOT explicitly enabled in CI:

| Test File | Feature Required | Status |
|-----------|------------------|--------|
| `crates/engine/tests/decoder_accuracy.rs` (T057) | `decoder-harness` | **Dormant** — including `resolution_rate_does_not_regress` always-on floor and `MIN_FIXTURE_COUNT=200` vacuity guard |
| `crates/engine/tests/corpus_override.rs` (T069) | `corpus-override` | **Dormant** — both positive ("every decoder fix carries marker") and negative ("no fix carries it without override") gates |
| `crates/capco/tests/corpus_parity.rs` (Phase 3 byte-identity) | `corpus-harness` | **Dormant** — Phase 3 declarative-migration regression gate |

I verified each suite passes when explicitly invoked with the correct feature flags (decoder_accuracy: 3 passed + 1 ignored; corpus_override: 2 passed; corpus_parity: 5 passed). The bug is in CI invocation, not the tests themselves.

**Fix** (CI workflow change): add a CI matrix or explicit step:
```yaml
- name: cargo nextest (Phase 4 regression suites)
  run: |
    cargo nextest run -p marque-engine --features decoder-harness --test decoder_accuracy
    cargo nextest run -p marque-engine --features corpus-override --test corpus_override
    cargo nextest run -p marque-capco --features corpus-harness --test corpus_parity
```

### H4 — `SUPERSEDED_TOKEN_MAP` citation points to wrong section/page

**File**: `crates/engine/src/decoder.rs:765-766`
**Contract**: Constitution VIII (citation fidelity): a misattributed citation is "a correctness defect of the same severity as a wrong predicate"
**Reporters**: Agent A (HIGH) + Agent E (MEDIUM); overseer adopts HIGH per Constitution VIII text

Citation reads `// CAPCO-2016 §A.6 p16 (COMINT title for the SI control system is no longer valid)`. The COMINT→SI supersession note actually appears at `crates/capco/docs/CAPCO-2016.md:1714`, which is **page 74 inside §H.4 (SCI Control System Markings)** — not §A.6 p16. Page 16 contains the SCI grammar example, not the supersession note. The behavior of `SUPERSEDED_TOKEN_MAP` (substituting COMINT→SI) is correct; only the comment citation is wrong.

**Fix**: Change comment to:
```rust
// CAPCO-2016 §H.4 p74 (COMINT title for the SI control system is no longer valid)
```

## MEDIUM Findings

### M1 — Aggregate-only regression floor on T057 (no per-class detection)

**File**: `crates/engine/tests/decoder_accuracy.rs:104-138, 458`
**Reporter**: Agent C (originally CRITICAL; overseer downgrades to MEDIUM — see note)

The 50% aggregate floor cannot detect a per-class regression masked by another class's improvement (e.g., Reordering 100%→60% offset by Typo 20%→40%). Tasks.md describes the per-class breakdown but the harness only asserts aggregate. Agent C labels this CRITICAL; the overseer downgrades to MEDIUM because the decoder is at ~53% aggregate today, so any meaningful regression would push it under 50% before per-class compensation could absorb it. The fix is still worth landing.

**Fix**: Pin currently-passing classes (Reordering 100%, WrongCase 100%, GarbledDelimiter 100%) at their current rates as per-class floors; ratchet Typo and MissingDelimiter as #133's checklist clears.

### M2 — Missing `static_assertions::assert_impl_all!` for `Box<dyn Recognizer<CapcoScheme>>: Send + Sync`

**File**: workspace-wide (no occurrence)
**Reporter**: Agent E

`Recognizer<S>: Send + Sync` is enforced via supertrait (`crates/rules/src/lib.rs:147`), so trait objects are `Send + Sync` by Rust's rules. The local `assert_send_sync<T>()` helper in `crates/engine/tests/decoder_dispatch.rs` is semantically equivalent but less canonical. Adding the standard assertion completes the Constitution VI evidence chain.

**Fix**: Add `static_assertions` as dev-dep; in `crates/engine/tests/decoder_dispatch.rs` add `assert_impl_all!(Box<dyn Recognizer<CapcoScheme>>: Send, Sync); assert_impl_all!(Arc<dyn Recognizer<CapcoScheme>>: Send, Sync);`.

### M3 — Missing `#[must_use]` on `Engine::with_deep_scan` and `Engine::with_corpus_override`

**File**: `crates/engine/src/engine.rs:249, 280`
**Reporter**: Agent A

Both methods are consume-and-return builders. Without `#[must_use]`, calling `engine.with_deep_scan()` without rebinding silently leaves the engine in strict-only mode — a latency-correctness bug (Constitution I) that would mislead a test harness.

**Fix**: `#[must_use = "this builder returns a new Engine; the result must be bound to take effect"]` on both methods.

### M4 — `unwrap_or(MarkingType::Banner)` contradicts the comment at decoder.rs:1233

**File**: `crates/engine/src/decoder.rs:1233`
**Reporter**: Agent A

Comment says "If inference fails the bytes are too degenerate for either path — skip." But the code falls through to `strict_parse_is_complete(m, MarkingType::Banner)` instead of skipping. Behavior is accidentally correct (degenerate inputs produce zero-attribute markings rejected downstream), but the comment-vs-code contradiction is misleading.

**Fix**: Replace with `let Some(kind) = infer_marking_type(bytes) else { return strict_result; };` — matches the comment's intent and the recognizer guard at line 156-159.

### M5 — Dead second guard in `try_canonical_reorder`

**File**: `crates/engine/src/decoder.rs:824-826`
**Reporter**: Agent A

`if class_segments.len() + dissem_segments.len() + other_segments.len() == 0` cannot be true — the preceding guard at line 821 already returns `None` when `class_segments.is_empty()`, and the loop at 809-819 pushes into at least one of the three vecs for each non-empty segment.

**Fix**: Remove lines 824-826.

### M6 — `validate_log_prior` rejects `-Inf`

**File**: `crates/config/src/corpus_override.rs:271-278`
**Reporter**: Agent B

Error message says "not NaN, +Inf, or -Inf" but `-Inf` is a legitimate "infinite penalty / dead token" value an operator may want for hard exclusion. The current code rejects it; either policy is defensible, but code and documented intent should agree.

**Fix**: Decide policy. If `-Inf` should be allowed, drop the `!is_finite()` rejection and update the error string. Otherwise, document explicitly why infinite penalties are forbidden.

### M7 — Two-pass body-field check in server is a maintenance hazard

**File**: `crates/server/src/lib.rs:352-365`
**Reporter**: Agent B

`reject_if_corpus_override` is called with `body_has_override = false` (hardcoded), then a separate post-deserialization `if req._corpus_override.is_present()` runs. The `body_has_override` parameter is dead at every call site. Future refactor risk: a developer might consolidate the path and inadvertently drop the second check.

**Fix**: Either remove the `body_has_override` parameter and handle body rejection entirely inside the handler post-deserialization, or add a doc comment at `reject_if_corpus_override` explaining the two-pass rationale.

### M8 — `require_probability` allows `p == 0.0`

**File**: `crates/capco/build.rs:277-287`
**Reporter**: Agent D

`require_probability` accepts `0.0`. For `strict_context_priors` (probabilities, not log-priors), a regenerator emitting `0.0` would silently produce a permissive floor that never rejects any candidate, defeating FR-011 semantics.

**Fix**: Add `v > 0.0` check specifically for `strict_context_priors` rows, or document why `0.0` is intentionally allowed.

### M9 — `schema_version_is_pinned` is runtime-only despite the name

**File**: `crates/capco/src/priors.rs:95-96`
**Reporter**: Agent D

The test name implies a compile-time guarantee that doesn't exist. `build.rs:73-82` already panics on a wrong schema version at build time, so build-time safety is intact, but the test as written is redundant with that panic and runtime-only.

**Fix**: Either rename to `schema_version_matches_at_runtime`, or replace with `static_assertions::const_assert_eq!(SCHEMA_VERSION, "marque-priors-1")`.

### M10 — Missing integration tests for query parameter uppercase + percent-encoded hyphen

**File**: `crates/server/tests/http.rs` (no test exists)
**Reporter**: Agent B

Unit-level `query_carries_corpus_override` covers `%5F` → `_` and `%2D` → `-`, but there are no integration tests against the live handler for `?CORPUS_OVERRIDE=` (uppercase) or `?corpus%2Doverride=1` (percent-encoded `-`). The unit test covers the logic, but a wiring regression that bypassed `query_carries_corpus_override` would slip past.

**Fix**: Add `rejects_corpus_override_query_uppercase` and `rejects_corpus_override_query_percent_encoded_hyphen` integration tests.

### M11 — Per-class unit tests missing for missing-delimiter, superseded-token, wrong-case, garbled-delimiter

**File**: `crates/engine/tests/decoder_recovery.rs`
**Reporter**: Agent C

Named tests exist for typo (3) and reordering (1), but not for the other four mangling classes. T057 covers them aggregate, but a regression in WrongCase requires running the 200-case harness to surface — which isn't even in CI today (see H3).

**Fix**: Add at least one `#[test]` per remaining class (e.g., `secret_lowercase_decodes_to_canonical`, `comint_supersedes_to_si`, `slash_garbled_delimiter_normalizes`, `secret_no_delim_dissem_normalizes`).

## LOW Findings

### L1 — `runner_up_ratio` saturation note (Agent A area 2)

`f32::MAX` saturation at `decoder.rs:308-314` is correct against `Confidence::validate` (`check_finite` passes). No bug; design is documented in `audit-record-v2.md`. No action.

### L2 — `UNAMBIGUOUS_LOG_MARGIN=1.6` "≈5× odds" comment shorthand

Mathematically correct (`e^1.6 ≈ 4.95`). Could be misread as "5× probability" rather than "5× odds ratio". Optionally clarify the comment.

### L3 — NaN handling in sort uses `unwrap_or(Ordering::Equal)`

`decoder.rs:282-288`. NaN posteriors should be impossible given finite priors and feature deltas, but this is unasserted. Optional: add `debug_assert!(!a.posterior.is_nan() && !b.posterior.is_nan())` or use `total_cmp` (stable since Rust 1.62).

### L4 — `marque-config` dep in WASM lacks `default-features = false`

`crates/wasm/Cargo.toml:92`. Today `marque-config`'s default = `[]`, so WASM is safe by transitive default. Defense-in-depth: pin `default-features = false` to make this robust against a future change to `marque-config` defaults.

### L5 — `__engine_promote` test exception lacks Constitution V carve-out

`marque/src/render.rs:794` and `crates/engine/tests/audit.rs:337` both call `__engine_promote` outside `Engine::fix_inner`, inside `#[cfg(test)]` blocks. The Constitution text has no test-code exemption; the code is unreachable in production builds. Either move the render-test into `marque-engine` (where internal access is legitimate), or add `#[cfg(test)]` carve-out language to Constitution §V.

### L6 — Stale `lint_10kb` baseline at 285µs upper-CI

`benches/baseline.json`. Reference-machine value not reproduced on WSL2; `bench-check.sh` enforces both relative (+10%) and absolute (16ms) thresholds, so a stale baseline can fail a green build on CI runners that don't match the reference machine. Either re-capture on the same class as `decoder_10kb_one_mangled_region` or relax the relative gate to absolute-only until re-capture lands.

### L7 — `render_audit_error_frame` doc-comment hardcodes `marque-mvp-1`

`marque/src/render.rs:529`. The doc comment shows `Shape: {"schema":"marque-mvp-1",...}`, but the format string emits `{AUDIT_SCHEMA_VERSION}` dynamically. Update to `Shape: {"schema":"<AUDIT_SCHEMA_VERSION>",...}`.

## False Positives (recorded for audit trail)

### FP1 — Agent D: priors.json fingerprint has wrong hex length (HIGH)

**Verified by overseer**: the fingerprint in `crates/capco/corpus/priors.json` is `sha512:` + exactly 128 hex digits, all valid hex. Matches `validate_corpus_fingerprint` in `build.rs:309-330` (`HEX_LEN = 128`). Build is green for the documented reason. Agent D miscounted.

## Verification Results

| Check | Command | Result |
|-------|---------|--------|
| Workspace compile | `cargo check --workspace` | PASS |
| Lint (zero warnings) | `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| Default-features tests | `cargo nextest run --workspace --profile ci` | 1152/1152 PASS |
| `decoder-harness` suite | `cargo test -p marque-engine --features decoder-harness --test decoder_accuracy` | 3 passed + 1 ignored |
| `corpus-override` suite | `cargo test -p marque-engine --features corpus-override --test corpus_override` | 2 passed |
| `corpus-harness` suite | `cargo test -p marque-capco --features corpus-harness --test corpus_parity` | 5 passed |
| WASM forced corpus-override | `cargo build -p marque-wasm --target wasm32-unknown-unknown --features corpus-override` | **FAIL as expected** ("does not contain this feature") |
| WASM normal build | `cargo build -p marque-wasm --target wasm32-unknown-unknown` | PASS |

## Contract Conformance

| Contract | Status |
|----------|--------|
| `audit-record-v2.md` Confidence shape (5 fields, f32) | CONFORMS |
| `audit-record-v2.md` v1→v2 superset | CONFORMS (`v1_records_parse_in_v2_consumer` exercises 12-field shape) |
| `audit-record-v2.md` v2 emitter reads top-level snapshot | **VIOLATES (H1)** |
| `audit-record-v2.md` single schema per build | CONFORMS (build.rs panics on unknown schema) |
| `recognizer-trait.md` `Parsed::Ambiguous{candidates: vec![]}` zero-candidate signal | CONFORMS |
| `recognizer-trait.md` `ParseContext` fields | CONFORMS |
| `recognizer-trait.md` `Send + Sync` supertrait | CONFORMS (implicit; M2 covers the missing explicit assertion) |
| `cli-server-wasm-gates.md` corpus-override 3-channel rejection | CONFORMS (M10 is integration-coverage gap) |
| `cli-server-wasm-gates.md` WASM compile-time exclusion | CONFORMS (verified by force-fail) |

## Constitution Conformance

| Principle | Status |
|-----------|--------|
| V (Audit-First) — `__engine_promote` engine-only | CONFORMS in production (test-only exceptions documented; L5) |
| V (Audit-First) — `FixProposal` is pure data | CONFORMS |
| V (G13) — content-ignorance sentinel sweep | CONFORMS (non-vacuous; `#[should_panic]` self-test confirms load-bearing) |
| VI (Send+Sync) — Recognizer + Rule supertraits | CONFORMS (M2: explicit assertion missing) |
| VI (Send+Sync) — no hidden interior mutability in recognizer family | CONFORMS |
| VII (Crate graph) — `marque-capco` no `marque-core`/`marque-engine` runtime dep | CONFORMS |
| VII (Crate graph) — `marque-scheme` no domain-crate dep | CONFORMS |
| VII (Crate graph) — `marque-wasm` no `marque-extract` | CONFORMS |
| VIII (Citation fidelity) — `DECODER_CITATION` §A.6 p15 traces | CONFORMS |
| VIII (Citation fidelity) — `SUPERSEDED_TOKEN_MAP` §A.6 p16 traces | **VIOLATES (H4)** — should be §H.4 p74 |

## Acknowledged Gaps (not blocking)

These are documented in `tasks.md` and tracked separately; no action needed in this review:

- **T057**: decoder accuracy at ~53% aggregate vs SC-004 ≥85% target. Tracked in GH issue #133. The harness ships `#[ignore]`-marked SC-004 gate plus an always-on regression floor at 50%. (M1 refines per-class detection.)
- **T069**: `CorpusOverrideInEffect` feature contribution carries `delta=0.0` (audit-trail-only). Substituting override priors into decoder scoring requires a coordinated `MARQUE_AUDIT_SCHEMA` bump and is deferred to a follow-up PR.

## Required Actions Before Merge

1. **H1**: Two one-line edits in `marque/src/render.rs` (lines 466, 470) to read top-level fields.
2. **H2**: Delete `feature_label()` in `decoder.rs:409-418`; change call at line 351 to `f.id.as_str()`.
3. **H3**: Add CI step that explicitly runs `decoder-harness`, `corpus-override`, and `corpus-harness` suites.
4. **H4**: Fix citation comment at `decoder.rs:765-766` from `§A.6 p16` to `§H.4 p74`.

## Recommended Follow-ups (post-merge)

- M1, M3, M4, M5 (Rust hygiene): can ride a single follow-up PR; mechanical changes.
- M2 (`static_assertions` adoption): single dev-dep + two assertions.
- M6, M8 (priors validation policy decisions): may require discussion before code.
- M7 (server two-pass body check): refactor or doc clarification.
- M9, M10, M11 (test-coverage refinements): can land incrementally.

## Reviewer Notes

The Phase 4 implementation surface is large (decoder + recognizer + audit v2 + 3-channel corpus-override threat boundary + 6-class fixture suite + 7 contract files) and the review surface that came back was correspondingly broad. What did NOT surface in any agent: any production-runtime correctness bug, any security vulnerability that survives the existing tests, any constitution-VII crate-graph violation, any audit-content leak, any place where the threat model's three-channel rejection actually permits a corpus-override through. The defects that did surface are concentrated in two patterns: (1) docs/code drift where the code is accidentally correct, and (2) CI invocation gaps where excellent tests exist but aren't run.

The path to APPROVE is short — the four HIGH findings are all small, mechanical fixes — but they should land before merge because each represents a regression hazard that compounds (H3 + M1 together would let H1 and H2 land silently if they regressed to incorrect runtime values).
