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
 *   node record-demo.js [--port 4343] [--output demo.webm] [--headed] [--no-video] [--debug-timing]
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
let noVideo = false;
let debugTiming = false;

for (let i = 0; i < argv.length; i++) {
  if (argv[i] === '--port'   && argv[i + 1]) port    = parseInt(argv[++i], 10);
  if (argv[i] === '--output' && argv[i + 1]) outFile = path.resolve(argv[++i]);
  if (argv[i] === '--headed') headed = true;
  if (argv[i] === '--no-video') noVideo = true;
  if (argv[i] === '--debug-timing') debugTiming = true;
}

const BASE_URL = `http://localhost:${port}`;
const EDITOR_CONTENT_SELECTOR = '.cm-content';

function getRunUrl() {
  if (!debugTiming) return BASE_URL;
  const url = new URL(BASE_URL);
  url.searchParams.set('debug_timing', '1');
  return url.toString();
}

function debugLog(label, details = '') {
  if (!debugTiming) return;
  const suffix = details ? ` ${details}` : '';
  console.log(`  [timing] ${label}${suffix}`);
}

// ---------------------------------------------------------------------------
// Timing — single source of truth for all delays in the recording.
// Adjust here only; nothing else in this file contains timing literals.
// ---------------------------------------------------------------------------

const TIMING = {
  // Per-character typing speed
  charMs:          45,
  // Hold after typing a portion mark — lets the engine fix land visually.
  // No DOM polling; just a flat pause so recording never stalls.
  portionPauseMs:  275,
  // Brief settle between non-portion segments
  segmentPauseMs:   50,
  // Extra hold after '. ' / '! ' / '? ' — sentence rhythm
  sentencePauseMs: 300,
  // Extra hold after ', ' or '; '
  commaPauseMs:    125,
  // Pause after clicking the editor to focus it
  focusSettleMs:   180,
  // How long to wait for the WASM engine to warm up after page load
  wasmWarmupMs:    10000,
  // Short settle after clearing the editor
  clearSettleMs:   300,
  // Hold on blank document so the UNCLASSIFIED banner registers
  blankHoldMs:     600,
  // Pause before moving the confidence slider
  preSliderMs:     1000,
  // Settle after slider change before continuing to type
  postSliderMs:    500,
  // Outro hold on the completed document
  outroMs:         1400,
  // Final hold after outro before saving video
  finalHoldMs:     500,
};


// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

const STYLE_MARKERS = Object.freeze({
  emphasis: { prefix: '[[em]]', suffix: '[[/em]]' },
});

async function waitForRecorderApi(page) {
  await page.waitForFunction(() => !!window.__marqueDemoRecorder?.ready, { timeout: 10_000 });
}

/**
 * Type text through the page-side recorder API so cadence runs entirely
 * inside the browser event loop (no per-character Playwright IPC).
 */
async function type(page, text, { charMs = TIMING.charMs } = {}) {
  // Fire the in-browser typing animation (returns immediately — no IPC hold).
  await page.evaluate(
    ({ text, timing }) => {
      window.__marqueDemoRecorder.appendText(text, timing);
    },
    {
      text,
      timing: {
        charMs,
        sentencePauseMs: TIMING.sentencePauseMs,
        commaPauseMs: TIMING.commaPauseMs,
      },
    }
  );
  // Poll for the page-side animation to finish. Single IPC per keystroke
  // batch instead of one round-trip per character.
  await page.waitForFunction(
    () => !window.__marqueDemoRecorder.busy,
    { timeout: 60_000 },
  );
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
  // Keep a tiny settle for visual continuity; insertion always appends.
  await page.waitForTimeout(TIMING.focusSettleMs);
}


/**
 * Type text with lightweight display styling hints.
 *
 * The editor is plain text, so styling is expressed as markers that the demo
 * page decorates visually. For now, `emphasis` maps to `[[em]]text[[/em]]`.
 */
