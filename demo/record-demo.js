#!/usr/bin/env node

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: Apache-2.0

/**
 * record-demo.js — Playwright demo video producer for marque
 *
 * Records a scripted walkthrough of the Marque interactive demo:
 *   Scene 0  — blank document reveal + engine warm-up
 *   Scene 1  — narrative typing with live fixes and confidence change
 *   Outro    — hold on fixed frame with rotating audit slot
 *
 * (A prior "deprecated-control migration" scene used FOUO→CUI. That
 * migration was removed in Phase E of the recursive-lattice plan —
 * FOUO remains valid in CAPCO ISM and CUI is a separate scheme. A
 * replacement migration demo lands with the future CUI adapter,
 * gated by agency config.)
 *
 * Usage:
 *   node record-demo.js [--port 4343] [--output demo.webm] [--headed]
 *
 * Requirements:
 *   npm install playwright
 *   npx playwright install chromium
 *
 * Optional (for MP4 transcode):
 *   ffmpeg  (apt install ffmpeg / brew install ffmpeg)
 */

'use strict';

const { chromium } = require('playwright');
const http  = require('http');
const fs    = require('fs');
const path  = require('path');
const { spawn, execSync } = require('child_process');

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

const argv = process.argv.slice(2);
let port    = 4343;
let outFile = path.resolve(__dirname, 'demo.webm');
let headed  = false;

for (let i = 0; i < argv.length; i++) {
  if (argv[i] === '--port'   && argv[i + 1]) port    = parseInt(argv[++i], 10);
  if (argv[i] === '--output' && argv[i + 1]) outFile = path.resolve(argv[++i]);
  if (argv[i] === '--headed') headed = true;
}

const BASE_URL = `http://localhost:${port}`;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function waitForFix()

function waitForServer(url, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    function attempt() {
      http.get(url, res => { res.resume(); resolve(); })
        .on('error', () => {
          if (Date.now() > deadline) reject(new Error(`Server at ${url} did not start`));
          else setTimeout(attempt, 200);
        });
    }
    attempt();
  });
}

const hold = (page, ms) => page.waitForTimeout(ms);

/** Jitter ±40 % around a base delay (keeps typing looking human). */
const jitter = base => Math.max(base * 0.4 + Math.random() * base * 0.8, 80);

const STYLE_MARKERS = Object.freeze({
  emphasis: { prefix: '[[em]]', suffix: '[[/em]]' },
});

/**
 * Type text into the CodeMirror editor character by character.
 * Handles special chars like newlines via keyboard.press.
 */
async function type(page, text, { charMs = Math.min(Math.random() * 100, 20) } = {}) {
  for (const ch of text) {
    if (ch === '\n') {
      await page.keyboard.press('Enter');
    } else {
      await page.keyboard.type(ch);
    }
    await page.waitForTimeout(jitter(charMs));
  }
}

function encodeStyledSegment(segment) {
  const text = segment?.text ?? '';
  if (!text) return '';
  const marker = STYLE_MARKERS[segment.style];
  if (!marker) return text;
  return `${marker.prefix}${text}${marker.suffix}`;
}

/** Focus the editor and place cursor at the end of document. */
async function focusEnd(page) {
  await page.locator('.cm-content').click();
  await page.keyboard.press('Control+End');
  await page.waitForTimeout(125);
}


/**
 * Type text with lightweight display styling hints.
 *
 * The editor is plain text, so styling is expressed as markers that the demo
 * page decorates visually. For now, `emphasis` maps to `*text*`.
 */
async function typeSegments(page, segments, opts = {}) {
  for (const segment of segments) {
    if (!segment || !segment.text) continue;
    if (segment.text.endsWith(') ')) {
      // After a portion marking, we pause briefly to let the debouncing drop and the fix to apply
      // Note: We're not gaming the portion identification here -- the engine knows it's a portion in under a millisecond.
      // But it doesn't get the information until the debounced change event fires, which is currently set to 50ms.
      // In the future, we could add a non-debounced event for "portion complete" to eliminate this artificial pause in the demo.
      opts = { ...opts, charMs: 50 };
    }
    const text = encodeStyledSegment(segment);
    await type(page, text, {
      ...opts,
      ...(typeof segment.charMs === 'number' ? { charMs: segment.charMs } : {}),
    });
    if (typeof segment.pauseAfterMs === 'number' && segment.pauseAfterMs > 0) {
      await hold(page, segment.pauseAfterMs);
    }
  }
}

async function typeLine(page, segments, opts = {}) {
  const list = Array.isArray(segments) ? segments : [{ text: String(segments ?? '') }];
  await typeSegments(page, list, opts);
  await page.keyboard.press('Enter');
  await page.keyboard.press('Enter');
}

// ---------------------------------------------------------------------------
// Scene runners
// ---------------------------------------------------------------------------

/**
 * Scene 0 — Blank document reveal.
 * Navigate, wait for WASM, clear any seed content, hold on empty state.
 */
async function scene0_blank(page) {
  console.log('  Scene 0: blank document reveal');
  await page.goto(BASE_URL);

  // Wait for CodeMirror and WASM engine
  await page.waitForSelector('.cm-content', { timeout: 10_000 });
  await hold(page, 2000); // WASM init + configure()

  // Clear any seed content so we start from a clean slate
  await page.locator('.cm-content').click();
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Delete');
  await hold(page, 300);

  // Hold on the blank document — UNCLASSIFIED banners visible
  await hold(page, 800);
}

