# marque-wasm

WASM build target for marque — browser and web-worker integration.

Compiles the marque rule engine to WebAssembly via `wasm-pack` and exposes a small JavaScript surface for linting and fixing pre-extracted text. NDJSON output is byte-identical to the native CLI's `--format json` output (SC-008 parity, enforced by test).

## Role in Marque

A deployment target. Wraps `marque-engine` + `marque-capco` + `marque-config` and exports them through `wasm-bindgen`. Format extraction is **the caller's responsibility** in the WASM context — `marque-extract` is intentionally excluded from this build because Kreuzberg is not WASM-portable. Run extraction in the host environment (browser, Node, worker) and feed text in.

Use a web worker to avoid blocking the main thread. The artifact is ~252KB (unoptimized; `wasm-opt` disabled pending an upstream fix for `memory.copy` compatibility).

## Building

```bash
wasm-pack build crates/marque-wasm --target web --profile release-wasm
```

Targets `bundler` and `nodejs` work the same way. Output lands in `crates/marque-wasm/pkg/`.

## Exported Functions

| Function | Returns | Purpose |
|----------|---------|---------|
| `lint(text, config_json?)` | NDJSON string (one diagnostic per line) | Lint pre-extracted text. |
| `fix(text, threshold, config_json?)` | JSON object: `{ fixed_text, applied[], remaining[] }` | Apply fixes at or above the given confidence threshold. |
| `compute_banner(text)` | JSON | Compute the composite banner from page portions. |
| `generate_cab(...)` | JSON | Generate a Classification Authority Block. |

`lint()` output conforms to `contracts/diagnostic.json`. `fix()` audit records conform to `contracts/audit-record.json`. The CLI parity test catches any divergence between the WASM and native serializations.

## Usage

```javascript
import init, { lint, fix } from "./pkg/marque_wasm.js";

await init();

const text = "(S) Example portion-marked sentence.";
const ndjson = lint(text);
for (const line of ndjson.split("\n").filter(Boolean)) {
  console.log(JSON.parse(line));
}

const result = JSON.parse(fix(text, 0.8));
console.log(result.fixed_text);
```

## Features

| Feature | Effect |
|---------|--------|
| `console_error_panic_hook` | Forwards Rust panics to the browser console. Enable for development builds. |

The `batch` feature flag from `marque-engine` is not exposed in the WASM build — concurrent batch processing requires the native runtime.

## License

Apache-2.0
