// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: Apache-2.0

/**
 * Marque Interactive Demo (worker-backed)
 *
 * Architecture:
 *   - The Marque WASM engine runs in a Web Worker (worker.js). The main
 *     thread never blocks on lint/fix/banner — typing latency stays at
 *     frame rate even on large inputs.
 *   - On debounce, the editor posts {type:'fix', seq, text, threshold,
 *     config} to the worker. The worker returns {fixedText, banner,
 *     applied[], remaining[]}.
 *   - Stale results are dropped via sequence numbers — only the most
 *     recent request's reply updates the editor.
 *
 * Surface:
 *   - Real-time banner roll-up from portion markings (computed against
 *     the post-fix text the engine returns).
 *   - Correct-as-you-type via fix() — applies each fix as a CodeMirror
 *     change at its byte offset, so the cursor never jumps.
 *   - Squiggly underlines for remaining diagnostics, with hover
 *     tooltips that show rule ID + message + CAPCO citation.
 *   - Inline audit stream — each fix produces a styled entry below the
 *     document, blending into the page background.
 *   - Faded placeholder + idle autoplay: an empty editor invites the
 *     user to type, and after ~6s of inactivity an autoplay sequence
 *     demonstrates the engine end-to-end. Any input pauses autoplay
 *     immediately.
 */

import {
  EditorView,
  Decoration,
  ViewPlugin,
  WidgetType,
  hoverTooltip,
  StateEffect,
  StateField,
  EditorState,
} from './vendor.js';

// ---------------------------------------------------------------------------
// Engine configuration (passed through to the worker)
// ---------------------------------------------------------------------------

const DEMO_CONFIG = JSON.stringify({
  corrections: {
    'SERCET':       'SECRET',
    'SECERT':       'SECRET',
    'SECRECT':      'SECRET',
    'SCERET':       'SECRET',
    'SECRTE':       'SECRET',
    'CONFIDETIAL':  'CONFIDENTIAL',
    'CONFIENTIAL':  'CONFIDENTIAL',
    'CONFIDENTAL':  'CONFIDENTIAL',
    'UNCALSSIFIED': 'UNCLASSIFIED',
    'UNCLASSFIED':  'UNCLASSIFIED',
    'UNCLASSIFED':  'UNCLASSIFIED',
    'NOFON':        'NOFORN',
    'NOFRON':       'NOFORN',
  },
});

// ---------------------------------------------------------------------------
// Timing — single source of truth for all delays in the page.
// Adjust here only; nothing else in this file contains timing literals.
// ---------------------------------------------------------------------------

const DEBOUNCE_MS          = 100;   // ms to wait after last keystroke before posting to worker
const APPLY_IDLE_MS        = 140;   // ms of editor quiet before applying pending fixes
const AUTOPLAY_IDLE_MS     = 6_000; // ms of user inactivity before autoplay starts
const AUTOPLAY_CHAR_MS     = 30;   // per-character typing speed during autoplay
const AUTOPLAY_NEWLINE_MS  = 150;  // extra pause at single newlines during autoplay
const AUTOPLAY_PARAGRAPH_MS = 150; // extra pause at paragraph breaks during autoplay
const HISTOGRAM_BUCKETS = 20;     // 0.05-wide buckets across [0, 1]
const PLACEHOLDER_TEXT =
  '';

const DEBUG_TIMING = new URLSearchParams(window.location.search).has('debug_timing');

function debugTimingLog(event, details = {}) {
  if (!DEBUG_TIMING) return;
  const payload = {
    t: Number(performance.now().toFixed(1)),
    ...details,
  };
  console.debug(`[demo-timing] ${event}`, payload);
}

// Mutable: bound to the slider in the side rail. Changing this re-issues a
// fix request so the user can watch how few diagnostics actually drop out
// when the bar is raised — that's the robustness story.
//
// The threshold gates which fixes auto-apply, not which recognizer runs.
// Post-#259 the engine itself installs `StrictOrDecoderRecognizer` by
// default (strict-first, decoder fallback) — there's no `--deep-scan`
// any more. At 1.00 the engine still parses both ways, but only fixes it
// is fully certain about cross the bar. Below 1.00 the decoder's recovery
// fixes start to apply.
let currentThreshold = 0.95;

// Autoplay script: a tour of marking domains, narrated, seeded with
// intentional typos so the audit log lights up while the script types.
// The final line drops `OC` (ORCON) on purpose — SI-G requires ORCON
// per CAPCO §H.4 p80, so the engine fires E047 with no auto-fix. This
// is the demo's showcase of a true error: an underline appears under
// SI-G and the autoplay-completion hook pops the tooltip.
//
// Verified against the engine (Apr 30, post-#259):
//   (u)                            → (U)                            [case fold, fix]
//   (U//rel to USA, FVEY)          → (U//REL TO USA, FVEY)          [case fold, fix]
//   (SERCET//SI//REL TO USA, GBR)  → (SECRET//SI//REL TO USA, GBR)  [edit-dist, fix]
//   (TS//SI-G/TK//RS/NOFRON//LES)  → (TS//SI-G/TK//RS/NOFORN//LES)  [edit-dist, fix]
//   E047  warn  no-fix  SI-G requires ORCON (§H.4 p80)
// (U//FOUO) and (//NC//REL TO USA, NATO) stay canonical — domain
// breadth, not a fix path.
const AUTOPLAY_SCRIPT ='';

// ---------------------------------------------------------------------------
// Worker boot
// ---------------------------------------------------------------------------

const worker = new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });
let workerReady = false;
const workerReadyPromise = new Promise(resolve => {
  worker.addEventListener('message', function onReady(ev) {
    if (ev.data && ev.data.type === 'ready') {
      workerReady = true;
      worker.removeEventListener('message', onReady);
      resolve();
    }
  });
});
worker.postMessage({ type: 'configure', config: DEMO_CONFIG });

// ---------------------------------------------------------------------------
// CodeMirror state plumbing — diagnostics and placeholder
// ---------------------------------------------------------------------------

const setDiagnosticsEffect = StateEffect.define();

const diagnosticsField = StateField.define({
  create() { return Decoration.none; },
  update(decorations, tr) {
    decorations = decorations.map(tr.changes);
    for (const e of tr.effects) {
      if (e.is(setDiagnosticsEffect)) decorations = e.value;
    }
    return decorations;
  },
  provide: f => EditorView.decorations.from(f),
});

class PlaceholderWidget extends WidgetType {
  constructor(text) { super(); this.text = text; }
  toDOM() {
    const el = document.createElement('span');
    el.className = 'cm-placeholder';
    el.textContent = this.text;
    return el;
  }
  eq(other) { return other.text === this.text; }
  ignoreEvent() { return true; }
}

const placeholderPlugin = ViewPlugin.fromClass(class {
  constructor(view) { this.decorations = this.compute(view); }
  update(update) {
    if (update.docChanged || update.viewportChanged) {
      this.decorations = this.compute(update.view);
    }
  }
  compute(view) {
    if (view.state.doc.length === 0) {
      return Decoration.set([
        Decoration.widget({
          widget: new PlaceholderWidget(PLACEHOLDER_TEXT),
          side: 1,
        }).range(0),
      ]);
    }
    return Decoration.none;
  }
}, { decorations: v => v.decorations });