async function typeSegments(page, segments, opts = {}) {
  for (const segment of segments) {
    if (!segment || !segment.text) continue;
    const text = encodeStyledSegment(segment);
    const segmentCharMs = typeof segment.charMs === 'number'
      ? segment.charMs
      : (typeof opts.charMs === 'number' ? opts.charMs : TIMING.charMs);
    await type(page, text, { ...opts, charMs: segmentCharMs });
    // After a portion mark, hold so the engine fix can land and the viewer
    // can register the change. Flat timeout — no DOM polling, no IPC.
    if (segment.text.endsWith(') ')) {
      await hold(page, TIMING.portionPauseMs);
    } else if (TIMING.segmentPauseMs > 0) {
      await hold(page, TIMING.segmentPauseMs);
    }
    if (typeof segment.pauseAfterMs === 'number' && segment.pauseAfterMs > 0) {
      await hold(page, segment.pauseAfterMs);
    }
  }
}

async function typeLine(page, segments, opts = {}) {
  const list = Array.isArray(segments) ? segments : [{ text: String(segments ?? '') }];
  await typeSegments(page, list, opts);
  await type(page, '\n\n', opts);
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
  await page.goto(getRunUrl());

  // Wait for CodeMirror, WASM engine, and recorder API.
  await page.waitForSelector(EDITOR_CONTENT_SELECTOR, { timeout: 10_000 });
  await waitForRecorderApi(page);
  await hold(page, TIMING.wasmWarmupMs);

  // Clear any seed content so we start from a clean slate
  await page.evaluate(() => {
    window.__marqueDemoRecorder.clearDocument();
  });
  await hold(page, TIMING.clearSettleMs);

  // Hold on the blank document — UNCLASSIFIED banners visible
  await hold(page, TIMING.blankHoldMs);
}

/**
 * Scene 1 — Narrative
 */
async function scene1(page) {
  console.log('  Scene 1');
  await focusEnd(page);

  await typeLine(page, [
    { text: '(U) '},
    { text: 'Meet Marque. ' },
    { text: 'We\'ll use some classified markings to show how it works.' },
    { text: ' (...but the demo is totally unclassified, of course!)' },
  ]);
  await typeLine(page, [
    { text: '(U//FOUO) '},
    { text: 'Marque is a rules engine for text, designed for complex tagging, marking, and redaction tasks.'},
    { text: ' What you see here isn\'t Marque -- this is just a web page. Marque is the '},
    { text: 'engine under the hood', style: 'emphasis' },
    { text: '.' }
  ])
  await typeLine(page, [
    { text: '(c//rel to deu, Fvey) '},
    { text: '. It\'s running locally in the browser here, but it can run in any environment' },
    { text: '. Marque\'s first ruleset is U.S. classification markings.' },
    { text: ' As we type, you\'ll see Marque identify errors and fix them, with the audit record of what changed and why in the sidebar.' },
    { text: ' The banners will also update instantly to reflect the markings we apply.' }
  ]);
  console.log('    (typed intro)');
  // Fade the memo header out — it dissolves as this paragraph is typed.
  await page.evaluate(() => window.__marqueDemoRecorder.fadeHeader());
  await hold(page, 80);
  await typeLine(page, [
    { text: '(U//FOUO//LES) '},
    { text: 'Classification and control markings are ' },
    { text: 'really complex', style: 'emphasis' },
    { text: '. Lots of rules. Special cases. Anything out of the norm and you have to look it up... or get it wrong (...or both).' },
    { text: ' It often takes ' },
    { text: '10+ minutes', style: 'emphasis' },
    { text: ' to mark a single document, and it\'s easy to make mistakes.' },
  ]);
  console.log('    (typed problem statement)');
  await typeLine(page, [
    { text: '(c//REL TO fra, DEU, fvey) '},
    { text: 'People apply '},
    { text: 'millions ', style: 'emphasis' },
    { text: 'of control markings across the U.S. government every day. ' },
  ]);
  await typeLine(page, [
    { text: '(S//TK//Rel to Fvey) '},
    { text: 'A lot of time and effort. Money. Brainpower. And a ' },
    { text: 'lot of errors', style: 'emphasis' },
    { text: ' and ' },
    { text: 'uncertainty', style: 'emphasis' },
    { text: '.' },
    { text: ' . . ' },
    { text: '   That ends now.' },
  ]);
  console.log('    (typed solution statement)');
  await typeLine(page, [
    { text: '(s//REL TO naTO) '},
    { text: 'Marque is ' },
    { text: 'insanely fast', style: 'emphasis' },
      { text: '. On an average laptop, it can scan and fix about ' },
    { text: '4 million pages', style: 'emphasis' },
    { text: '. Per '},
    { text: 'minute', style: 'emphasis' },
    { text: '.' },
  ]);
  console.log('    (typed speed claim)');
  await typeLine(page, [
    { text: '(ts//LES//RD//SI//ORCON//IMCON//NOFORN) '},
    { text: 'Every decision has a statistical ' },
    { text: 'confidence score', style: 'emphasis' },
    { text: '. Like here. This generates an ' },
    { text: 'error ', style: 'emphasis' },
    { text: 'that Marque isn\'t sure about at this 95% confidence level.' },
    { text: ' The marking is a mess.', style: 'emphasis' },
    { text: ' Wrong order. Wrong separators. Banner markings that don\'t belong here. Marque can fix it. Let\'s lower the threshold to 60%.' },
  ]);

  // Show the same diagnostic tooltip a user sees when hovering the latest
  // marked portion text in the editor.
  const latestMarkedDiagnostic = page.locator('.cm-content .marque-error, .cm-content .marque-warn').last();
  await latestMarkedDiagnostic.scrollIntoViewIfNeeded();
  await latestMarkedDiagnostic.hover();
  await hold(page, 1000);

  await hold(page, TIMING.preSliderMs);

  await page.evaluate((value) => {
    window.__marqueDemoRecorder.setThreshold(value);
  }, 0.6);
  await hold(page, TIMING.postSliderMs);
  await typeLine(page, [
    { text: '(S//SAR-BP-BLU E42//SI//NF//XD) '},
    { text: 'There. Much better' , style: 'emphasis' },
    { text: ' Every fix can be ' },
    { text: 'explained', style: 'emphasis' },
    { text: '. And ' },
    { text: 'audited', style: 'emphasis' },
    { text: '.' },
  ]);
}

