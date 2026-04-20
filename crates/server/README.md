<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque-server

REST microservice exposing the marque rule engine.

A thin axum-based HTTP wrapper around `marque-engine`. Designed to drop into existing service meshes — request handlers, response shapes, and JSON contracts mirror the CLI and WASM surfaces so a single client implementation works against any deployment target.

## Role in Marque

A deployment target alongside the CLI (`marque`) and WASM build (`marque-wasm`). Pulls in `marque-engine` with the `batch` feature and `marque-extract` so the server can accept either pre-extracted text or supported document bytes.

## Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `GET`  | `/v1/health` | Liveness probe. |
| `GET`  | `/v1/schema/version` | Returns the compiled-in ISM schema version. |
| `POST` | `/v1/lint` | Lint a document; returns diagnostics. |
| `POST` | `/v1/fix`  | Fix a document at or above the configured confidence threshold; returns fixed text + audit log. |

Endpoints documented elsewhere as planned but not yet implemented: `/v1/metadata`, `/v1/batch`. Auth and structured logging middleware via Tower are also planned (the dependencies are wired; no policy is enforced yet).

## Running

```bash
cargo run -p marque-server
# binds 127.0.0.1:3000 by default
```

Override the bind address:

```bash
MARQUE_ADDR=127.0.0.1:8080 cargo run -p marque-server
```

## Environment Variables

| Variable | Effect |
|----------|--------|
| `MARQUE_ADDR` | Bind address. Defaults to `127.0.0.1:3000`. |
| `MARQUE_LOG` | Tracing filter, e.g. `marque=debug` or `marque=trace`. |
| `MARQUE_CLASSIFIER_ID` | Classifier identity injected into audit records emitted by `/v1/fix`. |
| `MARQUE_CONFIDENCE_THRESHOLD` | Minimum fix confidence to auto-apply. |

## Usage

```bash
curl -s http://localhost:3000/v1/lint \
  -H 'content-type: application/json' \
  -d '{"text": "(S) Example sentence."}'
```

```bash
curl -s http://localhost:3000/v1/health
```

## License

Apache-2.0