// ---------------------------------------------------------------------------
// Page-break decoration — make form-feed (\f) visible as a horizontal rule.
// ---------------------------------------------------------------------------

class PageBreakWidget extends WidgetType {
  toDOM() {
    const el = document.createElement('span');
    el.className = 'cm-pagebreak';
    el.setAttribute('aria-label', 'Page break');
    el.textContent = '— page break —';
    return el;
  }
  eq() { return true; }
  ignoreEvent() { return true; }
}

const pageBreakPlugin = ViewPlugin.fromClass(class {
  constructor(view) { this.decorations = this.compute(view); }
  update(update) {
    if (update.docChanged || update.viewportChanged) {
      this.decorations = this.compute(update.view);
    }
  }
  compute(view) {
    const ranges = [];
    const text = view.state.doc.toString();
    for (let i = 0; i < text.length; i++) {
      if (text.charCodeAt(i) === 0x0c /* \f */) {
        ranges.push(
          Decoration.replace({ widget: new PageBreakWidget() }).range(i, i + 1),
        );
      }
    }
    return Decoration.set(ranges);
  }
}, { decorations: v => v.decorations });

// ---------------------------------------------------------------------------
// Scripted styling decorations (display-only)
//
// The recording script must type plain text through Playwright, but we still
// want a few words to read with emphasis. This plugin layers visual styling on
// top of the underlying text, so engine spans/offsets remain untouched.
// ---------------------------------------------------------------------------

const scriptedStylePlugin = ViewPlugin.fromClass(class {
  constructor(view) { this.decorations = this.compute(view); }
  update(update) {
    if (update.docChanged || update.viewportChanged) {
      this.decorations = this.compute(update.view);
    }
  }
  compute(view) {
    const text = view.state.doc.toString();
    const marks = [];

    for (const m of text.matchAll(/\bMarque\b/g)) {
      marks.push(Decoration.mark({ class: 'demo-brand' }).range(m.index, m.index + m[0].length));
    }

    const OPEN_SINGLE = '[em]';
    const CLOSE_SINGLE = '[/em]';
    const OPEN_DOUBLE = '[[em]]';
    const CLOSE_DOUBLE = '[[/em]]';

    const delimiterPrefixes = [
      OPEN_DOUBLE,
      CLOSE_DOUBLE,
      OPEN_SINGLE,
      CLOSE_SINGLE,
    ];

    const hideRange = (from, to) => {
      if (to > from) marks.push(Decoration.replace({}).range(from, to));
    };

    const trailingPrefixLen = (s, start, fullDelimiter) => {
      const available = s.length - start;
      const maxLen = Math.min(fullDelimiter.length - 1, available);
      for (let len = maxLen; len >= 1; len--) {
        if (s.slice(s.length - len) === fullDelimiter.slice(0, len) && s.length - len >= start) {
          return len;
        }
      }
      return 0;
    };

    let i = 0;
    while (i < text.length) {
      if (text.startsWith(OPEN_DOUBLE, i)) {
        hideRange(i, i + OPEN_DOUBLE.length);
        const contentStart = i + OPEN_DOUBLE.length;
        const closeAt = text.indexOf(CLOSE_DOUBLE, contentStart);

        if (closeAt === -1) {
          const partialLen = trailingPrefixLen(text, contentStart, CLOSE_DOUBLE);
          const contentEnd = text.length - partialLen;
          if (contentEnd > contentStart) {
            marks.push(Decoration.mark({ class: 'demo-emphasis' }).range(contentStart, contentEnd));
          }
          if (partialLen > 0) {
            hideRange(text.length - partialLen, text.length);
          }
          break;
        }

        if (closeAt > contentStart) {
          marks.push(Decoration.mark({ class: 'demo-emphasis' }).range(contentStart, closeAt));
        }
        hideRange(closeAt, closeAt + CLOSE_DOUBLE.length);
        i = closeAt + CLOSE_DOUBLE.length;
        continue;
      }

      if (text.startsWith(OPEN_SINGLE, i)) {
        hideRange(i, i + OPEN_SINGLE.length);
        const contentStart = i + OPEN_SINGLE.length;
        const closeAt = text.indexOf(CLOSE_SINGLE, contentStart);

        if (closeAt === -1) {
          const partialLen = trailingPrefixLen(text, contentStart, CLOSE_SINGLE);
          const contentEnd = text.length - partialLen;
          if (contentEnd > contentStart) {
            marks.push(Decoration.mark({ class: 'demo-emphasis' }).range(contentStart, contentEnd));
          }
          if (partialLen > 0) {
            hideRange(text.length - partialLen, text.length);
          }
          break;
        }

        if (closeAt > contentStart) {
          marks.push(Decoration.mark({ class: 'demo-emphasis' }).range(contentStart, closeAt));
        }
        hideRange(closeAt, closeAt + CLOSE_SINGLE.length);
        i = closeAt + CLOSE_SINGLE.length;
        continue;
      }

      if (i > 0 || i + 1 === text.length) {
        let hiddenPartial = false;
        for (const delimiter of delimiterPrefixes) {
          const maxLen = Math.min(delimiter.length - 1, text.length - i);
          for (let len = maxLen; len >= 1; len--) {
            if (i + len === text.length && text.slice(i, i + len) === delimiter.slice(0, len)) {
              hideRange(i, i + len);
              i += len;
              hiddenPartial = true;
              break;
            }
          }
          if (hiddenPartial) break;
        }
        if (hiddenPartial) continue;
      }

      i += 1;
    }

    const portionPattern = /(^|\n)(\((?:ts|s|c|u|r)(?:\/\/[^)\n]+)?\))(?=\s|$)/gim;
    for (const m of text.matchAll(portionPattern)) {
      const leadingLen = m[1] ? m[1].length : 0;
      const portionText = m[2] || '';
      if (!portionText) continue;
      const from = m.index + leadingLen;
      const to = from + portionText.length;
      marks.push(Decoration.mark({ class: 'demo-portion' }).range(from, to));
    }

    return Decoration.set(marks, true);
  }
}, { decorations: v => v.decorations });

// ---------------------------------------------------------------------------
// Banner classification → CSS class
// ---------------------------------------------------------------------------

const LEVEL_CLASSES = [
  ['TOP SECRET', 'level-ts'],
  ['SECRET',     'level-secret'],
  ['RESTRICTED',  'level-restricted'], // only in non-U.S. schemes but valid in that context
  ['CONFIDENTIAL','level-confidential'],
];

function classificationClass(banner) {
  const b = banner.toUpperCase();
  for (const [prefix, cls] of LEVEL_CLASSES) {
    if (b.startsWith(prefix)) {
      if (prefix === 'TOP SECRET' && b.includes('//') && b.length > 10) {
        return 'level-ts-sci';
      }
      return cls;
    }
  }
  return 'level-unclassified';
}

