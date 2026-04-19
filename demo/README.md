<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: Apache-2.0
-->

# Marque Interactive Demo

A single-page office-document demo showing Marque's real-time classification
marking lint and auto-fix capabilities.

## Quick Start

```sh
# 1. Build the WASM module (required before first run)
wasm-pack build crates/wasm --target web --dev

# 2. Install demo dependencies
cd demo && npm install

# 3. Start the dev server
npm start
```

The server opens `http://localhost:4242` with the interactive demo.

## Building the WASM

Development build (fast, no wasm-opt):

```sh
wasm-pack build crates/wasm --target web --dev
```

Production build (optimized):

```sh
wasm-pack build crates/wasm --target web --profile release-wasm
```

Enable the panic hook for better WASM error messages in the browser console:

```sh
wasm-pack build crates/wasm --target web --dev -- --features console_error_panic_hook
```

## What the Demo Shows

| Behavior | How |
|-----------|-----|
| Banner auto-updates as you type | `compute_banner()` on every keystroke (80ms debounce) |
| Typo correction | `fix()` with corrections map — e.g., SERCET → SECRET → S |
| Abbreviation enforcement | E009: SECRET → S, NOFORN → NF in portion markings |
| Deprecated control migration | E006: FOUO → CUI per CAPCO migration table |
| Squiggly underlines | `lint()` → CodeMirror `Decoration.mark` for remaining diagnostics |
| Hover tooltips | CodeMirror `hoverTooltip` showing rule ID, message, citation |
| Inline audit log | Each applied fix produces a timestamped audit entry |

## Recording a Demo Video

```sh
# Install Playwright browser (one time)
npx playwright install chromium

# Record the scripted walkthrough
npm run record
```

Output: `demo.webm` (+ `demo.mp4` if ffmpeg is available).

## Vendor Bundle (CodeMirror)

The demo uses a pre-built `vendor.js` bundle containing CodeMirror 6 modules.
This bundle is checked in, so you don't need npm for normal use. To rebuild it
(e.g., after upgrading CodeMirror):

```sh
cd demo
npm install
node bundle-vendor.js
```
