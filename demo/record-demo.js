#!/usr/bin/env node
/**
 * record-demo.js — Playwright demo video producer for marque
 *
 * Starts the demo server, records a narrated walkthrough via browser automation,
 * and saves a .webm video alongside a .mp4 transcode (if ffmpeg is available).
 *
 * Usage:
 *   node record-demo.js [--port 4242] [--output demo.webm] [--headed]
 *
 * Requirements:
 *   npm install playwright
 *   npx playwright install chromium
 *
 * Optional (for MP4 transcode):
 *   apt install ffmpeg  (or brew install ffmpeg)
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
let port    = 4343; // different from default dev port to avoid conflicts
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

/** Wait until the server is accepting connections. */
function waitForServer(url, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    function attempt() {
      http.get(url, res => {
        res.resume();
        resolve();
      }).on('error', () => {
        if (Date.now() > deadline) reject(new Error(`Server at ${url} did not start in time`));
        else setTimeout(attempt, 200);
      });
    }
    attempt();
  });
}

/** Type text character by character with a realistic delay. */
async function typeSlowly(page, selector, text, { delay = 80 } = {}) {
  const el = page.locator(selector);
  await el.click();
  for (const ch of text) {
    await page.keyboard.type(ch);
    await page.waitForTimeout(delay + Math.random() * 40);
  }
}

/** Type into the CodeMirror editor (which isn't a real textarea). */
async function typeIntoEditor(page, text, { delay = 80 } = {}) {
  // The CodeMirror content div is contenteditable
  const editor = page.locator('.cm-content');
  await editor.click({ position: { x: 5, y: 5 } });
  for (const ch of text) {
    await page.keyboard.type(ch);
    await page.waitForTimeout(delay + Math.random() * 30);
  }
}

/** Move the CM cursor to the end of the document. */
async function goToEditorEnd(page) {
  const editor = page.locator('.cm-content');
  await editor.click();
  await page.keyboard.press('Control+End');
  await page.waitForTimeout(200);
}

/** Pause for a beat (hold on a frame for the viewer). */
const hold = (page, ms) => page.waitForTimeout(ms);

// ---------------------------------------------------------------------------
// Script scenes
// ---------------------------------------------------------------------------

/**
 * Scene 1: Initial load — show the document with the SERCET typo autocorrecting
 * in the seed text.  The correction fires immediately after WASM loads.
 */
async function sceneInitialLoad(page) {
  console.log('  Scene 1: initial load');
  await page.goto(BASE_URL);

  // Wait for the CodeMirror editor to mount
  await page.waitForSelector('.cm-content', { timeout: 15_000 });

  // Wait a moment for the WASM engine to initialise and auto-correct the seed typo
  await hold(page, 2000);

  // Hold on the corrected document so the viewer can read it
  await hold(page, 3000);
}

/**
 * Scene 2: Show the audit trail entry that appeared from the SERCET→SECRET fix.
 */
async function sceneShowAudit(page) {
  console.log('  Scene 2: audit trail');
  // Scroll gently to reveal the audit stream below the document
  await page.evaluate(() => {
    const stream = document.getElementById('audit-stream');
    if (stream) stream.scrollIntoView({ behavior: 'smooth', block: 'center' });
  });
  await hold(page, 2500);

  // Scroll back up
  await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'smooth' }));
  await hold(page, 1000);
}

/**
 * Scene 3: User types a new TS//SCI paragraph with a banner-change effect.
 */
async function sceneTypeTsSci(page) {
  console.log('  Scene 3: type TS//SI paragraph');
  await goToEditorEnd(page);

  // Add two blank lines then a TS//SI//NF portion
  await page.keyboard.press('Enter');
  await page.keyboard.press('Enter');
  await hold(page, 400);
  await typeIntoEditor(page, '(TS//SI//NF) ', { delay: 90 });
  await hold(page, 800); // let banner update animate
  await typeIntoEditor(page, 'Sensitive compartmented reporting confirms the assessment.', { delay: 55 });
  await hold(page, 1500);
}