const ALL_LEVEL_CLASSES = [
  'level-unclassified', 'level-confidential', 'level-secret',
  'level-ts', 'level-ts-sci', 'level-restricted', 'level-empty',
];

function applyBanner(banner, topEl, bottomEl) {
  const cls = classificationClass(banner);
  topEl.classList.remove(...ALL_LEVEL_CLASSES);
  bottomEl.classList.remove(...ALL_LEVEL_CLASSES);
  topEl.classList.add(cls);
  bottomEl.classList.add(cls);
  topEl.textContent = banner;
  bottomEl.textContent = banner;

  // Toggle the document-card classified state, which gates visibility of
  // the static CAB block. UNCLASSIFIED (and the empty-doc placeholder) =
  // no CAB; anything else = CAB visible. Cheap string check matches the
  // banner output verbatim — `compute_banner()` returns canonical
  // uppercase text.
  const docCard = document.getElementById('document-card');
  if (docCard) {
    const classified = banner && !banner.startsWith('UNCLASSIFIED');
    docCard.classList.toggle('is-classified', !!classified);
  }
}

// ---------------------------------------------------------------------------
// Audit stream
// ---------------------------------------------------------------------------

let auditEntryCount = 0;
const MAX_AUDIT_ENTRIES = 80;

function ensureAuditList(stream) {
  let list = stream.querySelector('.audit-list');
  if (list) return list;

  list = document.createElement('div');
  list.className = 'audit-list';
  stream.appendChild(list);
  return list;
}

function prependAuditNode(node, stream, emptyEl) {
  const list = ensureAuditList(stream);

  emptyEl.hidden = true;
  stream.classList.add('has-entries');
  list.prepend(node);

  while (list.childElementCount > MAX_AUDIT_ENTRIES) {
    list.lastElementChild.remove();
  }

  stream.scrollTop = 0;
}

function enqueueAuditEntry(record, stream, emptyEl) {

  // Prefer the engine's RFC3339 timestamp from the audit record. Fall back
  // to the wall clock if the WASM emitted a V0 record without it.
  const ts = parseAuditTimestamp(record.timestamp);
  const timeStr = formatAuditTime(ts);

  const sourceLabel = formatAuditSource(record.source);
  const pct = Math.round((record.confidence ?? 1) * 100);
  const startOff = record.span?.start;
  const endOff   = record.span?.end;
  const hasSpan  = Number.isInteger(startOff) && Number.isInteger(endOff);

  const entry = document.createElement('article');
  entry.className = 'audit-entry';
  entry.setAttribute('role', 'log');

  // ── Row 1: timestamp · rule · source · span · confidence ──────────────
  const headRow = document.createElement('div');
  headRow.className = 'audit-row audit-head';

  const timeEl = document.createElement('span');
  timeEl.className = 'audit-time';
  timeEl.textContent = timeStr;
  headRow.appendChild(timeEl);

  const ruleEl = document.createElement('span');
  ruleEl.className = 'audit-rule';
  ruleEl.textContent = record.rule;
  headRow.appendChild(ruleEl);

  const sourceEl = document.createElement('span');
  sourceEl.className = 'audit-source';
  sourceEl.textContent = sourceLabel;
  headRow.appendChild(sourceEl);

  if (hasSpan) {
    const spanEl = document.createElement('span');
    spanEl.className = 'audit-span';
    spanEl.textContent = `span ${startOff}..${endOff}`;
    headRow.appendChild(spanEl);
  }

  const spacer = document.createElement('span');
  spacer.className = 'audit-spacer';
  headRow.appendChild(spacer);

  const confEl = document.createElement('span');
  confEl.className = 'audit-confidence';
  confEl.textContent = `${pct}%`;
  headRow.appendChild(confEl);

  entry.appendChild(headRow);

  // ── Row 2: diff — "original" → "replacement" ──────────────────────────
  // Decoder-path records (R001 / probabilistic recovery) intentionally
  // omit the "before" form per the audit-record-shape contract
  // (Constitution V Principle V / G13): the original byte sequence cannot
  // be echoed back into the audit log, even though the engine knows it.
  // When original is empty, render only the replacement — the meta row
  // below carries the recognition / features context.
  const original = record.original ?? '';
  const replacement = record.replacement ?? '';
  const diffRow = document.createElement('div');
  diffRow.className = 'audit-row audit-diff';

  if (original.length > 0) {
    const originalEl = document.createElement('span');
    originalEl.className = 'audit-original';
    originalEl.textContent = JSON.stringify(original);

    const arrowEl = document.createElement('span');
    arrowEl.className = 'audit-arrow';
    arrowEl.textContent = '→';

    const replacementEl = document.createElement('span');
    replacementEl.className = 'audit-replacement';
    replacementEl.textContent = JSON.stringify(replacement);

    diffRow.append(originalEl, arrowEl, replacementEl);
  } else {
    // Decoder path: only the canonical replacement is in the log.
    const insertEl = document.createElement('span');
    insertEl.className = 'audit-replacement';
    insertEl.textContent = `inserted ${JSON.stringify(replacement)}`;
    diffRow.appendChild(insertEl);
  }
  entry.appendChild(diffRow);

  // ── Row 3 (optional): provenance/extras — migration · classifier ·
  // dry-run · V2 recognition/runner-up/features ─────────────────────────
  const meta = [];
  if (record.migration_ref) {
    meta.push(['migration', record.migration_ref]);
  }
  if (record.classifier_id) {
    meta.push(['classifier', record.classifier_id]);
  }
  if (record.dry_run) {
    meta.push(['mode', 'dry-run']);
  }
  if (typeof record.recognition === 'number' && record.recognition < 1) {
    meta.push(['recognition', record.recognition.toFixed(3)]);
    if (typeof record.runner_up_ratio === 'number') {
      meta.push(['runner-up', record.runner_up_ratio.toFixed(3)]);
    }
    if (Array.isArray(record.features) && record.features.length > 0) {
      const labels = record.features.map(f =>
        typeof f === 'string'
          ? f
          : (f && f.id ? f.id : '')
      ).filter(Boolean);
      if (labels.length > 0) meta.push(['features', labels.join(' · ')]);
    }
  }

  if (meta.length > 0) {
    const metaRow = document.createElement('div');
    metaRow.className = 'audit-row audit-meta';
    for (const [key, value] of meta) {
      const k = document.createElement('span');
      k.className = 'audit-meta-key';
      k.textContent = key;
      const v = document.createElement('span');
      v.className = 'audit-meta-val';
      v.textContent = value;
      metaRow.append(k, v);
    }
    entry.appendChild(metaRow);
  }

  prependAuditNode(entry, stream, emptyEl);
  auditEntryCount++;
}

function parseAuditTimestamp(raw) {
  if (typeof raw === 'string' && raw.length > 0) {
    const t = Date.parse(raw);
    if (!Number.isNaN(t)) return new Date(t);
  }
  return new Date();
}