/**
 * Outro — fixed-frame hold (no page scrolling in the recording).
 */
async function outro(page) {
  console.log('  Outro: hold on fixed frame');
  await hold(page, TIMING.outroMs);
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

    // ── Launch browser (recording optional) ───────────────────────────────
    const videoDir = path.join(__dirname, '.video-tmp');
    if (!noVideo) {
      fs.mkdirSync(videoDir, { recursive: true });
    }

    const browser = await chromium.launch({
      headless: !headed,
      args: ['--no-sandbox'],
    });

    const contextOptions = {
      viewport: { width: 1280, height: 1000 },
      deviceScaleFactor: 2,
    };
    if (!noVideo) {
      contextOptions.recordVideo = {
        dir: videoDir,
        size: { width: 1280, height: 1000 },
      };
    }
    const context = await browser.newContext(contextOptions);

    const page = await context.newPage();
    if (debugTiming) {
      page.on('console', msg => {
        console.log(`  [browser:${msg.type()}] ${msg.text()}`);
      });
      page.on('pageerror', err => {
        console.log(`  [browser:error] ${err.message}`);
      });
    }

    // ── Run the script ─────────────────────────────────────────────────────
    console.log(noVideo ? 'Rehearsing in browser (no video)…\n' : 'Recording…\n');
    const runStartedAt = Date.now();
    await scene0_blank(page);
    await scene1(page);
    await outro(page);

    // Final hold on the completed document
    await hold(page, TIMING.finalHoldMs);
    debugLog('run-total', `${Date.now() - runStartedAt}ms`);

    // ── Save video ─────────────────────────────────────────────────────────
    const videoPath = noVideo ? null : await page.video()?.path();
    await context.close();   // finalizes the .webm
    await browser.close();

    if (noVideo) {
      console.log('\nRehearsal complete (no video output).');
    } else if (videoPath && fs.existsSync(videoPath)) {
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
