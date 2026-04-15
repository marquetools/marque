# marque-engine

Pipeline orchestration for Marque — wires the scanner, parser, and rule sets into a configured, ready-to-run engine.

`marque-engine` is the middle of the pipeline. It owns the `Engine` (single
document) and `BatchEngine` (concurrent, async) APIs that the CLI, the WASM
build, and the server all sit on top of.

## Role in Marque

```
marque-config ─┐
marque-core ───┼─→ marque-engine ─→ LintResult / FixResult + audit log
marque-rules ──┘
```

The engine drives the scanner, parses each candidate, evaluates every
registered rule with a `RuleContext`, and — for `fix` mode — promotes
`FixProposal`s to `AppliedFix` records when their confidence meets the
configured threshold.

## Usage

Single-document lint:

```rust
use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::Engine;

let engine = Engine::new(Config::default(), vec![Box::new(capco_rules())]);
let result = engine.lint(b"(S) example text");

println!("errors: {}, warnings: {}", result.error_count(), result.warn_count());
```

Apply fixes with audit records:

```rust
use marque_engine::{Engine, FixMode};
# use marque_capco::capco_rules;
# use marque_config::Config;
# let engine = Engine::new(Config::default(), vec![Box::new(capco_rules())]);

let result = engine.fix_with_threshold(b"(S) example", FixMode::Apply, None)?;
for applied in &result.applied {
    // serialize `applied` to the NDJSON audit stream
}
# Ok::<(), marque_engine::InvalidThreshold>(())
```

## Behavior

- **Confidence gate.** Only `FixProposal`s whose `confidence` is at or above
  the configured threshold are promoted. Proposals below the gate are
  surfaced as suggestions in `LintResult`, never written.
- **Reverse-span application.** Fixes are sorted and applied from the end of
  the document to the start, so earlier spans remain valid as edits land.
  Overlapping fixes are rejected per the C-1 invariant.
- **Page context.** The engine builds a `marque_ism::PageContext` from
  preceding portion markings and hands an `Arc<PageContext>` to banner/CAB
  rules through `RuleContext`. The accumulator resets at scanner-emitted
  `MarkingType::PageBreak` candidates (form-feed `\f` and `\n\n\n+`),
  *before* attempting to parse the page-break candidate, so a malformed
  candidate cannot block the reset.
- **Audit log.** `AppliedFix` is constructed exclusively by the engine via
  the `__engine_promote` constructor. Rule crates must not bypass this —
  see the architectural-invariants section of the workspace
  [`CLAUDE.md`](../../CLAUDE.md).
- **Clock injection.** `Engine::with_clock` accepts a `Clock` impl; a
  `FixedClock` is provided for deterministic test snapshots and for the CLI
  `--fixed-timestamp` debug flag.

## Batch processing

With the `batch` feature, `BatchEngine` wraps `Engine` behind `Arc` and uses
`recoco-utils` semaphores for row + byte backpressure. CPU-bound work runs
on `tokio::task::spawn_blocking`. Results stream out in **completion order**,
not submission order — correlate by the echoed `id`.

## Features

| Feature | Default | Effect |
|---|---|---|
| `batch` | on | Enables `BatchEngine` and pulls in `tokio`, `futures`, and `recoco-utils`. Disable for a leaner sync-only build (e.g. WASM). |

A `cache` feature (LMDB-backed incremental cache, opt-in) is planned for v0.2
but is not yet present in `Cargo.toml`.

## WASM compatibility

The sync `Engine` path is WASM-safe; build with `default-features = false` to
drop the async/batch dependencies. `marque-wasm` consumes the engine this way.

## License

Apache-2.0.