function formatAuditTime(date) {
  const hh = String(date.getHours()).padStart(2, '0');
  const mm = String(date.getMinutes()).padStart(2, '0');
  const ss = String(date.getSeconds()).padStart(2, '0');
  const ms = String(date.getMilliseconds()).padStart(3, '0');
  return `${hh}:${mm}:${ss}.${ms}`;
}

function formatAuditSource(source) {
  if (source === 'CorrectionsMap')    return 'corrections-map';
  if (source === 'BuiltinRule')       return 'rule';
  if (source === 'MigrationTable')    return 'migration';
  if (source === 'DecoderPosterior')  return 'decoder';
  return String(source ?? '').toLowerCase();
}

// ---------------------------------------------------------------------------
// Update loop — debounced post-to-worker / receive / apply
// ---------------------------------------------------------------------------

let debounceTimer = null;
let applyTimer = null;
let nextSeq = 1;
let activeSeq = 0;          // last-issued request seq
let activeRequestText = '';
let lastSettledText = null; // last text we've successfully processed
let lastDocChangeAt = 0;
const pendingFixRequests = new Map();

function clearPendingApply() {
  if (applyTimer !== null) {
    clearTimeout(applyTimer);
    applyTimer = null;
  }
}

function deferApplyWhileTyping(view, refs, msg) {
  const idleForMs = performance.now() - lastDocChangeAt;
  if (idleForMs >= APPLY_IDLE_MS) return false;

  debugTimingLog('apply-deferred', {
    seq: msg.seq,
    idleForMs: Number(idleForMs.toFixed(1)),
    waitMs: Number((APPLY_IDLE_MS - idleForMs).toFixed(1)),
  });

  clearPendingApply();
  applyTimer = setTimeout(() => {
    applyTimer = null;
    applyFixResult(view, refs, msg);
  }, APPLY_IDLE_MS - idleForMs);
  return true;
}

function scheduleUpdate(view, refs) {
  clearTimeout(debounceTimer);
  debugTimingLog('update-scheduled', { delayMs: DEBOUNCE_MS, docLen: view.state.doc.length });
  debounceTimer = setTimeout(() => requestUpdate(view, refs), DEBOUNCE_MS);
}

function requestUpdate(view, refs, { force = false } = {}) {
  const text = view.state.doc.toString();
  if (!force && text === lastSettledText) return;
  activeSeq = nextSeq++;
  activeRequestText = text;
  pendingFixRequests.set(activeSeq, performance.now());
  debugTimingLog('worker-request', {
    seq: activeSeq,
    force,
    docLen: text.length,
    threshold: currentThreshold,
  });
  worker.postMessage({
    type: 'fix',
    seq: activeSeq,
    text,
    threshold: currentThreshold,
    config: DEMO_CONFIG,
  });
}

function applyFixResult(view, refs, msg) {
  const requestStartedAt = pendingFixRequests.get(msg.seq);
  const roundTripMs = typeof requestStartedAt === 'number'
    ? performance.now() - requestStartedAt
    : null;

  // Drop stale results — a newer request has already been issued.
  if (msg.seq !== activeSeq) {
    debugTimingLog('worker-stale', {
      seq: msg.seq,
      activeSeq,
      roundTripMs: roundTripMs === null ? null : Number(roundTripMs.toFixed(1)),
    });
    pendingFixRequests.delete(msg.seq);
    return;
  }

  debugTimingLog('worker-result', {
    seq: msg.seq,
    roundTripMs: roundTripMs === null ? null : Number(roundTripMs.toFixed(1)),
    applied: (msg.applied || []).length,
    remaining: (msg.remaining || []).length,
  });

  if (deferApplyWhileTyping(view, refs, msg)) return;

  const currentText = view.state.doc.toString();

  // Worker replies are only safe to apply against the exact snapshot they
  // were computed from. Offset-in-bounds is not enough once the user keeps
  // typing — applying a valid-but-stale span is what can shove subsequent
  // keystrokes into the wrong visual position.
  if (currentText !== activeRequestText) {
    debugTimingLog('apply-skipped-text-mismatch', {
      seq: msg.seq,
      currentLen: currentText.length,
      requestLen: activeRequestText.length,
    });
    scheduleUpdate(view, refs);
    return;
  }

  // Build CodeMirror change set from the applied-fix spans.
  // The two-pass fix pipeline can produce multiple applied fixes targeting
  // the same span (e.g., pass-1 C001 'SERCET'→'SECRET' then pass-2 E009
  // 'SECRET'→'S' at the same offset). CodeMirror cannot apply two changes
  // at the same range in one transaction, so we deduplicate by span key,
  // keeping only the LAST fix per span for the change set. Each individual
  // fix still gets its own audit entry below — the audit log shows what
  // the engine did; the change set shows the net effect.
  if (msg.applied.length > 0) {
    const bySpan = new Map();
    for (const f of msg.applied) {
      bySpan.set(`${f.span.start}:${f.span.end}`, f);
    }
    const changes = [...bySpan.values()]
      .map(f => ({ from: f.span.start, to: f.span.end, insert: f.replacement }))
      .sort((a, b) => a.from - b.from);
    view.dispatch({ changes });
  }

  // After patching, the editor's text equals msg.fixedText.
  lastSettledText = msg.fixedText;

  // Decorations from the remaining diagnostics — operate on the post-fix text.
  const fixedText = msg.fixedText;
  const decorationRanges = [];
  const diagData = [];
  for (const d of (msg.remaining || [])) {
    const from = d.span?.start ?? 0;
    const to   = d.span?.end   ?? from;
    if (from >= to || to > fixedText.length) continue;
    const cls = d.severity === 'error' ? 'marque-error' : 'marque-warn';
    decorationRanges.push(Decoration.mark({ class: cls }).range(from, to));
    diagData.push({ from, to, rule: d.rule, message: d.message, citation: d.citation });
  }
  decorationRanges.sort((a, b) => a.from - b.from);
  view.dispatch({ effects: setDiagnosticsEffect.of(Decoration.set(decorationRanges)) });
  view._marqueDiagData = diagData;

  // Banner.
  applyBanner(msg.banner || 'UNCLASSIFIED', refs.topBanner, refs.bottomBanner);
  refs.lastBanner = msg.banner || 'UNCLASSIFIED';
  refs.lastFixedText = fixedText;

  // Audit entries — one row per applied fix, separator between batches.
  if (msg.applied.length > 0) {
    for (const f of msg.applied) {
      enqueueAuditEntry(f, refs.auditStream, refs.auditEmpty);
    }
  }

  // NOFORN-clears-REL-TO supersession callout. The PageRewrite that
  // implements `capco/noforn-clears-rel-to` doesn't surface as a fix —
  // it changes the banner roll-up. Detect heuristically: any portion
  // marking on the page contains NOFORN/NF AND any portion contains
  // REL TO, but the post-rewrite banner has dropped the REL TO list.
  maybeEmitNofornCallout(fixedText, refs.lastBanner, refs);

  // Side rail: threshold counter + histogram + remaining-diagnostics list.
  updateThresholdCounter(msg.applied, msg.remaining || [], refs);
  updateConfidenceHistogram(msg.applied, msg.remaining || [], refs);
  updateRemainingPanel(view, msg.remaining || [], refs);

  // Multi-page indicator — derives from the post-fix text.
  updatePageCount(fixedText, refs);
  pendingFixRequests.delete(msg.seq);
  debugTimingLog('apply-complete', {
    seq: msg.seq,
    fixedLen: fixedText.length,
    applied: (msg.applied || []).length,
  });
}