/**
 * Scene 4: Deliberately type a typo, watch it self-correct.
 */
async function sceneTypoCorrection(page) {
  console.log('  Scene 4: typo self-correction');
  await goToEditorEnd(page);

  await page.keyboard.press('Enter');
  await page.keyboard.press('Enter');
  await hold(page, 300);

  // Type a paragraph with another known typo — the corrections-map will catch it
  await typeIntoEditor(page, '(SERCET//NF) ', { delay: 100 });
  // Pause — let the debounce fire and show the correction mid-word
  await hold(page, 600);
  await typeIntoEditor(page, 'This line had a typo in the portion marking.', { delay: 60 });
  await hold(page, 1800);
}

/**
 * Scene 5: Hover a squiggly underline to show the tooltip.
 */
async function sceneTooltip(page) {
  console.log('  Scene 5: tooltip hover');
  // Find any element with the marque-warn or marque-error class
  const diagnostic = page.locator('.marque-error, .marque-warn').first();
  const exists = await diagnostic.count();
  if (exists > 0) {
    await diagnostic.hover();
    await hold(page, 2500);
    // Move away
    await page.mouse.move(100, 100);
    await hold(page, 500);
  }
}

/**
 * Scene 6: Scroll through the audit log — show it's been building up.
 */
async function sceneAuditScroll(page) {
  console.log('  Scene 6: scroll audit log');
  await page.evaluate(() => {
    const stream = document.getElementById('audit-stream');
    if (stream) stream.scrollIntoView({ behavior: 'smooth', block: 'start' });
  });
  await hold(page, 1000);
  await page.evaluate(() => window.scrollBy({ top: 300, behavior: 'smooth' }));
  await hold(page, 2000);
  await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'smooth' }));
  await hold(page, 800);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

(async () => {
  // ── 1. Start the demo server ────────────────────────────────────────────
  console.log(`Starting demo server on port ${port}…`);
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

    // ── 2. Launch browser with video recording ──────────────────────────
    const videoDir = path.join(__dirname, '.video-tmp');
    fs.mkdirSync(videoDir, { recursive: true });

    const browser = await chromium.launch({
      headless: !headed,
      args: ['--no-sandbox'],
    });

    const context = await browser.newContext({
      viewport: { width: 1280, height: 800 },
      deviceScaleFactor: 2, // HiDPI — crisp text in the recording
      recordVideo: {
        dir: videoDir,
        size: { width: 1280, height: 800 },
      },
    });

    const page = await context.newPage();

    // ── 3. Run the demo script ──────────────────────────────────────────
    console.log('Recording…\n');
    await sceneInitialLoad(page);
    await sceneShowAudit(page);
    await sceneTypeTsSci(page);
    await sceneTypoCorrection(page);
    await sceneTooltip(page);
    await sceneAuditScroll(page);

    // Final hold — let the viewer absorb the finished state
    await hold(page, 3000);

    // ── 4. Save video ───────────────────────────────────────────────────
    const videoPath = await page.video()?.path();
    await context.close(); // triggers video finalisation
    await browser.close();

    if (videoPath && fs.existsSync(videoPath)) {
      fs.renameSync(videoPath, outFile);
      console.log(`\nVideo saved: ${outFile}`);

      // Optional: transcode to MP4 with ffmpeg
      const mp4Out = outFile.replace(/\.webm$/i, '.mp4');
      try {
        execSync(
          `ffmpeg -y -i "${outFile}" -c:v libx264 -preset fast -crf 18 -movflags +faststart "${mp4Out}"`,
          { stdio: 'pipe' }
        );
        console.log(`MP4 saved:   ${mp4Out}`);
      } catch {
        console.log('(ffmpeg not available — skipping MP4 transcode)');
      }
    } else {
      console.error('Video file not found after recording.');
    }

  } finally {
    serverProc.kill();
    // Clean up temp dir
    fs.rmSync(path.join(__dirname, '.video-tmp'), { recursive: true, force: true });
  }
})().catch(err => {
  console.error(err);
  process.exit(1);
});
