// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: Apache-2.0

/**
 * Marque Interactive Demo
 *
 * Loads the Marque WASM module and wires up the classified-memo editor:
 * - Real-time banner rollup from portion markings (compute_banner)
 * - Correct-as-you-type via fix() at threshold 0.0 — applies targeted CodeMirror
 *   changes (one per applied fix) rather than a full-document replacement, so
 *   the cursor never jumps.
 * - Squiggly underlines via CodeMirror 6 Decoration API
 * - Hover tooltips showing rule ID, message, CAPCO citation
 * - Inline audit stream: each fix produces a stylized entry prepended below the
 *   document, blending into the page background.
 *
 * WASM path: served at /wasm/ by the dev server (bin/serve.js routes that prefix
 * to crates/wasm/pkg/ in the monorepo, or to the bundled wasm/ dir when
 * running from an npm-installed package).
 */

import initWasm, { configure, lint, fix, compute_banner }
  from '/wasm/marque_wasm.js';

import {
  EditorView,
  Decoration,
  hoverTooltip,
  StateEffect,
  StateField,
  EditorState,
} from './vendor.js';

// ---------------------------------------------------------------------------
// WASM engine configuration
// ---------------------------------------------------------------------------

/**
 * Corrections map: common typos and misspellings of classification terms.
 * The engine's pre-scanner AhoCorasick automaton detects these in raw text
 * and emits C001 diagnostics before the scanner/parser even runs, enabling
 * the two-pass fix pipeline to correct typos AND apply downstream rules
 * (e.g., SERCET → SECRET → S in a portion).
 */
const DEMO_CONFIG = JSON.stringify({
  corrections: {
    'SERCET':        'SECRET',
    'SECERT':        'SECRET',
    'SECRECT':       'SECRET',
    'SCERET':        'SECRET',
    'SECRTE':        'SECRET',
    'CONFIDETIAL':   'CONFIDENTIAL',
    'CONFIENTIAL':   'CONFIDENTIAL',
    'CONFIDENTAL':   'CONFIDENTIAL',
    'UNCALSSIFIED':  'UNCLASSIFIED',
    'UNCLASSFIED':   'UNCLASSIFIED',
    'UNCLASSIFED':   'UNCLASSIFIED',
    'NOFON':         'NOFORN',
    'NOFRON':        'NOFORN',
  },
});

// ---------------------------------------------------------------------------
// Pre-loaded document body
// ---------------------------------------------------------------------------

const INITIAL_BODY = ``;

// ---------------------------------------------------------------------------
// CodeMirror decoration effects and state field
// ---------------------------------------------------------------------------

const setDiagnosticsEffect = StateEffect.define();

const diagnosticsField = StateField.define({
  create() { return Decoration.none; },
  update(decorations, tr) {
    decorations = decorations.map(tr.changes);
    for (const e of tr.effects) {
      if (e.is(setDiagnosticsEffect)) {
        decorations = e.value;
      }
    }
    return decorations;
  },
  provide: f => EditorView.decorations.from(f),
});

// ---------------------------------------------------------------------------
// Classification level → CSS class
// ---------------------------------------------------------------------------