// ---------------------------------------------------------------------------
// Side-rail: threshold counter, histogram, remaining-diagnostics panel
// ---------------------------------------------------------------------------

function fixableCount(applied, remaining) {
  // Total diagnostics that have a fix proposal at all (regardless of where
  // they landed against the threshold).
  let total = applied.length;
  for (const r of remaining) if (r.fix) total++;
  return total;
}

function updateThresholdCounter(applied, remaining, refs) {
  const total = fixableCount(applied, remaining);
  const pct = Math.round(currentThreshold * 100);
  if (total === 0) {
    refs.thresholdCounter.textContent = '—';
    return;
  }
  // The trailing percentage is the threshold setting (the slider value),
  // not a per-fix confidence. Phrase as "above N%" so it reads as the
  // qualifying bar rather than as an averaged confidence.
  refs.thresholdCounter.textContent =
    `${applied.length} of ${total} above ${pct}%`;
}

function updateConfidenceHistogram(applied, remaining, refs) {
  const root = refs.histogram;
  const buckets = new Array(HISTOGRAM_BUCKETS).fill(0);

  const push = (c) => {
    if (typeof c !== 'number' || !Number.isFinite(c)) return;
    const clamped = Math.max(0, Math.min(1, c));
    let idx = Math.floor(clamped * HISTOGRAM_BUCKETS);
    if (idx === HISTOGRAM_BUCKETS) idx = HISTOGRAM_BUCKETS - 1;
    buckets[idx]++;
  };
  for (const a of applied) push(a.confidence);
  for (const r of remaining) if (r.fix) push(r.fix.confidence);

  const max = Math.max(1, ...buckets);
  const tIdx = Math.min(
    HISTOGRAM_BUCKETS - 1,
    Math.floor(currentThreshold * HISTOGRAM_BUCKETS),
  );

  // Reuse / create bar elements.
  while (root.children.length < HISTOGRAM_BUCKETS) {
    const bar = document.createElement('span');
    bar.className = 'histo-bar';
    root.appendChild(bar);
  }
  while (root.children.length > HISTOGRAM_BUCKETS) {
    root.removeChild(root.lastChild);
  }

  for (let i = 0; i < HISTOGRAM_BUCKETS; i++) {
    const bar = root.children[i];
    const h = (buckets[i] / max) * 100;
    bar.style.height = buckets[i] === 0 ? '2px' : `${Math.max(6, h)}%`;
    bar.classList.remove('is-above-threshold', 'is-below-threshold', 'is-at-threshold');
    if (i === tIdx) bar.classList.add('is-at-threshold');
    else if (i > tIdx) bar.classList.add('is-above-threshold');
    else bar.classList.add('is-below-threshold');
    bar.title = `${(i / HISTOGRAM_BUCKETS).toFixed(2)}–${((i + 1) / HISTOGRAM_BUCKETS).toFixed(2)}: ${buckets[i]}`;
  }
}

function updateRemainingPanel(view, remaining, refs) {
  const list = refs.remainingList;
  const empty = refs.remainingEmpty;
  const count = refs.remainingCount;

  if (!list || !empty || !count) {
    return;
  }

  // Wipe and rebuild — small lists, simple wins.
  list.replaceChildren();

  if (remaining.length === 0) {
    list.appendChild(empty);
    count.textContent = 'none';
    return;
  }
  count.textContent = `${remaining.length}`;

  for (const d of remaining) {
    const item = document.createElement('li');
    item.className = `remaining-item sev-${d.severity || 'info'}`;
    item.tabIndex = 0;

    const head = document.createElement('div');
    head.className = 'remaining-head';
    const ruleEl = document.createElement('span');
    ruleEl.className = 'remaining-rule';
    ruleEl.textContent = d.rule;
    head.appendChild(ruleEl);
    if (d.fix && typeof d.fix.confidence === 'number') {
      const conf = document.createElement('span');
      conf.className = 'remaining-confidence';
      conf.textContent = `${Math.round(d.fix.confidence * 100)}% conf`;
      head.appendChild(conf);
    }
    item.appendChild(head);

    const message = document.createElement('div');
    message.className = 'remaining-message';
    message.textContent = d.message || '';
    item.appendChild(message);

    if (d.citation) {
      const cite = document.createElement('div');
      cite.className = 'remaining-citation';
      cite.textContent = d.citation;
      item.appendChild(cite);
    }

    item.addEventListener('click', () => focusEditorSpan(view, d.span));
    item.addEventListener('keydown', (ev) => {
      if (ev.key === 'Enter' || ev.key === ' ') {
        ev.preventDefault();
        focusEditorSpan(view, d.span);
      }
    });
    list.appendChild(item);
  }
}

function focusEditorSpan(view, span) {
  if (!span) return;
  const len = view.state.doc.length;
  const from = Math.max(0, Math.min(len, span.start ?? 0));
  const to   = Math.max(from, Math.min(len, span.end ?? from));
  view.focus();
  view.dispatch({
    selection: { anchor: from, head: to },
    scrollIntoView: true,
  });
}

// ---------------------------------------------------------------------------
// NOFORN-supersession callout
//
// `capco/noforn-clears-rel-to` is a PageRewrite, not a FixProposal — it
// changes the banner roll-up rather than rewriting source bytes. Detect
// heuristically: any portion contains NOFORN/NF, any portion contains
// REL TO, and the resulting banner has dropped the REL TO list.
// ---------------------------------------------------------------------------

let lastNofornState = false;

const PORTION_RE = /\(([^)]+)\)/g;

function maybeEmitNofornCallout(fixedText, banner, refs) {
  let hasNoforn = false;
  let hasRelTo  = false;
  for (const m of fixedText.matchAll(PORTION_RE)) {
    const inner = m[1].toUpperCase();
    if (/\bNOFORN\b|\bNF\b/.test(inner)) hasNoforn = true;
    if (/REL\s+TO\b/.test(inner)) hasRelTo = true;
  }
  const bannerLacksRelTo = !banner.toUpperCase().includes('REL TO');
  const fired = hasNoforn && hasRelTo && bannerLacksRelTo;

  // Only emit when transitioning into the fired state — avoids spamming
  // the audit stream with one callout per keystroke.
  if (fired && !lastNofornState) {
    prependNofornCallout(refs.auditStream, refs.auditEmpty);
  }
  lastNofornState = fired;
}

