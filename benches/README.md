# Benchmarks

Criterion benchmarks for marque performance targets.

The actual benchmark source files live in `crates/marque-engine/benches/`:

- `lint_latency.rs` — SC-001: Engine::lint p95 latency on <= 10KB inputs
- `linear_scaling.rs` — SC-005: linear throughput scaling across input sizes

Run with:

```bash
cargo bench -p marque-engine
```
