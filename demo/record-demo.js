#!/usr/bin/env node

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: Apache-2.0

/**
 * record-demo.js — Playwright demo video producer for marque
 *
 * Records a scripted walkthrough of the Marque interactive demo:
 *   Scene 1  — (U//FOUO) → (U//CUI) — deprecated control migration
 *   Scene 2  — (SECRET//NOFORN) → (S//NF) — abbreviation enforcement
 *   Scene 3  — (SERCET//NF) → (S//NF) — typo correction + abbreviation
 *   Scene 4  — (TS//SI-G//NOFORN) → (TS//SI-G//NF) — TS/SCI + banner escalation
 *   Outro    — scroll to audit log
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
const jitter = base => base * 0.6 + Math.random() * base * 0.8;

/**
 * Type text into the CodeMirror editor character by character.
 * Handles special chars like newlines via keyboard.press.
 */
async function type(page, text, { charMs = 95 } = {}) {
  for (const ch of text) {
    if (ch === '\n') {
      await page.keyboard.press('Enter');
    } else {
      await page.keyboard.type(ch);
    }
    await page.waitForTimeout(jitter(charMs));
  }
}

/** Focus the editor and place cursor at the end of document. */
async function focusEnd(page) {
  await page.locator('.cm-content').click();
  await page.keyboard.press('Control+End');
  await page.waitForTimeout(150);
}

/**
 * Wait for a visible correction — polls until the editor text no longer
 * contains `errorText`.  Timeout after `ms` millis (returns without error).
 */
async function waitForCorrection(page, errorText, ms = 2000) {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    const content = await page.locator('.cm-content').innerText();
    if (!content.includes(errorText)) return;
    await page.waitForTimeout(50);
  }
}

/** Hold after a correction so the viewer can absorb the change. */
const afterCorrection = page => hold(page, 1400);
/** Short beat between paragraphs. */
const betweenParagraphs = page => hold(page, 900);

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
  await page.waitForSelector('.cm-content', { timeout: 20_000 });
  await hold(page, 2200); // WASM init + configure()

  // Clear any seed content so we start from a clean slate
  await page.locator('.cm-content').click();
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Delete');
  await hold(page, 400);

  // Hold on the blank document — UNCLASSIFIED banners visible
  await hold(page, 2000);
}

/**
 * Scene 1 — Deprecated control: type (U//FOUO), watch it auto-correct to (U//CUI).
 * FOUO is deprecated per CAPCO-2016 §F; the migration table replaces it with CUI.
 * Banner: UNCLASSIFIED (U-level marking doesn't elevate the banner).
 */
async function scene1_fouo(page) {
  console.log('  Scene 1: (U//FOUO) → (U//CUI)');
  await focusEnd(page);

  await type(page, '(U//FOUO) ');

  // Pause — let the debounce fire and the correction animate
  await waitForCorrection(page, 'FOUO', 2000);
  await afterCorrection(page);

  // Type body text
  await type(page, 'Initial assessment prepared for authorized recipients under appropriate handling controls.', { charMs: 55 });
  await hold(page, 800);

  // New paragraph
  await page.keyboard.press('Enter');
  await page.keyboard.press('Enter');
  await betweenParagraphs(page);
}

/**
 * Scene 2 — Abbreviation enforcement: type (SECRET//NOFORN), watch it correct
 * to (S//NF). Rule E009 enforces abbreviated forms in portion markings per
 * CAPCO-2016 §C.1.
 * Banner updates to SECRET//NOFORN.
 */
async function scene2_abbreviation(page) {
  console.log('  Scene 2: (SECRET//NOFORN) → (S//NF)');
  await focusEnd(page);

  await type(page, '(SECRET//NOFORN) ');

  await waitForCorrection(page, 'NOFORN', 2000);
  await afterCorrection(page);

  await type(page, 'Classified source material confirms the operational assessment with high confidence.', { charMs: 50 });
  await hold(page, 800);

  await page.keyboard.press('Enter');
  await page.keyboard.press('Enter');
  await betweenParagraphs(page);
}

/**
 * Scene 3 — Typo correction: type (SERCET//NF), watch the two-pass pipeline
 * correct the typo via the corrections map (C001: SERCET → SECRET) then
 * abbreviate (E009: SECRET → S), yielding (S//NF).
 * Banner remains SECRET//NOFORN (same classification level as scene 2).
 */
async function scene3_typo(page) {
  console.log('  Scene 3: (SERCET//NF) → (S//NF)');
  await focusEnd(page);

  await type(page, '(SERCET//NF) ');

  await waitForCorrection(page, 'SERCET', 2000);
  await afterCorrection(page);

  await type(page, 'Sensitive reporting corroborates the threat assessment.', { charMs: 52 });
  await hold(page, 800);

  await page.keyboard.press('Enter');
  await page.keyboard.press('Enter');
  await betweenParagraphs(page);
}

/**
 * Scene 4 — TS/SCI escalation: type (TS//SI-G//NOFORN), watch the engine
 * abbreviate NOFORN → NF (E009).
 * Banner escalates to TOP SECRET//SI-G//NOFORN — the climax of the demo.
 */
async function scene4_ts_sci(page) {
  console.log('  Scene 4: (TS//SI-G//NOFORN) → (TS//SI-G//NF)');
  await focusEnd(page);

  // Type slowly — this is the climax scene, give the viewer time to read it
  await type(page, '(TS//SI-G//NOFORN)', { charMs: 100 });

  // Hold to let the viewer see the marking before correction fires
  await hold(page, 800);
  await waitForCorrection(page, 'NOFORN', 3000);
  await hold(page, 2200); // extra hold on corrected form + banner

  // Brief sentence to complete the last paragraph
  await type(page, ' Compartmented analysis supports the assessment.', { charMs: 55 });
  await hold(page, 1200);
}

/**
 * Outro — scroll down to reveal the audit log, then back up.
 */
async function outro(page) {
  console.log('  Outro: audit log reveal');
  await hold(page, 800);

  await page.evaluate(() => {
    document.getElementById('audit-stream')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  });
  await hold(page, 3500);

  await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'smooth' }));
  await hold(page, 1500);
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
    await scene1_fouo(page);
    await scene2_abbreviation(page);
    await scene3_typo(page);
    await scene4_ts_sci(page);
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