function prependNofornCallout(stream, emptyEl) {
  const el = document.createElement('div');
  el.className = 'audit-callout';
  el.setAttribute('role', 'note');

  const tag = document.createElement('span');
  tag.className = 'audit-callout-tag';
  tag.textContent = 'lattice rule';
  el.appendChild(tag);

  const body = document.createElement('span');
  body.className = 'audit-callout-body';
  body.textContent =
    'REL markings are stripped from the banner when a portion contains NOFORN';
  el.appendChild(body);

  const rule = document.createElement('span');
  rule.className = 'audit-callout-rule';
  rule.textContent = 'capco/noforn-clears-rel-to';
  el.appendChild(rule);

  prependAuditNode(el, stream, emptyEl);
}

// ---------------------------------------------------------------------------
// Banner-derivation tooltip
//
// On focus or hover of either banner, parse all portion markings out of the
// post-fix text and list them. Doesn't reach into the engine — the surface
// area we need (which portions contributed) is already visible in the source.
// ---------------------------------------------------------------------------

function attachBannerTooltip(refs) {
  const tt = refs.bannerTooltip;
  if (!tt) return;

  function show(target) {
    const banner = refs.lastBanner || 'UNCLASSIFIED';
    const text   = refs.lastFixedText || '';
    const portions = [...text.matchAll(PORTION_RE)].map(m => m[1].trim());

    tt.replaceChildren();
    const title = document.createElement('div');
    title.className = 'banner-tooltip-title';
    title.textContent = 'Banner derivation';
    tt.appendChild(title);

    const ruleLine = document.createElement('div');
    ruleLine.className = 'banner-tooltip-rule';
    ruleLine.textContent =
      'max(classification) ∪ caveats across all portions on the page';
    tt.appendChild(ruleLine);

    if (portions.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'banner-tooltip-empty';
      empty.textContent = 'No portion markings yet — banner is the default.';
      tt.appendChild(empty);
    } else {
      const list = document.createElement('div');
      list.className = 'banner-tooltip-portions';
      const seen = new Set();
      for (const p of portions) {
        if (seen.has(p)) continue;
        seen.add(p);
        const li = document.createElement('span');
        li.textContent = `(${p})`;
        list.appendChild(li);
        if (seen.size >= 8) break;
      }
      tt.appendChild(list);
      if (portions.length > seen.size) {
        const more = document.createElement('span');
        more.className = 'banner-tooltip-empty';
        more.textContent = `…and ${portions.length - seen.size} more.`;
        tt.appendChild(more);
      }
    }

    const rect = target.getBoundingClientRect();
    // Position centered horizontally just under the banner.
    tt.setAttribute('aria-hidden', 'false');
    tt.dataset.open = 'true';
    // Render once to measure, then position.
    const ttRect = tt.getBoundingClientRect();
    let left = rect.left + rect.width / 2 - ttRect.width / 2;
    left = Math.max(8, Math.min(window.innerWidth - ttRect.width - 8, left));
    let top = rect.bottom + 6;
    if (top + ttRect.height > window.innerHeight - 8) {
      top = rect.top - ttRect.height - 6;
    }
    tt.style.left = `${left + window.scrollX}px`;
    tt.style.top  = `${top + window.scrollY}px`;
  }

  function hide() {
    tt.setAttribute('aria-hidden', 'true');
    tt.dataset.open = 'false';
  }

  for (const el of [refs.topBanner, refs.bottomBanner]) {
    el.addEventListener('mouseenter', () => show(el));
    el.addEventListener('mouseleave', hide);
    el.addEventListener('focus',      () => show(el));
    el.addEventListener('blur',       hide);
  }
}

// ---------------------------------------------------------------------------
// Threshold control wiring
// ---------------------------------------------------------------------------

function attachThresholdControls(view, refs) {
  const slider = refs.thresholdSlider;
  const valueEl = refs.thresholdValue;

  function setThreshold(t, opts = {}) {
    currentThreshold = Math.max(0, Math.min(1, Number(t) || 0));
    valueEl.textContent = `Fix Threshold: ${Math.round(currentThreshold * 100)}% Confidence`;
    if (slider.value !== String(currentThreshold)) {
      slider.value = String(currentThreshold);
    }
    if (!opts.silent) {
      // Force a fresh request even if the document hasn't changed — only
      // the threshold has, and we want the side rail to reflect that.
      requestUpdate(view, refs, { force: true });
    }
  }

  slider.addEventListener('input', () => setThreshold(slider.value));

  // Seed the readout — but don't force-issue a request before the worker
  // has even processed the initial empty-doc lint.
  setThreshold(currentThreshold, { silent: true });
}

// ---------------------------------------------------------------------------
// Multi-page support — page-break insertion + per-page banner ladder
// ---------------------------------------------------------------------------

const PAGE_BREAK = '\f';

function splitPages(text) {
  // Pages are separated by form-feed (\f) — what the engine treats as a hard
  // page break. Empty trailing pages are dropped so the page count matches
  // what the user sees.
  const raw = text.split(PAGE_BREAK);
  const pages = [];
  for (let i = 0; i < raw.length; i++) {
    const page = raw[i];
    if (i === raw.length - 1 && page.length === 0) continue;
    pages.push(page);
  }
  return pages.length > 0 ? pages : [''];
}

function attachPageBreakButton(view, refs) {
  const btn = refs.insertPageBreak;
  if (!btn) return;
  btn.addEventListener('click', () => {
    const sel = view.state.selection.main;
    const insert = (sel.from === 0 ? '' : '\n') + PAGE_BREAK + '\n';
    view.dispatch({
      changes: { from: sel.from, to: sel.to, insert },
      selection: { anchor: sel.from + insert.length },
    });
    view.focus();
  });
}

function updatePageCount(text, refs) {
  if (!refs.docPageCount) return;
  const pages = splitPages(text);
  if (pages.length <= 1) {
    refs.docPageCount.textContent = '';
    return;
  }
  refs.docPageCount.textContent = `${pages.length} pages · banner resets per page`;
}

// ---------------------------------------------------------------------------
// Scenario tabs — U.S. memo vs FGI / JOINT
// ---------------------------------------------------------------------------

const SCENARIOS = {
  us: {
    note: 'Standard U.S. classification with SCI, SAR, and dissem controls.',
    seed:
      'This memo summarizes the program review.\n\n' +
      '(SERCET//NOFORN) Initial findings indicate the system is operating ' +
      'within nominal parameters.\n\n' +
      '(C) Status reports will continue on a weekly cadence.\n',
  },
  fgi: {
    note:
      'Foreign Government Information, JOINT classification, and NATO ' +
      'tetragraphs. CAPCO §H.7 country-code ordering applies.',
    seed:
      'JOINT memo — coalition program coordination.\n\n' +
      '(//FGI GBR S) Analytical assessment shared by the United Kingdom.\n\n' +
      '(//JOINT SECRET USA, GBR, AUS) Combined operational summary; ' +
      'releasability follows the lowest classifier.\n\n' +
      '(//SECRET//REL TO USA, FVEY) Distribution limited to Five Eyes ' +
      'partners — note tetragraph form.\n',
  },
};