const LEVEL_CLASSES = [
  ['TOP SECRET', 'level-ts'],
  ['SECRET', 'level-secret'],
  ['CONFIDENTIAL', 'level-confidential'],
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

function escapeHtml(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// ---------------------------------------------------------------------------
// Banner update
// ---------------------------------------------------------------------------

const ALL_LEVEL_CLASSES = [
  'level-unclassified', 'level-confidential', 'level-secret',
  'level-ts', 'level-ts-sci', 'level-empty',
];

function updateBanners(text, topEl, bottomEl) {
  let banner;
  try {
    banner = compute_banner(text);
  } catch {
    banner = 'UNCLASSIFIED';
  }
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

/**
 * Prepend a styled entry into the audit stream for one applied fix.
 *
 * @param {object} fix  - An entry from fixResult.applied (AuditRecordJson)
 * @param {Element} stream - The #audit-stream container
 * @param {Element} emptyEl - The #audit-empty placeholder element
 */
function prependAuditEntry(fix, stream, emptyEl) {
  if (emptyEl && emptyEl.parentNode === stream) {
    stream.removeChild(emptyEl);
  }

  const now = new Date();
  const hh = String(now.getHours()).padStart(2, '0');
  const mm = String(now.getMinutes()).padStart(2, '0');
  const ss = String(now.getSeconds()).padStart(2, '0');
  const timeStr = `${hh}:${mm}:${ss}`;

  const sourceLabel = fix.source === 'CorrectionsMap' ? 'corrections-map'
    : fix.source === 'BuiltinRule' ? 'rule'
    : fix.source === 'MigrationTable' ? 'migration'
    : fix.source.toLowerCase();

  const pct = Math.round((fix.confidence ?? 1) * 100);

  const entry = document.createElement('div');
  entry.className = 'audit-entry';
  entry.setAttribute('role', 'log');
  entry.innerHTML = `
    <span class="audit-time">${timeStr}</span>
    <span class="audit-rule">${escapeHtml(fix.rule)}</span>
    <span class="audit-original">${escapeHtml(fix.original)}</span>
    <span class="audit-arrow">→</span>
    <span class="audit-replacement">${escapeHtml(fix.replacement)}</span>
    <span class="audit-source">${escapeHtml(sourceLabel)}</span>
    <span class="audit-confidence">${pct}%</span>
  `;

  // Prepend: insert before the first child (or append if empty)
  if (stream.firstChild) {
    stream.insertBefore(entry, stream.firstChild);
  } else {
    stream.appendChild(entry);
  }

  auditEntryCount++;
}

/**
 * Insert a thin separator rule between correction batches (keystroke groups).
 */
function prependAuditSeparator(stream) {
  if (auditEntryCount === 0) return;
  const hr = document.createElement('hr');
  hr.className = 'audit-separator';
  if (stream.firstChild) {
    stream.insertBefore(hr, stream.firstChild);
  } else {
    stream.appendChild(hr);
  }
}

// ---------------------------------------------------------------------------
// Main debounced update loop
// ---------------------------------------------------------------------------

let debounceTimer = null;
const DEBOUNCE_MS = 80;

/** Track last-processed text to skip redundant work. */
let lastProcessedText = null;

/**
 * runUpdate — called after each debounce interval.
 *
 * Fix strategy: instead of replacing the whole document (which moves the
 * cursor), we apply each fix as a targeted CodeMirror change using the byte
 * offsets returned by fix().  All changes in one transaction means one undo
 * step and correct cursor remapping.
 *
 * CodeMirror 6 ChangeSet.of() requires changes sorted by ascending `from`
 * position and expects all positions relative to the ORIGINAL document — the
 * engine already guarantees non-overlapping spans, so we just sort and pass.
 */
function runUpdate(view, topBanner, bottomBanner, auditStream, auditEmpty) {
  const text = view.state.doc.toString();

  if (text === lastProcessedText) return;
  lastProcessedText = text;

  // 1. Call fix() — threshold 0.0 applies every suggestion.
  let fixResult;
  try {
    fixResult = JSON.parse(fix(text, 0.0, DEMO_CONFIG));
  } catch (err) {
    console.error('[marque] fix() failed:', err);
    fixResult = null;
  }

  let diagList;

  if (fixResult && fixResult.applied && fixResult.applied.length > 0) {
    // Build targeted CodeMirror changes from the applied-fix spans.
    //
    // The engine's two-pass fix pipeline may produce multiple applied fixes
    // targeting the same span (e.g., pass-1 C001 "SERCET"→"SECRET" at span
    // 52..58, then pass-2 E009 "SECRET"→"S" at the same span). CodeMirror
    // cannot apply two changes at the same range in one transaction, so we
    // deduplicate by span key, keeping only the LAST fix per span. The last
    // fix represents the final state (e.g., the net effect is "SERCET"→"S").
    const bySpan = new Map();
    for (const f of fixResult.applied) {
      const key = `${f.span.start}:${f.span.end}`;
      bySpan.set(key, f);
    }
    const changes = [...bySpan.values()]
      .map(f => ({ from: f.span.start, to: f.span.end, insert: f.replacement }))
      .sort((a, b) => a.from - b.from);

    // Dispatch as a single transaction — cursor is remapped naturally by CM.
    view.dispatch({ changes });

    // After patching the document, re-lint the fixed text for accurate spans.
    const fixedText = view.state.doc.toString();
    try {
      const ndjson = lint(fixedText, DEMO_CONFIG);
      diagList = ndjson ? parseNdjson(ndjson) : [];
    } catch (err) {
      console.error('[marque] lint() failed:', err);
      diagList = [];
    }

    // Update lastProcessedText so the change-listener doesn't loop.
    lastProcessedText = fixedText;

    // Append audit entries for each applied fix (newest first).
    prependAuditSeparator(auditStream);
    for (const f of fixResult.applied) {
      prependAuditEntry(f, auditStream, auditEmpty);
    }
  } else if (fixResult) {
    diagList = fixResult.remaining || [];
  } else {
    diagList = [];
  }

  // 2. Build CodeMirror decorations from remaining diagnostic spans.
  const currentText = view.state.doc.toString();
  const decorationRanges = [];
  const diagData = [];

  for (const d of diagList) {
    const from = d.span?.start ?? 0;
    const to   = d.span?.end   ?? from;
    if (from >= to || to > currentText.length) continue;

    const cls = d.severity === 'error' ? 'marque-error' : 'marque-warn';
    decorationRanges.push(Decoration.mark({ class: cls }).range(from, to));
    diagData.push({ from, to, rule: d.rule, message: d.message, citation: d.citation });
  }

  decorationRanges.sort((a, b) => a.from - b.from);

  view.dispatch({
    effects: setDiagnosticsEffect.of(Decoration.set(decorationRanges)),
  });

  view._marqueDiagData = diagData;

  // 3. Update banners from whatever text is now in the editor.
  updateBanners(currentText, topBanner, bottomBanner);
}

// ---------------------------------------------------------------------------
// NDJSON parser
// ---------------------------------------------------------------------------

function parseNdjson(ndjson) {
  const results = [];
  let start = 0;
  const len = ndjson.length;
  while (start < len) {
    let end = ndjson.indexOf('\n', start);
    if (end === -1) end = len;
    if (end > start) {
      try { results.push(JSON.parse(ndjson.substring(start, end))); } catch { /* skip */ }
    }
    start = end + 1;
  }
  return results;
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

async function main() {
  await initWasm();

  // Pre-warm the engine cache with the corrections map — pays AhoCorasick +
  // rule-set construction cost here, not on the first keystroke.
  configure(DEMO_CONFIG);

  const topBanner    = document.getElementById('banner-top');
  const bottomBanner = document.getElementById('banner-bottom');
  const auditStream  = document.getElementById('audit-stream');
  const auditEmpty   = document.getElementById('audit-empty');

  // Tooltip that reads from view._marqueDiagData.
  const simpleTip = hoverTooltip((view, pos) => {
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
        dom.innerHTML = `
          <div class="tip-rule">${escapeHtml(match.rule)}</div>
          <div class="tip-message">${escapeHtml(match.message)}</div>
          <div class="tip-citation">${escapeHtml(match.citation)}</div>
        `;
        return { dom };
      },
    };
  });

  // Create CodeMirror editor.
  const startState = EditorState.create({
    doc: INITIAL_BODY,
    extensions: [
      diagnosticsField,
      simpleTip,
      EditorView.lineWrapping,
      EditorView.theme({
        '&': {
          background: 'transparent',
          color: '#222',
        },
        '.cm-content': {
          fontFamily: "Georgia, 'Times New Roman', serif",
          fontSize: '14px',
          lineHeight: '1.85',
          caretColor: '#222',
        },
        '.cm-cursor': { borderLeftColor: '#222' },
        '.cm-selectionBackground': { background: '#b3d4fc !important' },
        '&.cm-focused .cm-selectionBackground': { background: '#b3d4fc !important' },
        '.cm-gutters': { display: 'none' },
        '.cm-activeLine': { background: 'transparent' },
        '.cm-activeLineGutter': { background: 'transparent' },
      }, { dark: false }),
      EditorView.updateListener.of(update => {
        if (update.docChanged) {
          // Skip the synthetic change dispatched by runUpdate itself —
          // lastProcessedText is already set to the fixed text at that point.
          const newText = update.view.state.doc.toString();
          if (newText === lastProcessedText) return;

          clearTimeout(debounceTimer);
          debounceTimer = setTimeout(() => {
            runUpdate(update.view, topBanner, bottomBanner, auditStream, auditEmpty);
          }, DEBOUNCE_MS);
        }
      }),
    ],
  });

  const view = new EditorView({
    state: startState,
    parent: document.getElementById('editor-mount'),
  });

  // Run the initial pass immediately (catches the SERCET typo in the seed text).
  runUpdate(view, topBanner, bottomBanner, auditStream, auditEmpty);
}

main().catch(console.error);
