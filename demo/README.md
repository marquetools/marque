# Marque Interactive Demo

A single-page office-document demo showing Marque's real-time classification
marking lint and auto-fix capabilities.

## Running the demo

The demo loads the WASM build directly from `crates/marque-wasm/pkg/`. Serve
from the repo root (not the `demo/` directory):

```sh
python3 -m http.server 8080
```

Then open: <http://localhost:8080/demo/>

## Building the WASM (required before first run)

```sh
wasm-pack build crates/marque-wasm --target web --dev
```

For a production-optimized build (requires wasm-opt or `wasm-opt = false` in
`Cargo.toml`):

```sh
wasm-pack build crates/marque-wasm --target web --profile release-wasm
```

## What the demo shows

| Behavior | How |
|-----------|-----|
| Banner auto-updates as you type | `compute_banner()` on every keystroke (50 ms debounce) |
| Typos self-correct | `fix(text, 0.0)` — threshold 0 applies all fixes including corrections map |
| Block reordering | E003 fires at confidence 0.6; threshold 0.0 applies it |
| Squiggly underlines | `lint()` → CodeMirror `Decoration.mark` |
| Hover tooltips | CodeMirror `hoverTooltip` showing rule ID, message, citation |
| CAB generation | `generate_cab()` on button click |
| Playground | Raw NDJSON output from `lint()` |

## Vendor bundle (CodeMirror)

The demo uses a pre-built `vendor.js` bundle containing CodeMirror 6 modules.
This bundle is checked in, so you don't need npm for normal use. To rebuild it
(e.g., after upgrading CodeMirror):

```sh
cd demo
npm install
node bundle-vendor.js
```

The bundle entry point is `demo/bundle-vendor.js` and is built with esbuild.
`vendor.js` is the output artifact — it is committed so the demo works without
any build step after cloning.
