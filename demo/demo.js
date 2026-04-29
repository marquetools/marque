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

const FIX_THRESHOLD = 0.0;        // Tier 2 will surface this as a slider.
const DEBOUNCE_MS   = 80;
const AUTOPLAY_IDLE_MS = 6_000;
const AUTOPLAY_CHAR_MS = 70;
const PLACEHOLDER_TEXT =
  'Try typing — for example: This is a (sercet//noforn) test memo.';

// Autoplay script: a short representative passage that exercises typo
// correction (SERCET → SECRET → S), portion abbreviation, and banner
// roll-up. Newlines are part of the script.
const AUTOPLAY_SCRIPT =
  'This memo summarizes the program review.\n\n' +
  '(SERCET//NOFORN) Initial findings indicate the system is operating ' +
  'within nominal parameters.\n\n' +
  '(C) Status reports will continue on a weekly cadence.\n';

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

class PlaceholderWidget {
  constructor(text) { this.text = text; }
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
// Banner classification → CSS class
// ---------------------------------------------------------------------------

const LEVEL_CLASSES = [
  ['TOP SECRET', 'level-ts'],
  ['SECRET',     'level-secret'],
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
  'level-ts', 'level-ts-sci', 'level-empty',
];

function applyBanner(banner, topEl, bottomEl) {
  const cls = classificationClass(banner);
  topEl.classList.remove(...ALL_LEVEL_CLASSES);
  bottomEl.classList.remove(...ALL_LEVEL_CLASSES);
  topEl.classList.add(cls);
  bottomEl.classList.add(cls);
  topEl.textContent = banner;
  bottomEl.textContent = banner;
}

// ---------------------------------------------------------------------------
// Audit stream
// ---------------------------------------------------------------------------

let auditEntryCount = 0;

function prependAuditEntry(record, stream, emptyEl) {
  if (emptyEl && emptyEl.parentNode === stream) {
    stream.removeChild(emptyEl);
  }

  const now = new Date();
  const hh = String(now.getHours()).padStart(2, '0');
  const mm = String(now.getMinutes()).padStart(2, '0');
  const ss = String(now.getSeconds()).padStart(2, '0');
  const timeStr = `${hh}:${mm}:${ss}`;

  const sourceLabel = record.source === 'CorrectionsMap' ? 'corrections-map'
    : record.source === 'BuiltinRule'    ? 'rule'
    : record.source === 'MigrationTable' ? 'migration'
    : record.source === 'DecoderPosterior' ? 'decoder'
    : String(record.source).toLowerCase();

  const pct = Math.round((record.confidence ?? 1) * 100);

  const entry = document.createElement('div');
  entry.className = 'audit-entry';
  entry.setAttribute('role', 'log');

  const fields = [
    ['audit-time',        timeStr],
    ['audit-rule',        record.rule],
    ['audit-original',    record.original],
    ['audit-arrow',       '→'],
    ['audit-replacement', record.replacement],
    ['audit-source',      sourceLabel],
    ['audit-confidence',  `${pct}%`],
  ];
  for (const [cls, text] of fields) {
    const span = document.createElement('span');
    span.className = cls;
    span.textContent = text;
    entry.appendChild(span);
  }

  if (stream.firstChild) stream.insertBefore(entry, stream.firstChild);
  else stream.appendChild(entry);
  auditEntryCount++;
}

function prependAuditSeparator(stream) {
  if (auditEntryCount === 0) return;
  const hr = document.createElement('hr');
  hr.className = 'audit-separator';
  if (stream.firstChild) stream.insertBefore(hr, stream.firstChild);
  else stream.appendChild(hr);
}

// ---------------------------------------------------------------------------
// Update loop — debounced post-to-worker / receive / apply
// ---------------------------------------------------------------------------

let debounceTimer = null;
let nextSeq = 1;
let activeSeq = 0;          // last-issued request seq
let lastSettledText = null; // last text we've successfully processed

function scheduleUpdate(view, refs) {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => requestUpdate(view, refs), DEBOUNCE_MS);
}

function requestUpdate(view, refs) {
  const text = view.state.doc.toString();
  if (text === lastSettledText) return;
  activeSeq = nextSeq++;
  worker.postMessage({
    type: 'fix',
    seq: activeSeq,
    text,
    threshold: FIX_THRESHOLD,
    config: DEMO_CONFIG,
  });
}

function applyFixResult(view, refs, msg) {
  // Drop stale results — a newer request has already been issued.
  if (msg.seq !== activeSeq) return;

  const currentText = view.state.doc.toString();

  // The seqs match, but the user may have typed in the gap between the
  // request being sent and the reply arriving. If the editor's current
  // text differs from what we'd get by applying these fixes, we re-issue
  // a fresh request rather than apply stale changes.
  // (Cheap check: if there are applied fixes, their spans must still be
  // in-bounds for the current text.)
  if (msg.applied.length > 0) {
    const inBounds = msg.applied.every(f =>
      f.span.start >= 0 && f.span.end <= currentText.length
    );
    if (!inBounds) {
      scheduleUpdate(view, refs);
      return;
    }
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

  // Audit entries — one row per applied fix, separator between batches.
  if (msg.applied.length > 0) {
    prependAuditSeparator(refs.auditStream);
    for (const f of msg.applied) {
      prependAuditEntry(f, refs.auditStream, refs.auditEmpty);
    }
  }
}

// ---------------------------------------------------------------------------
// Idle autoplay
// ---------------------------------------------------------------------------

function makeAutoplay(view) {
  let charIdx = 0;
  let typingTimer = null;
  let idleTimer   = null;
  let aborted     = false;

  function tick() {
    if (aborted || charIdx >= AUTOPLAY_SCRIPT.length) return;
    const ch = AUTOPLAY_SCRIPT[charIdx++];
    const pos = view.state.doc.length;
    view.dispatch({
      changes: { from: pos, to: pos, insert: ch },
      // Keep the cursor at the inserted position so subsequent inserts append.
      selection: { anchor: pos + ch.length },
    });
    typingTimer = setTimeout(tick, AUTOPLAY_CHAR_MS);
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
  }

  idleTimer = setTimeout(start, AUTOPLAY_IDLE_MS);

  return { abort };
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
  };

  // Initial empty banner — UNCLASSIFIED.
  applyBanner('UNCLASSIFIED', refs.topBanner, refs.bottomBanner);

  // Worker → main: receive results, drop stale ones.
  worker.addEventListener('message', (ev) => {
    const msg = ev.data;
    if (!msg) return;
    if (msg.type === 'fix:result') applyFixResult(view, refs, msg);
    else if (msg.type === 'error') console.error('[marque worker]', msg);
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
