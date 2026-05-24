<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Benchmarks

Criterion benchmarks for marque performance targets.

The actual benchmark source files live in `crates/engine/benches/`:

- `lint_latency.rs` — Engine::lint p95 latency on <= 10KB inputs (interactive-latency gate)
- `linear_scaling.rs` — linear throughput scaling across input sizes

Run with:

```bash
cargo bench -p marque-engine
```
