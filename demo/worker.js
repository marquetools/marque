// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: Apache-2.0

/**
 * Marque WASM Web Worker
 *
 * Runs the Marque WASM engine off the main thread so typing latency stays
 * frame-rate even on large inputs. This mirrors the worker-first deployment
 * shape `marque-wasm` is built for: the engine boots once, owns its own
 * config-keyed cache, and answers `fix`/`banner` requests via postMessage.
 *
 * Recognizer dispatch lives inside the engine: post-#259, `Engine::new`
 * installs `StrictOrDecoderRecognizer` (strict-first / decoder-fallback)
 * by default. Lowercase, typo'd, superseded, or otherwise mangled
 * markings get recovered through the regular `fix` path with no flag
 * opt-in. The threshold gates which fixes auto-apply, not which
 * recognizer runs.
 *
 * Protocol (main → worker):
 *   { type: 'configure', config }                 → { type: 'ready' }
 *   { type: 'fix',    seq, text, threshold, config }
 *                                                 → { type: 'fix:result',    seq, ... }
 *   { type: 'banner', seq, text }                 → { type: 'banner:result', seq, banner }
 *
 * Sequence numbers let the main thread drop stale results when a newer
 * request has already been issued.
 */

import initWasm, {
  configure, fix, compute_banner,
} from '/wasm/marque_wasm.js';

let ready = false;
let warmedConfig = null;

async function ensureReady(configJson) {
  if (!ready) {
    await initWasm();
    ready = true;
  }
  // Pre-warm the engine cache when the config changes (or on first call).
  if (configJson !== warmedConfig) {
    try {
      configure(configJson ?? undefined);
      warmedConfig = configJson;
    } catch (err) {
      // Configure failure is non-fatal — fix()/banner() will fail with
      // the same error if the config is truly malformed; surface it then.
      self.postMessage({ type: 'error', stage: 'configure', error: String(err) });
    }
  }
}

self.onmessage = async (event) => {
  const msg = event.data;
  if (!msg || typeof msg !== 'object') return;

  try {
    switch (msg.type) {
      case 'configure': {
        await ensureReady(msg.config);
        self.postMessage({ type: 'ready' });
        return;
      }

      case 'fix': {
        await ensureReady(msg.config);
        const json = fix(msg.text, msg.threshold ?? 0.0, msg.config ?? undefined);
        const parsed = JSON.parse(json);
        const fixedText = parsed.fixed_text ?? msg.text;
        // Compute the banner from the post-fix text so the rendered banner
        // always reflects what the document will look like after fixes apply.
        let banner;
        try { banner = compute_banner(fixedText); }
        catch { banner = 'UNCLASSIFIED'; }
        self.postMessage({
          type: 'fix:result',
          seq: msg.seq,
          fixedText,
          banner,
          applied: parsed.applied ?? [],
          remaining: parsed.remaining ?? [],
        });
        return;
      }

      case 'banner': {
        await ensureReady(msg.config);
        let banner;
        try {
          banner = compute_banner(msg.text);
        } catch {
          banner = 'UNCLASSIFIED';
        }
        self.postMessage({ type: 'banner:result', seq: msg.seq, banner });
        return;
      }

      default:
        self.postMessage({ type: 'error', stage: 'dispatch', error: `unknown message type: ${msg.type}` });
    }
  } catch (err) {
    self.postMessage({
      type: 'error',
      stage: msg.type,
      seq: msg.seq,
      error: String(err && err.message ? err.message : err),
    });
  }
};
