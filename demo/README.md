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
wasm-pack build crates/wasm --target web --profile release
```

Enable the panic hook for better WASM error messages in the browser console:

```sh
wasm-pack build crates/wasm --target web --dev -- --features console_error_panic_hook
```

## What the Demo Shows

| Behavior | How |
|-----------|-----|
| Banner auto-updates as you type | `compute_banner()` after each fix pass (80ms debounce) |
| Typo correction | `fix()` with corrections map — e.g., SERCET → SECRET → S |
| Abbreviation enforcement | E009: SECRET → S, NOFORN → NF in portion markings |
| Squiggly underlines | Remaining diagnostics rendered as CodeMirror `Decoration.mark` |
| Hover tooltips | CodeMirror `hoverTooltip` showing rule ID, message, citation |
| Inline audit log | Each applied fix produces a timestamped audit entry |
| Idle autoplay | After ~6s of no input, a scripted typing sequence runs. Pauses on any keystroke, paste, or `beforeinput` event |

## Architecture

The Marque WASM engine runs in a Web Worker (`worker.js`). The main thread
posts `{type:'fix', text, threshold, config}` after debounce; the worker
returns `{fixedText, banner, applied[], remaining[]}`. The worker model
mirrors the deployment shape `marque-wasm` is built for and keeps typing
latency at frame rate even on large inputs. Stale results are dropped via
sequence numbers — only the most recent request's reply is applied.

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