function attachScenarioTabs(view, refs) {
  const tabs = refs.scenarioTabs;
  if (!tabs || tabs.length === 0) return;

  for (const tab of tabs) {
    tab.addEventListener('click', () => {
      const key = tab.dataset.scenario;
      const scenario = SCENARIOS[key];
      if (!scenario) return;
      for (const t of tabs) {
        const active = t === tab;
        t.classList.toggle('is-active', active);
        t.setAttribute('aria-selected', active ? 'true' : 'false');
      }
      if (refs.scenarioNote) refs.scenarioNote.textContent = scenario.note;

      // Replace the document content with the scenario seed. This is a hard
      // reset: we want to demonstrate a different domain, not splice into
      // existing prose.
      const len = view.state.doc.length;
      view.dispatch({
        changes: { from: 0, to: len, insert: scenario.seed },
        selection: { anchor: scenario.seed.length },
      });
      view.focus();
    });
  }
}

// ---------------------------------------------------------------------------
// Idle autoplay
// ---------------------------------------------------------------------------

function makeAutoplay(view) {
  let charIdx = 0;
  let typingTimer = null;
  let idleTimer   = null;
  let demoTipTimer = null;
  let demoTipEl   = null;
  let aborted     = false;

  function tick() {
    if (aborted) return;
    if (charIdx >= AUTOPLAY_SCRIPT.length) {
      // Done typing. After the final fix has settled, pop a demo
      // tooltip over the SI-G error so a viewer sees what an
      // unfixable diagnostic looks like — that's the showcase that
      // the autoplay was building toward.
      demoTipTimer = setTimeout(() => {
        if (!aborted) showSiGDemoTooltip(view);
      }, 1500);
      return;
    }
    const ch = AUTOPLAY_SCRIPT[charIdx++];
    const pos = view.state.doc.length;
    view.dispatch({
      changes: { from: pos, to: pos, insert: ch },
      // Keep the cursor at the inserted position so subsequent inserts append.
      selection: { anchor: pos + ch.length },
    });
    // Pause longer after newlines so the audit log can catch up and the
    // reader gets a beat between paragraphs. `\n\n` is a paragraph break
    // (we just typed the second `\n`); a single `\n` is a sentence end.
    let delay = AUTOPLAY_CHAR_MS;
    if (ch === '\n') {
      const prev = AUTOPLAY_SCRIPT[charIdx - 2];
      delay = prev === '\n' ? AUTOPLAY_PARAGRAPH_MS : AUTOPLAY_NEWLINE_MS;
    }
    typingTimer = setTimeout(tick, delay);
  }

  function showSiGDemoTooltip(view) {
    // Find the SI-G diagnostic the engine emitted. Prefer rule
    // matching, fall back to message-text matching for resilience.
    const diags = view._marqueDiagData || [];
    const target =
      diags.find(d => d.rule === 'E047') ||
      diags.find(d => /SI-G/.test(d.message || ''));
    if (!target) return;
    // CodeMirror coords are page-relative; absolute positioning on
    // body is the simplest anchor.
    const midPos = target.from + Math.floor((target.to - target.from) / 2);
    const coords = view.coordsAtPos(Math.min(midPos, view.state.doc.length));
    if (!coords) return;
    removeDemoTooltip();
    const dom = document.createElement('div');
    dom.className = 'marque-tooltip marque-tooltip--demo';
    const rule = document.createElement('div');
    rule.className = 'tip-rule'; rule.textContent = target.rule;
    const msg  = document.createElement('div');
    msg.className = 'tip-message'; msg.textContent = target.message;
    const cite = document.createElement('div');
    cite.className = 'tip-citation'; cite.textContent = target.citation;
    dom.append(rule, msg, cite);
    document.body.appendChild(dom);
    // Position above the SI-G token. After append we know the
    // tooltip's height so we can offset it correctly.
    const rect = dom.getBoundingClientRect();
    const left = Math.max(8, coords.left - rect.width / 2);
    const top  = (coords.top + window.scrollY) - rect.height - 10;
    dom.style.left = `${left}px`;
    dom.style.top  = `${top}px`;
    demoTipEl = dom;
    // Auto-dismiss so the demo doesn't leave the tooltip stuck on
    // screen forever.
    demoTipTimer = setTimeout(removeDemoTooltip, 6000);
  }

  function removeDemoTooltip() {
    if (demoTipEl && demoTipEl.parentNode) {
      demoTipEl.parentNode.removeChild(demoTipEl);
    }
    demoTipEl = null;
  }

  function start() {
    if (aborted) return;
    if (view.state.doc.length > 0) return; // user already typed
    charIdx = 0;
    tick();
  }

  function abort() {
    if (aborted) return;
    aborted = true;
    clearTimeout(typingTimer);
    clearTimeout(idleTimer);
    clearTimeout(demoTipTimer);
    removeDemoTooltip();
  }

  idleTimer = setTimeout(start, AUTOPLAY_IDLE_MS);

  return { abort };
}

