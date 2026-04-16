#!/usr/bin/env node
/**
 * record-demo.js — Playwright demo video producer for marque
 *
 * Records a scripted walkthrough of the Marque interactive demo:
 *   Scene 1  — (U/FOUO) typo → auto-corrects to (U//FOUO)
 *   Scene 2  — (DEU C//REL TO NATO) → (//DEU C//REL TO USA, NATO)
 *   Scene 3  — (SERCET//NF) → (S//NF)  — typo + abbreviation
 *   Scene 4  — (TS//FOUO//SAR-BUTTER POPCORN/SODA//SI-TK) — complex reorder + abbreviation
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
 * Scene 1 — FOUO: type (U/FOUO), watch it correct to (U//FOUO).
 * Banner updates to UNCLASSIFIED//FOR OFFICIAL USE ONLY.
 */
async function scene1_fouo(page) {
  console.log('  Scene 1: U/FOUO → U//FOUO');
  await focusEnd(page);

  // Type the marking with the deliberate single-slash mistake
  await type(page, '(U/FOUO) ');

  // Pause — let the debounce fire and the correction animate
  await waitForCorrection(page, 'U/FOUO', 2000);
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
 * Scene 2 — FGI: type (DEU C//REL TO NATO), watch it correct to
 * (//DEU C//REL TO USA, NATO).
 * Banner updates to //DEU CONFIDENTIAL//REL TO USA, NATO.
 */
async function scene2_fgi(page) {
  console.log('  Scene 2: DEU C//REL TO NATO → FGI corrected form');
  await focusEnd(page);

  await type(page, '(DEU C//REL TO NATO)');

  await waitForCorrection(page, 'DEU C//REL TO NATO', 2000);
  await afterCorrection(page);

  await type(page, ' Allied partners have provided supporting analysis consistent with ongoing bilateral sharing agreements.', { charMs: 50 });
  await hold(page, 800);

  await page.keyboard.press('Enter');
  await page.keyboard.press('Enter');
  await betweenParagraphs(page);
}

/**
 * Scene 3 — Typo: type (SERCET//NF), watch it correct to (S//NF).
 * Banner updates to SECRET//FGI DEU//NOFORN.
 */
async function scene3_typo(page) {
  console.log('  Scene 3: SERCET//NF → S//NF');
  await focusEnd(page);

  await type(page, '(SERCET//NF) ');

  await waitForCorrection(page, 'SERCET', 2000);
  await afterCorrection(page);

  await type(page, 'Sensitive source reporting confirms the threat assessment with high confidence.', { charMs: 52 });
  await hold(page, 800);

  await page.keyboard.press('Enter');
  await page.keyboard.press('Enter');
  await betweenParagraphs(page);
}

/**
 * Scene 4 — Complex TS marking: type the disordered/verbose form, watch the
 * engine reorder, strip FOUO, and abbreviate SAR program names.
 * Input:   (TS//FOUO//SAR-BUTTER POPCORN/SODA//SI-TK)
 * Output:  (TS//SI/TK//SAR-BP/SDA)
 * Banner:  TOP SECRET//SI/TK//SAR-BUTTER POPCORN/SODA//FGI DEU//NOFORN
 */
async function scene4_ts_complex(page) {
  console.log('  Scene 4: TS complex reorder + SAR abbreviation');
  await focusEnd(page);

  // Type slowly — this is the climax scene, give the viewer time to read it
  await type(page, '(TS//FOUO//SAR-BUTTER POPCORN/SODA//SI-TK)', { charMs: 110 });

  // Longer hold — many fixes fire simultaneously; let the viewer absorb the
  // input text before the correction snaps it into canonical form
  await hold(page, 800);
  await waitForCorrection(page, 'FOUO', 3000);
  await hold(page, 2200); // extra hold on corrected form + banner

  // Brief sentence to complete the last paragraph
  await type(page, ' Special access reporting corroborates the assessment.', { charMs: 55 });
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
    await scene2_fgi(page);
    await scene3_typo(page);
    await scene4_ts_complex(page);
    await outro(page);

    // Final hold on the completed document
    await hold(page, 2500);

    // ── Save video ─────────────────────────────────────────────────────────
    const videoPath = await page.video()?.path();
    await context.close();   // finalises the .webm
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