/**
 * Scene 1 — Narrative
 */
async function scene1(page) {
  console.log('  Scene 1');
  await focusEnd(page);

  await typeLine(page, [
    { text: '(U) ' , style: 'bold' },
    { text: 'Nothing here is classified. We\'ll use some classified markings to introduce you to ' },
    { text: 'Marque' },
    { text: '. ' },
  ]);

  await typeLine(page, [
    { text: '(u//Fouo) ' , style: 'bold' },
    { text: 'If you\'ve ever had to deal with markings, you know how complex they can be. Lots of rules, special cases. Existing tools slow you down. Taking ' },
    { text: '10+ minutes', style: 'emphasis' },
    { text: ' to mark a document is common.' },
  ]);

  await typeLine(page, [
    { text: '(s//REL TO fvey, CAN, FRA, Ita) ' , style: 'bold' },
    { text: 'That ends now.' },
  ]);

  await typeLine(page, [
    { text: '(s//REL TO naTO) ' , style: 'bold' },
    { text: 'Ultra-fast. On a mid-level laptop, Marque can fix about 8 ' },
    { text: 'million', style: 'emphasis' },
    { text: ' markings per ' },
    { text: 'second', style: 'emphasis' },
    { text: '. ' },
  ]);

  const slider = page.locator('#threshold-slider');
  await typeLine(page, [
    { text: '(ts//LES//RD//SI//ORCON//IMCON//NOFORN) ' , style: 'bold' },
    { text: 'Every decision has an empirical ' },
    { text: 'confidence score', style: 'emphasis' },
    { text: '. Like here. This generates an ' },
    { text: 'error', style: 'emphasis' },
    { text: ' but Marque can fix it with lower confidence.' },
  ]);

  await hold(page, 500);
  await typeLine(page, ' Let\'s dial down the confidence');

  await slider.hover();
  await slider.evaluate((el, value) => {
    el.value = String(value);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }, 0.5);
  await hold(page, 200);
  await typeLine(page, [
    { text: '(U//xd) ' , style: 'bold' },
    { text: 'Every fix is ' },
    { text: 'explainable', style: 'emphasis' },
    { text: ' and ' },
    { text: 'auditable', style: 'emphasis' },
    { text: '.' },
  ]);
}

/**
 * Outro — fixed-frame hold (no page scrolling in the recording).
 */
async function outro(page) {
  console.log('  Outro: hold on fixed frame');
  await hold(page, 1400);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

(async () => {
  // ── Start the demo server ──────────────────────────────────────────────────
  console.log(`\nStarting demo server on port ${port}…`);
  const serverProc = spawn(
    process.execPath,
    [path.join(__dirname, 'bin', 'serve.js'), '--port', String(port), '--no-open'],
    { stdio: ['ignore', 'pipe', 'pipe'] }
  );
  serverProc.stdout.on('data', d => process.stdout.write('  [server] ' + d));
  serverProc.stderr.on('data', d => process.stderr.write('  [server] ' + d));

  try {
    await waitForServer(BASE_URL);
    console.log('Server ready.\n');

    // ── Launch browser with video recording ────────────────────────────────
    const videoDir = path.join(__dirname, '.video-tmp');
    fs.mkdirSync(videoDir, { recursive: true });

    const browser = await chromium.launch({
      headless: !headed,
      args: ['--no-sandbox'],
    });

    const context = await browser.newContext({
      viewport:        { width: 1280, height: 800 },
      deviceScaleFactor: 2,       // HiDPI — crisp text in the recording
      recordVideo: {
        dir:  videoDir,
        size: { width: 1280, height: 800 },
      },
    });

    const page = await context.newPage();

    // ── Run the script ─────────────────────────────────────────────────────
    console.log('Recording…\n');
    await scene0_blank(page);
    await scene1(page);
    await outro(page);

    // Final hold on the completed document
    await hold(page, 2500);

    // ── Save video ─────────────────────────────────────────────────────────
    const videoPath = await page.video()?.path();
    await context.close();   // finalizes the .webm
    await browser.close();

    if (videoPath && fs.existsSync(videoPath)) {
      fs.renameSync(videoPath, outFile);
      console.log(`\nVideo saved: ${outFile}`);

      const mp4Out = outFile.replace(/\.webm$/i, '.mp4');
      try {
        execSync(
          `ffmpeg -y -loglevel error -i "${outFile}" ` +
          `-c:v libx264 -preset fast -crf 18 -movflags +faststart "${mp4Out}"`,
          { stdio: 'pipe' }
        );
        console.log(`MP4 saved:   ${mp4Out}`);
      } catch {
        console.log('(ffmpeg not available — skipping MP4 transcode)');
      }
    } else {
      console.error('Video file not found after recording.');
      process.exitCode = 1;
    }

  } finally {
    serverProc.kill();
    fs.rmSync(path.join(__dirname, '.video-tmp'), { recursive: true, force: true });
  }
})().catch(err => {
  console.error(err);
  process.exit(1);
});
