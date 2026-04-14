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

For a production-optimised build (requires wasm-opt or `wasm-opt = false` in
`Cargo.toml`):

```sh
wasm-pack build crates/marque-wasm --target web --profile release-wasm
```

## What the demo shows

| Behaviour | How |
|-----------|-----|
| Banner auto-updates as you type | `compute_banner()` on every keystroke (50 ms debounce) |
| Typos self-correct | `fix(text, 0.0)` — threshold 0 applies all fixes including corrections map |
| Block reordering | E003 fires at confidence 0.6; threshold 0.0 applies it |
| Squiggly underlines | `lint()` → CodeMirror `Decoration.mark` |
| Hover tooltips | CodeMirror `hoverTooltip` showing rule ID, message, citation |
| CAB generation | `generate_cab()` on button click |
| Playground | Raw NDJSON output from `lint()` |

## No build step required for the demo

The demo uses ES module imports from `esm.sh` (CodeMirror 6) and loads the
WASM package directly. No npm, no bundler.
