//! SC-001 benchmark: Engine::lint p95 latency on representative inputs.
//!
//! Target: <= 16ms p95 on inputs <= 10KB of raw text on commodity dev hardware.
//!
//! Reference baseline: x86_64 >= 3.0 GHz single-thread (e.g. modern laptop-class CPU),
//! warm cache, `--release` build, no tracing subscriber.

use criterion::{Criterion, criterion_group, criterion_main};

fn lint_latency_benchmark(_c: &mut Criterion) {
    // TODO: Implement once Engine::lint is wired with real rules.
    //
    // Steps:
    // 1. Load a ~10KB representative input from tests/corpus/
    // 2. Create a default Config and Engine with capco_rules()
    // 3. Benchmark Engine::lint(input)
    // 4. Assert p95 <= 16ms (via criterion's built-in stats)
}

criterion_group!(benches, lint_latency_benchmark);
criterion_main!(benches);