function installRecorderApi(view) {
  const resolveEditorScrollTarget = () => {
    const cmScroller = view.scrollDOM || view.dom.querySelector('.cm-scroller');
    if (cmScroller && cmScroller.scrollHeight > cmScroller.clientHeight + 1) {
      return cmScroller;
    }

    const docBody = view.dom.closest('.doc-body') || document.querySelector('.doc-body');
    if (docBody) return docBody;

    return cmScroller;
  };

  const scrollEditorToBottom = () => {
    const target = resolveEditorScrollTarget();
    if (!target) return;
    target.scrollTop = target.scrollHeight;
  };

  // appendText is synchronous (returns immediately). It chains setTimeout
  // calls internally so the browser event loop handles cadence with zero
  // Playwright IPC overhead. `window.__marqueDemoRecorder.busy` is true
  // while typing is in progress — Playwright polls this to know when done.
  const appendText = (text, timing = {}) => {
    const charMs = Number.isFinite(timing.charMs) ? timing.charMs : 30;
    const sentencePauseMs = Number.isFinite(timing.sentencePauseMs) ? timing.sentencePauseMs : 0;
    const commaPauseMs = Number.isFinite(timing.commaPauseMs) ? timing.commaPauseMs : 0;

    const chars = [...String(text ?? '')];
    if (chars.length === 0) return;

    window.__marqueDemoRecorder.busy = true;
    let index = 0;
    let prevCh = '';

    const typeNext = () => {
      if (index >= chars.length) {
        window.__marqueDemoRecorder.busy = false;
        return;
      }
      const ch = chars[index++];
      const from = view.state.doc.length;
      view.dispatch({ changes: { from, to: from, insert: ch } });
      // Keep the latest typed text visible once the page starts overflowing.
      scrollEditorToBottom();

      let wait = charMs > 0 ? charMs : 0;
      if (ch === ' ') {
        if ('.!?'.includes(prevCh) && sentencePauseMs > 0) wait += sentencePauseMs;
        else if (',;'.includes(prevCh) && commaPauseMs > 0) wait += commaPauseMs;
      }
      prevCh = ch;
      setTimeout(typeNext, wait);
    };

    setTimeout(typeNext, 0);
  };

  const clearDocument = () => {
    const len = view.state.doc.length;
    if (len > 0) {
      view.dispatch({ changes: { from: 0, to: len, insert: '' } });
    }
  };

  const setThreshold = (value) => {
    const slider = document.getElementById('threshold-slider');
    if (!slider) return;
    slider.value = String(value);
    slider.dispatchEvent(new Event('input', { bubbles: true }));
    slider.dispatchEvent(new Event('change', { bubbles: true }));
  };

  const fadeHeader = () => {
    const header = document.querySelector('.doc-header');
    if (header) header.classList.add('is-hidden');
  };

  const scrollEditor = (offset) => {
    const target = resolveEditorScrollTarget();
    if (!target) return;
    const delta = Number(offset) || 0;
    target.scrollTo({ top: target.scrollTop + delta, behavior: 'smooth' });
  };

  let ejectionCount = 0;

  const fifoText = (eject) => {
    const textWrapper = document.querySelector('#editor-mount > div > div.cm-scroller > div');
    if (!textWrapper) return;
    const textElements = textWrapper.querySelectorAll('.cm-line');
    const texts = Array.from(textElements);
    if (texts.length === 0) return;
    texts.slice(0, eject + ejectionCount).forEach(el => el.style.display = 'none');
    ejectionCount += eject;
  };

  window.__marqueDemoRecorder = {
    ready: true,
    busy: false,
    appendText,
    clearDocument,
    setThreshold,
    fadeHeader,
    scrollEditor,
    fifoText,
  };
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

async function main() {
  await workerReadyPromise;

  const refs = {
    topBanner:    document.getElementById('banner-top'),
    bottomBanner: document.getElementById('banner-bottom'),
    auditStream:  document.getElementById('audit-stream'),
    auditEmpty:   document.getElementById('audit-empty'),
    bannerTooltip:    document.getElementById('banner-tooltip'),
    thresholdSlider:  document.getElementById('threshold-slider'),
    thresholdValue:   document.getElementById('threshold-value'),
    thresholdCounter: document.getElementById('threshold-counter'),
    histogram:        document.getElementById('confidence-histogram'),
    remainingList:    document.getElementById('remaining-list'),
    remainingEmpty:   document.getElementById('remaining-empty'),
    remainingCount:   document.getElementById('remaining-count'),
    contrastStrip:    document.getElementById('contrast-strip'),
    contrastDismiss:  document.getElementById('contrast-dismiss'),
    insertPageBreak:  document.getElementById('insert-page-break'),
    docPageCount:     document.getElementById('doc-page-count'),
    scenarioTabs:     document.querySelectorAll('.scenario-tab'),
    scenarioNote:     document.getElementById('scenario-note'),
    lastBanner: 'UNCLASSIFIED',
    lastFixedText: '',
  };

  // Contrast strip: dismissable, persisted in localStorage.
  if (refs.contrastStrip && refs.contrastDismiss) {
    try {
      if (localStorage.getItem('marque-demo:contrast-dismissed') === '1') {
        refs.contrastStrip.hidden = true;
      }
    } catch { /* localStorage may be unavailable */ }
    refs.contrastDismiss.addEventListener('click', () => {
      refs.contrastStrip.hidden = true;
      try { localStorage.setItem('marque-demo:contrast-dismissed', '1'); }
      catch { /* ignore */ }
    });
  }

  // Initial empty banner — UNCLASSIFIED.
  applyBanner('UNCLASSIFIED', refs.topBanner, refs.bottomBanner);

  // Banner-derivation tooltip — hover/focus on either banner.
  attachBannerTooltip(refs);

  // Worker → main: receive results, drop stale ones.
  worker.addEventListener('message', (ev) => {
    const msg = ev.data;
    if (!msg) return;
    if (msg.type === 'fix:result')   applyFixResult(view, refs, msg);
    else if (msg.type === 'error')   console.error('[marque worker]', msg);
  });

  // Tooltip extension reads from view._marqueDiagData.
  const tooltip = hoverTooltip((view, pos) => {
    const diags = view._marqueDiagData || [];
    const match = diags.find(d => pos >= d.from && pos < d.to);
    if (!match) return null;
    return {
      pos: match.from,
      end: match.to,
      above: true,
      create() {
        const dom = document.createElement('div');
        dom.className = 'marque-tooltip';
        const rule = document.createElement('div');
        rule.className = 'tip-rule'; rule.textContent = match.rule;
        const msg  = document.createElement('div');
        msg.className = 'tip-message'; msg.textContent = match.message;
        const cite = document.createElement('div');
        cite.className = 'tip-citation'; cite.textContent = match.citation;
        dom.append(rule, msg, cite);
        return { dom };
      },
    };
  });

  let autoplay; // forward declaration so updateListener can abort

  const startState = EditorState.create({
    doc: '',
    extensions: [
      diagnosticsField,
      placeholderPlugin,
      pageBreakPlugin,
      scriptedStylePlugin,
      tooltip,
      EditorView.lineWrapping,
      EditorView.theme({
        '&': { background: 'transparent', color: 'var(--color-text-primary)' },
        '.cm-content': {
          fontFamily: 'var(--font-body)',
          fontSize: '14px',
          lineHeight: '1.85',
          caretColor: 'var(--color-text-primary)',
        },
        '.cm-cursor': { borderLeftColor: 'var(--color-text-primary)' },
        '.cm-selectionBackground': { background: 'rgba(126, 152, 218, 0.3) !important' },
        '&.cm-focused .cm-selectionBackground': {
          background: 'rgba(126, 152, 218, 0.4) !important',
        },
        '.cm-gutters': { display: 'none' },
        '.cm-activeLine': { background: 'transparent' },
        '.cm-activeLineGutter': { background: 'transparent' },
      }, { dark: false }),
      EditorView.updateListener.of(update => {
        if (!update.docChanged) return;
        lastDocChangeAt = performance.now();
        clearPendingApply();
        const newText = update.view.state.doc.toString();
        if (newText === lastSettledText) return;
        scheduleUpdate(update.view, refs);
      }),
    ],
  });

  const view = new EditorView({
    state: startState,
    parent: document.getElementById('editor-mount'),
  });

  installRecorderApi(view);

  // Threshold controls — wire after the view exists so chips/slider can
  // re-issue requestUpdate against the editor.
  attachThresholdControls(view, refs);

  // Tier 3 wiring — page-break, scenario tabs.
  attachPageBreakButton(view, refs);
  attachScenarioTabs(view, refs);

  // Pause autoplay on any user input (keystroke, paste, click that sets focus).
  // The autoplay's own dispatches set the 'autoplay' annotation, so we
  // detect human input by absence — abort whenever a non-tagged docChange
  // occurs after autoplay started. To keep this simple, we abort on the
  // first user-driven 'keydown' or 'beforeinput' event in the editor.
  const abortAutoplay = () => { if (autoplay) autoplay.abort(); };
  view.dom.addEventListener('keydown',     abortAutoplay, { once: true });
  view.dom.addEventListener('beforeinput', abortAutoplay, { once: true });
  view.dom.addEventListener('paste',       abortAutoplay, { once: true });

  autoplay = makeAutoplay(view);
}

main().catch(err => {
  console.error('[marque demo] init failed:', err);
});
