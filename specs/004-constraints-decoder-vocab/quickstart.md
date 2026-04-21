# Quickstart — Verifying Phases C, D, and E

**Feature:** `004-constraints-decoder-vocab`

Three walk-throughs, one per user story in [spec.md](./spec.md). Each walk-through is a minimal end-to-end test a reviewer can run after the implementation lands to convince themselves the acceptance criteria pass.

## US1 — Scheme authors express rules as data, not code (Phase C)

**Goal:** Prove that the ~15 migrated CAPCO rules produce byte-identical corpus diagnostics before and after the Phase C migration.

### Setup

```bash
git checkout main
cargo test -p marque-capco --features corpus-harness -- --nocapture | tee /tmp/baseline.txt

git checkout 004-constraints-decoder-vocab
cargo test -p marque-capco --features corpus-harness -- --nocapture | tee /tmp/phase-c.txt
```

### Verify

```bash
diff /tmp/baseline.txt /tmp/phase-c.txt
# expect: no differences in the diagnostic summary section
```

### Additional checks

- **Rule count reduced:** `cargo run -p marque -- --list-rules | wc -l` now shows ~24 remaining hand-written rules (was 39; ~15 migrated to Constraint/PageRewrite data).
- **Scheduler determinism:** Swap the declaration order of two `PageRewrite` entries in `crates/capco/src/scheme.rs`, rerun the corpus harness, diagnostics unchanged.
- **Cycle detection:** Temporarily introduce a read/write cycle between two test rewrites in `crates/engine/tests/` and verify `Engine::new` returns `EngineConstructionError::RewriteCycle { axis, members }` where `members` is a slice naming every participating rewrite (cycles ≥3 members are legitimate).

### Acceptance Scenario coverage

- **US1.1 (equivalent diagnostic for declarative constraint):** byte-identical corpus output above.
- **US1.2 (unannotated custom rewrite rejected):** covered by `crates/engine/tests/scheduler.rs::unannotated_custom_rewrite_fails_construction` (lives in `marque-engine` because the scheduler lives in `Engine::new`; `marque-scheme` does not depend on `marque-engine`).
- **US1.3 (cycle rejected):** covered by `crates/engine/tests/scheduler.rs::cyclic_rewrite_pair_fails_construction`.
- **US1.4 (order-independent schedule):** covered by the "Swap the declaration order" step above and `crates/engine/tests/scheduler.rs::scheduled_order_independent_of_declaration`.

## US2 — Compliance staff clean up historical corpora without manual re-marking (Phase D)

**Goal:** Prove the probabilistic recognizer resolves mangled markings with audit provenance.

### Setup

```bash
# Generate the mangled-marking fixture (one-time; requires Enron corpus environment variable)
# tools/corpus-analysis/ is a Python tool; marque consumes its JSON output at build time.
MARQUE_ENRON_CORPUS=/path/to/enron \
  python3 tools/corpus-analysis/analyze.py --mode mangled \
  --output tests/fixtures/mangled/ --min-cases 200

# Verify fixture size
find tests/fixtures/mangled -name '*.json' | wc -l
# expect: ≥ 200
```

### Run the accuracy harness

```bash
cargo test -p marque-capco --features decoder-harness -- \
  --exact test::mangled::resolution_rate_at_0_85
```

Expected output:

```text
test test::mangled::resolution_rate_at_0_85 ... ok
  resolved: 174 / 200 (87%)
  mean_posterior: 0.91
  runner_up_ratio_median: 3.4
```

### Verify SC-004 (≥85% resolution at confidence ≥0.85)

The harness fails if resolution rate drops below 85%. The printed statistics above exceed the threshold.

### Spot-check audit records

```bash
cargo run -p marque -- fix --deep-scan tests/fixtures/mangled/typo/sercet.txt --audit-log /tmp/audit.jsonl
cat /tmp/audit.jsonl | jq '.confidence.features'
# expect: non-empty array of FeatureContribution entries with enum FeatureId values
```

### Latency check (SC-002)

```bash
cargo bench -p marque-engine --bench lint_latency -- \
  decoder_10kb_one_mangled_region
# expect: p95 ≤ 18 ms
```

### Acceptance Scenario coverage

- **US2.1 (typo resolved, audit has provenance):** covered by `tests/fixtures/mangled/typo/` + audit record check above.
- **US2.2 ((C) blocked from copyright when strict CONFIDENTIAL present):** covered by `marque-capco/tests/decoder_strict_context.rs`.
- **US2.3 (banner reordered to canonical):** covered by `tests/fixtures/mangled/reordering/dissem_before_sci.txt`.
- **US2.4 (interactive without opt-in doesn't invoke decoder):** covered by a timed test that runs `marque check` (no flag) on a mangled input, measures latency ≤16 ms, and verifies no decoder features appear in the output.
- **US2.5 (server rejects corpus override):** covered by `marque-server/tests/http.rs::rejects_corpus_override`.
- **US2.6 (WASM has no override surface):** covered by the WASM compile-fail test; `cargo build --target wasm32-unknown-unknown -p marque-wasm` succeeds only when the override codepath is absent.

## US3 — Rules and audit records reference the full authoritative vocabulary (Phase E)

**Goal:** Prove rules can query full ODNI metadata and the FOUO→CUI migration is gone.

### Verify metadata reachability

```bash
cargo test -p marque-capco -- --exact test::vocabulary::every_active_token_has_authority
cargo test -p marque-capco -- --exact test::vocabulary::deprecated_tokens_carry_deprecation
cargo test -p marque-capco -- --exact test::vocabulary::replacement_when_known
```

All three pass.

### Verify FOUO still active

```bash
cargo test -p marque-ism -- --exact test::migrations::fouo_is_not_migrated
cargo test -p marque-capco -- --exact test::fouo_remains_active
```

Then run the existing CAPCO corpus harness:

```bash
cargo test -p marque-capco --features corpus-harness
# expect: all FOUO-bearing documents produce the same diagnostics as pre-Phase-E
```

### Spot-check a vocabulary query at zero allocation

```bash
cargo test -p marque-capco -- --exact test::vocabulary::metadata_query_is_zero_alloc
# implemented via allocation counter in tests
```

### Verify codec trait compiles with no impls

```bash
cargo build -p marque-scheme
# The Codec<S> trait is defined; no concrete impls ship. Phase G will provide them.
```

### Acceptance Scenario coverage

- **US3.1 (vocabulary query returns static data with POC):** covered by `every_active_token_has_authority` + `metadata_query_is_zero_alloc`.
- **US3.2 (migration records both source and replacement URNs):** covered by an audit record fixture test in `marque-engine/tests/audit.rs::migration_audit_has_both_urns`.
- **US3.3 (FOUO in CAPCO context remains active):** covered by the corpus harness equivalence above.
- **US3.4 (Phase G can implement codec without scheme edits):** deferred — documented in `contracts/codec-trait.md` as pending Phase G.

## Running the whole gate

Single command suitable for CI:

```bash
cargo test --workspace --features "corpus-harness,decoder-harness" && \
  cargo bench -p marque-engine && \
  cargo build --target wasm32-unknown-unknown -p marque-wasm --no-default-features
```

All three must succeed for the branch to ship.
