// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: Apache-2.0

/**
 * Marque WASM Web Worker
 *
 * Runs the Marque WASM engine off the main thread so typing latency stays
 * frame-rate even on large inputs. This mirrors the worker-first deployment
 * shape `marque-wasm` is built for: the engine boots once, owns its own
 * config-keyed cache, and answers `lint`/`fix`/`banner` requests via
 * postMessage.
 *
 * Protocol (main → worker):
 *   { type: 'configure', config }                 → { type: 'ready' }
 *   { type: 'fix',    seq, text, threshold, config }
 *                                                 → { type: 'fix:result',    seq, ... }
 *   { type: 'fix:deep', seq, text }               → { type: 'fix:result',    seq, ..., mode: 'deep' }
 *   { type: 'banner', seq, text }                 → { type: 'banner:result', seq, banner }
 *   { type: 'cab',    seq, text, classifiedBy, derivedFrom }
 *                                                 → { type: 'cab:result',    seq, cab }
 *
 * Sequence numbers let the main thread drop stale results when a newer
 * request has already been issued.
 */

import initWasm, {
  configure, fix, fix_deep_scan, compute_banner, generate_cab,
} from '/wasm/marque_wasm.js';

const textEncoder = new TextEncoder();

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
          mode: 'strict',
          fixedText,
          banner,
          applied: parsed.applied ?? [],
          remaining: parsed.remaining ?? [],
        });
        return;
      }

      case 'fix:deep': {
        // Phase D probabilistic recognizer. The deep-scan API takes only the
        // byte buffer — no threshold, no runtime config. Audit records carry
        // V2 provenance (recognition, runner_up_ratio, features).
        await ensureReady(msg.config);
        const bytes = textEncoder.encode(msg.text);
        const json = fix_deep_scan(bytes);
        const parsed = JSON.parse(json);
        const fixedText = parsed.fixed_text ?? msg.text;
        let banner;
        try { banner = compute_banner(fixedText); }
        catch { banner = 'UNCLASSIFIED'; }
        self.postMessage({
          type: 'fix:result',
          seq: msg.seq,
          mode: 'deep',
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

      case 'cab': {
        await ensureReady(msg.config);
        let cab;
        try {
          cab = generate_cab(
            msg.text,
            msg.classifiedBy ?? null,
            msg.derivedFrom  ?? null,
          );
        } catch (err) {
          cab = '';
          self.postMessage({
            type: 'cab:result',
            seq: msg.seq,
            cab: '',
            error: String(err && err.message ? err.message : err),
          });
          return;
        }
        self.postMessage({ type: 'cab:result', seq: msg.seq, cab });
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
