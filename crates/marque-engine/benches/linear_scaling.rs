//! SC-005 benchmark: linear throughput scaling across input sizes.
//!
//! Sweeps input size across at least one order of magnitude (1KB -> 100KB)
//! and asserts throughput stays linear with no super-linear growth.

use criterion::{Criterion, criterion_group, criterion_main};

fn linear_scaling_benchmark(_c: &mut Criterion) {
    // TODO: Implement once Engine::lint is wired with real rules.
    //
    // Steps:
    // 1. Generate inputs at 1KB, 2KB, 5KB, 10KB, 20KB, 50KB, 100KB
    //    by repeating a representative marking block.
    // 2. For each size, benchmark Engine::lint(input).
    // 3. Post-process: verify throughput (bytes/sec) does not degrade
    //    super-linearly as input size grows.
    let _sizes = [1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000];
}

criterion_group!(benches, linear_scaling_benchmark);
criterion_main!(benches);
