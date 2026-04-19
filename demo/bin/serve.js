#!/usr/bin/env node

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: Apache-2.0

/**
 * marque-demo — static dev server
 *
 * Usage:
 *   npx @marque/marque-demo [--port <n>] [--no-open]
 *
 * Serves the Marque interactive demo on localhost. Automatically opens the
 * browser unless --no-open is passed.
 *
 * WASM routing:
 *   /wasm/* is resolved in priority order:
 *     1. <demo-root>/wasm/          (pre-built, ships inside the npm package)
 *     2. <repo-root>/crates/wasm/pkg/  (local monorepo dev build)
 *
 * No external npm dependencies — uses only Node.js built-ins.
 */

'use strict';

const http = require('http');
const fs   = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// ---------------------------------------------------------------------------
// Parse argv
// ---------------------------------------------------------------------------

const argv = process.argv.slice(2);
let port = 4242;
let openBrowser = true;

for (let i = 0; i < argv.length; i++) {
  if (argv[i] === '--port' && argv[i + 1]) {
    port = parseInt(argv[++i], 10);
  } else if (argv[i] === '--no-open') {
    openBrowser = false;
  }
}

// ---------------------------------------------------------------------------
// Resolve roots
// ---------------------------------------------------------------------------

// This file lives at <demo-root>/bin/serve.js
const DEMO_ROOT = path.resolve(__dirname, '..');

// When installed via npm, WASM lives in <demo-root>/wasm/
const BUNDLED_WASM_ROOT = path.join(DEMO_ROOT, 'wasm');

// In the monorepo dev tree, WASM lives two levels up in the rust workspace.
const MONOREPO_WASM_ROOT = path.resolve(DEMO_ROOT, '..', 'crates', 'wasm', 'pkg');

function resolveWasmRoot() {
  if (fs.existsSync(path.join(BUNDLED_WASM_ROOT, 'marque_wasm.js'))) {
    return BUNDLED_WASM_ROOT;
  }
  if (fs.existsSync(path.join(MONOREPO_WASM_ROOT, 'marque_wasm.js'))) {
    return MONOREPO_WASM_ROOT;
  }
  return null;
}

// ---------------------------------------------------------------------------
// MIME types
// ---------------------------------------------------------------------------

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.css':  'text/css; charset=utf-8',
  '.js':   'application/javascript; charset=utf-8',
  '.mjs':  'application/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json; charset=utf-8',
  '.map':  'application/json; charset=utf-8',
  '.ts':   'application/typescript; charset=utf-8',
  '.ico':  'image/x-icon',
  '.png':  'image/png',
  '.svg':  'image/svg+xml',
};

function mimeFor(filePath) {
  return MIME[path.extname(filePath).toLowerCase()] || 'application/octet-stream';
}

// ---------------------------------------------------------------------------
// Request handler
// ---------------------------------------------------------------------------

const wasmRoot = resolveWasmRoot();

function handleRequest(req, res) {
  const url = req.url.split('?')[0]; // strip query string

  // /wasm/* → serve from WASM root
  if (url.startsWith('/wasm/')) {
    if (!wasmRoot) {
      res.writeHead(503, { 'Content-Type': 'text/plain' });
      res.end(
        'WASM module not found.\n\n' +
        'If running from the marque monorepo, build it first:\n' +
        '  wasm-pack build crates/wasm --target web --profile release-wasm\n\n' +
        'If running from an npm install, the package may be incomplete.\n'
      );
      return;
    }
    const rel = url.slice('/wasm/'.length);
    serveFile(res, path.join(wasmRoot, rel));
    return;
  }

  // / → index.html
  const filePath = url === '/' ? '/index.html' : url;
  serveFile(res, path.join(DEMO_ROOT, filePath));
}

function serveFile(res, absPath) {
  // Basic path traversal guard
  if (!absPath.startsWith(DEMO_ROOT) && !(wasmRoot && absPath.startsWith(wasmRoot))) {
    res.writeHead(403, { 'Content-Type': 'text/plain' });
    res.end('403 Forbidden');
    return;
  }

  fs.readFile(absPath, (err, data) => {
    if (err) {
      if (err.code === 'ENOENT') {
        res.writeHead(404, { 'Content-Type': 'text/plain' });
        res.end(`404 Not Found: ${absPath}`);
      } else {
        res.writeHead(500, { 'Content-Type': 'text/plain' });
        res.end(`500 Internal Server Error: ${err.message}`);
      }
      return;
    }

    res.writeHead(200, {
      'Content-Type': mimeFor(absPath),
      // Allow SharedArrayBuffer (needed by some WASM builds)
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    });
    res.end(data);
  });
}

// ---------------------------------------------------------------------------
// Start server
// ---------------------------------------------------------------------------

const server = http.createServer(handleRequest);

server.listen(port, '127.0.0.1', () => {
  const url = `http://localhost:${port}`;

  console.log('');
  console.log('  \x1b[1m\x1b[34mmar\x1b[33mque\x1b[0m demo');
  console.log(`  \x1b[2m→\x1b[0m \x1b[4m${url}\x1b[0m`);
  console.log('');

  if (!wasmRoot) {
    console.warn('  \x1b[33m⚠\x1b[0m  WASM module not found — lint/fix features will be unavailable.');
    console.warn('     Build it with:');
    console.warn('       wasm-pack build crates/wasm --target web --profile release-wasm');
    console.warn('');
  }

  if (openBrowser) {
    const target = url;
    try {
      const platform = process.platform;
      if (platform === 'darwin') execSync(`open "${target}"`);
      else if (platform === 'win32') execSync(`start "" "${target}"`);
      else execSync(`xdg-open "${target}"`);
    } catch {
      // Best-effort; ignore errors (e.g. headless CI environments)
    }
  }
});

server.on('error', err => {
  if (err.code === 'EADDRINUSE') {
    console.error(`\x1b[31mError:\x1b[0m Port ${port} is already in use.`);
    console.error(`Try a different port: marque-demo --port ${port + 1}`);
  } else {
    console.error(`\x1b[31mServer error:\x1b[0m ${err.message}`);
  }
  process.exit(1);
});
