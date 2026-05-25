<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque-utils

**Common utilities for the [Marque](https://github.com/marquetools/marque) ecosystem.**

This crate provides shared building blocks used across Recoco's core and operation modules. While Recoco primarily uses this crate internally, these utilities can be useful for developing custom operations or for standalone use in Rust projects.

## Installation

```toml
[dependencies]
marque-utils = { version = "0.1", features = ["batching", "fingerprint"] }
```

## 📦 Available Features

`marque-utils` is highly modular with **no default features** to keep dependencies minimal. Enable only what you need.

### Core Utilities

| Feature | Description | Key Dependencies | Use When |
|---------|-------------|------------------|----------|
| `batching` | Async batch processing with concurrency control | `tokio-util`, `serde` | Building efficient data pipelines with batch operations |
| `concur_control` | Concurrency limiting and rate control primitives | `tokio` | Managing concurrent operations and backpressure |
| `deserialize` | JSON deserialization helpers with better error messages | `serde`, `serde_json`, `serde_path_to_error` | Parsing JSON with detailed error reporting |
| `fingerprint` | Content hashing (BLAKE3) and fingerprinting | `blake3`, `base64`, `hex` | Change detection, deduplication, caching |
| `retryable` | Exponential backoff retry logic | `tokio`, `rand`, `time` | Network calls, external APIs, unreliable operations |


## 🛠️ Key Modules & Usage

### Batching

Efficient batch processing with concurrency control:

```rust
use marque_utils::batching::{Batcher, BatchConfig};

let config = BatchConfig {
    max_batch_size: 100,
    max_wait_ms: 1000,
    max_inflight: 10,
};

let batcher = Batcher::new(config, |batch| async move {
    // Process batch
    Ok(())
}).await?;

batcher.send(item).await?;
```

### Fingerprinting

Content-addressable hashing with BLAKE3:

```rust
use marque_utils::fingerprint::{fingerprint, Fingerprint};

let hash = fingerprint(b"hello world");
let hex_string = hash.to_hex();
let base64_string = hash.to_base64();
```

### Retry Logic

Exponential backoff for unreliable operations:

```rust
use marque_utils::retryable::{retry_with_backoff, RetryConfig};

let result = retry_with_backoff(
    || async { 
        // Your operation that might fail
        api_call().await
    },
    RetryConfig {
        max_attempts: 5,
        initial_delay_ms: 100,
        max_delay_ms: 10000,
        backoff_multiplier: 2.0,
    }
).await?;
```

### Concurrency Control

Limit concurrent operations:

```rust
use marque_utils::concur_control::Semaphore;

let sem = Semaphore::new(10); // Max 10 concurrent operations

let _permit = sem.acquire().await?;
// Do work while holding permit
// Permit is released when dropped
```

## 📊 Feature Dependencies

Some features depend on others. Most are fully independent, except:

- `batching` requires `concur_control`, `fingerprint`, and `retryable`
- `fingerprint` requires `deserialize`

Enabling a feature automatically enables its dependencies.

## 🎯 Common Feature Combinations

### For Data Processing Pipelines
```toml
marque-utils = { version = "0.2", features = ["batching", "fingerprint", "retryable"] }
```

## 🔧 Development

This crate is part of the Marque workspace. See the [main repository](https://github.com/marquetools/marque) for development guidelines.

## 📄 License

Marque License 1.0. See [main repository](https://github.com/marquetools/marque) for details.
