<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque-utils

**Common utilities for the [Marque](https://github.com/marquetools/marque) ecosystem.**

Shared building blocks used across Marque's engine and integration crates.
Originally adapted from Recoco's utility crate; trimmed to the surface Marque
actually needs.

## Installation

```toml
[dependencies]
marque-utils = { version = "0.1", features = ["concur_control"] }
```

## Available Features

No default features — enable only what you need. Every feature is independent;
none implies another.

| Feature | Description | Key dependencies |
|---------|-------------|------------------|
| `batching` | Async micro-batcher that coalesces concurrent single-item calls into batched `Runner` invocations | `async-trait`, `serde`, `tokio`, `tokio-util` |
| `bytes_decode` | BOM-sniffing byte-buffer → string decode (UTF-8/16) | `encoding_rs` |
| `concur_control` | Row + byte semaphore backpressure (`ConcurrencyController`) | `tokio` |
| `fingerprint` | 128-bit truncated-BLAKE3 content/value fingerprint with a serde `Serializer` for structural hashing | `base64`, `blake3`, `serde` |
| `retryable` | Retry-with-exponential-backoff combinator | `fastrand`, `tokio` |

The `error` module and `prelude` are always available (`anyhow` + `tracing`),
regardless of features.

## Key modules & usage

### Concurrency control

Bound in-flight work by row count and/or byte volume — backs `BatchEngine`:

```rust
use marque_utils::concur_control::{ConcurrencyController, Options};

let controller = ConcurrencyController::new(&Options {
    max_inflight_rows: Some(16),
    max_inflight_bytes: Some(256 * 1024 * 1024),
});

let _permit = controller.acquire(Some(|| doc.len())).await?;
// work proceeds while the permit is held; released on drop
```

### Batching

Coalesce concurrent calls into batched `Runner` invocations:

```rust
use marque_utils::batching::{Batcher, BatchingOptions, Runner};

let batcher = Batcher::new(runner, BatchingOptions { max_batch_size: Some(64) });
let output = batcher.run(input).await?;
```

### Fingerprinting

Deterministic content/value identity for dedup and cache keys. The
`Fingerprinter` is a serde `Serializer`, so it fingerprints any `Serialize`
value structurally — not just raw bytes:

```rust
use marque_utils::fingerprint::Fingerprinter;

let fp = Fingerprinter::default().with(&value)?.into_fingerprint();
let key = fp.to_base64();
```

This is distinct from the engine's audit digest, which stays a full-width
`blake3::Hash` (`blake3:<hex>`) for compliance.

### Retry

Exponential backoff for retryable operations:

```rust
use marque_utils::retryable::{run, RetryOptions};

let result = run(|| async { fallible().await }, &RetryOptions::default()).await?;
```

### Byte decoding

```rust
use marque_utils::bytes_decode::bytes_to_string;

let (text, had_errors) = bytes_to_string(raw_bytes);
```

## Development

Part of the Marque workspace. See the [main repository](https://github.com/marquetools/marque)
for development guidelines.

## License

Marque License 1.0. See the [main repository](https://github.com/marquetools/marque) for details.
